# Scoop 语言规范 第 5 部分：静态反射与注解

版本：0.1 草案

本部分定义静态反射 intrinsic、静态元数据类型、class literal、annotation class、注解使用和内建注解。标准库中的宏式 helper、序列化库、测试框架等不属于本文范围。

## 1. 静态反射总览

Scoop 0.1 保留编译器拥有的静态反射 intrinsic，用于查询类型名、布局信息和有限的声明元数据。

当前规范不定义通用源码展开或解释执行语法。静态反射 intrinsic 是普通 `@Intrinsic fun` 声明面，编译器可在 HIR/MIR/codegen 中用类型实参和已发布元数据直接 lowering。

静态反射主要覆盖：

- 类型名、大小、对齐、布局分类和描述符地址。
- 字段、enum variant、父类型、注解和函数参数的元数据列表。
- Splice field access：`value.[field]`。
- 目标平台查询和最小运行期类型信息。

## 2. 静态反射 Intrinsic

核心反射 intrinsic 由 sysroot 以普通 intrinsic 函数声明：

```kotlin
@Intrinsic
fun <T> nameOf(): String

@Intrinsic
fun <T> sizeOf(): Int

@Intrinsic
fun <T> alignOf(): Int

@Intrinsic
fun <T> kindOf(): Int

@Intrinsic
fun <T> descOf(): UIntPtr

@Intrinsic
fun <T> fieldsOf(): MetaList<FieldMeta>
@Intrinsic
fun <T> variantsOf(): MetaList<VariantMeta>
@Intrinsic
fun <T> superTypesOf(): MetaList<TypeMeta>
@Intrinsic
fun <T> annotationsOf(): MetaList<AnnotationMeta>
@Intrinsic
fun paramsOf(fn: FunctionMeta): MetaList<ParamMeta>
```

规则：

- 反射 intrinsic 必须由编译器或可信 sysroot 声明提供，不允许普通用户源码自行声明。
- 类型实参必须能在调用点确定。
- `sizeOf<T>()` / `alignOf<T>()` 返回目标 ABI 下的布局数值。
- `kindOf<T>()` 返回 sysroot 中定义的稳定布局分类常量。
- `descOf<T>()` 返回运行时类型描述符地址；不需要描述符的类型可返回 `0`。
- `fieldsOf` / `variantsOf` / `superTypesOf` / `annotationsOf` / `paramsOf` 返回静态元数据列表；具体列表 API 属于 sysroot/工具链契约。

## 3. 静态元数据类型

概念类型：

```kotlin
class MetaList<T>

struct FieldMeta {
    val name: String
    val type: TypeMeta
    val index: Int
    val annotations: MetaList<AnnotationMeta>
}

struct VariantMeta {
    val name: String
    val fields: MetaList<FieldMeta>
    val index: Int
    val annotations: MetaList<AnnotationMeta>
}

struct TypeMeta {
    val name: String
    val kind: TypeKind
    val annotations: MetaList<AnnotationMeta>
}

struct ParamMeta {
    val name: String
    val type: TypeMeta
    val index: Int
    val annotations: MetaList<AnnotationMeta>
}

struct FunctionMeta {
    val name: String
}
```

`TypeKind` 至少区分：

- `Struct`
- `Enum`
- `Class`
- `Interface`
- `Effect`
- `Tuple`
- `Primitive`

具体枚举命名可由 sysroot 暴露，但语言语义需要上述分类能力。

## 4. Splice Field Access

`.[field]` 用静态字段描述访问字段：

```kotlin
fun getByName(p: Point): Int {
    return p.["x"]
}

fun getByMeta(p: Point, field: FieldMeta): Int {
    return p.[field]
}
```

规则：

- `field` 可以是字符串字面量或能静态确定 `name` 的 `FieldMeta`。
- 编译器把 `value.[field]` 降为具体字段访问并发布静态字段 contract。
- 对非字段元数据或未知字段使用 splice 是编译错误。
- Splice field access 不提供动态运行期字段查找。

## 5. Platform Introspection

目标平台可表示为：

```kotlin
struct Platform {
    val triple: String
    val arch: String
    val vendor: String
    val os: String
    val env: String
}

@Intrinsic
fun getPlatform(): Platform
```

规则：

