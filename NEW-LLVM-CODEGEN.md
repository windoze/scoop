# 新 LLVM Codegen 实现计划

> 状态：实现中（W1-1 骨架已完成；见末尾「实现进度」）
> 范围：在新的 `scoop2_*` pipeline（HIR → MIR → LIR）之后，从零实现一套**完整、正确、无 placeholder** 的 LLVM codegen，并打通 build/run，使全套 pipeline 能产出正确的可执行程序。
> 输入：`scoop2_lir::LirProgram`（自包含，codegen 无需回查 HIR/MIR）。
> 约束：继续沿用 **explicit root frame** GC 方案，不做 statepoint/stackmap。禁止 `todo!`/`unimplemented!`/`unreachable!`/`panic!` 占位（仓库有 `no_placeholder` 守卫）。

---

## ⚠ 关键发现（2026-07-29 实测，改变整体策略）

实测 `scoop2_*` 端到端管线后发现一个**根本性事实**：新 pipeline **从未达到过端到端可运行状态**。

- `scoop2c` 只有 `check-source` / `dump-*` 子命令，无 `build` / `run`。
- 更关键：**sysroot（scoop.core 等）的函数体从未被 lowering**。`scoop2_hir::typecheck` 只为 `InputOrigin::User` 文件创建 `TypedFile`（含 `package_prefix` / body 类型）；sysroot 文件只贡献**符号声明**（Phase 1/3 注册到 index），其函数体（`println<T>`、`String.concat`、`Int.toString` 等所有可编译的库函数）从不进入 Phase 2（`resolve_file_bodies`）。
- 因此 `println("x")` 这类程序里，`println<String>` 永远不会被单态化——它既不是 intrinsic（无 body 的 `@Intrinsic`），也没有可编译 body，最终是个无解析的符号。

**尝试方案 A（扩展 HIR 让 sysroot 也产出 TypedFile）实测结果**：把 Phase 2 的 `filter(User)` 改为处理全部文件后，sysroot 函数体被 typecheck，但暴露出 **208 个潜在 typecheck 错误**（135 `callee_not_callable` 主要是 `__scoop_*` extern 符号、62 `unresolved_member` 等）。这说明 **sysroot 源码是为旧 pipeline 的 typecheck 模型写的，与新 `scoop2_hir` typechecker 不兼容**；`@Extern`/`@Intrinsic` 函数的注册/可调用性在新 typechecker 中未覆盖 sysroot body 的全部用法。

**结论与修订策略**：完整 e2e 不是「重写 codegen」单一任务，而是三层叠加的大工程：
1. **sysroot typecheck/lowering 修复**（独立大项，208 个潜在错误，跨 `scoop2_hir` + sysroot 源）；
2. **codegen 本体**（本计划主体）；
3. **build/run 驱动 + 链接**。

本计划聚焦 **第 2、3 层（codegen + 驱动 + 链接）**，并假设 sysroot lowering（第 1 层）能产出合法 `LirProgram`。为避免在 sysroot 修复上无限投入，codegen 采用**自底向上、可独立验证**的策略：先让无 sysroot 依赖的 codegen 单元测试（直接构造 `LirProgram`）跑通全部 lowering 路径，再在 sysroot lowering 修复后接入 e2e。



---

## 0. 背景与现状审计

### 0.1 新 pipeline 的成熟度（决定 codegen 的输入契约）

经审计，`scoop2_*` pipeline 已完整实现到 LIR：

- `scoop2_syntax`（parse）+ `scoop2_hir`（resolve + typecheck，产出 `TypedHir`）
- `scoop2_mir::lower::lower_module` → 生成 generic `Module`
- `scoop2_mir::materialize::materialize(module, Some("main"), hir)` 一次性跑完：**monomorphize → devirtualize → inline → effect_lower → compute_public_stable_keys**，产出 `MaterializedMir`（含 `BackendContracts`）
- `scoop2_lir::lower_to_lir(mir, hir, interner)` → 产出自包含 `LirProgram`

`LirProgram` 已携带 codegen 所需的全部信息（`crates/scoop2_lir/src/program.rs`）：`callables`（Plain + EffectStep）、`declarations`（extern）、`type_layouts`、`vtables`/`itables`/`class_itables`、`type_descriptors`、`global_init`、`class_inits`、`synthetic_types`（Step/Continuation/Frame）、`closure_layouts`。

**当前缺口（已知）**：LIR 在若干处仍带"占位语义"，codegen 落地时需要把它们补成真实信息，必要时回修 LIR：
1. `LirCallKind::Virtual`/`Interface` 映射时 `vtable_slot`/`itable_slot`/`interface_id` 硬编码为 0，由 `dispatch::backfill_call_sites` 回填——需确认回填已覆盖所有调用点（见 §5 风险 R1）。
2. `compute_field_offset`（lib.rs:442）用 ad-hoc 大小估算，与 `TypeLayoutTable` 不一致；codegen 需以 `TypeLayoutTable` 的 `FieldLayout.offset` 为唯一来源（必要时把 LIR 改为产出 `field_offset` 时查表而非估算）。
3. `map_rvalue` 中 `Call.result_ty = transport.result.source_ty`、`MemberAccess.result_ty` 已修正查 HIR——需逐项核验所有 `result_ty` 字段正确，因为 codegen 完全依赖它们选 LLVM 类型。
4. `TopLevelRef.ty` 用 `find_any_type` 兜底——需用真实全局类型。

