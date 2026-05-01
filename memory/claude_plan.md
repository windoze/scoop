## Execution Plan

Note: I do not record private chain-of-thought verbatim. This file contains a concise execution plan and progress log.

1. Read `TODO.md` as the task index.
2. Open the referenced `TODO-Px.md` files in listed order and identify the first detailed task that is not clearly completed there.
3. Check the latest commit message for any directly relevant unfinished work tied to that task.
4. Inspect only the code and tests needed for that task and any immediate blockers.
5. Implement the task fully, or if blocked by a concrete prerequisite, add the minimum prerequisite task in the appropriate `TODO-Px.md`, sync `TODO.md`, and stop.
6. Run the relevant verification commands, including targeted tests first and broader checks as required by the task.
7. Update the detailed task file with a completion record if finished. Sync `TODO.md` if task ids, titles, files, or ordering changed. Update `PLAN.md` only if phase-level planning changed.
8. Commit the resulting changes with a task-specific message and stop after this single task.

## Progress Log

- Plan file created.
- Read `TODO.md`, `TODO-P0.md`, `TODO-P1.md`, and `TODO-P2.md` in order.
- Identified the first incomplete detailed task as `P2-T01` in `TODO-P2.md`.
- Checked the latest commit message: `[P1-T03R] Confirm P1 is ready for typed work`; it does not record a directly relevant unfinished issue for `P2-T01`.
- Inspected current `effect_refactor_pipeline`, `dump_hir`, `ast_stage`, and `hir::lower_*` implementations.
- Execution focus for `P2-T01`:
  1. Add a dedicated refactor typed HIR stage module and output type.
  2. Route refactor `dump-hir` through that stage instead of legacy `hir::lower_for_dump`.
  3. Keep legacy `dump-hir` behavior unchanged.
  4. Add focused tests proving the new stage is constructible and the refactor route uses it.
  5. Run the task-specific verification commands, update `TODO-P2.md`, and commit once the task is complete.
- Added `crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs` with `TypedHirStageOutput` and placeholder `TypedHirEffectContracts`.
- Updated `effect_refactor_pipeline` so refactor `dump-hir` now goes through the new typed HIR stage, while legacy `dump-hir` still uses `hir::lower_for_dump`.
- Updated `crates/scoop/src/commands/dump_hir.rs` to branch by pipeline mode at the command edge and added tests for refactor-vs-legacy routing.
- During verification, `commands::parity::dump_hir_cli_parity_matches_legacy_and_refactor` failed because `P2-T01` intentionally switches refactor `dump-hir` to typed HIR output. I updated the parity guard to keep cross-mode success/stderr checks for HIR without incorrectly requiring identical stdout.
- Verification completed successfully:
  - `cargo test -p scoopc --no-default-features refactor_typed_hir_stage`
  - `cargo test -p scoopc --no-default-features effect_refactor_pipeline`
  - `cargo test -p scoop --no-default-features dump_hir`
  - `cargo test -p scoop --no-default-features parity`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-hir tests/fixtures/hir/minimal.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/hir/minimal.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
