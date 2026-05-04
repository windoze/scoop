# Claude Execution Plan

## Scope

- Follow the repository task workflow and complete exactly the first incomplete detailed task found through `TODO.md` and the referenced `TODO-Px.md` files.
- Treat `TODO-Px.md` as the source of truth when it disagrees with `TODO.md`.
- Do not proceed to any later task after the current detailed task is completed or blocked.

## Plan

1. Read `TODO.md` as the task index.
2. Open referenced `TODO-Px.md` files in index order and identify the first task whose heading is not prefixed with `[DONE]`.
3. Inspect the selected task requirements, dependencies, validation instructions, and completion record.
4. Check recent git context only as needed for the selected task, especially if the latest commit names an unfinished issue relevant to it.
5. Implement the task directly and avoid narrowing scope or using workarounds.
6. If a concrete prerequisite or spec mismatch blocks the task, add the minimum prerequisite task in the appropriate detailed TODO file, sync `TODO.md`, commit that bookkeeping, and stop.
7. Run the task-specific validation plus relevant broader tests required by the repository guidance.
8. Mark the completed task heading with `[DONE]`, update its completion record, and sync `TODO.md` if the indexed title/status changed.
9. Run formatting or final verification if needed.
10. Commit all relevant changes with a descriptive task-tagged commit message.
11. Stop after the commit.

## Progress Log

- Plan initialized before repository inspection.
- `TODO.md` inspected. The first incomplete indexed task appears to be `P6-T03R` in `TODO-P6-part3.md`; next step is to inspect that detailed task as source of truth.
- `TODO-P6-part3.md` confirms `P6-T03R` is the first incomplete detailed task. This invocation is a review task: audit clean LLVM body lowering against `P6-T03a` through `P6-T03i`, run the required validation matrix, fix any blocking findings or record concrete prerequisites, then mark only `P6-T03R` complete.
- Review found a blocking P6-T03h regression: continuation protocol tests expect explicit `ResumeUnwind` / `Abandon` lowering paths, but current body lowering still groups them with `Unreachable`. The plan is updated to first restore explicit verifier/lowering functions, then run the P6-T03R validation matrix.
- Additional P6-T03R review findings are being fixed in the same task: prevent refactor closure carrier materialization from defining legacy lambda bodies, fail fast on ambiguous suspend primary boundaries, fail fast on direct-entry args ABI drift instead of default values, and reject non-elided handle payload binders without payload.
- The review fixes have been implemented and initial targeted tests/fixture checks passed. Validation is moving to the full P6-T03R matrix: broad refactor LLVM/effect-lowered unit tests, required dump/build/run-pass fixtures, the required forbidden-pattern search, clippy, and then TODO completion updates.
- P6-T03R validation completed. Required task-specific unit tests, dump/build/run-pass fixtures, required `rg`, and `cargo clippy --all-targets -- -D warnings` passed. `TODO-P6-part3.md` and `TODO.md` were updated to mark only `P6-T03R` as `[DONE]`; next step is git diff/status review and commit.