**结论**：codegen 以 `LirProgram` 为输入，但实现过程中要把上述 4 点落实为"真实信息"，对 LIR 做最小必要修订。本计划默认 LIR 在这些点上可信（code phase 时核实，必要时补一个 LIR 小修复），不再把 LIR 当黑盒。

### 0.2 旧 codegen（`scoopc_codegen_llvm`）可复用的机制清单

旧 crate（~91k 行，消费老 `scoopc_lir`）功能覆盖完整，但其**复杂度主要来自三处**：(a) 同时维护 MIR-source 与 LIR-source 两条 lowering 路径；(b) effect_lowered 子树（~35 文件）把状态机 ABI 全部在 codegen 内物化；(c) handoff/多 cone 缓存/增量编译的产品级设施。

新设计把这些复杂度排除：状态机 ABI 已在 LIR 物化（`FrameSchema`/`StepLayout`/`StateDispatch`/`ContinuationLayout`），codegen 只做机械翻译。可**复用（拷贝后重写）**的机制（来自旧 crate 的机制映射，非代码搬运）：

| 机制 | 旧 crate 位置 | 复用要点 |
|------|--------------|---------|
| explicit root frame 链表 push/pop + descriptor 全局 | `main/frame.rs`, `runtime_abi.rs` | `alloca {header; [N x ptr]}`；TLS `__scoop_explicit_root_frame_top`；entry push、return/teardown pop；desc 全局 + offsets 数组 |
| GC slot 镜像（含 GC 指针的 local 同步到 frame） | `main/alloca.rs`, `gc.rs` | 存/取含 GC 指针的值时双向同步；调用边界保守 spill/restore |
| 类型 → LLVM 类型（class=header+payload；enum=tagged union/niche/value-only） | `ty.rs`, `types.rs`, `enum_lowering.rs` | `ptr addrspace(1)` 表示 GC 引用；enum 把 GC 指针单列字段，杜绝 ptr↔int punning |
| 调用 lowering（Direct/Virtual=vtable GEP/Interface=itable 线性扫描/Closure/FunValue/FunPtr） | `call/` | 均内联 LLVM，无新 runtime 调用 |
| 对象构造（class ctor=alloc+memset+init 体；struct/tuple=insertvalue；enum variant=tagged/boxed payload；closure=alloc obj+alloc env） | `class_ctor.rs`, `aggregates.rs`, `closure/` | alloc 经 `scoop_alloc_typed` |
| C-ABI `main` + runtime_init + 全局/once 初始化 | `emit.rs`, `main/`, `cone_init.rs` | `main` 序列固定 |
| 字符串字面量为 immortal 全局 | `main/immortal.rs` | off-heap、`SCOOP_GC_FLAG_IMMORTAL` |
| 内置/算子/`is`/`as`/反射内联 | `intrinsics/` | 算术内联；`is`=type_desc 比较；`as`=比较+panic；`sizeOf/alignOf/kindOf/descOf` |
| type descriptor + trace bitmap 全局 | `gc.rs` | 14 字段 struct；bitmap 数组全局；release trampoline |

新 codegen **不复用**的：effect_lowered 子树（已被 LIR 的 effect_prep 取代）、MIR-source 路径、handoff/多 cone/增量、stackmap/statepoint。

### 0.3 运行时契约（`runtime/c`，已审计）

构建：`scoop_runtime/build.rs` 用 `cc`（强制 clang）编出 **`libscooprt.a`**，默认 GC 后端 immix。它作为普通依赖 crate 被链接（`cc::compile` 自动 `cargo:rustc-link-lib`）。平台胶水在 `runtime/c/platform/`（posix/win32 + unwind）。**没有独立 Makefile**，唯一路径是 build.rs。

code layer 需调用的 C 符号（分组，签名见 `include/scoop_runtime.h` / `scoop_gc.h`）：
- **初始化/生命周期**：`scoop_runtime_init`、`scoop_gc_thread_attach_current`/`detach`、`scoop_enter_native`/`scoop_leave_native`、`scoop_entry_argv_array(argc,argv)`。
- **分配**：`scoop_alloc_typed(type_desc, size)`（GC 对象必经）、`scoop_alloc(size)`。
- **GC/roots**：TLS 全局 `__scoop_explicit_root_frame_top`（root frame 链表顶）；`scoop_gc_write_barrier(slot, value)`（堆内 ref 写）；`scoop_gc_register_global_root(base, type_desc)`（模块级 GC 全局）；`scoop_gc_safepoint_poll`（可选 poll）；`scoop_gc_collect`/`scoop_gc_collect_minor`。
- **pin/handle**：`scoop_pin`/`scoop_unpin`/`scoop_handle_new`/`scoop_handle_get`/`scoop_handle_drop`。
- **string**：`scoop_string_concat`/`equals`/`byte_length`/`bytes`/`from_owned_bytes`/`unsafe_slice_bytes`、`scoop_int_to_string`/`bool_to_string`/`char_to_string`/`float32/64_to_string`、`scoop_float32/64_to_int`。
- **array**：`scoop_mutable_array_new`/`len`/`elem_kind`/`elem_size`/`push_word`/`push_ref`/`push_composite`/`to_array_data`/`freeze`。
- **composite transport**：`scoop_composite_trace`/`copy`/`drop`（boxed value/enum payload/closure env/array elem）。
- **打印/panic**：`scoop_print`/`scoop_println`、`scoop_panic`、`scoop_runtime_error_fatal`。
- **once**：`scoop_once_begin`/`scoop_once_end`（全局/对象 init 守卫）。

