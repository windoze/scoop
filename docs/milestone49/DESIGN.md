# M49 设计：异步 I/O 与结构化并发基础

状态：实施中。日期：2026-10-16。本文确定 M49 的结构化并发基础、native 异步 I/O 后端、事件循环、取消与关闭语义，以及未来 TLS stream 所需的接口约束。已定稿的 core API 见语言规范 11.9.1～11.9.2；实现和验收进度见 [PROGRESS.md](PROGRESS.md)，未完成部分不表示已经交付。

依据：[语言规范](../specs/SCOOP-SPEC.md) 8.2、8.3、11.9，[实现规范](../specs/SCOOP-IMPL-SPEC.md)的协程降级与 2.16，[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)第 8、9 章，以及[标准基础库规范](../specs/SCOOP-STDLIB-SPEC.md)和[平台库规范](../specs/SCOOP-PLATFORM-SPEC.md)。实施前先将定稿的公共合同写入相应 spec，再修改代码。

## 1. 目标与范围

异步 I/O 需要明确回答：一次操作属于哪个任务、调用方何时能退出、取消如何传到等待中的操作，以及结果或失败由谁接收。M49 先建立 Kotlin 风格的 coroutine／Job／task tree，使这些行为由统一的库规则承担。

结构化并发部分包括：

- Job 的父子关系、结构化作用域与任务完成条件。
- `launch`、`async`、等待任务结束与取得结果。
- 父子任务的失败传播、协作取消与退出时的收尾等待。
- 可取消挂起操作的完成协调，供后续异步 I/O 接入。
- 复用已有 TaskContext 的当前 Job 绑定与传播。

异步 I/O 部分交付同一事件循环上的定时等待、DNS、TCP、UDP 与文件基本操作，提供可组合的异步字节流、显式关闭和跨挂起 buffer 借用。首版沿用平台库的三个目标：`aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`。

TLS 与 HTTP 不在本轮交付范围。TLS stream 作为明确的后续使用者约束字节流的组合、双向进展和资源语义，本轮不引入 TLS 库、证书配置、握手 API 或 HTTP 协议实现。文件目录遍历、文件监听、进程／TTY 的异步封装和任意 native fd 接管也不作为本轮完成条件。

结构化并发是 `scoop.core` 的一部分，以普通 Scoop 代码实现任务树、作用域和取消。异步 I/O 是独立的普通 Cone；平台线程、线程池和具体 I/O 驱动不进入 core。并发表示多个任务的生命周期可以重叠，不承诺每个任务占用一个线程。

本设计必须完全通过 Scoop 与既有 Cone 机制实现，不以修改编译器或底层运行机制作为功能实现手段。结构化并发可以使用 fixture 验证；异步 I/O 以 `scoop.test` 为主要验证方式，只有它无法满足具体验证需求时才使用 fixture。底层缺口的处理流程见第 2 节，验收分工见第 14 节。

恢复拦截钩子暂不设计，不作为本基础部分的前置工作。首版不提供脱离作用域的全局任务、监督式失败策略或一套通用执行器框架；这些不影响建立异步 I/O 所需的基本任务树。

## 2. 现有基础与库分层

已有 core 提供 `Continuation<T>`、`SuspendTask<T>`、`SuspendRegistration<T>`、`startCoroutine` 和 `suspendCoroutine`。它们已经支持同步完成、真实挂起、异常恢复和单次完成。TaskContext 已经支持启动时建立 child context，以及恢复协程时切换到 frame 保存的 context。

这些原语可以直接承载普通库的任务树，无需新增源码 `async` 关键字、Job intrinsic 或另一套协程状态机。

| 层次 | 责任与依赖 |
| --- | --- |
| `scoop.core` | 在既有挂起、启动、恢复、TaskContext 与原子能力上，用普通 Scoop 实现 Job、父子关系、作用域、结果等待、取消和挂起适配；不依赖 std、platform 或 asyncio，不包含具体 I/O 调度策略 |
| 独立异步 I/O Cone，工作名 `scoop.asyncio` | 位于 `sysroot/lib/scoop.asyncio/`，显式依赖 core、std 和 platform；定义 EventLoop、异步字节流与资源接口，本轮提供 LibuvEventLoop 实现，通过普通 C11 shim 接入 libuv；`scoop.test` 仅作为测试依赖 |
| `scoop.platform` | 提供地址、路径、错误、NativeByteBuffer 和 GC-free OS helper；保留已有 core／std 依赖，不反向依赖 asyncio；自身所缺的库能力先在所属层补齐 |
| libuv 与 Cone native 适配 | 作为 asyncio 的普通 native 构建输入，提供事件循环与 native 工作线程池；沿用既有 FFI、GC 与链接合同 |

结构化并发的公开声明归入 core；异步 I/O 使用正常 manifest、依赖和 import，不默认注入所有程序。Job 和作用域是普通库对象，不新增 compiler-known 身份、intrinsic 或私有 runtime 入口；编译器和 runtime 继续处理普通 managed 对象、调用、异常与 GC。

实现边界如下：

- 任务树、取消、操作协调和流组合使用普通 Scoop 源码。libuv 及必要的 C11 shim 通过既有 Cone native sources／依赖与公开 FFI 接入；shim 只承担 native I/O、内存和 ABI 适配，不接管任务树、协程降级或 managed 调度。
- 本功能不新增编译器特判，不修改协程 lowering、codegen、链接器、runtime、GC 或 ABI 来专门支持 Job／asyncio；不依赖私有布局或绕过正常的跨 Cone 可见性与调用规则。
- 合法的 Scoop／Cone 用法若暴露底层缺陷或能力缺口，必须暂停依赖该能力的上层实现，建立最小复现，先核对并按需修订对应 spec，在所属底层修复并建立独立回归，验证通过后再回到本设计继续实现。底层修复作为独立前置工作处理，不能在库内用特例、改写语义或转移到 C 实现来绕过缺陷。
- 第 8.2、10.3 节所需的错误域与异步 buffer 借用属于 platform 库的前置能力，须先在该普通 Cone 中完成规范、实现和验证；asyncio 只消费正常可访问的接口，不能自行延长同步指针借用、访问私有字段或复制一套 buffer 状态来绕过缺口。若这些库能力又暴露编译器或 runtime 问题，继续遵守上述先修底层的次序。

## 3. 基础 API 草案

core 结构化并发的 package、声明形式、访问性和参数签名已在语言规范 11.9.1～11.9.2 定稿。采用已有普通方法与泛型扩展表达 API，不要求 generic virtual method 或新的函数类型规则。

| API 方向 | 职责 |
| --- | --- |
| `Job` | 表示一个任务的生命周期；提供取消请求、完成状态和 `suspend join(): Unit` |
| `Deferred<T>` | 带结果的 Job；`suspend await(): T` 等待并取得结果，或抛出任务的失败／取消异常 |
| `CoroutineScope` | 持有任务归属，作为创建子任务的入口；不是线程或线程池 |
| `suspend coroutineScope(block)` | 建立嵌套作用域，执行正文，并在正文及其全部子任务结束后返回或抛出 |
| `scope.launch(block)` | 创建返回 Unit 的子任务，返回 Job |
| `scope.async(block)` | 创建带结果的子任务，返回 Deferred |
| `ensureActive()` | 检查当前 Job 的取消状态，需要时抛出库提供的取消异常；检查本身不挂起或切换线程 |

