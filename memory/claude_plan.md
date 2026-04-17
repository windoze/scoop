# 本次执行计划

## 目标
- 按 `TODO.md` 中顺序处理第一条未完成任务，并在完成后停止。

## 执行步骤
1. 检查最新一次 Git 提交，确认是否提到需要先修复的既有问题；若有，则优先修复这些问题。
2. 阅读 `TODO.md`，定位第一条未完成任务。
3. 阅读 `PLAN.md` 与相关代码，确认任务边界、依赖和现状。
4. 如果任务过大，则先把任务拆分为更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`；本次只执行拆分后的第一条子任务。
5. 实现当前目标任务，保证实现符合规范，不引入临时性绕过方案。
6. 运行相关格式化、检查和测试；若发现既有缺陷或规范不匹配，先修复或把阻塞项前移写入 `TODO.md`。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md` 记录进展。
8. 使用清晰的提交信息提交改动，然后停止。

## 进度记录
- 已创建本计划文件，待开始仓库检查。
- 已检查最新提交 `ad7a2b07a0035ac79a8e6fd6ca33ede9cd25b16a`；提交信息未额外声明需要先处理的新既有问题。
- 已确认 `TODO.md` 中第一条未完成任务是 `T3010b2b1b`：重新基线化 nested arm / nested handle / indirect helper call 路径上的 unified expected-context / coercion 缺口。
- 下一步：
  - 阅读该任务附近的 TODO/PLAN 说明与相关代码。
  - 用定向 fixture / 测试找出当前仍然失败的最小 repro。
  - 若缺口仍存在，则修复并补测试；若缺口已消失，则收窄或移除该任务并更新 `TODO.md` / `PLAN.md`。
- 已完成定向重新基线化：
  - `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 已通过，说明旧锚点不再代表当前 blocker。
  - `effect_resume_nested_escape_handle_tail.scoop` 现为新的最小 repro，但失败类型已变为 `call callee`，不是原先预期的 `value coercion`。
  - `effect_escape_continuation_resume_unit.scoop` / `effect_escape_continuation_resume_string.scoop` 复现了同一错误，进一步确认根因是 escaped continuation 的 `Continuation.resume(...)` 仍缺 dedicated lowering，而不是 `T3010b2b1b` 原描述中的 expected-context/coercion 缺口。
- 计划已调整：
  - 在 `TODO.md` / `PLAN.md` 中新增前置任务 `T3009b0` / `T3009b0R`，先接通 escaped continuation 的 scalar/ref `Continuation.resume(...)` lowering。
  - 将 `T3010b2b1b` 改为依赖 `T3009b0R` 之后再重新基线化。
  - 将原 `T3009b` 收窄为 composite resume payload 扩展任务，保持在 `T3013R` 之后。
- 下一步：
  - 检查文档改动与工作区状态。
  - 提交本轮“任务重排 + 阻塞说明”变更并停止，等待下次调用执行新的第一条未完成任务。
