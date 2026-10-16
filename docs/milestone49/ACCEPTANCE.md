# M49 验收

状态：已完成。日期：2026-10-16。实施与独立前置修复见 [PROGRESS.md](PROGRESS.md)，公共合同见 [异步 I/O 规范](../specs/SCOOP-ASYNCIO-SPEC.md)和语言规范 11.9.1～11.9.2。

## 交付范围

| 能力 | 验证入口 |
| --- | --- |
| core 任务树、Job／Deferred、失败传播、取消、Context 与跨线程完成 | [四组结构化并发 fixture](../../tests/fixtures/m49-structured) |
| 跨挂起 NativeByteBorrow、Libuv 错误域与共享 native helper | [platform 内存测试](../../sysroot/lib/scoop.platform/tests/memory)、[错误测试](../../sysroot/lib/scoop.platform/tests/system/errors.scoop)、[helper 测试](../../sysroot/lib/scoop.platform/tests/fs/native_helpers.scoop) |
| EventLoop 根入口、定时等待、跨线程投递、收尾与移动 GC | [loop 测试](../../sysroot/lib/scoop.asyncio/tests/loop) |
| 短传输、失败前缀、flush、copyTo 与 useAsync | [stream 测试](../../sysroot/lib/scoop.asyncio/tests/streams) |
| TCP 双向传输、半关闭、实际写进度、取消、关闭与 SIGPIPE | [TCP 测试](../../sysroot/lib/scoop.asyncio/tests/tcp) |
| 原始 DNS 名称、EAI 错误与工作请求取消 | [DNS 测试](../../sysroot/lib/scoop.asyncio/tests/dns) |
| 文件顺序读写、sync、排队／执行中取消、迟到 open 清理 | [文件测试](../../sysroot/lib/scoop.asyncio/tests/files) |
| UDP IPv4／IPv6、空包、截断、来源、整包发送、取消与组合 | [UDP 测试](../../sysroot/lib/scoop.asyncio/tests/udp) |
| asyncio 生产产物的公开泛型、接口与 native I/O 消费 | [外部消费者 scoop.test](../../sysroot/lib/scoop.asyncio/consumer-tests/tests/public_api.scoop) |

asyncio 共 45 项同 Cone scoop.test，使用真实 loopback、原始文件和普通 native 测试依赖。功能通过普通 Cone、公开 FFI、已有 GC／Context 合同实现。主动编写的 core 单文件最长 192 行，asyncio 实现最长 143 行；libuv 1.53.0 保持上游源码。

## 验证结果

| 环境与范围 | 结果 |
| --- | --- |
| Darwin asyncio | debug／release／moving GC 各 45 项通过 |
| Darwin core | 四组 fixture、debug／release 共八变体通过；各程序同时验证正常与 moving GC |
| Darwin platform | buffer 17 项、错误 3 项在 debug／release 通过；buffer 17 项另通过 moving GC；新增 helper 2 项三种模式通过 |
| Linux GNU asyncio | debug／release／moving GC 各 45 项通过 |
| Linux musl asyncio | debug／release／moving GC 各 45 项通过 |
| 外部消费者 | 三目标的 debug／release／moving GC 各 1 项通过 |
| Linux platform | GNU／musl 的 debug／release／moving GC 各 106 项通过 |
| Linux Rust workspace | 5,349 项单元／集成测试和 2 项 doctest 通过；保留已通过结果，失败组定向补验闭合 |
| Linux fixture 公共规则 | 50 项通过 |
| Linux GNU 正式 CLI | 4,159 项通过，29 项目标不适用；4,700 变体、16,438 进程 |
| Linux musl CLI 定向 | M49、native C、musl 专属及相关回归共 48 项、65 变体、217 进程通过 |

本机每批先格式化、workspace clippy 和相应 native 告警检查，再定向测试。Linux 执行 fmt／clippy、release 配套工具构建及 GNU／musl shim 检查。musl 环境使用仓库外普通 `--cc` 包装器补齐系统 Linux UAPI 搜索路径，未修改 vendor，也未混入 glibc 用户态头文件。

