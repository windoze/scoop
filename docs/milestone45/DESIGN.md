# M45 设计：平台库、确定性资源释放与 native IO buffer

状态：实施中；M38 已于 2026-10-16 完成，M45-1/2/3/4 本机实现与回归完成，下一批为 M45-5。设计日期：2026-10-14。调研基线：`e548b9ebe`。实际验证见 [实施记录](PROGRESS.md)。

依据：[语言规范](../specs/SCOOP-SPEC.md)、[标准基础库规范](../specs/SCOOP-STDLIB-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)、[编译器与产物规范](../specs/SCOOP-IMPL-SPEC.md)与[路线图](../ROADMAP.md)。本文及配套文档记录 M45 的交付范围、API 合同、实施任务和验收要求，不表示接口已实现。每批实施前先同步对应 spec，再修改代码。

## 0. 已确认决定

1. `Disposable` 和普通／挂起两个 `use` 重载放入 `scoop.core`。它们是普通 Scoop 接口与泛型扩展，不新增 `using` 语法、析构语义或 compiler-known role。`use` 所需的双失败异常随它归 core。
2. 其余本期新增的平台功能全部放入普通 `scoop.platform` Cone，包括 native IO 使用的 off-heap `NativeByteBuffer`。GC pressure 提示入口作为既有 core GC API 的增量提供；既有 std 数据类型保持原归属，不复制到 platform。
3. 显式 `dispose`／`use` 负责确定性清理；现有 GC-free release hook 仅兜底执行允许的原始 native 清理。不能从 hook 调用 `dispose`，也不能据此承诺所有资源都会及时释放。
4. 本期提供同步、允许阻塞的 IO；不实现异步 IO、事件循环、调度器或协程取消。挂起版 `use` 可以配合已经存在的协程使用，但不会把文件或 socket 操作变成异步操作。
5. M38 已完成。命令行参数解析、schema、help 与 CLI 注解属于独立 `scoop.cli`，不进入 platform。platform 只提供原始进程参数与进程能力；文本输出复用 core 的 TextWriter。
6. 单列任务修复已复现的泛型扩展回调参数推断问题；不能在库 API 或示例中以强制用户补类型规避。
7. `scoop.platform` 自身的正常行为、运行期失败和功能组合尽量全部通过 `scoop.test` 验证。只有需要编译失败、产物／链接、GC／ABI 或特殊进程边界的情况才增加必要 fixture，不按每个库方法另建 fixture。
8. **实施过程中遇到任何编译器或 core 缺口，必须先修复对应层并完成必要回归，再回来继续依赖该能力的 platform 工作。不得绕过。** 不论是已有实现缺陷还是缺失能力，都不能用修改库 API、强制显式类型、削弱泛型、转入 C 实现或特殊调用路径替代底层修复；涉及契约变化时先改 spec。
9. NativeByteBuffer 仅在容量分配／释放变化时通过一个 GC-free 入口反馈 pressure delta。GC 自己决定如何使用，首版完全忽略、入口实现为 no-op 也是完整实现；不要求 buffer 触发收集或分配失败后的 GC 重试。

## 1. 文档与交付范围

| 文档 | 责任 |
| --- | --- |
| [资源协议](RESOURCES.md) | core Disposable／use、双失败、确定性清理、release 边界与参数推断问题 |
| [NativeByteBuffer](NATIVE-BUFFER.md) | off-heap 存储、游标、初始化边界、同步借用、IO 进度与 GC pressure 提示 |
| [平台 API](PLATFORM-API.md) | 文件／目录、stdio、路径与系统字符串、进程／管道、线程／同步、阻塞网络、时钟与熵 |
| 本文 | Cone 组织、实现边界、分批任务、规范同步与测试／验收 |

首版公共能力如下，细节以配套文档为准：

| 所在位置 | 必须交付 |
| --- | --- |
| core 根 package | Disposable、两个 use 重载、ResourceUseException；既有 GC API 增加 gcAdjustPressure |
| `scoop.platform.memory` | NativeByteBuffer、受检字节访问、显式扩容、同步 native pointer 借用 |
| `scoop.platform.io` | ByteSource／ByteSink、批量读写、缓冲包装、UTF-8 输入输出、标准流 |
| `scoop.platform.fs` | Path、文件操作与 metadata、目录迭代、临时文件／目录、符号链接 |
| `scoop.platform.system` | OsString、统一系统错误、OS／架构／处理器信息 |
| `scoop.platform.env` | 原始 argv、环境快照、cwd／home／temporary directory 查询 |
| `scoop.platform.process` | Command、ChildProcess、退出状态、重定向、同步 communicate、匿名管道 |
| `scoop.platform.concurrent` | Thread、join、Mutex、ConditionVariable、同步 withLock、线程标识／yield |
| `scoop.platform.net` | IP／端点、阻塞 DNS、TCP listener／stream、UDP datagram |
| `scoop.platform.time` | SystemClock、MonotonicTimeSource、阻塞 sleep、耗时测量 |
| `scoop.platform.random` | SystemRandom、OS 熵填充，与现有 Random／Uuid 组合 |

