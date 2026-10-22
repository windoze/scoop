# M61 字符集迁移对照

2026-10-22 用户确认保留纯 Rust，只支持成熟 Rust 库能覆盖的编码。M54 的 108 个 canonical name 收敛为 43 个，65 个旧名称及其别名退役；完整名称和行为以 [ENCODINGS.md](../../sysroot/src/scoop.charset/ENCODINGS.md) 为准。

| 范围 | 实现与决定 |
| --- | --- |
| UTF-8、UTF-16／32 及显式字节序、ASCII、Latin-1 | 9 个基础编码，由 Rust 标准库标量接口与 Scoop 字节序／BOM 适配提供 |
| 已保留的 CJK、有状态及单字节编码 | 34 个编码使用 encoding_rs 0.8.42；采用其实际映射，不承诺与旧 GNU 表逐项等价 |
| 旧 HKSCS 年份、GB18030 年份、CP932／936／949／950 等区分 | 不用近似实现保留旧名称；明确退役 |
| 其余没有选定成熟 Rust 实现的历史编码 | 返回 UnsupportedCharsetException；不补写 CJK／ISO-2022 算法，不保留 GNU fallback |

本表记录名称集合的迁移决定。GB18030、GBK、Big5、Shift_JIS、EUC-JP、Windows 单字节表的差异在编码列表说明，依据固定上游源码及文档；不建立兼容性／完备性正负例，不复核上游 Unicode 或编码映射数据。

未采用 `encoding` 0.2 或 `charcode` 0.1.5 扩充集合；不再把保留全部旧编码设为完成条件。

依据：[encoding_rs 0.8.42](https://docs.rs/encoding_rs/0.8.42/encoding_rs/)、[encoding](https://docs.rs/encoding/latest/encoding/)、[charcode 0.1.5](https://docs.rs/charcode/0.1.5/charcode/)。
