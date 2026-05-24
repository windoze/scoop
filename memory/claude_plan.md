# Claude Execution Plan

## Current Invocation

I will complete exactly the first incomplete task from `TODO.md`, then stop after committing the result.

## Plan

1. Read `TODO.md` to identify the first task whose title is not prefixed with `[DONE]`.
2. Check recent repository state only as needed for that task, including whether the latest commit mentions an unfinished issue directly relevant to it.
3. Inspect the files and tests relevant to the selected task.
4. Implement the selected task completely, or add the minimum prerequisite task to `TODO.md` if a concrete blocker makes correct implementation impossible.
5. Run targeted validation first, then broader required validation for the task, addressing any unscheduled failing tests or fixtures.
6. Update `TODO.md` with `[DONE]` and a completion record only after implementation and validation pass.
7. Update this file when key steps complete or the plan changes.
8. Review git status and diff, then commit all intended changes with a task-specific message.
9. Stop without starting the next task.

## Status

- Plan file initialized.
- `TODO.md` identifies `P9-T06` as the first incomplete task: extract `scoopc_effect_facts_stage` and `scoopc_lir` crates.
- `TODO-7.md` details `P9-T06`: create `scoopc_effect_facts_stage` for effect-facts builder/stage glue and `scoopc_lir` for `effect_lowered/`, update `scoopc` façade, pipeline imports, dependency gate, and ensure LLVM depends directly on `scoopc_lir`.
- Latest commit is the completed `P9-T06-c` prerequisite; no additional unfinished blocker from the latest commit was found.
- Current unrelated untracked file: `PLUGIN_ABI.md`; leave it untouched.
- Inspection found `scoopc_effect_facts_stage` and `scoopc_lir` already present in the workspace, with `scoopc` facade re-exports and `scoopc_codegen_llvm` direct `scoopc_lir` dependency already wired.
- Validation passed for `P9-T06`: `cargo fmt`, `cargo build --workspace`, `cargo test --all --all-targets`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`, `cargo run -p scoop_tools -- dependency-gate`, `cargo tree -p scoopc_lir --depth 1`, `cargo tree -p scoopc_codegen_llvm --depth 1`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
- `TODO.md` and `TODO-7.md` were updated to mark `P9-T06` `[DONE]` and record completion details.
- Next step: inspect the final diff/status and commit the completed task.
