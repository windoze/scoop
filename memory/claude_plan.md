# 本轮执行计划

## 说明

按要求先记录本轮可审计的执行思路摘要与步骤计划。这里保留面向实现的决策摘要、检查项和进度，不写原始内部推理逐字稿。

## 当前目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 执行步骤

1. 检查最新一次 Git 提交的提交信息与改动，确认是否显式提到任何遗留问题。
2. 若最新提交提到需要顺手修复或遗留修复项，先定位并修复这些问题，再继续主任务。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 评估该任务是否过大：
   - 如果可直接完成，进入实现。
   - 如果过大，拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，然后执行拆分后的第一个子任务。
5. 阅读相关代码、测试与规格文档，确认实现边界以及是否存在阻塞性的规格缺口。
6. 实现当前任务，不引入规避性方案；若发现规格缺口，则先把缺口转化为更前置的 `TODO.md` 任务并更新 `PLAN.md`。
7. 运行相关测试，并补充必要测试；同时检查格式、lint 与告警情况。
8. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
9. 使用清晰的提交信息提交本轮变更，然后停止。

## 进度记录

- 已开始：创建本计划文件。
- 已完成：检查最新提交 `79ed773e2f6c23ed7e0c63791262138121f7d1e4`；提交信息未额外声明“尚未修复的遗留问题”。
- 已完成：读取 `TODO.md` / `PLAN.md`，定位首个未完成任务为 `T3016dR`。
- 已完成：复审 `state_machine_emitter.rs`、`runtime_abi.rs`、`gc.rs` 与 `runtime/c/scoop_runtime.c`，确认 continuation / effect frame / resume payload / captured callee suspend state 都走统一 trace/root 合同，未发现 test-only 保活。
- 已完成：新增 IR 回归 `escape_arm_gc_roots_use_frame_slot_or_entry_spill_contract`，锁定 escape arm 中 traced frame slot 与 entry-block spill slot 的组合 root 合同。
- 已完成：验证 `cargo test -p scoopc effect_runtime_functions_use_gc_statepoint_strategy -- --nocapture`、`cargo test -p scoopc escape_arm_gc_roots_use_frame_slot_or_entry_spill_contract -- --nocapture`、`cargo test -p scoop_runtime continuation_ -- --nocapture`、三条 `SCOOP_GC_STRESS=1` fixture、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 已完成当前任务：`T3016dR`。
- 下一任务（仅记录，不执行）：`T3017`。

## `T3016dR` 复审计划

1. 阅读 `TODO.md` 中 `T3016d` / `T3016dR` 描述与 `PLAN.md` 最近更新，明确上一轮修复声称覆盖的边界。
2. 阅读最新提交改动涉及的生产代码，重点关注：
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
   - 相关 runtime tracing / continuation / GC safepoint 合同代码
3. 确认修复机制是否属于统一生产合同，而不是 fixture-only、延迟回收、宽松超时或其它规避性手段。
4. 运行与 GC stress / continuation / object graph 相关的定向测试，以及必要的全量质量门槛：
   - 目标 fixture / 相关 runtime tests
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 如果复审中发现真实生产缺口：
   - 先修复代码与测试；
   - 重新执行相关验证；
   - 更新 `TODO.md` / `PLAN.md` / 本文件。
6. 如果复审通过：
   - 将 `T3016dR` 标记为完成；
   - 更新 `PLAN.md` 与本文件；
   - 提交本轮变更并停止。
