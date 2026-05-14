# Scoop 语言规范 第 6 部分：unsafe、FFI、GC 互操作与低层边界

版本：0.1 草案

本部分定义 unsafe context、`@NoGC`、`@Extern`、C layout、raw pointer、function pointer、GC pinning、stable GC handle、顶层可变全局和外部 ABI 边界。运行时具体符号名、GC 算法和 C runtime 内部 ABI 不属于语言规范。

## 1. 低层边界总览

Scoop 默认提供内存安全语言表面。以下能力需要显式 opt in：

- Raw pointer 与手动内存读写。
- 外部 ABI 调用与符号访问。
- 可变全局状态。
- GC pinning。
- Stable GC handle。
- C-compatible struct layout。
- Allocation-free / NoGC 代码。

这些能力主要用于 runtime、FFI、系统编程和库底层实现。普通应用代码应通过安全封装使用。

## 2. Unsafe context

Unsafe context 有两种：

```kotlin
@Unsafe
fun lowLevel() {
    ...
}

fun wrapper() {
    @Unsafe do {
        ...
    }
}
```

规则：

- `@Unsafe` 函数体是 unsafe context。
- `@Unsafe do { ... }` 创建局部 unsafe context。
- Unsafe primitive 只能在 unsafe context 中使用。
- 调用 `@Unsafe` 函数只能在 unsafe context 中进行。
- 调用 `@Extern(abi = "c")` 函数以及访问 `@Extern` 顶层变量只能在 unsafe context 中进行。
- `@Unsafe` 不暗示 `@NoGC`。

局部 unsafe block 必须写 `@Unsafe do { ... }`。由于裸 `{ ... }` 是 closure literal，`@Unsafe { ... }` 不是局部 unsafe block，必须拒绝；若需要注解 closure，应按 closure 注解规则处理。

## 3. Safe region

`@Safe` 在 unsafe context 内重新建立 safe 区域：

```kotlin
@Unsafe
fun f() {
    rawOperation()

    @Safe do {
        // unsafe primitive 禁止
    }
}
```

规则：

- `@Safe` 函数体、`@Safe do { ... }` block 和 `@Safe { ... }` closure 按非 unsafe context 检查。
- 在 `@Safe` 区域内调用 `@Extern(abi = "c")`、`@Unsafe` 函数、访问 `@Extern` 顶层变量或使用 raw pointer primitive 是编译错误。
- 可在 `@Safe` 内嵌套新的 `@Unsafe do { ... }` 局部重新开启 unsafe。
- `@Safe do { ... }` 是局部 block。
- `@Safe { ... }` 是 annotated closure，不是局部 block。

## 4. `@NoGC`

`@NoGC` 标记函数在执行自身和静态允许 callees 时不发生 GC-managed heap allocation：

```kotlin
@NoGC
@Unsafe
fun bumpAlloc(alloc: Ptr<Byte>, size: Int): Ptr<Byte> {
    return alloc + size
}
```

调用限制：

- `@NoGC` 函数只能调用其它 `@NoGC` 函数。
- `@NoGC` 函数可调用 `@Extern(abi = "c")` 函数，因为它们在调用图约束上隐式视为 `@NoGC` leaf。
- `@Extern(abi = "scoop")` 按 ordinary managed call 处理，不能因为带 `@Extern` 就在 `@NoGC` 中放行。
- 调用 `@Extern(abi = "c")` 仍需要 unsafe context。
- 调用普通可能分配的 Scoop 函数是编译错误。

分配限制：

编译器必须拒绝 `@NoGC` 函数中的可能 GC 分配，包括但不限于：

- 构造 class/object 实例。
- 把值类型装箱到接口或 `Any`。
- 创建逃逸 continuation 或 GC 分配的 state machine。
- 创建需要堆分配的 capturing closure。
- 创建 GC-managed array 或其它堆对象。
- 调用不能证明为 `@NoGC` 的函数。

`@NoGC` 是编译器检查保证。如果编译器无法证明无分配，必须报错。

