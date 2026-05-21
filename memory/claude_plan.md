# Claude Execution Plan

## Current Invocation

Goal: complete exactly the first incomplete task in `TODO.md`, validate it, update task records, commit the result, then stop. This file records the actionable execution plan, decisions, and progress log for the invocation.

Selected task: `P5-T02R` in `TODO-5.md`, a review task for `P5-T02` covering LIR callable, dynamic invoke, dispatch, and resume contracts.

## Execution Plan

1. Confirm the selected task remains the first incomplete task and inspect the latest commit for directly relevant unfinished work.
2. Review `P5-T02` requirements and completion notes against the actual implementation in `crates/scoopc_lir_facts/`, `crates/scoopc/src/effect_lowered/`, `crates/scoopc/src/pipeline/lir_facts_builder.rs`, and effect-lowered fixtures.
3. Verify that `LirFacts` covers callable inventory, plain callable contracts, effect-step contracts, dynamic invoke contracts, dispatch owner/slot contracts, continuation/resume publication, verifier coverage, and stable dump output.
4. Search for `LateLoweredProgramBuilder::from_canonical_inputs` and confirm every production LIR stage construction synchronously builds `LirFacts`.
5. If review finds a real P5-T02 blocker, fix it in this review task; if it requires a new prerequisite that cannot be completed here, update `TODO.md`/`TODO-5.md`, commit the blocker record, and stop.
6. Run the required P5-T02 validation set for the review: `cargo fmt`, `cargo check -p scoopc_lir_facts`, `cargo test -p scoopc_lir_facts`, `cargo test -p scoopc --no-default-features effect_lowered`, `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
7. Mark `P5-T02R` as `[DONE]` in both `TODO.md` and `TODO-5.md`, and fill its completion record with review findings, validation commands, and residual risk.
8. Update `PLAN.md` only if the review changes phase-level sequencing or completion criteria; otherwise leave it unchanged.
9. Inspect git status, diff, and recent log; commit all relevant changes for this task with a task-tagged message.
10. Stop without starting `P5-T03`.

## Progress Log

- Read `TODO.md` and `TODO-5.md`; first incomplete task is `P5-T02R` because `P5-T02` is marked `[DONE]` and the review heading remains `[TODO]`.
- Wrote this invocation plan before running build/test/code commands or editing implementation files.
- Checked current status and recent commits; only this plan file is dirty, and the latest commit `[P5-T02] Publish LIR contract facts` is directly the review target with no separate unfinished issue called out.
- Reviewed the P5-T02 implementation surface. `LirFacts` publishes callable inventory, plain/effect-step contracts, dynamic invoke, dispatch, resume packing, continuation object, and surface-resume dispatch groups; production `LirStageOutput` construction builds and verifies `LirFacts` immediately after each `LateLoweredProgramBuilder::from_canonical_inputs(...).build()?` path. Remaining `from_canonical_inputs` matches are tests/raw helpers, not alternate production outputs.
- Validation passed: `cargo fmt`, `cargo check -p scoopc_lir_facts`, `cargo test -p scoopc_lir_facts`, `cargo test -p scoopc --no-default-features effect_lowered`, `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
- Marked `P5-T02R` complete in `TODO.md` and `TODO-5.md`; no `PLAN.md` update was needed because this review did not change phase-level sequencing or completion criteria.
