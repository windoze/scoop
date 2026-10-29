"""Check actual caller specialization and its observed retained-body cost."""

import json
import re
import sys
from pathlib import Path

dump = Path(sys.argv[1])
release = sys.argv[2] == "release"
source = (dump / "mir.txt").read_text()
report = json.loads((dump / "optimizations.json").read_text())
functions = {function["id"]: function for function in report["functions"]}
exercise = next(id for id, function in functions.items() if function["name"] == "exercise")
getters = {
    id
    for id, function in functions.items()
    if function["name"] == "get" and function["parameters"][0]["name"] == "Array<Long>"
}
body = re.search(r"^  fun exercise @fn\d+.*?(?=^  fun |\Z)", source, re.M | re.S).group()
if release:
    assert getters
    assert not any(re.search(rf"^      call .*@fn{id} direct$", body, re.M) for id in getters)
    assert body.count(" poll.full.") >= 4
    events = [
        event
        for event in report["events"]
        if event["pass"] == "inline"
        and event["site"]["function"] == exercise
        and event["outcome"] == "inlined"
        and event["target"].get("callee", {}).get("kind") == "function"
        and event["target"]["callee"]["id"] in getters
    ]
    assert any(
        event["estimate"]["ordinary"]["size"] < event["estimate"]["original"]["size"]
        and event["estimate"]["ordinary"]["growth"] <= event["estimate"]["remaining_growth"]
        for event in events
    )
else:
    assert "poll.full." not in body
    assert not report["events"]
