//! Executable inventory for refactor MIR placeholders.
//!
//! The inventory freezes every placeholder that can currently reach direct-style MIR or be
//! forwarded by generic MIR materialization. Follow-up MIR closure tasks must update this table
//! when they remove an entry or discover a new one.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PlaceholderSurface {
    Item,
    Statement,
    Rvalue,
    Terminator,
    UnwindAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaceholderDisposition {
    ImplementInMir,
    ImplementBeforeMir,
    RejectBeforeMir,
    LegacyOnly,
}

#[derive(Debug, Clone, Copy)]
struct InventoryEntry {
    surface: PlaceholderSurface,
    reason: &'static str,
    pipeline_gap: &'static str,
    disposition: PlaceholderDisposition,
    owner_task: &'static str,
    must_eliminate_from_refactor_production: bool,
    handling_strategy: &'static str,
}

const fn entry(
    surface: PlaceholderSurface,
    reason: &'static str,
    pipeline_gap: &'static str,
    disposition: PlaceholderDisposition,
    owner_task: &'static str,
    must_eliminate_from_refactor_production: bool,
    handling_strategy: &'static str,
) -> InventoryEntry {
    InventoryEntry {
        surface,
        reason,
        pipeline_gap,
        disposition,
        owner_task,
        must_eliminate_from_refactor_production,
        handling_strategy,
    }
}

const ALL_DISPOSITIONS: &[PlaceholderDisposition] = &[
    PlaceholderDisposition::ImplementInMir,
    PlaceholderDisposition::ImplementBeforeMir,
    PlaceholderDisposition::RejectBeforeMir,
    PlaceholderDisposition::LegacyOnly,
];

