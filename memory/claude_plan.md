# Execution Plan

I will not record private chain-of-thought, but this file will track the actionable plan, progress, and any plan changes.

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Review the selected task details, dependencies, validation requirements, and any relevant recent commit context.
3. Inspect only the code, fixtures, and documentation needed for that task.
4. Implement the task completely, avoiding workarounds or scope narrowing.
5. Run formatting, linting, tests, and fixtures required by the task, addressing any unscheduled failures according to the policy.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and adding a completion record. Update `PLAN.md` only if phase-level planning changes.
7. Commit all task-related changes with the required co-author trailer, then stop.

## Progress

- Created initial execution plan.

## Progress Update

- Identified first incomplete task: `P10-T02R` in `TODO-7.md`.
- Review finding: artifact layout covers the required stage/fact products, but `ConeArtifact::read` does not enforce the documented compiler/schema compatibility policy.
- Plan change: add manifest compatibility validation and tests before marking the review task complete.

## Progress Update

- Implemented manifest compatibility validation for `ConeArtifact::read`.
- Added regression tests for incompatible compiler version and schema versions.
- Starting required validation: formatting, linting, then tests.

## Progress Update

- Validation completed successfully: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p scoopc_cone`, `cargo test --all --all-targets`, and `git diff --check`.
- Updating `TODO.md` and `TODO-7.md` to mark `P10-T02R` complete with the review finding and fix.

## Progress Update

- Marked `P10-T02R` as `[DONE]` in `TODO.md` and `TODO-7.md`.
- Recorded the review result: artifact schema coverage is complete, and the compatibility policy is now enforced during artifact reads.

## Completion

- `P10-T02R` is complete.
- Committing the artifact compatibility fix, task records, and progress file only; unrelated `run_agent.sh` and `PLUGIN_ABI.md` changes are intentionally left uncommitted.