`@NoGC` 不保证其它线程不会触发 GC，也不禁止显式 runtime GC；它只约束本函数路径不分配 GC-managed heap。

## 5. Raw pointer

Raw pointer 类型：

```kotlin
@Intrinsic
struct Ptr<T> {
    @NoGC @Unsafe fun <U> cast(): Ptr<U>
    @NoGC @Unsafe fun load(): T
    @NoGC @Unsafe fun store(value: T)
    @NoGC @Unsafe operator fun plus(offset: Int): Ptr<T>
    @NoGC @Unsafe operator fun minus(offset: Int): Ptr<T>
}
```

相关 intrinsic：

```kotlin
@Intrinsic @NoGC @Unsafe
fun <T> addressOf(var: T): Ptr<T>

@Intrinsic @NoGC @Unsafe
fun <T> stackAlloc(): Ptr<T>
```

规则：

- `Ptr<T>` 只能在 `T` 是 GC-free value type 时良构。
- `T` 不能直接或间接包含 GC-managed 引用。
- `Ptr<T>` 的指针算术以 `T` 为单位，不以字节为单位。
- `load` / `store` 要求调用者保证地址有效、对齐、生命周期和别名规则。
- `addressOf(var: T)` 要求实参是可取地址变量 slot。
- `stackAlloc<T>()` 只允许 GC-free `T`。
- 对 GC-managed 对象内部 raw pointer 的使用必须考虑 pinning，见后文。

## 6. `UIntPtr` 与指针整数转换

`UIntPtr` 是目标 word-sized 无符号整数别名：

```kotlin
typealias UIntPtr = UInt
```

规则：

- `UIntPtr` 本身不是 unsafe 类型。
- 指针和整数互转是 unsafe 操作，必须使用 sysroot intrinsic。
- 这种转换不使用 `as` / `as?`。

典型形态：

```kotlin
@Intrinsic @NoGC @Unsafe
fun <T> ptrToUIntPtr(p: Ptr<T>): UIntPtr

@Intrinsic @NoGC @Unsafe
fun <T> uintPtrToPtr(addr: UIntPtr): Ptr<T>
```

如果整数值不是有效地址，或地址生命周期/对齐不满足 `T`，行为由 unsafe 调用者负责。

## 7. Function pointer

函数指针：

```kotlin
@Intrinsic
struct FunPtr<F>
```

规则：

- `FunPtr<F>` 表示原生函数指针。
- `FunPtr<F>` 固定属于 native/C ABI family，不支持额外 `abi` 参数。
- `F` 必须是无 effect 的函数类型，例如 `(Int, Int) -> Int`、`() -> Int / Pure!`。
- 若 `F` 是 receiver function type `T.(A1, ..., An) -> R`，调用时 receiver 作为第一个显式参数传递。
- 调用 function pointer 是 unsafe 操作。
- `@CallingConvention(...)` 可标记函数指针别名或外部函数调用约定；它不带 `abi` 参数，也不能把 `FunPtr` 切换到 Scoop ABI。
- `FunPtr<F>` 不是 effect/control bridge token；普通 C ABI `@Extern` 边界不会因为返回/接收 `FunPtr<F>` 而放宽 effect-impermeable 规则。
- 后续 unsafe `fp(...)` / `fp.invoke(...)` 调用仍是普通 native function-pointer call，而不是 effect/state-machine 调用。
- 若未来需要 managed import/export callable，应使用专门的 import/export surface，而不是给 `FunPtr` 扩展 `abi = "scoop"`。

示例：

```kotlin
@CallingConvention("stdcall")
typealias MyFuncPtr = FunPtr<(Int, Int) -> Int>

@Unsafe do {
    val fp: MyFuncPtr = ...
    val result = fp.invoke(1, 2)
}
```

## 8. `@Extern`

`@Extern` 声明外部符号。当前函数支持两类 ABI：

