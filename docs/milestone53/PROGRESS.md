# M53 实施与验收

状态：已完成，2026-10-18。公共合同见 [HTTP 规范](../specs/SCOOP-HTTP-SPEC.md)，范围与后端选择见[设计](DESIGN.md)。M52 前置已闭合，详见 [M52 验收](../milestone52/PROGRESS.md)；M47 延后，不实现或等待 generator。

## 已交付功能

| 组成 | 实际能力 |
| --- | --- |
| 固定后端与资源 | llhttp 9.4.3 发布 C 源码保持原样；nghttp2 1.70.0 构建完整静态核心，排除应用、工具、测试、文档与 HTTP/3 应用目标。三个目标各自交付匹配的公开头和 archive。 |
| HTTP/1 内存编解码 | 有序字节字段、重复值、增量输入、连续消息、请求方法关联、信息响应、HEAD／CONNECT／Upgrade、定长／chunked／EOF body、独立 trailer、严格发送及公开接收兼容选项。 |
| HTTP/2 内存会话 | 请求／响应、HPACK、never-index、并发流、双向 body、信息响应、trailer、SETTINGS、RST_STREAM、GOAWAY、实际输出进度和手工窗口归还。回调不保存 managed 引用、不调用 Scoop 或挂起。 |
| 协商与移交 | 实际 ALPN 字节选择、显式无 ALPN 回退、分段明文前言判别、h2c 接受／拒绝、stream 1／3 与 HEAD 关联；切换前缀和同次读取后缀完整保留，不重复交付原请求 body。 |
| 异步 HTTP/1 | 接管已有字节流、scope 子任务、按方向串行收发、短传输、流水线、body 提前关闭／取消、并发 close、异常组合，以及 CONNECT／101 后的传输移交。 |
| 异步 HTTP/2 | 按流关联的双向 body 与响应、背压、流级 flush、独立 GOAWAY、单流取消、连接取消与关闭；共享 TLS I/O 使用连接所有者 Job，单流取消后其他流继续收发。 |

原始字段与 body 采用自有快照；native 临时指针不跨挂起保存。应用消费 DATA 后归还窗口，零长度操作不推进网络；关闭等待实际 I/O 与借用结束。正常关闭流中已完成的响应与 body 可在连接结束后读取，后续同批输入的连接错误不丢弃已完成响应。通知在锁外交付，收尾解除不再需要的 Job 监听和 continuation。

实现按功能分别提交：后端资源 `3f14892e1`、HTTP/1 内存 `2e7c05db5`、HTTP/2 内存 `cee1ff01e`、协商 `1d2891f99`、异步 HTTP/1 `c9df15d0d`、待输出查询 `84861e881`、异步 HTTP/2 `96ee4cb9f`、Linux 接入与发行说明 `8370b6f9f`。异步库按连接、流、I/O 和关闭职责拆分，其生产 Scoop 源文件最长 126 行；上游生成 C 不作机械拆分。

## 三目标验收

下表均覆盖 debug、release 与 moving GC；库测试列分别为两档通过项数。

| 目标 | 内存 HTTP 库 debug／release | 异步库 debug／release | HTTP 独立消费者 |
| --- | --- | --- | --- |
| Darwin / AArch64 | 50／50 | 26／26 | 6 组、12 变体、36 进程 |
| Linux / GNU amd64 | 50／50 | 26／26 | 6 组、12 变体、36 进程 |
| Linux / musl amd64 | 50／50 | 26／26 | 6 组、12 变体、36 进程 |

- 库测试合计 456 次通过。Darwin 按功能批次与定向复验合并，覆盖集合一致且无剩余失败；Linux 的全部测试按类分组运行，各类复用同一已编译测试目标，减少 moving GC 的累计开销。
- 独立消费者包括原生后端、HTTP/1、HTTP/2、协商和两种异步协议。HTTP/2 与独立原生 nghttp2 客户端／服务端双向互操作；异步消费者发送 70,000 字节 body 穿过默认流控窗口。生产 `.slib` 的 cold 构建仅编译用户 Cone，warm 无 child 编译调用。
- 组合用例覆盖逐字节／三字节输入、短写、非 UTF-8 与重复字段、每项公开兼容开关、字段及伪头错误、SETTINGS／HPACK／CONTINUATION 连接错误、流与连接窗口、取消和并发关闭。实际 TLS 覆盖 H2、HTTP/1.1 ALPN 与显式无 ALPN 回退，以及跨线程取消单流后另一流继续通信。
- 默认 native 根、OpenSSL 与 nghttp2 子目录的更新／覆盖及两个 link 入口在三目标通过。两个 Linux 目标各与 HTTP 消费者一起运行 7 组 fixture，共 15 变体、78 进程。GNU／musl 的 nghttp2 首次构建、重复调用直接复用及 12 个普通 Cone 与 runtime 准备均通过。
- 本机与 Linux 的 Rust 格式／workspace lint 通过；GNU toolchain 单测 36 项通过。自身 C 格式及严格编译检查、Python Ruff 通过。M52 已闭合的 workspace 与 GNU 全量 CLI 回归继续沿用，本轮运行全部新增 HTTP 功能及相关链接回归，不重复无关全量。

## 失败闭合与边界

- llhttp 的有意 fallthrough 注释在单独预处理后被移除，GCC 曾在 `-Werror` 下拒绝编译。HTTP Cone 与 native fixture 明确关闭该项告警，上游源码不变；三目标实际构建与消费者复验通过。
- 异步流的 reset 诊断曾被关闭状态覆盖，已调整错误优先级；取消测试的线程 join 次序已修正。语法／类型推断及测试预期差异均经定向复验闭合，无剩余失败。
- HTTP/1 收发限于 llhttp 9.4.3 已知方法集合，`CUSTOM` 等扩展方法明确诊断；宽容选项不改变此限制。不公开存在 NUL 不前进路径的 `header_value_relaxed`。非最终 chunked Transfer-Encoding、204 的 Content-Length 过滤及非法 h2c SETTINGS 数值产生 GOAWAY 等行为按固定后端实际合同记录，不另写接收校验补偿。
- 协议库不负责 TCP 建连、TLS 握手、连接池、自动重试、Cookie、路由或 HTTP/3，不做函数级裁剪。

## 交付与工作目录

两个 HTTP Cone 和 nghttp2 已纳入既有目标资源与发行清单，预编译 Cone 总数为 12。准备、后端构建和打包继续由 `scripts` 提供；`scoop` 与 driver 没有增加一次性维护子命令。

Darwin 与 GNU 宿主的当前工具包已手工组装、归档并解压到带空格的新位置。三个工具版本一致；三目标的空缓存 HTTP/2 编译、warm 复用、70,000 字节 body、两个 link 入口及 moving GC 运行均通过。GNU 包同时交付并验证 musl 目标资源；手工验证不进入常规测试或 CI。

原始日志、JSON 报告、缓存和生成物保存在外置盘原仓库已忽略的 `tmp/m53`，不提交。旧 `~/.cache/scoop-m53` 和工作树 `tmp` 均指向该位置。已结束的本机 fixture 生成目录从约 40 GB 清理到 2.6 GB，保留报告；Linux 的旧 M52 生成目录也已清理。本轮 fixture 与手工组装的重复生成物完成验收后清理，保留归档、安装目录、当前工具与必要缓存，便于后续使用。
