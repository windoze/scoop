# Current Invocation Plan

## Reasoning Summary

The task source of truth is `TODO.md`. I will identify the first task whose heading is not prefixed with `[DONE]`, complete exactly that task if possible, update its completion record, commit all relevant changes, and stop. If the task is blocked by a concrete prerequisite or spec mismatch, I will update `TODO.md` with the minimum necessary prerequisite task, commit that bookkeeping change, and stop instead of using a workaround.

## Step-by-Step Plan

1. Read `TODO.md` first and identify the first incomplete task by heading prefix.
2. Check the latest commit message only for directly relevant unfinished work tied to the selected task.
3. Inspect the selected task details, dependencies, required validation, and any related code or fixtures.
4. Implement the smallest spec-correct change needed for the selected task, avoiding workarounds and preserving unrelated worktree changes.
5. Add or update focused tests and fixtures required by the task.
6. Run the task-specified validation and any relevant targeted checks; fix failures that are in scope.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in the completion record. Update `PLAN.md` only if phase-level sequencing or criteria actually change.
8. Update this file as key steps complete or if the plan changes.
9. Review git status and diffs, include all relevant uncommitted files for this invocation, commit with a task-specific message, and stop.

## Progress

- Invocation started.
- Initial execution plan recorded before reading project task files or running commands.
- Read `TODO.md`; first incomplete task is `P3-T01` (`runtime 端：ScoopMutableArray out-of-line layout + 单态 new/push/freeze 入口`).
- Read `TODO-1.md` P3-T01 details and latest commit. Latest commit is completed `P2-T01`, with no directly relevant unfinished prerequisite.
- Implementation direction: add `ScoopMutableArray` beside existing `ScoopArrayBuilder`, reuse the existing out-of-line trace/drop/copy patterns, export the six `scoop_mutable_array_*` runtime symbols, and add Rust integration tests that exercise the C runtime ABI.
- GC note: because mutable array elements live in malloc-backed out-of-line storage, ref/composite pushes need a promotion-only write-barrier path before safepoint polling so old mutable arrays cannot silently retain nursery refs outside inline object storage.
- Implemented runtime changes and tests. Targeted mutable-array integration tests, `cargo build`, full fixture suite, `cargo clippy --all-targets -- -D warnings`, and `cargo test -p scoop_runtime --all-targets` all pass.
- Updated `TODO.md` and `TODO-1.md` to mark `P3-T01` as `[DONE]` with completion record. Next step is git review and commit.
