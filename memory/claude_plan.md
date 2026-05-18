# Execution Plan

## Scope
- Work on exactly the first incomplete task in `TODO.md`.
- Treat `TODO.md` as the authoritative task order and completion source.
- Do not proceed to the next task after completing or blocking the current one.

## Planned Steps
1. Read `TODO.md` and identify the first heading that is not prefixed with `[DONE]`.
2. Check recent Git context only as needed for the selected task, especially whether the latest commit mentions unfinished work directly relevant to it.
3. Read the selected task details, dependencies, validation requirements, and relevant code/tests.
4. Implement the task as specified, without narrowing scope or using workaround representations.
5. If a concrete blocker prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
6. Run targeted tests first, then broader required validation for the task.
7. Fix any failures that are in scope for the current task.
8. Mark the task title in `TODO.md` with `[DONE]` and update its completion record.
9. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria changed.
10. Inspect Git status and diffs, then commit all intended changes with a task-specific message.

## Progress Log
- Initialized execution plan before reading task details.
- Read `TODO.md`; first incomplete task is `P7-B2.7：B-33/B-34/B-35 extern、RuntimeError、NoGC/frame boundary contract`.
- Task scope: retire 19 active `InternalBugSentinel` rows across B-33, B-34, and B-35; implement verifier/helper contracts, remove corresponding codegen fallbacks, update audit/fixtures/stale counts, validate, mark complete, and commit.
- Relevant active rows are B-33 `UMB-0908/0909/0937/0938/1134/1135/1136/1137`, B-34 `UMB-0215/0216/0217/0218/0947/1047`, and B-35 `UMB-0827/0828/0881/0885/1209`.
- Implementation decision: express extern initializer and store mutability in materialized MIR validation, then replace the matching LLVM fallbacks with internal invariants. RuntimeError and explicit-frame slot checks already have upstream contracts; replace only the targeted fallback sites with invariant panics and leave other active buckets untouched.
- Implemented initial code changes in MIR validation/tests and LLVM lowering for the targeted B-33/B-34/B-35 fallback sites. Next step is formatting and targeted test runs, then audit/doc/fixture inventory updates.
- `cargo test -p scoopc mir::materialize -- --nocapture` passed with 51 tests, including new extern global validation regressions.
- `umb-audit diff` is in sync with 633 active entries; `umb-audit stats` reports active=633, retired=651, initial=1284; B-33/B-34/B-35 lists all report 0 entries.
- Target fixture directories pass: B-33 (4 ok, one skipped pending calling-convention/FunPtr fixture), B-34 (3 ok), B-35 (10 ok, six skipped cross-bucket/pending low-level fixtures).
- `cargo test -p scoopc audit:: -- --nocapture` and `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture` pass after audit/stale-count updates.
- `cargo clippy --all-targets -- -D warnings` passed.
- Updated `TODO.md`: marked P7-B2.7 `[DONE]`, added completion record, and updated active/retired status to active=633 and retired=651.
- Re-ran `cargo test -p scoopc audit:: -- --nocapture` after doc cleanup; it passed.
