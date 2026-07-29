# 新 LIR 实现计划

> 状态：实现计划
> 目标：在 `scoop2_lir` crate 中实现 MIR 和 codegen 之间的准备层。
> LIR 将 MIR 的语义级 IR 整理成 codegen 可以 1:1 机械翻译的结构，包括布局计算、ABI 决策、分发表生成、GC root 信息、effect step 准备。
> LIR 不做代码优化（那是 codegen 自己的事），只做布局计算和契约物化。
> 产出的 `LirProgram` 是自包含的，codegen 无需回查 HIR/MIR。

## 0. 前提与约束

### 0.1 输入

LIR 消费 `scoop2_mir::mir::materialize::MaterializedMir`，含：
- `module: Module` — 单态化后的 IR（含 effect lowering 后的状态机）
- `instance_keys: Vec<InstanceKey>` — 单态化实例
- `backend_contracts: BackendContracts` — 语言级布局/分发表（仅有结构信息，无槽位/偏移）

同时需要 `scoop2_hir::hir::TypedHir` 用于查询类型声明细节（members、enum_variants 等）。

### 0.2 输出

`LirProgram` — 自包含的结构，codegen 无需回查 HIR/MIR：
- 函数体（机械可翻译的 IR）
- 类型布局表
- 分发表（vtable/itable）
- GC 信息（类型描述符 + root map）
- 全局/类初始化计划
- Effect step 准备信息（frame schema、step layout、continuation layout）

### 0.3 设计原则

- **机械翻译友好**：codegen 遍历 LIR 结构时不需要做任何推断，所有决策已在 LIR 完成
- **后端无关**：LIR 不依赖 LLVM/C/WASM 的任何概念，布局计算使用抽象的 size/align/offset
- **自包含**：`LirProgram` 携带 codegen 所需的全部信息
- **无优化**：LIR 不做经典优化（常量折叠/死代码消除等），只做布局计算和契约物化；Scoop 专有优化（状态机清理等）在单独的 pass 中，位于 LIR 之后

### 0.4 管线位置

```
HIR → MIR (lower → materialize → devirtualize → inline → effect_lower → stable_keys)
    → LIR (layout → dispatch → abi → gc → effect_prep → global_init)
    → Codegen (LLVM / C / WASM)
```

---

## 1. Crate 结构

```
crates/scoop2_lir/
├── Cargo.toml
└── src/
    ├── lib.rs                    # 公开 API + LirProgram
    ├── program.rs                # LirProgram 容器 + 所有产出类型
    ├── layout/
    │   ├── mod.rs                # 类型布局 pass 入口
    │   ├── value.rs              # 值类型布局（struct/enum/tuple/option/scalar）
    │   ├── reference.rs          # 引用类型布局（class/interface/object）
    │   └── table.rs              # LayoutTable 数据结构
    ├── dispatch/
    │   ├── mod.rs                # 分发表 pass 入口
    │   ├── vtable.rs             # class vtable 生成
    │   └── itable.rs             # interface itable 生成
    ├── abi/
    │   ├── mod.rs                # ABI pass 入口
    │   ├── calling_convention.rs # 调用约定决策
    │   ├── closure.rs            # 闭包对象布局
    │   └── symbol.rs             # 符号名 mangling
    ├── gc/
    │   ├── mod.rs                # GC pass 入口
    │   ├── type_descriptor.rs    # 类型描述符（trace bitmap）
    │   └── root_map.rs           # Safepoint root map
    ├── effect/
    │   ├── mod.rs                # Effect step 准备 pass 入口
    │   ├── frame.rs              # Frame schema 物化
    │   ├── step.rs               # Step enum 布局
    │   └── continuation.rs       # Continuation 对象布局
    ├── global_init/
    │   └── mod.rs                # 全局初始化规划
    ├── verify.rs                 # LIR 验证（结构完整性）
    └── dump.rs                   # 调试输出
```

---

## 2. 实现步骤

### 步骤 1：Crate 骨架 + LirProgram 数据结构

**文件**：`crates/scoop2_lir/Cargo.toml`、`src/lib.rs`、`src/program.rs`

定义 `LirProgram` 容器和所有产出类型的数据结构。这一步只定义类型，不实现任何 pass。

