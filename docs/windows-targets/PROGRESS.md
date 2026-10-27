# Windows targets 实施进度

分支：`codex/windows-targets`。按 DESIGN.md 的 W0～W4 完成全部三个 target；只有完整编译、GC／异常、系统库与宿主验收通过后才声明正式支持。

| 阶段 | 状态 | 尚需闭合 |
| --- | --- | --- |
| W0 工具链与 ABI | 关键路线实验通过 | 复现入口与范围见下文；正式编译器集成在 W1／W2 完成 |
| W1 编译／链接 | 实施中 | ABI 分类与 consumer 表示已补齐；继续 target identity、机器发射、COFF／PE、工具选择与链接 |
| W2 core runtime | 待实施 | Win32 平台层、SEH、精确根、Unicode 启动与完整 core／std |
| W3 系统库 | 待实施 | 平台规范与实现、IOCP、全部 native 组件、真实 consumer |
| W4 宿主与发行 | 基础实施中 | 进程中断已实现；继续 Windows 命令、维护脚本、DLL／SDK 布局与打包、交叉编译 |

本地实现和完整验收完成后，才在 Linux `nuc12w.0d0a.com` 与 macOS `mm.0d0a.com` 的 `~/repos` 下新建测试目录，运行与变更范围相称的回归。原始日志与构建目录放在仓库外。

## 本地环境

- Windows x64 `10.0.26300`；MSVC toolset `14.44.35207`，实验选择 SDK `10.0.26100.0`（同时安装 `10.0.22621.0`）。
- LLVM `22.1.8`（`ca7933e47d3a3451d81e72ac174dcb5aa28b59d1`）。
- LLVM-MinGW `20260616`（LLVM `22.1.8`），分别准备 MSVCRT 与 UCRT prefix。
- Rust `1.99.1`（`58cf8f9c3`）；三个 Windows Rust target 分别验证 native 依赖，不能从 C probe 成功推断文本库兼容。

这些是实验环境记录，不是发行最低 OS／SDK 承诺，也不代替实际测试结果。

## 已完成的定向验证

- 三个 native 环境均在本机 Windows 成功执行真实 LLVM `invoke`／catch-all／cleanup／`resume`，使用 production Scoop personality 与 LSDA decoder；检查原异常 record、selector、一次 cleanup 与一次删除。
- 同一 decoder 的边界、溢出、action chain、非法 encoding 与 typed catch 反例均通过；Windows null type entry 使用实际 COFF emission 的 `absptr`／8 bytes，Unix 保持 `0x9b`／4 bytes。
- `llvm-readobj` 检查三个实验程序的真实 CRT imports：MSVC 与 MinGW UCRT 使用 UCRT，MinGW MSVCRT 使用 `msvcrt.dll`。
- MSVC 环境的 LLVM libunwind 源码用 Microsoft headers 与 `/MD` 构建成功；生产准备仍须把实验中的明确配置接入上游 CMake，不能复用 MinGW archive。
- C 变更先格式化并以 `-Wall -Wextra -Werror` 检查；实验 Python 与公共 fixture runner 通过 Ruff 0.16.10 format/check。

复现入口：[W0 experiments](../../spikes/windows-targets/README.md)。

第二批 W0 实验已通过：

- 三环境使用真实 zero-live statepoint invoke 与 Scoop SEH 同时发射、链接、catch／cleanup／resume。
- 三环境各测试保留／省略 frame pointer 的 16 KiB 栈帧；通过系统 `RtlVirtualUnwind` 和 production stackmap reader 找到精确槽，更新根后 LLVM 函数返回新地址。Win64 不使用 `FP = SP + stack_size - 8` 的 SysV 推算。
- MSVC 用 cl 产生同一真实 C++ 静态 `.lib` 和 DLL import library，分别由 cl/link.exe、clang-cl/lld-link 构建 wrapper 并消费；STL、RTTI、两条 native throw/catch、静态初始化／两次析构与 Scoop SEH 同处一个程序通过。两个 MinGW CRT 的 libc++ 路线同样通过。
- 三环境均验证 LLP64、Win32 线程、独立 TLS／errno 和私有 C locale。
- 三个 Rust target 的真实文本 staticlib 通过 NFC、IDNA、转换器创建／处理／释放及 Scoop SEH 组合；实际 imports 与所选 CRT 一致，无 libgcc DLL，也未在文本 archive 中发现第二套 `_Unwind_*` 定义。固定 Rust `1.99.1` 的 MSVCRT 路线无需重建 std。

