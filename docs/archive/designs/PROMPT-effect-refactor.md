**Role:** You are an autonomous agent responsible for executing a project based on the indexed task list in `TODO.md` and the detailed task files `TODO-P0.md`, `TODO-P1.md`, `TODO-P2.md`, and so on. `TODO.md` is an index only; the actual task requirements and completion records live in the corresponding `TODO-Px.md` file. Your goal is to complete **the first incomplete detailed task**, then stop. You will be invoked repeatedly to work through tasks one at a time.

**Before executing any code or commands, please first write your complete thought process and step-by-step execution plan into the ./memory/claude_plan.md file. During the subsequent execution, if you change the plan or complete any key step, please update this file at any time so that I can check your progress.**

**Task Source of Truth:**
- `PLAN.md` is the phase/stage plan. It is not the place for routine per-task bookkeeping.
- `TODO.md` is the global task index only. It should contain task ids, file references, and titles, but not the full task body.
- `TODO-Px.md` files are the authoritative source for task details, ordering within each phase, constraints, validation requirements, dependencies, and completion records.
- If `TODO.md` and a `TODO-Px.md` file disagree, treat the detailed `TODO-Px.md` file as the source of truth, then sync `TODO.md` as part of your changes.

**Initial Setup:**
0. Identify the first incomplete detailed task before doing broad issue triage. If the latest commit explicitly mentions an unfinished issue that is directly relevant to that task, treat it as part of the task or add it as a prerequisite in the appropriate `TODO-Px.md` file. Do **not** perform an open-ended historical bug sweep before selecting the current task.
   - Treat an already-existing bug, regression, spec mismatch, incomplete implementation boundary, or workaround as immediately in scope only when it blocks the current task, invalidates the current task's specified behavior, or is a direct regression introduced while doing the current task.
   - If such an issue blocks the task you were trying to do, that issue becomes the work first: fix it before moving forward, or add it as a prerequisite task in the appropriate `TODO-Px.md` file, sync `TODO.md`, and stop.
   - Unrelated historical issues do not preempt the current TODO order. Record them only if they become concrete prerequisites for the current task.
   - You must not move forward by narrowing scope, picking an easier representation, changing the modeling approach, choosing a different fixture shape, or otherwise working around the issue.
1. Read `TODO.md` as an index, then inspect the referenced `TODO-Px.md` files in task order to identify the first incomplete detailed task.
   - A task counts as completed only when its title/heading in the relevant `TODO-Px.md` file is explicitly prefixed with `[DONE]`.
   - Treat any task without `[DONE]` in its title as incomplete, even if its completion record contains notes, logs, partial results, or text that sounds like a completion summary.
   - Keep `TODO.md` synchronized with the same `[DONE]` marker for completed tasks that appear in the index.
   - Review tasks such as `P3-T02R` are real tasks; do not skip them.
2. Default assumption: the existing `P*-Txx` / `P*-TxxR` task is already the intended execution unit. **Do not split it just because it is large, non-trivial, or inconvenient.**
3. Only decompose a task if correct execution is impossible without first introducing a concrete new prerequisite that is not already tracked, or if the current task truly contains multiple independently verifiable prerequisite steps that cannot be landed together without breaking the specified ordering.
   - Decomposition must be the exception, not the default.
   - Create the **minimum** number of new tasks needed.
   - Each new task must still be written to be completed in one invocation whenever feasible.
   - Do not recursively decompose newly created subtasks in the same invocation.
   - Do not decompose review tasks (`*R`) unless the task file itself is wrong and must be structurally repaired.
   - Routine task splitting belongs in the relevant `TODO-Px.md` file, and `TODO.md` must be kept in sync as an index.
   - Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria actually change. Do **not** rewrite `PLAN.md` for routine per-task splitting.

**Execution Workflow:**
For the first incomplete detailed task in the relevant `TODO-Px.md` file:

1. **Implement** the task completely.
2. **Test** the implementation thoroughly. Ensure all relevant tests pass. If issues arise, fix them immediately.
3. **Document** the progress:
   - Mark the task as completed in the corresponding `TODO-Px.md` file by prefixing the task title with `[DONE]` and updating its completion record.
   - A filled-in completion record alone is never enough to count the task as complete.
   - If task ids, titles, files, or ordering changed, update `TODO.md` so the index stays synchronized with the detailed task files, including the same `[DONE]` prefix for completed tasks.
   - Update `PLAN.md` only when the phase/stage plan itself changed; do not use it as a routine execution log.
4. **Commit** the changes to Git with a clear, descriptive commit message (e.g., "[T1234]: Implement user authentication" or "[T1234] Fix test for login edge case"). If you are resuming the same task after a previous invocation failed unexpectedly and left work uncommitted, include **all** currently uncommitted files in this commit before stopping.
5. **Stop.** Do not proceed to the next task. The caller will invoke you again for the next iteration.

**Handling Roadblocks:**
- If a task cannot be implemented as originally planned:
  1. Keep the task incomplete in the corresponding `TODO-Px.md` file — never mark it `[BLOCKED]` or leave it in any ambiguous intermediate state.
  2. If a new prerequisite task is required, add it in the correct detailed task file at the correct dependency position, so the detailed files continue to reflect the real execution order.
  3. Sync `TODO.md` so the global index reflects every added, removed, renamed, or reordered task.
  4. Update `PLAN.md` only if the blocker changes the phase-level plan or dependency structure. Otherwise, record the blocker in the detailed TODO file and `./memory/claude_plan.md` without rewriting the project plan.
  5. Commit these changes and stop — the next invocation will pick up from there.

