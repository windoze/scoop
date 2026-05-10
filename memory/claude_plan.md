# 当前执行计划

## 任务定位
- 首个未完成任务：`G1-T02R`（Review 显式 hidden ABI 骨架，确认不再回退单 token 语义）。
- 本次调用只处理这一项；完成后更新 `TODO.md`、提交变更并停止。

## 执行摘要
- 先检查最近一次提交是否明确留下与 `G1-T02R` 直接相关的未完事项；若有，则把它视为本任务组成部分或前置条件。
- 按 `G1-T02R` 指定范围复核 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/closure/mod.rs` 以及新的 ABI helper 模块，确认：
  - effectful callable 使用统一 hidden ABI；
  - plain callable 保持 plain ABI；
  - ABI 判定来自 facts/schema，而不是 HIR-level effectful boolean；
  - 活跃实现中没有 effect call wrapper 或 deleted runtime bridge 作为 ABI 补丁。
- 运行任务要求的验证：`cargo check -p scoopc`，并对 declaration/code path 做定向搜索。
- 如果 review 中发现当前实现违背任务完成条件，则直接修复该问题；若存在无法在本次完成的真实前置阻塞，则最小化更新 `TODO.md` 以反映新的依赖顺序，然后停止。
- 若 review 通过，则把 `G1-T02R` 标记为 `[DONE]`，补全完成记录，并创建一次带任务号的提交。

## 具体步骤
1. 查看最近一次提交消息与当前工作树状态，确认是否存在与 `G1-T02R` 直接相关的未提交/未完成内容。
2. 阅读 `G1-T02R` 涉及的代码与 helper，整理当前 hidden ABI 的参数顺序、适用范围和判定来源。
3. 运行 `cargo check -p scoopc`，确认当前错误是否已切换到后续结构性 gap，而不是 ABI review 范围内的问题。
4. 用代码搜索验证活跃 declaration/code path 中不存在 deleted runtime bridge、旧单 token helper 或 legacy wrapper 表面结构。
5. 若需要，做最小修复并重复验证。
6. 更新本文件记录关键进展与结论。
7. 更新 `TODO.md`：给 `G1-T02R` 加上 `[DONE]` 并写入完成记录。
8. 提交所有本次相关变更，提交后停止。

## 进展记录
- 已完成：识别出首个未完成任务为 `G1-T02R`。
- 已完成：检查最近提交，最新提交为 `[G1-T02] Rebuild explicit callable hidden ABI skeleton`，未在提交消息中留下需要先插入 `TODO.md` 的同任务前置事项。
- 已完成：复核 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/closure/mod.rs`、`crates/scoopc/src/llvm/codegen/call/abi.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/emit.rs`。
- 已完成：确认 declaration/binding 层的 hidden ABI 形状为：
  - 顶层 effectful callable / callee resume entry：`[sret?] current_effect_ctx_ref, incoming_resume_token_ref, outcome, ...ordinary params`。
  - effectful closure callable：`[sret?] current_effect_ctx_ref, incoming_resume_token_ref, outcome, env, receiver?, params...`。
  - plain materialized MIR closure/body symbol：保持 plain ABI；仅有 `[sret?] env/ordinary params`，并在 body 入口显式清空 effect hidden ABI 槽位。
- 已完成：确认 ABI 判定来自 published late-lowered callable contract：`effect_step_abi()` / `needs_reentry()`；声明层未再依赖 HIR-level effectful boolean。
- 已完成：对 `mod.rs` / `closure/mod.rs` / `call/abi.rs` 执行精确搜索，未命中 `top_level_fun_uses_hidden_incoming_resume_token`、`mir_fun_uses_hidden_incoming_resume_token`、`function_type_uses_hidden_incoming_resume_token`、`effect_call_wrapper`、`declare_runtime_effect_is_active`、`declare_runtime_effect_set_active_with_trace`、`scoop_effect_*`、`scoop_continuation_*` 等 legacy 名字。
- 已完成：`cargo check -p scoopc` 重新验证；当前首批错误已落在后续任务缺口：`local_call_may_suspend_from_hir_ty` / `known_fun_body_may_outward_effect`（G4）、`alloc_effect_outcome_slot` / `effect_outcome_*` / `coerce_u64_word`（G2）、`codegen_perform_expr` / `codegen_handle_expr`（G7）、`codegen_call_impl`（G6）等，而不是本任务的 hidden ABI skeleton 回退。
- 已完成：更新 `TODO.md`，将 `G1-T02R` 标记为 `[DONE]` 并补充完成记录。
- 进行中：创建本次任务提交，提交后停止。
