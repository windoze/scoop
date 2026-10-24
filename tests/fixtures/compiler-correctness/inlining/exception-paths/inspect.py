"""Inspect actual callers, retaining unknown effects and enclosing handlers."""

import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
source = path.read_text()
release = sys.argv[2] == "release"
functions = list(re.finditer(r"^  fun (\S+) @fn(\d+).*?(?=^  fun |\Z)", source, re.M | re.S))


def body(name):
    return next(function.group() for function in functions if function[1] == name)


def calls_use(function):
    targets = re.findall(r"^      call (?:[^\n]* = )?@fn(\d+) direct", function, re.M)
    return any(f[2] in targets and f[1].startswith("$extension.use") for f in functions)


for name in ("usingLease", "directLease"):
    function = body(name)
    if release:
        for token in ("ClassAlloc", "ClosureAlloc", "ClosureCapture", "      call ", "landing_pad"):
            assert token not in function, (name, token)
    elif name == "usingLease":
        assert "ClosureAlloc" in function and calls_use(function), name

if release:
    imported = body("importedGuarded")
    assert "landing_pad" not in imported and "      call " not in imported
    assert not calls_use(body("cleanupFailure"))
    retained = body("retainedReference")
    assert "ClosureAlloc" not in retained and not calls_use(retained)
    factory = next(function for function in functions if function[1] == "escapingPayload")
    assert "ClassAlloc Payload" in factory.group()
    assert f"= @fn{factory[2]} direct" in retained and "global_assign retained" in retained
    assert "landing_pad" in body("cleanupFailure")
    assert "ClosureAlloc" in body("bodyFailure") and calls_use(body("bodyFailure"))
    assert calls_use(body("unresolved"))
    assert re.search(r"^      call .*external\d+ direct", body("opaqueGuarded"), re.M)
    # The returned object is the only live root at both collections; eliminated
    # closure locals must not turn into extra roots merely because they remain declared.
    retained_id = next(
        int(function[2]) for function in functions if function[1] == "retainedReference"
    )
    lowered = re.split(r"(?=^  fun )", path.with_name("lir.txt").read_text(), flags=re.M)[1:][
        retained_id
    ]
    payload = re.search(r"^    local %(\d+) payload: ptr<managed>$", lowered, re.M)[1]
    collections = [
        line for line in lowered.splitlines() if "runtime @scoop_rt_gc_collect()" in line
    ]
    assert len(collections) == 2
    assert all(f"live=[local{payload}:ptr<managed>@0]" in line for line in collections)
