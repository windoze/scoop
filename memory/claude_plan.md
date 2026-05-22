# 执行计划与进度

## 当前约束

- 只执行 `TODO.md` 中第一个标题未标记 `[DONE]` 的任务。
- `TODO.md` 是任务顺序、要求和完成状态的唯一来源。
- 除非阶段级计划或完成标准变化，否则不更新 `PLAN.md`。
- 本文件只记录可检查的执行计划、决策和进度，不写入内部推理细节。

## 本轮计划

1. 读取 `TODO.md`，定位第一个未完成任务。
2. 只检查与该任务直接相关的最近提交和工作区状态。
3. 阅读任务正文、完成条件和验证要求。
4. 对前置任务变更做 review；若发现阻塞问题，在本 review 内修复或登记最小前置任务。
5. 运行任务要求的验证和必要补充验证。
6. 将任务标题改为 `[DONE]`，补全 `TODO.md` / `TODO-6.md` 完成记录。
7. 提交本轮相关变更，然后停止。

## 进度记录

- 已定位当前任务：`P7-T01R`（Review LLVM entry/global LIR facts 迁移结果）。
- 最近提交为 `[P7-T01] Migrate LLVM entry globals to LIR facts`，直接属于本 review 范围。
- 复审结论：entry main selection 读取 `LirCallableFacts` + `TypeStore`；global init routines、extern/global storage physicalization 由 `LirFacts.global_init` 驱动。
- residual 搜索：`emit.rs` 中的 HIR side table 读取只剩 reachability 输入和 body/initializer scaffold；`codegen/main` 中未发现 `extern_globals` 读取，剩余 `top_level_vars` / `top_level_immutable_values` 读取均在 LIR facts 选中 root 后取 initializer/body scaffold，归属 P7-T02/P7-T03。
- 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features llvm_entry_global`；`cargo test -p scoopc llvm_entry_global`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
- 完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已用长超时重跑；entry/global/global-init 相关 fixtures 通过，仍保留 P7-T01 / P6 baseline 记录的 7 个既有非本任务失败。
- 已更新 `TODO.md` 与 `TODO-6.md`：`P7-T01R` 标记为 `[DONE]`，完成记录已填写；`PLAN.md` 未修改。
