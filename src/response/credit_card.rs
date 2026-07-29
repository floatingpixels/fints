use chrono::NaiveDate;

use crate::{
    error::Error,
    model::{
        Amount, CreditCardAmount, CreditCardBalance, CreditCardCurrentBalance, CreditCardEntry,
        CreditDebit, Timestamp,
    },
    wire::{Segment, Value},
};

use super::{component, optional_component};

const MAX_PAGE_ENTRIES: usize = 5_000;

pub(crate) struct CreditCardTransactionPage {
    pub(crate) reported_card_number: String,
    pub(crate) reported_account_id: Option<String>,
    pub(crate) current_balance: Option<CreditCardCurrentBalance>,
    pub(crate) last_statement_date: Option<NaiveDate>,
    pub(crate) next_statement_date: Option<NaiveDate>,
    pub(crate) entries: Vec<CreditCardEntry>,
}

pub(super) fn transactions(
    segments: &[Segment],
) -> Result<Option<CreditCardTransactionPage>, Error> {
    let Some(segment) = unique_segment(segments, b"HIKKU")? else {
        return Ok(None);
    };
    require_version(segment, "HIKKU", 1)?;
    let elements = segment.elements();
    let reported_card_number = single_text(elements, 1, "HIKKU card number")?;
    let reported_account_id = optional_single_text(elements, 2)?;
    let current_balance = optional_element(elements, 3)
        .map(parse_signed_balance)
        .transpose()?;
    let last_statement_date = optional_single_text(elements, 4)?
        .map(|value| parse_date(&value))
        .transpose()?;
    let next_statement_date = optional_single_text(elements, 5)?
        .map(|value| parse_date(&value))
        .transpose()?;
    let mut entries = Vec::new();
    for element in elements.iter().skip(6) {
        // CR0538 C.12.1 defines each repeated DEG as an entry. A trailing
        // all-empty DEG is therefore malformed rather than ignorable padding.
        entries.push(parse_entry(element.components())?);
        if entries.len() > MAX_PAGE_ENTRIES {
            return Err(Error::InvalidResponse {
                structure: "too many credit-card entries",
            });
        }
    }
    Ok(Some(CreditCardTransactionPage {
        reported_card_number,
        reported_account_id,
        current_balance,
        last_statement_date,
        next_statement_date,
        entries,
    }))
}

pub(super) fn balance(segments: &[Segment]) -> Result<Option<CreditCardBalanceFields>, Error> {
    let Some(segment) = unique_segment(segments, b"HIKKS")? else {
        return Ok(None);
    };
    require_version(segment, "HIKKS", 1)?;
    let elements = segment.elements();
    Ok(Some(CreditCardBalanceFields {
        reported_card_number: single_text(elements, 1, "HIKKS card number")?,
        reported_account_id: optional_single_text(elements, 2)?,
        current: parse_signed_balance(required_element(elements, 3, "HIKKS current balance")?)?,
        available: optional_element(elements, 4)
            .map(parse_amount_with_direction)
            .transpose()?,
        open_authorizations: optional_element(elements, 5)
            .map(parse_amount)
            .transpose()?,
        credit_limit: optional_element(elements, 6)
            .map(parse_amount)
            .transpose()?,
        next_statement_date: optional_single_text(elements, 7)?
            .map(|value| parse_date(&value))
            .transpose()?,
    }))
}

pub(crate) struct CreditCardBalanceFields {
    pub(crate) reported_card_number: String,
    pub(crate) reported_account_id: Option<String>,
    pub(crate) current: CreditCardCurrentBalance,
    pub(crate) available: Option<CreditCardAmount>,
    pub(crate) open_authorizations: Option<Amount>,
    pub(crate) credit_limit: Option<Amount>,
    pub(crate) next_statement_date: Option<NaiveDate>,
}

impl CreditCardBalanceFields {
    pub(crate) fn with_account(self, account: crate::model::Account) -> CreditCardBalance {
        CreditCardBalance {
            account,
            reported_card_number: self.reported_card_number,
            reported_account_id: self.reported_account_id,
            current: self.current,
            available: self.available,
            open_authorizations: self.open_authorizations,
            credit_limit: self.credit_limit,
            next_statement_date: self.next_statement_date,
        }
    }
}

