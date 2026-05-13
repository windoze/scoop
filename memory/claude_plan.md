# 当前执行计划

## 约束
- 先以 `TODO.md` 为唯一任务顺序与完成状态来源，定位首个未完成任务。
- 仅完成一个任务；若存在阻塞，则最小化补充前置任务并停止。
- 在执行过程中持续更新本文件，记录计划调整、关键发现、实现进度、验证结果与收尾状态。
- 不采用规避性方案；若遇到会阻塞当前任务的实现缺口或规范不匹配，先修复或在 `TODO.md` 中补充前置任务。

## 初始步骤
1. 读取 `TODO.md`，确认首个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题需要并入当前工作。
3. 阅读当前任务相关说明、依赖、验证要求，以及必要的上下文文件。
4. 实现任务要求的代码或文档变更。
5. 运行当前任务要求的验证，以及必要的构建、测试、lint（至少确保相关测试通过；如任务影响范围较大，再补充更广验证）。
6. 更新 `TODO.md`：将任务标题标记为 `[DONE]`，填写或刷新完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 检查工作区变更，按仓库既有风格创建一次 git 提交，然后停止。

## 进度记录
- 已创建初始计划，下一步开始读取 `TODO.md` 与最近提交信息。
- 已确认首个未完成任务为 `P3-T02R`（Review private naming 迁移）。
- 最新提交为 `[P3-T02] Stabilize effect helper private naming`，无额外提交正文说明未完成前置问题；当前按 `P3-T02R` 直接开展复核。
- 当前 review 重点：
  - 复跑 `P3-T01` / `P3-T02` 指定的 object / IR / grep 审计。
  - 复核 `ClosureId`、`StepSchemaId`、`ContinuationSchemaId`、`CaseTag`、`TypeId` 是否仍直接控制任何 private LLVM symbol 文本。
  - 记录允许保留数字但不属于 authoritative naming 的场景（如 basic block label、SSA 临时名等）。
- 下一步：优先检查工作区状态与当前代码/测试是否已存在未提交变更，然后执行针对性审计命令和代码搜索。
- 阻塞发现：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs` 中 `ProgramLayoutView::step_stem()` 仍使用 `__schema{StepSchemaId}` / `schema{StepSchemaId}` 作为 step/layout/frame/continuation 等 LLVM 命名 stem 回退路径；这与 `P3-T02R` 的完成条件直接冲突。
- 处理策略更新：先定位 `step_stem` 覆盖的所有命名 surface 与可复用的 stable key/hash helper，修复 dense-id 回退路径；修复完成后再重新跑 `P3` 审计并决定是否可将 `P3-T02R` 标记完成。
- 已完成实现：
  - 在 `effect_lowered::stable_naming` 新增 private type-name helper，复用 `PrivateSymbolMangler` 的 hash 主体生成 `scoop.refactor.*__h<hash>` 类型名。
  - `effect_lowered/layout.rs` 中 step/storage/complete/case/vtable/frame/continuation 的 type/global 命名已切到 stable key/hash，不再通过 `step_stem` / `schemaN` / `caseN` 模板退回 dense id。
  - `llvm/tests.rs` 新增 source inventory 防回流检查，并把受影响的 LLVM 行为测试迁移为 hashed family / IR 结构断言；`crates/scoop/src/commands/build.rs` 的 LLVM 断言也同步迁移到新的 private family。
- 已完成验证：
  - 定向测试：`stable_id_`、`effect_contract_struct_types_are_registered_for_effect_codegen`、`closure_call_with_real_outward_effect_uses_explicit_outcome_boundary`、`refactor_llvm_no_outward_plain_abi_layout_has_no_step_shell`、`materialized_mir_closure_private_symbols_use_stable_hash_namespaces`、`external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke`、`composed_continuation_resume_publishes_internal_outcome_surface_and_owner_core`、`build_emit_llvm_dynamic_entry_publication_keeps_plain_carrier_targets_buildable`。
  - 全量：`cargo test -p scoopc`。
  - lint：`cargo clippy -p scoopc --all-targets -- -D warnings` 与 `cargo clippy -p scoop --all-targets -- -D warnings`。
  - grep 复核：`crates/scoopc/src/llvm/codegen` 中 `scoop.lambda$<n>` / `scoop.lambda_resume$<n>` / `scoop.lambda_env$<n>` / `scoop.mir.lambda_env$` / `__scoop_type_desc_mir_closure_env__` / `__schema<n>` / `__k<n>` / `t<TypeId>__` 均为 0 命中。
- 当前状态：`P3-T02R` 已满足完成条件；下一步只剩检查工作区、整理最终提交说明并创建 commit，然后停止。
