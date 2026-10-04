# M28 实施记录

M28 正在实施，尚未达到总验收条件。目标与分批顺序见 [设计](DESIGN.md)，本机编译探针见 [调研](INVESTIGATION.md)。

## 当前工作

- 编译器 LLVM 绑定改为优先使用共享库，解决本机 LLVM 22.1.2 安装缺少静态 Polly 时不能链接的问题；保留 llvm-sys 的静态回退。
- 已提供并运行 LLVM libunwind 两套本地构建脚本，headers/archive 安装到按 target 隔离的私有 prefix；复现步骤见 [构建说明](BUILDING.md)。
- LLVM cleanup-only LSDA 已实现：对象 reader 和 runtime 跳过不存在的 TType offset，拒绝无 type table 的 catch action；runtime 分成 personality、bounded LSDA decoder 和 byte reader，最长文件 475 行。
- identity/LIR 已加入两个 Linux target、ELF symbol normalization 和独立 amd64 backend 合同；原生符号与 library requirement 的生产和读取保留 libc 目标。闭合 target 以小型枚举保存，完整布局通过已知 profile 查询，避免为每份 IR 复制相同配置。此批尚未开放 Linux 正式 CLI，接下来接入 C 工具链和 ELF/codegen。
- C bridge 已拆成 Darwin/Apple Clang 与 Linux/GCC 平台合同；Linux discovery 选择实际 GCC、musl wrapper/specs 和 headers，并以真实 ELF64/PIC/TLS 编译验证 libc。invocation 保存显式 PATH/REALGCC/native sysroot，不携带伪造的 macOS SDK/deployment。Darwin 持久合同 bytes/fingerprint 保持；Linux 正式 CLI 与 ELF bridge reader 接入仍在后续批次。
- 已实现 Linux thread/VM、amd64 精确 frame adapter、18 个 managed entry 汇编入口和 String sret adapter。共有 stackmap decoder 不再硬编码 AArch64 的 frame-size 对齐。线程在 boundary/anchor/native transition 范围 miss 时重新查询当前 OS 栈范围，并沿原有 world/park 协议发布；musl 主线程的深栈不会继续使用 attach 时的过小范围。runtime build 接入和完整 moving collector 闭环尚待后续完成。
- Linux platform bundle 已完整组合 ELF image、Linux thread/VM、amd64 frame。ELF adapter 读取加载后的 program headers 和实际 VM permissions，按 hidden linker bounds 取得 stackmaps；已提供动态 RELRO 与静态只读段两套 metadata scripts。共有 callable 范围检查不再硬编码 4-byte 指令。Linux SHA-256 使用最小配置的 Mbed TLS 3.6.6 模块，保留上游文件/许可证并隔离嵌入符号；Darwin 继续 CommonCrypto。
- runtime 构建配置已拆成共有源文件与封闭平台组合，并提供各自的 include/编译参数。target resolution 不再提前解析最终链接工具。runtime cache schema 2 按实际 depfile 跟踪外部头文件内容和 include locator，适用于 SDK、GCC、musl 与 LLVM unwind headers；安装路径只留在本地失效检查中，不进入 artifact 输入 identity。正式 Linux runtime index 与最终链接接入仍在后续批次。
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
- C 工具链变更：全 workspace fmt/clippy 通过；3 项 Linux discovery 测试、15 项 LIR C bridge 测试，以及公共 GCC/Clang depfile 转义测试通过。测试包含 glibc/musl 交叉误选、缺失 driver/sysroot、TLS section/尺寸及原 Darwin 固定向量。`object::ObjectSymbol::is_definition()` 不涵盖 ELF `STT_TLS`，TLS 定义使用类型和实际 section 判定。
- Linux runtime 组件：`linux_runtime` 的实际 LLVM 22.1 statepoint 测试在 glibc PIE、musl static、musl PIE 各运行 O0/O2，验证 0～3 个显式参数、精确 PC、root slot 回写后 `gc.relocate` 读到新地址、两层帧遍历、String 两种 sret 结果和 main/pthread 栈及 VM 操作。此测试只验证真实机器帧与 root 回写，不冒充完整 collector 验收。
- 线程栈增长测试在相同 6 种配置下通过，约 4 MiB 深栈依次触发 managed boundary、anchor、native-safe 和 callback 发布；musl 实际确认 4 次栈范围扩展。共有 v3 parser 的现有损坏表测试及 AArch64 frame adapter 数值测试也在 Linux 上通过；完整 Darwin 执行回归仍需真实 macOS。
- ELF image 测试在 glibc PIE、musl static/PIE 下验证完整 platform bundle、load bias、stackmap relocation、metadata 实际只读、无 `DT_TEXTREL`；移除 ELF section table locator 后仍通过。将 stackmap 页临时改为可写时，adapter 正确拒绝，恢复只读后成功。SHA-256 的空串、短串、多 block、百万 byte 向量在三个配置均通过。
- 原有 4 项 runtime image/registration 测试已在 Linux 运行，通过 canonical stackmap 的 Rust/C 共用向量及全部损坏 metadata 负例。迁移时发现 Linux piped core handler 会忽略 `RLIMIT_CORE`，现仅在预期 abort 的测试子进程关闭 dumpability；这组测试由约 114 秒恢复为不到 1 秒，生产 runtime 无此设置。Linux runtime 组件共 5 项 Rust 测试通过，全 workspace fmt/clippy 无警告。
- 新 runtime 构建配置分别通过 glibc、musl 的全部 C/汇编源文件编译（GCC、O2、Wall/Wextra/Werror、对应 LLVM unwind headers），检查完整 runtime ABI 的函数/数据/TLS 定义、重复定义和 SHA 符号隔离。外部头文件缓存测试覆盖三个 header root、内容变化、缺失 depfile 和 symlink 改指向；全 workspace fmt/clippy 通过。该批是完整 runtime 对象编译，尚非正式 Scoop 程序链接/运行验收。
