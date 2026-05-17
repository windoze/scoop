# 当前执行计划

## 目标

本次调用只完成 `TODO.md` 中按顺序出现的第一个未完成任务。完成后更新任务记录、运行相关验证、提交 Git commit，并停止，不继续处理后续任务。

## 执行原则

- 以 `TODO.md` 为任务排序和完成状态的唯一来源。
- 只有标题显式带 `[DONE]` 的任务才视为完成。
- 不做开放式历史问题扫查；仅处理会阻塞当前任务或直接影响当前任务正确性的缺陷。
- 不用规避、降级 fixture、替代表达或专用 hack 来绕过规范不匹配。
- 如果发现当前任务依赖缺失功能或阻塞缺陷，先把最小必要前置任务写入 `TODO.md`，保持当前任务未完成，提交该任务列表更新后停止。
- 除非阶段级计划、依赖或完成标准变化，否则不更新 `PLAN.md`。

## 步骤

1. 读取 `TODO.md`，找出第一个标题未带 `[DONE]` 的任务，记录任务编号、范围、依赖和验证要求。
2. 查看最新提交信息，仅判断是否存在与当前任务直接相关的未完成问题；不做无边界历史排查。
3. 针对当前任务读取最少必要代码、fixture、文档和测试上下文。
4. 如任务可直接完成，实施最小正确变更；如发现阻塞前置问题，则更新 `TODO.md` 记录前置任务并停止实现。
5. 为当前任务补充或更新必要测试/fixture，避免只验证窄路径。
6. 运行任务指定验证命令和相关回归测试；若失败，定位并修复属于当前任务范围的问题。
7. 更新 `TODO.md`：将当前任务标题加 `[DONE]`，填写完成记录、测试结果和关键变更说明。
8. 如执行过程中计划或关键状态变化，及时更新本文件。
9. 检查 Git 状态和 diff，确认提交内容只包含本次任务相关更改以及需要一并捕获的既有未提交状态。
10. 创建描述性 Git commit。
11. 停止，不处理下一个任务。

## 当前进度

- 已创建本执行计划文件。
- 已读取 `TODO.md`，确认本次执行的第一个未完成任务为 `C1-T02：在 sysroot 添加 AnyRef / AnyValue`。
- 已检查最新提交 `26d9647d [C1-T01] Implement sealed interface markers`，未发现与 `C1-T02` 直接相关的未完成阻塞说明。
- 当前工作树除本计划文件外无其它未提交变更。
- 已在 `sysroot/scoop.core/core.scoop` 的 Root types 区域加入空 `sealed interface AnyRef` / `sealed interface AnyValue`。
- 已在 `type_env` sysroot 测试中增加真实 sysroot marker 登记、互斥关系、非 runtime supertype 记录，以及用户 generic bound 使用验证。
- 已运行验证：`cargo test -p scoopc sysroot_type_env -- --nocapture`、`cargo test -p scoopc sealed -- --nocapture`、`cargo build`、`cargo clippy --all-targets -- -D warnings` 均通过。
