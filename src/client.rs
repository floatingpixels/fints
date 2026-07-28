use chrono::NaiveDateTime;

use crate::{
    engine::{
        BalanceResult, Engine, InitializationResult, PendingChallenge, SynchronizationResult,
    },
    error::{BankResponse, Error},
    model::{
        Account, Balance, Challenge, Credentials, InstituteId, ProductIdentity, ReusableState, Tan,
        TanMedium, TanMethod, TanProcess,
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

/// Concrete synchronous Gate 1 FinTS client.
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

    /// Opens the personalized dialog used for subsequent Gate 1 operations.
    ///
    /// If no selected method is usable, the method-discovery dialog is closed before
    /// `ChooseTanMethod` is returned.
    pub fn initialize(&mut self, now: NaiveDateTime) -> Result<Initialization, Error> {
        let request = self.engine.initialization_request(now.date(), now.time())?;
        let response = self.send(&request)?;
        match self.engine.accept_initialization(&response, now)? {
            InitializationResult::Connected => Ok(Initialization::Connected),
            InitializationResult::ChooseTanMethod => {
                self.terminate(now)?;
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
