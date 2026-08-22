use super::*;
use crate::{CreditDebit, model::TransactionFormat};
use chrono::NaiveDate;

const CAMT_DESCRIPTOR: &str = "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08";
const CAMT_DESCRIPTOR_WIRE: &str = "urn?:iso?:std?:iso?:20022?:tech?:xsd?:camt.052.001.08";

fn binary_message(prefix: &str, payload: &[u8], suffix: &str) -> Vec<u8> {
    let mut wire =
        b"HNHBK:1:3+000000000000+300+dialog1+2+dialog1:1'HIRMG:2:2+0010::accepted'".to_vec();
    wire.extend_from_slice(prefix.as_bytes());
    wire.extend_from_slice(payload.len().to_string().as_bytes());
    wire.push(b'@');
    wire.extend_from_slice(payload);
    wire.extend_from_slice(suffix.as_bytes());
    let length = format!("{:012}", wire.len());
    wire[10..22].copy_from_slice(length.as_bytes());
    wire
}

fn camt_message(xml: &[u8]) -> Vec<u8> {
    binary_message(
        &format!(
            "HICAZ:3:1:3+DE40123456780000123456::123456::280:12345678+{}+@",
            CAMT_DESCRIPTOR_WIRE
        ),
        xml,
        "'HNHBS:4:1+2'",
    )
}

fn camt_documents_message(documents: &[&[u8]]) -> Vec<u8> {
    let mut wire = format!(
        "HNHBK:1:3+000000000000+300+dialog1+2+dialog1:1'\
         HIRMG:2:2+0010::accepted'\
         HICAZ:3:1:3+DE40123456780000123456::123456::280:12345678+\
         {CAMT_DESCRIPTOR_WIRE}+"
    )
    .into_bytes();
    for (index, document) in documents.iter().enumerate() {
        if index > 0 {
            wire.push(b':');
        }
        wire.push(b'@');
        wire.extend_from_slice(document.len().to_string().as_bytes());
        wire.push(b'@');
        wire.extend_from_slice(document);
    }
    wire.extend_from_slice(b"'HNHBS:4:1+2'");
    let length = format!("{:012}", wire.len());
    wire[10..22].copy_from_slice(length.as_bytes());
    wire
}

// DK Anlage 3 v3.9, 7.1.2, 7.1.6, 7.1.7, and 7.2; FinTS Messages
// 2022-04-15 C.2.3.1.1.1; corrections G97 and G108. This independently
// written fixture covers two booking days without using captured data.
#[test]
fn camt_fixture_returns_only_booked_entries_and_preserves_references() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08">
  <BkToCstmrAcctRpt>
    <GrpHdr><MsgId>fictional-message</MsgId></GrpHdr>
    <Rpt>
      <Id>fictional-report</Id>
      <Acct><Id><IBAN>DE40123456780000123456</IBAN></Id><Ccy>EUR</Ccy></Acct>
      <Ntry>
        <NtryRef>fictional-entry</NtryRef>
        <Amt Ccy="EUR">12.30</Amt><CdtDbtInd>CRDT</CdtDbtInd>
        <RvslInd>true</RvslInd>
        <Sts><Cd>BOOK</Cd></Sts>
        <BookgDt><Dt>2026-07-28</Dt></BookgDt>
        <ValDt><Dt>2026-07-27</Dt></ValDt>
        <AcctSvcrRef>fictional-bank-reference</AcctSvcrRef>
        <BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>RCDT</Cd><SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn><Prtry><Cd>NTRF+166</Cd></Prtry></BkTxCd>
        <NtryDtls><TxDtls>
          <Refs>
            <InstrId>fictional-customer-reference</InstrId>
            <EndToEndId>fictional-end-to-end</EndToEndId>
          </Refs>
          <Amt Ccy="EUR">12.30</Amt><CdtDbtInd>CRDT</CdtDbtInd>
          <BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>RCDT</Cd><SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn><Prtry><Cd>NTRF+166</Cd></Prtry></BkTxCd>
          <RltdPties><Dbtr><Nm>Fictional &amp; Person</Nm></Dbtr><DbtrAcct><Id><IBAN>DE02123456780000987654</IBAN></Id></DbtrAcct></RltdPties>
          <RmtInf><Ustrd><![CDATA[Literal & and &amp;]]></Ustrd></RmtInf>
        </TxDtls></NtryDtls>
      </Ntry>
      <Ntry>
        <NtryRef>fictional-entry-two</NtryRef>
        <Amt Ccy="EUR">2.00</Amt><CdtDbtInd>DBIT</CdtDbtInd>
        <Sts><Cd>BOOK</Cd></Sts>
        <BookgDt><Dt>2026-07-27</Dt></BookgDt>
        <ValDt><Dt>2026-07-27</Dt></ValDt>
        <AcctSvcrRef>fictional-bank-reference-two</AcctSvcrRef>
        <BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>IDDT</Cd><SubFmlyCd>ESDD</SubFmlyCd></Fmly></Domn></BkTxCd>
        <NtryDtls><TxDtls>
          <Amt Ccy="EUR">2.00</Amt>
          <BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>IDDT</Cd><SubFmlyCd>ESDD</SubFmlyCd></Fmly></Domn></BkTxCd>
          <RltdPties><Cdtr><Nm>Fictional Creditor</Nm></Cdtr></RltdPties>
          <RmtInf><Ustrd>Fictional membership</Ustrd></RmtInf>
        </TxDtls></NtryDtls>
      </Ntry>
      <Ntry>
        <NtryRef>fictional-entry-three</NtryRef>
        <Amt Ccy="EUR">1.00</Amt><CdtDbtInd>CRDT</CdtDbtInd>
        <Sts><Cd>BOOK</Cd></Sts>
        <AcctSvcrRef>fictional-bank-reference-three</AcctSvcrRef>
        <BkTxCd/>
        <NtryDtls><TxDtls>
          <Amt Ccy="EUR">1.00</Amt>
          <BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>RCDT</Cd><SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn></BkTxCd>
          <RmtInf><Ustrd>Fictional date-free entry</Ustrd></RmtInf>
        </TxDtls></NtryDtls>
      </Ntry>
      <Ntry>
        <Amt Ccy="EUR">99.00</Amt><CdtDbtInd>DBIT</CdtDbtInd>
        <Sts><Cd>PDNG</Cd></Sts>
        <NtryDtls><TxDtls><RmtInf><Ustrd>Ignored pending detail</Ustrd></RmtInf></TxDtls></NtryDtls>
      </Ntry>
    </Rpt>
  </BkToCstmrAcctRpt>
