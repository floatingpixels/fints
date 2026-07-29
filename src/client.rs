use chrono::NaiveDateTime;

use crate::{
    engine::{
        BalanceResult, CreditCardBalanceResult, CreditCardTransactionsResult, DepotPositionsResult,
        Engine, InitializationResult, PendingChallenge, SecuritiesTransactionsResult,
        SynchronizationResult, TransactionsResult,
    },
    error::{BankResponse, Error},
    model::{
        Account, Balance, BookedTransactions, Challenge, Credentials, CreditCardBalance,
        CreditCardTransactions, DepotPositions, InstituteId, ProductIdentity, ReusableState,
        SecuritiesTransactions, Tan, TanMedium, TanMethod, TanProcess,
    },
    transport::Transport,
};

/// The user action accepted by a process-memory continuation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContinuationKind {
    Tan,
    DecoupledApproval,
}

/// How the caller is initiating a decoupled status poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollingMode {
    Manual,
    Automatic,
}

/// Result of a personalized dialog initialization.
pub enum Initialization {
    Connected,
    ChooseTanMethod,
    Challenge(Box<InitializationContinuation>),
}

/// Result of first-time system-ID synchronization.
pub enum Synchronization {
    Complete,
    Challenge(Box<SynchronizationContinuation>),
}

/// Result of a single advertised balance request.
pub enum BalanceRequest {
    Complete(Box<Balance>),
    Challenge(Box<BalanceContinuation>),
}

/// Result of an advertised booked-transaction request after pagination is exhausted.
pub enum BookedTransactionRequest {
    Complete(Box<BookedTransactions>),
    Challenge(Box<BookedTransactionContinuation>),
}

/// Result of an advertised depot-position request after pagination is exhausted.
pub enum DepotPositionRequest {
    Complete(Box<DepotPositions>),
    Challenge(Box<DepotPositionContinuation>),
}

/// Result of an advertised securities-transaction request after pagination is exhausted.
pub enum SecuritiesTransactionRequest {
    Complete(Box<SecuritiesTransactions>),
    Challenge(Box<SecuritiesTransactionContinuation>),
}

/// Result of an advertised credit-card transaction request after pagination is exhausted.
pub enum CreditCardTransactionRequest {
    Complete(Box<CreditCardTransactions>),
    Challenge(Box<CreditCardTransactionContinuation>),
}

/// Result of one advertised credit-card balance request.
pub enum CreditCardBalanceRequest {
    Complete(Box<CreditCardBalance>),
    Challenge(Box<CreditCardBalanceContinuation>),
}

/// Process-memory continuation for dialog initialization.
pub struct InitializationContinuation {
    pending: PendingChallenge,
}

/// Process-memory continuation for system-ID synchronization.
pub struct SynchronizationContinuation {
    pending: PendingChallenge,
}

/// Process-memory continuation for a balance request.
pub struct BalanceContinuation {
    pending: PendingChallenge,
}

/// Process-memory continuation for a booked-transaction request.
pub struct BookedTransactionContinuation {
    pending: PendingChallenge,
}

/// Process-memory continuation for depot positions.
pub struct DepotPositionContinuation {
    pending: PendingChallenge,
}

/// Process-memory continuation for booked securities transactions.
pub struct SecuritiesTransactionContinuation {
    pending: PendingChallenge,
}

/// Process-memory continuation for booked credit-card transactions.
pub struct CreditCardTransactionContinuation {
    pending: PendingChallenge,
}

/// Process-memory continuation for a credit-card balance.
pub struct CreditCardBalanceContinuation {
    pending: PendingChallenge,
}

/// Concrete synchronous FinTS client for the operations supported through Gate 4.
///
/// The client deliberately has no `Debug` implementation because it owns credentials,
/// dialog state, challenges, and authenticated protocol messages.
pub struct Client {
    engine: Engine,
    transport: Transport,
}

