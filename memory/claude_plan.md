# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after completing and committing that one task, or after committing a required prerequisite/blocker update if the task cannot be completed as written.

## Steps

1. Read `TODO.md` and identify the first incomplete task according to the `[DONE]` prefix rule.
2. Check the latest commit only for directly relevant unfinished work tied to that task.
3. Inspect the code, fixtures, and docs needed for the selected task.
4. Implement the smallest spec-correct change needed for the task; do not use workaround behavior.
5. Add or update focused tests/fixtures required by the task.
6. Run the task-specified validation and relevant repository tests.
7. If a blocking prerequisite is discovered, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
8. If validation succeeds, mark the task heading in `TODO.md` with `[DONE]` and update its completion record.
9. Update this plan file when key progress occurs or the plan changes.
10. Commit all changes for this invocation with a task-specific message, then stop.

## Current Status

- First incomplete task identified: `CG-T06` (`source classification`, `ResumeUnwind`, continuation storage route, cross-thread non-complete boundary).
- Latest commit checked: `9eab0104 [CG-T05R] Review effect adapter ABI`; no directly relevant unfinished issue was declared in the commit summary.

## CG-T06 Focus

1. Locate current verifier/backend gates for `LateLoweredSourceStatementClassificationKind::Unsupported`, `ResumeUnwind`, continuation `StoreMember`, and cross-thread resume non-complete handling.
2. Determine whether each required behavior already has an upstream MIR contract; if a required contract is missing, add the minimum prerequisite task before `CG-T06`, keep `CG-T06` incomplete, commit, and stop.
3. Otherwise implement the verifier/lowering/runtime boundary changes for `CG-T06` only.
4. Add targeted tests named by the task where missing: source classification verifier, resume unwind lowering, continuation storage route, and cross-thread non-complete policy fixtures.
5. Run the CG-T06 validation commands plus formatting/linting required by the repo policy.
6. Mark `CG-T06` as `[DONE]` with completion notes and commit the full task state.

## Progress Notes

- Located current codegen gaps:
  - `body.rs` verifier accepts `LateLoweredSourceStatementClassificationKind::Unsupported` even though materialization already rejects normal production unsupported classifications.
  - `ResumeUnwind` accepts only an empty cleanup terminal and lowers to `unreachable`; codegen needs to verify that this is tied to the published cleanup/finally pending-completion contract rather than a generic placeholder.
  - raw `StoreMember` ambiguous continuation route has a backend helper gate; CG-T06 should lock this with a continuation-route validation fixture/test.
  - thread resume thunks still call `scoop_refactor_thread_resume_noncomplete_fatal`; CG-T06 should consume the MIR-T13 complete-only diagnostic policy by rejecting non-pure surface layouts before thunk generation and avoiding runtime fatal for legal pure continuations.

## Completion Update

- Implemented `CG-T06`.
- Source classification: backend verifier now rejects `Unsupported` classifications before body emission.
- `ResumeUnwind`: verifier now consumes cleanup source slices, Suspend cleanup route, boundary owner/resume-state provenance, and enclosing `HandleDispatch` pending-completion/origin/payload ABI contracts before lowering the terminal path.
- Continuation storage route: `StoreMember` now validates `Unique` continuation route source local/type, not just ambiguous rejection.
- Thread boundary: removed the dedicated non-complete runtime fatal helper; codegen validates the helper operand is a `Pure` continuation, routes non-complete `RuntimeError` through ordinary runtime-error fatal, and treats other non-complete cases as unreachable under the published Pure dispatch contract.
- Validation passed: `cargo test -p scoopc refactor_llvm_source_classification_verifier`; `cargo test -p scoopc refactor_llvm_resume_unwind_lowering`; `cargo test -p scoopc refactor_mir_store_member_codegen`; `cargo test -p scoopc refactor_llvm_thread_resume_noncomplete_policy`; `cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`; `cargo test -p scoopc codegen_gap_inventory`; `cargo test -p scoop_runtime --lib abi_exports_allowlist`; the listed cross-thread/finally fixtures; `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`.
- `TODO.md` has been updated with `[DONE] CG-T06` and completion notes.
