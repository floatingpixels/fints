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
    /// Anonymous BPD must be refreshed before the returned 3920 identifiers
    /// can be matched to described TAN methods. Call
    /// [`Client::refresh_parameters`], then offer only methods present in both
    /// [`Client::allowed_tan_methods`] and [`Client::tan_methods`].
    RefreshParameters,
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
        self.engine.tan_methods()
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
        let Self { engine, transport } = self;
        synchronize_with_send(engine, now, |request| {
            transport.send(request).map_err(Error::from)
        })
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
        let result = self.engine.accept_initialization(&response, now)?;
        let Self { engine, transport } = self;
        finish_initialization_with_send(engine, result, now, |request| {
            transport.send(request).map_err(Error::from)
        })
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
        InitializationResult::ChooseTanMethod | InitializationResult::RefreshParameters => {
            Err(Error::InconsistentState)
        }
        InitializationResult::Challenge(pending) => Ok(Initialization::Challenge(Box::new(
            InitializationContinuation { pending: *pending },
        ))),
    }
}

fn finish_initialization_with_send(
    engine: &mut Engine,
    result: InitializationResult,
    now: NaiveDateTime,
    mut send: impl FnMut(&[u8]) -> Result<Vec<u8>, Error>,
) -> Result<Initialization, Error> {
    let outcome = match result {
        InitializationResult::Connected => return Ok(Initialization::Connected),
        InitializationResult::ChooseTanMethod => Initialization::ChooseTanMethod,
        InitializationResult::RefreshParameters => Initialization::RefreshParameters,
        InitializationResult::Challenge(pending) => {
            return Ok(Initialization::Challenge(Box::new(
                InitializationContinuation { pending: *pending },
            )));
        }
    };
    if engine.has_active_dialog() {
        let request = engine.termination_request(now.date(), now.time())?;
        let response = send(&request).inspect_err(|_error| {
            engine.abort_dialog();
        })?;
        engine.accept_termination(&response)?;
    }
    Ok(outcome)
}

