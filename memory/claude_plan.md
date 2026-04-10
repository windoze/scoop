## Current Objective

Complete exactly the first undone task from `TODO.md`, after checking the latest commit for any explicitly mentioned pre-existing issue. Based on prior progress summary, the intended task is `T0150b`, but the code changes were reverted while `TODO.md` / `PLAN.md` were already marked done. The immediate job is to restore the code so the repository state matches the task tracking.

## Constraints And Working Notes

- Stop after completing one task and committing it.
- Do not revert unrelated user changes.
- Keep `TODO.md` task ordering accurate.
- Treat the existing workspace as potentially dirty and verify before editing.
- Use targeted edits only; avoid repo-wide formatting churn.
- The repository has a known pre-existing `clippy` baseline with many unrelated failures. I will verify whether that remains unrelated to this task and document it if needed.

## Step-By-Step Plan

1. Inspect the latest commit message and current git status to confirm whether there is any explicitly mentioned inherited issue that must be fixed first, and to see the current workspace state.
2. Read `TODO.md` and `PLAN.md` to confirm the first incomplete task and verify whether `T0150b` is already marked done in docs while code is missing.
3. Reapply the `T0150b` implementation in the LLVM codegen path:
   - Thread `SourceMap + SourceId` through lowered-HIR LLVM emission APIs.
   - Update `MainCodegen` to carry `SourceMap` and entry source id.
   - Update nested codegen call sites and the effect trace fallback.
   - Update the build command to construct the codegen `SourceMap`.
   - Keep eager `LiteralKind::Int/String` behavior unchanged.
   - Preserve the small `runtime_abi.rs` doc-comment cleanup.
   - Restore the regression test covering multi-file `SourceMap` lowered-HIR codegen.
4. Run focused verification first:
   - `cargo test -p scoopc llvm::tests::lowered_hir_codegen_accepts_multi_file_source_map -- --exact`
   - `cargo test -p scoop commands::build::tests::build_frontend_ok_and_creates_parent_dir -- --exact`
5. Run broader validation:
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - Optionally rerun `cargo clippy --workspace --all-targets -- -D warnings` only to confirm whether failures are still pre-existing baseline issues rather than regressions from this task.
6. Reconcile planning files if necessary so they accurately reflect the completed state and any validation caveats.
7. Commit with a task-scoped message, likely `[T0150b] Wire LLVM codegen through SourceMap`, then stop.

## Progress Log

- Plan recorded before running repo inspection commands, per request.
- Checked the current worktree, latest commit subject/body, `TODO.md`, and `PLAN.md`. The latest commit did not mention an inherited issue beyond the already-completed `T0150a` work.
- Confirmed the expected mismatch from the prior handoff: `TODO.md` / `PLAN.md` already marked `T0150b` done, while the LLVM/build code had been restored to pre-task state.
- Reapplied the `T0150b` implementation in:
  - `crates/scoopc/src/llvm/mod.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/effect.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  - `crates/scoop/src/commands/build.rs`
- Restored the public lowered-HIR LLVM emission APIs to accept `&SourceMap + entry_source_id`, added single-file/build helpers that construct the required `SourceMap`, kept eager Int/String literal payload behavior unchanged, and restored the multi-file regression test.
- Focused validation passed:
  - `cargo test -p scoopc llvm::tests::lowered_hir_codegen_accepts_multi_file_source_map -- --exact`
  - `cargo test -p scoop commands::build::tests::build_frontend_ok_and_creates_parent_dir -- --exact`
- Broad validation passed:
  - `cargo test --all`
  - `cargo run -p scoop -- test` → `fixtures: ok (836)`
- Rechecked strict clippy:
  - `cargo clippy --workspace --all-targets -- -D warnings` still fails on the pre-existing repository baseline.
  - The logged failures still begin with many LLVM/inkwell deprecation errors (`ptr_type`, `ptr_sized_int_type_in_context`) across long-standing codegen files, followed by unrelated existing lints such as `private_interfaces`, `dead_code`, `large_enum_variant`, `vec_init_then_push`, and `clippy::result_large_err`.
  - I did not find evidence that the restored `T0150b` plumbing introduced a new lint category; the failure pattern matches the previously documented repo-wide baseline.
- Next step: review the final diff, then commit `[T0150b] Wire LLVM codegen through SourceMap` and stop.
