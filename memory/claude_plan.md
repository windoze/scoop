# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first incomplete task, then stop.
- Keep this file updated when the plan changes or a key step completes.
- Do not expose private chain-of-thought; this file records the actionable plan and progress.

## Initial Plan

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect the task requirements, dependencies, and validation instructions.
4. Examine only the code and fixtures needed for the selected task.
5. Implement the required changes without workarounds or spec deviations.
6. Run targeted tests first, then any required broader validation from the task.
7. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling its completion record.
8. Update this file with completed key steps and any plan changes.
9. Commit all relevant changes with a descriptive task-tagged message.
10. Stop without starting the next task.

## Progress

- Created initial execution plan before reading project files or running commands.
- Read `TODO.md`; first incomplete task is `P12-T02` (`sysroot` physical directory reorganization by cone FQN).
- Next step: read `TODO-5.md` for the full task body and check the latest commit only for directly relevant unfinished work.
- Read `TODO-5.md` task body for `P12-T02`; scope is physical sysroot file relocation by `package` FQN, plus hardcoded path updates and validation.
- Checked latest commit summary (`[P12-T01] Audit sysroot function declarations`); it is the direct prerequisite and does not indicate an unfinished blocker for this task.
- Moved the 10 tracked top-level sysroot files into cone-FQN subdirectories with `git mv`.
- Moved the 4 build fixture sysroot overlay `core.scoop` files into `scoop.core/core.scoop` so overlay relative paths continue replacing the base core file.
- Next step: update Rust tests/helpers and remaining hardcoded path references that assume flat sysroot layout.
- Updated Rust tests/helpers and source comments that assumed flat sysroot paths.
- Ran `cargo build` and `cargo test -p scoopc sysroot::tests -- --nocapture`; both passed.
- Full fixture run initially found 5 HIR golden span mismatches caused by the sysroot core comment path update; updated those HIR golden spans and confirmed `cargo run -p scoop -- test --fixtures tests/fixtures/hir` passes.
- Next step: run the remaining required full validation and quality checks.
- Confirmed `find sysroot -maxdepth 1 -name '*.scoop'` returns no files and nested sysroot files are under `sysroot/scoop.*` directories.
- Re-ran `cargo run -p scoop -- test`; full fixture baseline passed (`1339/1339` targets, `1376` checks).
- Ran `cargo test --all --all-targets`; passed (`856` tests in `scoopc` plus associated workspace tests).
- Ran `cargo clippy --all-targets -- -D warnings`; passed.
- Next step: update `TODO.md` / `TODO-5.md` completion records, then commit this task.
- Marked `P12-T02` as `[DONE]` in `TODO.md` and `TODO-5.md`, and recorded change scope, decisions, validation, PLAN closure, and temporary fixture status.
- Updated archived managed-ABI TODO references that pointed at the old flat overlay core paths.
- Next step: inspect final diff/status, stage relevant files, commit, and stop.
