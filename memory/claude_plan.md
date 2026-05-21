# Execution Plan

## Scope
- Follow `TODO.md` as the authoritative ordered task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Do not perform broad historical triage before selecting that task.

## Steps
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for unfinished work directly relevant to that task.
3. Inspect the relevant source, fixtures, and tests needed for the selected task.
4. Implement the smallest spec-correct change that completes the selected task.
5. Run targeted validation first, then broader required validation from the task.
6. If a concrete blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
7. If the task is completed, update `TODO.md` by prefixing the task heading with `[DONE]` and filling the completion record.
8. Commit all task-related changes with a descriptive task-tagged commit message.

## Progress Log
- Initialized execution plan before reading project task files.
- Read `TODO.md`; first incomplete task is `P3-T03R` in `TODO-4.md`, the review for MIR snapshot binding and pass artifacts query surface.
- Latest commit is `[P3-T03] Publish canonical MIR snapshot handoff`; it is directly relevant and does not explicitly mention unfinished follow-up work in the commit subject.
- Review focus: confirm P4-ready `MirStageOutput` has mandatory canonical `MaterializedMir`, `MirFacts` owns snapshot/pass metadata, and direct-style dump helpers cannot be mistaken for P4-ready outputs.
- Review completed with no blocking code changes: optional snapshot search terms have no active Rust matches; P4-ready handoff and helper separation satisfy `P3-T03R`.
- Validation completed: `cargo fmt`, `cargo test -p scoopc_mir_facts`, `cargo test -p scoopc --no-default-features mir_stage`, `cargo test -p scoopc --no-default-features effect_facts_stage`, `cargo test -p scoopc --no-default-features effect_lowering_stage`, `cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
- Updated `TODO.md` and `TODO-4.md` to mark `P3-T03R` as `[DONE]` with review conclusions and residual risks.
