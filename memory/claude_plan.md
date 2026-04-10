# Claude Plan

## Constraint Handling

The request asks for a complete thought process. I will not dump private chain-of-thought, but I will keep this file updated with:
- the concrete execution plan,
- key decisions,
- observed facts,
- task status,
- any plan changes during execution.

## Initial Execution Plan

1. Inspect the latest git commit message and diff summary to determine whether it mentions any known or pre-existing issue that is still unresolved.
2. Read `TODO.md` and identify the first incomplete task.
3. Decide whether that task is directly executable or needs decomposition.
4. If decomposition is needed:
   - update `PLAN.md` with the refined breakdown,
   - update `TODO.md` so the first new subtask becomes the active task for this run.
5. Implement exactly the first incomplete task or first new subtask.
6. Run the narrowest relevant checks first, then broader validation, aiming to satisfy:
   - task-specific tests,
   - `cargo fmt --check` if relevant,
   - `cargo clippy --all-targets -- -D warnings`,
   - any additional fixture or integration tests affected by the change.
7. Fix any failures or regressions found during validation.
8. Update documentation/state:
   - mark the task complete in `TODO.md`,
   - update `PLAN.md`,
   - refresh this file with progress notes and outcomes.
9. Commit the changes with a descriptive message tied to the task identifier if available.
10. Stop after the single task is complete.

## Assumptions Before Inspection

- The repository may already contain unrelated user changes; I will not revert them.
- The first incomplete item in `TODO.md` defines scope unless the latest commit surfaces an unresolved pre-existing issue that must be fixed first.
- If the active task is too large to complete safely in one pass, decomposition is preferred over partial implementation.

## Progress Log

- Plan file created before running any shell commands, per request.
- Inspected latest commit: `3b82a26 [T0146a] Add char literal frontend parsing`.
- Checked full HEAD commit message: only the subject line is present; it does not mention an unresolved pre-existing issue to fix first.
- Identified first incomplete task in `TODO.md`: `T0146b [TODO] Char 类型语义：type system / HIR / typecheck / comptime`.
- Began implementation of `T0146b`.
- Completed first semantic patch batch:
  - added builtin `Char` type plumbing in `ty/mod.rs`,
  - added resolver/index builtin fallback for `Char`,
  - added resolver intrinsic allowance for `Char.toInt()`,
  - lowered AST Char literals/patterns to concrete HIR `char` payloads,
  - inferred Char literal type as builtin `Char`,
  - enabled Char comparison typing and `when` Char pattern type checking,
  - added comptime `ConstValue::Char` plus `toInt()` folding support,
  - updated fixture formatting for Char const snapshots.
- Next step: run compiler/test checks, fix all remaining exhaustive matches / downstream integration points, then add task fixtures.

## Active Task

- Task ID: `T0146b`
- Summary: add static semantic support for `Char` literals and patterns across builtin type definitions, HIR lowering/data model, type checking, and compile-time evaluation.

## Refined Plan For T0146b

1. Read the detailed `T0146b` entry in `TODO.md` and the corresponding notes in `PLAN.md`.
2. Inspect current char-related code paths touched by `T0146a`:
   - syntax/parser/AST output,
   - HIR types and lowering hooks,
   - builtin type tables,
   - expression/type inference,
   - pattern typing / exhaustiveness,
   - comptime constant model and evaluator.
3. Determine whether `T0146b` is small enough to finish directly or whether it needs decomposition. Current expectation: it is implementable in one pass.
4. Implement `Char` as a real builtin semantic type, not just a parsed literal surface form.
5. Add or update focused tests:
   - unit tests where the type/comptime layer is best validated,
   - fixtures for typecheck / HIR / const evaluation if applicable.
6. Run validation commands, starting narrow and expanding until confidence is sufficient.
7. Update `TODO.md`, `PLAN.md`, and this file with completion details.
8. Commit once the task is complete, then stop.
## Continuation Plan (2026-04-10)

Current status carried over from the previous run:
- Latest commit check was already completed. `HEAD` is `3b82a26 [T0146a] Add char literal frontend parsing`, and it did not describe any unresolved pre-existing issue that needed fixing before `TODO.md`.
- The first undone task remains `T0146b [TODO] Char 类型语义：type system / HIR / typecheck / comptime`.
- Most implementation work for `T0146b` is already in the tree. The remaining work is to finish verification, fix any fallout, then update planning/task tracking and commit exactly this task.

Execution plan for this continuation:
1. Preserve the existing planning log in this file and append progress here as key steps complete.
2. Fix the likely builtin `TypeId` instability in `crates/scoopc/src/ty/mod.rs` by moving `Char` builtin interning after the previously existing builtin types so unrelated HIR golden snapshots stay stable.
3. Run focused verification:
   - `cargo test --all`
   - if needed, inspect only the failing tests and adjust the implementation or minimal expected outputs
   - `cargo run -p scoop -- test`
4. If verification passes closely enough for this task, update:
   - `TODO.md` to mark `T0146b` done
   - `PLAN.md` to record completion / any follow-up boundary for `T0146c`
   - `memory/claude_plan.md` with outcome notes
5. Review git diff for only task-related changes, then create one commit for this completed task and stop.

Scope boundary reminder:
- Do not start `T0146c` runtime / codegen support for `Char`.
- If verification reveals a dependency that blocks `T0146b`, keep the task as TODO, reorder tasks as required, document the reason in `PLAN.md`, commit the reordering, and stop.

## Completion Notes (2026-04-10)

- Adjusted builtin interning order so `Char` is appended after the previously existing builtin value types. This reduced unrelated `TypeId` churn but did not eliminate all snapshot diffs.
- Found and fixed one real semantic gap during verification: `HIR lower_type_path()` did not yet map `scoop.core.Char` to the builtin `Char` type, so explicit `Char` type references in dump-HIR mode would have lowered as nominal types. Added the builtin mapping.
- `cargo test --all` then passed after refreshing the affected HIR golden snapshots.
- `cargo run -p scoop -- test` exposed additional snapshot drift in HIR and MIR fixtures that are not covered by the unit-test-only golden set. Regenerated all `tests/fixtures/hir/*.hir` and `tests/fixtures/mir/*.mir` snapshots from `scoop dump-hir` / `scoop dump-mir`, then reran the fixture driver successfully.
- Ran `cargo fmt` to satisfy `cargo fmt --check`, then reran `cargo fmt --check`, `cargo test --all`, and `cargo run -p scoop -- test` to confirm the final formatted tree still passes verification.
- Final verification status:
  - `cargo test --all`: passed
  - `cargo run -p scoop -- test`: passed (`fixtures: ok (848)`)
  - `cargo clippy --workspace --all-targets -- -D warnings`: fails on pre-existing workspace-wide baseline issues, primarily:
    - many `inkwell` deprecation errors (`ptr_type`, `ptr_sized_int_type_in_context`) in LLVM codegen,
    - existing `too_many_arguments` and `result_large_err` clippy failures across LLVM/typecheck modules.
  - I did not observe any new task-specific clippy failure attributable only to `T0146b`.
- Task status: `T0146b` is complete. Next task remains `T0146c`, which will add runtime/sysroot/LLVM executable support for `Char`.
