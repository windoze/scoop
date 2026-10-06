"""Validate scalar and nested criterion values before launching a process."""

import math
import re
import signal

from .model import ConfigurationError
from .values import REFERENCE


def require(condition, message):
    if not condition:
        raise ConfigurationError(message)


def string(value, where, nonempty=False):
    require(isinstance(value, str), f"{where}: expected a string")
    require("\0" not in value and (value or not nonempty), f"{where}: invalid string")


def integer(value, where, minimum=0):
    require(type(value) is int and value >= minimum, f"{where}: expected integer >= {minimum}")


def hex_value(value, where):
    string(value, where)
    if REFERENCE.fullmatch(value):
        return
    try:
        bytes.fromhex(value)
    except ValueError as error:
        raise ConfigurationError(f"{where}: invalid hexadecimal bytes") from error


def byte_value(value, where, allow_file=True, is_path=False):
    if isinstance(value, str):
        if is_path:
            string(value, where, nonempty=True)
        return
    allowed = {"hex", "file"} if allow_file else {"hex"}
    require(
        isinstance(value, dict) and len(value) == 1 and bool(set(value) & allowed),
        f"{where}: expected text or a byte/file carrier",
    )
    if "hex" in value:
        hex_value(value["hex"], where)
        if is_path and not REFERENCE.fullmatch(value["hex"]):
            raw = bytes.fromhex(value["hex"])
            require(bool(raw) and b"\0" not in raw, f"{where}: invalid path bytes")
    else:
        string(value["file"], where, nonempty=True)


def environment(value, where):
    require(isinstance(value, dict), f"{where}: expected an environment table")
    for key, item in value.items():
        string(key, where, nonempty=True)
        require("=" not in key, f"{where}: invalid environment key")
        string(item, f"{where}.{key}")


def signal_value(value, where):
    if isinstance(value, str):
        value = signal.Signals.__members__.get(value)
    require(
        isinstance(value, int) and not isinstance(value, bool) and 0 < value < signal.NSIG,
        f"{where}: invalid signal",
    )


def process_values(step):
    if "exit" in step:
        integer(step["exit"], "exit")
        require(step["exit"] <= 255, "exit must be in 0..255; use signal for termination")
    if "signal" in step:
        signal_value(step["signal"], "signal")
    for stream in ("stdin", "stdout", "stderr"):
        if stream in step:
            value = step[stream]
            if stream != "stdin" and isinstance(value, dict) and set(value) == {"snapshot"}:
                string(value["snapshot"], f"{stream} snapshot", nonempty=True)
            else:
                byte_value(value, stream)
    if "env" in step:
        environment(step["env"], "env")
    if "cwd" in step:
        byte_value(step["cwd"], "cwd", allow_file=False, is_path=True)
    if "background" in step:
        require(type(step["background"]) is bool, "background must be boolean")
    if "diagnostics" in step:
        value = step["diagnostics"]
        if isinstance(value, dict):
            require(set(value) == {"file"}, "diagnostics must be an array or file")
            string(value["file"], "diagnostics file", nonempty=True)
        else:
            require(
                isinstance(value, list) and all(isinstance(item, dict) for item in value),
                "diagnostics must be an array of complete records",
            )
    if "diagnostics_normalize" in step:
        normalization_rules(step["diagnostics_normalize"], {"native-digests"})


def timeout(step):
    if "timeout" in step:
        value = step["timeout"]
        require(
            type(value) in (int, float) and math.isfinite(value) and value > 0,
            "timeout must be a positive finite number",
        )


def file_values(kind, value):
    for key in ("path", "from", "to"):
        if key in value:
            byte_value(value[key], f"{kind}.{key}", allow_file=False, is_path=True)
    for key in ("data", "old", "new"):
        if key in value:
            byte_value(value[key], f"{kind}.{key}")
    if kind == "concat":
        require(isinstance(value["parts"], list), "concat.parts: expected an array")
        for part in value["parts"]:
            byte_value(part, "concat.parts")
    if kind == "patch":
        integer(value["offset"], "patch offset")
        hex_value(value["hex"], "patch bytes")
    if kind == "truncate":
        integer(value["size"], "truncate size")
    if kind == "chmod":
        require(
            isinstance(value["mode"], str) and re.fullmatch(r"[0-7]{3,4}", value["mode"]),
            "chmod mode must be three or four octal digits",
        )


def check_values(check):
    for key in ("file", "glob", "snapshot", "same_as", "different_from"):
        if key in check:
            byte_value(check[key], key, allow_file=False, is_path=True)
    if "exists" in check:
        require(type(check["exists"]) is bool, "exists must be boolean")
    if "type" in check:
        require(
            check["type"] in ("file", "directory", "symlink", "missing"),
            "type must be file, directory, symlink, or missing",
        )
    if "sha256" in check:
        value = check["sha256"]
        string(value, "sha256")
        require(
            REFERENCE.fullmatch(value) or re.fullmatch(r"[0-9a-f]{64}", value),
            "sha256 must be a lowercase hexadecimal digest",
        )
    normalization_rules(check.get("normalize", []), {"paths", "newlines", "native-digests"})


def normalization_rules(rules, allowed):
    require(
        isinstance(rules, list) and all(isinstance(rule, str) for rule in rules),
        "normalize must be an array of rule names",
    )
    require(all(rule in allowed for rule in rules), "unknown normalization rule")
