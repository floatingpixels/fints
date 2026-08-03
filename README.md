# FinTS

`fints` is an asynchronous, read-only FinTS 3.0 PIN/TAN client for German banks. It
retrieves account balances, booked cash transactions, securities positions and
transactions, and credit-card balances and transactions. Payment initiation and every
other state-changing banking operation are deliberately excluded.

## Support

Support is negotiated from the institution's live BPD/UPD. An operation is available
only when the institution advertises a segment version implemented by the crate and the
user's account is authorized for it.

| Operation | FinTS segments | Response format | Live verification |
| --- | --- | --- | --- |
| Dialog, synchronization, TAN methods and media | `HKSYN`/`HISYN`; `HKTAN`/`HITAN` 6-7; `HKTAB`/`HITAB` 2-5 | FinTS PIN/TAN | Verified against multiple institutions |
| Cash-account balance | `HKSAL`/`HISAL` 5-8 | FinTS segment data | Verified |
| Booked cash transactions | `HKCAZ`/`HICAZ` 1; `HKKAZ`/`HIKAZ` 6-7 | camt.052.001.08 or MT940 | Both formats verified |
| Securities positions | `HKWPD`/`HIWPD` 5-6 | MT535 | Verified |
| Booked securities transactions | `HKWDU`/`HIWDU` 5 | MT536 | Implemented, but not live-verified because no available institution advertises `HIWDUS` |
| Credit-card balance | `HKKKS`/`HIKKS` 1 | G112 FinTS segment data | Verified |
| Booked credit-card transactions | `HKKKU`/`HIKKU` 1 | G112 FinTS segment data | Verified |

Live verification demonstrates operation families and deployed variations; it is not a
claim that every listed version is available at every institution. Unsupported,
unadvertised, and unauthorized combinations return typed `Limitation` values instead of
guessed fallbacks.

The crate is currently consumed through exact Git revisions and is not published on
crates.io:

```toml
[dependencies]
fints = { git = "https://github.com/floatingpixels/fints.git", rev = "<immutable-commit>" }
```

## Quick start

FinTS is a dialog protocol. Initialization may require parameter refresh, TAN-method and
medium selection, synchronization, a typed TAN, or decoupled approval. Operations use
their own continuation types; do not discard a continuation and retry the original
request inside the same dialog.

All operations are `async` and drive a tokio-compatible HTTPS transport; the crate
itself never waits, retries, or polls, so the caller schedules decoupled-approval
polling with its own runtime timer. Dropping an operation future mid-exchange is
detected: the next operation aborts the dialog, stale continuations fail with
`Error::StaleContinuation`, and a fresh `initialize` starts cleanly.

The example below leaves user input, waiting, and encrypted persistence as caller
hooks, but shows the complete control flow. A real UI should let the user choose among
the intersection of `allowed_tan_methods()` and `tan_methods()` rather than selecting
the first one.

