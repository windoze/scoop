#!/usr/bin/env python3
"""Report spec doctest and stdlib fixture coverage matrices."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Sequence


class MatrixError(Exception):
    """Raised when a fixtures matrix report cannot be produced."""


class ExpectationKind(Enum):
    PASS = "pass"
    FAIL = "fail"
    UNKNOWN = "unknown"
    MISSING_FILE = "missing_file"


@dataclass(frozen=True)
class Chapter:
    id: str
    title: str


@dataclass(frozen=True)
class SpecFixtureRef:
    rel_path: Path
    chapter_id: str | None


@dataclass
class Coverage:
    pass_count: int = 0
    fail_count: int = 0
    unknown_count: int = 0
    missing_files: int = 0


@dataclass(frozen=True)
class Gap:
    chapter_id: str
    title: str
    pass_count: int
    fail_count: int
    missing_pass: bool
    missing_fail: bool


@dataclass(frozen=True)
class Report:
    chapter_count: int
    fixture_count: int
    unmapped_fixture_count: int
    gaps: list[Gap]

    def render(self) -> str:
        if not self.gaps:
            out = (
                f"fixtures matrix: ok "
                f"(chapters={self.chapter_count}, fixtures={self.fixture_count})"
            )
            if self.unmapped_fixture_count > 0:
                out += f" (unmapped fixtures={self.unmapped_fixture_count})"
            return out

        out = (
            f"fixtures matrix: gaps "
            f"(chapters={self.chapter_count}, fixtures={self.fixture_count}, "
            f"missing={len(self.gaps)})\n"
        )
        out += (
            "note: 当前仅统计规范内 `// FIXTURE:` 的 doctest fixtures"
            "（按 `## N. Title` 章节归类）。\n"
        )
        for gap in self.gaps:
            missing: list[str] = []
            if gap.missing_pass:
                missing.append("pass")
            if gap.missing_fail:
                missing.append("fail")
            out += (
                f"- §{gap.chapter_id} {gap.title}: "
                f"pass={gap.pass_count} fail={gap.fail_count} "
                f"missing=[{', '.join(missing)}]\n"
            )
        if self.unmapped_fixture_count > 0:
            out += f"unmapped fixtures (no chapter context): {self.unmapped_fixture_count}\n"
        return out


@dataclass(frozen=True)
class StdlibDomain:
    id: str
    title: str
    prefixes: tuple[str, ...]


@dataclass(frozen=True)
class StdlibDomainCoverage:
    domain_id: str
    title: str
    fixture_count: int
    fixture_names: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class StdlibReport:
    domain_count: int
    total_fixtures: int
    covered: list[StdlibDomainCoverage]
    gaps: list[StdlibDomainCoverage]

    def render(self) -> str:
        covered_count = len(self.covered)
        gap_count = len(self.gaps)
        out = (
            f"stdlib coverage: {covered_count}/{self.domain_count} "
            f"domains covered ({self.total_fixtures} fixtures)\n"
        )

        if self.gaps:
            out += f"\ngaps ({gap_count} domains without fixtures):\n"
            for gap in self.gaps:
                out += f"  - §{gap.domain_id} {gap.title}\n"

        out += "\ncovered domains:\n"
        for covered in self.covered:
            out += (
                f"  + §{covered.domain_id} {covered.title} "
                f"({covered.fixture_count} fixtures)\n"
            )
        return out


STDLIB_DOMAINS: tuple[StdlibDomain, ...] = (
    StdlibDomain(
        "1",
        "Core types & primitives",
        (
            "minimal_",
            "value_type_",
            "class_",
            "enum_",
            "option_",
            "struct_",
            "string_",
            "bool_",
            "int_",
            "float_",
            "array_",
            "tuple_",
        ),
    ),
    StdlibDomain(
        "2",
        "Properties / Delegates",
        ("delegated_prop", "lazy_", "observable_", "vetoable_"),
    ),
    StdlibDomain(
        "3",
        "Collections",
        (
            "stdlib_iter_",
            "stdlib_set_",
            "stdlib_smoke_collections",
            "mutable_array_",
            "list_and_mutable_list",
            "stdlib_collections",
        ),
    ),
    StdlibDomain(
        "4",
        "Ranges / Progressions",
        ("kotlin_ranges_", "stdlib_smoke_ranges"),
    ),
    StdlibDomain(
        "5",
        "Text (String)",
        (
            "stdlib_string_",
            "string_interp",
            "string_escape",
            "string_multiline",
            "string_trim",
        ),
    ),
    StdlibDomain("6", "Text formatting", ("stdlib_string_builder", "stdlib_format")),
    StdlibDomain("7", "Math", ("stdlib_math",)),
    StdlibDomain("8", "Hashing", ("stdlib_hash",)),
    StdlibDomain("9", "Random", ("stdlib_random",)),
    StdlibDomain("10", "Time", ("std_env_time",)),
    StdlibDomain("11", "IO (stdin/stdout/stderr)", ("std_io_",)),
    StdlibDomain("12", "File system", ("std_fs_",)),
    StdlibDomain(
        "13",
        "Process / Env / Path",
        ("std_process_", "std_path_", "std_env_"),
    ),
    StdlibDomain(
        "14",
        "Concurrency / Threading",
        ("std_sync_", "std_thread_"),
    ),
    StdlibDomain("15", "Net", ("std_net_",)),
    StdlibDomain("16", "Unsafe / Pointers", ("unsafe_", "nogc_")),
    StdlibDomain("17", "Scope functions", ("kotlin_scope_functions",)),
    StdlibDomain(
        "18",
        "Preconditions",
        (
            "kotlin_require_",
            "kotlin_check_",
            "stdlib_smoke_test_and_preconditions",
        ),
    ),
    StdlibDomain("19", "Test utilities", ("std_test_",)),
    StdlibDomain("20", "Reflection", ("reflection_",)),
)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        if args.mode == "check":
            report = run_check(args.spec, args.fixtures_root)
        else:
            report = run_stdlib_check(args.fixtures_root)
    except MatrixError as exc:
        print(f"fixtures matrix: error: {exc}", file=sys.stderr)
        return 1

    print(report.render(), file=sys.stderr)
    return 0


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Report Scoop fixture coverage matrices."
    )
    parser.add_argument("mode", choices=("check", "stdlib"))
    parser.add_argument(
        "--spec",
        type=Path,
        default=Path("SCOOP_FULL_SPEC.md"),
        help="specification markdown path (default: SCOOP_FULL_SPEC.md)",
    )
    parser.add_argument(
        "--fixtures-root",
        type=Path,
        default=Path("tests/fixtures"),
        help="fixtures root directory (default: tests/fixtures)",
    )
    return parser.parse_args(argv)


def run_check(spec_path: Path, fixtures_root: Path) -> Report:
    try:
        spec_text = read_text(spec_path)
    except OSError as exc:
        raise MatrixError(f"无法读取规范文件：{spec_path}") from exc

    chapters, fixture_refs = parse_spec_chapters_and_fixtures(spec_text)
    chapter_ids = {chapter.id for chapter in chapters}
    coverage_by_chapter: dict[str, Coverage] = {}
    unmapped_fixture_count = 0

    for fixture_ref in fixture_refs:
        chapter_id = fixture_ref.chapter_id
        if chapter_id is None or chapter_id not in chapter_ids:
            unmapped_fixture_count += 1
            continue

        kind = read_fixture_expectation(fixtures_root, fixture_ref.rel_path)
        coverage = coverage_by_chapter.setdefault(chapter_id, Coverage())
        if kind is ExpectationKind.PASS:
            coverage.pass_count += 1
        elif kind is ExpectationKind.FAIL:
            coverage.fail_count += 1
        elif kind is ExpectationKind.UNKNOWN:
            coverage.unknown_count += 1
        elif kind is ExpectationKind.MISSING_FILE:
            coverage.unknown_count += 1
            coverage.missing_files += 1

    gaps: list[Gap] = []
    for chapter in chapters:
        coverage = coverage_by_chapter.get(chapter.id, Coverage())
        missing_pass = coverage.pass_count == 0
        missing_fail = coverage.fail_count == 0
        if missing_pass or missing_fail:
            gaps.append(
                Gap(
                    chapter_id=chapter.id,
                    title=chapter.title,
                    pass_count=coverage.pass_count,
                    fail_count=coverage.fail_count,
                    missing_pass=missing_pass,
                    missing_fail=missing_fail,
                )
            )

    return Report(
        chapter_count=len(chapters),
        fixture_count=len(fixture_refs),
        unmapped_fixture_count=unmapped_fixture_count,
        gaps=gaps,
    )


def parse_spec_chapters_and_fixtures(
    spec_text: str,
) -> tuple[list[Chapter], list[SpecFixtureRef]]:
    chapters: list[Chapter] = []
    fixtures: list[SpecFixtureRef] = []
    in_block = False
    block_lines: list[str] = []
    current_chapter_id: str | None = None

    for line in spec_text.splitlines():
        chapter = parse_chapter_heading(line)
        if chapter is not None:
            current_chapter_id = chapter.id
            chapters.append(chapter)

        trimmed = line.lstrip()
        if trimmed.startswith("```"):
            if in_block:
                rel_path = parse_fixture_path_from_block(block_lines)
                if rel_path is not None:
                    fixtures.append(
                        SpecFixtureRef(
                            rel_path=rel_path,
                            chapter_id=current_chapter_id,
                        )
                    )
                block_lines = []
                in_block = False
            else:
                in_block = True
            continue

        if in_block:
            block_lines.append(line)

    if in_block:
        raise MatrixError("规范文件存在未闭合的 fenced code block（缺少 ```）")

    seen: set[str] = set()
    unique_chapters: list[Chapter] = []
    for chapter in chapters:
        if chapter.id in seen:
            continue
        seen.add(chapter.id)
        unique_chapters.append(chapter)

    return unique_chapters, fixtures


def parse_chapter_heading(line: str) -> Chapter | None:
    line = line.lstrip()
    if not line.startswith("## "):
        return None
    rest = line.removeprefix("## ").strip()
    if "." not in rest:
        return None
    number, title = rest.split(".", 1)
    if not number or not all("0" <= char <= "9" for char in number):
        return None
    title = title.strip()
    if not title:
        return None
    return Chapter(id=number, title=title)


def parse_fixture_path_from_block(lines: Sequence[str]) -> Path | None:
    for line in lines[:64]:
        trimmed = line.lstrip()
        if not trimmed.startswith("//"):
            break
        directive = strip_comment_prefixes(trimmed)
        if directive.startswith("FIXTURE:"):
            return Path(directive.removeprefix("FIXTURE:").strip())
    return None


def read_fixture_expectation(fixtures_root: Path, rel_path: Path) -> ExpectationKind:
    abs_path = fixtures_root / rel_path
    try:
        source = read_text(abs_path)
    except FileNotFoundError:
        return ExpectationKind.MISSING_FILE
    except OSError as exc:
        raise MatrixError(f"无法读取 fixture 文件：{abs_path}") from exc
    return parse_expectation_from_source(source)


def parse_expectation_from_source(source: str) -> ExpectationKind:
    for line in source.splitlines()[:32]:
        trimmed = line.lstrip()
        if not trimmed.startswith("//"):
            break
        directive = strip_comment_prefixes(trimmed)
        if not directive.startswith("EXPECT:"):
            continue
        value = directive.removeprefix("EXPECT:").strip()
        if value == "pass":
            return ExpectationKind.PASS
        if value == "fail":
            return ExpectationKind.FAIL
        return ExpectationKind.UNKNOWN
    return ExpectationKind.UNKNOWN


def strip_comment_prefixes(trimmed_comment: str) -> str:
    directive = trimmed_comment
    while directive.startswith("//"):
        directive = directive[2:]
    return directive.strip()


def run_stdlib_check(fixtures_root: Path) -> StdlibReport:
    run_pass_dir = fixtures_root / "run-pass"
    fixture_stems: list[str] = []
    if run_pass_dir.is_dir():
        try:
            entries = list(run_pass_dir.iterdir())
        except OSError as exc:
            raise MatrixError(f"无法读取 run-pass 目录：{run_pass_dir}") from exc
        for path in entries:
            if path.suffix == ".scoop":
                fixture_stems.append(path.stem)
    fixture_stems.sort()

    covered: list[StdlibDomainCoverage] = []
    gaps: list[StdlibDomainCoverage] = []
    total_matched = 0

    for domain in STDLIB_DOMAINS:
        matched = [
            stem
            for stem in fixture_stems
            if any(stem.startswith(prefix) for prefix in domain.prefixes)
        ]
        count = len(matched)
        total_matched += count
        entry = StdlibDomainCoverage(
            domain_id=domain.id,
            title=domain.title,
            fixture_count=count,
            fixture_names=matched,
        )
        if count > 0:
            covered.append(entry)
        else:
            gaps.append(entry)

    return StdlibReport(
        domain_count=len(STDLIB_DOMAINS),
        total_fixtures=total_matched,
        covered=covered,
        gaps=gaps,
    )


def read_text(path: Path) -> str:
    with path.open("r", encoding="utf-8", newline="") as file:
        return file.read()


if __name__ == "__main__":
    raise SystemExit(main())
