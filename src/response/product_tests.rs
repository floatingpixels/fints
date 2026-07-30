use super::*;
use crate::{CreditDebit, QuantityUnit};
use chrono::NaiveDate;

fn message_with_binary(code: &str, version: u16, payload: &[u8]) -> Vec<u8> {
    let prefix = format!(
        "HNHBK:1:3+000000000000+300+dialog1+2+dialog1:1'\
         HIRMG:2:2+0010::accepted'{code}:3:{version}:4+@"
    );
    let suffix = "'HNHBS:4:1+2'";
    let mut wire = prefix.into_bytes();
    wire.extend_from_slice(payload.len().to_string().as_bytes());
    wire.push(b'@');
    wire.extend_from_slice(payload);
    wire.extend_from_slice(suffix.as_bytes());
    let length = format!("{:012}", wire.len());
    wire[10..22].copy_from_slice(length.as_bytes());
    wire
}

fn message(segments: &[&str]) -> Vec<u8> {
    let trailer = segments.len() + 2;
    let mut wire = "HNHBK:1:3+000000000000+300+dialog1+2+dialog1:1'".to_owned();
    for segment in segments {
        wire.push_str(segment);
        wire.push('\'');
    }
    wire.push_str(&format!("HNHBS:{trailer}:1+2'"));
    let length = format!("{:012}", wire.len());
    wire.replace_range(10..22, &length);
    wire.into_bytes()
}

// DK Anlage 3 v3.9, 4.3 MT535; FinTS Messages 2022 C.4.3.1 HIWPD 6.
// This expected payload is independently written and entirely fictional.
#[test]
fn mt535_position_fixture_preserves_only_explicit_values() {
    let payload = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":98A::STAT//20260728\r\n",
        ":22F::STTY//CUST\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS05\r\n",
        "/DE/FICT01\r\n",
        "Fictional Security\r\n",
        ":90B::MRKT//ACTU/EUR123,45\r\n",
        ":98A::PRIC//20260728\r\n",
        ":93B::AGGR//UNIT/10,\r\n",
        ":16R:SUBBAL\r\n",
        ":93C::TAVI//UNIT/AVAI/10,\r\n",
        ":16S:SUBBAL\r\n",
        ":19A::HOLD//EUR1234,50\r\n",
        ":70E::HOLD//1STK+511+00081+DE+20200102\r\n",
        "2120,00+EUR\r\n",
        ":16S:FIN\r\n",
        ":16R:ADDINFO\r\n",
        ":19A::HOLP//EUR1234,50\r\n",
        ":16S:ADDINFO\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWPD", 6, payload)).unwrap();
    let page = response.depot_positions(6).unwrap().unwrap();

    assert!(!page.more);
    assert_eq!(page.positions.len(), 1);
    let position = &page.positions[0];
    assert_eq!(position.instrument.isin(), Some("DE000FINTS05"));
    assert_eq!(position.instrument.wkn(), Some("FICT01"));
    assert_eq!(position.instrument.name(), "Fictional Security");
    assert_eq!(position.quantity.unit(), QuantityUnit::Units);
    assert_eq!(position.quantity.coefficient(), 10);
    assert_eq!(position.price.as_ref().unwrap().currency(), Some("EUR"));
    assert_eq!(
        position.price.as_ref().unwrap().date(),
        Some(NaiveDate::from_ymd_opt(2026, 7, 28).unwrap())
    );
    assert_eq!(position.market_values[0].amount().coefficient(), 123_450);
    assert_eq!(position.cost_basis.as_ref().unwrap().coefficient(), 12_000);
    assert_eq!(page.total_values[0].amount().coefficient(), 123_450);
    #[cfg(feature = "development-diagnostics")]
    {
        use crate::DepotBlockKind;

        let facts = &page.development_facts;
        assert_eq!(facts.positions().len(), 1);
        assert!(facts.positions()[0].isin_present());
        assert!(facts.positions()[0].wkn_present());
        assert!(facts.positions()[0].price_present());
        assert!(facts.positions()[0].market_value_present());
        assert!(facts.positions()[0].cost_basis_present());
        assert_eq!(
            facts
                .block_inventory()
                .iter()
                .map(|fact| (fact.kind(), fact.depth(), fact.occurrence()))
                .collect::<Vec<_>>(),
            [
                (DepotBlockKind::General, 1, 1),
                (DepotBlockKind::FinancialInstrument, 1, 1),
                (DepotBlockKind::SubBalance, 2, 1),
                (DepotBlockKind::AdditionalInformation, 1, 1),
            ]
        );
        let rendered = format!("{facts:?}");
        assert!(!rendered.contains("DE000FINTS05"));
        assert!(!rendered.contains("FICT01"));
        assert!(!rendered.contains("1234,50"));
    }
}

