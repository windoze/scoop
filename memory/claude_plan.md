# Execution Plan

## Scope

- Follow `TODO.md` as the source of truth.
- Identify the first task whose title is not prefixed with `[DONE]`.
- Complete exactly that task, or add the minimum prerequisite task if a concrete blocker prevents correct implementation.
- Do not proceed to the next task in this invocation.

## Steps

1. Read `TODO.md` and recent repository state to identify the first incomplete task and any directly relevant unfinished issue from the latest commit.
2. Inspect only the code, fixtures, docs, and tests needed for that task.
3. Implement the required change without workarounds or spec deviations.
4. Run the task's required validation and any focused regression tests; fix issues caused by the change.
5. Update this file when key milestones complete or if the plan changes.
6. Mark the task title `[DONE]` in `TODO.md` and update its completion record.
7. Commit all relevant changes with a task-specific message.
8. Stop after the commit.

## Progress

- Initial plan recorded.
- Identified first incomplete task: `P0-T01R` in `TODO-1.md` / `TODO.md`.
- Latest commit is `[P0-T01] Remove package-level comptime trimming`, directly relevant to this review task.
- Current task plan: audit the required files, search for removed package-level comptime symbols/paths, run P0-T01 validations plus the extra review search, fix any issues found, then mark `P0-T01R` complete and commit.
- Static review complete: required source locations no longer contain package-level `Item::ComptimeIf`, `ComptimeIfItem`, or `trim_package_level_comptime*`; remaining `ComptimeIf` symbols are statement-level comptime tracked by `P0-T02`.
- Fixture review complete: package-level comptime fixtures are now ordinary parse-fail fixtures, and removed cross-file/cone trimming fixture directories are absent.
- Validation complete: `cargo fmt`, `cargo test -p scoopc --no-default-features parser`, `cargo test -p scoopc --no-default-features session`, `cargo run -p scoop -- test`, and `cargo clippy --all-targets -- -D warnings` all passed.
- Completion bookkeeping complete: `P0-T01R` is marked `[DONE]` in `TODO.md` and `TODO-1.md` with review conclusions and validation notes.
