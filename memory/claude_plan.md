# Claude Plan

## Working Notes

I will keep this file updated with an external execution log and plan summary. It will not contain private hidden reasoning, but it will record the concrete steps, decisions, blockers, and verification progress for this invocation.

## Initial Plan

1. Read `TODO.md` as the task index.
2. Follow the referenced detailed task files (`TODO-P0.md`, `TODO-P1.md`, `TODO-P2.md`, etc.) in order.
3. Identify the first detailed task that is not clearly recorded as completed in its authoritative `TODO-Px.md` section.
4. Check the latest commit message to see whether it mentions unfinished work directly relevant to that task.
5. Inspect the code and tests relevant to the selected task.
6. Implement the task completely if possible.
7. Run focused validation first, then broader required validation such as relevant `cargo test` / `cargo clippy --all-targets -- -D warnings` if appropriate for the touched area and task requirements.
8. If I discover a concrete blocker that makes correct completion impossible, add the minimum prerequisite task to the correct detailed TODO file, sync `TODO.md`, and stop.
9. If I complete the task, record completion in the authoritative `TODO-Px.md` file and sync `TODO.md` only if task metadata or ordering changed.
10. Commit exactly the changes for this invocation with a task-based commit message, then stop.

## Update Policy

- Update this file after identifying the current task.
- Update this file before substantial edits.
- Update this file after validation results are known.
- Update this file if the plan changes or a blocker is found.

## Current Task

- Selected task: `P2-T01R` in `TODO-P2.md`.
- Reason: `P0` and `P1` tasks are recorded complete; `P2-T01` is complete; `P2-T01R` is the first detailed task whose completion record is still empty.
- Latest commit subject checked: `[P2-T01] Route refactor dump-hir through typed HIR stage`.
- Relevance assessment: the latest commit is directly related to `P2-T01R`, but it does not explicitly record an unfinished issue that must be added as a prerequisite.

## Task-Specific Review Plan (`P2-T01R`)

1. Inspect the new refactor typed HIR stage module and its dispatcher wiring.
2. Inspect `crates/scoop/src/commands/dump_hir.rs` to confirm `legacy` and `refactor` paths are intentionally separated at the command boundary.
3. Inspect `crates/scoopc/src/hir/lower/mod.rs` and relevant shared APIs to ensure no pipeline-mode branching was added inside legacy HIR/typecheck business logic.
4. Search `crates/scoopc/src/hir` and `crates/scoopc/src/typecheck` for `EffectPipelineMode|refactor|legacy`, then manually classify any hits.
5. Re-run the targeted validations required by `P2-T01R`.
6. If the review passes, write the completion record into `TODO-P2.md`, keep `TODO.md` unchanged unless metadata/order changed, commit, and stop.
7. If the review uncovers a real blocker for `P2-T02`, add the minimum prerequisite task(s) in `TODO-P2.md`, sync `TODO.md`, commit, and stop.

## Review Findings

- No blocker was found that requires inserting a new prerequisite before `P2-T02`.
- The latest commit is directly relevant to this review, but it did not explicitly note unfinished follow-up work.
- `crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs` now owns the refactor typed HIR stage entry and wraps the shared typed lowering result inside `TypedHirStageOutput`.
- `crates/scoopc/src/effect_refactor_pipeline/refactor.rs` routes `StageKind::TypedHir` directly to `hir_stage::run(...)`.
- `crates/scoop/src/commands/dump_hir.rs` keeps the command-boundary split explicit: `legacy` calls `scoopc::hir::lower_for_dump(...)`, while `refactor` calls `scoopc::effect_refactor_pipeline::load_typed_hir_stage_output_for_dump(...)`.
- `crates/scoopc/src/hir/lower/mod.rs` still exposes ordinary shared lowering APIs and does not contain pipeline-mode branching.
- Search results in `crates/scoopc/src/hir` / `crates/scoopc/src/typecheck` did not show pipeline-selector logic in business code. Remaining text hits were limited to legacy naming/diagnostics such as `legacy_eager_hir` and removed-syntax errors, not runtime routing branches.

## Validation Results

- Passed: `cargo test -p scoopc --no-default-features refactor_typed_hir_stage`
- Passed: `cargo test -p scoopc --no-default-features effect_refactor_pipeline`
- Passed: `cargo test -p scoop --no-default-features dump_hir`
- Passed: `cargo test -p scoop --no-default-features parity`
- Passed: `cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-hir tests/fixtures/hir/minimal.scoop`
- Passed: `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/hir/minimal.scoop`
- Passed: `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
- Passed: `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`

## Remaining Steps

1. Write the `P2-T01R` completion record into `TODO-P2.md`.
2. Check git status/diff/log for the commit step.
3. Create one commit for this invocation and stop.
