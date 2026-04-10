# Current Task: T0129 — 泛型 where 约束：实例化处 bound 检查

## Status: COMPLETED

## 完成摘要

### 修改清单
1. **`resolve/mod.rs`**: `FunSig` 新增 `where_clause: Option<ast::WhereClause>`
2. **`typecheck/expr/mod.rs`**: 新增 `FunWhereConstraintInfo` + `FunSigOwned.where_constraints`
3. **`typecheck/expr/collect.rs`**: 新增 `build_fun_where_constraints` + `build_fun_where_constraints_from_resolve_sig`
4. **`typecheck/expr/call.rs`**: 新增 `check_fun_where_constraints_after_instantiation`，6 个调用点插入检查
5. **`typecheck/expr/error.rs`**: 新增 `FunWhereConstraintNotSatisfied` 错误变体
6. **`typecheck/expr/ops.rs`**: member method 签名收集填充 where_constraints
7. **`typecheck/expr/infer.rs`**: effect handler arm 签名补齐空 where_constraints
8. **`cone/consume.rs`**: 跨包 FunSig 补齐 `where_clause: None`

### 新增 Fixtures (4 个)
- `where_clause_fun_not_satisfied_is_error` — 单约束不满足
- `where_clause_fun_multi_constraint_not_satisfied_is_error` — 多约束中 B 不满足
- `where_clause_fun_satisfies_bound_ok` — 满足约束
- `where_clause_fun_generic_passthrough_ok` — 泛型传递调用跳过检查

### 验收
- 139 单元测试 + 823 fixtures 通过