// HBCI 2.2 VII.4.3.1 and IX.2.4: HIWPD 5 contains exactly one binary
// SRG-1998 MT535 statement. This fixture is independently derived from the
// archived full-message example and uses fictional identities and values.
// HIWPD 4 is not accepted here because that version carries MT571.
#[test]
fn hiwpd_five_parses_its_official_mt535_payload_and_requires_version_match() {
    let payload = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":98A::STAT//20260730\r\n",
        ":22F::STTY//CUST\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS15\r\n",
        "/DE/FICT15\r\n",
        "Fictional Legacy Holding\r\n",
        ":90B::MRKT//ACTU/EUR12,50\r\n",
        ":98A::PRIC//20260730\r\n",
        ":93B::AGGR//UNIT/4,\r\n",
        ":16R:SUBBAL\r\n",
        ":93C::TAVI//UNIT/AVAI/4,\r\n",
        ":16S:SUBBAL\r\n",
        ":19A::HOLD//EUR50,00\r\n",
        ":16S:FIN\r\n",
        ":16R:ADDINFO\r\n",
        ":19A::HOLP//EUR50,00\r\n",
        ":16S:ADDINFO\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWPD", 5, payload)).unwrap();
    let page = response.depot_positions(5).unwrap().unwrap();

    assert!(!page.more);
    assert_eq!(page.positions.len(), 1);
    assert_eq!(page.positions[0].instrument().isin(), Some("DE000FINTS15"));
    assert_eq!(page.positions[0].quantity().coefficient(), 4);
    assert_eq!(page.total_values.len(), 1);

    assert!(matches!(
        response.depot_positions(6),
        Err(Error::UnsupportedSegment {
            code: "HIWPD",
            version: 5
        })
    ));
    assert!(matches!(
        response.depot_positions(4),
        Err(Error::UnsupportedSegment {
            code: "HIWPD",
            version: 4
        })
    ));

    let malformed = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS15\r\n",
        "Fictional Legacy Holding\r\n",
        ":16S:FIN\r\n",
        "-"
    );
    let response = Response::parse(&message_with_binary("HIWPD", 5, malformed.as_bytes())).unwrap();
    assert!(matches!(
        response.depot_positions(5),
        Err(Error::MalformedSecuritiesData { .. })
    ));
}

// DK Anlage 3 v3.9, 4.3: AGGR can be negative, INDC can carry a
// percentage quote, 35B can identify by WKN, and a structured line-two
// cost basis without currency is percentage-denominated.
#[test]
fn mt535_variants_preserve_sign_quality_and_percentage_cost_basis() {
    let payload = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":98A::STAT//20260728\r\n",
        ":22F::STTY//CUST\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:/DE/FICT02\r\n",
        "Fictional Bond\r\n",
        ":90A::INDC//PRCT/101,25\r\n",
        ":93B::AGGR//FAMT/N1000,\r\n",
        ":16R:SUBBAL\r\n",
        ":93C::TAVI//FAMT/AVAI/N1000,\r\n",
        ":16S:SUBBAL\r\n",
        ":70E::HOLD//1EUR+141+00024+DE+20200102\r\n",
        "2100,25+\r\n",
        ":16S:FIN\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWPD", 6, payload)).unwrap();
    let page = response.depot_positions(6).unwrap().unwrap();
    let position = &page.positions[0];

    assert_eq!(position.instrument.wkn(), Some("FICT02"));
    assert!(position.quantity.is_negative());
    assert_eq!(position.quantity.unit(), QuantityUnit::Nominal);
    assert_eq!(
        position.price.as_ref().unwrap().quality(),
        Some(crate::PriceQuality::Indicative)
    );
    let cost = position.cost_basis.as_ref().unwrap();
    assert!(cost.is_percentage());
    assert_eq!(cost.currency(), None);
    assert_eq!(cost.coefficient(), 10_025);
}

