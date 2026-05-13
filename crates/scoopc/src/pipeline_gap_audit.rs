//! Executable audit baseline for active pipeline gaps.
//!
//! The goal of this module is to freeze one shared audit entry-point for legacy residuals,
//! inventory classification rules, and codegen/doc status drift. Follow-up closure tasks should
//! update these baselines instead of redoing ad hoc repo-wide grep passes.

use std::fs;
use std::path::{Path, PathBuf};

#[cfg(feature = "llvm")]
use crate::llvm::codegen_gap_inventory::{CODEGEN_GAP_INVENTORY, codegen_gap_entry};

const ACTIVE_AUDIT_ROOTS: &[&str] = &["crates/scoopc/src", "crates/scoop/src", "tests/fixtures"];

const LEGACY_BUCKET_MARKER: &str = concat!("Legacy", "Only");

const REASON_ASSIGN_LHS_MISSING_LOCAL: &str = concat!("assign lhs ", "missing local");
const REASON_ASSIGN_LHS_LOWERING_PENDING: &str = concat!("assign lhs ", "lowering pending");
const REASON_CALL_CALLEE_LOWERING_PENDING: &str = concat!("call callee ", "lowering pending");
const REASON_CTOR_CALL_LOWERING_PENDING: &str = concat!("ctor call ", "lowering pending");
const REASON_SIZEOF_REQUIRES_ARG: &str = concat!("sizeOf intrinsic requires ", "value or type arg");
const REASON_NAMEOF_REQUIRES_TYPE_ARG: &str = concat!("nameOf intrinsic requires ", "type arg");
const REASON_RESUME_CANONICAL_CALLEE: &str =
    concat!("resume lowering requires ", "canonical callee shape");
const REASON_DISPATCH_CALLEE_LOWERING_PENDING: &str =
    concat!("dispatch callee ", "lowering pending");

const LEGACY_REASON_LEXICON: &[&str] = &[
    REASON_ASSIGN_LHS_MISSING_LOCAL,
    REASON_ASSIGN_LHS_LOWERING_PENDING,
    REASON_CALL_CALLEE_LOWERING_PENDING,
    REASON_CTOR_CALL_LOWERING_PENDING,
    REASON_SIZEOF_REQUIRES_ARG,
    REASON_NAMEOF_REQUIRES_TYPE_ARG,
    REASON_RESUME_CANONICAL_CALLEE,
    REASON_DISPATCH_CALLEE_LOWERING_PENDING,
];

const EXIT_CONDITION_OPEN_ZERO: &str = "Open = 0";
const EXIT_CONDITION_DEFAULT_PARTIAL_ZERO: &str = "default-mainline Partial = 0";
const EXIT_CONDITION_LEGACY_BUCKET_ZERO: &str = concat!("active code ", "Legacy", "Only", " = 0");

const EXIT_CONDITIONS: &[&str] = &[
    EXIT_CONDITION_OPEN_ZERO,
    EXIT_CONDITION_DEFAULT_PARTIAL_ZERO,
    EXIT_CONDITION_LEGACY_BUCKET_ZERO,
];

struct InventoryClassificationRule {
    name: &'static str,
    meaning: &'static str,
}

const INVENTORY_CLASSIFICATION_RULES: &[InventoryClassificationRule] = &[
    InventoryClassificationRule {
        name: "live contract",
        meaning: "Open inventory work and default-mainline Partial entries that still require an implementation closure.",
    },
    InventoryClassificationRule {
        name: "downstream impossible-state guard",
        meaning: "Executable sentinels that may remain only as impossible-state guards after upstream handoff closes.",
    },
    InventoryClassificationRule {
        name: "frontend reject",
        meaning: "Surfaces that must be rejected before MIR/codegen; backend unsupported is not an allowed substitute.",
    },
    InventoryClassificationRule {
        name: "historical-only mapping",
        meaning: "Legacy-only residue or closed/re-scoped ids kept only for audit and owner mapping, not as default production blockers.",
    },
];

fn expected_legacy_bucket_hits() -> Vec<String> {
    Vec::new()
}

