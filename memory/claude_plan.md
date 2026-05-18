# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, or add the minimum prerequisite task if a concrete blocker prevents correct implementation.
- Do not proceed to the next task after completion.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect the files and tests relevant to the selected task.
4. Implement the required changes without workarounds or spec deviations.
5. Add or update targeted tests and fixtures needed to validate the task.
6. Run the task-specific validation commands from `TODO.md`, plus broader checks if required.
7. Fix any regressions or blockers that directly affect the selected task.
8. Update this file after key milestones or if the plan changes.
9. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record.
10. Update `PLAN.md` only if phase-level sequencing or completion criteria change.
11. Review `git status`, `git diff`, and recent commits before committing.
12. Commit all changes relevant to this invocation with a descriptive task-tagged message.
13. Stop after the commit.

## Current Status

- Selected first incomplete task: `P7-B2.2：B-05 MIR CFG contract`.
- Task scope: retire B-05 `InternalBugSentinel` rows by enforcing CFG start block, target, arity, and terminator shape contracts before LLVM codegen.
- Latest commit `d7dcf15d [P7-B2.1] Retire MIR local signature UMB rows` is directly preceding completed work and does not introduce a B-05 blocker.
- Active B-05 rows: 25 (`control_flow.rs` 5, `mir_body/dispatch.rs` 7, `mir_body/terminator.rs` 13).
- Implementation plan: add production/materialized MIR verifier gates for branch Bool conditions and residual interpolated-string rvalues; keep existing CFG target validation; replace B-05 LLVM `UnsupportedMainBody` constructors with internal invariant panics; then update inventory, retired ledger, B-05 docs, fixtures, stale counts, and TODO completion record.
- Implementation status: B-05 LLVM `UnsupportedMainBody` constructors removed; active inventory is 1,028 and retired ledger is 256; B-05 active count is 0.
- Validation completed: `cargo run -p scoopc --bin umb-audit -- list --bucket B-05`, `cargo run -p scoopc --bin umb-audit -- diff`, `cargo run -p scoopc --bin umb-audit -- stats`, `cargo test -p scoopc mir:: -- --nocapture`, `cargo test -p scoopc audit:: -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, `cargo run -p scoop -- test tests/fixtures/umb_fix/B-05-mir-cfg/`, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` passed.
- `TODO.md` completion record updated and `[DONE]` prefix applied for `P7-B2.2`.
- Final pre-commit checks completed; next action is commit.
