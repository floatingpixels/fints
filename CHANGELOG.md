# Changelog

- **2026-07-31** — Versioned serialized reusable state and reject incompatible shapes;
  consumers must discard mismatched state and re-synchronize, which may require a TAN.

- **2026-07-31** — Counted malformed MT535 page totals alongside position losses,
  restored `ACTI`/`FIN` consistency, and documented the format-specific boundary.

- **2026-07-31** — Isolated malformed MT535 position fields, returning usable
  siblings with redacted degraded/skipped counts and value-free failure sites.

- **2026-07-31** — Classified invalid MT535 currency prefixes without retaining
  their values and clarified possible currency-less decimal structure.

- **2026-07-31** — Split MT535 price failures into value-free component stages
  and retained the first failing price ordinal plus bounded validation facts.

- **2026-07-31** — Renamed the permanent, opt-in structural diagnostics feature
  to `diagnostics` and documented its value-free but intentionally unstable fact API.

- **2026-07-31** — Preserved value-free depot block inventories across field
  errors and added redacted 90A/90B shape facts with exact official choice checks.

- **2026-07-30** — Made explicit per-account UPD operation permission decisive
  for depot and credit-card reads; descriptive account type no longer vetoes it.

- **2026-07-30** — Accepted the two official unnumbered `70E::HOLD`
  examples without fabricating ambiguous cost basis, derived depot-position
  support from versions, and clarified missing-HISYN recovery diagnostics.

- **2026-07-30** — Added officially archived HKWPD/HIWPD/HIWPDS 5
  negotiation, exact national-account requests, and version-bound MT535
  parsing with deterministic 5/6 selection and persisted capability state.

- **2026-07-30** — Added value-free MT535/MT536 block and field-presence
  diagnostics, safe depot probe result counts, and static block/tag parser
  contexts without changing protocol acceptance.

- **2026-07-30** — Completed the DD-to-wire mapping audit for Gate 1-4
  component-indexed response parsers and recorded every nested-group
  derivation at its parser; no additional incorrect offsets were found.

- **2026-07-30** — Accepted the T32 terminal decoupled-poll shape when a matching
  HITAN `S` accompanies a Gate 4 product result, while retaining continuing,
  unsolicited, and variant-two challenge rejection.

- **2026-07-30** — Corrected TAN-Medium-Liste 2-5 flat component mappings
  across nested account groups, added version-3 negotiation, and removed the
  selector experiment whose premise was invalidated.

- **2026-07-30** — Applied BPD/UPD returned with an experimental HKTAN rejection,
  added redacted HITAB component-shape facts, and added an opt-in version-2
  comparison without changing production negotiation.

- **2026-07-30** — Added an opt-in, single-shot experiment for omitted versus
  explicitly empty HKTAN DE 12 after contradictory unnamed-media discovery.
  Production behavior remains `TanMediumUnavailable`.

- **2026-07-30** — Added opt-in structural diagnostics for HITANS 6/7
  medium requirements, emitted HKTAB selectors, and identifier-free HITAB
  occupancy facts. TAN-medium protocol behavior is unchanged.

- **2026-07-30** — Applied current BPD after TAN-media initialization when
  deciding HKTAN 6/7 name occupancy. Unnamed HITAB 4 generators remain valid
  only when the method needs no designation; no selector is fabricated.

- **2026-07-30** — Applied consume-vs-enforce parsing to balance, cash,
  securities, and credit-card response formats. Unconsumed deviations no longer
  discard typed entries while structural and meaning-bearing checks stay strict.

- **2026-07-30** — Applied consume-vs-enforce parsing to extended feedback,
  BPD, HITAB/HITANS, and processing-irrelevant security fields. Capability
  snapshots now expose three advertised operation-retention windows.

- **2026-07-30** — Accepted an entirely absent, unconsumed HITAB generator-card
  pair across versions 2, 4, and 5 while rejecting partial pairs and prohibited
  card fields on other medium classes.

- **2026-07-30** — Added highest-common HKTAB/HITAB 2, 4, and 5 negotiation,
  exact legacy medium layouts, and the same-dialog HKTAB order after an
  initialization acknowledgement without HITAB.

- **2026-07-30** — Applied the complete HITANS 6/7 TAN-medium requirement and
  documented that UPD-zero adds no pre-HKTAB bootstrap; mandatory missing HITAB
  remains a typed failure without a common delivered media version.