初版 `launch`／`async` 在完成父子登记后使用 `startCoroutine` 启动，允许正文在调用栈内运行到第一次真实挂起或完成。返回句柄前任务可能已经完成，不能把“已返回句柄”当作任务开始的前提，也不以创建线程实现每次启动。

以下示例只展示 API 的组合方式，`readBoth` 是应用函数，不是拟新增的标准库 API：

```scoop
suspend fun <A, B> readBoth(
    readLeft: suspend () -> A,
    readRight: suspend () -> B
): (A, B) = coroutineScope suspend { scope ->
    val left = scope.async(readLeft)
    val right = scope.async(readRight)
    (left.await(), right.await())
}
```

两个读取都可能先挂起，再由后续 I/O 事件推进；作用域负责它们的共同生命周期。普通 `main` 可通过第 9 节的 `EventLoop.run` 启动根协程并进入 I/O 驱动；EventLoop 接口与本轮提供的 LibuvEventLoop 实现都属于异步 I/O Cone，底层仍使用既有启动原语。

## 4. 任务树与完成条件

### 4.1 归属与创建

- `coroutineScope` 在已有当前 Job 时建立子作用域；在底层协程入口没有当前 Job 时建立本次结构化工作的根作用域。根仍有明确的调用方和结束通知，不成为进程级全局任务。
- `scope.launch`／`scope.async` 的父任务由接收者 scope 确定，每个子任务只有一个父任务。关系在启动前建立，后续不重新挂到另一棵树上。
- 父任务必须先记录子任务，再允许其正文执行；同步成功、同步失败以及启动与取消的竞争均不能遗漏子任务。
- scope 正文退出后关闭新的直接子任务登记，再等待已有子任务。使用已关闭 scope 创建任务报普通状态错误，不能悄悄产生无人等待的任务。仍在执行的子任务可以按自身作用域规则管理其后代。

### 4.2 正文结束与任务结束

正文返回只表示本任务的计算结束，不等于 Job 已完成。正常完成还要求所有已登记子任务都已结束；子任务遵守同一规则，因此父任务的完成也覆盖后代。

Job 的最终结果只发布一次，为成功、失败或取消。取消请求尚未完成清理时不能观察为已完成。等待者在最终结果发布后恢复；完成时释放不再使用的子任务与等待者引用，结果句柄仅保留其公开行为所需的数据。

`join` 只等待目标结束，不负责重新抛出目标任务的失败；`await` 取得带类型的结果并传播该任务的失败。两者都允许多个等待者和完成后的再次调用，不消费一次性结果。等待者自身被取消时，撤销的是这次等待，不直接取消被等待的任务；已有父子取消传播仍按第 5 节执行。

## 5. 失败传播与协作取消

### 5.1 普通作用域的失败策略

首版采用普通结构化作用域的失败传播方式：

1. 正文或任一子任务以非取消异常失败时，所属作用域进入失败收尾，向其他活动子任务发出取消请求。
2. 作用域等待相关正文和子任务完成收尾后，再向调用方传播失败。不能先报告完成，再让失败任务或被取消的同级任务继续使用作用域拥有的资源。
3. `async` 的失败同样参与父任务的失败传播，即使调用方尚未执行 `await`。单纯捕获一次 `await` 异常不撤销已经发生的父作用域失败。
4. 单纯取消某个子任务只向其后代传播，不作为普通失败自动取消父任务。取消过程中实际发生的非取消异常仍按失败处理。

并发发生多个失败时，至少保留首先被记录的非取消失败，取消异常不能覆盖它；并发主失败的选择不承诺固定源码顺序。首版仅保留首先记录的非取消失败，其余独立失败不覆盖它，不扩展 core 的 Throwable 合同。单个正文内的 `try`／`catch`／`finally` 仍按语言原有的异常覆盖规则执行。

### 5.2 取消请求与取消观察

取消是幂等的请求，沿 Job 树向下传播，不是强制停止线程或立即销毁 frame。core 以普通异常类型表达取消，工作名 `CancellationException`，不引入特殊的 runtime 异常类别。

任务通过 `ensureActive` 或库提供的可取消挂起点观察取消。对正在执行且不检查取消的计算，不承诺立即停止。直接使用原始 `suspendCoroutine` 的任意第三方操作也不会自动变为可取消操作；它最终恢复后才能继续执行后续检查。

取消已挂起任务时，库通过所持有的 continuation 提交取消异常，让异常从原挂起点进入正常展开路径。不能以丢弃 continuation 代替取消，否则该调用的 `finally` 不会因此执行。

作用域在失败或取消后的内部收尾等待必须等到子任务真正结束，不能因为自身已取消就立即中止这段等待。公开的 `join`／`await` 可取消等待与作用域自身的收尾等待应分开处理；这里不要求先发布通用的取消屏蔽 API。

## 6. 可取消挂起与异步 I/O 接入

### 6.1 协调一次操作的完成

core 的结构化并发实现以普通 Scoop 在 `suspendCoroutine` 上包装可取消的等待，协调外部结果、外部失败和所属 Job 的取消，并提供供普通 Cone 接入的公开合同。底层 continuation 的单次完成规则保持不变。

| 事件或竞争 | 所需行为 |
| --- | --- |
| 操作登记前已取消 | 以取消结束本次等待，避免启动无用的外部操作 |
| 登记期间发生同步完成 | 使用已有同步完成路径，允许当前调用栈继续；不强迫所有操作都实际挂起 |
| 操作完成与取消并发 | 由库内同一次完成判定选出一个结果，只向底层 continuation 提交一次；不能通过重复 resume 后捕获异常来竞争 |
| 取消发生在登记过程中 | 登记与撤销必须衔接，不能遗漏取消或留下仍无人管理的操作 |
| 取消先结束等待，稍后才收到外部完成 | 迟到事件不再次恢复协程；I/O 适配仍完成该事件所需的资源清理 |
| 等待结束 | 移除不再需要的取消登记与等待者引用，不持续保活已结束的 frame |

首版以这次完成判定决定结果，不额外承诺“成功已经选定后，执行前的取消一定覆盖成功”。公共签名和同步登记异常按语言规范 11.9.2 定稿，保留 core 11.9 的既有协议。跨挂起 buffer 操作通过 Job.invokeOnCancellation 与原始 suspendCoroutine 协调资源收尾，不使用立即取消等待来提前归还借用。

Job 状态、子任务集合、等待者和结果发布使用现有原子与同步规则协调，不能因为没有线程池就假定回调总在同一线程。实际恢复 continuation、通知用户代码或请求外部操作取消应在内部临界区之外执行，允许同步完成和重入。

### 6.2 I/O 的等待结束与资源结束

每个异步 I/O 适配在提交操作时确定所属 Job，并负责把完成通知和取消请求接到上述等待机制。用户仅等待另一个已有任务时不转移该任务的所有权；直接发起的 I/O 则按该操作的任务归属管理。

