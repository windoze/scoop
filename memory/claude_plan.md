# Claude Plan

## Scope
- Execute exactly the first incomplete task in `TODO.md`, then stop.
- Treat `TODO.md` as authoritative for task order, dependencies, validation, and completion records.
- Do not perform broad historical triage before identifying the current task.

## Execution Plan
1. Read `TODO.md` and identify the first heading that is not prefixed with `[DONE]`.
2. Check the latest commit only for an explicitly mentioned unfinished issue that directly affects that task.
3. Inspect only the files and implementation areas needed for the selected task.
4. Implement the task as written, without narrowing scope or using fixture-only workarounds.
5. Add or update the smallest relevant tests/fixtures required by the task.
6. Run the task-specified validations and relevant regression tests.
7. If a concrete blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
8. If implementation succeeds, mark the task title `[DONE]` in `TODO.md` and update its completion record.
9. Commit all relevant changes with a clear task-tagged commit message.
10. Stop without starting the next task.

## Current Progress
- Plan initialized before task selection.
- First incomplete task selected: `MIR-T05：建立完整 MIR program item graph 与 top-level roots`.
- Latest commit references the same prerequisite relationship (`[MIR-T04] Record top-level roots prerequisite`), so it is directly relevant and confirms `MIR-T05` is the current execution unit.

## Current Task Plan: MIR-T05
1. Inspect the existing MIR item model, MIR lowering for top-level items, root/materialization indexes, and refactor stage output APIs.
2. Implement non-executable MIR declarations and executable initializer/root metadata for top-level values and related declarations.
3. Ensure `hir::Item::Val` no longer lowers to `Item::Todo` on the refactor production path.
4. Add query APIs for callable roots, initializer roots, global/extern roots, and nominal/object/typealias metadata roots.
5. Add/update targeted tests and fixture `tests/fixtures/mir_refactor/top_level_roots.scoop`.
6. Run the required validations for `MIR-T05` and fix any task-scoped regressions.
7. Mark `MIR-T05` `[DONE]` in `TODO.md`, update its completion record, then commit all relevant changes.

## Discoveries
- HIR refactor stage already publishes `TopLevelInitRootContract`, `ExternGlobalContract`, and a declaration graph in `hir::File::decls`.
- MIR lowering still emits `Item::Todo { kind: "top-level val" }` for `hir::Item::Val`, which is the direct blocker for `MIR-T05` and `MIR-T04`'s splice dump command.
- The minimal correct route is to copy those typed HIR contracts into explicit MIR item graph entries during refactor MIR lowering, then build query APIs from MIR items instead of HIR side tables.

## Progress
- Added MIR item graph entries for initializer roots, extern globals, and declaration metadata roots.
- Refactor MIR lowering now publishes HIR declaration graph and typed top-level/global contracts as MIR items, and skips the old top-level `val` Todo on the refactor path.
- Added `RefactorMirStageOutput` query APIs for initializer, global, and metadata roots.
- Added `refactor_mir_item_graph_publishes_top_level_roots` and `tests/fixtures/mir_refactor/top_level_roots.scoop`.
- Validation passed so far:
  - `cargo test -p scoopc --no-default-features refactor_mir_item_graph`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/top_level_roots.scoop`
  - `cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`
  - `cargo test -p scoopc --no-default-features refactor_mir_no_todo`
  - `cargo test -p scoopc --no-default-features refactor_materialized_mir`
  - `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/comptime/splice_field_access_v0_basic.scoop`
- `TODO.md` now marks `MIR-T05` as `[DONE]` with completion records and validation logs.
