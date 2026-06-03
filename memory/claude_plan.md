# Claude Execution Plan

## Scope

- Current invocation goal: identify and complete exactly the first incomplete task in `TODO.md`, then stop.
- Source of truth: `TODO.md` for task order, requirements, dependencies, validation, and completion records.
- `PLAN.md` will only be changed if phase-level sequencing, dependencies, assumptions, or completion criteria actually change.
- I will record concise, reviewable planning and progress here. I will not include private chain-of-thought; this file captures the actionable plan, decisions, blockers, and validation status.

## Initial Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Review the selected task details, dependencies, and validation requirements before doing broader exploration.
3. Check recent git context only as needed for the selected task, especially whether the latest commit mentions an unfinished issue directly relevant to it.
4. Inspect the relevant code, fixtures, docs, and tests for that task.
5. Implement the smallest correct change that completes the task without workarounds or spec deviations.
6. Run formatting first, then linting, then relevant/full tests as required by the task and repository policy.
7. If unscheduled failing tests or fixtures are observed, either fix them or add the minimum prerequisite/follow-up task in `TODO.md` before marking completion.
8. Mark exactly the completed task as `[DONE]` in `TODO.md` and update its completion record.
9. Update this file whenever the plan changes or a key step completes.
10. Commit all intended changes with a clear task-tagged commit message, then stop without starting the next task.

## Progress Log

- Initialized execution plan before reading `TODO.md` or running project commands.
- Identified first incomplete task: `T2-03-R：Review T2-03`.
- Latest commit is `[T2-03] Migrate LIR callable cross references`, directly relevant to the review task.
- Review objective: confirm local callable references use `LirCallableId`, cross-cone/bodyless references use `LirCallableHash`, construction resolves handles without dangling references, and remaining `StableLirCallableKey` usage is limited to stable identity sources, boundary maps, tests, diagnostics, or debug output.
- Review findings to fix before marking done:
  - `crates/scoopc_codegen_llvm` still carried an unused `current_lir_callable_key` state slot plus stable-key lookup setup; remove it so current callable context is id-only.
  - `scoopc_lir_facts::verify` allowed `LirCallableRef::Local` to fall through to declaration-style ABI checks when the local id was missing; reject dangling local ids explicitly and keep declaration fallback only for `ExternalHash`.
- Planned edits: remove the stale codegen stable-key context, tighten verifier/reachability local-ref handling, and add verifier regression tests for dangling local callable refs.
- Implemented review fixes:
  - Removed the unused codegen `current_lir_callable_key` state and the unused `LateLoweredProgram::callable_by_lir_key` lookup helper.
  - Changed verifier/reachability behavior so `LirCallableRef::Local` must resolve through the callable inventory and cannot fall back to declaration ABI symbols.
  - Added verifier regression coverage for dangling local call target refs and dangling local ABI symbol refs.
- Validation so far: `cargo fmt` passed; `cargo clippy --all-targets -- -D warnings` passed.
- Full validation passed: `cargo test --all --all-targets`; `cargo build -p scoop -p scoopc`; `python3 tools/dependency_gate.py`; `python3 tools/spec_fixtures.py check`; `python3 tools/run_fixtures.py`.
- Review confirmation searches passed: no `current_lir_callable_key` / `lir_callable_key_for_root` / `callable_by_lir_key` matches in `crates/`; no `StableLirCallableKey` matches under `crates/scoopc_codegen_llvm/src/llvm/codegen`; total `StableLirCallableKey` references under `crates` is 90, limited to identity, boundary, diagnostics/debug, and tests.
- Updated `TODO.md` to mark `T2-03-R` as `[DONE]` with the review findings, fixes, confirmations, and validation record.
