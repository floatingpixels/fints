use chrono::{NaiveDate, NaiveTime};

use crate::{
    error::Error,
    model::{Account, Amount, Balance, CreditDebit, InstituteState, SignedAmount, Timestamp},
    wire::{Segment, Value},
};

use super::{component, optional_component};

pub(super) fn parse(segments: &[Segment]) -> Result<Option<Balance>, Error> {
    let Some(segment) = segments.iter().find(|segment| {
        segment
            .header()
            .is_some_and(|header| header.code == b"HISAL")
    }) else {
        return Ok(None);
    };
    let version = segment.header().expect("header checked").version;
    if !(5..=8).contains(&version) {
        return Err(Error::UnsupportedSegment {
            code: "HISAL",
            version,
        });
    }
    let elements = segment.elements();
    if elements.len() < 5 {
        return Err(Error::InvalidResponse {
            structure: "balance.HISAL.element_shape",
        });
    }

    let account = parse_account(elements[1].components(), version)?;
    let product_name = single_text(elements, 2, "account product name")?;
    let account_currency = parse_currency(&single_text(elements, 3, "account currency")?)?;
    let booked = parse_signed_amount(elements[4].components())?;
    let pending = optional_element(elements, 5)
        .map(parse_signed_amount)
        .transpose()?;
    let credit_line = optional_element(elements, 6)
        .map(parse_amount)
        .transpose()?;
    let available = optional_element(elements, 7)
        .map(parse_amount)
        .transpose()?;
    let already_drawn = optional_element(elements, 8)
        .map(parse_amount)
        .transpose()?;
    let (overdraft, booking_time, due_date) = if version == 5 {
        let booking_date = optional_single_element(elements, 9, "booking date")?
            .map(|value| parse_date(&value))
            .transpose()?;
        let booking_clock = optional_single_element(elements, 10, "booking time")?
            .map(|value| parse_time(&value))
            .transpose()?;
        let booking_time = match (booking_date, booking_clock) {
            (Some(date), time) => Some(Timestamp::new(date, time)),
            (None, _) => None,
        };
        let due_date = optional_single_element(elements, 11, "balance due date")?
            .map(|value| parse_date(&value))
            .transpose()?;
        (None, booking_time, due_date)
    } else {
        (
            optional_element(elements, 9)
                .map(parse_amount)
                .transpose()?,
            optional_element(elements, 10)
                .map(parse_timestamp)
                .transpose()?,
            optional_single_element(elements, 11, "balance due date")?
                .map(|value| parse_date(&value))
                .transpose()?,
        )
    };
    let garnishable_after_month_end = (version == 8)
        .then(|| optional_element(elements, 12))
        .flatten()
        .map(parse_amount)
        .transpose()?;

    Ok(Some(Balance {
        account,
        product_name,
        account_currency,
        booked,
        pending,
        credit_line,
        available,
        already_drawn,
        overdraft,
        booking_time,
        due_date,
        garnishable_after_month_end,
    }))
}

fn parse_account(components: &[Value], version: u16) -> Result<Account, Error> {
    if version <= 6 {
        if components.len() < 4 {
            return Err(Error::InvalidResponse {
                structure: "balance.national_account",
            });
        }
        let account_number = component(components, 0, "account number")?;
        let subaccount = optional_component(components, 1);
        let country_code = component(components, 2, "account country code")?;
        let institute_code = component(components, 3, "account institute code")?;
        return Ok(Account {
            iban: None,
            bic: None,
            account_number: Some(account_number),
            subaccount,
            institute: Some(InstituteState {
                country_code,
                institute_code,
            }),
            currency: None,
            account_type: None,
            owner_name_1: None,
            owner_name_2: None,
            product_name: None,
            allowed_operations: Vec::new(),
            unlisted_operations_unknown: false,
        });
    }

    let country_code = optional_component(components, 4);
    let institute_code = optional_component(components, 5);
    let institute = match (country_code, institute_code) {
        (Some(country_code), Some(institute_code)) => Some(InstituteState {
            country_code,
            institute_code,
        }),
        (None, None) => None,
        _ => {
            return Err(Error::InvalidResponse {
                structure: "balance.international_account",
            });
        }
    };
    let account = Account {
        iban: optional_component(components, 0),
        bic: optional_component(components, 1),
        account_number: optional_component(components, 2),
        subaccount: optional_component(components, 3),
        institute,
        currency: None,
        account_type: None,
        owner_name_1: None,
        owner_name_2: None,
        product_name: None,
        allowed_operations: Vec::new(),
        unlisted_operations_unknown: false,
    };
    if account.iban.is_none() && account.account_number.is_none() {
        return Err(Error::MissingValue {
            field: "international account identity",
        });
    }
    Ok(account)
}

