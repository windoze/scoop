# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first incomplete task whose heading is not prefixed with `[DONE]`.
- Stop after implementing, validating, documenting, and committing that one task.

## Execution Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for directly relevant unfinished notes, without broad historical triage.
3. Inspect the code and fixtures needed for the selected task.
4. Implement the smallest spec-correct change required by that task.
5. Run the task-specific validation and broader relevant checks.
6. If a concrete blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
7. If validation passes, mark the task title `[DONE]` in `TODO.md` and update its completion record.
8. Commit all task-related changes with a clear task-tagged message.

## Current Status

- Plan file initialized before repository inspection.
- First incomplete task identified: `P4-T02` in `TODO-2.md`.
- Latest commit is `[P4-T01] Switch array literals to MutableArray wrappers`; it is directly relevant as the dependency baseline and does not add a separate blocker.
- Reference scan found additional builder users in `stdlib/collections_map.scoop` and `stdlib/collections_set.scoop`; these must be rewritten because leaving them would violate P4-T02.
- Implementation edits completed for sysroot, stdlib, compiler lowering/runtime declarations, runtime C, and runtime tests.
- Follow-up scan of `crates/`, `runtime/`, `sysroot/`, and `stdlib/` now has no `array_builder` / `__scoop_array_builder` / `ARRAY_BUILDER_` source hits.
- Validation completed: `cargo build`, targeted string fixtures, full fixtures, Rust tests, clippy, and no-builder-reference scans.
- Full fixture suite has one expected failing fixture: `tests/fixtures/run-pass/mutable_array_ops_basic.scoop`, which exercises deleted old `MutableArray<Int>.pop/insert/removeAt/splice` APIs and must be handled by P9 fixture triage.
- `TODO.md` and `TODO-2.md` updated with `[DONE] P4-T02` and completion record.
- Next step: inspect git diff/status, stage all task changes, and commit.

## P4-T02 Checklist

1. Rewrite `String.split` to use `mutableArrayNew<String>`, `MutableArray<String>.push`, and `freeze`.
2. Remove string-specialized builder declarations from `sysroot/string.scoop`.
3. Remove `stdlib/mutable_array.scoop`, because it is the builder-based mutable-array helper file slated for this task.
4. Rewrite remaining stdlib builder users in collections map/set to the MutableArray path.
5. Delete compiler constants/declarations/dispatch for `__scoop_array_builder_*` and `scoop_array_builder_*`.
6. Delete runtime `ScoopArrayBuilder` implementation and API exports; remove `scoop_array_alloc` if it has no remaining non-audit caller.
7. Update affected tests/fixtures/goldens and document any stdlib-dependent fixture fallout in `TODO-2.md`.
8. Run task validation, then mark `P4-T02` `[DONE]` and commit.