- `getPlatform()` 返回当前编译/执行环境可见的平台描述。
- `triple` 格式遵循 LLVM target triple 约定；验证细节实现定义。
- 平台选择应尽量通过包/构建层源选择完成，不应引入语言预处理器。

## 6. Runtime Type Info

Scoop 0.1 只提供引用类型的最小运行期类型信息：

```kotlin
someObj is User
someObj as User
someObj as? User
someObj.typeName
```

规则：

- `is` / `as` / `as?` 只对引用类型运行期类型测试。
- Smart cast 见第 3 部分。
- `typeName` 提供运行期类型名。
- 不提供动态字段访问、动态方法调用或动态实例创建。

## 7. Class Literal

Class literal 写作：

```kotlin
String::class
my.pkg.TypeName::class
```

规则：

- 左侧必须是类型名路径。
- 在注解参数中可作为静态参数。
- 在 v0 语义中可视为稳定类型名或 `TypeMeta` 输入。
- 它不引入 Kotlin/JVM 风格运行期反射对象模型。

## 8. 注解总览

注解为声明、类型、字段、参数、属性、局部变量和表达式附加静态元数据。

```kotlin
@Deprecated("Use newFoo() instead", replaceWith: "newFoo")
fun foo() { ... }

@Inline
fun fastPath(x: Int): Int = x * 2
```

规则：

- 注解值是编译器可读取的静态元数据。
- 普通注解不引入运行期对象。
- 注解可被编译器内建消费，也可通过静态反射读取。
- 注解不能改变控制流语义，除非本规范对内建注解明确规定。

## 9. Annotation Class

注解声明：

```kotlin
annotation class Deprecated(
    val message: String = "",
    val replaceWith: String = ""
)

annotation class Extern(val lib: String = "", val name: String = "")
annotation class CLayout(val aligned: Int = 0, val packed: Int = 0)
```

规则：

- 只能写作 `annotation class`。
- Annotation class 不是普通 class 功能；它定义注解名和静态 payload。
- 不允许类型参数、effect 参数或 `where` 子句。
- 构造参数必须是 `val`。
- 参数可有默认值。
- 无默认值参数在使用处必须提供。
- 不允许 supertypes、类型体、次构造器、方法、计算属性或接口实现。
- 不允许在非注解位置用构造器语法实例化 annotation class。

允许的参数类型：

- `String`
- `Int`
- `Float` / `Float64` / `Float32` 的规范可表示子集
- `Bool`
- Enum value
- Class literal / 类型元信息常量
- 前述类型的 `Array<T>`
- 其它 annotation type

## 10. 注解使用

注解放在目标前，使用 `@`：

```kotlin
@Deprecated("old")
fun oldFun() {}

@Deprecated(message = "old", replaceWith = "newFun")
fun oldFun2() {}
```

参数规则：

- 第一个参数可位置传入。
- 后续参数必须命名。
- 可混合“第一个位置参数 + 后续命名参数”。
- 注解参数必须是静态参数表达式。

### 10.1 注解目标

注解可用于：

- 函数和方法，包括 extension 函数。
- 类型：`struct`、`class`、`enum`、`interface`、`object`。
- 字段，包括 struct/class 字段和 enum variant 字段。
- 属性。
- 函数参数，包括 receiver 参数。
- 类型参数。
- 局部变量。
- 表达式。
- 模块/文件。
- Annotation class 自身。
- Enum variant。
- 构造器参数。

### 10.2 Use-Site Target

当一个语法声明对应多个底层目标时，可用 use-site target：

```kotlin
class Config(
    @property:Serialization.Rename("db_url") val dbUrl: String,
    @param:Validated val port: Int
)
```

可用前缀：

- `field:`
- `property:`
- `param:`
- `get:`
- `set:`
- `file:`

未指定时，注解应用到声明的主目标；例如 `val` / `var` 默认是 property，`fun` 默认是 function。

## 11. 命名空间注解

注解可嵌套在 `object` 中用于逻辑分组：

```kotlin
object Serialization {
    annotation class Rename(val key: String)
    annotation class Ignore
}

struct User {
    @Serialization.Rename("created_at")
    val createdAt: String

    @Serialization.Ignore
    val cacheKey: String
}
```

使用 dot-path：

```kotlin
@Namespace.AnnotationName(args...)
```

## 12. 内建注解

编译器识别以下内建注解：

