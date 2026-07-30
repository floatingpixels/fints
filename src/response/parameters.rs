use crate::{
    capabilities::ParameterSegmentAdvertisement,
    error::Error,
    model::{
        Account, CamtCapability, CreditCardCapability, InstituteState, OperationPermission,
        ReusableState, TanMedium, TanMediumClass, TanMediumStatus, TanMethod, TanProcess,
    },
    wire::{Segment, Value},
};

use super::camt_descriptor_matches;
use super::{component, optional_component};

const SUPPORTED_CAMT_DESCRIPTOR: &str = "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08";

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
    let mut received_accounts = Vec::new();
    let mut received_methods = Vec::new();
    let mut received_balance_versions = Vec::new();
    let mut advertised_balance_versions = Vec::new();
    let mut advertised_tan_media_versions = Vec::new();
    let mut balance_capability_advertised = false;
    let mut received_camt_descriptors = Vec::new();
    let mut camt_storage_period_days = None;
    let mut received_legacy_transaction_versions = Vec::new();
    let mut transaction_capability_advertised = false;
    let mut received_upd_version = None;
    let mut received_upd_usage = None;
    let mut balance_requires_tan = None;
    let mut camt_requires_tan = None;
    let mut legacy_transactions_require_tan = None;
    let mut depot_positions_advertised = false;
    let mut depot_positions_supported = false;
    let mut depot_positions_requires_tan = None;
    let mut securities_transactions_advertised = false;
    let mut securities_transactions_seen = false;
    let mut securities_transactions_supported = false;
    let mut securities_transactions_storage_period_days = None;
    let mut securities_transactions_requires_tan = None;
    let mut credit_card_transactions_advertised = false;
    let mut credit_card_transactions_seen = false;
    let mut credit_card_transactions = None;
    let mut credit_card_transactions_storage_period_days = None;
    let mut credit_card_transactions_requires_tan = None;
    let mut credit_card_balance_advertised = false;
    let mut credit_card_balance_account_required = None;
    let mut credit_card_balance_requires_tan = None;
    let mut advertised_parameter_segments = Vec::new();

    for segment in segments {
        let header = segment.header().ok_or(Error::InvalidResponse {
            structure: "parameters.business.segment_header",
        })?;
        if replace_bpd && is_parameter_segment(header.code) {
            advertised_parameter_segments.push(ParameterSegmentAdvertisement::new(
                header.code.iter().copied().map(char::from).collect(),
                header.version,
            ));
            if header.code == b"HITABS" {
                advertised_tan_media_versions.push(header.version);
            }
        }
        match header.code {
            b"HIBPA" => {}
            b"HIUPA" => {
                require_version(header.version, 4, "HIUPA")?;
                let version = parse_element_u16(segment, 2, "UPD version")?;
                if version > 999 {
                    return Err(Error::InvalidValue {
                        field: "UPD version",
                    });
                }
                received_upd_version = Some(version);
                received_upd_usage = Some(match single_text(segment, 3, "UPD usage")?.as_str() {
                    "0" => false,
                    "1" => true,
                    _ => {
                        return Err(Error::InvalidValue { field: "UPD usage" });
                    }
                });
            }
            b"HIUPD" => {
                require_version(header.version, 6, "HIUPD")?;
                if let Some(account) = parse_account(segment)? {
                    received_accounts.push(account);
                }
            }
            b"HISALS" if replace_bpd => {
                balance_capability_advertised = true;
                advertised_balance_versions.push(header.version);
                if header.version == 5 {
                    // HBCI 2.2 VII.2.2 contains only meaning-neutral order and
                    // signature limits. A malformed legacy parameter shape is
                    // retained as advertised-but-unsupported, not fatal to BPD.
                    if require_legacy_balance_parameters(segment).is_ok() {
                        received_balance_versions.push(header.version);
                    }
                } else if (6..=8).contains(&header.version) {
                    received_balance_versions.push(header.version);
                }
            }
            b"HICAZS" if replace_bpd => {
                transaction_capability_advertised = true;
                if header.version == 1 {
                    let (storage_period, descriptors) = parse_camt_parameters(segment);
                    camt_storage_period_days = camt_storage_period_days.or(storage_period);
                    received_camt_descriptors.extend(descriptors);
                }
            }
            b"HIKAZS" if replace_bpd => {
                transaction_capability_advertised = true;
                if (6..=7).contains(&header.version) {
                    received_legacy_transaction_versions.push(header.version);
                }
            }
            b"HIWPDS" if replace_bpd => {
                depot_positions_advertised = true;
                if header.version == 6 {
                    if depot_positions_supported {
                        return Err(Error::InvalidResponse {
                            structure: "duplicate HIWPDS version 6",
                        });
                    }
                    depot_positions_supported = true;
                }
            }
            b"HIWDUS" if replace_bpd => {
                securities_transactions_advertised = true;
                if header.version == 5 {
                    if securities_transactions_seen {
                        return Err(Error::InvalidResponse {
                            structure: "duplicate HIWDUS version 5",
                        });
                    }
                    securities_transactions_seen = true;
                    if let Some(storage_period) = parse_storage_period(segment) {
                        securities_transactions_storage_period_days = Some(storage_period);
                        securities_transactions_supported = true;
                    }
                }
            }
            b"HIKKUS" if replace_bpd => {
                credit_card_transactions_advertised = true;
                if header.version == 1 {
                    if credit_card_transactions_seen {
                        return Err(Error::InvalidResponse {
                            structure: "duplicate HIKKUS version 1",
                        });
                    }
                    credit_card_transactions_seen = true;
                    if let Some((capability, storage_period)) =
                        parse_credit_card_parameters(segment)
                    {
                        credit_card_transactions = Some(capability);
                        credit_card_transactions_storage_period_days = Some(storage_period);
                    }
                }
            }
            b"HIKKSS" if replace_bpd => {
                credit_card_balance_advertised = true;
                if header.version == 1 {
                    if credit_card_balance_account_required.is_some() {
                        return Err(Error::InvalidResponse {
                            structure: "duplicate HIKKSS version 1",
                        });
                    }
                    credit_card_balance_account_required =
                        Some(parse_credit_card_balance_parameters(segment)?);
                }
            }
            b"HIPINS" if replace_bpd => {
                require_version(header.version, 1, "HIPINS")?;
                balance_requires_tan = parse_tan_requirement(segment, "HKSAL")?;
                camt_requires_tan = parse_tan_requirement(segment, "HKCAZ")?;
                legacy_transactions_require_tan = parse_tan_requirement(segment, "HKKAZ")?;
                depot_positions_requires_tan = parse_tan_requirement(segment, "HKWPD")?;
                securities_transactions_requires_tan = parse_tan_requirement(segment, "HKWDU")?;
                credit_card_transactions_requires_tan = parse_tan_requirement(segment, "HKKKU")?;
                credit_card_balance_requires_tan = parse_tan_requirement(segment, "HKKKS")?;
            }
            b"HITANS" if replace_bpd && (6..=7).contains(&header.version) => {
                received_methods.extend(parse_tan_methods(segment, header.version)?);
            }
            _ => {}
        }
    }

    if let Some(version) = received_bpd_version.filter(|_| replace_bpd) {
        state.bpd_version = version;
        received_balance_versions.sort_unstable_by(|left, right| right.cmp(left));
        received_balance_versions.dedup();
        advertised_balance_versions.sort_unstable_by(|left, right| right.cmp(left));
        advertised_balance_versions.dedup();
        advertised_tan_media_versions.sort_unstable_by(|left, right| right.cmp(left));
        advertised_tan_media_versions.dedup();
        received_legacy_transaction_versions.sort_unstable_by(|left, right| right.cmp(left));
        received_legacy_transaction_versions.dedup();
        received_camt_descriptors.sort();
        received_camt_descriptors.dedup();
        advertised_parameter_segments.sort_by(|left, right| {
            left.code()
                .cmp(right.code())
                .then_with(|| right.version().cmp(&left.version()))
        });
        advertised_parameter_segments.dedup();
        received_methods.sort_by(|left, right| {
            left.security_function
                .cmp(&right.security_function)
                .then_with(|| right.hktan_version.cmp(&left.hktan_version))
        });
        received_methods.dedup_by(|left, right| left.security_function == right.security_function);
        state.balance_versions = received_balance_versions;
        state.advertised_balance_versions = advertised_balance_versions;
        state.advertised_tan_media_versions = advertised_tan_media_versions;
        state.balance_capability_advertised = balance_capability_advertised;
        state.balance_requires_tan = balance_requires_tan;
        state.transaction_capability_advertised = transaction_capability_advertised;
        state.advertised_camt_descriptors = received_camt_descriptors.clone();
        state.camt_storage_period_days = camt_storage_period_days;
        state.camt_capability = received_camt_descriptors
            .into_iter()
            .find(|descriptor| camt_descriptor_matches(descriptor, SUPPORTED_CAMT_DESCRIPTOR))
            .map(|descriptor| CamtCapability { descriptor });
        state.legacy_transaction_versions = received_legacy_transaction_versions;
        state.camt_requires_tan = camt_requires_tan;
        state.legacy_transactions_require_tan = legacy_transactions_require_tan;
        state.depot_positions_advertised = depot_positions_advertised;
        state.depot_positions_supported = depot_positions_supported;
        state.depot_positions_requires_tan = depot_positions_requires_tan;
        state.securities_transactions_advertised = securities_transactions_advertised;
        state.securities_transactions_supported = securities_transactions_supported;
        state.securities_transactions_storage_period_days =
            securities_transactions_storage_period_days;
        state.securities_transactions_requires_tan = securities_transactions_requires_tan;
        state.credit_card_transactions_advertised = credit_card_transactions_advertised;
        state.credit_card_transactions = credit_card_transactions;
        state.credit_card_transactions_storage_period_days =
            credit_card_transactions_storage_period_days;
        state.credit_card_transactions_requires_tan = credit_card_transactions_requires_tan;
        state.credit_card_balance_advertised = credit_card_balance_advertised;
        state.credit_card_balance_account_required = credit_card_balance_account_required;
        state.credit_card_balance_requires_tan = credit_card_balance_requires_tan;
        state.advertised_parameter_segments = advertised_parameter_segments;
        state.tan_methods = received_methods;
    }

    let mut transient_accounts = None;
    if let Some(version) = received_upd_version {
        let unlisted_operations_unknown =
            received_upd_usage.ok_or(Error::MissingValue { field: "UPD usage" })?;
        for account in &mut received_accounts {
            account.unlisted_operations_unknown = unlisted_operations_unknown;
        }
        state.upd_version = version;
        if version > 0 {
            state.accounts = received_accounts;
        } else {
            transient_accounts = Some(received_accounts);
        }
    }
    Ok(transient_accounts)
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
    if !matches!(header.version, 2 | 4 | 5) {
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
        2 => TanMediumLayout {
            name: 9,
            masked_phone: None,
            security_function: None,
            #[cfg(feature = "development-diagnostics")]
            card_number: 2,
            #[cfg(feature = "development-diagnostics")]
            card_sequence: 3,
        },
        4 => TanMediumLayout {
            name: 9,
            masked_phone: Some(10),
            security_function: None,
            #[cfg(feature = "development-diagnostics")]
            card_number: 2,
            #[cfg(feature = "development-diagnostics")]
            card_sequence: 3,
        },
        5 => TanMediumLayout {
            name: 10,
            masked_phone: Some(11),
            security_function: Some(2),
            #[cfg(feature = "development-diagnostics")]
            card_number: 3,
            #[cfg(feature = "development-diagnostics")]
            card_sequence: 4,
        },
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
            #[cfg(feature = "development-diagnostics")]
            development_card_number_present: optional_component(components, layout.card_number)
                .is_some(),
            #[cfg(feature = "development-diagnostics")]
            development_card_sequence_present: optional_component(components, layout.card_sequence)
                .is_some(),
        });
    }
    Ok(Some(media))
}

#[cfg(feature = "development-diagnostics")]
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

struct TanMediumLayout {
    name: usize,
    masked_phone: Option<usize>,
    security_function: Option<usize>,
    #[cfg(feature = "development-diagnostics")]
    card_number: usize,
    #[cfg(feature = "development-diagnostics")]
    card_sequence: usize,
}

fn parse_account(segment: &Segment) -> Result<Option<Account>, Error> {
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
    // date-range input, and conditional account binding.
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
    // G112 / CR 538 C.12.2 HIKKSS 1.
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
    segment
        .elements()
        .get(4)
        .and_then(|element| optional_component(element.components(), 0))
        .and_then(|value| value.parse::<u16>().ok())
}

fn parse_tan_methods(segment: &Segment, version: u16) -> Result<Vec<TanMethod>, Error> {
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
    // Fields through "Antwort HHD_UC erforderlich" are positional and mandatory.
    // Only trailing optional fields of the last repeated method may be cut.
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
        #[cfg(feature = "development-diagnostics")]
        development_medium_requirement: Some(
            crate::development_diagnostics::HitansMediumRequirementFact::new(
                version,
                medium_requirement_code,
                active_media_count,
                medium_name_required,
            ),
        ),
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
