use crate::{
    capabilities::ParameterSegmentAdvertisement,
    error::Error,
    model::{
        Account, CamtCapability, CreditCardCapability, InstituteState, OperationPermission,
        ReusableState, TanMedium, TanMediumClass, TanMediumStatus, TanMethod, TanProcess,
    },
    wire::{Segment, SegmentHeader, Value},
};

use super::camt_descriptor_matches;
use super::{component, optional_component};

/// camt.052 schema descriptors this crate can parse, in negotiation preference.
///
/// DK Anlage 3 specifies `camt.052.001.08` since v3.4; institutions that still
/// run the earlier Anlage 3 occupancy advertise `camt.052.001.02`. Both share the
/// entry and transaction-detail structure read here, so the older version is a
/// protocol-level compatibility, not an institution rule.
const SUPPORTED_CAMT_DESCRIPTORS: &[&str] = &[
    "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08",
    "urn:iso:std:iso:20022:tech:xsd:camt.052.001.02",
];

pub(super) fn apply(
    segments: &[Segment],
    state: &mut ReusableState,
    force_bpd_refresh: bool,
) -> Result<Option<Vec<Account>>, Error> {
    let received_bpd_version = bpd_version(segments)?;
    // Formals F.2 [IF3] requires clients to handle version wrap-around.
    // Changed versions replace atomically in either numeric direction; zero is
    // a dialog-transient BPD and an explicit refresh may replace same-version BPD.
    let replace_bpd = received_bpd_version
        .is_some_and(|version| version == 0 || version != state.bpd_version || force_bpd_refresh);
    let mut params = ParameterAccumulator::default();

    for segment in segments {
        let header = segment.header().ok_or(Error::InvalidResponse {
            structure: "parameters.business.segment_header",
        })?;
        if replace_bpd && is_parameter_segment(header.code) {
            params.record_advertisement(&header);
        }
        match header.code {
            b"HIBPA" => {}
            b"HIUPA" => params.upd.absorb_usage(segment, &header)?,
            b"HIUPD" => params.upd.absorb_account(segment, &header)?,
            b"HISALS" if replace_bpd => params.balance.absorb(segment, &header),
            b"HICAZS" if replace_bpd => params.cash.absorb_camt(segment, &header),
            b"HIKAZS" if replace_bpd => params.cash.absorb_legacy(&header),
            b"HIWPDS" if replace_bpd => params.depot.absorb(&header)?,
            b"HIWDUS" if replace_bpd => params.securities.absorb(segment, &header)?,
            b"HIKKUS" if replace_bpd => params.credit_card.absorb_transactions(segment, &header)?,
            b"HIKKSS" if replace_bpd => params.credit_card.absorb_balance(segment, &header)?,
            b"HIPINS" if replace_bpd => params.absorb_tan_requirements(segment, &header)?,
            b"HITANS" if replace_bpd && (6..=7).contains(&header.version) => {
                params.tan.absorb_methods(segment, &header)?;
            }
            _ => {}
        }
    }

    // Accumulate-then-commit: a parse error in the loop above returns before
    // any state write below, so a rejected response never leaves partially
    // replaced BPD or UPD state. The trailing UPD commit's missing-usage error
    // is unreachable because absorb_usage records version and usage together;
    // if a future change makes it reachable, its check must move ahead of the
    // BPD commits to keep this guarantee.
    if let Some(version) = received_bpd_version.filter(|_| replace_bpd) {
        state.bpd_version = version;
        params.advertisements.commit(state);
        params.tan.commit(state);
        params.balance.commit(state);
        params.cash.commit(state);
        params.depot.commit(state);
        params.securities.commit(state);
        params.credit_card.commit(state);
    }
    params.upd.commit(state)
}

/// Accumulated BPD/UPD facts for one parameter response, grouped by the
/// operation family that owns them. Families absorb their segments during the
/// single pass and commit to state only after the whole response parsed.
#[derive(Default)]
struct ParameterAccumulator {
    advertisements: AdvertisementParams,
    tan: TanParams,
    balance: BalanceParams,
    cash: CashTransactionParams,
    depot: DepotPositionParams,
    securities: SecuritiesTransactionParams,
    credit_card: CreditCardParams,
    upd: UpdParams,
}

impl ParameterAccumulator {
    fn record_advertisement(&mut self, header: &SegmentHeader<'_>) {
        self.advertisements
            .segments
            .push(ParameterSegmentAdvertisement::new(
                header.code.iter().copied().map(char::from).collect(),
                header.version,
            ));
        if header.code == b"HITABS" {
            self.tan.advertised_media_versions.push(header.version);
        }
    }

