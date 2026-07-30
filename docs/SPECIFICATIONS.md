# Specification Source Register

Official Deutsche Kreditwirtschaft (DK) FinTS specifications are authoritative for
this crate. A base PDF must be read together with the published correction and
extension register: a newer-looking PDF does not by itself supersede later entries in
that register.

Specification PDFs were downloaded only into the ignored `local/` directory for this
research pass. They are not part of the repository. Unless stated otherwise, the
access date for each source below is 2026-07-28.

## FinTS normative documents

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
- **Gate 3 sections:** C.3.2.2-C.3.2.3 (retain same-version BPD/UPD and
  completely replace them when their versions change); D.1-D.3 and the
  HIBPA/HIKOM Data Dictionary entries (institute identity and communication
  parameters); E.3 and the HIUPD 6 Data Dictionary entry (account-bound and
  non-account-bound UPD records, including the published 35-character IBAN
  correction).
- **Gate 4 sections:** B.6 (opaque pagination); E.3 and the HIUPD 6 Data
  Dictionary entries for account type and account/depot number. Account types
  30-39 identify securities depots and 50-59 identify credit-card accounts;
  advertised allowed operations still authorize each concrete request.
- **Gate 4 live-interoperability sections:** B.7.1 (an institute response places
  optional HNSHK 4 before its mandatory HIRMG 2 and optional HNSHA 2 before
  HNHBS); B.8 (HNVSK follows HNHBK and HNVSD contains the complete logical
  segment sequence, including any security controls, with continuous numbering);
  B.4.1-B.4.2 (`bin`, `code`, `dat`, and `tim` format and restriction rules);
  B.7.5.2 (class 0 acceptance, class 3 warnings, class 9 rejection of the
  referenced message or segment while valid sibling segments remain
  processable, and additional notices alongside those aggregate outcomes);
  B.7.6 (unsigned `HNHBK+HIRMG+HNHBS` dialog-abort message, including
  `unbekannt` and `9999` sentinels); C.3.2.2 and F.2 process condition [IF3]
  (dialog-transient BPD version zero and version wrap-around); E.2 and the HIUPA
  Data Dictionary entry (`UPD-Verwendung` values 0 and 1); and the message,
  segment, and HIUPD `Erlaubte GV` repetition maxima. C.5-C.5.1 defines the
  unsigned anonymous dialog that retrieves current BPD; HKVVB sends BPD version
  zero when none are retained. C.1.2 defines response 9800 as institute-side
  dialog termination and forbids a later HKEND; C.4.2-C.4.3 require a signed,
  encrypted `HNHBK/HNSHK/HKEND/HNSHA/HNHBS` customer termination and normally
  confirm it with response 0100. C.8/C.8.1-C.8.2 require first-contact
  system-ID acquisition to use a synchronization initialization containing
  HKSYN 3 with HKIDN system ID `0` and status `1`, followed by mandatory HISYN
  4 and dialog termination before any business dialog.
- **Apply alongside:** the Gate 1 Formals/protocol correction-register entries
  enumerated below, plus the current return-code volume.
- **Authorizes:** the Gate 1 wire codec, dialog and synchronization lifecycle,
  BPD/UPD handling, product fields, response envelopes, and protocol state
  numbering; and Gate 3 replacement of changed parameter sets, endpoint
  institute validation, and omission of non-account-bound UPD records from the
  account list. For Gate 4 it authorizes discovering depot and credit-card
  products from UPD without guessing from names or identifiers. For the
  authenticated-response interoperability fix it authorizes restoring and
  validating the logical institute-response order inside HNVSD before requiring
  HIRMG. It also authorizes surfacing exact mid-dialog aborts as bank errors,
  treating omitted allowed-operation entries according to `UPD-Verwendung`, and
  replacing a changed complete BPD without assuming numeric monotonicity. For an
  already-decided parameter-refresh outcome, a validated HKEND response carrying
  the exact global 9050/9800/unpublished-9952 set proves only that the discovery
  dialog ended; it does not assign a meaning to the unpublished companion or
  relax any other termination error. C.3.2.2 requires a BPD-version-zero request
  to receive the complete current BPD; the Formals define no alternate BPD
  acquisition if that anonymous initialization is rejected. It also makes
  complete BPD supplied during initialization immediately active, including
  when a segment-level error rejects only the accompanying HKTAN; absent 9800,
  the otherwise successful dialog still requires HKEND. C.8 additionally
  authorizes combining first system-ID acquisition with the function-999
  initialization instead of sending a regular initialization with system ID
  `0`; the response may carry BPD and must carry HISYN.
- **Access/redistribution:** the PDF grants implementation use and permits only
  free, unchanged redistribution with all notices and conditions retained. No PDF
  is committed; owner approval remains required before any redistribution.

### HBCI — Homebanking-Computer-Interface, Schnittstellenspezifikation, Teil A: Grundsätzliche Festlegungen

