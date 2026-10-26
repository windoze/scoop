"""Assert actual call-site choices against MIR and emitted LIR calls."""

import json
import re
import sys
from pathlib import Path

folder, profile = Path(sys.argv[1]), sys.argv[2]
report = json.loads((folder / "optimizations.json").read_text())
mir = (folder / "mir.txt").read_text()
functions = {f["name"]: f["id"] for f in report["functions"]}
external = {f["name"]: f["id"] for f in report["external_callables"]}
lir = re.split(r"(?=^  fun )", (folder / "lir.txt").read_text(), flags=re.M)[1:]


def body(name):
    return re.search(rf"(?ms)^  fun {name} @fn.*?(?=^  fun |\Z)", mir)[0]


def has_call(name, callee):
    found = re.search(rf"call .*external{external[callee]}\b", body(name)) is not None
    assert (
        re.search(rf"call .*external-fn{external[callee]}\b", lir[functions[name]]) is not None
    ) == found
    return found


if profile == "debug":
    for name, callee in [
        ("ordinarySite", "mix"),
        ("hotSite", "mix"),
        ("constantSite", "mix"),
        ("smallSite", "tiny"),
        ("coldSite", "tiny"),
    ]:
        assert has_call(name, callee), name
else:
    assert report["schema"] == 2
    for name, callee, retained in [
        ("ordinarySite", "mix", True),
        ("hotSite", "mix", False),
        ("constantSite", "mix", False),
        ("smallSite", "tiny", False),
        ("coldSite", "tiny", True),
    ]:
        assert has_call(name, callee) == retained, (name, body(name))
        events = [
            e
            for e in report["events"]
            if e["pass"] == "inline"
            and e["site"]["function"] == functions[name]
            and e["target"]
            == {"kind": "direct", "callee": {"kind": "external", "id": external[callee]}}
        ]
        assert events, name
        event = events[-1]
        expected = (
            "caller-growth"
            if name == "ordinarySite"
            else "not-profitable"
            if retained
            else "inlined"
        )
        assert event["outcome"] == expected, event
        estimate = event["estimate"]
        cost = estimate["ordinary"]
        assert (cost["penalty"] > cost["weighted_benefit"]) == retained, event
        if name == "hotSite":
            assert estimate["heat"]["loop_depth"] == 2 and cost["size"] > 72, event
        if name == "constantSite":
            assert estimate["baseline"]["size"] > 72 and cost["size"] < 12, event
            assert cost["benefit"]["simplification"] < 12, event
        if name == "coldSite":
            assert estimate["heat"]["source"] == "cold-exit" and cost["size"] <= 16, event
