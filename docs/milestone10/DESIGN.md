# M10 设计：协程

版本：1.0（已实现，2026-08-31）

对应 `docs/ROADMAP.md` 的 M10。目标：实现命名 `suspend` 函数/方法、完全类型化的状态机变换、`Continuation` 与最小启动/挂起原语（spec 8.2、11.9；impl spec 2.3），并保证异常、`finally` 与 M9 GC 在真实挂起后仍保持源码语义。

> M20 更新：coroutine core protocol改为统一invariant generic application，原interface variance bridge契约撤销；本设计中的协议声明与hidden ABI描述已按M20后的语言边界修订。

> M25 更新：handler 在挂起前物化异常并结束 native catch 的语义保持不变；native record、begin/end/rethrow 与生命周期实现改由 Scoop 自有异常 ABI 提供，详见 `docs/milestone25/DESIGN.md`。

## 0. 范围与关键决策

M10 交付以下闭环：普通 `main` 通过 `startCoroutine` 启动 task；task 调用 `suspendCoroutine` 保存 continuation 并真正返回；普通代码稍后调用 `resume` / `resumeWithException`；协程从原调用点继续并最终通知 completion。只编译从不挂起的 `suspend` 函数不算完成 M10。

- 支持顶层函数以及 class / interface / struct / enum 成员的 `suspend` 声明；支持 direct、virtual、interface 三种调用及泛型单态化实例；
- 挂起调用可以出现在实参、短路表达式、`if` / `when` / `while`、`try` / `catch` / `finally` 中；求值顺序与异常语义不能因状态机变换改变；
- source type `R` 与内部完成协议分离：内部 ABI 使用 `CoroutineStep<R>`，不以 `Any` 搬运结果，不把值类型统一装箱；
- frame 和每个恢复点的 continuation adapter 都是普通 managed 对象，由 TypeDescriptor 的递归扫描描述交给 M9 GC；runtime 不维护协程根表；
- M10 不引入调度器、事件循环、线程切换或取消；continuation 只保证在同一已注册线程上恢复；
- 通用函数类型、函数引用与 lambda/closure 仍不在本里程碑。core 以 `SuspendTask` / `SuspendRegistration` 两个普通 interface 提供无需这些前置特性的端到端入口；M11 在同一协议上增加函数值形态的普通源码适配重载。

最后一项是有意的边界，不是把 lambda 偷藏成 intrinsic：M10 所有用户可实现的协议仍由普通 class/interface 和普通分派完成，只有“建立状态机边界”的两个 core 函数是 intrinsic。

## 1. 语言子集与 core API

### 1.1 端到端示例

```
class Gate(var waiter: Continuation<Int>?) {
    fun complete(value: Int) {
        val continuation = waiter!!
        waiter = None
        continuation.resume(value)
    }
}

class GateRegistration(val gate: Gate) : SuspendRegistration<Int> {
    final override fun register(continuation: Continuation<Int>) {
        gate.waiter = Some(continuation)
    }
}

class Work(val gate: Gate) : SuspendTask<Int> {
    final override suspend fun run(): Int {
        val value = suspendCoroutine(GateRegistration(gate))
        return value + 1
    }
}

class Done() : Continuation<Int> {
    final override fun resume(value: Int) {
        println(value)
    }

    final override fun resumeWithException(exception: Throwable) {
        println("failed")
    }
}

fun main() {
    val gate = Gate(None)
    startCoroutine(Work(gate), Done())
    gate.complete(41)                 // 输出 42；run 已经离开首次调用栈
}
```

core 新增 `sysroot/lib/scoop.core/src/coroutine.scoop`：

```
interface Continuation<T> {
    fun resume(value: T)
    fun resumeWithException(exception: Throwable)
}

interface SuspendTask<T> {
    suspend fun run(): T
}

interface SuspendRegistration<T> {
    fun register(continuation: Continuation<T>)
}

@Intrinsic("coroutine_start")
fun <T> startCoroutine(task: SuspendTask<T>, completion: Continuation<T>)

@Intrinsic("coroutine_suspend")
suspend fun <T> suspendCoroutine(registration: SuspendRegistration<T>): T
```

