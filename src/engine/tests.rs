use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};

use super::*;
use crate::{
    ResponseClass,
    model::{Account, CamtCapability, InstituteState, OperationPermission},
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

fn binary_response(
    prefix: &str,
    payload: &[u8],
    suffix: &str,
    dialog_id: &str,
    message_number: u16,
) -> Vec<u8> {
    let mut wire = format!(
        "HNHBK:1:3+000000000000+300+{dialog_id}+{message_number}+{dialog_id}:1'\
         HIRMG:2:2+0010::accepted'{prefix}"
    )
    .into_bytes();
    wire.extend_from_slice(payload.len().to_string().as_bytes());
    wire.push(b'@');
    wire.extend_from_slice(payload);
    wire.extend_from_slice(suffix.as_bytes());
    let length = format!("{:012}", wire.len());
    wire[10..22].copy_from_slice(length.as_bytes());
    wire
}

fn mt940_page(statement: u16, bank_reference: &str, account_number: &str) -> Vec<u8> {
    format!(
        "\r\n:20:FICTIONAL{statement}\r\n:25:12345678/{account_number}\r\n\
         :28C:{statement}/1\r\n:60F:C260727EUR100,00\r\n\
         :61:2607280728C1,00NTRFFICTREF//{bank_reference}\r\n\
         :86:166?20SVWZ+Fictional page {statement}\r\n\
         :62F:C260728EUR101,00\r\n-"
    )
    .into_bytes()
}

fn mt535_page(page: u16, indicator: &str) -> Vec<u8> {
    format!(
        "\r\n:16R:GENL\r\n:28E:{page}/{indicator}\r\n:20C::SEME//NONREF\r\n\
         :23G:NEWM\r\n:98A::STAT//20260728\r\n\
         :22F::STTY//CUST\r\n\
         :97A::SAFE//12345678/300001\r\n:17B::ACTI//Y\r\n:16S:GENL\r\n\
         :16R:FIN\r\n:35B:ISIN DE000FINTS05\r\nFictional Security\r\n\
         :93B::AGGR//UNIT/1,\r\n:16R:SUBBAL\r\n\
         :93C::TAVI//UNIT/AVAI/1,\r\n:16S:SUBBAL\r\n:16S:FIN\r\n-"
    )
    .into_bytes()
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
        #[cfg(feature = "diagnostics")]
        development_medium_requirement: None,
    }
}

fn engine_with_method(process: TanProcess) -> Engine {
    let method = tan_method(process);
    let mut state = ReusableState::new();
    state.system_id = Some("fictional-system".to_owned());
    state.tan_methods.push(method);
    state.advertised_parameter_segments.push(
        crate::capabilities::ParameterSegmentAdvertisement::new("HITABS".to_owned(), 5),
    );
    state.selected_tan_method = Some("942".to_owned());
    Engine::new(
        InstituteId::new("280", "12345678").unwrap(),
        ProductIdentity::new("PROD123", "1.0").unwrap(),
        Credentials::new("fictional-user", None, "private-pin").unwrap(),
        state,
    )
    .unwrap()
}

fn transaction_account(iban: &str, account_number: &str, operations: &[(&str, u8)]) -> Account {
    Account {
        iban: Some(iban.to_owned()),
        bic: None,
        account_number: Some(account_number.to_owned()),
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
        allowed_operations: operations
            .iter()
            .map(|(code, required_signatures)| OperationPermission {
                code: (*code).to_owned(),
                required_signatures: *required_signatures,
            })
            .collect(),
        unlisted_operations_unknown: false,
    }
}

fn connected_transaction_engine(
    accounts: Vec<Account>,
    camt: bool,
    legacy_versions: Vec<u16>,
    camt_requires_tan: Option<bool>,
    legacy_requires_tan: Option<bool>,
) -> Engine {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = accounts;
    engine.state.transaction_capability_advertised = camt || !legacy_versions.is_empty();
    engine.state.camt_capability = camt.then(|| crate::model::CamtCapability {
        descriptor: "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08".to_owned(),
    });
    engine.state.legacy_transaction_versions = legacy_versions;
    engine.state.camt_requires_tan = camt_requires_tan;
    engine.state.legacy_transactions_require_tan = legacy_requires_tan;
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        engine.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));
    engine
}

fn connected_credit_card_engine(process: TanProcess, requires_tan: bool) -> Engine {
    let mut account = transaction_account(
        "DE40123456780000123456",
        "444433******1111",
        &[("HKKKU", 1)],
    );
    account.account_type = Some(50);
    let mut engine = engine_with_method(process);
    engine.state.accounts = vec![account];
    engine.state.credit_card_transactions_advertised = true;
    engine.state.credit_card_transactions = Some(crate::model::CreditCardCapability {
        account_required: false,
        date_range_allowed: true,
    });
    engine.state.credit_card_transactions_requires_tan = Some(requires_tan);
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        engine.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));
    engine
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

// Gate 4.6 persisted version 5 as an advertised generic fact before it became
// implemented. Reconstruct supported versions from that retained fact when the
// state enters the newer engine.
#[test]
fn gate46_state_reclassifies_advertised_balance_five_as_supported() {
    let mut state = ReusableState::new();
    state.bpd_version = 57;
    state.advertised_balance_versions = vec![5];
    assert!(state.balance_versions.is_empty());
    assert!(
        state
            .advertised_capabilities()
            .balance()
            .supports_version(5)
    );

    let engine = Engine::new(
        InstituteId::new("280", "12345678").unwrap(),
        ProductIdentity::new("PROD123", "1.0").unwrap(),
        Credentials::new("fictional-user", None, "private-pin").unwrap(),
        state,
    )
    .unwrap();

    assert_eq!(engine.state.balance_versions, [5]);
    assert_eq!(
        engine
            .state()
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [5]
    );
}

// ReusableState before HKWPD 5 support retained only the Gate-4
// depot_positions_supported boolean, whose implemented meaning was version 6.
// Preserve that migration while rejecting invented serialized version numbers.
#[test]
fn legacy_depot_position_state_migrates_to_six_and_invalid_versions_fail() {
    let institute = InstituteId::new("280", "12345678").unwrap();
    let product = ProductIdentity::new("PROD123", "1.0").unwrap();
    let mut legacy = ReusableState::new();
    legacy.legacy_depot_positions_supported = true;

    let engine = Engine::new(
        institute.clone(),
        product.clone(),
        Credentials::new("fictional-user", None, "private-pin").unwrap(),
        legacy,
    )
    .unwrap();
    assert_eq!(engine.state.depot_position_versions, [6]);

    let mut malformed = ReusableState::new();
    malformed.depot_position_versions = vec![7];
    assert!(matches!(
        Engine::new(
            institute,
            product,
            Credentials::new("fictional-user", None, "private-pin").unwrap(),
            malformed
        ),
        Err(Error::Input(InputError::ReusableState))
    ));
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

// PIN/TAN correction T8 and Formals C.3.2.2/P26: an explicit anonymous
// refresh sends client BPD version zero and atomically applies the complete
// response even when its institute-assigned version equals retained state.
#[test]
fn anonymous_refresh_replaces_same_version_incomplete_bpd() {
    let mut state = ReusableState::new();
    state.bpd_version = 57;
    let mut engine = Engine::new(
        InstituteId::new("280", "12345678").unwrap(),
        ProductIdentity::new("PROD123", "1.0").unwrap(),
        Credentials::new("fictional-user", None, "private-pin").unwrap(),
        state,
    )
    .unwrap();
    let request = engine.anonymous_initialization_request().unwrap();
    let payload = crate::wire::Message::parse(&request)
        .unwrap()
        .payload_segments()
        .unwrap();
    let hkvvb = payload
        .iter()
        .find(|segment| segment.header().unwrap().code == b"HKVVB")
        .unwrap();
    assert_eq!(hkvvb.elements()[1].components()[0].as_text().unwrap(), "0");

    let fixture = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HIBPA:3:3:3+57+280:12345678+Fictional Bank+9+1+300",
            concat!(
                "HITANS:4:6:3+1+1+0+N:N:0:942:2:fictional-method::1.0:",
                "Fictional approval:6:1:Approval:2048:N:1:N:0:0:N:N:00:0:N:1"
            ),
        ],
        "anonymous-refresh",
        1,
    );
    engine.accept_anonymous_initialization(&fixture).unwrap();

    assert_eq!(engine.state().bpd_version(), 57);
    assert_eq!(engine.state().tan_methods().len(), 1);
    assert_eq!(engine.state().tan_methods()[0].security_function(), "942");
}

// Formals C.3.2.2: institute BPD version zero is valid only for the current
// dialog. It replaces the effective capabilities without mutating reusable state.
#[test]
fn bpd_version_zero_is_effective_only_for_the_active_dialog() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.bpd_version = 57;
    engine.state.balance_versions = vec![5];
    engine.state.advertised_balance_versions = vec![5];
    engine.state.balance_capability_advertised = true;
    engine.state.balance_requires_tan = Some(false);
    engine
        .state
        .accounts
        .push(transaction_account("", "123456", &[("HKSAL", 1)]));

    let fixture = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HIBPA:3:3:3+0+280:12345678+Fictional Bank+9+1+300",
            "HISALS:4:6:3+1+1+0+N",
            "HIPINS:5:1:3+1+1+0+4:6:6:::HKSAL:N",
        ],
        "dialog1",
        1,
    );
    assert!(matches!(
        engine.accept_initialization(&fixture, now()).unwrap(),
        InitializationResult::ChooseTanMethod
    ));
    assert_eq!(engine.state().bpd_version(), 57);
    assert_eq!(
        engine
            .state()
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [5]
    );

    let request = engine
        .balance_request(0, now().date(), now().time())
        .unwrap();
    let payload = crate::wire::Message::parse(&request)
        .unwrap()
        .payload_segments()
        .unwrap();
    assert!(
        payload
            .iter()
            .any(|segment| segment.header().unwrap().code == b"HKSAL"
                && segment.header().unwrap().version == 6)
    );

    engine.abort_dialog();
    assert_eq!(engine.state().bpd_version(), 57);
    assert_eq!(
        engine
            .state()
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [5]
    );
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

    let termination = engine
        .termination_request(now().date(), now().time())
        .unwrap();
    let payload = crate::wire::Message::parse(&termination)
        .unwrap()
        .payload_segments()
        .unwrap();
    assert_eq!(
        payload
            .iter()
            .filter(|segment| segment.header().unwrap().code == b"HKEND")
            .count(),
        1
    );
    let terminated = response(
        &["HIRMG:2:2+0100::fictional termination"],
        "method-dialog",
        2,
    );
    engine.accept_termination(&terminated).unwrap();
    assert!(matches!(
        engine.termination_request(now().date(), now().time()),
        Err(Error::InconsistentState)
    ));
}

