# Execution Plan

## Objective
Complete exactly the first incomplete task listed in `TODO.md`, using `TODO.md` as the authoritative source for task order, dependencies, validation requirements, and completion records.

## Working Plan
1. Read `TODO.md` to find the first task whose title is not explicitly prefixed with `[DONE]`.
2. Inspect only the files and task context needed for that task, including recent git state if it is directly relevant.
3. Implement the task as written, avoiding shortcuts, fixture-only hacks, or spec deviations.
4. Run formatting, linting, tests, and fixtures required by the task and repository policy, fixing any unscheduled failures encountered.
5. Update `TODO.md` by prefixing the completed task title with `[DONE]` and adding a completion record with validation details.
6. Update this plan file at major milestones or if the plan changes.
7. Commit all task-related changes with a descriptive message and the required co-author trailer.
8. Stop after this one task.

## Current Status
Plan initialized. Next step is to read `TODO.md` and identify the first incomplete task.

## Milestone: First Incomplete Task Identified
The first incomplete task is `P4-T04R`, the review task for override / overload boundary semantics. The latest commit is `[P4-T04] Implement override overload boundaries`, so it is directly relevant and will be reviewed as part of this task.

## Review-Specific Plan
1. Read the P4-T04 and P4-T04R task details and completion record.
2. Inspect the P4-T04 commit diff and relevant implementation/tests.
3. Validate behavior with the required formatting, linting, tests, and fixtures.
4. Fix any review findings that are directly tied to P4-T04; otherwise record review completion.
5. Mark P4-T04R `[DONE]` in both `TODO.md` and `TODO-4.md`, commit, and stop.

## Milestone: Review Finding Fixed
The P4-T04 review found duplicate virtual method generic rejection logic in `inheritance.rs` and `annotations.rs`. The inheritance-side duplicate has been removed so `annotations.rs` remains the single source of truth for `virtual_method_cannot_be_generic` diagnostics.

## Validation Plan
1. Run `cargo fmt`.
2. Run `cargo clippy --all-targets -- -D warnings`.
3. Run targeted override / virtual generic fixtures.
4. Run `cargo test --all --all-targets`.
5. Run `python3 tools/spec_fixtures.py check` and `python3 tools/run_fixtures.py`.
6. Update TODO records and commit if validation passes.

## Milestone: Validation Passed
Formatting, linting, targeted override/virtual generic fixtures, full Rust tests, spec fixture check, and full fixture suite passed after removing the duplicate inheritance-side virtual generic check.

## Documentation Step
Update `TODO-4.md` and the root `TODO.md` index to mark `P4-T04R` as `[DONE]`, record the review fix, validation results, and design-plan closure, then commit the task changes.
