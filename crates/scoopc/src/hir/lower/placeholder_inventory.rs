//! Executable inventory for refactor HIR placeholders.
//!
//! The refactor typed HIR handoff currently still reuses the generic `LoweredHir` shape, so this
//! test fixture freezes every placeholder constructor that can be emitted from `src/hir/**` until
//! the follow-up HIR completeness tasks replace them with diagnostics or concrete contracts.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PlaceholderSurface {
    ExprTodo,
    StmtTodo,
    ItemTodo,
    ExprMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaceholderDisposition {
    ParserShouldReject,
    TypecheckOrComptimeShouldDiagnose,
    HirShouldImplement,
    HirHandoffContractShouldPublish,
    LegacyOnlyCanIgnore,
}

#[derive(Debug, Clone, Copy)]
struct InventoryEntry {
    surface: PlaceholderSurface,
    reason: &'static str,
    disposition: PlaceholderDisposition,
    owner_task: &'static str,
    must_eliminate_from_refactor_hir: bool,
    handling_strategy: &'static str,
}

const fn entry(
    surface: PlaceholderSurface,
    reason: &'static str,
    disposition: PlaceholderDisposition,
    owner_task: &'static str,
    must_eliminate_from_refactor_hir: bool,
    handling_strategy: &'static str,
) -> InventoryEntry {
    InventoryEntry {
        surface,
        reason,
        disposition,
        owner_task,
        must_eliminate_from_refactor_hir,
        handling_strategy,
    }
}

const ALL_DISPOSITIONS: &[PlaceholderDisposition] = &[
    PlaceholderDisposition::ParserShouldReject,
    PlaceholderDisposition::TypecheckOrComptimeShouldDiagnose,
    PlaceholderDisposition::HirShouldImplement,
    PlaceholderDisposition::HirHandoffContractShouldPublish,
    PlaceholderDisposition::LegacyOnlyCanIgnore,
];

const REFACTOR_HIR_PLACEHOLDER_INVENTORY: &[InventoryEntry] = &[
    entry(
        PlaceholderSurface::ExprTodo,
        "structured_concurrency_spawn_deferred",
        PlaceholderDisposition::ParserShouldReject,
        "HIR-T02",
        true,
        "Parser emits a deferred-feature diagnostic for user-facing spawn before HIR lowering.",
    ),
    entry(
        PlaceholderSurface::ExprTodo,
        "structured_concurrency_join_deferred",
        PlaceholderDisposition::ParserShouldReject,
        "HIR-T02",
        true,
        "Parser emits a deferred-feature diagnostic for user-facing join before HIR lowering.",
    ),
    entry(
        PlaceholderSurface::ExprTodo,
        "assign",
        PlaceholderDisposition::ParserShouldReject,
        "HIR-T02/HIR-T10",
        true,
        "Parser rejects assignment in expression context; statement assignment later consumes a typed place contract.",
    ),
    entry(
        PlaceholderSurface::ExprTodo,
        "with_update",
        PlaceholderDisposition::HirHandoffContractShouldPublish,
        "HIR-T09",
        true,
        "Typecheck publishes copy-update aggregate metadata and HIR lowers every supported aggregate shape.",
    ),
    entry(
        PlaceholderSurface::StmtTodo,
        "missing_stmt",
        PlaceholderDisposition::ParserShouldReject,
        "HIR-T11",
        true,
        "Parser recovery Missing statements must not enter successful refactor HIR lowering.",
    ),
    entry(
        PlaceholderSurface::StmtTodo,
        "comptime_block",
        PlaceholderDisposition::TypecheckOrComptimeShouldDiagnose,
        "HIR-T03",
        true,
        "Comptime expansion resolves block declarations/generated code before runtime HIR lowering.",
    ),
    entry(
        PlaceholderSurface::StmtTodo,
        "comptime_if",
        PlaceholderDisposition::TypecheckOrComptimeShouldDiagnose,
        "HIR-T03",
        true,
        "Comptime expansion evaluates the condition and keeps only the selected runtime branch.",
    ),
    entry(
        PlaceholderSurface::StmtTodo,
        "comptime_for",
        PlaceholderDisposition::TypecheckOrComptimeShouldDiagnose,
        "HIR-T03",
        true,
        "Comptime expansion unrolls compile-time iteration into ordinary runtime statements/items.",
    ),
    entry(
        PlaceholderSurface::StmtTodo,
        "for_custom_iterator",
        PlaceholderDisposition::HirShouldImplement,
        "HIR-T11",
        true,
        "HIR consumes the typechecked iterator/next contract or reports a missing-contract stage error.",
    ),
    entry(
        PlaceholderSurface::ItemTodo,
        "comptime_if_item",
        PlaceholderDisposition::TypecheckOrComptimeShouldDiagnose,
        "HIR-T03",
        true,
        "Package-level comptime if is expanded or diagnosed before top-level HIR items are built.",
    ),
    entry(
        PlaceholderSurface::ExprMissing,
        "missing_expr",
        PlaceholderDisposition::HirHandoffContractShouldPublish,
        "HIR-T01/HIR-T11",
        true,
        "Verifier rejects parser-recovery Missing expressions; synthesized Missing receivers must become explicit contracts.",
    ),
];

