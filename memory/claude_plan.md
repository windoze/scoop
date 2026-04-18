# 本轮执行计划

## 约束说明

- 本轮目标：只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始任何实际代码实现前，先记录计划，并在关键进展或计划变化时持续更新本文件。
- 如发现最新提交中提到的遗留问题，需优先修复，再进入 `TODO.md` 任务。
- 如当前首个未完成任务过大或被前置缺陷阻塞，必须先更新 `PLAN.md` 与 `TODO.md`，将任务拆分或重排依赖，然后仅执行新的首个子任务。
- 不接受规避式实现；任何与规范不符的问题都必须转化为显式任务并按依赖顺序处理。

## 初始执行步骤

1. 检查最新一次 Git 提交的信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，识别当前第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务是否已有计划、依赖或上下文约束。
4. 评估该任务是否可以在本轮完整落地：
   - 若可以：直接实现、补测试、验证、更新文档与任务状态。
   - 若不可以：把任务拆分为更小的子任务，更新 `PLAN.md` 与 `TODO.md`，并执行新的第一个子任务。
5. 在实现过程中，如发现规范缺口、语言功能缺失、已有 bug 或测试基础设施问题：
   - 先确认是否构成当前任务的真实前置依赖；
   - 若构成阻塞，则先在 `TODO.md` 中新增/重排修复任务，并在 `PLAN.md` 说明原因；
   - 本轮仅处理新的首个任务。
6. 对完成的任务执行充分验证，至少覆盖：
   - 相关 Rust 单测 / 集成测试 / fixture 测试；
   - 必要时执行 `cargo fmt`；
   - 必要时执行 `cargo clippy --all-targets -- -D warnings`；
   - 确认没有引入新的编译或 lint 警告。
7. 完成后更新：
   - `TODO.md`：将本轮任务标记为完成；
   - `PLAN.md`：记录当前状态、后续顺序、任何依赖变化；
   - `memory/claude_plan.md`：补充执行结果与关键决策。
8. 最后创建一次 Git 提交，提交信息清晰描述本轮完成内容，然后停止。

## 当前状态

- 已检查最新提交：`8d25808cf4641f3a0815897c01a08457cfdc9b52`，提交信息为 `[T3016a0] Track tail-merge blocker before cleanup repair`。
- 已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务为 `T3016a0`：修正 no-suspend handle 尾部 control-flow merge 的 result transport 回归。
- 已确认最新提交中提到的前置问题与 `T3016a0` 一致，因此本轮无需再额外插入更早任务，直接处理该 blocker。
- 已运行最小复现：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_tail_if_result.scoop`
  - 当前实际输出为 `0` / `0`，而期望输出为 `13` / `15`。
- 当前诊断：
  - `handle { if (flag) { 13 } else { 15 } }` 在 plan 阶段会生成独立 `if.merge` state。
  - 现有 emitter 只会在“下一跳直接进入 `ReturnHandle` / `CleanupEnter` 完成入口”时，把 `last_value` 写入 frame result slot。
  - 对于 `then/else -> if.merge(no-op) -> ReturnHandle` 这类透明 Goto 链，`last_value` 没有跨 merge state 继续 transport，最终 `ReturnHandle` 读到的是默认结果槽，因此出现 `0/0`。
- 下一步修复计划：
  1. 在 unified emitter 中把“carried result 可继续保留”的判断从单跳扩展为递归透明链判断，覆盖 `Goto -> ... -> ReturnHandle/ReturnFromFunction/ArmReturnHandle/...` 这类 completion path。
  2. 保持透明状态的判定严格，只允许无副作用、不会改写 carried result 的 op 作为 relay。
  3. 补充针对 tail `if/else` merge 形状的回归测试，避免以后再次只覆盖“直接尾值”而漏掉 merge state。
  4. 跑通定向 fixture、全量测试与 clippy，然后更新 `TODO.md`、`PLAN.md` 和本文件。

## 本轮结果

- 已完成 `T3016a0` 代码实现：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 现已支持递归透明 `Goto` 链的 carried-result transport。
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs` 新增结构测试 `tail_if_else_result_flows_through_transparent_merge_state`。
  - `tests/fixtures/run-pass/effect_handle_tail_if_result.scoop` 已从 `EXPECT: fail` 改回 `EXPECT: pass`。
- 已完成验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_tail_if_result.scoop`
    输出已恢复为 `13 / 15`。
  - `cargo test -p scoopc tail_if_else_result_flows_through_transparent_merge_state -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 额外核对：
  - 重新直跑 `tests/fixtures/run-pass/effect_nosuspend_finally_nested_handle.scoop` 后，当前分支仍输出 `0 / 0`。
  - 该问题已由现有 `T3016a` 跟踪，说明 `T3016a0` 修复的是更基础的 transparent tail-merge 缺口，但不等于 `T3016a` 已闭环。
- 文档状态：
  - `TODO.md` 已将 `T3016a0` 标记为完成。
  - `PLAN.md` 已将下一项推进到 `T3016a0R`，并同步记录本轮验证结果。
