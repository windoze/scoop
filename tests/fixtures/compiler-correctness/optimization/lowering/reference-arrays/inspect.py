"""Reference bodies must keep their table values and bounded safepoint spacing."""

import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
source = path.read_text()
release = sys.argv[2] == "release"
lowered = re.split(r"(?=^  fun )", path.with_name("lir.txt").read_text(), flags=re.M)[1:]


def body(name):
    match = re.search(rf"^  fun {name} @fn(\d+).*?(?=^  fun |\Z)", source, re.M | re.S)
    assert match, name
    return match.group(), lowered[int(match[1])]


names = (
    "sumNodes",
    "findNode",
    "copyNodes",
    "copyInterfaces",
    "projectInterfaces",
    "copyStrings",
    "copyFunctions",
    "sumOptional",
    "copyOptional",
    "clearWrapped",
    "fillNodes",
    "fillInterfaces",
)
for name in names:
    function, lir = body(name)
    assert ("poll.full." in function) == release, name
    assert function.count("<loop-header-poll>") == 1, name
    if release:
        # One managed entry poll and the original loop-header poll.
        assert lir.count("    poll ") == 2, name
        assert "itable_lookup" not in lir and "itable_find" not in lir, name
        for block in re.finditer(
            r"^    bb\d+ poll.full.[^\n]+\n(.*?)(?=^    bb|\Z)", function, re.M | re.S
        ):
            assert not re.search(r"^      (call|throw) ", block[1], re.M), name
            assert "ClassAlloc" not in block[1] and "Box {" not in block[1], name

# A MaybeUninit initializer needs no element loop in either profile.
empty, _ = body("emptyStorage")
assert "ArrayAllocate MutableArray<MaybeUninit<Node>>" in empty
assert "<loop-header-poll>" not in empty and "poll.full." not in empty

# The GC carrier shares the kernels but supplies different callers.
if len(sys.argv) == 3:
    for name in (
        "projectFinal",
        "projectEmpty",
        "copyAny",
        "copyNullableViews",
        "copyChoice",
        "wrappedSum",
        "copyAliased",
        "copyUntil",
    ):
        function, lir = body(name)
        assert ("poll.full." in function) == release, name
        if release:
            assert "itable_lookup" not in lir and "itable_find" not in lir, name
    for name in ("projectOpen", "copyPairs", "invokeElements"):
        assert "poll.full." not in body(name)[0], name
    if release:
        assert "itable_lookup" in body("projectOpen")[1]
else:
    for name in (
        "writeYoungNodes",
        "writeYoungViews",
        "writeYoungOptional",
        "writeYoungWrapped",
        "findReference",
    ):
        function, lir = body(name)
        assert "poll.full." in function, name
        assert "itable_lookup" not in lir and "itable_find" not in lir, name