### 1.1 参考与取舍

| 参考 | 采用的经验 | Scoop 边界 |
| --- | --- | --- |
| Rust std 的 fs/io/path/env/process/thread/sync/net/time | 小而可组合的能力、部分 IO、明确错误和资源归属 | 使用 Scoop 异常、GC 与普通接口，不移植借用检查器或 Drop |
| Rust OsString／Path | 系统字符串与 Unicode String 分开 | 首版三目标都使用 POSIX 原始字节，不损坏非 UTF-8 文件名／参数 |
| Kotlin/Native platform libraries、C interop | 系统调用由 native binding 承接，native allocation 有明确生命周期 | 公共 API 是 Scoop 类型；C shim 只处理 ABI／OS 操作，不直接公开整套 POSIX |
| C# IDisposable、Kotlin AutoCloseable／use | 幂等显式释放、作用域退出清理 | core dispose／use；GC-free hook 不等同于 managed finalizer |
| Netty ByteBuf、Rust bytes BufMut | 独立读写游标、区分容量与已初始化数据 | 首版连续独占 buffer，不引入引用计数 slice、pool 或分段存储 |

Kotlin/Native Worker 已标为过时 API，M45 不照搬其 worker queue／对象转移模型；使用现有 Scoop 多线程内存模型、callback 和 TaskContext。

## 2. Cone、依赖与实现位置

新增 `sysroot/lib/scoop.platform/Cone.toml`，基础配置为：

```toml
schema = 1

[cone]
group = "scoop"
name = "scoop.platform"
version = "0.1.0"
kind = "library"

[dependencies]
"scoop:scoop.core" = "0.1.0"
"scoop:scoop.std" = "0.1.0"

[test-dependencies]
"scoop:scoop.test" = "0.1.0"
```

依赖方向为 platform → std/core，test → core；platform 的测试目标额外依赖 test。core 不依赖 platform、std 或 test。Disposable／use 定义于 core 现有根 package，沿普通 public prelude binding 提供，不把 Cone 名误当作源码 package。

platform 源码按上述 package 和实际职责分文件；native 文件放在自己的 `native/` 目录。通过现有 `native.sources`、`native.libraries` 及 `when = { os = "darwin" }`／`when = { os = "linux" }` 选择 C11 shim，必要时再区分 gnu／musl。没有平台差异的 Scoop 源码保持共有；不增加 expect/actual、声明级条件编译、插件或运行期后端注册。

platform 无默认依赖或默认导入。应用显式声明 `scoop:scoop.platform:0.1.0`，使用 manifest Cone；single-file 模式保持 core-only。sysroot 默认定位、普通 `.slib`、跨 Cone 泛型实例化、native requirements 与 artifact-only link 复用现有流程。

复用 std 的 `ByteOrder`、集合、`Duration`、`Instant`、`Clock`、`TimeSource`、`TimeMark`、`Random` 和 Uuid。系统实现使用 `scoop.platform.time.SystemClock` 等独立名字，不向依赖 Cone 的 companion 注入成员。合适的 writer 实现 M38 已交付的 TextWriter 普通接口，不复制同名协议。

## 3. Native 与 runtime 边界

Scoop 负责公共对象、参数检查、异常、所有权状态、缓冲算法、编码与 API 组合。C shim 负责系统 header／macro／不透明结构、syscall、错误值快照、native allocation 和线程入口等实际 ABI 边界。只包含当前功能需要的 libc／OS 代码，不引入 C++、通用 FFI 生成器或另一套 IO runtime。