`throwable.scoop` 同时新增 `IllegalStateException`，用于 continuation 重复完成、完成状态与 frame 状态不匹配等可由用户触发的协议错误。

### 1.2 声明与调用规则

- parser 接受 `suspend fun`；对成员函数，`suspend` 可与 `final` / `open` / `abstract` / `override` 按现有 modifier 规则组合，重复修饰符诊断；
- `suspend` 是 callable 签名属性：override 与 interface 实现必须一致；普通/挂起声明不能只靠 `suspend` 相互重载；
- 普通函数体调用挂起函数是编译错误。唯一普通入口是登记为 coroutine builder 的 `startCoroutine`；它本身不把调用者变成挂起上下文；
- 顶层属性、object/companion、实例属性、delegate、`init`、构造函数/构造委托与普通属性访问器等声明拥有的初始化/访问体固定为非挂起上下文，不得调用 suspend callable。对应入口没有 hidden continuation 参数，也不能返回 `CoroutineStep`；该规则现在写入 M10 契约，即使相关属性/object 语法在后续里程碑才完整落地也不得改变；
- `main` 必须保持普通、无参数、`Unit` 返回。需要挂起的入口逻辑实现为 `SuspendTask<Unit>`；
- abstract / interface suspend 方法没有函数体，规则与现有普通 abstract 方法一致；virtual / interface 槽按源码签名占一个槽，槽中目标使用变换后的 hidden ABI；
- generic suspend callable 先按现有规则确定完整类型实参并单态化，再做状态机变换；generic 定义本身不生成 frame；
- `suspendCoroutine` 只能在挂起上下文调用；`registration.register` 是普通调用，不允许自身挂起。

### 1.3 完成协议

- 每个编译器生成的 continuation 恰好接受一次 `resume(value)` 或 `resumeWithException(exception)`；第二次调用抛 `IllegalStateException`；
- `startCoroutine` 总是返回 `Unit`。task 立即完成时，completion 在 `startCoroutine` 返回前被调用；task 返回 `Suspended` 时，completion 在最终恢复链完成后调用；
- task 自身未处理的异常交给 `completion.resumeWithException`。completion 方法自身抛出的异常不再递归转换成对同一 completion 的失败通知，而是传播给调用 `startCoroutine` / `resume` 的代码；
- `suspendCoroutine` 的安全 continuation 在 `register` 返回前处于 registering 状态。同步 `resume` 只锁存结果，待 `register` 正常返回后作为 `Completed` 返回，不重入 frame；正常返回但没有结果则转为 suspended；
- `register` 在未完成 continuation 时抛出，异常成为挂起调用的结果；若先同步完成又继续抛出，属于 registration 协议错误，以 `IllegalStateException` 结束该调用。无论哪条路径，保留下来的 safe continuation 之后都不能再次成功完成。

### 1.4 初始化上下文矩阵

挂起能力跟随**正在求值的词法体**，不能简单地从“表达式外面是否有构造语法”推断：

| 上下文 | 立即求值中能否调用 suspend | 说明 |
|---|---:|---|
| suspend 函数体、其局部 `val` / `var` initializer | 是 | 属于当前状态机，可产生 resume point |
| suspend caller 写出的显式实参，如 `C(awaitValue())` | 是 | 实参先在 caller 中求值，完成后才进入同步构造 |
| suspend 函数的缺省参数表达式 | 是 | 按 spec 8.5 在挂起调用处实例化和求值 |
| 顶层 `val` / `var` initializer 或 delegate 表达式 | 否 | 全局初始化入口必须在 `main` 前同步完成 |
| `object` / `companion object` 的属性 initializer、delegate、`init`、继承/委托初始化 | 否 | 不能发布部分初始化的单例 |
| class 的属性 initializer、delegate、`init`、构造函数体/构造委托 | 否 | 构造函数不是 suspend callable |
| struct / enum 变体或构造函数的缺省表达式 | 否 | 定义属于同步构造协议；显式实参仍按 caller 行处理 |
| getter / setter、计算属性、delegate `getValue` / `setValue` | 否 | 普通属性访问不携带 suspend 类型标记 |
| `const val` initializer | 否，而且不能有任何 call | 必须在 HIR 编译期求值，不存在 runtime initializer |
| initializer 中只构造并保存 suspend lambda / `SuspendTask` | 可以 | 构造对象不是执行其 suspend body；真正调用时重新检查上下文 |

