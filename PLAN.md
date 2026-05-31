# Scoop：`@ReleaseHook` 资源兜底回收 + `@NoGC` 强制 Pure 落地计划

> 生成时间：2026-05-30
> 设计基线：本文件（首版设计直接内联，无独立 design 文档；如后续 contract 变化需回写本文件与 spec）
> 格式参考：[`docs/archive/plans/PLAN-managed-abi.md`](./docs/archive/plans/PLAN-managed-abi.md)、[`docs/archive/plans/PLAN-gc-pacing-immortal.md`](./docs/archive/plans/PLAN-gc-pacing-immortal.md)
> 行号说明：下文行号以当前文件状态为准；后续若漂移，优先按文件路径、函数名、fixture 名定位。

## 0. 目标与定位

新增一个 `@ReleaseHook` annotation，给 **non-generic、final** 的 class 设置 type descriptor 上的 `release_fn`，使 GC 回收该对象时调用用户指定的资源释放函数（典型用途：释放 native handle，如 mutex / condvar / fd）。

明确的语义边界：

- 这是 **比 GC finalizer 更弱的、尽力而为（best-effort）的系统资源兜底回收**，不是确定性析构。
- 只覆盖「对象在运行期被 GC 回收」这条路径；**进程退出时仍存活的对象不会被回收**（OS 负责回收这类系统资源，符合预期）。
- 直接目的：让 `Mutex` / `CondVar` 这类类型不再依赖 compiler intrinsic，纯粹由普通 Scoop class + 内部 FFI（创建/销毁资源）实现。

本计划分阶段推进，**P0 是一个独立的正确性前置修复**，`@ReleaseHook` 的安全性依赖它：

- **P0**：修复 `@NoGC` 不强制 Pure 的缺陷（让 `@NoGC` 函数与 `@Extern` 一样禁止 effect row）。
- **P1-P3**：`@ReleaseHook` 注解的 surface、校验、trampoline codegen 与验证收尾。
- **P4**：用新机制改造 `scoop.sync` cone —— 把 `Mutex` / `CondVar` / `Once` 降为带 `@ReleaseHook` 的普通 class，删除 `Once.run` 的 `@Intrinsic`，并清光编译器内关于这三个类型的全部硬编码（机制的首个真实 first-user）。
- **P5**：把 `lazy` / `observable` / `vetoable` 属性委托从编译器合成降为普通库 class（实现已有的 `ReadOnlyProperty`/`ReadWriteProperty` 协议），删除三者的 by-name 注入与 per-property `Mutex` 合成，使属性委托收敛成唯一的泛型 lowering 路径。

## 1. 关键现状（已核实）

基础设施基本就绪，本计划主要是「把已有但对用户关闭的能力，通过注解安全地开放」：

- **`release_fn` 字段已存在**：`runtime/c/include/scoop_runtime.h:24`（`typedef void (*ScoopTypeReleaseFn)(void *object);`）、字段位于 `ScoopTypeDescriptor` 第 36 行（14 字段布局中的 `release_fn`）。
- **GC 已经会调用 `release_fn`**：仅对**不可达（死）对象**、在 sweep 阶段、**free 之前**调用；运行在 GC 锁 + stop-the-world 上下文、调用线程即发起线程（非独立线程），不会 re-enter GC。
  - baseline：`runtime/c/scoop_gc.c:1938-1939`（moving sweep）、`scoop_gc.c:2462-2463`（non-moving sweep）；
  - immix：`runtime/c/scoop_gc_backend_immix.c:5789-5790`（major reclaim）、minor nursery reclaim 约 `:4878`；
  - minimal：`runtime/c/scoop_gc_backend_minimal.c:664-666`；hosted：`runtime/c/scoop_gc_backend_hosted.c:675-676`。
  - **退出/teardown 路径不调用 `release_fn`**（无 `atexit` / destructor），与「尽力而为」定位一致。
