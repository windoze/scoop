# M54 实施进度

状态：功能实现完成，GNU／musl 最终验收仍在进行。完整范围见[设计](DESIGN.md)，本页记录当前实现、实际验证和未完成项；运行中的批次不记为已通过。

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
| GNU 库与普通消费者 | 其他 10 个相关库的 debug／release 已通过；client 库验收尚待完成。剩余普通 fixture 正在逐项运行，已完成结果保留 |
| musl 库与普通消费者 | 其他 10 个相关库的 debug／release 已通过；client 库验收尚待完成。剩余普通 fixture 正在逐项运行，已完成结果保留 |
| 合并后的 Linux 集成 | 已在独立 checkout 构建三个新 release 工具，正在准备相应 SDK；随后分别验证 native 强／弱引用、Basic／H2 和 observer。原固定版本批次继续运行 |

GNU client 首轮构建被系统 OOM 终止，内核记录已确认原因。大型库测试改为串行，普通 fixture 每目标使用一个 worker；重试完成前不将该库记为通过。此前普通批次因调整资源与切换已修复 SDK 而中止，保留已通过条目，仅续跑未完成部分。

已结束的旧 target 和诊断输出按批清理，保留需要复用的工具、源码、生产 SDK、缓存及实际报告。原始日志、JSON、历史进度副本与缓存归档均位于仓库外的 `scoop-work/m54`；本页不保存逐次流水账。

## 后续工作

1. 完成 GNU／musl 剩余 client 库 profile 与普通功能消费者；修复真实失败并复验受影响范围。
2. 完成合并后 Linux SDK 和必要集成回归，保留旧批次的实际版本与结果。
3. 汇总三目标最终结果，核对设计矩阵与分发资料，再更新 milestone／roadmap 为完成。

HTTP/3、HTTP/2 WebSocket 和设计中列出的高级连接能力不属于本期。URL 保留 RFC 3986／3987 profile，IDNA 固定 Unicode 15.1；CPU codec 计算、DNS／native work 的实际终止及磁盘缓存单实例协调等边界遵守各自规范，不由 client 绕过。
