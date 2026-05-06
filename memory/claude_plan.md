# Claude Execution Plan

## Current Invocation

- I will record an auditable execution plan here instead of private internal chain-of-thought.
- Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
- Check the latest commit only for an explicitly mentioned unfinished issue that is directly relevant to that task.
- Inspect only the task-relevant code, fixtures, and tests needed to implement or validate that task.
- Implement the first incomplete task exactly as specified, without narrowing scope or using fixture-only workarounds.
- If a spec-correct implementation is blocked by a concrete missing feature or bug, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
- Run the task-required validation plus focused tests for the touched area; fix any regressions caused by this work.
- Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record.
- Update this file after key progress points or plan changes.
- Commit all relevant changes with a task-tagged message, then stop without starting the next task.

## Progress

- Initial auditable plan written before running repository commands.
- Identified first incomplete task: `CG-T01R` (`Review CG-T01 raw MIR route gate`).
- Next step: check the latest commit for directly relevant unfinished notes, then review the CG-T01 implementation and run its required validation commands.
- Latest commit is `[CG-T01] Gate raw MIR codegen routes` with no unfinished note.
- Reviewed the CG-T01 gate path: refactor emit threads MIR-T12 routing facts into LLVM codegen, `codegen_top_level_mir_fun` invokes `raw_mir_backend_gate_failure`, and the gate rejects missing/non-`PlainRawMir` route facts plus `PerformResult`, `Handle`, `ResumeUnwind`, cleanup `Perform`, and non-raw call kinds before raw body emission.
- Next step: run the CG-T01 validation commands and search checks required by `CG-T01R`.
- Validation passed: `cargo test -p scoopc refactor_llvm_raw_route_gate`, `cargo test -p scoopc raw_mir_effect_control_route`, `cargo test -p scoopc codegen_gap_inventory`, `cargo test -p scoopc refactor_llvm_backend_gate`, `cargo test -p scoopc refactor_mir_codegen_routing_contract`, the two build fixtures from the CG-T01 completion record, and `cargo clippy --all-targets -- -D warnings`.
- Search review found direct effect/control mentions only in raw support rejection/gate paths, guarded generic MIR emission, refactor-specific fail-fast branches, and use-collection helpers; no handoff bypass was found.
- Marked `CG-T01R` `[DONE]` in `TODO.md` with review conclusion and validation record.
- Next step: commit the review bookkeeping and stop.
