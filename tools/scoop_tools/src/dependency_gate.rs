//! Dependency gate for pipeline-refactor base and fact crates.
//!
//! The gate checks each base crate from the package's own `cargo tree` view.
//! That catches accidental reverse dependencies from a base crate back to the
//! `scoopc` facade, stage crates, fact crates, driver/runtime crates, or a later
//! base crate in the P1 dependency direction. It also checks fact crates,
//! currently including `scoopc_hir_facts`, `scoopc_mir_facts`,
//! `scoopc_effect_facts`, and `scoopc_lir_facts`, so they only depend on base
//! crates and never on the facade, stages, or other facts.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use miette::{IntoDiagnostic as _, Result};

const BASE_CRATES: &[&str] = &[
    "scoopc_span",
    "scoopc_source",
    "scoopc_types",
    "scoopc_ids",
    "scoopc_project_model",
];

const FACT_CRATES: &[&str] = &[
    "scoopc_hir_facts",
    "scoopc_mir_facts",
    "scoopc_effect_facts",
    "scoopc_lir_facts",
];

const FORBIDDEN_WORKSPACE_CRATES: &[&str] = &[
    "scoop",
    "scoopc",
    "scoop_runtime",
    "scoop_tools",
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
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    checks: Vec<CrateCheck>,
    source_checks: Vec<SourceBoundaryCheck>,
}

impl Report {
    pub fn render(&self) -> String {
        if self.has_violations() {
            return self.render_failures();
        }

        let mut lines = vec![format!(
            "dependency gate: ok (checked {} pipeline crates: {} base, {} fact; {} source boundary files)",
            self.checks.len(),
            BASE_CRATES.len(),
            FACT_CRATES.len(),
            self.source_checks.len()
        )];
        for check in &self.checks {
            let base_dependencies = check.base_dependencies();
            let rendered = if base_dependencies.is_empty() {
                "none".to_string()
            } else {
                base_dependencies.join(", ")
            };
            lines.push(format!(
                "- {} [{}]: base deps [{}]",
                check.crate_name,
                check.kind.label(),
                rendered
            ));
        }
        for check in &self.source_checks {
            lines.push(format!(
                "- {} [{}]: no forbidden residuals ({})",
                check.label,
                check.kind_label,
                check.path.display()
            ));
        }
        lines.join("\n")
    }

    fn has_violations(&self) -> bool {
        self.checks.iter().any(|check| !check.violations.is_empty())
            || self
                .source_checks
                .iter()
                .any(|check| !check.violations.is_empty())
    }