- **用户类的 `release_fn` 当前恒为 null**：`crates/scoopc_codegen_llvm/src/llvm/codegen/gc.rs:1080`（`get_or_create_type_descriptor_global` 内，14 元素 values 数组第 9 项写死 `i8_ptr_ty.const_null()`）。Array/MutableArray 已在 runtime 侧手动填这个字段，证明机制可用。
- **对象指针布局**：`release_fn` 收到的 `void *object` 指向 **header**；用户字段在 `{ ScoopGcObjectHeader, payload }` 布局里、payload 从 LLVM 结构 index 1 开始（`gc.rs:1157`，header 类型见 `gc.rs:827`）。trampoline 取字段偏移须基于完整对象布局用 `target_data.offset_of_element`，不能只按 payload 算。
- **语言无 unwind**：不存在异常 unwind 进 C sweep 循环的风险，这一类安全问题不需要处理。
- **`@NoGC` 当前不强制 Pure（缺陷）**：`@NoGC` 仅在调用点 gate（`crates/scoopc_hir/src/typecheck/expr/call/gates.rs:214-240`）保守拒绝「可能分配」的调用，**未对函数自身 effect row 施加任何约束**；对照 `@Extern` 有 `check_extern_fun_effect_contract`（`crates/scoopc_hir/src/typecheck/annotations.rs:2509-2525`）强制 Pure。这是必须补上的语义。

## 2. 安全性论证（补完 P0 之后）

`@ReleaseHook` 的释放函数从 C sweep 循环里被调用，健全性依赖以下闭环：

- 释放函数限定为 `@NoGC` 或 `@Extern(abi = "c")`：
  - `@Extern(abi = "c")` 不在 Scoop effect 系统内，且已被 `check_extern_fun_effect_contract` 强制 Pure；
  - `@NoGC`（P0 修复后）禁止分配（已有 call gate）**且**强制 Pure（P0 新增）。
- args 只允许传 **GC-free** 字段值（不传 `self`、不传任何 GC 引用）：sweep 中途其它对象可能已被回收，只传裸值（标量 / `Ptr<T>` 等）从根本上避开「访问已死对象 / 对象复活」陷阱。`is_gc_free_value_type`（`crates/scoopc_hir/src/typecheck/lower.rs:3168`）已把 `Ref`（:3191）与 type param（:3194）判为非 GC-free，`Ptr<T>`（pointee GC-free 时）判为 GC-free，正好匹配 native handle 用例。
- 综合：无分配、无 effect、无 unwind、不 re-enter GC ⇒ 从 sweep 循环调用是健全的，全部由类型系统在编译期兜底，`@ReleaseHook` 自身**无需**再独立检查 effect。

## 3. 代码入口总表

| 主题 | 入口文件 / 位置 | 当前状态 | 目标动作 |
|---|---|---|---|
| `@NoGC` effect 契约（P0） | `crates/scoopc_hir/src/typecheck/annotations.rs`（`check_extern_fun_effect_contract:2509`、`check_builtin_annotations_on_fun_decl:2301`、`AnnotationError` 变体约 :376） | `@NoGC` 不约束 effect | 新增 `check_nogc_fun_effect_contract`，镜像 extern 版：禁 `eff_param`、要求 `effects.terms` 为空；在 fun decl 检查里对 `@NoGC` 调用 |
| 注解种类与识别 | `crates/scoopc_hir/src/typecheck/builtin_annotations.rs`（`BuiltinAnnotationKind:19-47`、`builtin_annotation_kind:~104`、`parse_experimental_annotation:~194`） | 无 `ReleaseHook` | 新增 `ReleaseHook` 变体 + 识别 `["ReleaseHook"]` / `["scoop","core","ReleaseHook"]`；新增 `parse_release_hook_annotation`（解析 `name` 字符串与 `args` 字符串数组） |
| type decl 注解校验 | `crates/scoopc_hir/src/typecheck/annotations.rs`（`check_builtin_annotations_on_type_decl`） | 无 | 校验：仅 class、non-generic、final（无 `Open`/`Abstract`/`Sealed`）、必须同时带 `@Experimental(feature = "releaseHook")`；解析并校验 `name`/`args`（见 §5 P1） |
| 泛型 / final 判定 | `crates/scoopc_ast/src/ast/mod.rs`（`TypeDecl.type_params:~825`、`Modifier::{Open,Abstract,Sealed}:535-537`） | — | non-generic = `type_params.is_empty()`；final = 不含 `Open`/`Abstract`/`Sealed` |
| GC-free 字段判定 | `crates/scoopc_hir/src/typecheck/lower.rs`（`is_gc_free_value_type:3168`） | 已有 | 复用：校验每个 arg 字段类型 GC-free |
| 释放函数符号解析 | `crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs`（`lir_callable_symbol_facts:267`）、`gc.rs:6-52`（`declare_dispatch_target_fun`） | 已有 | trampoline 内引用 FQN → 解析到 native/extern symbol；确保该函数不被 DCE |
| 字段偏移 / GEP | `crates/scoopc_codegen_llvm/src/llvm/codegen/layout.rs`（`lookup_struct_field:192`、`codegen_class_field_ptr:287`）、`gc.rs`（`offset_of_element` 用法 :920） | 已有 | trampoline 按字段名取 index/offset，从 header 基址 GEP 读值 |
| type descriptor 发射 | `crates/scoopc_codegen_llvm/src/llvm/codegen/gc.rs`（`get_or_create_type_descriptor_global:1023`、release_fn 槽 :1080） | release_fn 恒 null | 当类型带 `@ReleaseHook` 时，填入 trampoline 指针而非 null |
| 运行期约定文档 | `runtime/c/include/scoop_runtime.h:38-43`、`SCOOP_RUNTIME.md`、`SCOOP_FULL_SPEC.md`（release callback 约 :2959-2972；`@NoGC` 约 :2646-2700） | 仅注释级约定 | 把 `@ReleaseHook` 用户可见 contract 与 `@NoGC` Pure 语义写进 spec |

