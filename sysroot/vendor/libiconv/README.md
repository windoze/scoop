# libiconv

固定 GNU libiconv 1.19，完整归档来自 [GNU 官方发行](https://ftp.gnu.org/gnu/libiconv/libiconv-1.19.tar.gz)。静态构建 libiconv 与其 libcharset 支持库；目标资源不携带 GPL 命令行程序。

此后端供 IDNA 所用 Unicode 库以及独立字符集 Cone 使用。使用 GNU 的 libiconv 符号入口，避免将平台 iconv 的编码集合与行为当作固定后端。

`patches/strict-conversion.patch` 使普通未启用有损策略的转换严格报告不可表示字符：移除默认静默丢弃 Unicode tag 与尝试 U+FFFD 替代的路径。上游数据表、编码算法与显式 fallback 机制继续复用；`scoop.charset` 的有损入口明确调用这些机制，状态编码的替代字符由同一目标编码器生成。

库选择 LGPL-2.1-or-later，附带上游 COPYING* 与 AUTHORS，完整归档保留其他组成部分的许可。构建与目标资源布局见 [libidn2](../libidn2/README.md)；分发同时提供源码、头文件及静态再链接所需资源。
