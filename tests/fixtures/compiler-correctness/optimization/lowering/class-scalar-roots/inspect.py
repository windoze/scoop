"""Verify field replacement and the actual normal/exceptional root plans."""

import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
functions = {
    function[1]: (int(function[2]), function.group())
    for function in re.finditer(
        r"^  fun (\S+) @fn(\d+).*?(?=^  fun |\Z)", path.read_text(), re.M | re.S
    )
}
lowered = re.split(r"(?=^  fun )", path.with_name("lir.txt").read_text(), flags=re.M)[1:]


def body(name):
    return lowered[functions[name][0]]


for name, allocated, local in (
    ("construction", "Constructed", "value"),
    ("exceptionReferences", "Holder", "holder"),
    ("exceptionReturn", "Holder", "holder"),
    ("exceptionBox", None, "boxed"),
):
    function = functions[name][1]
    assert "landing_pad" in function and "rethrow" in function, name
    if allocated:
        assert f"ClassAlloc {allocated}" not in function, name
    else:
        assert not re.search(r"\b(?:Box|Unbox)\b", function), name
    # Declarations may remain, but removed object references are never roots.
    declared = re.search(rf"^    local %(\d+) {local}: ptr<managed>$", body(name), re.M)
    if declared:
        roots = "\n".join(
            line for line in body(name).splitlines() if "live=[" in line or "roots=[" in line
        )
        assert not re.search(rf"\blocal{declared[1]}:", roots), name

assert "param0:refs[0, 8, 16, 24]:unwind" in body("exceptionBox")
references = body("exceptionReferences")
field = re.search(r"^    local %(\d+) \$allocation\.\d+\.field\.1: struct\d+$", references, re.M)[1]
exceptional = [
    line for line in references.splitlines() if f"local{field}:refs[0, 8, 16, 24]:unwind" in line
]
assert len(exceptional) == 2
assert all(line.count(":refs[0, 8, 16, 24]:unwind") >= 2 for line in exceptional)

returned = body("exceptionReturn")
snapshot = re.search(r"^    local %(\d+) snapshot: ptr<managed>$", returned, re.M)[1]
collections = [
    line
    for line in returned.splitlines()
    if "runtime @scoop_rt_gc_collect()" in line and f"local{snapshot}:" in line
]
assert len(collections) == 1
assert collections[0].count(":ptr<managed>@0") == 2
