# Windows targets 实施进度

分支：`codex/windows-targets`。更新于 2026-10-29；已整合主线 `34c78ad3c`，包含原生调试、Throwable 栈快照开关与编译期输入 intrinsic。
本文只记录当前实现、验证结果和未完成项；设计合同见 [DESIGN.md](DESIGN.md)。

| 阶段 | 当前状态 | 剩余验收 |
| --- | --- | --- |
| W0 工具链与 ABI | 三种 native 环境的关键路线均已通过，正式准备脚本已接入 | 无独立 W0 阻塞 |
| W1 编译／链接 | 三目标 ABI、COFF、C/C++、PE、PDB、subsystem 与 RC 已实现；ABI 30 定向组合通过 | 无独立 W1 阻塞 |
| W2 core runtime | Win32 primitives、SEH、精确 roots、GC、线程与初始化已实现；ABI 30 栈诊断开关组合通过 | 无独立 W2 阻塞 |
| W3 系统库 | Windows 实现及所需 native 后端已完成，下表各组通过 | ABI 30 资源重建与高层 HTTP client／server 验收进行中 |
| W4 宿主与发行 | Windows 宿主、命令发现、私有 LLVM、SDK 选择和发行入口已实现；首个实际安装包通过三目标使用验收 | 使用最新 core／库重新生成最终归档并复测；随后进行跨宿主回归 |

本地实现和完整验收完成后，才在 Linux `nuc12w.0d0a.com` 与 macOS `mm.0d0a.com` 的 `~/repos` 下新建测试目录。目前尚未开始远端测试。原始日志、报告、归档和构建副本保存在仓库外。

## 环境与三个目标

- Windows x64 `10.0.26300`；MSVC toolset `14.44.35207`，SDK `10.0.26100.0`。
- LLVM `22.1.8`；LLVM-MinGW `20260616`，分别提供 MSVCRT 与 UCRT prefix。
- Rust `1.99.1`；Windows 宿主使用 `x86_64-pc-windows-gnullvm`。
- 合并后的 runtime ABI 为 30，子进程协议为 23。原先 ABI 28／29 的通过记录保留为历史验证；当前配套编译器、runtime 和系统库需要一起重建。上述版本是实际构建／验收环境，不是未经验证的最低支持环境承诺。

正式 target 为 `x86_64-pc-windows-msvc-ucrt`、`x86_64-w64-windows-llvm-mingw-msvcrt` 和 `x86_64-w64-windows-llvm-mingw-ucrt`。两种 UCRT target 的 native ABI 和资源保持独立。W0 复现入口见 [experiments](../../spikes/windows-targets/README.md)。

## 编译器、runtime 与构建

Windows final linker 保留显式指定的输出路径，包括无扩展名的文件。五种 native 工具组合的实际执行／PDB 检查，以及 Atomic 跨产物 CLI 重链接用例通过；MSVC 与 MinGW 驱动的自动 `.exe` 补全不再改变临时输出名。

三个环境的实际程序已覆盖 Win64 标量／aggregate／sret、C 与 Scoop 双向调用、跨 Cone 与 callback、普通 COFF／bigobj／COMDAT、短／长 import library、混合 archive、DLL 数据、Unicode 启动参数、CRT imports 和目标不匹配诊断。Microsoft clang-cl／cl、LLD／LINK 及两个 MinGW CRT 的五种 C++ 组合通过 STL、RTTI、内部异常、静态初始化／析构和 native backtrace。Debug 包含 CodeView 与相邻 PDB，函数入口的源文件／行定位已验证。

SEH personality 与 LSDA decoder、真实 statepoint invoke、多层／大栈帧的系统 unwind、移动根恢复、throw／catch／finally／rethrow、初始化失败、线程异常根、park／TLS／callback／shutdown 与并行 GC 已通过定向测试。GC stress 只用于相应正确性用例。Windows 的锁、condition、once、线程、对齐分配和计时使用 Win32／CRT primitives，Unix 保留原行为。

`Atomic<T>` 的竞争让出在 Windows 调用 `SwitchToThread`，Unix 调用 `sched_yield`；互斥与内存序合同不变。三个目标的并发、跨库产物和值存储 fixture 均通过。ABI 28 的 core consumer 每目标 51 个容器／82 项测试、std 每目标 97 个容器／104 项测试已通过。

`Cone.toml` 的 `[windows]` 支持 `subsystem = "console" | "gui"`，缺省 console，以及可选 `rc` 文件。资源编译器随 Windows toolchain 提供或从外部发现。manifest、产物、实际 PE subsystem／资源、普通更新与缓存复用均已验证。Clang 参数使用兼容 Windows 的普通路径拼写，包含中文／空格及 Rust canonical path 的实际 C++／PDB 回归通过。

Windows 主驱动的安装名为 **`sco.exe`**，帮助和版本信息使用 `sco`。Cargo 内部 binary 名、其他可执行程序、外部命令 `scoop-` 前缀和 `SCOOP_*` 环境变量不变；macOS／Linux 继续使用 `scoop`。归档布局测试覆盖三个宿主，实际 Windows `sco new/run/test`、`sco pm`、`sco example` 均通过。

