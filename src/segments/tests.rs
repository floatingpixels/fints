use super::*;

// FinTS 3.0 Formals 2017-10-06, C.5.1, HKIDN 2, HKVVB 3, HNHBK 3, HNHBS 1.
#[test]
fn anonymous_initialization_matches_independent_wire_fixture() {
    let institute = InstituteId::new("280", "12345678").unwrap();
    let product = ProductIdentity::new("PROD123", "1.0").unwrap();
    let state = ReusableState::new();

    let encoded = anonymous_initialization(&institute, &product, &state).unwrap();

    assert_eq!(
        encoded,
        b"HNHBK:1:3+000000000109+300+0+1'HKIDN:2:2+280:12345678+9999999999+0+0'HKVVB:3:3+0+0+0+PROD123+1.0'HNHBS:4:1+1'"
    );
}

// FinTS 3.0 Security HBCI 2024-06-11, B.5; PIN/TAN 2020-07-10, B.9.
#[test]
fn authenticated_message_keeps_pin_inside_binary_payload() {
    let institute = InstituteId::new("280", "12345678").unwrap();
    let credentials = Credentials::new("fictional-user", None, "private-pin").unwrap();
    let context = SecurityContext {
        institute: &institute,
        credentials: &credentials,
        system_id: "fictional-system",
        security_function: "942",
        profile_version: "2",
        dialog_id: "fictional-dialog",
        message_number: 2,
        date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
        time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
    };

    let encoded = termination(&context).unwrap();
    let expected = concat!(
        "HNHBK:1:3+000000000365+300+fictional-dialog+2'",
        "HNVSK:998:3+PIN:2+998+1+1::fictional-system+1:20260728:120000",
        "+2:2:13:@8@\0\0\0\0\0\0\0\0:5:1",
        "+280:12345678:fictional-user:V:0:0+0'",
        "HNVSD:999:1+@168@",
        "HNSHK:2:4+PIN:2+942+2+1+1+1::fictional-system+1",
        "+1:20260728:120000+1:999:1+6:10:16",
        "+280:12345678:fictional-user:S:0:0'",
        "HKEND:3:1+fictional-dialog'",
        "HNSHA:4:2+2++private-pin''",
        "HNHBS:5:1+2'"
    )
    .as_bytes();
    assert!(
        encoded == expected,
        "authenticated wire fixture did not match"
    );
    let parsed = Message::parse(&encoded).unwrap();
    let payload = parsed.payload_segments().unwrap();

    assert_eq!(parsed.segments()[1].header().unwrap().number, 998);
    assert_eq!(parsed.segments()[2].header().unwrap().number, 999);
    assert_eq!(payload[0].header().unwrap().code, b"HNSHK");
    assert_eq!(payload[1].header().unwrap().code, b"HKEND");
    assert_eq!(payload[2].header().unwrap().code, b"HNSHA");
    assert!(
        payload[2].elements()[3].components()[0]
            .as_text()
            .is_some_and(|value| value == "private-pin"),
        "PIN was not confined to the signature trailer"
    );
}
