//! Dependency gate for pipeline-refactor base and fact crates.
//!
//! The gate checks each base crate from the package's own `cargo tree` view.
//! That catches accidental reverse dependencies from a base crate back to the
//! `scoopc` facade, stage crates, fact crates, cone crate, driver/runtime crates,
//! or a later base crate in the P1 dependency direction. It also checks fact
//! crates, currently including `scoopc_hir_facts`, `scoopc_mir_facts`,
//! `scoopc_effect_facts`, and `scoopc_lir_facts`, so they only depend on base
//! crates and never on the facade, stages, cone, or other facts. Stage crates are
//! checked in pipeline order, and the cone-operation crate is checked separately:
//! cone may depend on stage/fact/base crates, but no stage may depend on cone.

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

const BASE_ONLY_STAGE_CRATES: &[&str] = &["scoopc_ast"];

const HIR_STAGE_CRATES: &[&str] = &["scoopc_hir"];

const MIR_STAGE_CRATES: &[&str] = &["scoopc_mir"];

const EFFECT_FACTS_STAGE_CRATES: &[&str] = &["scoopc_effect_facts_stage"];

const LIR_STAGE_CRATES: &[&str] = &["scoopc_lir"];

const CODEGEN_STAGE_CRATES: &[&str] = &["scoopc_codegen_llvm"];

const CONE_CRATES: &[&str] = &["scoopc_cone"];

const LINKER_CRATES: &[&str] = &["scoopld"];

const FORBIDDEN_WORKSPACE_CRATES: &[&str] = &[
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
            "dependency gate: ok (checked {} pipeline crates: {} base, {} fact, {} base-only stage, {} HIR stage, {} MIR stage, {} effect-facts stage, {} LIR stage, {} codegen stage, {} cone, {} linker; {} source boundary files)",
            self.checks.len(),
            self.count_crate_kind(CrateKind::Base),
            self.count_crate_kind(CrateKind::Fact),
            self.count_crate_kind(CrateKind::BaseOnlyStage),
            self.count_crate_kind(CrateKind::HirStage),
            self.count_crate_kind(CrateKind::MirStage),
            self.count_crate_kind(CrateKind::EffectFactsStage),
            self.count_crate_kind(CrateKind::LirStage),
            self.count_crate_kind(CrateKind::CodegenStage),
            self.count_crate_kind(CrateKind::Cone),
            self.count_crate_kind(CrateKind::Linker),
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

    fn count_crate_kind(&self, kind: CrateKind) -> usize {
        self.checks
            .iter()
            .filter(|check| check.kind == kind)
            .count()
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
    BaseOnlyStage,
    HirStage,
    MirStage,
    EffectFactsStage,
    LirStage,
    CodegenStage,
    Cone,
    Linker,
}

impl CrateKind {
    fn label(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Fact => "fact",
            Self::BaseOnlyStage => "base-only-stage",
            Self::HirStage => "hir-stage",
            Self::MirStage => "mir-stage",
            Self::EffectFactsStage => "effect-facts-stage",
            Self::LirStage => "lir-stage",
            Self::CodegenStage => "codegen-stage",
            Self::Cone => "cone",
            Self::Linker => "linker",
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

    for crate_name in BASE_ONLY_STAGE_CRATES {
        let dependency_names = cargo_tree_package_names(crate_name, &workspace_root)?;
        let violations =
            find_dependency_violations(crate_name, CrateKind::BaseOnlyStage, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::BaseOnlyStage,
            dependency_names,
            violations,
        });
    }

    for crate_name in HIR_STAGE_CRATES {
        let dependency_names = cargo_tree_package_names(crate_name, &workspace_root)?;
        let violations =
            find_dependency_violations(crate_name, CrateKind::HirStage, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::HirStage,
            dependency_names,
            violations,
        });
    }

    for crate_name in MIR_STAGE_CRATES {
        let dependency_names = cargo_tree_package_names(crate_name, &workspace_root)?;
        let violations =
            find_dependency_violations(crate_name, CrateKind::MirStage, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::MirStage,
            dependency_names,
            violations,
        });
    }

    for crate_name in EFFECT_FACTS_STAGE_CRATES {
        let dependency_names = cargo_tree_package_names(crate_name, &workspace_root)?;
        let violations =
            find_dependency_violations(crate_name, CrateKind::EffectFactsStage, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::EffectFactsStage,
            dependency_names,
            violations,
        });
    }

    for crate_name in LIR_STAGE_CRATES {
        if !crate_manifest_exists(crate_name, &workspace_root) {
            continue;
        }
        let dependency_names = cargo_tree_direct_package_names(crate_name, &workspace_root)?;
        let violations =
            find_dependency_violations(crate_name, CrateKind::LirStage, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::LirStage,
            dependency_names,
            violations,
        });
    }

    for crate_name in CODEGEN_STAGE_CRATES {
        let dependency_names = cargo_tree_direct_package_names(crate_name, &workspace_root)?;
        let violations =
            find_dependency_violations(crate_name, CrateKind::CodegenStage, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::CodegenStage,
            dependency_names,
            violations,
        });
    }

    for crate_name in CONE_CRATES {
        let dependency_names = cargo_tree_direct_package_names(crate_name, &workspace_root)?;
        let violations = find_dependency_violations(crate_name, CrateKind::Cone, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::Cone,
            dependency_names,
            violations,
        });
    }

    for crate_name in LINKER_CRATES {
        let dependency_names = cargo_tree_direct_package_names(crate_name, &workspace_root)?;
        let violations =
            find_dependency_violations(crate_name, CrateKind::Linker, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            kind: CrateKind::Linker,
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
    for rule in source_tree_boundary_rules() {
        checks.extend(run_source_tree_boundary_check(workspace_root, rule)?);
    }
    Ok(checks)
}

fn run_source_tree_boundary_check(
    workspace_root: &Path,
    rule: SourceTreeBoundaryRule,
) -> Result<Vec<SourceBoundaryCheck>> {
    let root = workspace_root.join(rule.root);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files)?;
    files.sort();

    let mut checks = Vec::new();
    for path in files {
        let rel_path = path
            .strip_prefix(workspace_root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| path.clone());
        let rel_text = rel_path.to_string_lossy();
        if source_path_is_excluded(&rel_text, rule.exclude_path_fragments) {
            continue;
        }
        let contents = fs::read_to_string(&path).into_diagnostic()?;
        checks.push(SourceBoundaryCheck {
            label: rule.label,
            kind_label: rule.kind_label,
            path: rel_path,
            violations: source_boundary_violations(&contents, rule.forbidden),
        });
    }
    Ok(checks)
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn source_path_is_excluded(path: &str, fragments: &[&str]) -> bool {
    fragments.iter().any(|fragment| path.contains(fragment))
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| miette::miette!("unable to locate workspace root from scoop_tools"))
}

