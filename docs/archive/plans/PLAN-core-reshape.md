# Scoop：core / stdlib reshape 计划

> 生成时间：2026-05-17
> 设计基线：本文档
> 当前状态：待开始
> 行号说明：下文以当前文件路径和函数名为准；后续若行号漂移，优先按文件路径、符号名和 fixture 名定位。

## 0. 工作原则

- 当前 `scoop.core` 是"语言核心 + 一组编译器后门 + 一些非核心 helper"的混合体；本轮目标是把它收敛成"独立、自包含的标准 cone"，编译器不再需要按 FQN special-case 就能消费它。
- 当前 `stdlib/` 残缺、设计错误、与 sysroot 边界混乱（同 package、互相侵入）；本轮直接删除整个 `stdlib/`，未来重新设计的标准库不在本计划 scope 内。
- 仓库尚无已发布版本，**不保留任何前向兼容性**；MutableArray layout、runtime symbol、sysroot 声明都可以一次性改换。
- "intrinsic 是否保留"的判定标准：**编译器在该 callsite 是否生成了实质代码**。如果编译器只是生成"调一个 runtime symbol"的代码、别的什么都不做，那它就不应该是 intrinsic，而应当是 `@Extern(abi = "scoop")` 或 `@Extern(abi = "c")` 普通声明，由 ordinary call lowering 处理。
- runtime symbol 永远是单态的；"泛型导入"在 FFI 模型下不存在。需要泛型 surface 的 helper（例如 `MutableArray.new<T>`），由 sysroot 写普通 Scoop 泛型 wrapper，wrapper 内部调用反射 const fun 把 type info materialize 成 const args，再调单态 runtime symbol。这条路径不需要任何新 ABI 能力。
- core 自身禁止使用 f-string，避免 desugar 链路自指。
- 本轮触及大量 fixture；fixture 的迁移原则是"能跑就保留并改 import / 能合并就合并 / 不能跑就删"，不再为停留在旧 stdlib 上的测试维持兼容路径。

## 1. 当前判断

### 1.1 现 sysroot 的实际形态

- `sysroot/core.scoop`：包含语言核心 + 内联 atomic + GC pin/handle + 测试 helper（`__scoop_thread_spawn_join_resume*` / `__scoop_stackmap_statepoint_smoke` / `__scoop_gc_debug_*`）。其中"测试 helper"和"atomic"不属于核心语义。
- `sysroot/string.scoop`：String 高级 helper（`substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim`）+ 内部辅助函数。这些不是语言核心，应迁出 core。
- `sysroot/print.scoop`：泛型 `print<T>/println<T>` 的 body。属于 core（对应 builtin `ToString` 接口的最直接消费表面）。
- `sysroot/scalar_string_bridge.scoop`：scalar `toString` 的 audited bridge，把 `@Intrinsic("scalar_*_to_string_bridge")` 包装成 `scoopAbi*ToString` helper。这一层 bridge 在"intrinsic 转 scoop ABI"后整体多余。
- `sysroot/collections.scoop`：`Iterable/Iterator/IntIterable/IntIterator/Map`。`Iterable/Iterator` 属于 core（for-loop desugar 依赖）；`IntIterable/IntIterator` 是过渡 surface，可以删；`Map` 是 delegated property 示例的最小表面，归 `scoop.delegates`。
- `sysroot/delegates.scoop`：`ReadOnlyProperty/ReadWriteProperty/lazy/observable/vetoable`。本轮不动（待重设计），但其对 thread/sync 的依赖要解开：`lazy(Synchronized)/observable/vetoable` 当前依赖 per-property `Mutex`，core 重塑后这个依赖需要从 sysroot 边界拆出。
- `sysroot/thread.scoop` / `sysroot/sync.scoop` / `sysroot/unsafe.scoop`：本轮不重设计，但要让 core 不再引用其中的类型与 intrinsic。

### 1.2 现 stdlib 的实际形态

- `stdlib/prelude.scoop`：`require/check/let/run/also/apply` + `IntProgression.forEach` + `Int.rangeTo/downTo/until` + 几个内部 zero/one helper。
- `stdlib/mutable_array.scoop`：`push/pop/insert/removeAt/splice` —— 全部基于"用 `__scoop_array_builder_*` 重建一份"的 O(n) 单次操作模式。
- `stdlib/array_iter.scoop` / `stdlib/mutable_array_iter.scoop` / `stdlib/collections_iter.scoop`：迭代器 helper。
- `stdlib/collections_map.scoop` / `stdlib/collections_set.scoop`：基于"重建数组"模式的简易 Map/Set。
- `stdlib/math.scoop`：少量数学 helper。
- `stdlib/mutable_list.scoop`：MutableList 别名级 helper。

### 1.3 编译器后门

真正的"编译器后门"——即编译器在 lowering 时必须按 FQN/语法形式 special-case 的路径——目前有三处：

- **f-string codegen**（`crates/scoopc/src/llvm/codegen/main/literal.rs::codegen_interpolated_string`、`crates/scoopc/src/llvm/codegen/mir_body/string.rs`）：直接在 LLVM 阶段把 f-string 拼成连续 UTF-8 字节序列。
- **数组字面量 lowering**（`__scoop_array_builder_*` 路径）：HIR 阶段把 `[a, b, c]` 拆成 `builder_new + N 次 push + build`。
- **二元/一元 operator 直接 codegen**（`crates/scoopc/src/llvm/codegen/mir_body/op.rs`）：编译器在 codegen 阶段按 `ast::BinaryOp` / 等价的一元 op 直接 emit `build_int_add/sub/mul/sdiv/srem/shl/icmp/...` 等 LLVM 指令。这一路径完全绕开 sysroot 的 method-level intrinsic 机制——内建标量类型连最基础的算术运算都不在 sysroot 暴露 method 声明。

其他形如 `@Intrinsic("scalar_int_to_string_bridge")` 的声明虽然名字带 "intrinsic"，但实际只是"把 runtime symbol 名字告诉编译器"的常规 lowering，不是 special-case。

### 1.4 runtime layout 上的关键事实

- `runtime/c/scoop_array.c`：`ScoopArray` 是 inline trailing data 布局（`uint8_t data[]` 紧跟在 header 后），不能原地 grow；`ScoopArrayBuilder` 是 out-of-line + capacity 的独立类型，是真正支持 grow 的容器。
- `runtime/c/scoop_runtime.c`：`scoop_string_concat` / `scoop_*_to_string` / `scoop_print` / `scoop_println` / `scoop_string_unsafe_slice_bytes` 都是 ordinary managed call 形态的 runtime helper，参数/返回包含 GC ref，没有 native ABI 限制。
- `runtime/c/scoop_handle.c`、`scoop_pin.c`：handle/pin 涉及 caller `@NoGC` discipline + GC ref 进出，C ABI / Scoop ABI 都无法直接表达——必须保留 intrinsic。

## 2. 设计目标

1. `scoop.core` 收敛成"独立标准 cone"。除三类真 intrinsic（见 §3.3）外，core 内任何函数都通过普通的 Scoop 语法或 `@Extern(abi = "scoop")` 实现，**编译器不再需要按 FQN special-case 来消费 core**。
2. **f-string desugar 化**：`f"..."` 在 HIR lowering 阶段被改写为 `StringBuilder().add(...).add(...).toString()` 调用链。LLVM 阶段不再有 f-string 后门。
3. **数组字面量 desugar 化**：`[a, b, c]` 在 HIR lowering 阶段被改写为 `mutableArrayNew<T>(capacity = 3).push(a).push(b).push(c)` 调用链（具体形式见 §6.4）。`__scoop_array_builder_*` 整套从 sysroot/runtime/编译器三处全部删除。
4. **MutableArray 升级**：runtime layout 改为 out-of-line + capacity，支持 amortized O(1) push；构造函数接受可选 `capacity` 预分配；`Array<T>` 仍保留 inline 紧凑 layout（不可变）。
5. **StringBuilder 出现，最小表面**：`scoop.lang.string.StringBuilder` 只暴露 `add(s: String): StringBuilder` 与 `toString(): String`。纯 Scoop 实现，未来扩展不碰编译器。
6. **stdlib 整体删除**：`stdlib/` 目录及其所有 fixture 引用一次性清理；desugar 依赖（`IntProgression.forEach` 等）迁入 core。
7. **自动 prelude**：每个源文件自动 `import scoop.core.*` 与 `import scoop.lang.string.*`，用户代码不再需要显式写这两行。
8. **intrinsic 大幅瘦身**：sysroot 中纯包装 runtime symbol 的 `@Intrinsic("xxx")` 声明全部转为 `@Extern(abi = "scoop")` 或 `@Extern(abi = "c") @NoGC`；编译器 `intrinsics.rs` 中对应的 dispatch 项全部删除。
9. **二元/一元 operator method 化**：内建标量类型在 sysroot 暴露 `@Intrinsic("int_plus")` 等 method 级声明；编译器把 `a + b` / `-a` 等 operator 在 typecheck/HIR lowering 阶段重写为对应 method call，由 method-level intrinsic 表 lower 成 LLVM IR。删除 `mir_body/op.rs` 中按 `ast::BinaryOp` 的直接 codegen 路径。
10. **测试 helper 迁出 core**：`__scoop_thread_spawn_join_resume*` / `__scoop_stackmap_statepoint_smoke` / `__scoop_gc_debug_*` 迁到测试 cone 或直接删除（视 fixture 实际依赖）。
11. **core 真正成为 cone（去 sysroot 化）**：当前 sysroot file 相对用户 file 享有四类后门特权——可只声明不实现（`signature_only_sysroot_ast` AST stripping）、可被部分编译（`is_compilable_sysroot_file` 过滤）、自动开 `@AllowIntrinsic` gate、body 缺失策略放宽。reshape 主线完成后（P5/P7/P8 落地"body / @Intrinsic / @Extern 三选一"约束），上述 4 类中的前 3 类（声明不实现、部分编译、body 豁免）失去存在理由，全部退场；`@AllowIntrinsic` 自动开作为标准 cone 的便利特权保留（用户写应用代码无法也无需声明 intrinsic，gate 仅对 cone author 有意义）。`sysroot/` 目录按 cone FQN 重组为 `sysroot/scoop.core/` / `sysroot/scoop.lang.string/` / 等子目录形态，便于将来 release 打包与 `--sysroot` 参数定位。
12. **scoop.thread / scoop.sync / scoop.delegates 留待下一轮重设计**：本轮只切断 core 对它们的依赖；这三个 cone 自身的 surface 与实现暂不动。

