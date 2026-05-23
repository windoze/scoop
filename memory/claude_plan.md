# Claude Execution Plan

## Scope

- Source of truth: `TODO.md`.
- Goal for this invocation: complete exactly the first incomplete task in `TODO.md`, then stop.
- Completion requires implementation, validation, `TODO.md` completion update, and a git commit.

## Execution Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Inspect only the code, fixtures, and documentation needed for that task, plus recent git context if it directly affects the selected task.
3. Implement the selected task without changing task scope or using workarounds.
4. Add or update the smallest relevant tests/fixtures for the task.
5. Run targeted validation first, then broader validation required by the task or affected area.
6. If any failing test/fixture is observed and is not already explicitly scheduled, fix it or add the minimum prerequisite/follow-up task in `TODO.md` before marking anything complete.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
8. Update this file whenever the plan materially changes or a key step completes.
9. Review git status/diff/log, then commit all intended changes with a task-specific message.
10. Stop after the commit without starting the next task.

## Current Progress

- Plan initialized before repository inspection.
- `TODO.md` inspected; first incomplete task is `P7-T04-c` (`迁移 physical ABI/layout 查询面到 LIR facts`).
- `TODO-6.md` task body inspected. Required scope: move physical ABI/layout and `effect_lowered/ty.rs` lookups from HIR scaffold side tables to `LirFacts.physical_layout` / type context / callable contracts; keep stage handoff shape unchanged.
- Latest commit is `[P7-T04-bR] Review LLVM stage handoff narrowing`; no separate unfinished issue was identified from the commit subject.
- Next step is targeted code inspection for scaffold fields/usages and existing LIR facts layout APIs.
- Implemented first pass of the migration: physical class/enum/vtable/itable lookups in the effect-lowered ABI materializer now use `LirFacts.physical_layout`; enum-unit/runtime-error checks use LIR enum facts; the physical ABI/layout entry now verifies the LIR TypeStore owner.
- Layout tests were adjusted so their ABI materializer helpers provide empty physical HIR side tables, proving the materializer path does not rely on those tables.
- Targeted layout tests initially exposed a real missing LIR facts contract: effect-owned builtin `Option<T>` enum layouts were not published for all effect TypeStore types. The LIR facts builder now synthesizes those `Option<T>` physical enum facts.
- `cargo test -p scoopc llvm::codegen::effect_lowered::layout` now passes (68 tests).
- `cargo test -p scoopc llvm::codegen::effect_lowered` passes (70 tests).
- `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered` passes (0 filtered-in tests for that feature set).
- `cargo test -p scoopc_lir_facts` passes.
- `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered` passes after regenerating `.effectlowered` goldens for the new LIR physical enum facts and current dump shape.
- `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` passes (421/421).
- Review found an indirect `interfaces` side-table dependency through value-box itable generation; `mir_value_box_itable_entries` now consumes LIR physical interface facts for interface metadata.
- Final validation passes: `cargo test -p scoopc llvm::codegen::effect_lowered::layout`; `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`; `cargo test -p scoopc_lir_facts`; `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`; `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
- `TODO.md` and `TODO-6.md` updated: `P7-T04-c` is marked `[DONE]` and its completion record is filled.
- Main task changes committed as `e08e2ffc` (`[P7-T04-c] Migrate physical ABI layout to LIR facts`).
- This file is being updated once more to record the completed commit step.