</Document>"#;
    let response = Response::parse(&camt_message(xml)).unwrap();
    let page = response
        .transactions(&TransactionFormat::Camt {
            descriptor: CAMT_DESCRIPTOR.to_owned(),
        })
        .unwrap()
        .unwrap();

    assert_eq!(page.account.unwrap().iban(), Some("DE40123456780000123456"));
    assert_eq!(page.entries.len(), 3);
    let entry = &page.entries[0];
    assert_eq!(entry.amount().coefficient(), 1_230);
    assert_eq!(entry.amount().scale(), 2);
    assert_eq!(entry.direction(), CreditDebit::Credit);
    assert_eq!(entry.is_reversal(), Some(true));
    assert_eq!(entry.booking_date(), NaiveDate::from_ymd_opt(2026, 7, 28));
    assert_eq!(entry.bank_transaction_code(), Some("PMNT.RCDT.ESCT"));
    assert_eq!(entry.proprietary_transaction_code(), Some("NTRF+166"));
    assert_eq!(entry.entry_reference(), Some("fictional-entry"));
    assert_eq!(
        entry.account_servicer_reference(),
        Some("fictional-bank-reference")
    );
    assert_eq!(entry.details().len(), 1);
    assert_eq!(
        entry.details()[0].end_to_end_reference(),
        Some("fictional-end-to-end")
    );
    assert_eq!(
        entry.details()[0].bank_transaction_code(),
        Some("PMNT.RCDT.ESCT")
    );
    assert_eq!(
        entry.details()[0].counterparty_name(),
        Some("Fictional & Person")
    );
    assert_eq!(
        entry.details()[0].counterparty_account(),
        Some("DE02123456780000987654")
    );
    assert_eq!(
        entry.details()[0].remittance_information(),
        &["Literal & and &amp;"]
    );
    assert_eq!(
        page.entries[1].booking_date(),
        NaiveDate::from_ymd_opt(2026, 7, 27)
    );
    assert_eq!(page.entries[1].is_reversal(), None);
    assert_eq!(
        page.entries[1].details()[0].counterparty_name(),
        Some("Fictional Creditor")
    );
    // Anlage 3 v3.9, 7.1.8.5.1 and 7.2.6: entry-level BkTxCd and both dates
    // may be absent, while the transaction-detail BTC remains populated.
    assert_eq!(page.entries[2].booking_date(), None);
    assert_eq!(page.entries[2].value_date(), None);
    assert_eq!(page.entries[2].bank_transaction_code(), None);
    assert_eq!(
        page.entries[2].details()[0].bank_transaction_code(),
        Some("PMNT.RCDT.ESCT")
    );
}

