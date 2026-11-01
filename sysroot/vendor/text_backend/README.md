# Rust 文本后端的固定依赖

内部包装位于 `sysroot/native/text_backend`，独立于编译器 Cargo workspace。提供字符集转换、IDNA、原始 Punycode、NFC，以及 readline 使用的 Unicode 基础操作。

- encoding_rs 0.8.42，遗留编码转换；Unicode 编码、ASCII 与 Latin-1 使用 Rust core 标量适配。
- idna 1.1.0，直接使用 UTS #46 和 Punycode API。
- icu_normalizer 2.3.0，与 idna_adapter 1.2.2／icu_properties 2.3.0 共用 Unicode 17.0.0 数据。
- icu_segmenter／icu_casemap／icu_properties 2.3.0 提供扩展字素分段、完整大小写和 Alnum；unicode-width 0.2.2 提供单标量宽度，均为 Unicode 17.0.0。
- 完整 46 项依赖锁定于包装的 Cargo.lock，名称、版本、声明许可与 MSRV 列于 `packages.json`。
- `crates.tar.gz` 是对该 lock 执行 `cargo vendor --locked --versioned-dirs` 的完整源码归档，包含 Cargo 的普通校验资料、原许可及通知；准备脚本解包后离线编译。没有 Scoop 特有的来源或产物证明。
- `licenses/<name-version>/` 同时保存各包的许可与通知，供发行包直接展示。实际声明仅涉及 MIT、Apache-2.0、BSD-3-Clause、Unicode-3.0；这些通知不代替目标系统运行库的许可。
- Rust 标准库不在 Cargo.lock 中；维护构建从实际 Rust 工具链保存其 `COPYRIGHT-library.html` 及 Apache-2.0／MIT 许可，发行时按目标收集到 `share/licenses`。

维护者更新依赖时重新生成 lock、vendor 归档、packages.json 与 notices，再运行各目标受影响的 Scoop 适配测试和真实链接；不复核上游 Unicode 算法或数据的兼容性、完备性。包装使用 no_std + alloc，idna 只开启 alloc／compiled_data，其他固定依赖保持现有功能。系统分配器薄适配提供堆内存，Rust 构建使用 panic=abort，panic handler 调用系统 abort；上游不可恢复分配失败和内部 panic 不保证转换成 Scoop 异常。普通应用只使用预编译目标归档。core／alloc 仍使用 Rust 标准库许可通知；构建用 proc macro 不进入目标程序。

Windows 维护构建使用 `scripts/release.toml` 固定的 Rust 1.99.1，安装 MSVC、GNU、GNU-LLVM 三种 x86-64 标准库，分别服务 Scoop 的 Microsoft UCRT、LLVM-MinGW MSVCRT、LLVM-MinGW UCRT 目标。`build_text_backends.py` 将实际 COFF 静态库发布为各目标目录下的 `libscoop_text_backend.a`；no_std 分配器使用所选 CRT 的 aligned allocation 和 abort，不再依赖 Rust std 的 Windows 系统库集合。此次调整先在 macOS aarch64 验证，x86_64 在 UI 批次最后统一补验；Windows 新资源及组合验收留待后续，原 W0 结果不作为新归档已验证的依据。

固定的数据生成说明见 `icu_normalizer_data`、`icu_properties_data`、`icu_casemap_data`、`icu_segmenter_data` 2.3.0 的 `src/lib.rs`：ICU release-78.1rc、CLDR 48.2.1；Unicode 版本对应 [ICU 头文件](https://github.com/unicode-org/icu/blob/release-78.1rc/icu4c/source/common/unicode/uchar.h)。