// DK Anlage 3 v3.9, 4.4 MT536; FinTS Messages 2022 C.4.3.2 HIWDU 5.
// Fees have no typed MT536 field and therefore are not inferred from free text.
#[test]
fn mt536_transaction_fixture_preserves_reference_amounts_and_dates() {
    let payload = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":13A::STAT//007\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":69A::STAT//20260701/20260728\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS05\r\n",
        "Fictional Security\r\n",
        ":90B::MRKT//ACTU/EUR123,45\r\n",
        ":16R:TRAN\r\n",
        ":16R:LINK\r\n",
        ":20C::RELA//FICTREF0001\r\n",
        ":16S:LINK\r\n",
        ":16R:TRANSDET\r\n",
        ":36B::PSTA//UNIT/2,\r\n",
        ":19A::PSTA//EUR246,90\r\n",
        ":19A::ACRU//EUR1,23\r\n",
        ":22F::TRAN//SETT\r\n",
        ":22H::REDE//RECE\r\n",
        ":22H::PAYM//FREE\r\n",
        ":98A::ESET//20260727\r\n",
        ":98A::SETT//20260729\r\n",
        ":70E::TRDE//Fictional purchase\r\n",
        ":16S:TRANSDET\r\n",
        ":16S:TRAN\r\n",
        ":16S:FIN\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWDU", 5, payload)).unwrap();
    let page = response.securities_transactions().unwrap().unwrap();

    assert!(!page.more);
    assert_eq!(page.entries.len(), 1);
    let entry = &page.entries[0];
    assert_eq!(entry.reference(), Some("FICTREF0001"));
    assert_eq!(entry.direction(), Some(crate::SecuritiesMovement::Receipt));
    assert_eq!(entry.quantity().unwrap().coefficient(), 2);
    assert_eq!(entry.amount().unwrap().amount().coefficient(), 24_690);
    assert_eq!(
        entry.effective_date(),
        Some(NaiveDate::from_ymd_opt(2026, 7, 27).unwrap())
    );
    assert_eq!(
        entry.value_date(),
        Some(NaiveDate::from_ymd_opt(2026, 7, 29).unwrap())
    );
    assert_eq!(entry.free_text(), &["Fictional purchase"]);
    #[cfg(feature = "development-diagnostics")]
    {
        use crate::DepotBlockKind;

        let facts = &page.development_facts;
        assert_eq!(facts.transactions().len(), 1);
        let presence = facts.transactions()[0];
        assert!(presence.isin_present());
        assert!(presence.reference_present());
        assert!(presence.quantity_present());
        assert!(presence.price_present());
        assert!(presence.amount_present());
        assert!(presence.accrued_interest_present());
        assert!(presence.transaction_kind_present());
        assert!(presence.movement_present());
        assert!(presence.effective_date_present());
        assert!(presence.value_date_present());
        assert!(presence.free_text_present());
        assert_eq!(
            facts
                .block_inventory()
                .iter()
                .map(|fact| (fact.kind(), fact.depth(), fact.occurrence()))
                .collect::<Vec<_>>(),
            [
                (DepotBlockKind::General, 1, 1),
                (DepotBlockKind::FinancialInstrument, 1, 1),
                (DepotBlockKind::Transaction, 2, 1),
                (DepotBlockKind::Link, 3, 1),
                (DepotBlockKind::TransactionDetails, 3, 1),
            ]
        );
    }
}

// DK Anlage 3 v3.9, 4.4: LINK/RELA is mandatory but NONREF means that no
// transaction reference was supplied; TRANSDET itself is optional.
#[test]
fn mt536_nonref_without_optional_details_fabricates_nothing() {
    let payload = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":69A::STAT//20260701/20260728\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS05\r\n",
        "Fictional Security\r\n",
        ":16R:TRAN\r\n",
        ":16R:LINK\r\n",
        ":20C::RELA//NONREF\r\n",
        ":16S:LINK\r\n",
        ":16S:TRAN\r\n",
        ":16S:FIN\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWDU", 5, payload)).unwrap();
    let page = response.securities_transactions().unwrap().unwrap();
    let entry = &page.entries[0];

    assert_eq!(entry.reference(), None);
    assert!(entry.quantity().is_none());
    assert_eq!(entry.direction(), None);
    assert!(entry.amount().is_none());
    assert!(entry.effective_date().is_none());
}

