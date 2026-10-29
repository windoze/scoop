"""Observe executable and debug-file publication using file metadata only."""

import json
import sys
from pathlib import Path

mode, root, output, argument = sys.argv[1:]
cache = Path(root) / "executables-v4"
programs = sorted(cache.glob("program-*/program"))
output = Path(output)
debug_file = Path(str(output) + (".dSYM" if sys.platform == "darwin" else ".debug"))
if mode == "count":
    assert len(programs) == int(argument), programs
elif mode in ("debug", "none"):
    assert output.is_file(), output
    assert debug_file.exists() == (mode == "debug"), debug_file
    assert not Path(str(output) + ".map").exists()
else:
    paths = sorted(cache.glob("*.json")) + programs + [output]
    for parent in [*(p.parent for p in programs), output.parent]:
        for path in parent.glob("program.*"):
            paths.extend([path, *path.rglob("*")] if path.is_dir() else [path])
    state = {str(p): [p.stat().st_size, p.stat().st_mtime_ns] for p in paths}
    if mode == "save":
        Path(argument).write_text(json.dumps(state))
    elif mode == "same":
        previous = json.loads(Path(argument).read_text())
        assert state == previous, (previous, state)
    else:
        raise ValueError(mode)
