#!/usr/bin/env python3
"""Run Scoop regression fixtures from outside the compiler binaries.

The script intentionally keeps fixture knowledge in `tools/` and drives the
compiler only through public `scoopc` / `scoop` command surfaces.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import dataclasses
import difflib
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Iterable, Sequence


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_FIXTURES = REPO_ROOT / "tests" / "fixtures"
DEFAULT_PROCESSES = 5
SYSROOT_OVERLAY_ENV = "SCOOP_SYSROOT_OVERLAY"
SYSROOT_DEPS_ENV = "SCOOP_SYSROOT_DEPS"


class FixtureError(Exception):
    def __init__(
        self,
        message: str,
        *,
        code: str | None = None,
        stdout: str = "",
        stderr: str = "",
    ) -> None:
        super().__init__(message)
        self.message = message
        self.code = code
        self.stdout = stdout
        self.stderr = stderr


@dataclasses.dataclass(frozen=True)
class Expectation:
    expect: str = "pass"
    error_contains: tuple[str, ...] = ()
    error_not_contains: tuple[str, ...] = ()
    error_code: str | None = None
    error_at: tuple[int, int] | None = None
    args: tuple[str, ...] = ()
    env: tuple[tuple[str, str], ...] = ()
    sysroot_deps: tuple[str, ...] = ()
    ast_golden: str | None = None
    build_llvm_contains: tuple[str, ...] = ()
    build_llvm_regex: tuple[str, ...] = ()
    build_llvm_not_contains: tuple[str, ...] = ()
    run_stdout: str | None = None
    run_stderr: str | None = None
    run_stdin: str | None = None
    run_mode: str | None = None
    run_stdout_contains: str | None = None
    run_stderr_contains: str | None = None
    run_stackmaps_records_gt: int | None = None
    expect_exit: int | None = None
    timeout_ms: int | None = None
    ignore_until_fix: str | None = None


@dataclasses.dataclass(frozen=True)
class Target:
    path: Path
    display: Path


@dataclasses.dataclass(frozen=True)
class Options:
    scoopc: Path
    scoop: Path
    opt_level: str | None
    gc_stress: bool
    gc_move: bool
    threads: int | None


@dataclasses.dataclass(frozen=True)
class CommandOutput:
    returncode: int
    stdout: str
    stderr: str
    timed_out: bool = False


@dataclasses.dataclass(frozen=True)
class WorkerSuccess:
    checks: int
    stdout: str = ""
    stderr: str = ""


@dataclasses.dataclass(frozen=True)
class WorkerFailure:
    message: str
    stdout: str = ""
    stderr: str = ""


@dataclasses.dataclass(frozen=True)
class WorkerResult:
    target: Target
    result: WorkerSuccess | WorkerFailure


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    root = resolve_fixture_root(args.fixture_path, args.fixtures)
    scoopc = resolve_tool("scoopc", "SCOOPC_BIN")
    scoop = resolve_tool("scoop", "SCOOP_BIN")
    options = Options(
        scoopc=scoopc,
        scoop=scoop,
        opt_level=args.opt_level,
        gc_stress=args.gc_stress,
        gc_move=args.gc_move,
        threads=args.threads,
    )
    targets = plan_targets(root)
    total_targets = len(targets)
    if total_targets == 0:
        print("fixtures: ok (0)")
        return 0

    max_workers = max(1, min(args.processes, total_targets))
    passed_targets = 0
    failed_targets = 0
    passed_checks = 0
    completed = 0
    stopped_early = False

    with concurrent.futures.ProcessPoolExecutor(max_workers=max_workers) as executor:
        pending = list(targets)
        futures: dict[concurrent.futures.Future[WorkerResult], Target] = {}

        def schedule_more() -> None:
            nonlocal pending
            while (
                pending
                and len(futures) < max_workers
                and not (args.exit_on_failure and failed_targets > 0)
            ):
                target = pending.pop(0)
                futures[executor.submit(run_worker, target, options)] = target

        schedule_more()
        while futures:
            done, _ = concurrent.futures.wait(
                futures, return_when=concurrent.futures.FIRST_COMPLETED
            )
            for future in done:
                target = futures.pop(future)
                completed += 1
                try:
                    worker = future.result()
                except BaseException as exc:  # pragma: no cover - worker crash path
                    worker = WorkerResult(
                        target=target,
                        result=WorkerFailure(f"fixture worker failed: {exc}"),
                    )

                if isinstance(worker.result, WorkerSuccess):
                    emit_stdout(worker.result.stdout)
                    emit_stderr(worker.result.stderr)
                    passed_targets += 1
                    passed_checks += worker.result.checks
                    print(
                        f"[{completed}/{total_targets}] PASS "
                        f"{worker.target.display} ({format_check_count(worker.result.checks)})"
                    )
                else:
                    failed_targets += 1
                    print_worker_failure(completed, total_targets, worker.target, worker.result)
                    if args.exit_on_failure:
                        pending.clear()
                        stopped_early = True
                schedule_more()

    if failed_targets == 0:
        print(f"fixtures: ok ({passed_checks})")
        return 0

    summary = (
        f"fixtures: failed ({failed_targets}/{total_targets} targets failed, "
        f"{passed_targets} targets passed, {format_check_count(passed_checks)} passed)"
    )
    if stopped_early:
        summary += ", stopped scheduling new targets after the first failure"
    print(summary)
    return 1


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run Scoop fixture regressions through public compiler commands."
    )
    parser.add_argument(
        "fixture_path",
        nargs="?",
        type=Path,
        help="fixture directory, case directory, or .scoop file",
    )
    parser.add_argument("--fixtures", type=Path, help="fixture root or single fixture")
    parser.add_argument(
        "-j",
        "--processes",
        type=positive_int,
        default=DEFAULT_PROCESSES,
        metavar="N",
        help=f"parallel fixture workers (default: {DEFAULT_PROCESSES})",
    )
    parser.add_argument("--exit-on-failure", action="store_true")
    parser.add_argument("-O", "--opt-level", metavar="LEVEL")
    parser.add_argument("--gc-stress", action="store_true")
    parser.add_argument("--gc-move", action="store_true")
    parser.add_argument("--threads", type=positive_int, metavar="N")
    return parser.parse_args(argv)


def positive_int(value: str) -> int:
    parsed = int(value, 10)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def resolve_fixture_root(positional: Path | None, flag: Path | None) -> Path:
    if positional is not None and flag is not None:
        raise SystemExit("positional fixture path conflicts with --fixtures")
    root = flag or positional or DEFAULT_FIXTURES
    try:
        return root.expanduser().resolve(strict=True)
    except FileNotFoundError as exc:
        raise SystemExit(
            f"cannot locate fixtures path: {root} (use --fixtures to select a root)"
        ) from exc


def resolve_tool(name: str, env_name: str) -> Path:
    env_value = os.environ.get(env_name)
    if env_value:
        path = Path(env_value).expanduser().resolve()
        if path.is_file():
            return path
        raise SystemExit(f"{env_name} points to a missing executable: {path}")

    exe_name = f"{name}.exe" if os.name == "nt" else name
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", REPO_ROOT / "target"))
    candidate = target_dir / "debug" / exe_name
    if candidate.is_file():
        return candidate.resolve()

    cargo = shutil.which("cargo")
    if cargo is None:
        raise SystemExit(
            f"cannot find {name}; set {env_name} or install cargo to build target/debug/{exe_name}"
        )
    build = subprocess.run(
        [cargo, "build", "--quiet", "-p", "scoop", "-p", "scoopc"],
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if build.returncode != 0:
        sys.stderr.write(build.stdout)
        sys.stderr.write(build.stderr)
        raise SystemExit(f"failed to build {name}")
    if candidate.is_file():
        return candidate.resolve()
    raise SystemExit(f"cargo build completed but {candidate} was not produced")


def run_worker(target: Target, options: Options) -> WorkerResult:
    try:
        checks, stdout = run_target(target.path, options)
        return WorkerResult(target=target, result=WorkerSuccess(checks=checks, stdout=stdout))
    except FixtureError as exc:
        return WorkerResult(
            target=target,
            result=WorkerFailure(exc.message, stdout=exc.stdout, stderr=exc.stderr),
        )
    except BaseException as exc:
        return WorkerResult(target=target, result=WorkerFailure(str(exc)))


def run_target(root: Path, options: Options) -> tuple[int, str]:
    if root.is_file():
        stdout = []
        run_one_file(root.parent, root, options, stdout)
        return 1, "".join(stdout)
    if is_resolve_multi_case_root(root):
        return run_check_source_case(root, "resolve", options), ""
    if is_resolve_cone_case_root(root):
        return run_check_source_case(root, "resolve", options, cone_case=True), ""
    if is_typecheck_multi_case_root(root):
        return run_check_source_case(root, "typecheck", options), ""
    if is_typecheck_cone_case_root(root):
        return run_check_source_case(root, "typecheck", options, cone_case=True), ""
    if is_run_pass_cone_case_root(root):
        stdout = []
        run_run_pass_cone_case(root, options, stdout)
        return 1, "".join(stdout)

    checks = 0
    stdout = []
    for target in plan_targets(root):
        sub_checks, sub_stdout = run_target(target.path, options)
        checks += sub_checks
        stdout.append(sub_stdout)
    return checks, "".join(stdout)


def plan_targets(fixtures_root: Path) -> list[Target]:
    if (
        fixtures_root.is_file()
        or is_resolve_multi_case_root(fixtures_root)
        or is_resolve_cone_case_root(fixtures_root)
        or is_typecheck_multi_case_root(fixtures_root)
        or is_typecheck_cone_case_root(fixtures_root)
        or is_run_pass_cone_case_root(fixtures_root)
    ):
        return [Target(fixtures_root, display_target(fixtures_root, fixtures_root))]

    resolve_multi_root = fixtures_root / "resolve_multi"
    resolve_cone_root = fixtures_root / "resolve_cone"
    typecheck_multi_root = fixtures_root / "typecheck_multi"
    typecheck_cone_root = fixtures_root / "typecheck_cone"
    run_pass_root = run_pass_cone_root(fixtures_root)

    skip_dirs = [
        path
        for path in (
            resolve_multi_root,
            resolve_cone_root,
            typecheck_multi_root,
            typecheck_cone_root,
            run_pass_root,
        )
        if path.is_dir()
    ]
    files = collect_scoop_files(fixtures_root, skip_dirs=skip_dirs)
    targets = [Target(path, display_target(fixtures_root, path)) for path in files]
    for case_dir in collect_case_dirs(resolve_multi_root):
        targets.append(Target(case_dir, display_target(fixtures_root, case_dir)))
    for case_dir in collect_case_dirs(resolve_cone_root):
        targets.append(Target(case_dir, display_target(fixtures_root, case_dir)))
    for case_dir in collect_case_dirs(typecheck_multi_root):
        targets.append(Target(case_dir, display_target(fixtures_root, case_dir)))
    for case_dir in collect_case_dirs(typecheck_cone_root):
        targets.append(Target(case_dir, display_target(fixtures_root, case_dir)))
    for case_dir in collect_case_dirs(run_pass_root):
        targets.append(Target(case_dir, display_target(fixtures_root, case_dir)))

    if not targets and (is_under_umb_fix(fixtures_root) or fixtures_root.name == "typecheck_cone_archive"):
        return []
    if not targets:
        raise FixtureError(f"fixtures directory contains no .scoop files: {fixtures_root}")
    return targets


def collect_scoop_files(root: Path, skip_dirs: Sequence[Path] = ()) -> list[Path]:
    out: list[Path] = []
    skip_resolved = tuple(path.resolve() for path in skip_dirs)

    def walk(dir_path: Path) -> None:
        resolved = dir_path.resolve()
        if any(is_relative_to(resolved, skipped) for skipped in skip_resolved):
            return
        for entry in sorted(dir_path.iterdir()):
            if entry.is_dir():
                if is_fixture_sysroot_overlay_dir(entry):
                    continue
                if entry.name == "build" and (
                    (entry / "debug").is_dir() or (entry / "release").is_dir()
                ):
                    continue
                walk(entry)
            elif entry.is_file() and entry.suffix == ".scoop":
                out.append(entry.resolve())

    walk(root)
    return sorted(out)


def collect_case_dirs(root: Path) -> list[Path]:
    if not root.is_dir():
        return []
    return sorted(
        entry.resolve()
        for entry in root.iterdir()
        if entry.is_dir() and not is_fixture_sysroot_overlay_dir(entry)
    )


def is_relative_to(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def display_target(fixtures_root: Path, target: Path) -> Path:
    try:
        rel = target.relative_to(fixtures_root)
        if rel.parts:
            return rel
    except ValueError:
        pass
    return Path(target.name) if target.name else target


def has_parent_dir_name(path: Path, name: str) -> bool:
    return path.is_dir() and path.parent.name == name


def is_resolve_multi_case_root(path: Path) -> bool:
    return has_parent_dir_name(path, "resolve_multi")


def is_resolve_cone_case_root(path: Path) -> bool:
    return has_parent_dir_name(path, "resolve_cone")


def is_typecheck_multi_case_root(path: Path) -> bool:
    return has_parent_dir_name(path, "typecheck_multi")


def is_typecheck_cone_case_root(path: Path) -> bool:
    return has_parent_dir_name(path, "typecheck_cone")


def is_run_pass_cone_case_root(path: Path) -> bool:
    return (
        has_parent_dir_name(path, "run_pass_cone")
        and (path / "Cone.toml").is_file()
        and (path / "src" / "main.scoop").is_file()
    )


def run_pass_cone_root(root: Path) -> Path:
    return root if root.name == "run_pass_cone" else root / "run_pass_cone"


def is_under_umb_fix(path: Path) -> bool:
    return "umb_fix" in path.parts


def is_fixture_sysroot_overlay_dir(path: Path) -> bool:
    return path.name.endswith(".sysroot")


PHASE_DIRS = {
    "parse",
    "spec_doctest",
    "build",
    "resolve",
    "typecheck",
    "unsafe_nogc",
    "infer",
    "codegen",
    "run-pass",
    "runtime_gc",
    "hir",
    "mir",
    "mir_lowered",
    "mir_materialized",
    "effect_facts",
    "effect_lowered",
    "scoopir",
    "umb_fix",
}


def phase_name(fixtures_root: Path, rel: Path) -> str | None:
    if len(rel.parts) >= 2:
        return rel.parts[0]
    if len(rel.parts) == 1:
        for ancestor in (fixtures_root, *fixtures_root.parents):
            if ancestor.name in PHASE_DIRS:
                return ancestor.name
    return None


def run_one_file(fixtures_root: Path, path: Path, options: Options, stdout: list[str]) -> None:
    exp = parse_expectations(path)
    rel = safe_relative(path, fixtures_root)
    if exp.ignore_until_fix is not None:
        stdout.append(f"SKIP {rel} (IGNORE-UNTIL-FIX:{exp.ignore_until_fix})\n")
        return

    phase = phase_name(fixtures_root, rel)
    if phase in (None, "parse", "spec_doctest"):
        result = lambda: run_parse_fixture(path, exp, options)
    elif phase == "build":
        result = lambda: run_build_fixture(rel, path, exp, options)
    elif phase == "resolve":
        result = lambda: run_check_source_file(path, "resolve", exp, options)
    elif phase in ("typecheck", "unsafe_nogc"):
        result = lambda: run_check_source_file(path, "typecheck", exp, options)
    elif phase == "infer":
        result = lambda: run_check_source_file(path, "infer", exp, options)
    elif phase in ("codegen", "run-pass", "runtime_gc"):
        result = lambda: run_run_pass_fixture(rel, path, exp, options)
    elif phase == "hir":
        result = lambda: run_dump_golden(path, "dump-hir", path.with_suffix(".hir"), options)
    elif phase in ("mir", "mir_lowered"):
        result = lambda: run_dump_golden(path, "dump-mir", path.with_suffix(".mir"), options)
    elif phase == "mir_materialized":
        result = lambda: run_dump_golden(path, "dump-ir", path.with_suffix(".mir"), options)
    elif phase == "effect_facts":
        result = lambda: run_dump_golden(
            path, "dump-effect-facts", path.with_suffix(".effectfacts"), options
        )
    elif phase == "effect_lowered":
        result = lambda: run_dump_golden(
            path, "dump-effect-lowered", path.with_suffix(".effectlowered"), options
        )
    elif phase == "scoopir":
        result = lambda: run_scoopir_fixture(path, exp, options)
    elif phase == "umb_fix":
        result = lambda: run_umb_fix_fixture(rel, path, exp, options)
    else:
        result = lambda: raise_fixture_error(
            f"fixtures phase `{phase}` is not implemented (fixture: {rel})",
            code="scoop::fixtures::unimplemented_phase",
        )
    assert_expectation(path, exp, result)


def run_umb_fix_fixture(rel: Path, path: Path, exp: Expectation, options: Options) -> None:
    if is_umb_fix_build_fixture(exp):
        run_build_fixture(rel, path, exp, options)
    elif is_umb_fix_run_pass_fixture(path, exp):
        run_run_pass_fixture(rel, path, exp, options)
    else:
        run_check_source_file(path, "typecheck", exp, options)


def is_umb_fix_build_fixture(exp: Expectation) -> bool:
    return bool(
        exp.build_llvm_contains
        or exp.build_llvm_regex
        or exp.build_llvm_not_contains
        or any(arg in ("--emit-llvm", "--emit-obj", "--emit-asm") for arg in exp.args)
    )


def is_umb_fix_run_pass_fixture(path: Path, exp: Expectation) -> bool:
    return bool(
        exp.run_stdout
        or exp.run_stderr
        or exp.run_stdin
        or exp.run_mode
        or exp.run_stdout_contains
        or exp.run_stderr_contains
        or exp.run_stackmaps_records_gt is not None
        or exp.expect_exit is not None
        or path.with_suffix(".stdout").is_file()
        or path.with_suffix(".stderr").is_file()
    )


def assert_expectation(path: Path, exp: Expectation, action) -> None:
    try:
        action()
    except FixtureError as exc:
        if exp.expect == "fail":
            assert_diagnostic_matches(path, exp, exc)
            return
        raise FixtureError(f"expected pass, but fixture failed: {exc.message}") from exc
    if exp.expect == "fail":
        raise FixtureError("expected failure, but fixture succeeded")


def run_parse_fixture(path: Path, exp: Expectation, options: Options) -> None:
    if exp.ast_golden is None:
        run_check_source_file(path, "parse", exp, options)
        return
    output = run_command(
        [options.scoopc, "dump-ast", path],
        env=env_for_expectation(path, exp),
    )
    if output.returncode != 0:
        raise command_failure("scoopc dump-ast", path, output)
    compare_text(
        normalize_newlines((path.parent / exp.ast_golden).read_text()),
        normalize_newlines(output.stdout),
        path.parent / exp.ast_golden,
        path,
        "AST snapshot",
    )


def run_check_source_file(
    path: Path, phase: str, exp: Expectation, options: Options
) -> None:
    cmd: list[Path | str] = [
        options.scoopc,
        "check-source",
        "--phase",
        phase,
        "--input",
        path,
    ]
    target_platform = target_platform_from_args(exp.args)
    if target_platform is not None:
        cmd.extend(["--target-platform", target_platform])
    output = run_command(cmd, env=env_for_expectation(path, exp))
    if output.returncode != 0:
        raise command_failure(f"scoopc check-source --phase {phase}", path, output)


def run_check_source_case(
    case_dir: Path, phase: str, options: Options, cone_case: bool = False
) -> int:
    paths = collect_case_source_files(case_dir, cone_case)
    min_count = 2
    if len(paths) < min_count:
        kind = f"{phase}_{'cone' if cone_case else 'multi'}"
        raise FixtureError(f"{kind} case requires at least 2 .scoop files (fixture: {case_dir})")

    with tempfile.TemporaryDirectory(prefix="scoop_fixture_case_") as tmp:
        tmp_root = Path(tmp)
        if cone_case:
            project_root, source_map = materialize_cone_case(case_dir, tmp_root)
        else:
            project_root, source_map = materialize_multi_case(case_dir, tmp_root)

        checked = 0
        for original in paths:
            exp = parse_expectations(original)
            selected = source_map[original]
            cmd: list[Path | str] = [
                options.scoopc,
                "check-source",
                "--phase",
                phase,
                "--input",
                project_root,
                "--source",
                selected,
            ]
            target_platform = target_platform_from_args(exp.args)
            if target_platform is not None:
                cmd.extend(["--target-platform", target_platform])
            output = run_command(cmd, env=env_for_target(case_dir, exp))

            def action(output: CommandOutput = output, selected: Path = selected) -> None:
                if output.returncode != 0:
                    raise command_failure(
                        f"scoopc check-source --phase {phase}",
                        selected,
                        output,
                    )

            assert_expectation(original, exp, action)
            checked += 1
        return checked


def collect_case_source_files(case_dir: Path, cone_case: bool) -> list[Path]:
    if not cone_case:
        return collect_scoop_files(case_dir)

    out: list[Path] = []
    cone_dirs = sorted(
        entry
        for entry in case_dir.iterdir()
        if entry.is_dir() and not is_fixture_sysroot_overlay_dir(entry)
    )
    if len(cone_dirs) < 2:
        raise FixtureError(f"cone case requires at least 2 cone directories (fixture: {case_dir})")
    for cone_dir in cone_dirs:
        files = collect_scoop_files(cone_dir)
        if not files:
            raise FixtureError(
                f"cone directory contains no .scoop files (fixture: {case_dir}; cone: {cone_dir.name})"
            )
        out.extend(files)
    return sorted(out)


def materialize_multi_case(case_dir: Path, tmp_root: Path) -> tuple[Path, dict[Path, Path]]:
    project_root = tmp_root / "case"
    src_root = project_root / "src"
    src_root.mkdir(parents=True)
    write_manifest(project_root, "fixture.case", "lib")
    source_map: dict[Path, Path] = {}
    for original in collect_scoop_files(case_dir):
        rel = original.relative_to(case_dir)
        dest = src_root / rel
        copy_source(original, dest)
        source_map[original] = dest
    return project_root, source_map


def materialize_cone_case(case_dir: Path, tmp_root: Path) -> tuple[Path, dict[Path, Path]]:
    cone_dirs = sorted(
        entry
        for entry in case_dir.iterdir()
        if entry.is_dir() and not is_fixture_sysroot_overlay_dir(entry)
    )
    root_source = next((entry for entry in cone_dirs if entry.name == "app"), cone_dirs[0])
    source_map: dict[Path, Path] = {}
    temp_dirs: dict[Path, Path] = {}
    for cone_dir in cone_dirs:
        temp_cone = tmp_root / cone_dir.name
        temp_dirs[cone_dir] = temp_cone
        (temp_cone / "src").mkdir(parents=True)
        deps = {}
        if cone_dir == root_source:
            for dep_dir in cone_dirs:
                if dep_dir != root_source:
                    deps[manifest_name_for_cone(dep_dir)] = f"../{dep_dir.name}"
        write_manifest(temp_cone, manifest_name_for_cone(cone_dir), "lib", deps)
        for original in collect_scoop_files(cone_dir):
            rel = original.relative_to(cone_dir)
            dest = temp_cone / "src" / rel
            copy_source(original, dest)
            source_map[original] = dest
    return temp_dirs[root_source], source_map


def manifest_name_for_cone(cone_dir: Path) -> str:
    safe = re.sub(r"[^A-Za-z0-9_.-]+", "_", cone_dir.name)
    return f"fixture.{safe}"


def write_manifest(
    root: Path, name: str, kind: str, deps: dict[str, str] | None = None
) -> None:
    lines = [
        "[cone]",
        f'name = "{name}"',
        'version = "0.0.0"',
        f'kind = "{kind}"',
        "",
    ]
    if deps:
        lines.append("[dependencies]")
        for dep_name, dep_path in deps.items():
            lines.append(f'"{dep_name}" = {{ path = "{dep_path}" }}')
        lines.append("")
    (root / "Cone.toml").write_text("\n".join(lines))


def copy_source(original: Path, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(original, dest)


def run_dump_golden(path: Path, command: str, golden_path: Path, options: Options) -> None:
    exp = parse_expectations(path)
    output = run_command(
        [options.scoopc, command, path],
        env=env_for_expectation(path, exp),
    )
    if output.returncode != 0:
        raise command_failure(f"scoopc {command}", path, output)
    compare_text(
        normalize_newlines(golden_path.read_text()),
        normalize_newlines(output.stdout),
        golden_path,
        path,
        f"{command} snapshot",
    )


def run_scoopir_fixture(path: Path, exp: Expectation, options: Options) -> None:
    with tempfile.TemporaryDirectory(prefix="scoop_fixture_scoopir_") as tmp:
        tmp_root = Path(tmp)
        project_root = tmp_root / "project"
        src_root = project_root / "src"
        src_root.mkdir(parents=True)
        write_manifest(project_root, "fixture.scoopir", "lib")
        copy_source(path, src_root / path.name)
        out = tmp_root / "artifact"
        output = run_command(
            [
                options.scoopc,
                "build-single-cone",
                "--cone-root",
                project_root,
                "--out",
                out,
                "--inputs-fingerprint",
                "00",
            ],
            env=env_for_expectation(path, exp),
        )
        if output.returncode != 0:
            raise located_frontend_failure(
                "scoopc build-single-cone",
                path,
                exp,
                output,
                options,
                phases=("parse", "typecheck"),
            )
        frontend_import = json.loads((out / "frontend_import.json").read_text())
        actual = json.dumps(frontend_import["public_api"], ensure_ascii=False, indent=2) + "\n"
        golden_path = path.with_suffix(".scoopir.json")
        compare_text(
            normalize_newlines(golden_path.read_text()),
            normalize_newlines(actual),
            golden_path,
            path,
            "ScoopIR snapshot",
        )


def run_build_fixture(rel: Path, path: Path, exp: Expectation, options: Options) -> None:
    emit = build_emit_from_args(exp.args)
    needs_llvm_assertions = bool(
        exp.build_llvm_contains or exp.build_llvm_regex or exp.build_llvm_not_contains
    )
    if emit is None:
        if needs_llvm_assertions:
            raise FixtureError(
                f"build LLVM IR assertions require --emit-llvm (fixture: {path})",
                code="scoop::fixtures::build_llvm_ir_assert_requires_emit_llvm",
            )
        return
    if needs_llvm_assertions and emit[0] != "llvm-ir":
        raise FixtureError(
            f"build LLVM IR assertions require --emit-llvm (fixture: {path})",
            code="scoop::fixtures::build_llvm_ir_assert_requires_emit_llvm",
        )

    output_path = build_fixture_output_path(rel, emit[1])
    if output_path.parent.exists():
        shutil.rmtree(output_path.parent)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    opt_level = options.opt_level or opt_level_from_args(exp.args)
    cmd: list[Path | str] = [
        options.scoopc,
        "emit-artifact",
        "--kind",
        emit[0],
        "--input",
        path,
        "--out",
        output_path,
    ]
    if opt_level is not None:
        cmd.extend(["--opt-level", opt_level])
    output = run_command(cmd, env=env_for_expectation(path, exp))
    if output.returncode != 0:
        raise located_frontend_failure(
            "scoopc emit-artifact",
            path,
            exp,
            output,
            options,
            phases=("typecheck", "parse"),
        )
    if not output_path.is_file():
        raise FixtureError(
            f"build fixture did not produce expected artifact: {output_path} (fixture: {path})",
            code="scoop::fixtures::build_artifact_missing",
        )
    if output_path.stat().st_size == 0:
        raise FixtureError(
            f"build fixture artifact is empty: {output_path} (fixture: {path})",
            code="scoop::fixtures::build_artifact_empty",
        )

    if needs_llvm_assertions:
        ir = output_path.read_text()
        for substring in exp.build_llvm_contains:
            if substring not in ir:
                raise FixtureError(
                    f"build LLVM IR did not contain expected substring: {substring} (fixture: {path})",
                    code="scoop::fixtures::build_llvm_ir_missing_substring",
                )
        for pattern in exp.build_llvm_regex:
            try:
                compiled = re.compile(pattern)
            except re.error as exc:
                raise FixtureError(
                    f"build LLVM IR regex failed to compile: {pattern} (fixture: {path}): {exc}",
                    code="scoop::fixtures::build_llvm_ir_regex_compile_failed",
                ) from exc
            if compiled.search(ir) is None:
                raise FixtureError(
                    f"build LLVM IR did not match expected regex: {pattern} (fixture: {path})",
                    code="scoop::fixtures::build_llvm_ir_missing_regex_match",
                )
        for substring in exp.build_llvm_not_contains:
            if substring in ir:
                raise FixtureError(
                    f"build LLVM IR contained forbidden substring: {substring} (fixture: {path})",
                    code="scoop::fixtures::build_llvm_ir_unexpected_substring",
                )


def build_emit_from_args(args: Sequence[str]) -> tuple[str, str] | None:
    if "--emit-llvm" in args:
        return "llvm-ir", "ll"
    if "--emit-obj" in args:
        return "obj", "obj" if os.name == "nt" else "o"
    if "--emit-asm" in args:
        return "asm", "asm" if os.name == "nt" else "s"
    return None


def build_fixture_output_path(rel: Path, ext: str) -> Path:
    return REPO_ROOT / "target" / "fixtures" / rel.with_suffix("") / f"artifact.{ext}"


def opt_level_from_args(args: Sequence[str]) -> str | None:
    iterator = iter(range(len(args)))
    for idx in iterator:
        arg = args[idx]
        if arg.startswith("-O"):
            rest = arg[2:]
            if rest:
                return rest
            if idx + 1 >= len(args):
                raise FixtureError(
                    "build fixture directive is missing opt-level value",
                    code="scoop::fixtures::build_missing_opt_level_value",
                )
            return args[idx + 1]
        if arg == "--opt-level":
            if idx + 1 >= len(args):
                raise FixtureError(
                    "build fixture directive is missing opt-level value",
                    code="scoop::fixtures::build_missing_opt_level_value",
                )
            return args[idx + 1]
        if arg.startswith("--opt-level="):
            return arg.split("=", 1)[1]
    return None


def target_platform_from_args(args: Sequence[str]) -> str | None:
    for idx, arg in enumerate(args):
        if arg.startswith("--target-platform="):
            return arg.split("=", 1)[1]
        if arg == "--target-platform" and idx + 1 < len(args):
            return args[idx + 1]
    return None


def run_run_pass_fixture(rel: Path, path: Path, exp: Expectation, options: Options) -> None:
    mode = exp.run_mode or "run"
    if mode == "run":
        cmd: list[Path | str] = [options.scoop, "run"]
        if options.opt_level is not None:
            cmd.extend(["--opt-level", options.opt_level])
        cmd.append(path)
        cmd.extend(exp.args)
        output = run_fixture_command(cmd, path, rel, exp, options)
        assert_run_output(path, rel, exp, output)
        return

    if mode == "dump-stackmaps":
        with tempfile.TemporaryDirectory(prefix="scoop_fixture_dump_stackmaps_") as tmp:
            exe = Path(tmp) / default_exe_name()
            build_output = run_command(
                [options.scoop, "build", path, "-o", exe],
                env=env_for_expectation(path, exp),
            )
            if build_output.returncode != 0:
                raise command_failure("scoop build", path, build_output)
            output = run_fixture_command(
                [options.scoop, "dump-stackmaps", "--verify-roots", exe],
                path,
                rel,
                exp,
                options,
            )
            assert_run_output(path, rel, exp, output)
            return

    raise FixtureError(
        f"fixtures phase `run-pass/{mode}` is not implemented (fixture: {rel})",
        code="scoop::fixtures::unimplemented_phase",
    )


def run_run_pass_cone_case(case_dir: Path, options: Options, stdout: list[str]) -> None:
    del stdout
    main = case_dir / "src" / "main.scoop"
    exp = parse_expectations(main)
    rel = safe_relative(case_dir, case_dir.parent)
    if exp.ignore_until_fix is not None:
        return

    def action() -> None:
        build_root = case_dir / "build"
        if build_root.exists():
            shutil.rmtree(build_root)
        try:
            if exp.expect == "fail":
                cmd: list[Path | str] = [options.scoop, "build", case_dir]
                if options.opt_level is not None:
                    cmd.extend(["--opt-level", options.opt_level])
                output = run_command(cmd, env=env_for_expectation(main, exp))
                if output.returncode != 0:
                    raise located_project_failure(
                        "scoop build",
                        case_dir,
                        main,
                        exp,
                        output,
                        options,
                        phases=("typecheck", "parse"),
                    )
                return

            cmd = [options.scoop, "run"]
            if options.opt_level is not None and not args_have_opt_level(exp.args):
                cmd.extend(["--opt-level", options.opt_level])
            cmd.extend(exp.args)
            output = run_fixture_command(
                cmd,
                main,
                rel,
                exp,
                options,
                cwd=case_dir,
            )
            assert_run_output(main, rel, exp, output)
            profile = "release" if "--release" in exp.args else "debug"
            exe = case_dir / "build" / profile / executable_name_for_cone(case_dir)
            if not exe.is_file():
                raise FixtureError(
                    f"run_pass_cone did not produce executable: {exe} (fixture: {rel})",
                    code="scoop::fixtures::run_pass_cone_missing_exe",
                )
        finally:
            if build_root.exists():
                shutil.rmtree(build_root)

    assert_expectation(main, exp, action)


def args_have_opt_level(args: Sequence[str]) -> bool:
    return any(
        arg == "--opt-level"
        or arg == "--opt_level"
        or arg == "-O"
        or arg.startswith("-O")
        or arg.startswith("--opt-level=")
        for arg in args
    )


def executable_name_for_cone(case_dir: Path) -> str:
    text = (case_dir / "Cone.toml").read_text()
    match = re.search(r"(?m)^\s*name\s*=\s*\"([^\"]+)\"", text)
    name = match.group(1) if match else case_dir.name
    suffix = ".exe" if os.name == "nt" else ""
    return f"{name}{suffix}"


def default_exe_name() -> str:
    return "a.exe" if os.name == "nt" else "a.out"


def run_fixture_command(
    cmd: Sequence[Path | str],
    fixture_path: Path,
    rel: Path,
    exp: Expectation,
    options: Options,
    cwd: Path | None = None,
) -> CommandOutput:
    env = env_for_expectation(fixture_path, exp)
    env.update(run_pass_env(options))
    for key, value in exp.env:
        env[key] = value
    input_bytes = None
    if exp.run_stdin is not None:
        input_bytes = (fixture_path.parent / exp.run_stdin).read_bytes()
    output = run_command(
        cmd,
        env=env,
        cwd=cwd,
        input_bytes=input_bytes,
        timeout_ms=exp.timeout_ms,
    )
    if output.timed_out:
        raise FixtureError(
            f"run-pass command timed out: {exp.timeout_ms}ms (fixture: {rel})",
            code="scoop::fixtures::run_exec_timeout",
            stdout=output.stdout,
            stderr=output.stderr,
        )
    if exp.expect_exit is not None:
        if output.returncode < 0:
            raise FixtureError(
                f"run-pass command was terminated by signal: {-output.returncode} (fixture: {rel})",
                code="scoop::fixtures::run_exec_signaled",
                stdout=output.stdout,
                stderr=output.stderr,
            )
        if output.returncode != exp.expect_exit:
            raise FixtureError(
                f"run-pass exit code mismatch: expected {exp.expect_exit}, got {output.returncode} "
                f"(fixture: {rel})",
                code="scoop::fixtures::run_exit_code_mismatch",
                stdout=output.stdout,
                stderr=output.stderr,
            )
    elif output.returncode != 0:
        code = "scoop::fixtures::run_exec_signaled" if output.returncode < 0 else "scoop::fixtures::run_exec_nonzero_exit"
        raise FixtureError(
            f"run-pass command exited non-zero: {output.returncode} (fixture: {rel})",
            code=code,
            stdout=output.stdout,
            stderr=output.stderr,
        )
    return output


def run_pass_env(options: Options) -> dict[str, str]:
    env: dict[str, str] = {}
    if options.gc_stress:
        env["SCOOP_GC_STRESS"] = "1"
    if options.gc_move:
        env["SCOOP_GC_MOVE"] = "1"
    if options.threads is not None:
        value = str(options.threads)
        env["SCOOP_GC_IMMIX_PARALLEL_MARK"] = value
        env["SCOOP_GC_IMMIX_PARALLEL_SWEEP"] = value
    return env


def assert_run_output(path: Path, rel: Path, exp: Expectation, output: CommandOutput) -> None:
    stdout = normalize_newlines(output.stdout)
    stderr = normalize_newlines(output.stderr)
    if exp.run_stackmaps_records_gt is not None:
        actual = parse_stackmap_records(stdout)
        if actual is None:
            raise FixtureError(
                f"dump-stackmaps output is missing records field (fixture: {rel})",
                code="scoop::fixtures::run_stackmaps_missing_records",
            )
        if actual <= exp.run_stackmaps_records_gt:
            raise FixtureError(
                f"dump-stackmaps records mismatch: expected > {exp.run_stackmaps_records_gt}, "
                f"got {actual} (fixture: {rel})",
                code="scoop::fixtures::run_stackmaps_records_not_gt",
            )
    if exp.run_stdout is not None:
        golden = path.parent / exp.run_stdout
        compare_text(
            normalize_newlines(golden.read_text()),
            stdout,
            golden,
            path,
            "stdout",
            code="scoop::fixtures::run_stdout_mismatch",
        )
    if exp.run_stdout_contains is not None and normalize_newlines(exp.run_stdout_contains) not in stdout:
        raise FixtureError(
            f"stdout did not contain expected substring: {exp.run_stdout_contains} (fixture: {rel})",
            code="scoop::fixtures::run_stdout_missing_substring",
        )
    if exp.run_stderr is not None:
        golden = path.parent / exp.run_stderr
        compare_text(
            normalize_newlines(golden.read_text()),
            stderr,
            golden,
            path,
            "stderr",
            code="scoop::fixtures::run_stderr_mismatch",
        )
    if exp.run_stderr_contains is not None and normalize_newlines(exp.run_stderr_contains) not in stderr:
        raise FixtureError(
            f"stderr did not contain expected substring: {exp.run_stderr_contains} (fixture: {rel})",
            code="scoop::fixtures::run_stderr_missing_substring",
        )


def parse_stackmap_records(stdout: str) -> int | None:
    for line in stdout.splitlines():
        if line.lstrip().startswith("records:"):
            try:
                return int(line.split(":", 1)[1].strip())
            except ValueError:
                return None
    return None


def run_command(
    cmd: Sequence[Path | str],
    *,
    env: dict[str, str] | None = None,
    cwd: Path | None = None,
    input_bytes: bytes | None = None,
    timeout_ms: int | None = None,
) -> CommandOutput:
    full_env = os.environ.copy()
    if env:
        full_env.update(env)
    argv = [str(part) for part in cmd]
    start_new_session = timeout_ms is not None and os.name != "nt"
    child = subprocess.Popen(
        argv,
        cwd=str(cwd) if cwd is not None else str(REPO_ROOT),
        env=full_env,
        stdin=subprocess.PIPE if input_bytes is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=start_new_session,
    )
    try:
        stdout_bytes, stderr_bytes = child.communicate(
            input=input_bytes,
            timeout=(timeout_ms / 1000.0) if timeout_ms is not None else None,
        )
        timed_out = False
    except subprocess.TimeoutExpired:
        timed_out = True
        if os.name != "nt":
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        else:
            child.kill()
        stdout_bytes, stderr_bytes = child.communicate()
    return CommandOutput(
        returncode=child.returncode,
        stdout=stdout_bytes.decode("utf-8", "replace"),
        stderr=stderr_bytes.decode("utf-8", "replace"),
        timed_out=timed_out,
    )


def env_for_expectation(path: Path, exp: Expectation) -> dict[str, str]:
    return env_for_target(path, exp)


def env_for_target(target: Path, exp: Expectation) -> dict[str, str]:
    env: dict[str, str] = {}
    overlay = fixture_sysroot_overlay_dir(target)
    if overlay is not None and overlay.is_dir():
        env[SYSROOT_OVERLAY_ENV] = str(overlay)
    if exp.sysroot_deps:
        env[SYSROOT_DEPS_ENV] = ",".join(exp.sysroot_deps)
    return env


def fixture_sysroot_overlay_dir(target: Path) -> Path | None:
    parent = target.parent
    stem = target.stem if target.is_file() else target.name
    if not stem:
        return None
    return parent / f"{stem}.sysroot"


def command_failure(label: str, fixture: Path, output: CommandOutput) -> FixtureError:
    stderr = output.stderr
    code = diagnostic_code_from_text(stderr)
    message = (
        f"{label} subprocess failed (fixture: {fixture}, status={output.returncode})\n"
        f"--- stdout ---\n{output.stdout}\n--- stderr ---\n{output.stderr}"
    )
    return FixtureError(message, code=code, stdout=output.stdout, stderr=output.stderr)


def located_frontend_failure(
    label: str,
    fixture: Path,
    exp: Expectation,
    original: CommandOutput,
    options: Options,
    *,
    phases: Sequence[str],
) -> FixtureError:
    original_error = command_failure(label, fixture, original)
    if exp.error_at is None:
        return original_error
    for phase in phases:
        cmd: list[Path | str] = [
            options.scoopc,
            "check-source",
            "--phase",
            phase,
            "--input",
            fixture,
        ]
        target_platform = target_platform_from_args(exp.args)
        if target_platform is not None:
            cmd.extend(["--target-platform", target_platform])
        output = run_command(cmd, env=env_for_expectation(fixture, exp))
        if output.returncode == 0:
            continue
        candidate = command_failure(f"scoopc check-source --phase {phase}", fixture, output)
        if exp.error_code is None or candidate.code == exp.error_code:
            return candidate
    return original_error


def located_project_failure(
    label: str,
    project_root: Path,
    expectation_file: Path,
    exp: Expectation,
    original: CommandOutput,
    options: Options,
    *,
    phases: Sequence[str],
) -> FixtureError:
    original_error = command_failure(label, expectation_file, original)
    if exp.error_at is None:
        return original_error
    for phase in phases:
        output = run_command(
            [
                options.scoopc,
                "check-source",
                "--phase",
                phase,
                "--input",
                project_root,
            ],
            env=env_for_expectation(expectation_file, exp),
        )
        if output.returncode == 0:
            continue
        candidate = command_failure(
            f"scoopc check-source --phase {phase}", expectation_file, output
        )
        if exp.error_code is None or candidate.code == exp.error_code:
            return candidate
    return original_error


def diagnostic_code_from_text(text: str) -> str | None:
    for token in re.findall(r"[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)+", text):
        return token
    return None


def assert_diagnostic_matches(path: Path, exp: Expectation, err: FixtureError) -> None:
    if exp.error_code is not None and err.code != exp.error_code:
        raise FixtureError(
            f"error code mismatch: expected {exp.error_code!r}, got {err.code!r}",
            stdout=err.stdout,
            stderr=err.stderr,
        )
    diagnostic_text = f"{err.message}\n{err.stderr}"
    if exp.error_at is not None and not diagnostic_mentions_line_col(
        diagnostic_text, *exp.error_at
    ):
        offset = diagnostic_span_offset(diagnostic_text)
        if offset is None or offset_to_line_col(path, offset) != exp.error_at:
            raise FixtureError(
                f"error location mismatch: expected {exp.error_at[0]}:{exp.error_at[1]}",
                stdout=err.stdout,
                stderr=err.stderr,
            )
    for expected in exp.error_contains:
        if expected not in diagnostic_text and normalize_diagnostic_text(
            expected
        ) not in normalize_diagnostic_text(diagnostic_text):
            raise FixtureError(
                f"error message mismatch: expected to contain {expected!r}",
                stdout=err.stdout,
                stderr=err.stderr,
            )
    for forbidden in exp.error_not_contains:
        if forbidden in diagnostic_text:
            raise FixtureError(
                f"error message mismatch: expected not to contain {forbidden!r}",
                stdout=err.stdout,
                stderr=err.stderr,
            )
    del path


def diagnostic_mentions_line_col(text: str, line: int, col: int) -> bool:
    patterns = [
        rf"(?<!\d){line}:{col}(?!\d)",
        rf":{line}:{col}\b",
        rf"\bline\s+{line}\b.*\bcol(?:umn)?\s+{col}\b",
    ]
    return any(re.search(pattern, text, flags=re.IGNORECASE | re.DOTALL) for pattern in patterns)


def diagnostic_span_offset(text: str) -> int | None:
    match = re.search(r"\bspan-offset:\s*(\d+)\b", text)
    if match is None:
        return None
    return int(match.group(1))


def offset_to_line_col(path: Path, byte_offset: int) -> tuple[int, int] | None:
    data = path.read_bytes()
    if byte_offset > len(data):
        return None
    prefix = data[:byte_offset].decode("utf-8", "replace")
    line = prefix.count("\n") + 1
    last_newline = prefix.rfind("\n")
    col_text = prefix if last_newline < 0 else prefix[last_newline + 1 :]
    return line, len(col_text) + 1


def normalize_diagnostic_text(text: str) -> str:
    return "".join(
        part
        for part in re.split(r"\s+", re.sub(r"[│╰─▶×]", "", text))
        if part
    )


def compare_text(
    expected: str,
    actual: str,
    golden_path: Path,
    fixture_path: Path,
    label: str,
    *,
    code: str | None = None,
) -> None:
    if expected == actual:
        return
    diff = "".join(
        difflib.unified_diff(
            expected.splitlines(keepends=True),
            actual.splitlines(keepends=True),
            fromfile=str(golden_path),
            tofile=f"{fixture_path}.actual",
        )
    )
    raise FixtureError(
        f"{label} did not match golden: {golden_path} (fixture: {fixture_path})\n{diff}",
        code=code,
    )


def normalize_newlines(text: str) -> str:
    return text.replace("\r\n", "\n")


def safe_relative(path: Path, root: Path) -> Path:
    try:
        return path.relative_to(root)
    except ValueError:
        return path


def raise_fixture_error(message: str, *, code: str | None = None) -> None:
    raise FixtureError(message, code=code)


def parse_expectations(path: Path) -> Expectation:
    text = path.read_text()
    expect = "pass"
    error_contains: list[str] = []
    error_not_contains: list[str] = []
    error_code = None
    error_at = None
    args: list[str] = []
    env: list[tuple[str, str]] = []
    sysroot_deps: list[str] = []
    ast_golden = None
    build_llvm_contains: list[str] = []
    build_llvm_regex: list[str] = []
    build_llvm_not_contains: list[str] = []
    run_stdout = None
    run_stderr = None
    run_stdin = None
    run_mode = None
    run_stdout_contains = None
    run_stderr_contains = None
    run_stackmaps_records_gt = None
    expect_exit = None
    timeout_ms = None
    ignore_until_fix = None

    for directive in fixture_directives(text):
        if directive.startswith("EXPECT:"):
            value = directive[len("EXPECT:") :].strip()
            if value in ("pass", "ok"):
                expect = "pass"
            elif value == "fail":
                expect = "fail"
        elif directive.startswith("EXPECT-ERROR:"):
            value = directive[len("EXPECT-ERROR:") :].strip()
            if value:
                error_contains.append(value)
        elif directive.startswith("EXPECT-NOT-ERROR:"):
            value = directive[len("EXPECT-NOT-ERROR:") :].strip()
            if value:
                error_not_contains.append(value)
        elif directive.startswith("EXPECT-NOT-ERROR-TERMS:"):
            error_not_contains.extend(
                parse_comma_separated_terms(
                    directive[len("EXPECT-NOT-ERROR-TERMS:") :].strip()
                )
            )
        elif directive.startswith("EXPECT-ERROR-CODE:"):
            error_code = directive[len("EXPECT-ERROR-CODE:") :].strip()
        elif directive.startswith("EXPECT-ERROR-AT:"):
            error_at = parse_line_col(directive[len("EXPECT-ERROR-AT:") :].strip())
        elif directive.startswith("ARGS:"):
            args.extend(directive[len("ARGS:") :].strip().split())
        elif directive.startswith("ENV:"):
            env.extend(parse_env_pairs(directive[len("ENV:") :].strip()))
        elif directive.startswith("SYSROOT-DEPS:"):
            sysroot_deps.extend(parse_sysroot_deps(directive[len("SYSROOT-DEPS:") :].strip()))
        elif directive.startswith("EXPECT-AST:"):
            ast_golden = directive[len("EXPECT-AST:") :].strip()
        elif directive.startswith("BUILD-LLVM-CONTAINS:"):
            value = directive[len("BUILD-LLVM-CONTAINS:") :].strip()
            if value:
                build_llvm_contains.append(value)
        elif directive.startswith("BUILD-LLVM-REGEX:"):
            value = directive[len("BUILD-LLVM-REGEX:") :].strip()
            if value:
                build_llvm_regex.append(value)
        elif directive.startswith("BUILD-LLVM-NOT-CONTAINS:"):
            value = directive[len("BUILD-LLVM-NOT-CONTAINS:") :].strip()
            if value:
                build_llvm_not_contains.append(value)
        elif directive.startswith("RUN-STDOUT:"):
            run_stdout = directive[len("RUN-STDOUT:") :].strip()
        elif directive.startswith("RUN-STDERR:"):
            run_stderr = directive[len("RUN-STDERR:") :].strip()
        elif directive.startswith("RUN-STDIN:"):
            run_stdin = directive[len("RUN-STDIN:") :].strip()
        elif directive.startswith("RUN-MODE:"):
            run_mode = directive[len("RUN-MODE:") :].strip()
        elif directive.startswith("RUN-STDOUT-CONTAINS:"):
            run_stdout_contains = directive[len("RUN-STDOUT-CONTAINS:") :].strip()
        elif directive.startswith("RUN-STDERR-CONTAINS:"):
            run_stderr_contains = directive[len("RUN-STDERR-CONTAINS:") :].strip()
        elif directive.startswith("RUN-STACKMAPS-RECORDS-GT:"):
            run_stackmaps_records_gt = parse_optional_int(
                directive[len("RUN-STACKMAPS-RECORDS-GT:") :].strip()
            )
        elif directive.startswith("EXPECT-EXIT:"):
            expect_exit = parse_optional_int(directive[len("EXPECT-EXIT:") :].strip())
        elif directive.startswith("TIMEOUT:"):
            timeout_ms = parse_optional_int(directive[len("TIMEOUT:") :].strip())
        elif directive.startswith("IGNORE-UNTIL-FIX:") or directive.startswith(
            "ignore-until-fix:"
        ):
            _, value = directive.split(":", 1)
            value = value.strip()
            if value:
                ignore_until_fix = value

    return Expectation(
        expect=expect,
        error_contains=tuple(error_contains),
        error_not_contains=tuple(error_not_contains),
        error_code=error_code,
        error_at=error_at,
        args=tuple(args),
        env=tuple(env),
        sysroot_deps=tuple(sysroot_deps),
        ast_golden=ast_golden,
        build_llvm_contains=tuple(build_llvm_contains),
        build_llvm_regex=tuple(build_llvm_regex),
        build_llvm_not_contains=tuple(build_llvm_not_contains),
        run_stdout=run_stdout,
        run_stderr=run_stderr,
        run_stdin=run_stdin,
        run_mode=run_mode,
        run_stdout_contains=run_stdout_contains,
        run_stderr_contains=run_stderr_contains,
        run_stackmaps_records_gt=run_stackmaps_records_gt,
        expect_exit=expect_exit,
        timeout_ms=timeout_ms,
        ignore_until_fix=ignore_until_fix,
    )


def fixture_directives(text: str) -> list[str]:
    directives: list[str] = []
    for raw_line in text.splitlines():
        trimmed = raw_line.lstrip()
        if not trimmed.startswith("//"):
            continue
        directive = trimmed[2:].strip()
        if is_fixture_directive(directive):
            directives.append(directive)
    return directives


def is_fixture_directive(directive: str) -> bool:
    prefixes = (
        "EXPECT:",
        "EXPECT-ERROR:",
        "EXPECT-NOT-ERROR:",
        "EXPECT-NOT-ERROR-TERMS:",
        "EXPECT-ERROR-CODE:",
        "EXPECT-ERROR-AT:",
        "ARGS:",
        "ENV:",
        "SYSROOT-DEPS:",
        "EXPECT-AST:",
        "BUILD-LLVM-CONTAINS:",
        "BUILD-LLVM-REGEX:",
        "BUILD-LLVM-NOT-CONTAINS:",
        "RUN-STDOUT:",
        "RUN-STDERR:",
        "RUN-STDIN:",
        "RUN-MODE:",
        "RUN-STDOUT-CONTAINS:",
        "RUN-STDERR-CONTAINS:",
        "RUN-STACKMAPS-RECORDS-GT:",
        "EXPECT-EXIT:",
        "TIMEOUT:",
        "IGNORE-UNTIL-FIX:",
        "ignore-until-fix:",
    )
    return directive.startswith(prefixes)


def parse_comma_separated_terms(value: str) -> list[str]:
    return [part.strip() for part in value.split(",") if part.strip()]


def parse_line_col(value: str) -> tuple[int, int] | None:
    parts = value.split(":")
    if len(parts) != 2:
        return None
    try:
        return int(parts[0].strip()), int(parts[1].strip())
    except ValueError:
        return None


def parse_optional_int(value: str) -> int | None:
    try:
        return int(value)
    except ValueError:
        return None


def parse_env_pairs(value: str) -> Iterable[tuple[str, str]]:
    for token in value.split():
        if "=" not in token:
            continue
        key, env_value = token.split("=", 1)
        key = key.strip()
        if key:
            yield key, env_value


def parse_sysroot_deps(value: str) -> Iterable[str]:
    for part in re.split(r"[\s,]+", value):
        part = part.strip()
        if part:
            yield part


def format_check_count(checks: int) -> str:
    return "1 check" if checks == 1 else f"{checks} checks"


def emit_stdout(stdout: str) -> None:
    if stdout:
        print(stdout, end="" if stdout.endswith("\n") else "\n")


def emit_stderr(stderr: str) -> None:
    if stderr:
        print(stderr, file=sys.stderr, end="" if stderr.endswith("\n") else "\n")


def print_worker_failure(
    completed: int, total_targets: int, target: Target, failure: WorkerFailure
) -> None:
    print(f"[{completed}/{total_targets}] FAIL {target.display} ({failure.message})")
    emit_stdout(failure.stdout)
    emit_stderr(failure.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
