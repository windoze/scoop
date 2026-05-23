# Current Invocation Plan

## Scope

- Follow `TODO.md` as the authoritative task order.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Do not perform broad historical triage before selecting the current task.
- If a concrete blocker or unscheduled failing test/fixture prevents completion, update `TODO.md` with the minimum required prerequisite/follow-up task, commit that bookkeeping, and stop.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check recent git context only as needed for that selected task, including whether the latest commit explicitly references an unfinished issue relevant to it.
3. Inspect the code, fixtures, and documentation directly relevant to the selected task.
4. Implement the task with the smallest spec-correct change; avoid workarounds or narrowed fixture shapes.
5. Run targeted validation first, then broader required validation for the task. Address any observed unscheduled failures according to the failure policy.
6. Update `TODO.md` by prefixing the task title with `[DONE]` and recording the completion details, or add a prerequisite/follow-up task if completion is blocked.
7. Update this plan file after key progress points or any plan change.
8. Inspect `git status`, `git diff`, and recent commits, then commit all intended changes with a task-specific message.
9. Stop without starting the next task.

## Execution Progress

- Initial plan file created before command execution.
- Selected first incomplete task: `P8-T01` in `TODO-6.md` (`最终 residual 搜索、文档冻结与未来 C backend 输入边界`).
- Latest commit `3e935453 [P7-T05R] Review P7 backend cleanup` directly supports this task by confirming P7 cleanup and leaving P8 as final residual/documentation freeze; no separate prerequisite was identified from the commit subject.

## Current Task Execution Plan

1. Read P8-T01 requirements and relevant P7 completion context.
2. Run targeted residual searches for old comptime/const surface, devirtualization ownership drift, stage-output nesting, upstream-wrapper handoff residuals, HIR/raw MIR/effect facts LLVM coupling, and duplicate fact-table fallback patterns.
3. Inspect dependency gate and documentation to see which final constraints are automated and where the future C backend boundary is currently described.
4. Make the smallest necessary source/documentation/dependency-gate changes so the final boundary is explicit and regressions are checked mechanically where feasible.
5. Run required validation for P8-T01: `cargo fmt`, `cargo run -p scoop_tools -- dependency-gate`, `cargo run -p scoop_tools -- spec-fixtures check`, `cargo test --all --all-targets`, and `git diff --check`.
6. Update `TODO.md` and `TODO-6.md` with `[DONE]` status and completion records if validation passes.
7. Commit the completed task and stop.

## Residual Findings

- Active source still tokenized `comptime` as a dedicated keyword (`Keyword::Comptime`) and had a parser test explicitly expecting that keyword. This is a P0/P8 residual because old comptime surface should fail naturally as ordinary parse/resolve/typecheck behavior, not by preserving a dedicated surface token.
- LLVM still contained `codegen/main/const_eval.rs` with `const_eval_*` helpers and an unused `const_initializer_for_top_level_var` helper. The P0/P8 cleanup guidance allows backend data initializers when needed, but this helper is currently dead and keeps old const-evaluator naming alive.

## Plan Update

- Remove the active `Comptime` keyword and replace the parser/lexer tests with coverage that `comptime` is now a normal identifier.
- Delete the dead LLVM `const_eval` module and unused top-level-var constant initializer helper instead of renaming unused code.
- Extend `dependency-gate` with final P8 source-boundary rules to prevent reintroducing `Comptime` keyword tokens or LLVM `const_eval` helpers.

## Implementation Progress

- Removed `Keyword::Comptime`, the lexer keyword mapping, parser recovery/display references, and updated parser/lexer regressions so `comptime` is a normal identifier.
- Deleted the unused LLVM `main/const_eval.rs` module and unused top-level var const initializer helper; updated the failure-policy audit baseline accordingly.
- Added dependency-gate source boundary checks for the old comptime keyword surface and LLVM const-eval residuals.
- Updated active docs and LIR type-context wire owner from the stale `P8/per-cone` wording to `P10 per-cone build artifact serialization`, with effect-lowered goldens synchronized.
- Targeted checks passed: parser unit tests, lexer regression, parse fixtures, effect-lowered fixtures, and dependency gate.
- Required `cargo test --all --all-targets` exposed `fixtures::tests::run_all_recreates_session_between_independent_fixtures` failing on `mir_lowered/aggregate_transport.scoop` MIR golden drift. This must be fixed or explicitly scheduled before P8-T01 can be marked complete.
- Reproduced the drift on the standalone `mir_lowered/aggregate_transport.scoop` fixture and regenerated its MIR golden; the standalone fixture now passes, and the original unit test passed before the filtered cargo invocation exceeded the short timeout while entering unrelated integration test binaries.
- Re-running `cargo test --all --all-targets` passed the earlier MIR fixture test but exceeded the 20 minute tool timeout while entering `scoop_runtime` integration test `gc_immix_write_barrier`; this must be investigated under the single-test timeout policy.
- `gc_immix_write_barrier` passed standalone, confirming the prior timeout was total-suite timeout rather than a stuck single test. A longer full-suite run then exposed stale `scoopc` baselines: LIR tests used the wrong TypeStore owner, native extern declarations missed the gc-leaf attribute on LIR-declared symbols, an LLVM stage test expected the old callable key shape, and the failure-policy sentinel list was stale. These have been updated and the targeted tests now pass.
- Final validation passed: `cargo fmt`, `cargo run -p scoop_tools -- dependency-gate`, `cargo run -p scoop_tools -- spec-fixtures check`, `cargo test --all --all-targets` with extended timeout, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
- `TODO.md` and `TODO-6.md` have been updated to mark `P8-T01` as `[DONE]` with a completion record. `PLAN.md` was not changed because no phase-level sequencing or dependency changed.
