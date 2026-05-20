# 执行计划

## 当前约束
- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 只处理第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 不做开放式历史问题扫描；只处理当前任务及其直接阻塞项。
- 若发现阻塞当前任务的规格/实现缺口，先在 `TODO.md` 中加入最小必要前置任务并提交后停止。

## 步骤
1. 读取 `TODO.md`，定位第一个未完成任务，并记录任务编号、要求、依赖和验证命令。
2. 查看最近提交，仅判断是否有明确提到与该任务直接相关的未完成问题。
3. 根据任务内容阅读最小必要的相关代码、文档和测试。
4. 若任务可直接完成，实施最小正确改动并添加或更新相应测试/fixture。
5. 运行任务要求的验证命令；必要时运行相关定向测试，修复失败直到通过。
6. 更新 `TODO.md`：给已完成任务标题加 `[DONE]`，并填写完成记录。
7. 仅当阶段级计划、依赖或完成标准变化时更新 `PLAN.md`。
8. 更新本文件记录关键进展与最终验证结果。
9. 检查 `git status`、`git diff` 和最近提交，确认只提交本次相关变更。
10. 使用符合仓库风格的提交信息提交所有本任务相关改动，然后停止。

## 进展记录
- 初始计划已写入，尚未读取 `TODO.md`。
- 已读取 `TODO.md` 和 `TODO-2.md`，第一个未完成任务是 `P1-T04R：Review project model 与 cone graph 迁移结果`。
- 本次任务类型为 review：需要复查 P1-T04 的迁移范围、依赖方向、cone identity 归属和 SourceConeGraph 语义；若发现阻塞项就在本 review 内修复，否则更新完成记录并提交。
- 最近提交为 `[P1-T04] Migrate project model and cone identity`，未包含明确未完成项；初始工作区只有本计划文件变更。
- 已复查 authoritative 定义搜索：`ConeId` / `ConeInfo` / `SourceConeGraph` / `SourceConeInfo` 只在 `scoopc_project_model/src/graph.rs` 定义；未发现 `resolve::ConeId` / `resolve::ConeInfo` 旧路径命中。
- 已完成验证：`cargo fmt`、`cargo test -p scoopc_project_model`、`cargo test -p scoopc --no-default-features source_cone_graph`、`cargo test --all --all-targets --no-default-features`、`cargo run -p scoop_tools -- dependency-gate`、`cargo tree -p scoopc_project_model`、authoritative 定义/旧路径搜索、`cargo clippy --all-targets -- -D warnings`。
- Review 结论：project model / cone graph 迁移满足 P1 crate DAG 与 cone DAG 语义，未发现需要代码修复或新增前置任务的阻塞项；下一步更新 TODO 完成记录并提交。
- 已更新 `TODO.md` 与 `TODO-2.md`，将 `P1-T04R` 标记为 `[DONE]` 并填写 review 完成记录；`git diff --check` 通过。
