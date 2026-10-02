"""Typed references and byte-preserving process values; never shell strings."""

import json
import os
import re
from pathlib import Path

from .model import ConfigurationError

REFERENCE = re.compile(r"\$\{([A-Za-z_][A-Za-z_0-9.-]*)\}")


def lookup(name, context):
    value = context
    try:
        for part in name.split("."):
            if part == "length":
                value = len(value)
            elif isinstance(value, list):
                value = value[int(part)]
            else:
                value = value[part]
    except (KeyError, IndexError, ValueError, TypeError) as error:
        raise ConfigurationError(f"unresolved reference ${{{name}}}") from error
    return value


def expand(value, context):
    if isinstance(value, str):
        match = REFERENCE.fullmatch(value)
        if match:
            return lookup(match[1], context)
        return REFERENCE.sub(lambda match: scalar(lookup(match[1], context)), value)
    if isinstance(value, list):
        return [expand(item, context) for item in value]
    if isinstance(value, dict):
        return {key: expand(item, context) for key, item in value.items()}
    return value


def scalar(value):
    if isinstance(value, (str, bytes, os.PathLike)):
        return os.fsdecode(value)
    if isinstance(value, (int, float, bool)):
        return str(value)
    raise ConfigurationError(f"cannot interpolate a structured value: {value!r}")


def references(value):
    if isinstance(value, str):
        yield from (match[1] for match in REFERENCE.finditer(value))
    elif isinstance(value, list):
        for item in value:
            yield from references(item)
    elif isinstance(value, dict):
        for item in value.values():
            yield from references(item)


def path_bytes(value):
    if isinstance(value, dict):
        if value.get("encoding") == "unix-bytes":
            return bytes.fromhex(value["hex"])
        if value.get("encoding") == "windows-wide":
            raw = b"".join(unit.to_bytes(2, "little") for unit in value["units"])
            return os.fsencode(raw.decode("utf-16-le", "surrogatepass"))
        if set(value) == {"hex"}:
            return bytes.fromhex(value["hex"])
        raise ConfigurationError(f"invalid path/argument carrier: {value!r}")
    return os.fsencode(value)


def path(value, base=None):
    result = Path(os.fsdecode(path_bytes(value)))
    return result if result.is_absolute() or base is None else base / result


def byte_value(value, base):
    if isinstance(value, bytes):
        return value
    if isinstance(value, str):
        return value.encode("utf-8", "surrogateescape")
    if isinstance(value, dict) and set(value) == {"file"}:
        return path(value["file"], base).read_bytes()
    if isinstance(value, dict) and set(value) == {"hex"}:
        return bytes.fromhex(value["hex"])
    raise ConfigurationError(f"expected text, bytes, {{file}}, or {{hex}}: {value!r}")


def json_value(value, base):
    if isinstance(value, dict) and set(value) == {"file"}:
        return json.loads(path(value["file"], base).read_text())
    return value


def argv_values(values, context):
    result = []
    for item in values:
        if isinstance(item, dict) and "each" in item:
            rows = expand(item["each"], context)
            if not isinstance(rows, list):
                raise ConfigurationError("argv each must resolve to an array")
            for row in rows:
                if "prefix" in item:
                    result.append(path_bytes(expand(item["prefix"], context)))
                value = lookup(item["field"], row) if "field" in item else row
                result.append(path_bytes(value))
        else:
            result.append(path_bytes(expand(item, context)))
    return result
