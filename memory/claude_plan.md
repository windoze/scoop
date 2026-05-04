# 当前执行计划

## 范围
- 本次只完成任务列表中的第一个未完成详细任务。
- 以 `TODO-Px.md` 的详细任务状态为准，`TODO.md` 仅作为索引同步。
- 若遇到阻塞当前任务的实现缺口或规格不匹配，先修复；若无法在本次正确完成，则新增最小必要前置任务、同步索引、提交并停止。

## 步骤
1. 读取 `TODO.md`，按索引顺序定位需要检查的详细任务文件。
2. 读取对应 `TODO-Px.md`，找出第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近提交是否明确提到与该任务直接相关的未完成问题；如相关，将其纳入当前任务或登记为前置任务。
4. 理解当前任务要求、依赖、验证命令和完成记录格式。
5. 在最小范围内实现任务；不通过弱化规格、替换表示或 fixture-only hack 绕过问题。
6. 运行相关测试；若失败，修复与当前任务直接相关的问题并重测。
7. 更新详细任务文件：给完成的任务标题加 `[DONE]`，填写或刷新 completion record。
8. 同步 `TODO.md` 中对应条目的 `[DONE]` 标记或任务顺序。
9. 仅在阶段级计划变化时更新 `PLAN.md`。
10. 检查 git diff，提交本次所有相关未提交更改，然后停止。

## 当前进度
- 已读取 `TODO.md` 与 `TODO-P6-part2.md`。
- 已确认本次唯一执行任务为 `P6-T02qe`：发布 refactor source-slice member read/write LLVM lowering contract。
- 最近提交 `[P6-T02qe] Track source-slice member lowering prerequisite` 与当前任务直接相关；详细任务文件已把它登记为 `P6-T03` 的前置 blocker，因此本次继续实现 `P6-T02qe` 本体。
- 已实现 canonical MIR member read/write 的 LLVM lowering helper，并修复 member value temp 类型过宽导致 raw MIR support 回落 HIR-compatible path 的直接 blocker。
- 已补充定向测试覆盖 MIR member read/store codegen，以及 unresolved member metadata / ambiguous continuation route 的 fail-fast。
- 已完成验证（含新增 build fixture）并更新 `TODO-P6-part2.md` / `TODO.md` 的 `[DONE]` 状态；`PLAN.md` 无阶段级变更，未修改。

## P6-T02qe 执行步骤
1. 检查 canonical MIR 中 `Rvalue::MemberAccess`、`StatementKind::StoreMember`、相关 metadata 与 continuation route 的定义。
2. 检查 LLVM generic MIR lowering 中当前拒绝 member read/write 的位置，确认可扩展的 effect-neutral helper 边界。
3. 实现 `MemberAccess` 的 LLVM value lowering，仅消费已 resolved 的 canonical metadata。
4. 实现 `StoreMember` 的 LLVM statement lowering，并对 unresolved/ambiguous/类型漂移情况 fail fast。
5. 补充 `refactor_mir_member_access_codegen` 与 `refactor_mir_store_member_codegen` 定向测试。
6. 运行任务要求的验证命令，修复与本任务直接相关的问题。
7. 标记 `TODO-P6-part2.md` 与 `TODO.md` 中 `P6-T02qe` 为 `[DONE]`，填写完成记录。
8. 检查差异并提交本次相关更改。