- `abi = "c"`：native C ABI boundary。
- `abi = "scoop"`：Scoop / Managed ABI binary boundary。
- 省略 `abi` 时默认是 `c`。
- 外部顶层变量当前只支持 C ABI 语义。

```kotlin
@Extern(lib = "mylib", name = "myfunc")
fun myFunc(x: Int): Int

@Extern(name = "managedHelper", abi = "scoop")
fun managedHelper(x: Int): String

@Extern(name = "errno")
@ThreadLocal
var errno: Int
```

参数：

- `name`：外部符号名。省略或空字符串时使用 Scoop 声明名。
- `lib`：可选外部库名。如何传给链接器由工具链定义。
- `abi`：可选 ABI 家族。省略时默认 `c`；当前只允许用于函数声明。`c` 表示 native C ABI，`scoop` 表示 external linkage 下的 ordinary managed ABI。

允许形态：

- `@Extern`
- `@Extern("symbol")`
- `@Extern(name = "symbol")`
- `@Extern(lib = "mylib", name = "symbol")`
- `@Extern("symbol", abi = "scoop")`
- `@Extern(name = "symbol", abi = "scoop")`

规则：

- `@Extern` 函数必须省略函数体。
- `@Extern` 函数声明不得显式再写 `@Unsafe` 或 `@NoGC`；这两个语义由 `abi` 决定。
- `abi = "c"`（以及省略 `abi` 的 `@Extern`）：
  - 调用需要 unsafe context。
  - 在调用图约束上隐式视为 `@NoGC`。
  - 是 effect-impermeable：
    - 不允许 effect row 参数。
    - effect row 必须省略或显式为 `Pure` / `Pure!`。
    - 不允许 effect propagation、continuation resume 或 longjmp-like non-local control 穿越边界。
  - receiver、参数和返回类型必须是 GC-free value type。
  - `Continuation<...>`、class、interface、`String`、`Any` 等 GC-managed/control 对象不能直接出现在签名中。
  - 返回/接收 `FunPtr<F>`、`UIntPtr`、`GcHandle.raw` 这类值也不允许 effect/continuation 穿越边界。
- `abi = "scoop"`：
  - 调用不需要 unsafe context。
  - 不隐式视为 `@NoGC`。
  - 当前 v1 只支持顶层函数。
  - 当前 v1 仍然要求 `Pure`、禁止 effect row 参数、禁止 outward suspend / continuation crossing。
  - 当前 v1 不支持泛型和 closure/function-value surface。
  - 参数和返回值可使用 GC ref 与 ordinary aggregate。
  - 当前不支持 `@CallingConvention`。
  - 它建模的是 DLL/so import-export 这类 binary boundary，不是普通多-cone 项目内调用。
外部变量：

- `@Extern` 可用于顶层变量。
- 外部变量声明不得有 initializer。
- 外部变量类型必须是 GC-free value type。
- 访问外部变量需要 unsafe context。
- 外部 TLS 变量可组合 `@Extern` 与 `@ThreadLocal`。
- 外部变量当前沿用 C ABI 语义；`abi = "scoop"` 尚不用于变量。

## 9. `@CallingConvention`

```kotlin
@CallingConvention("stdcall")
@Extern(name = "MessageBoxA", lib = "user32")
fun messageBoxA(
    hwnd: UIntPtr,
    text: Ptr<Byte>,
    caption: Ptr<Byte>,
    flags: UInt32
): Int
```

规则：

- 默认调用约定为平台 C ABI。
- 支持的调用约定名称由实现定义。
- 当前可移植源码应只依赖默认 C ABI，除非目标平台和工具链明确支持指定约定。
- `@CallingConvention` 不等同于 Managed ABI 模式；它只描述 machine/native calling convention。
- `@CallingConvention` 可用于 native `@Extern(abi = "c")` 与 `FunPtr` 别名；对 `@Extern(abi = "scoop")` 是无效组合。
- `@CallingConvention` 当前没有 `abi` 参数；`FunPtr` 的 ABI family 固定为 native/C ABI。

## 10. `@CLayout`

