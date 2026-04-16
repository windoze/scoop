# 当前执行计划：T3101 — Parser / AST：引入显式 `do { ... }` block

## 状态：已完成

## 已执行步骤

1. [x] 调研：parser/AST/typecheck 结构，确认无 `do` 关键字、无 `DoBlock` AST 节点
2. [x] 添加 `Do` 到 `Keyword` 枚举（token.rs）和 `"do"` 映射（lexer.rs）
3. [x] 添加 `ExprKind::DoBlock { do_span, body }` 到 AST（ast/mod.rs）
4. [x] 添加 `parse_do_block_expr` 方法和 `eat_keyword` cursor 方法
5. [x] 在 `try_parse_expr_atom` 中 `do` 优先于 `{` 匹配
6. [x] `@Safe`/`@Unsafe` 后支持可选 `do`：`self.eat_keyword(Keyword::Do)`
7. [x] 更新所有 AST 消费者：resolve、typecheck (infer/stmt/ops/util)、properties、HIR lower、comptime
8. [x] 新增 4 个 parser 单元测试 + 2 个 parse fixtures
9. [x] 全量测试通过：217 unit tests、965 fixtures（含 LLVM）
10. [x] 更新 TODO.md / PLAN.md
11. [x] 提交
