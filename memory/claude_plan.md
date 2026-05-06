# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not proceed to later tasks after completing or blocking the current task.
- Keep this file updated with the current plan, key discoveries, plan changes, validation results, and final status.

## Initial Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect only the code and tests needed to implement that task correctly.
4. Implement the smallest spec-correct change without workarounds.
5. Run the task-required tests and relevant broader checks.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in the completion record, or add a concrete prerequisite task if blocked.
7. Update this plan file with completion or blocker details.
8. Commit all relevant changes with a task-specific message.
9. Stop after the commit.

## Progress Log

- Started: preparing to identify the first incomplete task from `TODO.md`.
- Identified first incomplete task: `HIR-T01` (`建立 refactor HIR no-Todo verifier 与 stage error 通道`).
- Latest commit is `[HIR-T00] Freeze HIR placeholder inventory`; it is directly relevant as the dependency and does not introduce a separate unfinished prerequisite.
- Next step: inspect the refactor HIR stage, HIR data structures, existing placeholder inventory tests, and `dump-hir` test wiring before editing.
- Inspected `effect_refactor_pipeline/hir_stage.rs`: `TypedHirStageOutput::new` currently collects effect contracts and traversal deliberately ignores `Item::Todo`, `StmtKind::Todo`, `ExprKind::Todo`, and `ExprKind::Missing`.
- Implementation direction: add a refactor HIR completeness verifier in the typed HIR stage, expose a structured `HirStageError` through `HirLowerError`, and make both `hir_stage::run` and direct `TypedHirStageOutput` construction validate by default.
- Implemented the verifier and initial tests. First targeted compile run failed because `HirStageError` was defined but not re-exported from `hir::lower`; fixed the export and will rerun formatting/tests.
- `cargo test -p scoopc --no-default-features refactor_hir_no_todo` passed.
- `cargo test -p scoop --no-default-features dump_hir` exposed an obsolete parity test fixture that now contains local effect declarations and is correctly rejected by the no-Todo verifier. Updated that test to use an existing continuation HIR fixture without declaration placeholders.
- Final validation passed:
  - `cargo test -p scoopc --no-default-features refactor_hir_no_todo`
  - `cargo test -p scoopc --no-default-features refactor_typed_hir`
  - `cargo test -p scoop --no-default-features dump_hir`
  - `cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`
- Updated `TODO.md`: marked `HIR-T01` as `[DONE]` and recorded implementation details plus validation commands.
