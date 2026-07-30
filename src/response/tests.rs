use super::*;
use crate::model::{CamtCapability, CreditCardCapability, CreditDebit, TanMethod, TanProcess};
use crate::wire::{Element, Value};
use chrono::{NaiveDate, NaiveTime};

fn message(segments: &[String], dialog_id: &str, message_number: u16) -> Vec<u8> {
    let trailer_number = segments.len() + 2;
    let mut wire =
        format!("HNHBK:1:3+000000000000+300+{dialog_id}+{message_number}+{dialog_id}:1'");
    for segment in segments {
        wire.push_str(segment);
        wire.push('\'');
    }
    wire.push_str(&format!("HNHBS:{trailer_number}:1+{message_number}'"));
    let mut wire = encoding_rs::mem::encode_latin1_lossy(&wire).into_owned();
    let length = format!("{:012}", wire.len());
    wire[10..22].copy_from_slice(length.as_bytes());
    wire
}

// Deliberately assembled without the crate serializer. FinTS Formals B.8 and
// PIN/TAN B.9.8-B.9.10 place the logical inner segments in binary HNVSD.
fn secured_message(
    inner: &[u8],
    outer_trailer_number: u16,
    trailer_message_number: u16,
) -> Vec<u8> {
    let encryption_header = fictional_encryption_header(EncryptionHeaderFixture::default());
    secured_message_with_encryption_header(
        inner,
        outer_trailer_number,
        trailer_message_number,
        &encryption_header,
    )
}

struct EncryptionHeaderFixture<'a> {
    profile: &'a str,
    security_function: &'a str,
    security_role: &'a str,
    security_identity: &'a str,
    security_timestamp: &'a str,
    algorithm_codes: [&'a str; 3],
    key_filler: &'a [u8],
    key_identifier: &'a str,
    iv_identifier: &'a str,
    iv: Option<&'a [u8]>,
    compression: &'a str,
}

impl Default for EncryptionHeaderFixture<'static> {
    fn default() -> Self {
        Self {
            profile: "PIN:2",
            security_function: "998",
            security_role: "1",
            security_identity: "1::fictional-system",
            security_timestamp: "1:20260729:120000",
            algorithm_codes: ["2", "2", "13"],
            key_filler: &[0; 8],
            key_identifier: "5",
            iv_identifier: "1",
            iv: None,
            compression: "0",
        }
    }
}

fn fictional_encryption_header(fixture: EncryptionHeaderFixture<'_>) -> Vec<u8> {
    let mut header = format!(
        concat!("HNVSK:998:3+{}+{}+{}+{}+{}", "+{}:{}:{}:@{}@"),
        fixture.profile,
        fixture.security_function,
        fixture.security_role,
        fixture.security_identity,
        fixture.security_timestamp,
        fixture.algorithm_codes[0],
        fixture.algorithm_codes[1],
        fixture.algorithm_codes[2],
        fixture.key_filler.len()
    )
    .into_bytes();
    header.extend_from_slice(fixture.key_filler);
    header.extend_from_slice(
        format!(":{}:{}", fixture.key_identifier, fixture.iv_identifier).as_bytes(),
    );
    if let Some(iv) = fixture.iv {
        header.extend_from_slice(format!(":@{}@", iv.len()).as_bytes());
        header.extend_from_slice(iv);
    }
    header.extend_from_slice(
        format!(
            "+280:12345678:fictional-bank:V:0:0+{}'",
            fixture.compression
        )
        .as_bytes(),
    );
    header
}

fn secured_message_with_encryption_header(
    inner: &[u8],
    outer_trailer_number: u16,
    trailer_message_number: u16,
    encryption_header: &[u8],
) -> Vec<u8> {
    let mut wire = b"HNHBK:1:3+000000000000+300+dialog1+1+dialog1:1'".to_vec();
    wire.extend_from_slice(encryption_header);
    wire.extend_from_slice(b"HNVSD:999:1+@");
    wire.extend_from_slice(inner.len().to_string().as_bytes());
    wire.push(b'@');
    wire.extend_from_slice(inner);
    wire.extend_from_slice(
        format!("'HNHBS:{outer_trailer_number}:1+{trailer_message_number}'").as_bytes(),
    );
    let length = format!("{:012}", wire.len());
    wire[10..22].copy_from_slice(length.as_bytes());
    wire
}

fn fixture_offset(fixture: &[u8], needle: &[u8]) -> usize {
    fixture
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("fictional fixture marker must exist")
}

fn patch_fixture_length(fixture: &mut [u8]) {
    let length = format!("{:012}", fixture.len());
    fixture[10..22].copy_from_slice(length.as_bytes());
}

fn assert_redacted_encryption_error(error: Error, expected: &'static str) {
    assert!(matches!(
        &error,
        Error::InvalidResponse { structure } if *structure == expected
    ));
    assert_eq!(
        error.to_string(),
        format!("FinTS response has an invalid {expected} structure")
    );
    assert!(!error.to_string().contains("fictional-system"));
}

fn assert_encryption_header_error(header: Vec<u8>, expected: &'static str) {
    let fixture =
        secured_message_with_encryption_header(b"HIRMG:2:2+0010::accepted'", 3, 1, &header);
    let error = match Response::parse(&fixture) {
        Ok(_) => panic!("malformed fictional encryption header must fail"),
        Err(error) => error,
    };
    assert_redacted_encryption_error(error, expected);
}

struct SignatureHeaderFixture<'a> {
    profile: &'a str,
    security_function: &'a str,
    control_reference: &'a str,
    application_area: &'a str,
    security_role: &'a str,
    security_identity: &'a str,
    security_reference: &'a str,
    timestamp: &'a str,
    hash: &'a str,
    signature_algorithm: &'a str,
    key_name: &'a str,
    certificate: Option<&'a str>,
}

impl Default for SignatureHeaderFixture<'static> {
    fn default() -> Self {
        Self {
            profile: "PIN:2",
            security_function: "942",
            control_reference: "fictional-ref",
            application_area: "1",
            security_role: "1",
            security_identity: "1::fictional-system",
            security_reference: "1",
            timestamp: "1:20260729:120000",
            hash: "1:999:1",
            signature_algorithm: "6:10:16",
            key_name: "280:12345678:fictional-bank:S:0:0",
            certificate: None,
        }
    }
}

fn fictional_signature_header(fixture: SignatureHeaderFixture<'_>) -> String {
    let mut header = format!(
        "HNSHK:2:4+{}+{}+{}+{}+{}+{}+{}+{}+{}+{}+{}",
        fixture.profile,
        fixture.security_function,
        fixture.control_reference,
        fixture.application_area,
        fixture.security_role,
        fixture.security_identity,
        fixture.security_reference,
        fixture.timestamp,
        fixture.hash,
        fixture.signature_algorithm,
        fixture.key_name,
    );
    if let Some(certificate) = fixture.certificate {
        header.push('+');
        header.push_str(certificate);
    }
    header.push('\'');
    header
}

fn signed_response(header: &str, trailer_reference: &str) -> Vec<u8> {
    signed_response_with_trailer(header, &format!("HNSHA:4:2+{trailer_reference}'"))
}

fn signed_response_with_trailer(header: &str, trailer: &str) -> Vec<u8> {
    let inner = format!("{header}HIRMG:3:2+0010::fictional accepted'{trailer}");
    secured_message(inner.as_bytes(), 5, 1)
}

fn signed_clear_response(header: &str, trailer: &str) -> Vec<u8> {
    let mut wire = format!(
        "HNHBK:1:3+000000000000+300+dialog1+1+dialog1:1'\
         {header}HIRMG:3:2+0010::fictional accepted'{trailer}HNHBS:5:1+1'"
    );
    let length = format!("{:012}", wire.len());
    wire.replace_range(10..22, &length);
    wire.into_bytes()
}

fn assert_redacted_signature_error(error: Error, expected: &'static str) {
    assert!(matches!(
        &error,
        Error::InvalidResponse { structure } if *structure == expected
    ));
    assert_eq!(
        error.to_string(),
        format!("FinTS response has an invalid {expected} structure")
    );
    assert!(!error.to_string().contains("fictional-system"));
    assert!(!error.to_string().contains("fictional-ref"));
}

fn signature_error(header: &str, trailer_reference: &str) -> Error {
    match Response::parse(&signed_response(header, trailer_reference)) {
        Ok(_) => panic!("malformed fictional signature controls must fail"),
        Err(error) => error,
    }
}

// FinTS Formals B.7.1 and B.8; PIN/TAN 2020 B.1, B.9.4-B.9.10,
// and F.2: the optional bank-side HNSHK/HNSHA control pair surrounds the
// response segments restored from HNVSD. It carries no bank signature.
#[test]
fn authenticated_response_security_controls_precede_hirmg_and_apply_parameters() {
    let inner = concat!(
        "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
        "+1::fictional-system+1+1:20260729:120000",
        "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
        "HIRMG:3:2+0010::accepted'",
        "HIRMS:4:2:4+3920::methods:942'",
        "HIBPA:5:3:4+7+280:12345678+Fictional Bank+9+1+300'",
        "HIPINS:6:1:4+1+1+0+4:6:6:::HKSAL:N:HKTAN:N'",
        "HIUPA:7:4:4+fictional-user+3+0'",
        "HIUPD:8:6:4+123456::280:12345678",
        "+DE40123456780000123456+fictional-customer+1+EUR",
        "+Fictional Person++Checking++HKSAL:1'",
        "HNSHA:9:2+fictional-ref'"
    )
    .as_bytes();
    // PIN/TAN B.9 defines the key value and key-parameter identifier as
    // processing-irrelevant FinTS filler values; HBCI DD defines role 3.
    let encryption_header = fictional_encryption_header(EncryptionHeaderFixture {
        security_role: "3",
        security_identity: "2::fictional-system",
        security_timestamp: "1",
        key_filler: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        key_identifier: "6",
        ..EncryptionHeaderFixture::default()
    });
    let fixture = secured_message_with_encryption_header(inner, 10, 1, &encryption_header);
    let mut state = ReusableState::new();

    let response = Response::parse(&fixture).unwrap();
    response.apply_parameters(&mut state).unwrap();

    assert_eq!(response.allowed_tan_methods(), ["942"]);
    assert_eq!(state.bpd_version(), 7);
    assert_eq!(state.upd_version(), 3);
    assert_eq!(state.accounts().len(), 1);
}

// PIN/TAN F.2 also permits institute responses without the optional
// HNSHK/HNSHA pair; HIRMG then begins the logical sequence inside HNVSD.
#[test]
fn authenticated_response_without_optional_signature_controls_still_parses() {
    let inner = concat!("HIRMG:2:2+0010::accepted'", "HIRMS:3:2:4+0020::processed'");

    let response = Response::parse(&secured_message(inner.as_bytes(), 4, 1)).unwrap();

    assert_eq!(response.responses().len(), 2);
}

// Formals B.7.1 lists HNSHK/HNSHA as optional in the unencrypted institute
// layout. The same exact pair validation applies without HNVSK/HNVSD.
#[test]
fn signed_but_unencrypted_response_is_validated_as_a_control_pair() {
    let header = fictional_signature_header(SignatureHeaderFixture::default());
    let valid = signed_clear_response(&header, "HNSHA:4:2+fictional-ref'");
    assert_eq!(Response::parse(&valid).unwrap().responses()[0].code(), 10);

    let missing_trailer = message(
        &[
            header.trim_end_matches('\'').to_owned(),
            "HIRMG:3:2+0010::fictional accepted".to_owned(),
        ],
        "dialog1",
        1,
    );
    assert!(matches!(
        Response::parse(&missing_trailer),
        Err(Error::InvalidResponse {
            structure: "response.security_controls.pair"
        })
    ));
}

