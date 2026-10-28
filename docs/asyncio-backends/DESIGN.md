# EventLoop 平台后端与 GUI 主循环接入

状态：Darwin、Linux GNU／musl 后端、宿主接入和消费者迁移已交付；Windows 扩展实施中。日期：2026-10-28。原有验证范围见[实施记录](PROGRESS.md)，性能与回退见[性能记录](PERFORMANCE.md)；Windows 的分批验收见[Windows 进度](../windows-targets/PROGRESS.md)。

本设计对应[异步 I/O 库规范第 11 节](../specs/SCOOP-ASYNCIO-SPEC.md#11-平台后端与宿主主循环迁移合同)。沿用[语言规范](../specs/SCOOP-SPEC.md) 11.9～11.9.2 的协程与任务树、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)第 4 章的 native／callback 边界，以及[平台库规范](../specs/SCOOP-PLATFORM-SPEC.md)的错误、buffer 与 TTY 所有权。编译器、runtime ABI 和 `.slib` 格式不变。

## 1. 决策与范围

公共 `EventLoop` 继续表示异步 I/O 服务。提供 `PlatformEventLoop` 作为默认实现，macOS 使用系统 libdispatch，Linux 使用 io_uring 与 liburing。任务树、取消结果选择、buffer 借用、资源关闭和 continuation 恢复共用一份 Scoop 实现；平台后端只负责原生操作及其真实完成。

同一个实现支持两种驱动方式：命令行程序用现有同步 `run`；GUI 用 `attach` 把非阻塞处理接入宿主主循环。AppKit／其他 GUI 主循环继续负责窗口、输入、菜单和显示刷新，EventLoop 不接管这些职责。

首版覆盖现有全部 asyncio 能力：定时器、TCP、UDP、DNS、文件、目录、路径操作、TTY 和 `NativeWork`。TLS、HTTP、地址竞争、SOCKS5 等继续组合公共 API。文件监听、子进程、任意 fd 接管、通用任务执行器、协程拦截器、渲染线程与帧调度不在本次范围。

平台由构建目标决定，不增加运行时后端注册表或公共后端枚举。迁移时通过内部 libuv 适配验证共享逻辑；现已移除 `LibuvEventLoop` 与 libuv 依赖，并更新仓库内调用方。

## 2. 迁移前状态与拆分位置

以下保留迁移前的耦合与对应拆分，当前实现不再依赖 `uv_run`：

| 迁移前位置 | 迁移前职责／耦合 | 实现方式 |
| --- | --- | --- |
| `src/libuv_event_loop.scoop` | 根入口、Job／Context 检查、具体资源构造 | `PlatformEventLoop`，持有共享 owner |
| `src/loop_state.scoop` | 命令、完成、ready 队列及运行收尾 | 重整为 `LoopOwner`，支持阻塞驱动与宿主驱动 |
| `src/loop_operation.scoop` | 选择成功／取消／关闭结果，等待原生完成 | 保留同一算法，消除后端类型依赖 |
| `src/owned_endpoint.scoop`、`owned_file.scoop`、`owned_directory.scoop` | 具体持有 `LibuvEventLoop` | 只依赖共享 owner，保留各资源的并发规则 |
| `src/native_loop.scoop`、`native/loop.h` | 完成记录、uv 错误与 uv handle | 平台无关记录与不透明 native handle |
| `native/work.c`、`native/resolve.c`、文件／TTY helper | libuv worker／poll，部分已复用 platform | 复用 GC-free 平台 helper，由各后端提交 |

表内路径均相对 [scoop.asyncio](../../sysroot/src/scoop.asyncio/)。当前 native 代码按 `common/`、`darwin/`、`linux/` 分目录，以真实职责拆文件。

分层边界如下：

| 层 | 持有的状态 | 不承担的职责 |
| --- | --- | --- |
| core `Job`／`CoroutineScope` | 任务父子关系、取消传播、join、结果 | I/O、线程选择 |
| asyncio 共享层 | 根任务、操作表、资源表、队列、借用、结果选择 | dispatch source／SQE 的内部状态 |
| native 后端 | fd、请求、稳定内存、平台等待／取消、完成记录 | managed 对象、Job、用户闭包 |
| 宿主适配 | 唤醒 fd 监听、owner 线程入口、监听注销 | I/O 语义、资源关闭策略、另一套任务树 |

## 3. 公共入口与宿主协议

保留 `EventLoop` 全部现有方法、`currentEventLoop` 和 `runWithResult`。新增入口的公共声明如下：

```scoop
public class PlatformEventLoop public constructor() : EventLoop {
    public fun attach(host: EventLoopHost,
        onClosed: (Throwable?) -> Unit): EventLoopSession
}

public class EventLoopSession internal constructor(owner: LoopOwner) {
    public val loop: EventLoop
    public val isClosed: Boolean
    public fun post(block: (CoroutineScope) -> Unit): Boolean
    public fun requestClose()
}

public enum EventLoopNotification {
    ReadableDescriptor(Int), WindowsEvent(Ptr<Unit>)
}
public interface EventLoopHost {
    public fun isCurrentThread(): Boolean
    @Unsafe public fun watch(notification: EventLoopNotification,
        ready: () -> Unit): EventLoopWatch
}

public interface EventLoopWatch {
    public fun remove(onRemoved: () -> Unit)
}

public class MainDispatchHost public constructor() : EventLoopHost
```

`PlatformEventLoop`／`EventLoopSession` 属于 `scoop.asyncio`；宿主接口与仅在 Darwin 提供的 `MainDispatchHost` 放在 `scoop.asyncio.host`。`LoopOwner` 是库内共享状态，应用不能直接创建 session。普通 I/O 调用方不接触 fd、dispatch queue、ring 或 `pump`。

### 3.1 单次生命周期

`run` 与 `attach` 竞争同一个单次启动状态：只能接受其中一个，不能并发启动、重入或关闭后重用。两者调用时均不得已有当前 Job 或活动 EventLoop binding。构造不取得 native 资源，接受启动后才初始化；失败回滚已取得资源，实例进入关闭状态。

`run` 的调用线程成为 owner，保留既有同步返回／抛异常与结构化收尾合同。`attach` 要求 `host.isCurrentThread()` 为真，该线程成为 owner；先建立后端与内部根，再安装监听并投递启动通知，成功后立即返回 session。内部根在 attach 栈内建立 Context 并立即挂起，尚不执行应用投递或提交 I/O；用户代码从后续宿主回调开始。监听安装不得同步调用 `ready`；安装失败时内部根没有子任务或在途 I/O，可以取消并清理初始化资源。

`post`、`requestClose` 与 `isClosed` 可跨线程使用，发布与读取具有必要同步。其余驱动／宿主操作只在 owner 上执行。资源仍要求当前 Job 和所属 EventLoop binding；没有 binding 的 GUI 事件通过 `session.post` 进入，而不是直接调用挂起 I/O。

### 3.2 GUI 根任务与投递

`attach` 内部建立精确静态类型为 `EventLoop` 的 Context binding，用现有 `startCoroutine` 与 `coroutineScope` 建立一个根任务。根正文消费 GUI 投递的 mailbox，没有消息时挂起，直到请求关闭或任务树失败；不能在启动回调返回时结束这个根作用域。每条 `post` 回调得到同一个仍活动的 `CoroutineScope`，可用 `scope.launch`／`async` 启动有父任务的挂起工作。

`post` 总是异步排队，按入队顺序执行；跨线程之间以入队锁的顺序为准。返回 `false` 表示已停止接受应用投递，且不保留闭包；`true` 表示接受入队，不保证在之后的关闭／失败中仍会执行。关闭开始时丢弃尚未执行的应用投递，但必须继续消费完成、取消和清理命令。普通回调抛错按根正文失败处理，取消子任务并收尾。

mailbox 的执行通过根协程自己的 continuation 恢复完整 Context；不能仅临时绑定 Job 和 EventLoop，也不能假定保存一个闭包就保存了调用方的动态 Context。根继承 `attach` 时的 Context，外线程调用 `post` 不把自己的 Context 偷换进去。

`requestClose` 幂等且不阻塞：原子地停止接受应用投递，入队根取消命令并唤醒 owner，由 owner 调用根 Job 的 cancel。mailbox 等待的取消 handler 也只把恢复放入 owner 的 ready 队列，不在任意取消线程直接恢复根。即使关闭发生在首次宿主回调之前，也必须走完关闭流程且不执行已排队的应用回调。主动关闭引起的取消不作为 `onClosed` 的失败；独立的非取消异常及清理失败照常保留。

宿主必须继续运行至 `onClosed`。该回调恰好一次，在 owner 上、I/O 清理和监听注销完成后执行，此时 `isClosed` 为真；参数是运行／清理结果，组合规则沿既有 `ResourceUseException`。回调本身抛错不重新打开 loop、不再次通知；native 桥接用现有 `ForeignCallback` 捕获异常，适配器负责取出并报告，内置适配器写 stderr，不让异常跨 C 栈传播。

### 3.3 监听的具体合同

`watch` 只借用 `EventLoopNotification` 的资源：POSIX 可读 fd 或 Windows manual-reset event；不读、写、重置、关闭或更改其模式，loop 自己消费通知。HANDLE 保留指针宽度，不映射为 fd。`ready` 必须在 owner 上串行执行，可有合并或多余通知；不得在注册栈内同步执行。安装失败抛出后，host 不得再持有回调或活动监听。

`remove` 在 owner 调用，按宿主协议不得抛错，异步注销监听；`onRemoved` 恰好一次，并且此后不会再进入 `ready`。如果在 `ready` 内开始注销，必须先退出当前 `ready`，再通知移除完成。注销负责退休已经排队的通知，不能只从 poll 集合删掉 fd 就允许旧闭包继续运行。loop 在 `onRemoved` 前保留 fd 和相关上下文。

该协议只要求宿主提供对应原生通知资源的监听和注销完成，不要求 timer、worker pool 或通用 `dispatch` 接口。后端用自己的统一通知资源把命令、完成和计时到期接入这个入口。

Darwin 的内置 `MainDispatchHost` 用主 dispatch queue 上的 READ source 观察通知 pipe，沿已有 FFI callback 桥进入共享处理；cancel handler 负责移除完成。它适用于正常服务主 dispatch queue 的 AppKit／CFRunLoop 宿主，不调用永不返回的 `dispatch_main()`。自定义 run-loop mode、模态和嵌套循环的服务时机由宿主决定；需要其他 mode 行为的 GUI 层实现同一 host 协议，不能宣称任意 mode 都自动获得主队列服务。Apple 对主队列与主循环的关系见 [queue.h](https://github.com/apple-oss-distributions/libdispatch/blob/main/dispatch/queue.h)。

Linux 的 GUI 层把通知 eventfd 加入自己已有的 fd watch。Wayland 连接继续由 GUI 层遵守 libwayland 的 prepare／read／cancel 协议；X11、窗口事件和渲染也不转给 asyncio。参见 [Wayland 外部事件循环接口](https://wayland.freedesktop.org/docs/html/apb.html)。

## 4. 共享驱动与线程模型

`LoopOwner` 是命令和完成的单一消费者。I/O 提交、外线程取消和 GUI `post` 只发布命令；取消结果的竞争选择及投递入口关闭在短锁内完成。任何锁都不包围用户代码、continuation 恢复、阻塞等待或宿主注销。

一次内部 `pump` 非阻塞处理以下内容：消费唤醒提示；取得这一轮命令及已有 native 完成的有限快照；提交操作、解码完成；把可恢复项放入 ready 队列；执行本轮 ready；推进收尾。有新工作在处理期间到达，留到下一轮并保留／重新发送唤醒，不通过“持续直到所有队列为空”霸占 GUI 线程。每轮至少推进已排队的取消和关闭；不建立逻辑成本计费或跨阶段资源预算。

处理过程中 GUI 可能进入嵌套主循环；`pumping` 标志阻止递归进入状态机，嵌套通知只记录需再次处理，外层退出时重新唤醒。普通用户回调仍可能运行很久，本库不抢占它，也不把 CPU 计算自动搬到后台。

`run` 反复调用同一个 `pump`，无可执行工作时在统一通知 fd 上阻塞等待；等待使用 `NativeSafe`，不能标为 `GCLeaf`。`attach` 由 host 可读回调调用 `pump`，内部不等待 I/O、worker 或监听注销。正时长 timer 的到期由后端通知，宿主不需要周期轮询；空闲时没有常驻 idle 回调。

本库 I/O、mailbox 与本库排队恢复从 owner 线程进入 managed 代码。`scope.launch` 仍遵守现有“先内联执行到首次挂起”的合同；任意第三方挂起点仍可能从别的线程恢复。此设计不加入 continuation interceptor，因此**接入 GUI 主循环不等于所有挂起函数自动具备 UI 线程亲和性**。GUI 调用方从外部回调或其他恢复线程回到界面时，显式经 `session.post`；帧请求与 vsync 由 GUI 层安排。

## 5. 一份操作与资源生命周期逻辑

### 5.1 操作有两个独立事实

共享 `LoopOperation` 分别跟踪“已选定的用户结果”和“native 是否结束”。Job 取消、资源关闭、native 完成竞争前者；后者只能由后端完成记录确认。所有路径只选择一次结果、只恢复一次 continuation。

| 时序 | 对外结果 | 仍必须完成的工作 |
| --- | --- | --- |
| 登记时 Job 已取消 | 取消 | 不提交 native，不建立长期借用 |
| 原生完成先被共享层接受 | 成功或该 I/O 错误 | 提交进度、解除借用、撤销取消订阅 |
| Job 取消先选定 | 取消 | 请求撤销，等真实结束，提交已知实际进度 |
| close 先选定未决结果 | `IoClosedException` | 同上，不覆盖更早的取消／完成 |
| 取消后迟到 open／accept／connect 成功 | 保留取消 | 先关闭未交付资源，才报告完成 |

“先”以共享状态中的选择顺序为准，不以墙上时钟或 callback 创建时间推断。native cancel 的返回值、dispatch source cancel 请求、取消 SQE 的 CQE 都不能单独证明数据操作已经结束。

原生操作尚持有 `NativeByteBorrow` 时，不用会立即交付 Job 取消的通用可取消挂起包装它。沿现有 `LoopOperation` 的延迟完成方式，等真实借用结束后才恢复；部分读写进度即使最终抛取消／关闭也照常提交。原生只保存稳定 allocation、native request 和数值 id，managed 表保活 buffer owner、continuation 与资源。

### 5.2 资源与 loop 收尾

保留 `Open → Closing → Closed`：关闭入口拒绝新操作，取消尚未选定结果的操作；在途操作、buffer 借用与原生注销全部结束后才消费 fd／owner 并完成 close 等待者。并发 close 共享一次收尾；再次 close 不重复关闭或重抛旧错误。单独取消 TCP read／write、UDP receive、TTY receive 不关闭健康资源；`shutdownOutput` 提交后取消仍终止整个 stream。

loop 生命周期为 `Created → Running → Stopping → Draining → Detaching → Closed`；正常根完成可直接进入 Draining，同步运行跳过 host 注销。Stopping 停止应用投递并取消根；根及子任务执行各自 finally／close。根完成后 Draining 关闭所有遗留资源，即使某项失败也继续，并排空清理命令、完成和 ready。这里复用 core 的结构化 join，不另建 detached job 或公共 root-job 构造入口。

只有 managed 操作／资源／ready 已空、native 请求及控制请求已退休、worker 和 I/O callback 不再生产完成时，才进入 Detaching。先注销 host watch；确认 `onRemoved` 后再销毁后端、关闭通知 fd 并发布 `onClosed`。`run` 以同样的条件销毁并返回／抛错。应用停止入口与 native 通知 fd 的寿命在同一发布边界协调，避免外线程向已关闭甚至已复用的 fd 写通知。

当前执行的 host 移除通知持有自己的 callback lease，返回后释放；它不能阻塞等待自己退出，也不能在返回前释放仍使用的 C callback context。I/O 后端的退出条件与这个最后的宿主通知分开计数。

显式关闭是唯一正常生命周期。GC `release` 不启动 pump、调用宿主、恢复协程、等待 worker 或补做异步关闭。worker 已进入不可中断的 syscall 时，关闭耗时由其返回决定，不能为了 GUI 快速退出而提前释放它使用的内存。

## 6. 后端边界与唤醒

内部 native 接口保留按实际操作划分的 submit／cancel／close 函数，增加共用的初始化、通知 fd 获取、非阻塞完成批次获取与销毁入口。C 头文件使用不透明 loop／resource handle 与普通数据记录，不出现 uv、dispatch 或 io_uring 类型；Scoop 侧无需为每个平台复制资源类。

完成记录继续携带操作 id、结果类别、实际进度、错误 domain／code 以及必要的 native payload。沿现有事件编号分段扩展；不属于 runtime ABI。每个已接受操作最终产生一条逻辑完成；取消控制请求、两个 TTY poll 及其 timer 等内部请求由后端聚合并退休，不分别恢复同一 managed 操作。

通知是“可能有工作”，不是完成计数。Darwin 使用非阻塞 close-on-exec pipe；Linux 使用同属性 eventfd，并把它注册为 ring 的完成通知，同时用于命令／worker 完成唤醒。队列先发布、再通知；写满意味着已有未消费通知，重试 EINTR，其他 I/O 错误进入运行失败收尾。读端清空通知后检查真实队列／CQ；若本轮留下工作则重新唤醒，关闭检查也依据队列与活动请求，不依据 fd 是否可读。[liburing 明确允许通知合并及多余通知](https://man7.org/linux/man-pages/man3/io_uring_register_eventfd.3.html)。

错误统一保留实际来源：POSIX syscall 立即保存 errno，io_uring 的负结果取相反数形成 `PosixErrno`；DNS 保留 `AddressResolution` 与 `EAI_SYSTEM` 的 errno。正常 timer 到期的 `-ETIME` 解码为到期，不报 I/O 失败；Job 取消和 `IoClosedException` 仍由共享层决定。最终后端不产生 `.Libuv` 错误；清理平台库中该枚举项须与全部实际消费者迁移一起进行，不伪造 UV 错误码来维持内部测试。

## 7. macOS：libdispatch 后端

每个 loop 有一条串行 I/O dispatch queue，负责 endpoint、source 与请求的原生状态。串行 queue 不等于固定线程；其 event／cancel handler 都只执行 C、发布完成，不运行 Scoop 用户代码。owner 的 submit／cancel 通过 `dispatch_async_f` 等 C 入口入队，不用 `dispatch_sync` 等待 owner 与 I/O queue 互相返回。

TCP／UDP／TTY 使用 nonblocking fd、READ／WRITE source 与实际 syscall：先尝试操作，遇到 EAGAIN 再等待就绪；只在有等待操作时启用相应兴趣，避免可写 socket 造成空转。连接就绪后检查 `SO_ERROR`；listen／accept、地址复用与 SIGPIPE 处理沿 platform 的既有 POSIX helper。source suspend／resume 必须平衡，取消 suspended source 时也须使 cancel handler 能到达。

timer 用单调时钟 dispatch timer；到期仍核对截止时间，不能提前完成。取消 timer 或 fd 等待时，等待正在执行的 handler 与所需 cancel handler 结束才发布终止。一个 fd 被多个 source 引用时，资源关闭等所有 source 都退出再关闭 fd，不能把 fd 释放交给第一个 cancel handler。[Apple source 合同](https://github.com/apple-oss-distributions/libdispatch/blob/main/dispatch/source.h)要求这种顺序。

普通文件、目录、路径、DNS 和 `NativeWork` 复用现有 GC-free C helper，投递到后台 dispatch queue。文件操作保持每文件串行，不能在串行 I/O queue 或 GUI 主队列上执行可能阻塞的文件访问。尚未执行的 work 可以观察取消而跳过；已经执行的 work 必须完成并发布结果。dispatch 取消不是强制终止 `getaddrinfo` 或文件 syscall。

使用系统 libdispatch 的 `_f` API，不要求 Scoop 支持 Objective-C、Blocks 或 `.m`／`.mm` native 编译；也不引入 `dispatch_io` 的 buffer／channel 所有权模型来复制当前资源合同。主循环进入 managed 的入口只在 host 适配层使用既有 `ForeignCallback`。

## 8. Linux：io_uring 后端

### 8.1 依赖与最低能力

使用固定源码版本 liburing 2.15，随 Cone 的 native 输入与许可证一起发布；该版本已有 [2026-06-29 上游发布记录](https://github.com/axboe/liburing/releases/tag/liburing-2.15)。GNU／musl 分别按现有目标工具链构建，不依赖系统 liburing，也不手写 ring 的内存屏障和 SQ／CQ ABI。

首版支持与验证基线选择 **Linux 6.1 或更新内核**；这是 asyncio 新后端的运行要求，不改变其他 Scoop 程序的最低内核要求。初始化实际建立 ring、probe 所需操作并检查必要 feature，不仅判断版本字符串；所需操作为 ACCEPT、CONNECT、RECV、SEND、RECVMSG、SENDMSG、READ、WRITE、FSYNC、TIMEOUT、ASYNC_CANCEL、POLL_ADD，要求 `IORING_FEAT_NODROP`。操作存在也不保证任意 fd 类型／文件系统支持，提交错误仍按操作报告。探测依据见 [liburing probe](https://man7.org/linux/man-pages/man3/io_uring_get_probe.3.html)。

ring 被系统策略禁用、初始化失败或缺少所需能力时，启动报告明确的平台错误并回滚；不静默退回 libuv、epoll 或线程模拟整个后端。部署验证覆盖实际容器／运行用户的 io_uring 可用性；如果以后必须支持旧内核，再以具体部署需求单独设计后备。

### 8.2 提交与完成

每个 loop 一个 ring，仅 owner 线程提交 SQE、消费 CQE。首版使用普通 interrupt-driven ring 和单次请求，不开启 SQPOLL／IOPOLL、multishot、注册 buffer 或零拷贝发送，避免引入尚无收益依据的额外借用周期。

`user_data` 指向稳定 C request，内含共享操作 id 及内部请求类别；请求对象只在其所有 CQE 退休后释放。待提交操作由后端队列持有；SQ 满时保留并稍后提交，不丢请求，不把队列暂满作为业务 I/O 失败。部分提交按实际已提交数推进；取消／关闭请求应能随 SQ 空间释放继续提交，不能被新业务提交一直挤在后面。

每轮先消费已有 CQE，再推进待提交项；CQ 满时沿 NODROP／overflow 机制继续取回完成，不默默丢记录或只等下一次 eventfd 边沿。若 SQ/CQ 暂时需要重试，安排后续处理；无工作时不忙等。通知 eventfd 使用普通注册而非仅异步完成通知，队列本身始终是完成来源。

READ／WRITE 的文件 offset 使用 `-1`，保留真实 fd 位置，并维持现有每文件操作串行规则；不新建一套逻辑 offset 或绕过 append 语义。单次长度取平台可表达的合法前缀，短读写与取消的实际进度原样交给共享层。[liburing read 文档](https://man7.org/linux/man-pages/man3/io_uring_prep_read.3.html)说明了共享 fd offset 的同步要求。

普通文件的数据 I/O 与 fsync 首版设置 `IOSQE_ASYNC`，交由内核异步执行路径，减少 GUI owner 上直接尝试文件访问的延迟；网络等待不一律使用该 flag。这个选择有 worker 调度开销，需在验收中测量，不宣称提交 syscall 完全没有延迟。[liburing 对 IOSQE_ASYNC 的说明](https://man7.org/linux/man-pages/man3/io_uring_sqe_set_flags.3.html)给出了其执行方式与成本取舍。

### 8.3 取消和复合等待

按目标 request 的 `user_data` 提交 ASYNC_CANCEL，取消请求拥有独立 request。目标数据 CQE 与取消 CQE 都必须消费，顺序不限；取消返回 `-ENOENT`／`-EALREADY` 是正常竞争，不能代替目标结果。若目标仍在用户态待提交队列、尚未发布至 SQ，则移除并直接完成本地收尾；已发布的 SQE 继续跟踪其提交与取消，不能当作未提交请求直接释放。后端在所有相关请求退休后只发布一次逻辑完成，避免请求地址或 id 过早复用。参考 [liburing cancel 合同](https://man7.org/linux/man-pages/man3/io_uring_prep_cancel.3.html)。

TTY 用单次 POLL_ADD 观察自有终端 fd 和 platform resize fd，read／write 使用 nonblocking syscall；timeout 用独立 TIMEOUT。任一结果选定后取消其余等待并消费它们的 CQE，全部退出才释放 buffer／owner；迟到 resize 仍为下一次 receive 保留。`None` 无限等待，零 timeout 只做当下查询，不创建无限 poll。现有 SIGWINCH self-pipe 仍归 platform，不安装第二个信号处理器。

DNS、目录遍历、路径操作、open、seek／truncate 和 `NativeWork` 使用按需启动的有限 native worker 集合。它只接受本库已有的 GC-free 工作与完成记录，不提供公共线程池 API；队列可撤销，运行中的 syscall 等真实返回。常规文件数据 read／write／fsync 和网络 I/O 实际走 ring，不能把新后端实现成全部交给 worker 的包装。

### 8.4 提交失败与 ring 销毁

操作 CQE 错误只结束对应操作。提交失败须区分未发布、已发布与内核已接受的请求；只有明确尚未提交的请求可以本地结束。EINTR 等可重试情况保留原请求，不重复发布 SQE。运行层失败停止新业务、记录错误并进入取消／排空流程，仍按目标 CQE 和 worker 完成释放借用，不能用一次 submit 错误假定所有在途操作都停止。

`io_uring_queue_exit` 仅在无在途请求时调用，不能当作同步“取消全部并立即释放 buffer”的入口：它解除映射并关闭 ring fd，内核部分退出工作异步执行。参见 [queue_exit 合同](https://man7.org/linux/man-pages/man3/io_uring_queue_exit.3.html)与 [Linux 6.1 退出实现](https://github.com/torvalds/linux/blob/v6.1/io_uring/io_uring.c)。无法确认原生结束时继续保留借用，不发布关闭完成；本设计不承诺内核／运行环境失效后的有界恢复，也不另建灾难恢复线程来绕过这个边界。

## 9. 现有能力到后端的完整映射

| 能力 | 共享语义 | Darwin | Linux |
| --- | --- | --- | --- |
| `run`／`attach`、任务投递 | 同一 root／owner／pump | pipe 通知 | eventfd 通知 |
| `sleep`、连接总期限 | 单调截止时间、取消后等退休 | dispatch timer | TIMEOUT |
| TCP connect／accept／read／write | 地址、方向独占、短读写、部分进度 | socket syscall + source | CONNECT／ACCEPT／RECV／SEND |
| listen、bind、地址查询、shutdown | 既有 platform 规则；shutdown 提交后取消终止流 | owner 的原生控制队列 | owner 的短 POSIX 控制操作 |
| UDP send／receive | 完整数据报，含零长度、截断和地址 | sendmsg／recvmsg + source | SENDMSG／RECVMSG |
| 文件 read／write／sync | 每文件串行、真实位置、append、借用 | 后台 work | READ／WRITE／FSYNC |
| open／metadata／移动／删除／建目录／临时文件 | 路径、错误、迟到资源关闭 | 后台 work | native worker |
| seek／truncate／目录 open／next／close | 现有平台表示与枚举寿命 | 后台 work | native worker |
| DNS | `getaddrinfo`、原始 OsString、EAI 错误 | 后台 work | native worker |
| TTY bytes／resize／timeout | 单 owner、raw 恢复、非阻塞、End／resize 保留 | sources + timer | POLL_ADD + TIMEOUT |
| `NativeWork` | 单次 execute／release、等待借用结束 | 后台 work | native worker |
| socket／文件／TTY close | 等在途完成，消费资源一次 | 等 source／worker 退出 | 等 CQE／worker 退出 |

表中的短 POSIX 控制操作不得包含可能阻塞的 regular-file I/O 或默认带阻塞 linger 的 socket close。EAGAIN 按相应等待路径重新订阅；不把它当 EOF。UDP 不按字节流重发剩余后缀。网络写沿 platform 的 SIGPIPE 防护，不修改进程全局 signal disposition。现有 Happy Eyeballs、SOCKS5、TLS 和 HTTP 不另写平台分支。

## 10. FFI、GC 与构建

native I/O callback／worker 不执行 managed 代码，参数在提交前复制到请求自有 native 存储，或使用仍有效的 `NativeByteBorrow`。共享层的操作表保活 managed owner；native callback context 不能保存 movable 对象裸地址。

唯一新增的异步 native→managed 入口是宿主通知桥，使用已有 `ForeignCallback` 的 retain／release、线程 attach、Context 与异常捕获合同，不手工扩展 runtime 调度 ABI。注销完成后才释放注册持有的 callback 引用；当前调用的 lease 在回调返回时退出。宿主侧可能长时间运行／等待的 FFI 调用也遵守 NativeSafe 边界，不能阻塞在 `GCLeaf` 中。

Darwin 链接系统 libdispatch；Linux 固定 liburing 源码与普通 C helper 随现有 Cone native 管线构建，GNU 与 musl 不混用头文件。目标选择沿现有 manifest 条件输入与预编译 sysroot 机制，不修改编译阶段职责或增加构建来源证明。发布时重建受公共类名与 native 输入变化影响的普通 Cone；缓存仍只按已有输入更新规则决定复用。

## 11. 实施顺序与兼容性

1. 先按本设计更新正式生效的 asyncio 规范，保留本次提案与现状的区分；实施前不把新 API 描述为已可用。
2. 抽出共享 `LoopOwner`、操作／资源状态和一次非阻塞 pump；暂用内部 libuv 适配运行现有行为测试，确认这是同一套任务／取消算法。
3. 完成 Darwin libdispatch 的全部操作及 `run`，再加入 session mailbox、host 注销合同与 `MainDispatchHost`。关闭与回调寿命必须一起交付。
4. 完成 Linux ring、eventfd、worker 与取消退休，覆盖 GNU／musl；把同一个宿主协议接入真实 fd-watch 测试宿主。
5. 迁移现有 TLS／HTTP／工具、示例、测试和文档的 `LibuvEventLoop()` 构造；移除 libuv adapter、vendor 输入和失去消费者的 UV 错误转换，重建预编译资源并做三目标定向验收。

公共 I/O 方法与成功／取消／关闭语义保持；构造类名变化是显式源码迁移，Linux 的 io_uring 运行要求也是部署变化。底层错误 domain 从 Libuv 迁移到真实 POSIX 来源，不能把原始 UV 数字或文案作为兼容承诺。迁移不提高任何无关编译器或库的最低系统要求，不新建长期双实现选择开关。

## 12. 验收与设计限制

普通库行为继续在所属 Cone 的 `scoop.test` 中验证：所有现有 I/O 功能、同方向并发拒绝、各方向同时在途、短读写、零长度、EOF、数据报、timer、文件位置、目录、DNS、TTY、`NativeWork` 与错误路径。跨 TLS／HTTP 的组合用例复用现有测试，不为每个后端复制测试实现。三目标使用相同公共语义断言，只有后端原生错误与资源退休测试按平台区分。

竞争测试用明确屏障安排提交前取消、已提交取消、完成后取消、close／取消竞争、SQ 暂满、迟到资源成功、部分进度、源／取消 CQE 的不同顺序，以及关闭期间外线程投递；不靠极短 sleep 猜时序。GC 定向测试覆盖移动期间的 buffer owner 保活和 host callback 引用，不把 GC stress 施加到所有普通 I/O 测试。

宿主接入验证覆盖：安装不内联、首次回调前关闭、从外线程 post／close、根 Context 保留、回调抛错、嵌套 pump、空闲无轮询、连续 I/O 下宿主事件能继续推进、移除后无 callback、通知 fd 不提前关闭。真实外部主循环与 native callback 也通过测试 Cone 的 native helper 和 `scoop.test` 验证，不新增 fixture；保留源码与工具，复用正常系统库构建。

Darwin 用实际主 dispatch queue／CFRunLoop 验证主线程进入、等待与退出；未来 GUI 库用实际 AppKit 再覆盖菜单跟踪、窗口拖动和模态行为。Linux 先用真实 poll 型 fd-watch 宿主与 ring 共存验证，GUI 库接入时再测试实际 Wayland／X11 循环。前者不是完整窗口系统验收，不宣称本任务交付了 GUI 库。

Linux 在 6.1 基线与当前发行内核各验证 GNU／musl 所需行为；未开放 io_uring 的机器应报告未满足验收环境，不能把目标初始化失败当测试通过。补充 ring 禁用／缺能力的启动错误与回滚覆盖。

性能记录只用于判断后端是否适合 GUI：空闲 CPU／唤醒次数、post 到执行延迟、timer 偏差、连续 I/O 时宿主事件延迟、吞吐与关闭延迟，并与迁移前 libuv 对照。不预设新后端必然更快，也不为这些测量增加公共资源预算或程序合法性规则。

实现按第 11 节分批提交；各批的验证范围、失败闭合与设计限制见实施记录。

## 13. Windows：IOCP 后端

每个 loop 拥有一个 IOCP、一个不可继承的 manual-reset event 和一个原生收集线程。收集线程使用 GetQueuedCompletionStatus 等待，直接把请求内预留的完成节点入队，随后 SetEvent；无 I/O 或 timer 时没有周期轮询。owner 的提交、取消、解码和 continuation 恢复继续使用共享驱动。IOCP 句柄不直接交给 GUI；GUI 监听 event，按既有 remove 完成合同退出监听。

OVERLAPPED 在请求内稳定存放，owner 在提交前登记请求。收集线程只写完成 code／count 和原生队列，owner 取出后才执行具体操作的收尾。同步成功不启用 skip-completion 模式，仍等 packet；同步失败仅在 API 明确不会投递 packet 时本地收尾。`CancelIoEx(handle, &overlapped)` 只请求取消，不关闭健康资源，也不退休借用；ERROR_NOT_FOUND 是与真实完成的正常竞争。依据 [IOCP](https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports)、[完成获取](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-getqueuedcompletionstatus)和[取消](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex)合同实现，不用取消成功冒充数据完成。

原生队列发布、SetEvent、ResetEvent 与队列取快照在同一 SRW lock 下完成，外线程命令唤醒也经过该边界；owner 清除旧通知后产生的新通知不会丢失。取走的一批有限工作处理完后返回宿主，期间产生的工作保留下一轮通知。借出的 event 由 loop 独占重置和关闭，host 不消费其状态；不能使用会被 host 等待自动消费的 auto-reset event。

timer 使用 QPC 截止时间，由收集线程的有界毫秒等待推进；插入更早 deadline 时通过 PostQueuedCompletionStatus 唤醒收集线程。超时后重新核对 QPC，不提前完成；超长等待按 DWORD 可表示的有限片段推进。timer 移除与到期入队受同一锁保护，取消没有额外 callback 或悬空 context。收集线程只做内存／队列工作，不能在其上执行阻塞文件、DNS、用户 NativeWork 或 managed continuation。

不可 overlapped 的操作使用与 Linux 相同宽度的按需 native worker 集合，队列取消跳过尚未执行的工作；运行中的工作真实返回后发布结果。worker 启动失败回滚已启动的空闲线程，通过通知异步退休，不在 owner 上 join。IOCP 请求、timer、work 与完成全部排空后再请求收集线程／空闲 worker 退出；它们最后一次访问 loop 的发布边界结束后才允许销毁。宿主注销、event 关闭和 onClosed 保持共享顺序。

直接创建／poll／销毁错误使用普通 C-layout 的 domain／code 结果；异步完成用已有完成记录的错误 kind 和 code。Win32 DWORD 原位保存，Winsock 提交错误保留 Winsock，CRT errno 保留 PosixErrno；不把 HRESULT／NTSTATUS 猜测成 errno。平台错误记录与事件分支的扩展不改变 runtime ABI。

Windows 普通测试复用同一 Cone，原生定向测试覆盖真实 overlapped 完成、取消与迟到成功、借用退休、timer、空闲不唤醒和 worker 屏障。宿主测试使用真实 Windows event 等待验证安装不内联、重入、跨线程 post、注销前保留 HANDLE 与注销后无回调。端口收集、文件／socket／TTY 和完整消费按 Windows 进度分批记录，不能由底层组件通过推断整个库已可用。

网络端点保留完整 SOCKET 与各一个输入／输出请求。bind／listen 复用 platform 的 owned socket helper，connect 自建同族 socket 并先 bind；所有 socket 都以 overlapped、不可继承方式创建。ConnectEx 与 AcceptEx 使用实际 provider 的扩展入口，accept 的 receive length 为零，接收地址缓冲留在稳定请求内；在 SO_UPDATE_CONNECT_CONTEXT／SO_UPDATE_ACCEPT_CONTEXT 成功后才交付 stream。数据完成在 owner 用 WSAGetOverlappedResult(FALSE) 取得 Winsock code／flags，失败时保留 IOCP 已给出的进度；UDP 截断按 WSARecvFrom 的完整缓冲前缀规则处理。参见 [ConnectEx](https://learn.microsoft.com/en-us/windows/win32/api/mswsock/nc-mswsock-lpfn_connectex)、[AcceptEx](https://learn.microsoft.com/en-us/windows/win32/api/mswsock/nf-mswsock-acceptex)、[WSARecvFrom](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsarecvfrom) 与 [WSAGetOverlappedResult](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsagetoverlappedresult)。

DNS、路径修改和目录枚举继续使用共享 worker 生命周期。平台条件的字符串入口在 Windows 借用 UTF-16，在 POSIX 借用原始字节；请求在提交内复制，完成后释放。原生目录复用 platform owner，next 按实际长度复制独立、终止的名称，允许 managed 在原目录关闭后消费；错误同次取回，Windows 为 Win32。DNS 复用 platform resolver，结果地址的 C-layout 同次带回 Winsock／PosixErrno，取消后的地址链仍经正常消费释放。

文件 owner 保存独立 HANDLE，普通文件的串行位置显式写入 OVERLAPPED，按完成长度推进；同步成功仍等 packet，EOF 不变成失败。append 使用同步句柄与现有 worker，从而由 OS 同时维护追加位置，避免在异步追加后查询 EOF 误认其他写入的位置。其余 metadata／truncate／sync／close 通过 platform 的结果 helper 执行。参见 [ReadFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-readfile)、[WriteFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefile) 和 [同步／异步文件位置](https://learn.microsoft.com/en-us/windows/win32/fileio/synchronous-and-asynchronous-i-o)。
