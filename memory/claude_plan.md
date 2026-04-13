# 本轮执行计划（更新于 2026-04-13）

## 背景

- 当前轮次只允许完成一个任务并停止。
- 上一阶段实现已完成 `T2003u4c2`：将 pure `multiple escape-continuation` 与 `escape + sibling non-resuming` 的路由切换到 unified state-machine plan 输入。
- 最新提交 `1f40c1c833361ba775f44f43e3892e3da811f3dc` 已检查，提交信息未暴露需要优先修复的遗留问题。
- 相关代码改动、测试、`TODO.md` / `PLAN.md` 更新和 `cargo fmt`/测试/`clippy` 已完成；本轮剩余工作是验证最终工作树、补充进度记录并提交。

## 当前目标

完成并提交 `T2003u4c2`，然后停止，不进入 `T2003u4c3`。

## 已完成实现摘要

- `crates/scoopc/src/llvm/codegen/effect/escape_continuation.rs`
  - 新增基于 `HandleStateMachinePlan` 的 mixed escape direct/indirect site 解析 helper。
- `crates/scoopc/src/llvm/codegen/effect/shared.rs`
  - 新增从 plan 收集 escape capture metadata 的 helper。
- `crates/scoopc/src/llvm/codegen/effect/nonresuming.rs`
  - multiple escape / mixed nonresuming 路由改为向下传递 `state_machine_plan` 与 `arm_id`。
- `crates/scoopc/src/llvm/codegen/effect/multi_escape.rs`
  - top-level multiple escape 逻辑改为消费 unified plan resolver/capture helper。
- `crates/scoopc/src/llvm/codegen/effect/mixed.rs`
  - 各 mixed escape with nonresuming siblings 入口改为使用 plan-driven site 解析；旧 emitter 剩余 `body_lift_ids` 需求改由 plan-derived captures 反推。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_plan_tests.rs`
  - 新增 direct/indirect mixed escape resolver 单测。
- `TODO.md`
  - `T2003u4c2` 已标记 `[DONE]`。
- `PLAN.md`
  - 已记录 `T2003u4c2` 完成，并将下一步更新为 `T2003u4c3`。

## 已完成验证

- `cargo test -p scoopc resolve_mixed_escape`
- `cargo test --all`
- `cargo run -p scoop --features llvm -- test`
- `cargo run -p scoop -- test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all`
- `cargo test --all`（格式化后复验）

以上验证在上一阶段已通过；本轮不重复执行整套验证，除非工作树检查发现异常。

## 本轮收尾步骤

1. 查看 `git status --short`，确认当前未提交内容与本轮任务一致。
2. 查看必要的 diff 概况，确认 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 以及代码改动都在。
3. 如检查中发现遗漏，先更新 `memory/claude_plan.md` 和必要文件，再复核。
4. 执行 `git add`，准备提交本轮成果。
5. 使用提交信息 `[T2003u4c2] Route multiple escape/nonresuming through unified plan` 提交。
6. 提交后停止，不继续处理下一任务。

## 风险与边界

- 不回退任何非本轮改动。
- 不提前修改 `T2003u4c3` 范围的代码。
- 若工作树中出现与本轮不一致的异常改动，需要先记录到本文件，再决定是否可安全提交。

## 进度记录

- [x] 已将本轮计划和当前状态写入本文件。
- [x] 已检查最终工作树：`git status --short` 与 `git diff --stat` 仅包含 `T2003u4c2` 相关代码、`TODO.md`、`PLAN.md` 与本文件；未发现额外脏改动。
- [x] 已复核 `TODO.md` / `PLAN.md` / `memory/claude_plan.md` 的差异记录，任务状态与后续计划一致。
- [ ] 待完成最终提交。
