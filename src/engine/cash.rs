//! Cash-account reads: HKSAL balances and HKCAZ/HKKAZ booked transactions.
//!
//! Moved verbatim from the engine root so every operation family lives in a
//! family module; dialog state stays on [`Engine`].

use std::collections::HashSet;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::{
    error::{Error, InputError, Limitation},
    model::{Balance, BookedEntry, BookedTransactions, TanProcess, TransactionFormat},
    response::Response,
    segments,
};

use super::{
    Engine, PendingChallenge, PendingOperation, continued_challenge, pending_challenge,
    validate_continuation_process, validate_tan_process,
};

pub(super) const LOCAL_TRANSACTION_PAGE_LIMIT: u16 = 100;
const LOCAL_TRANSACTION_ENTRY_LIMIT: usize = 10_000;

pub(super) struct TransactionState {
    pub(super) account: crate::model::Account,
    pub(super) format: TransactionFormat,
    pub(super) from: Option<NaiveDate>,
    pub(super) to: Option<NaiveDate>,
    pub(super) requires_tan: bool,
    pub(super) continuation_point: Option<String>,
    pub(super) seen_continuation_points: HashSet<String>,
    pub(super) pages_requested: u16,
    pub(super) entries: Vec<BookedEntry>,
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

impl Engine {
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

    pub(super) fn transaction_format(
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
            (camt_signatures, parameters.camt_capability())
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
            .filter(|tan| tan.challenge.reference != "noref")
            // PIN/TAN 2020 B.4.2.1 step 2: the response to the submitted TAN
            // echoes HITAN with process 2 and the order reference. That is the
            // acknowledgment of this order, not a further challenge.
            .filter(|tan| !super::acknowledges_variant_two(tan, pending.as_ref()));
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
