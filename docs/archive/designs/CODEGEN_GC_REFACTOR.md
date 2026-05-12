# Codegen / GC 重构设计

- **状态**: 设计草案
- **范围**: compiler backend GC contract、runtime root enumeration、explicit root frame / stackmap roots 抽象，以及 C codegen + Boehm GC integration。

本文记录一次面向多 codegen / 多 GC backend 的重构设计。当前默认方向是把 managed roots 的 correctness 从 LLVM stackmap 中解耦出来，先落到 Scoop 自己定义的 explicit root frame（精确 shadow stack）协议上；LLVM statepoint/stackmap 继续保留为未来高性能 root provider 与诊断工具。目标不是永久放弃 stackmap，而是把现有实现中隐含的 GC 假设显式化，使后续可以同时支持：

- LLVM codegen + explicit root frame 精确 roots（默认）；
- LLVM codegen + statepoint/stackmap 精确 roots（可选 / 未来 fast path）；
- 非 LLVM codegen + explicit root frame 精确 roots；
- C codegen + Boehm GC 保守 roots；
- 未来 host-managed / WASM GC 等受限环境。

## 1. 背景

当前 LLVM backend 的 GC 协议围绕 LLVM statepoint/stackmap 建立：

- GC-managed 引用用 `addrspace(1)` 表示；
- managed 函数带 `gc "statepoint-example"`；
- pass pipeline 运行 `rewrite-statepoints-for-gc`；
- ordinary safepoint 前后将 stack-backed GC locals 做 `load -> call -> writeback`，依赖 statepoint rewrite 产生 relocatable roots；
- runtime 通过 stack walking + stackmap locations 枚举可写回的 `void** slot`；
- `@Extern` 边界通过 `scoop_enter_native(root_slots, len)` 暴露 native roots；
- hidden sret、deferred aggregate/value、effect frame、global roots 等路径各自维护额外 root slot。

这套协议能服务 LLVM + moving/precise GC，但它把 correctness 绑定到 LLVM 后端输出的 stackmap location 形态上。例如当前实现假设 roots 是 stackmap locations 的 SP/FP 连续后缀；复杂函数在 AArch64 上可能出现 `Indirect x19 + offset` 一类合法机器位置，使 runtime 无法按现有契约枚举 root slots。更根本的问题是，这套协议把以下概念混在一起：

- 哪些值是 GC ref；
- 哪些位置是 collector 可见 root slot；
- safepoint 前后是否需要 relocate / reload / writeback；
- runtime 如何枚举 roots；
- heap 内引用字段如何扫描；
- 写屏障是否需要；
- release hook 何时调用；
- allocation 是否由 Scoop runtime、自研 GC、Boehm 或 host collector 承担。

因此，新增 JavaScript backend、C backend 或 Boehm backend 时，不能只新增一个 `codegen = js/c` 分支。必须先把 GC contract 抽象成可组合的策略，并让所有精确 root discovery 最终都抽象成同一种 collector 输入：可读写的 `void**` root slots。

## 2. 目标与非目标

### 2.1 目标

- 让 codegen 通过统一的 GC policy 查询和发射 GC 相关边界。
- 保持 MIR / middle-end 后端无关，不把 LLVM statepoint、stackmap location、shadow frame offset、Boehm API 等细节写进 MIR 语义。
- 明确支持三类 root discovery：
  - explicit root frame roots（当前默认精确实现）；
  - precise stackmap roots（未来可选 / fast path）；
  - conservative stack/heap scan roots。
- 明确支持 moving 与 non-moving collector 的不同 reload/writeback 需求。
- 让 C codegen + Boehm GC 可以通过 runtime facade 接入，而不是让生成的 C 代码直接散落 `GC_malloc` / `GC_register_finalizer`。
- 将 Scoop 的 `release_fn` 建模为受限 release hook，而不是通常意义上的 finalizer。

### 2.2 非目标

- 本文不要求立即删除 LLVM statepoint/stackmap；但当前默认 correctness 路线改为 explicit root frame，stackmap 作为可选 provider 继续演进。
- 本文不要求一次性实现精确 MIR liveness。迁移初期可以保守 root 所有 in-scope GC locals。
- 本文不承诺 Boehm backend 具备精确对象数量、确定性回收时机或 moving GC 语义。
- 本文不设计完整语言级 release hook surface，只定义 codegen/runtime 需要承接的低层 contract。

## 3. 核心抽象

重构的中心不应是单个 `RootStrategy`，而应是更完整的 `GcBackendPolicy`。Root discovery 只是 GC contract 的一维。

建议抽象维度如下：