// HBCI Security 2024 B.5.1 and DD "Sicherheitsdatum und -uhrzeit";
// PIN/TAN 2020 B.9.4-B.9.6: only the timestamp type is mandatory, while
// date and its conditional time may be cut or explicitly left empty.
#[test]
fn authenticated_signature_header_accepts_normative_timestamp_shapes() {
    let timestamps = [
        "1",
        "1:20260729",
        "1:20260729:120000",
        "1:",
        "1::",
        "1:20260729:",
    ];

    for timestamp in timestamps {
        let header = fictional_signature_header(SignatureHeaderFixture {
            timestamp,
            hash: "1:999:1:",
            certificate: Some(""),
            ..SignatureHeaderFixture::default()
        });
        assert!(Response::parse(&signed_response(&header, "fictional-ref")).is_ok());
    }
    for role in ["3", "4"] {
        let header = fictional_signature_header(SignatureHeaderFixture {
            security_role: role,
            security_identity: "2::fictional-system",
            timestamp: "1",
            ..SignatureHeaderFixture::default()
        });
        assert!(Response::parse(&signed_response(&header, "fictional-ref")).is_ok());
    }
}

// PIN/TAN B.9 makes the response-side role, timestamp, hash parameters, and
// key name processing-irrelevant fillers. Their registered component geometry
// remains validated, but their discarded contents do not block HIRMG.
#[test]
fn authenticated_signature_header_reads_past_processing_irrelevant_fields() {
    let header = fictional_signature_header(SignatureHeaderFixture {
        security_role: "unexpected",
        timestamp: "ignored:timestamp:shape",
        hash: "ignored:hash:values:tail",
        key_name: "ignored:key:name:values:and:shape",
        ..SignatureHeaderFixture::default()
    });
    assert!(Response::parse(&signed_response(&header, "fictional-ref")).is_ok());
}

// HBCI Security 2024 B.5.1/DD and PIN/TAN B.9.1-B.9.6 define the fixed
// profile fields, mandatory shapes, filler formats, matching control reference,
// and forbidden certificate occupancy. Labels are constant and value-free.
#[test]
fn authenticated_signature_header_diagnostics_are_redacted_and_field_specific() {
    let cases = [
        (
            fictional_signature_header(SignatureHeaderFixture::default()).replacen(
                "HNSHK:2:4",
                "HNSHK:2:3",
                1,
            ),
            "signature_header.segment_header",
        ),
        (
            fictional_signature_header(SignatureHeaderFixture {
                profile: "PIN:2:extra",
                ..SignatureHeaderFixture::default()
            }),
            "signature_header.element_shape",
        ),
        (
            fictional_signature_header(SignatureHeaderFixture {
                profile: "RAH:2",
                ..SignatureHeaderFixture::default()
            }),
            "signature_header.security_profile",
        ),
        (
            fictional_signature_header(SignatureHeaderFixture {
                security_function: "899",
                ..SignatureHeaderFixture::default()
            }),
            "signature_header.security_function",
        ),
        (
            fictional_signature_header(SignatureHeaderFixture {
                control_reference: "0",
                ..SignatureHeaderFixture::default()
            }),
            "signature_header.control_reference",
        ),
        (
            fictional_signature_header(SignatureHeaderFixture {
                application_area: "2",
                ..SignatureHeaderFixture::default()
            }),
            "signature_header.application_area",
        ),
        (
            fictional_signature_header(SignatureHeaderFixture {
                security_identity: "1:@1@x:fictional-system",
                ..SignatureHeaderFixture::default()
            }),
            "signature_header.security_identity_shape",
        ),
        (
            fictional_signature_header(SignatureHeaderFixture {
                security_reference: "not-numeric",
                ..SignatureHeaderFixture::default()
            }),
            "signature_header.security_reference_shape",
        ),
        (
            fictional_signature_header(SignatureHeaderFixture {
                signature_algorithm: "5:10:16",
                ..SignatureHeaderFixture::default()
            }),
            "signature_header.signature_algorithm_shape",
        ),
        (
            fictional_signature_header(SignatureHeaderFixture {
                certificate: Some("fictional-certificate"),
                ..SignatureHeaderFixture::default()
            }),
            "signature_header.certificate_occupancy",
        ),
    ];

    for (header, expected) in cases {
        assert_redacted_signature_error(signature_error(&header, "fictional-ref"), expected);
    }

    let header = fictional_signature_header(SignatureHeaderFixture::default());
    assert_redacted_signature_error(
        signature_error(&header, "other-reference"),
        "signature_trailer.control_reference",
    );
    for (trailer, expected) in [
        (
            "HNSHA:4:1+fictional-ref'",
            "signature_trailer.segment_header",
        ),
        (
            "HNSHA:4:2+fictional-ref+occupied'",
            "signature_trailer.element_shape",
        ),
    ] {
        let error = match Response::parse(&signed_response_with_trailer(&header, trailer)) {
            Ok(_) => panic!("malformed fictional signature trailer must fail"),
            Err(error) => error,
        };
        assert_redacted_signature_error(error, expected);
    }
}

// PIN/TAN F.2 permits an institute dialog-end response without HNSHK/HNSHA.
#[test]
fn authenticated_termination_without_signature_controls_still_parses() {
    let inner = b"HIRMG:2:2+0100::fictional termination'";
    let response = Response::parse(&secured_message(inner, 3, 1)).unwrap();

    assert_eq!(response.responses()[0].code(), 100);
}

// FinTS Formals B.7.1 requires exactly one HIRMG after any HNSHK and before
// response data. PIN/TAN F.2 permits at most one matching HNSHK/HNSHA pair.
#[test]
fn authenticated_response_rejects_missing_duplicate_or_misplaced_controls_and_hirmg() {
    let cases = [
        (
            concat!(
                "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
                "+1::fictional-system+1+1:20260729:120000",
                "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
                "HIRMS:3:2:4+3920::methods:942'",
                "HNSHA:4:2+fictional-ref'"
            ),
            5,
        ),
        (
            concat!(
                "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
                "+1::fictional-system+1+1:20260729:120000",
                "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
                "HIRMG:3:2+0010::accepted'",
                "HIRMG:4:2+0010::accepted'",
                "HNSHA:5:2+fictional-ref'"
            ),
            6,
        ),
        (
            concat!(
                "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
                "+1::fictional-system+1+1:20260729:120000",
                "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
                "HIFOO:3:1+optional'",
                "HIRMG:4:2+0010::accepted'",
                "HNSHA:5:2+fictional-ref'"
            ),
            6,
        ),
        (
            concat!("HIRMG:2:2+0010::accepted'", "HNSHA:3:2+fictional-ref'"),
            4,
        ),
        (
            concat!(
                "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
                "+1::fictional-system+1+1:20260729:120000",
                "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
                "HIRMG:3:2+0010::accepted'"
            ),
            4,
        ),
        (
            concat!(
                "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
                "+1::fictional-system+1+1:20260729:120000",
                "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
                "HNSHK:3:4+PIN:2+942+fictional-ref+1+1",
                "+1::fictional-system+1+1:20260729:120000",
                "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
                "HIRMG:4:2+0010::accepted'",
                "HNSHA:5:2+fictional-ref'"
            ),
            6,
        ),
        (
            concat!(
                "HNSHA:2:2+fictional-ref'",
                "HIRMG:3:2+0010::accepted'",
                "HNSHK:4:4+PIN:2+942+fictional-ref+1+1",
                "+1::fictional-system+1+1:20260729:120000",
                "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'"
            ),
            5,
        ),
    ];

    for (inner, trailer_number) in cases {
        assert!(Response::parse(&secured_message(inner.as_bytes(), trailer_number, 1)).is_err());
    }
}

// FinTS Formals B.7.1/B.8 and PIN/TAN B.9.4/B.9.7 require HNSHK 4,
// HNSHA 2, matching nonzero control references, and no bank-signature value.
#[test]
fn authenticated_response_rejects_malformed_security_control_values() {
    let cases = [
        concat!(
            "HNSHK:2:3+PIN:2+942+fictional-ref+1+1",
            "+1::fictional-system+1+1:20260729:120000",
            "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
            "HIRMG:3:2+0010::accepted'",
            "HNSHA:4:2+fictional-ref'"
        ),
        concat!(
            "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
            "+1::fictional-system+1+1:20260729:120000",
            "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
            "HIRMG:3:2+0010::accepted'",
            "HNSHA:4:1+fictional-ref'"
        ),
        concat!(
            "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
            "+1::fictional-system+1+1:20260729:120000",
            "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
            "HIRMG:3:2+0010::accepted'",
            "HNSHA:4:2+other-ref'"
        ),
        concat!(
            "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
            "+1::fictional-system+1+1:20260729:120000",
            "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
            "HIRMG:3:2+0010::accepted'",
            "HNSHA:4:2+fictional-ref+@8@signature'"
        ),
    ];

    for inner in cases {
        assert!(Response::parse(&secured_message(inner.as_bytes(), 5, 1)).is_err());
    }
}

// FinTS Formals B.8 and PIN/TAN B.9.8-B.9.10 require exactly ordered
// HNVSK 3/HNVSD 1 controls around the complete binary logical message.
#[test]
fn authenticated_response_rejects_incomplete_or_misordered_outer_controls() {
    let inner = concat!("HIRMG:2:2+0010::accepted'", "HIRMS:3:2:4+0020::processed'");
    let valid = secured_message(inner.as_bytes(), 4, 1);
    let encryption_start = fixture_offset(&valid, b"HNVSK:998:3");
    let data_start = fixture_offset(&valid, b"HNVSD:999:1");
    let trailer_start = fixture_offset(&valid, b"HNHBS:4:1");
    let encryption = valid[encryption_start..data_start].to_vec();
    let data = valid[data_start..trailer_start].to_vec();

    let mut missing_header = valid.clone();
    missing_header.drain(encryption_start..data_start);
    patch_fixture_length(&mut missing_header);

    let mut missing_data = valid.clone();
    missing_data.drain(data_start..trailer_start);
    patch_fixture_length(&mut missing_data);

    let mut duplicate_header = valid.clone();
    duplicate_header.splice(data_start..data_start, encryption.iter().copied());
    patch_fixture_length(&mut duplicate_header);

    let mut misordered = Vec::new();
    misordered.extend_from_slice(&valid[..encryption_start]);
    misordered.extend_from_slice(&data);
    misordered.extend_from_slice(&encryption);
    misordered.extend_from_slice(&valid[trailer_start..]);
    patch_fixture_length(&mut misordered);

    for fixture in [missing_header, missing_data, duplicate_header, misordered] {
        assert!(matches!(
            Response::parse(&fixture),
            Err(Error::Wire(crate::wire::WireError::InvalidSecurityEnvelope))
        ));
    }
}

// PIN/TAN B.9.9 and its FinTS-Füllwert definition make the concrete key bytes,
// key-parameter identifier, role, and timestamp irrelevant to processing.
#[test]
fn authenticated_response_reads_past_processing_irrelevant_encryption_fields() {
    let inner = b"HIRMG:2:2+0010::accepted'";
    let cases = [
        ("1", "1::fictional-system", "1:20260729:120000", "6"),
        ("3", "2::fictional-system", "1", "5"),
        ("unexpected", "2::fictional-system", "ignored:shape", "1234"),
    ];

    for (role, identity, timestamp, key_identifier) in cases {
        let header = fictional_encryption_header(EncryptionHeaderFixture {
            security_role: role,
            security_identity: identity,
            security_timestamp: timestamp,
            key_filler: &[0xA5; 16],
            key_identifier,
            ..EncryptionHeaderFixture::default()
        });
        assert!(
            Response::parse(&secured_message_with_encryption_header(
                inner, 3, 1, &header
            ))
            .is_ok()
        );
    }

    let mut explicitly_unoccupied_certificate =
        fictional_encryption_header(EncryptionHeaderFixture::default());
    explicitly_unoccupied_certificate.pop();
    explicitly_unoccupied_certificate.extend_from_slice(b"+'");
    assert!(
        Response::parse(&secured_message_with_encryption_header(
            inner,
            3,
            1,
            &explicitly_unoccupied_certificate
        ))
        .is_ok()
    );
}

