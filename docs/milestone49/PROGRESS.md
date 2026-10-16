# M49 实施进度

基线：`d233d05a5`。日期：2026-10-16。规范先行，按功能验证后提交；本机定向验证后在 Linux GNU／musl 完整验收。日志和测试工作目录位于仓库外，不使用 `/tmp`。

| 批次 | 状态 |
| --- | --- |
| core 任务树、失败传播与取消 | 已实现，本机定向验收通过；Linux 留到整体完整验收 |
| 可取消挂起协调 | 已实现，本机同步／挂起／跨线程竞争与 GC 验收通过 |
| platform 异步 buffer 借用与 Libuv 错误域 | 已实现，本机 debug／release 与 moving GC 定向验收通过 |
| asyncio 事件循环、定时等待与投递 | 已实现，本机 debug／release／moving GC 八项 scoop.test 通过 |
| TCP 与异步流组合 | 待实施 |
| 文件、DNS、UDP | 待实施 |
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
