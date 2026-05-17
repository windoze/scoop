# Execution Plan

I will use this file as an external plan and progress log. I cannot record hidden chain-of-thought, but I will keep the actionable plan, decisions, blockers, and completed steps current here.

## Initial Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Read the task details, dependencies, validation requirements, and nearby context in `TODO.md`; read `PLAN.md` only if phase-level context appears necessary.
4. Inspect the smallest relevant set of source files and tests for the selected task.
5. Implement the task as written, without narrowing scope or adding workarounds.
6. If a spec-correct implementation is blocked by a missing prerequisite, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
7. Run targeted tests first, then the task-required validation commands.
8. Fix any failures introduced or exposed by this task that block its specified behavior.
9. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record.
10. Commit all relevant changes with a task-specific message and stop without starting the next task.

## Progress Log

- Plan initialized before repository inspection.
- Identified first incomplete task: `P8-T05` in `TODO-4.md`, indexed from `TODO.md`.
- Latest commit is `[P8-T04] Lower operators to scalar methods`; it is directly the dependency for `P8-T05` and does not mention an extra unfinished prerequisite.
- Current execution focus: verify `codegen_mir_binary` / `codegen_mir_unary` have no callers, remove dead LLVM direct `ast::BinaryOp` / `ast::UnaryOp` codegen, update task records, validate, commit, then stop.
- Discovery: `terminator.rs` still called the MIR operator helpers and MIR lowering still produced `Rvalue::Binary` for `compareTo(...) < 0`; treating that as in-scope because it directly blocks physical deletion of `mir_body/op.rs`.
- Implemented direction: residual `compareTo` comparisons now lower to canonical `scoop.core.Int.<lt/le/gt/ge>` method calls at the MIR lowering boundary, MIR `Rvalue::Unary/Binary` variants are removed, `op.rs` helpers are moved out before deleting `op.rs`, and legacy LLVM arithmetic operator codegen entry points are removed.
- Implementation and validation complete. `TODO.md` / `TODO-4.md` now mark `P8-T05` done; `PLAN.md` was not changed because the phase plan did not change.
- Validation summary: `cargo build`, static grep checks, targeted operator/fixture checks, `cargo test --all --all-targets`, and `cargo clippy --all-targets -- -D warnings` passed. Full fixture suite is 1341/1342 with only the pre-existing `run-pass/mutable_array_ops_basic.scoop` failure.
