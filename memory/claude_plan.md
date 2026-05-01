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

- 已创建本计划文件，并记录初始执行计划。
- 已读取 `TODO.md` 与 `TODO-P0.md`，确认首个未完成详细任务为 `P0-T02R`：review 并行 dispatcher 壳层，确认新旧主线只在 CLI / session / dispatcher 分流，未渗入低层业务实现。
- 当前执行重点：检查最新提交是否声明了与 `P0-T02R` 直接相关的未完成事项；审阅 `effect_refactor_pipeline`、driver 命令入口与低层业务目录；运行任务要求的搜索与定向验证；若 review 发现真实问题，则先修复或补最小前置任务。
- 已检查最近提交：`[P0-T02] Establish parallel effect pipeline dispatcher shell` 未声明与本 review 直接相关的未完成项，因此继续按 `P0-T02R` 正常审计。
- 已完成代码审计：`EffectPipelineMode` 的分流仍收口在 `crates/scoopc/src/effect_refactor_pipeline/`；`scoop` 命令层与 `scoopc` 二进制入口都通过 dispatcher/wrapper 进入阶段边界；`crates/scoopc/src/hir`、`mir`、`effect`、`llvm` 中对 `EffectPipelineMode|effect_pipeline|effect_pipeline_mode` 的搜索均为 0 命中。
- 已完成验证：`scoop`/`scoopc` 定向测试全部通过；`dump-ast`、`dump-hir`、`dump-mir` 的 legacy/refactor 输出一致；`dump-ir` 在本次复验中两种 mode 也都成功并通过文本比较；`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings` 通过。
- 已将 `P0-T02R` 完成记录回写到 `TODO-P0.md`。任务顺序、索引与阶段计划未变化，因此本次无需更新 `TODO.md` 与 `PLAN.md`。
- 下一步：检查工作区 diff，仅提交本次 review 相关变更（`TODO-P0.md` 与 `memory/claude_plan.md`），然后停止。
