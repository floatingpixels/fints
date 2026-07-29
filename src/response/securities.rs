use chrono::{NaiveDate, NaiveTime};

use crate::{
    error::Error,
    model::{
        Amount, DepotPosition, PriceQuality, QuantityUnit, SecuritiesAmount, SecuritiesMovement,
        SecuritiesQuantity, SecuritiesTransaction, SecurityInstrument, SecurityPrice,
    },
    wire::Segment,
};

const MAX_FIELDS: usize = 10_000;
const MAX_BLOCK_DEPTH: usize = 12;
const MAX_PAGE_ENTRIES: usize = 5_000;

pub(crate) struct DepotPositionPage {
    pub(crate) more: bool,
    pub(crate) institute_code: String,
    pub(crate) account_number: String,
    pub(crate) positions: Vec<DepotPosition>,
    pub(crate) total_values: Vec<SecuritiesAmount>,
}

pub(crate) struct SecuritiesTransactionPage {
    pub(crate) more: bool,
    pub(crate) institute_code: String,
    pub(crate) account_number: String,
    pub(crate) entries: Vec<SecuritiesTransaction>,
}

pub(super) fn positions(segments: &[Segment]) -> Result<Option<DepotPositionPage>, Error> {
    let payload = response_binary(segments, b"HIWPD", "HIWPD", 6)?;
    payload.map(parse_positions).transpose()
}

pub(super) fn transactions(
    segments: &[Segment],
) -> Result<Option<SecuritiesTransactionPage>, Error> {
    let payload = response_binary(segments, b"HIWDU", "HIWDU", 5)?;
    payload.map(parse_transactions).transpose()
}

fn response_binary<'a>(
    segments: &'a [Segment],
    code: &[u8],
    code_text: &'static str,
    version: u16,
) -> Result<Option<&'a [u8]>, Error> {
    let mut matching = segments
        .iter()
        .filter(|segment| segment.header().is_some_and(|header| header.code == code));
    let Some(segment) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err(Error::InvalidResponse {
            structure: "multiple securities response segments",
        });
    }
    let header = segment.header().ok_or(Error::InvalidResponse {
        structure: "securities response header",
    })?;
    if header.version != version {
        return Err(Error::UnsupportedSegment {
            code: code_text,
            version: header.version,
        });
    }
    let components = segment
        .elements()
        .get(1)
        .ok_or(Error::MissingValue {
            field: "securities response payload",
        })?
        .components();
    if components.len() != 1 {
        return Err(Error::InvalidResponse {
            structure: "securities response payload",
        });
    }
    Ok(Some(components[0].as_binary().ok_or(
        Error::InvalidValue {
            field: "securities response payload",
        },
    )?))
}

#[derive(Clone)]
struct Field {
    tag: String,
    value: String,
}

struct Block {
    name: String,
    fields: Vec<Field>,
    children: Vec<Block>,
}

fn document(input: &[u8]) -> Result<Block, Error> {
    let text = encoding_rs::mem::decode_latin1(input);
    // DK Anlage 3 v3.9, chapter 4 general syntax rule 6: the record starts
    // with CRLF and ends with the final CRLF "-" record. One trailing CRLF
    // after that terminator is an owner-ratified transport tolerance.
    let text = text.strip_suffix("\r\n").unwrap_or(&text);
    let body = text
        .strip_prefix("\r\n")
        .and_then(|value| value.strip_suffix("\r\n-"))
        .ok_or(Error::MalformedSecuritiesData)?;
    let mut logical = Vec::<Field>::new();
    for line in body.split("\r\n") {
        if let Some(rest) = line.strip_prefix(':')
            && let Some(end) = rest.find(':')
        {
            let tag = &rest[..end];
            if !tag.is_empty()
                && tag.len() <= 3
                && tag
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            {
                if logical.len() >= MAX_FIELDS {
                    return Err(Error::MalformedSecuritiesData);
                }
                logical.push(Field {
                    tag: tag.to_owned(),
                    value: rest[end + 1..].to_owned(),
                });
                continue;
            }
        }
        let field = logical.last_mut().ok_or(Error::MalformedSecuritiesData)?;
        field.value.push('\n');
        field.value.push_str(line);
    }
    let mut root = Block {
        name: "ROOT".to_owned(),
        fields: Vec::new(),
        children: Vec::new(),
    };
    let mut cursor = 0;
    while cursor < logical.len() {
        if logical[cursor].tag != "16R" {
            return Err(Error::MalformedSecuritiesData);
        }
        root.children.push(parse_block(&logical, &mut cursor, 1)?);
    }
    if root.children.is_empty() {
        return Err(Error::MalformedSecuritiesData);
    }
    Ok(root)
}

