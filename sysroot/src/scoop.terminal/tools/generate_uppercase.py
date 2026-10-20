"""Generate the terminal parser's Unicode Uppercase table from Unicode 17.0."""

import argparse
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("derived_core_properties", type=Path)
arguments = parser.parse_args()
source = arguments.derived_core_properties.read_text(encoding="utf-8")
if not source.startswith("# DerivedCoreProperties-17.0.0.txt"):
    parser.error("expected Unicode 17.0.0 DerivedCoreProperties.txt")

ranges = []
for line in source.splitlines():
    fields = line.split("#", 1)[0].split(";")
    if len(fields) != 2 or fields[1].strip() != "Uppercase":
        continue
    bounds = fields[0].strip().split("..")
    start, end = int(bounds[0], 16), int(bounds[-1], 16)
    if ranges and ranges[-1][1] + 1 == start:
        ranges[-1] = (ranges[-1][0], end)
    else:
        ranges.append((start, end))

lines = [
    "package scoop.terminal.internal.input",
    "",
    "// Generated from Unicode 17.0.0 DerivedCoreProperties.txt, Uppercase.",
    "// Source: https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt",
    "// License: LICENSE-Unicode at the Cone root; regenerate with tools/generate_uppercase.py.",
    "internal val uppercaseRanges: Array<(Int, Int)> = [",
]
entries = [f"({start}, {end})" for start, end in ranges]
for index in range(0, len(entries), 6):
    suffix = "," if index + 6 < len(entries) else ""
    lines.append("    " + ", ".join(entries[index : index + 6]) + suffix)
lines.append("]")
destination = Path(__file__).resolve().parent.parent / "src/internal/input/uppercase_ranges.scoop"
destination.write_text("\n".join(lines) + "\n", encoding="utf-8")
print(f"Wrote {len(ranges)} Uppercase ranges to {destination}")
