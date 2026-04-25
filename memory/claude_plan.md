## 当前执行计划（可验证摘要）

说明：按要求先记录执行计划与决策摘要；这里保存的是可验证的步骤、假设与变更记录，不包含逐字内部推理。

### 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行前或执行中发现更早应修复的既有问题，则先修复该问题，或者把它整理为新的前置任务插入 `TODO.md`，更新 `PLAN.md` 后停止。

### 执行步骤

1. 检查最新一次 git 提交，确认提交说明里是否提到已知问题、待补修项或回归；若有，优先处理。
2. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务，以及是否需要拆分为更小子任务。
3. 审查相关代码、测试与规范上下文，确认任务边界，并识别任何阻塞当前任务的既有缺陷或规格不匹配。
4. 若任务可直接完成：
   - 实现代码；
   - 添加或更新测试；
   - 运行相关验证，至少覆盖受影响范围，并尽量满足 `cargo clippy --all-targets -- -D warnings` 与相关测试要求。
5. 若任务不可直接完成：
   - 在 `TODO.md` 中插入前置修复任务并调整顺序；
   - 在 `PLAN.md` 中记录阻塞原因与依赖关系；
   - 提交变更后停止。
6. 完成任务后：
   - 更新 `TODO.md` 勾选状态；
   - 更新 `PLAN.md` 当前状态；
   - 在本文件补充结果摘要与验证记录；
   - 提交 git，随后停止，不继续做下一项任务。

### 记录规则

- 每当发现关键阻塞、调整计划、完成实现、完成测试或准备提交时，更新本文件。
- 不通过规避缺陷的方式推进任务；遇到规格缺口时，先修复或先建前置任务。

### 进度更新（2026-04-26）

- 已检查最新提交 `95504b115175e836c7d41856b9a37e2de2ecd9f3`，提交说明为 `[T5000c3R] Review shared analysis consumer boundary`，未在提交说明中看到需要优先修复的额外既有问题。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务为 `T5000cR Review：确认共享事实层已经脱离 LLVM backend 依赖方向`。
- 本轮接下来将围绕 `ProgramFacts`、`EffectAnalysisCtx`、`ExprFactResolver`、`effect_state_machine_analysis.rs` 与相关 LLVM 接缝做总复核；若发现 shared 层仍持有 backend 依赖或只能通过 `MainCodegen` 才能工作的路径，将先修复该问题或把它登记为新的前置任务。
- 审查过程中发现一个已存在的文档错配：`crates/scoopc/src/llvm/codegen/mod.rs` 顶部注释仍写“下一步 T5000c”；该注释已更新为反映 `T5000c` 已完成 shared facts 抽离、后续转向 `T5000d+` 的真实状态。

### 本轮结果（2026-04-26）

- `T5000cR` 已完成并已在 `TODO.md` / `PLAN.md` 记录：
  - `ProgramFacts`、`EffectAnalysisCtx`、`ExprFactResolver` 本身不依赖 LLVM backend 类型；
  - `effect_state_machine_analysis.rs` 中剩余 `MainCodegen` 相关逻辑只存在于 `#[cfg(feature = "llvm")]` 的薄包装接缝；
  - shared planning / direct-step summary 的入口可以在不构造 LLVM backend 上下文的前提下独立运行；
  - backend 侧改为统一消费 `Rc<ProgramFacts>` 与 shared resolver，而不是现场拼装平行分析 side table。
- 本轮顺手修复的既有问题：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 顶部注释的任务状态描述过期，已更新。
- 验证已完成并通过：
  - `cargo fmt --all --check`
  - `cargo check -p scoopc --lib`
  - `cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`
  - `cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一条待执行任务已切换为 `T5000d 扩展现有 MIR，形成最小 generic early MIR / ANF template`；本轮按要求在提交后停止。