impl Client {
    pub fn new(
        endpoint: &str,
        institute: InstituteId,
        product: ProductIdentity,
        credentials: Credentials,
        state: ReusableState,
    ) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::new(institute, product, credentials, state)?,
            transport: Transport::new(endpoint)?,
        })
    }

    pub fn state(&self) -> &ReusableState {
        self.engine.state()
    }

    pub fn into_state(self) -> ReusableState {
        self.engine.into_state()
    }

    pub fn accounts(&self) -> &[Account] {
        self.engine.accounts()
    }

    pub fn tan_methods(&self) -> &[TanMethod] {
        self.engine.state().tan_methods()
    }

    pub fn allowed_tan_methods(&self) -> &[String] {
        self.engine.allowed_tan_methods()
    }

    pub fn tan_media(&self) -> &[TanMedium] {
        self.engine.tan_media()
    }

    /// Redacted response codes from the most recently parsed bank message.
    ///
    /// Free response text and raw authenticated messages are never retained.
    pub fn last_responses(&self) -> &[BankResponse] {
        self.engine.last_responses()
    }

    pub fn select_tan_method(&mut self, security_function: &str) -> Result<(), Error> {
        self.engine.choose_tan_method(security_function)
    }

    pub fn select_tan_medium(&mut self, name: &str) -> Result<(), Error> {
        self.engine.choose_tan_medium(name)
    }

    /// Refreshes anonymous BPD and always closes the anonymous dialog.
    pub fn refresh_parameters(&mut self, now: NaiveDateTime) -> Result<(), Error> {
        let request = self.engine.anonymous_initialization_request()?;
        let response = self.send(&request)?;
        self.engine.accept_anonymous_initialization(&response)?;
        self.terminate(now)
    }

    /// Obtains a new assigned system ID and closes the synchronization dialog.
    pub fn synchronize(&mut self, now: NaiveDateTime) -> Result<Synchronization, Error> {
        let request = self
            .engine
            .synchronization_request(now.date(), now.time())?;
        let response = self.send(&request)?;
        match self.engine.accept_synchronization(&response, now)? {
            SynchronizationResult::Complete => {
                self.terminate(now)?;
                Ok(Synchronization::Complete)
            }
            SynchronizationResult::Challenge(pending) => Ok(Synchronization::Challenge(Box::new(
                SynchronizationContinuation { pending: *pending },
            ))),
        }
    }

    /// Discovers TAN media using the dedicated HKTAB initialization flow, then closes it.
    pub fn discover_tan_media(&mut self, now: NaiveDateTime) -> Result<&[TanMedium], Error> {
        let request = self
            .engine
            .tan_media_initialization_request(now.date(), now.time())?;
        let response = self.send(&request)?;
        self.engine.accept_tan_media_initialization(&response)?;
        self.terminate(now)?;
        Ok(self.engine.tan_media())
    }

    /// Opens the personalized dialog used for subsequent supported operations.
    ///
    /// If no selected method is usable, the method-discovery dialog is closed before
    /// `ChooseTanMethod` is returned.
    pub fn initialize(&mut self, now: NaiveDateTime) -> Result<Initialization, Error> {
        let request = self.engine.initialization_request(now.date(), now.time())?;
        let response = self.send(&request)?;
        match self.engine.accept_initialization(&response, now)? {
            InitializationResult::Connected => Ok(Initialization::Connected),
            InitializationResult::ChooseTanMethod => {
                if self.engine.has_active_dialog() {
                    self.terminate(now)?;
                }
                Ok(Initialization::ChooseTanMethod)
            }
            InitializationResult::Challenge(pending) => Ok(Initialization::Challenge(Box::new(
                InitializationContinuation { pending: *pending },
            ))),
        }
    }

    pub fn balance(
        &mut self,
        account_index: usize,
        now: NaiveDateTime,
    ) -> Result<BalanceRequest, Error> {
        let request = self
            .engine
            .balance_request(account_index, now.date(), now.time())?;
        let response = self.send(&request)?;
        map_balance(self.engine.accept_balance(&response, now)?)
    }

    /// Retrieves booked entries for one UPD account and exhausts same-dialog pagination.
    ///
    /// `from` and `to` are optional inclusive protocol dates. Returned data remains
    /// grouped with the requested account and must never be logged.
    pub fn booked_transactions(
        &mut self,
        account_index: usize,
        from: Option<chrono::NaiveDate>,
        to: Option<chrono::NaiveDate>,
        now: NaiveDateTime,
    ) -> Result<BookedTransactionRequest, Error> {
        let request =
            self.engine
                .transaction_request(account_index, from, to, now.date(), now.time())?;
        let response = self.send(&request)?;
        let result = self.engine.accept_transactions(&response, now)?;
        self.finish_transactions(result, now)
    }

    /// Retrieves advertised depot positions and exhausts same-dialog pagination.
    ///
    /// Positions and all account-bound values are private and must never be logged.
    pub fn depot_positions(
        &mut self,
        account_index: usize,
        now: NaiveDateTime,
    ) -> Result<DepotPositionRequest, Error> {
        let request = self
            .engine
            .depot_positions_request(account_index, now.date(), now.time())?;
        let response = self.send(&request)?;
        let result = self.engine.accept_depot_positions(&response, now)?;
        self.finish_depot_positions(result, now)
    }

    /// Retrieves advertised booked securities transactions with optional inclusive dates.
    pub fn securities_transactions(
        &mut self,
        account_index: usize,
        from: Option<chrono::NaiveDate>,
        to: Option<chrono::NaiveDate>,
        now: NaiveDateTime,
    ) -> Result<SecuritiesTransactionRequest, Error> {
        let request = self.engine.securities_transactions_request(
            account_index,
            from,
            to,
            now.date(),
            now.time(),
        )?;
        let response = self.send(&request)?;
        let result = self.engine.accept_securities_transactions(&response, now)?;
        self.finish_securities_transactions(result, now)
    }

    /// Retrieves advertised booked credit-card transactions with optional inclusive dates.
    pub fn credit_card_transactions(
        &mut self,
        account_index: usize,
        from: Option<chrono::NaiveDate>,
        to: Option<chrono::NaiveDate>,
        now: NaiveDateTime,
    ) -> Result<CreditCardTransactionRequest, Error> {
        let request = self.engine.credit_card_transactions_request(
            account_index,
            from,
            to,
            now.date(),
            now.time(),
        )?;
        let response = self.send(&request)?;
        let result = self
            .engine
            .accept_credit_card_transactions(&response, now)?;
        self.finish_credit_card_transactions(result, now)
    }

    /// Retrieves the independently reported G112 credit-card balance components.
    pub fn credit_card_balance(
        &mut self,
        account_index: usize,
        now: NaiveDateTime,
    ) -> Result<CreditCardBalanceRequest, Error> {
        let request =
            self.engine
                .credit_card_balance_request(account_index, now.date(), now.time())?;
        let response = self.send(&request)?;
        map_credit_card_balance(self.engine.accept_credit_card_balance(&response, now)?)
    }

    pub fn submit_initialization_tan(
        &mut self,
        continuation: InitializationContinuation,
        tan: &Tan,
        now: NaiveDateTime,
    ) -> Result<Initialization, Error> {
        let request = self
            .engine
            .tan_submission_request(&continuation.pending, tan, now)?;
        let response = self.send(&request)?;
        map_initialization(
            self.engine
                .accept_initialization_continuation(&response, continuation.pending)?,
        )
    }

    pub fn poll_initialization(
        &mut self,
        mut continuation: InitializationContinuation,
        mode: PollingMode,
        now: NaiveDateTime,
    ) -> Result<Initialization, Error> {
        let request = self
            .engine
            .decoupled_poll_request(&mut continuation.pending, mode, now)?;
        let response = self.send(&request)?;
        map_initialization(
            self.engine
                .accept_initialization_continuation(&response, continuation.pending)?,
        )
    }

    pub fn submit_synchronization_tan(
        &mut self,
        continuation: SynchronizationContinuation,
        tan: &Tan,
        now: NaiveDateTime,
    ) -> Result<Synchronization, Error> {
        let request = self
            .engine
            .tan_submission_request(&continuation.pending, tan, now)?;
        let response = self.send(&request)?;
        self.finish_synchronization(response, continuation.pending, now)
    }

    pub fn poll_synchronization(
        &mut self,
        mut continuation: SynchronizationContinuation,
        mode: PollingMode,
        now: NaiveDateTime,
    ) -> Result<Synchronization, Error> {
        let request = self
            .engine
            .decoupled_poll_request(&mut continuation.pending, mode, now)?;
        let response = self.send(&request)?;
        self.finish_synchronization(response, continuation.pending, now)
    }

    pub fn submit_balance_tan(
        &mut self,
        continuation: BalanceContinuation,
        tan: &Tan,
        now: NaiveDateTime,
    ) -> Result<BalanceRequest, Error> {
        let request = self
            .engine
            .tan_submission_request(&continuation.pending, tan, now)?;
        let response = self.send(&request)?;
        map_balance(
            self.engine
                .accept_balance_continuation(&response, continuation.pending)?,
        )
    }

    pub fn poll_balance(
        &mut self,
        mut continuation: BalanceContinuation,
        mode: PollingMode,
        now: NaiveDateTime,
    ) -> Result<BalanceRequest, Error> {
        let request = self
            .engine
            .decoupled_poll_request(&mut continuation.pending, mode, now)?;
        let response = self.send(&request)?;
        map_balance(
            self.engine
                .accept_balance_continuation(&response, continuation.pending)?,
        )
    }

    pub fn submit_booked_transaction_tan(
        &mut self,
        continuation: BookedTransactionContinuation,
        tan: &Tan,
        now: NaiveDateTime,
    ) -> Result<BookedTransactionRequest, Error> {
        let request = self
            .engine
            .tan_submission_request(&continuation.pending, tan, now)?;
        let response = self.send(&request)?;
        let result =
            self.engine
                .accept_transactions_continuation(&response, continuation.pending, now)?;
        self.finish_transactions(result, now)
    }

    pub fn poll_booked_transactions(
        &mut self,
        mut continuation: BookedTransactionContinuation,
        mode: PollingMode,
        now: NaiveDateTime,
    ) -> Result<BookedTransactionRequest, Error> {
        let request = self
            .engine
            .decoupled_poll_request(&mut continuation.pending, mode, now)?;
        let response = self.send(&request)?;
        let result =
            self.engine
                .accept_transactions_continuation(&response, continuation.pending, now)?;
        self.finish_transactions(result, now)
    }

    pub fn submit_depot_position_tan(
        &mut self,
        continuation: DepotPositionContinuation,
        tan: &Tan,
        now: NaiveDateTime,
    ) -> Result<DepotPositionRequest, Error> {
        let request = self
            .engine
            .tan_submission_request(&continuation.pending, tan, now)?;
        let response = self.send(&request)?;
        let result = self.engine.accept_depot_positions_continuation(
            &response,
            continuation.pending,
            now,
        )?;
        self.finish_depot_positions(result, now)
    }

    pub fn poll_depot_positions(
        &mut self,
        mut continuation: DepotPositionContinuation,
        mode: PollingMode,
        now: NaiveDateTime,
    ) -> Result<DepotPositionRequest, Error> {
        let request = self
            .engine
            .decoupled_poll_request(&mut continuation.pending, mode, now)?;
        let response = self.send(&request)?;
        let result = self.engine.accept_depot_positions_continuation(
            &response,
            continuation.pending,
            now,
        )?;
        self.finish_depot_positions(result, now)
    }

    pub fn submit_securities_transaction_tan(
        &mut self,
        continuation: SecuritiesTransactionContinuation,
        tan: &Tan,
        now: NaiveDateTime,
    ) -> Result<SecuritiesTransactionRequest, Error> {
        let request = self
            .engine
            .tan_submission_request(&continuation.pending, tan, now)?;
        let response = self.send(&request)?;
        let result = self.engine.accept_securities_transactions_continuation(
            &response,
            continuation.pending,
            now,
        )?;
        self.finish_securities_transactions(result, now)
    }

    pub fn poll_securities_transactions(
        &mut self,
        mut continuation: SecuritiesTransactionContinuation,
        mode: PollingMode,
        now: NaiveDateTime,
    ) -> Result<SecuritiesTransactionRequest, Error> {
        let request = self
            .engine
            .decoupled_poll_request(&mut continuation.pending, mode, now)?;
        let response = self.send(&request)?;
        let result = self.engine.accept_securities_transactions_continuation(
            &response,
            continuation.pending,
            now,
        )?;
        self.finish_securities_transactions(result, now)
    }

    pub fn submit_credit_card_transaction_tan(
        &mut self,
        continuation: CreditCardTransactionContinuation,
        tan: &Tan,
        now: NaiveDateTime,
    ) -> Result<CreditCardTransactionRequest, Error> {
        let request = self
            .engine
            .tan_submission_request(&continuation.pending, tan, now)?;
        let response = self.send(&request)?;
        let result = self.engine.accept_credit_card_transactions_continuation(
            &response,
            continuation.pending,
            now,
        )?;
        self.finish_credit_card_transactions(result, now)
    }

    pub fn poll_credit_card_transactions(
        &mut self,
        mut continuation: CreditCardTransactionContinuation,
        mode: PollingMode,
        now: NaiveDateTime,
    ) -> Result<CreditCardTransactionRequest, Error> {
        let request = self
            .engine
            .decoupled_poll_request(&mut continuation.pending, mode, now)?;
        let response = self.send(&request)?;
        let result = self.engine.accept_credit_card_transactions_continuation(
            &response,
            continuation.pending,
            now,
        )?;
        self.finish_credit_card_transactions(result, now)
    }

    pub fn submit_credit_card_balance_tan(
        &mut self,
        continuation: CreditCardBalanceContinuation,
        tan: &Tan,
        now: NaiveDateTime,
    ) -> Result<CreditCardBalanceRequest, Error> {
        let request = self
            .engine
            .tan_submission_request(&continuation.pending, tan, now)?;
        let response = self.send(&request)?;
        map_credit_card_balance(self.engine.accept_credit_card_balance_continuation(
            &response,
            continuation.pending,
            now,
        )?)
    }

    pub fn poll_credit_card_balance(
        &mut self,
        mut continuation: CreditCardBalanceContinuation,
        mode: PollingMode,
        now: NaiveDateTime,
    ) -> Result<CreditCardBalanceRequest, Error> {
        let request = self
            .engine
            .decoupled_poll_request(&mut continuation.pending, mode, now)?;
        let response = self.send(&request)?;
        map_credit_card_balance(self.engine.accept_credit_card_balance_continuation(
            &response,
            continuation.pending,
            now,
        )?)
    }

    /// Explicitly closes the active dialog. This also cancels a dropped continuation.
    pub fn terminate(&mut self, now: NaiveDateTime) -> Result<(), Error> {
        let request = self.engine.termination_request(now.date(), now.time())?;
        let response = self.send(&request)?;
        self.engine.accept_termination(&response)
    }

    fn finish_synchronization(
        &mut self,
        response: Vec<u8>,
        pending: PendingChallenge,
        now: NaiveDateTime,
    ) -> Result<Synchronization, Error> {
        match self
            .engine
            .accept_synchronization_continuation(&response, pending)?
        {
            SynchronizationResult::Complete => {
                self.terminate(now)?;
                Ok(Synchronization::Complete)
            }
            SynchronizationResult::Challenge(pending) => Ok(Synchronization::Challenge(Box::new(
                SynchronizationContinuation { pending: *pending },
            ))),
        }
    }

    fn finish_transactions(
        &mut self,
        mut result: TransactionsResult,
        now: NaiveDateTime,
    ) -> Result<BookedTransactionRequest, Error> {
        loop {
            match result {
                TransactionsResult::Complete(transactions) => {
                    return Ok(BookedTransactionRequest::Complete(transactions));
                }
                TransactionsResult::Challenge(pending) => {
                    return Ok(BookedTransactionRequest::Challenge(Box::new(
                        BookedTransactionContinuation { pending: *pending },
                    )));
                }
                TransactionsResult::Continue => {
                    let request = self
                        .engine
                        .next_transaction_page_request(now.date(), now.time())?;
                    let response = self.send(&request)?;
                    result = self.engine.accept_transactions(&response, now)?;
                }
            }
        }
    }

    fn finish_depot_positions(
        &mut self,
        mut result: DepotPositionsResult,
        now: NaiveDateTime,
    ) -> Result<DepotPositionRequest, Error> {
        loop {
            match result {
                DepotPositionsResult::Complete(value) => {
                    return Ok(DepotPositionRequest::Complete(value));
                }
                DepotPositionsResult::Challenge(pending) => {
                    return Ok(DepotPositionRequest::Challenge(Box::new(
                        DepotPositionContinuation { pending: *pending },
                    )));
                }
                DepotPositionsResult::Continue => {
                    let request = self
                        .engine
                        .next_depot_positions_page_request(now.date(), now.time())?;
                    let response = self.send(&request)?;
                    result = self.engine.accept_depot_positions(&response, now)?;
                }
            }
        }
    }

    fn finish_securities_transactions(
        &mut self,
        mut result: SecuritiesTransactionsResult,
        now: NaiveDateTime,
    ) -> Result<SecuritiesTransactionRequest, Error> {
        loop {
            match result {
                SecuritiesTransactionsResult::Complete(value) => {
                    return Ok(SecuritiesTransactionRequest::Complete(value));
                }
                SecuritiesTransactionsResult::Challenge(pending) => {
                    return Ok(SecuritiesTransactionRequest::Challenge(Box::new(
                        SecuritiesTransactionContinuation { pending: *pending },
                    )));
                }
                SecuritiesTransactionsResult::Continue => {
                    let request = self
                        .engine
                        .next_securities_transactions_page_request(now.date(), now.time())?;
                    let response = self.send(&request)?;
                    result = self.engine.accept_securities_transactions(&response, now)?;
                }
            }
        }
    }

    fn finish_credit_card_transactions(
        &mut self,
        mut result: CreditCardTransactionsResult,
        now: NaiveDateTime,
    ) -> Result<CreditCardTransactionRequest, Error> {
        loop {
            match result {
                CreditCardTransactionsResult::Complete(value) => {
                    return Ok(CreditCardTransactionRequest::Complete(value));
                }
                CreditCardTransactionsResult::Challenge(pending) => {
                    return Ok(CreditCardTransactionRequest::Challenge(Box::new(
                        CreditCardTransactionContinuation { pending: *pending },
                    )));
                }
                CreditCardTransactionsResult::Continue => {
                    let request = self
                        .engine
                        .next_credit_card_transactions_page_request(now.date(), now.time())?;
                    let response = self.send(&request)?;
                    result = self
                        .engine
                        .accept_credit_card_transactions(&response, now)?;
                }
            }
        }
    }

    fn send(&mut self, request: &[u8]) -> Result<Vec<u8>, Error> {
        self.transport.send(request).map_err(|error| {
            self.engine.abort_dialog();
            Error::Transport(error)
        })
    }
}