ABI struct（code layer 自行声明 LLVM 等价类型）：`ScoopObjectHeader{next,type_desc,size_bytes,flags,mark}`（payload 紧随）、`ScoopTypeDescriptor`（14 字段，前 7 字段偏移固定）、`ScoopCompositeTransportDescriptor`、`ScoopRootFrameHeader{prev,desc}`、`ScoopRootFrameDesc{slot_count,slot_offsets}`。`SCOOP_GC_FLAG_IMMORTAL=0x80000000`、`SCOOP_GC_MARK_IMMORTAL=0xFFFFFFFF`。元素种类 `SCOOP_ARRAY_ELEM_KIND_{WORD,REF,COMPOSITE}`。

---

## 1. 工作总览

要交付的不止是 codegen crate，而是**端到端能 build/run 的完整 pipeline**。拆为 5 块工作：

| # | 工作 | 产出 |
|---|------|------|
| W1 | 新 crate `scoop2_codegen_llvm`：消费 `LirProgram` 产出 LLVM module（IR / object） | 库 |
| W2 | 驱动集成：给 `scoop2c` 增加 `build`/`run` 子命令，串联 LIR→codegen→object→链接→执行 | 改 `scoop2c` |
| W3 | 链接：定位 `libscooprt.a`，用 clang 链接出可执行文件 | 链接器调用（在驱动内） |
| W4 | 正确性闭环：跑通 run-pass fixture，审计并修正/新增 fixture | fixture 通过 |
| W5 | 错误处理：非法程序在 codegen 层明确报错（而非生成坏码或 panic） | 诊断 |

**完成标准**（与任务要求一一对应）：
1. 新 crate，不复用旧 crate 代码（机制可借鉴，代码重写）。
2. 完整覆盖 spec 所有合法语法成分；任何合法程序生成合法结果；任何错误程序明确报错。
3. 无 `todo!`/`unimplemented!`/`panic!`/占位（通过 `no_placeholder` 守卫 + code review）。
4. explicit root frame 方案，不做 statepoint/stackmap。
5. e2e run-pass fixture 基本全过（已审计旧 fixture 需逐个核对 stdout 语义，必要时修正/新增）。
6. 全套 pipeline（除优化）完整重写完成：能编译、链接、生成正确可执行程序。

---

## 2. Crate 结构（W1）

```
crates/scoop2_codegen_llvm/
├── Cargo.toml              # 依赖 inkwell(llvm21-1) + llvm-sys(211) + scoop2_lir/scoop2_hir/scoop2_base
├── build.rs                # (沿用 llvm-sys 标准做法；从 LLVM_PREFIX / brew llvm 定位)
├── src/
│   ├── lib.rs              # 公开 API：CodegenError + emit_program(program, opts) -> EmittedModule
│   ├── error.rs            # CodegenError（miette Diagnostic；唯一错误出口）
│   ├── context.rs          # CodegenContext：inkwell Context + Module + TargetData + 全局缓存表
│   ├── target.rs           # host target triple/datalayout（无 llvm-config 也能用 inkwell 内建 host）
│   ├── types/
│   │   ├── mod.rs          # TypeLowerer：TypeId/TypeLayout -> LLVM BasicTypeEnum（带缓存）
│   │   ├── scalars.rs      # Unit/Bool/Char/Int*/Float*/String/Any/引用指针
│   │   ├── aggregates.rs   # struct/tuple/Option(niche)/enum(tagged union) LLVM struct
│   │   └── runtime.rs      # ScoopObjectHeader/ScoopTypeDescriptor/... 的 LLVM 类型声明
│   ├── globals/
│   │   ├── mod.rs          # 全局声明/定义管理（type_desc/vtable/itable/string-literal/closure）
│   │   ├── type_desc.rs    # TypeDescriptor 全局 + trace bitmap 全局（从 LirProgram.type_descriptors）
│   │   ├── dispatch.rs     # vtable 全局([N x ptr]) / itable 全局(_entries) / class_itables 填充
│   │   └── strings.rs      # immortal string literal 全局（content-hash 去重）
│   ├── runtime_abi.rs      # 所有 runtime C 符号的 declare（External + 正确签名）
│   ├── gc/
│   │   ├── mod.rs          # ExplicitRootFrame 管理：alloca/push/pop/desc 全局 + offsets
│   │   ├── mirror.rs       # 含 GC 指针 local 的 slot 镜像（存/取双向同步）
│   │   └── safepoint.rs    # 调用边界保守 spill/restore；safepoint poll 包装
│   ├── body/
│   │   ├── mod.rs          # FunctionLowerer：驱动单 callable 的 lowering（locals→alloca、块→BB）
│   │   ├── locals.rs       # local 分配 + GC slot 预留 + 镜像登记
│   │   ├── stmt.rs         # LirStmtKind lowering（Assign/StoreMember/StoreTupleIndex/StoreGlobal/Panic/Nop）
│   │   ├── rvalue.rs       # LirRvalue lowering 分发
│   │   ├── operand.rs      # LirOperand -> LLVM Value（local load / 常量物化）
│   │   ├── consts.rs       # LirConstValue 物化（含 string literal 走全局）
│   │   ├── call.rs         # LirCall lowering（Direct/Virtual/Interface/Closure/FunValue）
│   │   ├── construct.rs    # ClassCtor/StructLit/MakeTuple/MakeArray/EnumVariant/MakeClosure/ClassLit
│   │   ├── access.rs       # MemberAccess/TupleIndex/IndexAccess（GEP/load，含 write barrier）
│   │   ├── cast.rs         # TypeTest(is)/Cast(as) — type_desc 比较链 + panic 路径
│   │   ├── pattern.rs      # PatternMatch/PatternExtract（CondBr/IntEq/variant tag/or 链）
│   │   ├── control.rs      # terminator lowering（Return/Goto/CondBr/Unreachable）
│   │   └── effect.rs       # EffectStep callable lowering：step 机/continuation/resume/state dispatch
│   ├── intrinsics.rs       # 内置算子（算术/比较内联）、println/print、反射、atomic、unsafe ptr
│   ├── entry.rs            # C-ABI main 生成 + runtime_init + global_init 调用序
│   └── emit.rs             # module → object file（TargetMachine）+ IR text（调试）
└── tests/
    ├── no_placeholder.rs   # 仓库标准守卫（拷贝自 scoop2_lir）
    └── smoke.rs            # 无需 LLVM 链接的最小结构测试（可选）
```

