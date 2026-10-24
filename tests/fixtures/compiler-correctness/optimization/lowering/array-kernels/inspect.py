"""Check the actual optimized producer and its conservative boundaries."""

import re
import sys
from pathlib import Path

source = Path(sys.argv[1]).read_text()
release = sys.argv[2] == "release"


def body(name):
    match = re.search(rf"^  fun {name} @.*?(?=^  fun |\Z)", source, re.M | re.S)
    assert match, name
    return match.group()


for name in ["generatedLong", "generatedInt", "generatedBoolean"]:
    function = body(name)
    assert ("poll.full." in function) == release, name
    assert function.count("<loop-header-poll>") == 1, name
    assert function.count("      array_set ") == (2 if release else 1), name

function = body("writePrefix")
assert ("poll.full." in function) == release
assert ("poll.bounds." in function) == release

for name in ["shortArray", "exceptional", "referenceArray"]:
    assert "poll.full." not in body(name), name
