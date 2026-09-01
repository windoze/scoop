# M12 设计：FFI 注解族与原生边界

版本：0.1（草案）

对应 `docs/ROADMAP.md` 的 M12。目标：落地 spec 第 13、14 章的 FFI 最小闭环——核心注解、unsafe / NoGC 检查、C 布局、受控全局存储、`Ptr` / `FunPtr`，以及 C ABI / Scoop ABI 的 extern 调用；同时退役能够由正式 FFI 表达的临时 runtime intrinsic。M12 必须以真实 C 库、双向 struct 传值、全局符号、callback与 direct managed ref通过端到端验证，不能只停留在“发射一个未定义函数声明”。

## 0. 范围与关键决策

M12 交付以下闭环：Scoop 声明一个外部函数并链接原生库；在显式 unsafe context 中通过 C ABI 传入 GC-free 标量、指针或 `@CLayout` struct；外部函数可以读写显式全局存储并同步回调一个 `@NoGC` Scoop 顶层函数；返回后 Scoop 继续执行。Scoop ABI extern 另以普通 managed call直接传递 ref，作为 runtime / core 的 GC-aware 入口，并用于退役 `write(String)` intrinsic。

- 支持 `@Extern`、`@NoGC`、`@Unsafe`、`@Safe`、`@CLayout`、`@CallingConvention`、`@Global`、`@ThreadLocal`、`@InteriorMutable`；`@Intrinsic` 迁移到同一套通用注解 AST 与共存检查，生产编译仍只允许 core 声明已登记的 intrinsic。M14将其扩展到intrinsic type，并为compiler test增加只放宽provider来源检查的内部allowlist；
- 支持 `T : value` / `T : ref` 两种 kind bound，满足 `Ptr<T>` 与 GC handle API 的需要；interface 上界和 `where` 子句仍属于 M14；
- 支持 `Ptr<T>`、`Option<Ptr<T>>`、`FunPtr<(A...) -> R>`、`Option<FunPtr<F>>`，并复用 M11 的函数类型与中性函数声明引用语法：在明确的 `FunPtr<F>` 期望类型下，合格的顶层 `::name` 直接解析为 native callback 地址；在 managed 函数类型上下文或无期望类型时，同一语法仍产生 M11 callable reference。managed 函数值/lambda/closure 不是 C-FFI-safe，不能一般性转换为 `FunPtr`；`FunPtr` 本身不提供 `invoke`。M12 只交付同步同线程的 `@NoGC` callback，不导出 managed closure、不建立 callback token，也不允许 foreign thread进入 managed代码；这些能力由 M13 完成；
- 支持 M12 所需的最小顶层存储声明：extern `val` / `var` 无 initializer；本 Cone 的 `@Global` / `@ThreadLocal var` 只接受 GC-free 编译期常量 initializer。普通顶层属性、引用类型全局根、动态初始化顺序与 `const val` 仍在既有 backlog；
- `@Extern` 只支持顶层、非泛型、非挂起函数。外部方法、泛型 extern、C varargs 和运行期动态装载均给正式的不支持诊断；
- **现阶段不支持 suspend FFI**：`@Extern` 与 `suspend` 互斥，挂起函数声明引用不能在 `FunPtr` 上下文中解析为原生地址，`@NoGC suspend fun` 也非法；不生成同步/挂起 wrapper，不暴露 M10 hidden continuation ABI；
- M12 只支持 64 位 host 编译，以及默认/显式 `cdecl`。`stdcall`、Windows SEH/catchpad 与 cross compilation 不在本里程碑；未知或当前 target 不支持的 calling convention 在 HIR 诊断；
- 不手写 SysV AMD64 / AArch64 等聚合 ABI 分类器。C ABI extern 由 codegen 根据已经定型的 LIR FFI 描述生成小型 C bridge，再由系统 C 编译器完成真实 C ABI lowering；bridge 用 `_Static_assert` 校验每个 `@CLayout` 的 size / alignment / field offset。Scoop ABI extern 不经过该 bridge，直接复用 Scoop typed machine ABI；
- C ABI extern 是 GC leaf，签名不能含 managed ref；Scoop ABI extern 是 managed safepoint call，可以直接传递 M12 支持的 ref。两种 ABI 都是同步 ordinary call，不是协程 ABI；
- 外部异常穿越 FFI frame 不在 M12 重新定稿：C ABI 继续按 runtime spec 第 5 章视为未定义，Scoop ABI 维持初版禁止。C bridge与 Scoop ABI direct call都按 `nounwind` 契约发射，违反契约不转换成 Scoop 异常。

最后两点共同固定原生边界：C ABI 由 C 编译器负责“机器上怎样传”，Scoop ABI 由 Scoop 自身 typed ABI负责；HIR/LIR 按 ABI 分别检查允许的类型与效果。不能因为 C bridge 最终接受字节地址，或 Scoop ABI 在 LLVM 中把 ref表示为 `ptr`，就绕过对应的 C-FFI-safe、native-root、NoGC 或 unsafe 契约。

## 1. 语言子集与注解模型

### 1.1 端到端示例

原生库：

```c
typedef struct {
    int64_t x;
    int64_t y;
} CPoint;

int64_t sum_point(CPoint p) { return p.x + p.y; }
int64_t apply_i64(int64_t v, int64_t (*f)(int64_t)) { return f(v); }

int64_t native_counter = 7;
_Thread_local int64_t native_tls_counter = 11;
```

Scoop：

```
@CLayout
struct CPoint(val x: Int, val y: Int)

@Extern(lib = "m12_fixture", name = "sum_point")
fun sumPoint(p: CPoint): Int

@Extern(lib = "m12_fixture", name = "apply_i64")
fun apply(value: Int, callback: FunPtr<(Int) -> Int>): Int

@Extern(lib = "m12_fixture", name = "native_counter")
@Global
var nativeCounter: Int

@Extern(lib = "m12_fixture", name = "native_tls_counter")
@ThreadLocal
var nativeTlsCounter: Int

@NoGC
fun addOne(value: Int): Int = value + 1

@Global
var scoopCounter: Int = 0

fun main() {
    @Unsafe {
        val total = sumPoint(CPoint(20, 21))
        nativeCounter = total
        nativeTlsCounter = apply(nativeCounter, ::addOne)
        scoopCounter = nativeTlsCounter
    }
    println(scoopCounter)
}
```