// FinTS 3.0 PIN/TAN 2020-07-10, B.4.3.1, B.6.1 response 3920,
// and B.8.2 response 9955. This independently assembled response represents
// function-999 discovery that the institute terminates without returning UPD.
#[test]
fn bank_terminated_method_discovery_applies_bpd_without_sending_hkend() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.tan_methods.clear();
    engine.state.selected_tan_method = None;
    let fixture = response(
        &[
            "HIRMG:2:2+9050::fictional partial-error summary+9800::fictional termination",
            "HIRMS:3:2:4+9955::fictional one-step result+3920::fictional methods:942",
            "HIBPA:4:3:4+57+280:12345678+Fictional Bank+9+1+300",
            concat!(
                "HITANS:5:6:4+1+1+0+N:N:0:942:2:fictional-method::1.0:",
                "Fictional approval:6:1:Approval:2048:N:1:N:0:0:N:N:00:0:N:1"
            ),
        ],
        "terminated-discovery",
        1,
    );

    let result = engine.accept_initialization(&fixture, now()).unwrap();

    assert!(matches!(result, InitializationResult::ChooseTanMethod));
    assert_eq!(engine.state().bpd_version(), 57);
    assert_eq!(engine.state().upd_version(), 0);
    assert_eq!(engine.state().tan_methods().len(), 1);
    assert_eq!(engine.state().tan_methods()[0].security_function(), "942");
    assert_eq!(engine.allowed_tan_methods(), ["942"]);
    assert_eq!(
        engine
            .last_responses()
            .iter()
            .map(|response| (response.code(), response.class(), response.segment_number()))
            .collect::<Vec<_>>(),
        vec![
            (9050, ResponseClass::Error, None),
            (9800, ResponseClass::Error, None),
            (9955, ResponseClass::Error, Some(4)),
            (3920, ResponseClass::Warning, Some(4)),
        ]
    );
    let redacted = format!("{:?}", engine.last_responses());
    assert!(!redacted.contains("fictional partial-error summary"));
    assert!(!redacted.contains("fictional methods"));

    // The institute already ended this dialog, so no HKEND request can be built.
    assert!(matches!(
        engine.termination_request(now().date(), now().time()),
        Err(Error::InconsistentState)
    ));

    engine.choose_tan_method("942").unwrap();
    let next = engine
        .initialization_request(now().date(), now().time())
        .unwrap();
    let payload = crate::wire::Message::parse(&next)
        .unwrap()
        .payload_segments()
        .unwrap();
    assert_eq!(
        payload[0].elements()[2].components()[0].as_text().unwrap(),
        "942"
    );
    assert!(
        payload
            .iter()
            .any(|segment| segment.header().unwrap().code == b"HKTAN")
    );
}

// PIN/TAN 2020 B.4.3.1 and B.6.1 establish function-999/3920 discovery;
// correction T8 requires an anonymous BPD refresh when none of the returned
// identifiers has usable method parameters. Rückmeldungscodes 2026 A says
// unpublished 99xx values have no uniform standalone meaning.
#[test]
fn unpublished_99xx_discovery_refreshes_bpd_before_method_selection() {
    let mut state = ReusableState::new();
    state.system_id = Some("fictional-existing-system".to_owned());
    let mut engine = Engine::new(
        InstituteId::new("280", "12345678").unwrap(),
        ProductIdentity::new("PROD123", "1.0").unwrap(),
        Credentials::new("fictional-user", None, "private-pin").unwrap(),
        state,
    )
    .unwrap();
    let request = engine
        .initialization_request(now().date(), now().time())
        .unwrap();
    let payload = crate::wire::Message::parse(&request)
        .unwrap()
        .payload_segments()
        .unwrap();
    assert_eq!(
        payload[0].elements()[2].components()[0].as_text().unwrap(),
        "999"
    );

    let discovery = response(
        &[
            "HIRMG:2:2+9050::fictional summary+9800::fictional termination",
            "HIRMS:3:2:4+9952::fictional unpublished companion+3920::methods:942:943",
        ],
        "terminated-discovery",
        1,
    );
    assert!(matches!(
        engine.accept_initialization(&discovery, now()).unwrap(),
        InitializationResult::RefreshParameters
    ));
    assert_eq!(engine.state().bpd_version(), 0);
    assert_eq!(engine.state().upd_version(), 0);
    assert!(engine.state().tan_methods().is_empty());
    assert_eq!(engine.allowed_tan_methods(), ["942", "943"]);
    assert!(matches!(
        engine.termination_request(now().date(), now().time()),
        Err(Error::InconsistentState)
    ));
    assert_eq!(
        engine
            .last_responses()
            .iter()
            .map(|response| (response.code(), response.class(), response.segment_number()))
            .collect::<Vec<_>>(),
        vec![
            (9050, ResponseClass::Error, None),
            (9800, ResponseClass::Error, None),
            (9952, ResponseClass::Error, Some(4)),
            (3920, ResponseClass::Warning, Some(4)),
        ]
    );
    let rendered = format!("{:?}", engine.last_responses());
    assert!(!rendered.contains("fictional summary"));
    assert!(!rendered.contains("fictional unpublished companion"));

    // Allowed identifiers are live response state, never ReusableState.
    let restarted = Engine::new(
        InstituteId::new("280", "12345678").unwrap(),
        ProductIdentity::new("PROD123", "1.0").unwrap(),
        Credentials::new("fictional-user", None, "private-pin").unwrap(),
        engine.state().clone(),
    )
    .unwrap();
    assert!(restarted.allowed_tan_methods().is_empty());

    engine.anonymous_initialization_request().unwrap();
    let refreshed = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HIBPA:3:3:3+58+280:12345678+Fictional Bank+9+1+300",
            concat!(
                "HITANS:4:6:3+1+1+0+N:N:0:942:2:fictional-method::1.0:",
                "Fictional approval:6:1:Approval:2048:N:1:N:0:0:N:N:00:0:N:1"
            ),
        ],
        "anonymous-refresh",
        1,
    );
    engine.accept_anonymous_initialization(&refreshed).unwrap();
    assert_eq!(engine.state().bpd_version(), 58);
    assert_eq!(engine.state().tan_methods().len(), 1);
    assert_eq!(engine.allowed_tan_methods(), ["942", "943"]);
    assert!(matches!(
        engine.choose_tan_method("943"),
        Err(Error::Unsupported(Limitation::TanMethod))
    ));

    let termination = engine
        .termination_request(now().date(), now().time())
        .unwrap();
    let payload = crate::wire::Message::parse(&termination)
        .unwrap()
        .payload_segments()
        .unwrap();
    assert_eq!(
        payload
            .iter()
            .filter(|segment| segment.header().unwrap().code == b"HKEND")
            .count(),
        1
    );
    let terminated = response(
        &["HIRMG:2:2+0100::fictional termination"],
        "anonymous-refresh",
        2,
    );
    engine.accept_termination(&terminated).unwrap();
    assert!(matches!(
        engine.termination_request(now().date(), now().time()),
        Err(Error::InconsistentState)
    ));

    engine.choose_tan_method("942").unwrap();
    let personalized = engine
        .initialization_request(now().date(), now().time())
        .unwrap();
    let payload = crate::wire::Message::parse(&personalized)
        .unwrap()
        .payload_segments()
        .unwrap();
    assert_eq!(
        payload[0].elements()[2].components()[0].as_text().unwrap(),
        "942"
    );
    let connected = response(
        &["HIRMG:2:2+0010::accepted", "HIRMS:3:2:4+3920::methods:942"],
        "personalized-dialog",
        1,
    );
    assert!(matches!(
        engine.accept_initialization(&connected, now()).unwrap(),
        InitializationResult::Connected
    ));
    assert!(engine.has_active_dialog());
}

// PIN/TAN B.6.1 and B.8.2 define a response set, not an ordering rule.
#[test]
fn bank_terminated_method_discovery_is_order_independent() {
    let fixtures = [
        [
            "HIRMG:2:2+9050::summary+9800::termination",
            "HIRMS:3:2:4+9955::one-step result+3920::methods:942",
        ],
        [
            "HIRMG:2:2+9800::termination+9050::summary",
            "HIRMS:3:2:4+3920::methods:942+9955::one-step result",
        ],
    ];

    for segments in fixtures {
        let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
        engine.state.selected_tan_method = None;
        let fixture = response(&segments, "terminated-discovery", 1);

        assert!(matches!(
            engine.accept_initialization(&fixture, now()).unwrap(),
            InitializationResult::ChooseTanMethod
        ));
        assert!(matches!(
            engine.termination_request(now().date(), now().time()),
            Err(Error::InconsistentState)
        ));
    }
}

// PIN/TAN B.6.1 limits 3920 parameters to 900-997 and 999. B.8.2 does not
// permit an unrelated credential error to be absorbed by method discovery.
#[test]
fn method_discovery_requires_usable_methods_and_rejects_unrelated_errors() {
    let mut invalid_methods = engine_with_method(TanProcess::ProcessVariantTwo);
    invalid_methods.state.selected_tan_method = None;
    let fixture = response(
        &[
            "HIRMG:2:2+9050::summary+9800::termination",
            "HIRMS:3:2:4+9955::one-step result+3920::methods:899:998:not-a-code",
        ],
        "terminated-discovery",
        1,
    );
    assert!(matches!(
        invalid_methods.accept_initialization(&fixture, now()),
        Err(Error::MissingValue {
            field: "valid 3920 TAN method parameter"
        })
    ));
    assert!(matches!(
        invalid_methods.termination_request(now().date(), now().time()),
        Err(Error::InconsistentState)
    ));

    let mut unrelated = engine_with_method(TanProcess::ProcessVariantTwo);
    unrelated.state.selected_tan_method = None;
    let fixture = response(
        &[
            "HIRMG:2:2+9050::summary+9800::termination+9942::credential error",
            "HIRMS:3:2:4+9952::unpublished companion+3920::methods:942",
        ],
        "rejected-discovery",
        1,
    );
    assert!(matches!(
        unrelated.accept_initialization(&fixture, now()),
        Err(Error::Bank(response)) if response.code() == 9050
    ));
    assert!(
        unrelated
            .last_responses()
            .iter()
            .any(|response| response.code() == 9942)
    );
    assert!(matches!(
        unrelated.termination_request(now().date(), now().time()),
        Err(Error::InconsistentState)
    ));

    for methods in [None, Some("899:998:not-a-code")] {
        let mut malformed = Engine::new(
            InstituteId::new("280", "12345678").unwrap(),
            ProductIdentity::new("PROD123", "1.0").unwrap(),
            Credentials::new("fictional-user", None, "private-pin").unwrap(),
            ReusableState::new(),
        )
        .unwrap();
        let method_response = methods
            .map(|values| format!("HIRMS:3:2:4+9952::companion+3920::methods:{values}"))
            .unwrap_or_else(|| "HIRMS:3:2:4+9952::companion".to_owned());
        let segments = [
            "HIRMG:2:2+9050::summary+9800::termination",
            method_response.as_str(),
        ];
        let fixture = response(&segments, "invalid-discovery", 1);
        assert!(matches!(
            malformed.accept_initialization(&fixture, now()),
            Err(Error::MissingValue { .. })
        ));
        assert!(matches!(
            malformed.termination_request(now().date(), now().time()),
            Err(Error::InconsistentState)
        ));
    }

    let mut filler_only = Engine::new(
        InstituteId::new("280", "12345678").unwrap(),
        ProductIdentity::new("PROD123", "1.0").unwrap(),
        Credentials::new("fictional-user", None, "private-pin").unwrap(),
        ReusableState::new(),
    )
    .unwrap();
    let fixture = response(
        &[
            "HIRMG:2:2+9050::summary+9800::termination",
            "HIRMS:3:2:4+9952::companion+3920::methods:999",
        ],
        "filler-only-discovery",
        1,
    );
    assert!(matches!(
        filler_only.accept_initialization(&fixture, now()),
        Err(Error::Unsupported(Limitation::TanMethod))
    ));

    let mut selected = engine_with_method(TanProcess::ProcessVariantTwo);
    let fixture = response(
        &[
            "HIRMG:2:2+9050::summary+9800::termination",
            "HIRMS:3:2:4+9952::unpublished companion+3920::methods:942",
        ],
        "selected-method",
        1,
    );
    assert!(matches!(
        selected.accept_initialization(&fixture, now()),
        Err(Error::Bank(response)) if response.code() == 9050
    ));
}

