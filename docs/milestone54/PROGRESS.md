# M54 实施进度

状态：已完成（2026-10-20）。完整范围见[设计](DESIGN.md)，本页记录实现、三目标实际验收和必要限制；历史通过记录保留原有版本与运行模式。

## 已完成的功能

| 功能组 | 交付内容 | 独立与组合覆盖入口 |
| --- | --- | --- |
| IDNA／URL | 独立 `scoop.idna`、`scoop.url`；IDNA2008／UTS #46、Punycode、URI／IRI、builder、相对解析、query、origin 与规范网络主机 | `m54-idna`、`m54-url`、`m54-url-network-host`；Unicode、重定向、Cookie、代理与池键组合 |
| 字符集／压缩 | 独立 `scoop.charset`；严格／有损增量转换、BOM 与 NFC。单一 `scoop.compression` Cone 提供五种格式、内存／同步／异步流、zstd 字典和 RFC 7692 消息 | `m54-charset`、`m54-nfc-authentication`、`m54-compression*`、`m54-deflate-messages`；标准向量及独立 codec 对照 |
| HTTP 消息工具 | 扩展方法、字段、媒体类型、文件名、日期、Range、认证、缓存元数据、Cookie／PSL、multipart、SSE 与 WebSocket | `m54-http-*`、`m54-content-disposition`、`m54-multipart*`、`m54-public-suffix`、`m54-sse-*`、`m54-websocket-*` |
| HTTP 连接事实 | H1 复用／早期响应／取消；H2 设置、流输出与输入进度、GOAWAY 排空、未发送拒绝、空正文结束与首响应字节时间 | `m54-http1-*`、`m54-http2-*`、`m54-http-idle`、`m54-http-response-timing`；独立 socket／TLS／帧对端 |
| crypto／TLS | HTTP 所需摘要、多证书 bundle、显式／系统信任、异步验证、mTLS 与本地身份摘要 | `m54-protocol-digests`、`m54-certificate-bundle`、`m54-tls-*`；TLS 1.2／1.3、代理 TLS 与目标 TLS 组合 |
| I/O 与清理 | 异步文件查询／控制／临时创建／替换／删除／目录遍历、TCP 竞速与阶段通知、SOCKS5、IP CIDR、native buffer 快照、可等待真实完成的清理 | `m54-file-control`、`m54-path-modify`、`m54-temporary-file`、`m54-directory`、`m54-tcp-*`、`m54-socks5`、`m54-ip-network`、`m54-cleanup-files` |
| 统一 client | 纯 Scoop 请求／响应、HTTP/1.x／HTTP/2／ALPN、连接池、背压、Continue、duplex、trailer、自动内容解码和完整生命周期超时／取消 | `m54-http-client`、`m54-request-*`、`m54-client-pool`、`m54-client-continue`、`m54-client-response`、`m54-client-timeouts`、`m54-client-idle` |
| 策略与认证 | 有限重定向／重试、Basic／Bearer／Digest、suspend 凭据与刷新、显式代理／代理认证、环境代理与绕过规则 | `m54-client-redirects`、`m54-client-retries`、`m54-client-basic`、`m54-client-bearer*`、`m54-client-digest*`、`m54-client-proxy*`、`m54-client-environment` |
| Cookie／私有缓存 | suspend 存储接口、内存 Cookie jar、PSL／SameSite、私有内存／磁盘缓存、Vary／304、身份隔离与完整后提交 | `m54-cookie-sites`、`m54-client-cookies*`、`m54-client-cache*`；真实网络、并发存储和回调失败／取消 |
| 下载／续传 | 显式目标 Path、异步临时文件、进度与原子提交；强验证条件、原始表示、200／206／416、多段截止及部分文件保留 | `m54-client-download*`、`m54-client-resume*`、`m54-http-eof`；独立字节对照、H2、认证、缓存和清理组合 |
| 长连接 | SSE over H1／H2、ID／retry／显式重连；HTTP/1.1 WS／WSS、分片／控制帧、permessage-deflate、握手拒绝和会话关闭 | `m54-client-sse*`、`m54-client-websocket*`、`m54-websocket-lifetime`；认证、代理、H2 共存及真实来源子树清理 |
| 拦截与观察 | 有序 suspend 拦截器、逐次尝试回调、实际阶段与首字节时间、累计进度、脱敏摘要；H2 驱动不运行用户挂起回调 | `m54-client-interceptors*`、`m54-client-observer*`；Context、并发、取消、回调失败和资源交付 |

库入口说明位于 [client](../../sysroot/src/scoop.http.client/README.md)、[compression](../../sysroot/src/scoop.compression/README.md)、[URL](../../sysroot/src/scoop.url/README.md)、[IDNA](../../sysroot/src/scoop.idna/README.md) 和 [charset](../../sysroot/src/scoop.charset/README.md)。client 与压缩说明中的源码示例已在本机使用生产 `.slib` 编译，示例编译不作为网络运行验收。

