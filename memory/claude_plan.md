# Execution Plan

## Current Invocation

- Record this public plan before running repository commands.
- Read `TODO.md` and identified the first incomplete task: `P7-B3.2` for B-28/B-27 thread/sync intrinsic contracts.
- Check the latest commit only for directly relevant unfinished notes after the current task is known.
- Inspect the files and tests referenced by that task.
- Implement the smallest spec-correct change needed for the task, without workarounds.
- Run focused validation first, then broader required validation for the task.
- Update `TODO.md` by prefixing the completed task heading with `[DONE]` and adding a completion record.
- Update this file when key milestones complete or if the plan changes.
- Commit all intended changes for this task and stop without starting the next task.

## P7-B3.2 Plan

- Use `umb-audit list --bucket B-28` and `--bucket B-27` to lock the active IDs and source locations.
- Review `audit/strategies/B-28.md`, `audit/strategies/B-27.md`, category docs, fixtures, and relevant intrinsic lowering code.
- Determine the existing upstream typecheck/sysroot verifier coverage for thread and sync intrinsics.
- Replace B-28/B-27 `UnsupportedMainBody` fallbacks with internal invariant checks only where the contract is already enforced, or add the missing verifier/typecheck contract if needed.
- Retire the B-28/B-27 IDs in inventory/ledger/docs/fixtures/stale baseline.
- Validate with the required audit tests and targeted fixture suites, plus formatting and clippy.

## Progress

- Locked active scope: B-28 has 20 rows and B-27 has 58 rows.
- Replaced B-27/B-28 direct LLVM `UnsupportedMainBody` sentinels in thread/sync intrinsic lowering with verified contract invariants.
- Kept unrelated active generic/effect rows such as B-10 `sync intrinsic arg/ref/return` and B-26/B-29/B-30 rows in place.
- Synchronized active inventory and retired ledger: active 593 -> 515, retired 691 -> 769, B-27/B-28 active counts are both 0.
- Activated B-27/B-28 fixtures and updated fixture coverage to use retired ledger ownership.
- Focused B-27/B-28 fixture suites and audit tests now pass.
- Updated `TODO.md` with `[DONE] P7-B3.2` and the completion record.
- Final validation completed: audit diff/stats, B-27/B-28 fixtures, `cargo test -p scoopc audit:: -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, `cargo check -p scoopc`, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` all passed.