```rust
pub enum RootDiscovery {
    ExplicitRootFrame,
    LlvmStatepointStackmap,
    ConservativeStackScan,
    None,
}

pub enum MovementPolicy {
    MovingPrecise,
    NonMoving,
}

pub enum HeapScanPolicy {
    PreciseTypeDescriptors,
    ConservativeHeapScan,
    HostManaged,
}

pub enum WriteBarrierPolicy {
    None,
    RuntimeHook,
    GenerationalCardMark,
}

pub enum AllocationPolicy {
    ScoopRuntimeTypedAlloc,
    BoehmTypedFacade,
    HostManaged,
}

pub enum ReleaseHookPolicy {
    Unsupported,
    SynchronousSweepHook,
    CollectorManagedHook,
    DeferredReleaseQueue,
}

pub enum ThreadGcPolicy {
    RuntimeCooperativeStw,
    CollectorManagedThreads,
    SingleThreadOnly,
}
```

再组合成：

```rust
pub struct GcBackendPolicy {
    pub roots: RootDiscovery,
    pub movement: MovementPolicy,
    pub heap_scan: HeapScanPolicy,
    pub write_barrier: WriteBarrierPolicy,
    pub allocation: AllocationPolicy,
    pub release_hook: ReleaseHookPolicy,
    pub threads: ThreadGcPolicy,
}
```

典型组合：

| Backend | roots | movement | heap scan | barrier | allocation | release hook |
| --- | --- | --- | --- | --- | --- | --- |
| LLVM + baseline/Immix（默认） | `ExplicitRootFrame` | `MovingPrecise` 或 `NonMoving` | `PreciseTypeDescriptors` | `RuntimeHook` | `ScoopRuntimeTypedAlloc` | `SynchronousSweepHook` |
| LLVM + stackmap fast path（未来） | `LlvmStatepointStackmap` | `MovingPrecise` | `PreciseTypeDescriptors` | `RuntimeHook` | `ScoopRuntimeTypedAlloc` | `SynchronousSweepHook` |
| C/JS/其它非 LLVM 精确后端 | `ExplicitRootFrame` | collector 决定 | `PreciseTypeDescriptors` 或 host scan | policy 决定 | runtime/host facade | policy 决定 |
| C + Boehm | `ConservativeStackScan` | `NonMoving` | `ConservativeHeapScan` | `None` | `BoehmTypedFacade` | `CollectorManagedHook` 或 `DeferredReleaseQueue` |

### 3.1 Root slot provider 抽象

精确 GC 不应直接依赖“root 来自 shadow stack 还是 stackmap”。runtime 侧应把它们统一成一个抽象：**root slot provider**。provider 的唯一职责是向 collector 枚举可读写的 `void**` slots：

```c
typedef void (*ScoopGcRootSlotVisitor)(void **slot, void *ctx);
typedef uint64_t (*ScoopGcRootProviderVisitFn)(ScoopGcRootSlotVisitor visitor, void *ctx);
```

约束：

- `slot` 指向一个 pointer-or-null leaf storage；
- `*slot == NULL` 表示当前无 root；
- `*slot != NULL` 时必须是 GC object base pointer（对象头 / runtime 认可的对象起始地址），不能是 interior pointer；
- moving GC 可以原地写回 `*slot = forwarded_object`；
- provider 不负责 heap 内字段扫描，heap 内引用仍由 type descriptor / trace bitmap / trace_fn 处理。

在该抽象下：

- `ExplicitRootFrame` provider 从每个线程 TLS 上的 root-frame 链枚举 `void**` slots；
- `LlvmStatepointStackmap` provider 通过 unwind + stackmap 把 machine locations 转换成同样的 `void**` slots；
- `native_roots` provider 继续枚举 `enter_native(root_slots, len)` 暴露的 `void**` slots；
- handles/pinned/globals 也应最终适配成 root slot 或 root object provider。

这使未来完善 stackmap 时无需推翻 explicit root frame 路线。stackmap fast path 只需要替换或补充 provider，collector 的 mark/update/verify 逻辑仍消费同一种 root slot 流。

### 3.2 Explicit root frame 作为当前默认

Explicit root frame 是 Scoop 自己定义的精确 shadow-stack 形式。它不保存 root value 本身，而是保存 root slot 指针列表：

```c
typedef struct ScoopGcRootFrame {
  struct ScoopGcRootFrame *prev;
  void ***slots;
  uint32_t len;
} ScoopGcRootFrame;
```

其中 `slots[i]` 的类型语义是 `void**`：它指向一个真实 GC ref storage。root frame、slots 数组都由 generated code 放在当前 native stack frame 上，runtime 只在 push/pop 之间借用，不做 heap allocation。

