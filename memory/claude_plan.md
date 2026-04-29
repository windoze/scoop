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
