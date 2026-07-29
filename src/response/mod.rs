use crate::{
    error::{BankResponse, Error, Recovery, ResponseClass},
    model::{Balance, ReusableState, TanMedium, TransactionFormat},
    wire::{Message, Segment},
};

mod balance;
mod credit_card;
mod parameters;
mod securities;
mod tan;
mod transactions;

pub(crate) struct Response {
    dialog_id: String,
    message_number: u16,
    segments: Vec<Segment>,
    responses: Vec<BankResponse>,
    allowed_tan_methods: Vec<String>,
    has_tan_method_response: bool,
    continuation_points: Vec<ContinuationPoint>,
}

struct ContinuationPoint {
    segment_number: u16,
    value: String,
}

impl Response {
    pub(crate) fn parse(input: &[u8]) -> Result<Self, Error> {
        let message = Message::parse(input)?;
        let dialog_id = required_message_text(&message, 3, "dialog ID")?;
        let message_number = required_message_text(&message, 4, "message number")?
            .parse()
            .map_err(|_| Error::InvalidValue {
                field: "message number",
            })?;
        let segments = response_segments(&message)?;
        let first_header =
            segments
                .first()
                .and_then(Segment::header)
                .ok_or(Error::MissingValue {
                    field: "HIRMG response segment",
                })?;
        if first_header.code != b"HIRMG" {
            return Err(Error::InvalidResponse {
                structure: "HIRMG must be the first response segment",
            });
        }
        let mut responses = Vec::new();
        let mut allowed_tan_methods = Vec::new();
        let mut has_message_response = false;
        let mut has_tan_method_response = false;
        let mut continuation_points = Vec::new();

        for segment in &segments {
            let header = segment.header().ok_or(Error::InvalidResponse {
                structure: "segment header",
            })?;
            match header.code {
                b"HIRMG" | b"HIRMS" => {
                    if header.code == b"HIRMG" && has_message_response {
                        return Err(Error::InvalidResponse {
                            structure: "duplicate HIRMG response segment",
                        });
                    }
                    if header.version != 2 {
                        return Err(Error::UnsupportedSegment {
                            code: if header.code == b"HIRMG" {
                                "HIRMG"
                            } else {
                                "HIRMS"
                            },
                            version: header.version,
                        });
                    }
                    if header.code == b"HIRMG" && segment.elements().len() == 1 {
                        return Err(Error::InvalidResponse {
                            structure: "HIRMG without response elements",
                        });
                    }
                    has_message_response |= header.code == b"HIRMG";
                    let reference = (header.code == b"HIRMS")
                        .then_some(header.reference)
                        .flatten();
                    for element in &segment.elements()[1..] {
                        let components = element.components();
                        let code = component(components, 0, "response code")?;
                        if code.len() != 4 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
                            return Err(Error::InvalidValue {
                                field: "response code",
                            });
                        }
                        let numeric_code = code.parse().map_err(|_| Error::InvalidValue {
                            field: "response code",
                        })?;
                        let class = match numeric_code / 1000 {
                            0 => ResponseClass::Success,
                            3 => ResponseClass::Warning,
                            9 => ResponseClass::Error,
                            _ => {
                                return Err(Error::InvalidValue {
                                    field: "response code class",
                                });
                            }
                        };
                        responses.push(BankResponse::new(
                            numeric_code,
                            class,
                            reference,
                            recovery_for(numeric_code),
                        ));
                        if numeric_code == 3040 {
                            let segment_number = reference.ok_or(Error::InvalidResponse {
                                structure: "3040 without referenced request segment",
                            })?;
                            let value = component(components, 3, "continuation point")?;
                            if encoding_rs::mem::encode_latin1_lossy(&value).len() > 35 {
                                return Err(Error::InvalidValue {
                                    field: "continuation point",
                                });
                            }
                            continuation_points.push(ContinuationPoint {
                                segment_number,
                                value,
                            });
                        }
                        if numeric_code == 3920 {
                            has_tan_method_response = true;
                            allowed_tan_methods.extend(
                                components
                                    .iter()
                                    .skip(3)
                                    .filter_map(|value| value.as_text())
                                    .filter(|value| {
                                        value.len() == 3
                                            && value.bytes().all(|byte| byte.is_ascii_digit())
                                    })
                                    .map(|value| value.into_owned()),
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        if !has_message_response {
            return Err(Error::MissingValue {
                field: "HIRMG response segment",
            });
        }
        Ok(Self {
            dialog_id,
            message_number,
            segments,
            responses,
            allowed_tan_methods,
            has_tan_method_response,
            continuation_points,
        })
    }

    pub(crate) fn dialog_id(&self) -> &str {
        &self.dialog_id
    }

    pub(crate) fn message_number(&self) -> u16 {
        self.message_number
    }

    pub(crate) fn responses(&self) -> &[BankResponse] {
        &self.responses
    }

    pub(crate) fn allowed_tan_methods(&self) -> &[String] {
        &self.allowed_tan_methods
    }

    pub(crate) fn has_tan_method_response(&self) -> bool {
        self.has_tan_method_response
    }

    pub(crate) fn first_error(&self) -> Option<BankResponse> {
        self.responses
            .iter()
            .copied()
            .find(|response| response.class() == ResponseClass::Error)
    }

    pub(crate) fn apply_parameters(
        &self,
        state: &mut ReusableState,
    ) -> Result<Option<Vec<crate::model::Account>>, Error> {
        parameters::apply(&self.segments, state)
    }

    pub(crate) fn bpd_institute(&self) -> Result<Option<crate::model::InstituteState>, Error> {
        parameters::bpd_institute(&self.segments)
    }

    pub(crate) fn system_id(&self) -> Result<Option<String>, Error> {
        parameters::system_id(&self.segments)
    }

    pub(crate) fn tan_media(&self) -> Result<Vec<TanMedium>, Error> {
        parameters::tan_media(&self.segments)
    }

    pub(crate) fn balance(&self) -> Result<Option<Balance>, Error> {
        balance::parse(&self.segments)
    }

    pub(crate) fn transactions(
        &self,
        format: &TransactionFormat,
    ) -> Result<Option<transactions::TransactionPage>, Error> {
        transactions::parse(&self.segments, format)
    }

    pub(crate) fn depot_positions(&self) -> Result<Option<securities::DepotPositionPage>, Error> {
        securities::positions(&self.segments)
    }

    pub(crate) fn securities_transactions(
        &self,
    ) -> Result<Option<securities::SecuritiesTransactionPage>, Error> {
        securities::transactions(&self.segments)
    }

    pub(crate) fn credit_card_transactions(
        &self,
    ) -> Result<Option<credit_card::CreditCardTransactionPage>, Error> {
        credit_card::transactions(&self.segments)
    }

    pub(crate) fn credit_card_balance(
        &self,
    ) -> Result<Option<credit_card::CreditCardBalanceFields>, Error> {
        credit_card::balance(&self.segments)
    }

    pub(crate) fn has_segment(&self, code: &[u8]) -> bool {
        self.segments
            .iter()
            .any(|segment| segment.header().is_some_and(|header| header.code == code))
    }

    pub(crate) fn continuation_point(&self, segment_number: u16) -> Result<Option<&str>, Error> {
        let mut points = self
            .continuation_points
            .iter()
            .filter(|point| point.segment_number == segment_number);
        let point = points.next();
        if points.next().is_some() {
            return Err(Error::InvalidResponse {
                structure: "multiple continuation points for one request",
            });
        }
        Ok(point.map(|point| point.value.as_str()))
    }

    pub(crate) fn tan(&self, expected_version: u16) -> Result<Option<tan::TanResponse>, Error> {
        tan::parse(&self.segments, expected_version)
    }
}

// FinTS Formals B.7.1 and B.8 restore the logical institute-response order
// inside HNVSD. PIN/TAN B.1 and F.2 retain an optional HNSHK/HNSHA control
// pair around HIRMG, HIRMS, and response data, without bank-signature material.
fn response_segments(message: &Message) -> Result<Vec<Segment>, Error> {
    let payload = message.payload_segments()?;
    let security_enveloped = message.is_security_enveloped();
    if security_enveloped {
        validate_encryption_header(
            message
                .segments()
                .get(1)
                .ok_or_else(|| invalid_encryption_header("encryption_header.segment_header"))?,
        )?;
    }
    let signature_headers = payload
        .iter()
        .filter(|segment| {
            segment
                .header()
                .is_some_and(|header| header.code == b"HNSHK")
        })
        .count();
    let signature_trailers = payload
        .iter()
        .filter(|segment| {
            segment
                .header()
                .is_some_and(|header| header.code == b"HNSHA")
        })
        .count();

    if signature_headers == 0 && signature_trailers == 0 {
        return Ok(payload);
    }
    if !security_enveloped || signature_headers != 1 || signature_trailers != 1 || payload.len() < 3
    {
        return Err(Error::InvalidResponse {
            structure: "authenticated response security controls",
        });
    }

    let signature_header = payload.first().ok_or(Error::InvalidResponse {
        structure: "authenticated response security controls",
    })?;
    let signature_trailer = payload.last().ok_or(Error::InvalidResponse {
        structure: "authenticated response security controls",
    })?;
    validate_signature_header(signature_header)?;
    let control_reference = signature_control_reference(signature_header)?;
    validate_signature_trailer(signature_trailer, &control_reference)?;

    Ok(payload[1..payload.len() - 1].to_vec())
}

// HBCI Security B.5.3 supplies the shared HNVSK 3 layout; PIN/TAN
// B.9.1 and B.9.8-B.9.9 constrain it to the cleartext-over-TLS profile.
// The PIN/TAN definition of FinTS-Füllwert makes the key bytes and key
// parameter identifier processing-irrelevant: only format and restrictions are checked.
fn validate_encryption_header(segment: &Segment) -> Result<(), Error> {
    let header = segment
        .header()
        .ok_or_else(|| invalid_encryption_header("encryption_header.segment_header"))?;
    if header.code != b"HNVSK"
        || header.number != 998
        || header.version != 3
        || header.reference.is_some()
    {
        return Err(invalid_encryption_header(
            "encryption_header.segment_header",
        ));
    }

    if !(9..=10).contains(&segment.elements().len())
        || !element_has_components(segment, 1, 2)
        || !(2..=3).all(|index| element_has_components(segment, index, 1))
        || !element_has_components(segment, 4, 3)
        || segment
            .element(5)
            .is_none_or(|element| !(1..=3).contains(&element.components().len()))
        || segment
            .element(6)
            .is_none_or(|element| !(6..=7).contains(&element.components().len()))
        || !element_has_components(segment, 7, 6)
        || !element_has_components(segment, 8, 1)
        || segment
            .element(9)
            .is_some_and(|element| !element_is_empty(element))
    {
        return Err(invalid_encryption_header("encryption_header.element_shape"));
    }

    let profile = segment.elements()[1].components();
    if profile[0].as_text().as_deref() != Some("PIN")
        || !matches!(profile[1].as_text().as_deref(), Some("1" | "2"))
    {
        return Err(invalid_encryption_header(
            "encryption_header.security_profile",
        ));
    }

    if segment.elements()[2].components()[0].as_text().as_deref() != Some("998") {
        return Err(invalid_encryption_header(
            "encryption_header.security_function",
        ));
    }

    // The HBCI Security Data Dictionary says the role is not to be interpreted
    // currently. All three defined code values remain syntactically valid.
    if !matches!(
        segment.elements()[3].components()[0].as_text().as_deref(),
        Some("1" | "3" | "4")
    ) {
        return Err(invalid_encryption_header("encryption_header.security_role"));
    }

    let security_identity = segment.elements()[4].components();
    // The shared Data Dictionary defines 1 (sender) and 2 (receiver), while
    // PIN/TAN B.9.3 forbids CID and requires the customer system identifier.
    if !matches!(security_identity[0].as_text().as_deref(), Some("1" | "2"))
        || security_identity[1].as_text().as_deref() != Some("")
        || security_identity[2]
            .as_text()
            .is_none_or(|value| value.is_empty())
    {
        return Err(invalid_encryption_header(
            "encryption_header.security_identity_shape",
        ));
    }

    if !valid_security_timestamp(segment.elements()[5].components()) {
        return Err(invalid_encryption_header(
            "encryption_header.security_timestamp_shape",
        ));
    }

    let encryption = segment.elements()[6].components();
    if encryption[0].as_text().as_deref() != Some("2")
        || encryption[1].as_text().as_deref() != Some("2")
        || encryption[2].as_text().as_deref() != Some("13")
    {
        return Err(invalid_encryption_header(
            "encryption_header.algorithm_codes",
        ));
    }
    if encryption[3]
        .as_binary()
        .is_none_or(|value| value.len() > 512)
    {
        return Err(invalid_encryption_header(
            "encryption_header.key_filler_shape",
        ));
    }
    if !valid_key_identifier_filler(&encryption[4]) {
        return Err(invalid_encryption_header(
            "encryption_header.key_identifier_shape",
        ));
    }
    if encryption[5].as_text().as_deref() != Some("1")
        || encryption
            .get(6)
            .is_some_and(|value| value.as_text().is_none_or(|value| !value.is_empty()))
    {
        return Err(invalid_encryption_header("encryption_header.iv_occupancy"));
    }
    if segment.elements()[8].components()[0].as_text().as_deref() != Some("0") {
        return Err(invalid_encryption_header(
            "encryption_header.compression_function",
        ));
    }
    Ok(())
}

fn invalid_encryption_header(structure: &'static str) -> Error {
    Error::InvalidResponse { structure }
}

fn valid_security_timestamp(components: &[crate::wire::Value]) -> bool {
    if components
        .first()
        .and_then(crate::wire::Value::as_text)
        .as_deref()
        != Some("1")
    {
        return false;
    }
    let date = components.get(1);
    let time = components.get(2);
    match date {
        None => time.is_none(),
        Some(date) => {
            let Some(date) = date.as_text() else {
                return false;
            };
            if date.is_empty() {
                return time.is_none_or(|time| time.as_text().is_some_and(|time| time.is_empty()));
            }
            chrono::NaiveDate::parse_from_str(&date, "%Y%m%d").is_ok()
                && time.is_none_or(|time| {
                    time.as_text().is_some_and(|time| {
                        time.is_empty()
                            || chrono::NaiveTime::parse_from_str(&time, "%H%M%S").is_ok()
                    })
                })
        }
    }
}

fn valid_key_identifier_filler(value: &crate::wire::Value) -> bool {
    matches!(value.as_text().as_deref(), Some("5" | "6"))
}

fn validate_signature_header(segment: &Segment) -> Result<(), Error> {
    let header = segment.header().ok_or(Error::InvalidResponse {
        structure: "authenticated response signature header",
    })?;
    if header.code != b"HNSHK"
        || header.version != 4
        || header.reference.is_some()
        || !(12..=13).contains(&segment.elements().len())
        || !element_has_components(segment, 1, 2)
        || !(2..=5).all(|index| element_has_components(segment, index, 1))
        || !element_has_components(segment, 6, 3)
        || !element_has_components(segment, 7, 1)
        || !(8..=10).all(|index| element_has_components(segment, index, 3))
        || !element_has_components(segment, 11, 6)
        || segment
            .element(12)
            .is_some_and(|element| !element_is_empty(element))
    {
        return Err(Error::InvalidResponse {
            structure: "authenticated response signature header",
        });
    }

    let profile = segment.elements()[1].components();
    if profile[0].as_text().as_deref() != Some("PIN")
        || !matches!(profile[1].as_text().as_deref(), Some("1" | "2"))
    {
        return Err(Error::InvalidResponse {
            structure: "authenticated response signature header",
        });
    }
    let security_function =
        required_segment_text(segment, 2, "authenticated response security function")?;
    let security_function =
        security_function
            .parse::<u16>()
            .map_err(|_| Error::InvalidResponse {
                structure: "authenticated response signature header",
            })?;
    if !((900..=997).contains(&security_function) || security_function == 999)
        || required_segment_text(segment, 4, "authenticated response security area")? != "1"
    {
        return Err(Error::InvalidResponse {
            structure: "authenticated response signature header",
        });
    }

    let reference = signature_control_reference(segment)?;
    if reference.len() > 14 || reference == "0" {
        return Err(Error::InvalidResponse {
            structure: "authenticated response signature header",
        });
    }
    Ok(())
}

fn validate_signature_trailer(segment: &Segment, expected_reference: &str) -> Result<(), Error> {
    let header = segment.header().ok_or(Error::InvalidResponse {
        structure: "authenticated response signature trailer",
    })?;
    if header.code != b"HNSHA"
        || header.version != 2
        || header.reference.is_some()
        || !(2..=4).contains(&segment.elements().len())
        || !element_has_components(segment, 1, 1)
        || segment
            .element(2)
            .is_some_and(|element| !element_is_empty(element))
        || segment
            .element(3)
            .is_some_and(|element| !element_is_empty(element))
    {
        return Err(Error::InvalidResponse {
            structure: "authenticated response signature trailer",
        });
    }
    if required_segment_text(
        segment,
        1,
        "authenticated response security control reference",
    )? != expected_reference
    {
        return Err(Error::InvalidResponse {
            structure: "authenticated response security control reference",
        });
    }
    Ok(())
}

fn signature_control_reference(segment: &Segment) -> Result<String, Error> {
    required_segment_text(
        segment,
        3,
        "authenticated response security control reference",
    )
}

fn required_segment_text(
    segment: &Segment,
    element: usize,
    field: &'static str,
) -> Result<String, Error> {
    segment
        .element(element)
        .and_then(|element| {
            (element.components().len() == 1)
                .then(|| element.components()[0].as_text())
                .flatten()
        })
        .filter(|value| !value.is_empty())
        .map(|value| value.into_owned())
        .ok_or(Error::MissingValue { field })
}

fn element_has_components(segment: &Segment, element: usize, count: usize) -> bool {
    segment
        .element(element)
        .is_some_and(|element| element.components().len() == count)
}

fn element_is_empty(element: &crate::wire::Element) -> bool {
    element
        .components()
        .iter()
        .all(|value| value.as_text().is_some_and(|value| value.is_empty()))
}

fn required_message_text(
    message: &Message,
    element: usize,
    field: &'static str,
) -> Result<String, Error> {
    let value = message
        .segments()
        .first()
        .and_then(|segment| segment.element(element))
        .and_then(|element| element.components().first())
        .and_then(|value| value.as_text())
        .filter(|value| !value.is_empty())
        .ok_or(Error::MissingValue { field })?;
    Ok(value.into_owned())
}

pub(super) fn component(
    components: &[crate::wire::Value],
    index: usize,
    field: &'static str,
) -> Result<String, Error> {
    components
        .get(index)
        .and_then(|value| value.as_text())
        .filter(|value| !value.is_empty())
        .map(|value| value.into_owned())
        .ok_or(Error::MissingValue { field })
}

pub(super) fn optional_component(
    components: &[crate::wire::Value],
    index: usize,
) -> Option<String> {
    components
        .get(index)
        .and_then(|value| value.as_text())
        .filter(|value| !value.is_empty())
        .map(|value| value.into_owned())
}

fn recovery_for(code: u16) -> Option<Recovery> {
    match code {
        3040 | 3956 | 3957 | 9997 => Some(Recovery::RetryLater),
        3050 | 3081 => Some(Recovery::RefreshParameters),
        3072 | 9942 => Some(Recovery::CorrectCredentials),
        3920 | 3958 => Some(Recovery::ChooseTanMethod),
        3955 => Some(Recovery::WaitForApproval),
        9078 => Some(Recovery::CorrectProductIdentity),
        9075 => Some(Recovery::StrongAuthenticationRequired),
        9185 => Some(Recovery::CorrectEndpoint),
        9391 => Some(Recovery::Resynchronize),
        9000 | 9800 | 9951 => Some(Recovery::RestartDialog),
        9110 | 9130 | 9210 | 9380 => Some(Recovery::UnsupportedCapability),
        _ => None,
    }
}

#[cfg(test)]
mod product_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transaction_tests;
