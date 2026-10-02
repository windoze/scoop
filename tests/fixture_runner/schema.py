"""Validate one acceptance schema before starting any fixture process."""

import re

from .model import ConfigurationError
from .values import references

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
    "replace": {"path", "old", "new"},
    "patch": {"path", "offset", "hex"},
    "symlink": {"from", "to"},
    "hardlink": {"from", "to"},
    "chmod": {"path", "mode"},
    "touch": {"path"},
}
COMPARISONS = {
    "equals",
    "not_equals",
    "contains",
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
    "ar",
    "sdk",
    "deployment",
    "runtime",
    "variant",
    "variants",
    "target",
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


def validate(data):
    fields(data, TOP, {"schema", "name", "inputs", "steps"}, "fixture")
    if data["schema"] != 1:
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
    available = BUILTINS | set(data.get("vars", {}))
    variants = data.get("variants", [{"name": "normal"}])
    if not variants:
        raise ConfigurationError("variants must be nonempty")
    seen = set()
    for variant in variants:
        fields(variant, {"name", "env", "vars"}, {"name"}, "variant")
        name(variant["name"], "variant name")
        if variant["name"] in seen:
            raise ConfigurationError("duplicate variant name")
        seen.add(variant["name"])
        available |= set(variant.get("vars", {}))
    background = set()
    for step in data["steps"]:
        validate_step(step)
        step_name = step["name"]
        if step_name in available:
            raise ConfigurationError(f"duplicate or reserved step name: {step_name}")
        for reference in references({key: value for key, value in step.items() if key != "checks"}):
            if reference.split(".")[0] not in available:
                raise ConfigurationError(f"{step_name}: unknown or forward reference {reference}")
        available.add(step_name)
        for reference in references(step.get("checks", [])):
            if reference.split(".")[0] not in available:
                raise ConfigurationError(f"{step_name}: unknown check reference {reference}")
        if step.get("background"):
            background.add(step_name)
        if "wait" in step:
            if step["wait"] not in background:
                raise ConfigurationError(f"{step_name}: wait must name an active background step")
            background.remove(step["wait"])
        if "send_signal" in step and step["send_signal"]["process"] not in background:
            raise ConfigurationError(f"{step_name}: signal must name an active background step")
    if background:
        raise ConfigurationError(f"background processes without wait: {sorted(background)}")


def validate_step(step):
    actions = set(step) & ({"argv", "files", "wait", "await_file", "send_signal"})
    if len(actions) > 1:
        raise ConfigurationError("step has conflicting actions")
    action = next(iter(actions), None)
    allowed = BASE | (PROCESS if action == "argv" else {action, "timeout"} if action else set())
    fields(step, allowed, {"name"}, "step")
    name(step["name"], "step name")
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
                else:
                    fields(argument, {"hex"}, {"hex"}, "byte argument")
            elif not isinstance(argument, str):
                raise ConfigurationError("argv entries must be strings or byte/array values")
        if step.get("json") not in (None, "stderr", "stdout"):
            raise ConfigurationError("json must be stderr or stdout")
        if step.get("json") == "stderr" and "diagnostics" not in step:
            raise ConfigurationError(
                "structured stderr requires the complete diagnostics expectation"
            )
        if "diagnostics" in step and step.get("json") != "stderr":
            raise ConfigurationError("diagnostics requires json = 'stderr'")
    elif action == "files":
        for operation in step["files"]:
            if not isinstance(operation, dict) or len(operation) != 1:
                raise ConfigurationError("file operation must have exactly one action")
            kind, value = next(iter(operation.items()))
            if kind not in FILES:
                raise ConfigurationError(f"unknown file operation {kind}")
            fields(value, FILES[kind], FILES[kind], kind)
    elif action == "send_signal":
        fields(step[action], {"process", "signal"}, {"process", "signal"}, action)
    for check in step.get("checks", []):
        fields(check, {"actual", "file", "glob", "normalize"} | COMPARISONS, set(), "check")
        if len(set(check) & {"actual", "file", "glob"}) != 1 or len(set(check) & COMPARISONS) != 1:
            raise ConfigurationError("check requires one subject and one comparison")
        if any(rule not in {"paths", "newlines"} for rule in check.get("normalize", [])):
            raise ConfigurationError("unknown normalization rule")
        if (
            set(check) & {"same_as", "different_from", "exists", "type", "sha256"}
            and "file" not in check
        ):
            raise ConfigurationError("file comparison requires a file subject")
