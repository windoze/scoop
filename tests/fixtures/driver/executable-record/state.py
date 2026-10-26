"""Observe executable publication without reading generated file contents."""

import json
import sys
from pathlib import Path

mode, root, output, argument = sys.argv[1:]
cache = Path(root) / "executables-v2"
programs = sorted(cache.glob("program-*/program"))
if mode == "count":
    assert len(programs) == int(argument), programs
else:
    paths = (
        sorted(cache.glob("*.json"))
        + programs
        + sorted(cache.glob("program-*/program.map"))
        + [Path(output), Path(str(output) + ".map")]
    )
    state = {str(p): [p.stat().st_size, p.stat().st_mtime_ns] for p in paths}
    if mode == "save":
        Path(argument).write_text(json.dumps(state))
    elif mode == "same":
        previous = json.loads(Path(argument).read_text())
        assert state == previous, (previous, state)
    else:
        raise ValueError(mode)
