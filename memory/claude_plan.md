# 执行计划

## 当前状态

- 已读取 `TODO.md` 和 `TODO-2.md`。
- 第一个未完成任务是 `P1-T05：固定 cone-level compilation unit facade API`。
- 最近提交为 `[P1-T04R] Review project model migration`，未直接声明与 `P1-T05` 相关的未完成阻塞项。
- 已检查 `frontend.rs`、`pipeline/ast_stage.rs`、`pipeline/mod.rs`、driver/session/HIR/MIR/LLVM 入口和 `scoopc_project_model::graph`。
- 发现当前主要缺口：`ProjectInput` 内部字段/注释仍将 graph 扁平化 sources 描述为“当前编译单元”，`AstStageOutput` 仍是单文件正式 handoff 形状，HIR/MIR lowering 仍直接从 flatten sources 推导 request/source cone metadata。
- 已完成第一轮实现编辑：`scoopc_project_model` 新增 `SourceConeCompilationUnit` 视图；`pipeline` 新增 cone-level `AstCompilationUnitOutput`；`frontend` 将 flatten source 字段改为 build-closure source view，并新增 compilation-unit / consumer-unit API 和测试。
- 已完成验证：`cargo fmt --check`、`cargo test -p scoopc_project_model`、`cargo test -p scoopc --no-default-features frontend`、`cargo test -p scoopc --no-default-features pipeline`、`cargo test --all --all-targets --no-default-features`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo clippy --all-targets -- -D warnings` 均通过；按要求搜索 compilation-unit 相关关键词，剩余 `AstStageOutput` 命中为单文件 worker/dump helper 与 cone-level wrapper 内部成员。
- 已更新 `TODO.md` 和 `TODO-2.md`：`P1-T05` 标记为 `[DONE]` 并填写完成记录。

## 步骤计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未以 `[DONE]` 标记的任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题。
3. 阅读 `frontend.rs`、`pipeline/ast_stage.rs`、`pipeline/mod.rs`、driver/session/HIR/MIR/codegen 相关入口，确认现有 flatten build-closure 输入与 source-cone metadata 流向。
4. 设计并实现最小的 cone-level compilation unit facade/API：明确 build graph、cone unit、source file 三者边界；提供拓扑顺序遍历和 consumer 定位；让 AST handoff 能表达 cone unit。
5. 调整 HIR/MIR/codegen metadata 来源，使公开路径来自 cone unit / project model API，避免把 whole-build flatten 暴露为 compilation unit。
6. 增加覆盖 synthetic consumer 单文件、多文件同 cone 稳定遍历、dependency-before-consumer 的测试。
7. 运行 `cargo fmt`、P1-T05 指定测试和必要补充验证；修复直接相关问题。
8. 在 `TODO.md` 与 `TODO-2.md` 中标记 `P1-T05` 为 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
9. 检查 `git status`、`git diff`、最近提交，提交本次任务涉及的所有更改。
10. 完成一个任务后停止，不继续处理下一项。

## 约束

- 不绕过规格不一致或实现缺口；若阻塞当前任务，则添加最小必要前置任务并停止。
- 不修改或回滚无关的用户更改。
- 后续每完成关键步骤或调整计划都会更新本文件。
