use std::collections::HashSet;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::{
    error::{Error, InputError, Limitation},
    model::{
        Account, CreditCardBalance, CreditCardCapability, CreditCardTransactions, DepotPosition,
        DepotPositionParseCounts, DepotPositions, SecuritiesTransaction, SecuritiesTransactions,
        TanProcess,
    },
    response::Response,
    segments,
};

use super::{
    Engine, PendingChallenge, PendingOperation, continued_challenge, pending_challenge,
    validate_tan_process,
};

const PAGE_LIMIT: u16 = 100;
const ENTRY_LIMIT: usize = 10_000;

#[derive(Default)]
pub(super) struct ProductStates {
    positions: Option<PositionState>,
    securities: Option<SecuritiesState>,
    card_transactions: Option<CardTransactionState>,
    card_balance: Option<Account>,
}

impl ProductStates {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    fn active(&self) -> bool {
        self.positions.is_some()
            || self.securities.is_some()
            || self.card_transactions.is_some()
            || self.card_balance.is_some()
    }
}

struct PositionState {
    account: Account,
    version: u16,
    requires_tan: bool,
    continuation_point: Option<String>,
    seen: HashSet<String>,
    pages: u16,
    positions: Vec<DepotPosition>,
    total_values: Vec<crate::model::SecuritiesAmount>,
    degraded_positions: usize,
    skipped_positions: usize,
    malformed_page_totals: usize,
}

struct SecuritiesState {
    account: Account,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    requires_tan: bool,
    continuation_point: Option<String>,
    seen: HashSet<String>,
    pages: u16,
    entries: Vec<SecuritiesTransaction>,
}

struct CardTransactionState {
    account: Account,
    capability: CreditCardCapability,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    requires_tan: bool,
    continuation_point: Option<String>,
    seen: HashSet<String>,
    pages: u16,
    reported_card_number: Option<String>,
    reported_account_id: Option<String>,
    current_balance: Option<crate::model::CreditCardCurrentBalance>,
    last_statement_date: Option<NaiveDate>,
    next_statement_date: Option<NaiveDate>,
    entries: Vec<crate::model::CreditCardEntry>,
}

pub(crate) enum DepotPositionsResult {
    Complete(Box<DepotPositions>),
    Continue,
    Challenge(Box<PendingChallenge>),
}

pub(crate) enum SecuritiesTransactionsResult {
    Complete(Box<SecuritiesTransactions>),
    Continue,
    Challenge(Box<PendingChallenge>),
}

pub(crate) enum CreditCardTransactionsResult {
    Complete(Box<CreditCardTransactions>),
    Continue,
    Challenge(Box<PendingChallenge>),
}

pub(crate) enum CreditCardBalanceResult {
    Complete(Box<CreditCardBalance>),
    Challenge(Box<PendingChallenge>),
}