// FinTS 3.0 Formals 2017-10-06, C.8.1-C.8.2: HISYN 4 can complete
// synchronization directly or before a TAN continuation.
#[test]
fn synchronization_accepts_direct_and_continued_hisyn_fixtures() {
    let direct = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HISYN:3:4:2+fictional-direct-system",
        ],
        "direct-dialog",
        1,
    );
    let mut direct_engine = engine_with_method(TanProcess::ProcessVariantTwo);
    direct_engine.state.system_id = None;
    direct_engine
        .synchronization_request(now().date(), now().time())
        .unwrap();

    assert!(matches!(
        direct_engine
            .accept_synchronization(&direct, now())
            .unwrap(),
        SynchronizationResult::Complete
    ));
    assert_eq!(
        direct_engine.state().system_id(),
        Some("fictional-direct-system")
    );

    let challenged = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HISYN:3:4:2+fictional-continued-system",
            "HITAN:4:6:5+4++fictional-reference+Use a fictional TAN",
        ],
        "continued-dialog",
        1,
    );
    let mut continued_engine = engine_with_method(TanProcess::ProcessVariantTwo);
    continued_engine.state.system_id = None;
    continued_engine
        .synchronization_request(now().date(), now().time())
        .unwrap();
    let pending = match continued_engine
        .accept_synchronization(&challenged, now())
        .unwrap()
    {
        SynchronizationResult::Challenge(pending) => *pending,
        SynchronizationResult::Complete => panic!("expected synchronization challenge"),
    };
    continued_engine
        .tan_submission_request(&pending, &Tan::new("123456").unwrap(), now())
        .unwrap();
    let completion = response(&["HIRMG:2:2+0010::accepted"], "continued-dialog", 2);

    assert!(matches!(
        continued_engine
            .accept_synchronization_continuation(&completion, pending)
            .unwrap(),
        SynchronizationResult::Complete
    ));
    assert_eq!(
        continued_engine.state().system_id(),
        Some("fictional-continued-system")
    );
}

// FinTS 3.0 PIN/TAN 2020-07-10, B.4.3.1.3 and the HKTAN 6/7 Data
// Dictionary entries, with correction T33: first-use medium discovery is
// process 4 with Segmentkennung HKTAB and a meaning-neutral medium filler.
// This is an independently asserted wire shape, not an encoder round trip.
#[test]
fn tan_medium_discovery_uses_the_special_hktan_six_and_seven_shapes() {
    for (process, version) in [
        (TanProcess::ProcessVariantTwo, 6),
        (TanProcess::Decoupled, 7),
    ] {
        let mut engine = engine_with_method(process);
        engine.state.tan_methods[0].medium_name_required = true;

        let request = engine
            .tan_media_initialization_request(now().date(), now().time())
            .unwrap();
        let parsed = crate::wire::Message::parse(&request).unwrap();
        let payload = parsed.payload_segments().unwrap();
        let hktan = payload
            .iter()
            .find(|segment| segment.header().unwrap().code == b"HKTAN")
            .unwrap();

        assert_eq!(hktan.header().unwrap().version, version);
        assert_eq!(
            crate::wire::encode_segments(std::slice::from_ref(hktan)).unwrap(),
            format!("HKTAN:5:{version}+4+HKTAB+++++++++noref'").as_bytes()
        );
        assert!(
            payload
                .iter()
                .all(|segment| segment.header().unwrap().code != b"HKTAB")
        );
    }
}

// PIN/TAN B.4.3.1.3 requires HITAB during the first media-discovery dialog.
// If the initialization response does not carry it, archived E.2.1.2/E.2.1.4
// and current C.3.1.1 define the versioned HKTAB order. An order response that
// still omits HITAB and an explicitly empty list remain distinct failures.
#[test]
fn required_tan_medium_rejects_missing_and_empty_hitab_without_closing_the_dialog() {
    let mut missing = engine_with_method(TanProcess::ProcessVariantTwo);
    missing.state.tan_methods[0].medium_name_required = true;
    missing
        .tan_media_initialization_request(now().date(), now().time())
        .unwrap();
    let missing_response = response(&["HIRMG:2:2+0010::accepted"], "missing-media", 1);
    assert!(matches!(
        missing
            .accept_tan_media_initialization(&missing_response)
            .unwrap(),
        super::initialization::TanMediaInitializationResult::OrderRequired
    ));
    missing
        .tan_media_request(now().date(), now().time())
        .unwrap();
    let missing_order = response(
        &["HIRMG:2:2+0010::accepted", "HIRMS:3:2:3+0020::processed"],
        "missing-media",
        2,
    );
    assert!(matches!(
        missing.accept_tan_media_response(&missing_order),
        Err(Error::MissingValue {
            field: "HITAB TAN media response"
        })
    ));
    assert!(missing.has_active_dialog());

    let mut empty = engine_with_method(TanProcess::Decoupled);
    empty.state.tan_methods[0].medium_name_required = true;
    empty
        .tan_media_initialization_request(now().date(), now().time())
        .unwrap();
    let empty_response = response(
        &["HIRMG:2:2+0010::accepted", "HITAB:3:5:5+1"],
        "empty-media",
        1,
    );
    assert!(matches!(
        empty.accept_tan_media_initialization(&empty_response),
        Err(Error::Unsupported(Limitation::TanMediumUnavailable))
    ));
    assert!(empty.has_active_dialog());

    let mut unusable = engine_with_method(TanProcess::Decoupled);
    unusable.state.tan_methods[0].medium_name_required = true;
    unusable
        .tan_media_initialization_request(now().date(), now().time())
        .unwrap();
    let unusable_response = response(
        &["HIRMG:2:2+0010::accepted", "HITAB:3:5:5+1+M:1"],
        "unusable-media",
        1,
    );
    assert!(matches!(
        unusable.accept_tan_media_initialization(&unusable_response),
        Err(Error::Unsupported(Limitation::TanMediumUnavailable))
    ));
    assert!(unusable.has_active_dialog());
}

// PIN/TAN B.5.1/B.5.2: an institute HITAN answering a process-4 HKTAN must
// itself carry process 4, including when its reference is the no-SCA filler.
// A different process never changes the missing-HITAB result into a continuation.
#[test]
fn tan_medium_discovery_rejects_mismatched_dummy_hitan_process() {
    for (process, version) in [
        (TanProcess::ProcessVariantTwo, 6),
        (TanProcess::Decoupled, 7),
    ] {
        let mut engine = engine_with_method(process);
        engine.state.tan_methods[0].medium_name_required = true;
        engine
            .tan_media_initialization_request(now().date(), now().time())
            .unwrap();
        let hitan = format!("HITAN:3:{version}:5+2++noref+nochallenge");
        let response = response(
            &["HIRMG:2:2+0010::accepted", &hitan],
            "mismatched-media-process",
            1,
        );

        assert!(matches!(
            engine.accept_tan_media_initialization(&response),
            Err(Error::InvalidValue {
                field: "HITAN process"
            })
        ));
        assert!(engine.has_active_dialog());
    }
}

// Formals B.7.5 and PIN/TAN C.3.1.1: message responses, the HITAB data
// segment, and segment responses retain wire order while the media parser
// consumes only HITAB. Institution-authored text remains absent from Debug.
#[test]
fn populated_hitab_preserves_ordered_bank_responses_and_redaction() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.tan_methods[0].medium_name_required = true;
    engine
        .tan_media_initialization_request(now().date(), now().time())
        .unwrap();
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
        "",
        "",
        "",
        "Fictional phone",
    ]
    .join(":");
    let hitab = format!("HITAB:3:5:5+1+{medium}");
    let response = response(
        &[
            "HIRMG:2:2+0010::fictional accepted+1010::fictional notice",
            &hitab,
            "HIRMS:4:2:5+0020::fictional HKTAN processed",
        ],
        "ordered-media",
        1,
    );

    assert!(matches!(
        engine.accept_tan_media_initialization(&response).unwrap(),
        super::initialization::TanMediaInitializationResult::Complete
    ));
    assert_eq!(engine.tan_media()[0].name(), Some("Fictional phone"));
    assert_eq!(
        engine
            .last_responses()
            .iter()
            .map(BankResponse::code)
            .collect::<Vec<_>>(),
        [10, 1010, 20]
    );
    let rendered = format!("{:?}", engine.last_responses());
    assert!(!rendered.contains("fictional accepted"));
    assert!(!rendered.contains("fictional notice"));
    assert!(!rendered.contains("Fictional phone"));
}

// FinTS 3.0 Formals 2017-10-06, C.5.3: an accepted dialog remains open
// until HKEND even when the requested initialization result is unusable locally.
#[test]
fn accepted_unusable_initializations_can_still_be_terminated() {
    let mut media_engine = engine_with_method(TanProcess::ProcessVariantTwo);
    media_engine
        .tan_media_initialization_request(now().date(), now().time())
        .unwrap();
    let unexpected_challenge = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HITAN:3:6:4+4++fictional-reference+Unexpected challenge",
        ],
        "media-dialog",
        1,
    );
    assert!(matches!(
        media_engine.accept_tan_media_initialization(&unexpected_challenge),
        Err(Error::Unsupported(Limitation::TanMedium))
    ));
    media_engine
        .termination_request(now().date(), now().time())
        .unwrap();
    let terminated = response(&["HIRMG:2:2+0100::terminated"], "media-dialog", 2);
    media_engine.accept_termination(&terminated).unwrap();

    let mut discovery_engine = engine_with_method(TanProcess::ProcessVariantTwo);
    discovery_engine.state.selected_tan_method = None;
    discovery_engine
        .initialization_request(now().date(), now().time())
        .unwrap();
    let missing_selection = response(&["HIRMG:2:2+0010::accepted"], "discovery-dialog", 1);
    assert!(matches!(
        discovery_engine.accept_initialization(&missing_selection, now()),
        Err(Error::MissingValue {
            field: "3920 TAN method response"
        })
    ));
    discovery_engine
        .termination_request(now().date(), now().time())
        .unwrap();
    let terminated = response(&["HIRMG:2:2+0100::terminated"], "discovery-dialog", 2);
    discovery_engine.accept_termination(&terminated).unwrap();
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
                "HIUPD:4:6:3+123456::280:12345678+DE40123456780000123456",
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

// FinTS 3.0 Formals 2017-10-06, C.5: dialog identifiers scope every
// continuation. A process-memory continuation cannot cross a terminated dialog.
#[test]
fn stale_synchronization_continuation_cannot_cross_dialogs_or_persist_state() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.system_id = None;
    engine
        .synchronization_request(now().date(), now().time())
        .unwrap();
    let challenged = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HISYN:3:4:2+stale-fictional-system",
            "HITAN:4:6:5+4++stale-fictional-reference+Use a fictional TAN",
        ],
        "old-dialog",
        1,
    );
    let pending = match engine.accept_synchronization(&challenged, now()).unwrap() {
        SynchronizationResult::Challenge(pending) => *pending,
        SynchronizationResult::Complete => panic!("expected synchronization challenge"),
    };

    engine
        .termination_request(now().date(), now().time())
        .unwrap();
    let terminated = response(&["HIRMG:2:2+0100::terminated"], "old-dialog", 2);
    engine.accept_termination(&terminated).unwrap();
    engine
        .initialization_request(now().date(), now().time())
        .unwrap();
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "new-dialog", 1);
    assert!(matches!(
        engine.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));

    assert!(matches!(
        engine.tan_submission_request(&pending, &Tan::new("123456").unwrap(), now()),
        Err(Error::StaleContinuation)
    ));
    assert_ne!(engine.state().system_id(), Some("stale-fictional-system"));
}