    // PIN/TAN B.8.1: HIPINS field 5 is the parameter DEG at
    // element 4; five scalar fields precede repeated two-component
    // operation records. One segment carries the TAN requirement of
    // every operation family, so it is absorbed at the accumulator root.
    fn absorb_tan_requirements(
        &mut self,
        segment: &Segment,
        header: &SegmentHeader<'_>,
    ) -> Result<(), Error> {
        require_version(header.version, 1, "HIPINS")?;
        self.balance.requires_tan = parse_tan_requirement(segment, "HKSAL")?;
        self.cash.camt_requires_tan = parse_tan_requirement(segment, "HKCAZ")?;
        self.cash.legacy_requires_tan = parse_tan_requirement(segment, "HKKAZ")?;
        self.depot.requires_tan = parse_tan_requirement(segment, "HKWPD")?;
        self.securities.requires_tan = parse_tan_requirement(segment, "HKWDU")?;
        self.credit_card.transactions_requires_tan = parse_tan_requirement(segment, "HKKKU")?;
        self.credit_card.balance_requires_tan = parse_tan_requirement(segment, "HKKKS")?;
        Ok(())
    }
}

#[derive(Default)]
struct AdvertisementParams {
    segments: Vec<ParameterSegmentAdvertisement>,
}

impl AdvertisementParams {
    fn commit(mut self, state: &mut ReusableState) {
        self.segments.sort_by(|left, right| {
            left.code()
                .cmp(right.code())
                .then_with(|| right.version().cmp(&left.version()))
        });
        self.segments.dedup();
        state.advertised_parameter_segments = self.segments;
    }
}

#[derive(Default)]
struct TanParams {
    methods: Vec<TanMethod>,
    advertised_media_versions: Vec<u16>,
}

impl TanParams {
    // PIN/TAN B.5.1/B.5.2: HITANS field 5 is the parameter DEG
    // at element 4; three scalar fields precede the repeated
    // method DEGs, which flatten to 21/26 components.
    fn absorb_methods(
        &mut self,
        segment: &Segment,
        header: &SegmentHeader<'_>,
    ) -> Result<(), Error> {
        self.methods
            .extend(parse_tan_methods(segment, header.version)?);
        Ok(())
    }

    fn commit(mut self, state: &mut ReusableState) {
        self.advertised_media_versions
            .sort_unstable_by(|left, right| right.cmp(left));
        self.advertised_media_versions.dedup();
        self.methods.sort_by(|left, right| {
            left.security_function
                .cmp(&right.security_function)
                .then_with(|| right.hktan_version.cmp(&left.hktan_version))
        });
        self.methods
            .dedup_by(|left, right| left.security_function == right.security_function);
        state.advertised_tan_media_versions = self.advertised_media_versions;
        state.tan_methods = self.methods;
    }
}

#[derive(Default)]
struct BalanceParams {
    capability_advertised: bool,
    advertised_versions: Vec<u16>,
    supported_versions: Vec<u16>,
    requires_tan: Option<bool>,
}

impl BalanceParams {
    // Messages C.2.1.2: HISALS fields 2-4 are segment elements
    // 1-3; only v8 adds its parameter DEG as element 4.
    fn absorb(&mut self, segment: &Segment, header: &SegmentHeader<'_>) {
        self.capability_advertised = true;
        self.advertised_versions.push(header.version);
        if header.version == 5 {
            // HBCI 2.2 VII.2.2 contains only meaning-neutral order and
            // signature limits. A malformed legacy parameter shape is
            // retained as advertised-but-unsupported, not fatal to BPD.
            if require_legacy_balance_parameters(segment).is_ok() {
                self.supported_versions.push(header.version);
            }
        } else if (6..=8).contains(&header.version) {
            self.supported_versions.push(header.version);
        }
    }

    fn commit(mut self, state: &mut ReusableState) {
        self.supported_versions
            .sort_unstable_by(|left, right| right.cmp(left));
        self.supported_versions.dedup();
        self.advertised_versions
            .sort_unstable_by(|left, right| right.cmp(left));
        self.advertised_versions.dedup();
        state.balance_versions = self.supported_versions;
        state.advertised_balance_versions = self.advertised_versions;
        state.balance_capability_advertised = self.capability_advertised;
        state.balance_requires_tan = self.requires_tan;
    }
}

#[derive(Default)]
struct CashTransactionParams {
    capability_advertised: bool,
    camt_descriptors: Vec<String>,
    camt_storage_period_days: Option<u16>,
    legacy_versions: Vec<u16>,
    camt_requires_tan: Option<bool>,
    legacy_requires_tan: Option<bool>,
}

impl CashTransactionParams {
    // Messages C.2.3.1.1.1: HICAZS field 5 is the parameter DEG
    // at element 4; its own fields are flattened by that helper.
    fn absorb_camt(&mut self, segment: &Segment, header: &SegmentHeader<'_>) {
        self.capability_advertised = true;
        if header.version == 1 {
            let (storage_period, descriptors) = parse_camt_parameters(segment);
            self.camt_storage_period_days = self.camt_storage_period_days.or(storage_period);
            self.camt_descriptors.extend(descriptors);
        }
    }

    // Messages C.2.3.1.1 and DD: HIKAZS 6/7 advertise the
    // segment version in the header. No parameter component is
    // consumed for version negotiation.
    fn absorb_legacy(&mut self, header: &SegmentHeader<'_>) {
        self.capability_advertised = true;
        if (6..=7).contains(&header.version) {
            self.legacy_versions.push(header.version);
        }
    }

