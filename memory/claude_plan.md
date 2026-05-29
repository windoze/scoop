# Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after completing and committing that one task, or after committing any required prerequisite/task-list update if the task is blocked.

## Steps

1. Read `TODO.md` first and identify the first incomplete task.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect the task's referenced code, tests, fixtures, and specifications.
4. Implement the task without narrowing scope or using workaround behavior.
5. Update or add the smallest relevant tests/fixtures for the task.
6. Run validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then relevant/full tests and fixtures as needed.
7. If any unscheduled test or fixture failure is observed, fix it or add the minimum prerequisite/follow-up task in `TODO.md` before marking the current task complete.
8. Mark the task title `[DONE]` in `TODO.md` and update its completion record after implementation and validation pass.
9. Commit all task-related changes with a descriptive message.
10. Stop without starting the next task.

## Progress

- Plan initialized before repository inspection.
- First incomplete task identified: `P2-T03R` in `TODO-2.md`, review of the hard-cap/OOM work from `P2-T03`.
- Latest commit `af1cbbde [P2-T03] Add GC hard cap OOM path` is directly relevant and will be reviewed as the task target.
- Review findings to fix before completion:
  - Immix minor-GC to-space block allocation can allocate new blocks without checking `SCOOP_GC_MAX_HEAP_BYTES`.
  - LLVM generated allocations can dereference the result of `scoop_alloc_typed` without guarding the NULL OOM path.
- Implemented fixes in progress:
  - Added pending to-space reserve accounting to Immix hard-cap checks.
  - Routed generated `scoop_alloc_typed` calls through an internal checked wrapper that traps via `scoop_runtime_error_fatal(NULL)` on OOM.
  - Added regressions for nursery to-space hard-cap enforcement and generated-code OOM trap behavior.
- Validation progress:
  - `cargo fmt` passed.
  - `cargo clippy --all-targets -- -D warnings` passed.
  - Targeted hard-cap runtime regressions passed.
  - Rebuilt workspace tools and the generated-code OOM fixture passed.
- Full validation passed:
  - `cargo test --all --all-targets`
  - `python3 tools/spec_fixtures.py check`
  - `python3 tools/run_fixtures.py`
- `P2-T03R` marked `[DONE]` in `TODO.md` and `TODO-2.md` with completion record.
