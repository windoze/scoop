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
- Current task identified: `MIR-T06` (`建立 unified place/lvalue contract 并清理 assignment Todo`).
- Latest commit checked: `[MIR-T04] Complete MIR preclosure surfaces`; no directly relevant unfinished issue was identified.
- Next step: inspect current assignment/place lowering, HIR handoff metadata, and existing placeholder inventory entries before editing.
- Inspection complete: typed HIR already publishes local/top-level/member assignment contracts, and frontend typecheck already rejects illegal break/continue.
- Planned implementation update: remove refactor-reachable assignment/place placeholder constructors from MIR lowering by treating missing contracts/symbols as HIR-stage invariants, reject local declarations without initializer before MIR, and keep any remaining legacy-only assignment fallback explicitly classified in the inventory.
- Implementation update completed: MIR refactor assignment lowering no longer constructs missing-contract/local/member/boxed-init/unbound-local placeholders; local declarations without initializer are rejected in typecheck.
- Added validation coverage: `tests/fixtures/mir_refactor/assignment_places.scoop`, parse/typecheck diagnostics fixtures, and `refactor_mir_place_contract_*` tests.
- Validation passed: `cargo test -p scoopc --no-default-features refactor_mir_place_contract`.
- Additional validation passed: assignment fixture `dump-mir`, MIR placeholder inventory, no-Todo verifier tests, HIR preflight, and the new parse/typecheck diagnostic fixtures.
- `TODO.md` updated: `MIR-T06` is marked `[DONE]` with completion record and validation log.
- Next step: inspect git diff/status, commit all relevant changes for `MIR-T06`, then stop.