**设计原则**：
- **机械翻译**：遍历 `LirProgram` 时不做语义推断，所有布局/偏移/槽位取自 LIR 产出的表。遇到 LIR 缺信息（§0.1 四点）即在该点补 LIR，绝不 codegen 内猜。
- **单一错误出口**：所有"不该发生"的输入走 `CodegenError`（带 span/FQN），绝不 `panic!`/`unwrap`。`no_placeholder` 守卫强制这一点。
- **LLVM 类型缓存**：`TypeId`/`TypeLayout` → `BasicTypeEnum` 缓存，避免重复建 named struct。
- **addrspace**：GC 引用统一 `ptr addrspace(1)`，native/C-ABI 指针 `ptr addrspace(0)`（与旧设计一致，让 write barrier/targeting 清晰）。

---

## 3. 关键机制设计（W1 细节）

### 3.1 类型 lowering（types/）

`TypeLowerer::lower(ty: TypeId) -> BasicTypeEnum`，查 `program.type_layouts.get(ty)`：
- `Scalar`：Unit→`{}`（0 字节，用 i8 占位/void 处理）、Bool→i8、Char→i32、Int{bits,unsigned}→iN、Float{bits}→f32/f64。
- `Reference`/`Function`/`String`/`Any`/`Object`：`ptr addrspace(1)`。
- `Struct`/`Tuple`：named `{ field0, ... }`（按 `FieldLayout.offset` 顺序；offset 间隙插 `[N x i8]` padding，保证与 LIR 一致）。
- `Option`：按 `NicheStorage`——Pointer→直接 payload ptr(None=null)；U8{none_value}→payload 整型 + 编码 None；Tagged→`{ i8 tag; payload }`。
- `Enum`：`{ iN tag(在 tag_offset); payload_union }`；**GC 指针字段必须单列**（不复用 union 槽与标量覆盖），杜绝 ptr↔int punning，保证 GC 可枚举。若变体 payload 含 GC 指针且不能静态枚举 → boxed payload（单独 `scoop_alloc_typed` 对象，enum 内存只放指向它的 GC 指针）。
- `Nothing`：i8 占位（不实际产生值）。

class 对象类型 = `{ ScoopObjectHeader; <payload struct> }`；payload 字段顺序按 HIR members（偏移与 LIR 一致）。closure 对象 = `{ header; env_ptr(ptr addrspace1); invoke_fn_ptr(ptr addrspace0) }`。

### 3.2 explicit root frame（gc/）—— **GC 安全的核心**

每个"含 GC 操作"的函数（Plain 凡有 GC local 或调用、EffectStep）在入口：

1. **alloca**：`%frame = alloca { ScoopRootFrameHeader, [slot_count x ptr] }`（slot_count = 本函数 GC 指针叶子总数，由 body 的 locals 递归展开含 GC 指针的类型得到，等价 LIR `GcInfo.gc_locals` 经布局展开）。
2. **push**：`header.prev = __scoop_explicit_root_frame_top`；`header.desc = <本函数 desc 全局>`；清零所有 slot；`__scoop_explicit_root_frame_top = %frame`。
3. **desc 全局**：每个含 GC 函数一个 `ScoopRootFrameDesc{slot_count, ptr→offsets[]}` Internal 常量 + offsets `i32[]` 常量（offset = header_size + i*ptr_size）。
4. **slot 镜像**：含 GC 指针的 local（含内嵌 GC 指针的 struct/enum）的每个 GC 指针叶子登记一个 frame slot。store 到该 local 时**同时**把 GC 指针叶子写入对应 frame slot；use 时**优先**从 frame slot reload（GC 后的真实值）。
5. **safepoint**：所有 `scoop_alloc*`/普通调用/scoop_gc_collect 前，把所有 live GC local 的 frame slot 当作权威源；调用边界用保守 spill/restore 包裹（与旧 `with_conservative_gc_local_root_spills` 等价，但简化为"全部已登记 slot"，因为 slot 镜像已保证权威性）。
6. **return/teardown**：函数的每个 `ret`/`unreachable` 前：清零 slot → `__scoop_explicit_root_frame_top = %frame.prev`（pop）。

