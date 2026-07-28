# Per-Expression Effect Row 跟踪实现计划

## 目标

把 effect 跟踪从"函数级全局累加器"改为"per-expression 效果集合"，使每个表达式节点携带其 actual effect row。修复 handle/on 缩减 effect 的 bug，为 effect lowering 提供基础。

## 核心代数

`EffectRow` 是 effect 类型 TypeId 的**集合**（排序去重）。运算：

- **并集 `∪`** = `EffectRow::union` — 已实现（`ty.rs:102`）
- **差集 `−`** = 新增 `EffectRow::difference` — 从 self 移除 other 中存在的 terms
- **Pure** = `EffectRow::pure()` = 空集

## 实现步骤

### 步骤 1：新增 `EffectRow::difference`（`ty.rs`）

```rust
pub fn difference(&self, other: &EffectRow) -> EffectRow {
    let other_set: HashSet<TypeId> = other.terms.iter().copied().collect();
    let result: Vec<TypeId> = self.terms.iter()
        .filter(|t| !other_set.contains(t))
        .copied()
        .collect();
    EffectRow { terms: result }
}
```

### 步骤 2：新增侧表 `expr_effect_rows`（`facts.rs`）

在 `SemanticFacts` 中新增：
```rust
pub expr_effect_rows: NodeIdTable<EffectRow>,
```

### 步骤 3：`ExprChecker` 新增字段 + 构造点传入（`expr.rs`）

```rust
expr_effect_rows: &'a mut NodeIdTable<EffectRow>,
```

3 个构造点（`check_function` / `check_top_level_val` / `check_pure_static_init`）都需要传入。

### 步骤 4：`walk_expr` 漏斗计算 effect row（`expr.rs`）

在漏斗中 `walk_expr_inner` 返回后，调用 `compute_expr_effect_row(expr, ty)` 计算并写入 `expr_effect_rows`：

```rust
fn walk_expr(&mut self, expr: &Expr) -> TypeId {
    let ty = self.walk_expr_inner(expr);
    self.expr_types.set(expr.id, ty);
    self.backfill_child_types(expr);
    let row = self.compute_expr_effect_row(expr, ty);
    self.expr_effect_rows.set(expr.id, row);
    self.record_expr_facts(expr, ty);
    ty
}
```

### 步骤 5：`compute_expr_effect_row` 实现（`expr.rs`）

按表达式形式的 effect row 计算规则（全部基于已记录的子表达式 effect row 递归推导，不改决议算法）：

| 形式 | effect row |
|------|-----------|
| 字面量/Ident/变量 | `Pure` |
| `Call`（effect-op） | `EffectRow::single(effect_type_id)` |
| `Call`（普通函数） | callee 的 declared effect row（从 Signature 解析为 EffectRow） |
| `Binary`/`Unary`/`InfixCall` | 对应方法的 effect row（通常 Pure） |
| `MemberAccess`/`SafeMemberAccess` | receiver 的 effect row（透传） |
| `Cast`/`TypeTest` | inner 的 effect row（透传） |
| `If` | `cond.row ∪ then.row ∪ else.row` |
| `When` | `subject.row ∪ ⋃(arm.guard.row ∪ arm.body.row)` |
| `Block`/`DoBlock` 等 | block 内所有子表达式 effect row 的并集 |
| `Lambda` | `Pure`（body effect 封装在函数类型内，不泄漏） |
| `Handle` | `(body.row − handled) ∪ ⋃(arm.body.row) ∪ finally.row` |
| `WithUpdate`/`StructLit`/`TupleLit`/`ArrayLit` | 子表达式 effect row 的并集 |
| `Annotated` | inner 的 effect row |
| 其它 | `Pure` |

Handle 的关键计算：
```
handled = EffectRow::from_terms(arms 的 effect_path 解析为 TypeId 的集合)
residual = body_row.difference(&handled)
result = residual ∪ ⋃(arm.body.row) ∪ finally.row
```

### 步骤 6：更新函数级 effect 检查（`expr.rs`）

`check_function` 末尾，用函数体顶层表达式的 effect row 替代 `performed_effects` 累加器：

```rust
let body_row = c.expr_effect_rows.get(body_top_expr_id).cloned().unwrap_or(EffectRow::pure());
let declared_row = ...; // 从 declared_effect 解析为 EffectRow
if !body_row.is_subset_of(&declared_row) {
    // 报 required_effect_not_declared
}
```

旧机制（`performed_effects`/`escape_effects`/`effect_suspend_depth`）保留并存，确认新机制正确后再清理。

### 步骤 7：dump-hir 增强（`render.rs`）

在 `type_of` 闭包中追加 effect row 渲染：
- Pure 不显示（避免噪声）
- 非 Pure 显示 `eff=Raise` 或 `eff=Raise + IO`

### 步骤 8：查询 API + TypedFile 装配

- `TypedHir::expr_effect_row(file_id, node) -> Option<&EffectRow>`
- `check_file_bodies` 创建 `NodeIdTable::new()` 传入 ExprChecker
- `TypedFile` 构造把新表装入 `SemanticFacts`

### 步骤 9：Fixture

在 `tests/fixtures/typecheck/` 或 `tests/fixtures/infer/effects/` 新增验证 fixture：
- handle 缩减 effect（`handle { Raise.raise(1) } on { ... }` 外层 Pure）
- lambda 隔离 effect（lambda body effect 不泄漏）
- if 分支 effect 并集

## 修改文件清单

| 文件 | 改动 |
|------|------|
| `ty.rs` | 新增 `EffectRow::difference` |
| `hir/facts.rs` | `SemanticFacts` 新增 `expr_effect_rows` |
| `hir/mod.rs` | `TypedHir` 新增 `expr_effect_row()` |
| `typecheck/expr.rs` | `ExprChecker` 新增字段；`walk_expr` 调 `compute_expr_effect_row`；新增该函数；3 个构造点传入；`check_function` 用新检查 |
| `typecheck/mod.rs` | `check_file_bodies` 创建表并传入；`TypedFile` 装配 |
| `hir/render.rs` | `type_of` 追加 effect 渲染 |
| `tests/fixtures/` | 新增 effect fixture |

## 验证标准

1. `cargo build -p scoop2_hir -p scoop2c` ✅
2. `cargo test -p scoop2_hir --all-targets` ✅
3. typecheck fixtures 558 全绿 ✅
4. handle body 被 arm 截获的 effect 不出现在外层 effect row（`Pure`）✅
5. lambda body effect 不泄漏到外层 ✅
6. if/when 分支 effect = 并集 ✅
7. dump-hir 显示 `eff=...` ✅