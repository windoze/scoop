# 执行计划与决策摘要

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始执行步骤

1. 检查最新一次 Git 提交，确认是否提到了需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认现有计划、依赖关系和任务上下文。
4. 如果当前任务过大或存在阻塞，先把任务拆分并更新 `TODO.md` 与 `PLAN.md`，本轮只处理拆分后的第一个子任务。
5. 实现当前目标任务所需代码修改。
6. 运行与改动直接相关的测试，并补充必要测试。
7. 运行质量检查，至少包含格式、测试，以及在可行范围内执行 `cargo clippy --all-targets -- -D warnings`。
8. 更新 `TODO.md`、`PLAN.md`，记录完成情况或阻塞原因。
9. 提交 Git commit，然后停止，不继续下一个任务。

## 当前已知约束

- 需要先检查最新提交中是否有必须优先修复的问题。
- 不能用规避方案绕过规范缺口；若发现规范不匹配，必须先把问题加入 `TODO.md` 并调整依赖顺序。
- 在执行过程中，这个文件会持续更新，记录关键决策、进度和计划调整。

## 待确认信息

- 最新提交是否声明了既有问题。
- 第一个未完成任务是什么，是否存在前置依赖或需要拆分。
- 当前工作树是否有未提交改动需要避让。

## 当前结论（更新）

- 最新提交为 `[T3009b0a1c] Fix SuspendCall inactive continue path`。提交说明本身没有额外声明新的“必须先修”的既有问题；当前最直接的后续项就是其复审任务。
- `TODO.md` 中首个未完成任务是 `T3009b0a1cR`：**Review：确认 unified `SuspendCall` 的 inactive-path 已回到单一 state-machine 合同**。
- 当前工作树只有 `memory/claude_plan.md` 的计划更新，未发现其他未提交改动需要避让。

## 本轮执行计划（细化）

1. 读取 `T3009b0a1cR` 在 `TODO.md` / `PLAN.md` 中的完整描述，明确验收标准。
2. 查看最新提交 diff，确认它改动了哪些生产代码与测试。
3. 定向审查相关生产代码，重点检查：
   - `SuspendCall` inactive-path 是否仍存在旁路、旧 fallback 或 effect-only patch；
   - inactive / active 两条路径是否都由统一 state-machine 语义驱动；
   - 是否引入新的遗漏分支或与相邻 `RuntimeRaiseBoundary` 逻辑不一致的问题。
4. 运行定向测试与必要的全量质量检查；若审查发现问题，直接修复并补测。
5. 更新 `TODO.md`、`PLAN.md`、本计划文件，记录复审结论或修复内容。
6. 提交本轮变更并停止。

## 复审结果（更新）

- 已确认 `T3009b0a1c` 的实现本身没有回到 callee 名称 / 源码形状分流：`state_machine_emitter.rs` 中 `SuspendCall` 的 inactive/active 分流只按 `SuspendSiteKind` + TLS active 驱动，且 step function 会暂时清空 `current_fun_return_ty`，普通 call codegen 不会在 step function 内偷偷走 ordinary-frame propagation。
- 但在继续复审同一个 shared `UnifiedStateTerminator::Suspend` 出口时，发现两个未在 `TODO.md` 显式跟踪的更前置生产缺口：
  - direct `ObjectInitAccessBoundary` 最小复现：`handle { Config.x + 1 } with { Raise.raise(err: RuntimeError) -> 10 }` 当前只打印 `before`、`config.init`，`after` 与结果不会继续。
  - `NestedHandleBoundary` 最小复现：outer `handle` 包 inner `handle { helper(false) + 1 }` 时，当前只打印到 `inner_after`，`outer_after` 与结果不会继续。
- 这说明 shared `Suspend` terminator 仍把 `ObjectInitAccessBoundary` / `NestedHandleBoundary` 建模成“求值后无条件 suspend”。因此当前还不能把 `T3009b0a1cR` 标成完成；需要先把这两个更前置 blocker 加入 `TODO.md` 并前移顺序。

## 本轮计划调整

1. 在 `TODO.md` 中新增 `T3009b0a1d` / `T3009b0a1dR`，先修并复审 `ObjectInitAccessBoundary` 的 inactive-path。
2. 在 `TODO.md` 中新增 `T3009b0a1e` / `T3009b0a1eR`，再修并复审 `NestedHandleBoundary` 的 inactive-path。
3. 将 `T3009b0a1cR` 保持为 `[TODO]`，并移动到上述前置任务之后。
4. 更新 `PLAN.md` 记录新的 blocker、最小复现和执行顺序。
5. 提交计划重排变更并停止，等待下一轮先处理新的前置任务。
