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
          <RmtInf><Ustrd>Fictional invoice 42</Ustrd></RmtInf>
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
    assert_eq!(page.entries.len(), 2);
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
        page.entries[1].booking_date(),
        NaiveDate::from_ymd_opt(2026, 7, 27)
    );
    assert_eq!(page.entries[1].is_reversal(), None);
    assert_eq!(
        page.entries[1].details()[0].counterparty_name(),
        Some("Fictional Creditor")
    );
}

// quick-xml 0.41 default namespace cap and Gate 2 parse contract: malformed or
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
        Err(Error::MalformedTransactionData)
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
        Err(Error::MalformedTransactionData)
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
        Err(Error::MalformedTransactionData)
    ));
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
        "\r\n:86:166?20EREF+FICTIONAL-END?21SVWZ+Invoice 42?31DE02123456780000987654?32Fictional Person",
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
    let position = page.entries[1].statement_position().unwrap();
    assert_eq!(position.statement_number(), "7");
    assert_eq!(position.page_number(), Some("2"));
    assert_eq!(position.entry_index(), 2);
    assert!(page.entries[1].account_servicer_reference().is_none());
}
