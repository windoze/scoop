# Claude Execution Plan

## Guardrails

- Use `TODO.md` as the authoritative task source and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Treat `PLAN.md` as phase-level context only; update it only if phase sequencing or dependencies change.
- Avoid workarounds or weakened fixtures. If a spec-correct implementation is blocked by a missing prerequisite, record that prerequisite in `TODO.md`, commit, and stop.
- Preserve unrelated worktree changes and do not revert user edits.

## Initial Plan

1. Read `TODO.md` first to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Inspect the relevant implementation and tests for that task.
4. Implement the smallest spec-correct change needed for the selected task.
5. Run the required task-specific validation and broader relevant tests.
6. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in its completion record, or add a concrete prerequisite if blocked.
7. Update this plan file after key discoveries or changes.
8. Commit all relevant changes with a task-scoped commit message and stop.

## Progress

- Initial execution plan recorded before inspecting project files.
- `TODO.md` inspection selected the first incomplete task: `MIR-T12：建立 codegen routing / ABI handoff 守卫`.
- Latest commit is `[MIR-T11R] Review generic materialization contract`; no directly relevant unfinished `MIR-T12` work was found.
- Implemented MIR-owned codegen routing facts and verifier, publishing final P4 routing facts from effect facts and exposing route preflight in `dump-mir` / `dump-ir` output.
- Added `codegen_routing_contracts.scoop` and targeted tests for route publication, raw-route rejection, `NoOutward`/EffectStep ABI drift, and frontend-reject route diagnostics.
- Updated late-lowered statement classification so `Unsupported` source classifications fail during handoff instead of reaching backend emission.
- Validation completed so far: `refactor_mir_codegen_routing_contract`, `refactor_materialized_mir_codegen_route_verifier`, `dump-mir` / `dump-ir` / `dump-effect-lowered` on the routing fixture, plus relevant `scoop` dump CLI tests.
- `TODO.md` now marks `MIR-T12` as `[DONE]` with validation results. `PLAN.md` was not changed because phase-level sequencing did not change.
- Final validation also passed: `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/codegen_routing_contracts.scoop` and `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`.

## Current Task Plan: MIR-T12

1. Check the latest commit only for unfinished work directly relevant to `MIR-T12`.
2. Inspect existing MIR materialization, strict verifier, effect-lowered/codegen handoff, and test helpers around routing/ABI facts.
3. Add MIR-owned codegen routing facts for materialized callables, including unsupported effect/control site classification and ABI publication reason.
4. Add strict route verifier checks for raw-route unsupported terminators/call kinds, `PerformResult` binding, plain-vs-EffectStep ABI drift, and unsupported source classifications.
5. Make dump/preflight output expose routing facts enough for downstream codegen tasks.
6. Add focused tests and the `mir_refactor/codegen_routing_contracts.scoop` fixture covering success and forged-fact negative cases.
7. Run the task-specified test commands plus minimal relevant regressions, then update `TODO.md`, commit, and stop.
