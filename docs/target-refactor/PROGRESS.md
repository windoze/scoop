# Target 重构与 macOS x86_64 实施记录

状态：实施中。范围与边界见[设计](DESIGN.md)。

- 分支：`codex/target-refactor-macos-x86-64`。
- 起点：`09be3315a85cfa9e50f79e49785bd23e0880f1f0`，包含 Windows targets 与 AppKit／Win32 窗口库合入。
- 本阶段完成目标边界重构并加入 `x86_64-apple-darwin`；Linux ARM64 GNU／musl、OpenHarmony AMD64／ARM64 留待后续。
- 每批完成格式化／lint 和相应验证后提交、推送，再获取并合并远端 `main` 的新增提交。原始日志与临时构建放在 Git 忽略的 `tmp/target-refactor/`，测试设置 `TMPDIR` 为仓库的 `tmp`。

## 环境

本机为 Apple Silicon，macOS 27.0.1，Xcode macOS SDK 27.0。`arch -x86_64 /usr/bin/uname -m` 已成功；本阶段新增目标通过本机 Rosetta 执行，结果不视作真实 Intel 硬件验收。部署版本和 SDK slice 在实际编译时确认，尚未承诺旧系统运行范围。

## 批次

| 批次 | 状态 | 范围与结果 |
| --- | --- | --- |
| R0 | 完成 | 完成 Windows 后设计复审，更新本阶段范围及目录所有权规范，创建实施分支 |
| R1 | 完成 | 独立 target-data 与 Rust／Python 共用目录；PM／选库更新输入；585 项 Rust 测试通过，22 项脚本测试与 70 项 fixture 规则各跳过 3 项宿主不适用测试，其余通过 |
| R2 | 完成 | 显式 ABI／backend／EH／frame 与 codegen 调优数据；1033 项测试通过，配套 LLVM 构建后的 Windows 返回 PC 测试也已通过 |
| R3 | 完成 | 对象 machine／格式与 runtime frame 边界；713 项 Rust 测试通过，配套 core／runtime 的真实链接回归通过 |
| R4 | 待实施 | 工具角色、架构参数、SDK 与 Linux Clang |
| R5 | 待实施 | native／RC 的 target sysroot include 与维护流程 |
| R6 | 待实施 | macOS x86_64 实现、资源与本机运行 |
| R7 | 待实施 | 七目标回归与完成记录 |

### R1 验证范围

`cargo fmt --all`、workspace／all-targets Clippy 和 Ruff 0.16.10 通过；Rust 测试覆盖 target-data、LIR、manifest、toolchain 和 PM。实际目标选择／C producer 测试移至 target-data，LIR 仅保留自身类型的代表样例。检查正常依赖图确认 artifact-only linker 无 LLVM，LIR／lir-lower 不依赖目标实现或 ABI planner；独立 runtime builder 的 lockfile 与移动后的 crate 配套。

本机 LLVM 22.1.8 使用 SDK 27.0 编译 `x86_64-apple-macosx13.0` 的 C 探测程序，产物确认为 Mach-O x86_64 并成功执行。该结果证明本机具备交叉生成与 Rosetta 运行条件，不作为 macOS 13 的运行验收。R0 推送后已获取远端 main，无新增提交。

### R2 验证范围

`cargo fmt --all`、workspace／all-targets Clippy 通过；ABI planner、LIR、slib、target-data 和 codegen 的 1033 项测试通过。目标／backend 合同直接演进，固定编码与指纹测试已同步；runtime 的 EH 生命周期 harness 删除对已退役线程字段的引用，重新编译执行通过。

本机 Homebrew LLVM 22.1.8 未包含项目的 Windows statepoint 返回地址补丁，专门测试读到 WinEH padding 后的地址（16 而非实际返回地址 15）。该项未改期望；`tmp/target-refactor/llvm/` 的配套 LLVM 22.1.8 构建完成后复验通过，codegen 384 项测试全部通过。开发命令使用该 SDK 的 rpath，避免 Cargo 环境加载 Rust 自带的同名 LLVM 动态库。目标指纹变化后的系统资源将在本阶段重新准备。R1 推送后已获取远端 main，无新增提交。

### R3 验证范围

`cargo fmt --all` 与 workspace／all-targets Clippy 通过。使用配套 LLVM 22.1.8 的 codegen 384、slib 252、toolchain 53、target-data 10 项测试全部通过；重建配套 CLI、core 与 release runtime 后，linker 14 项测试通过，覆盖实际链接、符号裁剪、native 输入更新和 runtime index 搬迁。各测试保留真实源码与工具。

Mach-O envelope 显式保存目标，容器／符号／section 命名去除 arm64 假设；AArch64 relocation 规则成为独立模块。ELF、COFF、fat slice、native 动态输入与 SDK 文本桩从已选目标取得 machine。runtime frame 与 anchor 从 arch 目录移出，源码组合分别选择 OS、image、ISA、frame 与静态 bundle，Darwin OS 实现去除架构限制。现有 SysV／SEH／Darwin 帧算法保持。R2 推送后已获取远端 main，无新增提交。
