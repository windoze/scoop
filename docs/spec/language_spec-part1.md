# Scoop 语言规范 第 1 部分：总览、源文件、包与词法

版本：0.1 草案

本文档组把 `SCOOP_FULL_SPEC.md` 中的语言规范整理为中文分卷，并把原文中“遵循 Kotlin 语义”的部分展开为 Scoop 自身规范。本文档组只覆盖语言本体：语法、类型系统、效果系统、静态反射、注解、FFI/unsafe 边界和程序入口。标准库 API 设计不在本文档组范围内；只有当某个语法必须依赖核心类型名时，才说明该核心类型在语言层面的契约。

分卷：

- 第 1 部分：总览、源文件、包与词法
- 第 2 部分：类型系统、泛型与名义结构
- 第 3 部分：表达式、函数、属性、模式匹配与推断
- 第 4 部分：效果系统与异常语法糖
- 第 5 部分：静态反射与注解
- 第 6 部分：unsafe、FFI、GC 互操作与程序边界

## 1. 语言目标与非目标

Scoop 是静态类型、GC 管理内存的编程语言。它提供 Kotlin 风格的声明与调用体验，同时脱离 JVM 限制，核心特性包括：

- 真正的值类型：`struct`、`enum`、tuple，具备 copy 语义且不可变。
- GC 管理的引用类型：`class`、`interface`、`object`、装箱后的值类型。
- 泛型单态化：泛型按具体类型实例生成专门代码。
- 代数效果系统：统一表达错误、异步、生成器和自定义控制流效果。
- 静态反射 intrinsic 与注解元数据。
- 明确的低层边界：`@Unsafe`、`@NoGC`、`@Extern`、`@CLayout`、GC pin/handle。

本规范不定义完整标准库。集合操作、IO、线程、时间、路径、网络、测试框架等库 API 的可用性、签名和性能语义由标准库规范另行定义。本文只定义语言语法、类型规则，以及语法糖需要依赖的最小核心类型契约。

## 2. 源文件结构

一个 `.scoop` 源文件由以下部分按顺序组成：

1. 可选的文件级注解，例如 `@file:Suppress("deprecated")`。
2. 可选的 `package` 指令。
3. 零个或多个 `import` 指令。
4. 零个或多个顶层声明。

示例：

```kotlin
@file:Suppress("deprecated")

package app.main

import scoop.core.*
import app.model.User as AppUser

val Version: Int = 1

fun main(): Int / Pure! {
    return 0
}
```

### 2.1 Package

每个源文件最多有一个 `package` 指令。包名是点分隔路径：

```kotlin
package scoop.collections
```

未声明 `package` 的文件位于默认包。包名参与全限定名构造和可见性判断。

### 2.2 Import

导入支持三种形态：

```kotlin
import scoop.collections.List
import scoop.collections.*
import app.model.User as AppUser
```

规则：

- 显式导入单个符号。
- `*` 导入包内公开符号。
- `as` 为导入符号提供本地别名。
- 显式导入优先于 `*` 导入，且不依赖 import 语句书写顺序。
- 同一源文件中，显式导入或别名产生的本地名必须唯一；与其它导入冲突是编译错误。
- 同文件顶层声明可遮蔽导入别名；名称解析时优先当前包/当前文件中更近的声明。

## 3. Cone 包模型的语言边界

Scoop 包称为 Cone。语言层面需要固定以下内容：

- 一个 Cone 是编译、可见性和依赖解析的边界。
- `public` 符号可跨 Cone 使用并进入公共 API。
- `internal` 符号只在同一 Cone 内可见。
- `private` 顶层符号只在同一源文件内可见。
- 源文件通过 `package` 与 `import` 构成可解析的源码图。
- 可执行 Cone 的入口由程序边界规则确定，通常是 `main` 函数；常规约定入口文件可命名为 `src/main.scoop`，但语言入口不是由文件名本身决定。

`.cone` 归档格式、IR 版本、预单态化缓存、二进制分发和构建系统策略是工具链契约，不属于本文的语言语义。`Cone.toml` 中的平台源选择是包管理/构建层能力；它不得向语言引入预处理器语义。

## 4. 顶层声明

源文件顶层允许：