// FinTS G112 / CR 538, C.12.1 HIKKU 1 and "Umsatz Kreditkartenkonto".
#[test]
fn credit_card_transactions_fixture_preserves_repeated_typed_fields() {
    let fixture = message(&[
        "HIRMG:2:2+0010::accepted",
        concat!(
            "HIKKU:3:1:4+444433******1111+CARD-CUST",
            "+D:250,00:EUR:20260728+20260701+20260801",
            "+444433******1111:20260727:20260728::20260728",
            ":10,00:USD:D:0,9:9,00:EUR:D",
            ":Purchase:at merchant:Second group:detail:::::DE",
            ":Fictional Merchant:TERM-1:N:BOOKREF-1:F001:07-2026",
            ":Cash fee display:AEE display"
        ),
    ]);
    let response = Response::parse(&fixture).unwrap();
    let page = response.credit_card_transactions().unwrap().unwrap();

    assert_eq!(page.reported_card_number, "444433******1111");
    assert_eq!(page.reported_account_id.as_deref(), Some("CARD-CUST"));
    assert_eq!(
        page.current_balance.as_ref().unwrap().amount().direction(),
        CreditDebit::Debit
    );
    assert_eq!(
        page.current_balance.as_ref().unwrap().timestamp().date(),
        NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()
    );
    assert_eq!(page.entries.len(), 1);
    let entry = &page.entries[0];
    assert_eq!(entry.booking_reference(), Some("BOOKREF-1"));
    assert_eq!(entry.fee_code(), Some("F001"));
    assert_eq!(entry.country(), Some("DE"));
    assert_eq!(entry.merchant_name(), Some("Fictional Merchant"));
    assert_eq!(entry.booking_amount().amount().coefficient(), 900);
    assert_eq!(
        entry.description(),
        &["Purchase", "at merchant", "Second group", "detail"]
    );
    assert_eq!(entry.cash_fee(), Some("Cash fee display"));
    assert_eq!(entry.foreign_use_fee(), Some("AEE display"));
}

// FinTS G112 / CR 538, C.12.2 HIKKS 1 and B.8 btgv.
#[test]
fn credit_card_balance_fixture_keeps_balance_components_independent() {
    let fixture = message(&[
        "HIRMG:2:2+0010::accepted",
        concat!(
            "HIKKS:3:1:4+444433******1111+CARD-CUST",
            "+D:250,00:EUR:20260728:120000",
            "+1000,00:EUR:C+50,00:EUR+2000,00:EUR+20260801"
        ),
    ]);
    let response = Response::parse(&fixture).unwrap();
    let balance = response.credit_card_balance().unwrap().unwrap();

    assert_eq!(balance.reported_account_id.as_deref(), Some("CARD-CUST"));
    assert_eq!(balance.current.amount().direction(), CreditDebit::Debit);
    assert_eq!(
        balance.current.timestamp().time(),
        chrono::NaiveTime::from_hms_opt(12, 0, 0)
    );
    assert_eq!(
        balance.available.as_ref().unwrap().direction(),
        CreditDebit::Credit
    );
    assert_eq!(balance.open_authorizations.unwrap().coefficient(), 5_000);
    assert_eq!(balance.credit_limit.unwrap().coefficient(), 200_000);
}

// G112 / CR0538 B.8 and C.12.2: optional trailing DEG components may be cut;
// unused extension components do not alter the first typed values.
#[test]
fn credit_card_groups_accept_cut_optional_tails_and_unused_extensions() {
    let fixture = message(&[
        "HIRMG:2:2+0010::accepted",
        concat!(
            "HIKKS:3:1:4+444433******1111:IGNORED+CARD-CUST:IGNORED",
            "+D:250,00:EUR:20260728::IGNORED",
            "+1000,00:EUR:C:IGNORED",
            "+50,00:EUR:IGNORED",
            "+2000,00:EUR:IGNORED",
            "+20260801:IGNORED"
        ),
    ]);
    let balance = Response::parse(&fixture)
        .unwrap()
        .credit_card_balance()
        .unwrap()
        .unwrap();

    assert_eq!(balance.reported_card_number, "444433******1111");
    assert_eq!(balance.reported_account_id.as_deref(), Some("CARD-CUST"));
    assert_eq!(balance.current.amount().amount().coefficient(), 25_000);
    assert_eq!(balance.available.unwrap().amount().coefficient(), 100_000);
    assert_eq!(balance.open_authorizations.unwrap().coefficient(), 5_000);
    assert_eq!(balance.credit_limit.unwrap().coefficient(), 200_000);
    assert_eq!(
        balance.next_statement_date,
        NaiveDate::from_ymd_opt(2026, 8, 1)
    );
}

