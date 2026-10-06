"""Stale-dependency normalization retains the exact dependency and change relations."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.assertions import process_expectations
from fixture_runner.dependency_digests import normalize_dependency_digests
from fixture_runner.model import AssertionFailure, ConfigurationError
from fixture_runner.schema import validate_step


def message(values="123423"):
    digests = [character * 64 for character in values]
    return (
        "StaleDependency: Cone "
        + "a" * 64
        + " requires dev.example:provider:0.1.0 ("
        + "b" * 64
        + "); recorded HIR/MIR/LIR "
        + "/".join(digests[:3])
        + ", actual "
        + "/".join(digests[3:])
    )


class DependencyDigestTests(unittest.TestCase):
    def test_only_explicit_content_fields_change_and_relationships_remain(self):
        raw = message().encode()
        expected = normalize_dependency_digests(raw)
        self.assertEqual(expected, normalize_dependency_digests(message("567867").encode()))
        self.assertEqual(expected, normalize_dependency_digests(expected))
        self.assertIn(b"Cone " + b"a" * 64, expected)
        self.assertIn(b"(" + b"b" * 64 + b")", expected)
        for changed in [
            message("123123"),
            message("123453"),
            message("123432"),
            message().replace("dev.example:provider", "dev.example:other"),
            message().replace("a" * 64, "c" * 64),
            message().replace("b" * 64, "d" * 64),
        ]:
            self.assertNotEqual(expected, normalize_dependency_digests(changed.encode()))
        for raw in [b"unrelated=" + b"1" * 64, message().replace("1" * 64, "1" * 63).encode()]:
            self.assertEqual(raw, normalize_dependency_digests(raw))

    def test_complete_diagnostics_stay_exact_including_during_snapshot_update(self):
        diagnostic = {
            "code": "SCOOP_LINK_FAILED",
            "message": message(),
            "notes": [],
            "origin": {
                "kind": "artifact",
                "path": "provider.slib",
                "member": "manifest:direct_dependencies",
            },
            "phase": "FinalLink",
            "severity": "error",
        }
        expected = dict(
            diagnostic, message=normalize_dependency_digests(message().encode()).decode()
        )
        step = {
            "name": "reject_stale",
            "argv": ["scoop"],
            "exit": 1,
            "stdout": "",
            "stderr": "",
            "json": "stderr",
            "diagnostics": [expected],
            "diagnostics_normalize": ["dependency-digests"],
        }
        result = {
            "returncode": 1,
            "stdout": b"",
            "stderr": b"",
            "raw_stderr": b"",
            "diagnostics": [diagnostic],
        }
        validate_step(step)
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(process_expectations(step, result, {}, Path(directory)), 0)
            for changes in [
                {"message": message("123123")},
                {"code": "OTHER"},
                {"phase": "Compile"},
                {"origin": {"kind": "none"}},
                {"notes": ["unexpected"]},
            ]:
                with self.assertRaises(AssertionFailure):
                    process_expectations(
                        step,
                        dict(result, diagnostics=[dict(diagnostic, **changes)]),
                        {},
                        Path(directory),
                        True,
                    )
        with self.assertRaises(ConfigurationError):
            validate_step(
                dict(
                    step,
                    checks=[{"actual": "x", "equals": "x", "normalize": ["dependency-digests"]}],
                )
            )
