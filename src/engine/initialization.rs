use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::{
    error::{Error, Limitation},
    model::TanMedium,
    response::Response,
    segments::{self, SecurityContext},
};

use super::{
    DialogState, Engine, InitializationResult, PendingOperation, pending_challenge,
    validate_medium, validate_supported_method, validate_tan_process,
};

pub(crate) enum TanMediaInitializationResult {
    Complete,
    OrderRequired,
}

impl Engine {
    pub(crate) fn initialization_request(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_no_dialog()?;
        let method = self.selected_method().ok().cloned();
        if let Some(method) = &method {
            validate_supported_method(method)?;
            validate_medium(method, self.state.selected_tan_medium.as_deref())?;
        }
        let system_id = self.state.system_id.as_deref().unwrap_or("0");
        let context = SecurityContext {
            institute: &self.institute,
            credentials: &self.credentials,
            system_id,
            security_function: method
                .as_ref()
                .map_or("999", |method| method.security_function.as_str()),
            profile_version: if method.is_some() { "2" } else { "1" },
            dialog_id: "0",
            message_number: 1,
            date,
            time,
        };
        if method.is_none() && self.state.system_id.is_none() {
            // FinTS 3.0 Formals C.8/C.8.1 requires a new PIN/TAN customer
            // system to obtain its system ID in a synchronization dialog.
            // PIN/TAN correction T2 requires that HKSYN response to return a
            // user-valid TAN method, so function 999 can discover the method
            // and system ID in the same first-contact exchange.
            return segments::synchronization(&context, &self.product, &self.state, None, None);
        }
        segments::initialization(
            &context,
            &self.product,
            &self.state,
            method.as_ref(),
            "HKIDN",
            self.state.selected_tan_medium.as_deref(),
        )
    }

    #[cfg(feature = "tan-medium-selector-experiment")]
    pub(crate) fn tan_medium_selector_experiment_request(
        &mut self,
        variant: crate::client::TanMediumSelectorExperimentVariant,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_no_dialog()?;
        if self.continuation_active {
            return Err(Error::InconsistentState);
        }
        let method = self.selected_method()?.clone();
        validate_supported_method(&method)?;
        if !method.medium_name_required
            || self.state.selected_tan_medium.is_some()
            || self.tan_media.is_empty()
            || self.tan_media.iter().any(|medium| medium.name().is_some())
        {
            return Err(Error::InconsistentState);
        }
        let context = SecurityContext {
            institute: &self.institute,
            credentials: &self.credentials,
            system_id: self.state.system_id.as_deref().unwrap_or("0"),
            security_function: &method.security_function,
            profile_version: "2",
            dialog_id: "0",
            message_number: 1,
            date,
            time,
        };
        let medium_name = match variant {
            crate::client::TanMediumSelectorExperimentVariant::Omitted => None,
            crate::client::TanMediumSelectorExperimentVariant::Empty => Some(""),
        };
        segments::initialization(
            &context,
            &self.product,
            &self.state,
            Some(&method),
            "HKIDN",
            medium_name,
        )
    }

    #[cfg(feature = "tan-medium-selector-experiment")]
    pub(crate) fn accept_tan_medium_selector_experiment_initialization(
        &mut self,
        input: &[u8],
    ) -> Result<bool, Error> {
        let method = self.selected_method()?.clone();
        let response = Response::parse(input)?;
        self.record_responses(&response);
        // Formals C.3.2.2 makes a complete BPD delivered during initialization
        // immediately active. Apply it before classifying an experimental HKTAN
        // rejection so the next discovery never reasons from superseded HITANS.
        self.apply_parameters(&response)?;
        let hktan_only_rejection = response.responses().iter().any(|response| {
            response.class() == crate::ResponseClass::Error && response.segment_number() == Some(5)
        }) && response.responses().iter().all(|response| {
            response.class() != crate::ResponseClass::Error || response.segment_number() == Some(5)
        }) && response.responses().iter().any(|response| {
            response.class() == crate::ResponseClass::Success && response.segment_number().is_none()
        });
        if response.is_dialog_abort() || (response.first_error().is_some() && !hktan_only_rejection)
        {
            self.abort_dialog();
            return Ok(false);
        }
        // Formals B.7.5.2 rejects only the referenced syntactic unit. A class-9
        // HIRMS for the experimental HKTAN therefore leaves an otherwise
        // successful initialization dialog open; only 9800 ends it bank-side.
        let next_message_number = response
            .message_number()
            .checked_add(1)
            .ok_or(Error::InconsistentState)?;
        self.dialog = Some(DialogState {
            id: response.dialog_id().to_owned(),
            next_message_number,
            security_function: method.security_function.clone(),
            profile_version: "2",
            system_id: self
                .state
                .system_id
                .clone()
                .unwrap_or_else(|| "0".to_owned()),
            method: Some(method),
            anonymous: false,
        });
        self.continuation_active = false;
        Ok(true)
    }

