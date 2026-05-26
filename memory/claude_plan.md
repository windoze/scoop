# Execution Plan

I will follow the repository's TODO-driven workflow and complete exactly the first incomplete task.

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Review the selected task's requirements, dependencies, validation commands, and completion record expectations.
3. Inspect only the code, fixtures, docs, and recent commit context needed for that selected task.
4. Implement the task directly unless a concrete prerequisite or blocker makes correct implementation impossible.
5. If a prerequisite blocker is found, update `TODO.md` with the minimum necessary prerequisite task, commit that bookkeeping, and stop.
6. Otherwise, run formatting, linting, tests, and fixtures required by the task and repository instructions, fixing any unscheduled failures encountered.
7. Mark the completed task title in `TODO.md` with `[DONE]`, update its completion record, and update this plan file with key progress.
8. Commit all relevant changes with a clear task-scoped commit message, including the required co-author trailer.
9. Stop without starting the next task.

Current status: initial plan recorded before task selection; next step is reading `TODO.md`.

## Selected Task

First incomplete task: `P0-T01` — inventory the current `EXPECT-*` fixture directive set from `crates/scoopc/src/fixtures/expectations.rs` and document each directive's syntax, parameters, and semantics in a repository documentation page.

## Progress

- Read `TODO.md` and confirmed `P0-T01` is the first incomplete task.
- Checked the latest commit; it only adds the test infrastructure cleanup TODO and does not introduce a directly relevant unfinished prerequisite.
- Inspected `expectations.rs`, `fixtures/mod.rs`, and `fixtures/run_pass.rs` to identify parser rules and directive consumers.
- Created `docs/fixtures.md` with the parser/header rules, shared directives, diagnostic expectations, and parse/build/run-pass phase semantics.
- Validated that `docs/fixtures.md` covers all 22 directive prefixes parsed by `expectations.rs`.
- Ran `cargo fmt --check` successfully.
- Marked `P0-T01` complete in `TODO.md`.
- Next step: review the final diff and commit the task-scoped changes.
