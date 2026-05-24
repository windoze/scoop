# Current Invocation Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Avoid broad historical triage before selecting the current task.
- Do not use workarounds for missing or incorrect behavior; if a concrete blocker prevents the task, schedule the minimum prerequisite in `TODO.md`, commit, and stop.

## Steps

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect the files and tests relevant to the selected task.
4. Implement the smallest correct change needed for the task, or add a prerequisite task if the task is concretely blocked.
5. Run focused validation first, then required broader validation from the task.
6. If validation exposes an unscheduled test or fixture failure, fix it or schedule it before marking the task done.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record; update `PLAN.md` only if phase-level planning changes.
8. Run final relevant checks, inspect git status/diff/log, and commit all task-related changes with a clear task-tagged message.
9. Stop after committing this one task.

## Progress Log

- Initialized execution plan before reading task files or running commands.
- Identified first incomplete task: `P9-T06-c` in `TODO-7.md`, "发布 codegen-owned LLVM stage handoff 合同".
- Latest commit `4628494f [P9-T06] Extract LIR crates and schedule handoff prerequisite` is directly relevant because it scheduled this prerequisite before continuing P9-T06; this invocation will complete `P9-T06-c` rather than advancing to P9-T06.
- Required validation from the task: `cargo fmt`, `cargo check -p scoopc_codegen_llvm`, `cargo build --workspace`, `cargo test --all --all-targets`, `cargo run -p scoop_tools -- dependency-gate`, `cargo tree -p scoopc_codegen_llvm --depth 1`, and `git diff --check`.
- Implemented the main handoff change: moved LLVM stage output/base context/artifact types into `scoopc_codegen_llvm`, removed the codegen crate's normal `scoopc` dependency, and added a `scoopc::llvm` wrapper for frontend-orchestrated single-file helpers.
- Focused checks completed so far: `cargo check -p scoopc_codegen_llvm`, `cargo check -p scoopc`, `cargo tree -p scoopc_codegen_llvm --depth 1`, and `cargo run -p scoop_tools -- dependency-gate`.
- Completed validation: `cargo fmt`, `cargo check -p scoopc_codegen_llvm`, `cargo build --workspace`, `cargo test --all --all-targets` with 60-minute timeout, `cargo run -p scoop_tools -- dependency-gate`, `cargo tree -p scoopc_codegen_llvm --depth 1`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` all passed.
- Updated `TODO.md` and `TODO-7.md` to mark `P9-T06-c` as `[DONE]` with completion notes.