典型生成形态：

```c
void *root_slot = make_node();
void *left_slot = make_node();

void **slots[] = { &root_slot, &left_slot };
ScoopGcRootFrame frame;
scoop_gc_root_frame_push(&frame, slots, 2);
scoop_gc_collect();
scoop_gc_root_frame_pop(&frame);

/* GC 可能已经更新 root_slot / left_slot；后续必须从 slot 重新读取。 */
set_left(root_slot, left_slot);
```

关键 invariant：

- 所有可能触发 GC 的 safepoint 前，跨 safepoint live 的 GC refs 必须写入 canonical root slots；
- root frame 发布的是这些 slots 的地址；
- safepoint 返回后，后续使用必须从 canonical slots reload，不能继续使用 safepoint 前的 SSA/register pointer；
- push 必须在 slots 初始化完成后发布到 TLS；pop 必须严格 LIFO，并在 frame/slots 生命周期结束前执行；
- 若采用 function-scope frame，离开作用域或 variant 切换时应把 dead/inactive ref slots 清为 NULL，避免无界 false retention。

Scoop 的 enum / value layout 已经把 ref payload 与 scalar payload 分离。例如：

```scoop
enum E {
  Integer(Int),
  String(String)
}
```

其布局应类似 `tag + int_payload + ref_payload`。`ref_payload` 是 dedicated pointer-or-null slot：`Integer` variant 下为 NULL，`String` variant 下为 String object pointer。因此 root frame 可以无条件登记 `&e.ref_payload`，不需要 tag-aware scanning，也不会把 int payload 当作 ref 扫描。

### 3.3 Stackmap provider 的长期目标

LLVM stackmap provider 仍然有价值：它能减少 mutator push/pop 开销，并复用 LLVM statepoint 的 relocate 信息。但它必须满足与 explicit root frame 相同的 root slot provider contract。长期工作不是让 collector 依赖“SP/FP 连续后缀”这样的临时规则，而是让 stackmap provider 能可靠地产生 `void**` slots：

- 解析 statepoint record 中真实 GC live locations，而不是仅靠 location 形状和后缀顺序猜测；
- 支持 `Direct/Indirect` 的 platform register base，例如 AArch64 `x19 + offset`，将其转换为内存 slot 地址；
- 对 true `Register` roots 明确 fail-fast 或实现可修改线程上下文的 register update；
- 明确 base/derived pointer 策略：当前默认要求 root slot 中只保存 object base pointer；
- stackmap verifier 应按 provider contract 验证每个 safepoint record 是否可枚举、可更新，而不是验证旧的 SP/FP 后缀契约。

## 4. 编译器侧设计

### 4.1 GC type facts

当前 LLVM codegen 通过 LLVM `BasicTypeEnum` 递归判断 aggregate 内哪些 leaf 是 GC pointer。这个逻辑不能被 C/JS backend 复用。

需要新增 backend-agnostic 的 GC layout facts：

```rust
pub struct GcLeafPath {
    pub steps: Vec<GcFieldStep>,
    pub leaf_ty: TypeId,
}

pub enum GcFieldStep {
    StructField { index: u32 },
    TupleField { index: u32 },
    ArrayElement { index: u32 },
    EnumRefSlot { index: u32 },
}

pub fn gc_leaf_paths(ty: TypeId, cx: &TypeLayoutCx) -> Vec<GcLeafPath>;
```

要求：

- 输入是 `TypeId`、layout facts、nominal metadata，而不是 LLVM IR type。
- 输出只表达“语言值的哪些 leaf 是 GC ref”，不表达 GEP、shadow frame slot、stackmap location。
- LLVM backend 将 `GcLeafPath` 降成 GEP / alloca leaf slots。
- C backend 将 `GcLeafPath` 降成 C field address / local anchor。
- JS backend 将 `GcLeafPath` 降成对象字段 / closure env slot。
- enum / value type 的 mixed scalar/ref payload 必须通过 layout 层拆成 dedicated ref slots；`GcLeafPath` 只指向 pointer-or-null leaf slot，不指向可能同时承载 int/ref 的 mixed word。

### 4.2 GC pointer representation policy

不同 backend 对 GC pointer 的物理表示要求不同：

```rust
pub enum GcPointerRepresentation {
    LlvmGcAddressSpace,
    NativePointer,
    HostReference,
    OpaqueHandle,
}
```

LLVM statepoint backend 继续用 `addrspace(1)`。C + Boehm backend 必须尽量使用真实 C pointer 形态，例如 `void*` / `ScoopObject*` / `ScoopString*`，避免长期把 GC refs 存进 `uintptr_t` / `uint64_t`。