### 3.1 `scoop.sync` 迁移与硬编码清理入口（P4）

| 主题 | 入口文件 / 位置 | 当前状态 | 目标动作 |
|---|---|---|---|
| sync cone 源 | `sysroot/lib/scoop.sync/src/sync.scoop`（`Mutex:21`/`CondVar:24`/`Once:27`；op `@Extern(abi="scoop"):32-101,110-119`；`__scoop_sync_once_run` `@Intrinsic:130-131`） | opaque class + scoop-ABI extern + 1 个 intrinsic | 改为 final class + `Ptr<T>` handle + `@ReleaseHook`；op 改 `@Extern(abi="c")`；`Once.run` 纯 Scoop 重写 |
| sync C runtime | `sysroot/lib/scoop.sync/native/scoop_sync.c`（create/op/destroy；C 侧 type desc + `*_release` `:209-224/334-349/496-511`；handle 布局 `:173-182`） | C 分配 GC 对象 + C 侧 descriptor | 收缩为只管 raw native handle 的 create/destroy/op |
| `Once.run` codegen 特判 | `runtime_symbols.rs:26`、`runtime_abi.rs:266-283`、`call/lowering.rs:1558`、`intrinsics/sync.rs:6-92`、`effect_lowered/value.rs:1794,1925-1997`、`closure/mod.rs:691` | 硬编码 FQN dispatch + 专用 handler | 全部删除 |
| sync 无 effect 白名单 | `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:2857-2867` | 11 个 sync FQN 写死为无 effect | 删除，由 class 化后的常规 effect 推导承接 |
| sync cone 特殊处理 | `session/mod.rs:242,269`、`scoop_project_model/graph_loader.rs:668,682,710,767`、`pipeline/llvm_codegen_stage.rs:617` | `scoop.sync` 被特判 | 按需收敛/删除 |
| lazy/delegate 属性同步引用 Mutex（消费侧） | `impl_lowering.rs:33-36`（FQN 常量）、`decls.rs:965,982,1008,1025`、`sugar.rs:326,336,656,666,778,788,914,926` | 编译器硬编码 Mutex FQN 注入加锁 | P4-T04 决策：经普通解析引用 或 下放 sysroot helper |

## 4. 顺序总览

1. **P0**：`@NoGC` 强制 Pure（独立正确性修复，先行、单独可测）。
2. **P1**：`@ReleaseHook` 注解 surface + front-end/HIR 全套校验（不含 codegen）。
3. **P2**：trampoline codegen + 在 `gc.rs:1080` 填 `release_fn`。
4. **P3**：验证矩阵、跨后端/跨平台回归、文档与 spec 回写。
5. **P4**：用新机制改造 `scoop.sync` cone —— 把 `Mutex` / `CondVar` / `Once` 降为带 `@ReleaseHook` 的普通 class，删除 `Once.run` 的 `@Intrinsic` 实现，并清光编译器里关于这三个类型的全部硬编码。
6. **P5**：把 `lazy` / `observable` / `vetoable` 降为普通库 class，删除属性委托的全部 by-name 特判，只留泛型委托 lowering。