这个例子同时锁定：`@CLayout` 按值传递、C extern 的 unsafe 调用、顶层函数声明引用在 `FunPtr` 期望类型下的 native-address contextual resolution、extern global / TLS 与本地显式全局存储。callback 在发起 C 调用的同一已注册线程上、且在该调用返回前同步完成；M12 不提供 callback token、closure 保活、foreign-thread attach/detach 或异步调用协议，这些由 M13 完成。

### 1.2 通用注解语法

AST 不再把注解写死为“一个可选字符串”。M12 的注解使用统一结构：

```
Annotation {
    name: Ident,
    args: Vec<AnnotationArg>,
    span: Span,
}

AnnotationArg {
    name: Option<Ident>,
    value: AnnotationLiteral,       // String / Int / Boolean
    span: Span,
}
```

- 支持 marker、位置参数和命名参数，例如 `@NoGC`、`@CallingConvention("cdecl")`、`@Extern("m", "foo")`、`@Extern(lib = "m", name = "foo")`；
- 位置参数必须在命名参数之前；重复参数、未知参数、缺参数、类型不符或同一注解重复出现均在 HIR 诊断；
- parser 只负责保留语法与精确 span，不根据注解名字决定函数能否有 body。bodyless 声明由 HIR 结合 `abstract` / interface / `@Intrinsic` / `@Extern` 判断；
- M12 识别固定的核心注解集合。自定义 `annotation class`、use-site target、retention、重复注解与编译器插件处理不在 M12；未知注解给“当前里程碑不支持”的正式诊断；
- `@Unsafe { ... }` / `@Safe { ... }` 是 annotated block，AST 使用带 `SafetyMode` 的 block 节点，不把它伪装成普通函数调用。

### 1.3 目标与共存矩阵

| 注解 | M12 合法目标 | 可共存规则 |
|---|---|---|
| `@Intrinsic` | core 顶层函数；登记表明确允许时可用于 core 方法 | 默认与其他注解互斥；指针 intrinsic 可与 `@NoGC @Unsafe` 共存，GC primitive 可与 `@Unsafe` 共存 |
| `@Extern` | 顶层 function、顶层 `val` / `var` | function 可与 `@NoGC`、`@Unsafe`、`@CallingConvention` 组合；global 可与 `@Global` / `@ThreadLocal` 组合；不得与 `@Intrinsic`、`suspend` 共存 |
| `@NoGC` | function / method | 可与 `@Unsafe` 或 `@Safe` 二选一；不得用于 suspend callable |
| `@Unsafe` / `@Safe` | function / method、block | 两者互斥；`@Safe` 不能把 C ABI extern 改成 safe function |
| `@CLayout` | struct | 可与 `@InteriorMutable` 共存 |
| `@CallingConvention` | 顶层 extern function、可作为 `FunPtr` native-address resolution 目标的顶层 `@NoGC` function | M12 只接受 `cdecl`；不得用于 method、global 或 suspend callable |
| `@Global` / `@ThreadLocal` | 顶层 `var` | 两者互斥；extern `var` 同样必须二选一 |
| `@InteriorMutable` | struct | 可与 `@CLayout` 共存；其值的使用受 unsafe context 约束 |

注解解析后，HIR 只保留类型化属性，不把原始名称字符串交给下游。典型结构为 `FunctionFlavor::{User, Intrinsic, Extern}`、`Safety::{Safe, Unsafe}`、`GcEffect::{Managed, NoGc}`、`ExternAbi::{C, Scoop}`、`CallingConvention::Cdecl`；非法组合不可能进入 MIR。

### 1.4 unsafe context

- C ABI `@Extern` function 与显式 `@Unsafe` function 是 unsafe function；Scoop ABI extern 默认不是 unsafe function，但可显式加 `@Unsafe` 收紧调用条件；
- `@Unsafe` function 的整个 body 处于 unsafe context，且调用该函数也要求调用者位于 unsafe context；普通函数与 `@Safe` function 的 body 从 safe context 开始；
- `@Unsafe {}` 压入 unsafe，嵌套 `@Safe {}` 再压回 safe；离开 block 必须恢复外层状态。控制流跨 `return` / `throw` 不改变该词法规则；
- unsafe 检查发生在重载决议之后，不通过“只在 unsafe context 展示候选”改变候选层级或 MSC；
- 需要 unsafe 的操作包括：调用 unsafe function、`Ptr` 读写/算术/转换、`addressOf`、使用含 `@InteriorMutable` 的值。仅仅声明一个 `FunPtr` null 值不要求 unsafe；把 safe `@NoGC` 顶层函数引用在 `FunPtr` 上下文中解析为原生地址也不要求。若 callback 本身带 `@Unsafe`，该 resolution 表达式必须位于 unsafe context，由该显式动作授权 native code 随后调用它。

### 1.5 `@NoGC` 的可验证契约

`@NoGC` 是模块化效果契约，不是 codegen hint。HIR 在所有签名解析完成后验证：

- `@NoGC`也可标记struct/enum的GC-free表示契约；非generic声明立即检查，generic value type在HIR生成fully specialized `ConcreteTypeId`时检查。失败必须在HIR诊断，不能推迟到MIR、布局或codegen；
- callable 必须非 suspend；参数、返回值、receiver 和所有局部值必须是 GC-free。`ExportHir`中的generic template不携带GC-free布尔结论；其中出现的类型参数只形成“实例化时必须GC-free”的显式约束。HIR在type parameter全部resolve并构造`LocalConcreteHir`实例时完成最终检查，每个`ConcreteTypeId`必须直接带非可选`gc_free: bool`；
- body 不得构造、读取或写入 managed ref，不得装箱、分配 class/array/String、抛出或捕获异常，也不得执行可能产生 Scoop 异常的检查型操作；初版因此拒绝整数除法、数组访问、`!!`、checked cast 等，即使表面结果是值类型；
- 只允许调用另一个已验证的 `@NoGC` callable、C ABI extern，或 intrinsic registry 中标记为 NoGC 的 primitive。Scoop ABI extern 只有显式带 `@NoGC` 时才可调用；
- class / interface method 因 receiver 是 ref 不能 `@NoGC`；struct / enum method 只有在 receiver 的具体类型 GC-free 时合法；
- 递归与互递归按声明的 `@NoGC` 契约建立调用图，每个 body 仍独立验证，不能因递归跳过检查；
- 验证通过的函数不设置 GC strategy、不插入口/回边 safepoint poll，调用点可标为 GC leaf；这也是 callback bridge 能从无 stack map 的 C frame 同步进入 Scoop 的依据。

