use crate::{
    error::{Error, malformed_transaction_data},
    model::{Account, BookedEntry, InstituteState, TransactionFormat},
    wire::{Segment, Value},
};

use super::{camt_descriptor_matches, optional_component};

mod camt;
mod mt940;

const MAX_TRANSACTION_PAGE_ENTRIES: usize = 5_000;

pub(crate) struct TransactionPage {
    pub(crate) account: Option<Account>,
    pub(crate) entries: Vec<BookedEntry>,
}

pub(super) fn parse(
    segments: &[Segment],
    format: &TransactionFormat,
) -> Result<Option<TransactionPage>, Error> {
    match format {
        TransactionFormat::Camt { descriptor } => parse_camt(segments, descriptor),
        TransactionFormat::Mt940 { version } => parse_mt940(segments, *version),
    }
}

fn parse_camt(
    segments: &[Segment],
    expected_descriptor: &str,
) -> Result<Option<TransactionPage>, Error> {
    let mut account = None;
    let mut entries = Vec::new();
    let mut found = false;
    for segment in matching_segments(segments, b"HICAZ") {
        found = true;
        let header = segment.header().ok_or(Error::InvalidResponse {
            structure: "HICAZ segment header",
        })?;
        if header.version != 1 {
            return Err(Error::UnsupportedSegment {
                code: "HICAZ",
                version: header.version,
            });
        }
        let parsed_account = parse_international_account(
            segment
                .elements()
                .get(1)
                .ok_or(Error::MissingValue {
                    field: "HICAZ account",
                })?
                .components(),
        )?;
        if account
            .as_ref()
            .is_some_and(|current: &Account| !current.same_identity(&parsed_account))
        {
            return Err(Error::InconsistentState);
        }
        account = Some(parsed_account);
        let descriptor = single_text(segment, 2, "HICAZ camt descriptor")?;
        // Messages 2022 echoes the negotiated camt descriptor in HICAZ.
        // Descriptor identifiers are compared case-insensitively and permit
        // the optional schema-file suffix used by the DD.
        if !camt_descriptor_matches(&descriptor, expected_descriptor) {
            return Err(Error::InvalidValue {
                field: "HICAZ camt descriptor",
            });
        }
        // Messages 2022 Data Dictionary, "Gebuchte camt-Umsätze":
        // "camt-Umsätze gebucht" has repetition M n, normally one binary camt.052
        // document per booking day. G97 additionally requires accepting a single
        // document that itself covers multiple booking days.
        for booked in binary_components(segment, 3, "HICAZ booked camt payload")? {
            let payload = camt::parse(booked)?;
            ensure_camt_account(account.as_ref().ok_or(Error::InconsistentState)?, &payload)?;
            if entries
                .len()
                .checked_add(payload.entries.len())
                .is_none_or(|count| count > MAX_TRANSACTION_PAGE_ENTRIES)
            {
                return Err(malformed_transaction_data!());
            }
            entries.extend(payload.entries);
        }
    }
    Ok(found.then_some(TransactionPage { account, entries }))
}

fn parse_mt940(
    segments: &[Segment],
    expected_version: u16,
) -> Result<Option<TransactionPage>, Error> {
    if !(6..=7).contains(&expected_version) {
        return Err(crate::Limitation::TransactionsVersion.into());
    }
    let mut entries = Vec::new();
    let mut found = false;
    for segment in matching_segments(segments, b"HIKAZ") {
        found = true;
        let header = segment.header().ok_or(Error::InvalidResponse {
            structure: "HIKAZ segment header",
        })?;
        if header.version != expected_version {
            return Err(Error::UnsupportedSegment {
                code: "HIKAZ",
                version: header.version,
            });
        }
        let booked = single_binary(segment, 1, "HIKAZ booked MT940 payload")?;
        let parsed = mt940::parse(booked)?;
        if entries
            .len()
            .checked_add(parsed.len())
            .is_none_or(|count| count > MAX_TRANSACTION_PAGE_ENTRIES)
        {
            return Err(malformed_transaction_data!());
        }
        entries.extend(parsed);
    }
    Ok(found.then_some(TransactionPage {
        account: None,
        entries,
    }))
}

