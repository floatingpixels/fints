use chrono::{NaiveDate, NaiveTime};
use encoding_rs::mem;
use serde::{Deserialize, Serialize};

use crate::{capabilities::ParameterSegmentAdvertisement, error::InputError};

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
    #[cfg(feature = "diagnostics")]
    #[serde(skip)]
    pub(crate) development_medium_requirement:
        Option<crate::diagnostics::HitansMediumRequirementFact>,
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

    /// Whether HKTAN requires a selected medium name for this method.
    ///
    /// PIN/TAN makes this effective only when the requirement code is `2` and
    /// the institution advertises more than one active medium.
    pub fn medium_name_required(&self) -> bool {
        self.medium_name_required
    }

    /// Redacted source fields used to compute [`Self::medium_name_required`].
    ///
    /// This accessor exists only with the opt-in `diagnostics` feature. Its fact
    /// shape is unstable and must not drive caller control flow.
    #[cfg(feature = "diagnostics")]
    pub fn development_medium_requirement(&self) -> Option<crate::HitansMediumRequirementFact> {
        self.development_medium_requirement
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
    #[cfg(feature = "diagnostics")]
    pub(crate) development_card_number_present: bool,
    #[cfg(feature = "diagnostics")]
    pub(crate) development_card_sequence_present: bool,
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
    pub(crate) unlisted_operations_unknown: bool,
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
        self.operation_is_not_denied("HKSAL")
    }

    pub fn allows_booked_transactions(&self) -> bool {
        self.operation_is_not_denied("HKCAZ") || self.operation_is_not_denied("HKKAZ")
    }

    pub fn allows_depot_positions(&self) -> bool {
        self.operation_is_not_denied("HKWPD")
    }

    pub fn allows_securities_transactions(&self) -> bool {
        self.operation_is_not_denied("HKWDU")
    }

    pub fn allows_credit_card_transactions(&self) -> bool {
        self.operation_is_not_denied("HKKKU")
    }

    pub fn allows_credit_card_balance(&self) -> bool {
        self.operation_is_not_denied("HKKKS")
    }

    pub(crate) fn operation_permission(&self, code: &str) -> Option<&OperationPermission> {
        self.allowed_operations
            .iter()
            .find(|operation| operation.code == code)
    }

    pub(crate) fn operation_is_not_denied(&self, code: &str) -> bool {
        self.unlisted_operations_unknown || self.operation_permission(code).is_some()
    }

    pub(crate) fn required_signatures(&self, code: &str) -> Option<u8> {
        self.operation_permission(code)
            .map(|permission| permission.required_signatures)
            .or(self.unlisted_operations_unknown.then_some(1))
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
}

/// Serialized format version accepted for [`ReusableState`].
pub const REUSABLE_STATE_VERSION: u16 = 1;

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
///
/// The serialized form is bound to [`REUSABLE_STATE_VERSION`]. After a crate upgrade,
/// stored state may either fail to deserialize because its shape differs or be rejected
/// as [`crate::Error::ReusableStateVersion`] when passed to a client. Both outcomes are
/// expected and have the same recovery: discard the stored state and re-synchronize
/// instead of attempting repair or migration. This is safe because reusable state is
/// derived from bank parameters rather than authored by the caller.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReusableState {
    pub(crate) version: u16,
    pub(crate) system_id: Option<String>,
    pub(crate) bpd_version: u16,
    pub(crate) upd_version: u16,
    pub(crate) balance_versions: Vec<u16>,
    pub(crate) advertised_balance_versions: Vec<u16>,
    pub(crate) advertised_tan_media_versions: Vec<u16>,
    pub(crate) balance_capability_advertised: bool,
    pub(crate) balance_requires_tan: Option<bool>,
    pub(crate) transaction_capability_advertised: bool,
    pub(crate) camt_capability: Option<CamtCapability>,
    pub(crate) advertised_camt_descriptors: Vec<String>,
    pub(crate) camt_storage_period_days: Option<u16>,
    pub(crate) legacy_transaction_versions: Vec<u16>,
    pub(crate) camt_requires_tan: Option<bool>,
    pub(crate) legacy_transactions_require_tan: Option<bool>,
    pub(crate) depot_positions_advertised: bool,
    pub(crate) depot_position_versions: Vec<u16>,
    pub(crate) depot_positions_requires_tan: Option<bool>,
    pub(crate) securities_transactions_advertised: bool,
    pub(crate) securities_transactions_supported: bool,
    pub(crate) securities_transactions_storage_period_days: Option<u16>,
    pub(crate) securities_transactions_requires_tan: Option<bool>,
    pub(crate) credit_card_transactions_advertised: bool,
    pub(crate) credit_card_transactions: Option<CreditCardCapability>,
    pub(crate) credit_card_transactions_storage_period_days: Option<u16>,
    pub(crate) credit_card_transactions_requires_tan: Option<bool>,
    pub(crate) credit_card_balance_advertised: bool,
    pub(crate) credit_card_balance_account_required: Option<bool>,
    pub(crate) credit_card_balance_requires_tan: Option<bool>,
    // Derived from the current BPD in process and deliberately absent from persistence.
    #[serde(skip)]
    pub(crate) advertised_parameter_segments: Vec<ParameterSegmentAdvertisement>,
    pub(crate) tan_methods: Vec<TanMethod>,
    pub(crate) accounts: Vec<Account>,
    pub(crate) selected_tan_method: Option<String>,
    pub(crate) selected_tan_medium: Option<String>,
}

