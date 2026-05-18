# Current Invocation Plan

## Scope

- Follow `TODO.md` as the authoritative ordered task list.
- Complete exactly the first task whose title is not prefixed with `[DONE]`, then stop.
- Do not skip review tasks or tasks with partial completion notes.
- Do not perform broad historical issue triage before selecting the current task.
- Treat blockers or spec mismatches as prerequisites only when they directly block the selected task.

## Current Status

- `TODO.md` has been read.
- First incomplete task identified: `C4-T02` (`更新 user-visible failure / frontend reject audit 基线`).
- This plan is being written before running build, test, search, or Git commands for the task.

## Selected Task Notes

- Recompute `STALE_UNSUPPORTED_MAIN_BODY_COUNTS`, especially for `mir_body/aggregates.rs`, `mir_body/terminator.rs`, `mir_body/value_args.rs`, and `effect_lowered/value.rs` after CaptureBox removal.
- Confirm whether C2-T02 left any internal `unreachable!` guard that must be listed in `INTERNAL_BUG_SENTINEL_HITS`; prior completion notes say the guard was deleted, so expectation is no new sentinel.
- Register sealed-interface frontend rejects in `FRONTEND_REJECT_SURFACES` with matching source error-code locations and fixture markers.
- Confirm `STALE_USER_VISIBLE_UNSUPPORTED_MARKERS` remains empty or document the specific reason if not.
- Required validation: `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, `cargo test -p scoopc`, and `cargo run -p scoop -- test`.

## Execution Plan For C4-T02

1. Check the latest commit message for directly relevant unfinished work, then inspect the current worktree state.
2. Inspect `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` to understand the current audit tables and tests.
3. Search current source for `UnsupportedMainBody`, user-visible unsupported markers, and internal sentinel hits needed by the audit policy.
4. Recompute and update stale unsupported counts only where current source counts differ from the recorded baseline.
5. Search sealed-interface diagnostics and fixtures, then add each implemented `scoop::typecheck::sealed_interface_*` reject surface to `FRONTEND_REJECT_SURFACES` with precise source and fixture markers.
6. Run the targeted audit test; fix any policy mismatches without weakening the audit intent.
7. Run the task-required broader validations and fix in-scope failures.
8. Update `TODO.md` by marking `C4-T02` as `[DONE]` and filling in the completion record with scope, decisions, validation results, and plan/design closure.
9. Update this file at key milestones or if the plan changes.
10. Commit all task-related changes with a descriptive `[C4-T02]` message.
11. Stop without starting `C5-T01`.

## Progress

- Plan initialized for `C4-T02`.
- Latest commit checked: no directly relevant unfinished blocker for `C4-T02`.
- Initial audit test exposed expected baseline drift: `UnsupportedMainBody` counts and internal sentinel line numbers changed after prior tasks.
- Updated `pipeline_user_visible_failure_policy.rs` with recomputed stale unsupported counts, refreshed sentinel line locations, and sealed-interface frontend reject surfaces.
- `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` now passes.
- Broader `cargo test -p scoopc` exposed an in-scope validation blocker: runtime itable/RTTI collection still treated `scoop.core.AnyRef` as a runtime interface target.
- Fixed the blocker by filtering sealed interfaces out of runtime interface metadata/targets and adding an RTTI assertion that `AnyRef` / `AnyValue` do not appear in runtime match names.
- `cargo fmt` passed.
- `cargo test -p scoopc dump_rtti_class_itable_entries_preserve_parameterized_runtime_match_metadata -- --nocapture` passed.
- `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` passed after the blocker fix.
- `cargo test -p scoopc` passed with 871 tests.
- `cargo run -p scoop -- test` passed with `fixtures: ok (1405)`.
- `cargo clippy --all-targets -- -D warnings` passed.
- `TODO.md` updated: `C4-T02` is marked `[DONE]` with completion record.
- Next step: commit all task-related changes and stop.