- `fun`
- `val`
- 受限的 `var`
- `class` / `interface` / `struct` / `enum`
- `effect`
- `object`
- `annotation class`
- `typealias`

顶层不允许普通语句；例如顶层 `return` 是语法错误。

### 4.1 可见性修饰符

顶层声明、类型成员和对象成员可使用：

```kotlin
public
internal
private
```

规则：

- 未显式标注时默认 `public`，除非后续章节对特定声明另有规定。
- 三个可见性修饰符互斥；同时出现多个是编译错误。
- `public`：跨 Cone 可见。
- `internal`：同一 Cone 内可见。
- `private`：文件内或声明体局部可见；顶层 `private` 至少按 file-private 处理。

### 4.2 顶层不可变值

顶层 `val` 是运行期不可变值。它的 initializer 在首次读取时按 once-init 语义求值：

```kotlin
val base: Int = seed()
val next: Int = base + 1
```

规则：

- 普通顶层 `val` 的 initializer 只求值一次。
- 多次读取同一顶层 `val` 不会重复执行 initializer。
- 顶层 `val` 可被其它顶层 initializer、函数体和对象初始化过程读取。
- 顶层 `val` 初始化期间若递归读取自身，必须作为运行期错误处理，不能静默返回未初始化值。
- initializer 可以执行普通运行期计算；它不会自动内联成编译器常量。
- 简单顶层 `val name = expr` 需要显式类型注解：`val name: T = expr`。
- 顶层解构 `val pattern = expr` 可以从 initializer 推断整体类型；也可以写整体类型注解：`val (a, b): (Int, Int) = expr`。

顶层解构会把解构产生的 binder 加入顶层值命名空间：

```kotlin
val (left, right) = (20, 22)
val sum: Int = left + right

val Point { x, y }: Point = Point { x: sum, y: 1 }
```

顶层 `var` 见第 6 部分的全局可变状态规则；普通无标注顶层 `var` 是编译错误。

### 4.3 顶层可变值

顶层 `var` 只允许用于显式声明的全局存储：

```kotlin
@Global
var counter: Int = 0

@ThreadLocal
var threadCounter: Int = 0
```

规则：

- 顶层 `var` 必须显式标注 `@Global` 或 `@ThreadLocal`。
- 顶层 `var` 的类型必须满足第 6 部分定义的低层全局存储约束。
- 未标注的顶层 `var` 是编译错误。

## 5. 词法概览

### 5.1 标识符

标识符用于变量、类型、函数、包段、成员和注解名。关键字不能作为普通标识符使用，除非规范明确指定为上下文关键字。

`eff` 是上下文关键字：只在泛型参数/实参列表中引入 effect row 参数或实参时作为关键字。其它位置可以作为普通标识符。

`init` 和 `constructor` 在类型体的特定位置作为上下文结构使用，用于 class 初始化块和次构造器。

### 5.2 关键字

语言关键字包括：

```text
public internal private open abstract sealed inline override vararg annotation
package import typealias fun val var class interface struct enum effect object companion
handle with perform try catch finally
do return if else when for in out where while break continue is as as?
```

说明：

- `inline` 关键字已废弃；语言语义使用 `@Inline` 注解，见第 3 部分。
- `with` 保留给值类型 copy-update 表达式；不提供 Kotlin 的 `with(obj) { ... }` scope function 语法。
- `as?` 词法上作为安全 cast 关键字处理。

### 5.3 注释

支持行注释和块注释：

```kotlin
// line comment

/*
 block comment
*/
```

块注释必须闭合。嵌套块注释是否支持由实现定义；可移植源码不应依赖嵌套块注释。

## 6. 字面量

### 6.1 整数字面量

支持十进制、十六进制和二进制：

```kotlin
123
1_000
0xFF
0Xca_fe
0b1010
0B10_01
```

规则：

- `_` 只能出现在数字之间，不能位于开头、结尾或连续出现。
- `0x` / `0X` 后至少有一个十六进制数字。
- `0b` / `0B` 后至少有一个二进制数字。
- 字面量词法值至少支持到 `u128` 范围；最终能否赋给某个整数类型由类型检查决定。
- 负数不是单独的整数字面量；`-1` 是一元负号作用于 `1`。

