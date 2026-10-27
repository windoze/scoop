# 安装包布局与编辑器扩展

完整构建入口为 `scripts/package_toolchain.py`。工具包现以安装根内容为归档顶层，可直接解压到 `~/.scoop`，不再包含额外版本目录。安装根新增 `VERSION`，记录发行版本、源码 revision 与宿主平台；`tooling/` 包含同次构建的 VS Code 扩展 VSIX。Darwin 包交付 Darwin/AArch64 资源，Linux 包交付 GNU／musl 两套资源；系统库默认从 `lib/<target-triple>/*.slib` 读取，空缓存及 debug 应用不会触发系统库重编。

VS Code 扩展接入 `scoop-lsp`，提供诊断、补全、签名、导航、符号、引用、重命名与快速修复。默认启动 `~/.scoop/bin/scoop-lsp`；Tree-sitter 继续提供高亮、折叠和缩进。

## Scoop 编程 skill

工具包新增 `tooling/scoop-programming/`，面向 AI 提供 `scoop example` 检索、完整示例文件还原、语言差异、库 API 查找、Cone 依赖和 CLI 验证工作流。`share/scoop/toolchain.toml` 的 `programming_skill` 字段标明目录位置。skill 复用当前安装中的示例、文档和源码，随 dist 分发，不自动安装到 AI 工具或修改用户配置。

## PCRE2 正则表达式库

新增普通 Cone `scoop.regex`，基于 PCRE2-8 10.49 提供编译模式、编号／命名捕获、惰性搜索、字面／模板／回调替换及分割。公开位置使用 Long Unicode 标量索引，结果为独立快照，Regex 使用 close/use 管理 native 生命周期。PCRE2 采用独立 C 静态库和可选 JIT，不增加 Rust staticlib；Darwin、GNU、musl 目标资源、头文件及 PCRE2/SLJIT 许可随包交付。用法见 `src/scoop.regex/README.md`。

## HTTP server 库

新增纯 Scoop 的 `scoop.http.server`，提供显式路由、请求 Context、middleware、结果执行及 H1／H2／TLS 接入。公开便利能力包括 SSE、multipart 上传、query DTO 与静态 extractor、协商响应压缩、有限请求解压、条件文件／单 Range 响应，以及 HTTP/1.1 WebSocket／WSS 和 permessage-deflate。请求与会话沿结构化任务完成取消、排空及资源清理；WebSocket 在 HTTP 管线清理后启动独立会话。服务端 mTLS／动态 SNI、HTTP/3 和 H2 WebSocket 不在本次范围。

该 Cone 由现有准备、打包与文档流程自动发现，源码与目标 `.slib` 随正常系统库布局提供。入口和公开 API consumer 见 `src/scoop.http.server/README.md`；本次跨平台验证范围见仓库内的 `docs/http-server/PROGRESS.md`。

## M60 API 文档与内置库文档交付

新增 `scoop doc`，支持多 Cone、完整类型检查、声明与类型链接、继承／扩展导航、静态搜索及按输入变化复用。源代码直接嵌入 HTML；`.slib` 不增加完整源码或注释正文。渲染输入缺少源码文本时省略对应的源码展示和链接。

发行包的所有生产内置 Cone 统一提供声明说明，包含此前新增的 `scoop.terminal` 和 `scoop.terminal.asyncio`。打包流程用配套工具为各发行目标生成文档，并在全部生成成功后归档。安装根的 `docs/index.html` 可进入 Darwin 或 GNU／musl 的完整离线站点。

## M54 统一 HTTP client 与配套基础库

本期提供 Darwin/AArch64、Linux/GNU amd64 两类宿主工具包，覆盖 Darwin、GNU、musl 三个既有目标。宿主包包含 `scoop`、`scoopc`、`scoop-link`、`scoop-lsp`，以及 LLVM 22.1.8 的所需动态依赖。M47 延后，不附带 generator。

