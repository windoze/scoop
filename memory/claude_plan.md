# Claude Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after that task is implemented, validated, documented in `TODO.md`, and committed.

## Step-by-Step Plan
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent git context only as needed for that task, including whether the latest commit mentions an unfinished issue directly relevant to it.
3. Inspect the files and tests relevant to the selected task.
4. Implement the smallest spec-correct change that fully satisfies the selected task.
5. Add or update focused tests/fixtures required by the task.
6. Run formatting first, then linting, then relevant and full validation as required by the task and repository policy.
7. If validation exposes unscheduled failures, fix them if in scope or add the minimum prerequisite task(s) to `TODO.md` before marking the task complete.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record.
9. Commit all intended changes with a task-scoped message.
10. Stop without starting the next task.

## Current Status
- Identified first incomplete task: `TODO-3.md` `T3-04B0`.
- Recent commit is directly relevant: `[T3-04B] Schedule source identity prerequisite`.
- Current implementation gap: P6 still uses `LlvmIntrinsicCallContract` keyed by `source_path + span` for generic concrete FQN, reflection type args, and named intrinsic metadata.
- Implementation direction: preserve MIR/LIR `SiteId` through P6 call lowering and add identity-based contract lookup from published LIR facts, replacing span lookup where source-body/MIR call lowering has a LIR-owned call-site identity.
- Implemented first structural slice: LIR facts now publish `source_call_sites` keyed by `(StableLirCallableKey, SiteId)`, with verifier/dump support; P6 MIR/effect-lowered direct calls now receive `site_id` and prefer identity-keyed exact callee roots over span contracts.
- Fixed generic intrinsic regression found by full tests: source call-site facts now publish a separate `semantic_root_fqn` from the MIR direct-call stable template key, so P6 intrinsic lowering can use the base root while exact callee binding still carries the concrete callable root.
- Added named intrinsic metadata to LIR source call-site facts by correlating MIR `SiteId` with HIR source-site contracts during LIR fact construction; this restored named runtime/method intrinsic run-pass fixtures without relying on P6 source-span lookup.
- Updated effect-lowered golden fixtures to include the new `source_call_site_contracts` dump section.
- Final validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `cargo build -p scoop -p scoopc`, `python3 tools/dependency_gate.py`, and `python3 tools/run_fixtures.py`.
- `TODO-3.md` now marks `T3-04B0` as `[DONE]`; `TODO.md` current active task now points to `T3-04B` for the next invocation.
