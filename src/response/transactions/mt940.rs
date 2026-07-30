use chrono::{Datelike, NaiveDate};

use crate::{
    error::{Error, malformed_transaction_data},
    model::{Amount, BookedEntry, BookedTransactionDetail, CreditDebit, StatementPosition},
};

use super::MAX_TRANSACTION_PAGE_ENTRIES;

pub(super) fn parse(input: &[u8]) -> Result<Vec<BookedEntry>, Error> {
    let text = encoding_rs::mem::decode_latin1(input);
    // DK Anlage 3 v3.8, 8.1 requires CRLF separators and the final "-" record.
    // One transport-style CRLF after the terminator is an owner-ratified tolerance.
    let text = text.strip_suffix("\r\n").unwrap_or(&text);
    if !text.starts_with("\r\n") || !text.ends_with("\r\n-") {
        return Err(malformed_transaction_data!());
    }
    let mut statements = Vec::new();
    let mut fields = Vec::new();
    for line in text.split("\r\n").skip(1) {
        if line == "-" {
            if fields.is_empty() {
                return Err(malformed_transaction_data!());
            }
            statements.push(std::mem::take(&mut fields));
            continue;
        }
        if line.is_empty() {
            // DK 8.1 permits empty separator records between logical fields.
            continue;
        }
        if let Some((tag, value)) = parse_tag(line) {
            fields.push(Field {
                tag: tag.to_owned(),
                value: value.to_owned(),
            });
        } else {
            let current = fields.last_mut().ok_or(malformed_transaction_data!())?;
            current.value.push('\n');
            current.value.push_str(line);
        }
    }
    if !fields.is_empty() || statements.is_empty() {
        return Err(malformed_transaction_data!());
    }

    let mut entries = Vec::new();
    for statement in statements {
        entries.extend(parse_statement(&statement)?);
        if entries.len() > MAX_TRANSACTION_PAGE_ENTRIES {
            return Err(malformed_transaction_data!());
        }
    }
    Ok(entries)
}

struct Field {
    tag: String,
    value: String,
}

fn parse_tag(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix(':')?;
    let end = rest.find(':')?;
    let tag = &rest[..end];
    if tag.is_empty()
        || tag.len() > 3
        || !tag
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return None;
    }
    Some((tag, &rest[end + 1..]))
}

fn parse_statement(fields: &[Field]) -> Result<Vec<BookedEntry>, Error> {
    let statement_position = {
        let mut statements = fields.iter().filter(|field| field.tag == "28C");
        match (statements.next(), statements.next()) {
            (Some(statement), None) => parse_statement_number(&statement.value).ok(),
            _ => None,
        }
    };
    let opening = fields
        .iter()
        .find(|field| matches!(field.tag.as_str(), "60F" | "60M"))
        .ok_or(malformed_transaction_data!())?;
    let currency = parse_balance_currency(&opening.value)?;

    let mut entries = Vec::new();
    let mut index = 0_u32;
    let mut cursor = 0;
    while cursor < fields.len() {
        if fields[cursor].tag != "61" {
            cursor += 1;
            continue;
        }
        index = index.checked_add(1).ok_or(malformed_transaction_data!())?;
        let information = fields
            .get(cursor + 1)
            .filter(|field| field.tag == "86")
            .map(|field| field.value.as_str());
        let parsed = parse_entry(&fields[cursor].value, &currency)?;
        let structured = information
            .map(parse_information)
            .transpose()?
            .unwrap_or_default();
        let entry_statement_position = (parsed.bank_reference.is_none())
            .then_some(statement_position.as_ref())
            .flatten();
        entries.push(BookedEntry {
            amount: parsed.amount,
            direction: parsed.direction,
            booking_date: parsed.booking_date,
            value_date: Some(parsed.value_date),
            reversal: Some(parsed.reversal),
            entry_reference: None,
            account_servicer_reference: parsed.bank_reference,
            bank_transaction_code: Some(parsed.booking_code),
            proprietary_transaction_code: structured.transaction_code.clone(),
            statement_position: entry_statement_position.map(|(statement_number, page_number)| {
                StatementPosition::new(statement_number.clone(), page_number.clone(), index)
            }),
            details: vec![BookedTransactionDetail {
                amount: None,
                direction: None,
                customer_reference: useful_reference(parsed.customer_reference),
                end_to_end_reference: useful_reference(structured.end_to_end_reference),
                mandate_reference: useful_reference(structured.mandate_reference),
                creditor_reference: useful_reference(structured.creditor_reference),
                account_servicer_reference: None,
                bank_transaction_code: None,
                proprietary_transaction_code: structured.transaction_code,
                counterparty_name: structured.counterparty_name,
                counterparty_account: structured.counterparty_account,
                remittance_information: structured.remittance_information,
            }],
        });
        cursor += if information.is_some() { 2 } else { 1 };
    }
    Ok(entries)
}

