# Execution Plan

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Read only the task-relevant parts of `TODO.md`, `PLAN.md`, and nearby source/tests needed to understand requirements and constraints.
3. Implement the first incomplete task as written, without narrowing scope or using fixture-only workarounds.
4. If a concrete blocker or missing prerequisite prevents correct implementation, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
5. Run targeted validation for the changed area, then broader required checks when practical, fixing any failures introduced by the task.
6. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record.
7. Commit all relevant changes with a task-scoped message, then stop without starting the next task.

Progress log:

- Plan initialized. Next step: inspect `TODO.md` to select the first incomplete task.
- `TODO.md` inspected. First incomplete task is `P9-T02`: batch rewrite / merge / delete fixtures according to the P0-T01 three-way classification list. Next step: inspect `TODO-5.md`, relevant plan context, and latest commit for direct task blockers.
- `TODO-5.md` and `target/reshape-baseline/stdlib-fixtures.txt` inspected. Scope is 21 classified stdlib-dependent fixtures plus the task completion condition that no fixture tree references removed stdlib helper calls (`require/check/let/run/also/apply`).
- Latest commit notes the existing `mutable_array_ops_basic.scoop` failure is assigned to P9-T02, so it is in scope as a DELETE-class fixture.
- Execution approach updated: delete DELETE-class fixtures and sidecar stdout files; preserve KEEP-RENAME fixtures; replace remaining `require(...)` assertions with explicit `panic(...)` guards; rename local test functions named `run`/`apply` where needed to avoid stale stdlib-helper references in fixture source.
- DELETE-class run-pass fixtures and their stdout sidecars have been removed. Next step: rewrite retained fixture assertions and same-name local helper functions.
- Removed-helper references have been eliminated from fixture source: old stdlib imports are gone, `require/check/requireLazy/checkLazy` calls are gone, and local `run/apply` test helpers were renamed to non-stdlib names. Next step: run targeted fixture validation.
- Targeted validation passed for all 8 KEEP-RENAME fixtures, representative rewritten run-pass/typecheck fixtures, updated effect snapshots, refreshed MIR snapshots, and sysroot overlay build fixtures. Next step: run broad validation and then update task records.
- Broad fixture validation passed (`cargo run -p scoop -- test`: 1345/1345 targets, 1382 checks) and `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --all --all-targets` exposed in-scope Rust test coupling to removed/renamed fixtures: hardcoded `run`/`apply` fixture function names and unit tests that directly load DELETE-class stdlib fixtures. Next step: update those Rust tests to use the renamed retained fixtures or retained replacement fixtures, then rerun tests.
- Rust tests were updated to stop loading deleted stdlib fixtures and to expect renamed retained fixture functions. `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and the full fixture suite now pass. `TODO.md` and `TODO-5.md` have been updated to mark P9-T02 done with completion details. Next step: inspect final diff/status and commit the task.
