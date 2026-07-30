#[cfg(feature = "development-diagnostics")]
use crate::development_diagnostics::{ReceivedResponseFact, ReceivedSegmentFact};
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
    dialog_abort: bool,
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
                structure: "response.HIRMG.first",
            });
        }
        let mut responses = Vec::new();
        let mut allowed_tan_methods = Vec::new();
        let mut has_message_response = false;
        let mut has_tan_method_response = false;
        let mut continuation_points = Vec::new();

        for segment in &segments {
            let header = segment.header().ok_or(Error::InvalidResponse {
                structure: "response.segment_header",
            })?;
            match header.code {
                b"HIRMG" | b"HIRMS" => {
                    if header.code == b"HIRMG" && has_message_response {
                        return Err(Error::InvalidResponse {
                            structure: "response.HIRMG.duplicate",
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
                            structure: "response.HIRMG.elements",
                        });
                    }
                    has_message_response |= header.code == b"HIRMG";
                    let reference = (header.code == b"HIRMS")
                        .then_some(header.reference)
                        .flatten();
                    for element in &segment.elements()[1..] {
                        // Formals B.7.2/B.7.3 and DD `Rückmeldung`: repeated
                        // response DEGs are segment elements. Their scalar
                        // fields map directly to code/reference/text at
                        // components 0/1/2 and parameters from component 3.
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
                            // Rückmeldungscodes 2026-02-03 B.2 defines class-1
                            // notices as non-error diagnostics, while marking them
                            // FinTS-4-only. Owner-observed FinTS 3 interoperability
                            // requires retaining the same bounded, meaning-neutral
                            // shape without changing aggregate result semantics.
                            1 => ResponseClass::Notice,
                            3 => ResponseClass::Warning,
                            9 => ResponseClass::Error,
                            _ => {
                                return Err(Error::InvalidResponseCodeClass { code: numeric_code });
                            }
                        };
                        // Formals 2017-10-06, F "Rückmeldung" defines short
                        // diagnostic fields. They do not affect response
                        // classification or recovery, so interoperability
                        // retains longer or additional bank-supplied values
                        // verbatim under the bounded 1 MiB wire-message limit.
                        let data_element_reference = optional_component(components, 1);
                        let text = component(components, 2, "response text")?;
                        let parameters = components
                            .iter()
                            .skip(3)
                            .map(|value| {
                                value.as_text().map(|value| value.into_owned()).ok_or(
                                    Error::InvalidValue {
                                        field: "response parameter",
                                    },
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        responses.push(BankResponse::new(
                            numeric_code,
                            class,
                            reference,
                            recovery_for(numeric_code),
                            text,
                            data_element_reference,
                            parameters,
                        ));
                        if numeric_code == 3040 {
                            let segment_number = reference.ok_or(Error::InvalidResponse {
                                structure: "response.3040.segment_reference",
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
                                    .filter(|value| is_tan_security_function(value))
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
        // Formals B.7.6 fixes the unsigned abort message to
        // HNHBK+HIRMG+HNHBS. For a mid-dialog failure, either identifier may
        // be unavailable and use the normative sentinel.
        let dialog_abort = !message.is_security_enveloped()
            && message.segments().len() == 3
            && segments.len() == 1
            && (dialog_id == "unbekannt" || message_number == 9999)
            && responses
                .iter()
                .any(|response| response.class() == ResponseClass::Error);
        Ok(Self {
            dialog_id,
            message_number,
            dialog_abort,
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

    pub(crate) fn is_dialog_abort(&self) -> bool {
        self.dialog_abort
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

    pub(crate) fn is_bank_terminated_tan_method_discovery(&self) -> bool {
        // PIN/TAN 2020 B.4.3.1, B.6.1 (3920), and B.8.2 (9955):
        // function-999 discovery may be deliberately ended with 9800 and/or
        // 9955. Code 9050 is only the message-level partial-error summary.
        // This response-set classification is deliberately order-independent
        // and does not change the general meaning of any individual code.
        self.responses.iter().any(|response| {
            response.class() == ResponseClass::Error && matches!(response.code(), 9800 | 9955)
        }) && self
            .responses
            .iter()
            .filter(|response| response.class() == ResponseClass::Error)
            .all(|response| matches!(response.code(), 9050 | 9800 | 9955))
    }

    pub(crate) fn is_unclassified_bank_terminated_tan_method_discovery(&self) -> bool {
        // The current Rückmeldungscodes A register says 99xx historically has
        // institution-specific, differing meanings. An unpublished 99xx is
        // therefore only a structural companion to 9800 here; it receives no
        // standalone meaning and every published 99xx remains an error.
        let errors = || {
            self.responses
                .iter()
                .filter(|response| response.class() == ResponseClass::Error)
        };
        errors().any(|response| response.code() == 9800)
            && errors().any(|response| is_unpublished_99xx(response.code()))
            && errors().all(|response| {
                matches!(response.code(), 9050 | 9800 | 9955)
                    || is_unpublished_99xx(response.code())
            })
    }

    pub(crate) fn is_global_bpdless_tan_method_discovery_abort(&self) -> bool {
        // PIN/TAN B.4.3.1 requires current anonymous BPD before function-999
        // discovery and requires that discovery to return 3920. A response
        // containing exactly global 9050/9800 plus unpublished 9952 is
        // classified only as a bank-terminated attempt that did not reach the
        // mandatory method result. The unpublished code receives no meaning.
        !self.has_tan_method_response && self.is_exact_global_discovery_termination()
    }

    pub(crate) fn is_exact_global_discovery_termination(&self) -> bool {
        // Formals C.1.2 defines 9800 as institute-side dialog termination. This
        // deliberately narrow shape gives unpublished 9952 no standalone
        // meaning: it only proves that an otherwise validated dialog has ended.
        self.segments.len() == 1
            && self.responses.len() == 3
            && self.has_exact_global_discovery_errors()
    }

    pub(crate) fn is_exact_anonymous_bpd_method_failure(&self) -> bool {
        // PIN/TAN correction T8 requires anonymous BPD when 3920 identifiers
        // have no usable descriptions. This exact failure shape establishes
        // only that the mandatory parameter source remained unavailable; the
        // unpublished companion receives no inferred meaning.
        self.has_tan_method_response
            && !self.allowed_tan_methods.is_empty()
            && self.responses.len() == 4
            && self
                .responses
                .iter()
                .filter(|response| response.code() == 3920)
                .count()
                == 1
            && self.responses.iter().any(|response| {
                response.code() == 3920 && response.class() == ResponseClass::Warning
            })
            && self.has_exact_global_discovery_errors()
    }

    fn has_exact_global_discovery_errors(&self) -> bool {
        self.responses
            .iter()
            .filter(|response| response.class() == ResponseClass::Error)
            .count()
            == 3
            && self.responses.iter().all(|response| {
                response.class() != ResponseClass::Error
                    || (response.segment_number().is_none()
                        && (matches!(response.code(), 9050 | 9800)
                            || (response.code() == 9952 && is_unpublished_99xx(response.code()))))
            })
            && self
                .responses
                .iter()
                .any(|response| response.code() == 9050)
            && self
                .responses
                .iter()
                .any(|response| response.code() == 9800)
            && self
                .responses
                .iter()
                .any(|response| response.code() == 9952 && is_unpublished_99xx(response.code()))
    }

    pub(crate) fn first_error(&self) -> Option<BankResponse> {
        self.responses
            .iter()
            .find(|response| response.class() == ResponseClass::Error)
            .cloned()
    }

    pub(crate) fn apply_parameters(
        &self,
        state: &mut ReusableState,
    ) -> Result<Option<Vec<crate::model::Account>>, Error> {
        parameters::apply(&self.segments, state, false)
    }

    pub(crate) fn apply_parameter_refresh(
        &self,
        state: &mut ReusableState,
    ) -> Result<Option<Vec<crate::model::Account>>, Error> {
        parameters::apply(&self.segments, state, true)
    }

    pub(crate) fn bpd_version(&self) -> Result<Option<u16>, Error> {
        parameters::bpd_version(&self.segments)
    }

    pub(crate) fn bpd_institute(&self) -> Result<Option<crate::model::InstituteState>, Error> {
        parameters::bpd_institute(&self.segments)
    }

    pub(crate) fn system_id(&self) -> Result<Option<String>, Error> {
        parameters::system_id(&self.segments)
    }

    pub(crate) fn tan_media(
        &self,
        expected_version: u16,
        expected_reference: Option<u16>,
    ) -> Result<Option<Vec<TanMedium>>, Error> {
        parameters::tan_media(&self.segments, expected_version, expected_reference)
    }

    #[cfg(feature = "development-diagnostics")]
    pub(crate) fn development_tan_usage_option(
        &self,
        expected_version: u16,
        expected_reference: Option<u16>,
    ) -> Option<u8> {
        parameters::development_tan_usage_option(
            &self.segments,
            expected_version,
            expected_reference,
        )
    }

    #[cfg(feature = "development-diagnostics")]
    pub(crate) fn development_tan_medium_shapes(
        &self,
        expected_version: u16,
        expected_reference: Option<u16>,
    ) -> Vec<crate::development_diagnostics::TanMediumElementShapeFact> {
        parameters::development_tan_medium_shapes(
            &self.segments,
            expected_version,
            expected_reference,
        )
    }

    #[cfg(feature = "development-diagnostics")]
    pub(crate) fn development_segment_facts(&self) -> Vec<ReceivedSegmentFact> {
        self.segments
            .iter()
            .filter_map(Segment::header)
            .filter_map(|header| {
                std::str::from_utf8(header.code)
                    .ok()
                    .map(|code| ReceivedSegmentFact::new(code.to_owned(), header.version))
            })
            .collect()
    }

    #[cfg(feature = "development-diagnostics")]
    pub(crate) fn development_response_facts(&self) -> Vec<ReceivedResponseFact> {
        self.responses
            .iter()
            .map(|response| ReceivedResponseFact::new(response.code(), response.segment_number()))
            .collect()
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

    pub(crate) fn depot_positions(
        &self,
        version: u16,
    ) -> Result<Option<securities::DepotPositionPage>, Error> {
        securities::positions(&self.segments, version)
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
                structure: "response.continuation.duplicate",
            });
        }
        Ok(point.map(|point| point.value.as_str()))
    }

    pub(crate) fn tan(&self, expected_version: u16) -> Result<Option<tan::TanResponse>, Error> {
        tan::parse(&self.segments, expected_version)
    }
}

fn is_tan_security_function(value: &str) -> bool {
    value.len() == 3
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value
            .parse::<u16>()
            .is_ok_and(|value| (900..=997).contains(&value) || value == 999)
}

// FinTS Formals B.7.1 permits an optional HNSHK/HNSHA control pair in the
// unencrypted response layout. B.8 restores the same logical order inside
// HNVSD; PIN/TAN B.1 and F.2 use the controls without bank-signature material.
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
    if signature_headers != 1 || signature_trailers != 1 || payload.len() < 3 {
        return Err(Error::InvalidResponse {
            structure: "response.security_controls.pair",
        });
    }

    let signature_header = payload.first().ok_or(Error::InvalidResponse {
        structure: "response.security_controls.header",
    })?;
    let signature_trailer = payload.last().ok_or(Error::InvalidResponse {
        structure: "response.security_controls.trailer",
    })?;
    let control_reference = validate_signature_header(signature_header)?;
    validate_signature_trailer(signature_trailer, &control_reference)?;

    Ok(payload[1..payload.len() - 1].to_vec())
}

// HBCI Security B.5.3 supplies the shared HNVSK 3 layout; PIN/TAN
// B.9.1 and B.9.8-B.9.9 constrain it to the cleartext-over-TLS profile.
// The PIN/TAN definition of FinTS-Füllwert makes role, timestamp, key bytes,
// and key-parameter identifier processing-irrelevant; their bounded element
// geometry is retained while their discarded contents are read past.
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

    // HBCI Security B.5.3/DD HNVSK 3: elements 1..9 follow DD fields 2..10.
    // Internal widths are profile=2, identity=3, timestamp=1..3,
    // encryption=6..7, and key-name=6 because its field-1 `kik` contributes
    // two components before four scalar key fields.
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

    let encryption = segment.elements()[6].components();
    if encryption[0].as_text().as_deref() != Some("2")
        || !matches!(encryption[1].as_text().as_deref(), Some("2" | "18"))
        || !matches!(encryption[2].as_text().as_deref(), Some("13" | "14"))
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

// HBCI Security 2024 B.5.1/DD supplies the mandatory HNSHK 4 layout and
// field formats. PIN/TAN 2020 B.9.1-B.9.6 fixes the profile, function, area,
// identity occupancy, and certificate prohibition. Processing-irrelevant role,
// timestamp, hash, and key-name contents are read past inside their bounded
// element geometry.
fn validate_signature_header(segment: &Segment) -> Result<String, Error> {
    let header = segment
        .header()
        .ok_or_else(|| invalid_signature_header("signature_header.segment_header"))?;
    if header.code != b"HNSHK" || header.version != 4 || header.reference.is_some() {
        return Err(invalid_signature_header("signature_header.segment_header"));
    }
    // HBCI Security B.5.1/DD HNSHK 4: elements 1..12 follow DD fields 2..13.
    // Internal widths are profile=2, identity=3, timestamp=1..3,
    // hash=3..4, signature-algorithm=3, and key-name=6 because its nested
    // field-1 `kik` expands to two components before four scalar fields.
    if !(12..=13).contains(&segment.elements().len())
        || !element_has_components(segment, 1, 2)
        || !(2..=5).all(|index| element_has_components(segment, index, 1))
        || !element_has_components(segment, 6, 3)
        || !element_has_components(segment, 7, 1)
        || segment
            .element(8)
            .is_none_or(|element| !(1..=3).contains(&element.components().len()))
        || segment
            .element(9)
            .is_none_or(|element| !(3..=4).contains(&element.components().len()))
        || !element_has_components(segment, 10, 3)
        || !element_has_components(segment, 11, 6)
    {
        return Err(invalid_signature_header("signature_header.element_shape"));
    }

    let profile = segment.elements()[1].components();
    if profile[0].as_text().as_deref() != Some("PIN")
        || !matches!(profile[1].as_text().as_deref(), Some("1" | "2"))
    {
        return Err(invalid_signature_header(
            "signature_header.security_profile",
        ));
    }
    if segment.elements()[2].components()[0]
        .as_text()
        .is_none_or(|value| !is_tan_security_function(&value))
    {
        return Err(invalid_signature_header(
            "signature_header.security_function",
        ));
    }

    let reference = required_segment_text(segment, 3, "signature control reference")
        .map_err(|_| invalid_signature_header("signature_header.control_reference"))?;
    if encoding_rs::mem::encode_latin1_lossy(&reference).len() > 14 || reference == "0" {
        return Err(invalid_signature_header(
            "signature_header.control_reference",
        ));
    }
    if segment.elements()[4].components()[0].as_text().as_deref() != Some("1") {
        return Err(invalid_signature_header(
            "signature_header.application_area",
        ));
    }
    let identity = segment.elements()[6].components();
    if !matches!(identity[0].as_text().as_deref(), Some("1" | "2"))
        || identity[1].as_text().as_deref() != Some("")
        || identity[2].as_text().is_none_or(|value| value.is_empty())
    {
        return Err(invalid_signature_header(
            "signature_header.security_identity_shape",
        ));
    }
    if !valid_numeric_text(&segment.elements()[7].components()[0], 16) {
        return Err(invalid_signature_header(
            "signature_header.security_reference_shape",
        ));
    }
    let signature = segment.elements()[10].components();
    if signature[0].as_text().as_deref() != Some("6")
        || !valid_code_text(&signature[1])
        || !valid_code_text(&signature[2])
    {
        return Err(invalid_signature_header(
            "signature_header.signature_algorithm_shape",
        ));
    }
    if segment
        .element(12)
        .is_some_and(|element| element.components().len() != 1 || !element_is_empty(element))
    {
        return Err(invalid_signature_header(
            "signature_header.certificate_occupancy",
        ));
    }
    Ok(reference)
}

fn validate_signature_trailer(segment: &Segment, expected_reference: &str) -> Result<(), Error> {
    // HBCI Security DD defines the signature trailer "ab Segmentversion 2";
    // version 2 is the only currently defined HNSHA layout.
    let header = segment
        .header()
        .ok_or_else(|| invalid_signature_header("signature_trailer.segment_header"))?;
    if header.code != b"HNSHA" || header.version != 2 || header.reference.is_some() {
        return Err(invalid_signature_header("signature_trailer.segment_header"));
    }
    // HBCI Security B.5.2/DD HNSHA 2: control reference, validation result,
    // and user signature are separate elements 1..3. PIN/TAN leaves the
    // latter two empty, so no nested user-signature components are consumed.
    if !(2..=4).contains(&segment.elements().len())
        || !element_has_components(segment, 1, 1)
        || segment
            .element(2)
            .is_some_and(|element| !element_is_empty(element))
        || segment
            .element(3)
            .is_some_and(|element| !element_is_empty(element))
    {
        return Err(invalid_signature_header("signature_trailer.element_shape"));
    }
    if required_segment_text(
        segment,
        1,
        "authenticated response security control reference",
    )
    .map_err(|_| invalid_signature_header("signature_trailer.control_reference"))?
        != expected_reference
    {
        return Err(invalid_signature_header(
            "signature_trailer.control_reference",
        ));
    }
    Ok(())
}

fn invalid_signature_header(structure: &'static str) -> Error {
    Error::InvalidResponse { structure }
}

fn valid_code_text(value: &crate::wire::Value) -> bool {
    value.as_text().is_some_and(|value| {
        !value.is_empty() && encoding_rs::mem::encode_latin1_lossy(&value).len() <= 3
    })
}

fn valid_numeric_text(value: &crate::wire::Value, max: usize) -> bool {
    value
        .as_text()
        .is_some_and(|value| !value.is_empty() && value.len() <= max)
        && value
            .as_text()
            .is_some_and(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
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

pub(super) fn camt_descriptor_matches(value: &str, expected: &str) -> bool {
    strip_ascii_suffix(value, ".xsd").eq_ignore_ascii_case(strip_ascii_suffix(expected, ".xsd"))
}

fn strip_ascii_suffix<'a>(value: &'a str, suffix: &str) -> &'a str {
    value
        .get(..value.len().saturating_sub(suffix.len()))
        .filter(|prefix| {
            value
                .get(prefix.len()..)
                .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
        })
        .unwrap_or(value)
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

fn is_unpublished_99xx(code: u16) -> bool {
    (9900..=9999).contains(&code)
        && !matches!(
            code,
            9901 | 9910
                | 9920
                | 9930
                | 9931
                | 9939
                | 9941
                | 9942
                | 9943
                | 9951
                | 9953
                | 9954
                | 9955
                | 9956
                | 9957
                | 9958
                | 9959
                | 9960
                | 9961
                | 9962
                | 9963
                | 9964
                | 9980
                | 9991
                | 9992
                | 9997
                | 9998
                | 9999
        )
}

#[cfg(feature = "fuzzing")]
pub(crate) fn fuzz_camt(input: &[u8]) -> bool {
    transactions::fuzz_camt(input)
}

#[cfg(feature = "fuzzing")]
pub(crate) fn fuzz_mt940(input: &[u8]) -> bool {
    transactions::fuzz_mt940(input)
}

#[cfg(feature = "fuzzing")]
pub(crate) fn fuzz_securities_document(input: &[u8]) -> bool {
    securities::fuzz_document(input)
}

#[cfg(feature = "fuzzing")]
pub(crate) fn fuzz_credit_card_entry(input: &[u8]) -> bool {
    credit_card::fuzz_entry(input)
}

#[cfg(test)]
mod product_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transaction_tests;