fn parse_entry(components: &[Value]) -> Result<CreditCardEntry, Error> {
    // G112 / CR 538, "Umsatz Kreditkartenkonto". Components are positional;
    // the four repeated "Transaktionsbeschreibung" DEGs occupy eight flat
    // component slots. Absent trailing optional values remain absent.
    if components.len() > 29 {
        return Err(Error::InvalidResponse {
            structure: "credit-card entry components",
        });
    }
    Ok(CreditCardEntry {
        card_number: component(components, 0, "credit-card entry card number")?,
        receipt_date: parse_date(&component(components, 1, "credit-card receipt date")?)?,
        booking_date: parse_date(&component(components, 2, "credit-card booking date")?)?,
        statement_date: optional_component(components, 3)
            .map(|value| parse_date(&value))
            .transpose()?,
        value_date: optional_component(components, 4)
            .map(|value| parse_date(&value))
            .transpose()?,
        original_amount: optional_amount_with_direction(components, 5)?,
        exchange_rate: optional_component(components, 8)
            .map(|value| decimal(&value, "credit-card exchange rate"))
            .transpose()?,
        booking_amount: required_amount_with_direction(components, 9)?,
        description: (12..20)
            .filter_map(|index| optional_component(components, index))
            .collect(),
        country: optional_component(components, 20),
        merchant_name: optional_component(components, 21),
        terminal: optional_component(components, 22),
        billed: optional_component(components, 23)
            .map(|value| parse_bool(&value))
            .transpose()?,
        booking_reference: optional_component(components, 24),
        fee_code: optional_component(components, 25),
        statement_marker: optional_component(components, 26),
        cash_fee: optional_component(components, 27),
        foreign_use_fee: optional_component(components, 28),
    })
}

fn parse_signed_balance(components: &[Value]) -> Result<CreditCardCurrentBalance, Error> {
    if !(4..=5).contains(&components.len()) {
        return Err(Error::InvalidResponse {
            structure: "credit-card current balance",
        });
    }
    let direction = parse_direction(&component(components, 0, "credit-card balance direction")?)?;
    let amount = amount(
        component(components, 1, "credit-card balance amount")?,
        component(components, 2, "credit-card balance currency")?,
        "credit-card balance amount",
    )?;
    let date = parse_date(&component(components, 3, "credit-card balance date")?)?;
    let time = optional_component(components, 4)
        .map(|value| parse_time(&value))
        .transpose()?;
    Ok(CreditCardCurrentBalance {
        amount: CreditCardAmount { amount, direction },
        timestamp: Timestamp::new(date, time),
    })
}

fn parse_amount_with_direction(components: &[Value]) -> Result<CreditCardAmount, Error> {
    if components.len() != 3 {
        return Err(Error::InvalidResponse {
            structure: "credit-card amount with direction",
        });
    }
    let amount = amount(
        component(components, 0, "credit-card amount")?,
        component(components, 1, "credit-card currency")?,
        "credit-card amount",
    )?;
    let direction = parse_direction(&component(components, 2, "credit-card amount direction")?)?;
    Ok(CreditCardAmount { amount, direction })
}

fn parse_amount(components: &[Value]) -> Result<Amount, Error> {
    if components.len() != 2 {
        return Err(Error::InvalidResponse {
            structure: "credit-card amount",
        });
    }
    amount(
        component(components, 0, "credit-card amount")?,
        component(components, 1, "credit-card currency")?,
        "credit-card amount",
    )
}

fn optional_amount_with_direction(
    components: &[Value],
    index: usize,
) -> Result<Option<CreditCardAmount>, Error> {
    let values = (
        optional_component(components, index),
        optional_component(components, index + 1),
        optional_component(components, index + 2),
    );
    match values {
        (None, None, None) => Ok(None),
        (Some(value), Some(currency), Some(direction)) => Ok(Some(CreditCardAmount {
            amount: amount(value, currency, "credit-card original amount")?,
            direction: parse_direction(&direction)?,
        })),
        _ => Err(Error::InvalidResponse {
            structure: "partial credit-card original amount",
        }),
    }
}

