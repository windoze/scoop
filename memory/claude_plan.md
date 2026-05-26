# Execution Plan

- Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
- Check the latest commit only for unfinished work directly relevant to that task.
- Inspect the task requirements, dependencies, validation instructions, and relevant code/tests.
- Implement the task exactly as written, adding prerequisite TODO entries instead of using workarounds if a concrete blocker is found.
- Run formatting, linting, targeted tests, and then required broader validation in the requested order.
- Update `TODO.md` by prefixing the completed task title with `[DONE]` and recording completion details; update `PLAN.md` only if phase-level planning changes.
- Commit all task-related changes with a clear message and the required co-author trailer.
- Stop after completing or scheduling the first incomplete task.

## Current Task: P0-T01R

- First incomplete task identified: `P0-T01R`.
- Review scope: verify the documented `EXPECT-*` directive inventory covers every directive supported by `crates/scoopc/src/fixtures/expectations.rs` and that syntax/semantics are accurate enough for later Python migration.
- Compare source parsing logic, existing tests, and `docs/fixtures.md`.
- Fix documentation or task records if the review finds gaps.
- Run focused validation first, then formatting/linting/test commands required by the task policy as feasible.
- Mark only `P0-T01R` done, commit, and stop.

## Progress: P0-T01R

- Compared `expectations.rs` parser prefixes, parser unit tests, and `docs/fixtures.md` directive rows.
- Corrected the focused coverage check to compare parser prefixes, excluding the non-directive ARGS consumer rows in the documentation.
- Confirmed all 22 parser prefixes and all accepted `EXPECT:` values are documented.
- No documentation corrections were needed.
- `cargo fmt --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --all --all-targets` passed.
- `cargo run -p scoop -- test` passed.
- Updated `TODO.md` to mark only `P0-T01R` as completed and record the review validation.
