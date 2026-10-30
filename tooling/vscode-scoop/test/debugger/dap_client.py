"""Small DAP process client for the real Scoop debugger acceptance tests."""

import contextlib
import json
import os
import queue
import subprocess
import sys
import threading
import time


class Client:
    def __init__(self, adapter, log, stderr):
        self.log = log
        self.stderr = stderr
        self.process = subprocess.Popen(
            [str(adapter), "--repl-mode=variable"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self.stderr,
        )
        self.sequence = 0
        self.pending = []
        self.incoming = queue.Queue()
        self.output = ""
        self.terminal = None
        self.master = None
        self.terminal_output = bytearray()
        self.terminal_reader = None
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        try:
            while True:
                headers = {}
                while True:
                    line = self.process.stdout.readline()
                    if not line:
                        raise EOFError("lldb-dap closed its output")
                    if line in (b"\r\n", b"\n"):
                        break
                    name, value = line.decode().split(":", 1)
                    headers[name.lower()] = value.strip()
                payload = self.process.stdout.read(int(headers["content-length"]))
                self.incoming.put(json.loads(payload))
        except Exception as error:
            self.incoming.put(error)

    def send(self, message):
        self.sequence += 1
        message = dict(message, seq=self.sequence)
        self.log.write(json.dumps({"send": message}, ensure_ascii=False) + "\n")
        self.log.flush()
        payload = json.dumps(message).encode()
        self.process.stdin.write(f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload)
        self.process.stdin.flush()
        return self.sequence

    def request(self, command, arguments=None, *, wait=True, timeout=30):
        sequence = self.send({"type": "request", "command": command, "arguments": arguments or {}})
        return self.response(sequence, timeout) if wait else sequence

    def response(self, sequence, timeout=30):
        response = self.wait(lambda item: item.get("request_seq") == sequence, timeout)
        assert response.get("success"), response
        return response.get("body", {})

    def event(self, name, timeout=30):
        return self.wait(lambda item: item.get("event") == name, timeout).get("body", {})

    def wait(self, predicate, timeout):
        deadline = time.monotonic() + timeout
        while True:
            for index, item in enumerate(self.pending):
                if predicate(item):
                    return self.pending.pop(index)
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"DAP timeout; output: {self.output[-2000:]}")
            try:
                item = self.incoming.get(timeout=remaining)
            except queue.Empty as error:
                raise TimeoutError(f"DAP timeout; output: {self.output[-2000:]}") from error
            if isinstance(item, Exception):
                raise item
            self.log.write(json.dumps({"receive": item}, ensure_ascii=False) + "\n")
            self.log.flush()
            if item.get("type") == "request":
                self.run_in_terminal(item)
            else:
                if item.get("event") == "output":
                    self.output += item.get("body", {}).get("output", "")
                self.pending.append(item)

    def run_in_terminal(self, request):
        assert request["command"] == "runInTerminal", request
        arguments = request["arguments"]
        self.master, slave = os.openpty()
        environment = dict(os.environ, TERM="xterm-256color")
        for name, value in (arguments.get("env") or {}).items():
            if value is None:
                environment.pop(name, None)
            else:
                environment[name] = value
        # Set up a real controlling terminal before exec; do not run Python in
        # preexec_fn after the adapter reader thread has started.
        launcher = (
            "import fcntl,os,sys,termios; "
            "fcntl.ioctl(0,termios.TIOCSCTTY,0); "
            "os.execv(sys.argv[1],sys.argv[1:])"
        )
        try:
            self.terminal = subprocess.Popen(
                [sys.executable, "-c", launcher, *arguments["args"]],
                cwd=arguments.get("cwd"),
                env=environment,
                stdin=slave,
                stdout=slave,
                stderr=slave,
                start_new_session=True,
            )
        finally:
            os.close(slave)
        self.terminal_reader = threading.Thread(target=self.read_terminal, daemon=True)
        self.terminal_reader.start()
        self.send(
            {
                "type": "response",
                "request_seq": request["seq"],
                "command": "runInTerminal",
                "success": True,
                "body": {"processId": self.terminal.pid},
            }
        )

    def read_terminal(self):
        # Drain while the process is alive; Darwin can discard unread PTY
        # output when the last slave closes.
        with contextlib.suppress(OSError):
            while chunk := os.read(self.master, 4096):
                self.terminal_output.extend(chunk)

    def close(self):
        if self.process.poll() is None:
            with contextlib.suppress(Exception, KeyboardInterrupt):
                self.request("disconnect", {"terminateDebuggee": True}, timeout=5)
        for process in (self.process, self.terminal):
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        if self.master is not None:
            os.close(self.master)
        if self.terminal_reader is not None:
            self.terminal_reader.join(timeout=5)
        self.process.stdin.close()
        self.process.stdout.close()
