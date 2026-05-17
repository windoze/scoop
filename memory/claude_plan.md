# 执行计划

## 当前状态

- 任务入口：`TODO.md` 是唯一任务排序与完成状态来源。
- 当前 invocation 目标：只完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，然后停止。
- 已选任务：`C0-T01：建立本轮 baseline、确认先决事实与 fixture 分类`。
- 最新提交：`127405d4 Update plan`，未直接声明与 `C0-T01` 相关的未完成问题。
- 约束：如果发现当前任务被具体前置缺陷阻塞，则只添加最小必要前置任务并提交，不绕过实现。

## 步骤

1. 读取 `TODO.md`，确定第一个未完成任务及其验证要求。
2. 查看最新提交信息，判断是否明确提到与当前任务直接相关的未完成问题。
3. 读取当前任务相关代码、测试、fixture 和文档，确认实现范围。
4. 按任务要求做最小正确实现；如果遇到阻塞性缺口，更新 `TODO.md` 记录前置任务并停止。
5. 运行与当前任务直接相关的测试；必要时运行更广验证，修复发现的当前任务相关问题。
6. 在 `TODO.md` 中给完成任务标题加 `[DONE]`，并更新完成记录。
7. 仅在阶段级计划发生变化时更新 `PLAN.md`。
8. 检查 git 状态与 diff，提交本次任务相关的所有未提交变更。
9. 停止，不继续下一个任务。

## 进度日志

- 已创建初始执行计划；下一步读取 `TODO.md` 选择第一个未完成任务。
- 已读取 `TODO.md` 并选定 `C0-T01`；下一步运行 baseline 与审计命令，只更新审计记录和任务完成记录。
- `cargo build` 已通过；下一步运行 `cargo test --all --all-targets`。
- `cargo test --all --all-targets` 已通过；下一步运行完整 fixture suite。
- `cargo run -p scoop -- test` 已通过；下一步运行 CaptureBox、atomic-ref、sysroot 名字与 audit 基线审计。
- 审计命令已完成：CaptureBox 命中与 `TODO.md` 快照一致；atomic-ref 无命中；sysroot 无 `AnyRef`/`AnyValue`/`RefCell`/`Box`/`Atomic*` 定义，fixtures 仅有本地 `Box`。
- 下一步更新 `TODO.md`：将 `C0-T01` 标记 `[DONE]` 并写入 baseline、fixture 分类与 audit 影响面。
- `TODO.md` 已更新：`C0-T01` 已标记 `[DONE]`，完成记录包含 baseline、CaptureBox 摘要、closure fixture 分类、atomic-ref 缺口、sysroot 名字确认与 audit 影响面。
- 下一步检查 git diff 并提交本次审计任务变更。
