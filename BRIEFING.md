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

Only Gate 3 is active: broaden compatibility of the completed cash balance and booked
transaction operations through official advertisement, BPD/UPD, response-code, and
account-layout variations demonstrated by four fictional institution profiles.

The Gate 3 close-out stays narrow:

1. Keep capability and segment-version selection entirely advertisement-derived.
2. Cover one Atruvia, one Finanz Informatik, and two independent-institution protocol
   profiles with independently written fictional fixtures.
3. Distinguish absent capabilities from advertised but unsupported versions.
4. Reject a mismatched HIBPA institute identity before mutating reusable state.
5. Accept specification-permitted non-account UPD records without fabricating accounts.
6. Preserve actionable, redacted response and endpoint failures.

Do not add another banking operation, institution registry, provider abstraction,
endpoint directory, depots, credit cards, payments, or a universal public segment API.
Gate 4 remains closed.

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
