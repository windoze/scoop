"""Run published examples, using files returned by scoop example."""

import argparse
import contextlib
import json
import os
import signal
import socket
import subprocess
import time
from pathlib import Path

EXPECTED = {
    "start/hello": "Hello, Scoop!\n",
    "start/single-file": '{"name":"Ada","scores":[7,9]}\nAda: 9\nno note\n',
    "start/cone": "Hello, Ada!\nHello, Lin!\n",
    "language/overview": "point=(3, 4)\npoint missing\nAda\nguest\n3\n",
    "language/option": "Hello, Ada!\nHello, guest!\n3\n",
    "language/struct": "original=(3, 4)\nmoved=(8, 4)\ntrue\n",
    "language/enum": "Found: Ada\nNo matching name\n",
    "language/functions": "Hello, Scoop!\nHi, Ada.\nHello, Lin!\nHello, Lin!\n",
    "language/lambdas": "6\n0\n7\ncalls=1\n",
    "language/interfaces": "Ada\nScoop\n",
    "language/generics": "42\nScoop\nHello, Ada!\nHello, Scoop!\n",
    "language/visibility": "Hello, Ada!\n",
    "language/companion": "origin=(0, 0)\ndiagonal=(3, 3)\n",
    "language/errors": "count=3\nfinished\ncount must be positive\nfinished\n",
    "language/compiler-inputs": (
        "version=1.2.3\nHello, Scoop!\nbytes=14, first=72\n"
        "file=banner.txt\nfile=docs/help.txt\nhelp bytes=12\nprivate bytes=0\n"
    ),
    "language/imports": (
        "Hello, Ada!\nGood day, Ada.\nHello, Lin!\nGood day, Lin.\nHello, Lin!\nGood day, Grace.\n"
    ),
    "text/strings": "Hello, Ada!\n${name}\nscalars=2, bytes=4\nA|中|\n",
    "collections/list": "snapshot: Ada\nsnapshot: Lin\ncurrent first: Grace\nsizes=2,3\n",
    "collections/map": "red=2\nblue=1\nmissing=0\n",
    "io/resources": "stop writing\nclosed=true\nsaved before failure\ntemporary directory removed\n",
    "io/files": "Hello, Scoop!\nSecond line\nbytes=26\nnotes.txt\n",
    "json/typed": '{"name":"Ada","scores":[7,9]}\nAda: 9\nno note\n',
    "async/tasks": "42\nCancelled task cleaned up\n",
    "http/get": "200\nHello, Scoop!\n",
    "cli/options": "Hello, Ada!\nHello, Ada!\n",
    "test/assertions": None,
}


def run(command, cwd, environment, log, expected_code=0):
    result = subprocess.run(
        command, cwd=cwd, env=environment, capture_output=True, text=True, timeout=180
    )
    log.write_text(json.dumps(command, ensure_ascii=False) + "\n" + result.stdout + result.stderr)
    if result.returncode != expected_code:
        raise AssertionError(f"exit {result.returncode}, expected {expected_code}; see {log}")
    return result.stdout


