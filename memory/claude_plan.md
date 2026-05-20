# Execution Plan

## Scope

- Source of truth: `TODO.md` and `TODO-1.md`.
- First incomplete task: `P0-T02R`, `Review statement-level comptime 删除结果`.
- Goal for this invocation: complete exactly `P0-T02R`, update completion records, commit, and stop.
- If review finds a concrete blocker in the `P0-T02` deletion work, fix it within this review when feasible; if it cannot be fixed correctly in this invocation, add the minimum prerequisite task before `P0-T02R`, keep `P0-T02R` incomplete, commit the bookkeeping, and stop.
- This file records task understanding, execution steps, milestone updates, validation results, and blockers. It does not record private reasoning.

## Step-by-Step Plan

1. Check the latest commit message for any unfinished issue directly relevant to `P0-T02R`.
2. Inspect the required review locations from `TODO-1.md`: AST, statement parser, parser tests, comptime interpreter, typecheck lowering, MIR materialization templates, and HIR lowering.
3. Search active source and fixtures for `comptime for`, `comptime if`, `RuntimeComptimePlan`, `ComptimeFor`, `StmtKind::Comptime`, and the P0-T02 required search terms.
4. Confirm whether statement-level comptime surface/runtime plan are physically removed rather than replaced by dedicated reject/compatibility branches or migrated into HIR/MIR/typecheck special cases.
5. If residual active implementation code is found, remove or correct it as part of this review, then repeat the targeted inspection.
6. Run P0-T02 validation commands: `cargo fmt`, `cargo test -p scoopc --no-default-features parser`, `cargo test -p scoopc --no-default-features hir`, `cargo test -p scoopc --no-default-features mir`, and the required active-source searches.
7. Also run the P0-T02 completion-record validation that is relevant for review when feasible: `cargo test -p scoopc --no-default-features comptime`, `cargo clippy --all-targets -- -D warnings`, and the listed fixture checks.
8. Update `TODO.md` and `TODO-1.md` by marking `P0-T02R` as `[DONE]` and filling the completion record with review scope, conclusions, validation commands, and residual risks.
9. Commit all relevant changes with a `P0-T02R` message and stop without starting `P0-T03`.

## Progress

- Initial task identification complete: `P0-T02R` is the first incomplete task in `TODO.md` and `TODO-1.md`.
- Execution plan updated before running code or build/test commands for this invocation.
- Latest commit checked: `b650bee3 [P0-T02] Remove statement-level comptime`; it directly corresponds to the implementation under review and does not mention an unfinished issue that changes `P0-T02R` scope.
- Review inspection so far found no active `StmtKind::Comptime*`, `ComptimeFor`, `RuntimeComptimePlan`, or `parse_comptime_stmt` residuals in `crates/scoopc/src`.
- A stale `splice_field_contracts` comment still mentioned comptime expansion values even though HIR lowering now consumes only the typecheck contract; the comment was corrected in `crates/scoopc/src/ast/mod.rs`.
- Validation passed: `cargo fmt`; `cargo test -p scoopc --no-default-features parser`; `cargo test -p scoopc --no-default-features hir`; `cargo test -p scoopc --no-default-features mir`; `cargo test -p scoopc --no-default-features comptime`; `cargo clippy --all-targets -- -D warnings`; and the three targeted fixture runs from the P0-T02 completion record.
- Completion bookkeeping complete: `P0-T02R` is marked `[DONE]` in `TODO.md` and `TODO-1.md`, with review conclusions, validation commands, fixture classification, and residual risks recorded.