impl InitializationContinuation {
    pub fn challenge(&self) -> &Challenge {
        &self.pending.challenge
    }

    pub fn kind(&self) -> ContinuationKind {
        continuation_kind(&self.pending)
    }

    pub fn earliest_poll_at(&self) -> Option<NaiveDateTime> {
        self.pending.next_poll_at
    }
}

impl SynchronizationContinuation {
    pub fn challenge(&self) -> &Challenge {
        &self.pending.challenge
    }

    pub fn kind(&self) -> ContinuationKind {
        continuation_kind(&self.pending)
    }

    pub fn earliest_poll_at(&self) -> Option<NaiveDateTime> {
        self.pending.next_poll_at
    }
}

impl BalanceContinuation {
    pub fn challenge(&self) -> &Challenge {
        &self.pending.challenge
    }

    pub fn kind(&self) -> ContinuationKind {
        continuation_kind(&self.pending)
    }

    pub fn earliest_poll_at(&self) -> Option<NaiveDateTime> {
        self.pending.next_poll_at
    }
}

impl BookedTransactionContinuation {
    pub fn challenge(&self) -> &Challenge {
        &self.pending.challenge
    }

    pub fn kind(&self) -> ContinuationKind {
        continuation_kind(&self.pending)
    }

    pub fn earliest_poll_at(&self) -> Option<NaiveDateTime> {
        self.pending.next_poll_at
    }
}