fn expected_legacy_reason_hits() -> Vec<String> {
    Vec::new()
}

#[cfg(feature = "llvm")]
const CODEGEN_SCOPE_DRIFT_BASELINE: &[&str] = &[
    "PIPELINE_GAPS §3.4",
    "PIPELINE_GAPS §3.7",
    "PIPELINE_GAPS §4.1",
    "PIPELINE_GAPS §4.2",
    "PIPELINE_GAPS §5.2",
    "PIPELINE_GAPS §5.5",
    "PIPELINE_GAPS §5.6",
    "PIPELINE_GAPS §5.7",
    "PIPELINE_GAPS §6.1",
    "PIPELINE_GAPS §6.2",
    "PIPELINE_GAPS §6.3",
    "PIPELINE_GAPS §6.4",
    "PIPELINE_GAPS §6.5",
];

#[cfg(feature = "llvm")]
const CODEGEN_SCOPE_SHIFTED_PARTIAL_GAPS: &[&str] = &["PIPELINE_GAPS §4.1"];

#[cfg(feature = "llvm")]
const CODEGEN_CLOSED_RESCOPED_PRODUCTION_BLOCKERS: &[&str] = &[
    "PIPELINE_GAPS §3.4",
    "PIPELINE_GAPS §3.7",
    "PIPELINE_GAPS §4.2",
    "PIPELINE_GAPS §5.2",
    "PIPELINE_GAPS §5.5",
    "PIPELINE_GAPS §5.6",
    "PIPELINE_GAPS §5.7",
    "PIPELINE_GAPS §6.1",
    "PIPELINE_GAPS §6.2",
    "PIPELINE_GAPS §6.3",
    "PIPELINE_GAPS §6.4",
];

#[test]
fn pipeline_gap_audit_records_roots_rules_and_exit_conditions() {
    let repo_root = repo_root();
    for root in ACTIVE_AUDIT_ROOTS {
        assert!(
            repo_root.join(root).is_dir(),
            "audit root should exist: {root}"
        );
    }

    let rule_names = INVENTORY_CLASSIFICATION_RULES
        .iter()
        .map(|rule| rule.name)
        .collect::<Vec<_>>();
    assert_eq!(
        rule_names,
        vec![
            "live contract",
            "downstream impossible-state guard",
            "frontend reject",
            "historical-only mapping",
        ]
    );
    assert_eq!(
        EXIT_CONDITIONS,
        [
            EXIT_CONDITION_OPEN_ZERO,
            EXIT_CONDITION_DEFAULT_PARTIAL_ZERO,
            EXIT_CONDITION_LEGACY_BUCKET_ZERO,
        ]
    );

    println!("audit roots:\n{}", ACTIVE_AUDIT_ROOTS.join("\n"));
    println!(
        "classification rules:\n{}",
        INVENTORY_CLASSIFICATION_RULES
            .iter()
            .map(|rule| format!("{}: {}", rule.name, rule.meaning))
            .collect::<Vec<_>>()
            .join("\n")
    );
    println!("exit conditions:\n{}", EXIT_CONDITIONS.join("\n"));
    println!(
        "legacy reason lexicon:\n{}",
        LEGACY_REASON_LEXICON.join("\n")
    );
}

#[test]
fn pipeline_gap_audit_records_legacyonly_hits() {
    let observed = collect_line_hits(|line| line.contains(LEGACY_BUCKET_MARKER));
    assert_eq!(observed, expected_legacy_bucket_hits());
    println!("{} hits:\n{}", LEGACY_BUCKET_MARKER, observed.join("\n"));
}

#[test]
fn pipeline_gap_audit_records_legacy_reason_hits() {
    let observed = collect_line_hits(|line| {
        LEGACY_REASON_LEXICON
            .iter()
            .any(|reason| line.contains(reason))
    });
    assert_eq!(observed, expected_legacy_reason_hits());
    println!("legacy reason hits:\n{}", observed.join("\n"));
}