// Anlage 3 v3.9 7.1.7 and the camt.052.001.08 schema make transaction-detail
// Amt and BkTxCd optional. Partial domain/family/subfamily groups cannot form a
// typed code, so they remain absent; the sum rule covers supplied detail amounts.
#[test]
fn camt_optional_detail_amounts_and_partial_transaction_codes_remain_absent() {
    let xml = br#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08">
<BkToCstmrAcctRpt><Rpt><Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct>
<Ntry><Amt Ccy="EUR">5.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts><Cd>BOOK</Cd></Sts>
<AcctSvcrRef>fictional-optional-details</AcctSvcrRef>
<BkTxCd><Domn><Cd>PMNT</Cd></Domn></BkTxCd>
<NtryDtls>
<TxDtls><Amt Ccy="EUR">5.00</Amt><BkTxCd><Domn><Cd>PMNT</Cd></Domn></BkTxCd></TxDtls>
<TxDtls><BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>RCDT</Cd></Fmly></Domn></BkTxCd>
<RmtInf><Ustrd>Fictional amount-free detail</Ustrd></RmtInf></TxDtls>
</NtryDtls></Ntry></Rpt></BkToCstmrAcctRpt></Document>"#;
    let response = Response::parse(&camt_message(xml)).unwrap();
    let page = response
        .transactions(&TransactionFormat::Camt {
            descriptor: CAMT_DESCRIPTOR.to_owned(),
        })
        .unwrap()
        .unwrap();
    let entry = &page.entries[0];

    assert!(entry.bank_transaction_code().is_none());
    assert_eq!(entry.details().len(), 2);
    assert_eq!(entry.details()[0].amount().unwrap().coefficient(), 500);
    assert!(entry.details()[0].bank_transaction_code().is_none());
    assert!(entry.details()[1].amount().is_none());
    assert!(entry.details()[1].bank_transaction_code().is_none());
}

// FinTS Messages 2022-04-15 Data Dictionary, HICAZ "Gebuchte camt-Umsätze":
// its binary DE repeats M n, with normally one camt.052 message per booking
// day. This independently written fixture proves both documents remain ordered.
#[test]
fn hicaz_repeated_binary_components_aggregate_booking_day_documents() {
    let first = br#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"><BkToCstmrAcctRpt><Rpt><Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct><Ntry><Amt Ccy="EUR">1.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts><Cd>BOOK</Cd></Sts><BookgDt><Dt>2026-07-27</Dt></BookgDt><AcctSvcrRef>fictional-day-one</AcctSvcrRef><BkTxCd/><NtryDtls><TxDtls><Amt Ccy="EUR">1.00</Amt><BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>RCDT</Cd><SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn></BkTxCd></TxDtls></NtryDtls></Ntry></Rpt></BkToCstmrAcctRpt></Document>"#;
    let second = br#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"><BkToCstmrAcctRpt><Rpt><Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct><Ntry><Amt Ccy="EUR">2.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts><Cd>BOOK</Cd></Sts><BookgDt><Dt>2026-07-28</Dt></BookgDt><AcctSvcrRef>fictional-day-two</AcctSvcrRef><BkTxCd/><NtryDtls><TxDtls><Amt Ccy="EUR">2.00</Amt><BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>IDDT</Cd><SubFmlyCd>ESDD</SubFmlyCd></Fmly></Domn></BkTxCd></TxDtls></NtryDtls></Ntry></Rpt></BkToCstmrAcctRpt></Document>"#;
    let response = Response::parse(&camt_documents_message(&[first, second])).unwrap();
    let page = response
        .transactions(&TransactionFormat::Camt {
            descriptor: CAMT_DESCRIPTOR.to_owned(),
        })
        .unwrap()
        .unwrap();

    assert_eq!(page.entries.len(), 2);
    assert_eq!(
        page.entries[0].account_servicer_reference(),
        Some("fictional-day-one")
    );
    assert_eq!(
        page.entries[1].account_servicer_reference(),
        Some("fictional-day-two")
    );
}

