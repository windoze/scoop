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

- 2026-05-02（当前轮次继续）：重新按 `TODO.md -> TODO-P0.md -> TODO-P1.md -> TODO-P2.md -> TODO-P3.md` 顺序核对完成记录，确认当前首个未完成详细任务为 `TODO-P3.md` 中的 `P3-T02`：`把 P2 typed contract 下沉到 direct-style MIR，停止基于 span / 名字 / HIR fallback 猜测 Call / Perform / Resume / Handle 语义`。
- 已检查最新提交信息为 `[P3-T01R] Confirm refactor MIR stage split`；提交信息未显式列出与 `P3-T02` 直接相关且必须先处理的未完成事项，因此继续执行 `P3-T02`。

## 当前轮次执行计划（P3-T02）

1. 阅读 `P3-T02` 涉及的 MIR lowering、P2 typed contract side table、MIR stage 输出与 `dump-mir` formatter，确认 refactor 路径当前仍在哪里依赖 `Span` / 名字 / fallback 猜测语义。
2. 若发现现有实现缺少当前任务必需的信息下沉，则在 refactor MIR 路径中最小化补齐结构化 contract：优先让 stage/lowering 直接消费 P2 side tables，并把 `Call / Perform / Resume / Handle` 的 typed metadata 以 `SiteId` 或稳定 body-local 键暴露到 MIR/stage 输出。
3. 为 refactor `dump-mir` 或稳定 formatter 增加可验证输出，确保测试能直接断言 MIR 不再依赖旧的 span/name 猜测入口。
4. 新增或更新 `refactor_mir_lowering_contract_*` 定向测试与 `tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop` 等样本；验证 `resume` 的 unit sugar canonical 合流、`perform` payload contract、`handle` contract 以及 runtime error ordinary effect 合同在 MIR 中可见。
5. 运行任务要求的定向测试、CLI smoke 和 `clippy`；若遇到真实前置阻塞，则按要求把最小前置任务插入 `TODO-P3.md` / `TODO.md`，记录 blocker 后提交并停止。
6. 若实现与验证完成，则回写 `TODO-P3.md` 的 `P3-T02` 完成记录、同步本文件进度，并按仓库约定提交一次 git commit 后停止。

## 当前轮次进展（P3-T02）

- 已确认当前真正阻塞 `P3-T02` 的具体缺口：P2 typed HIR 会把 `k.resume(...)` canonicalize 成顶层 `scoop.core.Continuation.resume(receiver, payload)` 调用，而现有 MIR lowerer 仍只接受 `receiver.member` 形状，因此 `dump-mir --effect-pipeline refactor tests/fixtures/mir/dispatch_and_resume_call.scoop` 会落成 `Todo("resume callee lowering pending")`。这不是可绕过现象，必须直接修复为 typed-contract 驱动 lowering。
- 已在 `crates/scoopc/src/mir/mod.rs` / `mir/lower.rs` 中把 refactor MIR 需要的 typed contract 下沉到 MIR metadata：
  - `ResumeMetadata` 现在显式携带 `ResumeTuple`、`Answer`、`return_ty`、`Out` effect row、`Raise<RuntimeError>` effect，以及 `suspends_outward`；
  - `PerformMetadata` 现在显式携带 payload component types 与 `arg_mapping`；
  - `Handle` terminator 现在显式携带 `HandleMetadata` 与 richer `HandlerArm` metadata（handled effect、payload shape、body type、arm kind）。
- 已在 `crates/scoopc/src/mir/lower.rs` 中新增 `MirLoweringFacts::with_refactor_typed_contracts(...)`，让 refactor MIR stage 直接消费 `TypedHirEffectContracts`，优先使用按 `source_path + span` keyed 的结构化 contract，而不是继续依赖旧的 resume span 集合或 callee 名字/形状猜测。
- 已修复 refactor/legacy 共享 lowerer 对 typed `Continuation.resume` canonical 形状的支持：当调用点已由 typed contract 确认为 continuation resume 时，MIR lowering 现在既能处理旧的 member-call 形状，也能处理 P2 canonical 的顶层 `Continuation.resume(receiver, payload)` 形状，并在 MIR 中产出显式 `CallKind::Resume`。
- 已新增 `tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop`，并在 `crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs` 新增 `refactor_mir_lowering_contract_*` 测试，覆盖：
  - direct / fun-value / virtual / interface / resume 调用分类；
  - `perform` / `handle` typed metadata 下沉；
  - `k.resume()` / `k.resume(())` 与 `takesUnit()` / `takesUnit(())` 在 MIR 中统一为显式单 `Unit` 参数调用。
- 已按 P3 阶段约束放松 `crates/scoop/src/commands/parity.rs` 中的 `dump-mir` 守门：自 P3 起 refactor `dump-mir` 可以携带 richer metadata，不再要求与 legacy stdout 完全 parity；当前改为要求 legacy/refactor 退出状态一致、stderr 一致且双方都能成功产出输出。
- 已完成验证并通过：
  - `cargo test -p scoopc --no-default-features refactor_mir_lowering_contract`
  - `cargo test -p scoopc --no-default-features refactor_direct_mir_stage`
  - `cargo test -p scoopc --no-default-features effect_refactor_pipeline`
  - `cargo test -p scoop --no-default-features dump_mir`
  - `cargo test -p scoop --no-default-features parity`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/direct_and_fun_value_call.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/dispatch_and_resume_call.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-mir tests/fixtures/mir/dispatch_and_resume_call.scoop`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 下一步：回写 `TODO-P3.md` 的 `P3-T02` 完成记录，检查 git 状态并创建本轮提交，然后停止。
