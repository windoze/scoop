# Claude Execution Plan

## Scope

- Follow the task ordering rules from the user prompt.
- Complete exactly the first incomplete detailed task found through `TODO.md` and the referenced `TODO-Px.md` files, then stop.
- Keep `TODO.md` synchronized with the authoritative detailed TODO file.
- Commit all changes for this invocation before stopping.

## Reasoning Summary

- `TODO.md` is only an index; detailed task files are authoritative.
- A task is complete only if its detailed heading is explicitly prefixed with `[DONE]`.
- Existing bugs or gaps are in scope only if they block the selected task or invalidate its specified behavior.
- If the selected task cannot be completed without a concrete missing prerequisite, add the minimum prerequisite task in the correct TODO file, sync the index, commit, and stop.
- No fixture-only shortcuts, weakened tests, or representation workarounds are acceptable.

## Step-By-Step Plan

1. Read `TODO.md` as the global index.
2. Open the referenced `TODO-Px.md` files in index order and identify the first task whose detailed heading lacks `[DONE]`.
3. Check the latest commit message only for an unfinished issue directly relevant to that selected task.
4. Read the selected task details, dependencies, validation requirements, and completion record.
5. Inspect the minimal relevant code, fixtures, and tests needed to implement the selected task.
6. Implement the task as specified, preserving existing architecture and avoiding workarounds.
7. Add or update the smallest relevant tests or fixtures required by the task.
8. Run the required validation commands from the task and any focused checks needed for confidence.
9. If validation reveals a blocking prerequisite, update the detailed TODO file and `TODO.md`, commit that bookkeeping, and stop.
10. If the task is complete, mark the detailed task heading `[DONE]`, update its completion record, and sync `TODO.md` if needed.
11. Run final relevant verification after documentation updates if necessary.
12. Review the worktree, stage all relevant changes, and create a descriptive git commit for this task.
13. Stop without starting the next task.

## Progress Log

- 2026-05-05: Initial plan written before reading project files or running commands.
- 2026-05-05: Identified `P7-T02V` as the first incomplete detailed task. Latest commit is `[P7-T02V] Record callable-value run-pass blocker`, so the recorded blocker is directly in scope.
- 2026-05-05: Focused fixture status: `receiver_function_value_call_basic.scoop` and `top_level_callable_value_call_basic.scoop` pass; `unsafe_funptr_direct_named_call_basic.scoop`, `unsafe_funptr_receiver_call_basic.scoop`, and the combined callable-value fixture fail on default refactor run-pass. Investigation now targets `FunPtr` direct/named/receiver lowering and ABI behavior.
- 2026-05-05: Implemented the main fix path. MIR lowering now treats `FunPtr<F>` as callable, effect facts read `FunPtr<F>` surface rows, refactor LLVM MIR lowering emits native indirect calls for local `FunPtr` direct calls and `scoop.unsafe.invoke`, and top-level function-value direct calls load the authoritative top-level immutable closure instead of reusing a skipped callee temp. The six focused callable/FunPtr fixtures now pass individually.
- 2026-05-05: Marked `P7-T02V` complete in `TODO-P7.md` and synced `TODO.md`. Required callable-value/FunPtr fixtures, focused `scoopc` LLVM/effect tests, formatting, and clippy pass. An extra broad guard `cargo test -p scoop --test p7_default_pipeline` still fails in the async/await test's HandleDispatch completion-payload path; that is outside this task's callable-value/FunPtr completion criteria and remains for P7 full regression follow-up.

## Current Task

- Task: `P7-T02V` in `TODO-P7.md`.
- Goal: fix default refactor run-pass blockers for callable-value receiver, pattern binders, and `FunPtr` direct calls.
- Required focused fixtures:
  - `tests/fixtures/run-pass/receiver_function_value_call_basic.scoop`
  - `tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
  - `tests/fixtures/run-pass/unsafe_funptr_direct_named_call_basic.scoop`
  - `tests/fixtures/run-pass/unsafe_funptr_receiver_call_basic.scoop`
  - `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`
