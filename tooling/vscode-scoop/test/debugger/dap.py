"""Exercise a built Scoop program through the bundled lldb-dap (no rebuilds).

Use program.scoop for launch and console.scoop for terminal/attach. Raw protocol
logs belong outside the repository. Linux attach scopes ptrace permission to the
single adapter process; it does not change system security settings.
"""

import argparse
import json
import os
import select
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from dap_client import Client


def quoted(value):
    return '"' + str(value).replace("\\", "\\\\").replace('"', '\\"') + '"'


def lines(source):
    return {
        line.split("BREAK: ", 1)[1].strip(): index
        for index, line in enumerate(source.read_text().splitlines(), 1)
        if "BREAK: " in line
    }


def stopped(client):
    event = client.event("stopped")
    frames = client.request("stackTrace", {"threadId": event["threadId"]})["stackFrames"]
    assert frames, event
    return event["threadId"], frames


def variables(client, frame, globals_=False):
    scopes = client.request("scopes", {"frameId": frame["id"]})["scopes"]
    result = {}
    for scope in scopes:
        kind = "globals" if globals_ else "locals"
        if scope.get("presentationHint") == kind or scope["name"].lower() == kind:
            for variable in client.request(
                "variables", {"variablesReference": scope["variablesReference"]}
            )["variables"]:
                result[variable["name"]] = variable
    return result


def children(client, variable, **paging):
    return client.request(
        "variables", {"variablesReference": variable["variablesReference"], **paging}
    )["variables"]


def integer(variable, expected):
    assert variable["value"] == str(expected), variable


def configure(client, sequence, source, break_lines):
    client.event("initialized")
    result = client.request(
        "setBreakpoints",
        {
            "source": {"path": str(source)},
            "breakpoints": [{"line": line} for line in break_lines],
        },
    )["breakpoints"]
    assert len(result) == len(break_lines) and all(b["verified"] for b in result), result
    client.request("configurationDone")
    client.response(sequence)


def launch(client, configuration, source):
    markers = lines(source)
    sequence = client.request("launch", configuration, wait=False)
    configure(
        client, sequence, source, [markers["values"], markers["after-gc"], markers["capture"] + 1]
    )
    thread, frames = stopped(client)
    assert frames[0]["name"] == "inspectValues", frames
    assert Path(frames[0]["source"]["path"]).resolve() == source.resolve(), frames
    assert any(frame["name"] == "main" for frame in frames[1:]), frames
    values = variables(client, frames[0])
    integer(values["number"], 41)
    globals_ = variables(client, frames[0], globals_=True)
    integer(globals_["counter"], 40)
    integer(globals_["threadCounter"], 2)
    # DAP prepends the native pointer to a pointer type's summary.
    text = values["text"]["value"]
    assert '"' in text and json.loads(text[text.index('"') :]) == "hé\u0000llo", values["text"]
    assert "size=3" in values["values"]["value"], values["values"]
    middle = children(client, values["values"], start=1, count=1)
    assert len(middle) == 1 and middle[0]["name"] == "[1]", middle
    integer(middle[0], 11)
    box = {item["name"]: item for item in children(client, values["box"])}
    integer(box["value"], 20)
    assert "Box" in values["view"]["value"], values["view"]
    assert "Choice.Number" in values["tagged"]["value"], values["tagged"]
    variant = children(client, values["tagged"])[0]
    integer(children(client, variant)[0], 7)
    assert ".Some" in values["optional"]["value"], values["optional"]
    assert ".None" in values["absent"]["value"], values["absent"]
    client.request("next", {"threadId": thread})
    thread, frames = stopped(client)
    assert frames[0]["line"] > markers["values"], frames
    integer(variables(client, frames[0])["local"], 42)
    client.request("continue", {"threadId": thread})
    thread, frames = stopped(client)
    assert frames[0]["line"] == markers["after-gc"], frames
    # The existing compiler check proves that managed observations end at
    # safepoints. Do not turn an unavailable location into a cached old address.
    after = variables(client, frames[0])
    integer(after["local"], 42)
    client.request("continue", {"threadId": thread})
    thread, frames = stopped(client)
    assert frames[0]["line"] == markers["capture"] + 1, frames
    integer(variables(client, frames[0])["adjusted"], 41)
    client.request("continue", {"threadId": thread})
    assert client.event("exited")["exitCode"] == 0
    client.event("terminated")
    assert "debugging-ok" in client.output, client.output


