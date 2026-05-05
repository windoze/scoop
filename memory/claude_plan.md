# Claude Execution Plan

## Scope

- Follow the task workflow exactly: read `TODO.md` as the index, inspect detailed `TODO-Px.md` files in order, select the first incomplete detailed task, complete exactly that task, document it, commit, and stop.
- Keep `TODO-Px.md` as the source of truth. Synchronize `TODO.md` only when task status, titles, files, or ordering change.
- Do not update `PLAN.md` unless phase-level sequencing, assumptions, dependencies, or completion criteria change.

## Step-by-Step Plan

1. Inspect repository task state starting from `TODO.md`, then read the referenced detailed task files in order.
2. Identify the first detailed task whose heading is not prefixed with `[DONE]`.
3. Review the selected task requirements, dependencies, validation instructions, and completion record.
4. Inspect only the code and tests relevant to that task; include the latest commit context only if it explicitly mentions an unfinished issue directly relevant to the selected task.
5. Implement the smallest spec-correct change needed for the selected task. Do not introduce workarounds or weaker fixtures.
6. Add or update tests/fixtures required by the task.
7. Run the task-specified validation and any targeted tests needed to prove the change.
8. If a concrete blocker prevents spec-correct completion, add the minimum prerequisite task in the appropriate detailed TODO file, sync `TODO.md`, commit that bookkeeping, and stop.
9. If the task is completed, prefix the detailed task heading with `[DONE]`, update its completion record, and sync the matching `TODO.md` index entry.
10. Run final relevant validation after documentation updates.
11. Commit all uncommitted files related to this invocation with a clear task-id commit message.
12. Stop without starting the next task.

## Progress Log

- Started invocation and recorded the execution plan before inspecting task files or running repository commands.
- Read `TODO.md` and `TODO-P7.md`. Selected first incomplete detailed task: `P7-T02S` in `TODO-P7.md`.
- Current task focus: finish the remaining default refactor LLVM/lowering gaps for build fixtures, especially the current `task_atomic_claim_no_mutex_llvm.scoop` blocker after `P7-T02T`.
- Reproduced the current blocker: default refactor O0 build of `task_atomic_claim_no_mutex_llvm.scoop` failed on `Task.__state` member access result type drift.
- Implemented a targeted O0 MIR local storage fix for member-access locals whose materialized source type still contains type params, and added an O0 fixture-backed LLVM test.
- Continued fixing build-path-only exposure: compiler-temporary member access locals now consume the concrete field contract, materialized generic callable source IDs are restored after slot creation, and refactor lowering emits `unreachable` after `Never` assignments such as `panic(...)`.
- Verified `task_atomic_claim_no_mutex_llvm.scoop` now passes through `scoop test` on the default refactor path.
- Ran the P7-T02S directed fixture set and related scoopc/scoop targeted tests successfully.
- Marked `P7-T02S` as `[DONE]` in `TODO-P7.md` and synchronized the `TODO.md` index.
- Ran `cargo clippy --all-targets -- -D warnings` successfully and added it to the completion record.
