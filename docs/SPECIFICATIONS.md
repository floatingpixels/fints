# Specification Source Register

Official Deutsche Kreditwirtschaft (DK) FinTS specifications are authoritative for
this crate. A base PDF must be read together with the published correction and
extension register: a newer-looking PDF does not by itself supersede later entries in
that register.

Specification PDFs were downloaded only into the ignored `local/` directory for this
research pass. They are not part of the repository. Access date for every source below
is 2026-07-28.

## Gate 1 normative documents

### FinTS — Financial Transaction Services, Schnittstellenspezifikation, Formals

- **Protocol/release:** Version 3.0-FV, Final Version, 2017-10-06.
- **Official source:** [FinTS 3.0 specification download
  page](https://www.fints.org/de/spezifikation) (official filename
  `FinTS_3.0_Formals_2017-10-06_final_version.pdf`).
- **Research SHA-256:** `6b4809acd43acd2c6166c486964dee4b84b488a6a6902b29d1221458eaed7239`.
- **Gate 1 sections:** B.1 (character set); B.2-B.4 (data elements,
  length/restriction rules, formats); B.5-B.7 (message/segment structure and
  responses); C.1-C.8 and C.10 (dialog initialization, termination, interruption,
  synchronization, and version support); D (BPD); E (UPD); F (FinTS processes);
  G (data dictionary, including HNHBK, HNHBS, HKIDN, HKVVB, HKSYN/HISYN,
  HKEND, HIRMG/HIRMS, HIBPA, HIUPA, and HIUPD); H.1 (wire syntax); I.2
  (message order); I.3 (character-set overview); I.4 (transport-specific rules).
- **Apply alongside:** the Gate 1 Formals/protocol correction-register entries
  enumerated below, plus the current return-code volume.
- **Authorizes:** the Gate 1 wire codec, dialog and synchronization lifecycle,
  BPD/UPD handling, product fields, response envelopes, and protocol state
  numbering.
- **Access/redistribution:** the PDF grants implementation use and permits only
  free, unchanged redistribution with all notices and conditions retained. No PDF
  is committed; owner approval remains required before any redistribution.

### ZKA — HBCI 2.2 Erweiterung PIN/TAN

- **Protocol/release:** HBCI 2.2 PIN/TAN, Version 1.01, 2002-05-08.
- **Official source:** [official FinTS specification
  archive](https://www.fints.org/de/spezifikation/archiv) (research filename
  `HBCI_22_Erweiterung_PINTAN_1.01_2002-05-08.pdf`).
- **Research SHA-256:** `786a3c4e0a656100237edf7895f1ead9bea076dcb6ef49716a56eb4f6f409604`.
- **Gate 1 sections:** VI.7 (HTTPS communication access), specifically the
  `Filterfunktion` code `MIM` for MIME Base 64 and the requirement to filter the
  complete message.
- **Apply alongside:** FinTS 3.0 Formals Data Dictionary `Filterfunktion`
  (`MIM` = MIME Base 64; complete-message filtering) and the HIKOM 4 occupancy
  rule that leaves `Filterfunktion` unoccupied for communication service 3
  (HTTPS).
- **Authorizes:** the inherited interoperable HTTPS body mapping: send the complete
  FinTS message Base64-encoded as the HTTP POST request body and Base64-decode the
  complete response body before FinTS parsing. A response that is not valid Base64
  is a transport error; it is never retried as raw FinTS.
- **Access/redistribution:** accessed from the official archive on 2026-07-28.
  Redistribution rights for this historical PDF have not been reviewed, so the
  research copy remains ignored under `local/` and is not committed.

### FinTS — Financial Transaction Services, Schnittstellenspezifikation, Sicherheitsverfahren PIN/TAN

- **Protocol/release:** Version 3.0-FV Release 2020, Final Version Release 2020,
  2020-07-10.