规则：

- 对 precise moving backend，引用可以通过 root slot 更新。
- 对 conservative backend，引用必须保持 pointer-shaped，才能被保守扫描看见。
- pointer-to-int cast 是 unsafe 边界；一旦把 GC ref 隐藏进整数，conservative GC 不再有可靠语义。

### 4.3 Function root manager

每个 codegen backend 应通过函数级 root manager 处理 locals、temps、safepoints、native boundaries。

建议接口形状：

```rust
pub trait FunctionGcRootManager {
    type Value;
    type Slot;
    type RootToken;

    fn enter_function(&mut self, spec: FunctionGcSpec);
    fn leave_function(&mut self, exit: FunctionExitKind);

    fn declare_local(&mut self, local: LocalRef, ty: TypeId, slot: Self::Slot);
    fn register_temp_root(&mut self, ty: TypeId, slot: Self::Slot) -> Self::RootToken;
    fn unregister_temp_root(&mut self, token: Self::RootToken);

    fn before_managed_call(&mut self, site: SafepointSite);
    fn after_managed_call(&mut self, site: SafepointSite);

    fn enter_native_call(&mut self, site: NativeCallSite);
    fn leave_native_call(&mut self, site: NativeCallSite);

    fn keep_alive_until(&mut self, value: Self::Value, point: ProgramPoint);
}
```

Backend 降低方式：

- Explicit root frame（当前默认）：
  - `before_managed_call` 收集当前 safepoint live 的 root slots；
  - 将当前 SSA / 临时 GC refs flush 到 canonical slots；
  - 构造 stack 上的 `ScoopGcRootFrame` 与连续 `void** slots[]`，并 push 到 TLS；
  - call 返回后 pop frame；
  - `after_managed_call` 使跨 safepoint live 的 GC refs 从 canonical slots reload，禁止继续使用旧 SSA/register pointer。
- LLVM stackmap（未来 fast path）：
  - `before_managed_call` 做 `load` keepalive，使 statepoint 看到 gc-live roots；
  - `after_managed_call` 将 relocated SSA value write back；
  - `enter_native_call` 构造 `native_roots` slot array；
  - runtime stackmap provider 必须输出与 explicit root frame 相同的 `void**` root slots。
- Function-scope root frame（可选优化）：
  - function prologue push frame；
  - frame 描述该函数中所有可能跨 safepoint live 的 pointer-or-null slots；
  - scope exit / variant switch 清 NULL，避免 dead roots 被长期保活；
  - exit/cleanup pop frame。
- C + Boehm：
  - `before_managed_call` 通常 no-op；
  - `after_managed_call` no-op；
  - `keep_alive_until` 生成 C-level anchor，防止 C compiler 提前消除 source-live refs；
  - 不生成 statepoint、stackmap、explicit root frame。

### 4.4 Safepoint call classification

所有 runtime / user call 必须归类，不允许各模块随手 `build_call` 后再靠局部知识补 roots。

```rust
pub enum CallGcKind {
    ManagedSafepoint,
    AllocationSafepoint,
    LeafRuntime,
    NativeExtern,
    NoReturnTrap,
}
```

约束：

- `scoop_alloc_typed` 是 allocation safepoint。
- 可能触发 GC、park、resume、effect transport 的 runtime call 必须走 managed safepoint。
- `scoop_enter_native` / `scoop_leave_native` 是 leaf runtime call。
- `ExplicitRootFrame` 精确 backend 下，`ManagedSafepoint` / `AllocationSafepoint` 必须触发 root-frame push/pop 或保证等价的 function-scope frame 已覆盖 caller roots。
- Boehm backend 下 managed safepoint 不需要精确 root rewrite，但仍需要 keepalive anchor。

### 4.5 Allocation facade

Codegen 不应直接面向具体 collector。所有 backend 继续通过 runtime facade：

```c
void *scoop_alloc(uint64_t size_bytes);
void *scoop_alloc_typed(const ScoopTypeDescriptor *type_desc, uint64_t size_bytes);
```

不同 runtime backend 在 facade 内部分派：

- 自研 GC：登记对象头、heap list、type descriptor；
- Immix：block allocation / evacuation metadata；
- Boehm：`GC_malloc` / `GC_malloc_atomic` / finalizer registration；
- host-managed：host allocation / reference wrapper。

### 4.6 Write barrier facade

编译器应通过 policy 判断是否发射 write barrier。

```rust
pub enum StoreGcAction {
    DirectStore,
    RuntimeWriteBarrier,
}
```

