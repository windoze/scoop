# Execution Plan

I will follow the task list in `TODO.md` and complete exactly the first incomplete task, stopping after committing that task.

Selected task: `P0-T01R` — review the old surface / sysroot / fixture migration inventory produced by `P0-T01`. This is a documentation/review task; it should not change compiler behavior unless the review finds a concrete inventory gap that must be corrected.

1. Confirm `P0-T01R` is the first incomplete task in `TODO.md` and `TODO-1.md`, and check the latest commit for directly relevant unfinished work.
2. Re-read the `P0-T01` completion record and `P0-T01R` acceptance criteria.
3. Independently sample the listed old-surface hits and fixture globs across spec, sysroot, fixtures, parser, typecheck, lowering, MIR, and cone visibility/export entry points.
4. Reverse-check for obvious omissions in the required categories: `perform`, handler `with`, tuple `._0` / `._1`, f-string interpolation/brace escapes, `@Inline`, `AnyRef` / `AnyValue`, implicit public declarations, and operator-like functions without `operator`.
5. If the inventory is complete, mark `P0-T01R` `[DONE]` in both `TODO.md` and `TODO-1.md` and fill its completion record. If a real gap appears, repair the inventory or add the minimum prerequisite/follow-up task instead.
6. Run the required validation for this review task: `python3 tools/spec_fixtures.py check` and `python3 tools/run_fixtures.py`, plus targeted sampling commands used for the review.
7. Commit all task-related changes with a descriptive `P0-T01R` commit message, then stop.

Progress:
- Identified `P0-T01R` as the first incomplete task in `TODO.md` and `TODO-1.md`.
- Confirmed the latest commit is `[P0-T01] Record old surface migration inventory`, directly relevant as the subject of this review task.
- Re-read the `P0-T01` completion record and independently sampled the required old-surface categories across spec, sysroot, fixtures, parser, typecheck, lowering, MIR, cone export/visibility, and embedded Rust test snippets.
- Review finding: the `P0-T01` inventory is sufficient and does not require a new prerequisite task, but later migration tasks should use content searches for handler `with`, tuple `._N` / `with { _N... }`, and f-string `{...}` hits because many affected fixtures are not named after those surfaces.
- Confirmed the reviewed commit changed only `TODO.md`, `TODO-1.md`, and `memory/claude_plan.md`; it did not add positive fixtures or compiler behavior changes.
- Validation passed: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
- Marked `P0-T01R` done in `TODO.md` and `TODO-1.md` with review findings and validation results.
- Next step: commit the completed `P0-T01R` review changes and stop.
