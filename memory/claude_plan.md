# 当前执行计划：T3006

## 任务
用定向测试补齐统一 LLVM lowering 覆盖，并修复暴露出的合同缺口。

## 发现的问题

### 问题 1: Plan Builder 为 Call 子表达式生成冗余 VarRef/Literal ops
- **现象**: `effect_no_perform_no_handler_symbols_basic.scoop` build fixture 失败，报错 "top-level value ref"
- **根因**: Plan builder 处理 `ExprKind::Call { callee, args }` 时，递归调用 `build_expr(callee)` 和 `build_expr(arg)`。这为简单的 callee VarRef（如函数名 `helper`）和 Literal（如 `41`）分别生成了独立的 `HandleStateOp::VarRef` 和 `HandleStateOp::Literal`。
  - 这些 ops 在 emitter 中被 `codegen_expr_in_expected_context` 独立求值
  - 但 `codegen_var_ref(TopLevel { fqn: "helper" })` 无法处理函数引用（只支持 object init / const / var）
  - 关键：这些子表达式 ops 是冗余的，因为后续的 `HandleStateOp::Call` 已经携带完整的调用表达式，codegen_expr 会在 Call 内部重新处理 callee 和 args
- **修复方案**: 修改 plan builder 的 Call/MemberCall/其他复合表达式处理：递归仍用于检测 suspend 点和 state 切分，但对不可能引起 state 切分的简单子表达式（VarRef、Literal 等），不再生成独立 op。只有当子表达式触发了 state 切分（suspend/resume）时，才保留其对应的 ops。

## 执行步骤
1. 修复 plan builder 中 Call 子表达式的冗余 op 生成
2. 验证 build fixture 和 run-pass fixture 通过
3. 补充针对统一 LLVM lowering 的定向测试
4. 修复测试暴露的其他合同缺口
5. 通过全部质量门
6. 更新 TODO.md/PLAN.md，提交
