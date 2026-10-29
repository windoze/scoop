# Brotli

固定 Brotli 1.2.0；[正式发布](https://github.com/google/brotli/releases/tag/v1.2.0)，`brotli-1.2.0.tar.gz` 为对应官方 tag 的源码归档。本地无源码修改，采用 MIT 许可，附带上游 LICENSE。

`build_compression.py` 只构建 encoder、decoder、common 的静态库，Unix 使用 PIC；关闭命令行工具与测试。生产库支持 RFC 7932 的 Brotli 流，不启用非标准 large-window 扩展。

三个 Windows target 分别使用其编译器与 CRT，通过 Ninja 构建；Microsoft 使用 `/MD`，三个 `.lib` 按下面的统一 archive 名称发布，两个 MinGW 环境直接发布各自的 archive。

目标资源为 `sysroot/lib/<triple>/libbrotli{enc,dec,common}.a` 及 `include/brotli` 头文件，普通应用通过压缩 Cone 的 native 声明消费。
