"""Validate one acceptance schema before starting any fixture process."""

import re

from . import schema_values as values
from .model import ConfigurationError
from .values import json_value, references

TOP = {
    "schema",
    "name",
    "root",
    "inputs",
    "support",
    "tags",
    "tools",
    "targets",
    "vars",
    "variants",
    "steps",
}
BASE = {"name", "checks"}
PROCESS = {
    "argv",
    "cwd",
    "env",
    "stdin",
    "exit",
    "signal",
    "stdout",
    "stderr",
    "json",
    "diagnostics",
    "timeout",
    "background",
}
FILES = {
    "copy": {"from", "to"},
    "move": {"from", "to"},
    "remove": {"path"},
    "mkdir": {"path"},
    "write": {"path", "data"},
    "concat": {"path", "parts"},
    "replace": {"path", "old", "new"},
    "patch": {"path", "offset", "hex"},
    "truncate": {"path", "size"},
    "symlink": {"from", "to"},
    "hardlink": {"from", "to"},
    "chmod": {"path", "mode"},
    "touch": {"path"},
}
COMPARISONS = {
    "equals",
    "not_equals",
    "contains",
    "not_contains",
    "snapshot",
    "same_as",
    "different_from",
    "exists",
    "type",
    "sha256",
}
BUILTINS = {
    "fixture",
    "root",
    "work",
    "repo",
    "sysroot",
    "cache",
    "scoop",
    "scoopc",
    "scoop-link",
    "cc",
    "cc_args",
    "compile_args",
    "link_args",
    "ar",
    "sdk",
    "deployment",
    "runtime",
    "variant",
    "variants",
    "target",
    "target_profile",
    "symbol_prefix",
    "errno_eloop",
    "python",
}


def fields(value, allowed, required, where):
    if not isinstance(value, dict):
        raise ConfigurationError(f"{where}: expected a table")
    extra, missing = set(value) - allowed, required - set(value)
    if extra or missing:
        raise ConfigurationError(
            f"{where}: unknown fields {sorted(extra)}, missing fields {sorted(missing)}"
        )


def name(value, where):
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9-]*", value):
        raise ConfigurationError(f"{where}: expected an identifier")


def variables(value, where):
    values.require(isinstance(value, dict), f"{where}: expected a variables table")
    for key in value:
        name(key, where)
        values.require(key not in BUILTINS, f"{where}: reserved variable {key}")


def validate(data, base=None):
    fields(data, TOP, {"schema", "name", "inputs", "steps"}, "fixture")
    if type(data["schema"]) is not int or data["schema"] != 1:
        raise ConfigurationError("fixture schema must be 1")
    if not isinstance(data["name"], str) or not data["name"]:
        raise ConfigurationError("fixture name must be nonempty")
    for key in ("inputs", "support", "tags", "tools", "targets"):
        if not isinstance(data.get(key, []), list) or any(
            not isinstance(item, str) for item in data.get(key, [])
        ):
            raise ConfigurationError(f"{key}: expected an array of strings")
    if not isinstance(data["steps"], list) or not data["steps"]:
        raise ConfigurationError("fixture steps must be nonempty")
    if "root" in data:
        values.string(data["root"], "root", nonempty=True)
    variables(data.get("vars", {}), "vars")
    reserved = BUILTINS | set(data.get("vars", {}))
    variants = data.get("variants", [{"name": "normal"}])
    values.require(isinstance(variants, list) and variants, "variants must be a nonempty array")
    seen = set()
    for variant in variants:
        fields(variant, {"name", "env", "vars"}, {"name"}, "variant")
        name(variant["name"], "variant name")
        if variant["name"] in seen:
            raise ConfigurationError("duplicate variant name")
        seen.add(variant["name"])
        variables(variant.get("vars", {}), "variant vars")
        values.environment(variant.get("env", {}), "variant env")
        reserved |= set(variant.get("vars", {}))
    for step in data["steps"]:
        if base is not None and isinstance(step, dict) and "diagnostics" in step:
            try:
                step["diagnostics"] = json_value(step["diagnostics"], base)
            except (OSError, ValueError) as error:
                raise ConfigurationError(f"diagnostics expectation: {error}") from error
        validate_step(step)
        step_name = step["name"]
        if step_name in reserved:
            raise ConfigurationError(f"duplicate or reserved step name: {step_name}")
        reserved.add(step_name)
    previous = set()
    for variant in variants:
        validate_references(data, variant, previous)
        previous.add(variant["name"])


def check_references(value, available, background, previous, where):
    for reference in references(value):
        parts = reference.split(".")
        if parts[0] not in available:
            raise ConfigurationError(f"{where}: unknown or forward reference {reference}")
        if parts[0] == "variants" and len(parts) > 1 and parts[1] not in previous:
            raise ConfigurationError(f"{where}: unknown or forward variant reference {reference}")
        if parts[0] in background and parts[1:] != ["pid"]:
            raise ConfigurationError(f"{where}: only pid is available before wait: {reference}")


