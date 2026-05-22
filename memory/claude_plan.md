# Claude Execution Plan

## Scope

Complete exactly the first incomplete task listed in `TODO.md`, then stop after validation, documentation updates, and a git commit.

## Plan

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Review the selected task's requirements, dependencies, validation steps, and completion record.
3. Check the recent commit only for unfinished work directly relevant to the selected task.
4. Inspect the affected code, fixtures, and tests needed for the selected task.
5. Implement the smallest correct change that fully satisfies the task without workarounds.
6. Add or update tests/fixtures required by the task.
7. Run targeted validation first, then any broader validation required by the task.
8. If a concrete blocker prevents correct completion, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
9. If the task is completed, mark its `TODO.md` heading with `[DONE]` and update its completion record.
10. Update this plan file whenever a key step completes or the plan changes.
11. Inspect git status and diff, then commit all intended changes with a descriptive task-tagged message.
12. Stop without starting the next task.

## Progress

- Initial execution plan recorded before repository inspection.
- Selected first incomplete task: `P6-T03R` in `TODO-6.md`.
- Latest commit is `[P6-T03] Split object once and storage policy`; it is directly relevant and is the implementation under review.
- Current review checks: object once only serves object singletons, top-level eager init no longer uses once first-access, and `@Global` / `@ThreadLocal` / `@Extern` storage policy is consistent through HIR/MIR/LIR/codegen/runtime.
- Code review pass completed across the P6-T03 implementation paths. `scoop_once_begin/end` runtime declarations are only called from `object_init.rs`; top-level `val` eager init uses the compiler-private guard state machine; `@ThreadLocal` roots get TLS storage and a worker-thread init hook through `scoop_thread_init_current`; storage policy conflict is rejected in typecheck.
- No source fix has been identified yet; next step is the required validation matrix plus the extra `scoop_once` residual search.
- Validation completed successfully: `cargo fmt`, storage policy unit test, object-once LLVM unit tests, typecheck fixtures, global-init fixtures, runtime tests, clippy with `-D warnings`, and `git diff --check` all passed.
- `TODO.md` and `TODO-6.md` were updated to mark `P6-T03R` as `[DONE]` with the review conclusion and validation record.
- Next step is final git inspection and commit for `P6-T03R`; no next TODO task will be started in this invocation.
