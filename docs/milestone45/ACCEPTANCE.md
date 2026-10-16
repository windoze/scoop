# M45 验收记录

状态：2026-10-16 已完成 Darwin/AArch64、Linux GNU/amd64 与 Linux musl/amd64 验收。M38 完成后按 M45-1～10 顺序实施并逐功能提交；逐批实现、前置修复及失败闭合情况见 [PROGRESS.md](PROGRESS.md)。

## 交付范围

core 提供普通 Disposable、普通／挂起 use、保留原始双失败的 ResourceUseException 和允许 no-op 的 gcAdjustPressure。scoop.platform 作为普通 Cone，提供无损系统字符串／路径、系统错误、NativeByteBuffer、同步字节／文本 IO、文件／目录、原始环境、时钟／熵、线程／同步、进程／管道及阻塞 DNS／TCP／UDP。

本里程碑还修复了真实调用暴露的泛型回调推断、Throwable 身份、Unit lambda 尾值推断和安全调用语句。没有库名特判、临时强制类型或额外 runtime 子系统；异常身份合同使 runtime contract 升为 13，旧产物按既有版本规则重建。

## 验证范围与结果

| 范围 | Darwin/AArch64 | Linux GNU/amd64 | Linux musl/amd64 |
| --- | --- | --- | --- |
| 平台库行为（debug／release／debug moving GC） | 各 99 项通过 | 各 99 项通过 | 各 99 项通过 |
| M45、M38、GC、异常正式 CLI 组合 | 335 通过、7 项目标不适用 | 337 通过、5 项目标不适用 | 342 全部通过 |
| 格式、lint、native 严格告警 | 通过 | 通过 | 通过 |
| fixture runner 公共规则 | 50 项通过 | 50 项通过 | 同一宿主 |

最终 CLI 组合从现有 4181 项用例中选择 342 项：Darwin 共 429 variants／1978 processes，GNU 共 433／1990，musl 共 440／2034；每个目标各有 3 项既有结构 golden，无失败。Linux 两组同时运行，分别用时 2208.66／2210.61 秒。新增应用在 debug/release、普通／moving GC、独立链接及缓存复用全部通过。

Darwin 的编译器／runtime 修复按影响完成回归：最后的 HIR lowering 全组 1474 项通过；异常身份修复涉及的 MIR／LIR、lowering、codegen/runtime 与 slib 共 1732 项通过；pressure 入口涉及 toolchain 32 项通过。后续普通库功能与测试修改复用这些未受影响的结果。

Linux 在所有本机修改稳定后集中验证。全量 Rust 初轮 5349 通过、1 项缓存键固定向量期望失败，零忽略；核对 runtime ABI 12→13 后更新期望，两机均只补验该 1 项并通过，当前 5350 项全部闭合。Linux fmt／整仓 clippy、GNU／musl native 严格告警及 50 项 runner 规则均通过。

正常行为通过 scoop.test 验证，窄 native fixture 覆盖实际系统调用的失败回滚、部分进度、NativeSafe、owner 保活、release hook 与信号合同。网络使用 loopback 和动态端口；连接拒绝通过 native connect 边界注入，不依赖主机防火墙对未监听端口的行为。

下游应用从普通 platform `.slib` 完成 NativeByteBuffer → 文件 → 子进程管道 → TCP → 文件的数据传输，覆盖两种编译配置、moving GC、独立链接及实际构建复用。所有源码与工具始终存在；生产库只依赖 core／std，测试依赖没有进入生产闭包。

Linux musl 预检发现路径 shim 多余引用 `linux/fs.h`；所需 rename flag 已由 libc `stdio.h` 提供，删除该 include 后 GNU／musl 严格编译通过。该修正不改变行为或 ABI；Darwin 分支不受影响，未重复本机整批验收。

musl 平台库三种配置初轮均为 98/99，唯一失败是 DNS 测试假定 `localhost` 同时具有两种地址族，而本机 hosts 只为它配置 IPv4。测试改为按实际主机配置解析名称，并用数字 loopback 地址分别验证 IPv4／IPv6 和端口。只修改此用例，随后三目标的三种配置均补验该 1 项并通过；其余用例复用同批通过结果，没有未闭合失败。

## 记录与清理

原始日志、逐次 JSON 和临时诊断文件均保留在仓库外，本文只记录结果概要。优先清理已经结束的 fixture 工作目录，保留近期复用的 Cargo 和 Scoop 编译缓存。

本轮清理本机约 17.3 GiB、Linux 约 33.4 GiB 的已结束 fixture 工作目录；两机 `target/tmp` 均已清空。Cargo 增量目录、Scoop 缓存及仍在使用的 sysroot 保留。
