"""Input metadata and output reuse shared by native maintenance builds."""

import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RELEASE_CONFIG = ROOT / "scripts/release.toml"


def metadata(path):
    path = Path(path)
    info = path.stat()
    return [str(path), info.st_mtime_ns, info.st_size]


def source_metadata(source, directories):
    files = []
    for directory in directories:
        root = source / directory
        if not root.is_dir():
            raise ValueError(f"source directory is missing: {root}")
        for parent, children, names in os.walk(root):
            children[:] = sorted(name for name in children if name != ".git")
            files.extend(metadata(Path(parent) / name) for name in sorted(names))
    return files


def reusable(record, inputs, outputs):
    if not record.exists():
        return False
    saved = json.loads(record.read_text())
    return (
        saved["inputs"] == inputs
        and all(path.exists() for path in outputs)
        and all(metadata(Path(value[0])) == value for value in saved["dependencies"])
    )


def save_record(path, inputs, objects):
    dependencies = set()
    for depfile in objects.rglob("*.o.d"):
        text = depfile.read_text().replace("\\\n", " ")
        _, separator, prerequisites = text.partition(": ")
        if not separator:
            raise ValueError(f"invalid compiler depfile: {depfile}")
        for token in re.findall(r"(?:\\.|[^\s])+", prerequisites):
            value = Path(re.sub(r"\\(.)", r"\1", token).replace("$$", "$"))
            value = (value if value.is_absolute() else objects / value).resolve()
            if not value.is_relative_to(objects.parent.resolve()):
                dependencies.add(value)
    state = {"inputs": inputs, "dependencies": [metadata(path) for path in sorted(dependencies)]}
    temporary = path.with_suffix(".new")
    temporary.write_text(json.dumps(state, separators=(",", ":")) + "\n")
    temporary.replace(path)
