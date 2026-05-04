# Claude Execution Plan

## Scope

Complete exactly the first incomplete detailed task referenced by `TODO.md`, then commit and stop.

## Selected Task

- First incomplete detailed task: `P6-T02qg` in `TODO-P6-part2.md`.
- Title: 发布 non-`Unit` completion payload source / return-value contract，禁止 P6-T03 在 backend 回 raw MIR/tail shape 恢复完成值。
- Latest commit `[P6-T02qg] Track completion payload contract prerequisite` is directly relevant, so this invocation continues that task.

## Execution Plan

1. Inspect only the relevant late-lowered IR/materialization/dump and refactor LLVM ABI query code paths for completion terminators.
2. Add an explicit completion payload contract to the late-lowered representation, distinguishing `Unit` completion from non-`Unit` payload sources.
3. Materialize the payload source from existing P5 facts/state graph without letting P6 recover it from raw MIR or source shape.
4. Render the contract in `dump-effect-lowered` so non-`Unit` complete paths are visible.
5. Publish and validate the same contract through refactor LLVM ABI query/materialization with fail-fast checks for missing or type-drifting payloads.
6. Add targeted tests for late-lowered completion payload contracts and LLVM ABI query contracts, including `effect_resume_if_else_branch_single_perform.scoop`.
7. Run the task-required validation commands and any formatting/lint checks needed for touched crates.
8. Mark `P6-T02qg` `[DONE]` in `TODO-P6-part2.md`, sync `TODO.md`, update the completion record, commit all relevant changes, and stop.

## Progress

- Plan initialized before repository inspection.
- Read `TODO.md`; first incomplete indexed task is `P6-T02qg`.
- Read `TODO-P6-part2.md`; detailed heading for `P6-T02qg` is not `[DONE]`, so it is the current task.
- Implemented completion payload source publication in late-lowered state terminators, frame bindings, stable dumps, and refactor LLVM ABI query.
- Added targeted late-lowered and LLVM query tests for non-`Unit` completion payload sources, missing contracts, and source/type drift.
- Marked `P6-T02qg` as `[DONE]` in `TODO-P6-part2.md` and synced the `TODO.md` index.

## Validation

- `cargo test -p scoopc refactor_effect_lowered_completion_payload_contract`
- `cargo test -p scoopc refactor_llvm_completion_payload_contract`
- `cargo test -p scoopc refactor_llvm_`
- `cargo test -p scoopc refactor_effect_lowered_`
- `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
- `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
- `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
