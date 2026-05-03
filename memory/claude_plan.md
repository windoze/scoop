# Claude Plan

## Note

I cannot provide private chain-of-thought logs, but I can maintain a concise execution plan and progress record here.

## Initial Plan

1. Read `TODO.md` as the task index.
2. Open the referenced `TODO-Px.md` files in order and identify the first task whose heading is not prefixed with `[DONE]`.
3. Inspect recent git history only as needed to detect whether the latest commit contains an unfinished issue directly relevant to that task.
4. Read the task details carefully and inspect the relevant code, tests, and documentation.
5. Implement the task completely with the smallest correct change set.
6. Run targeted verification first, then broader required checks such as formatting, tests, and linting if they are relevant to the touched area.
7. If a concrete blocker prevents correct completion, record the blocker in the appropriate detailed TODO file, add the minimum prerequisite task, sync `TODO.md`, and stop.
8. If the task is completed, mark it `[DONE]` in the authoritative `TODO-Px.md` file, sync `TODO.md` if needed, and update this plan file with results.
9. Commit all required changes with a task-specific git commit message, then stop after this single task.

## Progress Log

- Plan file created before repository inspection.
- Read `TODO.md` and identified the first incomplete detailed task as `P6-T03` in `TODO-P6-part2.md`.
- Reviewed `TODO-P6-part2.md` requirements and checked the latest commit `[P6-T02n] Demote LLVM resume packings`; no new directly relevant unfinished issue was called out there beyond the already-tracked prerequisites.
- Inspected the current refactor LLVM path. `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` is still a placeholder, while `crates/scoopc/src/llvm/emit.rs` still fail-fast rejects effectful reachable callables before body lowering.
- Reproduced the current failure with `cargo run -p scoop -- --effect-pipeline refactor build tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop -o /tmp/effect_resume_if_else_branch_single_perform.out`; it fails with `refactor LLVM backend 尚未迁移 ... call boundary lowering, resume-state lowering`.
- Reviewed the published P5/P6 contracts and ABI query surface in `effect_lowered/ir.rs` and `llvm/codegen/effect_refactor/{layout,types}.rs`, including state graph, boundary lowering, dynamic invoke, local runtime-error, handle dispatch, and surface-resume dispatch contracts.

## Refined Implementation Plan

1. Wire refactor LLVM module building to retain and consume `RefactorAbiQuery` instead of discarding it after ABI materialization.
2. Implement `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` with a state-graph-driven body emitter that defines refactor callable entry functions from the published late-lowered contract.
3. Reuse existing MIR straight-line statement/rvalue lowering where possible, while mapping persistent values through published frame slots instead of reintroducing legacy effect lowering.
4. Lower boundary terminators using published contracts only:
   - direct/dynamic call dispatch via `call_target_layout(...)`
   - resume via `surface_resume_dispatch_layout(...)` and `surface_resume_method_layout(...)`
   - local runtime error via `call_local_runtime_error_contract(...)`
   - handle dispatch via `handle_dispatch_layout(...)`
   - outward `Step_F` construction via step/case/continuation layouts
5. Define the remaining refactor-owned function bodies that ABI materialization already declares when they are needed by reachable code paths, including callable direct/dynamic entries and surface-resume owner dispatch/internal resume bodies.
6. Update entry `main` lowering so refactor effectful entry callables run through the refactor callable entry path rather than legacy HIR body lowering.
7. Add or update unit tests and fixtures for state-graph/body lowering, then run the required formatting, tests, fixtures, and clippy checks.
8. If implementation reveals a concrete missing prerequisite contract that is not already tracked, record it in `TODO-P6-part2.md`, sync `TODO.md`, document it here, commit, and stop.

## Blocker Update

- I found a new prerequisite gap while trying to start `P6-T03` implementation.
- Current handoff publishes boundary semantics and dispatch plans, but it does not publish the authoritative operand/source contract needed to lower statement/terminator-anchored `Call / Perform / Resume` boundaries.
- Missing published facts include:
  - ordered boundary argument sources
  - dynamic call carrier source
  - resume continuation source
  - perform payload source
  - which statement inside a source slice is consumed by the boundary
