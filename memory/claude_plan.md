# Claude Plan

## Note

I will not record private chain-of-thought, but I will keep a concise, actionable execution log here.

## Initial plan

1. Inspect the latest commit message for any noted pre-existing issue and fix that first if needed.
2. Read `TODO.md` to identify the first unfinished task.
3. Read `PLAN.md` and relevant code/tests for that task.
4. If the task is too large, decompose it into smaller tasks, update `TODO.md` and `PLAN.md`, commit that planning change, and stop.
5. Otherwise implement the task with the smallest correct code change.
6. Run relevant tests, then broader required checks if needed.
7. Update `TODO.md`, `PLAN.md`, and this file to reflect completion or any newly discovered prerequisite issue.
8. Create one git commit for this iteration and stop.

## Progress

- Plan file created.
- Latest commit inspected: it explicitly tracks an existing async/task waiting regression, so that issue is in scope first.
- First unfinished task identified: `T5001f1` in `TODO.md`.
- Next step: reproduce the failing `await` / `Task.step()` paths, then inspect the compiler/runtime code that transports waiting-task resume payloads.
- Reproduced the regression and confirmed it is a crash, not a hang: `task_step_manual_basic` segfaults after `inner` prints.
- GDB shows `scoop_continuation_resume_with(...)` is called with a null continuation in the waiting-path resume step.
- IR inspection shows the generated await handler loads the escape continuation from the frame but never stores it into the local/root slot later passed to `__task_step_pending(...)`, so Waiting state records `null` instead of the captured continuation.
- Updated `effect/state_machine_emitter.rs` so the escaped continuation is written back through `store_local_value_exact(...)`, which keeps the ordinary local slot and explicit-frame home slot in sync before `__task_step_pending(...)`.
- Added an LLVM regression that checks the pending path emits `load_continuation -> local store -> explicit-frame store -> __task_step_pending(...)` in that order.
- Verified the fix with direct runs of `task_step_manual_basic.scoop`, `async_await_minimal_int_basic.scoop`, `async_await_string_basic.scoop`, and `async_fun_task_runtime_basic.scoop`; all now pass.
- Verified `cargo test -p scoopc async_task_pending_path_stores_escape_continuation_before_waiting_helper -- --nocapture`, `cargo test -p scoopc --lib`, and `cargo clippy -p scoopc --all-targets -- -D warnings` all pass.
- `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` now progresses past the original async blocker and next fails on an unrelated existing fixture, `class_init_order_primary_secondary_basic.scoop`.
