# Side Table 清理计划

## 问题概述

编译器 pipeline 目前存在 **60+ 个 side table**，分布在 AST / HIR / MIR 三层。其中真正有资格被称为"多阶段共享状态"的仅 **7 个**；其余 50+ 个要么是 typeck 产出被错误存放，要么是 fact 迁移未完成，要么是 AST side table 的冗余拷贝，要么是内部工作状态泄露。

### 历史成因

fact 机制是后来引入的架构改进——它定义了各阶段的 canonical 产出（`HirFacts` / `MirFacts` / `LirFacts`），下游阶段应该**只读 fact、不直接依赖上游 side table**。但迁移停留在"新代码用 fact，老代码继续读 side table"的半成品状态。side table 作为事实上的 handoff 通道仍被广泛使用。

### 危害

1. **双重维护**：同一信息在 AST side table（`Span → X`）和 HIR side table（`CallSite → X`）中各存一份，修改 typeck 时必须同步更新两处
2. **脆弱的 key**：`Span` 作为 key 对任何 AST 修改（甚至 reformat）敏感，导致静默失效
3. **不可序列化**：`RefCell<HashMap>` 嵌在 `ast::File` 上，无法跨进程传递或缓存
4. **隐式耦合**：下游阶段隐式依赖上游 side table 的存在，fact 机制形同虚设
5. **内部状态泄露**：materializer 的工作状态（`type_store`、`instance_map`）作为 side table 暴露，破坏封装

---

## 现状全景

### AST 层 Side Table（20 个 — 全部为设计缺陷）

**定义位置**：`crates/scoopc_ast/src/ast/mod.rs:36-171`
**存储方式**：`RefCell<HashMap<Span, ...>>` 挂在 `ast::File` 上
**写入者**：typecheck
**读取者**：HIR lowering / MIR lowering / LLVM codegen

| # | 名称 | 类型 | 内容 |
|---|------|------|------|
| 1 | `inferred_expr_tys` | `Span → TypeId` | 表达式推断类型 |
| 2 | `inferred_binding_tys` | `Span → TypeId` | 绑定推断类型 |
| 3 | `inferred_fun_return_tys` | `Span → TypeId` | 函数返回类型 |
| 4 | `inferred_performed_effect_tys` | `Span → TypeId` | perform 的效应类型 |
| 5 | `inferred_handle_arm_effect_tys` | `Span → TypeId` | handle arm 的效应类型 |
| 6 | `inferred_handle_arm_op_type_args` | `Span → Vec<TypeId>` | handle arm 操作类型参数 |
| 7 | `safe_member_access_resolved` | `Span → ResolvedMemberRef` | safe member access 解析 |
| 8 | `typechecked_member_resolved` | `Span → ResolvedMemberRef` | 类型化 member 解析 |
| 9 | `splice_field_contracts` | `Span → SpliceFieldContract` | splice 字段契约 |
| 10 | `with_update_contracts` | `Span → WithUpdateContract` | with update 契约 |
| 11 | `assign_place_contracts` | `Span → AssignPlaceContract` | assign place 契约 |
| 12 | `continuation_resume_call_sites` | `HashSet<Span>` | continuation resume 调用点 |
| 13 | `non_pure_continuation_resume_call_sites` | `HashSet<Span>` | 非纯 continuation resume |
| 14 | `zero_arg_unit_call_sugar_sites` | `HashSet<Span>` | 零参数 unit 调用糖 |
| 15 | `top_level_fun_value_refs` | `Span → TopLevelFunValueRef` | 顶层函数值引用 |
| 16 | `top_level_fun_call_bindings` | `Span → TopLevelFunCallBinding` | 顶层函数调用绑定 |
| 17 | `typechecked_call_arg_bindings` | `Span → CallArgBinding` | 类型化调用参数绑定 |
| 18 | `typechecked_effect_op_call_bindings` | `Span → EffectOpCallBinding` | 效应操作调用绑定 |
| 19 | `typechecked_ctor_call_bindings` | `Span → CtorCallBinding` | 构造函数调用绑定 |
| 20 | `release_hook_bindings` | `String → ReleaseHookBinding` | 释放钩子绑定 |

**问题**：这些是 typecheck 阶段的**产出**，语义上就是 typeck fact。它们被塞进 `ast::File` 的 `RefCell` 纯粹因为 typecheck 发生时还没有 fact 机制——AST 节点被下游共享，side table 成了"顺路搭车"的发布通道。

### HIR 层 Side Table（~35 个 — 混合）

**定义位置**：`crates/scoopc_hir/src/hir/lower/types.rs:230-340`（`LoweredHir` 结构体）
**写入者**：HIR lowering（大部分）、typecheck（调用点索引）
**读取者**：MIR lowering / materializer / LLVM codegen / effect facts stage

