# Claude Execution Plan

## Scope

- Follow the repository task workflow exactly: read `TODO.md` as the index, then inspect the referenced detailed `TODO-Px.md` files in order.
- Select the first detailed task whose heading is not prefixed with `[DONE]`.
- Complete exactly one detailed task in this invocation, then stop after committing.

## Execution Plan

1. Read `TODO.md` and the relevant `TODO-Px.md` files to identify the first incomplete detailed task.
2. Check the latest commit only for directly relevant unfinished work that affects the selected task.
3. Inspect the code and tests needed for that task, avoiding unrelated issue triage.
4. Implement the smallest spec-correct change that fully satisfies the task.
5. Add or update tests/fixtures required by the task.
6. Run focused verification first, then broader verification appropriate to the task.
7. Update the detailed TODO file by prefixing the completed task heading with `[DONE]` and adding a completion record.
8. Sync `TODO.md` with the same `[DONE]` marker if the completed task appears there.
9. Update this file after key milestones or if the plan changes.
10. Commit all relevant changes with a clear task-tagged commit message.

## Current Progress

- Initial execution plan written before running repository commands.
- Read `TODO.md` and `TODO-P6-part3.md`.
- Selected first incomplete detailed task: `P6-T03i` in `TODO-P6-part3.md`.
- Latest commit is `[P6-T03h] Close continuation protocol lowering`, which is the direct completed dependency for `P6-T03i`; no new prerequisite has been identified.
- Next step: inspect refactor LLVM runtime-error/body lowering and existing tests, then implement verifier/diagnostic cleanup for `P6-T03i`.
- Inspection found the concrete P6-T03i gap: `LocalRuntimeError` still calls `scoop_runtime_error_fatal` with `null_payload`, and call-boundary runtime-error consumption is not yet emitted from the published local runtime-error contract.
- Implementation direction: add a body verifier before state emission, route consumed runtime-error cases through the ABI query contract, materialize the extracted payload into the fatal runtime entry, and add regression tests/search guards.
- Implemented the initial body verifier, local runtime-error payload materialization, call-boundary consumed runtime-error dispatch, and search guards.
- During fixture verification, found and fixed a directly blocking handle-completion payload copy bug where boundary completion could branch to the handle return state without copying the boundary result into the published return payload local.
- During `effect_multi_escape_indirect_direct_while.scoop` verification, found and fixed a directly blocking double-resume runtime-error selection gap for callables whose `Step` schema publishes a runtime-error case without a paired runtime-error boundary.
- Verification completed and rerun for the `P6-T03i` matrix, including targeted tests, both run-pass fixtures, grep guard, and `cargo clippy --all-targets -- -D warnings`.
- Marked `P6-T03i` as `[DONE]` in `TODO-P6-part3.md` and synchronized `TODO.md`.
- Next step: inspect git diff/status, then commit the completed task changes and stop.