struct ParsedEntry {
    amount: Amount,
    direction: CreditDebit,
    reversal: bool,
    value_date: NaiveDate,
    booking_date: Option<NaiveDate>,
    booking_code: String,
    customer_reference: Option<String>,
    bank_reference: Option<String>,
}

fn parse_entry(value: &str, currency: &str) -> Result<ParsedEntry, Error> {
    let value_date = value.get(..6).ok_or(malformed_transaction_data!())?;
    let value_date = parse_short_date(value_date)?;
    let mut rest = value.get(6..).ok_or(malformed_transaction_data!())?;
    let booking_date = if rest
        .get(..4)
        .is_some_and(|date| date.bytes().all(|byte| byte.is_ascii_digit()))
    {
        let date = rest.get(..4).ok_or(malformed_transaction_data!())?;
        rest = rest.get(4..).ok_or(malformed_transaction_data!())?;
        Some(parse_month_day(date, value_date)?)
    } else {
        None
    };
    let (direction, reversal, after_direction) = if let Some(rest) = rest.strip_prefix("RC") {
        (CreditDebit::Credit, true, rest)
    } else if let Some(rest) = rest.strip_prefix("RD") {
        (CreditDebit::Debit, true, rest)
    } else if let Some(rest) = rest.strip_prefix('C') {
        (CreditDebit::Credit, false, rest)
    } else if let Some(rest) = rest.strip_prefix('D') {
        (CreditDebit::Debit, false, rest)
    } else {
        return Err(malformed_transaction_data!());
    };
    rest = after_direction;
    if rest.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) {
        rest = rest.get(1..).ok_or(malformed_transaction_data!())?;
    }
    let separator = rest.find('N').ok_or(malformed_transaction_data!())?;
    let amount = parse_amount(
        rest.get(..separator).ok_or(malformed_transaction_data!())?,
        currency,
    )?;
    rest = rest
        .get(separator + 1..)
        .ok_or(malformed_transaction_data!())?;
    let booking_code = rest
        .get(..3)
        .filter(|code| {
            code.len() == 3
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        })
        .ok_or(malformed_transaction_data!())?
        .to_owned();
    rest = rest.get(3..).ok_or(malformed_transaction_data!())?;
    // DK Anlage 3 v3.8, 8.2.3 defines a possible supplementary second line
    // after subfield 9. Gate 2 deliberately preserves only the first-line
    // customer and bank references that have typed result fields.
    let first_line = rest.split('\n').next().unwrap_or_default();
    let (customer_reference, bank_reference) = match first_line.split_once("//") {
        Some((customer, bank)) => {
            if bank.is_empty() {
                return Err(malformed_transaction_data!());
            }
            (nonempty_optional(customer), Some(bank.to_owned()))
        }
        None => {
            if first_line.is_empty() {
                return Err(malformed_transaction_data!());
            }
            (Some(first_line.to_owned()), None)
        }
    };
    Ok(ParsedEntry {
        amount,
        direction,
        reversal,
        value_date,
        booking_date,
        booking_code,
        customer_reference,
        bank_reference,
    })
}

#[derive(Default)]
struct StructuredInformation {
    transaction_code: Option<String>,
    end_to_end_reference: Option<String>,
    mandate_reference: Option<String>,
    creditor_reference: Option<String>,
    counterparty_name: Option<String>,
    counterparty_account: Option<String>,
    remittance_information: Vec<String>,
}

fn parse_information(value: &str) -> Result<StructuredInformation, Error> {
    let compact = value.replace('\n', "");
    let Some(transaction_code) = compact
        .get(..3)
        .filter(|code| code.bytes().all(|byte| byte.is_ascii_digit()))
    else {
        return Ok(StructuredInformation {
            remittance_information: vec![value.to_owned()],
            ..StructuredInformation::default()
        });
    };
    let mut result = StructuredInformation {
        transaction_code: Some(transaction_code.to_owned()),
        ..StructuredInformation::default()
    };
    let subfields = compact.get(3..).ok_or(malformed_transaction_data!())?;
    if !subfields.starts_with('?') {
        result.remittance_information.push(value.to_owned());
        return Ok(result);
    }
    let fields = split_control_fields(subfields)?;
    let mut remittance = String::new();
    let mut names = String::new();
    for (code, value) in fields {
        match code {
            20..=29 | 60..=63 => remittance.push_str(value),
            31 => result.counterparty_account = nonempty_optional(value),
            32 | 33 => names.push_str(value),
            _ => {}
        }
    }
    if !names.is_empty() {
        result.counterparty_name = Some(names);
    }
    if !remittance.is_empty() {
        result.end_to_end_reference = extract_labeled(&remittance, "EREF+");
        result.mandate_reference = extract_labeled(&remittance, "MREF+");
        result.creditor_reference = extract_labeled(&remittance, "CRED+");
        result.remittance_information.push(remittance);
    }
    Ok(result)
}

