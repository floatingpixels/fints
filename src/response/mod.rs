use crate::{
    error::{BankResponse, Error, Recovery, ResponseClass},
    model::{Balance, ReusableState, TanMedium, TransactionFormat},
    wire::{Message, Segment},
};

mod balance;
mod parameters;
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
        let segments = message.payload_segments()?;
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
        3920 | 3958 | 9075 => Some(Recovery::ChooseTanMethod),
        3955 => Some(Recovery::WaitForApproval),
        9078 => Some(Recovery::CorrectProductIdentity),
        9185 => Some(Recovery::CorrectEndpoint),
        9391 => Some(Recovery::Resynchronize),
        9000 | 9800 | 9951 => Some(Recovery::RestartDialog),
        9110 | 9130 | 9210 | 9380 => Some(Recovery::UnsupportedCapability),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod transaction_tests;
