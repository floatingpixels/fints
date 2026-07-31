#!/usr/bin/env bash
# Prints structural FinTS segment facts only: segment codes, element/component
# counts, and filled/empty slots. It never prints values, identifiers, amounts,
# names, credentials, or raw wire data.
#
# Usage:
#   tools/fints_segment_shape.sh <trace-file> [SEGMENT ...]
#
# With no segment identifiers, all FinTS segments are shown.

set -euo pipefail

trace_file="${1:?usage: fints_segment_shape.sh <trace-file> [SEGMENT ...]}"
shift

python3 - "$trace_file" "$@" <<'PY'
import re
import sys


def split_unescaped(text, separator):
    parts = []
    current = []
    index = 0
    while index < len(text):
        character = text[index]
        if character == "?" and index + 1 < len(text):
            current.extend(text[index:index + 2])
            index += 2
            continue
        if character == separator:
            parts.append("".join(current))
            current = []
            index += 1
            continue
        current.append(character)
        index += 1
    parts.append("".join(current))
    return parts


def segment_records(text):
    result = []
    current = []
    current_binary_payloads = []
    index = 0
    while index < len(text):
        character = text[index]
        if character == "?" and index + 1 < len(text):
            current.extend(text[index:index + 2])
            index += 2
            continue
        if character == "@":
            marker = re.match(r"@(\d+)@", text[index:])
            if marker is not None:
                payload_start = index + marker.end()
                payload_end = payload_start + int(marker.group(1))
                if payload_end > len(text):
                    raise SystemExit("trace contains a truncated FinTS binary block")
                current.append("@BINARY@")
                current_binary_payloads.append(text[payload_start:payload_end])
                index = payload_end
                continue
        if character == "'":
            result.append(("".join(current), current_binary_payloads))
            current = []
            current_binary_payloads = []
            index += 1
            continue
        current.append(character)
        index += 1
    if current:
        result.append(("".join(current), current_binary_payloads))
    return result


def segment_stream(text, depth=0):
    if depth > 8:
        raise SystemExit("trace contains excessive nested FinTS binary payloads")
    for segment, binary_payloads in segment_records(text):
        yield depth, segment
        for binary_payload in binary_payloads:
            if re.match(r"[A-Z][A-Z0-9]{4,5}:", binary_payload):
                yield from segment_stream(binary_payload, depth + 1)


def occupancy(parts):
    return ", ".join(
        f"[{index}]{'EMPTY' if part == '' else 'FILLED'}"
        for index, part in enumerate(parts, start=1)
    )


trace_path = sys.argv[1]
wanted = {identifier.upper() for identifier in sys.argv[2:]}
for identifier in wanted:
    if re.fullmatch(r"[A-Z][A-Z0-9]{4,5}", identifier) is None:
        raise SystemExit(f"invalid FinTS segment identifier: {identifier}")

raw_trace = open(trace_path, "rb").read().decode("utf-8", "replace")
payloads = []
for match in re.finditer(r"\b(?:payload_)?hex=([0-9a-fA-F]+)\b", raw_trace):
    try:
        payloads.append(bytes.fromhex(match.group(1)).decode("latin-1"))
    except ValueError as error:
        raise SystemExit("trace contains malformed hexadecimal payload data") from error

print("# FinTS structural skeleton only — no field values")
occurrences = {}
for payload in payloads:
    for depth, segment in segment_stream(payload):
        elements = split_unescaped(segment, "+")
        header = split_unescaped(elements[0], ":")
        code = header[0]
        if re.fullmatch(r"[A-Z][A-Z0-9]{4,5}", code) is None:
            continue
        if wanted and code not in wanted:
            continue
        occurrences[code] = occurrences.get(code, 0) + 1
        print(
            f"{code} occurrence={occurrences[code]} nesting={depth} "
            f"header_components={len(header)} -> {occupancy(header)}"
        )
        for element_index, element in enumerate(elements[1:], start=1):
            components = split_unescaped(element, ":")
            print(
                f"    element {element_index}: {len(components)} components "
                f"-> {occupancy(components)}"
            )
        print()
PY
