# Windows targets 实施进度

分支：`codex/windows-targets`。按 DESIGN.md 的 W0～W4 完成全部三个 target；只有完整编译、GC／异常、系统库与宿主验收通过后才声明正式支持。

| 阶段 | 状态 | 尚需闭合 |
| --- | --- | --- |
| W0 工具链与 ABI | 关键路线实验通过 | 复现入口与范围见下文；正式编译器集成在 W1／W2 完成 |
| W1 编译／链接 | 实施中 | 三目标开发 CLI 已支持 subsystem、RC、DLL 数据与跨 Cone C++；cl／clang-cl 消费同一 MSVC 静态库和 DLL 的完整程序通过，继续调试信息、原生输入诊断与组合验收 |
| W2 core runtime | 实施中 | core／std／test 已构建，启动、异常和移动 GC 的真实程序通过；继续线程／callback／unwind 组合及正式 fixture 验收 |
| W3 系统库 | 待实施 | 平台规范与实现、IOCP、全部 native 组件、真实 consumer |
| W4 宿主与发行 | 基础实施中 | 进程中断已实现；继续 Windows 命令、维护脚本、DLL／SDK 布局与打包、交叉编译 |

本地实现和完整验收完成后，才在 Linux `nuc12w.0d0a.com` 与 macOS `mm.0d0a.com` 的 `~/repos` 下新建测试目录，运行与变更范围相称的回归。原始日志与构建目录放在仓库外。

