//! Supported, opt-in structural diagnostics for owner-attended interoperability work.
//!
//! This module is compiled only with the `development-diagnostics` feature. It exposes
//! decision booleans and bounded segment code/version facts only: no raw wire values,
//! response text, credentials, identifiers, medium names, or financial data. Default
//! builds contain none of this module's code or storage.

use crate::{TanMedium, TanMediumClass, TanMediumStatus};

/// The SWIFT document carried by the latest depot response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepotDocumentKind {
    Mt535,
    Mt536,
}

/// A redacted MT535/MT536 block name.
///
/// Unknown block names are collapsed to `Other`; no received block text is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepotBlockKind {
    General,
    FinancialInstrument,
    SubBalance,
    AdditionalInformation,
    Transaction,
    Link,
    TransactionDetails,
    Other,
}

/// A redacted MT535/MT536 field tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepotTagKind {
    StatementNumber13A,
    Activity17B,
    Amount19A,
    Reference20C,
    Indicator22F,
    Movement22H,
    Status25D,
    Page28E,
    Instrument35B,
    TransactionQuantity36B,
    DateRange69,
    FreeText70,
    PercentagePrice90A,
    AmountPrice90B,
    ExchangeRate92B,
    Quantity93B,
    SubBalance93C,
    Location94,
    Account97A,
    DateTime98,
    Days99A,
    Other,
}

impl DepotTagKind {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::StatementNumber13A => 0,
            Self::Activity17B => 1,
            Self::Amount19A => 2,
            Self::Reference20C => 3,
            Self::Indicator22F => 4,
            Self::Movement22H => 5,
            Self::Status25D => 6,
            Self::Page28E => 7,
            Self::Instrument35B => 8,
            Self::TransactionQuantity36B => 9,
            Self::DateRange69 => 10,
            Self::FreeText70 => 11,
            Self::PercentagePrice90A => 12,
            Self::AmountPrice90B => 13,
            Self::ExchangeRate92B => 14,
            Self::Quantity93B => 15,
            Self::SubBalance93C => 16,
            Self::Location94 => 17,
            Self::Account97A => 18,
            Self::DateTime98 => 19,
            Self::Days99A => 20,
            Self::Other => 21,
        }
    }
}

/// One field tag occurrence, without its qualifier or value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepotTagFact {
    kind: DepotTagKind,
    depth: usize,
    occurrence: usize,
}

impl DepotTagFact {
    pub(crate) fn new(kind: DepotTagKind, depth: usize, occurrence: usize) -> Self {
        Self {
            kind,
            depth,
            occurrence,
        }
    }

    pub fn kind(self) -> DepotTagKind {
        self.kind
    }
    /// Nesting depth of the block containing this tag.
    pub fn depth(self) -> usize {
        self.depth
    }
    /// One-based occurrence count for this tag kind in traversal order.
    pub fn occurrence(self) -> usize {
        self.occurrence
    }
}

/// Which optional MT535/MT536 price tag was structurally present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepotPriceTagKind {
    Percentage90A,
    Amount90B,
}

/// Value-free classification of the price qualifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepotPriceQualifierKind {
    Market,
    Indicative,
    Unknown,
}

/// Value-free classification of the price-unit code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepotPriceUnitKind {
    Percentage,
    ActualAmount,
    Unknown,
}

/// Redacted structural shape of one optional 90A/90B price field.
///
/// This fact retains only enum classifications and component-presence booleans.
/// It never contains the qualifier, unit, currency, price, instrument identity,
/// or raw field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepotPriceShapeFact {
    tag: DepotPriceTagKind,
    qualifier: DepotPriceQualifierKind,
    unit: DepotPriceUnitKind,
    qualifier_present: bool,
    unit_present: bool,
    currency_present: bool,
    price_present: bool,
}

impl DepotPriceShapeFact {
    pub(crate) fn new(
        tag: DepotPriceTagKind,
        qualifier: DepotPriceQualifierKind,
        unit: DepotPriceUnitKind,
        qualifier_present: bool,
        unit_present: bool,
        currency_present: bool,
        price_present: bool,
    ) -> Self {
        Self {
            tag,
            qualifier,
            unit,
            qualifier_present,
            unit_present,
            currency_present,
            price_present,
        }
    }

