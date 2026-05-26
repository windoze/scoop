# Claude Execution Plan

## Scope

- Authoritative task source: `TODO.md`, with detailed task body in `TODO-7.md`.
- First incomplete task identified for this invocation: `P10-T07` / "P10 全包清场、文档同步与依赖审计".
- Complete exactly this task, mark it `[DONE]`, commit the changes, then stop.

## Step-by-Step Plan

1. Check the latest commit message for any explicitly unfinished issue directly relevant to `P10-T07`.
2. Inspect current git status so existing user changes are not overwritten or accidentally reverted.
3. Audit the required P10 final boundaries with targeted searches:
   - `inject_cone_dependency_public_api` only remains as an allowed compatibility/removal-boundary path, if at all.
   - frontend has no cross-cone flattened source loop that reloads upstream dependency sources for downstream cones.
   - `scoop` driver has no in-process whole-DAG fallback outside subprocess `build-single-cone` / `link-cone` dispatch.
   - whole-build SHA fingerprint paths are documented or removed/deprecated in favor of per-cone/link fingerprints.
4. Inspect relevant docs (`PLAN.md`, `PIPELINE_REFACTOR.md`, `PIPELINE-CLEANUP.md`, `README.md`, `SCOOP_RUNTIME.md`, and fixture docs if present) and update them to freeze the per-cone multiprocessing model, document P11 as the future generic-body wire-format milestone, and mark legacy whole-closure fingerprinting as deprecated.
5. If the audit finds a concrete implementation residual that blocks the task, fix it directly; if it requires a new prerequisite that cannot be completed here, insert the minimum prerequisite task before `P10-T07`, commit, and stop.
6. Run validation in required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo run -p scoop_tools -- dependency-gate`, `cargo test --all --all-targets`, `cargo run -p scoop -- test`, and `git diff --check`.
7. Address any failing test or fixture unless it is already explicitly scheduled for repair.
8. Mark `P10-T07` as `[DONE]` in both `TODO.md` and `TODO-7.md`, update the completion record with audit findings and validation commands, and avoid updating `PLAN.md` except for actual design boundary documentation required by the task.
9. Review status/diff/log, commit all intended task files with a `P10-T07` message, and stop.

## Progress Log

- 2026-05-26: 本次调用重新读取 `TODO.md` 与 `TODO-7.md`，确认首个未完成任务仍为 `P10-T07`。继续沿用本文件中的执行计划：先检查最新提交与工作区，再完成 P10 final cleanup 审计、必要修复、文档同步、验证、TODO 完成记录与提交。
- 2026-05-26: 本次调用的最新提交检查完成：`[P10-T06-d] Record completion state` 未包含直接相关的未完成阻塞。工作区已有 P10-T07 范围内未提交改动，将作为恢复中的任务状态继续审计并纳入最终提交。
- 2026-05-26: Read `TODO.md` and `TODO-7.md`; selected `P10-T07` as the first incomplete task.
- 2026-05-26: Plan refreshed before implementation or validation commands. Next step is checking latest commit relevance and worktree status.
- 2026-05-26: Latest commit is `[P10-T06-d] Record completion state`; it does not mention an unfinished issue directly relevant to `P10-T07`. Initial worktree status only shows this plan file modified.
- 2026-05-26: Boundary audit found stale P10 comments/naming in `scheduler.rs` and stale LIR `TypeId` stable-wire-format facts still marked `Deferred`; both are in scope for P10 final cleanup and will be fixed before documentation sync.
- 2026-05-26: Implemented cleanup: scheduler now names dispatch as artifact-cone dispatch, virtual-cone helpers are crate-private, LIR type-context facts now publish `Implemented` portable TypeStore wire format, and wire schema version is bumped to 1.2 so stale artifacts miss cleanly.
- 2026-05-26: Additional code audit fixed remaining current-path wire-format residuals: LLVM handoff docs now describe implemented portable `TypeStore` serialization, `LirTypeContextFacts::default()` no longer defaults to `Deferred`, and a duplicated scheduler comment was cleaned up.
- 2026-05-26: Documentation updated in `PLAN.md`, `PIPELINE_REFACTOR.md`, `PIPELINE-CLEANUP.md`, `README.md`, `SCOOP_RUNTIME.md`, and archive fixture README to freeze P10 facade/per-cone boundaries and P11 generic-body follow-up scope. Next step is validation, then TODO completion records.
- 2026-05-26: First clippy run failed because two virtual-cone helper wrappers became dead code after visibility narrowing. The unused context wrapper was removed and the test-only input wrapper is now `#[cfg(test)]`; validation restarts from `cargo fmt`.
- 2026-05-26: Full Rust test exposed stale generated virtual-cone artifacts with old schema under fixture build dirs. `virtual_cone::materialize_single_file` now removes the generated virtual cone root before re-materializing it, so schema bumps cannot reuse stale artifacts. Validation restarts from `cargo fmt`.
- 2026-05-26: Current validation restarted. `cargo fmt` completed successfully after the final cleanup patches.
- 2026-05-26: `cargo clippy --all-targets -- -D warnings` completed successfully.
- 2026-05-26: `cargo run -p scoop_tools -- dependency-gate` completed successfully.
- 2026-05-26: `cargo test --all --all-targets` failed in `commands::run::tests::run_builds_and_executes_minimal_main` because the `scoop` test binary could not locate a `scoopc` subprocess binary. This is a P10 facade subprocess-boundary blocker and will be fixed before validation continues.
- 2026-05-26: Added a test-only `scoopc` subprocess binary bootstrap for the `scoop run` integration-style unit test so `cargo test --all --all-targets` does not depend on an external prebuilt `target/debug/scoopc`.
- 2026-05-26: Validation restarted after test bootstrap fix. `cargo fmt` completed successfully.
- 2026-05-26: Re-run `cargo clippy --all-targets -- -D warnings` completed successfully after the test bootstrap fix.
- 2026-05-26: Re-run `cargo run -p scoop_tools -- dependency-gate` completed successfully after the test bootstrap fix.
- 2026-05-26: Re-run `cargo test --all --all-targets` completed successfully after the test bootstrap fix.
- 2026-05-26: `cargo run -p scoop -- test` completed with 10 failing fixture targets. Need inspect full saved output, fix failures or schedule explicit prerequisite before marking `P10-T07` done.
- 2026-05-26: Fixture failures were all effect-lowered golden mismatches caused by the intentional `wire_format=implemented` / `P10 TypeStore portable serialization` stable dump change. Updated the 10 affected `.effectlowered` golden files.
- 2026-05-26: Re-run `cargo run -p scoop -- test` completed successfully: 1507 targets / 1536 checks.
- 2026-05-26: Extra P10 boundary audits completed: `cargo metadata --format-version 1` confirms `scoop` only depends on internal `scoop_project_model`; `crates/scoop/src` has no forbidden compiler/linker residuals; current Rust sources no longer contain active stable-wire-format deferred residuals.
- 2026-05-26: Marked `P10-T07` as `[DONE]` in `TODO.md` and `TODO-7.md` with completion records. `git diff --check` completed successfully.
