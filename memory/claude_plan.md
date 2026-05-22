# Claude Execution Plan

## Current Invocation

This file records the public execution plan and progress for the current TODO-driven invocation. It intentionally contains actionable rationale and decisions, not private chain-of-thought.

## Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Review the selected task body, dependencies, validation requirements, and completion-record expectations.
3. Check recent repository state only as needed for the selected task, including whether the latest commit mentions an unfinished issue directly relevant to it.
4. Implement the selected task fully, unless a concrete blocker requires adding the minimum prerequisite task to `TODO.md` and stopping.
5. Run targeted validation for the affected area, then broader required checks when feasible. Any observed unscheduled test or fixture failure must be fixed or explicitly scheduled before completion.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record. Update `PLAN.md` only if phase-level sequencing or criteria change.
7. Commit all intended changes for this invocation with a descriptive task-tagged commit message.
8. Stop after completing exactly one task.

## Progress

- Started invocation and created this execution plan before running project commands.
- Read `TODO.md`; first incomplete task is `P7-T04-b-3` in `TODO-6.md`.
- Read the `P7-T04-b-3` task card. Scope is to introduce `hir::ClassInstanceKey`, change `ClassInitIndex` and LLVM layout helpers from string keys to typed keys, delete silent layout-key fallbacks, and add MIR verifier coverage for class constructor typed-target mismatch.
- Implemented the initial typed-key conversion: added `ClassInstanceKey`, changed `ClassInitIndex`, updated HIR class-init insertion, switched LLVM class layout/type-desc/class-ctor helpers to typed keys, removed the targeted `class_fqn.to_string()` fallbacks, and added a materialized MIR validation test for class-ctor result nominal mismatch.
- Validation completed successfully: `cargo test -p scoopc_types`, `cargo test -p scoopc --no-default-features hir`, `cargo test -p scoopc --no-default-features mir`, `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`, `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` all passed.
- Marked `P7-T04-b-3` as `[DONE]` in `TODO.md` and `TODO-6.md`; no `PLAN.md` update was needed because phase-level sequencing did not change.

## Current Task Plan

1. Inspect the latest commit and current worktree for directly relevant unfinished state.
2. Locate all existing class layout key construction and `class_inits` read/write sites.
3. Add `ClassInstanceKey` with controlled constructors in HIR and upgrade `ClassInitIndex`.
4. Update HIR lowering / generic monomorph collection to insert typed class keys.
5. Update LLVM layout APIs, effect-lowered class ctor layout key, and MIR-body class ctor layout key to use `ClassInstanceKey` and return verifier errors instead of string fallbacks.
6. Add/extend MIR verifier checks for `ClassCtor` target-local presence and nominal type match.
7. Run the task validation commands, fix observed unscheduled failures or schedule concrete prerequisites if blocked.
8. Mark `P7-T04-b-3` done in both TODO indexes, update this progress file, commit, and stop.