// HBCI Security B.5.3/DD and PIN/TAN B.9.1/B.9.8-B.9.9: profile-defining
// values, structural bounds, and forbidden IV occupancy remain strict.
// Every error exposes only a stable constant structural category.
#[test]
fn authenticated_response_encryption_diagnostics_are_redacted_and_field_specific() {
    let malformed_segment_header = Segment::new(vec![Element::new(vec![
        Value::text("HNVSK").unwrap(),
        Value::text("998").unwrap(),
        Value::text("2").unwrap(),
    ])]);
    assert_redacted_encryption_error(
        validate_encryption_header(&malformed_segment_header).unwrap_err(),
        "encryption_header.segment_header",
    );

    let cases = [
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                profile: "PIN:2:extra",
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.element_shape",
        ),
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                profile: "RAH:2",
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.security_profile",
        ),
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                security_function: "997",
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.security_function",
        ),
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                security_identity: "3::fictional-system",
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.security_identity_shape",
        ),
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                algorithm_codes: ["2", "19", "13"],
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.algorithm_codes",
        ),
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                key_filler: &[7; 513],
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.key_filler_shape",
        ),
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                iv: Some(&[1]),
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.iv_occupancy",
        ),
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                iv_identifier: "2",
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.iv_occupancy",
        ),
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                compression: "1",
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.compression_function",
        ),
    ];
    for (header, expected) in cases {
        assert_encryption_header_error(header, expected);
    }

    let mut text_key_filler = fictional_encryption_header(EncryptionHeaderFixture::default());
    let filler = fixture_offset(&text_key_filler, b"@8@");
    text_key_filler.splice(filler..filler + 11, b"not-a-value".iter().copied());
    assert_encryption_header_error(text_key_filler, "encryption_header.key_filler_shape");

    let mut occupied_certificate = fictional_encryption_header(EncryptionHeaderFixture::default());
    occupied_certificate.pop();
    occupied_certificate.extend_from_slice(b"+certificate'");
    assert_encryption_header_error(occupied_certificate, "encryption_header.element_shape");
}

// HBCI Security 2024 DD "Verschlüsselungsalgorithmus": algorithm fillers
// 13/14 and operation modes 2/18 are all syntactically permitted. Values
// outside those bounded code sets remain rejected without exposing content.
#[test]
fn authenticated_response_accepts_all_encryption_filler_code_combinations() {
    for mode in ["2", "18"] {
        for algorithm in ["13", "14"] {
            let header = fictional_encryption_header(EncryptionHeaderFixture {
                algorithm_codes: ["2", mode, algorithm],
                ..EncryptionHeaderFixture::default()
            });
            let fixture =
                secured_message_with_encryption_header(b"HIRMG:2:2+0010::accepted'", 3, 1, &header);
            assert!(Response::parse(&fixture).is_ok());
        }
    }
    for codes in [["3", "2", "13"], ["2", "17", "13"], ["2", "2", "12"]] {
        assert_encryption_header_error(
            fictional_encryption_header(EncryptionHeaderFixture {
                algorithm_codes: codes,
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.algorithm_codes",
        );
    }
}

// FinTS Formals B.5.2-B.5.3 and B.8 retain continuous inner numbering,
// the logical outer trailer number, and matching HNHBK/HNHBS message numbers.
#[test]
fn authenticated_response_keeps_inner_and_outer_numbering_strict() {
    let wrong_inner_number = concat!(
        "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
        "+1::fictional-system+1+1:20260729:120000",
        "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
        "HIRMG:4:2+0010::accepted'",
        "HNSHA:5:2+fictional-ref'"
    );
    assert!(matches!(
        Response::parse(&secured_message(wrong_inner_number.as_bytes(), 5, 1)),
        Err(Error::Wire(
            crate::wire::WireError::NonSequentialSegmentNumber {
                expected: 3,
                actual: 4
            }
        ))
    ));

    let valid_inner = concat!(
        "HNSHK:2:4+PIN:2+942+fictional-ref+1+1",
        "+1::fictional-system+1+1:20260729:120000",
        "+1:999:1+6:10:16+280:12345678:fictional-bank:S:0:0'",
        "HIRMG:3:2+0010::accepted'",
        "HNSHA:4:2+fictional-ref'"
    );
    assert!(matches!(
        Response::parse(&secured_message(valid_inner.as_bytes(), 6, 1)),
        Err(Error::Wire(
            crate::wire::WireError::NonSequentialSegmentNumber {
                expected: 5,
                actual: 6
            }
        ))
    ));
    assert!(matches!(
        Response::parse(&secured_message(valid_inner.as_bytes(), 5, 2)),
        Err(Error::Wire(crate::wire::WireError::MismatchedMessageNumber))
    ));
}

// FinTS Formals B.7.6 and PIN/TAN F.2: permitted unsecured institute
// responses still begin directly with HIRMG and contain no security controls.
#[test]
fn permitted_unsecured_response_still_parses_without_security_controls() {
    let fixture = message(&["HIRMG:2:2+9800::dialog aborted".into()], "unknown", 1);

    assert!(Response::parse(&fixture).is_ok());
}

// HBCI 2.2 VII.2.2 and II.5.3.1/3/4: HISAL 5 uses the national
// account group, legacy balance groups, separate optional booking date/time,
// and no overdraft field. The complete fixture is independently assembled.
#[test]
fn hisal_five_complete_and_sparse_fixtures_preserve_only_supplied_values() {
    let complete = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            concat!(
                "HISAL:3:5:4+654321:EUR:280:12345678",
                "+Fictional legacy checking+EUR",
                "+C:1234,56:EUR:20260729:101112",
                "+D:45,6:EUR:20260729",
                "+5000,:EUR+4500,:EUR+500,:EUR",
                "+20260728+131415+20260801"
            )
            .into(),
        ],
        "dialog1",
        2,
    );
    let balance = Response::parse(&complete)
        .unwrap()
        .balance()
        .unwrap()
        .unwrap();

    assert_eq!(balance.account().account_number(), Some("654321"));
    assert_eq!(balance.account().subaccount(), Some("EUR"));
    assert_eq!(balance.product_name(), "Fictional legacy checking");
    assert_eq!(balance.account_currency(), "EUR");
    assert_eq!(balance.booked().direction(), CreditDebit::Credit);
    assert_eq!(balance.booked().amount().coefficient(), 123_456);
    assert_eq!(
        balance.booked().time(),
        Some(NaiveTime::from_hms_opt(10, 11, 12).unwrap())
    );
    assert_eq!(balance.pending().unwrap().direction(), CreditDebit::Debit);
    assert_eq!(balance.credit_line().unwrap().coefficient(), 5_000);
    assert_eq!(balance.available().unwrap().coefficient(), 4_500);
    assert_eq!(balance.already_drawn().unwrap().coefficient(), 500);
    assert!(balance.overdraft().is_none());
    assert_eq!(
        balance.booking_time().unwrap().date(),
        NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()
    );
    assert_eq!(
        balance.booking_time().unwrap().time(),
        Some(NaiveTime::from_hms_opt(13, 14, 15).unwrap())
    );
    assert_eq!(balance.due_date(), NaiveDate::from_ymd_opt(2026, 8, 1));
    assert!(balance.garnishable_after_month_end().is_none());

    let sparse = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            concat!(
                "HISAL:3:5:4+654321::280:12345678",
                "+Fictional sparse account+EUR+D:7,:EUR:20260729"
            )
            .into(),
        ],
        "dialog2",
        2,
    );
    let balance = Response::parse(&sparse)
        .unwrap()
        .balance()
        .unwrap()
        .unwrap();

    assert_eq!(balance.booked().direction(), CreditDebit::Debit);
    assert!(balance.booked().time().is_none());
    assert!(balance.pending().is_none());
    assert!(balance.credit_line().is_none());
    assert!(balance.available().is_none());
    assert!(balance.already_drawn().is_none());
    assert!(balance.booking_time().is_none());
    assert!(balance.due_date().is_none());
}

// HBCI 2.2 VII.2.2 and II.5.3.1/3/4: positional national identity,
// amount, balance date/time, and optional legacy groups remain strict.
#[test]
fn malformed_hisal_five_fields_fail_with_redacted_typed_errors() {
    let malformed = [
        "HISAL:3:5:4+PRIVATE654321::280+Fictional+EUR+C:1,:EUR:20260729",
        "HISAL:3:5:4+PRIVATE654321::280:12345678+Fictional+EUR+C:1,:EUR:20260230",
        "HISAL:3:5:4+PRIVATE654321::280:12345678+Fictional+EUR+C:1,:EUR:20260729++1,",
        "HISAL:3:5:4+PRIVATE654321::280:12345678+Fictional+EUR+C:1,:EUR:20260729+++++20260728+246000",
    ];

    for segment in malformed {
        let fixture = message(
            &["HIRMG:2:2+0010::accepted".into(), segment.into()],
            "dialog1",
            2,
        );
        let error = match Response::parse(&fixture).unwrap().balance() {
            Ok(_) => panic!("malformed HISAL 5 fixture parsed"),
            Err(error) => error,
        };
        let redacted = format!("{error:?} {error}");
        assert!(!redacted.contains("PRIVATE654321"));
        assert!(!redacted.contains("20260230"));
        assert!(!redacted.contains("246000"));
    }
}

// Formals cut rules and the HBCI/FinTS balance DEGs permit omitted trailing
// optional components. Gate 4 also reads past later unused extension
// components while retaining all mandatory identity and amount fields.
#[test]
fn balance_groups_accept_cut_optional_tails_and_unused_extension_components() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            concat!(
                "HISAL:3:6:4+654321::280:12345678:IGNORED",
                "+Fictional extended balance+EUR",
                "+C:1,:EUR:20260729::IGNORED",
                "++2,:EUR:IGNORED",
                "++++20260729::IGNORED",
                "+20260801:IGNORED"
            )
            .into(),
        ],
        "dialog1",
        2,
    );
    let balance = Response::parse(&fixture)
        .unwrap()
        .balance()
        .unwrap()
        .unwrap();

    assert_eq!(balance.account().account_number(), Some("654321"));
    assert_eq!(balance.booked().amount().coefficient(), 1);
    assert_eq!(balance.credit_line().unwrap().coefficient(), 2);
    assert_eq!(
        balance.booking_time().unwrap().date(),
        NaiveDate::from_ymd_opt(2026, 7, 29).unwrap()
    );
    assert_eq!(balance.due_date(), NaiveDate::from_ymd_opt(2026, 8, 1));
}

// HBCI 2.2 VII.2.2: the optional HISAL 5 booking time has no meaning
// without its optional booking date, so a syntactically valid lone time is read past.
#[test]
fn hisal_five_drops_lone_booking_time_but_rejects_malformed_time() {
    for (time, accepted) in [("131415", true), ("246000", false)] {
        let fixture = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                format!(
                    "HISAL:3:5:4+654321::280:12345678+Fictional+EUR+C:1,:EUR:20260729++++++{time}"
                ),
            ],
            "dialog1",
            2,
        );
        let result = Response::parse(&fixture).unwrap().balance();
        if accepted {
            assert!(result.unwrap().unwrap().booking_time().is_none());
        } else {
            assert!(matches!(result, Err(Error::InvalidValue { .. })));
        }
    }
}