`LirProgram` 的核心字段：
- `callables: Vec<LirCallable>` — 函数体
- `type_layouts: TypeLayoutTable` — 类型布局表
- `vtables: Vec<VtableLayout>` — class vtable
- `itables: Vec<ItableLayout>` — interface itable
- `type_descriptors: Vec<TypeDescriptor>` — GC 类型描述符
- `gc_root_maps: Vec<GcRootMap>` — safepoint root map
- `global_init: GlobalInitPlan` — 全局初始化计划
- `class_inits: Vec<ClassInitPlan>` — 类初始化计划

`LirCallable` 的核心字段：
- `fqn: String`
- `symbol_name: String` — mangled 符号名
- `abi: LirCallableAbi` — Plain 或 EffectStep
- `params: Vec<LirParam>` — 含 ABI 信息
- `return_ty: TypeLayoutId` — 返回值布局
- `body: LirBody` — 函数体（locals + blocks）
- `frame_schema: Option<FrameSchema>` — EffectStep 专用
- `step_layout: Option<StepLayout>` — EffectStep 专用
- `continuation_layout: Option<ContinuationLayout>` — EffectStep 专用

`LirBody` 的核心字段：
- `locals: Vec<LirLocalDecl>` — 含 GC traceable 标志
- `blocks: Vec<LirBlock>` — 语句 + 终结符（从 MIR 1:1 映射）

**验证**：crate 编译通过。

### 步骤 2：类型布局计算

**文件**：`src/layout/`

为每个单态化后的具体类型计算目标无关的抽象布局。

**2a. 标量布局**
- Unit/Bool/Char/Float64/Float32/Int/UInt/IntN/UIntN：固定 size + align
- String/Any：引用布局（GC-managed pointer）

**2b. 复合值类型布局**
- Struct：按字段声明顺序计算偏移（含对齐 padding），总大小 + 对齐
- Tuple：同 struct（匿名字段）
- Option<T>：分析 T 的 niche 存储可能性（指针 null niche、u8 sentinel 等），选择最优编码
- Enum：tagged union 布局（tag 宽度 + payload union），或 value-only（单变体无 payload）

**2c. 引用类型布局**
- Class：GC 对象头（固定大小）+ 展平字段列表（含超类链）
- Interface：引用布局（itable 指针）
- Object：单例引用布局

**2d. 布局表**
```rust
TypeLayoutTable {
    entries: HashMap<TypeId, TypeLayout>,
}
TypeLayout {
    size: u64,
    align: u64,
    kind: TypeLayoutKind,
}
TypeLayoutKind {
    Scalar,
    Struct { fields: Vec<FieldLayout> },
    Enum { tag_size: u64, variants: Vec<EnumVariantLayout> },
    Option { storage: NicheStorage, payload: Box<TypeLayout> },
    Tuple { elements: Vec<FieldLayout> },
    Reference { gc_traceable: bool, kind: RefKind },  // Class/Interface/String/Any/Object
    Function,  // 函数值是引用
}
FieldLayout { offset: u64, ty: TypeLayoutId }
EnumVariantLayout { tag_value: u64, payload: Option<TypeLayoutId> }
NicheStorage { Pointer, U8, None }
```

**信息来源**：
- TypeStore：类型的结构形状
- HIR `members`：struct/class 的字段声明
- HIR `enum_variants`：enum 的变体声明
- HIR `index.supertypes_of`：class 继承链
- HIR `interface_fqns`：interface 集合
- HIR `extensible_class_fqns`：可继承 class 集合（影响 vtable 决策）

**验证**：为内置类型（Int/Bool/Option<Int>/Tuple<Int,Bool>）计算布局，验证 size/align 正确。

### 步骤 3：分发表生成

**文件**：`src/dispatch/`

**3a. Class vtable**
- 遍历 `BackendContracts.class_vtables`
- 为每个 class 的虚方法分配 slot index（按声明顺序）
- 构建 vtable 对象布局：`[fn_ptr_0, fn_ptr_1, ...]`
- 每个 slot 的函数指针目标：`owner_fqn.method_name`（overload_sig 消歧）
- 如果 class 有超类，先继承超类的 vtable slots，再追加自己的

```rust
VtableLayout {
    class_fqn: String,
    slots: Vec<VtableSlot>,
}
VtableSlot {
    slot_index: u32,
    method_name: String,
    owner_fqn: String,
    overload_sig: String,
    target_symbol: String,  // mangled 函数符号名
}
```

**3b. Interface itable**
- 遍历 `BackendContracts.interfaces` 和 `BackendContracts.class_itables`
- 为每个 interface 的方法分配 slot index
- 为每个 class × interface 组合，构建方法实现映射
- 分配 interface_id（全局唯一 u64）
- 构建 match_ids（用于运行时 interface 匹配）

