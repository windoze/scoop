# 当前执行计划

说明：这里记录可审阅的执行计划、决策依据摘要和进度更新；不包含隐藏推理链。

## 目标

完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后更新任务记录、运行相关验证、提交 Git，然后停止。

## 步骤

1. 读取 `TODO.md`，按文件顺序识别第一个未完成任务。
2. 查看最新提交信息；如果最新提交明确提到与当前任务直接相关的未完成事项，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 阅读当前任务的要求、依赖和验证标准，只处理该任务，不做开放式历史问题排查。
4. 检查与该任务相关的代码、测试和文档，确认实现边界。
5. 如果发现阻塞当前任务的缺失功能、规格不匹配或实现边界，优先修复；若无法在当前任务中正确完成，则在 `TODO.md` 插入最小必要前置任务并提交后停止。
6. 实现当前任务所需的最小正确代码和测试改动，避免绕过规格或削弱测试。
7. 运行任务要求的验证命令及必要的相关测试；若失败，修复后重新验证。
8. 将任务标题标记为 `[DONE]`，更新完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
9. 检查 Git 状态和差异，提交本次任务涉及的全部改动。
10. 停止，不继续处理下一个任务。

## 当前进度

- 已写入初始执行计划。
- 已读取 `TODO.md`：第一个未完成任务是 `CG-T04b0：发布 value erasure boxing MIR transport contract`。
- 已查看最新提交：`c98f98dd [CG-T04b] Record blocker status`，内容与当前 `CG-T04b0` 前置任务直接相关，将作为当前任务背景处理。
- 已开始实现 MIR contract：新增显式 value erasure transport rvalue、顶层 initializer source type 记录，并在 local/assignment/return/call-arg 等路径插入 `MirBoxingIntent`。
- 已补充 `value_boxing_transport.scoop` fixture 与 `refactor_mir_value_boxing_transport_contract` / `refactor_mir_composite_transport_metadata_contracts` 定向测试。
- 已通过定向验证：`cargo test -p scoopc refactor_mir_value_boxing_transport_contract`、`cargo test -p scoopc refactor_mir_composite_transport_metadata_contracts`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/value_boxing_transport.scoop`、`cargo clippy --all-targets -- -D warnings`。
- 已更新 `TODO.md`：`CG-T04b0` 标题和任务索引标记为 `[DONE]`，并填写完成记录。

## 当前任务执行计划

1. 定位 MIR value transport / boxing metadata 的数据结构、producer、verifier 和现有测试。
2. 找出 tuple/struct/value type 到 `Any` / `Ref` / erased carrier 的 initializer、assignment、return、call arg、effect-neutral handoff 生产路径。
3. 为这些 erasure boundary 发布 `MirBoxingIntent`，保留 `source_ty`、`target_ty`、`MirBoxingReason::{AnyErasure, RefErasure}`，并确保 `ValueTransportMetadata` 保留 source transport kind 与 trace/copy/drop requirements。
4. 增加 MIR production verifier，拒绝缺 boxing intent 的 aggregate erasure boundary；payload-bearing enum erasure 只标识 metadata，不在本任务猜 enum payload layout。
5. 补充或更新 `refactor_mir_value_boxing_transport_contract`、`refactor_mir_composite_transport_metadata_contracts` 及必要 fixture/MIR dump 覆盖。
6. 运行任务要求的定向验证和必要格式化/lint。
7. 更新 `TODO.md` 将 `CG-T04b0` 标记 `[DONE]` 并填写完成记录。
8. 检查差异并提交本次任务全部改动，然后停止。
