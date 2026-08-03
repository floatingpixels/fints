//! Typed G112 credit-card results: balances, entries, and transactions.

use super::*;

/// Exact amount with the G112 debit/credit direction.
///
/// This private financial value intentionally provides no `Debug` implementation.
pub struct CreditCardAmount {
    pub(crate) amount: Amount,
    pub(crate) direction: CreditDebit,
}

impl CreditCardAmount {
    pub fn amount(&self) -> &Amount {
        &self.amount
    }
    pub fn direction(&self) -> CreditDebit {
        self.direction
    }
}

/// Current G112 credit-card balance with its institution-supplied timestamp.
///
/// The date is mandatory in the `sdo` value and the time remains absent when the
/// institution omits it. This private financial value intentionally provides no
/// `Debug` implementation.
pub struct CreditCardCurrentBalance {
    pub(crate) amount: CreditCardAmount,
    pub(crate) timestamp: Timestamp,
}

impl CreditCardCurrentBalance {
    pub fn amount(&self) -> &CreditCardAmount {
        &self.amount
    }

    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

/// One booked credit-card entry returned by G112 HIKKU.
///
/// Optional fields remain absent. The cash and foreign-use fee fields are
/// bank-formatted display strings defined by G112, not parsed amounts. This
/// private result intentionally provides no `Debug` implementation.
pub struct CreditCardEntry {
    pub(crate) card_number: String,
    pub(crate) receipt_date: NaiveDate,
    pub(crate) booking_date: NaiveDate,
    pub(crate) statement_date: Option<NaiveDate>,
    pub(crate) value_date: Option<NaiveDate>,
    pub(crate) original_amount: Option<CreditCardAmount>,
    pub(crate) exchange_rate: Option<(u128, u8)>,
    pub(crate) booking_amount: CreditCardAmount,
    pub(crate) description: Vec<String>,
    pub(crate) country: Option<String>,
    pub(crate) merchant_name: Option<String>,
    pub(crate) terminal: Option<String>,
    pub(crate) billed: Option<bool>,
    pub(crate) booking_reference: Option<String>,
    pub(crate) fee_code: Option<String>,
    pub(crate) statement_marker: Option<String>,
    pub(crate) cash_fee: Option<String>,
    pub(crate) foreign_use_fee: Option<String>,
}

impl CreditCardEntry {
    pub fn card_number(&self) -> &str {
        &self.card_number
    }
    pub fn receipt_date(&self) -> NaiveDate {
        self.receipt_date
    }
    pub fn booking_date(&self) -> NaiveDate {
        self.booking_date
    }
    pub fn statement_date(&self) -> Option<NaiveDate> {
        self.statement_date
    }
    pub fn value_date(&self) -> Option<NaiveDate> {
        self.value_date
    }
    pub fn original_amount(&self) -> Option<&CreditCardAmount> {
        self.original_amount.as_ref()
    }
    pub fn exchange_rate(&self) -> Option<(u128, u8)> {
        self.exchange_rate
    }
    pub fn booking_amount(&self) -> &CreditCardAmount {
        &self.booking_amount
    }
    pub fn description(&self) -> &[String] {
        &self.description
    }
    pub fn country(&self) -> Option<&str> {
        self.country.as_deref()
    }
    pub fn merchant_name(&self) -> Option<&str> {
        self.merchant_name.as_deref()
    }
    pub fn terminal(&self) -> Option<&str> {
        self.terminal.as_deref()
    }
    pub fn billed(&self) -> Option<bool> {
        self.billed
    }
    pub fn booking_reference(&self) -> Option<&str> {
        self.booking_reference.as_deref()
    }
    pub fn fee_code(&self) -> Option<&str> {
        self.fee_code.as_deref()
    }
    pub fn statement_marker(&self) -> Option<&str> {
        self.statement_marker.as_deref()
    }
    pub fn cash_fee(&self) -> Option<&str> {
        self.cash_fee.as_deref()
    }
    pub fn foreign_use_fee(&self) -> Option<&str> {
        self.foreign_use_fee.as_deref()
    }
}

/// Exhaustively paginated G112 credit-card entries for one UPD account.
///
/// `reported_card_number` is absent when response code 3010 supplied no HIKKU
/// segment; it is never copied from the request. This private result
/// intentionally provides no `Debug` implementation.
pub struct CreditCardTransactions {
    pub(crate) account: Account,
    pub(crate) reported_card_number: Option<String>,
    pub(crate) reported_account_id: Option<String>,
    pub(crate) current_balance: Option<CreditCardCurrentBalance>,
    pub(crate) last_statement_date: Option<NaiveDate>,
    pub(crate) next_statement_date: Option<NaiveDate>,
    pub(crate) entries: Vec<CreditCardEntry>,
}

impl CreditCardTransactions {
    pub fn account(&self) -> &Account {
        &self.account
    }
    pub fn reported_card_number(&self) -> Option<&str> {
        self.reported_card_number.as_deref()
    }
    pub fn reported_account_id(&self) -> Option<&str> {
        self.reported_account_id.as_deref()
    }
    pub fn current_balance(&self) -> Option<&CreditCardCurrentBalance> {
        self.current_balance.as_ref()
    }
    pub fn last_statement_date(&self) -> Option<NaiveDate> {
        self.last_statement_date
    }
    pub fn next_statement_date(&self) -> Option<NaiveDate> {
        self.next_statement_date
    }
    pub fn entries(&self) -> &[CreditCardEntry] {
        &self.entries
    }
}

/// G112 credit-card balance components, kept independent without reconciliation.
///
/// No component is derived from the booked entries. This private result
/// intentionally provides no `Debug` implementation.
pub struct CreditCardBalance {
    pub(crate) account: Account,
    pub(crate) reported_card_number: String,
    pub(crate) reported_account_id: Option<String>,
    pub(crate) current: CreditCardCurrentBalance,
    pub(crate) available: Option<CreditCardAmount>,
    pub(crate) open_authorizations: Option<Amount>,
    pub(crate) credit_limit: Option<Amount>,
    pub(crate) next_statement_date: Option<NaiveDate>,
}

impl CreditCardBalance {
    pub fn account(&self) -> &Account {
        &self.account
    }
    pub fn reported_card_number(&self) -> &str {
        &self.reported_card_number
    }
    pub fn reported_account_id(&self) -> Option<&str> {
        self.reported_account_id.as_deref()
    }
    pub fn current(&self) -> &CreditCardCurrentBalance {
        &self.current
    }
    pub fn available(&self) -> Option<&CreditCardAmount> {
        self.available.as_ref()
    }
    pub fn open_authorizations(&self) -> Option<&Amount> {
        self.open_authorizations.as_ref()
    }
    pub fn credit_limit(&self) -> Option<&Amount> {
        self.credit_limit.as_ref()
    }
    pub fn next_statement_date(&self) -> Option<NaiveDate> {
        self.next_statement_date
    }
}
