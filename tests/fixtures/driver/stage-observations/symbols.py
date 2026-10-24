"""Check body attribution, exact instances and already loaded prebuilt metadata."""

import json
import sys
from pathlib import Path

directory = Path(sys.argv[1])
report = json.loads((directory / "symbols.json").read_text())
optimizations = json.loads((directory / "optimizations.json").read_text())
lir = (directory / "lir.txt").read_text()
assert report["schema"] == 1
entries = report["symbols"]
local = [entry for entry in entries if entry["producer"] == report["cone"]]
assert all(entry["symbol"] in lir for entry in local)
assert len({entry["symbol"] for entry in local}) == len(local)

answer = next(entry for entry in local if entry["name"] == "answer")
function = next(f for f in optimizations["functions"] if f["name"] == "answer")
assert answer["implementation"] == function["implementation"]
assert answer["template"] == function["materialization"]["template"]
assert answer["declaration"] == function["declaration"]
assert answer["signature"]["parameters"] == [p["exact"] for p in function["parameters"]]
assert answer["signature"]["result"] == function["result"]["exact"]

for name in ("identity", "read"):
    instances = [entry for entry in local if entry["name"] == name]
    assert len(instances) == 2, instances
    assert len({entry["body"] for entry in instances}) == 2
    assert len({tuple(entry["signature"]["parameters"]) for entry in instances}) == 2
    if name == "identity":
        assert len({entry["application"] for entry in instances}) == 2
        assert all(entry["template"]["kind"] == "generic-function" for entry in instances)

assert any(entry["role"] == "boxing-adjust" for entry in local)
gateway = next(entry for entry in local if entry["role"] == "root-gateway")
assert gateway["declaration"] is None and gateway["signature"] is None
prebuilt = [entry for entry in entries if entry["producer"] != report["cone"]]
assert any(entry["name"] and entry["signature"] and entry["declaration"] for entry in prebuilt)