const REFACTOR_MIR_PLACEHOLDER_INVENTORY: &[InventoryEntry] = &[
    entry(
        PlaceholderSurface::Item,
        "top-level val",
        "PIPELINE_GAPS.md §1.4",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T05",
        true,
        "Model top-level values as MIR declaration/initializer roots instead of Item::Todo.",
    ),
    entry(
        PlaceholderSurface::Statement,
        "comptime_block",
        "PIPELINE_GAPS.md §1.1",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T04",
        true,
        "Comptime expansion eliminates block statements before runtime MIR lowering.",
    ),
    entry(
        PlaceholderSurface::Statement,
        "comptime_if",
        "PIPELINE_GAPS.md §1.1",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T04",
        true,
        "Comptime expansion keeps only the selected runtime branch before MIR lowering.",
    ),
    entry(
        PlaceholderSurface::Statement,
        "comptime_for",
        "PIPELINE_GAPS.md §1.1",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T04",
        true,
        "Comptime expansion unrolls enumerable loops before MIR lowering.",
    ),
    entry(
        PlaceholderSurface::Terminator,
        "unterminated",
        "PIPELINE_GAPS.md §2.1",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T01",
        true,
        "The builder sentinel must be overwritten before stage output; strict MIR rejects leaks.",
    ),
    entry(
        PlaceholderSurface::Terminator,
        "break not in loop",
        "PIPELINE_GAPS.md §1",
        PlaceholderDisposition::RejectBeforeMir,
        "MIR-T06",
        true,
        "Frontend control-flow checks diagnose break outside loops before MIR lowering.",
    ),
    entry(
        PlaceholderSurface::Terminator,
        "continue not in loop",
        "PIPELINE_GAPS.md §1",
        PlaceholderDisposition::RejectBeforeMir,
        "MIR-T06",
        true,
        "Frontend control-flow checks diagnose continue outside loops before MIR lowering.",
    ),
    entry(
        PlaceholderSurface::Statement,
        "val decl missing symbol id",
        "PIPELINE_GAPS.md §1",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T06",
        true,
        "Typed HIR handoff must publish symbol ids for local declarations consumed by MIR.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "boxed var decl init pending",
        "PIPELINE_GAPS.md §1.6",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T06",
        true,
        "Boxed mutable locals without initializers need an explicit init/default contract.",
    ),
    entry(
        PlaceholderSurface::Statement,
        "assign lhs missing local",
        "PIPELINE_GAPS.md §1.6",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T06",
        true,
        "Assignment lowering must consume a typed local/place contract instead of falling back.",
    ),
    entry(
        PlaceholderSurface::Statement,
        "assign lhs lowering pending",
        "PIPELINE_GAPS.md §1.6",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T06",
        true,
        "All assignable surfaces accepted by typecheck need a unified MIR place/store model.",
    ),
    entry(
        PlaceholderSurface::Statement,
        "assign place contract missing",
        "PIPELINE_GAPS.md §1.6",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T06",
        true,
        "Typed HIR must publish authoritative assignment place contracts for refactor MIR.",
    ),
    entry(
        PlaceholderSurface::Statement,
        "assign place local missing",
        "PIPELINE_GAPS.md §1.6",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T06",
        true,
        "MIR local allocation and assignment place contracts must agree on source symbol ids.",
    ),
    entry(
        PlaceholderSurface::Statement,
        "assign place member receiver missing",
        "PIPELINE_GAPS.md §1.6",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T06",
        true,
        "Member assignment contracts must retain the receiver expression route used by MIR.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "missing expr",
        "PIPELINE_GAPS.md §1",
        PlaceholderDisposition::RejectBeforeMir,
        "MIR-T03",
        true,
        "Parser recovery and unsupported HIR fallbacks must become source diagnostics before MIR.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "class literal MIR lowering pending",
        "PIPELINE_GAPS.md §1.3",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T04",
        true,
        "Class literals are either lowered to metadata/string primitives or rejected before MIR.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "unbound local ref",
        "PIPELINE_GAPS.md §1",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T06",
        true,
        "Typed HIR symbol resolution must prevent unbound local references from reaching MIR.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "call callee lowering pending",
        "PIPELINE_GAPS.md §1.7",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T07",
        true,
        "Call lowering must consume typed call-site contracts for callee provenance.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "ctor call lowering pending",
        "PIPELINE_GAPS.md §1.7",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T07",
        true,
        "Constructor lowering must consume selected constructor and complete argument bindings.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "sizeOf intrinsic requires one positional arg",
        "PIPELINE_GAPS.md §6.3",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T07",
        true,
        "Reflection intrinsics need explicit MIR intrinsic metadata or frontend rejection.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "resume lowering requires canonical callee shape",
        "PIPELINE_GAPS.md §1.9",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T08",
        true,
        "Resume calls must be driven by typed resume-site metadata, not canonical callee shape.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "dispatch callee lowering pending",
        "PIPELINE_GAPS.md §1.8",
        PlaceholderDisposition::ImplementInMir,
        "MIR-T08",
        true,
        "Dispatch metadata must come from structured owner/member bindings.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "refactor perform contract missing",
        "PIPELINE_GAPS.md §1.10",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T08",
        true,
        "Typed handoff must publish perform-site payload/result/resume metadata.",
    ),
    entry(
        PlaceholderSurface::Terminator,
        "refactor perform contract missing",
        "PIPELINE_GAPS.md §1.10",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T08",
        true,
        "Missing perform metadata must be a stage diagnostic rather than a terminator Todo.",
    ),
    entry(
        PlaceholderSurface::Rvalue,
        "refactor handle contract missing",
        "PIPELINE_GAPS.md §1.11",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T08",
        true,
        "Typed handoff must publish handle-site body/arm/finally metadata.",
    ),
    entry(
        PlaceholderSurface::Terminator,
        "refactor handle contract missing",
        "PIPELINE_GAPS.md §1.11",
        PlaceholderDisposition::ImplementBeforeMir,
        "MIR-T08",
        true,
        "Missing handle metadata must be a stage diagnostic rather than a terminator Todo.",
    ),
];

const REQUIRED_KNOWN_MIR_REASONS: &[&str] = &[
    "top-level val",
    "assign lhs lowering pending",
    "assign place contract missing",
    "call callee lowering pending",
    "ctor call lowering pending",
    "resume lowering requires canonical callee shape",
    "dispatch callee lowering pending",
    "refactor perform contract missing",
    "refactor handle contract missing",
    "missing expr",
    "boxed var decl init pending",
    "break not in loop",
    "continue not in loop",
];

const ITEM_TODO_PATTERN: &str = concat!("Item", "::Todo");
const STATEMENT_TODO_PATTERN: &str = concat!("StatementKind", "::Todo(\"");
const RVALUE_TODO_PATTERN: &str = concat!("Rvalue", "::Todo(\"");
const TERMINATOR_TODO_PATTERN: &str = concat!("TerminatorKind", "::Todo(\"");
const UNWIND_TODO_PATTERN: &str = concat!("UnwindAction", "::Todo(\"");
const HIR_STMT_TODO_PATTERN: &str = concat!("StmtKind", "::Todo(\"");
const HIR_EXPR_TODO_PATTERN: &str = concat!("ExprKind", "::Todo(\"");

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PlaceholderKey {
    surface: PlaceholderSurface,
    reason: String,
}

impl PlaceholderKey {
    fn new(surface: PlaceholderSurface, reason: impl Into<String>) -> Self {
        Self {
            surface,
            reason: reason.into(),
        }
    }
}

