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
