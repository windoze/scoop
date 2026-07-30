# HIR 完善计划

> 状态：待实施
> 目标：让 HIR 成为唯一的 resolution 产出层——每个语法节点有精确 type + 每个 call site 有精确 ResolvedCall + 完整 outward effect row，下游（MIR/LIR/codegen）不再做任何 resolve 工作。

---

## 0. 设计原则

**HIR 是 resolution 的唯一权威来源。** 所有 type、variable、field、function、method 的解析在 HIR 完成，并在每个语法节点上标注好：
- **精确类型**（`expr_types`）——不用 `Nothing` 兜底。
- **精确 call site 决议**（`call_resolutions`）——每个 Call/InfixCall/Binary/Unary/Index 节点都有对应事实，不缺失。
- **精确成员访问决议**（`member_refs`）——每个 MemberAccess/SafeMemberAccess 节点都有对应事实。
- **outward effect row**（`expr_effect_rows`）——每个表达式都有完整 effect row，不缺失。

**MIR 只消费 HIR 的单一产出**，做 monomorphization / devirtualize / inline / effect_lower，不做任何 resolution。
**LIR 做机械布局转换。**
**Codegen 做机械翻译。**

当前 `SemanticFacts` 文档（`facts.rs:11`）写着「事实是尽力而为的决议快照：决议失败时不写入」——这违反了上述原则，是本计划要消除的根因。

---

## 1. 消除 `derive_call_resolution` 的静默 `None`

### 问题

`derive_call_resolution`（`typecheck/expr.rs:2416`）返回 `Option<ResolvedCall>`，有 4 条 `None` 返回路径：

1. **`Ident` callee，`value_refs` 未命中且不是类型名**（~line 2505）
   - 场景：未解析的标识符（拼写错误、缺少 import 等）
   - 当前行为：静默返回 None → `call_resolutions` 无条目 → MIR fallback

2. **`MemberAccess` callee，`TupleIndex` 成员名**（~line 2509）
   - 场景：`receiver._0`（tuple index 访问）不是方法调用——不应走 call resolution
   - 当前行为：返回 None（正确——但不应静默）

3. **`MemberAccess` callee，receiver 无类型**（~line 2511）
   - 场景：receiver 表达式未类型化（Nothing 兜底的下游）
   - 当前行为：静默返回 None

4. **`_ => None`**（line 2545）
   - 场景：callee 不是 Ident/MemberAccess/TypeApply（如嵌套调用 `f()(x)`）
   - 当前行为：静默返回 None

### 修复目标

- 路径 1、3：对合法程序不应发生（typecheck 已报错）。若发生，**必须在 HIR 报诊断**（`scoop::typecheck::unresolved_call_site`），而非静默跳过。
- 路径 2：TupleIndex 不是方法调用——`record_expr_facts` 的 Call 分支不应对 TupleIndex callee 进入 `derive_call_resolution`。改为在入口提前判断，不进入。
- 路径 4：`f()(x)`（函数值调用）——callee 是一个 Call 表达式，应解析为 `ResolvedCall::LocalValue` 或类似变体。需要覆盖。

### 验证标准

`derive_call_resolution` 对所有合法程序的 Call 节点返回 `Some`。若返回 `None`，必须伴随一条诊断。

---

## 2. 消除 `Nothing` 兜底类型

### 问题

`typecheck/expr.rs` 中有 ~44 处 `store.nothing()` 用作「无法确定类型」的兜底。`backfill_child_types`（~line 2184）对非 Ident 子表达式直接赋 `Nothing`：

```rust
let t = if let ExprKind::Ident(ident) = &child.kind {
    self.derive_ident_type(ident.symbol, child.id).unwrap_or(nothing)
} else {
    nothing   // <-- 非 Ident 节点一律 Nothing
};
```

completeness gate（`completeness.rs:200-205`）只检查「有没有类型条目」，不检查类型是否为 `Nothing`——所以 `Nothing` 通过了门禁。

### 具体位置（分类）

| 类别 | 典型位置 | 数量 | 原因 |
|------|---------|------|------|
| 函数体 walk 兜底 | `backfill_child_types`（~2184） | ~大量子表达式 | 非 Ident 子表达式未逐类型化 |
| 空数组 `[]` | ~3496 | 1 | 空数组无元素类型可推断 |
| 非 continuation receiver | ~4769 | 1 | 非 Continuation 类型上调 resume |
| 未解析 struct init | ~6098 | 1 | StructLit 字段类型解析失败 |
| 无标注 lambda | ~6471 | 1 | lambda 参数类型无法推断 |
| pattern binder | ~7134/7241 | 2 | 模式绑定类型未确定 |
| 其它 lenient 降级 | 各处 | ~36 | 各种「无法确定」路径 |

### 修复目标

