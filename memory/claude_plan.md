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

- Plan file initialized before reading task files or running commands.
- Selected task: `P6-T03d` in `TODO-P6-part3.md` - close refactor function ABI and entry shell lowering, including main wrapper.
- Latest commit checked: `84edfe87 [P6-T03c] Implement refactor pure statement lowering`; it directly precedes the selected task and does not advertise an unfinished blocker.
- Implementation status: code changes complete for `P6-T03d`; `TODO-P6-part3.md` and `TODO.md` marked `[DONE]` with completion notes.
- Validation status: passed `cargo test -p scoopc refactor_llvm_function_abi`, `cargo test -p scoopc refactor_llvm_main_wrapper`, `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/p6_refactor_main.ll`, and `cargo clippy --all-targets -- -D warnings`.
- Commit status: committed as `05ae0939 [P6-T03d] Close refactor function ABI shells`; recording final status in a follow-up progress commit.

## Current Edit Plan

1. Add fail-fast handling before refactor main tries to reuse the legacy `scoop_entry_argv_array` helper for `main(args: Array<String>)`.
2. Change refactor main wrapper so `Step_F::Complete` still maps `Unit`/`Int` to process exit codes, while a non-Complete `Step` with published outward cases returns a stable nonzero exit code instead of using `unreachable`.
3. Keep `unreachable` only when the published entry `Step` layout proves there are no outward cases.
4. Add tests named for the required validation filters: `refactor_llvm_function_abi*` and `refactor_llvm_main_wrapper*`.