**写屏障**：堆内（addrspace1）slot 写 GC 指针时经 `scoop_gc_write_barrier(slot_addr, value)`（返回写入值）。模块级 GC 全局用 `scoop_gc_register_global_root` 登记。

### 3.3 调用 lowering（body/call.rs）

- **Direct**：`runtime_abi` 已 declare 的符号或本 module 定义符号，直接 `call`。大返回值走 sret（caller 传 hidden ptr）—— sret 判定 = `param_abi == Indirect`（LIR 已决策）。
- **Virtual**：内联 LLVM——receiver→header ptr→GEP type_desc→load→GEP vtable(field 13)→load→GEP slot→load fn ptr→indirect call。slot 取 `LirCallKind::Virtual.vtable_slot`（LIR backfill 后已填）。
- **Interface**：内联线性扫描——type_desc→itable(field 12)→遍历 entries 比较 interface_id→命中的 entry 取 methods[slot]→indirect call。
- **Closure/FunValue**：从 closure 对象 GEP env_ptr/invoke_fn_ptr，env_ptr 作首参 indirect call。
- 所有调用经 safepoint 包装（§3.2-5）。

### 3.4 对象构造（body/construct.rs）

- **ClassCtor**：`scoop_alloc_typed(class_type_desc, size)`→cast addrspace1→GEP payload→memset 零→执行 init（字段初始化按声明/参数顺序；超类委托先调）。release hook 生成 trampoline 全局。
- **StructLit/MakeTuple**：`insertvalue` 逐字段（值类型，栈上；含 GC 指针则同步镜像 slot）。
- **EnumVariant**：插 tag + payload；GC 指针 payload 走单列字段或 boxed（§3.1）。
- **MakeArray**：`scoop_mutable_array_new(elem_kind, size, align, desc, cap)`→逐元素 push（word/ref/composite）→`freeze` 得 immutable Array。
- **MakeClosure**：先建 invoke LLVM 函数（捕获入 env）；`scoop_alloc_typed(closure_desc)`；env_ptr 先置 null（env alloc safepoint 安全）；若有捕获再 `scoop_alloc_typed(env_desc, env_size)` 写捕获值。
- **ClassLit**（`T::class`）：返回 type_desc 全局地址（`UIntPtr`）。

### 3.5 入口与初始化（entry.rs）

C-ABI `i32 main(i32 argc, ptr argv)`，固定序列：
1. `scoop_runtime_init()`（仅一次；runtime 内部自幂等）。
2. 含 GC 的 main：push root frame。
3. 顶层 val/var 初始化：按 `global_init` 顺序调各 init_callable（once 守卫可选；val 一次性）。GC 全局 `scoop_gc_register_global_root`。
4. 若 `main(args)` 形：`argv_array = scoop_entry_argv_array(argc, argv)` 作首参。
5. 调用户 `main`（Plain），取返回值；Unit main→exit 0，Int main→该退出码。
6. pop root frame，`ret exit_code`。

### 3.6 EffectStep lowering（body/effect.rs）

EffectStep callable 的 `LirCallable.abi == EffectStep`，LIR 已给 `frame_schema`/`step_layout`/`state_dispatch`/`continuation_layout`。codegen 翻译为状态机函数 `step(frame_ptr, resume_payload?) -> Step`：
- **入口**：从 `frame_ptr` 读 `state` 字段，按 `state_dispatch.entries` 生成 switch/jump（state 0=初始入口；state N=第 N 个 resume 续点对应的 block）。
- **Complete**：构造 `step_layout.complete_variant`（tag=0 + 完成值），return Step。
- **effect 操作（Perform）**：把 resume 续点编号写入 frame.state、保存 live local 到 frame slots（§3.2 镜像同样适用 frame slot），构造 `step_layout.effect_variants` 对应的 Step（tag + perform payload），return Step（挂起）。
- **continuation/resume**：按 `continuation_layout` 构造 continuation 对象（`scoop_alloc_typed`，字段含 resumed 标志/resume state tag/frame ptr/step fn ptr/resume value）；resume 调用 = 调 step 函数并按 state dispatch 续行。GC root frame 同样覆盖 frame 中的 GC slot。
- Step 类型本身是 LIR `synthetic_types` 中的 tagged union，按 §3.1 enum lowering。

> 说明：effect 不引入新 runtime 符号（与旧实现一致）；状态机、continuation、Step 全是纯 LLVM 实体 + `scoop_alloc_typed` + root frame。这是新设计相比旧 crate 最大简化点：旧 crate 用 35 文件在 codegen 内物化的 ABI，现在由 LIR 物化、codegen 机械翻译。

---

## 4. 驱动集成与链接（W2 + W3）

### 4.1 给 `scoop2c` 加 `build` / `run` 子命令

当前 `scoop2c` 只有 `check-source`/`dump-*`。新增：
- `scoop2c build <file.scoop> -o <exe|--obj <path>> [--opt-level <n>] [--emit-ir <path>]`：
  parse→resolve→typecheck→MIR lower→materialize(entry=main)→LIR→codegen→object/IR。
- `scoop2c run <file.scoop> [args...]`：build 到临时 exe 再 exec。

