use crate::model::ReusableState;

/// One safe BPD parameter-segment advertisement.
///
/// Segment codes and versions are generic protocol facts. This type never contains
/// parameter values, account data, or user data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSegmentAdvertisement {
    code: String,
    version: u16,
}

impl ParameterSegmentAdvertisement {
    pub(crate) fn new(code: String, version: u16) -> Self {
        Self { code, version }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn version(&self) -> u16 {
        self.version
    }
}

/// Advertised facts for one concrete read operation known by this crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationCapabilitySnapshot {
    advertised: bool,
    advertised_versions: Vec<u16>,
    supported_versions: Vec<u16>,
    tan_required: Option<bool>,
    descriptors: Vec<String>,
}

impl OperationCapabilitySnapshot {
    fn new(
        advertised: bool,
        advertised_versions: Vec<u16>,
        supported_versions: Vec<u16>,
        tan_required: Option<bool>,
        descriptors: Vec<String>,
    ) -> Self {
        Self {
            advertised,
            advertised_versions,
            supported_versions,
            tan_required,
            descriptors,
        }
    }

    pub fn advertised(&self) -> bool {
        self.advertised
    }

    pub fn advertised_versions(&self) -> &[u16] {
        &self.advertised_versions
    }

    /// Whether at least one advertised version is implemented by this crate.
    pub fn supported_by_crate(&self) -> bool {
        self.advertised_versions
            .iter()
            .any(|version| self.supported_versions.contains(version))
    }

    pub fn supports_version(&self, version: u16) -> bool {
        self.advertised_versions.contains(&version) && self.supported_versions.contains(&version)
    }

    /// Whether HIPINS explicitly marked this operation as TAN-required.
    ///
    /// `None` means HIPINS did not answer for this operation.
    pub fn tan_required(&self) -> Option<bool> {
        self.tan_required
    }

    /// Raw BPD descriptor strings, currently populated only for HKCAZ/camt.
    pub fn descriptors(&self) -> &[String] {
        &self.descriptors
    }
}

/// Redacted read model derived from the currently retained BPD capability facts.
///
/// The snapshot contains no account or personal data and is not serialized. The
/// generic parameter-segment list is complete for BPD acquired in this process;
/// older deserialized state can expose only the operation facts it already retained.
/// In particular, operation-specific transaction `advertised()` facts can under-report
/// after deserializing state created before capability snapshots, until BPD is refreshed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvertisedCapabilitySnapshot {
    balance: OperationCapabilitySnapshot,
    camt_cash_transactions: OperationCapabilitySnapshot,
    mt940_cash_transactions: OperationCapabilitySnapshot,
    depot_positions: OperationCapabilitySnapshot,
    depot_transactions: OperationCapabilitySnapshot,
    credit_card_transactions: OperationCapabilitySnapshot,
    credit_card_balance: OperationCapabilitySnapshot,
    parameter_segments: Vec<ParameterSegmentAdvertisement>,
}

impl AdvertisedCapabilitySnapshot {
    pub fn balance(&self) -> &OperationCapabilitySnapshot {
        &self.balance
    }

    pub fn camt_cash_transactions(&self) -> &OperationCapabilitySnapshot {
        &self.camt_cash_transactions
    }

    pub fn mt940_cash_transactions(&self) -> &OperationCapabilitySnapshot {
        &self.mt940_cash_transactions
    }

    pub fn depot_positions(&self) -> &OperationCapabilitySnapshot {
        &self.depot_positions
    }

    pub fn depot_transactions(&self) -> &OperationCapabilitySnapshot {
        &self.depot_transactions
    }

    pub fn credit_card_transactions(&self) -> &OperationCapabilitySnapshot {
        &self.credit_card_transactions
    }

    pub fn credit_card_balance(&self) -> &OperationCapabilitySnapshot {
        &self.credit_card_balance
    }

    pub fn parameter_segments(&self) -> &[ParameterSegmentAdvertisement] {
        &self.parameter_segments
    }
}

