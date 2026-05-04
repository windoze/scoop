# Claude Execution Plan

## Scope

- Work through exactly one detailed task from the TODO system in this invocation.
- Use `TODO.md` only as the task index.
- Use the referenced `TODO-Px.md` file as the source of truth for task completion and requirements.
- Stop after either completing the first incomplete detailed task or committing the minimum bookkeeping for a concrete blocker.

## Execution Plan

1. Read `TODO.md` to identify indexed task order and referenced detailed TODO files.
2. Inspect the detailed `TODO-Px.md` files in index order until the first task whose heading is not prefixed with `[DONE]` is found.
3. Check the latest commit only for unfinished work directly relevant to that selected task.
4. Read the selected task body, dependencies, constraints, validation requirements, and completion record.
5. Inspect only the code and fixtures needed to implement that task correctly.
6. Implement the smallest spec-correct change needed for the selected task.
7. Add or update focused tests/fixtures required by the task.
8. Run relevant validation commands, escalating to broader checks if failures indicate task-related regressions.
9. If a concrete blocker prevents spec-correct implementation, add the minimum prerequisite task in the correct `TODO-Px.md`, sync `TODO.md`, commit that bookkeeping, and stop.
10. If implementation succeeds, mark the task heading `[DONE]` in the detailed TODO file, update its completion record, and sync `TODO.md` if the indexed entry appears there.
11. Commit all relevant changes with a descriptive task-tagged message.
12. Stop without starting the next detailed task.

## Progress Log

- Initial plan written before running repository commands.
- Selected first incomplete detailed task: `P6-T06` in `TODO-P6-part3.md`.
- Latest commit `06fe5894 [P6-T05a] Implement plain local effect-control handoff` is directly relevant and is treated as the completed prerequisite for `P6-T06`.
- Current work focus: make `NoOutward` LLVM lowering use plain ABI and make plain call sites use ordinary direct/dynamic dispatch instead of refactor `Step_F` invoke shells.
- P6-T06 targeted checks currently pass for direct NoOutward samples, but adjacent P6-T05 dynamic-call fixture fails because plain virtual calls are still rejected instead of lowered through ordinary vtable dispatch.
- Updated implementation focus: add plain callable carrier fallback/ordinary dynamic dispatch support, then update stale `effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop` assertions away from `Step_F` dynamic invoke for plain calls.
- Added an adapter fixture for an effect-typed function value backed by a NoOutward lambda. It exposes a plain-lambda ABI gap: plain callable materialization currently assumes every plain body has a HIR top-level signature.
- Next step: make plain callable layout/body support materialized MIR lambda ordinary closure ABI, then continue with adapter wrapping.
- Implemented plain callable ABI lowering, ordinary plain direct/dynamic/virtual/interface call paths, plain lambda ordinary closure ABI support, and an effect-typed plain adapter shell with fail-fast layout selection.
- Updated P6-T06 fixtures and task records. Required unit tests, targeted fixtures, and `cargo clippy --all-targets -- -D warnings` passed.