// Formals B.7.6: the fixed unsigned HNHBK+HIRMG+HNHBS abort may use
// "unbekannt" and/or message 9999 when the institute cannot reference the
// active dialog. Central mid-dialog acceptance must preserve its class-9 code.
#[test]
fn fixed_dialog_abort_surfaces_bank_error_without_loosening_mismatch_checks() {
    let connected = || {
        let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
        let initialization = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
        engine
            .accept_initialization(&initialization, now())
            .unwrap();
        engine
    };

    for (dialog_id, message_number) in [("unbekannt", 2), ("dialog1", 9999)] {
        let mut engine = connected();
        let abort = response(
            &["HIRMG:2:2+9800::fictional abort"],
            dialog_id,
            message_number,
        );
        assert!(matches!(
            engine.accept_termination(&abort),
            Err(Error::Bank(bank)) if bank.code() == 9800
        ));
        assert!(!engine.has_active_dialog());
        assert_eq!(engine.last_responses()[0].code(), 9800);
    }

    for abort in [
        response(&["HIRMG:2:2+3900::warning only"], "unbekannt", 2),
        response(
            &[
                "HIRMG:2:2+9800::fictional abort",
                "HIRMS:3:2:4+9951::extra segment",
            ],
            "unbekannt",
            2,
        ),
        response(&["HIRMG:2:2+9800::wrong dialog"], "other-dialog", 2),
    ] {
        let mut engine = connected();
        assert!(matches!(
            engine.accept_termination(&abort),
            Err(Error::InconsistentState)
        ));
        assert!(engine.has_active_dialog());
    }
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
    let mut pending = pending_challenge(
        challenge,
        PendingOperation::Initialization,
        method,
        "dialog1",
        now(),
    )
    .unwrap();

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
    engine.state.advertised_balance_versions = vec![9, 8, 6];
    engine.state.balance_capability_advertised = true;
    engine.state.balance_requires_tan = Some(false);
    engine.state.accounts.push(Account {
        iban: None,
        bic: None,
        account_number: Some("123456".to_owned()),
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
        unlisted_operations_unknown: false,
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
        engine
            .state()
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [9, 8, 6]
    );
    assert!(
        engine
            .state()
            .advertised_capabilities()
            .balance()
            .supports_version(8)
    );
    assert!(
        !engine
            .state()
            .advertised_capabilities()
            .balance()
            .supports_version(9)
    );
    assert_eq!(
        hksal.elements()[1].components()[2].as_text().unwrap(),
        "123456"
    );

    let mismatched = response(
        &[
            "HIRMG:2:2+0010::accepted",
            concat!(
                "HISAL:3:8:4+DE21123456780000999999:FICTDEFFXXX::::",
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

// Formals E.2 "UPD-Verwendung": an omitted Erlaubte-GV entry is locally
// denied only for usage 0. Usage 1 permits the concrete request and leaves
// the final authorization decision to the institute.
#[test]
fn upd_usage_one_allows_unknown_cash_and_product_operations() {
    let connected = |mut engine: Engine| {
        let initialization = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
        assert!(matches!(
            engine
                .accept_initialization(&initialization, now())
                .unwrap(),
            InitializationResult::Connected
        ));
        engine
    };
    let account = |account_type, unknown| {
        let mut account = transaction_account("DE40123456780000123456", "123456", &[]);
        account.account_type = Some(account_type);
        account.unlisted_operations_unknown = unknown;
        account
    };

    let mut balance = engine_with_method(TanProcess::ProcessVariantTwo);
    balance.state.balance_versions = vec![8];
    balance.state.balance_capability_advertised = true;
    balance.state.balance_requires_tan = Some(false);
    balance.state.accounts.push(account(1, true));
    let mut balance = connected(balance);
    let wire = balance
        .balance_request(0, now().date(), now().time())
        .unwrap();
    assert!(
        crate::wire::Message::parse(&wire)
            .unwrap()
            .payload_segments()
            .unwrap()
            .iter()
            .any(|segment| segment.header().unwrap().code == b"HKSAL")
    );

    let mut transactions = engine_with_method(TanProcess::ProcessVariantTwo);
    transactions.state.transaction_capability_advertised = true;
    transactions.state.camt_capability = Some(CamtCapability {
        descriptor: "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08".to_owned(),
    });
    transactions.state.camt_requires_tan = Some(false);
    transactions.state.accounts.push(account(1, true));
    let mut transactions = connected(transactions);
    assert!(
        transactions
            .transaction_request(0, None, None, now().date(), now().time())
            .is_ok()
    );

    let mut depot = engine_with_method(TanProcess::ProcessVariantTwo);
    depot.state.depot_positions_advertised = true;
    depot.state.depot_position_versions = vec![6];
    depot.state.depot_positions_requires_tan = Some(false);
    depot.state.accounts.push(account(30, true));
    let mut depot = connected(depot);
    assert!(
        depot
            .depot_positions_request(0, now().date(), now().time())
            .is_ok()
    );

    for operation in ["balance", "transactions", "depot"] {
        let mut denied = engine_with_method(TanProcess::ProcessVariantTwo);
        denied.state.balance_versions = vec![8];
        denied.state.balance_capability_advertised = true;
        denied.state.balance_requires_tan = Some(false);
        denied.state.transaction_capability_advertised = true;
        denied.state.camt_capability = Some(CamtCapability {
            descriptor: "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08".to_owned(),
        });
        denied.state.camt_requires_tan = Some(false);
        denied.state.depot_positions_advertised = true;
        denied.state.depot_position_versions = vec![6];
        denied.state.depot_positions_requires_tan = Some(false);
        denied
            .state
            .accounts
            .push(account(if operation == "depot" { 30 } else { 1 }, false));
        let mut denied = connected(denied);
        let result = match operation {
            "balance" => denied.balance_request(0, now().date(), now().time()),
            "transactions" => denied.transaction_request(0, None, None, now().date(), now().time()),
            "depot" => denied.depot_positions_request(0, now().date(), now().time()),
            _ => unreachable!(),
        };
        assert!(matches!(result, Err(Error::Unsupported(_))));
    }
}

// Formals E.2/E.3 and DD "Erlaubte Geschäftsvorfälle": the per-account
// operation entry authorizes the request. DD "Kontoart" only classifies an
// account; correction P24 changes occupancy for PSD2 identification, not
// authorization. Explicit permission therefore wins when the type is absent or
// contradicts that descriptive classification.
#[test]
fn product_operations_use_upd_permission_not_descriptive_account_type() {
    let connect = |mut engine: Engine| {
        let initialization = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
        assert!(matches!(
            engine
                .accept_initialization(&initialization, now())
                .unwrap(),
            InitializationResult::Connected
        ));
        engine
    };
    let has_segment = |wire: &[u8], code: &[u8]| {
        crate::wire::Message::parse(wire)
            .unwrap()
            .payload_segments()
            .unwrap()
            .iter()
            .any(|segment| segment.header().unwrap().code == code)
    };

    for account_type in [None, Some(30)] {
        let mut account = transaction_account("DE40123456780000123456", "300001", &[("HKWPD", 1)]);
        account.account_type = account_type;
        let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
        engine.state.accounts = vec![account];
        engine.state.depot_positions_advertised = true;
        engine.state.depot_position_versions = vec![5];
        engine.state.depot_positions_requires_tan = Some(false);
        let mut engine = connect(engine);
        let wire = engine
            .depot_positions_request(0, now().date(), now().time())
            .unwrap();
        assert!(has_segment(&wire, b"HKWPD"));
    }

    let mut account = transaction_account("DE40123456780000123456", "300001", &[("HKWDU", 1)]);
    account.account_type = Some(50);
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = vec![account];
    engine.state.securities_transactions_advertised = true;
    engine.state.securities_transactions_supported = true;
    engine.state.securities_transactions_requires_tan = Some(false);
    let mut engine = connect(engine);
    let wire = engine
        .securities_transactions_request(0, None, None, now().date(), now().time())
        .unwrap();
    assert!(has_segment(&wire, b"HKWDU"));

    let mut account = transaction_account(
        "DE40123456780000123456",
        "444433******1111",
        &[("HKKKU", 1)],
    );
    account.account_type = None;
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = vec![account];
    engine.state.credit_card_transactions_advertised = true;
    engine.state.credit_card_transactions = Some(crate::model::CreditCardCapability {
        account_required: false,
        date_range_allowed: true,
    });
    engine.state.credit_card_transactions_requires_tan = Some(false);
    let mut engine = connect(engine);
    let wire = engine
        .credit_card_transactions_request(0, None, None, now().date(), now().time())
        .unwrap();
    assert!(has_segment(&wire, b"HKKKU"));

    let mut account = transaction_account(
        "DE40123456780000123456",
        "444433******1111",
        &[("HKKKS", 1)],
    );
    account.account_type = Some(30);
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = vec![account];
    engine.state.credit_card_balance_advertised = true;
    engine.state.credit_card_balance_account_required = Some(false);
    engine.state.credit_card_balance_requires_tan = Some(false);
    let mut engine = connect(engine);
    let wire = engine
        .credit_card_balance_request(0, now().date(), now().time())
        .unwrap();
    assert!(has_segment(&wire, b"HKKKS"));
}

#[test]
fn product_operations_without_upd_permission_remain_unauthorized() {
    let connect = |mut engine: Engine| {
        let initialization = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
        assert!(matches!(
            engine
                .accept_initialization(&initialization, now())
                .unwrap(),
            InitializationResult::Connected
        ));
        engine
    };
    let account = |account_type| {
        let mut account = transaction_account("DE40123456780000123456", "300001", &[]);
        account.account_type = account_type;
        account
    };

    let mut depot = engine_with_method(TanProcess::ProcessVariantTwo);
    depot.state.accounts = vec![account(Some(30))];
    depot.state.depot_positions_advertised = true;
    depot.state.depot_position_versions = vec![5];
    depot.state.depot_positions_requires_tan = Some(false);
    assert!(matches!(
        connect(depot).depot_positions_request(0, now().date(), now().time()),
        Err(Error::Unsupported(Limitation::DepotPositionsNotAuthorized))
    ));

    let mut card = engine_with_method(TanProcess::ProcessVariantTwo);
    card.state.accounts = vec![account(Some(50))];
    card.state.credit_card_transactions_advertised = true;
    card.state.credit_card_transactions = Some(crate::model::CreditCardCapability {
        account_required: false,
        date_range_allowed: true,
    });
    card.state.credit_card_transactions_requires_tan = Some(false);
    assert!(matches!(
        connect(card).credit_card_transactions_request(0, None, None, now().date(), now().time()),
        Err(Error::Unsupported(
            Limitation::CreditCardTransactionsNotAuthorized
        ))
    ));
}

// FinTS Messages 2022 C.2.1.2.1-C.2.1.2.3: every implemented HISALS
// advertisement has a corresponding HKSAL request version and cannot degrade
// into the unsupported-version limitation.
#[test]
fn supported_balance_advertisements_select_each_implemented_version() {
    for version in 5..=8 {
        let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
        engine.state.balance_versions = vec![version];
        engine.state.advertised_balance_versions = vec![version];
        engine.state.balance_capability_advertised = true;
        engine.state.balance_requires_tan = Some(false);
        engine.state.accounts.push(transaction_account(
            "DE40123456780000123456",
            "123456",
            &[("HKSAL", 1)],
        ));
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
        let payload = crate::wire::Message::parse(&request)
            .unwrap()
            .payload_segments()
            .unwrap();
        assert_eq!(
            payload
                .iter()
                .find(|segment| segment.header().unwrap().code == b"HKSAL")
                .unwrap()
                .header()
                .unwrap()
                .version,
            version
        );
    }
}

// HBCI 2.2 VII.2.2 supplies the HKSAL/HISAL 5 business layout; FinTS
// PIN/TAN T31 and B.8.1 keep TAN applicability advertisement-derived.
#[test]
fn legacy_balance_five_uses_existing_tan_paths_and_strict_account_binding() {
    for requires_tan in [false, true] {
        let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
        engine.state.advertised_balance_versions = vec![5];
        engine.state.balance_versions = vec![5];
        engine.state.balance_capability_advertised = true;
        engine.state.balance_requires_tan = Some(requires_tan);
        engine.state.accounts.push(transaction_account(
            "DE40123456780000123456",
            "123456",
            &[("HKSAL", 1)],
        ));
        let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
        assert!(matches!(
            engine.accept_initialization(&initialized, now()).unwrap(),
            InitializationResult::Connected
        ));

        let request = engine
            .balance_request(0, now().date(), now().time())
            .unwrap();
        let payload = crate::wire::Message::parse(&request)
            .unwrap()
            .payload_segments()
            .unwrap();
        assert_eq!(
            payload
                .iter()
                .find(|segment| segment.header().unwrap().code == b"HKSAL")
                .unwrap()
                .header()
                .unwrap()
                .version,
            5
        );
        assert_eq!(
            payload
                .iter()
                .any(|segment| segment.header().unwrap().code == b"HKTAN"),
            requires_tan
        );

        if !requires_tan {
            let complete = response(
                &[
                    "HIRMG:2:2+0010::accepted",
                    concat!(
                        "HISAL:3:5:4+123456::280:12345678",
                        "+Fictional checking+EUR+C:10,:EUR:20260729"
                    ),
                ],
                "dialog1",
                2,
            );
            assert!(matches!(
                engine.accept_balance(&complete, now()).unwrap(),
                BalanceResult::Complete(_)
            ));
        }
    }

    let mut mismatched = engine_with_method(TanProcess::ProcessVariantTwo);
    mismatched.state.advertised_balance_versions = vec![5];
    mismatched.state.balance_versions = vec![5];
    mismatched.state.balance_capability_advertised = true;
    mismatched.state.balance_requires_tan = Some(false);
    mismatched.state.accounts.push(transaction_account(
        "DE40123456780000123456",
        "123456",
        &[("HKSAL", 1)],
    ));
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog2", 1);
    mismatched
        .accept_initialization(&initialized, now())
        .unwrap();
    mismatched
        .balance_request(0, now().date(), now().time())
        .unwrap();
    let wrong_account = response(
        &[
            "HIRMG:2:2+0010::accepted",
            concat!(
                "HISAL:3:5:4+123456::280:87654321",
                "+Fictional checking+EUR+C:10,:EUR:20260729"
            ),
        ],
        "dialog2",
        2,
    );
    let error = match mismatched.accept_balance(&wrong_account, now()) {
        Err(error) => error,
        Ok(_) => panic!("mismatched HISAL 5 account was accepted"),
    };
    assert!(matches!(error, Error::InconsistentState));
    assert!(!format!("{error:?} {error}").contains("87654321"));
}

// FinTS Formals 2017-10-06, D.2 and HIBPA 3: the BPD institute identity
// identifies the responding institute. Parameters from a wrong endpoint must not
// replace reusable state for the configured institute.
#[test]
fn gate3_wrong_endpoint_parameters_are_rejected_before_state_mutation() {
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    let mismatched = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HIBPA:3:3:3+41+280:87654321+Different Fictional Bank+9+1+300",
            "HISALS:4:8:3+1+1+0+N",
        ],
        "dialog1",
        1,
    );

    assert!(matches!(
        engine.accept_initialization(&mismatched, now()),
        Err(Error::Unsupported(Limitation::InstituteMismatch))
    ));
    assert_eq!(engine.state().bpd_version(), 0);
    assert!(engine.state.balance_versions.is_empty());
    assert!(
        engine
            .state()
            .advertised_capabilities()
            .balance()
            .advertised_versions()
            .is_empty()
    );
}

// FinTS Formals D and Messages C.2.1.2: an advertised but unsupported HISALS
// version differs from a completely absent balance capability.
#[test]
fn gate3_balance_capability_errors_distinguish_version_from_absence() {
    let account = transaction_account("DE40123456780000123456", "123456", &[("HKSAL", 1)]);
    let mut unsupported = engine_with_method(TanProcess::ProcessVariantTwo);
    unsupported.state.accounts.push(account.clone());
    unsupported.state.balance_capability_advertised = true;
    unsupported.state.advertised_balance_versions = vec![4];
    unsupported.state.balance_requires_tan = Some(false);
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        unsupported
            .accept_initialization(&initialized, now())
            .unwrap(),
        InitializationResult::Connected
    ));
    let error = unsupported
        .balance_request(0, now().date(), now().time())
        .unwrap_err();
    assert!(matches!(
        &error,
        Error::Unsupported(Limitation::BalanceVersion)
    ));
    assert_eq!(
        unsupported
            .state()
            .advertised_capabilities()
            .balance()
            .advertised_versions(),
        [4]
    );
    assert!(
        !unsupported
            .state()
            .advertised_capabilities()
            .balance()
            .supports_version(4)
    );
    assert!(!error.to_string().contains('4'));

    let mut absent = engine_with_method(TanProcess::ProcessVariantTwo);
    absent.state.accounts.push(account);
    absent.state.balance_requires_tan = Some(false);
    assert!(matches!(
        absent.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));
    assert!(matches!(
        absent.balance_request(0, now().date(), now().time()),
        Err(Error::Unsupported(Limitation::BalanceNotAdvertised))
    ));
    assert!(
        absent
            .state()
            .advertised_capabilities()
            .balance()
            .advertised_versions()
            .is_empty()
    );
}

// FinTS Messages 2022-04-15, C.2.1.1.1: HKKAZ 6 requires the national
// account group. An IBAN-only UPD account cannot be silently coerced.
#[test]
fn gate3_legacy_transaction_account_mismatch_reports_transaction_limitation() {
    let mut account = transaction_account("DE40123456780000123456", "123456", &[("HKKAZ", 1)]);
    account.account_number = None;
    let mut engine = connected_transaction_engine(vec![account], false, vec![6], None, Some(false));

    assert!(matches!(
        engine.transaction_request(0, None, None, now().date(), now().time()),
        Err(Error::Unsupported(Limitation::TransactionsVersion))
    ));
}

// FinTS Messages 2022-04-15, C.2.3.1.1.1 and C.2.1.1.1.1-.2:
// prefer the advertised camt operation and keep the legacy operation as fallback.
#[test]
fn transaction_format_selection_prefers_camt_without_guessing_tan_status() {
    let account = transaction_account(
        "DE40123456780000123456",
        "123456",
        &[("HKCAZ", 1), ("HKKAZ", 1)],
    );
    let mut engine =
        connected_transaction_engine(vec![account], true, vec![7], Some(false), Some(true));

    let request = engine
        .transaction_request(
            0,
            NaiveDate::from_ymd_opt(2026, 7, 1),
            Some(now().date()),
            now().date(),
            now().time(),
        )
        .unwrap();
    let payload = crate::wire::Message::parse(&request)
        .unwrap()
        .payload_segments()
        .unwrap();

    assert!(
        payload
            .iter()
            .any(|segment| segment.header().unwrap().code == b"HKCAZ")
    );
    assert!(
        payload
            .iter()
            .all(|segment| segment.header().unwrap().code != b"HKKAZ")
    );
    assert!(
        payload
            .iter()
            .all(|segment| segment.header().unwrap().code != b"HKTAN")
    );

    let account = transaction_account(
        "DE40123456780000123456",
        "123456",
        &[("HKCAZ", 1), ("HKKAZ", 1)],
    );
    let fallback = connected_transaction_engine(vec![account], true, vec![7], None, Some(false));
    assert!(matches!(
        fallback.transaction_format(&fallback.state.accounts[0]),
        Ok((TransactionFormat::Mt940 { version: 7 }, false))
    ));
}

// FinTS Formals 2017-10-06, B.6 and Messages 2022-04-15,
// C.2.1.1.1.1-.2. Pagination repeats the same order in one dialog with the
// returned Aufsetzpunkt, and entries remain attached to the requested UPD account.
#[test]
fn booked_transactions_exhaust_pagination_and_keep_accounts_distinct() {
    let first_account = transaction_account("DE40123456780000123456", "123456", &[("HKKAZ", 1)]);
    let second_account = transaction_account("DE21123456780000654321", "654321", &[("HKKAZ", 1)]);
    let mut engine = connected_transaction_engine(
        vec![first_account, second_account],
        false,
        vec![7],
        None,
        Some(false),
    );

    engine
        .transaction_request(1, None, None, now().date(), now().time())
        .unwrap();
    let first_page = binary_response(
        "HIRMS:3:2:3+3040::more:fictional-next'HIKAZ:4:7:3+@",
        &mt940_page(1, "FICTBANKREF1", "654321"),
        "'HNHBS:5:1+2'",
        "dialog1",
        2,
    );
    assert!(matches!(
        engine.accept_transactions(&first_page, now()).unwrap(),
        TransactionsResult::Continue
    ));

    let continuation = engine
        .next_transaction_page_request(now().date(), now().time())
        .unwrap();
    let payload = crate::wire::Message::parse(&continuation)
        .unwrap()
        .payload_segments()
        .unwrap();
    let hkkaz = payload
        .iter()
        .find(|segment| segment.header().unwrap().code == b"HKKAZ")
        .unwrap();
    assert_eq!(
        hkkaz.elements().last().unwrap().components()[0]
            .as_text()
            .unwrap(),
        "fictional-next"
    );

    let second_page = binary_response(
        "HIKAZ:3:7:3+@",
        &mt940_page(2, "FICTBANKREF2", "654321"),
        "'HNHBS:4:1+3'",
        "dialog1",
        3,
    );
    let result = match engine.accept_transactions(&second_page, now()).unwrap() {
        TransactionsResult::Complete(result) => result,
        _ => panic!("expected complete paginated transaction result"),
    };

    assert_eq!(result.account().iban(), Some("DE21123456780000654321"));
    assert_eq!(result.entries().len(), 2);
    assert_eq!(
        result.entries()[0].account_servicer_reference(),
        Some("FICTBANKREF1")
    );
    assert_eq!(
        result.entries()[1].account_servicer_reference(),
        Some("FICTBANKREF2")
    );
}

// FinTS Formals 2017-10-06, B.6 plus the repository's bounded-pagination
// contract: repeated points and excessive page counts fail before another replay.
#[test]
fn transaction_pagination_rejects_repeated_points_and_page_overflow() {
    let account = transaction_account("DE40123456780000123456", "123456", &[("HKKAZ", 1)]);
    let mut repeated =
        connected_transaction_engine(vec![account.clone()], false, vec![7], None, Some(false));
    repeated
        .transaction_request(0, None, None, now().date(), now().time())
        .unwrap();
    let first = binary_response(
        "HIRMS:3:2:3+3040::more:fictional-repeat'HIKAZ:4:7:3+@",
        &mt940_page(1, "FICTBANKREF1", "123456"),
        "'HNHBS:5:1+2'",
        "dialog1",
        2,
    );
    repeated.accept_transactions(&first, now()).unwrap();
    repeated
        .next_transaction_page_request(now().date(), now().time())
        .unwrap();
    let second = binary_response(
        "HIRMS:3:2:3+3040::more:fictional-repeat'HIKAZ:4:7:3+@",
        &mt940_page(2, "FICTBANKREF2", "123456"),
        "'HNHBS:5:1+3'",
        "dialog1",
        3,
    );
    assert!(matches!(
        repeated.accept_transactions(&second, now()),
        Err(Error::RepeatedContinuationPoint)
    ));
    assert!(
        repeated
            .transaction_request(0, None, None, now().date(), now().time())
            .is_ok()
    );

    let mut bounded =
        connected_transaction_engine(vec![account], false, vec![7], None, Some(false));
    bounded
        .transaction_request(0, None, None, now().date(), now().time())
        .unwrap();
    bounded.accept_transactions(&first, now()).unwrap();
    bounded.transaction.as_mut().unwrap().pages_requested = LOCAL_TRANSACTION_PAGE_LIMIT;
    assert!(matches!(
        bounded.next_transaction_page_request(now().date(), now().time()),
        Err(Error::PaginationLimitReached)
    ));
    assert!(
        bounded
            .transaction_request(0, None, None, now().date(), now().time())
            .is_ok()
    );
}

// FinTS Messages 2022-04-15, C.2.1.1.2 and C.2.3.1.2: an accepted
// transaction response without the booked response segment is malformed.
// A terminal pagination error releases only the current operation, not the dialog.
#[test]
fn missing_transaction_page_does_not_wedge_the_dialog() {
    let account = transaction_account("DE40123456780000123456", "123456", &[("HKKAZ", 1)]);
    let mut engine = connected_transaction_engine(vec![account], false, vec![7], None, Some(false));
    engine
        .transaction_request(0, None, None, now().date(), now().time())
        .unwrap();

    let missing = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 2);
    assert!(matches!(
        engine.accept_transactions(&missing, now()),
        Err(Error::MissingValue {
            field: "booked transaction response"
        })
    ));
    assert!(
        engine
            .transaction_request(0, None, None, now().date(), now().time())
            .is_ok()
    );
}

