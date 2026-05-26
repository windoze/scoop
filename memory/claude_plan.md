# Claude execution plan

I will follow TODO.md as the source of truth, complete exactly the first incomplete task, validate it, update the task record, commit the result, and stop.

## Steps
1. Read TODO.md and identify the first task whose heading is not prefixed with [DONE].
2. Inspect only the files and context needed for that task.
3. Implement the task as written, or add the minimum prerequisite task if a concrete blocker prevents correct implementation.
4. Run formatting, linting, tests, and fixtures required by the task and repository instructions.
5. Update TODO.md completion status and this progress file; update PLAN.md only if phase-level planning changes.
6. Commit all task-related changes with the required co-author trailer, then stop.

## Progress
- Identified P0-T03R as the first incomplete task.
- Reviewed `docs/fixtures.md` against the actual `scoopc` tool CLI and `scoop` facade command surfaces.
- Tightened the frozen command contract around `scoop run` program exit/stdin semantics and documented the stable `--entry-package` facade flag.
- Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `cargo run -p scoop -- test` (`fixtures: ok (1536)`).
- Marked P0-T03R `[DONE]` in `TODO.md` and added its completion record.