| 注解 | 目标 | 语义 |
|---|---|---|
| `@Intrinsic` | 函数、类型 | 实现由编译器或运行时提供 |
| `@Extern(lib?, name?)` | 函数、顶层变量 | 外部符号，见第 6 部分 |
| `@Deprecated(message, replaceWith)` | 函数、类型、属性 | 使用处发 warning |
| `@Inline` | 函数 | 内联优化提示 |
| `@TailRec` | 函数 | 要求尾递归优化；否则编译错误 |
| `@AllowIntrinsic` | 文件/模块 | 允许可信源码声明 intrinsic surface |
| `@Suppress(warnings...)` | 表达式、声明、文件 | 抑制指定 warning |
| `@Experimental(feature = "...")` | 函数、类型、属性、文件 | 保留 feature-gate marker |
| `@CLayout(aligned?, packed?)` | Struct | C 兼容布局，见第 6 部分 |
| `@ThreadLocal` | 顶层 `var` | 线程局部 mutable global |
| `@Global` | 顶层 `var` | 进程全局 mutable global |
| `@CallingConvention(name)` | 函数、函数指针别名 | FFI 调用约定 |
| `@NoGC` | 函数 | 禁止 GC-managed heap 分配 |
| `@Unsafe` | 函数、`do` block | 允许 unsafe 操作 |
| `@Safe` | 函数、`do` block、closure | 在 unsafe 上下文中重新建立 safe 区域 |
| `@Target(targets...)` | Annotation class | 限制注解目标 |
| `@Retention(policy)` | Annotation class | 控制注解是否保留到 `.cone` |

### 12.1 AnnotationTarget

```kotlin
enum AnnotationTarget {
    Function,
    Property,
    Field,
    Param,
    Type,
    Constructor,
    LocalVariable,
    Expression,
    Module,
    TypeParam,
    EnumVariant,
}
```

`@Target` 使用这些 enum 值限制目标。

### 12.2 `@Retention`

`@Retention(policy)` 目前支持两档：

- `"local"`：只在当前源码边界内可见，不导出到 `.cone`。未显式标记时采用该策略。
- `"cone"`：保留到 `.cone` 元数据，使下游可见。

### 12.3 `@Suppress`

规则：

- 参数是一个或多个位置字符串字面量。
- 不支持命名参数。
- `@file:Suppress(...)` 作用于整个文件。
- 声明上的 `@Suppress` 作用于声明范围。
- 表达式上的 `@Suppress` 作用于检查该表达式期间产生的 warning。
- 稳定 warning code 至少包括：`deprecated`、`enum-size-disparity`、`redundant-when-else`。

### 12.4 `@Experimental`

规则：

- 使用形态固定为 `@Experimental(feature = "...")`。
- `feature` 必须是字符串字面量。
- 当前只保留 marker 和参数校验；不会自动启用或禁用具体语言特性。

## 13. 静态注解元数据

所有可注解元素的元数据都携带 `annotations`：

```kotlin
struct AnnotationMeta {
    val name: String
    val args: MetaList<AnnotationArgMeta>
}

struct AnnotationArgMeta {
    val name: String
    val value: Any
}
```

查询方式：

| 元素 | 访问方式 |
|---|---|
| 类型 | `annotationsOf<T>()` |
| 字段 | `field.annotations` |
| Enum variant | `variant.annotations` |
| 函数参数 | `param.annotations` |
| 函数 | `annotationsOf(fn)` |

## 14. `@Intrinsic` 与 Sysroot 声明

`@Intrinsic` 标记由编译器或运行时提供实现的声明：

```kotlin
@Intrinsic
struct Int {
    fun toString(): String
}

@Intrinsic
fun println(value: String)
```

规则：

- Intrinsic 声明有签名但可无 Scoop 函数体。
- Intrinsic 声明通常位于 sysroot，供编译器、工具链、LSP 和文档读取。
- 用户源码默认不能声明 `@Intrinsic`。
- 需要声明 intrinsic surface 的文件必须显式 opt in，例如 `@file:AllowIntrinsic`。
- `@Intrinsic` 不自动意味着 unsafe 或 NoGC；具体约束由该 intrinsic 声明和内建规则决定。

完整 sysroot API 不属于本文档范围；本文只固定 intrinsic 作为语言/编译器边界的声明机制。