无效裸指针造成的硬件 fault / UB 不转换为 Scoop 异常；`@NoGC` 只证明不会进入 Scoop GC 或 Scoop EH 路径。

## 2. FFI 类型、指针与布局

### 2.1 `value` / `ref` kind bound

- AST 的 type parameter 统一为带可选 kind bound 的 `TypeParamDecl`；M12 在 function / struct / enum / interface 参数上接受 `T : value` 与 `T : ref`；
- HIR 使用 `TypeParamKind::{Any, Value, Ref}`，generic 定义检查参数使用，实例化时检查具体类型；这不是用名字识别 `T`，也不与 M14 的 interface upper bound 共用无类型字符串；
- `value` 约束值类型，`ref` 约束 class / interface / `Any` / String / array 等引用类型。值类型是否 GC-free 是另一条递归属性，不能把 `value` 当作 GC-free 的同义词；
- M14 加 interface bound / `where` 时扩展为结构化 bound 列表；M12 不接受这些语法后再静默忽略。

### 2.2 GC-free、C-FFI-safe 与 Scoop-ABI-safe 分离

编译器提供三个独立、可递归且带环检测的判定。GC-free判定由HIR拥有：generic export template至多保存待实例化条件；每个`LocalConcreteHir` concrete type实体必须保存非可选布尔结果，不能使用`Option<bool>`、sentinel或后续stage回填：

- `is_gc_free(T)`：concrete `T`的表示不直接或间接含managed ref；fully specialized enum的每个variant均直接带`gc_free: bool`，enum自身的`gc_free: bool`恒等于所有variant结果的AND；
- `classify_c_ffi_type(T)`：给出明确的 C 边界表示，或返回带字段路径的诊断。C-FFI-safe 必然 GC-free，但 GC-free 不必然有稳定 C 表示；
- `classify_scoop_abi_type(T)`：判断类型能否按 M12 的普通 Scoop typed ABI直接交给 runtime。它与 GC-free正交，ref type正是该分类器与 C classifier 的核心差异。

M12 的 C ABI 边界类型集合：

| Scoop 类型 | FFI 边界表示 |
|---|---|
| `Unit` | 只允许作返回值，映射 C `void` |
| `Int` / `UInt` / `Boolean` | `int64_t` / `uint64_t` / `_Bool` |
| `Ptr<T>` / `Option<Ptr<T>>` | `void *`，null 对应 `None` |
| `FunPtr<F>` / `Option<FunPtr<F>>` | 与 `F` 精确匹配的 C function pointer，null 对应 `None` |
| `@CLayout` struct | 按第 2.5 节的固定布局按值传递；字段必须递归 C-FFI-safe |
| core `PinnedPtr<T>` / `GcHandle<T>` | 经 core contract 验证的透明 `UInt` 表示；二者类型身份仍不同 |

M12 从 C ABI 拒绝 ref、`Any`、String、array、普通 struct、tuple、enum、未具体化类型参数，以及包含这些类型的聚合。`Option<T>` 只在 `T` 为 `Ptr` / `FunPtr` 且命中 7.4 niche 保证时可过 C 边界；M12 不为任意 enum 发明 C tagged-union ABI。

M12 的 Scoop ABI 至少接受 `Unit`、基本/GC-free边界值，以及所有以一个 managed ref word表示的 concrete ref type（`String`、array、class/interface、函数值及其引用 supertype view）；ref 参数/返回值直接使用普通 Scoop ABI。包含 managed ref的 struct/enum/tuple 按值跨 native边界需要递归 native-root layout协议，M12 暂不开放，给精确的不支持诊断；这不影响 `write(String)` 等 direct-ref入口。

### 2.3 `Ptr<T>` 与内存 primitive

`Ptr` / `FunPtr` 在 core 中有源码声明，但 HIR 在验证唯一的 core contract 后将应用正规化为专用类型节点；下游不靠 FQN 或“单字段恰好是 UInt”猜测指针身份。

M12 同时落地调用点显式类型实参语法，因为 `sizeOf<T>()` / `alignOf<T>()` 没有 value argument 可供推断，`Ptr<T>.cast<U>()` 也必须直接给出目标类型。语法统一适用于普通/局部/重载函数、generic 值构造和 generic member/extension 调用；列表要么省略并完整推断，要么完整写出 callee 自己声明的参数。generic receiver 已经确定的宿主参数不在 member call 处重复。

- `Ptr<T>` 的具体 `T` 在 M12 必须是 GC-free value；`Ptr<Unit>` 表示 `void *`。这项 M12 限制保证 `@NoGC` 的 `load` / `store` 不会暗中搬运 managed ref；指向 managed 对象必须使用 pin 后得到的 opaque 地址，而不是 `Ptr<含 ref 的值类型>`；
- `Ptr<T>(raw: UInt)` 允许从整数显式制造地址，但构造本身要求 unsafe context；`raw == 0u` 产生 null。通常应优先从 extern、`addressOf` 或其他 `Ptr` 转换取得地址；
- `load` / `store` 按 `alignOf<T>()` 访问；用户构造未对齐指针时行为未定义。packed struct 字段由编译器自己的 field lowering 使用较低对齐，不通过普通 `Ptr<T>.load` 猜测；
- `plus` / `minus` 做 `offset * sizeOf<T>()` 的字节步进；整数溢出、越界、悬垂、别名与对象生命周期均由 unsafe 调用者负责；
- `cast<U>()` 只改静态 pointee type，不改地址；目标 `U` 同样必须是 GC-free value；
- `addressOf` 在 HIR 产生 `AddressOf(Place)`，而不是先把参数降成普通 rvalue call。合法 place 为参数、局部 `val` / `var`、本地 global 或 extern global；临时值、字面量、计算结果、class field 与 array element均拒绝；
- address-taken local / parameter 必须在 LIR 有稳定 stack slot，地址只在该 activation 生命周期内有效；`addressOf` 不提供 borrow/lifetime 检查；
- `sizeOf<T>` / `alignOf<T>` 在 LIR layout 已知后常量化为 `UInt`，不产生 runtime call。未定型或 unsized FFI 类型在 HIR 已被拒绝。

上述 primitive 在 intrinsic registry 中携带目标 stage、NoGC、unsafe 与签名约束；不在各 lowerer 里按函数名散落特殊分支。