## 3. Cone 划分与最终边界

### 3.1 Cone 划分总览

- **`scoop.core`**：语言核心。三类真 intrinsic + Scoop ABI helper + 普通 Scoop 代码。
- **`scoop.lang.string`**：StringBuilder + 三个 string-from-... runtime symbol（scoop ABI）+ 高级 String helper（substring/indexOf/contains/startsWith/endsWith/split/trim* 等）。
- **`scoop.unsafe`**：raw pointer + funptr + 内联 atomic（`__AtomicInt` 等从 core 迁入）。本轮只接收 atomic 迁移，其他不动。
- **`scoop.thread` / `scoop.sync` / `scoop.delegates` / `scoop.collections`**：本轮不重设计；只调整 core 对它们的引用。
- **删除**：整个 `stdlib/` 目录。

物理目录结构（P12 完成后）按 cone FQN 组织：

```
sysroot/
├── scoop.core/
│   ├── core.scoop          # 主类型 + 接口 + 反射 + GC
│   ├── string.scoop        # core 内部 String helper（byteLength/getByte 等的 sysroot side body）
│   ├── print.scoop         # 泛型 print<T>/println<T>
│   └── progression.scoop   # Int/Long/UInt/ULong Progression + rangeTo/downTo/until/forEach
├── scoop.lang.string/
│   ├── builder.scoop       # StringBuilder
│   └── helpers.scoop       # substring/indexOf/contains/startsWith/endsWith/split/trim*
├── scoop.unsafe/
│   └── unsafe.scoop        # raw pointer + funptr + 内联 atomic
├── scoop.thread/
├── scoop.sync/
├── scoop.delegates/
└── scoop.collections/
```

`Sysroot::default_path()` 的 caller 不再需要硬编码各个 `.scoop` 文件名——loader 已经递归扫描子目录，每个 file 的 `package` 声明决定其 cone 归属。将来添加 `--sysroot` 命令行参数时，参数指向的根目录与 `sysroot/` 同结构。

### 3.2 自动 prelude

每个源文件在 resolver/import 表构建阶段自动获得：

- `import scoop.core.*`
- `import scoop.lang.string.*`

用户显式写这两行 import 是允许的（等价、不报错、不 dedup 失败）。其他 cone（thread/sync/unsafe/delegates/collections）需要用户显式 import。

### 3.3 真 intrinsic 边界（三类，且仅这三类）

判定标准：**编译器在该 callsite 是否生成了实质代码**。

#### (a) inline 成 LLVM 指令 / 内存操作

编译器在 callsite 直接 emit GEP / load / store / arith / cast / atomic / 函数指针 indirect call 等 LLVM 指令，不经过外部 symbol。这一类的 sysroot 表面统一是 `@Intrinsic` 或 `@Intrinsic("name")` 标记的 method-level / 顶层 intrinsic 声明（无 body），编译器按 method-level intrinsic 表 lower 到具体 LLVM 指令。

集合操作：

- `Array<T>.size/get/__dataPtr`、`MutableArray<T>.size/get/set/__dataPtr`（按 layout 分流：Array inline、MutableArray indirect）
- `String.byteLength`、`String.getByte`（直接读 String header 字段 + GEP byte）

标量 operator method（按 Kotlin 命名约定，receiver 不同则同名 method 共存）：

- 整型（`Int` / `UInt` / `Int8/16/32/64` / `UInt8/16/32/64`）：
  - 算术：`plus/minus/times/div/rem`（lowering 按 receiver signed/unsigned 选 `add/sub/mul/sdiv/udiv/srem/urem`）
  - 一元：`unaryMinus`、`unaryPlus`、`inc`、`dec`
  - 位运算：`and/or/xor/inv`、`shl/shr/ushr`（`shr` lowering 成 `ashr`，`ushr` lowering 成 `lshr`；`shl` 同名）
  - 比较：`compareTo`（按 signed/unsigned 选 `icmp` predicate 后 select 三值）、`equals`（`icmp eq`）
- 浮点（`Float32/64`）：
  - 算术：`plus/minus/times/div/rem`（lowering 成 `fadd/fsub/fmul/fdiv/frem`）
  - 一元：`unaryMinus`（`fneg`）、`unaryPlus`
  - 比较：`compareTo`、`equals`（NaN 语义见 §9-P8 T8-1 baseline）
  - 数学 helper：`abs`、`isNaN`、`isInfinite`、`hash`
- 布尔：`and/or/xor/not`（非短路版本）。短路 `&&` / `||` 的短路语义由 HIR lowering 处理（生成 if-else 控制流），**不**走这些非短路 method。
- `Char`：`toInt`、`hash`、`compareTo`、`equals`、`plus(Int): Char`、`minus(Int): Char`、`minus(Char): Int`

原子操作：

- `__atomicIntLoad`、`__atomicIntStore`、`__atomicIntCompareExchange`

raw pointer ops（在 `scoop.unsafe`）：

- `Ptr<T>.load/store/cast/plus/minus`、`addressOf<T>`、`stackAlloc<T>`、`ptrToUIntPtr`、`uintPtrToPtr`、`addrOf/load/store` 兼容别名
- `FunPtr<F>.invoke(...)` 的全部 arity overload、`funPtrToUIntPtr`、`uintPtrToFunPtr`

#### (b) GC discipline 特殊待遇（GC ref 进出 + `@NoGC` caller）

C ABI 不允许 GC ref 穿越；scoop ABI 默认不隐含 `@NoGC`、且 `@Extern` 不允许显式叠加 `@NoGC`。这一类 helper 的语义本身就跨这两个 ABI 都无法表达，因此必须以 intrinsic 形态承载（caller side 不插 statepoint，callee 视为 `@NoGC`）。

- `GC.pin(obj: Any): Pinned`
- `GC.unpin(pinned: Pinned): Unit`
- `GC.handleNew(obj: Any): GcHandle`
- `GC.handleGet(h: GcHandle): Any`

#### (c) compile-time eval（`const fun` 反射）

不是 runtime call，而是编译期被 evaluate 成 const 值嵌入到调用方 IR 中。

- 现有：`fieldsOf<T>` / `variantsOf<T>` / `paramsOf` / `nameOf<T>` / `sizeOf<T>` / `alignOf<T>` / `superTypesOf<T>` / `annotationsOf<T>` / `getPlatform()`
- **新增**：`kindOf<T>(): Int`（element kind code: WORD=1 / REF=2 / COMPOSITE=3）、`descOf<T>(): UIntPtr`（composite transport descriptor 指针的擦除形态；非 composite 时返 0）

`kindOf/descOf` 是 §6 让 MutableArray 泛型 wrapper 能纯 Scoop 实现的关键反射 entry。

### 3.4 Scoop ABI helper（替代 intrinsic）

以下原本是 `@Intrinsic` 或 `@Intrinsic("xxx")` 的声明，本轮全部转为普通 `@Extern(abi = "scoop")` 顶层函数声明。caller 走 ordinary managed call 框架，编译器不再为它们维护任何按 FQN 的 dispatch。

