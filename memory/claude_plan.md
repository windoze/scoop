# Execution Plan

## Scope

- Source of truth: `TODO.md`.
- Goal for this invocation: identify and complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop after committing.
- `PLAN.md` will only be changed if the phase-level plan or dependency structure actually changes.

## Step-By-Step Plan

1. Read `TODO.md` first and identify the first incomplete task by heading prefix.
2. Check the latest commit message only for an unfinished issue directly relevant to that selected task.
3. Inspect the task body, dependencies, validation requirements, and nearby completion records.
4. Examine the relevant code and fixtures for the selected task.
5. Implement the smallest spec-correct change that fully completes the selected task.
6. Add or update tests and fixtures required by the task.
7. Run `cargo fmt`.
8. Run `cargo clippy --all-targets -- -D warnings`.
9. Run the relevant targeted tests or fixtures, then the full required validation if appropriate.
10. If any unscheduled failing test or fixture is observed, fix it or add the minimum prerequisite/follow-up task in `TODO.md` before marking the current task complete.
11. Mark the selected task `[DONE]` in `TODO.md` and update its completion record.
12. Review the final diff, then commit all relevant changes with a task-specific message.
13. Stop without starting the next task.

## Progress Log

- Initialized execution plan before reading project files or running commands.
- Read `TODO.md` and identified `P10-T07R` as the first incomplete task. The invocation scope is now limited to reviewing P10 completion and stopping after that task is resolved and committed.
- Read `TODO-7.md` P10-T07/P10-T07R details. Latest commit is `[P10-T07] Finalize per-cone cleanup` and does not explicitly mention an unfinished blocker, so the review proceeds against P10-T07R's stated checkpoints.
- Targeted review found the scheduler tests no longer cover several P10-T07R review checkpoints directly: mixed dependency cache-hit dispatch, failure propagation, concurrency cap, and jobs=1 serial fallback. I will add focused scheduler tests for those cases without changing production behavior.
- Added scheduler regression coverage and ran `cargo test -p scoop --bin scoop scheduler::tests`: 6/6 passed. Targeted residual searches found no forbidden compiler/linker/stage crate residuals in `crates/scoop/src`; `inject_cone_dependency_public_api` remains confined to archive fixture/compat exports.
- Full validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo run -p scoop_tools -- dependency-gate`, `cargo test --all --all-targets`, `cargo run -p scoop -- test`, and `cargo metadata` dependency check. Marked `P10-T07R` complete in `TODO-7.md` and `TODO.md`; updated status text in `PLAN.md` and `PIPELINE_REFACTOR.md` to reflect P10 completion.
- After full fixtures, an untracked `tests/fixtures/run-pass/build/debug/virtual/timeout_should_fail@0.0.0` directory appeared. This showed the timeout path can kill `scoop run` before its virtual cone cleanup guard runs. I am fixing the fixture runner parent process to remove the expected single-file virtual cone after command collection, including timeout errors.
- Added parent-side virtual cone cleanup in `run_fixture_command` plus a timeout cleanup unit test. Built the updated `scoopc` binary and reran `timeout_should_fail.scoop`; the generated virtual cone directory is now removed.
