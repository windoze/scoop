# Claude Plan

## Current Invocation
- Goal: complete exactly the first incomplete task listed in `TODO.md`, then stop.
- Source of truth: `TODO.md` for task ordering, requirements, validation, and completion records.
- Constraints: do not skip incomplete tasks, do not use workarounds for spec mismatches, and only update `PLAN.md` if phase-level planning changes.

## Execution Plan
1. Read `TODO.md` to identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit only for an explicitly mentioned unfinished issue directly relevant to that first incomplete task.
3. Inspect the relevant code, fixtures, and docs needed for that task.
4. Implement the smallest correct change that fully satisfies the task, or add the minimum prerequisite task if a concrete blocker makes implementation impossible.
5. Run the task-specified validation and any directly relevant tests; fix failures that are in scope.
6. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling/updating its completion record, or record the blocker/prerequisite if blocked.
7. Update this file after key milestones or plan changes.
8. Commit all relevant uncommitted changes with a clear task-scoped message.
9. Stop after this single task is completed or the blocker/prerequisite commit is made.

## Progress
- Plan initialized before repository inspection.
- Identified first incomplete task: `CG-T04e` (closure env/capture transport lowering).
- Latest commit `82fb727c [CG-T04d] Implement array composite transport lowering` does not explicitly mention an unfinished issue directly relevant to `CG-T04e`.
- Inspected MIR and LLVM codegen: MIR already carries closure env/capture-box transport metadata; LLVM currently gates closure env fields to scalar/String/ref and MakeClosure lowering does not directly validate the env contract.
- Implementation adjustment: extend closure env/capture-box lowering to aggregate/float captures and make MakeClosure lowering consume/validate `ClosureEnvTransportMetadata` rather than relying only on tuple shape.
- First targeted test run exposed an in-scope blocker for array captures: materialized MIR for a closure body using captured `Array<Point>.get` left generic receiver type `T` unresolved. This must be fixed before validating `CG-T04e` instead of weakening the fixture.
- Implemented closure env/capture lowering changes, repaired materialized member receiver metadata for captured array calls, and added `closure_env_composite_capture_basic.scoop` plus an LLVM IR unit test.
- Validation passed so far: `cargo test -p scoopc refactor_llvm_closure_env_transport`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop`, `cargo test -p scoopc codegen_gap_inventory`, `cargo test -p scoopc refactor_llvm_composite_transport_contract`, `cargo test -p scoopc refactor_llvm_array_composite_transport`, `cargo test -p scoopc refactor_mir_composite_transport_metadata_contracts`, and `cargo clippy --all-targets -- -D warnings`.

## Task-Specific Plan: CG-T04e
1. Inspect existing closure lowering, capture metadata, composite layout verifier, runtime descriptor hooks, and current tests/fixtures.
2. Determine whether MIR already publishes the required capture schema and capture-box metadata; if a concrete upstream contract blocker is missing, add the minimum prerequisite task before `CG-T04e` and stop.
3. If contracts exist, implement closure env layout lowering from MIR capture schema and composite layout descriptors for ref/value/composite captures.
4. Ensure mutable captures use capture boxes with trace/copy/drop/rooting behavior consistent with boxed composite transport.
5. Add or update targeted tests/fixtures for tuple/struct/enum/array captures and mutable capture behavior.
6. Run required validation: `cargo test -p scoopc refactor_llvm_closure_env_transport`, relevant run-pass fixtures, `cargo test -p scoopc codegen_gap_inventory`, and clippy if feasible.
7. Mark `CG-T04e` as `[DONE]` with a completion record if fully implemented, then commit all relevant changes.
