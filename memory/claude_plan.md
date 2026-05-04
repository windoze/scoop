# Claude Plan

## Current Invocation Plan

1. Read `TODO.md` as the task index.
2. Inspect referenced `TODO-Px.md` files in order until the first detailed task whose heading is not prefixed with `[DONE]` is found.
3. Read the selected task body, constraints, dependencies, and validation requirements.
4. Check recent git context only as needed to identify directly relevant unfinished work for the selected task.
5. Implement the selected task exactly as specified, without narrowing scope or using workarounds.
6. Run targeted validation first, then broader required checks if feasible for the task.
7. Update the detailed `TODO-Px.md` completion record and prefix the completed task heading with `[DONE]`.
8. Sync `TODO.md` with the same `[DONE]` marker or any task ordering/title changes.
9. Update this file when key steps complete or if the plan changes.
10. Commit all relevant changes with a descriptive task-tagged commit message, then stop.

## Status

- Initial plan written.
- Identified first incomplete detailed task: `P6-T03` in `TODO-P6-part2.md`.
- Task scope: complete refactor LLVM body lowering from the P5 state graph and published boundary contracts, then validate and mark only `P6-T03` complete.
- Inspected current refactor LLVM codegen entry, ABI/query helpers, and P5 dumps for `P6-T03` fixtures.
- Blocker found: P5/P6 handoff still lacks an authoritative completion payload source for non-`Unit` returns. Example: `effect_resume_if_else_branch_single_perform.scoop` publishes `run(): Int` with `Complete(t5)`, but the relevant state terminator is still `Return(Unit -> st1)`. P6 would have to inspect raw MIR/tail shape to recover the value, which is forbidden by `P6-T03`.
- Plan change completed: added prerequisite `P6-T02qg` before `P6-T03` in `TODO-P6-part2.md`, synced `TODO.md`, kept `P6-T03` incomplete, and recorded the blocker in `P6-T03` completion notes.
- Validation run: `cargo check -p scoopc`.
- Next step: commit the task-graph update and stop.
