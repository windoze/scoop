# Execution Plan

I will follow `TODO.md` as the source of truth, complete exactly the first task whose heading is not prefixed with `[DONE]`, validate it, update task records, commit the result, and stop.

## Steps

1. Read `TODO.md` and identify the first incomplete task.
2. Check the latest commit only for directly relevant unfinished context for that task.
3. Inspect the minimal code, fixtures, and documentation needed to understand the task.
4. Implement the task without changing scope or using workarounds.
5. Run the task's required validation plus relevant repository tests.
6. If a blocker prevents correct implementation, add the minimum prerequisite task to `TODO.md`, commit that bookkeeping, and stop.
7. If the task is completed, mark its TODO heading with `[DONE]`, update its completion record, and update this plan file.
8. Commit all changes for this invocation with a task-specific message.
9. Stop without starting the next task.

## Progress

- Initial plan recorded.
- Identified first incomplete task: `HIR-T14` (freeze HIR completeness validation matrix and stage completion record).
- Current focus: document final HIR invariants, verify no reachable refactor HIR Todo path remains, update task completion record, validate, commit, and stop.
- Added `HIR_COMPLETENESS_HANDOFF.md` with final invariants, validation matrix, fixture set, Todo scan classification, and later-stage gap list.
- Linked the handoff document from `PLAN.md` completion criteria.
- Validation passed so far: `cargo test -p scoopc --no-default-features refactor_hir_no_todo`, `cargo test -p scoopc --no-default-features refactor_hir_preflight`, `cargo test -p scoop --no-default-features dump_hir`, final `rg "Todo\\(" crates/scoopc/src/hir crates/scoopc/src/effect_refactor_pipeline` classification, and `cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`.
- Marked `HIR-T14` as `[DONE]` in `TODO.md` and recorded the handoff documentation plus validation results.
