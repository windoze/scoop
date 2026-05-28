# Execution Plan

I will follow the task order in `TODO.md` and complete exactly the first task whose heading is not prefixed with `[DONE]`. I will not perform broad historical triage before selecting that task.

## Steps

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for unfinished work that is directly relevant to that selected task.
3. Inspect the task-related code and tests.
4. Implement the task completely, or add the minimum prerequisite task to `TODO.md` if a concrete blocker prevents correct implementation.
5. Run formatting and validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then the relevant/full test and fixture suites as required by the task.
6. Update `TODO.md` with a `[DONE]` prefix and completion record if the task is completed. Update `PLAN.md` only if phase-level sequencing changes.
7. Commit all task-related changes with a descriptive message and stop.

## Progress

- Created this plan file before running project commands.
- Identified the first incomplete task from `TODO.md`: `P4-T02` in `TODO-4.md`, implementing generic overload shape rules.
- Implemented a generic overload shape model and added targeted fixtures for bound-only legal overloads, concrete+generic overloads, incomparable bounds, and consistency-shape rejection.
- Completed validation (`cargo fmt`, `cargo build -p scoop -p scoopc`, targeted fixtures, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `python3 tools/run_fixtures.py`) and marked `P4-T02` as `[DONE]` in `TODO.md` and `TODO-4.md`.
