use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};

use super::*;
use crate::{
    ResponseClass,
    model::{Account, InstituteState, OperationPermission},
};

fn response(segments: &[&str], dialog_id: &str, message_number: u16) -> Vec<u8> {
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

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 7, 28)
        .unwrap()
        .and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap())
}

fn tan_method(process: TanProcess) -> TanMethod {
    TanMethod {
        security_function: "942".to_owned(),
        hktan_version: if process == TanProcess::Decoupled {
            7
        } else {
            6
        },
        process,
        technical_id: "fictional-method".to_owned(),
        display_name: "Fictional approval".to_owned(),
        dk_method: (process == TanProcess::Decoupled).then(|| "Decoupled".to_owned()),
        max_tan_length: Some(6),
        tan_format: Some("1".to_owned()),
        medium_name_required: false,
        hhd_response_required: false,
        max_decoupled_polls: Some(1),
        first_poll_delay_seconds: Some(2),
        next_poll_delay_seconds: Some(3),
        manual_polling_allowed: process == TanProcess::Decoupled,
        automatic_polling_allowed: process == TanProcess::Decoupled,
    }
}

fn engine_with_method(process: TanProcess) -> Engine {
    let method = tan_method(process);
    let mut state = ReusableState::new();
    state.system_id = Some("fictional-system".to_owned());
    state.tan_methods.push(method);
    state.selected_tan_method = Some("942".to_owned());
    Engine::new(
        InstituteId::new("280", "12345678").unwrap(),
        ProductIdentity::new("PROD123", "1.0").unwrap(),
        Credentials::new("fictional-user", None, "private-pin").unwrap(),
        state,
    )
    .unwrap()
}

#[test]
fn malformed_reusable_state_is_rejected_before_transport_use() {
    let mut state = ReusableState::new();
    state.bpd_version = 1_000;

    let error = Engine::new(
        InstituteId::new("280", "12345678").unwrap(),
        ProductIdentity::new("PROD123", "1.0").unwrap(),
        Credentials::new("fictional-user", None, "private-pin").unwrap(),
        state,
    )
    .err()
    .unwrap();

    assert!(matches!(error, Error::Input(InputError::ReusableState)));
}

// FinTS 3.0 Formals 2017-10-06, C.5.1 and C.5.3.
#[test]
fn anonymous_parameter_dialog_uses_unsecured_termination_fixture() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    let fixture = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HIBPA:3:3:3+7+280:12345678+Fictional Bank+9+1+300",
        ],
        "anon",
        1,
    );

    engine.accept_anonymous_initialization(&fixture).unwrap();
    let termination = engine
        .termination_request(now().date(), now().time())
        .unwrap();

    assert_eq!(
        termination,
        b"HNHBK:1:3+000000000061+300+anon+2'HKEND:2:1+anon'HNHBS:3:1+2'"
    );
    assert_eq!(engine.state().bpd_version(), 7);
}

// FinTS 3.0 PIN/TAN 2020-07-10, B.4.3.1 and response code 3920.
#[test]
fn personalized_method_discovery_retains_only_safe_method_identifiers() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.selected_tan_method = None;
    let fixture = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HIRMS:3:2:4+3920::private bank wording:942",
        ],
        "method-dialog",
        1,
    );

    let result = engine.accept_initialization(&fixture, now()).unwrap();

    assert!(matches!(result, InitializationResult::ChooseTanMethod));
    assert_eq!(engine.allowed_tan_methods(), ["942"]);
    assert_eq!(engine.last_responses()[1].code(), 3920);
    assert_eq!(engine.last_responses()[1].class(), ResponseClass::Warning);
    engine.choose_tan_method("942").unwrap();
}