默认整数字面量类型为 `Int`，但可被期望类型吸收为具体整数类型；若数值超出目标类型范围，编译错误。

### 6.2 浮点字面量

支持十进制小数、科学计数法和 Float32 后缀：

```kotlin
3.14
1_000.5
1e3
2.5E-4
1_2.3_4e5_6
0.5f
1.0f32
```

规则：

- 没有后缀时类型为 `Float64`。
- `f` 或 `f32` 后缀表示 `Float32`。
- 没有小数点且没有指数的 token 是整数字面量，不是浮点字面量。
- `_` 只能出现在数字之间。
- 指数部分的 `e` / `E` 后可有 `+` / `-`，但必须有至少一个数字。
- 字面量值必须在目标浮点类型可表示范围内。
- 无后缀 `Float64` 字面量可在 `Float32` 期望上下文中被吸收为 `Float32`，但不能超出 `Float32` 范围。

### 6.3 字符字面量

字符字面量表示单个 Unicode scalar value：

```kotlin
'A'
'中'
'\n'
'\t'
'\r'
'\\'
'\''
'\0'
'\u0041'
```

规则：

- 字符字面量必须恰好包含一个字符或一个合法转义。
- `\uXXXX` 使用四位十六进制码点。
- surrogate 等非法 Unicode scalar value 是词法错误。
- 空字符字面量 `''` 和多字符字面量 `'ab'` 是词法错误。

字符字面量类型为 `Char`。

### 6.4 Bool 与 Unit

布尔字面量：

```kotlin
true
false
```

类型为 `Bool`。

Unit 字面量：

```kotlin
()
```

`()` 是 0 元 tuple 的唯一值。无显式返回类型的函数隐式返回 `Unit`。

### 6.5 字符串字面量

普通字符串没有插值。`$`、`{`、`}` 是普通字符：

```kotlin
val price = "costs $100"
val json = "{ \"key\": \"value\" }"
val shell = "echo ${HOME}"
```

插值字符串使用 `f` 前缀，插值表达式写在 `{ ... }` 中：

```kotlin
val greeting = f"Hello, {name}!"
val result = f"sum = {a + b}"
val literal = f"use {{braces}} in f-string"
```

规则：

- 普通字符串不允许裸换行。
- 插值字符串中的 `{{` 和 `}}` 表示字面花括号。
- 插值表达式按普通表达式规则检查和求值。

Raw 字符串使用三引号，无转义处理，可跨行：

```kotlin
val sql = """
    SELECT *
    FROM users
""".trimIndent()

val text = f"""
    Hello, {name}
""".trimIndent()
```

`trimIndent()` 是字符串标准库函数。字符串 API 的完整标准库形态不在本文档范围内。

## 7. 名称与作用域

Scoop 有独立但可交互的命名空间：

- 类型命名空间：`class`、`interface`、`struct`、`enum`、`object` 的类型面、`typealias`、类型参数。
- 值命名空间：`val`、`var`、函数值、对象单例值、enum variant 构造值、参数和局部变量。
- 函数重载集合：同名函数可按参数签名形成 overload set。
- 成员命名空间：类型、对象和 companion object 的属性、函数、嵌套类型与嵌套对象。

规则：

- 同一非函数命名空间内重复定义同名符号是编译错误。
- 顶层函数和成员函数可重载；仅返回类型不同不能构成合法重载。
- 局部作用域允许遮蔽外层局部绑定。
- 类型参数遮蔽同名顶层类型。
- 函数体和顶层 initializer 可引用同文件后续顶层符号；解析按两阶段索引处理。
- class 初始化阶段有额外前向引用限制，见第 2 部分。

## 8. 标准库排除说明

本文档组不规定以下内容：

- 集合类型的完整成员 API 和算法复杂度。
- 字符串、数组、Map/Set/List 等标准库函数完整列表。
- IO、文件系统、网络、时间、线程、同步原语的库接口。
- 测试框架、包仓库、构建命令。
- Kotlin 标准库函数的完整复制。

若语言语法提到 `Option<T>`、`Array<T>`、`MutableArray<T>`、`RuntimeError`、`Continuation<...>`、`GC` 等核心名字，其目的只是说明语法脱糖、类型检查或运行时边界的最小契约，不等于定义完整标准库。
