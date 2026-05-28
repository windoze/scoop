# Execution Plan

I will follow `TODO.md` as the source of truth and complete exactly the first task whose heading is not prefixed with `[DONE]`.

1. Confirm the first incomplete task from `TODO.md` and read its detailed entry in the package TODO file.
2. Check the latest commit message for any unfinished issue directly relevant to that selected task.
3. Inspect only the compiler, cone exporter, sysroot, fixture, and documentation surfaces named by the task.
4. Implement the task without weakening the visibility model or re-exporting `internal` declarations as a compatibility workaround.
5. Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, targeted cone/export validation, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
6. If validation reveals any unscheduled failures, fix them if in scope or add the minimum prerequisite task in `TODO.md` before marking this task complete.
7. Update `TODO.md` and `TODO-3.md` by prefixing `P3-T07` with `[DONE]` and filling its completion record. Update `PLAN.md` only if phase-level sequencing or acceptance criteria change.
8. Commit all changes for this invocation with a descriptive message and the required co-author trailer, then stop.

## Milestone: Selected Task

Selected first incomplete task: `P3-T07`, "默认 visibility 改为 `internal` 并同步 sysroot / cone export". The required outcome is that declarations without a visibility modifier become cone-internal, exported sysroot APIs opt into `public`, `.cone` API export only includes explicit public declarations, and fixtures prove same-cone internal visibility plus downstream non-export/non-visibility.

## Milestone: Existing Work Audited

The worktree already contained in-scope `P3-T07` edits: `visibility_from_modifiers` now defaults to `Internal`, sysroot API declarations have explicit `public` modifiers while helper declarations remain unmodified/internal, and a default-internal cross-cone fixture exists. I tightened the new fixture expectations and added no-modifier declarations to the ScoopIR public API filter so exporter validation proves default-internal declarations are excluded from `api.scoopir`.

## Milestone: Fixture Gaps Fixed

Targeted and full fixture validation exposed missing explicit `public` modifiers in active fixture sysroot overlays, low-level `scoop.unsafe` primitives consumed by `scoop.core`, and the public reflection kind constants. I made those APIs explicit, kept `__scoop_` helper functions internal where they remain same-cone implementation details, and regenerated HIR/MIR goldens whose only change was updated sysroot declaration spans.

## Milestone: Validation Complete

Validation passed: formatting, clippy, targeted cone/export fixtures, generated API inspection, full Rust tests, spec fixture check, and full fixture suite. I marked `P3-T07` as `[DONE]` in `TODO.md` and `TODO-3.md`; next step is reviewing the final diff and committing this task's changes.
