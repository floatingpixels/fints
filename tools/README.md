# Structural trace tools

These owner-run scripts turn an ephemeral raw trace into value-free protocol
structure. A raw trace contains credential-bearing and financial data: keep it only
under ignored local storage, never paste or commit it, and delete it after rendering.
Only the scripts' structural output is suitable for an issue or agent prompt.

Both scripts read the crate probe's `hex=<hexadecimal payload>` fields and
Finanzplaner's `payload_hex=<hexadecimal payload>` fields. They decode those payloads
locally and never write files.

## FinTS segment shapes

```sh
tools/fints_segment_shape.sh local/probe.trace
tools/fints_segment_shape.sh local/probe.trace HITAB HITANS HKTAB HKTAN
```

With no identifiers, every recognizable FinTS segment is shown. Additional arguments
restrict output to those segment codes. Output contains segment occurrences,
authenticated-payload nesting depth, element/component counts, and `FILLED`/`EMPTY`
slots only. For example, seeing a 13-component `HITAB` entry whose final slot is
filled answers: “Is the medium designation present but at an unexpected component?”

## SWIFT document shapes

```sh
tools/swift_shape.sh local/probe.trace
```

This finds MT535, MT536, and MT940 binary documents and prints their block paths,
per-tag ordinals, and character-class patterns: `A` for uppercase letters, `a` for
lowercase letters, `9` for digits, and punctuation literally. Repeated classes are
collapsed, such as `A{12}`. Comparing sibling `90B` patterns answers: “Which price
field deviates from the others, and is the difference a blank component, character
class, or digit width?”