fn parse_block(fields: &[Field], cursor: &mut usize, depth: usize) -> Result<Block, Error> {
    if depth > MAX_BLOCK_DEPTH {
        return Err(Error::MalformedSecuritiesData);
    }
    let start = fields.get(*cursor).ok_or(Error::MalformedSecuritiesData)?;
    if start.tag != "16R" || start.value.is_empty() {
        return Err(Error::MalformedSecuritiesData);
    }
    *cursor += 1;
    let mut block = Block {
        name: start.value.clone(),
        fields: Vec::new(),
        children: Vec::new(),
    };
    loop {
        let field = fields.get(*cursor).ok_or(Error::MalformedSecuritiesData)?;
        match field.tag.as_str() {
            "16R" => block.children.push(parse_block(fields, cursor, depth + 1)?),
            "16S" => {
                if field.value != block.name {
                    return Err(Error::MalformedSecuritiesData);
                }
                *cursor += 1;
                return Ok(block);
            }
            _ => {
                block.fields.push(field.clone());
                *cursor += 1;
            }
        }
    }
}

fn parse_positions(input: &[u8]) -> Result<DepotPositionPage, Error> {
    let root = document(input)?;
    validate_root(&root, true)?;
    let general = unique_child(&root, "GENL")?;
    let active = validate_general(general, false)?;
    let more = page_more(required_field(general, "28E")?)?;
    let (institute_code, account_number) = safe_identity(general)?;
    let positions = children(&root, "FIN")
        .map(parse_position)
        .collect::<Result<Vec<_>, _>>()?;
    if positions.len() > MAX_PAGE_ENTRIES {
        return Err(Error::MalformedSecuritiesData);
    }
    if active != !positions.is_empty() {
        return Err(Error::MalformedSecuritiesData);
    }
    let total_values = root
        .children
        .iter()
        .find(|block| block.name == "ADDINFO")
        .map(|block| {
            fields(block, "19A")
                .filter(|field| field.value.starts_with(":HOLP//"))
                .map(|field| parse_signed_amount(&field.value, ":HOLP//"))
                .collect()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(DepotPositionPage {
        more,
        institute_code,
        account_number,
        positions,
        total_values,
    })
}

fn parse_transactions(input: &[u8]) -> Result<SecuritiesTransactionPage, Error> {
    let root = document(input)?;
    validate_root(&root, false)?;
    let general = unique_child(&root, "GENL")?;
    let active = validate_general(general, true)?;
    let more = page_more(required_field(general, "28E")?)?;
    let (institute_code, account_number) = safe_identity(general)?;
    let mut entries = Vec::new();
    for financial in children(&root, "FIN") {
        if financial.children.len() != 1 || financial.children[0].name != "TRAN" {
            return Err(Error::MalformedSecuritiesData);
        }
        let instrument = parse_instrument(required_field(financial, "35B")?)?;
        let price = optional_field(financial, "90A")
            .or_else(|| optional_field(financial, "90B"))
            .map(|field| parse_price(&field.value, financial))
            .transpose()?;
        let transaction = unique_child(financial, "TRAN")?;
        entries.push(parse_transaction(transaction, &instrument, price.as_ref())?);
        if entries.len() > MAX_PAGE_ENTRIES {
            return Err(Error::MalformedSecuritiesData);
        }
    }
    if active != !entries.is_empty() {
        return Err(Error::MalformedSecuritiesData);
    }
    Ok(SecuritiesTransactionPage {
        more,
        institute_code,
        account_number,
        entries,
    })
}

fn safe_identity(block: &Block) -> Result<(String, String), Error> {
    let value = required_qualified(block, "97A", ":SAFE//")?
        .value
        .strip_prefix(":SAFE//")
        .ok_or(Error::MalformedSecuritiesData)?;
    let (institute, account) = value
        .split_once('/')
        .ok_or(Error::MalformedSecuritiesData)?;
    if institute.is_empty() || account.is_empty() {
        return Err(Error::MalformedSecuritiesData);
    }
    Ok((institute.to_owned(), account.to_owned()))
}

fn validate_root(root: &Block, positions: bool) -> Result<(), Error> {
    if root
        .children
        .first()
        .is_none_or(|block| block.name != "GENL")
    {
        return Err(Error::MalformedSecuritiesData);
    }
    let mut add_info_seen = false;
    for block in root.children.iter().skip(1) {
        match block.name.as_str() {
            "FIN" if !add_info_seen => {}
            "ADDINFO" if positions && !add_info_seen => add_info_seen = true,
            _ => return Err(Error::MalformedSecuritiesData),
        }
    }
    Ok(())
}

fn validate_general(block: &Block, transaction: bool) -> Result<bool, Error> {
    required_field(block, "28E")?;
    if required_qualified(block, "20C", ":SEME//")?.value != ":SEME//NONREF" {
        return Err(Error::MalformedSecuritiesData);
    }
    if required_field(block, "23G")?.value != "NEWM" {
        return Err(Error::MalformedSecuritiesData);
    }
    if transaction {
        if !fields(block, "69A")
            .chain(fields(block, "69B"))
            .any(|field| field.value.starts_with(":STAT//"))
        {
            return Err(Error::MalformedSecuritiesData);
        }
    } else {
        required_qualified_any(block, &["98A", "98C"], ":STAT//")?;
        if required_qualified(block, "22F", ":STTY//")?.value != ":STTY//CUST" {
            return Err(Error::MalformedSecuritiesData);
        }
    }
    required_qualified(block, "97A", ":SAFE//")?;
    match required_qualified(block, "17B", ":ACTI//")?
        .value
        .strip_prefix(":ACTI//")
    {
        Some("Y") => Ok(true),
        Some("N") => Ok(false),
        _ => Err(Error::MalformedSecuritiesData),
    }
}

fn parse_position(block: &Block) -> Result<DepotPosition, Error> {
    let instrument = parse_instrument(required_field(block, "35B")?)?;
    let quantity = parse_quantity(
        &required_qualified(block, "93B", ":AGGR//")?.value,
        ":AGGR//",
        true,
    )?;
    let price = optional_field(block, "90A")
        .or_else(|| optional_field(block, "90B"))
        .map(|field| parse_price(&field.value, block))
        .transpose()?;
    let market_values = fields(block, "19A")
        .filter(|field| field.value.starts_with(":HOLD//"))
        .map(|field| parse_signed_amount(&field.value, ":HOLD//"))
        .collect::<Result<Vec<_>, _>>()?;
    if children(block, "SUBBAL").next().is_none() {
        return Err(Error::MalformedSecuritiesData);
    }
    for child in &block.children {
        if child.name != "SUBBAL" || required_field(child, "93C").is_err() {
            return Err(Error::MalformedSecuritiesData);
        }
    }
    let cost_basis = optional_qualified(block, "70E", ":HOLD//")
        .map(|field| parse_cost_basis(&field.value))
        .transpose()?
        .flatten();
    Ok(DepotPosition {
        instrument,
        quantity,
        price,
        market_values,
        cost_basis,
    })
}

fn parse_transaction(
    block: &Block,
    instrument: &SecurityInstrument,
    price: Option<&SecurityPrice>,
) -> Result<SecuritiesTransaction, Error> {
    let valid_children = match block.children.as_slice() {
        [link] => link.name == "LINK",
        [link, details] => link.name == "LINK" && details.name == "TRANSDET",
        _ => false,
    };
    if !valid_children {
        return Err(Error::MalformedSecuritiesData);
    }
    let link = unique_child(block, "LINK")?;
    let reference = match required_qualified(link, "20C", ":RELA//")?
        .value
        .strip_prefix(":RELA//")
    {
        Some("NONREF") => None,
        Some(value) if !value.is_empty() => Some(value.to_owned()),
        _ => return Err(Error::MalformedSecuritiesData),
    };
    let mut parsed = optional_unique_child(block, "TRANSDET")?
        .map(parse_transaction_details)
        .transpose()?;
    Ok(SecuritiesTransaction {
        instrument: instrument.clone(),
        reference,
        quantity: parsed.as_ref().map(|details| details.quantity.clone()),
        price: price.cloned(),
        amount: parsed.as_mut().and_then(|details| details.amount.take()),
        accrued_interest: parsed
            .as_mut()
            .and_then(|details| details.accrued_interest.take()),
        transaction_kind: parsed
            .as_mut()
            .and_then(|details| details.transaction_kind.take()),
        movement: parsed.as_ref().map(|details| details.movement),
        effective_date: parsed.as_ref().map(|details| details.effective_date),
        value_date: parsed.as_ref().and_then(|details| details.value_date),
        reversal: parsed.as_ref().and_then(|details| details.reversal),
        free_text: parsed.map(|details| details.free_text).unwrap_or_default(),
    })
}

struct TransactionDetails {
    quantity: SecuritiesQuantity,
    amount: Option<SecuritiesAmount>,
    accrued_interest: Option<SecuritiesAmount>,
    transaction_kind: Option<String>,
    movement: SecuritiesMovement,
    effective_date: NaiveDate,
    value_date: Option<NaiveDate>,
    reversal: Option<bool>,
    free_text: Vec<String>,
}

fn parse_transaction_details(details: &Block) -> Result<TransactionDetails, Error> {
    let quantity = parse_quantity(
        &required_qualified(details, "36B", ":PSTA//")?.value,
        ":PSTA//",
        false,
    )?;
    let movement = match required_qualified(details, "22H", ":REDE//")?
        .value
        .strip_prefix(":REDE//")
    {
        Some("DELI") => SecuritiesMovement::Delivery,
        Some("RECE") => SecuritiesMovement::Receipt,
        _ => return Err(Error::MalformedSecuritiesData),
    };
    if required_qualified(details, "22H", ":PAYM//")?.value != ":PAYM//FREE" {
        return Err(Error::MalformedSecuritiesData);
    }
    let transaction_kind = match required_qualified(details, "22F", ":TRAN//")?
        .value
        .strip_prefix(":TRAN//")
    {
        Some(value @ ("BOLE" | "COLL" | "CORP" | "SETT")) => Some(value.to_owned()),
        _ => return Err(Error::MalformedSecuritiesData),
    };
    let effective_date =
        parse_qualified_date(required_qualified_any(details, &["98A", "98C"], ":ESET//")?)?;
    Ok(TransactionDetails {
        quantity,
        amount: optional_qualified(details, "19A", ":PSTA//")
            .map(|field| parse_signed_amount(&field.value, ":PSTA//"))
            .transpose()?,
        accrued_interest: optional_qualified(details, "19A", ":ACRU//")
            .map(|field| parse_signed_amount(&field.value, ":ACRU//"))
            .transpose()?,
        transaction_kind,
        movement,
        effective_date,
        value_date: optional_qualified_any(details, &["98A", "98C"], ":SETT//")
            .map(parse_qualified_date)
            .transpose()?,
        reversal: optional_qualified(details, "25D", ":MOVE//")
            .map(|field| match field.value.strip_prefix(":MOVE//") {
                Some("REVE") => Ok(true),
                _ => Err(Error::MalformedSecuritiesData),
            })
            .transpose()?,
        free_text: optional_qualified(details, "70E", ":TRDE//")
            .map(|field| {
                field
                    .value
                    .trim_start_matches(":TRDE//")
                    .split('\n')
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn parse_instrument(field: &Field) -> Result<SecurityInstrument, Error> {
    let mut isin = None;
    let mut wkn = None;
    let mut names = Vec::new();
    for line in field.value.split('\n') {
        if let Some(value) = line.strip_prefix("ISIN ") {
            if isin.replace(value.to_owned()).is_some() {
                return Err(Error::MalformedSecuritiesData);
            }
        } else if let Some(value) = line.strip_prefix("/DE/") {
            if wkn.replace(value.to_owned()).is_some() {
                return Err(Error::MalformedSecuritiesData);
            }
        } else if !line.is_empty() {
            names.push(line);
        }
    }
    if isin.is_none() && wkn.is_none() {
        return Err(Error::MalformedSecuritiesData);
    }
    let name = names.join("\n");
    if name.is_empty() {
        return Err(Error::MalformedSecuritiesData);
    }
    Ok(SecurityInstrument { isin, wkn, name })
}

fn parse_price(value: &str, block: &Block) -> Result<SecurityPrice, Error> {
    let (quality, rest) = if let Some(rest) = value.strip_prefix(":MRKT//") {
        (PriceQuality::Market, rest)
    } else if let Some(rest) = value.strip_prefix(":INDC//") {
        (PriceQuality::Indicative, rest)
    } else {
        return Err(Error::MalformedSecuritiesData);
    };
    let (percentage, rest) = if let Some(rest) = rest.strip_prefix("PRCT/") {
        (true, rest)
    } else if let Some(rest) = rest.strip_prefix("ACTU/") {
        (false, rest)
    } else {
        return Err(Error::MalformedSecuritiesData);
    };
    let (currency, number) = if percentage {
        (None, rest)
    } else {
        let (currency, number) = rest
            .split_at_checked(3)
            .ok_or(Error::MalformedSecuritiesData)?;
        validate_currency(currency)?;
        (Some(currency.to_owned()), number)
    };
    let (coefficient, scale) = decimal(number)?;
    let date_field = optional_qualified_any(block, &["98A", "98C"], ":PRIC//");
    let (date, time) = date_field
        .map(parse_qualified_timestamp)
        .transpose()?
        .unwrap_or((None, None));
    Ok(SecurityPrice::new(
        coefficient,
        scale,
        currency,
        percentage,
        Some(quality),
        date,
        time,
    ))
}

fn parse_quantity(
    value: &str,
    prefix: &str,
    allow_negative: bool,
) -> Result<SecuritiesQuantity, Error> {
    // For FAMT, the nominal-value currency can also appear in structured
    // 70E::HOLD line 1 field 2. Gate 4 deliberately does not expose that
    // separate metadata value; it is not inferred from the quantity itself.
    let rest = value
        .strip_prefix(prefix)
        .ok_or(Error::MalformedSecuritiesData)?;
    let (unit, mut number) = rest.split_once('/').ok_or(Error::MalformedSecuritiesData)?;
    let unit = match unit {
        "UNIT" => QuantityUnit::Units,
        "FAMT" => QuantityUnit::Nominal,
        _ => return Err(Error::MalformedSecuritiesData),
    };
    let negative = number.starts_with('N');
    if negative {
        if !allow_negative {
            return Err(Error::MalformedSecuritiesData);
        }
        number = &number[1..];
    }
    let (coefficient, scale) = decimal(number)?;
    Ok(SecuritiesQuantity::new(coefficient, scale, unit, negative))
}

fn parse_signed_amount(value: &str, prefix: &str) -> Result<SecuritiesAmount, Error> {
    let mut rest = value
        .strip_prefix(prefix)
        .ok_or(Error::MalformedSecuritiesData)?;
    let negative = rest.starts_with('N');
    if negative {
        rest = &rest[1..];
    }
    let (currency, number) = rest
        .split_at_checked(3)
        .ok_or(Error::MalformedSecuritiesData)?;
    validate_currency(currency)?;
    let (coefficient, scale) = decimal(number)?;
    Ok(SecuritiesAmount {
        amount: Amount::new(coefficient, scale, currency.to_owned()),
        negative,
    })
}

fn parse_cost_basis(value: &str) -> Result<Option<SecurityPrice>, Error> {
    // Anlage 3 v3.9 4.3 places the explicitly reported acquisition amount and
    // currency on line two of the structured HOLD field.
    // Anlage 3 v3.9 4.3 mandates the structured 70E::HOLD line-number digits.
    let body = value
        .strip_prefix(":HOLD//")
        .ok_or(Error::MalformedSecuritiesData)?;
    if !body
        .split('\n')
        .next()
        .is_some_and(|line| line.starts_with('1'))
    {
        return Err(Error::MalformedSecuritiesData);
    }
    let Some(line) = body.split('\n').nth(1) else {
        return Ok(None);
    };
    let line = line
        .strip_prefix('2')
        .ok_or(Error::MalformedSecuritiesData)?;
    let mut fields = line.split('+');
    let Some(number) = fields.next().filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let currency = fields.next().filter(|value| !value.is_empty());
    if let Some(currency) = currency {
        validate_currency(currency)?;
    }
    let (coefficient, scale) = decimal(number)?;
    Ok(Some(SecurityPrice::new(
        coefficient,
        scale,
        currency.map(str::to_owned),
        currency.is_none(),
        None,
        None,
        None,
    )))
}

fn decimal(value: &str) -> Result<(u128, u8), Error> {
    if value.len() > 24 {
        return Err(Error::MalformedSecuritiesData);
    }
    let (integer, fraction) = value
        .split_once(',')
        .ok_or(Error::MalformedSecuritiesData)?;
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::MalformedSecuritiesData);
    }
    let coefficient = format!("{integer}{fraction}")
        .parse()
        .map_err(|_| Error::MalformedSecuritiesData)?;
    let scale = u8::try_from(fraction.len()).map_err(|_| Error::MalformedSecuritiesData)?;
    Ok((coefficient, scale))
}

fn validate_currency(value: &str) -> Result<(), Error> {
    if value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        Ok(())
    } else {
        Err(Error::MalformedSecuritiesData)
    }
}

fn parse_qualified_date(field: &Field) -> Result<NaiveDate, Error> {
    let value = field
        .value
        .rsplit_once("//")
        .map(|(_, value)| value)
        .ok_or(Error::MalformedSecuritiesData)?;
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::MalformedSecuritiesData);
    }
    match field.tag.as_str() {
        "98A" if value.len() == 8 => parse_date(value),
        "98C" if value.len() == 14 => {
            let date = parse_date(&value[..8])?;
            NaiveTime::parse_from_str(&value[8..], "%H%M%S")
                .map_err(|_| Error::MalformedSecuritiesData)?;
            Ok(date)
        }
        _ => Err(Error::MalformedSecuritiesData),
    }
}

fn parse_qualified_timestamp(
    field: &Field,
) -> Result<(Option<NaiveDate>, Option<NaiveTime>), Error> {
    let value = field
        .value
        .rsplit_once("//")
        .map(|(_, value)| value)
        .ok_or(Error::MalformedSecuritiesData)?;
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::MalformedSecuritiesData);
    }
    match field.tag.as_str() {
        "98A" if value.len() == 8 => Ok((Some(parse_date(value)?), None)),
        "98C" if value.len() == 14 => Ok((
            Some(parse_date(&value[..8])?),
            Some(
                NaiveTime::parse_from_str(&value[8..], "%H%M%S")
                    .map_err(|_| Error::MalformedSecuritiesData)?,
            ),
        )),
        _ => Err(Error::MalformedSecuritiesData),
    }
}

