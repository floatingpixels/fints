# Changelog

- **2026-07-29** — Accepted HNSHK security timestamps with omitted optional date/time after the next owner-run synchronization reached dialog termination.
  Added constant field diagnostics while retaining PIN/TAN profile, identity, algorithm, key, certificate, control-pairing, and envelope validation.
  Fictional end-to-end synchronization now proves exactly one HKEND and durable assigned system state without captured protocol data.

- **2026-07-29** — Recognized specification-defined bank-terminated function-999 TAN-method discovery after the next owner-run live initialization.
  BPD and valid 3920 method identifiers now survive the 9050/9800/9955 response set without retaining the ended dialog or sending HKEND.
  Added independent fictional ordering, lifecycle, invalid-method, unrelated-error, redaction, and fresh-initialization regressions.

- **2026-07-29** — Corrected authenticated HNVSK validation after the next owner-run live initialization reached the encryption-header boundary.
  Accepts format-valid FinTS filler variants, all defined roles and party directions, and optional timestamps while retaining fixed codes, bounds, and forbidden IV occupancy.
  Added constant-only field diagnostics and independent fictional variants; the 92-test network-independent stack is green.

- **2026-07-29** — Accepted the specified optional HNSHK/HNSHA control pair around HIRMG and response data inside authenticated HNVSD responses, prompted by an owner-run live initialization.
  Added strict inner and outer security-framing validation with independent fictional parameter, ordering, numbering, and malformed-control fixtures.
  The 91-test network-independent stack is green; raw response data remains neither captured nor exposed.

- **2026-07-29** — Accepted MIME-folded Base64 responses per RFC 2045 section 6.8 and the HBCI PIN/TAN VI.7 MIM filter, prompted by the first owner-run live verification.
  Transport bounds and rejection behavior are unchanged; raw FinTS, HTML, and malformed Base64 remain typed transport errors.
  The 83-test network-independent stack is green.

- **2026-07-28** — Completed Gate 4 with advertised HKWPD 6 positions, HKWDU 5 booked securities transactions, and G112 HKKKU/HKKKS 1 credit-card reads.
  Added bounded MT535/MT536 parsing, exact optional-value semantics, operation-bound TAN continuations, and exhaustive guarded pagination without derived product data.
  The 81-test fictional network-independent stack is green; all product results remain private process-memory values and live verification remains owner-run.

- **2026-07-28** — Completed Gate 3 with four fictional multibank profiles and deterministic advertised capability/version selection.
  Added pre-mutation HIBPA institute validation, non-account HIUPD handling, the official 35-character IBAN correction, and distinct typed capability/SCA failures.
  The 64-test network-independent stack is green with no institution registry, new operation, dependency, credential, or live capture.

- **2026-07-28** — Completed Gate 2 with advertised HKCAZ/camt.052 booked transactions and bounded HKKAZ/MT940 fallback.
  Added operation-bound TAN continuations, exhaustive same-dialog pagination, repeated-point/page/entry limits, exact references or statement positions, and private typed results.
  The 51-test network-independent stack covers fictional independent wire/XML/MT940 fixtures; live verification remains owner-run.

- **2026-07-28** — Completed Gate 1 with a strict FinTS 3.0 codec, concrete PIN/TAN dialog engine, system-ID/BPD/UPD synchronization, and bounded rustls HTTPS transport.
  Added negotiated HKTAN/HITAN 6-7 continuations, HKTAB/HITAB 5 medium discovery, HKSAL/HISAL 6-8 balances, redacted response handling, and fictional independent-wire regressions.
  The 41-test network-independent verification stack is green; live bank verification remains an owner-run consumer integration step.

- **2026-07-28** — Initialized the product-neutral FinTS crate with serial protocol gates, strict security boundaries, a consumer handoff briefing, and an official-specification source register.
  The Rust-only Nix flake and nix-direnv environment pin Rust 1.97.1 with Cargo, Clippy, rustfmt, rust-analyzer, and Rust sources.
  Flake evaluation, tests, Clippy with warnings denied, formatting, and documentation generation are green.
