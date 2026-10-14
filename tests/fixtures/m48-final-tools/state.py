"""Observe tool discovery records across real program relinks."""

import json
import sys
from pathlib import Path

mode, root, saved = sys.argv[1:]
paths = sorted(Path(root).glob("*-tools-v1/*.json"))
assert any(p.parent.name == "final-tools-v1" for p in paths), paths
state = {str(p): [p.stat().st_size, p.stat().st_mtime_ns] for p in paths}
if mode == "save":
    Path(saved).write_text(json.dumps(state))
elif mode == "same":
    previous = json.loads(Path(saved).read_text())
    assert state == previous, (previous, state)
else:
    raise ValueError(mode)
