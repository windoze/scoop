"""Compose ordinary file, process, wait, and snapshot steps in one executor."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.execute import execute
from fixture_runner.model import Fixture
from fixture_runner.schema import validate


class ExecutionTests(unittest.TestCase):
    def test_background_signal_files_and_readonly_goldens(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            (base / "expected").write_bytes(b"expected\n")
            script = "import os,pathlib,time; pathlib.Path('ready').write_text(str(os.getpid())); time.sleep(30)"
            data = {
                "schema": 1,
                "name": "composition",
                "inputs": [],
                "steps": [
                    {
                        "name": "prepare",
                        "files": [{"write": {"path": "${work}/value", "data": "expected\n"}}],
                    },
                    {
                        "name": "child",
                        "argv": ["${python}", "-c", script],
                        "background": True,
                        "signal": "SIGTERM",
                        "stdout": "",
                        "stderr": "",
                    },
                    {"name": "ready", "await_file": "${work}/ready"},
                    {"name": "signal", "send_signal": {"process": "child", "signal": "SIGTERM"}},
                    {
                        "name": "wait",
                        "wait": "child",
                        "checks": [
                            {"file": "${work}/value", "snapshot": "expected"},
                            {"glob": "${work}/absent-*", "equals": []},
                        ],
                    },
                ],
            }
            validate(data)
            fixture = Fixture(
                base / "fixture.toml", base, "composition", data, frozenset(), frozenset()
            )
            result = execute(fixture, {"python": sys.executable}, base / "work")
            self.assertEqual(result.status, "passed", result.message)
            self.assertEqual((result.variants, result.processes, result.snapshots), (1, 1, 1))
            self.assertEqual((base / "expected").read_bytes(), b"expected\n")