fn parse_signed_amount(components: &[Value]) -> Result<SignedAmount, Error> {
    if components.len() < 4 {
        return Err(Error::InvalidResponse {
            structure: "balance.amount_group",
        });
    }
    let direction = match component(components, 0, "credit/debit sign")?.as_str() {
        "C" => CreditDebit::Credit,
        "D" => CreditDebit::Debit,
        _ => {
            return Err(Error::InvalidValue {
                field: "credit/debit sign",
            });
        }
    };
    let amount = parse_amount(&components[1..3])?;
    let date = parse_date(&component(components, 3, "balance date")?)?;
    let time = optional_component(components, 4)
        .map(|value| parse_time(&value))
        .transpose()?;
    Ok(SignedAmount::new(direction, amount, date, time))
}

fn parse_amount(components: &[Value]) -> Result<Amount, Error> {
    if components.len() < 2 {
        return Err(Error::InvalidResponse {
            structure: "balance.amount",
        });
    }
    let value = component(components, 0, "amount value")?;
    let currency = parse_currency(&component(components, 1, "amount currency")?)?;
    if value.len() > 15 {
        return Err(Error::InvalidValue {
            field: "amount value",
        });
    }
    let (integer, fraction) = value.split_once(',').ok_or(Error::InvalidValue {
        field: "amount value",
    })?;
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::InvalidValue {
            field: "amount value",
        });
    }
    let digits = format!("{integer}{fraction}");
    let coefficient = digits.parse().map_err(|_| Error::InvalidValue {
        field: "amount value",
    })?;
    let scale = u8::try_from(fraction.len()).map_err(|_| Error::InvalidValue {
        field: "amount value",
    })?;
    Ok(Amount::new(coefficient, scale, currency))
}

fn parse_currency(value: &str) -> Result<String, Error> {
    if value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        Ok(value.to_owned())
    } else {
        Err(Error::InvalidValue { field: "currency" })
    }
}

fn parse_timestamp(components: &[Value]) -> Result<Timestamp, Error> {
    if components.is_empty() {
        return Err(Error::InvalidResponse {
            structure: "balance.timestamp",
        });
    }
    let date = parse_date(&component(components, 0, "booking date")?)?;
    let time = optional_component(components, 1)
        .map(|value| parse_time(&value))
        .transpose()?;
    Ok(Timestamp::new(date, time))
}

fn parse_date(value: &str) -> Result<NaiveDate, Error> {
    NaiveDate::parse_from_str(value, "%Y%m%d").map_err(|_| Error::InvalidValue { field: "date" })
}

fn parse_time(value: &str) -> Result<NaiveTime, Error> {
    NaiveTime::parse_from_str(value, "%H%M%S").map_err(|_| Error::InvalidValue { field: "time" })
}

fn single_text(
    elements: &[crate::wire::Element],
    index: usize,
    field: &'static str,
) -> Result<String, Error> {
    elements
        .get(index)
        .map(|element| component(element.components(), 0, field))
        .transpose()?
        .ok_or(Error::MissingValue { field })
}

fn optional_element(elements: &[crate::wire::Element], index: usize) -> Option<&[Value]> {
    elements
        .get(index)
        .map(|element| element.components())
        .filter(|components| {
            components
                .first()
                .and_then(|value| value.as_text())
                .is_none_or(|value| !value.is_empty())
        })
}

fn optional_single_element(
    elements: &[crate::wire::Element],
    index: usize,
    field: &'static str,
) -> Result<Option<String>, Error> {
    let Some(components) = optional_element(elements, index) else {
        return Ok(None);
    };
    if components.is_empty() {
        return Err(Error::InvalidResponse { structure: field });
    }
    Ok(Some(component(components, 0, field)?))
}
