# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after implementing, validating, documenting completion, and committing that one task.

## Step-By-Step Plan

1. Inspect `TODO.md` to identify the first incomplete task by heading prefix.
2. Review the selected task details, dependencies, validation requirements, and completion record.
3. Check the latest commit only for unfinished work directly relevant to the selected task.
4. Inspect the minimum relevant source, fixture, and test files needed to implement the task correctly.
5. If a concrete blocker or missing prerequisite prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
6. Otherwise, implement the selected task with focused, incremental edits.
7. Add or update the smallest relevant tests/fixtures required by the task.
8. Run targeted validation first, then broader validation required by the task and repository guidelines.
9. Fix any failures introduced by this work and rerun the relevant validation.
10. Mark the selected task title in `TODO.md` with `[DONE]` and update its completion record.
11. Update this progress file as key steps complete or if the plan changes.
12. Review `git status`, `git diff`, and recent commits before committing.
13. Commit all task-related changes with a clear task-tagged message.
14. Stop without starting the next task.

## Progress

- Initial execution plan written before repository inspection.
- Selected first incomplete task: `P1-T02` from `TODO-2.md`, indexed by `TODO.md`.
- Task goal: move authoritative `Span` into `scoopc_span`, move source identity/map types into `scoopc_source`, and leave `scoopc::{span, source}` as re-export adapters without duplicate definitions.
- Latest commit checked: `796a3f1a [P1-T01R] Review base crate dependency gate`; no directly stated unfinished blocker was found.
- Implementation started: added required base-crate dependencies, moved `Span` and source/source-map definitions into `scoopc_span` and `scoopc_source`, and converted old `scoopc` modules to re-export adapters.
- Validation completed: `cargo fmt`, target base-crate tests, workspace tests without default features, clippy with `-D warnings`, dependency gate, `cargo tree -p scoopc_source`, and authoritative definition grep all passed.
- Bookkeeping completed: `TODO.md` and `TODO-2.md` now mark `P1-T02` as `[DONE]` with completion notes.
