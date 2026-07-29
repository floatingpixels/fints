use super::*;
use crate::model::{CreditDebit, TanProcess};
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
    let length = format!("{:012}", wire.len());
    wire.replace_range(10..22, &length);
    wire.into_bytes()
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

// PIN/TAN B.9.9 and its FinTS-Füllwert definition make the concrete key bytes
// and key-parameter identifier irrelevant. HBCI DD defines roles 1, 3, and 4
// without authorizing clients to interpret their business meaning.
#[test]
fn authenticated_response_accepts_filler_variants_and_all_defined_security_roles() {
    let inner = b"HIRMG:2:2+0010::accepted'";
    let cases = [
        ("1", "1::fictional-system", "1:20260729:120000"),
        ("3", "2::fictional-system", "1"),
        ("4", "2::fictional-system", "1:20260729"),
    ];

    for (role, identity, timestamp) in cases {
        let header = fictional_encryption_header(EncryptionHeaderFixture {
            security_role: role,
            security_identity: identity,
            security_timestamp: timestamp,
            key_filler: &[0xA5; 16],
            key_identifier: "6",
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

// HBCI Security B.5.3/DD and PIN/TAN B.9.1/B.9.8-B.9.9: fixed values,
// field formats, filler bounds, and forbidden IV occupancy remain strict.
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
                security_role: "2",
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.security_role",
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
                security_timestamp: "1:2026072X:120000",
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.security_timestamp_shape",
        ),
        (
            fictional_encryption_header(EncryptionHeaderFixture {
                algorithm_codes: ["2", "18", "13"],
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
                key_identifier: "1234",
                ..EncryptionHeaderFixture::default()
            }),
            "encryption_header.key_identifier_shape",
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

    assert_encryption_header_error(
        fictional_encryption_header(EncryptionHeaderFixture {
            security_role: "1234",
            ..EncryptionHeaderFixture::default()
        }),
        "encryption_header.security_role",
    );
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

// FinTS 3.0 PIN/TAN 2020-07-10, C.3.1.1 and Data Dictionary
// "TAN-Medium-Liste" 5.
#[test]
fn hitab_five_preserves_only_the_safe_medium_identity_fields() {
    let medium = [
        "M",
        "1",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "Fictional phone",
        "?+49***123",
    ]
    .join(":");
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            format!("HITAB:3:5:4+1+{medium}"),
        ],
        "dialog1",
        1,
    );

    let response = Response::parse(&fixture).unwrap();
    let media = response.tan_media().unwrap();

    assert_eq!(media.len(), 1);
    assert_eq!(media[0].class(), crate::TanMediumClass::Mobile);
    assert_eq!(media[0].status(), crate::TanMediumStatus::Active);
    assert_eq!(media[0].name(), Some("Fictional phone"));
    assert_eq!(media[0].masked_phone(), Some("+49***123"));
}

// Formals 2017-10-06, B.7.5: free bank text is not retained in public errors.
#[test]
fn bank_response_preserves_code_but_not_private_text() {
    let fixture = message(
        &["HIRMG:2:2+9942::private credential detail".into()],
        "dialog1",
        1,
    );

    let response = Response::parse(&fixture).unwrap();
    let error = Error::Bank(response.first_error().unwrap());
    let rendered = format!("{error:?} {error}");

    assert!(rendered.contains("9942"));
    assert!(!rendered.contains("private credential detail"));
}

// FinTS Formals 2017-10-06, B.7.5.2: class 0 accepts, class 3 warns,
// and class 9 rejects the referenced request.
#[test]
fn response_classes_are_typed_and_only_normative_classes_are_accepted() {
    let fixture = message(
        &[
            "HIRMG:2:2+0010::accepted+9942::rejected".into(),
            "HIRMS:3:2:4+3040::qualified:fictional-next".into(),
        ],
        "dialog1",
        1,
    );

    let response = Response::parse(&fixture).unwrap();
    assert_eq!(
        response.responses()[0].class(),
        crate::ResponseClass::Success
    );
    assert_eq!(
        response.responses()[2].class(),
        crate::ResponseClass::Warning
    );
    assert_eq!(response.responses()[1].class(), crate::ResponseClass::Error);
    assert_eq!(response.first_error().unwrap().code(), 9942);

    let invalid = message(&["HIRMG:2:2+1010::invalid class".into()], "dialog1", 1);
    assert!(matches!(
        Response::parse(&invalid),
        Err(Error::InvalidValue {
            field: "response code class"
        })
    ));
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
            structure: "HIRMG must be the first response segment"
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
            structure: "duplicate HIRMG response segment"
        })
    ));

    let empty = message(&["HIRMG:2:2".into()], "dialog1", 1);
    assert!(matches!(
        Response::parse(&empty),
        Err(Error::InvalidResponse {
            structure: "HIRMG without response elements"
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

// FinTS 3.0 Formals 2017-10-06, H.1.5 cut rule; PIN/TAN 2020-07-10,
// B.8.2: only trailing optional fields of the last repeated method may be cut.
#[test]
fn hitans_last_method_accepts_trailing_cut_but_rejects_mid_block_cut() {
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

    let mut cut_middle = first[..20].to_vec();
    cut_middle.extend(typed_method("941", "fictional-b"));
    let rejected = message(
        &[
            "HIRMG:2:2+0010::accepted".into(),
            "HIBPA:3:3:3+7+280:12345678+Fictional Bank+9+1+300".into(),
            format!("HITANS:4:7:3+1+1+0+N:N:0:{}", cut_middle.join(":")),
        ],
        "dialog1",
        1,
    );
    let mut state = ReusableState::new();
    assert!(
        Response::parse(&rejected)
            .unwrap()
            .apply_parameters(&mut state)
            .is_err()
    );
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
