# HTTP server 实施记录

完整范围见 [DESIGN.md](DESIGN.md)，公共合同见 [server 规范](../specs/SCOOP-HTTP-SERVER-SPEC.md)。本记录只保存范围、结果和待闭合事项，不保存原始日志。

## 当前状态

- 已完成阶段 A：路由、请求 Context、中间件与结果执行。
- 已完成阶段 B：H1.0／H1.1、TLS／ALPN、字段／Cookie／JSON／form、Continue、body 限制、连接／请求容量、总时限与 I/O idle、停机和完成通知。
- 已完成阶段 C：H2 明文 prior knowledge／TLS ALPN、每流请求与取消、GOAWAY 排空、全局请求容量、首部／空闲时限和正常终结。
- 后续仍需 D1～D7 的七项能力。路由级 deadline 覆盖随长流式端点补齐；当前 requestDeadline 是 server 级配置。服务端 mTLS／动态 SNI、HTTP/3 不在本次范围内。
- 每个功能验证后独立提交；本机采用定向 release Cone 测试，避免重复全量测试。
- Linux 工作目录为 `nuc12:~/repos/scoop-http-server-work`；最终串行运行一组最小库验证，不跑完整套件或大规模性能测试。
- 本机辅助工作及原始日志位于 `~/scoop-work/http-server`，所有命令的临时目录显式放在其 scratch 下，不使用系统 `/tmp`。
- 2026-10-24 清理旧 `target/fixture-cache` 和 `target/debug/incremental`，保留 release 命令及开发 sysroot。
- H1 验证后清理未使用的 `target/debug` 和已验证的 HTTP 消息测试产物，`target` 降至约 5 GiB；保留当前 server cache、release 命令和预编译 sysroot。

## 已验证

- 阶段 A：本机 release Cone 测试 18／18 通过；覆盖路由优先级与分组、HEAD／OPTIONS、单次 body、短写、提交回调、映射失败、两个应用的共享状态隔离及 Context 恢复。
- `scoop doc --no-deps --deny-doc-warnings` 通过，公开 API 文档可生成；源码检查及 `git diff --check` 通过。
- 字段与结构化提取：新增 4 个定向 release 测试全部通过，覆盖重复字段／Cookie、删除范围、媒体类型、codec 错误分类、form 加号和未支持内容编码；API 文档检查通过。
- HTTP Expect 消息工具：完整解析同时区分 Continue 和未知要求，5 个定向 release 测试通过，原布尔接口复用同一解析；API 文档检查通过。
- H1 接入：本机 release Cone 测试 36／36 通过；新增真实动态端口 TCP／TLS 覆盖请求隔离、HTTP/1.0 EOF、HEAD／trailer、Continue／417／提前拒绝、半关闭、容量、超时、优雅／强制／并发关闭和接管前失败。API 文档检查通过。
- 已发布本机 `scoop.http.server.slib`，并更新因 HTTP 消息工具变化而受影响的 HTTP／HTTP asyncio／HTTP client 开发产物；不修改客户端源码。
- 独立 `consumer-tests` Cone 消费发布的 `.slib`，通过公开 API 完成真实 JSON 请求、path 参数、Cookie 和响应；1／1 release 测试通过。该 Cone 单独运行，避免父 Cone 重复收集同一测试。
- H2 底层首部观测：新增 RequestHeadStarted／RequestHeadReceived，HTTP 定向测试 4／4、异步通知测试 1／1 通过，两个 Cone 的 API 文档检查通过。覆盖分段帧头、HPACK／CONTINUATION、非法首部和通知先于接受交付；响应／trailer 不重复开始；reset 后被后端忽略的迟到 trailer 也不启动计时，后续请求仍可处理。
- H2 server：新增 15 项定向网络／队列验证，并与原有 H1／TLS／应用测试一起通过 51／51 release 回归。覆盖 Context 隔离、Continue／HEAD／trailer、真实小窗口背压、提前 reset、producer 失败、容量、首部／空闲／请求 deadline、GOAWAY 已排队请求及宽限届满的悬挂清理。
- 强制 close 明确跳过 GOAWAY；调整后 3／3 定向关闭测试通过，其中新增并发强制关闭等待清理用例。server Cone 当前共有 52 个测试；没有为该局部调整重跑全部用例。
- H2 API 文档检查通过，源码按协议选择、活动状态、观测等待、请求与 I/O 拆分；新增生产文件均不足 100 行。HTTP／HTTP asyncio／HTTP client／server 开发产物已更新。
- 独立预编译 consumer 的 H1／H2 2／2 release 测试通过；H2 经公开选项接入、执行流式响应并完成优雅停机；强制关闭调整后重新发布，H2 consumer 定向复验 1／1 通过。
- 清理已验证的底层 HTTP／HTTP asyncio 测试产物及旧可执行缓存约 1.8 GiB，保留 release 工具、当前 server／consumer 程序和编译缓存。
- Linux 尚未验证；在完整库功能就绪后串行运行最小验证集合。
