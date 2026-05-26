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

Selected task: `P0-T02R` — review the fixture discovery rules inventory for
Python-portable readability and coverage of the existing fixture tree.

Review checkpoints:
1. Compare `docs/fixtures.md` discovery sections with
   `crates/scoopc/src/fixtures/mod.rs` target planning, phase routing, case-root
   predicates, sysroot overlay skipping, and `umb_fix` sub-routing.
2. Verify the documented discovery rules cover every non-archive fixture
   directory shape currently present under `tests/fixtures/**`.
3. Run formatting, linting, Rust tests, and the legacy fixture suite before
   marking the review task complete.

Progress:
- Source/documentation comparison found that `tests/fixtures/cone/` exists as a
  manifest-only directory with no `.scoop` files. I documented that it is not a
  phase or case collection and contributes no full-tree fixture target.
- Validation completed successfully: custom fixture discovery coverage check,
  `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all --all-targets`, and `cargo run -p scoop -- test`.
- Marked `P0-T02R` `[DONE]` in `TODO.md` and added the completion record.