fn parse_date(value: &str) -> Result<NaiveDate, Error> {
    NaiveDate::parse_from_str(value, "%Y%m%d").map_err(|_| Error::MalformedSecuritiesData)
}

fn page_more(field: &Field) -> Result<bool, Error> {
    let (page, indicator) = field
        .value
        .split_once('/')
        .ok_or(Error::MalformedSecuritiesData)?;
    if page.is_empty() || page.len() > 5 || !page.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::MalformedSecuritiesData);
    }
    match indicator {
        "MORE" => Ok(true),
        "ONLY" | "LAST" => Ok(false),
        _ => Err(Error::MalformedSecuritiesData),
    }
}

fn children<'a>(block: &'a Block, name: &'a str) -> impl Iterator<Item = &'a Block> {
    block
        .children
        .iter()
        .filter(move |child| child.name == name)
}

fn unique_child<'a>(block: &'a Block, name: &str) -> Result<&'a Block, Error> {
    let mut matching = block.children.iter().filter(|child| child.name == name);
    let value = matching.next().ok_or(Error::MalformedSecuritiesData)?;
    if matching.next().is_some() {
        return Err(Error::MalformedSecuritiesData);
    }
    Ok(value)
}

fn optional_unique_child<'a>(block: &'a Block, name: &str) -> Result<Option<&'a Block>, Error> {
    let mut matching = block.children.iter().filter(|child| child.name == name);
    let value = matching.next();
    if matching.next().is_some() {
        return Err(Error::MalformedSecuritiesData);
    }
    Ok(value)
}

