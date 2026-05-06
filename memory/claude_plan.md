# Claude Execution Plan

## Current Invocation

- Read `TODO.md` to identify the first task whose title is not prefixed with `[DONE]`.
- Check the latest commit only for an explicitly mentioned unfinished issue that directly affects that first incomplete task.
- Inspect the relevant code and fixtures for that task, keeping `TODO.md` as the source of truth.
- Implement the task as written, avoiding workarounds or scope narrowing.
- If a concrete blocker prevents spec-correct implementation, add the minimum prerequisite task to `TODO.md`, document the blocker here, commit, and stop.
- Run the validation commands required by the task and any focused tests needed for the touched area.
- Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling its completion record.
- Update this file after key steps or plan changes.
- Commit all relevant changes with a task-tagged message, then stop without starting the next task.

## Progress

- Initial plan written before repository commands.
- Identified first incomplete task: `CG-T01` (raw MIR effect/control route and unsupported call kind closure).
- Latest commit `bc105756 [CG-T00R] Review codegen inventory gate` does not explicitly introduce a separate unfinished issue for `CG-T01`.
- Next step: inspect the raw MIR LLVM route gate, codegen gap inventory, and existing focused tests before editing.
- Found existing MIR-T12 routing facts in `mir/codegen_route.rs` and publication through effect facts/lowering stages, but LLVM codegen did not yet carry those facts into raw MIR body selection.
- Edit plan: thread `MirCodegenRoutingFacts` through refactor LLVM emit inputs, require `PlainRawMir` before raw MIR emission, fail fast on missing/non-raw route facts, and add focused tests named by `CG-T01`.
- Implemented routing-fact threading into LLVM codegen, raw route fact enforcement, and focused `refactor_llvm_raw_route_gate` / `raw_mir_effect_control_route` tests.
- Passed focused tests and smoke build fixtures for raw-safe LLVM emit and effect body refactor reroute; next step is full lint validation and TODO completion bookkeeping.
- `cargo clippy --all-targets -- -D warnings` passed.
- Updated `TODO.md` to mark `CG-T01` as `[DONE]` and record implementation plus validation commands.
- Next step: inspect git status/diff, then commit the completed task and stop.