依赖说明：

- P0 必须早于 P1：`@ReleaseHook` 校验里「释放函数是 `@NoGC` 或 native `@Extern`」这条的安全性，依赖 `@NoGC` 已被保证 Pure。若 P0 未做，P1 要么不安全、要么被迫在 `@ReleaseHook` 里重复实现 effect 检查。
- P1 必须早于 P2：codegen 只在校验通过、注解语义确定后才发 trampoline。
- P3 收尾之前不算完成：单后端通过不代表四个 GC 后端 + 跨平台都正确。
- P4 必须晚于 P3：`scoop.sync` 改造是 `@ReleaseHook` 的首个真实 first-user，必须建立在机制已验证、四后端/跨平台已绿之上；否则会把机制 bug 与迁移 bug 混在一起难以定位。
- P5 必须晚于 P4：线程安全模式的 `lazy`/`observable`/`vetoable` 委托类内部要组合**库** `Mutex`/`Once`，这要求 `scoop.sync` 已先 class 化。P5 同时让 P4-T04 的「lazy/delegate 消费侧 Mutex 引用」决策点彻底消失（宿主类不再被注入 Mutex）。
- P5-T01 必须晚于 P5-T00/P5-T00R：库委托类按目标形态需要 `class Lazy<V> : ReadOnlyProperty<Any, V>` / `class ObservableProperty<V> : ReadWriteProperty<Any, V>`；当前 runtime itable metadata 对泛型 class 实现参数化 interface 的 stable type id 计算必须先修复并复核。

## 5. 分阶段计划

### P0. `@NoGC` 强制 Pure

目标：让 `abi = "scoop"` 的 `@NoGC` 函数与 `@Extern` 一样，禁止携带 effect（禁 `eff_param`、要求 `effects.terms` 为空）。

- P0-T01：在 `crates/scoopc_hir/src/typecheck/annotations.rs` 新增 `check_nogc_fun_effect_contract(fun: &ast::FunDecl)`，结构镜像 `check_extern_fun_effect_contract`（:2509-2525）；新增对应 `AnnotationError` 变体（参考 `ExternFunEffParamNotAllowed` / `ExternFunEffectsNotAllowed` 约 :376）。
- P0-T02：在 `check_builtin_annotations_on_fun_decl`（:2301）里，当函数带 `@NoGC` 时调用该检查。注意 `@Extern(abi="c")` 隐含 `@NoGC`：避免对 extern 函数重复报错或冲突诊断，复用/协调已有 extern 路径。
- P0-T03：fixtures
  - 新增 `tests/fixtures/typecheck/nogc_fun_with_effect_is_error.scoop`（带 effect row 的 `@NoGC` 函数应报错）；
  - 新增 `nogc_fun_with_eff_param_is_error.scoop`；
  - 正例：`nogc_fun_pure_ok.scoop`；
  - 回归既有 `@NoGC` 用例，确认无误伤（现存 `@NoGC` 函数若本就 Pure 不受影响）。
- P0-T04：spec 回写 `SCOOP_FULL_SPEC.md` §`@NoGC`（约 :2646-2700）——明确 `@NoGC` 蕴含 Pure。

验收：`@NoGC` 函数若声明 effect 一律编译期拒绝；全量 typecheck fixture 绿。

### P1. `@ReleaseHook` 注解 surface 与校验（front-end / HIR）

注解形态：`@ReleaseHook(name = "releaseFunctionFQN", args = ["field1", "field2", ...])`。

- P1-T01：`builtin_annotations.rs`
  - `BuiltinAnnotationKind` 新增 `ReleaseHook`（:19-47）；
  - `builtin_annotation_kind` 识别 `["ReleaseHook"]` / `["scoop","core","ReleaseHook"]`（:~104）；
  - 新增 `parse_release_hook_annotation`：解析 `name`（字符串字面量，FQN）与 `args`（字符串数组），结构参考 `parse_experimental_annotation`（:~194）。
