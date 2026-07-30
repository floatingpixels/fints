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

Gate 4 is the active owner-review boundary. It adds only the scoped product reads:
advertised HKWPD 6 depot positions, advertised HKWDU 5 booked securities
transactions, and G112 HKKKU/HKKKS 1 credit-card transactions and balances.

The Gate 4 handoff stays narrow:

1. Gate every operation on exact BPD advertisement, UPD authorization, account type,
   signature count, and HIPINS TAN parameters.
2. Parse MT535, MT536, and G112 response values only where the institution explicitly
   supplies them; never synthesize fees, references, balances, or transactions.
3. Exhaust opaque continuation points in the active dialog with repeated-point,
   page-count, and aggregate-entry bounds.
4. Keep all product results, challenges, and pagination state process-memory-only and
   under the existing redaction contract.
5. Return typed limitations for absent or unsupported capabilities. Never derive
   securities transactions from position snapshots.

No institution registry, provider abstraction, endpoint directory, payment operation,
or universal public segment API is part of this gate.

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

The owner performs live verification through Finanzplaner or through the crate's own
owner-run probe example. Both are opt-in, never part of the default verification stack,
and never persist captures. Report only generic protocol behavior: advertised segment
versions, TAN flow class, or a rounded/non-identifying result. Never commit or paste
authenticated wire messages, challenges, credentials, account identifiers, or balances
into Codex context, issues, commits, or fixtures.
