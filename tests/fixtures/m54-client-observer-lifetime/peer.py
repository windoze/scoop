import subprocess
import sys


def cases():
    for where in ("start", "upload", "headers", "read", "complete"):
        for child in ("direct", "child"):
            for stop in ("caller", "client", "total"):
                yield ["cancel", where, stop, child, "cleanup"]
            yield ["cancel", where, "client", child, "clean"]
    for where in ("start", "connection", "request", "upload", "headers", "read", "complete"):
        for child in ("direct", "child"):
            yield ["self", where, child, "client"]
        for kind in ("eof", "timeout"):
            yield ["fail", where, kind]
    for where in ("read", "complete"):
        for child in ("direct", "child"):
            yield ["self", where, child, "response"]
    for where in ("request", "upload", "headers", "read", "complete"):
        yield ["h2", where]
    yield ["rethrow"]
    yield ["queue"]
    yield ["source"]


def main():
    for case in cases():
        result = subprocess.run([sys.argv[1], *case], capture_output=True, text=True, timeout=45)
        assert result.returncode == 0, (case, result.stdout, result.stderr)
        assert result.stdout == "observer lifecycle case passed\n", (case, result.stdout)
        assert not result.stderr, (case, result.stderr)
    print("client observer cancellation deadlines cleanup self-wait and HTTP2 isolation passed")


if __name__ == "__main__":
    main()
