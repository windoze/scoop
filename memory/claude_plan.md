# Claude Plan

I will not record private chain-of-thought. This file captures the execution plan and progress for the current invocation.

## Initial execution plan
1. Read TODO.md and identify the first task whose heading is not prefixed with `[DONE]`.
2. Inspect the task details, dependencies, and validation requirements, plus recent git context if directly relevant.
3. Implement the selected task completely, or add the minimum prerequisite task if a concrete blocker prevents correct implementation.
4. Run formatting, linting, tests, and fixtures required by the task policy.
5. Update TODO.md with `[DONE]` and a completion record if the task is completed; update PLAN.md only if phase-level planning changes.
6. Commit all changes for this invocation and stop without starting the next task.

## Progress
- Created initial plan file before task execution.
- Identified `P1-T01R：Review 纯 spec 决议更新` as the first incomplete task.
- Reviewed the P1-T01 diff against `SPEC_FIX.md` A1, A2, and D1: `Nothing`, cone/package layering, and value-type `with` wording are consistent with the design baseline.
- Confirmed P1-T01 did not modify compiler behavior, fixtures, or P2/P3 language-surface code blocks.
- Ran `python3 tools/spec_fixtures.py check` and `git diff --check`; both passed.
- Marked P1-T01R `[DONE]` in `TODO.md` and `TODO-1.md` with the review completion record.
# Execution Plan

I will follow `TODO.md` as the authoritative task list and complete exactly the first task whose heading is not prefixed with `[DONE]`.

1. Read `TODO.md` to identify the first incomplete task and its requirements, dependencies, and validation steps.
2. Check recent repository state only as needed for that task, including the latest commit if it explicitly points to an unfinished issue relevant to the selected task.
3. Inspect the task-related source, tests, fixtures, and documentation before changing implementation.
4. Implement the task directly without weakening the intended behavior or using workarounds.
5. Run formatting, linting, tests, and fixture validation required by the task and by repository policy, fixing any unscheduled failures encountered.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and adding a completion record. Update `PLAN.md` only if the phase-level plan changes.
7. Commit all changes for this task with a descriptive message and the required co-author trailer, then stop.

## Current Task: P1-T02 delete `@Inline` annotation surface

Task-specific plan:

1. Remove the active `@Inline` specification text from `SCOOP_FULL_SPEC.md` and the split spec files, while keeping the removed lowercase `inline` keyword diagnostic documented as obsolete syntax.
2. Remove `annotation class Inline` from `sysroot/lib/scoop.core/src/core.scoop`.
3. Remove `Inline` from builtin annotation recognition and delete/rename the dedicated inline annotation checker so `@Inline` is no longer hard-coded by typecheck.
4. Update inline-related fixtures so `@Inline` is no longer a passing surface, while preserving coverage for the removed lowercase `inline` keyword.
5. Sync spec fixtures, update any goldens affected by the sysroot surface change, run formatting/linting/fixtures, then update TODO completion records and commit.

Progress:

- Completed the implementation: active specs, sysroot, and HIR typecheck no longer define or recognize `@Inline`; parser only keeps the lowercase `inline` removed-keyword diagnostic.
- Updated obsolete inline fixtures and regenerated affected HIR / effect-lowered goldens after removing the sysroot annotation class.
- Completed validation with formatting, spec fixture sync/check, clippy, Rust tests, full fixture suite, and targeted searches.
- Marked `P1-T02` as `[DONE]` in `TODO.md` and `TODO-1.md`; next step is committing this task.
