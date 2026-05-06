# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after implementing, validating, documenting, and committing that one task.

## Execution Plan

1. Read `TODO.md` first and identify the first incomplete task by heading state.
2. Check the latest commit only for an explicitly unfinished issue directly relevant to that task.
3. Inspect only the code, tests, and docs needed for the selected task.
4. Implement the smallest spec-correct change needed to complete the task.
5. Add or update relevant tests/fixtures for the task requirements.
6. Run targeted validation first, then broader required validation if feasible.
7. If a concrete blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
8. If the task is completed, prefix its `TODO.md` heading with `[DONE]` and update its completion record.
9. Update this plan file when key steps complete or if the plan changes.
10. Commit all relevant changes with a task-scoped message and stop.

## Progress

- Initial execution plan recorded.
- Read `TODO.md`; first incomplete task is `MIR-T13R: Review MIR-T13 policy gates`.
- Current plan is to review `MIR-T13` implementation, rerun its required validations, inspect the relevant diagnostics/smoke/preflight policy gates, then either record a blocking gap or mark `MIR-T13R` done and commit.
- Latest commit is `[MIR-T13] Add MIR policy gates`; it does not mention an unfinished issue.
- Review pass found the expected policy gates in code: GC intrinsic transport metadata/verifiers, cross-thread non-`Pure` continuation diagnostic, or-pattern binder diagnostic, preflight policy denylist entries, and ResumeUnwind cleanup/pending-completion smoke tests.
- Validation completed successfully: `refactor_mir_policy_gates`, `refactor_hir_preflight`, `refactor_materialized_mir`, `refactor_mir_no_todo`, the three MIR-T13 diagnostics fixtures, `dump-mir` and `dump-effect-lowered` for `handle_finally_boundary.scoop`, plus `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`.
- Updated `TODO.md`: marked `MIR-T13R` as `[DONE]` and added the review completion record.
- Git diff reviewed; only `TODO.md` and this progress plan are pending for the `MIR-T13R` commit.