取消协程的等待不等于 OS 已停止访问 buffer、句柄或回调记录。I/O 层需要区分等待结果已经确定与底层操作已经停止使用资源；在后者成立前继续保活必要资源，迟到回调也必须有正常收尾路径。借用调用方 buffer 的操作在解除借用前不能向调用方报告完成，包括以取消结束；能够提前结束等待的适配必须自行持有所需资源，不能继续访问调用方已经可以复用或关闭的借用。具体撤销方式、部分进度、关闭行为和 buffer 借用合同见第 10、11 节。

平台事件等待、线程使用、native callback 与 GC／FFI 边界沿现有平台和 runtime 规则处理。可复用的同步 I/O 类型与资源规则应先核对现有平台规范，不能把同步借用期直接延长到挂起之后。

## 7. TaskContext 与恢复方式

Job 树属于库对象关系，TaskContext 属于已有的逻辑执行上下文，两者不合并成新的 runtime 任务类型。

- 创建子任务时，以公开 Job 的精确静态类型安装该子任务的当前 Job 绑定，再通过 `startCoroutine` 继承其他有效 binding。恢复后的正文看到自己的 Job；父任务与子任务后续的 Context 重绑定互不影响。
- 原始 `startCoroutine` 只执行现有 Context 传播，不自动创建或登记 Job。只有 core 的结构化并发构建器建立任务树关系。
- `suspendCoroutine` 的 I/O 登记在原任务中进行；适配捕获实际的操作状态与 Job 引用，不能在外部完成回调中按回调当时的 Context 猜测操作归属。
- 真正恢复时仍由已有机制切换到 continuation 所属 frame 的 TaskContext，再在退出驱动时恢复 resumer 的 Context；库不重复实现这套切换。

任务树本身不承诺所有挂起操作都回到指定线程。本轮 I/O 适配按第 9 节投递恢复工作；无需为了任务树增加 core 恢复拦截，也不通过包装最终 completion 冒充对中途挂起点的拦截。

## 8. 异步 I/O 范围与 native 后端

### 8.1 后端选择与交付面

本轮提供 `LibuvEventLoop` 作为 `EventLoop` 接口的一个实现，使用 libuv，设计基线为 2026-09-24 发布的稳定版 1.53.0。它作为 `scoop.asyncio` 的固定版本 native 依赖，使用既有 Cone native 构建与链接机制交付，不成为所有程序的 runtime 依赖。C shim 隐藏 `uv_*` 结构布局；Scoop 公开接口不包含 libuv 类型或 fd。实现前锁定实际源码与目标构建输入，升级按普通依赖变更处理，不运行时猜测系统 libuv 的能力。

| 本轮能力 | 后端与边界 |
| --- | --- |
| 定时等待 | `uv_timer_t`；基于单调时间，不阻塞 loop 线程，不承诺精确唤醒时刻 |
| TCP connect／listen／accept／读写／输出半关闭 | `uv_tcp_t` 与 stream request；Linux 使用 libuv 的 epoll 后端，Darwin 使用 kqueue |
| UDP bind／sendTo／receiveFrom | `uv_udp_t`；保持 datagram、空包和截断语义，不实现字节流接口 |
| DNS resolve | `uv_queue_work` 在 libuv 工作线程执行既有平台 `getaddrinfo` helper；输入为拥有独立存储的原始名称字节 |
| 文件 open／顺序 readInto、writeFrom／sync／close | `uv_queue_work` 执行既有 GC-free 文件 helper；复用 Path、OpenOptions、原始错误与单次 close 合同 |

