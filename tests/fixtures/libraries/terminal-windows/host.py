"""Run console lifecycle checks in isolated, hidden Windows child consoles."""

import subprocess
import sys


def run(program, mode):
    startup = subprocess.STARTUPINFO()
    startup.dwFlags = subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = 0
    if mode == "unavailable":
        creation = subprocess.DETACHED_PROCESS
    elif mode == "pty-host":
        creation = subprocess.CREATE_NO_WINDOW
    else:
        creation = subprocess.CREATE_NEW_CONSOLE
    result = subprocess.run(
        [program, mode],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        startupinfo=startup,
        creationflags=creation,
        timeout=30,
        check=False,
    )
    expected = f"{mode}: passed\n".encode()
    if result.returncode != 0 or result.stdout != expected or result.stderr:
        raise AssertionError((mode, result.returncode, result.stdout, result.stderr))


for selected in ("session", "unavailable", "disconnect", "pty-host"):
    run(sys.argv[1], selected)
sys.stdout.buffer.write(b"windows terminal: passed\n")