// FinTS Messages 2022 C.4.3.1 and Formals B.6: the MT535 page indicator
// and FinTS Aufsetzpunkt must agree, repeated points terminate only the
// current request, and a fresh same-dialog request remains possible.
#[test]
fn depot_positions_reject_repeated_continuations_without_wedging_dialog() {
    let mut account = transaction_account("DE40123456780000123456", "300001", &[("HKWPD", 1)]);
    account.account_type = Some(30);
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = vec![account];
    engine.state.depot_positions_advertised = true;
    engine.state.depot_position_versions = vec![6];
    engine.state.depot_positions_requires_tan = Some(false);
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        engine.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));

    engine
        .depot_positions_request(0, now().date(), now().time())
        .unwrap();
    assert!(matches!(
        engine.next_depot_positions_page_request(now().date(), now().time()),
        Err(Error::InconsistentState)
    ));
    engine.continuation_active = true;
    assert!(matches!(
        engine.next_depot_positions_page_request(now().date(), now().time()),
        Err(Error::InconsistentState)
    ));
    engine.continuation_active = false;
    let first = binary_response(
        "HIRMS:3:2:3+3040::more:position-next'HIWPD:4:6:3+@",
        &mt535_page(1, "MORE"),
        "'HNHBS:5:1+2'",
        "dialog1",
        2,
    );
    assert!(matches!(
        engine.accept_depot_positions(&first, now()).unwrap(),
        DepotPositionsResult::Continue
    ));
    #[cfg(feature = "diagnostics")]
    {
        let facts = engine.development_depot_response().unwrap();
        assert_eq!(facts.document_kind(), crate::DepotDocumentKind::Mt535);
        assert_eq!(facts.positions().len(), 1);
        assert_eq!(
            facts
                .block_inventory()
                .iter()
                .map(|fact| fact.kind())
                .collect::<Vec<_>>(),
            [
                crate::DepotBlockKind::General,
                crate::DepotBlockKind::FinancialInstrument,
                crate::DepotBlockKind::SubBalance,
            ]
        );
    }
    engine
        .next_depot_positions_page_request(now().date(), now().time())
        .unwrap();

    let repeated = binary_response(
        "HIRMS:3:2:3+3040::more:position-next'HIWPD:4:6:3+@",
        &mt535_page(2, "MORE"),
        "'HNHBS:5:1+3'",
        "dialog1",
        3,
    );
    assert!(matches!(
        engine.accept_depot_positions(&repeated, now()),
        Err(Error::RepeatedContinuationPoint)
    ));
    assert!(
        engine
            .depot_positions_request(0, now().date(), now().time())
            .is_ok()
    );
}

