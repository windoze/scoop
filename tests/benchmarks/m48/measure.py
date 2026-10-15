"""Measure ordinary build commands, keeping source and tools present throughout."""

import argparse
import hashlib
import json
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

from cases import prepare


def process_tree(root):
    output = subprocess.check_output(["ps", "-axo", "pid=,ppid=,rss=,command="], text=True)
    rows = {}
    for line in output.splitlines():
        values = line.strip().split(None, 3)
        if len(values) == 4:
            pid, parent, rss, command = values
            rows[int(pid)] = (int(parent), int(rss), command)
    descendants = {root}
    while True:
        found = {pid for pid, row in rows.items() if row[0] in descendants}
        if found <= descendants:
            break
        descendants.update(found)
    return {pid: rows[pid] for pid in descendants if pid in rows}


def measure(command, log):
    seen = {}
    peak = 0
    start = time.perf_counter()
    with (
        log.with_suffix(".stdout").open("wb") as stdout,
        log.with_suffix(".stderr").open("wb") as stderr,
    ):
        process = subprocess.Popen(command, stdout=stdout, stderr=stderr)
        try:
            while process.poll() is None:
                rows = process_tree(process.pid)
                peak = max(peak, sum(row[1] for row in rows.values()))
                seen.update({pid: row[2] for pid, row in rows.items()})
                time.sleep(0.02)
            elapsed = time.perf_counter() - start
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait()
    text = log.with_suffix(".stderr").read_text()
    if process.returncode:
        raise RuntimeError(f"{command}: exit {process.returncode}\n{text[-6000:]}")
    result = next(
        item
        for line in text.splitlines()
        if (item := json.loads(line)).get("kind") in ("library", "executable")
    )
    return {
        "seconds": elapsed,
        "sampled_tree_peak_rss_kib": peak,
        "compiler_build_calls": len(result["observations"]["child_invocations"]),
        "observations": result["observations"],
        "sampled_processes": seen,
        "command": command,
        "result": result,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument(
        "--case", action="append", choices=["core", "std", "empty", "application", "native"]
    )
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error("rounds must be positive")
    repo = args.repo.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    tool = repo / "target/release/scoop"
    compiler = repo / "target/release/scoopc"
    report = {
        "revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=repo, text=True
        ).strip(),
        "host": platform.platform(),
        "scoop_profile": "debug",
        "tool_profile": "release",
        "tools_sha256": {
            str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in (tool, compiler)
        },
        "sampling": "ps descendant RSS sum; 20 ms sleep between polls; short-lived processes may be missed",
        "cold": "fresh Scoop build cache per pair; OS file cache is not flushed",
        "runs": [],
    }
    for case in args.case or ["core", "std", "empty", "application", "native"]:
        for round_number in range(args.rounds):
            with tempfile.TemporaryDirectory(prefix=f"scoop-m48-{case}-") as temporary:
                work = Path(temporary)
                root, expected, header = prepare(repo, work, case)
                command = [
                    str(tool),
                    "build",
                    str(root),
                    "--profile",
                    "debug",
                    "--sysroot",
                    str(repo / "sysroot"),
                    "--runtime-root",
                    str(repo / "runtime"),
                    "--scoopc",
                    str(compiler),
                    "--cache-dir",
                    str(work / "cache"),
                    "--target-dir",
                    str(work / "target"),
                    "--message-format",
                    "json",
                ]
                phases = ["cold", "warm"] + (["header-change"] if header else [])
                for phase in phases:
                    if phase == "header-change":
                        header.write_text(header.read_text() + "\n/* Changed build input. */\n")
                    log = args.output / f"{case}-{round_number}-{phase}"
                    sample = measure(command, log)
                    if expected is not None:
                        run = subprocess.run(
                            [sample["result"]["output"]], capture_output=True, text=True, check=True
                        )
                        if run.stdout != expected or run.stderr:
                            raise RuntimeError(f"incorrect {case} program output: {run}")
                    sample.update(case=case, round=round_number, phase=phase)
                    report["runs"].append(sample)
                    (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
                    print(
                        f"{case} {round_number + 1} {phase}: {sample['seconds']:.3f}s; compile={sample['compiler_build_calls']}",
                        flush=True,
                    )
    summary = {}
    for case, phase in sorted({(r["case"], r["phase"]) for r in report["runs"]}):
        runs = [r for r in report["runs"] if (r["case"], r["phase"]) == (case, phase)]
        summary[f"{case}/{phase}"] = {
            key: {"median": statistics.median(values), "min": min(values), "max": max(values)}
            for key in ("seconds", "sampled_tree_peak_rss_kib", "compiler_build_calls")
            for values in [[r[key] for r in runs]]
        }
    report["summary"] = summary
    (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
