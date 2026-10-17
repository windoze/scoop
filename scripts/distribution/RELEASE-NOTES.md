# M53 HTTP 协议库与工具链交付

本期提供 Darwin/AArch64、Linux/GNU amd64 两类宿主工具包，覆盖 Darwin、GNU、musl 三个既有目标。宿主包包含 `scoop`、`scoopc`、`scoop-link`，以及 LLVM 22.1.8 的所需动态依赖。M47 延后，不附带 generator。

统一布局保留十二个内置 Cone 的源码、预编译产物、公开 C 头、runtime 对象、OpenSSL／nghttp2 静态库和 Linux libunwind。新增的 `scoop.http` 提供 HTTP/1.0、1.1、HTTP/2 内存协议与协商；`scoop.http.asyncio` 接管已有异步字节流，提供请求／响应、body、背压、取消、关闭和升级移交。libuv、llhttp 与 Ryu 编入实际使用它们的 Cone／runtime。LLVM 原始源码归档、补丁和许可证另存于同版本的 `llvm-sources` 包。

HTTP/1 固定使用 llhttp 9.4.3，收发均限于其已知方法集合，`CUSTOM` 等扩展方法明确诊断。HTTP/2 固定使用 nghttp2 1.70.0 的完整静态核心；包中不附带其应用、测试或 HPACK 工具。HTTP 库使用已有传输和实际 TLS ALPN 结果，不提供 TCP 建连、TLS 握手、连接池或自动重试。

仓库中的准备、LLVM 构建和打包脚本供维护者显式调用；常规 CLI 测试和 CI 不自动运行打包，也不要求先生成归档。具体验证范围以仓库的 M52、M53 实施记录为准。此包的部署配置、源码 revision 与实际组件列表见 `toolchain.toml`。

项目维护者尚未选定 Scoop 自身的 LICENSE 时，包只用于当前本地构建和功能验收；此说明不授予新的对外分发许可。已有第三方许可随组件保留。