`@CLayout` 强制 struct 使用 C 兼容布局：

```kotlin
@CLayout(aligned = 4)
struct MyStruct {
    val field1: Int
    val field2: Byte
}
```

参数：

- `aligned`：struct 最小对齐，必须是 2 的幂。`0` 表示未指定。
- `packed`：字段最大对齐/packing。`0` 表示未指定。

规则：

- `@CLayout` 只能用于 struct。
- 该 struct 必须是 GC-free。
- 计算布局必须匹配目标 C ABI 对相应 aligned/packed 配置的规则。
- `@CLayout` struct 可用于 `@Extern(abi = "c")` 签名和 raw pointer pointee。
- 含 GC 引用的 struct 不能 `@CLayout`。

## 11. 顶层可变全局

普通顶层 `var` 必须显式标记：

```kotlin
@ThreadLocal
var threadLocalCounter: Int = 0

@Global
var globalCounter: Int = 0
```

规则：

- 顶层 `var` 必须有 `@ThreadLocal` 或 `@Global`，否则编译错误。
- 类型必须是 GC-free value type。
- `@ThreadLocal`：每个 OS 线程有独立实例。
- `@Global`：进程内共享一个实例。
- 全局可变状态初始化建议限制为编译期常量；实现可对复杂 initializer 报错或定义更强运行期初始化机制。
- 读写 `@Global` 不自动提供数据竞争保护；同步是用户或库责任。

`@Extern` 顶层变量仍受 `@Extern` 规则约束：无 initializer、GC-free、访问需要 unsafe context。

## 12. GC pinning

Pinning 阻止 GC 移动某个堆对象，使外部系统能短期持有其 raw address：

```kotlin
@Intrinsic
object GC {
    @NoGC @Unsafe
    fun pin(obj: Any): Pinned

    @NoGC @Unsafe
    fun unpin(pinned: Pinned)
}

struct Pinned(val value: Any)
```

语义：

- `GC.pin(obj)` 标记对象 pinned，直到对应 `GC.unpin`。
- Pinned 对象在 pinned 期间被视为 GC root，保持存活。
- 每次成功 `pin` 必须恰好对应一次 `unpin`。
- 丢弃 `Pinned` handle 而不 unpin 是资源泄漏。
- 重复 unpin 是运行期错误。
- Pinning 是 per-object；pin wrapper 不会自动 pin wrapper 引用的其它对象。
- Pinning 不阻止 GC 运行，只阻止该对象移动。
- 长期 pinning 可能降低 moving/compacting GC 效率。
- Pinning 是 unsafe，因为外部仍持有 raw pointer 时过早 unpin 会导致内存破坏。

`Pinned` 是 Scoop 侧 pin handle，不是长期 native identity token。跨 safepoint 的长期回调/注册/wake token 应使用 stable GC handle。

## 13. Stable GC handle

Stable handle 用于 native 代码跨 safepoint 保存对 GC 对象的引用身份，而不是保存 raw address：

```kotlin
@Intrinsic
object GC {
    @Unsafe
    fun handleNew(obj: Any): GcHandle

    @NoGC @Unsafe
    fun handleGet(handle: GcHandle): Any

    @NoGC @Unsafe
    fun handleDrop(handle: GcHandle): Unit
}

struct GcHandle(val raw: UIntPtr)
```

语义：

- `GC.handleNew(obj)` 创建新 handle，并保持对象存活。
- `GcHandle.raw` 可作为 word-sized opaque token 通过 FFI 往返。
- 复制 `GcHandle` 或 `raw` bits 不克隆底层 runtime handle record。
- 每次成功 `handleNew` 必须恰好对应一次成功 `handleDrop`。
- `handleGet` 返回当前对象引用；对象移动后地址可变化。
- `handleDrop` 释放 handle。
- 对 stale、未知、已 drop、损坏的 token 调用 `handleGet` / `handleDrop` 是运行期错误。
- GC handle 不保证稳定对象地址；需要 raw pointer 时必须额外 pin 且只在短期窗口内使用。

