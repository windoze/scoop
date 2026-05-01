# Scoop 语言规范 第 4 部分：效果系统、异常语法糖与 async/await

版本：0.1 草案

本部分定义代数效果、effect row、handler、continuation、`try` / `catch` / `finally`、运行期错误、`async` / `await` 与程序边界的效果规则。

## 1. 总览

Scoop 使用代数效果系统统一表达：

- 可恢复或不可恢复的控制流效果。
- 错误处理。
- async/await。
- generator/yield 风格流程。
- 用户自定义效果。

效果是静态类型系统的一部分。每个表达式有 required effect row：在没有隐式 ambient handler 的前提下，求值该表达式可能需要外层允许或处理的效果集合。

## 2. Effect 声明

```kotlin
effect Async {
    fun <T> await(task: Task<T>): T
}

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

效果 operation 使用 `perform`：

```kotlin
fun fetchData(): String / Raise<IOError> {
    val resp = httpGet("/data")
    if (resp.status != 200) {
        perform Raise.raise(IOError("bad status"))
    }
    return resp.body
}
```

规则：

- `perform E.op(args...)` 的实参按源码从左到右求值。
- 若当前动态 handler 栈中有匹配 operation 的 handler，则 dispatch 给最近匹配 handler。
- 若没有 handler，效果成为 unhandled effect，必须由函数签名 required effect row 表示。
- 若 unhandled effect 到达程序边界，well-typed 程序应已阻止；实现可将其作为运行期 panic。

注：`async fun foo(): T` 不等价于 `fun foo(): T / Async`。它脱糖为 `fun foo(): Task<T>`；`Async` effect 只存在于任务 computation 内部。

## 4. Effect row

Effect row 是编译期集合表达式：

```kotlin
/ Pure
/ Async
/ Raise<IOError>
/ (Async + Raise<IOError>)
/ E
/ (E + Raise<IOError>)
```

语法元素：

- `Pure`：空 row。
- `Async`、`Raise<IOError>`：effect item。
- `E`：effect row 变量，由 `<eff E>` 引入。
- `R1 + R2`：row union。

代数规则：

- `+` 满足结合律、交换律、幂等律。
- `Pure` 是单位元。
- Row 只存在于编译期。

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
extern fun ffi_read_file(path: Ptr<Byte>): Bytes / IO!
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
- Effect 参数不影响运行期布局和单态化。

Override 规则：

- 覆写方法 required row 不能比基方法更多。
- 若基方法 row 为 `R_base`，覆写 row 为 `R_over`，必须满足 `R_over ⊆ R_base`。
- 允许实现比接口更纯。

## 6. Handler

Handler 语法：

```kotlin
handle {
    body
} with {
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
} with {
    Raise.raise(error) -> println(f"caught: {error}")
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
    val value = perform Yield.next()
    value + 1
} with {
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
- Continuation 可立即恢复、保存后恢复、或从另一个 OS 线程恢复。
- Continuation 是高级控制流 API；普通 async 应优先暴露 `Task<T>`。

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

## 7. 动态 dispatch 规则

效果 dispatch 按动态 handler 栈进行。

定义：

- Effect operation：`Raise.raise`、`Async.await` 等限定 operation 名。
- Handler instance：一次进入 `handle` 表达式产生的动态 handler。
- Active handler：当前 computation 动态作用域内的 handler。
- Handled set：handler `with { ... }` 中 arm 覆盖的 operation 集合。

执行 `perform E.op(args...)`：

1. 从左到右求值 `args...`。
2. 找到最近的 active handler instance，其 handled set 包含 `E.op`。
3. 只派发给这个最近匹配 handler。
4. 若没有匹配 handler，效果向外传播为 unhandled effect。

### 7.1 Handler arm body 不在本 handler dispatch 作用域内

规范规则：

- handler arm body 求值期间，选中该 arm 的 handler instance 在 effect dispatch 时视为 inactive。

后果：

- 如果 arm body 再次 perform 同一个 operation，不会重新进入自己，而是派发给外层匹配 handler 或继续 unhandled。

示例：

```kotlin
handle {
    handle {
        perform Raise.raise("inner")
    } with {
        Raise.raise(e), k -> {
            perform Raise.raise("from arm") // 目标是外层 handler
        }
    }
} with {
    Raise.raise(e) -> println(e)
}
```

### 7.2 Continuation 与动态上下文

Continuation 捕获 suspend 点的动态 effect context。调用 `k.resume(...)` 时：

- 恢复后的 computation 在 captured context 下运行。
- 即使从另一 OS 线程 resume，也不使用该线程当时的 ambient handler 栈。
- 当恢复进入同一 handler 的 arm body 时，该 handler 对自己的 arm body 仍按 “inactive” 规则处理。

实现可用 handler-stack snapshot、显式 context 对象、TLS bridge 或其它私有机制；这些不改变语言语义。

## 8. Handler 的 finally

`handle` 可带 `finally`：

```kotlin
handle {
    body
} with {
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

## 9. `try` / `catch` / `finally`

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
} with {
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

## 10. RuntimeError

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

## 11. Async / Await

`Async` 是内建 effect：

```kotlin
effect Async {
    fun await<T>(task: Task<T>): T
}
```

核心任务形态：

```kotlin
enum TaskStep<T> {
    Pending,
    Ready(T),
}

class Task<T> {
    fun step(): TaskStep<T>
}
```

语言语义：

- `async { body }` 创建惰性 `Task<T>`。
- `async fun foo(): T` 脱糖为 `fun foo(): Task<T>`。
- `await expr` 只在 async computation 内有意义，脱糖为 `perform Async.await(expr)`。
- 调用 `async fun` 的 caller 获得 `Task<T>`，caller 签名不因此需要 `/ Async`。
- `/ Async` effect 存在于 Task 的 computation 内部。

示例：

```kotlin
async fun fetch(): Int {
    val inner: Task<Int> = async { 1 }
    return await inner + 1
}

val outer: Task<Int> = async {
    await fetch() + 10
}
```

### 11.1 Task stepping

`Task<T>` 是惰性、可手动驱动的核心抽象：

```kotlin
while (true) {
    when (task.step()) {
        Pending -> ()
        Ready(value) -> {
            println(value)
            break
        }
    }
}
```

规则：

- `step()` 启动或恢复 task，直到完成为 `Ready(value)` 或再次 suspend 为 `Pending`。
- `Pending` 表示尚未完成且当前不能继续推进；不是并发争用信号。
- Task 可由不同线程顺序驱动，但同一时刻最多一个 public driver。
- 并发 `step()`、重入同一 task、或观察到内部 Running 状态，是 executor/driver misuse，必须 trap；不表示为 `Pending`，也不表示为 `Raise<RuntimeError>`。
- Task 内部可保存入口 closure、内部 continuation、完成值和私有 step-result carrier；这些都是实现细节。
- Task 不引入独立于 `Continuation.resume(...)` 的第二套用户可见 resume ABI。

### 11.2 非目标

本阶段不定义：

- 公共 executor API。
- `spawn` / `join` 语法。
- 结构化并发语法。
- wakeup 注册、队列、work stealing。
- `scoop.task` 公共包。

这些是未来库或语言扩展主题。

## 12. Generator / Yield 风格

语言不提供专用 generator 语法；可用 resuming handler 建模：

```kotlin
effect Emit<T> {
    fun emit(value: T): Unit
}

fun fibonacci(): Unit / Emit<Int> {
    var a = 0
    var b = 1
    while (true) {
        perform Emit.emit(a)
        val next = a + b
        a = b
        b = next
    }
}

handle {
    fibonacci()
} with {
    Emit.emit(value), k -> {
        println(value)
        k.resume()
    }
}
```

序列类型和集合接口属于标准库范围。

## 13. Required effect 推断

每个表达式有 required effect row。

规则：

- `perform E.op(...)` 需要 effect item `E`，除非被内层 handler 处理。
- 调用函数需要该函数声明的 effect row，替换类型/effect 实参后计算。
- 调用函数值需要该函数类型上的 effect row。
- 调用函数值时是否 may-suspend 由 callee 表达式的静态函数类型决定，即使运行期值恰好是 pure closure。
- `!!`、`as`、`Continuation.resume` 等语言结构会贡献 `Raise<RuntimeError>`。
- Handler arm body 中执行的效果贡献到外层上下文，因为 arm body 不在该 handler 自己的 dispatch scope 内。
- `finally` 中执行的效果贡献到外层上下文。

### 13.1 Lambda effect 推断

Lambda 的 effect row：

- 有期望函数类型时，检查 body required effects `R_body ⊆ R_expected`。
- 无期望 row 时，推断为 body 中所有未处理效果的最小 row。

### 13.2 Effect row 实参推断

当函数参数类型含 row 变量：

```kotlin
fun <T, eff E> run(block: () -> T / E): T / E
```

调用时 `E` 可由 lambda body required effects 推断。推断也会使用 nominal 类型中的 `eff` 实参约束。

### 13.3 函数 effect 检查

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
        perform Raise.raise("Error")
    }
}
```

如果 `run` 是 effect-polymorphic，则 `noEffect` 需要 `Raise<String>`；由于省略 row 的 public 函数默认 `Pure`，这是编译错误。可改为：

```kotlin
fun noEffect(): Unit {
    try {
        someObj.run { perform Raise.raise("Error") }
    } catch (e: String) {
        println(e)
    }
}
```

或：

```kotlin
fun noEffect(): Unit / Raise<String> {
    someObj.run { perform Raise.raise("Error") }
}
```

## 14. 程序边界与 Entry Point

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
- 没有单独语言级 `process.args()` API。
