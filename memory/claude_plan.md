# 本轮执行计划

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始实际实现前，先检查最新一次提交是否提到任何已有问题；若有，这些问题也属于本轮范围，必须先修复。
- 若第一个未完成任务过大或被前置缺陷阻塞，需要先拆分任务并更新 `PLAN.md` / `TODO.md`，随后只执行拆分后的第一个子任务。
- 任何与规范不一致的实现、缺失能力、测试依赖 workaround 的情况，都必须转化为 `TODO.md` 中更靠前的明确任务，不能绕过。
- 需要保持 `memory/claude_plan.md` 持续更新，记录关键步骤完成情况、计划变化和阻塞原因。

## 初始执行步骤

1. 检查最新一次 git 提交，确认提交信息和变更中是否提到待修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否可以在本轮完整落地；若不能，则拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 阅读与目标任务直接相关的代码、规范、测试和计划文件，确认正确行为与现状差距。
5. 实现第一个未完成任务或第一个新子任务。
6. 运行必要的格式化、测试与 lint，至少覆盖受影响范围；若任务要求扩大验证范围，则补足。
7. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`，记录已完成内容与后续状态。
8. 生成一次 git 提交，提交信息应清晰描述本轮完成的任务。
9. 停止，不进入下一个任务。

## 记录约定

- 我不会在此文件中写逐字内部思维细节，而会记录可审计的判断依据、执行计划、关键发现与变更结果。
- 每完成一个关键阶段，都会追加“进度更新”小节，说明已经完成的步骤与下一步动作。

## 进度更新 1

- 已检查最新一次提交：`bd80866b4058842a77cc22001eb65f580e84268b`，提交信息为 `[T3016fR] Review top-level multi-escape replay contract`。提交说明仅表明上一轮 review 完成并把下一项推进到 `T3016g`，未额外声明必须先修的新增既有问题。
- 已读取 `TODO.md` 与 `PLAN.md`，确认当前第一个未完成任务是 `T3016g`：修正 immediate-resume + finally 下，resumed body 在 `resume(...)` 之后再次 `Raise.raise(...)` 时没有继续向外传播的回归。
- 当前判断：先复现 `tests/fixtures/run-pass/effect_resume_finally_body_raise_after_resume.scoop` 的失败行为，再阅读 unified effect state-machine 的 cleanup / completion / outward propagation 相关实现，确认是否需要拆分前置子任务。
- 下一步：运行目标 fixture、收集失败输出，并定向检查 `crates/scoopc/src/llvm/codegen/effect/` 下与 cleanup / handle completion / outward propagation 相关的代码。

## 进度更新 2

- 已复现目标回归。当前实际输出为：
  - `before`
  - `body_start`
  - `arm_start`
  - `body_after`
  - `41`
  - `finally`
  - `handle_unreachable`
  - `result`
  - `0`
  - `done`
- 与期望输出相比，差异是：`finally` 之后没有把 `Raise.raise(77)` 继续向外层 `catch` 传播，而是把当前 handle 误判成正常完成。
- 根因已定位到 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 cleanup propagation 路径：
  - `handle_cleanup_propagate_run` 在 outward propagation 场景下进入 `finally`；
  - cleanup state 的普通出口仍会沿 `cleanup_exit -> exit_state -> ReturnHandle` 写入 `STATE_TAG_HANDLE_RETURNED`；
  - 当该路径发生在 immediate-resume arm 触发的 resumed-body dispatch 内层时，外层 arm dispatch 会把这个 terminal `state_tag` 误当成 handle 已完成，从而吞掉本应继续外传的 `Raise.raise(...)`。
- 当前实现方案：在 cleanup-propagate 路径进入 `finally` 前保存原先的非终止 `state_tag`；cleanup 执行完成后，如果 cleanup 普通出口把 `state_tag` 改写成 terminal sentinel，则恢复为保存的 propagating `state_tag`，确保后续仍按统一 outward propagation 合同处理，而不是落回 `ReturnHandle`。
- 下一步：修改 emitter、补一条针对 cleanup-propagate state restore 的单测或 IR 断言，并重新运行目标 fixture、相关回归、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。

## 进度更新 3

- 生产修复已完成：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 `handle_cleanup_propagate_run` 现在会在进入 shared cleanup 前读取并保留 propagating `state_tag`；
  - cleanup 执行完毕后，若 shared finally 的普通出口把 `state_tag` 误写成 `STATE_TAG_HANDLE_RETURNED` / `STATE_TAG_FUNCTION_RETURNED`，则通过新的 `restore_propagating_state_tag_after_cleanup()` 恢复为原先的非终止状态；
  - 修复没有给 immediate-resume 单独加旁路，而是直接收口在共享 cleanup/propagation 出口上。
- 回归锁定已补充：
  - 新增 emitter IR 单测 `cleanup_propagate_ir_restores_propagating_state_after_shared_finally_exit`，验证 cleanup-propagate 路径会保留 pre-cleanup propagation state，而不是泄露 terminal sentinel。
  - `tests/fixtures/run-pass/effect_resume_finally_body_raise_after_resume.scoop` 已从 stale `EXPECT: fail` 切回 `EXPECT: pass`。
- 验证已完成：
  - `cargo test -p scoopc cleanup_propagate_ir_restores_propagating_state_after_shared_finally_exit -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_finally_body_raise_after_resume.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_raise_custom_finally.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 计划文件已同步更新：
  - `TODO.md` 已将 `T3016g` 标记为完成，并记录本轮实现与验证。
  - `PLAN.md` 已记录本轮根因、修复方式、测试结果，并把当前 effect 主线下一项推进到 `T3016gR`。
- 下一步：检查工作区 diff，确认只包含本轮改动，然后生成本轮提交并停止。