// DK Anlage 3 v3.9, 4.3-4.4: block ends must match their starts and mandatory
// GENL/FIN/TRANSDET fields cannot yield partial results.
#[test]
fn malformed_securities_documents_fail_without_partial_results() {
    let mismatched = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":16S:FIN\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWPD", 6, mismatched)).unwrap();
    let mismatched_site = match response.depot_positions(6) {
        Err(Error::MalformedSecuritiesData { site }) => site,
        _ => panic!("expected malformed MT535 block"),
    };
    assert!(mismatched_site.contains("document/16R:16S/mismatch"));

    let missing_quantity = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":98A::STAT//20260728\r\n",
        ":22F::STTY//CUST\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS05\r\n",
        "Fictional Security\r\n",
        ":16S:FIN\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWPD", 6, missing_quantity)).unwrap();
    let missing_quantity_site = match response.depot_positions(6) {
        Err(Error::MalformedSecuritiesData { site }) => site,
        _ => panic!("expected malformed MT535 position"),
    };
    assert!(missing_quantity_site.contains("MT535/FIN/93B:AGGR"));
    assert_ne!(mismatched_site, missing_quantity_site);

    let inactive_with_position = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//N\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS07\r\n",
        "Fictional Inactive Security\r\n",
        ":93B::AGGR//UNIT/1,\r\n",
        ":16S:FIN\r\n",
        "-"
    );
    let response = Response::parse(&message_with_binary(
        "HIWPD",
        6,
        inactive_with_position.as_bytes(),
    ))
    .unwrap();
    assert!(matches!(
        response.depot_positions(6),
        Err(Error::MalformedSecuritiesData { .. })
    ));

    let invalid_indicator = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:anything/UNKNOWN\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//N\r\n",
        ":16S:GENL\r\n",
        "-"
    );
    let response = Response::parse(&message_with_binary(
        "HIWPD",
        6,
        invalid_indicator.as_bytes(),
    ))
    .unwrap();
    assert!(matches!(
        response.depot_positions(6),
        Err(Error::MalformedSecuritiesData { .. })
    ));
}

// DK Anlage 3 v3.9, chapter 4 general syntax rule 6: an MT535/MT536
// record starts with CRLF and ends with CRLF followed by "-".
#[test]
fn securities_document_rejects_missing_leading_crlf() {
    let payload = concat!(
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":98A::STAT//20260728\r\n",
        ":22F::STTY//CUST\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//N\r\n",
        ":16S:GENL\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWPD", 6, payload)).unwrap();

    assert!(matches!(
        response.depot_positions(6),
        Err(Error::MalformedSecuritiesData { .. })
    ));
}

// DK Anlage 3 v3.9 chapter 4 rule 6 plus the owner-ratified transport shape:
// MT535 and MT536 accept one CRLF after the final "-" and reject two.
#[test]
fn securities_documents_accept_exactly_one_trailing_crlf() {
    let mt535 = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":98A::STAT//20260729\r\n",
        ":22F::STTY//CUST\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//N\r\n",
        ":16S:GENL\r\n",
        "-"
    );
    let mt536 = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":69A::STAT//20260701/20260729\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//N\r\n",
        ":16S:GENL\r\n",
        "-"
    );
    for (code, version, payload) in [("HIWPD", 6, mt535), ("HIWDU", 5, mt536)] {
        let one = format!("{payload}\r\n");
        let response =
            Response::parse(&message_with_binary(code, version, one.as_bytes())).unwrap();
        let accepted = if code == "HIWPD" {
            response.depot_positions(6).map(|value| value.is_some())
        } else {
            response
                .securities_transactions()
                .map(|value| value.is_some())
        };
        assert!(matches!(accepted, Ok(true)));

        let two = format!("{payload}\r\n\r\n");
        let response =
            Response::parse(&message_with_binary(code, version, two.as_bytes())).unwrap();
        let rejected = if code == "HIWPD" {
            response.depot_positions(6).map(|_| ())
        } else {
            response.securities_transactions().map(|_| ())
        };
        assert!(matches!(
            rejected,
            Err(Error::MalformedSecuritiesData { .. })
        ));
    }
}

