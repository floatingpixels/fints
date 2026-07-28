use chrono::{NaiveDate, NaiveTime};

use crate::{
    error::{Error, Limitation},
    model::{
        Account, Credentials, InstituteId, ProductIdentity, ReusableState, TanMethod,
        TransactionFormat,
    },
    wire::{Element, Message, Segment, Value, encode_segments},
};

pub(crate) struct SecurityContext<'a> {
    pub(crate) institute: &'a InstituteId,
    pub(crate) credentials: &'a Credentials,
    pub(crate) system_id: &'a str,
    pub(crate) security_function: &'a str,
    pub(crate) profile_version: &'a str,
    pub(crate) dialog_id: &'a str,
    pub(crate) message_number: u16,
    pub(crate) date: NaiveDate,
    pub(crate) time: NaiveTime,
}

pub(crate) enum TanStep<'a> {
    Initial {
        operation: &'static str,
        medium_name: Option<&'a str>,
    },
    Submit {
        reference: &'a str,
    },
    Poll {
        reference: &'a str,
    },
}

pub(crate) fn anonymous_initialization(
    institute: &InstituteId,
    product: &ProductIdentity,
    state: &ReusableState,
) -> Result<Vec<u8>, Error> {
    let segments = vec![
        message_header("0", 1)?,
        segment(
            "HKIDN",
            2,
            2,
            None,
            vec![
                group(&[&institute.country_code, &institute.institute_code])?,
                text("9999999999")?,
                text("0")?,
                text("0")?,
            ],
        )?,
        hkvvb(3, product, state)?,
        message_trailer(4, 1)?,
    ];
    Ok(Message::new(segments).encode()?)
}

pub(crate) fn synchronization(
    context: &SecurityContext<'_>,
    product: &ProductIdentity,
    state: &ReusableState,
    method: Option<&TanMethod>,
    medium_name: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let mut operations = vec![
        raw_segment(
            "HKIDN",
            2,
            vec![
                group(&[
                    &context.institute.country_code,
                    &context.institute.institute_code,
                ])?,
                text(&context.credentials.customer_id)?,
                text("0")?,
                text("1")?,
            ],
        ),
        raw_hkvvb(product, state)?,
    ];
    if let Some(method) = method {
        operations.push(raw_hktan(
            method.hktan_version,
            TanStep::Initial {
                operation: "HKIDN",
                medium_name,
            },
        )?);
    }
    operations.push(raw_segment("HKSYN", 3, vec![text("0")?]));
    authenticated(context, operations, None)
}

pub(crate) fn initialization(
    context: &SecurityContext<'_>,
    product: &ProductIdentity,
    state: &ReusableState,
    method: Option<&TanMethod>,
    tan_operation: &'static str,
    medium_name: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let mut operations = vec![
        raw_segment(
            "HKIDN",
            2,
            vec![
                group(&[
                    &context.institute.country_code,
                    &context.institute.institute_code,
                ])?,
                text(&context.credentials.customer_id)?,
                text(context.system_id)?,
                text("1")?,
            ],
        ),
        raw_hkvvb(product, state)?,
    ];
    if let Some(method) = method {
        operations.push(raw_hktan(
            method.hktan_version,
            TanStep::Initial {
                operation: tan_operation,
                medium_name,
            },
        )?);
    }
    authenticated(context, operations, None)
}

pub(crate) fn balance_request(
    context: &SecurityContext<'_>,
    account: &Account,
    version: u16,
    tan: Option<(&TanMethod, Option<&str>)>,
) -> Result<Vec<u8>, Error> {
    if !(6..=8).contains(&version) {
        return Err(Limitation::BalanceVersion.into());
    }
    let account_element = if version == 6 {
        national_account(account)?
    } else {
        international_account(account)?
    };
    let mut operations = vec![raw_segment(
        "HKSAL",
        version,
        vec![account_element, text("N")?],
    )];
    if let Some((method, medium)) = tan {
        operations.push(raw_hktan(
            method.hktan_version,
            TanStep::Initial {
                operation: "HKSAL",
                medium_name: medium,
            },
        )?);
    }
    authenticated(context, operations, None)
}

