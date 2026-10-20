# M60 API 文档与内置库文档交付

新增 `scoop doc`，支持多 Cone、完整类型检查、声明与类型链接、继承／扩展导航、静态搜索及按输入变化复用。源代码直接嵌入 HTML；`.slib` 不增加完整源码或注释正文。渲染输入缺少源码文本时省略对应的源码展示和链接。

发行包的十九个生产内置 Cone 统一提供声明说明，包含此前新增的 `scoop.terminal` 和 `scoop.terminal.asyncio`。打包流程用配套工具为各发行目标生成文档，并在全部生成成功后归档。安装根的 `docs/index.html` 可进入 Darwin 或 GNU／musl 的完整离线站点。

## M54 统一 HTTP client 与配套基础库

本期提供 Darwin/AArch64、Linux/GNU amd64 两类宿主工具包，覆盖 Darwin、GNU、musl 三个既有目标。宿主包包含 `scoop`、`scoopc`、`scoop-link`，以及 LLVM 22.1.8 的所需动态依赖。M47 延后，不附带 generator。

统一布局包含十七个内置 Cone 的源码、预编译产物、公开 C 头与 runtime 对象。本期新增 `scoop.http.client`、`scoop.idna`、`scoop.url`、`scoop.charset` 和单个 `scoop.compression` Cone。OpenSSL、nghttp2、文本、URL、压缩后端的静态库和 Linux libunwind 随目标资源交付；libuv、llhttp 与 Ryu 编入实际使用它们的 Cone／runtime。LLVM 原始源码归档、补丁和许可证另存于同版本的 `llvm-sources` 包。

运行时契约更新为 14，metadata ABI 更新为 9。本期工具配套新的目标 runtime 与预编译库；旧开发 sysroot 需要通过既有准备脚本重新生成。

纯 Scoop 的 `scoop.http.client` 在既有 HTTP、TLS 和 asyncio 库之上统一 HTTP/1.x／HTTP/2 与 ALPN，提供连接池、流式上传下载、完整生命周期超时、取消和 suspend 回调。请求策略包括重定向、重试、Basic／Bearer／Digest、mTLS、HTTP／HTTPS／SOCKS5 代理、Cookie 和私有内存／磁盘缓存；同时提供 multipart、断点续传、SSE、HTTP/1.1 WebSocket／WSS、permessage-deflate 与可观察性接口。URL、协议、压缩、TLS 和系统 I/O 能力由各自所属库提供。

`scoop.idna` 使用 libidn2／libunistring，不需要 ICU，IDNA 数据基线为 Unicode 15.1。`scoop.url` 使用 uriparser，提供 RFC 3986 URL／IRI 解析、拼装、相对引用和网络 origin，不包含浏览器全部 WHATWG 容错。`scoop.charset` 提供固定编码集合的内存及增量转换；`scoop.compression` 在同一 Cone 中提供 zstd、gzip、zlib／raw DEFLATE 和 Brotli 的内存、同步流与异步流。

HTTP/1 固定使用 llhttp 9.4.3。发送与客户端响应关联支持任意合法方法 token，服务端请求解析仍限于 llhttp 的已知方法集合。HTTP/2 固定使用 nghttp2 1.70.0 的完整静态核心；协议库保留独立使用已有传输的接口，client 负责连接选择、池与请求策略。

HTTP/3、HTTP/2 WebSocket，以及跨 origin H2 合并、证书固定／对端链检查接口、TLS 会话恢复控制／0-RTT、主动 H2 PING 保活留待后续。

仓库中的准备、LLVM 构建和打包脚本供维护者显式调用；常规 CLI 测试和 CI 不自动运行打包，也不要求先生成归档。M54 功能实现与三目标验收已完成，库测试、正式消费者、复用范围和失败闭合见仓库的 M54 验收记录。此包的部署配置、源码 revision 与实际组件列表见 `toolchain.toml`。

项目维护者尚未选定 Scoop 自身的 LICENSE 时，包只用于当前本地构建和功能验收；此说明不授予新的对外分发许可。已有第三方许可随组件保留。
