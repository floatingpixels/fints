use std::collections::HashSet;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};

#[cfg(feature = "development-diagnostics")]
use crate::development_diagnostics::InitializationRecoveryFacts;
use crate::{
    client::PollingMode,
    error::{BankResponse, Error, InputError, Limitation},
    model::{
        Balance, BookedEntry, BookedTransactions, Challenge, Credentials, InstituteId,
        ProductIdentity, ReusableState, Tan, TanMedium, TanMethod, TanProcess, TransactionFormat,
        valid_latin1_length,
    },
    response::Response,
    segments::{self, SecurityContext},
};

mod initialization;
mod products;

const LOCAL_DECOUPLED_POLL_LIMIT: u16 = 20;
const LOCAL_CONTINUATION_LIMIT: u16 = 20;
const LOCAL_TRANSACTION_PAGE_LIMIT: u16 = 100;
const LOCAL_TRANSACTION_ENTRY_LIMIT: usize = 10_000;

pub(crate) struct Engine {
    institute: InstituteId,
    product: ProductIdentity,
    credentials: Credentials,
    state: ReusableState,
    transient_bpd: Option<ReusableState>,
    allowed_tan_methods: Vec<String>,
    allowed_tan_methods_known: bool,
    tan_media: Vec<TanMedium>,
    transient_accounts: Option<Vec<crate::model::Account>>,
    last_responses: Vec<BankResponse>,
    #[cfg(feature = "development-diagnostics")]
    development_initialization_recovery: Option<InitializationRecoveryFacts>,
    requested_balance: Option<crate::model::Account>,
    transaction: Option<TransactionState>,
    products: products::ProductStates,
    continuation_active: bool,
    dialog: Option<DialogState>,
}

struct DialogState {
    id: String,
    next_message_number: u16,
    security_function: String,
    profile_version: &'static str,
    system_id: String,
    method: Option<TanMethod>,
    anonymous: bool,
}

struct TransactionState {
    account: crate::model::Account,
    format: TransactionFormat,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    requires_tan: bool,
    continuation_point: Option<String>,
    seen_continuation_points: HashSet<String>,
    pages_requested: u16,
    entries: Vec<BookedEntry>,
}

pub(crate) enum InitializationResult {
    Connected,
    ChooseTanMethod,
    RefreshParameters,
    RefreshAndRediscover,
    Challenge(Box<PendingChallenge>),
}

pub(crate) enum SynchronizationResult {
    Complete,
    Challenge(Box<PendingChallenge>),
}

pub(crate) enum BalanceResult {
    Complete(Box<Balance>),
    Challenge(Box<PendingChallenge>),
}

pub(crate) enum TransactionsResult {
    Complete(Box<BookedTransactions>),
    Continue,
    Challenge(Box<PendingChallenge>),
}

pub(crate) use products::{
    CreditCardBalanceResult, CreditCardTransactionsResult, DepotPositionsResult,
    SecuritiesTransactionsResult,
};

pub(crate) struct PendingChallenge {
    pub(crate) challenge: Challenge,
    pub(crate) method: TanMethod,
    operation: PendingOperation,
    dialog_id: String,
    polls: u16,
    continuations: u16,
    pub(crate) next_poll_at: Option<NaiveDateTime>,
    assigned_system_id: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingOperation {
    Synchronization,
    Initialization,
    Balance,
    Transactions,
    DepotPositions,
    SecuritiesTransactions,
    CreditCardBalance,
    CreditCardTransactions,
}

impl Engine {
    pub(crate) fn new(
        institute: InstituteId,
        product: ProductIdentity,
        credentials: Credentials,
        mut state: ReusableState,
    ) -> Result<Self, Error> {
        if !state.advertised_balance_versions.is_empty() {
            state.balance_versions = state
                .advertised_balance_versions
                .iter()
                .copied()
                .filter(|version| (5..=8).contains(version))
                .collect();
            state.balance_versions.sort_unstable_by(|a, b| b.cmp(a));
            state.balance_versions.dedup();
        }
        validate_reusable_state(&state)?;
        Ok(Self {
            institute,
            product,
            credentials,
            state,
            transient_bpd: None,
            allowed_tan_methods: Vec::new(),
            allowed_tan_methods_known: false,
            tan_media: Vec::new(),
            transient_accounts: None,
            last_responses: Vec::new(),
            #[cfg(feature = "development-diagnostics")]
            development_initialization_recovery: None,
            requested_balance: None,
            transaction: None,
            products: products::ProductStates::default(),
            continuation_active: false,
            dialog: None,
        })
    }

    pub(crate) fn state(&self) -> &ReusableState {
        &self.state
    }

    pub(crate) fn into_state(self) -> ReusableState {
        self.state
    }

    pub(crate) fn tan_methods(&self) -> &[TanMethod] {
        &self.parameters().tan_methods
    }

