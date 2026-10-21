"""Observe stable object outputs without reading their contents."""

import json
import sys
from pathlib import Path

mode, root, saved = sys.argv[1:]
paths = sorted((Path(root) / "objects-v1").glob("*.o"))
assert paths, "no producer object files"
state = {str(p): [p.stat().st_size, p.stat().st_mtime_ns] for p in paths}
if mode == "save":
    Path(saved).write_text(json.dumps(state))
elif mode in ("same", "subset"):
    previous = json.loads(Path(saved).read_text())
    if mode == "same":
        assert state == previous, (previous, state)
    else:
        assert all(state.get(p) == value for p, value in previous.items())
elif mode == "remove_one":
    paths[0].unlink()
    Path(saved).write_text(json.dumps({p: v for p, v in state.items() if p != str(paths[0])}))
    Path(saved + ".missing").write_text(str(paths[0]))
elif mode == "restored":
    previous = json.loads(Path(saved).read_text())
    missing = Path(saved + ".missing").read_text()
    assert set(state) == set(previous) | {missing}
    assert all(state[p] == value for p, value in previous.items())
else:
    raise ValueError(mode)
