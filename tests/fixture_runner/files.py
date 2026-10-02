"""Ordinary file preparation for relocation, corruption, and output tests."""

import os
import shutil

from .model import ConfigurationError
from .values import byte_value, expand, path


def prepare(operations, context, base):
    for operation in expand(operations, context):
        kind, value = next(iter(operation.items()))
        target = path(value.get("to", value.get("path")), base)
        source = path(value["from"], base) if "from" in value else None
        if kind in {"copy", "move", "write", "symlink", "hardlink", "touch"}:
            target.parent.mkdir(parents=True, exist_ok=True)
        if kind == "copy":
            if source.is_dir():
                shutil.copytree(source, target, dirs_exist_ok=True, symlinks=True)
            else:
                shutil.copy2(source, target)
        elif kind == "move":
            shutil.move(source, target)
        elif kind == "remove":
            if target.is_symlink() or target.is_file():
                target.unlink()
            elif target.is_dir():
                shutil.rmtree(target)
            else:
                raise ConfigurationError(f"cannot remove missing file {target}")
        elif kind == "mkdir":
            target.mkdir(parents=True, exist_ok=True)
        elif kind == "write":
            target.write_bytes(byte_value(value["data"], base))
        elif kind == "replace":
            old, new = byte_value(value["old"], base), byte_value(value["new"], base)
            contents = target.read_bytes()
            if contents.count(old) != 1:
                raise ConfigurationError(f"{target}: replacement must match exactly once")
            target.write_bytes(contents.replace(old, new))
        elif kind == "patch":
            contents = bytearray(target.read_bytes())
            replacement, offset = bytes.fromhex(value["hex"]), value["offset"]
            if offset < 0 or offset + len(replacement) > len(contents):
                raise ConfigurationError("byte patch is outside the file")
            contents[offset : offset + len(replacement)] = replacement
            target.write_bytes(contents)
        elif kind == "symlink":
            target.symlink_to(source, target_is_directory=source.is_dir())
        elif kind == "hardlink":
            os.link(source, target)
        elif kind == "chmod":
            target.chmod(int(value["mode"], 8))
        elif kind == "touch":
            target.touch()