- P1-T02：宿主校验（`check_builtin_annotations_on_type_decl`）
  - 仅允许在 **class** 上（非 struct/enum/interface/annotation）；
  - **non-generic**：`type_params.is_empty()`；
  - **final**：modifiers 不含 `Open` / `Abstract` / `Sealed`；
  - **必须同时带** `@Experimental(feature = "releaseHook")`；
  - 每条违例给出独立、清晰的诊断。
- P1-T03：`name` 释放函数校验
  - FQN 可解析、可访问；
  - 必须是 `@NoGC` 或 `@Extern(abi = "c")`；
  - 签名必须是 `void f(FieldType1, FieldType2, ...)`（返回 Unit；参数个数/顺序与 `args` 对应）。
- P1-T04：`args` 字段校验
  - 每个名字必须是该 class 的字段；
  - 每个字段类型必须 **GC-free**（复用 `is_gc_free_value_type`，`lower.rs:3168`）；
  - 字段类型与释放函数对应参数类型**精确匹配**（顺序按 `args` 列出的顺序）。
- P1-T05：把校验结果（目标函数 FQN + 字段名有序列表）存入 HIR side table，供 codegen 阶段消费（命名/落点参考现有 annotation→codegen 传递机制）。
- P1-T06：typecheck fixtures（全部 error 用例各一，正例一个）
  - generic class、open/abstract/sealed class、缺 `@Experimental`、非 class 宿主；
  - 释放函数不存在 / 不可访问 / 非 `@NoGC` 且非 `@Extern(c)` / 返回非 Unit / 参数个数或类型不匹配；
  - args 字段不存在 / 非 GC-free / 类型不匹配；
  - 正例：final non-generic class + `Ptr<T>` handle 字段 + `@Extern(abi="c")` 释放函数。

验收：所有非法形态编译期拒绝，正例通过 typecheck。

### P2. Trampoline codegen + 填 `release_fn`

trampoline 形态：`void __scoop_release_<TypeMangled>(void *object)`，内部把 `object`（header 基址）按该 class 完整布局 GEP 出各 `args` 字段值，按序调用释放函数。

- P2-T01：trampoline 生成
  - 函数签名匹配 `ScoopTypeReleaseFn`（`void(void*)`）；
  - 用 `lookup_struct_field` / `codegen_class_field_ptr`（`layout.rs:192/287`）按字段名取偏移，从 header 基址（payload 在完整布局，含 header）读出每个 GC-free 字段值；
  - 解析释放函数符号（`lir_callable_symbol_facts`，`identity.rs:267`；`declare_dispatch_target_fun`，`gc.rs:6-52`），生成调用；
  - 确保被引用的释放函数不被 DCE（trampoline 的引用即为保活点，必要时显式标记）。
- P2-T02：在 `get_or_create_type_descriptor_global`（`gc.rs:1023`）里，当该类型带 `@ReleaseHook` 时，把 values 第 9 项（`gc.rs:1080`）从 `const_null` 改为 trampoline 函数指针；其余类型保持 null。
- P2-T03：IR 级 fixture（`crates/scoopc_codegen_llvm` 的 LLVM 测试 / build fixture）
  - 断言带 `@ReleaseHook` 的类型其 descriptor `release_fn` 非 null、指向 trampoline；
  - 断言 trampoline 读取正确字段偏移并以正确顺序/类型调用目标函数；
  - 断言无注解类型 `release_fn` 仍为 null（无回归）。

验收：codegen 产出正确 trampoline 且 descriptor 正确接线。

### P3. 验证矩阵、回归与文档收尾

- P3-T01：run-pass 端到端 fixture
  - 一个 final non-generic class 持有 native handle（`Ptr<T>`），构造时经 `@Extern(abi="c")` FFI 创建资源，`@ReleaseHook` 指向销毁函数；
  - 制造对象不可达 + 触发 GC，断言释放函数被调用且字段值正确传入（可用计数器 / side-effect 探针）；
  - 断言进程退出时存活对象**不**触发释放（验证 best-effort 语义边界）。
