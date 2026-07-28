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

## Gate 1 API

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
   to obtain BPD/UPD and response code 3920, closes that discovery dialog, and returns
   `Initialization::ChooseTanMethod`.
2. Choose one of `tan_methods()` whose identifier is present in
   `allowed_tan_methods()`, then call `select_tan_method`.
3. If that method requires a named medium, call `discover_tan_media` and
   `select_tan_medium`.
4. If `state().system_id()` is absent, call `synchronize`. Persist the state only
   after synchronization completes.
5. Call `initialize` again. Once it returns `Connected`, call `balance` only for an
   account discovered through `accounts()`, then call `terminate`.

`refresh_parameters` performs a separate anonymous BPD refresh and closes its dialog.
It is the recovery path when 3920 supplies no usable method; the client does not
silently guess a method.

Typed TAN and decoupled approval challenges are operation-specific, process-memory
continuations. A continuation reports `ContinuationKind`, the challenge, and the
earliest permitted poll time. Submit a `Tan` only through the matching
`submit_*_tan` method. For a decoupled challenge, pass `PollingMode::Manual` or
`PollingMode::Automatic` to the matching `poll_*` method; an unadvertised mode,
early poll, expired challenge, or exhausted poll bound fails explicitly. Dropping a
continuation does not serialize it; call `terminate` to cancel the active dialog.

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
- Advertised `HKSAL`/`HISAL` versions 6-8 for one UPD-authorized cash account.
  The client obeys `HIPINS` instead of assuming that balance retrieval is TAN-free.

Missing capabilities, unsupported parameter combinations, and multiple required
signers return `Limitation`; the client never invents account or balance data.
Redirects are disabled, response size is bounded, and complete HTTPS bodies use the
officially inherited Base64 mapping. A non-Base64 response is a transport error, not
a raw-message fallback. Any transport failure discards the uncertain local dialog;
the caller starts a fresh initialization instead of replaying a message number.

Secret-bearing and private-data-bearing types intentionally omit `Debug`. Callers
must not log credentials, continuations, challenges, accounts, balances, endpoints
containing private query data, or serialized reusable state.

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