```rust
ItableLayout {
    interface_fqn: String,
    interface_id: u64,
    slots: Vec<ItableSlot>,  // interface 方法的 slot 定义
}
ClassItableLayout {
    class_fqn: String,
    interface_fqn: String,
    method_impls: Vec<Option<String>>,  // slot_index → impl symbol
}
ItableSlot {
    slot_index: u32,
    method_name: String,
    overload_sig: String,
}
```

**3c. 调用点解析**
- 遍历所有函数体中的 `CallKind::Virtual`/`Interface` 调用点
- 解析每个调用点为 `(receiver_ptr, vtable_slot_offset)` 或 `(receiver_ptr, itable_interface_id, itable_slot_index)`
- 更新调用点的 metadata

**验证**：为含虚方法的 class 计算 vtable，验证 slot 分配正确。

### 步骤 4：ABI 决策

**文件**：`src/abi/`

**4a. 符号名 mangling**
- 为每个函数生成唯一的 mangled 符号名
- 基于函数 FQN + overload_sig + type_args（使用 stable key 的 canonical 文本）
- extern/native 函数保留原始符号名

**4b. 调用约定**
- Plain 函数：参数 ABI（每个参数是 direct 还是 indirect/byref）
  - 标量/引用：direct
  - 大 aggregate（struct/tuple > 阈值）：indirect（by hidden pointer）
- EffectStep 函数：
  - 签名：`step(frame_ptr, resume_payload?) -> Step`
  - frame_ptr 是 GC-managed pointer
- Extern/native 函数：保留声明的 calling convention

**4c. 闭包对象布局**
- MIR 提供 `ClosureEnvTransportMetadata { env_ty, captures }`
- LIR 定义闭包对象布局：`{ invoke_fn_ptr, env_ptr }` 或 `{ invoke_fn_ptr, env_inline }`
- env 布局由 captures 列表决定

**4d. 返回值 ABI**
- 小返回值：direct
- 大返回值：indirect（caller 传出 hidden pointer）
- EffectStep 返回 Step：direct（tagged union 按值返回或指针）

**验证**：为简单函数生成 mangled 符号名 + ABI 信息。

### 步骤 5：GC 信息生成

**文件**：`src/gc/`

#### GC 安全模型：语义标记 + 后端自决

**核心设计原则**：LIR 只产出**语义标记**（"哪些 local 在哪些点是 GC 可见的"），不规定实现机制。codegen 根据自己的能力和目标平台选择最合适的实现策略。

**问题背景**：codegen 优化器（如 LLVM opt）可能把一个 GC-managed 引用 local 从 frame slot 提升到寄存器。如果该 local 在 safepoint 前被写入但未同步回 GC 可见位置，GC 移动对象后引用失效。不同后端对此有不同的解决能力：
- 当前运行时 GC 不支持 register root（需要平台相关汇编代码）
- 未来可能加上 register root 支持
- 不同 codegen 后端（LLVM/C/WASM）有不同的约束和能力

**LIR 的职责**：产出足够的语义信息，让 codegen 能自行做出正确的 root 管理决策：
1. 哪些 local 是 GC-managed 引用（`gc_traceable` 标志）
2. 每个函数中哪些点是 safepoint（调用点、effect 挂起点、GC intrinsic 调用）
3. 每个 safepoint 处哪些 GC-managed local 是 live 的

**LIR 不规定**：
- root 存储在哪里（frame slot、寄存器、statepoint stack map）
- 如何在 safepoint 前后同步 root
- root frame 的具体布局（由 codegen 决定）

#### 5a. GC 标记数据结构