```rust
use chrono::NaiveDateTime;
use fints::{
    Balance, BalanceRequest, Challenge, Client, ContinuationKind, Credentials, Error,
    Initialization, InstituteId, Limitation, PollingMode, ProductIdentity, ReusableState,
    Synchronization, Tan,
};

async fn read_balance(
    endpoint: &str,
    bank_code: &str,
    product_id: &str,
    credentials: Credentials,
    state: ReusableState,
    mut now: impl FnMut() -> NaiveDateTime,
    mut collect_tan: impl FnMut(&Challenge) -> Result<Tan, Error>,
    mut wait_until: impl AsyncFnMut(NaiveDateTime),
) -> Result<(Box<Balance>, ReusableState), Error> {
    let mut client = Client::new(
        endpoint,
        InstituteId::new("280", bank_code)?,
        ProductIdentity::new(product_id, "1.0")?,
        credentials,
        state,
    )?;

    let mut initialization = client.initialize(now()).await?;
    loop {
        initialization = match initialization {
            Initialization::Connected => break,
            Initialization::RefreshParameters => {
                client.refresh_parameters(now()).await?;
                client.initialize(now()).await?
            }
            Initialization::ChooseTanMethod => {
                let (security_function, medium_required) = {
                    let method = client
                        .tan_methods()
                        .iter()
                        .find(|method| {
                            client
                                .allowed_tan_methods()
                                .iter()
                                .any(|allowed| allowed == method.security_function())
                        })
                        .ok_or(Limitation::TanMethodParametersUnavailable)?;
                    (
                        method.security_function().to_owned(),
                        method.medium_name_required(),
                    )
                };
                client.select_tan_method(&security_function)?;

                if medium_required {
                    let medium_name = client
                        .discover_tan_media(now()).await?
                        .iter()
                        .find_map(|medium| medium.name())
                        .ok_or(Limitation::TanMediumUnavailable)?
                        .to_owned();
                    client.select_tan_medium(&medium_name)?;
                }

                // First contact normally receives a system ID during initialize().
                // If the institution omitted it, synchronize after method selection.
                if client.state().system_id().is_none() {
                    let mut synchronization = client.synchronize(now()).await?;
                    loop {
                        synchronization = match synchronization {
                            Synchronization::Complete => break,
                            Synchronization::Challenge(continuation) => {
                                match continuation.kind() {
                                    ContinuationKind::Tan => {
                                        let tan = collect_tan(continuation.challenge())?;
                                        client.submit_synchronization_tan(
                                            *continuation,
                                            &tan,
                                            now(),
                                        ).await?
                                    }
                                    ContinuationKind::DecoupledApproval => {
                                        let poll_at = continuation
                                            .earliest_poll_at()
                                            .ok_or(Error::InconsistentState)?;
                                        wait_until(poll_at).await;
                                        client.poll_synchronization(
                                            *continuation,
                                            PollingMode::Manual,
                                            poll_at,
                                        ).await?
                                    }
                                }
                            }
                        };
                    }
                }

                client.initialize(now()).await?
            }
            Initialization::Challenge(continuation) => match continuation.kind() {
                ContinuationKind::Tan => {
                    let tan = collect_tan(continuation.challenge())?;
                    client.submit_initialization_tan(*continuation, &tan, now()).await?
                }
                ContinuationKind::DecoupledApproval => {
                    let poll_at = continuation
                        .earliest_poll_at()
                        .ok_or(Error::InconsistentState)?;
                    wait_until(poll_at).await;
                    client.poll_initialization(
                        *continuation,
                        PollingMode::Manual,
                        poll_at,
                    ).await?
                }
            },
        };
    }

    let mut request = client.balance(0, now()).await?;
    let balance = loop {
        request = match request {
            BalanceRequest::Complete(balance) => break balance,
            BalanceRequest::Challenge(continuation) => match continuation.kind() {
                ContinuationKind::Tan => {
                    let tan = collect_tan(continuation.challenge())?;
                    client.submit_balance_tan(*continuation, &tan, now()).await?
                }
                ContinuationKind::DecoupledApproval => {
                    let poll_at = continuation
                        .earliest_poll_at()
                        .ok_or(Error::InconsistentState)?;
                    wait_until(poll_at).await;
                    client.poll_balance(*continuation, PollingMode::Manual, poll_at).await?
                }
            },
        };
    };

    client.terminate(now()).await?;
    Ok((balance, client.into_state()))
}
```

The selected account index comes from `client.accounts()`. Each other read operation
has the same shape: an initial method returns either a complete typed result or an
operation-specific challenge, and the matching `submit_*_tan` or `poll_*` method
consumes that challenge.

## Key concepts

- **Live capability negotiation.** BPD describes institution-wide segment versions and
  PIN/TAN rules; UPD describes the user's accounts and allowed operations. Checking both
  prevents a request from being inferred from an endpoint or account type alone.
- **Typed limitations.** Missing advertisements, unsupported versions, multiple required
  signers, and unsupported TAN combinations are normal `Limitation` results. The client
  does not retry a different format after failure or fabricate missing data.
- **Process-memory continuations.** TAN challenges, decoupled approvals, pagination
  points, dialog identifiers, and one-time TANs are never serializable. The continuation
  binds the next action to its operation and dialog and carries the earliest legal poll
  time.