网络等待由事件循环完成；文件和 DNS 由 libuv 的 native 工作线程池完成。该线程池由 libuv 在进程内共享，不是运行 Scoop 协程的 executor；worker 不进入 managed 代码、不分配 GC 对象，也不调用 Scoop continuation。首版不启用 io_uring，不另建线程池或第二套 epoll／kqueue 实现。[后端设计](https://docs.libuv.org/en/v1.x/design.html)、[线程池](https://docs.libuv.org/en/v1.x/threadpool.html)、[版本基线](https://github.com/libuv/libuv/releases/tag/v1.53.0)。

### 8.2 保留现有平台合同

同步 `File`／`TcpStream` 与异步资源分别拥有自己的 native 句柄；本轮不提供改变现有对象阻塞模式、借用现有 fd 或同时由两套驱动消费一个 socket 的入口。地址、路径、buffer 等数据类型直接复用平台库，异步 API 不复制另一套平台值类型。

文件和 DNS 统一使用工作请求调用既有 OS helper，而不直接用 libuv 对应的高级包装替换其语义：`uv_getaddrinfo` 会执行 IDNA 转换；`uv_fs_close` 会把部分错误视为成功，Darwin 的 `uv_fs_fsync` 也有额外处理。Scoop 保留名称原始字节、EAI_SYSTEM 对应的 errno，以及平台规范中的 close／sync 行为。worker 在实际调用后立即捕获错误，不到完成回调或 managed 恢复时重新读取 errno。[DNS 源码](https://github.com/libuv/libuv/blob/v1.53.0/src/unix/getaddrinfo.c)、[文件源码](https://github.com/libuv/libuv/blob/v1.53.0/src/unix/fs.c)。

实际 OS helper 错误继续使用 `NativeError` 的 PosixErrno／AddressResolution 域。libuv 网络与提交错误保留原始返回码，设计上为 `NativeErrorDomain` 增加 Libuv 分支并映射已有 `PlatformErrorKind`；不把 libuv 返回值或 loop 线程的 errno 冒充原 syscall 错误。`UV_EOF`、无数据通知以及由本库取消／关闭产生的完成状态按第 11 节解释。该错误域是待补齐的平台库前置能力，须先写入平台 spec，在 platform Cone 实现并验证，再由 asyncio 接入；不增加 runtime ABI，也不把 libuv 依赖引入 platform。

### 8.3 SIGPIPE 与 native 边界

异步输出继续满足平台规范 6.4、12.3：断开的 socket／fd 写入不能默认终止进程，不修改进程级 signal handler。libuv 的 Unix stream 路径会调用 write／writev，不能假定已有 MSG_NOSIGNAL 保护自动覆盖它。

Linux 的 C shim 在可能实际执行 socket 输出的 libuv 提交调用及 loop 驱动调用周围使用线程局部 SIGPIPE 屏蔽，保存旧 mask／pending、只处理本段新增信号、保存操作结果后恢复状态；恢复 managed 代码前完成恢复。Darwin 在 socket 可用后、任何输出前设置 SO_NOSIGPIPE。文件 worker 复用现有 fd 输出 helper 的保护，信号处理发生在实际执行写入的线程。不能只在 Scoop 发起挂起的瞬间屏蔽信号，却让稍后的 loop／worker 写入失去保护。[libuv stream 实现](https://github.com/libuv/libuv/blob/v1.53.0/src/unix/stream.c)。

## 9. EventLoop 接口、根协程与完成投递

### 9.1 接口与本轮实现

`EventLoop` 是普通 Scoop interface，定义根协程启动、异步 I/O 驱动与退出收尾的合同；`LibuvEventLoop` 是本轮提供的具体实现。采用 EventLoop 这个名称，是因为该抽象还负责完成投递与运行生命周期。它持有一棵由 core 管理的任务树，但不重新定义 Job、取消或 scope；子任务仍由 `CoroutineScope.launch`／`async` 创建。接口与实现先放在同一个普通 Cone 中，不为尚无调用者的其他后端增加工厂或插件机制。

| 声明 | 职责 |
| --- | --- |
| `EventLoop.run(block: suspend (CoroutineScope) -> Unit): Unit` | 非 suspend 的接口方法；启动一个根协程，在调用线程驱动事件，等待根任务、后代与资源收尾完成后返回或抛出 |
| `fun <T> EventLoop.runWithResult(block: suspend (CoroutineScope) -> T): T` | 普通泛型扩展；通过 run 执行 block、保存类型化结果，只有 run 成功完成后才交付结果；失败与取消继续传播 |
| `LibuvEventLoop()` | 创建实现 EventLoop 的普通对象；native loop 在 run 接受本次运行后初始化，未运行对象不持有需要关闭的 native 资源 |

接口方法不声明自身的类型参数，遵守现有语言对泛型虚方法的限制。runWithResult 使用普通 Scoop 对象保存结果，不擦除为裸指针、不新增 runtime 结果通道；run 的完成同步保证根任务及后代的写入对返回方可见。它复用同一次运行与收尾，不另启动一棵任务树。

I/O 的实际派发也经过 EventLoop 接口。以下均为普通非泛型 suspend 成员，具体参数与默认值在独立库规范定稿，数据类型复用平台库：

| 接口成员方向 | 结果与职责 |
| --- | --- |
| `sleep(duration)` | 定时等待，返回 Unit |
| `resolve(host, port, family)` | 返回平台地址值，保持原始名称与错误合同 |
| `connectTcp(address)`／`listenTcp(address, backlog, reuseAddress)` | 分别返回 AsyncTcpStream／AsyncTcpListener 接口 |
| `bindUdp(address, reuseAddress)` | 返回 AsyncUdpSocket 接口 |
| `openFile(path, options)` | 返回 AsyncFile 接口 |

便捷顶层函数如需提供，只查找 TaskContext 中静态类型为 `EventLoop` 的绑定并转发；资源创建后由相应资源接口承担读写、关闭与所属 loop 检查。通用 API 不强转为 LibuvEventLoop，也不检查具体后端类型；libuv 的提交、轮询、唤醒和完成记录属于实现细节。原 `runIo` 不再承担独立的驱动合同，如保留，只能是构造 LibuvEventLoop 并调用 runWithResult 的便捷封装。

下面只展示经接口启动根协程、创建子任务并取得结果；这些计算允许同步完成：

```scoop
fun main() {
    val loop: EventLoop = LibuvEventLoop()
    val result = loop.runWithResult suspend { scope ->
        val left = scope.async suspend { 20 }
        val right = scope.async suspend { 22 }
        left.await() + right.await()
    }
    println(result)
}
```

run 是无当前 Job 的同步根入口；已有结构化任务使用 scope 创建子任务，不能通过阻塞 run 脱离当前任务树。它也不能在已有活动 I/O 上下文中嵌套调用或从 native callback 重入驱动。一次接受的 run 按以下顺序启动：

1. 确定所属线程，准备命令／完成队列、唤醒能力和 native loop，再允许用户正文执行。
2. 以接口的精确静态类型 `EventLoop` 安装当前实例的 TaskContext 绑定，通过已有 `startCoroutine` 启动包装了 `coroutineScope(block)` 的首个协程；core 在进入 block 前建立根 Job，并将相应 CoroutineScope 交给 block。
3. 同步完成与真实挂起都由同一驱动接收。根 completion 记录结果并在需要时唤醒驱动；它不直接释放 loop，也不把正文返回当作整次运行结束。run 退出时恢复调用方原有 Context。

首版不隐式创建常驻事件线程。每个资源／操作固定归属一个 EventLoop 实例；用户 I/O 需要有效的当前 Job 与该实例的活动绑定，显式调用另一个 loop 的 I/O 方法同样报状态错误。LibuvEventLoop 的 native handle／request 固定归属内部的一个 libuv loop，全部 native 操作在所属线程执行，只有文档允许跨线程的唤醒入口例外。

本库 I/O 的实际挂起恢复由所属 loop 投递，立即完成仍可沿第 6.1 节同步返回。其他库的挂起点仍可能把协程恢复到其他线程，因此不声明整个 run 的 block 永远运行在入口线程。此后调用 I/O 时，实现把请求送回所属 loop；共享 managed 状态仍按既有同步合同访问。长时间不挂起的计算会占用执行它的线程，本轮没有 CPU 抢占或并行执行承诺。

### 9.2 跨线程投递与驱动步骤

本节规定 LibuvEventLoop 的实现方式；EventLoop 接口不暴露 libuv 队列或唤醒 handle。

外线程提交、取消或完成通知先进入受同步保护的命令队列，再通过 `uv_async_send` 唤醒 loop。唤醒只表示可能有工作；一次通知可以对应多条命令，不能用通知次数代替队列内容。投递与 loop 进入关闭状态在同一同步边界协调，该边界覆盖入队至唤醒调用结束的整个投递过程；关闭唤醒 handle 前停止接受新投递，等正在投递的调用退出并处理完已接受的命令，防止外线程访问已释放的 native loop。[async 唤醒合同](https://docs.libuv.org/en/v1.x/async.html)。

驱动交替执行以下步骤：

1. 取出已提交的命令，在 loop 线程创建／撤销 native 操作；内部锁外执行实际 native 调用。
2. 有 managed 工作可执行时，以 `UV_RUN_NOWAIT` 推进就绪 native 事件；没有可运行工作而仍需等待时，以 `UV_RUN_ONCE` 等待。native 等待使用普通 NativeSafe FFI。
3. 返回 managed 后处理完成记录，提交 buffer 进度、释放借用、处理未交付资源，再把可以恢复的 continuation 放入就绪队列。
4. 取就绪队列的一批现有工作，在所有内部锁之外恢复；新产生的工作进入后续轮次，使连续完成的操作之间仍有机会推进 native 事件。

不得递归调用 `uv_run`，也不得在仍有待投递的完成、取消或恢复工作时进入阻塞等待。唤醒 handle 在驱动期间维持等待能力；退出条件使用本库根任务与资源状态，不能仅凭 `uv_loop_alive` 推断结构化作用域已经结束。[loop API](https://docs.libuv.org/en/v1.x/loop.html)。

### 9.3 GC／FFI 与操作记录

LibuvEventLoop 的 native 侧只保存地址稳定的 C handle／request、native buffer 地址及普通操作编号。libuv callback 和 worker 均为 C 函数，不把 managed ref 放进 `void *data`，不调用 Scoop closure 或恢复 continuation。callback 可停止本次读订阅、保存状态及字节数、追加完成记录；完成节点随请求预留，避免依赖 callback 内的 managed 分配。

managed 侧的活动操作表持有 continuation、提交时捕获的 Job、资源 owner 与 buffer owner，均参与正常 GC 扫描。C 中的编号只用于本进程内找到该操作，在 native 完成与排队事件消费完之前不能复用。完成状态以带种类的数据区分字节数、EOF、错误和新获得的资源，不用一组无约束的可空字段推测事件含义。

`uv_run` 返回并完成 NativeSafe 返回握手后，managed 代码才消费事件和恢复任务。提交函数同步失败时走同一收尾路径，不能等待不会到来的 callback。恢复使用原 continuation 的 TaskContext，不从 C callback 当时的线程状态猜测所属 Job。此方案不需要 suspend FFI、异步裸 FunPtr、额外 GC pin 或新增 runtime 调度入口。

### 9.4 EventLoop 的显式生命周期

首版一个 EventLoop 实例只接受一次 run，对应一棵根任务树，依次经历已创建、运行中、收尾中和已关闭。并发调用或完成后再次调用 run 均报状态错误，拒绝的调用不影响已接受的运行；初始化失败也在释放已创建资源后进入关闭状态。运行结束后不复用该实例，下一次运行创建新实例。尚未运行的对象没有 native 资源，不需要依赖 GC hook 关闭。

EventLoop 实现显式拥有本次运行中尚未关闭的 I/O 资源，维护活动操作与资源集合；用户提前 close 会解除对应保有。这个集合承担本次驱动的生命周期，不把任意 Job 或所有普通对象变成自动资源管理器。驱动自身的内部收尾由 loop 持有并等待，不要求在已完成的根 Job 下登记新子任务，也不受用户操作的取消前置检查阻断。

正常退出先等正文及子任务结束；失败或取消先按任务树传播，并继续驱动其清理。随后停止接收新的用户操作，对本次仍打开的资源执行第 11.3 节的 close，处理所有请求完成、资源关闭 callback 与清理事件。LibuvEventLoop 最后关闭唤醒 handle 并调用 `uv_loop_close`，只有它成功且本库无遗留操作时才释放 native loop 存储。初始化半途失败也按实际已创建的对象逆向清理。

提前结束通过 core 的 Job 取消请求进行，loop 仍驱动到任务与资源收尾结束；首版不提供直接停止轮询并遗留活动任务的公共 stop。run 自身保证确定性的退出清理，不另要求调用者对 loop 执行一次 suspend close。

从 runWithResult 返回或在正文中保存的普通结果可以逃逸；与该 loop 绑定的 I/O 资源在 run 退出后已经关闭，逃逸引用不能延长 loop 生命周期。原生工作一旦进入不可撤销的阻塞调用，退出可能需要等待它结束；不承诺取消或 close 的固定时延，不通过遗弃线程或释放仍被使用的存储提前返回。

## 10. 异步字节流与 buffer 合同

### 10.1 最小公共协议

以下均为普通异步接口；LibuvEventLoop 返回相应的实现对象，公开协议不包含 libuv 存储。普通同步接口保持原有签名。名称可在发布前统一，行为按本节确定。

| 接口／类型 | 合同 |
| --- | --- |
| `AsyncByteSource` | `suspend readInto(buffer: NativeByteBuffer, count: Long): Long` |
| `AsyncByteSink` | `suspend writeFrom(buffer: NativeByteBuffer, count: Long): Long` 与 `suspend flush(): Unit` |
| `AsyncByteStream` | 组合 source／sink，并承诺允许一读一写同时在途；不要求 fd、地址查询或 TCP 半关闭 |
| `AsyncCloseable` | `suspend close(): Unit`；只有承担资源关闭责任的对象实现，不并入 source／sink 协议 |
| `AsyncTcpStream`／`AsyncTcpListener` | 由 EventLoop.connectTcp／listenTcp 创建；前者组合双向字节流与输出 shutdown，后者提供 accept；二者分别承担 close 责任 |
| `AsyncFile` | 由 EventLoop.openFile 创建，组合 source／sink 与 close，提供显式 sync；首版顺序读写共享文件位置，读／写／sync 同时最多一个，不承诺双向网络式进展 |
| `AsyncUdpSocket` | 由 EventLoop.bindUdp 创建，提供 sendTo／receiveFrom 与 close，复用 SocketAddress、Datagram 等平台数据类型 |

提供普通 suspend 泛型扩展 `readFully`／`writeFully`／`copyTo` 和作用域资源辅助函数（工作名 `useAsync`）。完整读写循环复用上述基本操作，逐次提交进度；正文与资源清理失败按既有 `ResourceUseException` 的组合方式表达。异步 close 不能通过同步 `Disposable.dispose` 冒充完成，也不改写 core 已有 use 的语义。

### 10.2 进度、EOF、缓冲与并发

- count 非负且不超过当前可写／可读区间。零长度请求检查合法状态后直接返回零，不执行 native I/O，也不表示 EOF。正请求允许短读／短写；暂时无数据或暂不可写时挂起，不能以零冒充完成或忙循环。单次 native 提交按后端长度表示与实际 syscall 上限选取合法前缀，不做溢出截断。
- 正请求 read 返回零才表示 EOF；EOF、传输错误与显式关闭分开。读端 EOF 不自动关闭写端。正请求 write 无进度按现有 `WriteZeroException` 处理。
- read 返回该抽象层交付的字节数，推进 writer；write 返回该层已经接受的输入字节数，推进 reader。返回后该次借用结束，调用方可以复用 buffer；层内缓冲须由该层自行拥有。write 不承诺对端收到、消费或持久化。
- flush 可挂起，负责把本层此前接受的积压输出向下游推进并传递 flush；不代表文件 sync 或协议关闭。原始 TCP／文件没有用户态输出缓冲时只需检查状态。输出操作之间的顺序由调用者安排，不能在一个 write 尚未结束时并发 flush。
- 双向 stream 允许各一个 read 和 write 同时在途，两方向并行时使用不同 buffer；同一 buffer 须等待前次借用结束才能复用。同方向并发／重入在启动新操作前报状态错误，首版不暗中建立无限排队。只在短暂状态转换期间持锁，不持有覆盖两方向的锁跨越挂起。组合接口本身不要求两个方向能同时执行 native 协议代码。
- 普通请求直接归属于发起调用的当前 Job，不为每个 read／write 私建 `async` 子任务。正常 I/O 失败在该调用点抛出，是否导致任务树失败由调用方是否处理决定。

### 10.3 跨挂起借用与部分进度

现有 NativeByteBuffer 的同步 `withReadableBytes`／`withWritableBytes` 仍禁止指针逃出 callback，不能直接用于跨挂起借用。异步操作接入前，须先在 platform Cone 补齐可供普通依赖方使用的借用入口：登记时检查区间并锁定该 owner 的借用状态，保活 owner 和稳定的 native allocation；直到 native 不再使用地址、完成进度已提交后才解除。具体入口与访问性先在平台 spec 定稿，并在所属库实现与验证，不增加静态 ownership／borrow checker；该前置能力完成前不能用 asyncio 内部 workaround 继续接入。

借用期间拒绝 close、扩容、compact、clear、游标推进、修改字节及另一借用；这些检查与借用登记／释放使用同一同步边界，不能把现有只防重入的 Boolean 当跨线程锁。writable 借用的新字节在提交 writer 前不可通过 safe API 读取。所有同步变更入口也必须遵守这个活动异步借用状态。该保护只维护存储与借用生命周期，不把普通复合 buffer 操作自动变成线程安全事务。

结果选择与释放借用分开：取消可以先成为最终结果，但借用仍保留到 native 完成。即使最终抛出取消或关闭异常，也先提交已知的实际读写进度；`writeFully` 的前缀不会回滚，已写入 native 的字节也不会自动重发。新获得但未交付的文件／连接要先关闭，解析结果等独立存储要先释放，再结束等待。首版这些操作统一等待收尾完成，不使用提前报告取消而在后台遗留清理的优化。

buffer wrapper 可以由 GC 移动，native allocation 不变；C 只持有后者的地址。不能跨挂起保留 managed Array／String 的同步借用指针；路径与 DNS 参数先复制为请求拥有的 native 字节。分配、借用登记或提交失败时回滚尚未发布的资源，不留下永久 busy 的 buffer。

## 11. 各类操作的取消与关闭

### 11.1 统一的结果协调

遵守第 6.1 节的一次完成判定。操作具有结果选择和 native 资源结束两个独立事实：只有两者均满足时才提交 buffer 进度、解除借用并恢复等待者。native 完成先选定成功时，随后取消不覆盖成功；取消先选定时，稍后成功仍执行进度和资源收尾，但不再次恢复为成功。

LibuvEventLoop 的取消回调只登记取消请求并唤醒所属 loop，不从任意线程调用 libuv 或同步恢复用户代码。登记前已取消不提交；登记途中取消由提交／撤销的同一操作记录衔接。取消撤销函数的返回只描述撤销尝试，不证明 callback 不会到来。内部收尾错误不得伪装成正常取消或覆盖已有非取消失败，按第 5 节及资源清理异常合同处理。

该实现区分停止订阅与取消 request。`uv_read_stop`／`uv_udp_recv_stop` 停止本次订阅后不会另行提供一个取消完成 callback，由 shim 在确认不再使用本次 buffer 后生成完成记录；已提交的 request 则等待其实际 callback。两条路径都处理此前已入队事件，不能凭一个统一的 cancel 返回值提前释放存储。

### 11.2 LibuvEventLoop 的操作映射

| 操作 | 提交与正常完成 | 取消／关闭行为 |
| --- | --- | --- |
| 定时等待 | 根据单调截止时间安排 `uv_timer_t`，到期记录完成 | 取消停止 timer 并关闭其 handle，等关闭结束再恢复；已入队到期事件按一次结果判定处理 |
| DNS | `uv_queue_work` 执行原始 getaddrinfo；完成后在 managed 侧构造地址值 | 尚在队列可尝试 `uv_cancel`；已执行则等 after-work。释放 native 结果与输入副本后报告取消 |
| TCP connect | 本次请求独占新建 `uv_tcp_t`；成功后交付 owned stream | 关闭尚未交付的 handle，等待 connect 与 close 相关回调结束；迟到成功不得泄漏连接 |
| TCP accept | listener 同时一个活动等待；到达连接后接受并交付新的 owned stream | 只撤销本次等待，listener 保持可用。已分配给该操作但未交付的连接要关闭；无等待者时保留后端待 accept 状态，不主动分配 Scoop stream 或建立预接收队列 |
| TCP readInto | `uv_read_start` 借用可写区；取得正字节数或终态后停止本次订阅；nread=0 继续等待，UV_EOF 映射 EOF | 在 loop 线程 `uv_read_stop`，处理已记录进度后解除借用。单独取消 read 不关闭健康 stream |
| TCP writeFrom | 可先 `uv_try_write`，正进度直接形成短写；无法立即写入时提交单个 `uv_write_t` | 1.53.0 的 `uv_cancel` 可用于 write；必须等 write callback，在其中读取 `uv_write_nwritten` 保存实际前缀。纯取消完成后健康 TCP 仍可用，实际传输错误另行处理 |
| TCP shutdownOutput | 作为输出方向操作提交 `uv_shutdown`，之后禁止新写；成功仅关闭输出，输入仍可读 | 提交前取消不改状态；提交后不能靠 `uv_cancel` 撤销，取消时终止整个 stream 并收齐回调，不承诺恢复成可写 |
| UDP receiveFrom | `uv_udp_recv_start`，一个请求消费一个 datagram 后停止；保存源地址、长度与截断标志 | 停止本次接收并处理已消费的报文进度；单独取消等待保留 socket。有效地址对应的零字节是空包，无地址的零字节通知不是报文 |
| UDP sendTo | 可立即发送或提交单个 `uv_udp_send_t`，保持一个 datagram，不拆包 | `uv_cancel` 不支持该请求。提交后取消等待实际 send callback，再释放借用；不为单次发送取消关闭共享 socket。显式 socket close 则终止其所有在途操作；已发报文不撤回 |
| 文件 open／read／write／sync | `uv_queue_work` 调用平台 helper；一次实际读写允许短传输，sync 与 flush 区分 | 排队时可尝试取消 work；执行中必须等 after-work。读写提交实际进度，迟到 open 成功先关闭未交付 fd；close 不能与正在执行的文件 syscall 竞争回收 fd |

libuv 1.53.0 才提供本设计使用的 write 取消及进度接口；不能退回旧版本后仍宣称同样的写取消合同。所有 request 存储至少保留至对应完成 callback；收到真实读端错误后停止订阅并记录该方向的终态，后续调用不得在 libuv 已无效的读状态上重启。[stream API](https://docs.libuv.org/en/v1.x/stream.html)、[request 取消](https://docs.libuv.org/en/v1.x/request.html)、[UDP API](https://docs.libuv.org/en/v1.x/udp.html)。

文件使用共享文件位置的首版接口，同一 AsyncFile 的读／写／sync 串行，避免把两个 worker 对共享偏移的竞争隐藏成流顺序。close 是单独的终止操作，可与当前操作竞争，但只能先请求取消并等待，再提交实际 close 工作；close 工作本身不因当前 Job 已取消而跳过，也不对已消费 fd 重试 close。

定时等待复用现有时间值类型，负时长非法，零时长在取消检查后可立即完成。正时长按单调截止时间判断；更新 libuv 的缓存时间并处理毫秒精度转换，回调仍核对是否确已到期，不能因舍入或旧时间提前结束等待。UDP 每方向最多一个在途操作；发送允许零长度 datagram，接收要求正容量，过大报文报实际错误而不拆成多次发送。

### 11.3 显式 close 的完成条件

close 的语义是终止本地资源使用并完成释放，不隐含 flush、文件 sync、向对端交付全部积压字节或 TLS 关闭握手。需要保留输出的调用方先显式完成 write／flush，需要 TCP 半关闭则显式调用 shutdownOutput；协议级优雅结束由相应协议库定义。

1. 在所属 loop 把资源置为 closing，拒绝新操作；已经完成的结果不被追溯改写。尚未确定结果的活动操作以本库普通异常（工作名 `IoClosedException`）结束，若其 Job 取消已先获胜则保持取消结果；不为主动关闭伪造 OS 错误码。
2. 停止接收事件并终止适用的 native request。LibuvEventLoop 对 TCP／UDP／listener 使用 `uv_close`；文件先等待已提交的工作完全结束，再由工作线程执行一次 raw close。listener 关闭不关闭已经交付给调用方的 stream。
3. 等待该资源全部在途操作完成、借用解除和 native 关闭完成，才释放存储并完成 close。对 libuv 实现，这包括等待 handle close callback；`uv_close` 返回不等于 handle 内存可以释放，`uv_stop` 也不能替代资源清理。[handle 生命周期](https://docs.libuv.org/en/v1.x/handle.html)。
4. close 允许在已经取消的 Job 的 finally 中发起；一旦开始，其必要的本地收尾等待不再被该 Job 的取消中断。它仍可报告实际清理错误，不等待对端配合来完成本地资源释放。
5. 并发 close 等待同一次收尾，不重复释放；关闭后的再次 close 是幂等空操作，不重新执行或重报旧清理错误。其余资源操作报状态错误。调用方异常与清理异常按资源辅助函数合同组合；EventLoop 收尾也不能因某个资源失败而跳过剩余资源。

GC release hook 不承担以上任何生命周期责任，仅是防 native 资源泄漏的 best-effort 最终兜底，不保证执行、及时性或完整清理。不能依靠它取消任务、恢复 continuation、flush、驱动 loop 或等待 callback，也不能直接 free 仍被 libuv 使用的 handle／request。正常正确性由显式 close、useAsync 和 EventLoop 的收尾保证。

## 12. 支持未来 TLS stream 的接口约束

未来 TLS stream 包装 `AsyncByteStream`，对外实现同一字节流协议；上层按 plaintext 字节数观察进度，底层按 ciphertext 字节数推进，两者不要求一一对应。TLS 作为普通库组合，不要求编译器、core 或本轮 native loop 理解 TLS 状态。

- **传输可替换。** 通用流协议不要求取得 fd、注册特定 poller 或访问 libuv handle。TLS adapter 可用 native 内存 BIO／BIO pair 执行有限的同步协议步骤，再通过底层 suspend 读写推进；native callback 内不执行 suspend。
- **双向可推进。** 一次 TLS read 可能需要写出协议数据，一次 TLS write 也可能需要输入。adapter 负责序列化协议引擎状态、协调底层各一个读写，并在等待输入前处理必要输出；不能持有阻止另一方向进展的锁挂起。调用者仅执行 read 时，协议所需输出也应由 adapter 自行推进，无需另起应用写任务。[TLS I/O 方向](https://docs.openssl.org/3.5/man3/SSL_get_error/)、[BIO pair](https://docs.openssl.org/3.5/man3/BIO_s_bio/)。
- **允许内部缓冲与立即完成。** 已有明文可直接完成 read；已接受的明文或待发送密文由 adapter 持有，不能在向调用方返回后继续借用其 buffer。输出背压通过实际挂起表达，不把所有待写数据无限复制到内部队列。TLS 重试可能要求保留同一份写入数据，这由 adapter 自有存储或尚未解除的借用承担。[TLS 写入重试](https://docs.openssl.org/3.5/man3/SSL_write/)。
- **取消允许留下终态。** 通用接口只保证一次恢复、进度与存储有效，不保证任意包装层取消后均可复用。未来 TLS 必须按具体操作明确继续使用或终止连接；已改变协议状态时不能假装回滚，也不能在后台使用已归还的输入。
- **EOF 与关闭分层。** 底层 EOF 不等于 TLS 有序结束；TLS 可把缺少关闭通知的截断报告为协议错误。TLS 优雅关闭可能需要收发 close_notify；资源 close 仍有不依赖对端响应的终止路径。TCP shutdownInput／shutdownOutput 不成为所有字节流必须实现的能力。[TLS shutdown](https://docs.openssl.org/3.5/man3/SSL_shutdown/)。
- **包装期间独占通道。** wrapper 明确借用还是接管底层关闭责任；包装期间，调用方不绕过 wrapper 直接消费或写入底层字节。该规则用普通库所有权合同表达，不引入新的语言 ownership 类型。

上述 TLS 文档只用于检查接口可实现性，不意味着本轮选定 OpenSSL 或交付 TLS。当前验收覆盖真实异步流的双向进展、短传输、取消、借用和关闭；TLS 握手、证书、ALPN 及协议互操作留给后续里程碑。

## 13. 文档与实施任务

### 13.1 实施前定稿

- [x] 定稿 core 内结构化并发的 package 与公开声明，包括 Job、Deferred、CoroutineScope、构建器及取消异常的签名；实现放在 `sysroot/lib/scoop.core/` 的普通 Scoop 源码中。
- [x] 定稿多失败时的主次呈现、可取消等待的公开入口及同步登记异常规则；保留第 4～6 节的结构化完成与单次恢复要求。
- [x] 在语言规范的 core 库合同中记录任务树、正常／失败／取消退出、`join`／`await` 和 Context 绑定；同步 8.2、8.3、11.9，修正构建器不属于 core 的旧表述，区分既有协程原语与新增的普通库能力。取消不变成所有 suspend 调用的隐式语言行为。
- [x] 核对实现规范的协程降级、2.16 与运行时规范第 8、9 章的引用，确认普通库对象可复用现有 continuation、异常、Context 与 GC 合同；如有底层缺口，按第 2 节先修复所属层并完成独立回归，再继续上层实现。
- [x] 补充 M49 异步 I/O 设计：明确 EventLoop 接口与 LibuvEventLoop 实现、根协程启动、操作取消、显式关闭与 TLS stream 接口约束；仅完成设计，不代表实现或验收完成。
- [x] 明确结构化并发归入 core、异步 I/O 为普通 Cone、以 Scoop／Cone 实现及先修底层的边界；区分 core 的 fixture 验证与 asyncio 的 `scoop.test` 主要验收入口。
- [ ] 将第 8～12 节写入独立异步 I/O 库规范；定稿 EventLoop.run、普通泛型扩展 runWithResult、LibuvEventLoop、流与资源接口、I/O 关闭异常和各操作声明，明确接口合同与后端实现边界。
- [ ] 先定稿平台规范中的异步 buffer 借用与 Libuv 错误域，列为 platform 库的前置能力；同步 core／std／platform 的相关引用。同步接口原有签名与行为保持不变，实际共享存储规则的扩展显式记录。
- [ ] 锁定 libuv 1.53.0 的三目标 native 构建与链接输入，确认 C helper 复用、SIGPIPE 与已有 FFI／GC 边界；不要求先引入 TLS 依赖。
- [ ] 确定 asyncio 的同 Cone `scoop.test` 用例与测试依赖，覆盖第 14.2 节；只有明确说明 `scoop.test` 无法满足的具体需求，才安排专项 fixture。

### 13.2 实施顺序

1. **任务树闭环。** 在 core 中用普通 Scoop 与现有启动／挂起原语实现作用域、父子登记、完成等待、launch 与带结果的 async，覆盖同步结束和真实挂起，可使用 fixture 验证。
2. **失败与取消。** 加入向下取消、失败传播、当前 Job 绑定和收尾等待，验证子任务清理完成后父作用域才退出。
3. **可取消挂起适配。** 完成登记、完成、取消及迟到事件之间的协调，先用可显式控制完成次序的操作验证，再接真实 I/O。
4. **平台库前置能力。** 在 platform Cone 完成异步 buffer 借用、错误域及实际复用所需的正常接口，按所属库的测试方式验证后，再由 asyncio 消费；不在 asyncio 内补写私有访问或复制状态作为替代。
5. **驱动与借用接入。** 在普通 asyncio Cone 定义 EventLoop 接口并提供 LibuvEventLoop，实现根协程启动、跨线程唤醒、C 完成队列和 managed 投递，接入已验证的 buffer 借用；使用 `scoop.test` 经接口调用验证定时等待、取消与退出闭环。
6. **网络字节流。** 接入 TCP connect／accept／双向传输与 shutdownOutput，完成显式 close、作用域收尾和 SIGPIPE 保护；实现基本流组合扩展，并通过同 Cone 测试验证。
7. **文件、DNS 与 UDP。** 接入工作线程上的平台 helper 和 datagram 操作，通过同 Cone 测试覆盖不可撤销的执行中请求、迟到资源与不同取消策略。
8. **组合与产物验收。** 以 `scoop.test` 为主验证三个目标的真实 I/O、跨 Cone 消费、GC／FFI 和关闭竞争；专项 fixture 必须符合第 14.2 节的使用条件。完成验收记录，TLS 实现不作为本轮门槛。

以上前三步是 M49 的 core 基础交付，完成它们不等于整个异步 I/O 里程碑完成。任一步发现底层缺口都先按第 2 节修复并验证，再恢复受影响的上层工作。每批实现先更新对应规范，格式化与 lint 后再运行适用测试。

## 14. 验收重点

### 14.1 结构化并发

结构化并发属于 core，可以使用 fixture 验证库行为、协程组合与跨 Cone 消费；沿用已有 core 验收方式。

| 类别 | 必须覆盖的行为 |
| --- | --- |
| 任务树 | 多层父子任务、正文先结束而子任务仍挂起、子任务同步完成／失败、启动与关闭或取消竞争；已关闭 scope 不产生新任务 |
| 等待与结果 | 多个 join／await 等待者、完成后再次等待、带值与引用的结果、失败结果、等待者取消不直接取消目标 |
| 失败传播 | 正文失败、未 await 的 async 失败、同级任务取消与 finally 完成；非取消失败不被后续取消覆盖 |
| 协作取消 | 执行中显式检查、挂起中恢复取消异常、取消重复请求、子任务单独取消、原始不可取消挂起点的边界 |
| 单次完成 | 同步完成、真实挂起、完成与取消两种获胜顺序、登记中取消、迟到回调；底层 continuation 仅提交一次 |
| Context | 子任务当前 Job 正确、其他 binding 继承与重绑定隔离、跨线程恢复、回调 context 不替代被恢复任务的 context |
| 生命周期与 GC | 挂起期间保活、取消和异常路径的 finally、完成后移除登记、迟到 I/O 事件前资源仍有效 |
| core API 消费 | core 声明的正常解析、泛型结果与跨 Cone 调用、真实构建与运行；源码和工具保持存在 |

竞争测试使用明确的完成控制或同步，不依赖任意 sleep；跨线程测试程序可依赖已有平台库，不使 core 的生产代码反向依赖 platform。编译错误、协程降级、GC／FFI 与产物边界只增加实际需要的专项覆盖，不重复全阶段快照。

### 14.2 异步 I/O

`scoop.asyncio` 是普通 Cone，以 `scoop.test` 为主要验证方式。测试计划放在 `sysroot/lib/scoop.asyncio/tests/`，通过 `[test-dependencies]` 引用 `scoop:scoop.test:0.1.0`，使用 `scoop test sysroot/lib/scoop.asyncio` 运行；测试源码与测试依赖不进入生产构建。普通测试函数以 EventLoop 接口类型持有 LibuvEventLoop，通过 run／runWithResult 驱动 suspend 场景，沿用既有测试发现与断言能力。

只有 `scoop.test` 无法满足某一具体验证需求时才使用最小的专项 fixture，并说明所缺能力和该 fixture 验证的边界；涉及 native、GC／FFI、竞争或跨 Cone 本身不构成改用 fixture 的理由。若原因是 `scoop.test` 或测试基础设施的缺陷，须先在所属层修复并建立回归，再继续 I/O 验证，不能以 fixture 绕过该缺陷。

| 类别 | 必须覆盖的行为 |
| --- | --- |
| 接口与根入口 | 经 EventLoop 接口调用 LibuvEventLoop；首次正文已具有正确的 EventLoop／Job 绑定；同步完成／失败、真实挂起、初始化失败收尾；runWithResult 保留值与引用结果，子任务失败不提前交付结果；I/O 入口与返回资源沿普通接口派发 |
| 事件循环 | 多操作共同推进、阻塞等待的跨线程唤醒、合并通知不丢命令、拒绝已有 Job 中阻塞运行与嵌套驱动、拒绝同一实例并发或重复 run、关闭与最后一次投递竞争 |
| TCP | 真实 loopback connect／accept、短读写、EOF 后仍可写、一读一写并发、同方向并发拒绝、输出半关闭及未交付连接清理 |
| 取消与进度 | 完成／取消两种获胜顺序、read_stop、已排队 write 的部分进度与 callback、已提交 shutdown 的终止路径；取消后不重复消费／重发 |
| UDP | 空包与无数据通知区分、截断与来源、整包发送、发送取消等待 callback、关闭同时收发的收尾 |
| 工作请求 | 文件真实读写与 sync、排队／执行中取消、迟到 open 成功后的关闭、DNS 原始字节与 EAI 错误；执行结束前不释放输入或复用 fd |
| buffer／GC | 借用期间拒绝释放、扩容和游标变更；完成前保活、moving GC、取消后实际进度、提交失败撤销借用；跨线程借用登记竞争 |
| 资源关闭 | 已取消 Job 的 finally 仍完成 close、并发 close 只收尾一次、关闭唤醒挂起操作、close callback 前不释放 C 存储、EventLoop.run 等待任务与操作收尾后才退出、逃逸资源已关闭 |
| 流组合 | readFully／writeFully／copyTo 对短传输和失败前缀的处理、内部缓冲允许立即完成、flush 的向下传播及一读一写独立进展 |
| 平台与产物 | 三目标真实构建与运行、普通 native 依赖链接、SIGPIPE handler／mask／原有 pending 保留、native 原始错误与库错误域不混淆 |

网络使用 loopback 和动态端口，DNS 不依赖公共外部服务。取消竞争由明确的同步点与必要的 native 测试控制安排，同时保留真实 OS 传输；不通过任意 sleep 或只手工恢复 continuation 证明完成。每项能力有独立覆盖，并有并发文件／网络、取消／关闭与 GC 的组合覆盖，不增加仅供测试使用的生产后端或插件框架。

实施验收记录见 PROGRESS.md，最终验收文档只记录范围、结果概要、失败闭合与必要限制，原始日志和逐次报告放在仓库外。
