"""Per-process measurements for the Darwin and Linux benchmark runners."""

import json
import os
import subprocess
import sys
import time


def run_process(command, environment, stdout_path, stderr_path):
    """Keep output off pipes and obtain resource usage from this child only."""
    if sys.platform not in ("darwin", "linux"):
        raise RuntimeError("process measurements require Darwin or Linux")
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        start = time.perf_counter_ns()
        with subprocess.Popen(command, env=environment, stdout=stdout, stderr=stderr) as child:
            try:
                _, status, usage = os.wait4(child.pid, 0)
                child.returncode = os.waitstatus_to_exitcode(status)
            except BaseException:
                child.kill()
                child.wait()
                raise
        elapsed = (time.perf_counter_ns() - start) / 1_000_000_000
    if child.returncode:
        raise RuntimeError(
            f"{command[0]}: exit {child.returncode}; output: {stdout_path}, {stderr_path}"
        )
    return {
        "wall_seconds": elapsed,
        "user_cpu_seconds": usage.ru_utime,
        "system_cpu_seconds": usage.ru_stime,
        "cpu_seconds": usage.ru_utime + usage.ru_stime,
        "peak_rss_bytes": usage.ru_maxrss * (1 if sys.platform == "darwin" else 1024),
    }


def gc_statistics(stderr, name):
    """Read the one runtime GC record alongside any ordinary diagnostic lines."""
    records = []
    for line in stderr.splitlines():
        try:
            record = json.loads(line)
        except ValueError:
            continue
        if isinstance(record, dict) and record.get("scoop_gc") == 1:
            records.append(record)
    if len(records) != 1:
        raise RuntimeError(f"{name}: expected one GC statistics record, found {len(records)}")
    metrics = records[0]
    try:
        count = metrics["minor_collections"] + metrics["full_collections"]
        consistent = sum(metrics["pause_buckets"]) == count
    except (KeyError, TypeError) as error:
        raise RuntimeError(f"{name}: malformed GC statistics") from error
    if not consistent:
        raise RuntimeError(f"{name}: inconsistent pause histogram")
    return metrics
