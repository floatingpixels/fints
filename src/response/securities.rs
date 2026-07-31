use chrono::{NaiveDate, NaiveTime};

use crate::{
    Limitation,
    error::{Error, malformed_securities_data},
    model::{
        Amount, DepotPosition, DepotPositionParseCounts, PriceQuality, QuantityUnit,
        SecuritiesAmount, SecuritiesMovement, SecuritiesQuantity, SecuritiesTransaction,
        SecurityInstrument, SecurityPrice,
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
    pub(crate) parse_counts: DepotPositionParseCounts,
    #[cfg(feature = "diagnostics")]
    pub(crate) development_facts: crate::DepotResponseFacts,
}

pub(crate) struct SecuritiesTransactionPage {
    pub(crate) more: bool,
    pub(crate) institute_code: String,
    pub(crate) account_number: String,
    pub(crate) entries: Vec<SecuritiesTransaction>,
    #[cfg(feature = "diagnostics")]
    pub(crate) development_facts: crate::DepotResponseFacts,
}

pub(super) fn positions(
    segments: &[Segment],
    version: u16,
) -> Result<Option<DepotPositionPage>, Error> {
    // HBCI 2.2 VII.4.3.1 assigns MT535 to HIWPD 5; Messages 2022 C.4.3.1
    // retains the same SRG-1998 MT535 payload for HIWPD 6. Version 4 carries
    // MT571 and is deliberately unsupported.
    if !(5..=6).contains(&version) {
        return Err(Limitation::DepotPositionsVersion.into());
    }
    let payload = response_binary(segments, b"HIWPD", "HIWPD", version)?;
    payload.map(parse_positions).transpose()
}

#[cfg(feature = "diagnostics")]
pub(super) fn position_structure(
    segments: &[Segment],
    version: u16,
) -> Result<Option<crate::DepotResponseFacts>, Error> {
    let payload = response_binary(segments, b"HIWPD", "HIWPD", version)?;
    payload
        .map(|input| {
            let root = document(input)?;
            let first_price_failure = first_price_failure(&root, true);
            Ok(crate::DepotResponseFacts::for_position_structure(
                block_inventory(&root),
                tag_inventory(&root),
                price_shapes(&root, true),
                first_price_failure.map(|(ordinal, _)| ordinal),
                first_price_failure.map(|(_, stage)| stage),
            ))
        })
        .transpose()
}

pub(super) fn transactions(
    segments: &[Segment],
) -> Result<Option<SecuritiesTransactionPage>, Error> {
    let payload = response_binary(segments, b"HIWDU", "HIWDU", 5)?;
    payload.map(parse_transactions).transpose()
}

#[cfg(feature = "diagnostics")]
pub(super) fn transaction_structure(
    segments: &[Segment],
) -> Result<Option<crate::DepotResponseFacts>, Error> {
    let payload = response_binary(segments, b"HIWDU", "HIWDU", 5)?;
    payload
        .map(|input| {
            let root = document(input)?;
            let first_price_failure = first_price_failure(&root, false);
            Ok(crate::DepotResponseFacts::for_transaction_structure(
                block_inventory(&root),
                tag_inventory(&root),
                price_shapes(&root, false),
                first_price_failure.map(|(ordinal, _)| ordinal),
                first_price_failure.map(|(_, stage)| stage),
            ))
        })
        .transpose()
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PriceParseStage {
    Qualifier,
    TagUnitPairing,
    CurrencyShape,
    DecimalShape,
    PriceTimestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PositionFailureSite {
    Instrument,
    Quantity,
    Price,
    MarketValue,
    CostBasis,
}

struct ParsedPosition {
    position: DepotPosition,
    degraded: bool,
    #[cfg(feature = "diagnostics")]
    failure_sites: Vec<PositionFailureSite>,
}

fn record_position_degradation(
    degraded: &mut bool,
    #[cfg(feature = "diagnostics")] failure_sites: &mut Vec<PositionFailureSite>,
    site: PositionFailureSite,
) {
    *degraded = true;
    #[cfg(feature = "diagnostics")]
    if !failure_sites.contains(&site) {
        failure_sites.push(site);
    }
    #[cfg(not(feature = "diagnostics"))]
    let _ = site;
}

#[cfg(feature = "diagnostics")]
fn diagnostic_position_failure_site(site: PositionFailureSite) -> crate::DepotPositionFailureSite {
    match site {
        PositionFailureSite::Instrument => crate::DepotPositionFailureSite::Instrument,
        PositionFailureSite::Quantity => crate::DepotPositionFailureSite::Quantity,
        PositionFailureSite::Price => crate::DepotPositionFailureSite::Price,
        PositionFailureSite::MarketValue => crate::DepotPositionFailureSite::MarketValue,
        PositionFailureSite::CostBasis => crate::DepotPositionFailureSite::CostBasis,
    }
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
        .ok_or(malformed_securities_data!("document/framing"))?;
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
                    return Err(malformed_securities_data!("document/field-limit"));
                }
                logical.push(Field {
                    tag: tag.to_owned(),
                    value: rest[end + 1..].to_owned(),
                });
                continue;
            }
        }
        let field = logical
            .last_mut()
            .ok_or(malformed_securities_data!("document/continuation-line"))?;
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
            return Err(malformed_securities_data!("document/top-level/16R"));
        }
        root.children.push(parse_block(&logical, &mut cursor, 1)?);
    }
    if root.children.is_empty() {
        return Err(malformed_securities_data!("document/empty"));
    }
    Ok(root)
}

