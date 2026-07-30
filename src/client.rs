use chrono::NaiveDateTime;

use crate::{
    engine::{
        BalanceResult, CreditCardBalanceResult, CreditCardTransactionsResult, DepotPositionsResult,
        Engine, InitializationResult, PendingChallenge, SecuritiesTransactionsResult,
        SynchronizationResult, TanMediaInitializationResult, TransactionsResult,
    },
    error::{BankResponse, Error},
    model::{
        Account, Balance, BookedTransactions, Challenge, Credentials, CreditCardBalance,
        CreditCardTransactions, DepotPositions, InstituteId, ProductIdentity, ReusableState,
        SecuritiesTransactions, Tan, TanMedium, TanMethod, TanProcess,
    },
    transport::{TraceSink, Transport},
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

/// Last request stage reached by [`Client::initialize`] or
/// [`Client::refresh_parameters`].
///
/// This process-memory diagnostic contains no wire data, credentials, identifiers, or
/// response values and is therefore safe for callers to include in redacted logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitializationStage {
    InitialDiscovery,
    AnonymousBpdRefresh,
    AnonymousTermination,
    RepeatedDiscovery,
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
    last_initialization_stage: Option<InitializationStage>,
}

impl Client {
    pub fn new(
        endpoint: &str,
        institute: InstituteId,
        product: ProductIdentity,
        credentials: Credentials,
        state: ReusableState,
    ) -> Result<Self, Error> {
        Self::new_with_trace(endpoint, institute, product, credentials, state, None)
    }

