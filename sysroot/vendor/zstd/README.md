# Zstandard

固定 Zstandard 1.5.7；[正式发布与源码](https://github.com/facebook/zstd/releases/tag/v1.5.7)，原始 `zstd-1.5.7.tar.gz` 随仓库保存。本地无源码修改，库采用 BSD-3-Clause，附带上游 LICENSE／COPYING。

`build_compression.py` 仅构建静态库与头文件，Unix 使用 PIC；关闭旧格式兼容、上游工具／测试／contrib 和内部多线程。普通编码、字典、流和 frame 处理使用同一 libzstd；Scoop 的异步流负责 suspend I/O，不启动 zstd worker。

三个 Windows target 分别使用其编译器与 CRT，通过 Ninja 构建；Microsoft 使用 `/MD`，`zstd_static.lib` 发布为统一的 `libzstd.a`，两个 MinGW 环境直接发布各自的 archive。

目标资源为 `sysroot/lib/<triple>/libzstd.a` 及 `include` 中的公共头文件。维护与分发沿用 M52 vendor 规则，应用不下载或现场重建依赖。