pub(crate) struct TransactionRequest<'a> {
    pub(crate) account: &'a Account,
    pub(crate) format: &'a TransactionFormat,
    pub(crate) from: Option<NaiveDate>,
    pub(crate) to: Option<NaiveDate>,
    pub(crate) continuation_point: Option<&'a str>,
}

pub(crate) fn transaction_request(
    context: &SecurityContext<'_>,
    request: TransactionRequest<'_>,
    tan: Option<(&TanMethod, Option<&str>)>,
) -> Result<Vec<u8>, Error> {
    let (operation, version, mut elements) = match request.format {
        TransactionFormat::Camt { descriptor } => (
            "HKCAZ",
            1,
            vec![
                international_account(request.account)?,
                group(&[descriptor])?,
                text("N")?,
            ],
        ),
        TransactionFormat::Mt940 { version: 6 } => (
            "HKKAZ",
            6,
            vec![national_account(request.account)?, text("N")?],
        ),
        TransactionFormat::Mt940 { version: 7 } => (
            "HKKAZ",
            7,
            vec![international_account(request.account)?, text("N")?],
        ),
        TransactionFormat::Mt940 { .. } => return Err(Limitation::TransactionsVersion.into()),
    };
    elements.push(optional_date(request.from)?);
    elements.push(optional_date(request.to)?);
    if request.continuation_point.is_some() {
        elements.push(text("")?);
        elements.push(text(request.continuation_point.unwrap_or_default())?);
    } else {
        while elements.last().is_some_and(element_is_empty) {
            elements.pop();
        }
    }
    let mut operations = vec![raw_segment(operation, version, elements)];
    if let Some((method, medium_name)) = tan {
        operations.push(raw_hktan(
            method.hktan_version,
            TanStep::Initial {
                operation,
                medium_name,
            },
        )?);
    }
    authenticated(context, operations, None)
}

pub(crate) fn tan_submission(
    context: &SecurityContext<'_>,
    method: &TanMethod,
    reference: &str,
    tan: &str,
) -> Result<Vec<u8>, Error> {
    authenticated(
        context,
        vec![raw_hktan(
            method.hktan_version,
            TanStep::Submit { reference },
        )?],
        Some(tan),
    )
}

pub(crate) fn decoupled_poll(
    context: &SecurityContext<'_>,
    method: &TanMethod,
    reference: &str,
) -> Result<Vec<u8>, Error> {
    if method.hktan_version != 7 {
        return Err(Limitation::TanMethod.into());
    }
    authenticated(
        context,
        vec![raw_hktan(7, TanStep::Poll { reference })?],
        None,
    )
}

pub(crate) fn termination(context: &SecurityContext<'_>) -> Result<Vec<u8>, Error> {
    authenticated(
        context,
        vec![raw_segment("HKEND", 1, vec![text(context.dialog_id)?])],
        None,
    )
}

pub(crate) fn anonymous_termination(
    dialog_id: &str,
    message_number: u16,
) -> Result<Vec<u8>, Error> {
    Ok(Message::new(vec![
        message_header(dialog_id, message_number)?,
        segment("HKEND", 2, 1, None, vec![text(dialog_id)?])?,
        message_trailer(3, message_number)?,
    ])
    .encode()?)
}

