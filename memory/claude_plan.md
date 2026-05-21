## Execution Plan

I will execute exactly one task from `TODO.md`, using `TODO.md` as the source of truth.

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for directly relevant unfinished work tied to that task.
3. Read the task details, dependencies, validation requirements, and completion record.
4. Inspect only the files needed to understand and implement that task.
5. Implement the required change without workarounds or scope narrowing.
6. Run targeted validation for the changed behavior, then broader validation required by the task.
7. If a concrete blocker prevents correct implementation, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
8. If the task is completed, update `TODO.md` by prefixing the task heading with `[DONE]` and refreshing its completion record.
9. Update this plan file after key milestones or if the execution plan changes.
10. Inspect git status/diff/log, commit all intended changes for this task with a clear task-tagged message, and stop without starting the next task.

Note: This file records an auditable plan and progress log, not private chain-of-thought.

## Progress Log

- Started invocation and wrote the initial execution plan before running repository inspection commands.
- Identified `P2-T03` as the first incomplete task in `TODO.md`; latest commit is `P2-T02R` and does not advertise unfinished work for this task.
- Read `P2-T03` details: remove MIR materialization/pass artifacts from `LoweredHir`, move request-root materialization ownership to MIR stage/pipeline, keep production codegen using post-MIR handoff, and validate with HIR/MIR/no-default-features/run-pass/clippy checks.
- Implementation direction chosen: `LoweredHir` now remains HIR-only; frontend/codegen handoff carries canonical `MaterializedMir` separately, and MIR stage output receives that snapshot explicitly before effect facts/lowering.
- Completed implementation and validation: removed HIR-owned MIR artifacts/accessors, added explicit codegen lowering handoff, added run-pass fixture coverage, and passed the required HIR/MIR/no-default/run-pass/clippy checks.
