use chrono::NaiveDate;
use quick_xml::{
    XmlVersion,
    events::Event,
    name::{Namespace, ResolveResult},
    reader::NsReader,
};

use crate::{
    error::{Error, malformed_transaction_data},
    model::{Amount, BookedEntry, BookedTransactionDetail, CreditDebit},
};

use super::MAX_TRANSACTION_PAGE_ENTRIES;

const MAX_XML_DEPTH: usize = 64;
const MAX_XML_NODES: usize = 20_000;

pub(super) struct CamtPayload {
    pub(super) iban: Option<String>,
    pub(super) other_account_id: Option<String>,
    pub(super) entries: Vec<BookedEntry>,
}

/// Enforces the node-count and nesting-depth bounds of one camt document.
///
/// Every accepted element passes through these checks, so the limits hold
/// regardless of which parsing branch consumes the event.
struct XmlBounds {
    nodes: usize,
}

impl XmlBounds {
    fn new() -> Self {
        Self { nodes: 0 }
    }

    /// Counts one element start or empty element against [`MAX_XML_NODES`].
    fn count_node(&mut self) -> Result<(), Error> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .filter(|count| *count <= MAX_XML_NODES)
            .ok_or(malformed_transaction_data!())?;
        Ok(())
    }

    /// Rejects a child of an element at `parent_depth` that would exceed
    /// [`MAX_XML_DEPTH`].
    fn check_depth(&self, parent_depth: usize) -> Result<(), Error> {
        if parent_depth >= MAX_XML_DEPTH {
            return Err(malformed_transaction_data!());
        }
        Ok(())
    }
}

