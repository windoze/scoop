# libidn2

固定上游版本 2.3.8，完整源码归档 `libidn2-2.3.8.tar.gz` 来自 [GNU 官方发行](https://ftp.gnu.org/gnu/libidn/libidn2-2.3.8.tar.gz)，可由 GNU 镜像取得相同发行文件。IDNA／UTS #46 数据为 Unicode 15.1.0。

Scoop 的 `patches/validation.patch` 修正固定版本的三个缺口：lookup 执行 ContextO 规则；RFC 5893 检查覆盖整个域名以及 RTL 标签的 EN／AN 混用和末尾 NSM；长度预检查在 NFC 后执行，避免误拒绝规范化后合法的输入。修改服务 `scoop.idna` 的严格 DNS hostname profile，复用现有 Unicode 属性与标签解码，不增加数据表、公开后端 API 或额外规范化。其余代码来自原始发行；上游命令行工具不进入目标资源。

静态构建把本库自带的 gnulib `locale_charset` 和 `_UC_CATEGORY_NONE` 分别命名为 `scoop_libidn2_locale_charset` 和 `scoop_libidn2_uc_category_none`，避免与 libunistring、libiconv 的定义冲突；公共 libidn2 API 不变。

`patches/punycode.patch` 在既有解码算法中增加内部 raw 入口，允许 RFC 3492 基本字符与纯 ASCII 编码的末尾分隔符，原有 IDNA 入口保留 DNS 限制。同时修正无效整数编码字符和低代理码点检查。Scoop 的 `Punycode` 使用 raw 入口；域名转换继续走 IDNA lookup。

库选择 LGPL-3.0-or-later 许可，附带上游 COPYING* 和 AUTHORS；原归档还保留工具、文档和测试的各自许可。分发包含原始源码、该补丁、构建脚本、头文件与静态库。静态链接应用按 LGPL 提供其适用的重新链接材料；Scoop 的普通 `.slib` 和独立链接入口可用于与修改后的后端重新链接，不为此新增授权机制。

准备入口为 `scripts/build_text_backends.py --target <triple> --build-dir <outside-repository-directory>`，依次准备同目标的 libiconv、libunistring、libidn2。头文件在 `sysroot/lib/<triple>/include`，静态库直接在目标目录，供已有 native library 搜索路径消费。
