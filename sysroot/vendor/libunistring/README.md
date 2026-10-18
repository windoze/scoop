# libunistring

固定上游版本 1.4.2，完整归档来自 [GNU 官方发行](https://ftp.gnu.org/gnu/libunistring/libunistring-1.4.2.tar.xz)。本地无源码修改，按上游默认启用符号命名空间，构建静态库；另通过编译宏把私有 `locale_charset` 命名为 `scoop_libunistring_locale_charset`，避免多个 gnulib 副本冲突。公共 API 不变，不依赖 ICU。

库用于 IDNA 的 UTF 转换、规范化与字符属性。它的 Unicode 数据不会自动升级 libidn2 固定的 IDNA／UTS #46 表，两者的版本分别记录。

库选择 LGPL-3.0-or-later，附带上游 COPYING* 与 AUTHORS，完整归档保留各文件的许可。构建与目标资源布局见 [libidn2](../libidn2/README.md)；使用同目标固定 libiconv，分发保留源码、头文件与静态再链接所需资源。