/// Parses one booked camt.052 document whose bound namespace must equal the
/// negotiated `namespace` (the HICAZ descriptor without its `.xsd` suffix).
pub(super) fn parse(input: &[u8], namespace: &str) -> Result<CamtPayload, Error> {
    let expected_namespace = namespace.as_bytes();
    let mut reader = NsReader::from_reader(input);
    let mut stack = Vec::new();
    let mut state = CamtState::default();
    let mut bounds = XmlBounds::new();
    let mut saw_declaration = false;
    let mut supplementary_depth = 0_usize;

    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| malformed_transaction_data!())?;
        if supplementary_depth > 0 {
            match event {
                Event::Start(_) => {
                    bounds.count_node()?;
                    bounds.check_depth(supplementary_depth)?;
                    supplementary_depth += 1;
                }
                Event::Empty(_) => bounds.count_node()?,
                Event::End(_) => supplementary_depth -= 1,
                Event::DocType(_) | Event::Eof => return Err(malformed_transaction_data!()),
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(start) => {
                let document_root = !state.saw_document
                    && stack.is_empty()
                    && start.local_name().as_ref() == b"Document";
                require_camt_namespace(namespace, expected_namespace, document_root)?;
                bounds.count_node()?;
                bounds.check_depth(stack.len())?;
                let local = std::str::from_utf8(start.local_name().as_ref())
                    .map_err(|_| malformed_transaction_data!())?
                    .to_owned();
                if local == "SplmtryData" {
                    // ISO 20022 permits an arbitrary-namespace Envlp below
                    // SplmtryData. It has no Gate 2 result semantics.
                    supplementary_depth = 1;
                    continue;
                }
                let currency = if local == "Amt" {
                    amount_currency(&start)?
                } else {
                    None
                };
                if local == "Document" {
                    if !stack.is_empty() || state.saw_document {
                        return Err(malformed_transaction_data!());
                    }
                    state.saw_document = true;
                } else if local == "BkToCstmrAcctRpt" {
                    if !ends_with(&stack, &["Document"]) || state.saw_message {
                        return Err(malformed_transaction_data!());
                    }
                    state.saw_message = true;
                } else if local == "Rpt" {
                    if !ends_with(&stack, &["BkToCstmrAcctRpt"]) {
                        return Err(malformed_transaction_data!());
                    }
                    state.reports = state
                        .reports
                        .checked_add(1)
                        .ok_or(malformed_transaction_data!())?;
                } else if local == "Ntry" {
                    if !ends_with(&stack, &["Rpt"]) || state.entry.is_some() {
                        return Err(malformed_transaction_data!());
                    }
                    state.entry = Some(EntryBuilder::default());
                } else if local == "TxDtls" {
                    if !ends_with(&stack, &["Ntry", "NtryDtls"]) || state.detail.is_some() {
                        return Err(malformed_transaction_data!());
                    }
                    state.detail = Some(DetailBuilder::default());
                }
                stack.push(Node {
                    local,
                    text: String::new(),
                    currency,
                });
            }
            Event::Empty(start) => {
                let document_root = !state.saw_document
                    && stack.is_empty()
                    && start.local_name().as_ref() == b"Document";
                require_camt_namespace(namespace, expected_namespace, document_root)?;
                bounds.count_node()?;
                if matches!(
                    start.local_name().as_ref(),
                    b"Document" | b"BkToCstmrAcctRpt" | b"Rpt" | b"Ntry" | b"TxDtls"
                ) {
                    return Err(malformed_transaction_data!());
                }
            }
            Event::Text(text) => {
                let text = text
                    .xml10_content()
                    .map_err(|_| malformed_transaction_data!())?;
                let text = quick_xml::escape::unescape(&text)
                    .map_err(|_| malformed_transaction_data!())?;
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&text);
                } else if !text.trim().is_empty() {
                    return Err(malformed_transaction_data!());
                }
            }
            Event::CData(text) => {
                let current = stack.last_mut().ok_or(malformed_transaction_data!())?;
                // CDATA is already literal character data; entity-looking text must
                // not pass through the normal text-node unescaper.
                current
                    .text
                    .push_str(&text.decode().map_err(|_| malformed_transaction_data!())?);
            }
            Event::GeneralRef(reference) => {
                let current = stack.last_mut().ok_or(malformed_transaction_data!())?;
                let reference = reference
                    .decode()
                    .map_err(|_| malformed_transaction_data!())?;
                let escaped = format!("&{reference};");
                current.text.push_str(
                    &quick_xml::escape::unescape(&escaped)
                        .map_err(|_| malformed_transaction_data!())?,
                );
            }
            Event::End(end) => {
                require_camt_namespace(namespace, expected_namespace, false)?;
                let local = std::str::from_utf8(end.local_name().as_ref())
                    .map_err(|_| malformed_transaction_data!())?
                    .to_owned();
                let node = stack.pop().ok_or(malformed_transaction_data!())?;
                if node.local != local {
                    return Err(malformed_transaction_data!());
                }
                let text = node.text.trim().to_owned();
                state.finish_node(&stack, node.local, text, node.currency)?;
                if state.entries.len() > MAX_TRANSACTION_PAGE_ENTRIES {
                    return Err(malformed_transaction_data!());
                }
            }
            Event::Decl(declaration) => {
                if saw_declaration || state.saw_document {
                    return Err(malformed_transaction_data!());
                }
                saw_declaration = true;
                if let Some(encoding) = declaration.encoding() {
                    let encoding = encoding.map_err(|_| malformed_transaction_data!())?;
                    if !encoding.eq_ignore_ascii_case(b"UTF-8") {
                        return Err(malformed_transaction_data!());
                    }
                }
            }
            Event::Comment(_) | Event::PI(_) => {}
            Event::DocType(_) => return Err(malformed_transaction_data!()),
            Event::Eof => break,
        }
    }

    if !stack.is_empty()
        || !state.saw_document
        || !state.saw_message
        || state.entry.is_some()
        || state.detail.is_some()
    {
        return Err(malformed_transaction_data!());
    }
    if state.reports > 1 {
        return Err(Error::MultipleCamtReports);
    }
    if state.reports == 0 {
        return Err(malformed_transaction_data!());
    }
    if state.iban.is_none() && state.other_account_id.is_none() {
        return Err(malformed_transaction_data!());
    }
    Ok(CamtPayload {
        iban: state.iban,
        other_account_id: state.other_account_id,
        entries: state.entries,
    })
}

fn require_camt_namespace(
    namespace: ResolveResult<'_>,
    expected: &[u8],
    document_root: bool,
) -> Result<(), Error> {
    match namespace {
        ResolveResult::Bound(Namespace(value)) if value.eq_ignore_ascii_case(expected) => Ok(()),
        ResolveResult::Bound(_) if document_root => Err(crate::Limitation::CamtNamespace.into()),
        ResolveResult::Bound(_) => Err(malformed_transaction_data!()),
        ResolveResult::Unbound | ResolveResult::Unknown(_) => Err(malformed_transaction_data!()),
    }
}

