"""Check actual floating kernels while preserving their original poll sites."""

import re
import sys
from pathlib import Path

source = Path(sys.argv[1]).read_text()
release = sys.argv[2] == "release"
for name in (
    "generatedFloat",
    "generatedDouble",
    "transformFloat",
    "transformDouble",
    "divideFloat",
    "divideDouble",
    "sumFloat",
    "sumDouble",
    "narrow",
    "copyFloat",
    "copyDouble",
):
    match = re.search(rf"^  fun {name} @.*?(?=^  fun |\Z)", source, re.M | re.S)
    assert match, name
    body = match.group()
    assert ("poll.full." in body) == release, name
    assert body.count("<loop-header-poll>") == 1, name
    if release:
        full = re.search(r"^    bb\d+ poll.full.*?(?=^    bb|\Z)", body, re.M | re.S)
        assert full, name
        assert not re.search(r"^      (call|throw) ", full.group(), re.M), name
