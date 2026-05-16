# Claude Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose title is not prefixed with `[DONE]`, then stop.
- Do not perform broad issue triage before selecting the current task.
- If a concrete blocker prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.

## Execution Plan
1. Read `TODO.md` and identify the first incomplete task by title prefix.
2. Check the latest commit message only for directly relevant unfinished work tied to that task.
3. Inspect the files and tests relevant to the selected task.
4. Implement the task with the smallest spec-correct change set.
5. Add or update focused tests/fixtures required by the task.
6. Run the task-specified validation commands, plus any narrowly relevant tests.
7. Fix failures that are directly in scope for the selected task.
8. Mark the task title `[DONE]` in `TODO.md` and update its completion record.
9. Update this plan file with completed key steps and validation results.
10. Commit all task-related changes with a descriptive message, including any pre-existing uncommitted files if this is a resumed task state.
11. Stop without starting the next task.

## Progress
- Plan initialized before reading `TODO.md`.
- Read `TODO.md`; first incomplete task is `P3-T02`.
- Latest commit is `eab69eff [P3-T01] Add mutable array runtime`; no explicit unfinished issue was found in the subject.
- Read `TODO-1.md` P3-T02 details. Selected implementation direction will be decided after inspecting current intrinsic dispatch; expected preference is split entries for inline/out-of-line lowering if it fits existing code cleanly.
- Inspected current implementation: shared `array_size/get/set/data_ptr` entries still mapped both `Array` and `MutableArray` to inline `ScoopArray` lowering.
- Implemented split entry direction: `Array` uses `*_inline`, `MutableArray` uses `*_outofline`; added LLVM `ScoopMutableArray` type builder and out-of-line data loading.
- Found a direct blocker for baseline compatibility: old mutable array literal builder still returned inline `ScoopArray`. Updated `scoop_array_builder_build_mutable_array*` to return `ScoopMutableArray` so existing mutable literals match the new receiver layout.
- Added compiler owner tests for mutable array size/get/set out-of-line IR and Array inline size stability; added runtime coverage for builder-to-mutable out-of-line transfer.
- `cargo fmt` was run for the workspace and produced formatting-only Rust diffs outside the direct implementation files; these are part of the verified worktree state.
- Updated TODO completion records for `P3-T02`.
- Validation completed: targeted compiler/runtime tests passed; three adjusted build fixtures passed; full fixture suite passed with `fixtures: ok (1369)`; `cargo test --all --all-targets` passed with 840 tests; `cargo clippy --all-targets -- -D warnings` passed.
