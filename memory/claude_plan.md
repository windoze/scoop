# 当前执行计划

## 约束
- 以 `TODO.md` 为唯一任务顺序来源，只处理第一个标题未带 `[DONE]` 的任务。
- 不跳到后续任务，不做开放式历史问题清扫。
- 若当前任务遇到直接阻塞的规格或实现缺口，先修复；若无法在本次正确完成，则在 `TODO.md` 中插入最小必要前置任务、提交并停止。
- 完成后必须更新 `TODO.md` 的任务标题为 `[DONE]`，填写完成记录，运行相关验证，并提交全部相关改动。

## 步骤
1. 阅读 `TODO.md`，确定第一个未完成任务及其依赖、验证要求和完成记录格式。
2. 查看最新提交信息，判断是否明确提到与该任务直接相关的未完成问题。
3. 阅读当前任务涉及的代码、测试、规格或 fixtures，限定范围内定位实现点。
4. 按任务要求做最小正确实现，不使用 workaround 或削弱测试形态。
5. 添加或更新必要测试/fixture，并运行任务指定验证；若出现相关失败，继续修复并复测。
6. 更新 `TODO.md`：仅在任务完成时给当前任务标题加 `[DONE]` 并记录验证结果；只有阶段级计划变化时才更新 `PLAN.md`。
7. 运行必要的格式化/检查，查看 git 状态和 diff，提交本次任务相关所有改动。
8. 提交后停止，不处理下一个任务。

## 进度
- 已读取 `TODO.md`，首个未完成任务为 `MIR-T10R：Review MIR-T10 composite transport contract`。
- 最新提交为 `[MIR-T10] Add composite transport contracts`，与当前 review 任务直接相关；本次将把该提交内容作为 review 范围。
- 初轮验证已通过：`refactor_mir_aggregate_transport`、`aggregate_transport.scoop` dump、`refactor_materialized_mir`。
- Review 发现并修复 production verifier 缺口：aggregate transport fields 与 perform payload transport 现在会反查实际 lowered operand/component type，并新增对应负例测试。
- 复测已通过：`refactor_mir_aggregate_transport`、`aggregate_transport.scoop` dump、`refactor_materialized_mir`、`refactor_mir_no_todo`、`refactor_mir_placeholder_inventory`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
- 已更新 `TODO.md`，将 `MIR-T10R` 标记为 `[DONE]` 并记录 review 结论、修复和验证结果。

## MIR-T10R 执行步骤
1. 阅读 `MIR-T10` 相关实现、transport metadata、strict verifier、materializer substitution/verifier、fixture 与测试。
2. 重跑 `MIR-T10` 的验证命令：`refactor_mir_aggregate_transport`、`aggregate_transport.scoop` dump、必要 materialized/no-todo/inventory 回归。
3. 抽查 `aggregate_transport.scoop` dump，确认 tuple/struct/enum/array/closure capture/effect payload 均有 explicit transport metadata。
4. 检查 materialized MIR/代码路径，确认 aggregate/closure/effect metadata 不保留裸 type param 或 source-shape fallback。
5. 若发现阻塞缺口，修复并复测；若无法正确修复，则更新 `TODO.md` 记录前置缺口并停止。
6. 若 review 通过，更新 `TODO.md` 将 `MIR-T10R` 标记为 `[DONE]` 并填写 review 记录，提交全部相关改动后停止。
