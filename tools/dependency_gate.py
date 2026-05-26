#!/usr/bin/env python3
"""Check pipeline crate dependency boundaries and source residual guards."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Sequence


BASE_CRATES = (
    "scoopc_span",
    "scoopc_source",
    "scoopc_types",
    "scoopc_ids",
    "scoop_project_model",
)

FACT_CRATES = (
    "scoopc_hir_facts",
    "scoopc_mir_facts",
    "scoopc_effect_facts",
    "scoopc_lir_facts",
)

BASE_ONLY_STAGE_CRATES = ("scoopc_ast",)
HIR_STAGE_CRATES = ("scoopc_hir",)
MIR_STAGE_CRATES = ("scoopc_mir",)
EFFECT_FACTS_STAGE_CRATES = ("scoopc_effect_facts_stage",)
LIR_STAGE_CRATES = ("scoopc_lir",)
CODEGEN_STAGE_CRATES = ("scoopc_codegen_llvm",)
CONE_CRATES = ("scoopc_cone",)
LINKER_CRATES = ("scoopld",)
DRIVER_CRATES = ("scoop",)

FORBIDDEN_WORKSPACE_CRATES = (
    "scoop",
    "scoopc",
    "scoop_runtime",
    "scoop_tools",
    "scoopld",
    "scoopc_ast",
    "scoopc_ast_facts",
    "scoopc_hir",
    "scoopc_hir_facts",
    "scoopc_mir",
    "scoopc_mir_facts",
    "scoopc_effect_facts_stage",
    "scoopc_effect_facts",
    "scoopc_lir",
    "scoopc_lir_facts",
    "scoopc_codegen_llvm",
    "scoopc_codegen_c",
    "scoopc_cone",
)


class DependencyGateError(Exception):
    """Raised when the dependency gate cannot complete."""


class CrateKind(Enum):
    BASE = "base"
    FACT = "fact"
    BASE_ONLY_STAGE = "base-only-stage"
    HIR_STAGE = "hir-stage"
    MIR_STAGE = "mir-stage"
    EFFECT_FACTS_STAGE = "effect-facts-stage"
    LIR_STAGE = "lir-stage"
    CODEGEN_STAGE = "codegen-stage"
    CONE = "cone"
    LINKER = "linker"
    DRIVER = "driver"


@dataclass(frozen=True)
class Violation:
    dependency: str
    reason: str


@dataclass(frozen=True)
class CrateCheck:
    crate_name: str
    kind: CrateKind
    dependency_names: frozenset[str]
    violations: tuple[Violation, ...]

    def base_dependencies(self) -> list[str]:
        return [
            name
            for name in sorted(self.dependency_names)
            if name != self.crate_name and name in BASE_CRATES
        ]


@dataclass(frozen=True)
class ForbiddenSourcePattern:
    pattern: str
    reason: str


@dataclass(frozen=True)
class SourceBoundaryRule:
    label: str
    kind_label: str
    path: str
    forbidden: tuple[ForbiddenSourcePattern, ...]


@dataclass(frozen=True)
class SourceTreeBoundaryRule:
    label: str
    kind_label: str
    root: str
    exclude_path_fragments: tuple[str, ...]
    forbidden: tuple[ForbiddenSourcePattern, ...]


@dataclass(frozen=True)
class SourceBoundaryViolation:
    pattern: str
    reason: str
    line: int


@dataclass(frozen=True)
class SourceBoundaryCheck:
    label: str
    kind_label: str
    path: Path
    violations: tuple[SourceBoundaryViolation, ...]


@dataclass(frozen=True)
class Report:
    checks: tuple[CrateCheck, ...]
    source_checks: tuple[SourceBoundaryCheck, ...]

    def render(self) -> str:
        if self.has_violations():
            return self.render_failures()

        lines = [
            "dependency gate: ok "
            f"(checked {len(self.checks)} pipeline crates: "
            f"{self.count_crate_kind(CrateKind.BASE)} base, "
            f"{self.count_crate_kind(CrateKind.FACT)} fact, "
            f"{self.count_crate_kind(CrateKind.BASE_ONLY_STAGE)} base-only stage, "
            f"{self.count_crate_kind(CrateKind.HIR_STAGE)} HIR stage, "
            f"{self.count_crate_kind(CrateKind.MIR_STAGE)} MIR stage, "
            f"{self.count_crate_kind(CrateKind.EFFECT_FACTS_STAGE)} effect-facts stage, "
            f"{self.count_crate_kind(CrateKind.LIR_STAGE)} LIR stage, "
            f"{self.count_crate_kind(CrateKind.CODEGEN_STAGE)} codegen stage, "
            f"{self.count_crate_kind(CrateKind.CONE)} cone, "
            f"{self.count_crate_kind(CrateKind.LINKER)} linker, "
            f"{self.count_crate_kind(CrateKind.DRIVER)} driver; "
            f"{len(self.source_checks)} source boundary files)"
        ]
        for check in self.checks:
            base_dependencies = check.base_dependencies()
            rendered = ", ".join(base_dependencies) if base_dependencies else "none"
            lines.append(
                f"- {check.crate_name} [{check.kind.value}]: base deps [{rendered}]"
            )
        for check in self.source_checks:
            lines.append(
                f"- {check.label} [{check.kind_label}]: "
                f"no forbidden residuals ({check.path.as_posix()})"
            )
        return "\n".join(lines)

    def has_violations(self) -> bool:
        return any(check.violations for check in self.checks) or any(
            check.violations for check in self.source_checks
        )

    def count_crate_kind(self, kind: CrateKind) -> int:
        return sum(1 for check in self.checks if check.kind == kind)

    def render_failures(self) -> str:
        lines = ["dependency gate: failed"]
        for check in self.checks:
            for violation in check.violations:
                lines.append(
                    f"- {check.crate_name} depends on {violation.dependency}: "
                    f"{violation.reason}"
                )
        for check in self.source_checks:
            for violation in check.violations:
                lines.append(
                    f"- {check.label} {check.path.as_posix()}:{violation.line} "
                    f"contains `{violation.pattern}`: {violation.reason}"
                )
        return "\n".join(lines)


@dataclass(frozen=True)
class MetadataGraph:
    package_id_by_name: dict[str, str]
    package_name_by_id: dict[str, str]
    normal_deps_by_id: dict[str, frozenset[str]]


def main(argv: Sequence[str] | None = None) -> int:
    parse_args(argv)
    try:
        report = run()
    except DependencyGateError as exc:
        print(f"dependency gate: error: {exc}", file=sys.stderr)
        return 1

    print(report.render(), file=sys.stderr)
    return 1 if report.has_violations() else 0


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check Scoop pipeline crate dependency boundaries."
    )
    return parser.parse_args(argv)


def run() -> Report:
    workspace_root = Path(__file__).resolve().parent.parent
    graph = cargo_metadata_graph(workspace_root)
    checks: list[CrateCheck] = []

    for crate_name in BASE_CRATES:
        checks.append(crate_check(graph, crate_name, CrateKind.BASE, direct=False))
    for crate_name in FACT_CRATES:
        checks.append(crate_check(graph, crate_name, CrateKind.FACT, direct=False))
    for crate_name in BASE_ONLY_STAGE_CRATES:
        checks.append(
            crate_check(graph, crate_name, CrateKind.BASE_ONLY_STAGE, direct=False)
        )
    for crate_name in HIR_STAGE_CRATES:
        checks.append(crate_check(graph, crate_name, CrateKind.HIR_STAGE, direct=False))
    for crate_name in MIR_STAGE_CRATES:
        checks.append(crate_check(graph, crate_name, CrateKind.MIR_STAGE, direct=False))
    for crate_name in EFFECT_FACTS_STAGE_CRATES:
        checks.append(
            crate_check(graph, crate_name, CrateKind.EFFECT_FACTS_STAGE, direct=False)
        )
    for crate_name in LIR_STAGE_CRATES:
        if not crate_manifest_exists(workspace_root, crate_name):
            continue
        checks.append(crate_check(graph, crate_name, CrateKind.LIR_STAGE, direct=True))
    for crate_name in CODEGEN_STAGE_CRATES:
        checks.append(
            crate_check(graph, crate_name, CrateKind.CODEGEN_STAGE, direct=True)
        )
    for crate_name in CONE_CRATES:
        checks.append(crate_check(graph, crate_name, CrateKind.CONE, direct=True))
    for crate_name in LINKER_CRATES:
        checks.append(crate_check(graph, crate_name, CrateKind.LINKER, direct=True))
    for crate_name in DRIVER_CRATES:
        checks.append(crate_check(graph, crate_name, CrateKind.DRIVER, direct=True))

    source_checks = run_source_boundary_checks(workspace_root)
    return Report(tuple(checks), tuple(source_checks))


def crate_check(
    graph: MetadataGraph, crate_name: str, kind: CrateKind, *, direct: bool
) -> CrateCheck:
    dependency_names = direct_package_names(graph, crate_name)
    if not direct:
        dependency_names = transitive_package_names(graph, crate_name)
    violations = find_dependency_violations(crate_name, kind, dependency_names)
    return CrateCheck(
        crate_name=crate_name,
        kind=kind,
        dependency_names=frozenset(dependency_names),
        violations=tuple(violations),
    )


def cargo_metadata_graph(workspace_root: Path) -> MetadataGraph:
    command = ["cargo", "metadata", "--format-version", "1", "--quiet"]
    try:
        result = subprocess.run(
            command,
            cwd=workspace_root,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except OSError as exc:
        raise DependencyGateError("启动 cargo metadata 失败") from exc

    if result.returncode != 0:
        raise DependencyGateError(
            "cargo metadata failed: " + result.stderr.strip()
            if result.stderr.strip()
            else "cargo metadata failed"
        )

    try:
        metadata = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise DependencyGateError("cargo metadata 输出不是合法 JSON") from exc

    workspace_ids = set(metadata.get("workspace_members", []))
    package_name_by_id: dict[str, str] = {}
    package_id_by_name: dict[str, str] = {}
    for package in metadata.get("packages", []):
        package_id = package.get("id")
        name = package.get("name")
        if not isinstance(package_id, str) or not isinstance(name, str):
            continue
        package_name_by_id[package_id] = name
        if package_id in workspace_ids:
            package_id_by_name[name] = package_id

    normal_deps_by_id: dict[str, frozenset[str]] = {}
    resolve = metadata.get("resolve") or {}
    for node in resolve.get("nodes", []):
        node_id = node.get("id")
        if not isinstance(node_id, str):
            continue
        deps: set[str] = set()
        for dep in node.get("deps", []):
            dep_id = dep.get("pkg")
            if not isinstance(dep_id, str):
                continue
            dep_kinds = dep.get("dep_kinds", [])
            if any(kind.get("kind") is None for kind in dep_kinds):
                deps.add(dep_id)
        normal_deps_by_id[node_id] = frozenset(deps)

    return MetadataGraph(
        package_id_by_name=package_id_by_name,
        package_name_by_id=package_name_by_id,
        normal_deps_by_id=normal_deps_by_id,
    )


def direct_package_names(graph: MetadataGraph, crate_name: str) -> frozenset[str]:
    root_id = package_id(graph, crate_name)
    names = {crate_name}
    for dep_id in graph.normal_deps_by_id.get(root_id, frozenset()):
        dep_name = graph.package_name_by_id.get(dep_id)
        if dep_name is not None:
            names.add(dep_name)
    return frozenset(names)


def transitive_package_names(graph: MetadataGraph, crate_name: str) -> frozenset[str]:
    root_id = package_id(graph, crate_name)
    seen_ids: set[str] = set()
    stack = [root_id]
    while stack:
        current = stack.pop()
        if current in seen_ids:
            continue
        seen_ids.add(current)
        stack.extend(graph.normal_deps_by_id.get(current, frozenset()))
    return frozenset(
        graph.package_name_by_id[package_id]
        for package_id in seen_ids
        if package_id in graph.package_name_by_id
    )


def package_id(graph: MetadataGraph, crate_name: str) -> str:
    try:
        return graph.package_id_by_name[crate_name]
    except KeyError as exc:
        raise DependencyGateError(f"workspace package not found: {crate_name}") from exc


def crate_manifest_exists(workspace_root: Path, crate_name: str) -> bool:
    return (workspace_root / "crates" / crate_name / "Cargo.toml").is_file()


def run_source_boundary_checks(workspace_root: Path) -> list[SourceBoundaryCheck]:
    checks: list[SourceBoundaryCheck] = []
    for rule in source_boundary_rules():
        path = workspace_root / rule.path
        try:
            contents = path.read_text(encoding="utf-8")
        except OSError as exc:
            raise DependencyGateError(f"无法读取源码边界文件：{rule.path}") from exc
        checks.append(
            SourceBoundaryCheck(
                label=rule.label,
                kind_label=rule.kind_label,
                path=Path(rule.path),
                violations=tuple(source_boundary_violations(contents, rule.forbidden)),
            )
        )
    for rule in source_tree_boundary_rules():
        checks.extend(run_source_tree_boundary_check(workspace_root, rule))
    return checks


def run_source_tree_boundary_check(
    workspace_root: Path, rule: SourceTreeBoundaryRule
) -> list[SourceBoundaryCheck]:
    root = workspace_root / rule.root
    if not root.exists():
        return []

    checks: list[SourceBoundaryCheck] = []
    for path in sorted(root.rglob("*.rs"), key=lambda item: item.parts):
        rel_path = path.relative_to(workspace_root)
        rel_text = rel_path.as_posix()
        if source_path_is_excluded(rel_text, rule.exclude_path_fragments):
            continue
        try:
            contents = path.read_text(encoding="utf-8")
        except OSError as exc:
            raise DependencyGateError(f"无法读取源码边界文件：{rel_text}") from exc
        checks.append(
            SourceBoundaryCheck(
                label=rule.label,
                kind_label=rule.kind_label,
                path=rel_path,
                violations=tuple(source_boundary_violations(contents, rule.forbidden)),
            )
        )
    return checks


def source_path_is_excluded(path: str, fragments: Sequence[str]) -> bool:
    return any(fragment in path for fragment in fragments)


def source_boundary_violations(
    contents: str, forbidden: Sequence[ForbiddenSourcePattern]
) -> list[SourceBoundaryViolation]:
    violations: list[SourceBoundaryViolation] = []
    for line_index, line in enumerate(contents.splitlines(), start=1):
        for pattern in forbidden:
            if pattern.pattern in line:
                violations.append(
                    SourceBoundaryViolation(
                        pattern=pattern.pattern,
                        reason=pattern.reason,
                        line=line_index,
                    )
                )
    return violations


def find_dependency_violations(
    checked_crate: str, kind: CrateKind, dependency_names: frozenset[str]
) -> list[Violation]:
    allowed_base_deps = set(allowed_base_dependencies(checked_crate))
    violations: list[Violation] = []

    for dependency in sorted(dependency_names):
        dependency_name = dependency
        if dependency_name == checked_crate:
            continue

        if kind == CrateKind.FACT:
            if dependency_name in FACT_CRATES:
                violations.append(
                    Violation(
                        dependency,
                        "fact crates must not depend on other fact crates",
                    )
                )
                continue
            if dependency_name in FORBIDDEN_WORKSPACE_CRATES:
                violations.append(
                    Violation(
                        dependency,
                        "fact crates must only depend on base crates, not facade, driver/runtime/tool, stage, backend, or fact crates",
                    )
                )
            continue

        if kind == CrateKind.BASE_ONLY_STAGE:
            if dependency_name in FORBIDDEN_WORKSPACE_CRATES:
                violations.append(
                    Violation(
                        dependency,
                        "base-only stage crates must not depend on facade, driver/runtime/tool, other stage, backend, or fact crates",
                    )
                )
            continue

        if kind == CrateKind.HIR_STAGE:
            allowed_stage_inputs = ("scoopc_ast", "scoopc_hir_facts")
            if (
                dependency_name in FORBIDDEN_WORKSPACE_CRATES
                and dependency_name not in allowed_stage_inputs
            ):
                violations.append(
                    Violation(
                        dependency,
                        "HIR stage crates must depend only on base crates, scoopc_ast, and scoopc_hir_facts",
                    )
                )
            continue

        if kind == CrateKind.MIR_STAGE:
            allowed_stage_inputs = (
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_hir_facts",
                "scoopc_mir_facts",
            )
            if (
                dependency_name in FORBIDDEN_WORKSPACE_CRATES
                and dependency_name not in allowed_stage_inputs
            ):
                violations.append(
                    Violation(
                        dependency,
                        "MIR stage crates must depend only on base crates, scoopc_ast, scoopc_hir, scoopc_hir_facts, and scoopc_mir_facts",
                    )
                )
            continue

        if kind == CrateKind.EFFECT_FACTS_STAGE:
            allowed_stage_inputs = (
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_hir_facts",
                "scoopc_mir",
                "scoopc_mir_facts",
                "scoopc_effect_facts",
            )
            if (
                dependency_name in FORBIDDEN_WORKSPACE_CRATES
                and dependency_name not in allowed_stage_inputs
            ):
                violations.append(
                    Violation(
                        dependency,
                        "effect-facts stage crates must depend only on base crates, AST/HIR/MIR stage inputs, HIR/MIR facts, and effect facts",
                    )
                )
            continue

        if kind == CrateKind.LIR_STAGE:
            allowed_stage_inputs = (
                "scoopc_mir",
                "scoopc_mir_facts",
                "scoopc_effect_facts",
                "scoopc_lir_facts",
            )
            if dependency_name in (
                "scoopc",
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_hir_facts",
                "scoopc_effect_facts_stage",
                "scoopc_codegen_llvm",
                "scoopc_codegen_c",
            ):
                violations.append(
                    Violation(
                        dependency,
                        "LIR stage crates must depend directly only on base, MIR, MIR facts, effect facts, and LIR facts",
                    )
                )
                continue
            if (
                dependency_name in FORBIDDEN_WORKSPACE_CRATES
                and dependency_name not in allowed_stage_inputs
            ):
                violations.append(
                    Violation(
                        dependency,
                        "LIR stage crates must not depend on facade, driver/runtime/tool, frontend stages, effect builder stage, backend, or non-LIR input facts",
                    )
                )
            continue

        if kind == CrateKind.CODEGEN_STAGE:
            allowed_stage_inputs = ("scoopc_lir", "scoopc_lir_facts")
            if (
                dependency_name == "scoopc_ast"
                or dependency_name == "scoopc_hir"
                or dependency_name == "scoopc_mir"
                or dependency_name == "scoopc_effect_facts_stage"
                or dependency_name == "scoopc_codegen_c"
                or dependency_name == "scoopc_cone"
                or (
                    dependency_name in FACT_CRATES
                    and dependency_name not in allowed_stage_inputs
                )
            ):
                violations.append(
                    Violation(
                        dependency,
                        "LLVM codegen crates must depend only on base, scoopc_lir, and scoopc_lir_facts",
                    )
                )
                continue
            if (
                dependency_name in FORBIDDEN_WORKSPACE_CRATES
                and dependency_name not in allowed_stage_inputs
            ):
                violations.append(
                    Violation(
                        dependency,
                        "LLVM codegen crates must not depend on the scoopc facade, driver/runtime/tool, frontend stages, MIR/effect stages, or non-LIR fact crates",
                    )
                )
            continue

        if kind == CrateKind.CONE:
            if (
                dependency_name in FORBIDDEN_WORKSPACE_CRATES
                and not cone_allowed_workspace_dependency(dependency_name)
            ):
                violations.append(
                    Violation(
                        dependency,
                        "cone crates may depend on base, fact, and stage crates, but not on facade, driver/runtime/tool, codegen-C, or other cone-operation crates",
                    )
                )
            continue

        if kind == CrateKind.LINKER:
            if dependency_name == "scoopc_cone" or dependency_name in BASE_CRATES:
                continue
            if dependency_name in FORBIDDEN_WORKSPACE_CRATES:
                violations.append(
                    Violation(
                        dependency,
                        "scoopld may depend only on base/project-model/cone metadata crates, not facade, driver/runtime/tool, stage, fact, or backend crates",
                    )
                )
            continue

        if kind == CrateKind.DRIVER:
            if dependency_name == "scoop_project_model":
                continue
            if (
                dependency_name in FORBIDDEN_WORKSPACE_CRATES
                or dependency_name.startswith("scoopc")
                or dependency_name == "scoopld"
            ):
                violations.append(
                    Violation(
                        dependency,
                        "scoop facade may depend directly only on scoop_project_model; compiler/linker interaction must go through subprocess CLIs",
                    )
                )
            continue

        if dependency_name in FORBIDDEN_WORKSPACE_CRATES:
            violations.append(
                Violation(
                    dependency,
                    "base crates must not depend on facade, driver/runtime/tool, stage, or fact crates",
                )
            )
            continue

        if dependency_name in BASE_CRATES and dependency_name not in allowed_base_deps:
            violations.append(
                Violation(
                    dependency,
                    "dependency is outside the allowed base-crate direction",
                )
            )

    return violations


def cone_allowed_workspace_dependency(dependency_name: str) -> bool:
    return (
        dependency_name in BASE_CRATES
        or dependency_name in FACT_CRATES
        or dependency_name in BASE_ONLY_STAGE_CRATES
        or dependency_name in HIR_STAGE_CRATES
        or dependency_name in MIR_STAGE_CRATES
        or dependency_name in EFFECT_FACTS_STAGE_CRATES
        or dependency_name in LIR_STAGE_CRATES
        or dependency_name in CODEGEN_STAGE_CRATES
    )


def allowed_base_dependencies(base: str) -> tuple[str, ...]:
    return {
        "scoopc_span": (),
        "scoopc_source": ("scoopc_span",),
        "scoopc_types": ("scoopc_span",),
        "scoopc_ids": ("scoopc_span", "scoopc_types"),
        "scoop_project_model": (
            "scoopc_span",
            "scoopc_source",
            "scoopc_types",
            "scoopc_ids",
        ),
    }.get(base, ())


def fp(pattern: str, reason: str) -> ForbiddenSourcePattern:
    return ForbiddenSourcePattern(pattern, reason)


def source_boundary_rules() -> tuple[SourceBoundaryRule, ...]:
    return (
        SourceBoundaryRule(
            "Scoop comptime keyword token",
            "frontend-boundary",
            "crates/scoopc_ast/src/syntax/token.rs",
            (
                fp(
                    "Comptime",
                    "old comptime surface must not be preserved as a dedicated keyword token",
                ),
            ),
        ),
        SourceBoundaryRule(
            "Scoop comptime lexer surface",
            "frontend-boundary",
            "crates/scoopc_ast/src/syntax/lexer.rs",
            (
                fp(
                    '"comptime" =>',
                    "old comptime surface must lex as a normal identifier, not a dedicated keyword",
                ),
            ),
        ),
        SourceBoundaryRule(
            "Scoop comptime parser surface",
            "frontend-boundary",
            "crates/scoopc_ast/src/parser/cursor.rs",
            (
                fp(
                    "Keyword::Comptime",
                    "parser recovery must not preserve a dedicated old comptime statement surface",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM const-eval module residual",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/main/mod.rs",
            (
                fp(
                    "const_eval",
                    "backend data initializer helpers must not restore old const-evaluator naming or module boundaries",
                ),
            ),
        ),
        SourceBoundaryRule(
            "scoopc umbrella Cargo backend-private deps",
            "stage-split-boundary",
            "crates/scoopc/Cargo.toml",
            (
                fp(
                    "inkwell =",
                    "scoopc umbrella must not depend directly on backend-private inkwell",
                ),
                fp(
                    "llvm-sys =",
                    "scoopc umbrella must not depend directly on backend-private llvm-sys",
                ),
            ),
        ),
        SourceBoundaryRule(
            "scoopc LLVM facade implementation",
            "stage-split-boundary",
            "crates/scoopc/src/llvm.rs",
            (
                fp(
                    "use inkwell",
                    "scoopc::llvm must remain a facade over scoopc_codegen_llvm",
                ),
                fp(
                    "inkwell::",
                    "scoopc::llvm must not recreate backend-private LLVM implementation helpers",
                ),
                fp(
                    "llvm_sys",
                    "scoopc::llvm must not depend directly on llvm-sys",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM top-level const initializer residual",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs",
            (
                fp(
                    "const_initializer_for_top_level_var",
                    "top-level eager init must not restore the old const initializer helper path",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM stage handoff",
            "backend-boundary",
            "crates/scoopc/src/pipeline/llvm_codegen_stage.rs",
            (
                fp(
                    "use crate::effect::",
                    "LLVM stage handoff must consume LIR-owned ordinary-callee suspend contracts, not effect-stage APIs",
                ),
                fp(
                    "use crate::effect::{",
                    "LLVM stage handoff must not import effect-stage APIs through grouped imports",
                ),
                fp(
                    "crate::effect::",
                    "LLVM stage handoff must not reach into effect-stage APIs for suspend analysis",
                ),
                fp(
                    "scoopc_effect_facts_stage::effect",
                    "LLVM stage handoff must not depend on the future effect-facts stage crate for suspend analysis",
                ),
                fp(
                    "scoopc::effect",
                    "LLVM stage handoff must not recover effect-stage APIs through the scoopc facade",
                ),
                fp(
                    "EffectLoweredStageOutput",
                    "LLVM stage output must not reintroduce P5 stage-output wrapper handoff",
                ),
                fp(
                    "EffectFactsStageOutput",
                    "LLVM stage output must not carry effect-facts stage-output wrapper handoff",
                ),
                fp(
                    "hir_compat_scaffold",
                    "LLVM stage output must not restore HIR compatibility scaffold handoff",
                ),
                fp(
                    "llvm_residual_pass_view",
                    "LirStageOutput must not restore LLVM-only residual accessors",
                ),
                fp(
                    "materialized_pass_view",
                    "LLVM stage handoff must not expose materialized MIR/pass-view accessors to production codegen",
                ),
                fp(
                    "MaterializedMirPassView",
                    "LLVM stage handoff must not expose materialized MIR/pass-view types to production codegen",
                ),
                fp(
                    "LlvmSourceCallableSignature",
                    "LLVM stage handoff must not publish HIR-derived callable signatures",
                ),
                fp(
                    "LlvmCallableSignatureContract",
                    "LLVM stage handoff must not publish HIR-derived callable signature fallback contracts",
                ),
                fp(
                    "build_callable_signature_contracts",
                    "LLVM stage handoff must not rebuild callable signatures from HIR functions",
                ),
                fp(
                    "source_signatures:",
                    "LLVM stage handoff must not carry HIR-derived source signature maps",
                ),
                fp(
                    "callable_signatures:",
                    "LLVM stage handoff must not carry HIR-derived callable signature maps",
                ),
                fp(
                    "precheck_invalid_integer_literals",
                    "integer literal validation belongs to frontend/typecheck, not LLVM HIR body scans",
                ),
                fp(
                    "precheck_expr_integer_literals",
                    "LLVM stage must not traverse HIR expressions for literal validation",
                ),
                fp(
                    "precheck_when_pattern_integer_literals",
                    "LLVM stage must not traverse HIR patterns for literal validation",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM emit handoff",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/emit.rs",
            (
                fp(
                    "EffectLoweredStageOutput",
                    "emit handoff must consume LIR/LIR facts, not P5 stage-output wrappers",
                ),
                fp(
                    "EffectFactsStageOutput",
                    "emit handoff must not carry effect-facts stage-output wrappers",
                ),
                fp(
                    "LoweredHir",
                    "emit handoff must not receive HIR body/scaffold input",
                ),
                fp(
                    "HirFacts",
                    "emit handoff must not receive HIR facts as an emit input wrapper",
                ),
                fp(
                    "MaterializedMirPassView",
                    "emit handoff must not receive raw MIR/pass-view input directly",
                ),
                fp(
                    "materialized_pass_view",
                    "emit handoff must not receive raw MIR/pass-view input directly",
                ),
                fp("crate::hir", "emit handoff must not traverse HIR APIs"),
                fp("crate::mir", "emit handoff must not traverse raw MIR APIs"),
            ),
        ),
        SourceBoundaryRule(
            "LLVM reachability",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/reachability.rs",
            (
                fp("use crate::hir", "backend reachability must not scan HIR"),
                fp("use crate::mir", "backend reachability must not scan raw MIR"),
                fp(
                    "MaterializedMirPassView",
                    "backend reachability must use LIR facts instead of MIR pass views",
                ),
                fp(
                    "try_devirtualize",
                    "ordinary dispatch devirtualization is MIR-owned, not backend reachability-owned",
                ),
                fp(
                    "devirtualization_facts",
                    "backend reachability must not consume MIR devirtualization facts directly",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM codegen context",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs",
            (
                fp(
                    "materialized_pass_view",
                    "LLVM production codegen context must not carry raw MIR/pass-view accessors",
                ),
                fp(
                    "MaterializedMirPassView",
                    "LLVM production codegen context must not carry raw MIR/pass-view types",
                ),
                fp(
                    "HirFacts",
                    "LLVM production codegen context must not save full HIR facts",
                ),
                fp(
                    "fun_index",
                    "LLVM production codegen context must not save HIR function indexes",
                ),
                fp(
                    "callable_signatures",
                    "LLVM production codegen context must not save HIR-derived callable signature maps",
                ),
                fp(
                    "LlvmCallableSignatureContract",
                    "LLVM production codegen context must not consume HIR-derived callable signature fallback contracts",
                ),
                fp(
                    "MaterializedMir",
                    "LLVM production codegen context must not save full materialized MIR wrappers",
                ),
                fp(
                    "MaterializedEffectFacts",
                    "LLVM production codegen context must not save full materialized effect-facts wrappers",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM legacy HIR function declarations",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/main/declare.rs",
            (
                fp(
                    "declare_top_level_fun(",
                    "top-level function declarations must be driven by LIR callable/signature facts, not HIR FunDecl helpers",
                ),
                fp(
                    "declare_top_level_fun_with_symbol",
                    "top-level function declarations must not reintroduce HIR FunDecl declaration helpers",
                ),
                fp(
                    "declare_top_level_fun_with_signature_override",
                    "signature overrides must not recover exported ABI declarations through HIR FunDecl helpers",
                ),
                fp(
                    "declare_top_level_fun_callee_resume_entry",
                    "callee resume declarations must use LIR/source-owned callable contracts instead of HIR FunDecl helpers",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM legacy HIR function emission",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/main/function.rs",
            (
                fp(
                    "codegen_top_level_fun(",
                    "top-level body emission must use LIR-owned source body contracts instead of HIR FunDecl entry helpers",
                ),
                fp(
                    "build_fun_callee_suspend_plan(",
                    "ordinary callee suspendability must be published through LIR/effect facts, not rebuilt from HIR FunDecl helpers",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM legacy HIR callable identity",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs",
            (
                fp(
                    "exported_abi_symbol_for_hir_fun",
                    "exported ABI identity must come from LIR callable symbol facts, not HIR FunDecl helpers",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM direct call lowering",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/call/lowering.rs",
            (
                fp(
                    "materialized_pass_view",
                    "direct call lowering must use LIR callable signature facts instead of MIR pass-view fallback",
                ),
                fp(
                    "MaterializedMirPassView",
                    "direct call lowering must not depend on MIR pass-view types",
                ),
                fp(
                    "materialized_owner_hir_fun_for_callable",
                    "direct call signature lookup must not recover HIR owners from materialized MIR fallback",
                ),
                fp(
                    "fun_index.get",
                    "direct/dispatch call lowering must use LIR callable signature facts instead of HIR fun_index fallback",
                ),
                fp(
                    "self.fun_index",
                    "direct/dispatch call lowering must not retain HIR function indexes",
                ),
                fp(
                    "dispatch_call_sites",
                    "dispatch kind must come from LIR facts or the explicit LLVM dispatch narrow contract",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM callable ABI facts",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/call/abi.rs",
            (
                fp(
                    "self.source_signatures.get",
                    "callable ABI signature lookup must use LIR source signature facts instead of HIR-derived maps",
                ),
                fp(
                    "fun_index.get",
                    "callable ABI lookup must not recover signatures from HIR fun_index",
                ),
                fp(
                    "callable_signatures.get",
                    "callable ABI lookup must fail fast instead of using HIR-derived signature fallback maps",
                ),
                fp(
                    "self.callable_signatures",
                    "callable ABI lookup must not consume HIR-derived signature fallback maps",
                ),
                fp(
                    "LlvmCallableSignatureContract",
                    "callable ABI lookup must not depend on HIR-derived LLVM signature contracts",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM source callable lookup",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs",
            (
                fp(
                    "materialized_pass_view",
                    "source body emission must use LIR-owned source callable contracts instead of MIR pass views",
                ),
                fp(
                    "MaterializedMirPassView",
                    "source body emission must not depend on MIR pass-view types",
                ),
                fp(
                    "materialized_mir_callable",
                    "source body emission must not expose materialized MIR body discovery helpers",
                ),
                fp(
                    "fun_index.get",
                    "source body emission must not recover callable source metadata from HIR fun_index",
                ),
                fp(
                    "hir_fun_for_callable_fqn",
                    "source body emission must consume the LIR/base callable source contract instead of HIR fun lookup",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM callable identity",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs",
            (
                fp(
                    "materialized_pass_view",
                    "callable identity must come from LIR callable symbol facts instead of MIR pass views",
                ),
                fp(
                    "MaterializedMirPassView",
                    "callable identity must not depend on MIR pass-view types",
                ),
                fp(
                    "stable_closure_key_for_materialized_callable",
                    "closure identity must be based on LIR source callable identity",
                ),
                fp(
                    "self.source_signatures.get",
                    "exported ABI identity must come from LIR callable symbol facts",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM dispatch target declaration",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/gc.rs",
            (
                fp(
                    "fun_index.get",
                    "dispatch target declarations must use LIR callable/source signature contracts",
                ),
                fp(
                    "declare_top_level_fun(",
                    "dispatch target declarations must not fall back to HIR function declarations",
                ),
                fp(
                    "HIR/materialized declaration source",
                    "dispatch target diagnostics must not expose HIR/materialized declaration fallback",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM MIR dispatch helpers",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs",
            (
                fp(
                    "fun_index.get",
                    "MIR dispatch helpers must use LIR callable signature facts",
                ),
                fp(
                    "declare_top_level_fun(",
                    "MIR dispatch helpers must not declare dispatch targets through HIR functions",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM ordinary callee analysis",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/ordinary_callee.rs",
            (
                fp(
                    "materialized_pass_view",
                    "ordinary callee analysis must not consume MIR pass-view escape facts from LLVM production codegen",
                ),
                fp(
                    "MaterializedMirPassView",
                    "ordinary callee analysis must not depend on MIR pass-view types",
                ),
                fp(
                    "self.fun_index",
                    "ordinary callee analysis must use LIR callable facts instead of HIR function indexes",
                ),
                fp(
                    "HirFacts",
                    "ordinary callee analysis must use narrow effect-analysis facts instead of full HIR facts",
                ),
                fp(
                    "collect_known_fun_call_suspendability",
                    "LLVM ordinary callee analysis must not re-run HIR-body suspendability collection",
                ),
            ),
        ),
        SourceBoundaryRule(
            "LLVM class ctor init body",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm/codegen/class_ctor.rs",
            (
                fp(
                    "ctor.body.as_ref",
                    "class ctor body emission must consume the LIR-owned init contract instead of HIR ctor bodies",
                ),
                fp(
                    "codegen_block_value(body)",
                    "class ctor body emission must not directly lower a ctor HIR body",
                ),
            ),
        ),
    )


def source_tree_boundary_rules() -> tuple[SourceTreeBoundaryRule, ...]:
    return (
        SourceTreeBoundaryRule(
            "HIR direct MIR residuals",
            "stage-split-boundary",
            "crates/scoopc_hir/src/hir",
            (),
            (
                fp(
                    "use crate::mir",
                    "HIR lowering must use base identity contracts instead of MIR-owned APIs",
                ),
                fp(
                    "crate::mir::",
                    "HIR lowering must not reach directly into raw MIR APIs",
                ),
                fp(
                    "scoopc_mir::",
                    "future HIR crate must not depend directly on the MIR stage crate",
                ),
                fp(
                    "scoopc::mir",
                    "future HIR crate must not depend on the scoopc facade for MIR APIs",
                ),
            ),
        ),
        SourceTreeBoundaryRule(
            "typecheck direct HIR residuals",
            "stage-split-boundary",
            "crates/scoopc_hir/src/typecheck",
            (),
            (
                fp(
                    "use crate::hir",
                    "typecheck must publish typed contracts without depending on HIR-owned APIs",
                ),
                fp(
                    "crate::hir::",
                    "typecheck must not reach directly into raw HIR APIs",
                ),
                fp(
                    "scoopc_hir::",
                    "future typecheck owner must not depend on the HIR stage crate for base data",
                ),
                fp(
                    "scoopc::hir",
                    "future typecheck owner must not depend on the scoopc facade for HIR APIs",
                ),
            ),
        ),
        SourceTreeBoundaryRule(
            "LLVM production direct HIR residuals",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm",
            ("/tests/", "tests.rs"),
            (
                fp(
                    "use crate::hir",
                    "LLVM production must consume HIR-shaped source payloads through the LIR-owned source namespace",
                ),
                fp(
                    "crate::hir::",
                    "LLVM production must not reach directly into raw HIR APIs",
                ),
                fp(
                    "scoopc_hir::",
                    "future LLVM crate must not depend directly on the HIR stage crate",
                ),
                fp(
                    "scoopc::hir",
                    "future LLVM crate must not depend on the scoopc facade for HIR APIs",
                ),
            ),
        ),
        SourceTreeBoundaryRule(
            "LLVM production direct MIR residuals outside LIR source-body helpers",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm",
            ("/tests/", "tests.rs", "/codegen/mir_body/"),
            (
                fp(
                    "use crate::mir",
                    "LLVM production must consume MIR-shaped source payloads through the LIR-owned source namespace",
                ),
                fp(
                    "crate::mir::",
                    "only the documented LIR-owned source-body helper may pattern-match MIR-shaped payloads",
                ),
                fp(
                    "scoopc_mir::",
                    "future LLVM crate must not depend directly on the MIR stage crate",
                ),
                fp(
                    "scoopc::mir",
                    "future LLVM crate must not depend on the scoopc facade for MIR APIs",
                ),
            ),
        ),
        SourceTreeBoundaryRule(
            "LLVM production direct effect-stage residuals",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm",
            ("/tests/", "tests.rs"),
            (
                fp(
                    "use crate::effect::",
                    "LLVM production must consume ordinary-callee suspend contracts through the LIR owner",
                ),
                fp(
                    "use crate::effect::{",
                    "LLVM production must not import effect-stage APIs through grouped imports",
                ),
                fp(
                    "crate::effect::",
                    "LLVM production must not reach into effect-stage APIs for suspend analysis",
                ),
                fp(
                    "scoopc_effect_facts_stage::effect",
                    "future LLVM crate must not depend on the effect-facts stage crate",
                ),
                fp(
                    "scoopc::effect",
                    "future LLVM crate must not recover effect-stage APIs through the scoopc facade",
                ),
            ),
        ),
        SourceTreeBoundaryRule(
            "LLVM production direct scoopc handoff residuals",
            "backend-boundary",
            "crates/scoopc_codegen_llvm/src/llvm",
            ("/tests/", "tests.rs"),
            (
                fp(
                    "crate::pipeline",
                    "standalone LLVM codegen must consume codegen-owned handoff types, not the scoopc pipeline module",
                ),
                fp(
                    "crate::frontend",
                    "single-file/frontend orchestration belongs to the scoopc wrapper, not the LLVM codegen crate",
                ),
                fp(
                    "scoopc::pipeline",
                    "LLVM codegen must not recover pipeline handoff through the scoopc facade",
                ),
                fp(
                    "scoopc::frontend",
                    "LLVM codegen must not recover frontend orchestration through the scoopc facade",
                ),
            ),
        ),
        SourceTreeBoundaryRule(
            "scoopc_lir direct HIR/AST residuals",
            "stage-split-boundary",
            "crates/scoopc_lir/src",
            (),
            (
                fp(
                    "use scoopc_hir",
                    "scoopc_lir must consume source payloads through MIR/LIR-owned contracts",
                ),
                fp(
                    "scoopc_hir::",
                    "scoopc_lir must not depend directly on the HIR stage crate",
                ),
                fp(
                    "scoopc::hir",
                    "scoopc_lir must not use the scoopc facade for HIR APIs",
                ),
                fp(
                    "use scoopc_ast",
                    "scoopc_lir must consume source payloads through MIR/LIR-owned contracts",
                ),
                fp(
                    "scoopc_ast::",
                    "scoopc_lir must not depend directly on the AST stage crate",
                ),
                fp(
                    "scoopc::ast",
                    "scoopc_lir must not use the scoopc facade for AST APIs",
                ),
                fp(
                    "scoopc_mir::hir",
                    "scoopc_lir must not use the MIR crate's frontend facade as a HIR dependency workaround",
                ),
                fp(
                    "scoopc_mir::ast",
                    "scoopc_lir must not use the MIR crate's frontend facade as an AST dependency workaround",
                ),
            ),
        ),
        SourceTreeBoundaryRule(
            "scoop facade compiler library residuals",
            "driver-boundary",
            "crates/scoop/src",
            (),
            (
                fp(
                    "use scoopc::",
                    "scoop facade must not import the compiler library API",
                ),
                fp(
                    "scoopc::",
                    "scoop facade must not call compiler library APIs in-process",
                ),
                fp(
                    "scoopc_cone",
                    "scoop facade must not depend on cone stage payload APIs",
                ),
                fp(
                    "scoopc_ast",
                    "scoop facade must not depend on compiler stage crates",
                ),
                fp(
                    "scoopc_hir",
                    "scoop facade must not depend on compiler stage crates",
                ),
                fp(
                    "scoopc_mir",
                    "scoop facade must not depend on compiler stage crates",
                ),
                fp(
                    "scoopc_effect_facts_stage",
                    "scoop facade must not depend on compiler stage crates",
                ),
                fp(
                    "scoopc_lir",
                    "scoop facade must not depend on compiler stage crates",
                ),
                fp(
                    "scoopc_codegen_llvm",
                    "scoop facade must not depend on backend crates",
                ),
            ),
        ),
    )


if __name__ == "__main__":
    raise SystemExit(main())
