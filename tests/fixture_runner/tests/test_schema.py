"""Validate discovery and shared criteria without compiling language fixtures."""

import copy
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.discovery import discover
from fixture_runner.model import ConfigurationError
from fixture_runner.schema import validate
from fixture_runner.values import argv_values, expand


class CriteriaTests(unittest.TestCase):
    def setUp(self):
        self.data = {
            "schema": 1,
            "name": "example",
            "inputs": ["main.scoop"],
            "steps": [
                {"name": "build", "argv": ["${scoop}"], "exit": 0, "stdout": "", "stderr": ""}
            ],
        }

    def test_unknown_conflicting_and_missing_fields(self):
        for changes in ({"unknown": True}, {"signal": "SIGTERM"}, {"json": "stderr"}):
            data = copy.deepcopy(self.data)
            data["steps"][0].update(changes)
            with self.assertRaises(ConfigurationError):
                validate(data)
        data = copy.deepcopy(self.data)
        del data["steps"][0]["stderr"]
        with self.assertRaises(ConfigurationError):
            validate(data)

    def test_concat_validates_parts_and_their_references(self):
        operation = {"path": "combined", "parts": [{"file": "first"}, "\n", {"hex": "00ff"}]}
        self.data["steps"].insert(0, {"name": "combine", "files": [{"concat": operation}]})
        validate(self.data)
        for parts in (
            "first",
            [True],
            [{"file": 3}],
            [{"hex": "zz"}],
            [{"unknown": "x"}],
            ["${build.stdout}"],
        ):
            with self.subTest(parts=parts), self.assertRaises(ConfigurationError):
                operation["parts"] = parts
                validate(self.data)

    def test_references_and_background_must_be_resolved(self):
        self.data["steps"][0]["argv"] = ["${future.result.output}"]
        with self.assertRaisesRegex(ConfigurationError, "forward reference"):
            validate(self.data)
        self.data["steps"][0]["argv"] = ["${scoop}"]
        self.data["steps"][0]["background"] = True
        with self.assertRaisesRegex(ConfigurationError, "without wait"):
            validate(self.data)
        self.data["steps"].append({"name": "finish", "wait": "build"})
        validate(self.data)

    def test_typed_references_and_raw_argument_expansion(self):
        context = {
            "build": {
                "result": {
                    "dependencies": [
                        {"path": "hello"},
                        {"path": {"encoding": "unix-bytes", "hex": "ff"}},
                    ]
                }
            }
        }
        self.assertEqual(expand("${build.result.dependencies.length}", context), 2)
        self.assertEqual(
            argv_values(
                [
                    "",
                    {"hex": "ff"},
                    {"each": "${build.result.dependencies}", "field": "path", "prefix": "--dep"},
                ],
                context,
            ),
            [b"", b"\xff", b"--dep", b"hello", b"--dep", b"\xff"],
        )

    def test_inline_sidecar_and_uncovered_sources(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "main.scoop"
            source.write_text(
                "// fixture:begin\n// schema=1\n// name='inline'\n// inputs=['main.scoop']\n// [[steps]]\n// name='check'\n// checks=[{actual=1,equals=1}]\n// fixture:end\nfun main() {}\n"
            )
            self.assertEqual(discover(root)[0].name, "inline")
            (root / "orphan.scoop").write_text("fun main() {}")
            with self.assertRaisesRegex(ConfigurationError, "no declared ownership"):
                discover(root)
            (root / "orphan.scoop").unlink()
            (root / "main.fixture.toml").write_text("schema=1")
            with self.assertRaisesRegex(ConfigurationError, "duplicate inline"):
                discover(root)

    def test_missing_golden_is_not_accepted_by_normal_discovery(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "fixture.toml").write_text(
                "schema=1\nname='golden'\ninputs=[]\n[[steps]]\nname='check'\nchecks=[{actual='test',snapshot='missing.txt'}]\n"
            )
            with self.assertRaisesRegex(ConfigurationError, "missing expectation"):
                discover(root)
            self.assertEqual(len(discover(root, update=True)), 1)
