Execution plan
==============

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for an unfinished issue directly relevant to that selected task.
3. Inspect only the files needed to understand and implement that task.
4. Implement the task as written, without substituting narrower fixture shapes or other workarounds.
5. Run formatting, linting, and relevant validation in the requested order; run the full suites when required by the kind of change.
6. If a blocking prerequisite or unscheduled failing test/fixture is discovered, update `TODO.md` with the minimum required prerequisite task, commit that bookkeeping, and stop.
7. If the task is completed, mark its TODO heading with `[DONE]`, update its completion record, commit all related changes with the required co-author trailer, and stop.

Progress
========

- Plan initialized before task execution.
- Selected first incomplete task: `P4-T02R` in `TODO-4.md`.
- Reviewed the latest `P4-T02` implementation scope and found no immediate issue requiring a prerequisite task.
- Validation passed for `P4-T02R`; task records updated to mark the review complete.
- Removed the validation-created `a.out` artifact and prepared the completion commit.