fn authenticated(
    context: &SecurityContext<'_>,
    operations: Vec<RawSegment>,
    tan: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let control_reference = context.message_number.to_string();
    let mut inner = Vec::new();
    inner.push(build_segment(
        2,
        raw_signature_header(context, &control_reference)?,
    )?);
    for operation in operations {
        let number = u16::try_from(inner.len() + 2).map_err(|_| Error::InconsistentState)?;
        inner.push(build_segment(number, operation)?);
    }
    let signature_number = u16::try_from(inner.len() + 2).map_err(|_| Error::InconsistentState)?;
    inner.push(build_segment(
        signature_number,
        raw_signature_trailer(context, &control_reference, tan)?,
    )?);
    let trailer_number = u16::try_from(inner.len() + 2).map_err(|_| Error::InconsistentState)?;
    let encrypted_payload = encode_segments(&inner)?;

    let outer = vec![
        message_header(context.dialog_id, context.message_number)?,
        build_segment(998, raw_encryption_header(context)?)?,
        segment(
            "HNVSD",
            999,
            1,
            None,
            vec![Element::new(vec![Value::binary(encrypted_payload)])],
        )?,
        message_trailer(trailer_number, context.message_number)?,
    ];
    Ok(Message::new(outer).encode()?)
}

struct RawSegment {
    code: &'static str,
    version: u16,
    elements: Vec<Element>,
}

fn raw_segment(code: &'static str, version: u16, elements: Vec<Element>) -> RawSegment {
    RawSegment {
        code,
        version,
        elements,
    }
}

fn build_segment(number: u16, raw: RawSegment) -> Result<Segment, Error> {
    segment(raw.code, number, raw.version, None, raw.elements)
}

fn raw_hkvvb(product: &ProductIdentity, state: &ReusableState) -> Result<RawSegment, Error> {
    Ok(raw_segment(
        "HKVVB",
        3,
        vec![
            text(&state.bpd_version.to_string())?,
            text(&state.upd_version.to_string())?,
            text("0")?,
            text(&product.registration_id)?,
            text(&product.version)?,
        ],
    ))
}

fn hkvvb(number: u16, product: &ProductIdentity, state: &ReusableState) -> Result<Segment, Error> {
    build_segment(number, raw_hkvvb(product, state)?)
}

fn raw_signature_header(
    context: &SecurityContext<'_>,
    control_reference: &str,
) -> Result<RawSegment, Error> {
    Ok(raw_segment(
        "HNSHK",
        4,
        vec![
            group(&["PIN", context.profile_version])?,
            text(context.security_function)?,
            text(control_reference)?,
            text("1")?,
            text("1")?,
            group(&["1", "", context.system_id])?,
            text("1")?,
            group(&[
                "1",
                &context.date.format("%Y%m%d").to_string(),
                &context.time.format("%H%M%S").to_string(),
            ])?,
            group(&["1", "999", "1"])?,
            group(&["6", "10", "16"])?,
            group(&[
                &context.institute.country_code,
                &context.institute.institute_code,
                &context.credentials.user_id,
                "S",
                "0",
                "0",
            ])?,
        ],
    ))
}

fn raw_signature_trailer(
    context: &SecurityContext<'_>,
    control_reference: &str,
    tan: Option<&str>,
) -> Result<RawSegment, Error> {
    let mut signature = vec![Value::text(&context.credentials.pin)?];
    if let Some(tan) = tan {
        signature.push(Value::text(tan)?);
    }
    Ok(raw_segment(
        "HNSHA",
        2,
        vec![text(control_reference)?, text("")?, Element::new(signature)],
    ))
}

fn raw_encryption_header(context: &SecurityContext<'_>) -> Result<RawSegment, Error> {
    Ok(raw_segment(
        "HNVSK",
        3,
        vec![
            group(&["PIN", context.profile_version])?,
            text("998")?,
            text("1")?,
            group(&["1", "", context.system_id])?,
            group(&[
                "1",
                &context.date.format("%Y%m%d").to_string(),
                &context.time.format("%H%M%S").to_string(),
            ])?,
            Element::new(vec![
                Value::text("2")?,
                Value::text("2")?,
                Value::text("13")?,
                Value::binary(vec![0; 8]),
                Value::text("5")?,
                Value::text("1")?,
            ]),
            group(&[
                &context.institute.country_code,
                &context.institute.institute_code,
                &context.credentials.user_id,
                "V",
                "0",
                "0",
            ])?,
            text("0")?,
        ],
    ))
}