    pub fn tag(self) -> DepotPriceTagKind {
        self.tag
    }
    pub fn qualifier(self) -> DepotPriceQualifierKind {
        self.qualifier
    }
    pub fn unit(self) -> DepotPriceUnitKind {
        self.unit
    }
    pub fn qualifier_present(self) -> bool {
        self.qualifier_present
    }
    pub fn unit_present(self) -> bool {
        self.unit_present
    }
    pub fn currency_present(self) -> bool {
        self.currency_present
    }
    pub fn price_present(self) -> bool {
        self.price_present
    }
}

impl DepotBlockKind {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::General => 0,
            Self::FinancialInstrument => 1,
            Self::SubBalance => 2,
            Self::AdditionalInformation => 3,
            Self::Transaction => 4,
            Self::Link => 5,
            Self::TransactionDetails => 6,
            Self::Other => 7,
        }
    }
}

/// One block opening in document order, without received values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepotBlockFact {
    kind: DepotBlockKind,
    depth: usize,
    occurrence: usize,
}

impl DepotBlockFact {
    pub(crate) fn new(kind: DepotBlockKind, depth: usize, occurrence: usize) -> Self {
        Self {
            kind,
            depth,
            occurrence,
        }
    }

    pub fn kind(self) -> DepotBlockKind {
        self.kind
    }

    pub fn depth(self) -> usize {
        self.depth
    }

    /// One-based occurrence count for this block kind in document order.
    pub fn occurrence(self) -> usize {
        self.occurrence
    }
}

/// Value-free occupancy facts for one parsed MT535 position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepotPositionPresenceFact {
    isin: bool,
    wkn: bool,
    name: bool,
    quantity: bool,
    price: bool,
    market_value: bool,
    cost_basis: bool,
}

impl DepotPositionPresenceFact {
    pub(crate) fn from_position(position: &crate::DepotPosition) -> Self {
        Self {
            isin: position.instrument().isin().is_some(),
            wkn: position.instrument().wkn().is_some(),
            name: !position.instrument().name().is_empty(),
            quantity: true,
            price: position.price().is_some(),
            market_value: !position.market_values().is_empty(),
            cost_basis: position.cost_basis().is_some(),
        }
    }

    pub fn isin_present(self) -> bool {
        self.isin
    }
    pub fn wkn_present(self) -> bool {
        self.wkn
    }
    pub fn name_present(self) -> bool {
        self.name
    }
    pub fn quantity_present(self) -> bool {
        self.quantity
    }
    pub fn price_present(self) -> bool {
        self.price
    }
    pub fn market_value_present(self) -> bool {
        self.market_value
    }
    pub fn cost_basis_present(self) -> bool {
        self.cost_basis
    }
}

/// Value-free occupancy facts for one parsed MT536 transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecuritiesTransactionPresenceFact {
    isin: bool,
    wkn: bool,
    name: bool,
    reference: bool,
    quantity: bool,
    price: bool,
    amount: bool,
    accrued_interest: bool,
    transaction_kind: bool,
    movement: bool,
    effective_date: bool,
    value_date: bool,
    reversal: bool,
    free_text: bool,
}

impl SecuritiesTransactionPresenceFact {
    pub(crate) fn from_transaction(entry: &crate::SecuritiesTransaction) -> Self {
        Self {
            isin: entry.instrument().isin().is_some(),
            wkn: entry.instrument().wkn().is_some(),
            name: !entry.instrument().name().is_empty(),
            reference: entry.reference().is_some(),
            quantity: entry.quantity().is_some(),
            price: entry.price().is_some(),
            amount: entry.amount().is_some(),
            accrued_interest: entry.accrued_interest().is_some(),
            transaction_kind: entry.transaction_kind().is_some(),
            movement: entry.direction().is_some(),
            effective_date: entry.effective_date().is_some(),
            value_date: entry.value_date().is_some(),
            reversal: entry.is_reversal().is_some(),
            free_text: !entry.free_text().is_empty(),
        }
    }

