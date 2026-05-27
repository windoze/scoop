# Execution Plan

I will not record private chain-of-thought here; this file captures the actionable plan and progress updates for the current invocation.

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. After selecting the task, check the latest commit only for unfinished work directly relevant to that task.
3. Inspect the task requirements, dependencies, validation instructions, and relevant code paths.
4. Implement the task completely, or add the minimum prerequisite task in `TODO.md` if a concrete blocker prevents correct implementation.
5. Run formatting, linting, tests, and fixtures required by the task and repository policy.
6. Update `TODO.md` completion status and validation record when the task is complete.
7. Commit all relevant changes with a descriptive message and required `Co-authored-by` trailer.
8. Stop after exactly one completed task or one committed prerequisite/blocker update.

## Progress

- Reset this progress file for the current invocation before selecting or executing the repository task.
- Selected first incomplete task: P4-T01, the repository-wide residual-token grep verification from `TEST_INFRA_CLEANUP.md` section 7 step 5 with the documented historical whitelist.
- Checked the latest commit (`[P3-T07R] Review P3 residual cleanup`) and found no unfinished issue that changes the selected task.
- Ran the full removed-token residual scan over tracked files and the working tree. Non-planning, non-archive files have no matches; the only non-archive matches are the current cleanup plan/source-of-truth documents that define or record this cleanup.
- Marked P4-T01 `[DONE]` in `TODO.md` with the validation record; preparing the required commit.
