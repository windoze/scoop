"""Windows process arguments, complete exit status, and descendant ownership."""

import json
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.model import AssertionFailure
from fixture_runner.process import Process


@unittest.skipUnless(os.name == "nt", "Windows process APIs")
class WindowsProcessTests(unittest.TestCase):
    def test_utf16_arguments_binary_streams_and_full_exit_code(self):
        arguments = ["雪 🧪", "", '"\\', "\ud800"]
        code = (
            "import ctypes,os,sys; "
            "os.write(1, b'|'.join(arg.encode('utf-16-le','surrogatepass') for arg in sys.argv[1:])); "
            "os.write(2, sys.stdin.buffer.read()); "
            "ctypes.windll.kernel32.ExitProcess(0xf1234567)"
        )
        with tempfile.TemporaryDirectory(prefix="fixture 雪 🧪 ") as directory:
            step = {
                "name": "unicode",
                "argv": [sys.executable, "-c", code, *arguments],
                "stdin": {"hex": "000a0d1aff"},
            }
            process = Process(step, {"work": directory}, Path(directory), dict(os.environ))
            try:
                result = process.finish()
            finally:
                process.cleanup()
            self.assertEqual(result["returncode"], 0xF1234567)
            self.assertEqual(
                result["stdout"],
                b"|".join(arg.encode("utf-16-le", "surrogatepass") for arg in arguments),
            )
            self.assertEqual(result["stderr"], bytes.fromhex("000a0d1aff"))

    def test_cleanup_terminates_an_immediately_spawned_descendant(self):
        from ctypes import wintypes

        from fixture_runner.windows_process import close_handle, function

        open_process = function(
            "OpenProcess", wintypes.HANDLE, wintypes.DWORD, wintypes.BOOL, wintypes.DWORD
        )
        wait = function("WaitForSingleObject", wintypes.DWORD, wintypes.HANDLE, wintypes.DWORD)
        with tempfile.TemporaryDirectory() as directory:
            ready = Path(directory) / "descendant.pid"
            code = (
                "import pathlib,subprocess,sys,time; "
                "child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); "
                f"pathlib.Path({json.dumps(str(ready))}).write_text(str(child.pid)); "
                "time.sleep(60)"
            )
            process = Process(
                {"name": "parent", "argv": [sys.executable, "-c", code]},
                {"work": directory},
                Path(directory),
                dict(os.environ),
            )
            handle = None
            try:
                deadline = time.monotonic() + 5
                while not ready.exists() and time.monotonic() < deadline:
                    time.sleep(0.01)
                self.assertTrue(ready.exists())
                handle = open_process(0x00100000, False, int(ready.read_text()))
                self.assertTrue(handle)
                self.assertEqual(wait(handle, 0), 258)  # WAIT_TIMEOUT
                process.cleanup()
                self.assertEqual(wait(handle, 5000), 0)
            finally:
                process.cleanup()
                if handle:
                    close_handle(handle)

    def test_timeout_reaps_the_process(self):
        with tempfile.TemporaryDirectory() as directory:
            process = Process(
                {
                    "name": "timeout",
                    "argv": [sys.executable, "-c", "import time; time.sleep(60)"],
                    "timeout": 0.1,
                },
                {"work": directory},
                Path(directory),
                dict(os.environ),
            )
            try:
                with self.assertRaisesRegex(AssertionFailure, "timed out"):
                    process.finish()
                self.assertIsNotNone(process.child.returncode)
            finally:
                process.cleanup()
