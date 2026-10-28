"""Input metadata and output reuse shared by native maintenance builds."""

import errno
import json
import os
import re
import subprocess
from contextlib import contextmanager
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RELEASE_CONFIG = ROOT / "scripts/release.toml"


@contextmanager
def build_lock(path):
    with path.open("a+b") as lock:
        if os.name == "nt":
            import msvcrt

            lock.seek(0)
            while True:
                try:
                    msvcrt.locking(lock.fileno(), msvcrt.LK_LOCK, 1)
                    break
                except OSError as error:
                    # LK_LOCK retries for ten seconds; a real build can take longer.
                    if error.errno not in (errno.EACCES, errno.EDEADLK):
                        raise
            try:
                yield
            finally:
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
        else:
            import fcntl

            fcntl.flock(lock, fcntl.LOCK_EX)
            yield


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


def save_record(path, inputs, objects, tools=()):
    dependencies = {Path(tool) for tool in tools}
    for name in dependency_paths(objects):
        value = Path(name)
        value = (value if value.is_absolute() else objects / value).resolve()
        if not value.is_relative_to(objects.parent.resolve()):
            dependencies.add(value)
    state = {"inputs": inputs, "dependencies": [metadata(path) for path in sorted(dependencies)]}
    temporary = path.with_suffix(".new")
    temporary.write_text(json.dumps(state, separators=(",", ":")) + "\n")
    temporary.replace(path)


def dependency_paths(objects):
    names = set()
    if (objects / "build.ninja").is_file():
        # Ninja consumes compiler depfiles into its dependency log. Read the
        # actual header inputs only after a build, never during a reuse check.
        command = ["ninja", "-C", str(objects), "-t", "deps"]
        with subprocess.Popen(
            command, stdout=subprocess.PIPE, text=True, encoding="utf-8"
        ) as process:
            for line in process.stdout:
                if line.startswith("    "):
                    names.add(line[4:].rstrip("\r\n"))
            if process.wait():
                raise subprocess.CalledProcessError(process.returncode, command)
    else:
        for depfile in objects.rglob("*.o.d"):
            text = depfile.read_text().replace("\\\n", " ")
            _, separator, prerequisites = text.partition(": ")
            if not separator:
                raise ValueError(f"invalid compiler depfile: {depfile}")
            names.update(
                re.sub(r"\\(.)", r"\1", token).replace("$$", "$")
                for token in re.findall(r"(?:\\.|[^\s])+", prerequisites)
            )
    return names