2026-10-28 新增需求：`Cone.toml` 的 `[windows]` 节已支持 `subsystem = "console" | "gui"`（默认 console）和可选 `rc` 文件。Windows toolchain 自带或发现 LLVM resource compiler，资源编译后进入 exe。manifest、产物、链接、缓存及工具供应／发现的定向验收已通过；具体合同见 [设计 3.4](DESIGN.md#34-windows-executable-配置与资源)。

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

COFF section inventory 已接入 generated-C 对象入口，独立区分 `.pdata`、`.xdata`、TLS、CodeView／linker metadata 与实际加载节；已有 wire tag 不变。4 个定向测试通过普通 COFF、MSVC `/bigobj /Z7` 与 LLVM 实际发射的 16 KiB 栈帧 statepoint 对象，确认 stackmap 的 ADDR64 引用解析到真实 callable；Clippy 无告警。Scoop 后端的 target 选择、atom 边界和最终链接继续实施。

三种 Windows 完整 target identity 已接入内部 LLVM 后端，分别映射 Microsoft／GNU machine triple，共用 Win64 数据布局和 SEH frame contract。发射包含真实 `.pdata`／`.xdata`、LSDA、statepoint stackmap、TLS 与 definition atom 边界。callable 代码、注册记录、SEH、stackmap 和常量池使用同一 COMDAT selection；Strong 保持不可重复，ODR 使用可选取的组。浮点常量池保留对齐与 relocation，显式元数据不并入池。Scoop 间接参数发射 caller copy，移除 SysV `byval`，保留 `sret` 并保证调用方临时副本至少 16-byte 对齐。

本机验证通过：三环境 Debug／Release、Strong／ODR 机器对象矩阵；实际 C↔Scoop 24-byte aggregate 传参／返回；浮点常量链接运行、重复 ODR 选取和不同 callable 地址；Windows／Linux switch table relocation 与独立 CString 边界；全部目标 Boolean／statepoint 属性及原 Linux／Darwin 发射回归。identity、LIR、ABI-plan、slib 的 1,005 个单元测试通过，现有 target 编码向量不变；Windows gnullvm 宿主的 workspace/all-targets Clippy 无告警。native 对象测试以 `SCOOP_TEST_CLANG`、`SCOOP_TEST_MINGW_MSVCRT`、`SCOOP_TEST_MINGW_UCRT` 显式选择三个编译器，COFF bigobj 测试还使用 PATH 中的 cl。

这一批完成内部机器发射与直接 native ABI 验证；正式 CLI target 仍待 native 开发资源选择、最终 PE 链接、runtime 和系统库闭环后开放，不将这些定向测试记为完整 Windows 验收。

generated-C 已按三个完整 target 使用宿主 Clang 与实际目标 headers；Microsoft 环境支持 Build Tools／SDK 发现及显式 `msvc.toml`，两个 MinGW 环境使用各自完整 prefix。header/object probe 检查 AMD64 COFF、LLP64、CRT 与 native 环境，保留 SEH unwind 信息；不执行目标程序或提前要求最终链接依赖。producer 记录新增 Windows 分支，现有目标的编码向量保持不变。

两项工具发现测试通过实际 headers、Unicode 输入／输出路径、producer 记录往返、显式 SDK 和错误 CRT prefix 诊断；四项 generated-C 对象测试（含三个环境的真实桥接编译、COFF 边界、native 链接与执行）以及七项 C producer 合同测试通过。workspace/all-targets Clippy 无告警，最后的测试 fixture 修订经 codegen all-targets Clippy 与定向测试确认。正式 native C/C++ 覆盖、PE 最终链接和 runtime 仍继续实施。

native C/C++ 编译现已区分 Clang GNU、clang-cl 和 cl 的实际参数／依赖输出，Microsoft 默认使用 `/MD`、UTF-8 与 `/EHsc`，Debug 对象包含 CodeView。`--cxx` 从 CLI 经 protocol 18 传递，`--cc` 在 Windows 只选择用户 native C；generated-C 保持独立 Clang。host 工具选择保留 clang++ 的 driver 名称，Windows 可按 `cl` 查找 `cl.exe`。普通发现记录包含工具、真实头文件和搜索候选；Microsoft 自动发现记录 SDK／toolset 版本目录的变化。Windows 编译参数使用不改变路径含义的兼容拼写，预处理行标记支持实际 UTF-8 octal escapes 和 cl 的 `#line`。

本机真实 native 编译／链接／运行矩阵通过 clang-cl、cl、MinGW MSVCRT 与 MinGW UCRT；两个 Microsoft 编译器均消费实际 cl 构建的 C++ 静态库，覆盖 C/C++ 混合源码、STL 返回值、RTTI、内部异常、公开 Scoop 头、Unicode 路径／头文件、CodeView 和实际 header 更新。59 项 manifest 与 39 项 protocol 测试、两项 SDK 发现测试、两项 GCC 调用合同回归和一项输入更新测试通过，相关 Clippy 无告警。正式 Windows CLI、完整 PE 产物消费／链接、runtime 与系统库仍未开放；上述验证不替代后续完整验收。

W1 的 Windows final-link profile 已完成内部工具调用：Microsoft 由 clang-cl 调用选定的 lld-link／link.exe，并使用所选 SDK／toolset；两个 MinGW 环境由目标 prefix 的 Clang driver 选择 compiler-rt／libc++。三个环境均静态加入各自的 Scoop LLVM unwind archive，使用 Unicode wmain、ASLR／NX、REF 和 no-ICF。真实 PE 验证包含正确 CRT imports、SEH、内部 C++ 异常／RTTI／STL、Unicode 路径／参数、相邻 PDB 名称与 map。普通更新输入已能取得所选 CRT／startup／辅助 archive 的实际 linker trace。

五种本地组合（Microsoft C、cl C++＋LLD、clang-cl C++＋Microsoft LINK、两个 MinGW C++）编译／链接／运行通过，workspace/all-targets Clippy 通过。MinGW 保留原生 PE subsystem 默认值：强行指定 10.0 会触发现有 CRT load configuration 的 loader 拒绝；同一输入恢复默认值后运行成功，Windows 10 的 API／支持基线不变。这一批仍是共享工具 profile；Scoop `.slib` 的 PE library resolver、PDB 发布／缓存与完整 runtime 集成继续实施，正式 Windows CLI 尚未开放。

PE profile 已接入 artifact linker：普通 AMD64 COFF／bigobj 符号读取、COMDAT／common／TLS 分类、短 import object 与普通 archive 成员分开处理，Windows 原生库按内容区分 static／dynamic。显式库绑定保留实际导入接口；CRT defaults、弱 alias 与 `.drectve` 交给所选原生 linker。PE 链接生成 String TD alias、只读 `.smap`、目标 quoting 的 response file，并把实际系统库 trace 接入更新输入。Windows 默认输出及缓存 executable 使用 `.exe`，PDB basename 纳入 executable 键并随结果发布。

四种组合（MSVC＋LLD、MSVC＋LINK、MinGW MSVCRT、MinGW UCRT）的定向测试通过真实普通静态库与 DLL import library、未选成员排除、generated startup 的 image/root 顺序、String alias、只读 stackmap 节、PDB／map 和 Unicode 路径。启动使用实际 runtime UTF-16 转换，覆盖空参数、BMP／非 BMP、引号／尾部反斜线、非法 surrogate 拒绝、native atexit 期间参数有效及完整 32-bit 退出码。MinGW 启动的私有 root 声明使用 hidden visibility，避免将本 image metadata 当作可自动导入数据；response file 由 LLD 解析，避免 Clang GNU parser 提前消费 Windows 路径的反斜线。完整 `.slib`／runtime／CLI 和缓存发布验收仍待 W2 闭环，不以这些边界测试替代。

## W2 runtime 基础

通用 runtime 的 mutex、condition、once、GC worker create/join、对齐分配和计时已通过有限的 platform primitives 调用。Windows 使用 SRW exclusive lock、condition variable、InitOnce、CRT `_beginthreadex`、thread handle、配对 aligned allocation，以及 QPC／GetThreadTimes；Darwin／Linux 的 pthread／POSIX 行为保留在对应实现。停稳等待遵守 OS 计时粒度；没有增加 pthread DLL、全局 timer-resolution 修改或新的 GC 协调机制。删除未被使用的私有 OS thread id 字段，私有 itable C 字段改名为 `interface_type`，避免 Windows SDK 的 `interface` 宏冲突，二进制布局不变。

三个环境均通过实际四 worker 并发测试，覆盖互斥计数、condition signal／broadcast／timeout、once publication、C TLS 隔离、join、动态 mutex、单调／线程 CPU 时钟和 8～128-byte 对齐分配。13 个受影响的 runtime C translation unit 在每个环境以 `-Wall -Wextra -Werror` 编译通过，包含启动统计分支。POSIX 构建与现有 native C 测试入口已加入同一实现；现有宿主的运行回归仍按约定等待本地完整验收。Win32 VM／PE 元数据／SEH 栈扫描和完整 GC／语言运行尚未完成，不以 primitives 测试作为完整 runtime 验收。

## W4 宿主基础

`scoop-process` 已实现 Windows console handler，按实际 process handle 终止 child，关闭控制台时在 handler 返回前等待 child；中断后停止启动新工具，普通退出保留完整 32-bit 状态。真实子进程测试覆盖 Ctrl+C／Ctrl+Break／close 三个 handler 分支、两项并行 child、drop 回收、中断拒绝新任务与高位退出码透传，在 Rust MSVC 与 gnullvm 两种宿主 target 上通过；相应 Clippy 无告警。完整 CLI 的系统控制事件验收随宿主命令完成，不以这里的 handler 调用测试替代。

宿主 LLVM 22.1.8 已用 LLVM-MinGW UCRT 完整构建并安装 LLVM／Clang DLL、Rust 开发输入及 Clang／LLD／对象工具。修复遗漏 `libclang-cpp.dll` 导致误加载 PATH 中 ESP32 DLL 的问题，并将 inkwell 的 target feature 与实际 AArch64／X86 backend 对齐。安装后的 Clang 生成 AMD64 COFF 成功；gnullvm 宿主的真实 codegen 测试通过 O0／O2 的 statepoint／异常发射及 AArch64／X86 Boolean ABI。相关 Rust Clippy、Python Ruff 与三个脚本测试通过。

Windows 构建选择支持 UTF-8 的 Ninja；实际编译器依赖记录包含 Unicode 外部头文件，修改头文件会使结果更新。完整构建后的原命令直接复用，保留源码和工具，确认安装工具、DLL 与 Ninja 输出 metadata 未改变。日志、源码解包和构建目录均在仓库外。

Windows 宿主已使用相邻 `.exe` 命令／generator 和 `%LOCALAPPDATA%/Scoop/Cache`。`run` 通过 `CreateProcessW` 执行选中的缓存程序，同时保留发布路径作为 `argv[0]`、原 UTF-16 参数、cwd、环境和标准流；仅继承选中的标准流 handle。文档发布在现有文件锁内替换目录，发布失败恢复旧站点，恢复也失败时保留并报告旧目录位置。

真实进程的参数／cwd／退出与回收测试在 MSVC、gnullvm 两种 Rust 宿主通过；generator 的 4 个测试、文档目录替换／实际文件锁失败恢复的 2 个测试及相关 Clippy 通过。gnullvm `scoop.exe` 已构建，配套 `libunwind.dll` 后，实际外部命令验证 Unicode／空参数／引号／反斜线、原始二进制标准流、环境／显式／安装位置 sysroot、完整高位退出码；系统实际发送 Ctrl+C／Ctrl+Break 时 CLI 与已登记 child 均退出。依赖 POSIX shell 的旧 compiler 测试仅在 Unix 运行；Windows 正式编译 CLI fixture 和完整发行脚本仍待 W1／W2 闭环后接入，不将当前宿主验证记为完整 Windows target 验收。

上游 GC 演进（#57）已合并：保留分片标记、phase 参与数量发布与独立 quiescence 唤醒，将新增 reservation 锁接入 platform 同步操作。统一 runtime ABI contract 25 合并 Windows 启动与上游五指针分配视图，并同步固定编码向量。workspace/all-targets Clippy、21 项 ABI 测试、三个 native 环境的同步与 17 个 runtime C 单元编译、5 项 Windows codegen 测试通过；POSIX 运行回归仍在本地完整验收后进行。

Win32 ThreadVmOps 已实现栈范围、页／CPU 数、对齐 reservation、读写 commit、保护页、整页 discard、整体 release 和 RSS。GC 对齐映射移到 OS 层，POSIX 共用已有 mmap／裁剪逻辑，Windows 只提交可用区间并释放原 reservation。三个 native 环境的实际程序验证 16 MiB region／64 KiB 大对象对齐、保留区整体释放、discard 后重新写入、保护页失败路径、主线程／worker 栈增长与 RSS；Clippy 和 C 警告检查通过。该批尚未接入完整 Windows runtime profile，后续随 PE image 与 SEH frame 实现组合。

PE metadata image 组件已从承载 runtime 的加载模块读取 section RVA／VirtualSize，以非 discardable 区间建立 metadata 地址范围，并读取 `.text`／`.smap`，不重新打开 exe 或包含文件对齐 padding。已有 PE 集成测试接入实际组件，在 Microsoft LLD／LINK 与两个 MinGW CRT 中验证生成 registration、静态存储、stackmap 长度及 DLL 数据边界；定向 Clippy 和 C 警告检查通过。真实 stackmap 解码与上下文扫描随 Windows SEH frame 实现闭合。

Windows JSON／CLI 数值后备已通过 platform 入口使用固定 gdtoa C parser。固定 mingw-w64 2026-09-17 修订及许可证，只保留所需解析源；私有符号与平台锁避免混入 CRT 的另一份 gdtoa 状态，不读取或修改应用 locale。三个 native 环境验证 Float 直接最近偶数舍入、Double、400-byte 输入、overflow／subnormal／signed zero、嵌入 NUL、Win64 tagged C 返回及四线程并发；global locale 与 UCRT 已有 thread locale 保持，Clippy 和所有相关 C 源的严格警告编译通过。Darwin／Linux 的原 libc 私有 locale 实现移到 platform/numeric，运行回归仍按约定在本地完整验收之后执行。

Windows startup 在进入 coordinator 前将有效 stdout／stderr CRT descriptor 设为 binary mode，`process.c` 使用对应 CRT 的 `_exit` 声明。PE 集成测试在 Microsoft LLD／LINK 和两个 MinGW CRT 中接入实际 I/O／退出实现，验证 LF、CR、NUL、0x1a、0xff 字节保留、缓冲刷新、高位退出码，以及正常返回执行 atexit、立即退出／trap 跳过 atexit；定向 Clippy 和 C 严格警告编译通过。测试的 coordinator 为边界替身，完整 managed 退出／GC 协调仍随 runtime 集成验证。

Windows managed entry shim 和独立 frame reader 已接入同一平台 bundle：shim 按 Win64 传递原始 PC／RSP／RBP、提供 shadow space 与 SEH 表；reader 按真实 RSP／RBP location 写根，并由 `RtlVirtualUnwind` 恢复上一帧，不使用 SysV FP 公式。固定 RBP profile 不需要扩展生成代码的 transition ABI。三个环境的 O0／O2 实际 LLVM 程序通过 0～3 个入口参数、返回 carrier、16 KiB 嵌套帧、主线程／worker 精确 root 更新、native boundary 和错误 location 检查；系统展开 shim 的结果与其发布的 anchor 一致。平台同步／VM／数值测试同时通过，定向 Clippy 和 C 严格警告检查通过。完整 runtime 编译、managed GC／异常／callback 组合及正式 target 开放继续实施。

完整 Windows runtime build profile 已组合全部公共 C、Windows OS／PE／frame、数值 parser 和入口 shim；三个环境的全部源文件在 GC stress／startup stats 分支启用时通过严格警告编译、runtime ABI 目录检查、索引往返和完整 native 对象集的链接运行。Microsoft SDK 路径放在 Clang builtin headers 之后，修复 C11 atomic header 选择并验证 pointer atomic 始终 lock-free；COFF runtime 目录允许 CRT helper 的正常 weak 合并，真实重复 strong 入口仍报错，shim 使用 NoDuplicates COMDAT。Clippy、四项平台组件矩阵及四种 PE 链接组合回归通过。这里的完整对象链接使用 native C consumer，未替代 managed startup／GC／异常验收；正式 CLI target 仍在后续闭环验证后开放。

scoop.test 的 native 报告通道已适配 Windows：使用所选 CRT 的 descriptor 复制／恢复、宽字符结果目录、二进制文件和 QPC 计时。三个 native 环境的 capture／nocapture 六种实际进程组合通过 Unicode／非 BMP 路径、JSON 事件、LF／CR／NUL／0x1a／0xff 字节及重复捕获后的标准流恢复；C 格式化与严格警告检查通过。正式 Scoop Cone 测试仍随 core／std 的真实 CLI 构建闭环验证，不将 native 通道测试计作完整测试框架验收。

三个完整 Windows target 已接入开发 CLI，默认选择 Microsoft UCRT。实际构建修复了 compiler child 的 DLL 搜索环境、Windows compiler 栈 reserve、runtime Clang 路径、Windows artifact summary 解码，以及准备脚本的 UTF-16 路径传输。工具发现更新版本使旧 include 顺序正常失效；三个 target 的完整 runtime 分别准备并发布，Unicode cache 路径与未变化 runtime 的复用通过。ELF／COFF 共用每份 symbol surface 的一次名称索引，消除每个 callable 重建全 Cone 名称的开销；MinGW 全局地址的 refptr 归入 callable 常量贡献。程序链接复用 runtime 已合并的 weak helper，允许用户 native 中相同的 COFF helper 正常合并。

三环境的 core／std／scoop.test 均通过实际 release CLI 构建；hello、try/catch/finally、String 移动根、boxed struct／tuple／enum／context 与 unwind 后收集的四组真实程序全部运行通过，scoop.test 自身每环境 4 项测试通过。正式测试还修复了带 `\\?\` 前缀报告目录的路径拼接，并移除 manifest 中重复且不适用于 clang-cl 的 C11 flags。相关 Rust 格式化／Clippy、Python Ruff、ELF 索引与 Windows codegen／refptr／常量池、registry／summary／工具发现测试通过，C 变更在三环境严格警告检查通过。这里是开发 CLI 的定向组合验收，尚未替代正式 fixture 全量、线程／callback／GC stress、系统库和发行验收；未开始远端测试。

正式 fixture runner 的进程层已支持 Windows：挂起创建后先加入独立 Job，再恢复初始线程；完成、超时和中断回收该用例的进程及后代。实际 Python 子进程验证 UTF-16 参数（含非配对 surrogate）、Unicode 工作路径、原始字节流、完整高位退出码、立即派生的子进程回收与超时。公共规则测试共 62 项，59 项通过、3 项仅适用 POSIX 的 signal／byte argv／FIFO 检查跳过，Ruff 通过；Windows target／工具选择和正式用例接入继续实施。

fixture runner 已接入三种完整 Windows target、各自的 Clang／archive companion、显式 native／unwind prefix 和 Microsoft SDK 配置；文本读写明确使用 UTF-8。fixture 文本由 Git 属性固定 LF，修复 Windows autocrlf 导致诊断字节位置与 golden 不一致的问题，不归一化实际诊断。公共规则测试扩至 64 项（61 通过、3 项 POSIX 专项不适用），Ruff 和 native C 严格警告检查通过。三个环境各通过 16 项异常／诊断 fixture 与 2 项真实 native scalar／aggregate 返回 fixture；Win64 native sret shim 保持 RCX／RAX 和 SEH 合同。定向 stress runtime 已分别准备，现有 `roots-gc-context` 和 `string-copies-moving` 在三个 target 均复现 managed 返回 PC 缺少精确 stackmap 记录，保留失败并继续修复；对应 release variant 尚未执行，未计为通过。`exceptions-return-paths` 的另一组 compiler 专测尚未扩展 target，本批单列为不适用。

`[windows].subsystem` 已贯通 manifest、executable `.slib`、编译更新键和最终链接更新键；默认／显式 console 与 gui 不改变 Cone identity 或 Scoop entry。三个目标的正式 fixture 各通过 15 个实际进程步骤，验证 PE subsystem、Unicode argv／退出码、GUI／console 切换、未变化时零 compiler child、artifact-only 重链接，以及 GUI Cone 的测试 executable 仍为 console。非法取值和 library 使用该节的诊断位置由正式 CLI 用例验证。

5 项 manifest／slib 定向测试、workspace/all-targets Clippy 与后续修改的定向 Clippy 通过；fixture runner 的 host canonical path 引用保持实际诊断原样，公共规则测试为 65 项（62 通过、3 项 POSIX 专项不适用），Ruff 通过。RC 配置与资源编译仍待实施；上述 subsystem 验证未替代前述 GC 返回 PC 修复和完整 Windows 验收。

移动 GC 的返回 PC 失败已闭合：LLVM 22.1.8 的 X86 statepoint emitter 将记录标签放在 WinEH NOP 之后，导致部分记录比实际返回地址多一个字节。配套补丁将标签移到 NOP 之前并保留 EH padding；runtime 的精确匹配规则保持不变。所有发射 Windows 对象的宿主均通过 LLVM 准备脚本应用该补丁，未变化命中先于补丁读取／应用。

修补后的宿主 LLVM 完整构建、安装与后续原命令复用通过；真实 COFF 回归覆盖三个 target 的 call／invoke，检查准确 PC 和必要 NOP。配套三个 CLI 与三个目标各自的 core／std／test 已重新构建；`roots-gc-context`、`string-copies-moving` 和新增 Windows 适用性的 `exceptions-return-paths` 全部通过，每目标 5 个变体／10 个进程步骤，包含 debug／release 的移动 GC。对应 Rust Clippy 和 Python Ruff 通过，先前两项 GC 失败与返回路径不适用项均在本批闭合；线程／callback、其余系统库和完整宿主验收继续实施，未开始远端测试。

`[windows].rc` 已使用所选目标的 Clang、headers 与 `native.include` 预处理，通过 LLVM `llvm-windres` 和 `llvm-cvtres` 生成资源 COFF，并作为现有 native 对象进入 executable `.slib`。资源更新记录跟随实际头文件、引用资源及搜索候选；测试 executable 省略生产资源。配套 LLVM 已安装 windres 入口，固定原 Ninja 后的完整准备命令直接复用现有安装。

三个目标的资源正式 fixture 各通过 19 个实际进程步骤，覆盖 GUI exe 的真实 RCDATA、native C 与 RC 组合、C flags 不泄漏到 RC、资源／头文件更新、未变化时零 compiler child、artifact-only 重链接及移除 RC 配置后的更新。非法 RC 路径的正式诊断位置、4 项 manifest 测试、2 项资源测试（含三个目标的真实 COFF／Unicode／缓存与失败不发布矩阵）通过。用例同时发现并修正了预处理文件编译时多余的 clang-cl 语言／SDK 参数与 MinGW C++ 选项；clang-cl 的 SDK headers 使用系统搜索顺序，保留 Clang builtin headers。四种 native 编译器组合的严格警告回归、3 项 native 对象更新测试、相关 Clippy、Ruff 与格式化通过。完整 Windows 系统库、宿主发行和远端回归仍待后续完成。

Windows 非 TLS native global 现通过普通 IAT 引用访问，产物中的实际 `__imp_` 引用保留原数据契约与显式 library 归属；静态 archive 和 DLL import library 使用同一份 `.slib`。三个目标的正式 fixture 各通过 9 个进程步骤，覆盖可变 scalar／C-layout struct、只读数据独占的 archive 成员、addressOf 与原生 getter 的地址一致性、写入可见性、本地 native TLS、artifact-only 切换到 DLL 及 warm 复用。受影响四个 crate 的 Clippy、后续 codegen Clippy、C／Scoop 格式化和 Python Ruff 通过。

以上 DLL 数据 fixture 最初使用三个目标各自的默认 LLD。额外的 Microsoft LINK CLI 验证发现既有 COFF 常量节关联到后方函数节，触发 LNK1243；该失败由下一批 COMDAT 顺序修正闭合。native C++ 的 CLI 契约汇总还需接入 Windows 已有的目标 C++ 配置；调试信息及后续 W2～W4 仍未完成，未开始远端测试。

COFF atom 物化现在将所属 COMDAT 排在关联节之前，只重排 section headers 和 section 序号，保留 symbol 索引、relocation 目标及机器内容。真实普通 COFF 与超过 65,535 个节的 bigobj 均经 Microsoft LINK 链接并运行通过，确认 association 高低位、代码和 relocation 保持正确。codegen Clippy 无告警；fixture runner 可显式选择 `--linker`，65 项公共规则测试通过 62 项、跳过 3 项 POSIX 专项，Ruff 通过。

三个 release CLI 与三个目标各自的 core／std／test 已重新构建。Microsoft UCRT 的 LLD／LINK、两个 MinGW CRT 的 LLD 共四种组合均通过 `windows-native-data-static-and-dll`、`roots-gc-context`、`string-copies-moving` 和 `exceptions-return-paths`，每种组合 6 个变体／19 个进程步骤；包括 debug／release 移动 GC、完整静态库和 DLL `.slib` 链接。原 Microsoft LINK LNK1243 已闭合，后续 native C++、调试信息和 W2～W4 继续实施。

Windows 跨 Cone C++ 需求现由所选目标的 final-link profile 提供，保留显式 native library 要求。generated-C 的原生 `.pdata`／`.xdata` 留给 COFF linker，不要求 Scoop atom 归属；Scoop LIR 的 unwind atom 与普通引用检查保持。真实 `captureErrno` 桥接发现的 orphan relocation 已闭合。内部 C 对象关闭当前 COFF 时间戳，新增 C producer flag 20；真实对象检查确认时间戳为零且 SEH 表保留。七项 C producer 合同测试、七项 relocation 测试及三环境真实工具链测试通过，相关六个 crate 的 all-targets Clippy 无告警，C++ 严格警告检查通过。

三个 CLI、各目标 core／std／test 及普通／GC stress runtime 已按更新后的 producer 配置准备。四种组合（clang-cl＋LLD、cl＋Microsoft LINK、两个 MinGW CRT）均通过 `native-cxx-artifacts` 的 debug／release 两个变体、10 个进程步骤，覆盖跨 Cone C++ 需求、STL、RTTI、内部异常、静态初始化、errno 与 artifact-only 重链接；debug 重链接保留相同符号配置及 PDB basename，精确链接指纹比较通过，原 MSVC 时间戳不稳定失败已闭合。独立 `errno-values` 正式 fixture 在三个目标各通过 4 个进程步骤。

完整 CLI 另消费 W0 已构建的真实 C++ 静态库和 DLL import library：两个 Microsoft wrapper 使用同一份 cl 产物，两个 MinGW wrapper 使用各自 libc++ 产物。四种组合均通过 native RTTI／STL／throw-catch、Scoop catch／移动 GC 和两次 native 静态析构。调试源位置信息、剩余原生输入诊断及 W2～W4 继续实施，未开始远端测试。