- **类型推断完善**：对所有合法表达式，类型推断必须给出精确类型。`Nothing` 只用于真正的 bottom type（不可达代码的表达式，如 `return` 之后）。
- **completeness gate 加强**：检查 `Nothing` 类型是否合法（仅允许在不可达位置）。若 `Nothing` 出现在可达表达式上，报 `scoop::typecheck::untyped_node`。
- **lenient 路径消除**：class init block / secondary ctor body 的 `block_lenient`（`completeness.rs:152`）应改为完整 typecheck（已部分完成——`check_init_body` 已实现，但 `completeness.rs` 的 `block_lenient` 仍存在作为兼容路径）。

### 验证标准

合法程序中不存在可达表达式类型为 `Nothing`。completeness gate 对所有 `Nothing` 报错（除非在不可达位置）。

---

## 3. 完善 `record_expr_facts` 覆盖所有 call 形态

### 问题

`record_expr_facts`（`typecheck/expr.rs:2274`）在 `walk_expr` 漏斗处为以下表达式形态写 `call_resolutions`：

- `ExprKind::Call` → `derive_call_resolution`
- `ExprKind::Binary` → `record_operator_call`
- `ExprKind::Unary` → 同 Binary 路径
- `ExprKind::InfixCall` → `record_infix_call`
- `ExprKind::Index` → `record_index_call`
- `ExprKind::MemberAccess` → `derive_member_ref`

但这些 recorder 内部有**静默 early return**（`let Some(...) = ... else { return; }`），导致事实缺失。具体：

### 3a. `record_operator_call`（~line 2553）

Binary 运算符（`a + b`）解析为 `receiver.plus(arg)` 方法调用。静默 return 路径：
- receiver 类型无法解析 owner FQN → return（无诊断）
- method_name 不在 member_funs 中 → return

**修复**：若 receiver 类型已知但 owner 无法解析或方法不存在，报 `scoop::typecheck::operator_not_resolved`。

### 3b. `record_infix_call`（~line 2587）

中缀运算符（`a until b`）。同 operator 路径的静默 return。

**修复**：同 3a。

### 3c. `record_index_call`（~line 2621）

索引访问 `arr[i]` 解析为 `arr.get(i)`。静默 return 路径：
- receiver 类型无法解析 → return
- `get` 方法不存在 → return

**修复**：同 3a。若 receiver 类型已知但无 `get` 方法，报错。

### 3d. `derive_member_ref`（~line 2365）

成员访问 `a.b`。静默 return 路径：
- receiver 无类型 → return
- owner 无法解析 → return
- member 未找到 → return

**修复**：同 3a。

### 验证标准

所有 `record_*` 函数在 receiver 类型已知时必须产出事实或报诊断。不静默 return。

---

## 4. 消除 `expr_effect_rows` 的缺失

### 问题

`expr_effect_rows`（`SemanticFacts`）由 `compute_expr_effect_row`（~line 2099）计算。它读取 `call_resolutions` 来获取 callee 的 effect row。当 `call_resolutions` 缺失（§1/§3 的静默 return），对应的 effect row 也是错的（退化为 Pure）。

### 修复目标

一旦 §1/§3 修复（`call_resolutions` 不再缺失），`expr_effect_rows` 也会自动完善。但需额外确认：
- `compute_expr_effect_row` 对 `Call` 节点总是能从 `call_resolutions` 读到 callee effect row。
- Handle 表达式的 effect row 减法正确（已减去被 arm 截获的 effect）。

### 验证标准

每个 `ExprKind::Call` 节点的 `expr_effect_rows` 携带正确的 outward effect row（= callee 的 declared effect row 减去本地 handle 捕获的 effect）。

---

## 5. 语义事实文档更新

### 当前文档（`facts.rs:11`）

```
- 事实是**尽力而为**的决议快照：决议失败（如重载歧义、未解析引用）时不写入，
  对应 NodeId 在表中缺失。MIR lowering 对缺失事实按「无法 lower」报错。
```

### 目标文档

```
- 事实是**完整精确**的决议快照：每个 Call/MemberAccess/Operator/Index 节点
  都有对应事实。决议失败时，typecheck 必须报诊断（而非静默跳过）。
  合法程序的 HIR 中不存在缺失事实的 call/member site。
```

---

## 6. 完成后的下游清理（MIR + Codegen）

HIR 完善后，以下下游代码可删除：

### MIR `lower/expr.rs` 可删除的 fallback 路径

