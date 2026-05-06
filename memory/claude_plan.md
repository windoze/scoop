# 执行计划

## 当前状态

- 本文件用于记录本次调用的可公开执行计划、关键决策和进度更新。
- 已读取 `TODO.md`，原第一个未完成任务为 `MIR-T04：完成 comptime、splice field、class literal、with-update 的 MIR 前置闭包`。
- 执行 `MIR-T04` 指定 splice-field `dump-mir` 验证时发现直接 blocker：strict refactor MIR 在 `tests/fixtures/comptime/splice_field_access_v0_basic.scoop` 上拒绝 `Item::Todo { kind: "top-level val" }`。
- 该 blocker 已由现有 `MIR-T05` 覆盖，因此本次只调整任务顺序和依赖，停止在前置任务修复之前。

## 步骤计划

1. 保持 `MIR-T04` 未完成，不标记 `[DONE]`。
2. 在 `TODO.md` 中将已存在的 `MIR-T05` 前移为 `MIR-T04` 的前置任务，并记录 blocker 原因。
3. 在 `PLAN.md` 中记录这次阶段依赖调整，因为 M3 top-level roots 现在必须先于剩余 M2 surface 闭包验证。
4. 提交 `TODO.md`、`PLAN.md` 和本计划文件的变更。
5. 停止，不实现 `MIR-T04` 或继续下一任务。

## 进度记录

- 已创建初始计划文件。
- 已确认当前任务为 `MIR-T04`。
- 已运行指定 splice-field `dump-mir` 验证，失败原因为 `<file>` 上 `top-level val` item Todo。
- 已更新 `TODO.md`：`MIR-T05` 前移到 `MIR-T04` 之前，`MIR-T04` 依赖增加 `MIR-T05`，并记录不得绕过该 blocker。
- 已更新 `PLAN.md`：记录 M3 top-level roots 需要先于 `MIR-T04` 验证执行。
