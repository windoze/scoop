# Scoop 语言规范 第 4 部分：效果系统与异常语法糖

版本：0.1 草案

本部分定义代数效果、effect row、handler、continuation、`try` / `catch` / `finally`、运行期错误与程序边界的效果规则。

## 1. 总览

Scoop 使用代数效果系统统一表达：

- 可恢复或不可恢复的控制流效果。
- 错误处理。
- generator/yield 风格流程。
- 用户自定义效果。

效果是静态类型系统的一部分。每个表达式有 required effect row：在没有隐式 ambient handler 的前提下，求值该表达式可能需要外层允许或处理的效果集合。

## 2. Effect 声明

```kotlin
effect Raise<E> {
    fun raise(error: E): Nothing
}

effect Emit<T> {
    fun emit(value: T)
}
```

规则：

- `effect` 声明类似接口，包含 operation 签名。
- Operation 可以泛型化。
- Operation 可有参数和返回类型。
- Operation 的返回类型决定 resuming handler 的 resume payload 类型。
- `Nothing` 返回通常表示不可正常返回的 operation，例如 `Raise.raise`。

## 3. 调用效果

效果 operation 使用普通 qualified call：

```kotlin
fun fetchData(): String / Raise<IOError> {
    val resp = httpGet("/data")
    if (resp.status != 200) {
        Raise.raise(IOError("bad status"))
    }
    return resp.body
}
```

规则：

- `E.op(args...)` 的实参按源码从左到右求值。
- 若当前动态 handler 栈中有匹配 operation 的 handler，则 dispatch 给最近匹配 handler。
- 若没有 handler，效果成为 unhandled effect，必须由函数签名 required effect row 表示。
- 若 unhandled effect 到达程序边界，well-typed 程序应已阻止；实现可将其作为运行期 panic。

注：
- 当前版本不定义内建 task runtime surface，也不定义 `async` / `await`、用户可见 `spawn` / `join` 等语法；这些属于后续库/语言设计话题，不影响本部分对一般 effect/continuation 的规则。
- `perform` 前缀已移除；实现可保留该关键字为解析期错误以提示改写为普通 qualified effect operation call。

## 4. Effect row

Effect row 是编译期集合表达式：

```kotlin
/ Pure
/ Emit<Int>
/ Raise<IOError>
/ (Emit<Int> + Raise<IOError>)
/ E
/ (E + Raise<IOError>)
```

语法元素：

- `Pure`：空 row。
- `Emit<Int>`、`Raise<IOError>`：effect item。
- `E`：effect row 变量，由 `<eff E>` 引入。
- `R1 + R2`：row union。

代数规则：

- `+` 满足结合律、交换律、幂等律。
- `Pure` 是单位元。
- 泛型 effect 不做逆变或协变处理；Emit<Any> 不包含 Emit<String>，反之亦然。

### 4.1 默认效果

省略 effect annotation 时，默认 `/ Pure`。

规则：

- Public 函数省略 effect annotation 表示显式要求 `/ Pure`；函数体若有 unhandled non-Pure effect 是编译错误。
- Private/internal 函数可在省略 effect annotation 时从函数体推断 non-Pure row。
- Entry point 省略 effect annotation 时特殊处理为 `/ Pure!`，见本部分 “程序边界”。

### 4.2 Row containment

Effect row 形成偏序 `⊆`：

- `Pure ⊆ R`。
- `R ⊆ R + S`。
- `R1 + R2` 是 `R1` 和 `R2` 的最小上界。

调用规则：

- 如果 callee required effects 是 `Req`，当前上下文允许 `Ctx`，则调用合法当且仅当 `Req ⊆ Ctx`。

### 4.3 Open 与 closed row

默认 effect row 是 open row：

```kotlin
fun <T, R, eff E = Pure> map(xs: Array<T>, f: (T) -> R / E): Array<R> / E
```

Open row `/ R` 表示函数体自身 required effects 必须满足 `R_body ⊆ R`。高阶参数传播的效果通过函数类型和 effect 参数表达，不要求声明枚举所有回调内部效果。

Closed row 使用 `!`：

```kotlin
fun main(): Unit / Pure!
fun readFile(path: String): Bytes / IO!
```

规则：

- `/ R!` 是上界保证：函数边界外可观察到的效果不得超出 `R`。
- `!` 应用于整个 row 表达式。
- `/ IO!` 等价于单 item closed row。
- `/ (IO + State)!` 或 `/ IO+State!` 表示多 item closed row。
- `Pure!` 表示无效果逃逸，是程序入口和安全擦除场景常见要求。

## 5. Effect polymorphism

泛型可量化 effect row：

```kotlin
interface Disposable<eff E = Pure> {
    fun dispose(): Unit / E
}

fun <T, eff E = Pure> using(
    d: Disposable<eff E>,
    body: () -> T / E
): T / E {
    try {
        return body()
    } finally {
        d.dispose()
    }
}
```

规则：

- `<eff E>` 引入 row 变量。
- Row 变量可出现在函数 effect annotation 和函数类型 effect annotation 中。
- Row 实参由调用点显式给出或由 lambda body required effects 推断。

