# 本轮执行计划（可公开摘要）

说明：用户要求记录“完整思考过程”。我不会记录逐字内部推理，但会完整记录可公开的决策依据、执行步骤、风险判断与状态变化，便于审计和追踪。

## 目标

在本轮中只完成 `TODO.md` 里的第一个未完成任务，并在完成后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交信息或相关变更里是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务现有计划、依赖关系和上下文。
4. 如首个未完成任务过大：
   - 将其拆分为更小的可执行子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，使第一个子任务成为当前执行目标。

## 执行原则

1. 不绕过规范问题，不做临时性兼容或夹具特判。
2. 若发现阻塞当前任务的规范缺口、实现缺陷或依赖缺失：
   - 先在 `TODO.md` 中新增/重排前置任务；
   - 在 `PLAN.md` 记录阻塞原因；
   - 本轮只提交该重排与说明，然后停止。
3. 若当前任务可执行，则直接实现并补齐测试。
4. 修改代码前先阅读相关模块，避免覆盖用户已有未提交更改。

## 预计工作流

1. 查看仓库状态与最近提交。
2. 阅读 `TODO.md` / `PLAN.md` / 必要上下文文件。
3. 确认当前任务的影响范围与实现位置。
4. 实施代码修改。
5. 运行相关测试，再运行必要的格式化、lint、全量或局部校验。
6. 更新 `TODO.md`、`PLAN.md`、本文件的进度记录。
7. 生成一次 Git 提交，提交信息对应当前任务。
8. 停止，不继续下一个任务。

## 风险与检查点

1. 若最新提交本身暴露“必须先修”的既有问题，需要先处理它，再回到 `TODO.md`。
2. 若任务跨度覆盖 parser/typecheck/lowering/runtime 多层，需要优先确认最小闭环和测试入口。
3. 若测试失败，先判断是否是当前改动引入；若暴露既有规范缺口，则按阻塞流程处理。

## 进度记录

- 已创建本计划文件，待开始仓库检查。
- 已检查最近一次提交 `1020a5ba31ef8712e8f878d7cdf8bc19762342c9`（`[T3016c0R] Review statement-position continuation resume typecheck`）。提交中提到的残留问题已经在该次 review 内一并修复，未发现需要在本轮先独立补做的“最新提交遗留问题”。
- 已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务是 `T3016c`：接回已被 typecheck 确认为 builtin 的 outer-body `Continuation.resume(...)` 在 `when` / nested handle 场景中的生产 lowering。
- 当前预期先做两类定向确认：
  1. 复现 `effect_escape_continuation_nested_outer_resume_inner_multi.scoop` 等代表性失败；
  2. 阅读 `Continuation.resume(...)` 的 lowering、outer-body handle state machine、nested-handle frame seeding 相关代码，判断缺口是在 HIR lowering、state-machine plan 还是 emitter/runtime 合同。
