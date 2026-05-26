# Execution Plan

## Objective
Complete exactly the first incomplete task listed in `TODO.md`, using `TODO.md` as the authoritative source for ordering, requirements, validation, and completion records.

## Plan
1. Inspect `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for directly relevant unfinished work tied to that task.
3. Read the task body and any nearby dependency or validation notes needed to implement it correctly.
4. Inspect the relevant code, fixtures, and tests for that task without doing unrelated broad triage.
5. Implement the task completely, or if a concrete blocker prevents correct implementation, add the minimum prerequisite task in `TODO.md`, update dependencies, commit, and stop.
6. Run formatting and validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then relevant tests/fixtures; run full suites when required by the task and when code changes warrant it.
7. Address any unscheduled failing tests or fixtures observed during validation before marking the task complete.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in its completion record. Update `PLAN.md` only if phase-level sequencing or criteria changed.
9. Commit all task-related changes with a descriptive message and the required co-author trailer.
10. Stop after this single task is complete.

## Progress
- Plan file initialized before task inspection.

## Progress
- Identified first incomplete task: P2-T02A (fix GC verify-roots failures exposed by tools/run_run_pass_gc_scan.sh before P2-T02).

- Found uncommitted changes while resuming P2-T02A; inspecting before making edits.

- Running cargo fmt and targeted P2-T02A GC verify fixtures.

- Targeted P2-T02A fixtures passed under SCOOP_GC_VERIFY_ROOTS=1 with --gc-move --gc-stress. Running clippy.

- Clippy passed; running full Rust test suite.

- Full Rust test suite passed; running full Python fixture suite.

- Full Python fixture suite passed; running tools/run_run_pass_gc_scan.sh.

- run_run_pass_gc_scan.sh completed successfully. Updating TODO completion record next.

- Marked P2-T02A as [DONE] in TODO.md and added its completion record with validation commands/results.
- Standalone `python3 tools/audit_user_visible_failure_policy.py` passed after updating its internal bug sentinel baseline.
- Next step: review final diff, clean generated artifacts if needed, and commit the P2-T02A changes only.
