# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after completing and committing that one task, or after committing any required prerequisite/blocker bookkeeping.

## Execution Plan

1. Read `TODO.md` first and determine the first incomplete task by heading prefix.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Read the minimum relevant files needed to understand and implement the task.
4. Implement the task without workarounds or scope narrowing.
5. Add or update tests/fixtures required by the task.
6. Run the task's required validation commands and any focused tests needed for confidence.
7. If validation exposes a task-blocking spec mismatch or missing feature, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
8. If the task is complete, update `TODO.md` by prefixing the task heading with `[DONE]` and filling the completion record.
9. Update this plan file when key steps complete or the plan changes.
10. Commit all relevant changes with a descriptive task-tagged message.
11. Stop without starting the next task.

## Progress Log

- Initialized execution plan before reading project task files.
- Read `TODO.md`; first incomplete task is `HIR-T07`: publish callable callee provenance and dispatch/ctor/intrinsic HIR contracts.
- Checked latest commit (`[HIR-T06] Canonicalize refactor HIR call arguments`) and current worktree; no directly relevant unfinished issue was found.
- Inspected HIR lowering, typed HIR stage contracts, and MIR lowering facts. Current gap: typed HIR only exposes coarse call kinds, while refactor MIR dispatch still reconstructs owner/member from callee FQN. Plan update: add structured call-site contracts to typed HIR and route refactor MIR dispatch through those contracts.
- Implemented structured typed HIR call-site contracts for direct top-level, member direct, extension, constructor, closure, function value, FunPtr, virtual/interface dispatch, intrinsic, continuation resume, and effect-op sites. Refactor MIR dispatch now consumes the typed HIR owner/member contract instead of reconstructing dispatch metadata from callee FQN.
- Added `refactor_hir_call_contracts_surface_ok.scoop` and a focused unit test covering the HIR-T07 call surfaces. Verified focused contract tests, typed HIR snapshots, no-Todo tests, dump-hir tests, dispatch/resume tests, the new fixture, and refactor `dump-hir` on the fixture.
- Ran `cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings` successfully.
- Updated `TODO.md` by marking `HIR-T07` as `[DONE]` and recording implementation details plus validation commands. `PLAN.md` did not need changes because phase-level sequencing was unchanged.
