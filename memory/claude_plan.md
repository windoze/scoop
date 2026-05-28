# 执行计划

## 本轮范围

- 以 `TODO.md` 作为权威任务列表。
- 本轮只完成首个未完成任务 `P3-T03R`，提交后停止。
- `PLAN.md` 只在阶段级顺序、依赖或完成标准变化时更新；本轮未发现需要更新。

## 步骤

1. 阅读 `TODO.md`，确认首个标题未带 `[DONE]` 的任务。
2. 只检查与该任务直接相关的最近提交上下文。
3. 阅读 `P3-T03R` 条目、`P3-T03` 完成记录、`SPEC_FIX.md` C1、相关 lowering/typecheck 代码和 with-update fixtures。
4. 复核 enum mismatch 分支是否不再保留 original value，matching variant 行为是否不变，struct/tuple `with` 是否不受影响，failure path 是否不走 `Raise`。
5. 如发现 concrete blocker，更新 `TODO.md` 添加最小前置任务并停止；如无 blocker，继续验证。
6. 按要求运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、构建、targeted fixtures、完整 Rust 测试和完整 fixture suite。
7. 将 `P3-T03R` 在 `TODO.md` 与 `TODO-3.md` 中标记为 `[DONE]`，填写完成记录。
8. 审查 git status/diff，只暂存本任务相关变更并提交。
9. 提交后停止，不开始下一任务。

## 进度

- 已在读取任务列表前初始化计划。
- 已确认首个未完成任务为 `P3-T03R`：Review enum `with` mismatch panic。最近提交 `[P3-T03] Panic on enum with variant mismatch` 与该任务直接相关，纳入复审范围。
- 已复核 `canonical_call.rs`、`infer.rs`、`SPEC_FIX.md` C1 与 with-update fixtures。当前实现将未命中 enum variant update set 的 arm 降为 `scoop.core.panic("enum with variant mismatch")`，matching variant update 保持 payload 重建，struct/tuple 路径保持独立。
- 已通过 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo build -p scoop -p scoopc` 与 targeted enum/tuple/struct with-update fixtures。
- 已通过完整验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1556)`）。
- 已更新 `TODO.md` 和 `TODO-3.md`：将 `P3-T03R` 标记为 `[DONE]` 并记录复审范围、核心结论、验证结果和设计闭合；`PLAN.md` 未变化。
