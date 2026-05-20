# Claude Execution Plan

## Scope

- Current invocation goal: complete exactly the first incomplete task in `TODO.md`, then stop.
- Source of truth: `TODO.md` for task order, task requirements, dependencies, validation, and completion records.
- `PLAN.md` will only be updated if phase-level sequencing, dependencies, assumptions, or completion criteria change.
- This file records the auditable plan, decisions, and progress updates for this invocation. It does not contain private chain-of-thought.

## Step-by-Step Plan

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Inspect the latest commit message to see whether it explicitly mentions an unfinished issue directly relevant to that task.
3. Read the selected task body, its dependencies, validation requirements, and any nearby completion records.
4. Inspect only the relevant source, tests, fixtures, and docs needed for that task.
5. Implement the task as specified, without narrowing scope or introducing workarounds.
6. If a concrete blocker or missing prerequisite prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task in the correct order, commit that bookkeeping change, and stop.
7. Run targeted validation first, then broader required validation from the task. Address failures that are in scope.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record with implementation and validation details.
9. Review `git status`, `git diff`, and recent log before committing.
10. Commit all changes required for this completed task with a clear task-tagged commit message.
11. Stop without starting the next incomplete task.

## Progress

- Initial plan written before reading project task files or running commands.
- Read `TODO.md`; first incomplete task is `P1-T04` in `TODO-2.md`.
- Next: read the `P1-T04` task body and inspect the latest commit for directly relevant unfinished work.
- Read `TODO-2.md`; `P1-T04` requires moving stage-independent manifest/cone graph data into `scoopc_project_model`, moving `ConeId`/`ConeInfo` out of resolver ownership, preserving source-cone dependency topological order, and keeping filesystem/sysroot loaders as `scoopc` adapters.
- Latest commit is `[P1-T03R] Review types and ids migration`; its remaining `StableConeKey::from_manifest` note is directly covered by `P1-T04`, with no separate prerequisite identified yet.
- Explored cone/project code. Implementation decision: `scoopc_project_model` will own `OptLevel`, manifest parse data, `ConeSourcePackage` data, `ConeId`/`ConeInfo`, `StableConeKey`, `SourceConeGraph` data, and `SourceConeGraph::from_nodes` topo validation. `scoopc` will keep only filesystem/sysroot loader functions as adapter APIs because those depend on session/sysroot/filesystem policy.
- Next edit step: add project-model modules and dependencies, then shrink `scoopc` modules to re-export/adapt the new authoritative definitions.
- Implemented the first migration pass: added `scoopc_project_model::{opt, manifest, package, graph}`, changed `scoopc::{opt, stable_id, cone::*}` to re-export/adapt those definitions, replaced associated filesystem loader methods with `scoopc::cone::load_*` adapter functions, and updated known `resolve::ConeId` call sites to the project-model/facade cone identity path.
- Next: run formatting and targeted compilation/tests, then fix any resulting errors.
- Ran `cargo fmt` and `cargo test -p scoopc_project_model`; fixed project-model `miette` feature parity and a copied Windows-path manifest test escape. Project-model tests now pass.
- Next: run full no-default-features workspace tests and fix cross-crate compile/test failures.
- Ran `cargo test --all --all-targets --no-default-features`; fixed resolver's remaining private-field access to `ConeId` by using ordering on the public type. Full no-default-features workspace tests now pass.
- Next: run dependency gate, authoritative-definition searches, and clippy.
- Ran `cargo run -p scoop_tools -- dependency-gate`; it passed and reported `scoopc_project_model` depends only on allowed base crates (`scoopc_ids`, `scoopc_source`, `scoopc_span`).
- Ran authoritative-definition searches for `ConeId`, `ConeInfo`, `SourceConeGraph`, `SourceConeInfo`, manifest/project types, `OptLevel`, and `StableConeKey`; authoritative definitions now only appear in `scoopc_project_model`. Searches for old `resolve::ConeId` / `ConeManifest::load_from_*` / `SourceConeGraph::load_*` paths found no active Rust call sites.
- Ran `cargo clippy --all-targets -- -D warnings`; fixed one test-only adapter helper dead-code warning, then clippy passed.
- Next: update TODO completion records, inspect the diff, and commit.
- Updated `TODO.md` and `TODO-2.md` to mark `P1-T04` as `[DONE]` and recorded the implementation scope, dependency decision, validation commands, and residual P1-T05 compilation-unit facade risk.
- Ran `git diff --check`; no whitespace errors. Reviewed status, diff summary, and recent commit log before staging.
- Next: stage intended files and commit `[P1-T04] Migrate project model and cone identity`.
