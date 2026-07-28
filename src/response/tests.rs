use super::*;
use crate::model::{CreditDebit, TanProcess};
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
