# Claude Execution Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after committing that single task or, if blocked, after recording the minimum prerequisite task and committing the bookkeeping change.

## Operating Constraints

- Do not skip review tasks or tasks with completion notes but no `[DONE]` prefix.
- Do not perform broad historical triage before selecting the current task.
- Do not use workarounds, fixture-only hacks, weakened fixtures, alternate representations, or spec deviations.
- Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria change.
- Mark the completed task by prefixing its `TODO.md` heading with `[DONE]` and updating its completion record.
- Commit all relevant uncommitted changes for the completed or blocked task before stopping.

## Current Task

- First incomplete task from `TODO.md`: `P7-A3：B-15 when / 模式匹配用户面早拒`.
- Scope: retire 55 `B-15` `FrontendReject` `UnsupportedMainBody` rows by moving user-facing `when` and pattern errors to frontend/typecheck/HIR/MIR verification, then update inventory, retired ledger, category/strategy docs, fixture coverage, stale count, and completion record.
- Required validation: `cargo test -p scoopc audit:: -- --nocapture`; `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`; `cargo run -p scoop -- test tests/fixtures/umb_fix/B-15-when-pattern/`; `cargo run -p scoop -- test tests/fixtures/typecheck/`.

## Step-by-Step Plan

1. Check the latest commit subject for any unfinished issue directly relevant to `P7-A3`.
2. Run `cargo run -p scoopc --bin umb-audit -- list --bucket B-15` to lock the active `B-15` IDs and source locations.
3. Inspect `audit/strategies/B-15.md`, `audit/UMB_categories/B-15.md`, active B-15 fixture files, and the codegen locations reported by the audit list.
4. Inspect existing parser/typecheck/HIR/MIR handling for `when`, enum variants, guard expressions, arm type unification, and payload arity to find the correct upstream gates.
5. Implement the upstream user-facing diagnostics for the full B-15 class without weakening fixture shape or substituting alternate representations.
6. Remove the B-15 codegen fallback constructors once the upstream gates make them unreachable, using assertions/panics only for internal invariants where appropriate.
7. Activate or add B-15 negative fixtures for enum variant missing/unknown, payload arity mismatch, arm type mismatch, invalid guard type, and exhaustiveness; ensure diagnostics avoid forbidden fallback terms.
8. Regenerate/update `audit/UMB_inventory.csv`, append the retired B-15 IDs to `audit/UMB_retired.csv`, and update `audit/UMB_categories/*`, `audit/strategies/B-15.md`, fixture index, stale count baseline, and any line-drift documentation required by audit.
9. Run the task-required validation commands, plus `cargo run -p scoopc --bin umb-audit -- diff`, `cargo run -p scoopc --bin umb-audit -- stats`, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` if code/docs changed.
10. If a concrete blocking implementation gap prevents spec-correct B-15 retirement, insert the minimum prerequisite task before `P7-A3` in `TODO.md`, record the blocker here, commit that bookkeeping, and stop.
11. If validation passes, mark `P7-A3` as `[DONE]` in `TODO.md`, fill the completion record, review the final diff/status/log, commit all relevant files with a `[P7-A3]` message, and stop.

## Progress Log

- Initial plan created before repository inspection.
- Identified first incomplete task as `P7-A2`: B-08/B-21 member store and struct field FrontendReject retirement.
- Latest commit is `[P7-A1] Retire B-16 control-flow context UMB rows`; no directly relevant unfinished issue was visible from the subject.
- `umb-audit list --bucket B-08` and `--bucket B-21` showed active rows currently classified as `InternalBugSentinel`, not `FrontendReject`; inspecting task references before editing implementation.
- Existing uncommitted changes already retired the P7-A2 frontend rows: B-08 `UMB-1131`/`UMB-1142`, B-21 `UMB-0750`/`UMB-0863`/`UMB-0962`.
- `umb-audit stats` passed with active=1,272, retired=12, initial=1,284; `umb-audit diff` passed with 1,272 entries in sync.
- `cargo test -p scoopc audit:: -- --nocapture` initially failed because B-08 category docs omitted the required `D-pending` row; added `D-pending=0` rows to B-08 and B-21 Expected Post-Fix Class tables.
- Validation then passed: `cargo test -p scoopc audit:: -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, B-08 fixture directory, B-21 fixture directory, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings`.
- Updated `TODO.md` to mark `P7-A2` as `[DONE]`, record retired IDs, inventory/ledger changes, stale count changes, fixture status, and validation results.
- Tightened `neg_struct_unknown_field.scoop` to cover direct struct-literal unknown fields instead of with-update unknown fields; reran B-21 fixtures successfully.
- `cargo fmt` introduced layout line drift after the removed B-21 fallback, so regenerated `audit/UMB_inventory.csv`; `umb-audit diff` and full `cargo test -p scoopc audit:: -- --nocapture` now pass again.
- Synchronized the resulting `layout.rs` line drift into affected bucket docs B-06, B-20, B-22, and B-36 so their active row tables match the regenerated inventory.
- New invocation identified first incomplete task as `P7-A3`: B-15 when / pattern matching user-facing FrontendReject retirement.
- Wrote the current `P7-A3` execution plan before running any build, test, audit, or Git commands.
- Latest commit is `[P7-A2] Retire member store struct field frontend rows`; no directly relevant unfinished issue was visible from the subject.
- `umb-audit list --bucket B-15` reports 55 active `FrontendReject` rows, all in `crates/scoopc/src/llvm/codegen/control_flow.rs`.
- Exploration found one concrete upstream gap: statement-position `when` did not typecheck guard expressions as `Bool`; expression-position paths already check guards, pattern shape, variant arity/unknown variants, and exhaustiveness.
- Implementation direction: add the missing statement-position guard check, pass HIR `when` result type into codegen when no explicit expected type is supplied, route non-switch / `is` patterns through the chain matcher, and replace B-15 codegen fallbacks with upstream-verified invariants.
- Implemented the statement-position guard Bool gate, HIR `when` expected-type fallback, generic chain matching for non-switch/`is` patterns, and replaced B-15 `UnsupportedMainBody` sites in `control_flow.rs` with internal invariants.
- Activated B-15 fixtures and added negative coverage for guard type, unknown variant, variant payload arity, and expected arm result type plus a tuple+nested-enum positive path; `cargo run -p scoop -- test tests/fixtures/umb_fix/B-15-when-pattern/` now passes with 7 fixtures.
- Updated `audit/UMB_inventory.csv` and `audit/UMB_retired.csv`; `umb-audit diff` is in sync with active=1,217, retired=67, and B-15 active entries=0.
- Updated B-15 docs, fixture index, spec coverage matrix, and audit legacy-gap accounting; `cargo test -p scoopc audit:: -- --nocapture` now passes.
- Final validation passed: `cargo run -p scoopc --bin umb-audit -- list --bucket B-15`, `... diff`, `... stats`, `cargo test -p scoopc audit:: -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, B-15 fixtures, full typecheck fixture directory, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings`.
- Marked `P7-A3` as `[DONE]` in `TODO.md` with completion record active 1,272 -> 1,217 and retired 12 -> 67.
