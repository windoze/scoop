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
