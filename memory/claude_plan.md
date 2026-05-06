# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative source.
- First incomplete task identified: `MIR-T09R：Review MIR-T09 runtime value primitives`.
- Complete exactly this review task, then stop after committing.

## Initial Steps

1. Check the latest commit message only for unfinished work directly relevant to `MIR-T09R`.
2. Read only the `MIR-T09` implementation, tests, fixtures, and docs needed to review runtime value primitive metadata.
3. Confirm the negative function-type cast path is rejected before MIR.

## Execution Steps

1. Re-run all `MIR-T09` validation commands required by `MIR-T09R`.
2. Inspect MIR dumps for `runtime_typecheck_cast.scoop`, `not_null_assert.scoop`, and `pattern_is_type.scoop`.
3. Search/review runtime typecheck, cast, pattern, not-null, materialization, and verifier code paths for placeholder/default-value/panic-only behavior.
4. If the review finds no blocking gap, mark `MIR-T09R` `[DONE]` and record the review findings and validations.
5. If the review finds a blocking gap, keep `MIR-T09R` incomplete, record the issue in `TODO.md`, and stop after committing the bookkeeping or fix as appropriate.

## Documentation And Completion

1. Update this file when the selected task is identified, when implementation starts, when validation completes, and if the plan changes.
2. Mark the task heading in `TODO.md` with `[DONE]` only after implementation and validation are complete.
3. Update the task completion record with files changed, validation commands, and results.
4. Update `PLAN.md` only if phase-level sequencing or completion criteria change.

## Progress Update

- Selected task: `MIR-T09R`.
- Review found one directly relevant verifier gap in `MIR-T09`: production runtime value metadata checks did not verify operand/source type consistency for `TypeCheck`/`Cast`, did not verify `as?` optional result type against the assignment target local, and did not verify top-level pattern `is` subject metadata against the pattern subject operand.
- Implemented the verifier tightening in `crates/scoopc/src/mir/mod.rs` and added forged metadata negative tests.
- Validation completed successfully after the fix: value primitive tests, new metadata verifier tests, three MIR dumps, function-type cast diagnostics fixtures, materialized MIR tests, no-Todo tests, placeholder inventory, and clippy warnings-as-errors.

## Commit Plan

1. Review `git status`, `git diff`, and recent commit style.
2. Commit all changes required for this completed task, including any pre-existing uncommitted files if this is a resumed task.
3. Stop after the commit and report the completed task and validation results.
