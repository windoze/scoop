# 本轮执行计划（第 1 次更新）

## 目标

按仓库约定完成 `TODO.md` 中第一个未完成任务；若发现前置缺陷、规格不匹配或任务过大，则先整理依赖、更新 `TODO.md` / `PLAN.md`，完成当前应优先处理的最小任务后停止。

## 已知约束

- 需要先检查最新提交是否提到了已知遗留问题；若有，必须先修复这些问题。
- 只能完成一个任务（或当前任务拆分后的第一个子任务），完成后停止。
- 任何规格不匹配、缺失功能、错误实现都不能绕过，必须转成 `TODO.md` 中显式任务并调整顺序。
- 需要在执行过程中持续更新本文件，记录关键进展、计划变化和阻塞信息。
- 需要在任务完成后更新 `TODO.md`、`PLAN.md`，运行相关测试，最后提交 git commit。

## 执行步骤

1. 查看最新一次 git commit，确认是否提到待修复的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 视需要阅读 `PLAN.md`、相关代码、测试和规格，判断该任务是否可直接完成，或是否需要拆分/前置修复。
4. 若任务可执行：
   - 实现改动；
   - 补充或调整测试；
   - 运行相关验证，包括尽可能覆盖的测试/检查命令；
   - 更新 `TODO.md`、`PLAN.md` 与本文件；
   - 提交 git commit；
   - 停止。
5. 若遇到前置缺陷或规格缺口：
   - 精确定义问题；
   - 更新 `TODO.md` / `PLAN.md` 的依赖与顺序；
   - 在本文件记录原因；
   - 提交 git commit；
   - 停止。

## 进度记录

- 已创建本计划文件，接下来开始检查最新提交与任务列表。
- 已检查最新提交 `dd36ce2`：提交标题为 `Update plan`，未在提交标题中直接声明新的单独遗留 bug；需继续结合 `ISSUES.md` / `TODO.md` 确认本轮首个未完成项对应的问题范围。
- 已定位 `TODO.md` 第一条未完成任务为 `T4008c0`：修复 statement-position `if/else` mixed replay 在 `else` 分支丢失第二次 continuation。
- 下一步：阅读 `T4008c0` 的任务说明、`ISSUES.md` 中对应问题描述，以及 effect state-machine / continuation lowering 相关代码与现有测试，先确认是否存在阻塞该任务的更前置缺口。
- 已复现失败，并确认关键现象：同一 fixture 若只保留 `run(false, ...)` 会通过；失败只在先执行一次 `run(true, ...)` 后的第二轮 `run(false, ...)` 出现。
- 根因确认：`scoop_continuation_resume_common` 在 resumed body 结束时，无条件把进入前保存的 `__scoop_callee_suspend_state` 恢复回 TLS。对于“第一次 resume 命中 indirect callee，再生成第二个 continuation”的路径，这会把已经消费过的 continuation-resume replay-state 重新塞回 TLS，导致下一轮 `run(false)` 的第一次 `resume` 把 `fetch(40)` 错当成“已有挂起 callee 的恢复”。
- 已实施修复：
  - `runtime/c/scoop_runtime.c`：改为只在 step_fn 没有消费/替换 captured callee state 时恢复旧 TLS；若 step_fn 已经写回新值，则保留新值，并在生成新的 replay-state 时也把该“已恢复后的值”作为 prev state。
  - `crates/scoop_runtime/tests/continuation_one_shot.rs`：新增回归，锁定“step_fn 替换 TLS 后，runtime 不能 resurrect caller 侧旧 saved state”。
# 2026-04-19 接手续做计划（本轮开始前更新）

## 当前目标

- 只完成 `TODO.md` 中第一条未完成任务：`T4008c0 修复 statement-position if/else mixed replay 在 else 分支丢失第二次 continuation`。
- 在继续任何代码/命令执行前，先把本轮的执行计划、当前判断和预期验证路径写入本文件。

## 已知状态

- 最新提交 `dd36ce2 Update plan` 未在提交标题中显式声明新的 pre-existing issue。
- 先前调查已确认失败可稳定复现，且问题不是单独的 else 分支逻辑错误，而是一次 mixed replay 完成后残留了跨 `run(...)` 的错误状态。
- 当前已存在未提交修改：
  - `runtime/c/scoop_runtime.c`
  - `crates/scoop_runtime/tests/continuation_one_shot.rs`
  - `memory/claude_plan.md`
- `runtime/c/scoop_runtime.c` 内仍有临时 `SCOOP_DEBUG_CALLEE_TLS` 调试输出，最终提交前必须清理。

## 当前技术判断

