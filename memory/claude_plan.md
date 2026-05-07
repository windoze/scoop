# Claude Plan

## Execution Plan

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit only for directly relevant unfinished work that affects that task.
3. Read only the files needed to understand and implement the selected task.
4. Implement the task as specified, without narrowing scope or introducing workarounds.
5. Run the task's required validation and any focused tests needed for confidence.
6. Update `TODO.md` by prefixing the completed task title with `[DONE]` and recording validation results.
7. Update `PLAN.md` only if the phase-level plan changes.
8. Commit all changes for this task with a clear task-tagged message.
9. Stop after completing exactly one task.

## Progress Log

- Started invocation and wrote the initial execution plan before running project commands.
- Identified the first incomplete task as `CG-T07: 收口 extern global 与 GC pin/handle runtime surface`.
- Checked latest commit `5c8980a2 [CG-T06R] Review unwind thread boundary lowering`; it does not introduce a directly relevant unfinished CG-T07 prerequisite.

## Current Task Plan: CG-T07

1. Check the latest commit for unfinished work directly relevant to CG-T07.
2. Inspect existing extern global parsing/typecheck/MIR/codegen/runtime handling and GC pin/handle policy.
3. Implement extern global lowering through MIR/codegen using storage metadata, or add the minimum prerequisite if an upstream contract is missing.
4. Confirm GC pin/handle intrinsic policy: implement if already specified and supported, otherwise ensure parser/typecheck rejects before MIR.
5. Add focused tests for `refactor_llvm_extern_global`, extern global fixtures, and GC pin/handle positive or negative policy.
6. Run required validations plus formatting/linting.
7. Mark `CG-T07` as `[DONE]` in `TODO.md` with completion records and commit the task changes.

## Implementation Notes

- Existing HIR/MIR already publishes `ExternGlobalContract` / `ExternGlobalRoot`; the missing boundary is LLVM codegen consumption of materialized MIR extern global roots for top-level load/store.
- GC pin/handle runtime intrinsics already have refactor LLVM lowering and runtime fixtures; this task will keep that positive policy and include it in validation rather than replacing it with frontend rejection.
- Unsafe extern-global access is currently represented in the contract, so the implementation will also add a focused typecheck diagnostic for direct extern-global read/write outside unsafe context.

## Key Progress

- Implemented extern global LLVM load/store support with materialized MIR `ExternGlobalRoot` as the primary contract source.
- Added unsafe-context typecheck rejection for direct extern global access outside `@Unsafe`.
- Completed refactor MIR lowering for `GC.handleGet`, which was required for the existing GC handle runtime fixture to pass under the current codegen path.
- Added extern global run-pass and unsafe negative fixtures, plus a focused `refactor_llvm_extern_global` LLVM IR test.
- Added a runtime test global symbol for the run-pass fixture and updated the runtime ABI allowlist.
- Validation passed: `cargo test -p scoopc refactor_llvm_extern_global`, extern-global run-pass fixture, extern-global unsafe negative fixture, `gc_pin_unpin_basic`, `gc_handle_roundtrip`, `cargo test -p scoopc codegen_gap_inventory`, `cargo test -p scoop_runtime --lib abi_exports_allowlist`, and `cargo clippy --all-targets -- -D warnings`.
- Marked `CG-T07` as `[DONE]` in `TODO.md` and recorded the completion details.