- **Official source:** [FinTS 3.0 specification download
  page](https://www.fints.org/de/spezifikation) (official filename
  `FinTS_3.0_Security_Sicherheitsverfahren_PINTAN_2020-07-10_final_version.pdf`).
- **Research SHA-256:** `77bb5724cb391434cd7188924a1ff89bff040bf340dc49a09c92cdeb4ee7dc3a`.
- **Gate 1 sections:** A (relationship to Formals and HBCI Security); B.1-B.3
  (PIN/TAN profile, authentication classes, and SCA); B.4 (dialog
  initialization, including anonymous initialization); B.5 (HKTAN/HITAN
  two-step and decoupled flows); B.6 (challenge data); B.8.1 (HIPINS); B.8.2
  (HITANS); B.9 (PIN/TAN occupancy of security segments); C.3.1 (HKTAB/HITAB
  TAN-media discovery); D (two-step method parameters); F.1-F.2 (message
  composition and examples).
- **Apply alongside:** T34, T33, T31, T21, T8, and T2 from the correction
  register. T32 is incorporated by this Release 2020 PDF.
- **Authorizes:** the Gate 1 PIN/TAN profile, supported-method selection,
  HKTAN/HITAN 6 and 7 flows, typed TAN and decoupled approval continuations,
  HIPINS/HITANS interpretation, and HKTAB/HITAB 5 media discovery.
- **Access/redistribution:** the same implementation grant and unchanged,
  free-redistribution conditions as the Formals PDF. No PDF is committed.

### FinTS — Financial Transaction Services, Schnittstellenspezifikation, Sicherheitsverfahren HBCI

- **Protocol/release:** Version 3.0-FV, Final Version, 2024-06-11.
- **Official source:** [FinTS 3.0 specification download
  page](https://www.fints.org/de/spezifikation) (official filename
  `FinTS_3.0_Security_Sicherheitsverfahren_HBCI_Rel_2024-06-11_final_version.pdf`).
- **Research SHA-256:** `37a82f7e51386f605154f1d5f47d64dc97ca3cab7161bc8769b9c7ca0f32735e`.
- **Gate 1 sections:** B.5 only, limited to the common security-envelope
  structures HNSHK 4, HNSHA 2, HNVSK 3, and HNVSD 1 that the PIN/TAN volume
  normatively reuses.
- **Apply alongside:** the PIN/TAN volume's B.9 occupancy rules. HBCI
  key/card security profiles are not in Gate 1.
- **Authorizes:** only the shared envelope segment layouts needed to encode the
  PIN/TAN profile; it does not authorize implementing an HBCI security profile.
- **Access/redistribution:** the PDF contains the same implementation grant and
  unchanged, free-redistribution conditions. No PDF is committed.

### FinTS — Financial Transaction Services, Schnittstellenspezifikation, Messages — Multibankfähige Geschäftsvorfälle

- **Protocol/release:** Version 3.0 Release 2022, FV, 2022-04-15.
- **Official source:** [FinTS 3.0 specification download
  page](https://www.fints.org/de/spezifikation) (official filename
  `FinTS_3.0_Messages_Geschaeftsvorfaelle_2022-04-15_final_version.pdf`).
- **Research SHA-256:** `a3db32dc27b596f67dea0d4447d07011d8945dc589aafcacb869018ea7d94afd`.
- **Gate 1 sections:** B.2 (account identifiers), B.4 (balance structure),
  C.2.1.2.1-C.2.1.2.3 (HKSAL/HISAL/HISALS versions 6, 7, and 8), and the
  related Data Dictionary entries for amount, booked balance, pending balance,
  credit/debit sign, currency, date, time, and timestamp.
- **Apply alongside:** G102/CR 525 is already incorporated into this Release
  2022 volume. The current return-code volume still applies.
- **Authorizes:** negotiated Gate 1 balance requests and responses for segment
  versions 6-8 and their typed account, amount, currency, sign, date/time, and
  optional-value semantics.
- **Access/redistribution:** the same implementation grant and unchanged,
  free-redistribution conditions. No PDF is committed.