def http_client(command, cwd, environment, logs):
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 8080))
    server_log = logs / "server.log"
    with server_log.open("w") as output:
        server = subprocess.Popen(
            command + ["run", "--release", "server.scoop"],
            cwd=cwd,
            env=environment,
            stdout=output,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            deadline = time.monotonic() + 180
            while True:
                if server.poll() is not None:
                    raise AssertionError(f"server exited; see {server_log}")
                # Redirected stdout may be buffered; observe the actual listening socket.
                try:
                    with socket.create_connection(("127.0.0.1", 8080), timeout=0.2):
                        break
                except OSError:
                    pass
                if time.monotonic() > deadline:
                    raise AssertionError(f"server did not start; see {server_log}")
                time.sleep(0.05)
            return run(
                command + ["run", "--release", "client.scoop", "--", "http://127.0.0.1:8080/"],
                cwd,
                environment,
                logs / "client.log",
            )
        finally:
            # This process group contains only the server launched for this example.
            with contextlib.suppress(ProcessLookupError):
                os.killpg(server.pid, signal.SIGTERM)
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(server.pid, signal.SIGKILL)
                server.wait(timeout=5)


def validate(topic, command, work, environment):
    id = topic["id"]
    directory = work / "programs" / id
    logs = work / "logs" / id
    logs.mkdir(parents=True, exist_ok=True)
    for file in topic["files"]:
        path = directory / file["path"]
        path.parent.mkdir(parents=True, exist_ok=True)
        if not path.exists() or path.read_text() != file["text"]:
            path.write_text(file["text"])
    temporary = directory / "temporary"
    temporary.mkdir(exist_ok=True)
    environment = {**environment, "TMPDIR": str(temporary) + "/"}
    if id == "http/get":
        stdout = http_client(command, directory, environment, logs)
    elif id == "language/compiler-inputs":
        executable = directory / "embedded-example"
        for label, value in (("present", "1.2.3"), ("missing", None), ("empty", "")):
            build_environment = environment.copy()
            if value is None:
                build_environment.pop("SCOOP_EXAMPLE_VERSION", None)
            else:
                build_environment["SCOOP_EXAMPLE_VERSION"] = value
            run(
                command + ["build", "--release", ".", "-o", str(executable)],
                directory,
                build_environment,
                logs / f"{label}-build.log",
            )
            stdout = run(
                [str(executable)],
                directory,
                {**environment, "SCOOP_EXAMPLE_VERSION": "runtime"},
                logs / f"{label}-run.log",
            )
            version = "dev" if value is None else value
            expected = EXPECTED[id].replace("version=1.2.3\n", f"version={version}\n", 1)
            assert stdout == expected, f"{id} ({label}): unexpected stdout {stdout!r}"
        return
    elif id == "test/assertions":
        stdout = run(
            command + ["test", "--release", "."], directory, environment, logs / "test.log"
        )
        listing = run(
            command + ["test", "--release", "--list", "."],
            directory,
            environment,
            logs / "list.log",
        )
        assert "division returns a value" in listing and "zero count reports the reason" in listing
        filtered = run(
            command + ["test", "--release", "--filter", "zero count", "."],
            directory,
            environment,
            logs / "filter.log",
        )
        assert "zero count reports the reason" in filtered
        assert "division returns a value" not in filtered
        return
    else:
        if id == "language/visibility":
            directory = directory / "app"
            source = "."
        elif id in ("start/cone", "language/imports"):
            source = "."
        else:
            source = topic["files"][0]["path"]
        args = ["run", "--release", source]
        if id == "cli/options":
            args += ["--", "Ada", "--count", "2", "--excited"]
        stdout = run(command + args, directory, environment, logs / "run.log")
    assert stdout == EXPECTED[id], f"{id}: unexpected stdout {stdout!r}"
    if id == "start/hello":
        run(command + ["check", "main.scoop"], directory, environment, logs / "check.log")
    if id == "cli/options":
        help_text = run(
            command + ["run", "--release", "greet.scoop", "--", "--help"],
            directory,
            environment,
            logs / "help.log",
        )
        assert all(name in help_text for name in ("Name to greet", "--count", "--excited"))
        invalid = run(
            command + ["run", "--release", "greet.scoop", "--", "Ada", "--count", "0"],
            directory,
            environment,
            logs / "invalid.log",
            2,
        )
        assert invalid == "count must be positive\n"
    if id.startswith("io/"):
        assert not list(temporary.glob("scoop-*")), "Example leaked its temporary directory"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scoop", type=Path, required=True)
    parser.add_argument("--sysroot", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--topics", nargs="+")
    args = parser.parse_args()
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    command = [str(args.scoop.resolve()), "--sysroot", str(args.sysroot.resolve())]
    environment = os.environ.copy()
    # All examples consume one installation and share its ordinary Cone cache.
    environment["SCOOP_CACHE_DIR"] = str(work / "cache")
    index = json.loads(
        run(
            command + ["example", "--list", "--format", "json"],
            work,
            environment,
            work / "index.log",
        )
    )
    ids = [topic["id"] for topic in index["topics"]]
    assert set(ids) == set(EXPECTED), "Update the explicit example checks when publishing topics"
    if args.topics:
        assert set(args.topics) <= set(ids), "Unknown topic selection"
        ids = args.topics
    failures = []
    for id in ids:
        try:
            topic = json.loads(
                run(
                    command + ["example", id, "--format", "json"],
                    work,
                    environment,
                    work / "topic.log",
                )
            )
            validate(topic, command, work, environment)
            print(f"PASS {id}", flush=True)
        except (AssertionError, OSError, subprocess.SubprocessError) as error:
            failures.append(id)
            print(f"FAIL {id}: {error}", flush=True)
    print(f"{len(ids) - len(failures)}/{len(ids)} topics passed; logs: {work / 'logs'}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