`startCoroutine` 是普通且同步返回的 builder，调用它本身不使 initializer 挂起；启动后的 task 不是“尚未完成的对象初始化”。M10 不另外禁止这种副作用，但它仍受全局初始化顺序和可见性规则约束。

## 2. 内部 ABI 与状态机

### 2.1 `CoroutineStep<R>`

每个具体返回类型 `R` 对应一个编译器内部值 enum：

```
enum CoroutineStep<R> {
    Completed(R),
    Suspended
}
```

它不进入源码命名空间。每个单态化 suspend callable 的目标 ABI 为：

```
f(source_args..., completion: Continuation<R>) -> CoroutineStep<R>
```

- `Completed(value)` 表示 callee 没有把 completion 留到返回之后，caller 直接继续；
- `Suspended` 表示 callee 已保存 completion；此后只能经该 completion 恢复，不能再从本次 native call 返回一个结果；
- 返回 `Completed` 与调用 completion 二选一。编译器生成代码违反该不变量属于编译器错误；用户通过 `SuspendRegistration` 重复恢复则是 `IllegalStateException`；
- `CoroutineStep<R>` 走普通 enum 布局与递归扫描，`R` 为值类型时保持值表示。

符号 mangling 加入 suspend ABI discriminator，避免与普通函数或后续 FFI wrapper 混淆；实体身份仍使用类型化 id，不能以符号文本反查。

### 2.2 frame 与恢复点 adapter

每个具体 suspend function 生成一个 frame 引用类型。frame 至少包含：

- lifecycle/state：`running`、某个 `suspended(k)`、`completed`；
- 外层 `Continuation<R>`；
- 参数及所有跨任一挂起点活跃的局部值；
- 跨挂起的控制状态，例如 `finally` 的 pending normal / return / exception；
- 只在对应状态有效的数据用编译器生成的 tagged enum 表示，不能留下未初始化的引用槽。

每个挂起点 `k` 另生成一个精确实现 `Continuation<Tk>` 的 adapter，其中 `Tk` 是该调用点的源码结果类型。adapter 保存 frame 引用、恢复点 id 与 consumed 状态：

1. 调用前把活跃值写入 frame，设置 `state = suspended(k)`，创建 adapter；
2. 调用 callee 的 hidden ABI；
3. `Completed(v)`：使 adapter 失效，把 `v` 写入调用结果位置并继续当前 CFG；
4. `Suspended`：当前状态机返回 `CoroutineStep.Suspended`；
5. 稍后 `adapter.resume(v)`：校验并消费 adapter，把 frame 切回 running，从 `k` 的 success 入口继续；
6. `adapter.resumeWithException(e)`：从 `k` 的 failure 入口继续，该入口等价于在原调用表达式处执行 `throw e`。

resume entry 会驱动 frame，直到再次 `Suspended` 或得到整个函数的 `Completed(R)` / 未处理异常。因为原 native caller 已经离开，后两者分别在 entry 的 body-execution catch 范围之外调用 frame 保存的外层 `completion.resume` / `completion.resumeWithException`；外层 completion 自身抛出的异常直接传播给本次 `resume` 的调用者。

不同挂起点即使结果类型相同也有不同的 `CoroutineResumePointId`；不同 frame / adapter / internal enum 使用不同类型化 id。可复用代码和 TypeDescriptor，但不可按名字合并实体。

### 2.3 CFG 切分与活跃值

M10 把 MIR 正式收敛为 CFG，而不是在现有结构化 statement 上拼接 label：

