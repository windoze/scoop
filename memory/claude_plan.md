## Execution Plan (high level)

### Plan
1. Inspect the latest Git commit message and diff; if it mentions known issues (failing tests, TODOs, regressions), fix those first.
2. Read `TODO.md` and identify the first incomplete task.
3. If the first task is too large, decompose it into smaller subtasks; update `PLAN.md` and replace/augment the task in `TODO.md`. Treat the first new subtask as the task for this run.
4. Implement the first incomplete task (or first subtask) completely.
5. Run thorough validation:
   - `cargo fmt --all`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --all`
   - Any task-specific commands (e.g. fixture runner) if relevant.
6. Document progress:
   - Mark the task completed in `TODO.md`.
   - Update `PLAN.md` to reflect the new state and any plan adjustments.
   - Update this file with what was completed.
7. Commit changes with a descriptive message.
8. Stop (do not start the next task).

### Progress log
- 2026-04-10: Plan created.
- 2026-04-10: Committed T0131 uncommitted changes (interface ToString + generic print/println).
- 2026-04-10: T0128 COMPLETED — generic class GC verification with 5 run-pass fixtures, all GC stress stable.

### Completed: T0128

**Scenarios verified**:
1. Generic class holding reference field → `generic_class_gc_ref_field.scoop` ✓
2. Generic class holding value type → `generic_class_gc_value_field.scoop` ✓
3. Generic class holding nullable reference → `generic_class_gc_nullable_ref.scoop` ✓ (new)
4. Generic type with specialized types → `generic_class_gc_specialized_type.scoop` ✓ (fixed: length→size)
5. GC allocation point safety → `generic_class_gc_multi_alloc.scoop` ✓

All 5 fixtures pass under SCOOP_GC_STRESS=1. 139 unit tests + 834 fixtures pass.
