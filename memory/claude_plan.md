# Claude Plan

## 可公开执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断第一个未完成任务，并只处理该任务。
2. 查看最近提交和当前工作区状态，确认是否存在与该任务直接相关的未完成事项或未提交更改。
3. 针对选中的任务阅读必要的代码、测试、规格和任务说明，避免做开放式历史问题清扫。
4. 如果任务可直接完成，进行最小正确实现，并在关键步骤后更新本文件。
5. 运行任务要求的验证以及相关测试；若发现与当前任务直接相关的阻塞问题，优先修复，或在 `TODO.md` 中插入最小必要前置任务后停止。
6. 完成后将该任务标题加上 `[DONE]`，更新其完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
7. 检查差异，提交所有本次任务相关更改，然后停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md`；第一个未完成任务是 `P3-T02R`，目标是 review MIR root inventory 迁移结果。
- 已读取 `TODO-4.md` 中的 `P3-T02R` 任务详情。最新提交 `1cf86387 [P3-T02] Migrate MIR root inventories to MirFacts` 与当前 review 直接相关。
- 当前工作区仅有本进度文件修改。已复查 `mir_stage.rs`、`scoopc_mir_facts`、`hir_preflight.rs` 与 MIR fixture：`MirStageOutput` 只保留 `mir_facts`，root 查询方法通过 facts 中的 item reference 定位 MIR item；`MirFacts.roots` 覆盖 callable/initializer/extern global/metadata 四类 root；旧字段名搜索无命中。
- `P3-T02R` 验证已通过：`cargo fmt`、`cargo test -p scoopc_mir_facts`、`cargo test -p scoopc --no-default-features mir_stage`、`cargo test -p scoopc --no-default-features hir_preflight`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered`、`cargo clippy --all-targets -- -D warnings`、旧字段名搜索、`git diff --check`。
- 已更新 `TODO.md` 和 `TODO-4.md`，将 `P3-T02R` 标记为 `[DONE]` 并填写 review 完成记录。
- 下一步检查最终 diff / git 状态 / 最近提交记录，然后提交本 review 任务相关更改并停止。