fn amount_currency(start: &quick_xml::events::BytesStart<'_>) -> Result<Option<String>, Error> {
    let mut currency = None;
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute.map_err(|_| malformed_transaction_data!())?;
        if attribute.key.as_ref() == b"Ccy" {
            if currency.is_some() {
                return Err(malformed_transaction_data!());
            }
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|_| malformed_transaction_data!())?
                .into_owned();
            validate_currency(&value)?;
            currency = Some(value);
        }
    }
    Ok(currency)
}

struct Node {
    local: String,
    text: String,
    currency: Option<String>,
}

#[derive(Default)]
struct CamtState {
    saw_document: bool,
    saw_message: bool,
    reports: usize,
    entry: Option<EntryBuilder>,
    detail: Option<DetailBuilder>,
    iban: Option<String>,
    other_account_id: Option<String>,
    entries: Vec<BookedEntry>,
}

impl CamtState {
    fn finish_node(
        &mut self,
        ancestors: &[Node],
        local: String,
        text: String,
        currency: Option<String>,
    ) -> Result<(), Error> {
        if local == "TxDtls" {
            let entry_direction = self
                .entry
                .as_ref()
                .ok_or(malformed_transaction_data!())?
                .direction;
            let detail = self
                .detail
                .take()
                .ok_or(malformed_transaction_data!())?
                .finish(entry_direction)?;
            self.entry
                .as_mut()
                .ok_or(malformed_transaction_data!())?
                .details
                .push(detail);
            return Ok(());
        }
        if local == "Ntry" {
            let entry = self
                .entry
                .take()
                .ok_or(malformed_transaction_data!())?
                .finish()?;
            if let Some(entry) = entry {
                self.entries.push(entry);
            }
            return Ok(());
        }
        if let Some(detail) = &mut self.detail {
            detail.apply(ancestors, &local, text, currency)?;
        } else if let Some(entry) = &mut self.entry {
            entry.apply(ancestors, &local, text, currency)?;
        } else if local == "IBAN" && ends_with(ancestors, &["Rpt", "Acct", "Id"]) {
            set_optional_once(&mut self.iban, text)?;
        } else if local == "Id" && ends_with(ancestors, &["Rpt", "Acct", "Id", "Othr"]) {
            set_optional_once(&mut self.other_account_id, text)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct EntryBuilder {
    amount: Option<Amount>,
    direction: Option<CreditDebit>,
    status: Option<String>,
    booking_date: Option<NaiveDate>,
    value_date: Option<NaiveDate>,
    reversal: Option<bool>,
    entry_reference: Option<String>,
    account_servicer_reference: Option<String>,
    bank_transaction_domain: Option<String>,
    bank_transaction_family: Option<String>,
    bank_transaction_subfamily: Option<String>,
    proprietary_transaction_code: Option<String>,
    details: Vec<BookedTransactionDetail>,
}

impl EntryBuilder {
    fn apply(
        &mut self,
        ancestors: &[Node],
        local: &str,
        text: String,
        currency: Option<String>,
    ) -> Result<(), Error> {
        if local == "Amt" && ends_with(ancestors, &["Ntry"]) {
            set_once(
                &mut self.amount,
                parse_amount(&text, required_currency(currency)?)?,
            )?;
        } else if local == "CdtDbtInd" && ends_with(ancestors, &["Ntry"]) {
            set_once(&mut self.direction, parse_direction(&text)?)?;
        } else if (local == "Sts" && !text.is_empty() && ends_with(ancestors, &["Ntry"]))
            || (local == "Cd" && ends_with(ancestors, &["Ntry", "Sts"]))
        {
            set_once(&mut self.status, text)?;
        } else if local == "Dt" && ends_with(ancestors, &["Ntry", "BookgDt"]) {
            set_once(&mut self.booking_date, parse_date(&text)?)?;
        } else if local == "DtTm" && ends_with(ancestors, &["Ntry", "BookgDt"]) {
            set_once(&mut self.booking_date, parse_date_time_date(&text)?)?;
        } else if local == "Dt" && ends_with(ancestors, &["Ntry", "ValDt"]) {
            set_once(&mut self.value_date, parse_date(&text)?)?;
        } else if local == "DtTm" && ends_with(ancestors, &["Ntry", "ValDt"]) {
            set_once(&mut self.value_date, parse_date_time_date(&text)?)?;
        } else if local == "RvslInd" && ends_with(ancestors, &["Ntry"]) {
            set_once(&mut self.reversal, parse_boolean(&text)?)?;
        } else if local == "NtryRef" && ends_with(ancestors, &["Ntry"]) {
            set_once(&mut self.entry_reference, nonempty(text)?)?;
        } else if local == "AcctSvcrRef" && ends_with(ancestors, &["Ntry"]) {
            set_once(&mut self.account_servicer_reference, nonempty(text)?)?;
        } else if local == "Cd" && ends_with(ancestors, &["Ntry", "BkTxCd", "Domn"]) {
            set_optional_once(&mut self.bank_transaction_domain, text)?;
        } else if local == "Cd" && ends_with(ancestors, &["Ntry", "BkTxCd", "Domn", "Fmly"]) {
            set_optional_once(&mut self.bank_transaction_family, text)?;
        } else if local == "SubFmlyCd" && ends_with(ancestors, &["Ntry", "BkTxCd", "Domn", "Fmly"])
        {
            set_optional_once(&mut self.bank_transaction_subfamily, text)?;
        } else if local == "Cd" && ends_with(ancestors, &["Ntry", "BkTxCd", "Prtry"]) {
            set_optional_once(&mut self.proprietary_transaction_code, text)?;
        }
        Ok(())
    }

    fn finish(self) -> Result<Option<BookedEntry>, Error> {
        let status = self.status.ok_or(malformed_transaction_data!())?;
        if status != "BOOK" {
            return Ok(None);
        }
        if self.details.is_empty() {
            return Err(malformed_transaction_data!());
        }
        let amount = self.amount.ok_or(malformed_transaction_data!())?;
        let direction = self.direction.ok_or(malformed_transaction_data!())?;
        if self.details.iter().any(|detail| {
            detail
                .direction
                .is_some_and(|detail_direction| detail_direction != direction)
        }) || !detail_amounts_equal_entry(&self.details, &amount)?
        {
            return Err(malformed_transaction_data!());
        }
        Ok(Some(BookedEntry {
            amount,
            direction,
            // Anlage 3 v3.9, 7.2.6 makes both dates optional for camt.052
            // BOOK entries. Missing values remain absent.
            booking_date: self.booking_date,
            value_date: self.value_date,
            reversal: self.reversal,
            entry_reference: self.entry_reference,
            account_servicer_reference: Some(
                self.account_servicer_reference
                    .ok_or(malformed_transaction_data!())?,
            ),
            // Anlage 3 v3.9, 7.1.8.5.1 permits an empty entry-level BkTxCd;
            // the required BTC is preserved from each TxDtls instead.
            bank_transaction_code: transaction_code(
                self.bank_transaction_domain,
                self.bank_transaction_family,
                self.bank_transaction_subfamily,
            ),
            proprietary_transaction_code: self.proprietary_transaction_code,
            statement_position: None,
            details: self.details,
        }))
    }
}

#[derive(Default)]
struct DetailBuilder {
    amount: Option<Amount>,
    direction: Option<CreditDebit>,
    customer_reference: Option<String>,
    end_to_end_reference: Option<String>,
    mandate_reference: Option<String>,
    creditor_reference: Option<String>,
    account_servicer_reference: Option<String>,
    bank_transaction_domain: Option<String>,
    bank_transaction_family: Option<String>,
    bank_transaction_subfamily: Option<String>,
    proprietary_transaction_code: Option<String>,
    debtor_name: Option<String>,
    debtor_iban: Option<String>,
    creditor_name: Option<String>,
    creditor_iban: Option<String>,
    remittance_information: Vec<String>,
}

impl DetailBuilder {
    fn apply(
        &mut self,
        ancestors: &[Node],
        local: &str,
        text: String,
        currency: Option<String>,
    ) -> Result<(), Error> {
        if local == "Amt" && ends_with(ancestors, &["TxDtls"]) {
            set_once(
                &mut self.amount,
                parse_amount(&text, required_currency(currency)?)?,
            )?;
        } else if local == "CdtDbtInd" && ends_with(ancestors, &["TxDtls"]) {
            set_once(&mut self.direction, parse_direction(&text)?)?;
        } else if local == "InstrId" && ends_with(ancestors, &["TxDtls", "Refs"]) {
            set_optional_once(&mut self.customer_reference, text)?;
        } else if local == "EndToEndId" && ends_with(ancestors, &["TxDtls", "Refs"]) {
            set_optional_once(&mut self.end_to_end_reference, text)?;
        } else if local == "MndtId" && ends_with(ancestors, &["TxDtls", "Refs"]) {
            set_optional_once(&mut self.mandate_reference, text)?;
        } else if local == "AcctSvcrRef" && ends_with(ancestors, &["TxDtls", "Refs"]) {
            set_optional_once(&mut self.account_servicer_reference, text)?;
        } else if local == "Ref"
            && ends_with(ancestors, &["TxDtls", "RmtInf", "Strd", "CdtrRefInf"])
        {
            set_optional_once(&mut self.creditor_reference, text)?;
        } else if local == "Cd" && ends_with(ancestors, &["TxDtls", "BkTxCd", "Domn"]) {
            set_optional_once(&mut self.bank_transaction_domain, text)?;
        } else if local == "Cd" && ends_with(ancestors, &["TxDtls", "BkTxCd", "Domn", "Fmly"]) {
            set_optional_once(&mut self.bank_transaction_family, text)?;
        } else if local == "SubFmlyCd"
            && ends_with(ancestors, &["TxDtls", "BkTxCd", "Domn", "Fmly"])
        {
            set_optional_once(&mut self.bank_transaction_subfamily, text)?;
        } else if local == "Cd" && ends_with(ancestors, &["TxDtls", "BkTxCd", "Prtry"]) {
            set_optional_once(&mut self.proprietary_transaction_code, text)?;
        } else if local == "Nm" && ends_with(ancestors, &["TxDtls", "RltdPties", "Dbtr"]) {
            set_optional_once(&mut self.debtor_name, text)?;
        } else if local == "IBAN"
            && ends_with(ancestors, &["TxDtls", "RltdPties", "DbtrAcct", "Id"])
        {
            set_optional_once(&mut self.debtor_iban, text)?;
        } else if local == "Nm" && ends_with(ancestors, &["TxDtls", "RltdPties", "Cdtr"]) {
            set_optional_once(&mut self.creditor_name, text)?;
        } else if local == "IBAN"
            && ends_with(ancestors, &["TxDtls", "RltdPties", "CdtrAcct", "Id"])
        {
            set_optional_once(&mut self.creditor_iban, text)?;
        } else if local == "Ustrd" && ends_with(ancestors, &["TxDtls", "RmtInf"]) {
            self.remittance_information.push(nonempty(text)?);
        }
        Ok(())
    }

    fn finish(
        self,
        entry_direction: Option<CreditDebit>,
    ) -> Result<BookedTransactionDetail, Error> {
        let direction = self.direction;
        let (counterparty_name, counterparty_account) = match direction.or(entry_direction) {
            Some(CreditDebit::Credit) => (self.debtor_name, self.debtor_iban),
            Some(CreditDebit::Debit) => (self.creditor_name, self.creditor_iban),
            None => (None, None),
        };
        let bank_transaction_code = transaction_code(
            self.bank_transaction_domain,
            self.bank_transaction_family,
            self.bank_transaction_subfamily,
        );
        Ok(BookedTransactionDetail {
            amount: self.amount,
            direction,
            customer_reference: self.customer_reference,
            end_to_end_reference: useful_reference(self.end_to_end_reference),
            mandate_reference: useful_reference(self.mandate_reference),
            creditor_reference: useful_reference(self.creditor_reference),
            account_servicer_reference: useful_reference(self.account_servicer_reference),
            bank_transaction_code,
            proprietary_transaction_code: self.proprietary_transaction_code,
            counterparty_name,
            counterparty_account,
            remittance_information: self.remittance_information,
        })
    }
}

fn parse_amount(value: &str, currency: String) -> Result<Amount, Error> {
    let value = value.strip_prefix('+').unwrap_or(value);
    let (integer, fraction) = value.split_once('.').unwrap_or((value, ""));
    if (integer.is_empty() && fraction.is_empty())
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 5
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || integer.len() + fraction.len() > 18
    {
        return Err(malformed_transaction_data!());
    }
    let coefficient = format!("{integer}{fraction}")
        .parse()
        .map_err(|_| malformed_transaction_data!())?;
    let scale = u8::try_from(fraction.len()).map_err(|_| malformed_transaction_data!())?;
    Ok(Amount::new(coefficient, scale, currency))
}

fn validate_currency(value: &str) -> Result<(), Error> {
    if value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        Ok(())
    } else {
        Err(malformed_transaction_data!())
    }
}

fn required_currency(value: Option<String>) -> Result<String, Error> {
    value.ok_or(malformed_transaction_data!())
}

fn parse_direction(value: &str) -> Result<CreditDebit, Error> {
    match value {
        "CRDT" => Ok(CreditDebit::Credit),
        "DBIT" => Ok(CreditDebit::Debit),
        _ => Err(malformed_transaction_data!()),
    }
}

fn parse_boolean(value: &str) -> Result<bool, Error> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(malformed_transaction_data!()),
    }
}

