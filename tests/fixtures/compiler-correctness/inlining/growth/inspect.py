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

    transformed = re.search(r"(?ms)^  fun transformed .*?(?=^  fun |\Z)", mir).group()
    assert not re.search(r"call .*interface Iterator<Long>\[", transformed)
    caller = next(f["id"] for f in functions if f["name"] == "transformed")
    events = [
        event
        for event in report["events"]
        if event["pass"] == "inline"
        and event["site"]["function"] == caller
        and event["outcome"] == "inlined"
    ]
    assert len({event["estimate"]["heat"]["loop_header"] for event in events}) >= 3
    for event in events:
        estimate = event["estimate"]
        assert estimate["remaining_growth"] == min(
            estimate["region_remaining_growth"], estimate["caller_remaining_growth"]
        )
    for function in functions:
        if function["name"] not in ("get", "checkIndex", "iterator", "next"):
            continue
        assert not re.search(rf"call .*@fn{function['id']}\b", transformed)
    assert not re.search(rf"call .*@fn{getter}\b", body)
    assert "MaybeUninit.AssumeInit" in body

    dispatch = re.search(r"(?ms)^  fun dispatch .*?(?=^  fun |\Z)", mir).group()
    assert "interface Score[" in dispatch
    helpers = [
        function
        for function in functions
        if function["name"] in ("next", "get")
        and function["parameters"]
        and function["parameters"][0].get("name") in ("ListIterator<Score>", "ArrayList<Score>")
    ]
    assert {(function["parameters"][0]["name"], function["name"]) for function in helpers} == {
        ("ListIterator<Score>", "next"),
        ("ArrayList<Score>", "get"),
    }
    for function in helpers:
        assert not re.search(rf"call .*@fn{function['id']}\b", dispatch)

    rotate = next(f["id"] for f in functions if f["name"] == "Vector.rotate")
    caller = next(f["id"] for f in functions if f["name"] == "rotations")
    rotations = re.search(r"(?ms)^  fun rotations .*?(?=^  fun |\Z)", mir).group()
    assert not re.search(rf"call .*@fn{rotate}\b", rotations)
    assert any(
        event["pass"] == "inline"
        and event["site"]["function"] == caller
        and event["target"] == {"kind": "direct", "callee": {"kind": "function", "id": rotate}}
        and event["outcome"] == "inlined"
        and event["estimate"]["in_loop"]
        and event["estimate"]["ordinary"]["execution"] > 40
        for event in report["events"]
    )
