//! Temporary, opt-in diagnostics for owner-attended live interoperability work.
//!
//! This module is compiled only with the `development-diagnostics` feature. It exposes
//! decision booleans and bounded segment code/version facts only: no raw wire values,
//! response text, credentials, identifiers, medium names, or financial data. Normal
//! consumers and production builds must leave the feature off.

/// One validated response/business segment observed during an owner-attended diagnostic.
///
/// Segment codes and versions are generic protocol facts. This type contains no segment
/// contents and deliberately does not implement serialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceivedSegmentFact {
    code: String,
    version: u16,
}

impl ReceivedSegmentFact {
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

/// Redacted structure of the most recent TAN-medium discovery response.
///
/// The ordered segment facts exclude security controls and all segment contents.
/// `discovered_medium_count` is `None` only when medium parsing failed before a
/// complete list could be established.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TanMediaDiscoveryFacts {
    received_segments: Vec<ReceivedSegmentFact>,
    discovered_medium_count: Option<usize>,
}

impl TanMediaDiscoveryFacts {
    pub(crate) fn new(received_segments: Vec<ReceivedSegmentFact>) -> Self {
        Self {
            received_segments,
            discovered_medium_count: None,
        }
    }

    pub(crate) fn set_discovered_medium_count(&mut self, count: usize) {
        self.discovered_medium_count = Some(count);
    }

    pub fn received_segments(&self) -> &[ReceivedSegmentFact] {
        &self.received_segments
    }

    pub fn discovered_medium_count(&self) -> Option<usize> {
        self.discovered_medium_count
    }
}

/// Redacted facts used to decide whether initialization may perform the one bounded
/// anonymous-BPD repair.
///
/// The value is process-memory only and deliberately does not implement serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitializationRecoveryFacts {
    usable_selected_method_present: bool,
    bpd_zero: bool,
    upd_zero: bool,
    tan_parameters_empty: bool,
    has_tan_method_response: bool,
    global_abort_shape: bool,
    eligible: bool,
}

impl InitializationRecoveryFacts {
    pub(crate) fn new(
        usable_selected_method_present: bool,
        bpd_zero: bool,
        upd_zero: bool,
        tan_parameters_empty: bool,
        has_tan_method_response: bool,
        global_abort_shape: bool,
        eligible: bool,
    ) -> Self {
        Self {
            usable_selected_method_present,
            bpd_zero,
            upd_zero,
            tan_parameters_empty,
            has_tan_method_response,
            global_abort_shape,
            eligible,
        }
    }

    pub fn usable_selected_method_present(self) -> bool {
        self.usable_selected_method_present
    }

    pub fn bpd_zero(self) -> bool {
        self.bpd_zero
    }

    pub fn upd_zero(self) -> bool {
        self.upd_zero
    }

    pub fn tan_parameters_empty(self) -> bool {
        self.tan_parameters_empty
    }

    pub fn has_tan_method_response(self) -> bool {
        self.has_tan_method_response
    }

    pub fn global_abort_shape(self) -> bool {
        self.global_abort_shape
    }

    pub fn eligible(self) -> bool {
        self.eligible
    }
}
