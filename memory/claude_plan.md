# 执行计划与决策摘要

说明：这里记录可审阅的执行计划、关键判断和进度更新，不包含未整理的内部推理草稿。随着仓库检查、实现和测试推进，我会持续更新本文件。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果存在更早暴露的前置缺陷，则先修复前置缺陷，再处理该任务。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交信息是否提到已知问题、遗留缺陷或需要先处理的事项。
2. 阅读 `TODO.md`，识别当前第一个未完成任务。
3. 结合 `PLAN.md`、相关代码和测试，判断该任务是否可以在本轮完整完成。
4. 如果任务过大或存在明确前置依赖：
   - 在 `PLAN.md` 中拆分为更小子任务；
   - 在 `TODO.md` 中调整顺序，使依赖关系正确；
   - 本轮只执行拆分后的第一个子任务。
5. 实现任务对应的代码改动。
6. 运行相关测试，并补充必要测试；同时检查格式、lint 与告警情况。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞原因。
8. 以清晰提交信息提交本轮改动，然后停止，不进入下一任务。

## 当前假设

- 仓库可能已有未提交修改；除非与当前任务直接冲突，否则不回退。
- 若最新提交或执行过程中暴露规范不匹配、缺失语言特性或运行时缺陷，则必须先把该问题显式纳入 `TODO.md` 并按依赖顺序处理，不能绕过。
- 若任务需要的实现边界不清晰，会先查看相关模块、测试夹具和规范文档后再落地修改。

## 待确认事项

- 最新提交是否明确提到需要优先修复的问题。
- `TODO.md` 中第一个未完成任务的范围、依赖和可测试入口。
- 当前工作树是否存在需要避让的用户修改。

## 进度更新（2026-04-17）

- 已检查工作树：当前未提交改动只有本文件本身。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务是 `T3009b0`：
  - 任务：为 escaped continuation 的 `Continuation.resume(...)` 接回 scalar/ref payload 的专用 lowering。
  - 当前依赖顺序：`T3009b0` → `T3009b0R` → `T3010b2b1b` → `T3010b2b1`。
- `PLAN.md` 已记录该任务被前移的原因：此前以为是 unified expected-context / coercion 缺口，但重新基线化后确认更前置的 blocker 是 escaped continuation `k.resume(...)` 仍落到 generic call path，报“暂不支持的 main 代码生成节点：call callee”。

## 下一步

1. 读取最新提交正文，确认是否还有未单列到 `TODO.md` 的前置问题。
2. 阅读 `TODO.md` 中 `T3009b0` 与相关 review 任务的详细描述。
3. 定向检索 `Continuation.resume`、`continuation_resume_call_sites`、state-machine emitter / 普通 call lowering 相关代码。
4. 运行最小复现或定向测试，确认当前失败形态。
5. 设计并实现 dedicated lowering，仅覆盖 scalar/ref payload；若实现中暴露更前置且未跟踪的规范缺口，则先回写 `TODO.md` / `PLAN.md` 并停止。

## 阻塞更新（2026-04-17）

- 已在生产代码中前置接通 `Continuation.resume(...)` 的 call-span dedicated lowering 原型：`codegen_call` 会优先读取 `continuation_resume_call_sites`，并走共享的 continuation payload/runtime resume helper，而不再直接回落到 generic `call callee`。
- 复跑 `tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop` 后确认，`call callee` 已不再是首个失败点，但暴露出一个更前置且此前未被 `TODO.md` 跟踪的缺口：
  - escape arm 内 `saved = Some(k)` 已执行且打印 `arm_saved`；
  - 离开 `handle` 后读取 `saved` 仍为 `None`，程序打印 `missing`；
  - 这说明 unified path 目前只把 outer-scope locals/params seed 进 effect frame，没有在 handle 完成后把被 frame 改写的 outer-scope mutable slot 写回 enclosing local。
- 该缺口会直接阻塞 escaped continuation 被保存并在后续 `k.resume(...)` 路径中读出，因此不能继续把 `T3009b0` 标记为完成。
- 已决定按流程把该 blocker 前移为新的前置任务 `T3009b0a` / `T3009b0aR`，然后停止本轮，不继续推进 `T3009b0`。
