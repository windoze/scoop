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
ALLOC_TYPED_CALL_RE = re.compile(r"\b(?:call|invoke)\b[^\n@]*@scoop_alloc_typed\b")


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
        call_count = len(ALLOC_TYPED_CALL_RE.findall(ir))
        symbol_occurrences = ir.count("scoop_alloc_typed")

        print(f"fixture={fixture.relative_to(REPO_ROOT)}")
        print(f"scoop_alloc_typed_calls={call_count}")
        print(f"scoop_alloc_typed_symbol_occurrences={symbol_occurrences}")

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