    fn commit(mut self, state: &mut ReusableState) {
        self.legacy_versions
            .sort_unstable_by(|left, right| right.cmp(left));
        self.legacy_versions.dedup();
        self.camt_descriptors.sort();
        self.camt_descriptors.dedup();
        state.transaction_capability_advertised = self.capability_advertised;
        state.advertised_camt_descriptors = self.camt_descriptors.clone();
        state.camt_storage_period_days = self.camt_storage_period_days;
        state.camt_capability = SUPPORTED_CAMT_DESCRIPTORS
            .iter()
            .find_map(|supported| {
                self.camt_descriptors
                    .iter()
                    .find(|descriptor| camt_descriptor_matches(descriptor, supported))
            })
            .map(|descriptor| CamtCapability {
                descriptor: descriptor.clone(),
            });
        state.legacy_transaction_versions = self.legacy_versions;
        state.camt_requires_tan = self.camt_requires_tan;
        state.legacy_transactions_require_tan = self.legacy_requires_tan;
    }
}

#[derive(Default)]
struct DepotPositionParams {
    advertised: bool,
    versions: Vec<u16>,
    requires_tan: Option<bool>,
}

impl DepotPositionParams {
    // HBCI 2.2 VII.4.3.1/IV.6 and Messages 2022 C.4.3.1:
    // HIWPDS 5 uses the legacy parameter envelope, whose field 4
    // parameter DEG is segment element 3. HIWPDS 6 adds the generic
    // security-class field, shifting its field 5 parameter DEG to
    // element 4. Both operation-specific DEGs are three flat J/N
    // fields. Gate 4 consumes only the advertised segment version,
    // so meaning-neutral parameter values are read past.
    fn absorb(&mut self, header: &SegmentHeader<'_>) -> Result<(), Error> {
        self.advertised = true;
        if (5..=6).contains(&header.version) {
            if self.versions.contains(&header.version) {
                return Err(Error::InvalidResponse {
                    structure: "duplicate supported HIWPDS version",
                });
            }
            self.versions.push(header.version);
        }
        Ok(())
    }

    fn commit(mut self, state: &mut ReusableState) {
        state.depot_positions_advertised = self.advertised;
        self.versions.sort_unstable_by(|a, b| b.cmp(a));
        state.depot_position_versions = self.versions;
        state.depot_positions_requires_tan = self.requires_tan;
    }
}

#[derive(Default)]
struct SecuritiesTransactionParams {
    advertised: bool,
    seen: bool,
    supported: bool,
    storage_period_days: Option<u16>,
    requires_tan: Option<bool>,
}

impl SecuritiesTransactionParams {
    // Messages C.4.3.2: HIWDUS field 5 is the parameter DEG at
    // element 4; its field 1 is component 0.
    fn absorb(&mut self, segment: &Segment, header: &SegmentHeader<'_>) -> Result<(), Error> {
        self.advertised = true;
        if header.version == 5 {
            if self.seen {
                return Err(Error::InvalidResponse {
                    structure: "duplicate HIWDUS version 5",
                });
            }
            self.seen = true;
            if let Some(storage_period) = parse_storage_period(segment) {
                self.storage_period_days = Some(storage_period);
                self.supported = true;
            }
        }
        Ok(())
    }

    fn commit(self, state: &mut ReusableState) {
        state.securities_transactions_advertised = self.advertised;
        state.securities_transactions_supported = self.supported;
        state.securities_transactions_storage_period_days = self.storage_period_days;
        state.securities_transactions_requires_tan = self.requires_tan;
    }
}

#[derive(Default)]
struct CreditCardParams {
    transactions_advertised: bool,
    transactions_seen: bool,
    transactions: Option<CreditCardCapability>,
    transactions_storage_period_days: Option<u16>,
    transactions_requires_tan: Option<bool>,
    balance_advertised: bool,
    balance_account_required: Option<bool>,
    balance_requires_tan: Option<bool>,
}

impl CreditCardParams {
    // G112 C.12.1: HIKKUS field 5 is the parameter DEG at
    // element 4; all four parameter fields are scalar.
    fn absorb_transactions(
        &mut self,
        segment: &Segment,
        header: &SegmentHeader<'_>,
    ) -> Result<(), Error> {
        self.transactions_advertised = true;
        if header.version == 1 {
            if self.transactions_seen {
                return Err(Error::InvalidResponse {
                    structure: "duplicate HIKKUS version 1",
                });
            }
            self.transactions_seen = true;
            if let Some((capability, storage_period)) = parse_credit_card_parameters(segment) {
                self.transactions = Some(capability);
                self.transactions_storage_period_days = Some(storage_period);
            }
        }
        Ok(())
    }

    // G112 C.12.2: HIKKSS field 5 is the parameter DEG at
    // element 4 and contains one scalar component.
    fn absorb_balance(
        &mut self,
        segment: &Segment,
        header: &SegmentHeader<'_>,
    ) -> Result<(), Error> {
        self.balance_advertised = true;
        if header.version == 1 {
            if self.balance_account_required.is_some() {
                return Err(Error::InvalidResponse {
                    structure: "duplicate HIKKSS version 1",
                });
            }
            self.balance_account_required = Some(parse_credit_card_balance_parameters(segment)?);
        }
        Ok(())
    }

