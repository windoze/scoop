# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative source.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not proceed to the next task after completing or blocking the current one.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check the latest commit only for unfinished work directly relevant to that task.
3. Inspect the task's referenced code, fixtures, and validation requirements.
4. Implement the smallest spec-correct change needed for the current task.
5. Add or update tests/fixtures required by the task.
6. Run the task-specific validation, then broader relevant checks if feasible.
7. If a concrete blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task and stop.
8. If the task is completed, prefix its `TODO.md` heading with `[DONE]` and update its completion record.
9. Update this plan file after major progress or any plan change.
10. Commit all relevant changes with a descriptive task-tagged message.

## Progress

- Initial plan recorded before running repository commands.
- New invocation started on 2026-05-06: re-read `TODO.md`; `MIR-T06` is already marked `[DONE]`, and the first incomplete task is `MIR-T07`.
- Verified git state before starting: prior `MIR-T06` work is already committed; latest commit `1fcdb1aa Update plan` did not mention an unfinished issue relevant to `MIR-T07`.
- Current task identified: `MIR-T07` (`收口 call/ctor/default/named/intrinsic typed call-site contract`).
- Execution plan for `MIR-T07`: inspect existing typed HIR call-site metadata, MIR call/ctor/intrinsic lowering, materializer call metadata, placeholder inventory, and current call fixtures; implement the smallest spec-correct call-site contract closure; add `refactor_mir_call_contract` coverage and `mir_refactor/call_contracts.scoop`; run the required dump and targeted tests; then mark `MIR-T07` `[DONE]`, update completion records, commit, and stop.
- Inspection update: typed HIR already publishes `TypedCallSiteContract` with selected direct/member/extension/ctor/intrinsic/function-value provenance and canonical arg binding, but refactor MIR currently only consumes dispatch/resume plus the older top-level binding side table. Implementation will make refactor call lowering consume the typed contract first, lower `nameOf<T>()` / `sizeOf<T>()` as explicit value primitives, lower constructor calls from selected ctor contracts, and keep the old call/ctor/sizeOf Todo constructors classified as legacy-only inventory entries.
- Implementation completed: refactor MIR call lowering now consumes typed call-site contracts for direct/member/extension/constructor/closure/fun-value/FunPtr/dispatch/intrinsic sites; `nameOf<T>()` lowers to `TypeMetadataLiteral`, `sizeOf<T>()` lowers to `SizeOf`, constructor calls use selected ctor contracts, and MIR-T07 placeholder inventory entries are legacy-only.
- Added `tests/fixtures/mir_refactor/call_contracts.scoop`, `refactor_mir_call_contract_lowers_typed_call_sites`, and upgraded reflection/getPlatform preflight samples to MIR smoke.
- Validation completed: `refactor_mir_call_contract`, call-contract `dump-mir`, `refactor_mir_placeholder_inventory`, `refactor_mir_no_todo`, `refactor_hir_preflight`, `refactor_hir_call_contracts_record_callable_provenance`, and `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings` passed.
- `TODO.md` updated: `MIR-T07` is marked `[DONE]` with completion record. Next step: inspect git diff/status, commit `MIR-T07`, then stop.

## Current Invocation: MIR-T07R

This section records the actionable plan and progress for the current invocation. It intentionally contains a concise, reviewable rationale and step-by-step plan rather than hidden chain-of-thought.

## Current Objective

- Complete exactly the first incomplete task in `TODO.md`, then stop.
- First incomplete task identified: `MIR-T07R` (`Review MIR-T07 typed call-site contract`).
- Treat `TODO.md` as the source of truth for task order, requirements, dependencies, validation, and completion records.
- Update this file whenever the plan changes or a key step is completed.

## Initial Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for directly relevant unfinished work tied to that task.
3. Read the task body and any referenced files/specs needed to understand its requirements.
4. Implement the task without narrowing scope or introducing fixture-only workarounds.
5. Add or update the smallest relevant tests/fixtures required by the task.
6. Run the task-specified validation commands, plus focused checks needed for changed code.
7. If the task is completed, prefix its `TODO.md` heading with `[DONE]` and update its completion record.
8. If a concrete blocker prevents correct implementation, add the minimum prerequisite task to `TODO.md`, leave the current task incomplete, commit that bookkeeping, and stop.
9. Commit all relevant changes with a clear task-tagged commit message.
10. Stop without starting the next task.

## Progress Log

- Plan file initialized before reading project task details.
- Read `TODO.md` and identified `MIR-T07R` as the first task without `[DONE]` in its heading.
- Latest commit was `[MIR-T07] Close typed call-site MIR gaps`; it did not advertise unfinished work that changes `MIR-T07R` scope.
- Reviewed the typed HIR call contract collector, refactor typed MIR handoff, MIR call lowering, fixture assertions, and placeholder inventory entries.
- Validation passed: `cargo test -p scoopc --no-default-features refactor_mir_call_contract`.
- Validation passed: `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/call_contracts.scoop`.
- Extra checks passed: `refactor_hir_call_contracts_record_callable_provenance`, `refactor_hir_preflight`, `refactor_mir_no_todo`, and `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`.
- Search audit found the MIR-T07 forbidden reason strings only in legacy fallback code, inventory/preflight audit lists, and tests; the refactor production dump did not contain them.
- Marked `MIR-T07R` as `[DONE]` in `TODO.md` and added the completion record.

## Task-Specific Plan: MIR-T07R

1. Check the latest commit message for unfinished work directly relevant to `MIR-T07R`.
2. Inspect the `MIR-T07` implementation and related call-site contract code paths.
3. Re-run all `MIR-T07` validation commands from `TODO.md`.
4. Inspect the MIR dump for `tests/fixtures/mir_refactor/call_contracts.scoop` to confirm metadata is sufficient for codegen consumers.
5. Search for `call callee lowering pending`, `ctor call lowering pending`, and `sizeOf intrinsic requires one positional arg` and verify they are not reachable from refactor production MIR.
6. If review passes, mark `MIR-T07R` `[DONE]` in `TODO.md`, add a completion record, commit the changes, and stop.
7. If review finds a gap, keep `MIR-T07R` incomplete, update `TODO.md` to route the fix back to `MIR-T07`, commit the bookkeeping, and stop.