    pub(crate) fn tan_media_initialization_request(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        self.tan_media_initialization_request_for_version(date, time, None)
    }

    #[cfg(feature = "tan-medium-selector-experiment")]
    pub(crate) fn tan_media_initialization_request_version_two(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        let method = self.selected_method()?;
        if !method.medium_name_required
            || self.tan_media.is_empty()
            || self.tan_media.iter().any(|medium| medium.name().is_some())
        {
            return Err(Error::InconsistentState);
        }
        self.tan_media_initialization_request_for_version(date, time, Some(2))
    }

    fn tan_media_initialization_request_for_version(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
        forced_version: Option<u16>,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_no_dialog()?;
        self.tan_media.clear();
        self.selected_tan_media_version = None;
        self.tan_media_discovery_responses.clear();
        #[cfg(feature = "development-diagnostics")]
        {
            self.development_tan_media_discovery = None;
            self.development_tan_media_initialization_name_supplied = None;
        }
        let method = self.selected_method()?.clone();
        validate_supported_method(&method)?;
        let version = self.tan_media_version(forced_version)?;
        self.selected_tan_media_version = Some(version);
        let system_id = self.state.system_id.as_deref().unwrap_or("0");
        let context = SecurityContext {
            institute: &self.institute,
            credentials: &self.credentials,
            system_id,
            security_function: &method.security_function,
            profile_version: "2",
            dialog_id: "0",
            message_number: 1,
            date,
            time,
        };
        let medium_name = self.state.selected_tan_medium.as_deref().or(Some("noref"));
        let request = segments::initialization(
            &context,
            &self.product,
            &self.state,
            Some(&method),
            "HKTAB",
            medium_name,
        )?;
        #[cfg(feature = "development-diagnostics")]
        {
            self.development_tan_media_initialization_name_supplied = Some(medium_name.is_some());
        }
        Ok(request)
    }

    pub(crate) fn accept_tan_media_initialization(
        &mut self,
        input: &[u8],
    ) -> Result<TanMediaInitializationResult, Error> {
        self.accept_tan_media_initialization_for_version(input, None)
    }

    #[cfg(feature = "tan-medium-selector-experiment")]
    pub(crate) fn accept_tan_media_initialization_version_two(
        &mut self,
        input: &[u8],
    ) -> Result<TanMediaInitializationResult, Error> {
        self.accept_tan_media_initialization_for_version(input, Some(2))
    }

