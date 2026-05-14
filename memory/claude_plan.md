# Claude Plan

## Initial plan

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Inspect the latest commit message for any directly relevant unfinished work tied to that task.
3. Read the task entry carefully, then inspect only the code and tests needed for that task.
4. Implement the task fully, or if a concrete blocker exists, add the minimum prerequisite task to `TODO.md` before proceeding.
5. Run the task-required validation plus relevant targeted tests, then broader required checks if the task lands.
6. Update `TODO.md` completion records and task title with `[DONE]` only after implementation and validation are complete.
7. Update this file as key steps finish or if the plan changes.
8. Commit all required changes with a task-specific commit message, then stop.

## Notes

- I will keep this file updated with progress and any blockers.
- I will not use workaround implementations that diverge from the task or spec.

## Progress update

- Identified the first incomplete task in `TODO.md` as `P3-T02`.
- The latest commit is `[P3-T01] Add scoop ABI extern metadata`, which matches the completed prerequisite task and does not itself record extra unfinished work beyond `P3-T02`.
- The working tree already contains uncommitted changes directly related to `P3-T02` in codegen, typecheck, LLVM tests, and new managed ABI fixtures. I will treat these as in-scope resumed task state and verify whether they fully implement the task or reveal a concrete blocker.
- Next step: inspect the diff and surrounding code for the touched files, then either finish the implementation and validations or record a prerequisite blocker in `TODO.md`.

## Resumed execution plan

1. Re-read `PROMPT.md` and `TODO.md` to confirm `P3-T02` remains the first incomplete task and that all currently dirty files are in-scope resumed task state.
2. Inspect the touched typecheck and codegen paths to separate already-landed `P3-T02` work from the remaining gap.
3. Finish the missing front-end contract for `@Extern` ABI-specific `@Unsafe/@NoGC` behavior if it is not yet enforced.
4. Verify that `ExternAbi::Scoop` declaration and call lowering are already wired through direct/HIR/MIR paths; if any concrete path is still missing, implement it instead of narrowing scope.
5. Add or update targeted fixtures and LLVM tests for the finalized ABI-specific contract.
6. Run the task-required validation set plus any focused tests needed to confirm the resumed worktree now fully satisfies `P3-T02`.
7. Only after implementation and validation succeed, update `TODO.md` / `memory/claude_plan.md` completion records for `P3-T02`.

## Current findings

- The current codegen and LLVM tests already contain `ExternAbi::Scoop` declaration/direct-call/hidden-sret paths, suggesting most lowering work is present in the resumed worktree.
- The current unsafe and `@NoGC` call gates already distinguish native `@Extern` from `abi = "scoop"` by declaration-site ABI.
- The likely remaining implementation gap is the explicit front-end rejection of `@Extern` declarations that redundantly or invalidly stack `@Unsafe` / `@NoGC`, which is part of the finalized `P3-T02` contract.

## Completion update

- Added explicit front-end diagnostics for `@Extern` stacked with `@Unsafe` / `@NoGC`: C ABI declarations now reject them as redundant, and Scoop ABI declarations reject them as unsupported.
- Verified the resumed lowering worktree already routes managed extern declaration/direct-call/aggregate-return through ordinary managed ABI rather than the native leaf scaffold, and kept that behavior locked with LLVM/build/run-pass regressions.
- Added targeted typecheck fixtures for the ABI-specific modifier contract and re-ran the existing `abi = "scoop"` `@NoGC` gate fixture.
- Validation completed successfully:
  - `cargo fmt --all`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_default_abi_unsafe_redundant_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_c_abi_nogc_redundant_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_unsafe_not_supported_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_nogc_not_supported_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_nogc_call_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/managed_abi_aggregate_return_hidden_sret.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/managed_abi_direct_call_ordinary_contract.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/managed_abi_string_gc`
  - `cargo test -p scoopc managed_extern_ -- --nocapture`
  - `cargo clippy --all-targets -- -D warnings`
- Updated `TODO.md` to mark `P3-T02` as complete and recorded the final implementation/validation summary there.
