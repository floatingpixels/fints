# Owner-run parser fuzzing

This separate cargo-fuzz package is not a workspace member and never runs in the
library's default verification or CI stack. Each target accepts raw bytes and succeeds
when the bounded parser returns normally or with a typed error; panics and uncontrolled
allocation are failures.

Run targets explicitly with nightly Rust:

```sh
cargo +nightly fuzz run message
cargo +nightly fuzz run camt
cargo +nightly fuzz run mt940
cargo +nightly fuzz run securities_document
cargo +nightly fuzz run credit_card_entry
```

The committed seeds are copied only from fictional fixtures:

- `message`: `src/wire/tests.rs`, independent HKVVB message.
- `camt`: `src/response/transaction_tests.rs`, fictional booking-day camt document.
- `mt940`: `src/response/transaction_tests.rs`, fictional MT940 statement.
- `securities_document`: `src/response/product_tests.rs`, fictional MT535 document.
- `credit_card_entry`: `src/response/product_tests.rs`, fictional HIKKU entry DEG.

Generated corpus entries and crash artifacts are ignored. Never add live payloads,
owner-derived values, credentials, or captures to a corpus.

## Clean soak record

The post-resilience parser soak completed on 2026-08-01 using the committed fictional
seeds, libFuzzer's 4 KiB input bound, and AddressSanitizer. Generated corpus entries
remained ignored and no crash or OOM artifacts were produced.

| Target | Duration | Executions | Findings |
| --- | ---: | ---: | ---: |
| `message` | 900 s | 9,376,040 | 0 |
| `camt` | 901 s | 33,862,447 | 0 |
| `mt940` | 900 s | 22,413,197 | 0 |
| `securities_document` | 900 s | 20,391,825 | 0 |
| `credit_card_entry` | 900 s | 40,360,603 | 0 |