    pub(crate) fn accounts(&self) -> &[crate::model::Account] {
        self.transient_accounts
            .as_deref()
            .unwrap_or(&self.state.accounts)
    }

    pub(crate) fn allowed_tan_methods(&self) -> &[String] {
        &self.allowed_tan_methods
    }

    pub(crate) fn tan_media(&self) -> &[TanMedium] {
        &self.tan_media
    }

    pub(crate) fn last_responses(&self) -> &[BankResponse] {
        &self.last_responses
    }

    #[cfg(feature = "development-diagnostics")]
    pub(crate) fn development_initialization_recovery(
        &self,
    ) -> Option<InitializationRecoveryFacts> {
        self.development_initialization_recovery
    }

    pub(crate) fn choose_tan_method(&mut self, security_function: &str) -> Result<(), Error> {
        if security_function.len() != 3
            || !security_function.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(InputError::TanMethod.into());
        }
        if !self
            .parameters()
            .tan_methods
            .iter()
            .any(|method| method.security_function == security_function)
            || (self.allowed_tan_methods_known
                && !self
                    .allowed_tan_methods
                    .iter()
                    .any(|method| method == security_function))
        {
            return Err(Limitation::TanMethod.into());
        }
        self.state.selected_tan_method = Some(security_function.to_owned());
        Ok(())
    }

    pub(crate) fn choose_tan_medium(&mut self, name: &str) -> Result<(), Error> {
        if name.is_empty()
            || !encoding_rs::mem::is_str_latin1(name)
            || encoding_rs::mem::encode_latin1_lossy(name).len() > 32
        {
            return Err(InputError::TanMedium.into());
        }
        if !self.tan_media.is_empty()
            && !self
                .tan_media
                .iter()
                .any(|medium| medium.name.as_deref() == Some(name))
        {
            return Err(Limitation::TanMedium.into());
        }
        self.state.selected_tan_medium = Some(name.to_owned());
        Ok(())
    }

    pub(crate) fn anonymous_initialization_request(&self) -> Result<Vec<u8>, Error> {
        self.ensure_no_dialog()?;
        segments::anonymous_initialization(&self.institute, &self.product, &self.state)
    }

    pub(crate) fn accept_anonymous_initialization(&mut self, input: &[u8]) -> Result<(), Error> {
        let response = Response::parse(input)?;
        self.record_responses(&response);
        self.apply_parameter_refresh(&response)?;
        if let Some(error) = response.first_error() {
            return Err(Error::Bank(error));
        }
        self.dialog = Some(DialogState {
            id: response.dialog_id().to_owned(),
            next_message_number: response
                .message_number()
                .checked_add(1)
                .ok_or(Error::InconsistentState)?,
            security_function: String::new(),
            profile_version: "1",
            system_id: "0".to_owned(),
            method: None,
            anonymous: true,
        });
        Ok(())
    }

    pub(crate) fn synchronization_request(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_no_dialog()?;
        let method = self.selected_method()?.clone();
        validate_supported_method(&method)?;
        validate_medium(&method, self.state.selected_tan_medium.as_deref())?;
        let context = self.initial_context(&method, date, time);
        segments::synchronization(
            &context,
            &self.product,
            &self.state,
            Some(&method),
            self.state.selected_tan_medium.as_deref(),
        )
    }

    pub(crate) fn accept_synchronization(
        &mut self,
        input: &[u8],
        received_at: NaiveDateTime,
    ) -> Result<SynchronizationResult, Error> {
        let response = Response::parse(input)?;
        self.record_responses(&response);
        self.apply_parameters(&response)?;
        if let Some(error) = response.first_error() {
            return Err(Error::Bank(error));
        }
        let system_id = response.system_id()?;
        let method = self.selected_method()?.clone();
        self.dialog = Some(DialogState {
            id: response.dialog_id().to_owned(),
            next_message_number: response
                .message_number()
                .checked_add(1)
                .ok_or(Error::InconsistentState)?,
            security_function: method.security_function.clone(),
            profile_version: "2",
            system_id: system_id.clone().unwrap_or_else(|| "0".to_owned()),
            method: Some(method.clone()),
            anonymous: false,
        });
        if let Some(tan_response) = response.tan(method.hktan_version)?
            && tan_response.challenge.reference != "noref"
        {
            validate_tan_process(&tan_response.process, "4")?;
            let mut pending = pending_challenge(
                tan_response.challenge,
                PendingOperation::Synchronization,
                method,
                response.dialog_id(),
                received_at,
            )?;
            pending.assigned_system_id = system_id;
            self.continuation_active = true;
            return Ok(SynchronizationResult::Challenge(Box::new(pending)));
        }
        self.state.system_id = Some(system_id.ok_or(Error::MissingValue {
            field: "assigned system ID",
        })?);
        self.continuation_active = false;
        Ok(SynchronizationResult::Complete)
    }

