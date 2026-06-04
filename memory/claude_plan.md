# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after completing and committing that single task, or after committing any required prerequisite/blocker scheduling if the task cannot be completed as written.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Read the task body, dependencies, validation requirements, and completion-record format.
4. Inspect only the relevant implementation, fixture, and test areas needed for the selected task.
5. Implement the smallest spec-correct change that completes the selected task without workaround behavior.
6. Add or update tests/fixtures required by the task and by any root-cause fixes found while implementing it.
7. Run `cargo fmt`, then `cargo clippy --all-targets -- -D warnings`, then the task-required test and fixture validation, using long timeouts for full suites when needed.
8. If any unscheduled test or fixture failure is observed, fix it if in scope or add the minimum prerequisite/follow-up task in `TODO.md` before marking the selected task complete.
9. Mark the selected task heading with `[DONE]` and update its completion record with implementation and validation details.
10. Review `git status`, `git diff`, and recent commits; commit all relevant changes with a task-tagged message.
11. Stop without starting the next task.

## Progress Log

- Initial execution plan recorded before shell commands or code execution.
- Selected first incomplete task: `TC-02：plain 路径（mir_body/）改 walk LIR 指令`.
- Next check: inspect latest commit message for unfinished work directly relevant to `TC-02`, then inspect the plain codegen path only.
- Latest commit is `[TC-02-PRE1] Add LIR closure adapters`; no extra unfinished commit-body item was found beyond the TC-02 failures already recorded in `TODO.md`.
- Implementation phase started: map current `mir_body/` MIR matches to LIR instruction equivalents, then replace plain callable traversal from MIR blocks/slices to `LirExecutableBody` states/instructions.
- Current `cargo test -p scoopc pipeline::llvm_codegen_stage::tests:: --lib` result shows 4 remaining LLVM unit failures: one `Array<String>` argv call ABI panic, two stale value-box/enum-box IR-name expectations, and one stale closure-env body matcher based on old `pass_mir_*` names.
- Next implementation focus: add LIR/plain call ABI support for runtime-reference nominal collection values such as `scoop.core.Array<T>`, then update unit assertions to require the LIR-specific allocation/payload markers while preserving descriptor-backed allocation and env-load semantics.
- Implemented LIR call ABI fixes for runtime-reference nominal collections and generic call-site signatures, LIR named intrinsic lowering, and LIR atomic-ref intrinsic lowering. Updated LLVM unit assertions from stale MIR-only IR markers to LIR markers where appropriate.
- `cargo test -p scoopc pipeline::llvm_codegen_stage::tests:: --lib -- --format terse` now passes 37/37. Starting required validation sequence with `cargo fmt`, then clippy, then full suites.
- Validation update: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `cargo build -p scoop -p scoopc`, `python3 tools/dependency_gate.py`, and `python3 tools/spec_fixtures.py check` pass.
- Full fixture suite still fails with 31 targets after the TC-02 partial fixes. This blocks marking `TC-02` done. I will add a minimal `TC-02-PRE2` prerequisite in `TODO.md` covering the remaining failures, keep `TC-02` incomplete, commit the current code plus task-list update, and stop.
