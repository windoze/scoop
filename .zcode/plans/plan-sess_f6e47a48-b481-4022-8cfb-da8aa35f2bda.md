# 泛型参数表示完整重构：TypeParamId 身份 + EffectRow tail

## 背景与动机（审计结论）

泛型 `T` 目前统一表示为 `TypeKind::Param(TypeParamType)`，但存在三个结构性缺陷：

1. **身份与替换不一致（潜在 hole）**：`TypeParamType` 文档声称身份是 `(file, span)`、name 不参与判定，但 `Subst` 内部用 `HashMap<Symbol, TypeId>` **只按 name 键**（`ty.rs:588-614`）。两个不同声明里同名 `T` 在共享 Subst 里会撞键。`build_subst` 还用 `span: Span::default()` 占位（`materialize/mod.rs:610`），所以即便 Subst 改成 span 键也匹配不到。

2. **variance 全丢**：AST `TypeParam.variance`（`decl.rs:360`）降级到 `TypeParamType` 时被丢弃，影响将来协变/逆变子类型健全性。

3. **effect 行语义混淆**：`<eff E>` 被塞成 `EffectRow.terms` 里的一个 `TypeKind::Param` term（`lower.rs:161-173`、`env.rs:631-640`），把「一整组抽象 effect（行变量）」和「恰好是参数的单个 effect」混为一谈。`eff_var_of_fqn_or_sig` 因此硬编码 symbol `"E"`（`expr.rs:4397`）——靠约定而非身份定位真正的 eff 参数。

**关键约束（来自探查）**：每个声明至多一个 `<eff E>`（`TypeParamList.effect_row: Option<EffectRowParam>`，parser 强制其为最后一项）。所以 tail 永远是 `Option<...>` 而非 list。eff-param 的值是一个完整 `EffectRow`，与 type-param 的值（单个 `TypeId`）属于不同值域——两者不能塞进同一个 `Subst`。

## 数据结构设计

```rust
// crates/scoop2_hir/src/ty.rs

/// 全局唯一的类型参数身份（声明时分配一次，不可伪造）。
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeParamId(pub u32);

/// 类型参数的种类：普通类型参数 vs effect 行参数。
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeParamKind { Type, Effect }

pub enum TypeKind {
    // ...
    Param(TypeParamId),   // 出现处只带 id（不再带 name/span）
}

/// 声明时的完整元数据（侧表，key = TypeParamId）。
pub struct TypeParamDecl {
    pub id: TypeParamId,
    pub name: Symbol,        // 仅诊断/显示
    pub span: Span,
    pub variance: Variance,  // 恢复（从 AST 带下来）
    pub bound: Option<TypeId>,
    pub kind: TypeParamKind, // Type | Effect
}

/// effect 行：已知具体 effect + 至多一个多态行变量。
pub enum EffectTail { Empty, Var(TypeParamId) }

pub struct EffectRow {
    pub terms: Vec<TypeId>,
    pub tail: EffectTail,   // <eff E> → Var(E)；纯 → Empty
}
```

## 两个替换表（值域不同，不可合并）

```rust
// 普通类型参数替换：TypeParamId → TypeId（现有 Subst 的角色，键改 id）
pub struct Subst { entries: HashMap<TypeParamId, TypeId> }

// effect 行参数替换：TypeParamId → EffectRow（新增，专管 tail）
pub struct EffSubst { entries: HashMap<TypeParamId, EffectRow> }
```

`apply_subst` 处理 `Param(id)` 时查 `Subst`；`apply_subst_row` 处理 `tail` 时查 `EffSubst`。两者在调用点（typecheck 推断、MIR 单态化）一起构造、一起传递。

## 实施步骤（按层、按依赖序）

### 阶段 1：基础设施（ty.rs，纯增量，不破坏现有调用）
1. 新增 `TypeParamId`、`TypeParamKind`、`TypeParamDecl`、`Variance`（Variance 从 syntax 搬或重导出）、`EffectTail` 类型定义。
2. `TypeStore` 增加 `type_param_decls: HashMap<TypeParamId, TypeParamDecl>` 侧表 + `next_param_id: u32` 分配器 + `intern_param(decl) -> TypeParamId` / `param_decl(id) -> &TypeParamDecl`。
3. `EffectRow` 加 `tail: EffectTail` 字段；逐个重写 7 个方法（`from_terms`→保留 terms、tail 默认 Empty；新增 `with_tail`/`from_terms_with_tail`；`union` 合并 terms + tail 取策略：两 tail 同 Var 则保留，否则保守取并集 terms 清空 tail 为 Empty 并 diag；`is_subset_of` 加 tail 包含判断；`difference` 只作用于 terms 不动 tail；`pure`/`single` 设 tail=Empty）。
4. `apply_subst_row_inner` 增加 tail 处理：对 `tail: Var(id)` 查 `EffSubst`，替换成展开后的 terms（Var 消失、tail→Empty）。
5. `apply_subst` 的 `FunctionType` 分支同时传 `Subst` + `EffSubst`。

### 阶段 2：身份键改造（Subst + apply_subst）
6. `Subst.entries` 从 `HashMap<Symbol, TypeId>` 改 `HashMap<TypeParamId, TypeId>`；`insert/get` 签名改 id。
7. `apply_subst` 的 `Param(p)` 分支：`subst.get(&p)` 改 id 键查找（`TypeKind::Param` 现在装 `TypeParamId`）。
8. 新增 `EffSubst` 及其在 `apply_subst_row` 的接入。
9. 更新 ty.rs 内 `#[cfg(test)]` 测试 helper（`param`/`param_at` 改用新 id 体系）。