GNU CLI 承担完整编译器回归，保留已通过项和源文件／缓存，随其他验收结束提高剩余用例并发度；最终按名称合并核对全部 4,188 项，无遗漏、无未闭合失败。musl CLI 聚焦当前变更及目标差异，避免重复已有前端全量。Rust 保留已通过的 debug 结果；driver 十一项在 debug 通过，其余七十二项使用 release 测试二进制，补验后核对全部八十三项无遗漏。以上结果来自分段执行与定向补验，并非一次无失败的全量命令。

## 失败闭合与独立回归

以下底层问题均以独立回归验证，先修订必要规范，再修复实现并单独提交：

- 泛型支持声明漏导出 internal suspend 成员：[跨 Cone fixture](../../tests/fixtures/m49-regressions/suspend-support)。
- 接口私有方法被选入导出却缺少 provider 正文：复用现有成员物化规则，[独立 typed IR 回归](../../compiler/hir-lower/src/tests/m23_type_semantics_production/source_mir_types/callables/private_interfaces.rs)与[跨 Cone 接口分派](../../tests/fixtures/m23-cli-source-dispatch/interfaces)共同验证。
- 普通 C 初始化／析构段及 libc `__cxa_atexit` 被误拒绝：[初始化与退出清理 fixture](../../tests/fixtures/m33-native-c/initializers)。
- native include／forced include 误套源码身份路径限制：[共享头文件 fixture](../../tests/fixtures/m49-regressions/native-includes)。
- 数值跳转表与命名常量共享只读段时被拒绝：[17 行最小程序](../../tests/fixtures/m49-regressions/shared-literal-section/main.scoop)。

Linux 首轮 Rust 的三项失败来自结构测试混用 core／普通模块身份；复用正式 core bootstrap 输入后，原结构断言通过。链接器另有一项 archive 测试仍要求拒绝普通 C constructor，改用仍被禁止的 C++ EH 后，原延后检查边界通过。没有增加生产编译器回退或平行语义校验。

driver 十九项布局测试共用的旧分类误要求本地协程 frame／恢复适配器进入公开布局表；按已有类型化生成身份修正测试，并将描述符检查拆为小模块。另修正一项槽位字段身份断言以识别实际生成的 context／frame，以及一项缺失元数据负例对错误顺序的旧预期。二十一项均完成 Linux 补验，未改变生产产物或 ABI。

完整 CLI 发现五项 M34 优化专测仍引用基线已删除的异常物化 API。按现行 runtime 规范直接保留 Throwable 引用，更新相关 MIR／LIR 预期与编号；保持优化、ABI、正常运行、moving GC 和 minor GC 断言。三目标各五项、十变体、五十八进程均通过，修正分批提交，生产代码不变。

M43 生命周期失败用例的末尾 try 还保留了多余 Unit 语句，与语言规范 8.1.2 冲突。仅删除该行，保留失败阶段与 finally 次数断言；Darwin／GNU 的 debug、release 及正常、moving GC 均通过。

两项旧 native-link 用例的 C constructor 拒绝预期改为实际初始化运行，并保留其他链接错误与失败不发布检查。Darwin 直接对象和归档、GNU／musl 归档路径全部通过。

接口私有支持正文修复在两机各通过 120 项类型／产物测试和 9 项接口测试。Darwin／GNU 各通过相关 CLI 17 项、18 变体、95 进程；musl 的接口与 suspend 支持补验为 2 项、3 变体、14 进程。GNU 完整执行的八项失败均已闭合，另复查十六项相关用例。

## 验收边界与清理

文件执行中取消等待真实 syscall 和 after-work，因而可能延迟作用域退出；TLS／HTTP 等后续能力不属于 M49。网络测试不依赖公网服务。原始日志、报告和测试工作目录保存在仓库外 `~/.cache/scoop-m49`。

本机先将约 103 GB target 清至 1.3 GB，后又清理 27.2 GiB 结束的旧 debug 数据；Linux 开始完整验收前用 Cargo 清理 55.5 GiB。最终另清理本机 3.2 GiB、Linux 2.6 GiB debug 数据；target 分别保留约 1.3 GB 和 792 MB 的 release 工具。用户原有 M47 文档改动未纳入本任务提交。