    /// Constructs a client with an optional raw transport trace sink.
    ///
    /// Trace events carry credential-bearing outgoing and incoming FinTS payloads.
    /// Installing a sink is an explicit per-client decision; the crate never stores,
    /// logs, formats, or otherwise retains those payloads.
    pub fn new_with_trace(
        endpoint: &str,
        institute: InstituteId,
        product: ProductIdentity,
        credentials: Credentials,
        state: ReusableState,
        trace_sink: Option<TraceSink>,
    ) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::new(institute, product, credentials, state)?,
            transport: Transport::new_with_trace(endpoint, trace_sink)?,
            last_initialization_stage: None,
        })
    }

    pub fn state(&self) -> &ReusableState {
        self.engine.state()
    }

    /// Derives a redacted snapshot of the currently retained BPD advertisements.
    pub fn advertised_capabilities(&self) -> crate::AdvertisedCapabilitySnapshot {
        self.state().advertised_capabilities()
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

    /// HKTAB/HITAB versions advertised in the current BPD, highest first.
    ///
    /// Segment versions are generic protocol facts and contain no personal data.
    pub fn advertised_tan_media_versions(&self) -> Vec<u16> {
        self.engine.advertised_tan_media_versions()
    }

    /// The HKTAB/HITAB version selected for the latest discovery attempt.
    pub fn selected_tan_media_version(&self) -> Option<u16> {
        self.engine.selected_tan_media_version()
    }

    /// Typed responses from the most recently parsed bank message.
    ///
    /// Explicit response-text accessors are caller-owned display/logging data and
    /// remain omitted from crate error messages and `Debug` output.
    pub fn last_responses(&self) -> &[BankResponse] {
        self.engine.last_responses()
    }

    /// Bank responses from the most recent TAN-media discovery operation.
    ///
    /// These process-memory diagnostics preserve the discovery response across the
    /// internal HKEND exchange without changing [`Self::last_responses`] semantics.
    /// Explicit response-text accessors remain caller-owned display/logging data and
    /// are omitted from crate error messages and `Debug` output.
    pub fn last_tan_media_discovery_responses(&self) -> &[BankResponse] {
        self.engine.last_tan_media_discovery_responses()
    }

    /// Last initialization or parameter-refresh request stage reached in this process.
    pub fn last_initialization_stage(&self) -> Option<InitializationStage> {
        self.last_initialization_stage
    }

    /// Opt-in redacted initialization-decision facts for owner-attended diagnostics.
    ///
    /// This API exists only with the opt-in `development-diagnostics` feature.
    #[cfg(feature = "development-diagnostics")]
    pub fn development_initialization_recovery(
        &self,
    ) -> Option<crate::InitializationRecoveryFacts> {
        self.engine.development_initialization_recovery()
    }

    /// Opt-in redacted structure of the most recent TAN-medium discovery.
    ///
    /// This survives the internal HKEND exchange that replaces [`Self::last_responses`]
    /// with the termination response. It contains only ordered segment codes/versions,
    /// advertised/selected media-discovery versions, response codes/request-segment
    /// references, the safe HITANS/HKTAB decision fields, and returned-medium
    /// classification/occupancy booleans. It never contains medium identifiers,
    /// response text/parameters, or segment contents.
    #[cfg(feature = "development-diagnostics")]
    pub fn development_tan_media_discovery(&self) -> Option<&crate::TanMediaDiscoveryFacts> {
        self.engine.development_tan_media_discovery()
    }

    pub fn select_tan_method(&mut self, security_function: &str) -> Result<(), Error> {
        self.engine.choose_tan_method(security_function)
    }

    pub fn select_tan_medium(&mut self, name: &str) -> Result<(), Error> {
        self.engine.choose_tan_medium(name)
    }

    /// Refreshes anonymous BPD and closes any dialog the institution leaves open.
    pub fn refresh_parameters(&mut self, now: NaiveDateTime) -> Result<(), Error> {
        let Self {
            engine,
            transport,
            last_initialization_stage,
        } = self;
        *last_initialization_stage = Some(InitializationStage::AnonymousBpdRefresh);
        let stage = last_initialization_stage
            .as_mut()
            .expect("parameter refresh stage was set");
        refresh_parameters_with_send(engine, now, stage, |request| {
            transport.send(request).map_err(Error::from)
        })
    }

    /// Obtains a new assigned system ID and closes the synchronization dialog.
    pub fn synchronize(&mut self, now: NaiveDateTime) -> Result<Synchronization, Error> {
        let Self {
            engine, transport, ..
        } = self;
        synchronize_with_send(engine, now, |request| {
            transport.send(request).map_err(Error::from)
        })
    }

    /// Discovers TAN media using the dedicated process-4 initialization and the
    /// highest mutually supported HKTAB/HITAB version, then closes the dialog.
    pub fn discover_tan_media(&mut self, now: NaiveDateTime) -> Result<&[TanMedium], Error> {
        let Self {
            engine, transport, ..
        } = self;
        discover_tan_media_with_send(engine, now, |request| {
            transport.send(request).map_err(Error::from)
        })?;
        Ok(self.engine.tan_media())
    }

    /// Opens the personalized dialog used for subsequent supported operations.
    ///
    /// On first contact without a system ID or selected method, this opens the required
    /// function-999 synchronization dialog and retains the system ID returned by HISYN.
    /// If no selected method is usable, the method-discovery dialog is closed before
    /// `ChooseTanMethod` is returned. If a BPD-less function-999 discovery is globally
    /// terminated without mandatory response 3920, the client performs one anonymous
    /// BPD-zero refresh and one fresh discovery attempt. A repeated omission fails
    /// explicitly and is never retried in a loop.
    pub fn initialize(&mut self, now: NaiveDateTime) -> Result<Initialization, Error> {
        self.last_initialization_stage = Some(InitializationStage::InitialDiscovery);
        let Self {
            engine,
            transport,
            last_initialization_stage,
        } = self;
        let stage = last_initialization_stage
            .as_mut()
            .expect("initialization stage was set");
        initialize_with_send(engine, now, stage, |request| {
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
        InitializationResult::ChooseTanMethod
        | InitializationResult::RefreshParameters
        | InitializationResult::RefreshAndRediscover => Err(Error::InconsistentState),
        InitializationResult::Challenge(pending) => Ok(Initialization::Challenge(Box::new(
            InitializationContinuation { pending: *pending },
        ))),
    }
}

fn initialize_with_send(
    engine: &mut Engine,
    now: NaiveDateTime,
    stage: &mut InitializationStage,
    mut send: impl FnMut(&[u8]) -> Result<Vec<u8>, Error>,
) -> Result<Initialization, Error> {
    let result = request_initialization_with_send(engine, now, &mut send)?;
    if matches!(result, InitializationResult::RefreshAndRediscover) {
        // PIN/TAN B.4.3.1 requires current anonymous BPD before function-999
        // discovery. Repair the missing prerequisite once, close that anonymous
        // dialog exactly once, and repeat discovery from a fresh dialog.
        refresh_parameters_with_send(engine, now, stage, &mut send)?;
        *stage = InitializationStage::RepeatedDiscovery;
        let repeated = match request_initialization_with_send(engine, now, &mut send) {
            Err(Error::MissingValue {
                field: "3920 TAN method response",
            }) => {
                return Err(Error::MissingValue {
                    field: "3920 TAN method response after anonymous BPD refresh",
                });
            }
            result => result?,
        };
        if matches!(repeated, InitializationResult::RefreshAndRediscover) {
            return Err(Error::MissingValue {
                field: "3920 TAN method response after anonymous BPD refresh",
            });
        }
        return finish_initialization_with_send(engine, repeated, now, send);
    }
    finish_initialization_with_send(engine, result, now, send)
}

fn request_initialization_with_send(
    engine: &mut Engine,
    now: NaiveDateTime,
    send: &mut impl FnMut(&[u8]) -> Result<Vec<u8>, Error>,
) -> Result<InitializationResult, Error> {
    let request = engine.initialization_request(now.date(), now.time())?;
    let response = send(&request).inspect_err(|_error| {
        engine.abort_dialog();
    })?;
    engine.accept_initialization(&response, now)
}

fn refresh_parameters_with_send(
    engine: &mut Engine,
    now: NaiveDateTime,
    stage: &mut InitializationStage,
    mut send: impl FnMut(&[u8]) -> Result<Vec<u8>, Error>,
) -> Result<(), Error> {
    *stage = InitializationStage::AnonymousBpdRefresh;
    let request = engine.anonymous_initialization_request()?;
    let response = send(&request).inspect_err(|_error| {
        engine.abort_dialog();
    })?;
    engine.accept_anonymous_initialization(&response)?;
    *stage = InitializationStage::AnonymousTermination;
    let request = engine.termination_request(now.date(), now.time())?;
    let response = send(&request).inspect_err(|_error| {
        engine.abort_dialog();
    })?;
    engine.accept_termination(&response)
}

fn finish_initialization_with_send(
    engine: &mut Engine,
    result: InitializationResult,
    now: NaiveDateTime,
    mut send: impl FnMut(&[u8]) -> Result<Vec<u8>, Error>,
) -> Result<Initialization, Error> {
    let preserves_discovery_refresh = matches!(&result, InitializationResult::RefreshParameters);
    let outcome = match result {
        InitializationResult::Connected => return Ok(Initialization::Connected),
        InitializationResult::ChooseTanMethod => Initialization::ChooseTanMethod,
        InitializationResult::RefreshParameters => Initialization::RefreshParameters,
        InitializationResult::RefreshAndRediscover => return Err(Error::InconsistentState),
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
        if preserves_discovery_refresh {
            engine.accept_discovery_refresh_termination(&response)?;
        } else {
            engine.accept_termination(&response)?;
        }
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

fn discover_tan_media_with_send(
    engine: &mut Engine,
    now: NaiveDateTime,
    mut send: impl FnMut(&[u8]) -> Result<Vec<u8>, Error>,
) -> Result<(), Error> {
    let request = engine.tan_media_initialization_request(now.date(), now.time())?;
    let response = send(&request).inspect_err(|_error| {
        engine.abort_dialog();
    })?;
    let discovery = match engine.accept_tan_media_initialization(&response) {
        Ok(TanMediaInitializationResult::Complete) => Ok(()),
        Ok(TanMediaInitializationResult::OrderRequired) => (|| {
            let request = engine.tan_media_request(now.date(), now.time())?;
            let response = send(&request).inspect_err(|_error| {
                engine.abort_dialog();
            })?;
            engine.accept_tan_media_response(&response)
        })(),
        Err(error) => Err(error),
    };

    let termination = if engine.has_active_dialog() {
        let request = engine.termination_request(now.date(), now.time())?;
        let response = send(&request).inspect_err(|_error| {
            engine.abort_dialog();
        })?;
        engine.accept_termination(&response)
    } else {
        Ok(())
    };
    termination?;
    discovery
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
    use crate::{Limitation, model::TanProcess, transport::Transport, wire::Message};

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 7, 29)
            .unwrap()
            .and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap())
    }

    // PIN/TAN 2020 B.4.2.2/T32 prints the complete decoupled sequence:
    // initial order plus HKTAN 4, status HKTAN S, and a final response carrying
    // HITAN S with the original order's feedback/result. G112 C.12.1 supplies
    // the independently derived HKKKU/HIKKU 1 field order used below.
    #[test]
    fn decoupled_credit_card_poll_accepts_terminal_hitan_and_exhausts_pages() {
        let mut state = tan_media_state(TanProcess::Decoupled);
        state.selected_tan_medium = Some("Fictional push medium".to_owned());
        state.accounts.push(Account {
            iban: None,
            bic: None,
            account_number: Some("444433******1111".to_owned()),
            subaccount: None,
            institute: Some(crate::model::InstituteState {
                country_code: "280".to_owned(),
                institute_code: "12345678".to_owned(),
            }),
            currency: Some("EUR".to_owned()),
            account_type: Some(50),
            owner_name_1: Some("Fictional Person".to_owned()),
            owner_name_2: None,
            product_name: Some("Fictional Card".to_owned()),
            allowed_operations: vec![crate::model::OperationPermission {
                code: "HKKKU".to_owned(),
                required_signatures: 1,
            }],
            unlisted_operations_unknown: false,
        });
        state.credit_card_transactions_advertised = true;
        state.credit_card_transactions = Some(crate::model::CreditCardCapability {
            account_required: false,
            date_range_allowed: true,
        });
        state.credit_card_transactions_requires_tan = Some(true);

        let initialization =
            secured_response(b"HIRMG:2:2+0010::fictional initialization accepted'", 1, 3);
        let challenge = secured_response(
            concat!(
                "HIRMG:2:2+3060::fictional warning'",
                "HIRMS:3:2:4+3955::approve elsewhere'",
                "HITAN:4:7:4+4++fictional-card-reference+Approve fictional entries'"
            )
            .as_bytes(),
            2,
            5,
        );
        let terminal_first_page = secured_response(
            concat!(
                "HIRMG:2:2+0010::fictional poll accepted'",
                "HIRMS:3:2:3+3040::fictional next page:card-next'",
                "HIKKU:4:1:3+444433******1111+++++",
                "444433******1111:20260728:20260729:::::::9,00:EUR:D'",
                "HITAN:5:7:3+S++fictional-card-reference+nochallenge'"
            )
            .as_bytes(),
            3,
            6,
        );
        let final_page = secured_response(
            concat!(
                "HIRMG:2:2+0010::fictional page accepted'",
                "HIRMS:3:2:3+0020::fictional order executed'",
                "HIKKU:4:1:3+444433******1111+++++",
                "444433******1111:20260729:20260730:::::::10,00:EUR:D'",
                "HITAN:5:7:4+4++noref+nochallenge'"
            )
            .as_bytes(),
            4,
            6,
        );
        let mut client = client_with_state(
            state,
            [initialization, challenge, terminal_first_page, final_page],
        );

        assert!(matches!(
            client.initialize(now()).unwrap(),
            Initialization::Connected
        ));
        let continuation = match client
            .credit_card_transactions(0, None, None, now())
            .unwrap()
        {
            CreditCardTransactionRequest::Challenge(continuation) => *continuation,
            _ => panic!("expected decoupled credit-card approval"),
        };
        let poll_at = now()
            .checked_add_signed(chrono::TimeDelta::seconds(2))
            .unwrap();
        assert_eq!(continuation.earliest_poll_at(), Some(poll_at));

        let result = client
            .poll_credit_card_transactions(continuation, PollingMode::Manual, poll_at)
            .unwrap();
        let complete = match result {
            CreditCardTransactionRequest::Complete(result) => result,
            CreditCardTransactionRequest::Challenge(_) => {
                panic!("terminal decoupled result must not create another challenge")
            }
        };
        assert_eq!(complete.entries().len(), 2);

        let requests = client.transport.fixture_requests();
        assert_eq!(requests.len(), 4);
        let poll = Message::parse(&requests[2])
            .unwrap()
            .payload_segments()
            .unwrap();
        let poll_hktan = poll
            .iter()
            .find(|segment| segment.header().unwrap().code == b"HKTAN")
            .unwrap();
        assert_eq!(
            poll_hktan.elements()[1].components()[0].as_text().unwrap(),
            "S"
        );
        let next_page = Message::parse(&requests[3])
            .unwrap()
            .payload_segments()
            .unwrap();
        let next_hkkku = next_page
            .iter()
            .find(|segment| segment.header().unwrap().code == b"HKKKU")
            .unwrap();
        assert_eq!(
            next_hkkku.elements().last().unwrap().components()[0]
                .as_text()
                .unwrap(),
            "card-next"
        );
        assert!(!format!("{:?}", client.last_responses()).contains("fictional"));
        assert!(!format!("{:?}", client.last_responses()).contains("444433"));
    }

    #[test]
    fn public_client_and_continuation_surface_is_send() {
        fn assert_send<T: Send>() {}

        assert_send::<Client>();
        assert_send::<TraceSink>();

        assert_send::<Initialization>();
        assert_send::<Synchronization>();
        assert_send::<BalanceRequest>();
        assert_send::<BookedTransactionRequest>();
        assert_send::<DepotPositionRequest>();
        assert_send::<SecuritiesTransactionRequest>();
        assert_send::<CreditCardTransactionRequest>();
        assert_send::<CreditCardBalanceRequest>();

        assert_send::<InitializationContinuation>();
        assert_send::<SynchronizationContinuation>();
        assert_send::<BalanceContinuation>();
        assert_send::<BookedTransactionContinuation>();
        assert_send::<DepotPositionContinuation>();
        assert_send::<SecuritiesTransactionContinuation>();
        assert_send::<CreditCardTransactionContinuation>();
        assert_send::<CreditCardBalanceContinuation>();
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
            #[cfg(feature = "development-diagnostics")]
            development_medium_requirement: None,
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

    fn tan_media_state(process: TanProcess) -> ReusableState {
        tan_media_state_with_versions(process, &[5])
    }

    fn tan_media_state_with_versions(
        process: TanProcess,
        advertised_versions: &[u16],
    ) -> ReusableState {
        let mut state = ReusableState::new();
        state.system_id = Some("fictional-system".to_owned());
        state.tan_methods.push(TanMethod {
            security_function: "942".to_owned(),
            hktan_version: if process == TanProcess::Decoupled {
                7
            } else {
                6
            },
            process,
            technical_id: "fictional-medium-method".to_owned(),
            display_name: "Fictional medium approval".to_owned(),
            dk_method: (process == TanProcess::Decoupled).then(|| "Decoupled".to_owned()),
            max_tan_length: Some(6),
            tan_format: Some("1".to_owned()),
            medium_name_required: true,
            hhd_response_required: false,
            max_decoupled_polls: (process == TanProcess::Decoupled).then_some(3),
            first_poll_delay_seconds: (process == TanProcess::Decoupled).then_some(2),
            next_poll_delay_seconds: (process == TanProcess::Decoupled).then_some(3),
            manual_polling_allowed: process == TanProcess::Decoupled,
            automatic_polling_allowed: process == TanProcess::Decoupled,
            #[cfg(feature = "development-diagnostics")]
            development_medium_requirement: None,
        });
        state
            .advertised_parameter_segments
            .extend(advertised_versions.iter().copied().map(|version| {
                crate::capabilities::ParameterSegmentAdvertisement::new(
                    "HITABS".to_owned(),
                    version,
                )
            }));
        state.selected_tan_method = Some("942".to_owned());
        state
    }

    fn tan_media_initialization_response(hitan_version: u16) -> Vec<u8> {
        secured_response(
            format!(
                "HIRMG:2:2+0010::fictional initialization accepted'\
                 HITAN:3:{hitan_version}:5+4++noref+nochallenge'"
            )
            .as_bytes(),
            1,
            4,
        )
    }

    fn fictional_tan_medium(version: u16) -> String {
        let mut components = match version {
            2 => vec![""; 13],
            3 | 4 => vec![""; 14],
            5 => vec![""; 15],
            _ => panic!("fixture supports only specified media versions"),
        };
        components[0] = "M";
        components[1] = "1";
        let name_index = if version == 5 { 13 } else { 12 };
        components[name_index] = "Fictional phone";
        if matches!(version, 3 | 4) {
            components[13] = "?+49***123";
        } else if version == 5 {
            components[14] = "?+49***123";
        }
        components.join(":")
    }

    fn tan_media_operation_response(version: u16, medium: Option<&str>) -> Vec<u8> {
        let hitab = medium.map_or_else(
            || format!("HITAB:3:{version}:3+1"),
            |medium| format!("HITAB:3:{version}:3+1+{medium}"),
        );
        secured_response(
            format!(
                "HIRMG:2:2+0010::fictional order accepted'\
                 {hitab}'\
                 HIRMS:4:2:3+0020::fictional HKTAB processed'"
            )
            .as_bytes(),
            2,
            5,
        )
    }

    fn process_four_hitan_without_hitab(hitan_version: u16) -> Vec<u8> {
        process_four_hitan_without_hitab_with_active_count(hitan_version, 1)
    }

    fn process_four_hitan_without_hitab_with_active_count(
        hitan_version: u16,
        active_media_count: u8,
    ) -> Vec<u8> {
        process_four_hitan_without_hitab_with_requirement(hitan_version, 2, active_media_count)
    }

    fn process_four_hitan_without_hitab_with_requirement(
        hitan_version: u16,
        requirement_code: u8,
        active_media_count: u8,
    ) -> Vec<u8> {
        // Independently assembled from PIN/TAN B.4.3.1.3, B.5.1/B.5.2,
        // C.3.1.1, and correction T33. The parameter segments intentionally
        // mirror a broad fictional BPD response; none is derived from live
        // contents or from the request serializer under test.
        let (function_six, function_seven) = if hitan_version == 6 {
            ("942", "943")
        } else {
            ("943", "942")
        };
        let method_six = format!(
            "{function_six}:2:fictional-medium-method::1.0:Fictional medium approval:\
             6:1:Approval:2048:N:1:N:0:0:N:N:00:{requirement_code}:N:{active_media_count}"
        );
        let method_seven = format!(
            "{function_seven}:2:fictional-push:Decoupled:1.0:Fictional push approval:\
             6:1:Approval:2048:N:1:N:0:0:N:N:00:{requirement_code}:N:{active_media_count}:5:2:3:J:J"
        );
        let segments = vec![
            concat!(
                "HIRMG:2:2+0020::fictional accepted",
                "+3076::fictional SCA not required",
                "+3060::fictional parameters included"
            )
            .to_owned(),
            "HIRMS:3:2:3+0020::fictional identity accepted:private-parameter".to_owned(),
            concat!(
                "HIRMS:4:2:4+1040::fictional current BPD included",
                "+3920::fictional allowed methods:942",
                "+0940::fictional unpublished success"
            )
            .to_owned(),
            "HIBPA:5:3:4+78+280:12345678+Fictional Bank+9+1+300".to_owned(),
            "HIKOM:6:4:4+3+fints.example.invalid:443".to_owned(),
            "HIPINS:7:1:4+1+1+0+4:6:6:::HKTAB:N:HKTAN:N".to_owned(),
            "DIPINS:8:1:4+fictional unparsed parameters".to_owned(),
            "HIPAES:9:1:4+fictional unparsed parameters".to_owned(),
            "DIPAES:10:1:4+fictional unparsed parameters".to_owned(),
            format!("HITANS:11:6:4+1+1+0+N:N:0:{method_six}"),
            format!("HITANS:12:7:4+1+1+0+N:N:0:{method_seven}"),
            "HITABS:13:2:4+1+1+0".to_owned(),
            "HITABS:14:4:4+1+1+0".to_owned(),
            "HIPROS:15:3:4+fictional unparsed parameters".to_owned(),
            "HISPAS:16:1:4+fictional unparsed parameters".to_owned(),
            "HIFRDS:17:4:4+fictional unparsed parameters".to_owned(),
            "HIKKSS:18:1:4+1+1+0+J".to_owned(),
            "HIKKUS:19:1:4+1+1+0+90:J:J:J".to_owned(),
            format!("HITAN:20:{hitan_version}:5+4++noref+nochallenge"),
        ];
        let mut inner = segments.join("'");
        inner.push('\'');
        secured_response(inner.as_bytes(), 1, 21)
    }

    fn client_with_state(
        state: ReusableState,
        responses: impl IntoIterator<Item = Vec<u8>>,
    ) -> Client {
        Client {
            engine: Engine::new(
                InstituteId::new("280", "12345678").unwrap(),
                ProductIdentity::new("PROD123", "1.0").unwrap(),
                Credentials::new("fictional-user", None, "private-pin").unwrap(),
                state,
            )
            .unwrap(),
            transport: Transport::fixture(responses),
            last_initialization_stage: None,
        }
    }

    fn discovery_client(responses: impl IntoIterator<Item = Vec<u8>>) -> Client {
        let mut state = ReusableState::new();
        state.system_id = Some("fictional-existing-system".to_owned());
        client_with_state(state, responses)
    }

    fn first_contact_client(responses: impl IntoIterator<Item = Vec<u8>>) -> Client {
        client_with_state(ReusableState::new(), responses)
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

    fn function_999_response(
        dialog_id: &str,
        inner: &[u8],
        message_number: u16,
        trailer_number: u16,
    ) -> Vec<u8> {
        // Independently assembled PIN/TAN B.9/F.2 envelope for a profile-1
        // institute response. No production response builder is reused.
        let mut wire =
            format!("HNHBK:1:3+000000000000+300+{dialog_id}+{message_number}").into_bytes();
        wire.extend_from_slice(
            format!("+{dialog_id}:{message_number}'HNVSK:998:3+PIN:1+998+1+1::0+1").as_bytes(),
        );
        wire.extend_from_slice(b"+2:2:13:@8@");
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

    fn global_missing_3920_response() -> Vec<u8> {
        // Independently assembled PIN/TAN response: all three response elements
        // are global HIRMG facts. The unpublished 99xx has no assigned meaning.
        secured_response(
            concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional bank termination",
                "+9952::fictional unpublished companion'"
            )
            .as_bytes(),
            1,
            3,
        )
    }

    fn anonymous_bpd_response() -> Vec<u8> {
        plain_response(
            &[
                "HIRMG:2:2+0010::accepted",
                "HIBPA:3:3:3+58+280:12345678+Fictional Bank+9+1+300",
                concat!(
                    "HITANS:4:6:3+1+1+0+N:N:0:942:2:fictional-method::1.0:",
                    "Fictional approval:6:1:Approval:2048:N:1:N:0:0:N:N:00:0:N:1"
                ),
            ],
            "anonymous-refresh",
            1,
        )
    }

    // PIN/TAN 2020 B.4.3.1.3, C.3.1.1, archived E.2.1.2/E.2.1.4,
    // and Formals C.10: both HKTAN variants initialize process 4; when
    // HITAB is not returned with that response, a separate HKTAB order uses
    // the highest common version and is followed by exactly one HKEND.
    #[test]
    fn tan_media_discovery_six_and_seven_negotiate_legacy_and_current_versions() {
        for (process, version) in [
            (TanProcess::ProcessVariantTwo, 6),
            (TanProcess::Decoupled, 7),
        ] {
            for media_version in [2, 3, 4, 5] {
                let initialization = tan_media_initialization_response(version);
                let medium = fictional_tan_medium(media_version);
                let operation = tan_media_operation_response(media_version, Some(&medium));
                let termination = secured_response(b"HIRMG:2:2+0100::fictional terminated'", 3, 3);
                let mut client = client_with_state(
                    tan_media_state_with_versions(process, &[media_version]),
                    [initialization, operation, termination],
                );

                let media = client.discover_tan_media(now()).unwrap();
                assert_eq!(media.len(), 1);
                assert_eq!(media[0].name(), Some("Fictional phone"));
                assert_eq!(client.advertised_tan_media_versions(), [media_version]);
                assert_eq!(client.selected_tan_media_version(), Some(media_version));
                assert_eq!(client.transport.fixture_requests().len(), 3);
                assert_eq!(
                    client
                        .last_tan_media_discovery_responses()
                        .iter()
                        .map(|response| (response.code(), response.segment_number()))
                        .collect::<Vec<_>>(),
                    [(10, None), (10, None), (20, Some(3))]
                );
                let rendered = format!("{:?}", client.last_tan_media_discovery_responses());
                assert!(!rendered.contains("fictional"));
                assert!(!rendered.contains("Fictional phone"));

                let initialization =
                    Message::parse(&client.transport.fixture_requests()[0]).unwrap();
                let payload = initialization.payload_segments().unwrap();
                let hktan = payload
                    .iter()
                    .find(|segment| segment.header().unwrap().code == b"HKTAN")
                    .unwrap();
                assert_eq!(
                    crate::wire::encode_segments(std::slice::from_ref(hktan)).unwrap(),
                    format!("HKTAN:5:{version}+4+HKTAB+++++++++noref'").as_bytes()
                );

                let operation = Message::parse(&client.transport.fixture_requests()[1]).unwrap();
                let payload = operation.payload_segments().unwrap();
                let hktab = payload
                    .iter()
                    .find(|segment| segment.header().unwrap().code == b"HKTAB")
                    .unwrap();
                let expected = if media_version <= 3 {
                    format!("HKTAB:3:{media_version}+0'").into_bytes()
                } else {
                    format!("HKTAB:3:{media_version}+0+A'").into_bytes()
                };
                assert_eq!(
                    crate::wire::encode_segments(std::slice::from_ref(hktab)).unwrap(),
                    expected
                );

                // last_responses retains its documented most-recent-message
                // semantics and therefore contains the internal HKEND response.
                assert_eq!(
                    client
                        .last_responses()
                        .iter()
                        .map(BankResponse::code)
                        .collect::<Vec<_>>(),
                    [100]
                );

                #[cfg(feature = "development-diagnostics")]
                {
                    let facts = client.development_tan_media_discovery().unwrap();
                    assert_eq!(
                        facts
                            .received_segments()
                            .iter()
                            .map(|segment| (segment.code(), segment.version()))
                            .collect::<Vec<_>>(),
                        [
                            ("HIRMG", 2),
                            ("HITAN", version),
                            ("HIRMG", 2),
                            ("HITAB", media_version),
                            ("HIRMS", 2)
                        ]
                    );
                    assert_eq!(facts.advertised_versions(), [media_version]);
                    assert_eq!(facts.selected_version(), media_version);
                    assert_eq!(facts.discovered_medium_count(), Some(1));
                    assert_eq!(facts.tan_usage_option(), Some(1));
                    let request = facts.hktab_request().unwrap();
                    assert_eq!(request.version(), media_version);
                    assert_eq!(request.medium_type(), 0);
                    assert_eq!(
                        request.medium_class(),
                        (media_version >= 4).then_some(crate::TanMediumClass::All)
                    );
                    assert!(!request.medium_name_field_present());
                    assert_eq!(
                        facts.initialization_hktan_medium_name_supplied(),
                        Some(true)
                    );
                    assert_eq!(facts.returned_media().len(), 1);
                    assert!(facts.returned_media()[0].name_present());
                    let rendered = format!("{facts:?}");
                    assert!(!rendered.contains("Fictional phone"));
                    assert!(!rendered.contains("fictional accepted"));
                    assert!(!rendered.contains("fictional-system"));
                }
            }
        }
    }

    // PIN/TAN 2020 B.5.1/B.5.2 and DD "Verfahrensparameter
    // Zwei-Schritt-Verfahren" 6/7 make HKTAN DE 12 mandatory for requirement
    // code 2 with more than one advertised active medium. Archived E.2.1.4 and
    // DD "TAN-Medium-Liste" 4 puts field 6's national account DEG across
    // components 6-9, so field 10 (the matching selector) is component 13.
    // One delivered unnamed class-G record neither overrides the BPD condition
    // nor makes card fields HKTAN selectors.
    #[test]
    fn hitab_four_generator_name_controls_selection_for_both_hktan_versions() {
        for (process, hitan_version) in [
            (TanProcess::ProcessVariantTwo, 6),
            (TanProcess::Decoupled, 7),
        ] {
            for (
                requirement_code,
                active_media_count,
                medium,
                expected_name,
                _card_group_present,
                selectable,
            ) in [
                (2, 2, "G:1:::::::::::", None, false, false),
                (
                    2,
                    2,
                    "G:1:Fictional-card:7:::::::::Fictional Generator",
                    Some("Fictional Generator"),
                    true,
                    true,
                ),
                (2, 1, "G:1:::::::::::", None, false, true),
                (1, 2, "G:1:::::::::::", None, false, true),
            ] {
                let initialization = process_four_hitan_without_hitab_with_requirement(
                    hitan_version,
                    requirement_code,
                    active_media_count,
                );
                let operation = tan_media_operation_response(4, Some(medium));
                let termination = secured_response(b"HIRMG:2:2+0100::fictional terminated'", 3, 3);
                let mut client = client_with_state(
                    tan_media_state_with_versions(process, &[4, 2]),
                    [initialization, operation, termination],
                );

                let result = client
                    .discover_tan_media(now())
                    .map(|media| media.first().and_then(TanMedium::name));
                if selectable {
                    assert_eq!(result.unwrap(), expected_name);
                } else {
                    let error = result.unwrap_err();
                    assert!(matches!(
                        error,
                        Error::Unsupported(Limitation::TanMediumUnavailable)
                    ));
                    let rendered = format!("{error:?}");
                    assert!(!rendered.contains("Fictional Generator"));
                    assert!(!rendered.contains("fictional-system"));
                }

                assert_eq!(client.tan_media().len(), 1);
                assert_eq!(
                    client.tan_media()[0].class(),
                    crate::TanMediumClass::Generator
                );
                assert_eq!(client.tan_media()[0].name(), expected_name);
                assert_eq!(client.selected_tan_media_version(), Some(4));
                assert_eq!(client.transport.fixture_requests().len(), 3);
                assert_eq!(client.last_responses()[0].code(), 100);

                #[cfg(feature = "development-diagnostics")]
                {
                    let facts = client.development_tan_media_discovery().unwrap();
                    let requirement = facts.hitans_requirement().unwrap();
                    assert_eq!(requirement.hktan_version(), hitan_version);
                    assert_eq!(requirement.requirement_code(), requirement_code);
                    assert_eq!(requirement.requirement_field_number(), 19);
                    assert_eq!(requirement.requirement_component_index(), 18);
                    assert_eq!(requirement.active_media_count(), Some(active_media_count));
                    assert_eq!(requirement.active_media_count_field_number(), 21);
                    assert_eq!(requirement.active_media_count_component_index(), 20);
                    assert_eq!(
                        requirement.medium_name_required(),
                        requirement_code == 2 && active_media_count > 1
                    );

                    let request = facts.hktab_request().unwrap();
                    assert_eq!(request.version(), 4);
                    assert_eq!(request.medium_type(), 0);
                    assert_eq!(request.medium_class(), Some(crate::TanMediumClass::All));
                    assert!(!request.medium_name_field_present());
                    assert_eq!(
                        facts.initialization_hktan_medium_name_supplied(),
                        Some(true)
                    );

                    assert_eq!(facts.returned_media().len(), 1);
                    let returned = facts.returned_media()[0];
                    assert_eq!(returned.class(), crate::TanMediumClass::Generator);
                    assert_eq!(returned.status(), crate::TanMediumStatus::Active);
                    assert_eq!(returned.name_present(), expected_name.is_some());
                    assert_eq!(returned.card_number_present(), _card_group_present);
                    assert_eq!(returned.card_sequence_present(), _card_group_present);
                    let shape = &facts.returned_medium_shapes()[0];
                    assert_eq!(shape.component_count(), 13);
                    assert_eq!(
                        shape.occupied_components(),
                        if expected_name.is_some() {
                            &[1, 2, 3, 4, 13][..]
                        } else {
                            &[1, 2][..]
                        }
                    );

                    let rendered = format!("{facts:?}");
                    assert!(!rendered.contains("Fictional-card"));
                    assert!(!rendered.contains("Fictional Generator"));
                    assert!(!rendered.contains("fictional-system"));
                }
            }
        }
    }

    // The same official sections require a HITAB response even when its
    // optional list is empty. Formals C.10 ties HITABS advertisements to the
    // corresponding HKTAB/HITAB version; advertising only legacy 2/4 does not
    // authorize inventing a version-5 response or a UPD-first bootstrap. A
    // missing HITAB remains distinguishable after the required HKEND response
    // replaces last_responses.
    #[test]
    fn missing_hitab_is_typed_and_still_terminates_once_for_both_hktan_versions() {
        for (process, version) in [
            (TanProcess::ProcessVariantTwo, 6),
            (TanProcess::Decoupled, 7),
        ] {
            let initialization = process_four_hitan_without_hitab(version);
            let order_without_hitab = secured_response(
                b"HIRMG:2:2+0010::fictional HKTAB accepted'\
                  HIRMS:3:2:3+0020::fictional order processed'",
                2,
                4,
            );
            let termination = secured_response(b"HIRMG:2:2+0100::fictional terminated'", 3, 3);
            let mut client = client_with_state(
                tan_media_state_with_versions(process, &[2, 4]),
                [initialization, order_without_hitab, termination],
            );

            assert!(matches!(
                client.discover_tan_media(now()),
                Err(Error::MissingValue {
                    field: "HITAB TAN media response"
                })
            ));
            assert_eq!(client.transport.fixture_requests().len(), 3);
            assert_eq!(client.advertised_tan_media_versions(), [4, 2]);
            assert_eq!(client.selected_tan_media_version(), Some(4));
            assert_eq!(client.last_responses()[0].code(), 100);
            assert_eq!(client.state().bpd_version(), 78);
            let operation_responses = client.last_tan_media_discovery_responses();
            assert_eq!(
                operation_responses
                    .iter()
                    .map(|response| (response.code(), response.segment_number()))
                    .collect::<Vec<_>>(),
                [
                    (20, None),
                    (3076, None),
                    (3060, None),
                    (20, Some(3)),
                    (1040, Some(4)),
                    (3920, Some(4)),
                    (940, Some(4)),
                    (10, None),
                    (20, Some(3)),
                ]
            );
            assert_eq!(
                operation_responses
                    .iter()
                    .map(BankResponse::text)
                    .collect::<Vec<_>>(),
                [
                    "fictional accepted",
                    "fictional SCA not required",
                    "fictional parameters included",
                    "fictional identity accepted",
                    "fictional current BPD included",
                    "fictional allowed methods",
                    "fictional unpublished success",
                    "fictional HKTAB accepted",
                    "fictional order processed",
                ]
            );
            let rendered = format!("{operation_responses:?}");
            assert!(!rendered.contains("fictional"));
            assert!(!rendered.contains("private-parameter"));

            #[cfg(feature = "development-diagnostics")]
            {
                let facts = client.development_tan_media_discovery().unwrap();
                assert_eq!(
                    facts
                        .received_segments()
                        .iter()
                        .map(|segment| (segment.code(), segment.version()))
                        .collect::<Vec<_>>(),
                    [
                        ("HIRMG", 2),
                        ("HIRMS", 2),
                        ("HIRMS", 2),
                        ("HIBPA", 3),
                        ("HIKOM", 4),
                        ("HIPINS", 1),
                        ("DIPINS", 1),
                        ("HIPAES", 1),
                        ("DIPAES", 1),
                        ("HITANS", 6),
                        ("HITANS", 7),
                        ("HITABS", 2),
                        ("HITABS", 4),
                        ("HIPROS", 3),
                        ("HISPAS", 1),
                        ("HIFRDS", 4),
                        ("HIKKSS", 1),
                        ("HIKKUS", 1),
                        ("HITAN", version),
                        ("HIRMG", 2),
                        ("HIRMS", 2),
                    ]
                );
                assert_eq!(
                    facts
                        .received_responses()
                        .iter()
                        .map(|response| (response.code(), response.segment_number()))
                        .collect::<Vec<_>>(),
                    [
                        (20, None),
                        (3076, None),
                        (3060, None),
                        (20, Some(3)),
                        (1040, Some(4)),
                        (3920, Some(4)),
                        (940, Some(4)),
                        (10, None),
                        (20, Some(3)),
                    ]
                );
                assert_eq!(facts.advertised_versions(), [4, 2]);
                assert_eq!(facts.selected_version(), 4);
                assert_eq!(facts.discovered_medium_count(), None);
                let rendered = format!("{facts:?}");
                assert!(!rendered.contains("fictional"));
                assert!(!rendered.contains("private-parameter"));
                assert!(!rendered.contains("942"));
            }
        }
    }

    // A received HITAB with an unsupported version is not skipped. The safe
    // structural diagnostic identifies it without retaining any segment data.
    #[test]
    fn unsupported_hitab_version_remains_typed_and_structurally_visible() {
        let initialization = tan_media_initialization_response(6);
        let discovery =
            secured_response(b"HIRMG:2:2+0010::fictional accepted'HITAB:3:6:3+1'", 2, 4);
        let termination = secured_response(b"HIRMG:2:2+0100::fictional terminated'", 3, 3);
        let mut client = client_with_state(
            tan_media_state(TanProcess::ProcessVariantTwo),
            [initialization, discovery, termination],
        );

        assert!(matches!(
            client.discover_tan_media(now()),
            Err(Error::UnsupportedSegment {
                code: "HITAB",
                version: 6
            })
        ));
        assert_eq!(client.transport.fixture_requests().len(), 3);

        #[cfg(feature = "development-diagnostics")]
        {
            let facts = client.development_tan_media_discovery().unwrap();
            assert_eq!(
                facts
                    .received_segments()
                    .iter()
                    .map(|segment| (segment.code(), segment.version()))
                    .collect::<Vec<_>>(),
                [("HIRMG", 2), ("HITAN", 6), ("HIRMG", 2), ("HITAB", 6)]
            );
            assert_eq!(facts.discovered_medium_count(), None);
        }
    }

    // Formals C.10 requires the highest common advertised operation version.
    // An unsupported-only advertisement is a local capability result and must
    // not emit even the process-4 initialization.
    #[test]
    fn tan_media_version_selection_is_deterministic_and_fails_before_transport() {
        let initialization = tan_media_initialization_response(6);
        let medium = fictional_tan_medium(4);
        let operation = tan_media_operation_response(4, Some(&medium));
        let termination = secured_response(b"HIRMG:2:2+0100::fictional terminated'", 3, 3);
        let mut mixed = client_with_state(
            tan_media_state_with_versions(TanProcess::ProcessVariantTwo, &[2, 4, 2]),
            [initialization, operation, termination],
        );
        mixed.discover_tan_media(now()).unwrap();
        assert_eq!(mixed.advertised_tan_media_versions(), [4, 2]);
        assert_eq!(mixed.selected_tan_media_version(), Some(4));

        let mut unsupported = client_with_state(
            tan_media_state_with_versions(TanProcess::ProcessVariantTwo, &[1, 6]),
            std::iter::empty::<Vec<u8>>(),
        );
        assert!(matches!(
            unsupported.discover_tan_media(now()),
            Err(Error::Unsupported(Limitation::TanMediumVersion))
        ));
        assert!(unsupported.transport.fixture_requests().is_empty());
        assert_eq!(unsupported.advertised_tan_media_versions(), [6, 1]);
        assert_eq!(unsupported.selected_tan_media_version(), None);
    }

    // A successful initialization may atomically replace BPD. The subsequent
    // order must use that response's highest common version, never the stale
    // version that was available before opening the dialog.
    #[test]
    fn tan_media_order_renegotiates_after_initialization_replaces_bpd() {
        let initialization = process_four_hitan_without_hitab(6);
        let medium = fictional_tan_medium(4);
        let operation = tan_media_operation_response(4, Some(&medium));
        let termination = secured_response(b"HIRMG:2:2+0100::fictional terminated'", 3, 3);
        let mut client = client_with_state(
            tan_media_state_with_versions(TanProcess::ProcessVariantTwo, &[5]),
            [initialization, operation, termination],
        );

        client.discover_tan_media(now()).unwrap();
        assert_eq!(client.advertised_tan_media_versions(), [4, 2]);
        assert_eq!(client.selected_tan_media_version(), Some(4));
        let request = Message::parse(&client.transport.fixture_requests()[1]).unwrap();
        let payload = request.payload_segments().unwrap();
        let hktab = payload
            .iter()
            .find(|segment| segment.header().unwrap().code == b"HKTAB")
            .unwrap();
        assert_eq!(
            crate::wire::encode_segments(std::slice::from_ref(hktab)).unwrap(),
            b"HKTAB:3:4+0+A'"
        );
    }

    fn assert_security_function(request: &[u8], expected: &str) {
        let payload = Message::parse(request).unwrap().payload_segments().unwrap();
        let hnshk = payload
            .iter()
            .find(|segment| segment.header().unwrap().code == b"HNSHK")
            .expect("personalized request has HNSHK");
        assert_eq!(
            hnshk.elements()[2].components()[0].as_text().unwrap(),
            expected
        );
    }

    fn initialization_error(result: Result<Initialization, Error>) -> Error {
        match result {
            Err(error) => error,
            Ok(_) => panic!("fictional initialization unexpectedly succeeded"),
        }
    }

    // FinTS 3.0 Formals C.8/C.8.1-C.8.2: a first PIN/TAN contact without a
    // system ID is a synchronization initialization containing HKSYN 3 and
    // returning HISYN 4. PIN/TAN B.4.3.1 and correction T2 allow function 999
    // to discover the user-valid method while the same response supplies BPD.
    #[test]
    fn first_contact_synchronizes_before_selected_method_initialization() {
        let synchronization = function_999_response(
            "first-contact",
            concat!(
                "HIRMG:2:2+0010::fictional synchronization accepted'",
                "HIRMS:3:2:5+3920::fictional methods:942'",
                "HIBPA:4:3:3+58+280:12345678+Fictional Bank+9+1+300'",
                "HITANS:5:6:3+1+1+0+N:N:0:942:2:fictional-method::1.0:",
                "Fictional approval:6:1:Approval:2048:N:1:N:0:0:N:N:00:0:N:1'",
                "HISYN:6:4:5+fictional-first-system'"
            )
            .as_bytes(),
            1,
            7,
        );
        let synchronization_termination = function_999_response(
            "first-contact",
            b"HIRMG:2:2+0100::fictional synchronization terminated'",
            2,
            3,
        );
        let selected_initialization =
            secured_response(b"HIRMG:2:2+0010::fictional initialization accepted'", 1, 3);
        let mut client = first_contact_client([
            synchronization,
            synchronization_termination,
            selected_initialization,
        ]);

        assert!(matches!(
            client.initialize(now()).unwrap(),
            Initialization::ChooseTanMethod
        ));
        assert_eq!(client.state().system_id(), Some("fictional-first-system"));
        assert_eq!(client.state().bpd_version(), 58);
        assert_eq!(client.allowed_tan_methods(), ["942"]);
        assert_eq!(client.tan_methods().len(), 1);

        let requests = client.transport.fixture_requests();
        let first = Message::parse(&requests[0]).unwrap();
        let payload = first.payload_segments().unwrap();
        assert_eq!(
            payload
                .iter()
                .map(|segment| segment.header().unwrap().code.to_vec())
                .collect::<Vec<_>>(),
            [b"HNSHK", b"HKIDN", b"HKVVB", b"HKSYN", b"HNSHA"]
        );
        let hnshk = &payload[0];
        assert_eq!(hnshk.elements()[1].components()[1].as_text().unwrap(), "1");
        assert_eq!(
            hnshk.elements()[2].components()[0].as_text().unwrap(),
            "999"
        );
        assert_eq!(hnshk.elements()[6].components()[2].as_text().unwrap(), "0");
        let hkidn = &payload[1];
        assert_eq!(hkidn.elements()[3].components()[0].as_text().unwrap(), "0");
        assert_eq!(hkidn.elements()[4].components()[0].as_text().unwrap(), "1");
        let hksyn = &payload[3];
        assert_eq!(hksyn.header().unwrap().version, 3);
        assert_eq!(hksyn.elements()[1].components()[0].as_text().unwrap(), "0");

        let termination = Message::parse(&requests[1]).unwrap();
        let termination_payload = termination.payload_segments().unwrap();
        assert_eq!(
            termination_payload[0].elements()[6].components()[2]
                .as_text()
                .unwrap(),
            "fictional-first-system"
        );

        client.select_tan_method("942").unwrap();
        assert!(matches!(
            client.initialize(now()).unwrap(),
            Initialization::Connected
        ));
        let requests = client.transport.fixture_requests();
        let selected = Message::parse(&requests[2]).unwrap();
        let selected_payload = selected.payload_segments().unwrap();
        assert!(
            selected_payload
                .iter()
                .all(|segment| segment.header().unwrap().code != b"HKSYN")
        );
        let selected_hkidn = selected_payload
            .iter()
            .find(|segment| segment.header().unwrap().code == b"HKIDN")
            .unwrap();
        assert_eq!(
            selected_hkidn.elements()[3].components()[0]
                .as_text()
                .unwrap(),
            "fictional-first-system"
        );
        assert_eq!(client.transport.fixture_responses_remaining(), 0);
    }

    // PIN/TAN B.6.1/B.8.2 permits the function-999 discovery response to end
    // the dialog with 9800/9955 while still returning 3920. If that response
    // also completes mandatory HKSYN with HISYN, the assigned ID remains
    // reusable and the client must not send HKEND to the terminated dialog.
    #[test]
    fn bank_terminated_first_contact_retains_hisyn_without_hkend() {
        let response = function_999_response(
            "terminated-first-contact",
            concat!(
                "HIRMG:2:2+9050::fictional summary+9800::fictional termination'",
                "HIRMS:3:2:5+9955::fictional method requirement",
                "+3920::fictional methods:942'",
                "HIBPA:4:3:3+58+280:12345678+Fictional Bank+9+1+300'",
                "HITANS:5:6:3+1+1+0+N:N:0:942:2:fictional-method::1.0:",
                "Fictional approval:6:1:Approval:2048:N:1:N:0:0:N:N:00:0:N:1'",
                "HISYN:6:4:5+fictional-terminated-system'"
            )
            .as_bytes(),
            1,
            7,
        );
        let mut client = first_contact_client([response]);

        assert!(matches!(
            client.initialize(now()).unwrap(),
            Initialization::ChooseTanMethod
        ));
        assert_eq!(
            client.state().system_id(),
            Some("fictional-terminated-system")
        );
        assert_eq!(client.transport.fixture_requests().len(), 1);
        assert_eq!(client.transport.fixture_responses_remaining(), 0);
    }

    // Formals C.8.2 makes HISYN mandatory in a synchronization response. A
    // successful-looking 3920/BPD response cannot silently complete first
    // contact without the assigned system ID.
    #[test]
    fn first_contact_rejects_missing_hisyn_without_persisting_state() {
        let response = function_999_response(
            "missing-hisyn",
            concat!(
                "HIRMG:2:2+0010::fictional accepted'",
                "HIRMS:3:2:5+3920::fictional methods:942'",
                "HIBPA:4:3:3+58+280:12345678+Fictional Bank+9+1+300'",
                "HITANS:5:6:3+1+1+0+N:N:0:942:2:fictional-method::1.0:",
                "Fictional approval:6:1:Approval:2048:N:1:N:0:0:N:N:00:0:N:1'"
            )
            .as_bytes(),
            1,
            6,
        );
        let mut client = first_contact_client([response]);

        assert!(matches!(
            client.initialize(now()),
            Err(Error::MissingValue {
                field: "assigned system ID"
            })
        ));
        assert!(client.state().system_id().is_none());
        assert_eq!(client.transport.fixture_requests().len(), 1);
    }

    // A caller may have a reusable system ID but no retained selected method.
    // In that established-system case, PIN/TAN B.4.3.1 discovery remains the
    // ordinary function-999 initialization and must not request a replacement.
    #[test]
    fn established_system_method_discovery_does_not_resynchronize() {
        let discovery = function_999_response(
            "existing-system",
            concat!(
                "HIRMG:2:2+0010::fictional accepted'",
                "HIRMS:3:2:4+3920::fictional methods:942'",
                "HIBPA:4:3:3+58+280:12345678+Fictional Bank+9+1+300'",
                "HITANS:5:6:3+1+1+0+N:N:0:942:2:fictional-method::1.0:",
                "Fictional approval:6:1:Approval:2048:N:1:N:0:0:N:N:00:0:N:1'"
            )
            .as_bytes(),
            1,
            6,
        );
        let termination = function_999_response(
            "existing-system",
            b"HIRMG:2:2+0100::fictional terminated'",
            2,
            3,
        );
        let mut client = discovery_client([discovery, termination]);

        assert!(matches!(
            client.initialize(now()).unwrap(),
            Initialization::ChooseTanMethod
        ));
        let requests = client.transport.fixture_requests();
        let payload = Message::parse(&requests[0])
            .unwrap()
            .payload_segments()
            .unwrap();
        assert!(
            payload
                .iter()
                .all(|segment| segment.header().unwrap().code != b"HKSYN")
        );
        let hkidn = payload
            .iter()
            .find(|segment| segment.header().unwrap().code == b"HKIDN")
            .unwrap();
        assert_eq!(
            hkidn.elements()[3].components()[0].as_text().unwrap(),
            "fictional-existing-system"
        );
        assert_eq!(
            client.state().system_id(),
            Some("fictional-existing-system")
        );
    }

    // PIN/TAN 2020 B.4.3.1 makes current anonymous BPD a prerequisite for
    // function-999 discovery and requires 3920 to carry the user methods.
    // T8 repairs missing usable parameters through an active BPD-zero refresh.
    // The no-3920 bank deviation receives one bounded prerequisite repair only.
    #[test]
    fn initialize_repairs_bpd_once_before_repeating_global_method_discovery() {
        let rediscovery = secured_response(
            b"HIRMG:2:2+0010::accepted'HIRMS:3:2:4+3920::methods:942:943'",
            1,
            4,
        );
        let responses = [
            global_missing_3920_response(),
            anonymous_bpd_response(),
            plain_response(
                &["HIRMG:2:2+0100::anonymous terminated"],
                "anonymous-refresh",
                2,
            ),
            rediscovery,
            secured_response(b"HIRMG:2:2+0100::discovery terminated'", 2, 3),
        ];
        let mut client = discovery_client(responses);

        assert!(matches!(
            client.initialize(now()).unwrap(),
            Initialization::ChooseTanMethod
        ));
        assert_eq!(
            client.last_initialization_stage(),
            Some(InitializationStage::RepeatedDiscovery)
        );
        assert_eq!(client.state().bpd_version(), 58);
        assert_eq!(client.tan_methods().len(), 1);
        assert_eq!(client.tan_methods()[0].security_function(), "942");
        assert_eq!(client.allowed_tan_methods(), ["942", "943"]);
        assert!(matches!(
            client.select_tan_method("943"),
            Err(Error::Unsupported(crate::Limitation::TanMethod))
        ));
        client.select_tan_method("942").unwrap();

        let requests = client.transport.fixture_requests();
        assert_eq!(requests.len(), 5);
        assert_security_function(&requests[0], "999");

        let anonymous = Message::parse(&requests[1])
            .unwrap()
            .payload_segments()
            .unwrap();
        assert!(
            anonymous
                .iter()
                .all(|segment| segment.header().unwrap().code != b"HNSHK")
        );
        let hkvvb = anonymous
            .iter()
            .find(|segment| segment.header().unwrap().code == b"HKVVB")
            .unwrap();
        assert_eq!(hkvvb.elements()[1].components()[0].as_text().unwrap(), "0");

        for index in [2, 4] {
            let payload = Message::parse(&requests[index])
                .unwrap()
                .payload_segments()
                .unwrap();
            assert_eq!(
                payload
                    .iter()
                    .filter(|segment| segment.header().unwrap().code == b"HKEND")
                    .count(),
                1
            );
        }
        assert_security_function(&requests[3], "999");
        assert_eq!(client.transport.fixture_responses_remaining(), 0);
    }

    // The bounded recovery is not a loop: after a successful anonymous refresh,
    // another global termination without mandatory 3920 is a redacted typed error.
    #[test]
    fn initialize_stops_after_second_missing_3920() {
        let responses = [
            global_missing_3920_response(),
            anonymous_bpd_response(),
            plain_response(
                &["HIRMG:2:2+0100::anonymous terminated"],
                "anonymous-refresh",
                2,
            ),
            global_missing_3920_response(),
        ];
        let mut client = discovery_client(responses);

        let error = initialization_error(client.initialize(now()));
        assert_eq!(
            client.last_initialization_stage(),
            Some(InitializationStage::RepeatedDiscovery)
        );
        assert!(matches!(
            error,
            Error::MissingValue {
                field: "3920 TAN method response after anonymous BPD refresh"
            }
        ));
        assert_eq!(client.transport.fixture_requests().len(), 4);
        assert_eq!(client.transport.fixture_responses_remaining(), 0);
        assert!(client.allowed_tan_methods().is_empty());
        assert_eq!(
            client
                .last_responses()
                .iter()
                .map(|response| (response.code(), response.segment_number()))
                .collect::<Vec<_>>(),
            [(9050, None), (9800, None), (9952, None)]
        );
        assert_eq!(
            client
                .last_responses()
                .iter()
                .map(BankResponse::text)
                .collect::<Vec<_>>(),
            [
                "fictional summary",
                "fictional bank termination",
                "fictional unpublished companion"
            ]
        );
        let rendered = format!("{error:?}");
        assert!(!rendered.contains("fictional"));
        assert!(!format!("{:?}", client.last_responses()).contains("fictional"));
    }

    #[test]
    fn initialization_stage_identifies_anonymous_refresh_failure() {
        let mut client = discovery_client([
            global_missing_3920_response(),
            global_missing_3920_response(),
        ]);

        let error = initialization_error(client.initialize(now()));

        assert_eq!(
            client.last_initialization_stage(),
            Some(InitializationStage::AnonymousBpdRefresh)
        );
        assert!(matches!(&error, Error::Bank(response) if response.code() == 9050));
        assert_eq!(client.transport.fixture_requests().len(), 2);
        assert_eq!(
            format!("{:?}", client.last_initialization_stage()),
            "Some(AnonymousBpdRefresh)"
        );
        #[cfg(feature = "development-diagnostics")]
        {
            let facts = client.development_initialization_recovery().unwrap();
            assert!(!facts.usable_selected_method_present());
            assert!(facts.bpd_zero());
            assert!(facts.upd_zero());
            assert!(facts.tan_parameters_empty());
            assert!(!facts.has_tan_method_response());
            assert!(facts.global_abort_shape());
            assert!(facts.eligible());
        }
        assert!(!format!("{error:?}").contains("fictional"));
        assert!(!format!("{:?}", client.last_responses()).contains("fictional"));
    }

    // PIN/TAN B.4.3.1 and correction T8 require anonymous BPD to describe the
    // institute-specific methods returned by 3920. Formals C.3.2.2 requires a
    // BPD-zero request to receive complete current BPD. If that mandatory
    // source is instead terminated with no BPD, no standardized bootstrap
    // remains and the client must neither retry nor invent a method.
    #[test]
    fn sequential_discovery_and_anonymous_bpd_failure_is_a_typed_limitation() {
        let personalized = function_999_response(
            "terminated-discovery",
            concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional termination",
                "+9952::fictional unpublished companion",
                "+3920::fictional methods:942'"
            )
            .as_bytes(),
            1,
            3,
        );
        let anonymous = plain_response(
            &[concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional termination",
                "+9952::fictional unpublished companion",
                "+3920::fictional methods:942"
            )],
            "anonymous-refused",
            1,
        );
        let mut client = discovery_client([personalized, anonymous]);

        assert!(matches!(
            client.initialize(now()).unwrap(),
            Initialization::RefreshParameters
        ));
        assert_eq!(
            client.last_initialization_stage(),
            Some(InitializationStage::InitialDiscovery)
        );

        let error = client.refresh_parameters(now()).unwrap_err();

        assert!(matches!(
            error,
            Error::Unsupported(Limitation::TanMethodParametersUnavailable)
        ));
        assert_eq!(
            client.last_initialization_stage(),
            Some(InitializationStage::AnonymousBpdRefresh)
        );
        assert_eq!(client.transport.fixture_requests().len(), 2);
        assert_eq!(client.transport.fixture_responses_remaining(), 0);
        assert!(!client.engine.has_active_dialog());
        assert_eq!(client.state().bpd_version(), 0);
        assert_eq!(client.state().upd_version(), 0);
        assert!(client.tan_methods().is_empty());
        assert_eq!(client.allowed_tan_methods(), ["942"]);
        assert!(matches!(
            client.select_tan_method("942"),
            Err(Error::Unsupported(Limitation::TanMethod))
        ));
        assert_eq!(
            client
                .last_responses()
                .iter()
                .map(|response| (response.code(), response.segment_number()))
                .collect::<Vec<_>>(),
            [(9050, None), (9800, None), (9952, None), (3920, None)]
        );

        let anonymous_request = Message::parse(&client.transport.fixture_requests()[1]).unwrap();
        let payload = anonymous_request.payload_segments().unwrap();
        assert!(
            payload
                .iter()
                .all(|segment| segment.header().unwrap().code != b"HNSHK")
        );
        let hkvvb = payload
            .iter()
            .find(|segment| segment.header().unwrap().code == b"HKVVB")
            .unwrap();
        assert_eq!(hkvvb.elements()[1].components()[0].as_text().unwrap(), "0");
        assert!(!format!("{error:?}").contains("fictional"));
        assert!(!format!("{:?}", client.last_responses()).contains("fictional"));
    }

    // The typed limitation is confined to the exact response set. A published
    // credential error in the anonymous response remains a bank rejection.
    #[test]
    fn anonymous_bpd_failure_does_not_absorb_published_errors() {
        let personalized = function_999_response(
            "terminated-discovery",
            concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional termination",
                "+9952::fictional unpublished companion",
                "+3920::fictional methods:942'"
            )
            .as_bytes(),
            1,
            3,
        );
        let anonymous = plain_response(
            &[concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional termination",
                "+9952::fictional unpublished companion",
                "+9942::fictional credential error",
                "+3920::fictional methods:942"
            )],
            "anonymous-refused",
            1,
        );
        let mut client = discovery_client([personalized, anonymous]);
        assert!(matches!(
            client.initialize(now()).unwrap(),
            Initialization::RefreshParameters
        ));

        let error = client.refresh_parameters(now()).unwrap_err();

        assert!(matches!(&error, Error::Bank(response) if response.code() == 9050));
        assert_eq!(
            client.last_initialization_stage(),
            Some(InitializationStage::AnonymousBpdRefresh)
        );
        assert_eq!(client.transport.fixture_requests().len(), 2);
        assert!(!format!("{error:?}").contains("fictional"));

        let standalone = plain_response(
            &[concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional termination",
                "+9952::fictional unpublished companion",
                "+3920::fictional methods:942"
            )],
            "anonymous-refused",
            1,
        );
        let mut standalone_client = discovery_client([standalone]);

        let error = standalone_client.refresh_parameters(now()).unwrap_err();

        assert!(matches!(&error, Error::Bank(response) if response.code() == 9050));
        assert_eq!(standalone_client.transport.fixture_requests().len(), 1);
        assert!(!format!("{error:?}").contains("fictional"));
    }

    // The persistent stage diagnostic distinguishes a successful anonymous
    // initialization from its subsequent HKEND request without wire details.
    #[test]
    fn direct_parameter_refresh_records_anonymous_termination_stage() {
        let responses = [
            anonymous_bpd_response(),
            plain_response(
                &["HIRMG:2:2+0100::anonymous terminated"],
                "anonymous-refresh",
                2,
            ),
        ];
        let mut client = discovery_client(responses);

        client.refresh_parameters(now()).unwrap();

        assert_eq!(
            client.last_initialization_stage(),
            Some(InitializationStage::AnonymousTermination)
        );
        assert_eq!(client.transport.fixture_requests().len(), 2);
        assert_eq!(client.transport.fixture_responses_remaining(), 0);
    }

    // Placement is part of the narrow classification. A segment-referenced
    // unpublished companion without 3920 is not the exact global prerequisite
    // failure and therefore does not start an anonymous network sequence.
    #[test]
    fn initialize_does_not_refresh_for_segment_referenced_missing_3920() {
        let response = secured_response(
            concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional termination'",
                "HIRMS:3:2:4+9952::fictional segment companion'"
            )
            .as_bytes(),
            1,
            4,
        );
        let mut client = discovery_client([response]);

        let error = initialization_error(client.initialize(now()));
        assert!(matches!(
            error,
            Error::MissingValue {
                field: "3920 TAN method response"
            }
        ));
        assert_eq!(client.transport.fixture_requests().len(), 1);
        assert_eq!(client.transport.fixture_responses_remaining(), 0);
    }

    // Rückmeldungscodes 2026 A/B.4: unpublished 99xx values remain
    // uninterpreted, while published credential errors and selected-method
    // authentication failures are never absorbed by BPD recovery.
    #[test]
    fn initialize_recovery_does_not_absorb_fatal_or_selected_method_errors() {
        let fatal = secured_response(
            concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional termination",
                "+9952::fictional companion",
                "+9942::fictional credential error'"
            )
            .as_bytes(),
            1,
            3,
        );
        let mut client = discovery_client([fatal]);
        let error = initialization_error(client.initialize(now()));
        assert_eq!(
            client.last_initialization_stage(),
            Some(InitializationStage::InitialDiscovery)
        );
        assert!(matches!(&error, Error::Bank(response) if response.code() == 9050));
        assert_eq!(client.transport.fixture_requests().len(), 1);
        #[cfg(feature = "development-diagnostics")]
        {
            let facts = client.development_initialization_recovery().unwrap();
            assert!(!facts.global_abort_shape());
            assert!(!facts.eligible());
            assert!(!format!("{facts:?}").contains("fictional"));
        }
        assert!(!format!("{error:?}").contains("fictional"));
        assert!(!format!("{:?}", client.last_responses()).contains("fictional"));

        let mut selected = Client {
            engine: synchronization_engine(),
            transport: Transport::fixture([global_missing_3920_response()]),
            last_initialization_stage: None,
        };
        let error = initialization_error(selected.initialize(now()));
        assert_eq!(
            selected.last_initialization_stage(),
            Some(InitializationStage::InitialDiscovery)
        );
        assert!(matches!(&error, Error::Bank(response) if response.code() == 9050));
        #[cfg(feature = "development-diagnostics")]
        {
            let facts = selected.development_initialization_recovery().unwrap();
            assert!(facts.usable_selected_method_present());
            assert!(!facts.eligible());
        }
        let requests = selected.transport.fixture_requests();
        assert_eq!(requests.len(), 1);
        assert_security_function(&requests[0], "942");
    }

    // PIN/TAN B.6.1 and correction T8: a bank-terminated function-999
    // refresh outcome receives no HKEND, while an open successful discovery
    // with the same missing method descriptions is closed exactly once.
    #[test]
    fn refresh_outcome_closes_only_an_open_discovery_dialog() {
        let new_engine = || {
            let mut state = ReusableState::new();
            state.system_id = Some("fictional-existing-system".to_owned());
            Engine::new(
                InstituteId::new("280", "12345678").unwrap(),
                ProductIdentity::new("PROD123", "1.0").unwrap(),
                Credentials::new("fictional-user", None, "private-pin").unwrap(),
                state,
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

    // PIN/TAN B.4.3.1 and F.2.5 require HKEND after an open function-999
    // discovery. Formals C.1.2 says 9800 proves institute-side termination,
    // while the current return-code register gives unpublished 9952 no
    // standalone meaning. Preserve the pending T8 refresh outcome only for
    // this exact, fully validated two-response sequence.
    #[test]
    fn open_discovery_refresh_survives_exact_bank_terminated_hkend_response() {
        let initial = function_999_response(
            "open-discovery",
            b"HIRMG:2:2+0010::accepted'HIRMS:3:2:4+3920::methods:942'",
            1,
            4,
        );
        let termination = function_999_response(
            "open-discovery",
            concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional bank termination",
                "+9952::fictional unpublished companion'"
            )
            .as_bytes(),
            2,
            3,
        );
        let mut client = discovery_client([initial, termination]);

        assert!(matches!(
            client.initialize(now()).unwrap(),
            Initialization::RefreshParameters
        ));
        assert_eq!(client.transport.fixture_requests().len(), 2);
        assert_eq!(client.transport.fixture_responses_remaining(), 0);
        assert!(!client.engine.has_active_dialog());
        assert_eq!(
            client
                .last_responses()
                .iter()
                .map(|response| (response.code(), response.segment_number()))
                .collect::<Vec<_>>(),
            [(9050, None), (9800, None), (9952, None)]
        );

        let request = &client.transport.fixture_requests()[1];
        let message = Message::parse(request).unwrap();
        let outer = message.segments();
        assert_eq!(
            outer
                .iter()
                .map(|segment| {
                    let header = segment.header().unwrap();
                    (header.code.to_vec(), header.number, header.version)
                })
                .collect::<Vec<_>>(),
            [
                (b"HNHBK".to_vec(), 1, 3),
                (b"HNVSK".to_vec(), 998, 3),
                (b"HNVSD".to_vec(), 999, 1),
                (b"HNHBS".to_vec(), 5, 1),
            ]
        );
        let payload = message.payload_segments().unwrap();
        assert_eq!(
            payload
                .iter()
                .map(|segment| {
                    let header = segment.header().unwrap();
                    (header.code.to_vec(), header.number, header.version)
                })
                .collect::<Vec<_>>(),
            [
                (b"HNSHK".to_vec(), 2, 4),
                (b"HKEND".to_vec(), 3, 1),
                (b"HNSHA".to_vec(), 4, 2),
            ]
        );
        let hnshk = &payload[0];
        assert_eq!(
            hnshk.elements()[1].components()[0].as_text().unwrap(),
            "PIN"
        );
        assert_eq!(hnshk.elements()[1].components()[1].as_text().unwrap(), "1");
        assert_eq!(
            hnshk.elements()[2].components()[0].as_text().unwrap(),
            "999"
        );
        assert_eq!(hnshk.elements()[3].components()[0].as_text().unwrap(), "2");
        assert_eq!(
            hnshk.elements()[6].components()[2].as_text().unwrap(),
            "fictional-existing-system"
        );
        assert_eq!(
            payload[1].elements()[1].components()[0].as_text().unwrap(),
            "open-discovery"
        );
        assert_eq!(
            payload[2].elements()[1].components()[0].as_text().unwrap(),
            "2"
        );
        #[cfg(feature = "development-diagnostics")]
        {
            let facts = client.development_initialization_recovery().unwrap();
            assert!(facts.has_tan_method_response());
            assert!(!facts.global_abort_shape());
            assert!(!facts.eligible());
            assert!(!format!("{facts:?}").contains("fictional"));
        }
        assert!(!format!("{:?}", client.last_responses()).contains("fictional"));
    }

    // Adjacent termination shapes remain fatal: a published credential error
    // cannot be absorbed by the discovery-refresh outcome, even alongside the
    // three otherwise matching global errors.
    #[test]
    fn discovery_refresh_does_not_absorb_published_hkend_error() {
        let initial = function_999_response(
            "open-discovery",
            b"HIRMG:2:2+0010::accepted'HIRMS:3:2:4+3920::methods:942'",
            1,
            4,
        );
        let termination = function_999_response(
            "open-discovery",
            concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional bank termination",
                "+9952::fictional unpublished companion",
                "+9942::fictional credential error'"
            )
            .as_bytes(),
            2,
            3,
        );
        let mut client = discovery_client([initial, termination]);

        let error = initialization_error(client.initialize(now()));

        assert!(matches!(&error, Error::Bank(response) if response.code() == 9050));
        assert_eq!(client.transport.fixture_requests().len(), 2);
        assert!(!format!("{error:?}").contains("fictional"));
        assert!(!format!("{:?}", client.last_responses()).contains("fictional"));
    }

    // The same termination response is not generally successful. When the
    // discovery already has a usable HITANS description, the pending outcome
    // is ChooseTanMethod and the ordinary HKEND error remains fatal.
    #[test]
    fn exact_hkend_error_is_not_accepted_for_choose_method_outcome() {
        let initial = function_999_response(
            "open-discovery",
            concat!(
                "HIRMG:2:2+0010::accepted'",
                "HIRMS:3:2:4+3920::methods:942'",
                "HIBPA:4:3:3+58+280:12345678+Fictional Bank+9+1+300'",
                "HITANS:5:6:3+1+1+0+N:N:0:942:2:fictional-method::1.0:",
                "Fictional approval:6:1:Approval:2048:N:1:N:0:0:N:N:00:0:N:1'"
            )
            .as_bytes(),
            1,
            6,
        );
        let termination = function_999_response(
            "open-discovery",
            concat!(
                "HIRMG:2:2+9050::fictional summary",
                "+9800::fictional bank termination",
                "+9952::fictional unpublished companion'"
            )
            .as_bytes(),
            2,
            3,
        );
        let mut client = discovery_client([initial, termination]);

        let error = initialization_error(client.initialize(now()));

        assert!(matches!(&error, Error::Bank(response) if response.code() == 9050));
        assert_eq!(client.transport.fixture_requests().len(), 2);
        assert_eq!(client.tan_methods().len(), 1);
        assert_eq!(client.allowed_tan_methods(), ["942"]);
        assert!(!format!("{error:?}").contains("fictional"));
        assert!(!format!("{:?}", client.last_responses()).contains("fictional"));
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
