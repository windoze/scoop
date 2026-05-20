# Execution Plan

## Current Invocation

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for an unfinished issue directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and nearby completion records.
4. Inspect only the code and tests needed to understand and implement that task.
5. Implement the selected task as written, without narrowing scope or using fixture-only workarounds.
6. If a concrete prerequisite blocks correct implementation, update `TODO.md` with the minimum prerequisite task, record the blocker here, commit that bookkeeping, and stop.
7. Run focused tests first, then the task-required validation commands. Fix any task-relevant regressions.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
9. Update `PLAN.md` only if the phase-level plan or dependency structure changes.
10. Inspect git status and diff, then commit all changes required for this completed task with a descriptive task-tagged message.
11. Stop after this one task.

## Progress Log

- Initial execution plan recorded before repository commands.
- Selected first incomplete task: `P0-T04` in `TODO-1.md` / `TODO.md`.
- Latest commit is `[P0-T03R] Record review completion`; it does not introduce a separate unfinished prerequisite for `P0-T04`.
- Focus for this invocation: classify remaining old `comptime` / Scoop `const` surface hits, clean active docs/fixtures/sysroot references, run required validation, mark only `P0-T04` done, and commit.
- Completed key cleanup edits in active implementation/docs: renamed annotation retention local policy, renamed the legacy metadata list type to `MetaList` in sysroot/overlays, rewrote active spec docs away from removed compile-execution surface, and updated B-24 audit docs.
- Revised keyword cleanup after parser verification: `comptime` must remain a reserved keyword tombstone so old statement forms fail during ordinary parsing instead of being reinterpreted as identifier expressions plus normal control flow. No AST/parser/lowering surface is restored.
- Verification completed: `cargo fmt`, full no-default test suite, full fixture suite, spec fixture check, clippy, and old-surface searches have passed after fixing task-blocking validation drift/flakes.
