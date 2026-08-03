# Working Agreement

`SCOPE.md` defines WHAT to build and is the scope authority. This file defines HOW to
work. Both change only by explicit owner request. Do not edit either file to legitimize
an implementation after the fact.

This repository is a product-neutral FinTS client library. It is self-contained: its
build, tests, fixtures, review, and documentation must never require a Finanzplaner or
other sibling checkout.

## Serial gates

- Work on exactly one `SCOPE.md` gate at a time.
- Do not prepare later operations, segment versions, provider abstractions, or public API
  for a later gate.
- At each gate, stop after the full verification stack, an immutable commit, and
  owner review. Versions and `CHANGELOG.md` entries are produced by the
  release-plz release PR; the owner releases by merging it.
- A task that does not directly close the current gate is out of scope.

## Anti-overcomplication tripwires

Stop and ask the owner before proceeding when:

- Non-test code under `src/` would exceed about 15,500 lines. Fixtures, generated test
  data, and tests do not count.
- Module boundaries follow Rust convention: split by cohesive responsibility (e.g.
  codec, dialog state, segment parsing, typed results), not by line count. Splitting a
  grown file into submodules is routine hygiene and needs no owner approval. A file may
  stay large while it remains one cohesive concern; stop and ask only if a single
  module approaches ~1,500 non-test lines, or if fine-grained splitting is fragmenting
  one concern across many small files.
- You are about to introduce a trait with one implementation, a registry/plugin system,
  a generic segment framework for hypothetical operations, or a config option nobody
  requested.
- A public abstraction is justified only by a future gate. Implement the concrete
  current-gate behavior first and abstract on the third demonstrated use.
- Supporting a bank quirk would silently weaken the official protocol contract or
  fabricate missing data. Report an explicit typed limitation instead.
- A live incompatibility cannot be represented by the current gate without expanding
  its supported operations or security profiles.

These are checkpoints, not quotas.

## Dependencies

The initial direct-dependency allowlist is:

- `serde` for caller-owned persistence of explicitly serializable protocol state.
- `thiserror` for typed, redacted errors.
- `chrono` for protocol-defined dates and times.
- `reqwest` (async client) with rustls TLS and default features disabled for HTTPS
  transport.
- `encoding_rs` only where an official FinTS encoding requirement proves it necessary.
- `base64` for the HTTPS body encoding inherited from the HBCI PIN/TAN mapping.
- `quick-xml` for bounded, namespace-aware parsing of HKCAZ camt.052 transaction data.
- `tokio` as a dev-dependency only (`rt`, `macros`), for the async test harness and the
  owner-run live probe example. Library code takes no direct tokio dependency.

Required transitive dependencies are allowed. Direct dev dependencies need the same
owner approval as runtime dependencies.

Both failure directions require an owner-approved edit to this list before code:

- Adding another direct dependency.
- Reimplementing functionality supplied by an allowlisted dependency.

Do not hand-roll cryptography, TLS, character encoding, URL parsing, or date arithmetic.
Keep optional features out until a current gate proves them necessary.

## Protocol and API boundaries

- Official Deutsche Kreditwirtschaft FinTS specifications are authoritative. Record the
  exact document and version used in `docs/SPECIFICATIONS.md`.
- Independent FinTS implementations (for example `fints-rs`, `python-fints`,
  `hbci4java`) may be consulted as evidence only. Prefer observing their behaviour—the
  wire traffic a working client produces against a real institution—over reading their
  source: observed protocol behaviour is a fact about the institution, carries no
  licence obligation, and answers structural questions faster than inference. Reading
  their source is permitted solely to generate hypotheses; the implementation must then
  be derived from a registered official source and cited to it. Never copy code,
  transcribe tables or constants, translate functions, write code while consulting
  theirs, treat their output as an oracle of correctness, or depend on them at runtime
  or in tests. Most are copyleft—`python-fints` and `hbci4java` are LGPL—so deriving
  from them would attach their obligations to this crate; this is prohibited regardless
  of intent, and it would also foreclose a permissive licence for this repository. Where
  consultation informed a change, record in `docs/SPECIFICATIONS.md` that it was
  corroboration only, naming the project, revision, and licence, alongside the official
  section the behaviour actually rests on. Behaviour with no official basis is an
  observed interoperability fact requiring an owner decision and a fictional regression
  fixture—never “implementation X does this”.
- Owner-run behavioural observation—running such a client against the owner's own bank
  access and reading the resulting message structure—is explicitly permitted and
  encouraged when a live question resists the specification. Only generic structural
  facts cross back into this repository: no captured payloads, credentials, or account
  data.
- Data-dictionary field numbers are not wire component indices. A nested data element
  group flattens into its own components (`ktv` occupies four, `kti` six), shifting
  every field after it. Every parser derived from a specification table records its
  field-to-component derivation in a comment and is covered by fixtures with each nested
  group both populated and absent.
- The crate owns FinTS 3.0 wire syntax, dialog and synchronization state, BPD/UPD
  parsing, TAN continuation, pagination, and only the read operations named in
  `SCOPE.md`.
- The caller owns product registration values, endpoint selection, credential
  persistence, imported-data persistence, UI, and retry policy between user actions.
