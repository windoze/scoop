# Execution Plan

## Scope
- Follow `TODO.md` as the source of truth.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, or add the minimum prerequisite task if a concrete blocker makes completion impossible.

## Steps
1. Read `TODO.md` and inspect the latest commit message for directly relevant unfinished work.
2. Read only the files needed to understand and implement the first incomplete task.
3. Implement the task with the smallest correct code changes.
4. Add or update focused tests/fixtures required by the task.
5. Run the task-specified validation and relevant regression tests.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record, or record a blocker/prerequisite if completion is impossible.
7. Run final verification as practical for the changed area.
8. Commit all relevant changes with a task-scoped commit message.
9. Stop after the first incomplete task is completed or blocked and recorded.

## Progress
- Plan initialized before repository inspection.
- First incomplete task identified: `CG-T02R` review of runtime value primitive lowering.
- Latest commit `[CG-T02] Lower runtime type primitives in LLVM` is directly relevant and will be reviewed as part of this task.
- Review focus: MIR metadata-driven `is`/`!is`/`as`/`as?`/`!!`/pattern type-test lowering, `Raise<RuntimeError>` failure boundaries, and function-type cast diagnostics.
- Initial code review located the CG-T02 implementation in MIR lowering, refactor LLVM MIR body codegen, frontend cast diagnostics, and runtime primitive fixtures.
- Verification in progress: rerunning the CG-T02 directed tests and fixtures before final review decision.
- Review finding: `Pattern::Is` carries `RuntimePatternTypeTestMetadata`, but LLVM pattern support/codegen still read only the pattern target type. This blocks metadata-driven static-fold cases such as value-type `when` pattern tests.
- Plan update: fix pattern `is Type` support/codegen to validate and consume MIR pattern metadata, then add a targeted refactor LLVM coverage case and rerun validation.
- Fix complete: `Pattern::Is` support/codegen now validates MIR pattern metadata, honors static folds, and uses runtime descriptor lowering only for dynamic ref-like cases.
- Validation passed: MIR/LLVM primitive tests, required run-pass/typecheck fixtures, parameterized runtime match fixtures, backend inventory/gate tests, and `cargo clippy --all-targets -- -D warnings`.
- Next step: mark `CG-T02R` complete in `TODO.md` and commit the review/fix.
- `TODO.md` updated: `CG-T02R` is marked `[DONE]` with review findings, the metadata fix, and validation commands.
