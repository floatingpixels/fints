//! Product-neutral FinTS 3.0 PIN/TAN client.
//!
//! The supported protocol surface is deliberately bounded by `SCOPE.md`.

#![forbid(unsafe_code)]

mod client;
mod engine;
mod error;
mod model;
mod response;
mod segments;
mod transport;
mod wire;

pub use client::{
    BalanceContinuation, BalanceRequest, BookedTransactionContinuation, BookedTransactionRequest,
    Client, ContinuationKind, CreditCardBalanceContinuation, CreditCardBalanceRequest,
    CreditCardTransactionContinuation, CreditCardTransactionRequest, DepotPositionContinuation,
    DepotPositionRequest, Initialization, InitializationContinuation, InitializationStage,
    PollingMode, SecuritiesTransactionContinuation, SecuritiesTransactionRequest, Synchronization,
    SynchronizationContinuation,
};
pub use error::{BankResponse, Error, InputError, Limitation, Recovery, ResponseClass};
pub use model::{
    Account, Amount, Balance, BookedEntry, BookedTransactionDetail, BookedTransactions, Challenge,
    Credentials, CreditCardAmount, CreditCardBalance, CreditCardCurrentBalance, CreditCardEntry,
    CreditCardTransactions, CreditDebit, DepotPosition, DepotPositions, InstituteId, PriceQuality,
    ProductIdentity, QuantityUnit, ReusableState, SecuritiesAmount, SecuritiesMovement,
    SecuritiesQuantity, SecuritiesTransaction, SecuritiesTransactions, SecurityInstrument,
    SecurityPrice, SignedAmount, StatementPosition, Tan, TanMedium, TanMediumClass,
    TanMediumStatus, TanMethod, TanProcess, Timestamp,
};
pub use transport::TransportError;
pub use wire::WireError;
