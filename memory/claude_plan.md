# Claude Execution Plan

## Scope

- Follow `TODO.md` as the index and the referenced `TODO-Px.md` files as the authoritative task details.
- Complete exactly the first incomplete detailed task, then stop after committing the result.
- Keep task bookkeeping synchronized between the detailed TODO file and `TODO.md`.
- Update this file whenever the plan changes or a key step completes.

## Execution Plan

1. Inspect `TODO.md` to determine the ordered task index and referenced detailed TODO files.
2. Inspect the relevant `TODO-Px.md` files in index order to find the first detailed task whose heading is not prefixed with `[DONE]`.
3. Check the latest commit message for directly relevant unfinished work only after identifying the current detailed task.
4. Read the selected task requirements, dependencies, constraints, validation expectations, and completion-record format.
5. Inspect the minimum relevant code and tests needed to implement that task correctly.
6. Implement the task without workarounds or scope narrowing. If a concrete blocker prevents correct implementation, add the minimum prerequisite task in the appropriate TODO file, sync `TODO.md`, commit, and stop.
7. Add or update focused tests/fixtures required by the task.
8. Run the task-relevant validation commands, then broader validation if appropriate and feasible.
9. Update the detailed TODO file by prefixing the completed task heading with `[DONE]` and filling in its completion record.
10. Sync `TODO.md` so the index uses the same `[DONE]` marker and task metadata.
11. Review the git diff to ensure only intended changes are included and no secrets or unrelated destructive changes are present.
12. Commit all currently uncommitted files relevant to the completed/resumed task with a clear task-tagged message.
13. Stop without starting the next task.

## Progress Log

- Initialized execution plan before reading project task files or running commands.
- Read `TODO.md`, `TODO-P6-part2.md`, and `TODO-P6-part3.md`. The first executable incomplete detailed task is `P6-T03a` in `TODO-P6-part3.md`; `TODO-P6-part2.md` states that the old monolithic `P6-T03` was abandoned/migrated and no longer carries pending work.
- Implemented the first pass of `P6-T03a`: added `effect_refactor/value.rs` as the effect-neutral value primitive facade, routed refactor source-slice lowering through it, removed direct use of the legacy statement fallback from `effect_refactor/body.rs`, and removed the old direct-call helper path from refactor print lowering.
- Ran validation for the task: `cargo test -p scoopc refactor_llvm_clean_backend_boundary`, `cargo test -p scoopc refactor_llvm_value_primitive`, the requested audit `rg` command, and `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`. The audit command now has no statement/function fallback hits under `effect_refactor`; remaining `effect_op_call_sites` hits are in typed HIR/ABI test scaffolding rather than the refactor body implementation.
- Extra non-required `cargo test -p scoopc refactor_llvm_` was run and still has existing layout/stage failures unrelated to the new value primitive boundary; the task-specific tests passed.
- Updated `TODO-P6-part3.md` and `TODO.md` to mark `P6-T03a` as `[DONE]` and recorded implementation details plus validation commands.