退出码：成功 0；编译/链接错误 1；用法错误 2（沿用现有约定）。诊断复用 `scoop2_base::diag` 渲染。

复用现有 `run_dump_mir`/`run_dump_lir` 已建好的"parse→…→materialize→LIR"管线，在 LIR 之后接 `scoop2_codegen_llvm::emit_program`。

### 4.2 链接（W3）

object → exe 用 **clang 作链接驱动**（与 runtime 一致，且 Windows 上用 clang-cl）：
- 链接输入：用户 object + `libscooprt.a`（+ 平台 unwind 胶水，已在 scooprt 内）。
- **定位 libscooprt.a**：`scoop_runtime` 的 cc 产物在 `OUT_DIR`，但驱动是独立二进制，不直接拿 build.rs 链接。方案（按优先级）：
  1. 环境变量 `SCOOP_LIBSCROOT` / `SCOOP_RUNTIME_LIB`（用户/CI 显式指定，最稳）。
  2. `scoop_runtime` crate 提供一个 build-script 产出的 `cargo:rustc-env` 或一个编译期 `env!` 暴露 `OUT_DIR` → 驱动 `cargo build -p scoop2c` 时把它烘焙进去（开发态零配置）。
  3. 兜底：在已知 target 目录下 `find` `libscooprt.a`。
- 链接标志：posix `-lm -lpthread -ldl`（Linux）；macOS 系统 libc 自带；Windows ntdll。`@Extern(abi="c")` 的额外 lib 由 sysroot/包声明提供（本阶段先支持单一可执行程序，跨包 lib 暂不支持，列为已知限制）。
- 可执行程序运行需能找到 immortal string/type_desc 等——均为 module 内全局，链接即可，无运行时查找。

### 4.3 与现有 fixture 基础设施对接（W4）

`tools/run_fixtures.py` 的 run-pass 走 `options.scoop run`（`SCOOP_BIN` 解析 `target/debug/scoop`）。新 pipeline 要复用 run-pass fixture，有两条路（实现时择一，倾向 A）：
- **A（推荐）**：让 `scoop2c` 支持与旧 `scoop` 兼容的 `run`/`build` 子命令子集，fixture runner 经 `SCOOP_BIN=target/debug/scoop2c` + 小补丁（或 run_fixtures 增加 `--driver scoop2c` 路由）驱动 run-pass。旧 `scoop` 驱动是 cone/多文件/项目图复杂体系，run-pass 多为单文件，scoop2c 单文件 build/run 足够。
- **B**：新增一个 fixture phase 目录（如 `tests/fixtures_ng/run_pass/`）专跑新 codegen，避免动旧 fixture runner。

无论哪条，run-pass fixture 的 `stdout` 校验语义照旧（精确匹配 `.stdout` 或 `RUN-STDOUT`，或 `EXPECT: pass` 期望退出码 0）。

---

## 5. 正确性与错误处理（W5 + W4）

### 5.1 "合法程序→合法结果"

逐 spec 功能域核对 codegen 覆盖（核对清单，实现时逐项打勾）：
- 标量/算术/比较/位/移位（内联）；Char/Bool/Float；Int 定宽变体；Unit。
- String：concat/runtime 转字符串走 runtime 符号；字面量 immortal 全局；`==` 内容比较（`scoop_string_equals`）；f-string 拼接。
- tuple/struct/enum/Option（含 niche）；`with` 更新（值拷贝改字段）；解构（pattern）。
- class 继承/vtable；interface/itable；`is`/`as`/`as?`；boxing（值→interface/Any）。
- 闭包/函数值/trailing lambda；泛型（已单态化，code 只见具体类型）。
- 控制流：if/while/for(→iterator)/match/return/break/continue（MIR 已降为 CFG，code 翻 terminator）。
- 顶层 val/var + @ThreadLocal/@Global；@Extern(c/scoop)；@CLayout；@NoGC；@Unsafe + Ptr/FunPtr；GC.pin/unpin/handle；@ReleaseHook；反射 sizeOf/alignOf/kindOf/descOf。
- effect/handle/on/finally/perform/resume/continuation（→ step 机，§3.6）。
- 四种 `main` 签名 + exit code 规约。

### 5.2 "错误程序→明确报错"

codegen 是 pipeline 末端，绝大多数非法程序在 parse/typecheck/MIR/LIR 阶段已报错。codegen 层需明确报错的剩余情形：
- LIR verify 已通过的程序理论上对 codegen 是良构的；但 codegen 仍需对**自身能力边界**报错（而非生成坏码），例如：遇到 LIR 声称有 body 但 codegen 未覆盖的 rvalue/terminator 变体 → `CodegenError::UnsupportedConstruct { fqn, kind }`（带 span）。
- 符号未定义（extern 符号在链接期才暴露）：codegen 发现 `Direct` callee 在 declarations/definitions 都找不到 → codegen 阶段报 `UndefinedSymbol`（链接期错误前移）。
- 目标平台不支持（如某个 intrinsic）→ 报带 `target_platform` 的错误。

**严格禁止**：`todo!`/`unimplemented!`/`panic!`/`unwrap` 期望良构。所有 `match` 穷尽性用显式 `_ => Err(CodegenError::...)` 兜底（而非 `_ => unreachable!`）。`no_placeholder` 守卫 + 一次专项 review 保证。

### 5.3 fixture 审计与修正（W4）

