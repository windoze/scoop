# M61 字符集覆盖核对

状态：保留旧候选调查供迁移对照，现行 43 个名称及行为以 [ENCODINGS.md](../../sysroot/src/scoop.charset/ENCODINGS.md) 为准；不声称转换等价。以下逐项覆盖原 ENCODINGS.md 的 108 个 canonical name；2026-10-22 用户确认只保留成熟 Rust 库能覆盖的编码；本表作为旧集合的迁移对照，不再要求补齐缺失的历史编码。

encoding_rs 0.8.42 的文档和源码已核对：网页名称会折叠标准／厂商编码，GB18030 采用带网页兼容差异的 2022 映射。单字节有同名表也不表示未定义位置及反向映射已验证。

| Canonical name | 候选路径 | 需要闭合的行为 |
| --- | --- | --- |
| ARMSCII-8 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| BIG5 | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| BIG5-HKSCS | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| BIG5-HKSCS:1999 | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| BIG5-HKSCS:2001 | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| BIG5-HKSCS:2004 | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| C99 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| CP1131 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| CP1133 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| CP1250 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP1251 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP1252 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP1253 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP1254 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP1255 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP1256 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP1257 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP1258 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP850 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| CP862 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| CP866 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| CP874 | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| CP932 | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| CP936 | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| CP949 | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| CP950 | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| EUC-CN | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| EUC-JP | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| EUC-KR | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| EUC-TW | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| GB18030 | encoding_rs 的 2022 网页变体 | 2005／2022 区分、A3 A0、19 个 PUA 与四字节反向映射 |
| GB18030:2022 | encoding_rs 的 2022 网页变体 | 2005／2022 区分、A3 A0、19 个 PUA 与四字节反向映射 |
| GBK | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| GB_1988-80 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| GB_2312-80 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| GEORGIAN-ACADEMY | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| GEORGIAN-PS | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| HP-ROMAN8 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| HZ | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| ISO-2022-CN | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| ISO-2022-CN-EXT | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| ISO-2022-JP | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| ISO-2022-JP-1 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| ISO-2022-JP-2 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| ISO-2022-JP-MS | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| ISO-2022-KR | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| ISO-8859-1 | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| ISO-8859-10 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-11 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| ISO-8859-13 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-14 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-15 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-16 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-2 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-3 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-4 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-5 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-6 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-7 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-8 | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| ISO-8859-9 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| ISO-IR-165 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| JAVA | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| JIS_C6220-1969-RO | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| JIS_X0201 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| JIS_X0208 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| JIS_X0212 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| JOHAB | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| KOI8-R | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| KOI8-RU | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| KOI8-T | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| KOI8-U | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| KSC_5601 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACARABIC | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACCENTRALEUROPE | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACCROATIAN | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACCYRILLIC | encoding_rs 有相关表 | 未定义字节／C1 控制、扩展映射；CP1258 另查组合行为 |
| MACGREEK | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACHEBREW | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACICELAND | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACROMAN | encoding_rs 有对应单字节表 | 逐字节有效域、反向唯一映射和非法输入 |
| MACROMANIA | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACTHAI | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACTURKISH | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MACUKRAINE | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| MULELAO-1 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| NEXTSTEP | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| PT154 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| RK1048 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| SHIFT_JIS | encoding_rs 有相关编码但非等价证明 | 标准／厂商扩展、历史 HKSCS、反向映射与状态错误恢复 |
| TCVN | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| TIS-620 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| UCS-2 | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UCS-2BE | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UCS-2LE | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UCS-4 | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UCS-4BE | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UCS-4LE | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| US-ASCII | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UTF-16 | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UTF-16BE | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UTF-16LE | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UTF-32 | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UTF-32BE | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UTF-32LE | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| UTF-7 | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |
| UTF-8 | 标准库／基础标量适配 | UTF BOM 与端序、UCS 限制；ASCII 7-bit、Latin-1 C1 本义 |
| VISCII | encoding_rs 无该实现 | 需要成熟宽松许可实现；不得仅注册名称或隐藏 GNU fallback |

## 未采用的候选补充

- `encoding` 0.2 的集合也主要面向 WHATWG，增加 HZ 等有限编码，仍不覆盖完整集合。
- `charcode` 0.1.5 声明额外的 ISO-2022-CN／KR／JP-2 和部分 Mac／DOS 表，但发布历史短，且仍未覆盖 EUC-TW、ISO-2022-CN-EXT、JOHAB 及旧 HKSCS 全集；不能作为完整替代结论。
- ICU 的 UCM 数据可用于核对映射；数据本身不提供纯 Rust 的复杂状态编码实现。
- 已选 Rust 组合不覆盖全部 108 种。ENCODINGS.md 已列出实际支持集合和明确移除项，不让名字掩盖缺失能力。

依据：[encoding_rs 0.8.42](https://docs.rs/encoding_rs/0.8.42/encoding_rs/)、[encoding](https://docs.rs/encoding/latest/encoding/)、[charcode 0.1.5](https://docs.rs/charcode/0.1.5/charcode/)、[ICU 映射数据](https://icu.unicode.org/charts/charset)。