- **Protocol/release:** Version 2.2, Final Version, 2000-05-10.
- **Official source:** [official FinTS specification
  archive](https://www.fints.org/de/spezifikation/archiv), archive filename
  `HBCI_V2.x_FV.zip`, contained research filename `HBCI22 Final.pdf`.
- **Research SHA-256:** PDF
  `df1e375d54b1c1a1530787bb9d3f62bea88634aceec656b4b216300f725357e8`;
  containing archive
  `4adb3b0b5778aa82cb5e36ef1163e5a08cadca4defb5a1e594aa9c4517611ebc`.
- **Balance-version-5 sections:** II.5.3.1 (`btg` amount), II.5.3.3 (`ktv`
  national account), II.5.3.4 (`sdo` signed balance and transfer timestamp),
  II.8.4 (response version matches request version), IV.6 (operation
  advertisement by parameter-segment version), and VII.2.2
  (HKSAL/HISAL/HISALS version 5 request, response, optional fields, and
  parameter segment without operation-specific parameters).
- **Retained-BPD sections:** III.3.2.2 (BPD are returned when the supplied
  version differs and, when returned, must be complete) and IV.1-IV.2
  (changed BPD become immediately active and receive a new BPD version).
- **Apply alongside:** FinTS 3.0 Formals C.3.2.2 and D.2, correction P26,
  FinTS 3.0 PIN/TAN including T31, and the current Messages definitions for
  the later HKSAL/HISAL/HISALS versions 6-8. The current change register's
  G102 adds version 8 without changing the archived version-5 contract.
- **Authorizes:** advertised HKSAL/HISAL/HISALS 5 using its exact national
  account request and legacy response layout, mapped into the existing typed
  balance without fabricating absent optional values. It also authorizes
  retaining complete same-version BPD and atomically replacing all retained
  capabilities only when a changed complete BPD version is supplied.
- **Access/redistribution:** accessed 2026-07-29. The PDF grants implementation
  use and permits only free, unchanged redistribution with all notices and
  conditions retained. No archive or PDF is committed; owner approval remains
  required before redistribution.

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
  (HTTPS). MIME Base64 decoding follows
  [RFC 2045 section 6.8](https://www.rfc-editor.org/rfc/rfc2045.html#section-6.8):
  folded lines and transport whitespace are ignored; other non-alphabet
  characters remain rejectable transmission errors.
- **Authorizes:** the inherited interoperable HTTPS body mapping: send the complete
  FinTS message Base64-encoded as the HTTP POST request body and Base64-decode the
  complete response body before FinTS parsing. The bounded decoder accepts MIME
  folding (`SP`, `HTAB`, `CR`, and `LF`) but rejects malformed Base64, HTML, and
  non-whitespace transport garbage; raw FinTS is never a fallback.
- **Access/redistribution:** accessed from the official archive on 2026-07-28.
  Redistribution rights for this historical PDF have not been reviewed, so the
  research copy remains ignored under `local/` and is not committed.

### IETF — RFC 2045, MIME Part One

- **Protocol/release:** Internet Standards Track RFC 2045, *Multipurpose Internet
  Mail Extensions (MIME) Part One: Format of Internet Message Bodies*, November
  1996.
- **Official source:** [RFC Editor canonical
  publication](https://www.rfc-editor.org/rfc/rfc2045.html).
- **Research SHA-256:** not applicable; the canonical RFC remains available from
  the RFC Editor in stable text, HTML, XML, and PDF representations.
- **Gate 4 live-interoperability section:** 6.8, Base64 Content-Transfer-Encoding,
  especially the 76-character folding rule, decoder handling of line breaks and
  whitespace, canonical padding, and permission to reject other non-alphabet
  characters as transmission errors.
- **Apply alongside:** HBCI 2.2 PIN/TAN VI.7 `MIM` and the FinTS 3.0 Formals
  `Filterfunktion` definition. It changes only HTTPS-body decoding, never FinTS
  message syntax or dialog behavior.
- **Authorizes:** accepting bounded MIME-folded and transport-whitespace-wrapped
  Base64 responses while retaining strict rejection of malformed Base64, HTML,
  non-whitespace transport garbage, and raw FinTS fallback.
- **Access/redistribution:** accessed 2026-07-29. RFC 2045 permits unlimited
  distribution; this repository links to the canonical publication.

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
- **Gate 4 live-interoperability sections:** B.1 (personalized PIN/TAN messages
  retain the security and encryption segments); B.4.3.1 (current anonymous BPD
  is a prerequisite for strong-authentication discovery; function 999 must return
  the user's methods through 3920 and may not return UPD); B.6.1 response 3920
  (allowed-method parameters and 9800 dialog termination); B.8.2 (9955
  termination when one-step TAN is unavailable, with the supported methods
  supplied by 3920);
  correction T8 (active anonymous BPD refresh using client version zero per
  Formals C.3.2.2/P26 when 3920 cannot be matched to a usable method
  description);
  the introductory
  `FinTS-Füllwert` definition immediately before B.1 (a filler is any value
  satisfying the field's format, restrictions, and occupancy and is irrelevant
  to processing); B.9.1-B.9.10 (PIN/TAN occupancy of HNSHK, HNSHA, HNVSK, and
  cleartext-binary HNVSD, including B.9.4-B.9.6's HNSHK certificate, hash, and
  filler rules and B.9.9's non-normative filler examples); C.3.1 and the
  TAN-Medium-Liste version 5 Data Dictionary entry (mobile-media name mandatory,
  both phone-number fields optional); F.2 (each institute response has
  HNVSK/HNVSD and may have one HNSHK/HNSHA pair around HIRMG and response data);
  F.2.5 (profile-1, security-function-999 HKEND composition).
  B.4.3.1.3 requires medium discovery to initialize with HKTAN process 4 and
  `Segmentkennung=HKTAB`, with the supplied medium-name filler ignored, then
  return HITAB after successful PIN validation and close the dialog with HKEND.
  It defines this as the first dialog for first use of a method and does not
  require UPD acquisition or an ordinary selected-method initialization first;
  the filler exception applies only to this HKTAB initialization.
  C.3.1.1 defines HKTAB/HITAB 5 and permits an empty repeated medium list only
  when no medium is available. Archived E.2.1.2-E.2.1.4 define matching
  HITABS/HKTAB/HITAB triples for legacy versions 2-4: HKTAB 2/3 carry only
  `TAN-Medium-Art`, while versions 4/5 also carry `TAN-Medium-Klasse`; their
  responses use the same-numbered TAN-Medium-Liste element version.
  TAN-Medium-Liste 2-4 fields 1-5 occupy flat components 1-5, nested field 6
  `ktv` occupies components 6-9, fields 7-9 occupy components 10-12, and field
  10 `Bezeichnung des TAN-Mediums` is therefore component 13. Version 3 adds
  the masked phone at component 14; version 4 adds masked/plain phones at
  components 14/15. Version 5 inserts the security function before the card
  fields and moves nested `ktv` to field 7/components 7-10, list number to
  component 13, designation to component 14, and masked/plain phones to
  components 15/16. The later nested `kti` occupies six flat components in
  every version. Card number and sequence precede `ktv` (components 3/4 in
  versions 2-4 and 4/5 in version 5), so their existing mapping is unchanged.
  Status is always component 2. Because the crate neither exposes nor consumes
  card number, card sequence, list number, unmasked phone, `ktv`, or `kti`, the
  acceptance-space policy reads their occupancy past; received identifiers are
  not retained and no placeholder is fabricated.
  Formals C.10 makes a BPD
  parameter-segment version the advertisement of that same operation version
  and calls for the highest common version. F.2 separates dialog
  initialization from subsequent order messages, authorizing a same-dialog
  versioned HKTAB order when the process-4 initialization response has not
  already supplied HITAB. The HKTAN 6 and 7 entries place the optional medium
  name at DE 12. Their shared method-parameter DD defines a 21-component
  version-6 method block and a 26-component version-7 block. Both put
  `Bezeichnung des TAN-Mediums erforderlich` at field 19 (zero-based parser
  index 18) and `Anzahl unterstützter aktiver TAN-Medien` at optional field 21
  (index 20); DE 12 is mandatory only when field 19 is `2` and field 21 is
  greater than one. The count belongs to that repeated method's BPD block and
  describes the method's supported active media; it is not scoped by access or
  medium class, and no rule replaces it with the number of records returned for
  one user by HITAB. Archived E.2.1.4 and the DD define HKTAB 4
  `TAN-Medium-Art=0` as all media and `TAN-Medium-Klasse=A` as all classes, so
  `+0+A` requests the complete relevant set. In TAN-Medium-Liste 4, field 10
  `Bezeichnung des TAN-Mediums` is component 13 after flattening nested `ktv`;
  it is mandatory for class `M` and optional for class `G`. Class-G card number
  and card sequence do not become HKTAN DE 12 selectors, and neither E.2.1.4 nor
  the Data Dictionary defines an implicit selection for a sole unnamed record.
  Correction T17
  introduced HKTAB/HITAB 4 and is incorporated in Release 2020's archived E.2.1.4;
  correction T33 permits `Segmentkennung` during process 4 for both HKTAN
  versions. The
  corresponding HITAN 6 and 7 entries
  permit a process-4 institute response and require its order reference for that
  process; their `noref`/challenge filler rule records that no TAN is required.
  Neither that HITAN nor the correction register substitutes for the HKTAB
  result or defines a parameter-refresh continuation; B.4.3.1.3 still requires
  HITAB before the client closes the dialog. The HITAB 4 response additionally
  carries mandatory `TAN-Einsatzoption` (`0`, `1`, or `2`), which describes
  parallel-use policy but does not waive HKTAN DE 12. The owner-attended
  response with occupied components 1, 2, and 13 therefore contained a valid
  class-G designation. The earlier `TanMediumUnavailable` result came from
  treating DD field 10 as flat component 10 and was an implementation defect,
  not a contradictory institute parameter set. The selector omission/empty
  experiment based on that premise has been removed without adding either wire
  shape to supported behavior.
- **Apply alongside:** T34, T33, T31, T21, T8, and T2 from the correction
  register. T32 is incorporated by this Release 2020 PDF.
- **Authorizes:** the Gate 1 PIN/TAN profile, supported-method selection,
  HKTAN/HITAN 6 and 7 flows, typed TAN and decoupled approval continuations,
  HIPINS/HITANS interpretation, and HKTAB/HITAB 2-5 media discovery. The
  medium-discovery sections authorize requiring HITAB after an accepted request
  and returning a typed limitation when an advertised required name is not
  selectable. One unnamed class-G record remains usable when the method does not
  require a name, but it cannot satisfy mandatory HKTAN DE 12: the crate neither
  fabricates a name nor substitutes discarded card identifiers. When the
  discovery initialization supplies current BPD, its applied HITANS condition
  governs this decision rather than the pre-dialog parameter snapshot. They do
  not authorize using `noref` as a selected medium in an ordinary initialization
  or acquiring UPD before first-access HKTAB discovery.
  A process-4 HITAN is validated as the response to the embedded HKTAN. If the
  same response omits HITAB, the archived/current operation definitions
  authorize sending the highest common advertised HKTAB 2-5 order in that
  open dialog; the matching HITAB remains mandatory. B.4.3.1 and
  F.2.5 additionally authorize closing an open function-999 discovery with a
  profile-1, security-function-999 HKEND using the active dialog state. It also
  authorizes treating the specification-defined function-999
  9050/9800/9955/3920 response set as completed method discovery without a
  client-side HKEND. T8 additionally authorizes a typed anonymous-parameter
  refresh when valid 3920 identifiers lack matching HITANS descriptions. Because
  the identifiers are institution-specific and only BPD/HITANS describes them,
  an anonymous refresh that is itself terminated without BPD leaves no compliant
  bootstrap path; the crate returns a typed limitation instead of retrying or
  constructing a method.
  Correction T2 additionally requires the institute to return a user-valid
  one- or two-step method when HKSYN requests a new system ID, completing the
  function-999 first-contact bootstrap without interpreting response text.
  If a BPD-zero client instead receives the exact global
  9050/9800/unpublished-9952 termination without mandatory 3920, B.4.3.1 and T8
  support one bounded repair of the missing anonymous-BPD prerequisite followed
  by one fresh function-999 attempt. The absent 3920 remains a protocol
  deviation, and the unpublished companion receives no inferred meaning.
  Unpublished 99xx companions receive no standalone meaning and known errors
  retain their normal meaning. It further
  authorizes strict inbound PIN/TAN security-control occupancy while accepting
  format-valid filler contents without evaluating their concrete values; it
  does not authorize a raw or loosely searched response sequence.
- **Access/redistribution:** accessed 2026-07-30; the same implementation
  grant and unchanged, free-redistribution conditions as the Formals PDF. No
  PDF is committed.

### FinTS — Financial Transaction Services, Schnittstellenspezifikation, Sicherheitsverfahren HBCI

- **Protocol/release:** Version 3.0-FV, Final Version, 2024-06-11.
- **Official source:** [FinTS 3.0 specification download
  page](https://www.fints.org/de/spezifikation) (official filename
  `FinTS_3.0_Security_Sicherheitsverfahren_HBCI_Rel_2024-06-11_final_version.pdf`).
- **Research SHA-256:** `37a82f7e51386f605154f1d5f47d64dc97ca3cab7161bc8769b9c7ca0f32735e`.
- **Gate 1 sections:** B.5 only, limited to the common security-envelope
  structures HNSHK 4, HNSHA 2, HNVSK 3, and HNVSD 1 that the PIN/TAN volume
  normatively reuses.
- **Gate 4 live-interoperability sections:** B.5.1-B.5.4, limited to the shared
  segment layouts and the placement of HNSHK/HNSHA inside the encrypted HNVSD
  sequence; Data Dictionary entries `Rolle des Sicherheitslieferanten, kodiert`
  (codes 1, 3, and 4; currently not to be interpreted), `Bezeichner für
  Sicherheitspartei` (codes 1 and 2), `Sicherheitsdatum und -uhrzeit` (optional
  date and conditional time, including a timestamp-type-only HNSHK),
  `Hashalgorithmus`, `Signaturalgorithmus`, `Schlüsselname`, and
  `Verschlüsselungsalgorithmus` (component formats, 512-byte binary bound,
  encryption algorithms 13/14, modes 2/18, identifiers 5/6 and 1, and
  unoccupied IV value). PIN/TAN B.9 remains the authority for profile-specific
  occupancy and makes these cleartext fields meaning-neutral fillers.
- **Apply alongside:** the PIN/TAN volume's B.9 occupancy rules. HBCI
  key/card security profiles are not in Gate 1.
- **Authorizes:** only the shared envelope segment layouts and field restrictions
  needed to encode and validate the PIN/TAN profile, including cut or explicitly
  empty optional HNSHK date/time components; it does not authorize an HBCI
  security profile.
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
- **Gate 2 sections:** C.2.1.1.1 (HKKAZ/HIKAZ/HIKAZS versions 6 and 7) and
  C.2.3.1.1.1 (HKCAZ/HICAZ/HICAZS version 1), including their request account
  groups, date ranges, maximum-entry and Aufsetzpunkt fields, binary booked-data
  response groups, BPD parameter segments, and the related Data Dictionary
  entries. HKCAZ is preferred; HKKAZ 7 and then 6 are the bounded legacy
  fallbacks only when advertised and authorized.
- **Gate 3 sections:** B.2.3 (national and international account
  identifications), C.2.1.1.1, C.2.1.2.1-C.2.1.2.3, and C.2.3.1.1.1,
  covering the independently negotiable HKKAZ 6/7, HKSAL 6/7/8, and HKCAZ 1
  profiles and their BPD parameter segments.
- **Gate 4 sections:** C.4.3.1 (HKWPD/HIWPD/HIWPDS version 6 depot
  positions) and C.4.3.2 (HKWDU/HIWDU/HIWDUS version 5 booked depot
  transactions), including the request fields, binary response payloads, BPD
  parameters, and operation-specific pagination fields.
- **Apply alongside:** G102/CR 525 is already incorporated into this Release
  2022 volume. The current return-code volume still applies.
- **Authorizes:** negotiated Gate 1 balance requests and responses for segment
  versions 6-8 and their typed account, amount, currency, sign, date/time, and
  optional-value semantics. Formals cut rules permit trailing optional DEG
  components to be omitted; result parsers consume the mandatory prefix and
  read past unused trailing components without weakening mandatory identity or
  amount validation. The HISALS segment-header version is the generic
  advertised-version fact used for negotiation, including when the version is
  outside the 6-8 range defined by this document; the archived HBCI source
  above separately authorizes version 5. It also authorizes Gate 2 HKCAZ 1 or
  bounded HKKAZ 7/6 booked transaction retrieval, including the
  operation-specific continuation field. For Gate 3 it authorizes
  deterministic selection from the advertised operation/version combinations,
  never institution-name dispatch. For Gate 4 it authorizes only advertised
  and UPD-authorized HKWPD 6 and HKWDU 5 using the specified MT535 and MT536
  response formats.
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
  9000, 9050, 9075, 9078, 9110, 9130, 9185, 9210, 9380, 9391, 9800, 9942,
  9951, 9955, and 9997.
- **Gate 4 live-interoperability sections:** A (the 0900-0999, 3900-3999,
  and 9900-9999 exception ranges historically permit institution-specific and
  differing meanings), B.2 (class-1 notice codes, explicitly marked FinTS
  4-only by this joint register), and B.4 (the complete published 99xx
  error-code set).
  B.2 defines 1040 and 1050 as notices that the prior BPD/UPD is outdated and
  the current version is included; neither prescribes another dialog or refresh
  after the included parameter segments are applied. Code 0940 is within
  A's historical exception range but is absent from the register even though A
  says meanings observed in those ranges are listed, so it has no
  specification-defined control-flow meaning. B.2 marks 1040 as FinTS 4-only
  and message-level; its segment-referenced FinTS 3 use remains only a bounded,
  non-fatal interoperability tolerance. Absence from the published entries for
  an exception range does not authorize assigning an unpublished code a
  semantic meaning; it only permits retaining it as a redacted generic fact.
  The B.2 class shape and non-error meaning also bound an owner-approved
  FinTS 3 interoperability tolerance after a conforming four-digit 1xxx code
  was observed there; it does not change aggregate result handling.
- **Gate 2 sections:** B.1-B.4, especially code 3040 and its mandatory
  Aufsetzpunkt parameter, plus 9210 for a rejected continuation point.
- **Gate 3 sections:** B.1-B.4, especially 3050 and 3081 (parameter refresh),
  3076 (SCA not required), 9075 (strong authentication required), and 9185
  (unsupported or obsolete FinTS/HBCI version).
- **Gate 4 sections:** B.1-B.4, especially 3010 (no entries or temporarily
  unavailable information), 3040 (partial response with continuation point),
  and 9210 (invalid account/depot binding).
- **Apply alongside:** Formals B.6-B.7 and the operation-specific selected
  response-code examples. The online register has the same 2026-02-03 release
  date and must be checked for later changes before implementation.
- **Authorizes:** typed Gate 1 through Gate 3 success, notice, warning, error,
  pagination, SCA, synchronization, parameter-refresh, and unsupported-version
  outcomes. In TAN-media discovery, 1040 and 1050 authorize applying the
  included BPD/UPD, but neither they nor unpublished 0940 authorize
  substituting HITAN for HITAB,
  retrying discovery, or starting another initialization. It also defines the
  bounded notice shape used by the narrow FinTS 3
  compatibility tolerance. Bank text remains reachable only through explicit
  caller accessors and absent from crate-owned formatting.
- **Access/redistribution:** accessed 2026-07-30; the same implementation
  grant and unchanged, free-redistribution conditions. No PDF is committed.

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
  choice when 3920 returns multiple methods), T17 (2009-12-15, HKTAB/HITAB 4;
  incorporated in Release 2020's archive), T8 (2008-02-29, anonymous BPD refresh
  when 3920 has no usable method), and T2 (2006-12-04, valid method required for
  synchronization).
- **Messages:** G102/CR 525 (2022-02-24, HKSAL version 8), incorporated into
  the Messages Release 2022 PDF.

Gate 2 additionally applies these entries:

- **G97** (2020-07-10): a valid camt.052 response may contain more than one
  booking day; transaction parsing must not assume one report date.
- **G108** (2023-01-26): camt.052.001.08 examples for HKCAZ, including
  continuation within a booking day and between booking days.
- **T31** (2019-09-12): turnover queries may be statically TAN-exempt or may
  require an HKTAN whose SCA exemption is decided during execution.

Gate 3 additionally applies **P22**, **P24**, and **P26** to every fictional
profile: replace parameter sets after code 3081, retain the account-type
requirement for payment accounts, and never persist a version-zero BPD. A received
version-zero BPD is usable only for its current dialog and replaces no reusable
capability state. These corrections authorize protocol-level compatibility only;
they do not authorize institution-specific branches.

Gate 4 additionally applies **G112** (2024-09-30), which defines the FinTS 3.0
credit-card balance and booked-transaction operations. No correction-register
entry changes the current HKWPD 6 or HKWDU 5 layouts.

## Financial-data documents

### DFÜ-Abkommen, Anlage 3 — Spezifikation der Datenformate

- **Protocol/release:** Version 3.9, Final Version, 2025-03-12.
- **Official source:** [valid DK data-format
  version](https://www.ebics.de/de/datenformate/gueltige-version) (official
  filename `Anlage_3_Datenformate_V3.9.pdf`).
- **Research SHA-256:** `218ceb0c0962a14eaf6f18960e158dd0e5ad0954eebdbe4132e3f09a3b58a6b3`.
- **Gate 2 sections:** 7, especially 7.1.1 (message pagination), 7.1.2
  (account), 7.1.6 (entry), 7.1.7 (transaction details), and 7.2
  (camt.052 occupancy). Gate 2 accepts only `BOOK` entries from
  `camt.052.001.08`, preserves supplied entry and account-servicer references,
  and keeps optional data absent when the XML omits it. The ISO `xs:decimal`
  and `xs:date` lexical spaces apply, and `SplmtryData/Envlp` may contain
  foreign-namespace extension data with no Gate 2 result semantics. Optional
  transaction-detail `Amt` and `BkTxCd` values remain absent when omitted or
  incomplete; the entry/detail sum rule applies only when detail amounts were
  supplied, while supplied directions and sums remain consistent with the entry.
- **Apply alongside:** G97 and G108 from the FinTS correction register and the
  FinTS Messages HKCAZ/HICAZ version 1 transport segments.
- **Authorizes:** namespace-aware, UTF-8 camt.052.001.08 parsing of the booked
  response payload returned by HICAZ, including exact amounts, booking/value
  dates, transaction references, counterpart data, and remittance information
  only where supplied. The advertised and echoed descriptor is compared after
  the Data Dictionary's optional `.xsd` suffix and ASCII case are normalized;
  delivery in another bound camt namespace remains a typed limitation.
- **Gate 4 sections:** 4.3 (MT535 `Statement of Holdings`, SRG 1998) and
  4.4 (MT536 `Statement of Transactions`, SRG 1998), including GENL/FIN,
  SUBBAL, TRAN/TRANSDET, instrument identifiers, quantities, prices, position
  values, structured cost-basis data, transaction amounts, dates, directions,
  reversal status, references, and continuation indicators.
- **Gate 4 acceptance note:** the document's `:22H::PAYM//FREE`,
  `:22F::TRAN//` value set, GENL constants, SUBBAL occupancy, and structured
  `70E::HOLD` line-number rules remain the recorded normative findings. The
  crate does not branch on the PAYM/GENL/SUBBAL values or expose SUBBAL, and it
  exposes `TRAN` only as an opaque institution value. Under the acceptance-space
  policy those values are therefore read past; malformed optional HOLD content
  yields an absent cost basis rather than discarding otherwise typed positions.
  Block pairing, safe identity, instrument, quantity, amount, direction, date,
  reference, pagination-indicator, and response-size checks remain enforced.
- **Gate 4 authorizes:** bounded SWIFT MT535 parsing for explicitly supplied
  depot positions and MT536 parsing for explicitly supplied booked securities
  transactions. Free text is never reinterpreted as a missing typed amount,
  fee, reference, or identifier.
- **Access/redistribution:** the official PDF is rights-reserved and was
  downloaded only to ignored `local/` for research. It is not committed or
  redistributed.

## Gate 4 credit-card extension

### FinTS correction G112 — Salden- und Umsatzabfrage für Kreditkarten

- **Protocol/release:** FinTS 3.0 Messages extension G112 / CR 538 Annex 1,
  Final Version, 2024-09-30.
- **Official source:** [FinTS correction and extension
  register](https://www.fints.org/de/spezifikation/aenderungen), entry G112
  (official filename
  `CR0538_Anl1_Neue_GV_zur_Salden-_und_Umsatzabfrage_für_Kreditkarten_FV.pdf`).
- **Research SHA-256:** `7a33aa90f4307f8d11bf7525c2f137fd0f82c84e8ed0fdbe30518803a8522d73`.
- **Gate 4 sections:** B.8 `Betrag mit Soll/Haben-Kennung`; C.12.1
  HKKKU/HIKKU/HIKKUS version 1; C.12.2 HKKKS/HIKKS/HIKKSS version 1; and
  the extension Data Dictionary entries for card identity, conditional
  international account binding, balances, dates, repeated booked entries,
  exact signed amounts, descriptions, merchant data, fee codes, and
  bank-supplied booking references.
- **Gate 4 acceptance note:** a partially occupied optional `Originalbetrag`
  group cannot produce a typed amount and is treated as absent. Complete groups
  remain fully validated, mandatory booked amounts remain mandatory, and unused
  trailing DEG components are read past.
- **Apply alongside:** Formals B.6 pagination, HIUPD account types 50-59 and
  allowed-operation entries, HIPINS TAN requirements, and the current
  return-code definitions for 3010, 3040, and 9210.
- **Authorizes:** advertised and UPD-authorized credit-card balance and booked
  transaction retrieval with conditional account binding, exact optional-value
  semantics, and exhaustive opaque continuation. A response balance is not
  derived from or reconciled to the returned entries.
- **Access/redistribution:** accessed 2026-07-28. Redistribution rights were
  not reviewed; the research copy remains ignored under `local/` and is not
  committed.

### DFÜ-Abkommen, Anlage 3 — Spezifikation der Datenformate (archived MT940 rules)

- **Protocol/release:** Version 3.8, Final Version, 2024-04-08.
- **Official source:** [official DK data-format
  archive](https://www.ebics.de/de/datenformate/archiv), archive
  `Anlage3_Archiv_V3_8.zip`, containing
  `Anlage_3_Datenformate_V3.8.pdf`.
- **Research SHA-256:** PDF
  `c22b5bf6d6d0c8557e0f69ca8cd97412d5f1c2fb711f77a844c3627412ea60ee`
  (download archive
  `84448f62d6bad5953a3745ba1a9f991c6da538605c29502b0790564cb488c66d`).
- **Gate 2 sections:** 8.1 (syntax), 8.2.1-8.2.4 (MT940 layout, occupancy,
  field 61 booking key, and structured field 86), and 8.2.5 (example).
  Version 3.9 section 8 explicitly marks MT940/MT942 as removed and points to
  this archived version for the last applicable rules.
- **Apply alongside:** FinTS Messages HKKAZ/HIKAZ versions 6 and 7 and Formals
  B.6 pagination. MT942 pending entries are outside Gate 2.
- **Authorizes:** the bounded advertised HKKAZ fallback parser for booked MT940
  entries, preserving the bank reference from field 61 or the exact statement
  sequence and entry position when no bank reference is supplied. Fields 20,
  25, 28C, and 62F/62M retain their specified syntax but do not discard booked
  entries when absent or unusable because their values are not otherwise
  consumed; an exact statement position is exposed only from a valid 28C. The
  60F/60M currency, field-61 amount/direction/code, framing, and bounds remain
  enforced, while over-length field-61 references remain bounded by the binary
  response and are preserved verbatim.
- **Access/redistribution:** the archive and PDF are rights-reserved research
  copies under ignored `local/`; neither is committed or redistributed.

### FinTS Formals pagination rules applied by Gate 2

- **Protocol/release:** FinTS 3.0-FV Formals, Final Version, 2017-10-06; same
  source, SHA-256, and redistribution terms as the Formals entry above.
- **Gate 2 sections:** B.6 `Aufsetzpunkt` and `Maximale Anzahl Einträge`.
  Code 3040 requires automatically resending the same retrieval order with the
  opaque point in the same dialog until complete; the point becomes invalid
  when that dialog ends.
- **Apply alongside:** the current return-code register's 3040 and 9210
  definitions and the operation-specific HKCAZ/HKKAZ field occupancy.
- **Authorizes:** automatic same-dialog pagination with opaque continuation
  points, subject to local repeated-point and page-count safety limits.

### FinTS correction G97 — clarification for booked camt turnover data

- **Protocol/release:** FinTS 3.0 Messages correction G97, clarification,
  2020-07-10.
- **Official source:** [FinTS correction and extension
  register](https://www.fints.org/de/spezifikation/aenderungen), entry G97.
- **Research SHA-256:** not applicable; G97 is published inline in the official
  register and has no separate downloadable artifact.
- **Gate 2 sections:** Messages Data Dictionary elements `Gebuchte camt-Umsätze`
  and `camt-Umsätze gebucht`.
- **Apply alongside:** FinTS Messages HKCAZ/HICAZ 1 and DK Anlage 3 v3.9
  chapter 7.
- **Authorizes:** accepting an otherwise valid camt.052 message whose entries
  cover more than one booking day.
- **Access/redistribution:** accessed 2026-07-28. The live register is
  rights-reserved and is linked rather than copied.

### FinTS correction G108 — camt.052 examples for ISO version 2019

- **Protocol/release:** CR 532 Annex 1, clarification G108, 2023-01-26.
- **Official source:** [FinTS correction and extension
  register](https://www.fints.org/de/spezifikation/aenderungen), entry G108
  `Beispiele für die eindeutige Belegung von camt.052-messages bei HKCAZ für die
  ISO-Version 2019`.
- **Research SHA-256:** `27efb2027c67dc0d2b0895460b1608b6bd0a47aea56e928c61ed7855cc3d0fa9`
  (official filename
  `CR0532_Anl1_Beispiele camt-Umsätze in ISO-Version 2019_FV.pdf`).
- **Gate 2 sections:** the three HKCAZ examples: without an Aufsetzpunkt,
  with an Aufsetzpunkt within one booking day, and with an Aufsetzpunkt between
  booking days.
- **Apply alongside:** Formals B.6, Messages HKCAZ/HICAZ 1, G97, and DK
  Anlage 3 v3.9 chapter 7.
- **Authorizes:** the camt.052.001.08 occupancy used by HKCAZ and exhaustive
  FinTS continuation across and within booking days. Gate 2 still discards
  pending entries.
- **Access/redistribution:** accessed 2026-07-28. Redistribution rights were not
  reviewed; the research copy remains ignored under `local/` and is not
  committed.

### FinTS PIN/TAN correction T31 — turnover query without TAN

- **Protocol/release:** FinTS 3.0 PIN/TAN correction T31, clarification,
  2019-09-12.
- **Official source:** [FinTS correction and extension
  register](https://www.fints.org/de/spezifikation/aenderungen), entry T31
  (attachment
  `CR0511_Anl1_Klarstellung_zur_Umsatzabfrage_ohne_TAN_3.0_FV.pdf`).
- **Research SHA-256:** `a1f7736f1bacd4bd9770dff8cfcf5dddd3eb880e8a56be225f40e353efc9cae5`.
- **Gate 2 sections:** Security PIN/TAN B.3 page 21 and the corrected
  balance/turnover SCA example.
- **Apply alongside:** HIPINS operation TAN status and the selected HKTAN/HITAN
  6 or 7 process.
- **Authorizes:** distinguishing a statically TAN-exempt turnover request from
  an HKTAN-accompanied request whose SCA exemption is decided during execution.
- **Access/redistribution:** accessed 2026-07-28. Redistribution rights were not
  reviewed; the research copy remains ignored under `local/` and is not
  committed.

Two T33 attachments were retained temporarily for exact Gate 1 review:

- `CR0517_Fehlerkorrektur·HKTAN_6.pdf` (SHA-256
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

These sources do not define FinTS behavior. They constrain the Rust implementation
and correspond to the direct dependency versions selected for the supported gates:

- [Rust `Debug`](https://doc.rust-lang.org/stable/std/fmt/trait.Debug.html):
  derived formatting exposes fields, so secret-bearing protocol types must omit it
  or provide deliberately redacted formatting.
- [`reqwest` 0.13.4 redirect
  policy](https://docs.rs/reqwest/0.13.4/reqwest/redirect/struct.Policy.html)
  and [blocking
  response](https://docs.rs/reqwest/0.13.4/reqwest/blocking/struct.Response.html):
  redirects can be disabled and a response can be read incrementally through
  `std::io::Read`, permitting a concrete bounded HTTPS transport without a
  transport trait or a direct async-runtime dependency.
- [`base64` 0.22.1 general-purpose
  engine](https://docs.rs/base64/0.22.1/base64/engine/general_purpose/index.html):
  implements the complete-message Base64 encoding inherited from the historical
  HBCI PIN/TAN HTTPS mapping after bounded removal of RFC 2045 MIME folding
  whitespace; canonical decoding failure remains an explicit transport error.
- [`quick-xml` 0.41.0
  `NsReader`](https://docs.rs/quick-xml/0.41.0/quick_xml/reader/struct.NsReader.html):
  provides streaming namespace-aware XML events for bounded camt.052 parsing,
  with default features disabled. Version 0.41 is the minimum because it fixes
  [RUSTSEC-2026-0195](https://rustsec.org/advisories/RUSTSEC-2026-0195.html)
  by bounding namespace declarations per element; the implementation keeps the
  default cap of 256 or a tighter value and treats XML failures as typed errors.
- [`encoding_rs` 0.8.35
  `mem`](https://docs.rs/encoding_rs/0.8.35/encoding_rs/mem/index.html):
  provides strict Latin-1 range checks and Latin-1/UTF-8 conversion primitives;
  label lookup must not be used because the web-encoding label `ISO-8859-1`
  resolves as Windows-1252.
- [`chrono` 0.4.45](https://docs.rs/chrono/0.4.45/chrono/): provides checked
  parsing and construction for protocol-defined dates and times.
- [`serde` derive](https://serde.rs/derive.html): applies only to the explicitly
  reusable synchronization and BPD/UPD capability state named in `SCOPE.md`.
- [`thiserror` 2.0.19](https://docs.rs/thiserror/2.0.19/thiserror/): supports typed
  errors, but all error text and source conversion still require an explicit
  redaction review.

As corroboration only, [`python-fints`](https://github.com/raphaelm/python-fints)
at commit `e3c916c90eb75745d4959b1fc1aff76b5ba23a19` (accessed 2026-07-28,
LGPL-3.0-or-later) was inspected to identify deployed interoperability questions,
including camt-only, legacy MT940, and SCA-response variations. Its complete-message
Base64 transport also corroborates the inherited HTTPS mapping. It is not authority,
no source or fixture was copied, and every implemented behavior remains independently
justified by the official documents above.

## Repository rule

Do not implement a segment or dialog behavior until its exact source is recorded
here. Tests must cite the applicable section and use independently written fictional
messages rather than copied confidential or live data. Do not commit specification
PDFs unless their redistribution terms have been reviewed and the owner explicitly
requests it.

A finding recorded here that later proves wrong is replaced by the accurate finding
together with one line stating what was incorrect. Superseded conclusions about a real
institution's behavior must not remain in the register.
