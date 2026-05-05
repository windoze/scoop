# Claude Execution Plan

## Scope

- Follow the project task workflow exactly once for this invocation.
- Read `TODO.md` as the task index, then inspect referenced `TODO-Px.md` files in order.
- Select the first detailed task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, or if a concrete blocker prevents completion, add the minimum prerequisite task(s), sync the index, commit, and stop.

## Execution Steps

1. Inspect the current task index and detailed TODO files to identify the first incomplete detailed task.
2. Check whether the latest commit mentions an unfinished issue directly relevant to that task.
3. Read the selected task body, constraints, dependencies, validation requirements, and completion record.
4. Inspect only the relevant code, fixtures, and docs needed for that task.
5. Implement the smallest spec-correct change needed, avoiding workarounds or fixture-only hacks.
6. Add or update tests/fixtures required by the task.
7. Run the task-specific validation commands and broader relevant checks.
8. If validation exposes an in-scope blocker, fix it when feasible; otherwise record the blocker as a prerequisite task, sync `TODO.md`, commit, and stop.
9. When complete, mark the detailed task heading with `[DONE]`, update its completion record, and sync `TODO.md` if the task appears there.
10. Update this plan file after key milestones or plan changes.
11. Review the git diff, commit all relevant changes with a task-prefixed message, and stop without starting the next task.

## Current Status

- Task selected: `P7-T02W` in `TODO-P7.md`.
- Latest commit `5f0b594f [P7-T03] Fix default regression blockers and add class init prerequisite` is directly relevant because it introduced this class-init hidden ordinary effect prerequisite.
- Reproduced the failure: the fixture only prints `main_before_call`, `helper_before_ctor`, `boom.init`; `caught` is never reached.
- Root cause found: P4 facts only scan `Call` / `Perform` / `Resume` / `Handle` sites, while `Rvalue::ClassCtor` currently has no `SiteId` and contributes no hidden init effect cases. As a result `helper` and the `main` call to `helper()` are solved as `NoOutward` / `Plain`, so the outer `HandleDispatch` has no boundary to consume.
- Implementation direction: make MIR class ctor rvalues carry a stable site and hidden init effect row, publish class-ctor site facts, select/materialize a class-ctor boundary, and lower that boundary in the refactor LLVM emitter without falling back to legacy selector paths.
- Implementation completed: class ctor MIR now carries deferred `SiteId` plus hidden effect row, P4/P5/P6 publish and consume `ClassCtor` site/boundary facts, and the failing fixture reaches the catch arm under the default refactor path.
- Validation completed: targeted class ctor fixtures, new P4 facts unit test, `effect_lowered`, `llvm::codegen::effect_refactor`, `llvm::tests`, and `cargo clippy --all-targets -- -D warnings` passed.
- TODO records updated: `P7-T02W` is marked `[DONE]` in `TODO-P7.md` and synchronized in `TODO.md`.