// quick-xml 0.41 default namespace cap and Gate 2 parse contract: malformed or
// DK Anlage 3 v3.x before v3.4 (camt.052.001.02): `Sts` is a plain code and
// parties carry `Nm` directly. The namespace must match the negotiated descriptor.
#[test]
fn camt_052_001_02_payload_parses_under_its_negotiated_descriptor() {
    const OLDER: &str = "urn:iso:std:iso:20022:tech:xsd:camt.052.001.02";
    const OLDER_WIRE: &str = "urn?:iso?:std?:iso?:20022?:tech?:xsd?:camt.052.001.02.xsd";
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Document xmlns="{OLDER}"><BkToCstmrAcctRpt><GrpHdr><MsgId>fictional</MsgId></GrpHdr><Rpt><Id>1</Id><Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct><Ntry><Amt Ccy="EUR">12.34</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>BOOK</Sts><BookgDt><Dt>2026-08-03</Dt></BookgDt><ValDt><Dt>2026-08-03</Dt></ValDt><AcctSvcrRef>fictional-02</AcctSvcrRef><BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>ICDT</Cd><SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn></BkTxCd><NtryDtls><TxDtls><Refs><EndToEndId>fictional-e2e</EndToEndId></Refs><RltdPties><Cdtr><Nm>Fictional Creditor</Nm></Cdtr><CdtrAcct><Id><IBAN>DE02120300000000202051</IBAN></Id></CdtrAcct></RltdPties><RmtInf><Ustrd>Fictional purpose</Ustrd></RmtInf></TxDtls></NtryDtls></Ntry><Ntry><Amt Ccy="EUR">1.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts>PDNG</Sts><AcctSvcrRef>fictional-pending</AcctSvcrRef><BkTxCd/></Ntry></Rpt></BkToCstmrAcctRpt></Document>"#
    );
    let wire = binary_message(
        &format!("HICAZ:3:1:3+DE40123456780000123456::123456::280:12345678+{OLDER_WIRE}+@"),
        xml.as_bytes(),
        "'HNHBS:4:1+2'",
    );

    let page = Response::parse(&wire)
        .unwrap()
        .transactions(&TransactionFormat::Camt {
            descriptor: OLDER.to_owned(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    let entry = &page.entries[0];
    assert_eq!(entry.amount().coefficient(), 1234);
    assert_eq!(entry.amount().scale(), 2);
    assert_eq!(entry.direction(), CreditDebit::Debit);
    assert_eq!(entry.booking_date(), NaiveDate::from_ymd_opt(2026, 8, 3));
    assert_eq!(entry.account_servicer_reference(), Some("fictional-02"));
    let detail = entry.details().first().unwrap();
    assert_eq!(detail.counterparty_name(), Some("Fictional Creditor"));
    assert_eq!(detail.end_to_end_reference(), Some("fictional-e2e"));

    // The 001.08 descriptor does not authorize a 001.02 document, and vice versa.
    assert!(matches!(
        Response::parse(&wire)
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::InvalidValue {
            field: "HICAZ camt descriptor"
        })
    ));
    let mismatched = binary_message(
        &format!(
            "HICAZ:3:1:3+DE40123456780000123456::123456::280:12345678+{CAMT_DESCRIPTOR_WIRE}+@"
        ),
        xml.as_bytes(),
        "'HNHBS:4:1+2'",
    );
    assert!(matches!(
        Response::parse(&mismatched)
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::Unsupported(crate::Limitation::CamtNamespace))
    ));
}

// cap-exceeding XML fails as one redacted typed error, never a partial result.
#[test]
fn camt_namespace_and_declaration_limits_fail_without_partial_results() {
    let wrong_namespace =
        br#"<Document xmlns="urn:fictional:wrong"><BkToCstmrAcctRpt/></Document>"#;
    assert!(matches!(
        Response::parse(&camt_message(wrong_namespace))
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::Unsupported(crate::Limitation::CamtNamespace))
    ));

    let foreign_structure = format!(
        "<Document xmlns=\"{CAMT_DESCRIPTOR}\"><BkToCstmrAcctRpt>\
         <Rpt xmlns=\"urn:fictional:misplaced\"/></BkToCstmrAcctRpt></Document>"
    );
    assert!(matches!(
        Response::parse(&camt_message(foreign_structure.as_bytes()))
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::MalformedTransactionData { .. })
    ));

    let declarations = (0..257)
        .map(|index| format!(" xmlns:p{index}=\"urn:fictional:{index}\""))
        .collect::<String>();
    let excessive = format!(
        "<Document xmlns=\"{CAMT_DESCRIPTOR}\"{declarations}><BkToCstmrAcctRpt/></Document>"
    );
    assert!(matches!(
        Response::parse(&camt_message(excessive.as_bytes()))
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::MalformedTransactionData { .. })
    ));

    let wrong_encoding = br#"<?xml version="1.0" encoding="ISO-8859-1"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08">
  <BkToCstmrAcctRpt/>
