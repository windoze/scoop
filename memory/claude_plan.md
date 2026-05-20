# Current Invocation Plan

I will follow `TODO.md` as the source of truth and complete exactly the first task whose heading is not prefixed with `[DONE]`.

Execution plan:

1. Read `TODO.md` and identify the first incomplete task, treating only headings prefixed with `[DONE]` as complete.
2. Inspect the latest commit message only for directly relevant unfinished work that affects that selected task.
3. Read the selected task details and any relevant source, fixtures, or tests needed to implement it without broad unrelated triage.
4. Implement the task as specified, avoiding workaround behavior or weakened fixture shapes.
5. If a concrete blocker prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
6. Run the task-specified validation and any targeted tests needed for confidence; fix failures that are in scope.
7. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record.
8. Update this progress file after key milestones or plan changes.
9. Inspect git status and diffs, then commit all changes relevant to this invocation with a descriptive task-tagged message.
10. Stop after the single selected task is completed or after committing any required blocker/task-list update.

Progress log:

- Plan initialized before reading task files or running commands.
- Selected first incomplete task: `P0-T01` from `TODO.md` / `TODO-1.md`.
- Latest commit is `2adc7b50 Update plan`; it does not identify a directly relevant unfinished issue for `P0-T01`.
- Current focus: remove package-level `comptime if` AST/parser surface and all package-level trimming paths, then update fixtures/tests and task records.
- Removed the AST/parser package-level `comptime if` surface and deleted the trim implementation/export from `comptime`; remaining work is to remove call sites and stale match branches throughout the pipeline and fixtures.
- Removed all active Rust references to `Item::ComptimeIf`, `ComptimeIfItem`, and `trim_package_level_comptime*`; old package-level fixtures are now either ordinary parse-fail fixtures or deleted when they existed only to verify trimming.
- Validation completed: `cargo fmt`, `cargo test -p scoopc --no-default-features parser`, `cargo test -p scoopc --no-default-features session`, `cargo run -p scoop -- test`, `cargo clippy --all-targets -- -D warnings`, plus active source searches for removed package-level comptime identifiers.
- Marked `P0-T01` done in `TODO.md` and `TODO-1.md` with completion notes. Next step is git diff/status review and commit.