fn raw_hktan(version: u16, step: TanStep<'_>) -> Result<RawSegment, Error> {
    if !(6..=7).contains(&version) {
        return Err(Limitation::TanMethod.into());
    }
    let fields = match step {
        TanStep::Initial {
            operation,
            medium_name,
        } => {
            let mut fields = vec![
                text("4")?,
                text(operation)?,
                text("")?,
                text("")?,
                text("")?,
                text("")?,
                text("")?,
                text("")?,
                text("")?,
                text("")?,
            ];
            if let Some(medium_name) = medium_name {
                fields.push(text(medium_name)?);
            }
            fields
        }
        TanStep::Submit { reference } => vec![
            text("2")?,
            text("")?,
            text("")?,
            text("")?,
            text(reference)?,
            text("N")?,
        ],
        TanStep::Poll { reference } => vec![
            text("S")?,
            text("")?,
            text("")?,
            text("")?,
            text(reference)?,
            text("N")?,
        ],
    };
    Ok(raw_segment("HKTAN", version, fields))
}

fn national_account(account: &Account) -> Result<Element, Error> {
    let institute = account
        .institute
        .as_ref()
        .ok_or(Limitation::BalanceVersion)?;
    group(&[
        account
            .account_number
            .as_deref()
            .ok_or(Limitation::BalanceVersion)?,
        account.subaccount.as_deref().unwrap_or(""),
        &institute.country_code,
        &institute.institute_code,
    ])
}

fn international_account(account: &Account) -> Result<Element, Error> {
    let (country, institute) = account
        .institute
        .as_ref()
        .map(|value| (value.country_code.as_str(), value.institute_code.as_str()))
        .unwrap_or(("", ""));
    if account.iban.is_none() && account.account_number.is_none() {
        return Err(Limitation::BalanceVersion.into());
    }
    group(&[
        account.iban.as_deref().unwrap_or(""),
        account.bic.as_deref().unwrap_or(""),
        account.account_number.as_deref().unwrap_or(""),
        account.subaccount.as_deref().unwrap_or(""),
        country,
        institute,
    ])
}

fn message_header(dialog_id: &str, message_number: u16) -> Result<Segment, Error> {
    segment(
        "HNHBK",
        1,
        3,
        None,
        vec![
            text("000000000000")?,
            text("300")?,
            text(dialog_id)?,
            text(&message_number.to_string())?,
        ],
    )
}

fn message_trailer(segment_number: u16, message_number: u16) -> Result<Segment, Error> {
    segment(
        "HNHBS",
        segment_number,
        1,
        None,
        vec![text(&message_number.to_string())?],
    )
}

fn segment(
    code: &str,
    number: u16,
    version: u16,
    reference: Option<u16>,
    mut elements: Vec<Element>,
) -> Result<Segment, Error> {
    let mut header = vec![
        Value::text(code)?,
        Value::text(&number.to_string())?,
        Value::text(&version.to_string())?,
    ];
    if let Some(reference) = reference {
        header.push(Value::text(&reference.to_string())?);
    }
    elements.insert(0, Element::new(header));
    Ok(Segment::new(elements))
}

fn text(value: &str) -> Result<Element, Error> {
    Ok(Element::text(value)?)
}

fn group(values: &[&str]) -> Result<Element, Error> {
    Ok(Element::new(
        values
            .iter()
            .map(|value| Value::text(value))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn optional_date(value: Option<NaiveDate>) -> Result<Element, Error> {
    text(
        &value
            .map(|date| date.format("%Y%m%d").to_string())
            .unwrap_or_default(),
    )
}

fn element_is_empty(element: &Element) -> bool {
    element
        .components()
        .iter()
        .all(|value| value.as_text().is_some_and(|text| text.is_empty()))
}

#[cfg(test)]
mod tests;
