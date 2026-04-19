# 执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，并在完成后停止。若在执行过程中发现前置缺陷、规范不匹配或任务过大，则先按仓库要求更新 `TODO.md` / `PLAN.md`，提交后停止。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到已有问题或待修复事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和当前阶段。
4. 如任务过大，先把任务拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并将新的首个子任务作为当前执行目标。

## 执行策略

1. 理解目标任务涉及的代码路径、测试路径和规范要求。
2. 实现任务所需修改，避免引入临时性绕过方案。
3. 运行相关测试，并根据需要补充或修复测试。
4. 运行必要的质量检查，至少覆盖与本次修改相关的构建、测试与 lint。
5. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况或阻塞原因。
6. 使用清晰的提交信息创建 Git 提交，然后停止，不继续下一个任务。

## 阻塞处理原则

1. 如果遇到前置缺陷、规范缺口或实现边界导致无法按规范完成当前任务，不做规避实现。
2. 将缺陷转化为新的前置任务，插入 `TODO.md` 中正确位置，并调整依赖顺序。
3. 在 `PLAN.md` 中记录阻塞原因、影响范围和新增任务。
4. 仅提交这些计划性调整，然后停止。

## 进度记录

- 已创建本计划文件，待开始检查最新提交与任务列表。
- 已检查最新提交 `[T4008b2] Re-type escape continuations from resumed-step summaries`；提交正文为空，未额外声明需要先修的历史问题。
- 已定位 `TODO.md` 首个未完成任务为 `T4008c`。
- 已确认 `T4008c` 同时覆盖两条相对独立的缺口：
  - `crates/scoopc/src/typecheck/expr/call.rs` / `infer.rs` 对“多 effect type params”存在独立 early reject。
  - 同两处文件对“receiver effect op”也存在独立 unsupported gate，且后续会继续影响 HIR / LLVM effect lowering。
- 决定先把 `T4008c` 细化为两个子任务，再执行新的第一个子任务：
  1. `T4008c1`：收口多 effect type params 的 effect op call / handle arm 实例化。
  2. `T4008c2`：打通 receiver effect op 的 perform / handler lowering / codegen。
- 已发现工作区存在用户侧未提交改动：`ISSUES.md` 当前为 dirty 状态；除非本轮结果必须同步收窄 issue 描述，否则不主动覆盖该文件。
- 在为 `T4008c1` 设计 run-pass 回归时，发现另一个独立旧缺口：`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 `emit_perform_op` 当前只接受 0/1 个 payload arg，直接对 2+ 形参 effect op 报 `state machine perform arity`。
- 该缺口不会阻塞 `T4008c1` 的“多 effect type params”收口，但会阻塞后续 receiver effect op lowering，因此需要作为新的后续任务显式记录，再继续推进 `T4008c1`。
- 在执行 `cargo run -q -p scoop -- test` 做全量 fixture 复验时，命中了与本轮实现无关的既有 blocker：`tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_if_multi.scoop` 当前在 clean `HEAD` 工作树里也会错误输出 `fetch_resume / resume_else_1 / missing2`，说明 statement-position if/else 的 direct + indirect same-stmt mixed replay 在 else 分支仍会丢失第二个 continuation。
- 已确认该问题会阻塞当前任务的全量验证，因此本轮不继续保留 `T4008c1` 的未提交实现；需要先把该 blocker 插入 `TODO.md` / `PLAN.md` 的正确顺序，并将下一次执行目标改为修复它。
