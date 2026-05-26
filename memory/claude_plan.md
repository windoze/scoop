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
- Selected first incomplete task: P1-T05, implementing `tools/dependency_gate.py` as the Python replacement for `scoop_tools dependency-gate`.
- Latest commit is `[P1-T04R] Review safepoint baseline parity`; it is not an unfinished blocker for P1-T05.
- Current worktree includes this plan update plus unrelated untracked `RTTI_REFINE.md` and `CALLER_LOCATION.md`, which will be left untouched.
- Next steps: port the Rust dependency gate rules to a standard-library Python script, validate it against the old Rust command on the current repository, run required formatting/lint/tests, update `TODO.md`, and commit the task changes.
- Implemented `tools/dependency_gate.py` using `cargo metadata --format-version 1` plus the source-boundary checks from the old Rust tool.
- Initial parity validation passed: `python3 -m py_compile tools/dependency_gate.py` and a full diff between `python3 tools/dependency_gate.py` and `cargo run -q -p scoop_tools -- dependency-gate` produced no differences.
- Repository validation completed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/run_fixtures.py tests/fixtures`, and `cargo run -p scoop -- test` all passed.
- `TODO.md` now marks P1-T05 done with a completion record. Next step: review the final diff and commit the task changes.
