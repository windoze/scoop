# M53 实施记录

HTTP 后端与生命周期合同见 [HTTP 规范](../specs/SCOOP-HTTP-SPEC.md)。M47 延后；准备及打包继续只使用仓库维护脚本。原始日志、缓存和生成数据位于外置盘仓库已忽略的 `tmp/m53`，不提交。

## 后端源码与目标资源

- 固定 llhttp 9.4.3 的发布 C 源码、nghttp2 1.70.0 官方 tag 归档与许可证；不修改上游状态机或做函数级裁剪。nghttp2 用维护脚本构建纯静态核心，排除应用、工具、示例、测试和文档。
- 沿用已有 native 静态库解析，将 nghttp2 目标目录加入默认库搜索及手工打包清单。缓存先检查实际输入更新；命中不解包、不运行 CMake、不读回 archive。
- Darwin 实际构建与重复调用直接复用通过；Rust 格式化、toolchain lint、Python Ruff 及自身 C 代码的格式／严格编译检查通过。toolchain 单测 32 项通过；默认库根、OpenSSL 子目录和新增 nghttp2 子目录的实际更新／覆盖／两个 link 入口共 3 个 fixture 变体通过。
- 独立 native HTTP 消费者在 debug／release 与 moving GC 下通过，覆盖逐字节 llhttp chunk／trailer、已知方法限制、nghttp2 客户端／服务端 SETTINGS、逐字节帧与 HPACK 输入、请求／响应及重复字段，重复构建无 child 编译调用。首次运行遇到的上游未使用参数警告及 fixture Unsafe 块语法已修正并复验。
- GNU／musl 的后端构建和 HTTP 消费验证在后续功能批次统一执行；M52 的 Linux 全量回归仍在独立收尾。