- **Version-bound reusable state.** `ReusableState` contains the assigned system ID,
  selected TAN settings, and retained BPD/UPD, including account identifiers and owner
  names. Store it only in encrypted storage. After a crate upgrade it may fail to
  deserialize or return `Error::ReusableStateVersion`; both mean discard it and
  re-synchronize. The state is derived from bank parameters and is not migrated.
- **Redaction by default.** Credentials, PINs, TANs, challenges, raw messages, account
  identifiers, balances, positions, and transactions never enter crate-owned logs,
  panic messages, or ordinary `Debug` output. Bank response text is available only
  through explicit `BankResponse` accessors, so its display and logging policy belongs
  to the caller. Installing a `TraceSink` is an explicit exception that delivers raw,
  credential-bearing payloads directly to the caller.
- **Bounded parsing and transport.** HTTPS redirects are disabled; response size,
  message structure, XML depth, pagination, continuation count, and aggregate results
  are bounded. Invalid transport Base64, malformed messages, and repeated continuation
  points fail explicitly.

## Caller responsibilities

A caller supplies:

- the institution's HTTPS FinTS PIN/TAN endpoint and German bank code;
- a product registration ID and application version for `HKVVB`;
- the user ID, optional customer ID, and PIN;
- user interaction for TAN-method and medium selection, TAN entry, and decoupled
  approval timing; and
- encrypted durable storage for serialized `ReusableState`.

Deutsche Kreditwirtschaft requires client applications to identify themselves with a
registered product identity. The application using this crate—not the crate itself—is
the registered product. See the official
[FinTS product-registration page](https://www.fints.org/de/hersteller/produktregistrierung).

The caller also owns endpoint discovery, credential storage, result persistence,
logging policy, retries between user actions, and UI. Live BPD/UPD—not an institute
directory—remains authoritative for supported operations and security methods.

## Deliberate exclusions

The crate does not implement:

- transfers, direct debits, payment initiation, or any other state-changing operation;
- FinTS 4.x;
- HBCI card or key-file security;
- institute or endpoint directories;
- credential or result storage; or
- background synchronization, scheduling, UI, or retry orchestration.

## Development and verification

The exact Rust toolchain is supplied by the Nix flake and nix-direnv:

```sh
direnv allow
# or
nix develop
```

Run the complete verification stack from that environment:

```sh
cargo test --all-targets
cargo test --all-features --all-targets
cargo test --doc
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo doc --no-deps
```

Network-independent fixtures are the primary evidence. `examples/live_probe.rs` is an
explicitly enabled, non-persisting live client for attended interoperability checks; it
requires `FINTS_LIVE_PROBE=1` and the endpoint, bank code, user, PIN, and registered
product identity environment variables. Its output includes bank-authored response
text, which may mention private account or order information.

`FINTS_LIVE_TRACE=1` additionally enables raw credential-bearing trace output. Keep any
capture in an ignored ephemeral file, render only its structure with
`tools/fints_segment_shape.sh` or `tools/swift_shape.sh`, and delete the raw file. The
scripts expose component occupancy and character-class patterns without exposing field
values. The optional `diagnostics` feature similarly exposes only value-free structural
facts; those facts are intended for human diagnosis and are not a stable control-flow
API.

The separate `fuzz/` package contains manually run cargo-fuzz targets for the FinTS wire,
camt, MT940, MT535/MT536, and credit-card parsers. It is not a workspace member and does
not run in the default verification stack.

## Project documents

- [`docs/SPECIFICATIONS.md`](docs/SPECIFICATIONS.md) registers the exact official
  documents, sections, hashes, corrections, and implementation authority behind the
  protocol behavior.
- [`SCOPE.md`](SCOPE.md) records the supported product boundary and explicit exclusions.
- [`AGENTS.md`](AGENTS.md) records the repository's contribution, security, dependency,
  and verification rules.
- [`docs/BRIEFING.md`](docs/BRIEFING.md) contains Finanzplaner-specific integration and
  live-verification context.
- [`CHANGELOG.md`](CHANGELOG.md) is the chronological implementation and
  interoperability record.