统一布局包含十七个内置 Cone 的源码、预编译产物、公开 C 头与 runtime 对象。本期新增 `scoop.http.client`、`scoop.idna`、`scoop.url`、`scoop.charset` 和单个 `scoop.compression` Cone。OpenSSL、nghttp2、文本、URL、压缩后端的静态库和 Linux libunwind 随目标资源交付；Linux 的 liburing、llhttp 与 Ryu 编入实际使用它们的 Cone／runtime；Darwin 的 asyncio 使用系统 libdispatch。LLVM 原始源码归档、补丁和许可证另存于同版本的 `llvm-sources` 包。

异步 I/O 的默认构造入口由 `LibuvEventLoop()` 改为 `PlatformEventLoop()`，不保留旧别名。Linux asyncio 需要内核 6.1 及可用的 io_uring，初始化探测失败会明确报错；该要求不提高其他库的最低系统版本。Darwin 提供 `MainDispatchHost`，两平台均支持通过 `EventLoopHost` 接入宿主主循环。

运行时契约更新为 14，metadata ABI 更新为 9。本期工具配套新的目标 runtime 与预编译库；旧开发 sysroot 需要通过既有准备脚本重新生成。

纯 Scoop 的 `scoop.http.client` 在既有 HTTP、TLS 和 asyncio 库之上统一 HTTP/1.x／HTTP/2 与 ALPN，提供连接池、流式上传下载、完整生命周期超时、取消和 suspend 回调。请求策略包括重定向、重试、Basic／Bearer／Digest、mTLS、HTTP／HTTPS／SOCKS5 代理、Cookie 和私有内存／磁盘缓存；同时提供 multipart、断点续传、SSE、HTTP/1.1 WebSocket／WSS、permessage-deflate 与可观察性接口。URL、协议、压缩、TLS 和系统 I/O 能力由各自所属库提供。

`scoop.idna` 使用 Rust idna／ICU4X，IDNA 数据基线为 Unicode 17.0.0，采用 UTS #46 + STD3 + CheckHyphens，接受其符号与 ContextO 行为。NFC 共用同一 ICU4X normalizer。字符集转换已改为 encoding_rs 0.8.42，提供 43 个名称，退役名称及映射差异见 ENCODINGS.md；readline 的 Unicode 操作也使用 ICU4X／unicode-width，原 GNU 文本链的源码、补丁、归档和头文件不再进入发行包。`scoop.url` 使用 uriparser，提供 RFC 3986 URL／IRI 解析、拼装、相对引用和网络 origin，不包含浏览器全部 WHATWG 容错。`scoop.charset` 提供固定编码集合的内存及增量转换；`scoop.compression` 在同一 Cone 中提供 zstd、gzip、zlib／raw DEFLATE 和 Brotli 的内存、同步流与异步流。

HTTP/1 固定使用 llhttp 9.4.3。发送与客户端响应关联支持任意合法方法 token，服务端请求解析仍限于 llhttp 的已知方法集合。HTTP/2 固定使用 nghttp2 1.70.0 的完整静态核心；协议库保留独立使用已有传输的接口，client 负责连接选择、池与请求策略。

HTTP/3、HTTP/2 WebSocket，以及跨 origin H2 合并、证书固定／对端链检查接口、TLS 会话恢复控制／0-RTT、主动 H2 PING 保活留待后续。

仓库中的准备、LLVM 构建和打包脚本供维护者显式调用；常规 CLI 测试和 CI 不自动运行打包，也不要求先生成归档。M54 功能实现与三目标验收已完成，库测试、正式消费者、复用范围和失败闭合见仓库的 M54 验收记录。此包的部署配置、源码 revision 与实际组件列表见 `toolchain.toml`。

项目维护者尚未选定 Scoop 自身的 LICENSE 时，包只用于当前本地构建和功能验收；此说明不授予新的对外分发许可。已有第三方许可随组件保留。