// FinTS Messages 2022-04-15, C.2.1.2.3 and B.1/B.4/B.6.
#[test]
fn hisal_eight_preserves_explicit_balance_metadata() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            concat!(
                "HISAL:3:8:4+DE40123456780000123456:FICTDEFFXXX::::",
                "+Fictional checking+EUR+C:1234,56:EUR:20260728:123456",
                "+D:12,3:EUR:20260728+5000,:EUR+4500,5:EUR",
                "+++20260728:123500++100,:EUR"
            )
            .into(),
        ],
        "dialog1",
        2,
    );

    let response = Response::parse(&fixture).unwrap();
    let balance = response.balance().unwrap().unwrap();

    assert_eq!(balance.account().iban(), Some("DE40123456780000123456"));
    assert_eq!(balance.booked().direction(), CreditDebit::Credit);
    assert_eq!(balance.booked().amount().coefficient(), 123_456);
    assert_eq!(balance.booked().amount().scale(), 2);
    assert_eq!(balance.pending().unwrap().direction(), CreditDebit::Debit);
    assert!(balance.already_drawn().is_none());
    assert!(balance.overdraft().is_none());
    assert_eq!(
        balance.booking_time().unwrap().date(),
        NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()
    );
    assert_eq!(
        balance.booking_time().unwrap().time(),
        Some(NaiveTime::from_hms_opt(12, 35, 0).unwrap())
    );
    assert_eq!(
        balance.garnishable_after_month_end().unwrap().coefficient(),
        100
    );
}

// FinTS Messages 2022-04-15, B.4: an omitted optional time remains absent.
#[test]
fn balance_timestamp_does_not_invent_midnight() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            concat!(
                "HISAL:3:8:4+DE40123456780000123456:FICTDEFFXXX::::",
                "+Fictional checking+EUR+C:1,:EUR:20260728++++++20260728"
            )
            .into(),
        ],
        "dialog1",
        2,
    );

    let balance = Response::parse(&fixture)
        .unwrap()
        .balance()
        .unwrap()
        .unwrap();

    assert_eq!(
        balance.booking_time().unwrap().date(),
        NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()
    );
    assert!(balance.booking_time().unwrap().time().is_none());
}

// Formals 2017-10-06 D/E; PIN/TAN 2020-07-10 B.8.1-B.8.2 and HITANS 7.
#[test]
fn bpd_upd_and_user_allowed_tan_method_are_interpreted_together() {
    let method = [
        "942",
        "2",
        "decoupled-app",
        "Decoupled",
        "1.0",
        "App approval",
        "",
        "",
        "Approval",
        "2048",
        "N",
        "1",
        "N",
        "0",
        "0",
        "N",
        "N",
        "00",
        "0",
        "N",
        "1",
        "5",
        "2",
        "3",
        "J",
        "J",
    ]
    .map(str::to_owned);
    let method_six = method[..21].join(":");
    let method_seven = method.join(":");
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIRMS:3:2:4+3920::methods:942".into(),
            "HIBPA:4:3:4+7+280:12345678+Fictional Bank+9+1+300".into(),
            "HISALS:5:8:4+1+1+0+N".into(),
            format!("HITANS:6:6:4+1+1+0+N:N:0:{method_six}"),
            format!("HITANS:7:7:4+1+1+0+N:N:0:{method_seven}"),
            "HIPINS:8:1:4+1+1+0+4:6:6:::HKSAL:N:HKTAN:N".into(),
            "HIUPA:9:4:4+fictional-user+3+0".into(),
            concat!(
                "HIUPD:10:6:4+123456::280:12345678+DE40123456780000123456",
                "+fictional-customer+1+EUR+Fictional Person++Checking++HKSAL:1"
            )
            .into(),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();

    let response = Response::parse(&fixture).unwrap();
    response.apply_parameters(&mut state).unwrap();

    assert_eq!(state.bpd_version(), 7);
    assert_eq!(state.upd_version(), 3);
    assert_eq!(state.accounts().len(), 1);
    assert!(state.accounts()[0].allows_balance());
    assert_eq!(state.tan_methods().len(), 1);
    assert_eq!(state.tan_methods()[0].security_function(), "942");
    assert_eq!(state.tan_methods()[0].hktan_version(), 7);
    assert_eq!(state.tan_methods()[0].process(), TanProcess::Decoupled);
    assert_eq!(state.tan_methods()[0].max_decoupled_polls(), Some(5));
}

// Formals E.2 "UPD-Verwendung": with value 1, an operation omitted from
// Erlaubte GV is unknown rather than denied; value 0 remains a local deny.
#[test]
fn upd_usage_controls_only_unlisted_operation_permissions() {
    for (usage, expected) in [("0", false), ("1", true)] {
        let fixture = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                format!("HIUPA:3:4:3+fictional-user+1+{usage}"),
                concat!(
                    "HIUPD:4:6:3+123456::280:12345678",
                    "+DE40123456780000123456+fictional-customer+1+EUR",
                    "+Fictional Person++Checking"
                )
                .into(),
            ],
            "dialog1",
            1,
        );
        let mut state = ReusableState::new();
        Response::parse(&fixture)
            .unwrap()
            .apply_parameters(&mut state)
            .unwrap();
        assert_eq!(state.accounts()[0].allows_balance(), expected);
        assert_eq!(state.accounts()[0].allows_booked_transactions(), expected);
        assert_eq!(state.accounts()[0].allows_depot_positions(), expected);
    }

    let malformed = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIUPA:3:4:3+fictional-user+1+2".into(),
        ],
        "dialog2",
        1,
    );
    assert!(matches!(
        Response::parse(&malformed)
            .unwrap()
            .apply_parameters(&mut ReusableState::new()),
        Err(Error::InvalidValue { field: "UPD usage" })
    ));
}

// FinTS Messages 2022-04-15, C.2.3.1.1.1 and C.2.1.1.1.1-.2;
// PIN/TAN correction T31. Capability and TAN status come from BPD/UPD, not endpoints.
#[test]
fn transaction_capabilities_prefer_supported_camt_and_retain_legacy_fallback() {
    let descriptor = "urn?:iso?:std?:iso?:20022?:tech?:xsd?:camt.052.001.08";
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+7+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HICAZS:4:1:3+1+1+0+90:J:N:{descriptor}"),
            "HIKAZS:5:7:3+1+1+0+90:J:N".into(),
            "HIKAZS:6:8:3+1+1+0+90:J:N".into(),
            "HIPINS:7:1:3+1+1+0+4:6:6:::HKCAZ:N:HKKAZ:J".into(),
            "HIUPA:8:4:3+fictional-user+3+0".into(),
            concat!(
                "HIUPD:9:6:3+123456::280:12345678+DE40123456780000123456",
                "+fictional-customer+1+EUR+Fictional Person++Checking",
                "++HKCAZ:1+HKKAZ:1"
            )
            .into(),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();

    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    assert!(state.transaction_capability_advertised);
    assert_eq!(
        state.camt_capability.as_ref().unwrap().descriptor,
        "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"
    );
    assert_eq!(state.legacy_transaction_versions, [7]);
    assert_eq!(state.camt_requires_tan, Some(false));
    assert_eq!(state.legacy_transactions_require_tan, Some(true));
    assert!(state.accounts()[0].allows_booked_transactions());
}

// FinTS Formals D/E and the registered Messages parameter segments: BPD
// advertisements are safe generic protocol facts. The snapshot retains all
// parameter codes/versions while operation support remains independently bounded.
#[test]
fn advertised_capability_snapshot_is_complete_redacted_and_operation_typed() {
    let descriptor = "urn?:iso?:std?:iso?:20022?:tech?:xsd?:camt.052.001.08";
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+61+280:12345678+Fictional Bank+9+1+300".into(),
            "HISALS:4:8:3+1+1+0+N".into(),
            format!("HICAZS:5:1:3+1+1+0+90:J:N:{descriptor}"),
            "HIKAZS:6:7:3+1+1+0+90:J:N".into(),
            "HIKAZS:7:9:3+fictional unsupported parameters".into(),
            "HIWPDS:8:6:3+1+1+0+J:J:J".into(),
            "HIWDUS:9:5:3+1+1+0+90".into(),
            "HIKKUS:10:1:3+1+1+0+90:J:J:J".into(),
            "HIKKSS:11:1:3+1+1+0+J".into(),
            concat!(
                "HIPINS:12:1:3+1+1+0+4:6:6:::HKSAL:N:HKCAZ:N:HKKAZ:J:",
                "HKWPD:N:HKWDU:J:HKKKU:N:HKKKS:J"
            )
            .into(),
            "HITANS:13:8:3+fictional unsupported parameters".into(),
            "HIXYZS:14:2:3+fictional unknown parameters".into(),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    let snapshot = state.advertised_capabilities();
    assert_eq!(snapshot.balance().advertised_versions(), [8]);
    assert!(snapshot.balance().supported_by_crate());
    assert_eq!(snapshot.balance().tan_required(), Some(false));

    assert_eq!(snapshot.camt_cash_transactions().advertised_versions(), [1]);
    assert!(snapshot.camt_cash_transactions().supported_by_crate());
    assert_eq!(
        snapshot.camt_cash_transactions().descriptors(),
        ["urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"]
    );
    assert_eq!(
        snapshot.camt_cash_transactions().storage_period_days(),
        Some(90)
    );
    assert_eq!(
        snapshot.mt940_cash_transactions().advertised_versions(),
        [9, 7]
    );
    assert!(snapshot.mt940_cash_transactions().supports_version(7));
    assert!(!snapshot.mt940_cash_transactions().supports_version(9));
    assert_eq!(
        snapshot.mt940_cash_transactions().tan_required(),
        Some(true)
    );

    assert_eq!(snapshot.depot_positions().advertised_versions(), [6]);
    assert_eq!(snapshot.depot_positions().tan_required(), Some(false));
    assert_eq!(snapshot.depot_transactions().advertised_versions(), [5]);
    assert_eq!(snapshot.depot_transactions().tan_required(), Some(true));
    assert_eq!(
        snapshot.depot_transactions().storage_period_days(),
        Some(90)
    );
    assert_eq!(
        snapshot.credit_card_transactions().advertised_versions(),
        [1]
    );
    assert_eq!(
        snapshot.credit_card_transactions().tan_required(),
        Some(false)
    );
    assert_eq!(
        snapshot.credit_card_transactions().storage_period_days(),
        Some(90)
    );
    assert_eq!(snapshot.credit_card_balance().advertised_versions(), [1]);
    assert_eq!(snapshot.credit_card_balance().tan_required(), Some(true));

    assert!(
        snapshot
            .parameter_segments()
            .iter()
            .any(|segment| { segment.code() == "HIXYZS" && segment.version() == 2 })
    );
    assert!(
        snapshot
            .parameter_segments()
            .iter()
            .any(|segment| { segment.code() == "HITANS" && segment.version() == 8 })
    );
    assert!(
        snapshot
            .parameter_segments()
            .iter()
            .all(|segment| segment.code() != "HIRMS")
    );
    assert!(!format!("{snapshot:?}").contains("12345678"));
}

// HBCI 2.2 IV.6 and VII.4.3.1: legacy HIWPDS 5 has maximum orders and
// minimum signatures followed immediately by its three-component operation
// parameter DEG. FinTS Messages 2022 C.4.3.1 inserts security class before the
// otherwise equivalent HIWPDS 6 DEG. These independently derived segments
// prove both envelopes are retained and ordered by implemented operation
// version, rather than by arrival order.
#[test]
fn depot_position_parameter_versions_five_and_six_are_negotiated() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+62+280:12345678+Fictional Bank+9+1+300".into(),
            "HIWPDS:4:5:3+1+1+J:N:J".into(),
            "HIWPDS:5:6:3+1+1+0+N:J:N".into(),
            "HIWPDS:6:4:3+1+1+N:N:N".into(),
            "HIPINS:7:1:3+1+1+0+1:6:6:::HKWPD:N".into(),
        ],
        "depot-versions",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    assert_eq!(state.depot_position_versions, [6, 5]);
    assert_eq!(state.depot_positions_requires_tan, Some(false));
    let capability = state.advertised_capabilities().depot_positions().clone();
    assert_eq!(capability.advertised_versions(), [6, 5, 4]);
    assert!(capability.supports_version(6));
    assert!(capability.supports_version(5));
    assert!(!capability.supports_version(4));
}

