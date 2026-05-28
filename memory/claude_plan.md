# Execution Plan

I will follow `TODO.md` as the source of truth and complete exactly the first incomplete task. I cannot record private chain-of-thought, but this file contains the complete operational plan and progress log for this invocation.

## Selected Task

First incomplete task: `P4-T01R`, "Review effective signature helper".

## Constraints

- Do not proceed to `P4-T02`; stop after this review task is completed and committed.
- Treat `P4-T01R` as a real task, not a formality.
- If the review finds an implementation gap that blocks the helper from serving later P4 tasks, fix it in this task when directly in scope; otherwise add the minimum prerequisite task in `TODO.md`, commit, and stop.
- Ensure no parallel signature-equivalence implementation is accepted.
- Mark the task complete only by prefixing its heading/status with `[DONE]` in both `TODO.md` and `TODO-4.md`.

## Step-by-Step Plan

1. Check repository state and the latest commit for any unfinished issue directly relevant to `P4-T01R`.
2. Inspect `P4-T01` completion changes and review the required design sections in `OVERLOAD_RESOLUTION.md`.
3. Review `crates/scoopc_hir/src/typecheck/overloads.rs` to confirm:
   - signature equivalence excludes return type and effect row;
   - type-parameter alpha-equivalence and `<T>` vs `<T: Any>` conflicts are handled;
   - equality uses a structural effective signature rather than pretty text alone;
   - diagnostics include both candidate locations and rendered signatures;
   - later P4 rules can reuse one helper model.
4. Review `crates/scoopc_hir/src/resolve/mod.rs` and any touched alias/type-equality call sites for consistency with the helper model.
5. Review the conflict fixtures required by `P4-T01R` and add or adjust tests only if coverage is missing.
6. If code changes are needed, implement targeted fixes and run validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, targeted overload fixtures, then full suites when code changed.
7. If only documentation/TODO records change after reviewing already-green code, run the task-required fixture validations unless no compiled-output-affecting change was made since the previous green run and the review evidence is sufficient.
8. Update `TODO-4.md` and `TODO.md` completion records for `P4-T01R`.
9. Commit all changes for this task with a descriptive message and the required co-author trailer.

## Progress Log

- Selected `P4-T01R` as the first incomplete task from `TODO.md`.
- Read the detailed `P4-T01R` requirements in `TODO-4.md`.
- Replaced the previous invocation plan with this current review-task plan before running shell commands or making implementation changes.
- Checked repository state and latest commit. Latest commit is the P4-T01 completion record and does not mention a directly relevant unfinished issue; unrelated untracked `GC_PACING.md` will be left untouched.
- Review found that effective type comparison is structural and the required fixtures exist, but function and constructor equivalence/default-arity checks are still split across parallel helper functions. I will consolidate them behind one reusable `EffectiveSignature` helper before completing the review.
- Consolidated function and constructor overload signature/default-arity checks behind a shared `EffectiveSignature` helper in `overloads.rs`.
- Validation completed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, targeted overload fixtures, `cargo test --all --all-targets`, and full `python3 tools/run_fixtures.py` all passed.
- Marked `P4-T01R` as `[DONE]` in `TODO.md` and `TODO-4.md` with a completion record.