#[test]
fn refactor_mir_placeholder_inventory() {
    assert_inventory_entries_are_actionable();
    assert_effect_refactor_pipeline_has_no_placeholder_constructors();

    let expected = REFACTOR_MIR_PLACEHOLDER_INVENTORY
        .iter()
        .map(|entry| PlaceholderKey::new(entry.surface, entry.reason))
        .collect::<BTreeSet<_>>();
    let observed = scan_refactor_mir_placeholder_surfaces();

    assert_eq!(
        observed, expected,
        "MIR placeholder surfaces changed; update the refactor MIR inventory and ownership map first"
    );
}

fn assert_inventory_entries_are_actionable() {
    assert!(
        ALL_DISPOSITIONS.contains(&PlaceholderDisposition::LegacyOnly),
        "inventory disposition set must keep the legacy-only bucket explicit"
    );

    let mut seen = BTreeSet::new();
    for entry in REFACTOR_MIR_PLACEHOLDER_INVENTORY {
        let key = PlaceholderKey::new(entry.surface, entry.reason);
        assert!(seen.insert(key), "duplicate inventory entry: {entry:?}");
        assert!(
            ALL_DISPOSITIONS.contains(&entry.disposition),
            "inventory entry has unknown disposition: {entry:?}"
        );
        assert!(
            !entry.pipeline_gap.is_empty(),
            "inventory entry lacks PIPELINE_GAPS mapping: {entry:?}"
        );
        assert!(
            !entry.owner_task.is_empty(),
            "inventory entry lacks owner task: {entry:?}"
        );
        assert!(
            !entry.handling_strategy.is_empty(),
            "inventory entry lacks handling strategy: {entry:?}"
        );
        assert!(
            entry.disposition != PlaceholderDisposition::LegacyOnly
                || !entry.must_eliminate_from_refactor_production,
            "legacy-only entries must not be required refactor eliminations: {entry:?}"
        );
    }

    for reason in REQUIRED_KNOWN_MIR_REASONS {
        let entry = REFACTOR_MIR_PLACEHOLDER_INVENTORY
            .iter()
            .find(|entry| entry.reason == *reason)
            .unwrap_or_else(|| panic!("required MIR placeholder reason is missing: {reason}"));
        assert!(
            entry.must_eliminate_from_refactor_production,
            "{reason} must be marked for refactor production elimination"
        );
    }
}

fn scan_refactor_mir_placeholder_surfaces() -> BTreeSet<PlaceholderKey> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut observed = BTreeSet::new();

    scan_mir_lower_placeholders(&manifest_dir, &mut observed);
    scan_hir_handoff_placeholders(&manifest_dir, &mut observed);
    assert_materializer_rejects_todo_rewrites(&manifest_dir);

    observed
}

fn scan_mir_lower_placeholders(manifest_dir: &Path, observed: &mut BTreeSet<PlaceholderKey>) {
    let path = manifest_dir.join("src/mir/lower.rs");
    let source = read_non_test_source(&path);

    collect_item_todo_constructors(&source, PlaceholderSurface::Item, observed);
    collect_string_reason_constructors(
        &source,
        STATEMENT_TODO_PATTERN,
        PlaceholderSurface::Statement,
        observed,
    );
    collect_string_reason_constructors(
        &source,
        RVALUE_TODO_PATTERN,
        PlaceholderSurface::Rvalue,
        observed,
    );
    collect_string_reason_constructors(
        &source,
        TERMINATOR_TODO_PATTERN,
        PlaceholderSurface::Terminator,
        observed,
    );
    collect_string_reason_constructors(
        &source,
        UNWIND_TODO_PATTERN,
        PlaceholderSurface::UnwindAction,
        observed,
    );
    collect_emit_todo_value_calls(&source, observed);

    if source.contains("TerminatorKind::Todo(UNTERMINATED)") {
        observed.insert(PlaceholderKey::new(
            PlaceholderSurface::Terminator,
            "unterminated",
        ));
    }
}

fn scan_hir_handoff_placeholders(manifest_dir: &Path, observed: &mut BTreeSet<PlaceholderKey>) {
    let hir_lower_dir = manifest_dir.join("src/hir/lower");
    let mut files = Vec::new();
    collect_rust_files(&hir_lower_dir, &mut files);
    for path in files {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "placeholder_inventory.rs")
        {
            continue;
        }
        let source = read_non_test_source(&path);
        collect_item_todo_constructors(&source, PlaceholderSurface::Item, observed);
        collect_string_reason_constructors(
            &source,
            HIR_STMT_TODO_PATTERN,
            PlaceholderSurface::Statement,
            observed,
        );
        collect_string_reason_constructors(
            &source,
            HIR_EXPR_TODO_PATTERN,
            PlaceholderSurface::Rvalue,
            observed,
        );
    }
}