    fn accept_tan_media_initialization_for_version(
        &mut self,
        input: &[u8],
        forced_version: Option<u16>,
    ) -> Result<TanMediaInitializationResult, Error> {
        let method = self.selected_method()?.clone();
        let response = Response::parse(input)?;
        self.record_responses(&response);
        self.tan_media_discovery_responses
            .extend_from_slice(response.responses());
        self.apply_parameters(&response)?;
        if let Some(error) = response.first_error() {
            return Err(Error::Bank(error));
        }
        let next_message_number = response
            .message_number()
            .checked_add(1)
            .ok_or(Error::InconsistentState)?;
        self.dialog = Some(DialogState {
            id: response.dialog_id().to_owned(),
            next_message_number,
            security_function: method.security_function.clone(),
            profile_version: "2",
            system_id: self
                .state
                .system_id
                .clone()
                .unwrap_or_else(|| "0".to_owned()),
            method: Some(method.clone()),
            anonymous: false,
        });
        // The initialization response may replace BPD. Negotiate again before
        // parsing an inline HITAB or emitting the separate HKTAB order. The
        // accepted dialog is already recorded so a local version limitation
        // still closes it exactly once.
        self.selected_tan_media_version = None;
        let media_version = self.tan_media_version(forced_version)?;
        self.selected_tan_media_version = Some(media_version);
        #[cfg(feature = "development-diagnostics")]
        {
            let hitans_requirement = self
                .selected_method()
                .ok()
                .and_then(|method| method.development_medium_requirement);
            self.development_tan_media_discovery =
                Some(crate::development_diagnostics::TanMediaDiscoveryFacts::new(
                    response.development_segment_facts(),
                    response.development_response_facts(),
                    self.advertised_tan_media_versions(),
                    media_version,
                    hitans_requirement,
                    self.development_tan_media_initialization_name_supplied,
                ));
        }
        if let Some(tan_response) = response.tan(method.hktan_version)? {
            // PIN/TAN B.5.1/B.5.2 fixes the institute response to the same
            // process 4 used by the embedded HKTAN. A dummy noref HITAN only
            // records that no SCA is needed; it never substitutes for the
            // mandatory HITAB result from B.4.3.1.3/C.3.1.1.
            validate_tan_process(&tan_response.process, "4")?;
            if tan_response.challenge.reference != "noref" {
                return Err(Limitation::TanMedium.into());
            }
        }
        let tan_media = response.tan_media(media_version, None);
        #[cfg(feature = "development-diagnostics")]
        {
            let facts = self
                .development_tan_media_discovery
                .as_mut()
                .expect("TAN-media diagnostics were initialized");
            facts.set_tan_usage_option(response.development_tan_usage_option(media_version, None));
            if let Ok(Some(media)) = &tan_media {
                facts.set_discovered_media(media);
                facts.set_returned_medium_shapes(
                    response.development_tan_medium_shapes(media_version, None),
                );
            }
        }
        let Some(tan_media) = tan_media? else {
            return Ok(TanMediaInitializationResult::OrderRequired);
        };
        // The initialization response may have replaced HITANS together with
        // the BPD. Apply the resulting HKTAN DE 12 condition to the discovered
        // list, not the stale method snapshot that secured the open dialog.
        let current_method = self.selected_method()?.clone();
        self.store_tan_media(tan_media, &current_method)?;
        Ok(TanMediaInitializationResult::Complete)
    }

    pub(crate) fn tan_media_request(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        if self.continuation_active {
            return Err(Error::InconsistentState);
        }
        let version = self
            .selected_tan_media_version
            .ok_or(Error::InconsistentState)?;
        let request = {
            let context = self.context(date, time)?;
            segments::tan_media_request(&context, version)?
        };
        #[cfg(feature = "development-diagnostics")]
        if let Some(facts) = self.development_tan_media_discovery.as_mut() {
            facts.set_hktab_request(crate::development_diagnostics::HktabRequestFact::all_media(
                version,
            ));
        }
        Ok(request)
    }