```rust
/// 函数的 GC 语义信息（LIR 产出，codegen 消费）。
GcInfo {
    /// 此函数中所有 GC-managed local 的列表。
    /// codegen 可据此决定 root 管理策略。
    gc_locals: Vec<GcLocal>,
    /// 此函数中的所有 safepoint。
    safepoints: Vec<GcSafepoint>,
}

/// 一个 GC-managed local 的语义信息。
GcLocal {
    local_id: u32,
    /// local 的引用类型（Class/String/Any/Interface 等）。
    ty: TypeLayoutId,
    /// 此 local 的基指针来源。
    /// None = 此 local 本身就是基指针（指向对象起始）。当前 Scoop 总是如此。
    /// Some(base_local_id) = 此 local 是 derived pointer（对象内部指针），
    ///   GC 移动对象时需要通过 base_local 找到对象起始来更新。
    /// 当前 Scoop 不产生 derived pointer，此字段总是 None。
    /// 预留给未来（如数组内部指针遍历）。
    base_local: Option<u32>,
}

/// 一个 safepoint 的语义信息。
/// codegen 必须保证：在此点执行时，GC 能找到所有 live GC local 的当前值。
GcSafepoint {
    block_id: u32,
    stmt_index: u32,
    /// safepoint 的类型（决定 codegen 如何包装此点）。
    kind: SafepointKind,
    /// 在此 safepoint 存活的 GC-managed local 列表。
    /// codegen 负责确保这些 local 的值在 GC 扫描时是最新的。
    /// 对于 LLVM statepoint：每个 local 对应一个 gc.relocate，
    ///   base/derived index 从 local_id 推导（当前 base==derived）。
    live_gc_locals: Vec<u32>,
}

/// safepoint 的类型。
enum SafepointKind {
    /// 函数调用（codegen 用此信息包装 statepoint 或插入 store/load）。
    /// callee_symbol 用于 LLVM statepoint 的 call target。
    Call { callee_symbol: String },
    /// 纯 GC safepoint（无实际调用，只是让 GC 有机会运行）。
    /// codegen 生成一个 safepoint poll（如读取 GC 页面标志）。
    Poll,
    /// Effect 挂起点（Step 返回，控制权交给调用者）。
    /// 对于 EffectStep 函数：函数返回时 frame 中的 GC 引用需要被 GC 可见。
    EffectSuspend,
}
```

#### 5b. Codegen 后端的实现选择

LIR 产出 `GcInfo` 后，各 codegen 后端根据自身能力选择实现策略：

| 后端 | 策略 | 机制 |
|------|------|------|
| **LLVM（无 register root）** | 显式 root frame | 分配 `{ScoopRootFrameHeader, [N x ptr]}` alloca；safepoint 前 store live GC local 到 frame slot；push/pop root frame 到 runtime TLS。LLVM 不会消除这些 store（frame 通过 runtime-visible 指针链访问）。 |
| **LLVM（有 register root）** | statepoint intrinsic | 使用 `llvm.experimental.gc.statepoint`，LLVM 自动管理 root 在寄存器/栈中的同步，生成 stack map 供 GC 使用。 |
| **C** | C 局部变量 + root frame | C ABI 保证函数调用边界 live 变量写回栈。safepoint（函数调用）天然保证 GC 引用在栈上。root frame 作为额外的 GC-visible 结构。 |
| **WASM** | 引擎管理 | WASM 局部变量由引擎管理，不存在寄存器提升。直接用局部变量即可，引擎负责 GC root 追踪。 |
| **未来后端（register root）** | 平台相关 root save/restore | safepoint 前用汇编保存寄存器中的 GC 引用到 GC-visible 位置；safepoint 后恢复。 |

#### 5c. 类型描述符（TypeDescriptor）

为每个 GC-managed 类型生成运行时类型描述符（这部分与后端无关，是类型布局的延伸）：
```rust
TypeDescriptor {
    type_fqn: String,
    size: u64,
    align: u64,
    trace_offsets: Vec<u64>,  // GC 指针偏移列表（精确版）
    release_fn: Option<String>,  // @ReleaseHook 函数符号
    type_id: u64,  // RTTI 类型 ID
    parent_type_id: Option<u64>,  // 超类 type_id
}
```

trace_offsets 计算方法：
- 遍历类型的所有字段（使用 LIR 步骤 2 的布局结果）
- 对每个字段，如果其类型是 GC-managed（`trace: bool` from MIR transport metadata），记录其偏移
- 递归展开 struct/tuple 内嵌的 GC 指针

#### 5d. Safepoint + liveness 计算

计算方法：
- 使用 MIR effect_lower 的 liveness 分析结果（`compute_live_out`）
- safepoint = 所有 `Rvalue::Call` 语句 + effect 挂起点（Step 返回）+ GC intrinsic 调用
- 对每个 safepoint，找出在该点存活且 GC-traceable 的 locals（`live_gc_locals`）
- 对于 EffectStep 函数，frame tuple 中 GC-traceable 的 slot 也要记录（作为特殊的 GcLocal）

**验证**：为含 class 实例的函数生成 GcInfo，验证 safepoint 覆盖完整、live GC local 列表正确。

### 步骤 6：Effect Step 准备

**文件**：`src/effect/`

**6a. Frame schema 物化**
- 从 `EffectStepAbi.frame_ty`（tuple TypeId）展开为具体 slot 表
- 从 `EffectStepAbi.frame_local` + `state_local` 确定帧在函数体中的位置

