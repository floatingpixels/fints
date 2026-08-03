//! Typed MT535/MT536 depot results: positions, transactions, and their value parts.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantityUnit {
    Units,
    Nominal,
}

/// Exact securities quantity supplied by the institution.
///
/// The value can reveal private holdings and intentionally provides no `Debug`
/// implementation.
#[derive(Clone)]
pub struct SecuritiesQuantity {
    coefficient: u128,
    scale: u8,
    unit: QuantityUnit,
    negative: bool,
}

impl SecuritiesQuantity {
    pub(crate) fn new(coefficient: u128, scale: u8, unit: QuantityUnit, negative: bool) -> Self {
        Self {
            coefficient,
            scale,
            unit,
            negative,
        }
    }

    pub fn coefficient(&self) -> u128 {
        self.coefficient
    }

    pub fn scale(&self) -> u8 {
        self.scale
    }

    pub fn unit(&self) -> QuantityUnit {
        self.unit
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }
}

/// Identifiers and name for one institution-supplied security.
///
/// These fields identify private holdings and intentionally provide no `Debug`
/// implementation.
#[derive(Clone)]
pub struct SecurityInstrument {
    pub(crate) isin: Option<String>,
    pub(crate) wkn: Option<String>,
    pub(crate) name: String,
}

impl SecurityInstrument {
    pub fn isin(&self) -> Option<&str> {
        self.isin.as_deref()
    }

    pub fn wkn(&self) -> Option<&str> {
        self.wkn.as_deref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceQuality {
    Market,
    Indicative,
}

/// Exact quoted security price or cost basis supplied by the institution.
///
/// `quality` is absent for a structured cost basis. This private holding value
/// intentionally provides no `Debug` implementation.
#[derive(Clone)]
pub struct SecurityPrice {
    coefficient: u128,
    scale: u8,
    currency: Option<String>,
    percentage: bool,
    quality: Option<PriceQuality>,
    date: Option<NaiveDate>,
    time: Option<NaiveTime>,
}

impl SecurityPrice {
    pub(crate) fn new(
        coefficient: u128,
        scale: u8,
        currency: Option<String>,
        percentage: bool,
        quality: Option<PriceQuality>,
        date: Option<NaiveDate>,
        time: Option<NaiveTime>,
    ) -> Self {
        Self {
            coefficient,
            scale,
            currency,
            percentage,
            quality,
            date,
            time,
        }
    }

    pub fn coefficient(&self) -> u128 {
        self.coefficient
    }

    pub fn scale(&self) -> u8 {
        self.scale
    }

    pub fn currency(&self) -> Option<&str> {
        self.currency.as_deref()
    }

    pub fn is_percentage(&self) -> bool {
        self.percentage
    }

    pub fn quality(&self) -> Option<PriceQuality> {
        self.quality
    }

    pub fn date(&self) -> Option<NaiveDate> {
        self.date
    }

    pub fn time(&self) -> Option<NaiveTime> {
        self.time
    }
}

/// Signed securities amount; the sign is kept independently from the exact amount.
///
/// This private holding value intentionally provides no `Debug` implementation.
pub struct SecuritiesAmount {
    pub(crate) amount: Amount,
    pub(crate) negative: bool,
}

impl SecuritiesAmount {
    pub fn amount(&self) -> &Amount {
        &self.amount
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }
}

/// One explicitly reported position in a depot statement.
///
/// Optional values remain absent, including supplied optional fields that could not
/// be typed without fabrication. The enclosing [`DepotPositions`] reports that
/// degradation. This private result intentionally provides no `Debug` implementation.
pub struct DepotPosition {
    pub(crate) instrument: SecurityInstrument,
    pub(crate) quantity: SecuritiesQuantity,
    pub(crate) price: Option<SecurityPrice>,
    pub(crate) market_values: Vec<SecuritiesAmount>,
    pub(crate) cost_basis: Option<SecurityPrice>,
    #[cfg(feature = "diagnostics")]
    pub(crate) price_location_detail_present: bool,
}

impl DepotPosition {
    pub fn instrument(&self) -> &SecurityInstrument {
        &self.instrument
    }

    pub fn quantity(&self) -> &SecuritiesQuantity {
        &self.quantity
    }

    pub fn price(&self) -> Option<&SecurityPrice> {
        self.price.as_ref()
    }

    pub fn market_values(&self) -> &[SecuritiesAmount] {
        &self.market_values
    }

    pub fn cost_basis(&self) -> Option<&SecurityPrice> {
        self.cost_basis.as_ref()
    }
}

/// Exhaustively paginated positions for exactly one UPD depot.
///
/// Institution-reported page totals remain in response order; they are never
/// recomputed or deduplicated. Position parsing losses are reported separately
/// without exposing which instrument was affected. This private result
/// intentionally provides no `Debug` implementation.
pub struct DepotPositions {
    pub(crate) account: Account,
    pub(crate) positions: Vec<DepotPosition>,
    pub(crate) total_values: Vec<SecuritiesAmount>,
    pub(crate) parse_counts: DepotPositionParseCounts,
}

impl DepotPositions {
    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn positions(&self) -> &[DepotPosition] {
        &self.positions
    }

    pub fn total_values(&self) -> &[SecuritiesAmount] {
        &self.total_values
    }

    /// Redacted counts of MT535 data omitted while parsing.
    pub fn parse_counts(&self) -> DepotPositionParseCounts {
        self.parse_counts
    }
}

/// Value-free MT535 parsing-loss counts across all returned pages.
///
/// A degraded position remains in the result with one or more malformed optional
/// values absent. A skipped position lacked a usable required position field. A
/// malformed page total is omitted while valid totals remain available.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DepotPositionParseCounts {
    degraded: usize,
    skipped: usize,
    malformed_page_totals: usize,
}

impl DepotPositionParseCounts {
    pub(crate) fn new(degraded: usize, skipped: usize, malformed_page_totals: usize) -> Self {
        Self {
            degraded,
            skipped,
            malformed_page_totals,
        }
    }