// HBCI 2.2 VII.4.3.1 advertises HIWPDS 5 independently of the current
// version-6 definition. A version-5-only BPD must therefore remain executable
// instead of degrading to DepotPositionsVersion.
#[test]
fn depot_position_parameter_version_five_is_supported_alone() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+57+280:12345678+Fictional Bank+9+1+300".into(),
            "HIWPDS:4:5:3+1+1+J:J:J".into(),
        ],
        "depot-five-only",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    let capability = state.advertised_capabilities().depot_positions().clone();
    assert_eq!(capability.advertised_versions(), [5]);
    assert!(capability.supports_version(5));
    assert!(capability.supported_by_crate());
}

// FinTS parameter-segment DD entries distinguish operation-defining fields
// from values this crate never sends or consumes. Unusable sub-records degrade
// only their operation while well-formed sibling capabilities survive.
#[test]
fn discarded_bpd_parameters_do_not_abort_the_capability_set() {
    let descriptor = "urn?:iso?:std?:iso?:20022?:tech?:xsd?:camt.052.001.08";
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+70+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HICAZS:4:1:3+1+1+0+unreadable:X:X:{descriptor}:"),
            "HIKAZS:5:7:3".into(),
            "HIWPDS:6:6:3".into(),
            "HIWDUS:7:5:3+1+1+0+unreadable".into(),
            "HIKKUS:8:1:3+1+1+0+90:X:J:J".into(),
            concat!(
                "HIPINS:9:1:3+1+1+0+4:6:6:::HKSAL:X:HKCAZ:N:",
                "HKKAZ:J:HKWPD:N:HKWDU:X:HKKKU:N"
            )
            .into(),
        ],
        "discarded-bpd-values",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    let snapshot = state.advertised_capabilities();
    assert!(snapshot.camt_cash_transactions().supported_by_crate());
    assert_eq!(
        snapshot.camt_cash_transactions().storage_period_days(),
        None
    );
    assert!(snapshot.mt940_cash_transactions().supports_version(7));
    assert!(snapshot.depot_positions().supported_by_crate());
    assert!(snapshot.depot_transactions().advertised());
    assert!(!snapshot.depot_transactions().supported_by_crate());
    assert_eq!(snapshot.depot_transactions().storage_period_days(), None);
    assert!(snapshot.credit_card_transactions().supported_by_crate());
    assert_eq!(
        snapshot.credit_card_transactions().storage_period_days(),
        Some(90)
    );
    assert_eq!(snapshot.balance().tan_required(), None);
    assert_eq!(
        snapshot.camt_cash_transactions().tan_required(),
        Some(false)
    );
    assert_eq!(snapshot.depot_transactions().tan_required(), None);

    for (version, parameters) in [(71, "unreadable:X:J:J"), (72, "90:X:X:J"), (73, "90:X:J:X")] {
        let unusable_card = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                format!("HIBPA:3:3:3+{version}+280:12345678+Fictional Bank+9+1+300"),
                format!("HIKKUS:4:1:3+1+1+0+{parameters}"),
            ],
            "unusable-card-parameters",
            1,
        );
        Response::parse(&unusable_card)
            .unwrap()
            .apply_parameters(&mut state)
            .unwrap();
        let snapshot = state.advertised_capabilities();
        let card = snapshot.credit_card_transactions();
        assert!(card.advertised());
        assert!(!card.supported_by_crate());
        assert_eq!(card.storage_period_days(), None);
    }
}

// Messages 2022 and G112 storage periods are caller-useful advertised facts.
// Zero is retained exactly and never treated as a malformed capability.
#[test]
fn zero_storage_periods_remain_supported_capability_facts() {
    let descriptor = "urn?:iso?:std?:iso?:20022?:tech?:xsd?:camt.052.001.08";
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+72+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HICAZS:4:1:3+1+1+0+0:X:X:{descriptor}"),
            "HIWDUS:5:5:3+1+1+0+0".into(),
            "HIKKUS:6:1:3+1+1+0+0:X:N:J".into(),
        ],
        "zero-storage-periods",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    let snapshot = state.advertised_capabilities();
    assert_eq!(
        snapshot.camt_cash_transactions().storage_period_days(),
        Some(0)
    );
    assert_eq!(snapshot.depot_transactions().storage_period_days(), Some(0));
    assert!(snapshot.depot_transactions().supported_by_crate());
    assert_eq!(
        snapshot.credit_card_transactions().storage_period_days(),
        Some(0)
    );
    assert!(snapshot.credit_card_transactions().supported_by_crate());
}

// Duplicate registered parameter segments remain an ambiguous BPD structure,
// including when the first occurrence's operation-local fields are unusable.
#[test]
fn duplicate_product_parameter_segments_remain_rejected() {
    for (first, second, structure) in [
        (
            "HIWPDS:4:6:3",
            "HIWPDS:5:6:3+1+1+0+J:J:J",
            "duplicate supported HIWPDS version",
        ),
        (
            "HIWDUS:4:5:3+1+1+0+unreadable",
            "HIWDUS:5:5:3+1+1+0+90",
            "duplicate HIWDUS version 5",
        ),
        (
            "HIKKUS:4:1:3+1+1+0+unreadable:X:J:J",
            "HIKKUS:5:1:3+1+1+0+90:J:J:J",
            "duplicate HIKKUS version 1",
        ),
    ] {
        let fixture = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                "HIBPA:3:3:3+73+280:12345678+Fictional Bank+9+1+300".into(),
                first.into(),
                second.into(),
            ],
            "duplicate-parameters",
            1,
        );
        assert!(matches!(
            Response::parse(&fixture)
                .unwrap()
                .apply_parameters(&mut ReusableState::new()),
            Err(Error::InvalidResponse {
                structure: received
            }) if received == structure
        ));
    }
}

// Messages 2022 HICAZS and the camt format registration treat the optional
// ".xsd" suffix and ASCII case as identifier normalization. Every advertised
// descriptor remains a safe generic fact. Empty or overlong sibling
// descriptors and discarded parameter flags are skipped without losing BPD.
#[test]
fn camt_descriptors_are_retained_and_supported_by_normalized_identity() {
    let supported = "URN?:ISO?:STD?:ISO?:20022?:TECH?:XSD?:CAMT.052.001.08.XSD";
    let unsupported = "urn?:iso?:std?:iso?:20022?:tech?:xsd?:camt.052.001.99";
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+8+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HICAZS:4:1:3+1+1+0+90:J:N:{unsupported}:{supported}"),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    assert_eq!(
        state
            .advertised_capabilities()
            .camt_cash_transactions()
            .descriptors()
            .len(),
        2
    );
    assert_eq!(
        state.camt_capability.as_ref().unwrap().descriptor,
        "URN:ISO:STD:ISO:20022:TECH:XSD:CAMT.052.001.08.XSD"
    );

    let overlong = "x".repeat(257);
    let sparse = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+9+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HICAZS:4:1:3+1+1+0+unreadable:X:X:{supported}::{overlong}"),
        ],
        "dialog2",
        1,
    );
    Response::parse(&sparse)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    let snapshot = state.advertised_capabilities();
    let capability = snapshot.camt_cash_transactions();
    assert_eq!(
        capability.descriptors(),
        ["URN:ISO:STD:ISO:20022:TECH:XSD:CAMT.052.001.08.XSD"]
    );
    assert!(capability.supported_by_crate());
    assert_eq!(capability.storage_period_days(), None);
}

// Gate 3 fictional Atruvia profile. The interoperability lead is non-authoritative;
// the accepted camt-only combination is governed by FinTS Messages 2022-04-15,
// C.2.3.1.1.1, Formals D/E, and PIN/TAN correction T31.
#[test]
fn gate3_atruvia_profile_accepts_advertised_camt_without_legacy_turnover() {
    let descriptor = "urn?:iso?:std?:iso?:20022?:tech?:xsd?:camt.052.001.08";
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+17+280:12345678+Fictional Cooperative Bank+9+1+300".into(),
            format!("HICAZS:4:1:3+1+1+0+90:J:N:{descriptor}"),
            "HIPINS:5:1:3+1+1+0+4:6:6:::HKCAZ:N".into(),
            "HIUPA:6:4:3+fictional-user+4+0".into(),
            concat!(
                "HIUPD:7:6:3+111111::280:12345678+DE40123456780000111111",
                "+fictional-customer+1+EUR+Fictional Person++Checking++HKCAZ:1"
            )
            .into(),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();

    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    assert!(state.transaction_capability_advertised);
    assert!(state.camt_capability.is_some());
    assert!(state.legacy_transaction_versions.is_empty());
    assert_eq!(state.camt_requires_tan, Some(false));
    assert!(state.accounts()[0].allows_booked_transactions());
}

// Gate 3 fictional Finanz Informatik profile. The independently written response
// exercises protocol-advertised duplicate/simultaneous versions under Formals D/E,
// Messages C.2.1.1.1 and C.2.1.2, plus return code 3076.
#[test]
fn gate3_finanz_informatik_profile_deduplicates_negotiated_versions() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted+3076::SCA not required".into(),
            "HIBPA:3:3:3+23+280:12345678+Fictional Savings Bank+9+1+300".into(),
            "HISALS:4:6:3+1+1+0+N".into(),
            "HISALS:5:8:3+1+1+0+N".into(),
            "HISALS:6:8:3+1+1+0+N".into(),
            "HIKAZS:7:6:3+1+1+0+90:J:N".into(),
            "HIKAZS:8:7:3+1+1+0+90:J:N".into(),
            "HIKAZS:9:7:3+1+1+0+90:J:N".into(),
            "HIPINS:10:1:3+1+1+0+4:6:6:::HKSAL:N:HKKAZ:N".into(),
            "HIUPA:11:4:3+fictional-user+5+0".into(),
            concat!(
                "HIUPD:12:6:3+222222::280:12345678+DE40123456780000222222",
                "+fictional-customer+1+EUR+Fictional Person++Checking",
                "++HKSAL:1+HKKAZ:1+HKSAL:1"
            )
            .into(),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();

    let response = Response::parse(&fixture).unwrap();
    response.apply_parameters(&mut state).unwrap();

    assert_eq!(state.balance_versions, [8, 6]);
    assert_eq!(state.legacy_transaction_versions, [7, 6]);
    assert_eq!(state.balance_requires_tan, Some(false));
    assert_eq!(state.legacy_transactions_require_tan, Some(false));
    assert_eq!(response.responses()[1].code(), 3076);
}

// HBCI 2.2 VII.2.2, FinTS Formals D, and Messages 2022 C.2.1.2:
// every HISALS header version is
// a safe generic capability fact, while request negotiation remains limited to
// the independently implemented HKSAL/HISAL versions 5-8.
#[test]
fn balance_advertisements_preserve_unsupported_versions_without_selecting_them() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+31+280:12345678+Fictional Bank+9+1+300".into(),
            "HISALS:4:4:3+fictional unsupported parameters".into(),
            "HISALS:5:8:3+1+1+0+N".into(),
            "HISALS:6:9:3+fictional unsupported parameters".into(),
            "HISALS:7:6:3+1+1+0+N".into(),
            "HISALS:8:9:3+fictional duplicate parameters".into(),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();
    state.bpd_version = 30;
    state.advertised_balance_versions = vec![7];
    state.balance_versions = vec![7];

    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    assert_eq!(
        state
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [9, 8, 6, 4]
    );
    assert_eq!(state.balance_versions, [8, 6]);
    assert!(
        state
            .advertised_capabilities()
            .balance()
            .supports_version(8)
    );
    assert!(
        !state
            .advertised_capabilities()
            .balance()
            .supports_version(9)
    );

    let unsupported_only = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+32+280:12345678+Fictional Bank+9+1+300".into(),
            "HISALS:4:4:3+fictional unsupported parameters".into(),
        ],
        "dialog2",
        1,
    );
    Response::parse(&unsupported_only)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(
        state
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [4]
    );
    assert!(state.balance_versions.is_empty());
}

