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
New invocation started. Next step is to read `TODO.md` and identify the first incomplete task without performing broad triage first.

## Current Invocation Plan
1. Read `TODO.md` and identify the first task whose heading is not explicitly prefixed with `[DONE]`.
2. Review the selected task body, dependencies, validation requirements, and any directly relevant latest-commit context.
3. Inspect only the code, tests, fixtures, and documentation needed for that task.
4. Implement the task completely, or if a concrete blocker prevents correct implementation, add the minimum prerequisite task to `TODO.md` and stop after committing.
5. Run formatting first, then clippy with warnings denied, then the required Rust and fixture validation for the task.
6. Update `TODO.md` completion state and record validation details; update this file at major milestones.
7. Commit all task-related changes with a descriptive message and required co-author trailer, then stop.

## Milestone: First Incomplete Task Identified (Current Invocation)
The first incomplete task is `P4-T05`, "把 constructor overload 纳入 definition-time 规则与 diagnostics". The latest commit is `P4-T04R`, which is the direct dependency and does not describe an unfinished blocker. The current worktree already has this plan-file update and an unrelated untracked `GC_PACING.md`; the latter will be left untouched unless it becomes directly relevant.

## P4-T05 Execution Plan
1. Inspect existing constructor overload metadata and checks in `overloads.rs`, `resolve/mod.rs`, constructor call selection, and parser/header metadata.
2. Determine which P4-T01/P4-T02/P4-T03 helpers already cover constructors and where constructor handling remains incomplete.
3. Implement uniform definition-time constructor checks for duplicate effective signatures, ctor-level generic shape mismatch, vararg/non-vararg overlap, and diagnostics that distinguish ctor-level from class-level type parameters.
4. Add targeted constructor overload fixtures for duplicate signature, generic shape mismatch, vararg overlap if missing, and legal overload/class-level-generic behavior.
5. Run formatting, clippy, targeted fixtures, full Rust tests, spec fixture check, and full fixture suite.
6. Mark `P4-T05` `[DONE]` in `TODO.md` and `TODO-4.md`, record validation, commit, and stop.

## Milestone: Implementation Draft Complete
Constructor declarations now carry secondary-constructor type parameters and where clauses through AST parsing, resolver metadata, type lowering, expression checking scopes, and overload definition-time collection. Targeted fixtures have been added for duplicate constructor signatures, ctor-level generic shape mismatch, vararg overlap, and legal class-level/ctor-level generic constructor overloads.

## Milestone: Validation Passed
Formatting, clippy with warnings denied, fixture driver build, targeted constructor overload fixtures, full Rust tests, spec fixture check, and the full fixture suite all passed. `P4-T05` has been marked `[DONE]` in `TODO.md` and `TODO-4.md` with completion details.

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
