# Execution Plan

I will not record private chain-of-thought here; this file captures the actionable plan and progress updates for the current invocation.

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to the selected task.
3. Inspect the selected task requirements, dependencies, validation instructions, and relevant code paths.
4. Implement the task completely, or add the minimum prerequisite task in `TODO.md` if a concrete blocker prevents correct implementation.
5. Run formatting, linting, tests, and fixtures required by the task and repository policy.
6. Update `TODO.md` completion status and validation record when the task is complete.
7. Commit all relevant changes with a descriptive message and required `Co-authored-by` trailer.
8. Stop after exactly one completed task or one committed prerequisite/blocker update.

## Progress

- Reset this progress file for the current invocation before selecting or executing the repository task.
- Selected first incomplete task: P4-T02, `cargo metadata --format-version 1 | grep scoop_tools` must have no matches.
- Checked latest commit (`[P4-T01] Verify residual token cleanup`); it does not mention unfinished work that changes P4-T02.
- Ran the P4-T02 metadata validation and confirmed `grep scoop_tools` has no matches.
- Marked P4-T02 `[DONE]` in `TODO.md` with the validation record.
