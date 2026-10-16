# M49 实施进度

基线：`d233d05a5`。日期：2026-10-16。规范先行，按功能验证后提交；本机定向验证后在 Linux GNU／musl 完整验收。日志和测试工作目录位于仓库外，不使用 `/tmp`。

| 批次 | 状态 |
| --- | --- |
| core 任务树、失败传播与取消 | 已实现，本机定向验收通过；Linux 留到整体完整验收 |
| 可取消挂起协调 | 已实现，本机同步／挂起／跨线程竞争与 GC 验收通过 |
| platform 异步 buffer 借用与 Libuv 错误域 | 已实现，本机 debug／release 与 moving GC 定向验收通过 |
| asyncio 事件循环、定时等待与投递 | 已实现，本机 debug／release／moving GC 八项 scoop.test 通过 |
| 异步字节流协议与组合扩展 | 已实现，本机 debug／release／moving GC 六项 scoop.test 通过 |
| TCP | 已实现，本机 debug／release／moving GC 定向验收通过 |
| DNS | 已实现，本机 debug／release／moving GC 验收通过 |
| 文件 | 已实现，本机八项定向及完整 asyncio release／moving GC 验收通过 |
| UDP | 实施中 |
| 三目标组合及完整验收 | 待实施 |

开始时清理旧 debug 构建数据，保留 release 工具，`target` 从约 103 GB 降至 1.3 GB。M47 的已有文档改动不属于本任务提交。

core 初次跨 Cone 验证发现：普通泛型模板调用 internal suspend 成员时，HIR 已保存支持声明，但 MIR callable 导出选择遗漏该成员。已先独立修复为遍历共有完整声明目录；新增 `m49-suspend-support-export` 覆盖 private 顶层及 internal 成员的真实跨 Cone 调用，debug／release 和 moving GC 通过。相关 Rust 119 项由一轮 118 项通过及旧断言修订后的单项补验闭合；格式化和 workspace clippy 通过。未改变 wire 或 runtime ABI。

## core 基础验收

实现按任务状态、构建器、可取消等待和通知登记拆为四个普通 Scoop 文件，最长 192 行。公开声明与精确 Job Context binding 均由普通 core 产物消费，未新增编译器角色或 runtime 机制。

本机 `m49-structured` 四组 fixture 的 debug／release 共八个变体通过，每个程序同时通过普通及 moving GC 运行。覆盖多层子树收尾、关闭后拒绝启动、值／引用结果、重复 await 与失败身份、未 await 的失败传播、finally、等待者取消与原始不可取消挂起、登记异常和迟到完成、取消回调失败、跨线程恢复及 Context、完成／取消竞争和启动／作用域关闭竞争。最终组合补验只重跑修改过的三组，六个变体全部通过；未重复无关全量测试。`target` 再次检查为约 1.9 GB。

## platform 前置能力验收

NativeByteBorrow 使用普通公开 Unsafe API 持有 owner，提交已知进度后解除借用；所有 buffer 状态变更与借用登记共享短临界区，用户 callback 在锁外运行。同步 API 保持原行为。Libuv 错误域保留原始负码，复用目标 errno 分类，不给 platform 引入 libuv 依赖。

本机 scoop.test 的 NativeByteBuffer 17 项和 PlatformError 3 项在 debug／release 全部通过；buffer 17 项另通过 moving GC。覆盖跨真实挂起、跨线程、借用竞争、非法长度回滚、重复完成、零指针及全部旧同步 buffer 用例。格式化、native lint 与 workspace clippy 通过。

下一批实现 TCP 与异步流组合；整个 M49 尚未完成。

接入未修改的 libuv C 源码时发现第二个底层缺口：Darwin 将 libc 的 `__cxa_atexit` 误判为 C++ ABI，同时两种对象格式拒绝普通 C 初始化／析构段。先修订实现及 runtime 规范，允许目标 CRT 的标准 C 初始化／清理，保留真实 C++ EH、动态 TLS、LTO 等边界。旧 constructor 拒绝 fixture 改为独立的实际初始化和退出清理回归，本机 debug／release 两变体、四进程通过；链接器相关两项 Rust 测试及 workspace clippy 通过。未为 libuv 添加特判。

## 事件循环基础验收

新增普通 asyncio Cone、[公开规范](../specs/SCOOP-ASYNCIO-SPEC.md)及未修改的 libuv 1.53.0 三目标源码清单。已交付 EventLoop.run、runWithResult、currentEventLoop 与 sleep；网络和工作请求接口按后续批次实现，规范中的完整接口表不表示提前交付。managed 命令队列与完成选择使用普通同步；C callback 只记录事件，返回 NativeSafe 后才恢复 continuation。定时器使用单调时间，支持向上舍入及超长 Duration 的分段等待。

同 Cone 八项 scoop.test 在本机 debug、release、moving GC 全部通过：值／引用结果、根与未 await 子任务失败、精确 Context binding、嵌套／重复／并发 run 拒绝、跨线程后续 I/O、合并唤醒、多定时器、取消登记前后与真实 native close 收尾。主动编写的 Scoop／C 实现文件最长 110 行；vendor 保持上游原文件。workspace clippy 与 shim 的 C11 警告检查通过。`target` 检查约 2.4 GB，仍远低于开始时的 103 GB；Linux 全量留到功能组合完成后。

## 异步流组合验收

