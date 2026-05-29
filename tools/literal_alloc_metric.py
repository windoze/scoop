#!/usr/bin/env python3
"""Measure `scoop_alloc_typed` calls emitted for literal-only IR fixtures."""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Sequence


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_FIXTURE = (
    REPO_ROOT
    / "tests"
    / "fixtures"
    / "umb_fix"
    / "P0-T03-gc-metrics"
    / "pos_literal_alloc_metric.scoop"
)
# codegen 现在把 typed 分配统一路由到内部 wrapper `@__scoop_alloc_typed_checked`
# （OOM 硬上限：分配失败时 fatal）。emit-artifact 默认 `--opt-level 0`，wrapper 不会被内联，
# 因此每个 codegen 分配点是一次 `call/invoke @__scoop_alloc_typed_checked`，而 wrapper 内部
# 再做一次 `call @scoop_alloc_typed`。两个符号要分别计数，否则会少计（只数 raw）或串味
# （子串把 `__scoop_alloc_typed_checked` 也算进 `scoop_alloc_typed`）。
#
# `@scoop_alloc_typed\b` 不会误匹配 `@__scoop_alloc_typed_checked`：后者紧跟 `@` 的是 `__`，
# 而正则在第一个 `@` 处要求紧跟 `scoop`。
ALLOC_TYPED_RAW_CALL_RE = re.compile(r"\b(?:call|invoke)\b[^\n@]*@scoop_alloc_typed\b")
ALLOC_TYPED_CHECKED_CALL_RE = re.compile(
    r"\b(?:call|invoke)\b[^\n@]*@__scoop_alloc_typed_checked\b"
)
CHECKED_SYMBOL = "__scoop_alloc_typed_checked"
RAW_SYMBOL = "scoop_alloc_typed"


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    fixture = args.fixture.expanduser().resolve()
    if not fixture.is_file():
        print(f"literal_alloc_metric: missing fixture: {fixture}", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory(prefix="scoop_literal_alloc_metric_") as tmp:
        ir_path = Path(tmp) / "literal_alloc_metric.ll"
        cmd = scoopc_command() + [
            "emit-artifact",
            "--kind",
            "llvm-ir",
            "--input",
            str(fixture),
            "--out",
            str(ir_path),
        ]
        if args.opt_level is not None:
            cmd.extend(["--opt-level", args.opt_level])

        output = subprocess.run(
            cmd,
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if output.returncode != 0:
            print(output.stdout, end="")
            print(output.stderr, end="", file=sys.stderr)
            return output.returncode

        ir = ir_path.read_text()
        raw_calls = len(ALLOC_TYPED_RAW_CALL_RE.findall(ir))
        checked_calls = len(ALLOC_TYPED_CHECKED_CALL_RE.findall(ir))
        # codegen 发出的 typed 分配点 = raw + checked 两种拼写的调用之和：O0 下为
        # checked(codegen 点) + raw(wrapper 内部一次)；wrapper 被内联时则全部退化为 raw。
        call_count = raw_calls + checked_calls

        # 子串总数把 `__scoop_alloc_typed_checked` 的每次出现也计入 `scoop_alloc_typed`，
        # 用 checked 计数回减即可得到 raw 符号自身的纯出现次数。
        checked_occurrences = ir.count(CHECKED_SYMBOL)
        raw_occurrences = ir.count(RAW_SYMBOL) - checked_occurrences
        symbol_occurrences = raw_occurrences + checked_occurrences

        try:
            fixture_label = fixture.relative_to(REPO_ROOT)
        except ValueError:
            fixture_label = fixture
        print(f"fixture={fixture_label}")
        print(f"scoop_alloc_typed_calls={call_count}")
        print(f"scoop_alloc_typed_raw_calls={raw_calls}")
        print(f"scoop_alloc_typed_checked_calls={checked_calls}")
        print(f"scoop_alloc_typed_symbol_occurrences={symbol_occurrences}")
        print(f"scoop_alloc_typed_raw_symbol_occurrences={raw_occurrences}")
        print(f"scoop_alloc_typed_checked_symbol_occurrences={checked_occurrences}")

        if args.expect_calls is not None and call_count != args.expect_calls:
            print(
                "literal_alloc_metric: expected "
                f"{args.expect_calls} calls, observed {call_count}",
                file=sys.stderr,
            )
            return 1
        if args.expect_min is not None and call_count < args.expect_min:
            print(
                "literal_alloc_metric: expected at least "
                f"{args.expect_min} calls, observed {call_count}",
                file=sys.stderr,
            )
            return 1

    return 0


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Count scoop_alloc_typed calls in a literal-allocation LLVM IR fixture."
    )
    parser.add_argument(
        "--fixture",
        type=Path,
        default=DEFAULT_FIXTURE,
        help=f"Scoop source fixture to emit (default: {DEFAULT_FIXTURE.relative_to(REPO_ROOT)})",
    )
    parser.add_argument("--opt-level", metavar="LEVEL", help="LLVM opt level passed to scoopc")
    parser.add_argument("--expect-calls", type=int, help="require an exact call count")
    parser.add_argument("--expect-min", type=int, help="require at least this many calls")
    return parser.parse_args(argv)


def scoopc_command() -> list[str]:
    env_value = os.environ.get("SCOOPC_BIN")
    if env_value:
        path = Path(env_value).expanduser().resolve()
        if path.is_file():
            return [str(path)]
        raise SystemExit(f"SCOOPC_BIN points to a missing executable: {path}")

    exe_name = "scoopc.exe" if os.name == "nt" else "scoopc"
    built = REPO_ROOT / "target" / "debug" / exe_name
    if built.is_file():
        return [str(built)]

    return ["cargo", "run", "--quiet", "-p", "scoopc", "--"]


if __name__ == "__main__":
    raise SystemExit(main())