macro_rules! continuation_accessors {
    ($type:ty) => {
        impl $type {
            pub fn challenge(&self) -> &Challenge {
                &self.pending.challenge
            }

            pub fn kind(&self) -> ContinuationKind {
                continuation_kind(&self.pending)
            }

            pub fn earliest_poll_at(&self) -> Option<NaiveDateTime> {
                self.pending.next_poll_at
            }
        }
    };
}

continuation_accessors!(DepotPositionContinuation);
continuation_accessors!(SecuritiesTransactionContinuation);
continuation_accessors!(CreditCardTransactionContinuation);
continuation_accessors!(CreditCardBalanceContinuation);

fn continuation_kind(pending: &PendingChallenge) -> ContinuationKind {
    match pending.method.process {
        TanProcess::Decoupled => ContinuationKind::DecoupledApproval,
        TanProcess::ProcessVariantOne | TanProcess::ProcessVariantTwo => ContinuationKind::Tan,
    }
}

fn map_initialization(result: InitializationResult) -> Result<Initialization, Error> {
    match result {
        InitializationResult::Connected => Ok(Initialization::Connected),
        InitializationResult::ChooseTanMethod => Err(Error::InconsistentState),
        InitializationResult::Challenge(pending) => Ok(Initialization::Challenge(Box::new(
            InitializationContinuation { pending: *pending },
        ))),
    }
}

fn map_balance(result: BalanceResult) -> Result<BalanceRequest, Error> {
    Ok(match result {
        BalanceResult::Complete(balance) => BalanceRequest::Complete(balance),
        BalanceResult::Challenge(pending) => {
            BalanceRequest::Challenge(Box::new(BalanceContinuation { pending: *pending }))
        }
    })
}

fn map_credit_card_balance(
    result: CreditCardBalanceResult,
) -> Result<CreditCardBalanceRequest, Error> {
    Ok(match result {
        CreditCardBalanceResult::Complete(balance) => CreditCardBalanceRequest::Complete(balance),
        CreditCardBalanceResult::Challenge(pending) => {
            CreditCardBalanceRequest::Challenge(Box::new(CreditCardBalanceContinuation {
                pending: *pending,
            }))
        }
    })
}
