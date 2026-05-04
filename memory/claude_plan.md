# Claude Execution Plan

## Scope

- Follow `TODO.md` as the global index only.
- Use the referenced `TODO-Px.md` files as the authoritative task source.
- Identify and complete exactly the first detailed task whose heading is not prefixed with `[DONE]`.
- Stop after completing and committing that single task, or after committing any required prerequisite/task-index repair if the task is blocked.

## Execution Plan

1. Read `TODO.md` to determine the ordered detailed task files and task ids.
2. Inspect the referenced `TODO-Px.md` files in index order until the first heading without `[DONE]` is found.
3. Check the latest commit message only for an explicitly unfinished issue directly relevant to the selected task.
4. Read the selected task body, constraints, dependencies, and validation requirements.
5. Implement the task as written without narrowing scope or using workarounds.
6. If a spec-correct implementation is blocked by an untracked prerequisite, add the minimum prerequisite task in the appropriate `TODO-Px.md`, sync `TODO.md`, commit, and stop.
7. Run focused tests for the change, then broader relevant validation as feasible.
8. Fix any failures introduced by the task.
9. Mark the task heading `[DONE]` in the detailed task file, update its completion record, and sync `TODO.md` if the task appears there.
10. Run final validation relevant to the task state.
11. Commit all task-related changes with a clear task-prefixed message.
12. Stop without starting the next task.

## Progress Log

- Initial plan recorded before repository inspection or command execution.
- Identified first incomplete detailed task as `P6-T03` in `TODO-P6-part2.md`.
- Latest commit is `[P6-T02qg] Publish completion payload contract`; it is a completed direct prerequisite, not a separate unfinished blocker.
- Current focus: inspect the existing refactor LLVM codegen stage and determine the smallest spec-correct implementation path for `P6-T03`.
- Existing uncommitted work already adds `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` and wires it into `llvm/emit.rs`.
- Next step: run targeted P6-T03 test/compile commands to identify remaining failures, then patch only the missing body-lowering pieces and tests required by the task.
- Fixed first compile/verification layer: refactor body functions now use fresh function-level codegen state; surface resume calls the published owner trampoline; resume-state dispatch deduplicates equivalent bindings; runtime fatal calls match the declared ABI.
- Targeted `cargo test -p scoopc refactor_llvm_` now compiles, with remaining failures limited to old stage tests that still expect P6-T01a fail-fast behavior.
- Next step: rerun real P6-T03 fixtures and address semantic/body-lowering failures before updating the tests.
- `effect_resume_if_else_branch_single_perform.scoop` now passes through the refactor run-pass fixture after fixing direct scalar arg binding and frame sync before locally consumed outward cases.
- `effect_multi_escape_indirect_direct_while.scoop` exposed a new blocker: surface-resume wrapper completion projection has only `owner_answer_ty -> wrapper_answer_ty` types, but no authoritative wrapper completion payload source when those types differ (`Unit -> Int`).
- Updated plan: add the minimum new prerequisite `P6-T02qh` before `P6-T03`, sync `TODO.md`, commit the current groundwork plus blocker record, and stop.