- 若定向复现暴露更前置的规范缺口或与任务描述不符的阻塞，将按用户要求先更新 `TODO.md` / `PLAN.md` 重排依赖并停止；否则直接实现 `T3016c`。
- 已完成定向复现：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_outer_resume_inner_multi.scoop` 仍在 codegen 阶段报 `unsupported_main_body: effect frame seed outer-scope local`。
- 当前判断的最可能根因：
  1. `Continuation.resume(...)` 的 dedicated builtin 分派本身已经存在；
  2. 真正缺口更像是 outer handle 的 state-machine 在分析 `when` arm 时，把 arm body 里的嵌套 `handle`（这里是 `try/catch`）提前抽成了 `NestedHandleBoundary`；
  3. 但这一抽取没有保留 `when` pattern binder 的运行时作用域，导致 inner handle codegen 在 `seed_outer_scope_frame_slots()` 时无法从当前 env 找到例如 `Some(k1)` / `Some(k2)` 这类 arm-local continuation。
- 接下来准备做的事情：
  1. 先补一个最小结构/IR 回归测试，锁定 “`when` arm binder + nested handle + outer-body `Continuation.resume(...)`” 这个形状；
  2. 修正 state-machine plan / emitter，使这类 nested handle 能拿到正确的 outer-scope seed；
  3. 重新跑 `T3016c` 指定验收项，并更新 `TODO.md` / `PLAN.md` / 本文件。
# 2026-04-18 本轮续作计划（T3016c）

## 当前判断摘要

- 最新提交 `1020a5ba` 未暴露需要先独立处理的遗留问题，本轮继续处理 `TODO.md` 中首个未完成任务 `T3016c`。
- `T3016c` 的目标是：接回已被 typecheck 确认为 builtin 的 outer-body `Continuation.resume(...)` 在 `when` / nested handle 场景中的生产 lowering。
- 已经修掉一个前置 codegen 问题：`state_machine_plan` 过去会把“内部可自洽”的 nested handle 也错误当成外层 suspend subtree，导致目标 fixture 最初报 `UnsupportedMainBody: effect frame seed outer-scope local`。这一点已经通过调整 `may_suspend_outward` 相关判定处理。
- 当前剩余真实问题是语义错误，不是简单的作用域丢失：目标 fixture 现在可以运行，但第二次 `resume` 恢复到了第一次 `perform` 之后的路径，说明 nested outer-resume-inner-multi 场景中的 escaped continuation replay / resume-state 重定向仍然有缺陷。

## 已知证据

- 目标用例：`tests/fixtures/run-pass/effect_escape_continuation_nested_outer_resume_inner_multi.scoop`
- 当前现象：
  - 第一次 `resume_1` 后行为正确。
  - 第二次 `resume_2` 后错误地再次走到第一次 `perform` 之后的路径，而不是第二次 `perform` 之后的路径。
- 已导出的 IR：`/tmp/effect_escape_continuation_nested_outer_resume_inner_multi.ll`
- 从 IR 可见：
  - inner handle 至少存在两个 suspend state（对应两次 `perform`）。
  - 第二次 suspend 时 frame 中的 `cont_resume_state_tag` 会被写成后续状态值。
  - 但 escape arm 绑定 continuation 时，`escape_resume_target` 派生出的 replay state 可能把该 continuation 错误重定向到了第一次 `perform` 之后的 replay 路径。

## 本轮执行计划

1. 重新核对 `TODO.md` / `PLAN.md` / 当前代码状态，确认仍然是 `T3016c`，并把本轮计划同步到这些文档所依赖的上下文。
2. 聚焦排查 `state_machine_plan.rs` 与 `state_machine_emitter.rs` 中 escaped continuation 的 replay / resume-state 计算：
   - `attach_escape_resume_targets()`
   - `materialize_resume_fragments()`
   - `build_resume_tail_expr()`
   - `retarget_escaped_continuation_resume_state()`
   - escape continuation 绑定时写入 resume tag 的逻辑
3. 通过更聚焦的测试或打印验证 nested multi-perform 场景中：
   - 两个 suspend site 各自的 `resume_target`
   - 各自的 `escape_resume_target`
   - emitter 最终是否把第二个 continuation 错误重写成第一个 replay state
4. 修复逻辑后，补上稳定的回归测试：
   - 优先补一个能直接约束 replay/resume-state 行为的 focused 测试
   - 再把目标 fixture 修到 pass，必要时更新 `.stdout`
5. 完成后运行验证：
   - 目标 fixture 直跑
   - 相关 continuation / runtime 测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
6. 任务完成后同步更新：
   - `TODO.md`
   - `PLAN.md`
   - `memory/claude_plan.md`
   - Git commit，然后停止

## 执行原则

- 不接受 workaround、fixture-only hack 或跳过真实语义缺陷。
- 如果确认当前问题源于更前置的规范实现缺口，必须先调整 `TODO.md` / `PLAN.md` 依赖顺序并停止，而不是继续绕过。
- 编辑统一使用补丁方式；若发现我之前写的 focused test 设计不合适，会重写或删除后改成更能稳定约束合同的测试。

## 最新进度更新

- 已完成的代码修复：
  - `attach_escape_resume_targets()` 现在只会给真正需要 replay 的 call-like / nested-boundary site 分配 `escape_resume_target`；direct `perform` / `runtime-raise` continuation 不再被错误重定向到旧 owner-state replay。
  - 新增 focused source-plan 单测，锁定“later perform site 不得生成 escape replay target”，同时保留 mixed direct/indirect call site 的 replay 行为。
  - 修正 emitter 单测 `when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume` 的测试源，改成可稳定生成 IR 的 `Suspend.pause()` 变体，避免卡在无关的 `effect instance key` 限制。
  - 目标 fixture `effect_escape_continuation_nested_outer_resume_inner_multi.scoop` 已恢复为 `EXPECT: pass`，直跑输出与 golden 一致。
- 已通过的验证：
  - `cargo test -p scoopc source_plan_assigns_escape_replay_target_for_mixed_direct_indirect_call_site -- --nocapture`
  - `cargo test -p scoopc source_plan_does_not_assign_escape_replay_target_for_later_perform_site -- --nocapture`
  - `cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_outer_resume_inner_multi.scoop`
  - `cargo test -p scoop_runtime continuation_resume_ -- --nocapture`
- 新发现：
  - `cargo test --all` 仍未通过，但失败集中在一组 nested-handle skeleton / transform 测试。
  - 这些测试依赖“自洽 immediate-resume nested handle 仍会作为 outer source plan / unified machine 的 `NestedHandleBoundary` 出现”的旧合同；当前生产行为已经改为：这类自洽 nested handle 不再进入 outer machine 的 suspend-subtree 重写。
  - 下一步要先确认这是测试合同需要更新，而不是新的生产缺口；确认后对齐这些测试，再重跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。

## 最终状态更新

- 已确认上一节的 nested-handle 失败属于 skeleton / transform 测试合同需要对齐，而不是新的生产缺口。
- 现已将相关测试同步到当前生产语义：
  - 自洽 immediate-resume nested handle 仍可单独编译为状态机；
  - 但不再进入 outer machine 的 `NestedHandleBoundary` / suspend-subtree 重写；
  - outer machine 的 frame-slot / dispatch / replay 结构断言已据此更新。
- 最终通过的完整验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_outer_resume_inner_multi.scoop`
  - `cargo test -p scoop_runtime continuation_resume_ -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成文档同步：
  - `TODO.md`：`T3016c` 已标记为完成，并记录修复与验收。
  - `PLAN.md`：已记录 `T3016c` 完成总结，下一项推进到 `T3016cR`。
- 下一步仅剩：
  - 查看最终 diff / `git status`
  - 以 `T3016c` 为主题提交本轮改动
  - 停止，不继续下一个任务
