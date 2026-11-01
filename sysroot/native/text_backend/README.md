# Scoop text backend

内部 Rust 静态库，通过普通 C ABI 承接字符集转换、IDNA、Punycode、NFC、字素分段、属性与大小写。公开 API 仍属于
`scoop.idna` 和 `scoop.charset`，此目录不是公开 Cone 或编译器 stage。

维护者通过 `scripts/build_text_backends.py` 准备同目标资源。固定依赖及其源码、许可见
`sysroot/vendor/text_backend`；头文件为 `include/scoop_text_backend.h`。

目标包装使用 `no_std + alloc`；固定的 encoding_rs、idna、ICU4X 和 unicode-width 均支持这条路径，
idna 关闭默认 std feature，保留 alloc／compiled_data。系统分配器薄适配提供堆内存，panic handler 直接 abort。
这移除了完整 UI／HTTP 消费者实际使用的 Rust unwind personality，不影响 Scoop、C++ 或 Objective-C 的异常机制。
macOS 两架构的运行及最终链接验证见 [UI 实施记录](../../../docs/ui/PROGRESS.md)；x86_64 使用 Rosetta，Windows 新资源后续验收。

独立维护检查（将工作目录设在仓库外）：

```sh
export CARGO_TARGET_DIR="$PWD/../scoop-work/m61/rust-target"
cargo fmt --manifest-path sysroot/native/text_backend/Cargo.toml
cargo clippy --manifest-path sysroot/native/text_backend/Cargo.toml --locked --all-targets -- -D warnings
SCOOP_SYSROOT="$PWD/sysroot" target/release/scoop test sysroot/native/text_backend/tests --release
```

测试检查实际 C 消费者的布局、长度、状态和生命周期；Scoop 接口适配及 readline 编辑行为仍在所属 Cone 的 tests 中。
不维护上游 Unicode 兼容性／完备性测试或官方一致性数据。普通应用不执行 Cargo。数据错误返回状态，
Rust 内部 panic／不可恢复 OOM 终止进程，不跨 C ABI 展开。