client manifest 没有 native 构建项。URL／IDNA、压缩、字符集、TLS／crypto、协议与平台 I/O 的缺口均由所属库公共 API 承担；同步与异步压缩共用同一 Cone 和后端。准备与分发沿用既有入口，当前为 17 个内置 Cone，包含目标静态库、头文件、源码和许可资料。

## 所属层修复

- 编译器检查 FFI 类型与 ABI，并负责自己生成的辅助调用；产物保留真实 native 未定义符号、重定位和链接库，最终由 linker 解析。普通 native 符号不再逐项进入手写登记表，ELF 弱引用保留平台规则。
- 已修复 Boolean `zeroext` ABI、ELF 只读常量／协程跳转表、字符串及 native 辅助引用、局部表和构造捕获等真实消费者问题。typed ABI／产物测试与 `m54-boolean-abi`、`m54-native-*`、`m54-coroutine-jump-tables` 等正式消费者覆盖这些边界。
- MIR 保留挂起调用异常后继需要的旧值；LLVM safepoint、statepoint 与根恢复标记保持不可复制。四个 GC 正确性 fixture 专门覆盖分支加载、guarded unwrap、协程异常活跃值和 site 不可复制，不用于普通功能压力验收。
- core 的作用域失败边界与 `nonCancellableScope`、协议取消、来源关闭和真实 native borrow 收尾已分别修复并验证。client 的缓存、下载、SSE、WebSocket、拦截器与 observer 保留主失败、清理异常和实际子树完成。
- HTTP/2 在 GOAWAY 排空后丢弃待发 SETTINGS ACK 时，不再把 nghttp2 的 `SESSION_CLOSING` 误报为连接失败。旧 SDK 的确定性失败已复现，修复仅位于 HTTP 适配层；独立 ACK、异步排空、未发送拒绝、连接池和重试五项组合在 Darwin／GNU／musl 均已通过。
- client 库编译采样定位到 LIR 符号与 ODR 定义的重复线性扫描；已改用已有排序、共享 callable 索引及 typed entity／role 反查索引，同时修复删除 atom 后 foundation 索引仍返回旧记录的问题。删除 ODR member 后同步更新索引，保持 Strong 优先、首个匹配项、key／linkage、定义顺序和 wire；删除后查询的独立回归在旧实现确定失败。
- codegen 的每个已生成 callable 计划原先保留一份整库 GC effect 表，导致大库编译持续累积副本。现共享这份只读表，各函数的 site 与根分组仍独立保存；不改变 GC、优化或产物合同。
- 删除 runtime 不读取的内容指纹字段及其 ELF 槽位查询、对象复制和回填流水线。实际需要的内容摘要保留在产物 metadata 中；runtime contract 更新为 14、metadata ABI 更新为 9，旧产物与开发 sysroot 必须重建。
- Shape 发射复用 symbol surface 已有的主符号索引；ELF atom 边界、可见性与 COMDAT 操作复用每个原对象的一份名称索引，消除逐 atom 的重复全表扫描。读取本对象只服务实际产物与 GC 数据，不接管普通 native 符号解析。

## 验证安排

GC stress 只用于 GC 正确性的定向测试。普通功能与完整功能验收使用正常运行模式；早期整批压力运行已停止，中止结果不计为通过。历史通过记录保留当时的源码版本和实际模式，不改称为本轮普通运行。

当前共有 130 个 M54 fixture，其中 126 个普通功能／产物消费者和 4 个 GC 专测；Darwin 适用 129 个，ELF native 弱引用另在 GNU／musl 验证。正式消费者覆盖 debug／release、实际运行和构建复用，原始报告保存在仓库外。

117 个普通预编译库消费者使用 runner 既有的批次共享缓存。两项代表用例的缓存占用由约 1010 MiB 降至 508 MiB；实际结果与无子进程的 warm 构建断言保持原要求。运行过的有效结果按实际变更范围复用，不因新增文档或合并后无关代码再跑整套。

## 实际验证结果