- `String.concat(other: String): String` → `@Extern(name = "scoop_string_concat", abi = "scoop") fun __scoop_string_concat(a: String, b: String): String`
- `String.unsafeSliceBytes(offset: Int, len: Int): String` → 同形式
- 标量 toString 桥：`scoop_int_to_string` / `scoop_char_to_string` / `scoop_bool_to_string` / `scoop_float32_to_string` / `scoop_float64_to_string`
- `print/println` 的 runtime 入口：`scoop_print` / `scoop_println`（替代当前 `__scoop_print_string` / `__scoop_println_string`）
- 三个新 string-from-... 入口（在 `scoop.lang.string`）：`scoop_string_from_byte_array` / `scoop_string_from_char_array` / `scoop_string_from_string_array`
- MutableArray 单态入口（在 `scoop.core`）：`scoop_mutable_array_new` / `scoop_mutable_array_push_word` / `scoop_mutable_array_push_ref` / `scoop_mutable_array_push_composite`（具体见 §6.3）
- `panic(message: String): Nothing` → `@Extern(name = "scoop_panic", abi = "scoop") fun panic(message: String): Nothing`
- `scoop_gc_collect`（如果保留这个公共 surface）

转换后，sysroot 中 `scalar_string_bridge.scoop` 的 audited bridge 一层（`__scoop_runtime_*_to_string_bridge` + `scoopAbi*ToString` wrapper）整体删除——`Int.toString()` body 直接调 `scoop_int_to_string`。

### 3.5 C ABI + `@NoGC` 的窄路径

无 GC ref 进出、需要 caller `@NoGC` discipline 的 substrate helper。`@Extern(abi = "c")` 隐含 `@NoGC` + `@Unsafe`。

- `GC.handleDrop(h: GcHandle): Unit`（无 GC ref 进出）
- 测试调试 helper（如 `__scoop_gc_debug_*`），如保留则迁此处

## 4. `scoop.core` 的最终内容

### 4.1 保留

类型 / 接口：

- 标量类型：`Bool` / `Char` / `Int` / `UInt` / `Float32` / `Float64` / `Int8/16/32/64` / `UInt8/16/32/64` + 别名 `Byte/Short/Long/UShort/ULong/UIntPtr/Double`
- `String`（intrinsic class，含 `byteLength/getByte` intrinsic + `length/toInt/concat/hash/isEmpty/replace/charAt/repeat/compareTo/trimIndent` 普通 Scoop method body）
- `Array<T>` / `MutableArray<T>`（layout 见 §6.1）
- `Unit` / `Nothing` / `Any`
- `ComptimeList<T>`（反射返回容器）
- 接口：`Hashable` / `ToString` / `Iterable<T>` / `Iterator<T>`
- 内置注解：`@Inline` / `@Suppress` / `@Deprecated` / `@CLayout` / `@Target` / `@Retention` / `@TailRec` / `@AllowIntrinsic` / `@Experimental` / `@ThreadLocal` / `@Global` + `AnnotationTarget` enum
- 元数据 struct：`TypeMeta` / `FieldMeta` / `VariantMeta` / `ParamMeta` / `PropertyMeta` / `FunctionMeta` / `AnnotationMeta` / `AnnotationArgMeta` / `TypeKind` / `Platform`

效应 / 错误：

- `effect Raise<in E>` + `RuntimeError` enum（`NullAssertionFailed` / `ClassCastFailed` / `ContinuationAlreadyResumed`）
- `Continuation<Resume, Answer, eff E = Pure>`（声明保留；任何 thread-side helper 删除）

GC：

- `GcHandle(val raw: UIntPtr)` / `Pinned(val value: Any)` 声明
- `object GC`：`pin/unpin/handleNew/handleGet`（intrinsic，§3.3 (b)）+ `handleDrop`（C ABI，§3.5）

print：

- 泛型 `fun <T> print(value: T) where T: ToString` / `fun <T> println(value: T) where T: ToString`，body 调 scoop ABI `scoop_print` / `scoop_println`

desugar 依赖（从 stdlib 迁入）：

- `IntProgression(first, last, step, increasing)` 结构（已在 core）
- `Int.rangeTo` / `Int.downTo` / `Int.until` / `IntProgression.forEach`（从 `stdlib/prelude.scoop` 迁入；后续按 §6.5 扩展到其他整数类型）

panic：

- `fun panic(message: String): Nothing`（scoop ABI `scoop_panic`，见 §3.4）

反射 const fun：见 §3.3 (c)

MutableArray 泛型 surface：见 §6

### 4.2 删除

- `__AtomicInt` 别名 + `__atomicIntLoad/Store/CompareExchange` 声明 → 迁 `scoop.unsafe`
- `__scoop_thread_spawn_join_resume` / `__scoop_thread_spawn_join_resume_u64` → 删除
- `__scoop_stackmap_statepoint_smoke` → 迁测试 cone 或删除
- `__scoop_gc_debug_alloc_garbage` / `__scoop_gc_debug_heap_object_count` → 迁测试 cone 或保留为 `@Extern(abi = "c")`
- `__scoop_print_string` / `__scoop_println_string` → 删除（被 `scoop_print` / `scoop_println` 取代）
- `__scoop_runtime_string_concat_bridge` + `__scoop_runtime_*_to_string_bridge` 五个 audited bridge → 删除（直接调 runtime symbol）
- `scoop/sysroot/scalar_string_bridge.scoop` 整个文件 → 删除
- `IntIterable` / `IntIterator` interface（在 `scoop/sysroot/collections.scoop`）→ 删除（普通 `Iterable<Int>` / `Iterator<Int>` 替代）
- `IntIterable.toArray()` intrinsic → 删除（用 `Iterator<T>.toArray<T>()` 替代，纯 Scoop 实现）

## 5. `scoop.lang.string` 的内容

新建 cone，目录约定 `sysroot/lang_string.scoop` 或 `sysroot/lang/string.scoop`（具体见 §9-P5）。

### 5.1 三个 string-from-... 入口（scoop ABI）

```scoop
@Extern(name = "scoop_string_from_byte_array", abi = "scoop")
@Unsafe
fun __scoop_string_from_byte_array(bytes: MutableArray<Byte>): String

@Extern(name = "scoop_string_from_char_array", abi = "scoop")
fun __scoop_string_from_char_array(chars: MutableArray<Char>): String

@Extern(name = "scoop_string_from_string_array", abi = "scoop")
fun __scoop_string_from_string_array(parts: MutableArray<String>): String
```

- byte 版：unchecked，直接 memcpy 字节；标 `@Unsafe`，用户不可见到的形式（仅 sysroot/lang.string 内部使用）。
- char 版：runtime 内做 codepoint→UTF-8 变长编码；非法 codepoint 降级为 U+FFFD。
- string 版：runtime 内一次扫描求总长度 + memcpy 各 slice，单次分配。

### 5.2 StringBuilder

```scoop
class StringBuilder {
    private val parts: MutableArray<String> = mutableArrayNew<String>(capacity = SB_DEFAULT_CAPACITY)

    fun add(s: String): StringBuilder {
        this.parts.push(s)
        return this
    }

    fun toString(): String {
        return __scoop_string_from_string_array(this.parts)
    }
}
```

`SB_DEFAULT_CAPACITY` 取一个合理小值（建议 8）。**只暴露 `add` 与 `toString` 两个方法**——未来扩展功能时增加新方法即可，编译器无需任何感知。

### 5.3 高级 String helper（从 sysroot/string.scoop 迁入）

`substring` / `indexOf` / `contains` / `startsWith` / `endsWith` / `split` / `trimStart` / `trimEnd` / `trim`，连同它们的内部 helper（`__scoop_string_matches_at` 等）。

迁入时改造点：

- `String.split` 当前依赖 `__scoop_array_builder_push_string` / `__scoop_array_builder_build_array_string`；改为 `MutableArray<String>.push` + `MutableArray.toArray()` 路径。
- 所有 `byteLength/getByte/unsafeSliceBytes` 调用保持不变（这三个 still core/intrinsic surface）。

### 5.4 不放在这里的

- `String.concat` / `String.unsafeSliceBytes` / `String.length` / `String.toInt` / `String.hash` / `String.isEmpty` / `String.replace` / `String.charAt` / `String.repeat` / `String.compareTo` / `String.trimIndent`：留在 core 的 `@Intrinsic class String` body method 内（这些是基础操作，core 已经持有）。

## 6. MutableArray layout 升级

### 6.1 Runtime layout

`Array<T>` 不变（inline trailing data）。

`MutableArray<T>` 改为 out-of-line + capacity：

