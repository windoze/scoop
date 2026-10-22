# Scoop text backend

内部 Rust 静态库，通过普通 C ABI 承接字符集转换、IDNA、Punycode 和 NFC。公开 API 仍属于
`scoop.idna` 和 `scoop.charset`，此目录不是公开 Cone 或编译器 stage。

维护者通过 `scripts/build_text_backends.py` 准备同目标资源。固定依赖及其源码、许可见
`sysroot/vendor/text_backend`；头文件为 `include/scoop_text_backend.h`。

独立维护检查（将工作目录设在仓库外）：

```sh
export CARGO_TARGET_DIR="$PWD/../scoop-work/m61/rust-target"
cargo fmt --manifest-path sysroot/native/text_backend/Cargo.toml
cargo clippy --manifest-path sysroot/native/text_backend/Cargo.toml --locked --all-targets -- -D warnings
SCOOP_SYSROOT="$PWD/sysroot" target/release/scoop test sysroot/native/text_backend/tests --release
```

测试包括实际 C 消费者的布局／生命周期、Unicode 17 IDNA 和 NFC 官方向量；库各自的
行为与组合回归仍在所属 Cone 的 tests 中。普通应用不执行 Cargo。数据错误返回状态，
Rust 内部 panic／不可恢复 OOM 终止进程，不跨 C ABI 展开。
