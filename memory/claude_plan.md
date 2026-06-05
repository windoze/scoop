# 当前执行计划

## 范围

- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 本轮只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 如果遇到阻塞当前任务的缺失功能、规格不一致、失败测试或未排期问题，优先修复；若无法在本轮直接完成，则在 `TODO.md` 中加入最小必要前置任务并提交后停止。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务，并检查其依赖、验证要求和完成记录。
2. 查看最近提交信息，确认是否有与该任务直接相关的未完成事项；若有，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 根据任务内容读取最小必要的相关代码、测试、规格或文档，避免无关历史问题扫查。
4. 按任务要求实现最小正确变更；如发现必须先修复的具体阻塞问题，先处理该阻塞或更新 `TODO.md` 后停止。
5. 增加或调整针对当前任务的测试/fixture，确保覆盖指定行为和回归风险。
6. 依次运行格式化、lint、相关测试；在需要全量验证时按要求运行完整 Rust 测试和 fixture 测试并使用足够超时。
7. 若发现未排期失败测试/fixture，修复或在 `TODO.md` 中加入正确顺序的任务，不能将当前任务标记完成。
8. 任务完成后，在 `TODO.md` 中给任务标题加 `[DONE]` 并更新完成记录；仅在阶段计划实际变化时更新 `PLAN.md`。
9. 检查工作区差异，确保只提交预期文件；如本轮是恢复未提交任务，则按要求包含当前未提交文件。
10. 使用符合项目风格的提交信息提交，然后停止，不继续下一项任务。

## 进度记录

- 已创建本计划，下一步读取 `TODO.md` 识别第一个未完成任务。
- 已识别本轮任务：`TC-04-R：Review TC-04`。最新提交 `TC-04-FIX1` 是该 review 的直接前置修复，下一步按 TC-04-R 验收项执行静态检查与抽样审查。
- TC-04-R 静态检查发现旧 `program.callable` / `callable_id_by_root` / `lir_*_to_mir` 模式已清零，但 carrier/dispatch 发布路径仍有 callable root/FQN live layout/facts 选择：`carrier.rs` 的 `published_callable_roots` / `plain_callable_roots` / `callable_layout_for_carrier_target` / `effect_step_callable_facts_for_root`，以及物理 vtable/itable 的 `impl_member_fqn` / `method_impl_fqns` 消费。该问题阻塞 TC-04-R 完成，下一步在 `TODO.md` 中插入最小前置任务 `TC-04-FIX2` 并停止。
- 已在 `TODO.md` 中插入 `TC-04-FIX2：清除 carrier/dispatch 残留 FQN callable 选择`，并更新 `TC-04-R` 依赖与审查阻塞记录。由于本轮只修改任务/计划文档且未改编译产物，按项目规则不运行格式化、lint、完整测试或 fixture；下一步检查差异并提交。
