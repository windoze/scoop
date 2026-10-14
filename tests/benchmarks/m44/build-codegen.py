"""Build existing allocation/write/poll programs with two fixed codegen toolsets."""

import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import time
from pathlib import Path

CASES = {
    "allocation": (40000 * 39999, 80000),
    "arrays": (2 * ((200000 - 1) // 4096 * 4096), 200000),
    "old-graph": ((200000 - 1) // 1024 * 1024, 200000),
    "primitive": (1 + 10000000 // 17 * sum(range(17)) + sum(range(10000000 % 17)), 10000000),
}


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", action="append", required=True, help="label=directory")
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--unwind-prefix", type=Path)
    args = parser.parse_args()
    toolsets = {}
    for item in args.tools:
        label, separator, directory = item.partition("=")
        if not separator or not label.isidentifier() or label in toolsets:
            parser.error("each toolset requires a distinct identifier=directory")
        toolsets[label] = Path(directory).resolve(strict=True)
    repo = Path(__file__).resolve().parents[3]
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=False)
    shutil.copytree(repo / "runtime", work / "runtime")
    shutil.copytree(repo / "sysroot", work / "sysroot")
    report = {
        "host": platform.platform(),
        "target": args.target,
        "profile": "release",
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "diff": subprocess.check_output(["git", "diff", "HEAD"], text=True),
        "gc_codegen_sources": {
            str(path.relative_to(repo)): path.read_text()
            for path in sorted((repo / "compiler/codegen/src/function/gc").rglob("*.rs"))
        },
        "llvm": subprocess.check_output(["llvm-config", "--version"], text=True).strip(),
        "cc": subprocess.check_output(["cc", "--version"], text=True).strip(),
        "tools": {},
        "cases": [],
    }
    for label, tools in toolsets.items():
        report["tools"][label] = {
            name: {"path": str(tools / name), "sha256": digest(tools / name)}
            for name in ("scoop", "scoopc", "scoop-link")
        }
        for name, (checksum, iterations) in CASES.items():
            source = repo / "tests/benchmarks" / f"{name}.scoop"
            output = work / label / name
            output.mkdir(parents=True)
            command = [
                tools / "scoop",
                "build",
                source,
                "--scoopc",
                tools / "scoopc",
                "--sysroot",
                work / "sysroot",
                "--runtime-root",
                work / "runtime",
                "--cache-dir",
                args.cache.resolve(),
                "--target",
                args.target,
                "--target-dir",
                output / "target",
                "--profile",
                "release",
                "--emit",
                "all",
                "--dump-dir",
                output / "dump",
                "--message-format",
                "json",
                "-o",
                output / "program",
            ]
            if args.unwind_prefix:
                command += ["--unwind-prefix", args.unwind_prefix.resolve()]
            start = time.perf_counter()
            result = subprocess.run(list(map(str, command)), capture_output=True, text=True)
            case = {
                "label": label,
                "name": name,
                "command": list(map(str, command)),
                "build_seconds": time.perf_counter() - start,
                "returncode": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
                "source": source.read_text(),
                "source_sha256": digest(source),
                "expected_stdout": f"{checksum}\n",
                "iterations": iterations,
                "binary": str(output / "program"),
                "dump": str(output / "dump"),
            }
            if result.returncode == 0:
                case.update(
                    binary_sha256=digest(output / "program"),
                    binary_bytes=(output / "program").stat().st_size,
                )
            report["cases"].append(case)
            (work / "build.json").write_text(json.dumps(report, indent=2) + "\n")
            result.check_returncode()
            print(f"{label}/{name}: built", flush=True)


if __name__ == "__main__":
    main()