Override 规则：

- 覆写方法 required row 不能比基方法更多。
- 若基方法 row 为 `R_base`，覆写 row 为 `R_over`，必须满足 `R_over ⊆ R_base`。
- 允许实现比接口更纯。

## 6. Handler

Handler 语法：

```kotlin
handle {
    body
} on {
    Effect.op(args...) -> handlerBody
    Other.op(x), k -> resumeBody
}
```

每个 arm 匹配一个 effect operation。

### 6.1 Non-resuming handler

`->` arm 不绑定 continuation：

```kotlin
handle {
    val data = fetchData()
    println(data)
} on {
    Raise.raise(error) -> println(f"caught: ${error}")
}
```

规则：

- 被处理 computation 在 operation 处放弃，不再继续。
- Handler arm body 的值成为 handle 表达式相应路径的结果。
- `resume(...)` 不存在；没有 continuation binder。
- 常用于错误处理。

### 6.2 Resuming handler

`, k ->` arm 捕获剩余 computation：

```kotlin
handle {
    val value = Yield.next()
    value + 1
} on {
    Yield.next(), k -> {
        val answer: Int = k.resume(41)
        answer
    }
}
```

规则：

- `k` 是 `Continuation<Resume, Answer, eff E>`。
- `Resume` 是 operation 返回类型。
- `Answer` 是最近 `handle` delimiter 的结果类型。
- `E` 是恢复 continuation 后可能 required 的 effect row。
- `k.resume(payload...)` 恢复 computation。
- 若恢复后的 computation 正常完成 delimiter，`k.resume(...)` 返回 delimiter answer。
- 若恢复过程中再次通过另一个 resuming handler suspend，会捕获 fresh continuation；`k.resume(...)` 本身仍只返回最终 answer 或 raise。
- Continuation 可立即恢复、或保存后在别处恢复。
- Continuation 是高级控制流 API；普通库抽象应优先暴露常规函数/对象接口，而不是把 raw continuation 当默认 API。

旧式 `Effect.op(args) -> resume { ... }` 语法已移除。

### 6.3 Continuation one-shot

`k.resume(...)` 是 one-shot：

- 第一次调用消费 continuation。
- 第二次调用执行 `Raise.raise(RuntimeError.ContinuationAlreadyResumed)`。
- 因此 `resume` 需要 `Raise<RuntimeError>`，除非被处理。
- 不支持 multi-shot continuation。

### 6.4 Resume payload 形状

`k.resume` 参数遵循 `Resume` 类型：

- 若 `Resume` 是 `Unit`，可写 `k.resume()`。
- 若 `Resume` 是 tuple `(A0, A1, ...)`，可写 `k.resume(v0, v1, ...)`。
- Tuple payload 的命名参数可用 `a0`、`a1` 等。
- 兼容单 payload 形式 `k.resume(value)`；对 tuple 表示传入 tuple 值本身。

## 7. Handler 的 finally

`handle` 可带 `finally`：

```kotlin
handle {
    body
} on {
    E.op(x) -> h
} finally {
    cleanup()
}
```

规则：

- `finally` 在 handled computation 正常完成、被 non-resuming arm 截断、或因效果/运行期错误离开时执行。
- `finally` 的清理语义必须与 `try/finally` 一致。
- 如果 `finally` 自身执行效果或 raise，其 required effects 按普通规则进入外层上下文。
- Continuation resume 和再次 suspend 必须保持 cleanup 正确执行；具体 state machine 是实现细节。

## 8. `try` / `catch` / `finally`

`try` 是 `Raise` effect 的语法糖：

```kotlin
try {
    val data = readFile("config.json")
    parse(data)
} catch (e: IOError) {
    defaultConfig
} finally {
    cleanup()
}
```

概念脱糖：

```kotlin
handle {
    val data = readFile("config.json")
    parse(data)
} on {
    Raise.raise(e: IOError) -> { defaultConfig }
} finally {
    cleanup()
}
```

规则：

- 至少需要一个 `catch`。
- 可有多个 `catch`，按书写顺序匹配。
- `catch (e: T)` 捕获 `Raise.raise` payload 可赋给 `T` 的错误。
- `catch` arm 是 non-resuming，原 computation 放弃。
- `finally` 可选。
- `try` 表达式类型由 try body 和 catch body 的 LUB 决定，再考虑 `finally` 的控制流。

## 9. RuntimeError

部分语言结构可在运行期失败，统一用 `Raise<RuntimeError>` 表达：

```kotlin
enum RuntimeError {
    NullAssertionFailed,
    ClassCastFailed,
    ContinuationAlreadyResumed,
}
```

规则：

- `x!!` 在 `None` 时执行 `Raise.raise(RuntimeError.NullAssertionFailed)`。
- `x as T` cast 失败时执行 `Raise.raise(RuntimeError.ClassCastFailed)`。
- 重复 resume continuation 执行 `Raise.raise(RuntimeError.ContinuationAlreadyResumed)`。
- 这些结构需要 `Raise<RuntimeError>`，除非被 `try/catch` 或 `handle` 处理。

