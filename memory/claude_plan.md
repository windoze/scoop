# 当前执行计划

1. 读取 `TODO.md`，将其作为任务索引使用。
2. 按 `TODO.md` 引用顺序读取对应的 `TODO-Px.md` 详细任务文件。
3. 确认第一个标题未带 `[DONE]` 的详细任务，必要时检查最近一次提交是否存在与该任务直接相关且未完成的问题。
4. 阅读实现所需的最小范围代码与测试，确认约束、依赖、验证要求与现有状态。
5. 直接完成该任务；如果遇到阻塞当前任务的真实前置问题，则在相应 `TODO-Px.md` 中补充最小前置任务并同步 `TODO.md`。
6. 运行与该任务相关的测试、格式化、lint 或构建检查，修复出现的问题，直到满足任务要求。
7. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
8. 在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并填写完成记录；如有必要同步 `TODO.md`，仅在阶段计划变化时更新 `PLAN.md`。
9. 按任务号创建一次 Git 提交，然后停止，不继续处理下一个任务。

## 进展记录

- 已写入初始计划，下一步开始读取任务索引并定位首个未完成的详细任务。
- 已读取 `TODO.md` 与 `TODO-P5.md`，确认首个未完成详细任务是 `P5-T06`（在 late-lowered representation 上加入窄的 devirtualization / inlining / DCE 后处理）。
- 已检查最近一次提交与当前工作树：最近提交为 `P5-T05R` review；工作树中已有 `crates/scoopc/src/effect_lowered/{ir,materialize,mod}.rs`、`crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs` 与新文件 `crates/scoopc/src/effect_lowered/opt.rs` 的未提交修改，判定为与 `P5-T06` 直接相关，后续需在不回退现有变更的前提下完成该任务并最终一并提交。
- 下一步：检查这些未提交变更与现有 late-lowered IR/stage，确认当前已实现了哪些后处理能力、还缺哪些 contract/test，再补齐实现与验证。
- 已确认当前实现方案：新增 `effect_lowered/opt.rs`，在 stage 输出前对 `LateLoweredProgram` 执行窄的 post-lowering 收缩；同时把 `ir.rs` / `materialize.rs` 中需要观察 pre-opt shell 的测试切回 raw builder，避免 stage-level post-opt 行为污染已有 contract 测试。
- 已完成验证：`refactor_late_opt*` 定向测试、`refactor_effect_lowered_stage`、`refactor_late_lowered_ir`、`refactor_step_materialization`、`refactor_boundary_lowering`、`refactor_continuation_object`、`refactor_resume_interface_completeness`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings` 与 `cargo fmt --all --check` 全部通过。
- 已回填 `TODO-P5.md` 的 `P5-T06` 完成记录并同步 `TODO.md` 索引；`PLAN.md` 不需要改动。
- 下一步：检查最终 diff / git status，按任务号创建提交，然后停止。
