# Claude Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Select the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly one task, then stop.
- Do not use workarounds for missing language/runtime behavior; if a blocker is discovered, record the minimum prerequisite task in `TODO.md`, commit, and stop.

## Steps
1. Read `TODO.md` and identify the first incomplete task.
2. Check the latest commit only for unfinished work directly relevant to that task.
3. Read the files and tests needed to understand the selected task.
4. Implement the task with the smallest correct change.
5. Add or update targeted tests/fixtures required by the task.
6. Run focused validation first, then broader validation required by the task or touched area.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and adding a completion record.
8. Update this plan file when key steps complete or if the plan changes.
9. Commit all relevant changes with a task-specific commit message.
10. Stop without starting the next task.

## Current Status
- Initial execution plan recorded.
- `TODO.md` read complete; first incomplete task is `P12-T03`.
- Latest commit checked; no directly relevant unfinished issue was identified.
- `TODO-5.md` read complete; `P12-T03` requires removing `signature_only_sysroot_ast`, all `strip_*` helpers, and `is_compilable_sysroot_file` filtering.
- Implementation approach: inspect `sysroot/mod.rs` and callers, replace `collect_compilable_sysroot_files` with an unfiltered `collect_sysroot_files`, update callers/tests, then run grep/build/fixture validation.
- Code edit progress: `Sysroot` now stores full AST files only; obsolete stripping/filter helpers and old callers were removed; `frontend` now collects all sysroot files as support sources; const/comptime no longer reloads signature-only sysroot ASTs.
- `cargo fmt` run complete.
- `cargo build` passed with no warnings after removing stale `compilable_files` callers and unused const/comptime diagnostics.
- Required removed-helper grep passed with no matches.
- `cargo test -p scoopc sysroot::tests -- --nocapture` passed after normalizing `collect_sysroot_files` root paths.
- `cargo build` re-run passed with no warnings.
- First full fixture run exposed one failure: `run-pass/inherited_member_call_multi_level_chain_basic.scoop`.
- Root cause fixed: cross-`TypeStore` codegen type mapping now prefers structural/kind-compatible matches before display-name fallback, preventing a nominal class `A` from mapping to a sysroot generic type parameter `A`.
- Focused failing fixture now passes.
- Full fixture suite passed: `cargo run -p scoop -- test` -> 1339/1339 targets, 1376 checks.
- First `cargo test --all --all-targets` run exposed stale unit-test helpers and builtin nominal type mapping gaps.
- Fixed via-MIR HIR test helper to use the production frontend support-source path.
- Fixed codegen cross-`TypeStore` mapping for standard scalar nominal FQNs such as `scoop.core.Int`.
- Focused regressions passed.
- `cargo test --all --all-targets` passed: 856 tests.
- `cargo clippy --all-targets -- -D warnings` passed.
- `TODO.md` and `TODO-5.md` updated: `P12-T03` is marked `[DONE]` and has a completion record.
- Next step: inspect git status/diff/log and create the required commit.