1. 按源码从左到右顺序把 call 从嵌套表达式中 A-normalize 为显式 CFG 操作；
2. 把 `if` / `when` / `while` 与短路运算展开为 normal edges；
3. 把 `try` / `catch` / `finally` 展开为显式 unwind / cleanup edges；
4. 对 suspend callable 的 CFG，在每个挂起调用之后切分 resume block；
5. 做 backwards liveness，只把跨挂起点活跃的值放入 frame；
6. 生成初始 entry、各 success/failure resume entry、frame/adapter 类型及普通非 suspend helper；
7. 最终公开 MIR 中不再存在源码级 suspend call，只有变换后的普通 call、enum 分支、frame 访问和普通 return/throw。

所有函数统一输出 CFG，避免 `StructuredBody | CoroutineBody` 两套长期并存的不变量。非 suspend 函数经过 CFG 化后行为应与 M1–M9 golden 等价；这是 M10 的第一项回归门槛。

## 3. 异常、`finally` 与恢复

### 3.1 调用点异常

挂起函数在首次 native call 内抛出时，沿调用点既有 unwind edge 传播；若先返回 `Suspended`，之后的 `resumeWithException` 从该调用点的 failure resume block 注入相同的 managed `Throwable`。因此下列两条路径必须进入同一个 catch：

```
try {
    val value = maySuspend()
    use(value)
} catch (e: Exception) {
    recover()
}
```

状态机不得把恢复失败直接跳到外层 completion，从而绕过源码 catch。

### 3.2 原生 EH 状态不能跨挂起

M8 的 catch 当前依赖 `__cxa_begin_catch` / `__cxa_end_catch` 配对。原生 catch 状态与 native 栈/线程关联，不能保存在协程 frame 中。对 suspend function 的 handler 采用以下统一规则：

- landingpad 选中 handler 后，先把 ABI 异常缓冲按 TypeDescriptor 复制成 managed 对象；
- 复制期间 ABI 缓冲仍是 M9 的外部根；复制完成后立即 `__cxa_end_catch`；
- catch local 与 pending exception 都指向 managed 副本，之后 catch/finally 可以任意挂起；
- 需要继续传播时，从 managed 对象重新 `scoop_rt_throw`。Scoop 当前本来就是 throw-by-value ABI，不承诺 C++ ABI 异常记录 identity，因此该物化不新增语言可观察的 identity 保证。

runtime 新增 `void *scoop_rt_materialize_exception(const void *caught)`：按 caught 对象的 TypeDescriptor 分配 managed 对象，保留新对象初始化后的 `td` / `gc_word`，只复制对象头之后的完整 payload。它只是“ABI 缓冲 → managed 对象”辅助；状态分派仍全部在生成代码中。

### 3.3 `finally`

挂起不是退出，不能执行 `finally`。进入 finally 前，把待执行动作编码为结构完备的内部 enum，例如：

```
PendingExit<R> = Normal(nextState) | Return(R) | Throw(Throwable)
```

finally 自身挂起时，该值随 frame 保留；finally 正常结束后执行 pending action；finally 内新的 `return` / `throw` 覆盖原 action。由此覆盖：

- try 正常路径、catch 正常路径；
- try/catch 中的 return；
- 首次调用或恢复失败产生的异常；
- finally 自身跨一个或多个挂起点。

M10 不含 `break` / `continue`，后续落地时只需给 `PendingExit` 增加对应的目标状态变体。

## 4. 各 stage 设计

### 4.1 AST / parser

- lexer 把 `suspend` 作为保留关键字；parser modifier 集合增加 `is_suspend`，接受 `suspend fun` 及成员 modifier 组合；
- `ast::FunctionDecl` 增加非可选 `is_suspend: bool`；不使用注解或名字推断；
- 解析错误恢复把 `suspend fun` 视作声明起点；孤立 `suspend`、重复 `suspend`、`suspend class` 等给定位准确的诊断；
- M10 不新增 function type / lambda AST。遇 `suspend (` 类型形态时给正式的“function types are not supported in M10”诊断，而不是落入内部 parser 错误。

