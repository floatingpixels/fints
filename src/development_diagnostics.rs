//! Temporary, opt-in diagnostics for owner-attended live interoperability work.
//!
//! This module is compiled only with the `development-diagnostics` feature. It exposes
//! decision booleans only: no wire values, response text, credentials, identifiers, or
//! financial data. Normal consumers and production builds must leave the feature off.

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
