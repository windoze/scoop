# 执行计划记录

## 说明

按要求，先记录当前的执行思路摘要与步骤计划。这里记录的是可审阅的分析摘要、决策依据与执行步骤，不包含逐字内部推理。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果遇到前置缺陷或规范缺口，先把该问题转化为新的待办并调整顺序，然后提交并停止。

## 初始步骤计划

1. 检查最新一次 Git 提交，确认提交说明中是否提到已有问题、遗留修复或已知缺陷。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与任务顺序是否一致。
4. 评估该任务是否过大：
   - 如果可直接完成，则进入实现。
   - 如果过大，则先把任务拆分为更小的子任务，更新 `PLAN.md` 与 `TODO.md`，然后执行第一个子任务。
5. 在实现前检查相关代码、测试与文档，确认是否存在阻塞当前任务的已有缺陷或规范不匹配。
6. 实现当前任务或其首个子任务。
7. 运行相关测试，并补充必要测试；如果存在 lint / 编译告警，修复到通过为止。
8. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
9. 提交本轮改动，提交后停止，不继续处理下一个任务。

## 执行中的更新规则

- 如果发现最新提交提到的遗留问题，先修复这些问题，再继续任务流程。
- 如果发现实现与规范不一致，不能绕过，必须先把缺陷加入 `TODO.md` 并调整依赖顺序。
- 每完成一个关键步骤后，更新本文件记录当前状态与下一步动作。

## 进度更新（2026-04-17）

### 已完成

1. 检查了最新一次提交 `33922195d0b6ec7db149fd5dcea35afe11c118cb`。
2. 阅读了 `TODO.md` 与 `PLAN.md`，确认最新提交已经把新的前置 blocker 细化并排入当前顺序。

### 当前判断

- 最新提交没有留下“应立即单独修而未建任务”的额外遗留项；它明确把新发现的问题拆成了前置任务。
- 初始读取时的第一个未完成任务是 `T3010b2b1`；继续复现后已确认它还依赖一个更基础、此前未单列的前置缺口。
- 当前经重新排序后的第一个未完成任务是 `T3010b2b0`：修正普通 callee frame 内 non-resuming perform/raise 后继续执行的控制流语义。

### 下一步

1. 定向运行与 `T3010b2b1` 直接相关的失败 fixture，确认最小复现。
2. 阅读 `state_machine_emitter.rs`、effect runtime 与相关测试，定位 arm body 中 non-resuming effect 的传播路径。
3. 如果问题范围仍过大，再把 `T3010b2b1` 继续细分；否则直接实现并验证。

## 进度更新（继续）

### 新发现的更前置 blocker

- `effect_escape_continuation_finally_arm_raise.scoop` 与 `effect_resume_finally_arm_raise.scoop` 复现结果表明：arm body 中的 `Raise.raise(...)` 只写 active flag，但 arm body 仍继续执行到 `arm_unreachable`，说明当前 `ExecuteArmBody` 还在绕过 state-machine 语义。
- `effect_multi_nonresuming_raise_custom_finally.scoop` 进一步暴露出更基础的问题：`throwAlarm()` 内部的 `Alarm.trip(seed + 1)` 之后仍会继续执行 `throw_alarm_unreachable`。
- 额外定向复现 `nothing_raise_in_helper_basic.scoop` 后再次确认：普通 helper `alwaysFail()` 会在 `Raise.raise(42)` 之后继续打印 `unreachable_in_helper`。

### 结论

- 当前阻塞不只是在 arm body。还有一个未在当前顺序中显式建模的更基础缺口：**普通 callee frame 在 non-resuming perform/raise 后没有终止自身执行**。
- 如果不先修这个前置缺口，`T3010b2b1` 的验收用例 `effect_multi_nonresuming_raise_custom_finally.scoop` 仍然不会正确通过；继续硬做 arm-body 修复会留下明确的 spec mismatch。

### 计划调整

1. 在 `TODO.md` 中新增前置任务 `T3010b2b0` / `T3010b2b0R`，专门跟踪普通 callee frame 的 non-resuming perform 终止语义。
2. 将 `T3010b2b1` 移到这两个前置任务之后，并更新依赖说明。
3. 同步更新 `PLAN.md` 的阶段说明与执行顺序。
4. 本轮只提交这次任务分解与依赖调整，然后停止，等待下一轮按新的首个任务继续执行。