    fn commit(self, state: &mut ReusableState) {
        state.credit_card_transactions_advertised = self.transactions_advertised;
        state.credit_card_transactions = self.transactions;
        state.credit_card_transactions_storage_period_days = self.transactions_storage_period_days;
        state.credit_card_transactions_requires_tan = self.transactions_requires_tan;
        state.credit_card_balance_advertised = self.balance_advertised;
        state.credit_card_balance_account_required = self.balance_account_required;
        state.credit_card_balance_requires_tan = self.balance_requires_tan;
    }
}

#[derive(Default)]
struct UpdParams {
    accounts: Vec<Account>,
    version: Option<u16>,
    usage: Option<bool>,
}

impl UpdParams {
    // Formals E.2, HIUPA 4: segment fields remain separate
    // elements, so DD fields 3/4 are element indices 2/3.
    fn absorb_usage(&mut self, segment: &Segment, header: &SegmentHeader<'_>) -> Result<(), Error> {
        require_version(header.version, 4, "HIUPA")?;
        let version = parse_element_u16(segment, 2, "UPD version")?;
        if version > 999 {
            return Err(Error::InvalidValue {
                field: "UPD version",
            });
        }
        self.version = Some(version);
        self.usage = Some(match single_text(segment, 3, "UPD usage")?.as_str() {
            "0" => false,
            "1" => true,
            _ => {
                return Err(Error::InvalidValue { field: "UPD usage" });
            }
        });
        Ok(())
    }

    // Formals E.3, HIUPD 6: field 2 is one ktv element whose four
    // components do not shift later segment elements.
    fn absorb_account(
        &mut self,
        segment: &Segment,
        header: &SegmentHeader<'_>,
    ) -> Result<(), Error> {
        require_version(header.version, 6, "HIUPD")?;
        if let Some(account) = parse_account(segment)? {
            self.accounts.push(account);
        }
        Ok(())
    }

    fn commit(mut self, state: &mut ReusableState) -> Result<Option<Vec<Account>>, Error> {
        let mut transient_accounts = None;
        if let Some(version) = self.version {
            let unlisted_operations_unknown = self
                .usage
                .ok_or(Error::MissingValue { field: "UPD usage" })?;
            for account in &mut self.accounts {
                account.unlisted_operations_unknown = unlisted_operations_unknown;
            }
            state.upd_version = version;
            if version > 0 {
                state.accounts = self.accounts;
            } else {
                transient_accounts = Some(self.accounts);
            }
        }
        Ok(transient_accounts)
    }
}

fn is_parameter_segment(code: &[u8]) -> bool {
    code != b"HIRMS" && code.starts_with(b"HI") && code.ends_with(b"S")
}

pub(super) fn bpd_version(segments: &[Segment]) -> Result<Option<u16>, Error> {
    let mut received = None;
    for segment in segments {
        let Some(header) = segment.header() else {
            return Err(Error::InvalidResponse {
                structure: "parameters.BPD.segment_header",
            });
        };
        if header.code != b"HIBPA" {
            continue;
        }
        if received.is_some() {
            return Err(Error::InvalidResponse {
                structure: "duplicate HIBPA",
            });
        }
        require_version(header.version, 3, "HIBPA")?;
        // Formals D.2: HIBPA DD field 2 is segment element 1.
        let version = parse_element_u16(segment, 1, "BPD version")?;
        if version > 999 {
            return Err(Error::InvalidValue {
                field: "BPD version",
            });
        }
        received = Some(version);
    }
    Ok(received)
}

pub(super) fn bpd_institute(segments: &[Segment]) -> Result<Option<InstituteState>, Error> {
    let Some(segment) = segments.iter().find(|segment| {
        segment
            .header()
            .is_some_and(|header| header.code == b"HIBPA")
    }) else {
        return Ok(None);
    };
    require_version(
        segment.header().expect("header checked").version,
        3,
        "HIBPA",
    )?;
    // Formals D.2: HIBPA field 3 is one `kik` DEG at element 2.
    // `kik` has two flat components: country at 0 and institute code at 1.
    let components = segment
        .elements()
        .get(2)
        .ok_or(Error::MissingValue {
            field: "BPD institute identity",
        })?
        .components();
    Ok(Some(InstituteState {
        country_code: component(components, 0, "BPD country code")?,
        institute_code: component(components, 1, "BPD institute code")?,
    }))
}

pub(super) fn system_id(segments: &[Segment]) -> Result<Option<String>, Error> {
    let Some(segment) = segments.iter().find(|segment| {
        segment
            .header()
            .is_some_and(|header| header.code == b"HISYN")
    }) else {
        return Ok(None);
    };
    require_version(
        segment.header().expect("header checked").version,
        4,
        "HISYN",
    )?;
    Ok(Some(single_text(segment, 1, "assigned system ID")?))
}