def console(client, configuration, source, attach):
    child = None
    try:
        if attach:
            assert sys.platform == "linux", "attach acceptance uses Linux PR_SET_PTRACER"
            # The permission belongs to this one test process and its adapter.
            launcher = (
                "import ctypes,os,sys; "
                "assert ctypes.CDLL(None,use_errno=True).prctl(0x59616d61,int(sys.argv[1]),0,0,0)==0; "
                "os.execv(sys.argv[2],[sys.argv[2]])"
            )
            child = subprocess.Popen(
                [sys.executable, "-c", launcher, str(client.process.pid), configuration["program"]],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            ready, _, _ = select.select([child.stdout], [], [], 20)
            assert ready and child.stdout.readline().strip() == b"debugger-input", (
                "target not ready"
            )
            sequence = client.request("attach", dict(configuration, pid=child.pid), wait=False)
        else:
            sequence = client.request(
                "launch", dict(configuration, console="integratedTerminal"), wait=False
            )
        configure(client, sequence, source, [lines(source)["console"]])
        if child:
            child.stdin.write(b"A\n")
            child.stdin.flush()
        else:
            assert client.master is not None, "adapter did not request a terminal"
            os.write(client.master, b"A\n")
        thread, frames = stopped(client)
        assert frames[0]["name"] == "main", frames
        assert frames[0]["line"] == lines(source)["console"], frames
        integer(variables(client, frames[0])["input"], 65)
        client.request("continue", {"threadId": thread})
        assert client.event("exited")["exitCode"] == 0
        client.event("terminated")
        if child:
            output = child.communicate(timeout=10)[0].decode()
            assert "debugger-pipe" in output and "debugger-io-ok" in output, output
        else:
            output = b""
            while select.select([client.master], [], [], 0.2)[0]:
                try:
                    chunk = os.read(client.master, 4096)
                except OSError:
                    break
                if not chunk:
                    break
                output += chunk
            assert b"debugger-tty" in output and b"debugger-io-ok" in output, output
    finally:
        if child and child.poll() is None:
            child.kill()
            child.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--adapter", type=Path, required=True)
    parser.add_argument("--program", type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument(
        "--formatter",
        type=Path,
        default=Path(__file__).resolve().parents[2] / "debugger/scoop_lldb.py",
    )
    parser.add_argument(
        "--mode", choices=("initialize", "launch", "terminal", "attach"), default="launch"
    )
    parser.add_argument("--source-map", action="store_true")
    parser.add_argument("--log", type=Path, required=True)
    args = parser.parse_args()
    if args.mode != "initialize" and (not args.program or not args.source):
        parser.error("process tests require --program and --source")
    with (
        args.log.open("w", encoding="utf-8") as log,
        Path(str(args.log) + ".stderr").open("w", encoding="utf-8") as stderr,
    ):
        client = Client(args.adapter.resolve(), log, stderr)
        try:
            capabilities = client.request(
                "initialize",
                {
                    "adapterID": "scoop",
                    "clientID": "scoop-debugging-acceptance",
                    "linesStartAt1": True,
                    "columnsStartAt1": True,
                    "pathFormat": "path",
                    "supportsVariableType": True,
                    "supportsVariablePaging": True,
                    "supportsRunInTerminalRequest": True,
                },
            )
            assert capabilities["supportsConfigurationDoneRequest"], capabilities
            if args.mode != "initialize":
                configuration = {
                    "program": str(args.program.resolve()),
                    "cwd": str(args.program.resolve().parent),
                    "initCommands": ["command script import " + quoted(args.formatter.resolve())],
                    "enableSyntheticChildDebugging": True,
                    "disableASLR": False,
                    "env": {"SCOOP_GC_STRESS_MOVE": "1", "SCOOP_GC_STRESS_MINOR": "1"},
                }
                source = args.source.resolve()
                with tempfile.TemporaryDirectory(prefix="scoop debugger source ") as directory:
                    if args.source_map:
                        mapped = Path(directory) / source.name
                        shutil.copyfile(source, mapped)
                        configuration["sourceMap"] = [[str(source.parent), directory]]
                        source = mapped
                    if args.mode == "launch":
                        launch(client, configuration, source)
                    else:
                        console(client, configuration, source, args.mode == "attach")
            print(f"SCOOP_DAP_OK {args.mode}")
        finally:
            client.close()


if __name__ == "__main__":
    main()
