# M28 实施记录

M28 正在实施，尚未达到总验收条件。目标与分批顺序见 [设计](DESIGN.md)，本机编译探针见 [调研](INVESTIGATION.md)。

## 当前工作

- 编译器 LLVM 绑定改为优先使用共享库，解决本机 LLVM 22.1.2 安装缺少静态 Polly 时不能链接的问题；保留 llvm-sys 的静态回退。
- 已提供并运行 LLVM libunwind 两套本地构建脚本，headers/archive 安装到按 target 隔离的私有 prefix；复现步骤见 [构建说明](BUILDING.md)。
- LLVM cleanup-only LSDA 已实现：对象 reader 和 runtime 跳过不存在的 TType offset，拒绝无 type table 的 catch action；runtime 分成 personality、bounded LSDA decoder 和 byte reader，最长文件 475 行。
- identity/LIR 已加入两个 Linux target、ELF symbol normalization 和独立 amd64 backend 合同；原生符号与 library requirement 的生产和读取保留 libc 目标。闭合 target 以小型枚举保存，完整布局通过已知 profile 查询，避免为每份 IR 复制相同配置。此批尚未开放 Linux 正式 CLI，接下来接入 C 工具链和 ELF/codegen。
- 后续按设计完成 target/toolchain、ELF/codegen/runtime、正式 CLI 与多 Cone，再进行三种 Linux 链接配置及 macOS/AArch64 回归。

每项实现记录实际运行的验证及其局限。原生探针通过不等于正式 Scoop CLI 已支持对应目标。

## 已执行验证

- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。
- `cargo test -p scoop-codegen --lib lsda`：11 项通过，实际链接本机共享 LLVM 22.1.2。包括 catch-all、cleanup-only、损坏表诊断和 C decoder。
- `runtime_eh_personality_tests` 与 `selected_profile_creates_the_canonical_aarch64_machine` 各 1 项通过；覆盖两阶段 personality 行为及共享 LLVM 中原有 AArch64 后端。
- `scripts/check_linux_unwind.py`：glibc PIE、musl 静态、musl PIE 均经过真实 LLVM 22.1 生成的纯 cleanup 中间函数，最终 catch 并删除异常。分别链接本机新建的 LLVM libunwind 静态库，检查实际 interpreter、静态输出无动态依赖，以及 link map 未引入 libgcc EH provider。
- 本机 Rust 验证设置 `LLVM_SYS_221_PREFIX=/usr/lib/llvm-22`、`TMPDIR=$PWD/target/tmp`，关闭 incremental 和 dev/test debuginfo，以控制构建目录体积。
- target/identity 变更后，workspace clippy 无警告；identity、LIR、LIR-lower、slib 共 1,554 项单元测试通过，Darwin 已有 canonical bytes/fingerprint 向量保持。另用本机 LLVM 22.1.2 输出 MIR，核对 amd64 data layout 与声明一致。
- 已清理两套完成验证的 `target/llvm-unwind` 中间目录，保留安装后的 headers/archive。
