# 当前执行计划

## 目标

完成任务索引中第一个尚未在详细 TODO 文件标题标记为 `[DONE]` 的任务，验证后提交，并在完成一个详细任务后停止。

## 执行步骤

1. 读取 `TODO.md`，只把它作为全局索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md` 详细任务文件，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；若存在，将其纳入当前任务或作为必要前置任务处理。
4. 阅读该任务的完整要求、依赖、约束、验证命令和完成记录。
5. 检查相关代码、测试和夹具，确认需要修改的最小范围。
6. 实现当前任务；如果发现阻塞当前任务的真实前置缺口，则更新详细 TODO 和索引、提交并停止。
7. 运行任务要求的验证命令和相关回归测试；若失败，修复后重新验证。
8. 在对应 `TODO-Px.md` 中给任务标题加 `[DONE]`，更新完成记录；如索引中有该任务，同步 `TODO.md` 的 `[DONE]` 状态。
9. 按需更新本文件记录关键进度。
10. 提交本次全部相关改动，提交信息包含任务编号和简洁说明。
11. 停止，不继续处理下一个任务。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-P6-part3.md`。
- 第一个未完成详细任务确认为 `P6-T05R：Review P6 阶段退出条件，确认 P7 只需切主线并执行 full regression`。
- 最新提交 `[P6-T06R] Review NoOutward plain ABI` 未声明与当前任务直接相关的未完成 blocker。
- 已通过 P4/P6-T06R no-default effect-facts 验证：plain ABI facts、NoOutward 不发布 StepSchema、plain/effect adapter call-site 区分、effect solver 与 dump-effect-facts。
- 已通过 P5 late-lowered/no-default handoff 验证：plain callable、source-slice ordinary call、lowered stage、continuation object、resume interface 与 NoOutward impl plan。
- 已通过默认 feature 的 P6 LLVM 单元验证：composition/wrapper projection、clean body lowering、call/boundary/handle/continuation/runtime-error、GC/runtime、plain-local control、plain ABI 与 effect-typed adapter。
- `dump-effect-lowered` fixture 命令已通过。
- build fixture 矩阵在 `tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop` 失败：生成的 LLVM IR 未包含 fixture 期望的 `declare %scoop.refactor.Step__fixtures_build_unitWorker @__scoop_refactor_dynamic_invoke__fixtures_build_unitWorker()`。
- 诊断结论：fixture 的 `unitWorker(): Unit / Ping {}` 没有实际 outward case，在 `NoOutward -> Plain` 合同下正确生成普通 ABI；旧 `Step_F` shell 期望已过时。
- 已修正 fixture，让 `unitWorker()` 真实执行 `Ping.tick()`，继续覆盖 Unit 零载荷 invoke/resume ABI 而不依赖 NoOutward 旧 shell。
- 修正后的 Unit payload fixture 已通过。
- 剩余 build artifact、run-pass 与 moving-GC fixture 矩阵已通过。
- 搜索守卫发现 `value.rs` 的 cross-thread resume thunk 仍用 `scoop_runtime_error_fatal(NULL)` 处理 non-Complete step；已改为新增无参数 runtime 终止入口 `scoop_refactor_thread_resume_noncomplete_fatal()`，避免伪造 RuntimeError payload。
- 已完成格式化、相关 runtime/LLVM validations、搜索守卫与 clippy。
- 已在 `TODO-P6-part3.md` 与 `TODO.md` 标记 `P6-T05R` 为 `[DONE]`，并写入 review 完成记录。
- 下一步：检查 git diff/status，提交本次所有相关改动后停止。
