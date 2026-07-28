use chrono::{NaiveDate, NaiveTime};

use crate::{
    error::Error,
    model::{Challenge, Timestamp},
    wire::{Segment, Value},
};

use super::{component, optional_component};

pub(crate) struct TanResponse {
    pub(crate) process: String,
    pub(crate) challenge: Challenge,
}

pub(super) fn parse(
    segments: &[Segment],
    expected_version: u16,
) -> Result<Option<TanResponse>, Error> {
    let Some(segment) = segments.iter().find(|segment| {
        segment
            .header()
            .is_some_and(|header| header.code == b"HITAN")
    }) else {
        return Ok(None);
    };
    let actual_version = segment.header().expect("header checked").version;
    if actual_version != expected_version || !(6..=7).contains(&actual_version) {
        return Err(Error::UnsupportedSegment {
            code: "HITAN",
            version: actual_version,
        });
    }
    let elements = segment.elements();
    let process = single_text(elements, 1, "HITAN process")?;
    if !matches!(process.as_str(), "1" | "2" | "3" | "4" | "S") {
        return Err(Error::InvalidValue {
            field: "HITAN process",
        });
    }
    let reference = optional_single_text(elements, 3).ok_or(Error::MissingValue {
        field: "HITAN order reference",
    })?;
    let text = optional_single_text(elements, 4).filter(|value| value != "nochallenge");
    let hhd_uc = optional_binary(elements, 5);
    let expires_at = optional_element(elements, 6)
        .map(parse_timestamp)
        .transpose()?;
    let medium_name = optional_single_text(elements, 7);

    Ok(Some(TanResponse {
        process,
        challenge: Challenge {
            reference,
            text,
            hhd_uc,
            expires_at,
            medium_name,
        },
    }))
}

fn parse_timestamp(components: &[Value]) -> Result<Timestamp, Error> {
    let date = NaiveDate::parse_from_str(
        &component(components, 0, "challenge expiry date")?,
        "%Y%m%d",
    )
    .map_err(|_| Error::InvalidValue {
        field: "challenge expiry date",
    })?;
    let time = optional_component(components, 1)
        .map(|value| {
            NaiveTime::parse_from_str(&value, "%H%M%S").map_err(|_| Error::InvalidValue {
                field: "challenge expiry time",
            })
        })
        .transpose()?;
    Ok(Timestamp::new(date, time))
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

fn optional_single_text(elements: &[crate::wire::Element], index: usize) -> Option<String> {
    elements
        .get(index)
        .and_then(|element| optional_component(element.components(), 0))
}

fn optional_binary(elements: &[crate::wire::Element], index: usize) -> Option<Vec<u8>> {
    elements
        .get(index)
        .and_then(|element| element.components().first())
        .and_then(|value| value.as_binary())
        .map(<[u8]>::to_vec)
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
