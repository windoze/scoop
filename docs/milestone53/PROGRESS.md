# M53 实施记录

HTTP 后端与生命周期合同见 [HTTP 规范](../specs/SCOOP-HTTP-SPEC.md)。M47 延后；准备及打包继续只使用仓库维护脚本。原始日志、缓存和生成数据位于外置盘仓库已忽略的 `tmp/m53`，不提交。

## 后端源码与目标资源

- 固定 llhttp 9.4.3 的发布 C 源码、nghttp2 1.70.0 官方 tag 归档与许可证；不修改上游状态机或做函数级裁剪。nghttp2 用维护脚本构建纯静态核心，排除应用、工具、示例、测试和文档。
- 沿用已有 native 静态库解析，将 nghttp2 目标目录加入默认库搜索及手工打包清单。缓存先检查实际输入更新；命中不解包、不运行 CMake、不读回 archive。
- Darwin 实际构建与重复调用直接复用通过；Rust 格式化、toolchain lint、Python Ruff 及自身 C 代码的格式／严格编译检查通过。toolchain 单测 32 项通过；默认库根、OpenSSL 子目录和新增 nghttp2 子目录的实际更新／覆盖／两个 link 入口共 3 个 fixture 变体通过。
- 独立 native HTTP 消费者在 debug／release 与 moving GC 下通过，覆盖逐字节 llhttp chunk／trailer、已知方法限制、nghttp2 客户端／服务端 SETTINGS、逐字节帧与 HPACK 输入、请求／响应及重复字段，重复构建无 child 编译调用。首次运行遇到的上游未使用参数警告及 fixture Unsafe 块语法已修正并复验。
- GNU／musl 的后端构建和 HTTP 消费验证在后续功能批次统一执行；M52 的 Linux 全量回归仍在独立收尾。

## HTTP/1 内存编解码

- 新增普通 `scoop.http` Cone：有序字节字段与异常上下文、客户端／服务端增量解码、请求方法关联、信息响应、HEAD／CONNECT／Upgrade、chunked／定长／EOF body、独立 trailer，以及严格发送编码器。字段与 body 在 native 返回后复制交付，切换保留未消费后缀。
- 覆盖全部公开 llhttp 接收选项及正反边界，已知方法限制在收发两侧明确诊断。记录该版本对非最终 chunked Transfer-Encoding 的实际 EOF 行为，不另加解析器补偿。
- Darwin 的 24 项库测试在 debug／release 与 moving GC 下全部通过。生产 `.slib` 的独立消费者两个变体通过，包含跨 Cone 异常、泛型持有字段、显式 GC、逐字节输入、body／trailer 和 warm 构建零 child 调用。Rust 格式／lint、自身 C 格式及严格编译检查通过。
- HTTP/2 会话、协商、异步连接和 GNU／musl 集成仍待后续批次完成。
