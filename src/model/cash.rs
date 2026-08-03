//! Typed cash-account results: booked transactions and the HKSAL balance.

use super::*;

/// An exact MT940 statement position supplied by the institution.
///
/// This is used only when the legacy response has no bank reference. It is not a
/// synthesized transaction fingerprint and intentionally provides no `Debug`
/// implementation.
pub struct StatementPosition {
    statement_number: String,
    page_number: Option<String>,
    entry_index: u32,
}

impl StatementPosition {
    pub(crate) fn new(
        statement_number: String,
        page_number: Option<String>,
        entry_index: u32,
    ) -> Self {
        Self {
            statement_number,
            page_number,
            entry_index,
        }
    }

    pub fn statement_number(&self) -> &str {
        &self.statement_number
    }

    pub fn page_number(&self) -> Option<&str> {
        self.page_number.as_deref()
    }

    pub fn entry_index(&self) -> u32 {
        self.entry_index
    }
}

/// Optional transaction-level data supplied inside a booked camt entry.
///
/// Every field can identify or describe private banking activity. The type therefore
/// intentionally provides no `Debug` implementation.
pub struct BookedTransactionDetail {
    pub(crate) amount: Option<Amount>,
    pub(crate) direction: Option<CreditDebit>,
    pub(crate) customer_reference: Option<String>,
    pub(crate) end_to_end_reference: Option<String>,
    pub(crate) mandate_reference: Option<String>,
    pub(crate) creditor_reference: Option<String>,
    pub(crate) account_servicer_reference: Option<String>,
    pub(crate) bank_transaction_code: Option<String>,
    pub(crate) proprietary_transaction_code: Option<String>,
    pub(crate) counterparty_name: Option<String>,
    pub(crate) counterparty_account: Option<String>,
    pub(crate) remittance_information: Vec<String>,
}

impl BookedTransactionDetail {
    pub fn amount(&self) -> Option<&Amount> {
        self.amount.as_ref()
    }

    pub fn direction(&self) -> Option<CreditDebit> {
        self.direction
    }

    pub fn customer_reference(&self) -> Option<&str> {
        self.customer_reference.as_deref()
    }

    pub fn end_to_end_reference(&self) -> Option<&str> {
        self.end_to_end_reference.as_deref()
    }

    pub fn mandate_reference(&self) -> Option<&str> {
        self.mandate_reference.as_deref()
    }

    pub fn creditor_reference(&self) -> Option<&str> {
        self.creditor_reference.as_deref()
    }

    pub fn account_servicer_reference(&self) -> Option<&str> {
        self.account_servicer_reference.as_deref()
    }

    pub fn bank_transaction_code(&self) -> Option<&str> {
        self.bank_transaction_code.as_deref()
    }

    pub fn proprietary_transaction_code(&self) -> Option<&str> {
        self.proprietary_transaction_code.as_deref()
    }

    pub fn counterparty_name(&self) -> Option<&str> {
        self.counterparty_name.as_deref()
    }

    /// Account identifier supplied for the counterparty.
    ///
    /// camt normally supplies an IBAN, while legacy MT940 subfield `?31` may
    /// contain either an IBAN or a national account number. The value is not
    /// normalized or reinterpreted.
    pub fn counterparty_account(&self) -> Option<&str> {
        self.counterparty_account.as_deref()
    }

    pub fn remittance_information(&self) -> &[String] {
        &self.remittance_information
    }
}

/// A booked cash-account entry returned by the institution.
///
/// Entry references are preserved verbatim. The library never derives a replacement
/// fingerprint. This type contains private transaction data and intentionally provides
/// no `Debug` implementation.
pub struct BookedEntry {
    pub(crate) amount: Amount,
    pub(crate) direction: CreditDebit,
    pub(crate) booking_date: Option<NaiveDate>,
    pub(crate) value_date: Option<NaiveDate>,
    pub(crate) reversal: Option<bool>,
    pub(crate) entry_reference: Option<String>,
    pub(crate) account_servicer_reference: Option<String>,
    pub(crate) bank_transaction_code: Option<String>,
    pub(crate) proprietary_transaction_code: Option<String>,
    pub(crate) statement_position: Option<StatementPosition>,
    pub(crate) details: Vec<BookedTransactionDetail>,
}

impl BookedEntry {
    pub fn amount(&self) -> &Amount {
        &self.amount
    }

    pub fn direction(&self) -> CreditDebit {
        self.direction
    }