- 阻塞 read/write、connect/accept、DNS、锁等待、join、waitpid、sleep 等使用现有 NativeSafe FFI。`@GCLeaf` 只用于真正满足其合同的短操作，不能以减少切换成本为由标注可能等待的调用。
- 托管数组／String 直接传给同步 C 调用时复用 core 的作用域借用与 pin；off-heap 数据只需保活 owner，不要求固定 GC wrapper。两类地址的生命周期不能混用。
- errno 在实际失败调用之后立即捕获。pthread／getaddrinfo 等返回自身错误码的 API 按真实约定处理，不统一从线程的“最后错误”读取。
- managed callback、native thread attachment、精确根、返回握手、异常隔离与 TaskContext 复用现有能力。C 不保存无根的 managed 指针，不让 Scoop 异常越过 C frame。
- release hook 只能走其允许的 raw 清理路径，不能阻塞等待 mutator、flush、join、运行用户代码或操作 callback ownership。
- 唯一预期新增的 GC 支持是一个 GC-free 的 pressure delta 提示入口，显式分配／释放与 release hook 共用，见 buffer 文档。GC 自己决定如何消费提示，首版可以完全 no-op；不要求精确账本、主动收集或分配失败后的 GC 重试。实现经 M44 的 GC 操作／适配边界接入；库不包含 Immix 私有 header，也不复制 page map、线程或收集逻辑。
- 普通库接口及推断修复不预设新的 IR／产物字段。只有实际 ABI 合同变化才按既有版本规则更新相应 fingerprint，并重建受影响产物。

遇到任何编译器或 core 缺口，先保留最小复现，按 spec 先行修复对应层并完成必要回归，再返回依赖它的 platform 实现。不得以库侧 workaround、类型名特判、降低 API 能力或 C 后备绕过，也不得把尚未修复的问题仅登记为风险后继续交付受影响功能。runtime 或构建链路中的实际缺陷沿用同一处理顺序。

## 4. 实施任务

各批先修订其负责的规范，随功能提交 Scoop 测试及必要底层回归。未注明完成的批次仍待实施；Linux 验收在本机所有批次稳定后集中执行。

| 任务 | 内容 | 前置与完成出口 |
| --- | --- | --- |
| M45-1 | 修复泛型扩展回调参数推断（本机完成） | 普通／挂起 lambda 参数从 receiver 和候选参数类型推导，源码与跨 Cone 都通过；保留应失败的重载／effect 诊断；Linux 纳入 M45-10 |
| M45-2 | platform Cone、系统字符串／路径及错误（本机完成） | 普通 native sources、scoop.test 宿主与下游 `.slib` 消费已通过；OsString／Path 无损往返、参数检查与错误快照通过；Linux 纳入 M45-10 |
| M45-3 | core Disposable 与两个 use（本机完成） | 依赖 M45-1，行为测试使用 M45-2 的测试宿主；普通实现与模板可只依赖 core 使用；正常、异常、双失败、嵌套和受控挂起／恢复清理通过 |
| M45-4 | NativeByteBuffer 与 GC pressure 提示（本机完成） | 依赖 M45-2/3；容量／游标／借用／释放闭环，增长失败保留原状态，hook 与显式路径不重复 free／反馈；delta 增减与回滚配平，GC 采用 no-op 也通过验收 |
| M45-5 | 同步字节 IO、文件／目录、标准流与文本 | 依赖 M45-4；实际读写、短 IO、EOF、错误进度、目录提前退出、UTF-8 与资源组合通过 |
| M45-6 | 环境／系统信息、时钟与系统熵 | 原始 argv／环境与 std 数据模型正确组合；sleep 不阻止其他 mutator 推进；不依赖 M38 |
| M45-7 | Thread、Mutex、ConditionVariable | 复用 callback／Context／NativeSafe；创建失败、join／dispose、线程异常与同步语义通过；worker ownership 完成后释放 |
| M45-8 | Command、ChildProcess、管道与 communicate | 依赖文件、线程；posix_spawn、参数／环境／cwd、退出／信号、双向管道和无死锁输出收集通过 |
| M45-9 | 阻塞 DNS、TCP 与 UDP | 依赖 IO、线程；loopback、部分传输、半关闭、UDP 空包／截断和 SIGPIPE 策略通过 |
| M45-10 | 真实应用组合、三目标与产物消费 | 三目标运行本库测试、debug/release 代表组合；源码和工具保持存在，验证下游产物消费、链接运行及实际调用复用；记录实际结果与限制，无未实现占位或绕行；pressure no-op 按明确合同验收 |

M45-1 修复现有语言规则的实现，不改变普通 lambda 必须显式写 `suspend` 才能成为挂起 lambda 的规则，也不开放“只相差外层 suspend”的重复签名。

## 5. Spec 先行与跨文档同步

实施时按下表同步权威合同，不把未来 API 写成当前已实现能力：