pub(super) fn tan_media(
    segments: &[Segment],
    expected_version: u16,
    expected_reference: Option<u16>,
) -> Result<Option<Vec<TanMedium>>, Error> {
    let mut matching = segments.iter().filter(|segment| {
        segment
            .header()
            .is_some_and(|header| header.code == b"HITAB")
    });
    let Some(segment) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err(Error::InvalidResponse {
            structure: "tan_media.HITAB.duplicate",
        });
    }
    let header = segment.header().expect("header checked");
    if !matches!(header.version, 2..=5) {
        return Err(Error::UnsupportedSegment {
            code: "HITAB",
            version: header.version,
        });
    }
    if header.version != expected_version {
        return Err(Error::InvalidResponse {
            structure: "tan_media.HITAB.version",
        });
    }
    if expected_reference.is_some() && header.reference != expected_reference {
        return Err(Error::InvalidResponse {
            structure: "tan_media.HITAB.reference",
        });
    }
    let layout = match header.version {
        2 => TanMediumLayout::legacy(false),
        3 | 4 => TanMediumLayout::legacy(true),
        5 => TanMediumLayout::current(),
        _ => unreachable!("supported HITAB versions were matched"),
    };
    let mut media = Vec::new();
    for element in segment.elements().iter().skip(2) {
        let components = element.components();
        let Some(class) = optional_component(components, 0).and_then(|code| match code.as_str() {
            "A" => Some(TanMediumClass::All),
            "G" => Some(TanMediumClass::Generator),
            "L" => Some(TanMediumClass::List),
            "M" => Some(TanMediumClass::Mobile),
            "S" => Some(TanMediumClass::Secoder),
            "B" => Some(TanMediumClass::Bilateral),
            _ => None,
        }) else {
            continue;
        };
        let Some(status) = optional_component(components, 1).and_then(|code| match code.as_str() {
            "1" => Some(TanMediumStatus::Active),
            "2" => Some(TanMediumStatus::Available),
            "3" => Some(TanMediumStatus::FollowUpActive),
            "4" => Some(TanMediumStatus::FollowUpAvailable),
            _ => None,
        }) else {
            continue;
        };
        let name = optional_component(components, layout.name);
        if class == TanMediumClass::Mobile && name.is_none() {
            continue;
        }
        let security_function = layout
            .security_function
            .and_then(|index| optional_component(components, index));
        if class == TanMediumClass::Bilateral && security_function.is_none() {
            continue;
        }
        // Card number, card sequence, list number, and unmasked phone are
        // deliberately not parsed: no supported operation consumes or exposes
        // them, and one unusable medium entry must not discard its siblings.
        media.push(TanMedium {
            class,
            status,
            security_function,
            name,
            masked_phone: layout
                .masked_phone
                .and_then(|index| optional_component(components, index)),
            #[cfg(feature = "diagnostics")]
            development_card_number_present: optional_component(components, layout.card_number)
                .is_some(),
            #[cfg(feature = "diagnostics")]
            development_card_sequence_present: optional_component(components, layout.card_sequence)
                .is_some(),
        });
    }
    Ok(Some(media))
}

#[cfg(feature = "diagnostics")]
pub(super) fn development_tan_usage_option(
    segments: &[Segment],
    expected_version: u16,
    expected_reference: Option<u16>,
) -> Option<u8> {
    let segment = segments.iter().find(|segment| {
        segment.header().is_some_and(|header| {
            header.code == b"HITAB"
                && header.version == expected_version
                && expected_reference.is_none_or(|reference| header.reference == Some(reference))
        })
    })?;
    let value = optional_component(segment.elements().get(1)?.components(), 0)?;
    match value.as_str() {
        "0" => Some(0),
        "1" => Some(1),
        "2" => Some(2),
        _ => None,
    }
}

#[cfg(feature = "diagnostics")]
pub(super) fn development_tan_medium_shapes(
    segments: &[Segment],
    expected_version: u16,
    expected_reference: Option<u16>,
) -> Vec<crate::diagnostics::TanMediumElementShapeFact> {
    let Some(segment) = segments.iter().find(|segment| {
        segment.header().is_some_and(|header| {
            header.code == b"HITAB"
                && header.version == expected_version
                && expected_reference.is_none_or(|reference| header.reference == Some(reference))
        })
    }) else {
        return Vec::new();
    };
    segment
        .elements()
        .iter()
        .skip(2)
        .map(|element| {
            let components = element.components();
            let occupied = components
                .iter()
                .enumerate()
                .filter_map(|(index, value)| {
                    let occupied = value.as_text().is_some_and(|value| !value.is_empty())
                        || value.as_binary().is_some_and(|value| !value.is_empty());
                    occupied.then_some(index + 1)
                })
                .collect();
            crate::diagnostics::TanMediumElementShapeFact::new(components.len(), occupied)
        })
        .collect()
}

struct TanMediumLayout {
    name: usize,
    masked_phone: Option<usize>,
    security_function: Option<usize>,
    #[cfg(feature = "diagnostics")]
    card_number: usize,
    #[cfg(feature = "diagnostics")]
    card_sequence: usize,
}

impl TanMediumLayout {
    const KTV_COMPONENTS: usize = 4;

