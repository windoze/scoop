# Execution Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Inspect the task's referenced code, fixtures, and validation requirements without doing broad unrelated triage.
4. Implement the selected task exactly as specified, adding prerequisite TODO entries instead of workarounds if a blocking spec or implementation gap is found.
5. Run the task's required validation and any narrowly relevant tests; fix failures that are in scope.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and recording completion details. Update `PLAN.md` only if phase-level sequencing changed.
7. Commit all relevant changes with a descriptive task-tagged message, then stop without starting the next task.

## Progress

- Initial execution plan recorded before reading task details.
- Selected first incomplete task: `P13-T02` from `TODO.md` / `TODO-5.md`.
- Scope confirmed as documentation-only: update `MANAGED_ABI.md` §2.2 typical examples and completion records; no code changes expected.
- Updated `MANAGED_ABI.md` §2.2, validated the rendered section via `cat MANAGED_ABI.md`, and ran `cargo clippy --all-targets -- -D warnings` successfully.
- Marked `P13-T02` as `[DONE]` in `TODO.md` and `TODO-5.md`; final whitespace check passed. Commit remains.
