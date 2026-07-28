# 完整修复 5 项 MIR 功能缺漏

按依赖顺序分 5 个阶段实施。每个阶段独立可编译、可测试。

---

## 阶段 1：Generic 单态化全覆盖 + verify 激活（最基础）

### 1a. 暴露 EffectRow 替换原语
**文件**: `crates/scoop2_hir/src/ty.rs`
- 把 `apply_subst_row`（当前 private，:569）改为 `pub`，或新增 `pub fn apply_subst_effect_row(&mut self, row: EffectRow, subst: &Subst) -> EffectRow` 包装器。
- 这是所有 `EffectRow` 字段替换的前提。

### 1b. 补全 `subst_*` 系列覆盖所有 TypeId/EffectRow 字段
**文件**: `crates/scoop2_mir/src/mir/materialize/mod.rs`

`subst_fun_decl` 新增：`fd.effect_row` 替换。

`subst_rvalue` 补全（逐字段，引用审计清单）：
- `TopLevelRef`: `hidden_effects`、`generic_eff_args`
- `MemberAccess`: `member.hidden_effects`
- `EnumVariant`: `args[*].value_ty`
- `ClassCtor`: `hidden_effects`
- `Call.transport`: `aggregate_return`、`array`（含 `array_ty`/`element_ty`/`element`）、`gc`（含 `subject_ty`/`token_ty`/`subject`）
- `StructLit`: `fields[*].value_ty`
- `WithUpdate`: `updates[*].value_ty`
- `PerformResult`: `result_ty`
- `PatternMatch`: 递归替换 `Pattern` 内的 TypeId（`Bind.ty`、`Is.ty`、嵌套 Tuple/Struct/Variant/Or）
- `PatternExtract`: `result_ty`

`subst_call_kind` 补全：
- `Direct`: `type_args`（当前只替换 `generic_type_args`）、`generic_eff_args`
- `Virtual`/`Interface`: `dispatch.generic_eff_args`

`subst_terminator` 补全：
- `Perform`: `metadata.op_type_args`、`metadata.payload_tuple_ty`
- `Handle`: 每条 arm 的 `op_type_args`、`payload_tuple_ty`

`subst_type_test_metadata` / `subst_cast_metadata` 补全：
- `RuntimeTypeParameterizedMatch` 全部变体（Nominal/Function/Option/Tuple/Union/StarProjection）
- `RuntimeCastResult`（Target.ty、Option.option_ty/some_ty）
- `RuntimeTypeTestMetadata.descriptor.ty`

`subst_member_access_metadata` 补全：`hidden_effects`

为减少重复代码，新增辅助函数：
- `subst_effect_row(store, row, subst)` — 调用暴露的 `apply_subst_effect_row`
- `subst_effect_rows(store, rows, subst)` — Vec 版本
- `subst_type_ids(store, tys, subst)` — Vec<TypeId> 版本
- `subst_optional_type_id(store, ty, subst)` — Option<TypeId> 版本

### 1c. `build_subst` 加 arity 检查
**文件**: `crates/scoop2_mir/src/mir/materialize/mod.rs`
- `build_subst`（:382）：当 `type_args.len() < type_params.len()` 时，新增 `MonomorphError` 变体 `arity_mismatch`，报错而非静默跳过。

### 1d. 补全 `verify_no_generic_residue` 覆盖
**文件**: `crates/scoop2_mir/src/mir/verify.rs`
- 对齐 1b 的替换覆盖：检查所有已替换字段。
- 新增 EffectRow 残留检查（`check_effect_row_for_param`）。
- `check_type_for_param` 改为递归检查（`Function`/`Nominal`/`Option`/`Tuple` 内嵌的 Param）。

### 1e. 激活 verify_materialized
**文件**: `crates/scoop2c/src/main.rs`（`run_dump_mir`）
- 在 materialize 成功后（:564），对 `monomorph_result.module` 调用 `verify_materialized`（含 `verify_no_generic_residue`），把错误转为诊断。

### 1f. 测试
- 新增 materialize 单元测试：构造含泛型 `EnumVariant`/`StructLit`/`WithUpdate`/`PatternMatch` 的模块，验证替换后无 Param 残留。
- 新增 `build_subst` arity mismatch 测试。

---

## 阶段 2：单候选 devirtualization（CHA 基础设施）

