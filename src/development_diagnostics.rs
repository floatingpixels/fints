//! Temporary, opt-in diagnostics for owner-attended live interoperability work.
//!
//! This module is compiled only with the `development-diagnostics` feature. It exposes
//! decision booleans and bounded segment code/version facts only: no raw wire values,
//! response text, credentials, identifiers, medium names, or financial data. Normal
//! consumers and production builds must leave the feature off.

use crate::{TanMedium, TanMediumClass, TanMediumStatus};

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
/// 2/4/5 has no TAN-medium designation field, so valid requests report `false`.
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
