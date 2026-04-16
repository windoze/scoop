# 当前执行计划：T3007R — Review：确认仓库中的 effect codegen 生产实现只剩统一主线

## 状态：已完成

## 已执行步骤

1. [x] 验证基线质量 — cargo check + clippy + test（213 passed）+ fixture suite（963 ok）全部通过
2. [x] 关键词搜索 — shape / scanner / CalleeSuspend / suspendable / flag-based / unwind / emit_effect_unwind / raise_target_stack 等全部零命中
3. [x] 审查 effect/mod.rs — 三个主入口全部走统一 state machine 主线，无 fallback
4. [x] 审查 state_machine_emitter.rs — 29 op + 9 terminator 变体全部基于 state machine 合同枚举
5. [x] 审查 runtime_abi.rs — 17 个 effect ABI 声明无 dead_code，全部被消费
6. [x] 审查 runtime_symbols.rs — 无 dead_code，无遗留符号
7. [x] 审查 expr.rs — Perform / Handle 入口单路径透传
8. [x] 审查 mod.rs — EffectOpTagState 无 dead_code，effect 相关路径正确
9. [x] 记录审查结论并更新 TODO.md / PLAN.md
10. [x] 提交变更

## 审查结论

effect codegen 生产实现只剩统一主线，无 shape-based legacy 或 flag-based unwind 残留。
T30（统一 effect LLVM codegen）阶段全部完成。