// HBCI 2.2 VII.2.2 defines meaning-neutral maximum-order/signature parameters.
// Zero maximum orders and trailing extension elements do not change HKSAL 5.
#[test]
fn hisals_five_is_validated_and_selected_deterministically() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+57+280:12345678+Fictional Bank+9+1+300".into(),
            "HISALS:4:5:3+0+1+ignored+extension".into(),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(
        state
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [5]
    );
    assert_eq!(state.balance_versions, [5]);
    assert!(
        state
            .advertised_capabilities()
            .balance()
            .supports_version(5)
    );

    let mixed = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+58+280:12345678+Fictional Bank+9+1+300".into(),
            "HISALS:4:5:3+1+1".into(),
            "HISALS:5:8:3+1+1+0+N".into(),
            "HISALS:6:6:3+1+1+0+N".into(),
        ],
        "dialog2",
        1,
    );
    Response::parse(&mixed)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(
        state
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [8, 6, 5]
    );
    assert_eq!(state.balance_versions, [8, 6, 5]);

    for parameters in ["1", "1+4"] {
        let malformed = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                "HIBPA:3:3:3+59+280:12345678+Fictional Bank+9+1+300".into(),
                format!("HISALS:4:5:3+{parameters}"),
            ],
            "dialog3",
            1,
        );
        Response::parse(&malformed)
            .unwrap()
            .apply_parameters(&mut state)
            .unwrap();
        assert_eq!(state.bpd_version(), 59);
        assert_eq!(
            state
                .advertised_capabilities()
                .balance()
                .advertised_versions(),
            [5]
        );
        assert!(state.balance_versions.is_empty());
    }
}

// FinTS Formals C.3.2.2, F.2 [IF3], and correction P26: a changed BPD
// version may wrap or reset, and a transmitted BPD set is complete. Same-version HIBPA
// without repeated business parameter segments must preserve every retained
// capability; a malformed new set must not partially mutate it.
#[test]
fn same_version_hibpa_preserves_all_retained_bpd_capabilities() {
    let mut state = ReusableState::new();
    state.bpd_version = 57;
    state.balance_versions = vec![5];
    state.advertised_balance_versions = vec![5];
    state.balance_capability_advertised = true;
    state.balance_requires_tan = Some(false);
    state.transaction_capability_advertised = true;
    state.camt_capability = Some(CamtCapability {
        descriptor: "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08".to_owned(),
    });
    state.legacy_transaction_versions = vec![7];
    state.camt_requires_tan = Some(false);
    state.legacy_transactions_require_tan = Some(true);
    state.depot_positions_advertised = true;
    state.depot_position_versions = vec![6];
    state.depot_positions_requires_tan = Some(false);
    state.securities_transactions_advertised = true;
    state.securities_transactions_supported = true;
    state.securities_transactions_requires_tan = Some(false);
    state.credit_card_transactions_advertised = true;
    state.credit_card_transactions = Some(CreditCardCapability {
        account_required: true,
        date_range_allowed: true,
    });
    state.credit_card_transactions_requires_tan = Some(false);
    state.credit_card_balance_advertised = true;
    state.credit_card_balance_account_required = Some(true);
    state.credit_card_balance_requires_tan = Some(false);
    state.tan_methods.push(TanMethod {
        security_function: "942".to_owned(),
        hktan_version: 6,
        process: TanProcess::ProcessVariantTwo,
        technical_id: "fictional-method".to_owned(),
        display_name: "Fictional approval".to_owned(),
        dk_method: None,
        max_tan_length: Some(6),
        tan_format: Some("1".to_owned()),
        medium_name_required: false,
        hhd_response_required: false,
        max_decoupled_polls: None,
        first_poll_delay_seconds: None,
        next_poll_delay_seconds: None,
        manual_polling_allowed: false,
        automatic_polling_allowed: false,
        #[cfg(feature = "development-diagnostics")]
        development_medium_requirement: None,
    });

    let unchanged = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+57+280:12345678+Fictional Bank+9+1+300".into(),
        ],
        "dialog1",
        1,
    );
    Response::parse(&unchanged)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    assert_eq!(state.balance_versions, [5]);
    assert_eq!(
        state
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [5]
    );
    assert_eq!(
        state.camt_capability.as_ref().unwrap().descriptor,
        "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"
    );
    assert_eq!(state.legacy_transaction_versions, [7]);
    assert_eq!(state.depot_position_versions, [6]);
    assert!(state.securities_transactions_supported);
    assert!(state.credit_card_transactions.is_some());
    assert_eq!(state.credit_card_balance_account_required, Some(true));
    assert_eq!(state.tan_methods().len(), 1);

    let wrapped = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+56+280:12345678+Fictional Bank+9+1+300".into(),
            "HISALS:4:6:3+1+1+0+N".into(),
        ],
        "dialog2",
        1,
    );
    Response::parse(&wrapped)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(state.bpd_version(), 56);
    assert_eq!(state.balance_versions, [6]);
    assert!(state.depot_position_versions.is_empty());

    let malformed_new = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+58+280:12345678+Fictional Bank+9+1+300".into(),
            "HIPINS:4:1:3+1+1+0+4:6:6:::HKSAL:X".into(),
        ],
        "dialog3",
        1,
    );
    Response::parse(&malformed_new)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(state.bpd_version(), 58);
    assert!(state.balance_versions.is_empty());
    assert_eq!(state.balance_requires_tan, None);

    let complete_new = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+59+280:12345678+Fictional Bank+9+1+300".into(),
            "HISALS:4:5:3+1+1".into(),
            "HIPINS:5:1:3+1+1+0+4:6:6:::HKSAL:N".into(),
        ],
        "dialog4",
        1,
    );
    Response::parse(&complete_new)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(state.bpd_version(), 59);
    assert_eq!(state.balance_versions, [5]);
    assert_eq!(state.balance_requires_tan, Some(false));
    assert!(!state.transaction_capability_advertised);
    assert!(!state.depot_positions_advertised);
    assert!(!state.securities_transactions_advertised);
    assert!(!state.credit_card_transactions_advertised);
    assert!(!state.credit_card_balance_advertised);
    assert!(state.tan_methods().is_empty());
}

// Gate 3 fictional independent-institution legacy profile. FinTS Messages
// C.2.1.1.1 and C.2.1.2.1 explicitly retain national HKKAZ/HKSAL version 6.
#[test]
fn gate3_independent_legacy_profile_accepts_national_account_operations() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+5+280:12345678+Fictional Private Bank+9+1+300".into(),
            "HISALS:4:6:3+1+1+0+N".into(),
            "HIKAZS:5:6:3+1+1+0+90:J:N".into(),
            "HIPINS:6:1:3+1+1+0+4:6:6:::HKSAL:N:HKKAZ:N".into(),
            "HIUPA:7:4:3+fictional-user+2+0".into(),
            concat!(
                "HIUPD:8:6:3+333333::280:12345678++fictional-customer+1",
                "+EUR+Fictional Person++Checking++HKSAL:1+HKKAZ:1"
            )
            .into(),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();

    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    assert_eq!(state.balance_versions, [6]);
    assert_eq!(state.legacy_transaction_versions, [6]);
    assert!(state.accounts()[0].iban().is_none());
    assert_eq!(state.accounts()[0].account_number(), Some("333333"));
}

// Gate 3 fictional independent-institution SCA profile. The current official
// return-code register defines 9075 as strong authentication required and 9185
// as an unsupported or obsolete FinTS/HBCI version.
#[test]
fn gate3_independent_sca_profile_exposes_actionable_recovery_categories() {
    let fixture = message(
        &["HIRMG:2:2+9075::strong authentication required+9185::version unsupported".into()],
        "dialog1",
        1,
    );
    let response = Response::parse(&fixture).unwrap();

    assert_eq!(
        response.responses()[0].recovery(),
        Some(crate::Recovery::StrongAuthenticationRequired)
    );
    assert_eq!(
        response.responses()[1].recovery(),
        Some(crate::Recovery::CorrectEndpoint)
    );
}

// FinTS Formals 2017-10-06, E.3 and HIUPD 6: non-account-bound operation
// records may omit the account binding. The published Data Dictionary correction
// additionally permits cutting the erroneous 35th IBAN character.
#[test]
fn gate3_upd_skips_non_account_records_and_applies_iban_length_correction() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+9+280:12345678+Fictional Direct Bank+9+1+300".into(),
            "HIUPA:4:4:3+fictional-user+6+0".into(),
            "HIUPD:5:6:3+++fictional-customer+++++++HKTAB:1".into(),
            concat!(
                "HIUPD:6:6:3+444444::280:12345678",
                "+DE40123456780000123456ABCDEFGHIJKLM",
                "+fictional-customer+1+EUR+Fictional Person++Checking++HKSAL:1"
            )
            .into(),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();

    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    assert_eq!(state.accounts().len(), 1);
    assert_eq!(
        state.accounts()[0].iban(),
        Some("DE40123456780000123456ABCDEFGHIJKL")
    );
}

fn fictional_tan_medium(
    version: u16,
    class: &str,
    name: Option<&str>,
    masked_phone: Option<&str>,
    ktv: bool,
) -> String {
    // This fixture construction is independent of the parser. PIN/TAN 2020 DD
    // TAN-Medium-Liste 2-5: ktv occupies four flat components. Versions 2-4
    // therefore place field 10 at component 13, while v5 places field 11 at
    // component 14 after its additional security-function field.
    let (component_count, ktv_start, name_index, masked_phone_index) = match version {
        2 => (13, 5, 12, None),
        3 | 4 => (14, 5, 12, Some(13)),
        5 => (15, 6, 13, Some(14)),
        _ => panic!("fixture supports TAN-Medium-Liste 2-5"),
    };
    let mut components = vec![""; component_count];
    components[0] = class;
    components[1] = "1";
    if ktv {
        components[ktv_start] = "654321";
        components[ktv_start + 1] = "01";
        components[ktv_start + 2] = "280";
        components[ktv_start + 3] = "12345678";
    }
    if let Some(name) = name {
        components[name_index] = name;
    }
    if let (Some(index), Some(masked_phone)) = (masked_phone_index, masked_phone) {
        components[index] = masked_phone;
    }
    components.join(":")
}

// FinTS 3.0 PIN/TAN 2020-07-10, C.3.1.1 and Data Dictionary
// "TAN-Medium-Liste" 5.
#[test]
fn hitab_five_preserves_only_the_safe_medium_identity_fields() {
    let medium = fictional_tan_medium(5, "M", Some("Fictional phone"), Some("?+49***123"), false);
    let mut bilateral = vec![""; 14];
    bilateral[0] = "B";
    bilateral[1] = "1";
    bilateral[2] = "free-form";
    bilateral[13] = "Fictional bilateral";
    let bilateral = bilateral.join(":");
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            format!("HITAB:3:5:4+1+{medium}+{bilateral}"),
        ],
        "dialog1",
        1,
    );

    let response = Response::parse(&fixture).unwrap();
    let media = response
        .tan_media(5, None)
        .unwrap()
        .expect("fictional HITAB is present");

    assert_eq!(media.len(), 2);
    assert_eq!(media[0].class(), crate::TanMediumClass::Mobile);
    assert_eq!(media[0].status(), crate::TanMediumStatus::Active);
    assert_eq!(media[0].name(), Some("Fictional phone"));
    assert_eq!(media[0].masked_phone(), Some("+49***123"));
    assert_eq!(media[1].class(), crate::TanMediumClass::Bilateral);
    assert_eq!(media[1].security_function(), Some("free-form"));

    let missing_name = fictional_tan_medium(5, "M", None, Some("?+49***123"), false);
    let generator = fictional_tan_medium(5, "G", Some("Fictional generator"), None, false);
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            format!("HITAB:3:5:4+1+{missing_name}+{generator}"),
        ],
        "dialog2",
        1,
    );
    let media = Response::parse(&fixture)
        .unwrap()
        .tan_media(5, None)
        .unwrap()
        .unwrap();
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].class(), crate::TanMediumClass::Generator);
}