// DK Anlage 3 v3.9, 4.3: 98C carries an ASCII-numeric date and time.
// A fictional Latin-1 high byte inside the date must be a typed parse error,
// never a UTF-8 character-boundary panic.
#[test]
fn securities_date_with_latin1_high_byte_is_typed_error() {
    let mut payload = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":98A::STAT//20260728\r\n",
        ":22F::STTY//CUST\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS05\r\n",
        "Fictional Security\r\n",
        ":90B::MRKT//ACTU/EUR123,45\r\n",
        ":98C::PRIC//2026072"
    )
    .as_bytes()
    .to_vec();
    payload.push(0xe4);
    payload.extend_from_slice(
        concat!(
            "12000\r\n",
            ":93B::AGGR//UNIT/1,\r\n",
            ":16R:SUBBAL\r\n",
            ":93C::TAVI//UNIT/AVAI/1,\r\n",
            ":16S:SUBBAL\r\n",
            ":16S:FIN\r\n",
            "-"
        )
        .as_bytes(),
    );
    let response = Response::parse(&message_with_binary("HIWPD", 6, &payload)).unwrap();

    assert!(matches!(
        response.depot_positions(6),
        Err(Error::MalformedSecuritiesData { .. })
    ));
}

// DK Anlage 3 v3.9, 4.3 specifies GENL constants, STAT data, SUBBAL, and
// structured 70E::HOLD line digits. These values are not consumed by the
// Gate 4 position result, so the acceptance-space policy reads past them.
#[test]
fn mt535_unconsumed_general_subbalance_and_cost_basis_fields_are_nonfatal() {
    let payload = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:opaque/ONLY\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FICTIONAL\r\n",
        ":20C::IGNO//unused\r\n",
        ":16S:FICTIONAL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS06\r\n",
        "Fictional Acceptance Security\r\n",
        ":93B::AGGR//UNIT/3,\r\n",
        ":70E::HOLD//unstructured fictional cost data\r\n",
        ":16S:FIN\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWPD", 6, payload)).unwrap();
    let page = response.depot_positions(6).unwrap().unwrap();

    assert_eq!(page.positions.len(), 1);
    assert!(page.positions[0].cost_basis().is_none());

    let active_empty = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:anything/LAST\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        "-"
    );
    let response =
        Response::parse(&message_with_binary("HIWPD", 6, active_empty.as_bytes())).unwrap();
    assert!(
        response
            .depot_positions(6)
            .unwrap()
            .unwrap()
            .positions
            .is_empty()
    );
}

// DK Anlage 3 v3.9, 4.4 specifies the GENL constants, PAYM value, TRAN code
// set, and block vocabulary. Gate 4 consumes the TRAN reference/details but
// not those constants, and a FIN may contain multiple TRAN blocks.
#[test]
fn mt536_unconsumed_constants_and_sibling_blocks_do_not_discard_transactions() {
    let two_transactions = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:opaque/ONLY\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FICTIONAL\r\n",
        ":20C::IGNO//unused\r\n",
        ":16S:FICTIONAL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS05\r\n",
        "Fictional Security\r\n",
        ":16R:FICTIONAL\r\n",
        ":16S:FICTIONAL\r\n",
        ":16R:TRAN\r\n",
        ":16R:LINK\r\n",
        ":20C::RELA//NONREF\r\n",
        ":16S:LINK\r\n",
        ":16R:FICTIONAL\r\n",
        ":16S:FICTIONAL\r\n",
        ":16S:TRAN\r\n",
        ":16R:TRAN\r\n",
        ":16R:LINK\r\n",
        ":20C::RELA//NONREF\r\n",
        ":16S:LINK\r\n",
        ":16S:TRAN\r\n",
        ":16S:FIN\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWDU", 5, two_transactions)).unwrap();
    assert_eq!(
        response
            .securities_transactions()
            .unwrap()
            .unwrap()
            .entries
            .len(),
        2
    );

    let opaque_values = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":69A::STAT//20260701/20260728\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS05\r\n",
        "Fictional Security\r\n",
        ":16R:TRAN\r\n",
        ":16R:LINK\r\n",
        ":20C::RELA//NONREF\r\n",
        ":16S:LINK\r\n",
        ":16R:TRANSDET\r\n",
        ":36B::PSTA//UNIT/1,\r\n",
        ":22F::TRAN//FICT\r\n",
        ":22H::REDE//RECE\r\n",
        ":22H::PAYM//APMT\r\n",
        ":25D::MOVE//PEND\r\n",
        ":98A::ESET//20260728\r\n",
        ":16S:TRANSDET\r\n",
        ":16S:TRAN\r\n",
        ":16S:FIN\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWDU", 5, opaque_values)).unwrap();
    let page = response.securities_transactions().unwrap().unwrap();
    assert_eq!(page.entries[0].transaction_kind(), Some("FICT"));
    assert_eq!(page.entries[0].is_reversal(), Some(false));
}