    pub fn isin_present(self) -> bool {
        self.isin
    }
    pub fn wkn_present(self) -> bool {
        self.wkn
    }
    pub fn name_present(self) -> bool {
        self.name
    }
    pub fn reference_present(self) -> bool {
        self.reference
    }
    pub fn quantity_present(self) -> bool {
        self.quantity
    }
    pub fn price_present(self) -> bool {
        self.price
    }
    pub fn amount_present(self) -> bool {
        self.amount
    }
    pub fn accrued_interest_present(self) -> bool {
        self.accrued_interest
    }
    pub fn transaction_kind_present(self) -> bool {
        self.transaction_kind
    }
    pub fn movement_present(self) -> bool {
        self.movement
    }
    pub fn effective_date_present(self) -> bool {
        self.effective_date
    }
    pub fn value_date_present(self) -> bool {
        self.value_date
    }
    pub fn reversal_present(self) -> bool {
        self.reversal
    }
    pub fn free_text_present(self) -> bool {
        self.free_text
    }
}

/// Redacted structure of the most recently parsed MT535 or MT536 response page.
///
/// The inventories contain only recognized block/tag kinds, nesting depth, and
/// occurrence counts. Price shapes contain only enum classifications and presence
/// booleans; entry facts contain booleans only. This type never contains securities
/// identifiers, currencies, amounts, references, dates, free text, or raw wire data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepotResponseFacts {
    document_kind: DepotDocumentKind,
    block_inventory: Vec<DepotBlockFact>,
    tag_inventory: Vec<DepotTagFact>,
    price_shapes: Vec<DepotPriceShapeFact>,
    positions: Vec<DepotPositionPresenceFact>,
    transactions: Vec<SecuritiesTransactionPresenceFact>,
}

impl DepotResponseFacts {
    pub(crate) fn for_position_structure(
        block_inventory: Vec<DepotBlockFact>,
        tag_inventory: Vec<DepotTagFact>,
        price_shapes: Vec<DepotPriceShapeFact>,
    ) -> Self {
        Self {
            document_kind: DepotDocumentKind::Mt535,
            block_inventory,
            tag_inventory,
            price_shapes,
            positions: Vec::new(),
            transactions: Vec::new(),
        }
    }

    pub(crate) fn for_transaction_structure(
        block_inventory: Vec<DepotBlockFact>,
        tag_inventory: Vec<DepotTagFact>,
        price_shapes: Vec<DepotPriceShapeFact>,
    ) -> Self {
        Self {
            document_kind: DepotDocumentKind::Mt536,
            block_inventory,
            tag_inventory,
            price_shapes,
            positions: Vec::new(),
            transactions: Vec::new(),
        }
    }

    pub(crate) fn for_positions(
        block_inventory: Vec<DepotBlockFact>,
        tag_inventory: Vec<DepotTagFact>,
        price_shapes: Vec<DepotPriceShapeFact>,
        positions: &[crate::DepotPosition],
    ) -> Self {
        Self {
            document_kind: DepotDocumentKind::Mt535,
            block_inventory,
            tag_inventory,
            price_shapes,
            positions: positions
                .iter()
                .map(DepotPositionPresenceFact::from_position)
                .collect(),
            transactions: Vec::new(),
        }
    }

    pub(crate) fn for_transactions(
        block_inventory: Vec<DepotBlockFact>,
        tag_inventory: Vec<DepotTagFact>,
        price_shapes: Vec<DepotPriceShapeFact>,
        transactions: &[crate::SecuritiesTransaction],
    ) -> Self {
        Self {
            document_kind: DepotDocumentKind::Mt536,
            block_inventory,
            tag_inventory,
            price_shapes,
            positions: Vec::new(),
            transactions: transactions
                .iter()
                .map(SecuritiesTransactionPresenceFact::from_transaction)
                .collect(),
        }
    }

