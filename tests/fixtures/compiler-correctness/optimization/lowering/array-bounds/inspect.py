"""Check the selected kernels and the retained checked fallback."""

import re
import sys
from pathlib import Path

source = Path(sys.argv[1]).read_text()
release = sys.argv[2] == "release"
roots = len(sys.argv) > 3 and sys.argv[3] == "roots"


def body(name):
    match = re.search(rf"^  fun {name} @.*?(?=^  fun |\Z)", source, re.M | re.S)
    assert match, name
    return match.group()


for name in ["transform"] if roots else ["sum", "transform"]:
    function = body(name)
    assert function.count("<loop-header-poll>") == 1, name
    assert ("poll.full." in function) == release, name
    if release:
        assert "      throw" not in function, name
        assert "poll.bounds." not in function, name

for name, guards in [("copy", 2)] if roots else [("prefix", 1), ("copy", 2)]:
    function = body(name)
    assert function.count("<loop-header-poll>") == 1, name
    assert ("poll.full." in function) == release, name
    assert function.count(" poll.bounds.") == (guards if release else 0), name
    if release:
        assert function.count("      throw") == guards, name
        full = re.search(r"^    bb\d+ poll.full.*?(?=^    bb|\Z)", function, re.M | re.S)
        assert full and "      call " not in full.group(), name
        assert "      throw" not in full.group(), name

if not roots:
    repeated = body("repeatSum")
    # Both the initializer and the inlined reduction must have a bounded inner.
    assert repeated.count(" poll.full.") == (2 if release else 0)
    for name in ["sideEffect", "rebound", "stepped", "negative"]:
        assert "poll.full." not in body(name), name