// Consumed MT536 structure remains strict: an unterminated TRANSDET block and
// a missing PAYM qualifier both fail as typed securities-data errors.
#[test]
fn malformed_mt536_consumed_structure_still_fails_without_partial_results() {
    let truncated_details = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":20C::SEME//NONREF\r\n",
        ":23G:NEWM\r\n",
        ":69A::STAT//20260701/20260728\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS05\r\n",
        "Fictional Security\r\n",
        ":16R:TRAN\r\n",
        ":16R:LINK\r\n",
        ":20C::RELA//NONREF\r\n",
        ":16S:LINK\r\n",
        ":16R:TRANSDET\r\n",
        ":36B::PSTA//UNIT/1,\r\n",
        ":22F::TRAN//SETT\r\n",
        ":22H::REDE//RECE\r\n",
        ":22H::PAYM//FREE\r\n",
        ":98A::ESET//20260728\r\n",
        ":16S:TRAN\r\n",
        ":16S:FIN\r\n",
        "-"
    )
    .as_bytes();
    let response = Response::parse(&message_with_binary("HIWDU", 5, truncated_details)).unwrap();
    assert!(matches!(
        response.securities_transactions(),
        Err(Error::MalformedSecuritiesData { .. })
    ));

    let missing_payment = concat!(
        "\r\n",
        ":16R:GENL\r\n",
        ":28E:1/ONLY\r\n",
        ":97A::SAFE//12345678/300001\r\n",
        ":17B::ACTI//Y\r\n",
        ":16S:GENL\r\n",
        ":16R:FIN\r\n",
        ":35B:ISIN DE000FINTS05\r\n",
        "Fictional Security\r\n",
        ":16R:TRAN\r\n",
        ":16R:LINK\r\n",
        ":20C::RELA//NONREF\r\n",
        ":16S:LINK\r\n",
        ":16R:TRANSDET\r\n",
        ":36B::PSTA//UNIT/1,\r\n",
        ":22F::TRAN//SETT\r\n",
        ":22H::REDE//RECE\r\n",
        ":98A::ESET//20260728\r\n",
        ":16S:TRANSDET\r\n",
        ":16S:TRAN\r\n",
        ":16S:FIN\r\n",
        "-"
    );
    let response =
        Response::parse(&message_with_binary("HIWDU", 5, missing_payment.as_bytes())).unwrap();
    assert!(matches!(
        response.securities_transactions(),
        Err(Error::MalformedSecuritiesData { .. })
    ));
}

