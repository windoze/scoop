"""Exact process expectations, file comparisons, and explicit golden updates."""

import difflib
import glob
import hashlib
import json
import signal

from .model import AssertionFailure
from .schema import COMPARISONS
from .values import byte_value, expand, json_value, path


def equal(actual, expected, label):
    if actual == expected:
        return
    if isinstance(actual, bytes):
        actual = actual.decode("utf-8", "backslashreplace")
    if isinstance(expected, bytes):
        expected = expected.decode("utf-8", "backslashreplace")
    if not isinstance(actual, str):
        actual = json.dumps(actual, ensure_ascii=False, indent=2, default=str)
    if not isinstance(expected, str):
        expected = json.dumps(expected, ensure_ascii=False, indent=2, default=str)
    diff = "".join(
        difflib.unified_diff(
            expected.splitlines(True),
            actual.splitlines(True),
            fromfile="expected",
            tofile="actual",
        )
    )
    raise AssertionFailure(f"{label} differs:\n{diff or f'{expected!r} != {actual!r}'}")


def signal_number(value):
    return int(value) if isinstance(value, int) else int(getattr(signal, value))


def process_expectations(step, result, context, base, update=False):
    expected = step.get("exit") if "exit" in step else -signal_number(step["signal"])
    equal(
        result["returncode"],
        expected,
        f"{step['name']} exit/signal (stderr: {result['raw_stderr'][:8000]!r})",
    )
    snapshots = 0
    for stream in ("stdout", "stderr"):
        criterion = step[stream]
        if isinstance(criterion, dict) and set(criterion) == {"snapshot"}:
            snapshots += check_all(
                [{"actual": result[stream], "snapshot": criterion["snapshot"]}],
                context,
                base,
                update,
            )
        else:
            equal(
                result[stream],
                byte_value(expand(criterion, context), base),
                f"{step['name']} {stream}",
            )
    if "diagnostics" in step:
        expected = expand(json_value(step["diagnostics"], base), context)
        equal(result["diagnostics"], expected, f"{step['name']} complete diagnostics")
    return snapshots


def normalize(data, rules, context):
    if "newlines" in rules:
        data = data.replace(b"\r\n", b"\n")
    if "paths" in rules:
        replacements = []
        for key in ("work", "fixture", "cache", "sysroot", "repo"):
            original = path(context[key])
            for spelling in {str(original), str(original.resolve())}:
                replacements.append((spelling.encode(), f"${key.upper()}".encode()))
        for original, replacement in sorted(
            replacements, key=lambda pair: len(pair[0]), reverse=True
        ):
            data = data.replace(original, replacement)
    return data


def check_all(checks, context, base, update):
    snapshots = 0
    for declaration in checks:
        check = expand(declaration, context)
        comparison = next(iter(set(check) & COMPARISONS))
        is_file = "file" in check
        if "glob" in check:
            subject = sorted(glob.glob(str(path(check["glob"], base))))
        else:
            subject = path(check["file"], base) if is_file else check["actual"]
        expected = check[comparison]
        if comparison == "exists":
            equal(subject.exists() or subject.is_symlink(), expected, str(subject))
            continue
        if comparison == "type":
            kind = (
                "symlink"
                if subject.is_symlink()
                else "file"
                if subject.is_file()
                else "directory"
                if subject.is_dir()
                else "missing"
            )
            equal(kind, expected, str(subject))
            continue
        actual = subject.read_bytes() if is_file else subject
        if comparison == "snapshot":
            snapshots += 1
            if not isinstance(actual, bytes):
                actual = (
                    actual.encode()
                    if isinstance(actual, str)
                    else (
                        json.dumps(actual, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
                    ).encode()
                )
            actual = normalize(actual, check.get("normalize", []), context)
            target = path(expected, base)
            if update:
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(actual)
            else:
                equal(actual, target.read_bytes(), str(target))
        elif comparison in ("same_as", "different_from"):
            other = path(expected, base).read_bytes()
            equal(actual == other, comparison == "same_as", f"{subject} vs {expected}")
        elif comparison == "sha256":
            equal(hashlib.sha256(actual).hexdigest(), expected, str(subject))
        else:
            if isinstance(actual, bytes):
                expected = byte_value(expected, base)
                actual = normalize(actual, check.get("normalize", []), context)
            else:
                expected = json_value(expected, base)
            if comparison == "equals":
                equal(actual, expected, str(subject) if is_file else "JSON value")
            elif comparison == "not_equals":
                equal(actual == expected, False, "values must differ")
            elif comparison in ("contains", "not_contains"):
                equal(expected in actual, comparison == "contains", f"{comparison}: {expected!r}")
    return snapshots
