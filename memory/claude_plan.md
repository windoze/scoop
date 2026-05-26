## Execution Plan

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Review only the files and context needed for that task, including the latest commit if it directly mentions an unfinished issue relevant to the selected task.
3. Implement the task as written, without narrowing scope or introducing workarounds for missing features or spec mismatches.
4. Run formatting, linting, and relevant/full validation in the required order, fixing any unscheduled failures or adding the minimum prerequisite task if a concrete blocker prevents completion.
5. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in its completion record. Update `PLAN.md` only if the phase-level plan changes.
6. Commit all changes for this task with a descriptive message and the required co-authored-by trailer, then stop.

## Progress

- Plan updated before executing the current task.
- Selected first incomplete task: P1-T02R, reviewing `tools/spec_fixtures.py` against the old `scoop_tools spec-fixtures` implementation for semantic equivalence.
- Review steps:
  1. Inspect the latest commit for relevant unfinished work, then compare `tools/spec_fixtures.py` with the old Rust implementation and documented contract.
  2. Run targeted parity checks for `check`, `sync`, and `check --fix`, including mismatch and stale-file behavior if needed.
  3. Fix any semantic gaps found during the review, without changing task scope or using fixture-specific workarounds.
  4. Run required formatting, linting, tests, and fixture validation in order.
  5. Mark P1-T02R `[DONE]` in `TODO.md`, update the completion record, commit all task changes, and stop.
- Latest commit only completed P1-T02; it did not mention an unfinished blocker for P1-T02R.
- Targeted parity review found and fixed one mismatch: Python now strips repeated leading `//` pairs before matching `FIXTURE:`, matching the old Rust parser.
- Validation completed successfully: Python compile, old/new spec fixture parity smoke, current old/new spec checks, `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, new fixture suite, and legacy fixture suite.
- P1-T02R has been marked `[DONE]` in `TODO.md`; next step is to commit these changes and stop.
