# M61 实施进度

状态：实施中，尚未完成。readline 基线为 `2ac608841`；以下记录正式实现与验证结果。

## 已明确的边界

- IDNA 使用 idna 1.1.0、idna_adapter 1.2.2 与 ICU4X 2.3.0；固定数据源为 ICU release-78.1rc，Unicode 17.0.0。
- IDNA 采用 UTS #46 non-transitional + STD3 + CheckHyphens + DNS 长度检查；符号与 ContextO 差异、根点、错误精度已写入规范。
- NFC 与 IDNA 共用 normalizer。显式状态错误与 Rust 上游不可恢复 OOM／panic 的边界已写入规范；不声称 panic=abort 可以捕获分配失败。
- 2026-10-22 用户确认保留纯 Rust，字符集仅支持成熟 Rust 库覆盖的编码；不再保留全部 108 种。原集合核对见 [覆盖记录](CHARSET-COVERAGE.md)，新集合随转换器实现更新 spec 与 ENCODINGS.md。

## 验收清单

| 项目 | 状态 |
| --- | --- |
| Rust 静态库、固定依赖及 C ABI | 已实现；Darwin 正式准备、离线 vendor 构建、C aggregate／空输入／所有权测试通过 |
| IDNA／Punycode 公共入口迁移 | 已完成；11 项 Scoop 测试通过，GNU libidn2 源码与构建依赖已移除 |
| NFC 公共入口迁移 | 已完成；Unicode 17 全部 20,034 行、100,170 条 NFC 不变量通过 |
| 字符集新集合、映射差异、严格／有损与增量 | Rust 迁移完成；43 个名称、65 个退役名称及 GB18030／CJK／Windows 映射差异已记录；Darwin 21 项 Scoop 测试与转换 C ABI 测试通过 |
| 官方 IDNA 向量与 Scoop 组合 | 6,389 行有效 scalar 输入通过；2 行 surrogate 按上游说明排除；根点／末尾根标签按 Scoop profile 验证 |
| Darwin 正式 Scoop 消费 | C ABI＋IDNA＋NFC conformance 3 项、charset 17 项、URL 14 项、HTTP NFC authentication 1 项、Cookie 18 项通过；API 文档零警告 |
| GNU、musl 原生及正式 Scoop 消费 | 待验证 |
| 缓存、失败不发布、并发准备与两个链接入口 | Darwin 常规准备复用通过；其余待验证 |
| 发行、许可、GNU 链移除及文档同步 | IDNA／NFC 的 29 个 Rust 依赖源码与许可已随附；新增 encoding_rs 后共 36 项；其余 GNU 链随 readline Unicode 迁移移除 |
| M61 完成后 readline Unicode 迁移 | 待完成 |

Darwin 静态库实际工具链为 rustc 1.99.1，报告的系统库为 System、c、m。字符集组合中 HTTP 扩展参数 3 项通过，API 文档零警告；HTTP client 与 Linux 验证继续进行。Rust fmt／clippy、Python ruff 0.16.10 与 diff whitespace 检查通过。Linux 结果不以 Darwin 试验替代。

日志、临时工程及构建工作放在仓库外 `../scoop-work/m61`。不新增 fixture，库行为使用 scoop.test。
