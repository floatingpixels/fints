use super::*;
use crate::model::{CreditCardCapability, InstituteState, OperationPermission};

fn context<'a>(institute: &'a InstituteId, credentials: &'a Credentials) -> SecurityContext<'a> {
    SecurityContext {
        institute,
        credentials,
        system_id: "fictional-system",
        security_function: "942",
        profile_version: "2",
        dialog_id: "fictional-dialog",
        message_number: 2,
        date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
        time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
    }
}

fn product_account(account_type: u8, account_number: &str) -> Account {
    Account {
        iban: Some("DE40123456780000123456".to_owned()),
        bic: None,
        account_number: Some(account_number.to_owned()),
        subaccount: Some("CARD-CUST".to_owned()),
        institute: Some(InstituteState {
            country_code: "280".to_owned(),
            institute_code: "12345678".to_owned(),
        }),
        currency: Some("EUR".to_owned()),
        account_type: Some(account_type),
        owner_name_1: Some("Fictional Person".to_owned()),
        owner_name_2: None,
        product_name: Some("Fictional Product".to_owned()),
        allowed_operations: vec![OperationPermission {
            code: if account_type == 30 { "HKWPD" } else { "HKKKU" }.to_owned(),
            required_signatures: 1,
        }],
        unlisted_operations_unknown: false,
    }
}

fn operation(encoded: &[u8], code: &[u8]) -> Vec<u8> {
    let payload = Message::parse(encoded).unwrap().payload_segments().unwrap();
    encode_segments(&[payload
        .into_iter()
        .find(|segment| segment.header().unwrap().code == code)
        .unwrap()])
    .unwrap()
}

// FinTS Messages 2022-04-15 C.4.3.1-C.4.3.2. These exact segments are
// independent golden fixtures for the current HKWPD 6 and HKWDU 5 layouts.
#[test]
fn depot_requests_match_independent_wire_fixtures() {
    let institute = InstituteId::new("280", "12345678").unwrap();
    let credentials = Credentials::new("fictional-user", None, "private-pin").unwrap();
    let context = context(&institute, &credentials);
    let account = product_account(30, "300001");

    let positions =
        depot_positions_request(&context, &account, Some("position-next"), None).unwrap();
    assert_eq!(
        operation(&positions, b"HKWPD"),
        b"HKWPD:3:6+300001:CARD-CUST:280:12345678++++position-next'"
    );

    let transactions = securities_transactions_request(
        &context,
        &account,
        Some(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()),
        Some(NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()),
        Some("transaction-next"),
        None,
    )
    .unwrap();
    assert_eq!(
        operation(&transactions, b"HKWDU"),
        concat!(
            "HKWDU:3:5+300001:CARD-CUST:280:12345678+N+",
            "+20260701+20260728++transaction-next'"
        )
        .as_bytes()
    );
}

// FinTS G112 / CR 538 C.12.1-C.12.2. The account DEG is present only when
// the advertised operation parameter requires it.
#[test]
fn credit_card_requests_match_conditional_account_binding_fixtures() {
    let institute = InstituteId::new("280", "12345678").unwrap();
    let credentials = Credentials::new("fictional-user", None, "private-pin").unwrap();
    let context = context(&institute, &credentials);
    let account = product_account(50, "444433******1111");
    let capability = CreditCardCapability {
        account_required: true,
        date_range_allowed: true,
        entry_count_allowed: true,
    };

    let transactions = credit_card_transactions_request(
        &context,
        &account,
        &capability,
        Some(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()),
        Some(NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()),
        Some("card-next"),
        None,
    )
    .unwrap();
    assert_eq!(
        operation(&transactions, b"HKKKU"),
        concat!(
            "HKKKU:3:1+DE40123456780000123456::444433******1111",
            ":CARD-CUST:280:12345678+444433******1111+CARD-CUST",
            "+20260701+20260728++card-next'"
        )
        .as_bytes()
    );

    let balance = credit_card_balance_request(&context, &account, true, None).unwrap();
    assert_eq!(
        operation(&balance, b"HKKKS"),
        concat!(
            "HKKKS:3:1+DE40123456780000123456::444433******1111",
            ":CARD-CUST:280:12345678+444433******1111+CARD-CUST'"
        )
        .as_bytes()
    );

    let capability = CreditCardCapability {
        account_required: false,
        date_range_allowed: true,
        entry_count_allowed: true,
    };
    let transactions =
        credit_card_transactions_request(&context, &account, &capability, None, None, None, None)
            .unwrap();
    assert_eq!(
        operation(&transactions, b"HKKKU"),
        b"HKKKU:3:1++444433******1111+CARD-CUST'"
    );
    let balance = credit_card_balance_request(&context, &account, false, None).unwrap();
    assert_eq!(
        operation(&balance, b"HKKKS"),
        b"HKKKS:3:1++444433******1111+CARD-CUST'"
    );
}