### FinTS — Financial Transaction Services, Schnittstellenspezifikation, Rückmeldungscodes — Ergänzung zum Band “FinTS Formals”

- **Protocol/release:** Version 3.0 / 4.x FV, 2026-02-03.
- **Official source:** [FinTS correction and extension
  register](https://www.fints.org/de/spezifikation/aenderungen) (official filename
  `FinTS_Rueckmeldungscodes_2026-02-03_FV.pdf`).
- **Research SHA-256:** `f0ab2a40c93921a31b715c6006e683ebd9ca7a4d3a842120b8c75d2d535c7b2d`.
- **Gate 1 sections:** A and B.1-B.4, especially status classes and codes 0020,
  0030, 0100, 3010, 3040, 3050, 3072, 3075, 3076, 3081, 3920, 3955-3958,
  9000, 9075, 9078, 9110, 9130, 9185, 9210, 9380, 9391, 9800, 9942, 9951,
  and 9997.
- **Apply alongside:** Formals B.6-B.7 and the operation-specific selected
  response-code examples. The online register has the same 2026-02-03 release
  date and must be checked for later changes before implementation.
- **Authorizes:** typed Gate 1 success, warning, error, continuation, SCA,
  synchronization, and indeterminate-status outcomes.
- **Access/redistribution:** the same implementation grant and unchanged,
  free-redistribution conditions. No PDF is committed.

## Mandatory corrections and extensions

