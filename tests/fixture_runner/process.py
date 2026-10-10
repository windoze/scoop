"""Run real tools, retain raw streams, and always reap owned processes."""

import json
import os
import signal
import subprocess
import tempfile
import time
from contextlib import ExitStack, suppress

from .assertions import signal_number
from .model import AssertionFailure, EnvironmentError, check_interrupted
from .values import argv_values, byte_value, expand, path


def canonical(diagnostic):
    return {
        key: [canonical(note) for note in value] if key == "notes" else value
        for key, value in diagnostic.items()
        if key not in {"schema", "kind", "display"}
    }


def decode_streams(stdout, stderr, mode):
    records, diagnostics, result = [], [], None
    raw_stderr = stderr
    if mode == "stdout":
        records = [json.loads(line) for line in stdout.splitlines()]
        result = records[-1] if records else None
    if mode == "stderr":
        lines = stderr.splitlines(keepends=True)
        count = 0
        for line in lines:
            try:
                record = json.loads(line)
            except (ValueError, UnicodeError):
                break
            if not isinstance(record, dict) or record.get("schema") != 1:
                break
            if record.get("kind") not in {
                "diagnostic",
                "library",
                "executable",
                "link",
            }:
                raise AssertionFailure(f"unknown Scoop JSON record: {record!r}")
            records.append(record)
            count += 1
            if record["kind"] == "diagnostic":
                diagnostics.append(canonical(record))
            else:
                result = record
                break
        stderr = b"".join(lines[count:])
    return {
        "stdout": stdout,
        "stderr": stderr,
        "raw_stderr": raw_stderr,
        "records": records,
        "diagnostics": diagnostics,
        "result": result,
    }


class Process:
    def __init__(self, step, context, base, environment):
        self.step, self.started = step, time.monotonic()
        env = environment | expand(step.get("env", {}), context)
        env = {key: str(value) for key, value in env.items()}
        argv = argv_values(step["argv"], context)
        try:
            with ExitStack() as stack:
                self.stdout = stack.enter_context(tempfile.TemporaryFile())
                self.stderr = stack.enter_context(tempfile.TemporaryFile())
                self.stdin = stack.enter_context(tempfile.TemporaryFile())
                self.stdin.write(byte_value(expand(step.get("stdin", ""), context), base))
                self.stdin.seek(0)
                self.child = subprocess.Popen(
                    argv,
                    cwd=path(expand(step.get("cwd", "${work}"), context)),
                    env=env,
                    stdin=self.stdin,
                    stdout=self.stdout,
                    stderr=self.stderr,
                    start_new_session=True,
                )
                self.files = stack.pop_all()
        except OSError as error:
            raise EnvironmentError(f"cannot start {argv!r}: {error}") from error

    def finish(self, interrupted=None):
        timeout = self.step.get("timeout", 300)
        while True:
            check_interrupted(interrupted)
            remaining = max(0.001, timeout - (time.monotonic() - self.started))
            try:
                code = self.child.wait(timeout=min(remaining, 0.1))
                break
            except subprocess.TimeoutExpired as error:
                if time.monotonic() - self.started >= timeout:
                    self.cleanup()
                    raise AssertionFailure(
                        f"{self.step['name']}: process timed out after {timeout}s"
                    ) from error
        self.stdout.seek(0)
        self.stderr.seek(0)
        result = decode_streams(self.stdout.read(), self.stderr.read(), self.step.get("json"))
        result.update(returncode=code, pid=self.child.pid)
        self.close_files()
        return result

    def send_signal(self, value):
        self.child.send_signal(signal_number(value))

    def close_files(self):
        self.files.close()

    def cleanup(self):
        # The session belongs only to this fixture process and its descendants.
        with suppress(ProcessLookupError):
            os.killpg(self.child.pid, signal.SIGKILL)
        self.child.wait()
        self.close_files()