| 文档 | 同步范围 |
| --- | --- |
| SCOOP-SPEC | 8.1/8.2/8.6 的既有函数类型、挂起与推断规则；11 章新增资源协议；11.7 双失败异常；9.1.6 与 14 章增加 GC-free pressure 提示及有限 release/runtime 合同，明确首版可忽略；12.6 普通 core API |
| 新增 SCOOP-PLATFORM-SPEC | 从本设计收敛公共平台 API、所有权、错误、IO 进度、buffer、线程／进程及三目标差异；实现前建立相应章节 |
| SCOOP-STDLIB-SPEC | 1.1、5.1、6、8、10 章的分层引用：std 保持纯 Scoop，系统来源／off-heap／IO 在 platform，不挪动现有类型 |
| SCOOP-RUNTIME-SPEC | 3.1/3.8/3.9 与 4 章的统一 GC-free pressure 提示、no-op 合同及 GC 自定策略边界；复核 3.5、4.3、7 章的线程／callback／退出组合，不擅自新增程序 shutdown 时自动 join 或取消 |
| SCOOP-IMPL-SPEC | 2.2 候选局部推断的实现职责；2.7/2.8 普通构建、测试及 native 闭包；2.12 core／普通依赖；仅实际 ABI 变化需要版本迁移 |
| ROADMAP | 任务进度、M26 off-heap 后续项的归属与 M45 的 scoop.test 优先规则 |

M38 的接口与验收仍由 M38 文档记录；M45 不向 platform 搬迁 CLI 参数解析功能。

## 6. 测试策略

### 6.1 以 scoop.test 承担库行为

测试主要放在 `sysroot/lib/scoop.platform/tests/`，按 resources、memory、io、fs、env、process、concurrent、net、time、random 分组，使用已有 `@Test` 容器、断言与 BeforeTest／AfterTest。同 Cone 的 internal 可见性足以测试实际边界，不扩展 private 访问权限。

Disposable／use 的行为测试同样放在这个测试目标中，只使用 core 资源声明和本地测试资源。不要给 core 增加到 test 的测试依赖：按当前依赖规则会形成 core → test → core 环。必要的 core-only 消费和推断回归由少量独立编译器用例承担。

M43 仅支持同步测试方法。挂起版 use 通过普通测试方法调用已有 startCoroutine／Continuation，显式保存、恢复或注入失败后断言结果；本期不新增 suspend Test、runTest、调度器或通用等待包装。

常用入口：

```text
scoop test sysroot/lib/scoop.platform
scoop test sysroot/lib/scoop.platform --release
scoop test sysroot/lib/scoop.platform --filter NativeByteBufferTest
```

### 6.2 覆盖矩阵

| 范围 | 主要 scoop.test 行为断言 |
| --- | --- |
| Disposable／use | 回调恰好一次、正常／异常退出、双失败保留、重复释放、嵌套逆序、内层构造失败、挂起期间保持资源与恢复后清理 |
| NativeByteBuffer | 空容量、边界／溢出、未初始化尾部不可读、数值字节序、扩容／compact／clear、数组独立副本、异常时游标、借用期间拒绝冲突操作 |
| 文件／目录／文本 | 临时目录内实际 IO、open 模式、seek、短读写、EOF、非 UTF-8 名称、symlink、提前退出关闭、无效 UTF-8 与缓冲清理 |
| 线程／同步 | barrier／谓词循环协调、锁可见性、虚假唤醒处理、join、worker 异常、失败回滚、受控 Context 捕获 |
| 进程／管道 | 含空格与非 UTF-8 参数、空参数、显式环境／cwd、stdin EOF、stdout/stderr 同时大量输出、正常／信号状态与 wait/reap |
| 网络 | loopback 与 OS 分配端口、TCP 部分传输／半关闭／拒绝连接、UDP 空包／边界／截断、损坏端点、断开连接时的错误 |
| 时间／随机 | std 接口组合、非递减 monotonic 读数、sleep 参数与线程进度、熵写入长度／范围、显式 SystemRandom 生成 UUID |
| 组合 | buffer → 文件 → 子进程管道、buffer → TCP/UDP、异常展开中的多资源清理、活动 IO／线程与 GC 并行推进 |

测试用独占临时目录、loopback 和动态端口；不依赖公网、外部 DNS 服务、固定文件名或固定端口。线程协调使用同步原语，不用 sleep 猜测对端进度；测试超时由外层执行控制提供。wall clock 可以校时，不能断言其单调；熵测试不以“字节必须不同／不为零”或随机统计阈值判定通过。

可稳定制造的 OS 错误以及受控 ByteSource／ByteSink 的部分进度由 Scoop 测试断言；native allocation 失败、真实 syscall 返回组合等需要精确注入的情况，使用实际 C shim／runtime 的窄专测。测试接缝不成为生产 API、provider、插件、策略开关或第二套实现，也不为此扩展 manifest。必要的外部辅助进程采用明确的本地程序；特殊辅助程序的构建可由少量现有 fixture 场景组织，业务断言仍尽量留在 Scoop 测试中。

