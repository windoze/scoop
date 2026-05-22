本轮执行计划 (拆分 P7-T04 为可独立完成的子任务)

## 背景

P7-T04 (收窄 LLVM stage handoff、physical ABI layout 与 TypeStore bridge) 共有 6 项必须实现要点，覆盖：
- `LlvmCodegenStageOutput` / `StageEmitInput` / `LirStageOutput` 三个 stage handoff 结构的形状重塑；
- `MaterializedBackendContracts` 扩容以收纳 LLVM 仍需的 HIR scaffold side tables；
- `effect_lowered/layout/abi.rs` 等 physical ABI/layout 路径从 `codegen.class_inits/enum_layouts` 等 HIR side table 迁到 `lir_facts.physical_layout` / `lir_facts.type_context`；
- `TypeId` cross-process stable wire-format 显式推迟。

经现场评估，6 项要点之间的耦合面、测试 fixture 触及面（含 effect_lowered/layout/tests、llvm_codegen_stage tests、run-pass fixture suite）以及生产代码命中面（`codegen/mir_body/`、`codegen/main/`、`codegen/call/`、`codegen/ty.rs`、`codegen/ordinary_callee.rs` 等 14+ 个生产文件）都不允许在单一 patch 中安全完成而不留下中间残破态。继续保持单一 P7-T04 既有阻塞 review 流程，也违反“class-wide fix, no localised patch”的纪律。

按 P7 系列已经成形的拆分模式（P7-T02-a / P7-T03-a / P7-T04-a 都是被拆出的前置/后置子任务），把剩余 P7-T04 拆成两个互相独立、各自可整体验证的子任务：

- `P7-T04-b`：收窄 stage handoff 结构形状（覆盖原 1/2/3/4/6 项要点）。不动 physical ABI/layout 内部读法。
- `P7-T04-c`：迁移 physical ABI/layout 查询面到 LIR facts（覆盖原 5 项要点）。不动 stage handoff 形状。

P7-T04 本身保留为收尾节点：在 -b 与 -c 都完成且互不留死代码后做交叉复核与最终验证（同名 `P7-T04R` 已经在 TODO-6.md 中存在，作为 -b/-c 完成后的合并 review）。这样：
1. -b 与 -c 各自可在一轮内完整推进、做 build/clippy/fixture 验证；
2. 形状改动（-b）不会被层层 layout 迁移（-c）拖进同一个 patch；
3. P7-T04 closure 不再背负实现，而是按 P7-T04R 已有 review 检查表统一裁定。

## 本轮要做的事

1. 在 `TODO-6.md` 中写入新任务 `P7-T04-b` 与 `P7-T04-c` 的完整任务卡（目标、必须修改位置、必须实现内容、验证清单、完成条件、依赖）。
2. 调整 `TODO-6.md` 中 `P7-T04` 已有任务卡：把它改为收尾任务，依赖 `P7-T04-b` 与 `P7-T04-c`，仍负责 wire-format 推迟的最终落地与全任务验证。
3. 同步 `TODO.md` 索引：插入 `P7-T04-b` 与 `P7-T04-c` 行，并保留 `P7-T04` / `P7-T04R` 在原位但更新依赖。
4. 还原上一轮在 `crates/scoopc_types/src/lib.rs` 投机性追加给 `TypeId` 的 `Default` derive（目前已撤销）。
5. 不在本轮内动 `crates/scoopc/src/llvm/`、`crates/scoopc/src/pipeline/llvm_codegen_stage.rs`、`crates/scoopc/src/pipeline/effect_lowering_stage.rs` 等实现文件。
6. 验证：`cargo fmt`；`cargo build -p scoopc --no-default-features` 确保 baseline 仍 clean；`git diff --check`。
7. 提交本轮拆分作为独立 commit（信息形式：`[P7-T04 split]`），然后停止。

## 不在本轮内做的事

- `MaterializedBackendContracts` 扩容；
- `LlvmCodegenStageOutput` / `StageEmitInput` / `LirStageOutput` 形状改造；
- `llvm_residual_pass_view()` 删除；
- physical ABI/layout 迁移；
- run-pass fixture 全量验证（拆分本身不会改变运行时行为）。

这些都进入 `P7-T04-b` 或 `P7-T04-c` 的工作范围，由后续会话承接。