- Without that contract, `P6-T03` would have to recover boundary inputs from raw `mir::Body` / `mir::Rvalue::Call` / `mir::TerminatorKind::Perform` / `mir::CallKind::Resume`, which conflicts with the task's contract-first boundary.
- I inserted a new prerequisite task `P6-T02o` into `TODO-P6-part2.md`, synchronized `TODO.md`, and updated `P6-T03` to depend on it.
- This invocation will stop after committing the blocker/task-file updates.

## Current Invocation Plan

1. Re-read `TODO.md` as the index and the referenced `TODO-Px.md` files in order, treating the detailed file headings as the source of truth.
2. Confirm the first incomplete detailed task after the previously inserted prerequisite, and inspect whether there are any uncommitted changes left from the prior invocation that must be included.
3. Read the selected task body carefully, then inspect only the code and tests needed to complete that task.
4. Implement the task end-to-end with the smallest correct change set that matches the published contracts and task requirements.
5. Run focused verification first, then required broader checks such as formatting, tests, and `cargo clippy --all-targets -- -D warnings` if the touched surface requires it.
6. Mark the task `[DONE]` in its authoritative `TODO-Px.md` file, sync `TODO.md` if needed, update this plan log with the result, and commit exactly this task's changes before stopping.

## Current Invocation Progress

- Resuming from the prior blocker/task-file update. Next step is to identify the first incomplete detailed task from the synchronized TODO files and determine whether this invocation should complete that new prerequisite or finish any uncommitted blocker bookkeeping first.
- Confirmed from `TODO.md` + `TODO-P6-part2.md` that the first incomplete detailed task is `P6-T02o`.
- Checked git state: only `memory/claude_plan.md` is currently modified; the prior blocker insertion was already committed as `[P6-T03] Add boundary operand contract prerequisite`.
- Read the current late-lowered / LLVM query surfaces. `LateLoweredCallBoundaryLowering` / `Perform` / `Resume` still publish only semantic dispatch plans, not ordered operand sources or source-slice anchor consumption. LLVM ABI query currently has call target, dynamic invoke, local runtime-error, handle dispatch, and surface-resume contracts, but no boundary operand contract query.
- Implementation plan refined:
  1. Add late-lowered operand-source and source-slice-consumption structs to `effect_lowered/ir.rs`, then attach explicit operand contracts to call/perform/resume boundary lowerings.
  2. Materialize those contracts in `effect_lowered/materialize.rs` by reading canonical MIR only at P5/P6 handoff time, with fail-fast on missing/ambiguous anchor or operand drift.
  3. Preserve and render the new contracts in `effect_lowered/opt.rs` and `effect_lowered/dump.rs`.
  4. Add LLVM-side published query wrappers plus ABI/anchor validation in `llvm/codegen/effect_refactor/{types,layout}.rs`.
  5. Add targeted tests for direct call, dynamic call, perform payload, resume operand publication, and query-layer failure cases.
- Implementation complete for `P6-T02o`:
  - `crates/scoopc/src/effect_lowered/{ir,materialize,opt,dump}.rs` now publish, preserve, and render boundary operand/source contracts plus statement/terminator anchor consumption.
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` now publish query-side boundary operand layouts and validate carrier/source/count/anchor consistency before `P6-T03` consumes them.
  - `crates/scoopc/src/mir/mod.rs` gained `ConstValue: PartialEq + Eq` so the new published contract can be compared in tests/query drift checks.
- Important semantic detail fixed during verification: the published contract keeps source-level argument shape instead of flattening to carrier ABI shape, so both `k.resume()` vs `k.resume(())` and `k.resume((7, "world"))` are represented correctly.
- Verification completed successfully:
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_effect_lowered_boundary_operand_contract`
  - `cargo test -p scoopc refactor_llvm_boundary_operand_contract`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- Remaining invocation work: mark `P6-T02o` done in `TODO-P6-part2.md` / `TODO.md`, commit this task, and stop. After this commit, the next incomplete detailed task will be `P6-T03`.
