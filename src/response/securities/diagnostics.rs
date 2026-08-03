//! Opt-in, value-free structural facts for MT535/MT536 depot responses.
//!
//! Everything here exists only with the `diagnostics` feature and never
//! drives control flow; fact shapes are explicitly unstable.

use super::*;

pub(in crate::response) fn position_structure(
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

pub(in crate::response) fn transaction_structure(
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

pub(super) fn diagnostic_position_failure_site(
    site: PositionFailureSite,
) -> crate::DepotPositionFailureSite {
    match site {
        PositionFailureSite::Instrument => crate::DepotPositionFailureSite::Instrument,
        PositionFailureSite::Quantity => crate::DepotPositionFailureSite::Quantity,
        PositionFailureSite::Price => crate::DepotPositionFailureSite::Price,
        PositionFailureSite::MarketValue => crate::DepotPositionFailureSite::MarketValue,
        PositionFailureSite::CostBasis => crate::DepotPositionFailureSite::CostBasis,
    }
}

pub(super) fn block_inventory(root: &Block) -> Vec<crate::DepotBlockFact> {
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

pub(super) fn tag_inventory(root: &Block) -> Vec<crate::DepotTagFact> {
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

pub(super) fn price_shapes(
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

fn split_currency_candidate(value: &str) -> Option<(&str, &str)> {
    let mut characters = value.char_indices();
    characters.next()?;
    characters.next()?;
    characters.next()?;
    let boundary = characters.next().map_or(value.len(), |(index, _)| index);
    Some(value.split_at(boundary))
}

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

pub(super) fn first_price_failure(
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
