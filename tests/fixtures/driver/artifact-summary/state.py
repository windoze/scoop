"""Observe summary records while their actual artifact inputs stay available."""

import json
import sys
from pathlib import Path

mode, cache, saved, count = sys.argv[1:]
paths = sorted(Path(cache).glob("records/artifacts-v1/*"))
assert len(paths) == int(count), paths
state = {str(path): [path.stat().st_size, path.stat().st_mtime_ns] for path in paths}
if mode == "save":
    Path(saved).write_text(json.dumps(state))
elif mode == "same":
    previous = json.loads(Path(saved).read_text())
    assert state == previous, (previous, state)
else:
    raise ValueError(mode)