| 范围 | 已有证据与当前状态 |
| --- | --- |
| Darwin 功能与正式消费者 | 129 个适用 fixture 均已有通过报告，包含历史实际模式；补核对的 `m54-noncancellable-scope` 与 `m54-tls-identity-fingerprint` 使用合并后工具在普通模式通过，共 4 个变体、12 个进程 |
| main 合并后的本机工具 | `a183651fa` 合入 main `2f7411e5a`；fmt／workspace clippy、530 项相关 Rust 测试、51 项 runner 测试通过，三个 release 命令及 17 个 Cone／runtime 重建成功 |
| main 合并后的本机集成 | native 要求、Basic／H2 和 observer 三项普通 fixture 全部通过，共 6 个变体、20 个进程；保留 M54 的 native 链接与 safepoint 合同 |
| HTTP/2 ACK 修复 | GNU／musl 的 HTTP、HTTP asyncio 与 client 候选库构建成功；每目标五项普通回归全部通过，各 10 个变体、30 个进程 |
| GNU 库与普通消费者 | 130 个 M54 fixture 均已有通过报告；本轮续跑 60 项全部通过，共 120 个变体、374 个进程，并复用前批已通过的 2 项。其他 10 个相关库每种模式 636 项、client debug／release 各 164 项通过；`4505e4861` 的 ABI 9 整库 debug 再次 164 项通过 |
| musl 库与普通消费者 | 130 个 M54 fixture 均已有通过报告。普通批次 79 项通过，唯一 TCP 用例失败已修正并闭合；保留原批次 79／1 的实际报告。其他 10 个相关库每种模式 636 项、client debug／release 各 164 项通过；ABI 9 sysroot 与整库 release 完成 |
| 合并后的 Linux 集成 | 三个 release 工具及 GNU／musl 的 17 个 Cone／runtime 准备完成；native 强／弱引用、Basic／H2 和 observer 每目标四项全部通过，各 10 个变体、32 个进程 |
| LIR 查询修复 | fmt／workspace clippy、1,245 项相关 Rust 测试与 1 项 doctest 通过；observer 的 debug／release 真实网络运行及无子进程 warm 构建通过。相同消费者的 debug `.slib` 字节与链接计划完全一致 |
| Darwin client 整库与 ABI 9 消费 | 普通 debug／release 各 164 项已有通过记录；ABI 9 与 Shape 索引修复后的 debug 再次 164 项通过。native 要求、Boolean ABI、Basic／H2、observer 四项正式消费者通过，共 8 个变体、26 个进程 |
| GC effect 表共享 | 独立保留／共享回归在旧实现确定失败；LLVM 22.1 下的 workspace clippy、25 项 statepoint 与 12 项对象划分测试通过。三个 release 工具在本机与 Linux 构建成功，本机默认工具也已更新 |
| runtime metadata 与 ELF 索引 | ABI 9 的类型化身份、LIR、产物、codegen 与链接定向回归，本机 1,038 项、Linux 309 项通过；后续 ELF 索引修复的本机 18 项、Linux 30 项对象与真实链接回归通过。两端 fmt／workspace clippy 与三个 release 工具构建通过 |
| ODR 反查索引 | 本机与 Linux 的 fmt／workspace clippy 及 643／656 项 LIR／产物／对象生成回归通过，覆盖原记录与克隆、删除成员和缺失 body 的查询行为；observer 的三目标 debug／release 网络运行及 warm 构建通过。Linux 最后复验 TCP 与 observer，每目标 4 个变体、12 个进程全部通过 |
| GC 定向正确性 | 四项专测均已有三个目标的通过记录。本轮只补齐缺失的 musl 分支加载专测，debug／release 共 2 个变体、6 个进程通过；其余沿用现有记录 |

musl 的 TCP 失败来自测试假定宿主 `localhost` 同时提供 IPv4／IPv6。相同 native 解析参数的独立探针确认了 glibc／musl 的实际结果差异；用例改为连接其绑定的数值 loopback 地址，并将运行超时收紧到 30 秒，保留双线程、两种连接并发配置及流所有权断言。三个目标的 debug／release 与 warm 构建均复验通过，不改变解析库行为。最终汇总以新报告关闭这一失败，没有用旧通过记录覆盖失败。

大型 client 库编译的 OOM 已定位到每个 callable 计划重复保留整库 GC effect 表，并通过共享只读表修复。Darwin 整库对照中，debug 最大 RSS 从 74.84 降至 31.74 GiB，release 从 68.67 降至 29.84 GiB；修复后的 GNU debug／release 与 musl debug 也全部通过，峰值为 24.17～25.79 GiB。诊断中主动停止的编译不计为测试失败、新 OOM 或通过。

删除无用指纹回填并复用 Shape 索引后，本机完整 client debug 的单次耗时从 793.8 降至 621.6 秒，峰值为 29.02 GiB。Shape 索引修改前后的 ABI 9 测试 `.slib` 字节相同；该比较不跨 metadata ABI。进一步修复逐 atom 三次全表查找的 ELF 边界路径后，GNU 完整 debug 的单次耗时从 2,180.5 降至 1,266.0 秒，峰值为 22.82 GiB；该对照包含这三项修复，不把约 42% 的改善归于单一索引。musl 整库 release 164 项通过，峰值为 19.88 GiB；其运行与 GNU 普通 fixture 重叠，不作为独立性能对照。上述单次结果不作为所有程序的稳定性能承诺。

已结束的旧 target 和诊断输出按批清理，保留需要复用的工具、源码、生产 SDK、缓存及实际报告。原始日志、JSON、历史进度副本与缓存归档均位于仓库外的 `scoop-work/m54`；本页不保存逐次流水账。

## 范围边界

HTTP/3、HTTP/2 WebSocket 和设计中列出的高级连接能力不属于本期。URL 保留 RFC 3986／3987 profile，IDNA 固定 Unicode 15.1；CPU codec 计算、DNS／native work 的实际终止及磁盘缓存单实例协调等边界遵守各自规范，不由 client 绕过。
