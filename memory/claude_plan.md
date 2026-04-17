# 执行计划

## 当前目标

按 `TODO.md` 的顺序完成第一个未完成任务，并在完成后停止；如果发现前置缺陷或规范不匹配，先修复或把依赖任务前移，再提交并停止。

## 已知约束

- 在开始实际实现前，先检查最近一次提交是否提到了遗留问题；若有，必须先处理。
- 只处理一个任务。
- 变更后必须更新 `TODO.md`、`PLAN.md`，并进行测试与提交。
- 不能用规避方案替代规范要求；若遇到缺失特性或实现边界，需要把前置修复任务写回 `TODO.md` 并调整顺序。
- 需要尽量保证构建、测试、`clippy` 无警告。

## 初始步骤

1. 查看最近一次提交信息与变更，确认是否提到需先处理的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的背景、依赖与当前计划。
4. 如任务过大，先拆分为更小子任务，并更新 `PLAN.md` 与 `TODO.md`。
5. 实现当前要执行的那个任务。
6. 运行相关测试；必要时补充或修正测试，并修复发现的问题。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md` 记录进展。
8. 提交本次修改，随后停止。

## 当前进展

- 已检查最近一次提交：`[T3015a] Frontload resumed-segment redispatch blocker`。提交说明没有额外列出必须先于任务清单处理的新遗留问题。
- 已读取 `TODO.md` 与 `PLAN.md`。
- 已确认当前第一个未完成任务是 `T3015a`：修正 escaped continuation 在第一次 resumed segment 后，下一次 outward `perform` 无法重新进入 captured handler dispatch loop。
- 当前判断：`T3015a` 已是为真实 blocker 前移后的明确任务，暂不需要继续拆分；先复现并定位根因，再决定改动范围。
- 已复现三个验收 fixture 的旧问题：第一次 `resume(...)` 之后都在下一次 `perform` 前截断。
- 已完成第一层修复：runtime continuation 不再捕获会在 `handle` 退出时失活的栈上 handler frame，而是捕获可跨返回存活的 handler stack 堆快照；并补了 runtime 定向测试覆盖。
- 已完成第二层修复：compiler 现在为 escaped continuation 生成可复用的 dispatch-loop entry，continuation resume 不再只进入 raw `step_fn`，而会重新跑统一 handler dispatch loop。
- 在修复 redispatch 后，`statement-container` matrix 暴露出 `WhileBody` rebuild 的次级问题：synthetic first-iteration flag 之前使用 `resume_first || cond`，但当前 `LogOr` 不短路，会在 resume 后多求值一次 `cond`。现已改为显式 `if (resume_first) true else cond`，恢复“先完成当前迭代尾部，再回到 cond”的语义。
- 当前定向结果：
  - `effect_multi_escape_indirect_callee_suspend_matrix.scoop` 已跑通并匹配 golden。
  - `effect_escape_continuation_indirect_perform_statement_container_matrix.scoop` 已跑通并匹配 golden。
  - `effect_escape_continuation_multi_perform_while_loop.scoop` 已跑通并匹配 golden。
  - `continuation_resume_ref_class.scoop`（同根因回归）已跑通并匹配 golden。
  - 已回收上述 4 条 fixture 的 `EXPECT: fail` 标记，避免继续被 xfail 掩盖。
  - 新增 emitter IR 定向测试 `escaped_continuation_ir_uses_dispatch_loop_entry_for_resume` 已通过。
  - `cargo fmt` 已完成。
  - `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 已在最终代码状态下重新通过。
  - `TODO.md` / `PLAN.md` 已更新为：`T3015a` 完成，下一项推进到 `T3015aR`。

## 下一步

1. 检查工作树与变更摘要，确认只包含本轮任务所需修改。
2. 提交本次修改并停止。

## 执行中更新规范

- 每完成一个关键阶段，就把结果、发现的问题、下一步写回本文件。
- 如果任务被新的前置缺陷阻塞，会在本文件中记录阻塞原因、需要新增/前移的任务以及停止点。