### 4.2 HIR

- `hir::Function`、`GenericFunction`、`MethodSig`、resolved call target 均携带非可选 `is_suspend`；`.slib` HIR meta 后续也沿用该字段；
- HIR 使用显式 `SuspensionContext::{Forbidden(reason), SuspendFunction, Builder}`，而不是“有当前函数时才检查”的可缺失状态。普通函数是 `Forbidden(Function)`；进入顶层/object/companion/实例属性 initializer、delegate、`init`、构造体、构造缺省表达式或属性访问器时压入对应的 `Forbidden(reason)`，离开该词法体后恢复外层上下文；
- 显式调用实参在进入 callee 的构造/函数体之前按 caller context 降级，因此 suspend caller 的 `C(awaitValue())` 会先生成 resume point，再执行普通构造；局部变量 initializer 不重置上下文；
- `Forbidden(reason)` 中的挂起 call 产生带具体原因的诊断；挂起函数中的普通 call 不受影响。`Builder` 只供登记过的 core/stdlib builder 自身展开使用，不能由用户语法隐式取得；
- 普通 getter/setter、计算属性与 delegate `getValue` / `setValue` 的 resolved target 必须是非 suspend；`const val` 走独立的 HIR const evaluator 和依赖环检查，不进入普通 expression/call lower，也不产生 MIR initializer；
- override/interface matching 除现有参数/返回类型外精确比较挂起性；itable/vtable 槽签名记录挂起性；
- overload 候选查重不把挂起性当区分项。决议完成后再检查调用上下文，避免用上下文偷偷改变候选层和 MSC；
- 表达式继续具有源码类型 `R`。HIR 不出现 `CoroutineStep`、hidden continuation 参数、frame 或 adapter；
- `coroutine_start` / `coroutine_suspend` 按 intrinsic registry 校验 core-only、泛型元数和精确签名。前者标记为 builder，后者仍是普通的 suspend call target；
- 控制流完整性分析按源码语义运行，不把“可能挂起”当作不落空路径。

### 4.3 MIR

MIR 是 M10 的主实现层：

- `Body` 改为基本块 arena + entry；call/branch/return/throw/unwind 是显式 terminator 或 effect statement，嵌套 call 在进入 CFG 前完成 A-normalize；
- 先复用现有单态化，得到完全具体的参数、返回值、局部与调用目标，再运行 coroutine transform；
- 所有 suspend 成员方法在进入 coroutine transform 前，已按 spec 3.3 把 receiver 正规化为隐含的 method-local `this` value；若 `this` 跨挂起点活跃，frame 保存的是该参数值——value type 保存完整值，ref type 保存 managed ref value——而不是调用方 receiver binding place 的地址；
- 引入 `CoroutineFrameId`、`CoroutineResumePointId`、`CoroutineAdapterId`、`CoroutineStepId` 等互不混用的 id；生成实体进入普通 type/function list，但保留 synthetic origin 供 dump 与诊断；
- frame 字段由 liveness + pending cleanup 构成；所有字段类型完备。frame/adapter 构造是普通 class allocation，字段 store 走既有写屏障；
- direct / virtual / interface suspend call 统一追加 continuation 参数并返回具体 `CoroutineStep<R>`；分派表槽指向 transformed symbol；
- `SuspendTask<T>`、`Continuation<T>`与hidden `CoroutineStep<T>`都使用同一个exact `T`；`startCoroutine`的generic constraint必须把task、completion与结果绑定为同一application。`SuspendTask<Dog>`不能作为`SuspendTask<Animal>`调用，也不得把不同`CoroutineStep<T>`擦除为同一ABI；
- `coroutine_start` 在 MIR 展开：调用 `SuspendTask<T>.run` hidden ABI，`Completed(v)` 时在 try 范围之外调用 completion.resume，`Suspended` 时返回；task body 的未处理异常转 completion failure；
- `coroutine_suspend` 生成/调用每个 `T` 的 safe-continuation helper，处理 registering / suspended / completed 状态，不进入 runtime；
- MIR dump 明确列出 frame 字段、状态编号、每个 resume point 的结果类型、CFG normal/unwind edges，作为状态机结构的权威 golden。