// HBCI 2.2 VII.4.3.1/IX.2.4 and Formals B.6: a version-5-only BPD emits
// HKWPD 5 without HKTAN when HIPINS advertises TAN-free execution, accepts
// only HIWPD 5 MT535 pages, and preserves that version across pagination.
#[test]
fn depot_positions_version_five_is_selected_and_paginated_tan_free() {
    let mut account = transaction_account("DE40123456780000123456", "300001", &[("HKWPD", 1)]);
    account.account_type = Some(30);
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = vec![account];
    engine.state.depot_positions_advertised = true;
    engine.state.depot_position_versions = vec![5];
    engine.state.depot_positions_requires_tan = Some(false);
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        engine.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));

    let first_request = engine
        .depot_positions_request(0, now().date(), now().time())
        .unwrap();
    let first_payload = crate::wire::Message::parse(&first_request)
        .unwrap()
        .payload_segments()
        .unwrap();
    assert!(first_payload.iter().any(|segment| {
        let header = segment.header().unwrap();
        header.code == b"HKWPD" && header.version == 5
    }));
    assert!(
        first_payload
            .iter()
            .all(|segment| segment.header().unwrap().code != b"HKTAN")
    );

    let first = binary_response(
        "HIRMS:3:2:3+3040::more:position-next'HIWPD:4:5:3+@",
        &mt535_page(1, "MORE"),
        "'HNHBS:5:1+2'",
        "dialog1",
        2,
    );
    assert!(matches!(
        engine.accept_depot_positions(&first, now()).unwrap(),
        DepotPositionsResult::Continue
    ));
    let next_request = engine
        .next_depot_positions_page_request(now().date(), now().time())
        .unwrap();
    let next_payload = crate::wire::Message::parse(&next_request)
        .unwrap()
        .payload_segments()
        .unwrap();
    let request = next_payload
        .iter()
        .find(|segment| segment.header().unwrap().code == b"HKWPD")
        .unwrap();
    assert_eq!(request.header().unwrap().version, 5);
    assert!(matches!(
        &request.elements().last().unwrap().components()[0],
        crate::wire::Value::Text(value) if value == b"position-next"
    ));

    let terminal = binary_response(
        "HIWPD:3:5:3+@",
        &mt535_page(2, "LAST"),
        "'HNHBS:4:1+3'",
        "dialog1",
        3,
    );
    let result = match engine.accept_depot_positions(&terminal, now()).unwrap() {
        DepotPositionsResult::Complete(result) => result,
        _ => panic!("expected completed version-5 depot positions"),
    };
    assert_eq!(result.positions().len(), 2);
}

// The selected operation version binds the response. A version-5 request must
// not reinterpret HIWPD 6, and identity mismatches stay typed and redacted.
#[test]
fn depot_positions_version_five_rejects_wrong_version_and_identity() {
    let connected = || {
        let mut account = transaction_account("DE40123456780000123456", "300001", &[("HKWPD", 1)]);
        account.account_type = Some(30);
        let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
        engine.state.accounts = vec![account];
        engine.state.depot_positions_advertised = true;
        engine.state.depot_position_versions = vec![5];
        engine.state.depot_positions_requires_tan = Some(false);
        let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
        assert!(matches!(
            engine.accept_initialization(&initialized, now()).unwrap(),
            InitializationResult::Connected
        ));
        engine
    };

    let mut wrong_version = connected();
    wrong_version
        .depot_positions_request(0, now().date(), now().time())
        .unwrap();
    let wrong = binary_response(
        "HIWPD:3:6:3+@",
        &mt535_page(1, "ONLY"),
        "'HNHBS:4:1+2'",
        "dialog1",
        2,
    );
    assert!(matches!(
        wrong_version.accept_depot_positions(&wrong, now()),
        Err(Error::UnsupportedSegment {
            code: "HIWPD",
            version: 6
        })
    ));

    let mut wrong_identity = connected();
    wrong_identity
        .depot_positions_request(0, now().date(), now().time())
        .unwrap();
    let payload = String::from_utf8(mt535_page(1, "ONLY"))
        .unwrap()
        .replace("12345678/300001", "12345678/399999")
        .into_bytes();
    let mismatched = binary_response("HIWPD:3:5:3+@", &payload, "'HNHBS:4:1+2'", "dialog1", 2);
    let error = match wrong_identity.accept_depot_positions(&mismatched, now()) {
        Err(error @ Error::InconsistentState) => error,
        _ => panic!("expected a redacted identity mismatch"),
    };
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("399999"));
    assert!(!rendered.contains("300001"));
    assert!(!rendered.contains("12345678"));
}

#[cfg(feature = "diagnostics")]
fn fictional_mt535_price_failure(
    financial_blocks: &str,
) -> (Box<crate::DepotPositions>, crate::DepotResponseFacts) {
    let mut account = transaction_account("DE40123456780000123456", "300001", &[("HKWPD", 1)]);
    account.account_type = None;
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = vec![account];
    engine.state.depot_positions_advertised = true;
    engine.state.depot_position_versions = vec![5];
    engine.state.depot_positions_requires_tan = Some(false);
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        engine.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));
    engine
        .depot_positions_request(0, now().date(), now().time())
        .unwrap();

    let payload = format!(
        "\r\n:16R:GENL\r\n:28E:1/ONLY\r\n\
         :97A::SAFE//12345678/300001\r\n:17B::ACTI//Y\r\n:16S:GENL\r\n\
         {financial_blocks}-"
    );
    let malformed = binary_response(
        "HIWPD:3:5:3+@",
        payload.as_bytes(),
        "'HNHBS:4:1+2'",
        "dialog1",
        2,
    );
    let result = match engine.accept_depot_positions(&malformed, now()).unwrap() {
        DepotPositionsResult::Complete(result) => result,
        _ => panic!("expected a completed fictional MT535 page"),
    };
    let facts = engine.development_depot_response().unwrap().clone();
    (result, facts)
}