- runtime 侧“恢复旧 callee TLS replay-state”问题已部分收敛，但不是最终根因。
- 最新证据更指向 codegen/emitter：某些 `Continuation.resume(...)` 完成路径结束时仍错误发布了 `pending_continuation`，进而在 TLS 中残留新的 `ContinuationResumeReplayState`。
- 高可疑位置是 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中 `UnifiedStateTerminator::Suspend` 的 `publish_pending_continuation` 发布逻辑；现在看起来是无条件发出，可能需要按 suspend 场景细分。

## 本轮执行计划

1. 检查当前工作树与相关文件状态，确认临时修改、失败任务位置和 emitter 现状。
2. 深入阅读 `state_machine_emitter.rs` 与相关 runtime 接口，定位“何时应该发布 pending continuation”的正确条件。
3. 如确认为 emitter bug，则修改 codegen 逻辑；如发现更前置的 spec/实现缺口，则按要求先更新 `TODO.md` / `PLAN.md` 重新排序并停止。
4. 清理临时调试日志，保留必要测试辅助与正式修复。
5. 运行定向复现、全量相关测试、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
6. 若任务完成，更新 `TODO.md`、`PLAN.md`、本文件，提交一次清晰的 git commit，然后停止。

## 立即下一步

- 先读取并核对：
  - `git status`
  - `TODO.md` 当前任务位置
  - `state_machine_emitter.rs` 中 `Suspend` terminator 逻辑
  - `runtime/c/scoop_runtime.c` 中 pending continuation / replay-state 相关路径

## 本轮新增判断（完成基线复验后）

- 重新运行 `tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_if_multi.scoop` 后，失败仍与前述一致：第二次 `run(false, ...)` 在第一次 `resume` 后直接跳到错误的 `fetch_resume / resume_else_1 / missing2` 路径。
- 结合 emitter / runtime 阅读，当前更精确的判断是：
  - `Suspend` terminator 里的 `publish_pending_continuation(...)` 不该无条件发出。
  - 对于 `Continuation.resume(...)` resumed body 内的 suspend，真正需要留下 outer replay 的是“会把 fresh continuation materialize 给外层后续 resume”的路径。
  - 这类路径包含：
    - call-like suspend boundary（如 `CallMaySuspend` / `CallStateMachineCallee` / init boundary / nested handle boundary）
    - 无本地 matching arm 的 outward `Perform`
    - 选中了 `EscapeContinuation` arm 的场景
  - 这类路径不应包含：
    - 选中了 `NonResuming` arm 的本地 `Perform`
    - `ImmediateResume` arm
    - `RuntimeRaise` 这类不会留下后续 continuation 的路径
- 当前失败很像是：第二次 resume 完成前命中了本地 non-resuming `Abort.stop`，但 `Suspend` terminator 仍无条件发布了一个 pending continuation，导致 runtime 在 resume 返回后错误构造出新的 replay-state 并残留到下一次 `run(...)`。

## 实施调整

- emitter 修复方向更新为：
  1. `Suspend` terminator 只在“call-like boundary / unmatched outward perform”时立即发布 pending continuation。
  2. 对于选中了 `EscapeContinuation` arm 的路径，改为在 `ArmMaterializeContinuation` terminator 里发布 pending continuation。
  3. `ArmReturnHandle` / `ArmResumeMatchedSite` 不发布。
  4. 清理 runtime 中临时 `SCOOP_DEBUG_CALLEE_TLS` 日志，再做验证。

## 完成结果

- 已完成 emitter 修复：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
    - `UnifiedStateTerminator::Suspend` 不再无条件发布 `pending continuation`。
    - 新逻辑仅对 call-like boundary 与无本地 matching arm 的 outward `Perform` 在 suspend 时立即发布。
    - `EscapeContinuation` arm 改为在 `ArmMaterializeContinuation` terminator 精确发布。
    - 新增 IR 回归 `non_resuming_arm_ir_does_not_publish_pending_continuation`，锁定 non-resuming arm 不会生成多余的 publish site。
- 已完成 runtime 收尾：
  - `runtime/c/scoop_runtime.c` 删除了临时 `SCOOP_DEBUG_CALLEE_TLS` 调试日志。
  - 保留并验证了先前针对 caller TLS / replay-state 不被错误 resurrect 的 runtime 修补。
  - `runtime/c/scoop_runtime_api.h` 已补登记 `scoop_test_continuation_resume_replay_state_create`，消除 ABI allowlist 失败。
- 已完成验证：
  - `cargo test -q -p scoopc non_resuming_arm_ir_does_not_publish_pending_continuation -- --nocapture`
  - `cargo test -q -p scoop_runtime continuation_resume_preserves_step_fn_replaced_callee_suspend_state -- --nocapture`
  - `cargo test -q -p scoop_runtime continuation_resume_does_not_resurrect_saved_replay_state_tls -- --nocapture`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_if_multi.scoop`
  - `cargo run -q -p scoop -- test` -> `fixtures: ok (1058)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 当前状态

- `T4008c0` 现可标记完成。
- 下一项应切换到 `T4008c1`。
