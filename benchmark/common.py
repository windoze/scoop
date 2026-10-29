"""Shared process measurement; raw build outputs stay in ignored logs/."""

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BUILD = ROOT / "build"
LOGS = ROOT / "logs"
JAVA_HOME = Path(
    os.environ.get(
        "BENCH_JAVA_HOME", "/Library/Java/JavaVirtualMachines/graalvm-21.jdk/Contents/Home"
    )
)
SCOOP = Path(os.environ.get("BENCH_SCOOP", str(Path.home() / ".scoop/bin/scoop")))
sys.path.insert(0, str(ROOT.parent / "tests/benchmarks"))
from process_metrics import run_process  # noqa: E402


def environment():
    env = dict(os.environ)
    for key in list(env):
        if key.startswith(("SCOOP_", "BENCH_")) or key in {
            "JAVA_TOOL_OPTIONS",
            "_JAVA_OPTIONS",
            "JDK_JAVA_OPTIONS",
            "GODEBUG",
            "GOFLAGS",
            "GOGC",
            "GOMEMLIMIT",
            "GOMAXPROCS",
        }:
            env.pop(key)
    env["LC_ALL"] = "C"
    env["PATH"] = str(SCOOP.parent) + os.pathsep + env["PATH"]
    return env


def commands():
    return {
        "scoop": [str(BUILD / "scoop-bench")],
        "go": [str(BUILD / "go-bench")],
        "java-jit": [
            str(JAVA_HOME / "bin/java"),
            "-Xms64m",
            "-Xmx1g",
            "-cp",
            str(BUILD / "java"),
            "Bench",
        ],
        "java-native": [str(BUILD / "java-native"), "-Xms64m", "-Xmx1g"],
    }


def execute(command, label, env=None):
    LOGS.mkdir(exist_ok=True)
    stdout = LOGS / f"{label}.stdout"
    stderr = LOGS / f"{label}.stderr"
    result = run_process(list(map(str, command)), env or environment(), stdout, stderr)
    return {
        "command": list(map(str, command)),
        **result,
        "stdout": stdout.read_text(),
        "stderr": stderr.read_text(),
    }


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def version(command):
    result = subprocess.run(command, capture_output=True, text=True, check=True)
    return (result.stdout + result.stderr).strip()