#### 后端布局索引（15 个，按 FQN 索引 — 应迁移为 fact）

| # | 名称 | 类型 | 内容 |
|---|------|------|------|
| 21 | `struct_layouts` | `String → StructLayout` | 结构体内存布局 |
| 22 | `enum_layouts` | `String → EnumLayout` | 枚举内存布局 |
| 23 | `object_inits` | `String → ObjectInit` | 对象初始化描述 |
| 24 | `class_inits` | `ClassInstanceKey → MonoClassInit` | 类初始化描述 |
| 25 | `generic_class_decls` | `String → GenericClassDecl` | 泛型类声明 |
| 26 | `release_hooks` | `String → ReleaseHook` | 释放钩子 |
| 27 | `extern_funs` | `String → ExternFun` | 外部函数 |
| 28 | `native_callable_funs` | `String → NativeCallableFun` | 原生可调用函数 |
| 29 | `extern_globals` | `String → ExternGlobal` | 外部全局变量 |
| 30 | `top_level_vars` | `String → TopLevelVar` | 顶层可变变量 |
| 31 | `top_level_immutable_values` | `String → TopLevelImmutableValue` | 顶层不可变值 |
| 32 | `nominal_kinds` | `String → ast::TypeKind` | 名义类型种类 |
| 33 | `interior_mutable_nominals` | `HashSet<String>` | 内部可变名义类型 |
| 34 | `nominal_variances` | `String → Vec<Variance>` | 类型参数变型 |
| 35 | `direct_supertypes` | `String → Vec<String>` | 直接超类型 |

**问题**：这些是按 FQN 索引的"全局知识"，HIR lowering 产出后由 LLVM codegen 消费。它们完全满足 fact 的定义（单阶段产出、下游消费、语义稳定），保留为 side table 的唯一理由是迁移工作量。

#### 调用点/站点索引（12 个 — 是 AST side table 的冗余拷贝）

| # | 名称 | 类型 | 内容 |
|---|------|------|------|
| 36 | `top_level_fun_call_sites` | `CallSite → TopLevelFunCallBinding` | 顶层函数调用绑定 |
| 37 | `top_level_fun_value_refs` | `CallSite → TopLevelFunValueRef` | 顶层函数值引用 |
| 38 | `call_arg_bindings` | `CallSite → CallArgBinding` | 调用参数绑定 |
| 39 | `with_update_contracts` | `CallSite → WithUpdateContract` | with update 契约 |
| 40 | `assign_place_contracts` | `CallSite → AssignPlaceContract` | assign place 契约 |
| 41 | `dispatch_call_sites` | `CallSite → DispatchCallKind` | 分发调用点 |
| 42 | `effect_op_call_sites` | `CallSite → EffectOpCallInfo` | 效应操作调用点 |
| 43 | `handle_payload_tuple_tys` | `CallSite → TypeId` | handle payload 元组类型 |
| 44 | `ctor_call_sites` | `CallSite → CtorCallInfo` | 构造函数调用点 |
| 45 | `continuation_resume_call_sites` | `HashSet<CallSite>` | continuation resume 调用点 |
| 46 | `non_pure_continuation_resume_call_sites` | `HashSet<CallSite>` | 非纯 continuation resume |
| 47 | `when_pat_binding_tys` | `WhenPatBindingSite → TypeId` | when 模式绑定类型 |

**问题**：这些是 AST side table 的 HIR 层镜像——typeck 写入 `Span → X` 后，HIR lowering 将其转换为 `CallSite → X` 再存一份。因为 AST side table 用 `Span` 做 key 太脆弱且不可序列化，HIR 层被迫重建。**两层 side table 表达同一批 typeck 产出，双倍维护成本。**

#### 真正合理的共享状态（7 个）

| # | 名称 | 内容 | 理由 |
|---|------|------|------|
| 48 | `generic_stable_template_keys` | `TemplateKey → StableTemplateKey` | 跨阶段 identity registry |
| 49 | `stable_type_param_keys` | `TypeParamType → StableTypeParamKey` | 跨阶段类型参数 identity |
| 50 | `generic_template_inventory` | 模板清单 | materializer 入口目录 |
| 51 | `callable_body_inventory` | callable body 清单 | materializer 入口目录 |
| 52 | `class_vtables` | `String → Vec<ClassVtableSlot>` | 全局虚表（多阶段消费） |
| 53 | `interfaces` | `String → InterfaceInfo` | 全局接口信息 |
| 54 | `class_itables` | `String → Vec<ClassItableEntry>` | 全局接口表 |

### MIR 层 Side Table（~8 个 — 半迁移状态 / 内部状态泄露）

