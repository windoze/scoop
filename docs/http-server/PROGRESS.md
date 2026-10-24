# HTTP server 实施记录

完整范围见 [DESIGN.md](DESIGN.md)，公共合同见 [server 规范](../specs/SCOOP-HTTP-SERVER-SPEC.md)。本记录只保存范围、结果和待闭合事项，不保存原始日志。

## 当前状态

- 已完成阶段 A：路由、请求 Context、中间件与结果执行。
- 已完成阶段 B：H1.0／H1.1、TLS／ALPN、字段／Cookie／JSON／form、Continue、body 限制、连接／请求容量、总时限与 I/O idle、停机和完成通知。
- 已完成阶段 C：H2 明文 prior knowledge／TLS ALPN、每流请求与取消、GOAWAY 排空、全局请求容量、首部／空闲时限和正常终结。
- 已完成 D1：SseMessage、增量 SseEncoder、借用 SseSink、server SSE 结果与可选心跳；显式 gzip flush 已随 D4 验证。
- 已完成 D2：form-data 单遍提取、严格元数据、part／文本／复制／discard 限制、请求内视图清理及显式文件保存；gzip multipart 组合已随 D5 验证。
- 已完成 D3：扁平 query DTO decoder、显式／companion codec、未知字段策略及静态 query／JSON extractor 适配。
- 已完成 D4：Accept-Encoding 协商、gzip／deflate／br／zstd 响应策略、元数据与 trailer 更新、流式压缩及双侧输出计数。
- 已完成 D5：请求解压策略、反向编码链、raw／decoded 唯一消费、各层实际输出限制、尾部校验与请求收尾。
- 路由级 deadline 覆盖已完成，端点／middleware 可在提交前调用 HttpContext.setDeadline 调整或关闭本次请求总时限。
- 已完成 D6：延迟文件结果、原生字节静态路径、条件短路、单区间与同句柄有限发送。
- 后续仍需实现 D7。服务端 mTLS／动态 SNI、HTTP/3 不在本次范围内。
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