fn required_amount_with_direction(
    components: &[Value],
    index: usize,
) -> Result<CreditCardAmount, Error> {
    Ok(CreditCardAmount {
        amount: amount(
            component(components, index, "credit-card booking amount")?,
            component(components, index + 1, "credit-card booking currency")?,
            "credit-card booking amount",
        )?,
        direction: parse_direction(&component(
            components,
            index + 2,
            "credit-card booking direction",
        )?)?,
    })
}

fn amount(value: String, currency: String, field: &'static str) -> Result<Amount, Error> {
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(Error::InvalidValue {
            field: "credit-card currency",
        });
    }
    let (coefficient, scale) = decimal(&value, field)?;
    Ok(Amount::new(coefficient, scale, currency))
}

fn decimal(value: &str, field: &'static str) -> Result<(u128, u8), Error> {
    if value.len() > 24 {
        return Err(Error::InvalidValue { field });
    }
    let (integer, fraction) = value.split_once(',').ok_or(Error::InvalidValue { field })?;
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::InvalidValue { field });
    }
    let coefficient = format!("{integer}{fraction}")
        .parse()
        .map_err(|_| Error::InvalidValue { field })?;
    let scale = u8::try_from(fraction.len()).map_err(|_| Error::InvalidValue { field })?;
    Ok((coefficient, scale))
}

fn parse_direction(value: &str) -> Result<CreditDebit, Error> {
    match value {
        "C" => Ok(CreditDebit::Credit),
        "D" => Ok(CreditDebit::Debit),
        _ => Err(Error::InvalidValue {
            field: "credit-card debit/credit direction",
        }),
    }
}

fn parse_bool(value: &str) -> Result<bool, Error> {
    match value {
        "J" => Ok(true),
        "N" => Ok(false),
        _ => Err(Error::InvalidValue {
            field: "credit-card billed indicator",
        }),
    }
}

fn parse_date(value: &str) -> Result<NaiveDate, Error> {
    NaiveDate::parse_from_str(value, "%Y%m%d").map_err(|_| Error::InvalidValue {
        field: "credit-card date",
    })
}

fn parse_time(value: &str) -> Result<chrono::NaiveTime, Error> {
    chrono::NaiveTime::parse_from_str(value, "%H%M%S").map_err(|_| Error::InvalidValue {
        field: "credit-card time",
    })
}

fn unique_segment<'a>(segments: &'a [Segment], code: &[u8]) -> Result<Option<&'a Segment>, Error> {
    let mut matching = segments
        .iter()
        .filter(|segment| segment.header().is_some_and(|header| header.code == code));
    let value = matching.next();
    if matching.next().is_some() {
        return Err(Error::InvalidResponse {
            structure: "multiple credit-card response segments",
        });
    }
    Ok(value)
}

fn require_version(segment: &Segment, code: &'static str, version: u16) -> Result<(), Error> {
    let actual = segment.header().ok_or(Error::InvalidResponse {
        structure: "credit-card response header",
    })?;
    if actual.version == version {
        Ok(())
    } else {
        Err(Error::UnsupportedSegment {
            code,
            version: actual.version,
        })
    }
}

fn required_element<'a>(
    elements: &'a [crate::wire::Element],
    index: usize,
    field: &'static str,
) -> Result<&'a [Value], Error> {
    elements
        .get(index)
        .map(|element| element.components())
        .ok_or(Error::MissingValue { field })
}

fn optional_element(elements: &[crate::wire::Element], index: usize) -> Option<&[Value]> {
    elements
        .get(index)
        .map(|element| element.components())
        .filter(|components| {
            components
                .iter()
                .any(|value| value.as_text().is_some_and(|text| !text.is_empty()))
        })
}

fn single_text(
    elements: &[crate::wire::Element],
    index: usize,
    field: &'static str,
) -> Result<String, Error> {
    let components = required_element(elements, index, field)?;
    if components.len() != 1 {
        return Err(Error::InvalidResponse {
            structure: "single credit-card text element",
        });
    }
    component(components, 0, field)
}

fn optional_single_text(
    elements: &[crate::wire::Element],
    index: usize,
) -> Result<Option<String>, Error> {
    let Some(components) = optional_element(elements, index) else {
        return Ok(None);
    };
    if components.len() != 1 {
        return Err(Error::InvalidResponse {
            structure: "single optional credit-card text element",
        });
    }
    Ok(optional_component(components, 0))
}
