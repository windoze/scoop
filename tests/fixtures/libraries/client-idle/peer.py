import os
import socket
import subprocess
import sys
import threading
import traceback

from protocols import serve


def exercise(binary, mode):
    failures = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(4)
        listener.settimeout(90)

        def worker():
            try:
                with listener.accept()[0] as connection:
                    serve(connection, mode)
            except BaseException:
                failures.append(traceback.format_exc())

        workers = [threading.Thread(target=worker, daemon=True) for _ in range(2)]
        for thread in workers:
            thread.start()
        result = subprocess.run(
            [binary, mode, f"http://127.0.0.1:{listener.getsockname()[1]}/"],
            capture_output=True,
            text=True,
            timeout=120,
            env=os.environ,
        )
        for thread in workers:
            thread.join(60)
            assert not thread.is_alive(), (mode, "idle peer did not finish")
        assert result.returncode == 0, (
            mode,
            result.returncode,
            result.stdout,
            result.stderr,
            failures,
        )
        assert not failures, (mode, failures)
        assert result.stdout == f"client idle {mode}\n" and not result.stderr


for mode in sys.argv[2:] or ("h1-progress", "h1-gzip", "h2-isolation", "h2-write", "h2c"):
    exercise(sys.argv[1], mode)
print("independent client idle progress cleanup and stream isolation passed")
