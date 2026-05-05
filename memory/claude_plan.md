# Claude Execution Plan

## Scope

- Follow the task workflow from the root task index and detailed TODO files.
- Complete exactly the first incomplete detailed task, then stop after committing.
- Keep task bookkeeping synchronized between the detailed TODO file and `TODO.md`.

## Execution Plan

1. Read `TODO.md` as the task index.
2. Open the referenced `TODO-Px.md` files in index order and identify the first detailed task whose heading is not prefixed with `[DONE]`.
3. Check the latest commit message for an unfinished issue only if it is directly relevant to the selected task.
4. Inspect the code and tests needed for that task, avoiding unrelated historical triage.
5. Implement the smallest spec-correct change for the selected task.
6. Add or update focused tests/fixtures required by the task.
7. Run relevant validation commands, fixing any task-relevant failures.
8. Mark the task heading as `[DONE]` in its detailed TODO file and update its completion record.
9. Sync `TODO.md` if the completed task appears in the index or task ordering/title changed.
10. Update this file after key milestones or plan changes.
11. Commit all relevant changes with a clear task-scoped commit message.
12. Stop without starting the next task.

## Progress

- Plan initialized before repository inspection.
- Selected first incomplete detailed task: `P7-T02R` in `TODO-P7.md`.
- Task type: review task. Scope is to inspect the P7-T02 changes, rerun required P7-T02 validation commands, search for hidden legacy fallback/default residue, update completion records, commit, and stop.
- Inspection checkpoint: default/session/fixture paths show omission propagates refactor by default; explicit legacy is appended only for legacy fixture subprocesses; P7 default-vs-explicit tests cover dump/build/run/test fixture entries.
- Next validation: rerun P7-T02 command matrix plus P7-T02R search.
- Validation checkpoint: P7-T02 targeted tests and CLI smoke commands passed; implementation-scope search in `crates tools tests` only reports expected unsupported diagnostics and hidden-fallback assertion text.
- Remaining validation: `cargo clippy --all-targets -- -D warnings`.
- Final validation checkpoint: `cargo clippy --all-targets -- -D warnings` passed.
- Bookkeeping checkpoint: marking `P7-T02R` done in `TODO-P7.md` and syncing `TODO.md`.
- Bookkeeping completed; preparing task-scoped commit. Generated untracked `crates/scoop/target/` is test output and will not be included.
