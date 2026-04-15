# 本轮执行计划

## 约束说明

- 按要求，本轮只处理 `TODO.md` 中第一个未完成任务，完成后即停止。
- 在开始任何仓库检查命令前，先写入本计划文件；后续若计划变化或关键步骤完成，将持续更新本文件。
- 不记录不可验证的内部推理细节；这里只保留可执行计划、观察结论与决策依据，便于审阅进度。

## 当前任务：T3003a

### 任务描述

为 unified state machine 补齐 emitter 所需的执行 payload 元数据。当前 `HandleStateOp` / `HandleBranchCondition` 主要只保留标签、`span` 与少量 id；这不足以让后续 LLVM emitter 真正"只吃 state machine"完成发射。

### 执行计划

1. 为每个 `HandleStateOp` 变体补齐完整的 HIR payload（`Box<hir::Expr>`、`Box<hir::Stmt>`、`Box<hir::ValDecl>`、`Box<hir::HandleArm>`）。
2. 为 `HandleBranchCondition` 补齐完整的条件表达式（`Box<hir::Expr>`），替换原有的 `Span`。
3. 更新 builder 代码，在构造时传入完整 HIR 数据。
4. 更新 segments / transform 中因 `Copy -> Clone` 变化导致的适配。
5. 添加 `payload_signature` 函数用于测试中的身份验证。
6. 添加综合测试 `unified_state_machine_preserves_execution_payload_metadata`，验证 payload 在 `plan -> segments -> unified machine` 流水线中稳定保留。

### 完成状态

已全部完成：
- 所有 `HandleStateOp` 变体已补齐 HIR payload。
- `HandleBranchCondition` 已从 `Span` 升级为 `Box<hir::Expr>`。
- Builder 代码已更新。
- `state_machine_segments.rs` 与 `state_machine_transform.rs` 中的 `Copy -> Clone` 适配已完成。
- 定向测试覆盖 `BindLocal`/decl、`WhileCondHeader`/stmt、`IfCond`/condition、`Perform`/expr、`ResumeAfterSite`/source_expr、`ExecuteArmBody`/arm 六类代表性 payload。
- `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 标记 T3003a 为 DONE，更新 TODO.md / PLAN.md，提交 commit。