### 2.4 `FunPtr<F>` 与 callback bridge

- `F` 直接使用 M11 的 `FunctionTypeId`，必须是 ordinary、完全具体化的函数类型；参数与返回值还必须逐项通过本章 C-FFI-safe classifier。managed 函数类型继续可以正常出现在变量、参数、返回值与字段中，但这些函数值本身不能作为 native callback穿越 C ABI；
- `FunPtr<F>()` 是唯一用户可直接构造的值，产生 null；传 raw integer 构造非 null `FunPtr` 是编译错误；
- `::name` 是中性的函数声明引用语法，不具有固有的 managed 或 `FunPtr` 类型。期望类型是 managed 函数类型时继续进入 M11 callable-reference resolution；没有期望类型时也只推导 managed 结果，绝不因为目标带 `@NoGC` 而自动选择 `FunPtr`；
- 在期望类型明确为 `FunPtr<F>` 的位置出现 `::name` 时，HIR 复用 M11 的声明候选查找，但进入独立的 native-address contextual resolution：目标声明签名必须与 `F` 精确相同，不应用 managed 函数类型型变；
- 目标必须是顶层、非 generic、非 suspend、非 extern、已验证的 `@NoGC` user function，且 calling convention 为 cdecl；局部/成员/扩展函数、lambda、匿名函数、绑定引用、intrinsic 与已有 managed 函数值都不能作为非 null `FunPtr` 的来源。目标同时带 `@Unsafe` 时，native-address resolution 表达式要求 unsafe context；
- HIR 直接产生类型化 `FunctionAddress`，不先产生 managed callable reference 或分配 M11 closure；MIR 为每个实际取址的 `(FunctionId, FunctionTypeId)` 生成一个 callback bridge，重复解析复用同一实体；
- codegen 生成一个 Scoop bridge，使用简单的“参数存储地址 + 可选结果地址”ABI 调用真实 `@NoGC` function；同时生成 C trampoline，以真实 C 签名收参、调用 Scoop bridge、再由 C 编译器返回结果。`FunPtr` 的地址指向 C trampoline，而不是 Scoop managed symbol；
- callback 只能在发起 extern 调用的同一已注册线程上同步执行。原生方保存指针后异步、跨线程或在 Scoop 程序退出后调用，均超出 M12 契约；M13 以独立的 managed callback registration协议支持 closure 保活和 foreign-thread 回调，不通过放宽 `FunPtr` 的来源规则实现。

### 2.5 `@CLayout`

参数缺省值为 `aligned = 0, packed = 0`。非零值必须是 1、2、4、8、16 之一；M12 的 64 位 host 不接受更大的显式对齐。

对字段 `i`：

```
field_align[i] = packed == 0
    ? natural_align(field[i])
    : min(natural_align(field[i]), packed)
offset[i] = align_up(end_of_previous, field_align[i])
struct_align = max(max(field_align), aligned == 0 ? 1 : aligned)
size = align_up(end_of_last, struct_align)
```

- 字段必须递归 C-FFI-safe；嵌套 struct 必须也有 `@CLayout`。generic `@CLayout` 定义若字段依赖类型参数，则实例化时必须得到 C-FFI-safe 具体类型后才产生布局；
- LIR layout 保存 size、align、每个 field offset 与有效 load/store alignment；packed 字段不能用自然对齐的 LLVM load/store；
- 同一布局用于普通 Scoop 值、数组元素、其他 `@CLayout` 字段和 FFI bridge，不能只在 extern call 临时重排；
- 生成的 C bridge 为每个使用到的布局发射对应 C struct，并用 `_Static_assert(sizeof)`、`_Alignof`、`offsetof` 校验。任一断言失败属于编译器布局错误，driver 报 bridge 构建失败，不继续链接；
- `@CLayout` 不承诺 C field 名称或 typedef 名称，只承诺布局与按值 ABI。C bitfield、union、flexible array member 与不完整类型不在 M12。

### 2.6 `@InteriorMutable`

- struct 字段在 Scoop 源码中仍是 `val`；该注解表示值可能被 raw pointer / FFI 在语言不可见处修改，不新增普通字段赋值语法；
- HIR 递归计算 `requires_unsafe_use(T)`：直接或间接包含 `@InteriorMutable` struct 的值，在构造、读取、复制、参数传递、返回、比较及取址时都要求 unsafe context；safe function 的公开签名不能含此类值；
- `@InteriorMutable` 不改变值语义或引入 identity。receiver 仍遵守 spec 3.3 的统一 pass-by-value 规则；只是该注解使 ABI 上的间接传递变得可观察，因而必须为 `this` 物化独立的 method-local copy。经 box/interface 调用时也不得把 payload 地址直接暴露给方法体。`addressOf(this)` 取得该副本的地址，有效期仅限当前 method activation；
- `@CLayout @InteriorMutable` 是合法且常见的组合；
- LIR 保留 interior-mutable 标志，禁止后续优化为字段添加 immutable / readonly / noalias 假设。M12 当前没有这类优化，但 metadata 不能在 stage 间丢失；
- 该注解不等于 `volatile` 或 atomic。并发可见性和数据竞争仍由外部 API 契约负责。

## 3. extern、bridge 与全局存储

### 3.1 extern function 身份

合法 extern function 必须是顶层、bodyless、非 generic、非 suspend，并具有对其 `abi` 完整合法的签名：C ABI 逐项通过 C-FFI-safe classifier，Scoop ABI 逐项通过 Scoop-ABI-safe classifier。HIR 将其表示为独立实体：

```
ExternFunction {
    source_name,
    native_symbol,
    library,
    abi,
    calling_convention,
    gc_effect,
    safety,
    params,
    return_type,
}
```

- `name = ""` 使用 Scoop 声明名；M12 的 native symbol 限制为可移植 C identifier，避免生成 bridge 时依赖 assembler label 扩展；
- `lib = ""` 表示符号由已经参与最终链接的对象/runtime 提供，不产生额外 linker 参数；非空 `lib` 是逻辑库名，不是路径或任意 linker flag；
- `abi` 只接受 `"c"` / `"scoop"`。`@CallingConvention` 缺省及显式 `"cdecl"` 等价；
- native library 不形成符号命名空间。同一 `native_symbol` 在一个编译单元内出现多次时，library、FFI 签名、ABI 与 calling convention 必须完全一致，否则 HIR 诊断；
- export侧、本地concrete侧与MIR侧的普通function id分别类型化，且都与各自的`ExternFunctionId`隔离。extern没有伪造的空body、unreachable shell或generic instance。