fn transaction_code(
    domain: Option<String>,
    family: Option<String>,
    subfamily: Option<String>,
) -> Option<String> {
    match (domain, family, subfamily) {
        (Some(domain), Some(family), Some(subfamily)) => {
            Some(format!("{domain}.{family}.{subfamily}"))
        }
        _ => None,
    }
}

fn detail_amounts_equal_entry(
    details: &[BookedTransactionDetail],
    entry: &Amount,
) -> Result<bool, Error> {
    let scale = details
        .iter()
        .filter_map(|detail| detail.amount.as_ref().map(Amount::scale))
        .fold(entry.scale(), u8::max);
    let entry_coefficient = scaled_coefficient(entry, scale)?;
    let (amount_seen, detail_sum) =
        details
            .iter()
            .try_fold((false, 0_u128), |(seen, sum), detail| {
                let Some(amount) = detail.amount.as_ref() else {
                    return Ok((seen, sum));
                };
                if amount.currency() != entry.currency() {
                    return Err(malformed_transaction_data!());
                }
                Ok((
                    true,
                    sum.checked_add(scaled_coefficient(amount, scale)?)
                        .ok_or(malformed_transaction_data!())?,
                ))
            })?;
    Ok(!amount_seen || detail_sum == entry_coefficient)
}