- SSE 发送底层：HTTP 编码器 4／4、异步 sink 3／3 定向 release 测试通过，两个 Cone 的 API 文档检查通过。覆盖标准字节向量、逐字节 UTF-8、多行与空值、ID／retry／注释、短写、flush、原始失败及关闭等待真实 buffer borrow；不重复全量协议测试。
- 请求期限覆盖：新增 3 个用例，连同 H2 deadline、H1 生命周期和管线共 20／20 定向 release 测试通过；API 文档检查通过。验证 H1／H2 延长或关闭默认时限、从原始起点启用期限、等待取消清理及 H2 流隔离，不重跑完整库套件。
- SSE server：新增 6／6 定向 release 测试通过，期限覆盖 3／3 复验通过，API 文档检查通过。验证 H1／H2 首部与首事件提前可读、批量 flush、HEAD、Last-Event-ID、writer 寿命及并发拒绝；1 字节 H2 窗口覆盖完整事件边界、心跳背压和正常收尾。reset／deadline／停机等待订阅清理；并发完成通知按路由选择，不依赖通知顺序。
- 已发布包含期限覆盖和 SSE 的 server 开发产物，独立 H2 consumer 1／1 定向 release 验证通过；通过公开 SSE／deadline／Last-Event-ID API 发送并由 SseSource 消费。清理已验证的底层测试目录与 9 个旧可执行缓存目录，保留当前 server／consumer 及编译缓存。
- Multipart：新增 11／11 定向 release 测试通过，结构化提取 4／4 和 SSE 6／6 回归通过；API 文档检查通过。覆盖逐字节输入、重复附件／空值、原始元数据与严格文本、字符集、单次消费、旧流失效、局部及外层限制、截断、输入与 sink 错误分类；H1 分块／H2 的 32 KiB 以上上传证明外层 EOF 前不发布，reset 与实际文件写入失败均清理暂存文件。测试显式选取 scratch 或当前目录下的临时文件父目录，不使用系统临时目录回退；没有重跑全库或编译器套件。
- Multipart 请求寿命收尾后复验 11／11、管线 8／8 通过；未消费的 body 也不能在请求结束后创建新视图。已发布 server 开发产物，独立 H2 consumer 1／1 release 验证通过，使用公开 multipart API 完成表单上传。
- Query／extractor：新增 11／11 定向 release 测试及原结构化提取 4／4 回归通过，API 文档检查通过。覆盖默认值／Option／重复列表、名称解码与字面语义、未知字段和复读、数值精度／范围、输入错误路径及 cause、装配错误分类；提取失败不调用 handler，组合提取不重复消费 body，适配后仍保持 Context 与流式资源寿命。明确通用 Decoder 的格式专属未知字段策略，JSON 行为不变。
- 已发布 D3 server 开发产物，独立 H1 consumer 1／1 release 测试通过，组合公开 query／JSON extractor 与泛型 adapt。新增生产文件均少于 75 行；清理 4 个已验证的旧可执行缓存，保留当前产物和编译缓存。
- D4 协商底层：HttpAcceptEncoding 的 4／4 定向 release 测试和 API 文档检查通过。覆盖缺失／空字段、跨行重复最低质量、严格 q 语法和原始偏移、星号、identity、候选次序及零质量排除；没有重跑全部 HTTP 协议测试。
- D4 server：新增 10／10 定向 release 测试通过，管线 8／8、SSE 6／6、H2 网络 3／3 回归通过，API 文档检查通过。覆盖四种编码与短写、阈值／406、嵌套策略、最终草稿、Vary／ETag／摘要／trailer、HEAD、原文长度和失败不 finish；真实 H1／H2 流式及 SSE gzip 在 producer 结束前完成可读 flush，H2 小窗口背压与 reset 等待清理且不影响其他流。
- 已发布 HTTP／HTTP asyncio／HTTP client／server 开发产物，独立 H1 consumer 1／1 release 验证通过，组合 typed extractor 与 gzip 输出；只使用公开字段工具和解压流。新生产文件均不足 60 行。清理 5 个旧可执行缓存及已验证的 HTTP 测试产物，target 约 11 GiB。准备／打包脚本已按根 Cone 清单自动发现 server，无需新增发布机制。
- D5：新增 13／13 定向 release 测试通过；multipart 11／11、结构化提取 4／4、管线 8／8、H1 网络 4／4 回归通过，API 文档检查通过。覆盖四种编码、重复字段与双层反序、raw 选择、未变换长度、原 head 保真、编码／解码／聚合及中间层／窗口上限、连续 member／frame、空输入／截断／尾随／checksum、原始 I/O 分类和延迟 EOF。H1 编码链与声明长度拒绝均先于 Continue；H2 慢上传 reset 等待清理且不影响其他流。H1／H2 gzip multipart 在尾部校验前不发布文件，checksum 失败删除暂存文件。
- 已发布 D5 server 开发产物，独立 H2 consumer 1／1 release 测试及 API 文档检查通过，以公开请求解压和 multipart API 消费 gzip 上传。新增生产文件最长 81 行；清理 6 个旧可执行缓存，保留当前 server／consumer 和编译缓存，未运行全量套件。
- D6 条件求值底层：新增 evaluateHttpPreconditions，4／4 定向 release 测试和 API 文档检查通过。覆盖条件优先级、强／弱标签、无标签的表示存在性、方法适用性、秒精度和显式日期参考、重复标签字段与仅解析生效条件；无文件／缓存 I/O。
- D6 server：新增 13／13 定向 release 测试通过；压缩 10／10、路由 6／6、管线 8／8、H2 生命周期 4／4、请求解压生命周期 1／1 回归通过。覆盖同一打开句柄的 metadata／读取、增长封顶与截短失败、HEAD／304 不读取、最终草稿条件、单 Range／416／回退、If-Range 强校验、已编码文件、原生路径字节、单次 URL 解码、显式目录索引和普通 symlink 语义。H1／H2 真实网络验证并发文件位置及取消背压传输；H2 已取消请求保留原始 stream 异常并正确记录 Cancelled。macOS 无法创建的非 UTF-8 文件名按实际 OS 错误映射，不假定所有文件系统都接受该名字。
- D6 API 文档检查通过，发布 server 开发产物；独立文件 consumer 1／1 release 验证通过，消费公开 file／staticFiles 的范围与条件响应。新增生产文件最长 59 行；清理 5 个旧可执行缓存，保留当前 server／consumer 程序及编译缓存，未运行全量套件。
- D7 握手底层：新增类型化 UnsupportedVersion 和 negotiateResponse，2／2 新测试及握手／协商／deflate 13／13 定向 release 回归通过，API 文档检查通过。合法不支持版本与缺失／重复／非法字段区分；构造一次 101 即保留实际协商数据，server 无需再次用客户端 accept 校验自产响应。会话接入仍在实施。
