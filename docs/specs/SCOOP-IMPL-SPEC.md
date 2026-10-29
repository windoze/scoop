# Scoop 编译器与产物规范

本文规定编译阶段的输入输出、实体身份、目标 ABI、`.slib` 格式、构建命令、缓存与链接契约。源码语义见 [语言规范](SCOOP-SPEC.md)，对象表示、GC 与 native 运行边界见 [运行时规范](SCOOP-RUNTIME-SPEC.md)。

主驱动的安装名称由宿主平台决定：Windows 为 `sco.exe`（命令 `sco`），macOS／Linux 仍为 `scoop`。本文后续的 `scoop ...` 命令在 Windows 上对应 `sco ...`，不随应用 target 改名。其他可执行文件名保持不变，包括 `scoopc.exe`、`scoop-link.exe`、`scoop-lsp.exe`、`scoop-pm.exe` 和外部命令 `scoop-<command>.exe`；例如 Windows 的 `sco example` 调用 `commands/scoop-example.exe`。环境变量和安装目录名称继续使用 `SCOOP_*` 与 `scoop`。Cargo package／内部构建产物仍名为 `scoop`，发行组装时仅将 Windows 主驱动复制为 `bin/sco.exe`。

## 1. 范围与目标兼容性

Windows 三环境扩展按 [Windows targets 设计](../windows-targets/DESIGN.md) 分阶段实施。W0 使用独立原生／LLVM 实验验证 MSVC＋UCRT、LLVM-MinGW＋MSVCRT 与 LLVM-MinGW＋UCRT 的 Win64、SEH／statepoint、C++ 和 Rust 文本后端组合；三个完整 target 随 W1／W2 接入开发 CLI。正式发行前须完成本规范的编译／链接与运行时规范的完整 GC／异常合同及相应系统库验收，不发布关闭异常或缺少精确根的临时 target；当前已验证范围见实施进度。

宿主 LLVM 准备同时安装 LLVM／Clang 动态库、Rust 构建所需的 llvm-config／头文件、Clang／clang-cl／Clang++、LLD 及所需 LLVM 对象／归档／资源工具。LLVM 与 inkwell 均只启用实际使用的 AArch64／X86 backend，不引用未构建 target 的初始化入口。Windows x64 宿主采用 LLVM-MinGW UCRT 开发环境构建，Rust 宿主为 `x86_64-pc-windows-gnullvm`；宿主身份不覆盖应用选择的完整 Scoop target。Windows 使用原生 Ninja 和 `.exe`／DLL 布局，必需的宿主 LLVM／Clang、C++／unwind DLL 与实际引用它们的工具相邻安装；Darwin／Linux 保持各自的 rpath 与动态库规则。开发安装与运行期工具包的组装仍由维护脚本负责，普通编译不构建宿主 LLVM。

Windows 发行准备使用同一 GNU-LLVM Rust 宿主构建主命令、示例命令和内部 runtime 准备程序，交付三个完整 Windows target 的库与 runtime。宿主 Clang／LLD／RC 工具及 Clang resource headers 安装在 `libexec/llvm`；两个 LLVM-MinGW 目标在各自 `lib/<target>/toolchain` 中保留所选 prefix 的 x86-64 开发资源、驱动与目录关系，Microsoft SDK／MSVC toolset 仍由构建机提供。目标 libunwind 随各自目录交付；宿主与目标 LLVM／MinGW、C++／unwind 的实际许可一并收集。发行前的 API 文档生成沿用相同 target 与 native sysroot 选择。

发行脚本默认显式准备目标资源，也可通过 `--prepared-sysroot` 消费维护者已经准备的 sysroot，跳过 native 后端、系统 Cone 和 runtime 的重建；后续命令构建、示例与 API 文档、组装和归档流程相同。该入口是普通产物消费，不增加来源审核或生产过程复核；缺失资源按实际 I/O 报错，编译消费仍执行既有格式／ABI 检查。

一个 Cone 是一次独立编译的单位。library 和 executable Cone 都产生完整 `.slib`；executable 的最终程序由独立链接阶段从产物闭包生成。产物是编译结果，不承担独立分发认证或源码／工具移除后的可用性保证。缓存只判断正常输入是否变化，不检查输出是否遭到外部破坏。

编译器使用自身及所选 C/C++ 工具链成功产生的输出，后续步骤只读取实际需要的数据，不重新证明生产过程。工作文件按普通生命周期管理，不设为只读、不审计权限或文件来源。实际 I/O、编译和链接错误正常报告；失败不发布部分结果。实体身份、ODR、ABI、GC 与读取边界所需的数据继续保留，不以验证资格或生产凭证替代这些普通合同。

编译请求明确选择相容的 LIR target、backend、C bridge toolchain、runtime build 与 final-link 配置。目标目录由 driver 相关代码维护，typed target/backend/toolchain identity 只规定规范化名称与版本的格式；格式合法不代表相应消费者支持。native 符号的对象前缀由选中目标的数据显式传入；identity 解码只检查身份和符号字节格式，目标消费边界按源码符号重建并核对归一化结果。LLVM/backend 和具体 ABI 实现由 codegen 选择，对象读取与最终链接由 slib/linker 的有限支持集合决定，未知或不相容组合必须明确诊断，不回退到宿主目标。阶段只消费其所需投影；library 编译不要求未使用的最终 linker、CRT 或 unwind archive，链接只执行其实际需要的工具，不通过移除源码编译器或 LLVM 工具证明隔离能力。

| canonical target triple | LIR target profile | 对象格式 | 平台契约 |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | `org.scoop-lang.target-profile/darwin-aarch64/1` | Mach-O relocatable | Darwin/AArch64 |
| `x86_64-unknown-linux-gnu` | `org.scoop-lang.target-profile/linux-x86-64-gnu/1` | ELF64 relocatable | Linux/amd64、glibc |
| `x86_64-unknown-linux-musl` | `org.scoop-lang.target-profile/linux-x86-64-musl/1` | ELF64 relocatable | Linux/amd64、musl |
| `x86_64-pc-windows-msvc-ucrt` | `org.scoop-lang.target-profile/windows-x86-64-msvc-ucrt/1` | AMD64 COFF | Windows/amd64、Microsoft UCRT |
| `x86_64-w64-windows-llvm-mingw-msvcrt` | `org.scoop-lang.target-profile/windows-x86-64-llvm-mingw-msvcrt/1` | AMD64 COFF | Windows/amd64、LLVM-MinGW MSVCRT |
| `x86_64-w64-windows-llvm-mingw-ucrt` | `org.scoop-lang.target-profile/windows-x86-64-llvm-mingw-ucrt/1` | AMD64 COFF | Windows/amd64、LLVM-MinGW UCRT |

这些目标采用 little-endian、64-bit data/code pointer 与 16-byte 调用栈对齐。合法非 null pointer 可经内部 64-bit carrier 逐 bit 往返，null carrier 全零。data、code、managed 与 metadata pointer 的语义仍不同，不因此增加源码整数转换。

Windows x64 宿主默认选择 `x86_64-pc-windows-msvc-ucrt`，不根据构建 Scoop 自身的 Rust ABI 或 PATH 上的 compiler 改选应用 target。显式 Windows target 使用上表完整名称，不将 LLVM machine triple 或缺少 native 环境／CRT 的简称当作别名。开发中的 Windows 系统库及发行支持范围按 [Windows 实施进度](../windows-targets/PROGRESS.md) 标明，不从 target 可解析推定全部库已通过验收。

backend contract 使用 LLVM 22.1、LLVM stackmap v3 和本规范的 GC/EH 约束。Linux 两个 libc target 共用 `org.scoop-lang.backend-profile/llvm-22-1-linux-x86-64/1`，但 native、runtime 与最终链接输入不能混用 glibc/musl。target、对象格式、调用约定、pointer/storage ABI 及实际 producer 配置必须相容；不支持的组合在生成目标产物前诊断。

Windows 后端数据使用三个完整 target profile，机器 triple 分别投影为 MSVC 的 `x86_64-pc-windows-msvc` 或两个 MinGW 环境的 `x86_64-w64-windows-gnu`。三者共享 `org.scoop-lang.backend-profile/llvm-22-1-windows-x86-64/1`、Win64 数据布局与 COFF 机器发射，完整 target identity 仍参与产物相容性和缓存。Scoop ABI classifier 新编码 5 表示 Win64 caller copy／interface 间接结果，native symbol normalization 新编码 3 保留 x64 COFF 原符号字节。backend 的 frame recovery 编码 2 表示通过 SEH 重建 RSP／RBP，寄存器仍为 LLVM 编号 7／6、root slot 8 bytes，不保存 SysV frame-record 几何；LSDA 为 LPStart `0xff`、type table `0x00`、call-site `0x01`，无 type table 时为 `0xff`。现有目标编码与 fingerprint 不变。

Windows stackmap reader 验证 location kind、寄存器、8-byte slot／对齐和 pair 结构，保留 RSP／RBP 的有符号 offset；不从 stack size 推造 RBP 位置。扫描时由运行时使用实际 SEH context 计算地址，并执行真实线程栈范围检查。编译产物读取不另建一套 SEH unwinder 重放系统 frame 恢复。

Windows statepoint 的 instruction offset 必须指向 call 的真实返回地址。LLVM 22.1.8 的 WinEH emitter 在部分 call／invoke 后插入必要的 NOP；配套 LLVM 修正为先登记 statepoint 标签，再发射该 NOP，保持 EH region 与 epilogue 行为。此修正适用于所有发射 Windows 对象的宿主；runtime 继续精确匹配 PC，不尝试相邻地址或跳过未登记 frame。

Windows executable 的 `[windows].subsystem` 与 `[windows].rc` 按语言规范 12.2 和 [Windows 设计 3.4](../windows-targets/DESIGN.md#34-windows-executable-配置与资源) 实施。subsystem 默认为 console，可选 gui；工具链提供或发现 RC compiler，使用所选目标 headers／预处理环境编译资源。executable `.slib` 保留最终链接所需的 subsystem 和资源产物，artifact-only linker 不重新读取 RC 源码。RC、实际 include 与引用资源以及工具选择进入普通更新记录；命中不读取资源内容或探测工具。

Windows generated-C producer 使用所选完整 target 的 Clang 模式，平台合同新增编码 3，记录实际 Clang 数值版本与预处理配置；宿主路径只保留在普通工具发现记录。三环境各有独立 C producer profile，Clang 接受的 machine triple 不取代完整 target。公共 flags 保持 C11、所选优化、无 debug／common／compiler-ident／stack-protector／builtin substitution；Windows 使用 x86-64 baseline、Windows 10 API 基线和 SEH unwind tables，Microsoft 环境使用动态 UCRT（`/MD` 等效设置）。新增 flag 编码 17、18、19 分别表示 Windows SEH、所选 Windows CRT 和 Windows 10 基线，原平台编码不变。

generated-C 对象中由 Clang 产生的 `.pdata`／`.xdata` 保留实际 Windows unwind 分类、内容和重定位，交给原生 linker 处理；它们不要求另建 Scoop definition atom，也不进入 Scoop atom 的 relocation 所属检查。COFF 格式、引用边界和桥接代码中的引用检查保持。Scoop LIR 发射的 unwind atoms 仍按自身 definition plan 关联和验证，不能将其未归属 relocation 视为 generated-C 辅助内容。

Windows 内部 C 对象（含 generated-C 和最终 startup）以 `-mno-incremental-linker-compatible` 关闭 COFF 当前时间戳，保持重复编译和等价重链接的确定性；最终 PE 本已关闭 incremental linking。该 producer flag 使用新增编码 20，进入 Windows C producer profile，既有平台编码不变。此配置不移除 SEH 表，也不在读取缓存对象时改写或归一化对象内容。

runtime index 的补充 deployment 值 0 表示使用完整 target 已规定的 OS 基线，Linux 与 Windows 均使用此值；Darwin 继续保存所选最低 macOS packed version。target 相容性仍由已有完整 identity 检查承担，不在补充 deployment 字段重复编码 OS／CRT。

Windows runtime 的普通 COFF 对象可包含 CRT headers 生成的重复可合并 helper（如 inline stdio 配置函数和 MinGW refptr）。runtime index 合并符号目录时允许类型／只读属性一致的 weak 定义重复，实际 COMDAT 选择仍由 PE linker 执行，所有原始对象保留；runtime ABI 入口仍要求 strong 定义，重复 strong 定义仍报错。程序输入汇总复用已合并的 runtime 目录，不再次将合法重复判为冲突；用户 native 对象与 runtime 之间的同类 COFF weak helper 也按相同规则合并。

MinGW 的 native sysroot 是对应 CRT 的完整 LLVM-MinGW prefix。Microsoft 环境可由已安装 Build Tools／SDK 发现，也可在显式 native sysroot 的 `msvc.toml` 中提供 `[msvc]` 表的 `vc_tools`、`windows_sdk`、`sdk_version`；前两项分别指向所选 `VC/Tools/MSVC/<version>` 与 Windows SDK 根，支持相对于该配置文件的路径。跨宿主使用同一显式布局。发现阶段以实际 headers 编译 AMD64 COFF probe，检查 Win64／LLP64、Microsoft 或 mingw-w64 环境及 `_UCRT` 对应关系；不执行目标程序，也不预先要求最终链接库齐全。配置、选中工具与实际头文件路径按既有输入更新规则记录和复用。

仓库的 `scripts/release.toml` 记录发行所用 LLVM 精确版本、系统与 CPU 基线。构建宿主命令时将默认 Darwin 部署版本编入工具；普通命令不读取打包配置或发行清单。`MACOSX_DEPLOYMENT_TARGET` 可为一次编译／链接选择明确的 `major.minor[.patch]`，非法值必须诊断。未覆盖时使用编入的基线，不查询运行机器的系统版本。所选最低版本贯通 LLVM 对象、generated-C、用户 native、runtime 和最终链接，进入原有生产配置与更新键；SDK 版本继续只记录实际 SDK。Linux 使用既有 x86-64 CPU 基线和所选 libc 工具链，发行最低 libc 版本由对应构建／验证环境确定，不能仅靠一个版本参数降低实际动态依赖。

宿主 Rust 命令使用 rustc 对应目标的默认 CPU 基线，不使用构建机器的 native CPU，也不以 Scoop backend 的 `generic` 配置覆盖平台必需特性；Darwin/AArch64 因而保留 Apple Silicon 的 AES／SHA2 等平台特性，Linux/amd64 仍使用 x86-64 基线。

三个宿主命令的 `--version` 使用同一份构建版本与源码 revision。维护脚本显式设置发行版本和 revision，普通开发构建使用仓库默认版本及当前 Git revision；源码归档没有 Git 信息时明确显示 revision 未提供。版本文本不替代配套工具的协议协商或产物兼容判断。

Windows 发行包按宿主 PE 的实际 imports 收集非系统 DLL，递归包含其依赖，并放在对应宿主 executable 的相邻目录；内置外部命令遵守相同的加载规则。Windows 基线自带的系统 DLL 与 API-set 不复制。LLVM 的 DLL 优先从本次选定的 LLVM 安装取得，宿主命令启动不依赖开发机 PATH 中的另一份 LLVM 或 MinGW DLL。目标开发资源及 SDK／toolset 的选择仍由各 target 的 native 构建合同负责。

Linux GNU 直接执行所选 GCC driver，不注入 musl wrapper 专用的 `REALGCC`；musl 将显式 `REALGCC` 或 PATH 中的 gcc 作为 wrapper 的实际 compiler。两者均通过真实 header/object probe 检查 libc 与机器，误选另一 libc 的 driver 必须得到不匹配诊断。

两个 Linux 目标的默认 C／C++、runtime 与 startup 编译显式使用 `-march=x86-64`，与 LLVM backend 的 CPU 基线一致；该选项属于普通 C 生产配置。用户 manifest 仍可按既有 native 参数规则显式选择自己的附加编译选项。

Native C++ 支持由语言规范 12.2.1 的 `native.cxx` 显式选择，目标环境包括 Darwin/AArch64、Linux/amd64 GNU，以及 Windows 设计规定的三个环境。当前 musl 工具链未配套 C++ 标准库和 ABI 运行库，C++ 与 musl 的静态／动态链接组合均不支持；源码构建和 artifact-only 消费都必须诊断该组合。Windows 正式 CLI 开放仍以完整 runtime 和链接闭环为前提。

## 2. 编译阶段与产物契约

M51 的普通 native include 支持语言规范 12.2.1 的路径字符串或条件路径表。预处理搜索目录、选项投影与 native 对象更新判断只使用当前目标选中的路径，并保持声明顺序；复用现有目标谓词，不引入依赖准备回调或目标变量插值。M63 后原始 manifest 的本机解析记录由 PM 按实际输入更新复用，driver 不再保存原始清单解析缓存；语言实体、`.slib` 与 runtime ABI 不变。

| 阶段 | 输入 | 输出与责任边界 |
| --- | --- | --- |
| parser | 当前 Cone 的完整源码集合 | 带准确源码位置的 AST 与语法诊断。 |
| HIR | 当前 AST 与依赖 Export HIR | 完成源码语义检查，分别输出 ExportHir 和 LocalConcreteHir。 |
| MIR | LocalConcreteHir 与实际选用的依赖 MIR metadata | 完整 concrete 类型、callable、显式控制流及执行 ABI。 |
| LIR 准备 | MIR、实际选用的依赖 LIR metadata 与 codegen 提供的目标布局描述 | 完整布局与逻辑签名输入；复用依赖已经保存的物理 ABI。 |
| ABI 规划 | LIR 定义的布局／签名输入与所选 codegen 目标 | 按完整签名分类的 Scoop/C ABI 计划集合。 |
| LIR 完成 | 准备结果与完整 ABI 计划 | 完整正文、存储、GC roots、符号与物理定义要求。 |
| codegen 发射 | 当前完整 LIR 与相应 producer 配置 | 目标对象、generated-C 源码、实际定义／引用与 GC/EH 数据。 |
| `.slib` | 三层 metadata 与完整对象集合 | 按需消费的确定性编译归档。 |
| program-link | `.slib` 依赖闭包、runtime/native 对象、final-link 配置 | 定义与引用闭合的 executable。 |

stage 只通过对应 IR/metadata 交换数据，不调用上游 stage 的实现。所有源码语义错误在 parser/HIR 结束；后续阶段报告自身的格式、引用、ABI、目标或链接错误。已验证且不变的数据在后续边界复用。

### 2.1 Parser / AST

源码集合非空，文件具有唯一 `SourceIdentity { cone, logical_path }`。manifest Cone 的 logical path 为完整的标准化 Cone-relative source path；默认目录中的文件保持 `src/...`，显式源码根不截短路径或改变相同文件的身份。single-file 的原始入口文件为 `main.scoop`，由输入选择保证；2.20 的生成正文在同一 Cone 中另有按方法身份派生的逻辑来源。SourceIdentity 只组合已有 ConeIdentity 与标准化逻辑路径，不把单文件输入的选择规则施加给所有诊断／正文来源。host 路径只用于本次 I/O 与诊断。parser 只接收按语言规范 12.2.2 及本规范 2.7 选定的完整源码集合，不自行补扫目录。

AST 保留原始源码顺序、byte span、显式类型实参、位置／命名／spread／尾随 lambda 实参，以及独立的声明、表达式、模式和控制转移结构。文件至多有一个开头 package，其后为 import，再后为声明。qualified path 为非空 identifier sequence；package、静态 owner 与实体的分界由名称解析决定。

driver 在完整解析后、HIR 前统一检查语言规范 12.1 的 manifest package/group 关系，生产、测试和文档路径共用此检查；只比较已经解析的实际名称段，不用字符串前缀或重新拆分转义段。core/std 豁免只按其 group/name 识别。单文件的 AST 与原文继续保留合法 package header，driver 产生 warning；HIR 创建当前 source namespace 时使用保留的 single-file 身份选择 RootPackage。AST 观察同样返回该 warning；HIR 失败时保留已经产生的源码 warning。后续 IR 不再重验 package/group，也不改源码或诊断偏移。

AST 不包含依赖 locator 解析、构建顺序、已推断类型或实体身份猜测。词法与语法恢复收集 Error 级诊断；任何解析失败阻止成功 HIR 输出。面向编辑态的可恢复语法结果与成功编译 AST 分开，公共基础的合同见 2.20；保留错误语法不表示该程序可以进入后续编译阶段。

语言9.1.8的条件接口实现使用独立的宿主体内AST节点，保存目标interface type、where、实现成员与准确span。它不声明新的nominal或binder，不伪装为嵌套interface；不支持的位置和块内非法存储／初始化形态产生正常源码诊断。

共享 lexer 按语言规范 2.5 消费仅位于 byte offset 0 的 `#!` 行，将其视为普通 trivia；不创建删除该行的临时源码，也不将其附着为文档片段。LF、CRLF 和行尾 EOF 沿用原有源码位置规则。

语言规范 9.7 的文档片段由同一 lexer 保留，包含原始文本、种类和准确 byte span，不另扫源码。parser 在实际声明语法入口建立附着关系；SourceFile 的文档表以对应 AST 节点的完整精确 span 记录关联，无目标的片段显式保存为未附着。此键只服务本文件 AST 语法关联，不能作为语言实体身份；HIR 将实际 AST 节点关联到其 typed 声明，不按名称、最近距离或 span 包含关系猜测目标。文档保留不改变 token 换行事实、语句边界或普通编译诊断。

文档请求在 parser 后由 driver 调用 `scoop-doc` 整理同一批片段；普通编译不执行此步骤。整理后的 Markdown、tag、待绑定符号路径、显式引用定义及规范化文本到原 UTF-8 byte span 的映射是 AST 侧纯数据。Markdown 库负责区分代码、引用定义和普通正文，不另写一套 Markdown 扫描器；tag 只在其允许的正文位置识别。语法 warning 也保留原位置，后续随本次 SourceIdentity 绑定。HIR 在实际声明作用域绑定这些输入，文档 stage 只消费完成后的引用，不调用 parser 或 hir-lower 实现。

lexer 使用 Unicode XID 属性库识别普通标识符，并保留反引号 token 的语法类别；AST 名称保存去掉引号后的实际文本。持久化 canonical identifier 接受任意合法的非空标识符文本，包括必须以反引号书写的名称，不重跑普通 token 的 XID 规则。package、注解和成员路径保存完整分段及实际 owner，不能按显示名称中的点号重新拆分。名称的 wire 编码继续为 UTF-8 text，已有合法记录的编码与实体身份不变。

循环标签只保留在 AST 和 HIR lowering 的当前 callable 词法环境中；HIR 的 break/continue 总是引用全局唯一的 LoopId。表达式跳转沿用现有语句 setup 与 Nothing 结果，不产生伪造的正常值。默认参数和泛型 body 的持久化形式以类型化的包围循环深度选择目标，0 表示最内层；标签文本不进入后续 stage。break/continue 的既有单字段 tag 10/11 编码保持为深度 0，非零深度使用同 tag 的第二字段 1（u32）；显式写出的零深度不是 canonical 编码。reader 检查深度确实指向该 body 当前 callable 的包围循环，实例化为新的 LoopId 后直接复用该结果。MIR cleanup 按目标 LoopId 选择已建立的循环入口和出口，不再要求跳转总是指向最内层循环。

do/while 与 while 共用有明确 LoopKind 的结构化循环记录；condition setup、条件、body 均完整保存。AST/HIR lowering 在同一 body 词法作用域内依次检查 do body 和 condition，并按有效 continue 出口检查 condition 的读取。持久化普通 while 继续使用 tag 8，do/while 使用 tag 9，字段与 while 相同；读取时按 LoopKind 的实际执行顺序检查局部定义。MIR 为 do body、条件和出口建立独立 CFG block，初次进入 body，continue 指向条件入口，条件为真回到 body；条件入口保留已有 loop-header GC poll 与挂起恢复契约。

局部 return 标签保留于 AST；parser 为直接具名调用的 lambda 实参补充隐式标签，显式标签优先。HIR lowering 在当前 callable 的词法边界解析标签，并复用已选返回类型与既有 Return / Nothing setup。lambda 无期望结果类型时合并自身显式返回与正常末尾结果，进入嵌套 callable 时保存并恢复独立推导状态；成功 HIR 与持久化 body 不保留标签文本，也不增加跨 callable 转移。

具名表达式体和非局部 property 的省略类型仅存在于 AST 与 hir-lower 内部声明查询。先建立 typed 声明身份和显式参数，再按表达式检查实际访问的声明完成结果类型；查询中的活动依赖用于诊断推导环，不反复猜测类型。重载先按参数选择目标，只有需要结果约束或提交获选调用时才查询声明结果，不能把推导环当作改选其他重载的理由。继承目标、分派槽和默认参数来源按已知签名部分确定，仅将依赖推导结果的类型相等约束延后；结果完成后检查这些约束，不重放继承与布局验证。表达式类型检查、重载选择及适配复用普通 lowering，推导的完整 body 可供后续编排复用。成功 HIR、共享签名及其持久化编码只保存完整结果类型，沿用原有签名/接口依赖失效及访问域检查；MIR/codegen 不重新推导声明。

控制结构的块体和单语句体使用同一 AST block 载体；单语句保留原始 span，并同样在 HIR 建立独立作用域。else-if 保存为 else block 中的嵌套 if，后续阶段沿用已有值合流、终止路径和循环 lowering，不增加平行表示。

parser 中仅有存在性与位置的修饰符使用可选 span；带取值的修饰符同时保存取值和 span，不独立维护 bool 与可缺失位置。AST literal pattern 使用封闭的字面量数据，保留原数字拼写、允许的数值符号和准确 span，不接受任意表达式作为字面量模式。

语言规范 8.4 的五个兼容修饰符在合法语法位置消费后丢弃。parser 可以保留诊断所需位置；HIR、MIR、LIR、共享签名与 ABI 不保存对应语义标志，codegen 不生成内联或尾调用属性。源码位置与哈希仍反映真实文本，不要求去掉标记前后的产物逐字节相同。

when 的 AST 明确区分无 subject、表达式 subject 与 val subject 声明；声明保存完整 binding pattern、可选完整类型及初始化表达式。arm 区分普通条件序列、case 模式与后备条件，保留 guard 和各条件位置。return/throw 可处于表达式位置；调用保留尾随实参来源及分组边界；annotation 数组保留每个元素的位置。

### 2.2 HIR

M47 与语言服务共享既有语义查询的数据和实现。编辑态查询可以返回明确的不可用原因，但不能向成功 ExportHir／LocalConcreteHir 写入缺失类型或正文；查询与完整编译输出的边界见 2.20。

文档请求仍完成正常声明、函数体、默认值、泛型、效果与可见性检查，再返回附加的源码 API 视图。视图通过现有 typed 声明及实际参数／binder 关联原 AST 节点，保留原声明作用域、源码签名片段与已完成的推断类型；不以 LocalConcrete 实例或机器符号恢复 API。主构造 val／var 的直接文档属于实际字段／property，构造参数是独立目标。getter／setter、variant payload、context 参数与类型参数沿真实 owner 和语义位置关联；内部 helper 不成为源码 API。

待绑定的文档引用在原定义处使用正常名称查询的作用域、层级、访问域与 direct dependency 集合解析。普通 Markdown URL 不参与语言查询；源码注解的文档效果按实际 core identity 识别。文档输出区分已解析引用和已诊断的无效引用，类型信息始终完整；文档 warning 单独保存并绑定 SourceIdentity，不改变普通编译 warning，也不导致跳过正文错误。该视图只用于当前请求和工具本地文档输入，不写入 `.slib`，不改变普通机器生产的输出格式。

文档 stage 将该视图及 Export 中本次已读取的完整源码投影为可序列化的渲染输入。显式签名保留作者写法，推断类型用原声明的 binder 名称呈现；package 保存实际分段，不从显示名称反推。页面锚点由既有持久声明身份的 typed 编码产生；类型／context 参数等子项使用真实 owner 和语义位置，派生 equality 使用实际 nominal owner 与派生角色，不使用请求内 arena 序号或具体化实例作路由身份。渲染输入保留已解析链接、声明与源码位置、结构化 warning、实际 property 能力及文档注解效果，使站点目录或展示选项改变时可直接重用。路由键仅用于站点寻址，不成为新的语言身份或产物验证合同。

文档中的源码原文直接嵌入生成的 HTML；源码导航仅链接站点内生成的页面与行锚点，不引用原始文件的 `file://` 地址。渲染输入没有对应源码文本时，省略其源码页、展示区和链接，不凭位置记录重建源码。完整源码仅用于当前请求和工具本地文档输入，不写入 `.slib`；持久 `SourceRecord` 继续只保存身份、摘要、长度及位置元数据。正常编译所需的源文件缺失仍按实际 I/O 错误处理。

扩展 receiver 的导航由实际 receiver 目标及完成后的类型投影产生，扩展仍归属声明 Cone／package。派生 equality 保留声明 owner 和字段／payload 的原 binder 类型要求，页面明确其适用性取决于各组成类型的同类型 equality；不把条件声明展示为任意实例均可调用。跨 Cone 文档查询沿已加载的 nominal 表示与既有同签名成员查询确定派生声明，保存原 nominal owner／派生角色，不为文档解析选择、生成或具体化机器调用。core 自身的推断 primitive 类型链接沿 DefinedCoreProtocols 指向真实当前声明，普通 Cone 沿已导入的协议指向实际 core 声明。

公开展示使用 HIR 的 effective lookup 与 inheritance domain，并计入词法 owner；公开且允许外部继承的类型中的 protected 成员可展示。`DocHidden` 的实际目标和后代从 API 目录移除，但不删源码或构造参数文本；`DocAlias` 仅按 core 注解身份提取静态字符串，空白关键词产生带应用位置的文档 warning。文档缓存保留筛选前数据及诊断，private 展示选项和 warning 政策不要求重做 HIR。

调用候选已经确定的完整形参类型直接传入实参检查；不先以缺少期望类型的完整调用试算来判断同一实参是否依赖上下文。尚未确定的形参按语言规范 8.6 的固定点顺序收集独立约束、处理延期实参与默认种子。语法无法决定上下文需求时，在当前候选中尝试独立检查一次；成功结果及其状态直接用于该实参，失败仅在取得期望类型后重试，不因默认种子阶段到来而重复同一次无上下文尝试。上下文需求查询不得使嵌套调用逐层重复检查其正文和实参；保留候选隔离、诊断位置及运行期实参求值顺序。

数组元素、相等操作数和 if／when／try 值分支沿用同一独立检查与结果复用规则。分支在实际词法绑定与 smart cast 环境中检查，独立失败只保留诊断，不提交失败状态。默认种子的优先次序只查询语法中的数字默认表示；实际尝试失败时继续其他种子，不能把试算当作布尔查询再重做成功结果。源码顺序的 setup 与值求值保持，已完成的 when 后备分支不因参与种子选择而再次展开。

HIR 完成名称解析、类型检查、重载与泛型实参选择、可见性、继承与覆写、模式穷尽性、构造／初始化规则、常量、效果及 FFI 声明检查。源码与依赖声明遵守同一语言规则；文件发现顺序、声明存储位置和 provider 读取顺序不能改变结果。

HIR metadata 保存并直接提供已生成的来源形状与 representation 数据；按实际格式和 typed 引用解析，不以另一份 source inventory 重新比较字段、variant、CLayout 或 intrinsic 形状，也不保留只由测试调用的来源资格包装与语义复核 API。语言规则仍由 HIR lowering 在产生这些结果时完成。

继承边、slot schema、完整 slot contract 与已选实现同样作为普通数据消费；不另建 checked inheritance graph 重放源码身份、继承环、覆写、可见性、签名或 interface 展开。实际类型关系查询与 slot selection 编码继续使用原 typed 数据；格式构造和引用解析只处理当前记录所需的局部关系。

GC/ZST facts 在生产时由具体 HIR 投影，辅助形状只属于这次投影，不保留独立的 fact-shape transcript、依赖资格包装或全图重算 API。声明访问数据直接保存实际 visibility、词法 owner 与诊断 origin；不另行重建 owner 身份或比较来源副本来证明这些数据有效。

smart cast 分析使用已有 BindingId、类型约束及带实际 LoopId 的控制结果，在当前 callable 内传播正常出口事实。多个正常前驱取共同事实，局部写入撤销旧值事实，finally 按实际进入和离开结果合并；循环按条件、body 与回边求稳定结果，并确认被使用的类型视图在对应路径成立。when 和短路 lowering 保存的临时值按实际求值顺序参与分析；不可变捕获值和构造器参数按原 BindingId 关联快照，快照不会因源变量后续写入而改变。完成的 HIR 仅保留普通 typed 读取、必要拆箱与既有控制流，不向 MIR、共享元数据或 runtime 添加流分析状态，也不在后续 stage 重放该分析。

函数 smart cast 的读取视图与 binding 的声明存储类型分别处理。显式类型检查、转换及引用相等读取原存储值；不需要函数适配的期望类型继续使用声明视图。产生适配闭包的读取不建立来源与新值的快照等价关系。MIR 为动态函数适配器保存来源时，先使用既有引用转换将来源适配到捕获字段的 Any 类型；typed function 来源也必须满足同一物理字段契约。

重载探测共享已经检查的声明、签名和正文，包括依赖模板、constructor、lambda 与局部函数；候选新增或修改的函数、局部值与词法绑定各自隔离，只有选中候选的结果进入当前状态。一次探测不复制此前所有函数正文或当前函数的全部局部值。嵌套调用的候选状态在堆上保存，失败诊断只借用该状态，不为传递诊断把整份 lowering 状态移回线程栈。参数形状不匹配的候选在建立可变试算状态前退出，普通函数名不经过 nominal constructor 的准备。内部共享不改变实体分配顺序、诊断与重载规则，输出仍为原有完整 HIR 数据。

依赖声明 catalog 在构建时按 typed nominal owner 与成员名、operator role、constructor、dispatch slot 和 context parameter 需求建立查询索引；property accessor 和 intrinsic 使用各自 typed key 索引。重复成员查询只访问匹配候选，不重扫全依赖声明集合。索引保留原实体顺序、可见性与 intrinsic 唯一性语义，候选共享不可变的接口、源码与默认参数模板，不改变持久身份或产物格式。

Export HIR lowering 的结构类型与完整函数签名通过结构键查询已有 typed ID，不逐次扫描类型 arena。函数键包含 suspend effect、可选 receiver、按序参数和返回类型；nominal application 继续使用原专用索引。类型索引与 arena 属于同一候选状态，失败试算一起丢弃；命中不分配新 ID，新类型仍按原分配顺序进入完整输出。索引仅为当前 lowering 的内部数据，不写入 metadata 或兼容性规则。

候选局部约束求解可以先把已确定的参数、可选 receiver 与 suspend effect 传给 lambda，再用正文推断尚未确定的返回类型。这个部分期望仅存在于 lowering 的推断上下文，不是缺少返回类型的 HIR 函数类型；提交 lambda 时其参数、返回值、捕获和正文仍全部确定。当前 Cone 与依赖模板调用使用同一推断路径，不按扩展名或库名特判。

`SemanticHir` 保存已检查的声明与正文，对外有两个不同的输出合同：

- **ExportHir**：跨 Cone 的声明与必要正文。区分 public/re-export lookup、protected inheritance/slot、generic hidden support 和 interface dependency；支持声明的存在不扩大可见性。保存签名、默认值、const、alias、注解、类型关系与模板；符合 2.19 选择条件的普通非泛型小函数可附带优化正文，其他普通实现仍由原提供方执行。
- **LocalConcreteHir**：本 Cone 实际需要发射的完整实体与正文。全部类型参数已替换，所有调用、构造、字段、variant、slot 与 external target 已确定；不含未完成推断、待实例化请求或上游普通函数正文。

两者使用不同的实体 ID 家族和显式映射。导出声明、完整泛型 application 与本地 concrete 实体不能用同一 arena index 或别名混用。

executable 的入口发现只检查 root Cone，并按语言规范 12.4.4 一次确定 `NoArgsUnit`、`ArgvUnit`、`NoArgsInt`、`ArgvInt` 四种形态之一。LocalConcreteHir 保存实际 main 的 typed identity、完整源码签名和入口形态；有参数时同时解析 argv 构造所需的实际 core `Array<String>`、String 及 helper targets。argv 构造使用普通 internal core 函数 `__scoopProgramArguments(): Array<String>`，通过现有 compiler protocol 记录实际函数 identity；其正文读取 runtime 已保存的 raw argv，严格解码、复制并报告非法参数下标。raw argv 访问器的 Scoop 返回类型为 `Ptr<Int8>?`，保留越界返回 null 的契约；helper 在 argc 范围内访问并按普通 Option 规则取出指针。后续 stage 使用该 typed 引用，不按名称重新查找。缺少入口、多个入口或非法签名在 HIR 报错；不能留给 linker 通过函数名挑选或修补。

每个 expression 具有非可选类型和 definition/evaluation origin。已解析调用保存完整逻辑签名、适配前 receiver 类型、实参映射、唯一 typed target 与效果；receiver 和实参的执行顺序遵守语言规范 8.5，不由类型求解顺序决定。

数组／tuple 为保持左到右求值而物化的临时引用使用元素在当前表达式中的源码位置；导入常量的初始化表达式继续保留其定义／求值双来源，不能用定义方 span 与消费方文件拼接来源。

固定元数 intrinsic 的操作种类与必填操作数属于同一分支：一元操作携带 operand，二元操作携带 lhs/rhs；数据借用携带 object、block 及已确定的对象与函数签名信息。函数类型的构造／替换返回 FunctionTypeId，enum variant 保存 enum application 身份；需要通用类型时显式取得对应 canonical type。declared nominal 的字段等数据归属独立 payload，表示专用操作只接收其适用的 payload 或窄引用。

整数表达式的一元／二元分支使用对应的窄操作类别；div/rem 仅属于二元 managed 操作。该完整结构贯穿 Export HIR、LocalConcrete HIR 与绑定默认模板；旧 wire 的操作标签与参数字段在解码绑定处一次组合并检查元数，生产端由完整表达式投影原编码。方法引用生成固定 intrinsic 正文时，按已决议操作直接建立所需形参和值，不将它们重新装成任意参数列表。

数据借用的源码和导出模板仍通过普通泛型调用保存声明与实参映射；HIR 具体化在解析实际 intrinsic 和完整实例签名的边界，将其形成具名 object／block、对象类别和 FunctionTypeId。对象类别及回调签名取自已决议形参，不能从可能为 Nothing 的实参结果猜测。无正文的源码 intrinsic 复用 source parameter interface 中的逻辑参数；导入模板复用其完整实例签名，不为不存在的正文生成参数 local。MIR 接收完整借用操作，先后求值 object 与 block，保留原 pin／finally cleanup 与异常传播，不重新按参数个数或值类型恢复关系。

LIR struct store 在创建 Scoop／C 表示时返回对应的窄引用，后续字段填充沿用该引用；C payload 的消费接口接收已选中的字段与布局。不可变类别的存储投影仍属于普通 arena 查询，不增加另一套身份或重复保存 nominal 数据。宽表示入口只完成一次实际分支选择，intrinsic 不以空 declared 字段参与源字段或 C payload 操作。

MIR 中 intrinsic struct 与 intrinsic class 分别保存其允许的表示类别；例如 Array／Atomic 不可填入 struct 表示，Ptr／MaybeUninit 不可填入 class 表示。该区分沿既有 nominal 生产路径完成，后续物理类型与布局消费不再排除另一类 nominal 的变体。

构造器和未绑定成员引用在 HIR lowering 完成候选选择与全部类型实参推导，再生成具有完整函数类型的普通闭包适配 body。适配 body 只调用已选定的构造器或方法，不复制构造初始化；virtual / interface 调用保留原动态分派目标。适配 body、泛型和默认参数产物复用既有 lambda、调用及构造形态，不另建引用 ABI。

receiver 函数类型在源码 HIR、共享签名及 exact type identity 中保留独立的 receiver 类型槽。类型替换、lambda 参数推导和产物消费使用该形状，不从名字或普通首参数猜测 receiver。lowering 将 receiver 展平为第一个实际源码参数，复用 closure / invoke、函数值适配和既有 ABI 分类；closure environment 等隐藏参数独立处理。`this@label` 在 HIR lowering 绑定到实际不可变 receiver binding，外层 receiver 沿普通捕获路径保存，不把标签带入运行期名称查询。

默认值、泛型正文及词法 callable 保存定义处绑定、完整 binder、局部值和 capture 身份。再次消费只替换实际类型与求值上下文，不重新选择名称、重载、字段或接口默认实现。局部函数和 closure 的捕获引用原绑定，不能改为消费方的同名局部变量。

补齐元数据等新增依赖后再次推进导入正文队列时，已经完成的函数和构造器正文直接复用；不能重新展开构造初始化并替换其中的局部绑定，使既有闭包捕获失去对应定义。

默认模板分别保存提供方参数的预期类型和正文表达式的实际类型。前端按子类型规则检查默认值；`Nothing` 正文可以满足任意参数类型，产物投影、读取及实例化必须保留这两个类型，不能要求它们逐字相同或把底类型调用改写为参数类型的成功返回。模板契约仍校验预期类型与原提供方参数一致；通用格式构造器不重复进行依赖源码类型表示的子类型判断。

源码调用记录的实参保留实际表达式类型。通常实参已由前端适配为参数类型；Nothing 实参保留底类型，在 HIR 源码签名和 MIR 实现签名连接时通过实际 nominal 的 intrinsic 表示识别。两处均保留参数个数和位置检查；调用结果仍须与被调用声明的完整结果类型相同，不能因上下文需要而改写。

nominal application 保存原声明与完整实参；struct、enum、class、interface 各有独立 typed identity。alias 透明展开；字段、variant、构造与 property accessor 保存各自原身份。按值循环、继承环、泛型约束、GC-free 和 ZST 属性在所属语义边界确定。

依赖 struct 的存储属性读取使用已发布字段身份；显式 receiver 与成员、扩展正文中的隐式 receiver 遵守同一规则。Storage accessor 不要求另行导出不存在的函数体。

解析名义声明头时，先建立该批声明的完整继承与接口关系并检查继承环，再验证依赖这些关系的类型实参上界；合法的 `S : I<S>` / `I<T : I<T>>` 不因声明顺序或尚未填入 S 的接口表而失败。延后检查只保留本次实际出现的 typed 上界要求及诊断上下文；局部声明与依赖接口使用同一时机，不重放完整声明解析。

sealed 的继承限制在 HIR 按真实 origin、package 与声明作用域检查；成功声明保存完整直接子类型集合，class 的有效 modality 仍为 abstract。when 按这些关系与已有类型替换检查闭合家族，不在 MIR 或链接阶段重新求穷尽性。共享 nominal details 对 sealed 增加第 12 字段的规范化直接子类型身份列表，未 sealed 的原 11 字段编码保持不变；相关子类型的普通声明与继承签名进入现有共享声明闭包，私有数据不增加 public lookup binding。reader 在普通格式与引用边界检查集合及关系，消费方复用已读信息，不增加继承授权或运行期注册协议。

条件实现的直接关系复用同一子类型集合与已有条件声明记录；格式边界检查直接引用时同时读取普通父类型和条件接口目标，不重新求解条件。HIR 穷尽性检查保留每个条件关系的适用域及未确定参数，在覆盖该子类型家族时使用这些事实；只有确定不可能的应用才被排除，投影无法反解不等于不可能。

interface implementation delegation 在 HIR lowering 中完成 slot 选择、隐藏字段分配和普通转发成员的生成。字段身份由真实宿主与继承项位置确定，和源码 property backing/delegate 字段区分；它不产生可查找的源码属性。构造记录在 base/primary store 后、body 初始化前保存 delegate，转发正文只包含普通字段读取与已绑定的接口调用。泛型构造及转发正文保留原 binder、字段和 slot 身份，跨 Cone 通过现有声明、字段和正文闭包消费，不在下游重新决定委托成员；MIR/runtime 沿用普通对象布局、接口分派和 GC 扫描。

透明 alias 的声明查询先在定义处绑定独立的 typed 类型参数、bound 与完整目标，再在使用处替换实参；活动 alias 声明栈检查全部展开依赖中的循环。嵌套 alias 使用实际 owner chain 和正常类型名称空间，局部 alias 只存在于词法作用域并在 HIR 类型中展开。alias 不产生 nominal、构造器、layout、TD 或另一种单态化实体。公开泛型 alias 的接口保存规范化 binder/bound 与 typed signature target，跨 Cone 消费和再次发布复用该目标；不在使用处重绑定源码名称。无参数记录保留已有四字段编码，有参数记录增加第 5 字段的规范化 binder 列表；需要宿主实例化参数时，第 6 字段保存该列表的宿主前缀长度；带 binder 的目标使用完整展开 signature，不把参数应用伪装成无参 alias 边。

property 具有完整类型、读写能力、getter/setter、访问域与互斥的 stored/accessor/delegated/const/extern 表示。required accessor 有 Body 或 AbstractSlot 的明确类别；普通缺失初始化不能靠空正文、late-init flag 或可空类型补齐。

泛型 object/companion 在消费方物化时，字段及其 backing property 保留声明方身份。字段类型位置的校验沿字段的声明来源取得对应属性记录，不要求消费方重新声明该属性。

HIR 将属性赋值右值展开为前置语句时，先固定已经求值的 receiver，再接入这些语句；最终的 storage write 或 setter call 使用保存的目标。不可重新绑定的类引用局部值已经固定了目标，可直接复用。本地与导入的属性遵守相同的求值顺序。

class 构造输出明确区分 allocation、同 receiver 的 initializer、this/base delegation 与 common initialization；完整成功路径、异常出口与初始化顺序已确定。abstract class 仅有供派生构造使用的 initializer。release policy 为 None 或携带完整 hook target 的 SynchronousGcFree。

inner class（语言规范 9.1.3.1）的 binder 先保存外层 class 的完整 binder 前缀，再保存自身参数。普通 class source shape 中的 EnclosingInstance 字段角色及其 signature type 明确保存所需 outer application；该字段使用独立的 typed source field key，不产生源码 property。构造声明接口把 outer 作为独立 receiver 保存，参数名、默认值及 overload signature 仍只对应源码参数。实际 allocation callable 的值输入为 outer、源码参数；initializer 的输入为正在初始化的 inner、outer、源码参数。构造模板使用独立的 EnclosingInstance local selector，不能把 outer 与初始化中的 This 或源码 Parameter 索引混用。隐藏 outer 字段身份使用 source field tag `5`（仅 owner），隐藏参数 selector 使用 tag `7`（无 payload）；普通字段、This 与源码参数的既有编码不变。构造委派通过隐藏参数读取 outer，最终构造器在 base 后先写 outer 字段；普通成员与闭包通过该字段获得外层 receiver。所有引用均按普通字段、参数、捕获及 GC root 处理，不增加 runtime 入口或另一套对象模型。

局部类型与匿名 object（语言规范 9.1.3.2）在定义处完成普通 nominal 声明、类型、继承、成员及构造检查；它们使用原 Cone、词法 owner 和定义路径形成 typed nominal identity，外层 binder 作为完整参数前缀保存。匿名 object 没有 singleton/initialization unit，每次求值使用普通 class allocation。不可变捕获通过已绑定值、隐藏构造输入及普通字段保存，构造引用在创建时固定捕获值；value type 和 interface 不增加捕获环境。成功 HIR 保存完整声明与捕获类型，generic/default template 直接携带所需声明和 typed 引用，消费方不重新解析局部名称。局部/匿名 class 的捕获值按稳定槽位组成普通 tuple（空环境用 Unit），保存为一个隐藏实例字段；构造输入将该环境作为隐式 context receiver，与源码值参数分开。inner 的 outer receiver 与局部类型的捕获 receiver 使用不同的 typed context kind；二者复用普通构造参数传递、字段存储与 GC 描述。捕获集合在该声明全部 body 分析完成后确定，随后一次完成相关构造输入与环境读取的类型，再交付 HIR。捕获字段的 source field tag 为 `6`（仅 owner），隐藏参数 selector tag 为 `8`（无 payload）；已有角色编码不变。

singleton、runtime property 与 generic delegated application 保存完整初始化单元及 cycle throw target。constant 已完成依赖环检查与求值，没有 runtime unit。interface/default/virtual 选择在 HIR 唯一确定，qualified super 保存 direct target。

core 的 Add 等普通运算符接口不进入编译器内建 protocol 表。HIR 先将 operator 表达式形成相应角色的普通调用请求，在定义处完成成员、bound 或扩展决议；不以某个 core 接口的实现关系作为前置过滤。generic/default body 保存所选 typed callable 或 bound slot，具体化按实际 conformance 映射实现，消费方不得重新按 import 或方法名选择。已有数值 intrinsic 可以显式覆写普通 interface slot：其声明保持 Final，产物保存实际 slot 关系与原 intrinsic implementation，不能要求它只能是 DirectOnly。top-level、extension、constructor 与 source extern 仍遵守原直接调用合同。NoGC 实现与 Managed slot 的适配复用现有 callable/ABI；具体值直接调用仍保留 intrinsic。LocalConcrete intrinsic 必须从已解析的 source parameter interface（或已导入模板签名）保留完整物理参数类型，包括 receiver；它没有源码 local arena，不伪造参数 local id。当实际 itable 引用 intrinsic 实现时，MIR 以该实现原有 callable materialization 生成一个具备完整参数、返回值及 effect 的可调用正文，复用直接运算与 callable reference 的 typed 运算降级；不为未参与分派的 intrinsic 额外生成入口。装箱与 receiver 调整仍使用既有 interface adapter。由声明 slot 或 nominal dispatch selection 实际引用的 intrinsic 实现入口随普通 callable binding 导出，供 dependency 的 itable、装箱适配与 bound 具体化引用；只有直接展开用途的 intrinsic 不导出机器入口。

`==` / `!=` 按语言规范 9.3、11.11 从普通成员中选择 operator equals，保存实际 callable 或普通接口 bound target；不要求 core Equals conformance。`Equals<T>.equals` 按普通 interface 声明、override 和 bound call 处理，不进入 core operator／protocol 身份表，也不为同形方法或结构比较补齐接口。

不可重载的 `===` / `!==` 同样从左到右各求值一次；右侧展开出的 HIR 前置语句必须位于左侧取值之后，不能改变左侧已取得的引用。

判定派生签名是否被手写成员占用时，检查名为 `equals`、参数为完整宿主类型的原成员签名，不以 operator 标记或当前调用处的可见性过滤。普通同签名成员仍可按普通函数调用，但不会成为 operator；导入的 nominal 使用已保存的成员声明执行相同判断，不因成员正文或源码不可见而重新派生。

结构比较在当前静态类型环境中绑定字段的 operator equals。宿主包含类型参数时，先按顺序保存两个操作数，再将已绑定的比较正文展开为普通字段调用和短路表达式；enum 的 payload 读取受相应 variant 检查保护。泛型正文、默认参数和跨 Cone 模板保留这些普通 HIR 操作，具体化时不得按具体宿主重新选择字段重载，也不得与直接比较该具体宿主的派生正文合并。封闭宿主仍使用按 exact owner 标识的既有派生 callable。`Equals.equals` 继续由普通接口、泛型正文、装箱和跨库调用机制承担。

开放结构比较的展开模板可以复用已绑定的字段比较，但每次内联生成的表达式必须采用本次比较操作的完整 definition/evaluation origin；嵌套展开沿用该来源。不能把第一次生成模板的方法上下文或位置带入其他方法。比较位于默认参数中时，保留该默认表达式已有的定义／求值来源，由普通缺省实例化继续传播实际调用位置。

按 exact owner 生成的派生 callable 没有独立的词法源码正文，其表达式保留触发派生的来源；该来源可以是依赖中的默认值或模板。检查其中的依赖调用时仍验证 definition/evaluation 的文件、位置和上下文引用，但不能要求求值来源属于当前 Cone，或要求这个按类型生成的 helper 出现在导入函数模板表中。生成身份继续只由 exact owner 决定，不因首次触发派生的文件或调用点改变。

构造器的公共初始化片段采用当前终结构造器的求值上下文；在片段具体化期间请求的独立派生比较正文不继承该上下文覆盖。派生正文使用自身已经绑定的完整来源，返回后继续按原构造器上下文处理初始化片段，不能把另一个文件的比较来源与当前构造器的上下文拼接。

控制表达式的分支结果物化为临时赋值或丢弃结果语句时，语句 span 使用该表达式在当前正文的 evaluation span。lambda 尾表达式生成的隐式返回或丢弃结果语句也使用 evaluation span。导入常量的字面量继续保留 provider definition origin，不能把 provider 的字节偏移与 consumer 的文件或正文上下文拼接；普通正文、缺省值和导出内联模板遵守同一规则。

依赖携带的 source record 与 source context 用于诊断及实际语义引用。读取其格式、范围和引用即可使用，不再遍历 provider 的副本逐字段比较来证明来源；源码显示内容不构成产物认证信息。

派生比较保留值传递、可见性和 InteriorMutable 的实际类型使用检查。NoGc 实现满足 Managed interface slot 时分别保留实现体 effect 和接口调用 effect，适用于显式 `equals` 及其他普通接口方法；具体值的 intrinsic operator 比较保留原 effect。其他签名、访问、ordinary/suspend 与安全性检查遵守普通规则。

extern 声明产生完整 `SourceNativeExternalContract`：symbol bytes、逻辑 library/default namespace、function/data/TLS、mutability、C/Scoop ABI、calling convention、完整的 native 签名、GC effect 与定义位置。Scoop callable 另保留完整源码签名；启用 errno 捕获时，两者按语言规范 13.4.2 建立明确的结果适配关系。普通／suspend 效果与 GC effect 独立，后续不得由 symbol 或参数类型反推。

C ABI extern 函数的声明按语言规范 13.4.1 另存非可选的调用模式 `NativeSafe` 或 `GcLeaf`：未标注时前端填入 NativeSafe，合法 `@GCLeaf` 填入 GcLeaf。此模式随声明、模板实例化和实际调用经 HIR/MIR 传递，不由参数 GC-free、`@NoGC` 或 native symbol 名称推导。它属于当前 Scoop 声明的 caller 协议，不进入目标 C symbol 的物理 ABI 合并键；同一 C symbol 的不同声明可以选择不同模式，但不得因此放宽签名等既有 native 合同检查。

C extern 的结果适配明确区分普通返回与 errno 捕获。前端检查 `captureErrno` 的常量、适用位置，以及透明 alias 展开后的精确二元 tuple `(R, Int)`，将第一项 `R` 投影为 native 返回类型；`Unit` 投影为 `void`，其他类型按既有 C-FFI-safe 规则检查。合法捕获分支完整保存 native `R`、Scoop tuple 结果与捕获行为，不以可缺失字段或后续重新猜测 tuple 形状代替。调用、函数引用及泛型消费保持固定的 Scoop 返回类型，不因结果被忽略而关闭捕获。release 调用对完整 Scoop 参数和结果应用既有 `ReleaseValue` 检查。

C-FFI-safe、Scoop ABI、GC-free 与 release-safe 是不同合同。C 参数不得含 managed reference，Scoop ABI direct ref 不改为 handle 或 pin。generic 条件按原 binder 保存并在实际 application 检查。

实际依赖分为 lookup observations 与 committed uses。前者包括空查找、不适用候选、star surface 和 re-export；后者才形成 MIR/LIR 的 callable、类型、布局与 native 需求。未展开默认值和未物化模板不产生机器调用。

具体化沿用当前 Cone 声明的 nominal 物化闭包作为初始根，包含本地非泛型声明；正文调用、存储及其他实际类型用途继续扩展具体化闭包。本地物化集合与发布声明闭包承担不同职责，不能用较小的发布闭包替代本地根，也不因本地物化而发布私有类型。比较实现按实际调用与发布需求生成；类型的接口集合始终来自普通声明和继承关系。

定义 core 时，输出协议所引用的基本类型主体也通过同一物化请求入口补齐；它们不依赖某个普通接口或无关源码调用间接触发物化。已物化的主体复用原结果。

公开非泛型值类型的可用派生 operator 正文由定义 Cone 发布，消费者引用该生成 callable；字段不可比较时不发布该正文，也不因此拒绝类型声明或构造。普通 `.equals` 调用与 `==` 使用同一派生成员签名；需要实现显式用户接口中的 operator slot 时，定义处即检查并生成对应正文。

when 的前端决议遵守语言规范第 5 章：普通条件保存已解析 Boolean 求值及其局部 setup，case 保存 typed pattern；多条件组合为现有有序短路 Boolean 求值，其局部 setup 保留在同一条件内。无 subject 不伪造 Unit subject。subject 声明复用不可失败 binding planner，初始化、快照及解构各按规则执行一次，并继续匹配完整快照。fallback 明确区分源码 else、语句正常落空及已检查的不可达。路径类型事实在当前作用域合并，普通表达式不进入模式名字解析。

有 subject 与无 subject 的 when 使用不同的完整分支；只有前者携带 case 条件与依赖 subject 的覆盖信息，subject 及其实际类型不由独立可空字段拼接。 arm 的 guard、正文及位置可共享表示；纯条件分支的主条件只包含 Boolean 谓词或后备条件，其末端仅允许 else 或正常落空。覆盖结果记录实际选用的穷尽性类别，类型从当前 subject 取得，不再另存需要后续比对的 subject 类型或 enum 身份。导出模板的既有 wire 标签和可选 subject 编码保持不变；解码绑定在这一格式边界一次选定完整分支，后续具体化和 MIR 转换直接消费已选形态。语义 Return 必填结果表达式，正常 Unit 返回携带真实 Unit 值；源码 bare return 在 HIR 转换为这一完整形式。结果表达式的副作用先于清理执行，Nothing 路径没有正常返回值。

已解析的导出正文／默认模板 Return 同样必含结果表达式。现有 wire 的可空返回字段仅用于编码兼容：旧的空字段在解码绑定时形成该语句定义位置的 Unit 常量，新产物写入完整表达式；不将这个 wire 选择传播回语义 IR，也不改变字段编号或 section 版本。

return/throw 与 Elvis 复用正常值和终止路径的封闭表示；只有正常到达合流点的分支提供结果，不为跳转生成 Unit、零值、null 或未初始化 local。表达式跳转的实际 transfer 保存在当前位置的有序 setup 中；其结果及全终止控制表达式使用 Nothing 类型的 Unreachable 节点，明确表示没有正常值，不读取存储也不执行 runtime 调用。MIR 将该节点结束为不可达正常边。尾随 lambda 在每个候选内先映射最后形参，再复用普通调用检查；成功 HIR 只保存完整已选调用。捕获、具体化、完成性质、默认模板、导出正文与 dump 均覆盖这些表示。

整型枚举在源码 HIR 中具有明确的表示类别、底层整数 kind 和已检查的有序 variant 常量；父子关系、继承数值成员、普通上转及接口关系在 HIR lowering 完成。共享源码定义与表示数据保留 enum/variant identity、底层整数和数值，消费方不重新求值 initializer，也不按纯 unit variant 猜测枚举类别。依赖成员查找沿实际整数父类型继续遍历其接口，仅对接口节点读取接口方法与分派选择，不能把整数父类型当成 interface application。数值上转与引用上转均为已检查的单向类型适配；上转结果按目标类型继续参与装箱。

#### 2.2.1 表达式名称解析范围

语言规范 12.4.5 保持普通 import 与既有限定类型查询的边界。普通表达式不新增 package 起始的名称查找分支，parser／AST 保留 `::name` 与 `receiver::member`，不增加前置点分声明选择器。跨包函数、值与声明引用由 import／alias 定位，再消费普通 typed 候选；既有类型、注解与模式路径不受影响。

语言规范 11.19 的编译期入口由普通导入定位，沿本规范 2.10.1 求值；它们及 current_source_location 的声明引用沿 2.10 报告明确的 intrinsic 语义错误，短名／alias／re-export 保留具体拒绝原因。此能力不要求扩展普通包限定表达式或新增 runtime ABI。

语言规范 12.4.6 的块级 import 由 AST 保存原 selector、alias 与 span；parser／formatter 保持语句顺序，不把它移动到文件头。HIR lowering 在 import 语句处复用普通 namespace selector，向当前词法 scope 加入已解析 binding，退出 scope 时恢复；import 不产生运行期 HIR statement。闭包、局部函数、局部类型及默认值沿既有定义处 scope 快照继承导入。查询、operator／extension 分层与语言服务消费相同的作用域和 typed origin，不另建 intrinsic 专用导入入口。此能力不改变 MIR／LIR、runtime ABI 或已有 `.slib` 编码。

### 2.3 MIR

MIR 只接收 LocalConcreteHir 和实际选用的依赖 MIR metadata，不接受 ExportHir 模板或执行泛型推断。每个 expression 具有本层 exact type；GC-free、ZST、字段和 variant 身份完整保留，synthetic 类型在产生时也完整定义。foundation 从已生成的 Module 提取实际身份记录，不先完整重放类型、CFG、ABI、coroutine、callback、release、constant image 或 GC 语义检查；IR/meta crate 不保留独立的语言验证器及其测试专用入口。具体转换的查询与格式／引用边界继续在实际使用处处理。MIR 输入组装直接复用同次生产的 foundation、entry bridge 与 Module；初始化物化从实际 unit 及 callable 索引取得 initializer／ensure，不再重建 generated ID、ODR key、物理／逻辑签名或比较两份 callable surface。

调用与短路控制流从表达式提取为 CFG 时，必须先完成并保存此前操作数的取值，再执行后续操作数的调用或分支。没有显式调用的表达式仍可能读取可变字段、全局、指针或原子存储，或执行分配；不能因此延后其求值。聚合元素、调用实参和其他多操作数节点共享这一顺序规则，提前终止的操作数保留此前副作用并阻止后续求值。

私有正文构造为短路右操作数展开的 `!!`、cast、数组边界检查及其临时值必须留在右侧分支；不能并入包围整条语句的 prelude。左操作数按原顺序执行一次，右侧异常和副作用仅在该分支实际进入时发生。此规则复用普通结构化条件与 CFG，不新增产物或 runtime 机制。

`Nothing` 操作数没有正常结果。MIR 的私有构造表示须在建立整数算术、比较、移位或转换节点前传播这一控制流事实；此前操作数仍按顺序求值，终止后的操作数及整数运算本身不执行。不能将 `Nothing` 当作整数值送入要求精确位宽的构造器，也不能用虚构的零值代替；CFG 保留原调用的异常边及 catch/finally 清理。

errno 捕获调用保留 HIR 已确定的结果适配和 native 合同，表达式的结果仍为完整的 Scoop tuple。后续解构、存储或跨 suspend 保存使用普通值规则；不引入 last-error 读取节点、线程槽位或额外的 managed error 对象。

when 的普通谓词、多条件、模式、guard 与 fallback 降为既有 CFG；模式投影仅在匹配成功后执行，guard 失败继续下一 arm，合法落空保留正常边，终止分支不提供结果。return/throw 表达式使用相同 transfer 与 cleanup 通路。

MIR 控制流显式表示普通边、异常边、循环目标和 cleanup。Return、Break、Continue 只执行真正退出的 scope 清理；Throw/Rethrow 保持异常路径；finally 的覆盖规则和挂起不退出作用域的规则遵守语言规范。variant payload 只在同一值、同一 variant 的有效分支内读取。开放派生比较中的嵌套 enum 字段在其 tag 检查到达时保存为一个 immutable local，同一短路分支内对应的 test／payload projection 复用该 local；每层 payload 的取值仍处于外层 tag 的成功分支，不能提前到分支外。MIR lowering 在生成时保持同一稳定值的支配关系，相关转换由专门的 CFG／运行结果测试覆盖；后续投影与后端直接使用完整输出，不重新遍历全部表达式证明支配关系。

语言规范 13.11 的数据借用 intrinsic 在 MIR lowering 中展开为一次 receiver/block 求值、typed pin 帧 push、按精确对象种类取得数据指针与长度、普通 closure 调用以及所有正常／异常出口的 pop cleanup。block 为普通非 suspend 函数，帧不跨挂起保存。MIR/LIR 明确区分帧操作与数据地址操作，不把栈帧创建伪装成返回 native callee 栈地址的 runtime 函数。LIR 按实际 Array/String layout 计算元素区和长度字段；codegen 在 caller 入口分配帧存储，调用运行时规范 3.4 的 NoGC push/pop。数据地址按已确定的借用操作发射为 AS1→AS0 转换，不另附供自产 LLVM 复核的证明标记。原始数据指针是借用域中的 GC-free 值，帧中的对象承担保活与禁止移动职责；零长度数据地址不要求实际存在可解引用的元素。

所有调用以互斥分支确定 direct、virtual、interface、closure 或 function bridge target；各分支仅保存允许的 callee，closure／bridge 签名只保存一次。动态 receiver 为必填值，不从可为空的实参列表恢复。super 不重新进入动态分派；方法 receiver 为完整值参数，value receiver 的可观察存储属于本次方法调用。

structured MIR 与最终 MIR 的动态调用将 receiver 保存在对应目标分支，普通实参列表不再重复包含它；遍历、效果分析与 CFG 求值均先处理 receiver，再按顺序处理实参。virtual／interface 的静态声明目标只允许本地、单态化或外部 Scoop callable。已知 receiver 实际类型的去虚拟化先适配该值，再将其放入新 direct 调用的首个逻辑实参；closure／function bridge 直接消费各自唯一的函数签名身份。

最终 MIR 仅保存已经完成的调用与表达式。语言 Cast 在 mir-lower 的私有正文构造过程中展开，最终表达式类别不保留 Cast。协程注册操作和 pending cleanup chain 留在该 stage 的私有工作数据；成功输出不带临时 callee，也不保存恒为 Root 的 pending 字段。Return 携带完整语义结果，物理 void／ZST 分类属于 LIR。

CFG 的私有协程工作记录保留挂起操作的语义顺序、块内位置、receiver 与结果 local；尚未生成的注册调用不使用普通 callee 或空语句占位。协程变换把这些记录与已有 CFG 语句一起用于 liveness 和恢复分析，生成 continuation adapter 后直接形成完整接口调用。cleanup 中的待定 return／throw／break／continue 继续保留原值和目标，最终只输出引用实际 saved slots 与恢复入口的协程 metadata。

协程活跃度分别处理调用的正常后继与异常后继；异常边发生在实际调用处，不能统一放在块末再被后续赋值杀死。挂起前保存成功恢复、失败恢复及待定 cleanup 所需的值，失败恢复先还原异常处理所需的旧值，再进入原 catch／finally。成功路径在调用后覆盖的局部值，若仍被异常路径使用，也必须跨挂起保存；结果 local 只在成功恢复时定义。此规则同样覆盖挂起之后的普通调用失败和直接 suspendCoroutine 注册，不增加运行时根兜底或改变源码赋值语义。

release 在 suspend ABI 转换前，从 source-signature CFG 与私有 call-site 登记中确定本 Cone 直接可见的同步 suspend 正文：没有原语挂起、动态／外部 suspend 边、递归依赖或自身 EH／pending cleanup；这一事实可沿直接 suspend 依赖传播。普通 GC、可能抛异常或有副作用的非 suspend 调用不因此变成 NoGC 或纯操作。只在 2.19 的调用点评分、累计增长与六层展开深度范围内克隆正文，稳定模板不使用已展开函数重置成本；同步事实本身不能删除仍存在的调用点。

该克隆复用普通内联的参数求值、local 身份与 CFG 重映射，同时更新 call-site 的块、语句位置、语义顺序和调用点 cleanup。共享插入位置的待生成 suspend 注册仍位于原语句之前；callee 无自身 cleanup，克隆调用继承 caller 的 pending 链及异常出口。内联完成后才转换 hidden completion／step ABI，并只生成一次正式状态机。全部 suspend site 已实际消除的函数沿既有 Immediate 路径；剩余调用仍参与尾转交或状态机生成。该局部阶段不导入普通外部正文，也不把无帧尾转交判为同步。

release 在状态机生成前识别整函数的尾挂起转交。首版要求所有挂起点都是本 Cone 的直接 suspend 调用，结果 exact type 与当前函数相同；调用后仅沿未取地址的普通局部初始化／赋值复制和无条件边返回该结果，没有 EH、pending cleanup、Context restore 或其他结果处理。内联的返回汇合临时值遵守同一规则；不按临时变量名称放宽检查。可以有多个尾调用分支及直接返回普通值的分支。合格调用追加原 hidden completion，并直接返回 callee 的 CoroutineStep；普通返回仍构造 Completed。本函数不生成 frame／adapter，callee 可以真正挂起。无帧尾转交与无挂起点的立即完成以不同 lowering 分支表示，不能据此前者推断同步；原语注册、唯一完成及异常协议保持。该优化不要求 LLVM 机器 tail call，不承诺尾递归常数栈。

`==` / `!=` 保存已选定的 operator callable；若它来自普通用户接口，则保留该接口 application、slot 与实际 implementation。派生结构比较降为普通 typed 成员调用和短路控制流；`Equals<T>.equals` 是同一 operator 契约的普通接口调用。bound 调用在单态化时映射到已确定的实现，不能重新按名字搜索 equals。基本类型的精确 intrinsic 比较保持原运算与 effect；接口分派需要的适配正文同样是完整 typed callable，不能把 NoGc 直接实现误当成具有相同 effect 的 Managed slot 入口。

使用结构表示的值，其装箱接口闭包来自 LocalConcreteHir 的实际声明／继承关系，并由 MIR 的实际表项写入产物。tuple 的结构比较不增加 Equals 表项；Ptr 的显式接口仍按普通规则保留。产物读写检查接口引用、slot、callable 与签名的一致性，不因 exact type 使用结构表示就强制接口集合为空，也不在后续 stage 重放字段相等或源码 implements 的决议。

class allocation 只分配最派生对象一次，使用实际 exact TD；base/this initializer 不分配、不修改 TD。receiver 在可能 GC 的操作间保活并重读，只有完整构造成功后发布 release-ready 与结果。构造失败没有可用结果。

closure 具有完整捕获字段、类型、invoke 与扫描属性；绑定方法引用在调用时保留原动态分派。普通 local direct call 的捕获参数不改变原绑定身份。callback adapter 有精确 closure/native storage/status 签名，在返回 C 前处理全部异常。

suspend callable 的执行 ABI 为 `(source args..., Continuation<R>) -> CoroutineStep<R>`，后者仅含 Completed(R) 与 Suspended。输出不保留源码级 suspend call；frame、continuation、恢复入口和 saved slots 均有独立身份和完整类型。跨挂起只保存已初始化的 exact payload，不保存 native block address 或 active EH record。

接口 adapter 的物理 receiver 是具体对象引用；装箱 adapter 挂起时保存的 receiver 使用生成 box 的 exact type，而不伪装成源码 interface。其 `CoroutineSlot<box<T>>` 以 box 的 exact identity 区分，并与 box 一样归属 T 的定义或 application ODR group；扫描仍为一个 managed object 引用。普通 nominal 的 suspend 装箱 adapter 在优化前确定该 receiver slot，并随已发布的 box 支持输出；是否消除 adapter 的状态机不改变这项有限支持。结构类型／application 的内部 ODR saved slot 由实际 frame 所在实现物化，保留 foundation／Code／Link 数据，不作为公共 type／layout bridge 输出；它们不扩张私有 payload 的发布闭包，也不因优化消除而改变依赖指纹。普通源码 nominal 的有限 slot 支持仍按定义方发布。

object 的源码 exact type 与 backing class exact type 是同一对象的两种视图，共用分发表及 receiver adapter。adapter 的 implementor 保留源码 object 身份；backing 表通过类型记录中已有的 `Object { backing }` 关系引用它，不另建 callable 或放宽为任意继承 receiver。

completion 与 resume 具有唯一获胜方，并明确 payload 的 release/acquire 关系。恢复失败沿原调用点的异常／cleanup 路径传播。挂起状态使用 frame 的 TaskContext，恢复前后的切换见 2.16。

MIR metadata 保存源码声明到实际 callable、constructor、accessor、generated body 的映射，exact signature、GC effect、继承、dispatch、object/init 关系及 Strong/ODR 归属。它不定义目标布局；布局由 LIR metadata 提供。

mir-lower 完成 concrete CFG、dispatch、closure／adapter 与协程执行 ABI 后，由独立优化模块按 2.19 有界交替执行 receiver 实际类型传播、唯一目标去虚拟化、按调用点评分选择的内联、局部分配替换、接口视图复用与常量／CFG 清理；driver 显式传入优化配置。scoop-mir 保持纯数据职责，program-link 不隐藏重编译或导入普通外部正文。

release 默认启用 MIR 优化，debug 保留未优化的完整 MIR；两种模式的阶段 dump 分别记录实际输出，使用相同的实体身份与 ABI。私有 CoroutineDriver／CoroutineAdapter 属于实现正文，其实际身份与签名保留在 MIR foundation 和 Code 中，不进入供依赖消费的 Strong callable bridge；消除这些 helper 不改变依赖语义指纹。数据流在异常边合并可能抛出时的局部状态，不能只把 block 的最终状态交给 handler。实际表项或闭包 invoke／bridge 完整可用时才改写目标；外部目标使用已经选中的 callable 签名和引用。

函数内类型事实为不可达、未知或唯一实际类型。最派生 allocation、已知 box／closure 和 final class 提供事实，复制和合法转换传播，合流只保留一致事实，循环求不动点；未知返回、可变字段和取址后的可变 local 保守处理。base／this initializer 不收窄实际类型，initializing receiver 的使用仍受语言规范中的构造约束；静态 open class 或本 Cone 实现数量不构成全程序闭合。去虚拟化按原 typed slot 查询实际表项，一次生成含完整 callee 与 receiver 适配的新调用分支，保留原 effect、EH、Context 与 continuation。

已知装箱值或 final intrinsic class 的接口调用，也可消费 concrete HIR 中同一 exact owner／typed interface slot 的完整实现选择；这些语义关系随已使用类型保留，不要求在 consumer 重建 provider 的物理 itable。选中外部 primitive intrinsic 时保留其操作身份及完整签名，以原 primitive 下降生成优化正文；装箱 receiver 显式解箱，实参和结果必须匹配已选实现的 exact 类型。该正文与本地 intrinsic 一样强制展开，包含除零异常准备，不受普通内联策略影响；未知实际类型或不匹配的调用继续原动态分派。

自动内联处理本 Cone 已有完整 concrete 正文的非递归 callable，包括本地物化的外部泛型和普通 adapter；也可消费依赖公开的普通非泛型可移植正文。后者复用 shared-source 的可移植 HIR 正文格式，保留提供方声明身份，经 hir-lower 成为独立的完整 concrete 优化正文，经 mir-lower 的原正文下降和 CFG 构造进入同一内联成本模型。优化正文不是当前 Cone 的函数定义，不产生外部 Strong 的副本；没有正文或未选中展开时仍调用原外部入口。普通正文选择可完整移植的标量／值计算、字符串常量、字段读写、构造表达式、分支、while／do-while 与局部 break／continue、GC-free place 取址及 typed pointer cast／offset／load／store、普通调用；closure、Context、协程和局部异常处理仍保留外部调用。循环 target、条件 setup、pointer 的 exact pointee、读写及取址 local 身份复用既有可移植正文和 MIR 克隆规则，保留原求值顺序、循环 poll、异常和 GC 语义；取址存储属于该次展开产生的独立 local，不延伸其生命周期或引入 managed root。字符串保留提供方常量身份，字段与构造器使用原 typed 引用；字段 receiver 和构造实参按原顺序各求值一次，写屏障、异常、初始化与发布语义沿用普通下降。该选择不改变语言可用性或公共签名。

上述普通优化正文同时适用于顶层函数、名义类型的普通方法及属性访问器；声明的源码种类不构成顶层限定。只有完整正文的 owner 与 callable 类型实参均为空时才作为非泛型优化正文导入；泛型宿主及泛型成员继续使用已有具体化路径。成员的 receiver 从原可移植正文与名义声明取得，作为完整调用签名的首个实参下降，访问器保留独立的 typed accessor identity；不把实例成员改成无 receiver 的顶层函数。字段、继承/default 选择、调用 effect 和参数／结果表示沿原绑定传递，取得正文不授权去虚拟化未知动态调用或扩大源码可见性。导出仅判断当前可移植正文所支持的结构，不以节点数量预先决定 consumer 的内联收益；移除旧的 96 节点尺寸过滤。已导出正文引用的普通 helper 若同样可完整移植，沿现有 typed callable 依赖队列递归携带其正文；队列按真实声明去重并终止递归依赖，不把内部可见性改变为源码可见性。消费方可在内联暴露该调用点后继续按原成本模型评估，不能只导出第一层包装。消费方同步完成新发现的普通 callable 正文，只处理新增依赖使用，复用已完成的签名与正文；不能因只扫描一次而遗漏后续 helper。导入后的调用可保留已完整绑定的 imported callable application，包括原有 intrinsic 声明；该表示变化不使原本可移植的正文失去资格，下降继续使用其真实 typed 身份及签名。导出选择对各 profile 一致，实际自动内联仍为 release 默认能力；复用已有 shared-source 格式与 provider 物理入口，没有优化正文的相容旧库保持外部调用。

导入普通正文中的 callable-owned 字符串字面量引用提供方已经定义和登记的 immortal object，不在 consumer 复制对象或增加 GC root。MIR 保留常量的实际 provider；LIR 经既有物理依赖选择取得同一对象的 typed definition 与 symbol，仅发射 managed 地址空间的外部常量声明。物理依赖 subject 增加 `ImmortalObject`（wire tag 12），合同使用不透明 immortal 地址（tag 8）；对象内容、布局及登记仍由 provider 负责，不重读或重验完整 payload。新增依赖种类沿现有 Compile／Link 引用闭合传递，既有字符串身份、shared-source 格式和目标 ABI 保持。折叠后属于独立 property 的字符串常量暂不进入普通正文选择，避免将其声明归属误当成当前 callable 的 provider。

已选依赖 callable 的 MIR lowering role 为 `PrimaryValueConstructor` 时，按其完整 exact 签名与 owner 的既有 struct 字段顺序生成直接组装值的独立优化正文，复用本 Cone 主构造器的 MIR 语义。依据是语言规范 4.1.1 的主值构造器合同，不从函数名称、参数形状或空字段推测构造行为；默认实参仍由 caller 求值，secondary constructor 的委托／副作用与 class 初始化保持原调用。这类正文使用同一普通内联选择与增长限制，不作为强制 intrinsic 展开，不增加 Strong 定义或 `.slib` 布局。所有 profile 保留一致的角色、签名和外部入口，debug 保留调用；携带既有角色的相容库不要求新增正文记录。

递归 SCC、无正文的外部 Strong、native／callback／gateway 和无法完整重写的特定 EH／Context／协程控制流保留调用。内联重分配 local、value、block 和临时存储身份，连接 return／异常出口，保留实参各求值一次、独立按值 place、位置、初始化发布和清理。Managed、分配或可能抛异常本身不构成拒绝理由；不复制后端 root/frame plan，站点由后续 LIR 统一形成。导入正文的局部值仅在实际展开时取得 caller 的新身份，候选之间以本地函数或外部 callable 的不同 typed id 区分，防止跨 Cone 调用环反复展开。

含 EH 的普通正文可以按 2.19 的调用点事实先消除已经证明不可达的异常路径；只有剩余正文可由现有内联器完整展开时才入选。原生异常状态、重抛与清理的残余路径仍保留原调用，不通过共享函数内异常槽模拟新的内联异常作用域。

首版克隆不含自身 landing pad／catch／resume 协议的普通 CFG；callee 的可能抛出操作与显式 throw 继承调用点的异常出口，caller 已有 catch／finally 可以继续承接异常。依赖函数局部异常槽的 callee 及协程执行入口保留调用。克隆产生的局部值属于 caller 的新 SyntheticValue 身份，所有形参先取得独立局部副本；常量折叠只处理可证明无副作用的布尔与整数运算，整数保留对应 MIR 运算的精确位宽与符号规则。地址可见的 local 不参与常量替换。Managed callee 的循环保留 poll 标记，NoGC callee 的循环内联到 Managed caller 后也不得新增 poll。

多个内联 return 先写入独立的汇合临时值，再在 continuation 初始化原调用结果；不得把原本稳定的结果 local 改为可重复赋值的身份，破坏其后 variant test／payload projection 的关系。优化删除引用后，已选外部 callable 声明目录可以保留未使用项及其稳定索引；声明的完整签名仍保留，实际链接成员由对象中的真实引用选择，不因目录项无调用而拒绝合法 MIR。

带 release 的对象构造保留同块相邻的 allocation、最外层 initializer 调用与 release-ready 发布。内联可以整体复制包含该序列的普通正文，但对应的最外层 initializer 调用本身保留，不将发布拆入缺少构造前序的 continuation；异常路径仍不发布。

输出为 Library 或携带本模块非可选 entry 的 Executable，不能由 `Option<Entry>` 与独立 kind 拼成矛盾状态。entry 保留 HIR 已确定的四种形态、本层 main target 和完整签名；生成的 root gateway 明确区分成功退出码与失败状态，有参数形态包含受检 argv 构造与 main 调用的完整正常／异常路径。gateway 与相关 helper 是实际需要发射的 typed callable，不能留到 program-link 阶段生成 managed 语义。

### 2.4 LIR

LIR 以完整操作表达托管分配、字段／数组写入、值复制、原子引用访问、`ManagedPoll` 与 `PublishReleaseReady`，携带实际类型、布局、GC effect 和 live set。Immix 普通对象上限、TLAB 资格及 card/page-map 算法属于 codegen 的 GC 实现，不作为通用 LIR 策略常量；实际 runtime 符号、共享布局和兼容性摘要仍由现有 ABI 数据目录保存。

codegen 的 release 小型常量分配按运行时规范 3.1 直接完成 TLAB 登记，使用统一的四指针 context 前缀与四项 atomic u64 计数视图：依次为 nursery 对象数、nursery 字节数、直接旧代对象数、直接旧代字节数。内联成功分配只更新前两项，runtime 在读取时合成总量；不对同一次分配同时维护子集与总量。成功 bump 后，在对象 GC 字写入已对齐的精确大小，再按原规则发布 starts；不再生成大小数组的地址计算或 u16 登记。GC 字大小／pin 位域保持；context 是当前线程稳定 mutator 的 32-byte 前缀，计数器直接从该地址的 offset 128 寻址，不逐次加载计数器指针。完整线程状态仍为 256 bytes，计数原子操作和 nursery／direct 域含义保持；此布局由 runtime contract 26 规定，所有 profile、目标和预编译机器正文必须配套重建；不改变 16-byte 对象头、scan 或通用 LIR 操作。复用输入的完整本地 TD／typed array layout，跨物理对象分区保留已确定的 alignment 与 release-policy；外部 TD 缺少事实时保留原加载与完成入口，不扩充跨 Cone 格式。内联只在大小合格、布局已知、确定无 release hook 的成功分配分支内发射，不增加 LIR 操作、持久证明、managed 引用或根。不再生成 line 位图的地址计算、读写或跨字分支；line 占用由 collector 的完整标记／复制与收尾产生。分配结果按实际快路径出口与原慢路径出口构造 phi，慢路径 safepoint／relocation 保持。debug 与未命中的分配使用相同 ABI 下的既有完成入口。

codegen 对上述小型普通对象分配可分析结果之后的确定初始化路径，按运行时规范 3.1 缩短清零范围；ARM64 的对象上限为 128 bytes，x86 为 64 bytes。x86 更大对象的试验出现寄存器分配变化与不稳定收益，暂保留完整清零，不扩大默认范围。分析只消费当前 LIR 的 typed store、局部副本、无条件 CFG 边与既有值布局：只将实际存储完整覆盖、无 padding 的字段计入已覆盖字节，取其连续尾部，清零前缀至少保留 16-byte 对象头。局部被重新赋值时更新别名，取址局部不参与；有界对象范围之外的写入不提供覆盖事实。未知操作、可观察操作及循环回边结束分析，未证明部分按原样清零。原字段写入与快／慢分配合流保持，不增加持久 LIR／`.slib` 字段、runtime 分支、对象字段或 GC roots；跨 Cone 内联后按同一规则消费完整正文，未内联调用结束分析。

ARM64 release 的 TLAB context 地址在每次函数调用内按首次使用取得并复用。codegen 用入口初始化的非逃逸 AS0 局部槽表达空缓存，各实际分配点只在缓存为空时读取 TLS；交由既有 LLVM 标量优化提升为 SSA，不把取址无条件移到只含失败／异常分配的函数入口。若已有常量资格、大小或溢出条件直接否定 TLAB 路径，则先选择原 collecting slow path，不取得 context。首次 TLS load 使用 volatile 约束执行位置，避免 LLVM 将条件取址变成无条件取址加 select；这不是线程同步，缓存和 context 字段仍按各自普通／原子访问规则处理。缓存跨普通调用、GC、refill、异常处理和同步 callback 保留，依据运行时规范 3.1 的 attachment 生命周期；各分配点继续加载当前 cursor、limit 与块元数据，不复用这些可变内容。x86 试验收益不足以抵消宽对象路径回退，沿用逐次读取；debug 也不建立该缓存。统一 runtime ABI 与 LIR／跨 Cone 格式保持。

`ManagedPoll` 表示运行时规范 3.2 的条件 poll，携带唯一慢路径 site 与完整 live set。codegen 发射 acquire 检查和快／慢分支，只在慢分支物化该站点的 managed leaves、调用协调入口并回写 relocation；后续使用通过普通 canonical storage／SSA 合流取得快路径原值或慢路径新值。Full 模式用于已激活的普通 managed 执行，只读取 world phase；Gateway 模式保留入口 pending 的 mode／epoch 检查，不能仅按 world phase 删除首次激活。TLS 与全局数据引用使用闭合 runtime ABI 目录中的普通数据合同。

普通 Managed call 的 LIR 根计划分别记录调用期间所需的来源与返回后是否需要恢复。直接 ABI 的 managed pointer／接口 object 实参由 LLVM 的实际 AS1 call operand 保持为调用期间的 root；返回后已死的来源不生成恢复 `llvm.fake.use` 或 leaf 写回。正常后继仍活跃的来源保留恢复；Indirect／byval 实参的内部 leaves 沿显式 leaf 暴露与恢复路径保留，不能把 AS0 storage 地址当作 managed root。codegen 展开分配、装箱、数组与 String 复制时，分配后仍使用的输入同样必须恢复，即使源码层操作之后不再读取它们。poll 的活跃值全部需要恢复。invoke 按已有 normal／unwind 两份使用条件恢复显式 compiler roots；native transition 保持原发布、borrow 与返回握手协议。最终逻辑根身份只要求实际恢复的 leaves，物理 root 数继续消费 RS4GC 的实际结果，包含只有调用期间用途的 direct AS1 实参。此内部 LIR 信息不改变 `.slib`、调用 ABI 或 runtime stackmap 格式，全部 profile 遵守相同规则。

debug 在原读取位置将含 managed 引用的普通聚合 load 分解为实际字段读取，保留目标布局、对齐与源码位置；volatile 读取保持原有访问。随后执行 SROA／mem2reg／SCCP／EarlyCSE／InstSimplify，合并无中间写入的重复读取，避免调用实参与后继根的等值读取直到机器下降才合并。debug 不运行会移动或重组用户计算的 InstCombine，保证仍有实际计算的语句能在原求值顺序中设置断点。两种模式都在晚期聚合分解和 RS4GC 前完成读取规范化；release 复用原有 InstCombine 等优化步骤，不增加独立根等价分析。该 profile 规则与调试信息级别正交，不为调试新增读取或保活。

LIR lowering 在最终 CFG 上按运行时规范 3.2 省去有界普通叶函数的入口 poll，普通入口与循环选 Full。root／initialization startup gateway 由已有的完整 callable body key 区分，入口始终选 Gateway；callback adapter 沿 runtime 已激活协议选择普通模式。叶函数资格由明确的操作集合、无环直线路径和小值复制决定，不依赖源函数名称或调用方推测；codegen 消费已选模式，不再重做资格分析。所有 profile 一致，缺少 poll 的合法 body 仍有 callable registration；既有 `.slib` 与 ABI 相容，不新增跨 Cone 效果承诺。

driver 按“lir-lower 准备布局与逻辑签名 → codegen 规划目标 ABI → lir-lower 完成正文”的顺序编排，stage 之间只传递 LIR crate 定义的完整数据。准备结果与最终 Module 是不同类型；不得用缺失签名、默认 ABI 或完整 lowering 后丢弃正文重跑来实现准备。布局、scalar leaves、扫描信息与已选依赖 ABI 在准备时建立／映射一次并复用。聚合输入以 typed 字段引用共享已准备的存储节点；不按逻辑字节数展开大型 tagged payload，也不复制嵌套值布局。后端只为可能使用寄存器的聚合读取其标量叶子。

具体 Scoop/C ABI 分类器属于 codegen，按完整参数序列、结果和逻辑类别生成计划；覆盖本地／外部函数、虚／接口槽、closure／bridge、suspend 执行入口、callback、gateway、release 与 runtime helper。计划通过原有 typed callable/type/slot/runtime entry 身份或专用批次局部 ID 关联；定义、调用、adapter 与导出复用相同计划，不按 symbol 或参数数量猜测。通用 lir-lower 只消费布局事实和物理计划，不登记架构、不执行目标 ABI 算法；LIR/identity 仅保留完整数据、编码和必要的局部结构检查。

LIR 保存目标上完整的类型布局、值表示、字段 offset、alignment、scan、调用 ABI、控制流与实际定义／引用。codegen 不按名称、实参、result storage 或上下文补类型、签名、poll 或逻辑 live set。

固定的 LIR foundation 为实际定义建立一次 plan 与 plan/role 查询索引，后续解析和 registration 检查复用这些索引，不为每次查询扫描全部 atom。索引保留同一 plan/role 下的所有定义，唯一目标查询仍区分缺失和歧义；它不进入产物格式或实体身份，返回的定义集合保持原有 canonical 顺序。

definition、ODR member 与 symbol request 的查询复用各自已有的 canonical 排序；symbol request 命中仍须同时匹配 key 和 linkage。callable body 保留依赖顺序，通过与不可变 body 表共享的 member→body 索引解析 ODR 定义，不为每个定义扫描整张 callable 表。codegen 删除 atom 后同步更新 foundation 的 plan/role 与位置索引，保留原 foundation 的共享查询结果；这些查询优化不增加语义复核或 wire 数据。

foundation 按 typed entity／role 反查 ODR 定义时，共享一次建立的 subject→member 与 body→位置索引；保留 Strong 定义优先、原表中首个匹配项及缺失记录的行为。codegen 删除 ODR member 后更新相关索引，原 foundation 与克隆仍各自返回所属记录，不因索引复用跳过实际类型、引用或格式检查。

对象 symbol surface 同样为 atom boundary 建立一次查询索引，按 atom 的 typed identity 返回既有边界符号；逐 callable／safepoint 发射只查询实际需要的 atom，不重复建立整个 Cone 的 boundary 表。codegen 删除未发射的定义或 atom 后同步更新索引，不能返回已删除记录；索引不写入 wire，也不改变既有 symbol 或定义顺序。

callable 对象发射直接借用 production 中已有的 runtime scan 计划，按 body identity 取得本函数的扫描树与 Context key；不在每个 LLVM module 中重新扫描全部函数来构造相同计划。Context cell 与扫描 atom 的边界也通过上述索引取得；无 callable 输出的 metadata module 不另建扫描计划。实际扫描内容、发射顺序和 GC 契约保持。

逐 callable LLVM module 直接接收已选定的完整函数，声明范围由该函数自身、函数局部 typed call targets、函数取址及 foreign callback 注册确定；后者包含已有 adapter、trampoline 与签名 descriptor。声明在正文发射前完成，保留完整 ABI、GC effect 与真实目标身份，不扫描或重复声明整个 Cone 的 callable 目录。非 callable metadata module 继续声明其完整引用表面；初始化、TypeDescriptor、全局值及对象划分合同不变。该局部选择只减少 LLVM 临时声明，不修改 LIR arena 或产生缺项的 callable 数据。

ELF 可见性与 COMDAT 准备将固定 symbol surface 的实际符号名和所属定义组索引一次，在各 callable LLVM module 中只查询该 module 实际包含的 global、function 和 alias；不逐 module 重建全部符号名或探查未发射的 atom。保持原有计划顺序、可见性、weak linkage、alias owner 与定义组冲突检查。定义计划完成最终裁剪后，metadata module 使用裁剪后的计划；该 codegen 局部查询索引不进入 IR、wire 或 runtime ABI。

每个 pointer（包括 null）具有 Managed、Raw、Code 或 Metadata provenance；LLVM opaque pointer 不抹去这种区别。managed 使用 address space 1，其余使用 0，转换必须为明确合法的 typed operation。

LLVM module 的 data layout 将 managed 地址空间声明为 non-integral pointer（生产配置为 `ni:1`）。普通复制仍按精确 storage 执行，但优化器不得从整数搬运值自行重建 managed pointer；GC leaf 从 typed pointer load 取得。分配结果、card 地址和 scoped data borrow 的显式 typed 转换继续按各自合同检查，此声明不改变目标的指针大小、对齐或原生 ABI。

Scoop ABI 保留完整逻辑参数序列及 exact identity，物理分类为：

| 值 | 参数 | 结果 |
| --- | --- | --- |
| Unit / ZST | ElidedZst，仍求值 | Unit 为 UnitVoid；其他为 ElidedZst。 |
| scalar、单字 managed/raw/code pointer 及其 niche enum | Direct | Direct |
| interface 及其 niche enum | DirectParts：object、itab | Darwin／SysV：DirectParts；Win64：Indirect |
| size ≤ 16、alignment ≤ 8 的 GC-free struct／tuple／tagged enum | 按目标分类的 DirectParts 或所需间接形式 | 同一分类的直接或间接结果 |
| 其余非 ZST aggregate | Indirect | Indirect |

DirectParts 明确保存每个 carrier 的类型、来源 offset／extent、alignment 与 provenance，以及完整返回约定。接口为 AS1 object 和 AS0 metadata，不能落入单 scalar 分支；slot 的隐藏 receiver 独立采用单字 object projection。所有 caller／callee、typed invoke、adapter、Scoop ABI extern 和跨 Cone 签名消费同一计划。

Boolean 标量的物理类型为 LLVM `i1`，直接参数与直接结果使用 `zeroext`；caller 在传参时、callee 在返回时按目标寄存器规则零扩展，确保 C runtime 的 `_Bool` 入口接收规范的 0／1。函数定义、声明、直接与间接调用、普通 invoke 和 statepoint 中实际调用的参数／结果遵守同一规则。仅低位有效的中间 `i1` 不能原样作为 C `_Bool` 传入；aggregate carrier 不因内部包含 Boolean 而获得标量扩展属性。此约定纳入现有 target fingerprint：Scoop ABI classifier 编码由 3 更新为 4，旧产物与缓存重建，不增加独立登记或验证通道。

LLVM 22.1 不保证 RS4GC 改写 aggregate 内部的 managed leaf。接口及含接口 niche 的实际返回、存储、后续传参必须使用当前 object，与原 AS0 metadata 重组；仅存在 gc.relocate 或恢复标记不能证明实际使用已更新。按 2.5 在普通优化全部完成后拆分含 managed leaf 的 aggregate 数据流，再交给 RS4GC 改写实际标量使用，防止 InstCombine 将拆分与重组折回 safepoint 前的旧 aggregate。外部双字表示、niche 的 null／metadata 规则和既有 ABI 保持。

GC-free 小值按 exact layout 和 scalar leaves 复用下述目标 aggregate classifier，保留整数／浮点／混合寄存器类别；tagged enum 的固定 tag 和共享 payload 字节区域不随 variant 改变分类。每个 aggregate 作为整体分配寄存器或回退；coercion 不越过 exact storage，padding 确定，浮点按 bits 搬运。callee 按需要建立独立 place，不能把 caller storage 当成按值别名。MaybeUninit 依自身值表示分类，不继承 T 的 nonnull、tag、niche 或专门接口 ABI。

编译器检查源码 FFI 声明的类型与 ABI，并按实际声明生成调用；codegen／LLVM 对自身产生的复制、清零、比较、浮点及 TLS 等辅助调用负责。普通 libc／工具链辅助引用按实际 native 未定义符号和 relocation 保存，不要求逐项进入手写登记表。无 GC 的辅助调用不新增 Managed site，也不能满足或替代周围的 Managed invoke 记录；Scoop runtime ABI 与目标 EH 的专用类型合同继续检查。

小值的参数与结果分别保存 coercion：SysV 参数展开为最多两个 INTEGER／SSE carrier，结果为单 carrier 或双 carrier 结构；参数分类在完整签名中扣除隐藏 sret 和先前参数占用的 GPR／SSE，任一寄存器类别不足时整个 aggregate 使用 byval，未使用的另一类别寄存器留给后续参数。Darwin 普通 aggregate 参数按 64-bit 字块（需要 16-byte 对齐时为 i128）传递，短结果可使用精确位宽整数；HFA 使用保留连续寄存器约束的浮点数组 carrier。HFA 必须由 1 至 4 个同精度浮点叶组成，且叶连续覆盖完整 storage；额外 padding 不算浮点成员。Darwin 的连续 carrier 由 LLVM 的 aggregate 参数规则整体放置，不能展开成彼此无关的标量参数。

carrier 记录实际读取的 extent，允许其机器类型宽于 extent；读入前将 carrier 临时存储清零，只复制 extent 字节，回写亦仅复制该范围。跨 Cone canonical ABI 保存完整 carrier 序列而非仅保存 DirectParts 标签，target classifier 的版本同时进入现有 target fingerprint。产物消费者复用保存的物理计划并检查 exact storage／引用一致性，不另行通过源码重建一套参数分类。

Win64 的 GC-free aggregate 仅在 exact size 为 1、2、4、8 bytes 时使用对应位宽的单个整数 carrier，其余使用间接形式；单成员 float／double struct 也遵守该整数 carrier 规则，不沿用 SysV SSE／Darwin HFA 分类。普通 scalar 与 interface 参数仍按顺序传递；interface 展开为 AS1 object 与 AS0 metadata 两个物理参数，interface／其 niche 的结果使用保留 `RefScan` 的 exact 间接 storage，不能先产生再提取 LLVM 隐式 sret aggregate。该结果计划沿现有 canonical ABI 的 Indirect 分支保存，consumer 复用其表示，不增加第二种签名格式。

每个 indirect 实参使用调用方新建的 exact storage，callee 按值接收。物理参数为 indirect result storage（若有），随后按逻辑顺序省略 ZST 并传递其余参数。Darwin／SysV 的 Scoop indirect 参数使用对应 exact type/alignment 的 byval；Win64 使用已经创建的 caller copy 地址，临时副本至少 16-byte 对齐而不改变值本身的 exact layout，不能添加改变机器传参方式的 byval。各目标间接结果使用 sret。Darwin sret 使用 x8，SysV amd64 使用 RDI，Win64 使用 RCX；两种 amd64 ABI 均在 RAX 返回同一地址。Win64 的前四个物理参数按位置使用整数／浮点参数寄存器，其后使用栈，shadow space、栈对齐和探测交给 LLVM；不使用 SysV 的两组独立寄存器计数。Win64 C ABI 的小整数不添加 signext／zeroext，Boolean 仍使用 zeroext；Scoop Boolean 规则保持上述约定。

ZST 的求值、调用、构造、类型及对象身份不能因零 payload 消失。被观察地址的独立 place 有满足 alignment 的存储 token；同时存活的不同 place 不共享地址。Ptr<ZST> 的元素位移为零，仍遵守 operand 求值和 unsafe 有效性条件。

call target 同时确定 destination、calling convention、全部参数与 return convention；void/direct/indirect-result 以及 Managed/NoGc/NativeSafe/NativeGcLeaf/NativeBorrowed 使用完整互斥分支。Managed call/poll 带 site ID 与逻辑 live set，invoke 带正常／异常 root set；NativeSafe/NativeBorrowed 带 site ID、caller roots 与按返回形式确定的 result publication。NoGc 与 NativeGcLeaf 不携带虚构 safepoint 或 root plan；NativeGcLeaf 保留完整 C ABI 调用计划，不能伪装成普通 Scoop NoGc ABI 调用。C extern 的物理调用计划为 DirectC 或 StorageBridge，与 NativeSafe/GcLeaf 协议独立。

NativeBorrowed 的调用与结果发布按返回约定一次构造：间接结果复用调用的 result storage，含 managed scan 的直接结果携带对应 root storage，void／ZST／GC-free 结果使用不需要结果 root 的分支。C 与 Scoop struct 的专用布局接口保留具体表示类别，不接受不适用的通用 struct 后再断言。存储分配和表示转换由 lowerer 完成，IR crate 只保存完整数据。

live set 包含 post-site live references 与 managed 实参，以及 aggregate 的完整 managed leaf path。普通 call/poll 的机器 relocation 与显式 invoke/native root frames 遵守运行时规范 3.2；跨 transition 的实参和结果必须使用更新后的存储。

root gateway 使用运行时规范 2.8 的精确 C 原型：argc、原始 argv、指向独立 Int32 退出码槽的 raw pointer，返回 UInt32 status。其内部对 main 的调用仍使用实际 Scoop ABI；不能因外层返回 UInt32 就覆盖 main 的 Int 结果。入口 poll 先于 argv 的 managed 构造，数组、当前 String 与其他跨 safepoint 的引用使用普通 roots、relocation 和写屏障；数组构造及 main 的异常在 gateway 内完成捕获、failure-root 发布与 EndCatch。BeginCatch 返回原 managed Throwable，普通 catch、callback 和协程 lowering 直接保存该引用，不生成复制对象的调用；后续根与 relocation 沿普通引用处理。eager gateway 仍为无参数 status 返回，不能与 root gateway 共用错误的函数指针原型。LIR lowering 生成完整 ABI 与 gateway 结构，codegen 直接发射；对象发布不重放源码 main 或已完成的 gateway 检查。

值类型布局使用原 exact 类型与字段身份。tagged enum 的 tag、payload slot 与 scan 一致；niche 只用于语言允许的引用／pointer Option。整数宽度与 signedness、Float/Double 精度、内部计数和 runtime ID 均保留各自类型，物理位宽相同不等于同一实体或源码类型。

wrapping 整数运算不携带 nsw/nuw；除零和 signed MIN/-1 已有安全控制流，shift count 已按位宽正规化，compareTo 的结果为 Long。浮点运算遵守 2.18。

C ABI storage 为封闭类型结构：integer、Boolean、Float/Double、带完整 pointee 的 data pointer、带完整 signature 的 function pointer、完整 C-layout struct。只有结果可为 void，只有 Ptr<Unit> 的 pointee 可为 opaque void。by-value struct 关系无环，pointer 可引用 forward-declared struct。

nullable data/code pointer 保留对应 exact enum 与 pointer 类型，不得把同一 enum 绑定到不同 pointee/signature。CLayout 的 aligned/packed 各为 Natural、A1、A2、A4、A8 或 A16。C ABI storage 与 Scoop 值表示分别准确保存；C ABI 分类不能用 Scoop aggregate ABI 代替。

在当前 Darwin/AArch64、Linux/amd64 GNU 和 musl profile 上，固定参数的 cdecl C extern 未选择 `captureErrno = true` 时，合法 canonical C signature 必须使用 DirectC，包括 integer、Boolean、Float/Double、data/code pointer 和按值 C-layout struct。条件按已有 C-FFI-safe projection 判断，包括 Char、PinnedPtr／GcHandle 与合法 nullable pointer；不把普通 C-layout struct 凭大小或字段数当作标量。Unit 仅映射为 void 结果。errno 捕获继续使用 StorageBridge；本条不扩展 callback、FunPtr 或 native global/TLS 协议。

GNU／musl 共享 SysV AMD64 eightbyte 分类，保留 INTEGER／SSE／MEMORY、整 aggregate 的寄存器耗尽回退、byval 和 sret。Darwin/AArch64 使用对应 AAPCS64／Darwin 分类，保留 HFA、普通 aggregate、栈参数和间接结果；四个 Double 的 HFA 不受 Scoop 16-byte 小值阈值限制。完整物理签名覆盖 nested layout、padding、alignment、扩展、coercion 与隐藏参数顺序。classifier 只消费已有 C projection 和完整签名，不建立运行期 libffi 或 FFI 插件；以所选目标 C compiler 产生的独立函数作互调验证。

DirectC 由 codegen 的 ABI 规划入口根据 LIR 准备提供的 canonical C signature 与目标 C ABI 确定完整的物理参数/结果、calling convention、小整数扩展和 Boolean 表示转换，codegen 按该计划发射 LLVM 类型与 ABI 属性，目标后端分配寄存器及栈位置。不构造 storage bridge 专用的参数局部变量、返回缓冲区或 memcpy 往返；目标 ABI 要求的栈传参、byval、sret 与表示转换保留。StorageBridge 保留完整 storage signature，由系统 C compiler 生成桥接；不能只设置 LLVM C calling convention 就把未经分类的 struct 直接传递。

DirectC 的聚合值计划复用带 offset／extent／alignment 的 carrier 描述，并分别保存参数与结果 coercion。SysV MEMORY 参数记录 `byval` 及至少 8-byte 的参数副本对齐，隐藏 sret 使用结果的实际对齐且占用首个整数参数寄存器；寄存器不足的聚合参数整组回退。Darwin 超出寄存器 aggregate／HFA 分类的参数传递 caller 的独立副本指针，不标为 SysV `byval`，间接结果使用 sret 的 x8 约定。副本对齐与原类型的存储对齐分开保存；原始 packed storage 不因 ABI 要求而改变布局。

当前三个 profile 的 DirectC 参数与结果中，有符号／无符号 8-bit、16-bit 整数分别使用 signext／zeroext；Boolean 使用 LLVM i1 与 zeroext，32-bit、64-bit 整数及浮点、pointer 无整数扩展属性。TargetProfileContract 的 C ABI lowering 记录 aggregate 分类合同，进入现有 target fingerprint 和构建缓存键；迁移时登记新 wire tag，退役的 ScalarDirectOrSystemCBridge tag 2 不复用。

`captureErrno = true` 的 LIR 计划分别保存真实 C 函数结果 `R`、bridge 的 `Int32` 返回值，以及 Scoop `(R, Int)` 的 exact layout 和结果重建操作。带捕获的 bridge 继续通过原有 caller-owned C result storage 写回非 void 的 `R`，以 C `int32_t` 返回捕获的 errno；`R = Unit` 时省略原结果缓冲区，在 Scoop 侧构造零 payload 的 Unit 元素。普通不捕获的 storage bridge 保持原有返回方式。tuple 不出现在目标 C 函数原型中，也不作为 C struct 返回；必要的 C/Scoop 表示转换在已保存 errno 后完成。

每次调用使用独立、在 native 调用期间地址稳定的 GC-free 临时存储；嵌套、递归与 callback 内的调用不能复用仍在使用的缓冲区。NativeSafe 调用在返回握手后使用结果，GCLeaf 与 release 按各自无切换路径使用结果。捕获值及包装 tuple 不增加 GC root、Scoop TLS/thread 操作或堆分配，也不能代替原有实参保活与指针生命周期规则。

compiler-owned static storage 带完整 scan 和封闭初值 ZeroedForRuntimeUnit 或 EncodedStaticValue；后者具有 canonical bytes 与排序的 immortal relocations。raw global/TLS 另存完整常量，不进入 managed static-root registration。ZST token、全零初值与 relocation 规则见运行时规范 2.8。

LIR metadata 保存完整 value/instance/element layout、Scoop/C ABI、scan、TD、dispatch、initialization、runtime registration 与 required definitions。ordinary nominal 定义使用原 provider 的布局；实际泛型或 structural application 使用其 ODR 布局。结构字段可按 canonical exact key 组成，不要求为没有实际物理定义的 constituent 另造 layout 实体。

每条 native contract 按 target 正规化 symbol，并保存完整 kind/library/signature/storage/GC effect；它进入 LIR 语义内容，host 搜索路径不进入。String 关联实际声明的 TD 与 provider，不由名字或相同 InlineBytes 布局代替。

整型枚举的 MIR/LIR 保留 exact enum 身份并直接使用底层整数标量布局；variant 构造产生其已检查常量，匹配按该表示区分 variant。参数、结果、字段、数组及 C ABI 使用同一整数的 size、alignment、signedness 与目标分类，不添加 tag、payload 区域或值域检查。boxed enum 的父 TD 指向 boxed integer，父整数解箱与函数值适配遵守运行时规范 2.3。Option 的现有 niche 范围不变，源码 enum 身份不能因机器签名相同而合并。

### 2.5 Codegen 与对象合同

GC 代码生成模块集中实现分配、完整托管写操作、发布与 poll 的快／慢路径。字段与数组 store、含引用聚合复制、数组构造／拼接、clone 及原子引用写入必须把实际写入和对应屏障放在同一操作中；通用发射器交付实际地址、值、类型／scan、内存序和必要的 safepoint 数据，不单独拼接写后屏障。后续 LLVM 优化可按 2.19 合并完整有界内层中的等价 card store，仍须在下一次可能 GC 或发布前完成。批量构造／复制保留整段范围屏障，失败 CAS 不标脏，GC-free 写入不增加屏障。runtime 执行的 clone／boxing 等完整操作沿既有入口调用，内部写入遵守运行时规范 3.6。

分配、单槽写入与 poll 的既有快路径仍静态内联，不为模块隔离增加外部热路调用或重复 TLS 查询；保持原内存序，地址计算、写入与屏障之间无 safepoint。根活跃度、statepoint relocation、异常控制流与调用 ABI 继续由原有模块负责。纯内部职责重组不改变共享布局、runtime ABI 或产物格式。

codegen 的 LLVM 发射入口只接收当前 Cone 的完整 LIR；不在后端重新遍历全部类型、CFG、签名、variant 支配、callback、常量初值、safepoint 身份和根计划来复核上游语义。发射按现有 typed 数据执行实际操作，保留当前查询／计算的范围、引用与 LLVM builder 错误；相关语言和根计划规则由原有前端／lowering 完成。generated-C 同样直接渲染既有 bridge 与 C 布局，不重建 canonical layout 或 atom 计划副本作比较；真实目标 C 编译的 sizeof／alignof／offsetof 断言继续保留。

启动 gateway 的完整控制流由 lowering 生成，实际入口、参数、失败根与 GC 行为通过转换和运行测试覆盖；LIR crate 不保留只由测试调用的 gateway 全体结构复核器。

发射前的 ABI 规划入口只消费 LIR 定义的布局／签名数据，不读取 HIR 或重新解释 MIR 正文。具体 LLVM 初始化、CPU/features、data layout、机器栈与过程 ABI 规则归 codegen；目标布局描述与 backend contract 作为完整数据传递。Scoop producer 消费 LIR target/backend，generated-C 源码生成消费 LIR target/C toolchain 合同。宿主 compiler/SDK/sysroot 路径、环境与命令构造归 toolchain，driver 执行 generated-C 编译后将对象交回现有对象边界处理。输出可以有多个对象，物理分片数量与文件名不是 Cone 或实体身份。

逐 callable 发射使用的整库函数 GC effect 表建立一次，函数计划及优化后计划共享这份只读数据。每个函数的 site 集合在入口按所属 body 分组一次，与根分组独立保存；不为每个函数重新扫描整库 site 集合。保留已生成函数计划时不逐份复制整库属性表。共享只改变进程内所有权，不改变 GC effect 选择、safepoint 身份、输出或 ABI。

每个 callable 的 LLVM module 只声明正文实际引用的类型描述符、数组描述符、普通全局量和初始化登记，并完整发射该 body 所属的 C 字符串等关联定义。归属和初始化登记的查询表按 Cone 建立一次；元数据分区仍取得其完整定义。逐函数 module 可复用同一个 LLVM Context，但对象、ODR、registration 与 GC 的物理选择边界保持独立，不以合并函数对象消除准备开销。

ELF callable module 在普通优化、statepoint lowering 与实际 safepoint 登记完成后，先用 LLVM 的全局死引用清理去除无实际使用的声明及其孤立常量使用链，再准备最终 visibility／COMDAT 并发射对象。清理覆盖当前 module 的全部声明，包括不在当前 Cone 定义计划中的外部声明；不能仅凭原始 use-list 非空推断仍有代码或数据引用。真实引用、Strong／ODR 定义、alias owner 和登记数据保持原链接合同；不可达的 TD 不因无重定位的残留 hidden 声明而成为链接根。元数据 module 复用既有分区清理，不增加链接期保留全部声明或 TD 的规则。

元数据对象按已经确定的物理定义集合投影。实现先按对象序列逐层拆分候选模块，再从较小的模块生成最终对象，避免为每个对象重复复制全部 LLVM 元数据。每层只保留所选定义及其实际引用声明，沿用同一别名、TLS 边界与可见性处理；拆分过程不改变最终对象划分、符号、布局、注册内容或 ABI。

codegen 保留 LIR 已分类的数据关系：direct callee 携带实际 LLVM function，dispatch 携带对应分派数据；native transition 的准备分支一并取得 safepoint 与 roots，发射入口握手后保留退出所需的 roots、frame 与结果存储，不拆成需要彼此同步的可选参数，也不额外保存已消费且后续不再使用的状态。LLVM builder、对象及目标发射的实际失败继续走后端错误路径。

release 在内联与有界循环选择完成后、最终 LIR 根计划之前，复用 MIR 数据流传播未取址局部的当前复制事实。扩展的复制事实只服务该末尾清理，不改变既有调用点内联与循环选择策略。每次局部定义后建立同类型别名；重赋撤销以该源为代表的别名，保留副本自身的值、常量与具体类型事实。合流只保留各已到达路径共同成立的事实，循环按既有固定点计算，不因定义位于循环中就统一排除。调用不能隐式改写未取址局部；有取址的源和目标不作为可替换代表。消除不再读取的纯局部复制时保持可变源的旧快照、异常路径及实际定义／使用关系，不按生成变量名识别，也不令两个可变局部共享槽。

release 在普通 LLVM 优化之前，将未逃逸入口栈存储上的编译器 GC 专用 load／restore store 转为普通访存，交给现有 SROA／mem2reg、复制传播与死存储消除。按实际地址使用确认不逃逸：局部 GEP／bitcast、load／store 和 LLVM 内存／生命周期 intrinsic 可保留；取址传给普通调用、发布为 compiler root、存出地址、间接 ABI 或未知使用保留必要存储与 volatile 重读／回写。用户原子、volatile 操作和 TLAB context 的位置约束不参与该消除。接口不因自身类型被强制保留中间槽；按值快照、局部重赋和真实内存别名仍由原数据流表达。debug 可继续保留存储，GC 正确性不依赖优化是否开启。

全部普通 LLVM 优化和 helper 展开完成后、逻辑根定稿和 RS4GC 之前，codegen 对含 AS1 leaf 的 aggregate SSA 作晚期分解。沿真实定义捕获分量；aggregate PHI／select 拆成对应分量合流，循环回边保留各自 incoming，invoke 结果在正常边取得。没有 managed leaf 的子 aggregate 可以整体保留。原标量 extract 复用同一分量，实际 ret／store／call 等消费点才重建 aggregate，使跨 safepoint 的业务使用成为 RS4GC 可重写的标量使用。分解暴露的等价指针合流按 2.19 在根定稿前归并；此后在 RS4GC 前不再运行会合并 split／rebuild 的普通优化，RS4GC 后再发射机器码。条件 poll 的快／慢路径由标量 PHI 选择原 object 或 relocate 结果，itab 不参与搬迁。该变换不增加 root 身份、发布、对象字段或 `.slib` 数据，不对自产 IR 再建一套完整验证体系。

优化后的 AS1 根若通过 phi／select 合并 immortal 常量与动态对象，仍表示对象起点。codegen 在根准备处为不含派生地址的 managed phi／select 保留对象起点事实，并以 LLVM 22.1 的 `is_base_value` 标记传给 RS4GC。逻辑恢复标记在某条路径折叠为常量时，共享后继仍可能使用原合并值，因此该处理不只依赖恢复标记当前指向的 SSA；不能让常量分支被替换为 null base 而产生不符合当前 runtime 的 base／derived pair。纯常量选择仍不参与 relocation；typed 根恢复、base=derived 与实际 stackmap 数据生成保持。

可能收集的函数中，AS1 地址转整数在普通 LLVM 优化前保留为 codegen 自有的 NoGC 操作，不能当作跨 safepoint 不变的纯整数表达式进行 CSE 或提升。临时操作以 LLVM inaccessible memory 的读写维持顺序，不读写普通对象或栈内存，允许无关字段和局部存储继续优化。该操作仍持有实际 AS1 operand，根定稿和 RS4GC 按其真正使用位置更新引用；RS4GC 完成后才原位降低为 ptrtoint 并删除临时操作声明，不在最终产物留下调用、符号或诊断代码。识别依代码生成器创建的操作身份，不按普通 extern 名称匹配。已证明无 safepoint 的有界卡表区域可以按原依赖与必经路径规则移动该操作；普通 NoGc 函数不需要此保护。页表索引、卡偏移等派生数值不得复用搬迁前的地址位，不增加备用 roots 或 runtime ABI。

LLVM 优化、RS4GC 与对象发射使用前一步的成功结果，生产路径不在变换前后完整验证自产 module，也不另写 RS4GC 后的策略、指针活跃性和 root restoration 复核器。普通优化后保留必要的逻辑根身份与 base 合并信息，再执行 RS4GC；注册元数据的物理根数消费其实际 relocation 结果，不重新分析指针活跃性。GC 与 ABI 回归通过实际 IR、机器输出和运行测试验证，不为内部转换添加证明专用的指针边界标记。

managed 单槽写屏障按运行时规范 3.6 内联读取四级 page map 与 region prefix，再原子置脏；这段计算没有 safepoint，不保留跨 safepoint 的 region metadata。runtime ABI 数据符号 PageMap 使用 tag 32，旧单 arena CardTable 的 tag 6 退役；PageMap 的 32768-byte／8-byte 对齐数据合同和新符号进入现有 registry fingerprint。范围写屏障保持既有 NoGC 入口，由 runtime 逐映射标记完整范围。

release 的 codegen 分配结果可携带实际 TLAB 成功的布尔 SSA 事实，普通 HeapStore、typed box 的紧接 payload 复制及 typed 数组的初始元素写入据此跳过年轻目标的初始化屏障；慢分配仍执行原单槽、完整值或数组范围屏障。普通字段跟踪未取址副本，复用尾部清零的初始化别名规则；无条件边仅向单前驱且尚未发射的后继传递事实，不回填已经发射的块。重赋保留各副本快照，调用、poll、取址、对象读取、外传、条件分支、合流及未分析操作撤销该事实。该布尔值不成为 GC root，不以对象刚分配、类型名字或长期缓存代替实际快分支证明；分配后所用的 payload／字段引用仍消费原根计划恢复的值。证明与 guarded barrier 留在 codegen GC 层，不向 MIR／LIR／`.slib` 增加 nursery 策略或新 ABI；debug 保留原屏障。

同一初始化区间的尾部清零分析消费已有 exact LIR 类型、布局和真实 HeapStore，不按 GC-free 一概排除已确定覆盖的引用。直接引用按一个完整指针写入，接口按两个完整分量写入，null niche enum 复用其实际存储表示；普通 struct／tuple 递归要求字段连续且完整覆盖，不把 LLVM aggregate 的 padding 或 tagged enum 的未写部分计为覆盖。仅省去运行时规范 3.1 允许的连续尾部；原存储、屏障、root 恢复及发布位置保持。该分析在已有 codegen 分配路径内执行，不重建 scan、不加入持久摘要或另设初始化状态机，普通 LLVM 优化与晚期 aggregate 分解继续沿原顺序执行。

TLAB bump 由 codegen GC 层消费同一布局常量与运行时规范 3.1 的当前范围合同。ARM64 release 已知 8-byte 对齐且规格化尺寸为正的正规对象常量使用剩余字节比较，省去游标重复对齐和空值判断；资格及尺寸合法性仍来自现有 typed 布局与检查，其他目标和未知情形继续使用通用分支。目标选择属于现有 codegen 配置，不按函数名，不向 MIR／LIR 添加策略字段，不跨 GC 缓存游标或 limit，也不改变初始化、位图发布和 collecting slow path 的顺序。

已知 nursery 布局的 release 成功分支统一生成对象完成操作，常量大尺寸与动态大小复用同一 memset、对象头、精确大小、starts 和计数正文；不先因 128-byte 阈值调用共享 C 完成入口。清零仍由 LLVM 按目标及长度决定内联或使用共享内存实现，常量小对象的确定初始化分析保留原范围限制，动态／较大对象完整清零。debug、未知 TD 与 collecting slow path 保持，root plan 和重定位职责不变；该选择不增加 IR 策略字段、分析框架或产物／runtime ABI。

单槽和 runtime 范围屏障对实际覆盖的每个 card 字节使用 relaxed atomic store 1，并按运行时规范 3.6 登记其 64-card 组。新增 NoGc runtime 入口 `RecordDirtyCardGroup`（tag 33），完整 C 签名为 `(region metadata pointer, UInt64 card index) -> void`；只操作组链接和 heap 私有链，不读写 managed storage、不分配、不取 TLS 或 park。生成代码先表达完整登记调用，release 的既有有界循环变换将与每个 card store 对应的登记一起移到正常出口；只有这项 compiler-owned 屏障调用可通过该变换，不推广到一般 NoGc／native 调用。完成普通优化后，使用临时内部 LLVM helper 与标准 always-inliner 展开登记状态的 relaxed 读取，已登记时跳过真实 runtime 调用；临时 helper 不成为 Scoop callable 或最终机器入口。所有 profile 都生成完整卡组登记与快速检查，原精确 roots、poll 和用户内存序保持。

nursery 容量、full 压力、可选 survivor 与 minor／full 分片标记归 runtime 私有实现（运行时规范 3.1、3.9），不作为 LIR 常量、构建 profile 或 `.slib` 字段导出；现有生成代码继续只消费稳定的 allocation context 与分配慢入口。调整这些策略不重编依赖或改变精确 roots；generated card store 在 mutator 阶段仍只写 1，collector 私有的保留卡状态不进入生成代码。分片之间不移动对象、不并发扫描普通字段，复用已有引用写入屏障记录已发布 owner 的改写；minor 清除扫描卡前保留最终 remembered 回写所需位图，年轻初始化屏障省略的既有证明仍成立。各片继续通过原 safepoint 与根计划停稳／恢复，普通 ABI 消费者无需感知某次暂停是否完成整个周期。

该共享快路径沿用 runtime ABI contract 17 引入的卡组登记，当前统一 contract 为 30，metadata ABI 9 和 `.slib` 容器格式保持。旧 ABI 产物必须按现有 runtime contract 兼容性边界重建；不添加混合布局或运行时全堆扫描的兼容模式。region prefix 和卡组布局由 codegen GC 模块与 runtime 共同实现，LIR 仍只承载完整托管写入及实际 runtime 符号合同。

ARM64 默认对 ArraySet 的单引用槽屏障使用已有数组对象基址查询 mapping；按运行时规范 3.1 的 mapping 合同读取 GC 字大小位，选择 small 的 16 MiB 对齐地址或 large 的首行前缀地址，再读取 owner region。只选择地址而不预读两条路径，共用原 region prefix 与登记；实际 card 地址仍取被写入的元素引用槽；一个对象的全部存储属于同一 mapping。多引用值继续使用原范围屏障，不按元素大小推定相邻槽属于同一卡。release 可以按 2.19 提升有界内层中的不变查询；其他 profile 与未命中路径保持原位置的完整操作。x86_64 的直接查询在大数组随机写入中回退，默认保留已有页表生成路径；没有按函数名称选择优化或新增策略开关。

数组 mapping 查询在 codegen 内暂以私有、带类型身份标记的 NoGc 操作保留；输入是由当前 AS1 owner 经既有 GC 地址保护取得的整数。普通 LLVM 优化与有界循环处理期间保留其完整操作身份，2.19 的查询提升与卡合并可复用该操作；只在原有无 safepoint、无同步的已证明区域移动它。随后与卡组快路径在根计划定稿前展开，最终产物不保留该操作或额外 runtime 入口。普通 extern 即使使用同名符号也不获得该操作身份。RS4GC 后才降低其地址输入的 GC 保护，保证 mapping 读取使用当前副本。

普通 HeapStore 的局部屏障复用只保留当前 LLVM block 内一个未改变的 LIR 对象引用及其 region prefix；精确相同的字段槽可复用已完成的 mark，其他字段只复用查询。block 切换、local 写入、调用（含 NoGc）、poll、原子访问、发布和未列入无副作用允许集合的指令使缓存失效。数组／范围复制及 AtomicRef 继续使用各自原有屏障，不改变成功／失败 CAS 的标记条件，也不按静态字段距离猜测 card 身份。

每个实际物理定义都有 typed owner、role、linkage、symbol 和精确 section range。引用使用 typed target 与真实 relocation；跨对象 external 声明不能变成可缺失的 weak reference。Strong 属于原定义 Cone，ODR 使用其稳定 member identity。

对象格式与架构分别决定容器结构和机器规则：ELF 不隐含 x86-64，AArch64 不隐含 Mach-O。所选对象规则解释 machine、e_flags、relocation 写入范围／addend、stackmap/EH 指针及最终动态 relocation；仅开放当前已实现的完整 target，不增加空 handler。对象边界验证目标格式、section/symbol extent、alignment、权限、relocation 写入宽度/addend、定义与引用闭合。Mach-O 使用其真实 relocation 与 weak 属性；ELF 使用 ET_REL、实际 section/symbol/RELA/COMDAT 数据，TLS 必须保持 STT_TLS/SHF_TLS，不能伪装成普通 data。
已验证的 relocation form 明确保留机器语义；现有 ELF form 的 tag 11 表示 x86-64 RELA（kind、width、signed addend），其编码不变，不把同一数字 relocation kind 自动解释为其他架构。ELF header flags 在所选机器的对象读取边界检查一次，后续 envelope 复用该结果。

Windows 使用 AMD64 COFF relocatable（含 bigobj），对象格式 identity 为 `org.scoop-lang.object-format/coff-relocatable/1`。对象 reader 保存原 section／symbol 序号、长名称、section characteristics、aux symbol、COMDAT selection／关联 section、weak external fallback 与实际 relocation kind／写入宽度／隐式 addend；aux symbol 占用原符号表序号，但不能作为 relocation 的普通目标。COFF relocation form 使用新 tag 12（kind、width），隐式 addend 保留在原 field／canonical value，现有 Mach-O／ELF 编码不变。引用检查在读取边界完成，后续发射与链接复用结果。PE import-library 的短 import member 不伪装成 relocatable 对象；它由 native library 输入选择处理。三种 Windows 完整 target 独立保存身份，普通 COFF 本身没有可证明 CRT 的标签，reader 不从文件名或 object header 推测 CRT。

COFF section inventory 将 `.pdata`／`.pdata$…` 记为 Windows unwind function table，将 `.xdata`／`.xdata$…` 记为 Windows unwind information，section-role 编码分别为新增 tag 14、15；两者不冒充 ELF `.eh_frame` 或独立 LSDA。`.llvm_stackmaps` 使用已有 stackmap role 和真实 ADDR64 function relocation；`.tls`／`.tls$…` 按 section characteristics 区分有初始化／零填充 TLS。其余实际加载节按 code／writable／zero-fill 属性分类，CodeView、linker directives 与被标记移除的节属于 object metadata。现有 section-role 编码保持不变。

ELF／COFF 机器发射在同一份未变化的 Cone symbol surface 上建立一次名称索引，函数模块只处理本模块出现的定义／别名，不为每个函数重复重建全 Cone 的符号拼写与映射。finalization 改变 surface 后，metadata 模块使用更新后的索引；不改变既有 linkage、COMDAT 或 atom 归属规则。

MinGW 后端为全局数据地址生成的 `.rdata$.refptr.*` 指针，与普通 `.rdata` 常量池一起归入所属 callable 的 relocatable constant atom；保留指针 relocation，将 helper 符号本地化并跟随 callable 的 COMDAT 选择，不作为新的公共定义发布。

Windows callable 的 definition atom 新增 unwind table／unwind information 两种 role（编码 10／11）；`.xdata` 内的 unwind header／handler RVA 与后续 LSDA 使用各自不重叠的真实边界。代码、stackmap、SEH、callable／safepoint registration 与 callable 私有常量按 COFF COMDAT association 同步保留／裁剪；Strong 不转为可任意选取的 ODR 定义。已有目标的 atom role 编码不变。Windows personality 的对象符号为 `scoop_eh_personality_seh`，通过同一 target EH requirement 投影选择，普通 Unix personality 符号保持原样。

COFF atom 物化完成后，若关联节先于其所属 COMDAT，则按实际 association 将所属节排在关联节之前，以满足 Microsoft LINK 的输入约束。只调整 section header 顺序与符号／association 中的 section 序号；原 symbol table 序号、relocation 索引、节内容及其文件偏移保持，不重新发射机器代码。已经满足顺序的对象不调整。普通 COFF 与 bigobj 使用各自的 section number 字段宽度，真实关联环或越界按格式错误报告；产物 reader 仍保存最终对象中的实际序号。

LLVM 在同一 callable 对象中产生的多个 `.rdata` 常量池合并为一个私有 contribution，保留各常量的实际对齐、内容和 relocation，并作为该 callable 的常量 atom 关联保留。合并选择原 symbol table 中最先定义的常量节作为主 contribution，并按这些节的 definition symbol 顺序拼接；主节定义必须先于重定向到它的原节符号，避免 COFF linker 将尚未关联的符号视为已丢弃。LLVM 的可跨对象合并的常量符号在此成为局部符号；不得因此合并 callable identity。显式命名的 `.rdata$scoop.*` 元数据仍按其原 definition plan 独立拥有边界。常量合并保留原 section header 槽，随后按前述 COMDAT 规则排列；已有 symbol 序号和完整 target identity 不变。

ELF atom 边界归一化按每个实际对象建立一次原始符号名到符号表序号的查询索引，借用原字符串表。边界、可见性和 COMDAT 操作复用该索引，不为每个 atom 重扫整个符号表。索引表示原对象，新增或改名的符号仍由既有编辑记录管理；原 symbol／section 序号、relocation、弱定义规则与输出顺序保持，不增加 native 符号登记或解析职责。

relocation 的 section、atom、offset 和 target 均属于本次实际对象。raw source-native symbol 按 Darwin 添加一个前导下划线、ELF 保持 bytes 的规则正规化。Scoop-owned symbol 从 persistent identity 产生，不用 FQN 或 source 短名回退。

所有 Scoop body、NoGc body、managed adapter、root/init gateway 与 release hook 都有 callable registration。生成 C bridge/trampoline、runtime C 和 ordinary native producer 使用各自实际对象合同。实际保留的 safepoint 与其 owner、role、machine root count、stackmap 完整对应；无 safepoint 的 body 也可合法登记。

runtime 按运行时规范 2.8 在首条 stackmap 合并时一次完成 site 的 owner 与 frame 验证，后续相同 ODR 记录复用结果；该启动优化不改变登记记录、`.slib`、ABI 或各 profile 的生产合同。

body 的 EH、LSDA、unwind、stackmap、关联常量与私有 Context cells 属于该 body 的同一次物理选择。TD、scan、dispatch、static storage、初始化和六类 registration 满足运行时规范；release hook 为 raw C、nounwind、NoGC，无 managed transition 或 EH/safepoint。

LLVM 在机器发射中由 switch 生成的引用查找表和跳转表同样属于该 callable 的关联常量。AddressTakenConstant／Singleton 描述需要装载期地址重定位的后端常量；ELF 的 .rodata 贡献另用 AddressTakenConstant／BackendReadOnlyData 描述，包含指向本函数代码的相对跳转表。BackendReadOnlyData 是 DefinitionAtomSubkey 的无 payload 变体（wire tag 10），不改变既有子键的编码。两个贡献均在机器发射后按实际对象保留或移除，表内引用沿用普通引用闭合及 fingerprint。ELF 同时识别普通段名及带当前 callable 实际符号后缀的 COMDAT 段名，不把其他函数、字符串或固定宽度 literal 段归入该贡献。Mach-O 的地址表放入只读 __DATA_CONST 段；ELF 分别使用 .data.rel.ro.scoop.constants 与 .rodata.scoop.constants，保留各自权限，并与 callable 一同进行 ODR 选择。它们不获得源码实体身份，不通过忽略 orphan relocation 或关闭 switch 优化绕过对象合同；固定宽度、无引用的数值 literal pool 保持原有按内容表示。

ELF 的非合并 `.rodata`（ODR body 为 `.rodata.<primary symbol>`）可以与已有 atom 共享。后端表位于该段前缀，范围止于首个已有 atom 的起点（没有已有 atom 时止于段尾）；后续已有定义保持各自范围，空前缀不产生后端 atom。该前缀沿用 BackendReadOnlyData 子键，与 Singleton 引用表可以同时存在；`.rodata.cst4/8/16` 的数值 literal pool 保持原路径。

已有 CallableCStringIdentity 的 C 字符串直接发射到所选目标的 C 字符串段（Mach-O 的 __TEXT,__cstring 或 ELF 的 .rodata.str1.1），沿用自身的 callable 归属与起止边界，不重复计入后端 .rodata 贡献。该规则同时适用于普通 callable 和 ODR callable；不把任意 byte array 当作 C 字符串。

编译器生成的失败消息（包括 foreign callback 状态解码失败）由 LIR lowering 建立 callable 所属的 CString global，相关指令显式引用它；codegen 沿用该 global 发射调用，不另建缺少 atom 归属的匿名字符串。

Mach-O 的数值跳转表及查找表可以与已有边界的字符串等常量共享 __TEXT,__const，位置可在段首、已有 atom 之间或之后。按代码实际引用的局部符号确定每张无归属表的起点，范围止于下一个被引用的局部表、已有 atom 起点或段尾；已有 atom 内的符号仍按该 atom 处理。表内不含 relocation，其实际 bytes 参与 callable 内容指纹，不把同段的字符串、相邻表或其 relocation 一并归入当前表。局部符号名与物理位置不获得新的语义身份，也不因表不在段首而拒绝正常 LLVM 输出。

C extern 按 2.4 的 DirectC/StorageBridge 计划发射。DirectC 使用真实 native symbol 和完整目标 C ABI，不生成或经过 outbound storage bridge；对象中的调用 relocation 直接表达该 native 引用。只有实际采用 StorageBridge 的调用才要求相应 generated-C unit；callback、native global/TLS 等其他桥接用途照常保留。两种路径均保留相同的 native contract 与 library requirement，不因省去 bridge 丢失链接需求。

Windows 的非 TLS native global 存取桥接通过 `__declspec(dllimport)` 发射实际 `__imp_` IAT 引用，读、写和 addressOf 使用同一地址。源码契约仍以原始 native symbol、类型和 library binding 表达；对象引用分类与最终 provider 选择将对应 IAT 引用关联到该数据契约。最终 provider 为普通对象／archive 时，由 PE linker 生成常规 local-import 指针；为 import library 时使用 DLL 的 IAT。同一份 `.slib` 可以按普通 TargetDefault 库选择消费两者，不复制 DLL 数据。此路线上有意产生的 local import 使用目标 linker 的 `/ignore:4217`（MinGW LLD 经 `--Xlink` 传递）关闭该项告警，其他诊断保持。TLS 保留原来的 native TLS 声明和符号种类检查，不对 TLS 施加 dllimport。

generated-C bridge 保留实际声明的 C 类型、签名、目标工具链与编译选项；bridge 具有精确 primary/signature/context 定义范围，不能由邻近符号猜测。DirectC 与 StorageBridge 均不得按 extern 名称被 builtin 替换。普通 C ABI 与 Scoop NativeBorrowed 调用不混用 storage bridge。标量直接调用不依赖 bridge 内联、跨对象 LTO 或相容的 Clang bitcode；现有 generated-C 独立编译流程继续适用于保留的 bridge。

errno 捕获 bridge 使用所选 target 的 `<errno.h>`。全部参数解包及必要准备完成后，先执行 `errno = 0`，再调用真实 C 函数；紧接着以独立语句将 errno 复制到 native 局部整数，然后才复制 `R`、执行其他 helper 或返回捕获值。当前 profile 的 C `int` 为 32 位，可精确映射为 Scoop `Int`。非 void `R` 通过既有 C storage 写回，bridge 返回 `int32_t`；void 不产生虚假的 C 返回对象。目标 libc 的 errno 访问所产生的 native 引用进入既有 generated-C 对象与链接需求，不能使用宿主 accessor 或新增 Scoop runtime 槽位 API。

NativeGcLeaf 在两种物理路径上都不发射 enter/leave native、RETURNING、statepoint、caller-root push/pop 或专用于该调用的 relocation reload；调用前后表达式的真实 safepoint 不受影响。NativeSafe 可复用 DirectC，但仍保留自身的线程协调与 caller roots。DirectC 的 GCLeaf 调用不增加桥接函数调用或由桥接协议导致的参数/结果内存往返；不承诺消除 ABI 必需的搬运、正常寄存器 spill、动态链接跳板或显式的 errno、pin/unpin 操作成本。`captureErrno = true` 在 M33 使用带立即捕获的 StorageBridge，不属于无桥接开销的范围。

GCLeaf 契约不意味着 pure、readnone、readonly 或 nosync，不能据此删除外部副作用或推导用户数据同步属性。代码中的调用模式必须与所属 Scoop 声明一致，native symbol 去重不改变已选择的模式。直接调用的 ABI 通过与目标 C compiler 的调用交互验证；验收同时检查优化后的机器码，确认 DirectC 没有新增 bridge 调用或桥接缓冲区，GCLeaf 没有 GC 边界操作，不能只凭 LIR 标记或微基准耗时判定。

对象内容 fingerprints 使用实际定义 bytes 和 canonical relocation target；物理地址、archive member 分组或 section ordinal 不得冒充语义身份。内容摘要只保存在实际需要它们的产物 metadata 中，不写入 runtime descriptor。runtime 不消费的指纹字段、零槽、patch intent、物理槽位查询及回填阶段全部删除；LLVM 直接发射完整的 runtime record，符号地址由普通 relocation 和最终 linker 绑定。

摘要依赖必须无环，节点仅依赖实际内容所需的上游摘要。Code、RuntimeImage 与 Artifact 分别覆盖自己的内容，不能建立自引用。产物的 RuntimeImage 摘要在生产时计算一次；最终 linker 只按选中的定义建立 image 指针表，不为不可见的 runtime 字段重新编码摘要。

对象中的实际 stackmap 与 LSDA 遵守运行时规范 2.8、5.2。LLVM 机器发射成功后直接使用产出，不执行自建的机器指令、frame chain、stackmap 或 LSDA 全面验证；只读取后续地址、GC 数据和产物内容计算实际需要的字段，并复用结果。stackmap 按实际 function relocation 关联 callable，以符号地址加 LLVM instruction offset 取得返回地址，读取并归一化实际根记录；不再次反汇编检查 call/frame chain，也不重建预期 member/function/site 全集做覆盖证明。优化后的 invoke 是否仍存在决定 EH atom 是否保留，不为此重建或比对独立 EH manifest；删除 final/provisional 对象复制、指纹清零后重填及最终字节比较等消费 replay。仅为验证服务的边界和索引删除；仍用于 codegen、重定位、GC 或实际链接选择的索引在同一份 surface 上建立一次。

发布直接接收本次 codegen 与 generated-C 的实际对象、成员分配及生成记录。C bridge 的编译器 profile、源码模板、flags 和 unit 集合不再与自行构造的生产记录重复比对；对象中的 SDK／工具版本也不作为来源证明。所需 section 与符号正常读取，Code 指纹继续消费实际生成记录。删除无生产调用的旧 single-Cone 分级资格流水线；不以另一套 `Verified` 包装保留这些入口。当前完整产物的投影直接采用已有生产计划与实际对象，不深比较它们的副本来证明同源。

同一次对象生产中的定义、stackmap 与 registration 处理共享最初解析的 section、符号及 relocation 信息，直接借用原对象字节；不在每个步骤重新计算整包 SHA-256 来证明字节未变化。registration 直接关联 typed 计划与已有的 definition/member 分配，复用规范化 stackmap 和 canonical ABI；不另查符号表定位内容摘要。body 指纹按已解析的 primary／associated atom、实际 relocation 和该 body 的 stackmap 输入计算；runtime scan 同属 associated atom，不从计划重新生成 scan 字节或遍历闭合证明。runtime 解析实际 stackmap 建立 GC 索引，重复 ODR 记录按实际 PC 与 payload 合并。image 与 root entry 直接使用生成计划；Library／Executable 分支由同一完整枚举表示。归档保留原始对象，不为内容摘要复制、清零、回填或重新解析对象。首次对象格式检查、实际 definition/body 指纹及运行时记录编码继续保留。

immortal object 的登记身份与 ODR ABI 直接消费现有登记计划及 canonical shape ABI；RuntimeImage 编码同一计划中的对象大小、对齐与 type registration 引用。不为了取得发布资格再次读取 String payload、重建登记记录、核对指针目标或比较 digest 依赖图。实际对象生成、普通符号引用与 ODR 选择继续使用既有数据。

静态存储、初始化和 type registration 直接使用已有计划及 definition/member 分配。scan／layout、实际 descriptor 和 gateway body 的内容摘要只供产物计算使用，复用已有 canonical shape ABI、atom 范围、typed 引用和 callable 结果。发布不重读初值、cell、诊断文字、登记指针表，不重新比较生产副本或建立物理验证资格链。实际读取仍检查范围和缺失引用。

shape 发射按 typed owner 与 role 查询同一 symbol surface 在构建时已建立的主符号到 definition 映射。唯一性由该表的构建边界检查，后续查询不再次遍历完整 definition 集合；裁剪实际 definition 时同步移除对应索引。查询继续报告缺失引用，计划顺序与产物编码不变。

Code 指纹直接编码已有的定义、undefined requirements、native contracts、imports 与最终成员目录。计算和归档组装不重建这些集合来比较副本，也不再次比对所有组件的 producer／target 或复制的 relocation 闭包。已有结果直接进入 metadata 和内容摘要；当前编码、范围读取及真实链接引用错误正常处理。

Scoop LIR 与 generated-C 的 external requirements 保留实际对象中的未定义符号、重定位和所需链接库。Scoop 的 current／dependency／ODR 身份、源码 FFI 合同及 runtime／EH 引用按各自已有规则分类，其余普通 native 符号直接交给 program-link，不要求存在 Scoop extern 声明或 target-helper 登记项。不重建 definition／atom 集合、扫描必需调用覆盖或证明机器 relocation 与模板相同；没有对应定义或导入的 Scoop mangled 符号仍是引用错误。

普通 native undefined requirement 使用独立 Native 变体（wire tag 10），保存实际符号字节；对象正文的 relocation 编码使用 tag 16 与符号 bytes，内容指纹同时包含该名字及真实 relocation／addend。退役 CBridgeTargetSupport requirement tag 7 及其逐符号合同 registry，不复用旧 tag。LIR link-identity-closure 升至 17，编译器与开发 sysroot 一起重建；不改变源码实体、runtime ABI 或原生符号的强弱属性。

undefined requirements 完成普通分类后直接合并、排序，保留 callable／shape 的实际依赖分区。finalizer 不重建全部 relocation uses 做 coverage／重叠证明，也不比较 current／external／shape 闭包副本；查找真实 import 时保留索引边界错误。完成分类的外部需求使用同一数据类型，不再套封口资格包装。

物理 definition 读取只查询实际需要的符号和 atom 边界，建立供地址、relocation 和内容指纹使用的索引。保留缺失符号、歧义定义和本次读取范围的诊断；不比较所有生成符号与计划的全集、重验既定 linkage／primary 位置，也不扫描未归属 section 或 padding 来证明零值及生产覆盖。padding 不属于 definition 的语义内容，不再保存无消费者的 padding-end 资格字段。

Runtime metadata 发射直接使用完整 LIR 计划及当前 LLVM module 中的声明，按实际引用查询符号并生成完整记录、scan 和初始化数据。不比较 global／function 的类型、linkage、初值或 producer 副本来证明已完成的发射。对象物化仅处理实际 codegen、relocation、GC 或链接选择需要的边界与 linkage，不读取对象为无人消费的字段回填指纹。已由 LLVM 发射的 global alias 不重放边界计划；generated-C 使用真实主符号及其 extent，不复查 manifest 或 C 模板。


Mach-O／ELF 对象按实际目标格式读取 section、符号及 relocation，按标志与本次使用的特殊段分类；Scoop LIR 与 generated-C 使用同一规则，不设来源专属段名／flags 白名单或必须含 text 的资格。版本、优化提示和其他未消费的 load command、section group 不另行审计或保存证明副本；不比较整张物理表的覆盖、分区、padding 与重叠。格式库负责读取所需表，实际地址／索引／内容访问保留局部边界错误。 符号属性按实际位读取，不以描述标志组合或未消费的符号类型拒绝整个对象；relocation 仅解析实际使用的 symbol index，不要求 undefined 符号表每项均有使用，也不以重复名称预审替代本次引用解析。


### 2.6 `.slib` 格式与指纹

`.slib` 是 target-specific、bitwise reproducible 的 normal、self-contained ar archive。容器 schema 与 manifest schema 的格式版本为 2，mangling schema 为 1；persistent identity schema 为 2，HIR/MIR/LIR metadata outer schema 为 3。函数声明 key 增加规范化条件集合及 companion 存在条件，签名类型增加固定 companion 投影，binder 增加负向接口集合与独立 companion 条件；默认表达式保存依赖 companion 值访问。空集合在 wire 中省略对应字段，解码后仍是完整集合，无条件声明保持原身份。旧产物按格式版本拒绝，重建得到新产物。它不允许 thin archive、外部成员引用或相邻文件 payload。

归档 magic 为 `!<arch>\n`，首项固定为 `manifest.zst`，内容为 deterministic CBOR manifest 的单个 Zstandard frame。其余 payload 按 manifest 中的 SlibMemberId bytes 严格排序，物理短名为从 0 开始的 ordinal：`m` 加 8 位零填充十进制。header 使用 SysV/GNU short-name encoding：名称后为 `/` 与空格填充，mtime/uid/gid 为十进制 0，mode 为八进制 `100644`，size 为无多余前导零的十进制，trailer 为 `` `\n``，奇数 payload 后补 `0x0A`。

writer 按上述目录与 header 规则生成归档。普通 reader 根据目录按需定位 section，不重读全部成员来验证归档生成过程或对照内容 hash；payload 不因自身 magic 而递归作为归档解释。实际解码与 I/O 错误照常传播，长度转换和索引保持内存安全。

每个非 manifest 成员为五字段 record：`1=id, 2=stable_key, 3=role, 4=byte_length, 5=sha256`。SlibMemberId 来自 ConeIdentity 与稳定非路径 member key，物理名、扩展名和生成顺序不参与身份。

| member role / stable-key tag | 值 | 用途 |
| --- | --- | --- |
| HirMetadata | 1 | Compile |
| MirMetadata | 2 | Compile |
| LirMetadata | 3 | Compile、Link |
| LinkObject | 4 | Link |
| DiagnosticAttachment | 5 | Diagnostics |
| ExtensionBlob | 6 | 显式声明的 purpose |

三种 metadata 各恰有一个成员；完整生产产物具有非空 LinkObject 集合。role 与 stable-key tag 必须匹配，成员由目录身份识别，不由物理名、ordinal 或数量推断 producer。

Manifest、三层 metadata 和 DiagnosticAttachment 各自使用单个、无字典且带原始长度的 Zstandard frame；writer 统一使用标准 compression level `3`，不设置压缩开关或按压缩率选择编码。LinkObject 和 ExtensionBlob 保存原始 bytes。成员 record 的 byte_length 与 sha256 描述实际存储 bytes，ar header 的 size 也使用存储长度；成员身份和三层语义指纹仍由既有逻辑内容确定。依赖发现只解压 manifest；成员首次实际使用时解压并缓存结果，后续请求复用该缓冲区。frame 解码只承担长度、截断和格式等局部正确性检查，不复核成员摘要或重放语义。此前 level `-3` 生成的 frame 仍是相同格式的合法输入；压缩参数调整不递增容器／metadata schema，不要求重建未变化的预编译库。

容器 1、manifest 1 和旧 profile 不提供兼容读取，必须重建。压缩库是编译器宿主依赖，与 sysroot 中面向目标平台的 Zstandard 库分开。方案、测量依据和最小验证范围见 [存储精简与压缩方案](../slib-storage/DESIGN.md)。

所有 wire metadata 使用 RFC 8949 deterministic CBOR 的明确 schema：最短整数与长度编码、定长容器、严格有序且无重复的整数 field tags、声明的封闭 variant、完整 typed 引用。不能序列化 Rust discriminant、arena index 或 hash-map 遍历顺序。消费者按所需 schema 解码，传播解码库自然返回的错误，不增加 canonical 重编码比较、生产语义 replay 或损坏检测合同。

解码入口只要求解码能力，成功消费当前记录后直接返回。最短编码、字段顺序、截断与尾随输入由当前字段或记录的读取处理；不分配第二份完整编码来比较，也不为这种比较设置输出长度额度。

```text
ByteSpan(bytes) = little_endian_u64(length(bytes)) || bytes
H(domain, value) = SHA-256(ByteSpan(ASCII(domain)) || WireCborV1(value))
```

domain 无结尾 NUL；可变长度 raw 片段使用 ByteSpan，只有公式明确声明的定长 ID/digest 可以 raw 拼接。运行时 metadata 的 RuntimeEncode 是独立编码，不能用 WireCborV1 替代；callable body identity 使用运行时规范 2.8 的 v2 domain。

`ConeIdentity = H("scoop-cone-id-v1", ConeCoordinate)`。ConeCoordinate 使用语言规范 12.2 的 canonical group、name 与 version；manifest 同时保存 coordinate record 和 identity，二者必须一致。长度转换 checked 到 u64；WireCborV1 item 不隐式再套 ByteSpan，只有明确写出的外层 framing 才参与指纹。

CapabilityId 为 map `{1=namespace, 2=name, 3=nonzero u32 major}`；purpose bits 为 Graph=1、Compile=2、Link=4、Diagnostics=8。capability 表达格式和消费能力，不授予来源资格。LinkObject 明确指定 target、object format 和 verifier capability。内建对象格式为 `org.scoop-lang.object-format/mach-o-relocatable/1` 与 `org.scoop-lang.object-format/elf-relocatable/1`；Scoop verifier 为 `org.scoop-lang.link-object/scoop-lir/5`，generated C 为 `org.scoop-lang.link-object/generated-c-bridge/1`。

Scoop LinkObject logical key 为 `{1=nonzero unit_count, 2=unit_set_digest}`；digest 为 `H("scoop-lir-object-unit-set-v1", sorted unique nonempty ObjectDefinitionPlanId array)`。generated-C 使用同一 map，domain 为 `scoop-generated-bridge-object-unit-set-v1`，元素为 GeneratedBridgeUnitId。物理分组只改变 Code/Artifact 层。

GeneratedBridgeUnitId 的 domain 为 `scoop-generated-bridge-unit-v1`。key 为 OutboundFunction=1（native contract fingerprint 与普通/errno 捕获的结果适配）、GlobalRead=2、GlobalWrite=3、GlobalAddress=4（各含 native contract fingerprint）、CallbackTrampoline=5（C signature fingerprint 与 context parameter index）、StaticCallbackTrampoline=6（storage-bridge generated callable ID 与 C signature fingerprint）。OutboundFunction 的结果适配确定 bridge 的私有返回方式；同一 native symbol 的捕获与不捕获 recipe 不能共用 unit identity。NativeSafe/GcLeaf 不改变 bridge 本身的结果适配，仍可复用相同 recipe。

bridge unit 是与 producer 无关的 recipe identity；实际 atom 使用 producer ConeIdentity 与 PrimaryEntry、SignatureDescriptor、ContextDescriptor 或 StaticAssertSupport role，domain 为 `scoop-generated-bridge-atom-v1`。前三者可具有实际 bytes/range；StaticAssertSupport 不产生物理定义。unit、实际 atom 和 member 的关系必须完整，不能把 producer-specific symbol 写入 unit identity。

未知 optional capability 按目录跳过，不读取或计算其内容 hash，不进入语义 view。ExtensionBlob.required_for 只允许 0 或 Link bit；未知 Link-required capability 或 LinkObject verifier 阻止 Link 消费。blob 不成为第二种对象入口，已知 Link contribution 只能表示规范化 native library requirements，不能注入 raw linker argv、脚本或未检查对象。

完整生产 profile 为 `org.scoop-lang.slib-profile/cross-cone-generic/6`。descriptor 是五字段 map：`1=id, 2=required_manifest, 3=required_hir, 4=required_mir, 5=required_lir`；各列表按 capability 排序。其必需 section 为：

早期 `single-cone-strong` 与 `cross-cone-semantics-strong` profile 不再是生产或消费入口，不随当前 section 版本维护正向 descriptor／fingerprint 矩阵。旧 profile 按不支持的产物 profile 报错，当前 profile 的 required inventory 和版本兼容检查保留。

| 位置 / namespace | section 与 major |
| --- | --- |
| Manifest / `org.scoop-lang.manifest` | `single-cone-production/9` |
| HIR / `org.scoop-lang.hir` | `identity-foundation/10`、`core-bootstrap-interface/17`、`cross-cone-interface/87`、`cross-cone-type-semantics/31` |
| MIR / `org.scoop-lang.mir` | `identity-foundation/8`、`core-bootstrap-bridge/2`、`cross-cone-param-free-bridge/2`、`cross-cone-type-bridge/20` |
| LIR / `org.scoop-lang.lir` | `identity-foundation/11`、`cross-cone-param-free-bridge/3`、`cross-cone-layout-abi/18`、`cone-production/15`、`link-identity-closure/19`、`link-support/1` |

本次合并采用 HIR `cross-cone-interface/87`，同时保存编译期输入与调试信息需要的源码接口。callable intrinsic tag 30 保留 main 的 `ThrowablePrintStackTrace`；`CompileEnv` 使用 tag 34，`IncludeStr`／`IncludeBytes`／`IncludeDir` 保持 31／32／33。旧分支与 main 的不兼容 profile 均按通常版本规则重建；不协商或混用两份同号接口。

各 section 按消费用途检查 required inventory。Compile 需要完整语言与相邻 IR 合同；Link 只消费 identity、ABI、对象、native、production 和链接支持数据，不为链接展开 HIR 模板。profile fingerprint 覆盖 descriptor 的实际内容。

Compile 在当前源码解析成功后读取依赖的 HIR foundation、声明、core protocol 和 alias 数据；名称查找不要求预先解码 MIR／LIR。HIR 完成后，以当前输出中的实际外部引用、已提交的可执行 callable 用途和 core protocol 为起点，按 provider 选择机器阶段所需的依赖闭包，再读取这些 provider 的 MIR／LIR 与物理定义记录。可执行用途复用 HIR 已收集的调用、函数引用、回调地址与分派记录，不能只以导出接口的引用表代替本地执行需求。声明可见性仍由完整 direct／support 图决定，当前产物仍保存原 manifest 的 direct edges；未使用的依赖不因此成为机器导入。已经读取的 HIR 数据与解析后的 identity key 在后续阶段共享，不重新解码或重放 HIR 检查；机器阶段只追加实际消费的后续层。源码解析失败不加载依赖的语义 payload。正常编译和链接复用普通文件入口，缺失输入及实际读取的格式、引用和 ABI 错误继续报告。

Compile 的依赖读取直接采用保存的 defined symbol owners、ODR member ABI 目录和产物摘要；typed 引用解析到同一份已读 identity 数据。跨产物同名符号归属、ODR key 和 ABI 冲突仍在本次实际合并中处理。为下游编译取得这些数据不读取机器对象、不生成 final／provisional 副本、不重建注册记录或重算 Code／RuntimeImage 摘要，也不要求消费方提供 C bridge 工具配置。已编译产物的对象生成与注册内容由生产阶段负责，摘要统一使用 manifest 中后续实际消费的字段，不另建 Compile／Link 重复摘要，也不为无消费方的对象数量扫描目录。测试不再以改写这些字节后被 Compile 入口拒绝作为验收。

依赖文件通过同一普通 artifact 入口保持文件句柄、一次解析的 manifest 和成员目录。目录给出成员偏移；仅在请求某个 metadata／对象成员时读取该成员，已读成员复用同一份缓冲区。内存产物与文件产物使用同一目录偏移与成员头解析规则，不为未使用成员扫描内容或逐项复核归档。Compile 加载实际所需的 HIR／MIR／LIR metadata，不读机器对象；文件打开、seek、read 的实际错误正常传播，不比较读取前后的文件身份、长度或时间，也不要求文件被替换或删除后继续成功。

普通构建后的 ProgramLink、显式 `scoop link` 和独立链接器都使用该文件入口。parent 的构建结果只保留路径与摘要，在真正链接时首次打开文件；显式链接的发现与成员消费共享文件句柄和已解析 manifest，不捕获整包输入快照或重算内容摘要。同一 Cone 的重复候选按已有 Artifact 指纹区分结果，不比较整包字节；本次依赖、符号和 ODR 选择继续使用实际数据。

manifest 保存 canonical coordinate、ConeIdentity、kind/source form、完整 exact direct dependencies 与三层语义指纹、language/runtime/identity/mangling ABI、target/backend 配置及其兼容摘要、全部成员目录、三层/Code/RuntimeImage/Artifact fingerprints 和 required sections。

成员内容、三层语义与 Artifact 摘要在生产这些结果时计算并保存。普通 reader 直接读取已保存的值；整包字节容器不计算另一份摘要，目录读取不逐成员哈希，Compile／Link section 解码也不重新计算三层语义摘要或重新编码 manifest 以复核 Artifact 值。格式解析、长度／范围、typed 引用与所选 target／ABI 配置检查仍在实际使用边界完成。该调整不改变 wire 字段、生产端摘要公式或正常输入变化的失效规则，也不新增另一个带复核的 reader。

依赖发现只读取归档头和首个 manifest 成员，取得 Cone、依赖、构建摘要与测试目录，不扫描后续成员头或读取其内容。字节输入与文件输入共用该摘要读取逻辑。准备 prebuilt 输入直接传递发现时取得的摘要和实际路径，不再次捕获完整归档、重算摘要或比较发现前后状态；实际编译／链接需要成员时再读取，普通 I/O 错误照常传播。

显式 artifact、artifact 搜索目录及默认 core 产物的发现共用 `artifact-summaries-v2` 普通摘要记录。按选中实际路径、mtime、大小和所选 target／backend 配置定位；未变化时直接读取已保存的摘要，不打开原归档或重新读取 manifest。路径／输入 metadata 或配置变化后按上述最小读取取得新摘要；普通锁协调同键首次读取，成功后原子发布记录。缺失实际输入仍报告输入错误，记录读取失败正常传播，不扫描或恢复损坏记录。该记录不改变语言身份、语义摘要或源码依赖的失效规则。

production 与 Link metadata 保存消费方实际使用的 body/type/site ID、Strong/ODR 定义、shared ABI、object/bridge unit 到 member/range 的映射、registration identities、静态存储与初始化单元、image/root entry 归属、defined symbol owners、未定义符号使用以及 native contracts 与逻辑 library requirements。只用于 codegen 的对象定义计划、digest finalization、shape/bridge 生产计划、重复的 callable/shape ABI 和 runtime registration 生产副本保留在生产阶段内存中，不写入 production section。完整 runtime/GC 记录仍正常发射到机器对象。

`cone-production/15` 仅保存字段 2 canonical definitions、5 registration production、6 image plan、7 entry plan。共用该生产投影的 `strong-production` section 编码为 25；旧 Strong profile 仍不作为消费入口。registration production 仅保存字段 1 identities、6 static storages、7 initialization units；image plan 仅保存字段 4 symbol、6 support atoms，support atoms 仅保存字段 11 array-bounds message 和 12 array-size-overflow message。其余生产字段没有下游消费者，当前格式不编码也不解码这些副本。

`link-identity-closure/19` 的字段 5 保存按 `(source member, symbol bytes)` 排序、去重的全部跨物理 member 符号使用，每项仅有字段 1 member 和 2 symbol。生产时直接投影已经解析的实际 relocation，包含同 Cone、跨 Cone callable／shape、ODR、generated-C、runtime 和 native 引用；不能只投影 legacy undefined requirement 子集而遗漏跨 Cone 使用。对象内部已经闭合的引用不重复保存。18 及以前版本的使用表不满足完整引用合同，最终链接需要重建这些输入，不从机器对象补扫。完整 relocation 和 requirement 继续用于生产阶段的对象处理与 Code 指纹，不作为链接器不消费的副本落盘。defined symbol owner 中的 definition boundary 记录仅保存字段 1 member 和 3 owner，省略可由 atom ID、Start/End 和 target 原生符号规范化确定的字段 2 名称；普通符号继续保存三个字段及其实际名称。所有表具有明确排序与唯一性；不保存 producer 的绝对搜索路径。该修正不改变机器对象、源码实体或 runtime ABI。

executable entry 保存完整签名以及封闭的声明分支：生产入口引用源码 PersistentFunctionId，测试入口引用 PersistentGeneratedCallableId。测试入口的 generated key 由当前 ConeIdentity 与 TestEntry role 确定，不生成伪造的源码 main identity；两者均通过正常 Strong callable body、root gateway 和 failure-root 链接。HIR 完成入口选择并检查签名，后续 stage 传递实际声明、body 与签名，不重新按名称选择入口。

前端以独立的 TestEntry 函数身份合成测试入口，正文为普通非泛型、非 suspend 的 Scoop HIR。该函数可以作为测试初始化与方法调用适配的词法根；各适配保留所对应测试声明的源码文件与位置。具体化将入口及其闭包按既有 generated callable materialization 处理，不把生成函数加入源码函数、成员分派或生产 main 的候选集合。

生成的测试入口向 scoop.test.execution.isTestSelected 传递现有目录的完整显示名，支持语言规范 16.5 的独立程序名称过滤；用例身份、排序与 scoop 命令的目录索引参数保持不变。

M43 的 TestEntry generated key 使用 tag 17，HIR／MIR `identity-foundation` 分别升至 9／7；入口声明改为 Source／Generated 封闭分支后，HIR `core-bootstrap-interface` 升至 15、MIR `core-bootstrap-bridge` 升至 2、LIR `cone-production` 升至 12，仍使用 Strong profile 的 `strong-production` 升至 22。旧 section 需重建；其余未改变编码的 section 与 runtime ABI 保持原版本。

测试标记按语言规范 16.3 在前端使用已绑定的 annotation identity 和源码角色检查。Target、重复应用与参数复用普通注解规则；容器、方法角色、签名及同类 hook 数量由当前 Cone 的声明关系决定，不依赖名称或运行期扫描。后续 stage 消费已检查的调用与入口，不重做测试声明语义检查。

Export HIR 的当前目标测试目录按语言规范 16.4 保留 typed class／constructor／function 引用、声明位置和忽略原因，不作为依赖的测试发现接口。构造重载选择与普通调用共享同一候选决议；默认实参的代码生成在入口合成的实际调用处完成，目录发现本身不创建可执行的初始化副作用。

测试用途产物在可选 manifest section `org.scoop-lang.manifest/test-directory/1` 中保存该目录的展示投影：容器限定名、用例方法名与完整显示名、各自的 Cone-relative 源码路径及一基行列、忽略原因。目录按前端顺序嵌套保存容器与用例，不通过解析显示名恢复关系；可执行调用仍使用 HIR 中的 typed 引用。该 section 不参与 Compile／Link 的语义要求，受普通 ArtifactFingerprint 覆盖；生产产物不写入，测试产物即使无容器也写入空目录。manifest summary 在读取边界解码并复用该投影，缓存命中不重新发现测试或加载完整 HIR。

入口的四种 ABI 形态、完整签名、root gateway 的实际 C ABI 与定义指纹随各层 typed entry 保留，并进入对应语义／ABI fingerprints 和承载它们的 section profile。Link 投影保留精确 entry、gateway 与 failure-root 引用，使 artifact-only 链接无需源码或名称推断。当前 runtime ABI contract 30、metadata ABI 9 同时进入兼容检查、runtime 与启动对象的构建输入；旧 ABI 的薄接口、物理签名、poll／card／初始化发布／Context 槽合同及产物必须重建或拒绝，不能用缺省字段或 bitcast 混用。contract 20 同时退役无生产消费者的标量格式化、hash、equals 及专用打印 C 入口；沿用现有 runtime 输入更新与 ABI 重建边界，不增加兼容分支。新 ABI 的 debug／release 仍互通；各批次的实际实现与 section 版本见 [M34 记录](../milestone34/PROGRESS.md)。

operator equals 的成员签名、operator 标记、已绑定调用及必要的派生正文使用既有 callable、模板与 exact-type metadata。`Equals<T>.equals` 的显式 conformance、接口 slot、实现和分派适配使用普通 interface metadata；该 slot 使用源码显式声明的 operator 实现，不按结构比较补齐接口，也不保存 Equals 专用的 core protocol 或条件接口规则。

自 `core-bootstrap-interface/14` 起删除旧 Equality protocol 字段；自 `cross-cone-interface/68` 与 `cross-cone-type-semantics/26` 起移除统一方案添加的派生比较接口槽位声明，恢复普通源码接口实现。旧版本产物必须重建，不能把旧 Equality slot 当作新接口成员或独立 operator 使用。既有结构 operator 的生成身份和单态化格式保持不变。M34 后续编码扩展版本为 `cross-cone-interface/69`、`cross-cone-type-semantics/27`。M35 普通 when 增加可缺省 subject、Case/Predicate/Always 条件和显式 Fallthrough 后，版本升级为 `cross-cone-interface/70`、`cross-cone-type-semantics/28`；随后显式 Nothing/Unreachable 表达式增加为 `cross-cone-interface/71`、`cross-cone-type-semantics/29`。旧 body 编码必须重建。注解参数完整 signature key 与静态数组值随后将 `cross-cone-interface` 升至 72；type semantics、MIR/LIR 与 runtime ABI 在这一批保持原版本。

派生 operator 使用既有 typed 生成身份和普通单态化／ODR 规则，所需的宿主、完整签名及字段调用在各自边界确定；没有源码声明的生成 callable 不伪装成 SourceFunctionId。导入的 operator 候选来自已保存的成员或派生签名；派生候选不伪造 Equals slot。artifact-only 链接只消费已闭合的实现，不重新扫描字段或选择重载；普通接口 adapter 引用实际声明或继承的实现，不因值支持结构比较而新增 itable 项。

接口方法与实际 metadata 内容的变化按既有 semantic／ABI fingerprints、section profiles 和缓存兼容规则演进。实现迁移时同步更新受影响的格式与 core 产物；M41 删除 core Equality/equalTo，改为普通 Equals/operator equals，旧依赖须按既有失效规则重建。不为库接口迁移增加 runtime 比较 ABI。

`single-cone-production/9` 只保存 field 11 的物理 ODR member 目录。field 1～10、12～14 不再写入；distribution、output、image owner、typed registration projection、OptimizationMode 副本、重复的 RuntimeImage／Code fingerprint、native contract／library／C++ 和 C bridge production 投影不再作为后续复核材料持久化。RuntimeImage 与 Code fingerprint 使用顶层 manifest 已保存的值，native requirements 使用 LIR foundation，入口使用实际语义与 production 数据。ODR directory 的 member 保存实际 role 与 OdrAbiFingerprint，field 4 保留不用；纯语义 member 不产生空物理条目。删除 production section 的 C bridge 与全量投影重建比较 API；对象生产阶段的实际 Code 输入与指纹公式保留。

`link-identity-closure/16` 只保存三个字段：field 1 为物理成员目录（Scoop member 保留 typed definition 分配，generated-C member 只保存 member id，不复制已无消费者的 unit 列表），field 4 为 defined symbol owners，field 5 为 undefined symbol uses。不再写入只服务消费者重放的 definition atom ranges、patch sites、对象目录副本及 image／entry owner 投影（原 field 2、3、6、7、8）。实际机器对象的生成、重定位和内容摘要仍使用生产端数据，不为生成此 section 再构造上述索引或对象投影。两个 section 的旧格式按版本规则重建；runtime ABI 不变。

runtime metadata ABI 9 删除没有运行时消费者的内容指纹字段。相应的 digest patch intent、物理回填位置和对象回填阶段一并删除；LLVM 直接发射完整记录，符号地址由普通重定位完成，归档复用原始对象及已有摘要。RuntimeImageFingerprint 仍由生产端对实际产物内容计算一次并保存在 manifest，最终 linker 只组装选中的记录指针表，不重算该摘要。原 field 14 的 canonical runtime image records 仅供最终链接重算摘要，现无消费者，故不再保存或读取。受影响的格式版本为 `single-cone-production/9`、`cone-production/13`、`strong-production/23`；该布局变更引入 runtime ABI contract 14。旧产物按版本规则重建，不能混用旧记录布局。

`cross-cone-link-closure` 与 `cross-cone-layout-link-closure` 不再作为 metadata section 生成、保存或消费。已保存的 ordinary／layout ABI imports、member、symbol 与 ODR 数据用于实际编译和链接，不另存语义 imports 副本、relocation use 副本、完整对象目录及 coverage digest，也不在读取或发布时重建并比较它们。生产端 Code contribution 直接使用既有的 ordinary 与 physical import 数据，沿用原 capability id 作为该指纹输入的字段名称，保持 Code 指纹公式；这些名称不再注册为 section contract。移除 section 同步改变 cross-Cone profile descriptor／fingerprint，旧 profile 按格式规则重建，不保留旧 reader。runtime ABI 不变。前批取消 native contract Code 副本后失去消费方的 projection reader 与全量比对 API 也一并退役；实际 native contract 编码与指纹输入保留。

`link-support/1` 为单字段 map `{1=runtime_data_aliases}`，把必要的 runtime 数据符号关联到实际定义；String alias 不创建额外 TD 或存储。

ProgramLink 直接读取保存的 member 到 typed definition 映射、defined symbol owners、undefined symbol uses、ODR ABI 与 native requirements，执行本次实际的对象选择和符号连接。它不再为消费已生成的 Scoop／generated-C 对象重建 producer partition、重定位、digest patch、stackmap、registration、final/provisional 对象副本、Code／RuntimeImage 指纹或 coverage 证明，也不再次读取 production section 来重算 Code。每个文件输入的 manifest、metadata 与按需成员共用同一个文件入口。显式依赖 locator 首次读取 manifest 摘要后保留该文件入口，并随候选与已完成构建结果共享；不得丢弃入口，再由 Compile／Link 打开同一文件重读 manifest。命中已保存的摘要记录时仍不打开产物，首次真正需要成员时才建立入口。选中的稳定对象文件已存在时直接传给 native linker，不读取对应归档成员；缺少该输出时才从同一入口读取该成员并保存一次。用户 native 对象仍按真实 native 符号选择的需要读取，链接失败照常报告。

ProgramLink 从已保存的 LIR 数据取得 definition 的 owner／role、image 与入口符号、六类注册的 definition／symbol，以及 native ABI 实际使用的标量／指针表示。它不重建整份 strong production、digest finalization、注册生产计划、shape support、descriptor／dispatch exports 或 physical imports，也不重新编码比较它们。runtime 数据别名直接读取其 typed owner；String 的布局合同在生产该别名时检查。链接所需的 native contract、ODR ABI、符号选择与 runtime image records 继续使用，当前符号连接错误正常报告。此读取简化不增加持久字段，不改变 wire/profile 或 runtime ABI。

Compile 为本次实际使用选择外部物理定义时，直接从 provider 的导出记录取得 callable ABI、layout／scan、descriptor、dispatch 和初始化／静态存储语义，并从已有 definition symbol plan 取得符号与 primary atom。不再为同一导出重建物理符号、扫描 atom 集合，或反查整份 type／callable 注册表重做布局、scan、继承与 dispatch slot 比对。provider 视图只借用实际需要的导出、definition symbol plan 和初始化／静态存储记录，不要求整份 strong production。缺失的实际 subject、错误的本地／外部引用及当前选择所需的类型与 ABI 规则仍在相应边界报告；导出生产时的完整性检查保持。

Compile reader 从 production 字段直接读取 definition symbol plan、静态存储语义、初始化语义及其 cell／registration 定义引用，解析本次消费所需的 typed 引用及当前依赖的初始化定义。只供对象生产使用的 digest node／patch、完整注册生产字段不进入 Compile 数据结构。它不重建整份 ConeProductionSection、digest graph、type／callable／safepoint／immortal 注册表、入口或 runtime image，也不再从 HIR 公开类型、Function 声明和 MIR 初始化使用重新证明这些已有记录。消费方不把读取结果重新编码后与原字节比较。已保存的 LIR dependency records 只解析 typed 引用，不从 HIR materialized roots 和注册表再次推导闭包并比较；实际外部导入选择继续使用这些记录。失去消费者的完整 V2 production／registration 重放、digest 重建及其独占测试一起退役。完整生产计划继续供对象发射使用。此前的读取职责调整不改变 wire；当前存储精简同步升级 production、链接 section 和 profile，具体字段见本节，runtime ABI 不变。

Compile 读取已完成的 HIR 接口时，直接解析保存的 typed 声明、默认值、调用点与类型信息；不再为同一依赖重新验证声明形状、可见性、默认参数来源、const 求值、intrinsic 或调用点签名。别名展开与实际引用解析仍用于后续编译；当前源码的语言检查继续由 HIR lowering 完成。仅服务消费端复核的 authority 实现、改写候选后要求拒绝的测试和发布时语义重放一起删除。

Compile 读取 ordinary callable bridge 与 callable ABI 时，直接采用已保存的 ABI、调用协议和 typed 引用，不从 HIR／MIR 声明重新生成同一桥接表，不重做参数／返回值布局分类或逐项比较。读取 MIR dependency records 时只解析实际 typed 引用，不再从 HIR materialized type／shape／property／singleton uses 推导整份候选闭包并比较，也不重做已有 HIR call-site 与 MIR 调用清单证明。生产端源码检查、依赖选择及 ABI／GC 生成保持；只服务上述消费者复核的测试、目录与重放 API 一并退役。

Compile 读取 MIR 类型、callable、constructor、object、dispatch、coroutine 与 equality 记录时保留本层格式、typed 引用和必要签名关系，不重新遍历 HIR 声明／继承图推导同一清单和语义。初始化单元从已保存的 strong callable 记录按 unit／role 取得 initializer 与 ensure 及其签名，不重算 generated identity，也不与 HIR source-unit 清单再次证明；缺失调用对仍报告实际引用错误。对应 HIR→MIR 对照重放及改写候选后要求来源复核的专用测试退役，真实源码生产、产物读写和语言规则 fixture 保留。

callable ABI 的底层 reader 直接解析保存的 signature／protocol、body ID、definition plan 与 symbol。它用已有 typed body key 和 definition 记录解析实际引用关系，读取 primary atom，不调用生产构造器重新派生 body／definition ID，不重编码 signature 或整张 ABI 表再比较。保留调用协议与 GC effect、目标配置、provider、表顺序及实际引用错误；生产端 ABI 分类和 definition 生成照常。只供候选与重建期望对照的单条／表级 ABI API 和测试一起退役。

Compile 的 dispatch 导出同样直接读取保存的 table、owner、role、slot signature、implementation 和 ABI body 引用，不从 MIR schema 重新生成表和 ABI 绑定，不把解码后的表重编码成另一份期望。reference-dispatch 的 receiver layout 从已读取的本地／依赖 layout 导出中按 typed exact 引用取得，不重算布局。表顺序、entry position、typed 引用和 definition 关系保留；生产端 slot／receiver／ABI 检查继续执行。

Compile 的 param-free shape-support 从保存的八种 role 读取 source、exact、layout／scan／descriptor／registration 与 box／coroutine helper 引用。它不从 MIR root 清单或源码 nominal 重新生成相同闭包，不为同一 role 重编码比较。source 与 descriptor 的必要角色、typed 引用及 canonical source 顺序在读取边界确定；实际布局、helper 生产和物理导入仍使用各自的数据与规则。专供重建期望的 shape-support reader API、候选修改工厂及重复表构造器一并删除。

Compile 的 layout／descriptor 导出直接读取已保存的值存储、字段偏移、enum 区域、class 完整字段和实例形状，以及 descriptor 的布局、继承、分派、注册和诊断名称引用。字段、MaybeUninit payload、box payload 和数组元素在表示中保存自己的 typed layout key 与完整存储事实，不要求这些内嵌值另外拥有物理布局导出或 symbol。reader 解析 key 中的类型引用并读取存储，不从 MIR 重新放置字段、重建继承布局、推导 GC／niche 或重生成 descriptor。保留实际格式、引用、目标配置及内存范围检查；不再次证明生产端语义。只为重建期望或修改候选后复核来源而存在的重放模块、API 和测试一并退役。生产端布局算法与 runtime ABI 保持；上述内嵌存储编码随本批 `cross-cone-layout-abi/16` 发布。

发布直接组合本次生成的 layout／ABI section 与 registration 数据，不再完整比较 producer／target、layout／scan／descriptor／dispatch／callable／shape-support 副本及所选导入清单。依赖 lowering 只投影实际语义根，不重建另一份物理引用全集或初始化依赖边来复核已有结果；实际导入选择、typed 引用查询与最终符号闭合继续处理缺失依赖和链接冲突。

物理 shape import 只保存 provider、typed subject、symbol 与 definition 引用，不再复制整份 ABI／layout／scan／descriptor／dispatch／storage／initialization 语义合同。消费时从已读取的选中 provider 取得实际合同并连接保存的 symbol／definition；不重编码比较合同副本。删除对应合同副本 reader、逐字段比较 API 与独占改写测试。此字段删除将 `cross-cone-layout-abi` 升至 16，旧版本产物按普通 profile 兼容规则重建；Code contribution 继续编码实际内存合同，runtime ABI 不变。

归档 writer 接收同一次对象生产得到的 member 与 Code 数据后，不再逐 member 重新计算 fingerprint 与生产目录比较。member 的普通格式检查和实际归档编码保留。defined symbol owners、undefined symbol uses、ODR directory 的 consumer 已使用保存数据后，失去用途的“与重建结果比较”API 及其专用测试也一并移除。

HIR／MIR／LIR foundation 读取复用 identity 边界已解析的 typed records，保留当前实际使用的引用和结构检查；不得为证明解码前后相同而完整编码原始 foundation、再编码重建对象并逐字节比较。仅服务这一深比较的错误分支和候选顺序改写测试退役。

旧 single-cone Link 的逐级验证资格 API、从 final 对象清零摘要再重建 provisional 的路径及其独占测试一并删除，不保留为备用 reader。仍被正常 Compile／ProgramLink 使用的 section 读取和真正生产对象的编码、重定位、摘要填充继续存在；归档格式测试使用正常产物，不能通过改坏最终对象来延续退役的消费复核合同。

旧 single-cone strong 与 cross-cone semantics strong Compile 的独立解码／验证／提交流水线及其专属测试退役，随之删除仅供这些入口使用的 native ABI 全量重放。正常源码依赖与当前产物统一由共有 layout reader 读取，不为历史阶段 profile 保留备用 Compile reader；归档／section 格式辅助数据和当前路径使用的基础引用检查继续保留。

persistent identity 只由首次引入的层声明一次，并同时保存 typed ID 与 canonical key；其他层引用它。相同 kind/ID 的冲突 payload 或跨层重复声明拒绝。LIR foundation 的 exact-type 集合引用已有 HIR/MIR exact records，不复制声明。源码文件、binder、字段、variant、body application 和调用位置的引用必须指向真实定义。

一次编译中的已验证 identity graph 共享不可变节点表与已声明节点顺序；继续登记新层时才取得可写表。节点的 typed ID、canonical key 及其具体类型共用一次查找，外部图导入只合并新增节点并核对实际冲突，不为每个依赖重新排序整个图或重建已有 key 集合。已完成图中的引用不可改变；新增记录仍检查 ID／key 一致性、引用存在性和层次方向，环检测只处理本次新增且需要解析的节点及其相互依赖，使用无递归工作队列。成功产物的次序在输出边界确定，冲突和环诊断按 typed ID 选择稳定位置，不要求内部工作队列保持全局拓扑排序。

semantic session 复用同一份不可变图的导入结果与未改变的索引；新增图仍核对 origin、typed key 与已有实体的一致性，失败不提交部分导入。进程内索引可以使用维护中的快速非密码哈希，不能参与 persistent ID、内容指纹或 wire 编码，HashMap 遍历顺序不能决定产物或诊断。这些共享与查找调整不改变 stage 边界、持久 ID、`.slib` 格式或 runtime ABI。

共有 HIR expression 为 `{1=kind, 2=result_type, 3=definition_origin, 4=evaluation_origin}`。Call 的 tag 为 57，payload 保存 callee、arguments、SourceCallReceiver；receiver 为 NoReceiver=1 或 Receiver=2（完整静态类型）。实际 call-site 保留完整逻辑实参与结果、定义／求值位置及原 receiver，Unit/ZST 参数不能按物理省略规则消失。

普通成员的接收者适用性由 HIR 成员绑定与条件求解完成。产物的类型依赖投影消费已检查的成员选择，保留原 receiver 的完整类型引用、所选声明归属和调用形状检查；不再从声明头重做接收者到成员 owner 的语言子类型证明。条件接口不要求额外产生无条件 `exact_supertypes` 边，IR／meta crate 也不另建条件求解器。

nominal declaration 保留原 binder 条件、GC-free pointee requirements 与 NoGC 标记。property/accessor、constructor safety/effect、InteriorMutable、CLayout、泛型 companion、release、annotation 和编码合成所需的声明事实均不可由缺字段的默认值替代。读取只检查格式、kind、引用与合同连接，不重新进行定义处的重载或可见性选择。

C extern 的 NativeSafe/GcLeaf 模式作为声明及调用的语义字段进入相应 HIR/MIR/LIR metadata、语义指纹和构建缓存。跨 Cone 编译与泛型展开保留原声明的模式；artifact-only program-link 消费已生成的调用，不能按合并后的 C symbol 统一提升或降级调用模式。该字段不改变 C bridge 的物理签名，也不要求为同一 native ABI 构造第二套桥接格式。

`captureErrno`、完整 Scoop 结果类型、native 返回投影及 bridge 结果适配进入对应声明、调用与 bridge 的 HIR/MIR/LIR metadata、语义/Code 指纹和缓存。编译消费方按已保存的结果适配生成 `(R, Int)`，链接消费方保留实际 bridge 及 native requirements；不按合并后的 native symbol 重新决定捕获，也不将旧的单结果 bridge 当作捕获 bridge。必需字段与 recipe key 的变化按既有 metadata/schema 兼容规则演进，旧产物不能缺字段后静默当作不捕获。

HIR `cross-cone-interface/69` 的 C extern implementation 必须保存调用模式与结果适配；LIR `identity-foundation/8` 的 OutboundFunction key 必须保存结果适配。generated-C 的 OutboundWrappers 模板版本为 2。目标工具链展开 `<errno.h>` 后产生的 libc errno accessor 引用作为普通 native 未定义符号保留：Darwin 为 `__error`，GNU/musl 为 `__errno_location`，均为无参数、返回 native pointer 的 C 函数。源码生成仍只使用 `errno` 宏，不自行生成目标 accessor 调用。

DirectC/StorageBridge 及其完整物理调用计划进入相应 LIR metadata 与既有语义/Code 指纹和缓存投影，不加入 source native symbol 的 ABI 冲突键。DirectC 保留真实 native undefined reference、contract 与 library requirement；没有实际桥接用途时，不生成 outbound bridge recipe、物理定义或 member 要求。跨 Cone 与泛型消费按当前 target 得到同一完整计划；artifact-only 链接只消费产物记录，不重做 ABI lowering，也不为直接调用重新插入 bridge。

native-boundary record 保存 owner、type-parameter count、source shape 与 C ABI projection。projection 为 SourceRepresentation=1、UInt64Field=2（唯一 field ID）或 NullablePointer=3（empty variant 与 pointer payload field ID）；必须匹配真实声明的字段、variant 与类型，不改变普通 Scoop aggregate/niche 表示。

三层语义指纹是 Merkle 指纹：各自 canonical own-layer projection 加真实 direct/support/re-export 依赖的同层指纹。其 domains 为 `scoop-hir-semantic-v1`、`scoop-mir-semantic-v1`、`scoop-lir-semantic-v2`。LIR context 包含 identity ABI、target profile 与 target fingerprint，不含 backend optimization、member assignment、range 或 patch offset；backend 兼容性仍单独检查。

HIR 语义包括名称候选、签名、默认值、const、模板和完整依赖；无法完整表达细粒度 lookup observations 时，保守计入全部相关直接依赖。普通私有非泛型正文、机器码、优化模式与对象分组进入 Code/构建缓存，不能反向改变声明或 shared ABI 身份。

MIR `identity-foundation/6` 与 LIR foundation 保存实际实现的实体目录，包含优化新建的局部值；它们进入 Code 与 Link validation，不作为依赖语义贡献。MIR 的可消费语义由 bootstrap／param-free／type bridge 保存，LIR 的 shared ABI 由对应 layout／param-free section 保存。内联新增局部值不能使同一源码的 debug／release 依赖互相过期；完整 identity、格式、引用与 ABI 校验仍在各自边界执行。

`MemberFingerprint = H("scoop-slib-member-content-v1", member_record)`；`LinkMemberFingerprint = H("scoop-slib-link-member-v1", member_record)`，后者只用于 LinkObject 或 Link-required blob。

`CodeFingerprint` 使用 `scoop-code-v1`，覆盖按 member ID 排序的 Link fingerprints、已知 Link contributions、generated-C production、native library requirements、defined/undefined symbol 集合、去除诊断位置的 native contracts，以及 section registry 声明的 Code projections。只改变实际合同或符号需求也必须使 Code 改变；diagnostic/unknown optional payload 不进入 Code。

`ArtifactFingerprint = SHA-256(ByteSpan("scoop-artifact-v1") || ByteSpan(canonical_manifest_without_artifact_fingerprint) || ByteSpan(MemberFingerprint_0) || ... )`，成员顺序为目录顺序。它覆盖全部附件与 blob；manifest 不包含自身 member hash。RuntimeImage 编码及 body/stackmap domains 见运行时规范 2.8。

产物数据按本次 Graph、Compile、Link 的实际需要读取，不以验证资格转换表达阶段关系。一个消费进程复用同一按需入口；同一产物的同一 section 首次需要时解码一次，后续共享或借用结果。发布直接使用已生成的 IR、对象与摘要，不重新打开自身归档，不由 parent 重做 child 的正确性检查。显式 locator 与缓存共用普通读取入口，不维护另一套不可信产物 reader；更新判定只需路径和记录时不打开归档。

### 2.7 `scoop` 与 single-Cone `scoopc`

`scoop` 负责精确依赖图的构建顺序、全局构建缓存、runtime 构建和程序链接；`scoopc` 一次只编译指定的一个 Cone，不跟随上游 manifest locator 或读取上游源码。

**M63包管理，本轮消费与工具接入已完成。** manifest/workspace根选择、版本要求、Git/registry获取和`Cone.lock`由独立`scoop-pm`负责，完整合同见[包管理规范](SCOOP-PACKAGE-SPEC.md)，实施步骤见[包管理设计](../package-manager/DESIGN.md)。`scoop pm`转发到配套可执行程序；普通manifest构建通过一次prepare请求取得lock的精确图、本地源码/产物位置和已展开配置，再接入现有调度。默认build/run遇到缺失lock时仍调用PM自动解析并创建，成功后继续构建；只有显式locked/frozen才禁止创建。driver不链接PM实现，不处理网络或再次求解；registry源码缓存默认放在`~/.scoop/packages/`供多个项目共享。以下workspace/锁定输入规则已经实现，machine protocol版本在对应段落单独说明；源码打包/发布流程留至下一阶段。

`Cone.lock`只保存可移植的精确选择；PM返回的本地准备结果是该图与清单的投影，不是另一份锁文件或构建成功收据。按root/purpose/target分别构图，不将workspace多个程序合并为一个单版本图；跨根共享任务时必须比较实际配置、来源和依赖结果，不能仅按ConeIdentity合并。`scoop`消费该锁定图，向`scoopc`传递展开后的单Cone编译配置与具体direct/support产物。`scoopc`不读取原manifest或lock，不匹配版本要求；旧独立manifest入口退役，普通Cone统一由scoop编排，不能为保留低层入口另建一套清单绑定。

开发 sysroot 准备、发行组装、安装及打包属于仓库维护脚本，不进入 `scoop` 命令或 driver。脚本调用普通 Cone 构建与已有 runtime 编译库；日常命令只消费选定的 sysroot 和显式指定的构建输入。

`scripts/package_toolchain.py` 是完整安装包的单一构建入口，负责 LLVM 输入准备、宿主命令、该宿主支持的全部目标 native／`.slib`／runtime、内置库文档和 VS Code 扩展。Darwin/AArch64 包包含 `aarch64-apple-darwin`，Linux/GNU amd64 包同时包含 `x86_64-unknown-linux-gnu` 与 `x86_64-unknown-linux-musl`。工具包归档直接以安装根内容为顶层，不包裹版本目录或额外的 `.scoop`／`sysroot` 目录，可用 `tar -xJf <archive> -C ~/.scoop` 安装。完整布局包含 `bin`、`commands`、`lib/<canonical-target-triple>`、`include`、`src`、`vendor`、`native`、`libexec`、`docs`、`share` 和 `tooling`；`commands` 即使为空也进入归档，存在内容时按原目录复制并保留可执行权限，仓库保留空目录的 `.gitkeep` 不进入发行包；扩展以可直接安装的 VSIX 放在 `tooling`，由同一入口按 npm lockfile 构建。安装根的纯文本 `VERSION` 记录发行版本、源码 revision 和宿主平台，与工具构建及发行清单共用本次参数，只用于查看安装版本。默认 LLVM 输入缺失时由维护脚本取得固定版本的上游归档并解包；显式输入路径失败照常报错。全部组件准备成功后才组装并发布归档。

仓库的 `tooling/scoop-programming/` 保存面向 AI 的 Scoop 编程 skill，包含 `SKILL.md`、界面元数据与按需阅读的参考资料。完整打包入口将该目录原样复制到工具包的 `tooling/scoop-programming/`，在 `share/scoop/toolchain.toml` 的 `programming_skill` 字段记录安装根相对路径。skill 使用配套的 `scoop example`、CLI、离线 API 文档及库源码指导程序开发，不另行复制示例库或实现编译入口；打包及普通命令不自动安装、注册 skill，也不修改 AI 工具的用户配置。

宿主包将 `scoop`、`scoopc`、`scoop-link` 和 `scoop-lsp` 放在 `bin`，四个命令共用发行版本与源码 revision；实际使用的 LLVM 动态库及非系统传递依赖放在 `libexec/llvm/lib`。Darwin 通过相对 rpath 和 dylib install name 加载；Linux 的每层私有依赖使用相对 `$ORIGIN` RUNPATH。组装脚本从实际动态依赖取得需要复制的库并修改加载路径，不在日常命令启动时重扫这些文件。版本、源码 revision、目标部署配置、Cone coordinate 和 native 版本写入 `share/scoop/toolchain.toml` 供说明；不把清单作为认证或逐文件验证输入。宿主和 LLVM 源码归档由手工脚本生成，不加入常规测试或 CI 的前置步骤。

M63 在同一 `bin` 布局中分发配套 `scoop-pm`，宿主构建与第三方许可证收集均包含该程序及其依赖，它本身不依赖 LLVM；独立`pm`转发优先于既有`commands/`外部子命令查找。源码包registry的package/静态索引维护由PM负责，不与此处工具链/sysroot发行组装混为一个发布入口。

```text
scoop new <path> --group group [--name name] [--kind executable|library] [--version version] [--package package] [--no-tests]
scoop build [root-input]
scoop build [--workspace | -p group:name] [--lockfile path] [--locked] [--offline]
scoop pm <command> [arg...]
scoop-pm <command> [arg...]
scoop clean [root-input] [--target-dir path] [--message-format human|json]
scoop run [root-input] [-- program-arg...]
scoop run file.scoop [runner-option...] [program-arg...]
scoop [--sysroot path] <external-command> [arg...]
scoop link --root-slib root.slib --runtime-objects index [--dependency-slib dep.slib]... [-o output]
scoopc build <file.scoop> [--direct-slib direct.slib]... [--support-slib support.slib]... --out-slib current.slib
```

无 lock 的首次 build/run 与已有 lock 的构建使用同一个 prepare 入口，具体顺序见[包管理规范 7.2](SCOOP-PACKAGE-SPEC.md#72-自动准备与低层调用)。PM 完成解析、来源准备和 lock 发布后，driver 才调度编译；PM 失败则停止本次命令。后续编译或链接失败保留正确生成的 lock，run 不执行旧程序。

低层 `scoop-link` 使用相同的普通产物链接合同。manifest build/run省略root时按包管理规范3.1从cwd定位最近清单及所属workspace，成员内默认该成员，workspace根采用default-members；`run -- args`也采用该选择，必须得到唯一executable。低层scoopc输入必需。single-file与artifact-only link不启动PM或读取相邻workspace。

`scoop` 优先解析内置子命令；其余子命令名称必须是单个文件名，并直接执行所选 `<SYSROOT>/commands/scoop-<名称>`，Windows 使用 `scoop-<名称>.exe`。目标为宿主可直接执行的程序（Unix 包括带解释器声明的脚本），不经过 shell 解释或 PATH 搜索。子命令名称之后的全部参数原样传递，包含选项、`--`、空字符串及宿主原始参数字节；Windows 保留原 UTF-16 参数。`scoop` 自身的选项必须放在外部子命令之前。`--sysroot` 采用本节既有的显式参数、`SCOOP_SYSROOT`、安装根优先顺序，对内置子命令仍可放在子命令前后。外部程序继承 cwd、环境和标准输入输出，正常退出码与信号终止状态由 `scoop` 传回；程序缺失、不可执行或启动失败按实际路径报告，不触发编译、链接或工具探测。

`scoop new` 创建普通 manifest Cone，group 必需，name 默认取目标目录名，kind 默认 executable，version 默认 0.1.0，package 默认 group；名称、版本与 package 使用现有 coordinate 和源码名称规则，显式 package 必须为 group 本身或其子包。生成 package 时按名称段转义连字符或关键字。固定模板包含 Cone.toml、executable 的 src/main.scoop 或 library 的 src/lib.scoop；默认还包含 tests/basic.scoop 及 scoop.test 测试依赖，`--no-tests` 同时省略两者。生产依赖沿用 manifest 的默认 core 规则，library 不生成 main。所有参数先验证，再创建目标目录；目标必须不存在，普通文件、空目录、非空目录均拒绝覆盖。父目录可按实际需要创建，I/O 失败按原路径报告；此命令不构建、不下载模板，也不读取 sysroot 或构建缓存。 `new` 的 CLI 参数校验可对内存中的 package 声明调用现有 parser，以复用转义标识符和关键字规则；它不读取或解析项目源码。构建编排库继续通过 scoopc 执行源码编译，不调用 HIR/MIR/LIR lowering 或 codegen 实现。

**源码格式化与检查。** `scoop fmt`、`scoop check` 及低层 `scoopc check` 的完整命令、格式规则、缓存和验收范围见[设计](../fmt-check/DESIGN.md)。两项命令均已实现，验证状态见设计文首及验证记录。

`scoop fmt [input...]` 接受 Cone目录、Cone.toml或显式`.scoop`文件；M63中manifest模式沿用workspace根选择，调用PM的本地members展开而不要求依赖可解或联网。manifest模式收集生产与测试源码，包括合法target条件下的全部声明路径，分别按文件解析和格式化，不将不同平台的源码合并成编译输入；显式文件不要求manifest、main或可用依赖。缺省生产目录仍为src/，缺省tests/不存在时忽略，显式路径缺失按I/O错误报告；格式化集合允许为空。文件并集按普通解析后的实际路径去重，保持确定顺序，不改变正常编译对同一target内重复源码选择的诊断规则。只处理用户选中的源码，不发现依赖、读取sysroot、启动scoopc或编译native输入。

格式化采用四空格缩进和 100 列软行宽，只改语法允许改变的空白；保留全部非空白 token、注释原文及顺序、字符串内容、shebang、显式标点、声明与 import 顺序。语法上下文决定换行和相邻关系，不能改变 return 值、调用、下标、尾随 lambda 或文档附着；行宽不能优先于语义。布局规则与换行选择详见设计第 4 节。语法错误文件报告原始诊断并保持原文，其他已选文件可继续处理；每个成功文件只在文本变化时写回，保留普通文件权限和可执行位。批量格式化不提供跨文件事务。

`scoop fmt --check` 使用同一选择与排版，只报告需要修改的文件，不写源码。格式检查存在差异或任何文件处理失败时退出 1，全部符合时退出 0；写回模式全部成功时退出 0。参数用法错误沿用退出 2。human／JSON 诊断和格式结果写 stderr，stdout 保持为空；格式差异是独立结果，不伪装成语言错误。

scoop build [root-input] --emit-slib [--tests] [-o output.slib]
`scoop check [root-input]` 沿用 build 的 manifest／single-file 输入、目标及依赖选择，默认检查 Production；`--tests` 将根切换为现有 Test 用途，只接受 manifest，依赖仍为 Production。Test 检查使用生成的测试入口，不额外要求生产 main；验证 executable 的生产入口需使用默认检查。检查完成普通 parser、package 检查、全部正文、必要 generator、effect 及 ExportHir／LocalConcreteHir，保留具体化中的类型条件与递归错误，然后返回诊断及无产物的成功结果；不进入根 Cone 的 MIR／LIR、native 编译、机器码或最终链接。检查不替代后端、native 源码或链接验收。

首版检查继续通过 direct／support `.slib` 消费依赖。预编译依赖与成功的源码依赖缓存直接复用，需要更新的源码依赖按现有构建图正常产生完整库产物；因此“根不生成机器码”不表示整个命令永不运行后端。根本身不发布 `.slib`、对象或 executable，也不准备 runtime；仅当实际源码依赖需要机器生产时发现相应 native 工具。低层检查只消费上层传入的具体编译配置和依赖，不跟随locator或读取清单；Test用途由scoop的请求显式传入。直接`scoopc check`只保留单文件输入及具体依赖参数。

检查成功保存普通本机更新记录及 warning。键按实际前端输入区分根、用途、源码选择与 metadata、目标、编译器／协议、依赖选择及使用的 generator；Production 不纳入仅测试配置。先判断更新，再读取源码、探测工具或准备依赖机器生产；命中不读产物内容复核、不重放 HIR。生成失败、检查失败或中断不发布成功记录，也不返回旧成功。首版复用依赖机器缓存和独立检查记录，不通过读取 build 产物补造检查记录；全图纯语义依赖传输另行设计，不新增残缺 `.slib` profile。

显式单文件 run 支持省略程序参数分隔符：root 后首个普通位置参数开始原样转发，其后的所有元素（包括 `--`、已知 runner 选项和空字符串）均属于程序。转发开始前，已知 runner 选项仍由 Scoop 解析，未知连字符选项仍报参数错误；显式 `--` 开始转发并只消费这一次分隔符。因此 `run file.scoop Alice --release` 转发 Alice 和 --release，而 `run file.scoop --release -- Alice` 以 release 构建并转发 Alice。首个程序参数与 runner 选项冲突时必须使用 `--`，例如 `run file.scoop -- --help`。普通 manifest run 仍要求显式分隔符；选项的值不被误认成 root。

单Cone编译输入为两种封闭形式；workspace在PM层展开成多个已绑定Cone，不成为第三种语言编译单元：

- ExpandedCone：PM已从Cone.toml展开workspace与依赖，scoop传入单Cone身份、kind、生产/测试源码及native配置、精确依赖和诊断来源。scoopc不定位或解析Cone.toml。未配置`sources`时递归扫描`src/`；显式配置按语言规范12.2.2选择当前target的源码，不追加默认目录。目录只收集扩展名精确为`.scoop`的regular files，也可显式选择单个`.scoop`文件；最终按标准化Cone-relative UTF-8路径排序。
- SingleFile：operand 扩展名精确为 `.scoop`，解析 symlink 后为存在的 regular file。目录输入始终按 manifest／workspace 定位，即使目录名以 `.scoop` 结尾。只编译该文件，logical path 固定 main.scoop；不读取相邻 manifest、Scoop/C/C++ source 或 blob。
`scoop build --emit-slib`保留完整PM/lock准备、依赖编译、native对象生产及根`.slib`发布，跳过runtime和最终链接，用于分离产物编译与程序链接。`--tests`仅与此选项共用，编译Test用途的根产物；默认Production。输出规则见包管理规范7.2，输出意图不改变编译键；后续普通build复用相同产物再链接，不把executable Cone伪装为library。


manifest 字段、路径形式及 `when` 的合法性先于 target 筛选检查；文件系统检查和递归枚举只作用于选中路径。选中目录相同或互相包含、单文件与目录重叠、重复标准化路径或 symlink 重复指向同一源文件时，报告来源条目，不静默去重。选中路径缺失或类型不符、最终空集合、非 UTF-8 或实际 I/O／目录遍历循环错误均在 parse 前失败；显式清单为空或全部被筛掉也不回退到 `src/`。

更新判定只发现选中文件集合及其修改时间、大小和逻辑名，不先读源码内容。`sources` 的语义投影区分默认与显式模式，规范化路径、条目顺序和条件的等价写法；未选中文件的内容与原始 span 不使编译失效。需要编译时，parent传递本地位置和已展开的源码配置，scoopc读取本次所需源码；不创建、封存完整输入镜像，不检查读取前后inode、owner、权限或文件替换。两端复用同一已展开源码选择规则，向parser交付相同的完整集合；scoopc不通过读取原manifest补充配置，后续阶段不能再追加默认源码。`SourceIdentity`、file-private实体及SourceLocation使用从Cone根开始的完整路径，选择条目的顺序和分组不进入身份；改变源码集合只通过既有源码内容、依赖和指纹规则影响产物。

SingleFile 的 coordinate 为保留值 `scoop:single-file:0.0.0`，kind 为 Executable；用户 manifest 不能声明该 coordinate。生效直接依赖为 `{core, std} ∪ 头部显式 dep`，共享单文件头部读取器按语言规范 12.1 处理坐标与原始 span。build/run 接受 cone-path，按既有精确 coordinate 查找库产物及传递闭包；不下载依赖或从相邻 manifest 构建。scoopc 接受 direct/support slib：沿用现有 core 槽位，调用者提供 std 与其他所需产物，检查 direct 集合及 support 闭包，不自行定位其余依赖。缺失 std 不能回退 core-only。单文件 `.slib` 仍要求一个源码及 executable kind，分类为 LocalExecutableRoot，可缓存、运行与显式 link，不能成为其他 Cone 的依赖或可分发 Cone。

单文件头部解析记录使用独立 `single-file-headers-v1` namespace，以实际输入路径、mtime、大小判断更新，先读取记录再读取源码。记录命中不打开源码或读取命中产物复核；输入变化才重读头部并发现依赖。实际编译的 scoopc 在一次源文件读取中复用文本处理头部与语法，保留 shebang、注释和 package 的原文与位置。默认依赖只在共享读取器中定义，图、子请求、direct 集合校验和编译缓存均使用该集合；模式与依赖改变通过实际输入及现有结果指纹判断更新。

测试配置遵守语言规范 12.2.4。PM 的包声明保留独立的测试源码清单、测试依赖及其 locator/span；生产语义投影仅含生产字段。普通 build/run 与生产用途的 scoopc 不发现测试文件或定位测试依赖，编译缓存键不包含测试配置或测试文件摘要。PM 按 manifest/lock/config 的路径、mtime、大小和工具版本复用已有解析及准备记录，未变化时不打开原清单或重跑 TOML parser；变化时按有效配置判断相应用途是否更新，缺失输入仍报告普通定位错误。原始文本与 span 只用于诊断，文本变化本身不等于生产语义变化。记录格式或解析规则改变时升级 namespace，不迁移旧记录或重放语言规则。

普通 manifest 入口通过 PM 展开 workspace/依赖，协议 19 将有效配置交付给 scoopc；child 只读取实际源码，不重解析清单。直接 manifest 入口已删除，不保留另一套新版声明解析或版本绑定；生产/测试分别判断更新及不重放检查的合同保持。

single-Cone编译请求显式携带Production或Test用途，默认Production；`scoop test`及`scoop check --tests`使用PM为测试根锁定的图，并向scoopc传入Test用途及具体生产/测试依赖。测试编译只接受ExpandedCone，support仍恰为其余传递闭包；scoopc不读取声明、求解或补建缺失的测试依赖。解析时使用已展开的生产/测试源码配置，将仅用于测试的依赖身份交给前端，并以原ConeIdentity产生具有TestEntry的executable `.slib`，不要求改写源码manifest kind。该字段自machine protocol 6加入；当前协议版本见下文，旧版本拒绝协商；普通生产请求的用途必须显式编码，不能靠源码目录或输出名推断。

文档是独立的输出意图，源码用途固定为 Production，且只接受 manifest Cone。机器／文档请求的 single-Cone 输出为 `.slib`、文档渲染输入、或两者同时生产的闭合选择；仅文档请求不携带虚假的 `.slib` 输出路径。检查意图按上文设计另加无产物的闭合分支。driver 在 parser 后整理文档，再完成正常 HIR 及 §2.2 的文档投影；仅文档随后发布本地 JSON 渲染输入并返回，不进入 MIR／LIR、native 编译或机器对象生产。该路径只用共享 target registry 选择目标，不探测无关的 C／C++ 工具或准备 runtime。生产附带文档从同一次 HIR 提取，在实际机器生产成功后发布文档输入；输出与普通 `.slib` 分别保存。

文档成功响应携带渲染输入位置及独立的文档 warning，普通编译 warning 仍保留原分类；渲染输入包含页面目录组装所需的声明、typed 路由和诊断。仅文档成功没有 artifact summary。文档请求不与 observation-only 混用；仅文档可输出 AST／HIR dump，不能请求尚未执行的 MIR／optimizations／LIR 观察。普通编译不整理或报告文档问题；语言正文、默认值或可见性错误仍阻止文档输出，失败不覆盖之前的结果。工具本地 JSON 随编译器版本更新，不是 `.slib` section 或独立分发合同。

`scoop doc`接受零个或多个manifest root，M63中沿用PM的workspace/default-members及显式根选择。各root使用独立的已锁定图；只合并实际依赖绑定与配置相容的源码文档任务，不建立synthetic Cone。默认包含显式root及图中选中的源码依赖，`--no-deps`保留全部显式root；prebuilt节点保持原消费来源。目标选择只需要纯target registry数据，native工具与输入仅在确实需要生产依赖`.slib`时准备。仅文档root没有机器生产、runtime或最终链接。

文档前端输入记录使用实际 root／target、生产语义配置、选中源码的普通文件状态、依赖选择与结果及配套工具版本判断更新。命中且所需文档结果存在时，不读取源码内容或探测工具，也不读取命中结果复核内容；仅在站点确实需要重新渲染时读取已保存的文档输入。源码依赖的机器产物与文档分别判断缺失：冷产物附带文档，机器命中时仅补文档。README 与其实际引用资源独立决定首页更新。普通文件锁协调同一结果；全部页面、资源、链接和 warning 政策检查成功后替换站点，失败保留旧站点。Unix 使用目录交换；Windows 在同一发布锁内先将旧目录改名到同级临时位置，再发布新目录，第二次改名失败时恢复旧目录。若普通 I/O 错误也阻止恢复，报告旧目录实际保存的位置并保留其内容，不删除旧结果。

文档渲染先汇总选中 Cone 的可见声明和 typed 路由，再生成页面与链接。先比较各根所有节点的精确依赖边，包括 prebuilt 节点；全部根中同身份节点的绑定相容、同身份源码的实际输入也相容时，共用一个站点 catalog；若同一 ConeIdentity 对应不同源码或依赖绑定，则每个根分别建立 catalog，放在 `r/<root-key>/` 下，由顶层首页列出各根入口。root-key 由根身份与实际清单位置决定，不随源码内容或依赖版本变化；同一绑定的文档编译结果仍共享。各 catalog 的链接与搜索只指向该根实际选中的声明，不能用另一个根的绑定补全链接；整站一次发布，任何根失败均保留旧站点。`--no-deps` 在各根范围内只保留显式选择的根。这里的页面范围只影响展示地址，不改变语言实体身份。

Cone、package、类型、函数 overload 组与源码页在所属 catalog 内使用完整身份确定的相对路径；显示名称与文件路径独立，成员、参数及 accessor 保留实际目标锚点。多目标引用连接到完整的可见 overload 集；解析成功但未生成页面的目标保留文本并注明缺少文档，不伪造 URL 或新增名称诊断。页面包含签名、tag、可见性与 property 能力、原文源码行及公开 re-export 的实际来源。

`scoop-doc` 使用 pulldown-cmark 处理 Markdown，使用 MiniJinja 的内置模板与 HTML 转义生成页面；原始 HTML 当作文本显示，不执行代码示例或活动 URL。站点共享样式和脚本保存在本地，导航在无 JavaScript 时仍可用；搜索索引通过普通静态 JavaScript 加载，覆盖 Cone／package、名称、声明种类、摘要、DocAlias 及可见 re-export，不使用 fetch、CDN 或外部服务。渲染只消费投影、README 内容与调用方已确定的资源路由，不重新发现或读取源码。

`scoop test [root-input]` 使用同一构建图，只对 manifest 根 Cone 启用 Test 用途：发现根的生产与测试依赖，依赖节点仍按 Production 构建，不递归启用依赖自身的测试。根按 executable 产物规划和链接，保留源码 manifest 及 ConeIdentity。测试编译键包含用途、每个源码的角色、测试源码选择与测试依赖语义；本机源码实际路径、修改时间和大小参与更新判定，诊断显示 locator 与 span 不进入键；生产键不包含这些测试配置。测试源码或配置改变不使未改变的生产依赖缓存失效。

测试程序发布到 `target/<canonical-target-triple>/<profile>/<group>/<cone-name>/tests/<cone-name>`，`--target-dir` 替换 target 根目录，`--no-run` 完成构建和链接后返回。运行直接使用本次成功构建确定的已有或新生成 executable，不制作 execution copy；失败构建不得运行旧文件。过滤和忽略选择不改变测试编译产物；最终报告需要独立的用例结果通道，不能仅凭进程退出码判定整轮执行完成。

M63统一采用包管理规范3.1的`target/<target>/<profile>/<group>/<name>/`成员输出目录，测试放在该目录的`tests/`内；一次workspace测试分别产生/运行各根的测试程序，不将它们链接到同一进程。

`--list` 完成更新判定后从已保存的测试目录输出容器与用例，不启动测试程序，与 `--no-run` 互斥。human 列表写 stdout，JSON 模式沿用 stderr 的结构化记录流，构建记录之后输出 `test-list` 记录；记录包含嵌套容器、用例位置与忽略原因，以及容器、用例和忽略计数。

`--filter text` 对完整显示名进行大小写敏感的子串匹配；默认选择未忽略用例，`--ignored` 改为只选择被忽略用例。列表与执行采用同一选择函数，移除没有选中用例的容器；空选择区分无测试、无匹配和忽略策略下无用例。`--no-run` 仍构建完整程序。前端为全部用例（含 Ignore）生成适配，并在实例初始化之前用普通库选择调用包围完整生命周期；入口参数沿既有 Array<String> main ABI 传递，无需新增 runtime ABI 或改变测试编译键。

每次实际测试运行创建独立的临时运行目录，以 `SCOOP_TEST_RUN_DIR` 交给普通测试库。库向其中的 events.jsonl 写入并刷新 begin（目录内用例序号）、end（同一序号、纳秒耗时和失败数组）及 complete 记录。父命令只按该独立通道生成测试状态，不从用户 stdout/stderr 识别事件；读取时检查实际选择顺序、开始／结束配对和唯一完成记录。缺少完整记录时，即使进程退出 0 也失败；shutdown 的非零退出或信号不能被已有 complete 掩盖。已开始但未结束的用例失败，剩余未开始用例为未运行；启动或收尾失败记录到目标。空选择直接报告 0 用例与原因，不启动程序。

默认将目标 stdout/stderr 写入独立文件；测试库在 begin 后刷新 stdio、保存并重定向 fd 1／2 到本次用例的文件，在生命周期结束后刷新并恢复，再写 end。文件按开始顺序命名，不从显示名构造路径；后台工作应在用例结束前清理，异步输出仍遵守既有线程／stdio 规则。`--nocapture` 保持继承的 stdout/stderr，不重定向，并与 --list／--no-run 互斥。捕获包含实例初始化到清理的全程；失败与中断用例的输出、目标输出分别报告，普通通过用例的输出默认不展示。用例中提前退出时，尚未恢复的 stdio 输出随该未结束用例保留，包括此时运行的 native atexit 输出；不能根据用户输出内容猜测阶段。JSON 报告中的输出以 UTF-8 text 或非 UTF-8 hex 保存，不有损转换。运行目录在报告后清理，不在缓存中持久化运行结果。

测试源码发现复用相同的路径选择、平台筛选和文件读取规则，在合并前保留生产集合的非空要求，在合并时检查两种角色之间的路径重叠及物理文件重复。源码角色是编译输入属性，不参与 SourceIdentity；parser 将角色随每个已解析文件交给前端，不能由目录名、文件顺序或显示路径重新推断。成功结果保留按完整 source identity 排序的全部文件及其角色，解析错误仍按原文件定位。

前端输入显式标明仅用于测试的 direct dependency 身份，在同一已加载的依赖语义数据上按源码角色收集名称候选。package binding 在按实体合并前筛选直接来源，qualified-type 索引保留来源贡献以复用相同数据；选中真实 nominal 后的静态成员遍历不按原始声明 Cone 再次排除实体。不为生产与测试分别加载或完整校验相同的依赖闭包，不在下游 IR 重做定义处名称查找。

编译输入的依赖均使用exact version、全部为library，同一程序闭包中同一group:name只允许一个版本。M63中原manifest要求/来源由PM求解为该精确图，循环/来源冲突在解析边界诊断；实际产物消费继续检查target/ABI/schema与精确语义绑定，不因manifest范围兼容而重绑旧`.slib`。不为复用已完成节点重读产物或重算语义/内容摘要。canonical记录顺序由每个静态图生成，逐次取全部前驱已排序的coordinate bytes最小者，不按输入顺序或并发完成顺序决定。实际执行使用依赖完成后的ready set，按canonical顺序优先启动可运行节点；独立源码Cone可以并发，每个child仍只编译一个Cone。

`scoop build/run/test` 的 `-j N`／`--jobs N` 指定同时运行的编译 child 上限，N 必须为正，默认使用宿主可用并行度。并发数只影响调度，不进入语言身份、编译键、产物格式或 machine protocol。缓存更新判定与锁内重查先于 child 请求准备；命中节点不占用编译 worker，缺失的显式观察沿相同调度执行。同键锁覆盖实际编译／观察到结果发布，不在运行 child 时释放。持有运行任务的锁时，其他键的锁获取不得阻塞该任务完成与释放锁；繁忙键延期到后续调度。节点成功后立即供其 dependents 使用；观察记录和 warning 按 canonical 顺序汇集。

多个workspace根共用此jobs上限；诊断先按root key、再按各图canonical顺序汇集，不能为每个根另开N个worker。PM只准备依赖，不调度编译child；源码包下载锁与编译任务锁分别协调各自结果。

非 core Cone 未显式声明 core 时，注入 `scoop:scoop.core:0.1.0` direct edge；core 不依赖自身。core 是普通库，显式 locator 遵守同一发现规则。缺省 sysroot 来源为 `lib/<canonical-target-triple>/scoop.core.slib`；不读取或枚举随包 `src/scoop.core`。已选来源损坏或不兼容时直接报错，缺失目标产物时报告目标、coordinate 和路径，不回退到源码或其他目标。

显式声明 core 版本但未指定 locator 时，复用图中已选 core；尚未选定时先查显式 artifact search root。搜索无候选不立即报缺失，待其他声明的源码／产物依赖展开完毕后，若仍无 core 才使用上述 sysroot 缺省来源。这使普通库的显式 core 依赖与隐式 core 边绑定同一实际库，也允许后续发现的依赖提供显式 core locator；无效候选或显式 locator 错误仍直接报告。

其他已声明的 `scoop` group 依赖在没有显式 locator、且全部显式 artifact search root 均无候选时，最后检查 `<sysroot>/lib/<canonical-target-triple>/<name>.slib`。使用普通产物入口匹配完整 coordinate、library kind 和目标，继续定位其实际产物依赖；不要求额外的 cone-path，不因应用选择 debug 或空缓存而重建发布库。其他 group 不查该默认位置；显式来源失败、候选歧义或已选 artifact 不兼容时不回退。随包源码只供阅读和显式源码请求，默认定位不读取它。`scoop link` 遵守同一显式产物优先、最后采用 sysroot 产物的顺序，不读取源码或补建缺失 artifact。

M63中上述manifest来源选择由PM执行：普通非`scoop`裸依赖改用明确registry，系统库候选需满足版本要求后锁成精确coordinate；core仍固定0.1.0。单文件与artifact-only链接保留原精确artifact查找方式，不沿PM自动下载缺失来源。

配套 scoopc 默认来自 scoop executable 同目录，允许显式指定。首次使用或工具路径、修改时间、大小及配置改变时取得其能力／协议；未变化时复用记录，不启动探测、不完整读取程序计算指纹，不在执行前后重捕获工具。本机工具记录保存在 `tools-v1`，保存路径、mtime、长度、生效的配套协议配置和实际能力响应；工具输入摘要由这组普通输入生成。compile key domain 接续为 `scoop-cone-compile-cache-v7`，使用独立的 C 工具输入与 native 对象构建输入摘要，不改变语言实体或 runtime ABI。每个 source cache miss 启动一次 single-Cone 编译，传入实际 direct/support artifact 路径；direct 对应 manifest edge，support 为其余传递依赖，两组互斥。工具启动、协议读写和实际编译失败正常报告，缓存复用不重放 child 成功产物及依赖闭包的完整验证。

C 工具链的版本、SDK／libc 配置和实际调用参数保存在 `native-tools-v2` 普通记录。发现语义版本也进入调用方使用的配置键，使工具选择或参数规则变更正常触发更新。更新依据为请求、影响工具选择的配置以及实际程序、探测所用头文件、specs 和相关搜索目录的路径／mtime／大小；命中不启动 xcrun、版本查询、预处理或探测编译。Darwin 记录选中的 developer／SDK 路径、SDK 版本和 deployment；Linux 记录 GCC 版本及实际预处理宏配置，不读取整个 GCC、cc1 或 assembler 内容来构造身份。Linux CBridge 合同中的 compiler inputs 摘要表示探测得到的编译配置；本机路径和文件 metadata 只进入构建更新键，不进入产物语义指纹。首次探测仍检查实际目标／libc 能力；失败不发布记录，普通输入缺失和工具执行失败正常报告。

C++ 配套 driver、标准库配置、宏及搜索输入保存在 `cxx-tools-v1`；只在实际 C++ 请求和相关输入变化时发现，不完整读取工具或标准库二进制来计算更新键。C++ 与 native 的本机输入 metadata 仅用于本机构建更新，不进入语言身份或 runtime ABI。工具版本与目标名称的文本不作为配对资格，不通过比较 `-dumpfullversion`／`-dumpmachine` 的字符串拒绝可用的编译器。

最终链接工具使用 `final-tools-v1` 普通发现记录，按已选 C 调用配置、最终链接选项（含实际生效的 unwind prefix）及 C++ 需求定位；保存实际 linker、版本、运行库选择、系统输入摘要、探测头文件与搜索候选，Darwin 同时保存已解析的 SDK 导出目录。输入未变化时直接复用发现结果，即使 Scoop 源码变化需要重新链接，也不重复 xcrun／版本／搜索目录查询、探测编译、探测链接或读取系统库内容。实际工具、SDK／系统输入、搜索候选或配置变化时重新发现；普通锁协调同键请求，失败不发布记录。SDK stub 的摘要和长度在首次读取时保存，不保留供身份重算或输入复制使用的完整原始字节；首次工具探测直接使用选中的 SDK 文件，实际程序链接仍按本次符号选择生成必要 stub。

首次 final-link 探测以实际编译／链接成功确认所选调用可用，使用 depfile 和 Linux linker trace 记录实际系统输入；不再读取探测 executable 重验目标、装载命令、段权限、EH 符号或 fixup，不生成 linker map 比对 unwind provider。目标／libc 由已有 C 工具选择确定，运行库由实际链接参数选择，输出行为由专门运行测试覆盖。

`scoop link` 与独立 `scoop-link` 同样复用 C／C++／最终链接工具发现记录；两者没有源码 Cone root，缓存位置依次使用 `--cache-dir`、`SCOOP_CACHE_DIR`、平台默认缓存目录。Windows 默认使用 `%LOCALAPPDATA%/Scoop/Cache`，Linux 使用 `$XDG_CACHE_HOME/scoop` 或 `$HOME/.cache/scoop`，Darwin 使用 `$HOME/Library/Caches/Scoop`；平台环境值只采用绝对路径，无可用默认值时要求显式缓存目录。显式 artifact 输入的读取和必要链接工作继续按实际请求执行。

native 预处理同时取得实际 include 名称及其所在文件，按该次 include 的名称、引号查找目录和配置搜索根记录候选；不把所有头文件的父目录互相交叉展开。预处理器输出的 include 观察指令不进入后续预处理单元编译，原有行号与诊断映射保持。

搜索目录不要求预先存在，不在预处理前 canonicalize 或核验每个 include 参数。缺失候选仍可作为普通更新观察，新增实际头文件后按原搜索顺序重新选择；实际文件读取失败由执行读取的工具报告。

用户 native 源码的普通对象结果保存在 `native-objects-v1`，记录实际选中的源码、上次预处理得到的头文件、include 搜索配置与相关候选路径、编译参数和 C／C++ 工具输入。记录命中且对象存在时直接返回对象路径，不预处理、不读头文件、不复制预处理结果或对象。更新时由构建编排调用 native 工具预处理并编译一次，全部成功后发布对象和记录；不同输入的对象使用不同位置。仅 Scoop 源码变化时复用 native 对象。更新判断观察实际头文件及可改变其选择的候选路径；不因同目录新增无关文件而重新预处理。parent 将这些对象交给 scoopc 打包，child 不再次发现 C++ 工具或编译同一 native 源码；独立 scoopc 仍从实际源码完成正常 native 编译。native 路径只用于输入选择与诊断，不比较 inode、审计符号链接来源或限制头文件的物理归属。头文件搜索和 forced include 接受含 `..` 的相对路径及绝对路径，使用普通 PathBuf 按 Cone 根解析，不套用 Scoop source identity 的 ConeRelativePath。

完成节点保存实际生成物位置和后续需要的摘要，不强制持有整份归档或多份验证结构。后续需要编译或链接时按需读取 section，命中库输出或最终 executable 时不读取产物。不同子进程各自首次读取所需依赖数据是正常工作，不增加驻留服务或共享内存协议来消除进程边界。

生产用途的 library 不要求 main；executable 必须恰有一个本 Cone 顶层 ordinary、非泛型、非 suspend 的 main，接受无参数或单个 `Array<String>` 参数，返回 Unit 或 Int，具体规则见语言规范 12.4.4。依赖的同名函数不参与入口选择，manifest 与 single-file mode 使用相同规则。测试用途仅适用于 manifest Cone，由前端选择本 Cone 的 generated TestEntry；其测试 `.slib` 是 executable root。请求机器产物时，scoopc 的正式产物是当前 `.slib`；仅文档请求按前述合同返回渲染输入。scoopc 不构建 runtime、不解析 native library 搜索目录、不调用最终 linker。

build/run 接受 `--profile debug|release`，默认 debug；`--release` 为 release 简写，与显式 profile 同时出现为参数错误。实际优化合同见 2.19。

`scoop --enable-gc-stress` 是可放在子命令前后的全局构建选项，对 build/run/test/link 的最终 runtime 选择生效，独立于 debug/release；低层 scoop-link 提供同名选项。默认选择不含 stress 支持的 runtime，启用后选择含 stress 能力的版本，再由运行时环境变量控制开关（运行时规范第 3 章）。该选择不改变 Scoop 源码编译或普通库 `.slib` 的身份、profile 和缓存；只影响 runtime 对象及最终链接结果。显式 `--runtime-objects` 的能力必须与请求相符。

默认 target root 为 manifest Cone root 下的 target，SingleFile 为调用者 cwd 下的 target。`--target-dir` 相对调用者 cwd 解析并替换该根；输出目录追加 `<canonical-target-triple>/<profile>`。library 文件名为 `<cone-name>.slib`，manifest executable 为 `<cone-name>`，single-file 为 `<sanitized-input-stem>`。build 的 `-o` 覆盖最终 root 文件的完整路径，不追加 triple/profile。

Windows target 的默认 executable 文件名（包括测试程序）在上述名称后追加 `.exe`；最终链接缓存也保存带 `.exe` 后缀的可执行文件。显式 `-o` 路径保持原样。该选择依据构建 target，不依据宿主系统。

Windows 通过 `--debug-info line-tables|full` 或产物链接的 `--emit-debug-file` 生成并发布 `<output>.pdb`；`--keep-symbols` 独立控制普通符号。PE 的 CodeView 路径保存公开 PDB basename，缓存链接不得泄漏临时 PDB 名称。此 basename 参与最终 executable 键；PDB 使用与 Darwin dSYM／ELF debug file 相同的普通更新及发布通路，命中只要求所需文件存在，不读取内容。未请求调试文件时不生成或要求 PDB。

Windows 的 `--debug-info line-tables|full` 使用 CodeView，与优化 profile 独立；debug 默认 full，release 默认 none。函数的显示名称、实际声明文件和位置由 HIR 经 MIR／LIR 传递，LLVM subprogram 继续以实际机器符号关联函数；位置复用当前已读取源码或依赖的 source record，不在 codegen 重新读取源码。当前源码使用本次输入的文件定位路径，独立于 `SourceLocation.file` 的包坐标显示名称；该宿主路径不进入持久化 SourceRecord 或实体身份。只保留 source record 的依赖泛型使用原始逻辑文件路径和位置，不猜测提供方的宿主目录。没有源码声明的辅助函数不伪造文件或位置。函数入口的声明位置与正文各语句的位置是不同信息，只有已传递的实际位置才能生成行记录；不得把整段正文标成声明所在行。调试 metadata 不参与实体身份、ABI 或 GC／EH 根信息，release 的必要展开表与 stackmap 保持原合同。

用户 Cone 的默认构建缓存位于 `<target-root>/<canonical-target-triple>/build/<profile>`，包含本次图的源码依赖、工具发现、中间产物和最终链接缓存，不使用用户级平台缓存目录。build/run/test 与 check 使用本次选择的 profile；doc 使用 debug，并以首个显式 root（省略时为当前 Cone）选择默认缓存 target root。支持 `--target-dir` 的命令同时将默认缓存放到指定 target root 下，`-o`／`--output-dir` 不改变缓存位置。显式 `--cache-dir` 优先，其次为 `SCOOP_CACHE_DIR`，两者均直接指定完整缓存根，不再追加 triple/profile；未指定时才使用上述 Cone 本地布局。默认缓存选择只定位输入和目标，不读取源码内容或探测工具。

`scoop clean`接受与build相同的manifest目录、Cone.toml或单文件输入；M63中manifest/workspace模式通过PM的本地成员发现定位workspace target根，不求解版本或获取依赖，单文件保持独立target根。它递归删除整个target目录（包括所有target、profile、缓存和目录内其他生成物），目录不存在时成功返回；`--target-dir`相对cwd解析并直接指定被清理的根，此时不依赖项目清单或PM，因此损坏的项目配置仍可显式清理。不读取源码/sysroot、启动编译器或探测native工具；target根外的显式输出和缓存不属于清理范围，Cone.lock与全局PM源码缓存不因项目clean被删除。实际删除失败按路径报告并退出1；参数错误退出2。human成功摘要或schema 1、kind为`clean`、含`target_dir`与`removed`的JSON结果写stderr，stdout保持为空。

新结果在临时输出位置生成，工具成功后 rename 发布，再更新构建记录；失败不发布部分结果。普通构建锁协调相同结果的生产，取得锁后重查记录即可复用，不比较两份完整产物。不同输入的结果不得覆盖正在使用的同一缓存输出。用户首次指定另一输出位置时可发布一次，此后命中不重读、哈希或覆盖已有文件。不进行写后读回、sync_all、目录同步、只读封存、私有目录或路径来源审计。

最终程序使用 `executables-v4` 普通构建记录：键采用当前图的 Code 结果、runtime 构建结果、target、优化模式、最终链接选项、调试文件种类及所需 executable basename、library roots 和实际 Scoop／C 工具输入及生效调用配置。记录保存稳定 executable、可选独立调试文件路径、link plan 摘要，以及上次真实链接使用的工具、系统输入、库文件、脚本和会影响库选择的候选路径 metadata。缺失候选也参与正常更新判断；新增候选、symlink 选择变化或实际库更新时重新解析并链接。命中判断先于 Link 闭包读取、final 工具发现、startup／response file 生成和对象物化；不读取命中 executable 内容。所需 executable 或已请求的独立调试文件缺失时更新；用户输出或已请求的独立调试文件缺失，或选择另一相同 basename 的输出位置时，只发布已有结果，不重新链接。改变带调试文件的 executable basename 时正常重新链接，以保持 ELF debuglink 与平台调试文件关联。编译或链接失败不发布新记录；同键构建使用普通锁，锁内重查。

用户 library／executable 输出共用按结果标识保存的发布记录，命中且文件存在时直接返回。发布通过临时文件与 rename 完成，只在需要新位置或新结果时复制一次；不检查输入／输出 inode、owner、符号链接来源或缓存目录归属，不制作 execution copy，也不同步落盘。run／test 使用本次结果的稳定缓存路径，argv[0] 保留用户输出位置；另一构建并发发布不同结果不会改变已选执行目标。普通文件创建、复制、rename 和工具执行错误照常报告。

run 直接执行本次成功构建确定的文件；同一请求构建失败时不得执行旧结果。不通过私有二进制副本防御外部文件替换。argv[0] 使用稳定输出路径，其余参数原样传递；带参数的 Scoop main 得到包括该第 0 项的完整数组，与 Java/Kotlin 从首个用户参数开始的数组不同。继承调用者 cwd、environment 和 stdio，保留操作系统实际报告的 exit/signal，不因源级 Int 超出 `0..255` 而拒绝运行，也不把 main 的非零返回值改写成 gateway 失败。工具输出在程序启动前完成，程序 stderr 不包装为诊断 JSON。

`scoop link` 接受显式 root/dependency artifacts、cone-path、可选 runtime object index、target、library search roots 与输出路径。它按 artifact exact dependencies 查找，core 也只从已有 artifact 取得；不启动 scoopc、不读取 runtime/Scoop 源码或补建缺失依赖。

`--sysroot` 定位 Scoop core 及上述 `scoop` group 默认产物；Linux 的 `--cc`、`--native-sysroot` 选择 C 工具链。`--unwind-prefix` 和 `--link-mode static|dynamic` 属于 runtime/executable 链接输入，library `.slib` 构建不要求 unwind archive。Linux 缺省 unwind prefix 为 `<Scoop sysroot>/lib/<canonical triple>/unwind`。Darwin 使用所选 Xcode 开发环境。

三个宿主命令按显式 `--sysroot`、`SCOOP_SYSROOT`、实际执行文件所属 `bin` 的父目录依次选择 Scoop sysroot。默认定位跟随执行文件的普通 symlink，不依赖 cwd 或编译时仓库路径；显式相对路径仍按调用者 cwd 解析。开发工具从 Cargo 输出目录执行时也必须显式选择开发 sysroot。配套 scoopc 默认取实际 scoop 执行文件的相邻位置，显式 sysroot 不改变已选宿主命令。Windows 上默认配套命令和内置 generator 使用 `.exe` 文件名；显式工具路径保持原样。

因此将工具包解压到 `~/.scoop` 并从 `~/.scoop/bin` 启动时，默认系统库位置就是 `~/.scoop/lib/<canonical-target-triple>`；debug／release 应用及空缓存均直接消费其中的预编译 `.slib`。随包 `src` 不参与默认构建，显式 source locator 或直接构建库源码才请求重编。安装到其他位置仍沿同一相邻 sysroot 规则，不额外混用 HOME 下另一套库。

直接 scoopc 与 scoop 的用户 native 源码编译使用所选 sysroot 的 `include` 提供公开 Scoop C 头，实际头文件沿普通 depfile 更新；不默认读取仓库 runtime 头文件目录。parent 已提供 native 对象的 protocol 请求直接使用这些对象，不另行定位头文件或编译相同源码。

当前 Cone 的 `native.cxx = true` 要求解析同一 target 的配套 C++ driver：GNU 使用对应工具链的 `g++`，Darwin 使用同一 Xcode 工具链的 `clang++`。选中的 C++ 源码交给该 driver，`.c`、generated-C bridge、runtime 和生成的启动 C 代码仍按各自 C 编译配置生成对象。未启用 `cxx` 却选中 C++ 源码、启用后缺少配套 driver／运行库、启用后 target 为 musl，均在调用 native 编译器前诊断；不回退到宿主 C++ 工具链。

Windows native C/C++ driver 单独选择：Microsoft 默认使用宿主 LLVM 的 clang-cl，也接受显式 cl；MinGW 使用 Clang／Clang++ 与所选 prefix 的 libc++。`--cc` 选择用户 native C driver，`--cxx` 可选择相容 C++ driver；Microsoft 未显式选择 C++ 时沿用选中 C driver，MinGW 沿配套位置选 Clang++。generated-C 优先使用安装根 libexec/llvm/bin 的 Clang；直接运行开发输出时允许从 PATH 定位宿主 Clang，不将其 builtin 交给 cl。实际 header/object probe 检查 Win64、LLP64、CRT、MSVC STL 或 libc++，无须执行程序；C++ probe 包含 RTTI、标准库和内部异常。路径只进入宿主调用及普通更新记录，未使用的 C++ 配置不探测。交叉宿主的源码、对象与库路径必须作为文件参数传递，不能让 clang-cl 将 `/Users/...` 等 POSIX 路径解释为 Microsoft 编译选项。

Windows 最终链接的 `--linker` 选择实际 linker：Microsoft 环境默认宿主配套 lld-link，也可显式选择 link.exe；MinGW 使用配套 GNU 接口的 LLD。Microsoft 经 clang-cl driver 传递已选 SDK／toolset 的库目录与正常 CRT defaults，native 对象可来自 cl 或 clang-cl，C++ producer 的选择不改变此链接调用。MinGW 经目标 prefix 的 Clang driver 选择 startup、compiler-rt 与 libc++。三个环境均使用 Unicode wmain startup，动态依赖其已选 CRT；辅助 C++／Scoop unwind archive 可静态加入，不能借此切换 CRT。`--link-mode static` 不适用于首期 Windows `/MD` 资源；默认或 dynamic 使用同一配套。Scoop 的 LLVM unwind archive 始终取已选 `<unwind-prefix>/lib/libunwind.a`，C++ 模式也不替换为另一套 `_Unwind_*` 实现。

Windows final-link 首次使用实际 header/object/link probe 验证 PE32+ AMD64、selected CRT imports、SEH 与所需 C++ 运行库；交叉宿主无需执行目标程序。工具、库搜索目录、实际 trace 输入与 unwind archive 进入普通更新记录，命中时不重新探测。最终 PE 使用 ASLR、NX、REF 裁剪和禁止 ICF；COMDAT 关联保留被选代码的 GC／EH metadata。请求调试信息／调试文件时输出相邻 `<output>.pdb`，沿用已替换文本 map 的统一伴随文件接口；PDB 使用相邻文件名定位，随 executable 一起纳入结果存在性与发布，不把临时目录写入其部署合同。

Windows generated-C bridge 的主定义拥有 `WindowsUnwindTable`／`WindowsUnwindInfo` 关联 atom，对应 `.pdata`／`.xdata`，不投影 Unix `EhFrame`。桥编译保留包含叶函数的 frame pointer，使独立桥单元始终有完整的 SEH 表与稳定边界；这些表与桥正文按 COMDAT 关联一起选择和裁剪。该要求独立于优化及调试级别。

Windows executable 在 PE header 中预留 8 MiB 线程栈、初始提交 4 KiB，后续由系统按需提交；runtime 以默认大小创建的线程沿用该设置。该预留满足 debug 大值类型参数和深层调用的实际栈需求，不保证任意递归深度，也不改变语言语义、ABI 或 stackmap 格式。Microsoft LINK／LLD 与 MinGW LLD 使用相同值，链接配置参与现有更新判断。

Windows 10 是受支持 OS／API 基线，不要求把每种 CRT 的 PE subsystem version 改成 10.0。MinGW 保留配套 CRT 的默认 PE subsystem version；当前 mingw-w64 load configuration 未提供 Windows 6.3+ subsystem 所需的 security cookie，强行改版本会被 loader 以 `STATUS_INVALID_IMAGE_FORMAT` 拒绝。该处理不降低所声明的 Windows 10 运行基线，也不由 Scoop 改写 CRT 的 load configuration。

Microsoft native 默认 `/MD`、C11／C++20、`/EHsc`、UTF-8 源码／执行字符集与符合标准的预处理器；debug 优化不切换 debug CRT。cl 使用 `/E` 与 `/sourceDependencies` JSON 收集实际头文件，clang-cl 使用其支持的 Clang depfile 参数；二者都保留诊断行标记。默认 CRT／target、输入／输出、依赖与阶段参数由 driver 管理；不支持的 native include 模式得到明确诊断，不悄悄改变搜索语义。MSVC flags 中 `/I`、`/FI`、`/D`、`/U` 按既有 include／macro 规则处理。普通用户 flags 不得切换 `/MT`、`/MDd`、LTCG 或其他受 driver 管理的配置。

`c_flags`／`cxx_flags` 只作用于当前 Cone 的相应 native 源码，按 argv 元素与声明顺序传递。native 默认语言标准分别为 C11 和 C++20，可由对应 flags 中的 `-std` 覆盖。`cxx` 和实际生效的编译参数、C++ 源码及头文件、配套编译器与标准库 ABI 配置进入构建输入和 compile key。library 构建把 C++ 链接及运行库需求写入 `.slib` 的逻辑 native requirements，不执行最终 C++ 链接。

Darwin native `.m` 按语言规范 12.2.1 使用所选 Apple Clang，预处理输入语言为 Objective-C，编译输入为对应预处理结果；两步均使用 ARC。`objc_flags` 与 C／C++ flags 分开传递和记录，只在实际选中 Objective-C 源码时进入 native 对象配置；实际头文件、目标、SDK 与工具输入沿普通 native 更新规则处理。条件筛选先于 `.m` 的目标限制，非 Darwin 选中时在预处理前诊断。ARC、Objective-C runtime ABI 与输入语言由 driver 管理；`.mm` 不支持。对象按已有 native member 打包，类、selector、ARC 引用和 framework 使用普通 Mach-O 符号解析与最终链接，不增加窗口专用对象种类或 linker 入口。

Apple Clang native 对象中的 `_objc_msgSend$<selector>` 未定义引用由 Darwin linker 合成 selector stub；Scoop 将实际 `_objc_msgSend` 作为其运行库需求解析，不能把合成 stub 伪装成 SDK 的动态 export。已有 native 定义和显式源码 extern 仍走通常的定义／库归属检查。该规则仅适用于 Mach-O 的非空 selector，不改变其他 target 的符号解析。

LIR foundation 以必需的 Boolean `native_cxx` 保存当前 Cone 的显式 C++ 运行库需求；该字段与普通 native library 表一起进入 Code。foundation wire 为 8；production/8 不再复制这一数据，读取方采用同一份 foundation。需求不依赖是否选中 C++ 文件，也不从机器符号推断。C++ 编译器沿已选择 C driver 的配套位置解析，保留相同 target、sysroot 和环境；以实际目标 headers、对象和链接配置判断相容性，不要求 C/C++ driver 的完整版本文本相等；缺失配套 driver、标准库或 ABI 配置直接诊断。

native 与 runtime 的真实编译保存源码、depfile 中实际使用的头文件（含 scoop_rt.h）、include 配置、工具输入及影响 include 选择的目录状态。后续先检查这些输入的 metadata；全部未变化时不调用预处理器、不读取全部头文件。增加、删除或改变 include 搜索输入后重新发现并编译必要部分；所选 target、宏、SDK、C/C++ 配置与诊断位置沿正常工具调用传递。不为构建中外部替换输入建立稳定性检测、重试或不可变预处理镜像。

全局构建缓存由 scoop 管理。同一工作区、Cone、Production/Test、target、profile 可直接定位普通构建记录；本机路径可参与记录定位，不进入语言身份。记录保存已解析的有效配置、选中输入的逻辑名／修改时间／大小、工具与协议版本、实际依赖输入键／必要语义摘要、生成物及用户输出位置。更新判定先于源码读取、输入复制、预处理、能力查询和链接准备。输入内容在确需编译时读一次，供 parser 与必要语义计算共用。

PM的全局源码/解析缓存与编译缓存职责分离，默认布局和覆盖参数见[包管理规范11.1](SCOOP-PACKAGE-SPEC.md#111-跨项目共享目录)。多个项目可直接引用同一registry版本的解包目录或同一Git commit的checkout，不为每个项目复制依赖源码。同Cone在不同精确依赖绑定、target、profile或工具链下的编译结果仍分别判断更新；共享下载不能被当作共享编译结果的充分条件。

记录不存在、缓存格式改变、有效输入变化、所需输出不存在或请求的观察结果尚未产生时更新相应结果。输入未变化且结果存在时直接采用记录；不核对 artifact/member/object hash、warning 来源、收据或指纹自洽性，不检测、修复、恢复或隔离外部破坏的缓存。记录按 schema 普通解码，实际读取错误直接传播；不通过扫描或重建来取得另一份更可信记录。旧缓存用新版本或 namespace 自然失效，不解析或迁移旧证明链。

输入缺失是普通源码构建错误，输出缺失只生成该输出。manifest 仅测试配置变化不使生产节点失效；未选中源码、程序 argv、测试过滤及显示方式不使编译失效。工具变化可以直接失效，不为恢复内容可能相同的命中强制读取整个程序。尚未使用的 generator 配置不影响构建；实际使用的工具按同一规则记录。

machine transport 为一次 request/response 的 length-prefixed canonical CBOR，protocol version 为 24（展开 native 配置加入 `objc_flags`；合并 full 静态变量调试级别、Windows C++ 工具 locator 与编译期环境、文件及目录输入；展开配置保留原 Cone 根定位；当前 Cone 采用已展开配置，已有 Check、观察、生成器配置、工具输入和片段诊断原文保持），stdout 只含协议 frame。build request field 6 为输出选择：Artifact、Documentation、ArtifactWithDocumentation，保存各自实际输出 locator；Check 使用新 tag 4 的单字段 map，只保存 tag、不含 locator；成功 response 的 field 2 使用对应 product 选择，Artifact 与 ArtifactWithDocumentation 变体携带 artifact summary，文档输出保存渲染输入 locator 和独立文档 warning；Check product 同样使用 tag 4 的单字段 map，warning 与实际 generator 输入沿共有成功字段保存。Check 拒绝 dump、observation-only 与非空 native 对象输入。build request field 9 为 OptimizationMode（Debug=1、Release=2），field 10 为按归一化路径排序的选中 native 源码所对应的已编译 native 对象 locator 数组，机器生产时数量必须与 manifest 的实际 target 选择一致；没有 native 源码或仅文档时为空。field 11 为当前 Cone 的 CompilationPurpose（Production=1、Test=2）；field 12 为按名称排序且唯一的 generator 注册数组，保存名称与已按接收 CLI cwd 解析的绝对宿主程序路径，未使用的注册不探测程序。build request field 13 为 DebugInfoLevel（none=0、line-tables=1、full=2）；和 field 9 的优化模式独立，生产级别进入源码 Cone 的编译更新键。target request 保存 canonical triple，以及待解析的 C driver／native sysroot 选择或已解析的 C 工具调用记录。parent 将已得到的记录直接交给 child，child 解码后用于实际编译，不重复执行探测；独立 scoopc 请求仍执行正常工具发现。调用记录保存普通版本／deployment 配置、host locator 和本机输入摘要，不是产物验证资格，host locator 不进入 artifact identity。旧或不匹配协议拒绝。request envelope field 5 为 observation-only 标志（unsigned 0／1）；为 true 时必须请求非空 dump 集合，沿同一编译实现执行到最后一个所需阶段，不发布机器对象或 `.slib`。观察成功使用 response tag 3，携带 warning 和 dump 描述，不伪造产物摘要。

成功响应直接携带生产时得到的产物摘要：Cone、直接依赖、target、产物 profile、各语义指纹、artifact fingerprint 及测试目录，并携带 warning 和 dump 描述。parent 将该摘要与产物实际位置保存为普通构建记录，不重新打开 child 产物、重读摘要或逐字段核对响应与归档。摘要是后续编译、链接及测试列举需要的数据，不是验证资格；不得包含无人消费的归档长度或成员数来支持复核。protocol 8 用此摘要替换原独立的七个指纹字段。

protocol 19 的 current 输入为 ExpandedCone（tag 3）或 SingleFile（tag 2）；旧 ManifestRoot tag 1 不再接受。ExpandedCone 的 field 1 保存包管理规范 7.1 的 ExpandedConeInputV1 UTF-8 JSON 字节，沿用现有 CBOR frame，不生成临时清单或源码树。parent 交付已绑定依赖的生产/测试配置、原始清单诊断文本及 host 路径；child 只解码必要格式与引用，不重新读取或解释原始清单。native 准备同样复用 parent 已加载的配置。独立 scoop-pm 的 JSON prepare 协议由包管理规范 7 定义；两种协议各自检查版本，不扩展成可协商任意构建任务的服务协议。

本机编译结果保存在 `compile-v3`：按输入键保存普通 `record.cbor`（生产摘要、warning 与实际 generator 工具输入）。成功 response 的 field 5 返回实际调用的名称、所选程序路径、generator 协议版本和程序修改时间／大小；v1 没有额外生成器选项，程序更新状态表示实际工具版本，不额外启动版本探测。命中先按已用名称比较当前绑定和程序状态，未使用的注册不参与输入键或失效；缺失／变化使该 Cone 正常重编译。默认 scoop.codec 程序相对于实际选中的 scoopc 安装位置解析。

同一输入键下的不同 generator 结果按生产摘要中已有的 artifact fingerprint 分目录保存 `artifact.slib` 与其 dumps，使新绑定产生的结果不覆盖仍被其他构建使用的路径。记录只指向最近完成的结果，旧版本路径不在发布时删除；这仍是普通编译缓存，不另建 generator 缓存。命中只读记录、比较实际输入并检查所需输出是否存在；输出数据在真正的编译／链接消费者需要时才读取。编译成功后的输出经 rename 发布，记录普通原子替换，不读回、同步落盘或比较并发产物。取得构建锁后重查输入与结果即可复用。库的用户输出在目标位置旁保存所对应结果的普通记录；未变化且文件存在时不读回或重新复制。相同位置改为可执行文件时废止此前库输出记录，不使下一次库构建误用其他构建结果。

仅驻留内存的生成片段在 machine 诊断来源中携带对应原文、普通 Cone／逻辑路径与 byte span（origin tag 6），主位置和 notes 使用相同表示。parent 沿共同源码坐标规则显示实际行列，无需猜测磁盘路径；缓存中的生成 warning 也保留该来源。普通磁盘来源继续使用已有表示，片段原文不因此加入 `.slib`。

generator 的 stderr 沿 driver 日志目标输出。scoop 在取得合法的 child 响应与对应退出状态后透传这些日志，不因存在日志将成功响应改为失败；错误响应、请求编号和 stdout frame 检查保持。日志不是结构化编译诊断，即使选择 JSON 消息格式也保留其原始 stderr 字节。

显式观察使用 `--emit ast|hir|mir|optimizations|lir|symbols|all --dump-dir dir [--dump-scope root|sources]`，默认 root；sources 覆盖源码节点，不从 prebuilt 反造 AST 或 LocalConcreteHir。HIR dump 同时包含 Export、LocalConcrete 与共有跨 Cone 接口。当前输入已有相应 dump 时直接复用；缺失时只执行产生该观察所需的前置阶段，不重做仍有效的机器对象、归档或最终链接。源码变化使观察记录失效，诊断使用当前定位；观察失败不能报告成功。dump 文件保存在对应编译输入键的缓存目录，命中后按请求复制到输出目录；只为缺失阶段调用 observation-only child。dump 描述只保存阶段及文件位置，不保存或复核内容摘要，不要求私有空目录、来源隔离或持久化同步屏障。

`optimizations` 在 MIR 优化完成后产生 `optimizations.json`；`all` 按 AST、HIR、MIR、optimizations、LIR、symbols 顺序输出。既有 stage wire tag 1／2／3／4 保持，新观察使用 tag 5，wire tag 不决定流水线顺序。该 JSON 是本机观察输出，schema 为 1，不写入 `.slib`、语言身份、ABI、Code 指纹或优化输入。不开启时不收集逐项决策；开启与关闭必须得到相同的 MIR、机器代码与程序行为。仅请求该观察时沿既有 observation-only 路径停在 MIR，不生成 LIR 或机器产物；命中观察缓存直接复制已有文件。文档模式仍只允许 AST／HIR。

首批报告覆盖普通 MIR 内联与局部 box／closure／class 分配消除，保存实际迭代轮次、收敛状态、启用的 pass，以及按发生顺序记录的决策。内联报告采用选择器实际使用的执行／体积成本、阈值与剩余增长，区分普通估价、局部对象字段替换共同估价、必需展开及无法选择的原因；分配报告采用现有来源、循环、支配与使用分析，区分消除和保留原因，不重新执行优化或另建逃逸分析。debug 未启用的普通 pass 不伪造失败事件。重复轮次或同轮重新选择可再次观察同一调用，记录表示当时的决定，不承诺一条源码对应一条最终事件。

决策以 MIR 的 typed callable、精确类型及现有 materialization／implementation 身份关联函数，显示名只服务阅读。位置保存观察时的函数、block、statement、源 span 与已有内联来源；这些临时 MIR 坐标不作为跨构建持久 id，也不声称对应最终 MIR 行号。driver 可复用当前 Cone 与已加载依赖的 HIR declaration origin 补充声明源码身份，缺失时记为 null，不猜测文件或读取依赖源码来补全位置。报告不是编译 warning、不进入 child stderr，也不是 PGO profile；采样、调试信息与后续其他 pass 的诊断按实际消费者另行扩展。

`symbols` 在 LIR 完成后产生 `symbols.json`，使用 stage wire tag 6、JSON schema 1。它以实际 callable body 的 typed identity 和既有 symbol 编码建立函数级采样归因，保存逻辑机器符号、物理 producer、implementation／实例身份、可用的声明显示名、完整 exact signature 及声明来源。当前 Cone 的普通函数及生成正文、release hook、入口／初始化 gateway 均按实际角色记录；同时复用本次已经加载的依赖 LIR callable-body、MIR signature 与 HIR declaration 元数据，覆盖预编译库中的现有正文，不重新读取依赖源码或产物、不重放语义验证。泛型实例与 adapter 保留各自身份，不按名称合并；可读名仅用于显示，缺失名称或声明来源记为 null。

该观察是可供 `sample`／`perf` 原始文本按确切符号注释的旁置映射，不是最终链接存活符号清单，也不提供地址范围或指令行表。声明 byte span 不得当作采样 PC 的执行行号；LLVM 内联／克隆、native／runtime 正文及无法识别的符号保留原始采样信息，不推测内联调用栈。Darwin 工具展示的单个目标前缀下划线可按目标符号规则处理，原符号、地址偏移和采样权重不得因可读名相同而合并。采样工具选择了定义边界起始符号时，旁置工具可读取显式给定二进制的 LLVM nm 结果，仅将与一个已知 callable body 唯一同址的实际边界符号关联到该正文；多个正文同址时保留歧义，不猜测身份。macOS sample 将同址别名折叠为 `<deduplicated_symbol>` 时，旁置工具可显式接收本次构建的 Mach-O 程序，用报告的主镜像加载地址及 LLVM readobj 的实际 __TEXT 地址消除 ASLR，再批量调用 LLVM symbolizer。只为明确列出的、位于该主镜像内的 PC 附加已识别符号；保留原帧、折叠计数及省略号，不把多个地址或省略的样本重新分配到函数。采样仍由成熟平台工具完成，不引入插桩、PGO 输入、通用事件格式或运行时查询。

最终程序移除普通内部符号后，旁置工具使用 llvm-symbolizer 读取 executable 与独立调试文件，按精确 linkage name 关联同次 symbols 观察，同时保留 DWARF 行号与 inline 链。Darwin sample 用程序 `__TEXT` 与报告主镜像加载地址计算 ASLR slide；Linux PIE 使用实际映射的 load bias，不能直接把运行地址当作链接地址。没有信息的地址保留未知，不按可读名称合并重载／泛型实例，不推测内联帧。

所有 profile 可显式请求 symbols，未请求时不收集或渲染该映射。它不进入 `.slib`、语言身份、ABI、Code 或优化输入，不改变机器代码、GC roots 或程序行为；仅请求缺失的 symbols 时沿 observation-only 路径停在 LIR，已有机器产物和最终链接继续复用。缓存命中仅复制已有观察。文档模式只允许 AST／HIR 的约定保持。

诊断具有明确 Error/Warning severity，primary 与 notes 保存实际 Cone、logical source 和 byte span。源码读取等文件级错误仅有实际 host path、没有字节区间时，使用独立的 HostPath origin（wire tag 5），不得丢失路径或伪造区间；已有 HostPathSpan 编码保持。产物错误保存 container/manifest/member/section 与 wire field path，I/O 错误不伪装成源码位置。human 与逐行 JSON 使用同一结构化记录，写 stderr；stdout 留给被执行程序。缓存中的 warning 在本次显示时附加 locator，不丢失原位置。显示源码诊断时仅按需读取涉及的文件以计算行列，同一轮显示共享读取结果；此读取不参与缓存更新判定。

工具中断停止后续构建并回收正在运行的 child。Unix 向 child 传递原信号并保留真实 signal；Windows 处理 Ctrl+C／Ctrl+Break／console close，以持有的 process handle 终止尚在运行的 child，使用 `STATUS_CONTROL_C_EXIT`，不将它解释为 POSIX signal。Windows 外部程序的普通退出保留完整 32-bit 状态。某节点失败后停止派发新节点，不启动 dependent 或 program-link，并等待已启动的 child 收尾；失败节点不发布结果，已经成功的节点继续遵循普通缓存规则。已收集失败按 canonical 顺序选择报告，不能因为完成时间不同而重排同一组诊断。

### 2.8 程序链接与最终结果复用

Windows executable 的 subsystem 由根 Cone 的 `[windows].subsystem` 决定，默认 console；单文件与测试 executable 使用 console。manifest 的 Cone record 可带 field 5 的 Windows subsystem（Console=1、Gui=2），仅 executable 使用；无该字段的现有产物按 console 解释，library 与非 Windows producer 不写该字段。此配置不改变 Cone／实体 identity 或语言 ABI，保存在 executable `.slib` 供 artifact-only 链接读取，并作为最终链接的实际输入参与更新与链接指纹。GUI 链接使用 PE Windows subsystem；Microsoft 显式选择 Unicode `wmainCRTStartup`，MinGW 使用配套 Unicode CRT startup。两者均由生成的 `wmain` 进入同一 Scoop startup，保持 CRT 初始化／终止与 argv 处理。

Windows 资源使用配套或外部发现的 LLVM `llvm-windres`（LLVM RC 的 windres 入口）和 `llvm-cvtres`。资源工具只在实际选择 RC 且更新记录未命中后发现／调用，预处理器显式使用已选 target 的 Clang 与环境。RC 编译生成 `.res`，再转换为 AMD64 COFF，按 RC 的归一化 native source 路径作为 `NativeLinkObject` 保存在 executable `.slib`；资源内容进入 Code 与最终链接更新键，不另增加 runtime ABI。现有 native 对象通道同时传递 C/C++ 对象与资源对象，资源对象排在已排序的 C/C++ 对象之后。artifact-only 链接直接消费该对象，不需要 RC 源码或资源工具。

RC 更新记录使用预处理器的实际头文件依赖，以及 LLVM RC 对实际文件资源的解析结果，记录源文件、包含文件、引用资源、搜索候选与工具的普通 metadata；未变化命中不重新预处理、解析或编译。资源文件按资源编译器的实际搜索顺序定位：绝对路径直接使用，否则依次为工作目录（Cone 根）、RC 父目录、显式 include 目录。工具环境不继承额外的 INCLUDE 搜索路径；所选 SDK／MinGW headers 通过目标配置提供。资源编译和已有 native 编译共用更新记录与失败不发布规则，不重新实现 RC 语法或资源编码。

program-link 消费完整 `.slib` Link 闭包、普通 runtime 对象、已有 native 文件、LIR target 与 final-link 配置。它不展开模板、重新编译 Scoop 或编译用户 C/C++；只可用明确选择的 C compiler 生成本次固定启动代码与最终 image tables。

`scoop --enable-startup-stats` 和独立 `scoop-link --enable-startup-stats` 按运行时规范第 7 章选择编入启动诊断的 runtime，对 build/run/test/link 生效，独立于 profile 和 GC stress。默认 runtime 必须在预处理阶段排除启动计时、报告及专用数据访问；环境变量不能启用未编入的代码。该开关只影响 runtime 构建／选择及最终链接结果，不改变 Scoop 源码编译缓存、`.slib`、对象布局或 ABI；编译器不消费测量记录。显式 runtime index 的配置必须与请求一致。

运行时规范 3.9 的搬迁覆盖登记随既有 GC stress 构建能力或显式 heap 验证编入。普通 runtime 不生成逐存活对象的 claim 调用、覆盖验证函数及专用 bitmap 生命周期操作，启动诊断开关本身不启用这项验证。不同配置仅改变 collector 私有侧元数据，所有 profile 继续使用同一对象／根 ABI；无须重建未变化的 Scoop `.slib`，重建 runtime 并重新链接即可消费该优化。

root 是唯一 executable，依赖为 library。正常构建使用已确定的依赖结果、目标及链接配置，显式 link 读取所需 Link 输入。不以重算三层语义指纹、整包内容或重复生产校验来授予链接资格。

最终 executable 是缓存结果。先根据 Cone 代码结果、runtime/native 输入、库选择、工具与链接配置判断更新；输入未变化且输出存在时直接返回，不能调用 read_program_link_closure、生成 startup/image tables/response file、调用 C compiler/linker 或复制／读取 executable 内容。只改变 native、runtime 或链接配置时不重编译无关 Scoop Cone。库搜索记录影响实际选择的目录状态，新增优先候选或配置改变后重新选择。

program-link 从完整产物闭包合并 C++ 链接和运行库需求；任一 Cone 要求 C++，就采用所选 target 的 C++ final-link 配置。Linux GNU 由配套 `g++` 驱动并使用 `libstdc++`，Darwin 由配套 `clang++` 驱动并使用 `libc++`；同时纳入各自的 C++ ABI 运行库与系统依赖。Windows 使用前述完整 target 的 final-link profile，由 Microsoft CRT defaults 或 MinGW Clang++ 配套参数加入所需 STL／ABI 运行库，不生成 Unix 的 `c++`／`stdc++` 动态库 requirement；各 Cone 显式声明的 native libraries 仍照常汇总。C++ driver 调用相应平台 linker，既有对象格式、GC、EH 与 image 契约继续适用。不能仅把 C 编译器替换为 C++ 编译器后继续原样使用抑制其默认运行库的 C 链接参数，也不能靠 root 重复声明 `cxx` 才满足依赖。

musl 闭包出现 C++ requirement 时，在解析 C++ 工具链或调用最终 linker 前报错，指出要求 C++ 的 Cone；不因其源码已预编译而绕过限制。实际使用的 C++ driver、标准库／ABI 运行库、unwind provider 及链接参数进入 final-link 配置和 ResolvedLinkPlan。

首次生产将对象保存在 `objects-v1/<member-content-digest>.o` 稳定缓存位置，后续链接直接传递对象路径；不每次从 slib 解包、改写、哈希或重新物化。文件名使用生成成员时已有的内容摘要，不能为查缓存重新哈希或读取已有对象。归档 writer 保存所写对象的字节范围与摘要，发布时借用同一份归档缓冲区，不另存对象字节副本。scoopc 使用 `SCOOP_CACHE_DIR` 或平台默认缓存位置，parent 把当前构建缓存位置传入 child；独立链接使用自身选中的缓存。缺失所需对象时从本次已有对象字节补产，临时文件写入成功后原子发布；并发已有结果直接采用，不比较内容或同步落盘。显式输入首次缺少独立对象文件时按需生成并记录，后续复用；已有的普通 native 独立对象直接使用原文件路径，归档内选中成员才需要独立缓存文件。供 native 工具使用的对象路径按当前工作目录解析为绝对路径，避免工具切换目录改变相对缓存路径的含义。同一进程按需入口共享已读 section，diagnostic、opaque、unknown optional 成员不是链接对象；无法处理的实际 Link-required 格式仍报告错误。

Strong 由原 provider 提供。ODR 在普通依赖语义一致后按完整 group/member key 与 shared ABI 合并，物理选择见 2.13、2.19。全部 typed imports 和实际 relocation 必须闭合；metadata-only 查询可以没有机器引用。

SourceExtern 按实际 target/native symbol 合并完整 library、kind、TLS、mutability、ABI、calling convention、GC effect 与投影后的 native signature/storage。任一字段冲突均报错并指出来源；随后从实际 native 对象或 provider export 解析定义。`captureErrno`、包装后的 Scoop tuple 结果与 NativeSafe/GcLeaf 是声明的调用行为，不进入 native ABI 合并键；返回 `R` 与捕获后返回 `(R, Int)` 的声明可共用同一 native 定义，Scoop callable 和所需 bridge 仍按各自合同保留。core 与普通 Cone 使用同一规则。

普通 native 对象只读取本次符号选择和 FFI 绑定所需的格式、目标、定义／引用、kind 和可写性；读取字段时保留必要的边界检查，不重复完整验证 relocation、unwind、TLS、段权限或装载结构。对象不含完整函数类型时，FFI 实现履行声明的责任不被伪造的类型证明替代。显式 library 的 symbol 必须由该库按平台规则提供，默认 namespace 的多 provider 歧义不能按搜索目录先后静默解决。

参数自由 source extern 的普通 managed 调用经原 provider 的 Scoop 薄入口，包含 native transition 与 caller-root publication，入口的 Scoop GC effect 为 Managed；native callee 自身的 effect 仍是其声明合同。泛型正文在定义方与消费方使用同一入口；NoGc 和 release 的直接 native 调用遵守各自规则。

runtime 构建配置的 field 4、5 分别保存 Boolean `enable_gc_stress`、`enable_startup_stats`（沿用 wire 的无符号整数编码，0/1），参与 runtime fingerprint。资源位于 `lib/<triple>/runtime/index.cbor`、`runtime-stress/index.cbor`、`runtime-startup-stats/index.cbor` 和 `runtime-stress-startup-stats/index.cbor`；准备脚本显式生成四种配置。旧 schema 5 索引需重建，runtime ABI 不变。消费方检查请求配置一致，不重读对象证明该配置。

runtime object index schema 7 保存相对索引目录的普通对象路径、生产时已有的对象摘要、合并后的定义／引用及 target/runtime ABI。对象 locator field 2 使用 UTF-8 文本和 `/` 分隔符，不保存生产宿主的路径编码；同一份目标 runtime 资源可由 Windows、Linux 和 Darwin 宿主消费。发布路径为 `objects/<file>.o`，开发缓存使用同一读写入口；相对路径可以包含正常的 `..`，不检查文件系统来源。路径必须非空、无 NUL、反斜线或盘符冒号，且不能以 `/` 开头。消费时检查格式、target（含 libc）、runtime ABI 和最低部署要求；Darwin runtime 的最低系统版本不能高于本次链接选择的部署版本。生产时完整 C toolchain 合同保留为生产记录和更新输入，不要求消费方编译器版本、SDK 或工具路径与生产方相同。读取 index 直接采用这些结果，不重读对象、核对 digest、重建符号摘要或重放 runtime ABI 导出检查；首次生成对象时读取所需符号并检查 runtime ABI 导出。定义的绑定区分 Strong、Common 与 Weak；wire tag 分别为 0、2、1。链接器把 index 中的对象路径直接交给 native linker，不重新物化对象。旧索引需重新准备；此格式变更不改变 runtime ABI 或 `.slib`。

sysroot 采用同一源码与产物布局：`src/<cone-name>` 保存普通 Cone，`vendor/<project>` 保存共享上游输入，`lib/<canonical-target-triple>` 保存 `.slib`、runtime、OpenSSL、nghttp2 与 Linux unwind。维护脚本按目标的生产 sources 条件选择内置库；显式非空 sources 在该目标全部未选中时，不构建或生成该库文档。缺省 sources 与显式空表仍交给普通编译器处理，选中库依赖该目标不可用的库时报缺失依赖，不创建替代实现。liburing、OpenSSL、Ryu、llhttp、nghttp2 及 LLVM／libunwind 的上游源码、版本说明、许可证和补丁集中在 vendor，目标生成头与 archive 不写入 vendor。runtime 编排显式接收 runtime source root 和所选 sysroot 的 vendor root；Ryu 源码与 include 从后者取得，按实际 depfile 输入更新，命中不读取源码或复制输入。公开头的发行位置为 `include`。

Rust 文本后端的维护源码位于 `native/text_backend`，独立 Cargo package 与 lock 不进入编译器 stage workspace；固定依赖源码及许可位于 vendor。维护脚本离线生成目标 `libscoop_text_backend.a` 与头文件，Rust LTO 在准备阶段完成；同时保留实际构建工具链的标准库版权通知和许可，发行时与该目标归档一并收集到 `share/licenses`。普通 Cone 通过既有 C ABI 和 native.libraries 消费普通机器对象；两个链接入口去重同一个目标归档，不执行 Cargo 或探测 Rust。更新命中先于解包、源码复制与工具探测；具体文本行为及故障边界见字符集与 IDNA 规范。

发行打包在各目标的预编译资源准备完成后，使用本次配套工具为全部内置库源码生成文档。文档入口位于 SYSROOT 的 `docs/index.html`，按目标生成的共同站点位于 `docs/<canonical-target-triple>/`；页面、搜索、资源及内嵌源码一同进入归档，跨库导航遵守该目标的真实声明身份。文档警告或生成失败阻止本次归档发布，保留已有完整发行包。此步骤属于维护者打包编排，普通 `scoop doc` 不隐式准备 sysroot；`.slib` 不增加完整源码、注释正文或文档缓存。

普通 build／run／test 的 executable 和两个 link 命令默认消费 `lib/<canonical-target-triple>/runtime/index.cbor`，不触发 runtime 源码构建。显式 `--runtime-objects` 选择另一份索引；build／run／test 的 `--runtime-root` 明确请求从源码构建，与对象选项互斥。缺失实际需要的索引时报告目标和所选路径，不补建或改选其他目标；library 构建不读取未使用的 runtime。

runtime 构建使用 `runtime-v7` 普通记录，按实际源码根、选中源码、include／unwind 配置、flags、C 工具输入和规则版本定位。命中前只查询上次实际源码、depfile 头文件及 include 候选的路径／mtime／大小；不读取源码、扫描并复制整棵头文件树或重读 compiler 内容。更新时直接从原始源码预处理并编译，观察实际 include 名称；全部成功后保存对象、index 与记录。每批对象位于 index 同级的 `objects` 子目录，使准备脚本可按同一相对布局复制本次已知输出，不改写索引或重新提取对象信息。对象采用稳定且不同批次不覆盖的位置；所需 index／对象缺失时补产，真实输入缺失照常报错，失败不发布新记录或替换已有结果。本机 metadata 不改变 runtime ABI。

Darwin 最终 executable 将 Scoop 保留的持久符号命名空间（`_scoop$1$*`，包含 callable、类型／派发表、注册记录、bridge 和 body 起止别名）设为 private external，移出动态导出。跨 Cone 的 Strong／ODR 选择与全部引用在静态链接闭包中完成；选中的 weak 定义不再供 dyld 再绑定，直接调用可由 native linker 绑定到同一镜像中的实际入口。对象和 `.slib` 仍保留原定义与 ODR 合并规则，exact 类型／接口身份、别名地址及 GC／EH 元数据引用不变。源码 public 可见性不等于 native 动态导出；普通 native／runtime C 符号、系统库导入和通过地址传递的 callback 保持原合同。该配置对全部 profile 生效，进入既有 final-link 指纹；消费旧的相容预编译对象时同样适用，不按其他宽泛前缀隐藏 native 符号，也不增加产物格式或 runtime ABI 版本。

native linker 成功后直接发布其输出，不重新读取整份 Mach-O／ELF，重放输入对象、重定位、stackmap、EH、动态绑定或代码签名的正确性检查；不通过链接 map／trace 审计实际读取文件。必要的符号选择、对象格式、ABI／GC 数据及工具参数在产生输入与实际链接时完成，工具退出失败正常传播。生成程序的正常运行和专门 ABI／GC 回归承担输出行为验收，不保留修改刚生成的可执行文件后要求发布复核拒绝的测试。

逻辑 native library requirement 沿完整依赖闭包传递，manifest 的 native.libraries 与非空 SourceExtern lib 使用同一解析流程。driver 将选中 manifest 要求合并到既有 LIR foundation native library 表；相同 key 去重，产物不要求每个库都有 Scoop extern 使用者。Code 覆盖完整逻辑要求，Link reader 复用同一份 LIR foundation 表，不从机器符号反推库名。先在全部显式 library roots 中按既有规则收集候选；该层无候选时检查所选 Scoop sysroot 的 `lib/<canonical-target-triple>` 及其 `openssl/lib`、`nghttp2/lib`；这一层无候选时，再交给所选 target 的平台 provider 在其默认系统库目录中解析。已选层的候选损坏、target/ABI 不匹配或歧义直接报错，不能用下一层掩盖。默认位置只参与实际要求的库查找，未使用的库不探测或准备。两个 link 命令采用同一顺序。TargetDefault 与显式 kind/grouping 保持原合同；OrderedGroup 只限制候选顺序，不引入 whole-archive、用户脚本或任意 linker 参数。

Cone native 源码对象使用 `org.scoop-lang.link-object/native/1` LinkObject 类别，logical key 为归一化 Cone 相对 native 源码路径的 UTF-8 字节，允许语言规范 12.2.1 的前置 `..`，member identity 仍包含所属 Cone。native 路径使用独立于 Scoop 逻辑源码路径的类型，产物 writer／reader 不以 Scoop 源码路径规则拒绝共享 vendor 输入。实际 Link fingerprint 按 member ID 排序加入既有 Code 对象投影；这些对象没有 Scoop Strong/ODR 或 generated-C recipe。program-link 从依赖闭包的对应成员读取普通 native 对象，每个成员作为独立输入参与符号解析；不同 Cone 的相同对象内容不能据此合并。对象格式、target、实际定义与引用沿普通 native 对象规则检查。

Linux 默认目录来自已选 native toolchain/sysroot 及其 target/link-mode 配置；Darwin 来自已选 SDK 的系统库与 framework 目录（包括 `.tbd` provider）。系统 provider 按对应平台的库选择规则解析动态/静态格式；静态模式不得以动态库满足需求。交叉编译不补入宿主的 `/usr/lib`、framework 或其他宿主工具链目录。无合法候选时报告所请求的库和 target；选定 provider 的实际内容、加载合同与工具链/SDK 配置进入既有 ResolvedLinkPlan 和链接缓存。只查已声明的逻辑库，不自动发现相邻 native source/object/archive，也不扩大空 lib 的默认 namespace。

Linux 系统库选择由所选 C driver/linker 按目标配置完成。发行版提供的标准 linker script（例如 GNU 的 `libm.so`）可解析为多个实际 archive/ELF 输入；其脚本内容摘要、实际输入及顺序进入同一解析结果和 link plan。最终链接消费已选文件快照，不重新解释系统脚本或按宿主目录重新选库。该规则不开放显式用户脚本输入。

重复声明已经包含在 target 固定系统输入中的同一 provider 时复用既有定义，不产生第二个符号候选；相同加载名但不同内容仍是冲突。Darwin SDK 的 `$ld$previous$` 指令按 platform 和 deployment 的半开版本区间选择符号的实际加载库及 compatibility version，未命中的指令不改变当前接口。此类信息属于目标 SDK 的正常动态库加载合同。

musl 将数学库、pthread 等接口合入 libc；所选工具链为这些固定系统库提供的空兼容 archive 按该平台规则绑定到现有 libc provider。此别名仅适用于实际系统解析选中的固定兼容文件，显式 roots 中的空 archive 不获得额外符号。

普通 archive 使用既有 archive reader 返回的普通成员顺序作为从 0 开始的 member index（不包含 TOC 和名称表），保留实际 header／payload offset、length 与 digest，允许 BSD/GNU 命名；不另遍历原始 headers／padding 或按名称前缀拒绝普通成员。拒绝 thin、外部或嵌套 archive，实际读取的损坏边界由 reader 报告。通过普通对象 reader 建立候选的定义／引用索引，不为候选或选中 member 另建完整 relocation/EH/TLS/初始化验证器。实际 undefined references 驱动成员选择，新增引用继续闭合，每个物理 member 最多加入一次。

Windows 同样接受普通 COFF archive。short import member 单独保存其 DLL／逻辑符号／function 或 data 种类，`__imp_` 对应 IAT 指针；它不作为 relocatable runtime/native object。使用某个 import member 时把已选 import library 交给目标 linker，保留原导入名／ordinal 和普通 descriptor 成员。COFF COMDAT 的可合并定义按 native Weak 优先级参加候选选择，实际 selection 条件仍由 native linker 处理；NODUPLICATES 保持 Strong，Common 高于 Weak。MSVC `/GL`／`/LTCG` 输入不属于本期普通 COFF 合同，明确诊断。

Windows 显式 library roots 的 TargetDefault 候选为 `L.obj`、`L.o`、`L.lib`、`libL.a`、`libL.dll.a`；StaticArchive 取普通 archive，包括含普通定义成员并附带 imports 的混合 archive（例如 Rust staticlib），不把静态库自身的 DLL 依赖误当成整份库是 import library。纯 import library 不匹配 StaticArchive；Dynamic 取实际包含 imports 的 archive，不凭 `.lib` 扩展名区分。短格式 import 与 GNU `.idata$*` 长格式 import 都将原 archive 交给 PE linker，保留原成员的导入表分组；长格式 import 不作为独立普通对象抽取。长格式 code section 中未标 function type 的 thunk 仍是函数，只有 `__imp_` 定义的数据 import 仍可匹配源级数据名称；IAT 本身的只读属性不代表 DLL 数据只读。roots、目标 sysroot、已选 SDK／MinGW provider 的顺序不变，framework 在 Windows 不适用。显式库仍检查符号归属和已知种类；未被显式库约束的普通 native 引用允许由真实 PE linker 按对象 directives／CRT defaults 完成，不另建立封闭的 Windows API 符号名单。实际 linker trace 的外部输入进入 executable 更新记录。

尚未满足的 native 引用在候选选择时保留 Strong／Common／Weak 绑定，按下述目标优先级排除较弱候选；不能将一个强定义与 archive 中的弱后备实现误报为多 provider 歧义。最高优先级有多个 Strong 时仍报歧义；仅有同级 Weak／Common 时，优先复用 namespace provider，否则按既有 native 输入顺序选择。已经引入的对象定义继续按平台规则满足引用，不为查找另一个后备实现额外抽取 archive member；显式 library 的归属检查不变。

Linux 默认 namespace 中，工具链系统 archive 按普通链接顺序提供后备实现：显式 native 输入已有同名 Strong 候选时，不再把系统 archive 的同名成员计为第二个 Strong 候选。这包括 Rust staticlib 携带的算术 builtins 与配套 `libgcc.a` 的组合。系统动态库的 exports、多个显式 native Strong 候选的歧义以及显式 library 归属检查保持原规则。

ELF native 对象中的未定义 STB_WEAK 引用不加入必须解析的 requirements，也不单独拉取 archive member。对象保留原有弱引用标志，最终地址由目标 linker／loader 按平台规则处理；缺少可选定义可以得到空地址，已经引入的定义仍可满足引用。普通未定义强引用及 Scoop 的 extern 合同仍必须闭合，不能因另一对象对同名符号使用弱引用而放宽。

普通 native 符号由最终 linker 从实际对象、archive 和目标系统库中解析；library 产物允许尚未解析的引用，只有最终链接无法满足的强引用报链接错误。符号不在编译器名单中、或没有对应的 Scoop FFI 声明，本身不是错误。源码声明的类型／ABI 检查、Scoop 类型化符号、runtime ABI、GC 和对象格式检查不扩张为 libc 函数的封闭登记体系。

native 对象按普通目标工具链输入处理，不按段名、load command 白名单、C++ 符号前缀、COMMON 或可执行栈标志拒绝。Scoop 索引所需的格式与目标必须可读取，其余对象内容及嵌入的正常链接要求交给目标 linker 处理；实际工具失败照常报告。C/C++ 运行库按所选工具链配置参与链接。普通 C unwind、TLS 和目标 CRT 支持的初始化／析构记录沿平台规则执行，不要求额外 manifest 开关，不注册为 Scoop initialization unit 或 managed destructor。异常与初始化边界遵守运行时规范 5.5、第 7 章。普通 native 重复定义不使用 Scoop ODR 判等；Strong 覆盖 Common 和 Weak；ELF 的 Common 覆盖 Weak，Mach-O 的已有 Weak 定义覆盖 Common。同名 Common 由 native linker 合并大小与对齐，两个同名 Strong 报冲突。

非空 `@Extern(lib=L)` 在 Darwin 的每个显式 library root R 下，对 TargetDefault 检查 `R/L.o`、`R/libL.a`、`R/libL.dylib`、`R/libL.tbd` 与 `R/L.framework/L`；不去掉已有 lib 前缀或扩展名，不递归扫描。显式 kind 只收窄对应格式；相同内容及加载合同的重复候选可合并，不同候选为歧义。空 lib 只查询已引入的 runtime/native 对象和动态 exports，不扫描目录搜索任意库。

Darwin 支持 `.o`、`.a`、dylib/framework 与所选 SDK 的系统 providers。普通 load/re-export、真实 install name、target/deployment、symbol kind 和绑定关系参与解析；显式库不能被另一库的同名 export 替代。多个已引入 framework 经 re-export 到达同一最终 provider 和同一原始符号时，默认 namespace 将它们视为一个定义，保留实际选中的装载路径；不同最终定义仍按强弱规则诊断歧义。显式 library 仍只能沿该库自己的导出链绑定。动态库内部的 undefined symbol、two-level／flat namespace 和 weak binding 由实际 loader 按库自身的装载信息处理；reader 不扫描内部导入的 C++ ABI 前缀或 ordinal 来拒绝输入。Scoop 所选 export 的种类、实际 re-export ordinal 和显式库归属仍按本次解析需要处理。`@rpath` 从明确目录解析，实际 LC_RPATH 进入输出和 link plan；依赖中的 `@loader_path` 相对 provider。需要部署布局才能解释的直接 `@loader_path`、`@executable_path` 或未知形式拒绝。

Darwin 使用所选系统 linker、按 native 需求选择的 C／C++ 链接 driver、SDK/deployment 与 libSystem，Level I unwind 经系统正常导出解析；C++ 模式同时使用配套 libc++ 与 C++ ABI 运行库。final-link 不使用 dead_strip、不同 body 的 function ICF、LTO 或 raw 用户 linker options。stackmap relocation 后位于只读 `__DATA_CONST`，采用传统 rebase/bind（no_fixup_chains）格式；普通绑定与随后执行的 weak coalescing 分别解释，允许同一指针对相同符号和 addend 具有这两阶段记录。实际需要的 ad-hoc signing 属于平台装载格式。

最终 executable 的普通 release 默认清除不再参与动态链接的内部符号；debug 默认保留，`scoop build/run/test/link --keep-symbols` 与独立 `scoop-link --keep-symbols` 显式保留。artifact-only link 默认清除，不通过读取更多 Compile 数据推测生产 profile。未请求独立调试文件时沿用 linker 内裁剪；请求时先保留调试信息完成链接，再按 2.22 生成调试文件、裁剪程序，Darwin 最后重新完成 ad-hoc signing。必要 native 动态导入／导出、GC／stackmap、EH 和登记数据继续保留；不截短持久 ID，也不改变参与链接的 `.slib`／`.o` 符号及定义边界。

独立调试文件按 2.22 替换原文本 map 的伴随输出与结果复用通路；不再提供 `--emit-symbol-map` 或原 map parser，不将旧选项静默解释成另一种产物。`--emit symbols` 继续提供逻辑符号观察，离线地址归因使用 llvm-symbolizer 与实际 executable／独立调试文件。此替换不增加运行时测量代码。

Linux 接受 `.o`、`.a`，动态模式另接受 ELF `.so`；framework 与静态程序的动态库需求为错误。用户 `.so` 必须是 DSO，不是 PIE executable 或 linker script。未版本化的源码 extern 只匹配默认导出版本，DT_NEEDED 不等于显式库 re-export；TLS、IFUNC、版本化导入按实际 kind/version 检查。

Linux gnu 默认 dynamic PIE，musl 默认 static ET_EXEC；musl 可显式 dynamic PIE，不支持 glibc static。非 C++ 链接明确选择 CRT、LLVM unwind、libc、libm/线程及算术 builtins；GNU C++ 链接使用配套 libstdc++／C++ ABI 运行库，并统一由配套 libgcc_s 提供 Scoop Level I 与 C++ 所需的 unwind，不再额外链接另一套 LLVM unwind provider。musl 不支持 C++ 模式。动态 metadata/stackmap 满足实际 RELRO，静态 musl 的不可变记录位于只读 PT_LOAD；最终文件不得有 text relocation。

最终链接按既有选择顺序将对象路径写入临时 response file，正确引用路径中的空白、引号和反斜杠。Linux 由实际 GNU linker 展开对象列表，不能由 C／C++ driver 先展开成再次受系统 argv 长度限制的命令。response file 与链接工作目录只承担工具参数传递，不改变对象选择、输入检查、顺序或 ResolvedLinkPlan 身份，也不开放用户 linker 参数。

DSO 依赖查找使用显式 roots、provider 目录、实际 RPATH/RUNPATH 与所选系统目录。真实 SONAME、DT_NEEDED、加载顺序和 RUNPATH 进入 link plan，临时快照路径不写入 executable 的依赖名；同名导出使实际 ELF 顺序无法满足显式库绑定时报告冲突。

链接生成 `int main(int argc, char **argv)`、最终 image descriptors、六张 producer pointer tables、image pointer array 与唯一 root entry 引用。C main 按运行时规范 2.8 调用 `scoop_rt_run_program(images, image_count, root_entry, (int32_t)argc, (const char *const *)argv, enable_stack_trace)`，末尾常量由 2.21 的链接选项确定。main 原样返回其 C int 结果，不丢弃 argv[0] 或主动截断退出码。这些生成物只引用所选记录，不复制 TD、storage、cell 或 gateway body，也不在 C main 中构造 managed Array/String。`scoop_td_String` 是实际 String TD 的同地址 alias。

ResolvedLinkPlan 使用 `scoop-resolved-link-plan-v5`，覆盖 root/依赖、Code 与对象内容、runtime/native/startup 输入、ODR 选择、最终 image 内容、系统 providers、库绑定、archive members、加载路径、工具配置和有序操作。物化与输出 locator 不进入内容身份，实际 install name/runpath 与普通符号保留策略必须进入。

本次链接按实际 target、symbol、Strong/ODR 选择、startup/image 与 GC/EH 要求生成程序，native linker 不能改选另一份 ODR 实现。工具成功后普通 rename 发布并保存链接结果，不重新读取刚生成的 executable 完整复核生产过程。目标装载格式与执行位属于输出工作；后端、链接与 GC 合同由针对性程序／结构测试验证，不成为每次发布和命中的重放阶段。失败不发布部分文件。

### 2.9 虚方法与接口分派

每个 slot 有原根声明、完整签名、访问域和 Abstract/Concrete/InterfaceDefault 的明确选择。class vtable 保留 base prefix，override 不改变 family 位置；Any 不预留 equals/hash/toString 槽。

interface 的逻辑方法序按直接父接口声明序继承，再追加当前声明；相同 exact application 的菱形路径去重，移除被原 typed override 覆盖的成员，保留其余相对顺序。getter/setter 独立，private helper 不占槽。实际类／值类型表项匹配原 slot 的参数、结果、execution 与 GC effect。逻辑方法序与下面的物理表位置分别保存，不能将去重后的方法集合当作完整父表。

物理 interface 表按直接父接口声明序完整内嵌各父表，再追加不能复用已有物理槽的当前逻辑方法。单继承的完整父表是 offset 0 前缀；多继承使用编译期固定偏移，菱形保留每个父表内部的完整布局，不为去重删除重复片段。复用要求原 typed 方法身份、exact 声明实例和完整调用 ABI 一致；不同泛型 application 不因擦除后签名相同合并。override 的新槽不覆盖父表布局，内嵌父槽来自同一实际类型对父契约的最终实现，包括 default 和必要 adapter。

一个实际类型仅为其最派生非空接口表分配完整数组；父表符号及 TD 目录项复用数组中的完整片段。同一父表有多个可用片段时，选择 exact interface identity 排序中的第一个最派生表及其按声明序首次出现的片段。各片段语义一致；选择不依赖 arena 编号、导入次序或优化 profile。空接口目录项和合法 null slots 保持。LIR 保存逻辑入口、物理槽来源、存储表及偏移，正常 typed native alias 表达只读父表片段；允许相应 dispatch atom 的包含式范围共享并保留各视图 relocation，其他 atom 的范围规则不变。数组及其所有父表视图必须属于同一个 native object member；ODR 表也按实际共享存储分组，不能将 alias 与所属数组拆开，ELF COMDAT 使用所属数组的 definition group。各 typed definition 身份保持独立，分组只约束物理发射。

上述布局由所有构建 profile 共同遵守。`lir/cross-cone-layout-abi/17` 保存实际物理布局并与逻辑方法位置区分，退役 `/16` 产物及缓存重建；跨 Cone 从完整 exact 父关系和方法身份得到同一布局，不能继续消费旧槽 ABI。接口值、对象头、TD 字段和 GC scan 格式不变。设计及成本验证见 [接口父表直接投影](../program-performance/INTERFACE-PROJECTION.md)。

itable 以实际 interface TD 为键，不要求跨 Cone 的全局槽编号。generic receiver 始终是完整 exact application，只沿声明中的 exact base/interface 关系分派。

实际类型的 TD 接口目录按 persistent exact interface identity 确定顺序；已知具体类型使用的目录索引及跨 Cone 保存的 dispatch 记录遵循同一顺序。该顺序不是 runtime type ID 的数值顺序，runtime 不能据此直接作二分查找。目录读取边界遵循运行时规范 2.1；逻辑目录顺序与内嵌父表的物理槽顺序分别保存。

普通接口值保存 object 和对应 exact interface 的 itab。静态已知 Child 继承 Parent 时，第一次上转型就从已有 itab 按固定偏移得到父表，不读取对象 TD 的接口目录；单继承前缀直接复用指针。仅为调用继承方法时，直接读取 Child 表中对应真实方法与 ABI 的物理槽，不物化中间 Parent 值。赋值、传参、返回及 smartcast 后已有的 Child 视图共同使用这条路径。Any 首次取表、无父关系的接口转换和需恢复接口视图的 adapter 按各自语义处理；已知实际类型直接引用既有静态表，相同 exact 视图直接复制。`is`／`as` 的接口检查可按 2.19 使用返回目录项的查询完成，并复用该次查询的 slots；源码 Boolean 结果不携带 itab。其他检查继续使用既有类型关系入口。空接口以 entry 存在判断成功，不用合法 null slots 判失败。接口方法从 itab 直接取 slot，不再次查询 object TD。

LIR 的 dispatch slot 显式保存单字 object receiver projection 和完整入口签名，普通接口参数／结果仍为双字。兼容 class 实现可直接作为 slot；需要完整接口 this 的 default／变型／装箱入口由已有 typed adapter 重建视图或复制 payload。去虚拟化与直接调用按实际 callee 的签名适配，不能只因都含 pointer 就混同 ABI。静态表和 adapter 继续使用正常 typed definition／reference、provider 和 ODR relocation。

intrinsic class 的 adapter 同样使用该声明已经确定的物理类型与 exact identity。`String`／`Any` 的 receiver 保持本层对应的 intrinsic reference type，不为 adapter 另造普通 `Class` 别名；源级接口继承及 default body 仍按普通声明处理。

值类型只有显式声明或继承的普通 interface conformance；有界泛型调用、interface 调用、装箱与 `is` / `as` 消费同一实际接口闭包。结构 operator equals 不产生 Equals conformance。显式 `Equals<T>.equals` 与其他接口方法使用相同的调用规则；已知 concrete value 的直接调用不因另有接口实现而强制装箱或运行期查表。

按语言规范 13.2，NoGc value method 可以实现 Managed interface slot。源级实现兼容性允许这种 effect 收紧，但实际 itable entry 仍须符合 slot 的完整 Managed ABI：由普通 value/interface adapter 完成 receiver 适配，并以 NoGc 合同调用实际实现；adapter 保留自身的 Managed effect、入口 poll 与必要 roots。具体类型直接调用原 NoGc 方法不经过该 adapter。签名与产物分别记录 slot、adapter 和实现，不能用强制转换或抹去 GC effect 代替适配；不引入新的 runtime 分派机制。

ExportHir 的 interface slot 合同允许目标实现的 NoGc effect 收紧 Managed 声明，并分别保存二者的原始 effect；class vtable override 不采用这项适配规则。值类型与 GC-free 限制由 HIR 的普通声明和 effect 检查完成，产物只检查对应的签名兼容关系，不重新执行源码语义检查。

引用 receiver 适配为 Identity 或 ReferenceDispatch，并满足实际继承／接口关系；物理 pointer 形状相同不足以接受不兼容签名。装箱值的方法 receiver 按值取得 payload 副本，可观察的 this 存储不能指向 box；interface default 可以使用同一 box 的合法 interface 视图。

qualified super 保存实际 direct target。dispatch 数据不重新进行 overload、default 或 override 选择，也不因 provider 为 core 使用另一套规则。

### 2.10 Intrinsic 合同

intrinsic 的识别来自实际声明及其完整 name/target/shape/signature，普通候选和效果规则先确定其合法使用。成功 HIR 保存封闭 typed kind，后续不得按 symbol、FQN 或 annotation 文本重新识别。

类型与目标已经确定且使用方式合法的 intrinsic 调用必须下降为该操作规定的 IR、编译期值或 runtime helper。本地／跨 Cone 的直接调用、泛型具体化后的约束实现选择、语义允许的静态函数引用以及去虚拟化得到的唯一目标遵守同一合同；后续内联或传播暴露的新目标也必须消费。此下降独立于普通函数内联开关、成本阈值与 debug／release 模式；优化模式只影响是否推导出额外的目标事实。默认实参、访问器、初始化器和闭包正文没有例外。动态目标尚不唯一时保留分派，合法动态引用所需的 callable 入口继续发射；语言规范 13.1 禁止引用的 intrinsic 不生成这类入口。

不可引用 intrinsic 的检查与诊断（current_source_location、compile_env、include_str、include_bytes 已实现，include_dir 随入口接通）：HIR 的 managed／native 引用候选检查复用实际声明已有的 typed intrinsic kind，封闭匹配 current_source_location 与 2.10.1 四个操作及其拒绝原因，不新增独立能力表、持久 flag 或 FQN 名称判断。完成正常查找与访问检查后，保留专门的引用失败原因；唯一目标直接报告该 intrinsic 不支持引用，多候选诊断保留相应原因，不通过静默删除候选退化成未找到函数或笼统签名错误。检查先于 closure／adapter／FunctionAddress 构造，错误不推迟到 MIR 或链接。主位置为原引用表达式，消息标明实际声明名称；LSP 仍可利用已解析目标导航到声明。current_source_location 不捕获引用创建点或生成正文的位置冒充间接调用处；直接调用继续使用原 evaluation origin。其他 intrinsic 的合法引用下降保持。

具体化将约束选择得到的 primitive intrinsic 归一化为完整的标量操作，与早期直接调用一致；除法／余数同时物化原有异常依赖。所用操作身份来自本地声明或依赖 HIR 中的 typed implementation，不靠函数名或普通外部函数体推断。

| 合同 | 必须确定的内容 |
| --- | --- |
| intrinsic type | 原 nominal identity、完整 application 与表示 kind；LIR 给出目标布局。 |
| current_source_location | 直接调用表达式的 evaluation origin，结果为普通 SourceLocation 值；规划的引用检查在 HIR 明确拒绝 callable reference，不能合成动态调用处。 |
| include_str / include_bytes / include_dir / compile_env | 语言规范 11.19 规划的定义处常量输入、原定义来源、完整结果类型和独立常量引用；在 Export HIR 完成前展开，具体合同见 2.10.1。 |
| Ptr/FunPtr/addressOf | 实际 pointee/signature/place、unsafe 与非零条件，不取普通按值实参的临时副本。 |
| sizeOf/alignOf | 完整被查询类型，结果由目标布局产生。 |
| integer/float operation | 完整 operand/result kind 与语言操作，不保留待决议源码调用。 |
| atomic operation | 实际 AtomicInt/Long/Boolean/Ref application、值类型、操作种类及完整成功/失败内存序；遵守语言规范 11.14。 |
| MaybeUninit operation | 独立 wrapper 与 payload 的完整类型，以及零构造、合法值包装或 unsafe 取值种类；遵守语言规范 11.15，不保存初始化证明或隐藏标记。 |
| managed callback | native/managed 签名、context index、mode、原 registration 与实际 application。 |
| Throwable 与栈快照 | 实际 core Throwable identity、可继承的固定前缀、普通 byte-array TD、Managed 的首次抛出准备及 printStackTrace 操作；遵守语言规范 11.7、运行时规范 5.6。 |

每个 intrinsic 只承担其指定语义，展开后是普通 typed 运算、内存访问或有明确 runtime 合同的操作。Const 能力只来自声明允许的操作集合，不能由同名用户函数取得。

String 的 byte length、byte-at 和 cursor 单字符解码使用独立 typed intrinsic kind，HIR 在登记边界检查 top-level／非泛型／非 suspend 签名，MIR 映射到封闭 runtime 操作，LIR 选择 NoGc 内部 leaf 并保留 managed pointer 参数。其调用不生成 NativeBorrowed transition、虚构 site 或 caller roots；外围真实 poll／异常／移动后的引用重载不变。源码 `@NoGC` 规则不放宽。HIR intrinsic wire tag 与 LIR runtime function tag 以新增项扩展，不复用旧 tag；旧 reader 对未知项照常报不兼容，编译器与开发 sysroot 一起重建，现有对象／metadata ABI 布局不变。

codegen 在现有 typed call 合同检查后展开普通 byte-length／byte-at：按运行时规范 2.4 生成 AS1 load 与必要的 unsigned bounds check，失败路径复用原 leaf 的 fatal 行为。只有对应 typed runtime 操作参与展开；同名 extern 不改写。invoke 与 UTF-8 单字符解码仍走既有调用发射。展开先于普通优化、最终 roots 和 RS4GC；不修改 LIR／runtime ABI 或新建另一套 intrinsic 表。预编译 core 必须用新编译器重建，已有库对象不会被应用编译阶段重新优化。

LIR 的非零 boxed descriptor 同时保留选定 shape 的 payload offset，供 codegen 消费既有布局结果；不重新推导目标布局，也不增加持久 wire 字段。typed UnboxValue 生成 TD 相等的快速路径与原 leaf 慢路径：快速路径从 AS1 对象地址向已有 typed local 复制完整 payload 字节（包含 padding），ZST 不产生虚假存储。非 exact 的 enum 父整数解箱继续由慢路径处理。普通优化、最终根计划和 RS4GC 顺序保持；不把含引用的 payload 指针转成 raw pointer 后跨 GC 使用。

boxed descriptor 同时保存已选 shape 的实例 size／alignment，供所有 profile 的实际装箱复用普通 managed allocation 生成路径；不从名称或表示相似性识别 native 调用，不在 codegen 重建 shape。去掉专用 box C 后备及其 128 B 选择条件，实际 TLAB 完成条件仍由已有分配策略决定。当前实例不超过 128 B 时采用 LLVM memcpy，较大实例使用共享 NoGC memcpy 控制复制代码体积；两者都从已更新的源 place 复制完整 payload。含引用源 place 由 BoxValue 的既有 safepoint root plan 管理，复制前从搬迁后的规范存储重读，复制后按实际年轻分配事实及完整扫描布局执行必要写屏障。全 profile 使用相同物理 box 布局与 ABI；无消费者的 runtime call identity、专属递归根计划投影和 Array clone／Context fork 调用目录项退役，旧 wire tag 不复用。native RecursiveRegion API 保留自己的真实 native 消费者。普通 extern、局部非逃逸 box 消除、真实分配及动态拆箱慢路径保持，详见运行时规范 2.3。该次入口退役由 runtime ABI contract 21 引入，当前统一 contract 30；已有产物与缓存沿当前版本机制重建，无兼容分支。 Context 的 current／enter／leave 降低为两种 typed LIR 指令 TaskContextLoad／TaskContextStore，直接访问既有线程根槽，不再以不存在的 C 函数表示生成操作。snapshot 复用该 load 与既有堆字段读取，restore 复用实际 TaskContext 字段布局和普通 HeapStore／写屏障；不增加 safepoint、对象字段或新的运行配置。

typed ArrayClone 复用 LIR 已有的源 live root 与完整数组布局。源／目标 element exact type 和 layout 一致时，codegen 从有效源对象读取 count，使用同一 sizing 计算及普通分配路径；唯一原 site 附着于实际分配慢调用，分配后重新加载源根，按 AS1 完整复制元素字节，再执行必要的范围屏障。目标对象使用目标 nominal TD，Array／MutableArray 互转不复用源身份；ZST 不访问 payload。该展开在最终 roots／RS4GC 之前完成，不删除入口／源码循环 poll，不新增 ABI 或跨过程分析。

ArrayAlloc、ArrayClone 与 ArrayAssembly 的生成初始化区间只消费已求值的实参，分配后不调用用户代码或经过 safepoint。release 在这段区间内保留分配的实际 TLAB 成功条件，成功时跳过最终数组范围屏障，其他路径仍按完整元素区间标记。clone 的 memcpy、assembly 内部的逐元素复制及字面量写入保持原求值、顺序、padding、溢出与独立身份语义；输入 object 和 interface object 分量均在分配后消费恢复的引用。无引用、ZST 及已知零字节 payload 不生成年龄判断；普通 ArraySet、带用户 initializer 的数组构造和共享 C 后备不因此省略屏障。该事实只覆盖当前 typed 操作内部，不沿其复制循环向其他 LIR 操作传播。

原子类型使用真实 core 声明、封闭 intrinsic kind 与目标布局，值字段不作为源码可访问的普通字段。构造沿普通 nominal constructor 参数推导形成 `AtomicNew`，保存实际 application 和初值，不伪造源码函数身份。AtomicRef 的 HIR／MIR 表示及跨 Cone 元数据保留实际引用参数 T；四种对象的目标布局由 object header 和自然对齐的单个隐藏值槽组成，初始化在分配后、发布前以 Relaxed store 写入。HIR 检查内存序是编译期常量且组合合法；MIR 保存已确定的操作和内存序，LIR 使用 AtomicLoad/AtomicStore/AtomicRmw/AtomicCmpXchg，携带对象 base、值字段偏移与实际值类型。codegen 降低为对应 LLVM 原子指令，Boolean 使用 i8，整数 fetchAdd/fetchSub 不添加溢出假设；CAS 使用 strong，AtomicRef 按对象身份比较。五种内存序、CAS 的成功/失败区别及其语义进入普通 IR/metadata 和指纹，跨 Cone 与优化不能退化为普通内存访问。

load、store 与 CAS 分别保存对应操作的合法内存序；CAS 是完整的成功／失败序组合，不能只保存成功序后由后端推断失败序。源码原子值种类封闭为 Int、Long、Boolean、managed reference；既有协程等编译器内部状态字继续使用 MachineAtomicLoad／MachineAtomicStore／MachineAtomicCompareExchange 及原来的固定同步序，不以源码数值类型替代其 machine scalar domain。

每个原子方法的 intrinsic 身份同时包含实际原子族与操作，不能把不同 owner 的源码声明合并为同一函数。HIR 在普通参数推导及默认值物化后，将实际 MemoryOrder variant 转为封闭的操作内存序；只追踪本次调用生成的实参临时值，不把源码局部变量折叠成内存序常量。HIR、默认参数／泛型模板与 MIR 使用完整的原子表达式，分别保存 receiver、所需的值实参及已确定的内存序；后续 stage 不重新解析枚举名或补全缺失的 CAS 失败序。

AtomicRef 的 base、expected/new 引用与读取结果沿用 managed pointer 和 statepoint/relocation 契约。写入后的卡表屏障及无 safepoint 区间遵守运行时规范 3.6；NoGc 或普通优化不能消除必要的原子语义、同步顺序或写屏障。LLVM 22.1 与 RewriteStatepointsForGC 的具体组合按 M33-8 验证，最小 IR 与实际 GC 运行分别验收；遇到后端限制须保持语言原子性与 GC 契约调整实现，不能用普通 load/store 代替。

AtomicRef<I> 的隐藏槽保持一个 AS1 object；store／CAS 投影 object，load／exchange／compareAndExchange 在 NoGC 区间内查询静态 I 的表并重建双字返回值。槽布局不扩大为接口宽度，不用两个原子访问模拟一个逻辑值。

MaybeUninit 保留独立的 intrinsic nominal/application 表示，MIR／LIR 操作不提前擦除其有效值规则。layout／scan 递归复用 T，GC-free 条件随实际 payload 传播；Option 分类不继承 wrapper payload 的 niche。codegen 的 zero、wrap、assumeInit 为有类型的零化或完整复制，含引用值沿普通 roots／写屏障处理。unsafe 调用在 HIR 按既有 context 检查，不在后端增加初始化验证框架。

HIR 在普通调用解析和 unsafe 检查后把三个操作归一化为有类型的 wrapper 操作；类型限定的 companion 仅用于解析，不触发 singleton 初始化。显式 receiver 表达式仍按普通规则求值。默认实参模板保存同一操作（expression tag 73，操作 tag 1／2／3），不把 wrapper 零值重写为合法 T 的常量。

安全操作的方法引用在普通 closure adapter 中展开为同一 wrapper 操作，bound receiver 按引用创建规则只求值一次；不要求 intrinsic 具有独立的普通函数 body。assumeInit 的方法引用仍按普通函数类型的既有 safety 规则拒绝。

LIR 的 MaybeUninit 操作写入完整类型的独立 local place；零大小结果仍保留逻辑 place。包装与取值在该 place 中完成全部字节的复制，普通 aggregate local 间的复制也保留 padding，不能经只含源码字段的 SSA 聚合丢失存储字节。临时 place 由既有局部存储及 roots 规则管理，LLVM 可在不改变这些可观察结果时消除它。

LLVM 的 wrapper 存储以不超过 64 位、满足现有对齐的整数单元承载连续非引用字节，余数保留为字节；这样数组读取与 assumeInit 后的标量读取可合并为整字访问，避免字节聚合在向量化时变成逐字节扩宽与重组。完整 size、alignment、padding 和各 managed leaf 的 offset 不变，引用槽仍为 AS1 pointer。整数单元只表达原始 bits，不继承 payload 的 Boolean、浮点、nonnull、tag 或 niche 不变量；所有 profile 采用同一内部表示，跨 Cone 的已有物理 ABI、scan 与 `.slib` 布局编码保持。

该类型的源码 intrinsic 名为 `core_maybe_uninit`；三个操作分别为 `maybe_uninit_zero`、`maybe_uninit_initialized`、`maybe_uninit_assume_init`。前两者属于该泛型类型的 companion，后者是实例方法，均没有独立的 callable 类型参数。它们的源码 GC effect 为 NoGc，assumeInit 另为 Unsafe；普通 NoGC 使用点仍检查实际 wrapper／payload 是否 GC-free。源码名只在 HIR 声明边界解析，后续使用实际 nominal/application 和操作种类。nominal intrinsic wire sum 增加 tag 14，callable intrinsic sum 增加 tag 25（操作 tag 1／2／3 依次对应上述三个操作）；MIR application 与 LIR exact value layout 都保存 payload 的完整 exact identity，wrapper 的 layout constituent 没有 null niche 或 C storage 资格。

上述扩展对应 HIR interface 69／type semantics 27、MIR type bridge 19、LIR layout ABI 15／layout link closure 8。MIR representation 的 tag 13 保存 payload exact type；LIR value representation 的 tag 9 保存 payload value layout 引用，由该 layout 得到 exact type、size、alignment 和 scan，不伪造源码字段。未改变编码的 section 保留原版本，旧 section 产物重建。

共有 CallableSourceEffects 的 implementation 为 Scoop=1、Intrinsic=2、SourceExternScoop=3 或 SourceExternC=4；仅 Intrinsic 带完整 kind payload。ordinary/suspend、GC effect、release callability 与 implementation 的组合必须一致，不能用缺失字段或另一个表补出。

#### 2.10.1 编译期输入与静态数据

语言规范 11.19 的四个 API 与两个普通值类型位于 core Cone 的 `scoop.compiler` package，普通导入和 typed registry 负责识别。prelude 只消费其实际登记的 binding，不因 core 拥有此 package 就将其公开成员注入短名称候选；新增声明不在根 package 转导出。现有位置、布局等 intrinsic 不随本次扩展迁移。

HIR 在普通名称、参数和完整类型选择后对编译期输入操作求值，复用现有 const 依赖环检查、数值常量语义及已经实现的聚合值通路。本轮不继续扩展通用常量与常量表达式，不补 Option Elvis／when 常量求值或新增通用常量合并机制。成功输出保存聚合值的完整类型、实际 application／variant 和按声明顺序的全部字段／元素常量；不使用 env 专属 OptionString 表示、运行期 constructor call 或缺失的类型信息。已实现的常量资格按实际值递归检查，enum 不检查未选择 variant 的 payload；普通 struct 与 intrinsic representation 分开处理，不能仅用空字段列表认定后者为 ZST。const property、正文、默认模板与跨 Cone 接口复用已有常量值表示，ordinary val 的静态初值也消费该结果。语言规范 9.1.2 区分可表示的常量值与当前支持的求值表达式；本版不求值 struct 主／次构造、enum payload 构造及其默认实参，不为常量求值调整普通构造器的默认值注册／准备顺序。include 系列的静态资源表示和发射已实现，输入参数只组合已经实现的常量表达式。

静态 enum 初值的数据通路已实现：初值保存实际 application／variant 与按声明序齐全的字段；空 variant 使用空字段序列。HIR 至 LIR 逐层保留实际 variant 和字段，LLVM 按既有 integral／niche／tagged 布局发射；inactive GC slot 与 padding 保持零，String payload 保留 immortal relocation。该表示能力服务本节的常量求值扩展，不单独扩大源码 const 表达式集合；普通 enum 构造调用及默认 payload 的求值不属于本版范围。

Unit 与 tuple 的静态初值沿同一数据通路保存。完整存储类型提供每个 tuple 元素的类型，元素初值按源码顺序递归编码；LIR 将 Unit 表示为无元素的 aggregate。显式标注类型的 ordinary top-level property 若由 Unit／tuple 字面量及现有静态常量表达式组成，直接采用 EncodedStaticValue；包含运行期表达式时保留完整 initializer 的正常求值顺序。tuple 使用既有自然对齐，嵌套 String 保留 immortal relocation，Unit／空 tuple 的 storage token 遵守运行时规范 2.8；不新增 ABI 或产物编码。

const property 的本地值与静态初值共用 HirConstantImage，完整 property／表达式类型提供递归元素类型。读取常量只物化普通 literal、StructConstruct 或 VariantConstruct，不经过 StructInit 或任何源码 constructor。跨 Cone 的聚合常量使用独立于注解标量的 canonical 值结构；标量沿用原表示，聚合项按完整 property signature 的字段类型解释，enum 另保留实际 typed variant identity。共享接口仅保存该常量事实，消费方不重放其 initializer。类型／字段匹配在实际物化边界检查，保留格式、typed 引用和 property 接口闭包检查；不在 meta crate 中保留仅供测试调用的语义重放 authority。常量中的多个 String 使用 property identity 与元素序号区分，避免将不同字段错误合并。

聚合 const 值将 `cross-cone-interface` 升至 82：canonical const value 的标量 tag 1～5 不变，Unit／tuple／struct／enum 分别使用 tag 6／7／8／9；默认模板的 String owner 增加 tag 3，保存 property identity 与元素序号。其他 section 和 runtime ABI 保持不变；旧 HIR section 及依赖它的产物需要重建，不提供兼容 reader。

`env` 的 `compile_env` intrinsic 使用 callable intrinsic sum 的 tag 34，无额外 payload；完整签名仍由普通 callable declaration 保存。此编码对应 `cross-cone-interface/87`，不改变固定 core bootstrap protocol 或 runtime ABI。

`includeStr` 的 `include_str` intrinsic 使用 callable intrinsic sum 的 tag 31，无额外 payload；完整签名为 String → String，对应 `cross-cone-interface/84`。环境与文件输入的成功记录使用闭合种类（tag 1／2）；文件记录保存原 Cone 根、实际根、原 locator、解析后目标、长度与修改时间。machine protocol 20 的成功 field 6、编译缓存 field 4 及 check／doc 的第三项统一保存该输入序列；缓存域同步升级。展开的 Cone 配置保留原输入根，供 include 的词法范围检查使用，实际源码发现仍使用原有 real root。

`includeBytes` 复用同一文件宿主，HIR 保存带完整 `Array<Byte>` 类型的静态字节数组表达式，数据使用原始 byte string；普通数组字面量仍沿原有分配语义执行，不因元素恰为常量而改为共享对象。默认／泛型模板保存这份数据，消费时不再读取文件。MIR 的既有 immortal 对象表扩展为 String 或字节数组的闭合数据种类；字节数组项同时保存实际 class application、原始字节、对象身份和定义来源。String 优化只消费 String 项。LIR 的字节数组 global 关联既有 ArrayType 布局与 TD，直接发射 header、logical count 和字节内容，并沿共有 immortal 登记与链接引用链消费，不经过运行期数组分配或拷贝。本轮只接通 intrinsic 的静态结果，不扩展 9.1.2 已推迟的静态对象 const 组合。

该扩展为 callable intrinsic 增加 tag 32、默认表达式增加 tag 77（byte string payload），对应 `cross-cone-interface/85`；immortal 对象角色新增 ArrayConstant=2、结构位置角色新增 ArrayConstant=13，对应 LIR `identity-foundation/10`。原有 String 编码及角色保持；runtime ABI 与文件输入协议不变。

`includeDir` 在普通参数绑定后消费 path、include 和 exclude，包括命名实参与默认数组；这些参数不留下运行期数组构造或求值临时变量。HIR 用普通 StructConstruct 表示 EmbeddedDir，其 files 字段为带完整 Array<EmbeddedFile> 类型的静态文件表表达式，逐项保存逻辑路径 String 与原始字节。默认／泛型模板沿同一表示传递。MIR 将每项路径、字节数组和文件表登记为已有 immortal 对象，文件表保留对子对象的 typed 引用。LIR 使用实际 Array<EmbeddedFile> 的布局与 TD，按普通 EmbeddedFile 字段布局发射引用；链接从文件表的实际引用闭包选择子对象。此扩展为 callable intrinsic 增加 tag 33、默认表达式增加 tag 78（路径文本与 byte string 的条目数组），对应 `cross-cone-interface/86` 与 LIR `identity-foundation/11`；不新增 intrinsic type、runtime ABI 或通用常量求值规则。

includeDir 的目录输入记录使用 CompilerInputV1 的 tag 3，保存定义 Cone 根、实际根、原目录 locator、实际目录、include／exclude 与按逻辑相对路径排序的条目。每项保留实际目标；目录与未选中条目只记录其种类，选中文件另保存长度和修改时间。更新检查重新枚举目录和解析链接，比较这份记录，不读取文件内容，也不将未选中文件的修改时间作为输入。递归循环按当前祖先目录的实际路径检测；同一目标经不同非循环逻辑路径出现时保持独立条目。glob 使用 glob 0.3.4 的 Pattern 匹配器，配合 11.19 的路径语法检查，不使用该库的文件系统遍历。此输入扩展将 machine protocol 升至 21，compile cache key 域升至 v10，check／doc 缓存目录升至 v4。

文件定位／读取、目录筛选和环境查询由输入宿主提供给 HIR 的窄输入接口完成，driver 与分析宿主供给实际实现。HIR 负责语言语义并保存结果，IR/meta crate 只保存 typed 数据，不自行读取文件或环境，也不依赖 driver／上游 stage 的实现。每次编译中同一实际请求的结果及已记录输入直接复用，不因常量试探、模板发布或下游 stage 重读。失败的试探不发布常量与绑定事实；输入读取的缓存不赋予失败表达式成功语义。

输入宿主使用定义 Cone 的根目录执行 include 范围检查；manifest 模式沿已有 Cone root，single-file 沿解析入口 symlink 后的实际文件父目录。按语言规范 11.19 同时检查定位路径和解析全部链接后的实际目标，父目录 symlink、链式链接与路径中的 `..` 按实际文件系统语义处理，不能仅比较字符串前缀或只检查末段链接。includeDir 对根及每个遍历条目检查，包括 glob 排除项；进入目录前先确认其目标仍在 Cone 内。合法链接的输出名称保留逻辑相对路径，内容去重不省略另一条路径的范围检查。这里只实施 include 资源的目录边界，不增加 owner／权限归属／inode 审计，也不修改普通源码、native 输入或包管理的路径合同。

HIR 在完成常量名称求值后、请求宿主查询环境前，拒绝空、含 `=`／NUL 或非精确 `SCOOP_` 前缀的 env 名称；错误定位 name 实参并说明允许的前缀，不把拒绝结果表示为 Option::None。只观察实际查询的合法名称，不枚举或记录其余环境。文件越界诊断定位原 path 实参，并补充具体条目、解析后目标及定义 Cone 根；非法输入不产生常量或成功依赖记录。

编译期输入操作在原定义的语义检查中完成，泛型／default／inline body 随后保存已求出的常量值或静态引用；运行期不可达性和未使用默认参数不延后读取。生成正文以目标声明的原实际文件作为相对路径基准，single-file 以真实入口定位。诊断保留精确调用／实参位置与实际读取路径；宿主 locator 只存在于输入记录与诊断，不成为值、常量身份或可移植模板。

常量存储合并属于内部优化，编译器应尽量合并可以安全共享的数据，范围包括普通常量和 intrinsic 结果。候选比较完整 exact type 与内容；浮点等已有常量保持原始位型，不能以数值相等混淆不同表示。哈希若用于索引，命中仍用完整 key 相等判断。不同泛型特化及泛型类 companion 中的声明、application 和常量实体身份保持独立，不能因值相同就合并语言实体或 companion 对象。原输入依赖单独按输入记录，不能因内容相同而删除另一文件或环境变量依赖。

泛型具体化、默认参数展开、闭包、内联及跨 Cone 消费可以复用常量数据，但没有统一对象身份或唯一物理副本的强制合同。是否合并、在何阶段合并以及不同 profile 的选择由实际优化实现决定；未合并不能导致源码错误。复用既有 typed 常量、静态对象和产物定义／引用关系，不以本功能为由预建跨阶段的全局常量身份框架。普通字符串常量同样适用语言规范 9.1.2 的 identity 规则；普通运行期对象构造及要求独立副本的操作仍保留既有语义。

MIR 保存完整常量值／对象引用，LIR 沿已有布局生成静态数组、enum／struct 值和 immortal relocation。Option<String> 使用运行时规范 2.5 的既有 niche；Some 空字符串引用实际非 null String。代码、初始化静态值和其他常量通过普通 relocation 引用实际发射的数据定义。链接按实际引用选择定义及其既有 immortal 登记，不将全部常量变成无条件根，未引用的数据可裁剪。debug／release 都直接生成静态内容，不在程序启动时构造数组、解码文件或组装目录；这一静态求值要求不等于必须合并相同内容。

沿 2.7 的成功响应／构建记录增加实际输入：定义 Cone 根与解析后的实际根、单文件的 host locator／实际目标／长度／修改时间；目录根及 include／exclude、归一化匹配集合、所选文件 metadata，以及条目范围和遍历更新所需的目录／链接解析状态；实际查询的合法 SCOOP_ 环境变量名称与缺失／有效值状态。成功记录只在整次编译成功时发布。下一次先按记录判断更新：目录必须发现新增、删除、改名及链接解析目标变化，不能只检查旧匹配文件；glob 排除项的新增或链接改指也须满足目录范围。实际目标变更使相关输入重新求值，越界／断链则按原源码位置报告并使本次构建失败，不能继续使用旧结果；未匹配普通文件的内容变化不触发源码重编。env 的缺失与空值不同，未查询变量不影响复用。全部输入未变且输出存在时直接复用，只观察必要 metadata、目录条目、链接解析结果与已查询环境值，不读取源码、资源内容、命中 `.slib` 或执行工具探测；文件内容更新沿既有修改时间／长度规则，不增设内容 hash 验证。目录与链接状态服务匹配集合和实际目标更新，不能只依赖根目录 mtime 发现深层变化；同次观察结果直接复用，不在后续 stage 重做范围检查。

父 driver 向实际编译子进程原样传递可查询的 `SCOOP_` 环境变量，包括空值和非 Unicode 值；不以内部 sysroot／缓存配置覆盖这些名字。内部 child 命令用显式 `--object-cache-dir` 传递对象缓存位置，源码查询仍读取原继承值。记录以完成求值的实际输入为准；target 与 runtime 环境不参与查询。文件／环境变化只使重新编译源码的 provider 失效，纯预编译依赖消费保留其中的旧常量。正常失败不发布结果，同键并发沿现有锁协调；不为构建中外部替换输入增加快照复制、稳定性重试或来源审计。

env 的实际更新记录使用名称与 `Option<String>`，空数组表示变量缺失，单元素数组保存存在的值。这些值作为环境种类进入上述 machine protocol 21 的输入序列，以及编译／check／doc 缓存记录。记录随 build、check 和 doc 的实际 HIR 结果返回，不写入供纯预编译消费的环境查询要求。重新构建时，非法 Unicode 的当前值使旧记录失效，再由源码求值给出诊断。

build、check、doc 和 LSP 的共享 HIR 路径使用同一求值规则。check／doc 的缓存同样记录相关文件、目录和 env 输入；LSP 在分析到相应区域时求值，输入更新使依赖该输入的语义结果失效，未完成结果不发布。语言服务不为这些值进入机器生成或启动外部生成器；其环境来自语言服务进程，外部父 shell 改值不会自动改变已启动进程的环境。

新增常量种类、独立对象身份、模板引用及构建输入按实际编码改变升级相应 IR section／成功响应／缓存 schema，并一并重建开发 core；不在设计阶段占用未实现的 wire tag 或版本号。无变化的格式保持原版本，旧缓存按现有机制失效，不提供迁移或兼容 reader。实现范围、glob 选择与分层验收见[设计](../compile-time-intrinsics/DESIGN.md)。

### 2.11 输出完备性与 typed identity

语言9.1.8的条件接口声明属于原nominal owner，保存原Cone、typed owner、完整目标interface、规范化正向条件及真实成员／slot引用。companion使用原host binder和singleton，条件声明、函数、slot及concrete实现使用相应不同typed identity；不以字符串或未决application代替具体实体。HIR在定义处收集完整关系集合，检查重叠、条件作用域、继承覆盖及全部实现正文。条件成员的签名和正文保留本地条件环境；条件查询不能使用实现正文的局部this事实证明guard。

Export HIR保留完整条件关系、成员适用性、投影绑定、继承及必要support body，不按某个consumer的import或已用实参裁剪接口集合。消费方、再次发布方和linker不能为外来owner追加实现。查询结果只有在相同参数事实下复用；完全具体的同一application保持唯一接口集合。实际编码改变时升级对应格式，IR/meta只承载数据与正常格式／引用检查，不重建语义查询器。

M57 将 `cross-cone-interface` 升至 81。nominal declaration details 的可缺省 field 13 保存非空条件接口声明序列，每项保存 typed 条件声明 id、目标 signature key 和完整原宿主 binder 条件；缺省表示没有接口实现块记录。Parser 要求实现块具有非空 where，AST 直接保存必填条件；无条件接口沿用普通类型头的记录。field 14 保存检查后实际存在的条件 conformance，每项记录完整接口和原条件的完整宿主 application；继承重列沿用基类条件，空接口也保存关系，已证明不可能的继承义务不导出。消费方直接代入这些关系，不从实现块和基类重新推断重列或覆盖结果。callable declaration 的可缺省 field 12 保存成员条件的原声明 id 与完整原宿主 application，复用条件 conformance 的 application 编码；普通成员不携带该字段。本体实现块的成员继续使用 B+C binder，继承合成成员使用其真实宿主的 binder 并单独携带代入后的基类条件，正文、调用和具体化均沿用此条件，不反向推断构造实参的内部参数约束。dispatch selection role 增加 tag 3，保存接口 application 和条件声明的完整原宿主 application，区分互斥条件下的不同 slot 选择。这些记录使用既有签名、binder、实体引用和正文格式；读取时检查普通归属、引用与形状，不重放声明方的重叠或正文检查。旧产物须重建，其他 section 与 runtime ABI 保持不变。

具体化根据完整实参落实条件并填充普通itable／vtable输入；保留定义处选择的重载、default表达式和字段codec，仅替换已绑定投影与slot目标。进入LocalConcreteHir前必须消除待定条件，类型、调用与实现关系完整；后续stage不重放条件推导。宿主布局、构造、GC scan、companion初始化及ABI维持普通规则，不引入类型定义分支、赢家集合、dictionary或运行期条件选择。

target/backend/C toolchain 的 typed identity 保留既有 canonical 编码及实体派生关系，支持集合在实际消费者处判断；仅移动实现和改变进程内表示不提升 wire/schema/runtime ABI，只有实际编码或可消费合同变化才升级相关格式。artifact-only 消费使用已保存的布局与 ABI，不加载 codegen、创建 TargetMachine 或重新分类。具体 target/backend 布局与合同常量可由 codegen 所属的静态数据模块供 artifact-only 消费者共享；该模块不含 LLVM、ABI 分类器或工具执行，公共 identity/LIR 只定义数据类型和规范化编码。

所有下游转换必需的数据都必须在上游成功输出中完整且类型化。合法的缺省、空集合或无实例分支使用明确 sum；缺失的类型、签名、root、结果存储、构造目标、字段或 layout 不能由 Option、默认数值或名称回退掩盖。

type、function type、generic declaration、完整 application、concrete function、runtime/extern target、dispatch slot、layout、TD、property/accessor/storage、initialization、callback 与 safepoint 使用不同 ID 类型。stage-local ID 不持久化，跨 stage 关系显式映射；同形结构与相同整数值不建立身份。

语言服务的文本版本、分析 revision 和语法节点定位只标识进程内的一份输入或查询位置，不替代上述实体身份，不进入 `.slib`、ODR 或 runtime ABI。语法未完成、尚不能建立持久声明身份时，查询保留语法位置与不可用状态，不能按名称或 span 伪造语义实体。

persistent keys 保留原 ConeIdentity、typed owner chain、kind 和该声明种类所需的规范化签名；不含正文、arena ordinal、host 路径或 link symbol。透明 alias 不建立新的 exact type，完整 generic arguments 和 structural type constituents 参与 exact identity。

泛型函数的重复签名与持久化声明 key 包含语言规范 3.2.2 的规范化条件集合。正向 bound 与 kind 继续保持原互斥表示，负向接口条件独立保存完整类型和诊断来源。HIR lowering 在普通推断完成后执行三态负向查询；MSC 使用相同规则并保留源声明的条件事实。IR／meta 只表达数据与 typed 引用，不重新实现语言条件判定。

显式扩展调用在同一候选层没有适用目标时，优先采用类型实参个数匹配候选的失败诊断；仅在没有这类候选时使用个数不匹配的诊断。选定的诊断组保留各候选实际产生的失败信息，仅合并来源、位置、消息及附加信息均相同的诊断；本地与依赖候选遵守同一规则。声明身份或枚举顺序不能让首个候选的 receiver／参数错误覆盖另一候选的约束或 lambda 正文错误。复用已有探测诊断，不重新检查失败候选；有适用候选时仍丢弃未选候选的失败信息。

依赖 companion 在 Export HIR 和 signature key 中保存固定投影及宿主 binder，条件表独立保存投影的存在性、正向／kind 与负向条件。普通推断不从投影倒推宿主；宿主确定后才解析真实 public companion application 并验证条件。正文保留投影值访问及定义处所选 bound callable，具体化替换真实 companion 类型与 singleton，复用原有 ensure、interface／class bound 调用及实例身份；不增加隐式 codec 参数或新的初始化机制。

CallableApplicationKey 区分原声明、NoOwner/ExactNominalOwner/EnclosingCallableApplication/EnclosingInitializationApplication，以及无 callable arguments 或完整非空 arguments。lexical body、local value、capture、initializer 与默认值的原定义路径在替换后保持；物理 producer 不替代定义 owner。

成员调用要求在 Export HIR 中保存独立的 typed 要求身份、完整调用契约与正文要求引用。父函数声明 key 先以规范化契约确定，再由父声明及规范化位置确定要求身份，不用普通 function／interface slot ID 替代要求。参数替换后成功的要求绑定复用同一公开成员查询结果，保存实际 typed 目标、参数映射、默认值／vararg 与结果适配；未知主体通过定义环境中相同契约转发。失败候选沿已有 probe 事务丢弃，不提交实体或展开任意方法正文。

默认参数中的闭包在其他 callable 中创建时，同时携带原词法参数的完整替换与已满足的要求绑定；跨 Cone 模板保留这些绑定，不能借用创建位置的同名要求或重新选择已经确定的实现。普通正文中的闭包继续继承原词法环境。

泛型 application 仍只由声明及完整类型实参标识，不增加调用点作用域、隐式 dictionary 或所选方法的 specialization 参数。Export HIR／跨 Cone 声明与正文编码完整条件和要求引用，consumer 使用实际类型的公开成员与原默认模板，不能按本地扩展重选。进入 LocalConcreteHir 前消去要求与投影，只留下正常调用、构造、值适配和 singleton 访问；要求落实后新增的实际调用边在具体化请求处遵守既有递归规则：沿调用链重新请求同一声明且完整实参改变时诊断，阻止无限实例化，同实例递归直接复用。MIR、LIR、linker、runtime 不重放源码要求查询，runtime ABI 不变。

具体化工作队列在检测到上述无限实例化时停止；普通类型条件错误（例如 Context key 在替换后重复）继续收集其他独立请求的诊断。错误输出不包含未完成的 concrete module，不能把一般条件失败误作队列不再能终止的标记。

persistent mangling schema 为 `persistent-v1`，语义 symbol 为 `scoop$1$<kind>$<lowercase-hex-owner-id>`，再按目标加平台前缀。kind tags 为：

| 实体 | tag | 实体 | tag |
| --- | --- | --- | --- |
| CallableBody | cb | StaticStorage | ss |
| ImmortalObject | io | TypeDescriptor | td |
| Layout | ly | ScanProgram | sp |
| DispatchTable | dt | DispatchSlot | ds |
| InitializationCell | ic | RootRegistration | rr |
| ImmortalRegistration | ir | InitializationRegistration | nr |
| TypeRegistration | tr | SafepointRegistration | sr |
| CallableRegistration | cr | ImageDescriptor | im |
| GeneratedBridge | br | OdrMember | od |
| DefinitionBoundaryStart | bs | DefinitionBoundaryEnd | be |
| RootEntryDescriptor | re | | |

symbol 只表示已存在的 typed entity，不能反向补声明或决定 ABI。完整 identity 的短 runtime type/site ID 必须验证全程序无碰撞。

诊断来源使用实际 Cone、logical source、definition/evaluation context 与 byte span。synthetic entity 使用其真实 generated role，不伪造源码位置；显示名称和 mangled symbol 不代替实体身份。

正确性由产生结果的相应 stage 完成：源码语义、当前转换的完整 IR、ABI/GC 数据和实际链接绑定分别承担其职责。消费已有结果按需解码并安全索引，不完整重放名称、类型、引用、布局或生产语义，不用 Verified 包装和深比较建立证明链。加载后的 GC、地址与内存边界契约保持。

对象定义的符号表序号查询复用一次建立的 member 内索引。image 注册表校验按当前 atom 收集并按 offset 查询实际 relocation，Strong binding 复用已排序的 `(member, atom, offset)` 查询；不得为每个 safepoint／type／callable 表项重新扫描整个对象或 Cone 的 relocation／symbol 集合。数量、唯一 offset、实际 target、role、宽度与编码值检查保持不变；这些查询数据不进入产物格式或实体身份。

对象定义的符号表序号查询复用一次建立的 member 内索引。image 注册表校验按当前 atom 收集并按 offset 查询实际 relocation，Strong binding 复用已排序的 `(member, atom, offset)` 查询；不得为每个 safepoint／type／callable 表项重新扫描整个对象或 Cone 的 relocation／symbol 集合。数量、唯一 offset、实际 target、role、宽度与编码值检查保持不变；这些查询数据不进入产物格式或实体身份。

### 2.12 Core 与普通依赖

core 是普通 library Cone，可修改和重建，使用相同声明、可见性、泛型、产物、ABI 与链接规则。sysroot 只提供默认 locator，不改变实体归属或来源资格。

M42 标准基础库：`scoop:scoop.std:0.1.0` 是只直接依赖 core 的普通 library Cone，公开契约见 [标准基础库规范](SCOOP-STDLIB-SPEC.md)。其代码全部由 Scoop 源码组成，使用公开 core API 和普通跨 Cone 编译、单态化、产物及链接路径。不得为库新增 native source、`@Extern`／`@Intrinsic`、按库名识别的 lowering、特殊容器布局或产物字段。Collection 是普通 core 源码接口扩充，新增接口关系按已有 API／布局／代码指纹失效规则重建依赖，不另增版本或发布机制。若合法库源码暴露编译器、core 或 runtime 缺陷，先建立独立最小回归并修复底层，再继续库功能；不得在库中建立绕过缺陷的平行实现。修复确实改变既有语言、ABI 或格式契约时，先修订相应规范并按原规则迁移，不预先为 M42 扩大协议。

只有编译器主动构造或调用的语言角色需要独立的 typed protocol 引用，例如 Unit/Any/Nothing、整数/Char/Float/Double 表示、Option/Iterator、编译器异常、初始化循环、协程和 callback。引用必须解析到实际声明及完整签名，缺失、重复或形状错误在相应边界报告。Any/Nothing 必须具有语言规范 11.1 的源码声明和普通 public binding；内部顶／底类型表示只引用这些声明，不能用固定名字补建缺失的名义声明。

Option protocol 保存同一个实际 owner、Some payload 与 None variant；Integer protocol 完整覆盖八种整数 kind。compiler exception protocol 包含实际 Throwable、UnwrapException、ClassCastException、ArithmeticException、IndexOutOfBoundsException、IllegalStateException 与 IllegalArgumentException 及所需调用目标。

内存中的 core protocol 使用具名、类型明确的角色字段，不以位置数组保存可任意混合的 entry 类别。wire 解码在既有边界检查 tag、引用与 owner 后形成具体字段；只改变 Rust 内存表示而不改变编码时不提升 section 版本。普通 provider、签名及实体关系检查保持，不增加来源资格或证明对象。

ToString、Hash、普通 operator、print/println、容器和 codec 通过普通声明与接口分派使用。新增普通 core 成员不要求新增编译器角色表；intrinsic 表示不自动赋予未声明的 conformance。

Disposable、普通／挂起 use 和 ResourceUseException 同样是普通 core 声明。use 的成功与失败出口由 Scoop try/catch 和函数调用表达，跨 Cone 复用已绑定的泛型正文与协程转换，不按名称生成专用 cleanup，也不把该异常加入 compiler exception protocol。

gcAdjustPressure 是普通 core C ABI extern，绑定运行时规范 3.1 的 GC-free pressure 操作。沿普通 GCLeaf 调用、native 引用及链接路径处理；不添加 intrinsic、compiler-known role、buffer 特判或产物字段。NativeByteBuffer 的 owner、游标、同步借用和 release hook 使用已有普通 class、try/finally、FFI 及 release lowering。

write/writeError 通过普通 String 作用域借用调用 C ABI 字节输出后备，flushOutput 通过普通 C ABI 调用刷新 stdout；编译器复用 NativeSafe 调用及 pin 帧，不为输出名称增加 intrinsic 或专用状态切换。print/println/eprint/eprintln 的 ToString bound、求值次序和跨产物泛型展开均遵守普通函数规则（语言规范 14.4）。

exit 是返回实际 Nothing 的普通 core Scoop ABI extern；调用及产物保留底类型、既有返回 carrier 和无正常后继的语义，不伪装成 Unit 或添加不可达占位正文。runtime 的单向终止和刷新合同见运行时规范第 7 章。

core prelude 从普通 public bindings 提供最低优先级名称，direct/support 依赖与 re-export 保留原身份。普通调用、默认值、模板与 native 引用使用同一 Export/MIR/LIR 合同；String 的 layout、TD、registration 与 alias 也来自实际声明。

Any/Nothing 的 intrinsic kind、原声明、完整 application 与静态描述经过同一 HIR/MIR/LIR metadata 通道。别名和再次发布保留原身份；实际 core 源码声明必须可通过普通依赖查询取得。Any 可以保留内部的顶类型分类，Nothing 可以使用带底类型表示的普通 class application，但分类不替代源码声明。

HIR 将 Nothing 参与的子类型、结果合并和调用完成性质在源码边界确定。向其他类型适配只保留原求值，不生成目标值。MIR 将无正常返回的求值结束为明确的不可达正常边，保留原有异常出口；后续调用实参、字段写入和正常 return 不能越过该终止点。LIR/codegen 复用已确定的控制流和调用签名，不根据函数名称猜测 noreturn，也不把它当成 Unit/ZST。

### 2.13 跨 Cone 模板与 ODR

普通泛型模板所引用的非泛型 suspend 函数／成员，按实际支持声明生成 provider 的完整 callable 导出，包括非 public 声明。选择过程使用同一共有声明目录；公开 binding 与用于模板消费的支持声明不构成不同的执行资格。消费者复用原声明的协程执行 ABI，不因其访问域遗漏 MIR 导出，也不将原正文重新单态化为另一函数。

非泛型接口的私有普通方法与其他 nominal 的直接成员使用相同的自动物化规则：所属类型及签名进入物化闭包时，provider 生成完整正文和 callable binding，不要求先出现本地调用。方法仍保持私有访问域，不占接口分派槽；带类型参数的方法仍按实际 application 物化。

Export HIR 保存模板的已解析正文、原 binder、默认值、局部 callable/capture、字段/variant 和完整定义位置。实例化只按 actual application 替换类型，不重新执行 provider 的名称、重载、接口或 codec 选择。

负向条件随原 binder 和泛型 callable 导出，消费方在正常调用适用性检查中使用；已绑定正文不在具体化时重选普通重载。条件变化进入既有声明接口与更新判断，不增加缓存内容复核。实例身份仍由原声明和完整宿主／函数实参决定，不引入调用点作用域或隐式 dictionary。

普通参数自由定义由原 Cone 提供，即使签名含已闭合 generic application 也仍是该 Strong 定义。泛型和 structural application 的机器实体由实际需要它的 Cone 物化，并保持程序内共享身份。

默认实参在调用方展开，其中的词法 callable 及其 closure／协程辅助实体按原默认值所属 callable application 共享。参数自由默认值也使用原声明、NoOwner、NoCallableArguments 的已有 application key；该上下文只归属实际展开的默认值实体，不把原普通函数或构造器改成 ODR 定义。嵌套 callable 继承实际词法父上下文，本地展开与跨 Cone 展开必须产生相同身份。捕获的参数和默认值局部变量保留其原词法 owner，调用方求值仍有独立 storage。仅建立共享上下文不产生所属 callable 的机器正文。

SpecializationKey 是四分支 CBOR sum：Nominal=1 `{origin generic type, nonempty exact arguments}`、Callable=2 `{application}`、DelegatedProperty=3 `{origin extension property, nonempty receiver arguments}`、StructuralType=4 `{exact type}`。`OdrGroupId = H("scoop-odr-v1", key)`。

OdrMemberKey 为 `{1=group, 2=role, 3=typed discriminator}`，`OdrMemberId = H("scoop-odr-member-v1", key)`。role tags 为 CallableBody=1、GeneratedNominal=2、Layout=3、ScanProgram=4、TypeDescriptor=5、DispatchTable=6、DispatchAdapter=7、StaticStorage=8、ImmortalObject=9、InitializationCell=10、RegistrationRecord=12、DiagnosticBytes=13、AddressTakenConstant=14、ObjectSupport=15、ReleaseHook=16；11 保留不用。

每个物理 member 保存与角色匹配的 shared ABI，OdrAbiFingerprint 使用 `scoop-odr-member-abi-v1`。callable ABI payload 为 `{1=GC effect, 2=ScoopAbiSignature}`；包含实际调用约定、参数/结果、物理类型、布局与 scan，不包含正文或 CFG。registration ABI 包含 kind/version/size，site/root 数量属于所选实现。

普通依赖定义一致后，同一 group/member 的完整 key 与 ABI 必须一致；不比较不同 producer 的 LIR、机器码、EH 或 stackmap 内容。不同优化实例可以具有不同内部实现，但同一 body 与其关联数据必须一起选择。

同组独立 helper 可以取合法并集，不要求整个 group 来自同一 producer。TD、static storage、cell、published/failure root 与初始化状态具有唯一实际地址，不能因多个 consumer 各自物化而重复。

泛型 companion 按原 companion 声明和完整宿主 application 区分，包括 phantom arguments；generic delegated extension 按原 property 与完整 receiver arguments 区分。每次实际物化都完整提供所需 storage、unit、initializer/ensure 和 registration，同一 application 最终只初始化一次。

源码模板可以合法引用其支持声明；support closure 不使这些声明成为普通 public lookup。普通外来 constructor/accessor/helper 仍引用原 Strong 定义，不复制成消费者的新声明。

类型描述符、装箱、closure、协程与其他实际 helper 的依赖必须闭合。ordinary nominal 的有限跨 Cone shape support 由定义方提供，generic/structural helper 按原 application ODR 归属；未用模板、纯语义 identity 或查询本身不产生空机器定义。

### 2.14 Runtime image 与初始化

runtime metadata ABI 9、runtime ABI contract 30 的字段与启动协议由运行时规范 2.7、2.8、9.2 定义。RootEntry 的物理大小为 128 bytes，旧版 ABI 不可混用。每个 `.slib` 的机器对象保存完整候选 registration，metadata 保存链接所需的 identity 与物理归属；最终 image 表只包含选中的 producer records。

image、root entry、main body、root gateway、initializer、ensure、startup gateway 和 release hook 使用各自身份。根、cell、storage 和失败状态完整关联，不以可空函数指针或命名约定推断 unit kind。

全部 image 和 roots 在首个 managed initializer 前登记，原始 argc/argv 在 eager 初始化前保存。每次 eager/root gateway 有独立 native→managed boundary 和入口 poll；全部 eager 成功后 root gateway 才构造其需要的 argv 数组并调用 main，所有异常在回到 C 前处理完成。Unit 成功写退出码 0，Int 成功写完整实际值，两者均返回 status 0；failure root 发布后返回 status 1，runtime 忽略退出码槽并按统一失败路径以 1 终止。构造、static initial state、failure root 与 Context cells 遵守运行时规范，不由最终 linker 补造缺失语义。

初始化顺序遵守语言规范 12.3；普通 lazy ensure 可以因实际访问执行。最终链接保证同一 ODR unit 共享唯一状态，runtime 按实际加载地址和当前状态协调。

codegen 对真实 `ManagedRuntimeFunction::InitializationEnter` 的普通调用生成就绪快读：沿已登记 descriptor 的 cell 指针，对 offset 0 的 8-byte state 作 acquire load，Initialized 时直接产生 typed Ready 结果。其余状态保留原 managed call 及 safepoint，仅在慢边物化／恢复根，再与 Ready 合流；普通 extern 和 invoke 后备不改变协议。全部 profile 使用同一原子发布契约和 cell 布局，不在 MIR 伪造无 GC effect，也不删除 ensure 的入口 poll。后续 storage 读取仍使用原精确 root 及重定位路径；状态快读不缓存 singleton 对象地址，不重新验证 descriptor 或另建跨 Cone 初始化状态。旧 runtime contract 17 的普通 state 写入不兼容，按既有 ABI 指纹检查要求重建。

### 2.15 集合、Char 与字符串

List、MutableList、ArrayList、StringBuilder 及迭代器是普通 core 类型；数组和 String 的属性/成员也从真实声明选择，不能按 `.size`、`.charLength` 等拼写跳过访问与类型检查。

ArrayGenerate 保存完整目标 application、元素类型、Long count 与 ordinary `(Long) -> T` initializer。负长度、求值顺序、逐索引初始化和异常遵守语言规范 10.6；ZST 只省略 payload，不省略 initializer 调用。生成控制流在任意 GC/异常点保持数组可扫描。

Char 为独立 nominal identity，常量和各层表示保存合法 Unicode scalar，物理布局为 GC-free u32。它不与 UInt 合并，Option、数组、装箱与 ABI 使用实际 Char 类型；不因表示相同增加整数算术或 CharRange。

String 的普通 getCharAt(idx) 方法和 charLength 属性按 Unicode scalar，byteLength 使用物理 UTF-8 byte count；String 不提供 get／下标运算或 length 别名。公开 API 从 core 真实声明导出并跨 Cone 消费，内部 UTF-8 表示与普通 helper 的 Option 结果保持；定位算法由 core 正文实现，异常边界见运行时规范第 6 章。List getter 仍为可抛出的普通 managed 调用。

String 相等的同引用、长度不等及空串判断使用普通 core 正文；剩余比较沿真实 string_compare_to intrinsic。短 hash 使用有界普通正文，较长文本共享普通 core FNV-1a 循环，位宽、字节顺序与回绕结果保持。调用点常量事实保存实际 StringConstId，复用普通内联与控制流简化；只有真实 typed String 字节长度、合法定点字节读取和比较可按已知内容折叠，普通 extern 不参与。常量身份与 provider 归属保持，不能因文本相等推导对象身份相同，也不能因常量 id 不同推导身份不同。

有界 hash 按实际 byte length 选择普通 core 正文，当前至多展开 32 字节，并在有效字节处理完毕时直接返回，避免短输入经过剩余的空判断。长度界限是库算法的代码体积取舍，不是新的 intrinsic、调用协议或内联特判。同一正文适用于直接调用、接口分派和跨 Cone 消费，常量点仍由既有调用点评分决定是否展开。扩大范围时须同时比较界限两侧的动态输入、长输入和调用者代码增长；超出范围时沿共享循环完整计算一次，不先扫描前缀再重新计算。各 profile 保持相同 hash 算法与结果，managed 循环遵守普通 poll 和输入重定位合同。

release 可对已知不超过八字节的常量比较生成有长度保护的定宽读取；只读取实际存在的字节，并按无符号 UTF-8 字节的字典序产生原比较结果。其他输入保留共享比较。StringCharacterAtByte 继续使用已有 NoGC leaf；单独 ASCII 展开须按代表性 ASCII／Unicode 调用点的实际收益选择，不能省略边界检查或改变原诊断。以上读取不增加 safepoint、roots 或跨 safepoint 内部指针；没有新增 intrinsic 身份、runtime 符号或产物格式。展开后的普通正文仍由已有调用点评分选择，不按 core 函数名强制内联。

release 在普通 MIR 内联与局部分配优化之后识别真实 `RuntimeFn::StringConcat` 调用，融合同一基本块中单独使用的连续拼接。分析只穿过稳定局部值复制和常量准备；其他调用、可变存储写入、可能抛出或改变绑定的操作及控制流边界结束融合。中间结果及其别名必须具有唯一写入且只读一次，不能删除其他位置仍使用的结果。输入按原顺序各求值一次，保留共享输入与重复片段；融合不改变异常前的副作用前缀。首版最多展开八个片段，合并不超过 64 bytes 的相邻常量片段；更长的常量仍引用原 String。普通二元拼接只在静态热循环调用点或全部输入为短常量时使用同一生成路径；热度复用内联器的循环／冷出口分析，冷点及不合格链保留共享 runtime 后备。

融合结果是编译器内部的 typed StringConcat 值操作，区分已求值 String 输入与直接 UTF-8 常量片段。LIR 携带实际 String TD、分配 safepoint 及输入 live set；codegen checked 累计长度，一次分配结果，写入 byte count 并按序复制。所有可能移动的输入在分配后从规范 root 重读，再计算 bytes 地址；长度可跨 safepoint 保留，内部指针不可。常量片段直接写入结果，不创建额外 immortal 对象或 managed root。此操作不进入 `.slib` 公共格式，不改变 String 布局、实体身份或旧二元 runtime ABI；debug 和 release 继续消费相同库合同。普通拼接与下述短插值展开共用该融合；它不改写 StringBuilder 或插值值的格式化算法。

已知表示的字节复制使用真实 core intrinsic：`string_copy_utf8_unchecked(Ptr<Byte>, Long)`、`string_slice_bytes(String, Long, Long)`、`string_from_bytes(Array<Byte>)` 返回 String；`string_to_bytes(String)` 与 `string_to_mutable_bytes(String)` 分别返回 exact Array<Byte>／MutableArray<Byte>。第一项为 Unsafe，其指针在分配期间必须稳定、可读，长度非负且内容为合法 UTF-8；其余内部操作由调用的普通 core 正文保证 UTF-8 与范围前置条件。String intrinsic wire tags 4～8 按上述顺序追加，原 tags 1～3 不变；源码及跨 Cone 正文均按 typed 身份降低，不改变同名普通 extern 的语义。各 profile 使用同一 core 协议，配套 core 与依赖库按常规依赖更新重建；`.slib` 容器格式和外部 runtime ABI 不变。

MIR 的 StringCopy 保存完整操作与按序求值的输入，LIR 携带实际目标 TD／数组布局、源表示和分配根计划。一次分配后设置长度，再复制精确字节数；无引用 payload 不需要写屏障。受托管的输入即使源码调用后已死，也必须保留至展开后的复制完成，并在可能 GC 的分配后使用重定位结果计算源地址。原生指针不充当 managed root。byte array 结果始终具有独立存储，不能返回 String 的内部视图；普通 List 输入仍先执行原有逐项快照，保留 getter 的次数、顺序与异常，不对任意 initializer 或 List 绕过调用。

普通 UTF-8 构造还使用内部 `string_allocate_zeroed(Long): String`、unsafe `string_byte_at_unchecked(String, Long): Byte` 与 unsafe `string_set_byte_unchecked(String, Long, Byte): Unit`（String intrinsic tags 9～11）。零字节分配作为 StringCopy 的零初始化源复用现有大小检查、TD、TLAB／慢分配，不增加第二次清零；MIR 的 typed 字节操作及 LIR 的字节 load／store 只表达既有表示与前置条件，不增加扫描算法或 C 后备。字节范围由普通 core 正文保证，写只用于私有构造中的对象，见 runtime 2.4；普通受检读取仍可用于其他内部调用。所有 profile、跨 Cone 正文导出及产物消费遵守同一操作合同，配套 core／依赖产物按现有输入更新重建。

StringBuilder 使用四项内部 unsafe 表示复制（String intrinsic tags 12～15，原 tags 不变）：`string_from_byte_prefix_unchecked(MutableArray<UInt8>, Long): String`、`string_copy_to_bytes_unchecked(String, MutableArray<UInt8>, Long): Unit`、`bytes_copy_prefix_unchecked(MutableArray<UInt8>, MutableArray<UInt8>, Long): Unit`、`bytes_copy_raw_unchecked(Ptr<Byte>, Long, MutableArray<UInt8>, Long): Unit`。数组前缀长度和目标起始位置由普通 core 保证有效；写入源／目标范围不重叠。String 来源复制全部字节，数组来源复制指定前缀，原生来源复制指定长度。MIR StringCopy 保存完整源、目标与按序求值的操作数；分配结果使用原 LIR StringCopy 与根计划，写已有数组使用无 safepoint 的 typed ByteCopy。目标写入是不可删除、不可视作纯读取的副作用。托管输入以实际数组布局定位，分配后从重定位引用取地址，原生源要求跨分配稳定；不引入每次追加的 pin／callback 或通用容器 runtime。

Char 的单标量 UTF-8 编码使用普通 core 正文与四字节 GC-free 局部缓冲，复用上述复制；零字节不是终止符。整数十进制编码复用同一稳定缓冲复制。调用点内联与常量传播决定编码正文是否展开，LLVM 按已知长度降低 memcpy；动态长复制保留共享 memcpy，不复制扫描算法。debug 使用普通分配后备，release 复用已有 TLAB／慢路径。fromChars、严格／lossy UTF-8 解码与字符索引定位共享普通 core helper，不再调用旧 C 算法。

普通正文对已解析的普通方法 application 与直接函数引用使用同一可移植调用合同；不能因为属性 getter 或方法以本地 method application 表示，就丢弃包含该调用的完整正文。沿既有引用记录保留 exact owner、receiver、参数和访问器身份，泛型替换仍由原具体化流程处理；正文可用性不代替 consumer 的调用点评分。

Char 字面量、已知 Char 的 code 和合法常量的 unchecked 标量构造参与已有 MIR 常量事实及调用点评估。只折叠已知合法 Unicode 标量，不把非法 unchecked 输入变成另一标量；动态检查、分配与复制仍保留原有副作用。

严格、可空和 lossy UTF-8 转换按语言规范 11.4 的实际 core 声明组合普通扫描、校验和构造正文。CharacterCodingException 是普通 core 异常，成功/失败结果通过完整 typed Scoop ABI 传递；不按方法短名增加编译器特判或新的异常角色。lossy 的 maximal subpart 与严格路径的 byteOffset 使用同一个至多读取四字节的 NoGC helper，生成代码不得用截断、locale 转码或忽略非法字节替代。完整扫描保持 managed poll；字节借用、结果 String 与跨分配保存的 managed 输入遵守普通 roots、pin 与 relocation 合同。

C 字符串接口由普通 core 代码组合：withCString 检查 NUL，复制到带终止符的 MutableArray<UInt8>，再通过语言规范 13.11 借用；fromCString 通过普通 C ABI 的 strlen 取得长度，复用严格 pointer 解码。strlen 按 GCLeaf 合同调用；没有独立的编译器 C 字符串类型、隐式编码转换或栈缓冲优化。

编译器为异常边生成的零参数构造适配器不参加源码构造重载选择；core 自身的函数体也遵守同一规则。源码调用仍选择实际 source constructor，并按普通规则求值缺省实参，泛型／默认模板保存该源码调用。适配器继续由编译器异常边的完整 typed 引用使用。
f-string 按源码顺序保存 text/expression part 与原位置，表达式与对应 ToString 转换交错执行，保持异常和挂起行为。移除空文本、合并不超过 64 bytes 的相邻文本后，有效片段不超过八个时，HIR 使用 hygienic 不可变局部变量保存各转换结果，再生成已有 typed 二元 StringConcat 的连续单用链；转换按真实 core ToString conformance 绑定，不统一装箱为接口，也不调用用户同名 extension。零段使用空串，单段直接返回文本或转换结果；f-string 的 const、NoGC 与 suspend 合法性仍先检查。超过八段按普通 core StringBuilder 的构造、append 与 toString 下降；上限是代码展开策略，不限制程序合法性或输出字节数。

release 的短插值复用上述 MIR 融合，二至八段一次分配最终结果；两段冷调用点可以保留一次二元 runtime 后备。debug 使用相同的前端展开与现有二元后备，不承诺融合。所有 String 临时值沿普通 roots 和协程 frame 保活，最终分配后重新取得复制地址。generic、默认值与跨 Cone 正文保存原 callable、常量 owner、求值位置和已有 primitive 操作，不新增 HIR 正文格式。成功 HIR 不保留待后端解释的字符串插值或 StringBuilder 指令；stage 之间只传递完整 typed IR。

普通容器布局和内部存储策略不属于编译器协议。其模板、接口调用、快照和字符串构建使用普通类型/ABI/GC 合同。

数组构造若初始化器是正文仅返回已知全零值的直接 lambda（或其不可变局部副本），可复用分配器已经清零的元素存储，省去逐项调用／写零循环。支持无副作用的 MaybeUninit.uninit、零整数、false、Unit 及其全零值组合；保留 count 与 initializer 表达式的求值、负数检查和分配。其他 initializer（包括返回零但含用户调用、写入或异常的正文）仍逐项执行，不依类型名跳过任意用户代码。

ArrayList 以普通 MutableArray<MaybeUninit<T>> 实现容量存储，elementCount 与扩容、插删、清零均为 core 正文；编译器不按 ArrayList 名称提供特殊 shape、分配或 initialized-length 字段。ArrayList 将容量检查与冷扩容正文分开，扩容先取得清零数组，再复制有效前缀；循环可捕获稳定的 backing 与计数，以复用普通范围分析。StringBuilder 以普通 MutableArray<UInt8> 和 byteCount 保存内容，公开容量参数、primitive 重载和快照行为见语言规范 11.6。整数重载复用既有 writer，先格式化到 GC-free 栈缓冲，再按实际长度复制；不得将可移动数组的未固定内部地址传入 managed formatter。

### 2.16 Task-local Context

Context 的语言行为见语言规范 8.3.5，运行时 key、scope、task 与 callback ABI 见运行时规范第 9 章。HIR 保存 exact key、完整绑定值类型及已选 Context 操作；泛型条件在实际 application 中替换，不按运行期对象类型猜 key。

Context runtime 的擦除 binding 槽保存单字 object；compiler-owned `context-binding-ref` 使用与 Any 相同的 managed-pointer 表示及独立 generated identity，不冒充源码空接口。绑定接口时提取 object，按 exact key 取出后为静态接口重建 itab；该重建为 NoGC。用户可见的参数、结果和字段仍保存完整双字接口。

Context scope 在完整 value 求值与成功 push 后才生效。真实离开 scope 的正常结果、return、break、continue 和异常都恢复 mark；挂起不退出 scope，已消费的 mark 不继续保活旧绑定。

协程 frame 保存完整 TaskContext 与跨挂起 mark。只有取得实际恢复权的入口切换到 frame context，所有 driver 出口恢复 previous context。普通 closure 只捕获实际词法值，callback 注册另按语言规定捕获 binding snapshot。

MIR 中已选的 Context fork 在 LIR 根计划形成前降低为普通 managed 分配和 root 字段写入。分配、字段位置与 TD／scan 共用实际 generated TaskContext 的对象布局；当前支持目标上为 16 字节对象头加一个 8 字节 managed root。所有 profile 使用同一布局与基本操作，release 复用已有 TLAB 快路径。输入 snapshot 只求值一次，并作为分配后写入所需的活跃引用参与根计划；若分配触发搬迁，字段必须写入恢复后的引用，并执行普通含引用写屏障。即使 snapshot 为空也建立独立的 TaskContext，不合并任务身份。该降低只处理 typed Context 操作，不改变同名普通 extern 的协议；runtime 的 fork 后备、初始 root 创建及共享绑定树操作继续有效，不增加 TLS 字段、隐藏参数或产物 ABI。

MIR Context snapshot 同样在 LIR 根计划前降低为 typed current、判空和普通 HeapLoad，字段偏移复用 generated TaskContext 的同一布局。release codegen 将真实 NoGC runtime 身份的 current／enter／leave 普通调用生成对运行时规范 9.2 原根槽的读写；普通 extern、invoke 和 debug 保留 C 后备。它们的 managed 结果及 previous guard 继续参与后续根计划与 SSA 重定位，不增设独立的引用缓存。槽读取和写入保留实际内存效果，可能 GC 或切换绑定后从当前槽或实际 relocate 结果取得引用。

ARM64 release 复用已有分配 context 的首次使用机制，在一次 invocation 中惰性取得并复用 native 根槽地址；x86_64 直接读取 TLS 指针。缓存只存 AS0 地址，初次 TLS 取址不提前到从未执行该操作的路径；槽内 managed 引用和 TaskContext 字段不作相同缓存。runtime ABI contract 20 为该 TLS data symbol 登记 `RuntimeAbiSymbolV1` tag 34，所有 profile 的符号、大小和 attach／detach 合同一致，旧 sysroot 与产物按原 ABI 边界重建；对象表示、GC roots、metadata ABI 9 和 `.slib` 容器保持。

语言规范 11.9.1～11.9.2 的 Job、任务树和取消适配是普通 core 源码，使用精确 Job Context binding、已有原子、异常与 continuation。编译器不按这些库名称生成专用身份、调度或取消路径；跨 Cone 按普通泛型模板、可见性与调用规则消费。

每个 body 的 Context key uses 与 slot cells 属于该 body 的物理选择，callable registration 保存完整关联。最终登记在首个 gateway 前填充 cells，同一 exact key 在程序中映射到同一 slot。

### 2.17 静态类型描述与编码方法

静态描述使用原声明、字段、variant、logical property、constructor/default 和 annotation 事实。语言规范 9.6 的 TypeInfo/TypeRef/TypeShape 及成员、注解记录由普通 core 类型承载；编译器内部查询仍为原 HIR 数据的视图，公开描述在具体类型已知时生成普通值和当前层 describe 闭包，不引入 CTFE。引用的相等编码既有 typed persistent/exact identity，不能用名称或地址回退；成员的声明处 Option 事实复用默认 codec 的原类型查询。查询不执行被描述声明的 constructor/default/getter/companion，也不扩张可见性。

具体条件接口的判定按原条件 id 与完整具体实参复用；接口表、成员具体化和公开类型描述共享该结果。类型描述在替换条件目标或属性签名前过滤不适用项，避免展开不存在的 companion 投影；静态声明视图本身不引入条件求解器。条件接口目标的 type-use 注解沿用 nominal 的 Supertype 槽，排在普通直接父类型之后，按实现块声明序编号，跨 Cone 保留相同对应关系。

公开描述的递归关系由普通 TypeRef 和闭包断开，引用构造不调用其 describe。描述函数及子引用闭包沿既有泛型具体化机制生成：同型递归复用已有请求，实参沿递归环增长时使用现有 polymorphic-recursion 诊断，不加深度预算或内容缺失的占位描述。成员要求展开后的补充递归检查只诊断包含实际要求展开边的环，不因同一 Cone 存在 requires 就将普通类型描述及其有限子引用之间的生成依赖视为多态递归。公开模型的普通数据声明本身不改变 `.slib` wire 格式或 runtime ABI；查询的 HIR 登记及产物依赖必须使用实际声明身份。

`typeInfo<T>` 是普通 core 泛型正文；内部 `__scoopTypeReference<T>()` 先调用 `type_reference` 取得 TypeReferenceData，再用普通 TypeRef constructor 和调用 `type_description<T>` 的普通闭包构造引用。引用数据由 identity、displayName、nominal、arguments 四项组成；成员引用为各自独立的普通 struct。两个 intrinsic 在 concrete HIR 中生成普通正文，不进入 MIR。引用构造函数作为 core protocol 的 field 10 保存实际 generic callable identity，下游按此身份请求模板，不按名称猜函数。intrinsic wire tags 26／27 分别表示引用数据和当前层描述；因此 HIR core-bootstrap-interface 升至 16，cross-cone-interface 升至 77。注解声明的惰性描述正文使用 `GeneratedCallableKey::AnnotationDescription`（tag 18），键保存实际 PersistentAnnotationId 与查询所在 callable template；无参数辅助函数继承该 callable 的实例化上下文，复用其普通 ODR 归属。递归注解描述保持同一查询宿主，不逐层产生新的宿主。它不是 runtime 注册表。HIR／MIR／LIR identity-foundation 分别升至 10／8／9；runtime ABI 保持。cross-cone-interface/77 的 property declaration field 9 保存已经绑定的原 property override 身份序列，供描述直接读取。

`annotation_key`（intrinsic tag 28）只用于标识公开 core 查询声明。HIR lowering 先进行普通 callee 绑定，再将声明实参解析为 PersistentAnnotationId，输出 AnnotationKey 表达式；不制造普通类型实参或运行时 intrinsic 调用。共享正文使用表达式 tag 75 保存这个 typed id，默认参数的直接引用表 field 7 保存注解声明引用及其正常定义来源。引用闭包按已有声明依赖规则保留注解，具体化复用上述普通 AnnotationKey 和惰性描述辅助函数。cross-cone-interface 升至 78，cross-cone-type-semantics 升至 30；core protocol 结构、identity foundation 与 runtime ABI 不变。

描述生成使用已经绑定的声明事实。正常类型加载补齐注解参数、计算属性结果和未调用构造器参数的类型，避免只有布局使用过的类型才能被描述；不调用对应正文或重新执行注解求值。

语言规范 11.17 的 inspect 合成在 HIR lowering 复用普通 requirement 选择、方法签名登记、成员访问和显式依赖选择；生成普通 typed 读取、活动 variant 分支、闭包及 core 视图构造。合成作用域保持实现声明的实际 owner 与 source file，不增加成员访问许可。共享正文保存已选成员／inspector 与词法捕获，后续泛型具体化只替换参数。ValueView、集合 inspector 与 DebugPrinter 是普通 core 源码；MIR、LIR 与 runtime 不解释 DebugIgnore／DebugRedact 或读取反射字段。没有专用布局、全局 inspector 表或新的 wire 格式。

语言规范 12.7 的 scoop.cli 完全由普通库源码实现：ClassInfo.isFinal 直接读取已有 class modifier；schema 使用描述中的成员／primary constructor 关联与原声明 Option 事实，在构造边界检查一次配置。argv 归一化、稳定 Decoder、数值文本及 help 不进入编译器 stage，复用已有浮点文本 extern ABI。

concrete HIR 的 TypeIdentityLiteral 指向已完全绑定的 TypeId，值类型固定为 String。MIR lower 只从现成 exact-type 表读取该 id 的十六进制 bytes 并生成普通 StringConst；不重新计算身份、遍历字段或解释描述。这让描述函数沿同一实例化队列生成，身份仍由原最终关系统一建立。

annotation 声明使用独立 PersistentAnnotationId。普通值参数保存完整 signature type key；annotation 原子及其数组参数使用独立的 annotation identity 和实际 core Array 模板身份，不把 annotation class 编成 runtime TypeId。应用保存实际 typed target 和按参数顺序补齐的静态值。CanonicalConstValueV1 保持原标量用途；CanonicalAnnotationValueV1 以封闭的 Scalar、IntegralEnum、Annotation、Array 保存静态值。整型枚举值保留完整 application signature、variant identity 和底层整数位型，嵌套应用保存实际 annotation identity 和完整参数；数组保存明确元素类型、有序值以及逐元素的 SourceIdentity 和 span，默认值保留定义处来源。共享 reader 在既有边界检查引用、枚举 variant/数值、嵌套参数及 tag/payload 一致性，不重放名称查找或默认值求值。参数/default 的循环依赖在源码定义处诊断。名义类型字段保持原身份，generic 字段保留 binder；tuple 保留位置，class 保存真实 field/property/constructor 关联。

HIR foundation field 35 保存 annotation declaration keys，共有源码接口 field 14 保存 annotation declarations 与有序 applications；实际 annotation 依赖使用 AnnotationDependency tag 11。NominalDeclarationDetails field 11 保存可选 primary constructor 及声明序参数到 property 的映射，普通参数为空，val/var 参数引用原 logical property。

注解 target 保留原声明的类型化身份：named callable、constructor、accessor、annotation 和 typealias 使用各自声明身份；普通值参数以 callable 身份和源码参数序号选择，receiver 单独选择。局部 callable 复用既有 lexical generated callable identity。字段使用现有 field/variant-field identity；顶层或泛型 property 的存储以原 property 身份与 backing/delegate 类别表达。use-site 只在 HIR 名称绑定时选择真实已存在的实体，不新增运行期布局或隐藏参数。共享接口保存这些已绑定位置；reader 检查实际声明引用和共享参数表中的索引；局部 callable 复用已绑定的源码参数位置，不另存一份只用于重放检查的签名表，也不重放 accessor body 或默认目标选择。

annotation 声明保存已绑定的允许目标集合与 repeatable 属性。集合使用语言规范 9.4 的十二种类别，元注解应用本身仍是普通有序静态数据；不在 reader 中重跑 core 名称解析或元注解求值。所有 annotation 参数/default 和元注解先完成绑定，再按最终声明属性检查前向应用；普通使用处按实际目标和 annotation identity 检查。共享 reader 在原注解数据边界检查目标类别、重复应用与该声明属性一致。annotation declaration record 的 field 5 保存这两个属性。

类型使用注解独立于 TypeId 保存。每个使用记录以实际 SourceIdentity 和类型语法范围组成类型化位置身份，包含已解析的完整类型、binder 与使用树内的注解路径；没有注解的节点由该完整类型给出，不另存一套语义类型图。路径区分类型实参、tuple 元素、函数 receiver/参数/结果与 pointer pointee。字段/property/完整参数/结果、alias 目标等描述位置通过实际 typed 声明和位置选择关联使用记录，不能由名称或源码范围包含关系猜测宿主。

HIR lower 在正常类型解析过程中记录这些位置和 alias 的实际替换关系，注解值待相应 schema/常量完成后仅绑定一次。alias 与泛型替换复用原 binder 映射，合并相应路径的静态应用，不污染共享类型或重跑类型检查。annotation 参数的 annotation class 及其 Array 使用独立 annotation identity，不制造 runtime TypeId；用于确认 Array alias 转发关系的临时参数不进入静态描述。

共享注解表增加类型使用记录与声明位置关联，完整类型用既有 signature key 与独立 annotation 参数类型表示，应用沿用 canonical 静态值并保留各自来源。reader 在原静态数据边界检查实际引用、binder、路径及 TYPE/Repeatable 规则，不重放 alias 展开、默认值或布局。静态查询从同一记录取得类型、各节点注解和参数替换结果。该变更使用 cross-cone-interface/78；当前版本见 2.6 与 2.11，旧产物须重建。MIR/LIR 和 runtime ABI 保持不变。

property accessor 的 Storage、Constant、Body、AbstractSlot、StorageBody tags 分别为 1..5。StorageBody 表示普通存储语义产生的实际 accessor body，不能通过识别机器正文推断。NominalDeclarationDetails field 12 保存 sealed 直接子类型（2.2）；generated callable tag 17 为 TestEntry，tag 18 为 AnnotationDescription。

companion 继承直接宿主的 binder 与 bound，不捕获宿主实例或 primary 参数值；ordinary static nested declaration 隔断该 binder。companion declaration、完整 application 和 concrete singleton 分别有身份，泛型状态共享规则见 2.13。

ValueEncoder<R>.encode 与 ValueDecoder<R>.decode 是实际 companion/codec 的普通方法，receiver 与数据参数/结果独立。手写、继承与 default 先按语言规则选定，仅为缺失的 requirement 合成方法；字段 codec、variant、constructor 和默认值在定义处完成选择。

核心Option/Array/MutableArray/ArrayList的编码／解码字段先检查完整字段的显式依赖和递归this，再递归选择元素codec并调用既有工厂，最后才考虑其他名义类型的默认companion（语言规范11.13.4）。新增核心容器的条件companion关系不能遮蔽已注入的元素策略；选择仍只产生已有普通调用和投影，不新增IR或runtime合同。ValueInspector的既有选择规则不变。

Serialize 的 rename／required／emitNull／ignore 来自同一已绑定 annotation 配置，当前源码与依赖使用已有静态值，不扩展 wire 参数值域。字段的声明处 Option 事实从原成员已绑定、展开 alias 的类型取得；具体化后不重新推断。None 省略只检查 default 的直接 typed None 表达式，不执行 default 或通用常量等价性分析。合成结果为普通 typed 字段访问、Option presence 分支、None 值、调用、控制流、闭包与构造；encode/decode 分别保留语言规范 11.13 的求值顺序、缺失字段处理和可见性。Export HIR 保存完整正文及支持声明，下游只实例化，不重新解释 annotation 或搜索 codec。

codec 不因数据继承关系自动传递，不能按 value 的动态类型改选 companion。不同 codec 声明即使处理同一 R 也保持独立身份；数据类型本身不因此获得编码接口。tuple、Unit 和容器组合使用普通 helper，JSON 是普通库，不增加专用 MIR、runtime type table 或 C 入口。

### 2.18 Float / Double

语言语义见语言规范 11.2.2，运行边界见运行时规范 6.1。Float/Double 分别为 F32/F64，alias 不建立新 nominal identity。literal 与 const 按目标精度直接求值，成功输出保存原始同宽 bits，不经宿主浮点近似。

算术、默认比较、分类与转换使用明确 typed operation。`compareTo` 返回 Long；六种默认 predicate 与其符号保持一致。显式 `ieeeCompare` 通过普通 core body 组合分类与比较，返回普通 IeeeComparison enum，不增加 enum 专用 intrinsic。浮点除零不使用整数异常规则；相等不能替换为 identity 或位型相等。所有 storage、aggregate、数组、closure、frame、boxing 和 C ABI 保留完整精度及 exact type。

浮点操作不得使用 fast、nnan、ninf、nsz、reassoc 或 contract 等改变语言语义的假设。默认比较将所有 NaN 合并并排在正无穷之后，将正负零合并；const 与后端使用相同规则。显式 IEEE 比较的 Unordered 不满足有序关系。float→integer 按目标位宽饱和，不生成越界 poison 或先转 64-bit 再截断；同格式转换保持 bits。

C storage 使用真实 float/double；fmodf/fmod 的 NoGC 签名分别为 `(F32,F32)->F32`、`(F64,F64)->F64`，其生成由 LLVM 负责，按实际未定义符号进入普通 native 链接。内部常量池与机器局部 label 不获得源码实体身份。

浮点字符串化通过普通 core 正文组合 Ryu 的真实 C ABI 入口、GC-free 栈缓冲和既有 typed StringCopy。文本格式 helper 仅消费 Ryu 的有界字节结果，可按普通 NoGC／Unsafe 规则实现；内联继续按调用点选择，不新增浮点格式 intrinsic 或按符号名改写 extern。Ryu 的最短转换不复制进 codegen。旧 C String 返回包装不再发射或提供；JSON／CLI 的实际解析 Option ABI 适配保留，短输入栈缓冲与动态慢路径见运行时规范 6.1。配套 core 与依赖按正常输入更新重建，外部对象布局和元数据 ABI 保持。

metadata 的相等、去重与 hash 比较 raw bits，包括 signed zero 与 NaN；这不赋予语言 Float/Double Hash conformance。各 wire 域为：

| wire 域 | tag / payload |
| --- | --- |
| FloatKind | unsigned 32 或 64 |
| 浮点常量 | `{1=kind, 2=同宽 raw bits}`，F32 不接受超出 32 bits 的值。 |
| HIR literal / nominal representation / canonical const | 67 / 10 / 5 |
| MIR intrinsic representation | 6 |
| LIR scalar / canonical type / canonical value / shape representation | 4 / 13 或 14 / 14 / 8 |
| canonical C storage 与 LIR C type | 6 |
| HIR intrinsic | 22 |
| HIR expression unary / binary / conversion | 68 / 69 / 70 |
| LIR instruction unary / binary / conversion | 71 / 72 / 73 |
| literal equality pattern | 4，payload 为 FloatKind。 |
| object relocation constant-pool target | 15，payload 为实际常量 ByteSpan。 |

FloatBinaryOperator 的算术 wire tags 1～5 不变；旧 IEEE predicates 的 6～11 与 totalOrder 的 12 退役，不再解码。CompareTo 使用 13，默认 Equal/NotEqual/Less/LessEqual/Greater/GreaterEqual 使用 14～19，避免消费旧 IR 时悄然改变其比较语义。受影响 core 与依赖须重建；沿用现有未知 tag 与 core 协议兼容性检查，不增加 runtime ABI。

浮点 literal 语法不能表示 NaN，有限 literal 集合不能覆盖完整浮点域，剩余域需要 wildcard。JSON 使用显式 codec 和目标精度直接解析，有限／非有限值遵守语言格式规则。

### 2.19 优化与物理定义选择

接口引用简化必须保留能够直接投影到目标父接口的已有 Child 视图；不得将其折回 Any 或其他单字来源后重新查表。表复用只保存静态 metadata SSA，不为避免查表延长对象副本的存活范围或新增 managed roots。

debug 使用 Scoop machine O0 与 generated-C O0，release 使用 O2；两者均完成正确代码生成所需的 SSA/GC 转换。runtime build 配置独立，默认 O2。优化模式不改变源码语义、persistent entity identity、shared state 或对外 ABI，兼容的不同优化产物可以共同链接。

Linux/amd64 的 managed 调用保留固定的 outgoing argument area，使运行时规范 3.2 的 `FP = SP + N - 8` 对每个 safepoint 都成立。LLVM 22.1 后端关闭 X86 call-frame size optimization，避免按值参数被改成调用点临时 push/pop；源码参数语义、calling convention 和其余 O2 优化保持。此设置在创建任何 target machine 前由 codegen 统一初始化。

LLVM 22.1 关闭寄存器分配后的独立 tail duplication pass 及机器基本块布局中的 tail duplication，避免在 roots 与 site identity 定稿后复制 STATEPOINT。只关闭布局阶段不够：独立 pass 也可能复制共同的终结调用或异常合流后的 Managed 调用。条件 poll 后接不返回的 Managed 调用也必须只保留一个实际调用点及一条对应 stackmap 记录；给 statepoint 添加 LLVM `noduplicate`／`convergent` 调用属性不能阻止该机器优化。此限制沿用统一的 target machine 初始化，保留其余 release O2 优化，不改变 SafepointId、返回地址或对象验证规则。

Linux/amd64 release 的最内层循环采用 LLVM 的 32 字节优先入口对齐（LLVM 22.1 的 `x86-experimental-pref-innermost-loop-alignment=5`），减少周边代码增长使短内层跨越取指块而造成的回退。此选择按真实机器循环生效，不依赖源码名称或 profile 数据，也不扩大到每个基本块和函数。入口填充与最终位置由 LLVM 统一生成，对象分区和链接保留其 section 对齐，stackmap／展开信息仍对应最终机器地址。不通过在热内层中插入条件分支填充解决该问题；优化模式不改变调用 ABI、GC roots 或可观察语义。

普通优化可以消除已证明不可达的代码及 site，但保留实际求值、异常、Context、initialization、release-ready 与严格浮点语义。类型错误不能因优化删除代码而消失。

release 的普通 LLVM 优化按 callable 选择。typed LIR effect 为 NoGc 且该 callable 的实际调用／poll 合同不含任何 safepoint 时，可以运行循环规范化、归纳变量化简、LICM 及 loop／SLP vectorization；不能仅凭优化前没有 LLVM statepoint 指令推断资格。其余定义使用不复制 site／恢复标记、不移动握手位置的标量管线，包括表达式重结合、关闭 PRE 和回边加载拆分的 GVN、位级死代码消除。严格浮点不启用 fast-math；正常 Managed 回边 poll、原子顺序、invoke 出口及逻辑根恢复保持。普通优化后保留实际 site 与逻辑根身份，再执行 RS4GC 并从其 relocation 结果记录物理根数；机器层限制继续适用。

codegen 在生成带 SafepointId 的普通调用、显式 statepoint 以及保留逻辑根身份的 llvm.fake.use 时，统一附加 LLVM noduplicate 调用属性，使 LLVM IR 变换直接遵守调用点与恢复标记不可复制的约束。优化仍可消除已证明不可达的完整调用点及对应恢复；普通 ID 值在编译器数据结构中的复制不代表产生新的调用点。GC 根定稿继续核对实际保留的 site，机器层仍遵守前述 LLVM 22.1 限制。

两条 release 管线均在 SROA／mem2reg 与初始表达式化简后执行 constraint-elimination、correlated-propagation 和 SCCP，再用 InstCombine、受限 GVN、BDCE、DSE／ADCE、MemorySSA early-CSE 及不可达块清理收尾。范围事实只来自实际支配条件和对应整数语义，不凭无溢出假设删除检查。NoGc 在同一标量阶段之后执行其既有循环／向量优化；不重复运行整个标量管线。Managed 不因此启用一般 SimplifyCFG、循环复制或跳转线程化。机器 O2 不等于 LLVM IR 的 default<O2>；不能用默认 O2／O3 替代这里的管线选择。NoGc／gc-leaf 本身也不提供 readonly、无副作用或内存不相交的事实。

release 的短路条件与合流形成的不可达错误分支，在 GC 根和 safepoint 计划定稿前由普通 LLVM CFG 化简消除；仅做常量传播不足以识别这些关系。Managed 管线的该化简关闭块推测及公共指令提升／下沉，随后沿用 MemorySSA 整理根加载，保留仍可达的错误调用与根恢复。后段加载合并可能新暴露与支配分支矛盾的动态比较，收尾应消费这些路径约束，再折叠常量与删除不可达块。debug 保留原来的 O0 控制流和对应实际调用点。不能等机器发射删除调用后再忽略缺失的 stackmap，也不为已删除的分支登记运行时 safepoint。

Managed release 在全部标量及受限内层优化之后、最终根计划之前，再执行路径约束化简、SCCP 与不可达块清理，折叠后段 GVN／MemorySSA 优化新暴露的常量条件。单独的 unreachableblockelim 不承担常量终结指令折叠；不能让 RS4GC 首次删除这些路径而留下过时的 site／根描述。此收尾只做常量传播和死路径删除，不开放一般 CFG 复制，也不在根定稿之后补做普通优化。

两条 release 管线在上述 BDCE 与 DSE 之间执行 memcpyopt，消除可转发的按值中间副本并合并初始化；SROA 在前、死存储与死代码清理在后。消除的是可替换的复制，不改变值的独立性：源值在复制与消费之间被写入时仍保留快照。含引用副本保留 AS1 provenance、实际需要的 typed logical root restore、2.5 的标量重定位与必要存储／屏障／发布；不能以 NoGc 或 byval 推导整个调用无内存副作用。其普通优化仍先于最终根计划与 RS4GC，不为复制优化移动握手或复制 SafepointId。

上述同时满足 typed NoGc 与实际无 safepoint 条件的 release 函数，在 loop-rotate／LICM 后执行 simple-loop-unswitch（trivial 与受 LLVM 自身代码增长限制的 nontrivial 变换），再执行 indvars、loop-load-elim 与小规模完整 loop-unroll，最后执行既有 loop／SLP vectorization 及清理。unroll 仅考虑最多四次的完整展开，关闭 partial、runtime、upper-bound 展开及普通／profile peeling；不提高 LLVM 原有展开成本阈值。Managed 函数及带 site 的定义不进入这些变换。loop-idiom、jump-threading、aggressive-instcombine、vector-combine 仍关闭；不改变严格浮点、GC 合同或 poll 频率。

MIR 内联以 call site 为决策单位。稳定函数模板的摘要只保存正文体积、控制流、调用与可化简操作等事实，包括不依赖实参的化简结果；摘要可在多个调用点及迭代之间复用，不能按 callee 缓存接受／拒绝结论。正文是否存在、完整调用 ABI、递归及控制流是否合法先单独判断。普通候选没有统一的小函数尺寸或执行成本门槛；同一 callee 可以在热调用点展开，在普通或冷调用点保留。LLVM 的现有普通管线只执行函数内优化；此处复用 MIR 化简、去虚拟化、范围及局部分配证明，在生成 LIR roots 前完成内联，不在已有 safepoint 上追加普通 LLVM 内联器。

调用点使用当前 CFG 的自然循环深度、只通向异常／trap 的冷出口、实参常量、exact receiver、结果用途和 caller 当前规模，估计保留调用会留下的可消除工作。普通静态频次为 1，循环逐层提高频次并在固定深度饱和，冷出口降低频次；同 header 的多个回边只计一层，不可归约环保守计至少一层。热度放大可消除的调用／ABI 适配、已经按实参证明可折叠的运算与检查、可解析的动态分派及已有分配替换证明支持的收益。已经在独立正文中能够消除的操作不重复计收益；被常量条件排除的整块正文只降低预计体积，不能把原本不执行的分支全部计作运行收益。不把 callee 的全部执行成本、未知逃逸或未证明可省略的 GC roots、保存／重载及 safepoint 当作收益。

尺寸惩罚以实际化简后保留正文相对被替换调用的预计净增长为基础，控制流复杂度和 caller 当前规模增加惩罚。收益与惩罚使用简单的 MIR 权重，不代表机器周期或保证发生的复制次数。静态热度越高，可接受的净增长及复杂度越大；热度本身不省略冷分支的代码体积。普通内联按热度优先、同级原 block／statement 顺序选择，随后仍按原位置展开，保持求值次序。

增长按调用点所在的最内层自然循环累计；同 header 的多个回边属于同一区域，循环之外及没有自然 header 的不可归约环使用共享区域。复用现有支配与自然循环分析，区域由当前 caller 的 block 身份区分，不依赖源码或函数名称。内联新暴露的循环保有独立额度，循环包装本身的完整增长仍计入其原调用区域。各区域以原始 caller 大小为基准，并按调用点热度使用有限上限；整个 caller 另有累计总上限，防止不断暴露的新区域造成无界膨胀。区域与总增长均跨轮保留，重新分析、化简及删除代码不返还额度；克隆只追加 block，原 header 身份与累计记录保持。递归祖先与六层展开限制保持。这些限制只控制优化膨胀，不决定程序是否合法或 ABI；具体参数记录于性能文档，统一用于本 Cone、泛型及跨 Cone 正文。同步 suspend 在原有合法性范围内复用同一评分、静态热度与区域累计，尚无实参证明的收益记为零，保留独立的基础增长常数 96。

内联产生新调用点或改变常量、类型及控制流后，重新评估当前 caller 的相关调用点；稳定模板摘要仍复用。普通 release 默认启用上述低成本分析，不依赖 PGO，不新增试内联或多套策略框架。`--emit optimizations` 的调用点记录包含热度及来源、实参事实、结果用途、预计可消除工作、净增长、复杂度、caller 规模和接受／拒绝理由；增长估价同时记录区域 header、区域剩余与整个 caller 剩余，实际可用量为两者较小值。先判断调用目标、正文可用性与递归约束，再报告额度耗尽，不能用增长理由遮住没有正文或尚未解析的动态调用。未请求时不保存事件，任何诊断代码都不进入程序或库的机器产物。诊断格式可随模型更新，不进入 MIR／LIR 持久格式或 ABI。未来 profile 可以向同一收益模型提供频次事实。

内联选择重新分析当前 caller 时，可以利用先前展开暴露的 exact receiver，复用普通去虚拟化的完整签名与 dispatch 规则直接选择实现正文，不必等待下一轮才使用已知事实。入选调用保存相应的完整实参适配，包含具体 receiver 类型和必要 unbox；展开时使用这份调用，保留原参数求值与值快照。未知 receiver、缺失正文或不满足成本限制的调用保留现有处理。新直接目标同样受递归／展开祖先限制约束，不改变跨 Cone、接口或调用 ABI，也不新增持久分析摘要。

复制成本按 MIR 类型的已确定表示区分：以 intrinsic struct 身份保留的 Float、Double 和 Char 属于标量，局部赋值与参数不附加 aggregate copy 成本；普通 struct、tuple、enum 仍按值复制计价。此区分共用于普通及同步 suspend 内联，不改变字段访问权重、全局阈值或值的独立存储语义。

内联成本计算和展开后的化简复用局部值的数据流事实，转发同类型的纯局部副本；只有源值在对应路径保持不变时才替换，取地址的存储及循环中重新赋值的快照保守保留。通用实参数据流只传递常量与动态类型，不携带 caller 的局部变量 id。清理已经无读者的纯副本不删除实参副作用、分配、调用、内存读取或异常检查。只通向显式异常／trap 出口、且不能正常返回的分支可以降低执行成本权重；代码增长仍按全部实际保留的正文计入，冷分支不会因此被删除或取得无界展开资格。

含 EH 的候选可进一步使用实参的已知 exact receiver／closure 类型，沿现有完整签名与 dispatch 规则将唯一目标改为直接调用，并按实际操作证明某块不会发起 managed 异常。证明只接受已明确无抛出语义的标量运算、普通字段读写／捕获读取及相应局部存取；直接调用须有本次已可用的完整正文，其所有正常可达操作和调用同样满足条件，因此无需沿已证不可执行的异常边追踪处理块。未知动态目标、无正文的外部／native 调用、未闭合的递归调用以及可能失败的运算保持未知；NoGC、Managed、无写入或不捕获均不能替代无抛出事实。按需缓存同一次正文选择期间未变化的 callee 结论，不导入额外源码或新增持久效果格式。

仅在块内全部表达式、语句及终结指令均不发起异常时移除该块的 unwind 边，再用普通可达性与数据流清除因此失去入口的 catch／cleanup。正常路径的 finally／dispose、字段写入、参数求值与返回值快照保留；不能仅因某个 catch 很冷而删除它，也不把实际仍会抛出的操作标为 nounwind。若最终仍有 LandingPad／BeginCatch／EndCatch、CaughtException、Resume 或 Rethrow，则此项特化不授权展开。按真正留下的完整正文计算现有执行成本与累计增长，使用上述调用点评分，不预支尚无证明的消除收益。该 release 选择不改变 MIR／LIR 数据格式、跨 Cone 语义、精确 GC 根规则或 `.slib`／调用 ABI；debug 保留原始异常路径。

普通成本拒绝的循环内数组访问，可以复用本节计数循环的归纳变量、稳定长度与顺序快照证明，在选择内联时化简 callee 的实际分支。已证明非负且小于同一数组长度的索引，通过本次实参与形参的对应关系参与判断；caller 的 local 身份不写入 callee IR。callee 只保留能完全证明执行的无调用、无 EH 标量数组路径，未知条件、取址或需要新增运行期长度 guard 时仍保留原调用选择。实际入选的是已经删除不可达分支的正文，按同一套操作和参数复制成本计价；不以假设将来可能发生的消除抵扣仍会复制的错误路径。循环正文中的其他调用不使不可变数组长度失效，索引／上界重赋和取址仍按原证明排除；实参副作用、异常顺序、原 loop poll 和精确 roots 保持。该按调用点的化简使用同一评分、不返还之前的增长额度，也不改变普通数据流或跨 Cone／`.slib` ABI。

启用字段替换时，局部 class 的最后一个 receiver 调用可以与字段替换共同估价：复用局部分配来源、支配和循环分析，确认没有 release hook 或其他未匹配引用使用，且满足下文正常／异常路径的分配证明，并检查待展开正文只经纯别名和普通字段读写使用该 receiver。调用其他对象的方法不等于发布 receiver；返回／捕获／取址／身份观察或传出 receiver 则不适用。对这一调用点，执行成本和增长均按展开正文与同时消除的分配、别名及字段间接操作之差计算，执行成本保留既有冷分支权重；不按类型或方法名称特殊处理。估价采用同一套操作与值复制权重，保留初始化与实参副作用，含引用字段沿普通精确 roots 处理。每个选择至少扣减一单位已有 caller 增长额度，不返还以前的额度，使用同一调用点评分；真正的实例消除仍由下面的字段替换执行，接口、泛型与调用 ABI 不变。

普通 release MIR 优化交替执行去虚拟化、内联与局部化简，当前最多三轮；没有实际改写时提前结束，达到轮次上限不视为已收敛。内联使用同一阶段入口的稳定 callee 模板，跨轮保留函数事实摘要、caller 累计增长与每个块的展开祖先，化简不返还增长额度。每轮新直接边进入当前调用图的递归判断，重复展开祖先仍保留调用。下一轮只重新分析实际改变的函数；本期不消费更新过的 callee 正文，因而不为其建立额外 caller 摘要或通用 pass 调度框架。所有改写仍先于最终 LIR roots 生成，调用实参的顺序、异常出口及 poll 责任保持。

release 在同一迭代中折叠普通引用复制及多余的 Any 擦除。相同静态类型的副本可以关联同一来源；不同接口视图保留携带的表，class 收窄保留其静态类型，不能为折叠视图丢失 Child 的可投影表或 final class 的已知类型。清理已无读者的纯副本不增加新的 MIR local 或实体身份。

该局部折叠只消费 MIR 中已经合法的转换，不重新执行 smart cast 流分析或把接口命中当作唯一实现。赋值使涉及旧存储的关联失效；取地址的局部存储、字段／全局／指针读取与函数适配不作为可替换的纯局部副本。每个基本块独立建立来源关系，不把转换移出类型检查的成功分支，也不跨循环入口、合流或异常入口假定旧结果仍可用。清理无读者的纯引用／接口视图时保留其来源的分配、内存读取、调用和异常检查。

driver 向 lir-lower 显式传入当前优化模式。release 在单个 LIR 基本块内，为未取地址的同一 MIR 局部值、同一 exact interface 复用已经取得的 itab；赋值／调用结果写入使对应局部值的表缓存失效，进入其他 LIR 块即清空。复用结果只是 GC-free metadata SSA 值，普通调用与 GC 不改变该表；每次构造接口值仍从当前的 managed 引用取得 object，不能为缓存表额外延长一份对象副本的 GC 活跃范围。LIR lowering 内部产生的分支同样遵守块边界，不把分支内取得的表带出其支配范围。空接口的 null itab 仍是合法结果，遵守运行时规范 2.1。debug 不启用接口视图折叠或表复用，既有 `itable_lookup` 与 `is_instance` 入口签名保持。

release 的 LIR lowering 对接口条件分支及其否定形式分析成功路径。只有后续实际构造非空接口视图、来源是未取地址的同一局部值时，才将该检查改为 `scoop_rt_itable_find` 并保存静态 entry 指针。source 写入使对应事实失效；合流只保留所有可达普通前驱共有的同一次成功检查，循环按固定点处理，异常入口不继承这些事实。数据流不重新执行语言 smartcast 检查，不合并不同查询的结果或引入对象副本。普通调用及其正常继续路径可以复用已证明的 metadata；异常路径和新对象值继续走自己的检查。成功后按需加载 entry 的 slots，再复用既有父表投影及块内 itab 缓存。对于不满足下述循环复用条件的检查，没有后续视图需求的 Boolean 和空接口保持原入口；debug 也保持原入口。运行时规范 2.1 定义新查询，runtime ABI contract 升至 15，NoGC runtime wire tag 25 表示 ITableFind；metadata ABI 9 和父表布局 capability 17 保持。

同块 Boolean 局部副本可以连接原接口检查与条件分支，包括否定运算引入的临时值；查询仍在原检查求值位置执行一次，保留所有 Boolean 使用。只有参与该链的 Boolean 存储未取地址、且受检对象从检查到分支未被重新赋值时才复用；不把原对象检查延后到分支位置。

release 对可能在循环中重复执行的 exact interface 检查，可进一步复用同一函数调用内稳定局部值的查询结果。来源限于未取地址且未被写入的参数，或只有一个写入点、该写入点不在任何正常／异常控制流环中的非空引用局部值。按实际 CFG 判断是否重复执行，不改写 MIR 已记录的 loop poll 责任；循环中创建或重赋的来源、字段／全局／指针读取保持原查询。仅当至少一个同源同接口的检查位于控制流环中时建立复用状态，普通单次检查不增加状态。

每个合格的局部值／exact interface 组合保存 GC-free entry 指针与独立的已初始化 Boolean。函数入口只初始化状态，在第一次实际到达原检查求值点时调用 `itable_find`，随后复用成功或失败结果；零次循环、未进入的条件和检查前的异常不提前执行查询。该规则也适用于没有视图消费者的 Boolean 检查与空接口检查，区分未初始化、null entry 未命中和非 null entry／null slots 的空接口命中。成功后的视图继续消费同一 entry 和既有父表投影；每次仍从当前 managed 引用取得 object，不保存对象副本或延长 GC roots。稳定来源的 metadata 可跨普通调用、移动 GC 和 catch 复用；这不扩大前述一般成功路径事实的异常传播规则。debug 保持原路径，接口布局、跨 Cone／`.slib` 格式和 runtime ABI 不变，不增加全局或 runtime 缓存。

同一迭代在内联／化简之后分析函数内分配来源并消除可替换的局部 box，再化简新暴露的分支。来源为本次正文中 typed 创建点的可能集合，普通局部复制与 Retype 传播并集；合并外来值时保留已有候选，取地址、返回、存储、身份观察及未内联调用均保守保留对象。候选和分析结果不持久化，不把 NoEscape 等同于可删除。

首批替换要求创建点支配全部相关使用，所有引用使用都可改写，且来源唯一、没有外来值，正常与异常路径均满足支配和跨迭代存活限制。循环内创建的候选若有别名跨创建迭代存活则保留；按具体回边判断，不能因创建点位于某个外层循环就拒绝内层的重复使用。回边目标支配其来源，且创建块严格支配该目标时，创建位于该内层循环之前，该回边不代表新动态实例；其他回边继续保守检查别名 liveness。box 的 payload 在原创建点保存为普通 typed local 快照，保持求值顺序和完整值复制，以快照替换 exact payload 类型的拆箱，折叠由现有 boxed type／继承／itable 事实决定的类型检查，然后删除无用引用别名。普通 nominal 的 box 若由其他 Cone 提供，消费方 MIR 的本地 itable 集合不表示其完整接口闭包；不得将本地缺少表项折叠为 false，这类接口检查继续使用实际对象的运行期描述符。整型 enum 的非 exact 父整数拆箱暂保留；错误转换的失败路径不能因同位宽而消失。含引用 payload 继续经最终 LIR 精确 roots 保活与移动回写，身份、pin、地址及未知使用仍保留实际分配；不引入栈对象、重物化或新的 GC ABI。

catch／finally／resume 本身不排除整函数的局部分配消除。支配与跨迭代别名存活分析将带隐式 unwind 的 MIR 块入口和正常完成点分开：隐式异常边保守从入口出发，块内定义只在正常完成点杀死旧别名的存活；显式 Throw／Rethrow 的异常边仍位于语句之后。跨块使用必须由创建块的正常完成点支配，同块使用继续遵守语句顺序。这个分析图不改变实际 MIR 块、异常入口、throw、poll 或调用位置；不得用可能尚未执行的定义遮蔽异常处理读取的上一轮对象。字段在原位置转为局部赋值，异常发生于更新之前时处理器观察旧字段值，更新之后观察新值，先前读取的引用／值快照保持独立。所有实际正常／异常活跃值仍交由现有 LIR invoke 根计划保存和移动回写，不增设对象根或 native 异常槽。

优化决策报告复用同一次来源、循环、支配与使用分析，记录实际保留原因；不再仅因函数含 EH 而统一报告 `exception-flow`。报告不进入持久 IR 或 `.slib`，无需改变产物或 runtime ABI。

同一来源分析也登记局部 ClosureAlloc。高阶函数及 invoke 展开后，只有所有环境引用使用均可替换时才删除实例；按原 captures 的求值顺序保存每个捕获值，以独立的物理字段索引匹配 ClosureCapture，不能把字段布局顺序当作求值顺序。捕获值副本、引用共享、bound receiver 的单次求值与普通 roots 保持。未知调用、身份／地址观察、逃逸存储、无法处理的函数类型关系及未满足上述异常路径证明的使用保留环境；被环境捕获的其他候选在后续迭代重新分析。实例消除不删除或残缺化语义 class／invoke／bridge 记录，其物理支持项仍遵循输出选择合同。

普通 declared class 的 ClassAlloc 同样可以参与局部分配消除，包括构造器展开后由 Assign 定义的创建点。只有没有 release hook、来源唯一且创建点支配所有使用，并且所有对象引用使用均为可删除别名或可替换的普通字段读写时，才以字段局部值替代实例；继承字段使用已经确定的 base-first 索引和完整类型。构造器／方法只有经既有内联规则展开后才能暴露这些操作，优化不重新执行正文、改变求值顺序或跳过构造副作用。未内联调用、发布／返回／捕获对象、身份观察、取址、原子对象操作或无法证明的正常／异常控制流保留实际分配。

每个字段对应一个 typed、可写的合成 local，原 FieldSet 在原位置变为 local 赋值，FieldAccess 读取当前字段值；普通按值读取仍产生独立快照。语言规范 9.1.1 已保证合法字段读取之前相应初始化完成，零填充的未就绪 payload 不是源码默认值，因此不为字段 local 制造缺省值或重放前端初始化检查。含引用字段继续由最终 LIR 的使用／定义关系形成精确 roots，跨 GC 更新并在最后使用后结束存活；不保留被消除的对象引用或新增栈对象。循环沿用上述跨创建迭代别名限制，内层游标更新保持同一实例的字段状态。语义 class、字段、构造器和公开 ABI 记录保持，优化不增加持久产物字段或 runtime ABI。

LIR 准备阶段依据最终 MIR 的实际引用选择普通 closure 的私有物理支持。只将环境关联的生成 invoke／bridge 作为可省略 callable，普通源码定义、初始化、入口、dispatch、callback 和协程协议入口仍是发射根。由这些根的正文沿本地调用、ClosureAlloc／ClosureCapture 与环境支持关系传播；直接引用某个 helper 或同 class 的其他实例均保留对应支持。没有外部根的 helper 相互引用不使自身存活。未选环境不发射其 TD、对象／值 layout、scan 和私有 callable；仍被实际类型检查或其他环境使用的函数签名 TD 保留。首版保留 suspend closure 支持，不扩大为任意定义的全程序 DCE。选择仅为 lir-lower 的本次私有状态，MIR 身份、签名和完整语义目录保持，不增加产物字段、重建语义校验或改变对外导出合同。

release 在普通 MIR 优化迭代结束后、LIR poll／roots 生成前，可以对 Managed 的计数循环分块。接受独立前驱中初始化的非负 Int／Long 归纳变量、稳定上界、严格小于条件及每轮唯一的加一更新。上界可为整数常量、不被写入或取址的参数、进入循环前已经保存且不再改变的整数值，或来自同样稳定数组引用的 ArrayLen。已保存的值须为单次定义，该定义严格支配 header，且不位于从正文到下一次 header／退出前可能执行的区域内；因此允许外层遍历中的内联场景，不限于紧邻 header 的直线路径。header 可以由只有唯一前驱的无条件跳转块组成；body 可以是直线或含条件分支的无环区域，只有一个入口与一个返回 header 的 latch，所有继续迭代的路径均到达该 latch，归纳变量的唯一加一更新必须支配 latch。区域由唯一 latch 沿前驱反向取得，止于 header；离开区域的 return、break 或检查失败路径沿原出口执行，不要求这些路径经过更新。只在退出路径执行的调用、分配、抛出及清理代码不属于被复制的内层；首次命中后立即离开，不执行后续元素的访问或副作用。选择成功后合并可直接连接的直线块，保留分支条件、语句次序、源码范围和原 header 的 poll；不提前执行另一分支的计算或元素访问。header 只含条件的纯局部临时值及稳定数组长度；body 可含整数／Boolean／Float／Double 的局部运算，以及下述固定表示值的 MIR ArrayGet／ArraySet、普通字段读取、身份比较和 ArrayLen。浮点按实际 intrinsic struct 身份识别，不将普通 GC-free struct 当作标量；算术、比较和转换沿用 2.18 的 typed 操作及语义。归纳变量更新只沿正文中单次定义的私有临时值解释别名，不用重赋后的值改写先前表达式的快照含义。继续迭代的区域内含调用、分配、其他非局部访问、取地址、不在下述范围内的聚合操作、EH、多回边或内部循环时保留原形。已知不超过 64 次的短循环不分块。

String 的字符扫描、字符边界定位及共享长 hash 由普通 core 正文完成，不在 codegen 重写算法。既有分块操作集合接受真实 typed StringByteLength／StringByteAt 和已建立范围的 StringByte load／store：每次只进行有界访问且不分配、停顿或抛出可捕获异常，保留受检读取失败 leaf 的原位置与行为。普通 extern 不具备该资格，StringCompare 等完整扫描也不因此取得有界资格。继续使用原 64 次上限、外层 poll、短尾、提前退出与精确根恢复；这些访问可参与已有无 safepoint 内层优化，不添加函数名规则或新的循环框架。字符计数由普通 core 在每个至多 64 字节的小块内使用窄计数器，再累加为 Long；该上限保证计数不溢出，release 沿既有分块优化形成有界内层，完整扫描仍定期 poll，源码 NoGC 的 GC-free 类型限制不变。

上述数组值范围还包括单字引用 class／Any／String／数组／函数引用、双字 interface，以及复用这些或标量 payload 表示的 MaybeUninit。恰有两个 variant、一支为空、另一支仅含一个引用字段的可选值也可参与复制、构造和已有 typed variant 测试／投影；使用实际 enum 身份和字段，不按 Option 名称猜测表示，不把一般 struct／enum 加入有界内层。普通 class 字段读取可返回这些值，引用身份比较只观察 object 分量。引用转换仅在表示转换不需要目录查询时参与：单字引用重定型、取 interface object、空接口视图、final class 的静态接口表和已知子接口的父表投影；关系从当前 exact 类型及父接口记录取得，不重建布局。动态类型检查、装箱、未知分派及其他实际调用仍保留原循环。

引用数组与 wrapper 的初始化、读写和条件分支保持原求值、别名、variant 与部分初始化合同。分块仅减少 poll 执行频率，不删除或延迟原 ArraySet 的完整写屏障；代码生成仍在实际写入后完成必要的单槽／范围标记。数组、元素引用、循环携带值和出口快照均由最终 LIR 的实际使用产生 roots，外层 poll 后从更新的引用重取，不增加备用 root、缓存裸元素地址或跨 safepoint 保存页表／card metadata。MaybeUninit 零化保留合法 wrapper 的空引用，不能作为合法 payload 读取。LLVM 的显式向量化选择继续排除 managed 引用访存和写屏障，不把引用复制转换为未经相应 GC 合同处理的 SIMD 或内存批量操作。


对于已经展开为普通 MIR 的数组访问，可以利用上述非负、加一及严格小于上界的事实选择循环内始终成立的分支。数组身份来自实际 typed 引用及按求值次序建立的副本关系，数组长度不随元素写入变化；不得按方法或局部变量名称识别检查。上界就是同一稳定数组的长度时，直接消除已证明不会发生的越界分支，再沿普通分块处理完整块和尾部。在无环区域中按拓扑次序传播当前位置的值，合流只保留所有实际前驱共同成立的事实；不同分支写入的值或索引快照不得相互替代。同一事实用于归纳变量证明与数组内联选择时也遵守该合流规则。已知检查选择相应后继；未知的数据条件继续保留两条路径，其中继续迭代的部分都须满足内层操作合同，离开循环的路径保留原有出口。先尝试检查消除，再选择可分块区域，不能仅因检查失败块已在区域外而跳过原有范围优化。任何未被证明的检查或可能发生的异常仍保留，不改变 receiver、index、RHS 的原求值顺序（语言规范 10.1、10.5）。

如果访问另一个稳定数组，需要运行期条件 `上界 <= 数组长度` 才能证明合法，则只在原 header 已进入正文且剩余次数至少为 64 后检查这些条件。所有条件成立时执行复制出的 64 次无检查内层；任一条件不成立或不足 64 次时进入原有标量正文，保留逐元素检查、先前写入、副作用与异常。guard 不抛异常、不执行元素访问，也不将越界异常移到循环前；零次循环不执行新 guard。原数组引用和上界在循环期间必须保持不变，包含取址、重赋或不明确来源时放弃此选择。跨条件分支取得的必要长度 guard 取并集；每个 guard 只读取在原循环入口已经可用的稳定数组引用，不能引用仅在某个分支创建的对象。只复制已选的无调用／无异常区域，并按同一块映射重定向内部边，保留原循环的其他分支和唯一 poll，不复制异常分配或新增内层 safepoint；不构建另一套循环布局或改变 ABI。

原 header 和入口的 poll 责任保持；进入 body 前在已成立的 `0 <= i < n` 条件下计算 `remaining = n - i`。remaining 至少为 64 时，在所需 guard 成立后进入常数 64 次的完整块；不足 64 次时，无 guard 的循环进入剩余次数的标量块，需要 guard 的循环则进入上述原有标量正文。完整块及无 guard 的尾块以独立 typed 倒计数约束，非零继续内层，零返回原 header。完整块复制有限 body，并为只在区域内使用的临时定义重分配 caller local 身份；出口读取的值及循环携带值保留原 typed local，两份互斥正文写入同一存储，再由正常后端合流。退出边不经过内层倒计数更新，也不修改原出口的语句次序。原 i 加一仍在 body 中执行。该结构避免 `i + 64` 溢出，末块、零次及负上界保持原退出行为。数组基址沿普通 managed 引用与 roots 处理，在外层 poll 后重新取得；不固定对象地址，不引入跨 safepoint 保存的裸元素指针，不扩大原有对象的可观察存活。按长度构造保留原分配、初始化次序及失败不发布结果的语义（语言规范 10.6）；不增加持久 IR 字段或 runtime ABI。

Managed release 标量优化后，仅对实际 LLVM CFG 为单入口、单 latch 的有界内层请求向量化：去除 latch 返回 header 的边后区域必须无环，其他边不离开区域，所有路径到达 latch；倒计数 phi 位于 header，唯一外来初值是 1～64 常量，来自 latch 的回边恰好减一并在零终止。单块自循环是同一规则的特例，不能只凭终止条件推定其他路径也执行了计数更新。正文只允许整数及严格浮点的标量 SSA 运算、比较和转换，以及从循环外 AS1 基址经 GEP 形成的整数／Boolean／F32／F64 load／store；这些访存不能为 volatile 或 atomic，不读写 managed 引用或 root slot。调用、其他内存操作、指针合流／地址空间转换、原子、EH 和不明确的倒计数均排除；不能只检查 metadata 或名称。合格函数只追加 loop-simplify、LCSSA、仅显式选中循环的 loop-vectorize 和 InstCombine／ADCE，不开放一般循环复制／移动管线；普通 NoGc 管线与函数的 Managed effect 保持。metadata 只传递本次结构分析的选择，不能为外层 poll 或剩余标量块取得优化资格。最终 roots 与 RS4GC 仍在其后执行。

浮点分块与内层向量化不增加 fast-math／reassoc／contract 等假设，保留 subnormal、NaN、Infinity、signed zero、实际精度及每个元素的舍入次数；独立乘加不能融合。LLVM 22.1 的启用向量化 hint 默认还可许可浮点重排，因此 codegen 在统一初始化时设置 `hints-allow-reordering=false`；仅不写 fast-math 标记不足以保证此合同。循环携带的浮点归约保持源程序累加顺序，只有 LLVM 能保持该顺序的有序向量归约才可使用，否则保留标量归约。需要实际 LLVM call 的操作继续不取得内层向量化资格；普通 typed 浮点操作的 NoGC 后备仍按 2.18 处理。此能力复用已有内联范围、边界检查、分块与基址恢复路径，不新增根、数组表示或产物 ABI。

上述内层选择前可以把其普通 AS1 基址加载移到唯一外来前驱的终结指令之前：来源必须是函数入口的不逃逸 alloca，所有使用均为直接 load／store，内层不写该槽，也不存在取地址、atomic／volatile 加载或其他不合格操作。该位置位于进入内层之前的 poll／恢复之后；只移动普通加载，不移动 volatile 根恢复、逻辑根标记、site 或数组元素访问。提升后的 SSA 基址只供无 safepoint 的内层使用，返回外层后仍沿原 roots 重取，不把根槽操作交给向量化。

同一实际有界区域分析也服务引用写入的 mapping 查询提升。只有去除回边后无环、无其他退出且每条路径到达唯一 latch 的 1～64 次内层可以参与；区域不得含调用、用户原子操作、同步或未知 effect。volatile 访存仅允许保留在原位置的非原子 AS1 引用重读，且地址必须来自当前函数入口的 alloca，可经常量 GEP、仅在这些栈地址间选择的 select／phi；参数、堆、全局或不明确的地址以及 volatile store 仍排除。这些栈重读维持次数和彼此顺序，不代表 safepoint 或同步，不能作为待提升查询的输入。按 runtime PageMap 的实际数据引用与四级加载链，或 2.5 的 typed ArraySet 私有 mapping 操作身份识别查询；不按块／临时值名称或额外证明标记识别。查询及所需的 immutable region prefix 加载必须支配 latch，地址计算仅依赖循环外值或上述稳定根槽的普通加载。只提升这些加载及其必要的纯地址运算，不提升元素访存、card store、用户状态或其他 mutable metadata。

保留栈 volatile 重读的查询提升扩展只在 AArch64 release 默认启用；x86_64 的该类负载尚未取得稳定收益，继续排除含 volatile 的区域。普通引用循环沿用原有查询提升条件，该目标选择不改变布局、调用约定或 GC 语义。

提升发生在唯一外来边上；外来前驱还有其他后继时创建专用 preheader 并更新 header phi，避免在未进入内层的路径查询 mapping。该变换复用既有有界区域与根槽分析，保留原 poll／恢复与 roots，metadata 不跨下一次 safepoint。引用循环仍不取得标量 SIMD 资格；不新增 LIR／`.slib` 字段、runtime ABI、堆布局或跨阶段重复的循环证明。

上述 release 区域若已取得循环外不变的 mapping prefix，还可合并每轮必经的 card store。按实际 64-bit SSA 的加减、常数缩放和 GEP，以及 header phi 的固定加减回边，确认引用槽偏移每轮增量为 0、8 或 16 字节；不按循环名或额外标记推定。未知变化、截断／符号扩展后不能保持增量、条件执行、调用、同步、发布、提前退出和不合格 volatile 访存继续保留逐次标记。迭代次数直接复用本次区域分析的 1～64 常数，单次内层不做无收益的合并。

合并在唯一正常退出边上执行，维护原后继 phi 的 incoming block 和值；零次循环、guard 失败、不满足相同条件的动态短尾及其他出口不经过该边。从最后一次实际引用槽偏移与固定总增量取得本块起止 card，覆盖中间所有实际触及的 cards，每个写入流最多发射三个 relaxed atomic store；允许端点重复，不额外标脏未触及的卡。原引用／元素写入及 volatile 重读保持位置和顺序，只把仅用于最终卡计算的纯地址运算移到该退出边，不额外提升普通根加载。该边先于原 poll／根恢复和发布，AS1 引用、metadata 与根的生命周期不跨新的 safepoint；全部 profile 的对象、接口、GC 与产物 ABI 保持，不增加 MIR／LIR 字段或引用 SIMD 资格。

release 还可对上述常数有界区域中的小型引用读取内层作四倍部分展开。接受最多四个无环块、每条路径均到达同一 latch、迭代次数可被四整除且不超过 64 的内层，正文最多 24 条非 phi／分支指令，至少包含一次从 AS1 对象读取 AS1 引用；其余操作限于普通非 volatile／非 atomic 读取、AS1 地址计算、引用相等／不等判断与标量运算。条件块保留原分支和标量合流，不将空值分支中原本不执行的字段读取变为无条件读取。新读取的引用及其派生地址仅在内层使用；稳定根槽的普通加载复用已有分析，但不因此提升加载。含写入、调用、safepoint、提前退出、内环或引用递推的内层保持原形。附加展开提示前，先分离可能与外层回边共用的内层退出边并保持后继 phi，避免循环简化把提示复制给外层。卡标记合并等前序变换拆分过退出边时，后续变换使用已更新的区域出口。LLVM 只按显式倍数展开选中内层，关闭其他循环的自动完整／部分展开、peeling 与运行期余数生成；严格浮点归约保留原计算次序，不增加引用 SIMD 资格。该变换在 roots 定稿前进行，保留外层 poll、恢复、完整 64 次上限、短尾与全部 GC／ABI 合同，不新增 MIR／LIR 或 `.slib` 字段。

内层选择只授权尝试优化，不要求 LLVM 必须向量化或展开；无法识别 reduction 等普通 missed-optimization remark 不作为编译错误，也不写入结构化 child 协议的 stderr。该局部 pass 运行期间忽略 LLVM remark，错误／警告仍按原诊断路径传递，结束后恢复原 context handler。

条件 poll 在 LIR 保存慢路径 site 与完整 live set，仅实际慢路径产生 statepoint／stackmap；合流连接更新引用，NoGC 不新增 poll。内联、去虚拟化及 CFG 改写均先于最终 roots 定稿，后端不靠完整 runtime poll 调用维持普通循环的检查。

codegen 按运行时规范 3.2 的入口状态保证与 3.5 的线程握手生成 poll：普通 Full 仅以 LLVM acquire 读取全局 phase，不取得 TLS；Gateway 的全局 epoch／phase 使用 acquire，本线程 mode／observed epoch 使用 monotonic（对应 C relaxed）。有界普通叶函数可由 LIR lowering 省去入口 poll，Gateway 仍保留 pending 激活。全部构建 profile 使用同一规则；保留的读取仍位于原 poll，条件慢边、状态点与 roots 保持。此选择不弱化 runtime 向收集者发布状态及 native 返回握手的内存序，也不增加或改变 callable 参数。

release 对 typed ArraySet 的 stride 大于 16 bytes、恰有一个 managed leaf 的元素，复用实际数组布局和已有 scan offset，在写入前读取旧引用，完整写入后比较新引用；仅在新引用非 null 且不同于旧引用时生成原卡表查询、置脏和卡组登记（运行时规范 3.6）。普通 LLVM 优化可以转发写后读取、折叠常量和已知相同引用。选择不依赖函数、类型或符号名称，不改变 setter 调用协议；入口 poll 或先前分配之后才形成槽地址与比较值，区间内不新增 safepoint、root、状态点或外部调用。单／双字引用元素保持已有有界循环的页表提升与批量标记，多引用元素保持范围屏障，数组 literal／clone／assembly 与年轻对象初始化不增加旧槽扫描。debug 保留原写入路径。外部接口表示、`.slib`、runtime ABI、原子与 native 写入合同保持；预编译 setter 需重新生成才能采用此选择。

依据语言规范 1.2，编译器可以假定普通非原子访问不存在数据竞争，不为竞争场景提供单字或逐字段不撕裂的保证。普通字段、数组元素、全局存储和聚合值复制无需为此改用 LLVM `unordered` 原子 load/store、增加发布屏障或限制访问的机器指令粒度；显式同步原语以及 runtime/GC 内部同步的契约仍须保持。

最终 GC 计划从实际保留的 callable/site 和完整逻辑 managed leaves 得到。普通优化后保留逻辑根身份，标明包含可移动对象的 base phi/select；RS4GC 完成真实下降后，先清理 unreachable 前末尾无使用者的 gc.relocate，再按实际保留的 gc.relocate 引用的不同 derived SSA 值数量生成注册元数据，随后执行机器发射和原 stackmap 格式／根合同检查。此清理只处理终止块中连续的末尾无用 relocation，不重写控制流、不运行 InstCombine、PRE 或代码复制；它使无返回后继的调用后无用 relocation 与机器准备阶段一致。调用实参仍按 ABI 传入，被调用帧负责它实际继续使用的引用；正常返回调用的仅调用期间根和显式 native／EH roots 保持。多个逻辑 leaf 可以对应同一 SSA 值；已知 null／不可移动常量通常不产生 relocation pair，但不能仅按已折叠的逻辑恢复标记提前推算物理根数：共享后继或循环仍可能使原动态 SSA 值跨站点存活。该计数消费实际下降结果，不添加独立的指针活跃性分析或 RS4GC 策略复核器。每个保留 site 具有唯一 owner/role，site identity 不因删除其他 site 而重编号。

release 在 roots 定稿前以 MemorySSA 消除跨无写入分支的重复加载，防止同一根槽的两个 SSA 加载直到机器发射时才合并，造成注册计数与 stackmap 不一致。任一路径可能修改该位置时保留独立引用；每个逻辑 leaf 的调用后恢复继续保留，并使用对应的重定位结果。

全部 profile 在晚期聚合分解后、根身份定稿和 RS4GC 之前，合并同一基本块内等价的指针 PHI／select。PHI 按实际前驱与 incoming value 对应关系比较，select 按条件及两支的值比较，类型与地址空间必须一致；只含同一非自身输入的 PHI 和两支相同的 select 可以直接使用该输入。替换暴露的后续等价合流继续处理，不能把不同路径的对象或复制快照合并。此清理只替换标量值，不改变控制流、site 或逻辑恢复标记，不运行可能将分量重建折回旧聚合的 InstCombine。这样，聚合视图与已有 object 分量共享已证明相同的 SSA 根，不把合并推迟到机器准备阶段；返回、存储和实参仍在使用处从 RS4GC 更新后的分量重建。

GC 转换后每个 post-site managed use 必须来自正确更新的引用；invoke 的 normal/unwind 与 native transition 继续使用各自显式 root frames。不能在 root plan 定稿后增加未经描述的 safepoint、重复 site、外来 owner 或异常边。backend contract field 28 为 1，表示此最终根与发射合同。

ODR 只比较 key 与 shared ABI，按实际需要选择物理 member。body 的机器码、EH、stackmap、callable/safepoint registration、函数私有常量与 Context cells 必须同选；独立 helper 可分别选择。2.10.1 规划的共享常量通过真实引用单独选择，不归入每个 callable materialization 的私有数据组。共享同一完整接口数组的 dispatch definition 也必须同选，重复候选按这组实际物理 definitions 整组复用；不能将父表 alias 与其数组分别选自不同对象。shared TD、storage、cell 与初始化保持唯一，native linker 不得改变已确定选择。

program-link 在上述 ODR 选择之后，按已保存的逐物理 member 定义／引用关系裁剪不可达 callable 与类型／shape 元数据，对全部 profile 默认生效。保留 root entry、静态存储及根登记、immortal 及登记、初始化 cell／登记和 generated-C bridge；这些 member 是遍历入口，不以候选 image 的完整登记表、类型或派发表本身作为入口。runtime 的引用、typed runtime data alias 的目标，以及已声明 native 输入中对象的 Scoop 符号引用也作为入口；native archive 的候选引用保守保留对应 Scoop 定义，不因此提前抽取 archive member。沿实际机器符号引用传递保留完整物理 member，包含函数地址、动态派发表、release hook、callback、初始化与跨 Cone 调用。未到达的类型和递归引用环可以整体删除；保留的初始化等记录仍可能使其引用的类型或函数留下。

每个保留的 TypeDescriptor 同时保留同一 exact type 的 TypeRegistration；这条登记关联由已读的 typed entity 与 definition role 建立，ODR 选择后关联实际保留的实现。TD 的父类型、接口、相关类型、布局、scan 和 callable 引用继续按实际符号闭合。不能仅删除仍存活 TD 的登记，不能按名称、布局相同或泛型声明合并不同 exact application。Link 输出直接携带已有定义记录的完整 typed entity，不重新进行语言语义查询。

生产端将普通 Strong 的 TypeDescriptor 与同一 exact type 的 TypeRegistration 合为一个独立物理 member；两者本来就必须同选，不因分组额外保活其他类型。Layout、ScanProgram 和 dispatch 元数据按实际定义独立划分；同一完整接口数组及其父表 alias 必须继续同组。其余静态存储、初始化与 immortal 可保持既有组合。ODR 的独立数据划分及 callable 代码／EH／stackmap／登记／私有常量／Context cells 的同组规则保持。该物理划分适用于全部 profile，不改变静态表内部布局或公开 ABI；已有较粗物理分组的相容输入仍可消费，实际删除粒度以其已有 member 为准。

该选择复用 Link 输入中已有的符号、实体与物理成员数据，不重新读取机器对象或重建语言调用图，不改变源码可见性、类型检查、native 声明合同和 ABI。不可达物理 member 的内容与登记一起退出最终链接；静态存储、初始化与 immortal 的进一步裁剪不属于本次选择。输入 `.slib` 继续保存可供其他 consumer 使用的完整候选，沿用 `link-identity-closure/19`，不新增 wire 字段；机器对象与实际分组变化正常进入原有内容指纹。所有 profile 统一使用前述完整符号使用表及相同布局。

最终 image descriptors 和六张表由 program-link 从所选 records 生成，不改写输入 `.slib`。选择与实际内容进入 RuntimeImage、Code、Artifact 和 ResolvedLinkPlan 的对应摘要。

OptimizationMode 在 machine request field 9 中编码 Debug=1、Release=2；生产端 Code 输入继续保存模式，production section 不再复制该值；compile cache domain 为 `scoop-cone-compile-cache-v2`，mode 位于 field 13。generated-C flag 的 SelectedOptimization tag 为 15，tag 6 保留不用。

digest kinds 4、8、9，digest owner tags 4、11、9，保留不用；全部 runtime digest patch role 与 patch intent 退役。registration 公共 identity 只含 linkage、semantic ID、ODR group/member；body、layout、descriptor、scan 与 normalized-stackmap fingerprints 仍用于所选实现自身的内容、引用及 GC 合同，不作为不同优化正文必须相等的条件。

### 2.20 可恢复语法与语言服务公共基础

本节规定 M47 与 LSP 的共同基础。模块分工与查询边界见[公共基础设计](../language-service/DESIGN.md)，实际交付与验证见[实施与验收](../language-service/PROGRESS.md)；M47 的同步生成流程由[其主设计](../milestone47/DESIGN.md)及[协议](../milestone47/PROTOCOL.md)规定。合法源码的语言语义、成功 HIR 的完备性、产物和 runtime 合同保持。

M63中项目成员、workspace继承和精确依赖绑定统一来自独立PM的本地数据模型，见[包管理规范7](SCOOP-PACKAGE-SPEC.md#7-pm-与构建-driver-的接口)。analysis/LSP不自行解释第二套版本规则，不在编辑或补全时下载依赖；本地缺少锁定源码时提示执行fetch。编辑buffer仍由语言服务覆盖对应source，不能通过重写全局共享源码cache实现编辑态。

**源码与位置。** 同一文本版本提供不可变 UTF-8 源码、byte range 和可复用的行索引。普通文件继续沿用 SourceIdentity；编辑 buffer 和生成正文的本地定位由输入宿主维护，不使用显示路径、LSP URI、文档版本或请求编号构造语言实体身份。语义查询只消费显式传入的源码和依赖视图，不进行源码发现、磁盘读取或工具启动。LSP 在边界协商位置编码并转换为 byte offset；其零基行列、UTF-16 默认值及换行定义不能直接套用 CLI 的显示列或持久 SourcePointRecord。

共同源码数据由 `scoop-source` 提供，AST 为兼容现有消费者重导出原 Span 与诊断类型。当前 Cone 文本与诊断报告共享不可变文本，按需建立并复用行索引。CLI 沿用 LF 分行与一基 Unicode 标量列；编辑器坐标使用零基编码单位列及 LF／CRLF／CR 分行，CRLF 中间和字符内部不是合法编辑位置。超出行长的编辑器列归到行尾，越界行或 byte offset 作为位置错误返回。parser 在词法扫描前检查源码长度能否由 u32 Span 表示，不截断过大的输入。

**语法。** 同一 lexer／parser 支持完整文件、普通函数体和完整类型三个实际入口。可恢复结果总有相应的根节点和结构化诊断；保留全部原始 token、空白、注释及非法文本。多余文本保存在错误节点中，缺失语法以不占源码字节的显式缺失项表示；恢复不得伪造标识符、类型或正文。各入口消费全部输入，片段外剩余文本同样产生错误。编译适配只在没有词法／语法错误时投影完整 AST，driver 再交给正常 HIR lowering；generator 的片段不享有放宽规则。语法树及其遍历不依赖 HIR、LSP、LLVM 或项目图。

严格片段 API 分别为 `scoop_parser::parse_function_body` 与 `scoop_parser::parse_type`，输出现有完整 Block／TypeRef。函数体必须包含外层大括号；类型必须恰为一个完整类型。前后允许普通 trivia，诊断及 AST span 直接指向原始片段的 UTF-8 字节，不通过包装声明调整位置。片段与文件共用词法、诊断合并和排序；正文内已恢复的错误仍阻止返回成功 AST。

`scoop-syntax` 使用 rowan 的不可变树保存语法数据。lexer 在识别实际 token 的同一次扫描中保留 trivia；grammar 在原有 production 中记录节点事件，并同步构造由这些相同语法决定得到的紧凑 AST 投影，不再解析一遍 CST。可恢复入口返回语法树、诊断、显式预期项和仅在无错误时可取的完整 AST；严格入口消费这一结果。试探检查点同时保存 token 位置、事件、诊断、文档附着及局部 callable 编号，回退不能留下试探结果。

`scoop-fmt` 消费同次成功解析的完整 CST，其中保留原文 token 和排版所需语法角色，只产生格式化文本，不进行 I/O、语义分析或调用 parser 实现；CLI 编排原 parser 与格式库。排版所需的语法角色若尚未记录，在原 grammar 中补充，不通过另写 parser 或名称推断获得。首版只格式化语法完整文件，错误恢复及局部已知事实不构成允许写回的成功输入。非空白 token 保留、解析结构与文档附着一致、幂等性通过专门测试验证，不在每次格式化时再跑完整语义或多轮解析。格式库及 CLI 的交付不自动启用 LSP formatting capability，编辑器协议接入单独安排；详见[格式化与检查设计](../fmt-check/DESIGN.md)。

声明的语法视图保留本次解析内的父子关系、节点种类、范围和实际声明名称位置；名称在原 grammar 确认后记录，包括扩展声明消歧得到的名称。正文或签名后部出错不删除已经取得的名称，名称缺失仍显式为空。这些索引只属于当前语法版本，不是语言实体 ID；结构查询不从同名标识符或最近的范围猜测声明名称。

函数签名与正文在 AST 输入数据中具有独立边界。原函数 grammar 在消费完整参数表、结果类型及 where 子句后记录签名；该范围内发生词法／语法错误时不发布完整签名。记录直接关联所属声明的本次解析索引，并保留实际的块正文、表达式正文或省略正文形式。正文出错不撤销已经取得的完整签名，试探回退则同时撤销该记录。编辑态读取这一声明数据，不能将错误正文替换成空正文或把省略的类型项补成默认类型。完整编译 AST 仍必须通过整个入口的无错误条件；声明数据不是另一种成功编译输出。

HIR 前端将声明收集、声明签名、声明所需的正文计算与普通正文 lowering 分开编排。收集阶段建立原有实体及 import；签名阶段完成名义关系、可显式确定的 callable 签名、继承选择和参数接口登记。推断结果、默认模板、初始化及注解中的表达式仍沿原实现按实际依赖完成，不能因拆分阶段而提前宣称这些事实可用。普通编译依次完成上述计算、全部必要正文、生成、effect 与最终封装；阶段之间传递同一组原声明及实体，不重新收集或重放已完成的签名检查。

函数／方法的声明分配、注解合同和签名检查消费借用的签名、真实正文形式及声明位置，不要求持有正文 AST。普通函数声明按同一接口登记，另把实际表达式正文登记到原推断流程。待推断但尚未提供正文的函数不能从内部暂存字段取得 Unit 等确定类型；结果查询保持不可用，直到实际正文推断完成。参数环境、binder、可见性和候选规则沿原前端实现。

错误文件另外保留完整顶层声明片段及其 grammar 声明索引；有错误的声明不成为完整片段，其名称、已完成函数签名和未知范围仍留在同一恢复结果中。片段沿原 production 的成功边界取得，词法或语法错误覆盖的参数、成员及正文不会因内部恢复而变成完整声明。恢复文件的 package 可用性和 import 列表完整性单独记录，坏文件头不能冒充根 package 或空 import。完整文件直接复用严格 AST；错误文件把原先已取得的完整声明移入片段，不复制或拼装另一份假 SourceFile 交给正常编译入口。

文件的 package、import 与原文范围通过独立借用视图供名称空间、导入和 core 合同诊断使用。这些计算不读取声明正文，也不要求成功 SourceFile；普通编译从原 AST 取得视图并在声明阶段复用。恢复输入只有在相应文件头已完成时才能提供这一已知上下文，原文范围保持原文件坐标。

声明收集接收原完整声明或独立完成的函数头，并在同一 Cone 名称空间中分配实体。函数头与真实正文的输入列表分开：导入、签名、重复签名、继承、默认参数接口与注解读取共同的函数头；只有确有完整正文的声明才能登记结果推断和正文检查。正文不可用不删除已知候选，也不产生成功 HIR；尚无完整函数头的语法声明仍须由编辑态名称阻塞规则处理。

原 grammar 同时保留已取得的声明名称、可见性和声明类别；函数和属性还保留已经确定的 receiver 形式，函数保留 operator／infix 修饰符。由这些事实确定 type／value 名称空间及实际查询角色；完整声明及其子树不重复生成不可用项。编辑态把未完成声明的这些事实放入原导入和查找层，按普通 package、Production／Test 与访问域过滤。相关层含有可访问、且可能参与当前查询的未完成候选时，该名称查询返回不可用：不能选择同层的另一个重载，也不能继续穿透至低优先级声明；普通函数不阻塞扩展调用，确定的非值声明不与同层的实际值竞争。未完成项只有本次语法来源和内部 binding 身份，不取得成功语言实体 ID，也不写入导出 HIR。

语法完整而签名解析失败的函数同样保留声明归属及已知查询角色。原签名计算在发生错误时记录该声明不可用，名称查找、调用候选及结果查询复用这一状态；不能将参数解析中省略的失败项或内部暂存结果类型作为已完成签名。相邻完整声明继续完成自身签名，不通过重复检查整个文件判断可用性。表达式正文的待推断结果与签名失败分别保留；成功 HIR 仍要求所有实际依赖完成。

编辑态声明入口接收宿主已选择的源码身份、用途、原文、文件头和声明片段，复用普通依赖世界、core 输入及声明／签名计算。入口持有当前分析的声明事实，不封装成功 Module，也不启动代码生成。函数参数与 binder 已完成而表达式结果仍待推断时，编辑态签名保留完整参数及明确的待推断结果；整个签名失败与仅结果待推断不是同一种状态。声明参数描述保留值类型、vararg 元素类型及显式默认表达式位置，不将未检查的默认表达式描述为完整默认模板；继承默认值与参数省略仍由普通调用规则决定。固定查询和编辑态描述沿同一签名及类型图读取，协议适配层负责显示与位置转换。

分析 worker 对选中项目使用普通依赖图建立声明上下文。预编译依赖按依赖顺序读取已有 HIR，并沿正常 core 协议与共享语义世界导入；源码依赖复用普通前端已经完成的 Export HIR、真实依赖选择及源码绑定关系，直接形成相同的内存语义输入。为此，当前 Cone lowering 在 Export HIR 完成后提供独立结果，普通编译继续从该结果构造 LocalConcrete HIR；语义消费不要求具体化、MIR／LIR、LLVM、链接或 generator 执行，也不经序列化再解码取得已知声明。所需源码依赖的语义输出尚不可用时保留明确的依赖不可用状态，不能忽略该依赖后继续名称查找。已完成分析以所选配置、Cone 和实际源码版本复用；输入未变化的查询不重读依赖或重跑声明解析。函数及参数声明上的 hover 读取实际分配实体、声明位置和已完成类型；缺少可用事实时不猜测类型，协议层负责将结果表示为源码签名或不可用状态。

错误函数正文的恢复数据由原 block／postfix grammar 产生，并通过同一事件流归属实际函数。块记录保留真实 opening token、父块、完整语句、未知语句范围和结束符是否存在；只有词法与语法均完成的语句才能作为完整 AST 片段。lambda／匿名函数的作用域边界与普通块保持区分，未取得相应参数上下文时不能误用外层同名 binding。缺失 selector 保留已经完成的 receiver、navigation 形式和点号位置；receiver 自身有错误时仅保留不可用范围，不将省略了错误实参的内部 AST 当作完整表达式。EOF 前的独立语句仍可保留，未知区域不能在适配时删除或变成空的成功正文。完整块不另复制 AST，错误块只保留独立完成的语句；试探回退同时撤销全部恢复事件。

未完成的括号调用由原实参 grammar 保存 opening token、实际同层逗号、结束位置与闭合状态，并按原顺序保留每个实参槽位。命名参数和 spread 前缀独立于其表达式保存；只有无词法／语法错误的表达式成为完整输入，缺失或损坏表达式保留真实范围。callee、显式类型实参、receiver／super 形式沿实际调用语法取得，callee 自身不完整时保持不可用。恢复调用与所属函数、块通过同一 grammar 事件关联，嵌套调用各自保留范围，试探回退同步撤销；正常实参列表不另复制表达式 AST，严格编译仍拒绝恢复错误。编辑态据此使用真实给定输入查询调用候选，不能通过删除坏实参或填入假值取得一次成功绑定。

恢复的括号调用若继续接收尾随 lambda，原 postfix grammar 将该实参按实际调用范围附加到同一记录，括号范围与尾随实参范围保持分开。完整 lambda 保存原表达式，损坏 lambda 只保存未知范围；不能忽略尾随实参后查询另一个调用形状。

普通调用的参数映射先按声明规则绑定已给实参，再检查缺参并补齐允许省略的输入。第一步保留未给参数、真实源码顺序、命名参数、spread／vararg 形式和尾随 lambda 的对应关系；已能确定的重复、未知名称与调用形式错误仍立即失败。第二步才应用 required／default／vararg 的省略规则并形成完整调用映射。编辑态可消费第一步的事实，但缺少输入不等于采用默认值，也不构成一次成功调用；普通编译复用这两步并保持原缺参诊断。

实参类型检查共用普通候选的固定点计算，先收集实际检查所得表达式及延期输入，再由完整调用入口完成约束求解和类型适配。前一步检查已固定的刚性类型关系：显式实参或宿主应用固定的类型不能违背已检查输入的 subtype 关系；依赖尚未固定变量的关系保持延期。partial bindings 只用于向延期实参提供期望类型，不证明泛型边界、具体类型应用或全部实参已通过检查；后一步继续执行原完整求解。编辑态读取前一步的既有事实与失败原因，未解输入保持未解，不能将推断提示转换成已完成调用的类型参数。

半成品调用沿实际源码作用域查找声明，并对各候选分别使用已给实参映射。缺失表达式仍占据原语法槽位，不能删除后重新编号；第一个未知操作之后不继续沿用先前 flow 检查实参。结果保留声明签名、源码槽位到形参的映射、缺少的形参与未检查输入，不把候选列表当作成功重载选择。候选推断状态彼此隔离，显示类型在各自实际 binder 中使用普通类型名称格式化，宿主应用带入的调用方参数仍按原身份取得名称，不按同名参数替换；不将候选私有 arena 中的 TypeId 交给其他视图解释。这些显示文本不参与身份、绑定或类型判断。

调用实参若是头部完整、正文损坏的 callable literal，原 argument production 将该实际槽位关联到 literal 的块入口；不把它当作完整表达式。具名调用的隐式 lambda 标签来自同一次调用 production，显式标签优先，匿名函数不取得隐式标签。候选固定点保留最后一次已经计算的 partial hints，恢复查询不重新求解同一组约束。只有当前查找层留下一个候选、且未知实参之前的全部输入已经成功检查时，才使用该候选的实际映射和期望上下文进入此 body；未知输入之后的实参保持不可用。期望函数的 receiver／参数类型必须已知，结果仍待推断时可以沿同一 callable 期望数据保留局部查询；不为了补齐结果而合成函数类型。多个候选或未定的必要上下文不按显示文本合并、不任意选择，候选私有状态只随其实际查询结果保留，不把其中的 TypeId 交给另一候选解释，也不把这一局部查询当作成功重载选择或成功调用。

尾随 lambda 是独立的实际实参；恢复调用和签名帮助区分省略括号的 `call { ... }` 与带括号的 `call(...) { ... }`，不为前者生成括号位置或虚假空输入。原调用 production 在尾随表达式完成后决定是否保留恢复调用，完整前序实参与损坏正文按同一参数映射处理，完整调用不发布重复的恢复输入。函数结果的显式分组调用、safe receiver、泛型、标签与 suspend 继续使用原消歧和作用域规则。

局部具名函数的恢复输入保留原函数声明 owner、所在块、完整头部和自己的正文。普通编译与恢复查询共用局部签名解析、泛型约束、重复声明、默认参数接口、参数绑定及作用域进入／退出；仅在此前语句已知且头部检查成功时进入正文，自身名字在签名完成后按原局部声明规则可见。返回类型尚待推断时不使用占位结果作为已知类型；label、context、suspend、捕获与循环目标遵守该内部函数的真实边界。已知接收者事实将 TypeId 与原 SourceContextId 一起保存，读取时复用该上下文中的 owner 与 binder，不能退回外层函数的同名参数。损坏正文不会被包装成成功 AST 或成功 HIR 正文。

类型限定调用的候选查询与普通调用共用限定目标选择，保留值对类型名的词法遮蔽，按实际目标区分嵌套类型构造、enum variant 构造与 object／companion 成员调用。构造候选读取原声明签名、访问域及默认参数形式；类型命名空间不要求形成值表达式，候选查询不合成完整实参或成功构造结果。源码与已载入 Export HIR 的限定名均复用普通静态绑定与构造声明；透明别名保留自身模板参数映射和固定应用，泛型 companion 按实际宿主应用取得 receiver。variant 构造不要求访问或初始化 companion，查找失败不能回退到被遮蔽的其他名字。

函数正文的实际入口由 grammar 保存为所属块的 opening token 或表达式起点；不能把表达式正文内的 catch、lambda 等内部块误当函数入口。编辑态 receiver 查询在原函数作用域中建立参数，并依次用原语句检查器处理完整前缀。第一个未知语句若是普通嵌套块，可按原父块关系进入其完整前缀；初始化式、return 和 throw 的值表达式入口由相应 grammar 记录实际语句起点、值起点及已完成的类型注记或标签。前端先沿原规则检查注记与 return 上下文，再检查值起点已取得的 receiver 或半成品调用；初始化式尚未完成时不建立新局部 binding。表达式正文仍使用原入口，显式 return 的限制不变。尚未取得必要条件或参数上下文的控制结构和内部 callable、失败前缀及未知语句之后保持不可用，不借用失效的绑定或 flow 事实。名称实际选中局部 binding 时记录其 typed 身份和定义位置，不能按同名变量反查。查询在语句边界检查取消，未完成的计算不提交分析状态；成功查询也只发布 receiver 事实，不封装函数正文或 Module。

if 的恢复入口保留实际条件起点；只有条件及右括号完整时，才关联原条件 AST 与损坏分支的实际块或单语句入口。原条件检查器在进入分支前检查该条件一次，分支分别使用其真／假结果的既有 smart cast 事实，并遵守原分支词法作用域。单语句分支不伪造块 opening token；一个分支的失败、绑定与写入不能污染另一分支，整个未知条件语句之后仍不可用。条件自身未完成时只查询其真实已完成输入，不把尚未建立的收窄当作分支事实。恢复事件随原语法试探撤销，完整 if 不另复制条件 AST。

while 的恢复输入在原 grammar 中记录完整条件、实际循环标签及损坏 body 的块或单语句入口；条件不完整时保留其真实表达式起点，完整循环不另复制条件 AST。原循环目标在条件与 body 的查询范围内生效；完整条件通过普通 Boolean 检查后，body 使用其真分支事实及独立词法作用域。由于损坏 body 的回边未知，进入这一循环时沿既有 flow 事实操作移除可变 binding 的收窄、别名关系及相关 Boolean 蕴含，保留不可变 binding 与地址逃逸状态；每轮条件和完整前缀可以重新建立事实。不得以首次迭代的状态代替未知循环的稳定结果，也不为恢复拼造成功循环 HIR。查询结束或取消后退出原循环控制区，未知语句之后仍不可用。

for 的普通 lowering 将 iterable 求值、iterator 选择、唯一 core `Iterator<T>` 应用检查及 next／Option 元素读取作为同一迭代头部计算，供恢复查询直接复用。恢复输入只有在 pattern、iterable 与头部结束符完整时才关联 body；iterable 不完整时仅保留其在外层作用域中的真实输入入口，不预先激活自身循环标签。头部完成后沿原不可反驳模式生产器建立该轮只读 binding，pattern 与 body 保持同一词法作用域；错误模式不发布部分绑定。进入 body 前按 while 的未知回边边界处理可变 flow 事实，原迭代控制目标只覆盖 body。恢复查询不重新按名称选择 next、Some 或 None，不合成完整 for AST 或成功循环 HIR，普通编译仍消费同一头部计算完成真实循环。

do-while 的恢复记录区分损坏 body 的实际入口与完整 body 后的损坏条件；后者保存原完整 body 和条件的真实表达式起点，完整循环不产生这份额外输入。body 查询先建立原循环控制区，不使用后置条件的收窄。条件查询在同一轮词法作用域中检查完整 body，再复用普通 do-while 条件入口：合并正常出口与实际目标的 continue flow，并沿原控制结果计算可能被 continue 跳过的 binding；finally 覆盖跳转等规则不另实现。读取未初始化 binding 产生原诊断，未知或失败 body 之后不推测条件作用域。未知回边仍按 while 的可变事实边界处理；退出或取消时恢复条件初始化状态和控制区，不构造成功循环 HIR。

when 的恢复记录只在该 production 未完成时保存 subject 与 arm 头部，完整输入不另复制这份 AST。头部和 body 来源保持原顺序，语法错误与 else 顺序错误不被省略为更小的成功 when；未知 arm 保留不可用边界。普通 subject 计算直接接收实际 subject 与是否需要普通条件引用的信息，普通 arm 计算直接接收实际条件／guard，不依赖伪造 body。模式绑定、primary 与 guard 的真假 flow、条件完成性和分支 scope 使用原实现；值分支的上下文需求继续由原完整 body 决定。恢复查询仅进入完整头部后的实际 body、完整 primary 后的未完成 guard 或已有逗号条件之后的输入，未知头部阻止后续 arm 事实，未知 body 不影响其他候选分支。subject 声明在初始化完成前不可见，取消与退出恢复 scope 和 flow；结果仅为编辑态事实，不发布成功 HIR 或替代普通穷尽性检查。

try 的恢复输入只有真实块入口或原完整 AST 块；未知块不提供假正常出口。即使后续 catch 头部或 finally 未完成，已经取得的 try／handler 前缀仍保持原归属。普通语句 try、值 try 与恢复查询共用 catch 类型解析、Throwable 子类型和前序覆盖检查，参数身份来自原名称位置，catch 参数 scope 与其 body scope 沿原实现建立。完整 try body 的 catch 入口和全部完整前置区域的 finally 入口复用原 flow 计算；未知或语义失败区域使用去除可变 binding 及其别名／Boolean 关系的既有事实，保留不可变事实和地址逃逸状态，不重复分析完整区域以验证自己。没有 finally 查询需要时不额外检查完整 handler body。作用域、flow 和取消状态随查询退出恢复；不合成成功 try HIR，不改变普通完整编译的异常与清理职责。

lambda 与匿名函数的恢复记录只保留原 grammar 已完成的参数／receiver／结果头部、实际标签和损坏 body 的真实入口；完整 literal 不复制恢复头部，语法试探撤销相应记录。普通 lowering 与恢复查询共用内部 callable 的进入／退出流程、参数类型与不可反驳模式检查、receiver 和捕获环境。期望类型来自已解析的初始化注记或当前实际返回上下文；没有必要参数信息时保持不可用，不从外层同名变量推测参数类型。lambda 的隐式 `it`、显式参数数量与类型、匿名函数的 receiver／结果相容性沿原检查处理。内部 callable 清空外层循环目标，使用自身的 suspend、安全与 return 标签规则；捕获和构造参数访问沿普通路径，内部写入不改变外层 flow。查询完成或取消都恢复原上下文，只发布正文前缀的查询事实，不合成空 body、完整 callable 类型或成功 lambda／匿名函数 HIR。

精确 import 的来源记录分别保留 selector 末段与显式 alias 的名称范围。引用索引沿已经解析的目标建立 import 引用；正文只有实际选中精确导入查找层时才关联对应导入，不按同名文本推测。alias 在当前分析索引中拥有独立的 typed 身份，其声明与使用可单独查询；原声明的引用仍保留这些实际使用及 alias 来源，供编辑决定哪些拼写属于原声明。`import f as f` 仍有独立 alias，静态宿主限定、局部及隐式成员查找不能误算作该 alias 的使用。一个 import 引入的重载集合保留各真实目标，转导出继续保留原绑定链，尚未完成的来源覆盖不能作为完整重命名依据。

跨 Cone 的引用沿普通依赖绑定已有的 public export binding ID 与重导出路径记录，区分最终声明和实际使用的导出名称。源码 public import 的精确目标、显式 alias 与星号转导出分别关联原冻结绑定；显式 alias 的本地使用和下游路径可共同查询，不合并同名的其他绑定。星号及当前包依赖查找只记录成功选择层中的真实目标，限定类型读取原查找结果。引用位置同时保留路径首项与本地 alias 来源，供重命名区分当前拼写、无 alias 的转发和显式改名边界；不新增产物字段，也不以显示名称反查绑定。

重命名消费同一引用索引与原声明来源：精确名称通过原 lexer 识别，构造名称选择原类型／透明别名，property 与构造参数等共享声明 token 的实际目标一并处理。无 alias 的导出转发沿原 binding 路径联动，显式 alias 构成拼写边界；方法和 logical property 的重写关系使用原实现索引。需要修改的声明必须有项目源码，所需语法、声明和正文分析不完整时不返回部分编辑。运算符 token、隐式调用名称或构造委派关键字不能替换为普通标识符。

编辑结果携带原 SourceVersion 和精确 UTF-8 Span。冲突检查将这些编辑应用到私有不可变快照，复用未变化的源码与依赖输入，运行普通分析，并按原声明来源经位置映射比较绑定；不以“没有新诊断”代替捕获／遮蔽检查，不以 FQN 或显示名对应编辑前后的实体。私有分析不发布到当前宿主、不取消原快照，不复用可能与后续真实 revision 混淆的候选语义缓存。失败或取消丢弃候选；LSP 在仍有效的快照上将结果转换为带各文档实际版本的批量编辑。

import 的目标名或 alias 名称导航读取已解析绑定的原声明来源，重载集合可有多个定义；此读取不要求建立整个 Cone 的正文引用索引。命名实参导航使用成功调用的真实 callee 与形参序号，当前声明和依赖接口都取各自原参数位置，不按名称重新配对。无源码依赖只保留已知语义来源，位置是否可供编辑器打开由宿主的实际源码输入决定。

命名实参说明与导航读取同一已检查参数映射，保留实际 callee、形参序号、应用类型及正文的原 binder 上下文。hover 的 vararg 显示原元素类型，文档按 callee 和形参序号取得对应 M60 目标；不重新执行重载选择或按同名参数配对。已完成调用的独立事实可在后续正文失败时继续读取，失败试探与尚未检查的调用不发布事实。没有真实 callee 来源时只展示已有参数类型，不构造参数文档或定义目标。

编辑态调用目标完整区分普通 callable、已有 class／struct 构造器身份以及当前／依赖 enum variant 构造器身份。原重载和函数值选择成功后记录目标，构造参数读取原参数接口及 M60 callable 参数目标，不能以字段／property 身份替代。单位 variant 的值使用也关联原声明。构造函数值的内部 adapter 不作为可导航或可重命名的源声明；引用索引按源构造声明合并实际泛型应用，不重跑选择或从结果类型猜测具体构造器。

具名构造调用与构造函数值同时保留实际构造器和源码名称的原类型绑定。透明 typealias 取已选候选保存的 alias，普通类型取原构造 owner；显式 import alias 与重导出路径沿该名称的实际绑定关联，不把结果类型当作源码名称。定义与签名读取实际构造器，类型声明的反向引用包含相应构造名称，选择 alias 则查询该 alias 的实际使用；普通构造器引用仍区分重载。variant 的末段名称保持 variant 身份，类型限定段分别保留原查找事实；失败试探、失败正文与取消不发布名称引用。

一元、二元运算符及 when 的 `in`／`!in` 条件在 AST 中保留实际运算符 token 范围；lowering 合成的比较或组合条件没有虚构 token。源码运算符只关联普通重载或依赖调用实际选中的 callable，与具名调用共用来源、文档、导航和引用事实。派生结构比较、字面量符号、短路、引用相等及直接生成的内建运算若没有源码 callable 选择，不从类型或显示名称反推目标。运算符引用不冒充具有名称与实参列表的普通调用，不改变运算符效果、求值顺序或诊断范围；失败试探和取消沿原正文边界丢弃。

下标表达式和下标 place 保留实际左方括号范围，`++`／`--` 与复合赋值保留各自实际运算符范围。下标读取和写回的 `get`／`set` 关联该左方括号，更新的 `inc`／`dec` 与复合赋值实际选中的运算关联更新 token；只执行原候选选择成功的分支，不把试探的另一种实现记为引用。同一个源码 token 可以关联多个实际 callable，例如下标读后写回；同位置记录按目标去重而不覆盖，导航及 hover 分别返回它们的来源和签名。精确导入 alias 仅遮蔽其实际代表的目标，其他同位置目标继续可查询。临时变量和合成下标不产生源码位置，重复求值、参数组装、默认值和诊断仍沿原 lowering 契约。

扩展运算符沿原 typed role 查找；选中精确导入层时，以实际 callable 与导入绑定关联其来源，包括显式 alias。不能使用 `plus` 等内部诊断名称去匹配 alias 拼写，也不将同层未选中的其他运算符视为这次使用。

显式访问器正文与普通函数正文共享分析结果和来源查询；区域保存真实实现函数及原 getter／setter 身份，不能把内部实现函数作为新的源码函数声明。访问器的 scope、receiver、backing field、泛型、捕获和效果检查沿普通 lowering；参数引用及 M60 文档归属实际访问器，`set()` 的隐式 `value` 使用原参数槽的空源码范围。局部类型的访问器须使用外层已完成的声明事实，失败外层不发布残留区域。存储／委托的生成访问器不成为用户正文；当前文件诊断遍历同一源码区域列表。错误与取消维持原独立区域的可用性边界，不重放已经缓存的正文事实。

顶层运行时初始化式的编辑查询借用原 initialization unit、初始化函数和表达式，不构造源码函数声明；源码正文归属以 typed callable 或 initialization unit 区分。存储／委托初始化沿原 lowering、推断、依赖与效果上下文，已完成推断后读取实际属性类型。表达式内部的局部声明、调用和属性引用与普通正文共享观察结果，外层失败或取消时不发布残留局部区域；诊断及导航使用实际表达式范围。

常量求值在原成功边界保留源码常量读取的实际 property 目标与精确导入来源，供声明说明、导航和引用索引共用。前向引用先关联当前求值工作表中的实际定义，成功分配后转换为对应 PropertyId；依赖引用保留原 PropertyDeclarationId。静态初值的试探失败不提交引用，递归求值不把被引用常量的内部使用位置归到调用方。查询复用已求值的声明类型、文档和来源，不重新求值初始化式，也不为没有运行期正文的常量创建函数或初始化单元。

静态单位 variant 的读取保留原构造选择、实际枚举应用及名称位置，与正文中的 variant 使用同一声明来源。成功的静态初值试探连同类型、来源和已观察绑定的所属状态提交，不能单独复制引用却丢弃它使用的 arena；失败试探整体丢弃。限定类型和透明 alias 的查询沿原类型查找，variant 文档按原 M60 variant 目标读取，不将同名 alias 或具体应用变成新声明。

常量中的具名数值内建调用沿原求值器保留所选 callable、已检查参数类型、实参映射和结果类型。普通方法形式与具名中缀形式使用实际名称位置；调用及命名实参的导航、文档、引用和签名帮助复用同一记录。静态调用的类型描述使用已检查声明的 binder，正文调用继续使用原词法上下文，不为常量制造函数或源上下文。只有整个常量或静态初值成功时提交其调用，推断试探、失败求值和取消不发布残留记录。

参数默认值按原参数 owner 与序号标识独立源码区域，使用已经登记的默认模板输入及原求值环境。函数、局部函数、class／struct 构造器和 variant 的默认值共用表达式、捕获、类型约束及效果检查，前序参数引用仍指向原参数声明；不为默认值分配函数。首次真实检查保存源码事实，调用方触发的模板准备与编辑器直接查询复用该结果，之后不为取得引用重新检查已完成模板。失败区域不发布完整正文事实，独立完成的相邻默认值与函数正文仍可查询；取消或失败试探不提交其候选状态。引用与诊断遍历包含未被调用声明的默认值，实例化不把定义处引用复制为调用方源码引用。

跨源码区域的 capture 通过原 BindingId 关联实际定义；参数统一指向原 owner 的参数名称 token，包含 struct 主构造参数。局部类型完成参数接口时只处理本次登记的接口，不重入外层正在准备的默认值。主、次构造器及其参数的文档在共同声明采集入口关联原身份，不依赖注解求值或构造正文检查。

构造签名展示复用原构造候选的完整参数类型、声明 binder 与结果类型；源码默认值仅记录原表达式位置，不因 hover 提前检查或执行。依赖构造器和 variant 沿已有声明接口读取相同事实，保留默认值存在性及其实际来源；调用、构造器函数值、构造声明和参数声明共用真实 callee 与参数序号。variant 的参数形式和原名称保持不变，单位 variant 不虚构带括号的调用。文档使用 M60 的原构造器／variant／参数目标，不借用同位置 nominal 或 property 的注释。

构造委派、实例 property 初始化式、接口委托表达式、`init` 块和次构造正文作为真实源码区域共享编辑查询。区域分别保留原构造器、PropertyId、接口委托 ClassFieldId 或 class 的原成员序号；参数沿原 ConstructorParamId／BindingId 取得定义、类型和文档，不能用合成函数替代构造身份。单例区域归属原 initialization unit，不展示 hidden constructor。区域边界仅覆盖实际表达式或块，无关方法不依赖构造检查成功。

查询复用普通构造委派图、基类选择、共同初始化和次构造正文检查，保持字段初始化顺序及 receiver 非逃逸、捕获、泛型和效果规则；隐式基类委派与循环也进入源码诊断。首次检查同时保存来源事实，类型推断及局部类型已经完成的初始化式不重新检查；多个终端构造器消费同一共同源码时不重复登记引用。字段访问沿原成功选择关联 logical property，隐式字段存储和生成的委托操作没有虚构源码 token。失败构造检查保持显式不可用，独立完成的其他区域仍可查询；取消候选不发布检查状态或来源事实。

实例属性的类型推断在消费其初始化式前沿原查询完成；失败属性不使用分配阶段的临时类型继续检查。其他不依赖该失败类型的初始化式与方法可以独立取得事实，后续字段读取仍须通过原已初始化集合与类型查询。共享初始化中的局部 callable 若生成另一份执行实例，其默认值仍只保留首次源码登记，不把执行副本变成新的源码定义。

来源若保存整个声明范围，导航适配可在对应源码版本中以原 grammar 记录的 AST 声明范围取得名称 token。函数、property、构造器、variant 与参数／表示字段的成功语法 production 将实际 AST 范围直接附着到所属声明索引；引用查询也可沿同一关联从名称位置取得原声明目标，声明结果展示名称范围。CST 节点仍保留自己的边界，不能假定其包含已在外层消费的注解或修饰符。试探回退同时撤销这一附着，不能以最近范围或同名文本代替关联。语义来源、诊断及普通范围查询保持原范围，不为编辑器名称选择改变持久 metadata。

文档 hover 使用 M60 的同一注释标准化、Markdown／标签解析及定义处符号绑定。宿主按源码版本准备 parser 实际附着的注释；编辑态只绑定已取得声明上下文的文档，不要求完整正文，不构造成功 Export HIR 或补齐待推断签名。文档数据保留实际 typed 目标及原声明来源；文档选择使用实际声明目标，不能用共享源码范围合并不同实体。参数的直接文档与所属声明的 `@param` 沿 M60 的优先级读取。源码依赖复用普通 Export HIR 分析产生的已绑定文档，通过已有声明身份映射到导入目标，随原依赖输入失效，不再次编译源码或把文档写入新的产物字段。LSP 只将实际可取得源码的单一符号目标转换为链接，无源码或多重载目标保留标签；普通 Markdown URL 与代码内容沿原文档语法处理。文档绑定在声明边界响应取消，未完成视图不发布。

属性声明和精确 import 的位置查询复用原 logical property，提供声明名称、可写性、完整值类型、扩展 receiver 及原 binder；声明名称的定义导航读取同一属性的来源。声明类型的展示不要求 getter／initializer 执行或无关函数完成；尚未结束的类型推断不暴露内部暂存类型，局部类型属性遵守所属正文的事实可用边界。依赖属性通过原 property 和访问器声明取得类型参数，不重新执行名称选择或产生调用、初始化请求。M60 文档按 property 身份选择，同位置的构造参数仍保留独立文档目标。

类型名称的导航与引用在普通类型查找选定目标时记录：名义声明、typealias、注解和类型参数保留不同的 typed 身份，限定类型路径的各个声明名称分别关联，package 段不冒充类型声明。透明别名的名称引用仍指向别名，不能用展开类型替代；泛型实参及结构类型中的名称沿原递归解析记录。精确 import 的别名只关联实际选中该导入层的使用，词法遮蔽和显式限定不产生别名引用。声明名称位置来自原声明分配，完整正文的关联仅在该正文检查完成后发布；语义试探和取消不留下未提交引用。依赖使用既有类型／别名身份及定义来源，不按类型显示文本或 M60 文档链接重新推断绑定。

类型说明复用上述关联与原声明数据，提供名义声明、typealias、类型参数及精确 import 的 hover。声明名称、泛型参数和约束来自实际 typed 声明；透明别名保留原名称并显示其展开结果，使用处另列已完成的实际应用。应用的显示名称在原解析 binder 上下文中取得，不在退出作用域后借用同名参数或内部编号。导入说明读取冻结绑定及依赖的原声明数据，不将 import alias 当成原声明名称。文档沿 M60 的真实声明／类型参数目标选择；查询不检查无关正文，缺失必要事实或取消时不发布推测说明。

`typeDefinition` 读取已完成的实际类型：值与 property 使用应用类型，调用名称使用原调用检查记录的结果类型，函数声明名称使用已完成的结果签名。类型语法的应用在原类型解析成功时记录；别名使用处按实际实参展开，别名声明读取原目标。`this` 以真实语法 token 定位，并复用已取得的 receiver 类型用于 hover 与类型定义导航；不将关键字伪装成具名局部 binding。名义类型导航复用固定查询的声明来源，类型参数与未具体化的伴随投影保留原参数来源；tuple／普通函数等无独立声明的结构类型不猜测一个成员类型作为目标。未知类型和无源码依赖不产生虚构位置，查询不重新求解调用、实例化正文或根据显示名称查找声明。

类型的实现导航沿已完成的继承关系建立反向索引。接口对应声明阶段已知的子接口及实现类型，class 对应其子类，包含间接关系、抽象类型和 object 的真实声明；每个源声明只返回一次，泛型应用不生成新的实现位置。索引读取原接口闭包与 class 基类链，预编译中间基类和接口保留原声明身份；不按方法名称推导 conformance，也不为声明关系检查无关函数正文。查询位置沿实际类型名称或精确 import 绑定选定目标，透明别名仅在已有完整应用时沿展开类型查找。当前项目各源码 Cone 的结果使用普通持久身份合并；失败声明视图保留不可用状态，取消或输入变化不发布半成品或旧索引。

成员实现导航复用普通方法重写关系、接口 conformance 的已选槽位目标及 logical property 的重写关系。抽象声明连接继承关系，返回值只包含已有源码实现的声明；具体声明自身、接口默认实现及继承提供方均可成为结果。依赖中间声明的关系沿既有 dispatch slot 和 property 引用读取，不再次比较显示名称或签名。getter／setter 合并到原 property，泛型应用、内部 adapter 和待生成方法不产生额外结果。声明关系不要求正文类型检查成功，查询不运行 generator；源码位置上的调用／属性选择仍只能使用普通绑定已经取得的目标。引用索引与实现索引共用声明身份转换，未知签名、来源缺失、取消与当前输入版本继续保持各自边界。

源码依赖的内存接口与正式编译共用声明身份、签名、默认值、泛型正文、别名与 core 协议投影。只有当前源码已完成的 Export HIR 参与此投影；源码 native 契约只收集实际源码引用的类型，具体化后才出现的 callback application 等引用仍由普通后续入口补充。两者共用类型闭合与记录构造，不伪造 LocalConcrete HIR。身份会话直接接收已经构造的 canonical records，为源码和产物分配同一域内的 typed 引用；源码输入没有尚不存在的 MIR／LIR 指纹，同一会话不混用一个 Cone 的多份输入。

分析宿主按所选项目输入保存已载入的产物接口和身份会话，按当前 Cone 的实际依赖源码版本保存完整依赖接口。当前 Cone 的正文变化不重读、解码或检查未变化的依赖；源码依赖变化时保守重建受影响的依赖视图与当前声明分析。完整源码使用当前快照已经解析的严格 AST，语法或语义失败保留真实依赖诊断及不可用状态，不消费上一版成功接口。取消不提交未完成结果；不同 Cone、目标、用途和输入版本的 arena 不混用。

签名层完成后，编辑态正文查询沿原定义处推断入口计算实际依赖的完整表达式正文或属性初始化式。缺失表达式正文的声明没有可供推断的输入，依赖它的结果查询报告不可用；不能静默返回内部待填类型，也不能将正常的推断环或初始化错误当作独立成功结果。

完整函数正文的编辑态关联在普通 local 分配、名称绑定和表达式检查时产生，保存声明、读写、捕获所使用的实际 binding、源码 token 范围及检查所得类型。嵌套 callable 保留原定义根与各自 binder 上下文；类型说明不把内部泛型参数放到外层同名参数的作用域中解释。失败正文的关联不作为完整事实发布，其他函数按需继续查询。取消检查在语句和表达式入口执行，取消状态只属于查询执行，未完成候选整体丢弃，不以合成诊断或成功 HIR 掩盖取消。

编辑器诊断合并当前文件的语法错误、原声明／签名检查和完整函数正文检查所得诊断；损坏正文不伪装成完整输入，其他可检查函数仍继续分析。正文事实与 hover、导航共用同版缓存，诊断查询不再次检查已完成正文。主位置与附注按各自 SourceIdentity 定位到同一工作区快照的实际源码，不把其他文件的 byte range 套到当前文件；缺少源码的附注不伪造位置。源码、配置或依赖变化后更新受影响的打开文档，发布时仍检查工作区 revision 与文档 version；修复、关闭或语义上下文失效时移除旧诊断。

声明分析在输入声明、名义参数、函数／成员签名及重复签名遍历的自然边界检查宿主取消信号。源码依赖的完整 Export HIR 分析也沿同一入口取消，在声明、正文及完成阶段之间检查信号；语句和表达式复用现有正文检查入口，试探和嵌套 callable 共享已经发生的取消。取消结果独立于源码诊断，未完成的私有分析不继续生成或产物封装。普通编译通过不会取消的入口复用同一阶段实现，不增加时间或工作量配额；新的文本版本可重新计算，不能将已取消阶段放入成功缓存。

具名调用的来源关联在普通重载选择成功后记录，目标是当前或导入的原 callable 声明。callee token 由实际 AST 调用保留；函数值调用的局部绑定与内部 invoke 目标保持区分，不把脱糖产生的名字当作用户书写的成员引用。源码函数保留真实名称位置；派生方法没有对应名称 token 时不伪造它。跨文件 hover 的默认表达式取自声明文件，跨产物导航仅在宿主实际提供了对应源码时换算位置。

编辑器输入宿主为当前 sysroot 的已选预编译库关联随包源码：`lib/<target>/<name>.slib` 对应 `src/<name>/Cone.toml`，按相同 Cone 坐标及当前目标的生产源码选择建立 SourceIdentity 到本地文件的映射。默认查找和显式指向同一 sysroot 产物的依赖遵守同一规则；其他位置的同名产物不借用这份源码。关联文本复用普通文件输入缓存和快照，只供导航及位置转换，依赖语义仍消费原 `.slib`，不将关联源码加入源码编译、项目符号索引或重命名范围。源码包未提供时保留产物语义并省略导航；实际读取错误照常报告，不使已可用的产物语义失效。安装根和配置变化后重新选择关联，不保存生产机器的绝对路径，也不扩展 `.slib` 格式。

具名 callable reference（`::function`、绑定／未绑定成员与扩展）及 native `FunPtr` 地址的源码关联同样由普通重载选择成功后记录。目标保留原声明和实际名称 token，adapter、闭包或具体化实例不成为导航目标。失败候选不发布关联；查询复用实际绑定，不再次按名称选择候选。

property 的来源在原声明分配及访问绑定时记录：当前声明保存实际名称位置，引用保留 logical property 的 typed 身份、应用类型与读／写角色。常量折叠、存储访问、getter／setter 和扩展访问不改变导航目标；构造参数产生的 property 指向其原参数名称，不把 backing field 或合成 accessor 当作该名称的声明。导入 property 沿所选声明取得已有来源，没有源码时仍可提供已完成的类型事实。失败正文和未提交候选不发布成功引用。

引用查询按当前输入建立反向索引，复用已完成的声明与正文关联。跨 Cone 的函数和 property 沿普通持久声明身份关联；局部 binding／function 只在所属 Cone 的当前分析实例内解释。函数参数保留所属 callable 和原参数序号，命名实参沿已检查的实参映射指向该参数，不能按参数拼写匹配。声明位置与使用位置分别记录，未完成正文不产生推测引用。首个引用查询范围为选中项目的源码声明和已完成函数正文，索引显式保留未完成分析；无源码依赖仅作为引用目标，不产生虚构声明位置。重命名须另行确认所需语法区域与引用覆盖完整，不能将这一可用事实索引当作完整编辑保证。输入版本失效时旧索引随原语义分析一起释放，命中不重新绑定正文。

完整调用的签名帮助读取普通参数组装所得形参类型和实参映射，保留命名参数、vararg、spread 与尾随 lambda 的真实位置关系。普通源码、导入声明、构造调用沿原参数组装路径记录；函数值调用沿原函数类型检查记录，不给没有名称的函数类型参数编造名字。只有实际调用表达式完成检查后才发布其应用签名，后续正文错误不撤销此前独立完成的调用；失败试探与取消状态不发布。语法光标按该 AST 调用对应的实际括号、逗号和实参范围定位，不按最近 HIR 或参数顺序猜测命名实参的目标；读取签名帮助不重新选择重载或执行默认值。

导入 callable 的 hover 使用该次普通绑定选中的源声明，以其原 owner／callable binder 描述声明签名；应用类型继续来自实际调用事实。源码与导入签名的 callable 身份分别使用各自 typed 引用，不用临时 FunctionId 代表导入声明。默认值只读取已发布事实，缺少源文本时显示存在默认值而不猜测其拼写；导航仅在宿主拥有该实际 SourceIdentity 的文本时换算已知源码位置。

receiver 补全在检查该 receiver 的实际词法与 flow 上下文中取得候选，并与该文本版本的 receiver 事实一起复用。名义成员沿普通访问域、继承覆盖及接口选择查找，扩展沿当前文件的原名称层级查找，并用原约束求解器检查已知 receiver；尚未提供实参时只表示候选，不表示已完成重载选择。缺失或失败的声明使相应候选范围明确不完整，不以空集合冒充确定不存在。安全导航沿普通 Option 内层类型规则查询；共享结果保存成员类别与原 typed 目标，LSP 负责名称编辑和协议表示。

已有 selector 的 receiver／名称对应关系由完整 AST 保存，普通表达式检查完成 receiver 后记录其类型、词法 binder，以及当时按 smart cast／安全导航取得的成员 receiver 类型视图。成员候选在实际查询该 selector 时沿保存的上下文枚举并缓存；诊断、导航与引用索引不为无关 selector 提前枚举补全候选。后续名称尚未匹配或调用失败不撤销此前独立完成的 receiver 事实，正文整体仍不可用；此前已有错误、失败的投机候选或未知作用域中的结果不得发布。LSP 根据当前 selector token 的实际范围替换输入名称，不搜索相近 HIR 或同名声明；嵌套作用域、捕获和 smart cast 均取该 receiver 实际检查时的上下文。候选查询取消时不提交其部分状态，重复查询直接复用同版结果。

光标正文定位先按实际源码与范围筛选，再检查候选的外层正文可用性。一次批量正文枚举共用该次取得的区域集合，不为每个区域重复构造全 Cone 的正文来源；发现新的局部声明后仍沿原正文查询重新选择最内层区域，不复用过期的范围或外层失败结论。

编辑态在当前正文的私有分析状态中检查 lambda，复用普通参数、正文与作用域恢复过程。已确定 header 的 lambda 即使正文随后失败，也保留该状态内独立完成的 receiver 及其类型上下文；不能仅复制引用而丢弃它们所属的 arena。普通编译仍按完整 lambda 提交或回退，重载试探失败的整份分析状态仍丢弃，编辑态保留的事实不构成成功 lambda 或成功正文。

列表恢复使用各 production 的实际结束符与 follow 集合：坏参数、实参或类型项在同层逗号处恢复，后续完整项保留独立语法节点；确定已进入下一项时记录缺失逗号，进入当前列表的 follow 或 EOF 时记录缺失结束符。嵌套分隔符按所在语法处理，结束符留给所属列表。恢复不放宽空项、尾随逗号、参数签名或类型投影等语言规则，已记录错误仍阻止严格 AST 输出。

lambda 参数试探只有实际消费了所属 header 的 `->` 才提交；恢复插入的预期项不能将普通块正文变成参数表。when arm 使用其所在块的层级恢复，保留后续 arm、guard 与正文边界；错误的 case／else 次序和控制体仍按原规则诊断。点号后的缺失 selector 保留已解析 receiver 与显式缺失名称，不能用伪造表达式补全。

**语义与查询。** 共享查询契约使用既有 typed 声明、application、binder、局部实体与源码位置，返回类型、成员、注解、默认值、正文和绑定事实。契约及只读结果属于 IR 数据通道；查询实现在前端复用正常的名称、类型、访问、重载和 effect 规则。M47 从待填方法取得上下文，LSP 从当前文本版本的语法位置取得上下文；协议消息和 UI 数据不进入公共契约。

源码 callable 的身份按原声明位置、实际 owner、binder 和完整签名构造。固定查询可在最终 Export HIR 封装前取得已经完成的声明身份；查询与最终封装使用同一构造路径，不能以 arena 下标替代跨进程引用，也不为查询先构造含有缺失正文的 Module。

正文查询直接借用当前已检查的普通 HIR，或共享依赖已经发布的 typed body；其结果明确区分 Available、PendingGeneration、NotPublished 与 NoBody。空的普通函数体仍为 Available，抽象、intrinsic 和 extern 声明为 NoBody，未知声明引用单独报告查找失败。默认值查询保留原模板、定义处 binder、前参关系与消费声明的类型实参，不在读取时执行或移入生成正文。注解查询按真实目标和 annotation ID 读取普通绑定结果，保持数组逐元素的来源，不重复执行 default。

已有默认模板、泛型正文与查询的逻辑正文投影共享同一套节点转换。投影输入显式借用已完成的类型、调用／局部实体、持久身份和来源关系，不要求先封装完整 ExportHir。当前尚待生成的方法不参加正文投影，已完成正文仍按原节点、绑定、控制流和完整类型读取；依赖读取沿已选中的 callable 关系，不为查询重新运行名称绑定或产物验证。该只读输入视图不是新的成功编译产物。

固定查询按需准备并复用正文投影的持久身份及局部来源关系，同一正文／默认模板的重复读取复用已投影结果。投影借用普通编译图，查询额外产生的类型应用不进入这些输入。依赖调用引用只读取已绑定的完整签名，不要求完成其排队中的实现；读取查询不提前执行后续依赖正文阶段。普通生成结束后仍沿既有顺序完成依赖、效果与成功产物封装。

正文 JSON 适配沿既有逻辑树逐节点编码完整表达式、语句、模式、存储目标、调用与 capture 引用，保留局部 selector、定义／求值来源及精确常量。原定义与默认值应用视图复用同一编码；类型位置通过已有应用映射投影，局部泛型函数签名保留自己引入的 binder 层，requirement 实现片段保持其独立定义类型并另传本次实参。正文查询不输出 arena 下标、Debug 文本或产物字节，不重放已完成的语义校验。具体 JSON 形状见 [M47 正文编码](../milestone47/BODY-JSON.md)。

默认值的应用视图由共享的原定义投影、原提供方的 binder 层级及本次应用的完整类型实参组成。原提供方取模板的 definition root，不能用继承该 default 的 callable 代替。读取应用后的类型时复用普通模板的类型替换；外部 callable／member 的声明身份保持，引用中携带的类型同样应用该映射。局部泛型声明新引入的 binder 保持原绑定，插入的应用实参相应提升层级，不能捕获查询方或局部声明中的同名参数。视图不执行默认值、不复制或改写原定义树。

默认模板输入以实际 callable 与参数位置查找已有模板；必需参数、vararg 空省略或没有模板的引用返回未找到，合法参数不自动产生新的可移植模板。显式应用实参按 definition root 的完整 binder 顺序给出，其元数、种类和 bound 沿原泛型规则检查；缺省实参列表只读取定义。M47 八类查询的请求与结果 envelope 见[查询编码](../milestone47/QUERY-JSON.md)：成组结果保持输入顺序，未知声明与合法空列表分开；协议分发复用共享查询，不包含另一套名称绑定、模板选择或类型判定。

类型描述保留结构类型的完整组成、名义声明身份与实参、原声明参数及其 bound；开放参数必须属于所选 binder 上下文。父类型沿原有直接父关系计算，companion 描述沿实际声明关系形成类型应用，不读取或执行 singleton 初始化。成员描述保留声明处类型和当前名义应用替换后的类型；字段与 variant 引用使用原 typed 身份，不递归展开引用到的其他名义类型。

类型投影区分查询位置的词法上下文与被描述声明的 binder 上下文。读取成员的 declarationType 或原泛型 bound 时，从已返回的真实名义／callable 参数取得声明上下文；宿主参数与方法参数沿原签名分别形成外层和内层 binder。即使查询方存在同名参数，也不能用它解释被描述声明的开放类型。该上下文仅用于类型描述和签名投影，不改变名称查找或成员访问域；M47 适配器在进程内从实际查询结果建立它，对端不提交 arena 参数身份。

M47 的 JSON 投影属于 `scoop-generator-protocol`，其字段合同见[JSON 数据编码](../milestone47/JSON.md)。投影沿既有完整类型、声明引用和注解值形成显式分支，来源使用实际 SourceIdentity 与 byte range。跨进程引用保留 typed identity 的类别与 32 字节内容；读取十六进制引用不创建语言声明，后续查询仍在当前视图中按该类别查找。常量保留整数宽度／符号和精确十进制值，浮点保留 IEEE 位模式。

完整类型输入先解码为既有签名类型的结构与 typed 待解析引用，再在当前源码和已选依赖的声明索引中查找。该索引不以先前发送过哪些描述为依据；未知引用或引用类别不符直接报告查询错误，不按名称回退，也不重新派生或认证声明身份。binder 的 depth／index 只解释为本次真实签名上下文中的参数。类型应用复用原有名义应用、泛型实参数量／bound、函数指针、companion 和表示检查；结构类型使用原 interner。读取类型与声明元数据不赋予源码访问权限，实际名称查找与成员访问仍走普通规则。失败查询只返回本次诊断，不影响普通编译的诊断、使用记录或初始化请求。

完整类型与成员消息沿上述查询保留名义声明的真实来源、注解、原参数及其约束，声明列表按原顺序返回。字段、属性、构造参数和方法签名分别给出声明类型与应用类型。应用泛型方法时，宿主实参属于查询位置，方法自身参数仍属于该方法；协议复用已有的 binder 替换与层级提升，不把两者拼成同一层，也不让同名或相同 arena 参数造成捕获。default 的实参同时保留声明侧映射与本次应用，供同一替换路径投影。访问结论与访问域直接来自普通前端，正文只报告已有可用性，不因读取成员而投影或执行正文。字段形状及上下文规则见 [M47 成员编码](../milestone47/MEMBERS-JSON.md)。

注解声明消息读取普通注解绑定已经确定的参数类型、默认常量、允许目标及 repeatable 规则；源码与依赖使用相同字段，不重新求值默认值。注解参数不继承查询方法的泛型作用域。target 消息保持既有名义／variant／字段／属性／扩展属性／callable／参数／receiver／alias／存储关系，局部 callable 使用其真实生成声明身份；解码 target 只取得 typed 引用，不凭消息创建实体或赋予可见性。

固定查询的声明引用解析以实际声明和组成部分为索引，复用已经构造的身份及定义来源；未携带注解的合法目标同样存在。参数按真实 callable 的值参数位置查找，receiver 及 backing／delegate 目标只在该组成部分存在时成立。未知声明、错误引用类别和不存在的组成部分返回未找到；合法目标上的空注解列表不表示查找失败。字段、variant 字段和参数保留自己的已有来源，receiver 与存储目标关联到所属 callable／property 的定义位置。源码引用解析、逻辑正文投影和成功产物封装复用同一身份构造结果，不为查找先创建含缺失正文的成功 HIR，也不重新执行默认值或读取依赖源码。

依赖声明按其已发布视图提供组成部分：字段、访问器的 storage 实现、native storage、委托模板以及已发布的存储注解目标可确定相应存储存在；仅有 `RuntimeAccessor` 分类不能推出未发布的 backing 或 delegate。隐式访问器参数没有单独来源时沿用访问器定义位置。

存储字段和 enum variant 按实际声明顺序读取；基类字段通过父关系访问，不混入子类声明表。class 的 backing／delegate 字段保留所关联的 logical property 身份，存储元数据不表示源码可访问性或 setter 能力。当前来源取绑定时记录的成员范围，依赖来源取既有 foundation 的对应 typed definition origin；读取描述不寻找上游源码，也不以 owner 的范围替代缺失的成员位置。

logical property 描述独立保留 getter／setter 身份、实现形式、访问域和 override 关系；只读属性没有 setter。给定查询上下文的访问结论沿原有 property／callable 访问检查计算，包含 protected receiver 限制，不由客户端从可见性关键字推测。访问器正文查询复用普通正文可用性；存储或常量访问器无普通正文，abstract slot 为 NoBody，已有源码访问器返回其实际 typed body。

构造器描述沿实际声明读取 primary／secondary／variant 关系，编译器的零参适配器不成为额外源码构造器。完整源码参数按原位置区分 required、default、vararg 空数组省略与 vararg default；主构造参数和字段／property 的关系沿已有 typed 关联读取，不从名字建立关系。参数及 default 描述保留定义身份和原 binder，同时给出所查询应用下的类型与 default 实参映射；查询不执行 default 或将其迁移到另一作用域。

构造器正文使用原初始化投影，保留输入 selector、delegation、基类初始化、primary field store、共同字段／init block 和 secondary 正文的实际结构及执行次序。它不伪装成普通函数，也不省去实际 secondary 正文。当前源码可在成功 HIR 封装前按需读取普通 class／struct 的完整构造计划；原生表示的隐式构造和 variant 没有普通构造正文，返回 NoBody。依赖仅返回已发布的构造初始化模板，未发布时返回 NotPublished。查询与原泛型初始化产物复用投影代码，单个构造器及所属 class 的共同初始化分别缓存；不为读取投影整个成功模块，也不重放构造器语义检查。

当前源码 callable 的完整参数查询复用同一参数接口，包括不经名义成员查找的待填方法。签名查询同时保留已有 release callability；协议适配取得这些真实参数、声明 binder 和效果后，形成完整待填方法签名。参数、context 参数与类型参数使用各自所属声明及位置；不能发送 vararg arena 引用或用另一次名称查找补齐参数类型。

待填方法与普通接口转发方法没有独立的参数源码声明，其合成签名项使用实现类型的触发位置；本地接口与依赖接口沿同一规则处理 context 参数。原 requirement 的来源仍通过实际接口引用取得，不把原接口文件的 span 与当前实现文件组合成一个位置。

待填方法按已选定的实际 requirement 登记普通 override 的默认值来源；默认值准备、不同提供方冲突以及调用省略实参仍复用语言规范 8.5 的路径。生成钩子读取固定签名前已完成这些参数接口，不在 JSON 适配器中重新选择或拼接默认表达式。

方法描述沿名义声明的实际成员关系读取，访问器不重复列为普通方法。定义处与当前 owner 应用下的 receiver、context 参数、值参数、结果和方法自身的泛型约束同时保留；约束替换使用原调用解析的替换算法，开放的方法参数仍有其原声明身份。依赖方法的参数在固定查询视图内只准备一次，并复用已加载 owner 的参数身份，不因查询顺序或重复读取形成不同 binder。正文引用和注解目标指向原 callable；访问结论沿原 receiver 检查计算，声明列表不冒充继承后的候选选择结果。

导出查找指定当前 Cone 或已引入的直接 provider、完整名称段和 type／value 命名空间。provider 筛选先于最长 package 前缀选择；之后只沿真实 static owner 与 companion／re-export 关系查找，不因未命中而改查同名库或较短 package。当前源码只贡献公共导出，包含已解析的 public import；private／internal 声明即使在查询方法内部可访问也不成为导出。结果保留各类实际声明身份和重载集合，未找到与歧义分别返回；查找不读取上游源码、不加载未引入的依赖。

固定生成查询复用前端已有的写时复制快照，一批生成任务共用一份视图；源码、声明和普通正文共享，不为每次查询复制或重新绑定。查询形成的额外类型应用只留在此视图，不成为程序的实际使用，也不请求初始化或附着到原文件的文档／注解位置。新输入的类型片段仍检查普通注解和实际类型约束；失败查询不能使原程序在后续 effect 检查中失败。已取得的当前声明身份缓存由最终封装复用，新增的查询局部 TypeId 不跨回原 lowering。

生成钩子只在普通继承、override 和 default 选择之后保留仍需具体实现的普通方法；必要签名、参数名、实际 owner 与 requirement 映射在普通正文绑定前确定。属性访问器和允许保留的抽象声明不成为生成任务。没有生成钩子的入口保持直接报告缺失实现；有钩子时，未返回的必要方法在生成结束后报告同一语言错误。钩子借用当前前端查询，所有结果收齐后才按原方法作用域 lower 正文，不重跑先前源码的绑定。

driver 的任务规划读取普通 GenerateWith 应用及数组逐元素来源，按实际 owner 与完整接口应用检测重复，再以原 conformance requirement 关系选择待填方法。同一 owner／generator 保留全部有效 bindings 并合并方法；不同 generator 只有争用同一待填方法时才冲突。没有待填方法的组不启动程序，错误配置仍正常诊断。接口方法引用使用原 dispatch slot 身份，源码查询与最终产物复用同一 slot 构造路径；任务消息保留完整接口应用、slot、原方法签名和请求位置。

driver 使用既有宿主进程管理和 Content-Length JSON-RPC 分帧，逐组同步执行生成程序；查询上下文只能是该组已有的待填方法。各组成功结果收齐并检查输出结束及进程退出状态后，再沿严格函数体片段入口解析；最终响应后的额外字节、异常退出及错误诊断均使本次生成失败。warning 随正常正文保留。失败和成功诊断都与本次实际片段原文成对保存，不能仅保留逻辑文件名或将所有错误压到配置位置。具体消息及错误字段见 [M47 协议](../milestone47/PROTOCOL.md)2、5、6 节。

生成正文的词法输入文件与诊断文本可以不同：package、import 和访问域取原方法文件，表达式及局部声明的来源取该片段。片段使用由实际方法声明身份派生的普通逻辑源码名，并由 driver 保存原文和注解关联；名称不依赖 generator 路径或进程顺序。原方法参数仍保留声明处来源。此区别仅服务正常绑定、诊断及源码记录，不引入新的来源认证规则。

产物的来源记录包含这些正文来源，其数量不等于原始输入文件数。single-file 的输入选择仍在 driver 边界完成，产物按已选 Cone 的 executable／source form 分类，不以来源记录数量重新验收单文件输入形状。

错误源码中的声明、作用域和表达式事实按实际依赖分别可用或不可用。依赖缺失语法、失败推断或尚未完成分析的结果必须说明原因；不能用 Any、Nothing、空正文、空候选集合或上一版的成功事实表示当前未知。尚未完成的声明仍影响名称遮蔽与候选完整性，不能简单删除后声称决议成功。每个已知类型／正文都保持完整；恢复状态只属于分析结果，不成为普通 HIR 类型或下游输入。只有正常编译完成必要检查和正文补齐后才发布成功 HIR。

编辑输入独立保存 package 与 import 是否完整。文件头失败使该文件的声明和正文上下文不可用，不中断其他文件的独立查询，也不改变原来源编号。已知声明名称继续按类别、修饰符和普通可见性阻塞相关查找；package 已知时归入原包，未知时保守视为可能属于所查包，不能用根包或空 import 上下文代替。引用索引保留该文件的不可用范围，不能把部分索引当作完整重构依据。普通编译仍要求完整文件头，核心合同诊断沿实际来源范围报告。

完整 import 语法的语义失败按实际导入项隔离。编辑态保留同文件已经解析成功的 import；失败精确导入在原精确导入层占据本地名称，失败星号导入在原星号层保持未知候选边界，不能删除错误导入后选中较低层目标。局部作用域及更高优先级的已知绑定继续沿普通查找规则使用，其他文件的独立声明不受该错误中断。失败导入只保存原源码位置和查找层的不可用状态，不成为类型、callable、别名声明或产物导出；引用索引保留相应不可用范围。普通编译仍报告原 import 诊断并拒绝成功 HIR，编辑态的部分查询不改变该结果。

名义声明的约束、表示字段、构造参数或父类型检查失败时，编辑态保留其原身份与名称，但不把删除错误成员后的较小声明用作成功类型或构造签名。不可用性沿本次普通解析已经取得的类型／透明别名依赖及实际外层声明传播，不重新解析或重放成功声明；合法依赖环不会自行变成错误。类型与布局循环沿原检查结果标记受影响声明，独立声明仍可完成查询，失败节点不能进入继承选择、构造、成员索引或成功正文分析。单个方法签名失败继续沿原方法的失败边界处理，不因此撤销同类型中独立的完整方法。

errno 捕获在完整声明登记后完成的 extern 接口检查仍遵守局部失败边界。编辑态把失败函数标为签名不可用，并保留失败声明的实际源码范围；失败变量继续在原名称层阻塞使用，无关声明可以继续分析。该状态不改变 extern ABI 或普通编译的拒绝条件，不把已登记但未通过该检查的声明作为成功查询结果。

class／struct／interface／enum／object／companion 声明的完整头部与成员输入可以独立于方法正文保存。普通 AST 和恢复声明使用相同的头部数据及名义声明流程；完整成员、完整方法签名和恢复正文分别保留，嵌套声明仍在其原 owner 内登记。已组装的恢复名义输入把实际来源范围关联到原声明索引，嵌套声明同样逐项登记；名称位置仍来自同次 grammar 的名称 token。nominal body 的每次成员 production 明确关联自己的声明，不能以相近源码范围配对，也不能把初始化式、初始化块或损坏成员内部的局部函数提升为方法。struct 的表示字段、默认值及构造器语法必须完整；其原字段和构造调用规则继续用于方法查询。interface 方法的省略／默认正文形式仍决定抽象槽位与默认实现，保留原槽位顺序、继承关系及 conformance 的实际提供方；恢复不生成新的实现或槽位。enum 的全部 variant、字段、默认值及整数常量保持原语法与顺序，方法正文恢复不改变表示或匹配穷尽性；缺失的 variant 信息不能被省略。object 与 companion 的完整头部、单例成员和恢复方法沿同一声明路径登记；companion 保持所属宿主及其泛型参数绑定，不能提升成顶层 object。声明中的构造器、未知字段初始化或初始化块仍阻止该恢复输入成立；方法正文恢复不跳过普通单例初始化、继承与成员规则。只有必要的类型头、成员头部及表示语法均完整时，才提供该名义声明的语义视图；语法未知的成员仍阻塞相应类型，不能通过省略成员制造较小的成功声明。语法完整但语义失败的方法签名沿普通候选不可用规则阻塞相关查找，不抹去已经完整的其他签名。恢复方法的 receiver、泛型参数、可见性和 flow 沿原正文检查上下文解释，普通完整 HIR 的发布条件不变。

源码位置到语义实体的关联由本次正常绑定产生，并保留实际语法目标与 typed 引用；不能从名称、机器符号或最近的 span 猜测。该关联供导航、诊断与重构读取，不接受 generator 提交的绑定旁表，也不改变新正文的普通名称解析。文档事实复用 2.1、2.2 与语言规范 9.7 的既有解析、附着和绑定结果。

**更新与执行。** 一次分析读取固定的源码版本、选中 Cone／target／Production 或 Test 配置及依赖视图。编辑器的未保存文本优先于该文档的磁盘内容；LSP 同一 didChange 中的修改依次作用于前一修改后的文本。宿主在输入变化时更新相应结果，未变化的文本、语法和已完成查询直接复用；首版允许变化文件完整重解析和受影响 Cone 的保守语义失效，不要求先实现局部重解析或通用增量数据库。

LSP 负责协议生命周期、文档版本与取消请求的映射；共享分析负责一致的读取视图、计算中的合作式取消和结果所用 revision。发布诊断、编辑或解析延迟结果前核对相应输入版本，不能将旧范围应用于新文本；取消的请求仍按 LSP 返回响应。M47 使用本次编译的固定视图与串行同步查询，不引入文档通知或编辑器调度。声明与签名查询不启动 generator，读取待生成正文返回 PendingGeneration。

公共源码编辑表示指定 SourceVersion 上的 byte range 替换，不执行文件写入。语法快速修复直接消费原 grammar 的缺失项，只对已确定的分隔符等固定标点提供插入；缺少名称、类型、表达式或模式时不编造内容，也不匹配诊断文案重新判断语法。LSP 在客户端支持 code action literal 与 documentChanges 时提供相应 quickfix，打开文档使用实际客户端 version。延迟 resolve 保存来源版本及缺失项定位，从同版语法结果取得编辑；文本已变化则返回 ContentModified。仅客户端 version 变化而文本未变时可以复用语法，并返回最新文档 version。

工作区符号查询索引已选项目的源码声明，按同一文件实际生效的配置去重；包含顶层声明与名义类型的成员，不将 callable 内部局部声明放入工作区符号表。索引记录 grammar 已取得的名称、容器及指定版本的位置，不声称错误声明已有成功语义身份；签名或正文尚未完成时仍可导航已知名称。每份不可变源码按需复用自己的声明索引，文件修改或移出所选输入后旧记录不进入当前查询。工作区请求使用实际工作区快照，不伪造文档 URI 或 SourceId。引用和重命名仍须使用普通前端的绑定目标，不能以该名称索引代替语义绑定。

`scoop-analysis` 是与编译 driver 同层的分析宿主，保存本地 SourceId、文本 revision 与共享不可变输入；语法结果按文件按需建立并复用。输入更新使旧分析快照失效，读取时检查取消；快照保留原版本的文本，不混用新输入。工作区加载经配套 PM 的 members 与 metadata --offline 取得成员及精确锁定图，再从已展开配置选择源码与 SourceIdentity；不重新解析包声明、匹配版本或下载依赖。未保存清单经版本 1 overlays 输入同一 PM 展开路径，不写回清单或 lock。缺少锁定输入的成员保留本地语法与符号，其他成员可继续分析，完整边界见包管理规范 7.2。不从本地句柄派生语言身份，也不处理 LSP URI、客户端版本或进程传输。`scoop-lsp` 是独立 stdio 命令，传输线程、文档输入与分析 worker 分离；初版通过 LSP 3.17 的基础协议能力提供同步、诊断与结构查询，只声明实际实现的能力。

VS Code 扩展通过标准 LSP 客户端启动 `scoop-lsp --stdio`，管理初始化、文档增量同步、工作区与文件监听、配置更新及退出。服务器依次取显式配置、`~/.scoop/bin/scoop-lsp`、`PATH`；命令不使用 shell 解释。语义配置只传递服务器支持的 target、sysroot、purpose、artifactSearchRoots、packageManager、packageCacheDir、lockfile、configurations 和 activeConfiguration，未指定值沿服务端默认规则。扩展同步 `.scoop` 与打开的 `Cone.toml`，由服务器提供诊断、补全、签名、导航、符号、引用、重命名、选择范围与 quickfix；Tree-sitter 继续提供高亮、折叠和缩进，避免重复注册折叠结果。未找到或启动失败的服务器不影响这些本地语法能力，用户可查看输出并显式重启。受限工作区只启用本地语法能力，信任工作区后启动服务器。扩展不执行编译、sysroot 准备或生成器。

工作区配置明确选取根 Cone、target、Production／Test、sysroot 与产物搜索目录；同一物理源码可属于多份配置，共享文本但分别保存项目语义输入。根 Cone 的 Test 配置使用既有 production／test 源码与依赖选择，源码依赖仍使用 Production。目录、显式源码依赖、显式产物、版本依赖和默认 core 的选择沿普通构建规则；配置错误保留为 manifest 位置诊断，不移除已经加载的源码语法输入，也不隐式执行编译、sysroot 准备或生成器。未归属已选 Cone 的文档仅提供语法服务。

宿主按文件更新时间与长度复用未变的磁盘文本和已打开产物；打开的文档覆盖磁盘，Cone.toml 也遵守这一规则。配置或监听通知（包括 Cone.lock 变化）重新选择项目输入，关闭文档恢复磁盘文本；实际文本未变时保留语法结果。依赖产物只打开一次并按需读取所需 HIR，目录及成员缓冲可在输入宿主与分析线程间共享，文件定位读取互斥；不为共享再解析或核对相同目录。源文件、配置与依赖选择一起进入分析快照，旧项目结果不混入更新后的请求。

编译期 include 输入沿同一分析宿主记录。动态资源监听只为已查询文件／目录输入的源码 Cone 注册根及后代，包含失败请求与源码依赖；定位根和实际根不同则同时注册。资源通知先比较现有输入记录，输入未变不重新分析；失败请求在相应 Cone 的通知后重试。实际输入更新独立推进 revision，使旧快照失效并调度打开文档诊断，无需重载不变的项目配置；普通源码／清单／产物事件仍沿原项目更新路径处理。监听集合随当前配置和分析结果更新，不支持动态注册的客户端保留主动通知与查询时检查。环境值来自语言服务进程，继承父进程新环境需要重启。

支持 relativePatternSupport 的客户端使用以 Cone 父目录为 baseUri 的 RelativePattern，覆盖工作区外的源码依赖，以及 Cone 根自身的删除／重建；目录名中的 glob 元字符按字面量转义。否则使用字符串模式，并保留查询时的输入检查。

进程内分析复用和 2.7、2.8 的构建缓存分别按各自实际输入更新，不增加缓存内容复核、源码镜像、来源认证或通用资源计费。语言服务只读取所需的依赖语义 metadata，不为查询进入 MIR／LIR、机器具体化、LLVM 或 native 链接准备。

### 2.21 异常与 trap 的原生栈诊断

`scoop build/run/test` 接受 `--enable-stack-trace[=true|false]`：无值时为 true，debug 默认 true、release 默认 false，显式值覆盖默认值。它与 `--debug-info`、符号保留及 GC stress 独立；`--debug-info full` 不隐式打开 release 的栈快照。仅消费机器产物的 `scoop link`／`scoop-link` 接受同一显式开关，默认关闭；这些入口没有源码构建 profile，不额外读取 Compile 数据或扩充 `.slib` 来猜测默认值。仅生成 `.slib` 时不固化此选项。

开关属于最终链接输入：有效布尔值进入 executable 更新键及生成的 startup 调用，startup 源码已进入 ResolvedLinkPlan 指纹。切换选项仅重新链接，源码编译和预编译库沿原输入更新规则复用；不向 HIR／MIR／LIR 复制配置，不增加协议／`.slib` 编码或 runtime 构建变体。启动入口新增 `uint32_t enable_stack_trace` 参数，runtime ABI contract 从 29 升至 30，metadata ABI 仍为 9，首次更新按正常 ABI 边界配套重建。

所有已编译的 throw 仍保留 typed 准备调用及其保守 GC 效果，运行时按进程级开关决定是否执行下述采集；关闭时直接返回原对象。`printStackTrace()`、未捕获异常、语言 trap 及 compiler fatal 按运行时规范 5.6 输出不可用提示，传播和终止行为保持原合同。

语言规范 11.7 的 Throwable 使用实际 core intrinsic class 声明，仍可作为普通 class 的基类。HIR 确定它的 nominal identity、继承关系、构造与 printStackTrace 操作；MIR 保留封闭的表示／操作类别，LIR 给出运行时规范 5.6 的 24-byte 基类前缀、完整派生布局和 GC scan。不能只把 Exception 特化而遗漏直接继承 Throwable 的 class，也不能按类型显示名或机器符号重新识别该根类型。

所有首次抛出路径在异常实参求值后、native unwind 前执行 typed Managed 准备操作，包括源码 throw、编译器生成的语言异常及保存后再次抛出。操作完成原对象的保活、物理栈采集、普通 byte-array 快照分配和一次性原子发布；已有快照时直接复用。准备结果为可能搬迁后的同一 Throwable，随后用该结果调用现有 NoGc throw。同步 rethrow 不重新准备或采集。准备操作的真实 GC／内存效果进入常规调用、roots、relocation 和写屏障规划，不能放在现有 NoGc throw 实现中隐式分配，也不能在 roots／statepoint 生成之后补插可能 GC 的调用。

首次抛出准备、throw 与 trap 保留各自实际来源。禁止将采集提前到异常构造、提升到其他用户调用之前，或在 catch／顶层 gateway 中采集当前栈代替原抛出栈。编译器生成的采集及适配 helper 标记为人工位置；已执行的源码内联保留原 inline 调用关系。共用 trap 消息字符串不等于共用触发位置；按 message 合并的 trap block 必须同时保留可区分的 source origin，必要时按位置分块并对诊断调用使用 no-merge／no-tail 约束，不能把不同源码失败点报告成同一行。

所有 profile 保留物理回溯所需的 unwind 信息，覆盖 Scoop 正文、runtime 与实际参与调用链的 generated-C bridge；其生产不依赖源码调试级别。frame pointer 不自动替代 unwind table，删除普通符号表或分离 DWARF 不能删除 `.eh_frame`／目标 compact unwind、必要的异常表及 GC stackmap。用户 native 代码的可展开性遵守其工具链配置，不能通过调试器假造缺失帧。跨 native 帧回溯不放宽运行时规范 5.5 禁止异常跨 C ABI 传播的合同。

语言 trap 进入能采集当前栈并以状态 1 终止的 runtime 入口；编译器可预见的 ABI／不变量 fatal 进入保留 abort 语义的诊断入口。不以直接 `llvm.trap` 代替要求打印诊断的这两条路径，也不将优化器推导的 unreachable 或外部硬件信号统一改为可捕获异常。

运行时在实际打印时消费标准 native 符号与已有的 dSYM／ELF 分离调试信息或 Windows PDB；原始地址采集不解析调试信息、不启动外部 llvm-symbolizer／LLDB 进程。缺少调试文件时仍保留可离线归因的模块标识和地址。独立调试文件的生产改造、运行时符号器接入及验收见 [原生调试设计](../debugging/DESIGN.md) 与 [异常和 trap 栈诊断设计](../debugging/STACK_TRACES.md)。实际外部调试文件不参与运行产物缓存的内容复核。

Throwable 从普通空基类改为 intrinsic 前缀会改变所有派生异常布局，并新增 typed intrinsic／Managed 准备操作。实施时递增实际受影响的 IR／meta 编码和 runtime ABI contract，编译器、runtime、core 及依赖机器产物配套重建；不能混用旧异常对象布局，也不以 runtime ABI 号之外的旁表补救。本文不提前占用尚未编码的 wire tag 或 ABI 版本号。

Throwable intrinsic 前缀落地后，HIR nominal intrinsic representation 使用 tag 15，MIR param-free intrinsic 使用 tag 12，LIR instance representation 使用 tag 7；HIR nominal interface 也使用同一 intrinsic 编码，相关 section 升至 cross-cone-interface/83、cross-cone-type-semantics/31、cross-cone-type-bridge/20、cross-cone-layout-abi/18。runtime ABI contract 从 27 升至 28，metadata ABI 9 不变。派生 class 的 base prefix 引用 Throwable 的真实 instance layout，扫描组合直接继承其隐藏引用槽，不制造源码字段身份。旧库和消费者沿版本更新机制重建。

Throwable 准备在 MIR 的具体正文与协程转换、内联完成后展开为普通 Managed runtime 调用，其结果直接供随后的 NoGc throw 消费。Export HIR／LocalConcrete HIR 仅保留实际 `Array<UInt8>` 的会话内类型引用，MIR 准备调用携带该 exact class，LIR 将其转换为现有 TD 引用；不增加独立类型描述表或调试快照。准备调用和普通打印调用均参与常规 GC 根／statepoint 规划。同步 rethrow 无需重复准备。

`throwable_print_stack_trace` 为 core 的顶层普通效果 intrinsic，参数为实际 Throwable、结果为 Unit；公共 final `printStackTrace` 方法通过它输出。intrinsic 编码使用 tag 30，HIR core-bootstrap-interface／cross-cone-interface 分别升至 17／84。LIR Managed runtime 新增 prepare／print 的 tag 17／18，NoGc fatal 为 tag 26；cone-production／strong-production 分别升至 15／25。runtime ABI contract 升至 29，metadata ABI 保持 9。generated C 的 canonical flags 启用 unwind／asynchronous unwind tables，分别使用新 flag tag 17／18；Darwin 和 Windows 以 tag 19 保留包含叶函数的 frame pointer，使所有桥产生所需的展开信息，真实输入变化自然更新桥对象。生成 C 桥的主定义在 ELF 携带 EhFrame 关联 atom，在 Darwin 另携带 CompactUnwind atom，在 Windows 携带 WindowsUnwindTable／WindowsUnwindInfo atom；边界和 relocation 与桥正文一起进入现有对象指纹及链接选择。旧编译器、runtime、core 和系统库沿正常版本边界配套重建。

### 2.22 原生源码调试与独立调试文件

本节确定基础调试的实现合同，实施状态与定向验收见 [调试设计](../debugging/DESIGN.md) 及其实施记录。基础调试不以 2.21 的 Throwable 布局变更或正式 Scoop 表达式求值为前提。

`scoop build/run/test` 与 `scoopc` 接受 `--debug-info=none|line-tables|full`。none 不发射调试元数据；line-tables 保存文件、行列、函数和内联位置；full 再保存静态类型、词法作用域、参数、局部变量及全局存储。默认 debug=full、release=none，显式设置覆盖默认值。优化 profile 与调试级别正交，不改变 O0／O2、语言语义、实体身份、物理 ABI 或 GC roots。级别沿本次源码编译图传递，预编译依赖保留自身对象已有内容，不为补调试数据自动重建。

2.1／2.2 保留 SourceIdentity、定义及求值来源、准确 byte span 与已有行列记录。HIR 的源码变量保留真实声明关系和词法作用域；生成变量明确分类。2.3 在 CFG、内联、闭包和协程变换中转移实际来源及变量绑定，保留内联调用点链；2.4 将它们关联到真实存储和 ABI，指令、terminator 与新增生成操作同步维护位置。启用的调试记录必须结构完整；无源码对应的生成操作使用明确分支，不继承上一条用户指令的位置。后端不反查 parser／HIR stage，也不从可读名称重建实体身份。

具体化的源码调试位置同时消费当前声明和已选导入 callable／constructor 模板的既有定义来源。导入模板使用 provider 的 SourceIdentity 与 canonical 行列坐标，不能仅因其不在当前 Cone 的导出声明表中就标为生成代码。

文件身份、位置、源码作用域与变量声明集中在公共源码／调试记录中，IR 节点使用有类型的共享引用，不在每层复制完整记录。记录经 IR crate 传递，复用已有声明和变量身份；优化、内联、单态化、closure capture 与协程 saved slot 只新增实际变化的关联。持久化只保留跨 Cone 模板下游真正需要的来源和声明，沿用 HIR 已有来源表；MIR／LIR 的会话调试旁表不作为完整快照重复写入 `.slib`。定义来源与求值来源的用途保持独立，即使它们引用同一位置记录。

跨 Cone 模板的现有局部表增加必填 field 5，保存声明时的局部名称；selector、类型、可变性与定义来源仍占原 field 1–4。该字段在 `cross-cone-interface/82` 引入；包含 2.21 栈诊断 intrinsic 的当前完整编码为 2.6 列出的 `/84`，旧模板产物沿版本机制重建。MIR／LIR 不新增调试 wire 快照。模板实例化、默认表达式及构造片段直接保留该名称，生成项仍由原定义分支区分。源码局部声明复用 PersistentLocalValueId，名称与声明记录共享；内联副本保留原声明，只更新声明所在位置的内联链。词法父子关系从 HIR 已绑定的结构化正文与声明投影，MIR CFG 化前建立公共作用域记录；不为同一正文另存一份作用域树快照。源码参数序号和具体类型沿实际参数／局部关系传给 LIR，不从机器参数名反推。调试类型显示名复用具体化 MIR 的可读类型名称和实参，类型引用及 DWARF 唯一标识仍由 exact type id 确定。全局与 TLS 的调试绑定沿原 property／singleton 定义来源关联到实际静态存储，只在定义该存储的对象中发射；托管全局观察 GC 原本更新的静态根，TLS 使用目标原生定位。生成的失败根和 delegate 协议存储显式无源码变量，不执行 getter 或初始化函数来提供变量值。

非引用局部使用真实存储的 declare 记录，SSA 参数及分片使用 value／fragment 记录，并在实际读取时更新其位置。含 managed 引用的变量只观察程序已实际产生的值，不为调试新增读取、栈槽或 root；safepoint 后使未确认的位置 unavailable，已按该边 live set 回写的 canonical root storage 可立即提供新位置，其余变量等后续真实读取／赋值再更新。normal／unwind 边分别使用该边实际恢复的根，调用结果在旧观察失效之后建立位置。被复制、拆分或消除的聚合不能继续显示旧叶子地址。

已有 canonical root storage 先使用 declare，使 SROA 能按真实布局转移存储分片；GC lowering 后统一转换成带解引用及原 fragment 的 value 记录，避免机器发射将静态 declare 和失效位置混用。参数声明及其观察位置属于实际函数作用域，内联参数属于对应内联函数，不能使用没有正文的独立文件词法块。RS4GC 更新机器 SSA 后，调试观察同步使用支配观察点的最新真实 relocation；无法确定的合流位置保持 unavailable，不为调试建立额外 phi、读取或 root。转换只修改 LLVM 调试记录，不加入读取、栈槽或保活用途的指令。LLVM C API 未提供的记录替换、表达式拼接及支配查询通过小型 C++ 绑定调用对应 LLVM 22.1 API。聚合拆分删除的旧 SSA 节点不得用零值替代其调试观察；未能迁移的观察保持 unavailable。

2.5 使用 LLVM 22.1 的标准调试元数据和 debug records，在优化之前 finalize。当前目标采用 DWARF 5、`DW_LANG_C` 静态类型视图，producer 标明 scoopc；Scoop 基础类型通过 typedef 保留名称。每个实际发射对象保存自身需要的编译单元、文件、函数和类型；函数同时保存源码名称与既有精确 linkageName，包括 line-tables 的最终 DWARF；LLVM 的发射模式不得省略该精确身份。line-tables 只创建函数／位置记录，不创建变量和类型定义。所有字段偏移、大小、对齐与参数分片来自已有目标布局／ABI。引用按真实原生地址描述，不将 LLVM GC address space 当作硬件地址空间。ZST 不因调试增加 payload 存储。

语言规范 11.12 的 canonical semantic source path 与 debugger 文件路径独立。本地源码使用 driver 已知的实际位置；导入正文沿现有来源记录传递，可定位时关联本地源码根，否则保留可由 sourceMap 映射的逻辑路径。路径配置只影响调试生产，不重写 currentSourceLocation。不得在链接时读取源码以重建行表。

2.19 的 SROA、mem2reg、内联与 GC relocation 必须维护调试值。地址变量使用实际存储，SSA 值使用 value／fragment 记录；managed 值跨 safepoint 后只描述更新后的引用。不可定位或已消除的值显示 unavailable，不显示过期旧地址。实现不得为调试额外延长对象生存期、增加 roots、关闭 moving GC 或执行 managed getter／toString。完整 Scoop 求值和 managed 写入另行设计。

2.6 的对象内容及 Code 更新关系包含实际调试信息；影响持久模板数据时才递增对应 meta 编码，不能只为 native DWARF 无条件改 ABI。2.7 的正常更新键包含生产级别及实际路径配置。没有源码行信息的对象不能由 linker 补齐；`scoop link` 和 `scoop-link` 只接受 `--emit-debug-file` 生成已有内容的外置文件，`--keep-symbols` 仍独立控制 executable 普通符号表。

Darwin 输出 `<output>.dSYM/Contents/Resources/DWARF/<basename>`：保留 Mach-O debug map 完成链接，在输入对象仍可用时执行所选 dsymutil，再按符号策略 strip，最后签名。Linux GNU／musl 链接使用标准 SHA-1 build-id，为动态及静态程序的栈快照保留模块匹配标识；该标识不承担授权或防伪用途。输出 `<output>.debug` 时，完整链接后用 llvm-objcopy `--only-keep-debug`，按符号策略裁剪 executable，再写入 `.gnu_debuglink`。所有模式保留展开表、必要 EH、GC stackmap 与运行登记数据；generated-C 桥保留物理回溯需要的 unwind 数据。目标工具、输入及失败按普通 toolchain／linker 合同处理，不增加输出内容审计。

generated-C 桥没有用户源码映射，继续使用 `-g0`，通过实际符号和展开表参与物理回溯；不为人工桥生成伪造的 Scoop 源码行。用户 native 源码沿其原有编译配置提供调试信息。

最终结果缓存同时记录程序及请求的调试结果。请求外置文件时，最终 executable basename 进入结果键并用于内部生成文件名，保证换输出目录可复用、换 basename 时重新生成匹配的 debuglink／dSYM；上游 Cone 不因此重编译。命中只检查输入更新与所需结果存在，不读取／哈希调试文件内容，不启动调试工具或重新准备链接输入。dSYM 目录按普通暂存、复制及 rename 发布；工具失败不发布部分新结果。UUID／debuglink 仅承担平台文件关联。

编辑器复用官方 lldb-dap，scoop-lsp 保持语言服务职责。VS Code 提供源码 breakpoints、构建到 executable 的启动配置、attach、参数／cwd／环境／终端及 sourceMap，按需导入只读 Python formatter。发行包包含匹配平台的 lldb、lldb-dap、LLDB library、Python 支持及实际所需附加文件。不得把 C 兼容 evaluator、通用异常筛选或硬件 watchpoint 宣称为完整 Scoop 求值、Scoop 异常过滤或 GC 感知数据断点。

所有宿主的发行包在公开 `bin` 目录中为 LLVM／LLDB 命令统一增加 `scoop-` 前缀：`scoop-lldb`、`scoop-lldb-dap`、`scoop-dsymutil`、`scoop-llvm-objcopy`、`scoop-llvm-strip`、`scoop-llvm-symbolizer`、`scoop-llvm-nm` 与 `scoop-llvm-readobj`；Windows 附加 `.exe`。不在该目录保留无前缀别名，避免加入 PATH 后与其他 LLVM 安装冲突。编译器查找同目录的调试工具、编辑器的默认 adapter 与性能分析命令使用发行名称；显式选择外部 LLVM 时仍使用其上游名称。私有 `libexec` 布局保留上游工具名。

Windows 的 `bin/scoop-lldb.exe` 与 `bin/scoop-lldb-dap.exe` 使用原生 launcher，定位安装根下的私有 LLDB、Python 和 formatter，并保留用户参数、工作目录、标准流及子进程退出码。LLDB 的 Clang resource headers 与其 DLL 按相对安装布局一同发布，使迁移安装目录后的表达式解析仍可使用配套头文件。

随包 LLDB 为 PE 优先选择已匹配的 PDB；LLVM-MinGW CRT 自带的 DWARF 不能遮蔽程序的 CodeView／PDB 源码、类型和变量。没有匹配 PDB 时仍使用可用的 DWARF 或普通符号，不删除链接输入中的其他调试节，也不改变非 PE 的符号格式选择。

Windows PDB 的 procedure 源码名称用于调试器显示，原生符号名仍保留用于地址关联。调试类型中的 `uintptr_t` 使用目标指针宽度的无符号整数编码；零长度数组和零大小成员保留其实际类型及偏移，不为调试增加 payload。PDB reader 推导的匿名 union 视图由只读 formatter 按原成员读取，不执行目标代码。

Windows/x86_64 的 PDB TLS 位置按选中线程的 TEB、模块 PE TLS index 和变量在模板中的偏移解析到该线程的实际存储，不把初始化模板当作变量值。无法取得对应线程或模块 TLS 时显示 unavailable；调试器不调用目标函数来查询 TLS。

Linux/amd64 静态程序的 TLS 调试定位遵循目标的 variant II TLS 布局：使用当前线程的 thread pointer 与主 executable 的 PT_TLS 大小、对齐和起始偏移，不依赖不存在的动态 linker link map，也不读取其他线程的 TLS。该兼容处理位于 LLDB 的目标地址解析中；编译器继续发射标准 DW_OP_form_tls_address，不增加 runtime 调试入口或执行目标函数。动态加载模块仍使用原动态 loader 定位。

闭包捕获值的调试观察复用 ClosureEnvironment 中的真实 captured-value 身份和共享声明，只在现有字段读取产生 LIR temporary 后记录值；不为调试读取捕获对象、延长生命期或增加 GC roots。相同声明／内联作用域复用一个 DWARF 变量，managed 观察在 safepoint 后失效。人工 helper 使用 DW_AT_artificial，源码 inline 函数仍保留用户属性。
