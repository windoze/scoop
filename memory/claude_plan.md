# Claude Execution Plan

## Scope

Complete exactly the first incomplete detailed task referenced by `TODO.md`, then stop after committing the completed work.

## Reasoning Summary

The detailed `TODO-Px.md` files are the source of truth for task completion state. I will first use `TODO.md` only as an index, then inspect the referenced detailed TODO files in order. A task is complete only if its detailed heading is explicitly prefixed with `[DONE]`; completion notes alone are not sufficient.

I will avoid broad historical triage. Existing issues are in scope only if they block the selected task, invalidate its specified behavior, or are direct regressions introduced while completing it. If a blocker prevents spec-correct implementation, I will add the minimum prerequisite task in the appropriate detailed TODO file, sync `TODO.md`, commit that bookkeeping, and stop.

## Step-by-Step Plan

1. Read `TODO.md` as the task index.
2. Inspect the referenced `TODO-Px.md` files in order to identify the first detailed task whose heading is not prefixed with `[DONE]`.
3. Read the selected task body, dependencies, constraints, validation requirements, and completion record.
4. Inspect the relevant code and tests for that task only.
5. Implement the smallest spec-correct change that fully satisfies the selected task.
6. Add or update tests/fixtures required by the selected task.
7. Run the task-specific validation commands, then broader relevant checks if needed.
8. If validation exposes a blocker directly relevant to the task, fix it; if it cannot be fixed within the current task without a new prerequisite, update the TODO files accordingly, commit, and stop.
9. Mark the selected task heading as `[DONE]` in the authoritative `TODO-Px.md` file and update its completion record.
10. Sync `TODO.md` so the index matches any completed, added, removed, renamed, or reordered tasks.
11. Update this plan file with key progress and final validation results.
12. Commit all relevant changes with a clear task-tagged commit message.
13. Stop without starting the next task.

## Progress Log

- Initial plan written before repository inspection.
- Selected first incomplete task: `P7-T01R` in `TODO-P7.md`.
- Task type: review selector default flip, verifying omission defaults to refactor and explicit legacy remains the only temporary rollback/compare entry.
- Initial review findings: `SessionOptions::default()` resolves to refactor, `scoop` and `scoopc` CLI omission both parse as refactor, and explicit legacy remains routed through session options. No latest-commit blocker was identified; latest commit is `[P7-T01] Default effect pipeline to refactor`.
- Validation completed successfully: P7-T01 selector tests, smoke commands, required `rg` search, `cargo fmt --all`, and `cargo clippy --all-targets -- -D warnings` all passed.
- Task bookkeeping completed: `P7-T01R` marked `[DONE]` in `TODO-P7.md`, and `TODO.md` synchronized.