impl Default for ReusableState {
    fn default() -> Self {
        Self {
            version: REUSABLE_STATE_VERSION,
            system_id: None,
            bpd_version: 0,
            upd_version: 0,
            balance_versions: Vec::new(),
            advertised_balance_versions: Vec::new(),
            advertised_tan_media_versions: Vec::new(),
            balance_capability_advertised: false,
            balance_requires_tan: None,
            transaction_capability_advertised: false,
            camt_capability: None,
            advertised_camt_descriptors: Vec::new(),
            camt_storage_period_days: None,
            legacy_transaction_versions: Vec::new(),
            camt_requires_tan: None,
            legacy_transactions_require_tan: None,
            depot_positions_advertised: false,
            depot_position_versions: Vec::new(),
            depot_positions_requires_tan: None,
            securities_transactions_advertised: false,
            securities_transactions_supported: false,
            securities_transactions_storage_period_days: None,
            securities_transactions_requires_tan: None,
            credit_card_transactions_advertised: false,
            credit_card_transactions: None,
            credit_card_transactions_storage_period_days: None,
            credit_card_transactions_requires_tan: None,
            credit_card_balance_advertised: false,
            credit_card_balance_account_required: None,
            credit_card_balance_requires_tan: None,
            advertised_parameter_segments: Vec::new(),
            tan_methods: Vec::new(),
            accounts: Vec::new(),
            selected_tan_method: None,
            selected_tan_medium: None,
        }
    }
}