    pub(crate) fn accept_tan_media_response(&mut self, input: &[u8]) -> Result<(), Error> {
        let version = self
            .selected_tan_media_version
            .ok_or(Error::InconsistentState)?;
        let response = match self.accept_dialog_response(input) {
            Ok(response) => response,
            Err(error @ Error::Bank(_)) => {
                self.tan_media_discovery_responses
                    .extend_from_slice(&self.last_responses);
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        self.tan_media_discovery_responses
            .extend_from_slice(response.responses());
        #[cfg(feature = "development-diagnostics")]
        {
            let facts = self
                .development_tan_media_discovery
                .as_mut()
                .ok_or(Error::InconsistentState)?;
            facts.extend(
                response.development_segment_facts(),
                response.development_response_facts(),
            );
        }
        if let Some(error) = response.first_error() {
            return Err(Error::Bank(error));
        }
        let tan_media = response
            .tan_media(version, Some(3))?
            .ok_or(Error::MissingValue {
                field: "HITAB TAN media response",
            })?;
        #[cfg(feature = "development-diagnostics")]
        {
            let facts = self
                .development_tan_media_discovery
                .as_mut()
                .expect("TAN-media diagnostics were initialized");
            facts.set_tan_usage_option(response.development_tan_usage_option(version, Some(3)));
            facts.set_discovered_media(&tan_media);
            facts.set_returned_medium_shapes(
                response.development_tan_medium_shapes(version, Some(3)),
            );
        }
        let current_method = self.selected_method()?.clone();
        self.store_tan_media(tan_media, &current_method)
    }

    fn store_tan_media(
        &mut self,
        tan_media: Vec<TanMedium>,
        method: &crate::model::TanMethod,
    ) -> Result<(), Error> {
        self.tan_media = tan_media;
        if method.medium_name_required
            && !self.tan_media.iter().any(|medium| medium.name().is_some())
        {
            return Err(Limitation::TanMediumUnavailable.into());
        }
        Ok(())
    }

    fn negotiated_tan_media_version(&self) -> Result<u16, Error> {
        self.advertised_tan_media_versions()
            .into_iter()
            .find(|version| matches!(version, 2 | 4 | 5))
            .ok_or_else(|| Limitation::TanMediumVersion.into())
    }

    fn tan_media_version(&self, forced_version: Option<u16>) -> Result<u16, Error> {
        match forced_version {
            Some(version)
                if matches!(version, 2 | 4 | 5)
                    && self.advertised_tan_media_versions().contains(&version) =>
            {
                Ok(version)
            }
            Some(_) => Err(Limitation::TanMediumVersion.into()),
            None => self.negotiated_tan_media_version(),
        }
    }

    pub(crate) fn accept_initialization(
        &mut self,
        input: &[u8],
        received_at: NaiveDateTime,
    ) -> Result<InitializationResult, Error> {
        #[cfg(feature = "development-diagnostics")]
        {
            self.development_initialization_recovery = None;
        }
        let requested_method = self.selected_method().ok().cloned();
        let initial_system_synchronization =
            requested_method.is_none() && self.state.system_id.is_none();
        let response = Response::parse(input)?;
        self.record_responses(&response);
        self.apply_parameters(&response)?;
        let usable_selected_method_present = requested_method.is_some();
        let bpd_zero = self.state.bpd_version == 0;
        let upd_zero = self.state.upd_version == 0;
        let tan_parameters_empty = self.parameters().tan_methods.is_empty();
        #[cfg(feature = "development-diagnostics")]
        let has_tan_method_response = response.has_tan_method_response();
        let global_abort_shape = response.is_global_bpdless_tan_method_discovery_abort();
        let refresh_before_rediscovery = !usable_selected_method_present
            && bpd_zero
            && upd_zero
            && tan_parameters_empty
            && global_abort_shape;
        #[cfg(feature = "development-diagnostics")]
        {
            self.development_initialization_recovery = Some(
                crate::development_diagnostics::InitializationRecoveryFacts::new(
                    usable_selected_method_present,
                    bpd_zero,
                    upd_zero,
                    tan_parameters_empty,
                    has_tan_method_response,
                    global_abort_shape,
                    refresh_before_rediscovery,
                ),
            );
        }
        let bank_terminated_discovery =
            requested_method.is_none() && response.is_bank_terminated_tan_method_discovery();
        let unclassified_terminated_discovery = requested_method.is_none()
            && response.is_unclassified_bank_terminated_tan_method_discovery();
        if refresh_before_rediscovery {
            // B.4.3.1 makes anonymous BPD a prerequisite for function-999
            // discovery. Repair that prerequisite once at the Client layer;
            // never infer a method from HITANS without a subsequent 3920.
            self.dialog = None;
            self.continuation_active = false;
            self.allowed_tan_methods.clear();
            self.allowed_tan_methods_known = false;
            return Ok(InitializationResult::RefreshAndRediscover);
        }
        if bank_terminated_discovery || unclassified_terminated_discovery {
            if !response.has_tan_method_response() {
                return Err(Error::MissingValue {
                    field: "3920 TAN method response",
                });
            }
            validate_discovered_methods(&response)?;
            if initial_system_synchronization {
                self.state.system_id = Some(response.system_id()?.ok_or(Error::MissingValue {
                    field: "assigned system ID",
                })?);
            }
            // PIN/TAN 2020 B.6.1 and B.8.2: 9800/9955 terminates the
            // function-999 discovery dialog at the institute. T8 requires an
            // anonymous BPD refresh when 3920 cannot be matched to a described
            // method. A bank-terminated dialog is never followed by HKEND.
            self.dialog = None;
            self.continuation_active = false;
            if !self.has_usable_allowed_tan_method() {
                return Ok(InitializationResult::RefreshParameters);
            }
            if bank_terminated_discovery {
                return Ok(InitializationResult::ChooseTanMethod);
            }
        }
        if let Some(error) = response.first_error() {
            return Err(Error::Bank(error));
        }
        if initial_system_synchronization {
            self.state.system_id = Some(response.system_id()?.ok_or(Error::MissingValue {
                field: "assigned system ID",
            })?);
        }
        let next_message_number = response
            .message_number()
            .checked_add(1)
            .ok_or(Error::InconsistentState)?;
        self.dialog = Some(DialogState {
            id: response.dialog_id().to_owned(),
            next_message_number,
            security_function: requested_method.as_ref().map_or_else(
                || "999".to_owned(),
                |method| method.security_function.clone(),
            ),
            profile_version: if requested_method.is_some() { "2" } else { "1" },
            system_id: self
                .state
                .system_id
                .clone()
                .unwrap_or_else(|| "0".to_owned()),
            method: requested_method.clone(),
            anonymous: false,
        });
        if requested_method.is_none() {
            if !response.has_tan_method_response() {
                return Err(Error::MissingValue {
                    field: "3920 TAN method response",
                });
            }
            validate_discovered_methods(&response)?;
            if !self.has_usable_allowed_tan_method() {
                return Ok(InitializationResult::RefreshParameters);
            }
        }
        let selected = self.state.selected_tan_method.as_deref();
        if selected.is_none()
            || !self
                .parameters()
                .tan_methods
                .iter()
                .any(|method| Some(method.security_function.as_str()) == selected)
            || (self.allowed_tan_methods_known
                && !self
                    .allowed_tan_methods
                    .iter()
                    .any(|method| Some(method.as_str()) == selected))
        {
            return Ok(InitializationResult::ChooseTanMethod);
        }

        let method = self.selected_method()?.clone();
        if let Some(tan_response) = response.tan(method.hktan_version)?
            && tan_response.challenge.reference != "noref"
        {
            validate_tan_process(&tan_response.process, "4")?;
            self.continuation_active = true;
            return Ok(InitializationResult::Challenge(Box::new(
                pending_challenge(
                    tan_response.challenge,
                    PendingOperation::Initialization,
                    method,
                    response.dialog_id(),
                    received_at,
                )?,
            )));
        }
        self.continuation_active = false;
        Ok(InitializationResult::Connected)
    }

    fn has_usable_allowed_tan_method(&self) -> bool {
        self.parameters().tan_methods.iter().any(|method| {
            self.allowed_tan_methods
                .iter()
                .any(|allowed| allowed == &method.security_function)
        })
    }
}

fn validate_discovered_methods(response: &Response) -> Result<(), Error> {
    if response.allowed_tan_methods().is_empty() {
        return Err(Error::MissingValue {
            field: "valid 3920 TAN method parameter",
        });
    }
    if response
        .allowed_tan_methods()
        .iter()
        .all(|method| method == "999")
    {
        return Err(Limitation::TanMethod.into());
    }
    Ok(())
}
