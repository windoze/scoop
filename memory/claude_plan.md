## Execution plan

1. Read `TODO.md` to find the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect the task's referenced code, fixtures, and validation requirements.
4. Review finding: the handler `on` implementation and fixtures are present, but `docs/spec/language_spec-part1.md` still omits `on` from the keyword list.
5. Fix the split spec keyword list so it matches the parser surface.
6. Validation completed successfully: formatting, clippy, spec fixture check, targeted handler/with-update fixtures, full Rust tests, and full fixture suite.
7. `TODO.md` and `TODO-2.md` now mark P2-T02R as `[DONE]` with the completion record filled in; no phase-level `PLAN.md` update was needed.
8. Commit all task-related changes with a descriptive message and stop without starting the next task.
