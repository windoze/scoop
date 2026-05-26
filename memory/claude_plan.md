# Claude Plan

## Current objective
Identify the first incomplete task in TODO.md, complete only that task, validate it, update TODO.md, and commit the result.

## Execution plan
1. Read TODO.md to find the first heading not prefixed with [DONE].
2. Inspect only the files and tests relevant to that task, plus latest commit context if it directly mentions an unfinished issue relevant to the task.
3. Implement the task without workarounds or spec deviations.
4. Run formatting, linting, targeted validation, then full required validation according to TODO.md and project policy.
5. Update TODO.md completion status and record. Update PLAN.md only if phase-level plan changes.
6. Commit all task-related changes and stop.

## Progress
- Initial plan written before repository inspection.
- Selected first incomplete task: P1-T07R, review `tools/audit_pipeline_gap.py` against `crates/scoopc/src/pipeline_gap_audit.rs`.
- Latest commit is P1-T07 and is directly relevant to this review.
- Review found one small Rust/Python behavior drift in `parse_gap_heading`: empty markdown headings should be ignored instead of raising `IndexError`.
- Fixed the heading parser drift and completed validation: fmt, clippy, Python compile/audit, old Rust audit test, full Rust tests, new fixture runner, and old fixture runner all passed.
- Marked P1-T07R done in TODO.md with a completion record.
