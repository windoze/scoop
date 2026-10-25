"""Check a real inline decision produced by an observation-only build."""

import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text())
assert report["schema"] == 2 and report["rounds"] > 0
assert report["passes"] == {"inline": True, "allocation-elision": True}
functions = {f["id"]: f for f in report["functions"]}
events = [
    e
    for e in report["events"]
    if e["pass"] == "inline"
    and e["target"]["kind"] == "direct"
    and e["target"]["callee"]["kind"] == "function"
    and functions[e["target"]["callee"]["id"]]["name"] == "answer"
    and functions[e["site"]["function"]]["name"] == "main"
]
assert len(events) == 1, events
event = events[0]
assert event["outcome"] == "inlined"
cost = event["estimate"]["ordinary"]
assert cost["penalty"] <= cost["weighted_benefit"]
assert cost["growth"] <= event["estimate"]["remaining_growth"]
callee = functions[event["target"]["callee"]["id"]]
assert callee["declaration"]["source"]["path"]
assert callee["materialization"]["template"]["kind"] == "function"
assert callee["parameters"][0]["exact"]
instances = [
    f
    for f in functions.values()
    if f["name"] == "next"
    and f["parameters"]
    and f["parameters"][0]["name"] == "ListIterator<Long>"
]
assert instances, "generic dependency body was not observed"
for function in instances:
    assert function["materialization"]["context"]["kind"] == "application"
    assert function["declaration"]["source"]["cone"] != report["cone"]
assert any(c["name"] and c["declaration"] for c in report["external_callables"])