    pub fn booking_date(&self) -> Option<NaiveDate> {
        self.booking_date
    }

    pub fn value_date(&self) -> Option<NaiveDate> {
        self.value_date
    }

    pub fn is_reversal(&self) -> Option<bool> {
        self.reversal
    }

    pub fn entry_reference(&self) -> Option<&str> {
        self.entry_reference.as_deref()
    }

    pub fn account_servicer_reference(&self) -> Option<&str> {
        self.account_servicer_reference.as_deref()
    }

    pub fn bank_transaction_code(&self) -> Option<&str> {
        self.bank_transaction_code.as_deref()
    }

    pub fn proprietary_transaction_code(&self) -> Option<&str> {
        self.proprietary_transaction_code.as_deref()
    }

    pub fn statement_position(&self) -> Option<&StatementPosition> {
        self.statement_position.as_ref()
    }

    pub fn details(&self) -> &[BookedTransactionDetail] {
        &self.details
    }
}

/// Booked entries for exactly one UPD account.
///
/// Keeping the account on every result prevents entries from different accounts in one
/// connection from being conflated. This private-data container intentionally provides
/// no `Debug` implementation.
pub struct BookedTransactions {
    pub(crate) account: Account,
    pub(crate) entries: Vec<BookedEntry>,
}

impl BookedTransactions {
    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn entries(&self) -> &[BookedEntry] {
        &self.entries
    }
}

impl Amount {
    pub(crate) fn new(coefficient: u128, scale: u8, currency: String) -> Self {
        Self {
            coefficient,
            scale,
            currency,
        }
    }

    pub fn coefficient(&self) -> u128 {
        self.coefficient
    }

    pub fn scale(&self) -> u8 {
        self.scale
    }

    pub fn currency(&self) -> &str {
        &self.currency
    }
}

pub struct SignedAmount {
    direction: CreditDebit,
    amount: Amount,
    date: NaiveDate,
    time: Option<NaiveTime>,
}

impl SignedAmount {
    pub(crate) fn new(
        direction: CreditDebit,
        amount: Amount,
        date: NaiveDate,
        time: Option<NaiveTime>,
    ) -> Self {
        Self {
            direction,
            amount,
            date,
            time,
        }
    }

    pub fn direction(&self) -> CreditDebit {
        self.direction
    }

    pub fn amount(&self) -> &Amount {
        &self.amount
    }

    pub fn date(&self) -> NaiveDate {
        self.date
    }

    pub fn time(&self) -> Option<NaiveTime> {
        self.time
    }
}

pub struct Balance {
    pub(crate) account: Account,
    pub(crate) product_name: String,
    pub(crate) account_currency: String,
    pub(crate) booked: SignedAmount,
    pub(crate) pending: Option<SignedAmount>,
    pub(crate) credit_line: Option<Amount>,
    pub(crate) available: Option<Amount>,
    pub(crate) already_drawn: Option<Amount>,
    pub(crate) overdraft: Option<Amount>,
    pub(crate) booking_time: Option<Timestamp>,
    pub(crate) due_date: Option<NaiveDate>,
    pub(crate) garnishable_after_month_end: Option<Amount>,
}

impl Balance {
    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn product_name(&self) -> &str {
        &self.product_name
    }

    pub fn account_currency(&self) -> &str {
        &self.account_currency
    }

    pub fn booked(&self) -> &SignedAmount {
        &self.booked
    }

    pub fn pending(&self) -> Option<&SignedAmount> {
        self.pending.as_ref()
    }

    pub fn credit_line(&self) -> Option<&Amount> {
        self.credit_line.as_ref()
    }

    pub fn available(&self) -> Option<&Amount> {
        self.available.as_ref()
    }

    pub fn already_drawn(&self) -> Option<&Amount> {
        self.already_drawn.as_ref()
    }

    pub fn overdraft(&self) -> Option<&Amount> {
        self.overdraft.as_ref()
    }

    pub fn booking_time(&self) -> Option<Timestamp> {
        self.booking_time
    }

    pub fn due_date(&self) -> Option<NaiveDate> {
        self.due_date
    }

    pub fn garnishable_after_month_end(&self) -> Option<&Amount> {
        self.garnishable_after_month_end.as_ref()
    }
}

pub(crate) fn valid_latin1_length(value: &str, minimum: usize, maximum: usize) -> bool {
    mem::is_str_latin1(value)
        && (minimum..=maximum).contains(&mem::encode_latin1_lossy(value).len())
}