- Public inputs and outputs are product-neutral. Never expose Tauri, SQL, Finanzplaner,
  or consumer storage types.
- Acceptance follows the specification's full permitted value space, not the fixture's
  example. Meaning-neutral fields—constants and fillers whose value does not change
  interpretation, safety, or security—are read past, not enforced. Rejection is
  reserved for malformed, ambiguous, meaning-changing, or bound-violating data. Strict
  bounds, redaction, and structural validation remain untouched.
- Prefer concrete operation types over a universal segment tree in the public API.
- Missing advertised operations and unsupported parameter combinations are normal typed
  limitations, not guessed fallbacks.
- The non-default `diagnostics` feature is a supported, opt-in capability for
  value-free structural facts: enums, counts, boolean presence, and block inventories.
  It is off by default and adds no diagnostic code or storage when disabled. Its fact
  shapes may change in any revision and never drive control flow; callers branch only
  on typed limitations, errors, and advertised-capability snapshots.

## Security and personal data

- Forbid unsafe Rust in library code.
- Never log or include in `Debug`, `Display`, panic messages, snapshots, or crate-owned
  error formatting: credentials, PINs, TANs, challenges, tokens, raw authenticated
  messages, account identifiers, balances, positions, or transaction data. Bank-sent
  response free text is retained verbatim for explicit caller access but never appears
  through these implicit formatting paths; the caller owns its display and logging
  policy.
- A caller-installed diagnostic trace hook may receive raw outgoing and incoming
  transport payloads. It is opt-in per client construction, never enabled by any
  default path, and documented as carrying credential-bearing traffic; the caller owns
  everything it does with the data. The crate itself still never logs, stores, or
  embeds raw traffic in errors.
- Typed errors carry static descriptions plus non-secret structural context such as a
  field or site identifier, offset, or protocol code. Uniform context-free messages are
  a defect, not a virtue.
- Product registration ID and version are caller inputs. The real Finanzplaner
  registration ID never enters this repository, its fixtures, or its history.
- One-time TANs, challenges, dialog identifiers, tokens, and live sessions remain
  process-memory values. Only reusable state explicitly named by `SCOPE.md` may be
  serializable.
- Reusable state carries an explicit version. State written by a different version is
  rejected with a typed error instructing the caller to discard it and re-synchronize;
  the crate ships no per-field migration or defaulting whose purpose is to read an
  older shape. This is possible because the state is derived, not authored: discarding
  it costs one synchronization. Protocol tolerance—accepting the shapes institutions
  legitimately vary—is a separate concern and is unaffected.
- Owner credentials, account data, and live protocol captures never enter the
  repository. `local/` is ignored for ephemeral owner-controlled work.
- Fixtures use fictional identities and values. Sanitizing a live capture is not enough
  unless the result is structurally reviewed and contains no owner-derived identifiers
  or values.

## Correctness and tests

- Parser and serializer tests use minimal official-spec-based fixtures with the exact
  specification section recorded alongside the test.
- Test malformed lengths, escaping, ordering, response codes, unknown optional data,
  and redaction at the gate where they become relevant.
- Round trips alone are insufficient because encoder and decoder can share the same
  mistake. Expected wire messages must be independently derived, not merely independently
  typed: where a specification prints a full example message, that example is the
  fixture source. A fixture written from the same reading of a field table that produced
  the parser proves nothing—this failure mode has already reached production once.
- Network-independent tests are the primary evidence. Live tests are owner-run,
  opt-in, never part of the default verification stack, and never persist captures.
- A bank-specific workaround requires a fictional regression fixture and a comment
  identifying the observed protocol condition without naming the owner or account.
- Bounded fuzz targets for the wire and format parsers are authorized as owner-run
  tooling outside the default verification stack. Do not add property-test frameworks,
  code generators, mock servers, or snapshot tooling unless the current `SCOPE.md` gate
  explicitly requires them.

## Documentation and commits

- `README.md` explains use and development.
- `BRIEFING.md` is non-normative consumer and agent handoff context.
- `docs/SPECIFICATIONS.md` is the source register.
- `CHANGELOG.md` is generated by release-plz from Conventional Commit messages;
  write commit subjects as complete, user-meaningful change descriptions and do
  not hand-edit generated sections. The hand-written history through 0.3.0
  remains as a frozen section.
- Public API documentation states redaction, continuation, and missing-data semantics.
- Commit small Conventional Commit slices. Protocol golden or fixture changes must be
  deliberate and explained in the commit message.

## Verification

The full stack must stay green:

```sh
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo doc --no-deps
```

`rust-toolchain.toml` is the single toolchain authority: local development enters it
via `direnv allow` or `nix develop`, and CI installs the same pinned toolchain via
rustup across a Linux/macOS/Windows matrix, plus cargo-deny license and
security-advisory audits and an advisory latest-stable job.

## When uncertain

Prefer, in order: the smallest official-spec-backed implementation that closes the
current gate, an explicit typed limitation, then an owner question. Never resolve
uncertainty by implementing the more general protocol model.

Before implementing a fix for observed live behavior, state the hypothesis, the
observation that would falsify it, and the cheapest way to obtain that observation.
When observing costs about as much as reasoning, observe. A confident conclusion
reached without a cheap available check is a defect risk, not a finding.
