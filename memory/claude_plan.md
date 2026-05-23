Scoop task execution plan and progress log

Current invocation goal
- Complete exactly the first incomplete task in TODO.md, then stop.
- Treat TODO.md as authoritative for ordering, dependencies, validation, and completion records.

Execution plan
1. Read TODO.md and identify the first task whose heading is not prefixed with [DONE].
2. Check the latest commit message only for unfinished issues directly relevant to that selected task.
3. Inspect the selected task's requirements, dependencies, affected code, and nearby tests/fixtures.
4. Implement the smallest spec-correct change needed for that task; do not use workarounds or weaken fixtures.
5. Run targeted validation first, then broader validation required by the task or affected area.
6. If any unscheduled failing test or fixture is observed, fix it or add the minimum prerequisite/follow-up task in TODO.md before marking the task complete.
7. Mark the completed task heading in TODO.md with [DONE] and update its completion record.
8. Update PLAN.md only if phase-level sequencing, dependencies, assumptions, or completion criteria changed.
9. Review the diff, run final relevant checks, and commit all intended changes with a task-specific message.
10. Stop without starting the next task.

Progress log
- Created initial execution plan before reading project task details.
- Identified first incomplete task from TODO.md: P7-T04-bR, a review task for the LLVM stage handoff shape narrowing in TODO-6.md.
- Read TODO-6.md requirements for P7-T04-bR. Latest commit is df64e064 `[P7-T04-b] Narrow LLVM stage handoff`, directly matching the reviewed task and not naming an extra unfinished prerequisite.

Selected task plan: P7-T04-bR
1. Read the P7-T04-b and P7-T04-bR entries in TODO-6.md to capture the exact review scope and validation requirements.
2. Inspect the latest commit message for directly relevant unfinished work.
3. Review the P7-T04-b implementation against the task contract: LLVM handoff should use the narrowed LIR/base/codegen context shape, not broad frontend or raw MIR/HIR fallback state.
4. Search for residual handoff leaks and type-discipline regressions in codegen and related tests.
5. Fix any issues that invalidate P7-T04-bR; if a concrete prerequisite blocks review completion, record it in TODO.md instead of marking the review done.
6. Run the task-specified and relevant targeted validation, then update TODO.md and TODO-6.md completion records if the review passes.
7. Commit the review changes and stop.

Review findings and adjusted implementation plan
- Residual API search confirmed `llvm_residual_pass_view`, `LirStageContext`, and `EffectLoweredStageOutput` are gone from Rust sources.
- Remaining `HirFacts` and `MaterializedMirPassView` hits are in the explicit `LlvmStageBaseContext` / backend codegen context, matching the P7-T04-b allowance for backend-private residuals before P7-T04-c.
- Review gap found: ABI visibility handoff consistency is documented but not enforced strongly enough. `StageEmitInput::new` can accept a partial ABI visibility tuple, and `LlvmStageBaseContext::verify_lir_type_context` checks owner/wire-format but not whether LIR facts fingerprints match the base context TypeStores.
- Plan update: add narrow verifier checks for TypeStore fingerprints and ABI visibility option tuple consistency, update the stale stage handoff comment, add focused tests, then run the required validation.
- Implemented the review fix: `LlvmStageBaseContext` now verifies materialized/effect TypeStore fingerprints, `StageEmitInput::new` rejects partial ABI visibility tuples, emit verifies ABI visibility facts against their TypeStore owner, and `llvm_codegen_stage_abi_visibility_handoff_is_complete_and_verified` covers the handoff.
- Validation passed: `cargo fmt`; `cargo test -p scoopc --no-default-features llvm_codegen_stage`; `cargo test -p scoopc llvm_codegen_stage`; `cargo test -p scoopc --no-default-features pipeline::effect_lowering_stage`; `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`; `cargo test -p scoopc llvm::codegen::effect_lowered::layout`; `cargo run -p scoop_tools -- dependency-gate`; `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
- Residual search after changes: no `hir_compat_scaffold`, `llvm_residual_pass_view`, or `EffectLoweredStageOutput` Rust hits in the reviewed scopes; remaining `HirFacts` / `MaterializedMirPassView` hits are confined to `LlvmStageBaseContext` and backend-private codegen context residuals allowed until P7-T04-c.
- Updated TODO.md and TODO-6.md to mark P7-T04-bR as [DONE] with the review conclusion, fix summary, residual search classification, and validation record.
