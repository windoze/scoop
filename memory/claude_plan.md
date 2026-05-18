# Claude Execution Plan

## Scope

- Work from `TODO.md` as the source of truth.
- Complete only the first incomplete task, then stop.
- If the selected task is blocked by a concrete prerequisite, update `TODO.md`, commit that bookkeeping, and stop.

## Execution Steps

1. Read `TODO.md` to identify the first heading not prefixed with `[DONE]`.
2. Check recent repository context only as needed for that task, including the latest commit if it appears directly relevant.
3. Inspect the files and tests related to the selected task.
4. Implement the smallest spec-correct change that fully satisfies the selected task.
5. Add or update relevant tests and fixtures.
6. Run targeted validation first, then broader required validation from the task record.
7. Fix any failures caused by the current task or any blocker that invalidates it.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
9. Update this plan file after key progress points or if the plan changes.
10. Inspect `git status`, `git diff`, and recent commits before committing.
11. Commit all intended changes for the completed task with a task-specific message.
12. Stop without starting the next task.

## Current Status

- `TODO.md` inspected.
- First incomplete task: `P7-B2.5：B-17/B-18 scalar coercion 与 literal/string contract`.
- Next step: inspect recent commit context and B-17/B-18 audit/codegen/verifier/fixture files only.

## P7-B2.5 Working Plan

1. Confirm the latest commit does not mention a directly relevant unfinished B-17/B-18 issue.
2. Use `umb-audit list --bucket B-17` and `--bucket B-18` plus the strategy/category docs to lock the exact active IDs and source locations.
3. Inspect existing MIR materialized verifier coverage and LLVM fallback sites for scalar coercion, equality, bool/string operators, and literal/string value loading.
4. Add verifier/helper coverage for the whole identified B-17/B-18 contract class instead of patching individual sites.
5. Replace retired codegen `UnsupportedMainBody` fallbacks with verifier-backed internal invariants.
6. Update active inventory, retired ledger, bucket docs, fixture index/status, stale count baseline, and TODO completion record.
7. Run the task-required validation and formatting/lint checks.
8. Commit the completed P7-B2.5 change and stop.

## Progress Log

- Identified active B-17 IDs: 47; active B-18 IDs: 4.
- Replaced B-17/B-18 `UnsupportedMainBody` fallback sites with verifier/typecheck-backed internal invariants while leaving unrelated B-13/B-29 rows active.
- Activated B-17/B-18 fixtures and corrected active negative diagnostics.
- Targeted fixture validation passed: `cargo run -p scoop -- test tests/fixtures/umb_fix/B-17-coercion-scalar/` and `cargo run -p scoop -- test tests/fixtures/umb_fix/B-18-literals-strings/`.
- Updated audit inventory, retired ledger, bucket docs, fixture index, stale counts, and policy sentinel baseline.
- Final validation passed: `umb-audit diff`, `umb-audit stats`, `cargo test -p scoopc audit:: -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, both B-17/B-18 fixture directories, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings`.
- Next step: inspect final diff/status and commit P7-B2.5.
