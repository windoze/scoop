# 当前执行计划

## 目标
- 以 `TODO.md` 为唯一任务真源，定位并完成首个未完成任务。
- 本次调用只完成一个任务；完成后更新记录、验证并提交，然后停止。

## 执行约束
- 先确定当前任务，再做与该任务直接相关的代码与测试调查，不做开放式问题扫查。
- 若发现阻塞当前任务的真实缺陷、规格不匹配或缺失能力，必须先修复；若无法在本次直接修复，则在 `TODO.md` 中新增最小前置任务并调整顺序后停止。
- 除非阶段计划发生变化，否则不改 `PLAN.md`；例行进度记录写入本文件与 `TODO.md`。
- 完成任务后必须运行相关验证；若适用，补充或调整测试，确保 `cargo clippy --all-targets -- -D warnings` 与相关测试通过。
- 完成状态以 `TODO.md` 任务标题带有 `[DONE]` 为准，单独填写完成记录不算完成。

## 初始步骤
1. 读取 `TODO.md`，识别首个标题未带 `[DONE]` 的任务。
2. 查看最近一次提交消息，判断是否存在与该任务直接相关且未完成的问题需要一并处理。
3. 阅读任务条目中的要求、依赖、验证方式，并检查相关代码与测试位置。
4. 实施最小且正确的代码修改，不引入规避性实现。
5. 运行任务要求的验证与必要的补充验证；若失败，先修复再重跑。
6. 更新本文件记录关键进展；更新 `TODO.md` 的任务状态与完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 按仓库约定创建一次提交，提交消息包含当前任务编号，然后停止。

## 计划更新规则
- 在识别出当前任务后，补充更具体的实施与验证步骤。
- 在完成关键节点（任务识别、实现完成、验证完成、记录更新、提交完成）后同步更新本文件。

## 当前任务识别
- 首个未完成任务：`G1-T02`（重建 effectful callable 的显式 hidden ABI 骨架）。
- 最近提交：`[G0-T01R] Fix cleanup regressions found in review`。
- 目前未发现最近提交消息中明确标出的、必须先独立插入 `TODO.md` 的同任务未完成前置问题；先按 `G1-T02` 条目要求开展实现与验证。

## 当前任务的具体执行步骤
1. 阅读 `G1-T02` 关联设计文档与现有 LLVM codegen/closure 路径，定位现有单 hidden token ABI 的入口与调用点。
2. 运行一次 `cargo check -p scoopc`，确认当前缺失项与 `G1-T02` 描述一致，并据此收敛改动边界。
3. 以 facts/schema 驱动方式重建 effectful callable 的显式 hidden ABI 声明层，覆盖：顶层函数、resume entry、closure/function-value callable、可能残留的 effect call wrapper 路径。
4. 在函数体 codegen 上下文中记录 `current_effect_ctx_ref`、`incoming_resume_token_ref`、`outcome` 三个显式 hidden ABI 入口，避免继续依赖旧的单 token 语义 helper。
5. 重新运行 `cargo check -p scoopc`，必要时补做更窄测试/检查，直到 `G1-T02` 指定的 ABI 缺失项消失；如出现阻塞性规格缺口，按要求回写 `TODO.md`。
6. 完成后更新 `TODO.md` 的 `[DONE]` 标记和完成记录，并在本文件写入验证结果与提交状态。

## 当前进展
- 已完成：
  - 为 `CompilationUnitCodegenCx` 补入 published late-lowered callable contract 引用，用它替代旧的 HIR effectful 布尔值来判断 root callable 是否需要显式 hidden ABI / resume shell。
  - 重写顶层函数、closure callee resume entry、generic callee resume entry 的声明路径，hidden ABI 统一为：`current_effect_ctx_ref`、`incoming_resume_token_ref`、`ScoopEffectOutcome *outcome`。
  - 删除未再被调用的 effect call wrapper 壳层与旧的 `*_uses_hidden_incoming_resume_token` helper；活跃源码中已无这些旧 helper 名字。
  - 在 `FunctionBodyCodegenCx` 中加入 `current_effect_ctx_ref`、`current_incoming_resume_token_ref`、`current_effect_outcome_ptr` 三个槽位，并在顶层函数 / HIR closure body 入口绑定，在 plain materialized MIR closure path 显式清空。
  - 将 closure/plain materialized MIR declaration path 从“按函数类型 effect row 猜 hidden token”改为按已发布 contract 决定：plain callable 入口不再混入 hidden effect ABI。
- 最新验证：
  - `cargo check -p scoopc` 仍失败，但 `G1-T02` 针对的缺失项已从错误列表中消失：不再出现 `declare_callee_resume_entry_function_impl`、`declare_top_level_fun_callee_resume_entry_impl`、`declare_top_level_fun_effect_call_wrapper_impl`、`ensure_top_level_fun_effect_call_wrapper_defined_impl`、`codegen_top_level_fun_effect_call_wrapper_impl`，也不再出现 `top_level_fun_uses_hidden_incoming_resume_token` / `mir_fun_uses_hidden_incoming_resume_token` / `function_type_uses_hidden_incoming_resume_token` 残余引用。
  - 当前首批剩余错误已切到后续任务：`local_call_may_suspend_from_hir_ty` / `known_fun_body_may_outward_effect`（G4）、`alloc_effect_outcome_slot` / `effect_outcome_*` / `coerce_u64_word`（G2）、`codegen_perform_expr` / `codegen_handle_expr`（G5）等。
- 已补做：
  - `cargo fmt`：通过。
  - 对 `crates/scoopc/src` 重新 grep：上述旧 helper / wrapper impl 名字无命中。
- 剩余步骤：
  1. 更新 `TODO.md` 的 `G1-T02` 完成记录并加 `[DONE]`。
  2. 查看 git 状态/差异/近期提交风格后创建提交。
