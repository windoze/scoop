# 当前执行计划

说明：这里记录可审阅的执行计划、决策依据摘要和进度更新；不包含隐藏推理链。

## 目标

完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务：`CG-T04b：收口 value boxing composite transport lowering`。完成后更新任务记录、运行相关验证、提交 Git，然后停止。

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

- 已读取 `TODO.md`：第一个未完成任务是 `CG-T04b：收口 value boxing composite transport lowering`。
- 已查看最新提交：`c2d7c038 [CG-T04b0] Publish value erasure boxing contract`；该提交是当前任务直接前置，不显示新的未完成 blocker。
- 已定位当前 LLVM lowering 的阻塞点：`Rvalue::Transport` 在 `mir_body.rs` raw/effect-neutral lowering 中仍被拒绝，`codegen_gap_inventory` 仍把所有 value erasure transport gate 到 `CG-T04b`。
- 下一步编辑会复用 `CG-T04a` composite descriptor verifier，为 tuple/struct value erasure 建立 GC-managed boxed object lowering，并继续把 enum payload erasure gate 到 `CG-T04c`。
- 已实现 `Rvalue::Transport` value erasure LLVM lowering：tuple/struct 通过 `scoop_alloc_typed` 分配 value box、写入 payload，并发布 value-box type descriptor；具备 Any/Ref boxing intent 的 raw/materialized MIR 不再被 `CG-T04b` gate 拒绝。
- 已保留 payload-bearing enum erasure 的 `CG-T04c` owner gate，避免在 `CG-T04b` 中猜 enum payload layout。
- 已新增 `refactor_llvm_value_boxing_transport` 单测和 `tests/fixtures/run-pass/value_boxing_tuple_struct_any_basic.scoop` run-pass fixture。
- 已通过验证：`cargo test -p scoopc refactor_llvm_value_boxing_transport`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/value_boxing_tuple_struct_any_basic.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_composite_transport_contract`、`cargo test -p scoopc refactor_mir_value_boxing_transport_contract`、`cargo test -p scoopc refactor_mir_composite_transport_metadata_contracts`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/value_boxing_transport.scoop`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo clippy --all-targets -- -D warnings`。
- 已更新 `TODO.md`：`CG-T04b` 在任务索引和标题中标记为 `[DONE]`，并填写完成记录。

## 当前任务执行计划

1. 定位 `Rvalue::Transport`、`ValueTransportMetadata.boxing`、composite layout descriptor、backend verifier 和现有 gate/test 的实现位置。
2. 跟踪 refactor LLVM raw/materialized MIR lowering 中 value erasure boxing 的当前拒绝点，确认 tuple/struct/value type -> `Any` / `Ref` / erased carrier 应消费的 MIR metadata 与 layout descriptor。
3. 实现最小正确 lowering：按 `MirBoxingIntent` 与 `CG-T04a` descriptor 分配 boxed composite、写入 descriptor identity、复制/存储 payload，并保留 trace/copy/drop hook 可枚举性；缺 metadata/layout 时 fail-fast。
4. 保留 payload-bearing enum boxing 的 `CG-T04c` owner-specific gate，不在本任务猜 enum payload schema。
5. 补充或更新 `refactor_llvm_value_boxing_transport` 测试与 tuple/struct/value type boxing run-pass fixture，确保 `Any` / `Ref` carrier 可用于 runtime type/value 操作或后续 copy/drop。
6. 运行 `cargo test -p scoopc refactor_llvm_value_boxing_transport`、相关 fixture、`cargo test -p scoopc codegen_gap_inventory`，并按项目要求运行格式化/lint；失败则修复后重跑。
7. 更新 `TODO.md`，将 `CG-T04b` 标题和任务索引标记 `[DONE]` 并填写完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
8. 检查 Git 状态和差异，提交本次任务涉及的全部改动，然后停止。
