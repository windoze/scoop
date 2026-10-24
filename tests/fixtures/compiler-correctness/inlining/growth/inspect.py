import json
import re
import sys
from pathlib import Path

profile, folder = sys.argv[1], Path(sys.argv[2])
mir = (folder / "mir.txt").read_text()
body = re.search(r"(?ms)^  fun traverse .*?(?=^  fun |\Z)", mir).group()
if profile == "debug":
    assert "call " in body
else:
    report = json.loads((folder / "optimizations.json").read_text())
    functions = report["functions"]
    caller = next(f["id"] for f in functions if f["name"] == "traverse")
    getter = next(
        f["id"]
        for f in functions
        if f["name"] == "get" and f["parameters"][0]["name"] == "ArrayList<Long>"
    )
    assert any(
        event["pass"] == "inline"
        and event["site"]["function"] == caller
        and event["target"] == {"kind": "direct", "callee": {"kind": "function", "id": getter}}
        and event["outcome"] == "inlined"
        for event in report["events"]
    )
    assert not re.search(rf"call .*@fn{getter}\b", body)
    assert "MaybeUninit.AssumeInit" in body
