# Claude Plan

说明：出于安全与协作可读性考虑，这里记录可审计的执行计划、关键判断依据与进度更新，不记录逐字内部推理。

## 初始计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且尚未完成的问题；如果有，将其视为当前任务的一部分或在 `TODO.md` 中补充为前置任务。
3. 阅读该任务对应说明、依赖、验证要求，并只收集完成该任务所需的最小上下文。
4. 实现该任务；若遇到阻塞当前任务的真实缺陷或缺失特性，不绕过，改为先修复或在 `TODO.md` 中插入最小前置任务。
5. 运行任务要求的验证，以及必要的 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`。
6. 更新文档记录：
   - 将已完成任务在 `TODO.md` 中标记为 `[DONE]` 并补充 completion record。
   - 仅当阶段计划或依赖结构变化时更新 `PLAN.md`。
   - 在本文件追加进度与计划调整。
7. 按仓库约定创建一次 git commit，然后停止，不继续下一项任务。

## 进度日志

- 已创建执行计划文件；下一步读取 `TODO.md` 并识别当前应执行的首个未完成任务。
- 已确认首个未完成任务为 `P8-T03a`：迁移默认单文件 LLVM artifact 入口与默认测试 helper 到 refactor LLVM stage，移除 materialized-HIR entry-main 对 `Handle` fallback 的隐藏依赖。
- 最近一次提交为 `[P8-T03a] Track single-file LLVM stage blocker`，与当前任务直接相关；当前无需新增更早前置任务，直接按 `TODO-P8.md` 中的 blocker 描述实现该任务。

## 当前执行分解

1. 审计默认单文件 LLVM artifact 入口：`crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/effect_refactor_pipeline/mod.rs`、`crates/scoopc/src/bin/scoopc.rs` 以及相关包装层，确认默认路径当前如何接到 `from_materialized_lowered_hir(...)`。
2. 审计默认 LLVM 单测 helper：`crates/scoopc/src/llvm/tests.rs` 与 stage 测试，区分默认生产 helper 和显式历史/对照 helper 的现状。
3. 进行最小必要修改：
   - 默认单文件入口改走 refactor LLVM stage handoff；
   - 保留的 `*_from_lowered_hir` / `*_from_materialized_lowered_hir` 仅作为显式对照 helper；
   - 更新默认单测 helper 与命名/注释，避免隐式回退。
4. 增加或调整回归守护，覆盖：
   - 默认单文件入口会触发 stage；
   - 含 `handle` / `try` 的 `main` 不再命中已删除 HIR lowering；
   - `scoopc` 默认单文件 artifact 路径无 hidden fallback。
5. 运行任务要求的代表性测试、smoke、格式化与 `clippy`。
6. 更新 `TODO-P8.md` 完成记录、将 `TODO.md` 中 `P8-T03a` 标为 `[DONE]`，然后提交本轮改动并停止。

## 当前实现状态

- 已把 `effect_refactor_pipeline::emit_single_file_llvm_artifact_to_file(...)` 改为经 crate 内部的 refactor LLVM stage helper 发射 `.ll/.o/.s`，不再复用默认 `emit_minimal_main_*` 的旧 materialized-HIR 隐式路径。
- 已把 `llvm::emit` 中默认 `build_minimal_main_module_with_opt_level(...)` 改为先构建 single-file refactor stage output，再从 stage handoff 建 module；因此默认 `emit_minimal_main_ir/obj/asm` 已不再间接调用 `from_materialized_lowered_hir(...)`。
- 已保留显式 `*_from_materialized_lowered_hir` helper，但在注释中明确其仅用于显式历史/对照测试，不能再作为默认单文件生产入口。
- 已新增两类回归守护：
  - stage 计数测试：验证默认 `emit_minimal_main_ir` helper 和 public `emit_single_file_llvm_artifact_to_file` 均会触发 refactor LLVM stage；
  - 默认 helper 回归：验证 `main` 含 `handle` 的默认单文件 IR helper 能成功生成 IR，且不会回落旧 effect backend 符号。

## 当前阻塞

- 在运行 `P8-T03a` 定向回归时，`llvm::tests::effect_contract_struct_types_are_registered_for_effect_codegen` 已按 refactor 默认路径改写并通过；但后续继续迁移默认 outward helper 测试时，发现 default single-file refactor stage 在 nominal upcast direct call boundary 上存在真实 stage 缺口：`helper(Derived())` / `helper(Impl())` 一类样本会在 late lowering 失败，报 `local4 的类型为 t388，但 published operand contract 期望 t385`。
- 这不是测试能改回 explicit materialized helper 规避的问题，因为它直接阻塞 `P8-T03a` 要求的“默认 helper/public single-file entry 迁移到唯一 refactor 主线”。
- 因此已在 `TODO-P8.md` / `TODO.md` 中新增前置任务 `P8-T03aa`，并把 `P8-T03a` 依赖更新为 `P8-T03R, P8-T03aa`。

## 本轮结论

- 本轮不继续尝试完成 `P8-T03a`；按工作流要求，在记录 blocker 和前置任务后停止。
- 提交内容将包含：
  - 已完成的默认入口/默认 helper refactor-stage 迁移骨架；
  - 新增的 stage-use / `handle main` 守护测试；
  - 对一条已过时默认测试的 refactor 语义改写；
  - `TODO-P8.md` / `TODO.md` 中新增的 blocker 前置任务与依赖更新。
