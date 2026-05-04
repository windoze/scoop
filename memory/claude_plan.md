# 执行计划

1. 先读取 `TODO.md`，仅把它当作任务索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md` 详细任务文件，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否有与该任务直接相关且未完成的问题；若有，将其并入当前任务或在对应详细任务文件中登记为前置依赖。
4. 实施当前任务所需的最小正确修改，不采用规避性方案。
5. 运行与该任务直接相关的验证；若任务落地，补充必要测试并执行格式化、测试与 lint。
6. 更新 `memory/claude_plan.md` 记录关键进展或计划调整。
7. 在对应 `TODO-Px.md` 中把已完成任务标题标记为 `[DONE]` 并更新完成记录；如有新增前置任务或顺序变化，同步更新 `TODO.md`，仅在阶段计划变化时更新 `PLAN.md`。
8. 按仓库提交风格创建一次 git 提交，然后停止，不继续下一个任务。

## 进展

- 已创建初始执行计划，下一步开始定位首个未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P6-part2.md`，确认当前首个未完成详细任务为 `P6-T02q`。
- 已检查最近提交：最新提交为 `[P6-T02qa] Publish continuation write-read provenance`，与当前任务直接相邻且构成前置条件，但未额外声明新的未完成事项。
- 已确认工作区存在与 `P6-T02q` 直接相关的未提交修改：`effect_lowered/{ir,materialize,dump}.rs`、`llvm/codegen/effect_refactor/layout.rs`，因此本次按“恢复并完成当前任务”处理。
- 当前收尾计划：
  1. 审核现有未提交实现是否已经完整发布 `resume-boundary wrapper -> underlying continuation surface route` contract。
  2. 补齐缺失的代码、dump 展示、ABI query 校验或定向测试。
  3. 运行任务要求的定向测试与 `clippy`，确认无 warning。
  4. 若验证通过，则把 `P6-T02q` 标记为 `[DONE]`，更新完成记录并提交当前所有未提交文件。
- 已补齐并确认的实现点：
  1. `LateLoweredResumeBoundaryOperandContract` 现在强制携带 `underlying_continuation_route`，不再允许缺省。
  2. late lowering 会优先从 `P6-T02qa` 发布的 member write/read provenance 解析底层 route；若没有更深 provenance，则发布 boundary 自身的 self-route。
  3. LLVM ABI/query 校验已把该 bridge 视为 published contract，并对缺失 publication / publication 漂移 fail fast。
  4. `dump-effect-lowered` 已稳定输出 `underlying_route:`，便于检查 boundary-local wrapper 到底层 surface route 的桥接结果。
- 已完成验证：
  1. `cargo test -p scoopc refactor_boundary_lowering_`
  2. `cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout`
  3. `cargo test -p scoopc refactor_llvm_boundary_operand_contract`
  4. `cargo test -p scoopc refactor_llvm_`
  5. `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  6. `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 已完成任务文档收尾：`TODO-P6-part2.md` 与 `TODO.md` 已同步把 `P6-T02q` 标记为 `[DONE]`。
- 下一步仅剩检查最终 diff、提交本次任务相关未提交文件，然后停止。
