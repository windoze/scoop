# 当前执行计划

## 约束
- 只处理 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后停止。
- `TODO.md` 是任务排序、依赖、验证和完成记录的权威来源。
- 如遇阻塞当前任务的缺陷、缺失特性或未排期失败，先修复；若不能在当前任务内正确修复，则在 `TODO.md` 中新增最小前置任务并提交后停止。
- 不使用规避方案、降级夹具或改变预期建模方式来绕过语言/运行时/测试缺陷。
- 仅在阶段级计划、依赖或完成标准变化时更新 `PLAN.md`。

## 步骤
1. 读取 `TODO.md`，定位第一个未完成任务，并确认最近提交是否指出与该任务直接相关的未完成事项。
2. 读取该任务涉及的说明、相关源码和测试，确认实现边界与验证要求。
3. 按任务要求实施最小正确变更；如发现必须先处理的具体前置问题，更新 `TODO.md` 后提交并停止。
4. 运行与变更相关的定向测试；必要时运行更广的验证命令，确保未留下未排期失败。
5. 将任务标题标记为 `[DONE]`，更新任务完成记录；如计划结构未变，不更新 `PLAN.md`。
6. 检查工作区差异，提交本次任务相关的全部未提交文件，提交信息使用任务编号和简明说明。
7. 停止，不继续下一个任务。

## 进度
- 已创建初始执行计划。
- 已读取 `TODO.md` 与最近提交；第一个未完成任务确定为 `P7-T04-b：收窄 LLVM stage handoff 形状`。
- 当前任务验证范围：`cargo fmt`、`cargo test -p scoopc --no-default-features llvm_codegen_stage`、`cargo test -p scoopc --no-default-features pipeline::effect_lowering_stage`、`cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`、`cargo run -p scoop_tools -- dependency-gate`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
- 执行重点：引入显式 LLVM stage base context，收窄 `LlvmCodegenStageOutput`、`StageEmitInput`、`LirStageOutput` 的公共输入形状，删除 `LirStageContext` 与 `llvm_residual_pass_view()` residual accessor，并保留 TypeId wire-format 推迟决策记录。
- 已完成第一轮实现：`LirStageOutput` 不再保存 MIR/effect residual；LLVM stage 新增 `LlvmStageBaseContext`，`LlvmCodegenStageOutput` 与 `StageEmitInput` 改为携带 LIR/LIR facts/base context；layout 测试 helper 改为从 base context 构造 codegen 输入。
- 默认 LLVM layout tests 初次运行发现 TypeStore owner 选择错误：LIR program 使用 effect-owned TypeStore，而 base context 暴露 materialized MIR TypeStore，已改为由 `LlvmStageBaseContext` 暴露 effect-owned TypeStore，并继续让 `MaterializedMir` 承担 backend contracts owner。
- `run-pass` 初次验证发现 `sysroot_atomic_basic.scoop` 缺失 `Atomic<Box<Pair>>` layout key；已把 base context 的 backend contract side tables 改为显式合并 MaterializedMir contracts 与 HIR lowering 后的 side tables，避免继续依赖 `LoweredHir` 整包同时补齐 materialized 前缺失的 generic class layout 条目。
- 已完成验证并更新 `TODO.md` / `TODO-6.md`：`P7-T04-b` 标记为 `[DONE]`，完成记录包含实现摘要、`sysroot_atomic_basic` 修复和全部验证命令结果。