The official [FinTS correction and extension
register](https://www.fints.org/de/spezifikation/aenderungen) is a live,
rights-reserved web publication (current release shown on access: 2026-02-03).
It is authoritative alongside the base PDFs and is not redistributable merely
because it is publicly accessible.

Gate 1 applies these entries:

- **Formals/protocol:** P26 (2025-01-02, BPD version changes and client-only
  version zero), P24 (2021-11-12, HIUPD account type mandatory for payment
  accounts), P23 (2021-07-07, code 9997), P22 (2019-07-22, code 3081), P21
  (2018-09-26, product/security warnings and errors), P20 (2018-02-23,
  indeterminate code 9000), P18 (2016-09-20, UPD extension and FinTS
  processes, incorporated by Formals 2017), P17 (2016-06-29, code 9185), P16
  (2014-12-16, no SSLv3 for PIN/TAN), P15 (2014-08-28, customer product rather
  than internal library identity), P12 (2013-04-11, code 3072), P10
  (2012-03-23, code 9391), P9 (2012-03-23, no compressed bank response unless
  the client sent compressed data), and P4 (2006-06-10, UPD version zero
  handling).
- **PIN/TAN:** T34 (2023-01-26, HKTAN 6/7 HHD_UC expiry handling), T33
  (2020-12-18, corrected HKTAN 6/7 `Segmentkennung` occupancy), T32
  (2020-07-10, decoupled flow; incorporated in Release 2020), T31
  (2019-09-12, balance/turnover SCA clarification), T21 (2010-07-07, user
  choice when 3920 returns multiple methods), T8 (2008-02-29, anonymous BPD
  refresh when 3920 has no usable method), and T2 (2006-12-04, valid method
  required for synchronization).
- **Messages:** G102/CR 525 (2022-02-24, HKSAL version 8), incorporated into
  the Messages Release 2022 PDF.

Two correction attachments were retained temporarily for exact review:

- **T31 attachment:** `CR0511_Anl1_Klarstellung_zur_Umsatzabfrage_ohne_TAN_3.0_FV.pdf`,
  Security PIN/TAN B.3 page 21, 2019-09-12; SHA-256
  `a1f7736f1bacd4bd9770dff8cfcf5dddd3eb880e8a56be225f40e353efc9cae5`.
  It authorizes distinguishing a statically TAN-exempt balance request from an
  HKTAN-accompanied request whose SCA exemption is decided at execution time.
- **T33 attachments:** `CR0517_Fehlerkorrektur·HKTAN_6.pdf` (SHA-256
  `dfc8f99baf55e550eed99fa1d62700d65f748d780b1bde601c5c23b179c798cf`)
  and `CR0517_Fehlerkorrektur·HKTAN_7.pdf` (SHA-256
  `a69f25759028f5d9753258a65320d8ff4f83b9588588abd6781adf08f1f3301a`),
  correcting Security PIN/TAN B.5 pages 58 and 65. They authorize only the
  corrected `Segmentkennung` restrictions.

The correction attachments inherit the applicable specification restrictions and
are not committed.

## Official policy and directory sources

- [FinTS V3.0 Eigenschaften](https://www.fints.org/de/spezifikation/fints-v30-eigenschaften),
  accessed 2026-07-28, establishes the four-volume source structure and the
  distinct Formals, HBCI Security, PIN/TAN Security, and Messages authorities.
- [FinTS product registration](https://www.fints.org/de/hersteller/produktregistrierung),
  accessed 2026-07-28, requires the registration number in HKVVB
  `Produktbezeichnung` on every dialog initialization and a separately supplied
  `Produktversion`.
- [FAQ Produktregistrierung](https://www.fints.org/de/hersteller/faq-produktregistrierung),
  revision 2023-06-28, accessed 2026-07-28, establishes that the recognizable
  customer application supplies the operational registration identity; a library
  or kernel registration is for internal testing only.
- [Bedingungen zur Nutzung der FinTS-Bankenliste](https://www.fints.org/de/hersteller/bankenliste),
  accessed 2026-07-28, establishes that the voluntary, irregularly updated list is
  neither complete nor current, may not be redistributed in a software product,
  and may not authorize institutes or security methods. Live BPD is authoritative
  for those capabilities.

These web pages are rights-reserved. No bank-list data, product registration value,
or owner data is stored in this repository.

## Non-protocol implementation references

These sources do not define FinTS behavior. They constrain only a future Rust
implementation and must be rechecked against the dependency versions selected in
the implementation commit:

- [Rust `Debug`](https://doc.rust-lang.org/stable/std/fmt/trait.Debug.html):
  derived formatting exposes fields, so secret-bearing protocol types must omit it
  or provide deliberately redacted formatting.
- [`reqwest` redirect policy](https://docs.rs/reqwest/latest/reqwest/redirect/struct.Policy.html)
  and [blocking response](https://docs.rs/reqwest/latest/reqwest/blocking/struct.Response.html):
  redirects can be disabled and a response can be read incrementally through
  `std::io::Read`, permitting a concrete bounded HTTPS transport without a
  transport trait or a direct async-runtime dependency.
- [`encoding_rs::mem`](https://docs.rs/encoding_rs/latest/encoding_rs/mem/index.html):
  provides strict Latin-1 range checks and Latin-1/UTF-8 conversion primitives;
  label lookup must not be used because the web-encoding label `ISO-8859-1`
  resolves as Windows-1252.
- [`chrono`](https://docs.rs/chrono/latest/chrono/): provides checked parsing and
  construction for protocol-defined dates and times.
- [`serde` derive](https://serde.rs/derive.html): applies only to the explicitly
  reusable Gate 1 synchronization state named in `SCOPE.md`.
- [`thiserror`](https://docs.rs/thiserror/latest/thiserror/): supports typed errors,
  but all error text and source conversion still require an explicit redaction
  review.

As corroboration only, the deployed
[`python-fints` HTTPS transport](https://github.com/raphaelm/python-fints/blob/master/fints/connection.py)
uses the inherited complete-message Base64 mapping. It is not authority for message
content, security, dialog behavior, or error handling.

## Repository rule

Do not implement a segment or dialog behavior until its exact source is recorded
here. Tests must cite the applicable section and use independently written fictional
messages rather than copied confidential or live data. Do not commit specification
PDFs unless their redistribution terms have been reviewed and the owner explicitly
requests it.