| 函数/路径 | 行号 | 说明 |
|----------|------|------|
| `lower_call` 的 `match &callee.kind` fallback | ~441-636 | resolution == None 时重新解析；HIR 完善后 None 不再发生 |
| `infer_type_args_from_call` | ~338-403 | MIR 推断类型实参；HIR 应在 `inferred_type_args` 中填充 |
| `resolve_typeref` | ~3550-3659 | MIR 做完整类型名解析（用于 is/as）；应改为消费 HIR 事实 |
| `lower_infix_call` 的手工 dispatch 构建 | ~1665-1730 | 应改为消费 `call_resolutions` |
| `lower_index` 的 IndexAccess 直接发射 | ~1733-1758 | 应改为消费 `call_resolutions` 中的 `get` 方法决议 |
| `lower_via_call_resolution` fallback | ~1471-1547 | binary/unary 运算符 fallback；HIR 完善后不再触发 |
| `lower_unary` 的 `!` 特殊路径 | ~1196-1256 | `Symbol::default()` owner + 手工 equals dispatch |
| `owner_fqn_of` | ~1550-1567 | 标量→`scoop.core.<T>` 映射；应在 HIR 完成 |
| for 循环降级的 `iterator`/`hasNext`/`next` 按名查找 | stmt.rs ~576,624-625 | 应在 HIR resolve 为精确 call site |
| MIR 直接查 HIR 表的 ~95 处 | 全 lower/ | `hir.top_level_funs`/`member_funs`/`members`/`interner.get` 等 |

### Codegen 可清理的路径

| 路径 | 说明 |
|------|------|
| `lower_direct` 的 `try_lower_intrinsic_by_fqn` 优先 | 应改为 LIR 携带 intrinsic name |
| `intrinsics.rs` 的 FQN 后缀匹配 + `intrinsic_name_from_fqn` | 应改为消费 LIR 携带的 intrinsic name |
| `disambiguate_overloaded_intrinsic` | codegen 内做重载选择；应由 HIR 在 call site 确定 |
| `resolve_extern_runtime_symbol` 的 FQN→运行时符号推断 | 应从 `@Extern(name=...)` 注解透传 |

---

## 7. 实施步骤（建议顺序）

每步确保 `cargo build` + `cargo test` 绿。

### 步骤 1：HIR derive_call_resolution 无静默 None

- 路径 2（TupleIndex）：提前判断，不进入 call resolution。
- 路径 4（非 Ident/MemberAccess/TypeApply callee）：覆盖函数值调用场景。
- 路径 1、3：对合法程序保证返回 Some；若返回 None，报诊断。
- 更新 `SemanticFacts` 文档。

### 步骤 2：record_expr_facts 无静默 return

- `record_operator_call`/`record_infix_call`/`record_index_call`/`derive_member_ref`：receiver 类型已知时必须产出事实或报诊断。

### 步骤 3：消除 Nothing 兜底（分阶段）

- 3a：完善 `backfill_child_types`——对所有子表达式递归推导类型（而非非 Ident 一律 Nothing）。
- 3b：逐个消除 `store.nothing()` 兜底点（空数组推断、lambda 参数推断等）。
- 3c：`completeness.rs` 的 `block_lenient` 改为严格检查（或移除——`check_init_body` 已覆盖）。
- 3d：completeness gate 检查可达表达式上的 Nothing 并报错。

### 步骤 4：expr_effect_rows 完善

- 确认 `compute_expr_effect_row` 在 call_resolutions 完善后自动正确。
- 添加完整性检查：每个 Call 节点都有 effect row 条目。

### 步骤 5：HIR inferred_type_args 填充

- `derive_call_resolution` 对泛型调用，从实参类型推断 `inferred_type_args` 并写入。
- 消除 MIR 的 `infer_type_args_from_call`。

### 步骤 6：下游清理

- MIR：删除 fallback 路径、删除 `resolve_typeref`/`owner_fqn_of`/`infer_type_args_from_call`。
- MIR：`lower_infix_call`/`lower_index` 改为消费 `call_resolutions`。
- MIR：for 循环降级消费 HIR 事实而非按名查找。
- Codegen：intrinsic name 由 LIR 携带（从 HIR `@Intrinsic` 注解透传到 LIR）。
- Codegen：删除 `intrinsic_name_from_fqn` 和 FQN 后缀匹配。

---

## 8. 验证清单

- [ ] `derive_call_resolution` 对合法程序的所有 Call 节点返回 Some。
- [ ] `record_operator_call`/`record_infix_call`/`record_index_call`/`derive_member_ref` 不静默 return。
- [ ] 合法程序中不存在可达表达式类型为 Nothing。
- [ ] completeness gate 对所有可达 Nothing 报错。
- [ ] 每个 Call 节点都有 `expr_effect_rows` 条目。
- [ ] 泛型调用的 `inferred_type_args` 在 HIR 填充（MIR 不再推断）。
- [ ] MIR `lower_call` 不再有 fallback 路径（resolution == None 时报错）。
- [ ] MIR `lower_infix_call`/`lower_index` 消费 `call_resolutions`。
- [ ] MIR 不再直接查 `hir.top_level_funs`/`member_funs`/`members` 等表。
- [ ] Codegen intrinsic 分发消费 LIR 携带的 intrinsic name（不再按 FQN 匹配）。
- [ ] `cargo test --all` 通过。
- [ ] run-pass fixture 不回归。