```rust
FrameSchema {
    frame_ty: TypeLayoutId,
    slots: Vec<FrameSlot>,
}
FrameSlot {
    slot_index: u32,
    kind: FrameSlotKind,  // State | SourceLocal
    ty: TypeLayoutId,
    gc_traceable: bool,
}
```

**6b. Step enum 布局**
- 从 `EffectStepAbi.step_ty` + `step_variants` 构建 tagged union 布局
- Complete variant 的 payload = 原始返回类型
- effect 操作 variant 的 payload = Perform 的参数类型

```rust
StepLayout {
    step_ty: TypeLayoutId,
    complete_variant: StepVariantLayout,
    effect_variants: Vec<StepVariantLayout>,
}
StepVariantLayout {
    name: String,
    tag_value: u64,
    payload: Option<TypeLayoutId>,
}
```

**6c. Continuation 对象布局**
- 从 MIR 的合成 `<fqn>$continuation` struct 定义完整布局

```rust
ContinuationLayout {
    cont_fqn: String,
    fields: Vec<ContinuationField>,
}
ContinuationField {
    name: String,
    offset: u64,
    ty: TypeLayoutId,
    kind: ContinuationFieldKind,  // Header | ResumedFlag | ResumeStateTag | FramePtr | StepFnPtr | ...
}
```

**6d. State dispatch 信息收集**
- 从 EffectStep 函数体中提取 state dispatch 入口信息
- 记录每个 resume state 的编号 + 对应的 block id
- 供 codegen 生成分发代码（jump table 或 CondBr 链）

**验证**：为 `effect_handle.scoop` 的 EffectStep 函数生成 frame/step/continuation 布局。

### 步骤 7：全局初始化规划

**文件**：`src/global_init/`

**7a. 顶层 val/var 初始化**
- 收集所有 `Item::Initializer(InitializerRoot)`
- 按依赖关系排序（如果 val A 依赖 val B，B 先初始化）
- 产出初始化执行计划

```rust
GlobalInitPlan {
    entries: Vec<GlobalInitEntry>,
}
GlobalInitEntry {
    fqn: String,
    ty: TypeLayoutId,
    is_var: bool,
    init_callable: String,  // 初始化函数符号名
}
```

**7b. 类初始化**
- 从 `BackendContracts.class_inits` + HIR 类型声明构建类初始化计划
- 属性赋值顺序 + init block + 超类委托链

```rust
ClassInitPlan {
    class_fqn: String,
    field_inits: Vec<FieldInit>,
    init_blocks: Vec<InitBlock>,
    super_init: Option<String>,  // 超类初始化函数
}
FieldInit {
    field_name: String,
    ty: TypeLayoutId,
    init_kind: InitKind,  // DefaultValue | PropertyParam | PropertyInitializer
}
InitBlock {
    body_callable: String,  // init block 的函数符号
}
```

**验证**：为含顶层 val 的程序生成初始化计划。

### 步骤 8：MIR→LIR body 映射

**文件**：`src/program.rs`（或 `src/body.rs`）

将 MIR 的 `Body` 映射为 LIR 的 `LirBody`：
- 每个 `LocalDecl` → `LirLocalDecl`（附加 GC traceable 标志）
- 每个 `BasicBlock` → `LirBlock`（1:1 映射语句和终结符）
- 每个 `Rvalue` → `LirRvalue`（附加布局 ID）
- 每个 `TerminatorKind` → `LirTerminator`（1:1 映射）
- 调用点附加 vtable/itable slot offset 信息
- StoreMember 附加字段偏移信息
- Cast/TypeTest 附加类型描述符信息

这一步是机械的 1:1 映射，附加 LIR pass 计算出的布局/槽位信息。

**验证**：MIR body 正确映射到 LIR body，所有 TypeId 替换为 TypeLayoutId。

### 步骤 9：LIR 验证

**文件**：`src/verify.rs`

验证 LIR 的结构完整性：
- 所有 TypeLayoutId 在 LayoutTable 中存在
- 所有 vtable/itable slot 引用有效
- 所有符号名唯一
- EffectStep 函数有完整的 frame/step/continuation 布局
- GC root map 覆盖所有 safepoint
- 无 MIR 残留类型（TypeId 已全部替换为 TypeLayoutId）

### 步骤 10：主 pass 编排 + 接入管线

**文件**：`src/lib.rs`

