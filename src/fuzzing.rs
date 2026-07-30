//! Internal owner-run fuzz harnesses.
//!
//! This module is available only with the non-default `fuzzing` feature. It exposes no
//! parsed values and exists solely for the separate `fuzz/` package.

const MAX_FUZZ_INPUT_BYTES: usize = 1024 * 1024;

/// Exercises the bounded FinTS wire-message parser.
pub fn message(input: &[u8]) -> bool {
    input.len() <= MAX_FUZZ_INPUT_BYTES && crate::wire::Message::parse(input).is_ok()
}

/// Exercises the bounded camt.052 parser.
pub fn camt(input: &[u8]) -> bool {
    input.len() <= MAX_FUZZ_INPUT_BYTES && crate::response::fuzz_camt(input)
}

/// Exercises the bounded MT940 parser.
pub fn mt940(input: &[u8]) -> bool {
    input.len() <= MAX_FUZZ_INPUT_BYTES && crate::response::fuzz_mt940(input)
}

/// Exercises the bounded MT535/MT536 document parser.
pub fn securities_document(input: &[u8]) -> bool {
    input.len() <= MAX_FUZZ_INPUT_BYTES && crate::response::fuzz_securities_document(input)
}

/// Exercises one bounded credit-card transaction entry.
pub fn credit_card_entry(input: &[u8]) -> bool {
    input.len() <= MAX_FUZZ_INPUT_BYTES && crate::response::fuzz_credit_card_entry(input)
}

#[cfg(test)]
mod tests {
    #[test]
    fn committed_fictional_seeds_reach_successful_parser_paths() {
        assert!(super::message(include_bytes!(
            "../fuzz/corpus/message/fictional-hkvvb"
        )));
        assert!(super::camt(include_bytes!(
            "../fuzz/corpus/camt/fictional-booking-day"
        )));
        assert!(super::mt940(include_bytes!(
            "../fuzz/corpus/mt940/fictional-statement"
        )));
        assert!(super::securities_document(include_bytes!(
            "../fuzz/corpus/securities_document/fictional-mt535"
        )));
        assert!(super::credit_card_entry(include_bytes!(
            "../fuzz/corpus/credit_card_entry/fictional-hikku-entry"
        )));
    }

    #[test]
    fn fuzz_harnesses_reject_inputs_above_the_transport_bound() {
        let oversized = vec![0; super::MAX_FUZZ_INPUT_BYTES + 1];
        assert!(!super::message(&oversized));
        assert!(!super::camt(&oversized));
        assert!(!super::mt940(&oversized));
        assert!(!super::securities_document(&oversized));
        assert!(!super::credit_card_entry(&oversized));
    }
}
