# Claude Plan

## Initial execution plan

1. Read TODO.md to identify the first task whose heading is not prefixed with [DONE].
2. Read the selected task details, related PLAN.md context if needed, and the latest commit only if it explicitly mentions unfinished work relevant to that task.
3. Inspect the smallest relevant code and test areas for that task.
4. Implement the task exactly as specified, adding or updating tests/fixtures where required.
5. Run formatting, clippy, tests, and fixtures as required by the task and repository policy.
6. Update TODO.md by prefixing the completed task with [DONE] and filling its completion record; update PLAN.md only if phase-level sequencing changes.
7. Commit all task-related changes with a clear task-tagged message and stop without starting the next task.

## Current invocation plan

1. Read `TODO.md` first and identify the first task heading that is not prefixed with `[DONE]`.
2. Review the selected task details, dependencies, and any directly relevant `PLAN.md` or latest-commit context.
3. Inspect only the code, fixtures, tests, and documentation needed to complete that task.
4. Implement the selected task fully, or add the minimum prerequisite task to `TODO.md` if a concrete blocker prevents correct implementation.
5. Run `cargo fmt`, then `cargo clippy --all-targets -- -D warnings`, then the relevant/full tests and fixture suites required by the task policy.
6. Update `TODO.md` with `[DONE]` and a completion record when the task is complete; update `PLAN.md` only for phase-level changes.
7. Commit all task-related changes with a clear task-tagged message, then stop without starting the next task.

## Progress

- Current invocation plan recorded before task execution.
- Selected first incomplete task: P1-T04, implementing `tools/safepoint_baseline.py` as the python replacement for `scoop_tools safepoint-baseline`.
- Latest commit is `[P1-T03R] Review fixtures matrix parity`; it does not mention unfinished safepoint work. Current worktree already contains this plan update and an unrelated untracked `RTTI_REFINE.md`, which will be left untouched.
- Direct task context reviewed: `TODO.md`, `PLAN.md`, `TEST_INFRA_CLEANUP.md`, old Rust `tools/scoop_tools/src/safepoint_baseline.rs`, old CLI dispatch, and current docs mentioning the safepoint baseline entrypoint.
- Fixed the direct stale-workload blocker by replacing the deleted async/task handoff workload with a current buildable safepoint root-pressure loop fixture, then added the Python safepoint baseline script with the same report shape and IR metrics parser.
- Validation completed: Python compile passed; old Rust `safepoint-baseline` and new `tools/safepoint_baseline.py` reports diff cleanly; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/run_fixtures.py tests/fixtures`, and `cargo run -p scoop -- test` all passed. `TODO.md` now marks P1-T04 done.
