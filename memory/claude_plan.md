# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not proceed to any later task after completing or blocking the current task.

## Initial Steps

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit message only for unfinished work that is directly relevant to that selected task.
3. Inspect the relevant code and fixtures for that task without broad unrelated triage.

## Execution Plan

1. Implement the selected task as specified, avoiding workaround behavior or scope narrowing.
2. Add or update focused tests/fixtures required by the task.
3. Run the task-specified validation and any directly relevant checks.
4. If a failure blocks the task and is not already scheduled, either fix it or insert the minimum prerequisite task into `TODO.md`, then stop.
5. Mark the selected task `[DONE]` in `TODO.md` only after implementation and validation are complete.
6. Update the task completion record with files changed, tests run, and result notes.
7. Commit all intended changes for this invocation with a descriptive task-tagged message.
8. Stop after the commit.

## Progress Log

- Plan initialized before reading task details.
- Selected first incomplete task: `P9-T06` in `TODO-7.md`.
- Latest commit is `[P9-T06-b] Publish LIR-owned ordinary-callee suspend contract`, which is the direct prerequisite for this task and does not add a separate unfinished blocker.
- Current task requirements: create `scoopc_effect_facts_stage` and `scoopc_lir`, move effect-facts builder/stage glue and `effect_lowered`, update `scoopc` façade and pipeline imports, activate dependency-gate checks, and switch `scoopc_codegen_llvm` to direct `scoopc_lir` usage.
- Validation target: `cargo fmt`, `cargo build --workspace`, `cargo test --all --all-targets`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`, `cargo run -p scoop_tools -- dependency-gate`, `cargo tree -p scoopc_lir --depth 1`, and `git diff --check`.
- Implementation approach: physically move `effect_facts` into `scoopc_effect_facts_stage` and `effect_lowered` into `scoopc_lir`; keep `scoopc` as façade only. The P4 stage will publish the data-only `scoopc_effect_facts::EffectFacts` product plus effect-owned type context, while `scoopc_lir` reconstructs the legacy MIR-keyed view internally from the published facts and MIR stable instance keys so it does not depend on the effect builder stage.
- Progress: `cargo check --workspace` passes after extracting `scoopc_effect_facts_stage` and `scoopc_lir` with the current `scoopc` façade. Next step is removing the temporary `scoopc_codegen_llvm -> scoopc` dependency and compiling LLVM codegen against direct LIR inputs.
- Blocker discovered: `scoopc_codegen_llvm` cannot be switched off the `scoopc` normal dependency by import rewiring alone because `llvm/emit.rs` and `llvm/frontend.rs` still consume `scoopc`-owned frontend and pipeline handoff APIs. Added new prerequisite `P9-T06-c` before `P9-T06` to publish a codegen-owned LLVM stage handoff before completing the direct dependency switch. The direct-codegen attempt was reverted to keep the tree compiling.
