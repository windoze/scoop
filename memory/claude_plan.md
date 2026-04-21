# 执行计划与进度记录

## 当前阶段

初始化。本文件先记录本轮的可执行计划，随后会在读取仓库状态后持续更新，不直接暴露不可审查的内部推理细节。

## 初始执行计划

1. 检查最新一次 Git 提交，确认提交说明里是否提到已知问题、回归、临时修复或未完成边界。
2. 打开并阅读 `TODO.md`，定位第一个未完成任务。
3. 打开并阅读 `PLAN.md`，核对该任务在整体计划中的上下文、依赖和优先级。
4. 如果第一个未完成任务过大，则先细分为可单次完成的子任务，并同步更新 `TODO.md` 与 `PLAN.md`，然后本轮只实现细分后的第一个子任务。
5. 实现当前目标任务；若在实现、测试或审查中发现任何既有问题、规格不匹配或实现边界缺口，则优先修复，或将其作为前置任务插入 `TODO.md` 后停止继续推进原任务。
6. 运行与修改范围相关的验证，包括格式化、测试，以及按要求执行 `cargo clippy --all-targets -- -D warnings`；若失败则继续修复直到通过，或按阻塞流程调整任务计划。
7. 完成后更新 `TODO.md`、`PLAN.md` 和本文件，记录已完成内容、验证结果及任何计划变更。
8. 使用清晰的提交信息提交本轮变更，然后停止，不继续执行下一个任务。

## 当前已知约束

- 本轮只完成 `TODO.md` 中的第一个未完成任务。
- 不能通过变通方案绕过规格或实现缺口。
- 若发现前置缺陷，必须先修复或将其显式插入 `TODO.md` 作为前置任务。
- 代码修改后需要保证无编译/检查告警。

## 进度

- 已创建计划文件，准备开始读取仓库状态。
- 已检查最新提交：`e8261ffbbb61d1a1242e76bd5b5ec55f73926bc7 [T4016T1R] Close boxed enum callable destructuring gap`。提交说明未额外挂出新的待修 issue。
- 已确认当前首个可执行未完成条目不是顶层父任务 `T4016`，而是其下一个具体子任务 `T4016T2`。
- 已阅读 `TODO.md` / `PLAN.md` / `SCOOP_TASK.md` / `sysroot/core.scoop` / `runtime/c/scoop_task.c` / lowering 与 LLVM task 相关代码，确认当前 task 语义主体仍主要驻留在 C runtime，`T4016T2` 的目标是把这部分迁回 Scoop。

## 探针结论

为避免在错误前提上直接改生产代码，先做了若干最小编译探针，验证 ordinary Scoop 定义的 generic task object model 是否已经具备实现 `T4016T2` 的必要前提。当前结论：

- `Mutex`、普通类字段、普通 enum 字段本身可用，不是 blocker。
- 但把 generic task state 直接搬到 Scoop 时出现多条真实前置缺口：
  - `class Task<T>(..., var state: __TaskState<T>)` 这类 generic rich enum / continuation 状态字段在 LLVM 路径报 `unsupported_main_body: struct field type`。
  - `T?` / `Option<T>` 或 `Option<Nominal<T>>` 槽位会在 codegen 中漏出 `TypeKind::Param(T)`，触发 `unsupported_main_body: Option<T> inner type` / `class field type`。
  - `Any?` 私有槽位方案里，`as` / `as?` 会把 `Raise<RuntimeError>` 引入 `Task.step()`；而 `is` smart-cast + generic member access 又会报 `unsupported_expr: member access（未 resolve）`。
  - generic state carrier / `Task<T>` 的普通 ctor 路径，在类型参数只通过包装状态对象暴露时，会出现 `no_matching_overload` / `class ctor call overload mismatch/ambiguous`。

## 决策

- 当前不继续硬做 `T4016T2`，因为这会迫使实现绕过 ordinary Scoop generic task-state object model 的真实缺口，违反“不得以 workaround 推进”的约束。
- 改为先在 `TODO.md` 中插入新的前置任务，精确收口上述 LLVM / typecheck 缺口；同步更新 `PLAN.md` 的顺序和说明。
- 完成这些计划更新后提交并停止，等待下一轮从新插入的前置任务开始。
