import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text())
mode = sys.argv[2]
external = {item["name"]: item["id"] for item in report["external_callables"]}


def decisions(name):
    target = {"kind": "direct", "callee": {"kind": "external", "id": external[name]}}
    events = [item for item in report["events"] if item.get("target") == target]
    assert events, name
    return events


for name in ["Request.<init>", "delayed", "stored", "imageValue"]:
    expected = "unsupported-body" if mode == "full" else "no-mir-body"
    assert all(item["outcome"] == expected for item in decisions(name)), name

if mode == "full":
    for name in ["make", "handled", "current", "changeStored", "changeThreadValue", "writeFirst"]:
        assert any(item["estimate"] is not None for item in decisions(name)), name
elif mode == "off":
    assert all(item["outcome"] == "no-mir-body" for item in decisions("ordinary"))