    fn legacy(has_masked_phone: bool) -> Self {
        // PIN/TAN 2020 DD, TAN-Medium-Liste 2/3/4: fields 1-5 occupy
        // components 1-5; nested field 6 `ktv` occupies 6-9; validity
        // fields 7/8 are 10/11, list number field 9 is 12, and designation
        // field 10 is 13. Version 2 then has nested field 11 `kti` at 14-19;
        // v3 has masked phone field 11 at 14 and `kti` field 12 at 15-20;
        // v4 adds plain phone field 12 at 15 and moves `kti` field 13 to
        // 16-21. Only designation and the v3/v4 masked phone are consumed.
        let name = 5 + Self::KTV_COMPONENTS + 3;
        Self {
            name,
            masked_phone: has_masked_phone.then_some(name + 1),
            security_function: None,
            #[cfg(feature = "diagnostics")]
            card_number: 2,
            #[cfg(feature = "diagnostics")]
            card_sequence: 3,
        }
    }

    fn current() -> Self {
        // TAN-Medium-Liste 5 inserts the security function before the card
        // group: fields 1-6 occupy components 1-6; field 7 is the four-slot
        // `ktv` at components 7-10; validity fields 8/9 are 11/12, list
        // number field 10 is 13, designation field 11 is 14, masked/plain
        // phone fields 12/13 are 15/16, and nested field 14 `kti` is 17-22.
        // Only security function, designation, and masked phone are consumed.
        let name = 6 + Self::KTV_COMPONENTS + 3;
        Self {
            name,
            masked_phone: Some(name + 1),
            security_function: Some(2),
            #[cfg(feature = "diagnostics")]
            card_number: 3,
            #[cfg(feature = "diagnostics")]
            card_sequence: 4,
        }
    }
}

fn parse_account(segment: &Segment) -> Result<Option<Account>, Error> {
    // Formals E.3, HIUPD 6: DD field 2 is the `ktv` element at index 1;
    // inside it, fields account/subaccount/country/institute map to flat
    // components 0..=3. DD fields 3..10 remain segment elements 2..=9,
    // and repeated field 11 begins at element 10.
    let elements = segment.elements();
    let national = elements
        .get(1)
        .ok_or(Error::MissingValue {
            field: "UPD account",
        })?
        .components();
    let account_number = optional_component(national, 0);
    let subaccount = optional_component(national, 1);
    let institute = match (
        optional_component(national, 2),
        optional_component(national, 3),
    ) {
        (Some(country_code), Some(institute_code)) => Some(InstituteState {
            country_code,
            institute_code,
        }),
        (None, None) => None,
        _ => {
            return Err(Error::InvalidResponse {
                structure: "UPD institute identity",
            });
        }
    };
    let mut iban = optional_single_text(elements, 2);
    if account_number.is_none() && iban.is_none() {
        // Formals E.3 permits HIUPD records for non-account-bound operations.
        return Ok(None);
    }
    if iban
        .as_ref()
        .is_some_and(|value| value.chars().count() == 35)
    {
        // Formals HIUPD 6 correction: tolerate the original erroneous 35-character
        // maximum by discarding exactly the final character.
        iban.as_mut().expect("IBAN checked").pop();
    }

    let mut allowed_operations = Vec::new();
    for element in elements.iter().skip(10) {
        let components = element.components();
        let Some(code) = optional_component(components, 0) else {
            continue;
        };
        if code.is_empty()
            || code.len() > 6
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            continue;
        }
        let Some(required_signatures) =
            optional_component(components, 1).and_then(|value| value.parse::<u8>().ok())
        else {
            continue;
        };
        allowed_operations.push(OperationPermission {
            code,
            required_signatures,
        });
    }

    Ok(Some(Account {
        iban,
        bic: None,
        account_number,
        subaccount,
        institute,
        currency: optional_single_text(elements, 5),
        account_type: optional_single_text(elements, 4)
            .map(|value| {
                value.parse().map_err(|_| Error::InvalidValue {
                    field: "UPD account type",
                })
            })
            .transpose()?,
        owner_name_1: optional_single_text(elements, 6),
        owner_name_2: optional_single_text(elements, 7),
        product_name: optional_single_text(elements, 8),
        allowed_operations,
        unlisted_operations_unknown: false,
    }))
}

fn parse_tan_requirement(segment: &Segment, operation: &str) -> Result<Option<bool>, Error> {
    // PIN/TAN B.8.1: HIPINS parameter fields 1..5 are flat components
    // 0..=4; repeated field 6 consists of two-component operation records.
    let Some(parameters) = segment.elements().get(4) else {
        return Ok(None);
    };
    let components = parameters.components();
    for pair in components.get(5..).unwrap_or_default().chunks_exact(2) {
        if optional_component(pair, 0).as_deref() == Some(operation) {
            return match optional_component(pair, 1).as_deref() {
                Some("J") => Ok(Some(true)),
                Some("N") => Ok(Some(false)),
                _ => Ok(None),
            };
        }
    }
    Ok(None)
}