</Document>"#;
    assert!(matches!(
        Response::parse(&camt_message(wrong_encoding))
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::MalformedTransactionData { .. })
    ));
}

// Anlage 3 v3.9 chapter 7 and ISO 20022 lexical rules: xs:decimal accepts
// leading plus and one empty side of the decimal point; xs:date permits a
// timezone; SplmtryData/Envlp may contain a foreign-namespace subtree.
#[test]
fn camt_accepts_permitted_lexical_and_supplementary_shapes() {
    for (label, amount, date, supplementary) in [
        ("leading plus", "+5.", "2026-07-29", ""),
        ("leading decimal point", ".5", "2026-07-29", ""),
        ("date timezone", "5.00", "2026-07-29+02:00", ""),
        (
            "foreign supplementary subtree",
            "5.00",
            "2026-07-29",
            "<SplmtryData><Envlp xmlns=\"urn:fictional:extension\"><Any><Deep/></Any></Envlp></SplmtryData>",
        ),
    ] {
        let xml = format!(
            "<Document xmlns=\"{CAMT_DESCRIPTOR}\"><BkToCstmrAcctRpt><Rpt>\
             <Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct>\
             {supplementary}\
             <Ntry><Amt Ccy=\"EUR\">{amount}</Amt><CdtDbtInd>CRDT</CdtDbtInd>\
             <Sts><Cd>BOOK</Cd></Sts><BookgDt><Dt>{date}</Dt></BookgDt>\
             <AcctSvcrRef>fictional-lexical</AcctSvcrRef>\
             <BkTxCd/><NtryDtls><TxDtls><Amt Ccy=\"EUR\">{amount}</Amt>\
             <BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>RCDT</Cd>\
             <SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn></BkTxCd>\
             </TxDtls></NtryDtls></Ntry></Rpt></BkToCstmrAcctRpt></Document>"
        );
        let result = Response::parse(&camt_message(xml.as_bytes()))
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: format!("{CAMT_DESCRIPTOR}.XSD").to_uppercase(),
            });
        assert!(result.is_ok(), "{label}");
        let page = result.unwrap().unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(
            page.entries[0].booking_date(),
            NaiveDate::from_ymd_opt(2026, 7, 29)
        );
    }

    for (amount, date) in [("+.", "2026-07-29"), ("1.00", "2026-07-29+14:01")] {
        let malformed = format!(
            "<Document xmlns=\"{CAMT_DESCRIPTOR}\"><BkToCstmrAcctRpt><Rpt>\
             <Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct>\
             <Ntry><Amt Ccy=\"EUR\">{amount}</Amt><CdtDbtInd>CRDT</CdtDbtInd>\
             <Sts><Cd>BOOK</Cd></Sts><BookgDt><Dt>{date}</Dt></BookgDt>\
             <AcctSvcrRef>fictional-malformed</AcctSvcrRef><BkTxCd/>\
             <NtryDtls><TxDtls><Amt Ccy=\"EUR\">{amount}</Amt>\
             <BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>RCDT</Cd>\
             <SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn></BkTxCd>\
             </TxDtls></NtryDtls></Ntry></Rpt></BkToCstmrAcctRpt></Document>"
        );
        assert!(matches!(
            Response::parse(&camt_message(malformed.as_bytes()))
                .unwrap()
                .transactions(&TransactionFormat::Camt {
                    descriptor: CAMT_DESCRIPTOR.to_owned()
                }),
            Err(Error::MalformedTransactionData { .. })
        ));
    }

    let mismatched = binary_message(
        "HICAZ:3:1:3+DE40123456780000123456::123456::280:12345678+\
         urn?:iso?:std?:iso?:20022?:tech?:xsd?:camt.052.001.09+@",
        b"not parsed after descriptor mismatch",
        "'HNHBS:4:1+2'",
    );
    assert!(matches!(
        Response::parse(&mismatched)
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::InvalidValue {
            field: "HICAZ camt descriptor"
        })
    ));
}

