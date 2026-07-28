use chrono::{NaiveDate, NaiveTime};
use encoding_rs::mem;
use serde::{Deserialize, Serialize};

use crate::error::InputError;

#[derive(Clone)]
pub struct ProductIdentity {
    pub(crate) registration_id: String,
    pub(crate) version: String,
}

impl ProductIdentity {
    pub fn new(
        registration_id: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, InputError> {
        let registration_id = registration_id.into();
        let version = version.into();
        if !valid_latin1_length(&registration_id, 1, 25) {
            return Err(InputError::ProductRegistration);
        }
        if !valid_latin1_length(&version, 1, 5) {
            return Err(InputError::ProductVersion);
        }
        Ok(Self {
            registration_id,
            version,
        })
    }
}

#[derive(Clone)]
pub struct InstituteId {
    pub(crate) country_code: String,
    pub(crate) institute_code: String,
}

impl InstituteId {
    pub fn new(
        country_code: impl Into<String>,
        institute_code: impl Into<String>,
    ) -> Result<Self, InputError> {
        let country_code = country_code.into();
        let institute_code = institute_code.into();
        if country_code.len() != 3 || !country_code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(InputError::CountryCode);
        }
        if !valid_latin1_length(&institute_code, 1, 30) {
            return Err(InputError::InstituteCode);
        }
        Ok(Self {
            country_code,
            institute_code,
        })
    }
}

pub struct Credentials {
    pub(crate) user_id: String,
    pub(crate) customer_id: String,
    pub(crate) pin: String,
}

impl Credentials {
    pub fn new(
        user_id: impl Into<String>,
        customer_id: Option<String>,
        pin: impl Into<String>,
    ) -> Result<Self, InputError> {
        let user_id = user_id.into();
        let customer_id = customer_id.unwrap_or_else(|| user_id.clone());
        let pin = pin.into();
        if !valid_latin1_length(&user_id, 1, 30) {
            return Err(InputError::UserId);
        }
        if !valid_latin1_length(&customer_id, 1, 30) {
            return Err(InputError::CustomerId);
        }
        if !valid_latin1_length(&pin, 1, 99) {
            return Err(InputError::Pin);
        }
        Ok(Self {
            user_id,
            customer_id,
            pin,
        })
    }
}

pub struct Tan {
    pub(crate) value: String,
}

pub struct Challenge {
    pub(crate) reference: String,
    pub(crate) text: Option<String>,
    pub(crate) hhd_uc: Option<Vec<u8>>,
    pub(crate) expires_at: Option<Timestamp>,
    pub(crate) medium_name: Option<String>,
}

impl Challenge {
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn hhd_uc(&self) -> Option<&[u8]> {
        self.hhd_uc.as_deref()
    }

    pub fn expires_at(&self) -> Option<Timestamp> {
        self.expires_at
    }

    pub fn medium_name(&self) -> Option<&str> {
        self.medium_name.as_deref()
    }
}

