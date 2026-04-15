# 本轮执行计划

## 约束说明

- 按要求，本轮只处理 `TODO.md` 中第一个未完成任务，完成后即停止。

## 当前任务：T3003R — Review：确认 LLVM lowering 输入面只剩 state machine

### 审查范围
1. `effect/mod.rs` — 统一 lowering 入口 + re-export
2. `state_machine_plan.rs` — plan builder 输入来源
3. `state_machine_segments.rs` — segmentation 无旁路
4. `state_machine_transform.rs` — transform + `UnifiedHandleLoweringContract` 封装
5. `runtime_abi.rs` — 保留的 ABI 声明不构成旁路输入
6. `mod.rs` / `expr.rs` — `Perform` / `Handle` 入口无旁路数据传递

### 检查项
- [ ] `build_unified_lowering_contract` 只从 `handle` HIR + codegen 上下文构造
- [ ] `UnifiedHandleLoweringContract` 不携带源码路径 / scanner / shape 信息
- [ ] 下游 emitter 所需结构都可从 contract 读取
- [ ] `codegen_perform_expr` / `codegen_handle_expr` 占位入口不传递旁路信息
- [ ] 无 shape-based 旁路输入

### 进展
- [DONE] 审查 build_unified_lowering_contract 构建链
- [DONE] 审查 UnifiedHandleLoweringContract 封装
- [DONE] 审查 SuspendSourcePath 不可达性
- [DONE] 审查 HandleStateOp/HandleBranchCondition/SuspendSiteKind
- [DONE] 审查 expr.rs 入口无旁路
- [DONE] 检索 shape/scanner 关键词无命中
- [DONE] 确认 flag-based unwind 非主线
- [DONE] 确认 runtime_abi.rs 无旁路
- [DONE] 更新 TODO.md / PLAN.md

### 结论
LLVM lowering 的主输入只有 state machine，无 shape-based 旁路输入或旧依赖链残留。

### 下一步
下一个未完成任务为 **T3004** — 实现 heap-allocated full state machine 的 LLVM lowering 主体。