```c
typedef struct ScoopMutableArray {
  ScoopGcObjectHeader header;
  uint64_t len;
  uint64_t cap;
  uint64_t elem_size_bytes;
  uint64_t elem_align_bytes;
  const ScoopCompositeTransportDescriptor *elem_desc;
  uint8_t *data;
  uint32_t elem_kind;
  uint32_t _reserved_u32;
} ScoopMutableArray;
```

这与现 `ScoopArrayBuilder` 几乎一致——本质上是把 builder 类型晋升为 MutableArray 自身，删除独立的 builder 类型。

GC trace：visit `[0, len)` 个 ref 元素（如果 elem_kind == REF）或 composite 内嵌 ref（如果 elem_kind == COMPOSITE）。

`ScoopArray`（不可变 Array）保留 inline trailing data 布局；GC trace / data 访问规则不变。

### 6.2 编译器 intrinsic 分流

`array_size` / `array_get` / `array_set` / `array_data_ptr` 这四个 intrinsic name 按 receiver layout 分流：

- 当 receiver 是 `Array<T>`：从 inline trailing data 计算 GEP（路径与现有 lowering 一致）。
- 当 receiver 是 `MutableArray<T>`：先 load `data` 指针字段，再以 `data + idx*elem_size` 计算 GEP；`size` 读 `len` 字段；`__dataPtr` 直接返 `data` 字段。

`array_set` 仅对 MutableArray 有意义（Array 没有 set）；当 elem_kind == REF 时 emit GC write barrier。

### 6.3 MutableArray 泛型 surface（普通 Scoop + scoop ABI，不是 intrinsic）

```scoop
// runtime 单态入口
@Extern(name = "scoop_mutable_array_new", abi = "scoop")
fun __scoop_mutable_array_new(
    elemKind: Int,
    elemSize: Int,
    elemAlign: Int,
    elemDesc: UIntPtr,
    capacity: Int,
): MutableArray<Any>

// 三个 push 入口（按 element kind 分流）
@Extern(name = "scoop_mutable_array_push_word", abi = "scoop")
fun __scoop_mutable_array_push_word(arr: MutableArray<Any>, value: UIntPtr): Unit

@Extern(name = "scoop_mutable_array_push_ref", abi = "scoop")
fun __scoop_mutable_array_push_ref(arr: MutableArray<Any>, value: Any): Unit

@Extern(name = "scoop_mutable_array_push_composite", abi = "scoop")
fun __scoop_mutable_array_push_composite(arr: MutableArray<Any>, slot: UIntPtr, elemSize: Int): Unit
```

sysroot 普通 Scoop 泛型 wrapper：

```scoop
fun <T> mutableArrayNew(capacity: Int = 0): MutableArray<T> {
    val raw: MutableArray<Any> = __scoop_mutable_array_new(
        kindOf<T>(),
        sizeOf<T>(),
        alignOf<T>(),
        descOf<T>(),
        capacity,
    )
    return @Unsafe do { unsafeRefCast<MutableArray<Any>, MutableArray<T>>(raw) }
}

fun <T> MutableArray<T>.push(value: T): Unit {
    when (kindOf<T>()) {
        ARRAY_ELEM_KIND_WORD -> {
            val word: UIntPtr = @Unsafe do { unsafeReinterpretAsWord<T>(value) }
            __scoop_mutable_array_push_word(this.asAny(), word)
        }
        ARRAY_ELEM_KIND_REF -> {
            __scoop_mutable_array_push_ref(this.asAny(), value as Any)
        }
        ARRAY_ELEM_KIND_COMPOSITE -> {
            val slot: Ptr<T> = stackAlloc<T>()
            slot.store(value)
            __scoop_mutable_array_push_composite(
                this.asAny(),
                ptrToUIntPtr(slot),
                sizeOf<T>(),
            )
        }
    }
}
```

`unsafeRefCast` / `unsafeReinterpretAsWord` / `MutableArray<T>.asAny()` 这一组 unsafe primitive 是否需要新增（或者复用现有 `Ptr.cast` + `addressOf` 组合）—— 由 §9-P3 任务在实现时决定，原则是"能用现有 raw pointer ops 组合出来就不新增"。

`ARRAY_ELEM_KIND_*` 是 `kindOf<T>()` 返回的常量值，在 core 暴露为 `const val`。

### 6.4 数组字面量 desugar

HIR 阶段 `[a, b, c]: Array<T>` desugar 成：

```scoop
@Unsafe do {
    val tmp = mutableArrayNew<T>(capacity = 3)
    tmp.push(a)
    tmp.push(b)
    tmp.push(c)
    tmp.freeze()  // MutableArray<T> -> Array<T>，零拷贝（仅在不再修改时）
}
```

`MutableArray<T>` 字面量则不调 `freeze()`。

`freeze()` 的实现策略待 §9-P4 定：

- 选项 A：runtime helper `scoop_mutable_array_freeze` 单次拷贝到一个 inline `ScoopArray`（最简单，实现成本低）
- 选项 B：在 layout 上让 `Array<T>` 与 `MutableArray<T>` 共用同一物理布局，`freeze` 只是 type-level 的 erase（节省一次拷贝，但 Array 失去 inline 紧凑性）

倾向 A：`Array<T>` 紧凑性是高频读路径上的重要属性（cache locality + 一次 indirection 节省）。

### 6.5 IntProgression 多类型扩展

为 `Long` / `UInt` / `ULong` 等整数类型增加对应的 `Progression` struct + `rangeTo/downTo/until/forEach` overload。这一步纯 Scoop，无编译器改动。

## 7. f-string desugar

### 7.1 desugar 时机

HIR lowering 阶段（`crates/scoopc/src/hir/lower/expr/`），把 `InterpolatedString { parts: [Text, Expr, Text, Expr, ...] }` 改写为：

```scoop
StringBuilder()
    .add("text part 1")
    .add(<expr 1>.toString())
    .add("text part 2")
    .add(<expr 2>.toString())
    ...
    .toString()
```

要求：

- 每个 `<expr>` 必须 `: ToString`（typecheck 阶段验证；不满足时报当前现有的"interpolation expr must be ToString"诊断）
- 文本片段的转义解析（`{{` / `}}` / `\n` 等）保持现有 parser 阶段处理；desugar 拿到的是已解码的 `Text { content: String }`
- raw f-string（`f"""..."""`）的处理与普通 f-string 一致——仅文本片段的 escape 规则不同，desugar 形态相同

### 7.2 删除路径

完成 §7.1 后删除：