- **2026-07-30** — Preserved TAN-media discovery bank responses across internal
  HKEND while retaining strict missing-HITAB handling; documented 1040 and
  unpublished 0940 without assigning either a new control-flow meaning.

- **2026-07-30** — Retained redacted ordered TAN-media response codes/references
  across HKEND and validated process-4 HITAN acknowledgements without treating
  them as substitutes for mandatory HITAB or as continuations.

- **2026-07-30** — Added explicit missing-HITAB and
  `TanMediumUnavailable` outcomes, bounded discovery structure diagnostics, and
  exactly-once HKEND handling for accepted but unusable medium discovery.

- **2026-07-30** — Accepted and retained bounded class-1 bank notices as
  non-fatal ordered responses, with a new public `ResponseClass::Notice` branch.
  Undefined classes remain rejected with only their safe numeric code exposed.

- **2026-07-30** — Corrected first contact to acquire the PIN/TAN system ID
  through function-999 HKSYN/HISYN before selected-method initialization, with
  bounded fictional lifecycle fixtures.

- **2026-07-30** — Restored `Client: Send` by making opt-in trace sinks transferable
  and covered every public operation result and continuation at compile time.

- **2026-07-30** — Added static, redacted rejection-site context across transaction,
  securities, and generic response parsing plus isolated owner-run parser fuzz targets.

- **2026-07-30** — Kept chrono clock support dev-only for the owner-run probe and
  documented the pre-snapshot transaction-capability caveat and permanent diagnostics.

- **2026-07-30** — Added the gated owner-run live probe, an explicit per-client raw
  transport trace sink, and one derived redacted BPD capability snapshot. Default
  clients remain trace-free; tests cover exchange pairing, redaction, and serde confinement.

- **2026-07-30** — Retained bounded bank response text, data-element references, and
  parameters behind explicit caller accessors while keeping crate errors and `Debug`
  output redacted. Added Latin-1, embedded-account, ordered-error, and bound fixtures.

- **2026-07-30** — Added a typed no-bootstrap limitation when function-999 discovery has 3920 but its required anonymous BPD-zero refresh is terminated without BPD.
  No retry or method inference occurs; sequential fictional fixtures also make direct-refresh initialization versus termination stages observable without wire data.

- **2026-07-30** — Preserved a pending parameter refresh when its open function-999 discovery ends with the exact validated global 9050/9800/unpublished-9952 HKEND response.
  Normal termination errors stay fatal; independent profile-1 HKEND and two-response fixtures cover envelope, numbering, lifecycle, adjacent published errors, and redaction.

- **2026-07-30** — Added explicitly opt-in, boolean-only structural diagnostics for the bounded initialization-recovery decision.
  Normal builds contain neither the gated public API nor its storage; fictional feature-enabled tests preserve redaction and existing protocol behavior.

- **2026-07-30** — Exposed the last initialization request stage as a typed, process-memory-only diagnostic for owner-run compatibility checks.
  The four fixed stage values contain no credentials, identifiers, response values, or wire data; fictional tests cover initial, anonymous-refresh, and rediscovery failures.

- **2026-07-29** — Added one bounded anonymous-BPD repair for the exact global 9050/9800/unpublished-9952 function-999 termination without mandatory 3920.
  The client closes only the anonymous refresh dialog, retries discovery once, and keeps repeated omission or any published authentication error fatal.
  Added fictional end-to-end transport sequencing, method-intersection, placement, retry-bound, and redaction regressions.

- **2026-07-29** — Expanded acceptance across dialog aborts, parameter state, security fillers, camt, SWIFT, and credit-card data to the full specification-permitted value space.
  Strict bounds, redaction, malformed-input rejection, and typed unsupported outcomes remain intact.
  Added independent accepted-shape and adjacent-malformed fixtures for every changed boundary.

- **2026-07-29** — Added typed anonymous-BPD recovery when terminated function-999 discovery returns valid 3920 identifiers without usable HITANS descriptions.
  Unpublished 99xx companions remain uninterpreted and contextual; every published credential, security, lock, registration, and protocol error remains fatal.
  Added fictional refresh, method-intersection, dialog-lifecycle, malformed-response, redaction, and process-memory regressions.

- **2026-07-29** — Added archive-sourced HKSAL/HISAL/HISALS 5 negotiation, national request encoding, and typed legacy balance parsing after owner-run capability evidence.
  Same-version HIBPA responses now preserve all retained BPD capabilities; changed complete BPD still replace them atomically.
  Added independent version-5 wire, sparse/complete/malformed, TAN, identity, serialization, replacement, and redaction regressions.

