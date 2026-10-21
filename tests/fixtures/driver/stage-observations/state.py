"""Check that stage observation leaves compiled and linked outputs untouched."""

import json
import sys
from pathlib import Path

mode, root, output, saved = sys.argv[1:]
root = Path(root)
paths = [Path(output)]
for pattern in (
    "compile-v2/*/artifact.slib",
    "compile-v2/*/record.cbor",
    "objects-v1/*.o",
    "executables-v1/*.json",
    "executables-v1/program-*/program",
):
    paths.extend(root.glob(pattern))
state = {str(p): [p.stat().st_size, p.stat().st_mtime_ns] for p in sorted(paths)}
if mode == "save":
    Path(saved).write_text(json.dumps(state))
else:
    assert mode == "same"
    previous = json.loads(Path(saved).read_text())
    assert state == previous, (previous, state)
