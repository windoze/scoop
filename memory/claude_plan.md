# T0150h-2 执行计划

## 背景

- 最新提交信息为 `[T0150h-1] 修复数值字面量运算的 expected-type absorption`，未在提交说明中发现需要先处理的遗留问题。
- `TODO.md` 当前首个未完成任务是 `T0150h-2 数组字面量目标类型向更深嵌套表达式传播`。
- 该任务的核心缺口已经实现到代码中，当前主要剩余回归验证、文档状态同步、清理临时文件与提交。

## 已完成实现摘要

- 在 typecheck 阶段补齐 expected type 向更深层表达式传播：
  - `block` / `unsafe block` / `safe block`
  - `when`
  - 相关 block tail value 推断路径
- 在 HIR lowering 阶段补齐 expected type 继续下传：
  - block 末尾表达式
  - `if` / `when` 分支
  - 数组字面量中复杂元素表达式
- 针对 `Array<UInt8>` 这类 builtin 标量别名场景，补齐 lowering 时的类型规整，避免后端把 builtin 标量误当作 nominal struct，导致 coercion 失败。
- 已补充 typecheck / run-pass fixtures 覆盖数组字面量中的深层嵌套 expected-type 传播。

## 待执行步骤

1. 运行 fixture 全量回归：`cargo run -p scoop -- test`
2. 运行严格 clippy：`cargo clippy --workspace --all-targets --message-format short -- -D warnings`
3. 若检查通过，更新项目文档状态：
   - `TODO.md` 把 `T0150h-2` 标记为完成
   - `PLAN.md` 记录该任务已完成及实现要点
   - 继续更新本文件，记录关键步骤完成情况
4. 删除本轮排障产生的临时复现文件 `memory/repro_t0150h2.scoop`（若仍存在）
5. 检查工作区差异，确认没有残留调试代码或意外文件
6. 提交本轮改动，提交信息使用 `[T0150h-2] 补齐数组字面量深层 expected-type 传播`
7. 停止，不继续执行下一个任务

## 风险与关注点

- 需要确认 fixture 全量回归不会暴露新的 expected-type 回归。
- 需要确认 clippy 在新增辅助函数与控制流分支上不会报出告警。
- 若回归失败，先修复失败点，再继续状态同步与提交。

## 当前状态

- 代码实现：已完成
- 单测与工作区测试：`cargo test --all` 已通过
- 格式化：`cargo fmt --all` 已通过
- fixture 全量回归：`cargo run -p scoop -- test` 已通过（`fixtures: ok (876)`）
- 严格 clippy：`cargo clippy --workspace --all-targets --message-format short -- -D warnings` 已通过
- 文档状态同步：进行中
- 待完成：清理工作区、检查差异、提交

## 进度记录

- 已完成全量 fixture 回归，未出现失败；仅有既有 warning，不影响任务验收。
- 已完成严格 `clippy` 检查，无新增 warning。
- 正在把 `T0150h-2` 的完成说明回写到 `TODO.md` 与 `PLAN.md`，并清理排障临时文件。
- 已完成工作区差异检查：`git diff --check` 通过，目标源码文件内未发现残留调试输出。
- 下一步为提交本轮任务，提交后停止，不继续处理下一个 TODO。
