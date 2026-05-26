#!/usr/bin/env python3
"""Audit active pipeline-gap inventory baselines."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Sequence


ACTIVE_AUDIT_ROOTS = ("crates/scoopc/src", "crates/scoop/src", "tests/fixtures")
CODEGEN_GAP_INVENTORY_PATH = (
    "crates/scoopc_codegen_llvm/src/llvm/codegen_gap_inventory.rs"
)
PIPELINE_GAPS_DOC_PATH = "docs/archive/designs/PIPELINE_GAPS.md"

LEGACY_BUCKET_MARKER = "LegacyOnly"
LEGACY_REASON_LEXICON = (
    "assign lhs missing local",
    "assign lhs lowering pending",
    "call callee lowering pending",
    "ctor call lowering pending",
    "sizeOf intrinsic requires value or type arg",
    "nameOf intrinsic requires type arg",
    "resume lowering requires canonical callee shape",
    "dispatch callee lowering pending",
)
EXPECTED_LEGACY_BUCKET_HITS: tuple[str, ...] = ()
EXPECTED_LEGACY_REASON_HITS: tuple[str, ...] = ()

EXIT_CONDITIONS = (
    "Open = 0",
    "default-mainline Partial = 0",
    "active code LegacyOnly = 0",
)
INVENTORY_CLASSIFICATION_RULES = (
    (
        "live contract",
        "Open inventory work and default-mainline Partial entries that still require an implementation closure.",
    ),
    (
        "downstream impossible-state guard",
        "Executable sentinels that may remain only as impossible-state guards after upstream handoff closes.",
    ),
    (
        "frontend reject",
        "Surfaces that must be rejected before MIR/codegen; backend unsupported is not an allowed substitute.",
    ),
    (
        "historical-only mapping",
        "Legacy residue or closed/re-scoped ids kept only in docs or dedicated regression tests, not in active executable inventories.",
    ),
)

CODEGEN_SCOPE_DRIFT_BASELINE = (
    "PIPELINE_GAPS §2.3",
    "PIPELINE_GAPS §3.1",
    "PIPELINE_GAPS §3.2",
    "PIPELINE_GAPS §3.3",
    "PIPELINE_GAPS §3.5",
    "PIPELINE_GAPS §3.6",
    "PIPELINE_GAPS §3.8",
    "PIPELINE_GAPS §3.9",
    "PIPELINE_GAPS §3.10",
    "PIPELINE_GAPS §3.11",
    "PIPELINE_GAPS §3.12",
    "PIPELINE_GAPS §3.13",
    "PIPELINE_GAPS §4.1",
    "PIPELINE_GAPS §4.3",
    "PIPELINE_GAPS §4.4",
    "PIPELINE_GAPS §4.5",
    "PIPELINE_GAPS §5.1",
    "PIPELINE_GAPS §5.3",
    "PIPELINE_GAPS §5.4",
    "PIPELINE_GAPS §7.6",
)
CODEGEN_SCOPE_SHIFTED_PARTIAL_GAPS: tuple[str, ...] = ()
CODEGEN_CLOSED_RESCOPED_PRODUCTION_BLOCKERS: tuple[str, ...] = ()
CHECK_COUNT = 5


class AuditError(Exception):
    """Raised when the pipeline gap audit cannot complete."""


@dataclass(frozen=True)
class CodegenGapEntry:
    gap_id: str
    production_blocker: bool


@dataclass(frozen=True)
class AuditReport:
    legacy_bucket_hits: int
    legacy_reason_hits: int
    scope_drift: int
    closed_rescoped_blockers: int
    failures: tuple[str, ...]

    def render(self) -> str:
        if self.failures:
            lines = [f"pipeline gap audit: failed ({len(self.failures)} violations)"]
            lines.extend(f"- {failure}" for failure in self.failures)
            return "\n".join(lines)
        return (
            "pipeline gap audit: ok "
            f"(roots={len(ACTIVE_AUDIT_ROOTS)}, "
            f"rules={len(INVENTORY_CLASSIFICATION_RULES)}, "
            f"exit_conditions={len(EXIT_CONDITIONS)}, "
            f"legacy_bucket_hits={self.legacy_bucket_hits}, "
            f"legacy_reason_hits={self.legacy_reason_hits}, "
            f"scope_drift={self.scope_drift}, "
            f"closed_rescoped_blockers={self.closed_rescoped_blockers}, "
            f"checks={CHECK_COUNT})"
        )

    def has_failures(self) -> bool:
        return bool(self.failures)


def main(argv: Sequence[str] | None = None) -> int:
    parse_args(argv)
    try:
        report = run()
    except AuditError as exc:
        print(f"pipeline gap audit: error: {exc}", file=sys.stderr)
        return 1

    print(report.render(), file=sys.stderr)
    return 1 if report.has_failures() else 0


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Audit active pipeline gap inventory baselines."
    )
    return parser.parse_args(argv)


def run() -> AuditReport:
    repo_root = Path(__file__).resolve().parent.parent
    failures: list[str] = []

    failures.extend(check_roots_rules_and_exit_conditions(repo_root))

    legacy_bucket_hits = collect_line_hits(
        repo_root, lambda line: LEGACY_BUCKET_MARKER in line
    )
    failures.extend(
        compare_lines(
            "LegacyOnly hits", legacy_bucket_hits, EXPECTED_LEGACY_BUCKET_HITS
        )
    )

    legacy_reason_hits = collect_line_hits(
        repo_root,
        lambda line: any(reason in line for reason in LEGACY_REASON_LEXICON),
    )
    failures.extend(
        compare_lines(
            "legacy reason hits", legacy_reason_hits, EXPECTED_LEGACY_REASON_HITS
        )
    )

    statuses = parse_pipeline_gap_statuses(repo_root)
    codegen_entries = parse_codegen_gap_inventory(repo_root)
    scope_drift_hits = codegen_scope_drift_hits(statuses, codegen_entries, failures)
    closed_blocker_hits = closed_rescoped_codegen_blockers(statuses, codegen_entries)
    failures.extend(
        compare_lines(
            "codegen scope drift baseline",
            scope_drift_hits,
            CODEGEN_SCOPE_DRIFT_BASELINE,
        )
    )
    failures.extend(
        compare_lines(
            "closed/re-scoped production blockers",
            closed_blocker_hits,
            CODEGEN_CLOSED_RESCOPED_PRODUCTION_BLOCKERS,
        )
    )

    return AuditReport(
        legacy_bucket_hits=len(legacy_bucket_hits),
        legacy_reason_hits=len(legacy_reason_hits),
        scope_drift=len(scope_drift_hits),
        closed_rescoped_blockers=len(closed_blocker_hits),
        failures=tuple(failures),
    )


def check_roots_rules_and_exit_conditions(repo_root: Path) -> list[str]:
    failures: list[str] = []
    for root in ACTIVE_AUDIT_ROOTS:
        if not (repo_root / root).is_dir():
            failures.append(f"audit root should exist: {root}")

    rule_names = tuple(rule[0] for rule in INVENTORY_CLASSIFICATION_RULES)
    expected_rule_names = (
        "live contract",
        "downstream impossible-state guard",
        "frontend reject",
        "historical-only mapping",
    )
    if rule_names != expected_rule_names:
        failures.append(
            "classification rule names drifted: "
            f"expected {format_lines(expected_rule_names)}, got {format_lines(rule_names)}"
        )

    expected_exit_conditions = (
        "Open = 0",
        "default-mainline Partial = 0",
        "active code LegacyOnly = 0",
    )
    if EXIT_CONDITIONS != expected_exit_conditions:
        failures.append(
            "exit conditions drifted: "
            f"expected {format_lines(expected_exit_conditions)}, got {format_lines(EXIT_CONDITIONS)}"
        )

    return failures


def collect_line_hits(repo_root: Path, matches: Callable[[str], bool]) -> list[str]:
    files: list[Path] = []
    for root in ACTIVE_AUDIT_ROOTS:
        collect_files(repo_root / root, files)
    files.sort()

    hits: list[str] = []
    for path in files:
        if path.name == "pipeline_gap_audit.rs":
            continue
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        relative = repo_relative_path(repo_root, path)
        for index, line in enumerate(source.splitlines(), start=1):
            if matches(line):
                hits.append(f"{relative}:{index}:{line}")
    return hits


def collect_files(dir_path: Path, files: list[Path]) -> None:
    try:
        entries = list(dir_path.iterdir())
    except OSError as exc:
        raise AuditError(f"failed to read {dir_path}: {exc}") from exc

    for path in entries:
        if path.is_dir():
            collect_files(path, files)
        elif path.is_file():
            files.append(path)


def parse_pipeline_gap_statuses(repo_root: Path) -> dict[str, str]:
    status_prefix = "- 状态：`"
    source = read_text(repo_root / PIPELINE_GAPS_DOC_PATH)
    statuses: dict[str, str] = {}
    current_gap: str | None = None

    for line in source.splitlines():
        gap_id = parse_gap_heading(line)
        if gap_id is not None:
            current_gap = gap_id
            continue
        stripped = line.strip()
        if not stripped.startswith(status_prefix) or current_gap is None:
            continue
        rest = stripped.removeprefix(status_prefix)
        end = rest.find("`")
        if end == -1:
            raise AuditError(f"unterminated status marker for {current_gap}")
        statuses[current_gap] = rest[:end]

    return statuses


def parse_gap_heading(line: str) -> str | None:
    trimmed = line.strip()
    rest = strip_any_prefix(trimmed, ("### ", "## "))
    if rest is None:
        return None
    parts = rest.split()
    if not parts:
        return None
    token = parts[0].removesuffix(".")
    if token and all(char.isascii() and (char.isdigit() or char == ".") for char in token):
        return f"PIPELINE_GAPS §{token}"
    return None


def parse_codegen_gap_inventory(repo_root: Path) -> list[CodegenGapEntry]:
    source = read_text(repo_root / CODEGEN_GAP_INVENTORY_PATH)
    start = source.find("pub const CODEGEN_GAP_INVENTORY")
    if start == -1:
        raise AuditError(f"{CODEGEN_GAP_INVENTORY_PATH} is missing CODEGEN_GAP_INVENTORY")
    end = source.find("];", start)
    if end == -1:
        raise AuditError(f"{CODEGEN_GAP_INVENTORY_PATH} has an unterminated inventory")
    inventory = source[start:end]

    pattern = re.compile(
        r'gap!\(\s*"(?P<gap_id>[^"]+)"\s*,'
        r'\s*"[^"]*"\s*,'
        r'\s*"[^"]*"\s*,'
        r"\s*[A-Za-z_][A-Za-z0-9_]*\s*,"
        r"\s*(?:true|false)\s*,"
        r"\s*(?P<production_blocker>true|false)\s*,"
        r'\s*"(?P<trigger>(?:[^"\\]|\\.)*)"\s*\)',
        re.DOTALL,
    )
    entries = [
        CodegenGapEntry(
            gap_id=match.group("gap_id"),
            production_blocker=match.group("production_blocker") == "true",
        )
        for match in pattern.finditer(inventory)
    ]
    if not entries:
        raise AuditError(f"{CODEGEN_GAP_INVENTORY_PATH} has no parsable gap entries")
    return entries


def codegen_scope_drift_hits(
    statuses: dict[str, str], entries: Sequence[CodegenGapEntry], failures: list[str]
) -> list[str]:
    shifted_partial_gaps = set(CODEGEN_SCOPE_SHIFTED_PARTIAL_GAPS)
    observed = [
        entry.gap_id
        for entry in entries
        if statuses.get(entry.gap_id) == "Closed/Re-scoped"
        or entry.gap_id in shifted_partial_gaps
    ]

    by_gap_id = {entry.gap_id: entry for entry in entries}
    for gap_id in CODEGEN_SCOPE_DRIFT_BASELINE:
        status = statuses.get(gap_id)
        if status is None:
            failures.append(f"missing documented status for {gap_id}")
            continue
        if gap_id not in by_gap_id:
            failures.append(f"missing codegen inventory entry for {gap_id}")
            continue
        if status not in ("Closed/Re-scoped", "Partial"):
            failures.append(
                f"{gap_id} should remain a non-open scope-drift baseline entry, "
                f"got status {status}"
            )

    return observed


def closed_rescoped_codegen_blockers(
    statuses: dict[str, str], entries: Sequence[CodegenGapEntry]
) -> list[str]:
    return [
        entry.gap_id
        for entry in entries
        if entry.production_blocker and statuses.get(entry.gap_id) == "Closed/Re-scoped"
    ]


def compare_lines(
    label: str, observed: Sequence[str], expected: Sequence[str]
) -> list[str]:
    if list(observed) == list(expected):
        return []
    return [
        f"{label} drifted: expected {format_lines(expected)}, got {format_lines(observed)}"
    ]


def format_lines(lines: Sequence[str]) -> str:
    if not lines:
        return "[]"
    return "[" + ", ".join(repr(line) for line in lines) + "]"


def strip_any_prefix(value: str, prefixes: Sequence[str]) -> str | None:
    for prefix in prefixes:
        if value.startswith(prefix):
            return value.removeprefix(prefix)
    return None


def repo_relative_path(repo_root: Path, path: Path) -> str:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError as exc:
        raise AuditError(f"{path} is outside repo root: {exc}") from exc


def read_text(path: Path) -> str:
    try:
        with path.open("r", encoding="utf-8", newline="") as file:
            return file.read()
    except OSError as exc:
        raise AuditError(f"failed to read {path}: {exc}") from exc


if __name__ == "__main__":
    raise SystemExit(main())
