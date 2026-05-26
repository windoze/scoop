#!/usr/bin/env python3
"""Extract spec doctest fixtures from SCOOP_FULL_SPEC.md."""

from __future__ import annotations

import argparse
import errno
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


GENERATED_DIR = "spec_doctest"


class SpecFixturesError(Exception):
    """Raised when spec fixture extraction or syncing cannot continue."""


@dataclass(frozen=True)
class GeneratedFixture:
    rel_path: Path
    content: str


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        fixtures = run(args.mode, args.fix, args.spec, args.fixtures_root)
    except SpecFixturesError as exc:
        print(f"spec fixtures: error: {exc}", file=sys.stderr)
        return 1

    if not fixtures:
        print("spec fixtures: no blocks found (ok)", file=sys.stderr)
    else:
        print(f"spec fixtures: ok ({len(fixtures)})", file=sys.stderr)
    return 0


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Extract Scoop spec doctest fixtures into tests/fixtures/spec_doctest."
    )
    parser.add_argument("mode", choices=("sync", "check"))
    parser.add_argument(
        "--fix",
        action="store_true",
        help="in check mode, write only files whose generated content changed",
    )
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


def run(mode: str, fix: bool, spec_path: Path, fixtures_root: Path) -> list[Path]:
    try:
        spec_text = read_text(spec_path)
    except OSError as exc:
        raise SpecFixturesError(f"无法读取规范文件：{spec_path}") from exc

    by_path: dict[Path, str] = {}
    for fixture in extract(spec_text):
        validate_fixture_path(fixture.rel_path)
        if fixture.rel_path in by_path:
            raise SpecFixturesError(
                f"规范里存在重复的 `// FIXTURE:` 路径：{fixture.rel_path}"
            )
        by_path[fixture.rel_path] = fixture.content

    desired = dict(sorted(by_path.items(), key=lambda item: item[0].as_posix()))
    out_root = fixtures_root / GENERATED_DIR
    if mode == "sync":
        sync(out_root, desired)
    elif fix:
        check_fix(out_root, desired)
    else:
        check(out_root, desired)

    return list(desired.keys())


def extract(spec_text: str) -> list[GeneratedFixture]:
    fixtures: list[GeneratedFixture] = []
    in_block = False
    block_lines: list[str] = []

    for line in spec_text.splitlines():
        trimmed = line.lstrip()
        if trimmed.startswith("```"):
            if in_block:
                fixture = parse_block(block_lines)
                if fixture is not None:
                    fixtures.append(fixture)
                block_lines = []
                in_block = False
            else:
                in_block = True
            continue

        if in_block:
            block_lines.append(line)

    if in_block:
        raise SpecFixturesError("规范文件存在未闭合的 fenced code block（缺少 ```）")

    return sorted(fixtures, key=lambda fixture: fixture.rel_path.as_posix())


def parse_block(lines: Sequence[str]) -> GeneratedFixture | None:
    fixture_path: Path | None = None

    for line in lines[:64]:
        trimmed = line.lstrip()
        if not trimmed.startswith("//"):
            break
        # Match Rust `trim_start_matches("//")`: repeated leading `//` pairs
        # are stripped before directive matching.
        directive = trimmed
        while directive.startswith("//"):
            directive = directive[2:]
        directive = directive.strip()
        if directive.startswith("FIXTURE:"):
            raw_path = directive.removeprefix("FIXTURE:").strip()
            fixture_path = Path(raw_path)
            break

    if fixture_path is None:
        return None

    content = "".join(f"{line}\n" for line in lines)
    return GeneratedFixture(rel_path=fixture_path, content=content)


def validate_fixture_path(rel_path: Path) -> None:
    raw_path = os.fspath(rel_path)
    if raw_path == "":
        raise SpecFixturesError("`// FIXTURE:` 路径不能为空")
    if rel_path.is_absolute():
        raise SpecFixturesError(f"`// FIXTURE:` 路径必须是相对路径：{rel_path}")
    if any(part == ".." for part in rel_path.parts):
        raise SpecFixturesError(f"`// FIXTURE:` 路径不允许包含前缀/根目录/..：{rel_path}")
    if rel_path.suffix != ".scoop":
        raise SpecFixturesError(f"`// FIXTURE:` 路径必须以 .scoop 结尾：{rel_path}")
    if not rel_path.parts or rel_path.parts[0] != GENERATED_DIR:
        raise SpecFixturesError(
            f"`// FIXTURE:` 路径必须位于 `{GENERATED_DIR}/` 下：{rel_path}"
        )