主线调试整合复用共享 DebugOrigin／变量记录，并将 Windows PDB 接入统一伴随文件发布。原生栈诊断使用所选 LLVM unwinder 与串行 DbgHelp 查询；三个 Windows target 的 O0／O2、中文源码／安装路径、实际 PDB 行号、无 PDB 地址输出、并发查询和模块卸载后的历史地址测试已通过。Throwable 历史、并发首次抛出与 GC 组合已在 ABI 30 的三个目标定向验收中通过。

ABI 30 的程序级栈诊断开关通过 Windows UTF-16 入口原样传递，release 默认关闭、debug 默认开启，显式设置覆盖默认值。四种原生链接组合验证开／关参数、Unicode、PE／PDB 与 trap；6 项 runtime ABI、3 项开关默认值／CLI 测试、workspace Clippy 和 70 项 fixture runner 规则测试（3 项宿主跳过）通过。完整资源重建前保留此前测试产物，新资源位于独立外部目录，复用未变化的 native 后端。

ABI 30 的三个目标各通过 13 项定向 fixture，覆盖开关与重链接、Throwable 历史／并发／GC、三种调试级别、Unicode 启动参数、PDB、RC、subsystem、native DLL 数据、终端及四种编译期输入。MSVC 首轮两个缺少配套库的用例在准备 platform／terminal 后定向复测通过；两个 MinGW 分别完成 11 项公共组合和 2 项终端／并发组合。

合并后的 12 项 Windows toolchain 测试、8 项 Windows codegen 测试和 4 项通用调试元数据测试通过。C 桥额外覆盖三个 target 的 debug／release 展开 atom 与真实执行；PE 链接覆盖 LLD／LINK、Unicode 参数、相邻 PDB、CRT 和新的 trap 栈输出。导入 callable／constructor 的声明位置进入共享调试记录，当前／导入来源经 HIR→MIR、Unicode／CRLF 坐标及泛型 constructor 回归通过。fixture runner 当前 70 项规则测试完成，其中 3 项按宿主跳过。

编译期输入主线整合后，47 项协议、11 项产物固定向量、7 项缓存键及 8 项 Windows codegen 测试通过。Windows 环境输入覆盖缺失、空值、Unicode 和无效 UTF-16；child 复用原有 Windows 工具环境，并保留调用者的 `SCOOP_*` 值。macOS 打包采用主线修正后的 Rust 默认 CPU baseline，Windows 宿主命名和 target 选择保持独立。

文件 fixture 的 Windows 扩展仍在验收。ABI 29／协议 23 下新增验收的 107 个诊断 fixture 全部通过，覆盖 smart cast、类型别名、尾随 lambda、可见性、源码头、UTF-8 与 when 规则；共享预编译 core，共 107 次 CLI 调用。公共 runner 的格式化／Ruff 通过，68 项规则测试完成（其中 3 项按宿主跳过）。33 个继承规则用例复用固定的共享 core 后全部通过；6 个 primitive inherited-method 用例也已通过，其中五个共享固定输入，combined 保留修改前后的真实 core 构建。6 个根类型错误用例精确断言被测规则诊断，去除对其他 core 源码连带错误的完整快照依赖，均已通过。较大的完整 core 构建步骤采用 900 秒测试时限；普通运行断言不变。Windows 本地化错误、LF 字节位置、COFF archive 成员、当前 runtime 字段以及泛型初始化登记的断言按实际合同调整，不通过隐藏输入或工具验收复用。

上述 Windows 扩展在 ABI 29／协议 23 下完成默认 MSVC 目标的 403 项实际验收：107 个诊断用例、292 个语言／跨产物用例和 4 个编译期输入用例全部通过。结构装箱用例的编译、产物消费和运行通过后，移除与该行为无关的完整归档成员数量快照，定向复测通过；归档格式继续由专门产物测试覆盖。没有将该结果扩大为 ABI 30 或其他目标的完整重复验收。

## 系统库与 native 后端

以下结果来自 ABI 28 的三个 Windows target 正式 release Cone，测试源码和工具保持存在；仅列主要计数。

| 验证组 | 每目标结果 |
| --- | --- |
| platform | 42 个容器／148 项通过 |
| asyncio | 63 个容器／179 项通过；含文件、网络、取消、host、TTY 与 consumer |
| terminal | 17 个容器／63 项通过 |
| readline | 11 个容器；39 项通过，6 个子进程入口由父测试实际启动 |
| regex / PCRE2 | 4 个容器／19 项通过 |
| crypto / OpenSSL | 10 个容器／25 项通过，consumer 5 项通过 |
| TLS／异步 TLS | 分别 28／22 项通过，两组 consumer 各 3 项通过 |
| IDNA／URL／charset | 分别 11／19／18 项通过 |
| compression | 19 个容器／49 项通过 |
| HTTP 协议层 | 99 个容器／303 项通过 |
| math 系统差异 | 16 项标量与 2 项预编译 consumer 通过 |