    fn render_failures(&self) -> String {
        let mut lines = vec!["dependency gate: failed".to_string()];
        for check in &self.checks {
            for violation in &check.violations {
                lines.push(format!(
                    "- {} depends on {}: {}",
                    check.crate_name, violation.dependency, violation.reason
                ));
            }
        }
        for check in &self.source_checks {
            for violation in &check.violations {
                lines.push(format!(
                    "- {} {}:{} contains `{}`: {}",
                    check.label,
                    check.path.display(),
                    violation.line,
                    violation.pattern,
                    violation.reason
                ));
            }
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CrateCheck {
    crate_name: String,
    kind: CrateKind,
    dependency_names: BTreeSet<String>,
    violations: Vec<Violation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrateKind {
    Base,
    Fact,
}

impl CrateKind {
    fn label(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Fact => "fact",
        }
    }
}

impl CrateCheck {
    fn base_dependencies(&self) -> Vec<String> {
        self.dependency_names
            .iter()
            .filter(|name| name.as_str() != self.crate_name)
            .filter(|name| BASE_CRATES.contains(&name.as_str()))
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Violation {
    dependency: String,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceBoundaryCheck {
    label: &'static str,
    kind_label: &'static str,
    path: PathBuf,
    violations: Vec<SourceBoundaryViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceBoundaryViolation {
    pattern: &'static str,
    reason: &'static str,
    line: usize,
}

pub fn run() -> Result<Report> {
    let workspace_root = workspace_root()?;
    let mut checks = Vec::new();

    for crate_name in BASE_CRATES {
        let dependency_names = cargo_tree_package_names(crate_name, &workspace_root)?;
        let violations = find_dependency_violations(crate_name, CrateKind::Base, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::Base,
            dependency_names,
            violations,
        });
    }

    for crate_name in FACT_CRATES {
        let dependency_names = cargo_tree_package_names(crate_name, &workspace_root)?;
        let violations = find_dependency_violations(crate_name, CrateKind::Fact, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::Fact,
            dependency_names,
            violations,
        });
    }

    let source_checks = run_source_boundary_checks(&workspace_root)?;
    let report = Report {
        checks,
        source_checks,
    };
    if report.has_violations() {
        return Err(miette::miette!("{}", report.render_failures()));
    }
    Ok(report)
}

fn run_source_boundary_checks(workspace_root: &Path) -> Result<Vec<SourceBoundaryCheck>> {
    let rules = source_boundary_rules();
    let mut checks = Vec::with_capacity(rules.len());
    for rule in rules {
        let path = workspace_root.join(rule.path);
        let contents = fs::read_to_string(&path).into_diagnostic()?;
        checks.push(SourceBoundaryCheck {
            label: rule.label,
            kind_label: rule.kind_label,
            path: PathBuf::from(rule.path),
            violations: source_boundary_violations(&contents, rule.forbidden),
        });
    }
    Ok(checks)
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| miette::miette!("unable to locate workspace root from scoop_tools"))
}

fn cargo_tree_package_names(crate_name: &str, workspace_root: &Path) -> Result<BTreeSet<String>> {
    let output = Command::new("cargo")
        .args([
            "tree", "--edges", "normal", "--prefix", "none", "-p", crate_name,
        ])
        .current_dir(workspace_root)
        .output()
        .into_diagnostic()?;

    if !output.status.success() {
        return Err(miette::miette!(
            "cargo tree failed for {crate_name}: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8(output.stdout).into_diagnostic()?;
    Ok(parse_cargo_tree_package_names(&stdout))
}

fn parse_cargo_tree_package_names(tree: &str) -> BTreeSet<String> {
    tree.lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct SourceBoundaryRule {
    label: &'static str,
    kind_label: &'static str,
    path: &'static str,
    forbidden: &'static [ForbiddenSourcePattern],
}

#[derive(Debug, Clone, Copy)]
struct ForbiddenSourcePattern {
    pattern: &'static str,
    reason: &'static str,
}

fn source_boundary_rules() -> Vec<SourceBoundaryRule> {
    vec![
        SourceBoundaryRule {
            label: "LLVM stage handoff",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/pipeline/llvm_codegen_stage.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "EffectLoweredStageOutput",
                    reason: "LLVM stage output must not reintroduce P5 stage-output wrapper handoff",
                },
                ForbiddenSourcePattern {
                    pattern: "EffectFactsStageOutput",
                    reason: "LLVM stage output must not carry effect-facts stage-output wrapper handoff",
                },
                ForbiddenSourcePattern {
                    pattern: "hir_compat_scaffold",
                    reason: "LLVM stage output must not restore HIR compatibility scaffold handoff",
                },
                ForbiddenSourcePattern {
                    pattern: "llvm_residual_pass_view",
                    reason: "LirStageOutput must not restore LLVM-only residual accessors",
                },
                ForbiddenSourcePattern {
                    pattern: "materialized_pass_view",
                    reason: "LLVM stage handoff must not expose materialized MIR/pass-view accessors to production codegen",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "LLVM stage handoff must not expose materialized MIR/pass-view types to production codegen",
                },
                ForbiddenSourcePattern {
                    pattern: "LlvmSourceCallableSignature",
                    reason: "LLVM stage handoff must not publish HIR-derived callable signatures",
                },
                ForbiddenSourcePattern {
                    pattern: "source_signatures:",
                    reason: "LLVM stage handoff must not carry HIR-derived source signature maps",
                },
                ForbiddenSourcePattern {
                    pattern: "precheck_invalid_integer_literals",
                    reason: "integer literal validation belongs to frontend/typecheck, not LLVM HIR body scans",
                },
                ForbiddenSourcePattern {
                    pattern: "precheck_expr_integer_literals",
                    reason: "LLVM stage must not traverse HIR expressions for literal validation",
                },
                ForbiddenSourcePattern {
                    pattern: "precheck_when_pattern_integer_literals",
                    reason: "LLVM stage must not traverse HIR patterns for literal validation",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM emit handoff",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/emit.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "EffectLoweredStageOutput",
                    reason: "emit handoff must consume LIR/LIR facts, not P5 stage-output wrappers",
                },
                ForbiddenSourcePattern {
                    pattern: "EffectFactsStageOutput",
                    reason: "emit handoff must not carry effect-facts stage-output wrappers",
                },
                ForbiddenSourcePattern {
                    pattern: "LoweredHir",
                    reason: "emit handoff must not receive HIR body/scaffold input",
                },
                ForbiddenSourcePattern {
                    pattern: "HirFacts",
                    reason: "emit handoff must not receive HIR facts as an emit input wrapper",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "emit handoff must not receive raw MIR/pass-view input directly",
                },
                ForbiddenSourcePattern {
                    pattern: "materialized_pass_view",
                    reason: "emit handoff must not receive raw MIR/pass-view input directly",
                },
                ForbiddenSourcePattern {
                    pattern: "crate::hir",
                    reason: "emit handoff must not traverse HIR APIs",
                },
                ForbiddenSourcePattern {
                    pattern: "crate::mir",
                    reason: "emit handoff must not traverse raw MIR APIs",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM reachability",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/reachability.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "use crate::hir",
                    reason: "backend reachability must not scan HIR",
                },
                ForbiddenSourcePattern {
                    pattern: "use crate::mir",
                    reason: "backend reachability must not scan raw MIR",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "backend reachability must use LIR facts instead of MIR pass views",
                },
                ForbiddenSourcePattern {
                    pattern: "try_devirtualize",
                    reason: "ordinary dispatch devirtualization is MIR-owned, not backend reachability-owned",
                },
                ForbiddenSourcePattern {
                    pattern: "devirtualization_facts",
                    reason: "backend reachability must not consume MIR devirtualization facts directly",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM codegen context",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/codegen/mod.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "materialized_pass_view",
                    reason: "LLVM production codegen context must not carry raw MIR/pass-view accessors",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "LLVM production codegen context must not carry raw MIR/pass-view types",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM direct call lowering",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/codegen/call/lowering.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "materialized_pass_view",
                    reason: "direct call lowering must use LIR callable signature facts instead of MIR pass-view fallback",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "direct call lowering must not depend on MIR pass-view types",
                },
                ForbiddenSourcePattern {
                    pattern: "materialized_owner_hir_fun_for_callable",
                    reason: "direct call signature lookup must not recover HIR owners from materialized MIR fallback",
                },
                ForbiddenSourcePattern {
                    pattern: "fun_index.get",
                    reason: "direct/dispatch call lowering must use LIR callable signature facts instead of HIR fun_index fallback",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM callable ABI facts",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/codegen/call/abi.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "self.source_signatures.get",
                    reason: "callable ABI signature lookup must use LIR source signature facts instead of HIR-derived maps",
                },
                ForbiddenSourcePattern {
                    pattern: "fun_index.get",
                    reason: "callable ABI lookup must not recover signatures from HIR fun_index",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM source callable lookup",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/codegen/mir_body/callable_lookup.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "materialized_pass_view",
                    reason: "source body emission must use LIR-owned source callable contracts instead of MIR pass views",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "source body emission must not depend on MIR pass-view types",
                },
                ForbiddenSourcePattern {
                    pattern: "materialized_mir_callable",
                    reason: "source body emission must not expose materialized MIR body discovery helpers",
                },
                ForbiddenSourcePattern {
                    pattern: "fun_index.get",
                    reason: "source body emission must not recover callable source metadata from HIR fun_index",
                },
                ForbiddenSourcePattern {
                    pattern: "hir_fun_for_callable_fqn",
                    reason: "source body emission must consume the LIR/base callable source contract instead of HIR fun lookup",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM callable identity",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/codegen/main/identity.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "materialized_pass_view",
                    reason: "callable identity must come from LIR callable symbol facts instead of MIR pass views",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "callable identity must not depend on MIR pass-view types",
                },
                ForbiddenSourcePattern {
                    pattern: "stable_closure_key_for_materialized_callable",
                    reason: "closure identity must be based on LIR source callable identity",
                },
                ForbiddenSourcePattern {
                    pattern: "self.source_signatures.get",
                    reason: "exported ABI identity must come from LIR callable symbol facts",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM dispatch target declaration",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/codegen/gc.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "fun_index.get",
                    reason: "dispatch target declarations must use LIR callable/source signature contracts",
                },
                ForbiddenSourcePattern {
                    pattern: "declare_top_level_fun(",
                    reason: "dispatch target declarations must not fall back to HIR function declarations",
                },
                ForbiddenSourcePattern {
                    pattern: "HIR/materialized declaration source",
                    reason: "dispatch target diagnostics must not expose HIR/materialized declaration fallback",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM MIR dispatch helpers",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/codegen/mir_body/dispatch.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "fun_index.get",
                    reason: "MIR dispatch helpers must use LIR callable signature facts",
                },
                ForbiddenSourcePattern {
                    pattern: "declare_top_level_fun(",
                    reason: "MIR dispatch helpers must not declare dispatch targets through HIR functions",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM ordinary callee analysis",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/codegen/ordinary_callee.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "materialized_pass_view",
                    reason: "ordinary callee analysis must not consume MIR pass-view escape facts from LLVM production codegen",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "ordinary callee analysis must not depend on MIR pass-view types",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM class ctor init body",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/llvm/codegen/class_ctor.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "ctor.body.as_ref",
                    reason: "class ctor body emission must consume the LIR-owned init contract instead of HIR ctor bodies",
                },
                ForbiddenSourcePattern {
                    pattern: "codegen_block_value(body)",
                    reason: "class ctor body emission must not directly lower a ctor HIR body",
                },
            ],
        },
    ]
}

fn source_boundary_violations(
    contents: &str,
    forbidden: &[ForbiddenSourcePattern],
) -> Vec<SourceBoundaryViolation> {
    let mut violations = Vec::new();
    for (line_index, line) in contents.lines().enumerate() {
        for pattern in forbidden {
            if line.contains(pattern.pattern) {
                violations.push(SourceBoundaryViolation {
                    pattern: pattern.pattern,
                    reason: pattern.reason,
                    line: line_index + 1,
                });
            }
        }
    }
    violations
}

fn find_dependency_violations(
    checked_crate: &str,
    kind: CrateKind,
    dependency_names: &BTreeSet<String>,
) -> Vec<Violation> {
    let allowed_base_dependencies: BTreeSet<&str> = allowed_base_dependencies(checked_crate)
        .iter()
        .copied()
        .collect();
    let mut violations = Vec::new();

    for dependency in dependency_names {
        let dependency_name = dependency.as_str();
        if dependency_name == checked_crate {
            continue;
        }

        if kind == CrateKind::Fact {
            if FACT_CRATES.contains(&dependency_name) {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "fact crates must not depend on other fact crates".to_string(),
                });
                continue;
            }

            if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name) {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "fact crates must only depend on base crates, not facade, driver/runtime/tool, stage, backend, or fact crates"
                        .to_string(),
                });
            }

            continue;
        }

        if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name) {
            violations.push(Violation {
                dependency: dependency.clone(),
                reason: "base crates must not depend on facade, driver/runtime/tool, stage, or fact crates"
                    .to_string(),
            });
            continue;
        }

        if BASE_CRATES.contains(&dependency_name)
            && !allowed_base_dependencies.contains(dependency_name)
        {
            violations.push(Violation {
                dependency: dependency.clone(),
                reason: "dependency is outside the allowed base-crate direction".to_string(),
            });
        }
    }