fn fields<'a>(block: &'a Block, tag: &'a str) -> impl Iterator<Item = &'a Field> {
    block.fields.iter().filter(move |field| field.tag == tag)
}

fn required_field<'a>(block: &'a Block, tag: &str) -> Result<&'a Field, Error> {
    let mut matching = block.fields.iter().filter(|field| field.tag == tag);
    let value = matching.next().ok_or(Error::MalformedSecuritiesData)?;
    if matching.next().is_some() {
        return Err(Error::MalformedSecuritiesData);
    }
    Ok(value)
}

fn optional_field<'a>(block: &'a Block, tag: &str) -> Option<&'a Field> {
    block.fields.iter().find(|field| field.tag == tag)
}

fn required_qualified<'a>(block: &'a Block, tag: &str, prefix: &str) -> Result<&'a Field, Error> {
    optional_qualified(block, tag, prefix).ok_or(Error::MalformedSecuritiesData)
}

fn optional_qualified<'a>(block: &'a Block, tag: &str, prefix: &str) -> Option<&'a Field> {
    block
        .fields
        .iter()
        .find(|field| field.tag == tag && field.value.starts_with(prefix))
}

fn required_qualified_any<'a>(
    block: &'a Block,
    tags: &[&str],
    prefix: &str,
) -> Result<&'a Field, Error> {
    optional_qualified_any(block, tags, prefix).ok_or(Error::MalformedSecuritiesData)
}

fn optional_qualified_any<'a>(block: &'a Block, tags: &[&str], prefix: &str) -> Option<&'a Field> {
    block
        .fields
        .iter()
        .find(|field| tags.contains(&field.tag.as_str()) && field.value.starts_with(prefix))
}