fn scaled_coefficient(amount: &Amount, scale: u8) -> Result<u128, Error> {
    let exponent = u32::from(
        scale
            .checked_sub(amount.scale())
            .ok_or(malformed_transaction_data!())?,
    );
    amount
        .coefficient()
        .checked_mul(
            10_u128
                .checked_pow(exponent)
                .ok_or(malformed_transaction_data!())?,
        )
        .ok_or(malformed_transaction_data!())
}

fn parse_date(value: &str) -> Result<NaiveDate, Error> {
    let date = value.get(..10).ok_or(malformed_transaction_data!())?;
    if !valid_timezone(value.get(10..).ok_or(malformed_transaction_data!())?) {
        return Err(malformed_transaction_data!());
    }
    NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| malformed_transaction_data!())
}

fn valid_timezone(value: &str) -> bool {
    if value.is_empty() || value == "Z" {
        return true;
    }
    let bytes = value.as_bytes();
    if bytes.len() != 6
        || !matches!(bytes[0], b'+' | b'-')
        || bytes[3] != b':'
        || !bytes[1..3].iter().all(u8::is_ascii_digit)
        || !bytes[4..6].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    let hour = u16::from(bytes[1] - b'0') * 10 + u16::from(bytes[2] - b'0');
    let minute = u16::from(bytes[4] - b'0') * 10 + u16::from(bytes[5] - b'0');
    hour < 14 && minute < 60 || hour == 14 && minute == 0
}

