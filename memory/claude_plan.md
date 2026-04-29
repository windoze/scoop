# Claude Plan

## Planning Notes

I will keep this file as a concise execution log and plan rather than private hidden reasoning.

1. Check the latest commit message and diff to see whether it mentions any known pre-existing issue that must be fixed first.
2. Read `TODO.md` and identify the first incomplete task.
3. If that task is too large, refine it into smaller subtasks and update `PLAN.md` plus `TODO.md` so the first subtask becomes the active task.
4. Inspect the relevant code and tests for the active task, while also watching for any existing bug, regression, or spec mismatch that must take priority.
5. Implement the required change with the smallest correct edit.
6. Run focused verification first, then broader required checks such as formatting, tests, and linting as appropriate for the touched area.
7. Update this file, `TODO.md`, and `PLAN.md` with the result.
8. Create one git commit for this iteration and stop.

## Progress Log

- Plan file created. Next step: inspect the latest commit for any pre-existing issue note, then read `TODO.md`.
- Latest commit checked: commit message does not declare a separate pre-existing blocker to fix first.
- First incomplete task identified from `TODO.md`: `T5001f4` (cross-function class object graph GC-stress regression).
- Before entering `T5001f4`, I extended `scoop test --fixtures` so it also accepts a single `.scoop` fixture file instead of only a directory. The runner now detects file targets and executes exactly that fixture.
- Focused verification for the new runner behavior passed:
  1. `cargo test -p scoop run_all_accepts_single_fixture_file -- --nocapture`
  2. `cargo test -p scoop test_command_parses_run_pass_gc_flags -- --nocapture`
- `T5001f4` root cause identified: class ctor factory path kept the freshly allocated object only in `%rt_alloc_class` SSA while later ctor-arg evaluation could allocate `String` and trigger moving GC. That stale object pointer then flowed into ctor init / field writes / return.
- Implemented fix: `class_ctor.rs` now spills the freshly allocated class object via `defer_gc_sensitive_cg_value(...)` and reloads it from explicit-frame-backed storage before ctor init and before return.
- Added focused LLVM regression: `class_ctor_factory_keeps_allocated_object_rooted_across_gc_sensitive_arg_eval`.
- Focused verification for `T5001f4` passed:
  1. `cargo test -p scoopc class_ctor_factory_keeps_allocated_object_rooted_across_gc_sensitive_arg_eval -- --nocapture`
  2. `cargo test -p scoopc class_ctor_this_local_reloads_from_explicit_frame_after_safepoint -- --nocapture`
  3. `SCOOP_GC_STRESS=1 cargo run -p scoop -- run tests/fixtures/run-pass/gc_cross_function_class_object_graph.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_cross_function_class_object_graph.scoop`
  5. `cargo clippy -p scoop -p scoopc --all-targets -- -D warnings`
- Full fixture suite re-run result: the suite now passes `gc_cross_function_class_object_graph.scoop` and the next blocker is `tests/fixtures/run-pass/higher_order_aggregate_return_struct_mapper.scoop`, which needs to be inserted as the next prerequisite task before the pending reviews.
- Current wrap-up plan:
  1. Update `TODO.md` and `PLAN.md` to mark `T5001f4` done.
  2. Insert the newly exposed blocker as the next prerequisite task before pending review items.
  3. Commit the current changes and stop.
- `T5001f5` reproduction confirmed: normal mode passes, but `SCOOP_GC_STRESS=1` corrupts the first direct `mapper("go")` result into `!!` / `2`.
- `T5001f5` root cause identified from emitted LLVM IR: inside the higher-order closure body, builtin `String.concat` keeps the receiver in stale SSA (`%load_str`) while evaluating the later `"!"` argument, whose string allocation may trigger moving GC. The concat call then consumes the stale receiver pointer, so the returned `Labelled.text` / `Labelled.score` are already wrong before the higher-order aggregate return is read back.
- Planned fix for `T5001f5`: in builtin string-method lowering, spill the receiver through `defer_gc_sensitive_cg_value(...)` and reload it from explicit-frame-backed storage after later argument evaluation, then add an LLVM regression that locks the reload-before-`scoop_string_concat` sequence in the higher-order aggregate-return path.
- Implemented `T5001f5` fix: `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs` now defers the `String` receiver before string-method lowering consumes later arguments, and materializes it from explicit-frame-backed storage right before use. This closes the stale-receiver window for `concat` and the same arg-evaluation pattern in other string methods.
- Added LLVM regression `higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval` to lock the closure-lowered `String.concat` path: after the `"!"` allocation, the receiver must reload from an explicit-frame slot before calling `scoop_string_concat`.
- Verification completed for `T5001f5`:
  1. `cargo test -p scoopc higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval -- --nocapture`
  2. `env SCOOP_GC_STRESS=1 cargo run -p scoop -- run tests/fixtures/run-pass/higher_order_aggregate_return_struct_mapper.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/higher_order_aggregate_return_struct_mapper.scoop`
  4. `cargo clippy -p scoop -p scoopc --all-targets -- -D warnings`
- Full suite re-run result after `T5001f5`: `cargo run -p scoop -- test` now passes the higher-order aggregate fixture and the next blocker is `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`. This needs to be inserted as the next prerequisite task before the pending reviews.

## Current Iteration Plan