impl ReusableState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn system_id(&self) -> Option<&str> {
        self.system_id.as_deref()
    }

    /// The serialized reusable-state format version.
    pub fn version(&self) -> u16 {
        self.version
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

pub(crate) fn valid_latin1_length(value: &str, minimum: usize, maximum: usize) -> bool {
    mem::is_str_latin1(value)
        && (minimum..=maximum).contains(&mem::encode_latin1_lossy(value).len())
}

mod cash;
mod credit_card;
mod securities;

pub use cash::*;
pub use credit_card::*;
pub use securities::*;

#[cfg(test)]
mod tests {
    use serde::de::{
        self, IntoDeserializer, Visitor,
        value::{Error as ValueError, MapDeserializer, SeqDeserializer},
    };
    use serde::ser::{self, Impossible, SerializeSeq, SerializeStruct};

    use super::*;

    enum StateValue {
        Bool(bool),
        U16(u16),
        None,
        Sequence(Vec<StateValue>),
    }

    impl<'de> IntoDeserializer<'de, ValueError> for StateValue {
        type Deserializer = Self;

        fn into_deserializer(self) -> Self::Deserializer {
            self
        }
    }

    impl<'de> de::Deserializer<'de> for StateValue {
        type Error = ValueError;

        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            match self {
                Self::Bool(value) => visitor.visit_bool(value),
                Self::U16(value) => visitor.visit_u16(value),
                Self::None => visitor.visit_none(),
                Self::Sequence(values) => {
                    visitor.visit_seq(SeqDeserializer::<_, ValueError>::new(values.into_iter()))
                }
            }
        }

        fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            match self {
                Self::None => visitor.visit_none(),
                value => visitor.visit_some(value),
            }
        }

        fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            match self {
                Self::Sequence(values) => {
                    visitor.visit_seq(SeqDeserializer::<_, ValueError>::new(values.into_iter()))
                }
                _ => Err(de::Error::custom("expected state sequence")),
            }
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
            string bytes byte_buf unit unit_struct newtype_struct tuple tuple_struct
            map struct enum identifier ignored_any
        }
    }

    #[derive(Debug)]
    struct ProjectionError;

    impl std::fmt::Display for ProjectionError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("unsupported test projection")
        }
    }

    impl std::error::Error for ProjectionError {}

    impl ser::Error for ProjectionError {
        fn custom<T>(_message: T) -> Self
        where
            T: std::fmt::Display,
        {
            Self
        }
    }

    struct ProjectionSerializer;

    impl ser::Serializer for ProjectionSerializer {
        type Ok = Vec<(String, StateValue)>;
        type Error = ProjectionError;
        type SerializeSeq = Impossible<Self::Ok, Self::Error>;
        type SerializeTuple = Impossible<Self::Ok, Self::Error>;
        type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
        type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
        type SerializeMap = Impossible<Self::Ok, Self::Error>;
        type SerializeStruct = ProjectionFields;
        type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

        fn serialize_struct(
            self,
            _name: &'static str,
            _length: usize,
        ) -> Result<Self::SerializeStruct, Self::Error> {
            Ok(ProjectionFields { fields: Vec::new() })
        }

        fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_str(self, _value: &str) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
        where
            T: ?Sized + Serialize,
        {
            Err(ProjectionError)
        }

        fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_unit_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
        ) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_newtype_struct<T>(
            self,
            _name: &'static str,
            _value: &T,
        ) -> Result<Self::Ok, Self::Error>
        where
            T: ?Sized + Serialize,
        {
            Err(ProjectionError)
        }

        fn serialize_newtype_variant<T>(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _value: &T,
        ) -> Result<Self::Ok, Self::Error>
        where
            T: ?Sized + Serialize,
        {
            Err(ProjectionError)
        }

        fn serialize_seq(self, _length: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_tuple(self, _length: usize) -> Result<Self::SerializeTuple, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_tuple_struct(
            self,
            _name: &'static str,
            _length: usize,
        ) -> Result<Self::SerializeTupleStruct, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _length: usize,
        ) -> Result<Self::SerializeTupleVariant, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_map(self, _length: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_struct_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _length: usize,
        ) -> Result<Self::SerializeStructVariant, Self::Error> {
            Err(ProjectionError)
        }
    }

    struct ProjectionFields {
        fields: Vec<(String, StateValue)>,
    }

    impl SerializeStruct for ProjectionFields {
        type Ok = Vec<(String, StateValue)>;
        type Error = ProjectionError;

        fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize,
        {
            self.fields
                .push((key.to_owned(), value.serialize(StateValueSerializer)?));
            Ok(())
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            Ok(self.fields)
        }
    }

    struct StateValueSerializer;

    impl ser::Serializer for StateValueSerializer {
        type Ok = StateValue;
        type Error = ProjectionError;
        type SerializeSeq = StateSequence;
        type SerializeTuple = Impossible<Self::Ok, Self::Error>;
        type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
        type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
        type SerializeMap = Impossible<Self::Ok, Self::Error>;
        type SerializeStruct = Impossible<Self::Ok, Self::Error>;
        type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

        fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
            Ok(StateValue::U16(value))
        }

        fn serialize_seq(self, length: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            Ok(StateSequence {
                values: Vec::with_capacity(length.unwrap_or(0)),
            })
        }

        fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
            Ok(StateValue::Bool(value))
        }

        fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_str(self, _value: &str) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
            Ok(StateValue::None)
        }

        fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
        where
            T: ?Sized + Serialize,
        {
            value.serialize(self)
        }

        fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_unit_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
        ) -> Result<Self::Ok, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_newtype_struct<T>(
            self,
            _name: &'static str,
            _value: &T,
        ) -> Result<Self::Ok, Self::Error>
        where
            T: ?Sized + Serialize,
        {
            Err(ProjectionError)
        }

        fn serialize_newtype_variant<T>(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _value: &T,
        ) -> Result<Self::Ok, Self::Error>
        where
            T: ?Sized + Serialize,
        {
            Err(ProjectionError)
        }

        fn serialize_tuple(self, _length: usize) -> Result<Self::SerializeTuple, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_tuple_struct(
            self,
            _name: &'static str,
            _length: usize,
        ) -> Result<Self::SerializeTupleStruct, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _length: usize,
        ) -> Result<Self::SerializeTupleVariant, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_map(self, _length: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_struct(
            self,
            _name: &'static str,
            _length: usize,
        ) -> Result<Self::SerializeStruct, Self::Error> {
            Err(ProjectionError)
        }

        fn serialize_struct_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _length: usize,
        ) -> Result<Self::SerializeStructVariant, Self::Error> {
            Err(ProjectionError)
        }
    }

    struct StateSequence {
        values: Vec<StateValue>,
    }

    impl SerializeSeq for StateSequence {
        type Ok = StateValue;
        type Error = ProjectionError;

        fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize,
        {
            self.values.push(value.serialize(StateValueSerializer)?);
            Ok(())
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            Ok(StateValue::Sequence(self.values))
        }
    }

    // Serde is caller-format-neutral, so this focused test format projects the
    // complete empty-current shape plus selected scalar and version-vector values
    // through the actual derives without adding a direct dev dependency.
    #[test]
    fn current_reusable_state_shape_survives_serde_round_trip() {
        let mut original = ReusableState::new();
        original.bpd_version = 57;
        original.upd_version = 1;
        original.advertised_balance_versions = vec![5];
        original.advertised_tan_media_versions = vec![4, 2];
        original.depot_position_versions = vec![6, 5];
        original.camt_storage_period_days = Some(90);
        original.securities_transactions_storage_period_days = Some(60);
        original.credit_card_transactions_storage_period_days = Some(30);
        original
            .advertised_parameter_segments
            .push(ParameterSegmentAdvertisement::new("HISALS".to_owned(), 5));

        let projected = original.serialize(ProjectionSerializer).unwrap();
        assert!(
            !projected
                .iter()
                .any(|(field, _)| field == "advertised_parameter_segments")
        );
        assert!(projected.iter().any(|(field, value)| field == "version"
            && matches!(value, StateValue::U16(REUSABLE_STATE_VERSION))));
        let restored = ReusableState::deserialize(MapDeserializer::<_, ValueError>::new(
            projected.into_iter(),
        ))
        .unwrap();

        assert_eq!(restored.version(), REUSABLE_STATE_VERSION);
        assert_eq!(restored.bpd_version(), 57);
        assert_eq!(restored.upd_version(), 1);
        assert!(restored.balance_versions.is_empty());
        assert_eq!(
            restored
                .advertised_capabilities()
                .balance()
                .advertised_versions(),
            [5]
        );
        assert!(
            restored
                .advertised_capabilities()
                .balance()
                .supports_version(5)
        );
        assert_eq!(restored.advertised_tan_media_versions(), [4, 2]);
        let capabilities = restored.advertised_capabilities();
        assert_eq!(capabilities.depot_positions().advertised_versions(), [6, 5]);
        assert!(capabilities.depot_positions().supports_version(5));
        assert_eq!(
            capabilities.camt_cash_transactions().storage_period_days(),
            Some(90)
        );
        assert_eq!(
            capabilities.depot_transactions().storage_period_days(),
            Some(60)
        );
        assert_eq!(
            capabilities
                .credit_card_transactions()
                .storage_period_days(),
            Some(30)
        );
        assert!(capabilities.parameter_segments().is_empty());
    }

    #[test]
    fn reusable_state_rejects_unknown_serialized_fields() {
        let mut fields = ReusableState::new()
            .serialize(ProjectionSerializer)
            .unwrap();
        fields.push(("future_field".to_owned(), StateValue::Bool(false)));

        assert!(
            ReusableState::deserialize(MapDeserializer::<_, ValueError>::new(fields.into_iter()))
                .is_err()
        );
    }

    #[test]
    fn current_reusable_state_requires_formerly_defaulted_fields() {
        let mut fields = ReusableState::new()
            .serialize(ProjectionSerializer)
            .unwrap();
        fields.retain(|(field, _)| field != "advertised_balance_versions");

        assert!(
            ReusableState::deserialize(MapDeserializer::<_, ValueError>::new(fields.into_iter()))
                .is_err()
        );
    }

    #[test]
    fn current_account_shape_requires_upd_usage_semantics() {
        fn fields(include_usage: bool) -> Vec<(&'static str, StateValue)> {
            let mut fields = vec![
                ("iban", StateValue::None),
                ("bic", StateValue::None),
                ("account_number", StateValue::None),
                ("subaccount", StateValue::None),
                ("institute", StateValue::None),
                ("currency", StateValue::None),
                ("account_type", StateValue::None),
                ("owner_name_1", StateValue::None),
                ("owner_name_2", StateValue::None),
                ("product_name", StateValue::None),
                ("allowed_operations", StateValue::Sequence(Vec::new())),
            ];
            if include_usage {
                fields.push(("unlisted_operations_unknown", StateValue::Bool(false)));
            }
            fields
        }

        assert!(
            Account::deserialize(MapDeserializer::<_, ValueError>::new(
                fields(true).into_iter()
            ))
            .is_ok()
        );
        assert!(
            Account::deserialize(MapDeserializer::<_, ValueError>::new(
                fields(false).into_iter()
            ))
            .is_err()
        );
    }
}
