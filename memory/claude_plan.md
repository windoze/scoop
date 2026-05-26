# Execution Plan

I will follow the task order in `TODO.md` and complete exactly the first task whose heading is not prefixed with `[DONE]`.

Steps:
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for an explicitly unfinished issue directly relevant to that selected task.
3. Inspect the relevant source, tests, fixtures, and documentation for that task.
4. Implement the task without weakening the intended behavior or using workarounds.
5. Run formatting, linting, and the required tests/fixtures in the prescribed order.
6. If failures reveal unscheduled issues, fix them when in scope or add the minimum prerequisite/follow-up task in `TODO.md` before marking the current task done.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
8. Update this file at key milestones and update `PLAN.md` only if phase-level sequencing changes.
9. Commit all task-related changes with a descriptive message and stop.

## Current Task

Selected task: `P0-T03` — freeze the stdout/stderr/exit-code contracts for
the compiler and driver commands that the future external fixture runner is
allowed to call.

Execution checkpoints:
1. Check the latest commit for unfinished work directly related to `P0-T03`.
2. Inspect existing documentation and CLI implementations for `scoopc dump-*`,
   `scoopc emit-artifact`, `scoopc build-single-cone`, `scoopc link-cone`,
   `scoop build`, and `scoop run`.
3. Document the stable contracts in the fixture infrastructure documentation,
   including success/failure exit-code behavior and the fields future external
   tooling may consume.
4. Add or adjust focused tests if the command contracts are not already covered
   well enough to keep them stable.
5. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, the relevant
   tests, and the fixture suite required for this task.
6. Mark `P0-T03` `[DONE]` in `TODO.md`, update its completion record, commit the
   task changes, and stop.

Progress:
- Identified `P0-T03` as the first incomplete task.
- Latest commit completed `P0-T02R` and did not mention unfinished `P0-T03`
  work.
- Inspected the `scoopc` command parser/binary entry points, `scoop` facade
  dispatch, build/run subprocess handling, and existing fixture documentation.
- Added a `docs/fixtures.md` section that freezes stdout, stderr, exit-code,
  and data-product expectations for the allowed `scoopc` and `scoop` command
  surfaces.
- Added a lightweight documentation test so the external command surface and
  contract columns stay present in live docs.
- Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all --all-targets`, and `cargo run -p scoop -- test`
  (`fixtures: ok (1536)`).
- Marked `P0-T03` `[DONE]` in `TODO.md` and added the completion record.
