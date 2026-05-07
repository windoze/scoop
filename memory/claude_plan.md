# Claude Execution Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Complete exactly the first incomplete task, then stop after committing.
- Treat a task as complete only if its heading is prefixed with `[DONE]`.
- Avoid unrelated issue triage; only address blockers directly relevant to the selected task.

## Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for directly relevant unfinished-task notes.
3. Inspect the code and fixtures needed for that task.
4. Implement the smallest spec-correct change required by the task.
5. Run the task-specific validation, then broader relevant checks if needed.
6. If a concrete blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
7. If the task is completed, update `TODO.md` by prefixing the task heading with `[DONE]` and filling the completion record.
8. Update `memory/claude_plan.md` when key steps complete or the plan changes.
9. Commit all relevant changes with a clear task-tagged message.
10. Stop without starting the next task.

## Current Status

- Initial execution plan recorded.
- `TODO.md` has been read.
- First incomplete task: `CG-T05：收口 effect-typed adapter 与 NoOutward plain ABI`.
- Required focus: NoOutward/plain callable ABI, effect-typed adapter publication, hidden-sret/aggregate return adapter, `main(args)` plain argv ABI, and residual effect/control verifier behavior.
- Latest commit checked: `CG-T04R`, no directly relevant unfinished CG-T05 note found.
- Code inspection found the concrete CG-T05 blocker: `define_effect_typed_plain_closure_adapter` rejects hidden-sret aggregate returns instead of wrapping them as `Step_F::Complete`.
- Planned edit: teach the adapter to allocate an sret result slot, call the plain entry with the sret parameter, load the aggregate result, and pass it to `refactor_build_step_complete`.
- Planned tests: add a run-pass fixture where a pure/plain closure returning a tuple is coerced to an effect-typed function value and called through the effect surface; keep existing NoOutward/main(args) coverage and run CG-T05 directed tests.
- Implementation update: `value.rs` now emits a hidden-sret call path in the effect-typed plain adapter and loads the aggregate payload before `Step_F::Complete` construction.
- Fixture update: added `tests/fixtures/run-pass/effect_typed_plain_adapter_aggregate_return_basic.scoop` plus stdout golden.
- First validation update: `refactor_llvm_no_outward_plain_abi` passed. `refactor_llvm_effect_typed_adapter` failed because the string test embedded its own forbidden text; the assertion has been rewritten to construct that text via `concat!` split literals. The new fixture failed at run time/compile-run harness, so the next step is to build/run it directly and inspect the concrete diagnostic.
- Fixture diagnostic update: direct build showed adapter layout matching compared source/materialized `TypeId`s directly and failed to find the dynamic-invoke layout for an aggregate return. The matcher now maps layout source types through `equivalent_codegen_type_id` before comparing args/return types.
- Further diagnostic update: the plain lambda entry was still declared with a scalar ABI because the lambda declaration path ignored the P5 plain ABI handoff signature and used the wrong materialized type store. Added `declare_materialized_mir_closure_fun_with_signature` and switched plain lambda ABI materialization to consume `plain.param_tys()` / `plain.return_ty()`.
- Surface-resume update: aggregate adapter fixture exposed an invalid dynamic surface-resume candidate match where continuation answer type was ignored. Candidate selection now also requires matching `answer_ty`, preventing an `Int` continuation from being projected into a tuple-return wrapper schema.
- Link update: once invalid candidates are filtered, dynamic surface-resume wrappers with no valid target are now defined as unreachable instead of left as external declarations, matching the existing static surface-resume behavior for empty target sets.
- Validation complete: ran `cargo fmt`; `cargo test -p scoopc refactor_llvm_effect_typed_adapter`; `cargo test -p scoopc refactor_llvm_no_outward_plain_abi`; new aggregate adapter run-pass fixture; `entry_main_args_int_exit_basic`; NoOutward/dynamic-entry build fixtures; effectful closure regression fixture; `cargo test -p scoopc codegen_gap_inventory`; and `cargo clippy --all-targets -- -D warnings`.
- After removing a temporary diagnostic expansion, reran `cargo fmt`, `cargo test -p scoopc refactor_llvm_effect_typed_adapter`, the new aggregate adapter fixture, and `cargo clippy --all-targets -- -D warnings` successfully.
- `TODO.md` has been updated to mark `CG-T05` as `[DONE]` with completion notes and validation commands.
- Next step: inspect git status/diff, then commit all relevant changes with a `CG-T05` message.
