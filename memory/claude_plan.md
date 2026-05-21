# Current Execution Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Complete exactly the first task whose title is not prefixed with `[DONE]`.
- Do not advance to the next task in this invocation.
- If a concrete blocker or prerequisite is discovered, update `TODO.md`, commit that bookkeeping, and stop.

## Step-by-Step Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for unfinished work directly relevant to that task.
3. Inspect the code and tests needed for the selected task.
4. Implement the required change without narrowing scope or introducing workarounds.
5. Run focused validation first, then broader required validation for the task.
6. Fix any task-relevant failures discovered during validation.
7. Update `TODO.md` by prefixing the completed task title with `[DONE]` and recording the completion details.
8. Update this progress file after key milestones.
9. Inspect the final diff and git status.
10. Commit all task-related changes with a descriptive task-tagged commit message.
11. Stop after the commit.

## Progress Log

- Plan initialized before inspecting project files or running commands.
- Identified `P3-T03` as the first incomplete task from `TODO.md` / `TODO-4.md`.
- Latest commit is `[P3-T02R] Review MIR root inventory migration`, which matches the immediately preceding task and does not add a separate prerequisite.
- Current worktree only contains this progress file before task implementation.
- Inspected the MIR stage boundary, materialization output, pass view, effect facts stage, and existing `scoopc_mir_facts` snapshot/pass metadata skeleton.
- Implementation direction: split direct-style-only MIR output from P4-ready `MirStageOutput`, make P4 input carry a mandatory canonical `MaterializedMir`, and populate `MirFacts` with snapshot, family, and pass artifact metadata when the handoff is built.
- Implemented the boundary split and mandatory P4 handoff, removed the missing-snapshot effect-facts error path, and added tests for P4-ready snapshot/family/pass artifact facts.
- Validation completed so far: `cargo fmt`; `cargo test -p scoopc_mir_facts`; `cargo test -p scoopc --no-default-features mir_stage`; `cargo test -p scoopc --no-default-features effect_facts_stage`; `cargo test -p scoopc --no-default-features effect_lowering_stage`; `cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`; `cargo clippy --all-targets -- -D warnings`.
- Updated `TODO.md` and `TODO-4.md` to mark `P3-T03` done and record the implementation, validation, and residual risks.
- Final diff/status/log inspection completed; preparing to commit the `P3-T03` changes.
