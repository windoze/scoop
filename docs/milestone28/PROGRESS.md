# M28 实施记录

M28 正在实施，尚未达到总验收条件。目标与分批顺序见 [设计](DESIGN.md)，本机编译探针见 [调研](INVESTIGATION.md)。

## 当前工作

- 编译器 LLVM 绑定改为优先使用共享库，解决本机 LLVM 22.1.2 安装缺少静态 Polly 时不能链接的问题；保留 llvm-sys 的静态回退。
- 已记录 Linux glibc/musl amd64 支持范围，并修订 LLVM cleanup-only LSDA 的规范；对象 decoder 与 runtime personality 正在验证。
- 后续按设计完成 target/toolchain、ELF/codegen/runtime、正式 CLI 与多 Cone，再进行三种 Linux 链接配置及 macOS/AArch64 回归。

每项实现记录实际运行的验证及其局限。原生探针通过不等于正式 Scoop CLI 已支持对应目标。

## 已执行验证

- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。
- `cargo test -p scoop-codegen --lib lsda`：11 项通过，实际链接本机共享 LLVM 22.1.2。包括 catch-all、cleanup-only、损坏表诊断和 C decoder。
- 本机 Rust 验证设置 `LLVM_SYS_221_PREFIX=/usr/lib/llvm-22`、`TMPDIR=$PWD/target/tmp`，关闭 incremental 和 dev/test debuginfo，以控制构建目录体积。