fn require_legacy_balance_parameters(segment: &Segment) -> Result<(), Error> {
    // HBCI 2.2 VII.2.2: HISALS 5 is a "Geschäftsvorfall ohne
    // Parameter" containing only maximum orders and minimum signatures.
    let elements = segment.elements();
    if elements.len() < 3 {
        return Err(Error::InvalidResponse {
            structure: "HISALS 5 parameters",
        });
    }
    let maximum_orders = parse_element_u16(segment, 1, "HISALS maximum orders")?;
    if maximum_orders > 999 {
        return Err(Error::InvalidValue {
            field: "HISALS maximum orders",
        });
    }
    let signatures = parse_element_u16(segment, 2, "HISALS minimum signatures")?;
    if signatures > 3 {
        return Err(Error::InvalidValue {
            field: "HISALS minimum signatures",
        });
    }
    Ok(())
}

fn parse_credit_card_parameters(segment: &Segment) -> Option<(CreditCardCapability, u16)> {
    // G112 / CR 538 C.12.1 HIKKUS 1: storage period, maximum-entry input,
    // date-range input, and conditional account binding are scalar DD fields
    // 1..4, hence flat components 0..3 of segment element 4.
    let components = segment.elements().get(4)?.components();
    let storage_period = optional_component(components, 0)?.parse::<u16>().ok()?;
    let date_range_allowed = yn(components, 2, "HIKKUS date-range input").ok()?;
    let account_required = yn(components, 3, "HIKKUS account binding").ok()?;
    Some((
        CreditCardCapability {
            date_range_allowed,
            account_required,
        },
        storage_period,
    ))
}

fn parse_credit_card_balance_parameters(segment: &Segment) -> Result<bool, Error> {
    // G112 / CR 538 C.12.2 HIKKSS 1: its sole parameter DD field is flat
    // component 0 of segment element 4.
    let components = segment
        .elements()
        .get(4)
        .ok_or(Error::MissingValue {
            field: "HIKKSS parameters",
        })?
        .components();
    yn(components, 0, "HIKKSS account binding")
}

fn yn(components: &[Value], index: usize, field: &'static str) -> Result<bool, Error> {
    match component(components, index, field)?.as_str() {
        "J" => Ok(true),
        "N" => Ok(false),
        _ => Err(Error::InvalidValue { field }),
    }
}

fn parse_camt_parameters(segment: &Segment) -> (Option<u16>, Vec<String>) {
    // Messages DD "Parameter Kontoumsätze/Zeitraum camt" 1: scalar fields
    // 1..3 are components 0..2 and repeated descriptor field 4 starts at 3.
    let Some(components) = segment
        .elements()
        .get(4)
        .map(|element| element.components())
    else {
        return (None, Vec::new());
    };
    let storage_period =
        optional_component(components, 0).and_then(|value| value.parse::<u16>().ok());
    let descriptors = components
        .iter()
        .skip(3)
        .filter_map(|value| {
            value
                .as_text()
                .filter(|value| !value.is_empty() && value.chars().count() <= 256)
                .map(|value| value.into_owned())
        })
        .collect();
    (storage_period, descriptors)
}

fn parse_storage_period(segment: &Segment) -> Option<u16> {
    // Messages DD "Parameter Depotumsätze" 1: the only parameter field is
    // component 0 of the parameter DEG at segment element 4.
    segment
        .elements()
        .get(4)
        .and_then(|element| optional_component(element.components(), 0))
        .and_then(|value| value.parse::<u16>().ok())
}

fn parse_tan_methods(segment: &Segment, version: u16) -> Result<Vec<TanMethod>, Error> {
    // PIN/TAN DD "Parameter Zwei-Schritt-TAN-Einreichung" 6/7: scalar
    // fields 1..3 occupy components 0..2. Repeated method field 4 follows
    // at component 3; each method DEG is 21 components in v6 and 26 in v7.
    let Some(parameters) = segment
        .elements()
        .get(4)
        .map(|element| element.components())
    else {
        return Ok(Vec::new());
    };
    if parameters.len() < 3 {
        return Ok(Vec::new());
    }
    let width = if version == 6 { 21 } else { 26 };
    let method_values = &parameters[3..];
    if method_values.is_empty() {
        return Ok(Vec::new());
    }
    if method_values.len().div_ceil(width) > 98 {
        return Err(Error::InvalidResponse {
            structure: "too many HITANS methods",
        });
    }
    let mut blocks = method_values.chunks_exact(width);
    let mut methods = blocks
        .by_ref()
        .filter_map(|components| parse_tan_method(components, version).ok())
        .collect::<Vec<_>>();
    let trailing = blocks.remainder();
    if let Ok(method) = parse_tan_method(trailing, version) {
        methods.push(method);
    }
    Ok(methods)
}

