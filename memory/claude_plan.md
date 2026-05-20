# Claude Execution Plan

I cannot record private chain-of-thought. This file records the actionable execution plan, decisions, and progress for the current invocation.

## Initial Plan

1. Read `TODO.md` first to identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work that is directly relevant to that first incomplete task.
3. Read the selected task body, dependencies, validation requirements, and completion-record expectations.
4. Inspect only the code and fixtures needed for that task.
5. Implement the task as written, without narrowing scope or using fixture-only workarounds.
6. If a concrete prerequisite or spec mismatch blocks the task, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
7. Run focused validation first, then any required broader validation listed by the task.
8. Mark exactly the selected task complete by prefixing its title with `[DONE]` and updating its completion record.
9. Inspect `git status`, `git diff`, and recent commits before committing.
10. Commit all intended changes for this invocation with a descriptive task-tagged message, then stop.

## Progress

- Plan file initialized before running repository commands.
- First incomplete task identified: `P0-T04R` in `TODO-1.md` (`Review P0 全包完成度`).
- Latest commit is `[P0-T04] Complete P0 cleanup`; it is directly relevant as the implementation under review but does not itself mention an unfinished issue.
- Next step: perform the P0 review by checking required paths, classifying legacy surface search results, running required validation, then update the review completion record and commit.
- Review search/classification pass completed: no old `ConstEval`, `const fun`, `const val`, `Modifier::Const`, `Keyword::Const`, `is_const`, trimming, runtime comptime plan, or comptime AST/lowering identifiers were found in active `crates/scoopc/src`, `sysroot`, `stdlib`, or build overlays. Remaining active `comptime` hits are the lexer reserved tombstone, parser recovery/display/test, and parse-fail regression fixtures.
- Spot checks completed: `tests/fixtures/parse/comptime_syntax_basic.scoop` and `tests/fixtures/parse/package_level_comptime_if_basic.scoop` assert ordinary parse failures; the sysroot and one build overlay expose reflection helpers as ordinary `@Intrinsic fun` returning `MetaList<T>`, not old `const fun` / `ComptimeList`.
- Review found one stale non-language reference in `tools/scoop_tools/src/fixtures_matrix.rs`: the stdlib reflection coverage domain still matched old `comptime_*` fixture prefixes. Updated it to the current `reflection_` prefix so active tooling no longer refers to old comptime reflection fixture names.
- Because a file changed after the first validation run, required validation was rerun after this fix.
- Validation rerun completed successfully: `cargo fmt`, `cargo test --all --all-targets --no-default-features`, `cargo run -p scoop -- test`, `cargo run -p scoop_tools -- spec-fixtures check`, `cargo clippy --all-targets -- -D warnings`, and legacy surface searches.
- `TODO.md` and `TODO-1.md` updated to mark `P0-T04R` as `[DONE]`; `PLAN.md` top status updated to show P0 review complete and P1/TODO-2 initialization as the next step.
