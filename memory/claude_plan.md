# Claude Execution Plan

## Scope
- Execute exactly the first incomplete task in `TODO.md`.
- Treat `TODO.md` as authoritative for ordering, requirements, validation, and completion records.
- Stop after completing one task, documenting it, and committing the changes.

## Plan
1. Read `TODO.md` to identify the first heading that is not prefixed with `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Inspect the selected task's referenced code, fixtures, and validation requirements.
4. Implement the task with minimal, spec-correct changes and no workaround behavior.
5. Run targeted validation first, then any task-required broader validation.
6. If an unscheduled failing test or fixture is found, fix it or add the minimum prerequisite task in `TODO.md` before marking the current task complete.
7. Mark the completed task title with `[DONE]` in `TODO.md` and update its completion record.
8. Update this progress file after key steps and before any plan changes.
9. Inspect git status and diff, then commit all relevant changes with a task-tagged message.
10. Stop without starting the next task.

## Progress
- Created initial execution plan before inspecting project task state.
- Read `TODO.md`; first incomplete task is `P9-T05` (`scoopc_mir` crate extraction).
- Read `TODO-7.md` P9-T05 requirements and recent commits; latest commit is P9-T04R and is directly aligned with the selected task sequence.
- P9-T05 requires creating `scoopc_mir`, moving MIR/monomorph/RTTI/mangler ownership, preserving the umbrella façade, updating dependency gate, and validating the workspace plus run-pass fixtures.
- Created `crates/scoopc_mir`, moved `mir/`, `monomorph/`, and `rtti/` sources there, and changed the `scoopc` umbrella to re-export the new crate's MIR surfaces.
- Moved production HIR stage facts construction into `scoopc_hir::stage` to avoid `scoopc_mir -> scoopc` back edges; removed the old uncompiled HIR stage files from the umbrella pipeline.
- `cargo check --workspace` and `cargo test --all --all-targets` pass after updating moved audit paths and MIR placeholder inventory roots.
- Completed validation for P9-T05: `cargo fmt`, `cargo build --workspace`, `cargo test --all --all-targets`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, `cargo tree -p scoopc_mir`, and `git diff --check` all pass.
- Marked P9-T05 `[DONE]` in `TODO.md` and `TODO-7.md` with completion notes.
