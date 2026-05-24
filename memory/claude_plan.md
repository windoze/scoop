# Execution Plan

I will follow `TODO.md` as the source of truth, complete exactly the first task whose heading is not prefixed with `[DONE]`, validate the result, update task bookkeeping, commit the finished work, and stop.

## Steps
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for an explicitly unfinished issue directly relevant to that task.
3. Inspect the code, fixtures, and docs needed for that task without broad unrelated triage.
4. Implement the task as specified, avoiding workarounds or scope narrowing.
5. Run formatting, linting, targeted validation, and then required full validation in the requested order.
6. If any unscheduled test or fixture failure appears, fix it or add the minimum required prerequisite/follow-up task in `TODO.md` before marking completion.
7. Mark the completed task heading with `[DONE]` and update its completion record in `TODO.md`; update `PLAN.md` only if phase-level sequencing changed.
8. Commit all task-related changes with a clear message and stop.

## Progress
- Created this plan file before task execution.
- Identified `P9-T08` as the first incomplete task.
- Audited the umbrella crate and found `scoopc` still had LLVM-private build/dependency glue plus a non-facade `llvm.rs` wrapper.
- Implementation focus: move LLVM toolchain ownership to `scoopc_codegen_llvm`, move single-file emit orchestration into `pipeline/`, leave `scoopc::llvm` as a façade, extend `dependency_gate` with cone crate rules, update README/TODO completion records, then validate and commit.
- Implemented the umbrella/codegen boundary cleanup: removed direct LLVM-private deps, stale umbrella-only manifest deps, and `scoopc/build.rs`; added the LLVM toolchain check to `scoopc_codegen_llvm`; made `scoopc::llvm` a compatibility façade; and re-exported stackmap from the codegen crate.
- Implemented dependency-gate cone coverage and README crate-structure updates.
- Validation completed successfully in required order, including the final rerun after manifest cleanup: `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo build --workspace`; `cargo test --all --all-targets`; `cargo run -p scoop -- test`; `cargo run -p scoop_tools -- dependency-gate`; `git diff --check`.
- Marked `P9-T08` `[DONE]` in `TODO.md` and `TODO-7.md` with completion record.
