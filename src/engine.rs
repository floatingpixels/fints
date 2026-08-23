use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};

#[cfg(feature = "diagnostics")]
use crate::diagnostics::{InitializationRecoveryFacts, TanMediaDiscoveryFacts};
use crate::{
    client::PollingMode,
    error::{BankResponse, Error, InputError, Limitation},
    model::{
        Challenge, Credentials, InstituteId, ProductIdentity, REUSABLE_STATE_VERSION,
        ReusableState, Tan, TanMedium, TanMethod, TanProcess, valid_latin1_length,
    },
    response::Response,
    segments::{self, SecurityContext},
};

mod cash;
mod initialization;
mod products;

pub(crate) use initialization::TanMediaInitializationResult;

const LOCAL_DECOUPLED_POLL_LIMIT: u16 = 20;
const LOCAL_CONTINUATION_LIMIT: u16 = 20;

pub(crate) struct Engine {
    institute: InstituteId,
    product: ProductIdentity,
    credentials: Credentials,
    state: ReusableState,
    transient_bpd: Option<ReusableState>,
    allowed_tan_methods: Vec<String>,
    allowed_tan_methods_known: bool,
    tan_media: Vec<TanMedium>,
    selected_tan_media_version: Option<u16>,
    transient_accounts: Option<Vec<crate::model::Account>>,
    last_responses: Vec<BankResponse>,
    tan_media_discovery_responses: Vec<BankResponse>,
    #[cfg(feature = "diagnostics")]
    development_initialization_recovery: Option<InitializationRecoveryFacts>,
    #[cfg(feature = "diagnostics")]
    development_tan_media_discovery: Option<TanMediaDiscoveryFacts>,
    #[cfg(feature = "diagnostics")]
    development_tan_media_initialization_name_supplied: Option<bool>,
    #[cfg(feature = "diagnostics")]
    development_depot_response: Option<crate::DepotResponseFacts>,
    requested_balance: Option<crate::model::Account>,
    transaction: Option<cash::TransactionState>,
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

pub(crate) use cash::{BalanceResult, TransactionsResult};
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
        if state.version() != REUSABLE_STATE_VERSION {
            return Err(Error::ReusableStateVersion {
                expected: REUSABLE_STATE_VERSION,
                found: state.version(),
            });
        }
        state
            .depot_position_versions
            .sort_unstable_by(|a, b| b.cmp(a));
        state.depot_position_versions.dedup();
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
            selected_tan_media_version: None,
            transient_accounts: None,
            last_responses: Vec::new(),
            tan_media_discovery_responses: Vec::new(),
            #[cfg(feature = "diagnostics")]
            development_initialization_recovery: None,
            #[cfg(feature = "diagnostics")]
            development_tan_media_discovery: None,
            #[cfg(feature = "diagnostics")]
            development_tan_media_initialization_name_supplied: None,
            #[cfg(feature = "diagnostics")]
            development_depot_response: None,
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

    pub(crate) fn advertised_tan_media_versions(&self) -> Vec<u16> {
        self.parameters().advertised_tan_media_versions()
    }

    pub(crate) fn selected_tan_media_version(&self) -> Option<u16> {
        self.selected_tan_media_version
    }

    pub(crate) fn last_responses(&self) -> &[BankResponse] {
        &self.last_responses
    }

    pub(crate) fn last_tan_media_discovery_responses(&self) -> &[BankResponse] {
        &self.tan_media_discovery_responses
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn development_initialization_recovery(
        &self,
    ) -> Option<InitializationRecoveryFacts> {
        self.development_initialization_recovery
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn development_tan_media_discovery(&self) -> Option<&TanMediaDiscoveryFacts> {
        self.development_tan_media_discovery.as_ref()
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn development_depot_response(&self) -> Option<&crate::DepotResponseFacts> {
        self.development_depot_response.as_ref()
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
        let discovery_parameters_pending = self.state.bpd_version == 0
            && self.state.upd_version == 0
            && self.state.selected_tan_method.is_none()
            && self.parameters().tan_methods.is_empty()
            && self.allowed_tan_methods_known
            && !self.allowed_tan_methods.is_empty();
        self.apply_parameter_refresh(&response)?;
        if discovery_parameters_pending
            && self.state.bpd_version == 0
            && self.parameters().tan_methods.is_empty()
            && response.is_exact_anonymous_bpd_method_failure()
        {
            // PIN/TAN B.4.3.1 requires anonymous BPD to expose the method
            // descriptions needed to interpret 3920. T8 supplies no alternate
            // acquisition when that mandatory source itself is terminated.
            self.abort_dialog();
            return Err(Limitation::TanMethodParametersUnavailable.into());
        }
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
        self.finish_termination(response)
    }

    pub(crate) fn accept_discovery_refresh_termination(
        &mut self,
        input: &[u8],
    ) -> Result<(), Error> {
        let response = self.accept_dialog_response(input)?;
        if response.is_exact_global_discovery_termination() {
            // PIN/TAN B.4.3.1 requires the open function-999 discovery dialog
            // to be closed with HKEND. Formals C.1.2 says 9800 proves that the
            // institute has ended it. Preserve only the already-decided T8
            // refresh outcome; normal termination errors remain unchanged.
            self.abort_dialog();
            return Ok(());
        }
        self.finish_termination(response)
    }

    fn finish_termination(&mut self, response: Response) -> Result<(), Error> {
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
            .depot_position_versions
            .iter()
            .any(|version| !(5..=6).contains(version))
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

/// PIN/TAN 2020 B.4.2.1, process variant 2 step 2: the institution answers the
/// TAN submission with HITAN carrying process `2` and the reference of the
/// order being approved. Such a segment acknowledges that order and may
/// accompany its result; it never opens a further challenge.
pub(super) fn acknowledges_variant_two(
    tan: &crate::response::tan::TanResponse,
    pending: Option<&PendingChallenge>,
) -> bool {
    pending.is_some_and(|pending| {
        pending.method.process == TanProcess::ProcessVariantTwo
            && tan.process == "2"
            && tan.challenge.reference == pending.challenge.reference
    })
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
