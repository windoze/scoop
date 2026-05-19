# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after completing and committing that one task, or after committing any required prerequisite/blocker update.

## Execution Plan

1. Read `TODO.md` first and identify the first incomplete task by heading prefix.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and completion-record format.
4. Inspect only the code, fixtures, docs, and tests needed to implement that task correctly.
5. Implement the smallest spec-correct change; do not use fixture-only hacks, workaround representations, or narrowed behavior.
6. If a concrete missing feature or blocker prevents correct implementation, update `TODO.md` with the minimum prerequisite task in the correct order, record the blocker here, commit, and stop.
7. Run task-relevant tests first, then broader validation required by the task. Address any failures caused by the task before proceeding.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record with implementation and validation notes.
9. Re-run any validation needed after documentation updates if the task requires it.
10. Inspect `git status`, `git diff`, and recent log before committing.
11. Commit all relevant uncommitted changes for this invocation with a task-tagged message.
12. Stop without starting the next task.

## Progress Log

- Initialized plan before reading project task files.
- Read `TODO.md`; selected first incomplete task `P8-T01`.
- Current target: delete `LlvmEmitError::UnsupportedMainBody`, remove diagnostic/stale policy remnants, verify active inventory remains zero, then mark only `P8-T01` done and commit.
- Checked latest commit `e86c84a4 [P7-C5] Retire effect callable UMB rows`; no unfinished P8-T01 blocker is recorded there.
- Verified P8 trigger with `cargo run -p scoopc --bin umb-audit -- stats`: active=0, retired=1284, initial=1284.
- Removed `LlvmEmitError::UnsupportedMainBody`, the `scoop::llvm::unsupported_main_body` diagnostic mapping, the stale UMB count table/test, and the obsolete UMB trigger text in `codegen_gap_inventory`.
- Validation completed: `rg -n "UnsupportedMainBody" crates/scoopc/src/llvm` has no matches; `umb-audit stats` and `diff` pass; `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, `cargo test --all --all-targets`, and `cargo clippy --all-targets -- -D warnings` pass.
- Marked `P8-T01` as `[DONE]` in `TODO.md` with completion record. Do not start `P8-T02` in this invocation.
