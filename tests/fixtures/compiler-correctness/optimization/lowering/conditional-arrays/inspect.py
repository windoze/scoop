"""Check bounded conditional regions and their retained checked paths."""

import re
import sys
from pathlib import Path

source = Path(sys.argv[1]).read_text()
release = sys.argv[2] == "release"
expected = (
    "generatedConditional",
    "sumSelected",
    "transform",
    "retainPositive",
    "partition",
    "clampFloat",
    "clampDouble",
    "sumMasked",
    "straightSum",
)
for name in (*expected, "firstNegative", "readSnapshot", "earlyUpdate"):
    match = re.search(rf"^  fun {name} @.*?(?=^  fun |\Z)", source, re.M | re.S)
    if name in ("readSnapshot", "earlyUpdate") and not match:
        continue
    assert match, name
    body = match.group()
    optimized = release
    assert ("poll.full." in body) == optimized, name
    assert body.count("<loop-header-poll>") == 1, name
    if not optimized:
        continue
    blocks = {
        int(m[1]): (m[2], m[3])
        for m in re.finditer(r"^    bb(\d+) ([^\n]+)\n(.*?)(?=^    bb|\Z)", body, re.M | re.S)
    }
    full = {i for i, (label, _) in blocks.items() if label.startswith("poll.full.")}
    header = next(i for i, (label, _) in blocks.items() if "<loop-header-poll>" in label)
    exits = []
    for i in full:
        text = blocks[i][1]
        assert not re.search(r"^      (call|throw) ", text, re.M), name
        assert "ClassAlloc" not in text, name
        target = re.search(r"^      (?:goto|branch) (bb\d+(?: bb\d+)?)$", text, re.M)
        assert target, name
        for destination in map(int, re.findall(r"bb(\d+)", target[1])):
            if destination not in full:
                exits.append(destination)
    if name in ("firstNegative", "readSnapshot"):
        assert header in exits and len(set(exits)) >= 2, (name, exits)
    else:
        assert exits == [header], (name, exits)
