# Claude Plan

## Execution Policy

- I will follow `TODO.md` only as the task index and use the corresponding `TODO-Px.md` file as the source of truth.
- I will select exactly the first detailed task whose heading is not prefixed with `[DONE]`.
- I will complete only that one task in this invocation, then stop after committing.
- I will not use workarounds for spec mismatches; if a concrete prerequisite blocks the selected task, I will add the minimum prerequisite task in the correct detailed TODO file, sync `TODO.md`, commit, and stop.
- I will update this file whenever the selected task, plan, key progress, blocker, validation result, or completion state changes.

## Initial Step-By-Step Plan

1. Read `TODO.md` as the index.
2. Inspect referenced `TODO-Px.md` files in index order to find the first detailed task without `[DONE]` in its heading.
3. Check the latest commit message only for unfinished work directly relevant to that selected task.
4. Read the selected task body, constraints, dependencies, and validation requirements.
5. Inspect the relevant implementation and tests for that task.
6. Implement the smallest spec-correct change needed for the selected task.
7. Add or update focused tests/fixtures required by the task.
8. Run the task-specific validation commands, then broader relevant checks if feasible.
9. If validation fails, fix the cause and rerun the relevant checks.
10. Mark the task `[DONE]` in the authoritative `TODO-Px.md` file, update its completion record, and sync `TODO.md` if needed.
11. Commit all changes for this invocation with a clear task-tagged message.
12. Stop without starting the next task.

## Current Status

- Selected task: `P6-T03e` in `TODO-P6-part3.md` - close direct/dynamic/virtual/interface call lowering without legacy callable wrappers.
- Source of truth: `TODO-P6-part3.md` lines 247-273. Dependencies through `P6-T03d` are marked `[DONE]` in both the detailed file and `TODO.md`.
- Latest commit checked: `913f4b52 [P6-T03d] Record completion status`; it does not identify an unfinished blocker for `P6-T03e`.
- Implementation direction: define published callable carrier shells and lower dynamic invoke sites by consuming `RefactorDynamicInvokeLayout` plus carrier/args ABI, not legacy callable wrappers.
- Implementation status: code changes and fixture updates are complete for `P6-T03e`.
- Validation status: passed `cargo test -p scoopc refactor_llvm_call_lowering`, `cargo test -p scoopc refactor_llvm_dynamic_invoke_lowering`, `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`, `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`, and `cargo clippy --all-targets -- -D warnings`.
- TODO status: `P6-T03e` is marked `[DONE]` in `TODO-P6-part3.md`, and `TODO.md` is synchronized.
- Next step: commit all changes for this invocation and stop.

## Current Edit Plan

1. Check latest commit for direct blockers relevant to `P6-T03e`.
2. Inspect refactor LLVM call lowering, callable ABI/query publication, dynamic invoke tests, and any existing fail-fast diagnostics.
3. Extend dispatch carrier layouts with published slot metadata needed by body lowering.
4. Define closure/vtable/itable carrier shell bodies that bridge receiver + explicit args into the callable direct-entry args tuple.
5. Lower boundary dynamic calls and source-slice non-boundary dynamic calls through the same published dynamic invoke layout; dispatch boundary `Step_F` or extract no-outward `Complete` for pure source-slice calls.
6. Add or update targeted tests for `refactor_llvm_call_lowering` and `refactor_llvm_dynamic_invoke_lowering`.
7. Run the required validations: `cargo test -p scoopc refactor_llvm_call_lowering`, `cargo test -p scoopc refactor_llvm_dynamic_invoke_lowering`, the specified refactor fixture command, and `cargo clippy --all-targets -- -D warnings` if feasible.
8. Mark `P6-T03e` `[DONE]`, update its completion record, sync `TODO.md`, commit, and stop.
