# Async / Suspend / Executor 重构设计

> 状态：设计草案  
> 适用范围：异步语义、结构化并发、runtime 分层、平台 backend/cone 切分  
> 非目标：本文不定义 effectful FFI，也不要求移除现有 substrate；它的目标是在“不新增 substrate”的前提下，重画 async/runtime 的架构边界。

## 0. 背景

当前语言已经有一套较完整的 effect / continuation / state machine 基础能力，但 `async` / `Task<T>` 这一层表面设计仍然带来几个结构性问题：

1. `async fun foo(): T` 对外暴露为 `Task<T>`，把“颜色”从 effect row 挪到了返回类型上。
2. 若直接把所有底层 effect 暴露到 public API，又容易让上层库陷入类似 Java checked exception 的 effect 污染。
3. `fork/join`、executor、reactor、I/O backend、blocking fallback 本质上是不同层次的问题，但当前容易被混成同一个“async 机制”。
4. 平台相关的 runtime 机制应可替换；core runtime 不应继续长期承载大量非 substrate helper。

本文的主张是：

- 把“可能挂起”和“并发调度”拆开。
- 把“服务语义”和“运行时机制”拆开。
- 把“必须留在 substrate 的部分”和“可迁移到 Scoop/FFI mixed lib 的部分”拆开。

## 1. 设计目标与非目标

### 1.1 目标

1. 保持语言层的 color-agnostic 方向。
   - public API 可以表达“可能挂起”，但不应被迫暴露整套底层 effect taxonomy。
2. 把 `fork/join` 从“可挂起语义”中分离出来。
   - `fork/join` 是 executor 的专属能力，而不是所有可挂起代码都天然拥有的能力。
3. 允许 blocking backend、单线程 executor、多线程 executor 共用一套 public surface。
4. 不新增 substrate。
   - 只复用已有的 continuation/effect substrate、GC substrate、线程/原子/条件变量、FFI、`GcHandle` 等基础设施。
5. 让平台 I/O/reactor/backend 可替换。
   - Linux 可以有 `poll` / `epoll` / `io_uring` cone。
   - Windows 可以有 `IOCP` cone。
   - blocking fallback 也是合法实现。
6. 让非 substrate 的 runtime/helper 逐步从 core runtime 中迁出，形成独立包/cone。

### 1.2 非目标

- 本文不定义 effectful FFI。
- 本文不要求 continuation / effect context / outward suspend 穿过 Managed ABI。
- 本文不要求立即删除现有 `async` / `await` / `Task<T>`；它们可以作为过渡 surface 或 adapter 保留。
- 本文不试图把所有平台 shim 都改成 Managed ABI；最低层 host API 仍可主要走 `ExternAbi::C`。

## 2. 核心判断

### 2.1 public effect row 只暴露“调用者必须承担的义务”

public effect row 不应描述整套实现架构图。

从表面形式看，effect 很像“环境向程序提供的 capability”；但它相比普通 interface 还多了承接控制流协议的职责，因此更应谨慎决定哪些东西值得进入 public effect row。

需要区分三类东西：

1. 控制义务
   - 例如：当前计算可能挂起，进展依赖环境。
2. 服务能力
   - 例如：`HttpClient`、`Db`、`FileSystem`、`Clock`。
3. 后端机制
   - 例如：`epoll`、`io_uring`、`IOCP`、blocking socket、completion queue。

其中：

- 控制义务适合放在 effect row。
- 服务能力更适合建模为普通 interface/object capability。
- 后端机制应尽量压到 runtime/cone 内部，不进入 public effect row。

### 2.2 `Concurrent` 不是一个好名字

如果 effect 想表达的是“可能挂起”，那 `Concurrent` 过于强调并发/调度，容易把 `fork/join` 和 `I/O suspend` 混在一起。

本文建议把该 effect 暂定为：

```scoop
effect Suspend
```

若后续更强调“只是可能挂起而非必然异步”，也可考虑命名为 `MaySuspend`。本文后续统一使用 `Suspend` 作为暂定名称。

### 2.3 `fork/join` 属于 executor scope，而不是 `Suspend` 自身

