# Claude Execution Plan

## Scope

- Follow `TODO.md` as the source of truth.
- Identify and complete exactly the first task whose title is not prefixed with `[DONE]`.
- Stop after committing the completed task or any required prerequisite/task-list update.

## Execution Plan

1. Read `TODO.md` and identify the first incomplete task by title prefix.
2. Check the latest commit message only for directly relevant unfinished work tied to that task.
3. Read the task details, dependencies, validation requirements, and nearby context in `TODO.md`; read `PLAN.md` only if phase-level context is needed.
4. Inspect the relevant implementation, fixture, and test files for the selected task.
5. If the task is blocked by a concrete missing feature or spec mismatch, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
6. Otherwise implement the task with targeted patches.
7. Run the task-specific validation first, then broader relevant checks required by the task or repository guidance.
8. Fix issues found by validation without working around spec behavior.
9. Mark the task title `[DONE]` in `TODO.md` and update its completion record with implementation and validation notes.
10. Review `git status`, `git diff`, and recent commits; commit all intended changes with a task-specific message.
11. Stop without starting the next task.

## Progress Log

- 2026-05-19: Initial execution plan recorded before inspecting repository task details.
- 2026-05-19: Selected first incomplete task `P7-B2.1`: retire B-02/B-04 MIR local, param, and return type contract fallbacks.
- 2026-05-19: Locked retired scope to 35 IDs: B-02 (6) and B-04 (29). Implementation will add MIR verifier coverage first, then replace matching LLVM UMB fallbacks with internal invariants.
- 2026-05-19: Implemented MIR production/materialized verifier contracts for local references, parameter locals, return operands, member/store operands, and constructor argument shape.
- 2026-05-19: Retired B-02/B-04 codegen fallbacks, regenerated active inventory to 1,053, updated retired ledger to 231, activated B-02/B-04 fixtures, and marked `P7-B2.1` done in `TODO.md`.
- 2026-05-19: Validation passed: B-02/B-04 fixture directories, `cargo test -p scoopc mir:: -- --nocapture`, `cargo test -p scoopc audit:: -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, `umb-audit stats/diff`, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings`.