### 2a. 在 HIR 暴露子类层次信息
**文件**: `crates/scoop2_hir/src/resolve/index.rs`
- 新增 `pub fn supertypes_iter(&self) -> impl Iterator<Item = (Symbol, &[Symbol])>`（暴露 `supertypes` 的迭代器）。

**文件**: `crates/scoop2_hir/src/hir/mod.rs`
- `TypedHir` 新增字段：
  - `direct_subtypes: HashMap<Symbol, Vec<Symbol>>`（super FQN → 直接子类 FQN 列表，反转 `supertypes`）
- `into_typed_hir` 中构建（遍历 `index.supertypes_iter()`，反转）。

### 2b. 构建去虚化上下文
**文件**: `crates/scoop2_mir/src/mir/devirtualize.rs`
- `DevirtContext` 新增字段：
  - `direct_subtypes: &HashMap<Symbol, Vec<Symbol>>`
  - `interner` 已有

新增 `exact_receiver_fqn` 函数（移植自 scoopc_mir）：
- 返回 `Some(fqn)` 当且仅当 receiver 的 nominal ref 不在 `direct_subtypes` 的 key 集合中（即无已知子类）。
- 值类型 nominal 总是精确。
- interface ref → None（interface 本身不可精确）。

新增 `descendants_and_self(root_fqn) -> Vec<Symbol>`：
- BFS 遍历 `direct_subtypes`，收集 root 及所有后代。

### 2c. 实现单候选去虚化
**文件**: `crates/scoop2_mir/src/mir/devirtualize.rs`

`devirtualize_call_kind` 扩展（保持现有 final-type 逻辑，新增单候选路径）：
- **Virtual**：若 `exact_receiver_fqn` 返回 `Some`，改写为 Direct（receiver 是无子类的具体 class）。
- **Interface**：用 `descendants_and_self` 找所有实现该 interface 方法的具体 class；若恰好 1 个候选，改写为 Direct。
- 若候选 > 1，保留 Virtual/Interface。

### 2d. 接入 pipeline
**文件**: `crates/scoop2_mir/src/mir/materialize/mod.rs`
- 构造 `DevirtContext` 时传入 `&hir.direct_subtypes`。

### 2e. 测试 + fixture
- 单元测试：构造含子类层次的模块，验证单候选去虚化。
- 新增 fixture `mir2/single_candidate_devirt.scoop`：open class + 单子类，验证 vtable 调用退化为 direct。

---

## 阶段 3：HOF inline pass 重写（effect-transparent）

### 3a. 实现 effect-transparency 检测
**文件**: `crates/scoop2_mir/src/mir/inline.rs`

新增 `is_effect_transparent(fd, store) -> bool`：
- 检查 `fd.effect_row` 的每个非 Pure term 是否都是某个函数类型参数的 effect row 中出现的 `Param`。
- 利用 hash-consing 的 TypeId 相等性：函数的 `effect_row.terms` 中的 `Param` TypeId 必须出现在某个参数的 `FunctionType.effects.terms` 中。
- 若所有 effect term 都可追溯到参数转发 → effect-transparent。

### 3b. 放宽 inline 门控
**文件**: `crates/scoop2_mir/src/mir/inline.rs`
- `try_make_inlineable`：把 `effect_row.is_pure()` 条件改为 `effect_row.is_pure() || is_effect_transparent(fd, store)`。
- 放宽单块限制：允许 **少量块**（如 ≤ 4 块），支持简单循环（forEach 的 while 循环形态）。
- 放宽语句上限：对 effect-transparent HOF 提高到合理值（如 30）。
- 支持 `Goto`/`CondBr` 终结符（多块内联）。

### 3c. 多块内联机制
**文件**: `crates/scoop2_mir/src/mir/inline.rs`
- 重写 `do_inline` 支持多块：
  - 把 callee 的所有块复制到 caller，重命名 block id 和 local id。
  - 入口块内联到调用点（splice 语句到当前块）。
  - `Return` 终结符改为：赋值结果到 target local + `Goto` 到调用点之后的续接块。
  - `Goto`/`CondBr` 目标重定向到复制后的新 block id。

### 3d. 闭包内联（第二级）
**文件**: `crates/scoop2_mir/src/mir/inline.rs`
- 新增 `extract_closure_inline_site`：当 `CallKind::Closure { callee, invoke_fqn }` 或 `CallKind::FunValue { callee }` 的 callee 是一个已知的 `MakeClosure` local 时，查找 invoke FunDecl，内联其 body。
- 闭包的 `$env` 参数绑定到 `MakeClosure` 的 env operand；其余参数绑定到调用实参。
- 修正闭包 effect row：读取 invoke FunDecl 的 `ty`（FunctionType.effects）而非硬编码的 `effect_row: Pure`（expr.rs:1841 的 bug）。