// PIN/TAN 2020 archived E.2.1.2-E.2.1.4 and DD TAN-Medium-Liste 2-4
// define the legacy request versions and flat component positions.
#[test]
fn legacy_hitab_versions_use_their_independent_media_layouts() {
    for (version, name, masked_phone) in [
        (2, "Fictional list", None),
        (3, "Fictional SMS", Some("?+49***345")),
        (4, "Fictional Push", Some("?+49***456")),
    ] {
        let medium = fictional_tan_medium(version, "M", Some(name), masked_phone, false);
        let fixture = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                format!("HITAB:3:{version}:4+1+{medium}"),
            ],
            "legacy-media",
            1,
        );
        let media = Response::parse(&fixture)
            .unwrap()
            .tan_media(version, None)
            .unwrap()
            .unwrap();
        assert_eq!(media[0].name(), Some(name));
        assert_eq!(
            media[0].masked_phone(),
            masked_phone.map(|value| value.trim_start_matches('?'))
        );
        assert_eq!(media[0].security_function(), None);
    }
}

// The populated national account DEG proves the component offset rather than
// relying on a run of empty placeholders. The account values are not retained.
#[test]
fn hitab_four_and_five_names_follow_the_flattened_ktv_group() {
    for version in [4, 5] {
        let medium = fictional_tan_medium(version, "G", Some("Fictional Generator"), None, true);
        let fixture = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                format!("HITAB:3:{version}:4+1+{medium}"),
            ],
            "flattened-account-group",
            1,
        );
        let media = Response::parse(&fixture)
            .unwrap()
            .tan_media(version, None)
            .unwrap()
            .unwrap();
        assert_eq!(media[0].name(), Some("Fictional Generator"));
    }
}

// Values immediately following the nested ktv are validity dates, not medium
// designations. A mobile entry with only that old, incorrect parser position
// occupied remains unusable instead of leaking the date into HKTAN DE 12.
#[test]
fn hitab_valid_from_is_not_misread_as_the_medium_name() {
    for (version, valid_from_index) in [(4, 9), (5, 10)] {
        let mut components = fictional_tan_medium(version, "M", None, None, false)
            .split(':')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        components[valid_from_index] = "20260730".to_owned();
        let medium = components.join(":");
        let fixture = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                format!("HITAB:3:{version}:4+1+{medium}"),
            ],
            "valid-from-is-not-name",
            1,
        );
        assert!(
            Response::parse(&fixture)
                .unwrap()
                .tan_media(version, None)
                .unwrap()
                .unwrap()
                .is_empty()
        );
    }
}

// PIN/TAN 2020 archived E.2.1.2/E.2.1.4 and DD TAN-Medium-Liste
// 2/3/4/5: card number and sequence form one conditional class-G group. The
// crate does not expose or consume either meaning-neutral identifier.
#[test]
fn hitab_generator_accepts_a_completely_absent_discarded_card_group() {
    for version in 2..=5 {
        let expected_name = format!("Fictional Generator {version}");
        let medium = fictional_tan_medium(version, "G", Some(&expected_name), None, false);
        let fixture = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                format!("HITAB:3:{version}:4+1+{medium}"),
            ],
            "generator-without-card-group",
            1,
        );
        let media = Response::parse(&fixture)
            .unwrap()
            .tan_media(version, None)
            .unwrap()
            .unwrap();
        assert_eq!(media.len(), 1);
        assert_eq!(media[0].class(), crate::TanMediumClass::Generator);
        assert_eq!(media[0].name(), Some(expected_name.as_str()));
    }
}

// The card pair is neither exposed nor consumed. Partial generator occupancy
// and card fields on other classes are read past without retaining identifiers.
#[test]
fn hitab_discarded_card_fields_never_reject_the_medium_list() {
    for (version, card_number, card_sequence, expected_class) in [
        (
            2,
            Some("fictional-card-2"),
            None,
            crate::TanMediumClass::Generator,
        ),
        (3, None, Some("3"), crate::TanMediumClass::Generator),
        (4, None, Some("4"), crate::TanMediumClass::Generator),
        (
            5,
            Some("discarded-card-5"),
            Some("5"),
            crate::TanMediumClass::Mobile,
        ),
    ] {
        let class = if expected_class == crate::TanMediumClass::Mobile {
            "M"
        } else {
            "G"
        };
        let name = format!("Fictional medium {version}");
        let mut medium = fictional_tan_medium(version, class, Some(&name), None, false)
            .split(':')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let card_index = if version == 5 { 3 } else { 2 };
        if let Some(card_number) = card_number {
            medium[card_index] = card_number.to_owned();
        }
        if let Some(card_sequence) = card_sequence {
            medium[card_index + 1] = card_sequence.to_owned();
        }
        let medium = medium.join(":");
        let fixture = message(
            &[
                "HIRMG:2:2+0010::accepted".into(),
                format!("HITAB:3:{version}:4+1+{medium}"),
            ],
            "discarded-card-fields",
            1,
        );
        let media = Response::parse(&fixture)
            .unwrap()
            .tan_media(version, None)
            .unwrap()
            .unwrap();
        assert_eq!(media.len(), 1);
        assert_eq!(media[0].class(), expected_class);
    }
}

// Formals C.10: every HITABS occurrence advertises the same HKTAB/HITAB
// version, and the complete BPD set must remain available after persistence.
#[test]
fn hitabs_versions_are_retained_as_ordered_media_discovery_capabilities() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:4+81+280:12345678+Fictional Bank+9+1+300".into(),
            "HITABS:4:2:4+1+1+0".into(),
            "HITABS:5:5:4+1+1+0".into(),
            "HITABS:6:3:4+1+1+0".into(),
            "HITABS:7:4:4+1+1+0".into(),
            "HITABS:8:2:4+1+1+0".into(),
        ],
        "media-bpd",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(state.advertised_tan_media_versions(), [5, 4, 3, 2]);
}

// Entry-local discarded fields and unknown records never hide usable siblings.
// The operation envelope's negotiated version and request reference remain strict.
#[test]
fn legacy_hitab_skips_unusable_entries_but_rejects_envelope_mismatch() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            concat!(
                "HITAB:3:4:4+unexpected+X:1",
                "+L:1+M:1:::::::::::Fictional Push"
            )
            .into(),
        ],
        "malformed-media",
        1,
    );
    let media = Response::parse(&fixture)
        .unwrap()
        .tan_media(4, None)
        .unwrap()
        .unwrap();
    assert_eq!(media.len(), 2);
    assert_eq!(media[0].class(), crate::TanMediumClass::List);
    assert_eq!(media[1].name(), Some("Fictional Push"));
    assert_eq!(media[1].masked_phone(), None);

    let cross_version_class = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HITAB:3:2:4+1+S:1".into(),
        ],
        "invalid-class",
        1,
    );
    let media = Response::parse(&cross_version_class)
        .unwrap()
        .tan_media(2, None)
        .unwrap()
        .unwrap();
    assert_eq!(media[0].class(), crate::TanMediumClass::Secoder);

    let wrong_reference = message(
        &["HIRMG:2:2+0010::accepted".into(), "HITAB:3:4:7+1".into()],
        "wrong-reference",
        1,
    );
    assert!(matches!(
        Response::parse(&wrong_reference)
            .unwrap()
            .tan_media(4, Some(3)),
        Err(Error::InvalidResponse {
            structure: "tan_media.HITAB.reference"
        })
    ));

    assert!(matches!(
        Response::parse(&wrong_reference)
            .unwrap()
            .tan_media(2, None),
        Err(Error::InvalidResponse {
            structure: "tan_media.HITAB.version"
        })
    ));
}

// PIN/TAN 2020 B.4.3.1.3 and C.3.1.1: successful dedicated medium
// discovery returns exactly one HITAB data segment. Its repeated medium list
// may be empty, but the segment itself may not be silently inferred.
#[test]
fn hitab_presence_and_uniqueness_remain_structurally_distinct() {
    let missing = message(&["HIRMG:2:2+0010::accepted".into()], "dialog1", 1);
    assert!(
        Response::parse(&missing)
            .unwrap()
            .tan_media(5, None)
            .unwrap()
            .is_none()
    );

    let empty = message(
        &["HIRMG:2:2+0010::accepted".into(), "HITAB:3:5:4+1".into()],
        "dialog2",
        1,
    );
    assert_eq!(
        Response::parse(&empty)
            .unwrap()
            .tan_media(5, None)
            .unwrap()
            .expect("empty HITAB remains present")
            .len(),
        0
    );

    let duplicate = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HITAB:3:5:4+1".into(),
            "HITAB:4:5:4+1".into(),
        ],
        "dialog3",
        1,
    );
    assert!(matches!(
        Response::parse(&duplicate).unwrap().tan_media(5, None),
        Err(Error::InvalidResponse {
            structure: "tan_media.HITAB.duplicate"
        })
    ));
}

// Formals 2017-10-06, B.7.5 and F "Rückmeldung": bank text should be shown
// unchanged; its DEG also supplies an optional offending-field position and
// up to ten parameters. Explicit access retains them while formatting stays silent.
#[test]
fn bank_response_retains_diagnostics_but_implicit_formatting_stays_redacted() {
    let fixture = message(
        &["HIRMG:2:2+0010::Akzeptiert+9942:3,2:Ungültiges Konto 12345678:Feld:Wert".into()],
        "dialog1",
        1,
    );

    let response = Response::parse(&fixture).unwrap();
    let responses = response.responses();
    assert_eq!(responses[1].text(), "Ungültiges Konto 12345678");
    assert_eq!(responses[1].data_element_reference(), Some("3,2"));
    assert_eq!(
        responses[1].parameters(),
        &["Feld".to_owned(), "Wert".to_owned()]
    );

    let error = Error::Bank(responses[1].clone());
    let rendered = format!("{error:?} {error}");

    assert!(rendered.contains("9942"));
    assert!(!rendered.contains("Ungültiges Konto 12345678"));
    assert!(!rendered.contains("3,2"));
    assert!(!rendered.contains("Feld"));
    assert!(!rendered.contains("Wert"));
}

// Formals 2017-10-06, F "Rückmeldung" gives diagnostic field sizes, but these
// values do not control parsing or recovery. The bounded wire message remains
// the memory-safety limit, and caller-visible diagnostics stay verbatim.
#[test]
fn bank_response_retains_extended_diagnostics_without_hiding_the_response() {
    let overlong_text = "x".repeat(81);
    let overlong_parameter = "x".repeat(36);
    let parameters = std::iter::repeat_n("extra", 11).collect::<Vec<_>>();
    let fixture = message(
        &[
            "HIRMG:2:2+9050::Fehler".into(),
            format!(
                "HIRMS:3:2:4+9942:12345678:{overlong_text}:{overlong_parameter}:{}",
                parameters.join(":")
            ),
        ],
        "dialog1",
        1,
    );
    let response = Response::parse(&fixture).unwrap();
    let diagnostic = &response.responses()[1];
    assert_eq!(diagnostic.text(), overlong_text);
    assert_eq!(diagnostic.data_element_reference(), Some("12345678"));
    assert_eq!(diagnostic.parameters().len(), 12);
    assert_eq!(diagnostic.parameters()[0], overlong_parameter);
    assert_eq!(diagnostic.parameters()[11], "extra");
    let rendered = format!("{diagnostic:?} {}", Error::Bank(diagnostic.clone()));
    assert!(!rendered.contains(&overlong_text));
    assert!(!rendered.contains("12345678"));
    assert!(!rendered.contains(&overlong_parameter));

    let missing_text = message(&["HIRMG:2:2+9942:1".into()], "dialog2", 1);
    assert!(matches!(
        Response::parse(&missing_text),
        Err(Error::MissingValue {
            field: "response text"
        })
    ));
}