impl Tan {
    pub fn new(value: impl Into<String>) -> Result<Self, InputError> {
        let value = value.into();
        if !valid_latin1_length(&value, 1, 99) {
            return Err(InputError::Tan);
        }
        Ok(Self { value })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum TanProcess {
    ProcessVariantOne,
    ProcessVariantTwo,
    Decoupled,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct TanMethod {
    pub(crate) security_function: String,
    pub(crate) hktan_version: u16,
    pub(crate) process: TanProcess,
    pub(crate) technical_id: String,
    pub(crate) display_name: String,
    pub(crate) dk_method: Option<String>,
    pub(crate) max_tan_length: Option<u8>,
    pub(crate) tan_format: Option<String>,
    pub(crate) medium_name_required: bool,
    pub(crate) hhd_response_required: bool,
    pub(crate) max_decoupled_polls: Option<u16>,
    pub(crate) first_poll_delay_seconds: Option<u16>,
    pub(crate) next_poll_delay_seconds: Option<u16>,
    pub(crate) manual_polling_allowed: bool,
    pub(crate) automatic_polling_allowed: bool,
}

impl TanMethod {
    pub fn security_function(&self) -> &str {
        &self.security_function
    }

    pub fn hktan_version(&self) -> u16 {
        self.hktan_version
    }

    pub fn process(&self) -> TanProcess {
        self.process
    }

    pub fn technical_id(&self) -> &str {
        &self.technical_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn medium_name_required(&self) -> bool {
        self.medium_name_required
    }

    pub fn hhd_response_required(&self) -> bool {
        self.hhd_response_required
    }

    pub fn max_decoupled_polls(&self) -> Option<u16> {
        self.max_decoupled_polls
    }

    pub fn first_poll_delay_seconds(&self) -> Option<u16> {
        self.first_poll_delay_seconds
    }

    pub fn next_poll_delay_seconds(&self) -> Option<u16> {
        self.next_poll_delay_seconds
    }

    pub fn manual_polling_allowed(&self) -> bool {
        self.manual_polling_allowed
    }

    pub fn automatic_polling_allowed(&self) -> bool {
        self.automatic_polling_allowed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TanMediumClass {
    All,
    Generator,
    List,
    Mobile,
    Secoder,
    Bilateral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TanMediumStatus {
    Available,
    Active,
    FollowUpAvailable,
    FollowUpActive,
}

#[derive(Clone)]
pub struct TanMedium {
    pub(crate) class: TanMediumClass,
    pub(crate) status: TanMediumStatus,
    pub(crate) security_function: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) masked_phone: Option<String>,
}

impl TanMedium {
    pub fn class(&self) -> TanMediumClass {
        self.class
    }

    pub fn status(&self) -> TanMediumStatus {
        self.status
    }

    pub fn security_function(&self) -> Option<&str> {
        self.security_function.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn masked_phone(&self) -> Option<&str> {
        self.masked_phone.as_deref()
    }
}

/// Private account metadata returned by the institution through the UPD.
///
/// These fields identify a real account of the user. `Account` values appear inside
/// serialized [`ReusableState`], so callers must apply the same encrypted-storage and
/// redaction requirements. This type intentionally provides no `Debug` implementation.
#[derive(Clone, Deserialize, Serialize)]
pub struct Account {
    pub(crate) iban: Option<String>,
    pub(crate) bic: Option<String>,
    pub(crate) account_number: Option<String>,
    pub(crate) subaccount: Option<String>,
    pub(crate) institute: Option<InstituteState>,
    pub(crate) currency: Option<String>,
    pub(crate) account_type: Option<u8>,
    pub(crate) owner_name_1: Option<String>,
    pub(crate) owner_name_2: Option<String>,
    pub(crate) product_name: Option<String>,
    pub(crate) allowed_operations: Vec<OperationPermission>,
}

impl Account {
    pub fn iban(&self) -> Option<&str> {
        self.iban.as_deref()
    }

    pub fn bic(&self) -> Option<&str> {
        self.bic.as_deref()
    }

    pub fn account_number(&self) -> Option<&str> {
        self.account_number.as_deref()
    }

    pub fn subaccount(&self) -> Option<&str> {
        self.subaccount.as_deref()
    }

    pub fn currency(&self) -> Option<&str> {
        self.currency.as_deref()
    }

    pub fn account_type(&self) -> Option<u8> {
        self.account_type
    }

    pub fn product_name(&self) -> Option<&str> {
        self.product_name.as_deref()
    }

    pub fn owner_name_1(&self) -> Option<&str> {
        self.owner_name_1.as_deref()
    }

    pub fn owner_name_2(&self) -> Option<&str> {
        self.owner_name_2.as_deref()
    }

    pub fn allows_balance(&self) -> bool {
        self.allowed_operations
            .iter()
            .any(|operation| operation.code == "HKSAL")
    }

    pub fn allows_booked_transactions(&self) -> bool {
        self.allowed_operations
            .iter()
            .any(|operation| matches!(operation.code.as_str(), "HKCAZ" | "HKKAZ"))
    }

    pub fn allows_depot_positions(&self) -> bool {
        self.allowed_operations
            .iter()
            .any(|operation| operation.code == "HKWPD")
    }

    pub fn allows_securities_transactions(&self) -> bool {
        self.allowed_operations
            .iter()
            .any(|operation| operation.code == "HKWDU")
    }

    pub fn allows_credit_card_transactions(&self) -> bool {
        self.allowed_operations
            .iter()
            .any(|operation| operation.code == "HKKKU")
    }

    pub fn allows_credit_card_balance(&self) -> bool {
        self.allowed_operations
            .iter()
            .any(|operation| operation.code == "HKKKS")
    }

    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        match (self.iban.as_deref(), other.iban.as_deref()) {
            (Some(left), Some(right)) => left == right,
            _ => {
                self.account_number.is_some()
                    && self.account_number == other.account_number
                    && self.subaccount == other.subaccount
                    && match (&self.institute, &other.institute) {
                        (Some(left), Some(right)) => {
                            left.country_code == right.country_code
                                && left.institute_code == right.institute_code
                        }
                        _ => false,
                    }
            }
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct OperationPermission {
    pub(crate) code: String,
    pub(crate) required_signatures: u8,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct InstituteState {
    pub(crate) country_code: String,
    pub(crate) institute_code: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct CamtCapability {
    pub(crate) descriptor: String,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub(crate) struct CreditCardCapability {
    pub(crate) account_required: bool,
    pub(crate) date_range_allowed: bool,
    pub(crate) entry_count_allowed: bool,
}

#[derive(Clone)]
pub(crate) enum TransactionFormat {
    Camt { descriptor: String },
    Mt940 { version: u16 },
}

/// Caller-persisted protocol state for subsequent FinTS dialogs.
///
/// Its serialized form transitively contains private UPD data, including account
/// identifiers such as IBANs and account numbers, and account owner names. Callers must
/// persist it only in encrypted storage and must never log or display the serialized
/// bytes.
///
/// This state deliberately never contains credentials, PINs, TANs, challenges, dialog
/// identifiers, or live session state.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct ReusableState {
    pub(crate) system_id: Option<String>,
    pub(crate) bpd_version: u16,
    pub(crate) upd_version: u16,
    pub(crate) balance_versions: Vec<u16>,
    #[serde(default)]
    pub(crate) balance_capability_advertised: bool,
    pub(crate) balance_requires_tan: Option<bool>,
    #[serde(default)]
    pub(crate) transaction_capability_advertised: bool,
    #[serde(default)]
    pub(crate) camt_capability: Option<CamtCapability>,
    #[serde(default)]
    pub(crate) legacy_transaction_versions: Vec<u16>,
    #[serde(default)]
    pub(crate) camt_requires_tan: Option<bool>,
    #[serde(default)]
    pub(crate) legacy_transactions_require_tan: Option<bool>,
    #[serde(default)]
    pub(crate) depot_positions_advertised: bool,
    #[serde(default)]
    pub(crate) depot_positions_supported: bool,
    #[serde(default)]
    pub(crate) depot_positions_requires_tan: Option<bool>,
    #[serde(default)]
    pub(crate) securities_transactions_advertised: bool,
    #[serde(default)]
    pub(crate) securities_transactions_supported: bool,
    #[serde(default)]
    pub(crate) securities_transactions_requires_tan: Option<bool>,
    #[serde(default)]
    pub(crate) credit_card_transactions_advertised: bool,
    #[serde(default)]
    pub(crate) credit_card_transactions: Option<CreditCardCapability>,
    #[serde(default)]
    pub(crate) credit_card_transactions_requires_tan: Option<bool>,
    #[serde(default)]
    pub(crate) credit_card_balance_advertised: bool,
    #[serde(default)]
    pub(crate) credit_card_balance_account_required: Option<bool>,
    #[serde(default)]
    pub(crate) credit_card_balance_requires_tan: Option<bool>,
    pub(crate) tan_methods: Vec<TanMethod>,
    pub(crate) accounts: Vec<Account>,
    pub(crate) selected_tan_method: Option<String>,
    pub(crate) selected_tan_medium: Option<String>,
}

impl ReusableState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn system_id(&self) -> Option<&str> {
        self.system_id.as_deref()
    }

    pub fn bpd_version(&self) -> u16 {
        self.bpd_version
    }

    pub fn upd_version(&self) -> u16 {
        self.upd_version
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    pub fn tan_methods(&self) -> &[TanMethod] {
        &self.tan_methods
    }

    pub fn selected_tan_method(&self) -> Option<&str> {
        self.selected_tan_method.as_deref()
    }

    pub fn selected_tan_medium(&self) -> Option<&str> {
        self.selected_tan_medium.as_deref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreditDebit {
    Credit,
    Debit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantityUnit {
    Units,
    Nominal,
}

/// Exact securities quantity supplied by the institution.
///
/// The value can reveal private holdings and intentionally provides no `Debug`
/// implementation.
#[derive(Clone)]
pub struct SecuritiesQuantity {
    coefficient: u128,
    scale: u8,
    unit: QuantityUnit,
    negative: bool,
}

impl SecuritiesQuantity {
    pub(crate) fn new(coefficient: u128, scale: u8, unit: QuantityUnit, negative: bool) -> Self {
        Self {
            coefficient,
            scale,
            unit,
            negative,
        }
    }

    pub fn coefficient(&self) -> u128 {
        self.coefficient
    }

    pub fn scale(&self) -> u8 {
        self.scale
    }

    pub fn unit(&self) -> QuantityUnit {
        self.unit
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }
}

/// Identifiers and name for one institution-supplied security.
///
/// These fields identify private holdings and intentionally provide no `Debug`
/// implementation.
#[derive(Clone)]
pub struct SecurityInstrument {
    pub(crate) isin: Option<String>,
    pub(crate) wkn: Option<String>,
    pub(crate) name: String,
}

impl SecurityInstrument {
    pub fn isin(&self) -> Option<&str> {
        self.isin.as_deref()
    }

    pub fn wkn(&self) -> Option<&str> {
        self.wkn.as_deref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceQuality {
    Market,
    Indicative,
}

/// Exact quoted security price or cost basis supplied by the institution.
///
/// `quality` is absent for a structured cost basis. This private holding value
/// intentionally provides no `Debug` implementation.
#[derive(Clone)]
pub struct SecurityPrice {
    coefficient: u128,
    scale: u8,
    currency: Option<String>,
    percentage: bool,
    quality: Option<PriceQuality>,
    date: Option<NaiveDate>,
    time: Option<NaiveTime>,
}

impl SecurityPrice {
    pub(crate) fn new(
        coefficient: u128,
        scale: u8,
        currency: Option<String>,
        percentage: bool,
        quality: Option<PriceQuality>,
        date: Option<NaiveDate>,
        time: Option<NaiveTime>,
    ) -> Self {
        Self {
            coefficient,
            scale,
            currency,
            percentage,
            quality,
            date,
            time,
        }
    }

    pub fn coefficient(&self) -> u128 {
        self.coefficient
    }

    pub fn scale(&self) -> u8 {
        self.scale
    }

    pub fn currency(&self) -> Option<&str> {
        self.currency.as_deref()
    }

    pub fn is_percentage(&self) -> bool {
        self.percentage
    }

    pub fn quality(&self) -> Option<PriceQuality> {
        self.quality
    }

    pub fn date(&self) -> Option<NaiveDate> {
        self.date
    }

    pub fn time(&self) -> Option<NaiveTime> {
        self.time
    }
}

/// Signed securities amount; the sign is kept independently from the exact amount.
///
/// This private holding value intentionally provides no `Debug` implementation.
pub struct SecuritiesAmount {
    pub(crate) amount: Amount,
    pub(crate) negative: bool,
}

impl SecuritiesAmount {
    pub fn amount(&self) -> &Amount {
        &self.amount
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }
}

/// One explicitly reported position in a depot statement.
///
/// Optional values remain absent and the private result intentionally provides no
/// `Debug` implementation.
pub struct DepotPosition {
    pub(crate) instrument: SecurityInstrument,
    pub(crate) quantity: SecuritiesQuantity,
    pub(crate) price: Option<SecurityPrice>,
    pub(crate) market_values: Vec<SecuritiesAmount>,
    pub(crate) cost_basis: Option<SecurityPrice>,
}

impl DepotPosition {
    pub fn instrument(&self) -> &SecurityInstrument {
        &self.instrument
    }

    pub fn quantity(&self) -> &SecuritiesQuantity {
        &self.quantity
    }

    pub fn price(&self) -> Option<&SecurityPrice> {
        self.price.as_ref()
    }

    pub fn market_values(&self) -> &[SecuritiesAmount] {
        &self.market_values
    }

    pub fn cost_basis(&self) -> Option<&SecurityPrice> {
        self.cost_basis.as_ref()
    }
}

/// Exhaustively paginated positions for exactly one UPD depot.
///
/// Institution-reported page totals remain in response order; they are never
/// recomputed or deduplicated. This private result intentionally provides no
/// `Debug` implementation.
pub struct DepotPositions {
    pub(crate) account: Account,
    pub(crate) positions: Vec<DepotPosition>,
    pub(crate) total_values: Vec<SecuritiesAmount>,
}

impl DepotPositions {
    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn positions(&self) -> &[DepotPosition] {
        &self.positions
    }

    pub fn total_values(&self) -> &[SecuritiesAmount] {
        &self.total_values
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecuritiesMovement {
    Delivery,
    Receipt,
}

/// One booked securities transaction, containing only explicitly supplied values.
///
/// `NONREF` and an omitted optional transaction-detail block produce absent
/// fields. Free text is not interpreted as a fee or identity. This private result
/// intentionally provides no `Debug` implementation.
pub struct SecuritiesTransaction {
    pub(crate) instrument: SecurityInstrument,
    pub(crate) reference: Option<String>,
    pub(crate) quantity: Option<SecuritiesQuantity>,
    pub(crate) price: Option<SecurityPrice>,
    pub(crate) amount: Option<SecuritiesAmount>,
    pub(crate) accrued_interest: Option<SecuritiesAmount>,
    pub(crate) transaction_kind: Option<String>,
    pub(crate) movement: Option<SecuritiesMovement>,
    pub(crate) effective_date: Option<NaiveDate>,
    pub(crate) value_date: Option<NaiveDate>,
    pub(crate) reversal: Option<bool>,
    pub(crate) free_text: Vec<String>,
}

impl SecuritiesTransaction {
    pub fn instrument(&self) -> &SecurityInstrument {
        &self.instrument
    }
    pub fn reference(&self) -> Option<&str> {
        self.reference.as_deref()
    }
    pub fn quantity(&self) -> Option<&SecuritiesQuantity> {
        self.quantity.as_ref()
    }
    pub fn price(&self) -> Option<&SecurityPrice> {
        self.price.as_ref()
    }
    pub fn amount(&self) -> Option<&SecuritiesAmount> {
        self.amount.as_ref()
    }
    pub fn accrued_interest(&self) -> Option<&SecuritiesAmount> {
        self.accrued_interest.as_ref()
    }
    pub fn transaction_kind(&self) -> Option<&str> {
        self.transaction_kind.as_deref()
    }
    pub fn direction(&self) -> Option<SecuritiesMovement> {
        self.movement
    }
    pub fn effective_date(&self) -> Option<NaiveDate> {
        self.effective_date
    }
    pub fn value_date(&self) -> Option<NaiveDate> {
        self.value_date
    }
    pub fn is_reversal(&self) -> Option<bool> {
        self.reversal
    }
    pub fn free_text(&self) -> &[String] {
        &self.free_text
    }
}

/// Exhaustively paginated booked securities transactions for one UPD depot.
///
/// This private result intentionally provides no `Debug` implementation.
pub struct SecuritiesTransactions {
    pub(crate) account: Account,
    pub(crate) entries: Vec<SecuritiesTransaction>,
}

impl SecuritiesTransactions {
    pub fn account(&self) -> &Account {
        &self.account
    }
    pub fn entries(&self) -> &[SecuritiesTransaction] {
        &self.entries
    }
}

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

/// A protocol timestamp whose time component may be absent.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
    date: NaiveDate,
    time: Option<NaiveTime>,
}

impl Timestamp {
    pub(crate) fn new(date: NaiveDate, time: Option<NaiveTime>) -> Self {
        Self { date, time }
    }

    pub fn date(self) -> NaiveDate {
        self.date
    }

    pub fn time(self) -> Option<NaiveTime> {
        self.time
    }
}

pub struct Amount {
    coefficient: u128,
    scale: u8,
    currency: String,
}

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
