# M45 验收记录

状态：本机最终组合验收完成，Linux GNU／musl 待运行。M38 完成后按 M45-1～10 顺序实施；逐批实现、前置修复及失败闭合情况见 [PROGRESS.md](PROGRESS.md)。

## 交付范围

core 提供普通 Disposable、普通／挂起 use、保留原始双失败的 ResourceUseException 和允许 no-op 的 gcAdjustPressure。scoop.platform 作为普通 Cone，提供无损系统字符串／路径、系统错误、NativeByteBuffer、同步字节／文本 IO、文件／目录、原始环境、时钟／熵、线程／同步、进程／管道及阻塞 DNS／TCP／UDP。

本里程碑还修复了真实调用暴露的泛型回调推断、Throwable 身份、Unit lambda 尾值推断和安全调用语句。没有库名特判、临时强制类型或额外 runtime 子系统；异常身份合同使 runtime contract 升为 13，旧产物按既有版本规则重建。

## 验证范围与结果

| 范围 | Darwin/AArch64 | Linux GNU/amd64 | Linux musl/amd64 |
| --- | --- | --- | --- |
| 平台库 99 项行为测试 | debug、release、debug moving GC 全部闭合 | 待运行 | 待运行 |
| M45、M38、GC、异常正式 CLI 组合 | 335 通过、7 项目标不适用 | 待运行 | 待运行 |
| 格式、lint、native 严格告警 | 通过 | 待运行 | 待运行 |
| fixture runner 公共规则 | 50 项通过 | 待运行 | 同一宿主 |

Darwin 最终组合共 429 variants、1978 processes、3 项既有结构 golden，无失败。新增应用在 debug/release、普通／moving GC、独立链接及缓存复用全部通过。

Darwin 的编译器／runtime 修复按影响完成回归：最后的 HIR lowering 全组 1474 项通过；异常身份修复涉及的 MIR／LIR、lowering、codegen/runtime 与 slib 共 1732 项通过；pressure 入口涉及 toolchain 32 项通过。后续普通库功能与测试修改没有重复这些未受影响的 Rust 测试。Linux 在所有本机修改稳定后集中验证。

正常行为通过 scoop.test 验证，窄 native fixture 覆盖实际系统调用的失败回滚、部分进度、NativeSafe、owner 保活、release hook 与信号合同。网络使用 loopback 和动态端口；连接拒绝通过 native connect 边界注入，不依赖主机防火墙对未监听端口的行为。

下游应用从普通 platform `.slib` 完成 NativeByteBuffer → 文件 → 子进程管道 → TCP → 文件的数据传输，覆盖两种编译配置、moving GC、独立链接及实际构建复用。所有源码与工具始终存在；生产库只依赖 core／std，测试依赖没有进入生产闭包。

## 记录与清理

原始日志、逐次 JSON 和临时诊断文件均保留在仓库外，本文只记录结果概要。优先清理已经结束的 fixture 工作目录，保留近期复用的 Cargo 和 Scoop 编译缓存。