| # | 名称 | 内容 | 问题 |
|---|------|------|------|
| 55 | `mir_bodies` | `BodyId → MirBody` | 已有 `MirFacts`，但 body 本体仍以 side table 存 |
| 56 | `body_source_map` | `BodyId → SourceInfo` | 应合并进 `MirFacts` |
| 57 | `type_store` | materializer 类型分配 | 🔴 materializer 内部状态，不应暴露 |
| 58 | `instance_map` | monomorph instance → body | 🔴 同上 |

---

## 建议的修改方案

### 总体原则

```
         Typecheck          HIR Lowering        MIR Materializer
            │                    │                     │
            ▼                    ▼                     ▼
      TypeckFacts           HirFacts              MirFacts
      (新增)                (已有，扩展)           (已有，扩展)
            │                    │                     │
            └────────────────────┼─────────────────────┘
                                 │
                                 ▼
                         下游阶段只读 fact
```

- **fact = 单向产出-消费**：每个阶段产出 fact，下游只读 fact，不回写
- **side table = 真·跨阶段共享状态**：仅保留 identity registry 和 dispatch 表
- **消除 `Span` 索引**：所有 fact key 使用 `(source_path, stable_span)` 或结构化 identity

### Phase 1：消除 AST 层 side table（优先级最高）

**目标**：新建 `TypeckFacts`，将 AST 层 20 个 side table 全部迁移进去。

```
crates/scoopc_typeck/src/facts.rs  (新建)

pub struct TypeckFacts {
    // 类型推断
    pub inferred_expr_tys: HashMap<StableSpan, TypeId>,
    pub inferred_binding_tys: HashMap<StableSpan, TypeId>,
    pub inferred_fun_return_tys: HashMap<StableSpan, TypeId>,
    pub inferred_performed_effect_tys: HashMap<StableSpan, TypeId>,
    pub inferred_handle_arm_effect_tys: HashMap<StableSpan, TypeId>,
    pub inferred_handle_arm_op_type_args: HashMap<StableSpan, Vec<TypeId>>,

    // Name / overload resolution
    pub safe_member_access_resolved: HashMap<StableSpan, ResolvedMemberRef>,
    pub typechecked_member_resolved: HashMap<StableSpan, ResolvedMemberRef>,
    pub top_level_fun_value_refs: HashMap<StableSpan, TopLevelFunValueRef>,
    pub top_level_fun_call_bindings: HashMap<StableSpan, TopLevelFunCallBinding>,
    pub typechecked_call_arg_bindings: HashMap<StableSpan, CallArgBinding>,
    pub typechecked_effect_op_call_bindings: HashMap<StableSpan, EffectOpCallBinding>,
    pub typechecked_ctor_call_bindings: HashMap<StableSpan, CtorCallBinding>,
    pub release_hook_bindings: HashMap<String, ReleaseHookBinding>,

    // 契约推导
    pub splice_field_contracts: HashMap<StableSpan, SpliceFieldContract>,
    pub with_update_contracts: HashMap<StableSpan, WithUpdateContract>,
    pub assign_place_contracts: HashMap<StableSpan, AssignPlaceContract>,

    // 语义标记
    pub continuation_resume_call_sites: HashSet<StableSpan>,
    pub non_pure_continuation_resume_call_sites: HashSet<StableSpan>,
    pub zero_arg_unit_call_sugar_sites: HashSet<StableSpan>,
}
```

**步骤**：

1. **新建 `TypeckFacts` 结构体**，key 从 `Span` 改为 `StableSpan = (source_path, start, end)`
2. **修改 typecheck 代码**：所有写入 `ast::File` side table 的地方改为写入 `TypeckFacts`
3. **修改 HIR lowering**：从 `TypeckFacts` 读取，不再从 `ast::File` side table 读取
4. **删除 HIR 层的调用点索引**（#36-#47，12 个）——它们只是 AST side table 的拷贝，`TypeckFacts` 直接用 `StableSpan` 即可替代 `CallSite`
5. **删除 `ast::File` 上全部 20 个 `RefCell` 字段**

**风险**：低。typeck → HIR lowering 是紧耦合的单线程过程，fact 在此场景下就是显式化的 handoff，不会引入新的并发或生命周期问题。

### Phase 2：扩展 HirFacts，迁移布局索引

**目标**：将 HIR 层 15 个布局索引（#21-#35）迁移到 `HirFacts`。