Windows 文件／路径／环境／进程采用 Unicode 系统接口；网络使用 Winsock／IOCP，测试覆盖取消、迟到完成和缓冲区保活。TTY／readline 使用真实隐藏 console 和 ConPTY，覆盖模式恢复、Unicode、resize、粘贴、Ctrl-C／EOF、补全与跨线程输出。Unix 专属 FIFO、SIGPIPE、PTY job control 按平台选取。

PCRE2 10.49、OpenSSL 3.5.9、uriparser 1.0.2、Rust 文本后端、Zstandard、zlib-ng、Brotli、nghttp2 与 LLVM libunwind 已为三个 target 分别准备；Microsoft 使用动态 CRT，两个 MinGW 使用各自 CRT prefix。实际 imports 符合所选 CRT，不引入 libgcc DLL。未变化的准备输入直接复用，普通应用消费预编译资源。

数学库保持系统行为，不加入结果修正 workaround。规范、设计与库文档记录 C pow 与 Java Math 的差异，以及所用 LLVM-MinGW MSVCRT 的 FMA 中间溢出；Microsoft `_hypotf` 薄 wrapper 仅适配导出名。

终端异步、log、console log 和 CLI 的 MSVC／MinGW UCRT 测试已通过。JSON 完整 API 在默认 MSVC 目标上 71 个容器／101 项全部通过。HTTP asyncio 在 ABI 29 的默认 MSVC 目标运行 55 个容器／145 项，144 项通过；唯一失败是收到连接错误便假定异步关闭已完成的测试时序。按既有 isClosing／isClosed 合同显式等待 close 后，原测试源码与必要 helper 的小型 Cone 定向验证 2 项全部通过，失败闭合；库行为未改。HTTP client／server 及 consumer 继续验收。此前并行大型编译触及内存上限，已降低并发；没有修改编译器语义或增加资源预算机制。

## 发行与剩余工作

打包入口组装五个宿主命令、递归解析的宿主 DLL、私有 LLVM、两个 MinGW CRT prefix、三个目标的 slib／runtime／unwind、实际许可证、三个 API 文档站点、VSIX、示例和编程 skill。源码副本排除生成的 target 和 Cone.lock。10 项打包测试、Python Ruff 和 workspace/all-targets Clippy 已通过。

完整 LLVM／Clang SDK 与启用 Python 的 Windows LLDB 已构建完成。SDK 包含 LLDB 构建所需的 llvm-tblgen、TableGen／Support／Demangle／Debuginfod 静态库；Windows Python binding 使用 `.pyd`。打包允许 Python 安装中内容相同的 VC runtime DLL 副本，同时保留真实同名冲突诊断。

所有宿主公开 `bin` 中的 LLVM／LLDB 命令统一带 `scoop-` 前缀；编译器相邻调试工具查找、VS Code 默认 adapter、性能分析脚本与文档已同步。Windows 主驱动仍为 `sco.exe`，其他宿主仍为 `scoop`。Windows launcher 的 Unicode／空格路径迁移、参数、环境、退出码及公开文件名集合通过真实子进程验证；POSIX launcher 用例按宿主跳过。VS Code 16 项测试、相关源码 lint／类型检查、workspace Clippy、7 项性能分析规则和 4 项 DLL 打包测试通过，VSIX 已重新生成；整个 tooling 的 Markdown 格式检查仍有既有差异。

真实调试器包迁移到中文与空格目录、仅保留发行 bin 和 Windows 系统 PATH 后，6 个 LLVM 命令、LLDB 的私有 Python／原生模块／formatter 与 DAP initialize 均通过。MSVC/PDB 完整 launch 会话的源码映射、函数名、断点／单步、参数、全局／修改后的 TLS、String／数组／enum／对象、闭包及 moving GC 引用检查通过；离线原生符号名／源码名断点与真实类型布局检查通过。修复 CodeView 的 uintptr 编码、PDB reader 的零大小字段丢失和 TLS 模板误读；CodeView 无法描述的部分计算值／聚合分片仍显示 unavailable，不加入调试专用存储或保活。4 项后端调试测试与 workspace Clippy 通过。

ABI 30 的三个 target 均通过上述真实 DAP launch 与离线 formatter 检查。MinGW CRT 同时携带 DWARF 时，LLDB 现在优先选择程序匹配的 PDB，解决源码断点与变量被 CRT 调试单元遮蔽的问题。真实混合格式程序、普通 MSVC/PDB 程序及只有 DWARF 的 PE 均通过；不删除调试节，不改变非 PE 的选择。上游补丁通过干净源码应用检查及 C++ 格式化，配套 LLDB 已重新构建。

首个归档已解压到中文及空格路径。在仅含安装 bin 和 Windows 系统目录的 PATH 下，三个目标的默认 SDK／工具发现、new/run/test、C++、console／GUI 和 RC 均通过；检查实际 PE imports 确认所选 CRT。随后修复了 canonical native sysroot 路径，并用更新的程序完成实际安装目录回归。该原型归档不是最终发行物。

剩余工作为：完成 ABI 30 三目标预编译资源重建与剩余高层库测试；重新生成并验证最终 release 归档；提交、推送并合并主线；最后执行设计约定的 Linux／macOS 回归。完成这些步骤前不声明全部 Windows 验收完成。