- P3-T02：四后端 parity（baseline moving / baseline non-moving / immix / minimal / hosted）——release_fn 调用一致；immix minor 与 major reclaim 都正确、单对象不重复释放。
- P3-T03：跨平台矩阵至少 `linux/amd64` + `macos/aarch64`。
- P3-T04：用 `@ReleaseHook` 写一个最小 demo 类型（持 `Ptr<T>` handle + `@Extern(abi="c")` create/destroy）作为机制 tracer bullet。真实的 `Mutex` / `CondVar` / `Once` 迁移在 P4 单独成阶段，本任务不做。
- P3-T05：文档/spec 回写
  - `SCOOP_FULL_SPEC.md`：新增 `@ReleaseHook` 章节（形态、约束、best-effort 语义、退出不回收、与 `@NoGC`/`@Extern` 的关系）；
  - `runtime/c/include/scoop_runtime.h:38-43` 与 `SCOOP_RUNTIME.md`：把 release callback 调用约定与 `@ReleaseHook` 关联说明对齐。

验收：端到端 + 四后端 + 双平台全绿；文档与 spec 同步；最小 demo 类型以纯 Scoop 形式工作。

### P4. `scoop.sync` 迁移到 `@ReleaseHook` 并清理 intrinsic

目标：把 `Mutex` / `CondVar` / `Once` 从「C 分配 + C 侧 type descriptor + `@Extern(abi="scoop")` op + `Once.run` `@Intrinsic` + 一批编译器特判」收敛为「普通 final non-generic class + `Ptr<T>` native handle 字段 + `@ReleaseHook` + `@Extern(abi="c")` op」，并删光编译器内关于这三个类型的全部硬编码。完成后，再加同类 sync 原语应只动 sysroot 源、编译器零改动。

当前现状（已核实）：

- `sysroot/lib/scoop.sync/src/sync.scoop`：`Mutex:21` / `CondVar:24` / `Once:27` 是 opaque `public class`；create/lock/unlock/destroy、condvar wait/notify、once create/is_done 都是 `@Extern(abi="scoop")`（`:32-101`、`:110-119`）；只有 `__scoop_sync_once_run`（`:130-131`）是 `@Intrinsic`（闭包 env/fn-ptr marshalling）。
- 这些对象当前由 C 侧 `scoop_sync_*_create` 用 `scoop_alloc_typed` + **C 写死的 type descriptor**（`sysroot/lib/scoop.sync/native/scoop_sync.c:209-224/334-349/496-511`，已各自 `.release_fn = scoop_sync_*_release`）分配；native handle 以 `{ ScoopObjectHeader, void *native }` 形式存放（`scoop_sync.c:173-182`），native 资源 malloc 自 pthread 原语。
- 编译器硬编码（全部要删或重指，见 §3 子表）：`Once.run` 的 codegen 特判与 runtime 声明、effect-facts 的无 effect 白名单、`scoop.sync` 的 session/project-model/auto-import 特殊处理，以及 lazy/delegate 属性同步对 `Mutex` 的 FQN 引用。

子任务：

