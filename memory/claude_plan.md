# Execution Plan

## Scope
- Work on exactly the first incomplete task in `TODO.md`.
- Treat a task as complete only when its heading is prefixed with `[DONE]`.
- Do not advance to the next task after finishing the selected task.

## Steps
1. Read `TODO.md` and identify the first heading without `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and completion-record expectations.
4. Inspect only the relevant code, fixtures, tests, and docs needed for that task.
5. Implement the smallest spec-correct change that fully completes the task.
6. Add or update targeted tests/fixtures required by the task.
7. Run the specified validation commands and relevant focused tests.
8. If any unscheduled failing test or fixture is observed, fix it or add the minimum prerequisite task in `TODO.md` before marking the current task complete.
9. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in its completion record.
10. Update this plan file whenever a key step completes or the plan changes.
11. Inspect git status and diff, then commit all intended changes with a task-tagged message.
12. Stop after the commit.

## Current Status
- `TODO.md` read and first incomplete task identified as `P7-T05-c` in `TODO-6.md`.
- Latest commit subject is `Update plan`; it does not explicitly name an unfinished issue relevant to `P7-T05-c`.
- Worktree initially has only this plan file modified.
- Initial residual search confirms the task-listed production hits still exist: `fun_index`, `HirFacts`, `callable_signatures`, `LlvmCallableSignatureContract`, full `MaterializedMir` / `MaterializedEffectFacts` wrapper fields, and HIR dispatch side-table inputs under `pipeline/llvm_codegen_stage.rs`, `llvm/emit.rs`, and `llvm/codegen/*`.
- Implemented the main residual cleanup: LLVM codegen context no longer receives/saves `fun_index`, `HirFacts`, HIR-derived callable signature maps, or full MIR/effect wrappers; ordinary callee analysis now consumes LIR callable facts plus a narrow `EffectAnalysisFacts` query object; dispatch source lookup is represented as an explicit `LlvmDispatchCallKey -> LirCallSiteKind` narrow contract.
- Quick checks passed: `cargo fmt`; `cargo test -p scoopc --no-default-features llvm_codegen_stage`; `cargo test -p scoopc --no-default-features llvm::codegen`.
- `dependency_gate` extended for the P7-T05-c residual classes and now passes.
- Full run-pass initially exposed missing callable signature ownership after removing the LLVM HIR fallback. Fixed by publishing materialized callable source signatures and MIR direct/dispatch call-site helper signatures into `LirFacts.source_signatures`.
- Required validation now passes: `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo run -p scoop_tools -- dependency-gate`; `cargo test -p scoopc_lir_facts`; `cargo test -p scoopc --no-default-features llvm_codegen_stage`; `cargo test -p scoopc --no-default-features llvm::codegen`; `cargo test -p scoopc llvm::codegen`; `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`; `git diff --check`.
- `TODO.md` and `TODO-6.md` now mark `P7-T05-c` as `[DONE]` with the completion record filled in.
- Next step: inspect git status/diff/log, then commit the intended changes.