```rust
pub fn lower_to_lir(
    mir: &MaterializedMir,
    hir: &TypedHir,
    interner: &Interner,
) -> LirProgram {
    let mut program = LirProgram::new();
    // 1. 类型布局计算
    compute_type_layouts(&mut program, mir, hir);
    // 2. 分发表生成
    generate_dispatch_tables(&mut program, mir, hir);
    // 3. ABI 决策
    decide_abi(&mut program, mir, hir);
    // 4. GC 信息生成
    generate_gc_info(&mut program, mir, hir);
    // 5. Effect step 准备
    prepare_effect_steps(&mut program, mir, hir);
    // 6. 全局初始化规划
    plan_global_init(&mut program, mir, hir);
    // 7. MIR→LIR body 映射
    map_bodies(&mut program, mir);
    // 8. 验证
    verify_lir(&program);
    program
}
```

接入 scoop2c 管线（在 materialize 之后）。

### 步骤 11：调试输出 + fixture

**文件**：`src/dump.rs`

实现 LIR 的稳定文本输出格式，用于调试和 golden test。

新增 fixture：
- `tests/fixtures/lir/basic_layout.scoop` — 简单类型的布局验证
- `tests/fixtures/lir/dispatch.scoop` — vtable/itable 分发表验证
- `tests/fixtures/lir/effect_step.scoop` — EffectStep 布局验证
- `tests/fixtures/lir/gc_roots.scoop` — GC root map 验证

---

## 3. 实现顺序与依赖

```
步骤 1（Crate 骨架）           ← 无依赖
    ↓
步骤 2（类型布局计算）          ← 依赖步骤 1
    ↓
步骤 3（分发表生成）            ← 依赖步骤 1、2（需要布局信息判断引用类型）
步骤 4（ABI 决策）             ← 依赖步骤 1、2
步骤 5（GC 信息生成）          ← 依赖步骤 1、2
步骤 6（Effect step 准备）     ← 依赖步骤 1、2
步骤 7（全局初始化规划）        ← 依赖步骤 1、2
    ↓
步骤 8（MIR→LIR body 映射）    ← 依赖步骤 2-7（需要所有 pass 的产出）
    ↓
步骤 9（LIR 验证）             ← 依赖步骤 8
步骤 10（主 pass 编排）        ← 依赖步骤 1-9
步骤 11（调试输出 + fixture）  ← 依赖步骤 10
```

## 4. 风险与注意事项

1. **类型布局的平台无关性**：LIR 的布局计算应使用抽象的 size/align（不依赖具体平台），由 codegen 在最终阶段做平台相关的调整（如 LLVM 的 DataLayout）。但基本的对齐规则（如指针 8 字节对齐）需要在 LIR 确定。

2. **enum niche 优化**：Option<T> 的 niche 存储分析需要判断 T 是否有"不可能的值"（如指针的 null）。这需要类型系统知识（哪些类型是引用类型）。

3. **vtable 继承**：子类的 vtable 需要包含超类的所有虚方法 slot。slot 分配顺序需要与超类一致（前 N 个 slot = 超类的 slot），追加自己的新方法。

4. **GC register promotion 与后端多样性**：codegen 优化器可能把 GC-managed 引用 local 提升到寄存器，导致 root 不一致。**解决方案**：LIR 只产出语义标记（哪些 local 是 GC-managed、哪些点是 safepoint、每个 safepoint 哪些 GC local 是 live 的），由各 codegen 后端根据自身能力选择实现策略（explicit root frame / statepoint / register root / 引擎管理）。当未来运行时加上 register root 支持时，无需修改 LIR——只需对应 codegen 后端切换到 register root 策略。

5. **EffectStep frame 的 GC 信息**：frame tuple 中的 live locals 可能包含 GC 指针。frame schema 需要标记哪些 slot 是 GC-traceable 的，供 codegen 生成 safepoint root map。

6. **闭包的逃逸分析**：当前 LIR 不做逃逸分析（所有闭包 env 堆分配）。后续可添加逃逸分析 pass，让栈上分配的闭包 env 不需要 GC root。

7. **多后端兼容**：LIR 的布局计算应足够通用，使 LLVM/C/WASM codegen 都能消费。C codegen 可能需要额外的约束（如不支持 tagged union，需要展开为 struct + enum）。

8. **GC root frame 的 slot 复用**：当前设计为每个 GC-managed local 分配一个 frame slot（不做 slot 复用）。后续优化可分析 non-overlapping live ranges，复用 frame slot 减少 frame 大小。