def sync(out_root: Path, desired: dict[Path, str]) -> None:
    create_dir(out_root)
    base = out_root.parent

    for rel_path, content in desired.items():
        write_file(base / rel_path, content, skip_unchanged=False)

    remove_stale_scoop_files(out_root, {base / rel_path for rel_path in desired})


def check(out_root: Path, desired: dict[Path, str]) -> None:
    base = out_root.parent

    for rel_path, content in desired.items():
        abs_path = base / rel_path
        try:
            existing = read_text(abs_path)
        except FileNotFoundError as exc:
            raise SpecFixturesError(
                f"缺少生成文件：{abs_path}（请运行 python3 tools/spec_fixtures.py sync）"
            ) from exc
        except OSError as exc:
            raise SpecFixturesError(f"无法读取：{abs_path}") from exc
        if existing != content:
            raise SpecFixturesError(
                f"生成文件与规范不一致：{abs_path}（请运行 python3 tools/spec_fixtures.py sync）"
            )

    extras = find_extra_scoop_files(out_root, {base / rel_path for rel_path in desired})
    if extras:
        msg = "spec_doctest 目录存在多余文件（请运行 python3 tools/spec_fixtures.py sync 清理）：\n"
        msg += "".join(f"  - {path}\n" for path in extras)
        raise SpecFixturesError(msg.rstrip())


def check_fix(out_root: Path, desired: dict[Path, str]) -> None:
    create_dir(out_root)
    base = out_root.parent

    for rel_path, content in desired.items():
        write_file(base / rel_path, content, skip_unchanged=True)

    remove_stale_scoop_files(out_root, {base / rel_path for rel_path in desired})


def write_file(path: Path, content: str, *, skip_unchanged: bool) -> None:
    if skip_unchanged:
        try:
            existing = read_text(path)
        except FileNotFoundError:
            existing = None
        except OSError as exc:
            raise SpecFixturesError(f"无法读取：{path}") from exc
        if existing == content:
            return

    create_dir(path.parent)
    try:
        write_text(path, content)
    except OSError as exc:
        raise SpecFixturesError(f"无法写入：{path}") from exc


def create_dir(path: Path) -> None:
    try:
        path.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        raise SpecFixturesError(f"无法创建目录：{path}") from exc


def read_text(path: Path) -> str:
    with path.open("r", encoding="utf-8", newline="") as file:
        return file.read()


def write_text(path: Path, content: str) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as file:
        file.write(content)


def remove_stale_scoop_files(dir_path: Path, keep: set[Path]) -> None:
    if not dir_path.exists():
        return
    for path in sorted(dir_path.iterdir()):
        try:
            is_dir = path.is_dir()
            is_file = path.is_file()
        except OSError as exc:
            raise SpecFixturesError(f"无法读取目录项：{path}") from exc

        if is_dir:
            remove_stale_scoop_files(path, keep)
            try_remove_empty_dir(path)
            continue

        if is_file and path.suffix == ".scoop" and path not in keep:
            try:
                path.unlink()
            except OSError as exc:
                raise SpecFixturesError(f"无法删除：{path}") from exc


def find_extra_scoop_files(dir_path: Path, keep: set[Path]) -> list[Path]:
    if not dir_path.exists():
        return []

    extras: list[Path] = []
    for path in sorted(dir_path.iterdir()):
        try:
            is_dir = path.is_dir()
            is_file = path.is_file()
        except OSError as exc:
            raise SpecFixturesError(f"无法读取目录项：{path}") from exc

        if is_dir:
            extras.extend(find_extra_scoop_files(path, keep))
        elif is_file and path.suffix == ".scoop" and path not in keep:
            extras.append(path)

    return sorted(extras, key=lambda path: path.as_posix())


def try_remove_empty_dir(path: Path) -> None:
    try:
        path.rmdir()
    except OSError as exc:
        if exc.errno not in (errno.ENOENT, errno.ENOTEMPTY, errno.EEXIST):
            raise SpecFixturesError(f"无法删除目录：{path}") from exc


if __name__ == "__main__":
    raise SystemExit(main())