fn crate_manifest_exists(crate_name: &str, workspace_root: &Path) -> bool {
    workspace_root
        .join("crates")
        .join(crate_name)
        .join("Cargo.toml")
        .is_file()
}

fn cargo_tree_package_names(crate_name: &str, workspace_root: &Path) -> Result<BTreeSet<String>> {
    cargo_tree_package_names_with_extra_args(crate_name, workspace_root, &[])
}

fn cargo_tree_direct_package_names(
    crate_name: &str,
    workspace_root: &Path,
) -> Result<BTreeSet<String>> {
    cargo_tree_package_names_with_extra_args(crate_name, workspace_root, &["--depth", "1"])
}

fn cargo_tree_package_names_with_extra_args(
    crate_name: &str,
    workspace_root: &Path,
    extra_args: &[&str],
) -> Result<BTreeSet<String>> {
    let mut args = vec![
        "tree", "--edges", "normal", "--prefix", "none", "-p", crate_name,
    ];
    args.extend_from_slice(extra_args);
    let output = Command::new("cargo")
        .args(args)
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
struct SourceTreeBoundaryRule {
    label: &'static str,
    kind_label: &'static str,
    root: &'static str,
    exclude_path_fragments: &'static [&'static str],
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
            label: "Scoop comptime keyword token",
            kind_label: "frontend-boundary",
            path: "crates/scoopc_ast/src/syntax/token.rs",
            forbidden: &[ForbiddenSourcePattern {
                pattern: "Comptime",
                reason: "old comptime surface must not be preserved as a dedicated keyword token",
            }],
        },
        SourceBoundaryRule {
            label: "Scoop comptime lexer surface",
            kind_label: "frontend-boundary",
            path: "crates/scoopc_ast/src/syntax/lexer.rs",
            forbidden: &[ForbiddenSourcePattern {
                pattern: "\"comptime\" =>",
                reason: "old comptime surface must lex as a normal identifier, not a dedicated keyword",
            }],
        },
        SourceBoundaryRule {
            label: "Scoop comptime parser surface",
            kind_label: "frontend-boundary",
            path: "crates/scoopc_ast/src/parser/cursor.rs",
            forbidden: &[ForbiddenSourcePattern {
                pattern: "Keyword::Comptime",
                reason: "parser recovery must not preserve a dedicated old comptime statement surface",
            }],
        },
        SourceBoundaryRule {
            label: "LLVM const-eval module residual",
            kind_label: "backend-boundary",
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/mod.rs",
            forbidden: &[ForbiddenSourcePattern {
                pattern: "const_eval",
                reason: "backend data initializer helpers must not restore old const-evaluator naming or module boundaries",
            }],
        },
        SourceBoundaryRule {
            label: "scoopc umbrella Cargo backend-private deps",
            kind_label: "stage-split-boundary",
            path: "crates/scoopc/Cargo.toml",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "inkwell =",
                    reason: "scoopc umbrella must not depend directly on backend-private inkwell",
                },
                ForbiddenSourcePattern {
                    pattern: "llvm-sys =",
                    reason: "scoopc umbrella must not depend directly on backend-private llvm-sys",
                },
            ],
        },
        SourceBoundaryRule {
            label: "scoopc LLVM facade implementation",
            kind_label: "stage-split-boundary",
            path: "crates/scoopc/src/llvm.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "use inkwell",
                    reason: "scoopc::llvm must remain a facade over scoopc_codegen_llvm",
                },
                ForbiddenSourcePattern {
                    pattern: "inkwell::",
                    reason: "scoopc::llvm must not recreate backend-private LLVM implementation helpers",
                },
                ForbiddenSourcePattern {
                    pattern: "llvm_sys",
                    reason: "scoopc::llvm must not depend directly on llvm-sys",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM top-level const initializer residual",
            kind_label: "backend-boundary",
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs",
            forbidden: &[ForbiddenSourcePattern {
                pattern: "const_initializer_for_top_level_var",
                reason: "top-level eager init must not restore the old const initializer helper path",
            }],
        },
        SourceBoundaryRule {
            label: "LLVM stage handoff",
            kind_label: "backend-boundary",
            path: "crates/scoopc/src/pipeline/llvm_codegen_stage.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "use crate::effect::",
                    reason: "LLVM stage handoff must consume LIR-owned ordinary-callee suspend contracts, not effect-stage APIs",
                },
                ForbiddenSourcePattern {
                    pattern: "use crate::effect::{",
                    reason: "LLVM stage handoff must not import effect-stage APIs through grouped imports",
                },
                ForbiddenSourcePattern {
                    pattern: "crate::effect::",
                    reason: "LLVM stage handoff must not reach into effect-stage APIs for suspend analysis",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_effect_facts_stage::effect",
                    reason: "LLVM stage handoff must not depend on the future effect-facts stage crate for suspend analysis",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::effect",
                    reason: "LLVM stage handoff must not recover effect-stage APIs through the scoopc facade",
                },
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
                    pattern: "LlvmCallableSignatureContract",
                    reason: "LLVM stage handoff must not publish HIR-derived callable signature fallback contracts",
                },
                ForbiddenSourcePattern {
                    pattern: "build_callable_signature_contracts",
                    reason: "LLVM stage handoff must not rebuild callable signatures from HIR functions",
                },
                ForbiddenSourcePattern {
                    pattern: "source_signatures:",
                    reason: "LLVM stage handoff must not carry HIR-derived source signature maps",
                },
                ForbiddenSourcePattern {
                    pattern: "callable_signatures:",
                    reason: "LLVM stage handoff must not carry HIR-derived callable signature maps",
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
            path: "crates/scoopc_codegen_llvm/src/llvm/emit.rs",
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
            path: "crates/scoopc_codegen_llvm/src/llvm/reachability.rs",
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
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "materialized_pass_view",
                    reason: "LLVM production codegen context must not carry raw MIR/pass-view accessors",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "LLVM production codegen context must not carry raw MIR/pass-view types",
                },
                ForbiddenSourcePattern {
                    pattern: "HirFacts",
                    reason: "LLVM production codegen context must not save full HIR facts",
                },
                ForbiddenSourcePattern {
                    pattern: "fun_index",
                    reason: "LLVM production codegen context must not save HIR function indexes",
                },
                ForbiddenSourcePattern {
                    pattern: "callable_signatures",
                    reason: "LLVM production codegen context must not save HIR-derived callable signature maps",
                },
                ForbiddenSourcePattern {
                    pattern: "LlvmCallableSignatureContract",
                    reason: "LLVM production codegen context must not consume HIR-derived callable signature fallback contracts",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMir",
                    reason: "LLVM production codegen context must not save full materialized MIR wrappers",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedEffectFacts",
                    reason: "LLVM production codegen context must not save full materialized effect-facts wrappers",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM legacy HIR function declarations",
            kind_label: "backend-boundary",
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/declare.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "declare_top_level_fun(",
                    reason: "top-level function declarations must be driven by LIR callable/signature facts, not HIR FunDecl helpers",
                },
                ForbiddenSourcePattern {
                    pattern: "declare_top_level_fun_with_symbol",
                    reason: "top-level function declarations must not reintroduce HIR FunDecl declaration helpers",
                },
                ForbiddenSourcePattern {
                    pattern: "declare_top_level_fun_with_signature_override",
                    reason: "signature overrides must not recover exported ABI declarations through HIR FunDecl helpers",
                },
                ForbiddenSourcePattern {
                    pattern: "declare_top_level_fun_callee_resume_entry",
                    reason: "callee resume declarations must use LIR/source-owned callable contracts instead of HIR FunDecl helpers",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM legacy HIR function emission",
            kind_label: "backend-boundary",
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/function.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "codegen_top_level_fun(",
                    reason: "top-level body emission must use LIR-owned source body contracts instead of HIR FunDecl entry helpers",
                },
                ForbiddenSourcePattern {
                    pattern: "build_fun_callee_suspend_plan(",
                    reason: "ordinary callee suspendability must be published through LIR/effect facts, not rebuilt from HIR FunDecl helpers",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM legacy HIR callable identity",
            kind_label: "backend-boundary",
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs",
            forbidden: &[ForbiddenSourcePattern {
                pattern: "exported_abi_symbol_for_hir_fun",
                reason: "exported ABI identity must come from LIR callable symbol facts, not HIR FunDecl helpers",
            }],
        },
        SourceBoundaryRule {
            label: "LLVM direct call lowering",
            kind_label: "backend-boundary",
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/call/lowering.rs",
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
                ForbiddenSourcePattern {
                    pattern: "self.fun_index",
                    reason: "direct/dispatch call lowering must not retain HIR function indexes",
                },
                ForbiddenSourcePattern {
                    pattern: "dispatch_call_sites",
                    reason: "dispatch kind must come from LIR facts or the explicit LLVM dispatch narrow contract",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM callable ABI facts",
            kind_label: "backend-boundary",
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/call/abi.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "self.source_signatures.get",
                    reason: "callable ABI signature lookup must use LIR source signature facts instead of HIR-derived maps",
                },
                ForbiddenSourcePattern {
                    pattern: "fun_index.get",
                    reason: "callable ABI lookup must not recover signatures from HIR fun_index",
                },
                ForbiddenSourcePattern {
                    pattern: "callable_signatures.get",
                    reason: "callable ABI lookup must fail fast instead of using HIR-derived signature fallback maps",
                },
                ForbiddenSourcePattern {
                    pattern: "self.callable_signatures",
                    reason: "callable ABI lookup must not consume HIR-derived signature fallback maps",
                },
                ForbiddenSourcePattern {
                    pattern: "LlvmCallableSignatureContract",
                    reason: "callable ABI lookup must not depend on HIR-derived LLVM signature contracts",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM source callable lookup",
            kind_label: "backend-boundary",
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs",
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
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs",
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
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/gc.rs",
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
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs",
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
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/ordinary_callee.rs",
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "materialized_pass_view",
                    reason: "ordinary callee analysis must not consume MIR pass-view escape facts from LLVM production codegen",
                },
                ForbiddenSourcePattern {
                    pattern: "MaterializedMirPassView",
                    reason: "ordinary callee analysis must not depend on MIR pass-view types",
                },
                ForbiddenSourcePattern {
                    pattern: "self.fun_index",
                    reason: "ordinary callee analysis must use LIR callable facts instead of HIR function indexes",
                },
                ForbiddenSourcePattern {
                    pattern: "HirFacts",
                    reason: "ordinary callee analysis must use narrow effect-analysis facts instead of full HIR facts",
                },
                ForbiddenSourcePattern {
                    pattern: "collect_known_fun_call_suspendability",
                    reason: "LLVM ordinary callee analysis must not re-run HIR-body suspendability collection",
                },
            ],
        },
        SourceBoundaryRule {
            label: "LLVM class ctor init body",
            kind_label: "backend-boundary",
            path: "crates/scoopc_codegen_llvm/src/llvm/codegen/class_ctor.rs",
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

fn source_tree_boundary_rules() -> Vec<SourceTreeBoundaryRule> {
    vec![
        SourceTreeBoundaryRule {
            label: "HIR direct MIR residuals",
            kind_label: "stage-split-boundary",
            root: "crates/scoopc_hir/src/hir",
            exclude_path_fragments: &[],
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "use crate::mir",
                    reason: "HIR lowering must use base identity contracts instead of MIR-owned APIs",
                },
                ForbiddenSourcePattern {
                    pattern: "crate::mir::",
                    reason: "HIR lowering must not reach directly into raw MIR APIs",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_mir::",
                    reason: "future HIR crate must not depend directly on the MIR stage crate",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::mir",
                    reason: "future HIR crate must not depend on the scoopc facade for MIR APIs",
                },
            ],
        },
        SourceTreeBoundaryRule {
            label: "typecheck direct HIR residuals",
            kind_label: "stage-split-boundary",
            root: "crates/scoopc_hir/src/typecheck",
            exclude_path_fragments: &[],
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "use crate::hir",
                    reason: "typecheck must publish typed contracts without depending on HIR-owned APIs",
                },
                ForbiddenSourcePattern {
                    pattern: "crate::hir::",
                    reason: "typecheck must not reach directly into raw HIR APIs",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_hir::",
                    reason: "future typecheck owner must not depend on the HIR stage crate for base data",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::hir",
                    reason: "future typecheck owner must not depend on the scoopc facade for HIR APIs",
                },
            ],
        },
        SourceTreeBoundaryRule {
            label: "LLVM production direct HIR residuals",
            kind_label: "backend-boundary",
            root: "crates/scoopc_codegen_llvm/src/llvm",
            exclude_path_fragments: &["/tests/", "tests.rs"],
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "use crate::hir",
                    reason: "LLVM production must consume HIR-shaped source payloads through the LIR-owned source namespace",
                },
                ForbiddenSourcePattern {
                    pattern: "crate::hir::",
                    reason: "LLVM production must not reach directly into raw HIR APIs",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_hir::",
                    reason: "future LLVM crate must not depend directly on the HIR stage crate",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::hir",
                    reason: "future LLVM crate must not depend on the scoopc facade for HIR APIs",
                },
            ],
        },
        SourceTreeBoundaryRule {
            label: "LLVM production direct MIR residuals outside LIR source-body helpers",
            kind_label: "backend-boundary",
            root: "crates/scoopc_codegen_llvm/src/llvm",
            exclude_path_fragments: &["/tests/", "tests.rs", "/codegen/mir_body/"],
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "use crate::mir",
                    reason: "LLVM production must consume MIR-shaped source payloads through the LIR-owned source namespace",
                },
                ForbiddenSourcePattern {
                    pattern: "crate::mir::",
                    reason: "only the documented LIR-owned source-body helper may pattern-match MIR-shaped payloads",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_mir::",
                    reason: "future LLVM crate must not depend directly on the MIR stage crate",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::mir",
                    reason: "future LLVM crate must not depend on the scoopc facade for MIR APIs",
                },
            ],
        },
        SourceTreeBoundaryRule {
            label: "LLVM production direct effect-stage residuals",
            kind_label: "backend-boundary",
            root: "crates/scoopc_codegen_llvm/src/llvm",
            exclude_path_fragments: &["/tests/", "tests.rs"],
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "use crate::effect::",
                    reason: "LLVM production must consume ordinary-callee suspend contracts through the LIR owner",
                },
                ForbiddenSourcePattern {
                    pattern: "use crate::effect::{",
                    reason: "LLVM production must not import effect-stage APIs through grouped imports",
                },
                ForbiddenSourcePattern {
                    pattern: "crate::effect::",
                    reason: "LLVM production must not reach into effect-stage APIs for suspend analysis",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_effect_facts_stage::effect",
                    reason: "future LLVM crate must not depend on the effect-facts stage crate",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::effect",
                    reason: "future LLVM crate must not recover effect-stage APIs through the scoopc facade",
                },
            ],
        },
        SourceTreeBoundaryRule {
            label: "LLVM production direct scoopc handoff residuals",
            kind_label: "backend-boundary",
            root: "crates/scoopc_codegen_llvm/src/llvm",
            exclude_path_fragments: &["/tests/", "tests.rs"],
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "crate::pipeline",
                    reason: "standalone LLVM codegen must consume codegen-owned handoff types, not the scoopc pipeline module",
                },
                ForbiddenSourcePattern {
                    pattern: "crate::frontend",
                    reason: "single-file/frontend orchestration belongs to the scoopc wrapper, not the LLVM codegen crate",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::pipeline",
                    reason: "LLVM codegen must not recover pipeline handoff through the scoopc facade",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::frontend",
                    reason: "LLVM codegen must not recover frontend orchestration through the scoopc facade",
                },
            ],
        },
        SourceTreeBoundaryRule {
            label: "scoopc_lir direct HIR/AST residuals",
            kind_label: "stage-split-boundary",
            root: "crates/scoopc_lir/src",
            exclude_path_fragments: &[],
            forbidden: &[
                ForbiddenSourcePattern {
                    pattern: "use scoopc_hir",
                    reason: "scoopc_lir must consume source payloads through MIR/LIR-owned contracts",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_hir::",
                    reason: "scoopc_lir must not depend directly on the HIR stage crate",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::hir",
                    reason: "scoopc_lir must not use the scoopc facade for HIR APIs",
                },
                ForbiddenSourcePattern {
                    pattern: "use scoopc_ast",
                    reason: "scoopc_lir must consume source payloads through MIR/LIR-owned contracts",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_ast::",
                    reason: "scoopc_lir must not depend directly on the AST stage crate",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc::ast",
                    reason: "scoopc_lir must not use the scoopc facade for AST APIs",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_mir::hir",
                    reason: "scoopc_lir must not use the MIR crate's frontend facade as a HIR dependency workaround",
                },
                ForbiddenSourcePattern {
                    pattern: "scoopc_mir::ast",
                    reason: "scoopc_lir must not use the MIR crate's frontend facade as an AST dependency workaround",
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

        if kind == CrateKind::BaseOnlyStage {
            if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name) {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "base-only stage crates must not depend on facade, driver/runtime/tool, other stage, backend, or fact crates"
                        .to_string(),
                });
            }

            continue;
        }

        if kind == CrateKind::HirStage {
            let allowed_stage_inputs = ["scoopc_ast", "scoopc_hir_facts"];
            if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name)
                && !allowed_stage_inputs.contains(&dependency_name)
            {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "HIR stage crates must depend only on base crates, scoopc_ast, and scoopc_hir_facts"
                        .to_string(),
                });
            }

            continue;
        }

        if kind == CrateKind::MirStage {
            let allowed_stage_inputs = [
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_hir_facts",
                "scoopc_mir_facts",
            ];
            if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name)
                && !allowed_stage_inputs.contains(&dependency_name)
            {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "MIR stage crates must depend only on base crates, scoopc_ast, scoopc_hir, scoopc_hir_facts, and scoopc_mir_facts"
                        .to_string(),
                });
            }

            continue;
        }

        if kind == CrateKind::EffectFactsStage {
            let allowed_stage_inputs = [
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_hir_facts",
                "scoopc_mir",
                "scoopc_mir_facts",
                "scoopc_effect_facts",
            ];
            if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name)
                && !allowed_stage_inputs.contains(&dependency_name)
            {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "effect-facts stage crates must depend only on base crates, AST/HIR/MIR stage inputs, HIR/MIR facts, and effect facts"
                        .to_string(),
                });
            }

            continue;
        }

        if kind == CrateKind::LirStage {
            let allowed_stage_inputs = [
                "scoopc_mir",
                "scoopc_mir_facts",
                "scoopc_effect_facts",
                "scoopc_lir_facts",
            ];
            if dependency_name == "scoopc"
                || dependency_name == "scoopc_ast"
                || dependency_name == "scoopc_hir"
                || dependency_name == "scoopc_hir_facts"
                || dependency_name == "scoopc_effect_facts_stage"
                || dependency_name == "scoopc_codegen_llvm"
                || dependency_name == "scoopc_codegen_c"
            {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "LIR stage crates must depend directly only on base, MIR, MIR facts, effect facts, and LIR facts"
                        .to_string(),
                });
                continue;
            }

            if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name)
                && !allowed_stage_inputs.contains(&dependency_name)
            {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "LIR stage crates must not depend on facade, driver/runtime/tool, frontend stages, effect builder stage, backend, or non-LIR input facts"
                        .to_string(),
                });
            }

            continue;
        }

        if kind == CrateKind::CodegenStage {
            let allowed_stage_inputs = ["scoopc_lir", "scoopc_lir_facts"];
            if dependency_name == "scoopc_ast"
                || dependency_name == "scoopc_hir"
                || dependency_name == "scoopc_mir"
                || dependency_name == "scoopc_effect_facts_stage"
                || dependency_name == "scoopc_codegen_c"
                || dependency_name == "scoopc_cone"
                || FACT_CRATES.contains(&dependency_name)
                    && !allowed_stage_inputs.contains(&dependency_name)
            {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "LLVM codegen crates must depend only on base, scoopc_lir, and scoopc_lir_facts"
                        .to_string(),
                });
                continue;
            }

            if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name)
                && !allowed_stage_inputs.contains(&dependency_name)
            {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "LLVM codegen crates must not depend on the scoopc facade, driver/runtime/tool, frontend stages, MIR/effect stages, or non-LIR fact crates"
                        .to_string(),
                });
            }

            continue;
        }

        if kind == CrateKind::Cone {
            if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name)
                && !cone_allowed_workspace_dependency(dependency_name)
            {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "cone crates may depend on base, fact, and stage crates, but not on facade, driver/runtime/tool, codegen-C, or other cone-operation crates"
                        .to_string(),
                });
            }

            continue;
        }

        if kind == CrateKind::Linker {
            if dependency_name == "scoopc_cone" || BASE_CRATES.contains(&dependency_name) {
                continue;
            }
            if FORBIDDEN_WORKSPACE_CRATES.contains(&dependency_name) {
                violations.push(Violation {
                    dependency: dependency.clone(),
                    reason: "scoopld may depend only on base/project-model/cone metadata crates, not facade, driver/runtime/tool, stage, fact, or backend crates"
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

fn cone_allowed_workspace_dependency(dependency_name: &str) -> bool {
    BASE_CRATES.contains(&dependency_name)
        || FACT_CRATES.contains(&dependency_name)
        || BASE_ONLY_STAGE_CRATES.contains(&dependency_name)
        || HIR_STAGE_CRATES.contains(&dependency_name)
        || MIR_STAGE_CRATES.contains(&dependency_name)
        || EFFECT_FACTS_STAGE_CRATES.contains(&dependency_name)
        || LIR_STAGE_CRATES.contains(&dependency_name)
        || CODEGEN_STAGE_CRATES.contains(&dependency_name)
}

fn allowed_base_dependencies(base: &str) -> &'static [&'static str] {
    match base {
        "scoopc_span" => &[],
        "scoopc_source" => &["scoopc_span"],
        "scoopc_types" => &["scoopc_span"],
        "scoopc_ids" => &["scoopc_span", "scoopc_types"],
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
    fn allows_base_only_stage_crate_to_depend_on_base_crates() {
        let violations = find_dependency_violations(
            "scoopc_ast",
            CrateKind::BaseOnlyStage,
            &set(&["scoopc_ast", "scoopc_source", "scoopc_types", "scoopc_span"]),
        );

        assert!(violations.is_empty());
    }

    #[test]
    fn rejects_base_only_stage_dependency_on_facade() {
        let violations = find_dependency_violations(
            "scoopc_ast",
            CrateKind::BaseOnlyStage,
            &set(&["scoopc_ast", "scoopc_span", "scoopc"]),
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].dependency, "scoopc");
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
    fn allows_mir_stage_dependency_direction() {
        let violations = find_dependency_violations(
            "scoopc_mir",
            CrateKind::MirStage,
            &set(&[
                "scoopc_mir",
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_hir_facts",
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
    fn rejects_mir_stage_dependency_on_later_stage_or_facade() {
        let violations = find_dependency_violations(
            "scoopc_mir",
            CrateKind::MirStage,
            &set(&[
                "scoopc_mir",
                "scoopc_hir",
                "scoopc",
                "scoopc_effect_facts",
                "scoopc_lir_facts",
                "scoopc_codegen_llvm",
            ]),
        );

        let rejected = violations
            .iter()
            .map(|violation| violation.dependency.as_str())
            .collect::<BTreeSet<_>>();
        assert!(rejected.contains("scoopc"));
        assert!(rejected.contains("scoopc_effect_facts"));
        assert!(rejected.contains("scoopc_lir_facts"));
        assert!(rejected.contains("scoopc_codegen_llvm"));
    }

    #[test]
    fn allows_effect_facts_stage_dependency_direction() {
        let violations = find_dependency_violations(
            "scoopc_effect_facts_stage",
            CrateKind::EffectFactsStage,
            &set(&[
                "scoopc_effect_facts_stage",
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_hir_facts",
                "scoopc_mir",
                "scoopc_mir_facts",
                "scoopc_effect_facts",
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
    fn rejects_effect_facts_stage_later_stage_or_facade_dependency() {
        let violations = find_dependency_violations(
            "scoopc_effect_facts_stage",
            CrateKind::EffectFactsStage,
            &set(&[
                "scoopc_effect_facts_stage",
                "scoopc_hir",
                "scoopc_mir",
                "scoopc_effect_facts",
                "scoopc",
                "scoopc_lir",
                "scoopc_lir_facts",
                "scoopc_codegen_llvm",
            ]),
        );

        let rejected = violations
            .iter()
            .map(|violation| violation.dependency.as_str())
            .collect::<BTreeSet<_>>();
        assert!(rejected.contains("scoopc"));
        assert!(rejected.contains("scoopc_lir"));
        assert!(rejected.contains("scoopc_lir_facts"));
        assert!(rejected.contains("scoopc_codegen_llvm"));
    }

    #[test]
    fn allows_lir_stage_direct_dependency_direction() {
        let violations = find_dependency_violations(
            "scoopc_lir",
            CrateKind::LirStage,
            &set(&[
                "scoopc_lir",
                "scoopc_mir",
                "scoopc_mir_facts",
                "scoopc_effect_facts",
                "scoopc_lir_facts",
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
    fn rejects_lir_stage_direct_frontend_or_facade_dependency() {
        let violations = find_dependency_violations(
            "scoopc_lir",
            CrateKind::LirStage,
            &set(&[
                "scoopc_lir",
                "scoopc",
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_hir_facts",
                "scoopc_effect_facts_stage",
                "scoopc_codegen_llvm",
            ]),
        );

        let rejected = violations
            .iter()
            .map(|violation| violation.dependency.as_str())
            .collect::<BTreeSet<_>>();
        assert!(rejected.contains("scoopc"));
        assert!(rejected.contains("scoopc_ast"));
        assert!(rejected.contains("scoopc_hir"));
        assert!(rejected.contains("scoopc_hir_facts"));
        assert!(rejected.contains("scoopc_effect_facts_stage"));
        assert!(rejected.contains("scoopc_codegen_llvm"));
    }

    #[test]
    fn allows_codegen_stage_direct_lir_dependency_direction() {
        let violations = find_dependency_violations(
            "scoopc_codegen_llvm",
            CrateKind::CodegenStage,
            &set(&[
                "scoopc_codegen_llvm",
                "scoopc_lir",
                "scoopc_lir_facts",
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
    fn rejects_codegen_stage_facade_or_earlier_stage_dependency() {
        let violations = find_dependency_violations(
            "scoopc_codegen_llvm",
            CrateKind::CodegenStage,
            &set(&[
                "scoopc_codegen_llvm",
                "scoopc_lir",
                "scoopc_lir_facts",
                "scoopc",
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_mir",
                "scoopc_effect_facts_stage",
                "scoopc_hir_facts",
            ]),
        );

        let rejected = violations
            .iter()
            .map(|violation| violation.dependency.as_str())
            .collect::<BTreeSet<_>>();
        assert!(rejected.contains("scoopc"));
        assert!(rejected.contains("scoopc_ast"));
        assert!(rejected.contains("scoopc_hir"));
        assert!(rejected.contains("scoopc_mir"));
        assert!(rejected.contains("scoopc_effect_facts_stage"));
        assert!(rejected.contains("scoopc_hir_facts"));
    }

    #[test]
    fn allows_cone_to_depend_on_stage_fact_and_base_crates() {
        let violations = find_dependency_violations(
            "scoopc_cone",
            CrateKind::Cone,
            &set(&[
                "scoopc_cone",
                "scoopc_ast",
                "scoopc_hir",
                "scoopc_hir_facts",
                "scoopc_mir",
                "scoopc_mir_facts",
                "scoopc_effect_facts_stage",
                "scoopc_effect_facts",
                "scoopc_lir",
                "scoopc_lir_facts",
                "scoopc_codegen_llvm",
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
    fn rejects_stage_dependency_on_cone() {
        let cases = [
            ("scoopc_ast", CrateKind::BaseOnlyStage),
            ("scoopc_hir", CrateKind::HirStage),
            ("scoopc_mir", CrateKind::MirStage),
            ("scoopc_effect_facts_stage", CrateKind::EffectFactsStage),
            ("scoopc_lir", CrateKind::LirStage),
            ("scoopc_codegen_llvm", CrateKind::CodegenStage),
        ];

        for (crate_name, kind) in cases {
            let violations =
                find_dependency_violations(crate_name, kind, &set(&[crate_name, "scoopc_cone"]));

            assert_eq!(
                violations.len(),
                1,
                "{crate_name} should reject scoopc_cone"
            );
            assert_eq!(violations[0].dependency, "scoopc_cone");
        }
    }

    #[test]
    fn rejects_cone_dependency_on_facade_or_driver_crates() {
        let violations = find_dependency_violations(
            "scoopc_cone",
            CrateKind::Cone,
            &set(&["scoopc_cone", "scoopc", "scoop", "scoop_tools"]),
        );

        let rejected = violations
            .iter()
            .map(|violation| violation.dependency.as_str())
            .collect::<BTreeSet<_>>();
        assert!(rejected.contains("scoopc"));
        assert!(rejected.contains("scoop"));
        assert!(rejected.contains("scoop_tools"));
    }

    #[test]
    fn allows_scoopld_to_depend_on_project_model_only() {
        let violations = find_dependency_violations(
            "scoopld",
            CrateKind::Linker,
            &set(&["scoopld", "scoopc_project_model", "scoopc_span"]),
        );

        assert!(violations.is_empty());
    }

    #[test]
    fn rejects_scoopld_dependency_on_stage_or_facade() {
        let violations = find_dependency_violations(
            "scoopld",
            CrateKind::Linker,
            &set(&[
                "scoopld",
                "scoopc",
                "scoop",
                "scoopc_lir",
                "scoopc_codegen_llvm",
            ]),
        );

        let rejected = violations
            .iter()
            .map(|violation| violation.dependency.as_str())
            .collect::<BTreeSet<_>>();
        assert!(rejected.contains("scoopc"));
        assert!(rejected.contains("scoop"));
        assert!(rejected.contains("scoopc_lir"));
        assert!(rejected.contains("scoopc_codegen_llvm"));
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

    #[test]
    fn excludes_source_tree_paths_by_fragment() {
        assert!(source_path_is_excluded(
            "crates/scoopc_codegen_llvm/src/llvm/tests/baseline.rs",
            &["/tests/", "tests.rs"]
        ));
        assert!(!source_path_is_excluded(
            "crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs",
            &["/tests/", "tests.rs"]
        ));
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }
}
