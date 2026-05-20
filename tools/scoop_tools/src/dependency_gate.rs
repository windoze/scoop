//! Dependency gate for pipeline-refactor base crates.
//!
//! The gate checks each base crate from the package's own `cargo tree` view.
//! That catches accidental reverse dependencies from a base crate back to the
//! `scoopc` facade, stage crates, fact crates, driver/runtime crates, or a later
//! base crate in the P1 dependency direction.

use std::collections::BTreeSet;
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
}

impl Report {
    pub fn render(&self) -> String {
        if self.has_violations() {
            return self.render_failures();
        }

        let mut lines = vec![format!(
            "dependency gate: ok (checked {} base crates)",
            self.checks.len()
        )];
        for check in &self.checks {
            let base_dependencies = check.base_dependencies();
            let rendered = if base_dependencies.is_empty() {
                "none".to_string()
            } else {
                base_dependencies.join(", ")
            };
            lines.push(format!("- {}: base deps [{}]", check.crate_name, rendered));
        }
        lines.join("\n")
    }

    fn has_violations(&self) -> bool {
        self.checks.iter().any(|check| !check.violations.is_empty())
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
        lines.join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CrateCheck {
    crate_name: String,
    dependency_names: BTreeSet<String>,
    violations: Vec<Violation>,
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

pub fn run() -> Result<Report> {
    let workspace_root = workspace_root()?;
    let mut checks = Vec::new();

    for crate_name in BASE_CRATES {
        let dependency_names = cargo_tree_package_names(crate_name, &workspace_root)?;
        let violations = find_dependency_violations(crate_name, &dependency_names);
        checks.push(CrateCheck {
            crate_name: (*crate_name).to_string(),
            dependency_names,
            violations,
        });
    }

    let report = Report { checks };
    if report.has_violations() {
        return Err(miette::miette!("{}", report.render_failures()));
    }
    Ok(report)
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

fn find_dependency_violations(base: &str, dependency_names: &BTreeSet<String>) -> Vec<Violation> {
    let allowed_base_dependencies: BTreeSet<&str> =
        allowed_base_dependencies(base).iter().copied().collect();
    let mut violations = Vec::new();

    for dependency in dependency_names {
        let dependency_name = dependency.as_str();
        if dependency_name == base {
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
            &set(&["scoopc_source", "scoopc_span", "scoopc"]),
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].dependency, "scoopc");
    }

    #[test]
    fn rejects_later_base_dependency() {
        let violations = find_dependency_violations(
            "scoopc_span",
            &set(&["scoopc_span", "scoopc_project_model"]),
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].dependency, "scoopc_project_model");
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }
}