    pub(crate) fn balance_request(
        &mut self,
        account_index: usize,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        if self.continuation_active {
            return Err(Error::InconsistentState);
        }
        let account = self
            .accounts()
            .get(account_index)
            .ok_or(Limitation::BalanceNotAuthorized)?
            .clone();
        if !account.allows_balance() {
            return Err(Limitation::BalanceNotAuthorized.into());
        }
        let signatures = account.required_signatures("HKSAL").unwrap_or(0);
        if signatures > 1 {
            return Err(Limitation::MultipleSigners.into());
        }
        let parameters = self.parameters();
        let version = self
            .parameters()
            .balance_versions
            .iter()
            .copied()
            .find(|version| (5..=8).contains(version))
            .ok_or(if parameters.balance_capability_advertised {
                Limitation::BalanceVersion
            } else {
                Limitation::BalanceNotAdvertised
            })?;
        let tan = match parameters.balance_requires_tan {
            Some(true) => {
                let method = self.active_method()?.clone();
                Some((method, self.state.selected_tan_medium.clone()))
            }
            Some(false) => None,
            None => return Err(Limitation::PinTanParameters.into()),
        };
        let message = {
            let context = self.context(date, time)?;
            segments::balance_request(
                &context,
                &account,
                version,
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        self.requested_balance = Some(account);
        Ok(message)
    }

    pub(crate) fn accept_balance(
        &mut self,
        input: &[u8],
        received_at: NaiveDateTime,
    ) -> Result<BalanceResult, Error> {
        let response = self.accept_dialog_response(input)?;
        if let Some(error) = response.first_error() {
            self.requested_balance = None;
            return Err(Error::Bank(error));
        }
        if let Some(balance) = response.balance()? {
            let requested = self
                .requested_balance
                .take()
                .ok_or(Error::InconsistentState)?;
            if !requested.same_identity(&balance.account) {
                return Err(Error::InconsistentState);
            }
            self.continuation_active = false;
            return Ok(BalanceResult::Complete(Box::new(balance)));
        }
        let method = self.active_method()?.clone();
        let tan_response = response
            .tan(method.hktan_version)?
            .ok_or(Error::MissingValue {
                field: "balance or HITAN response",
            })?;
        validate_tan_process(&tan_response.process, "4")?;
        self.continuation_active = true;
        Ok(BalanceResult::Challenge(Box::new(pending_challenge(
            tan_response.challenge,
            PendingOperation::Balance,
            method,
            response.dialog_id(),
            received_at,
        )?)))
    }

    pub(crate) fn transaction_request(
        &mut self,
        account_index: usize,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        if self.continuation_active || self.transaction.is_some() {
            return Err(Error::InconsistentState);
        }
        if from.zip(to).is_some_and(|(from, to)| to < from) {
            return Err(InputError::TransactionDateRange.into());
        }
        let account = self
            .accounts()
            .get(account_index)
            .ok_or(Limitation::TransactionsNotAuthorized)?
            .clone();
        let (format, requires_tan) = self.transaction_format(&account)?;
        let tan = if requires_tan {
            let method = self.active_method()?.clone();
            Some((method, self.state.selected_tan_medium.clone()))
        } else {
            None
        };
        let message = {
            let context = self.context(date, time)?;
            segments::transaction_request(
                &context,
                segments::TransactionRequest {
                    account: &account,
                    format: &format,
                    from,
                    to,
                    continuation_point: None,
                },
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        self.transaction = Some(TransactionState {
            account,
            format,
            from,
            to,
            requires_tan,
            continuation_point: None,
            seen_continuation_points: HashSet::new(),
            pages_requested: 1,
            entries: Vec::new(),
        });
        Ok(message)
    }

    pub(crate) fn next_transaction_page_request(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        if self.continuation_active {
            return Err(Error::InconsistentState);
        }
        let transaction = self.transaction.as_ref().ok_or(Error::InconsistentState)?;
        if transaction.pages_requested >= LOCAL_TRANSACTION_PAGE_LIMIT {
            return self.fail_transaction(Error::PaginationLimitReached);
        }
        let continuation_point = transaction
            .continuation_point
            .clone()
            .ok_or(Error::InconsistentState)?;
        let account = transaction.account.clone();
        let format = transaction.format.clone();
        let from = transaction.from;
        let to = transaction.to;
        let requires_tan = transaction.requires_tan;
        let tan = if requires_tan {
            let method = self.active_method()?.clone();
            Some((method, self.state.selected_tan_medium.clone()))
        } else {
            None
        };
        let message = {
            let context = self.context(date, time)?;
            segments::transaction_request(
                &context,
                segments::TransactionRequest {
                    account: &account,
                    format: &format,
                    from,
                    to,
                    continuation_point: Some(&continuation_point),
                },
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        let next_page = self
            .transaction
            .as_ref()
            .ok_or(Error::InconsistentState)?
            .pages_requested
            .checked_add(1);
        let Some(next_page) = next_page else {
            return self.fail_transaction(Error::PaginationLimitReached);
        };
        self.transaction
            .as_mut()
            .ok_or(Error::InconsistentState)?
            .pages_requested = next_page;
        Ok(message)
    }

    pub(crate) fn accept_transactions(
        &mut self,
        input: &[u8],
        received_at: NaiveDateTime,
    ) -> Result<TransactionsResult, Error> {
        let response = self.accept_dialog_response(input)?;
        self.accept_transaction_response(response, None, received_at)
    }

    pub(crate) fn tan_submission_request(
        &mut self,
        pending: &PendingChallenge,
        tan: &Tan,
        now: NaiveDateTime,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_pending_dialog(pending)?;
        if !self.continuation_active {
            return Err(Error::InconsistentState);
        }
        ensure_not_expired(&pending.challenge, now)?;
        if pending.method.process != TanProcess::ProcessVariantTwo {
            return Err(Limitation::TanProcessVariant.into());
        }
        if pending.method.hhd_response_required {
            return Err(Limitation::PinTanParameters.into());
        }
        let tan_length = encoding_rs::mem::encode_latin1_lossy(&tan.value).len();
        if pending
            .method
            .max_tan_length
            .is_some_and(|maximum| tan_length > usize::from(maximum))
        {
            return Err(InputError::Tan.into());
        }
        match pending.method.tan_format.as_deref() {
            Some("1") if !tan.value.bytes().all(|byte| byte.is_ascii_digit()) => {
                return Err(InputError::Tan.into());
            }
            None | Some("1" | "2") => {}
            Some(_) => return Err(Limitation::PinTanParameters.into()),
        }
        let context = self.context(now.date(), now.time())?;
        segments::tan_submission(
            &context,
            &pending.method,
            &pending.challenge.reference,
            &tan.value,
        )
    }

    pub(crate) fn decoupled_poll_request(
        &mut self,
        pending: &mut PendingChallenge,
        mode: PollingMode,
        now: NaiveDateTime,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_pending_dialog(pending)?;
        if !self.continuation_active {
            return Err(Error::InconsistentState);
        }
        ensure_not_expired(&pending.challenge, now)?;
        if pending.method.process != TanProcess::Decoupled {
            return Err(Limitation::TanMethod.into());
        }
        let allowed = match mode {
            PollingMode::Manual => pending.method.manual_polling_allowed,
            PollingMode::Automatic => pending.method.automatic_polling_allowed,
        };
        if !allowed {
            return Err(Limitation::DecoupledPolling.into());
        }
        let bank_limit = pending
            .method
            .max_decoupled_polls
            .unwrap_or(LOCAL_DECOUPLED_POLL_LIMIT);
        let limit = bank_limit.min(LOCAL_DECOUPLED_POLL_LIMIT);
        if pending.polls >= limit {
            return Err(Error::PollLimitReached);
        }
        if pending.next_poll_at.is_some_and(|earliest| now < earliest) {
            return Err(Error::PollTooEarly);
        }
        pending.polls += 1;
        pending.next_poll_at = pending
            .method
            .next_poll_delay_seconds
            .map(|seconds| {
                now.checked_add_signed(TimeDelta::seconds(i64::from(seconds)))
                    .ok_or(Error::InconsistentState)
            })
            .transpose()?;
        let context = self.context(now.date(), now.time())?;
        segments::decoupled_poll(&context, &pending.method, &pending.challenge.reference)
    }

    pub(crate) fn accept_initialization_continuation(
        &mut self,
        input: &[u8],
        pending: PendingChallenge,
    ) -> Result<InitializationResult, Error> {
        self.ensure_pending_dialog(&pending)?;
        if pending.operation != PendingOperation::Initialization {
            return Err(Error::InconsistentState);
        }
        let response = self.accept_dialog_response(input)?;
        if let Some(error) = response.first_error() {
            return Err(Error::Bank(error));
        }
        if let Some(tan_response) = response.tan(pending.method.hktan_version)?
            && tan_response.challenge.reference != "noref"
        {
            validate_continuation_process(&tan_response.process, pending.method.process)?;
            return Ok(InitializationResult::Challenge(Box::new(
                continued_challenge(pending, tan_response.challenge)?,
            )));
        }
        self.continuation_active = false;
        Ok(InitializationResult::Connected)
    }

    pub(crate) fn accept_balance_continuation(
        &mut self,
        input: &[u8],
        pending: PendingChallenge,
    ) -> Result<BalanceResult, Error> {
        self.ensure_pending_dialog(&pending)?;
        if pending.operation != PendingOperation::Balance {
            return Err(Error::InconsistentState);
        }
        let response = self.accept_dialog_response(input)?;
        if let Some(error) = response.first_error() {
            return Err(Error::Bank(error));
        }
        if let Some(balance) = response.balance()? {
            let requested = self
                .requested_balance
                .take()
                .ok_or(Error::InconsistentState)?;
            if !requested.same_identity(&balance.account) {
                return Err(Error::InconsistentState);
            }
            self.continuation_active = false;
            return Ok(BalanceResult::Complete(Box::new(balance)));
        }
        let tan_response =
            response
                .tan(pending.method.hktan_version)?
                .ok_or(Error::MissingValue {
                    field: "continuation HITAN",
                })?;
        validate_continuation_process(&tan_response.process, pending.method.process)?;
        Ok(BalanceResult::Challenge(Box::new(continued_challenge(
            pending,
            tan_response.challenge,
        )?)))
    }

    pub(crate) fn accept_transactions_continuation(
        &mut self,
        input: &[u8],
        pending: PendingChallenge,
        received_at: NaiveDateTime,
    ) -> Result<TransactionsResult, Error> {
        self.ensure_pending_dialog(&pending)?;
        if pending.operation != PendingOperation::Transactions {
            return Err(Error::InconsistentState);
        }
        let response = self.accept_dialog_response(input)?;
        self.accept_transaction_response(response, Some(pending), received_at)
    }

    pub(crate) fn accept_synchronization_continuation(
        &mut self,
        input: &[u8],
        pending: PendingChallenge,
    ) -> Result<SynchronizationResult, Error> {
        self.ensure_pending_dialog(&pending)?;
        if pending.operation != PendingOperation::Synchronization {
            return Err(Error::InconsistentState);
        }
        let response = self.accept_dialog_response(input)?;
        if let Some(error) = response.first_error() {
            return Err(Error::Bank(error));
        }
        if let Some(tan_response) = response.tan(pending.method.hktan_version)?
            && tan_response.challenge.reference != "noref"
        {
            validate_continuation_process(&tan_response.process, pending.method.process)?;
            return Ok(SynchronizationResult::Challenge(Box::new(
                continued_challenge(pending, tan_response.challenge)?,
            )));
        }
        let system_id =
            response
                .system_id()?
                .or(pending.assigned_system_id)
                .ok_or(Error::MissingValue {
                    field: "assigned system ID",
                })?;
        self.state.system_id = Some(system_id.clone());
        if let Some(dialog) = &mut self.dialog {
            dialog.system_id = system_id;
        }
        self.continuation_active = false;
        Ok(SynchronizationResult::Complete)
    }

    pub(crate) fn termination_request(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        let dialog = self.dialog.as_ref().ok_or(Error::InconsistentState)?;
        if dialog.anonymous {
            segments::anonymous_termination(&dialog.id, dialog.next_message_number)
        } else {
            let context = self.context(date, time)?;
            segments::termination(&context)
        }
    }

    pub(crate) fn accept_termination(&mut self, input: &[u8]) -> Result<(), Error> {
        let response = self.accept_dialog_response(input)?;
        if let Some(error) = response.first_error() {
            return Err(Error::Bank(error));
        }
        if !response
            .responses()
            .iter()
            .any(|response| response.code() == 100)
        {
            return Err(Error::MissingValue {
                field: "dialog termination response",
            });
        }
        self.dialog = None;
        self.continuation_active = false;
        self.requested_balance = None;
        self.transaction = None;
        self.products.clear();
        self.transient_bpd = None;
        Ok(())
    }

    pub(crate) fn abort_dialog(&mut self) {
        self.dialog = None;
        self.continuation_active = false;
        self.requested_balance = None;
        self.transaction = None;
        self.products.clear();
        self.transient_bpd = None;
    }

    fn accept_dialog_response(&mut self, input: &[u8]) -> Result<Response, Error> {
        let response = Response::parse(input)?;
        self.record_responses(&response);
        if response.is_dialog_abort()
            && let Some(error) = response.first_error()
        {
            // Formals B.7.6: the institute already ended the dialog and may
            // not know its ID or message number. Preserve the actionable bank
            // response instead of masking it as a local state mismatch.
            self.abort_dialog();
            return Err(Error::Bank(error));
        }
        let dialog = self.dialog.as_mut().ok_or(Error::InconsistentState)?;
        let expected_message_number = dialog.next_message_number;
        if response.dialog_id() != dialog.id || response.message_number() != expected_message_number
        {
            return Err(Error::InconsistentState);
        }
        dialog.next_message_number = dialog
            .next_message_number
            .checked_add(1)
            .ok_or(Error::InconsistentState)?;
        Ok(response)
    }

    fn record_responses(&mut self, response: &Response) {
        self.last_responses.clear();
        self.last_responses.extend_from_slice(response.responses());
    }

    fn selected_method(&self) -> Result<&TanMethod, Error> {
        let selected = self
            .state
            .selected_tan_method
            .as_deref()
            .ok_or(Limitation::TanMethod)?;
        self.parameters()
            .tan_methods
            .iter()
            .find(|method| method.security_function == selected)
            .ok_or_else(|| Limitation::TanMethod.into())
    }

    fn parameters(&self) -> &ReusableState {
        self.transient_bpd.as_ref().unwrap_or(&self.state)
    }

    fn ensure_no_dialog(&self) -> Result<(), Error> {
        if self.dialog.is_none() {
            Ok(())
        } else {
            Err(Error::InconsistentState)
        }
    }

    pub(crate) fn has_active_dialog(&self) -> bool {
        self.dialog.is_some()
    }

    fn ensure_pending_dialog(&self, pending: &PendingChallenge) -> Result<(), Error> {
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.id == pending.dialog_id)
        {
            Ok(())
        } else {
            Err(Error::StaleContinuation)
        }
    }

    fn active_method(&self) -> Result<&TanMethod, Error> {
        self.dialog
            .as_ref()
            .and_then(|dialog| dialog.method.as_ref())
            .ok_or(Error::InconsistentState)
    }

    fn initial_context<'a>(
        &'a self,
        method: &'a TanMethod,
        date: NaiveDate,
        time: NaiveTime,
    ) -> SecurityContext<'a> {
        SecurityContext {
            institute: &self.institute,
            credentials: &self.credentials,
            system_id: "0",
            security_function: &method.security_function,
            profile_version: "2",
            dialog_id: "0",
            message_number: 1,
            date,
            time,
        }
    }

    fn context(&self, date: NaiveDate, time: NaiveTime) -> Result<SecurityContext<'_>, Error> {
        let dialog = self.dialog.as_ref().ok_or(Error::InconsistentState)?;
        if dialog.anonymous {
            return Err(Error::InconsistentState);
        }
        Ok(SecurityContext {
            institute: &self.institute,
            credentials: &self.credentials,
            system_id: &dialog.system_id,
            security_function: &dialog.security_function,
            profile_version: dialog.profile_version,
            dialog_id: &dialog.id,
            message_number: dialog.next_message_number,
            date,
            time,
        })
    }

    fn apply_parameters(&mut self, response: &Response) -> Result<(), Error> {
        self.apply_parameters_with_mode(response, false)
    }

    fn apply_parameter_refresh(&mut self, response: &Response) -> Result<(), Error> {
        self.apply_parameters_with_mode(response, true)
    }

    fn apply_parameters_with_mode(
        &mut self,
        response: &Response,
        force_bpd_refresh: bool,
    ) -> Result<(), Error> {
        if let Some(institute) = response.bpd_institute()?
            && (institute.country_code != self.institute.country_code
                || institute.institute_code != self.institute.institute_code)
        {
            return Err(Limitation::InstituteMismatch.into());
        }
        let received_bpd_version = response.bpd_version()?;
        let accounts = if received_bpd_version == Some(0) {
            // Formals C.3.2.2 gives BPD version zero dialog-only validity.
            // Parse it into an effective clone so reusable state remains intact.
            let mut transient = self.state.clone();
            let accounts = if force_bpd_refresh {
                response.apply_parameter_refresh(&mut transient)?
            } else {
                response.apply_parameters(&mut transient)?
            };
            self.state.upd_version = transient.upd_version;
            if transient.upd_version > 0 {
                self.state.accounts = transient.accounts.clone();
            }
            self.transient_bpd = Some(transient);
            accounts
        } else if force_bpd_refresh {
            let accounts = response.apply_parameter_refresh(&mut self.state)?;
            if received_bpd_version.is_some() {
                self.transient_bpd = None;
            }
            accounts
        } else {
            let accounts = response.apply_parameters(&mut self.state)?;
            if received_bpd_version.is_some() {
                self.transient_bpd = None;
            }
            accounts
        };
        if let Some(accounts) = accounts {
            self.transient_accounts = Some(accounts);
        } else if self.state.upd_version > 0 {
            self.transient_accounts = None;
        }
        if response.has_tan_method_response() {
            self.allowed_tan_methods = response.allowed_tan_methods().to_vec();
            self.allowed_tan_methods_known = true;
        }
        Ok(())
    }

    fn transaction_format(
        &self,
        account: &crate::model::Account,
    ) -> Result<(TransactionFormat, bool), Error> {
        let camt_signatures = account.required_signatures("HKCAZ");
        let legacy_signatures = account.required_signatures("HKKAZ");
        if camt_signatures.is_none() && legacy_signatures.is_none() {
            return Err(Limitation::TransactionsNotAuthorized.into());
        }
        let parameters = self.parameters();
        let mut missing_tan_parameters = false;
        if let (Some(signatures), Some(capability)) =
            (camt_signatures, parameters.camt_capability.as_ref())
            && signatures <= 1
        {
            if let Some(requires_tan) = parameters.camt_requires_tan {
                return Ok((
                    TransactionFormat::Camt {
                        descriptor: capability.descriptor.clone(),
                    },
                    requires_tan,
                ));
            }
            missing_tan_parameters = true;
        }
        if let Some(signatures) = legacy_signatures
            && signatures <= 1
            && let Some(version) = parameters
                .legacy_transaction_versions
                .iter()
                .copied()
                .find(|version| (6..=7).contains(version))
        {
            if let Some(requires_tan) = parameters.legacy_transactions_require_tan {
                return Ok((TransactionFormat::Mt940 { version }, requires_tan));
            }
            missing_tan_parameters = true;
        }
        if camt_signatures
            .into_iter()
            .chain(legacy_signatures)
            .all(|signatures| signatures > 1)
        {
            return Err(Limitation::MultipleSigners.into());
        }
        if missing_tan_parameters {
            return Err(Limitation::PinTanParameters.into());
        }
        if parameters.transaction_capability_advertised {
            Err(Limitation::TransactionsVersion.into())
        } else {
            Err(Limitation::TransactionsNotAdvertised.into())
        }
    }

    fn accept_transaction_response(
        &mut self,
        response: Response,
        pending: Option<PendingChallenge>,
        received_at: NaiveDateTime,
    ) -> Result<TransactionsResult, Error> {
        if let Some(error) = response.first_error() {
            self.transaction = None;
            self.continuation_active = false;
            return Err(Error::Bank(error));
        }
        let format = self
            .transaction
            .as_ref()
            .ok_or(Error::InconsistentState)?
            .format
            .clone();
        let page = match response.transactions(&format) {
            Ok(page) => page,
            Err(error) => return self.fail_transaction(error),
        };
        let method = self.active_method().ok().cloned();
        let tan_response = method
            .as_ref()
            .map(|method| response.tan(method.hktan_version))
            .transpose()?
            .flatten()
            .filter(|tan| tan.challenge.reference != "noref");
        if tan_response.is_some()
            && !self
                .transaction
                .as_ref()
                .is_some_and(|transaction| transaction.requires_tan)
        {
            return self.fail_transaction(Error::InvalidResponse {
                structure: "unsolicited transaction TAN challenge",
            });
        }
        if page.is_some() && tan_response.is_some() {
            return self.fail_transaction(Error::InvalidResponse {
                structure: "transaction result and TAN challenge together",
            });
        }
        if let Some(tan_response) = tan_response {
            validate_tan_process(
                &tan_response.process,
                if pending.is_some() {
                    match method.as_ref().map(|method| method.process) {
                        Some(TanProcess::ProcessVariantTwo) => "2",
                        Some(TanProcess::Decoupled) => "S",
                        _ => return Err(Limitation::TanProcessVariant.into()),
                    }
                } else {
                    "4"
                },
            )?;
            self.continuation_active = true;
            let next = match pending {
                Some(pending) => continued_challenge(pending, tan_response.challenge)?,
                None => pending_challenge(
                    tan_response.challenge,
                    PendingOperation::Transactions,
                    method.ok_or(Error::InconsistentState)?,
                    response.dialog_id(),
                    received_at,
                )?,
            };
            return Ok(TransactionsResult::Challenge(Box::new(next)));
        }

        let continuation_point = match response.continuation_point(3) {
            Ok(point) => point.map(str::to_owned),
            Err(error) => return self.fail_transaction(error),
        };
        let no_entries = response
            .responses()
            .iter()
            .any(|response| response.code() == 3010);
        if page.is_none() && !no_entries && continuation_point.is_none() {
            return self.fail_transaction(Error::MissingValue {
                field: "booked transaction response",
            });
        }
        if let Some(page) = page {
            if let Some(account) = page.account
                && !self
                    .transaction
                    .as_ref()
                    .ok_or(Error::InconsistentState)?
                    .account
                    .same_identity(&account)
            {
                return self.fail_transaction(Error::InconsistentState);
            }
            if self
                .transaction
                .as_ref()
                .ok_or(Error::InconsistentState)?
                .entries
                .len()
                .checked_add(page.entries.len())
                .is_none_or(|count| count > LOCAL_TRANSACTION_ENTRY_LIMIT)
            {
                return self.fail_transaction(Error::PaginationLimitReached);
            }
            self.transaction
                .as_mut()
                .ok_or(Error::InconsistentState)?
                .entries
                .extend(page.entries);
        }
        self.continuation_active = false;
        if let Some(point) = continuation_point {
            if self
                .transaction
                .as_ref()
                .ok_or(Error::InconsistentState)?
                .seen_continuation_points
                .contains(&point)
            {
                return self.fail_transaction(Error::RepeatedContinuationPoint);
            }
            let transaction = self.transaction.as_mut().ok_or(Error::InconsistentState)?;
            transaction.seen_continuation_points.insert(point.clone());
            transaction.continuation_point = Some(point);
            return Ok(TransactionsResult::Continue);
        }
        let transaction = self.transaction.take().ok_or(Error::InconsistentState)?;
        Ok(TransactionsResult::Complete(Box::new(BookedTransactions {
            account: transaction.account,
            entries: transaction.entries,
        })))
    }

    fn fail_transaction<T>(&mut self, error: Error) -> Result<T, Error> {
        self.transaction = None;
        self.continuation_active = false;
        Err(error)
    }
}

fn validate_supported_method(method: &TanMethod) -> Result<(), Error> {
    if !matches!(
        method.process,
        TanProcess::ProcessVariantTwo | TanProcess::Decoupled
    ) {
        return Err(Limitation::TanProcessVariant.into());
    }
    if method.hhd_response_required {
        return Err(Limitation::PinTanParameters.into());
    }
    Ok(())
}

fn validate_reusable_state(state: &ReusableState) -> Result<(), Error> {
    if state.bpd_version > 999
        || state.upd_version > 999
        || state
            .advertised_balance_versions
            .iter()
            .any(|version| *version > 999)
        || state
            .advertised_camt_descriptors
            .iter()
            .any(|descriptor| !valid_latin1_length(descriptor, 1, 256))
        || state
            .system_id
            .as_deref()
            .is_some_and(|value| !valid_latin1_length(value, 1, 30))
        || state.selected_tan_method.as_deref().is_some_and(|value| {
            value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_digit())
        })
        || state
            .selected_tan_medium
            .as_deref()
            .is_some_and(|value| !valid_latin1_length(value, 1, 32))
        || state.tan_methods.iter().any(|method| {
            method.security_function.len() != 3
                || !method
                    .security_function
                    .bytes()
                    .all(|byte| byte.is_ascii_digit())
                || !matches!(method.security_function.parse::<u16>(), Ok(900..=997))
                || !(6..=7).contains(&method.hktan_version)
        })
    {
        Err(InputError::ReusableState.into())
    } else {
        Ok(())
    }
}

fn validate_medium(method: &TanMethod, medium: Option<&str>) -> Result<(), Error> {
    if method.medium_name_required && medium.is_none() {
        Err(Limitation::TanMedium.into())
    } else {
        Ok(())
    }
}

fn validate_tan_process(actual: &str, expected: &str) -> Result<(), Error> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::InvalidValue {
            field: "HITAN process",
        })
    }
}

