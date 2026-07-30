# HIR 完善计划

> 状态：步骤 1/2/5 已完成；步骤 3/4/6 需要重新设计
> 目标：HIR 产出物从类型结构上保证完整性——不存在「缺失的事实」。

---

## 0. 设计原则

**HIR 产出物的类型结构本身必须保证完整性。** 不能有 `Option<...>` 表示「本应必然存在但可能缺失」的决议结果。

当前问题：`NodeIdTable<T>` 本质是 `HashMap<NodeId, T>`，`get` 返回 `Option<&T>`。这允许「某个 Call 节点在 `call_resolutions` 中有条目也可能没有」。合法程序的 HIR 不应该允许这种可能性。

正确的做法：HIR 内部的填充过程可以使用 `Option`（中间状态），但最终 move 进 `TypedHir` 时，类型签名必须保证：

- **每个表达式节点必然有精确类型**——`expr_types` 覆盖 HIR 中出现的所有表达式节点，无例外。
- **每个 Call/Operator/Infix/Index 节点必然有精确 `ResolvedCall`**——`call_resolutions` 覆盖所有此类节点。
- **每个 MemberAccess 节点必然有精确 `ResolvedMember`**——`member_refs` 覆盖所有此类节点。
- **每个表达式必然有 outward effect row**——`expr_effect_rows` 覆盖所有表达式节点。
- **类型不使用 `Nothing` 兜底**——`Nothing` 只表示真正的 bottom type（不可达代码），不表示「类型推断失败」。

**MIR 只消费 HIR 的单一产出**，做 monomorphization / devirtualize / inline / effect_lower，不做任何 resolution。如果 HIR 保证了完整性，MIR 的 `call_resolution()` 返回 `Option<&ResolvedCall>` 可以改为返回 `&ResolvedCall`（或 MIR 在 HIR 保证下永远走 `Some` 分支）。

---

## 已完成

- **步骤 1**：`derive_call_resolution` 覆盖 TypeApply callee + FunValue 变体；消除静默 None（合法程序不再返回 None）。
- **步骤 2**：`resolve_member_owner_fqn` 支持 String/Any/标量类型（`scalar_fqn` 回退）；MIR fallback 路径不再被合法程序触发（0 次 fallback hit）。`lower_infix_call`/`lower_index` 消费 `call_resolutions`。
- **步骤 5**：`inferred_type_args` 在 HIR 填充（`fill_inferred_type_args`）；MIR 不再调 `infer_type_args_from_call`。

---

## 待完成（重新设计）

### 步骤 A：HIR 数据结构保证完整性

**目标**：HIR 产出物的类型结构从设计上排除「缺失事实」的可能性。

**当前状态**：
- `SemanticFacts` 中的表都是 `NodeIdTable<T>`（= `HashMap<NodeId, T>`）。
- `get()` 返回 `Option<&T>`——允许缺失。
- MIR 消费时用 `if let Some(rc) = ...` 模式，缺失时走 fallback。

**设计方向**（需进一步细化）：

1. **完整性验证内置到产出物构建**：`TypedHir::into_typed_hir()` 在 move 出 facts 时做完整性检查——对合法程序，缺失任何应覆盖节点即为编译器 bug（panic 或内部错误），不产出残缺 HIR。
   - 这不是事后补救的 completeness gate，而是产出物的构建约束。

2. **消除 `Nothing` 兜底**：`backfill_child_types` 对非 Ident 子表达式赋 Nothing 的行为应改为「精确推导或报错」。`store.nothing()` 的 ~45 处使用需逐一审查：
   - 错误恢复路径（已报诊断）：保留 `Nothing` 返回值，但确保 completeness 检查报告这些节点。
   - 类型推断缺口（空数组、lambda 参数、pattern binder）：需完善推断逻辑。

3. **`block_lenient` 移除**：class init block / secondary ctor body 的宽容检查应改为完整 typecheck（`check_init_body` 已实现，`block_lenient` 是兼容残留）。

### 步骤 B：MIR 消费改为「不可缺失」模式

**目标**：MIR 不再有 fallback 路径。

