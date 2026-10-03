"""Malformed criteria must fail during discovery, before tool execution."""

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.discovery import discover
from fixture_runner.model import ConfigurationError
from fixture_runner.schema import validate


def criteria():
    return {
        "schema": 1,
        "name": "preflight",
        "inputs": [],
        "steps": [{"name": "tool", "argv": ["${scoop}"], "exit": 0, "stdout": "", "stderr": ""}],
    }


class PreflightTests(unittest.TestCase):
    def test_nested_process_types_fail_before_execution(self):
        for changes in (
            {"env": {"MODE": []}},
            {"env": {"A=B": "1"}},
            {"exit": True},
            {"exit": -15},
            {"timeout": "20"},
            {"timeout": float("nan")},
            {"background": "yes"},
            {"stdout": {"hex": "zz"}},
            {"stdout": {"file": 7}},
            {"cwd": {"hex": "00"}},
            {"json": "stderr", "diagnostics": ["any failure"]},
        ):
            with self.subTest(changes=changes):
                data = criteria()
                data["steps"][0].update(changes)
                with self.assertRaises(ConfigurationError):
                    validate(data)

    def test_nested_file_and_check_types_fail_before_execution(self):
        for step in (
            {"files": "not an array"},
            {"files": [{"truncate": {"path": "x", "size": True}}]},
            {"files": [{"patch": {"path": "x", "offset": -1, "hex": "00"}}]},
            {"files": [{"chmod": {"path": "x", "mode": "888"}}]},
            {"checks": [{"file": "x", "exists": "false"}]},
            {"checks": [{"file": "x", "sha256": "invalid"}]},
            {"checks": [{"actual": "a", "equals": "a", "normalize": [{}]}]},
        ):
            with self.subTest(step=step):
                data = criteria()
                data["steps"] = [{"name": "invalid"} | step]
                with self.assertRaises(ConfigurationError):
                    validate(data)

    def test_variant_variables_are_checked_in_their_own_scope(self):
        data = criteria()
        data["variants"] = [
            {"name": "normal"},
            {"name": "moving", "vars": {"option": "--moving"}},
        ]
        data["steps"][0]["argv"].append("${option}")
        with self.assertRaisesRegex(ConfigurationError, "forward reference option"):
            validate(data)
        data["vars"] = {"option": "--normal"}
        validate(data)
        data["variants"][0]["env"] = {"RESULT": "${variants.moving.tool.stdout}"}
        with self.assertRaisesRegex(ConfigurationError, "forward variant reference"):
            validate(data)

    def test_background_results_become_available_only_after_wait(self):
        data = criteria()
        data["steps"][0]["background"] = True
        data["steps"] += [
            {"name": "observe", "checks": [{"actual": "${tool.stdout}", "equals": ""}]},
            {"name": "finish", "wait": "tool"},
        ]
        with self.assertRaisesRegex(ConfigurationError, "only pid is available before wait"):
            validate(data)
        data["steps"][1]["checks"][0]["actual"] = "${tool.pid}"
        data["steps"][2]["checks"] = [{"actual": "${tool.stdout}", "equals": ""}]
        validate(data)

    def test_long_diagnostics_are_loaded_and_references_checked_during_discovery(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            (base / "fixture.toml").write_text(
                "schema=1\nname='long-diagnostic'\ninputs=[]\n[[steps]]\nname='reject'\n"
                "argv=['${scoop}']\nexit=1\nstdout=''\nstderr=''\njson='stderr'\n"
                "diagnostics={file='diagnostics.json'}\n"
            )
            expected = [{"message": "${future.result.output}"}]
            (base / "diagnostics.json").write_text(json.dumps(expected))
            with self.assertRaisesRegex(ConfigurationError, "forward reference future"):
                discover(base)
            expected[0]["message"] = "${work}/input was rejected"
            (base / "diagnostics.json").write_text(json.dumps(expected))
            found = discover(base)[0]
            self.assertEqual(found.data["steps"][0]["diagnostics"], expected)
            (base / "diagnostics.json").write_text("{")
            with self.assertRaisesRegex(ConfigurationError, "diagnostics expectation"):
                discover(base)

    def test_variable_initializers_cannot_reference_later_bindings(self):
        data = criteria()
        data["vars"] = {"option": "${missing}"}
        with self.assertRaisesRegex(ConfigurationError, "forward reference missing"):
            validate(data)
        data["vars"] = {"option": "ready"}
        valid = copy.deepcopy(data)
        validate(valid)
        data["variants"] = [{"name": "normal", "vars": {"target": "override"}}]
        with self.assertRaisesRegex(ConfigurationError, "reserved variable target"):
            validate(data)