fn split_control_fields(value: &str) -> Result<Vec<(u8, &str)>, Error> {
    let mut fields = Vec::new();
    let mut cursor = 0;
    while cursor < value.len() {
        if value.as_bytes().get(cursor) != Some(&b'?') {
            return Err(malformed_transaction_data!());
        }
        let code = value
            .get(cursor + 1..cursor + 3)
            .ok_or(malformed_transaction_data!())?
            .parse()
            .map_err(|_| malformed_transaction_data!())?;
        let start = cursor + 3;
        let end = value[start..]
            .find('?')
            .map_or(value.len(), |offset| start + offset);
        fields.push((code, &value[start..end]));
        cursor = end;
    }
    Ok(fields)
}

fn extract_labeled(value: &str, label: &str) -> Option<String> {
    let start = value.find(label)? + label.len();
    let tail = &value[start..];
    let end = [
        "EREF+", "KREF+", "MREF+", "CRED+", "DEBT+", "COAM+", "OAMT+", "SVWZ+", "ABWA+", "ABWE+",
    ]
    .iter()
    .filter_map(|next| tail.find(next))
    .min()
    .unwrap_or(tail.len());
    nonempty_optional(tail[..end].trim())
}

fn parse_statement_number(value: &str) -> Result<(String, Option<String>), Error> {
    let (statement, page) = value
        .split_once('/')
        .map_or((value, None), |(statement, page)| (statement, Some(page)));
    if !valid_number(statement, 5) || page.is_some_and(|page| !valid_number(page, 5)) {
        return Err(malformed_transaction_data!());
    }
    Ok((statement.to_owned(), page.map(str::to_owned)))
}

fn parse_balance_currency(value: &str) -> Result<String, Error> {
    let after_direction = value.get(1..).ok_or(malformed_transaction_data!())?;
    after_direction
        .as_bytes()
        .windows(3)
        .position(|window| window.iter().all(u8::is_ascii_uppercase))
        .and_then(|start| after_direction.get(start..start + 3))
        .map(str::to_owned)
        .ok_or(malformed_transaction_data!())
}

fn parse_amount(value: &str, currency: &str) -> Result<Amount, Error> {
    if value.len() > 15 {
        return Err(malformed_transaction_data!());
    }
    let (integer, fraction) = value.split_once(',').ok_or(malformed_transaction_data!())?;
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(malformed_transaction_data!());
    }
    let coefficient = format!("{integer}{fraction}")
        .parse()
        .map_err(|_| malformed_transaction_data!())?;
    let scale = u8::try_from(fraction.len()).map_err(|_| malformed_transaction_data!())?;
    Ok(Amount::new(coefficient, scale, currency.to_owned()))
}

fn parse_short_date(value: &str) -> Result<NaiveDate, Error> {
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(malformed_transaction_data!());
    }
    // DK Anlage 3 v3.8, 8.2.1 encodes the year as two-digit JJ. The fixed
    // 80/79 pivot makes that underspecified wire value deterministic.
    let year: i32 = value[..2]
        .parse()
        .map_err(|_| malformed_transaction_data!())?;
    let year = if year > 79 { 1900 + year } else { 2000 + year };
    let month = value[2..4]
        .parse()
        .map_err(|_| malformed_transaction_data!())?;
    let day = value[4..6]
        .parse()
        .map_err(|_| malformed_transaction_data!())?;
    NaiveDate::from_ymd_opt(year, month, day).ok_or(malformed_transaction_data!())
}

fn parse_month_day(value: &str, value_date: NaiveDate) -> Result<NaiveDate, Error> {
    let month: u32 = value
        .get(..2)
        .ok_or(malformed_transaction_data!())?
        .parse()
        .map_err(|_| malformed_transaction_data!())?;
    let day: u32 = value
        .get(2..)
        .ok_or(malformed_transaction_data!())?
        .parse()
        .map_err(|_| malformed_transaction_data!())?;
    // DK Anlage 3 v3.8, 8.2.1 supplies the optional booking date as MMTT.
    // Resolve only the adjacent Dec/Jan rollover relative to the full value date.
    let year = match (value_date.month(), month) {
        (12, 1) => value_date.year() + 1,
        (1, 12) => value_date.year() - 1,
        _ => value_date.year(),
    };
    NaiveDate::from_ymd_opt(year, month, day).ok_or(malformed_transaction_data!())
}

fn useful_reference(value: Option<String>) -> Option<String> {
    value.filter(|value| !matches!(value.as_str(), "NOTPROVIDED" | "NONREF"))
}

fn nonempty_optional(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn valid_number(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && value.bytes().all(|byte| byte.is_ascii_digit())
}