    pub fn document_kind(&self) -> DepotDocumentKind {
        self.document_kind
    }
    pub fn block_inventory(&self) -> &[DepotBlockFact] {
        &self.block_inventory
    }
    pub fn tag_inventory(&self) -> &[DepotTagFact] {
        &self.tag_inventory
    }
    pub fn price_shapes(&self) -> &[DepotPriceShapeFact] {
        &self.price_shapes
    }
    pub fn positions(&self) -> &[DepotPositionPresenceFact] {
        &self.positions
    }
    pub fn transactions(&self) -> &[SecuritiesTransactionPresenceFact] {
        &self.transactions
    }
}

/// Redacted HITANS fields governing HKTAN's TAN-medium-name occupancy.
///
/// The field numbers are one-based Data Dictionary positions; the component
/// indices are the corresponding zero-based parser positions. No method
/// identifier, name, or other HITANS value is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitansMediumRequirementFact {
    hktan_version: u16,
    requirement_code: u8,
    requirement_field_number: usize,
    requirement_component_index: usize,
    active_media_count: Option<u8>,
    active_media_count_field_number: usize,
    active_media_count_component_index: usize,
    medium_name_required: bool,
}

impl HitansMediumRequirementFact {
    pub(crate) fn new(
        hktan_version: u16,
        requirement_code: u8,
        active_media_count: Option<u8>,
        medium_name_required: bool,
    ) -> Self {
        Self {
            hktan_version,
            requirement_code,
            requirement_field_number: 19,
            requirement_component_index: 18,
            active_media_count,
            active_media_count_field_number: 21,
            active_media_count_component_index: 20,
            medium_name_required,
        }
    }

    pub fn hktan_version(self) -> u16 {
        self.hktan_version
    }

    pub fn requirement_code(self) -> u8 {
        self.requirement_code
    }

    pub fn requirement_field_number(self) -> usize {
        self.requirement_field_number
    }

    pub fn requirement_component_index(self) -> usize {
        self.requirement_component_index
    }

    pub fn active_media_count(self) -> Option<u8> {
        self.active_media_count
    }

    pub fn active_media_count_field_number(self) -> usize {
        self.active_media_count_field_number
    }

    pub fn active_media_count_component_index(self) -> usize {
        self.active_media_count_component_index
    }

    pub fn medium_name_required(self) -> bool {
        self.medium_name_required
    }
}

/// Redacted shape of the same-dialog HKTAB request actually emitted.
///
/// `medium_name_field_present` records only the HKTAB segment shape. HKTAB
/// 2-5 has no TAN-medium designation field, so valid requests report `false`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HktabRequestFact {
    version: u16,
    medium_type: u8,
    medium_class: Option<TanMediumClass>,
    medium_name_field_present: bool,
}

impl HktabRequestFact {
    pub(crate) fn all_media(version: u16) -> Self {
        Self {
            version,
            medium_type: 0,
            medium_class: (version >= 4).then_some(TanMediumClass::All),
            medium_name_field_present: false,
        }
    }

    pub fn version(self) -> u16 {
        self.version
    }

    pub fn medium_type(self) -> u8 {
        self.medium_type
    }

    pub fn medium_class(self) -> Option<TanMediumClass> {
        self.medium_class
    }

    pub fn medium_name_field_present(self) -> bool {
        self.medium_name_field_present
    }
}

/// Redacted occupancy and classification facts for one parsed HITAB medium.
///
/// This type never retains or exposes the medium name or generator-card values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReturnedTanMediumFact {
    class: TanMediumClass,
    status: TanMediumStatus,
    name_present: bool,
    card_number_present: bool,
    card_sequence_present: bool,
}

impl ReturnedTanMediumFact {
    fn from_medium(medium: &TanMedium) -> Self {
        Self {
            class: medium.class,
            status: medium.status,
            name_present: medium.name.is_some(),
            card_number_present: medium.development_card_number_present,
            card_sequence_present: medium.development_card_sequence_present,
        }
    }

    pub fn class(self) -> TanMediumClass {
        self.class
    }