Boehm backend：

- `WriteBarrierPolicy::None`；
- C codegen 可直接生成 assignment；
- runtime 仍可保留 `scoop_gc_write_barrier(slot, value)` no-op facade，服务 ABI 兼容和混合测试。

Moving/generational backend：

- 继续通过 runtime hook 或更具体 card-mark/write barrier lowering。

### 4.7 MIR liveness

短期可以保守 root 所有 in-scope GC locals。长期应在 MIR/pass 层提供 safepoint root set：

```rust
pub struct SafepointRootSet {
    pub site_id: SafepointId,
    pub live_locals: Vec<LocalId>,
    pub temp_roots: Vec<TempRootId>,
    pub leaf_paths: Vec<(LocalId, GcLeafPath)>,
}
```

规则：

- MIR 只表达 `LocalId`、`TypeId`、safepoint site、liveness；
- 不表达 LLVM statepoint、shadow frame offset、Boehm stack scanning API；
- `RootDiscovery::ConservativeStackScan` 可以选择忽略精确 root set，但仍可用它生成 keepalive anchors。

## 5. 运行时侧设计

### 5.1 Root source registry

runtime 内部应把 root 枚举写成组合式 root sources：

```c
typedef uint64_t (*ScoopRootSourceVisitFn)(ScoopGcTraceVisitor visitor, void *ctx);
```

每个 backend 可以组合：

- stackmap roots；
- native roots；
- shadow stack roots；
- conservative stack roots；
- module-global roots；
- pinned object roots；
- stable handle roots。

当前 baseline/Immix 中散落的 stackmap/native roots 枚举应逐步收口到 shared helper，避免新增 shadow stack / Boehm 时继续复制。

### 5.2 Capability matrix

`runtime/c/scoop_gc_backend.h` 和 `crates/scoop_runtime/src/gc_backend.rs` 需要补充能力位：

```c
#define SCOOP_GC_CAP_STACKMAP_ROOTS 0/1
#define SCOOP_GC_CAP_NATIVE_ROOTS 0/1
#define SCOOP_GC_CAP_SHADOW_STACK_ROOTS 0/1
#define SCOOP_GC_CAP_CONSERVATIVE_STACK_ROOTS 0/1
#define SCOOP_GC_CAP_CONSERVATIVE_HEAP_SCAN 0/1
#define SCOOP_GC_CAP_MOVING 0/1
#define SCOOP_GC_CAP_PRECISE_ROOTS_UPDATE 0/1
#define SCOOP_GC_CAP_WRITE_BARRIER_REQUIRED 0/1
#define SCOOP_GC_CAP_RELEASE_HOOKS 0/1
#define SCOOP_GC_CAP_SYNCHRONOUS_RELEASE_HOOKS 0/1
#define SCOOP_GC_CAP_EXACT_OBJECT_COUNT 0/1
```

Boehm backend 预期：

```c
#define SCOOP_GC_CAP_STACKMAP_ROOTS 0
#define SCOOP_GC_CAP_NATIVE_ROOTS 0
#define SCOOP_GC_CAP_SHADOW_STACK_ROOTS 0
#define SCOOP_GC_CAP_CONSERVATIVE_STACK_ROOTS 1
#define SCOOP_GC_CAP_CONSERVATIVE_HEAP_SCAN 1
#define SCOOP_GC_CAP_MOVING 0
#define SCOOP_GC_CAP_PRECISE_ROOTS_UPDATE 0
#define SCOOP_GC_CAP_WRITE_BARRIER_REQUIRED 0
#define SCOOP_GC_CAP_RELEASE_HOOKS 1
#define SCOOP_GC_CAP_SYNCHRONOUS_RELEASE_HOOKS 0
#define SCOOP_GC_CAP_EXACT_OBJECT_COUNT 0
```

### 5.3 Shadow stack ABI

Shadow stack backend 需要 runtime TLS 支持：

```c
typedef struct ScoopShadowRootFrame {
  struct ScoopShadowRootFrame *prev;
  uint32_t len;
  void ***slots;
} ScoopShadowRootFrame;

void scoop_gc_shadow_frame_push(ScoopShadowRootFrame *frame);
void scoop_gc_shadow_frame_pop(ScoopShadowRootFrame *frame);
```

`slots[i]` 表示可读写的 `void**` root slot 地址。若未来选择 shadow slot 本身作为 authoritative storage，也可以改成 inline `void* values[]`，但必须明确 moving GC 后程序变量从哪里 reload。

### 5.4 Release hook contract

Scoop 的 `release_fn` 不是通常意义上的 finalizer。它是受限 release hook：

