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

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct ReusableState {
    pub(crate) system_id: Option<String>,
    pub(crate) bpd_version: u16,
    pub(crate) upd_version: u16,
    pub(crate) balance_versions: Vec<u16>,
    pub(crate) balance_requires_tan: Option<bool>,
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