- I will keep this file as a concise execution log and plan summary rather than private hidden reasoning.
- Goal for this invocation: complete exactly the first undone item in `TODO.md`, after first checking whether the latest commit indicates an older issue that must be fixed ahead of the planned queue.

### Planned Steps

1. Inspect the latest commit message and, if needed, its referenced issue context.
2. Read `TODO.md` and `PLAN.md` to identify the first incomplete task and whether it already has prerequisite notes.
3. If the task is too large, decompose it into smaller ordered subtasks and update `TODO.md` plus `PLAN.md` before implementation.
4. Inspect only the code and tests needed for the active task, while treating any pre-existing bug or spec mismatch discovered during that work as immediately in scope.
5. Implement the smallest correct fix or feature change.
6. Run focused tests first, then broader required verification for touched components, including `cargo clippy --all-targets -- -D warnings` if feasible for the affected crates.
7. Update `memory/claude_plan.md`, `TODO.md`, and `PLAN.md` to reflect the result.
8. Create one git commit for this iteration and stop.

### Progress Log

- Initial plan for this invocation written before running repository inspection commands.
- Latest commit checked: `08807f5bcd3406736ca8459666b51118a8188cf0` (`[T5001f5] Reload String receivers after arg GC`). It documents the completed `T5001f5` fix and does not introduce a separate older blocker that must preempt the queue.
- First incomplete task identified from `TODO.md`: `T5001f6` (`task_step_cross_thread_sequential_handoff_gc_stress.scoop`). Current plan is to reproduce that runtime GC-stress regression, inspect the task/await handoff implementation, make the smallest correct fix, run focused verification plus required lint/tests for touched crates, then update `TODO.md` / `PLAN.md` and commit.
- Reproduction narrowed the failure to a pre-existing explicit-frame root corruption that is not unique to cross-thread handoff: `task_step_manual_gc_aggregate_transport_basic.scoop` also fails under `SCOOP_GC_VERIFY_ROOTS=1` with invalid explicit-frame roots.
- Root cause identified from fresh LLVM IR: effect state-machine `step_function_return_val` / `dispatch_function_return_val` allocas for `TaskStep<...>` and `__TaskStepResult<...>` were created without deterministic initialization. Some resume/dispatch paths later loaded these return slots before a concrete write on every path, so their GC-pointer fields were `undef` and then got mirrored into explicit-frame root slots.
- Implemented fix: initialize those effect-function return allocas with `default_value(...)` immediately after entry alloca creation, using `store_local_value_exact(...)` so the stack slot and explicit-frame home slots start as `NULL`/zeroed source-of-truth instead of undefined GC refs.
- Added LLVM regression `async_task_effect_return_slots_start_null_before_resume_writes` to lock that async/task state-machine return slots no longer materialize `ptr addrspace(1) undef` for `TaskStep` / `__TaskStepResult` return values.
- Scope widened after validation: besides the return-slot bug, current worktree still exposed a broader class of “fresh GC object / GC receiver crosses safepoint but later consumers keep using stale SSA” paths. I audited and fixed the ones directly reachable from this task and the user-requested follow-up checks: effect frame allocation, effect transport boxed payload, array builder receiver, closure / MIR closure allocation, MIR capture box allocation, function-value call closure receiver, vtable/itable receivers, and `Continuation.resume` boxed payload sequencing. I also removed the duplicated class-ctor-only reload helper by moving the shared helpers into `codegen/mod.rs`.
- Focused compiler/runtime verification completed for the repaired `T5001f6` path:
  1. `cargo test -p scoopc async_task_effect_return_slots_start_null_before_resume_writes -- --nocapture`
  2. `cargo test -p scoopc array_of_string_uses_ref_element_runtime_apis_without_ptr_to_u64 -- --nocapture`
  3. `cargo test -p scoopc closure_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`
  4. `cargo test -p scoopc virtual_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`
  5. `cargo test -p scoopc interface_call_with_real_outward_effect_uses_explicit_outcome_boundary -- --nocapture`
  6. `cargo test -p scoopc continuation_resume_boxed_payload_reloads_box_object_before_runtime_call -- --nocapture`
  7. `cargo test -p scoopc continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization -- --nocapture`
  8. `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/runtime_gc/task_step_manual_gc_aggregate_transport_basic.scoop`
  9. `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`
  10. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/task_step_manual_gc_aggregate_transport_basic.scoop`
  11. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`
- Main task status: `T5001f6` is now fixed. Both the manual task-step transport fixture and the cross-thread sequential handoff fixture pass again, including under `GC_MOVE + GC_STRESS + VERIFY_ROOTS` direct runs.
- New blocker discovered while checking the requested neighboring paths: `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop` still aborts after `after_handle / alice` with two stale explicit-frame roots. I verified that `when_subject` root clearing, `Continuation.resume` receiver reload, and boxed payload object reload are now present in IR, so the remaining bug is a separate consumed-continuation/root-lifetime correctness issue rather than the original `T5001f6` handoff regression.
- Wrap-up plan for this invocation per `PROMPT.md`:
  1. Mark `T5001f6` done in `TODO.md`.
  2. Insert a new prerequisite task before `T5001f6R` for the newly exposed `Continuation.resume` consumed-root verify-roots regression.
  3. Update `PLAN.md` to record both the completed handoff fix and the new blocker.
  4. Run final lint/verification for touched crates, then create one git commit and stop.
