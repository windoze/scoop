# Claude Execution Plan

## Scope

Complete exactly the first incomplete detailed task referenced by `TODO.md`, then stop after committing the resulting changes.

## Plan

1. Inspect `TODO.md` as the task index.
2. Open the referenced `TODO-Px.md` files in order and identify the first task whose detailed heading is not prefixed with `[DONE]`.
3. Check the latest commit message only for unfinished work directly relevant to that selected task.
4. Implement the selected task as written, without narrowing scope or using workaround behavior.
5. If a concrete prerequisite blocks correct implementation, add the minimum prerequisite task in the correct `TODO-Px.md`, sync `TODO.md`, commit, and stop.
6. Run the task-specific validation and any broader relevant checks required by the task.
7. Mark the completed task heading with `[DONE]` in its `TODO-Px.md` completion record, and sync `TODO.md` if the task appears there.
8. Commit all relevant changes with a task-specific message.
9. Stop without starting the next task.

## Progress Log

- Initial plan created before inspecting project task files.
- Selected current task: `P6-T03b` in `TODO-P6-part3.md` because it is the first detailed task without a `[DONE]` heading.
- Task goal: publish source-slice statement classification, render it in `dump-effect-lowered`, validate it at handoff/materialization boundaries, and make refactor LLVM body lowering fail fast on unclassified or unsupported statements instead of silently skipping.
- Implemented the main handoff shape in code: `LateLoweredCallable` now carries source statement classifications; builder/materialization publishes them; stable dump renders them; LLVM ABI materialization validates them; refactor body lowering consumes them instead of local skip heuristics.
- Validation completed: target classification tests passed, both required `dump-effect-lowered` commands succeeded, and `cargo clippy --all-targets -- -D warnings` passed.
- Marked `P6-T03b` as `[DONE]` in `TODO-P6-part3.md` and synced the `[DONE]` marker in `TODO.md`.
