## Current Invocation Plan

Note: This file records the actionable execution plan and progress. It intentionally does not include private chain-of-thought.

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check recent git context only as needed to determine whether the latest commit mentions an unfinished issue directly relevant to that task.
3. Inspect the task requirements, dependencies, and validation instructions from `TODO.md`.
4. Implement the first incomplete task exactly as written, without narrowing scope or using workarounds.
5. If a concrete blocker prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
6. Run the relevant validation commands, including broader checks if required by the task or if changes affect shared behavior.
7. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in its completion record.
8. Update this plan file at key progress points.
9. Commit all relevant changes with a task-specific commit message.
10. Stop after exactly one task is completed or after committing a required blocker/prerequisite update.

## Progress

- Initial plan recorded.
- Identified first incomplete `TODO.md` task: `HIR-T10`, assignment LHS / HIR place contract.
- Next step: inspect recent git context and existing assignment/typecheck/HIR lowering code relevant to `HIR-T10`.
- Recent commit is `[HIR-T09] Add with-update HIR contracts`; no unfinished latest-commit issue blocks `HIR-T10`.
- Current implementation already typechecks assignment statements for local `var`, top-level `var`, and ordinary mutable member fields; parser/typecheck do not currently accept index setter or safe-member setter assignment forms.
- Implementation plan refined: add typecheck assignment-place side table, convert it to a HIR-level `AssignPlaceContract` with HIR symbol/member identities, require contracts in the refactor HIR verifier, expose them in stable typed HIR contracts, and make refactor MIR assignment lowering consume the place contract instead of deriving the place semantics from arbitrary LHS expression shape.
- Implemented assignment-place contracts across typecheck, HIR lowering, refactor HIR verifier, typed HIR stable dump, and refactor MIR lowering.
- Verified with `cargo test -p scoopc --no-default-features refactor_hir_places`, `refactor_hir_no_todo`, `refactor_hir_placeholder_inventory`, `refactor_typed_hir`, `cargo test -p scoop --no-default-features dump_hir`, and `cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`.
- Next step: mark `HIR-T10` done in `TODO.md` and commit the completed task.
