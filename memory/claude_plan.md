# Execution Plan

I cannot record private chain-of-thought, but this file will track the concrete execution plan, decisions, and progress for this invocation.

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for an explicitly unfinished issue that is directly relevant to that task.
3. Inspect the task's referenced code, fixtures, and validation requirements.
4. Implement the task as written, without narrowing scope or using workaround behavior.
5. Run the task-specific validation, then broader relevant tests if needed.
6. Update `TODO.md` by prefixing the task heading with `[DONE]` and filling in the completion record.
7. Update this file whenever a key step completes or the plan changes.
8. Commit all intended changes for this task with a task-tagged commit message, then stop.

## Progress

- Initial plan recorded before project inspection.
- Identified the first incomplete task from `TODO.md`: `P2-T06` in `TODO-3.md`.
- Checked the latest commit (`[P2-T05R] Review source-site contract migration`); it does not mention an unfinished issue directly relevant to `P2-T06`.

## Current Task: P2-T06

Planned execution:

1. Inspect existing legality checks, HIR facts publication, HIR preflight, and user-visible failure policy tests.
2. Add or tighten barrier checks only where current code allows a spec-invalid declaration to pass beyond HIR/typecheck.
3. Add fixtures/tests proving `@CallingConvention` generics and top-level `var` without storage policy are rejected before MIR/codegen, and legal global roots publish resolved storage policy without generic identity.
4. Update the policy test to make allowed post-HIR failure classes explicit.
5. Run the P2-T06 validation commands, fix regressions, then mark `P2-T06` done in both TODO indexes and commit.

## Implementation Progress

- Added structural `HirFacts` verifier checks for global root monomorphism and top-level var storage policy legality.
- Added HIR/preflight tests proving legal global roots are monomorphic and mutable roots carry resolved storage policy.
- Added a typecheck fixture for generic `@CallingConvention` body rejection.
- Updated the user-visible failure policy audit to include the P2 declaration legality gates and explicit post-HIR allowed failure classes.

## Validation Progress

- Ran `cargo fmt`.
- Ran `cargo test -p scoopc_hir_facts`.
- Ran `cargo test -p scoopc --no-default-features hir_preflight`.
- Ran `cargo test -p scoopc --no-default-features pipeline_user_visible_failure_policy`.
- Ran `cargo test -p scoopc --no-default-features typecheck`.
- Ran `cargo test -p scoopc --no-default-features hir_stage`.
- Ran `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`.
- Ran `cargo run -p scoop -- test --fixtures tests/fixtures/hir`.
- Ran `cargo clippy --all-targets -- -D warnings`.
- Marked `P2-T06` as `[DONE]` in `TODO.md` and `TODO-3.md`, with a completion record.
- Ran `git diff --check` successfully.