#[cfg(feature = "llvm")]
#[test]
fn pipeline_gap_audit_records_codegen_scope_drift_baseline() {
    let statuses = parse_pipeline_gap_statuses();
    let observed = CODEGEN_GAP_INVENTORY
        .iter()
        .filter_map(|entry| {
            let status = statuses.get(entry.gap_id)?;
            (status == "Closed/Re-scoped"
                || CODEGEN_SCOPE_SHIFTED_PARTIAL_GAPS.contains(&entry.gap_id))
            .then(|| entry.gap_id.to_string())
        })
        .collect::<Vec<_>>();

    assert_eq!(observed, expected_lines(CODEGEN_SCOPE_DRIFT_BASELINE));
    for gap_id in CODEGEN_SCOPE_DRIFT_BASELINE {
        let status = statuses
            .get(*gap_id)
            .unwrap_or_else(|| panic!("missing documented status for {gap_id}"));
        let entry = codegen_gap_entry(gap_id)
            .unwrap_or_else(|| panic!("missing codegen inventory entry for {gap_id}"));
        assert!(
            matches!(status.as_str(), "Closed/Re-scoped" | "Partial"),
            "{gap_id} should remain a non-open scope-drift baseline entry, got status {status}"
        );
        println!(
            "scope drift: {gap_id} [{status}] blocker={}",
            entry.production_blocker
        );
    }
}

#[cfg(feature = "llvm")]
#[test]
fn pipeline_gap_audit_records_closed_rescoped_codegen_blockers() {
    let statuses = parse_pipeline_gap_statuses();
    let observed = CODEGEN_GAP_INVENTORY
        .iter()
        .filter_map(|entry| {
            let status = statuses.get(entry.gap_id)?;
            (entry.production_blocker && status == "Closed/Re-scoped")
                .then(|| entry.gap_id.to_string())
        })
        .collect::<Vec<_>>();

    assert_eq!(
        observed,
        expected_lines(CODEGEN_CLOSED_RESCOPED_PRODUCTION_BLOCKERS)
    );
    println!(
        "closed/re-scoped production blockers:\n{}",
        observed.join("\n")
    );
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn expected_lines(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|line| (*line).to_string()).collect()
}

fn collect_line_hits(matches: impl Fn(&str) -> bool) -> Vec<String> {
    let repo_root = repo_root();
    let mut hits = Vec::new();
    let mut files = Vec::new();
    for root in ACTIVE_AUDIT_ROOTS {
        collect_files(&repo_root.join(root), &mut files);
    }
    files.sort();

    for path in files {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "pipeline_gap_audit.rs")
        {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let relative = path
            .strip_prefix(&repo_root)
            .unwrap_or(&path)
            .display()
            .to_string();
        for (index, line) in source.lines().enumerate() {
            if matches(line) {
                hits.push(format!("{relative}:{}:{line}", index + 1));
            }
        }
    }

    hits
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
    {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

#[cfg(feature = "llvm")]
fn parse_pipeline_gap_statuses() -> std::collections::BTreeMap<String, String> {
    const STATUS_PREFIX: &str = "- \u{72b6}\u{6001}\u{ff1a}`";

    let source = fs::read_to_string(repo_root().join("PIPELINE_GAPS.md"))
        .expect("failed to read PIPELINE_GAPS.md");
    let mut statuses = std::collections::BTreeMap::new();
    let mut current_gap = None::<String>;

    for line in source.lines() {
        if let Some(gap_id) = parse_gap_heading(line) {
            current_gap = Some(gap_id);
            continue;
        }
        if let Some(rest) = line.trim().strip_prefix(STATUS_PREFIX)
            && let Some(gap_id) = current_gap.as_ref()
        {
            let end = rest
                .find('`')
                .unwrap_or_else(|| panic!("unterminated status marker for {gap_id}"));
            statuses.insert(gap_id.clone(), rest[..end].to_string());
        }
    }

    statuses
}

#[cfg(feature = "llvm")]
fn parse_gap_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed
        .strip_prefix("### ")
        .or_else(|| trimmed.strip_prefix("## "))?;
    let token = rest.split_whitespace().next()?.trim_end_matches('.');
    token
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == '.')
        .then(|| format!("PIPELINE_GAPS §{token}"))
}