- 附着在 `ScoopTypeDescriptor` 上；
- 对象已经不可达、即将回收其存储前调用；
- 至多调用一次；
- 不允许触发 GC；
- 不允许分配 Scoop GC 对象；
- 不允许复活对象；
- 不保证对象之间的调用顺序；
- 只适合释放 native / non-GC resource，例如 fd、mutex、condvar、OS handle、malloc buffer。

未来语言级“类 finalizer”应映射到这个受限 hook，而不是开放普通用户 finalizer。效果系统应提供类似 `NoGcRelease` 的受限 effect，禁止 allocation、GC、suspend、resurrection。

Release hook policy：

```rust
pub enum ReleaseHookPolicy {
    Unsupported,
    SynchronousSweepHook,
    CollectorManagedHook,
    DeferredReleaseQueue,
}
```

当前自研 mark-sweep / Immix 更接近 `SynchronousSweepHook`。Boehm backend 更可能是 `CollectorManagedHook` 或 `DeferredReleaseQueue`。

## 6. C codegen + Boehm GC 对接

### 6.1 Runtime backend

新增：

```text
runtime/c/scoop_gc_backend_boehm.c
```

并在 backend selection 中新增：

```c
#define SCOOP_GC_BACKEND_BOEHM 5
```

build glue 需要支持：

- 检测 Boehm GC header / library；
- 定义 `SCOOP_GC_BACKEND=SCOOP_GC_BACKEND_BOEHM`；
- 链接 `gc` / `gccpp` 或平台对应库；
- 对 pthread-enabled Boehm 使用正确 link flags；
- 没有 libgc 时给出明确诊断或跳过 Boehm-only tests。

### 6.2 Allocation

`scoop_alloc_typed(desc, size)` 在 Boehm backend 内部应：

1. 初始化 Boehm runtime，例如 `GC_INIT()`；
2. 根据 descriptor / layout 判断是否 pointer-free；
3. pointer-free 对象可用 `GC_malloc_atomic(size)`；
4. pointer-bearing 对象用 `GC_malloc(size)`；
5. 写入 `ScoopGcObjectHeader`，包括 `type_desc`、`size_bytes` 等字段；
6. 若 `desc->release_fn != NULL`，注册 release hook；
7. 返回对象 header 起始地址，保持现有 Scoop object layout。

生成的 C 代码不应直接调用 `GC_malloc`。它只调用 `scoop_alloc_typed` facade。

### 6.3 Heap scanning

Boehm backend 可以保守扫描 heap，因此 `ScoopTypeDescriptor.trace_bitmap` / `trace_fn` 不一定参与 tracing。但 descriptor 仍必须保留，因为它还承担：

- `release_fn`；
- runtime type id；
- parent type descriptor；
- vtable / itable；
- allocation size / alignment；
- 未来可选 typed allocation optimization。

逻辑上应把 descriptor 拆成角色：

```text
RuntimeTypeInfo       -> type_id, parent, vtable, itable
TraceDescriptor       -> precise heap scan metadata
ReleaseDescriptor     -> release_fn
AllocationDescriptor  -> size, align, pointer-free marker
```

结构体可以暂时不拆，但 codegen/runtime 文档应按角色描述，避免 Boehm backend 被迫使用所有字段。

### 6.4 Conservative stack roots

C codegen + Boehm 使用 `RootDiscovery::ConservativeStackScan`：

- 不生成 LLVM statepoint；
- 不生成 stackmap；
- 不生成 shadow frame；
- 不调用 `scoop_enter_native(root_slots, len)` 暴露 native roots；
- 保持 GC refs 为 pointer-shaped C locals / fields；
- 对跨 allocation/runtime call 仍 live 的 refs 生成 keepalive anchor。

keepalive 可以先通过 runtime/compiler barrier facade：

```c
void scoop_gc_keep_alive(void *p);
```

其实现可以是 no-op，但必须阻止 C compiler 在调用点前过早消除 `p`。实现可考虑：

```c
void scoop_gc_keep_alive(void *p) {
  asm volatile("" : : "r"(p) : "memory");
}
```

或平台无 inline asm 时使用 `volatile` sink。

### 6.5 Globals

Boehm 通常会扫描静态数据段，但为了 runtime ABI 统一，仍保留：

```c
void scoop_gc_register_global_root(void *base, const ScoopTypeDescriptor *type_desc);
```

Boehm backend 可选择：

- 对普通 C globals 退化为 no-op；
- 对动态生成或不保证被 Boehm 自动扫描的 storage，维护一张 scanned root table；
- 在 mixed/runtime tests 中继续通过该 facade 表达 module-global roots。