    violations
}

fn allowed_base_dependencies(base: &str) -> &'static [&'static str] {
    match base {
        "scoopc_span" => &[],
        "scoopc_source" => &["scoopc_span"],
        "scoopc_types" => &["scoopc_span"],
        "scoopc_ids" => &["scoopc_span"],
        "scoopc_project_model" => &["scoopc_span", "scoopc_source", "scoopc_types", "scoopc_ids"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cargo_tree_package_names() {
        let names = parse_cargo_tree_package_names(
            "scoopc_project_model v0.1.0 (/repo/crates/scoopc_project_model)\n\
             scoopc_source v0.1.0 (/repo/crates/scoopc_source)\n\
             scoopc_span v0.1.0 (/repo/crates/scoopc_span)\n",
        );

        assert!(names.contains("scoopc_project_model"));
        assert!(names.contains("scoopc_source"));
        assert!(names.contains("scoopc_span"));
    }

    #[test]
    fn allows_declared_base_direction() {
        let violations = find_dependency_violations(
            "scoopc_project_model",
            CrateKind::Base,
            &set(&[
                "scoopc_project_model",
                "scoopc_source",
                "scoopc_types",
                "scoopc_ids",
                "scoopc_span",
            ]),
        );

        assert!(violations.is_empty());
    }

    #[test]
    fn rejects_facade_dependency() {
        let violations = find_dependency_violations(
            "scoopc_source",
            CrateKind::Base,
            &set(&["scoopc_source", "scoopc_span", "scoopc"]),
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].dependency, "scoopc");
    }

    #[test]
    fn rejects_later_base_dependency() {
        let violations = find_dependency_violations(
            "scoopc_span",
            CrateKind::Base,
            &set(&["scoopc_span", "scoopc_project_model"]),
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].dependency, "scoopc_project_model");
    }

    #[test]
    fn allows_fact_crate_to_depend_on_base_crates() {
        let violations = find_dependency_violations(
            "scoopc_hir_facts",
            CrateKind::Fact,
            &set(&[
                "scoopc_hir_facts",
                "scoopc_project_model",
                "scoopc_source",
                "scoopc_types",
                "scoopc_ids",
                "scoopc_span",
            ]),
        );

        assert!(violations.is_empty());
    }

    #[test]
    fn allows_mir_fact_crate_to_depend_on_base_crates() {
        let violations = find_dependency_violations(
            "scoopc_mir_facts",
            CrateKind::Fact,
            &set(&[
                "scoopc_mir_facts",
                "scoopc_project_model",
                "scoopc_source",
                "scoopc_types",
                "scoopc_ids",
                "scoopc_span",
            ]),
        );

        assert!(violations.is_empty());
    }

    #[test]
    fn allows_effect_fact_crate_to_depend_on_base_crates() {
        let violations = find_dependency_violations(
            "scoopc_effect_facts",
            CrateKind::Fact,
            &set(&[
                "scoopc_effect_facts",
                "scoopc_types",
                "scoopc_ids",
                "scoopc_span",
            ]),
        );

        assert!(violations.is_empty());
    }

    #[test]
    fn allows_lir_fact_crate_to_depend_on_base_crates() {
        let violations = find_dependency_violations(
            "scoopc_lir_facts",
            CrateKind::Fact,
            &set(&[
                "scoopc_lir_facts",
                "scoopc_project_model",
                "scoopc_ids",
                "scoopc_span",
            ]),
        );

        assert!(violations.is_empty());
    }

    #[test]
    fn rejects_fact_crate_facade_dependency() {
        let violations = find_dependency_violations(
            "scoopc_hir_facts",
            CrateKind::Fact,
            &set(&["scoopc_hir_facts", "scoopc_span", "scoopc"]),
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].dependency, "scoopc");
    }

    #[test]
    fn rejects_fact_crate_dependency_on_another_fact_crate() {
        let violations = find_dependency_violations(
            "scoopc_hir_facts",
            CrateKind::Fact,
            &set(&["scoopc_hir_facts", "scoopc_mir_facts"]),
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].dependency, "scoopc_mir_facts");
    }

    #[test]
    fn detects_forbidden_source_boundary_pattern() {
        let violations = source_boundary_violations(
            "fn ok() {}\nfn bad(_: EffectLoweredStageOutput) {}\n",
            &[ForbiddenSourcePattern {
                pattern: "EffectLoweredStageOutput",
                reason: "stage wrapper is forbidden",
            }],
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 2);
        assert_eq!(violations[0].pattern, "EffectLoweredStageOutput");
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }
}
