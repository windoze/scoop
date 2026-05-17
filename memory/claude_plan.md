# Claude Execution Plan

## Current Invocation

- Goal: complete exactly the first incomplete task from `TODO.md`, then stop.
- Source of truth: use `TODO.md` for task ordering, requirements, dependencies, validation, and completion records.
- Plan:
  1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
  2. Check recent git context only as needed for the selected task, especially if the latest commit explicitly references an unfinished issue relevant to it.
  3. Read the task's referenced code, tests, fixtures, and project context.
  4. Implement the task directly, without narrowing scope or using workarounds.
  5. If a concrete blocker prevents spec-correct implementation, add the minimum prerequisite task to `TODO.md`, keep the current task incomplete, commit that bookkeeping, and stop.
  6. Run targeted validation first, then broader relevant validation required by the task.
  7. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record.
  8. Update this file after key milestones or any plan change.
  9. Commit all relevant changes with a descriptive task-tagged message.
  10. Stop without starting the next task.

## Progress Log

- Initial plan recorded before task inspection.
- Identified first incomplete task: `P11-T01` in `TODO-5.md`.
- Task scope: audit usage of `__scoop_gc_collect`, `__scoop_gc_debug_alloc_garbage`, `__scoop_gc_debug_heap_object_count`, and `__scoop_stackmap_statepoint_smoke`; record fixture usage and migration decisions for P11-T02 without implementing the migration yet.
- Completed the audit record in `TODO-5.md` and marked `P11-T01` done in `TODO.md` / `TODO-5.md`.
- Key decision update: `__scoop_gc_debug_heap_object_count` can move as C ABI leaf, but `__scoop_gc_debug_alloc_garbage` and `__scoop_stackmap_statepoint_smoke` must keep GC-aware/managed special handling in the future test cone.
- Validation completed: required `rg` hit-count checks and overlay/stackmap usage checks match the completion record; `git diff --check` reports no whitespace errors.
