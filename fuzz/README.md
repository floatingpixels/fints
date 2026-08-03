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

The post-refactor parser soak completed on 2026-08-03 at release v0.3.2 (after the
camt bounds-guard extraction and the parser-module restructuring) using the committed
fictional seeds, libFuzzer's 4 KiB input bound, and AddressSanitizer. Generated corpus
entries remained ignored and no crash or OOM artifacts were produced.

| Target | Duration | Executions | Findings |
| --- | ---: | ---: | ---: |
| `message` | 901 s | 8,485,228 | 0 |
| `camt` | 901 s | 30,211,071 | 0 |
| `mt940` | 901 s | 23,113,810 | 0 |
| `securities_document` | 901 s | 23,329,336 | 0 |
| `credit_card_entry` | 901 s | 41,689,410 | 0 |

The previous clean soak (2026-08-01, post-resilience) recorded 9.4M/33.9M/22.4M/20.4M/40.4M
executions with the same methodology and zero findings.