### 3.2 C ABI 为什么生成 bridge

LLVM IR 中写出 `%S` 参数并不等价于 C 前端为 `struct S` 选择的机器 ABI，尤其是小聚合拆分、寄存器分类和 aggregate return。M12 的 C ABI 使用如下稳定边界：

```
Scoop generated code
    -> storage-pointer bridge ABI
generated C bridge
    -> host C ABI
native symbol
```

- Scoop 一侧把每个值放入按 LIR size/align 分配的 storage，向 bridge 传 storage 地址；非 `Unit` 返回值另传 result storage；
- C bridge 用 `memcpy` 在 storage 与真实 C scalar/struct/function-pointer 局部之间搬运，再直接调用 native symbol；C compiler 自行处理参数分类、sret、zero extension 等 target ABI；
- 对纯 scalar 的调用首版也走同一 bridge，减少两套边界不变量；后续可在 ABI 等价已验证时去掉 bridge；
- bridge 函数名带 Cone 内 typed extern id，不与 native symbol 或 Scoop mangling 冲突；
- bridge 描述是 LIR module 的结构化数据，不生成 C 文本后再从文本反推类型；codegen 是唯一文本生成者。

Scoop ABI extern 不进入这条 bridge链。编译器按普通 direct managed call生成目标签名：ref为直接 managed pointer，value/return storage沿用 Scoop typed ABI；runtime symbol必须按该 ABI 实现。把 `String` 先改成 `PinnedPtr<String>` 或把参数统一装进 byte storage都会抹掉 Scoop ABI 需要验证的 direct-ref语义，因此禁止作为回退。

### 3.3 ABI、GC 与异常边界

- `abi = "c"` 的 bridge call 标为 GC leaf：不设置 statepoint，不允许 callee 调 Scoop runtime；它唯一能进入 Scoop 的路径是同步调用 M12 的 `@NoGC` callback；
- `abi = "scoop"` 直接调用 native symbol，并作为普通 managed safepoint保留 caller roots；direct ref实参按 spec 14.3 作为同步借用传入。native symbol若要跨可能触发 GC 的操作继续使用它们，必须登记并重新读取 native root slots；
- 显式 `@NoGC` 的 Scoop ABI extern 可降为 GC leaf，但其正确性是 runtime/core 作者承诺；
- 以上线程行为是 M12 单 mutator基线。M13 启用多 mutator后，可能阻塞的 C ABI outbound call必须发布 caller roots并进入 native-safe状态，Scoop ABI direct-ref callee则只能在登记 native roots的显式 runtime入口参与握手；不能继续把所有 native调用统一视为“其他线程 GC 无关”的 leaf；
- 两类 extern 都不产生 M10 resume point。suspend caller 可以调用 ordinary extern，但阻塞式 C 调用会阻塞当前线程；编译器不自动异步化；
- extern 调用按 `nounwind` 边界处理。C++ / foreign exception 穿越属于 runtime spec 第 5 章的既有未决项，M12 不把它捕获、物化或映射为 Scoop `Throwable`。

### 3.4 库解析与 driver

- driver 收集 LIR 中去重且保持首次出现顺序的非空 library 名，最终以独立参数 `-l<name>` 交给系统 linker；参数不经 shell；
- CLI 临时增加可重复的 `-L` / `--library-path`，供 M12 单 Cone 构建与 fixture 使用。M17 落地 `Cone.toml` / `.slib` 后，库依赖进入 Cone metadata，此临时入口可保留为命令行覆盖；
- M12 不支持 `dlopen` / `dlsym`、版本化符号、framework、任意 link args 或由 `lib` 注入路径。需要这些能力时由 M17 的结构化 native dependency 配置设计；
- 链接顺序固定为 Scoop object、FFI bridge object、runtime archive、extern libraries、C++ ABI 支持库，保证静态库符号按声明顺序可解析。

### 3.5 M12 的全局存储子集

AST 新增顶层 `GlobalDecl`，但不假装已经实现完整 Kotlin property：

- `@Extern val/var name: T` 必须无 initializer；读取每次访问真实 native symbol，`var` 可赋值，`val` 仅禁止 Scoop 写入；
- extern global 当前只支持 C data ABI；`abi = "scoop"` 只对函数调用有定义，用在 `val` / `var` 上直接诊断；
- extern `var` 必须带且只带一个 `@Global` / `@ThreadLocal`；extern `val` 不要求二者，并且不能带这两个仅用于 `var` 的注解。只读 native TLS 不在 M12 的声明子集；
- 本 Cone 只接受 `@Global var` / `@ThreadLocal var`，必须显式类型和 GC-free 编译期常量 initializer；无注解 var、普通 top-level val、动态 initializer 与 delegate 均给指向既有 backlog 的不支持诊断；
- 常量 initializer 支持数值/布尔、null `Ptr` / `FunPtr`、以及由这些值构成的 GC-free struct；不调用函数、不分配、不读取其他 global，因此没有初始化顺序或循环；
- 本地 global 在 LLVM 中发射为可写 global 或 TLS global，符号使用 Scoop mangling；extern global 经 C bridge 生成 typed get/set/address helper，TLS helper 每次在当前线程解析该 TLS symbol；
- HIR 使用 `GlobalId`，表达式为 `GlobalRead(GlobalId)`，赋值目标为 `GlobalWrite(GlobalId)`，`addressOf` 保存同一 typed place。MIR/LIR 不用函数名或字符串区分 local/global/extern global。

这部分只满足第 13 章的显式 FFI 存储。顶层属性在 `main` 前执行任意 initializer、引用类型全局根、跨文件顺序、循环诊断及 `const val` 仍按 ROADMAP 原项另行设计。

## 4. core / runtime 迁移

### 4.1 core FFI contract

新增 core FFI 源文件，声明/验证 `Ptr`、`FunPtr`、`PinnedPtr`、`GcHandle` 与相关 intrinsic。HIR 启动时像 M10 的 coroutine core 一样验证唯一 contract，得到类型化 `FfiCore`；验证失败是 core 配置错误，后续 stage 不按 FQN fallback。

