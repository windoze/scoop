# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose title is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop.
- If the task is blocked by a concrete prerequisite, update `TODO.md`, commit the bookkeeping change, and stop.

## Steps

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent Git history only for directly relevant unfinished work tied to that task.
3. Inspect the smallest relevant part of the codebase needed for the selected task.
4. Implement the task without weakening scope or using fixture-only workarounds.
5. Run the task-specified validation and any directly relevant tests.
6. Update `TODO.md` by prefixing the completed task title with `[DONE]` and adding a completion record.
7. Update this plan file when the concrete task, major progress, blockers, or verification results change.
8. Commit all relevant changes with a descriptive task-tagged message.

## Current Status

- Selected task: `CG-T03R` (`Review CG-T03 call/ctor/intrinsic lowering`).
- Review focus: class constructor lowering contract, top-level function references, runtime reflection/platform intrinsics, interface default dispatch, and removal of backend semantic guessing via string splitting or default-arg fallback.
- Latest commit is `[CG-T03] Lower call contracts in LLVM`; no directly relevant unfinished issue was found in the commit summary.
- Code review checkpoints inspected: MIR call/ctor metadata definitions, typed call lowering, refactor LLVM class ctor lowering, platform/type metadata/sizeOf lowering, plain interface dispatch target resolution, top-level function value lowering, and the `rsplit_once` search surface.
- Initial review result: the main call/ctor/interface/default checks passed, but the new minimal `nameOf<T>()` run-pass fixture failed with a non-zero process exit.
- Validation progress before this failure: `refactor_llvm_call_contract_lowering`, `refactor_mir_call_contract_lowers_typed_call_sites`, the ctor/getPlatform/interface-default/top-level-function-value/sizeOf fixtures, `codegen_gap_inventory`, `refactor_llvm_backend_gate`, and `cargo clippy --all-targets -- -D warnings` passed.
- Current blocker: diagnose and fix the `nameOf<T>()` runtime/codegen failure, because it is directly relevant to `CG-T03`'s reflection intrinsic requirement.
- Fix applied: MIR reflection intrinsic lowering now canonicalizes typed intrinsic FQNs by stripping generic/overload suffixes before matching `sizeOf`/`nameOf`; the generic materialization fallback now lowers `nameOf<T>()` from top-level call binding to `TypeMetadataLiteral`; the LLVM MIR direct-call base helper was corrected to preserve the stripped base.
- Final validation status: new `nameOf` fixture, `refactor_mir_call_contract_lowers_typed_call_sites`, `refactor_llvm_call_contract_lowering`, ctor/getPlatform/interface-default/top-level-function-value/sizeOf fixtures, `codegen_gap_inventory`, `refactor_llvm_backend_gate`, `cargo fmt`, and final `cargo clippy --all-targets -- -D warnings` all passed.
- `TODO.md` updated: `CG-T03R` is now prefixed with `[DONE]` in both the task index and heading, with a completion record covering the review conclusion, `nameOf<T>()` fix, added fixture, and validation commands.
- Next step: inspect the final diff and commit the completed task.
- Note: an exploratory build of `tests/fixtures/mir_refactor/call_contracts.scoop` failed on unrelated closure-call lowering (`refactor plain closure callee type`); this is not part of `CG-T03R`'s validation target and does not invalidate the reviewed call/ctor/intrinsic/interface contracts.
