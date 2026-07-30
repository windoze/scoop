# HIR 重新设计：正确性与完整性

> 状态：设计文档
> 原则：「正确」和「完整」，不是「先运行起来」。

---

## 1. 问题陈述

当前 HIR 存在两个根本性架构缺陷：

### 1.1 输出数据结构允许缺失

`NodeIdTable<T>` 内部是 `Vec<Option<T>>`，`get()` 返回 `Option<&T>`。这意味着：

- 编译器可以构造出一个「有洞」的 HIR 输出——某些节点有条目，某些没有。
- MIR 消费者必须处理 `None`（用 fallback、兜底值、或 panic）。
- 完整性只能靠运行时检查（completeness gate）事后验证，不是类型层面的保证。

**正确的设计**：HIR 输出的类型签名应该保证完整——如果某个表声称覆盖某类节点，那从数据结构上就不可能缺少条目。缺失只能发生在 typecheck 内部的中间状态（Phase 1-3），一旦 move 进最终输出（Phase 4），完整性是类型保证的。

### 1.2 编译器可异常终止

scoop2_hir 中有 8 处 `panic!`/`unreachable!`、14 处 `unwrap()`、7 处 `expect()`；scoop2_mir 中有 2 处 `panic!`/`unreachable!`、7 处 `unwrap()`。编译器不应该因为遇到意外输入而 crash——应该给出明确的诊断。

**正确的设计**：编译器的任何代码路径，遇到不该发生的情况时，要么：
- 把它当作源代码错误，产出一条诊断（`Diagnostic`），然后继续或停止编译。
- 如果确实不可能发生（例如 `Vec::new().first()` 在空数组上），用穷尽 match 的 `_` 分支返回一个错误结果，而非 `unreachable!`。

---

## 2. 设计目标

### 目标 A：HIR 输出的类型结构保证完整性

HIR 的最终产出 `TypedHir` 必须满足：

- **expr_types 覆盖所有表达式节点**：不存在「某个表达式节点没有类型」的可能性。类型签名上，如果 MIR 要读某表达式的类型，拿到的一定是 `TypeId`，不是 `Option<TypeId>`。
- **call_resolutions 覆盖所有 call site**：每个 Call/Operator/Infix/Index 节点都有对应的 `ResolvedCall`。
- **member_refs 覆盖所有 MemberAccess**：每个成员访问节点都有对应的 `ResolvedMember`。
- **type_ref_resolutions 覆盖所有 TypeRef 节点**：每个类型引用都有解析后的 `TypeId`。
- **expr_effect_rows 覆盖所有表达式**：每个表达式都有 outward effect row。

**实现方式**（择一或组合）：

1. **冻结式 NodeIdTable**：`NodeIdTable<T>` 在 typecheck 阶段用 `Vec<Option<T>>`（允许缺失）。move 进 `TypedHir` 时，调用一个 `freeze()` 方法，它遍历所有应覆盖的 NodeId，发现缺失则报诊断（`scoop::typecheck::incomplete_resolution`）。冻结后返回 `FrozenNodeIdTable<T>`，其 `get()` 返回 `&T`（不返回 Option），因为冻结过程已保证完整。

2. **整体性保证**：`TypedHir` 的构造函数（`into_typed_hir`）在 move 时做完整性验证。验证不通过则 `TypedHir` 不被构造（返回 `Result<TypedHir, Vec<Diagnostic>>`）。MIR 拿到的 `TypedHir` 从类型上保证完整。

3. **分开中间表和输出表**：typecheck 内部用可缺失的表（Phase 1-3）；Phase 4 把它们转换成不可缺失的输出表（缺失即诊断）。输出表的 API 返回 `&T` 而非 `Option<&T>`。

### 目标 B：编译器不 crash

`crates/scoop2_hir/` 中不允许出现：

- `panic!` / `todo!` / `unimplemented!` / `unreachable!`
- `unwrap()`（除非在 100% 保证 Some 的场景，且有注释说明为何安全）
- `expect()`（同上）