`I/O` 不是 executor 的能力；`fork/join` 才是 executor 的能力。

因此：

- `HttpClient.get(): Response / Suspend` 是合理的。
- `fork` / `join` 不应被建成所有 `/ Suspend` 代码都自动拥有的普通 effect op。
- `fork/join` 只能出现在某个 executor 提供的 scope/capability 中。

### 2.4 blocking backend 必须是合法实现

`/ Suspend` 表示“该计算在某些实现下可能挂起”，而不是“该计算必须走异步 executor”。

因此：

- 一个 blocking backend 可以直接阻塞当前线程并同步返回结果。
- 一个非 blocking backend 可以选择 suspend/resume。
- 二者应共享同一套 public API 与 effect row。

这是本文的 color-agnostic 基线。

## 3. 分层模型

本文建议把系统切成四层。

### 3.1 Core substrate

这一层继续留在 compiler/core runtime 中，不是本文要迁出的对象：

- effect / continuation substrate
- state machine lowering
- GC / allocator / write barrier
- `pin/unpin`
- `GcHandle`
- raw pointer / atomics / unsafe substrate
- host substrate：线程、互斥、条件变量等

本文不要求移除这些基础能力，只要求不要在其上继续叠加 async 专用 special-case substrate。

### 3.2 Platform shim package

这一层是最薄的平台相关封装，通常主要基于 `ExternAbi::C`：

- socket / file / timer / reactor syscall wrapper
- `poll` / `epoll` / `io_uring` / `IOCP` / `select`
- wakeup fd / eventfd / completion port 等 host 原语

职责：

- submission
- polling
- completion delivery
- 线程唤醒

不负责：

- continuation 语义
- effect context
- `fork/join`
- 结构化并发策略

### 3.3 Common Scoop runtime package

这一层是平台无关、但不属于 core substrate 的高层 runtime 逻辑，优先用普通 Scoop 实现：

- `Suspend` handler
- completion queue
- waiter table / token registry
- sync / single-thread / multi-thread executor
- `ExecutorScope` / `Child` / `scope` / `fork` / `join`
- `GcHandle.raw` token 生命周期管理
- fail-fast / supervisor / cancellation bookkeeping

这一层是本文希望逐步从 core runtime 中分离出来的主体。

### 3.4 Service / backend cone

这一层承接：

- `HttpClient` / `Db` / `Clock` / `FileSystem` 等服务能力
- Linux backend cone
- Windows backend cone
- 未来按领域拆分的 mixed Scoop/FFI lib

它们可以是混合实现：

- 最底层 host shim 继续走 `ExternAbi::C`
- 上层 managed helper 可逐步使用 Managed ABI
- 主体逻辑尽量写成 Scoop

## 4. Public surface 草案

### 4.1 `Suspend` 只表达“可能挂起”

```scoop
effect Suspend
```

语义：

- `/ Suspend` 表示该计算可能把控制权交给环境，等待外部进展。
- blocking backend 可以合法地同步完成它，而不发生真正的 suspend/resume。
- `Suspend` 不等同于 `fork/join`，也不等同于某种特定 reactor/executor。

### 4.2 服务能力使用普通 capability

```scoop
interface HttpClient {
    fun get(req: Request): Response / (Suspend + Raise<HttpError>)
}

interface Db {
    fun query(sql: String): Rows / (Suspend + Raise<DbError>)
}

interface Clock {
    fun sleep(ms: Int): Unit / Suspend
}
```

这里的关键是：

- `HttpClient` / `Db` / `Clock` 是 capability/interface，不是 public effect taxonomy。
- public effect row 只暴露 `Suspend` 和必要的错误义务。

### 4.3 executor 单独提供结构化并发能力

```scoop
class Child<T, eff E = Pure>

interface Executor {
    fun <R, eff E> run(body: ExecutorScope.() -> R / (Suspend + E)): R / E
}

interface ExecutorScope {
    fun <T, eff E> fork(body: () -> T / (Suspend + E)): Child<T, E>
    fun <T, eff E> join(child: Child<T, E>): T / (Suspend + E)
}

fun <R, eff E> runBlocking(body: () -> R / (Suspend + E)): R / E
```

