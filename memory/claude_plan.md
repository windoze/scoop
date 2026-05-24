# Current Invocation Plan

## Scope

- Source of truth: `TODO.md`.
- Goal: identify and complete exactly the first incomplete task whose heading is not prefixed with `[DONE]`, then stop.
- Constraint: do not proceed to the next task after completing or blocking the selected task.

## Execution Steps

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent git context only as needed for unfinished work directly relevant to that task.
3. Inspect the affected implementation, tests, fixtures, and docs for the selected task.
4. Implement the task as specified, without narrowing scope or using workarounds.
5. Run focused validation first, then broader required validation from the task; address any unscheduled failures discovered.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and recording completion evidence, or add the minimum prerequisite/blocker task if completion is impossible.
7. Update this plan file after key milestones or plan changes.
8. Review git status and diffs, then commit all relevant changes with a task-scoped message.
9. Stop after the commit.

## Current Status

- First incomplete task identified: `P9-T06R` in `TODO-7.md`.
- Task type: review of `P9-T06` extraction for `scoopc_effect_facts_stage` and `scoopc_lir`.
- Review focus: LIR direct dependencies must not include HIR/AST/umbrella `scoopc`; `scoopc_codegen_llvm` must depend directly on `scoopc_lir`; effect facts builder must live and function in the new stage crate.
- Initial review result: crate manifests and direct cargo trees show `scoopc_lir` and `scoopc_codegen_llvm` have the expected direct dependency direction.
- Issue found: `dependency_gate` does not yet actively check `scoopc_effect_facts_stage`, even though P9-T06 required activating both `scoopc_effect_facts_stage` and `scoopc_lir`.
- Fix completed: added an `effect-facts-stage` dependency-gate crate kind/check, plus tests for accepted dependencies and rejected LIR/codegen/umbrella dependencies.
- Documentation completed: marked `P9-T06R` as `[DONE]` in `TODO-7.md`, updated the root `TODO.md` index/status, and recorded validation evidence.
- Validation completed: `cargo fmt`; `cargo build --workspace`; `cargo test --all --all-targets`; `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`; `cargo run -p scoop_tools -- dependency-gate`; `cargo tree -p scoopc_lir --depth 1`; `cargo tree -p scoopc_codegen_llvm --depth 1`; `cargo tree -p scoopc_effect_facts_stage --depth 1`; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
- Next step: inspect final diff/status and commit the P9-T06R changes.
