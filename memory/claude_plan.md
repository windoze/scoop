# 当前执行计划

## 约束

- `TODO.md` 是任务顺序和完成状态的权威来源。
- 本次只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 若当前任务被实现缺口或规格不匹配阻塞，先在 `TODO.md` 中加入最小必要前置任务并提交后停止。
- 完成任务后需要更新 `TODO.md`、运行相关验证、提交 Git commit。

## 初始步骤

1. 读取 `TODO.md`，定位第一个未完成任务。
2. 查看最新提交信息，判断是否明确提到与当前任务直接相关的未完成事项。
3. 读取当前任务涉及的源码、测试和文档，确认需求、依赖和验证命令。
4. 实现当前任务要求的最小正确变更，不绕开规格或降低测试覆盖。
5. 运行当前任务要求的验证；若失败，修复与当前任务直接相关的问题。
6. 将任务标题标记为 `[DONE]` 并更新完成记录。
7. 提交所有本次任务相关变更，然后停止。

## 进展记录

- 已创建初始执行计划，下一步读取 `TODO.md` 定位当前任务。
- 已定位当前任务：`CG-T04d：收口 array composite element transport lowering`。
- 最新提交为 `[CG-T04c] Implement enum payload transport lowering`，与当前任务前置链相关但未提示需要先插入的新阻塞任务。
- 已确认 MIR `CallTransportMetadata.array` / `ArrayElementTransportMetadata` 已覆盖 builder push/build、get、set，并携带 `ValueTransportMetadata`；当前任务可直接消费该 contract。
- 已实现 runtime composite array ABI 与 refactor LLVM descriptor-backed lowering，并新增 `refactor_llvm_array_composite_transport` 单测和 `array_composite_transport_basic.scoop` fixture。
- 定向测试发现 composite `Array.get` 的 materialized MIR 被未具体化的 call-site binding 阻断；已修复 direct-call materialization，使非具体 site binding 会回退到基于参数/结果的实例推断。
- 继续修复 materialized array call metadata：从 materialized receiver/result 恢复 concrete element type，并把 array set 的泛型 erasure 临时值还原为 concrete source operand，避免 `T` 泄漏到 pass-view frame slot。
- 已完成验证并将 `TODO.md` 中 `CG-T04d` 标记为 `[DONE]`；最终 clippy 与 array/composite 定向回归均已通过，下一步提交本次任务变更。

## CG-T04d 执行计划

1. 阅读 array lowering、composite transport descriptor、runtime array descriptor 与现有相关测试。
2. 确认当前 array element metadata 是否已经携带 size、align、trace/copy/drop 与 inline/boxed policy；若缺少 upstream contract，按规则先更新 `TODO.md` 并停止。
3. 实现 composite array build/get/set lowering，确保 tuple/struct/enum element 不走 `u64`/word 静默截断路径。
4. 扩展 C runtime array/builder 表示，使 composite element descriptor 驱动 element size/align、trace/copy/drop、get/set copy。
5. 补充定向 Rust 测试与 run-pass fixture，覆盖 tuple/struct/enum composite element 的 array build/get/set。
6. 运行 `CG-T04d` 要求的验证命令和必要回归，修复与当前任务直接相关的失败。
7. 更新 `TODO.md` 将 `CG-T04d` 标记为 `[DONE]` 并记录验证结果。
8. 提交本次任务全部变更后停止。
