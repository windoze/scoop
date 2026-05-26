# Execution Plan

- Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
- Check the latest commit only for unfinished work directly relevant to that task.
- Inspect the task requirements, dependencies, validation instructions, and relevant code/tests.
- Implement the task exactly as written, adding prerequisite TODO entries instead of using workarounds if a concrete blocker is found.
- Run formatting, linting, targeted tests, and then required broader validation in the requested order.
- Update `TODO.md` by prefixing the completed task title with `[DONE]` and recording completion details; update `PLAN.md` only if phase-level planning changes.
- Commit all task-related changes with a clear message and the required co-author trailer.
- Stop after completing or scheduling the first incomplete task.

## Current Task: P0-T02

- First incomplete task identified: `P0-T02`.
- Task scope: document the current fixture discovery rules from `crates/scoopc/src/fixtures/mod.rs`, including the phase router, `plan_targets`, `is_run_pass_cone_case_root`, and subdirectory conventions, in a form that can be ported directly to the Python runner.
- Preserve existing task ordering and unrelated uncommitted changes; do not alter `PLAN.md` unless phase-level assumptions change.
- Inspect the Rust discovery implementation and current fixture tree, add the inventory to `docs/fixtures.md`, run required validation, then mark only `P0-T02` as done and commit the task changes.

## Progress: P0-T02

- Identified `P0-T02` as the first incomplete task.
- Confirmed the latest commit only completed `P0-T01R` and does not mention an unfinished issue for `P0-T02`.
- Noted pre-existing uncommitted changes in `PLAN.md`, `TODO.md`, and `run_agent.sh`; they will be preserved while this task is completed.
