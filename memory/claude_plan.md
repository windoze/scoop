# Claude Plan

## 说明

用户要求先写计划再执行其他命令。我不会写入逐字的私有推理，但会在此维护可审阅的执行计划、关键判断依据摘要和进度更新。

## 初始执行计划

1. 读取 `TODO.md`，把它当作任务索引而不是详细说明来源。
2. 按 `TODO.md` 引用顺序读取对应的 `TODO-Px.md` 文件，定位第一个未完成的详细任务。
3. 检查最近一次提交信息，确认是否存在与该当前任务直接相关且明确未完成的问题；若有，则把它视为当前任务的一部分或在对应 `TODO-Px.md` 中加入最小前置任务。
4. 阅读当前任务涉及的代码、规格、测试和依赖约束，确认实现边界与验收条件。
5. 如无阻塞，直接完整实现该任务；如遇会阻止规格正确实现的真实缺口，则先新增最小前置任务并同步 `TODO.md`，然后停止。
6. 运行与该任务相关的测试与必要的质量检查，优先保证任务相关验证通过；若任务影响范围足够大，再运行更广泛的校验。
7. 更新对应 `TODO-Px.md` 的完成记录；如果任务编号、标题、顺序或依赖变化，同步更新 `TODO.md`；仅当阶段计划确实变化时才更新 `PLAN.md`。
8. 检查工作区变更，避免误改他人未要求变更。
9. 按仓库约定创建一次 git 提交，提交信息包含当前任务编号。
10. 停止，不继续处理下一个任务。

## 进度日志

- 已创建本文件并记录初始计划。
- 已读取 `TODO.md` 索引并检查 `TODO-P0.md`、`TODO-P1.md`、`TODO-P2.md`、`TODO-P3.md` 的完成记录。
- 已确认首个未完成详细任务为 `TODO-P3.md` 中的 `P3-T01R`：`Review refactor MIR stage 入口，确认新路径已与 legacy mir::lower_for_dump 分离`。
- 已检查最新提交信息为 `[P3-T01] Route refactor dump-mir through MIR stage`；提交信息未显式列出会阻塞当前 review 的未完成前置事项，因此继续执行 `P3-T01R`。

## 当前任务执行细化

1. 阅读 `P3-T01R` 要求涉及的文件：refactor `mir_stage`、`dump_mir` 命令、fixture helper、legacy MIR lowering/materialize、以及 P2 到 P3 的 handoff 位置。
2. 验证 refactor `dump-mir` 是否经由独立 MIR stage，而不是继续换壳调用 legacy `mir::lower_for_dump`。
3. 验证 stage 输出是否已把 P4 需要的 canonical MIR handoff 显式化。
4. 搜索 `crates/scoopc/src/mir` 与 `crates/scoopc/src/effect_refactor_pipeline`，确认没有把 pipeline selector 下沉到旧 `mir/lower.rs` / `mir/materialize.rs` / `mir/pass_view.rs` 业务函数。
5. 运行 `P3-T01R` 要求的定向测试、命令和 `clippy`。
6. 若 review 通过：回写 `TODO-P3.md` 的 `P3-T01R` 完成记录，并按需更新本文件进度，然后创建一次 git 提交并停止。
7. 若发现阻塞问题：先修复；若无法在本轮直接修复，则按要求在详细 TODO 中插入最小前置任务并同步索引，然后提交并停止。

## review 结果摘要

- `crates/scoop/src/commands/dump_mir.rs` 已在命令边界按 `EffectPipelineMode` 分流：`legacy` 继续直连 `scoopc::mir::lower_for_dump(...)`，`refactor` 改走 `scoopc::effect_refactor_pipeline::load_direct_style_mir_stage_output_for_dump(...)`，说明 `dump-mir --effect-pipeline refactor` 已从 legacy lowerer 分离。
- `crates/scoopc/src/effect_refactor_pipeline/refactor.rs` 已把 `StageKind::DirectStyleMir` 显式收口到 `mir_stage::run(...)`；`crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs` 中 `RefactorMirStageOutput` 已显式承载 direct-style `LoweredMir`、`TypeStore`、typed HIR effect contracts、callable body 查询面以及可选 `materialized_mir` handoff。
- `crates/scoop/src/fixtures/mod.rs` 的 `mir_fixture(...)` 已统一经 `scoopc::effect_refactor_pipeline::lower_direct_style_mir_for_dump(...)` 进入，因此 refactor fixture 路径也继承同一 stage 入口，而不是直连 legacy `mir::lower_for_dump(...)`。
- `crates/scoopc/src/hir/lower/types.rs` 中 `LoweredHir::into_materialized_mir()` 已把原先挂在 `LoweredHir` 私有字段上的 canonical materialized MIR handoff 显式暴露给 refactor MIR stage。
- 搜索结果表明 `crates/scoopc/src/mir/**/*.rs` 中没有 `EffectPipelineMode` / `effect_pipeline` / `legacy` / `refactor` selector 分支命中；命中的 `pipeline` 仅为 `mir/lower.rs` 顶部注释。selector 相关命中集中在 `effect_refactor_pipeline/` dispatcher 与测试中，未渗入旧 `mir/lower.rs` / `mir/materialize.rs` / `mir/pass_view.rs` 业务函数。
- 复验通过：`cargo test -p scoopc --no-default-features refactor_direct_mir_stage`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline`、`cargo test -p scoop --no-default-features dump_mir`、`cargo test -p scoop --no-default-features parity`、`cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-mir tests/fixtures/mir/direct_zero_arg_call.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/direct_zero_arg_call.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/direct_and_fun_value_call.scoop`。
- 已补跑并通过：`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。
- 已回写 `TODO-P3.md` 中 `P3-T01R` 的完成记录。
- 下一步：检查 git 状态，确认本轮只提交 `TODO-P3.md` 与 `memory/claude_plan.md` 的变更，然后创建提交并停止。
