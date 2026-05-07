# Claude Execution Plan

## Scope
- Follow `TODO.md` as the source of truth.
- Identify and complete exactly the first task whose title is not prefixed with `[DONE]`.
- Stop after committing that task or, if blocked, after recording the minimum required prerequisite task and committing that bookkeeping.

## Execution Steps
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for an unfinished issue directly relevant to that task.
3. Inspect the relevant implementation and tests for the selected task.
4. Implement the smallest spec-correct change needed for the task.
5. Run the task's required validation plus any targeted tests needed for confidence.
6. Fix any regressions or blockers that are directly in scope for the selected task.
7. Update `TODO.md` by prefixing the task title with `[DONE]` and filling its completion record.
8. Update this plan file when key steps complete or if the plan changes.
9. Commit all relevant uncommitted files with a descriptive task-tagged message.
10. Stop without starting the next task.

## Current Progress
- Initial execution plan recorded.
- `TODO.md` read; first incomplete task is `CG-T04f: 收口 cross-thread resume payload transport lowering`.
- Latest commit is `[CG-T04e] Implement closure env transport lowering`; no explicit unfinished issue was identified from the subject.
- Implementation direction selected: add a generic refactor cross-thread resume transport helper that carries word, GC ref, optional composite descriptor, and optional copied payload storage. MIR call metadata will publish an `EffectPayload` transport for the resume value, LLVM will consume that metadata, and runtime will expose descriptor-derived native root slots while the parent thread is blocked in `join`.
- First targeted LLVM test failed at the composite descriptor verifier because the generic helper's payload transport reached codegen without a concrete materialized layout type. This is in scope for `CG-T04f`; fix the MIR/materialization metadata so LLVM consumes a concrete payload contract instead of inferring it locally.
- Implemented generic cross-thread resume transport metadata/lowering/runtime helper and added composite/ref runtime_gc coverage. Targeted LLVM test, existing ref payload runtime_gc fixture, new composite runtime_gc fixture, codegen inventory, composite contract, backend gate, and runtime ABI lib allowlist have passed; clippy remains to run before marking the task done.
- `cargo clippy --all-targets -- -D warnings` passed. `TODO.md` has been updated to mark `CG-T04f` done with the validation record; next step is final git review and commit.