// G112 / CR0538 C.12.1 and B.8 btgv: dates and supplied complete amounts stay
// typed. A partial optional Originalbetrag group is treated as absent because
// none of its incomplete values can be represented without fabrication.
#[test]
fn malformed_credit_card_entries_are_typed_errors() {
    let bad_date = message(&[
        "HIRMG:2:2+0010::accepted",
        concat!(
            "HIKKU:3:1:4+444433******1111+++++",
            "444433******1111:20261340:20260728:::::::9,00:EUR:D"
        ),
    ]);
    let response = Response::parse(&bad_date).unwrap();
    assert!(matches!(
        response.credit_card_transactions(),
        Err(Error::InvalidValue {
            field: "credit-card date"
        })
    ));

    let bad_amount = message(&[
        "HIRMG:2:2+0010::accepted",
        concat!(
            "HIKKU:3:1:4+444433******1111+++++",
            "444433******1111:20260727:20260728:::::::9.00:EUR:D"
        ),
    ]);
    let response = Response::parse(&bad_amount).unwrap();
    assert!(matches!(
        response.credit_card_transactions(),
        Err(Error::InvalidValue {
            field: "credit-card booking amount"
        })
    ));

    let partial_original_amount = message(&[
        "HIRMG:2:2+0010::accepted",
        concat!(
            "HIKKU:3:1:4+444433******1111+++++",
            "444433******1111:20260727:20260728:::10,00::::9,00:EUR:D"
        ),
    ]);
    let response = Response::parse(&partial_original_amount).unwrap();
    let page = response.credit_card_transactions().unwrap().unwrap();
    assert!(page.entries[0].original_amount().is_none());

    let malformed_complete_original = message(&[
        "HIRMG:2:2+0010::accepted",
        concat!(
            "HIKKU:3:1:4+444433******1111+++++",
            "444433******1111:20260727:20260728:::invalid:EUR:D::9,00:EUR:D"
        ),
    ]);
    let response = Response::parse(&malformed_complete_original).unwrap();
    assert!(matches!(
        response.credit_card_transactions(),
        Err(Error::InvalidValue {
            field: "credit-card original amount"
        })
    ));
}

// Owner-ratified CR0538 live shape: one all-empty final entry DEG is padding,
// and components after the 29 defined entry positions are meaning-neutral
// extensions. A second empty DEG or a partially filled DEG remains malformed.
#[test]
fn credit_card_entries_accept_one_empty_tail_and_unknown_components() {
    let entry = concat!(
        "444433******1111:20260727:20260728::20260728",
        ":10,00:USD:D:0,9:9,00:EUR:D",
        ":Purchase:at merchant:Second group:detail:::::DE",
        ":Fictional Merchant:TERM-1:N",
        ":BOOKREF-1:F001:07-2026:Cash fee:AEE:EXTENSION:MORE"
    );
    let accepted = message(&[
        "HIRMG:2:2+0010::accepted",
        &format!("HIKKU:3:1:4+444433******1111+++++{entry}+"),
    ]);
    assert_eq!(
        Response::parse(&accepted)
            .unwrap()
            .credit_card_transactions()
            .unwrap()
            .unwrap()
            .entries
            .len(),
        1
    );

    for tail in ["++", "+partially-filled"] {
        let fixture = message(&[
            "HIRMG:2:2+0010::accepted",
            &format!("HIKKU:3:1:4+444433******1111+++++{entry}{tail}"),
        ]);
        assert!(
            Response::parse(&fixture)
                .unwrap()
                .credit_card_transactions()
                .is_err()
        );
    }
}

// FinTS Messages 2022 C.4.3.1-C.4.3.2 and G112 C.12.1-C.12.2:
// operation versions and conditional credit-card inputs come only from BPD,
// while HIPINS independently supplies the TAN requirement.
#[test]
fn gate4_capabilities_are_negotiated_only_from_exact_parameter_versions() {
    let fixture = message(&[
        "HIRMG:2:2+0010::accepted",
        "HIBPA:3:3:2+7+280:12345678+Fictional Bank+9+1+300",
        "HIWPDS:4:6:2+1+1+0+J:J:J",
        "HIWDUS:5:5:2+1+1+0+90",
        "HIKKUS:6:1:2+1+1+0+90:J:J:J",
        "HIKKSS:7:1:2+1+1+0+J",
        concat!(
            "HIPINS:8:1:2+1+1+0+4:6:6:::HKWPD:N:HKWDU:J",
            ":HKKKU:N:HKKKS:J"
        ),
    ]);
    let response = Response::parse(&fixture).unwrap();
    let mut state = ReusableState::new();
    response.apply_parameters(&mut state).unwrap();

    assert!(state.depot_positions_supported);
    assert!(state.securities_transactions_supported);
    assert_eq!(state.depot_positions_requires_tan, Some(false));
    assert_eq!(state.securities_transactions_requires_tan, Some(true));
    let card = state.credit_card_transactions.unwrap();
    assert!(card.account_required);
    assert!(card.date_range_allowed);
    assert_eq!(state.credit_card_transactions_requires_tan, Some(false));
    assert_eq!(state.credit_card_balance_account_required, Some(true));
    assert_eq!(state.credit_card_balance_requires_tan, Some(true));
}