### 6.3 必要专测与 fixture

允许新增的范围限于：

- M45-1 的类型／重载正例、准确的 negative 诊断，以及必要的源码／上游产物组合；不把每个库运行错误写成编译失败 fixture。
- core-only 使用 Disposable／use、platform 普通 `.slib`／native requirements 的真实下游与 artifact-only 消费，三目标 ABI 差异。
- release hook、owner 精确保活、moving GC、pressure 通知增减／回滚、no-op 合同及通知不重入收集的 runtime／GC 专测；窄测试接缝观察通知，不要求生产 GC 保留统计或由 pressure 触发某次收集。
- 当前 scoop.test 无法自行准备的 native 辅助进程、异常退出或进程级隔离。新增场景应写明该例为何不能由普通库测试覆盖。

不对库行为保存 AST/HIR/MIR/LIR 或完整符号表快照，不重复已经覆盖的 compiler／GC 全量验收。Rust／C 底层修复按影响跑相关单元测试和已有 fixture；共享 ABI、GC 或产物路径确实变化时，再据影响扩大回归。只有文档或普通库实现变化时，不要求完整 Rust workspace／全 fixture 重跑。

## 7. 完成条件与后续范围

三个现有 target：`aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`，均须完成实际构建和运行；交叉编译成功不能代替目标上的运行证据。本库正常／失败／组合测试以 scoop.test 为主，保留 debug/release 代表性运行，以及所改 runtime／编译器边界的必要验证。

交付时有真实应用依赖 platform `.slib` 完成链接／运行；生产产物不包含测试声明或仅测试依赖。实施中发现的编译器／core 缺口已在对应层修复并回归，受影响的库实现和测试已移除临时定位用 workaround，不能把它们留作完成条件的例外。M45-1 的隐式 lambda 参数推断通过，资源双失败不丢失异常，IO 部分进度可观察，native buffer 不能经 safe API 暴露未初始化字节，显式与兜底释放不会重复处理同一资源。

pressure 仅按已承诺的提示合同验收：buffer 正确报告容量变化，入口始终 GC-free，GC 可以完全忽略提示。首版 no-op 是允许的策略选择，不属于未实现占位；它不改变其他编译器／core 缺口必须先修的要求。

明确后续：异步 IO／事件循环／执行器、协程取消与异步 dispose、mmap／文件锁／文件监听、Unix-domain socket／高级 IPC／信号回调、TLS／HTTP、完整时区数据库与 DST、动态库加载、全局环境／cwd 修改 API、native arena／pool／共享 slice／引用计数 buffer、O_DIRECT 特殊对齐、Windows／新 CPU target、platform-native integer 迁移。当前必要的 SIGPIPE 防护和 OS 时间／熵调用不扩展为这些框架。

本期不承诺 GC 或进程退出会完成这些新增资源的 flush、join、子进程回收、解锁或业务操作；既有 core 对标准输出的退出刷新合同保持。程序需要资源操作的完成结果时，必须通过相应显式 API 和 use 的正常／异常退出路径完成。

## 8. 参考资料

- [Rust std](https://doc.rust-lang.org/std/)、[std::io](https://doc.rust-lang.org/std/io/)、[std::fs](https://doc.rust-lang.org/std/fs/)、[std::process](https://doc.rust-lang.org/std/process/)、[std::thread](https://doc.rust-lang.org/std/thread/)、[std::net](https://doc.rust-lang.org/std/net/)。
- [Rust OsString](https://doc.rust-lang.org/std/ffi/struct.OsString.html) 与 [set_var 的线程约束](https://doc.rust-lang.org/std/env/fn.set_var.html)。
- [Kotlin/Native platform libraries](https://kotlinlang.org/docs/native-platform-libs.html)、[C interop 与 nativeHeap／memScoped](https://kotlinlang.org/docs/native-c-interop.html)、[Worker](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.native.concurrent/-worker/)。
- [.NET Dispose 模式](https://learn.microsoft.com/en-us/dotnet/standard/garbage-collection/implementing-dispose)、[Kotlin AutoCloseable](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin/-auto-closeable/)、[Kotlin use](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin/use.html)。
- [Netty ByteBuf](https://netty.io/4.1/api/io/netty/buffer/ByteBuf.html)、[Rust bytes BufMut](https://docs.rs/bytes/latest/bytes/trait.BufMut.html)。
