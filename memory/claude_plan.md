# 当前执行计划：T3102 — Typecheck / HIR：收口 do block 的 expression statement 与 tail value 语义

## 状态：研究中

## 目标
- `do { expr }` → 类型为 expr 的类型（tail value）
- `do { expr; }` → 类型为 `Unit`（expression statement，分号终止）
- `if` / `when` / `handle` / lambda body / `do` block 的值语义统一按此规则工作
- HIR / diagnostics 对 tail expr 与 terminated expr stmt 保持可区分形状

## 研究步骤
- [ ] 了解 parser 对 block 内语句和尾表达式的处理
- [ ] 了解 StmtKind 中 expression statement 的表示方式
- [ ] 了解 typecheck 对 block 值的计算逻辑
- [ ] 了解 HIR 层 block 的表示

## 执行步骤（待研究后细化）
- [ ] 实现变更
- [ ] 创建测试 fixtures
- [ ] 验证质量门
