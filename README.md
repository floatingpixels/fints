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

1. Call `initialize`. With neither a system ID nor a selected method, the client opens
   the required function-999 synchronization dialog, sends HKSYN, retains the HISYN
   system ID, obtains response code 3920, and closes any open dialog. It returns
   `Initialization::ChooseTanMethod` when matching BPD method descriptions are usable.
   If that response supplies usable BPD and method parameters but omits mandatory
   HISYN, initialization returns `MissingValue("assigned system ID")` while retaining
   those parameters. The caller may select from the retained allowed-method
   intersection; the client then performs the required fresh synchronization when no
   system ID exists, or ordinary initialization when one is already retained.
2. If it instead returns `Initialization::RefreshParameters`, call
   `refresh_parameters` once. Then offer only `tan_methods()` whose identifiers also
   occur in `allowed_tan_methods()`; never construct a method from a 3920 identifier.
   If the institution terminates that anonymous BPD-zero request without complete BPD,
   the crate returns `Limitation::TanMethodParametersUnavailable`; there is no
   specification-defined alternate bootstrap or automatic retry.
3. Call `select_tan_method` for that intersection. `medium_name_required()` is true
   only when HITANS advertises requirement code 2 and more than one active medium.
   If the method requires a named medium, call `discover_tan_media` and
   `select_tan_medium`. A missing HITAB response fails as a typed missing value;
   an empty or non-selectable required list returns
   `Limitation::TanMediumUnavailable`. The accepted discovery dialog is still
   closed exactly once. A process-4 HITAN acknowledges the embedded HKTAN; when
   that initialization response does not already contain HITAB, the client sends
   the separate HKTAB order in the same dialog using the highest mutually
   supported advertised version (2, 3, 4, or 5). No common version returns
   `Limitation::TanMediumVersion` before network I/O.
   A sole unnamed generator record is not an implicit selection: when HKTAN
   requires DE 12, only the HITAB medium designation supplies that value, so an
   unnamed list returns `Limitation::TanMediumUnavailable`.
   UPD is not a prerequisite for this first-access flow, and the HKTAB filler is
   not a substitute for a real medium in an ordinary initialization.
4. Call `initialize` again. Once it returns `Connected`, call only an advertised
   operation authorized for an account discovered through `accounts()`, then call
   `terminate`.

`synchronize` remains available for an explicit bank-directed resynchronization after
a method has been selected; ordinary first contact obtains the system ID through the
initial `initialize` call.

`refresh_parameters` actively requests current BPD with client BPD version zero,
atomically applies the complete response, and closes its anonymous dialog. It is the
recovery path when 3920 supplies no usable method; the client does not silently guess a
method. `last_initialization_stage()` reports whether this direct refresh last reached
anonymous initialization or termination. Allowed 3920 identifiers remain process-memory
response state and are not added to serialized `ReusableState`.

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
`last_responses()`. Alongside the numeric code, optional segment reference, response
class, and bounded recovery category, they retain the bank-authored free text,
data-element reference, and parameters through explicit accessors. The caller owns
display and logging policy for that diagnostic text; crate error messages and `Debug`
output omit it. `last_tan_media_discovery_responses()` separately preserves the
discovery operation's responses across its internal HKEND cleanup without changing
`last_responses()` semantics. Raw authenticated messages are not retained.

`Client::new_with_trace` optionally accepts a per-client `TraceSink`. It receives raw
outgoing and Base64-decoded incoming FinTS payloads with a monotonically increasing
exchange index. These payloads can contain credentials and all private protocol data;
installing a sink explicitly makes the caller responsible for its handling. `Client::new`
has no trace path, and the crate never logs or stores traced payloads itself.

## Supported Gate 1 profile

- FinTS 3.0 delimiter syntax using the Latin-1 code set, strict byte lengths,
  escaping, binary values, ordering, and numbering.
- PIN/TAN security envelopes `HNSHK` 4, `HNSHA` 2, `HNVSK` 3, and `HNVSD` 1.
- `HKTAN`/`HITAN` 6 for typed process-variant-2 TANs and version 7 for typed or
  decoupled approval; process variant 1 and required HHD responses are typed
  limitations.
- BPD/UPD, system-ID synchronization, TAN-method selection, and negotiated
  `HKTAB`/`HITAB` 2-5 medium discovery.
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
- `advertised_capabilities()` derives one redacted snapshot for balance, camt and MT940
  cash transactions, depot positions and transactions, and credit-card reads. It
  includes advertised and supported versions, HIPINS TAN facts, camt descriptors,
  advertised retention windows, and every safe parameter-segment code/version observed
  in the current BPD.
- Non-account-bound HIUPD records are accepted without fabricating accounts, and the
  official HIUPD 6 correction for an erroneous 35-character IBAN is applied exactly.
- HIBPA parameters whose institute identity differs from the configured institute
  fail before reusable state changes. Codes such as 9075 and 9185 retain distinct
  actionable recovery categories without interpreting bank free text.

Gate 3 adds no institution registry, provider abstraction, endpoint discovery, new
operation, or dependency. Stale or wrong endpoint selection remains caller-owned;
the crate reports the typed protocol or transport evidence it can verify.

## Supported Gate 4 products

