# Claude Execution Plan

## Scope

Complete exactly one detailed TODO task in this invocation: the first incomplete task found by reading `TODO.md` as the index and then checking the referenced `TODO-Px.md` files in task order. Stop after implementing, validating, documenting, and committing that one task, or after committing any required prerequisite/blocker bookkeeping if the task cannot be completed spec-correctly.

## Execution Plan

1. Read `TODO.md` as the global index only.
2. Open the referenced `TODO-Px.md` files in indexed order and identify the first task whose detailed heading is not prefixed with `[DONE]`.
3. Check the latest commit message only for unfinished work directly relevant to that selected task.
4. Inspect the selected task requirements, constraints, dependencies, validation steps, and completion-record format in the authoritative `TODO-Px.md` file.
5. Examine only the code, fixtures, docs, and tests needed to implement that task correctly.
6. Implement the smallest spec-correct change without workarounds or fixture-only special cases.
7. Add or update focused tests/fixtures required by the task.
8. Run the task-specific validation commands first, then broader relevant checks if needed.
9. If a concrete blocker prevents correct implementation, add the minimum prerequisite task in the appropriate `TODO-Px.md`, sync `TODO.md`, record the blocker here, commit, and stop.
10. If validation passes, mark the selected detailed task heading as `[DONE]`, update its completion record, and sync the same `[DONE]` marker in `TODO.md` if present there.
11. Update this plan file when key steps complete or if the plan changes.
12. Review the final diff, commit all relevant uncommitted changes with a descriptive task-tagged message, and stop without starting the next task.

## Progress

- Initial execution plan written before reading TODO files or running commands.
- Read `TODO.md`; selected first incomplete detailed task `P6-T02qh` in `TODO-P6-part3.md`.
- Latest commit `f6ccbdcd [P6-T03] Record wrapper completion projection blocker` is directly relevant to `P6-T02qh`, so its blocker context will be treated as part of this task.
- Initial focused tests passed for existing `P6-T02qh` coverage. Remaining implementation work is to add explicit owner/wrapper same-answer-type coverage for the `OwnerComplete` payload source path.
- Implemented same-answer-type tests and a wrapper owner-trampoline path that returns `WrapperPayload` complete directly from the published handle arm completion source.
- `cargo test -p scoopc refactor_effect_lowered_surface_resume_wrapper_completion` passed.
- `cargo test -p scoopc refactor_llvm_surface_resume_wrapper_completion` passed.
- `dump-effect-lowered` for `effect_multi_escape_indirect_direct_while.scoop` shows the published `owner Unit -> wrapper Int` payload source (`payload=local6:t5`).
- The required refactor run-pass fixture still fails because call-boundary local consumption discards the callee continuation, so the escaped continuation resumes the caller directly and skips the callee `fetch_resume` path. This is a distinct missing continuation-composition contract, so the plan changes to add the minimum prerequisite task before `P6-T02qh`, keep `P6-T02qh` incomplete, sync `TODO.md`, commit, and stop.
- Added prerequisite task `P6-T02qga` in `TODO-P6-part3.md`, updated `P6-T02qh` dependency/blocker record, and synced the new task into `TODO.md`.
