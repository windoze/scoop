# 当前执行计划

## 目标

本轮只完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后更新任务记录、验证、提交并停止。

## 步骤

1. 读取 `TODO.md`，按现有顺序定位第一个未完成任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；只处理会阻塞当前任务的问题。
3. 阅读当前任务涉及的代码、测试、规格和说明，确认实现边界与验证要求。
4. 如发现当前任务必须依赖未跟踪的前置修复，则用最少的新前置任务更新 `TODO.md`，提交后停止。
5. 否则实现当前任务，保持改动最小且符合现有代码风格。
6. 运行当前任务要求的验证命令和必要的相关测试；如失败，修复后重跑。
7. 在 `TODO.md` 中把当前任务标题前缀改为 `[DONE]`，并更新完成记录。
8. 检查工作区差异，确保只包含本轮应提交的内容。
9. 按仓库约定提交所有本轮相关改动。
10. 停止，不继续处理下一个任务。

## 进度记录

- 已创建本计划文件，下一步读取 `TODO.md` 定位当前任务。
- 已读取 `TODO.md`，第一个未完成任务为 `P5-T03：保留 cone identity/kind 到 resolver/typecheck/codegen`。
- 下一步检查最近提交和当前实现入口：`resolve::ConeId`、`IndexedFile`、`TypeEnv`、HIR lowering setup、LLVM codegen inputs。
- 最近提交为 `af06e63f [P5-T02] Support source path dependencies`，没有额外阻塞当前任务的未完成提示。
- 已完成上下文检查：当前已有 per-source `ConeId` 和 visibility 过滤，但 `ConeKind`/stable cone identity 未贯穿 `IndexedFile`、`Index`、`TypeEnv`、HIR lowering 与 LLVM codegen context；部分 typecheck helper 仍把 `internal` 当作跨 cone 可见。
- 当前编辑计划：新增 source cone metadata handoff，扩展 resolver/typecheck 查询 API，修正 typecheck visibility helper，并把 source cone map 传到 HIR/MIR/LLVM 以供 stable identity/native ownership/init routine 查询。
- 已完成第一轮代码编辑：source cone metadata 已进入 `ProjectInput`、`IndexedFile`/`Index`、`TypeEnv`、HIR lowering、MIR materialization options 和 LLVM codegen context；typecheck type lowering / import context 已改用 resolver 的 cone-aware visibility。
- 下一步运行格式化与定向检查，按编译错误继续修正。
- 验证中发现完整 fixture suite 的 `run-pass/intrinsic_synthetic_property_basic.scoop` 暴露 source-aware stable key 迁移缺口：top-level storage/object stable key 不能使用调用点 source。已修正为按 declaration source 所属 cone 取 stable key。
- 已完成最终验证：`cargo test -p scoopc`、`cargo test -p scoop --bin scoop`、`cargo clippy --all-targets -- -D warnings`、完整 `cargo run -p scoop -- test` 均通过。
- 已更新 `TODO.md`，`P5-T03` 标记为 `[DONE]` 并写入完成记录。下一步检查 git diff/status 并提交本轮改动。
