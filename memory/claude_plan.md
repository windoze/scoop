# Claude Plan

## Execution Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Read the selected task requirements, dependencies, validation requirements, and completion-record format.
4. Inspect the minimal relevant code and fixture areas needed for that task.
5. Implement the task as written without narrowing scope or introducing workarounds.
6. Run the task-specific validations, then broader relevant checks if needed.
7. If a concrete blocker prevents correct implementation, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
8. If the task is completed, prefix the task heading in `TODO.md` with `[DONE]`, update its completion record, commit all relevant changes, and stop.

## Progress

- Created initial execution plan before running project commands.
- Identified first incomplete task: `P6-T01` in `TODO-6.md`.
- Latest commit is `[TODO-6-INIT] Detail final pipeline tasks`; no separate unfinished issue was advertised beyond the current TODO-6 setup.
- Next step is focused inspection of LIR facts, LIR facts builder, effect lowering stage, and MIR root/facts inputs for the P6-T01 contract.
- Inspection confirmed `scoopc_lir_facts` has no global init/storage facts yet, and MIR facts currently expose initializer dependency counts but not dependency identities.
- Implementation approach: publish dependency identities in MIR facts, add a backend-neutral global init/storage group to LIR facts, then build it from `MirFacts` in `lir_facts_builder`.
- Implemented MIR initializer dependency facts, LIR global init/storage contracts, verifier checks, stable dump output for non-empty global facts, and builder construction from `MirFacts`.
- Validation passed: `cargo fmt`; `cargo test -p scoopc_lir_facts`; `cargo test -p scoopc --no-default-features lir_facts_builder`; `cargo test -p scoopc_mir_facts`; `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`; `git diff --check`; `cargo clippy --all-targets -- -D warnings`.
- TODO bookkeeping is complete; final `git diff --check` passed after documentation updates. Next step is committing the `P6-T01` changes only.