新增 AsyncByteSource／Sink／Stream、AsyncCloseable、readFully／writeFully／copyTo 和泛型 useAsync，分别放在 70 行流文件和 14 行资源文件。六项 scoop.test 在 debug／release／moving GC 通过；端点通过真实定时器逐次挂起，覆盖短传输、EOF、读写失败前缀、零进度、范围与容量错误、接口包装后的 flush 转发、关闭责任及正文／清理双失败。未增加执行器或测试专用生产后端。下一步接入实际 TCP 双向流。

## TCP 与资源收尾验收

新增 connect／listen／accept、IPv4／IPv6 双向字节流、输出半关闭、取消和幂等关闭。run 收尾等待未关闭资源与全部 native callback；未交付的迟到连接／listener 成功先关闭。监听复用 platform 的普通 C socket_bind helper，保持 reuseAddress=false 与原始 PosixErrno；其余 libuv 错误保留负码。Linux 在线程局部屏蔽／恢复 SIGPIPE，Darwin 使用 SO_NOSIGPIPE。

新增十二项同 Cone 测试，覆盖短读写、EOF 后写入、同方向互斥、跨线程提交、真实排队写取消后的精确前缀与继续写入、关闭竞争、取消后 shutdown 终止、原生错误与 SIGPIPE handler／mask／pending 保留。写取消通过 libuv 公开同步 API 先观察真实排队状态，不依靠任意延迟。全部二十六项（包含既有 loop／stream）通过 moving GC；debug／release 首轮各二十五项通过，修正一个测试的越界异常预期后定向补验闭合。C11 警告检查、workspace clippy 与差异检查通过；实现文件最长 143 行。target 仍约 2.4 GB，保留正在复用的构建缓存。Linux 三目标组合验证仍待全部功能完成。

## 原始 DNS 与平台 helper 复用

公开 platform 的同步 Unsafe 原始字符串／路径借用、已验证的 OpenOptions helper flags、普通 C helper 头文件及共享 NativeSocketAddress 类型。两个新增平台测试通过普通与 moving GC；asyncio 直接消费这些普通产物声明。首次 DNS 链接显示同形 nominal 不能替代同一 native 合同的 exact type，已改为共享 platform 定义，未改编译器 ABI 规则。

resolve 通过 uv_queue_work 调用原始 getaddrinfo helper，提交时复制原始名称，after-work 释放输入，managed 消费或取消时释放 addrinfo。保留 AddressResolution 与同次 EAI_SYSTEM errno；原生队列撤销和执行后完成共用已登记请求的单次完成路径。本机三项 scoop.test 通过，覆盖 IPv4／IPv6／localhost 与实际 TCP 组合、NUL／port／真实 EAI 失败，以及提交前后反复取消和后续解析。文件将复用同一工作请求队列；release、moving GC 及 Linux 在后续组合验证中闭合。

## native include 路径修订

文件工作请求测试的普通 native 依赖需要共享 libuv 头文件，发现 manifest 将 include／forced include 套入禁止越出 Cone 根的源码身份路径。先修订语言、实现及 M33 设计，改用普通 PathBuf，保留原始路径交给实际文件系统解析；相对路径以 Cone 根为基准，允许绝对路径，仍拒绝空路径／NUL。没有增加来源审计、复制隔离或产物验证。manifest 解析缓存更新为 manifests-v2。

manifest／toolchain 共九十项 Rust 测试及三项 manifest cache 测试通过；新增共享 include／forced header 的 debug、release 实际 C 编译运行回归，并把两个旧越界路径负例改为空路径语法负例，三个 fixture／四个变体／六个进程全部通过。workspace clippy 通过。编辑器后台构建结束后用 Cargo 清理 27.2 GiB 旧 debug 数据，复建定向测试后 target 约 1.9 GB。

## 异步文件与工作队列验收

open／顺序 read、write／sync／close 复用 platform 原始 helper，经实际 libuv worker 执行。一个文件只允许一个活动操作；关闭先请求取消并等待 worker 与 buffer 借用结束，再提交一次 raw close。open 预留关闭请求，迟到成功在报告取消前释放 fd；路径和原始 errno 随异常保留。实现文件最长 96 行。

新增八项同 Cone scoop.test，本机定向全部通过；完整三十七项 asyncio 在 release 和 moving GC 全部通过，也闭合 DNS 的对应 profile。覆盖 append／EOF／sync／原始路径与失败、文件到 TCP 的短 buffer copy、并发关闭、取消 finally、真实排队请求和阻塞 FIFO 执行中取消、迟到 open 的 fd 清理。工作线程占位器只存在于普通 test-dependency，使用实际 uv_queue_work 与 syscall 建立确定顺序，不增加生产后端。C11 警告检查、workspace clippy 和差异检查通过。target 约 1.9 GB；Linux 完整验证仍留在 UDP 完成后。

## 共享常量段前置修复

UDP 的 debug 测试通过后，release 对象暴露已有边界缺口：LLVM 数值跳转表与已命名字符串共享 __TEXT,__const，原 literal reader 仅接受整段独占表。以普通多分支算术和数组构造建立 17 行最小复现，先修订实现规范，再让表的实际范围止于首个已有 atom；保留表内无 relocation、代码实际引用及既有 atom 边界检查，不忽略未知段或关闭优化。

九项相关 Rust 测试、workspace clippy，以及独立 CLI fixture 的 debug／release 两变体、四个进程通过。诊断用对象提取代码已移除。该修复不修改格式或 runtime ABI，作为独立底层提交；随后继续 UDP 组合验收。
