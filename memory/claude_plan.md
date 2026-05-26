Execution plan for this invocation:

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Inspect the selected task's nearby requirements, dependencies, validation notes, and completion record.
4. Implement the task as written, without narrowing scope or using workarounds for missing language/runtime behavior.
5. If a concrete prerequisite blocks the task, update `TODO.md` with the minimum prerequisite task in dependency order, commit that bookkeeping, and stop.
6. Otherwise, run formatting, linting, relevant tests, and then any required full suites in the required order.
7. Address any unscheduled test or fixture failures by fixing them or scheduling them explicitly before marking the task complete.
8. Mark the completed task heading in `TODO.md` with `[DONE]`, update its completion record, and update this progress file at key milestones.
9. Commit all changes for this single completed task with an appropriate message and stop without starting the next task.

Progress update:

- Selected first incomplete task: `P1-T01` (`tools/run_fixtures.py`).
- Probed the legacy runner against public commands and found a blocker: current approved commands lack a generic resolve/typecheck-only frontend check surface. `dump-hir` misses many expected typecheck failures, while `build-single-cone` runs too far and fails valid typecheck-only fixtures during HIR/lowering.
- Plan changed to schedule the minimum prerequisite command-surface task before `P1-T01`, update the design/plan/TODO bookkeeping, commit that change, and stop.