- P4-T01：重写 `sync.scoop`——`Mutex`/`CondVar`/`Once` 改为 final non-generic class，持 `Ptr<T>` native handle 字段；构造经 `@Extern(abi="c")` create-native（返回裸 handle）填字段；lock/unlock/wait/notify/isDone 改为 method body 内解出 `self.handle` 调 `@Extern(abi="c")` op；`@ReleaseHook(name=destroyNative, args=["handle"])` + `@Experimental(feature="releaseHook")`。`Once.run` 用纯 Scoop 重写（基于已 class 化的 `Mutex`/`CondVar` + `isDone`），删除 `@Intrinsic`。
- P4-T02：收缩 `scoop_sync.c`——删除 GC 对象分配、C 侧 type descriptor 与 `*_release` wrapper；只保留对 raw native handle 的 create/destroy/op（create 返回 malloc 的 native struct 指针，destroy 接收并释放）。
- P4-T03：删除 `Once.run` intrinsic 全套硬编码——`runtime_symbols.rs:26`、`runtime_abi.rs:266-283`（`declare_runtime_sync_once_run`）、`call/lowering.rs:1558`（FQN dispatch）、`intrinsics/sync.rs:6-92`（`codegen_sysroot_sync_once_run`）、`effect_lowered/value.rs:1794`+`1925-1997`（`lower_sync_intrinsic`）、`closure/mod.rs:691`（`lookup_pure_unit_closure_type` 的 once 特例）。
- P4-T04：删除/重指其余 `scoop.sync` 特判——`effect_facts/builder.rs:2857-2867` 的 sync 无 effect 白名单（class 化后应由常规 effect 推导承接）；`session/mod.rs:242/269`、`scoop_project_model/graph_loader.rs:668/682/710/767`、`pipeline/llvm_codegen_stage.rs:617` 的 `scoop.sync` 特殊处理按需收敛。**决策点**：lazy/delegate 属性同步注入的 `Mutex`（`impl_lowering.rs:33-36` FQN 常量 + `decls.rs:965/982/1008/1025` + `sugar.rs:326/336/656/666/778/788/914/926`）是**消费侧**引用而非 Mutex 实现的一部分；二选一并在本任务记录决策：(a) 保留为经普通名字解析的 stdlib 引用（类比引用 `scoop.core` 类型），或 (b) 把 lazy-property 加锁合成下放到 sysroot helper，使编译器持零 sync FQN。
- P4-T05：测试与守卫——sync run-pass（lock/unlock、condvar wait/notify、once 单次执行 + 并发竞争）；四后端 + 跨平台 parity；新增「零编译器硬编码」grep 守卫测试，断言 `Mutex`/`CondVar`/`Once` 的 FQN 不再出现在编译器 crate（消费侧若选 (a)，守卫需精确排除该唯一允许点）；删除/迁移已被取代的旧 sync fixtures。

验收：`scoop.sync` 三类型为纯 Scoop class + FFI；`Once.run` 无 `@Intrinsic`；编译器内无这三类型的实现性硬编码（消费侧引用按 P4-T04 决策处理，P5 会将其彻底消除）；sync 全量回归与四后端/跨平台绿。

### P5. `lazy` / `observable` / `vetoable` 降为普通库 class

目标：把这三个属性委托从「编译器合成 backing 字段 + per-property `Mutex` 注入 + by-name 特判」降为实现已有 `ReadOnlyProperty`/`ReadWriteProperty` 协议的**普通库 class**，让属性委托 lowering 收敛成唯一的泛型路径。完成后，编译器只认识「属性委托协议」（字段存包装类 + `getValue`/`setValue`），对 `lazy`/`observable`/`vetoable` 一无所知。

关键依据（已核实）：

- 泛型委托路径**已经**把委托对象存成宿主类的普通字段、把读写编译成 `getValue`/`setValue`（`tests/fixtures/hir/delegated_property_lowering.scoop`）；这正是这三者要走的路，机制无需新增。
- 普通 class **本就能自由持有并改写 `var` 字段**（带正常 GC write barrier，零注解）；`sysroot/lib/scoop.core/src/core.scoop:1506` 的 `RefCell<T>`、`Atomic<T: ref>` 即是现成例子。`@InteriorMutable` 只是**值类型（struct 必须 `val`，见 `structs.rs:37-44`）的后门**，与 class 委托无关——因此本阶段**不需要任何新原语、不需要 cell**。
- 现有特判按代码注释自陈是「早期阶段」策略（`decls.rs:1002-1005`），无本质必要性。
- 当前新增前置缺口：泛型 class 实现参数化 interface 时，runtime itable metadata 可能对仍含 class type param 的 interface 实例使用 `NoTypeParamResolver` 计算 stable type id，触发 `missing stable type parameter key`。该问题必须先修复，不能通过改变委托库形状绕开。

子任务：