```rust
// crates/scoopc_hir/src/facts.rs 已有 HirFacts，扩展如下字段：

pub struct HirFacts {
    // ... 现有字段 ...

    // 后端布局
    pub struct_layouts: HashMap<String, StructLayout>,
    pub enum_layouts: HashMap<String, EnumLayout>,
    pub object_inits: HashMap<String, ObjectInit>,
    pub class_inits: HashMap<ClassInstanceKey, MonoClassInit>,
    pub generic_class_decls: HashMap<String, GenericClassDecl>,
    pub release_hooks: HashMap<String, ReleaseHook>,
    pub extern_funs: HashMap<String, ExternFun>,
    pub native_callable_funs: HashMap<String, NativeCallableFun>,
    pub extern_globals: HashMap<String, ExternGlobal>,
    pub top_level_vars: HashMap<String, TopLevelVar>,
    pub top_level_immutable_values: HashMap<String, TopLevelImmutableValue>,

    // 类型元数据
    pub nominal_kinds: HashMap<String, TypeKind>,
    pub interior_mutable_nominals: HashSet<String>,
    pub nominal_variances: HashMap<String, Vec<Variance>>,
    pub direct_supertypes: HashMap<String, Vec<String>>,
}
```

**步骤**：

1. 在 `HirFacts` 中新增上述字段
2. 修改 HIR lowering：构建 `HirFacts` 时填充这些字段
3. 修改 LLVM codegen：从 `HirFacts` 读取，不再从 `LoweredHir` 的 side table 读取
4. 从 `LoweredHir` 结构体中删除对应的 15 个字段

**风险**：低。这些数据已经是 HIR lowering 产出、LLVM codegen 消费的单向流，fact 机制就是为此设计的。

### Phase 3：消除 MIR 层的内部状态泄露

**目标**：将 materializer 的内部工作状态封在 materialization 内部，产出写入 `MirFacts`。

**步骤**：

1. **`type_store`**：materializer 完成工作后，不需要持久化的类型信息写入 `MirFacts`，其余在 materialization 结束时释放
2. **`instance_map`**：monomorph instance → body 的映射应作为 `MirFacts` 的一部分发布，而非暴露为 side table
3. **`body_source_map`**：合并进 `MirFacts`
4. 删除 `MaterializedMir` 上不再需要的 side table 字段

**风险**：中。materializer 的内部依赖较复杂，需要仔细梳理哪些数据是内部临时状态、哪些是阶段产出。

### Phase 4：梳理调用点解析的 fallback 链（与 FG-04、FG-09 联动）

**目标**：当 `TypeckFacts` 和扩展后的 `HirFacts` / `MirFacts` 到位后，消除下游阶段中的多级 fallback 逻辑。

**关联的 fact gap**：

- **FG-04**：`CallKind::Direct` 应从 `String` FQN 升级为 resolved `InstanceKey`，由 upstream fact 直接提供
- **FG-09**：P4 call-site target/declared row 应从 `MirFacts` 直接获取，消除 FQN + arg count + receiver type 的多级 fallback

**步骤**：

1. `MirFacts` 中的 call site 信息增加 `resolved_instance: InstanceKey` 和 `declared_effect_row: EffectRow`
2. 修改 MIR lowering：在构建 MIR 时解析 call target 并写入 fact
3. 修改 P4 effect facts stage：直接从 fact 读取，删除 fallback 链
4. 修改 P5 LIR facts：同上述

**风险**：中高。涉及 MIR lowering 和 effect facts stage 的核心逻辑，需要充分的回归测试。

---

## 实施优先级

| Phase | 内容 | 收益 | 风险 | 建议 |
|-------|------|------|------|------|
| Phase 1 | 消除 AST side table + HIR 调用点索引 | 消除 32 个 side table，打破双重维护 | 低 | **立即开始** |
| Phase 2 | 扩展 HirFacts | 消除 15 个 side table | 低 | Phase 1 之后 |
| Phase 3 | 清理 MIR 内部状态 | 消除 4 个 side table | 中 | Phase 2 之后 |
| Phase 4 | 消除 fact gap fallback | 消除下游重复推导 | 中高 | Phase 3 之后，需充分测试 |

---

## 不变与保留

以下 side table 作为真正的"多阶段共享状态"应该保留，不做迁移：

| 名称 | 保留理由 |
|------|----------|
| `generic_stable_template_keys` | 跨阶段 identity registry，需要双向查找 |
| `stable_type_param_keys` | 同上 |
| `generic_template_inventory` | materializer 入口目录，全局视图 |
| `callable_body_inventory` | 同上 |
| `class_vtables` | 全局虚表，多阶段消费 |
| `interfaces` | 全局接口信息，多阶段消费 |
| `class_itables` | 全局接口表，多阶段消费 |

它们共同特征是：**不随单个函数体变化，生命周期等同于整个 compilation unit，且被多个下游阶段以不同方式索引**。fact 机制是按 body 组织的单向流，不适合这些全局、多索引的数据结构。

---

## 验证标准

每完成一个 Phase 后运行：

```bash
# 完整测试套件
cargo test --all --all-targets

# 回归测试
python3 tools/run_fixtures.py

# spec doctest
python3 tools/spec_fixtures.py check
```

目标：每个 Phase 完成后所有测试保持绿色，无新增 regression。
