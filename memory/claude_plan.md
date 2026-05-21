# Claude Execution Plan

## Scope

- Authoritative task source: `TODO.md`, with detailed task body in `TODO-5.md`.
- Current invocation target: complete exactly `P5-T01` (`建立 scoopc_lir_facts crate 与正式 LirStageOutput 壳层`), then stop.
- Do not start `P5-T01R` or later tasks.
- If a concrete blocker prevents spec-correct `P5-T01`, update `TODO.md`/`TODO-5.md` with the minimum prerequisite task, commit that bookkeeping, and stop.

## Selected Task

- First incomplete task found in `TODO.md`: `P5-T01`.
- Required outcome: create an independent `scoopc_lir_facts` fact crate and replace or wrap `EffectLoweredStageOutput` as a formal `LirStageOutput = { lir, lir_facts }` shell.
- Required constraints: `LirStageOutput` must not embed `EffectFactsStageOutput` or other upstream stage output bundles; `scoopc_lir_facts` must obey fact-crate dependency rules; avoid broad LLVM backend cleanup in this task.

## Step-by-Step Plan

1. Check latest git commit and working tree state to identify direct unfinished context for `P5-T01` and avoid touching unrelated user changes.
2. Inspect the existing LIR/effect-lowered pipeline, output shape, dump/test entry points, dependency gate, workspace manifests, and README sections relevant to fact crates.
3. Add `crates/scoopc_lir_facts/` with crate-level docs, `#![forbid(unsafe_code)]`, a minimal `LirFacts` data product, verifier/dump skeleton, and focused unit tests.
4. Register `scoopc_lir_facts` in the workspace, `scoopc` dependencies, and `scoop_tools` dependency gate as a fact crate.
5. Refactor `EffectLoweredStageOutput` into a formal `LirStageOutput` shell, keeping `LateLoweredProgram` as the LIR body and publishing `LirFacts`; retain only necessary explicit base context/type context, not upstream stage output wrappers.
6. Update pipeline facade, dump naming, tests, and README/documentation references so public API and output labels describe the LIR stage.
7. Run the task-required validation: `cargo fmt`, `cargo check -p scoopc_lir_facts`, `cargo test -p scoopc_lir_facts`, `cargo test -p scoopc --no-default-features effect_lowering_stage`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, plus `git diff --check`.
8. Fix validation failures caused by the current changes without using workarounds or weakening the required shape.
9. Mark `P5-T01` as `[DONE]` in both `TODO.md` and `TODO-5.md`, and fill the completion record with scope, decisions, validation commands, and residual risks.
10. Review git status/diff/log, stage the intended files, commit with a `P5-T01` message, and stop.

## Progress

- `TODO.md` inspected; first incomplete task is `P5-T01` in `TODO-5.md`.
- `TODO-5.md` inspected; `P5-T01` requires `scoopc_lir_facts`, formal `LirStageOutput`, no nested P4 output, dependency gate registration, and the listed cargo validations.
- Git context checked; working tree only contains this plan update, and latest commit `[P4-T04R] Review P4 completion` has no explicit unfinished issue beyond handing off to `P5-T01`.
- Relevant code inspected: current `EffectLoweredStageOutput` stores `MirStageOutput`, `EffectFactsStageOutput`, and `LateLoweredProgram`; dependency gate tracks HIR/MIR/effect fact crates; README documents P4 but not LIR facts yet.
- Implementation direction set: add `scoopc_lir_facts`, introduce `LirStageOutput` with `lir` + `lir_facts`, keep only an explicit temporary context for current backend/test compatibility instead of nested upstream stage-output wrappers, and leave broad codegen query migration to `P5-T03`.
- Implementation completed: added `scoopc_lir_facts`, registered it in workspace/dependency gate, added `StableLirCallableKey`, introduced `LirStageOutput`/`lir_facts()` and LIR dump naming, and updated README.
- Validation completed successfully: `cargo fmt`, `cargo check -p scoopc_lir_facts`, `cargo test -p scoopc_lir_facts` (after rerun; first attempt timed out waiting on cargo locks), `cargo test -p scoopc --no-default-features effect_lowering_stage`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
- `TODO.md` and `TODO-5.md` updated to mark `P5-T01` as `[DONE]` with completion notes, validation commands, and residual risk assigned to P5-T02/P5-T03.