- P5-T00：修复泛型 class 实现参数化 interface 的 runtime itable stable type id 计算——确保 `Lazy<Int> : ReadOnlyProperty<Any, Int>` 这类实例不会以未替换 type param 求 stable id，并用 fixture 锁定通用形态。
- P5-T01：在 `scoop.delegates` 写 `lazy`/`observable`/`vetoable` 的库实现——普通 class 持自身 `var` 状态（lazy: `inited`/`value`；observable/vetoable: backing value + 回调），实现 `ReadOnlyProperty`/`ReadWriteProperty`；线程安全模式内部组合**库** `Mutex`（P4 后）或 `Once`。三个顶层 `lazy`/`observable`/`vetoable` 从 `@Intrinsic` 降为返回包装类的普通 `fun`；`lazy` 的 `LazyThreadSafetyMode`（None/Synchronized/Publication）各模式行为对齐。
- P5-T02：删除 by-name 合成与分叉——`hir/lower/sugar.rs` 三者的 get/set 合成（`:201-622`/`:624-854`/`:856-1047`）、`hir/lower/util/decls.rs` 的 backing 字段注入（`:933-987`/`:1000-1046`）、`impl_lowering.rs:33-36` 的 `SYNC_MUTEX_*` FQN 常量及其 `decls.rs:964-986`/`1021-1029` 使用点、`ParsedStdDelegateExpr::{Lazy,Observable,Vetoable}` 分叉；使 `DelegatedPropertyInfo` 只剩泛型分支。
- P5-T03：回归与守卫——把现有 lazy/observable/vetoable run-pass/hir fixtures 切到库实现并验证语义不变（含 lazy 三模式、observable 回调在「写后」、vetoable 否决不写）；把「零编译器硬编码」grep 守卫扩展到 `lazy`/`observable`/`vetoable` 与 `scoop.sync.Mutex` 注入点（P4-T04 的消费侧允许点此时应可一并删除）。

验收：三个委托为纯库 class；编译器属性委托 lowering 只剩泛型路径；`impl_lowering.rs` 的 `SYNC_MUTEX_*` 常量与 `sugar.rs`/`decls.rs` 的三者合成全部删除；语义与迁移前逐项一致，回归与守卫绿。

## 6. 风险与注意点

- **退出不回收**：必须在用户文档明确——`@ReleaseHook` 是尽力而为，不保证进程退出时被调用；需确定性释放的场景仍需显式 API。
- **共享 handle 双重释放**：同一 native handle 若被两个对象字段共享，会被各自的 trampoline 各释放一次——属用户责任，文档提示即可，编译器不介入。
- **字段偏移 vs header**：trampoline 必须基于含 header 的完整对象布局算偏移；这是最容易出错的实现点。
- **DCE 保活**：释放函数可能没有普通调用点，仅被 trampoline 引用；需确保不被优化掉。
- **`@Extern(abi="c")` 隐含 `@NoGC`**：P0 给 `@NoGC` 加 Pure 检查时，注意与 extern 既有 Pure 检查的协调，避免对同一函数重复或冲突诊断。
- **immix 移动/疏散**：确认死对象在被 evacuate 前其字段对 release_fn 仍然完整可读，且 minor→major 不会对同一对象重复调用 release_fn。
- **sync 双重释放 / 显式 destroy 共存（P4）**：现有 `Mutex.destroy()` 等显式销毁与 `@ReleaseHook` 会形成两条释放路径；native handle 必须用「已销毁」标志幂等化（现 `ScoopSyncMutexNative.destroyed` 已有此意），避免显式 destroy 后 GC 再次释放导致双重释放。
- **sync 行为基线不可回退（P4）**：迁移后 Mutex/CondVar/Once 的可见语义（可重入性、condvar 原子 unlock-wait-relock、once 的并发单次执行）必须与现有实现逐项对齐；以现有 sync 测试为基线，不得借迁移悄悄改变语义。
- **lazy/delegate 属性同步依赖（P4）**：编译器为 lazy/Observable/Vetoable 属性合成的加锁仍需一个 Mutex；这是消费侧依赖，P4-T04 必须显式决策是保留为普通解析引用还是下放 sysroot，不能在删 intrinsic 时连带破坏属性同步。P5 会把这三者整体库化，从而彻底消除该依赖。
- **委托库化的语义对齐与分配开销（P5）**：库版必须逐项复刻现有可见语义——lazy 三模式（None/Synchronized/Publication）、observable 回调在写之后、vetoable 否决则不写、并发可见性；线程安全靠委托类内部组合库 `Mutex`/`Once`。另：backing value 从「宿主类内联真字段」变为「委托对象字段」，每个委托属性多一次分配（与现有泛型委托一致），可接受但需在文档/基线说明。