fn validate_continuation_process(actual: &str, process: TanProcess) -> Result<(), Error> {
    let expected = match process {
        TanProcess::ProcessVariantOne => return Err(Limitation::TanProcessVariant.into()),
        TanProcess::ProcessVariantTwo => "2",
        TanProcess::Decoupled => "S",
    };
    validate_tan_process(actual, expected)
}

fn ensure_not_expired(challenge: &Challenge, now: NaiveDateTime) -> Result<(), Error> {
    if challenge.expires_at.is_some_and(|expiry| {
        expiry
            .time()
            .is_some_and(|time| now > expiry.date().and_time(time))
            || (expiry.time().is_none() && now.date() > expiry.date())
    }) {
        Err(Error::ChallengeExpired)
    } else {
        Ok(())
    }
}

fn pending_challenge(
    challenge: Challenge,
    operation: PendingOperation,
    method: TanMethod,
    dialog_id: &str,
    received_at: NaiveDateTime,
) -> Result<PendingChallenge, Error> {
    let next_poll_at = if method.process == TanProcess::Decoupled {
        method
            .first_poll_delay_seconds
            .map(|seconds| {
                received_at
                    .checked_add_signed(TimeDelta::seconds(i64::from(seconds)))
                    .ok_or(Error::InconsistentState)
            })
            .transpose()?
    } else {
        None
    };
    Ok(PendingChallenge {
        challenge,
        operation,
        method,
        dialog_id: dialog_id.to_owned(),
        polls: 0,
        continuations: 0,
        next_poll_at,
        assigned_system_id: None,
    })
}

fn continued_challenge(
    mut pending: PendingChallenge,
    challenge: Challenge,
) -> Result<PendingChallenge, Error> {
    pending.continuations = pending
        .continuations
        .checked_add(1)
        .ok_or(Error::ContinuationLimitReached)?;
    if pending.continuations > LOCAL_CONTINUATION_LIMIT {
        return Err(Error::ContinuationLimitReached);
    }
    pending.challenge = challenge;
    Ok(pending)
}

#[cfg(test)]
mod tests;
