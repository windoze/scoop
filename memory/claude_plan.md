# 当前执行计划

## 约束摘要
- 先读取 `TODO.md` 作为索引，再按索引打开对应的 `TODO-Px.md` 详细文件。
- 只处理第一个标题未带 `[DONE]` 的详细任务；完成后停止。
- 若发现当前任务被具体缺陷或缺失功能阻塞，先修复阻塞项；若不能在本轮修复，则在合适的 `TODO-Px.md` 中插入最少的新前置任务，并同步 `TODO.md` 后提交停止。
- 不用变通、降级夹具或特例绕过规范行为。
- 完成任务后更新详细任务的 `[DONE]` 标记和完成记录，并同步 `TODO.md` 中对应索引项。
- 仅当阶段级计划变化时更新 `PLAN.md`。
- 完成实现和验证后提交 Git，然后停止。

## 步骤计划
1. 检查当前仓库状态和最近提交，确认是否有与首个未完成任务直接相关的未完成事项或工作区改动。
2. 读取 `TODO.md`，按索引顺序定位需要检查的 `TODO-Px.md` 文件。
3. 打开对应详细任务文件，找到第一个标题未以 `[DONE]` 标记的任务，并记录其需求、依赖和验证要求。
4. 基于任务要求检查相关代码、测试和夹具，确定最小实现范围。
5. 实现任务，避免修改无关文件或覆盖他人改动。
6. 运行任务要求的验证命令以及必要的相关测试；若失败，定位并修复。
7. 更新对应 `TODO-Px.md` 的任务标题和完成记录，同步 `TODO.md` 中索引项；仅在阶段计划确实变化时更新 `PLAN.md`。
8. 查看最终 diff，确保没有秘密文件、无关回滚或意外改动。
9. 提交本轮所有相关改动，提交信息使用任务编号和简明描述。
10. 停止，不继续处理下一个任务。

## 进度记录
- 已创建本执行计划，下一步将读取任务索引并定位首个未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P6-part3.md`，确认首个未完成详细任务是 `P6-T03f：闭合 boundary lowering，覆盖 Call / Perform / Resume / runtime-error / nested-handle outward`。
- 最近提交为 `[P6-T03e] Record completion status`，未发现其显式指出与 `P6-T03f` 直接相关的未完成阻塞；当前工作区仅有本计划文件改动。
- 下一步将检查 refactor LLVM body/layout/types 与 effect-lowered handoff 中现有 boundary lowering 能力，按任务要求补齐缺口并运行指定验证。
- 初次运行 `P6-T03f` run-pass 验证时，两个 fixture 都先在 `scoop.core.println::<Int>` 的 pure source slice 上失败：`TopLevelRef(__scoop_println_string)` 尚未被 refactor value primitive 支持。该问题直接阻塞当前任务验证，先作为当前任务前置修复处理，然后继续 boundary lowering。
- 已实现 P6-T03f：补齐 refactor boundary lowering 的 `RuntimeError` / `Handle` 显式分支，保留既有 `Call` / `Perform` / `Resume` 路径；同时修复阻塞验证的 sysroot internal print 与 primitive/String `ToString` refactor value primitive。
- 已运行并通过任务验证：`cargo test -p scoopc refactor_llvm_boundary_lowering`、`cargo test -p scoopc refactor_llvm_runtime_error_case`、两个指定 run-pass fixture、相关 value/dynamic 单测，以及 `cargo clippy --all-targets -- -D warnings`。
- 已将 `P6-T03f` 在 `TODO-P6-part3.md` 与 `TODO.md` 标记为 `[DONE]`，`PLAN.md` 无阶段级变更，无需更新。
- 下一步是检查最终 diff 并提交本轮改动，提交后停止。