fn synchronize_with_send(
    engine: &mut Engine,
    now: NaiveDateTime,
    mut send: impl FnMut(&[u8]) -> Result<Vec<u8>, Error>,
) -> Result<Synchronization, Error> {
    let request = engine.synchronization_request(now.date(), now.time())?;
    let response = send(&request)?;
    match engine.accept_synchronization(&response, now)? {
        SynchronizationResult::Complete => {
            let request = engine.termination_request(now.date(), now.time())?;
            let response = send(&request)?;
            engine.accept_termination(&response)?;
            Ok(Synchronization::Complete)
        }
        SynchronizationResult::Challenge(pending) => Ok(Synchronization::Challenge(Box::new(
            SynchronizationContinuation { pending: *pending },
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

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use chrono::{NaiveDate, NaiveTime};

    use super::*;
    use crate::{model::TanProcess, wire::Message};

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 7, 29)
            .unwrap()
            .and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap())
    }

    fn synchronization_engine() -> Engine {
        let mut state = ReusableState::new();
        state.tan_methods.push(TanMethod {
            security_function: "942".to_owned(),
            hktan_version: 6,
            process: TanProcess::ProcessVariantTwo,
            technical_id: "fictional-method".to_owned(),
            display_name: "Fictional approval".to_owned(),
            dk_method: None,
            max_tan_length: Some(6),
            tan_format: Some("1".to_owned()),
            medium_name_required: false,
            hhd_response_required: false,
            max_decoupled_polls: None,
            first_poll_delay_seconds: None,
            next_poll_delay_seconds: None,
            manual_polling_allowed: false,
            automatic_polling_allowed: false,
        });
        state.selected_tan_method = Some("942".to_owned());
        Engine::new(
            InstituteId::new("280", "12345678").unwrap(),
            ProductIdentity::new("PROD123", "1.0").unwrap(),
            Credentials::new("fictional-user", None, "private-pin").unwrap(),
            state,
        )
        .unwrap()
    }

    fn secured_response(inner: &[u8], message_number: u16, trailer_number: u16) -> Vec<u8> {
        let mut wire = format!(
            "HNHBK:1:3+000000000000+300+dialog1+{message_number}+dialog1:1'\
             HNVSK:998:3+PIN:2+998+1+1::fictional-system+1+2:2:13:@8@"
        )
        .into_bytes();
        wire.extend_from_slice(&[0; 8]);
        wire.extend_from_slice(b":5:1+280:12345678:fictional-bank:V:0:0+0'HNVSD:999:1+@");
        wire.extend_from_slice(inner.len().to_string().as_bytes());
        wire.push(b'@');
        wire.extend_from_slice(inner);
        wire.extend_from_slice(format!("'HNHBS:{trailer_number}:1+{message_number}'").as_bytes());
        let length = format!("{:012}", wire.len());
        wire[10..22].copy_from_slice(length.as_bytes());
        wire
    }

    fn plain_response(segments: &[&str], dialog_id: &str, message_number: u16) -> Vec<u8> {
        let trailer = segments.len() + 2;
        let mut wire =
            format!("HNHBK:1:3+000000000000+300+{dialog_id}+{message_number}+{dialog_id}:1'");
        for segment in segments {
            wire.push_str(segment);
            wire.push('\'');
        }
        wire.push_str(&format!("HNHBS:{trailer}:1+{message_number}'"));
        let length = format!("{:012}", wire.len());
        wire.replace_range(10..22, &length);
        wire.into_bytes()
    }

    // PIN/TAN B.6.1 and correction T8: a bank-terminated function-999
    // refresh outcome receives no HKEND, while an open successful discovery
    // with the same missing method descriptions is closed exactly once.
    #[test]
    fn refresh_outcome_closes_only_an_open_discovery_dialog() {
        let new_engine = || {
            Engine::new(
                InstituteId::new("280", "12345678").unwrap(),
                ProductIdentity::new("PROD123", "1.0").unwrap(),
                Credentials::new("fictional-user", None, "private-pin").unwrap(),
                ReusableState::new(),
            )
            .unwrap()
        };
        let mut terminated_engine = new_engine();
        let terminated = plain_response(
            &[
                "HIRMG:2:2+9050::summary+9800::termination",
                "HIRMS:3:2:4+9952::unpublished companion+3920::methods:942",
            ],
            "terminated-discovery",
            1,
        );
        let result = terminated_engine
            .accept_initialization(&terminated, now())
            .unwrap();
        let mut sends = 0;
        assert!(matches!(
            finish_initialization_with_send(&mut terminated_engine, result, now(), |_request| {
                sends += 1;
                Err(Error::InconsistentState)
            })
            .unwrap(),
            Initialization::RefreshParameters
        ));
        assert_eq!(sends, 0);

        let mut open_engine = new_engine();
        let open = plain_response(
            &["HIRMG:2:2+0010::accepted", "HIRMS:3:2:4+3920::methods:942"],
            "open-discovery",
            1,
        );
        let result = open_engine.accept_initialization(&open, now()).unwrap();
        assert!(matches!(
            finish_initialization_with_send(&mut open_engine, result, now(), |request| {
                let payload = Message::parse(request)?.payload_segments()?;
                assert_eq!(
                    payload
                        .iter()
                        .filter(|segment| segment.header().unwrap().code == b"HKEND")
                        .count(),
                    1
                );
                sends += 1;
                Ok(plain_response(
                    &["HIRMG:2:2+0100::terminated"],
                    "open-discovery",
                    2,
                ))
            })
            .unwrap(),
            Initialization::RefreshParameters
        ));
        assert_eq!(sends, 1);
        assert!(!open_engine.has_active_dialog());
    }

    // HBCI Security 2024 B.5.1/DD permits HNSHK timestamp type without
    // date/time; PIN/TAN F.2 permits the optional response-side control pair.
    // This exercises the production Client::synchronize orchestration seam.
    #[test]
    fn synchronization_sends_one_hkend_and_preserves_assigned_system_id() {
        let synchronization = concat!(
            "HNSHK:2:4+PIN:2+942+fiction-ref+1+1",
            "+1::fictional-system+1+1+1:999:1+6:10:16",
            "+280:12345678:fictional-bank:S:0:0'",
            "HIRMG:3:2+3060::fictional warning'",
            "HIRMS:4:2:6+0020::fictional synchronization accepted'",
            "HIRMS:5:2:4+0020::fictional identity accepted",
            "+3920::fictional methods:942'",
            "HIRMS:6:2:5+3076::fictional SCA exemption'",
            "HISYN:7:4:2+fictional-assigned-system'",
            "HNSHA:8:2+fiction-ref'"
        );
        let termination = concat!(
            "HNSHK:2:4+PIN:2+942+fiction-ref+1+1",
            "+1::fictional-assigned-system+2+1+1:999:1+6:10:16",
            "+280:12345678:fictional-bank:S:0:0'",
            "HIRMG:3:2+0100::fictional termination'",
            "HNSHA:4:2+fiction-ref'"
        );
        let mut responses = VecDeque::from([
            secured_response(synchronization.as_bytes(), 1, 9),
            secured_response(termination.as_bytes(), 2, 5),
        ]);
        let mut operations = Vec::new();
        let mut engine = synchronization_engine();

        let result = synchronize_with_send(&mut engine, now(), |request| {
            let payload = Message::parse(request)?.payload_segments()?;
            operations.push(
                if payload
                    .iter()
                    .any(|segment| segment.header().unwrap().code == b"HKSYN")
                {
                    "HKSYN"
                } else if payload
                    .iter()
                    .any(|segment| segment.header().unwrap().code == b"HKEND")
                {
                    "HKEND"
                } else {
                    "unexpected"
                },
            );
            responses.pop_front().ok_or(Error::InconsistentState)
        })
        .unwrap();

        assert!(matches!(result, Synchronization::Complete));
        assert_eq!(operations, ["HKSYN", "HKEND"]);
        assert_eq!(
            engine.state().system_id(),
            Some("fictional-assigned-system")
        );
        assert!(responses.is_empty());
    }
}
