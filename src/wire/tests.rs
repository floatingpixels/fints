use super::*;

fn message_with_middle_segment(middle: Segment) -> Message {
    Message {
        segments: vec![
            Segment::new(vec![
                Element::new(vec![
                    Value::text("HNHBK").unwrap(),
                    Value::text("1").unwrap(),
                    Value::text("3").unwrap(),
                ]),
                Element::text("000000000000").unwrap(),
                Element::text("300").unwrap(),
                Element::text("0").unwrap(),
                Element::text("1").unwrap(),
            ]),
            middle,
            Segment::new(vec![
                Element::new(vec![
                    Value::text("HNHBS").unwrap(),
                    Value::text("3").unwrap(),
                    Value::text("1").unwrap(),
                ]),
                Element::text("1").unwrap(),
            ]),
        ],
    }
}

// FinTS 3.0 Formals 2017-10-06, H.1.1-H.1.5 and B.3.3.
#[test]
fn independently_written_wire_message_parses_and_reencodes() {
    let fixture = b"HNHBK:1:3+000000000071+300+0+1'HKVVB:2:3+0+0+0+Produkt+1.0'HNHBS:3:1+1'";

    let parsed = Message::parse(fixture).unwrap();

    assert_eq!(parsed.segments().len(), 3);
    assert_eq!(parsed.encode().unwrap(), fixture);
}

// FinTS 3.0 Formals 2017-10-06, H.1.3-H.1.4.
#[test]
fn escaping_and_binary_data_are_independent_syntax_cases() {
    let middle = Segment::new(vec![
        Element::new(vec![
            Value::text("HITST").unwrap(),
            Value::text("2").unwrap(),
            Value::text("1").unwrap(),
        ]),
        Element::text("A+B:C'D?E@F").unwrap(),
        Element::new(vec![Value::binary(b"+:'?@\0".to_vec())]),
    ]);

    let encoded = message_with_middle_segment(middle).encode().unwrap();

    assert!(
        encoded
            .windows(b"A?+B?:C?'D??E?@F".len())
            .any(|window| window == b"A?+B?:C?'D??E?@F")
    );
    assert!(
        encoded
            .windows(b"@6@+:'?@\0".len())
            .any(|window| window == b"@6@+:'?@\0")
    );
    let parsed = Message::parse(&encoded).unwrap();
    assert_eq!(
        parsed.segments()[1].elements()[2].components()[0].as_binary(),
        Some(b"+:'?@\0".as_slice())
    );
    assert_eq!(parsed.encode().unwrap(), encoded);
}

// FinTS 3.0 Formals 2017-10-06, H.1.5.
#[test]
fn empty_and_trailing_cut_optional_elements_remain_distinct() {
    let fixture = b"HNHBK:1:3+000000000062+300+0+1'HITST:2:1+A++C:::F'HNHBS:3:1+1'";

    let parsed = Message::parse(fixture).unwrap();
    let segment = &parsed.segments()[1];

    assert_eq!(segment.elements().len(), 4);
    assert_eq!(segment.elements()[2].components()[0].as_text().unwrap(), "");
    assert_eq!(segment.elements()[3].components().len(), 4);
    assert_eq!(parsed.encode().unwrap(), fixture);
}

// FinTS 3.0 Formals 2017-10-06, B.3.3 and H.1.4.
#[test]
fn malformed_lengths_fail_before_exposing_payload_data() {
    let wrong_message_length = b"HNHBK:1:3+000000000999+300+0+1'HITST:2:1+A'HNHBS:3:1+1'";
    let truncated_binary = b"HNHBK:1:3+000000000068+300+0+1'HITST:2:1+@99@abc'HNHBS:3:1+1'";

    assert!(matches!(
        Message::parse(wrong_message_length),
        Err(WireError::InvalidMessageLength { .. })
    ));
    assert!(matches!(
        Message::parse(truncated_binary),
        Err(WireError::BinaryHasTrailingData { .. } | WireError::TruncatedBinary { .. })
    ));
}

// FinTS 3.0 Formals 2017-10-06, H.1.3.
#[test]
fn malformed_escapes_are_rejected() {
    let invalid = b"HNHBK:1:3+000000000067+300+0+1'HITST:2:1+A?x'HNHBS:3:1+1'";
    let incomplete = b"HNHBK:1:3+000000000067+300+0+1'HITST:2:1+A?";

    assert!(matches!(
        Message::parse(invalid),
        Err(WireError::InvalidEscape { .. })
    ));
    assert!(matches!(
        Message::parse(incomplete),
        Err(WireError::MissingSegmentTerminator)
    ));
}

// FinTS 3.0 Formals 2017-10-06, B.5.1 and B.5.3.
#[test]
fn segment_numbers_must_be_strictly_sequential() {
    let fixture = b"HNHBK:1:3+000000000064+300+0+1'HITST:4:1+A'HNHBS:3:1+1'";

    assert!(matches!(
        Message::parse(fixture),
        Err(WireError::NonSequentialSegmentNumber {
            expected: 2,
            actual: 4,
        })
    ));
}