`panic(message: String): Nothing` 可作为不可恢复 trap；它不替代上述可表达为 effect 的运行期错误。

## 10. Async / structured concurrency surface（当前未定义）

当前版本不定义内建 task runtime surface，也不定义 `async` / `await` 语法、用户可见 `spawn` / `join` 语法或公共 executor API。

本部分当前只固定：

- 一般 effect 系统；
- continuation 的可恢复语义；
- `Raise<RuntimeError>`、程序边界与 `panic(...)` 等基础运行期边界。

若未来重新引入相应库或语法，应在独立设计文档中给出新的 surface 与 lowering contract；不得把历史 task/executor 叙事视为现行规范。

这些能力在当前仓库中只保留为历史设计背景，见根目录 `ASYNC_REFACTOR.md` 与 `SCOOP_FULL_SPEC.md` 的相关说明。

## 11. Generator / Yield 风格

语言不提供专用 generator 语法；可用 resuming handler 建模：

```kotlin
effect Emit<T> {
    fun emit(value: T): Unit
}

fun fibonacci(): Unit / Emit<Int> {
    var a = 0
    var b = 1
    while (true) {
        Emit.emit(a)
        val next = a + b
        a = b
        b = next
    }
}

handle {
    fibonacci()
} on {
    Emit.emit(value), k -> {
        println(value)
        k.resume()
    }
}
```

序列类型和集合接口属于标准库范围。

## 12. Required effect 推断

每个表达式有 required effect row。

规则：

- `E.op(...)` 需要 effect item `E`，除非被内层 handler 处理。
- 调用函数需要该函数声明的 effect row，替换类型/effect 实参后计算。
- 调用函数值需要该函数类型上的 effect row。
- 调用函数值时是否 may-suspend 由 callee 表达式的静态函数类型决定，即使运行期值恰好是 pure closure。
- `!!`、`as`、`Continuation.resume` 等语言结构会贡献 `Raise<RuntimeError>`。
- Handler arm body 中执行的效果贡献到外层上下文，因为 arm body 不在该 handler 自己的 dispatch scope 内。
- `finally` 中执行的效果贡献到外层上下文。

### 12.1 Lambda effect 推断

Lambda 的 effect row：

- 有期望函数类型时，检查 body required effects `R_body ⊆ R_expected`。
- 无期望 row 时，推断为 body 中所有未处理效果的最小 row。

### 12.2 Effect row 实参推断

当函数参数类型含 row 变量：

```kotlin
fun <T, eff E> run(block: () -> T / E): T / E
```

调用时 `E` 可由 lambda body required effects 推断。推断也会使用 nominal 类型中的 `eff` 实参约束。

### 12.3 函数 effect 检查

对显式 row `R_decl`：

- 函数体 required effects `R_body` 必须满足 `R_body ⊆ R_decl`。

对 public 函数省略 row：

- `R_decl = Pure`，任何 unhandled non-Pure effect 是编译错误。

对 private/internal 函数省略 row：

- 编译器可推断 `R_decl = R_body`。

对 entry point：

- 必须是 `Pure!`，见下一节。

示例：

```kotlin
fun noEffect(): Unit {
    someObj.run {
        Raise.raise("Error")
    }
}
```

如果 `run` 是 effect-polymorphic，则 `noEffect` 需要 `Raise<String>`；由于省略 row 的 public 函数默认 `Pure`，这是编译错误。可改为：

```kotlin
fun noEffect(): Unit {
    try {
        someObj.run { Raise.raise("Error") }
    } catch (e: String) {
        println(e)
    }
}
```

或：

```kotlin
fun noEffect(): Unit / Raise<String> {
    someObj.run { Raise.raise("Error") }
}
```

## 13. 程序边界与 Entry Point

Entry point 是由运行时直接调用、没有显式用户调用点的函数。独立可执行程序通常使用 `main`。

规则：

- Entry point 在没有 ambient effect handlers 的环境中被运行时调用。
- Entry point 必须是 `/ Pure!`。
- 如果 entry point 显式声明其它 effect row，包括 open `/ Pure`，是编译错误。
- 如果 entry point 省略 effect annotation，按 `/ Pure!` 处理。
- Entry point body 内可使用 effect，但必须在函数内部全部处理，不能逃逸到程序边界。

当前可执行 `main` 形态：

```kotlin
fun main(): Unit / Pure!
fun main(): Int / Pure!
fun main(args: Array<String>): Unit / Pure!
fun main(args: Array<String>): Int / Pure!
```

省略 effect annotation 等价于 `/ Pure!`：

```kotlin
fun main(): Int {
    return 0
}
```

返回规则：

- `Unit` 返回的 `main` 正常返回时进程退出码为 `0`。
- `Int` 返回的 `main` 正常返回时该值作为进程退出码。
- `panic(...)` 或其它非正常终止路径不属于 `main` 正常返回契约。

`args` 规则：

- `args` 携带原生可执行程序 argv。
- 包含 `argv[0]`，即可执行文件名/路径。
- 这不是 Kotlin/Java 只暴露用户参数的约定。
