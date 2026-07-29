# FinTS

Product-neutral Rust client for the bounded, read-only FinTS 3.0 PIN/TAN operations in
`SCOPE.md`.

- `SCOPE.md` — supported protocol surface, gates, and explicit exclusions.
- `AGENTS.md` — implementation, dependency, security, and verification rules.
- `BRIEFING.md` — non-normative Finanzplaner integration and agent handoff context.
- `docs/SPECIFICATIONS.md` — exact official specification source register.
- `CHANGELOG.md` — completed progress, at most three lines per entry.

The crate is intentionally independent of Finanzplaner, Tauri, persistence, UI, and
institution directories. Consumers provide their own endpoint, registered product
identity, credentials, and durable storage.

## Gate 1 through Gate 4 API

`Client` is a concrete synchronous HTTPS client for one institute and one user. The
caller supplies:

- an HTTPS FinTS PIN/TAN endpoint;
- an `InstituteId`;
- the registered customer application's `ProductIdentity`;
- `Credentials`; and
- a new or previously serialized `ReusableState`.

The product identity is sent in `HKVVB` on every initialization. This crate has no
built-in registration ID and is not itself the registered customer application.
`ReusableState` derives Serde traits so the caller can choose its own encrypted
persistence format. Credentials are never serializable.

A new connection follows this bounded sequence:

1. Call `initialize`. With no selected method, the client uses security function 999
   to obtain response code 3920 and closes any open discovery dialog. It returns
   `Initialization::ChooseTanMethod` when matching BPD method descriptions are usable.
2. If it instead returns `Initialization::RefreshParameters`, call
   `refresh_parameters` once. Then offer only `tan_methods()` whose identifiers also
   occur in `allowed_tan_methods()`; never construct a method from a 3920 identifier.
3. Call `select_tan_method` for that intersection. If the method requires a named
   medium, call `discover_tan_media` and
   `select_tan_medium`.
4. If `state().system_id()` is absent, call `synchronize`. Persist the state only
   after synchronization completes.
5. Call `initialize` again. Once it returns `Connected`, call only an advertised
   operation authorized for an account discovered through `accounts()`, then call
   `terminate`.

`refresh_parameters` actively requests current BPD with client BPD version zero,
atomically applies the complete response, and closes its anonymous dialog. It is the
recovery path when 3920 supplies no usable method; the client does not silently guess a
method. Allowed 3920 identifiers remain process-memory response state and are not added
to serialized `ReusableState`.

Typed TAN and decoupled approval challenges are operation-specific, process-memory
continuations. A continuation reports `ContinuationKind`, the challenge, and the
earliest permitted poll time. Submit a `Tan` only through the matching
`submit_*_tan` method. For a decoupled challenge, pass `PollingMode::Manual` or
`PollingMode::Automatic` to the matching `poll_*` method; an unadvertised mode,
early poll, expired challenge, or exhausted poll bound fails explicitly. Dropping a
continuation does not serialize it; call `terminate` to cancel the active dialog.
A rejected TAN deliberately ends the Gate 1 flow: terminate when possible, then
restart the complete dialog instead of retrying the TAN inside the existing dialog.
The same operation-bound continuation contract applies to every Gate 2 and Gate 4
read. After a TAN or decoupled approval completes, the client automatically exhausts
any remaining same-dialog pages before returning the result.

The most recently parsed bank response codes are available through
`last_responses()`. They retain only the numeric code, optional segment reference,
response class, and bounded recovery category. Bank free text and raw authenticated
messages are not retained.

## Supported Gate 1 profile

- FinTS 3.0 delimiter syntax using the Latin-1 code set, strict byte lengths,
  escaping, binary values, ordering, and numbering.
- PIN/TAN security envelopes `HNSHK` 4, `HNSHA` 2, `HNVSK` 3, and `HNVSD` 1.
- `HKTAN`/`HITAN` 6 for typed process-variant-2 TANs and version 7 for typed or
  decoupled approval; process variant 1 and required HHD responses are typed
  limitations.
- BPD/UPD, system-ID synchronization, TAN-method selection, and the special
  `HKTAB`/`HITAB` 5 medium-discovery initialization.
- Advertised `HKSAL`/`HISAL` versions 5-8 for one UPD-authorized cash account.
  Version 5 uses its archived HBCI-defined national-account and legacy response
  layout. The client obeys `HIPINS` instead of assuming that balance retrieval is
  TAN-free.

Missing capabilities, unsupported parameter combinations, and multiple required
signers return `Limitation`; the client never invents account or balance data.
Redirects are disabled, response size is bounded, and complete HTTPS bodies use the
officially inherited MIME Base64 mapping. Bounded MIME folding whitespace is accepted;
malformed Base64, HTML, non-whitespace transport garbage, and raw FinTS remain
transport errors. Any transport failure discards the uncertain local dialog; the
caller starts a fresh initialization instead of replaying a message number.
Authenticated institute responses accept only the specified HNVSK/HNVSD envelope and
an optional matched HNSHK/HNSHA control pair around HIRMG and response data; arbitrary
prefix segments and incomplete security framing remain typed response errors. FinTS
filler values are format-checked but not semantically interpreted; structural failures
expose only stable field categories, never received values.

Secret-bearing and private-data-bearing types intentionally omit `Debug`. Callers
must not log credentials, continuations, challenges, accounts, balances, endpoints
containing private query data, or serialized reusable state.

