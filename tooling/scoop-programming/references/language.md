# 容易误用的语言规则

这些要点用于选择正确写法；具体程序优先参考同版本 `scoop example`。仓库开发环境还可查 `docs/specs/SCOOP-SPEC.md`；安装包中以配套示例、库声明、API 文档和实际诊断为依据，不要求存在编译器仓库。

## Option 与控制流

`T?` 是 `Option<T>`，存在值写 `Some(value)`，缺失写 `None`。没有 `null` 字面量，也没有 `value != null` 后自动变为 T 的智能转换。解构使用 `case`：

```scoop
fun greeting(name: String?): String =
    when (name) {
        case Some(value) -> f"Hello, ${value}!"
        case None -> "Hello, guest!"
    }
```

`?.` 产生可选结果，`?:` 提供缺失分支；`!!` 会在缺失时抛出异常，只在实际业务保证存在时使用。`Option` 不变，`T??` 是嵌套两层；安全调用不自动展平原本已经可选的返回值。使用有结果的 `when` 时处理所有变体。更多示例：`language/option`、`language/enum`。

## 值类型、引用与能力

- `struct Point(val x: Int, val y: Int)` 是不可变值，存储字段只能用 `val`。更新写 `point.{x: 8}`，产生副本；不使用 Kotlin 的 `data class` 或 `.copy(x = 8)`。
- `enum Lookup { Found(String), Missing }` 是带标签的值类型；构造写 `Lookup.Found("Ada")`，匹配写 `case Lookup.Found(name)`。它不是 `enum class`。
- tuple、struct 和 enum 的复制都是值复制；其中的引用字段仍指向同一对象。`val` 禁止重新绑定，不禁止修改引用对象。
- `==` 的结构比较不自动让自定义值类型实现 `Equals<T>`、`Hash` 或 `ToString`。字典 key、泛型断言、插值和序列化需满足各入口实际的名义约束或成员要求；先查声明，再按需声明 conformance 或提供公开方法。
- `===`／`!==` 仅比较引用身份。不要用它们比较值类型。

完整构造、解构、接口化和副本更新见 `language/struct`、`language/interfaces`、`language/overview`。

## 字符串、数值与容器

- 插值使用 `f"Hello, ${name}!"`；普通字符串的 `${name}` 保留原文，不使用 Kotlin 的 `$name` 插值。
- `Char` 表示 Unicode 标量。`String.charLength`、常用容器 `size` 与索引使用 `Long`，例如 `values[0L]`。UTF-8 字节、Unicode 标量、字素簇和终端显示列宽是不同单位。
- 依据 API 使用 `Int`、`Long` 等准确类型；不要假设可隐式转换任意数值变量。需要 Long 常量时用 `0L` 等写法。
- `Array<T>` 只读，数组字面量写 `[7, 9]`；固定长度可写存储使用 `MutableArray<T>`，动态增删使用 `ArrayList<T>`。
- `toArray()` 创建元素序列的浅快照；之后修改原列表不会改变快照序列，但元素引用的对象仍可共享。`List` 等协议和扩展方法的可用性以实际声明及 import 为准。

参考 `text/strings`、`collections/list`、`collections/map`；需要字节、日期或文本算法时再查 `scoop.std` 的配套示例。

## 函数、闭包与协程

- 表达式体 `fun twice(x: Int): Int = x * 2` 直接给结果；普通块体函数需要显式 `return` 返回非 Unit 结果。参数 binding 不可重新赋值。
- lambda 的末尾表达式为结果；裸 `return` 非法，局部提前返回用明确标签，例如 `adjust@ { x -> if (x < 0) return@adjust 0; x * 2 }`。
- 闭包只捕获外层局部 `val`，不能读取或写入捕获的局部 `var`。快照先保存为 `val`；共享可变状态放在由 `val` 引用的显式对象中。跨线程共享仍需要同步，GC 不提供数据竞争保障。
- 函数值调用按函数类型使用位置参数，不保留原声明的命名参数、默认参数或 vararg 调用便利。
- 挂起 lambda 显式写 `suspend { ... }`，不会由期望类型自动转换。普通 `main` 需要通过实际事件循环入口进入挂起上下文，不能直接调用挂起函数，也不引入 `kotlinx.coroutines` API。
- `suspend` 不等于后台线程或并行执行。任务取消后仍按 API 使用 `join`／`await` 等待收尾；不把 sleep 当同步协议。

参考 `language/functions`、`language/lambdas`、`async/tasks`。异步 API 的完整入口与资源管理见 [libraries.md](libraries.md)。

## 可见性与泛型

声明默认 `internal`，边界是 Cone。公开类型、构造器、成员及 companion 按各自规则显式公开；`public class` 不会自动公开其构造器，`public interface` 也不会自动公开全部成员。接口实现显式声明 conformance，并按合同写 `public override`；同名方法不等于实现接口。

名义泛型不变，`Box<Child>` 不自动成为 `Box<Parent>`；不使用 Kotlin 的 `in`／`out` 投影或 `*`。类型位置写完整类型参数，调用点可按规则推断。interface 方法本身不能带方法级类型参数。需要复杂泛型约束或条件实现时查当前声明、规范和诊断，不凭 Kotlin 经验构造签名。

跨 Cone 示例见 `language/visibility`；泛型和 companion 见 `language/generics`、`language/companion`。公开 codec 的示例见 `json/typed`。

## 错误与确定性释放

用普通 `try`／`catch`／`finally` 表达错误和收尾。`Exception.message` 是可选值，必要时写 `error.message ?: "fallback"`。从示例取得异常构造器签名，不假设它接受裸 String。

文件、流、连接、响应和 native 句柄按其 API 使用 `use`、`useAsync`、专门的作用域函数或 `try/finally` 关闭。GC 不代替确定性释放，`val` 也不表达资源所有权。普通应用优先使用公开库，只有实际 native 互操作需求才进入指针、FFI 和 GC 契约。
