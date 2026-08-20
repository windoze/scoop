# M28 验收记录

2026-10-05：M28 已完成。正式 CLI 支持 Linux glibc / musl amd64，保留 Darwin/AArch64。设计与代码边界见 [DESIGN.md](DESIGN.md)，复现步骤见 [BUILDING.md](BUILDING.md)，分批修复见 [PROGRESS.md](PROGRESS.md)。

## 平台和工具链

| 平台 | 本次验证环境 | 链接模式 |
| --- | --- | --- |
| Linux glibc amd64 | LLVM 22.1.2、GCC 15.2.0、GNU ld 2.46、glibc 2.43 | 默认动态 PIE |
| Linux musl amd64 | LLVM 22.1.2、GCC 15.2.0 / musl-gcc | 默认静态 executable；显式动态 PIE |
| macOS AArch64 | M3、Homebrew LLVM 22.1.8、系统 Xcode 工具链 | 原有 Mach-O executable |

Linux 使用从 `../llvm-project-22.1.2.src` 分别构建的两套 LLVM libunwind 静态 PIC 库。构建脚本在私有副本应用 musl PIE 的 FDE 索引未命中补丁，原始源码保留。Darwin 使用系统 libSystem 的 unwinder。

远程回归在 `m3u.0d0a.com` 的 `~/repos/scoop/target/m28-darwin` 隔离 worktree 执行，临时目录位于该 worktree 的 `target/tmp`。

## 完整文件 fixture

三个目标都执行了普通 `--all`，共发现并选择 2,320 个声明。验收及后续复验均未使用 `--update-snapshots`。下表按用例去重，glibc 的八个失败项使用修正后的普通复验结果。

| 目标 | 通过 | 不适用 | 变体 | 执行进程 | stage / plan / 输出快照 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `x86_64-unknown-linux-gnu` | 2,295 | 25 | 2,382 | 11,856 | 12,073 |
| `x86_64-unknown-linux-musl` | 2,297 | 23 | 2,384 | 11,862 | 12,073 |
| `aarch64-apple-darwin` | 2,314 | 6 | 2,404 | 12,076 | 12,305 |

glibc 的原始全量报告为 **2,287 通过、8 失败、25 不适用**。八个失败均位于最后的产物指纹检查：这些期望值生成于 imported-storage visibility 修复之前。逐个对比保留的旧 ELF 与当前对象，确认 storage 从默认 visibility / GOT 访问变为 hidden / 直接访问；程序的 IR、构建、普通运行、moving GC 运行和独立重链接检查均已通过。重复构建的归档逐字节相同。

仅更新以下用例的 `program` 指纹，随后全部普通复验通过，合计 8 个变体、50 个进程和 96 份快照：

- `core-nominals-storage-roots`、`core-values-prelude`；
- `default-access-source-profiles-domain-binding`、`default-access-value-access-combined`；
- `default-operations-nominal-operations-standalone`、`default-operations-operation-signatures-combined`；
- `default-properties-nominals`、`heap-zst-combined`。

musl 和 Darwin 的完整运行均为零失败。已把最终报告中的通过集合与每个 target 的 fixture 声明逐项比较，所有适用用例都有通过结果，不适用集合也与声明一致。

原始报告保留在本地：

```text
target/m28-final-gnu/report.json
target/m28-final-gnu-repair-first/report.json
target/m28-final-gnu-repair-second/report.json
target/m28-final-musl/report.json
target/m28-darwin-evidence/m28-final-darwin-complete/report.json
```

Linux runner 使用同一份已构建的 `target/m28-atexit-tools/{scoop,scoopc,scoop-link}` 和 `/usr/lib/llvm-22/bin/llc`，两个 target 各有独立工作目录。Darwin 使用隔离 worktree 的三个 `target/release` 工具及 LLVM 22.1.8 的 `llc`。完整命令见构建说明。

## Rust、Python 和格式检查

- Linux workspace 共 5,318 项：`cargo test --release --workspace --no-fail-fast` 首轮 5,317 项通过，唯一失败是假编译器未读 stdin 就退出的测试竞态。测试 helper 改为先读完请求后，所属 `scoop --lib` 的 96 项全部通过；生产 transport 未改动。
- macOS/AArch64 workspace 共 5,290 项，完整运行全部通过。
- 两个宿主的 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 均通过，三个正式 binary 已构建。
- Python fixture runner 的 38 项公共规则测试通过；Ruff 0.16.10 format / check 通过。

Linux Rust 记录为 `target/m28-final-workspace-tests.log` 和 `target/m28-child-test-tests.log`；Darwin workspace、clippy、构建日志已复制到 `target/m28-darwin-evidence/`。

## 覆盖与边界

正式 fixture 覆盖源码与 `.slib` 消费、跨 Cone 泛型 / ODR、独立链接、C ABI 和 Scoop ABI、TLS、foreign callback、异常、初始化、release hook、协程、Context 及 moving GC。目标相关 LIR、符号表、链接计划、完整诊断和产物指纹保存各自期望；源码错误保留完整位置和消息比较。

musl 全量中的普通用例采用默认静态模式。显式动态 PIE 由 `m28-cli-program-musl-pie`、`m28-deep-frames-musl-pie` 和 `m28-native-dso` 覆盖，包括分配 / 移动 GC / 异常、独立重链接、2,048 层深调用与 600 层 cleanup、DSO 版本 / TLS / constructor / archive 依赖。底层帧、栈增长、ELF image 和真实 unwind 测试另覆盖 glibc PIE、musl 静态、musl PIE 三种配置。

23 个既有用例保留 Darwin 条件：16 个验证 dylib / framework / two-level namespace / Darwin loader，7 个使用 Mach-O 对象字段、归档布局或损坏向量。ELF 对象、归档和最终 image 的损坏检查由 slib、driver、linker 的实际对象测试及 Linux 专用 fixture 覆盖。glibc 的另外两项不适用是 musl PIE 专用用例；Darwin 的六项不适用是 M28 Linux 专用用例。

本阶段不提供 Linux arm64、glibc 全静态或 musl static PIE，也未验证旧 glibc 发行版的最低部署版本。Linux runtime 使用 `/proc/self/maps` 核对实际 metadata 权限。后续 arm64 可复用 Linux OS/VM、ELF image、metadata 脚本与共有链接编排，补充相应 ABI、机器帧 / relocation、入口和工具链输入。

测试结束后清理本轮临时用例、重复 cache / sysroot 和探针构建产物，保留工具、unwind 安装产物、日志和报告。