- **2026-07-29** — Preserved every advertised HISALS version as a redacted generic capability fact after an owner-run balance limitation.
  HKSAL negotiation remained restricted to versions 6-8 at that revision; unsupported-only advertisements returned the typed `BalanceVersion` limitation.
  Added fictional mixed-version, BPD-replacement, limitation, redaction, and reusable-state migration regressions.

- **2026-07-29** — Accepted HNSHK security timestamps with omitted optional date/time after the next owner-run synchronization reached dialog termination.
  Added constant field diagnostics while retaining PIN/TAN profile, identity, algorithm, key, certificate, control-pairing, and envelope validation.
  Fictional end-to-end synchronization now proves exactly one HKEND and durable assigned system state without captured protocol data.

- **2026-07-29** — Recognized specification-defined bank-terminated function-999 TAN-method discovery after the next owner-run live initialization.
  BPD and valid 3920 method identifiers now survive the 9050/9800/9955 response set without retaining the ended dialog or sending HKEND.
  Added independent fictional ordering, lifecycle, invalid-method, unrelated-error, redaction, and fresh-initialization regressions.

- **2026-07-29** — Corrected authenticated HNVSK validation after the next owner-run live initialization reached the encryption-header boundary.
  Accepts format-valid FinTS filler variants, all defined roles and party directions, and optional timestamps while retaining fixed codes, bounds, and forbidden IV occupancy.
  Added constant-only field diagnostics and independent fictional variants; the 92-test network-independent stack is green.

- **2026-07-29** — Accepted the specified optional HNSHK/HNSHA control pair around HIRMG and response data inside authenticated HNVSD responses, prompted by an owner-run live initialization.
  Added strict inner and outer security-framing validation with independent fictional parameter, ordering, numbering, and malformed-control fixtures.
  The 91-test network-independent stack is green; raw response data remains neither captured nor exposed.

- **2026-07-29** — Accepted MIME-folded Base64 responses per RFC 2045 section 6.8 and the HBCI PIN/TAN VI.7 MIM filter, prompted by the first owner-run live verification.
  Transport bounds and rejection behavior are unchanged; raw FinTS, HTML, and malformed Base64 remain typed transport errors.
  The 83-test network-independent stack is green.

- **2026-07-28** — Completed Gate 4 with advertised HKWPD 6 positions, HKWDU 5 booked securities transactions, and G112 HKKKU/HKKKS 1 credit-card reads.
  Added bounded MT535/MT536 parsing, exact optional-value semantics, operation-bound TAN continuations, and exhaustive guarded pagination without derived product data.
  The 81-test fictional network-independent stack is green; all product results remain private process-memory values and live verification remains owner-run.

- **2026-07-28** — Completed Gate 3 with four fictional multibank profiles and deterministic advertised capability/version selection.
  Added pre-mutation HIBPA institute validation, non-account HIUPD handling, the official 35-character IBAN correction, and distinct typed capability/SCA failures.
  The 64-test network-independent stack is green with no institution registry, new operation, dependency, credential, or live capture.

- **2026-07-28** — Completed Gate 2 with advertised HKCAZ/camt.052 booked transactions and bounded HKKAZ/MT940 fallback.
  Added operation-bound TAN continuations, exhaustive same-dialog pagination, repeated-point/page/entry limits, exact references or statement positions, and private typed results.
  The 51-test network-independent stack covers fictional independent wire/XML/MT940 fixtures; live verification remains owner-run.

- **2026-07-28** — Completed Gate 1 with a strict FinTS 3.0 codec, concrete PIN/TAN dialog engine, system-ID/BPD/UPD synchronization, and bounded rustls HTTPS transport.
  Added negotiated HKTAN/HITAN 6-7 continuations, HKTAB/HITAB 5 medium discovery, HKSAL/HISAL 6-8 balances, redacted response handling, and fictional independent-wire regressions.
  The 41-test network-independent verification stack is green; live bank verification remains an owner-run consumer integration step.

- **2026-07-28** — Initialized the product-neutral FinTS crate with serial protocol gates, strict security boundaries, a consumer handoff briefing, and an official-specification source register.
  The Rust-only Nix flake and nix-direnv environment pin Rust 1.97.1 with Cargo, Clippy, rustfmt, rust-analyzer, and Rust sources.
  Flake evaluation, tests, Clippy with warnings denied, formatting, and documentation generation are green.