// Rückmeldungscodes 2026-02-03 A and B.4: 99xx historically permits
// institution-specific meanings, but every currently published code keeps its
// defined error status. The absent fictional companion has no standalone meaning.
#[test]
fn unpublished_99xx_classification_excludes_every_published_code() {
    assert!(is_unpublished_99xx(9952));
    for code in [
        9901, 9910, 9920, 9930, 9931, 9939, 9941, 9942, 9943, 9951, 9953, 9954, 9955, 9956, 9957,
        9958, 9959, 9960, 9961, 9962, 9963, 9964, 9980, 9991, 9992, 9997, 9998, 9999,
    ] {
        assert!(!is_unpublished_99xx(code), "{code} is published");
    }
}

// FinTS Formals 2017-10-06 B.7.5.2 permits additional notices alongside the
// aggregate success/warning/error result. Rückmeldungscodes 2026-02-03 B.2
// defines class 1 and its bounded DEG shape, while marking it FinTS-4-only.
// The owner-authorized FinTS 3 interoperability tolerance retains notices
// non-fatally in exact wire order.
#[test]
fn response_notices_are_nonfatal_ordered_and_explicitly_accessible() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::Akzeptiert+1010:1,2:Hinweis für Konto 12345678:alpha".into(),
            "HIRMS:3:2:4+1040:3:Parameter geändert:beta+3076::Freigabe nicht erforderlich".into(),
        ],
        "dialog1",
        1,
    );

    let response = Response::parse(&fixture).unwrap();
    assert_eq!(
        response
            .responses()
            .iter()
            .map(|response| response.code())
            .collect::<Vec<_>>(),
        [10, 1010, 1040, 3076]
    );
    assert_eq!(
        response.responses()[0].class(),
        crate::ResponseClass::Success
    );
    assert_eq!(
        response.responses()[1].class(),
        crate::ResponseClass::Notice
    );
    assert_eq!(
        response.responses()[2].class(),
        crate::ResponseClass::Notice
    );
    assert_eq!(
        response.responses()[3].class(),
        crate::ResponseClass::Warning
    );
    assert_eq!(response.responses()[1].text(), "Hinweis für Konto 12345678");
    assert_eq!(response.responses()[2].text(), "Parameter geändert");
    assert_eq!(
        response.responses()[1].data_element_reference(),
        Some("1,2")
    );
    assert_eq!(response.responses()[1].parameters(), ["alpha"]);
    assert_eq!(response.responses()[2].segment_number(), Some(4));
    assert_eq!(response.responses()[2].parameters(), ["beta"]);
    assert!(response.first_error().is_none());

    let rendered = format!("{:?}", Error::Bank(response.responses()[1].clone()));
    assert!(rendered.contains("1010"));
    assert!(!rendered.contains("Hinweis für Konto 12345678"));
    assert!(!rendered.contains("1,2"));
    assert!(!rendered.contains("alpha"));
}

// Formals B.7.5.2: only class 9 rejects the referenced request. A preceding
// class-1 notice therefore cannot become first_error() or mask the bank error.
#[test]
fn response_notice_does_not_mask_a_later_error() {
    let fixture = message(
        &["HIRMG:2:2+1010::Nur ein Hinweis+9942::Abgelehnt".into()],
        "dialog1",
        1,
    );

    let response = Response::parse(&fixture).unwrap();
    assert_eq!(
        response.responses()[0].class(),
        crate::ResponseClass::Notice
    );
    assert_eq!(response.responses()[1].class(), crate::ResponseClass::Error);
    assert_eq!(response.first_error().unwrap().code(), 9942);
}

// The response-code DE remains exactly four digits. No specification defines
// class 2, so the parser rejects it while retaining only the safe numeric code
// as structural error context and never formatting the accompanying text.
#[test]
fn undefined_response_class_retains_only_the_safe_numeric_code() {
    let fixture = message(
        &["HIRMG:2:2+2010::Fictional account 12345678".into()],
        "dialog1",
        1,
    );

    let error = match Response::parse(&fixture) {
        Err(error) => error,
        Ok(_) => panic!("undefined response class was accepted"),
    };
    assert!(matches!(
        error,
        Error::InvalidResponseCodeClass { code: 2010 }
    ));
    let rendered = format!("{error:?} {error}");
    assert!(rendered.contains("2010"));
    assert!(!rendered.contains("Fictional account 12345678"));
}

// FinTS Formals 2017-10-06, B.7.1 and B.7.5: HIRMG begins the response
// payload exactly once; unrecognized optional response segments remain skippable.
#[test]
fn message_response_order_is_strict_but_unknown_optional_segments_are_skipped() {
    let accepted = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIFOO:3:1:4+future optional data".into(),
        ],
        "dialog1",
        1,
    );
    assert!(Response::parse(&accepted).is_ok());

    let misplaced = message(
        &[
            "HIFOO:2:1:4+future optional data".into(),
            "HIRMG:3:2+0010::accepted".into(),
        ],
        "dialog1",
        1,
    );
    assert!(matches!(
        Response::parse(&misplaced),
        Err(Error::InvalidResponse {
            structure: "response.HIRMG.first"
        })
    ));

    let duplicate = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIRMG:3:2+0010::accepted".into(),
        ],
        "dialog1",
        1,
    );
    assert!(matches!(
        Response::parse(&duplicate),
        Err(Error::InvalidResponse {
            structure: "response.HIRMG.duplicate"
        })
    ));

    let empty = message(&["HIRMG:2:2".into()], "dialog1", 1);
    assert!(matches!(
        Response::parse(&empty),
        Err(Error::InvalidResponse {
            structure: "response.HIRMG.elements"
        })
    ));
}

fn typed_method(security_function: &str, technical_id: &str) -> Vec<String> {
    [
        security_function,
        "2",
        technical_id,
        "",
        "1.0",
        "Fictional typed approval",
        "6",
        "1",
        "Approval",
        "2048",
        "N",
        "1",
        "N",
        "0",
        "0",
        "N",
        "N",
        "00",
        "0",
        "N",
        "1",
        "",
        "",
        "",
        "N",
        "N",
    ]
    .map(str::to_owned)
    .to_vec()
}

// FinTS 3.0 PIN/TAN 2020-07-10 DD, "Verfahrensparameter
// Zwei-Schritt-Verfahren" 6/7 fields 19 and 21, plus B.5.1/B.5.2 HKTAN
// DE 12: a medium name is mandatory only for requirement code 2 together
// with more than one advertised active medium. The same positions govern
// HKTAN 6 and 7; an absent optional count does not invent a requirement.
#[test]
fn hitans_six_and_seven_apply_the_complete_medium_name_requirement() {
    for version in [6, 7] {
        for (requirement, active_count, expected) in [
            ("0", Some("2"), false),
            ("0", Some("not-a-count"), false),
            ("1", Some("2"), false),
            ("2", Some("2"), true),
            ("2", Some("1"), false),
            ("2", None, false),
        ] {
            let mut method = typed_method("942", "fictional-medium-condition");
            method[18] = requirement.to_owned();
            method[20] = active_count.unwrap_or_default().to_owned();
            if version == 6 {
                method.truncate(if active_count.is_some() { 21 } else { 20 });
            } else if active_count.is_none() {
                method.truncate(20);
            }
            let fixture = message(
                &[
                    "HIRMG:2:2+0010::accepted".into(),
                    "HIBPA:3:3:3+7+280:12345678+Fictional Bank+9+1+300".into(),
                    format!("HITANS:4:{version}:3+1+1+0+N:N:0:{}", method.join(":")),
                ],
                "dialog1",
                1,
            );
            let mut state = ReusableState::new();

            Response::parse(&fixture)
                .unwrap()
                .apply_parameters(&mut state)
                .unwrap();

            assert_eq!(state.tan_methods()[0].medium_name_required(), expected);
        }
    }

    let mut malformed_method = typed_method("942", "fictional-malformed-count");
    malformed_method[18] = "2".to_owned();
    malformed_method[20] = "10".to_owned();
    let malformed = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+7+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HITANS:4:7:3+1+1+0+N:N:0:{}", malformed_method.join(":")),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&malformed)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert!(!state.tan_methods()[0].medium_name_required());
}

// FinTS 3.0 PIN/TAN 2020-07-10, B.8.2 permits up to 98 repeated
// two-step-method parameter blocks in one HITANS 7 DEG.
#[test]
fn hitans_seven_accepts_more_than_two_advertised_methods() {
    let methods = [
        typed_method("940", "fictional-a"),
        typed_method("941", "fictional-b"),
        typed_method("942", "fictional-c"),
    ]
    .concat()
    .join(":");
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+7+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HITANS:4:7:3+1+1+0+N:N:0:{methods}"),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();

    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();

    assert_eq!(state.tan_methods().len(), 3);
    assert_eq!(state.tan_methods()[2].security_function(), "942");
}

// One malformed fixed-width method block is operation-local. HITANS remains
// advertised and well-formed sibling descriptions stay selectable.
#[test]
fn hitans_skips_an_unparseable_method_block_without_losing_siblings() {
    let methods = [
        typed_method("899", "fictional-invalid"),
        typed_method("942", "fictional-valid"),
    ]
    .concat()
    .join(":");
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+74+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HITANS:4:7:3+1+1+0+N:N:0:{methods}"),
        ],
        "mixed-method-blocks",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&fixture)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(state.tan_methods().len(), 1);
    assert_eq!(state.tan_methods()[0].security_function(), "942");
}

// FinTS 3.0 Formals 2017-10-06, H.1.5 cut rule; PIN/TAN 2020-07-10,
// B.8.2: trailing optional fields may be cut. A shorter unusable final
// sub-record is skipped without discarding its well-formed sibling.
#[test]
fn hitans_last_method_accepts_trailing_cut_and_skips_a_short_block() {
    let first = typed_method("940", "fictional-a");
    let trailing_cut = first[..20].join(":");
    let accepted = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+7+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HITANS:4:7:3+1+1+0+N:N:0:{trailing_cut}"),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&accepted)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(state.tan_methods().len(), 1);

    let mut with_short_tail = first;
    with_short_tail.extend_from_slice(&typed_method("941", "fictional-b")[..5]);
    let skipped = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+7+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HITANS:4:7:3+1+1+0+N:N:0:{}", with_short_tail.join(":")),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();
    Response::parse(&skipped)
        .unwrap()
        .apply_parameters(&mut state)
        .unwrap();
    assert_eq!(state.tan_methods().len(), 1);
    assert_eq!(state.tan_methods()[0].security_function(), "940");
}

// Observed protocol condition: some institutions emit fixed two-decimal amounts
// with trailing zeroes. Gate 1 accepts that deviation without changing the value.
#[test]
fn balance_accepts_exact_fractional_trailing_zeroes() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            concat!(
                "HISAL:3:8:4+DE40123456780000123456:FICTDEFFXXX::::",
                "+Fictional checking+EUR+C:512,30:EUR:20260728",
                "+D:1234,00:EUR:20260728"
            )
            .into(),
        ],
        "dialog1",
        2,
    );

    let response = Response::parse(&fixture).unwrap();
    let balance = response.balance().unwrap().unwrap();

    assert_eq!(balance.booked().amount().coefficient(), 51_230);
    assert_eq!(balance.booked().amount().scale(), 2);
    assert_eq!(balance.pending().unwrap().amount().coefficient(), 123_400);
    assert_eq!(balance.pending().unwrap().amount().scale(), 2);
}