已审计 run-pass 共 **426** 个（其中 **153** 涉及 effect）。旧 fixture 由旧 pipeline 生成，**不能盲信正确**。处理策略：
1. 先跑最小子集（算术/控制流/string/class）→ 暴露 codegen 最常见路径 bug。
2. 逐域扩展；对每个 `.stdout` 与"Scoop 语义"对照（不是与旧实现对照），发现旧 fixture stdout 错误→修正 fixture（在 PR 说明），或写新 fixture 替代。
3. effect 153 个放最后（依赖 §3.6 完整）。
4. 已知旧 fixture 不可靠来源：GC pacing（`SCOOP_GC_PACING`）、堆对象计数断言（依赖具体 GC 后端行为）——这类 fixture 标注 `ENV`/`SYSROOT-DEPS`，新 codegen 须尊重这些 env。
5. 新增少量针对性 fixture（如 enum niche 编码、closure 捕获含 ref、interface dispatch 多态）补覆盖盲区。

### 5.4 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| **R1**：LIR 调用点 slot 回填不全 → virtual/interface 分发错误 | 分发错值/崩 | codegen 落地前先写一个"slot 完整性"断言测试：对每个 Virtual/Interface 调用点校验 `vtable_slot`/`itable_slot`/`interface_id` 非零且在表内；不全则补 LIR `backfill_call_sites`。 |
| **R2**：field_offset LIR ad-hoc 估算与 TypeLayoutTable 不一致 | 字段读写错位 | 改 LIR：`compute_field_offset` 改查 `type_layouts` 的 `FieldLayout.offset`；codegen 只信 LIR。 |
| **R3**：enum GC 指针字段不能静态枚举导致 GC 漏扫 | GC 收集活对象 → 内存安全 | box 含 GC 指针且不可静态枚举的 payload（§3.1）；trace bitmap 仅记可静态枚举的 GC 偏移。 |
| **R4**：root frame 在 LLVM 优化下被消除 | GC 移动对象后引用失效 | root frame 经 runtime-visible TLS 指针链访问，LLVM 不会跨调用消除其 store；早期关闭优化（`--opt-level` 仅在验证 safe 后启用），默认 O0/O1。 |
| **R5**：libscooprt.a 定位失败 | 无法链接 | §4.2 三级回退；CI 固定走 env 显式指定。 |
| **R6**：inkwell/llvm-sys 21 与本机 LLVM 版本不匹配 | build 失败 | 强制 `LLVM_SYS_211_PREFIX=/opt/homebrew/opt/llvm@21`（CI/文档）；本机已确认有 llvm@21。 |
| **R7**：旧 fixture stdout 反映旧实现而非 spec | 误判 codegen bug | §5.3-2 逐个核对语义，必要时修 fixture。 |

---

## 6. 实现步骤（建议顺序）

每步都先确保 `cargo build`/`cargo test` 绿，再进入下一步；每步不留 placeholder。

1. **Crate 骨架 + target + runtime_abi + 类型 lowering**（W1 基座）
   建 `scoop2_codegen_llvm`，加入 workspace；inkwell 初始化 host target；声明全部 runtime 符号；实现 `TypeLowerer`（覆盖所有 TypeLayoutKind）；为内置类型出 smoke（构造 module、取 LLVM 类型）。✅验证：crate 编译；内置类型 LLVM 类型正确。
2. **全局层**：type_desc 全局 + trace bitmap + vtable/itable 全局 + string literal 全局 + closure layout。
   ✅验证：对一个含 class/interface/string 的程序产出 module，肉眼/IR 检查全局正确。
3. **GC root frame + 含 GC local 的镜像**（gc/）。
   ✅验证：单函数 alloca/push/pop/desc IR 正确；含 GC local 存取镜像正确。
4. **Plain 函数体 lowering**：locals→alloca、stmt（Assign/Store*）、rvalue 分发、operand/const、terminator。
   ✅验证：算术/控制流程序 IR 正确。
5. **构造与访问**：construct.rs（全部）+ access.rs + cast.rs + pattern.rs + intrinsics（算术/print/string/反射）。
   ✅验证：string/class/struct/enum/closure/match 程序 IR 正确。
6. **调用 lowering**：Direct/Virtual/Interface/Closure/FunValue + safepoint 包装 + write barrier。
   ✅验证：含继承/接口/闭包/函数值调用的程序 IR 正确（R1 slot 完整性测试在此落实）。
7. **entry + global_init + main 四签名**（entry.rs）。
   ✅验证：可生成完整 module。
8. **emit object + 驱动 build 子命令 + 链接**（W2+W3）。
   ✅验证：`scoop2c build hello.scoop -o /tmp/a.out && /tmp/a.out` 跑通最小程序。
9. **run 子命令 + run-pass 最小子集**（W4 启动）。
   ✅验证：算术/控制流/string/class run-pass 通过。
10. **EffectStep lowering**（body/effect.rs）。
    ✅验证：effect run-pass 通过（§3.6）。
11. **全量 run-pass + fixture 审计修正 + 错误处理收尾**（W4+W5）。
    ✅验证：run-pass 基本全过；非法程序明确报错；`no_placeholder` 守卫通过；code review 确认无占位。

---

## 7. 完成定义（DoD）

