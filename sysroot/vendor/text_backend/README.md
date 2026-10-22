# Rust 文本后端的固定依赖

内部包装位于 `sysroot/native/text_backend`，独立于编译器 Cargo workspace。当前提供字符集转换、IDNA、原始 Punycode 和 NFC；readline Unicode 操作的迁移进度见 [M61](../../../docs/milestone61/PROGRESS.md)。

- encoding_rs 0.8.42，遗留编码转换；Unicode 编码、ASCII 与 Latin-1 使用 Rust 标准库标量适配。
- idna 1.1.0，直接使用 UTS #46 和 Punycode API。
- icu_normalizer 2.3.0，与 idna_adapter 1.2.2／icu_properties 2.3.0 共用 Unicode 17.0.0 数据。
- 完整 36 项依赖锁定于包装的 Cargo.lock，名称、版本、声明许可与 MSRV 列于 `packages.json`。
- `crates.tar.gz` 是对该 lock 执行 `cargo vendor --locked --versioned-dirs` 的完整源码归档，包含 Cargo 的普通校验资料、原许可及通知；准备脚本解包后离线编译。没有 Scoop 特有的来源或产物证明。
- `licenses/<name-version>/` 同时保存各包的许可与通知，供发行包直接展示。实际声明仅涉及 MIT、Apache-2.0、BSD-3-Clause、Unicode-3.0；这些通知不代替目标系统运行库的许可。

维护者更新依赖时重新生成 lock、vendor 归档、packages.json 与 notices，再运行三个目标的库测试、官方 Unicode 向量和真实链接。Rust 构建使用 panic=abort；上游不可恢复分配失败和内部 panic 不保证转换成 Scoop 异常。普通应用只使用预编译目标归档。

固定的数据生成说明见 `icu_normalizer_data`、`icu_properties_data` 2.3.0 的 `src/lib.rs`：ICU release-78.1rc、CLDR 48.2.1；Unicode 版本对应 [ICU 头文件](https://github.com/unicode-org/icu/blob/release-78.1rc/icu4c/source/common/unicode/uchar.h)。