这些是后端机制与依赖闭合实验，尚不包含完整 compiler pipeline、managed heap／GcHandle、GC 协调、callback 或系统库；不据此开放正式 target。后续按 W1～W4 完成全部设计与验收。

## W1 ABI 数据与分类

Win64 aggregate 分类已按三个环境的实际 Clang 22.1 输出实现：1／2／4／8-byte 整数 carrier、其余 caller copy，以及小整数／Boolean 扩展规则。interface 参数保留两个 pointer carrier，结果计划使用保留 scan 的间接 storage；canonical ABI 与 consumer 绑定复用现有 Indirect 表示。ABI-plan、identity、LIR 的 750 个单元测试与 1 个文档测试通过，相关 crate 的 clippy 无告警。完整 target 选择和 LLVM 属性发射随后接入，当前仍未开放 Windows CLI target。

COFF reader 已覆盖普通 AMD64 对象及 MSVC bigobj，保留长节名、超过 255 的 section 序号、aux symbol 槽、COMDAT association、weak external fallback、`.pdata`／`.xdata` 引用和隐式 addend。新增 relocation form 参与已有引用闭合、stackmap 指针读取与 canonical fingerprint；现有目标的编码保持不变。使用 LLVM 22.1 汇编器与 cl `/bigobj` 的真实对象验证，并检查越界 relocation、aux symbol 引用和目标机器不符。`scoop-slib`、ABI-plan、target-data 的 260 个测试通过，clippy 无告警；测试以 `SCOOP_TEST_CLANG` 指定 Clang，并在 PATH 中提供 cl。

本次同时闭合已有测试问题：Windows 在打开目录时返回 PermissionDenied，Unix 在读取目录时返回 IsADirectory；slib 的三处固定向量同步到此前已采用的 runtime ABI 21，与 LIR 的既有向量一致，未修改 runtime ABI 或现有 target fingerprint。COFF 的完整 section inventory、代码发射和最终链接仍按 W1 后续步骤完成。

## W4 宿主基础

`scoop-process` 已实现 Windows console handler，按实际 process handle 终止 child，关闭控制台时在 handler 返回前等待 child；中断后停止启动新工具，普通退出保留完整 32-bit 状态。真实子进程测试覆盖 Ctrl+C／Ctrl+Break／close 三个 handler 分支、两项并行 child、drop 回收、中断拒绝新任务与高位退出码透传，在 Rust MSVC 与 gnullvm 两种宿主 target 上通过；相应 Clippy 无告警。完整 CLI 的系统控制事件验收随宿主命令完成，不以这里的 handler 调用测试替代。

宿主 LLVM 22.1.8 已用 LLVM-MinGW UCRT 完整构建并安装 LLVM／Clang DLL、Rust 开发输入及 Clang／LLD／对象工具。修复遗漏 `libclang-cpp.dll` 导致误加载 PATH 中 ESP32 DLL 的问题，并将 inkwell 的 target feature 与实际 AArch64／X86 backend 对齐。安装后的 Clang 生成 AMD64 COFF 成功；gnullvm 宿主的真实 codegen 测试通过 O0／O2 的 statepoint／异常发射及 AArch64／X86 Boolean ABI。相关 Rust Clippy、Python Ruff 与三个脚本测试通过。

Windows 构建选择支持 UTF-8 的 Ninja；实际编译器依赖记录包含 Unicode 外部头文件，修改头文件会使结果更新。完整构建后的原命令直接复用，保留源码和工具，确认安装工具、DLL 与 Ninja 输出 metadata 未改变。日志、源码解包和构建目录均在仓库外。