// DK Anlage 3 v3.9 4.3: once the bounded 16R/16S tree is available, later
// field failures must preserve only its value-free structure. The fictional
// price deliberately uses unknown qualifier/unit categories so an attended
// run can distinguish that shape without retaining the price or currency.
#[cfg(feature = "diagnostics")]
#[test]
fn malformed_mt535_price_preserves_redacted_structure_diagnostics() {
    let (result, facts) = fictional_mt535_price_failure(concat!(
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS25\r\n",
        "Fictional Diagnostic Security\r\n",
        ":90B::FICT//ODD1/EUR12,\r\n",
        ":93B::AGGR//UNIT/1,\r\n",
        ":16R:SUBBAL\r\n",
        ":93C::TAVI//UNIT/AVAI/1,\r\n",
        ":16S:SUBBAL\r\n",
        ":16S:FIN\r\n"
    ));
    assert_eq!(result.positions().len(), 1);
    assert!(result.positions()[0].price().is_none());
    assert_eq!(result.parse_counts().degraded(), 1);
    assert_eq!(result.parse_counts().skipped(), 0);
    let formatted = format!("{facts:?}");
    for private_value in ["FICT", "ODD1", "EUR", "12,"] {
        assert!(!formatted.contains(private_value));
    }

    assert_eq!(facts.document_kind(), crate::DepotDocumentKind::Mt535);
    assert_eq!(
        facts
            .block_inventory()
            .iter()
            .map(|fact| (fact.kind(), fact.depth(), fact.occurrence()))
            .collect::<Vec<_>>(),
        [
            (crate::DepotBlockKind::General, 1, 1),
            (crate::DepotBlockKind::FinancialInstrument, 1, 1),
            (crate::DepotBlockKind::SubBalance, 2, 1),
        ]
    );
    assert!(facts.tag_inventory().iter().any(|fact| {
        fact.kind() == crate::DepotTagKind::AmountPrice90B
            && fact.depth() == 1
            && fact.occurrence() == 1
    }));
    assert!(facts.tag_inventory().iter().any(|fact| {
        fact.kind() == crate::DepotTagKind::SubBalance93C
            && fact.depth() == 2
            && fact.occurrence() == 1
    }));
    assert_eq!(facts.positions().len(), 1);
    assert_eq!(facts.position_failures().len(), 1);
    assert_eq!(facts.position_failures()[0].ordinal(), 1);
    assert_eq!(
        facts.position_failures()[0].disposition(),
        crate::DepotPositionFailureDisposition::Degraded
    );
    assert_eq!(
        facts.position_failures()[0].site(),
        crate::DepotPositionFailureSite::Price
    );
    let shape = facts.price_shapes()[0];
    assert_eq!(shape.tag(), crate::DepotPriceTagKind::Amount90B);
    assert_eq!(shape.qualifier(), crate::DepotPriceQualifierKind::Unknown);
    assert_eq!(shape.unit(), crate::DepotPriceUnitKind::Unknown);
    assert!(shape.qualifier_present());
    assert!(shape.unit_present());
    assert!(!shape.currency_prefix_width_available());
    assert_eq!(shape.currency_shape(), None);
    assert!(shape.price_present());
    assert_eq!(facts.first_failing_price_ordinal(), Some(1));
    assert_eq!(
        facts.first_failing_price_stage(),
        Some(crate::DepotPriceFailureStage::Qualifier)
    );
}

// DK Anlage 3 v3.9 4.3 pages 405-407 and HBCI 2.2 IX.2.4 pages 89-91
// independently define each validated MT535 price/timestamp component. These
// fictional failures prove that diagnostics name only the static stage.
#[cfg(feature = "diagnostics")]
#[test]
fn mt535_price_failure_stages_are_static_value_free_and_retained() {
    let cases = [
        (
            ":90B::FICT//ACTU/EUR12,\r\n",
            "",
            crate::DepotPriceFailureStage::Qualifier,
        ),
        (
            ":90B::MRKT//PRCT/12,\r\n",
            "",
            crate::DepotPriceFailureStage::TagUnitPairing,
        ),
        (
            ":90B::MRKT//ACTU/EU112,\r\n",
            "",
            crate::DepotPriceFailureStage::CurrencyShape,
        ),
        (
            ":90B::MRKT//ACTU/EUR12.5\r\n",
            "",
            crate::DepotPriceFailureStage::DecimalShape,
        ),
        (
            ":90B::MRKT//ACTU/EUR12,\r\n",
            ":98C::PRIC//20261301129999\r\n",
            crate::DepotPriceFailureStage::PriceTimestamp,
        ),
    ];

    for (price, timestamp, expected_stage) in cases {
        let financial = format!(
            ":16R:FIN\r\n:35B:ISIN DE000FINTS26\r\n\
             Fictional Staged Price\r\n{price}{timestamp}\
             :93B::AGGR//UNIT/1,\r\n:16S:FIN\r\n"
        );
        let (result, facts) = fictional_mt535_price_failure(&financial);
        assert_eq!(result.positions().len(), 1);
        assert!(result.positions()[0].price().is_none());
        assert_eq!(result.parse_counts().degraded(), 1);
        assert_eq!(facts.first_failing_price_ordinal(), Some(1));
        assert_eq!(facts.first_failing_price_stage(), Some(expected_stage));
        assert_eq!(facts.price_shapes().len(), 1);
        assert!(!facts.block_inventory().is_empty());
        assert_eq!(
            facts.position_failures()[0].site(),
            crate::DepotPositionFailureSite::Price
        );
        let rendered = format!("{facts:?}");
        for private_value in [
            "FICT",
            "EU1",
            "EUR",
            "12,",
            "12.5",
            "20261301129999",
            "DE000FINTS26",
        ] {
            assert!(!rendered.contains(private_value));
        }
    }

    let (_, timestamp_facts) = fictional_mt535_price_failure(concat!(
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS27\r\n",
        "Fictional Timestamp Shape\r\n",
        ":90B::MRKT//ACTU/EUR12,\r\n",
        ":98C::PRIC//20261301129999\r\n",
        ":93B::AGGR//UNIT/1,\r\n",
        ":16S:FIN\r\n"
    ));
    let timestamp = timestamp_facts.price_shapes()[0];
    assert!(timestamp.qualifier_shape_valid());
    assert!(timestamp.tag_unit_pair_valid());
    assert!(timestamp.currency_shape_valid());
    assert!(timestamp.decimal_shape_valid());
    assert!(timestamp.timestamp_present());
    assert_eq!(
        timestamp.timestamp_tag(),
        Some(crate::DepotPriceTimestampTagKind::DateTime98C)
    );
    assert_eq!(
        timestamp.timestamp_qualifier(),
        Some(crate::DepotPriceTimestampQualifierKind::Price)
    );
    assert!(timestamp.timestamp_qualifier_shape_valid());
    assert!(timestamp.timestamp_length_digit_shape_valid());
    assert!(!timestamp.timestamp_value_valid());

    let (_, malformed_timestamp_facts) = fictional_mt535_price_failure(concat!(
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS30\r\n",
        "Fictional Timestamp Digits\r\n",
        ":90B::MRKT//ACTU/EUR12,\r\n",
        ":98A::PRIC//2026072X\r\n",
        ":93B::AGGR//UNIT/1,\r\n",
        ":16S:FIN\r\n"
    ));
    let malformed_timestamp = malformed_timestamp_facts.price_shapes()[0];
    assert_eq!(
        malformed_timestamp.timestamp_tag(),
        Some(crate::DepotPriceTimestampTagKind::Date98A)
    );
    assert!(malformed_timestamp.timestamp_qualifier_shape_valid());
    assert!(!malformed_timestamp.timestamp_length_digit_shape_valid());
    assert!(!malformed_timestamp.timestamp_value_valid());
}

// DK Anlage 3 v3.9 4.3 page 406 defines the mandatory 90B currency as
// `a 3` plus an ISO 4217 code. HBCI 2.2 II.5.2 and IX.2.4 independently
// require the alphabetic currency code in uppercase. These fictional shapes
// distinguish invalid prefixes and a possible currency-less decimal without
// exposing the prefix or amount.
#[cfg(feature = "diagnostics")]
#[test]
fn mt535_currency_shape_categories_are_value_free() {
    use crate::DepotPriceCurrencyShapeKind::{
        Absent, BlankSpacePadded, LowercaseAlphabetic, MixedCaseAlphabetic, Numeric, Other,
    };

    let cases = [
        ("1,", Absent, false, false, true, None),
        ("   7,25", BlankSpacePadded, true, true, false, None),
        (
            "eur12,",
            LowercaseAlphabetic,
            true,
            true,
            false,
            Some("eur12,"),
        ),
        (
            "EuR12,",
            MixedCaseAlphabetic,
            true,
            true,
            false,
            Some("EuR12,"),
        ),
        ("1234,56", Numeric, true, true, true, Some("1234,56")),
        ("E-112,", Other, true, true, false, Some("E-112,")),
    ];

    for (payload, expected_shape, width_available, remainder_valid, entire_valid, private_marker) in
        cases
    {
        let financial = format!(
            ":16R:FIN\r\n:35B:ISIN DE000FINTS31\r\n\
             Fictional Currency Shape\r\n:90B::MRKT//ACTU/{payload}\r\n\
             :93B::AGGR//UNIT/1,\r\n:16S:FIN\r\n"
        );
        let (result, facts) = fictional_mt535_price_failure(&financial);
        assert_eq!(result.parse_counts().degraded(), 1);
        assert_eq!(
            facts.first_failing_price_stage(),
            Some(crate::DepotPriceFailureStage::CurrencyShape)
        );
        let shape = facts.price_shapes()[0];
        assert_eq!(shape.currency_shape(), Some(expected_shape));
        assert_eq!(shape.currency_prefix_width_available(), width_available);
        assert!(!shape.currency_shape_valid());
        assert_eq!(shape.decimal_shape_valid(), remainder_valid);
        assert_eq!(shape.entire_payload_decimal_shape_valid(), entire_valid);
        let rendered = format!("{facts:?}");
        if let Some(private_marker) = private_marker {
            assert!(!rendered.contains(private_marker));
        }
        assert!(!rendered.contains("DE000FINTS31"));
    }
}

// The ordinal is the one-based order of the price fields, not an identifier.
// A valid first fictional position followed by one malformed price must report 2.
#[cfg(feature = "diagnostics")]
#[test]
fn mt535_first_failing_price_ordinal_distinguishes_one_exceptional_position() {
    let (result, facts) = fictional_mt535_price_failure(concat!(
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS28\r\n",
        "Fictional Valid Price\r\n",
        ":90B::MRKT//ACTU/EUR11,\r\n",
        ":93B::AGGR//UNIT/1,\r\n",
        ":16S:FIN\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS29\r\n",
        "Fictional Invalid Price\r\n",
        ":90B::MRKT//ACTU/EUR12.5\r\n",
        ":93B::AGGR//UNIT/1,\r\n",
        ":16S:FIN\r\n"
    ));
    assert_eq!(result.positions().len(), 2);
    assert_eq!(result.parse_counts().degraded(), 1);
    assert_eq!(facts.price_shapes().len(), 2);
    assert_eq!(facts.first_failing_price_ordinal(), Some(2));
    assert_eq!(
        facts.first_failing_price_stage(),
        Some(crate::DepotPriceFailureStage::DecimalShape)
    );
    assert!(facts.price_shapes()[0].decimal_shape_valid());
    assert!(!facts.price_shapes()[1].decimal_shape_valid());
}

// FinTS Messages 2022 C.4.3.1 and return codes 3040/3010: a terminal
// no-entry page completes exhaustive pagination without erasing earlier pages.
#[test]
fn terminal_empty_depot_page_preserves_collected_positions() {
    let mut account = transaction_account("DE40123456780000123456", "300001", &[("HKWPD", 1)]);
    account.account_type = Some(30);
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = vec![account];
    engine.state.depot_positions_advertised = true;
    engine.state.depot_position_versions = vec![6];
    engine.state.depot_positions_requires_tan = Some(false);
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        engine.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));

    engine
        .depot_positions_request(0, now().date(), now().time())
        .unwrap();
    let first = binary_response(
        "HIRMS:3:2:3+3040::more:position-next'HIWPD:4:6:3+@",
        &mt535_page(1, "MORE"),
        "'HNHBS:5:1+2'",
        "dialog1",
        2,
    );
    assert!(matches!(
        engine.accept_depot_positions(&first, now()).unwrap(),
        DepotPositionsResult::Continue
    ));
    engine
        .next_depot_positions_page_request(now().date(), now().time())
        .unwrap();

    let empty = response(&["HIRMG:2:2+3010::no entries"], "dialog1", 3);
    let result = match engine.accept_depot_positions(&empty, now()).unwrap() {
        DepotPositionsResult::Complete(result) => result,
        _ => panic!("expected a completed paginated result"),
    };
    assert_eq!(result.positions().len(), 1);
    assert_eq!(
        result.positions()[0].instrument().isin(),
        Some("DE000FINTS05")
    );
    assert_eq!(
        result.parse_counts(),
        crate::DepotPositionParseCounts::default()
    );
}

