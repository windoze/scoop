## 当前执行计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并确认其要求、依赖、验证方式。
2. 查看最近一次提交信息，判断是否存在与该任务直接相关且明确未完成的问题；如果有，将其视为当前任务的一部分或在 `TODO.md` 中补充为前置任务。
3. 检查工作区当前状态，只在不干扰已有用户改动的前提下处理当前任务。
4. 实现当前任务要求的最小正确改动；若遇到阻塞当前任务的真实缺口或规格不匹配，则先修复，或按要求在 `TODO.md` 中添加最小前置任务并停止。
5. 运行当前任务要求的验证，并补充必要测试；同时确保不会引入编译、lint 或测试警告。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 完成后更新 `TODO.md`：为任务标题添加 `[DONE]`，填写完成记录；仅在阶段计划真的变化时更新 `PLAN.md`。
8. 按仓库提交风格创建一次提交，提交信息对应当前任务编号，然后停止，不继续处理下一个任务。

## 说明

- 这里记录的是可审阅的执行摘要与步骤，不包含内部推理细节。
- 如果发现阻塞项，会先在本文件和 `TODO.md` 中记录，再按流程提交并停止。

## 当前进展

- 已读取 `TODO.md`，首个未完成任务为 `G6-T07：重建 direct/static/dynamic call lowering 与 plain/effect ABI 分流`。
- 最近一次提交为 `[G5-T06/G5-T06R] Finalize codegen-owned continuation driver`；提交信息未显式声明与 `G6-T07` 直接相关的未完成前置问题。

## 下一步

1. 检查当前工作区状态，避免误动已有未提交改动。
2. 运行 `cargo check -p scoopc`，确认 `G6-T07` 当前仍暴露的 call lowering 缺口。
3. 阅读与 call lowering 直接相关的模块，定位 legacy wrapper 壳与缺失实现入口。
4. 以最小改动重建新的 call lowering 实现，并把 direct/static/dynamic 调用按 facts 驱动分流到 plain/effect ABI。
5. 运行格式化、`cargo check`、必要的 `cargo clippy --all-targets -- -D warnings`；若任务要求达成，再更新 `TODO.md` 和提交。

## 已确认的实现切口

1. 在 `crates/scoopc/src/llvm/codegen/call/` 下新增非 legacy lowering 模块，承接 `codegen_call_impl`、`codegen_top_level_fun_call_impl`、`try_codegen_*call_impl`、`load_*slot_fn_ptr_i8_impl`、`codegen_*value_call_impl`、`emit_*extern*_impl`。
2. `codegen_top_level_fun_call_impl` / vtable / itable / function-value / funptr 路径不再恢复 wrapper/TLS；effectful ABI 统一改为：
   - 读取当前 `current_effect_ctx_ref`（缺省时传 null）
   - 传 null `incoming_resume_token_ref`
   - 为当前调用点分配显式 `EffectOutcome` slot
3. 调用返回后的传播检查不再看旧 runtime active flag；改为显式读取 `EffectOutcome`，并在需要时把 nested outcome 回写到当前函数的 `current_effect_outcome_ptr`。
4. `class_ctor.rs` 与 `mir_body.rs` 中仍调用已删除 `declare_runtime_effect_is_active` 的 suppressed 路径会一并改成显式 outcome 判定；`effect_lowered/body.rs` 的 class-ctor boundary 也会把“当前函数 outcome 指针”作为没有显式 capture 时的 fallback source。
5. `Continuation.resume` / `perform` / `handle` / MIR effect call helper 仍归 `G7-T08`；本任务只保留显式 fail-fast，不提前补它们的 lowering。

## 关键进展

- 已新增 `crates/scoopc/src/llvm/codegen/call/lowering.rs`，把 `G6-T07` 要求的 `codegen_call_impl`、`codegen_top_level_fun_call_impl`、`try_codegen_*call_impl`、`load_*slot_fn_ptr_i8_impl`、`codegen_*value_call_impl`、`emit_*extern*_impl` 接回到新的 non-legacy 模块。
- 顶层 direct call、vtable/itable dispatch、funptr call、function-value/closure call 已切到 plain/effect ABI 分流：
  - plain call 继续直接返回源码返回值；
  - effectful path 为当前 call site 分配显式 `EffectOutcome` slot，并直接传 `current_effect_ctx_ref + null incoming token + outcome`。
- `class_ctor.rs` 与 `mir_body.rs` 的 suppressed class-ctor call path 已删掉旧 `declare_runtime_effect_is_active()` probe，改为显式 `EffectOutcome` 判定。
- `effect_lowered/body.rs` 的 class-ctor boundary 已支持在没有 suspend-site 显式 capture 时回退观察当前函数的 `current_effect_outcome_ptr`。

## 最新验证

- `cargo fmt`：通过。
- `cargo check -p scoopc`：仍失败，但 `G6-T07` 目标中的 call lowering impl 缺失已全部消失；当前首批错误已切到：
  - `codegen_perform_expr` / `codegen_handle_expr` / `codegen_mir_*effect*call*`（`G7-T08`）
  - `emit_raise_runtime_error_variant`（独立 runtime-error helper，未在本任务范围内继续展开）
