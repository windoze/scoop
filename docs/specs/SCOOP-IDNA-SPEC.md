# Scoop IDNA 库规范

版本：M54 设计，尚未实施。提供独立普通 Cone `scoop:scoop.idna:0.1.0`，依赖 core、std；使用者包括 [URL 库](SCOOP-URL-SPEC.md)和 HTTP 域名处理。IDNA 不依赖 URL、HTTP、TLS、asyncio 或 ICU，不执行 DNS，也不维护公共后缀名单。

## 1. 实现选择

采用 libidn2 2.3.8，Unicode 基础操作使用固定的 libunistring 1.4.2。前者提供 IDNA2008、UTS #46 和 Punycode；后者提供其所需的规范化与 Unicode 操作。两者是本 Cone 的普通 native 依赖，不作为 Scoop API 暴露，不复制到 URL 或 client。

libidn2 2.3.8 的 IDNA 数据基线为 Unicode 15.1；libunistring 的数据版本不把这组 IDNA 表自动升级为 Unicode 17。文档与版本查询分别记录后端版本及 IDNA 数据版本。升级 IDNA 表属于本库的正常版本更新，不能在 client 中追加另一套字符例外表。

不要求整个 ICU。未选择 ada-idna 0.7.0 作为本期严格 DNS 入口，是因为它明确包含 WHATWG 的宽松处理，ASCII 快路径及 ToUnicode 不能直接作为完整域名有效性检查；仅执行 Punycode 也不足以替代 IDNA。

## 2. 转换合同

默认使用 UTS #46 non-transitional 处理及 IDNA2008 有效性规则：大小写与兼容映射、NFC、Punycode、A-label roundtrip、Bidi、ContextJ／ContextO、连字符位置、标签与 DNS 长度检查。ASCII 输入和已经是 `xn--` 的输入同样检查，不通过只转小写的快路径绕过校验。

输入为 String，已具有 core 的合法 UTF-8 保证；内嵌 NUL、控制字符、空域名、空中间标签和不符合 DNS 主机名规则的字符报 IdnaException。只允许末尾一个根点，转换后保留；根域 `.` 可以作为独立 IDNA 域名值表达，但 URL 的 HTTP 目标另要求实际 host。DNS wire name 总长不超过 255 字节、每标签不超过 63 字节，按 A-label 结果计算。

全角点等 UTS #46 定义的映射由同一个后端完成。不能为了得到可连接字符串而删除非法字符。包装层检查实际 DNS 结果与原输入中的结构字符边界；若固定后端的某条校验不符合上述合同，必须在 `scoop.idna` 所属实现或其后端补齐并独立回归，不能降低合同或让 URL／client 补判。

本期不提供 transitional 兼容开关、IDNA2003、混淆字符检测或注册局政策。转换结果表示算法合法，不表示域名已注册、可解析、视觉上不混淆或获某机构授权。

## 3. 公共 API

| API | 结果 |
| --- | --- |
| `Idna.toAscii(domain: String): String` | 完整验证后返回小写 ASCII／A-label，保留可选末尾根点 |
| `Idna.toAsciiOrNone(domain): String?` | 仅对无效域名返回 None |
| `Idna.toUnicode(domain: String): String` | 验证输入对应的域名后返回规范 U-label；非法 A-label 不作为成功原样返回 |
| `Idna.toUnicodeOrNone(domain): String?` | 与上一项相同，只转换正常格式失败 |
| `Idna.labelToAscii(label)`／`labelToUnicode(label)` | 单标签版本，拒绝任何标签分隔符，执行相同有效性检查 |
| `Punycode.encode(text)`／`decode(text)` | RFC 3492 的原始单标签编码；名称明确区分它与 IDNA 验证 |

libidn2 的 ToUnicode 本身不是有效性校验入口。公共 `toUnicode` 先取得经过 lookup／roundtrip 验证的域名，再调用后端解码，不把其 unused flags 当成已经执行验证。验证结果在本次转换内复用，不在 URL 或 TLS 的每次访问中重复重跑 Unicode 算法。

`IdnaException` 保存 operation、kind、后端原码与已复制 detail；kind 区分 InvalidLabel、InvalidAlabel、Disallowed、Context、Bidi、NameTooLong 和 Backend。不能确定某个标签的位置时 labelIndex 为 None，不虚构索引。参数／分配／后端运行故障不被 OrNone 吞掉。

输入借用只在同步 FFI 调用内有效，返回文本为独立 Scoop String。转换临时资源在返回前全部释放，没有需要调用者关闭的 IDNA 对象。计算是同步内存操作，不创建线程或伪装成网络 suspend。

## 4. DNS、URL 与 TLS 的使用

URL 从已解析的 host 组件调用 IDNA；百分号解码和 IPv4／IPv6 识别属于 URL，不属于 IDNA。TLS 继续只接收 ASCII／A-label DNS 名或 IP，不从展示文本重新转换。Cookie 域名使用相同 A-label，公共后缀判断另由 HTTP 的域名策略数据承担。

通用 DNS 的 OsString 接口保留原始名称语义，不被改成自动 IDNA 接口；网络 client 显式把规范化名称交给它。服务发现中 `_service._tcp` 等名称不属于本库的 DNS 主机名 profile，不能为了接受它们而放宽 URL 的 host 校验。

## 5. 构建与验收

源码、头文件、静态库和许可按 M52 的三个目标布局准备；使用 libidn2 的 LGPL-3.0-or-later 库许可选择，libunistring 与 Unicode 数据保留各自许可。分发同时履行对应源码、修改说明和静态再链接材料要求；不携带 libidn2 的 GPL 命令行程序作为运行依赖。

使用上游 UTF-8 API，不调用 locale 转换入口。libunistring 构建若需要 iconv，使用目标依赖，不能误用宿主库；该 native 依赖不使 IDNA 的公共接口依赖字符集 Cone。源准备由维护脚本完成，普通应用只消费预编译资源。

测试采用固定 IDNA 数据版本的官方向量和上游测试：Unicode／A-label roundtrip、ASCII 非法项、NFC 等价输入、德语 ß、希腊末尾 sigma、连接符、Bidi、ContextJ／ContextO、伪 A-label、分隔符映射、空标签、63／255 字节边界和末尾根点。为固定版本中任何已知差异建立精确回归，不能把同一个有缺陷的转换结果作为唯一 oracle。

三个目标运行真实跨 Cone 消费、并行独立调用和 moving GC；更新 native 或 Unicode 数据时重跑本库和 URL／Cookie 的对应组合用例。

## 6. 依据

- [libidn2 官方源码与发行](https://ftp.gnu.org/gnu/libidn/)、[API 参考](https://libidn.gitlab.io/libidn2/reference/api-index-full.html)。
- [libunistring 发行](https://ftp.gnu.org/gnu/libunistring/)。
- [UTS #46](https://www.unicode.org/reports/tr46/)、[RFC 5890](https://www.rfc-editor.org/rfc/rfc5890.html)、[RFC 5891](https://www.rfc-editor.org/rfc/rfc5891.html)、[RFC 5892](https://www.rfc-editor.org/rfc/rfc5892.html)、[RFC 5893](https://www.rfc-editor.org/rfc/rfc5893.html)。
- [ada-idna 的 ToASCII 合同](https://github.com/ada-url/idna/blob/v0.7.0/include/ada/idna/to_ascii.h)。
