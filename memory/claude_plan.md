# Claude Execution Plan

## Current Invocation Plan

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for an explicitly unfinished issue directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and completion-record expectations.
4. Inspect only the code and fixtures needed to implement that task correctly.
5. Implement the task without workarounds or scope narrowing.
6. Run the task-specified validation plus any relevant focused tests; fix failures that are in scope.
7. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in its completion record.
8. Update this file when key steps complete or if the plan changes.
9. Commit all changes for this task with a clear task-tagged message.
10. Stop after completing exactly this one task.

## Progress Log

- Initial plan recorded before reading project task details.
- Read `TODO.md`; first incomplete task is `CG-T05R: Review CG-T05 adapter 与 NoOutward ABI`.
- Current execution scope is a review task: rerun `CG-T05` validation, inspect adapter/plain ABI implementation and fixtures, search for complete-only `Step_F` workarounds, then update `TODO.md`, commit, and stop.
- Reviewed the main CG-T05 implementation paths: plain callable ABI publication, effect-typed plain closure adapter, hidden-sret aggregate wrapping, plain `main(args)` argv wrapper, and route verifier guards against Step schemas on plain bodies.
- Review found an uncovered adapter selection risk: layouts were matched by args/return shape only, which can be ambiguous for multiple effect-typed surfaces with identical args/return but different effect rows. Plan change: make adapter layout matching include effect-family identity and add a run-pass regression fixture.
- Implemented effect-family matching for effect-typed plain adapter layout selection and added `effect_typed_plain_adapter_multiple_effect_rows_basic.scoop`.
- Validation passed: `cargo test -p scoopc refactor_llvm_effect_typed_adapter`, `cargo test -p scoopc refactor_llvm_no_outward_plain_abi`, all CG-T05 fixtures, the new multiple-effect-row fixture, `cargo test -p scoopc codegen_gap_inventory`, complete-only Step_F search, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`.
