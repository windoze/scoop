# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop after committing.
- Do not proceed to later tasks.

## Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check recent Git context only as needed to see whether the latest commit mentions an unfinished issue directly relevant to that task.
3. Read the task details, dependencies, and validation requirements.
4. Inspect the relevant implementation and tests for that task.
5. Implement the smallest correct change that satisfies the task without workarounds or spec deviations.
6. Add or update focused tests/fixtures required by the task.
7. Run the task-required validation commands, plus any targeted tests needed during debugging.
8. If a concrete blocker prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task, leave the current task incomplete, commit that bookkeeping, and stop.
9. If the task is completed, update `TODO.md` by prefixing the task heading with `[DONE]` and filling the completion record.
10. Update this file when key steps complete or if the plan changes.
11. Inspect Git status/diff/log, stage only intended changes, commit with a task-tagged message, and stop.

## Progress

- Started by recording this execution plan before reading project task files or running commands.
- Read `TODO.md`; the first incomplete task is `P2-T04R` in `TODO-3.md`.
- Read `TODO-3.md` task details and latest commits. Latest commit is `[P2-T04] Migrate declaration facts to HirFacts`, directly relevant but not explicitly marked unfinished.
- Review found that `ProgramFacts` is deleted and `ExprFactResolver` uses `HirFacts`, but some declaration/entity queries still derive from `LoweredHir` side tables. Plan adjusted to fix those review findings before marking `P2-T04R` done.
- Applied targeted fixes: MIR lowering now derives member/nominal/enum facts from `HirFacts`; LLVM effect nominal and top-level/extern type queries use `HirFacts`; HIR stage now publishes dispatch/interface table facts.
- Targeted tests exposed duplicate callable fact identities for overloaded functions. Fixed callable fact identity keys to include source position while preserving display names.
- Regenerated HIR golden fixtures to reflect published dispatch/interface fact counts. Targeted `hir_stage`, `mir_stage`, `effect`, and HIR fixture validations passed after fixes.
- Full validation passed: `cargo test --all --all-targets --no-default-features`, `cargo run -p scoop_tools -- dependency-gate`, `cargo tree -p scoopc_hir_facts`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`, and required targeted tests/searches.
- Marked `P2-T04R` as `[DONE]` in `TODO.md` and `TODO-3.md` with review findings, fixes, validation commands, and residual risks.
