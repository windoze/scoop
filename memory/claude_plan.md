# Execution Plan

## Scope
- Follow `TODO.md` as the authoritative ordered task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Selected task for this invocation: `P3-T04` in `TODO-4.md`, "切换下游 MIR 查询到 `mir_facts` / pass artifacts surface".
- This file records an execution plan and progress log; it does not contain private chain-of-thought.

## Step-by-Step Plan
1. Check the latest commit subject/body only for unfinished work directly relevant to `P3-T04`.
2. Inspect the current downstream MIR query paths named by `P3-T04`: effect facts stage, effect lowering stage, effect facts/lowered builders, and LLVM bridge call sites.
3. Inspect `scoopc_mir_facts` and MIR pass view APIs to identify the narrow query surface already available and the smallest missing facts needed for downstream use.
4. Move MIR-derived nominal metadata ownership from LIR/effect lowering into MIR stage/facts, especially replacing `collect_nominal_direct_supertypes_from_mir_file(...)` style recomputation with published `MirFacts` data.
5. Update downstream inputs and call sites so migrated facts are read through `MirFacts` or canonical pass query surface, without copying `MirFacts` into later stage outputs or preserving duplicate owners.
6. Add or adjust focused tests that prove migrated MIR-derived facts are not recomputed downstream.
7. Run the validation required by `P3-T04`: `cargo fmt`, targeted `scoopc` tests for `effect_facts_stage`, `effect_lowering_stage`, and `effect_lowered`, fixture tests for `tests/fixtures/effect_lowered`, and `cargo clippy --all-targets -- -D warnings`.
8. If a concrete blocker prevents spec-correct completion, add the minimum prerequisite task to `TODO.md` / `TODO-4.md`, commit that bookkeeping, and stop.
9. If `P3-T04` is completed, update `TODO.md` and `TODO-4.md` by marking `P3-T04` as `[DONE]`, fill the completion record with scope, decisions, validation, and residual risks, then commit all task-related changes.

## Progress Log
- Read `TODO.md`; first incomplete task is `P3-T04` in `TODO-4.md`.
- Read `TODO-4.md`; `P3-T04` requires downstream MIR root/pass/global fact queries to use `mir_facts` / pass artifacts surface, including moving nominal direct supertypes out of LIR-side MIR-file recomputation.
- Latest commit is `[P3-T03R] Review MIR snapshot handoff`; it does not explicitly mention unfinished work that blocks `P3-T04`.
- Found the concrete duplicate owner: effect lowering and `LateLoweredProgramBuilder` were collecting nominal direct supertypes by scanning MIR files. Added MIR-owned metadata facts and switched the builder call chain to consume those facts instead.
- Implementation now removes the old `collect_nominal_direct_supertypes_from_mir_file` path, adds `EffectFactsStageOutput` / `EffectLoweredStageOutput` accessors for `MirFacts`, and updates affected tests to consume the MIR-owned metadata fact.
- Validation completed so far: `cargo fmt`, `cargo test -p scoopc_mir_facts`, `cargo test -p scoopc --no-default-features mir_stage`, `cargo test -p scoopc --no-default-features effect_facts_stage`, `cargo test -p scoopc --no-default-features effect_lowering_stage`, `cargo test -p scoopc --no-default-features effect_lowered`, `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`, `cargo clippy --all-targets -- -D warnings`, repository search for removed nominal recompute helper, and `git diff --check`.
- Updated `TODO.md` and `TODO-4.md` to mark `P3-T04` as `[DONE]` with completion scope, validation, and residual P4/P5/P7 transition risks.
