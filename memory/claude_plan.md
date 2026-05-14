## 当前执行计划

说明：按安全与协作要求，这里记录的是可审阅的执行计划、决策依据摘要、关键进展与阻塞，不记录不可审阅的内部推理原文。

### 初始步骤

1. 读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成事项；若是，则将其视为该任务的一部分，或按要求在 `TODO.md` 中补成前置任务。
3. 阅读任务相关代码、测试、文档与约束，确认实现边界与验收方式。
4. 实现该任务，避免以规避、降级或特例方式绕过缺失能力或错误行为。
5. 运行与该任务相关的验证；至少覆盖任务要求的测试，并尽量补充必要回归验证。
6. 更新 `TODO.md`：仅在任务真正完成时将任务标题标记为 `[DONE]`，并补充完成记录；若遇到阻塞，则按依赖顺序添加最小前置任务并保持当前任务未完成。
7. 仅当阶段/依赖结构发生变化时更新 `PLAN.md`。
8. 提交当前任务相关的所有变更，然后停止，不继续下一个任务。

### 进展记录

- 已创建本计划文件。
- 已读取 `TODO.md`，首个未完成任务为 `P4-T03：隔离 array literal synthetic helper call-site identity，修复 enum ctor contract 污染`。
- 已检查最近一次提交：`[P4-T03] Track array-literal call-site blocker`。该提交直接描述当前任务的未完成阻塞，因此视为当前任务的一部分，而不是独立历史问题。

### 当前任务执行计划（P4-T03）

1. 检查与 array literal 合成 helper、typed call-site contract、MIR lowering、LLVM 阶段回归相关的源码与测试。
2. 复现 `refactor_llvm_array_composite_transport` 或更小范围的相关失败，确认污染发生的具体 handoff 点。
3. 在上游修复 synthetic helper call-site identity 冲突，确保 helper call 与数组元素中的用户 call/ctor call 拥有稳定且互不复用的 identity。
4. 补充或强化 direct MIR / LLVM 回归，锁定：
   - 真正的 `__scoop_array_builder_push` 仍保持 helper 形状；
   - 数组元素中的 enum ctor / 普通 call 不再被错误改写成 helper call。
5. 运行任务要求的测试及必要补充验证；若出现直接阻塞当前任务的新前置问题，则按 `TODO.md` 顺序要求回写为前置任务并停止。
6. 若任务完成，回写 `TODO.md` 完成记录并加 `[DONE]`；必要时同步 `PIPELINE_GAPS.md` / `PLAN.md`。
7. 提交本次任务相关全部变更，然后停止。

### 当前执行结果

1. 已定位根因：array literal helper `__scoop_array_builder_push` 复用了元素表达式 span；对 enum variant 这类允许“无 typed contract”的 unresolved callee，会把 helper 的 direct-call contract 错误覆盖到元素自身，导致 direct MIR 里出现单参数 `__scoop_array_builder_push(...)` 假元素。
2. 已实现修复：
   - 为 array builder synthetic helper calls 分配独立稳定的 synthetic call-site span；
   - `build_array_lit_expr(...)` 与 `synth_array_lit_from_exprs(...)` 的 helper `new/push/build*` 已不再复用元素用户 span；
   - 保持 backend 对 malformed `array_builder_push` contract 的严格拒绝，不在下游做猜测兜底。
3. 已补回归：
   - 新增 direct MIR 回归 `refactor_mir_array_literal_helper_calls_keep_distinct_call_contracts`；
   - 现有 LLVM 回归 `refactor_llvm_array_composite_transport` 已恢复通过；
   - `aggregate_transport.mir` golden 已同步到新的 helper call-site identity。
4. 已完成验证：
   - `cargo test -p scoopc refactor_mir_array_literal_helper_calls_keep_distinct_call_contracts -- --nocapture`
   - `cargo test -p scoopc refactor_mir_aggregate_transport_records_composite_contracts -- --nocapture`
   - `cargo test -p scoopc refactor_llvm_array_composite_transport -- --nocapture`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop`
   - `cargo clippy --all-targets -- -D warnings`
5. 已回写文档：
   - `TODO.md`：`P4-T03` 标记为 `[DONE]`，并补齐完成记录；同时把 `P5-T01` 的前置阻塞说明收紧为“已闭合前置”。
   - `PIPELINE_GAPS.md`：`§1.13` 已关闭，`§4.4` / `§4.5` 的描述已改成仅表示真实 composite transport/backend residual。
6. 下一步：检查工作区差异，按任务要求创建提交，然后停止。
