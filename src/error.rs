use thiserror::Error;

use crate::{transport::TransportError, wire::WireError};

/// A recovery decision the caller can present without exposing bank response text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    RetryLater,
    Resynchronize,
    ChooseTanMethod,
    CorrectCredentials,
    CorrectEndpoint,
    CorrectProductIdentity,
    RefreshParameters,
    RestartDialog,
    UnsupportedCapability,
    WaitForApproval,
    ContactInstitute,
}

/// The protocol-defined class of a FinTS response code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseClass {
    /// Class 0: the referenced request was accepted.
    Success,
    /// Class 3: the request was accepted with a warning.
    Warning,
    /// Class 9: the referenced request was rejected.
    Error,
}

/// A protocol capability that the current gate deliberately cannot use.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum Limitation {
    #[error("the institution does not advertise a supported balance operation")]
    BalanceNotAdvertised,
    #[error("the account is not authorized for balance retrieval")]
    BalanceNotAuthorized,
    #[error("the institution requires an unsupported HKSAL version")]
    BalanceVersion,
    #[error("the institution requires an unsupported TAN method")]
    TanMethod,
    #[error("the institution requires a different TAN medium")]
    TanMedium,
    #[error("the operation requires multiple signers")]
    MultipleSigners,
    #[error("the institution requires an unsupported PIN/TAN parameter combination")]
    PinTanParameters,
    #[error("the institution requires an unsupported TAN process variant")]
    TanProcessVariant,
    #[error("the selected TAN method does not advertise this decoupled polling mode")]
    DecoupledPolling,
}

/// Errors from input validation. Values are deliberately omitted from every variant.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum InputError {
    #[error("product registration ID must contain 1 to 25 Latin-1 bytes")]
    ProductRegistration,
    #[error("product version must contain 1 to 5 Latin-1 bytes")]
    ProductVersion,
    #[error("country code must contain exactly three digits")]
    CountryCode,
    #[error("institute code must contain 1 to 30 Latin-1 bytes")]
    InstituteCode,
    #[error("user ID must contain 1 to 30 Latin-1 bytes")]
    UserId,
    #[error("customer ID must contain 1 to 30 Latin-1 bytes")]
    CustomerId,
    #[error("PIN must contain 1 to 99 Latin-1 bytes")]
    Pin,
    #[error("TAN must contain 1 to 99 Latin-1 bytes")]
    Tan,
    #[error("TAN method identifier must contain exactly three digits")]
    TanMethod,
    #[error("TAN medium name must contain 1 to 32 Latin-1 bytes")]
    TanMedium,
    #[error("reusable FinTS state is malformed")]
    ReusableState,
}

/// A bank response code stripped of its potentially private free text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BankResponse {
    code: u16,
    class: ResponseClass,
    segment_number: Option<u16>,
    recovery: Option<Recovery>,
}

impl BankResponse {
    pub(crate) fn new(
        code: u16,
        class: ResponseClass,
        segment_number: Option<u16>,
        recovery: Option<Recovery>,
    ) -> Self {
        Self {
            code,
            class,
            segment_number,
            recovery,
        }
    }

    /// The four-digit FinTS response code.
    pub fn code(self) -> u16 {
        self.code
    }

    /// The referenced request segment, when `HIRMS` supplied one.
    pub fn segment_number(self) -> Option<u16> {
        self.segment_number
    }

    /// The bounded recovery category known for this response code.
    pub fn recovery(self) -> Option<Recovery> {
        self.recovery
    }

    /// The response class derived from the first digit of the four-digit code.
    pub fn class(self) -> ResponseClass {
        self.class
    }
}

/// Failures exposed by the bounded Gate 1 protocol engine.
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("FinTS response has an invalid {structure} structure")]
    InvalidResponse { structure: &'static str },
    #[error("FinTS response uses unsupported segment {code} version {version}")]
    UnsupportedSegment { code: &'static str, version: u16 },
    #[error("FinTS response has an invalid value in {field}")]
    InvalidValue { field: &'static str },
    #[error("FinTS response is missing {field}")]
    MissingValue { field: &'static str },
    #[error("FinTS response message or dialog state is inconsistent")]
    InconsistentState,
    #[error("the FinTS continuation belongs to a different dialog")]
    StaleContinuation,
    #[error("the TAN or approval challenge has expired")]
    ChallengeExpired,
    #[error("the decoupled approval polling limit has been reached")]
    PollLimitReached,
    #[error("the FinTS continuation limit has been reached")]
    ContinuationLimitReached,
    #[error("the decoupled approval may not be polled again yet")]
    PollTooEarly,
    #[error("FinTS bank rejected or qualified the request with code {0:?}")]
    Bank(BankResponse),
    #[error(transparent)]
    Unsupported(#[from] Limitation),
}