约束：

- `fork/join` 只能在 `ExecutorScope` 里使用。
- `runBlocking` 提供 `/ Suspend` 的 blocking 解释，但不提供 `fork/join`。
- 不存在“所有 `/ Suspend` 代码默认都能 fork/join”的语义。

## 5. 运行时模型

### 5.1 `Suspend` 和 scheduling 是两回事

`Suspend` 表示“等待外部进展”；scheduler 表示“安排多个可运行计算”。

二者相关，但不是同一层抽象：

- `I/O wait` 只需要进展能力，不要求一定有 executor。
- `fork/join` 需要 scheduler/executor。

### 5.2 `Ticket` 是内部 completion handle，不是 public 语义起点

本文允许 common runtime 内部使用类似 `Ticket<T>` 的等待对象统一：

- I/O completion
- timer completion
- child completion

但 `Ticket<T>` 的定位是：

- runtime 内部的 typed completion handle
- completion state + waiter 的内部载体
- 不是 public surface 的基础语义对象

它和当前 `Task<T>` 的区别是：

- `Task<T>` 更像“计算对象 + completion + public driver”的合体
- `Ticket<T>` 更像“完成凭证 / 等待句柄”
- `Child<T>` 才更接近“被调度的子计算”

### 5.3 blocking backend

在 blocking backend 中：

- `HttpClient.get()` 可以直接执行 blocking 调用并返回结果。
- `Clock.sleep()` 可以直接阻塞线程。
- 这时 `/ Suspend` 只是静态义务；运行时并不一定发生真正的 suspend。

若代码不使用 `fork/join`，则根本不需要 executor。

### 5.4 “同步 executor”

一旦代码使用 `fork/join`，即使底层 I/O 全部是 blocking，也至少需要一个退化的 scheduler。

本文建议该模式实现为“同步 executor”，而不是把 `fork` 直接 inline 成普通函数调用：

- `fork`：创建 child record，放入本地 ready queue，立即返回 `Child`
- `join`：若 child 未运行或未完成，则当前线程帮助推进 child，直到其完成
- `scope` 退出：确保所有 child 被收束

这样可以同时满足：

- `fork` 仍然是 schedule-only
- 程序不会死锁
- 最坏情况下只是完全串行化

### 5.5 单线程 executor

单线程 executor = 同一套 runtime 对象 + 一个 worker。

典型结构：

- ready queue
- timer heap / completion queue
- reactor integration
- 一个 worker event loop

### 5.6 多线程 executor

多线程 executor = 同一套 runtime 对象 + 多个 worker。

典型结构：

- 每 worker 本地 ready queue/deque
- 全局注入队列
- reactor / timer driver
- work stealing 或其它调度策略

语义上和单线程 executor 共享同一套 public API；区别只在调度实现。

## 6. `fork` 的 schedule-only 语义

`fork` 不能被 lower 成“现在就调用 `body()`”。

它必须执行的是：

1. 创建 child 记录
2. 创建新的 runnable computation/fiber
3. 注册到当前 executor scope
4. 推入 ready queue
5. 立即返回 `Child`

它唯一保证的是：

- child 不会以 inline direct call 的方式运行在 `fork` 自身的调用栈上

它不保证的是：

- parent 的下一句一定先于 child 的第一句执行
- child 一定在同一 OS 线程上运行

这些都由 executor flavor 决定。

## 7. FFI / Managed ABI 边界

### 7.1 Managed ABI 的定位

Managed ABI 的目标是：

- 标准化 managed value 过边界时的 ABI 合同
- 让 external managed helper / cone 不再依赖编译器按 FQN special-case
- 把非 substrate helper 从 core runtime 中切出去

它不是：

- effectful FFI
- continuation crossing ABI
- outward suspend ABI

### 7.2 effectful FFI 当前显式不做

本文固定以下边界：

- 不让 continuation / resume interface 穿过 FFI 边界
- 不让 effect context 穿过 Managed ABI
- 不把 reactor callback 建成“native 直接 resume continuation”的语义