// Anlage 3 v3.9 chapter 7 retains the DK single-Rpt restriction. A second
// otherwise well-placed Rpt has a dedicated redacted error; zero reports is malformed.
#[test]
fn camt_multiple_reports_have_a_distinct_typed_error() {
    let multiple = format!(
        "<Document xmlns=\"{CAMT_DESCRIPTOR}\"><BkToCstmrAcctRpt>\
         <Rpt><Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct></Rpt>\
         <Rpt></Rpt>\
         </BkToCstmrAcctRpt></Document>"
    );
    assert!(matches!(
        Response::parse(&camt_message(multiple.as_bytes()))
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::MultipleCamtReports)
    ));
    let none = format!(
        "<Document xmlns=\"{CAMT_DESCRIPTOR}\"><BkToCstmrAcctRpt/>\
         </Document>"
    );
    assert!(matches!(
        Response::parse(&camt_message(none.as_bytes()))
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::MalformedTransactionData { .. })
    ));
}

// DK Anlage 3 v3.9, 7.1.1, 7.1.6, 7.1.7, 7.4, and 7.5 require a
// structurally complete document, matching entry/detail sums, and at least one
// TxDtls per booked entry. Malformed documents never yield partial entries.
#[test]
fn malformed_camt_documents_fail_without_partial_results() {
    let truncated =
        br#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"><BkToCstmrAcctRpt>"#;
    let sum_mismatch = br#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"><BkToCstmrAcctRpt><Rpt><Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct><Ntry><Amt Ccy="EUR">1.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts><Cd>BOOK</Cd></Sts><AcctSvcrRef>fictional-sum</AcctSvcrRef><BkTxCd/><NtryDtls><TxDtls><Amt Ccy="EUR">2.00</Amt><BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>RCDT</Cd><SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn></BkTxCd></TxDtls></NtryDtls></Ntry></Rpt></BkToCstmrAcctRpt></Document>"#;
    let misplaced_entry = br#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"><BkToCstmrAcctRpt><Ntry/></BkToCstmrAcctRpt></Document>"#;
    let missing_details = br#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"><BkToCstmrAcctRpt><Rpt><Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct><Ntry><Amt Ccy="EUR">1.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts><Cd>BOOK</Cd></Sts><AcctSvcrRef>fictional-no-details</AcctSvcrRef><BkTxCd/></Ntry></Rpt></BkToCstmrAcctRpt></Document>"#;
    let direction_mismatch = br#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"><BkToCstmrAcctRpt><Rpt><Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct><Ntry><Amt Ccy="EUR">1.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts><Cd>BOOK</Cd></Sts><AcctSvcrRef>fictional-direction</AcctSvcrRef><BkTxCd/><NtryDtls><TxDtls><Amt Ccy="EUR">1.00</Amt><CdtDbtInd>DBIT</CdtDbtInd></TxDtls></NtryDtls></Ntry></Rpt></BkToCstmrAcctRpt></Document>"#;

    let documents: &[&[u8]] = &[
        truncated,
        sum_mismatch,
        misplaced_entry,
        missing_details,
        direction_mismatch,
    ];
    let mut sites = Vec::new();
    for document in documents {
        let result = Response::parse(&camt_message(document))
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned(),
            });
        let site = match result {
            Err(Error::MalformedTransactionData { site }) => site,
            _ => panic!("expected malformed camt transaction data"),
        };
        assert!(site.starts_with("fints::response::transactions::camt:"));
        sites.push(site);
    }
    assert!(
        sites.windows(2).any(|pair| pair[0] != pair[1]),
        "distinct camt rejection sites must remain distinguishable"
    );

    assert!(matches!(
        Response::parse(&camt_message(truncated))
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            }),
        Err(Error::MalformedTransactionData { .. })
    ));
}

// Formals cut rules allow trailing optional DEG components to be omitted, and
// unknown trailing components are read past. The first descriptor/payload
// component remains the only value consumed by these operation layouts.
#[test]
fn transaction_segment_groups_accept_unused_trailing_components() {
    let xml = br#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"><BkToCstmrAcctRpt><Rpt><Acct><Id><IBAN>DE40123456780000123456</IBAN></Id></Acct><Ntry><Amt Ccy="EUR">1.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts><Cd>BOOK</Cd></Sts><AcctSvcrRef>fictional-extension</AcctSvcrRef><BkTxCd/><NtryDtls><TxDtls><Amt Ccy="EUR">1.00</Amt></TxDtls></NtryDtls></Ntry></Rpt></BkToCstmrAcctRpt></Document>"#;
    let camt = binary_message(
        &format!(
            "HICAZ:3:1:3+DE40123456780000123456::123456::280:12345678+\
             {CAMT_DESCRIPTOR_WIRE}:IGNORED+@"
        ),
        xml,
        "'HNHBS:4:1+2'",
    );
    assert_eq!(
        Response::parse(&camt)
            .unwrap()
            .transactions(&TransactionFormat::Camt {
                descriptor: CAMT_DESCRIPTOR.to_owned()
            })
            .unwrap()
            .unwrap()
            .entries
            .len(),
        1
    );

    let mt940 = concat!(
        "\r\n:60F:C260727EUR100,00",
        "\r\n:61:260728C1,00NTRFFICTREF",
        "\r\n-"
    );
    let legacy = binary_message("HIKAZ:3:7:3+@", mt940.as_bytes(), ":IGNORED'HNHBS:4:1+2'");
    assert_eq!(
        Response::parse(&legacy)
            .unwrap()
            .transactions(&TransactionFormat::Mt940 { version: 7 })
            .unwrap()
            .unwrap()
            .entries
            .len(),
        1
    );
}