- `Ptr` / `FunPtr` 正规化为专用 HIR 类型；`PinnedPtr` / `GcHandle` 保持两个不同的 generic struct 身份，但验证为透明 `UInt` ABI；
- `ptr_load` / `ptr_store` / `ptr_cast` / `address_of` / `size_of` / `align_of` 保留编译器 intrinsic，因为它们需要 place 或 layout 信息；登记表明确允许 `@NoGC @Unsafe` 共存；
- `gcCollect` / `gcStats` 继续作为测试专用 intrinsic，不进入公开 spec。

### 4.2 M9 GC API

M9 的四个 public intrinsic 迁为带真实安全边界的普通 core API：

- 对外的 `pin` / `unpin` / `getGcHandle` / `releaseGcHandle` 是 `@Unsafe` 普通 generic function，并使用 `T : ref`；
- 它们调用重命名的 core-only low-level GC intrinsic。低层 intrinsic 同样带 `@Unsafe`，但不谎称 `@NoGC`：pin / handle 操作会进入 runtime 并可能参与 GC；
- `PinHandle<T>` 更名并迁移为 spec 的 `PinnedPtr<T>`；现有 M9 fixture 与 golden 同批更新，不能长期保留两个等价 handle 类型。

### 4.3 输出 primitive

M7 的 `write` `@Intrinsic("rt_write")` 退役：

```
@Extern(name = "scoop_rt_write", abi = "scoop")
fun write(message: String)
```

- `String` 直接按 managed ref传入，不生成 pin/unpin wrapper，不要求 unsafe context；
- runtime implementation只在调用期间读取该引用，不分配、不触发 Scoop GC、不回调 managed代码，也不保存引用，因此无需 native root frame；
- 若以后 `write` 实现需要跨可能触发 GC 的操作继续使用 `message`，必须先登记 native root slot并在操作后 reload，不能通过把源码签名改成 `PinnedPtr<String>` 回避 Scoop ABI 契约；
- HIR intrinsic registry、MIR `RuntimeFn::Write` 与 LIR `scoop_rt_print` 特判全部删除。调用从 core源码解析为普通 `ExternFunctionId`，并在 LIR/codegen 中保持 Scoop ABI managed direct call。

Int / Boolean 的临时 `toString` runtime 映射仍由 M14 的 ToString 接口化统一退役；M12 不为清理无关 intrinsic 扩大范围。

## 5. 各 stage 设计

### 5.1 AST / parser

- `Annotation` 改为第 1.2 节的通用参数列表；所有支持目标都保存 `Vec<Annotation>`，span 覆盖完整注解；
- 注解可出现在顶层 function/global、member function、struct 和 annotated block 前；parser 不检查核心注解语义，但继续负责明显的参数语法错误和恢复；
- `Decl` 新增 `Global(GlobalDecl)`；只解析明确类型的顶层 `val` / `var`，initializer 可缺失以支持 extern；具体合法组合交给 HIR；
- 复用 M11 的 `TypeRefKind::Function` 与 `Expr::CallableReference`；M12 parser 只新增统一 `TypeParamDecl { variance, kind_bound }` 以及 FFI 注解/global 语法，不能再建立一套 callback-only function signature AST；
- `FunctionBody::None` 的注释与 parser 规则不再绑定 `@Intrinsic`，以便 bodyless extern；有 body 的 `@NoGC` / `@Unsafe` 正常解析；
- member parser 接受注解与 modifier 的固定顺序“annotations → modifiers → fun”；重复 modifier 仍由 parser 诊断。

### 5.2 HIR

- 增加 `GlobalId`、`ExternFunctionId`、`CallbackBridgeId`、`FfiCore` 及类型化 attribute/effect enums；`Type` 增加 `Ptr(T)`、`FunPtr(FunctionTypeId)`，其中 `FunctionTypeId` 直接来自 M11 的正式 managed 函数类型系统；
- 分三步处理注解：解析参数/缺省值 → target/coexistence matrix → 与完整签名/body 联合验证。只有全部成功的 typed attributes 能写入 HIR；
- `FunctionKind` 扩为结构完备的 `Defined(Body) | Abstract | Intrinsic(IntrinsicId) | Extern(ExternFunctionId)`；只有 `Defined` 有 body，`Abstract` 只参与 override/dispatch contract，extern 与 intrinsic 引用各自的类型化实体，不用空 body、unreachable shell或 `Option<Body>` 混合；
- safety context 与 suspension context 是两个独立、可嵌套的状态栈；extern 普通调用只检查 safety，不改变 suspend 状态；
- 实现 `is_gc_free`、`classify_c_ffi_type`、`classify_scoop_abi_type`、`requires_unsafe_use` 四个不同判定，并缓存递归结果；诊断给出如“C ABI field `outer.inner` contains ref type `String`”的完整路径。构造`LocalConcreteHir`时把结果写入必填字段；MIR不得再次遍历字段猜测或补齐；
- `@NoGC` body checker在普通类型检查和调用决议完成后运行，消费 resolved call/operation，不重复名称解析；generic NoGC 的条件约束进入 `GenericFunction` 和调用点实例化检查；
- 中性的 `Expr::CallableReference` 在 HIR 按 expected type类别一次性定型：`FunPtr<F>` expected type下只对显式 `::name` 尝试第 2.4 节 native-address resolution，成功后生成 `ExprKind::FunctionAddress`；managed function type或无 expected type时继续走 M11 的 managed callable-reference/closure 路径。不能先生成一种结果再转换成另一种，也不能把一个已经定型的函数值事后拆成地址；
- 全局声明先建完整 signature/storage 表，再检查 initializer 和函数 body，使前向读取、重复声明与 local shadowing 有确定规则；M12 的本地 initializer 禁止引用其他 global，因此不产生初始化依赖图；
- `@Extern suspend`、`@NoGC suspend`，以及 suspend function reference 在 `FunPtr` 上下文中的 native-address resolution，必须在 HIR 报错；MIR 不保留“稍后再拒绝”的组合。

### 5.3 MIR

