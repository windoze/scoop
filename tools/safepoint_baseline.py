#!/usr/bin/env python3
"""Generate the safepoint and gc-live roots baseline report."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


OPT_LEVELS = (0, 2)


class SafepointBaselineError(Exception):
    """Raised when the safepoint baseline report cannot be generated."""


@dataclass(frozen=True)
class Workload:
    name: str
    fixture_path: str
    intent: str


@dataclass(frozen=True)
class SafepointMetrics:
    statepoint_calls: int = 0
    rooted_statepoints: int = 0
    total_gc_live_roots: int = 0
    max_gc_live_roots: int = 0


@dataclass(frozen=True)
class WorkloadResult:
    workload_name: str
    fixture_path: str
    opt_level: int
    metrics: SafepointMetrics


WORKLOADS = (
    Workload(
        name="inline_wrapper_string",
        fixture_path="tests/fixtures/build/safepoint_inline_wrapper_string_basic.scoop",
        intent="summary-driven inlining + DirectCallOnly provenance 摊平后的普通调用边界",
    ),
    Workload(
        name="non_escaping_closure",
        fixture_path="tests/fixtures/build/safepoint_non_escaping_closure_basic.scoop",
        intent="non-escaping closure simplification 对局部 closure 调用边界的影响",
    ),
    Workload(
        name="root_pressure_loop",
        fixture_path="tests/fixtures/build/safepoint_root_pressure_loop_basic.scoop",
        intent="普通 loop 中多个 live String root 跨调用边界的局部 roots 压力",
    ),
)


def main(argv: Sequence[str] | None = None) -> int:
    parse_args(argv)
    try:
        report = run()
    except SafepointBaselineError as exc:
        print(f"safepoint baseline: error: {exc}", file=sys.stderr)
        return 1

    print(report, file=sys.stderr)
    return 0


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate the safepoint and gc-live roots baseline report."
    )
    return parser.parse_args(argv)


def run() -> str:
    workspace_root = Path(__file__).resolve().parent.parent
    target_dir = workspace_root / "target"
    try:
        target_dir.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        raise SafepointBaselineError(
            f"创建临时目录父目录失败：{target_dir}"
        ) from exc

    results: list[WorkloadResult] = []
    with tempfile.TemporaryDirectory(
        prefix=f"safepoint_baseline-{os.getpid()}-", dir=target_dir
    ) as temp_root_name:
        temp_root = Path(temp_root_name)
        for workload in WORKLOADS:
            for opt_level in OPT_LEVELS:
                output = temp_root / f"{workload.name}_O{opt_level}.ll"
                try:
                    build_fixture_ir(
                        workspace_root, workload.fixture_path, opt_level, output
                    )
                    ir = output.read_text(encoding="utf-8")
                except OSError as exc:
                    raise SafepointBaselineError(
                        f"读取 LLVM IR 失败：{output}"
                    ) from exc
                except SafepointBaselineError as exc:
                    raise SafepointBaselineError(
                        f"生成 workload `{workload.name}` 的 O{opt_level} LLVM IR 失败: {exc}"
                    ) from exc
                results.append(
                    WorkloadResult(
                        workload_name=workload.name,
                        fixture_path=workload.fixture_path,
                        opt_level=opt_level,
                        metrics=analyze_ir(ir),
                    )
                )

    return render_report(results)


def build_fixture_ir(
    workspace_root: Path, fixture_path: str, opt_level: int, output: Path
) -> None:
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "scoop",
        "--",
        "build",
        fixture_path,
        "--emit-llvm",
        "--opt-level",
        str(opt_level),
        "-o",
        str(output),
    ]
    try:
        result = subprocess.run(command, cwd=workspace_root, check=False)
    except OSError as exc:
        raise SafepointBaselineError(f"启动 cargo build 失败：{fixture_path}") from exc

    if result.returncode != 0:
        raise SafepointBaselineError(
            "`cargo run -p scoop -- build "
            f"{fixture_path} --emit-llvm --opt-level {opt_level}` 失败，"
            f"退出码 {format_exit_status(result.returncode)}"
        )


def analyze_ir(ir: str) -> SafepointMetrics:
    statepoint_calls = 0
    rooted_statepoints = 0
    total_gc_live_roots = 0
    max_gc_live_roots = 0

    for line in ir.splitlines():
        trimmed = line.lstrip()
        if trimmed.startswith("declare ") or "@llvm.experimental.gc.statepoint" not in trimmed:
            continue

        statepoint_calls += 1
        root_count = gc_live_root_count(trimmed)
        if root_count > 0:
            rooted_statepoints += 1
        total_gc_live_roots += root_count
        max_gc_live_roots = max(max_gc_live_roots, root_count)

    return SafepointMetrics(
        statepoint_calls=statepoint_calls,
        rooted_statepoints=rooted_statepoints,
        total_gc_live_roots=total_gc_live_roots,
        max_gc_live_roots=max_gc_live_roots,
    )


def gc_live_root_count(line: str) -> int:
    gc_live_prefix = '[ "gc-live"('
    start = line.find(gc_live_prefix)
    if start < 0:
        return 0

    roots = line[start + len(gc_live_prefix) :]
    end = roots.find(") ]")
    if end < 0:
        return 0

    return roots[:end].count("ptr addrspace(1)")


def render_report(results: Sequence[WorkloadResult]) -> str:
    out: list[str] = [
        "# Safepoint Baseline",
        "",
        "| workload | opt | statepoints | rooted statepoints | total gc-live roots | max gc-live roots | fixture |",
        "| --- | --- | ---: | ---: | ---: | ---: | --- |",
    ]

    for result in results:
        out.append(
            f"| `{result.workload_name}` | `O{result.opt_level}` | "
            f"{result.metrics.statepoint_calls} | "
            f"{result.metrics.rooted_statepoints} | "
            f"{result.metrics.total_gc_live_roots} | "
            f"{result.metrics.max_gc_live_roots} | `{result.fixture_path}` |"
        )

    out.extend(["", "## Workloads", ""])
    for workload in WORKLOADS:
        out.append(f"- `{workload.name}`: {workload.intent}（`{workload.fixture_path}`）")

    out.extend(["", "## O0 vs O2 Deltas", ""])
    for workload in WORKLOADS:
        o0 = find_result(results, workload.name, 0)
        o2 = find_result(results, workload.name, 2)
        out.append(
            f"- `{workload.name}`: "
            f"statepoints {o0.metrics.statepoint_calls} -> {o2.metrics.statepoint_calls} "
            f"({delta(o0.metrics.statepoint_calls, o2.metrics.statepoint_calls):+}), "
            f"rooted statepoints {o0.metrics.rooted_statepoints} -> {o2.metrics.rooted_statepoints} "
            f"({delta(o0.metrics.rooted_statepoints, o2.metrics.rooted_statepoints):+}), "
            f"total gc-live roots {o0.metrics.total_gc_live_roots} -> {o2.metrics.total_gc_live_roots} "
            f"({delta(o0.metrics.total_gc_live_roots, o2.metrics.total_gc_live_roots):+}), "
            f"max gc-live roots {o0.metrics.max_gc_live_roots} -> {o2.metrics.max_gc_live_roots} "
            f"({delta(o0.metrics.max_gc_live_roots, o2.metrics.max_gc_live_roots):+})"
        )

    out.extend(
        [
            "",
            "## Notes",
            "",
            "- `statepoints` 统计的是 LLVM IR 中实际发射的 `llvm.experimental.gc.statepoint` 调用点，不包含 declaration。",
            '- `gc-live roots` 统计的是每个 statepoint 上 `"gc-live"(...)` metadata 中的 `ptr addrspace(1)` 个数，用作当前 root-pressure 的最小可复验代理指标。',
            "- 默认 workload 同时覆盖：普通调用边界摊平、non-escaping closure 简化，以及 loop 中多个 live root 跨调用边界的局部 roots 压力。",
        ]
    )
    return "\n".join(out) + "\n"


def find_result(
    results: Sequence[WorkloadResult], workload_name: str, opt_level: int
) -> WorkloadResult:
    for result in results:
        if result.workload_name == workload_name and result.opt_level == opt_level:
            return result
    raise SafepointBaselineError(f"missing O{opt_level} result for {workload_name}")


def delta(before: int, after: int) -> int:
    return after - before


def format_exit_status(returncode: int) -> str:
    if returncode < 0:
        return f"signal: {-returncode}"
    return f"exit status: {returncode}"


if __name__ == "__main__":
    raise SystemExit(main())
