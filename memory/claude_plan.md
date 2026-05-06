# Current Invocation Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after completing and committing that one task, or after committing a required blocker/prerequisite update.

## Execution Steps
1. Read `TODO.md` to identify the first incomplete task and its requirements.
2. Check the latest commit message only for directly relevant unfinished context for that task.
3. Inspect the minimal code, fixtures, and docs needed to understand the selected task.
4. Implement the task without narrowing scope or introducing workarounds.
5. Add or update focused tests/fixtures required by the task.
6. Run the task's required validation plus relevant targeted checks.
7. If validation exposes a task-blocking implementation gap, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
8. If the task is complete, prefix its `TODO.md` heading with `[DONE]`, update its completion record, and avoid routine `PLAN.md` edits unless phase-level sequencing changed.
9. Commit all relevant uncommitted changes with a descriptive task-tagged message.
10. Stop without starting the next task.

## Progress Log
- Initial plan recorded before running repository commands.
- Read `TODO.md`; first incomplete task is `CG-T03: 收口 call/ctor/function-ref/intrinsic/default/interface lowering`.
- Next step is to inspect the latest commit for directly relevant unfinished context, then examine only the code paths needed for CG-T03.
- Latest commit is `[CG-T02R] Review runtime value primitive lowering`; no directly relevant unfinished CG-T03 note found.
- Initial code exploration found likely CG-T03 gaps: `getPlatform()` lacks refactor runtime lowering, class ctor MIR lacks selected ctor/complete ordered args metadata, interface dispatch metadata lacks selected slot/default target, and tests need a `refactor_llvm_call_contract_lowering` coverage point.
- Implementation plan refined: extend MIR `ClassCtor` with selected ctor span and expected ordered param count, extend `DispatchMetadata` with selected member FQN/span, add interface slot declaration identity, make refactor ctor codegen reject incomplete/named args instead of selecting/defaulting in backend, add `getPlatform()` literal lowering, then add targeted unit/fixture coverage.
- Implemented the main CG-T03 changes and added targeted fixtures. `cargo test -p scoopc refactor_llvm_call_contract_lowering` now passes; next step is to run the MIR contract test and run-pass/build fixtures for ctor defaults, platform intrinsic, interface default dispatch, function references, and reflection/sizeOf coverage.
- Targeted validations passed for `refactor_llvm_call_contract_lowering`, MIR call contracts, class ctor named/default/delegation, `getPlatform()` runtime, interface default dispatch, top-level function value, sizeOf codegen, codegen gap inventory, and backend gate. Next step is `cargo clippy --all-targets -- -D warnings`, then TODO completion record and commit.
- `cargo clippy --all-targets -- -D warnings` passed. `TODO.md` was updated to mark `CG-T03` as `[DONE]` with the completion record and validation list. Next step is to commit all relevant changes for this invocation.
