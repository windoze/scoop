"""Exercise concurrent isolation and cancellation with real child processes."""

import contextlib
import io
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.model import Fixture
from fixture_runner.schema import validate
from fixture_runner.suite import run_suite


def fixture(base, name, steps):
    data = {"schema": 1, "name": name, "inputs": [], "steps": steps}
    validate(data)
    return Fixture(base / f"{name}.fixture.toml", base, name, data, frozenset(), frozenset())


class SuiteTests(unittest.TestCase):
    def test_compile_only_runs_complete_host_fixtures_and_records_skipped_programs(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            step = {
                "name": "write",
                "files": [{"write": {"path": "${work}/marker", "data": "ran"}}],
            }
            host = fixture(base, "host", [step])
            host.data["requires_target_execution"] = False
            program = fixture(base, "program", [step])
            with contextlib.redirect_stdout(io.StringIO()):
                results = run_suite([host, program], {}, base, False, 2, compile_only=True)
            self.assertEqual([result.status for result in results], ["passed", "execution-skipped"])
            self.assertTrue((base / "cases/0/normal/marker").is_file())
            self.assertFalse((base / "cases/1").exists())

    def test_fixtures_overlap_but_keep_private_work_and_report_order(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            fixtures = []
            for name, peer in (("first", "second"), ("second", "first")):
                fixtures.append(
                    fixture(
                        base,
                        name,
                        [
                            {
                                "name": "ready",
                                "files": [
                                    {"write": {"path": "${work}/value", "data": name}},
                                    {"write": {"path": "${cache}/" + name, "data": "ready"}},
                                ],
                            },
                            {"name": "peer", "await_file": "${cache}/" + peer, "timeout": 3},
                            {
                                "name": "check",
                                "checks": [
                                    {"file": "${work}/value", "equals": name},
                                ],
                            },
                        ],
                    )
                )
            with contextlib.redirect_stdout(io.StringIO()):
                results = run_suite(fixtures, {"cache": str(base)}, base, False, 2)
            self.assertEqual([result.name for result in results], ["first", "second"])
            self.assertEqual([result.status for result in results], ["passed", "passed"])
            self.assertNotEqual(
                (base / "cases/0/normal/value").read_bytes(),
                (base / "cases/1/normal/value").read_bytes(),
            )

    def test_interrupt_reaps_running_child_and_never_starts_queued_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            step = {
                "name": "child",
                "argv": [
                    "${python}",
                    "-c",
                    "import os,pathlib,time; pathlib.Path('ready').write_text(str(os.getpid())); time.sleep(30)",
                ],
                "exit": 0,
                "stdout": "",
                "stderr": "",
            }
            fixtures = [fixture(base, name, [step]) for name in ("active", "queued")]
            ready = base / "cases/0/normal/ready"

            def interrupt_when_running(_):
                deadline = time.monotonic() + 3
                while not ready.exists():
                    if time.monotonic() >= deadline:
                        self.fail("fixture child did not become ready")
                    time.sleep(0.01)
                raise KeyboardInterrupt

            with (
                patch("fixture_runner.suite.as_completed", interrupt_when_running),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                results = run_suite(fixtures, {"python": sys.executable}, base, False, 1)
            self.assertEqual([result.status for result in results], ["interrupted", "interrupted"])
            self.assertEqual([result.processes for result in results], [1, 0])
            self.assertFalse((base / "cases/1/normal/ready").exists())
            if os.name == "nt":
                import ctypes
                from ctypes import wintypes

                from fixture_runner.windows_process import close_handle, function

                open_process = function(
                    "OpenProcess", wintypes.HANDLE, wintypes.DWORD, wintypes.BOOL, wintypes.DWORD
                )
                wait = function(
                    "WaitForSingleObject", wintypes.DWORD, wintypes.HANDLE, wintypes.DWORD
                )
                handle = open_process(0x00100000, False, int(ready.read_text()))
                if handle:
                    try:
                        self.assertEqual(wait(handle, 0), 0)
                    finally:
                        close_handle(handle)
                else:
                    self.assertEqual(ctypes.get_last_error(), 87)  # Process no longer exists.
            else:
                with self.assertRaises(ProcessLookupError):
                    os.kill(int(ready.read_text()), 0)