impl ReusableState {
    /// Derives a redacted advertised-capability snapshot from retained BPD facts.
    pub fn advertised_capabilities(&self) -> AdvertisedCapabilitySnapshot {
        let balance_versions = self.parameter_versions("HISALS", &self.advertised_balance_versions);
        let balance_supported_versions = balance_versions
            .iter()
            .copied()
            .filter(|version| (5..=8).contains(version))
            .collect();
        let camt_versions = self.parameter_versions(
            "HICAZS",
            if self.advertised_camt_descriptors.is_empty() {
                &[]
            } else {
                &[1]
            },
        );
        let mt940_versions = self.parameter_versions("HIKAZS", &self.legacy_transaction_versions);
        let depot_position_versions = self.parameter_versions(
            "HIWPDS",
            if self.depot_positions_supported {
                &[6]
            } else {
                &[]
            },
        );
        let depot_transaction_versions = self.parameter_versions(
            "HIWDUS",
            if self.securities_transactions_supported {
                &[5]
            } else {
                &[]
            },
        );
        let card_transaction_versions = self.parameter_versions(
            "HIKKUS",
            if self.credit_card_transactions.is_some() {
                &[1]
            } else {
                &[]
            },
        );
        let card_balance_versions = self.parameter_versions(
            "HIKKSS",
            if self.credit_card_balance_account_required.is_some() {
                &[1]
            } else {
                &[]
            },
        );

        AdvertisedCapabilitySnapshot {
            balance: OperationCapabilitySnapshot::new(
                self.balance_capability_advertised || !balance_versions.is_empty(),
                balance_versions,
                balance_supported_versions,
                self.balance_requires_tan,
                Vec::new(),
            ),
            camt_cash_transactions: OperationCapabilitySnapshot::new(
                !camt_versions.is_empty(),
                camt_versions,
                self.camt_capability
                    .as_ref()
                    .map(|_| vec![1])
                    .unwrap_or_default(),
                self.camt_requires_tan,
                self.advertised_camt_descriptors.clone(),
            ),
            mt940_cash_transactions: OperationCapabilitySnapshot::new(
                !mt940_versions.is_empty(),
                mt940_versions,
                self.legacy_transaction_versions.clone(),
                self.legacy_transactions_require_tan,
                Vec::new(),
            ),
            depot_positions: OperationCapabilitySnapshot::new(
                self.depot_positions_advertised || !depot_position_versions.is_empty(),
                depot_position_versions,
                self.depot_positions_supported
                    .then_some(6)
                    .into_iter()
                    .collect(),
                self.depot_positions_requires_tan,
                Vec::new(),
            ),
            depot_transactions: OperationCapabilitySnapshot::new(
                self.securities_transactions_advertised || !depot_transaction_versions.is_empty(),
                depot_transaction_versions,
                self.securities_transactions_supported
                    .then_some(5)
                    .into_iter()
                    .collect(),
                self.securities_transactions_requires_tan,
                Vec::new(),
            ),
            credit_card_transactions: OperationCapabilitySnapshot::new(
                self.credit_card_transactions_advertised || !card_transaction_versions.is_empty(),
                card_transaction_versions,
                self.credit_card_transactions
                    .as_ref()
                    .map(|_| vec![1])
                    .unwrap_or_default(),
                self.credit_card_transactions_requires_tan,
                Vec::new(),
            ),
            credit_card_balance: OperationCapabilitySnapshot::new(
                self.credit_card_balance_advertised || !card_balance_versions.is_empty(),
                card_balance_versions,
                self.credit_card_balance_account_required
                    .map(|_| vec![1])
                    .unwrap_or_default(),
                self.credit_card_balance_requires_tan,
                Vec::new(),
            ),
            parameter_segments: self.advertised_parameter_segments.clone(),
        }
    }

    fn parameter_versions(&self, code: &str, fallback: &[u16]) -> Vec<u16> {
        let mut versions = self
            .advertised_parameter_segments
            .iter()
            .filter(|segment| segment.code() == code)
            .map(ParameterSegmentAdvertisement::version)
            .collect::<Vec<_>>();
        if versions.is_empty() {
            versions.extend_from_slice(fallback);
        }
        versions.sort_unstable_by(|left, right| right.cmp(left));
        versions.dedup();
        versions
    }
}
