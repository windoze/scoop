# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list and complete exactly the first heading not prefixed with `[DONE]`.
- Do not skip review tasks, do not split tasks unless a concrete prerequisite is required, and do not proceed to the next task after completion.
- Treat blockers only when they affect the selected task; unrelated historical issues will not preempt TODO order.

## Plan

1. Read `TODO.md` to locate the first incomplete task and its referenced details.
2. Check the latest commit only for directly relevant unfinished work tied to that selected task.
3. Read the selected task body, dependencies, validation requirements, and any referenced files.
4. Inspect the implementation and tests needed for that task before editing.
5. Implement the smallest spec-correct change, avoiding fixture-only hacks or narrowed behavior.
6. Add or update focused tests/fixtures required by the task.
7. Run the task-required validation commands and targeted checks; fix any task-relevant failures.
8. If a spec-correct implementation is blocked, insert the minimum prerequisite task in `TODO.md`, document the blocker here, commit, and stop.
9. If complete, mark the task heading `[DONE]`, update its completion record, inspect status/diff/log, commit all intended changes, and stop.

## Progress

- Current invocation initialized this plan before running build/test commands or editing project code.
- Read `TODO.md`; first incomplete task is `P2-T05` in `TODO-3.md`.
- Read `P2-T05` details. Latest commit is `[P2-T04R] Review declaration facts migration`, directly relevant as the prerequisite but not marked unfinished.
- Next step: inspect current `TypedHirEffectContracts`, `SourceSiteMigrationFacts`, and `MirLoweringFacts` paths, then migrate source-site contracts into `HirFacts` and remove fallback dual-track MIR lowering input.
- Implemented the main migration shape: `HirFacts.source_sites` now carries full source-site contracts, MIR lowering builds only from `HirFacts`, the fallback side-table APIs and `TypedHirEffectContracts` name were removed, and `cargo check -p scoopc --no-default-features` passes.
- Completed validation after fixes: HIR/typecheck/MIR fixtures pass, full `cargo test --all --all-targets --no-default-features` passes, `cargo clippy --all-targets -- -D warnings` passes, dependency gate passes, and old fallback/bridge/type-name searches in Rust/HIR outputs have no hits. The HIR detailed contract dump is now named `source_site_contracts`.
- Marked `P2-T05` as `[DONE]` in `TODO.md` and `TODO-3.md`, with completion record covering implementation scope, validation, searches, and residual risk.
