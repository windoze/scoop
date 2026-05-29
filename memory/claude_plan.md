# Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first incomplete task, then stop.
- Mark the completed task with `[DONE]`, update its completion record, validate as required, and commit all relevant changes.

## Steps
1. Read `TODO.md` to identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit only for unfinished issues directly relevant to that selected task.
3. Inspect the selected task requirements and the affected code/tests.
4. Implement the task without weakening scope or using fixture-only workarounds.
5. Run formatting first, then linting, then relevant tests; run full validation when code changes require it.
6. If validation exposes unscheduled failures, either fix them or add the minimum prerequisite/follow-up task in `TODO.md` before marking completion.
7. Update `TODO.md` completion state and record. Update `PLAN.md` only if phase-level sequencing or criteria change.
8. Commit the completed task changes with a descriptive task-tagged message.
9. Stop without starting the next task.

## Progress
- Initial plan recorded before repository commands.
- Identified first incomplete task from `TODO.md`: `P5-T05R` review of overload diagnostics audit.
- Latest commit is `[P5-T05] Audit overload diagnostics`, directly relevant as the subject of this review.
- Review focus: candidate locations, ambiguity/no-applicable reasons, forbidden internal terms, and frontend rejection before backend/codegen.
- Review found gaps to fix before completion: location helpers can fall back to file-only strings, several overload negative fixtures lack candidate-location/reason/forbidden-term assertions, and the audit script only checks a narrow fixed fixture subset.
- Implemented review fixes: dynamic overload fixture audit, `EXPECT-NOT-ERROR-TERMS`, all-line fixture directive parsing, candidate location fallback with explicit unknown markers, more specific no-applicable mapping/generic rejection reasons, and expanded overload diagnostic fixture assertions.
- Targeted validation passed so far: `python3 tools/audit_user_visible_failure_policy.py`; `python3 tools/run_fixtures.py tests/fixtures/typecheck --exit-on-failure`; `python3 tools/run_fixtures.py tests/fixtures/infer --exit-on-failure`; targeted `typecheck_multi` Phase A-C cases; targeted umbrella overload fixtures.
- Full validation passed: `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all --all-targets`; `python3 tools/spec_fixtures.py check`; `python3 tools/audit_user_visible_failure_policy.py`; `python3 tools/run_fixtures.py`.
- Updated `TODO.md` and `TODO-5.md`: `P5-T05R` is marked `[DONE]` with completion record. Next step: inspect diff/status and commit.