- `crates/scoopc/src/llvm/codegen/main/literal.rs::codegen_interpolated_string`
- `crates/scoopc/src/llvm/codegen/mir_body/string.rs` 的对应 interpolation 路径
- `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中仅 f-string 使用的 runtime symbol declaration（`scoop_bool_to_string` 仍由标量 `toString` 桥使用，不删；按使用方实际审查）

### 7.3 core 自身禁用 f-string

`scoop/sysroot/*.scoop` 与 `scoop.lang.string` 的源代码不允许使用 f-string 字面量——会形成 desugar → StringBuilder → core 自指依赖。检查方式：sysroot 文件 lint 或 typecheck 阶段一个 simple gate（"sysroot file 中出现 f-string"）。

## 8. 顺序总览

1. **P0**：冻结当前 baseline 与回归矩阵。
2. **P1**：自动 prelude 接入（`scoop.core.*` + `scoop.lang.string.*`，`scoop.lang.string` 暂时为空 cone）。零破坏面，最先做。
3. **P2**：反射 const fun 补全（`kindOf<T>` / `descOf<T>` + `ARRAY_ELEM_KIND_*` 常量）。
4. **P3**：MutableArray layout 升级（runtime + 编译器 intrinsic 分流 + scoop ABI new/push 入口 + sysroot 泛型 wrapper）。
5. **P4**：数组字面量 desugar 切换（HIR 阶段改走 `mutableArrayNew + push + freeze`）。`__scoop_array_builder_*` 从用户可见 surface 退出。
6. **P5**：新建 `scoop.lang.string` cone（三个 string-from-... 入口 + StringBuilder + 高级 String helper 迁移）。
7. **P6**：f-string desugar 切换。LLVM 后门删除。
8. **P7**：Intrinsic → scoop ABI 批量转换（`String.concat` / `String.unsafeSliceBytes` / 标量 toString / `print/println` / `panic` 等）。`scalar_string_bridge.scoop` 删除。
9. **P8**：算术 / 逻辑 operator method 化。在 sysroot 给标量类型暴露 `@Intrinsic("int_plus")` 等 method 级声明；HIR/typecheck 把 `a + b` / `-a` 等改写为对应 method call；method-level intrinsic 表实现 IR-direct lowering；删除 `mir_body/op.rs` 的按 `ast::BinaryOp` 直接 codegen 路径。
10. **P9**：删除 `stdlib/` 整个目录；`require/check/let/run/also/apply` 跟着删除（如果有 fixture 依赖，按"能跑就改 import 移到测试 fixture 内 / 否则删"原则处理）。
11. **P10**：core 中 thread/sync/atomic 引用清理。`__AtomicInt` 系列迁 `scoop.unsafe`；`__scoop_thread_spawn_join_resume*` 删除；`Continuation` 内部确认无 thread 依赖。
12. **P11**：测试 helper 迁移（`__scoop_gc_debug_*` 视实际 fixture 依赖迁 test cone 或留 C ABI）。
13. **P12**：core 真正成为 cone（去 sysroot 化）。审计 sysroot file 满足"body / @Intrinsic / @Extern 三选一"约束 → 物理目录按 cone FQN 重组（`sysroot/scoop.core/` 等）→ 取消 `signature_only_sysroot_ast` / `is_compilable_sysroot_file` 整套 → body 缺失策略统一 → `is_sysroot()` 收窄到仅用于 `@file:AllowIntrinsic` 自动开 gate（标准 cone 便利特权，保留）。
14. **P13**：spec 与文档更新（删 §10.3 `var StringBuilder.lastChar` 示例 / 写 StringBuilder 最小表面 / 写 `scoop.lang` 简介 / 更新 `MANAGED_ABI.md` 第 2.2 节"典型例子"列表 / 加入 sysroot 目录组织说明）。

依赖说明：

- P0 早于其他全部阶段——锁定基线。
- P1 早于 P5/P6——StringBuilder 与 desugar 出来的代码都要靠 prelude 自然引用 `scoop.lang.string` 名字。
- P2 早于 P3——MutableArray 泛型 wrapper 依赖 `kindOf/descOf`。
- P3 早于 P4——数组字面量 desugar 依赖 MutableArray.push 已经可用。
- P4 早于 P5——StringBuilder 内部用 `MutableArray<String>.push`，这条路径必须先打通。
- P5 早于 P6——f-string desugar 目标 `StringBuilder` 必须先存在。
- P6 早于 P9——P9 大量 fixture 含 f-string，desugar 路径必须先稳。
- P7 与 P3-P6 顺序无强依赖（可并行/穿插），但建议放在 P6 之后，避免转换期间多条路径同时变动。
- P8 与 P3-P7 顺序上独立——operator method 化跟 stdlib reshape 主线没有功能依赖；放在 P7 之后是因为 P7 与 P8 都属于 sysroot intrinsic 表面整理，连续做更不易分散注意力。但 P8 必须早于 P9 的 fixture 大批量迁移——operator 改写会触动**几乎所有**算术 fixture 的 IR snapshot，必须先稳定再清 stdlib。
- P9 早于 P10-P11——清理顺序：先把 stdlib 完全删掉，再清理 core 内部残留。
- P12 必须在 P10-P11 之后——审计 sysroot 全部 method/fun 已满足"body / @Intrinsic / @Extern 三选一"是 P12 取消 `signature_only_sysroot_ast` 的前置条件；在 atomic / 测试 helper 没迁完之前 core 还有"光声明"形态的 surface。
- P13 最后——前面所有改动稳定后一次性更新文档。

## 9. 分阶段计划

### P0. 冻结 baseline 与回归矩阵

参考：

- 当前 `tests/fixtures/run-pass/`、`tests/fixtures/typecheck/`、`tests/fixtures/llvm/` 全集
- 当前 `crates/scoopc/src/intrinsics.rs` 的 dispatch 表
- 现 sysroot 9 个文件、stdlib 9 个文件的全部内容
- `MANAGED_ABI.md` §2.2、§3、§5
- `SCOOP_FULL_SPEC.md` §8（String literals）、§10（Properties）

目标：

- 把"现在能跑通"的 fixture 集合写成一份白名单；后续每个 P 阶段完成后回放这份白名单。
- 列出所有 stdlib-dependent fixture（`tests/fixtures/run-pass/stdlib_*.scoop` 等），分类为"需要保留并改 import" / "可合并到其他 fixture" / "可删除"。
- 列出所有 f-string-dependent fixture，确认 P6 后回归测试覆盖面。

任务：

- T0-1：跑全量 fixture，记录当前 pass 集合（`target/fixture-scan-baseline-reshape.txt`）。
- T0-2：扫描 fixture 中的 import 语句，统计：
  - `import scoop.core.*` 显式出现位置
  - 其他 cone import 模式
  - 用到 stdlib 的 fixture 完整列表（按 `import scoop.core.array.*` / `import scoop.core.collections.*` 等模式 grep）
- T0-3：扫描 fixture 中的 f-string 出现位置，记录 desugar 测试覆盖面。
- T0-4：把 P0 输出的三个清单 commit 进 `docs/reshape-baseline/`。

### P1. 自动 prelude 接入

参考：

- `crates/scoopc/src/resolve/imports.rs` 的 import 表构建路径
- `crates/scoopc/src/resolve/mod.rs::resolve_paths` 阶段
- `sysroot/core.scoop` 当前实际的 export 列表

目标：

- 每个用户源文件在 import 表构建时自动注入 `scoop.core.*` 与 `scoop.lang.string.*` 两条 star import。
- 显式写这两行的源码不报"重复 import"错。
- sysroot 自身的源码不参与自动注入（避免自环）；它们继续显式写 `import scoop.core.*` 等。

任务：

- T1-1：在 `ImportTable::build` 中接入"对非 sysroot 文件自动追加两条 star import"。
- T1-2：决定 dedup 策略——倾向"自动注入路径与显式 import 视为等价；显式重复不报错也不重复展开"。
- T1-3：建一个空的 `scoop.lang.string` cone surface（一个空 file，仅 `package scoop.lang.string`），让 P1 之后该 import 可解析。
- T1-4：按 P0-T2 清单批量删除 fixture 中的 `import scoop.core.*` 行（可保留，由 dedup 策略决定）。
- T1-5：回归 P0 baseline。

### P2. 反射 const fun 补全

参考：

- `sysroot/core.scoop` 中现有反射 intrinsic 声明
- `crates/scoopc/src/intrinsics.rs` 中反射 dispatch
- `crates/scoopc/src/typecheck/expr/comptime/` 当前 const eval 路径
- `runtime/c/scoop_array.c` 中 `SCOOP_ARRAY_ELEM_KIND_WORD/REF/COMPOSITE` 常量定义

目标：

- 在 core 暴露：

```scoop
@Intrinsic
const fun <T> kindOf(): Int       // 返 1=WORD / 2=REF / 3=COMPOSITE

@Intrinsic
const fun <T> descOf(): UIntPtr   // 返 composite descriptor 指针；非 composite 返 0

const val ARRAY_ELEM_KIND_WORD: Int = 1
const val ARRAY_ELEM_KIND_REF: Int = 2
const val ARRAY_ELEM_KIND_COMPOSITE: Int = 3
```

任务：

- T2-1：编译器 const fun eval 阶段为 `kindOf<T>` 实现"按 T 的 layout / GC kind 分类返常量"。
- T2-2：为 `descOf<T>` 实现"非 composite 返 0；composite 返指向已 emit 的 transport descriptor 全局地址（lowering 成 `ptrtoint`）"。
- T2-3：sysroot 声明 + dispatch 接通。
- T2-4：写两个回归 fixture（typecheck + run-pass 各一个），分别覆盖 word / ref / composite 三种 element kind 上 `kindOf<T>` 的常量返回。

### P3. MutableArray layout 升级

参考：

- `runtime/c/scoop_array.c`（`ScoopArray` / `ScoopArrayBuilder` 两个类型）
- `runtime/c/scoop_runtime_api.h`（`scoop_array_*` 一组 X-macro 入口）
- `crates/scoopc/src/intrinsics.rs::ARRAY_*` 表
- `crates/scoopc/src/llvm/codegen/array.rs`（如果存在）或 array intrinsic lowering 实际入口

目标：

- runtime 的 `ScoopMutableArray` 类型按 §6.1 落地（out-of-line + cap）。
- 编译器 array intrinsic（`array_size/get/set/__dataPtr`）按 receiver 是 Array vs MutableArray 分流到两条 lowering 路径。
- runtime 实现三个 push 单态入口 + 一个 new 入口，按 §6.3 ABI 暴露给 sysroot。
- sysroot 提供 `mutableArrayNew<T>(capacity)` / `MutableArray<T>.push(v)` 普通 Scoop 泛型 wrapper。

任务：

- T3-1：runtime 端实现 `ScoopMutableArray` 与 `scoop_mutable_array_new/push_word/push_ref/push_composite`，包含倍数扩容（ratio = 2）+ GC write barrier（push_ref 路径）。
- T3-2：编译器端给 `array_size/get/set/__dataPtr` 加 receiver layout 分流。
- T3-3：sysroot `core.scoop` 加 `mutableArrayNew` / `MutableArray<T>.push` wrapper。
- T3-4：写一组小 fixture（不依赖任何字面量）验证 push amortized O(1)（构造 1024-elem MutableArray）。
- T3-5：回归 baseline——此时数组字面量仍走旧 builder，但 MutableArray.push 已可用。

### P4. 数组字面量 desugar 切换 + 删除 builder

参考：

- HIR 阶段当前的 array literal lowering（搜 `ArrayLiteral` / `__scoop_array_builder_*`）
- `runtime/c/scoop_array.c::scoop_array_builder_*`
- `runtime/c/scoop_runtime_api.h` 中 `scoop_array_builder_*` X-macro 行
- `stdlib/mutable_array.scoop`（其全部依赖 builder，将随 P8 一起清理）

目标：

- HIR 阶段把 `[a, b, c]: Array<T>` desugar 成 `mutableArrayNew<T>(capacity = 3).push(a).push(b).push(c).freeze()`；`MutableArray<T>` 字面量同形不调 `freeze`。
- 实现 `MutableArray<T>.freeze(): Array<T>` —— scoop ABI 调 `scoop_mutable_array_freeze` runtime helper，单次拷贝。
- 删除 `__scoop_array_builder_*` sysroot 声明、runtime 实现、编译器 lowering 路径。
- `sysroot/string.scoop::String.split` 中对 builder 的引用先临时切换到 `MutableArray<String>.push` 路径（在 P5 之前过渡）。

任务：

- T4-1：实现 `scoop_mutable_array_freeze` runtime helper + sysroot `MutableArray<T>.freeze()`。
- T4-2：HIR array literal lowering 改走新路径。
- T4-3：删除 builder runtime + sysroot + 编译器 lowering（一次性）。
- T4-4：临时迁移 `sysroot/string.scoop::String.split`（P5 时再随 string helper 一同搬到 `scoop.lang.string`）。
- T4-5：回归 baseline。

### P5. `scoop.lang.string` cone 建立

参考：

- §5 全部
- `sysroot/string.scoop` 当前内容
- `runtime/c/scoop_runtime.c` 中 `scoop_string_*` 一组 helper

目标：

- 新建 `sysroot/lang_string.scoop`（或 `sysroot/lang/string.scoop`，由实现时决定文件布局）；声明 `package scoop.lang.string`。
- 实现三个 string-from-... runtime helper（C 端） + 对应 scoop ABI 声明。
- 实现 `class StringBuilder`（按 §5.2，最小表面）。
- 把 `substring/indexOf/contains/startsWith/endsWith/split/trim*` 从 `sysroot/string.scoop` 搬过来。

任务：

- T5-1：runtime 端实现 `scoop_string_from_byte_array` / `scoop_string_from_char_array` / `scoop_string_from_string_array`。
- T5-2：sysroot 声明三个 scoop ABI 入口。
- T5-3：实现 `StringBuilder`。
- T5-4：迁移高级 String helper。
- T5-5：从 `sysroot/string.scoop` 删除已迁出的部分；该文件仅保留 core 内部 helper（`__scoop_string_*` 系列）+ core String body method 的依赖。
- T5-6：写 StringBuilder fixture（`tests/fixtures/run-pass/lang_string_builder_basic.scoop`），覆盖 add 链 + toString。
- T5-7：回归 baseline。

### P6. f-string desugar 切换

参考：

- §7
- `crates/scoopc/src/parser/expr.rs::find_interpolation_close_in_f_string`
- `crates/scoopc/src/ast/mod.rs` 中 `InterpolatedString` 表示
- `crates/scoopc/src/hir/mod.rs::InterpolatedStringPart`
- `crates/scoopc/src/llvm/codegen/main/literal.rs::codegen_interpolated_string`
- `crates/scoopc/src/llvm/codegen/mir_body/string.rs` 对应分支

目标：

- HIR lowering 阶段把 InterpolatedString 改写成 `StringBuilder` 调用链。
- 删除 LLVM 阶段的 f-string 后门。
- 在 sysroot 文件中加一个 lint：禁止 sysroot 源码使用 f-string。

任务：

- T6-1：HIR 阶段写 desugar 函数（输入 `parts: [Part]`，输出 `Expr`，类型为 `String`）。
- T6-2：typecheck 阶段保持现有"each expr part : ToString"诊断。
- T6-3：删除 LLVM 后门。
- T6-4：sysroot lint：扫描 sysroot 文件中是否含 f-string token，含则编译期报错。
- T6-5：跑 P0-T3 收集的 f-string fixture 全集；预期全过。
- T6-6：回归 baseline。

### P7. Intrinsic → scoop ABI 批量转换

参考：

- §3.4
- `sysroot/core.scoop` 中所有 `@Intrinsic` 与 `@Intrinsic("xxx")` 标记
- `sysroot/scalar_string_bridge.scoop` 整文件
- `sysroot/print.scoop` body
- `crates/scoopc/src/intrinsics.rs` dispatch 表
- `crates/scoopc/src/llvm/codegen/intrinsics/` 所有按 FQN 的 codegen 路径

转换列表（每条都是"删除 intrinsic 标记 → 改写为 `@Extern(abi = "scoop")` 顶层声明 → 删除 intrinsics.rs / codegen 中对应 FQN dispatch 项"）：

- `__scoop_runtime_string_concat_bridge` → 删（直接调 `scoop_string_concat`）
- 五个 `__scoop_runtime_*_to_string_bridge` → 删（直接调 `scoop_*_to_string`）
- `String` 类的 intrinsic body method（`length/toInt/concat/hash/isEmpty/replace/charAt/repeat/compareTo/trimIndent`）：body 内调 scoop ABI helper，method 自身不再是 intrinsic
- `__scoop_print_string` / `__scoop_println_string` → 改名 `scoop_print` / `scoop_println` 并改为 `@Extern(abi = "scoop")`
- `panic` → `@Extern(name = "scoop_panic", abi = "scoop")`
- `__scoop_gc_collect` → `@Extern(name = "scoop_gc_collect", abi = "scoop")`

任务：

- T7-1：批量改写 sysroot 声明。
- T7-2：删除 `sysroot/scalar_string_bridge.scoop`。
- T7-3：删除 `intrinsics.rs` 中对应 dispatch + `codegen/intrinsics/` 中按 FQN 的 lowering 路径。
- T7-4：必要时新增 runtime 端的 `scoop_print` / `scoop_println` 符号导出（如果当前命名是 `scoop_print_string` 等带 `_string` 后缀，runtime side 改名或加 alias）。
- T7-5：回归 baseline——这一步是大批量删代码，但都是包装层；若回归出问题，一般是 statepoint / root spill 的等价性问题，需对照 `MANAGED_ABI.md` §3.2 检查。

### P8. 算术 / 逻辑 operator method 化

参考：

- `crates/scoopc/src/llvm/codegen/mir_body/op.rs`（当前按 `ast::BinaryOp` 直接 emit LLVM 指令的整条 dispatch）
- `crates/scoopc/src/typecheck/expr/binary.rs`（如果存在；或当前 binary expr typecheck 入口）
- `crates/scoopc/src/hir/lower/expr/`（HIR 阶段是否已对 binary expr 做表示——决定 desugar 注入点）
- `crates/scoopc/src/intrinsics.rs` 中现有 method-level intrinsic 表（参考 `array_size`/`array_get` 的注册形态）
- `sysroot/core.scoop` 现有 `@Intrinsic struct Int` / `Float64` / `Char` 等 body method 的写法
- 前一轮 `docs/archive/plans/PLAN-managed-abi.md` P4 前置 II 落地的 method-level `@Intrinsic("name")` 表机制

目标：

- 在 sysroot 给所有内建标量类型暴露算术/比较/逻辑 method 的 `@Intrinsic("...")` 声明（无 body）：
  - 整型（`Int` / `UInt` / `Int8/16/32/64` / `UInt8/16/32/64`）：`plus/minus/times/div/rem`、`shl/shr`、`bitAnd/bitOr/bitXor/bitNot`、`unaryMinus`、`compareTo`、`equals`
  - 浮点（`Float32/64`）：`plus/minus/times/div`、`unaryMinus`、`compareTo`、`equals`
  - 布尔：`and/or/not`（非短路版本）
  - `Char`：`compareTo`、`equals`（`toInt/hash` 已存在）
- 编译器 method-level intrinsic 表新增对应 entry，每条 emit 对应 LLVM 指令（按 receiver 类型 signed/unsigned 决定 `sdiv/udiv` / `srem/urem` / `ashr/lshr` 等）。
- HIR/typecheck 阶段：`a + b` / `-a` / `a < b` / `a == b` / `!a` 这一组 surface lower 成对应 method call。短路 `&&` / `||` 的短路语义仍由 HIR lowering 处理（生成 if-else 控制流，**不**改写为 `Bool.and/or` 的非短路 method）。
- 删除 `mir_body/op.rs` 中按 `ast::BinaryOp` 直接 codegen 的整条路径。
- 不在本阶段改 `String.equals` / `String.compareTo` 等已经走 method body 的 ref-type method（它们已经是 sysroot body method + scoop ABI 路径，不属于 operator codegen 后门）。

任务：

- T8-1：扫描所有内建标量类型当前的算术/比较语义边界（signed/unsigned 行为、`Int.div(0)` 是否 trap、float NaN compare 语义、`Int.MIN_VALUE` 取负的 wrap 行为等），写成一份 "behavioral baseline" 短文，确保 method intrinsic lowering 与现有直接 codegen **逐位一致**。
- T8-2：扩展编译器的 method-level intrinsic 表。entry key 命名采用 `<receiver>_<method>` 形式（与 Kotlin method 名对齐）：
  - 整型：`int_plus` / `int_minus` / `int_times` / `int_div` / `int_rem` / `int_unary_minus` / `int_inc` / `int_dec` / `int_and` / `int_or` / `int_xor` / `int_inv` / `int_shl` / `int_shr` / `int_ushr` / `int_compare_to` / `int_equals`
  - 无符号整型：`uint_*` 一组（`uint_div/rem` lowering 成 `udiv/urem`；`uint_compare_to` 用 unsigned predicate）
  - 各定宽整型（`int8_*` / `int16_*` / `int32_*` / `int64_*` / `uint8_*` / ...）：与 `int_*` 同形，仅 LLVM 类型宽度不同
  - 浮点：`float64_plus / float64_minus / float64_times / float64_div / float64_rem / float64_unary_minus / float64_compare_to / float64_equals / float64_abs / float64_is_nan / float64_is_infinite / float64_hash`，`float32_*` 一组同形
  - 布尔：`bool_and / bool_or / bool_xor / bool_not`
  - Char：`char_to_int / char_hash / char_compare_to / char_equals / char_plus_int / char_minus_int / char_minus_char`

  每个 entry 的 lowering 直接产生对应 LLVM 指令。
- T8-3：在 sysroot `core.scoop` 给标量类型 body 加 method 声明。
- T8-4：HIR/typecheck 阶段改写 binary/unary operator 为 method call。短路 `&&` / `||` 保持现有 if-else lowering，**不**走 method 路径。
- T8-5：删除 `mir_body/op.rs` 中按 `ast::BinaryOp` 的直接 codegen 路径。该文件可能完全清空或仅保留少数辅助 helper。
- T8-6：写一组算术 fixture 矩阵（每种类型 × 每种 op），覆盖：正常值、边界值（`Int.MIN_VALUE` 取负、divide by zero、NaN 比较等）。
- T8-7：跑 P0-T1 baseline。**预期**：算术相关 fixture 的 IR snapshot 大量变化（call instruction 包装一层），但运行结果一致。如出现行为差异，T8-1 的 baseline 短文是仲裁依据。

依赖：依赖前一轮 PLAN-managed-abi 已落地的"内建类型作为一等 struct/class implementer + method-level intrinsic 表"机制（即 P4 前置 I/II）。本轮不需要再扩展该机制，只用它。

### P9. 删除 `stdlib/`

参考：

- `stdlib/` 全部 9 个文件
- `tests/fixtures/run-pass/stdlib_*.scoop`、`tests/fixtures/run-pass/*` 中其他对 `scoop.core.array.*` / `scoop.core.collections.*` 等的 import
- `crates/scoopc/src/frontend.rs::default_stdlib_path` 与 stdlib 注入路径

目标：

- 删除整个 `stdlib/` 目录。
- 删除 driver 中"自动注入 stdlib"的代码路径。
- desugar 依赖（`IntProgression.forEach` / `Int.rangeTo/downTo/until` 等）从 `stdlib/prelude.scoop` 迁入 `sysroot/core.scoop`（或独立 `sysroot/progression.scoop` 文件，按 sysroot 现有文件粒度决定）。
- `require/check/let/run/also/apply` 跟着删除。
- fixture 按 P0-T2 清单批量处理：能改 import 的改、能合并的合并、跟着 stdlib 一起死的直接删。

任务：

- T9-1：迁移 desugar 依赖到 core。
- T9-2：批量改写 fixture import / 合并 / 删除。
- T9-3：删除 `stdlib/` 目录与 frontend 注入路径。
- T9-4：回归 baseline——预期 P0 baseline 中的 `stdlib_*` fixture 被替换为对应的新 fixture 或删除项。

### P10. core 中 thread/sync/atomic 引用清理

参考：

- §4.2
- `sysroot/core.scoop` 中 `__AtomicInt` 系列 + 测试 helper
- `sysroot/unsafe.scoop`（迁入 atomic 的目的地）
- `sysroot/delegates.scoop`（其 `lazy(Synchronized)/observable/vetoable` 对 thread/sync 的依赖）

目标：

- `__AtomicInt` 别名 + 三个原子操作 intrinsic 声明从 `sysroot/core.scoop` 迁到 `sysroot/unsafe.scoop`。
- 删除 `__scoop_thread_spawn_join_resume*`。
- 验证 `Continuation<R, A, eff E>` 在 core 中无任何 thread 类型/helper 的依赖；如有遗漏一并清。
- `sysroot/delegates.scoop` 中 `lazy(Synchronized)/observable/vetoable` 的 thread/sync 依赖原样保留（`scoop.delegates` 留待下一轮重设计）；但要确保 core 不再隐式依赖 `scoop.sync`。

任务：

- T10-1：迁 atomic。
- T10-2：删 thread test helper。
- T10-3：grep 验证 core / lang.string 内无任何 `scoop.thread` / `scoop.sync` 名字。
- T10-4：回归 baseline。

### P11. 测试 helper 迁移

参考：

- `__scoop_stackmap_statepoint_smoke` / `__scoop_gc_debug_*` 当前使用方
- `tests/fixtures/runtime_gc/*` / 反映 GC 行为的 fixture

目标：

- 这些 helper 从 core 移出。如果仅用于 fixture，迁到一个 test-only cone（例如 `scoop.runtime.test`）；如果完全无用直接删。
- `scoop.runtime.test` 不进入自动 prelude；fixture 显式 import。

任务：

- T11-1：审查每个 helper 的实际使用方，分类。
- T11-2：实施迁移或删除。
- T11-3：回归 baseline。

### P12. core 真正成为 cone（去 sysroot 化）

参考：

- §0 工作原则中"core 不再是后门"的精神
- `crates/scoopc/src/sysroot/mod.rs`：`Sysroot::load_from`、`signature_only_sysroot_ast`、`is_compilable_sysroot_file`、`strip_item_bodies` / `strip_type_member_bodies` 一组
- `crates/scoopc/src/source.rs::SourceFile::{load_sysroot, is_sysroot}`
- `crates/scoopc/src/typecheck/builtin_annotations.rs::file_allows_intrinsic`（line 135 附近）
- `crates/scoopc/src/typecheck/annotations.rs::source_is_sysroot`（line 3192）+ body 缺失策略 line 2280

目标：

- 取消 sysroot file 相对用户 file 的所有"语义层后门特权"，**仅保留** `@file:AllowIntrinsic` 在标准 cone 文件中的自动开启（这是便利特权，不是语义后门——用户写应用代码无法也无需声明 intrinsic，gate 仅对 cone author 有意义）。
- 物理目录按 cone FQN 重组为 `sysroot/scoop.core/` / `sysroot/scoop.lang.string/` / `sysroot/scoop.unsafe/` / `sysroot/scoop.thread/` / `sysroot/scoop.sync/` / `sysroot/scoop.delegates/` / `sysroot/scoop.collections/` 子目录形态，便于将来 release 打包与 `--sysroot` 参数定位。
- 验证：reshape 主线（P5/P7/P8）完成后，core / lang.string / unsafe 等所有 sysroot file 中每个 method/fun 都满足 "body / @Intrinsic / @Extern 三选一"——届时 `signature_only_sysroot_ast` AST stripping 没有用户依赖，可以拆掉。

任务：

- T12-1（审计）：扫描 sysroot 全部 file，确认每个 method/fun 都有 body / `@Intrinsic` / `@Extern` 三类之一。如有"光声明无 body 也无 `@Intrinsic`/`@Extern`"的 surface，回到对应 P 阶段任务（P5/P7/P8 等）补完。
- T12-2（目录重组）：把 `sysroot/*.scoop` 按文件的 `package` 声明搬到对应子目录：
  - `sysroot/scoop.core/`：core.scoop（主类型）+ string.scoop（内部 helper）+ print.scoop + progression.scoop
  - `sysroot/scoop.lang.string/`：builder.scoop + helpers.scoop
  - `sysroot/scoop.unsafe/`：unsafe.scoop
  - `sysroot/scoop.thread/`：thread.scoop
  - `sysroot/scoop.sync/`：sync.scoop
  - `sysroot/scoop.delegates/`：delegates.scoop
  - `sysroot/scoop.collections/`：collections.scoop（如果还保留）
  loader 已经递归扫描子目录（`crates/scoopc/src/sysroot/mod.rs::collect_scoop_files` line 339-356），无需改 loader。
- T12-3（取消 `signature_only_sysroot_ast`）：删除 `signature_only_sysroot_ast` / `strip_item_bodies` / `strip_type_member_bodies` / `strip_object_decl_bodies` / `strip_type_decl_bodies` / `strip_comptime_else_bodies` 整套；删除 `is_compilable_sysroot_file` / `is_always_compilable_sysroot_file` 过滤——所有 sysroot file 全编译，与用户 file 一致。
- T12-4（body 缺失策略统一）：删除 `crates/scoopc/src/typecheck/annotations.rs` line 2280 附近"sysroot file 不要求 body" 的特殊豁免——sysroot file 与用户 file 用同一规则（body / @Intrinsic / @Extern 三选一）。
- T12-5（`is_sysroot()` 语义收窄 + 命名澄清）：`SourceFile::is_sysroot()` 仅在 `@file:AllowIntrinsic` 自动开 gate 处保留（参考 `typecheck/builtin_annotations.rs::file_allows_intrinsic`、`typecheck/annotations.rs::source_is_sysroot` line 3180）。把所有其他位置的 `is_sysroot()` 检查删除或移除。完成后 grep `is_sysroot\(\)` 应只在 `builtin_annotations.rs` / `annotations.rs` 两处保留。

依赖说明：

- T12-1 必须在 P11 完成后才能开工——P10/P11 完成才意味着 core / lang.string / unsafe 等中的"非语言核心 surface（atomic、测试 helper）"已迁出，剩下的全是真正的 cone 内容，才好做"三选一"审计。
- T12-2 ~ T12-4 严格按顺序：先重组目录（T12-2）；然后才能拆"光声明" surface（T12-3）；最后调整 body 策略（T12-4）。
- T12-5 与 T12-3/T12-4 可重叠—— T12-3/T12-4 删除"sysroot 特殊待遇"代码路径时自然会减少 `is_sysroot()` 的调用方；T12-5 是对剩下的检查做最终收窄。

### P13. spec 与文档更新

参考：

- `SCOOP_FULL_SPEC.md` §8（String literals）、§10.3（Extension Properties）
- `MANAGED_ABI.md` §2.2（"典型例子"列表）
- `SCOOP_RUNTIME.md`（如涉及 array layout 章节）

任务：

- T13-1：删除 spec §10.3 `var StringBuilder.lastChar` 示例。
- T13-2：在 spec 中加入"`scoop.lang` 简介"小节，说明 `scoop.lang.string` 与 StringBuilder 最小表面 + sysroot 目录组织约定（`sysroot/<cone-fqn>/` 形态 + 将来 `--sysroot` 参数路径）。
- T13-3：更新 `MANAGED_ABI.md` §2.2 的 "典型例子" 列表（标注哪些已经成为 scoop ABI helper）。
- T13-4：如果 array layout 章节存在于 `SCOOP_RUNTIME.md` 或独立文档，更新 MutableArray 部分。
- T13-5：清理 sysroot 文件中过期的 TODO 注释（很多 `T0143`/`T1317`/`T1325` 等历史工单引用，本轮重塑后大多失效）。

## 10. 风险

- **MutableArray layout 改动 + GC trace**：runtime 端 layout 是 GC trace 的输入。改动期间稍有疏忽会导致 GC 漏扫描。需要在 P3 单独跑一组 stress fixture（构造大量含 ref 元素的 MutableArray + 强制 GC）。
- **数组字面量 desugar 性能**：当前 builder 路径在 codegen 阶段已有一定优化（直接 emit 内联 push）；新路径走 `mutableArrayNew + N 次 push` 可能在小数组（N ≤ 4）场景下生成更冗余的 IR。如出现 fixture IR snapshot 大幅膨胀，考虑给 desugar 加一个"小数组 inline emit"的特例（仍是 desugar，不是后门，只是 desugar 形态分支）。
- **f-string desugar 与 `expr.toString()` 的可见性**：desugar 出来的 `expr.toString()` 调用必须能解析到 ToString member。当前 typecheck 对"member call to default trait method"在某些 edge case 下还有问题（参考 sysroot 中 `Hashable.hash` 默认实现的注释）。如出现"interpolation expr 解析失败"，可能要先补 typecheck 的 member call 路径，再切 f-string desugar。
- **fixture 大批量迁移的回归审查**：P8 一次性删 stdlib + 改大量 fixture，回归矩阵会出现很多"该 fixture 删除"的项。需要先对 P0-T2 清单做明确的"保留/合并/删除"分类，避免误删。
- **`scoop.delegates` 的 thread/sync 残留依赖**：`lazy(Synchronized)/observable/vetoable` 当前要 per-property `Mutex`，本轮不重设计 delegates。如果 core 重塑过程中有 typecheck/lowering 路径间接拽进 thread/sync，需要在 P9 单独追查。
- **runtime symbol 改名兼容**：当前有 `scoop_print_string` 这种带 `_string` 后缀的 runtime 符号；迁移到 scoop ABI 后建议改名为 `scoop_print`（与 `scoop_println` 对称），但要同时改 runtime side export 与 sysroot 端 `@Extern(name = ...)`，以及 `scoop_runtime_api.h` 的 X-macro 列表。漏改其一会链接错误。
- **测试 helper 删除影响**：`__scoop_stackmap_statepoint_smoke` 当前由 explicit-root-frame 主线 fixture 依赖，删除前必须确认是否有不可替代的 GC 端到端 smoke 没有别的覆盖。
- **`scoop.lang.string` 与 prelude 自指**：StringBuilder 定义在 `scoop.lang.string`，自动 prelude 又把 `scoop.lang.string.*` 注入到 *所有* 用户文件——包括 `scoop.lang.string` 自己的源文件（不算 sysroot）。要确认 resolver 处理"package 自身的 star import"是幂等的；这个边界 case 在 P1 时要专门测一次。
- **operator method 化的语义保真**：当前 `mir_body/op.rs` 的直接 codegen 在某些边界值上有微妙行为（`Int.MIN_VALUE` 取负的 wrap、`-0.0` 与 `+0.0` 的 `equals` 区分、NaN 比较的传播规则、shr 在 signed/unsigned 上的 ashr/lshr 选择、shift amount 等于或超过类型宽度时的 LLVM UB 行为等）。新 method intrinsic 的 lowering 必须**逐位一致**，否则会成为隐性回归。P8-T1 的 behavioral baseline 是仲裁依据，**不允许**在 method 化过程中"顺便修一下"任何已有行为；任何行为变更必须以独立 PR 形式单独决策。
- **operator method 化对 fixture IR snapshot 的冲击面**：几乎所有含算术的 fixture 在 P8 后 IR snapshot 都会变化（`add i64 ...` 之外多一层 `call @int_plus(...)`，再被 inliner 消化）。P0-T1 baseline 收集后必须区分"运行结果回归"与"IR 形态回归"两类——P8 期间允许后者大量变化，但前者必须为零。
- **P12 去 sysroot 化的隐性依赖**：取消 `signature_only_sysroot_ast` AST stripping 之前，sysroot file 中**任何**没有 body 也没有 `@Intrinsic` / `@Extern` 标记的 method/fun 都会让 P12-T03 编译失败。这种"光声明"surface 在 P5/P7/P8 的 sysroot 修改中很容易意外残留（例如把一个 method 从 `@Intrinsic` 改成普通 method 时漏了写 body）。P12-T01 审计是这一类问题的兜底；但更稳的做法是 P5/P7/P8 各任务在完成时各自跑一遍 grep 确认无"光声明"surface 残留——把 P12-T01 的审计成本前置分摊。
- **目录重组对 fixture / 测试中路径断言的冲击**：`tests/fixtures/`、`crates/scoopc/src/llvm/tests/`、`crates/scoopc/src/sysroot/mod.rs::tests` 等位置可能存在对 sysroot 文件路径的硬编码（如 `sysroot/core.scoop`）。P12-T02 重组目录时这些断言会失效，需要在子任务内一并修。grep 范围要覆盖整仓 `sysroot/.*\.scoop`、`sysroot/core`、`sysroot/string` 等模式。