fn assert_materializer_rejects_todo_rewrites(manifest_dir: &Path) {
    let path = manifest_dir.join("src/mir/materialize.rs");
    let source = read_non_test_source(&path);

    assert!(
        !source.contains("StatementKind::Nop | StatementKind::Todo(_) => {}"),
        "materializer must reject StatementKind::Todo instead of no-op rewriting it"
    );
    assert!(
        !source.contains("| TerminatorKind::Todo(_) => {}"),
        "materializer must reject TerminatorKind::Todo instead of no-op rewriting it"
    );
    assert!(
        !source.contains("Rvalue::Todo(_) => {}"),
        "materializer must reject Rvalue::Todo instead of no-op rewriting it"
    );
    assert!(
        !source.contains("fn rewrite_terminator(") || source.contains("terminator.unwind"),
        "materializer must inspect terminator.unwind so UnwindAction::Todo cannot pass through"
    );
}

fn assert_effect_refactor_pipeline_has_no_placeholder_constructors() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let pipeline_dir = manifest_dir.join("src/effect_refactor_pipeline");
    let mut files = Vec::new();
    collect_rust_files(&pipeline_dir, &mut files);

    let mut observed = BTreeSet::new();
    for path in files {
        let source = read_non_test_source(&path);
        collect_item_todo_constructors(&source, PlaceholderSurface::Item, &mut observed);
        collect_string_reason_constructors(
            &source,
            STATEMENT_TODO_PATTERN,
            PlaceholderSurface::Statement,
            &mut observed,
        );
        collect_string_reason_constructors(
            &source,
            RVALUE_TODO_PATTERN,
            PlaceholderSurface::Rvalue,
            &mut observed,
        );
        collect_string_reason_constructors(
            &source,
            TERMINATOR_TODO_PATTERN,
            PlaceholderSurface::Terminator,
            &mut observed,
        );
        collect_string_reason_constructors(
            &source,
            UNWIND_TODO_PATTERN,
            PlaceholderSurface::UnwindAction,
            &mut observed,
        );
    }

    assert!(
        observed.is_empty(),
        "effect refactor pipeline should not construct MIR placeholders directly: {observed:?}"
    );
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
    {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "rs")
        {
            out.push(path);
        }
    }
}

fn read_non_test_source(path: &Path) -> String {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    strip_cfg_test_tail(&source).to_string()
}

fn strip_cfg_test_tail(source: &str) -> &str {
    const TEST_MODULE: &str = "#[cfg(test)]\nmod tests";

    source
        .find(TEST_MODULE)
        .map_or(source, |index| &source[..index])
}

fn collect_string_reason_constructors(
    source: &str,
    pattern: &str,
    surface: PlaceholderSurface,
    out: &mut BTreeSet<PlaceholderKey>,
) {
    let mut remaining = source;
    while let Some(index) = remaining.find(pattern) {
        let after_pattern = &remaining[index + pattern.len()..];
        let end = after_pattern
            .find('"')
            .unwrap_or_else(|| panic!("unterminated placeholder reason after {pattern}"));
        out.insert(PlaceholderKey::new(surface, &after_pattern[..end]));
        remaining = &after_pattern[end + 1..];
    }
}

fn collect_emit_todo_value_calls(source: &str, out: &mut BTreeSet<PlaceholderKey>) {
    let mut remaining = source;
    while let Some(index) = remaining.find("self.emit_todo_value(") {
        let after_call = &remaining[index + "self.emit_todo_value(".len()..];
        if let Some(reason_start) = after_call.find("\"") {
            let after_quote = &after_call[reason_start + 1..];
            let end = after_quote
                .find('"')
                .unwrap_or_else(|| panic!("unterminated emit_todo_value reason"));
            out.insert(PlaceholderKey::new(
                PlaceholderSurface::Rvalue,
                &after_quote[..end],
            ));
        }
        remaining = after_call;
    }
}

fn collect_item_todo_constructors(
    source: &str,
    surface: PlaceholderSurface,
    out: &mut BTreeSet<PlaceholderKey>,
) {
    let mut remaining = source;
    while let Some(index) = remaining.find(ITEM_TODO_PATTERN) {
        let after_item = &remaining[index + ITEM_TODO_PATTERN.len()..];
        let block_end = after_item.find('}').unwrap_or(after_item.len());
        let block = &after_item[..block_end];
        if let Some(kind_index) = block.find("kind: \"") {
            let after_kind = &block[kind_index + "kind: \"".len()..];
            let end = after_kind
                .find('"')
                .unwrap_or_else(|| panic!("unterminated item todo kind"));
            out.insert(PlaceholderKey::new(surface, &after_kind[..end]));
        }
        remaining = after_item;
    }
}