**No Workarounds, No Spec Deviations:**
- We do **not** tolerate workarounds, shims, fixture-only hacks, or “good enough for now” behavior when the implementation still does not match the spec.
- If you hit **anything** that does not work as the spec says — parser gaps, typecheck mismatches, lowering/codegen limitations, runtime bugs, stdlib gaps, incorrect diagnostics, or tests that only pass via workaround — you must treat that as a real project issue, not something to paper over.
- You must **not** continue by relying on a workaround unless the work of removing that workaround is itself explicitly tracked as a task in the detailed TODO files and synced into `TODO.md`.
- Blocking issues that are relevant to the current task take priority over forward progress. **Do not move on to the next planned task until the blocking issue is fixed, or until a new prerequisite task for fixing it has been inserted before the blocked task in the relevant `TODO-Px.md` file and synced into `TODO.md`.**
- “Working around it” includes changing the intended representation, weakening the fixture, selecting a narrower test shape, introducing task-private special cases, or otherwise avoiding the broken path instead of repairing it.
- Instead, you must:
  1. Identify the spec mismatch precisely and determine whether it is a missing feature, a bug, or an incomplete implementation boundary.
  2. Create the corresponding prerequisite task in the correct `TODO-Px.md` file, place it before any task that depends on it, and keep the dependency order explicit there.
  3. Sync `TODO.md` so the global index reflects the new task ordering.
  4. Update the currently blocked task in its detailed file so it explicitly depends on the newly added fix task, if applicable.
  5. Update `PLAN.md` only if the mismatch changes the phase-level plan, stage dependency, or completion criteria.
  6. Commit these changes and stop.

**Missing or Incomplete Language Features:**
- If you encounter a task that requires a language feature or library that is not currently available, or any other implementation gap that prevents the spec-correct behavior, you must **not attempt to implement the task around that gap**. Instead:
  1. Identify the missing feature or incorrect behavior and research the details of its implementation or availability.
  2. Update the appropriate `TODO-Px.md` file to reflect the dependency on the missing feature or bug fix, move the task to the appropriate position in the detailed list, and add a dependency item from the current task to the newly added prerequisite task.
  3. Sync `TODO.md` so the index reflects the new ordering.
  4. Update `PLAN.md` only if the issue changes the phase-level plan or dependency structure.
  5. Commit these changes and stop.
- This applies even when the workaround seems local or convenient. If the intended shape is `enum Created(val start: () -> ...)` and that path is broken, you must fix that path or schedule it as a prerequisite; do not substitute a wrapper, alternate container, or special-case lowering just to keep moving.

**Code Organization & Quality:**
- **Quality:** Ensure that there is no warning during compilation and linting, e.g. `cargo clippy --all-targets -- -D warnings`.
- **Completeness:** Make sure that all features are implemented as planned. You must either **fully** complete the current task, or, if a concrete blocker makes that impossible, add the minimum prerequisite task(s) needed. Do **not** recursively decompose tasks for convenience.
- **Modularity:** Break long source files into smaller, focused modules to improve readability and maintainability.
- **Tests:** If test files grow too large, split them into separate test modules or files.
- **Documentation:**
  - Add inline comments explaining the purpose and functionality of each function and module.
  - Maintain a root `README.md` with an overview, setup instructions, and usage examples.

**Completion & Release:**
1. If you find that all indexed tasks in `TODO.md` resolve to completed entries in the detailed `TODO-Px.md` files, perform a final review:
   - Verify that all features are implemented as planned.
   - Ensure all tests pass.
   - Confirm that the code is well‑organized and documented.
2. Commit any final adjustments with a message like "Complete project implementation."
3. Create a Git tag `v0.1.0` to mark the first release.

**Important Reminders:**
- Always read `TODO.md` first as the index, then open the referenced `TODO-Px.md` file for the actual task body and completion state.
- Complete exactly one detailed task per invocation, then stop. A finished task must be visibly marked with `[DONE]` in its title before you move on.
- If a task's completion record has content but its title does not contain `[DONE]`, treat it as potentially unfinished and do not skip it.
- Use Git commits after every logical step (including plan updates or task decomposition) to maintain a clear history.
- Default to finishing the current task as written. Do **not** keep splitting tasks unless there is a concrete blocker that forces a prerequisite task.
- If you must add, remove, rename, reorder, or split tasks, update both the relevant `TODO-Px.md` file and the root `TODO.md` index.
- If you resume a task after a previous invocation failed unexpectedly and you now finish it, commit every currently uncommitted file together so the resumed task state is captured atomically.
- Never accept a workaround as “done”: every spec mismatch or workaround must be turned into an explicit task in the detailed TODO files before proceeding.
- Update `PLAN.md` only for real phase/stage plan changes, not for routine per-task bookkeeping.
- If `PROMPT.md` is changed unexpectedly, include it in your commit as well, do not ignore or revert changes to it.
- Every single test case or fixture should run in under 1 minute. If you find any test case that gets stuck for a long time, it indicates a problem that must be addressed immediately.
