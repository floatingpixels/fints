# FinTS Client Briefing

This document is non-normative handoff context. `SCOPE.md` and `AGENTS.md` remain the
authorities.

## Why this repository exists

Finanzplaner is adding local, explicit, read-only synchronization for products exposed
through FinTS 3.0 PIN/TAN. Protocol work is deliberately isolated here so it can remain
product-neutral, fixture-driven, independently versioned, and consumable at an exact Git
revision without a sibling checkout.

The registered customer product is Finanzplaner, not this library. The caller supplies
its complete product registration ID and application version separately. This crate
must not embed, display, log, validate against, or otherwise know the real registration
value.

## Repository boundary

This crate owns:

- FinTS message and segment encoding/decoding needed by the current gate.
- Dialog initialization, synchronization, and required BPD/UPD state.
- TAN method and medium discovery plus typed and decoupled TAN continuation.
- Pagination and the scoped read operations.
- Product-neutral typed results, continuations, limitations, and redacted errors.

Finanzplaner owns:

- Institute-directory import and endpoint review.
- Product registration values and application version.
- SQLCipher storage of reusable connection state and credentials.
- Backup/restore, UI, user-triggered orchestration, and process-memory TAN entry.
- Product mapping, transaction identity, atomic import, reconciliation, and history.

The crate does not need Finanzplaner's private institute CSV/PDF, source tree, database,
or actual credentials.

## Current assignment

Only Gate 1 is active: a registered FinTS 3.0 PIN/TAN connection that discovers BPD/UPD
and obtains an advertised cash-account balance through `HKSAL`, supporting typed or
decoupled TAN where required.

Start with the smallest vertical protocol path:

1. Register the exact official specification documents and sections before coding.
2. Write independent fictional wire fixtures for dialog initialization and the required
   response segments.
3. Implement only the message codec and concrete segments those fixtures require.
4. Add synchronization plus BPD/UPD interpretation needed to select the advertised
   balance and TAN behavior.
5. Add explicit continuation values for typed and decoupled TAN.
6. Return a typed balance result without consumer persistence or mapping concerns.
7. Audit every public error and `Debug` representation for secret and private-data
   leakage.

Do not implement booked transactions, pagination beyond what Gate 1 proves necessary,
depots, credit cards, payments, a provider registry, or a universal public segment API.

## Cross-repository handoff

Each consumer handoff should contain:

- The full immutable Git commit revision.
- The public operations and state types added or changed.
- The exact fixture and verification commands that passed.
- Explicit unsupported BPD/UPD or TAN combinations.
- Any reusable state the caller must persist and any state that must remain in memory.
- Confirmation that no sibling checkout, real product ID, credentials, or live data is
  required.

Finanzplaner then pins that revision. Consumer findings return here as concrete protocol
fixtures or limitations, producing a new immutable revision rather than an edited tag or
committed path dependency.

## Live verification

The owner performs live verification through Finanzplaner. Report only generic protocol
behavior: advertised segment versions, TAN flow class, or a rounded/non-identifying
result. Never commit or paste authenticated wire messages, challenges, credentials,
account identifiers, or balances into Codex context, issues, commits, or fixtures.
