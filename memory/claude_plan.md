# Claude Plan

## Note

I will keep this file updated with a concise, action-oriented plan, key findings, and progress notes during execution.

## Current Goal

Complete exactly the first unfinished task in `TODO.md`, after first checking whether the latest commit mentions any pre-existing issue that must be fixed first.

## Execution Plan

1. Inspect the latest commit message and diff summary to see whether it mentions any known issue, regression, TODO, workaround, or follow-up that must be fixed first.
2. Read `TODO.md` and identify the first unfinished task.
3. Read `PLAN.md` and any files directly relevant to that task to understand scope and dependencies.
4. If the first unfinished task is too large to complete safely in one pass, decompose it into smaller ordered subtasks, update `PLAN.md`, and rewrite or augment `TODO.md` so the first new subtask is the one executed now.
5. Implement the selected task with the smallest correct code change that matches the spec and existing project structure.
6. Run targeted tests first, then broader validation as needed, including linting or other required checks if the touched area warrants them.
7. If I discover a pre-existing bug, regression, spec mismatch, incomplete implementation boundary, or workaround during probing or testing, treat it as immediately in scope: either fix it before continuing or insert a prerequisite task ahead of the blocked task and stop.
8. Update `TODO.md`, `PLAN.md`, and this file to reflect progress and any task reordering.
9. Commit the resulting changes with a descriptive message, then stop.

## Progress Log

- Plan file created. Next step: inspect the latest commit for any pre-existing issue that must be handled first.
- Latest commit inspected: `8cdff5ef [T5002b2a] Make indirect call token explicit`. The commit message itself does not mention a pre-existing bug or follow-up fix that must be handled before the planned task.
- Read `TODO.md` and `PLAN.md`. The first unfinished item is `T5002b2aR`, a review task for the ordinary indirect-call explicit-token work.
- Relevant implementation areas identified:
  - `crates/scoopc/src/llvm/codegen/call/dispatch.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/llvm/codegen/closure/mod.rs`
  - `crates/scoopc/src/llvm/codegen/call/resume.rs`
  - `crates/scoopc/src/llvm/tests.rs`
- Current verification plan for `T5002b2aR`:
  1. Confirm closure / funptr / vtable / itable production signatures explicitly reserve `incoming_resume_token_ref` in a consistent parameter position.
  2. Confirm each ordinary indirect-call boundary does `publish incoming token -> call -> consume outcome -> clear token`.
  3. Run focused LLVM tests for closure / funptr / vtable / itable explicit-token IR checks.
  4. Run relevant run-pass fixtures/minimal programs so the review checks behavior, not just declaration shape.
  5. If no issue is found, mark `T5002b2aR` done and update `PLAN.md`; otherwise fix the issue immediately or insert a prerequisite task before proceeding.
- Review finding before test execution:
  - `crates/scoopc/src/llvm/codegen/mir_body.rs` still loads `pass_mir` closure `env_ptr` / `fn_ptr` before installing the ordinary effect boundary. This is inconsistent with the ordinary function-value closure path, which defers and reloads the closure object after the boundary to avoid stale SSA / stale moved-address use.
  - This means `T5002b2aR` is not yet actually complete. I will fix the `pass_mir_closure_call` path first, then rerun the focused review checks.
- Follow-up issue found while adding the regression:
  - production-lowered, pass-visible effectful closure calls exposed another pre-existing bug in `crates/scoopc/src/llvm/codegen/mir_body.rs`: top-level pass MIR body parameter binding only skipped hidden sret, but not the newly inserted hidden `incoming_resume_token_ref`.
  - This shifted pass MIR params by one slot on effectful functions and caused the MIR binder to read the token pointer as a user parameter.
  - Planned fix: update pass MIR top-level param binding offset to skip both hidden sret and hidden incoming token, then rerun the review validation.
- Implemented during review:
  - `pass_mir_closure_call` now defers and reloads the closure object after the ordinary effect boundary before reloading `env_ptr` / `fn_ptr`.
  - top-level pass MIR body binding now skips both hidden sret and hidden incoming token.
  - Added LLVM regression `production_pass_mir_closure_call_reloads_closure_after_effect_boundary` to lock the repaired materialized-MIR closure boundary contract on a supported effectful shape.
- Validation completed for the repaired subset:
  - Focused LLVM tests passed: existing indirect-call explicit-outcome tests, the new production pass-MIR closure reload test, and the non-outward-effect smoke tests.
  - Run-pass fixtures passed: `effect_indirect_perform_nonresuming_function_value_local.scoop`, `effect_handle_hidden_suspend_virtual_helper_basic.scoop`, `effect_handle_hidden_suspend_interface_helper_basic.scoop`.
  - `cargo clippy --all-targets -- -D warnings` passed.
- Remaining blocker discovered during review:
  - production MIR bridge still cannot lower materialized effectful closure bodies that directly perform an effect; the path currently fails in `mir_body.rs` with `UnsupportedMainBody { kind: "pass MIR rvalue" }` / `pass MIR terminator`.
  - Because that leaves a real materialized-MIR closure shape unsupported, `T5002b2aR` cannot be marked complete yet.
  - Action taken: inserted a new prerequisite task `T5002b2a1` ahead of `T5002b2aR` in `TODO.md`, updated `PLAN.md`, and will stop after committing this dependency/order update plus the fixes already made.