- MIR只接收`LocalConcreteHir`；`ExportHir`中的generic template、导出声明id及待实例化条件在输入类型上不可达。增加独立arena：`extern_functions`、`globals`、`callback_bridges`；所有id类型互不混用，extern不进入user function arena；
- `Callee` 增加 `Extern(ExternFunctionId)`；call 保留 ABI、GC effect、nounwind 与 FFI signature，不先退化成 symbol string；M10 coroutine transform 把它视为 ordinary non-suspend call；
- 增加 `GlobalRead` / `GlobalWrite` / `AddressOf`、`PtrLoad` / `PtrStore` / `PtrOffset` / `PtrCast`、`FunctionAddress`；每个节点携带完整 concrete type，不允许用统一 `UInt` 后再猜 pointee；
- 为实际取址的 NoGC function 生成普通、非 managed 的 callback bridge body。bridge 参数是具体 storage pointer，内部 load 后 direct-call 原函数并写 result；synthetic origin 记录源函数与 signature；
- 本地global initializer已是`LocalConcreteHir` typed constant，MIR只机械映射常量聚合，不执行CFG或类型替换；
- MIR dump 锁定 extern identity、ABI/effect、global storage/TLS、pointer operation 和 callback bridge 映射。

### 5.4 LIR

- `Layout` 增加显式 aggregate alignment、field offsets 与每字段 access alignment；`@CLayout` 及普通 Scoop layout 都在此唯一计算；
- `NativeBridge` 只记录 C ABI extern function/global、C type tree、native symbol/library、调用约定、TLS 与 generated bridge symbol；codegen 只消费该结构。Scoop ABI extern保留独立的 typed direct-call描述；
- C ABI extern call lower 为 storage 分配/写入、调用 bridge、读取 result，并标 `GcLeaf + NoUnwind`；Scoop ABI extern直接 lower为目标签名的 `Managed + NoUnwind` call，ref实参保持 `Ptr` 类型且进入 statepoint roots；
- callback bridge function 标为 `NoGc`，不含 statepoint；`FunctionAddress` 指向 C trampoline symbol；
- 增加 raw `Load` / `Store` / `PtrOffset` / `GlobalAddress` / `FunctionAddress` 指令；raw store 不插 managed heap card barrier，managed `HeapStore` / `ArraySet` 的既有屏障不变；
- address-taken local/param 强制 stack slot；普通 SSA temp 不能被临时补成可寻址 place；
- `sizeOf` / `alignOf` 在本阶段替换成常量。LIR 输出不再含注解名、kind bound、unsafe context 或 intrinsic call。

### 5.5 codegen / driver / runtime

- codegen 按 function effect 决定 GC strategy：managed function 维持 M9 的 statepoint + poll；NoGC function/callback bridge 不设置 GC strategy、不插 poll，并为已验证的 leaf call加对应 LLVM 属性；
- 精确发射 CLayout padding、alloca/global alignment 和 packed field 的低对齐 load/store；`Ptr` 在 LLVM 侧使用 opaque pointer，只有 `toUInt` 等显式转换才做 `ptrtoint` / `inttoptr`；
- 从 C ABI `NativeBridge` 生成一个 C source 和对应 object；C source 包含固定宽整数头、layout static assertions、extern prototypes、outbound wrappers、global helpers 与 inbound callback trampolines。Scoop ABI direct extern不生成 outbound C bridge；
- driver 使用 `cc` crate以与 runtime 相同 host triple 编译 bridge，随后按第 3.4 节链接。生成文件位于构建输出目录并进入诊断路径，便于定位 C bridge 编译失败；
- runtime 把现有输出实现导出为 direct-ref `scoop_rt_write(const ScoopString *)`，并提供 spec 14.3 所需的 native root frame登记/移除能力；保留既有 pin/handle实现。M12 不新增 managed callback registry、foreign-thread attach/detach、线程切换或多 mutator GC；M13 在这条 native-root基础上增加 callback token与反向 managed入口。

## 6. 测试计划

### 6.1 parser / HIR unit

- marker、位置/命名参数、多个注解、重复/未知/错类型参数、bodyless extern 与带 body 的 NoGC/Unsafe；
- 每个注解的 target/coexistence matrix，特别是 `@Extern suspend`、`@NoGC suspend`、Unsafe+Safe、Global+ThreadLocal；
- `T : value/ref` 的定义点与实例化点检查；M11 function type / tuple / Unit 语法；同一个 `@NoGC ::name` 分别置于 managed function type、无 expected type与 `FunPtr<F>` 上下文，前两者必须产生 managed callable reference，只有后者产生 `FunctionAddress`；
- GC-free、C-FFI-safe、Scoop-ABI-safe、interior-mutable 的独立分类与嵌套字段路径诊断；C ABI 拒绝 `String` 而同签名 Scoop ABI 接受；NoGC 调用图与隐式异常操作；
- FunPtr expected-type callable-reference resolution，以及 generic/local/member/extension/lambda/suspend/non-NoGC rejection；
- extern native symbol 重复但 library或签名不一致、unsupported ABI/callconv、非法 lib/name。

### 6.2 IR / codegen golden

- AST dump 锁定通用 annotation args、global 与 function signature type；HIR dump 只出现 typed attributes/effects；
- HIR/MIR dump 锁定 `FunctionAddress` 不先经过 managed callable reference/closure；同一声明在 managed 上下文中的引用仍保留独立 closure实体；同时锁定 typed extern/global/callback ids、raw pointer operations，且不存在 extern fake body或 suspend extern；
- LIR dump 锁定 CLayout size/align/offset、packed access alignment、仅 C ABI具有 bridge storage、GC leaf/managed call属性、TLS helper，以及 `@InteriorMutable` / `addressOf(this)` 时值类型 method-local `this` 的独立 stack slot；
- LLVM IR 锁定 NoGC function无 GC strategy/poll、Scoop ABI `write(String)` 以 direct ref调用并成为 managed statepoint、没有 pin/unpin或 outbound C bridge，以及 pointer conversion和 local address materialization；
- 生成 C bridge snapshot 锁定 prototype、`_Static_assert`、struct return、callback trampoline与global/TLS helper。

### 6.3 端到端 fixture

fixture runner 约定同目录可带 `native.c`（需要 foreign unwind 的未来用例可带 `native.cc`，M12 不启用）。runner 把它编译为独立 archive，向 driver传入临时 `-L`，不能把测试函数偷偷加入 Scoop runtime。

