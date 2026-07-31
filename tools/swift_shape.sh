#!/usr/bin/env bash
# Prints structural MT535, MT536, and MT940 facts only: block paths, tag
# ordinals, and character-class patterns. It never prints values, identifiers,
# amounts, names, credentials, or raw wire data.
#
# Usage:
#   tools/swift_shape.sh <trace-file>

set -euo pipefail

trace_file="${1:?usage: swift_shape.sh <trace-file>}"

python3 - "$trace_file" <<'PY'
import re
import sys


def shape(text):
    classes = []
    for character in text:
        if character.isdigit():
            classes.append("9")
        elif character.isalpha():
            classes.append("A" if character.isupper() else "a")
        else:
            classes.append(character)

    collapsed = []
    index = 0
    while index < len(classes):
        end = index + 1
        while end < len(classes) and classes[end] == classes[index]:
            end += 1
        run = end - index
        collapsed.append(
            classes[index] * run if run < 4 else f"{classes[index]}{{{run}}}"
        )
        index = end
    return "".join(collapsed)


def block_kind(value):
    candidate = value.strip()
    if re.fullmatch(r"[A-Z][A-Z0-9]{0,15}", candidate):
        return candidate
    return "UNKNOWN_BLOCK"


trace_path = sys.argv[1]
raw_trace = open(trace_path, "rb").read().decode("utf-8", "replace")
payloads = []
for match in re.finditer(r"\bhex=([0-9a-fA-F]+)\b", raw_trace):
    try:
        payloads.append(bytes.fromhex(match.group(1)).decode("latin-1"))
    except ValueError as error:
        raise SystemExit("trace contains malformed hexadecimal payload data") from error

documents = []
for payload in payloads:
    for marker in re.finditer(r"@(\d+)@", payload):
        start = marker.end()
        length = int(marker.group(1))
        candidate = payload[start:start + length]
        if ":16R:" in candidate:
            documents.append(("MT535/MT536", candidate))
        elif ":20:" in candidate and (
            ":28C:" in candidate or ":60F:" in candidate or ":60M:" in candidate
        ):
            documents.append(("MT940", candidate))

if not documents:
    print("no MT535, MT536, or MT940 payload found in this trace")
    raise SystemExit(0)

for document_index, (document_kind, document) in enumerate(documents, start=1):
    print(
        f"# ---- {document_kind} payload {document_index}: "
        f"{len(document)} bytes, structure only ----"
    )
    block_path = []
    ordinals = {}
    normalized = document.replace("\r\n", "\n").replace("\r", "\n")
    for line in normalized.split("\n"):
        if line == "":
            continue
        match = re.match(r"^:([0-9]{2}[A-Z]?):(.*)$", line)
        if match is None:
            path = "/".join(block_path) or document_kind
            print(f"    [{path}] (continuation) {shape(line)}")
            continue

        tag, value = match.group(1), match.group(2)
        if tag == "16R":
            block_path.append(block_kind(value))
        ordinals[tag] = ordinals.get(tag, 0) + 1
        path = "/".join(block_path) if block_path else document_kind
        print(f"    [{path}] :{tag}: #{ordinals[tag]:<3} {shape(value)}")
        if tag == "16S" and block_path:
            block_path.pop()
    print()
PY
