# Windows targets 实施进度

分支：`codex/windows-targets`。按 DESIGN.md 的 W0～W4 完成全部三个 target；只有完整编译、GC／异常、系统库与宿主验收通过后才声明正式支持。

| 阶段 | 状态 | 尚需闭合 |
| --- | --- | --- |
| W0 工具链与 ABI | 进行中 | 三环境 C／C++、真实文本后端、Scoop personality／statepoint 与 frame 几何 |
| W1 编译／链接 | 待实施 | 完整 target identity、Win64 ABI、COFF／PE、工具选择与链接 |
| W2 core runtime | 待实施 | Win32 平台层、SEH、精确根、Unicode 启动与完整 core／std |
| W3 系统库 | 待实施 | 平台规范与实现、IOCP、全部 native 组件、真实 consumer |
| W4 宿主与发行 | 待实施 | Windows 命令、维护脚本、DLL／SDK 布局与打包、交叉编译 |

本地实现和完整验收完成后，才在 Linux `nuc12w.0d0a.com` 与 macOS `mm.0d0a.com` 的 `~/repos` 下新建测试目录，运行与变更范围相称的回归。原始日志与构建目录放在仓库外。

## 本地环境

- Windows x64；MSVC toolset `14.44.35207`，已安装 SDK `10.0.22621.0`／`10.0.26100.0`。
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

复现入口：[W0 experiments](../../spikes/windows-targets/README.md)。这批验证尚不包含 managed GC、native C++ 与 Scoop handler 共存、Rust 文本后端或正式 target 消费；W0 保持进行中。