### 6.6 Pinning

Boehm 是 non-moving collector，因此 `pin` 不需要阻止移动。但 Scoop `pin` 仍有 root-retention 语义：外部系统持有裸指针时，pin 期间对象必须保持存活。

Boehm backend 下：

- `scoop_pin(obj)` 将对象加入 pin table 并递增计数；
- `scoop_unpin(obj)` 递减计数，计数为 0 时移除；
- pin table 自身必须被 Boehm 扫描或显式注册为 roots；
- pin 不需要参与 relocation。

### 6.7 Stable handles

即使 Boehm 不移动对象，也应保留 stable handle table：

- 保持 `handleNew/get/drop` 语义一致；
- handle table 持有对象引用，使对象在 handle 生命周期内存活；
- handle token 仍是 opaque integer；
- 不需要在 GC 后更新 `handle->obj`。

如果底层表用 `malloc`，Boehm 未必扫描它。实现应使用 Boehm-scanned allocation、uncollectable scanned allocation，或显式 root registration。

### 6.8 Write barrier

Boehm backend 使用 `WriteBarrierPolicy::None`：

- C codegen 可直接发射 assignment；
- `scoop_gc_write_barrier` 保留为 ABI facade，内部可用 `memcpy` / direct store；
- 不需要 old-to-young barrier、card mark 或 returning updated value。

### 6.9 Threading

Boehm backend 使用 `ThreadGcPolicy::CollectorManagedThreads` 或 `SingleThreadOnly`。

若启用 Boehm thread support：

- `scoop_thread_register` / `scoop_thread_unregister` 应映射到底层 Boehm thread registration 机制，或依赖 Boehm pthread wrapper；
- 不使用 Scoop runtime cooperative STW；
- `scoop_gc_safepoint` / `scoop_gc_safepoint_poll` 可以是 no-op 或 keepalive/checkpoint facade。

若只支持 single-thread：

- capability 明确标记；
- 多线程 tests gate 掉或使 collect 退化为 no-op。

### 6.10 Collect / debug APIs

`scoop_gc_collect()` 可映射到 `GC_gcollect()`，但 Boehm 不应承诺：

- collect 后所有不可达对象立即释放；
- release hook 立即执行；
- exact object count；
- exact bytes freed；
- heap reserved bytes 与自研 GC 同语义。

相关 debug APIs 需要 capability gate：

- `scoop_gc_debug_heap_object_count` 可返回 0 / unsupported sentinel / backend-specific approximate；
- fixture 不应在 Boehm backend 下断言精确对象数；
- release callback tests 需要区分 synchronous 与 collector-managed hook。

### 6.11 Release hook on Boehm

Boehm backend 有两种可选实现：

#### 方案 A：直接 collector-managed hook

`scoop_alloc_typed` 中注册 Boehm finalizer。finalizer 内调用 `desc->release_fn(obj)`。

优点：

- 实现简单；
- 对 native resource cleanup 足够直接。

缺点：

- 调用时机、线程、顺序由 Boehm 决定；
- 不适合测试“collect 后立即 release”；
- 必须确保 release_fn 不触发 Scoop GC，不分配，不复活对象。

#### 方案 B：deferred release queue

Boehm finalizer 不直接调用 `release_fn`，只把对象或 release record 入队。Scoop runtime 在显式 safe release drain 点调用 release_fn。

优点：

- 更容易控制 no-GC release 上下文；
- 可减少 collector finalizer 线程直接进入 Scoop runtime 的风险。

缺点：

- 需要额外队列和 drain 协议；
- 仍不能承诺普通语言 finalizer 语义；
- 对资源及时释放仍不如显式 `close/destroy`。

建议初期采用方案 A，但 capability 标记为非同步 release hook；语言级文档不承诺及时性。

## 7. JS / Shadow-stack 对接补充

Shadow stack backend 与 Boehm backend 的关键差异：

- shadow stack 是 precise root discovery；
- 若 collector moving，shadow slot 或 local slot 必须是 authoritative storage；
- GC 后必须 reload / writeback；
- false retention 比 conservative scan 少；
- codegen 成本更高，需要 prologue/epilogue 和 cleanup path。

因此 `RootDiscovery::ShadowStack` 不应复用 Boehm 的 conservative no-op call boundary。两者可以共享 `GcLeafPath`、allocation facade、release hook policy，但 root manager 行为不同。

## 8. 迁移计划

### 阶段 0：文档与 capability 收口

