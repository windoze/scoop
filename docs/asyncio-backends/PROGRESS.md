# EventLoop 后端实施记录

设计与合同见 [DESIGN.md](DESIGN.md) 和 [asyncio 规范](../specs/SCOOP-ASYNCIO-SPEC.md)。

## 进度

- 2026-10-27：开始实施，保留现有公共行为，以共享 owner、Darwin、宿主接入、Linux、消费者迁移的顺序提交。
- 共享层已完成：资源、操作和连接竞速只依赖 `LoopOwner`；一次非阻塞处理消费有限快照并阻止递归进入，同步运行复用该流程。native 公共头不再暴露 uv 类型，过渡适配的布局集中在私有头。
- Darwin 原生后端已完成：通知 pipe、串行 I/O queue、source 与后台 work 覆盖全部现有操作，错误保留 POSIX 来源。source 取消处理器与 worker 退出均计入关闭条件。
- `PlatformEventLoop`、`EventLoopSession` 和宿主协议已交付；Darwin 的 `MainDispatchHost` 使用真实主 dispatch queue。应用投递经根 continuation 恢复 attach Context，监听注销完成后才销毁后端和通知关闭。
- Linux 原生后端已完成：固定 liburing 2.15、eventfd、真实 ring 数据操作、独立取消 CQE、有限 native worker 与完整退休条件。仓库其余消费者和发行资源正在迁移，旧 adapter 随该批移除。

## 验证约定

普通行为与真实 native 宿主测试均使用所属 Cone 的 `scoop.test`，不新增 fixture。每批先格式化与检查，再运行影响范围内的 release 测试；完成平台替换后再运行相关组合测试。原始日志保存在仓库外，不使用 `/tmp`。

Linux 远端原仓库存在未提交工作，测试使用独立源目录及目标输出，不覆盖该工作区。

## 已完成验证

- 共享层：Darwin release，`scoop check --tests` 与受影响 C 的严格编译检查通过；asyncio 55 个容器、135 项测试全部通过，含分轮投递及嵌套处理两项新增测试。复用了同一测试构建，未运行全仓测试。
- 验证前重建了与现有 sysroot 配套的 release 命令，解决旧本地命令的 ABI 版本不匹配。未修改 ABI。
- 首次清理使用 `cargo clean --profile dev`，仓库 `target` 从约 21 GB 降至 8.9 GB；配套命令重建后约 9.4 GB。

- Darwin 后端：严格 C 检查与 `scoop check --tests` 通过；56 个容器、138 项 release 测试覆盖原有行为及真实 PTY 的字节、resize、超时、取消、挂断和 raw 模式恢复。首次运行仅两项新增恢复断言失败，已核实并排除 Darwin 内核设置的 PENDIN 暂态位，TTY 三项重测通过。

- 宿主接口：Darwin release 58 个容器、145 项全部通过。新增真实 poll fd-watch 与 CFRunLoop 覆盖首次回调前关闭、外线程投递／关闭、完整 Context、根异常／子任务 finally、嵌套通知、安装失败回滚、注销后 fd 关闭及 onClosed 异常捕获。

- 底层 TTY：`88cf4483c` 修复会话首进程意外取得控制终端的问题；规范先明确所有权，再加 `O_NOCTTY`。真实子进程／PTY 回归在三目标各通过 4 项。musl 测试同时初始化 termios 的 libc 扩展槽，避免比较内核未填写的字节。
- Linux 后端与宿主进度：严格 GNU／musl C 检查、三目标 `scoop check --tests` 通过。Darwin 150 项；Linux 当前 7.0 内核 GNU／musl 各 153 项；独立 KVM 中 Linux 6.1.190 基线 GNU／musl 各 153 项全部通过。包含真实 target／cancel CQE 两种退休顺序、SQ 满、seccomp 禁用 setup／submit、缺能力、内核读完成后的取消进度、空闲无回调、持续 UDP 下宿主推进和 GC 后外线程关闭。
- 首次 6.1 GNU 仅旧 worker 排队读断言失败：已将该验证保留为 Darwin 专项，Linux 以真实数据 CQE 断言实际进度；普通 open／DNS 排队取消仍为共享测试。修正后两种 libc 基线全部闭合。
- 第二次清理使用 `scoop clean` 清除本任务不再使用的 `target/http-server` 输出，仓库 `target` 从约 9.4 GB 降至 1.7 GB；Linux 只使用独立的约 1.2 GB Cargo 输出，未触碰原远端工作树及其 target。
