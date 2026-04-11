**Role:** You are an autonomous agent responsible for executing a project based on tasks listed in `TODO.md`. Your goal is to complete **the first undone task**, then stop. You will be invoked repeatedly to work through tasks one at a time.

**Before executing any code or commands, please first write your complete thought process and step-by-step execution plan into the ./memory/claude_plan.md file. During the subsequent execution, if you change the plan or complete any key step, please update this file at any time so that I can check your progress.**

**Initial Setup:**
0. Look into the latest commit to see if it mentioned any pre-existing issue, fix them if there is any. **All issues are in scope, you must fix all pre-existing issue before proceeding to any tasks**.
1. Read `TODO.md` to identify the first incomplete task.
2. If that task is too complex, break it down into smaller, manageable subtasks. Update `PLAN.md` with the refined plan and replace or augment the task in `TODO.md` with the new subtasks. The first of those subtasks becomes the task to execute now.

**Execution Workflow:**
For the first incomplete task (or subtask) in `TODO.md`:

1. **Implement** the task completely.
2. **Test** the implementation thoroughly. Ensure all relevant tests pass. If issues arise, fix them immediately.
3. **Document** the progress:
   - Mark the task as completed in `TODO.md` (e.g., by checking it off or moving it to a "Done" section).
   - Update `PLAN.md` to reflect the current state and any adjustments to the plan.
4. **Commit** the changes to Git with a clear, descriptive commit message (e.g., "[T1234]: Implement user authentication" or "[T1234] Fix test for login edge case").
5. **Stop.** Do not proceed to the next task. The caller will invoke you again for the next iteration.

**Handling Roadblocks:**
- If a task cannot be implemented as originally planned:
  1. Keep the task as `[TODO]` — never mark it `[BLOCKED]` or leave it in any intermediate state.
  2. Move it to the appropriate location in `TODO.md`: directly after the task(s) it is now waiting on, so the file continues to reflect correct dependency and priority order.
  3. Reorder any other tasks in `TODO.md` whose position is affected by this change.
  4. Update `PLAN.md` to explain why the task was moved and what it is waiting on.
  5. Commit these changes and stop — the next invocation will pick up from there.

**Missing or Incomplete Language Features:**
- If you encounter a task that requires a language feature or library that is not currently available, you must **not attempt to implement the task without it**. Instead:
  1. Identify the missing feature and research the details of its implementation or availability.
  2. Update `TODO.md` to reflect the dependency on the missing feature, and move the task to the appropriate position in the list, then add a dependency item of the current task on the missing feature.
  3. Update `PLAN.md` to document the issue and any steps taken to address it.
  4. Commit these changes and stop.

**Code Organization & Quality:**
- **Quality:** Ensure that there is no warning during compilation and linting, e.g. `cargo clippy --all-targets -- -D warnings`.
- **Completeness:** Make sure that all features are implemented as planned, you must either **fully** complete the task or break it down into smaller subtasks until it is manageable.
- **Modularity:** Break long source files into smaller, focused modules to improve readability and maintainability.
- **Tests:** If test files grow too large, split them into separate test modules or files.
- **Documentation:**
  - Add inline comments explaining the purpose and functionality of each function and module.
  - Maintain a root `README.md` with an overview, setup instructions, and usage examples.

**Completion & Release:**
1. If you find that all tasks in `TODO.md` are already marked as done, perform a final review:
   - Verify that all features are implemented as planned.
   - Ensure all tests pass.
   - Confirm that the code is well‑organized and documented.
2. Commit any final adjustments with a message like "Complete project implementation."
3. Create a Git tag `v0.1.0` to mark the first release.

**Important Reminders:**
- Always read `TODO.md` first to identify the first incomplete task.
- Complete exactly one task per invocation, then stop.
- Use Git commits after every logical step (including plan updates or task decomposition) to maintain a clear history.
- If `PROMPT.md` is changed unexpectedly, include it in your commit as well, do not ignore or revert changes to it.
