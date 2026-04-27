# 执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；若在开始任务前或执行过程中发现最新提交提到的预存问题、已有回归、规格不符、实现边界缺口或测试失败，则先处理这些问题。完成一个逻辑任务后更新文档、运行验证、提交 Git，然后停止。

## 工作原则

- 全程使用中文记录和汇报。
- 不绕过规格或实现缺口；任何阻塞正确实现的问题都要修复，或作为前置任务加入 `TODO.md` 并提交后停止。
- 不回退用户已有改动；遇到脏工作区时只处理与本轮任务相关的文件。
- 保持变更范围尽量小，遵循仓库现有结构和测试习惯。
- 每个关键阶段完成后更新本文件，便于查看进度。

## 步骤

1. 检查最新 Git 提交，确认提交信息或变更中是否提示了需要先修复的既有问题。
2. 查看仓库状态，识别已有未提交改动，避免误覆盖用户工作。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 阅读 `PLAN.md` 及相关源码、测试或规格，确认该任务的真实范围和依赖。
5. 若任务过大，先把它拆分成可执行子任务，更新 `TODO.md` 和 `PLAN.md`，提交拆分结果并停止。
6. 若发现最新提交或现有实现中有更优先的预存问题，先修复该问题；若无法直接修复，则把它加入 `TODO.md` 的正确前置位置，更新 `PLAN.md`，提交后停止。
7. 实现当前第一个未完成任务，必要时添加或更新最小但充分的测试夹具或 Rust 测试。
8. 运行相关验证；若验证暴露同一任务范围内的问题，继续修复并复测。
9. 按要求更新 `TODO.md` 标记任务完成，并同步更新 `PLAN.md`。
10. 运行最终相关验证，视变更范围决定是否运行更完整的测试。
11. 检查差异，确认没有无关改动或调试残留。
12. 使用清晰的任务式提交信息提交本轮变更。
13. 停止，不继续处理下一个任务。

## 当前状态

- 已检查最新提交：提交信息为 `Update plan`，但该提交修改了 `ISSUES.md` / `ISSUES-1.md`，其中 `ISSUES.md` 记录了当前 MIR materialization 与 LLVM HIR 兼容边界的 P1/P2 问题。
- 已检查工作区：除本计划文件外暂无其它未提交改动。
- 已阅读 `TODO.md` / `PLAN.md` 的当前进度；按正常顺序下一步会进入 `T5000i2`，但最新提交记录的既有 P1 问题优先级更高。
- 本轮优先处理的既有问题：`ISSUES.md` 中第一条 P1，`scoop build` frontend 对 support/stdlib/sysroot sources 过度收集 `MonomorphKey`，并在 lowering 时把所有 `files_to_lower` 都当作 request roots。
- 已阅读 `crates/scoop/src/commands/build.rs`、`crates/scoopc/src/llvm/frontend.rs`、`crates/scoopc/src/hir/lower/mod.rs` 的相关入口，确认 production build 的问题与 `ISSUES.md` 记录一致。
- 已实现修复：
  - `BuildInput` 新增 `is_mir_request_source_index(...)` 与 `mir_request_source_paths()`；
  - build typecheck 阶段只对 request sources 调用 `check_file_exprs_with_monomorph_keys(...)`，support sources 走普通 `check_file_exprs(...)`；
  - `lower_main_hir_for_build(...)` 改用 `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)`，显式传入 request roots。
- 已新增回归：
  - `build_frontend_single_file_request_roots_exclude_stdlib_support_sources`；
  - `build_frontend_cone_request_roots_exclude_stdlib_support_sources`。
- 已更新文档：
  - `ISSUES.md` 将该 P1 标记为已修复；
  - `TODO.md` 新增完成项 `T5000i1P1`，并让 `T5000i2` 依赖它；
  - `PLAN.md` 记录本次预存问题修复与验证结果。
- 已通过验证：
  - `cargo fmt --all --check`
  - `cargo test -p scoop build_frontend_ -- --nocapture`
  - `cargo test -p scoopc mir::materialize -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1201)`）
- 下一步：检查最终 diff，确认无无关改动，然后提交本轮修复并停止。
