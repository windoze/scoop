//! Executable inventory for MIR placeholders.
//!
//! The inventory freezes every placeholder that can currently reach direct-style MIR or be
//! forwarded by generic MIR materialization. Follow-up MIR closure tasks must update this table
//! when they remove an entry or discover a new one.

use super::{BasicBlockId, Body, MirPlaceholderCategory, MirValidationError};
use super::{Operand, Rvalue, StatementKind, TerminatorKind, UnwindAction};
use crate::span::Span;

/// Validate the MIR body shape before it is lifted into LIR instructions.
///
/// This is the MIR-side guard for placeholders that LIR intentionally cannot represent.
pub fn validate_body_for_lir_lift(fqn: &str, body: &Body) -> Result<(), MirValidationError> {
    for (block_index, block) in body.blocks.iter().enumerate() {
        let block_id = BasicBlockId::from_raw(block_index as u32);
        for stmt in &block.stmts {
            match &stmt.kind {
                StatementKind::Assign { value, .. } => {
                    validate_rvalue_for_lir_lift(fqn, block_id, stmt.span, value)?;
                }
                StatementKind::StoreMember { member, .. } if member.resolved.is_none() => {
                    return Err(unresolved_member_error(fqn, block_id, stmt.span));
                }
                StatementKind::StoreMember { .. }
                | StatementKind::StoreTopLevelVar { .. }
                | StatementKind::Nop => {}
                StatementKind::Todo(reason) => {
                    return Err(todo_error(
                        fqn,
                        block_id,
                        stmt.span,
                        MirPlaceholderCategory::Statement,
                        reason,
                    ));
                }
            }
        }

        if let UnwindAction::Todo(reason) = &block.terminator.unwind {
            return Err(todo_error(
                fqn,
                block_id,
                block.terminator.span,
                MirPlaceholderCategory::UnwindAction,
                reason,
            ));
        }

        match &block.terminator.kind {
            TerminatorKind::Todo(reason) => {
                return Err(todo_error(
                    fqn,
                    block_id,
                    block.terminator.span,
                    MirPlaceholderCategory::Terminator,
                    reason,
                ));
            }
            TerminatorKind::CondBr { cond, .. } if !matches!(cond, Operand::Local(_)) => {
                return Err(MirValidationError::TypeContract {
                    fqn: fqn.to_string(),
                    block: Some(block_id),
                    span: block.terminator.span,
                    surface: "branch condition",
                    detail: "branch condition must be lowered to a local before LIR lift",
                });
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Perform { .. }
            | TerminatorKind::Handle { .. } => {}
        }
    }

    Ok(())
}

fn validate_rvalue_for_lir_lift(
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    value: &Rvalue,
) -> Result<(), MirValidationError> {
    match value {
        Rvalue::Todo(reason) => Err(todo_error(
            fqn,
            block,
            span,
            MirPlaceholderCategory::Rvalue,
            reason,
        )),
        Rvalue::UnresolvedName { name } => Err(MirValidationError::TypeContract {
            fqn: fqn.to_string(),
            block: Some(block),
            span,
            surface: "unresolved name",
            detail: if name.is_empty() {
                "unresolved name must be rejected before LIR lift"
            } else {
                "named lookup must be resolved before LIR lift"
            },
        }),
        Rvalue::MemberAccess { member, .. } if member.resolved.is_none() => {
            Err(unresolved_member_error(fqn, block, span))
        }
        Rvalue::Use(_)
        | Rvalue::Transport { .. }
        | Rvalue::TopLevelRef(_)
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::MemberAccess { .. }
        | Rvalue::EnumVariant { .. }
        | Rvalue::ClassCtor { .. }
        | Rvalue::Call { .. }
        | Rvalue::MakeTuple { .. }
        | Rvalue::StructLit { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::KindOf { .. }
        | Rvalue::AlignOf { .. }
        | Rvalue::DescOf { .. }
        | Rvalue::TypeMetadataLiteral(_)
        | Rvalue::InterpolatedString { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. }
        | Rvalue::MakeClosure { .. }
        | Rvalue::PerformResult { .. } => Ok(()),
    }
}

fn todo_error(
    fqn: &str,
    block: BasicBlockId,
    span: Span,
    category: MirPlaceholderCategory,
    reason: &str,
) -> MirValidationError {
    MirValidationError::ProductionTodo {
        fqn: fqn.to_string(),
        block: Some(block),
        span,
        category,
        reason: reason.to_string(),
    }
}

fn unresolved_member_error(fqn: &str, block: BasicBlockId, span: Span) -> MirValidationError {
    MirValidationError::TypeContract {
        fqn: fqn.to_string(),
        block: Some(block),
        span,
        surface: "member access",
        detail: "member access must be resolved before LIR lift",
    }
}

#[cfg(test)]
mod lir_lift_guard_tests {
    use super::*;
    use crate::mir::{
        BasicBlock, ConstValue, LocalDecl, LocalSourceKind, MemberAccessMetadata, Statement,
        Terminator,
    };
    use crate::ty::TypeStore;

    const TEST_FQN: &str = "sample.main";

    fn test_span() -> Span {
        Span::new(10, 20)
    }

    fn body_with_assign(value: Rvalue) -> Body {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let target = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("tmp".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::CompilerTemporary,
        });
        let bb = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: test_span(),
                kind: StatementKind::Assign { target, value },
            }],
            terminator: Terminator {
                span: test_span(),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb;
        body
    }

    #[test]
    fn lir_lift_guard_rejects_unresolved_name() {
        let body = body_with_assign(Rvalue::UnresolvedName {
            name: "missing".to_string(),
        });

        assert_eq!(
            validate_body_for_lir_lift(TEST_FQN, &body),
            Err(MirValidationError::TypeContract {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId::from_raw(0)),
                span: test_span(),
                surface: "unresolved name",
                detail: "named lookup must be resolved before LIR lift",
            })
        );
    }

    #[test]
    fn lir_lift_guard_rejects_unresolved_member_access() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let body = body_with_assign(Rvalue::MemberAccess {
            site_id: None,
            receiver: Operand::Const(ConstValue::Unit),
            member: MemberAccessMetadata {
                name: "value".to_string(),
                receiver_ty: builtins.unit,
                resolved: None,
                hidden_effects: crate::ty::EffectRow::pure(),
            },
        });

        assert_eq!(
            validate_body_for_lir_lift(TEST_FQN, &body),
            Err(MirValidationError::TypeContract {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId::from_raw(0)),
                span: test_span(),
                surface: "member access",
                detail: "member access must be resolved before LIR lift",
            })
        );
    }

    #[test]
    fn lir_lift_guard_accepts_total_body() {
        let body = body_with_assign(Rvalue::Use(Operand::Const(ConstValue::Unit)));

        assert_eq!(validate_body_for_lir_lift(TEST_FQN, &body), Ok(()));
    }
}