// DK Anlage 3 v3.8, 8.1 and 8.2.1-8.2.5; FinTS Messages 2022-04-15
// C.2.1.1.1.2. The fixture contains no real identifiers or captured values.
#[test]
fn mt940_fixture_preserves_bank_reference_or_exact_statement_position() {
    let mt940 = concat!(
        "\r\n:20:FICTIONAL1",
        "\r\n:25:12345678/123456",
        "\r\n:28C:7/2",
        "\r\n:60F:C260727EUR100,00",
        "\r\n:61:2607280728C12,30NTRFFICTREF//FICTBANKREF",
        "\r\n:86:166?20EREF+FICTIONAL-END?21KREF+FICTIONAL-CUSTOMER?22SVWZ+Invoice 42?31123456789?32Fictional Person",
        "\r\n:61:260728D2,00NDDTNONREF",
        "\r\n:86:105?20MREF+FICTIONAL-MANDATE?21SVWZ+Membership",
        "\r\n:62F:C260728EUR110,30",
        "\r\n-"
    )
    .as_bytes();
    let response =
        Response::parse(&binary_message("HIKAZ:3:7:3+@", mt940, "'HNHBS:4:1+2'")).unwrap();
    let page = response
        .transactions(&TransactionFormat::Mt940 { version: 7 })
        .unwrap()
        .unwrap();

    assert_eq!(page.entries.len(), 2);
    assert_eq!(
        page.entries[0].account_servicer_reference(),
        Some("FICTBANKREF")
    );
    assert!(page.entries[0].statement_position().is_none());
    assert_eq!(
        page.entries[0].details()[0].end_to_end_reference(),
        Some("FICTIONAL-END")
    );
    assert_eq!(
        page.entries[0].details()[0].counterparty_account(),
        Some("123456789")
    );
    let position = page.entries[1].statement_position().unwrap();
    assert_eq!(position.statement_number(), "7");
    assert_eq!(position.page_number(), Some("2"));
    assert_eq!(position.entry_index(), 2);
    assert!(page.entries[1].account_servicer_reference().is_none());
}

// DK Anlage 3 v3.8, 8.2.1-8.2.3 specifies :20:, :25:, :28C:, closing
// balances, and 16-character field-61 references. Gate 2 consumes only the
// opening-balance currency and the references themselves; unavailable or
// malformed statement numbering merely makes StatementPosition absent.
#[test]
fn mt940_unconsumed_statement_fields_and_long_references_are_nonfatal() {
    let mt940 = concat!(
        "\r\n:28C:not/a/number",
        "\r\n:60F:C260727EUR100,00",
        "\r\n:61:260728C1,00NTRFFICTIONAL-CUSTOMER-REFERENCE//FICTIONAL-BANK-REFERENCE-LONG",
        "\r\n:61:260728D2,00NDDTFICTIONAL-CUSTOMER-ONLY-LONG",
        "\r\n-"
    );
    let response = Response::parse(&binary_message(
        "HIKAZ:3:7:3+@",
        mt940.as_bytes(),
        "'HNHBS:4:1+2'",
    ))
    .unwrap();
    let page = response
        .transactions(&TransactionFormat::Mt940 { version: 7 })
        .unwrap()
        .unwrap();

    assert_eq!(page.entries.len(), 2);
    assert_eq!(
        page.entries[0].account_servicer_reference(),
        Some("FICTIONAL-BANK-REFERENCE-LONG")
    );
    assert_eq!(
        page.entries[1].details()[0].customer_reference(),
        Some("FICTIONAL-CUSTOMER-ONLY-LONG")
    );
    assert!(page.entries[0].statement_position().is_none());
    assert!(page.entries[1].statement_position().is_none());
}

