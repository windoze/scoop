# Current Invocation Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, or add the minimum prerequisite task if a concrete blocker prevents correct completion.
- Stop after committing the completed task or committed task-list/blocker update.

## Execution Plan

1. Read `TODO.md` first and identify the first incomplete task.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect the files and tests needed for the selected task, avoiding unrelated historical triage.
4. Implement the selected task with the smallest correct code changes.
5. Add or update focused tests/fixtures required by the task.
6. Run the task-specific validation from `TODO.md`, then broader relevant validation as needed.
7. If a failing test/fixture is observed and is not already explicitly scheduled, fix it or add the minimum prerequisite task in `TODO.md` before marking completion.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record after validation succeeds.
9. Update `PLAN.md` only if the phase-level plan changes.
10. Inspect git status/diff/log, then commit all intended changes with a clear task-tagged message.
11. Stop without starting the next task.

## Progress Log

- Initialized plan before reading or running project commands.
- Identified first incomplete task: `P9-T05R` in `TODO-7.md`, reviewing the completed `scoopc_mir` extraction.
- Latest commit is `[P9-T05] Extract scoopc_mir crate`; no separate unfinished issue is visible in the commit subject, so the review scope remains `P9-T05R`.
- Review focus: confirm RTTI ownership, opt/pass pipeline migration, and stable-id/mangler cleanup; fix any concrete issue found before marking the review done.
- Audit found `scoopc_mir` has no actual dependency on effect/LIR/codegen crates and dependency-gate passes; updated the MIR module boundary comment to clarify stable exported symbols vs backend-private symbols.
- Validation passed so far: `cargo fmt`, `cargo build --workspace`, `cargo test --all --all-targets`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`, `cargo run -p scoop_tools -- dependency-gate`, and `cargo clippy --all-targets -- -D warnings`.
- Marked `P9-T05R` as `[DONE]` in `TODO.md` and `TODO-7.md` with a completion record.
- `git diff --check` passed; final status/diff/log review found intended changes plus unrelated untracked `PLUGIN_ABI.md`, which will be left unmodified and uncommitted.