- scalar：Int/UInt/Boolean/Unit、显式/缺省 symbol、多个 extern library；
- CLayout：struct 参数、struct 返回、nested/aligned/packed、C→Scoop→C 往返字节一致；
- pointer：malloc/free extern、Ptr offset/load/store/cast、nullable pointer、addressOf parameter/local/global/value-type `this`；通过 `addressOf(this)` 改写 method-local copy 不得改变原 receiver 或 boxed payload；
- callback：同一顶层 `@NoGC ::name` 同时作为 managed 函数值调用和 native callback 使用；C→NoGC Scoop→C，重载目标按 expected `FunPtr` 精确选择，另覆盖 null callback；
- global：extern val、extern global var、extern TLS、本地 Global/TLS 的读写与 addressOf；
- managed ABI：core `write(String)` 直接进入 Scoop ABI runtime symbol；另用 native root slot跨一次强制 GC 保活并 reload一个 direct ref，验证 root登记、扫描、移除与 reload路径；未来 moving collector沿同一 slot写回新地址而无需改变 ABI。pin/handle public API仍只在显式请求稳定地址/长期保活时要求 unsafe context；
- 组合：suspend function 内在 `@Unsafe` block 调 ordinary extern，验证它是普通调用而非 resume point；M1–M11 全部 fixture 原样通过。

### 6.4 negative / runtime-error

- ref / ordinary struct / enum / tuple 穿越 C ABI、含 managed ref的 value aggregate穿越 M12 Scoop ABI、CLayout 间接含 ref或嵌套非 CLayout、非法 pack/align；
- safe context 调 C extern或指针操作、Safe block嵌套后仍调用 unsafe、InteriorMutable 在 safe signature/body 中使用；
- NoGC body分配/装箱/字符串/异常/检查型操作/managed call，NoGC class method；
- addressOf rvalue/field/array element，Ptr pointee非 GC-free，非空 FunPtr直接构造；
- `FunPtr` native-address resolution 的目标为 generic/local/member/extension/suspend/non-NoGC，向 `FunPtr` 位置传入 lambda/匿名函数/绑定引用、已有 managed 函数值或无 expected type时已经推导出的 `::name`；把 M12 callback 保存后异步调用、从 foreign thread调用，或把 managed closure直接交给 C 均不是合法 fixture，并明确指向 M13 managed callback协议；
- extern 有 body、generic、method、suspend、varargs、unsupported stdcall；global缺 annotation、动态 initializer或 ref type。

## 7. 实现顺序与验收门

1. **通用注解与 effect 基线**：AST/parser、typed attributes、target/coexistence、unsafe blocks、NoGC checker；不改变现有 intrinsic行为，M1–M11 回归；
2. **kind bound 与 pointer core**：`value/ref`、FfiCore、Ptr/FunPtr类型、addressOf/sizeOf/alignOf/raw memory instructions；
3. **CLayout**：统一 layout、packed alignment、C-FFI-safe classifier与 C static-assert 生成；
4. **extern function + linking**：typed extern、C ABI bridge、Scoop ABI direct managed call、native root frame、library options，完成 scalar/struct C ABI fixture与 direct-ref Scoop ABI fixture；
5. **global/TLS**：最小顶层存储、extern global helpers、addressOf global；
6. **callback**：复用 M11 `FunctionTypeId` 与函数声明引用语法、按 expected type选择 native-address resolution、NoGC bridge与 C trampoline；
7. **core迁移与完整回归**：GC public API安全化、write intrinsic退役、组合/negative/golden、M1–M11全量。

每一步必须先完成对应 unit/golden，再进端到端；bridge 只支持 scalar 不算完成第 4 步，只有 extern call 没有 C→Scoop callback 不算完成 M12。

## 8. 临时决策与明确不做

1. **只支持 64 位 host + cdecl**：cross target、x86 stdcall/fastcall/vectorcall、Windows DLL import/export与 SEH 后续统一设计。
2. **C bridge 只属于 C ABI**：C ABI extern首版都经过 storage-pointer bridge；其 bridge合并与零拷贝是后续优化。Scoop ABI必须直接使用 Scoop typed ABI，不能为了复用 bridge而擦除 managed ref。
3. **顶层存储不是完整 property**：仅 extern global及常量初始化的显式 Global/TLS；动态初始化、引用根、const、object/companion仍留原 backlog。
4. **FunPtr 不是 managed function value**：它只保存 native callback 地址、不可直接 invoke、仅支持同步同线程 callback。`::name` 只是两条 resolution 路径共享的源码语法，不表示两类值可互相转换；M11 的 lambda/closure 虽然是正式函数值，仍不能转换为 `FunPtr`。M13 另行引入 callback token、类型化 adapter和 C trampoline的组合，不改变此规则。
5. **不支持 suspend FFI**：没有 hidden continuation导出、自动wrapper或异步callback适配。
6. **不导入 C header**：用户手写声明；header parser/bindgen是独立工具，不进入编译器 core。
7. **不解决 foreign unwind**：维持 runtime spec 第5章边界；M12 extern按 nounwind契约。
8. **M12 的 Ptr pointee必须 GC-free**：以后若要指向含 managed ref 的 pinned payload，必须先设计写屏障、移动 GC与 lifetime契约，不能仅放宽一个 HIR检查。
9. **自定义 annotation class不在 M12**：本里程碑只交付编译器核心注解；9.4 的通用注解声明/插件消费另立里程碑。

## 9. 文档同步项

- **spec 8.1 / 8.2 / 13.4 / 13.10 / 14.2**：复用正式函数类型与中性函数声明引用语法，按期望类型类别区分 managed callable reference/native function address，并固定两者边界及 suspend FFI禁令；M12实现不得放宽；
- **spec 13.2**：实现时把 NoGC 的 generic 条件、隐式异常操作与非 suspend约束写成明确规则；
- **spec 13.4–13.10 / 14.2–14.4**：同步 M12 的 extern top-level/non-generic子集、C-FFI-safe与Scoop-ABI-safe分类、direct ref/native root契约、Ptr pointee GC-free限制、CLayout pack/align算法、FunPtr同步同线程边界；
- **impl spec 2.1–2.5 / 2.7**：同步通用 annotation AST、HIR typed attributes/effects、MIR extern/global实体、LIR bridge描述、按 effect设置statepoint及driver native link输入；
- **runtime spec 3/4/8**：同步 NoGC callback不参与GC、Scoop ABI direct ref与native root frame、direct `write(String)` 及无suspend FFI runtime入口；managed closure registration和 foreign-thread入口明确交给 M13；
- **ROADMAP M12/M13及backlog**：链接本文；顶层属性 backlog注明M12只完成显式FFI存储子集；M7的write intrinsic项在实现完成后勾销；M9线程协调与M10跨线程恢复并入M13。
