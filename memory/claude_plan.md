# Claude Execution Plan

Date: 2026-05-06

This file records the operational plan and progress for the current invocation. It summarizes intended actions and decisions without exposing private chain-of-thought.

## Scope

- Use `TODO.md` as the authoritative task list.
- Complete exactly the first incomplete task whose heading is not prefixed with `[DONE]`.
- Do not proceed to later tasks after completing or blocking the current task.
- Avoid workarounds, fixture-only hacks, or spec deviations.
- Update `TODO.md` completion records and task heading when the task is complete.
- Update `PLAN.md` only if the phase-level plan or dependencies change.
- Commit the completed task or blocker/task-list update before stopping.

## Initial Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit message only for directly relevant unfinished work tied to that task.
3. Inspect the minimum necessary code, fixtures, and docs for the selected task.
4. Implement the task as written, using the smallest correct code changes.
5. Add or update tests/fixtures required by the task.
6. Run the task-specified validation commands and any relevant focused tests.
7. Fix any failures that are in scope for the current task.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record.
9. Update this file with key progress and any plan changes.
10. Commit all relevant changes with a task-tagged message.
11. Stop after the commit.

## Progress Log

- Plan file initialized before reading project task files or running commands.
- Identified first incomplete task: `HIR-T03` (`收口 comptime block/if/for 与 package-level comptime if`).
- Next step: inspect only directly relevant recent commit context and current comptime/HIR implementation before editing.
- Relevant context found: latest commit is `[HIR-T02] Gate parser-only HIR surfaces`; no unfinished directly relevant note.
- Existing package-level `comptime if` trim already runs before resolve/typecheck in HIR dump paths.
- Current HIR-T03 gap: statement-level `ComptimeBlock`, `ComptimeIf`, and `ComptimeFor` are still lowered to `StmtKind::Todo(...)` in `hir/lower/stmt.rs`.
- Implementation approach: add a comptime expansion pass that consumes the typed/resolved AST after typecheck and before refactor typed HIR lowering, then lower only the expanded ordinary statements.
- Implemented draft code for `RuntimeComptimePlan` plus refactor HIR lowering consumption: selected `comptime if` branches are inlined, `comptime for` bodies are unrolled, and primitive compile-time loop binders can lower to HIR literals.
- Next step: compile focused tests, fix type/API issues, then add HIR-T03 regression tests.
- Added `refactor_hir_comptime_expands_block_if_for_and_package_if` and `tests/fixtures/hir/refactor_comptime_control_flow.{scoop,hir}`.
- Validation run so far: `cargo test -p scoopc --no-default-features refactor_hir_comptime`, new HIR fixture, `refactor_hir_no_todo`, `cargo test -p scoop --no-default-features dump_hir`, and clippy all pass.
- The task-listed `dump-hir tests/fixtures/comptime/splice_field_access_v0_basic.scoop` command still fails before HIR with existing `scoop::typecheck::missing_type_annotation` for top-level `P`; this is outside the comptime placeholder eliminated by HIR-T03 and will be recorded in `TODO.md`.
- Marked `HIR-T03` as `[DONE]` in `TODO.md` with completion record and validation notes.
- Re-ran focused `refactor_hir_comptime`, the new HIR fixture, and clippy after formatting; all passed.
- Next step: inspect git diff/status and create the HIR-T03 commit.
