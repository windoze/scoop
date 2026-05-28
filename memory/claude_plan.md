# Execution Plan

I will follow the task ordering in TODO.md and complete exactly the first task whose heading is not prefixed with `[DONE]`.

1. Read TODO.md to identify the first incomplete task and its validation requirements.
2. Check the latest commit message for any directly relevant unfinished issue.
3. Inspect the files needed for that task, avoiding unrelated historical triage.
4. Implement the task completely, or add the minimum prerequisite task in TODO.md if a concrete blocker prevents correct implementation.
5. Run formatting, linting, tests, and fixtures according to the task requirements and repository policy.
6. Update TODO.md by prefixing the completed task heading with `[DONE]` and adding a completion record, or leave it incomplete and document any prerequisite/blocker.
7. Commit all task-related changes with a descriptive message and the required co-author trailer.
8. Stop after this one task.

I will update this file at key milestones or if the plan changes.

## Current Task

First incomplete task: P3-T06, replacing `AnyRef` / `AnyValue` sealed marker bounds with internal `ref` / `value` generic bound constraints.

Implementation focus:

1. Remove active sysroot uses of `AnyRef` / `AnyValue` and migrate atomics to `T: ref` / `T: value`.
2. Replace marker-bound satisfaction with direct `ref` / `value` bound-kind satisfaction in generic type and function instantiation.
3. Remove the obsolete sealed-marker metadata path from type environment handling.
4. Update fixtures and unit tests so active tests cover `ref` / `value` constraints and old marker names are no longer positive surface.
5. Run formatting, linting, Rust tests, targeted fixtures, and the full fixture suite before marking P3-T06 done.

## Progress

- Migrated sysroot atomics and unsafe atomic primitives to `T: ref` / `T: value`.
- Replaced sealed-marker metadata and marker satisfaction with bound-kind satisfaction checks for type and function instantiation.
- Updated active spec prose, fixtures, and goldens for the removed marker declarations and changed sysroot nominal count.
- Validation completed: formatting, clippy, targeted ref/value fixtures, full Rust test suite, full fixture suite, and spec fixture check.
