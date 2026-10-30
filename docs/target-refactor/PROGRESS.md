# Target 重构与 macOS x86_64 实施记录

状态：实施中。范围与边界见[设计](DESIGN.md)。

- 分支：`codex/target-refactor-macos-x86-64`。
- 起点：`09be3315a85cfa9e50f79e49785bd23e0880f1f0`，包含 Windows targets 与 AppKit／Win32 窗口库合入。
- 本阶段完成目标边界重构并加入 `x86_64-apple-darwin`；Linux ARM64 GNU／musl、OpenHarmony AMD64／ARM64 留待后续。
- 每批完成格式化／lint 和相应验证后提交、推送，再获取并合并远端 `main` 的新增提交。原始日志与临时构建放在 Git 忽略的 `tmp/target-refactor/`，测试设置 `TMPDIR` 为仓库的 `tmp`。
- 验证顺序按用户要求：全部实现及本机验证完成后，再统一验证 Linux 和 Windows；连接显式使用 `/usr/bin/ssh nuc12` 与 `/usr/bin/ssh 192.168.2.67`。

## 环境

本机为 Apple Silicon，macOS 27.0.1，Xcode macOS SDK 27.0。`arch -x86_64 /usr/bin/uname -m` 已成功；本阶段新增目标通过本机 Rosetta 执行，结果不视作真实 Intel 硬件验收。部署版本和 SDK slice 在实际编译时确认，尚未承诺旧系统运行范围。

## 批次

| 批次 | 状态 | 范围与结果 |
| --- | --- | --- |
| R0 | 完成 | 完成 Windows 后设计复审，更新本阶段范围及目录所有权规范，创建实施分支 |
| R1 | 完成 | 独立 target-data 与 Rust／Python 共用目录；PM／选库更新输入；585 项 Rust 测试通过，22 项脚本测试与 70 项 fixture 规则各跳过 3 项宿主不适用测试，其余通过 |
| R2 | 完成 | 显式 ABI／backend／EH／frame 与 codegen 调优数据；1033 项测试通过，配套 LLVM 构建后的 Windows 返回 PC 测试也已通过 |
| R3 | 完成 | 对象 machine／格式与 runtime frame 边界；713 项 Rust 测试通过，配套 core／runtime 的真实链接回归通过 |
| R4 | 本地完成 | Darwin 配置与 Linux GCC／Clang producer、C++ family 已拆开；本机 489 项测试通过，远端验收在 R7 闭合 |
| R5 | 本地完成 | target sysroot include、架构参数与交叉编译 fixture 流程已验证；RC-only 实机检查在 R7 闭合 |
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

### R4 工具选择与 Darwin 配置

`cargo fmt --all` 与 workspace／all-targets Clippy 通过，toolchain、target-data、linker 的 77 项测试通过。Darwin 真实 probe 同时覆盖 Xcode 默认发现和显式 Apple Clang／SDK，所得 producer 合同相同。目录的 native 环境用于 C producer、C++、unwind 与最终链接选择；Darwin 的 target、SDK、部署版本和 linker 架构独立传递，各目标默认部署版本从发行配置编入。工具发现缓存键随配置语义更新，命中流程保持先判断更新。R3 推送后已获取远端 main，无新增提交。

Linux generated-C 接入 GCC／Clang，C++ 独立记录 compiler family，GNU 方言不等同于 GCC；GCC 专用的前端、wrapper 环境、collect2 和 sysroot 查询按实际 producer 使用。compiler protocol 升至 25，工具记录直接更新。新增真实 C ABI 双向调用、Clang PIC／TLS、LLVM unwind 链接及 C／C++ 混合 family 测试。本机 LIR 379、protocol 47、target-data 10、toolchain 53 项通过，格式化和 workspace lint 通过。原有 AArch64 系统 Cone 全部准备成功；runtime builder 曾读到开发中的中间状态，完成修改后单独补跑四种 runtime 配置，全部成功。

用户调整验证顺序前已启动的 Linux 小批检查中，lint 通过、测试返回失败，未继续排查或扩大远端测试。工作副本与日志保留于该机仓库 `tmp/target-refactor/`，将在全部本机验证完成后的 R7 一并闭合。Darwin 配置批次推送后已获取远端 main，无新增提交。

### R5 target sysroot include

`native.include` 增加 `{ sysroot = "backend/include" }`，以选中 Scoop target 的资源目录为根。普通路径、目标条件与声明顺序保持，C／C++／Objective-C 和 RC-only 程序共用解析；实际头文件仍沿既有输入更新记录参与复用。内置 native 库移除重复目标路径表，OpenSSL wrapper 直接包含所选生成头，TLS fixture 同步删除旧头复制与不再使用的资源目录链接，并修正测试 Cone 已有的重复依赖声明。

compiler／package protocol 更新至 26／4，manifest 与 native 对象／资源记录直接演进。移除无生产消费者的 `PreparedNativeInputs` Wire 编码及冗余配置副本。格式化与 workspace／all-targets Clippy 通过；manifest、protocol、PM、toolchain 的 245 项测试通过，包含真实 include 优先级、输入变化、复用和 sysroot 搬迁验证。配套 release CLI 的两个 native include fixture 与 TLS 系统验证 fixture 通过，RC-only 实机验证留待 R7 Windows。R4 推送后已获取远端 main，无新增提交。

### R5 维护与 fixture 流程

目标选择采用宿主的明确默认值，执行能力只在需要运行目标程序时判断；`--compile-only` 与 `requires_target_execution` 保留完整用例并单列未运行结果。Darwin companion 使用所选 compiler、SDK、machine 与部署版本，Linux Clang companion 接收实际 target／sysroot。CMake native 依赖及 LLVM／LLDB 的 CPU 参数与 Apple 架构名按目录中的架构选择，不再以 Darwin 推定 AArch64。

Ruff 0.16.10 通过；76 项 fixture 规则与 22 项维护脚本测试各跳过 3 项宿主不适用测试，其余通过。本机显式选择 Linux GNU 的 3 项 manifest 诊断 fixture 通过，需要执行的 native fixture 明确记录为 `execution-skipped`；这不是 Linux 运行验收。配套 release CLI 完成 AArch64 的 27 个内置 Cone 与四种 runtime 配置准备。include 批次推送后已获取远端 main，无新增提交。