// FinTS 3.0 PIN/TAN 2020-07-10, B.4.3.1.3: first-use medium discovery
// is a special initialization with HKTAN referring to HKTAB, not an HKTAB order.
#[test]
fn tan_medium_discovery_uses_the_special_initialization_shape() {
    let mut engine = engine_with_method(TanProcess::Decoupled);

    let request = engine
        .tan_media_initialization_request(now().date(), now().time())
        .unwrap();
    let parsed = crate::wire::Message::parse(&request).unwrap();
    let payload = parsed.payload_segments().unwrap();
    let hktan = payload
        .iter()
        .find(|segment| segment.header().unwrap().code == b"HKTAN")
        .unwrap();

    assert_eq!(hktan.header().unwrap().version, 7);
    assert_eq!(hktan.elements()[1].components()[0].as_text().unwrap(), "4");
    assert_eq!(
        hktan.elements()[2].components()[0].as_text().unwrap(),
        "HKTAB"
    );
    assert!(
        payload
            .iter()
            .all(|segment| segment.header().unwrap().code != b"HKTAB")
    );
    assert_eq!(
        hktan.elements()[11].components()[0].as_text().unwrap(),
        "noref"
    );
}

// FinTS 3.0 Formals correction P4: UPD version zero is valid only in this dialog.
#[test]
fn upd_version_zero_accounts_remain_process_memory_only() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    let fixture = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HIUPA:3:4:3+fictional-user+0+0",
            concat!(
                "HIUPD:4:6:3+202051::280:12345678+DE02120300000000202051",
                "+fictional-customer+1+EUR+Fictional Person++Checking++HKSAL:1"
            ),
        ],
        "anon",
        1,
    );

    engine.accept_anonymous_initialization(&fixture).unwrap();

    assert_eq!(engine.accounts().len(), 1);
    assert!(engine.state().accounts().is_empty());
    assert_eq!(engine.state().upd_version(), 0);
}

// FinTS 3.0 PIN/TAN correction T32, HKTAN/HITAN 7 decoupled polling bounds.
#[test]
fn decoupled_continuation_enforces_delay_expiry_and_poll_limit() {
    let mut engine = engine_with_method(TanProcess::Decoupled);
    let fixture = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HIRMS:3:2:4+3955::approval elsewhere",
            "HITAN:4:7:4+4++fictional-reference+Approve in the fictional app",
        ],
        "dialog1",
        1,
    );
    let pending = match engine.accept_initialization(&fixture, now()).unwrap() {
        InitializationResult::Challenge(pending) => *pending,
        _ => panic!("expected a decoupled continuation"),
    };
    assert!(matches!(
        engine.initialization_request(now().date(), now().time()),
        Err(Error::InconsistentState)
    ));
    assert_eq!(
        pending.next_poll_at,
        now().checked_add_signed(TimeDelta::seconds(2))
    );

    let mut pending = pending;
    pending.method.automatic_polling_allowed = false;
    assert!(matches!(
        engine.decoupled_poll_request(&mut pending, crate::PollingMode::Automatic, now()),
        Err(Error::Unsupported(Limitation::DecoupledPolling))
    ));
    assert!(matches!(
        engine.decoupled_poll_request(&mut pending, crate::PollingMode::Manual, now()),
        Err(Error::PollTooEarly)
    ));
    engine
        .decoupled_poll_request(
            &mut pending,
            crate::PollingMode::Manual,
            now().checked_add_signed(TimeDelta::seconds(2)).unwrap(),
        )
        .unwrap();
    assert!(matches!(
        engine.decoupled_poll_request(
            &mut pending,
            crate::PollingMode::Manual,
            now().checked_add_signed(TimeDelta::seconds(5)).unwrap()
        ),
        Err(Error::PollLimitReached)
    ));

    pending.challenge.expires_at = Some(crate::Timestamp::new(now().date(), Some(now().time())));
    assert!(matches!(
        engine.decoupled_poll_request(
            &mut pending,
            crate::PollingMode::Manual,
            now().checked_add_signed(TimeDelta::seconds(6)).unwrap()
        ),
        Err(Error::ChallengeExpired)
    ));
}

// FinTS 3.0 PIN/TAN 2020-07-10, D: advertised TAN length and format.
#[test]
fn typed_tan_validation_never_exposes_the_supplied_value() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    let fixture = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HIRMS:3:2:4+0030::TAN required",
            "HITAN:4:6:4+4++fictional-reference+Use a fictional TAN",
        ],
        "dialog1",
        1,
    );
    let pending = match engine.accept_initialization(&fixture, now()).unwrap() {
        InitializationResult::Challenge(pending) => pending,
        _ => panic!("expected a typed TAN continuation"),
    };
    let tan = Tan::new("secret-value").unwrap();

    let error = engine
        .tan_submission_request(&pending, &tan, now())
        .unwrap_err();
    let rendered = format!("{error:?} {error}");

    assert!(matches!(error, Error::Input(InputError::Tan)));
    assert!(!rendered.contains("secret-value"));
}

