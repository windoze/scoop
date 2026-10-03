"""Actual POSIX status, byte streams, diagnostic completeness, and cleanup."""

import json
import os
import signal
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.assertions import process_expectations
from fixture_runner.model import AssertionFailure
from fixture_runner.process import Process, decode_streams


class ProcessTests(unittest.TestCase):
    def run_program(self, code, **fields):
        with tempfile.TemporaryDirectory() as directory:
            step = {
                "name": "program",
                "argv": [sys.executable, "-c", code],
                "exit": 0,
                "stdout": "",
                "stderr": "",
            } | fields
            context = {"work": directory}
            process = Process(step, context, Path(directory), dict(os.environ))
            try:
                return process.finish(), step
            finally:
                process.cleanup()

    def test_normal_exit_is_distinct_from_signal(self):
        normal, _ = self.run_program("raise SystemExit(37)")
        signaled, _ = self.run_program("import os,signal; os.kill(os.getpid(),signal.SIGTERM)")
        self.assertEqual(normal["returncode"], 37)
        self.assertEqual(signaled["returncode"], -signal.SIGTERM)

    def test_raw_argv_and_stdin_are_preserved(self):
        code = "import os,sys; sys.stdout.buffer.write(os.fsencode(sys.argv[1])+sys.stdin.buffer.read())"
        result, step = self.run_program(
            code,
            argv=[sys.executable, "-c", code, {"hex": "ff"}],
            stdin={"hex": "00612062"},
            stdout={"hex": "ff00612062"},
        )
        process_expectations(step, result, {}, Path("."))

    def test_structured_prefix_does_not_consume_program_json(self):
        diagnostic = {
            "schema": 1,
            "kind": "diagnostic",
            "severity": "warning",
            "code": "W1",
            "message": "warning",
            "origin": {"kind": "source", "path": "main.scoop", "start": 1, "end": 2},
            "display": {"path": "/tmp/main.scoop"},
            "notes": [
                {
                    "message": "definition",
                    "origin": {"kind": "source", "path": "lib.scoop", "start": 3, "end": 4},
                    "display": None,
                }
            ],
        }
        tool = {"schema": 1, "kind": "executable", "output": "/tmp/program"}
        program = b'{"schema":1,"kind":"diagnostic","message":"program bytes"}\n'
        stderr = (json.dumps(diagnostic) + "\n" + json.dumps(tool) + "\n").encode() + program
        result = decode_streams(b"stdout", stderr, "stderr")
        self.assertEqual(result["stderr"], program)
        self.assertEqual(len(result["diagnostics"]), 1)
        self.assertNotIn("display", result["diagnostics"][0]["notes"][0])
        self.assertEqual(result["diagnostics"][0]["notes"][0]["origin"]["path"], "lib.scoop")
        result["returncode"] = 0
        with self.assertRaises(AssertionFailure):
            process_expectations(
                {
                    "name": "tool",
                    "exit": 0,
                    "stdout": "stdout",
                    "stderr": program,
                    "diagnostics": [],
                },
                result,
                {},
                Path("."),
            )

    def test_cleanup_reaps_a_running_process(self):
        with tempfile.TemporaryDirectory() as directory:
            step = {"name": "sleep", "argv": [sys.executable, "-c", "import time; time.sleep(30)"]}
            process = Process(step, {"work": directory}, Path(directory), dict(os.environ))
            process.cleanup()
            self.assertEqual(process.child.returncode, -signal.SIGKILL)
