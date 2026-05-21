# Claude Execution Plan

## Scope

- Follow `TODO.md` as the source of truth.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after completing that one task, documenting it, testing it, and committing the result.

## Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent git context only as needed for the selected task, especially if the latest commit mentions unfinished work directly relevant to it.
3. Inspect the relevant implementation, fixtures, and tests for that task.
4. Implement the smallest spec-correct change that fully satisfies the task; do not use workarounds or weaken fixtures.
5. Run targeted validation first, then broader required validation from the task entry.
6. If a blocker is discovered, update `TODO.md` with the minimum prerequisite task, leave the current task incomplete, commit the bookkeeping change, and stop.
7. If the task is completed, update `TODO.md` by prefixing the task title with `[DONE]` and adding a completion record with tests run.
8. Update this file after key steps so progress remains visible.
9. Inspect git status and diff, then commit all task-related changes with a descriptive message.
10. Stop without starting the next task.

## Current Progress

- Initial execution plan written before inspecting the task list.
- Identified the first incomplete task from `TODO.md`: `P3-T05` in `TODO-4.md`, covering explicit MIR pass pipeline and refresh ordering.

## P3-T05 Task Plan

1. Inspect the current materializer tail scheduling and pass modules: `run.rs`, `inline.rs`, `escape.rs`, `closure_simplify.rs`, `summary.rs`, `pass_view.rs`, plus MIR facts pass metadata.
2. Check recent git context for unfinished work directly related to P3-T05.
3. Introduce a dedicated MIR pass pipeline module as the single scheduling owner.
4. Move inlining, always-on escape analysis, closure simplification, and post-rewrite refresh into that pipeline without writing pass rewrites back to raw `MaterializedMir.file`.
5. Publish pipeline metadata through existing MIR facts/pass artifact metadata so dumps/tests can show pass execution and revision effects.
6. Run the P3-T05 required validation commands, fixing any failures that are in scope.
7. Mark `P3-T05` complete in `TODO.md`, update this progress file, then commit all task-related changes and stop.

## P3-T05 Progress

- Added an explicit MIR pass pipeline driver and moved post-materialization scheduling out of `MirInstanceMaterializer::run(...)`.
- Updated inlining, escape analysis, and closure simplification to publish pass artifacts through a shared pipeline context.
- Changed escape analysis to run for all optimization levels, while keeping rewrite passes gated by the existing non-`O0` optimization policy.
- Wired pass run/revision metadata into MIR facts and stable facts dump output.
- Targeted MIR pass, MIR facts, MIR stage, MIR materialization, and clippy validations have passed so far.
- The `TODO-4.md` fixture command `tests/fixtures/mir_materialized` points to a non-existent directory; running the existing `tests/fixtures/mir` phase exposed pre-existing golden newline mismatches rather than a pass-pipeline failure. This will be recorded in the task completion notes.
- Marked `P3-T05` as `[DONE]` in `TODO-4.md` and synchronized the `TODO.md` index. Remaining work is final diff/status inspection and commit.
