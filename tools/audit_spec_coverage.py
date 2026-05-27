#!/usr/bin/env python3
"""Audit the UMB fixture spec coverage inventory and matrix."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


UMB_FIX_ROOT = "tests/fixtures/umb_fix"
FIXTURE_INDEX_PATH = "tests/fixtures/umb_fix/_index.csv"
SPEC_COVERAGE_MATRIX_PATH = "audit/spec_coverage_matrix.md"
B01_README_PATH = "tests/fixtures/umb_fix/B-01-builder-invariant/_README.md"
ARCHIVED_LEDGER_PATH = "docs/archive/audits/unsupported-main-body/UMB_retired.csv"
EXPECTED_FIXTURE_INDEX_HEADER = (
    "fixture_path,bucket,kind,spec_anchor,umb_ids,status,notes"
)
HEADER_SCAN_LINES = 32
FORBIDDEN_NEGATIVE_TERMS = (
    "后端",
    "backend",
    "LLVM",
    "codegen",
    "UnsupportedMainBody",
)
XFAIL_DIRECTIVES = ("XFAIL", "EXPECT-FAIL", "EXPECT-FAILURE")

VALID_BUCKETS = (
    "B-01",
    "B-02",
    "B-03",
    "B-04",
    "B-05",
    "B-06",
    "B-07",
    "B-08",
    "B-09",
    "B-10",
    "B-11",
    "B-12",
    "B-13",
    "B-14",
    "B-15",
    "B-16",
    "B-17",
    "B-18",
    "B-19",
    "B-20",
    "B-21",
    "B-22",
    "B-23",
    "B-25",
    "B-26",
    "B-27",
    "B-28",
    "B-29",
    "B-30",
    "B-31",
    "B-32",
    "B-33",
    "B-34",
    "B-35",
    "B-36",
)


class AuditError(Exception):
    """Raised when the spec coverage audit cannot complete."""


@dataclass(frozen=True)
class FixtureIndexEntry:
    fixture_path: str
    bucket: str
    kind: str
    spec_anchor: str
    umb_ids: str
    status: str
    notes: str

    @classmethod
    def from_record(cls, line_number: int, record: Sequence[str]) -> "FixtureIndexEntry":
        expected_fields = len(EXPECTED_FIXTURE_INDEX_HEADER.split(","))
        if len(record) != expected_fields:
            raise AuditError(
                f"{FIXTURE_INDEX_PATH}:{line_number} has the wrong field count"
            )
        return cls(
            fixture_path=record[0],
            bucket=record[1],
            kind=record[2],
            spec_anchor=record[3],
            umb_ids=record[4],
            status=record[5],
            notes=record[6],
        )


@dataclass(frozen=True)
class FixtureHeaders:
    directives: dict[str, list[str]]
    ignore_until_fix: str | None

    @classmethod
    def from_fixture(cls, repo_root: Path, fixture_path: str) -> "FixtureHeaders":
        content = read_text(repo_root / fixture_path)
        directives: dict[str, list[str]] = {}
        ignore_until_fix: str | None = None

        for line in content.splitlines()[:HEADER_SCAN_LINES]:
            trimmed = line.lstrip()
            if not trimmed.startswith("//"):
                continue
            comment = trimmed.removeprefix("//").strip()
            if ":" not in comment:
                continue
            raw_key, raw_value = comment.split(":", 1)
            key = raw_key.strip().upper()
            value = raw_value.strip()
            if key == "IGNORE-UNTIL-FIX":
                ignore_until_fix = value
            else:
                directives.setdefault(key, []).append(value)

        return cls(directives=directives, ignore_until_fix=ignore_until_fix)

    def directive_values(self, key: str) -> list[str]:
        return self.directives.get(key.upper(), [])

    def require_single(
        self, key: str, context: str, failures: list[str]
    ) -> str | None:
        values = self.directive_values(key)
        if len(values) != 1:
            failures.append(f"{context} must contain exactly one {key} header")
            return None
        return values[0]


@dataclass(frozen=True)
class AuditReport:
    fixture_count: int
    bucket_count: int
    failures: tuple[str, ...]

    def render(self) -> str:
        if self.failures:
            lines = [f"spec coverage audit: failed ({len(self.failures)} violations)"]
            lines.extend(f"- {failure}" for failure in self.failures)
            return "\n".join(lines)
        return (
            "spec coverage audit: ok "
            f"(fixtures={self.fixture_count}, buckets={self.bucket_count}, checks=7)"
        )

    def has_failures(self) -> bool:
        return bool(self.failures)


def main(argv: Sequence[str] | None = None) -> int:
    parse_args(argv)
    try:
        report = run()
    except AuditError as exc:
        print(f"spec coverage audit: error: {exc}", file=sys.stderr)
        return 1

    print(report.render(), file=sys.stderr)
    return 1 if report.has_failures() else 0


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Audit UMB fixture spec coverage inventory invariants."
    )
    return parser.parse_args(argv)


def run() -> AuditReport:
    repo_root = Path(__file__).resolve().parent.parent
    entries = fixture_index_entries(repo_root)
    failures: list[str] = []

    failures.extend(check_fixture_index_in_sync(repo_root, entries))
    failures.extend(check_all_fixture_rows_are_active(repo_root, entries))
    failures.extend(check_index_no_longer_tracks_live_umb_ids(repo_root, entries))
    failures.extend(check_every_bucket_has_at_least_one_pos_and_one_neg(repo_root, entries))
    failures.extend(check_builder_invariant_readme_records_archive(repo_root))
    failures.extend(check_spec_coverage_matrix_in_sync(repo_root, entries))
    failures.extend(check_no_forbidden_terms_in_neg_messages(repo_root, entries))

    return AuditReport(
        fixture_count=len(entries),
        bucket_count=len(VALID_BUCKETS),
        failures=tuple(failures),
    )


def check_fixture_index_in_sync(
    repo_root: Path, entries: Sequence[FixtureIndexEntry]
) -> list[str]:
    failures: list[str] = []
    indexed_paths = {entry.fixture_path for entry in entries}
    actual_paths = actual_fixture_paths(repo_root)

    missing_from_index = sorted(actual_paths - indexed_paths)
    if missing_from_index:
        failures.append(
            f"fixtures missing from {FIXTURE_INDEX_PATH}: "
            + ", ".join(missing_from_index)
        )

    missing_fixtures = sorted(indexed_paths - actual_paths)
    if missing_fixtures:
        failures.append(
            f"{FIXTURE_INDEX_PATH} rows pointing at missing fixtures: "
            + ", ".join(missing_fixtures)
        )

    for entry in entries:
        failures.extend(validate_fixture_index_entry(repo_root, entry))

    return failures


def validate_fixture_index_entry(repo_root: Path, entry: FixtureIndexEntry) -> list[str]:
    failures: list[str] = []
    if not (
        entry.fixture_path.startswith(UMB_FIX_ROOT)
        and entry.fixture_path.endswith(".scoop")
    ):
        failures.append(f"{entry.fixture_path} is not an umb_fix .scoop path")
    if entry.bucket not in VALID_BUCKETS:
        failures.append(f"{entry.fixture_path} has invalid bucket `{entry.bucket}`")
    if entry.kind not in ("positive", "negative"):
        failures.append(f"{entry.fixture_path} has invalid fixture kind `{entry.kind}`")
    if not entry.spec_anchor.strip():
        failures.append(
            f"{entry.fixture_path} has an empty spec_anchor in {FIXTURE_INDEX_PATH}"
        )
    if not entry.notes.strip():
        failures.append(f"{entry.fixture_path} has empty notes in {FIXTURE_INDEX_PATH}")
    if entry.status != "active":
        failures.append(
            f"{entry.fixture_path} has non-active status `{entry.status}` in "
            f"{FIXTURE_INDEX_PATH}"
        )

    headers = FixtureHeaders.from_fixture(repo_root, entry.fixture_path)
    if headers.ignore_until_fix is not None:
        failures.append(f"{entry.fixture_path} still contains IGNORE-UNTIL-FIX after P8")

    required: dict[str, str | None] = {}
    for key in ("EXPECT", "SPEC", "COVERS", "BUCKETS"):
        required[key] = headers.require_single(key, entry.fixture_path, failures)

    if entry.kind == "negative":
        for key in ("EXPECT-ERROR-CODE", "EXPECT-ERROR-AT", "EXPECT-ERROR", "REASON"):
            headers.require_single(key, entry.fixture_path, failures)

    spec = required["SPEC"]
    if spec is not None and spec != entry.spec_anchor:
        failures.append(f"{entry.fixture_path} SPEC header drifted from {FIXTURE_INDEX_PATH}")

    covers = required["COVERS"]
    if covers is not None:
        header_ids = parse_umb_id_list(covers, entry.fixture_path, failures)
        index_ids = parse_umb_id_list(entry.umb_ids, entry.fixture_path, failures)
        if header_ids != index_ids:
            failures.append(
                f"{entry.fixture_path} COVERS header drifted from {FIXTURE_INDEX_PATH}"
            )

    buckets = required["BUCKETS"]
    if buckets is not None:
        for bucket in parse_bucket_list(buckets):
            if bucket not in VALID_BUCKETS:
                failures.append(
                    f"{entry.fixture_path} BUCKETS header contains invalid bucket `{bucket}`"
                )

    return failures


def check_all_fixture_rows_are_active(
    repo_root: Path, entries: Sequence[FixtureIndexEntry]
) -> list[str]:
    failures: list[str] = []
    ignored: list[str] = []
    xfailed: list[str] = []

    for entry in entries:
        if entry.status != "active":
            failures.append(f"{entry.fixture_path} must remain active after P8")

        headers = FixtureHeaders.from_fixture(repo_root, entry.fixture_path)
        if headers.ignore_until_fix is not None:
            ignored.append(f"{entry.fixture_path} ({headers.ignore_until_fix})")
        for directive in XFAIL_DIRECTIVES:
            if headers.directive_values(directive):
                xfailed.append(f"{entry.fixture_path} ({directive})")

    if ignored:
        failures.append(
            "umb_fix fixtures must not use IGNORE-UNTIL-FIX after P8: "
            + ", ".join(ignored)
        )
    if xfailed:
        failures.append(
            "umb_fix fixtures must not use xfail directives after P8: "
            + ", ".join(xfailed)
        )

    return failures


def check_index_no_longer_tracks_live_umb_ids(
    repo_root: Path, entries: Sequence[FixtureIndexEntry]
) -> list[str]:
    failures: list[str] = []

    for entry in entries:
        if parse_umb_id_list(entry.umb_ids, entry.fixture_path, failures):
            failures.append(
                f"{entry.fixture_path} must use COVERS: NONE after the UMB ledger is archived"
            )

        headers = FixtureHeaders.from_fixture(repo_root, entry.fixture_path)
        covers = headers.require_single("COVERS", entry.fixture_path, failures)
        if covers is not None and parse_umb_id_list(covers, entry.fixture_path, failures):
            failures.append(
                f"{entry.fixture_path} must not retain live UMB id coverage after P8"
            )

    return failures


def check_every_bucket_has_at_least_one_pos_and_one_neg(
    repo_root: Path, entries: Sequence[FixtureIndexEntry]
) -> list[str]:
    failures: list[str] = []
    by_bucket_kind: dict[str, dict[str, int]] = {}
    for entry in entries:
        by_bucket_kind.setdefault(entry.bucket, {}).setdefault(entry.kind, 0)
        by_bucket_kind[entry.bucket][entry.kind] += 1

    for bucket in VALID_BUCKETS:
        if bucket == "B-01":
            expected = f"ARCHIVED: {ARCHIVED_LEDGER_PATH}"
            actual = sentinel_marker_value(repo_root, "SENTINEL-COVERS")
            if actual != expected:
                failures.append(
                    "B-01 sentinel coverage must point at the archived retired ledger"
                )
            continue

        kinds = by_bucket_kind.get(bucket)
        if kinds is None:
            failures.append(f"{bucket} has no rows in {FIXTURE_INDEX_PATH}")
            continue
        if kinds.get("positive", 0) <= 0:
            failures.append(f"{bucket} must have at least one positive umb_fix fixture")
        if kinds.get("negative", 0) <= 0:
            failures.append(f"{bucket} must have at least one negative umb_fix fixture")

    return failures


def check_builder_invariant_readme_records_archive(repo_root: Path) -> list[str]:
    failures: list[str] = []
    sentinel = sentinel_marker_value(repo_root, "SENTINEL")
    expected_test_ref = (
        "tools/audit_spec_coverage.py::"
        "check_builder_invariant_readme_records_archive"
    )
    if expected_test_ref not in sentinel:
        failures.append(f"B-01 README should point at this sentinel test, got `{sentinel}`")

    sentinel_status = sentinel_marker_value(repo_root, "SENTINEL-STATUS")
    if sentinel_status != "archived-by-P8-T02":
        failures.append("B-01 sentinel coverage should be archived after P8-T02")

    sentinel_covers = sentinel_marker_value(repo_root, "SENTINEL-COVERS")
    if sentinel_covers != f"ARCHIVED: {ARCHIVED_LEDGER_PATH}":
        failures.append("B-01 retired ids must be discoverable in the archived ledger")

    if not (repo_root / ARCHIVED_LEDGER_PATH).is_file():
        failures.append(f"B-01 archived ledger path is missing: {ARCHIVED_LEDGER_PATH}")

    return failures


def check_spec_coverage_matrix_in_sync(
    repo_root: Path, entries: Sequence[FixtureIndexEntry]
) -> list[str]:
    failures: list[str] = []
    matrix = read_text(repo_root / SPEC_COVERAGE_MATRIX_PATH)
    if "(planned)" in matrix:
        failures.append(f"{SPEC_COVERAGE_MATRIX_PATH} still contains planned fixture references")

    indexed_paths = {entry.fixture_path for entry in entries}
    for path in backticked_paths_with_prefix(matrix, UMB_FIX_ROOT):
        if not (repo_root / path).is_file():
            failures.append(
                f"{SPEC_COVERAGE_MATRIX_PATH} references missing fixture `{path}`"
            )
        if path not in indexed_paths:
            failures.append(
                f"{SPEC_COVERAGE_MATRIX_PATH} references `{path}` but it is absent from "
                f"{FIXTURE_INDEX_PATH}"
            )

    for bucket_link in matrix_bucket_links(matrix):
        if not (repo_root / "audit" / bucket_link).is_file():
            failures.append(
                f"{SPEC_COVERAGE_MATRIX_PATH} references missing bucket doc `{bucket_link}`"
            )

    return failures


def check_no_forbidden_terms_in_neg_messages(
    repo_root: Path, entries: Sequence[FixtureIndexEntry]
) -> list[str]:
    failures: list[str] = []

    for entry in entries:
        if entry.kind != "negative":
            continue

        headers = FixtureHeaders.from_fixture(repo_root, entry.fixture_path)
        messages = headers.directive_values("EXPECT-ERROR")
        if not messages:
            failures.append(f"{entry.fixture_path} is negative but has no EXPECT-ERROR header")

        for message in messages:
            for term in FORBIDDEN_NEGATIVE_TERMS:
                if contains_forbidden_term(message, term):
                    failures.append(
                        f"{entry.fixture_path} EXPECT-ERROR contains forbidden term "
                        f"`{term}`: {message}"
                    )

    return failures


def sentinel_marker_value(repo_root: Path, marker: str) -> str:
    path = repo_root / B01_README_PATH
    content = read_text(path)
    prefix = f"- {marker}:"
    for line in content.splitlines():
        trimmed = line.lstrip()
        if trimmed.startswith(prefix):
            return trimmed.removeprefix(prefix).strip()
    raise AuditError(f"{path} is missing `{prefix}`")


def fixture_index_entries(repo_root: Path) -> list[FixtureIndexEntry]:
    csv = read_text(repo_root / FIXTURE_INDEX_PATH)
    records = parse_csv_records(csv, FIXTURE_INDEX_PATH)
    if not records:
        raise AuditError(f"{FIXTURE_INDEX_PATH} is empty")

    header = ",".join(records.pop(0))
    if header != EXPECTED_FIXTURE_INDEX_HEADER:
        raise AuditError(f"{FIXTURE_INDEX_PATH} header mismatch")

    return [
        FixtureIndexEntry.from_record(index + 2, record)
        for index, record in enumerate(records)
    ]


def actual_fixture_paths(repo_root: Path) -> set[str]:
    root = repo_root / UMB_FIX_ROOT
    paths: list[Path] = []
    collect_scoop_files(root, paths)
    return {repo_relative_path(repo_root, path) for path in paths}


def collect_scoop_files(dir_path: Path, paths: list[Path]) -> None:
    try:
        entries = sorted(dir_path.iterdir(), key=lambda path: path.name)
    except OSError as exc:
        raise AuditError(f"failed to read {dir_path}: {exc}") from exc

    for path in entries:
        if path.is_dir():
            if path.name.endswith(".sysroot"):
                continue
            collect_scoop_files(path, paths)
        elif path.suffix == ".scoop":
            paths.append(path)


def backticked_paths_with_prefix(content: str, prefix: str) -> list[str]:
    paths: set[str] = set()
    rest = content
    while True:
        start = rest.find("`")
        if start == -1:
            break
        rest = rest[start + 1 :]
        end = rest.find("`")
        if end == -1:
            break
        token = rest[:end]
        if token.startswith(prefix) and token.endswith(".scoop"):
            paths.add(token)
        rest = rest[end + 1 :]
    return sorted(paths)


def matrix_bucket_links(content: str) -> list[str]:
    delimiters = set("()[]`| \n")
    tokens: list[str] = []
    current: list[str] = []
    for char in content:
        if char in delimiters:
            if current:
                tokens.append("".join(current))
                current.clear()
        else:
            current.append(char)
    if current:
        tokens.append("".join(current))
    return sorted(
        {
            token
            for token in tokens
            if token.startswith("UMB_categories/B-") and token.endswith(".md")
        }
    )


def parse_bucket_list(value: str) -> set[str]:
    return {bucket.strip() for bucket in value.split(",") if bucket.strip()}


def parse_umb_id_list(value: str, context: str, failures: list[str]) -> list[str]:
    trimmed = value.strip()
    if not trimmed or trimmed == "NONE":
        return []

    ids: list[str] = []
    seen: set[str] = set()
    for part in trimmed.split(","):
        umb_id = part.strip()
        if not is_umb_id(umb_id):
            failures.append(f"{context} contains invalid UMB id `{umb_id}`")
        if umb_id in seen:
            failures.append(f"{context} repeats UMB id `{umb_id}`")
        seen.add(umb_id)
        ids.append(umb_id)
    return ids


def is_umb_id(value: str) -> bool:
    return (
        len(value) == len("UMB-0000")
        and value.startswith("UMB-")
        and all(char.isdigit() and char.isascii() for char in value[4:])
    )


def contains_forbidden_term(message: str, term: str) -> bool:
    if term.isascii():
        return term.lower() in message.lower()
    return term in message


def parse_csv_records(csv: str, context: str) -> list[list[str]]:
    records: list[list[str]] = []
    record: list[str] = []
    field: list[str] = []
    index = 0
    in_quotes = False
    saw_any = False

    while index < len(csv):
        char = csv[index]
        saw_any = True
        if in_quotes:
            if char == '"' and index + 1 < len(csv) and csv[index + 1] == '"':
                index += 1
                field.append('"')
            elif char == '"':
                in_quotes = False
            else:
                field.append(char)
            index += 1
            continue

        if char == '"' and not field:
            in_quotes = True
        elif char == ",":
            finish_csv_field(record, field)
        elif char == "\n":
            finish_csv_record(records, record, field)
        elif char == "\r" and index + 1 < len(csv) and csv[index + 1] == "\n":
            index += 1
            finish_csv_record(records, record, field)
        elif char == "\r":
            finish_csv_record(records, record, field)
        else:
            field.append(char)
        index += 1

    if in_quotes:
        raise AuditError(f"{context} has an unterminated quoted CSV field")
    if saw_any and (record or field):
        finish_csv_record(records, record, field)
    return records


def finish_csv_field(record: list[str], field: list[str]) -> None:
    record.append("".join(field))
    field.clear()


def finish_csv_record(
    records: list[list[str]], record: list[str], field: list[str]
) -> None:
    finish_csv_field(record, field)
    if len(record) != 1 or record[0]:
        records.append(list(record))
    record.clear()


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
