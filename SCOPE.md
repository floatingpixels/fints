# Scope

This repository implements the smallest product-neutral Rust client required for
read-only FinTS 3.0 PIN/TAN synchronization. It is an independently versioned library,
consumed by applications at an exact immutable version or Git revision.

This file is the authority for WHAT to build. It changes only by explicit owner request.

## Authority and compatibility

- Official Deutsche Kreditwirtschaft FinTS specifications are the protocol authority.
  Each implemented segment and behavior records its exact source in
  `docs/SPECIFICATIONS.md`.
- `fints-rs` may be consulted as implementation reference only. It is not copied, used
  as an oracle, or included as a dependency.
- Compatibility is capability-driven. Live BPD/UPD determine supported operations and
  versions. An absent or unsupported operation returns a typed limitation.
- Broad support means participating institutions that expose compatible advertised
  FinTS behavior; it is not a promise that every bank, provider, product, or account is
  reachable.

## Product-neutral boundary

The crate owns:

- FinTS 3.0 wire syntax and validation needed by the active gate.
- Dialog initialization, synchronization, and termination.
- BPD and UPD parsing required by scoped operations.
- TAN method/medium discovery and typed or decoupled TAN continuation.
- Pagination and continuation for scoped read operations.
- Product-neutral request, result, reusable-state, continuation, limitation, and error
  types.
- A redacted advertised-capability snapshot containing segment codes, versions, and
  parameter facts the institution advertises, without account or personal data.
- HTTPS transport using rustls TLS.

The caller owns:

- PIN/TAN endpoint selection and institute lookup.
- Product registration ID and application version supplied separately for `HKVVB`.
- Benutzerkennung, optional Kunden-ID, PIN, and TAN collection.
- Encryption and persistence of credentials and reusable protocol state.
- Process lifetime, user interaction, retry orchestration, logging policy, and UI.
- Account/product mapping, imported-data identity, persistence, and reconciliation.

The crate never embeds a consumer identity and does not register itself as the customer
product.

## Security contract

- Credentials, PINs, TANs, challenges, tokens, raw authenticated messages, private
  account data, and consumer product registration values never appear in logs, errors,
  panic messages, or ordinary `Debug` output.
- Bank-sent response free text (Rückmeldungstext) is retained verbatim, bounded by its
  specified length, and exposed to the caller as typed display/diagnostic data. The
  institution authors this text and it may reference accounts or orders; the caller
  owns all display and logging policy for it. It never appears in the crate's own error
  messages, `Debug` output, or panic messages.
- Typed TANs, decoupled approvals, challenges, dialog identifiers, tokens, and live
  sessions remain process-memory values.
- Only assigned system ID, selected TAN method/medium, and the minimum required BPD/UPD
  synchronization state may be exposed for caller-owned reusable persistence.
- Network redirects, unbounded response bodies, repeated pagination/continuation points,
  and malformed length or escape data fail explicitly.
- The library performs no payment initiation or state-changing banking operation.

## Read model

- Account discovery comes from UPD.
- Cash and savings balances use `HKSAL` only when the required operation/version is
  advertised.
- Booked cash transactions use advertised `HKCAZ`, with `HKKAZ` only as the explicitly
  bounded fallback introduced by Gate 2.
- Securities positions use advertised `HKWPD` only; booked securities
  transactions use advertised `HKWDU` only.
- Credit-card balances and booked transactions use advertised `HKKKU` and related typed
  operations only.
- Values are returned only when explicitly supplied by the institution. Missing values
  remain missing; the crate does not infer currencies, identifiers, cost basis, pending
  entries, or transaction identity.

## Serial gates

Each gate ends with the full verification stack, a maximum-three-line `CHANGELOG.md`
entry, one immutable commit/revision, and owner review. Work on later gates must not
start early.

### Gate 1 — Registered connection and balance

- Official-spec-based independent parser and serializer fixtures cover the exact FinTS
  message structure and concrete segment versions needed by the gate.
- Dialog initialization supplies the caller's product ID and version correctly, obtains
  or reuses an assigned system ID, and handles synchronization and termination.
- Required BPD/UPD data discovers accounts, advertised balance operation versions, TAN
  methods, and TAN media without assuming support from the endpoint itself.
- Typed and decoupled TAN flows return explicit process-memory continuations.
- `HKSAL` returns one typed cash-account balance and its explicitly supplied metadata.
- Errors and public `Debug` output pass dedicated redaction regressions.

Gate 1 does not include booked transactions, depots, credit cards, payments, a universal
segment API, or bank-specific claims. Its release is ready for an exact-pinned consumer
integration and owner-run live balance verification.

### Gate 2 — Booked cash transactions

- Advertised `HKCAZ` plus bounded `HKKAZ` fallback return booked entries only.
- Pagination and continuation are exhausted with repeated-point and safety-bound
  detection.
- Multiple accounts per connection remain distinct.
- Bank-returned entry references, or exact statement sequence and entry position where
  defined, are preserved without inventing fingerprints.

### Gate 3 — Broad multibank cash compatibility

- Fictional regressions cover protocol differences demonstrated by one Atruvia
  institution, one Finanz Informatik institution, and two independently operated
  institutions.
- Unsupported BPD/UPD combinations and stale/wrong endpoints return actionable typed
  limitations.
- Compatibility fixes remain protocol-based and do not introduce an institution plugin
  registry.

### Gate 4 — Depots and credit cards

- Advertised `HKWPD` returns positions, quantities, prices, market values, currencies,
  and cost basis only where explicitly present.
- Advertised `HKWDU` returns booked securities transactions with their dates,
  instruments, quantities, prices, amounts, fees, and references only where
  explicitly present. Bank-supplied references are preserved without inventing
  transaction identity. Pagination and continuation follow the same exhaustion,
  repeated-point, and safety-bound rules as Gate 2.
- Advertised credit-card operations including `HKKKU` return balances and booked
  transactions without fabrication.
- Unsupported or insufficiently typed product data remains an explicit limitation.
  Institutions that do not advertise `HKWDU` return the normal typed limitation; the
  crate never derives securities transactions from position snapshots.

## Explicitly out of scope

- FinTS 4.1.
- HBCI card or key-file security.
- Payment initiation, transfers, direct debits, order changes, or any write operation.
- EBICS, screen scraping, PSD2 aggregators, cloud relays, background synchronization,
  telemetry, and credential storage.
- Institute directories or runtime endpoint feeds.
- Consumer UI, databases, backup, product mapping, categorization, and reconciliation.
- Loans, insurance, pensions, PayPal, crypto exchanges, and untyped guessed products.
- A universal FinTS framework, provider plugin system, or compatibility claims not
  demonstrated by the current gate.

## Verification

Default verification is entirely network-independent:

```sh
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo doc --no-deps
```

Fixtures contain fictional data and cite their official specification basis. Live
verification is owner-run through the consuming application and never stores captures in
this repository.
