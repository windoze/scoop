# Claude Execution Plan

## Scope

- Follow `TODO.md` as the global index and the referenced `TODO-Px.md` files as the source of truth.
- Complete exactly the first incomplete detailed task, then stop after committing the result.
- Do not perform broad historical issue triage before selecting the current task.
- Do not use workarounds, narrowed fixtures, or spec deviations to make progress.

## Execution Steps

1. Read `TODO.md` to identify the ordered detailed task files and task ids.
2. Inspect the referenced `TODO-Px.md` files in order and select the first task whose detailed heading is not prefixed with `[DONE]`.
3. Check the latest commit only for an explicitly mentioned unfinished issue directly relevant to that selected task.
4. Read the selected task body, constraints, dependencies, validation requirements, and completion-record format.
5. Explore only the code and tests relevant to that selected task.
6. Implement the smallest spec-correct change needed to complete the task.
7. Add or update tests and fixtures required by the task.
8. Run the relevant validation commands, and fix any issues that block the selected task.
9. Mark the detailed task heading `[DONE]`, update its completion record, and sync `TODO.md` if the indexed entry appears there.
10. Update this plan file at key milestones or if the plan changes.
11. Commit all task-related changes, including any pre-existing uncommitted files if this is a resumed unfinished task state.
12. Stop without starting the next task.

## Blocker Handling

- If a concrete prerequisite prevents correct implementation, keep the current task incomplete.
- Add the minimum prerequisite task in the correct `TODO-Px.md` position and sync `TODO.md`.
- Update `PLAN.md` only if phase-level sequencing or completion criteria change.
- Commit the bookkeeping changes and stop.

## Progress Log

- Initial plan written before inspecting task files or running commands.
- Selected first incomplete detailed task: `P7-T02X` in `TODO-P7.md`.
- Latest commit `b7e20be7 [P7-T03] Add continuation composition prerequisite` is directly relevant because it introduced/tracks the continuation-composition blocker now represented by `P7-T02X`.
- Next steps: inspect the failing fixture and relevant continuation provenance/composition implementation, reproduce the failure, implement the smallest spec-correct fix, validate with the task commands, then mark `P7-T02X` done and commit.
- Reproduced target behavior: `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop` prints only `40` and `-1`; fixture test exits non-zero instead of producing final `12`.
- Diagnosis: late-lowered dump shows cross-call readback provenance for `k.resume(5)` already resolves to `start`'s Ask binder route, but resume-boundary outward dispatch still hands the extracted callee continuation directly to the handler binder. The fix should publish resume-boundary continuation-composition contracts and let composed resume dispatch consume both call-boundary and resume-boundary contracts.
- Implemented fix: resume-boundary lowering now publishes continuation composition contracts, LLVM composed resume dispatch consumes both call-boundary and resume-boundary compositions, and ABI layout preserves same-owner resume-boundary site inventory while still using cross-owner underlying routes when needed.
- Validation so far: target fixture now prints `40`, `-1`, `12`; fixture harness passes; `cargo test -p scoopc --lib effect_lowered`, `cargo test -p scoopc --lib llvm::codegen::effect_refactor`, and `cargo clippy --all-targets -- -D warnings` pass.
- Task bookkeeping updated: `TODO-P7.md` marks `P7-T02X` as `[DONE]` with completion notes, and `TODO.md` is synchronized with the same marker. Next step is to inspect the final diff and commit the task changes.
