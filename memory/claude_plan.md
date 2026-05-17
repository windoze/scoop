Execution Plan
==============

Status: selected task `C4-T01C` after reading `TODO.md`.

Plan
----
1. Read `TODO.md` and identify the first incomplete task by finding the first task heading that is not prefixed with `[DONE]`.
2. Check the latest commit only for an explicitly mentioned unfinished issue that is directly relevant to that selected task.
3. Inspect the code, fixtures, and tests directly relevant to that task, avoiding unrelated historical triage.
4. Implement the selected task as written, without narrowing scope or introducing workaround behavior.
5. If a concrete spec or implementation blocker prevents correct completion, add the minimum prerequisite task before the blocked task in `TODO.md`, update this plan, commit the bookkeeping change, and stop.
6. Run targeted validation first, then broader validation required by the task or affected area. Address failures that are in scope.
7. Mark exactly the selected task complete in `TODO.md` by prefixing its title with `[DONE]` and updating its completion record.
8. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria changed.
9. Commit all relevant uncommitted changes with a descriptive task-tagged commit message.
10. Stop after completing or blocking exactly one task.

Progress Log
------------
- Initialized plan file before running repository commands or reading task details.
- Read `TODO.md`; the first incomplete task is `C4-T01C`: add sealed interface frontend reject / accept fixtures.
- Next checks are limited to this task: latest commit relevance, sealed-interface diagnostics, existing fixture conventions, implementation of the required fixture set, validation, TODO completion record, and one task commit.
- Latest commit is `[C4-T01B] Add closure capture semantics fixtures`; it does not mention an unfinished issue directly relevant to `C4-T01C`.
- Added the initial C4-T01C fixture set under `tests/fixtures/typecheck`, including user-source bound-only rejects and sysroot overlay fixtures for sealed marker declaration-shape errors.
- Current parser lacks inline `<T: Bound>` and `BoundA + BoundB` type-bound syntax, so positive bound coverage uses existing `where T: ...` and repeated where-constraint syntax.
- Targeted validation `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck --exit-on-failure` passed with `fixtures: ok (491)`.
- Full validation `cargo run -p scoop -- test` passed with `fixtures: ok (1403)`.
- Audit grep `rg -n "sealed_interface_" tests/fixtures/typecheck crates/scoopc/src/pipeline_user_visible_failure_policy.rs` passed; it finds the new fixture markers, while audit table registration remains owned by `C4-T02`.
- Lint validation `cargo clippy --all-targets -- -D warnings` passed.
- Updated `TODO.md`: marked `C4-T01C` as `[DONE]`, updated current status, and recorded scope, decisions, validation, and plan/design closure.
