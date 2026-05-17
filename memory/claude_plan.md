# Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, or add the minimum concrete prerequisite task if the task is blocked.
- Stop after committing the completed task or committed blocker/task-list update.

## Steps
1. Inspect `TODO.md` to find the first incomplete task and its validation requirements.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Read the files needed to understand and implement that task.
4. Make the smallest spec-correct implementation changes; avoid workaround behavior.
5. Add or update tests/fixtures required by the task.
6. Run the task-specific validation, plus broader checks when practical.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and recording completion details, or record a blocker/prerequisite if implementation cannot proceed.
8. Update this file after key milestones or plan changes.
9. Commit all relevant changes with a task-specific message.
10. Stop without starting the next task.

## Progress
- New invocation started on 2026-05-17. I will re-read `TODO.md`, identify the first heading without `[DONE]`, verify only directly relevant latest-commit context, implement or record a concrete blocker, run the required validation, update task records, commit the result, and stop.
- First incomplete task for this invocation: `P6-T01` in `TODO-3.md`, implementing HIR f-string desugar to `StringBuilder().add(...).toString()`. Latest commit is `[P5-T03] Move String helpers to lang string`; no directly stated unfinished issue is present.
- Implementation plan refined: lower each f-string to a block that constructs `scoop.lang.string.StringBuilder`, appends decoded text via synthesized string literals, appends expression parts through ordinary `scoop.core.ToString.toString` interface dispatch, then returns `StringBuilder.toString()`. Add synthesized string literal support where MIR/codegen currently expects string literal contents from source spans.
- Implemented the first pass of HIR desugar, synthesized string literals, ToString typecheck diagnostic, a HIR owner test, and run/typecheck fixtures. Next step is targeted compilation/tests to catch exhaustiveness, span, and ABI issues.
- Targeted Rust tests `cargo test -p scoopc fstring_desugar -- --nocapture` now pass after adjusting the HIR owner test to avoid an invalid `main` signature.
- Targeted fixtures pass: `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/fstring_*.scoop` and the new `typecheck/fstring_interpolation_non_tostring_is_error.scoop` diagnostic fixture.
- Fixed the generated ToString call shape to use member-access interface dispatch instead of a direct top-level interface function call; existing f-string fixtures now avoid the missing `scoop.core.ToString.toString` body path.
- `cargo test --all --all-targets` and `cargo clippy --all-targets -- -D warnings` pass. Full `cargo run -p scoop -- test` completes with 7 unrelated baseline failures listed for the completion record.
- `TODO.md` and `TODO-3.md` updated to mark `P6-T01` `[DONE]` and record implementation scope, decisions, validation, and the 7 remaining full-fixture baseline failures. Next step: inspect git status/diff, then commit all relevant changes for this task.
- Initial execution plan recorded before task inspection.
- First incomplete task identified: `P5-T03` in `TODO-2.md`.
- Task goal: move nine advanced `String` extension helpers from `sysroot/string.scoop` into `sysroot/lang_string.scoop`, preserve behavior, add/verify automatic-prelude visibility, validate, update completion records, and commit.
- Migrated the nine `fun String.*` helpers into `sysroot/lang_string.scoop`, removed them from `sysroot/string.scoop`, and added `lang_string_helpers_auto_prelude` visibility fixture.
- Targeted visibility fixture initially failed with `scoop::resolve::unresolved_member` for `scoop.core.String.substring`.
- Root cause was core internal helpers still calling migrated `substring`; replaced those internal calls with `unsafeSliceBytes` so `scoop.core` does not depend on `scoop.lang.string`. Targeted visibility/string fixtures now pass.
- Full fixture run then exposed comptime failures: the const gate still whitelisted old `scoop.core.*` helper FQNs, and the comptime support path loaded `stdlib/` as sysroot so auto prelude was skipped. Updated both; the `const_fun_string_methods` comptime fixture now passes.
- Full fixture baseline now has only the pre-existing `tests/fixtures/run-pass/mutable_array_ops_basic.scoop` failure recorded by P4-T02; all other targets pass.
- Rust workspace tests and `cargo clippy --all-targets -- -D warnings` pass after updating LLVM ABI assertions for the new `scoop.lang.string.substring` symbol.
- `TODO.md` and `TODO-2.md` updated to mark `P5-T03` as `[DONE]` with completion details. Next step: review diff/status, then commit this task only.