    pub fn status(self) -> TanMediumStatus {
        self.status
    }

    pub fn name_present(self) -> bool {
        self.name_present
    }

    pub fn card_number_present(self) -> bool {
        self.card_number_present
    }

    pub fn card_sequence_present(self) -> bool {
        self.card_sequence_present
    }
}

/// Redacted component occupancy of one repeated TAN-medium DEG.
///
/// Positions are one-based flat wire-component positions after nested DEGs are
/// expanded, not Data Dictionary field numbers. Only the component count and
/// occupied positions are retained; no values or identifiers are exposed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TanMediumElementShapeFact {
    component_count: usize,
    occupied_components: Vec<usize>,
}

impl TanMediumElementShapeFact {
    pub(crate) fn new(component_count: usize, occupied_components: Vec<usize>) -> Self {
        Self {
            component_count,
            occupied_components,
        }
    }

    pub fn component_count(&self) -> usize {
        self.component_count
    }

    pub fn occupied_components(&self) -> &[usize] {
        &self.occupied_components
    }
}

/// One validated response/business segment observed during an owner-attended diagnostic.
///
/// Segment codes and versions are generic protocol facts. This type contains no segment
/// contents and deliberately does not implement serialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceivedSegmentFact {
    code: String,
    version: u16,
}

impl ReceivedSegmentFact {
    pub(crate) fn new(code: String, version: u16) -> Self {
        Self { code, version }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn version(&self) -> u16 {
        self.version
    }
}

/// One validated HIRMG/HIRMS response observed during TAN-medium discovery.
///
/// The code and optional request-segment reference are generic protocol facts. This
/// type contains no response text, parameters, data-element references, or wire data
/// and deliberately does not implement serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceivedResponseFact {
    code: u16,
    segment_number: Option<u16>,
}

impl ReceivedResponseFact {
    pub(crate) fn new(code: u16, segment_number: Option<u16>) -> Self {
        Self {
            code,
            segment_number,
        }
    }

    pub fn code(self) -> u16 {
        self.code
    }

    pub fn segment_number(self) -> Option<u16> {
        self.segment_number
    }
}

/// Redacted structure of the most recent TAN-medium discovery response.
///
/// The ordered segment facts exclude security controls and all segment contents. The
/// ordered response facts retain only four-digit codes and request-segment references.
/// Advertised and selected HKTAB/HITAB versions are generic BPD facts.
/// `discovered_medium_count` is `None` only when medium parsing failed before a complete
/// list could be established.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TanMediaDiscoveryFacts {
    received_segments: Vec<ReceivedSegmentFact>,
    received_responses: Vec<ReceivedResponseFact>,
    advertised_versions: Vec<u16>,
    selected_version: u16,
    discovered_medium_count: Option<usize>,
    hitans_requirement: Option<HitansMediumRequirementFact>,
    hktab_request: Option<HktabRequestFact>,
    returned_media: Vec<ReturnedTanMediumFact>,
    returned_medium_shapes: Vec<TanMediumElementShapeFact>,
    tan_usage_option: Option<u8>,
    initialization_hktan_medium_name_supplied: Option<bool>,
}

impl TanMediaDiscoveryFacts {
    pub(crate) fn new(
        received_segments: Vec<ReceivedSegmentFact>,
        received_responses: Vec<ReceivedResponseFact>,
        advertised_versions: Vec<u16>,
        selected_version: u16,
        hitans_requirement: Option<HitansMediumRequirementFact>,
        initialization_hktan_medium_name_supplied: Option<bool>,
    ) -> Self {
        Self {
            received_segments,
            received_responses,
            advertised_versions,
            selected_version,
            discovered_medium_count: None,
            hitans_requirement,
            hktab_request: None,
            returned_media: Vec::new(),
            returned_medium_shapes: Vec::new(),
            tan_usage_option: None,
            initialization_hktan_medium_name_supplied,
        }
    }

    pub(crate) fn extend(
        &mut self,
        received_segments: Vec<ReceivedSegmentFact>,
        received_responses: Vec<ReceivedResponseFact>,
    ) {
        self.received_segments.extend(received_segments);
        self.received_responses.extend(received_responses);
    }