impl Engine {
    pub(crate) fn depot_positions_request(
        &mut self,
        account_index: usize,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_product_idle()?;
        #[cfg(feature = "diagnostics")]
        {
            self.development_depot_response = None;
        }
        let account = self.product_account(
            account_index,
            "HKWPD",
            Limitation::DepotPositionsNotAuthorized,
        )?;
        let parameters = self.parameters();
        if parameters.depot_position_versions.is_empty() {
            return Err(if parameters.depot_positions_advertised {
                Limitation::DepotPositionsVersion
            } else {
                Limitation::DepotPositionsNotAdvertised
            }
            .into());
        }
        let version = parameters
            .depot_position_versions
            .first()
            .copied()
            .ok_or(Limitation::DepotPositionsVersion)?;
        let requires_tan = required_tan(
            parameters.depot_positions_requires_tan,
            Limitation::PinTanParameters,
        )?;
        let tan = self.product_tan(requires_tan)?;
        let message = {
            let context = self.context(date, time)?;
            segments::depot_positions_request(
                &context,
                &account,
                version,
                None,
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        self.products.positions = Some(PositionState {
            account,
            version,
            requires_tan,
            continuation_point: None,
            seen: HashSet::new(),
            pages: 1,
            positions: Vec::new(),
            total_values: Vec::new(),
            degraded_positions: 0,
            skipped_positions: 0,
            malformed_page_totals: 0,
        });
        Ok(message)
    }

    pub(crate) fn accept_depot_positions(
        &mut self,
        input: &[u8],
        received_at: NaiveDateTime,
    ) -> Result<DepotPositionsResult, Error> {
        let response = self.accept_dialog_response(input)?;
        self.accept_depot_position_response(response, None, received_at)
    }

    pub(crate) fn next_depot_positions_page_request(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        if self.continuation_active {
            return Err(Error::InconsistentState);
        }
        let state = self
            .products
            .positions
            .as_ref()
            .ok_or(Error::InconsistentState)?;
        if state.pages >= PAGE_LIMIT {
            return self.fail_positions(Error::PaginationLimitReached);
        }
        let continuation_point = state
            .continuation_point
            .as_deref()
            .ok_or(Error::InconsistentState)?;
        let tan = self.product_tan(state.requires_tan)?;
        let message = {
            let context = self.context(date, time)?;
            segments::depot_positions_request(
                &context,
                &state.account,
                state.version,
                Some(continuation_point),
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        self.products
            .positions
            .as_mut()
            .ok_or(Error::InconsistentState)?
            .pages += 1;
        Ok(message)
    }

    pub(crate) fn accept_depot_positions_continuation(
        &mut self,
        input: &[u8],
        pending: PendingChallenge,
        received_at: NaiveDateTime,
    ) -> Result<DepotPositionsResult, Error> {
        self.ensure_pending_dialog(&pending)?;
        if pending.operation != PendingOperation::DepotPositions {
            return Err(Error::InconsistentState);
        }
        let response = self.accept_dialog_response(input)?;
        self.accept_depot_position_response(response, Some(pending), received_at)
    }

    fn accept_depot_position_response(
        &mut self,
        response: Response,
        pending: Option<PendingChallenge>,
        received_at: NaiveDateTime,
    ) -> Result<DepotPositionsResult, Error> {
        if let Some(error) = response.first_error() {
            return self.fail_positions(Error::Bank(error));
        }
        let challenge = match self.product_response_challenge(
            &response,
            pending,
            PendingOperation::DepotPositions,
            self.products
                .positions
                .as_ref()
                .is_some_and(|state| state.requires_tan),
            received_at,
        ) {
            Ok(challenge) => challenge,
            Err(error) => return self.fail_positions(error),
        };
        if let Some(challenge) = challenge {
            return Ok(DepotPositionsResult::Challenge(Box::new(challenge)));
        }
        let version = self
            .products
            .positions
            .as_ref()
            .ok_or(Error::InconsistentState)?
            .version;
        #[cfg(feature = "diagnostics")]
        {
            // Parse and retain the value-free block tree before any position
            // field can fail. A later typed parse error must not erase the
            // structural evidence needed for an owner-attended diagnosis.
            self.development_depot_response = response
                .development_depot_position_structure(version)
                .ok()
                .flatten();
        }
        let page = match response.depot_positions(version) {
            Ok(Some(page)) => page,
            Ok(None)
                if response
                    .responses()
                    .iter()
                    .any(|response| response.code() == 3010) =>
            {
                match response.continuation_point(3) {
                    Ok(None) => {}
                    Ok(Some(_)) => {
                        return self.fail_positions(Error::InvalidResponse {
                            structure: "empty depot-position result with continuation",
                        });
                    }
                    Err(error) => return self.fail_positions(error),
                }
                return self.finish_empty_positions();
            }
            Ok(None) => {
                return self.fail_positions(Error::MissingValue {
                    field: "depot-position response",
                });
            }
            Err(error) => return self.fail_positions(error),
        };
        #[cfg(feature = "diagnostics")]
        {
            self.development_depot_response = Some(page.development_facts.clone());
        }
        let point = match response.continuation_point(3) {
            Ok(point) => point.map(str::to_owned),
            Err(error) => return self.fail_positions(error),
        };
        if page.more != point.is_some() {
            return self.fail_positions(Error::InvalidResponse {
                structure: "MT535 page indicator and FinTS continuation",
            });
        }
        if let Err(error) =
            self.verify_securities_identity(&page.institute_code, &page.account_number, true)
        {
            return self.fail_positions(error);
        }
        let state = self
            .products
            .positions
            .as_mut()
            .ok_or(Error::InconsistentState)?;
        let reported_count = state
            .positions
            .len()
            .checked_add(state.skipped_positions)
            .and_then(|count| count.checked_add(page.positions.len()))
            .and_then(|count| count.checked_add(page.parse_counts.skipped()));
        if reported_count.is_none_or(|count| count > ENTRY_LIMIT) {
            return self.fail_positions(Error::PaginationLimitReached);
        }
        state.degraded_positions += page.parse_counts.degraded();
        state.skipped_positions += page.parse_counts.skipped();
        state.malformed_page_totals += page.parse_counts.malformed_page_totals();
        state.positions.extend(page.positions);
        state.total_values.extend(page.total_values);
        self.continuation_active = false;
        if let Some(point) = point {
            if !state.seen.insert(point.clone()) {
                return self.fail_positions(Error::RepeatedContinuationPoint);
            }
            state.continuation_point = Some(point);
            return Ok(DepotPositionsResult::Continue);
        }
        let state = self
            .products
            .positions
            .take()
            .ok_or(Error::InconsistentState)?;
        Ok(DepotPositionsResult::Complete(Box::new(DepotPositions {
            account: state.account,
            positions: state.positions,
            total_values: state.total_values,
            parse_counts: DepotPositionParseCounts::new(
                state.degraded_positions,
                state.skipped_positions,
                state.malformed_page_totals,
            ),
        })))
    }

    fn finish_empty_positions(&mut self) -> Result<DepotPositionsResult, Error> {
        let state = self
            .products
            .positions
            .take()
            .ok_or(Error::InconsistentState)?;
        self.continuation_active = false;
        Ok(DepotPositionsResult::Complete(Box::new(DepotPositions {
            account: state.account,
            positions: state.positions,
            total_values: state.total_values,
            parse_counts: DepotPositionParseCounts::new(
                state.degraded_positions,
                state.skipped_positions,
                state.malformed_page_totals,
            ),
        })))
    }

    pub(crate) fn securities_transactions_request(
        &mut self,
        account_index: usize,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_product_idle()?;
        #[cfg(feature = "diagnostics")]
        {
            self.development_depot_response = None;
        }
        validate_range(from, to)?;
        let account = self.product_account(
            account_index,
            "HKWDU",
            Limitation::SecuritiesTransactionsNotAuthorized,
        )?;
        let parameters = self.parameters();
        if !parameters.securities_transactions_supported {
            return Err(if parameters.securities_transactions_advertised {
                Limitation::SecuritiesTransactionsVersion
            } else {
                Limitation::SecuritiesTransactionsNotAdvertised
            }
            .into());
        }
        let requires_tan = required_tan(
            parameters.securities_transactions_requires_tan,
            Limitation::PinTanParameters,
        )?;
        let tan = self.product_tan(requires_tan)?;
        let message = {
            let context = self.context(date, time)?;
            segments::securities_transactions_request(
                &context,
                &account,
                from,
                to,
                None,
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        self.products.securities = Some(SecuritiesState {
            account,
            from,
            to,
            requires_tan,
            continuation_point: None,
            seen: HashSet::new(),
            pages: 1,
            entries: Vec::new(),
        });
        Ok(message)
    }

    pub(crate) fn accept_securities_transactions(
        &mut self,
        input: &[u8],
        received_at: NaiveDateTime,
    ) -> Result<SecuritiesTransactionsResult, Error> {
        let response = self.accept_dialog_response(input)?;
        self.accept_securities_response(response, None, received_at)
    }

    pub(crate) fn next_securities_transactions_page_request(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        if self.continuation_active {
            return Err(Error::InconsistentState);
        }
        let state = self
            .products
            .securities
            .as_ref()
            .ok_or(Error::InconsistentState)?;
        if state.pages >= PAGE_LIMIT {
            return self.fail_securities(Error::PaginationLimitReached);
        }
        let continuation_point = state
            .continuation_point
            .as_deref()
            .ok_or(Error::InconsistentState)?;
        let tan = self.product_tan(state.requires_tan)?;
        let message = {
            let context = self.context(date, time)?;
            segments::securities_transactions_request(
                &context,
                &state.account,
                state.from,
                state.to,
                Some(continuation_point),
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        self.products
            .securities
            .as_mut()
            .ok_or(Error::InconsistentState)?
            .pages += 1;
        Ok(message)
    }

    pub(crate) fn accept_securities_transactions_continuation(
        &mut self,
        input: &[u8],
        pending: PendingChallenge,
        received_at: NaiveDateTime,
    ) -> Result<SecuritiesTransactionsResult, Error> {
        self.ensure_pending_dialog(&pending)?;
        if pending.operation != PendingOperation::SecuritiesTransactions {
            return Err(Error::InconsistentState);
        }
        let response = self.accept_dialog_response(input)?;
        self.accept_securities_response(response, Some(pending), received_at)
    }

    fn accept_securities_response(
        &mut self,
        response: Response,
        pending: Option<PendingChallenge>,
        received_at: NaiveDateTime,
    ) -> Result<SecuritiesTransactionsResult, Error> {
        if let Some(error) = response.first_error() {
            return self.fail_securities(Error::Bank(error));
        }
        let challenge = match self.product_response_challenge(
            &response,
            pending,
            PendingOperation::SecuritiesTransactions,
            self.products
                .securities
                .as_ref()
                .is_some_and(|state| state.requires_tan),
            received_at,
        ) {
            Ok(challenge) => challenge,
            Err(error) => return self.fail_securities(error),
        };
        if let Some(challenge) = challenge {
            return Ok(SecuritiesTransactionsResult::Challenge(Box::new(challenge)));
        }
        #[cfg(feature = "diagnostics")]
        {
            self.development_depot_response = response
                .development_securities_transaction_structure()
                .ok()
                .flatten();
        }
        let page = match response.securities_transactions() {
            Ok(Some(page)) => page,
            Ok(None)
                if response
                    .responses()
                    .iter()
                    .any(|response| response.code() == 3010) =>
            {
                match response.continuation_point(3) {
                    Ok(None) => {}
                    Ok(Some(_)) => {
                        return self.fail_securities(Error::InvalidResponse {
                            structure: "empty securities result with continuation",
                        });
                    }
                    Err(error) => return self.fail_securities(error),
                }
                let state = self
                    .products
                    .securities
                    .take()
                    .ok_or(Error::InconsistentState)?;
                self.continuation_active = false;
                return Ok(SecuritiesTransactionsResult::Complete(Box::new(
                    SecuritiesTransactions {
                        account: state.account,
                        entries: state.entries,
                    },
                )));
            }
            Ok(None) => {
                return self.fail_securities(Error::MissingValue {
                    field: "securities-transaction response",
                });
            }
            Err(error) => return self.fail_securities(error),
        };
        #[cfg(feature = "diagnostics")]
        {
            self.development_depot_response = Some(page.development_facts.clone());
        }
        let point = match response.continuation_point(3) {
            Ok(point) => point.map(str::to_owned),
            Err(error) => return self.fail_securities(error),
        };
        if page.more != point.is_some() {
            return self.fail_securities(Error::InvalidResponse {
                structure: "MT536 page indicator and FinTS continuation",
            });
        }
        if let Err(error) =
            self.verify_securities_identity(&page.institute_code, &page.account_number, false)
        {
            return self.fail_securities(error);
        }
        let state = self
            .products
            .securities
            .as_mut()
            .ok_or(Error::InconsistentState)?;
        if state
            .entries
            .len()
            .checked_add(page.entries.len())
            .is_none_or(|count| count > ENTRY_LIMIT)
        {
            return self.fail_securities(Error::PaginationLimitReached);
        }
        state.entries.extend(page.entries);
        self.continuation_active = false;
        if let Some(point) = point {
            if !state.seen.insert(point.clone()) {
                return self.fail_securities(Error::RepeatedContinuationPoint);
            }
            state.continuation_point = Some(point);
            return Ok(SecuritiesTransactionsResult::Continue);
        }
        let state = self
            .products
            .securities
            .take()
            .ok_or(Error::InconsistentState)?;
        Ok(SecuritiesTransactionsResult::Complete(Box::new(
            SecuritiesTransactions {
                account: state.account,
                entries: state.entries,
            },
        )))
    }

    pub(crate) fn credit_card_transactions_request(
        &mut self,
        account_index: usize,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_product_idle()?;
        validate_range(from, to)?;
        let account = self.product_account(
            account_index,
            "HKKKU",
            Limitation::CreditCardTransactionsNotAuthorized,
        )?;
        let parameters = self.parameters();
        let capability = parameters.credit_card_transactions.clone().ok_or(
            if parameters.credit_card_transactions_advertised {
                Limitation::CreditCardTransactionsVersion
            } else {
                Limitation::CreditCardTransactionsNotAdvertised
            },
        )?;
        if (from.is_some() || to.is_some()) && !capability.date_range_allowed {
            return Err(Limitation::CreditCardTransactionsVersion.into());
        }
        let requires_tan = required_tan(
            parameters.credit_card_transactions_requires_tan,
            Limitation::PinTanParameters,
        )?;
        let tan = self.product_tan(requires_tan)?;
        let message = {
            let context = self.context(date, time)?;
            segments::credit_card_transactions_request(
                &context,
                &account,
                &capability,
                from,
                to,
                None,
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        self.products.card_transactions = Some(CardTransactionState {
            account,
            capability,
            from,
            to,
            requires_tan,
            continuation_point: None,
            seen: HashSet::new(),
            pages: 1,
            reported_card_number: None,
            reported_account_id: None,
            current_balance: None,
            last_statement_date: None,
            next_statement_date: None,
            entries: Vec::new(),
        });
        Ok(message)
    }

    pub(crate) fn accept_credit_card_transactions(
        &mut self,
        input: &[u8],
        received_at: NaiveDateTime,
    ) -> Result<CreditCardTransactionsResult, Error> {
        let response = self.accept_dialog_response(input)?;
        self.accept_credit_card_transaction_response(response, None, received_at)
    }

    pub(crate) fn next_credit_card_transactions_page_request(
        &mut self,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        if self.continuation_active {
            return Err(Error::InconsistentState);
        }
        let state = self
            .products
            .card_transactions
            .as_ref()
            .ok_or(Error::InconsistentState)?;
        if state.pages >= PAGE_LIMIT {
            return self.fail_card_transactions(Error::PaginationLimitReached);
        }
        let continuation_point = state
            .continuation_point
            .as_deref()
            .ok_or(Error::InconsistentState)?;
        let tan = self.product_tan(state.requires_tan)?;
        let message = {
            let context = self.context(date, time)?;
            segments::credit_card_transactions_request(
                &context,
                &state.account,
                &state.capability,
                state.from,
                state.to,
                Some(continuation_point),
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        self.products
            .card_transactions
            .as_mut()
            .ok_or(Error::InconsistentState)?
            .pages += 1;
        Ok(message)
    }

    pub(crate) fn accept_credit_card_transactions_continuation(
        &mut self,
        input: &[u8],
        pending: PendingChallenge,
        received_at: NaiveDateTime,
    ) -> Result<CreditCardTransactionsResult, Error> {
        self.ensure_pending_dialog(&pending)?;
        if pending.operation != PendingOperation::CreditCardTransactions {
            return Err(Error::InconsistentState);
        }
        let response = self.accept_dialog_response(input)?;
        self.accept_credit_card_transaction_response(response, Some(pending), received_at)
    }

    fn accept_credit_card_transaction_response(
        &mut self,
        response: Response,
        pending: Option<PendingChallenge>,
        received_at: NaiveDateTime,
    ) -> Result<CreditCardTransactionsResult, Error> {
        if let Some(error) = response.first_error() {
            return self.fail_card_transactions(Error::Bank(error));
        }
        let challenge = match self.product_response_challenge(
            &response,
            pending,
            PendingOperation::CreditCardTransactions,
            self.products
                .card_transactions
                .as_ref()
                .is_some_and(|state| state.requires_tan),
            received_at,
        ) {
            Ok(challenge) => challenge,
            Err(error) => return self.fail_card_transactions(error),
        };
        if let Some(challenge) = challenge {
            return Ok(CreditCardTransactionsResult::Challenge(Box::new(challenge)));
        }
        let page = match response.credit_card_transactions() {
            Ok(Some(page)) => page,
            Ok(None)
                if response
                    .responses()
                    .iter()
                    .any(|response| response.code() == 3010) =>
            {
                match response.continuation_point(3) {
                    Ok(None) => {}
                    Ok(Some(_)) => {
                        return self.fail_card_transactions(Error::InvalidResponse {
                            structure: "empty credit-card result with continuation",
                        });
                    }
                    Err(error) => return self.fail_card_transactions(error),
                }
                return self.finish_empty_card_transactions();
            }
            Ok(None) => {
                return self.fail_card_transactions(Error::MissingValue {
                    field: "credit-card transaction response",
                });
            }
            Err(error) => return self.fail_card_transactions(error),
        };
        let point = match response.continuation_point(3) {
            Ok(point) => point.map(str::to_owned),
            Err(error) => return self.fail_card_transactions(error),
        };
        let state = self
            .products
            .card_transactions
            .as_mut()
            .ok_or(Error::InconsistentState)?;
        // G112 leaves card-number masking to the institution. The active request
        // binds the result; only the UPD customer/account ID can be compared
        // byte-for-byte when both sides supplied it.
        if let (Some(expected), Some(reported)) = (
            state.account.subaccount.as_deref(),
            page.reported_account_id.as_deref(),
        ) && expected != reported
        {
            return self.fail_card_transactions(Error::InconsistentState);
        }
        if state
            .entries
            .len()
            .checked_add(page.entries.len())
            .is_none_or(|count| count > ENTRY_LIMIT)
        {
            return self.fail_card_transactions(Error::PaginationLimitReached);
        }
        if let Some(current) = &state.reported_card_number
            && current != &page.reported_card_number
        {
            return self.fail_card_transactions(Error::InconsistentState);
        }
        state.reported_card_number = Some(page.reported_card_number);
        if page.reported_account_id.is_some() {
            state.reported_account_id = page.reported_account_id;
        }
        if page.current_balance.is_some() {
            state.current_balance = page.current_balance;
        }
        if page.last_statement_date.is_some() {
            state.last_statement_date = page.last_statement_date;
        }
        if page.next_statement_date.is_some() {
            state.next_statement_date = page.next_statement_date;
        }
        state.entries.extend(page.entries);
        self.continuation_active = false;
        if let Some(point) = point {
            if !state.seen.insert(point.clone()) {
                return self.fail_card_transactions(Error::RepeatedContinuationPoint);
            }
            state.continuation_point = Some(point);
            return Ok(CreditCardTransactionsResult::Continue);
        }
        let state = self
            .products
            .card_transactions
            .take()
            .ok_or(Error::InconsistentState)?;
        Ok(CreditCardTransactionsResult::Complete(Box::new(
            CreditCardTransactions {
                account: state.account,
                reported_card_number: state.reported_card_number,
                reported_account_id: state.reported_account_id,
                current_balance: state.current_balance,
                last_statement_date: state.last_statement_date,
                next_statement_date: state.next_statement_date,
                entries: state.entries,
            },
        )))
    }

    fn finish_empty_card_transactions(&mut self) -> Result<CreditCardTransactionsResult, Error> {
        let state = self
            .products
            .card_transactions
            .take()
            .ok_or(Error::InconsistentState)?;
        self.continuation_active = false;
        Ok(CreditCardTransactionsResult::Complete(Box::new(
            CreditCardTransactions {
                account: state.account,
                reported_card_number: state.reported_card_number,
                reported_account_id: state.reported_account_id,
                current_balance: state.current_balance,
                last_statement_date: state.last_statement_date,
                next_statement_date: state.next_statement_date,
                entries: state.entries,
            },
        )))
    }

    pub(crate) fn credit_card_balance_request(
        &mut self,
        account_index: usize,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_product_idle()?;
        let account = self.product_account(
            account_index,
            "HKKKS",
            Limitation::CreditCardBalanceNotAuthorized,
        )?;
        let parameters = self.parameters();
        let account_required = parameters.credit_card_balance_account_required.ok_or(
            if parameters.credit_card_balance_advertised {
                Limitation::CreditCardBalanceVersion
            } else {
                Limitation::CreditCardBalanceNotAdvertised
            },
        )?;
        let requires_tan = required_tan(
            parameters.credit_card_balance_requires_tan,
            Limitation::PinTanParameters,
        )?;
        let tan = self.product_tan(requires_tan)?;
        let message = {
            let context = self.context(date, time)?;
            segments::credit_card_balance_request(
                &context,
                &account,
                account_required,
                tan.as_ref()
                    .map(|(method, medium)| (method, medium.as_deref())),
            )?
        };
        self.products.card_balance = Some(account);
        Ok(message)
    }

    pub(crate) fn accept_credit_card_balance(
        &mut self,
        input: &[u8],
        received_at: NaiveDateTime,
    ) -> Result<CreditCardBalanceResult, Error> {
        let response = self.accept_dialog_response(input)?;
        self.accept_credit_card_balance_response(response, None, received_at)
    }

    pub(crate) fn accept_credit_card_balance_continuation(
        &mut self,
        input: &[u8],
        pending: PendingChallenge,
        received_at: NaiveDateTime,
    ) -> Result<CreditCardBalanceResult, Error> {
        self.ensure_pending_dialog(&pending)?;
        if pending.operation != PendingOperation::CreditCardBalance {
            return Err(Error::InconsistentState);
        }
        let response = self.accept_dialog_response(input)?;
        self.accept_credit_card_balance_response(response, Some(pending), received_at)
    }

    fn accept_credit_card_balance_response(
        &mut self,
        response: Response,
        pending: Option<PendingChallenge>,
        received_at: NaiveDateTime,
    ) -> Result<CreditCardBalanceResult, Error> {
        if let Some(error) = response.first_error() {
            return self.fail_card_balance(Error::Bank(error));
        }
        let requires_tan = self.parameters().credit_card_balance_requires_tan == Some(true);
        let challenge = match self.product_response_challenge(
            &response,
            pending,
            PendingOperation::CreditCardBalance,
            requires_tan,
            received_at,
        ) {
            Ok(challenge) => challenge,
            Err(error) => return self.fail_card_balance(error),
        };
        if let Some(challenge) = challenge {
            return Ok(CreditCardBalanceResult::Challenge(Box::new(challenge)));
        }
        let fields = match response.credit_card_balance() {
            Ok(Some(fields)) => fields,
            Ok(None) => {
                return self.fail_card_balance(Error::MissingValue {
                    field: "credit-card balance response",
                });
            }
            Err(error) => return self.fail_card_balance(error),
        };
        let account = self
            .products
            .card_balance
            .take()
            .ok_or(Error::InconsistentState)?;
        // G112 permits institution-defined card-number masking; preserve the
        // response value without treating its display form as the UPD identity.
        if let (Some(expected), Some(reported)) = (
            account.subaccount.as_deref(),
            fields.reported_account_id.as_deref(),
        ) && expected != reported
        {
            return self.fail_card_balance(Error::InconsistentState);
        }
        self.continuation_active = false;
        Ok(CreditCardBalanceResult::Complete(Box::new(
            fields.with_account(account),
        )))
    }

    fn ensure_product_idle(&self) -> Result<(), Error> {
        // Product requests check the pre-existing cash transaction state here;
        // cash requests independently reject continuation_active. Through the
        // concrete public Client, only one synchronous operation can enter.
        if self.continuation_active || self.products.active() || self.transaction.is_some() {
            Err(Error::InconsistentState)
        } else {
            Ok(())
        }
    }

    fn product_account(
        &self,
        index: usize,
        operation: &str,
        unauthorized: Limitation,
    ) -> Result<Account, Error> {
        let account = self.accounts().get(index).ok_or(unauthorized)?.clone();
        // Formals E.2/E.3: "Erlaubte Geschäftsvorfälle" and UPD-Verwendung
        // determine whether an operation is authorized for an account. Kontoart
        // is descriptive classification; correction P24 changes its occupancy
        // for payment-account identification, not its authorization semantics.
        let permission = account.required_signatures(operation).ok_or(unauthorized)?;
        if permission > 1 {
            return Err(Limitation::MultipleSigners.into());
        }
        Ok(account)
    }

    fn product_tan(
        &self,
        required: bool,
    ) -> Result<Option<(crate::model::TanMethod, Option<String>)>, Error> {
        if required {
            Ok(Some((
                self.active_method()?.clone(),
                self.state.selected_tan_medium.clone(),
            )))
        } else {
            Ok(None)
        }
    }

    fn product_response_challenge(
        &mut self,
        response: &Response,
        pending: Option<PendingChallenge>,
        operation: PendingOperation,
        requires_tan: bool,
        received_at: NaiveDateTime,
    ) -> Result<Option<PendingChallenge>, Error> {
        let method = self.active_method().ok().cloned();
        let tan_response = method
            .as_ref()
            .map(|method| response.tan(method.hktan_version))
            .transpose()?
            .flatten()
            .filter(|tan| tan.challenge.reference != "noref");
        if tan_response.is_some() && !requires_tan {
            return Err(Error::InvalidResponse {
                structure: "unsolicited product TAN challenge",
            });
        }
        let Some(tan_response) = tan_response else {
            return Ok(None);
        };
        let result_code: &[u8] = match operation {
            PendingOperation::DepotPositions => b"HIWPD",
            PendingOperation::SecuritiesTransactions => b"HIWDU",
            PendingOperation::CreditCardTransactions => b"HIKKU",
            PendingOperation::CreditCardBalance => b"HIKKS",
            _ => return Err(Error::InconsistentState),
        };
        if response.has_segment(result_code) {
            // PIN/TAN 2020 B.4.2.2 (correction T32), steps 2a-2c:
            // a final decoupled status response contains the original order's
            // feedback and may contain its explicit result together with the
            // HITAN process-S response. Code 3956 instead identifies a still
            // pending approval; 3955/3957/3958 likewise describe a continuing
            // or unusable approval state rather than terminal order delivery.
            let terminal_decoupled_result = pending.as_ref().is_some_and(|pending| {
                pending.method.process == TanProcess::Decoupled
                    && tan_response.process == "S"
                    && tan_response.challenge.reference == pending.challenge.reference
                    && !response
                        .responses()
                        .iter()
                        .any(|response| (3955..=3958).contains(&response.code()))
            });
            if terminal_decoupled_result {
                return Ok(None);
            }
            return Err(Error::InvalidResponse {
                structure: "product result and TAN challenge together",
            });
        }
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
        Ok(Some(match pending {
            Some(pending) => continued_challenge(pending, tan_response.challenge)?,
            None => pending_challenge(
                tan_response.challenge,
                operation,
                method.ok_or(Error::InconsistentState)?,
                response.dialog_id(),
                received_at,
            )?,
        }))
    }

    fn verify_securities_identity(
        &mut self,
        institute_code: &str,
        account_number: &str,
        positions: bool,
    ) -> Result<(), Error> {
        let requested = if positions {
            self.products.positions.as_ref().map(|state| &state.account)
        } else {
            self.products
                .securities
                .as_ref()
                .map(|state| &state.account)
        }
        .ok_or(Error::InconsistentState)?;
        if requested.account_number.as_deref() != Some(account_number)
            || requested
                .institute
                .as_ref()
                .is_none_or(|institute| institute.institute_code != institute_code)
        {
            return Err(Error::InconsistentState);
        }
        Ok(())
    }

    fn fail_positions<T>(&mut self, error: Error) -> Result<T, Error> {
        self.products.positions = None;
        self.continuation_active = false;
        Err(error)
    }

    fn fail_securities<T>(&mut self, error: Error) -> Result<T, Error> {
        self.products.securities = None;
        self.continuation_active = false;
        Err(error)
    }

    fn fail_card_transactions<T>(&mut self, error: Error) -> Result<T, Error> {
        self.products.card_transactions = None;
        self.continuation_active = false;
        Err(error)
    }

    fn fail_card_balance<T>(&mut self, error: Error) -> Result<T, Error> {
        self.products.card_balance = None;
        self.continuation_active = false;
        Err(error)
    }
}

fn required_tan(value: Option<bool>, missing: Limitation) -> Result<bool, Error> {
    value.ok_or_else(|| missing.into())
}

fn validate_range(from: Option<NaiveDate>, to: Option<NaiveDate>) -> Result<(), Error> {
    if from.zip(to).is_some_and(|(from, to)| to < from) {
        Err(InputError::TransactionDateRange.into())
    } else {
        Ok(())
    }
}