**当前状态**：
- `lower_call` 在 `call_resolution` 为 `None` 时有 200+ 行 fallback 代码。
- `lower_infix_call` / `lower_index` 有 fallback 路径。
- MIR 仍有 ~47 处 `interner.resolve()` 调用（Symbol→String 转换，非 resolution）。

**设计方向**：

1. **MIR 不再查 HIR resolution 表**：`hir.top_level_funs` / `hir.member_funs` / `hir.members` / `hir.enum_variants` / `hir.type_constraints` 的直接访问应全部改为消费 HIR facts。
   - 需要把这些信息在 HIR 阶段预计算到 facts 中（如 `member_overload_sig` 需要的信息应在 `ResolvedCall`/`ResolvedMember` 中携带）。

2. **删除 fallback 路径**：一旦步骤 A 保证 HIR 事实不缺失，MIR 的 `call_resolution()` 返回 `Option` 可以改为「panic on None」（内部错误）或返回 `&T`（类型保证非 None）。

3. **`resolve_typeref` / `owner_fqn_of` / `owner_fqn_of_type`** 等 MIR 中的类型解析函数应删除或改为消费 HIR 预计算结果。

### 步骤 C：Codegen intrinsic 分发

**目标**：Codegen 不再按 FQN 字符串匹配 intrinsic。

**当前状态**：
- `intrinsic_map`（FQN→注解名）是主路径。
- `intrinsic_name_from_fqn`（FQN 后缀启发式）是 fallback。
- 功能正确，但架构不纯。

**设计方向**：

1. **LIR 携带 intrinsic name**：`LirCall` 或 `LirDeclaration` 中的 `Direct` 调用携带 intrinsic name（从 HIR `@Intrinsic` 注解透传）。
2. **Codegen 按携带的 name 分发**，不再查 FQN。
3. **删除 `intrinsic_name_from_fqn`** 函数。

---

## 实施步骤（建议顺序）

### 阶段 1：HIR 产出物完整性（步骤 A）

1. `into_typed_hir()` 增加完整性约束：对每个 User 文件，遍历 AST 中所有 Call/MemberAccess/Operator/Index 表达式节点，验证 `call_resolutions`/`member_refs` 覆盖。不覆盖即 panic（编译器内部 bug）。
2. 移除 `block_lenient`（class init block / secondary ctor body 改用 `check_init_body` 的严格路径）。
3. 逐个审查 `store.nothing()` 使用点：
   - 错误恢复 → 保留，但完整性约束会报告。
   - 类型推断缺口 → 完善推断。

### 阶段 2：MIR 消费纯化（步骤 B）

1. 在 HIR 中预计算 MIR 需要的所有信息（overload sig、owner FQN、method signatures 等），放入 `ResolvedCall`/`ResolvedMember`。
2. 删除 MIR 中的 fallback 路径（`lower_call` 的 `match &callee.kind` fallback、`resolve_typeref`、`owner_fqn_of` 等）。
3. 删除 MIR 对 HIR resolution 表的直接访问。

### 阶段 3：Codegen 分发纯化（步骤 C）

1. LIR `LirCall::Direct` 携带 `intrinsic_name: Option<String>`。
2. Codegen `lower_direct` 优先用携带的 `intrinsic_name`。
3. 删除 `intrinsic_name_from_fqn`。

---

## 验证清单

- [x] `derive_call_resolution` 对合法程序的所有 Call 节点返回 Some。
- [x] MIR fallback 路径不再被合法程序触发。
- [x] 泛型调用的 `inferred_type_args` 在 HIR 填充。
- [x] `lower_infix_call`/`lower_index` 消费 `call_resolutions`。
- [ ] HIR 产出物构建时保证 facts 完整性（`into_typed_hir` 约束）。
- [ ] 合法程序中不存在可达表达式类型为 Nothing。
- [ ] `block_lenient` 移除。
- [ ] MIR 不再直接查 HIR resolution 表。
- [ ] MIR fallback 路径删除（改为 panic on None）。
- [ ] Codegen intrinsic 分发消费 LIR 携带的 name。
- [ ] `cargo test --all` 通过。
- [ ] run-pass fixture 不回归。