fn parse_tan_method(components: &[Value], version: u16) -> Result<TanMethod, Error> {
    // PIN/TAN DD "Verfahrensparameter Zwei-Schritt-Verfahren" 6/7 contains
    // no nested DEG: DD fields 1..21/26 map directly to flat components
    // 0..20/25. Only trailing optional fields of the last repeated method
    // may be cut.
    if components.len() < 20 {
        return Err(Error::InvalidResponse {
            structure: "cut HITANS method parameters",
        });
    }
    let security_function = component(components, 0, "TAN security function")?;
    let numeric_function: u16 = security_function.parse().map_err(|_| Error::InvalidValue {
        field: "TAN security function",
    })?;
    if !(900..=997).contains(&numeric_function) {
        return Err(Error::InvalidValue {
            field: "TAN security function",
        });
    }
    let process_variant = component(components, 1, "TAN process variant")?;
    let dk_method = optional_component(components, 3);
    let process =
        if version == 7 && matches!(dk_method.as_deref(), Some("Decoupled" | "DecoupledPush")) {
            if process_variant != "2" {
                return Err(Error::InvalidValue {
                    field: "decoupled process variant",
                });
            }
            TanProcess::Decoupled
        } else {
            match process_variant.as_str() {
                "1" => TanProcess::ProcessVariantOne,
                "2" => TanProcess::ProcessVariantTwo,
                _ => {
                    return Err(Error::InvalidValue {
                        field: "TAN process variant",
                    });
                }
            }
        };
    if process == TanProcess::Decoupled && components.len() < 24 {
        return Err(Error::InvalidResponse {
            structure: "cut decoupled HITANS method parameters",
        });
    }
    // PIN/TAN 2020 DD, "Verfahrensparameter Zwei-Schritt-Verfahren" 6/7:
    // field 19 is the name-requirement code and optional field 21 is the
    // number of active media. HKTAN 6/7 DE 12 is mandatory only when the
    // former is 2 and the latter is greater than one.
    let medium_requirement_code =
        match component(components, 18, "TAN medium requirement")?.as_str() {
            "0" => 0,
            "1" => 1,
            "2" => 2,
            _ => {
                return Err(Error::InvalidValue {
                    field: "TAN medium requirement",
                });
            }
        };
    let active_media_count = optional_component(components, 20)
        .filter(|value| value.len() == 1 && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse::<u8>().ok());
    let medium_name_required = match medium_requirement_code {
        0 | 1 => false,
        2 => active_media_count.is_some_and(|count| count > 1),
        _ => {
            unreachable!("validated TAN medium requirement code")
        }
    };

    Ok(TanMethod {
        security_function,
        hktan_version: version,
        process,
        technical_id: component(components, 2, "TAN method technical ID")?,
        display_name: component(components, 5, "TAN method name")?,
        dk_method,
        max_tan_length: optional_component(components, 6)
            .map(|value| {
                value.parse().map_err(|_| Error::InvalidValue {
                    field: "maximum TAN length",
                })
            })
            .transpose()?,
        tan_format: optional_component(components, 7),
        medium_name_required,
        hhd_response_required: optional_yes(components, 19)?,
        max_decoupled_polls: (version == 7)
            .then(|| optional_u16(components, 21, "maximum decoupled polls"))
            .transpose()?
            .flatten()
            .and_then(|value| (value != 0).then_some(value)),
        first_poll_delay_seconds: (version == 7)
            .then(|| optional_u16(components, 22, "first decoupled poll delay"))
            .transpose()?
            .flatten(),
        next_poll_delay_seconds: (version == 7)
            .then(|| optional_u16(components, 23, "next decoupled poll delay"))
            .transpose()?
            .flatten(),
        manual_polling_allowed: version == 7 && optional_yes(components, 24)?,
        automatic_polling_allowed: version == 7 && optional_yes(components, 25)?,
        #[cfg(feature = "diagnostics")]
        development_medium_requirement: Some(crate::diagnostics::HitansMediumRequirementFact::new(
            version,
            medium_requirement_code,
            active_media_count,
            medium_name_required,
        )),
    })
}

fn optional_yes(components: &[Value], index: usize) -> Result<bool, Error> {
    match optional_component(components, index).as_deref() {
        None | Some("N") => Ok(false),
        Some("J") => Ok(true),
        _ => Err(Error::InvalidValue {
            field: "yes/no parameter",
        }),
    }
}

fn optional_u16(
    components: &[Value],
    index: usize,
    field: &'static str,
) -> Result<Option<u16>, Error> {
    optional_component(components, index)
        .map(|value| value.parse().map_err(|_| Error::InvalidValue { field }))
        .transpose()
}

fn parse_element_u16(segment: &Segment, index: usize, field: &'static str) -> Result<u16, Error> {
    single_text(segment, index, field)?
        .parse()
        .map_err(|_| Error::InvalidValue { field })
}

fn single_text(segment: &Segment, index: usize, field: &'static str) -> Result<String, Error> {
    segment
        .elements()
        .get(index)
        .map(|element| component(element.components(), 0, field))
        .transpose()?
        .ok_or(Error::MissingValue { field })
}

fn optional_single_text(elements: &[crate::wire::Element], index: usize) -> Option<String> {
    elements
        .get(index)
        .and_then(|element| optional_component(element.components(), 0))
}

fn require_version(actual: u16, expected: u16, code: &'static str) -> Result<(), Error> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::UnsupportedSegment {
            code,
            version: actual,
        })
    }
}
