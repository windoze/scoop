# P5-T01 Execution Plan

## Objective
Complete exactly the first incomplete task in `TODO.md`, then stop after committing the completed work.

## Current task

- First incomplete task: `P5-T01` in `TODO-5.md`.
- Goal: implement overload call-site resolution Phase A-C: candidate collection, visibility filtering, applicability filtering, and no-applicable diagnostics.
- Scope is limited to `P5-T01`; after completion, stop without starting `P5-T01R`.

## Step-by-step plan

1. Check the latest commit only for directly relevant unfinished work tied to `P5-T01`.
2. Read the relevant overload design sections and the existing call resolution/typecheck implementation.
3. Identify the current candidate collection, visibility, argument mapping, generic instantiation, member call, and constructor overload paths.
4. Implement Phase A-C behavior:
   - candidate collection order: local, member, extension, top-level, imported;
   - same-name shadowing by the first candidate-producing layer;
   - visibility filtering before applicability;
   - applicability filtering for arity, named/default/vararg mapping, argument subtyping, function type subtyping, composite variance, and `Nothing` subtyping;
   - no-applicable diagnostics that include all same-name candidates and rejection reasons.
5. Add or update targeted fixtures for local shadowing, visibility-before-applicability, and no-applicable diagnostics.
6. Run validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `python3 tools/run_fixtures.py`.
7. Address any unscheduled failing test or fixture by fixing it or scheduling the minimum required task before marking `P5-T01` complete.
8. Mark `P5-T01` as `[DONE]` in both `TODO.md` and `TODO-5.md`, and fill its completion record.
9. Commit all task-related changes with a descriptive message and the required co-author trailer.
10. Stop without starting the next task.

## Progress log

- Identified `P5-T01` as the first incomplete task.
- Updated this progress plan before implementation work.
- Checked the latest commit (`[P4-T05R] Review constructor overload definition checks`); it is the completed dependency/review task and does not mention unfinished work that changes `P5-T01` scope.
- Reviewed the P5/overload design notes and the current resolver/typecheck call paths.
- Implemented Phase A-C changes: top-level callable candidate layering now combines fun/constructor candidates per scope layer, invisible direct members no longer suppress visible extension/inherited candidates, cross-file function signatures are visibility-filtered before applicability, and no-applicable overload diagnostics now include candidate signatures, locations, and rejection reasons.
- Added targeted fixtures for local shadowing, visibility-before-applicability, and no-applicable diagnostics; focused fixture runs pass.
- Full validation passed: formatting, clippy, Rust tests, and the full fixture suite.
