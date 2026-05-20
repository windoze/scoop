# Execution Plan

## Scope

- Source of truth: `TODO.md` and the referenced detailed task in `TODO-1.md`.
- First incomplete task: `P0-T02`, `删除 statement-level comptime if/for 与 runtime comptime plan`.
- Goal for this invocation: complete exactly `P0-T02`, update completion records, commit, and stop.
- If a concrete prerequisite blocks correct execution, add the minimum prerequisite task in `TODO.md` / `TODO-1.md`, commit that bookkeeping, and stop.
- This file records task understanding, execution steps, milestone updates, validation results, and blockers. It does not record private reasoning.

## Step-by-Step Plan

1. Check the latest commit message for any unfinished issue directly relevant to `P0-T02`.
2. Inspect the statement-level comptime implementation surface listed by `P0-T02`: AST, statement parser, parser tests, runtime comptime planner/walker, typecheck lowering, HIR lowering, MIR materialization, and listed fixtures.
3. Search for `StmtKind::Comptime`, `ComptimeFor`, `RuntimeComptimePlan`, `parse_comptime_stmt`, and related `comptime if/for` identifiers to find all active code paths.
4. Remove statement-level `comptime if/for` AST nodes and parser entry points so `comptime` is no longer a valid statement-start keyword.
5. Remove the runtime comptime plan and AST walker while preserving const-eval code that belongs to `P0-T03`.
6. Clean downstream typecheck/HIR/MIR branches that only existed because comptime statements were expected to be pre-spliced.
7. Update or delete the fixtures named by `P0-T02` so they no longer validate available statement-level comptime behavior or runtime splicing.
8. Run required validation: `cargo fmt`, `cargo test -p scoopc --no-default-features parser`, `cargo test -p scoopc --no-default-features hir`, `cargo test -p scoopc --no-default-features mir`, and the required active-source search.
9. Run `cargo clippy --all-targets -- -D warnings` if feasible because the repository instructions require warning-free compilation/linting; fix issues caused by this task.
10. Update `TODO.md` and `TODO-1.md` by marking `P0-T02` as `[DONE]` and filling the completion record with scope, decisions, validations, and residual risk.
11. Commit all relevant changes with a `P0-T02` message and stop without starting `P0-T02R`.

## Progress

- Initial task identification complete: `P0-T02` is the first incomplete task in `TODO.md` and `TODO-1.md`.
- Execution plan recorded before running code or build/test commands.
- Latest commit checked: `b992c0c8 [P0-T01R] Review package-level comptime removal`; no unfinished issue in the message blocks or preempts `P0-T02`.
- Code/fixture inspection found the statement-level comptime surface in AST, parser, resolver, typecheck, HIR lowering, const interpreter runtime planner/evaluator branches, HIR runtime plan plumbing, preflight tests, parser tests, and the P0-T02 fixtures.
- The implementation will remove the full statement-level `comptime` statement family (`comptime {}`, `comptime if`, `comptime for`) because the required validation forbids active `StmtKind::Comptime*` and `parse_comptime_stmt` hits, and `comptime` must no longer be a valid statement start.
- Source edits complete for the main removal path: AST/parser `StmtKind::Comptime*`, runtime comptime plan/walker, const-interpreter statement branches, resolver/typecheck/HIR lowering branches, runtime plan plumbing, and comptime splice fallback were removed.
- Fixture edits complete: old statement-level comptime HIR/MIR/comptime/UMB fixtures were deleted or rewritten; the parse fixture now asserts ordinary parse failure for block-level `comptime`.
- Validation complete for code paths: `cargo fmt`; `cargo test -p scoopc --no-default-features parser`; `cargo test -p scoopc --no-default-features hir`; `cargo test -p scoopc --no-default-features mir`; `cargo test -p scoopc --no-default-features comptime`; `cargo clippy --all-targets -- -D warnings`; active-source search for `StmtKind::Comptime|ComptimeFor|RuntimeComptimePlan|parse_comptime_stmt` returned no matches.
- Fixture validation: `cargo run -p scoop -- test --fixtures tests/fixtures/parse/comptime_syntax_basic.scoop`, `... literal_const_comptime_matrix.scoop`, and `... float_literal_basic.scoop` passed. A combined multi-fixture command was rejected by the CLI because this invocation shape accepts only one fixture argument; the fixtures were then run individually.
- Completion bookkeeping complete: `P0-T02` is marked `[DONE]` in `TODO.md` and `TODO-1.md` with scope, decisions, validation commands, and residual risks.