#[cfg(test)]
mod inventory_tests {
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
        InMir,
        BeforeMir,
        Reject,
    }

    #[derive(Debug, Clone, Copy)]
    struct InventoryEntry {
        surface: PlaceholderSurface,
        reason: &'static str,
        pipeline_gap: &'static str,
        disposition: PlaceholderDisposition,
        owner_task: &'static str,
        must_eliminate_from_production: bool,
        handling_strategy: &'static str,
    }

    const fn entry(
        surface: PlaceholderSurface,
        reason: &'static str,
        pipeline_gap: &'static str,
        disposition: PlaceholderDisposition,
        owner_task: &'static str,
        must_eliminate_from_production: bool,
        handling_strategy: &'static str,
    ) -> InventoryEntry {
        InventoryEntry {
            surface,
            reason,
            pipeline_gap,
            disposition,
            owner_task,
            must_eliminate_from_production,
            handling_strategy,
        }
    }

    const ALL_DISPOSITIONS: &[PlaceholderDisposition] = &[
        PlaceholderDisposition::InMir,
        PlaceholderDisposition::BeforeMir,
        PlaceholderDisposition::Reject,
    ];

    const MIR_PLACEHOLDER_INVENTORY: &[InventoryEntry] = &[
        entry(
            PlaceholderSurface::Terminator,
            "unterminated",
            "PIPELINE_GAPS.md §2.1",
            PlaceholderDisposition::InMir,
            "MIR-T01",
            true,
            "The builder sentinel must be overwritten before stage output; strict MIR rejects leaks.",
        ),
        entry(
            PlaceholderSurface::Rvalue,
            "missing expr",
            "PIPELINE_GAPS.md §1",
            PlaceholderDisposition::Reject,
            "MIR-T03",
            true,
            "Parser recovery and unsupported HIR fallbacks must become source diagnostics before MIR.",
        ),
    ];

    const REQUIRED_KNOWN_MIR_REASONS: &[&str] = &["missing expr"];

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
    fn mir_placeholder_inventory() {
        assert_inventory_entries_are_actionable();
        assert_pipeline_has_no_placeholder_constructors();

        let expected = MIR_PLACEHOLDER_INVENTORY
            .iter()
            .map(|entry| PlaceholderKey::new(entry.surface, entry.reason))
            .collect::<BTreeSet<_>>();
        let observed = scan_mir_placeholder_surfaces();

        assert_eq!(
            observed, expected,
            "MIR placeholder surfaces changed; update the MIR inventory and ownership map first"
        );
    }

    fn assert_inventory_entries_are_actionable() {
        let mut seen = BTreeSet::new();
        for entry in MIR_PLACEHOLDER_INVENTORY {
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
        }

        for reason in REQUIRED_KNOWN_MIR_REASONS {
            let entry = MIR_PLACEHOLDER_INVENTORY
                .iter()
                .find(|entry| entry.reason == *reason)
                .unwrap_or_else(|| panic!("required MIR placeholder reason is missing: {reason}"));
            assert!(
                entry.must_eliminate_from_production,
                "{reason} must be marked for production elimination"
            );
        }
    }

    fn scan_mir_placeholder_surfaces() -> BTreeSet<PlaceholderKey> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut observed = BTreeSet::new();

        scan_mir_lower_placeholders(&manifest_dir, &mut observed);
        scan_hir_handoff_placeholders(&manifest_dir, &mut observed);

        observed
    }

    fn scan_mir_lower_placeholders(manifest_dir: &Path, observed: &mut BTreeSet<PlaceholderKey>) {
        let dir = manifest_dir.join("src/mir/lower");
        let mut entries = fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name != "tests.rs")
            })
            .collect::<Vec<_>>();
        entries.sort();
        let source: String = entries
            .iter()
            .map(|path| read_non_test_source(path))
            .collect::<Vec<_>>()
            .join("\n");

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

        if source.contains("TerminatorKind::Todo(UNTERMINATED)")
            || source.contains("TerminatorKind::Todo(UNTERMINATED.to_string())")
        {
            observed.insert(PlaceholderKey::new(
                PlaceholderSurface::Terminator,
                "unterminated",
            ));
        }
    }

    fn scan_hir_handoff_placeholders(manifest_dir: &Path, observed: &mut BTreeSet<PlaceholderKey>) {
        let hir_lower_dir = manifest_dir.join("../scoopc_hir/src/hir/lower");
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

    fn assert_pipeline_has_no_placeholder_constructors() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let pipeline_dir = manifest_dir.join("../scoopc/src/pipeline");
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
            "effect pipeline should not construct MIR placeholders directly: {observed:?}"
        );
    }

    fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
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
}
