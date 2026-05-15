# Claude Execution Plan

## Current Invocation

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Inspect the task requirements, dependencies, and validation instructions in `TODO.md`.
4. Implement the selected task exactly as written, without narrowing scope or using workarounds.
5. If a concrete blocker prevents correct implementation, update `TODO.md` with the minimum prerequisite task needed, keep the blocked task incomplete, commit that bookkeeping, and stop.
6. Run the relevant tests and quality checks for the changed area; fix any task-related failures.
7. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record.
8. Update this file as key steps complete or if the plan changes.
9. Commit all relevant changes with a descriptive task-tagged message, then stop without starting the next task.

## Progress Log

- Planned initial workflow before inspecting or modifying project code.
- Identified first incomplete task: `P4-T01n`, which verifies that synthetic properties on `@Intrinsic class/struct` work like ordinary class/struct properties.
- Current task plan: inspect existing property fixtures and lowering/codegen paths, add focused fixtures for getter-only, getter+setter, and getter depending on an `@Intrinsic method`, run the required validations, update `TODO.md`, then commit and stop.
- Implemented the planned direction: computed properties without backing fields are now tracked through the shared HIR lowering path, with getter reads lowered to getter calls and setter assignments lowered to synthetic setter calls. Added targeted typecheck and run-pass fixtures for intrinsic class/struct synthetic properties.
- Validation complete for `P4-T01n`: `cargo fmt --all`, targeted intrinsic property fixtures, `cargo test -p scoopc`, full typecheck fixtures, full run-pass fixtures, and `cargo clippy --all-targets -- -D warnings` all pass. `TODO.md` is updated with `[DONE]` and the completion record; next step is commit.