def validate_references(data, variant, previous):
    available, background = set(BUILTINS), set()
    for value in (data.get("vars", {}), variant.get("vars", {})):
        check_references(value, available, background, previous, "vars")
        available.update(value)
    check_references(variant.get("env", {}), available, background, previous, "variant env")
    for step in data["steps"]:
        step_name = step["name"]
        inputs = {key: value for key, value in step.items() if key != "checks"}
        check_references(inputs, available, background, previous, step_name)
        available.add(step_name)
        if step.get("background"):
            background.add(step_name)
        if "wait" in step:
            if step["wait"] not in background:
                raise ConfigurationError(f"{step_name}: wait must name an active background step")
            background.remove(step["wait"])
        if "send_signal" in step and step["send_signal"]["process"] not in background:
            raise ConfigurationError(f"{step_name}: signal must name an active background step")
        check_references(step.get("checks", []), available, background, previous, step_name)
    if background:
        raise ConfigurationError(f"background processes without wait: {sorted(background)}")


def validate_step(step):
    values.require(isinstance(step, dict), "step must be a table")
    actions = set(step) & ({"argv", "files", "wait", "await_file", "send_signal"})
    if len(actions) > 1:
        raise ConfigurationError("step has conflicting actions")
    action = next(iter(actions), None)
    allowed = BASE | (PROCESS if action == "argv" else {action, "timeout"} if action else set())
    fields(step, allowed, {"name"}, "step")
    name(step["name"], "step name")
    values.timeout(step)
    if action == "argv":
        if ("exit" in step) == ("signal" in step):
            raise ConfigurationError("process must declare exactly one of exit or signal")
        fields(step, allowed, {"stdout", "stderr", "argv"}, "process")
        if not isinstance(step["argv"], list) or not step["argv"]:
            raise ConfigurationError("argv must be a nonempty array")
        for argument in step["argv"]:
            if isinstance(argument, dict):
                if "each" in argument:
                    fields(
                        argument,
                        {"each", "field", "prefix"},
                        {"each"},
                        "argv expansion",
                    )
                    values.require(
                        isinstance(argument["each"], (str, list)),
                        "argv each must be an array or reference",
                    )
                    for key in ("field", "prefix"):
                        if key in argument:
                            values.string(argument[key], f"argv {key}", nonempty=True)
                else:
                    fields(argument, {"hex"}, {"hex"}, "byte argument")
                    values.hex_value(argument["hex"], "byte argument")
            elif not isinstance(argument, str):
                raise ConfigurationError("argv entries must be strings or byte/array values")
            else:
                values.string(argument, "argument")
        if step.get("json") not in (None, "stderr", "stdout"):
            raise ConfigurationError("json must be stderr or stdout")
        if step.get("json") == "stderr" and "diagnostics" not in step:
            raise ConfigurationError(
                "structured stderr requires the complete diagnostics expectation"
            )
        if "diagnostics" in step and step.get("json") != "stderr":
            raise ConfigurationError("diagnostics requires json = 'stderr'")
        values.process_values(step)
    elif action == "files":
        values.require(isinstance(step["files"], list), "files must be an array")
        for operation in step["files"]:
            if not isinstance(operation, dict) or len(operation) != 1:
                raise ConfigurationError("file operation must have exactly one action")
            kind, value = next(iter(operation.items()))
            if kind not in FILES:
                raise ConfigurationError(f"unknown file operation {kind}")
            fields(value, FILES[kind], FILES[kind], kind)
            values.file_values(kind, value)
    elif action == "send_signal":
        fields(step[action], {"process", "signal"}, {"process", "signal"}, action)
        name(step[action]["process"], "signal process")
        values.signal_value(step[action]["signal"], "signal")
    elif action == "wait":
        name(step[action], "wait process")
    elif action == "await_file":
        values.byte_value(step[action], action, allow_file=False, is_path=True)
    values.require(isinstance(step.get("checks", []), list), "checks must be an array")
    for check in step.get("checks", []):
        fields(check, {"actual", "file", "glob", "normalize"} | COMPARISONS, set(), "check")
        if len(set(check) & {"actual", "file", "glob"}) != 1 or len(set(check) & COMPARISONS) != 1:
            raise ConfigurationError("check requires one subject and one comparison")
        values.check_values(check)
        if any(rule not in {"paths", "newlines"} for rule in check.get("normalize", [])):
            raise ConfigurationError("unknown normalization rule")
        if (
            set(check) & {"same_as", "different_from", "exists", "type", "sha256"}
            and "file" not in check
        ):
            raise ConfigurationError("file comparison requires a file subject")
