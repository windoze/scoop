## 本轮执行计划（初始）

### 目标
- 按照仓库要求，先检查最新提交是否提到遗留问题；若有，优先修复。
- 然后读取 `TODO.md`，定位第一个未完成任务，只完成这一个任务并停止。

### 思路摘要
- 先收集上下文：最新提交信息、`TODO.md`、`PLAN.md`、工作区状态。
- 判断第一个未完成任务是否过大；若过大，先在 `PLAN.md` / `TODO.md` 中拆分为更小子任务。
- 结合相关代码与测试实现该任务，避免任何规避性做法；若发现规范不匹配或前置能力缺失，则先把阻塞项转成更靠前的任务。
- 运行必要的格式化、测试与 lint，确保变更可验证。
- 更新 `TODO.md`、`PLAN.md` 与本文件，最后提交 git commit，然后停止。

### 分步计划
1. 检查最新提交信息，确认是否存在提交中提到但尚未解决的遗留问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务及其上下文。
3. 评估任务规模与依赖：
   - 若可直接完成，继续实现。
   - 若过大或被前置缺陷阻塞，先更新 `TODO.md` / `PLAN.md` 进行拆分或重排。
4. 阅读相关代码、测试与规范，确定需要修改的模块。
5. 实现任务并补充/调整测试。
6. 运行格式化、测试、lint，修复发现的问题直到通过。
7. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 记录完成情况。
8. 使用清晰的提交信息提交本轮变更。
9. 停止，不继续处理下一个任务。

### 执行约束
- 全程使用中文记录与说明。
- 不接受 workaround；若遇到规范缺口或实现边界，必须先在 `TODO.md` / `PLAN.md` 记录为前置任务。
- 只完成一个任务。

## 当前进展

### 已完成
- 已检查最新提交 `470582e75874f25ef952d352c02e20f627d0cdcb`，提交信息与 diff 主要是对 continuation / Task 过渡注释与计划文档的对齐，没有发现一个独立于当前任务序列之外、需要在进入 `TODO.md` 前额外抢修的新遗留问题。
- 已阅读 `TODO.md` 与 `PLAN.md`，确认 `T4016` 已经拆分；当前首个可执行的未完成子任务是 `T4016b`：把 answer type、returning-resume 与 `-> resume` 移除接入 parser / typecheck / HIR / lowering 主线。
- 已进一步盘点实现现状：
  - parser / AST / HIR / LLVM 仍把 `-> resume` immediate-resume 作为显式 arm kind；
  - typecheck 仍把 `Continuation.resume(...)` 钉死成 `Unit`；
  - runtime ABI 仍是 `void scoop_continuation_resume(void*)`，因此“真正 answer-returning 的 `resume`”不能与“删除 `-> resume` 语法”在同一小步里稳妥落地。
- 基于上述依赖，已把 `T4016b` 重新拆分为：
  - `T4016b1`：删除用户态 `-> resume`，把 tail-resume 收口为内部 lowering / codegen 分类；
  - `T4016b2`：把 continuation answer type 接入 binder 静态模型与显式 surface；
  - `T4016c`：提供 runtime / ABI answer-return 通道；
  - `T4016b3`：基于统一 answer-return 通道完成 `Continuation.resume(...): Answer` 主线接入。

### 下一步
- 先提交任务拆分与依赖重排。
- 之后执行新的首个子任务 `T4016b1`：
  1. 删除 parser / AST / HIR / resolver 对 `-> resume` 的公开语义支持。
  2. 把旧 immediate-resume fixtures 改写为 `, k ->` + `k.resume(...)`。
  3. 在 lowering / codegen 内部改为基于 escape-continuation arm 的 tail `k.resume(...)` 形状做内部分类。
  4. 运行相关测试、更新 `TODO.md` / `PLAN.md` / 本文件并提交。