### 3e. 修复闭包 effect_row 硬编码
**文件**: `crates/scoop2_mir/src/mir/lower/expr.rs`（:1841）
- 把 `effect_row: EffectRow::pure()` 改为从 lambda 的推断函数类型 `.effects` 取真实 effect row。

### 3f. 测试
- 单元测试：构造 effect-transparent HOF + 闭包调用，验证两级内联。
- 验证 `forEach` 风格（多块 + effect 转发）可内联。

---

## 阶段 4：Stable key 全实体覆盖

### 4a. 构造器 stable key
**文件**: `crates/scoop2_mir/src/mir/transport.rs` + `mod.rs`
- `ClassCtorCallMetadata` 新增 `stable_template_key: Option<StableTemplateKey>`。
- `Rvalue::ClassCtor` lowering 中计算并填充（用 ctor 的 param_types 构建 overload_sig）。

### 4b. Enum variant stable key
**文件**: `crates/scoop2_mir/src/mir/transport.rs` + `mod.rs`
- `Rvalue::EnumVariant` 新增 `stable_key: Option<StableTemplateKey>`（基于 enum_fqn + variant_name + payload 类型）。

### 4c. 闭包 stable key
**文件**: `crates/scoop2_mir/src/mir/mod.rs`
- `MakeClosure` 新增 `stable_key: Option<StableTemplateKey>`。
- 闭包 invoke_fqn 的合成需稳定化：不用递增 counter，改用基于 owner_fqn + 位置（源 span 或 AST node id）的确定性命名。

### 4d. 修复 effect term 编码稳定性
**文件**: `crates/scoop2_mir/src/mir/stable_id.rs`（:150）
- `encode_effect_row_with_closed`：把 `format!("ty#{}", t.0)` 改为递归调用 `encode_type(store, interner, *t)`，使 effect term 编码基于 canonical 类型文本而非 session 相关的 TypeId 数字。
- 需要把 `types` + `interner` 传入该函数。

### 4e. 为所有 public 函数生成 stable key
**文件**: `crates/scoop2_mir/src/mir/mod.rs` + `materialize/mod.rs`
- `FunDecl` 新增 `stable_template_key: Option<StableTemplateKey>` 字段。
- 新增 pass `compute_public_stable_keys(module, interner)`：遍历所有顶层非 private 函数，计算 stable key 并填充（不依赖调用点）。

### 4f. 测试
- 单元测试：验证 ctor/variant/closure 的 stable key 跨"会话"稳定。
- 验证 effect term 编码不再含原始 TypeId 数字。

---

## 阶段 5：D-call/I-call fallback 路径修复

### 5a. 为 fallback 路径解析真实 owner
**文件**: `crates/scoop2_mir/src/mir/lower/expr.rs` + `lower/stmt.rs`

5 个 fallback 路径当前传 `Symbol::default()`，需改为从 receiver 类型解析真实 owner FQN：
- `expr.rs:809`（Not 经 equals）：从 inner operand 的类型解析 owner。
- `expr.rs:1137`（operator fallback）：同上。
- `stmt.rs:248`（index set）：从 receiver operand 解析 owner。
- `stmt.rs:488/539/592`（for-loop iterator/hasNext/next）：从 iterable operand 解析 owner。

新增辅助函数 `resolve_owner_fqn_from_operand(builder, operand) -> Symbol`：
- 取 operand 的类型 → 若是 `Ref(Nominal)` 或 `Value(Nominal)`，返回 nominal fqn。
- 否则返回 `Symbol::default()`（无法解析时退回原行为）。

### 5b. 测试 + fixture
- 扩展 `mir2/interface_dispatch.scoop`：添加 interface-backed 的 `set`/迭代场景，验证生成 `Interface` 而非 `Virtual`。

---

## 验证

每阶段完成后：
- `cargo build -p scoop2_hir -p scoop2_mir -p scoop2c` 无错误
- `cargo test -p scoop2_mir -p scoop2_hir` 全部通过
- `python3 tools/run_fixtures.py tests/fixtures/mir2/` 无回归（新 fixture 通过）
- `no_placeholder` 守卫通过（无 todo!/panic!/unimplemented!）
