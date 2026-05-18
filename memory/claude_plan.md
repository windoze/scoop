# Claude Execution Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after committing that single task or, if blocked, after recording the minimum prerequisite task and committing the bookkeeping change.

## Operating Constraints

- Do not skip review tasks or tasks with completion notes but no `[DONE]` prefix.
- Do not perform broad historical triage before selecting the current task.
- Do not use workarounds, fixture-only hacks, weakened fixtures, alternate representations, or spec deviations.
- Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria change.
- Mark the completed task by prefixing its `TODO.md` heading with `[DONE]` and updating its completion record.
- Commit all relevant uncommitted changes for the completed or blocked task before stopping.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check the latest commit only for an unfinished issue directly relevant to that task.
3. Inspect the task’s referenced code, fixtures, and validation requirements.
4. Implement the task as written, fixing any directly blocking spec mismatch rather than working around it.
5. Add or update the narrowest relevant tests and fixtures needed for the task.
6. Run the task-specified validation and any nearby targeted tests; run broader validation if required by the task.
7. If validation exposes a blocking missing feature that cannot be completed in this invocation, update `TODO.md` with the minimum prerequisite task and stop after committing.
8. If validation passes, update `TODO.md` with `[DONE]` and a completion record.
9. Re-check the worktree, review the diff, commit the task changes with a descriptive task-tagged message, and stop.

## Progress Log

- Initial plan created before repository inspection.
- Identified first incomplete task as `P7-A2`: B-08/B-21 member store and struct field FrontendReject retirement.
- Latest commit is `[P7-A1] Retire B-16 control-flow context UMB rows`; no directly relevant unfinished issue was visible from the subject.
- `umb-audit list --bucket B-08` and `--bucket B-21` showed active rows currently classified as `InternalBugSentinel`, not `FrontendReject`; inspecting task references before editing implementation.
- Existing uncommitted changes already retired the P7-A2 frontend rows: B-08 `UMB-1131`/`UMB-1142`, B-21 `UMB-0750`/`UMB-0863`/`UMB-0962`.
- `umb-audit stats` passed with active=1,272, retired=12, initial=1,284; `umb-audit diff` passed with 1,272 entries in sync.
- `cargo test -p scoopc audit:: -- --nocapture` initially failed because B-08 category docs omitted the required `D-pending` row; added `D-pending=0` rows to B-08 and B-21 Expected Post-Fix Class tables.
- Validation then passed: `cargo test -p scoopc audit:: -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, B-08 fixture directory, B-21 fixture directory, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings`.
- Updated `TODO.md` to mark `P7-A2` as `[DONE]`, record retired IDs, inventory/ledger changes, stale count changes, fixture status, and validation results.
- Tightened `neg_struct_unknown_field.scoop` to cover direct struct-literal unknown fields instead of with-update unknown fields; reran B-21 fixtures successfully.
- `cargo fmt` introduced layout line drift after the removed B-21 fallback, so regenerated `audit/UMB_inventory.csv`; `umb-audit diff` and full `cargo test -p scoopc audit:: -- --nocapture` now pass again.
- Synchronized the resulting `layout.rs` line drift into affected bucket docs B-06, B-20, B-22, and B-36 so their active row tables match the regenerated inventory.
