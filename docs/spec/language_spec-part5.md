# Scoop 语言规范 第 5 部分：编译期执行、静态反射与注解

版本：0.1 草案

本部分定义 `const fun`、`const val`、`comptime`、静态反射、class literal、annotation class、注解使用和内建注解。标准库中的宏式 helper、序列化库、测试框架等不属于本文范围。

## 1. 编译期执行总览

Scoop 提供编译期执行能力，用于类型安全代码生成和元编程：

- `const fun`：可在编译期求值的函数。
- `const val`：编译期常量。
- `comptime if`：编译期分支选择。
- `comptime for`：编译期迭代展开。
- 静态反射 intrinsic：`fieldsOf<T>()`、`nameOf<T>()` 等。
- Splice field access：`value.[field]`。

Scoop 不引入单独宏系统。编译期功能沿用普通类型检查、普通函数语法和受限执行子集。

## 2. `const fun`

```kotlin
const fun add(a: Int, b: Int): Int = a + b

const val X: Int = add(1, 2)
val y = add(runtimeA, runtimeB)
```

规则：

- `const fun` 可在编译期求值。
- `const fun` 也可在运行期调用；`const` 是能力标记，不是“只能编译期调用”。
- `const fun` 的声明级 effect contract 必须省略，或显式 `/ Pure` / `/ Pure!`。
- `const fun` 不能声明 effect row 参数，例如 `<eff E = ...>`。
- `const fun` 当前不是 effect-polymorphic surface。

这个限制保证 `const fun` 可被编译期解释器执行。Effectful 或 effect-polymorphic `const fun` 留待未来规范化。

### 2.1 `const fun` 允许内容

允许：

- 调用其它 `const fun`。
- 调用编译器 intrinsic，例如 `fieldsOf`、`nameOf`、`sizeOf`、`alignOf`。
- 局部 `val` / `var`。
- `if`、`when`、`for`、`while`。
- 值类型运算、构造和解构。
- 整数、浮点、布尔、字符、tuple、struct、enum。
- `String` 操作；`String` 虽是运行期引用类型，但编译期按不可变内容值处理。
- `comptime if` / `comptime for`。
- 可由编译期解释器证明终止和纯净的普通表达式子集。

### 2.2 `const fun` 禁止内容

禁止：

- 调用非 `const fun`。
- 声明或执行 non-Pure effect。
- 声明 effect row 参数。
- 访问全局可变状态。
- 创建普通 class/object 实例；运行期堆分配不可用于编译期解释。`String` 是特例。
- 创建依赖捕获环境的 closure/lambda；闭包捕获集合难以静态验证。
- 使用 unsafe 原语、FFI 调用或 GC pin/handle。

## 3. `const val`

```kotlin
const fun triple(x: Int): Int = x + x + x

const val Base: Int = 3
const val Value: Int = triple(Base)
const val Label: String = "top".concat("-level")
```

规则：

- initializer 在编译期求值。
- initializer 必须满足 `const fun` 同等纯计算限制。
- `const val` 可被普通运行期代码读取。
- 编译器可把读取替换为常量值。
- `const val` 可作为注解参数和其它编译期表达式输入。
- 与普通顶层 `val` 不同，`const val` 没有运行期 once-init。

## 4. `comptime if`

`comptime if` 在编译期选择分支，未选择分支不参与后续类型检查/导出。

函数体内：

```kotlin
fun <T> serialize(value: T): String {
    comptime if (T is struct) {
        return serializeStruct(value)
    } else comptime if (T is enum) {
        return serializeEnum(value)
    } else {
        return value.toString()
    }
}
```

顶层：

```kotlin
comptime if (getPlatform().os == "windows") {
    fun platformName(): String = "windows"
} else {
    fun platformName(): String = "posix"
}
```

规则：

- 条件必须是编译期可求值的 `Bool`。
- 只保留被选中的分支。
- 未选中分支不进入名称解析、类型检查和公共 API 导出。
- 顶层 `comptime if` 分支体内只能出现顶层 items。
- 支持 `else comptime if`，也可写成 `else if` 的语法糖，只要语义为编译期分支。

## 5. `comptime for`

`comptime for` 遍历编译期集合并展开循环体：

```kotlin
fun <T> debugPrint(value: T) {
    print(f"{nameOf<T>()}(")
    comptime for (field in fieldsOf<T>()) {
        print(f"{field.name}={value.[field]}, ")
    }
    println(")")
}
```

规则：

- 迭代对象必须是编译期可枚举集合，例如 `ComptimeList<FieldMeta>`。
- 循环体在编译期按元素展开。
- 每次迭代的 binder 是编译期元数据值。
- 未展开的抽象 `comptime for` 不存在于运行期。

## 6. 静态反射 intrinsic

反射 intrinsic 隐式为 `const fun`，返回编译期数据结构。

类型反射：

```kotlin
const fun <T> fieldsOf(): ComptimeList<FieldMeta>
const fun <T> variantsOf(): ComptimeList<VariantMeta>
const fun <T> nameOf(): String
const fun <T> sizeOf(): Int
const fun <T> alignOf(): Int
const fun <T> superTypesOf(): ComptimeList<TypeMeta>
const fun <T> annotationsOf(): ComptimeList<AnnotationMeta>
const fun paramsOf(fn: FunctionMeta): ComptimeList<ParamMeta>
```

规则：

- 在编译期上下文中，intrinsic 由编译器求值并嵌入结果。
- 在运行期上下文中，如果调用没有被要求编译期求值，语义上是普通调用；实现可用内建 lowering 或常量嵌入。
- 反射主要用于静态代码生成，不提供完整运行期 reflection。

