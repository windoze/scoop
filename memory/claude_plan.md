# Claude Plan

## Goal
Complete exactly the first incomplete task from TODO.md, then stop after committing the result.

## Execution Plan
1. Read TODO.md and identify the first task whose heading is not prefixed with [DONE].
2. Review the selected task details, dependencies, validation requirements, and any directly relevant latest-commit context.
3. Inspect only the code, fixtures, docs, and tests needed for that task.
4. Implement the task completely, avoiding workarounds and preserving existing user changes.
5. Run required formatting, linting, tests, and fixtures in the requested order, adding prerequisite TODO entries if an unscheduled blocking failure is found.
6. Update TODO.md by prefixing the completed task heading with [DONE] and filling its completion record; update PLAN.md only if phase-level planning changes.
7. Commit all task-related changes with a clear task-tagged message and the required co-author trailer.

## Progress
- Plan file initialized before repository inspection.
- Selected first incomplete task: P0-T04.
- Removed the four fixture-runner self-test fixtures and their three dedicated golden output files.
- Completed validation: formatting, clippy, full Rust tests, full fixture suite, and targeted marker greps all passed.
- Marked P0-T04 as done in TODO.md with validation details.