- 更新 runtime capability matrix；
- 修正 minimal/hosted 中“shadow stack roots”历史注释与实际行为不一致的问题；
- 明确 `release_fn` 是 release hook，不是普通 finalizer。

### 阶段 1：抽 LLVM root manager 外壳

- 新增 LLVM 内部 `FunctionGcRootManager`；
- 将当前 `collect_conservative_gc_root_slots`、`with_conservative_gc_local_root_spills`、`extra_gc_root_slots`、sret/deferred root 注册迁入该边界；
- 不改变 LLVM IR 输出。

### 阶段 2：抽 backend-agnostic GC leaf facts

- 新增 `GcLeafPath` / `gc_leaf_paths(TypeId)`；
- LLVM backend 改为将 leaf paths 降成 GEP；
- global roots、hidden sret、deferred values 统一复用该分析。

### 阶段 3：统一 call GC classification

- 所有 runtime/user call 走 `CallGcKind`；
- allocation、managed runtime、native extern、leaf runtime 分开；
- 删除散落的“某个 helper 内自己决定是否 root”的路径。

### 阶段 4：shadow stack runtime source

- TLS/thread record 加 shadow root top；
- runtime 加 push/pop frame；
- root visitor 加 shadow stack source；
- 加 C runtime tests 验证 shadow roots 保活对象。

### 阶段 5：C codegen + Boehm backend

- 新增 `SCOOP_GC_BACKEND_BOEHM`；
- 实现 `scoop_alloc_typed` -> Boehm allocation facade；
- 实现 pin/handle/global root facade；
- 实现 no-op write barrier；
- 实现 collector-managed release hook；
- C codegen 使用 native pointer representation 和 keepalive anchors。

### 阶段 6：MIR safepoint liveness

- 为 materialized MIR/pass view 增加 safepoint root-set side table；
- LLVM/shadow stack backend 从保守 in-scope roots 逐步切到 live roots；
- C/Boehm backend 继续可选择只用 root-set 生成 keepalive anchors。

## 9. 测试计划

### 9.1 编译器测试

- LLVM IR golden：重构后 statepoint/stackmap IR 不变。
- Root manager unit tests：hidden sret、deferred aggregate、effect frame leaf paths 均注册正确。
- C codegen golden：GC refs 保持 pointer-shaped，不被长期编码为 integer。
- C codegen keepalive tests：跨 allocation 后仍 live 的 refs 会生成 keepalive anchor。

### 9.2 运行时测试

- capability matrix 与选择的 backend 一致。
- shadow stack roots 能保活对象。
- Boehm backend 下 pin/handle 能保活对象。
- Boehm backend 下 write barrier facade 可链接且不破坏 store。
- release hook 至多一次调用；同步性断言只在 `SCOOP_GC_CAP_SYNCHRONOUS_RELEASE_HOOKS=1` 时启用。

### 9.3 Fixture 策略

- 精确 object count / bytes freed tests 只在 exact backend 下运行。
- moving/update tests 只在 `MOVING && PRECISE_ROOTS_UPDATE` 下运行。
- Boehm tests 避免依赖立即回收和立即 release。
- C+Boehm run-pass tests 聚焦语义正确性、指针形态、pin/handle、release hook smoke。

## 10. 关键风险

- Conservative GC 下，C compiler 优化可能让 source-live refs 不再出现在 stack 上；必须有 keepalive anchor。
- Conservative GC 可能 false retention，不能用它承诺及时释放资源。
- Boehm finalizer 语义不等于 Scoop release hook 的理想同步语义；必须通过 capability 和文档限制承诺。
- 若 C codegen 把 refs 存进整数或压缩 payload，Boehm 可能漏扫。
- pin/handle table 若用普通 malloc，Boehm 可能不扫描；表本身必须可被扫描或显式 root。
- shadow stack 与 moving GC 组合时，必须明确 authoritative slot，避免 GC 后继续使用 stale local。
- effect/continuation runtime function 的 child-codegen 上下文切换必须同时切换 root manager 状态。

## 11. 未决问题

- Boehm release hook 初期采用直接 finalizer 还是 deferred queue？
- `scoop_gc_keep_alive` 是否作为公开 runtime ABI，还是仅作为 C codegen 内部 helper？
- C codegen 是否需要禁用某些优化，或用编译器属性保证 GC refs 不被隐藏？
- `ScoopTypeDescriptor` 是否需要显式 pointer-free flag，以便 Boehm 选择 `GC_malloc_atomic`？
- 对 dynamically loaded modules，Boehm 是否能自动扫描其 globals，还是必须通过 `scoop_gc_register_global_root` 注册？
- 语言级 release hook 的 effect 名称与静态检查规则如何设计？
