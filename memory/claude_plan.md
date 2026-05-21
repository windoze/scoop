# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first incomplete task whose heading is not prefixed with `[DONE]`.
- Stop after completing and committing that task, or after committing a required prerequisite/blocker update if the task cannot be completed as written.

## Execution Plan

1. Read `TODO.md` to locate the first incomplete task and capture its requirements, dependencies, validation steps, and completion-record expectations.
2. Check the latest commit only for unfinished issues directly relevant to the selected task.
3. Inspect only the code, fixtures, docs, and tests needed for the selected task.
4. Implement the task as written without narrowing scope or introducing workarounds.
5. If a spec mismatch or missing prerequisite blocks correct implementation, update `TODO.md` with the minimal prerequisite task in dependency order, keep the current task incomplete, commit that bookkeeping change, and stop.
6. Run the task-specific validation required by `TODO.md`, plus any directly relevant focused tests.
7. Fix any regressions introduced while completing the task and rerun validation.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record.
9. Run final relevant checks needed for confidence.
10. Inspect git status/diff/log, then commit all intended task changes with a descriptive task-tagged message.
11. Stop without starting the next task.

## Progress Log

- Initialized plan before reading task details or running code/commands.
- Read `TODO.md`; first incomplete task is `P2-T05R` in `TODO-3.md`, a review task for source-site contract migration. Latest commit is `c94064a0 [P2-T05] Migrate source-site contracts to HirFacts`, directly relevant to the review.
- P2-T05R review found blocking issues to fix in this review: lingering `LoweredHir`-to-facts migration paths for MIR lowering helpers, effect-site placeholder fallback branches, incomplete verifier/preflight coverage for source-site contract classes, and incomplete payload/dump authority for some `HirFacts.source_sites` data.
- Revised execution plan: inspect the affected source sections, remove or narrow the legacy paths, make missing effect contracts fail before fallback MIR construction, expand `HirFacts` verification/preflight coverage, move detailed source-site dump to authoritative `HirFacts`, update fixtures/tests, then run P2-T05 validation plus the extra all-targets no-default-features test required by P2-T05R.
- Implemented P2-T05R fixes: source-site payload completeness, verifier/preflight coverage, `HirFacts`-based detailed HIR dump, MIR lowering no-fallback errors for missing effect contracts, and removal of `MirLoweringFacts::from_lowered_hir(...)` call paths.
- Regenerated HIR golden fixtures and added `with_update_struct_field` HIR fixture for non-empty with-update contract coverage.
- Validation completed successfully: `cargo fmt`; focused `scoopc_hir_facts`, `hir_preflight`, `mir_lowering_facts`, `hir_stage`, `mir_stage`; HIR/typecheck/MIR fixtures; `cargo test --all --all-targets --no-default-features`; dependency gate; `cargo tree -p scoopc_hir_facts`; old bridge keyword search; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
- Updated `TODO.md` and `TODO-3.md` to mark `P2-T05R` as `[DONE]` and record review findings, fixes, validation, and residual risk.