// DK Anlage 3 v3.8, 8.1 and 8.2.1: blank separator lines are ignorable,
// the opening-balance date is n..6, and exactly one trailing CRLF after the
// SWIFT terminator is owner-ratified. A second trailing CRLF remains malformed.
#[test]
fn mt940_accepts_variable_opening_date_blank_lines_and_one_trailing_crlf() {
    let payload = concat!(
        "\r\n",
        ":20:FICTIONAL1\r\n",
        "\r\n",
        ":25:12345678/123456\r\n",
        ":28C:1\r\n",
        ":60F:C1EUR1,00\r\n",
        ":61:260729C1,00NTRFNONREF\r\n",
        ":62F:C1EUR2,00\r\n",
        "-\r\n"
    );
    let response = Response::parse(&binary_message(
        "HIKAZ:3:7:3+@",
        payload.as_bytes(),
        "'HNHBS:4:1+2'",
    ))
    .unwrap();
    assert_eq!(
        response
            .transactions(&TransactionFormat::Mt940 { version: 7 })
            .unwrap()
            .unwrap()
            .entries
            .len(),
        1
    );

    let malformed = format!("{payload}\r\n");
    let response = Response::parse(&binary_message(
        "HIKAZ:3:7:3+@",
        malformed.as_bytes(),
        "'HNHBS:4:1+2'",
    ))
    .unwrap();
    assert!(matches!(
        response.transactions(&TransactionFormat::Mt940 { version: 7 }),
        Err(Error::MalformedTransactionData { .. })
    ));
}

// DK Anlage 3 v3.8, 8.2.4 permits unstructured :86: information. A leading
// three-digit transaction code does not imply the presence of ?nn subfields.
#[test]
fn mt940_numeric_prefix_without_control_fields_stays_unstructured() {
    let mt940 = concat!(
        "\r\n:20:FICTIONAL1\r\n:25:12345678/123456\r\n:28C:1/1",
        "\r\n:60F:C260727EUR100,00\r\n:61:260728C1,00NTRFFICTREF",
        "\r\n:86:105Fictional unstructured detail",
        "\r\n:62F:C260728EUR101,00\r\n-"
    );
    let response = Response::parse(&binary_message(
        "HIKAZ:3:7:3+@",
        mt940.as_bytes(),
        "'HNHBS:4:1+2'",
    ))
    .unwrap();
    let page = response
        .transactions(&TransactionFormat::Mt940 { version: 7 })
        .unwrap()
        .unwrap();

    assert_eq!(
        page.entries[0].details()[0].remittance_information(),
        &["105Fictional unstructured detail"]
    );
}

// DK Anlage 3 v3.8, 8.1 and 8.2.1-8.2.4 require a complete SWIFT
// terminator; Gate 2 still requires an opening balance for its consumed
// currency, a valid :61:, and a decimal amount.
// Every malformed fixture is independently framed as a valid HIKAZ response.
#[test]
fn malformed_mt940_statements_fail_without_partial_results() {
    let truncated = concat!(
        "\r\n:20:FICTIONAL1\r\n:25:12345678/123456\r\n:28C:1/1",
        "\r\n:60F:C260727EUR100,00\r\n:61:260728C1,00NTRFFICTREF",
        "\r\n:62F:C260728EUR101,00"
    );
    let missing_opening_balance = concat!(
        "\r\n:20:FICTIONAL1\r\n:25:12345678/123456\r\n:28C:1/1",
        "\r\n:61:260728C1,00NTRFFICTREF\r\n:62F:C260728EUR101,00\r\n-"
    );
    let malformed_entry = concat!(
        "\r\n:20:FICTIONAL1\r\n:25:12345678/123456\r\n:28C:1/1",
        "\r\n:60F:C260727EUR100,00\r\n:61:260728X1,00NTRFFICTREF",
        "\r\n:62F:C260728EUR101,00\r\n-"
    );
    let bad_amount = concat!(
        "\r\n:20:FICTIONAL1\r\n:25:12345678/123456\r\n:28C:1/1",
        "\r\n:60F:C260727EUR100,00\r\n:61:260728C1,XNTRFFICTREF",
        "\r\n:62F:C260728EUR101,00\r\n-"
    );

    for statement in [
        truncated,
        missing_opening_balance,
        malformed_entry,
        bad_amount,
    ] {
        assert!(matches!(
            Response::parse(&binary_message(
                "HIKAZ:3:7:3+@",
                statement.as_bytes(),
                "'HNHBS:4:1+2'"
            ))
            .unwrap()
            .transactions(&TransactionFormat::Mt940 { version: 7 }),
            Err(Error::MalformedTransactionData { .. })
        ));
    }
}