// MT535 position-loss counts aggregate across exhaustive FinTS pagination:
// a degraded optional price remains in page one and a page-two position with
// no mandatory quantity is skipped.
#[test]
fn depot_position_parse_counts_accumulate_across_pages() {
    let account = transaction_account("DE40123456780000123456", "300001", &[("HKWPD", 1)]);
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = vec![account];
    engine.state.depot_positions_advertised = true;
    engine.state.depot_position_versions = vec![6];
    engine.state.depot_positions_requires_tan = Some(false);
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        engine.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));
    engine
        .depot_positions_request(0, now().date(), now().time())
        .unwrap();

    let first_payload = String::from_utf8(mt535_page(1, "MORE"))
        .unwrap()
        .replace(":93B::AGGR", ":90B::MRKT//ACTU/   7,25\r\n:93B::AGGR")
        .replace(
            ":16S:FIN\r\n-",
            concat!(
                ":16S:FIN\r\n",
                ":16R:ADDINFO\r\n",
                ":19A::HOLP//EUR10,00\r\n",
                ":19A::HOLP//EU110,00\r\n",
                ":16S:ADDINFO\r\n",
                "-"
            ),
        );
    let first = binary_response(
        "HIRMS:3:2:3+3040::more:position-next'HIWPD:4:6:3+@",
        first_payload.as_bytes(),
        "'HNHBS:5:1+2'",
        "dialog1",
        2,
    );
    assert!(matches!(
        engine.accept_depot_positions(&first, now()).unwrap(),
        DepotPositionsResult::Continue
    ));
    engine
        .next_depot_positions_page_request(now().date(), now().time())
        .unwrap();

    let second_payload = String::from_utf8(mt535_page(2, "LAST"))
        .unwrap()
        .replace(":93B::AGGR//UNIT/1,\r\n", "");
    let second = binary_response(
        "HIWPD:3:6:3+@",
        second_payload.as_bytes(),
        "'HNHBS:4:1+3'",
        "dialog1",
        3,
    );
    let result = match engine.accept_depot_positions(&second, now()).unwrap() {
        DepotPositionsResult::Complete(result) => result,
        _ => panic!("expected completed depot positions"),
    };
    assert_eq!(result.positions().len(), 1);
    assert!(result.positions()[0].price().is_none());
    assert_eq!(result.total_values().len(), 1);
    assert_eq!(
        result.parse_counts(),
        crate::DepotPositionParseCounts::new(1, 1, 1)
    );
}

// G112 C.12.1 and return code 3010: an empty response contains no
// bank-reported card identifier or transaction data. The requested UPD account
// remains the result binding, but is never relabeled as response content.
#[test]
fn empty_credit_card_transactions_do_not_fabricate_reported_values() {
    let mut account = transaction_account(
        "DE40123456780000123456",
        "444433******1111",
        &[("HKKKU", 1)],
    );
    account.account_type = Some(50);
    let mut engine = engine_with_method(TanProcess::ProcessVariantTwo);
    engine.state.accounts = vec![account];
    engine.state.credit_card_transactions_advertised = true;
    engine.state.credit_card_transactions = Some(crate::model::CreditCardCapability {
        account_required: false,
        date_range_allowed: true,
    });
    engine.state.credit_card_transactions_requires_tan = Some(false);
    let initialized = response(&["HIRMG:2:2+0010::accepted"], "dialog1", 1);
    assert!(matches!(
        engine.accept_initialization(&initialized, now()).unwrap(),
        InitializationResult::Connected
    ));
    engine
        .credit_card_transactions_request(0, None, None, now().date(), now().time())
        .unwrap();
    let empty = response(&["HIRMG:2:2+3010::no entries"], "dialog1", 2);
    let result = match engine
        .accept_credit_card_transactions(&empty, now())
        .unwrap()
    {
        CreditCardTransactionsResult::Complete(result) => result,
        _ => panic!("expected an empty completed result"),
    };

    assert_eq!(result.reported_card_number(), None);
    assert_eq!(result.reported_account_id(), None);
    assert!(result.current_balance().is_none());
    assert!(result.entries().is_empty());

    // G112 Data Dictionary "Kreditkartennummer": the institution chooses the
    // response masking form, so it is preserved rather than compared with UPD.
    engine
        .credit_card_transactions_request(0, None, None, now().date(), now().time())
        .unwrap();
    let masked = response(
        &["HIRMG:2:2+0010::accepted", "HIKKU:3:1:3+444433********11"],
        "dialog1",
        3,
    );
    let result = match engine
        .accept_credit_card_transactions(&masked, now())
        .unwrap()
    {
        CreditCardTransactionsResult::Complete(result) => result,
        _ => panic!("expected a completed masked-card result"),
    };
    assert_eq!(result.reported_card_number(), Some("444433********11"));
}

// PIN/TAN 2020 B.4.2.2 (correction T32), steps 2a-2c: code 3956 means
// approval is still pending. The last status response may instead carry the
// original order's explicit result together with HITAN process S. G112 C.12.1
// supplies HIKKU as that independently derived fictional result shape.
#[test]
fn decoupled_product_result_rejects_continuing_or_new_challenges() {
    for (responses, process) in [
        (
            [
                "HIRMG:2:2+0010::accepted",
                "HIRMS:3:2:3+3956::approval pending",
                "HIKKU:4:1:3+444433******1111",
                "HITAN:5:7:3+S++fictional-card-reference+nochallenge",
            ],
            TanProcess::Decoupled,
        ),
        (
            [
                "HIRMG:2:2+0010::accepted",
                "HIRMS:3:2:3+3955::new approval",
                "HIKKU:4:1:3+444433******1111",
                "HITAN:5:7:3+4++fictional-new-reference+Approve again",
            ],
            TanProcess::Decoupled,
        ),
        (
            [
                "HIRMG:2:2+0010::accepted",
                "HIRMS:3:2:3+0020::executed",
                "HIKKU:4:1:3+444433******1111",
                "HITAN:5:7:3+S++fictional-other-reference+nochallenge",
            ],
            TanProcess::Decoupled,
        ),
        (
            [
                "HIRMG:2:2+0010::accepted",
                "HIRMS:3:2:3+0020::executed",
                "HIKKU:4:1:3+444433******1111",
                "HITAN:5:6:3+2++fictional-card-reference+nochallenge",
            ],
            TanProcess::ProcessVariantTwo,
        ),
    ] {
        let mut engine = connected_credit_card_engine(process, true);
        engine
            .credit_card_transactions_request(0, None, None, now().date(), now().time())
            .unwrap();
        let challenge = response(
            &[
                "HIRMG:2:2+0010::accepted",
                "HIRMS:3:2:4+3955::approval elsewhere",
                if process == TanProcess::Decoupled {
                    "HITAN:4:7:4+4++fictional-card-reference+Approve fictional entries"
                } else {
                    "HITAN:4:6:4+4++fictional-card-reference+Approve fictional entries"
                },
            ],
            "dialog1",
            2,
        );
        let mut pending = match engine
            .accept_credit_card_transactions(&challenge, now())
            .unwrap()
        {
            CreditCardTransactionsResult::Challenge(pending) => *pending,
            _ => panic!("expected a credit-card approval challenge"),
        };
        if process == TanProcess::Decoupled {
            engine
                .decoupled_poll_request(
                    &mut pending,
                    crate::PollingMode::Manual,
                    now().checked_add_signed(TimeDelta::seconds(2)).unwrap(),
                )
                .unwrap();
        } else {
            engine
                .tan_submission_request(&pending, &Tan::new("123456").unwrap(), now())
                .unwrap();
        }

        let error = engine
            .accept_credit_card_transactions_continuation(
                &response(&responses, "dialog1", 3),
                pending,
                now(),
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            Error::InvalidResponse {
                structure: "product result and TAN challenge together"
            }
        ));
        assert!(!format!("{error:?}").contains("fictional-card"));
        assert!(!format!("{error}").contains("444433"));
    }

    let mut unsolicited = connected_credit_card_engine(TanProcess::Decoupled, false);
    unsolicited
        .credit_card_transactions_request(0, None, None, now().date(), now().time())
        .unwrap();
    let error = unsolicited
        .accept_credit_card_transactions(
            &response(
                &[
                    "HIRMG:2:2+0010::accepted",
                    "HITAN:3:7:4+4++fictional-unsolicited-reference+Approve",
                ],
                "dialog1",
                2,
            ),
            now(),
        )
        .err()
        .unwrap();
    assert!(matches!(
        error,
        Error::InvalidResponse {
            structure: "unsolicited product TAN challenge"
        }
    ));
}

// PIN/TAN correction T31 and FinTS PIN/TAN 2020-07-10, B.5.2:
// the advertised operation TAN status governs the transaction continuation.
#[test]
fn booked_transaction_tan_continuation_is_typed_and_operation_bound() {
    let account = transaction_account("DE40123456780000123456", "123456", &[("HKKAZ", 1)]);
    let mut engine = connected_transaction_engine(vec![account], false, vec![7], None, Some(true));
    let request = engine
        .transaction_request(0, None, None, now().date(), now().time())
        .unwrap();
    let payload = crate::wire::Message::parse(&request)
        .unwrap()
        .payload_segments()
        .unwrap();
    assert!(payload.iter().any(|segment| {
        segment.header().unwrap().code == b"HKTAN"
            && segment.elements()[2].components()[0]
                .as_text()
                .is_some_and(|operation| operation == "HKKAZ")
    }));

    let challenge = response(
        &[
            "HIRMG:2:2+0010::accepted",
            "HITAN:3:6:3+4++fictional-transaction-reference+Approve fictional entries",
        ],
        "dialog1",
        2,
    );
    let pending = match engine.accept_transactions(&challenge, now()).unwrap() {
        TransactionsResult::Challenge(pending) => *pending,
        _ => panic!("expected transaction TAN challenge"),
    };
    engine
        .tan_submission_request(&pending, &Tan::new("123456").unwrap(), now())
        .unwrap();
    let completion = binary_response(
        "HIKAZ:3:7:3+@",
        &mt940_page(1, "FICTBANKREF1", "123456"),
        "'HNHBS:4:1+3'",
        "dialog1",
        3,
    );

    assert!(matches!(
        engine
            .accept_transactions_continuation(&completion, pending, now())
            .unwrap(),
        TransactionsResult::Complete(_)
    ));
}

// SCOPE Gate 2: BPD/UPD capabilities are authoritative and unsupported
// combinations remain typed limitations instead of guessed fallbacks.
#[test]
fn transaction_capability_failures_are_typed_before_transport() {
    let unauthorized = connected_transaction_engine(
        vec![transaction_account("DE40123456780000123456", "123456", &[])],
        false,
        vec![7],
        None,
        Some(false),
    );
    assert!(matches!(
        unauthorized.transaction_format(&unauthorized.state.accounts[0]),
        Err(Error::Unsupported(Limitation::TransactionsNotAuthorized))
    ));

    let account = transaction_account("DE40123456780000123456", "123456", &[("HKKAZ", 1)]);
    let mut unsupported =
        connected_transaction_engine(vec![account.clone()], false, vec![], None, None);
    unsupported.state.transaction_capability_advertised = true;
    assert!(matches!(
        unsupported.transaction_format(&account),
        Err(Error::Unsupported(Limitation::TransactionsVersion))
    ));

    unsupported.state.transaction_capability_advertised = false;
    assert!(matches!(
        unsupported.transaction_format(&account),
        Err(Error::Unsupported(Limitation::TransactionsNotAdvertised))
    ));
}
