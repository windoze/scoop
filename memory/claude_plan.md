# Claude Execution Plan

## 本轮接手状态

- 用户要求每轮只完成 `TODO.md` 中第一个未完成任务，然后提交并停止。
- 上一位模型已经完成 `T3016` 的实现、测试与文档更新，但尚未创建 Git 提交。
- 本轮目标不是继续实现新任务，而是核验当前工作区、补充必要记录、提交 `T3016`，随后停止。

## 已知完成项摘要

- 已修复 `handle { ... }` 内部 `return` 经过状态机后没有正确作为函数返回向外传播的问题。
- 已补齐 `finally` 清理路径在 cleanup replay 后覆盖 `FUNCTION_RETURNED` / `HANDLE_RETURNED` 完成模式的问题。
- 已为普通场景、带 `finally` 场景、nested handle 场景新增 run-pass fixtures。
- `TODO.md` 已将 `T3016` 标记为完成，`PLAN.md` 已记录完成状态并将下一项推进到 `T3016R`。

## 根因与实现要点

1. `ReturnFromFunction` terminator 本来就会写 frame 返回值并设置 `STATE_TAG_FUNCTION_RETURNED`。
2. 真正缺口在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 `codegen_handle_expr_via_state_machine()`：
   - `handle_done` 读取 `post_state_tag` 后没有消费 `FUNCTION_RETURNED`，导致 handle 作为表达式完成时丢失“函数已经返回”的完成模式。
3. 修复措施：
   - 在 `handle_done` 路径接入 `STATE_TAG_FUNCTION_RETURNED` 分支。
   - 顶层 / 普通函数路径复用现有 `finish_function_return_path()` / `return_context`。
   - 为 step function / dispatch loop 内递归生成的 nested handle 添加 synthetic bridge：
     - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 新增 `effect_function_return_context`。
     - 使 runtime function 内 early-return 先写回当前 handle frame，再桥接回真实外层函数返回。
4. `finally` 路径补强：
   - 在统一 frame system fields 中新增持久化 `completion_tag`。
   - dispatch loop 进入 cleanup 前保存 terminal completion tag，cleanup 完成后恢复到 `state_tag`，确保 `finally` 跑完后仍保留原始返回语义。

## 本轮执行计划

1. 检查工作区状态，确认待提交内容仅包含 `T3016` 相关修改。
2. 视情况做最小必要核验；若工作区与摘要一致，则不扩展任务范围。
3. 将本轮状态更新到本文件。
4. 创建 Git 提交，提交信息使用：
   - `[T3016] Connect handle return function propagation`
5. 停止，不继续处理 `T3016R`、`T3017` 或其它任务。

## 风险与边界

- 全量 LLVM fixture suite 中已知仍会停在 `effect_escape_continuation_async_executor_fifo.scoop` 的 stale `EXPECT: fail`，这是既有 `T3017`，不属于本轮新引入问题。
- 本轮不得继续实现新任务；若核验中发现与摘要不一致的未记录问题，需要先判断是否属于 `T3016` 提交前必须修复的问题。

## 本轮进度记录

- 2026-04-18：接手上一位模型的未提交状态，重建并更新本计划文件，准备做最终核验与提交。
- 2026-04-18：已检查 `git log -1 --stat --oneline`，最新提交为 `[T3015R] Review escaped continuation handler-context closure`，未提及需要在 `T3016` 之前额外插队处理的遗留问题。
- 2026-04-18：已检查 `git status --short`，工作区修改与上一位模型摘要一致，仅包含 `T3016` 相关代码、fixture、`TODO.md`、`PLAN.md` 与本文件。
- 2026-04-18：已复跑 `cargo test --all`，全部通过。
- 2026-04-18：已复跑 `cargo clippy --all-targets -- -D warnings`，全部通过。
- 2026-04-18：当前状态已满足提交条件；下一步仅创建 `[T3016] Connect handle return function propagation` 提交并停止。
