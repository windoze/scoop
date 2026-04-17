# 执行计划

## 说明

按要求，我会先把可审计的执行计划与判断摘要写入此文件，再开始执行命令。出于安全与系统约束，这里不会记录逐字的内部私有推理过程，但会持续记录：

- 当前目标
- 已知约束
- 执行步骤
- 关键判断摘要
- 进度更新与计划变更

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

在开始任务前，需要先：

1. 检查最新一次 git 提交，确认其中是否提到任何已知问题或遗留问题。
2. 如果最新提交提到了需要先修复的问题，则这些问题优先于 `TODO.md` 任务处理。
3. 读取 `TODO.md`，定位第一个未完成任务。
4. 如果该任务过大，则拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。

## 约束与完成标准

- 不允许用变通方案绕过规范要求。
- 如果遇到规范缺口、实现边界或前置依赖缺失，必须在 `TODO.md` 中新增或重排任务，并更新 `PLAN.md`。
- 完成当前任务后必须：
  - 实现代码
  - 运行相关测试
  - 更新 `TODO.md`
  - 更新 `PLAN.md`
  - 提交 git commit
  - 停止，不继续下一个任务
- 质量门槛包括：
  - 相关测试通过
  - 尽量运行 `cargo clippy --all-targets -- -D warnings`
  - 不引入新的警告或回归

## 初始执行步骤

1. 查看工作区状态，避免误覆盖已有修改。
2. 查看最新提交的提交信息与改动，判断是否包含待修复问题。
3. 打开 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务。
4. 阅读与该任务相关的代码、测试与规范上下文。
5. 判断任务是否可直接完成；若过大则先拆分并更新计划文件。
6. 实现最先应执行的任务。
7. 运行最小充分测试，再补充更高层测试。
8. 更新 `TODO.md`、`PLAN.md` 和本文件。
9. 提交当前变更。

## 进度记录

- 初始化：已创建本计划文件。
- 已检查工作区状态：当前只有本文件未提交修改。
- 已检查最新提交 `bdd33ca9a32cd9eea9952cbbfd72b4ef6159e6a9`（`[T3013R] Review GC-safe effect transport contract`）。
- 已读取 `TODO.md` / `PLAN.md`。
- 当前观察：
  - `TODO.md` 中按顺序的首个未完成任务是 `T3009b`。
  - 但最新提交在 `PLAN.md`/`TODO.md` 的复审记录里再次明确提到一个已知生产问题：handle 入口目前只注册首个 op-tag，多 op handler 注册与 unmatched propagation 仍有缺口，该问题已由 `T3014` 跟踪。
  - 按用户要求“若最新提交提到既有问题，则必须先修复这些问题再继续任务”，本轮需要优先核实并处理该既有问题，而不是直接进入 `T3009b`。
- 下一步调整：
  1. 阅读 `T3014` / `T3014R` 在 `TODO.md` 与 `PLAN.md` 中的详细描述。
  2. 检查 effect handler 注册相关实现与现有测试，确认缺口是否确实独立于 `T3009b`，以及是否需要先在 `TODO.md` 中重排顺序。
  3. 如果确认必须前置，则先更新 `TODO.md` / `PLAN.md` 的顺序与说明，再实现该问题；如果无需重排但仍可直接修复，则本轮直接落地修复并在完成后更新文档。
- 进一步检查结论：
  - `dispatch_unmatched` 在当前生产代码里已经分支到 outward propagate，不再像 `TODO.md` 旧描述那样直接落到 `handle_done`；因此 `T3014` 的实际残留缺口已收窄为“runtime handler stack 只为首个 dispatch entry 注册 op-tag”。
  - 现有 run-pass fixtures 中，`effect_multi_nonresuming_custom_indirect.scoop`、`effect_op_tag_two_effects_nested_dispatch.scoop`、`effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`、`effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop` 当前都能结束，说明问题更偏向动态上下文合同而不是最基础的 arm dispatch 已全面损坏。
- 已完成实现：
  - 在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中新增 helper，把 handle 入口的 runtime handler 注册改为“每个 `dispatch entry` 分配一个独立的 `ScoopEffectHandlerFrame` 并逐个 push”。
  - `handle_done` / `handle_propagate` 两条出口现在都按逆序逐个 pop 所有已注册 frame，保持与 runtime 栈语义一致。
  - 新增 LLVM IR 定向测试 `multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack`，锁定 multi-op handle 会生成 2 次 push 和对应的 pop 序列。
- 已完成验证：
  - `cargo test -p scoopc multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack -- --nocapture`
- 当前待执行：
  1. 更新 `TODO.md`、`PLAN.md` 与本文件，然后提交。

## 当前结果

- 已执行 `cargo fmt`。
- 已通过 `T3014` 相关定向验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_op_tag_two_effects_nested_dispatch.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`
- 已通过质量门槛：
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已复跑 `cargo run -p scoop --features llvm -- test`：
  - suite 仍只停在已跟踪的 stale `EXPECT: fail` `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`）
  - 未出现新的更早 handler-stack / multi-op registration 失败点
- 已更新文档：
  - `TODO.md` 中已把 `T3014` 标记为完成，并把 `T3014/T3014R` 顺序前置到 `T3009b/T3009bR` 之前，以匹配“实现后立即 review”的项目约束和本轮前置处理顺序。
  - `PLAN.md` 已记录本轮 `T3014` 实现摘要，并将当前主线下一项推进到 `T3014R`。
- 最后待执行：
  1. 快速检查 diff / git 状态。
  2. 提交本轮变更。
