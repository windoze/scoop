# zlib-ng

固定 zlib-ng 2.3.3；[正式发布](https://github.com/zlib-ng/zlib-ng/releases/tag/2.3.3)，`zlib-ng-2.3.3.tar.gz` 为对应官方 tag 的原始源码归档。采用 zlib 许可，附带上游 LICENSE.md。

`build_compression.py` 构建原生 `zng_` API 的 PIC 静态库，关闭 zlib 兼容名字、gzFile 文件操作和测试；保留运行时 CPU 检测，禁止把构建机指令集当部署要求。启用严格 inflate 距离检查。gzip／zlib／raw DEFLATE 与 WebSocket 消息适配共享这一后端。

`patches/raw-window-limit.patch` 修正 raw 模式初始化时固定使用 32768 距离上限的问题，改为配置窗口的实际大小。原实现即使启用 INFLATE_STRICT，也会接受落在当前输出 buffer 内、超过协商窗口的距离引用；修正用于 WebSocket 的 8～15 位解码窗口，不修改 DEFLATE 算法或格式。

目标资源为 `sysroot/lib/<triple>/libz-ng.a` 及 `include/zlib-ng.h` 等头文件，源码与许可一同分发。