也就是说，effectful FFI 在当前阶段是显式 deferred 的：在这套交互模型想清楚之前，不把它作为 async/runtime 架构成立的前提。

换句话说：

- native 侧可以做 registration / polling / completion delivery
- Scoop 侧负责 waiter 管理、scheduler、resume

### 7.3 基于 `GcHandle.raw` 的 completion token

对于 async backend，推荐使用现有 `GcHandle.raw` 作为长期 opaque token：

1. Scoop 侧创建 registration/completion 对象
2. 生成 `GcHandle`，把 `raw` 交给 native registration state
3. native 侧仅保存 opaque token
4. completion 到来后，native 侧把 token 放入 completion queue 或通过 token 记录完成
5. Scoop 侧再用 `GC.handleGet` 找回对象，写入结果并安排 resume

这样：

- 不需要 effectful FFI
- 不需要让 continuation 穿越 FFI
- blocking backend 与 async backend 可以共存

### 7.4 平台 cone 的实现风格

平台 cone 预期会是 mixed Scoop/FFI lib：

- 最薄 host shim：`ExternAbi::C`
- managed-facing helper：可逐步使用 Managed ABI
- 主体逻辑：普通 Scoop

这也是 Managed ABI 的主要价值所在：不是新增 async 语义，而是为这种 mixed lib 建立正式边界。

## 8. 可替换 backend 模型

理想情况下，以下组合都应成立：

- blocking backend + `runBlocking`
- blocking backend + 同步 executor
- `poll` backend + 单线程 executor
- `epoll` backend + 单线程 executor
- `epoll` backend + 多线程 executor
- `io_uring` backend + 多线程 executor
- `IOCP` backend + 多线程 executor

要求：

- public surface 不变
- 只是 backend cone 与 executor 实现不同
- service API 也不因 backend 而改签名

## 9. 与现有 `async` / `Task<T>` 的关系

本文不要求立即删除现有 `async` / `await` / `Task<T>`。

更现实的迁移方向是：

1. 先把真正的 runtime/helper/cone 边界画清楚。
2. 让高层 runtime 库可以在现有 substrate 之上实现。
3. 再决定：
   - `Task<T>` 是否降级为 adapter
   - `async/await` 是否保留为语法糖
   - public 主线是否改为 `Suspend + ExecutorScope`

换句话说，本文优先解决的是架构分层，不是立刻做语法替换。

## 10. 迁移方向

### 10.1 应留在 core substrate 的部分

- effect / continuation substrate
- state machine lowering
- GC / allocator / write barrier / pin / handle
- host substrate：线程 / 原子 / 条件变量

### 10.2 应逐步迁出 core runtime 的部分

- 非 substrate 的 helper
- completion queue / waiter registry
- executor 实现
- `scope/fork/join`
- 平台 reactor/backend 适配层
- 高层 service runtime 包

### 10.3 Managed ABI 的作用

Managed ABI 的主要作用是：

- 把 mixed Scoop/FFI lib 的 managed 边界标准化
- 让 helper/cone 可以成为独立包
- 让 compiler 以后按 ABI 处理 helper，而不是按名字处理 helper

它不是本文 async/runtime 语义成立的唯一前提；本文 async/runtime 架构的可实现性主要来自已有 substrate。

## 11. 仍待收口的问题

1. effect 最终命名是否采用 `Suspend`、`MaySuspend`，还是兼容保留 `Concurrent`。
2. `Child<T>` 是否需要类型级“不得逃出 owning scope”的约束。
3. cancellation 的最小 public surface 应该放在 `ExecutorScope` 还是先只保留内部语义。
4. common runtime 与 platform cone 之间的最小 completion/submission SPI 应如何标准化。
5. 现有 `Task<T>` 是否仅保留为 adapter，还是继续保留为一等 public surface。

## 12. 一句话总结

本文的核心主张不是“新增一套 async substrate”，而是：

**在复用既有 continuation/effect/GC/host substrate 的前提下，把“可能挂起”“并发调度”“平台 backend”“managed helper/cone”四层边界重新画清楚，并把非 substrate 的 runtime/helper 逐步迁出 core runtime，收口为可替换的 Scoop/FFI mixed lib。**
