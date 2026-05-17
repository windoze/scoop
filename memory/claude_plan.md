# Claude Execution Plan

## Scope
- Follow `TODO.md` as the source of truth.
- Complete exactly the first task whose title is not prefixed with `[DONE]`.
- Do not continue to the next task after completion.

## Plan
1. Read `TODO.md` and identify the first incomplete task exactly as written.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect the code and tests needed for that task, avoiding unrelated issue triage.
4. Implement the smallest spec-correct change that fully satisfies the task.
5. Add or update focused tests/fixtures required by the task.
6. Run the task's required validation commands and any targeted tests needed for confidence.
7. If a concrete blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
8. If the task is completed, prefix the task title in `TODO.md` with `[DONE]` and update its completion record.
9. Commit all current uncommitted task-related changes with a clear task-tagged message.
10. Stop after this single task.

## Progress
- Initial execution plan recorded before repository inspection.
- Identified first incomplete task: `C4-T01A`, refreshing MIR / mir_lowered fixture snapshots affected by CaptureBox deletion.
- Current task steps: check latest commit for directly relevant unfinished notes, inspect the three fixture groups, regenerate or refresh their expected outputs without changing fixture intent, verify `closure_capture_var.hir` still records `mutable: true`, run the required fixture suite and CaptureBox grep, update `TODO.md` completion record, then commit all task changes.
- Refreshed generated `.actual.mir` / `.actual.raw.mir` for the three target fixtures and synchronized the stable `.mir` goldens. The updated snapshots now show ordinary `Int` captures and `ClosureCaptureTransportMetadata.mutable: true`, with no CaptureBox transport.
- Verified `tests/fixtures/hir/closure_capture_var.hir` still contains `mutable: true` capture metadata for both closures.
- Validation completed so far: the three refreshed target fixtures pass individually, full `cargo run -p scoop -- test` passes with `fixtures: ok (1382)`, the required CaptureBox grep over MIR fixture directories has no output, and `cargo clippy --all-targets -- -D warnings` passes.
- `TODO.md` has been updated: `C4-T01A` is marked `[DONE]`, the current status line now names `C4-T01A`, and the completion record documents changed snapshots, validation, and closure to `PLAN.md` / `CLOSURE_FIX.md`.
