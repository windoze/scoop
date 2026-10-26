"""Keep bounded iterations and their original shared side exits distinct."""

import re
import sys
from pathlib import Path

source = Path(sys.argv[1]).read_text()
release = sys.argv[2] == "release"
names = (
    "findIndex",
    "findValue",
    "firstMismatch",
    "copyUntil",
    "prefixDouble",
    "findObject",
    "findWithCollection",
)
for name in names:
    match = re.search(rf"^  fun {name} @.*?(?=^  fun |\Z)", source, re.M | re.S)
    assert match, name
    body = match.group()
    assert ("poll.full." in body) == release, name
    assert body.count("<loop-header-poll>") == 1, name
    if not release:
        continue
    blocks = {
        int(m[1]): (m[2], m[3])
        for m in re.finditer(r"^    bb(\d+) ([^\n]+)\n(.*?)(?=^    bb|\Z)", body, re.M | re.S)
    }
    full = {i for i, (label, _) in blocks.items() if label.startswith("poll.full.")}
    header = next(i for i, (label, _) in blocks.items() if "<loop-header-poll>" in label)
    exits = set()
    for block in full:
        text = blocks[block][1]
        assert not re.search(r"^      (call|throw|return) ", text, re.M), name
        assert "ClassAlloc" not in text, name
        target = re.search(r"^      (?:goto|branch) (bb\d+(?: bb\d+)?)$", text, re.M)
        assert target, name
        exits.update(int(i) for i in re.findall(r"bb(\d+)", target[1]) if int(i) not in full)
    assert header in exits and len(exits) >= 2, (name, exits)
    assert all(not blocks[i][0].startswith("poll.") for i in exits - {header}), name
