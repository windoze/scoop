# Claude Execution Plan

## Scope

This file records the actionable plan and progress for the current invocation. It intentionally contains concise, auditable reasoning and steps rather than private chain-of-thought.

## Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and nearby context.
4. Inspect only the code, fixtures, and documentation needed to implement that task correctly.
5. Implement the task without narrowing scope or using workaround behavior.
6. Run the task-specified validation and any focused tests needed for confidence; fix failures that are in scope.
7. Update `TODO.md` by prefixing the completed task title with `[DONE]` and adding a completion record.
8. Update this file when key steps complete or if the plan changes.
9. Review the worktree, commit all files relevant to this invocation with a clear task-tagged message, and stop without starting the next task.

## Progress

- Initialized plan for this invocation.
- Read `TODO.md`; first incomplete task is `P6-T01` (`实现 auto dependency cone 列表`).
- Checked latest commit `65fab627 [P5-T04] Generate per-cone init routines`; no explicitly unfinished issue in the commit subject blocks `P6-T01`.
- Inspected `PLAN.md` §9 and the current source cone graph/sysroot/frontend paths.
- Implementation approach: introduce the fixed default auto dependency list (`scoop.core`, `scoop.lang.string`, `scoop.collections`, `scoop.delegates`), load `scoop.unsafe` through a `scoop.core` manifest dependency, support explicit sysroot dependencies for opt-in cones, and update fixtures/tests so `scoop.thread`, `scoop.sync`, and `scoop.runtime.test` are not loaded by default.
- Implemented the auto sysroot dependency closure, session/fixture extra sysroot dependency injection, and initial fixtures; `cargo check -p scoopc` and `cargo check -p scoop` pass after formatting.
- Focused validation passed for graph/session/fixture expectation unit tests, default thread exclusion, explicit thread dependency, `typecheck`, `run-pass`, `runtime_gc`, `run_pass_cone`, `build`, and affected UMB fixture subsets. Fixed one build unit test to declare the explicit `scoop.sync` sysroot dependency it relies on.
- Full validation passed: `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `cargo run -p scoop -- test` (fixtures: ok, 1569 checks).
- Updated `TODO.md` to mark `P6-T01` done and record the implementation/validation summary. Next step is to review the worktree and commit this invocation only.