- [ ] `crates/scoop2_codegen_llvm` 独立新 crate，无旧 crate 代码搬运；`cargo build -p scoop2_codegen_llvm` 通过。
- [ ] `scoop2c build/run` 可用：能编译、链接（含 libscooprt.a）、执行 Scoop 程序。
- [ ] run-pass fixture 基本全过（修正/新增的 fixture 在 PR 列明）。
- [ ] 无 `todo!`/`unimplemented!`/`unreachable!`/`panic!` 占位（`no_placeholder` 守卫 + review）。
- [ ] spec 全部合法语法成分可 codegen；非法程序明确报错（codegen 层 `CodegenError`）。
- [ ] explicit root frame GC 方案；未引入 statepoint/stackmap。
- [ ] 至此（除优化外）全套 pipeline 完整重写完成：parse→resolve→typecheck→MIR→LIR→LLVM codegen→链接→执行。

---

## 8. 实现进度（持续更新）

### 🎉 重大里程碑：端到端可执行（部分功能）

`scoop2c build/run` 已打通完整管线，**算术 / 控制流 / 比较类程序能正确编译、链接、执行**：
- `fun main(): Int { return 42 }` → 退出码 42 ✓
- `7 * 6`（intrinsic int_times）→ 退出码 42 ✓
- `if (x > 3) { return 100 }`（intrinsic int_gt + CondBr）→ 退出码 100 ✓

完整管线：parse → typecheck（sysroot bodies，`run_typecheck_with_options(lower_sysroot_bodies=true)`）→ MIR lower → materialize（含 devirtualize）→ LIR → LLVM codegen → object → clang 链接 libscooprt.a → 执行。

### 已完成

- **codegen 本体**（`scoop2_codegen_llvm`，~3000 行，11 单元测试全绿）：类型 lowering、函数体 lowering（Use/Const/Call/MakeTuple/StructLit/TupleIndex/IntEq/Assign/Panic + Return/Goto/CondBr/Unreachable）、Direct 调用 + intrinsic（int/bool/char/float 算子 + 比较运算符 lt/le/gt/ge/eq/ne）、runtime ABI 全量声明、string literal immortal 全局、**GC explicit root frame + slot 镜像（moving-GC 正确）**、**object 输出（合法 ELF/Mach-O）**、**entry main（runtime_init + 用户 main + exit code）**、no_placeholder 守卫。
- **驱动**（`scoop2c`）：新增 `build`/`run` 子命令；`build_lir_program` 共享 e2e 管线；`locate_libscooprt`（env + target + build OUT_DIR 三级回退）；clang 链接。
- **sysroot 精简重写**：保留语言必须（基础标量/Any/Hashable/ToString/String/Array/MutableArray/Option/Continuation/RuntimeError/panic/println/print/UIntPtr），移除全部辅助包（collections/delegates/lang.string/runtime.test/sync/thread/unsafe 的辅助内容）。
- **跨 crate 缺口修复**（用户要求"补齐缺口"）：
  - typecheck：`callee_not_callable` 按 FQN 查 extern、String `byteLength`/`getByte` @Intrinsic 成员、`__scoop_` resolve 重排、扩展函数 receiver `fqn_of_simple` 跨包解析、nominal-builtin 标量结构等价（`scalar_kinds_equal`，解决 "expected Bool got Bool" 跨 store TypeId 不稳定）、intrinsic 表扩充（int/uint/char/float 全算子 + float_to_int）。
  - typecheck→MIR：`ResolvedCall` 增 `inferred_type_args`；MIR 侧 `infer_type_args_from_call`（从实参类型推断泛型实参，使 `println<String>` 可单态化）。
  - MIR：运算符方法调用的 receiver 前置（Direct fallback / Method-Direct / devirtualize 三处），修复 `a*b` 丢失 lhs 的 bug。
  - codegen：冗余 unreachable block 的 Return 类型回退（零值）。

### 当前阻塞（println 等需 interface dispatch）

- `println("x")` 需要 `value.toString()` 的 **interface/virtual dispatch**（W1-6）。当前 codegen 仅实现 Direct 调用；Virtual/Interface/Closure/FunValue 未实现。且 LIR 中该调用的 dispatch metadata 损坏（`owner_fqn="scoop"`），需修 LIR map_rvalue 的 Virtual 元数据映射 + 实现 W1-2（vtable/itable 全局）+ W1-6（dispatch lowering）。

### 待办

1. **W1-6 Virtual/Interface dispatch**（解锁 println/toString/class 方法）：修 LIR Virtual metadata + vtable/itable 全局 + dispatch lowering。
2. **W1-4 剩余 rvalues**：MemberAccess(class)/EnumVariant/MakeArray/MakeClosure/WithUpdate/TopLevelRef/ClassLit/InterpolatedString。
3. **W1-2 全局**：type_desc+bitmap、vtable/itable、closure 布局。
4. **W1-5 access/cast/pattern**：class 字段 GEP+load+write barrier、is/as、模式匹配。
5. **W1-9 run-pass fixtures**（sysroot 精简后需审计：使用已移除辅助功能的 fixture 需调整）。
6. **W1-10 EffectStep lowering**、**W1-11 full run-pass + fixture 审计**。

### 注意：sysroot 精简对 fixture 的影响

精简 sysroot 后，使用已移除功能（@CLayout/atomic/GC/reflection/collections/progression 等）的 typecheck fixture 会失败（~64 个）。这是预期的（sysroot 有意精简）；这些 fixture 待辅助功能逐步加回后再恢复。
