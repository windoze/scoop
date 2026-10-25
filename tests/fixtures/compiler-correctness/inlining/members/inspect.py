"""Check cross-Cone member calls without assuming declaration-specific policy."""

import json
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
source = path.read_text()
release = sys.argv[2] == "release"
functions = {
    match[1]: match.group()
    for match in re.finditer(r"^  fun (\S+) @fn\d+.*?(?=^  fun |\Z)", source, re.M | re.S)
}

for name in (
    "methodKernel",
    "valueAccessorKernel",
    "referenceKernel",
    "accessorKernel",
    "directDefaultKernel",
    "packetKernel",
    "emptyKernel",
    "constructSnapshotKernel",
    "replaceKernel",
    "literalKernel",
):
    function = functions[name]
    if release:
        assert "      call " not in function, name
    else:
        assert "      call " in function, name

for name in ("virtualKernel", "interfaceKernel", "defaultKernel"):
    function = functions[name]
    assert "      call " in function and " direct" not in function, name

report = json.loads(path.with_name("optimizations.json").read_text())
external = {item["name"]: item["id"] for item in report["external_callables"]}
assert re.search(r"^      call .*external\d+ direct", functions["secondaryKernel"], re.M)
factory = functions["classConstructKernel"]
assert f"external{external['makeCell']} direct" in factory
if release:
    number = int(re.search(r" @fn(\d+)", factory)[1])
    decisions = [item for item in report["events"] if item["site"]["function"] == number]
    assert decisions and all(item["outcome"] == "execution-cost" for item in decisions)
validate = functions["validateKernel"]
assert (f"external{external['Recorder.validate']} direct" in validate) != release
assert "      call " in validate
assert "  imported_immortal " in path.with_name("lir.txt").read_text()
callback = functions["callbackSnapshotKernel"]
for name in ("Recorder.snapshot", "Recorder.validate"):
    assert (f"external{external[name]} direct" in callback) != release, name
assert "      call " in callback

primitive_calls = re.findall(
    r"^      call .*external(\d+) direct", functions["primitiveKernel"], re.M
)
expected = "coreLongHash" if release else "Long.hash"
assert primitive_calls == [str(external[expected])], (expected, primitive_calls)

if release:
    assert "SafeIntegerDivRem" in functions["guardedKernel"]
    assert "landing_pad" in functions["guardedKernel"]
    assert "      call " not in functions["genericKernel"]
    collected = functions["collectedKernel"]
    assert f"external{external['Cell.afterCollection']} direct" not in collected
    number = int(re.search(r" @fn(\d+)", collected)[1])
    lowered = re.split(r"(?=^  fun )", path.with_name("lir.txt").read_text(), flags=re.M)[1:][
        number
    ]
    roots = [line for line in lowered.splitlines() if "live=[" in line]
    assert roots and any("sp<managed-call:" in line for line in roots)
    assert all("live=[param1:ptr<managed>@0]" in line for line in roots)
else:
    for name in ("primitiveKernel", "guardedKernel"):
        assert re.search(r"^      call .*external\d+ direct", functions[name], re.M), name