const REQUIRED_ELIMINATION_REASONS: &[&str] = &[
    "comptime_block",
    "comptime_if",
    "comptime_for",
    "comptime_if_item",
    "structured_concurrency_spawn_deferred",
    "structured_concurrency_join_deferred",
    "assign",
    "with_update",
    "missing_stmt",
    "for_custom_iterator",
];

const EXPR_TODO_PATTERN: &str = concat!("ExprKind", "::Todo(\"");
const STMT_TODO_PATTERN: &str = concat!("StmtKind", "::Todo(\"");
const ITEM_TODO_PATTERN: &str = concat!("Item", "::Todo");

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
fn refactor_hir_placeholder_inventory() {
    assert_inventory_entries_are_actionable();

    let expected = REFACTOR_HIR_PLACEHOLDER_INVENTORY
        .iter()
        .map(|entry| PlaceholderKey::new(entry.surface, entry.reason))
        .collect::<BTreeSet<_>>();
    let observed = scan_hir_placeholder_constructors();

    assert_eq!(
        observed, expected,
        "HIR placeholder constructors changed; update the refactor HIR inventory and classification first"
    );

    assert_refactor_hir_stage_does_not_use_legacy_dump_fallback();
}

fn assert_inventory_entries_are_actionable() {
    assert!(
        ALL_DISPOSITIONS.contains(&PlaceholderDisposition::LegacyOnlyCanIgnore),
        "inventory classification set must keep the legacy-only bucket explicit even when unused"
    );

    let mut seen = BTreeSet::new();
    for entry in REFACTOR_HIR_PLACEHOLDER_INVENTORY {
        let key = PlaceholderKey::new(entry.surface, entry.reason);
        assert!(seen.insert(key), "duplicate inventory entry: {entry:?}");
        assert!(
            !entry.owner_task.is_empty(),
            "inventory entry lacks owner task: {entry:?}"
        );
        assert!(
            !entry.handling_strategy.is_empty(),
            "inventory entry lacks handling strategy: {entry:?}"
        );
        assert!(
            entry.disposition != PlaceholderDisposition::LegacyOnlyCanIgnore
                || !entry.must_eliminate_from_refactor_hir,
            "legacy-only entries must not be required refactor eliminations: {entry:?}"
        );
    }

    for reason in REQUIRED_ELIMINATION_REASONS {
        let entry = REFACTOR_HIR_PLACEHOLDER_INVENTORY
            .iter()
            .find(|entry| entry.reason == *reason)
            .unwrap_or_else(|| panic!("required placeholder reason is missing: {reason}"));
        assert!(
            entry.must_eliminate_from_refactor_hir,
            "{reason} must be marked for refactor HIR elimination"
        );
    }
}

fn scan_hir_placeholder_constructors() -> BTreeSet<PlaceholderKey> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let hir_dir = manifest_dir.join("src/hir");
    let mut files = Vec::new();
    collect_rust_files(&hir_dir, &mut files);

    let mut placeholders = BTreeSet::new();
    for path in files {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "placeholder_inventory.rs")
        {
            continue;
        }

        let source = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        collect_string_reason_constructors(
            &source,
            EXPR_TODO_PATTERN,
            PlaceholderSurface::ExprTodo,
            &mut placeholders,
        );
        collect_string_reason_constructors(
            &source,
            STMT_TODO_PATTERN,
            PlaceholderSurface::StmtTodo,
            &mut placeholders,
        );
        collect_item_todo_constructors(&source, &mut placeholders);

        if source.contains("(ExprKind::Missing,") || source.contains("kind: ExprKind::Missing") {
            placeholders.insert(PlaceholderKey::new(
                PlaceholderSurface::ExprMissing,
                "missing_expr",
            ));
        }
    }

    placeholders
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

fn collect_item_todo_constructors(source: &str, out: &mut BTreeSet<PlaceholderKey>) {
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
            out.insert(PlaceholderKey::new(
                PlaceholderSurface::ItemTodo,
                &after_kind[..end],
            ));
        }
        remaining = after_item;
    }
}

fn assert_refactor_hir_stage_does_not_use_legacy_dump_fallback() {
    let hir_stage = include_str!("../../effect_refactor_pipeline/hir_stage.rs");
    assert!(
        hir_stage.contains("crate::hir::lower_typed_for_dump(session, source)"),
        "refactor typed HIR stage must enter the typed HIR lowering entrypoint"
    );
    assert!(
        !hir_stage.contains("crate::hir::lower_for_dump(session, source)"),
        "refactor typed HIR stage must not carry the legacy dump-only HIR fallback"
    );
    assert!(
        hir_stage.contains("format!(\"{:#?}\\n\", self.hir_file())"),
        "refactor stable dump should render the stage-owned typed HIR file"
    );
}