### 阶段 3：AST → HIR 降级点（身份 minting）
10. `build_tp_map`（`env.rs:1204-1219`）改为返回 `Vec<TypeParamDecl>`（按声明序，含 variance/bound/kind），每个分配 `TypeParamId` 并登记进 `type_param_decls` 侧表。合并 `merge_type_params`（`mod.rs:4121`）、`type_param_map_of`（`overloads.rs:638`）、`type_param_map`（extern_fn/overloads）这些重复实现到统一 helper。
11. `TypeLowering.type_params`（`lower.rs:28`）字段类型从 `HashMap<Symbol, TypeParamType>` 改 `HashMap<Symbol, TypeParamId>`（仍按 name 查局部，但查到的是 id）；`lower_path`（`lower.rs:216-225`）查到 id 后 `store.param(id)`。
12. eff-param 降级（`lower.rs:161-173`、`env.rs:631-640`）：eff-param 分配 `TypeParamId(kind=Effect)`，写入 `EffectRow.tail = Var(id)` 而非塞进 terms。
13. `resolve_signature_effect_row`（`env.rs:610-671`）与 `lower_effect_row`（`lower.rs:147-191`）同步填 tail。

### 阶段 4：typecheck 体内推断（消除硬编码 "E"）
14. `eff_var_of_fqn_or_sig`（`expr.rs:4394`）改为从签名携带的真实 eff-param id 取（不再硬编码 "E"）。这要求 Signature 能暴露其 eff-param 的 `TypeParamId`——给 `Signature` 加 `eff_param: Option<TypeParamId>` 字段。
15. `infer_eff_var_from_arg`（`expr.rs:4404`）、`subst_eff_row`（`expr.rs:4598`）、`subst_eff_var_in_type`（`expr.rs:4556`）、`record_callee_effects`（`expr.rs:4617`）、call-site 推断块（`expr.rs:5734-5761`）全部改用 id + `EffSubst`。
16. `compute_expr_effect_row`（`expr.rs:1932`）Handle/Call 分支：tail 在 difference/union 时正确传播。
17. `check_closed_effect_row_no_row_var`（`mod.rs:3172`）改为「closed 时 tail 必须为 Empty」。
18. where-clause 约束存储（`type_constraints`/`type_param_bounds_for`，`env.rs:249-285`）从 name 键改 id 键。

### 阶段 5：MIR（携带身份过 lowering 边界）
19. `FunDecl.type_params`（`mir/mod.rs:103`）从 `Vec<Symbol>` 改 `Vec<TypeParamId>`（或 `Vec<TypeParamDecl>` 快照）。lowering 写入点（`builder.rs:745/1131/1419`）从 AST TypeParamList 带下 id。
20. `build_subst`（`materialize/mod.rs:592-620`）按位置 zip `Vec<TypeParamId>` 与 type_args，用 id 键填 Subst（不再重建 name+占位 span）。
21. MIR eff-param 推断/单态化用 `EffSubst`。
22. `stable_id.rs` / stable_template_key（`builder.rs:230/322`）：从 name 字符串改用 id（更稳定，无歧义）。
23. `verify_no_generic_residue`（`verify.rs:625`）保持——单态化后 Param 必须消失，id 体系下同样适用。

### 阶段 6：诊断/渲染 + 测试
24. `TypedHir.render` / Debug 渲染：Param 显示时从侧表查 name（id→name），保证可读性。
25. 回归 fixture（核心，必须全绿）：`tests/fixtures/infer/effects/*`（eff_row_fn_type_e_plus_base_infers_ok、eff_row_higher_order_return_infers_ok、use_site_eff_row_* 等）、`tests/fixtures/typecheck/hir_source_effect_facts_polymorphic_ok`、`closed_effect_row_contains_row_var_is_error`、`interface_impl_effect_row_*`、`tests/fixtures/mir_lowered/generic_materialization.scoop`。
26. 新增 fixture：多声明同名 T 不混淆（验证 id 身份修复）、variance 携带到 HIR（render 可见）。

## 验证清单
- [ ] `Subst` 键改为 `TypeParamId`，两声明同名 T 不撞键（新 fixture 覆盖）
- [ ] `EffectRow.tail` 为一等公民，`<eff E>` 不再伪装成 term
- [ ] `eff_var_of_fqn_or_sig` 不再硬编码 "E"，从签名真实 eff-param id 取
- [ ] variance 从 AST 保留到 `TypeParamDecl`
- [ ] `cargo test -p scoop2_hir` 全绿
- [ ] `cargo test -p scoop2_mir` 全绿
- [ ] `python3 tools/run_fixtures.py`（typecheck/infer/effects/run-pass 阶段）不回归
- [ ] grep 确认 `TypeParamType` 结构体已移除或仅作过渡别名；`file: FileId(0)` / `span: Span::default()` 占位模式消失

## 不在本次范围
- 不动 LIR/codegen（无 type-param 身份，单态化后 Param 已消失）。
- 不实现基于 variance 的协变/逆变子类型派发（元数据先存上，强用留后续）。
- effect-polymorphic 行的完整 subsumption 算法（tail 的精确包含判断先保守，保证不 unsound 即可）。