## 7. 编译期元数据类型

概念类型：

```kotlin
struct FieldMeta {
    val name: String
    val type: TypeMeta
    val index: Int
    val annotations: ComptimeList<AnnotationMeta>
}

struct VariantMeta {
    val name: String
    val fields: ComptimeList<FieldMeta>
    val index: Int
    val annotations: ComptimeList<AnnotationMeta>
}

struct TypeMeta {
    val name: String
    val kind: TypeKind
    val annotations: ComptimeList<AnnotationMeta>
}

struct ParamMeta {
    val name: String
    val type: TypeMeta
    val index: Int
    val annotations: ComptimeList<AnnotationMeta>
}

struct FunctionMeta {
    val name: String
    val params: ComptimeList<ParamMeta>
    val returnType: TypeMeta
    val annotations: ComptimeList<AnnotationMeta>
}
```

`ComptimeList<T>` 是编译期-only 列表类型，不能出现在普通运行期值中。

`TypeKind` 至少区分：

- `Struct`
- `Enum`
- `Class`
- `Interface`
- `Tuple`
- `Primitive`
- `Object`
- `Function`

具体枚举命名可由 sysroot 暴露，但语言语义需要上述分类能力。

## 8. Splice field access

`.[field]` 用编译期 `FieldMeta` 访问字段：

```kotlin
comptime for (field in fieldsOf<T>()) {
    val fieldValue = value.[field]
}
```

规则：

- 只在 `comptime for` 或等价编译期上下文中合法。
- `field` 必须是当前编译期迭代中的 `FieldMeta`。
- 编译器在展开时把 `value.[field]` 替换为具体字段访问。
- 对非字段元数据使用 splice 是编译错误。

## 9. Platform introspection

目标平台可表示为：

```kotlin
struct Platform {
    val triple: String
    val arch: String
    val vendor: String
    val os: String
    val env: String
}

const fun getPlatform(): Platform
```

规则：

- 编译期求值时，`getPlatform()` 返回编译目标平台。
- 运行期调用时，返回当前执行环境平台。
- `triple` 格式遵循 LLVM target triple 约定；验证细节实现定义。
- 平台选择应尽量通过包/构建层源选择完成，不应引入语言预处理器。

## 10. Runtime type info

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
- 泛型代码生成和序列化等场景应使用静态反射与 `const fun`。

## 11. Class literal

Class literal 写作：

```kotlin
String::class
my.pkg.TypeName::class
```

规则：

- 左侧必须是类型名路径。
- 在注解参数中可作为编译期常量。
- 在 v0 语义中可视为稳定类型名或 `TypeMeta` 输入。
- 它不引入 Kotlin/JVM 风格运行期反射对象模型。

## 12. 注解总览

注解为声明、类型、字段、参数、属性、局部变量和表达式附加编译期元数据。

```kotlin
@Deprecated("Use newFoo() instead", replaceWith: "newFoo")
fun foo() { ... }

@Inline
fun fastPath(x: Int): Int = x * 2
```

规则：

- 注解值存在于编译期。
- 普通注解不引入运行期对象。
- 注解可被编译器内建消费，也可通过静态反射读取。
- 注解不能改变控制流语义，除非本规范对内建注解明确规定。

## 13. Annotation class

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
- Annotation class 不是普通 class 功能；它定义注解名和编译期 payload。
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

## 14. 注解使用

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
- 注解参数必须是编译期常量表达式。

### 14.1 注解目标

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

### 14.2 Use-site target

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

## 15. 命名空间注解

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

## 16. 内建注解

编译器识别以下内建注解：

| 注解 | 目标 | 语义 |
|---|---|---|
| `@Intrinsic` | 函数、类型 | 实现由编译器或运行时提供 |
| `@Extern(lib?, name?)` | 函数、顶层变量 | 外部符号，见第 6 部分 |
| `@Deprecated(message, replaceWith)` | 函数、类型、属性 | 使用处发 warning |
| `@Inline` | 函数 | 内联优化提示 |
| `@TailRec` | 函数 | 要求尾递归优化；否则编译错误 |
| `@AllowIntrinsic` | 文件/模块 | 允许用户源码声明 intrinsic surface |
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
| `@Retention(policy)` | Annotation class | 控制注解保留策略 |

### 16.1 AnnotationTarget

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

### 16.2 `@Suppress`

规则：

- 参数是一个或多个位置字符串字面量。
- 不支持命名参数。
- `@file:Suppress(...)` 作用于整个文件。
- 声明上的 `@Suppress` 作用于声明范围。
- 表达式上的 `@Suppress` 作用于检查该表达式期间产生的 warning。
- 稳定 warning code 至少包括：`deprecated`、`enum-size-disparity`、`redundant-when-else`。

### 16.3 `@Experimental`

规则：

- 使用形态固定为 `@Experimental(feature = "...")`。
- `feature` 必须是字符串字面量。
- 当前只保留 marker 和参数校验；不会自动启用或禁用具体语言特性。

## 17. 编译期注解访问

所有可注解元素的元数据都携带 `annotations`：

```kotlin
struct AnnotationMeta {
    val name: String
    val args: ComptimeList<AnnotationArgMeta>
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

示例：

```kotlin
const fun hasAnnotation(
    annotations: ComptimeList<AnnotationMeta>,
    name: String
): Bool {
    comptime for (ann in annotations) {
        comptime if (ann.name == name) { return true }
    }
    return false
}
```

## 18. `@Intrinsic` 与 sysroot 声明

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