// FinTS Formals 2017-10-06, C.6-C.7; repository safety contract:
// a server cannot keep a typed continuation alive without a local bound.
#[test]
fn repeated_typed_continuations_have_a_local_safety_bound() {
    let method = tan_method(TanProcess::ProcessVariantTwo);
    let challenge = Challenge {
        reference: "fictional-reference".to_owned(),
        text: None,
        hhd_uc: None,
        expires_at: None,
        medium_name: None,
    };
    let mut pending =
        pending_challenge(challenge, PendingOperation::Initialization, method, now()).unwrap();

    for sequence in 1..=LOCAL_CONTINUATION_LIMIT {
        pending = continued_challenge(
            pending,
            Challenge {
                reference: format!("fictional-reference-{sequence}"),
                text: None,
                hhd_uc: None,
                expires_at: None,
                medium_name: None,
            },
        )
        .unwrap();
    }
    assert!(matches!(
        continued_challenge(
            pending,
            Challenge {
                reference: "one-too-many".to_owned(),
                text: None,
                hhd_uc: None,
                expires_at: None,
                medium_name: None,
            }
        ),
        Err(Error::ContinuationLimitReached)
    ));
}

#[test]
fn date_only_challenge_expiry_does_not_invent_a_time() {
    let challenge = Challenge {
        reference: "fictional-reference".to_owned(),
        text: None,
        hhd_uc: None,
        expires_at: Some(crate::Timestamp::new(now().date(), None)),
        medium_name: None,
    };

    assert!(ensure_not_expired(&challenge, now()).is_ok());
    assert!(matches!(
        ensure_not_expired(
            &challenge,
            now().checked_add_signed(TimeDelta::days(1)).unwrap()
        ),
        Err(Error::ChallengeExpired)
    ));
}

// FinTS 3.0 Messages 2022-04-15, C.2.1.2; Formals E (UPD authorization).
#[test]
fn balance_request_requires_advertised_version_and_account_permission() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.balance_versions = vec![8];
    engine.state.balance_requires_tan = Some(false);
    engine.state.accounts.push(Account {
        iban: None,
        bic: None,
        account_number: Some("202051".to_owned()),
        subaccount: None,
        institute: Some(InstituteState {
            country_code: "280".to_owned(),
            institute_code: "12345678".to_owned(),
        }),
        currency: Some("EUR".to_owned()),
        account_type: Some(1),
        owner_name_1: Some("Fictional Person".to_owned()),
        owner_name_2: None,
        product_name: Some("Checking".to_owned()),
        allowed_operations: vec![OperationPermission {
            code: "HKSAL".to_owned(),
            required_signatures: 1,
        }],
    });
    let initialization = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        engine
            .accept_initialization(&initialization, now())
            .unwrap(),
        InitializationResult::Connected
    ));

    let request = engine
        .balance_request(0, now().date(), now().time())
        .unwrap();
    let parsed = crate::wire::Message::parse(&request).unwrap();
    let payload = parsed.payload_segments().unwrap();
    let hksal = payload
        .iter()
        .find(|segment| segment.header().unwrap().code == b"HKSAL")
        .unwrap();

    assert_eq!(hksal.header().unwrap().version, 8);
    assert_eq!(
        hksal.elements()[1].components()[2].as_text().unwrap(),
        "202051"
    );

    let mismatched = response(
        &[
            "HIRMG:2:2+0010::accepted",
            concat!(
                "HISAL:3:8:4+DE02120300000000999999:BYLADEM1001::::",
                "+Different fictional account+EUR+C:1,:EUR:20260728"
            ),
        ],
        "dialog1",
        2,
    );
    assert!(matches!(
        engine.accept_balance(&mismatched, now()),
        Err(Error::InconsistentState)
    ));
}