### 4.4 LIR

- 接收MIR CFG，计算synthetic enum/class的普通布局与RefScan；frame中值类型字段递归展开扫描；M13修订后的tagged enum使用逐variant GC-free flag分配独占ref-bearing slot，并合并为不读取tag的固定ref偏移；
- frame load/store 降为现有 `HeapLoad` / `HeapStore` 字节偏移；adapter 与 safe continuation 分派使用普通 itable；
- MIR 的 unwind/cleanup edge 映射为现有 `Invoke` / `LandingPad` / `CleanupPad` / `BeginCatch` / `EndCatch` / `Resume` 结构；
- 对 suspend handler 插入异常物化调用并在进入可挂起 CFG 前结束 catch；任何返回 `CoroutineStep.Suspended` 的块都不得带未配对的 BeginCatch；
- LIR 本身不新增 coroutine 指令，dump 中只剩普通 CFG、enum、堆访问和调用。

### 4.5 codegen / driver

- codegen 不理解 `suspend`；机械发射 synthetic 类型的 TD、itable、布局、扫描树及普通函数；
- 所有生成 helper 继续设置 M9 的 GC strategy 并执行 statepoint rewrite，frame/adapter 分配和恢复调用都是 safepoint；
- driver / linker 无新增 runtime 库或链接参数；M10 不引入线程库、event loop 或 C++ coroutine ABI。

## 5. GC 与对象生命周期

- 挂起链的保活路径是 producer 持有 continuation adapter → adapter 持有 frame → frame 持有外层 completion / adapter；每条引用都在普通堆对象字段中；
- producer 不保存 continuation 且 registration 返回时，协程可变为不可达并由 GC 回收；这等价于永不恢复，不触发 cancellation/finally；
- frame 只保存 liveness 证明需要的源值；调试友好地保存全部局部可作为后续模式，但不是 M10 默认；
- frame 中的 class/interface/String/Array 引用、含引用 struct/tuple、tagged enum 与 `PendingExit` 必须复用 M9 的递归 RefScan，不能只收集顶层指针字段；
- 在挂起期间强制 `gcCollect()` 后恢复，是 M10 的必测路径；仅在不触发 GC 的 fixture 中正确不算验收通过；
- adapter 完成后标记 consumed；对 frame/adapter 的额外用户引用不会导致第二次恢复成功。

## 6. 测试计划

### 6.1 独立 fixture

- 立即完成：无挂起点的 suspend task、suspend 调用链全部返回 `Completed`；
- 真挂起：保存 continuation，`startCoroutine` 返回后再 resume；成功与失败各一条；
- 同步恢复：`register` 内 resume，验证不重入且后续语句只执行一次；
- 多挂起点：同类型与不同类型结果，验证 resume point 不按类型或符号错误合并；
- 成员分派：final / virtual / interface suspend 方法；value/ref receiver 都按值把 method-local `this` 保留进跨挂起 frame；
- 泛型：泛型 suspend 顶层函数与泛型成员函数在多个具体类型上的 frame/step 实例；
- 控制流：挂起位于实参中间、if/when 分支、while 条件/体、短路表达式，验证前序副作用不重复；
- 异常：首次调用立即抛、挂起后失败、catch 内再挂起、finally 内挂起、finally 覆盖 pending return/throw；
- GC：frame 跨强制回收保留 String、class、含引用 struct、tagged enum 与外层 continuation 链。

### 6.2 negative / runtime-error fixture

