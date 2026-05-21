# Current Invocation Plan

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Review only the context needed for that task, including `PLAN.md`, recent commit metadata, and relevant source or fixture files.
3. Implement the task exactly as specified, without narrowing scope or using workarounds.
4. Run targeted validation first, then broader required checks from the task where feasible.
5. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in its completion record; update `PLAN.md` only if phase-level planning changes.
6. Commit all files relevant to this invocation with a task-specific commit message.
7. Stop after completing exactly one task.

Progress:
- Initial plan recorded before inspecting the current task.
- Identified first incomplete task: `P3-T02` in `TODO-4.md`, dependent on completed `P3-T01R`.
- Latest commit is `[P3-T01R] Review MIR facts crate`; it matches the completed prerequisite and does not add an unfinished blocker for `P3-T02`.
- Implemented root inventory ownership in `MirFacts` and removed the parallel root map fields from `MirStageOutput`.
- Updated MIR stable dumps and `mir_lowered` fixtures to show the `mir_facts` boundary.
- Marked `P3-T02` as complete in `TODO.md` and `TODO-4.md` after validation.

Task-specific plan:
1. Inspect current `MirStageOutput` root inventory fields and `scoopc_mir_facts` root inventory model.
2. Extend `MirFacts` root inventory entries so they can own FQN, stable identity, item index, span/source/type references, and root kind where available without depending on MIR stage types.
3. Build `MirFacts` during MIR stage construction from `LoweredMir.file` and remove parallel root map fields from `MirStageOutput`.
4. Rewrite existing root query methods to locate MIR items via `MirFacts`.
5. Add or update unit tests and stable dumps to show MIR/facts alignment.
6. Run the task-required validation commands, fix failures, then mark `P3-T02` complete and commit.

Validation completed:
- `cargo fmt`
- `cargo test -p scoopc_mir_facts`
- `cargo test -p scoopc --no-default-features mir_stage`
- `cargo test -p scoopc --no-default-features hir_preflight`
- `cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered`
- `cargo clippy --all-targets -- -D warnings`
- `git diff --check`