## Supported Gate 2 profile

- `booked_transactions` prefers advertised and UPD-authorized `HKCAZ`/`HICAZ` 1
  with `camt.052.001.08`, then falls back only to advertised `HKKAZ`/`HIKAZ` 7
  or 6.
  This deterministic fallback also applies when `HIPINS` is silent on advertised
  `HKCAZ`, or when `HKCAZ` requires multiple signatures while `HKKAZ` requires one;
  a failed request never triggers fallback.
- Only `BOOK` entries are returned. Pending camt entries and MT942 data are outside
  Gate 2; unsupported descriptors, segment versions, and missing account permission
  are typed limitations.
- FinTS response-code 3040 pagination is exhausted in the active dialog. Opaque
  continuation points are never persisted; repeated points, more than 100 pages,
  or more than 10,000 aggregate entries fail explicitly without a partial result.
- camt XML is parsed as bounded, namespace-aware UTF-8. The legacy MT940 fallback
  uses its specified Latin-1 form and preserves either the supplied bank reference
  or the exact statement number, page number, and entry position. Gate 2 does not
  compare MT940 `:25:` with the requested UPD account because deployed formats vary;
  the result remains bound to the UPD account used for the request. This limitation
  remains deliberate after the Gate 3 fictional profile review.
- Amounts, directions, dates, reversal status, references, transaction codes,
  counterpart data, and remittance information are returned only where the
  institution supplies them. No transaction fingerprint is synthesized.

`BookedTransactions`, `BookedEntry`, `BookedTransactionDetail`, and
`StatementPosition` contain private financial data and deliberately omit `Debug`.
They are process results, not serializable reusable state, and callers must never
log them.

## Supported Gate 3 compatibility

- Four independently written fictional profiles cover the advertised combinations
  demonstrated by one Atruvia institution, one Finanz Informatik institution, and
  two independently operated institutions. Names never select protocol behavior.
- BPD capabilities are replaced as a complete set and supported versions are
  selected deterministically. An advertised but unsupported balance version differs
  from an operation that was never advertised.
- A same-version HIBPA does not erase retained capabilities when business
  parameter segments are omitted; a changed BPD version replaces the complete set.
- `advertised_balance_versions()` exposes only the generic HISALS version numbers;
  pair each with `supports_balance_version()` for redacted compatibility diagnostics.
- Non-account-bound HIUPD records are accepted without fabricating accounts, and the
  official HIUPD 6 correction for an erroneous 35-character IBAN is applied exactly.
- HIBPA parameters whose institute identity differs from the configured institute
  fail before reusable state changes. Codes such as 9075 and 9185 retain distinct
  actionable recovery categories without retaining bank free text.

Gate 3 adds no institution registry, provider abstraction, endpoint discovery, new
operation, or dependency. Stale or wrong endpoint selection remains caller-owned;
the crate reports the typed protocol or transport evidence it can verify.

## Supported Gate 4 products

- `depot_positions` uses only advertised and UPD-authorized `HKWPD`/`HIWPD` 6
  for account types 30-39. Its bounded MT535 parser preserves supplied
  instrument identifiers, quantities and signs, market or indicative prices,
  currencies, dates, market values, and amount- or percentage-denominated cost
  basis.
- `securities_transactions` uses only advertised and UPD-authorized
  `HKWDU`/`HIWDU` 5 for account types 30-39. Its bounded MT536 parser preserves
  supplied references, instruments, quantities, prices, amounts, accrued
  interest, movement types, dates, reversal status, and free text. The protocol
  sentinel `NONREF` and an omitted optional transaction-detail block remain
  missing values. MT536 has no typed fee field, so free text is never interpreted
  as a fee or transaction identity.
- `credit_card_transactions` and `credit_card_balance` use G112
  `HKKKU`/`HIKKU`/`HIKKUS` 1 and `HKKKS`/`HIKKS`/`HIKKSS` 1 only for account
  types 50-59. The conditional international account binding and optional date
  range come exclusively from BPD; UPD must independently authorize the
  operation. Institution-defined card-number masking is preserved as returned,
  and current-balance dates and optional times are preserved exactly. A card
  balance is neither derived from nor reconciled to returned entries.
- FinTS continuation points are exhausted in the active dialog with the same
  repeated-point, 100-page, and 10,000-entry bounds as Gate 2. `HIPINS` decides
  whether each operation requires TAN handling.
- Unsupported versions and unadvertised or unauthorized operations are typed
  `Limitation` values. The crate never derives securities transactions from
  position snapshots and never fills an absent response field from request data.
- An IBAN-only depot without a UPD account number cannot pass strict MT535/MT536
  `:97A:` identity verification and returns a typed error. This deliberate
  limitation is revisited only if live findings justify a bounded protocol rule.

All Gate 4 position, transaction, card, balance, and continuation types contain
private financial data, deliberately omit `Debug`, remain process-memory values,
and must never be logged or serialized as reusable state.

## Development

The repository uses an exact Rust toolchain through a Nix flake and nix-direnv:

```sh
direnv allow
# or
nix develop
```

The shell supplies `rustc`, Cargo, Clippy, rustfmt, rust-analyzer, and Rust sources from
the toolchain pinned by `rust-toolchain.toml` and `flake.lock`.

Run the full verification stack before every gate handoff:

```sh
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo doc --no-deps
```

Live bank access is never part of the default test suite. Owner credentials and captures
must stay outside the repository.