#[cfg(feature = "fuzzing")]
pub(super) fn fuzz_document(input: &[u8]) -> bool {
    document(input).is_ok()
}

fn parse_block(fields: &[Field], cursor: &mut usize, depth: usize) -> Result<Block, Error> {
    if depth > MAX_BLOCK_DEPTH {
        return Err(malformed_securities_data!("document/16R/depth"));
    }
    let start = fields
        .get(*cursor)
        .ok_or(malformed_securities_data!("document/16R/start"))?;
    if start.tag != "16R" || start.value.is_empty() {
        return Err(malformed_securities_data!("document/16R/name"));
    }
    *cursor += 1;
    let mut block = Block {
        name: start.value.clone(),
        fields: Vec::new(),
        children: Vec::new(),
    };
    loop {
        let field = fields
            .get(*cursor)
            .ok_or(malformed_securities_data!("document/16R:16S/unterminated"))?;
        match field.tag.as_str() {
            "16R" => block.children.push(parse_block(fields, cursor, depth + 1)?),
            "16S" => {
                if field.value != block.name {
                    return Err(malformed_securities_data!("document/16R:16S/mismatch"));
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
    validate_root(&root).map_err(|_| malformed_securities_data!("MT535/ROOT/GENL"))?;
    let general =
        unique_child(&root, "GENL").map_err(|_| malformed_securities_data!("MT535/GENL"))?;
    let _active =
        validate_general(general).map_err(|_| malformed_securities_data!("MT535/GENL/17B:ACTI"))?;
    let more = page_more(
        required_field(general, "28E").map_err(|_| malformed_securities_data!("MT535/GENL/28E"))?,
    )
    .map_err(|_| malformed_securities_data!("MT535/GENL/28E"))?;
    let (institute_code, account_number) =
        safe_identity(general).map_err(|_| malformed_securities_data!("MT535/GENL/97A:SAFE"))?;
    let reported_positions = children(&root, "FIN").count();
    if reported_positions > MAX_PAGE_ENTRIES {
        return Err(malformed_securities_data!("MT535/page/entry-limit"));
    }
    let mut positions = Vec::with_capacity(reported_positions);
    let mut degraded = 0usize;
    let mut skipped = 0usize;
    #[cfg(feature = "diagnostics")]
    let mut position_failures = Vec::new();
    for (index, block) in children(&root, "FIN").enumerate() {
        let _ordinal = index + 1;
        match parse_position(block) {
            Ok(parsed) => {
                if parsed.degraded {
                    degraded += 1;
                }
                #[cfg(feature = "diagnostics")]
                position_failures.extend(parsed.failure_sites.into_iter().map(|site| {
                    crate::DepotPositionFailureFact::new(
                        _ordinal,
                        crate::DepotPositionFailureDisposition::Degraded,
                        diagnostic_position_failure_site(site),
                    )
                }));
                positions.push(parsed.position);
            }
            Err(_site) => {
                skipped += 1;
                #[cfg(feature = "diagnostics")]
                position_failures.push(crate::DepotPositionFailureFact::new(
                    _ordinal,
                    crate::DepotPositionFailureDisposition::Skipped,
                    diagnostic_position_failure_site(_site),
                ));
            }
        }
    }
    let total_values = root
        .children
        .iter()
        .find(|block| block.name == "ADDINFO")
        .map(|block| {
            fields(block, "19A")
                .filter(|field| field.value.starts_with(":HOLP//"))
                .filter_map(|field| parse_signed_amount(&field.value, ":HOLP//").ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    #[cfg(feature = "diagnostics")]
    let development_facts = crate::DepotResponseFacts::for_positions(
        block_inventory(&root),
        tag_inventory(&root),
        price_shapes(&root, true),
        &positions,
        position_failures,
        first_price_failure(&root, true),
    );
    Ok(DepotPositionPage {
        more,
        institute_code,
        account_number,
        positions,
        total_values,
        parse_counts: DepotPositionParseCounts::new(degraded, skipped),
        #[cfg(feature = "diagnostics")]
        development_facts,
    })
}

fn parse_transactions(input: &[u8]) -> Result<SecuritiesTransactionPage, Error> {
    let root = document(input)?;
    validate_root(&root).map_err(|_| malformed_securities_data!("MT536/ROOT/GENL"))?;
    let general =
        unique_child(&root, "GENL").map_err(|_| malformed_securities_data!("MT536/GENL"))?;
    let active =
        validate_general(general).map_err(|_| malformed_securities_data!("MT536/GENL/17B:ACTI"))?;
    let more = page_more(
        required_field(general, "28E").map_err(|_| malformed_securities_data!("MT536/GENL/28E"))?,
    )
    .map_err(|_| malformed_securities_data!("MT536/GENL/28E"))?;
    let (institute_code, account_number) =
        safe_identity(general).map_err(|_| malformed_securities_data!("MT536/GENL/97A:SAFE"))?;
    let mut entries = Vec::new();
    for financial in children(&root, "FIN") {
        let instrument = parse_instrument(
            required_field(financial, "35B")
                .map_err(|_| malformed_securities_data!("MT536/FIN/35B"))?,
        )
        .map_err(|_| malformed_securities_data!("MT536/FIN/35B"))?;
        let price = optional_field(financial, "90A")
            .or_else(|| optional_field(financial, "90B"))
            .map(|field| parse_price(field, financial, false))
            .transpose()
            .map_err(mt536_price_error)?;
        for transaction in children(financial, "TRAN") {
            entries.push(parse_transaction(transaction, &instrument, price.as_ref())?);
            if entries.len() > MAX_PAGE_ENTRIES {
                return Err(malformed_securities_data!("MT536/page/entry-limit"));
            }
        }
    }
    if !active && !entries.is_empty() {
        return Err(malformed_securities_data!("MT536/GENL/17B:ACTI"));
    }
    #[cfg(feature = "diagnostics")]
    let development_facts = crate::DepotResponseFacts::for_transactions(
        block_inventory(&root),
        tag_inventory(&root),
        price_shapes(&root, false),
        &entries,
    );
    Ok(SecuritiesTransactionPage {
        more,
        institute_code,
        account_number,
        entries,
        #[cfg(feature = "diagnostics")]
        development_facts,
    })
}

#[cfg(feature = "diagnostics")]
fn block_inventory(root: &Block) -> Vec<crate::DepotBlockFact> {
    fn visit(
        block: &Block,
        depth: usize,
        counts: &mut [usize; 8],
        facts: &mut Vec<crate::DepotBlockFact>,
    ) {
        let kind = match block.name.as_str() {
            "GENL" => crate::DepotBlockKind::General,
            "FIN" => crate::DepotBlockKind::FinancialInstrument,
            "SUBBAL" => crate::DepotBlockKind::SubBalance,
            "ADDINFO" => crate::DepotBlockKind::AdditionalInformation,
            "TRAN" => crate::DepotBlockKind::Transaction,
            "LINK" => crate::DepotBlockKind::Link,
            "TRANSDET" => crate::DepotBlockKind::TransactionDetails,
            _ => crate::DepotBlockKind::Other,
        };
        counts[kind.index()] += 1;
        facts.push(crate::DepotBlockFact::new(
            kind,
            depth,
            counts[kind.index()],
        ));
        for child in &block.children {
            visit(child, depth + 1, counts, facts);
        }
    }

    let mut counts = [0; 8];
    let mut facts = Vec::new();
    for child in &root.children {
        visit(child, 1, &mut counts, &mut facts);
    }
    facts
}

#[cfg(feature = "diagnostics")]
fn tag_inventory(root: &Block) -> Vec<crate::DepotTagFact> {
    fn kind(tag: &str) -> crate::DepotTagKind {
        match tag {
            "13A" => crate::DepotTagKind::StatementNumber13A,
            "17B" => crate::DepotTagKind::Activity17B,
            "19A" => crate::DepotTagKind::Amount19A,
            "20C" => crate::DepotTagKind::Reference20C,
            "22F" => crate::DepotTagKind::Indicator22F,
            "22H" => crate::DepotTagKind::Movement22H,
            "25D" => crate::DepotTagKind::Status25D,
            "28E" => crate::DepotTagKind::Page28E,
            "35B" => crate::DepotTagKind::Instrument35B,
            "36B" => crate::DepotTagKind::TransactionQuantity36B,
            "69A" | "69B" => crate::DepotTagKind::DateRange69,
            "70C" | "70E" => crate::DepotTagKind::FreeText70,
            "90A" => crate::DepotTagKind::PercentagePrice90A,
            "90B" => crate::DepotTagKind::AmountPrice90B,
            "92B" => crate::DepotTagKind::ExchangeRate92B,
            "93B" => crate::DepotTagKind::Quantity93B,
            "93C" => crate::DepotTagKind::SubBalance93C,
            "94B" | "94C" => crate::DepotTagKind::Location94,
            "97A" => crate::DepotTagKind::Account97A,
            "98A" | "98C" => crate::DepotTagKind::DateTime98,
            "99A" => crate::DepotTagKind::Days99A,
            _ => crate::DepotTagKind::Other,
        }
    }

    fn visit(
        block: &Block,
        depth: usize,
        counts: &mut [usize; 22],
        facts: &mut Vec<crate::DepotTagFact>,
    ) {
        for field in &block.fields {
            let kind = kind(&field.tag);
            counts[kind.index()] += 1;
            facts.push(crate::DepotTagFact::new(kind, depth, counts[kind.index()]));
        }
        for child in &block.children {
            visit(child, depth + 1, counts, facts);
        }
    }

    let mut counts = [0; 22];
    let mut facts = Vec::new();
    for child in &root.children {
        visit(child, 1, &mut counts, &mut facts);
    }
    facts
}

#[cfg(feature = "diagnostics")]
fn price_shapes(
    root: &Block,
    require_mt535_tag_unit_pair: bool,
) -> Vec<crate::DepotPriceShapeFact> {
    use crate::{
        DepotPriceCurrencyShapeKind, DepotPriceQualifierKind, DepotPriceShapeFact,
        DepotPriceTagKind, DepotPriceTimestampQualifierKind, DepotPriceTimestampTagKind,
        DepotPriceUnitKind, diagnostics::DepotPriceShapeInput,
    };

    children(root, "FIN")
        .flat_map(|financial| {
            financial.fields.iter().filter_map(|field| {
                let tag = match field.tag.as_str() {
                    "90A" => DepotPriceTagKind::Percentage90A,
                    "90B" => DepotPriceTagKind::Amount90B,
                    _ => return None,
                };
                let (qualifier, rest) = field
                    .value
                    .strip_prefix(':')
                    .and_then(|value| value.split_once("//"))
                    .unwrap_or(("", ""));
                let (unit, payload) = rest.split_once('/').unwrap_or(("", ""));
                let qualifier_kind = match qualifier {
                    "MRKT" => DepotPriceQualifierKind::Market,
                    "INDC" => DepotPriceQualifierKind::Indicative,
                    _ => DepotPriceQualifierKind::Unknown,
                };
                let unit_kind = match unit {
                    "PRCT" => DepotPriceUnitKind::Percentage,
                    "ACTU" => DepotPriceUnitKind::ActualAmount,
                    _ => DepotPriceUnitKind::Unknown,
                };
                let qualifier_shape_valid = matches!(
                    qualifier_kind,
                    DepotPriceQualifierKind::Market | DepotPriceQualifierKind::Indicative
                );
                let tag_unit_pair_valid = matches!(
                    (require_mt535_tag_unit_pair, tag, unit_kind),
                    (
                        true,
                        DepotPriceTagKind::Percentage90A,
                        DepotPriceUnitKind::Percentage
                    ) | (
                        true,
                        DepotPriceTagKind::Amount90B,
                        DepotPriceUnitKind::ActualAmount
                    ) | (
                        false,
                        _,
                        DepotPriceUnitKind::Percentage | DepotPriceUnitKind::ActualAmount
                    )
                );
                let entire_payload_decimal_shape_valid = decimal(payload).is_ok();
                let (
                    currency_prefix_width_available,
                    currency_shape,
                    price_present,
                    currency_shape_valid,
                    decimal_shape_valid,
                ) = match unit_kind {
                    DepotPriceUnitKind::Percentage => (
                        false,
                        None,
                        !payload.is_empty(),
                        true,
                        entire_payload_decimal_shape_valid,
                    ),
                    DepotPriceUnitKind::ActualAmount => {
                        let candidate = split_currency_candidate(payload);
                        let currency_shape = Some(
                            candidate.map_or(DepotPriceCurrencyShapeKind::Absent, |(prefix, _)| {
                                classify_currency_shape(prefix)
                            }),
                        );
                        let decimal_shape_valid =
                            candidate.is_some_and(|(_, value)| decimal(value).is_ok());
                        (
                            candidate.is_some(),
                            currency_shape,
                            candidate.is_some_and(|(_, value)| !value.is_empty()),
                            currency_shape
                                == Some(DepotPriceCurrencyShapeKind::UppercaseAlphabetic),
                            decimal_shape_valid,
                        )
                    }
                    DepotPriceUnitKind::Unknown => (false, None, !payload.is_empty(), false, false),
                };
                let timestamp_field = financial
                    .fields
                    .iter()
                    .filter(|field| field.tag.starts_with("98"))
                    .find(|field| field.value.starts_with(":PRIC"))
                    .or_else(|| {
                        financial
                            .fields
                            .iter()
                            .find(|field| field.tag == "98A" || field.tag == "98C")
                    });
                let (
                    timestamp_present,
                    timestamp_tag,
                    timestamp_qualifier,
                    timestamp_qualifier_shape_valid,
                    timestamp_length_digit_shape_valid,
                    timestamp_value_valid,
                ) = if let Some(timestamp) = timestamp_field {
                    let timestamp_tag = match timestamp.tag.as_str() {
                        "98A" => DepotPriceTimestampTagKind::Date98A,
                        "98C" => DepotPriceTimestampTagKind::DateTime98C,
                        _ => DepotPriceTimestampTagKind::Unknown,
                    };
                    let (timestamp_qualifier, timestamp_value) = timestamp
                        .value
                        .strip_prefix(':')
                        .and_then(|value| value.split_once("//"))
                        .unwrap_or(("", ""));
                    let timestamp_qualifier = match timestamp_qualifier {
                        "PRIC" => DepotPriceTimestampQualifierKind::Price,
                        _ => DepotPriceTimestampQualifierKind::Unknown,
                    };
                    let timestamp_qualifier_shape_valid =
                        timestamp_qualifier == DepotPriceTimestampQualifierKind::Price;
                    let timestamp_length_digit_shape_valid = match timestamp_tag {
                        DepotPriceTimestampTagKind::Date98A => {
                            timestamp_value.len() == 8
                                && timestamp_value.bytes().all(|byte| byte.is_ascii_digit())
                        }
                        DepotPriceTimestampTagKind::DateTime98C => {
                            timestamp_value.len() == 14
                                && timestamp_value.bytes().all(|byte| byte.is_ascii_digit())
                        }
                        DepotPriceTimestampTagKind::Unknown => false,
                    };
                    (
                        true,
                        Some(timestamp_tag),
                        Some(timestamp_qualifier),
                        timestamp_qualifier_shape_valid,
                        timestamp_length_digit_shape_valid,
                        timestamp_qualifier_shape_valid
                            && timestamp_length_digit_shape_valid
                            && parse_qualified_timestamp(timestamp).is_ok(),
                    )
                } else {
                    (false, None, None, true, true, true)
                };
                Some(DepotPriceShapeFact::new(DepotPriceShapeInput {
                    tag,
                    qualifier: qualifier_kind,
                    unit: unit_kind,
                    qualifier_present: !qualifier.is_empty(),
                    unit_present: !unit.is_empty(),
                    currency_prefix_width_available,
                    currency_shape,
                    price_present,
                    qualifier_shape_valid,
                    tag_unit_pair_valid,
                    currency_shape_valid,
                    decimal_shape_valid,
                    entire_payload_decimal_shape_valid,
                    timestamp_present,
                    timestamp_tag,
                    timestamp_qualifier,
                    timestamp_qualifier_shape_valid,
                    timestamp_length_digit_shape_valid,
                    timestamp_value_valid,
                }))
            })
        })
        .collect()
}

#[cfg(feature = "diagnostics")]
fn split_currency_candidate(value: &str) -> Option<(&str, &str)> {
    let mut characters = value.char_indices();
    characters.next()?;
    characters.next()?;
    characters.next()?;
    let boundary = characters.next().map_or(value.len(), |(index, _)| index);
    Some(value.split_at(boundary))
}

#[cfg(feature = "diagnostics")]
fn classify_currency_shape(value: &str) -> crate::DepotPriceCurrencyShapeKind {
    use crate::DepotPriceCurrencyShapeKind;

    if value.bytes().all(|byte| byte == b' ') {
        DepotPriceCurrencyShapeKind::BlankSpacePadded
    } else if value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        DepotPriceCurrencyShapeKind::UppercaseAlphabetic
    } else if value.bytes().all(|byte| byte.is_ascii_lowercase()) {
        DepotPriceCurrencyShapeKind::LowercaseAlphabetic
    } else if value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        DepotPriceCurrencyShapeKind::MixedCaseAlphabetic
    } else if value.bytes().all(|byte| byte.is_ascii_digit()) {
        DepotPriceCurrencyShapeKind::Numeric
    } else {
        DepotPriceCurrencyShapeKind::Other
    }
}

#[cfg(feature = "diagnostics")]
fn first_price_failure(
    root: &Block,
    require_mt535_tag_unit_pair: bool,
) -> Option<(usize, crate::DepotPriceFailureStage)> {
    children(root, "FIN")
        .flat_map(|financial| {
            financial
                .fields
                .iter()
                .filter(|field| field.tag == "90A" || field.tag == "90B")
                .map(move |field| (field, financial))
        })
        .enumerate()
        .find_map(|(index, (field, financial))| {
            parse_price(field, financial, require_mt535_tag_unit_pair)
                .err()
                .map(|stage| {
                    let stage = match stage {
                        PriceParseStage::Qualifier => crate::DepotPriceFailureStage::Qualifier,
                        PriceParseStage::TagUnitPairing => {
                            crate::DepotPriceFailureStage::TagUnitPairing
                        }
                        PriceParseStage::CurrencyShape => {
                            crate::DepotPriceFailureStage::CurrencyShape
                        }
                        PriceParseStage::DecimalShape => {
                            crate::DepotPriceFailureStage::DecimalShape
                        }
                        PriceParseStage::PriceTimestamp => {
                            crate::DepotPriceFailureStage::PriceTimestamp
                        }
                    };
                    (index + 1, stage)
                })
        })
}

fn safe_identity(block: &Block) -> Result<(String, String), Error> {
    let value = required_qualified(block, "97A", ":SAFE//")?
        .value
        .strip_prefix(":SAFE//")
        .ok_or(malformed_securities_data!())?;
    let (institute, account) = value.split_once('/').ok_or(malformed_securities_data!())?;
    if institute.is_empty() || account.is_empty() {
        return Err(malformed_securities_data!());
    }
    Ok((institute.to_owned(), account.to_owned()))
}

fn validate_root(root: &Block) -> Result<(), Error> {
    if root
        .children
        .first()
        .is_none_or(|block| block.name != "GENL")
    {
        return Err(malformed_securities_data!());
    }
    Ok(())
}

fn validate_general(block: &Block) -> Result<bool, Error> {
    required_field(block, "28E")?;
    required_qualified(block, "97A", ":SAFE//")?;
    match required_qualified(block, "17B", ":ACTI//")?
        .value
        .strip_prefix(":ACTI//")
    {
        Some("Y") => Ok(true),
        Some("N") => Ok(false),
        _ => Err(malformed_securities_data!()),
    }
}

fn parse_position(block: &Block) -> Result<ParsedPosition, PositionFailureSite> {
    let instrument = parse_instrument(
        required_field(block, "35B").map_err(|_| PositionFailureSite::Instrument)?,
    )
    .map_err(|_| PositionFailureSite::Instrument)?;
    let quantity = parse_quantity(
        &required_qualified(block, "93B", ":AGGR//")
            .map_err(|_| PositionFailureSite::Quantity)?
            .value,
        ":AGGR//",
        true,
    )
    .map_err(|_| PositionFailureSite::Quantity)?;
    let mut degraded = false;
    #[cfg(feature = "diagnostics")]
    let mut failure_sites = Vec::new();
    let price = optional_field(block, "90A")
        .or_else(|| optional_field(block, "90B"))
        .and_then(|field| match parse_price(field, block, true) {
            Ok(price) => Some(price),
            Err(_) => {
                record_position_degradation(
                    &mut degraded,
                    #[cfg(feature = "diagnostics")]
                    &mut failure_sites,
                    PositionFailureSite::Price,
                );
                None
            }
        });
    let mut market_values = Vec::new();
    for field in fields(block, "19A").filter(|field| field.value.starts_with(":HOLD//")) {
        match parse_signed_amount(&field.value, ":HOLD//") {
            Ok(amount) => market_values.push(amount),
            Err(_) => record_position_degradation(
                &mut degraded,
                #[cfg(feature = "diagnostics")]
                &mut failure_sites,
                PositionFailureSite::MarketValue,
            ),
        }
    }
    let cost_basis = optional_qualified(block, "70E", ":HOLD//").and_then(|field| {
        match parse_cost_basis(&field.value) {
            Ok(value) => value,
            Err(_) => {
                record_position_degradation(
                    &mut degraded,
                    #[cfg(feature = "diagnostics")]
                    &mut failure_sites,
                    PositionFailureSite::CostBasis,
                );
                None
            }
        }
    });
    Ok(ParsedPosition {
        position: DepotPosition {
            instrument,
            quantity,
            price,
            market_values,
            cost_basis,
            #[cfg(feature = "diagnostics")]
            price_location_detail_present: optional_location_detail(block).is_some(),
        },
        degraded,
        #[cfg(feature = "diagnostics")]
        failure_sites,
    })
}

fn parse_transaction(
    block: &Block,
    instrument: &SecurityInstrument,
    price: Option<&SecurityPrice>,
) -> Result<SecuritiesTransaction, Error> {
    let link =
        unique_child(block, "LINK").map_err(|_| malformed_securities_data!("MT536/TRAN/LINK"))?;
    let reference = match required_qualified(link, "20C", ":RELA//")
        .map_err(|_| malformed_securities_data!("MT536/TRAN/LINK/20C:RELA"))?
        .value
        .strip_prefix(":RELA//")
    {
        Some("NONREF") => None,
        Some(value) if !value.is_empty() => Some(value.to_owned()),
        _ => return Err(malformed_securities_data!("MT536/TRAN/LINK/20C:RELA")),
    };
    let mut parsed = optional_unique_child(block, "TRANSDET")
        .map_err(|_| malformed_securities_data!("MT536/TRAN/TRANSDET"))?
        .map(|details| {
            parse_transaction_details(details)
                .map_err(|_| malformed_securities_data!("MT536/TRAN/TRANSDET"))
        })
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
        &required_qualified(details, "36B", ":PSTA//")
            .map_err(|_| malformed_securities_data!("MT536/TRANSDET/36B:PSTA"))?
            .value,
        ":PSTA//",
        false,
    )
    .map_err(|_| malformed_securities_data!("MT536/TRANSDET/36B:PSTA"))?;
    let movement = match required_qualified(details, "22H", ":REDE//")
        .map_err(|_| malformed_securities_data!("MT536/TRANSDET/22H:REDE"))?
        .value
        .strip_prefix(":REDE//")
    {
        Some("DELI") => SecuritiesMovement::Delivery,
        Some("RECE") => SecuritiesMovement::Receipt,
        _ => return Err(malformed_securities_data!("MT536/TRANSDET/22H:REDE")),
    };
    // Anlage 3 v3.9 4.4 prescribes the PAYM value, but Gate 4 does not consume
    // it. Presence is sufficient under the repository acceptance-space policy.
    required_qualified(details, "22H", ":PAYM//")
        .map_err(|_| malformed_securities_data!("MT536/TRANSDET/22H:PAYM"))?;
    let transaction_kind = required_qualified(details, "22F", ":TRAN//")
        .map_err(|_| malformed_securities_data!("MT536/TRANSDET/22F:TRAN"))?
        .value
        .strip_prefix(":TRAN//")
        .map(str::to_owned);
    let effective_date = parse_qualified_date(
        required_qualified_any(details, &["98A", "98C"], ":ESET//")
            .map_err(|_| malformed_securities_data!("MT536/TRANSDET/98A:98C:ESET"))?,
    )
    .map_err(|_| malformed_securities_data!("MT536/TRANSDET/98A:98C:ESET"))?;
    Ok(TransactionDetails {
        quantity,
        amount: optional_qualified(details, "19A", ":PSTA//")
            .map(|field| parse_signed_amount(&field.value, ":PSTA//"))
            .transpose()
            .map_err(|_| malformed_securities_data!("MT536/TRANSDET/19A:PSTA"))?,
        accrued_interest: optional_qualified(details, "19A", ":ACRU//")
            .map(|field| parse_signed_amount(&field.value, ":ACRU//"))
            .transpose()
            .map_err(|_| malformed_securities_data!("MT536/TRANSDET/19A:ACRU"))?,
        transaction_kind,
        movement,
        effective_date,
        value_date: optional_qualified_any(details, &["98A", "98C"], ":SETT//")
            .map(parse_qualified_date)
            .transpose()
            .map_err(|_| malformed_securities_data!("MT536/TRANSDET/98A:98C:SETT"))?,
        reversal: optional_qualified(details, "25D", ":MOVE//")
            .map(|field| field.value == ":MOVE//REVE"),
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
                return Err(malformed_securities_data!());
            }
        } else if let Some(value) = line.strip_prefix("/DE/") {
            if wkn.replace(value.to_owned()).is_some() {
                return Err(malformed_securities_data!());
            }
        } else if !line.is_empty() {
            names.push(line);
        }
    }
    if isin.is_none() && wkn.is_none() {
        return Err(malformed_securities_data!());
    }
    let name = names.join("\n");
    if name.is_empty() {
        return Err(malformed_securities_data!());
    }
    Ok(SecurityInstrument { isin, wkn, name })
}

fn mt536_price_error(stage: PriceParseStage) -> Error {
    match stage {
        PriceParseStage::Qualifier => {
            malformed_securities_data!("MT536/FIN/90A:90B/qualifier")
        }
        PriceParseStage::TagUnitPairing => {
            malformed_securities_data!("MT536/FIN/90A:90B/tag-unit")
        }
        PriceParseStage::CurrencyShape => {
            malformed_securities_data!("MT536/FIN/90B/currency-shape")
        }
        PriceParseStage::DecimalShape => {
            malformed_securities_data!("MT536/FIN/90A:90B/decimal-shape")
        }
        PriceParseStage::PriceTimestamp => {
            malformed_securities_data!("MT536/FIN/98A:98C:PRIC/timestamp-shape")
        }
    }
}

fn non_blank_component(value: &str) -> Option<&str> {
    (!value.is_empty() && !value.bytes().all(|byte| byte == b' ')).then_some(value)
}

#[cfg(feature = "diagnostics")]
fn optional_location_detail(block: &Block) -> Option<&str> {
    // DK Anlage 3 v3.9 4.3 and HBCI 2.2 IX.2.4 make 94B optional and
    // its final free-text component conditional on the separating slash.
    // Unknown optional qualifiers have no Gate 4 result semantics; retain
    // only whether a non-blank final component was structurally supplied.
    fields(block, "94B").find_map(|field| {
        let (_, location) = field.value.split_once("//")?;
        let (_, detail) = location.split_once('/')?;
        non_blank_component(detail)
    })
}

fn parse_price(
    field: &Field,
    block: &Block,
    require_mt535_tag_unit_pair: bool,
) -> Result<SecurityPrice, PriceParseStage> {
    let (quality, rest) = if let Some(rest) = field.value.strip_prefix(":MRKT//") {
        (PriceQuality::Market, rest)
    } else if let Some(rest) = field.value.strip_prefix(":INDC//") {
        (PriceQuality::Indicative, rest)
    } else {
        return Err(PriceParseStage::Qualifier);
    };
    // DK Anlage 3 v3.9 4.3 and HBCI 2.2 IX.2.4 define MT535 Option A as
    // 90A + PRCT and Option B as 90B + ACTU. For MT536, the v3.9 chapter
    // 4.4 full example itself prints 90B + PRCT despite its table; preserve
    // the existing unit-driven acceptance there rather than changing another
    // format while closing this MT535 finding.
    let (percentage, rest) = match (require_mt535_tag_unit_pair, field.tag.as_str()) {
        (true, "90A") => (
            true,
            rest.strip_prefix("PRCT/")
                .ok_or(PriceParseStage::TagUnitPairing)?,
        ),
        (true, "90B") => (
            false,
            rest.strip_prefix("ACTU/")
                .ok_or(PriceParseStage::TagUnitPairing)?,
        ),
        (false, "90A" | "90B") if rest.starts_with("PRCT/") => (true, &rest[5..]),
        (false, "90A" | "90B") if rest.starts_with("ACTU/") => (false, &rest[5..]),
        _ => return Err(PriceParseStage::TagUnitPairing),
    };
    let (currency, number) = if percentage {
        (None, rest)
    } else {
        let (currency, number) = rest
            .split_at_checked(3)
            .ok_or(PriceParseStage::CurrencyShape)?;
        let currency = non_blank_component(currency).ok_or(PriceParseStage::CurrencyShape)?;
        validate_currency(currency).map_err(|_| PriceParseStage::CurrencyShape)?;
        (Some(currency.to_owned()), number)
    };
    let (coefficient, scale) = decimal(number).map_err(|_| PriceParseStage::DecimalShape)?;
    let date_field = optional_qualified_any(block, &["98A", "98C"], ":PRIC//");
    let (date, time) = date_field
        .map(parse_qualified_timestamp)
        .transpose()
        .map_err(|_| PriceParseStage::PriceTimestamp)?
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
        .ok_or(malformed_securities_data!())?;
    let (unit, mut number) = rest.split_once('/').ok_or(malformed_securities_data!())?;
    let unit = match unit {
        "UNIT" => QuantityUnit::Units,
        "FAMT" => QuantityUnit::Nominal,
        _ => return Err(malformed_securities_data!()),
    };
    let negative = number.starts_with('N');
    if negative {
        if !allow_negative {
            return Err(malformed_securities_data!());
        }
        number = &number[1..];
    }
    let (coefficient, scale) = decimal(number)?;
    Ok(SecuritiesQuantity::new(coefficient, scale, unit, negative))
}

fn parse_signed_amount(value: &str, prefix: &str) -> Result<SecuritiesAmount, Error> {
    let mut rest = value
        .strip_prefix(prefix)
        .ok_or(malformed_securities_data!())?;
    let negative = rest.starts_with('N');
    if negative {
        rest = &rest[1..];
    }
    let (currency, number) = rest
        .split_at_checked(3)
        .ok_or(malformed_securities_data!())?;
    validate_currency(currency)?;
    let (coefficient, scale) = decimal(number)?;
    Ok(SecuritiesAmount {
        amount: Amount::new(coefficient, scale, currency.to_owned()),
        negative,
    })
}

fn parse_cost_basis(value: &str) -> Result<Option<SecurityPrice>, Error> {
    // Anlage 3 v3.9 4.3 and HBCI 2.2 IX.2.4 place the explicitly reported
    // acquisition amount and currency on structured HOLD line two. Their field
    // tables require the `1`/`2` line-number digits, but both full-message
    // examples omit them while retaining the same two-line structure. Accept
    // that independently printed shape only when line one is structurally
    // recognizable; arbitrary or ambiguous optional content remains absent.
    let body = value
        .strip_prefix(":HOLD//")
        .ok_or(malformed_securities_data!())?;
    let mut lines = body.split('\n');
    let first = lines.next().ok_or(malformed_securities_data!())?;
    let Some(second) = lines.next() else {
        return Ok(None);
    };
    let line = if first.starts_with('1') {
        second
            .strip_prefix('2')
            .ok_or(malformed_securities_data!())?
    } else if unnumbered_cost_basis_header(first) {
        second
    } else {
        return Err(malformed_securities_data!());
    };
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

fn unnumbered_cost_basis_header(value: &str) -> bool {
    let mut fields = value.split('+');
    let unit = fields.next();
    let transaction_type = fields.next();
    let depository = fields.next();
    let country = fields.next();
    let date = fields.next();
    unit.is_some_and(|field| {
        field.len() == 3 && field.bytes().all(|byte| byte.is_ascii_uppercase())
    }) && transaction_type
        .is_some_and(|field| field.len() == 3 && field.bytes().all(|byte| byte.is_ascii_digit()))
        && depository.is_some_and(|field| {
            field.len() == 5 && field.bytes().all(|byte| byte.is_ascii_digit())
        })
        && country.is_some_and(|field| {
            field.len() == 2 && field.bytes().all(|byte| byte.is_ascii_uppercase())
        })
        && date.is_some_and(|field| {
            field.len() == 8 && field.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn decimal(value: &str) -> Result<(u128, u8), Error> {
    if value.len() > 24 {
        return Err(malformed_securities_data!());
    }
    let (integer, fraction) = value.split_once(',').ok_or(malformed_securities_data!())?;
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(malformed_securities_data!());
    }
    let coefficient = format!("{integer}{fraction}")
        .parse()
        .map_err(|_| malformed_securities_data!())?;
    let scale = u8::try_from(fraction.len()).map_err(|_| malformed_securities_data!())?;
    Ok((coefficient, scale))
}

fn validate_currency(value: &str) -> Result<(), Error> {
    if value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        Ok(())
    } else {
        Err(malformed_securities_data!())
    }
}

fn parse_qualified_date(field: &Field) -> Result<NaiveDate, Error> {
    let value = field
        .value
        .rsplit_once("//")
        .map(|(_, value)| value)
        .ok_or(malformed_securities_data!())?;
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(malformed_securities_data!());
    }
    match field.tag.as_str() {
        "98A" if value.len() == 8 => parse_date(value),
        "98C" if value.len() == 14 => {
            let date = parse_date(&value[..8])?;
            NaiveTime::parse_from_str(&value[8..], "%H%M%S")
                .map_err(|_| malformed_securities_data!())?;
            Ok(date)
        }
        _ => Err(malformed_securities_data!()),
    }
}

fn parse_qualified_timestamp(
    field: &Field,
) -> Result<(Option<NaiveDate>, Option<NaiveTime>), Error> {
    let value = field
        .value
        .rsplit_once("//")
        .map(|(_, value)| value)
        .ok_or(malformed_securities_data!())?;
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(malformed_securities_data!());
    }
    match field.tag.as_str() {
        "98A" if value.len() == 8 => Ok((Some(parse_date(value)?), None)),
        "98C" if value.len() == 14 => Ok((
            Some(parse_date(&value[..8])?),
            Some(
                NaiveTime::parse_from_str(&value[8..], "%H%M%S")
                    .map_err(|_| malformed_securities_data!())?,
            ),
        )),
        _ => Err(malformed_securities_data!()),
    }
}

fn parse_date(value: &str) -> Result<NaiveDate, Error> {
    NaiveDate::parse_from_str(value, "%Y%m%d").map_err(|_| malformed_securities_data!())
}

fn page_more(field: &Field) -> Result<bool, Error> {
    let (_, indicator) = field
        .value
        .split_once('/')
        .ok_or(malformed_securities_data!())?;
    match indicator {
        "MORE" => Ok(true),
        "ONLY" | "LAST" => Ok(false),
        _ => Err(malformed_securities_data!()),
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
    let value = matching.next().ok_or(malformed_securities_data!())?;
    if matching.next().is_some() {
        return Err(malformed_securities_data!());
    }
    Ok(value)
}

fn optional_unique_child<'a>(block: &'a Block, name: &str) -> Result<Option<&'a Block>, Error> {
    let mut matching = block.children.iter().filter(|child| child.name == name);
    let value = matching.next();
    if matching.next().is_some() {
        return Err(malformed_securities_data!());
    }
    Ok(value)
}

fn fields<'a>(block: &'a Block, tag: &'a str) -> impl Iterator<Item = &'a Field> {
    block.fields.iter().filter(move |field| field.tag == tag)
}

fn required_field<'a>(block: &'a Block, tag: &str) -> Result<&'a Field, Error> {
    let mut matching = block.fields.iter().filter(|field| field.tag == tag);
    let value = matching.next().ok_or(malformed_securities_data!())?;
    if matching.next().is_some() {
        return Err(malformed_securities_data!());
    }
    Ok(value)
}

fn optional_field<'a>(block: &'a Block, tag: &str) -> Option<&'a Field> {
    block.fields.iter().find(|field| field.tag == tag)
}

fn required_qualified<'a>(block: &'a Block, tag: &str, prefix: &str) -> Result<&'a Field, Error> {
    optional_qualified(block, tag, prefix).ok_or(malformed_securities_data!())
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
    optional_qualified_any(block, tags, prefix).ok_or(malformed_securities_data!())
}

fn optional_qualified_any<'a>(block: &'a Block, tags: &[&str], prefix: &str) -> Option<&'a Field> {
    block
        .fields
        .iter()
        .find(|field| tags.contains(&field.tag.as_str()) && field.value.starts_with(prefix))
}
