# 执行计划

## 约束说明

- 按要求，本次先记录执行计划，再开始任何仓库检查与实现工作。
- 这里记录的是可审计的执行计划、决策依据与进度，不包含内部推理细节的逐字转录。
- 本轮目标是：先处理最新提交中提到的既有问题；然后完成 `TODO.md` 中第一个未完成任务；完成后更新文档、测试并提交；随后停止。

## 初始计划

1. 检查最近一次提交：
   - 查看最新提交消息与变更范围。
   - 判断提交中是否明确提到已知问题、遗留缺陷、后续修复项或 TODO。
   - 若存在此类问题，先定位并修复，再继续后续任务。
2. 读取任务与计划文件：
   - 查看 `TODO.md`，定位第一个未完成任务。
   - 查看 `PLAN.md`，理解现有任务分解与依赖关系。
   - 如当前首个未完成任务过大，先拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
3. 实施当前应执行任务：
   - 阅读相关代码、测试与规范上下文。
   - 在不引入规避方案的前提下完成实现。
   - 如遇到规范缺口、实现边界或阻塞项，按要求先在 `TODO.md` / `PLAN.md` 中新增前置任务、调整顺序并停止。
4. 验证：
   - 运行与改动相关的测试。
   - 视情况运行更广泛的检查，至少覆盖构建、相关测试与必要的 lint。
   - 若发现失败，先修复再重跑。
5. 记录与提交：
   - 更新 `TODO.md`，标记当前任务完成，或在阻塞时调整任务顺序并记录依赖。
   - 更新 `PLAN.md`，记录当前状态、完成情况与任何调整。
   - 再次更新本文件，记录关键步骤完成情况与最终结论。
   - 使用清晰提交信息创建一次 git commit。
6. 停止：
   - 完成一个任务后停止，不继续处理下一个任务。

## 待确认事项

- 最新提交是否包含“先修复”的既有问题。
- `TODO.md` 中的第一个未完成任务是什么。
- 当前工作区是否存在用户未提交改动，需要避免覆盖。

## 进度

- 已创建本计划文件并写入初始执行计划。
- 已检查最新提交标题与统计信息：最新提交为 `[T3015a] Restore resumed-segment handler redispatch`，提交消息本身未额外注明新的“必须先修复”的遗留问题。
- 已读取 `TODO.md` / `PLAN.md` 并定位当前首个未完成任务：`T3015aR`。
- 当前判断：`T3015aR` 属于 review 任务，范围明确，暂不需要再拆分子任务。
- 已完成 `T3015aR` 的生产代码复审，覆盖：
  - `runtime/c/scoop_runtime.c`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
- 复审结论：
  - runtime 侧 continuation 已通过 handler-stack 堆快照摆脱原始栈上 handler frame 生命周期；
  - compiler 侧初始 handle 入口与 escaped continuation resume 统一复用 `scoop.effect.dispatch.*` dispatch-loop entry；
  - multi-site 与 statement-container matrix 共享同一套生产 redispatch 机制，未发现 fixture-only / shape-based patch 残留；
  - 本轮未发现需要额外修复的新增生产代码问题。
- 已完成定向与全量验证：
  - `cargo test -p scoop_runtime --test continuation_cross_thread_handler_stack`
  - `cargo test -p scoopc escaped_continuation_ir_uses_dispatch_loop_entry_for_resume -- --nocapture`
  - `cargo test -p scoopc indirect_if_branch_callee_keeps_handle_call_site_active_dispatch -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_indirect_callee_suspend_matrix.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_multi_perform_while_loop.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_ref_class.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已更新 `TODO.md` 与 `PLAN.md`，将 `T3015aR` 标记完成，并把下一项执行顺序推进到 `T3009b2`。

## 已执行计划（T3015aR）

1. 阅读 `T3015a` / `T3015aR` 在 `TODO.md` 与 `PLAN.md` 中的上下文，明确本轮复审目标与验收标准。
2. 定向审查上轮真实改动的生产代码：
   - `runtime/c/scoop_runtime.c`
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
   - 如有必要，补查 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 与相邻 ABI/入口代码
3. 重点核对：
   - continuation 捕获/恢复的 handler stack 生命周期是否闭环；
   - resumed segment 继续执行时，新的 `perform` 是否一定经由统一 dispatch-loop entry，而非偶然落回 raw `step_fn`；
   - caller TLS / handler stack / pin/unpin 是否在正常路径和早退路径上都成对恢复；
   - 是否存在 fixture-only、单场景分流或按源码形状兜底的残留。
4. 若发现真实生产问题：
   - 直接修复生产代码；
   - 重新运行对应定向测试与必要的全量检查；
   - 在本文件中记录修复原因与结果。
5. 若未发现问题：
   - 运行复审所需的定向验证，确认现有实现与任务描述一致；
   - 更新 `TODO.md` / `PLAN.md` / 本文件，标记 `T3015aR` 完成并记录审查结论；
   - 提交 git commit 后停止。

## 收尾状态

- 当前没有生产代码改动；本轮变更仅涉及计划/任务/进度文档。
- 下一步只剩：检查工作区 diff，提交一次文档性 commit，然后停止。