fn matching_segments<'a>(
    segments: &'a [Segment],
    code: &'static [u8],
) -> impl Iterator<Item = &'a Segment> {
    segments
        .iter()
        .filter(move |segment| segment.header().is_some_and(|header| header.code == code))
}

fn parse_international_account(components: &[Value]) -> Result<Account, Error> {
    let iban = optional_component(components, 0);
    let bic = optional_component(components, 1);
    let account_number = optional_component(components, 2);
    let subaccount = optional_component(components, 3);
    let institute = match (
        optional_component(components, 4),
        optional_component(components, 5),
    ) {
        (Some(country_code), Some(institute_code)) => Some(InstituteState {
            country_code,
            institute_code,
        }),
        (None, None) => None,
        _ => {
            return Err(Error::InvalidResponse {
                structure: "HICAZ account institute",
            });
        }
    };
    if iban.is_none() && account_number.is_none() {
        return Err(Error::MissingValue {
            field: "HICAZ account identity",
        });
    }
    Ok(Account {
        iban,
        bic,
        account_number,
        subaccount,
        institute,
        currency: None,
        account_type: None,
        owner_name_1: None,
        owner_name_2: None,
        product_name: None,
        allowed_operations: Vec::new(),
        unlisted_operations_unknown: false,
    })
}

fn ensure_camt_account(account: &Account, payload: &camt::CamtPayload) -> Result<(), Error> {
    let iban_match = match (account.iban.as_deref(), payload.iban.as_deref()) {
        (Some(left), Some(right)) => Some(left == right),
        _ => None,
    };
    let account_number_match = match (
        account.account_number.as_deref(),
        payload.other_account_id.as_deref(),
    ) {
        (Some(left), Some(right)) => Some(left == right),
        _ => None,
    };
    if iban_match == Some(false)
        || account_number_match == Some(false)
        || !matches!(
            (iban_match, account_number_match),
            (Some(true), _) | (_, Some(true))
        )
    {
        Err(Error::InconsistentState)
    } else {
        Ok(())
    }
}

fn single_text(segment: &Segment, index: usize, field: &'static str) -> Result<String, Error> {
    let components = segment
        .elements()
        .get(index)
        .ok_or(Error::MissingValue { field })?
        .components();
    if components.len() != 1 {
        return Err(Error::InvalidResponse { structure: field });
    }
    optional_component(components, 0).ok_or(Error::MissingValue { field })
}

fn binary_components<'a>(
    segment: &'a Segment,
    index: usize,
    field: &'static str,
) -> Result<Vec<&'a [u8]>, Error> {
    let components = segment
        .elements()
        .get(index)
        .ok_or(Error::MissingValue { field })?
        .components();
    if components.is_empty() {
        return Err(Error::MissingValue { field });
    }
    components
        .iter()
        .map(|component| component.as_binary().ok_or(Error::InvalidValue { field }))
        .collect()
}

fn single_binary<'a>(
    segment: &'a Segment,
    index: usize,
    field: &'static str,
) -> Result<&'a [u8], Error> {
    let components = segment
        .elements()
        .get(index)
        .ok_or(Error::MissingValue { field })?
        .components();
    if components.len() != 1 {
        return Err(Error::InvalidResponse { structure: field });
    }
    components[0]
        .as_binary()
        .ok_or(Error::InvalidValue { field })
}

#[cfg(feature = "fuzzing")]
pub(super) fn fuzz_camt(input: &[u8]) -> bool {
    camt::parse(input).is_ok()
}

#[cfg(feature = "fuzzing")]
pub(super) fn fuzz_mt940(input: &[u8]) -> bool {
    mt940::parse(input).is_ok()
}
