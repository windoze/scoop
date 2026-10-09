import argparse
import json
import os
import platform
import shutil
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("kind", choices=["standard", "interfaces", "poll"])
parser.add_argument("mode", choices=["off", "on"])
args = parser.parse_args()
repo = Path.cwd()
host = "linux-gnu" if platform.system() == "Linux" else "darwin"
target = "x86_64-unknown-linux-gnu" if host == "linux-gnu" else "aarch64-apple-darwin"
work = repo / f"tmp/m34/final-{args.kind}-{args.mode}-{host}"
work.mkdir(parents=True, exist_ok=False)
source = repo if args.mode == "on" else repo / "tmp/m34/baseline-source"
tools = repo / ("target/release" if args.mode == "on" else "tmp/m34/baseline-tools")
cache = repo / (
    f"tmp/m34/index-{host if host == 'darwin' else 'gnu'}/cache"
    if args.mode == "on"
    else f"tmp/m34/baseline-{host}-release" + ("-llvm22" if host == "darwin" else "") + "/cache"
)
if args.mode == "on" and host == "darwin" and (repo / "tmp/m34/acceptance-darwin/cache").is_dir():
    cache = repo / "tmp/m34/acceptance-darwin/cache"
revision = (
    os.environ.get("M34_COMPILER_REVISION")
    or subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
)
if args.mode == "off":
    revision = "e97b40f5c9c408e2ff785e5d09c6bf9222c78979"
report = {
    "host": platform.platform(),
    "target": target,
    "revision": revision,
    "mode": args.mode,
    "kind": args.kind,
    "builds": [],
    "commands": [],
}
report["tools"] = {
    name: subprocess.check_output(command, text=True).strip()
    for name, command in [("cc", ["cc", "--version"]), ("llvm", ["llvm-config", "--version"])]
}
runtime = source / "runtime"
if args.kind == "poll":
    runtime = work / "runtime"
    shutil.copytree(source / "runtime", runtime)
    header = runtime / "src/thread/testing.h"
    header.write_text("#define SCOOP_THREAD_TESTING\n" + header.read_text())


def run(command):
    started = time.perf_counter()
    result = subprocess.run(list(map(str, command)), capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"{command}: {result.returncode}\n{result.stderr}")
    return time.perf_counter() - started, result


base = [
    tools / "scoop",
    "build",
    "--scoopc",
    tools / "scoopc",
    "--sysroot",
    source / "sysroot",
    "--runtime-root",
    runtime,
    "--cache-dir",
    cache,
    "--profile",
    "release",
    "--message-format",
    "json",
    "--target",
    target,
]
if host == "linux-gnu":
    base += ["--unwind-prefix", repo / "sysroot/native" / target / "unwind"]


def build(name, path, output, extra=()):
    command = [*base, path, "--target-dir", work / f"target-{name}", "-o", output, *extra]
    first, result = run(command)
    records = [json.loads(line) for line in result.stderr.splitlines()]
    warm, _ = run(command)
    item = {
        "name": name,
        "first_build_seconds": first,
        "warm_build_seconds": warm,
        "command": list(map(str, command)),
        "records": records,
        "output_bytes": output.stat().st_size,
        "root_slib_bytes": Path(records[-1]["root"]["path"]).stat().st_size,
    }
    report["builds"].append(item)
    (work / "build.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"{args.mode} {name}: {first:.3f}s / {warm:.3f}s", flush=True)


if args.kind == "standard":
    assert args.mode == "on"
    for name in [
        "aggregate",
        "allocation",
        "arrays",
        "collections",
        "old-graph-large",
        "old-graph",
        "primitive",
        "survivors",
    ]:
        build(name, repo / f"tests/benchmarks/{name}.scoop", work / name)
elif args.kind == "interfaces":
    fixture = repo / "tests/benchmarks/interfaces"
    commands = [
        [
            "cc",
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-c",
            fixture / "phases.c",
            "-o",
            work / "phases.o",
        ],
        ["ar", "rcs", work / "libm34_interfaces.a", work / "phases.o"],
    ]
    for command in commands:
        run(command)
        report["commands"].append(list(map(str, command)))
    artifacts = work / "artifacts"
    build(
        "provider",
        fixture / "provider",
        artifacts / "dev.m34/interface-bench-provider/0.1.0/cone.slib",
        ["--emit", "all", "--dump-dir", work / "provider-dump"],
    )
    build(
        "consumer",
        fixture / "consumer",
        work / "interfaces",
        [
            "--cone-path",
            artifacts,
            "--library-path",
            work,
            "--emit",
            "all",
            "--dump-dir",
            work / "consumer-dump",
        ],
    )
else:
    for name in ["leaf", "runner"]:
        command = [
            "cc",
            "-std=c11",
            "-D_POSIX_C_SOURCE=200809L",
            "-O2",
            "-fno-omit-frame-pointer",
            "-fno-optimize-sibling-calls",
            "-fno-builtin",
            "-pthread",
            "-c",
            repo / f"tests/benchmarks/ffi/{name}.c",
            "-o",
            work / f"{name}.o",
        ]
        run(command)
        report["commands"].append(list(map(str, command)))
    command = ["ar", "rcs", work / "libffi_benchmark.a", work / "leaf.o", work / "runner.o"]
    run(command)
    report["commands"].append(list(map(str, command)))
    build(
        "poll",
        repo / "tests/benchmarks/poll-response/program.scoop",
        work / "poll",
        ["--library-path", work, "--emit", "all", "--dump-dir", work / "dump"],
    )
