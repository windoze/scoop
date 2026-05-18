# 执行计划

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务是 `P7-B1：B-01 helper invariant 统一迁移`。
- 最新提交 `e821e873 [P7-A4] Retire B-36 spec-uncovered frontend rows` 未显示与 P7-B1 直接相关的未完成事项。
- 本文件用于记录可公开的计划、关键进展和验证结果；不会包含不可公开的内部推理细节。

## 步骤计划

1. 用 `umb-audit list --bucket B-01` 锁定当前 B-01 active rows 和文件位置。
2. 阅读 `PLAN.md` P7-B1 相关段落、`audit/strategies/B-01.md`、`audit/UMB_categories/B-01.md`、现有 LLVM codegen helper 和测试。
3. 引入或复用集中 helper：`expect_insert_block`、`expect_parent_function`、`expect_entry_block`、`expect_basic_value` 或等价形式；panic 文案包含 helper 名称和上下文。
4. 迁移所有 B-01 builder/current-function/entry-block/basic-value 读取点，删除 B-01 `UnsupportedMainBody` fallback；不新增新的 UMB fallback。
5. 同步 active inventory、retired ledger、B-01 category/strategy 文档、fixture coverage、stale count baseline。
6. 运行 P7-B1 指定验证：`cargo run -p scoopc --bin umb-audit -- list --bucket B-01`、`cargo test -p scoopc audit:: -- --nocapture`、`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`、`cargo test -p scoopc llvm -- --nocapture`；按需要追加 `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings`。
7. 完成后更新 `TODO.md`：在 P7-B1 标题前添加 `[DONE]` 并填写完成记录；仅当阶段级计划变化时更新 `PLAN.md`。
8. 检查工作区状态、差异和最近提交，提交本次任务相关全部更改。
9. 完成 P7-B1 后停止，不继续处理 P7-B2.1。

## 进度日志

- 初始化计划文件。
- 已确定当前任务为 P7-B1，并记录执行步骤。
- 已运行 `umb-audit list --bucket B-01`，确认当前 B-01 active entries 为 71，覆盖 builder insert block、parent/current function、entry block 等内部 invariant。
- 已阅读 P7-B1 计划、B-01 strategy/category、sentinel README 与 audit 测试；B-01 不需要用户 `.scoop` fixture，coverage 需要从 active sentinel 转为 retired ledger 覆盖。
- 已新增 `MainCodegen::expect_*` helper，并将 B-01 的 71 个 `UnsupportedMainBody` fallback 迁移为 helper panic / helper invariant。
- 已把 71 个 B-01 ID 追加到 `audit/UMB_retired.csv`，重新生成 `audit/UMB_inventory.csv`；当前统计为 active=1088、retired=196、B-01 active=0。
- 已同步 B-01 category/strategy/overview、spec coverage matrix、sentinel README、audit sentinel 测试和 stale count baseline。
- 已完成验证：`umb-audit list --bucket B-01`、`umb-audit diff`、`umb-audit stats`、`cargo test -p scoopc audit:: -- --nocapture`、`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`、`cargo test -p scoopc llvm -- --nocapture`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings` 均通过。
- 已将 `TODO.md` 中 P7-B1 标记为 `[DONE]` 并填写完成记录；未更新 `PLAN.md`，因为阶段级计划未变化。
