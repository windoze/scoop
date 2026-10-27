"""Actual process status, byte streams, diagnostic completeness, and cleanup."""

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

    def test_normal_exit_is_preserved(self):
        normal, _ = self.run_program("raise SystemExit(37)")
        self.assertEqual(normal["returncode"], 37)

    @unittest.skipUnless(os.name == "posix", "POSIX signal status")
    def test_signal_exit_is_distinct_from_normal_status(self):
        signaled, _ = self.run_program("import os,signal; os.kill(os.getpid(),signal.SIGTERM)")
        self.assertEqual(signaled["returncode"], -signal.SIGTERM)

    @unittest.skipUnless(os.name == "posix", "POSIX byte arguments")
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
            if os.name == "posix":
                self.assertEqual(process.child.returncode, -signal.SIGKILL)
            else:
                self.assertIsNotNone(process.child.returncode)

    def test_documentation_result_ends_the_structured_prefix(self):
        tool = {"schema": 1, "kind": "documentation", "output": "site/index.html"}
        stderr = (json.dumps(tool) + "\n").encode()
        result = decode_streams(b"", stderr, "stderr")
        self.assertEqual(result["result"], tool)
        self.assertEqual(result["stderr"], b"")
        self.assertEqual(result["diagnostics"], [])

    def test_check_result_has_no_artifact_and_ends_the_structured_prefix(self):
        tool = {"schema": 1, "kind": "check", "cache_hit": True}
        result = decode_streams(b"", (json.dumps(tool) + "\n").encode(), "stderr")
        self.assertEqual(result["result"], tool)
        self.assertEqual(result["stderr"], b"")
        self.assertEqual(result["diagnostics"], [])

    def test_format_result_preserves_differences_and_failed_status(self):
        tool = {"schema": 1, "kind": "format", "success": False, "mode": "check"}
        result = decode_streams(b"", (json.dumps(tool) + "\n").encode(), "stderr")
        self.assertEqual(result["result"], tool)
        self.assertEqual(result["stderr"], b"")
        self.assertEqual(result["diagnostics"], [])

    def test_clean_result_preserves_the_selected_directory_and_missing_state(self):
        tool = {"schema": 1, "kind": "clean", "target_dir": "project/target", "removed": False}
        result = decode_streams(b"", (json.dumps(tool) + "\n").encode(), "stderr")
        self.assertEqual(result["result"], tool)
        self.assertEqual(result["stderr"], b"")
        self.assertEqual(result["diagnostics"], [])

    def test_test_report_preserves_build_result_and_captured_json_bytes(self):
        build = {"schema": 1, "kind": "executable", "output": "program"}
        report = {
            "schema": 1,
            "kind": "test-report",
            "output": {"stderr": {"text": '{"schema":1,"kind":"diagnostic"}\n'}},
        }
        diagnostic = {"schema": 1, "kind": "diagnostic", "message": "incomplete test run"}
        raw = b"\n".join(json.dumps(record).encode() for record in [build, report, diagnostic])
        result = decode_streams(b"", raw, "test-stderr")
        self.assertEqual(result["result"], build)
        self.assertEqual(result["test_result"], report)
        self.assertEqual(result["stderr"], b"")
        self.assertEqual(result["diagnostics"], [{"message": "incomplete test run"}])
        self.assertEqual(result["records"], [build, report, diagnostic])

    def test_test_mode_handles_compile_errors_and_rejects_duplicate_reports(self):
        diagnostic = b'{"schema":1,"kind":"diagnostic","message":"compile failed"}\n'
        result = decode_streams(b"", diagnostic, "test-stderr")
        self.assertIsNone(result["result"])
        self.assertIsNone(result["test_result"])
        report = b'{"schema":1,"kind":"test-report"}\n'
        with self.assertRaisesRegex(AssertionFailure, "multiple test result"):
            decode_streams(b"", report * 2, "test-stderr")
