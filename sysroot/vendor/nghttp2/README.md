# nghttp2 1.70.0

固定版本：[v1.70.0](https://github.com/nghttp2/nghttp2/tree/v1.70.0)。`nghttp2-1.70.0.tar.gz` 是该官方 tag 的 [源码归档](https://codeload.github.com/nghttp2/nghttp2/tar.gz/refs/tags/v1.70.0)，保留完整源码；MIT 许可证另复制为 COPYING。

维护者运行 `scripts/build_nghttp2.py --target <triple>`，使用上游 CMake 的纯静态库配置，生成 `sysroot/lib/<triple>/nghttp2/include` 与 `lib/libnghttp2.a`。普通应用构建只消费已准备资源；不自动下载、解包或构建。完整上游协议核心保持不变，不进行函数级裁剪。构建边界见 [M53 设计](../../../docs/milestone53/DESIGN.md)。
