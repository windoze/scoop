# Windows 十进制解析

来源：[mingw-w64 gdtoa](https://github.com/mingw-w64/mingw-w64/tree/6de5d3b4db33c23e404148321ccb4a7bf92ca4fc/mingw-w64-crt/gdtoa)。
固定 commit：`6de5d3b4db33c23e404148321ccb4a7bf92ca4fc`，2026-09-17；该目录当月仍有线程初始化与清理修订。
原作者 David M. Gay，许可见 LICENSE 及各源文件头。

只构建 binary32／binary64 解析及其依赖；不构建格式化或扩展精度入口。
Windows 三种 native 环境共用此 C 实现，避免 MSVCRT `_strtof_l` 的 Double 中转和不支持线程私有 locale 的限制。
解析固定使用 `.`，不读取或修改应用 locale；每种精度直接最近偶数舍入。

`scoop-integration.patch` 保存对上游的最小修改：禁用 locale 选择、去掉标准库 alias 与 MinGW 专用锁初始化，接入 `scoop_config.h`。
配置将库符号加私有前缀并启用多线程；两把锁由 `runtime/src/platform/numeric/windows.c` 提供。
上游数值算法保留原文及格式；Scoop 的 wrapper、输入完整消费与有限值判断位于 runtime。