- 普通函数，以及顶层属性、object/companion 属性、delegate、`init`、构造函数/构造委托、普通 getter/setter 等非挂起体调用 suspend；`suspend main`；override/interface 实现挂起性不一致；只差 suspend 的重复重载；
- `const val` 含普通/suspend call、构造、分配、普通属性读取或 const 依赖环；这些规则对应的语法尚未落地时先由设计约束锁定，语法落地的同一批变更必须补 fixture；
- 对非 core 声明使用两个 coroutine intrinsic name、intrinsic 签名错误；
- 孤立/重复/错误位置的 `suspend`，以及 M10 未支持的 suspend function type/lambda；
- continuation 双 resume、先 failure 后 success、同步 resume 后 registration 抛出；断言 `IllegalStateException` 类型与位置相关输出。

### 6.3 golden / 回归

- AST/HIR dump 锁定 suspend 标志与源码结果类型；
- positive fixture 锁定 `suspend fun f() = C(awaitValue())` 合法、局部 initializer 可挂起，以及“只保存 SuspendTask、不执行”的同步初始化合法；
- MIR golden 锁定 CFG、frame 字段、resume state、hidden ABI、normal/unwind/cleanup edges；
- LIR/LLVM golden 锁定 synthetic TD/RefScan、itable、landingpad 在返回 Suspended 前已结束 catch；
- M1–M9 全部 fixture 原样通过。CFG 化是全 MIR 结构调整，允许集中更新 golden，但每个语义输出和既有 negative 诊断不得无故变化。

## 7. 实现顺序与验收门

1. **CFG 基线**：在没有任何 suspend 源码时完成 MIR CFG 化及 M8 EH 路径迁移，M1–M9 全回归；
2. **声明与立即完成 ABI**：parser/HIR suspend 规则、core 协议、`CoroutineStep.Completed`、direct/virtual/interface/generic 调用；
3. **真实挂起与恢复**：frame、liveness、adapter、`Suspended`、start/suspend intrinsics；
4. **异常与 cleanup**：failure resume、异常物化、catch/finally 跨挂起；
5. **GC 压力与完整测试**：强制回收、嵌套 frame 链、全部 golden/negative/runtime error。

每一步先完成 IR/unit 测试，再加端到端 fixture；一个步骤未过验收门时不开始下一个步骤的行为扩展。

## 8. 临时决策与明确不做

1. **单线程 continuation 状态**：M10 用普通字段检查，无原子状态机。M13 随runtime线程注册/STW握手补齐跨线程完成/恢复的原子状态；标准库dispatcher与调度策略仍在后续。
2. **无取消**：没有 cancellation exception、structured concurrency 或 abandoned-frame cleanup；永不恢复即普通不可达对象。
3. **core adapter 代替 lambda builder**：`SuspendTask` / `SuspendRegistration` 是稳定的最小协议；普通/挂起函数类型、函数引用与捕获 lambda 由 M11 承接，`launch` / `async` 不在 M10。
4. **无 FFI suspend ABI**：已在进入 M12 前明确，现阶段不支持 suspend FFI。`@Extern` 与 `suspend` 互斥，挂起函数的声明引用不能在 `FunPtr` 上下文中解析为原生地址；HIR 必须直接诊断，不生成 wrapper，也不把 M10 hidden continuation ABI 暴露为外部符号 ABI。
5. **状态机不走 LLVM coroutine intrinsics**：变换固定在 MIR，便于跨后端复用并让 frame 布局/GC 扫描成为 Scoop 自己的显式契约。
6. **无 frame 优化**：若静态证明整条路径不可能挂起，后续可做 stack fast path / frame elision；M10 先保证语义与 GC 正确。

## 9. 文档同步项

- **spec 8.2**：固定挂起调用、单次完成、异常/finally 与 main 入口语义；
- **spec 11.7 / 11.9**：新增 `IllegalStateException`，固定 Continuation、SuspendTask、SuspendRegistration、startCoroutine 与 suspendCoroutine；
- **impl spec 2.2–2.4**：固定 HIR suspend 标志、MIR CFG/状态机职责、完全类型化 hidden ABI 与 LIR 的 EH 映射；
- **runtime spec 5/8**：固定 ABI 异常物化与“frame 是普通 managed 对象、runtime 不调度”的边界；
- **ROADMAP M10**：链接本文并写明 adapter 形态与标准库边界。
