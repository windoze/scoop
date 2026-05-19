# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop.
- If a concrete blocker prevents completion, update `TODO.md` with the minimum required prerequisite task, commit that bookkeeping, and stop.

## Execution Steps

1. Read `TODO.md` and identify the first incomplete task.
2. Check recent git context only as needed for that selected task.
3. Inspect the relevant source, fixtures, tests, and task requirements.
4. Implement the selected task without workarounds or scope narrowing.
5. Add or update tests/fixtures required by the task.
6. Run targeted validation first, then broader validation required by the task.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
8. Update this file when key steps complete or if the plan changes.
9. Inspect git status/diff/log, then commit all intended task changes with a descriptive task-tagged message.
10. Stop after the commit.

## Current Status

- Plan initialized before repository inspection.
- First incomplete task identified: `P2-T03` (`Any as?` closed Pure function runtime cast).
- Latest commit is `[P2-T02] Emit native callable body symbols`; no explicit unfinished issue was found in the recent commit subject list.
- Initial worktree change is this plan file only.
- Relevant implementation findings:
  - Typecheck currently rejects all function runtime casts via `check_function_type_cast_boundary`.
  - Closure allocation currently writes a single `ScoopClosure` runtime descriptor.
  - Runtime `is_instance` checks do not handle `RefTypeKind::Function` targets.
- Implementation plan refined:
  - Allow only `Any as?` to closed `Pure!` function targets; keep effectful and unsupported function casts rejected.
  - Generate signature-specific closure object type descriptors and use them for closure allocation.
  - Add function-target descriptor checks to runtime ref instance checks and MIR validation/codegen support.
  - Add/adjust fixtures for positive closed Pure cast/call, signature mismatch, and effectful target rejection.
- Implementation progress:
  - Typecheck now allows `Any as?` to closed Pure function targets and still rejects effectful/open/unsupported function casts.
  - Closure descriptors now use runtime signatures (receiver/params/return) instead of a single `ScoopClosure` descriptor; open/closed Pure rows share the same runtime signature key because effect closure-ness is compile-time-only.
  - Runtime instance checks now handle function targets; MIR validation/codegen marks function runtime descriptors as supported.
  - Added run-pass and typecheck fixtures for successful cast/call, signature mismatch, and effectful target rejection.
  - Targeted fixture and failure-policy validations have passed after fixing descriptor/signature issues.
- Validation progress:
  - `cargo fmt` passed.
  - Targeted new fixtures passed.
  - `cargo run -p scoop -- test tests/fixtures/typecheck/` passed (499 checks).
  - `cargo build` passed.
  - `cargo clippy --all-targets -- -D warnings` passed.
  - `cargo test --all --all-targets` first hit the 600s timeout after many passing tests; rerun with 1200s timeout passed.
  - `cargo run -p scoop -- test` passed (1562 checks).
  - After the receiver-signature refinement, targeted run-pass, failure-policy audit, clippy, `cargo test --all --all-targets`, and full fixture suite were rerun and passed.
- TODO bookkeeping:
  - `P2-T03` is marked `[DONE]` in `TODO.md` with completion notes and validation results.
  - `TODO.md` current-status line now points to `P3-T01` as the next task.
