# Execution Plan

## Scope

- Work on exactly the first incomplete task in `TODO.md`.
- Treat `TODO.md` as the source of truth for ordering, requirements, dependencies, validation, and completion records.
- Stop after completing and committing that one task, or after committing any required prerequisite/blocker task updates.

## Steps

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Review only the task-relevant context, including recent commit information if it directly mentions unfinished work relevant to that task.
3. Implement the task as written, without narrowing scope or using workaround representations.
4. Run targeted tests first, then any validation commands required by the task.
5. Fix any directly relevant failures introduced by or blocking the current task.
6. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in the completion record.
7. Commit all files relevant to this task with a clear task-tagged message.
8. Stop without starting the next task.

## Progress Log

- Plan initialized before reading project task details.
- `TODO.md` identifies `P2-T04` as the first incomplete task: migrate declaration/entity facts into `HirFacts` and reduce/remove overlapping `ProgramFacts` query surfaces.
- Next steps: inspect task-relevant fact models and consumers, then make targeted code changes and run the validations listed for `P2-T04`.
- Expanded the `scoopc_hir_facts` declaration/type-context model so HIR facts can represent field identities, enum variants, source-cone ownership without fake source ids, and duplicate checks for the new declaration facts.
- Wired HIR stage fact construction for declarations, fields, enum variants, globals, native/extern metadata, source cones, and type parameter ownership; removed the old overlapping `ProgramFacts` module in favor of a source-site-only migration bridge for P2-T05.
- Updated HIR declaration/entity consumers to read `HirFacts`, regenerated HIR golden snapshots for the new facts dump, and completed required test/clippy validation successfully.
- Marked `P2-T04` as `[DONE]` in `TODO.md` and `TODO-3.md` with completion notes and validation results. Next step is to inspect the final diff and commit only this task's changes.
