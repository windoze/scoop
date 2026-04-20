# 当前执行计划

## 约束说明

- 按要求先记录计划，再执行任何仓库检查或构建命令。
- 这里记录的是可审计的执行计划与决策摘要，不包含冗长的内部推理展开。
- 本轮目标是完成 `TODO.md` 中第一个未完成任务，完成后立即停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交说明里是否提到需要先修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大或依赖缺失，拆分任务并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。

## 执行步骤

1. 阅读与当前任务相关的代码、规格、测试和计划文档。
2. 确认是否存在阻塞当前任务的规范偏差、实现缺口或历史问题。
3. 若存在阻塞：
   - 将缺口转化为新的前置任务并写入 `TODO.md`。
   - 在 `PLAN.md` 中记录依赖关系和阻塞原因。
   - 提交文档变更后停止。
4. 若不存在阻塞：
   - 实现当前任务。
   - 运行相关测试、必要的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
   - 修复所有因此暴露的问题。

## 收尾步骤

1. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
2. 在 `TODO.md` 中将本轮完成的任务标记为完成。
3. 在 `PLAN.md` 中更新当前状态、后续顺序和任何风险说明。
4. 用清晰的 Git 提交信息提交本轮所有改动。
5. 停止，不继续处理下一个任务。

## 进度记录

- 2026-04-20：已创建本计划文件，尚未开始仓库检查。
- 2026-04-20：已检查最新提交、`TODO.md`、`PLAN.md`、`ISSUES.md` 与 `with` 相关实现；当前首个未完成任务位于 `T4010`。
- 2026-04-20：已用最小 probe 复核现状：tuple `with` 与 enum `with` 都仍在 typecheck 阶段统一报 `with_update_base_not_supported`，当前实现确认为 struct-only。
- 2026-04-20：决定将原 `T4010a` 再拆为 `T4010a1`（tuple / struct+tuple 混合 copy-update）与 `T4010a2`（enum payload copy-update 语义），本轮执行 `T4010a1`。
- 2026-04-20：已完成 `T4010a1` 实现。核心改动包括：
  - 把 `with` 的 typecheck side table 从 struct-FQN map 升级为“路径前缀 -> 具体 aggregate TypeId”。
  - 让 HIR lowering 按具体值类型递归重建 tuple / struct，并保持 base 单次求值。
  - 新增 tuple nested path / type mismatch / overlapping-path fixtures，以及 typed lowering 单测。
- 2026-04-20：已验证 `cargo test -q -p scoopc lower_typed_single_source_file_expands_with_update_over_tuple_nested_paths -- --nocapture`、`cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -q -p scoop -- test --fixtures target/t4010a1-fixtures/run-pass`、`cargo run -q -p scoop -- test --fixtures tests/fixtures/hir`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
- 2026-04-20：下一项已切换为 `T4010a2`，但按本轮要求将在提交 `T4010a1` 后停止。
