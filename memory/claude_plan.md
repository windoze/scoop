## Execution Plan

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Review only the files and context needed for that task, including the latest commit if it directly mentions an unfinished issue relevant to the selected task.
3. Implement the task as written, without narrowing scope or introducing workarounds for missing features or spec mismatches.
4. Run formatting, linting, and relevant/full validation in the required order, fixing any unscheduled failures or adding the minimum prerequisite task if a concrete blocker prevents completion.
5. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in its completion record. Update `PLAN.md` only if the phase-level plan changes.
6. Commit all changes for this task with a descriptive message and the required co-authored-by trailer, then stop.

## Progress

- Plan created before task execution.
- Selected first incomplete task: P1-T02, implementing `tools/spec_fixtures.py {sync,check}` as the replacement for `scoop_tools spec-fixtures`.
- Added the initial standalone Python script for spec doctest extraction, validation, sync, check, and check-fix behavior.
- Validation passed for the new script, old-script parity, cargo formatting/linting/tests, and both fixture runners. P1-T02 has been marked `[DONE]` in `TODO.md`; next step is to commit these changes and stop.