fn parse_date_time_date(value: &str) -> Result<NaiveDate, Error> {
    value
        .get(..10)
        .ok_or(malformed_transaction_data!())
        .and_then(parse_date)
}

fn useful_reference(value: Option<String>) -> Option<String> {
    value.filter(|value| !matches!(value.as_str(), "NOTPROVIDED" | "NONREF"))
}

fn nonempty(value: String) -> Result<String, Error> {
    if value.is_empty() {
        Err(malformed_transaction_data!())
    } else {
        Ok(value)
    }
}

fn set_once<T>(target: &mut Option<T>, value: T) -> Result<(), Error> {
    if target.replace(value).is_some() {
        Err(malformed_transaction_data!())
    } else {
        Ok(())
    }
}

fn set_optional_once(target: &mut Option<String>, value: String) -> Result<(), Error> {
    set_once(target, nonempty(value)?)
}

fn ends_with(stack: &[Node], expected: &[&str]) -> bool {
    stack.len() >= expected.len()
        && stack[stack.len() - expected.len()..]
            .iter()
            .map(|node| node.local.as_str())
            .eq(expected.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_bounds_reject_excess_nodes_and_depth() {
        let mut bounds = XmlBounds::new();
        for _ in 0..MAX_XML_NODES {
            bounds.count_node().expect("nodes within the bound");
        }
        assert!(bounds.count_node().is_err());

        let bounds = XmlBounds::new();
        assert!(bounds.check_depth(MAX_XML_DEPTH - 1).is_ok());
        assert!(bounds.check_depth(MAX_XML_DEPTH).is_err());
    }
}
