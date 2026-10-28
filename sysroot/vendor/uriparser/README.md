# uriparser

固定版本 1.0.2，发布于 2026-05-09。完整源码来自 [上游发行](https://github.com/uriparser/uriparser/releases/tag/uriparser-1.0.2)，归档 `uriparser-1.0.2.tar.xz` 未作修改。

Scoop 选择库和头文件的 BSD-3-Clause 许可，保留 AUTHORS 及归档中的各项许可。维护脚本只构建和复制窄字符静态库与头文件；上游工具、测试、文档和宽字符 API 不作为运行依赖。库不执行 DNS，也没有 IDNA 或 ICU 依赖。

准备入口是 `scripts/build_uriparser.py --target <triple> --build-dir <outside-repository-directory>`。输出 `sysroot/lib/<triple>/liburiparser.a` 和 `include/uriparser`，沿普通 Cone native library 路径链接。原始源码与许可随工具链分发。

Windows 三个完整目标使用上游 CMake 和 UTF-8 Ninja。Microsoft 环境选择 clang-cl／cl 和对应 x64 SDK，保持上游 `URIPARSER_MSVC_STATIC_CRT=OFF` 的动态 `/MD`；两个 LLVM-MinGW 目标显式传入各自 CRT prefix 的 `--cc .../bin/clang.exe`。工作目录可位于 E: 等其他磁盘，各目标单独安装静态 archive 和窄字符头文件，不构建上游工具、测试或文档。
