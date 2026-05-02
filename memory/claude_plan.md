# Claude Plan

## Initial Plan

1. Read `TODO.md` as the task index.
2. Open the referenced detailed task files in order (`TODO-P0.md`, `TODO-P1.md`, `TODO-P2.md`, etc.) and identify the first task whose heading is not prefixed with `[DONE]`.
3. Check the latest commit message for any unfinished issue directly relevant to that task.
4. Implement the selected task completely without introducing workarounds.
5. Run the relevant formatting, tests, and lint checks required by the task and repository policy.
6. Update the detailed TODO file completion record and add `[DONE]` to the task heading when the task is actually complete.
7. Sync `TODO.md` if task status, title, ordering, or references changed.
8. Update this plan file with progress and any material plan changes while working.
9. Commit all required changes with a task-specific message, then stop.

## Reasoning Summary

- I will first identify the authoritative current task from the detailed TODO files before doing broader investigation.
- If I hit a concrete blocker that prevents spec-correct completion, I will add the minimum prerequisite task in the appropriate detailed TODO file, sync `TODO.md`, record the blocker here, commit, and stop.
- I will avoid exposing private internal chain-of-thought and instead keep this file as a concise decision log, execution plan, and progress record.

## Progress Log

- Plan initialized before repository inspection.
- Read `TODO.md` index and `TODO-P6.md`; confirmed the first incomplete detailed task is `P6-T01a` (add fail-fast guard so refactor LLVM stage does not silently fall back to the legacy effect backend).
- Next steps: inspect the latest commit message for directly relevant unfinished work, inspect the current refactor LLVM stage and the legacy-effect entry points it still reaches, then implement the minimal guard plus tests and required TODO bookkeeping.
- Checked the latest commit: `[P6-T01R] Record legacy effect backend blocker`. It directly matches the current task and confirms the intended scope.
- Implementation plan refined:
  1. Add a dedicated LLVM emit error for “refactor effect lowering not migrated yet”.
  2. In the refactor LLVM emit path, inspect reachable late-lowered callables before body emission.
  3. If a reachable callable still needs outward-case / boundary / resume-state lowering, fail fast instead of entering the legacy handler-stack backend.
  4. Keep non-effectful samples working unchanged.
  5. Add a regression test that proves an effectful refactor build now fails explicitly.
- Code changes applied:
  - Added `LlvmEmitError::RefactorEffectLoweringUnsupported`.
  - Added reachable-callable inspection in `crates/scoopc/src/llvm/emit.rs` for refactor stage outputs.
  - Added a regression test covering effectful refactor build rejection in `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`.
- Next step: run formatting and the required targeted validations, then update TODO bookkeeping and commit.
- Validation update:
  - `cargo test -p scoopc refactor_llvm_codegen_stage` passed after widening the guard to inspect `entry main + reachable callees`.
  - Refactor non-effectful CLI checks passed for `.ll`, `.o`, `.s`, and `run` on minimal fixtures.
  - Refactor effectful CLI check now fails explicitly on `tests/fixtures/effect_facts/handle_perform.scoop` with `RefactorEffectLoweringUnsupported` instead of silently entering the legacy backend.
- Bookkeeping update:
  - Marked `P6-T01a` as `[DONE]` in `TODO-P6.md` and synced `TODO.md`.
- Final step remaining: inspect the final diff, commit all task-related changes, then stop.