    pub fn degraded(self) -> usize {
        self.degraded
    }

    pub fn skipped(self) -> usize {
        self.skipped
    }

    pub fn malformed_page_totals(self) -> usize {
        self.malformed_page_totals
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecuritiesMovement {
    Delivery,
    Receipt,
}

/// One booked securities transaction, containing only explicitly supplied values.
///
/// `NONREF` and an omitted optional transaction-detail block produce absent
/// fields. Free text is not interpreted as a fee or identity. This private result
/// intentionally provides no `Debug` implementation.
pub struct SecuritiesTransaction {
    pub(crate) instrument: SecurityInstrument,
    pub(crate) reference: Option<String>,
    pub(crate) quantity: Option<SecuritiesQuantity>,
    pub(crate) price: Option<SecurityPrice>,
    pub(crate) amount: Option<SecuritiesAmount>,
    pub(crate) accrued_interest: Option<SecuritiesAmount>,
    pub(crate) transaction_kind: Option<String>,
    pub(crate) movement: Option<SecuritiesMovement>,
    pub(crate) effective_date: Option<NaiveDate>,
    pub(crate) value_date: Option<NaiveDate>,
    pub(crate) reversal: Option<bool>,
    pub(crate) free_text: Vec<String>,
}

impl SecuritiesTransaction {
    pub fn instrument(&self) -> &SecurityInstrument {
        &self.instrument
    }
    pub fn reference(&self) -> Option<&str> {
        self.reference.as_deref()
    }
    pub fn quantity(&self) -> Option<&SecuritiesQuantity> {
        self.quantity.as_ref()
    }
    pub fn price(&self) -> Option<&SecurityPrice> {
        self.price.as_ref()
    }
    pub fn amount(&self) -> Option<&SecuritiesAmount> {
        self.amount.as_ref()
    }
    pub fn accrued_interest(&self) -> Option<&SecuritiesAmount> {
        self.accrued_interest.as_ref()
    }
    pub fn transaction_kind(&self) -> Option<&str> {
        self.transaction_kind.as_deref()
    }
    pub fn direction(&self) -> Option<SecuritiesMovement> {
        self.movement
    }
    pub fn effective_date(&self) -> Option<NaiveDate> {
        self.effective_date
    }
    pub fn value_date(&self) -> Option<NaiveDate> {
        self.value_date
    }
    pub fn is_reversal(&self) -> Option<bool> {
        self.reversal
    }
    pub fn free_text(&self) -> &[String] {
        &self.free_text
    }
}

/// Exhaustively paginated booked securities transactions for one UPD depot.
///
/// This private result intentionally provides no `Debug` implementation.
pub struct SecuritiesTransactions {
    pub(crate) account: Account,
    pub(crate) entries: Vec<SecuritiesTransaction>,
}

impl SecuritiesTransactions {
    pub fn account(&self) -> &Account {
        &self.account
    }
    pub fn entries(&self) -> &[SecuritiesTransaction] {
        &self.entries
    }
}
