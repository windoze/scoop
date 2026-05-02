## 当前执行计划

1. 读取 `TODO.md`，将其仅作为任务索引使用。
2. 按索引顺序打开对应的 `TODO-Px.md`，定位第一个标题未标记 `[DONE]` 的详细任务。
3. 检查最近一次提交是否存在与该任务直接相关且尚未收尾的问题；若是，则将其视为当前任务的一部分或必要前置。
4. 阅读当前任务的详细要求、约束、依赖、验证标准，并结合相关代码实现现状确定最小正确改动。
5. 完整实现该任务；若遇到阻塞当前任务的真实缺口，则先修复，或在对应 `TODO-Px.md` / `TODO.md` 中插入最小必要前置任务并停止。
6. 运行与该任务相关的验证，包括必要的测试、`cargo fmt`、`cargo test`、以及按要求执行 `cargo clippy --all-targets -- -D warnings`（若作用域允许则尽量覆盖当前改动相关范围与仓库要求）。
7. 更新文档记录：在对应 `TODO-Px.md` 中将已完成任务标题前加上 `[DONE]` 并填写完成记录；如任务索引、标题、顺序或状态发生变化，则同步更新 `TODO.md`。仅在阶段计划真的变化时更新 `PLAN.md`。
8. 检查工作区中与本任务相关的未提交更改，按要求创建一次原子提交，然后停止，不继续下一个任务。

## 进度更新约定

- 当我识别出当前目标任务时，更新本文件记录任务编号与目标。
- 当执行计划因真实阻塞而调整时，更新本文件说明原因与新的执行路径。
- 当实现、验证、文档更新、提交等关键步骤完成时，更新本文件记录结果。

## 当前目标任务

- 已根据 `TODO.md` 与 `TODO-P6.md` 确认当前首个未完成详细任务为 `P6-T02R`：Review LLVM type/layout 合同，确认 canonical `Step_F`、frame、continuation ABI 已固定且不再依赖 legacy signal/outcome 模型。
- 最近一次提交为 `[P6-T02] Seal refactor LLVM ABI layout contract`，与当前 review 任务直接相关，因此本次将围绕该提交的实际实现与验证结果完成审阅，不做无关历史问题扩散。

## 当前审阅步骤

1. 阅读 `TODO-P6.md` 中 `P6-T02` / `P6-T02R` 的目标、完成记录与验证要求。
2. 检查 `P6-T02` 涉及的关键文件：`crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 以及相关入口/共享 helper。
3. 搜索 refactor ABI 主实现中是否仍残留 `EffectSignal` / `EffectOutcome` / `LegacyEffectBoundary` 等 legacy contract 作为最终模型。
4. 复跑 `P6-T02R` 要求的测试与命令，确认 review 是否通过。
5. 若审阅发现 blocker，则按约束补充最小前置任务并同步 `TODO.md`；若审阅通过，则将 `P6-T02R` 标记为 `[DONE]`、补全完成记录并提交。

## 当前结果

- 已完成静态审阅与定向搜索，确认 `P6-T02R` 当前存在 blocker，不能直接标记完成。
- blocker 内容：`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 仍会在 `derive_resume_interface_specs()` 中按 `(step_schema, effect_family)` 补齐/合成缺失的 `ResumeInterfaceId`，并让 callable / continuation layout 继续消费按 step 聚合的派生 interface 列表，而不是严格使用 `LateLoweredProgram.resume_interfaces()`、`LateLoweredCallable.resume_interfaces()`、`LateLoweredContinuationObject.implemented_interfaces()` 这组 authoritative handoff。
- 风险：这会掩盖 P5 -> P6 handoff 的 interface identity 漂移，使后续 `P6-T03` body emitter 无法把当前 ABI query 当作无需 remap/reconstruct 的稳定 contract。
- 已据此在 `TODO-P6.md` 中新增最小前置任务 `P6-T02a`，并同步 `TODO.md`；当前 invocation 将按要求在提交这些任务编排变更后停止，不继续 `P6-T02R`。

## 说明

- 本文件记录可审阅的执行计划与关键决策，不包含内部推理细节。