// FinTS 3.0 Formals 2017-10-06, B.8; PIN/TAN 2020-07-10, B.9.8-B.9.10.
#[test]
fn pin_tan_security_envelope_uses_reserved_physical_segment_numbers() {
    let fixture = b"HNHBK:1:3+000000000094+300+0+1'HNVSK:998:3+PIN:2+998'HNVSD:999:1+@12@HITST:2:1+A''HNHBS:3:1+1'";

    let parsed = Message::parse(fixture).unwrap();
    let payload = parsed.payload_segments().unwrap();

    assert_eq!(parsed.segments().len(), 4);
    assert_eq!(payload.len(), 1);
    assert_eq!(payload[0].header().unwrap().code, b"HITST");
    assert_eq!(parsed.encode().unwrap(), fixture);
}

// FinTS 3.0 Formals 2017-10-06, B.5.3 and B.8.
#[test]
fn pin_tan_security_payload_must_restore_logical_numbering() {
    let fixture = b"HNHBK:1:3+000000000094+300+0+1'HNVSK:998:3+PIN:2+998'HNVSD:999:1+@12@HITST:4:1+A''HNHBS:3:1+1'";

    assert!(matches!(
        Message::parse(fixture),
        Err(WireError::NonSequentialSegmentNumber {
            expected: 2,
            actual: 4,
        })
    ));
}

// FinTS 3.0 Formals 2017-10-06, B.1 (currently code set 1, Latin-1).
#[test]
fn outbound_text_must_be_representable_in_latin1() {
    assert!(matches!(
        Value::text("Umlaut: ä"),
        Ok(Value::Text(bytes)) if bytes == b"Umlaut: \xe4"
    ));
    assert!(matches!(
        Value::text("Euro: €"),
        Err(WireError::NonLatin1Text)
    ));
}

// Repository security contract: authenticated message data must not enter errors.
#[test]
fn wire_errors_contain_only_structure_and_safe_counts() {
    let fixture = b"HNHBK:1:3+000000000999+300+0+1'HITST:2:1+private'HNHBS:3:1+1'";

    let rendered = match Message::parse(fixture) {
        Ok(_) => panic!("fixture must fail its declared message length"),
        Err(error) => error.to_string(),
    };

    assert!(!rendered.contains("private"));
    assert_eq!(
        rendered,
        format!(
            "FinTS message length is 999 bytes, actual length is {} bytes",
            fixture.len()
        )
    );
}

// Formals segment numbering permits 999 physical segments, while HIUPD
// Erlaubte GV repeats up to 999 times. The parser bounds are 1000 and remain
// subordinate to the independent one-megabyte message bound.
#[test]
fn specification_maximum_segment_and_element_counts_fit_the_parser_bounds() {
    let make_header = |code: &str, number: usize| {
        Element::new(vec![
            Value::text(code).unwrap(),
            Value::text(&number.to_string()).unwrap(),
            Value::text("1").unwrap(),
        ])
    };
    let bounded_message = |count: usize| {
        let mut segments = Vec::with_capacity(count);
        segments.push(Segment::new(vec![
            Element::new(vec![
                Value::text("HNHBK").unwrap(),
                Value::text("1").unwrap(),
                Value::text("3").unwrap(),
            ]),
            Element::text("000000000000").unwrap(),
            Element::text("300").unwrap(),
            Element::text("0").unwrap(),
            Element::text("1").unwrap(),
        ]));
        for number in 2..count {
            segments.push(Segment::new(vec![make_header("HITST", number)]));
        }
        segments.push(Segment::new(vec![
            Element::new(vec![
                Value::text("HNHBS").unwrap(),
                Value::text(&count.to_string()).unwrap(),
                Value::text("1").unwrap(),
            ]),
            Element::text("1").unwrap(),
        ]));
        Message::new(segments).encode().unwrap()
    };

    assert_eq!(
        Message::parse(&bounded_message(999))
            .unwrap()
            .segments()
            .len(),
        999
    );
    assert!(matches!(
        Message::parse(&bounded_message(1_001)),
        Err(WireError::TooManySegments { limit: 1_000 })
    ));

    let message_with_elements = |count: usize| {
        let mut elements = Vec::with_capacity(count);
        elements.push(make_header("HITST", 2));
        elements.extend((1..count).map(|_| Element::text("X").unwrap()));
        message_with_middle_segment(Segment::new(elements))
            .encode()
            .unwrap()
    };
    assert_eq!(
        Message::parse(&message_with_elements(1_000))
            .unwrap()
            .segments()[1]
            .elements()
            .len(),
        1_000
    );
    assert!(matches!(
        Message::parse(&message_with_elements(1_001)),
        Err(WireError::TooManyElements {
            segment: 2,
            limit: 1_000
        })
    ));
}
