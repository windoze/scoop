# Execution Plan

## Constraints
- `TODO.md` is the source of truth for task order, requirements, dependencies, validation, and completion records.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Do not work around spec or implementation gaps; if a blocker prevents correct execution, add the minimum prerequisite task in `TODO.md`, commit, and stop.
- Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria change.
- Keep this file updated when the plan changes or key steps complete.

## Step-by-Step Plan
1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Inspect only the files and context needed for that task, including the latest commit if it directly mentions unfinished work relevant to the task.
3. Implement the task as written, preserving existing design and avoiding workaround behavior.
4. Add or update focused tests/fixtures required by the task.
5. Run the task-specified validation commands and any directly relevant checks; fix failures in scope.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record, or record a concrete prerequisite/blocker if completion is impossible.
7. Update this plan file with completed key steps and any plan changes.
8. Inspect git status/diff/log, stage intended files, and commit the task result with a clear task-tagged message.
9. Stop without starting the next task.

## Progress
- Initial execution plan written.
- Identified `P1-T01` as the first incomplete task: create base crate shells, wire `scoopc` facade dependencies/re-exports, add a dependency gate, update docs, validate, then mark only this task complete.
- Implemented `P1-T01`: added 5 base crate shells, `scoopc::base` facade anchors, `scoop_tools dependency-gate`, and README crate overview.
- Validation completed successfully: `cargo fmt`, `cargo check --workspace --no-default-features`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, and `cargo test -p scoop_tools dependency_gate`.
- Updated `TODO.md` and `TODO-2.md` to mark only `P1-T01` as `[DONE]` with a completion record.

# Current Invocation Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not proceed to the next task after completion.
- Keep this file updated with the execution plan, major progress, plan changes, blockers, validation, and commit status.

## Execution Plan

1. Read `TODO.md` first and identify the first incomplete task by heading prefix.
2. Inspect the current git state and recent commit only as needed for the selected task and any directly relevant unfinished issue.
3. Read the selected task requirements, dependencies, validation requirements, and completion-record expectations.
4. Inspect only the code, fixtures, docs, and tests needed to implement that task correctly.
5. Implement the required changes with minimal, spec-correct edits; if a blocking prerequisite is discovered, update `TODO.md` instead of using a workaround.
6. Run targeted validation first, then broader required validation from the task; fix any regressions introduced by the work.
7. Mark the task heading `[DONE]` in `TODO.md` and update its completion record.
8. Update this plan file with completed steps, validation results, and any deviations.
9. Review `git status`, `git diff`, and recent commits, then commit all intended changes with a task-specific message.
10. Stop after the commit.

## Progress Log

- Initial plan written before task execution.
- Read `TODO.md`; first incomplete task is `P1-T01R` in `TODO-2.md`.
- Reviewed the `P1-T01R` task body and relevant P1 design constraints. This invocation is a review task for the previous base-crate shell work, not an implementation task for `P1-T02`.
- Latest commit is `[P1-T01] Establish base crate shells`; no unfinished issue is mentioned in the commit subject.
- Reviewed workspace membership, base crate manifests/lib shells, `scoopc::base` facade anchors, `README.md`, and the `dependency-gate` implementation.
- Validation passed: `cargo fmt`, `cargo check --workspace --no-default-features`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p scoop_tools dependency_gate`, and all five required `cargo tree -p ...` checks.
- Marked `P1-T01R` as `[DONE]` in `TODO.md` and `TODO-2.md` with the review conclusion and validation record.