所有「不该发生」的情况要么：
- 改为穷尽 match 的 `_` 分支，返回错误结果或诊断。
- 如果是「解析器保证的非空」（如 `Vec` 的 `first()` 在已检查非空后调用），改用直接索引 `v[0]` 或保留 `unwrap` 但添加 `// SAFETY:` 注释说明为何保证非空。

当前 8 处 panic + 14 处 unwrap + 7 处 expect = 29 处需要逐一审查和修复。

### 目标 C：MIR 只消费，不做 resolve

MIR lower 层：

- **不调用任何 resolve 函数**：`resolve_typeref`、`owner_fqn_of`、`resolve_owner_fqn_from_operand` 全部删除。
- **没有 fallback 路径**：`lower_call`、`lower_unary`、`lower_via_call_resolution`、`lower_infix_call`、`lower_index` 的 None 分支全部删除。如果 HIR 保证了完整性（目标 A），这些分支不可能触发。
- **不做类型名解析**：所有 TypeRef → TypeId 的解析结果从 HIR `type_ref_resolutions` 获取。
- **不做 FQN 构建或字符串匹配**：所有 owner FQN、方法名等从 HIR `ResolvedCall`/`ResolvedMember` 直接读取。

当前需要删除/改写的 MIR resolve 代码：
- `resolve_typeref` / `resolve_typeref_fallback`（11 处调用）→ 改为 `type_ref_resolution(node_id)` 直接读取；`resolve_typeref_fallback` 整段删除
- `owner_fqn_of`（expr.rs，3 处调用）→ HIR `ResolvedCall::Method.owner_fqn` 已携带
- `owner_fqn_of_type`（stmt.rs）+ `resolve_owner_fqn_from_operand`（4 处调用）→ 不再需要（MIR 不从 operand/类型推导 owner）
- `derive_enum_variant_call`（expr.rs，**当前已是死代码，零调用方**）→ 整段删除
- `lower_call` / `lower_unary` / `lower_via_call_resolution` / `lower_infix_call` / `lower_index` 的 None fallback 分支 → 全部删除

---

## 3. 实施计划

### 阶段 1：HIR 输出完整性保证（目标 A）

1. 设计 `FrozenNodeIdTable<T>` 或等价机制。
2. 在 `into_typed_hir` 中实现冻结/验证逻辑。
3. `TypedHir` 的公开 API 改为返回 `&T`（非 Option）。
4. 遍历 AST 的所有应覆盖节点，确保覆盖。

### 阶段 2：消除 panic/unwrap（目标 B）

逐一审查 28 处 panic 级代码：
- 8 处 `panic!`/`unreachable!`：改为穷尽 match 或诊断。
- 14 处 `unwrap()`：审查安全性，不安全的改为诊断。
- 6 处 `expect()`：同上。

### 阶段 3：MIR 纯化（目标 C）

1. 删除所有 resolve 函数。
2. 删除所有 fallback 路径。
3. 所有信息从 HIR 输出直接读取。
4. MIR 的 `FnLowering` API 改为返回 `&T`（与 HIR 输出一致）。

### 阶段 4：验证

- `cargo test --all` 全绿。
- run-pass fixture 不回归。
- `no_placeholder` 守卫通过。
- 代码审计：grep 确认 0 处 panic/unwrap/fallback/resolve。

---

## 4. 验证清单

- [ ] `TypedHir` 的公开 API 返回 `&T`（非 Option）
- [ ] HIR 输出不可能构造出有缺失的结构
- [ ] scoop2_hir 中 0 处 `panic!`/`todo!`/`unimplemented!`/`unreachable!`
- [ ] scoop2_hir 中 0 处不安全的 `unwrap()`/`expect()`
- [ ] scoop2_mir lower/ 中 0 处 resolve 函数调用
- [ ] scoop2_mir lower/ 中 0 处 fallback 路径
- [ ] cargo test --all 通过
- [ ] run-pass fixture 不回归
- [ ] no_placeholder 守卫通过
