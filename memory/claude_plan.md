# Claude Execution Plan

## Scope

- Use `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after implementing, validating, documenting, and committing that one task.

## Plan

1. Read `TODO.md` and identify the first incomplete task.
2. Check the latest commit only for directly relevant unfinished work.
3. Inspect the code and tests needed for that task.
4. Implement the smallest spec-correct change for the task.
5. Run targeted validation first, then required broader validation for the task.
6. Address any observed unscheduled test or fixture failures before marking the task done.
7. Update `TODO.md` with `[DONE]` and a completion record when the task is complete.
8. Commit all task-related changes with a descriptive task-id commit message.

## Progress

- Started execution plan log.
- Identified first incomplete task: `P9-T03R` in `TODO-7.md`, a review of the `scoopc_codegen_llvm` extraction.
- Recent commit is `[P9-T03] Extract scoopc_codegen_llvm crate`; no separate unfinished issue was mentioned in the latest commit summary.
- Reviewed the LLVM extraction shape: `scoopc_codegen_llvm` is still a staged crate that temporarily depends on the `scoopc` facade, and the source tree documents the required P9-T06 switch to direct `scoopc_lir`/`scoopc_lir_facts` inputs.
- Residual searches found no non-test direct HIR residuals in the LLVM source tree and no production direct MIR residuals outside the documented `codegen/mir_body` source-body helpers.
- Validation completed successfully: `cargo fmt`, `cargo build --workspace --features llvm`, `cargo test --all --all-targets --features llvm`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets --features llvm -- -D warnings`, cargo tree checks, and targeted residual searches.
- Marked `P9-T03R` complete in `TODO.md` and `TODO-7.md`; next task is `P9-T04`.
