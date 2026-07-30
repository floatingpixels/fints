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
- At each gate, stop after the full verification stack, a maximum-three-line
  `CHANGELOG.md` entry, an immutable commit, and owner review.
- A task that does not directly close the current gate is out of scope.

## Anti-overcomplication tripwires

Stop and ask the owner before proceeding when:

- Non-test code under `src/` would exceed about 12,500 lines. Fixtures, generated test
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
- `reqwest` with rustls TLS and default features disabled for HTTPS transport.
- `encoding_rs` only where an official FinTS encoding requirement proves it necessary.
- `base64` for the HTTPS body encoding inherited from the HBCI PIN/TAN mapping.
- `quick-xml` for bounded, namespace-aware parsing of HKCAZ camt.052 transaction data.

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
- `fints-rs` may be inspected as implementation reference only. Do not copy it, use it
  as an oracle, or depend on it at runtime or in tests.
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
- Product registration ID and version are caller inputs. The real Finanzplaner
  registration ID never enters this repository, its fixtures, or its history.
- One-time TANs, challenges, dialog identifiers, tokens, and live sessions remain
  process-memory values. Only reusable state explicitly named by `SCOPE.md` may be
  serializable.
- Owner credentials, account data, and live protocol captures never enter the
  repository. `local/` is ignored for temporary owner-controlled work.
- Fixtures use fictional identities and values. Sanitizing a live capture is not enough
  unless the result is structurally reviewed and contains no owner-derived identifiers
  or values.

## Correctness and tests

- Parser and serializer tests use minimal official-spec-based fixtures with the exact
  specification section recorded alongside the test.
- Test malformed lengths, escaping, ordering, response codes, unknown optional data,
  and redaction at the gate where they become relevant.
- Round trips alone are insufficient because encoder and decoder can share the same
  mistake. Include independently written expected wire messages.
- Network-independent tests are the primary evidence. Live tests are owner-run,
  opt-in, never part of the default verification stack, and never persist captures.
- A bank-specific workaround requires a fictional regression fixture and a comment
  identifying the observed protocol condition without naming the owner or account.
- Do not add fuzzing, property-test frameworks, code generators, mock servers, or
  snapshot tooling unless the current `SCOPE.md` gate explicitly requires them.

## Documentation and commits

- `README.md` explains use and development.
- `BRIEFING.md` is non-normative consumer and agent handoff context.
- `docs/SPECIFICATIONS.md` is the source register.
- `CHANGELOG.md` records completed progress, with at most three lines per entry.
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

Run it from `direnv allow` or `nix develop` so CI, agents, and local development use the
same toolchain.

## When uncertain

Prefer, in order: the smallest official-spec-backed implementation that closes the
current gate, an explicit typed limitation, then an owner question. Never resolve
uncertainty by implementing the more general protocol model.
