# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop after committing the result.

## Execution Plan

1. Read `TODO.md` to identify the first incomplete task and its requirements.
2. Inspect the latest commit only for unfinished work directly relevant to that task.
3. Read the relevant code, fixtures, and documentation needed for the selected task.
4. Implement the smallest spec-correct change that fully satisfies the task.
5. Add or update targeted tests/fixtures required by the task.
6. Run targeted validation first, then broader required validation if practical.
7. If any unscheduled failing test or fixture is observed, fix it or schedule the minimum prerequisite task before marking the current task done.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in its completion record.
9. Update this file after key progress points or plan changes.
10. Review `git status`, `git diff`, and recent commits, then commit all relevant changes with a descriptive task-tagged message.

## Current Status

- Selected first incomplete task: `P7-T04-b-4` from `TODO-6.md`.
- Latest commit `a0902e9c [P7-T04-b-3R] Review ClassInstanceKey keying` does not indicate an unfinished issue that changes the selected task.
- Implemented the core migration shape: `CgTy` aggregate variants now carry `MonoTypeId`, `cg_ty_of` is infallible over `MonoTypeId`, `expect_cg_ty_of` and the `monomorph miss` warning path are removed, and `codegen_type_store_for_type_id` has been deleted.
- Layout side tables for struct and enum fields now store `Option<MonoTypeId>` at the codegen boundary; LIR/HIR fact emission converts back to raw `TypeId` only for existing fact wire contracts.
- Fixed the observed `parameterized_supertype_interface_dispatch.scoop` regression by making erased interface reference dispatch use the erased `Any` receiver ABI instead of the generic interface template receiver `TypeId`.
- Validation completed: `cargo fmt`; `cargo test -p scoopc_types`; `cargo test -p scoopc --no-default-features hir`; `cargo test -p scoopc --no-default-features mir`; `cargo test -p scoopc --no-default-features llvm::codegen`; `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`; `cargo test -p scoopc llvm::codegen`; `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`; `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
- `TODO.md` and `TODO-6.md` now mark `P7-T04-b-4` as `[DONE]` with completion records.
- Next step: final diff/status checks and commit the completed task.
