# Claude Plan

## Working Rules
- 先以 `TODO.md` 为唯一任务排序与完成状态来源，定位首个未标记 `[DONE]` 的任务。
- 不做开放式历史问题排查；仅在其直接阻塞当前任务时，才将其作为前置问题处理并回写 `TODO.md`。
- 不采用规避方案、临时兼容层或缩小范围的方式跳过规范要求。

## Execution Plan
1. 读取 `TODO.md`，识别首个未完成任务，并检查最近提交是否存在与该任务直接相关的未完成事项。
2. 阅读该任务涉及的代码、测试、文档与约束，确认依赖与验证要求。
3. 实现该任务所需的最小正确修改；若发现真实阻塞，则把最小前置任务插入 `TODO.md` 的正确位置并停止继续实现。
4. 运行与该任务相关的验证，包括任务要求的测试，以及必要时的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
5. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
6. 在任务完成时更新 `TODO.md`：为任务标题加上 `[DONE]`，填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 按仓库提交风格创建一次原子提交，然后停止，不进入下一个任务。

## Progress Log
- 已创建初始计划文件；下一步读取 `TODO.md` 并定位当前执行单元。
- 已确认首个未完成任务为 `G4-T05`：重建 ordinary callee suspend/reentry 分析与 lowering。
- 当前分析重点：
  - 检查最近提交是否包含与 `G4-T05` 直接相关的未完成事项。
  - 阅读 `PLAN.md`、`EFFECT_REFACTOR.md`、`EFFECT_REFACTOR_GAPS.md` 中与 ordinary callee / reentry / facts 驱动判定相关的约束。
  - 审查 `crates/scoopc/src/llvm/codegen/effect/ordinary_callee.rs` 及其调用点，确认需要迁移/重建的 helper 与现有缺口。
  - 运行 `cargo check -p scoopc` 观察当前前沿错误是否与任务描述一致。
- 已确认当前实现策略：
  - 新建 `crates/scoopc/src/llvm/codegen/ordinary_callee.rs` 作为 neutral module，接回当前孤立的 ordinary-callee 分析逻辑。
  - 在该模块中实现 `build_fun_callee_suspend_plan_impl`、`build_ordinary_callee_suspend_plan_impl`、`hir_ty_declared_effectful_impl`、`local_call_may_suspend_from_hir_ty_impl`、`known_fun_body_may_outward_effect_impl`、`function_value_expr_body_may_outward_effect_when_called_for_local_impl`。
  - 继续在同一模块中实现 ordinary callee resume-entry / dispatch lowering：通过显式 `incoming_resume_token_ref` 读取 suspend-state，恢复 saved locals 与 resume slot，然后执行对应 `resume_tail`。
  - `needs_reentry` 的 shell 判定继续只依赖已发布 callable facts（`callable_needs_callee_resume_shell(...)`）；ordinary local/function-value suspendability 继续消费共享分析上下文与 pass summary，而不是恢复 TLS scratch。
  - 暂不顺手修复当前 `cargo check -p scoopc` 中属于后续任务或已完成任务回归的其它缺口，除非它们直接阻塞 `G4-T05` 自身 helper 的接回与验证。
- 已完成的关键实现步骤：
  - 新增 `crates/scoopc/src/llvm/codegen/ordinary_callee.rs`，把孤立的 ordinary-callee 分析接回活跃模块图。
  - `mod.rs` 已改为委托 `build_fun_callee_suspend_plan`、ordinary outward/suspendability helper、resume dispatch、resume entry body 到新模块实现。
  - `closure/mod.rs` 已改为使用新的 closure suspend-plan builder，并在 closure function body 中显式记录 lambda callable FQN，避免分析上下文继续误用外层 callable 标识。
  - 已删除孤立的 `crates/scoopc/src/llvm/codegen/effect/ordinary_callee.rs`，避免保留假的 legacy 入口。
- 当前验证结果：
  - `cargo fmt`：通过。
  - `cargo check -p scoopc`：仍失败，但 `local_call_may_suspend_from_hir_ty`、`hir_ty_declared_effectful`、`known_fun_body_may_outward_effect`、`function_value_expr_body_may_outward_effect_when_called_for_local` 等 G4 helper 已不再出现在前沿错误中；当前剩余前沿已切到 `emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled` / `declare_runtime_effect_is_active`（既有 G2 回归）以及 `codegen_perform_expr` / `codegen_handle_expr` / `codegen_call_impl` 等后续 G6/G7 缺口。
  - `cargo clippy -p scoopc --all-targets -- -D warnings`：仍失败，失败前沿与 `cargo check -p scoopc` 一致，未新增 G4 helper 缺口。