推荐长期 round-trip：

1. Scoop 调用 `GC.handleNew(obj)`。
2. 把 `handle.raw` 传给 native 注册状态。
3. 回调/完成事件带回 `raw`。
4. Scoop 重建 `GcHandle { raw: raw }`。
5. 调用 `GC.handleGet` 取回当前对象引用。
6. 注册结束或完成后调用 `GC.handleDrop`。

## 14. `@Extern` 与 GC 互操作边界

普通 C ABI `@Extern` 不能直接传 GC 引用。原因：

- Native 代码不受 Scoop GC stack map 管理。
- Moving GC 可移动对象，raw address 跨 safepoint 会失效。
- Effect/continuation 不能穿越 native leaf boundary。

可用模式：

- 短期同步调用需要 raw pointer：pin 对象，取得 pointer，调用 native，返回后 unpin。
- 长期注册/回调需要身份：使用 `GcHandle.raw`。
- Native 若传递函数地址，可使用 `FunPtr<F>`；但 `F` 必须无 effect，后续 unsafe 调用仍是普通 native call，不建立新的 effect boundary。

禁止模式：

- 在普通 C ABI `@Extern` 签名中直接使用 `Any`、`String`、class、interface、`Continuation`。
- Native 保存未 pinned 的 GC object raw address 跨 safepoint。
- Native 通过普通 C ABI `@Extern` 调用直接恢复 continuation 或抛出 Scoop effect。

## 15. Internal atomic value types

实现可在 sysroot/runtime 定义内部 atomic value types：

```kotlin
struct __AtomicInt
struct __AtomicLong
struct __AtomicBoolean
```

规则：

- 这些类型必须是 GC-free。
- 布局等同其底层标量类型。
- 原子 load/store/CAS 等操作是 compiler intrinsic 或 runtime intrinsic。
- 具体 API 属于 sysroot/runtime 表面，不是普通标准库语言要求。

## 16. FFI-managed resource release callback

Scoop 不提供通用用户 finalizer。为了 FFI 管理资源，运行时可为特定 GC-managed 对象类型关联 release callback。

高层语义：

- 每个 GC-managed 类型有实现定义的 type descriptor。
- Type descriptor 可包含可选 release callback。
- 当 GC 判定对象不可达并准备回收时，若存在 callback，则以对象存储地址调用。
- Callback 用于释放 unmanaged 资源，例如 OS handle、非 GC arena 内存。

限制：

- 这不是 Scoop 0.1 的用户级语言特性。
- Callback 由编译器为特定 runtime/library 类型自动合成。
- Callback 不能复活对象。
- Callback 不能依赖其它 GC-managed 对象仍然存活。
- Callback 的调用上下文和允许操作由实现定义；应按 `@NoGC` / `@Unsafe` 风格约束。

## 17. Managed ABI 状态说明

本文当前把 `@Extern(..., abi = "scoop")` 作为 Managed ABI / Scoop ABI 的设计入口。

在 Scoop 0.1 草案中：

- `abi = "scoop"` 表示 external linkage + ordinary managed call。
- 它是 DLL/so import-export 这类 binary boundary，不是普通多-cone 项目内调用。
- 默认 `@Extern` 仍是 `abi = "c"`。
- `abi = "scoop"` 不隐含 `@Unsafe` / `@NoGC`。
- `abi = "scoop"` 仍保持 `Pure` only、effect-impermeable，且不允许 continuation / outward suspend 穿越边界。

## 18. 与标准库的关系

本部分出现的 `GC`、`Ptr<T>`、`FunPtr<F>`、`Pinned`、`GcHandle`、`UIntPtr` 等名字是低层核心 surface。它们可以由 sysroot 声明并由编译器/runtime 实现。

本文不规定：

- 平台 IO API。
- Reactor/executor API。
- Thread/sync 标准库。
- 文件、网络、时间、环境变量接口。
- 高级资源管理库。

这些库可以在本文定义的 unsafe/FFI/GC 边界上构建。
