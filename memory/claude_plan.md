# Claude Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Complete exactly the first incomplete task, then stop.
- Do not perform broad unrelated triage before identifying that task.
- Keep `PLAN.md` changes limited to real phase/stage plan changes.

## Execution Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check recent Git context only for unfinished work directly relevant to that task.
3. Read the relevant source, tests, fixtures, and documentation for the selected task.
4. Implement the task directly unless a concrete spec/blocking prerequisite makes correct completion impossible.
5. If blocked by an unscheduled prerequisite, update `TODO.md` with the minimum required prerequisite task, commit that bookkeeping, and stop.
6. Run formatting first, then clippy with warnings denied, then the required tests/fixtures for the task. Use full-suite validation when code changes require it.
7. Fix any observed unscheduled test or fixture failure before marking the task done, or schedule the minimum prerequisite/follow-up before completion if the policy allows it.
8. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record with implementation and validation details.
9. Commit all intended changes with a task-tagged message.
10. Stop without starting the next task.

## Progress

- Initial plan recorded before repository inspection.
- Identified first incomplete task: `P2-T04R` in `TODO-2.md`, a review task for backend pacing parity.
- Latest commit is `[P2-T04] Add backend pacing parity`, directly relevant to the review and with no explicit unfinished issue in the subject.
- Next step is to inspect P2-T04 changes and relevant runtime tests, then fix any backend parity regression found during review.
- Static review found the intended hosted/minimal pacing path.
- A parallel validation attempt made different Cargo feature builds race on the same `gc_microbench` binary path, so non-Immix tests temporarily launched an Immix binary and failed for the wrong backend.
- Sequential reruns of `gc-minimal` and `gc-hosted` `gc_pacing_env` both passed; further Cargo feature validation must run sequentially to avoid shared target binary collisions.
- Completed validation: `cargo fmt`, clippy with warnings denied, backend-specific `gc_pacing_env` tests, minimal/hosted runtime suites, full workspace tests, spec fixture check, and full fixture suite.
- Marked `P2-T04R` as `[DONE]` in `TODO.md` and `TODO-2.md` with the review and validation record.
- Remaining step: inspect final diff/status and commit the review task changes.
