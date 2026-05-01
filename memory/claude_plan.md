# Claude Plan

## Session Goal

完成 `TODO.md` 索引所对应的第一个“未完成的详细任务”，完成实现、验证、任务记录更新与 git 提交后停止。

## Constraints

- 先以 `TODO.md` 作为索引，再以对应 `TODO-Px.md` 作为任务真实来源。
- 只处理第一个未完成的详细任务。
- 若遇到阻塞当前任务的真实缺口，不绕过；改为在对应 `TODO-Px.md` 中插入最小必要前置任务，并同步 `TODO.md`，然后提交并停止。
- 仅在阶段计划真正变化时更新 `PLAN.md`。
- 需要运行相关测试、修复失败，并确保本次变更有清晰提交。

## Execution Plan

1. 读取 `TODO.md`，确认索引到哪些详细任务文件。
2. 按顺序读取相关 `TODO-Px.md`，定位第一个未完成的详细任务，并确认其完成判定、约束、依赖与记录方式。
3. 检查最近提交是否存在与该任务直接相关且未完成的问题；若有，将其纳入当前任务范围或作为前置依赖记录。
4. 阅读与当前任务相关的代码、测试、文档和现状实现，建立最小充分上下文。
5. 实现当前任务，避免引入与任务无关的改动。
6. 运行相关测试与必要质量检查；若失败，先修复再继续。
7. 更新对应 `TODO-Px.md` 的完成记录；如任务索引、标题、顺序或文件引用变化，则同步更新 `TODO.md`。
8. 若阶段级计划发生变化，再更新 `PLAN.md`；否则不改。
9. 检查工作区状态，使用符合仓库风格的提交信息创建一次 git commit。
10. 停止，并向用户汇报完成内容、验证结果和任何剩余风险。

## Progress Log

- 已读取 `TODO.md` 与 `TODO-P0.md`，确认首个未完成详细任务为 `P0-T03`：建立“共享模块 vs 复制实现”边界清单，并固化为仓库文档。
- 已检查最近提交：`[P0-T02R] Review parallel effect pipeline dispatcher shell` 未声明与 `P0-T03` 直接相关的未完成事项，因此继续按 `P0-T03` 执行。
- 当前执行重点：审阅 `effect_refactor_pipeline`、CLI/session/driver glue 与 `parser`、`hir`、`mir`、`effect`、`llvm`、runtime ABI helper 等关键目录入口；据此产出 `EFFECT_REFACTOR_BOUNDARY_INVENTORY.md`，明确“共享 / 复制 / 后续再判定”分类、理由、单一 API 或后续分叉入口；随后运行任务要求的定向搜索与测试。
- 若审阅中发现当前任务无法在不违背设计基线的前提下定性某个关键模块，需在文档中明确记录“不确定原因”和“后续进入前必须决策”的约束，而不是写模糊结论。
- 已完成关键目录抽样：确认 driver/session/dispatcher 只负责路由；`parser`、`source/span`、`sysroot`、`target`、`ty` 都是中立基础设施；`hir`、`typecheck`、`mir`、`effect`、`llvm` 与 effect/continuation runtime slice 则承载会在后续阶段独立演化的业务 contract。
- 已新增 `EFFECT_REFACTOR_BOUNDARY_INVENTORY.md`，固化共享条目、复制条目、搜索守护规则与 P1-P6 的分叉时点；当前清单没有需要保留为“后续再判定”的条目。
- 已完成验证：`rg -n "EffectPipelineMode|effect_pipeline|effect_pipeline_mode" crates/scoopc/src/parser crates/scoopc/src/source.rs crates/scoopc/src/span.rs crates/scoopc/src/sysroot crates/scoopc/src/target crates/scoopc/src/ty runtime/c/scoop_root_frame.h runtime/c/scoop_stackmap.h runtime/c/scoop_stackmap.c` 为 0 命中；`cargo test -p scoop --no-default-features cli`、`cargo test -p scoopc --no-default-features session`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings` 通过。
- 已将 `P0-T03` 完成记录回写到 `TODO-P0.md`。任务顺序、标题、索引与阶段计划未变化，因此本次无需更新 `TODO.md` 与 `PLAN.md`。
- 本次任务的实现、验证与文档回写已经完成；剩余动作仅是把 `EFFECT_REFACTOR_BOUNDARY_INVENTORY.md`、`TODO-P0.md` 与 `memory/claude_plan.md` 提交入库，然后停止。