    pub(crate) fn set_hktab_request(&mut self, request: HktabRequestFact) {
        self.hktab_request = Some(request);
    }

    pub(crate) fn set_discovered_media(&mut self, media: &[TanMedium]) {
        self.discovered_medium_count = Some(media.len());
        self.returned_media = media
            .iter()
            .map(ReturnedTanMediumFact::from_medium)
            .collect();
    }

    pub(crate) fn set_returned_medium_shapes(&mut self, shapes: Vec<TanMediumElementShapeFact>) {
        self.returned_medium_shapes = shapes;
    }

    pub(crate) fn set_tan_usage_option(&mut self, option: Option<u8>) {
        self.tan_usage_option = option;
    }

    pub fn received_segments(&self) -> &[ReceivedSegmentFact] {
        &self.received_segments
    }

    pub fn received_responses(&self) -> &[ReceivedResponseFact] {
        &self.received_responses
    }

    pub fn advertised_versions(&self) -> &[u16] {
        &self.advertised_versions
    }

    pub fn selected_version(&self) -> u16 {
        self.selected_version
    }

    pub fn discovered_medium_count(&self) -> Option<usize> {
        self.discovered_medium_count
    }

    pub fn hitans_requirement(&self) -> Option<HitansMediumRequirementFact> {
        self.hitans_requirement
    }

    pub fn hktab_request(&self) -> Option<HktabRequestFact> {
        self.hktab_request
    }

    pub fn returned_media(&self) -> &[ReturnedTanMediumFact] {
        &self.returned_media
    }

    pub fn returned_medium_shapes(&self) -> &[TanMediumElementShapeFact] {
        &self.returned_medium_shapes
    }

    /// HITAB's TAN usage option (`0`, `1`, or `2`), when valid and present.
    ///
    /// This generic code describes whether the customer may use all active
    /// media in parallel, exactly one at a time, or one mobile and one
    /// generator in parallel. It contains no medium identifier.
    pub fn tan_usage_option(&self) -> Option<u8> {
        self.tan_usage_option
    }

    /// Whether the process-4 initialization HKTAN occupied its DE 12 medium name.
    ///
    /// This exposes only occupancy; the supplied designation or filler is not retained.
    pub fn initialization_hktan_medium_name_supplied(&self) -> Option<bool> {
        self.initialization_hktan_medium_name_supplied
    }
}

/// Redacted facts used to decide whether initialization may perform the one bounded
/// anonymous-BPD repair.
///
/// The value is process-memory only and deliberately does not implement serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitializationRecoveryFacts {
    usable_selected_method_present: bool,
    bpd_zero: bool,
    upd_zero: bool,
    tan_parameters_empty: bool,
    has_tan_method_response: bool,
    global_abort_shape: bool,
    eligible: bool,
}

impl InitializationRecoveryFacts {
    pub(crate) fn new(
        usable_selected_method_present: bool,
        bpd_zero: bool,
        upd_zero: bool,
        tan_parameters_empty: bool,
        has_tan_method_response: bool,
        global_abort_shape: bool,
        eligible: bool,
    ) -> Self {
        Self {
            usable_selected_method_present,
            bpd_zero,
            upd_zero,
            tan_parameters_empty,
            has_tan_method_response,
            global_abort_shape,
            eligible,
        }
    }

    pub fn usable_selected_method_present(self) -> bool {
        self.usable_selected_method_present
    }

    pub fn bpd_zero(self) -> bool {
        self.bpd_zero
    }

    pub fn upd_zero(self) -> bool {
        self.upd_zero
    }

    pub fn tan_parameters_empty(self) -> bool {
        self.tan_parameters_empty
    }

    pub fn has_tan_method_response(self) -> bool {
        self.has_tan_method_response
    }

    pub fn global_abort_shape(self) -> bool {
        self.global_abort_shape
    }

    pub fn eligible(self) -> bool {
        self.eligible
    }
}
