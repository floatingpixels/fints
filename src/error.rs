use thiserror::Error;

use crate::{transport::TransportError, wire::WireError};

/// A recovery decision derived without interpreting bank response free text.
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
    StrongAuthenticationRequired,
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
    #[error("the institution does not advertise a supported booked-transaction operation")]
    TransactionsNotAdvertised,
    #[error("the account is not authorized for booked-transaction retrieval")]
    TransactionsNotAuthorized,
    #[error("the institution requires an unsupported booked-transaction format or version")]
    TransactionsVersion,
    #[error("the institution delivered an unsupported camt namespace")]
    CamtNamespace,
    #[error("the institution does not advertise depot positions")]
    DepotPositionsNotAdvertised,
    #[error("the account is not authorized for depot positions")]
    DepotPositionsNotAuthorized,
    #[error("the institution requires an unsupported depot-position version")]
    DepotPositionsVersion,
    #[error("the institution does not advertise booked securities transactions")]
    SecuritiesTransactionsNotAdvertised,
    #[error("the account is not authorized for booked securities transactions")]
    SecuritiesTransactionsNotAuthorized,
    #[error("the institution requires an unsupported securities-transaction version")]
    SecuritiesTransactionsVersion,
    #[error("the institution does not advertise credit-card transactions")]
    CreditCardTransactionsNotAdvertised,
    #[error("the account is not authorized for credit-card transactions")]
    CreditCardTransactionsNotAuthorized,
    #[error("the institution requires an unsupported credit-card transaction version")]
    CreditCardTransactionsVersion,
    #[error("the institution does not advertise credit-card balances")]
    CreditCardBalanceNotAdvertised,
    #[error("the account is not authorized for credit-card balances")]
    CreditCardBalanceNotAuthorized,
    #[error("the institution requires an unsupported credit-card balance version")]
    CreditCardBalanceVersion,
    #[error("the institution requires an unsupported TAN method")]
    TanMethod,
    #[error("the institution did not provide TAN method parameters through anonymous BPD")]
    TanMethodParametersUnavailable,
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
    #[error("the endpoint returned parameters for a different institution")]
    InstituteMismatch,
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
    #[error("transaction date range must not end before it begins")]
    TransactionDateRange,
}

/// A typed bank response with explicitly accessible institution-authored diagnostics.
///
/// The caller owns display and logging policy for [`Self::text`],
/// [`Self::data_element_reference`], and [`Self::parameters`]. Those values are
/// deliberately omitted from this type's `Debug` output and crate-owned errors.
#[derive(Clone, PartialEq, Eq)]
pub struct BankResponse {
    code: u16,
    class: ResponseClass,
    segment_number: Option<u16>,
    recovery: Option<Recovery>,
    text: String,
    data_element_reference: Option<String>,
    parameters: Vec<String>,
}

impl BankResponse {
    pub(crate) fn new(
        code: u16,
        class: ResponseClass,
        segment_number: Option<u16>,
        recovery: Option<Recovery>,
        text: String,
        data_element_reference: Option<String>,
        parameters: Vec<String>,
    ) -> Self {
        Self {
            code,
            class,
            segment_number,
            recovery,
            text,
            data_element_reference,
            parameters,
        }
    }

    /// The four-digit FinTS response code.
    pub fn code(&self) -> u16 {
        self.code
    }

    /// The referenced request segment, when `HIRMS` supplied one.
    pub fn segment_number(&self) -> Option<u16> {
        self.segment_number
    }

    /// The bounded recovery category known for this response code.
    pub fn recovery(&self) -> Option<Recovery> {
        self.recovery
    }

    /// The response class derived from the first digit of the four-digit code.
    pub fn class(&self) -> ResponseClass {
        self.class
    }

    /// The institution-authored response text, retained verbatim from the wire.
    ///
    /// The caller owns display and logging policy for this diagnostic text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The offending data-element position supplied for a segment response.
    ///
    /// The caller owns display and logging policy for this diagnostic value.
    pub fn data_element_reference(&self) -> Option<&str> {
        self.data_element_reference.as_deref()
    }

    /// The bank-supplied parameters that further qualify this response.
    ///
    /// The caller owns display and logging policy for these diagnostic values.
    pub fn parameters(&self) -> &[String] {
        &self.parameters
    }
}

impl std::fmt::Debug for BankResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BankResponse")
            .field("code", &self.code)
            .field("class", &self.class)
            .field("segment_number", &self.segment_number)
            .field("recovery", &self.recovery)
            .finish_non_exhaustive()
    }
}

/// Failures exposed by the bounded supported-gate protocol engine.
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
    #[error("the FinTS transaction pagination limit has been reached")]
    PaginationLimitReached,
    #[error("the institution repeated a transaction continuation point")]
    RepeatedContinuationPoint,
    #[error("booked transaction data is malformed")]
    MalformedTransactionData,
    #[error("camt transaction data contains more than one report")]
    MultipleCamtReports,
    #[error("securities data is malformed")]
    MalformedSecuritiesData,
    #[error("the decoupled approval may not be polled again yet")]
    PollTooEarly,
    #[error("FinTS bank rejected or qualified the request with code {0:?}")]
    Bank(BankResponse),
    #[error(transparent)]
    Unsupported(#[from] Limitation),
}