- `depot_positions` uses only advertised and UPD-authorized `HKWPD`/`HIWPD` 5-6.
  Its bounded MT535 parser preserves supplied
  instrument identifiers, quantities and signs, market or indicative prices,
  currencies, dates, market values, and amount- or percentage-denominated cost
  basis. Malformed optional position values and portfolio page totals remain
  absent with typed degraded/skipped/malformed-total counts; an unusable required
  position is skipped without hiding that loss.
- `securities_transactions` uses only advertised and UPD-authorized
  `HKWDU`/`HIWDU` 5. Its bounded MT536 parser preserves
  supplied references, instruments, quantities, prices, amounts, accrued
  interest, movement types, dates, reversal status, and free text. The protocol
  sentinel `NONREF` and an omitted optional transaction-detail block remain
  missing values. MT536 has no typed fee field, so free text is never interpreted
  as a fee or transaction identity. The MT535 loss-isolation counters do not apply
  to MT536 transactions or credit-card entries: their malformed consumed fields
  still fail the complete typed page/response.
- `credit_card_transactions` and `credit_card_balance` use G112
  `HKKKU`/`HIKKU`/`HIKKUS` 1 and `HKKKS`/`HIKKS`/`HIKKSS` 1. The conditional
  international account binding and optional date range come exclusively from
  BPD; UPD must independently authorize the operation. Institution-defined
  card-number masking is preserved as returned, and current-balance dates and
  optional times are preserved exactly. A card balance is neither derived from
  nor reconciled to returned entries.
- UPD `Kontoart` remains optional descriptive metadata for caller-side routing.
  It never vetoes an operation that the account's allowed-operation list
  explicitly authorizes; `UPD-Verwendung` still governs unlisted operations.
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

### Owner-run parser fuzzing

The separate `fuzz/` crate contains bounded cargo-fuzz targets for the wire, camt,
MT940, securities-document, and credit-card-entry parsers. It is not a workspace member
and is never part of CI or the default verification stack. Its committed seed corpora
come only from the repository's fictional specification fixtures.

Run a target explicitly with nightly Rust, for example:

```sh
cargo +nightly fuzz run message
```

Typed parser errors are expected; the target succeeds while parsing remains panic- and
OOM-free. Generated corpora, crashes, and artifacts stay ignored under `fuzz/`.

### Owner-run live probe

`examples/live_probe.rs` is an opt-in diagnostic caller and persists nothing. It refuses
to run unless `FINTS_LIVE_PROBE=1` is set:

```sh
FINTS_LIVE_PROBE=1 \
FINTS_ENDPOINT='https://bank.example/fints' \
FINTS_BLZ='12345678' \
FINTS_USER_ID='owner-input' \
FINTS_PIN='owner-input' \
FINTS_PRODUCT_ID='registered-caller-input' \
FINTS_PRODUCT_VERSION='1.0' \
FINTS_PROBE_SYNCHRONIZE=1 \
cargo run --features diagnostics --example live_probe
```

`FINTS_CUSTOMER_ID` and `FINTS_COUNTRY_CODE` are optional. Method selection uses
`FINTS_TAN_METHOD` or `FINTS_TAN_METHOD_INDEX`; a required medium uses
`FINTS_TAN_MEDIUM_INDEX`. Each one-time TAN is read interactively from stdin and the
terminal may echo it.

The optional account-index flags `FINTS_PROBE_BALANCE_ACCOUNT`,
`FINTS_PROBE_DEPOT_POSITIONS_ACCOUNT`, `FINTS_PROBE_DEPOT_TRANSACTIONS_ACCOUNT`,
`FINTS_PROBE_CARD_BALANCE_ACCOUNT`, and `FINTS_PROBE_CARD_TRANSACTIONS_ACCOUNT`
enable the corresponding read. Financial values and identifiers are never printed;
depot probes report only result counts and, with diagnostics enabled, value-free
block and field-presence facts. A successfully parsed block tree remains available
after a later field error. Depot-position results report typed degraded, skipped, and
malformed-page-total counts: malformed optional values remain absent on an otherwise
usable position, while a position lacking a usable required instrument or aggregate
quantity is skipped. Valid portfolio totals remain available when a malformed sibling
is counted and omitted.
Optional 90A/90B diagnostics report only structural categories and validation
booleans; per-position diagnostics add only the one-based source ordinal, disposition,
and static failure site. The probe prints all caller-visible bank response texts,
which may reference the owner's accounts or orders.

`FINTS_LIVE_TRACE=1` additionally installs the raw trace sink and writes complete
credential-bearing request and response payloads to stdout as hexadecimal and escaped
Latin-1 text. Use it only in an owner-controlled terminal. For structural capture,
redirect it only to an ignored ephemeral file, render that file through `tools/`, and
delete it immediately; never paste the raw output into an issue or agent conversation
or enable it in normal consumers.

### Supported diagnostics

The non-default `diagnostics` feature exposes initialization decision
booleans, TAN-medium-discovery structure, and MT535/MT536 block inventories and
field-presence booleans for interoperability work. It is supported, opt-in, and safe to
enable in a shipped build when diagnosing a failing connection. It never exposes
inspected field values, segment contents, response text or parameters, medium names,
generator-card values, securities identifiers, amounts, references, or dates. Default
builds carry no diagnostic code or storage; the public API and its storage fields exist
only when the feature is explicitly enabled.

Diagnostic facts describe internal parsing and negotiation structure. Their shape may
change in any revision and is not a stable API contract: callers may render or log the
facts for humans, but must never branch on them. Control flow uses typed limitations,
errors, and advertised-capability snapshots only.
