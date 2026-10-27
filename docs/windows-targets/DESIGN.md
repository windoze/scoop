# Windows MSVCRT／UCRT target 与工具链设计

状态：实施中，W0 关键路线实验通过，继续 W1～W4；尚未开放正式 Windows target。日期：2026-10-27。本文覆盖 MSVCRT、UCRT 两种 CRT，并按 native 工具链环境区分完整 target。实施与验收记录见 [进度](PROGRESS.md)。

依据：[语言规范](../specs/SCOOP-SPEC.md) 12～14、[实现规范](../specs/SCOOP-IMPL-SPEC.md) 1、2.4～2.10，以及[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) 3～7。实施时先将本文对应阶段的精确合同纳入这些规范，再改实现；当前三个已实现目标的合同保持有效。

## 1. 决策

完整 target 同时确定架构、OS、native ABI／标准库环境和 CRT。MSVC 与 LLVM-MinGW 即使同用 UCRT，也不能隐藏在同一个 target 下；它们的 C++ ABI、startup 和运行库配套不同。Clang 是编译器实现，既能提供 MinGW 环境，也能以 clang-cl 提供 MSVC 环境，不能直接等同于 libc++。

保留 MSVCRT、UCRT 两种 CRT，并支持以下三个明确组合。UCRT＋MSVC 为 Windows 默认目标：

| Scoop canonical target | native 环境 | CRT | C++ ABI／标准库 |
| --- | --- | --- | --- |
| `x86_64-pc-windows-msvc-ucrt` | Microsoft SDK／MSVC toolset，cl 或 clang-cl | UCRT | MSVC C++ ABI、MSVC STL／VCRuntime |
| `x86_64-w64-windows-llvm-mingw-msvcrt` | LLVM-MinGW，mingw-w64 配置 `msvcrt` | 旧 `msvcrt.dll` | MinGW／Itanium C++ ABI、libc++／libc++abi |
| `x86_64-w64-windows-llvm-mingw-ucrt` | LLVM-MinGW，mingw-w64 配置 `ucrt` | UCRT | MinGW／Itanium C++ ABI、libc++／libc++abi |

这些是 Scoop 的 canonical target 名称，不要求 LLVM 原样识别扩展后缀。Microsoft target 映射到 LLVM `x86_64-pc-windows-msvc`；两个 LLVM-MinGW target 映射到 `x86_64-w64-windows-gnu`，各自绑定配套 CRT 资源。driver 的有限目录保存这张映射，不让 LLVM triple 丢掉 Scoop 的环境／CRT 身份。

对应 target profile 为 `org.scoop-lang.target-profile/windows-x86-64-msvc-ucrt/1`、`org.scoop-lang.target-profile/windows-x86-64-llvm-mingw-msvcrt/1`、`org.scoop-lang.target-profile/windows-x86-64-llvm-mingw-ucrt/1`。manifest 均使用 `os = "windows"`、`arch = "x86_64"`，`env` 分别为 `msvc-ucrt`、`llvm-mingw-msvcrt`、`llvm-mingw-ucrt`；沿用已有条件语法，不另设独立的 CRT／工具链覆盖开关。

裸 `windows-ucrt` 或 `windows-msvcrt` 不作为 canonical target；不根据机器上恰好装了哪个 compiler 静默解释含糊名称。现代 MSVC 配套 UCRT，不登记 MSVC＋旧系统 MSVCRT 的虚构组合。这里的 LLVM-MinGW 合同明确绑定 libc++；如果以后实际支持 GCC／libstdc++ 的 Windows 环境，也应有自己的 target identity。

三个 target 共用可复用的 Win64／COFF backend 实现，C ABI 为 Win64，对象为 AMD64 COFF，程序为 PE32+；target 数据仍分别保存 machine triple 和实际环境。`.slib`、runtime、native archives、sysroot 目录和缓存按完整 target 区分。同 CRT 不等于产物相容，普通 target mismatch 检查即可阻止错误消费，不另立一套 C++ 身份或来源资格机制。

`cl.exe` 与采用 MSVC ABI／STL 的 `clang-cl` 可以服务同一个 MSVC target。具体 compiler 路径、版本、SDK 位置和实际 flags 留在现有 producer／更新配置，不放进 canonical target；这不免除 MSVC toolset 版本、CRT linkage 等实际兼容要求。

首期为 x86-64 CPU 基线、Windows 10 x64 及以上；具体发行最低 build 由最老验证环境确定并写入 `scripts/release.toml`。MSVCRT 的存在不构成 Windows 7 兼容承诺。x86、ARM64、UWP、无 CRT target 和 Scoop 动态库输出不在本期；既有 native DLL 消费仍要支持。

## 2. 工具链由我们配套交付

### 2.1 组件职责

| 组件 | 用途 | 共享边界 |
| --- | --- | --- |
| 宿主 LLVM 22.1.8 library | `scoopc` 通过 inkwell 生成代码 | 每种宿主构建一份，与应用 CRT 无关 |
| 宿主 Clang／Clang++／clang-cl | generated-C、用户 native、runtime 和第三方 C/C++ | 同一宿主 LLVM 工具支持多个已登记环境 |
| 用户选择的 MSVC `cl.exe`／`link.exe` | MSVC native C/C++，以及需要 Microsoft linker 的库 | Windows MSVC 环境；不要求完整 Visual Studio IDE |
| LLD、llvm-ar／ranlib、llvm-readobj、llvm-rc／cvtres | PE 链接、归档、准备阶段检查和资源编译 | 同一宿主工具共用；按实际需要执行 |
| mingw-w64 headers、CRT startup、Win32／CRT import libraries | MinGW 环境的 Windows API、启动和 DLL 导入 | MSVCRT／UCRT 分别配置 |
| Windows SDK、MSVC toolset headers／libraries | Microsoft 环境的 C／C++、启动和系统导入 | 从已安装 Build Tools／SDK 或显式 SDK 根取得 |
| compiler-rt builtins、LLVM libunwind | LLVM 辅助调用、栈探测、Scoop SEH 展开 | 按 CRT 和 native 环境准备；不替代 MSVC C++ EH |
| libc++／libc++abi 或 MSVC STL／VCRuntime | 所选 C++ 环境的标准库与 ABI 支持 | 按实际 native C++ 需求链接，不能互换 |
| Scoop runtime、内置 `.slib`、第三方 native 库 | 语言运行与标准库 | 按完整 target 分别构建 |

系统已经有 `msvcrt.dll`／UCRT，并不代表构建机已经有 headers、startup objects、MSVC STL 和 import libraries。MinGW 开发资源随工具包配套；Microsoft 环境从所选 Build Tools／Windows SDK 取得对应开发资源。Windows 10+ 自带 UCRT，见 [Microsoft 的部署说明](https://learn.microsoft.com/en-us/cpp/windows/universal-crt-deployment?view=msvc-170)。VCRuntime／MSVC STL 的 DLL 不因此成为系统必有组件；按实际动态依赖部署，不能承诺所有 C++ 程序免装运行库。

MinGW 环境采用 [LLVM-MinGW 构建配方](https://github.com/mstorsjo/llvm-mingw/blob/master/README.md)，其[构建脚本](https://github.com/mstorsjo/llvm-mingw/blob/master/build-mingw-w64.sh)同时配置 headers 和 CRT。Microsoft 环境使用其 SDK／toolset 配套资源，不能以 mingw-w64 的 headers 代替 MSVC STL 的开发环境。仅更改最后的 `-lmsvcrt`／`-lucrt`，或只定义 `_UCRT`，不构成完整环境切换。

首个正式实现固定 LLVM 22.1.8，沿用仓库 backend 版本；发行准备同时记录验证过的 MSVC toolset／SDK 版本和 llvm-mingw／mingw-w64 revision。用户的受支持 toolset 通过普通编译／链接使用，不要求与发行准备工具路径或完整版本文本相同。上游构建配方与必要补丁沿用现有 vendor 管理；准备阶段先验证完整原生环境，再纳入发行脚本。

### 2.2 安装布局与选择

继续使用现有 Scoop sysroot；Windows 的 C 开发资源是其中按目标选择的 native sysroot，二者不是同一个目录：

```text
<install>/
  bin/                                      scoop、scoopc、scoop-link、scoop-lsp
  libexec/llvm/bin/                          宿主 Clang、LLD、LLVM 工具
  libexec/llvm/lib/                          宿主 LLVM 库
  libexec/llvm/lib/clang/<version>/include/   Clang resource headers
  lib/x86_64-pc-windows-msvc-ucrt/
    *.slib、runtime、各 native 组件
    toolchain/                              Microsoft 开发资源入口及 Scoop 辅助库
  lib/x86_64-w64-windows-llvm-mingw-msvcrt/
    *.slib、runtime、各 native 组件
    toolchain/                              MSVCRT 配套开发资源
  lib/x86_64-w64-windows-llvm-mingw-ucrt/
    *.slib、runtime、各 native 组件
    toolchain/                              LLVM-MinGW UCRT 配套开发资源
  include/                                  Scoop 公开 C 头文件
```

MinGW 的 `toolchain/` 保留上游 prefix 的目录关系；Microsoft 环境引用实际 SDK／VC include 和 library 根，并放置相应 Scoop／LLVM 辅助资源。Clang 内建头可共用，目标 runtime 搜索路径必须属于所选完整 target。一个安装根可同时提供上述三个目录，继续按 `<sysroot>/lib/<canonical-target-triple>` 唯一定位；不需要另选一套 C++ profile 才能解释同名产物，也不合并不同环境的 headers／archives。

先选完整 target，再解析对应工具。沿用 C compiler／native-sysroot 显式覆盖入口，补充实际 C++ compiler 和 final linker 的选择；覆盖只能提供该 target 相容的实现，不能把 MSVC target 的编译器换成 MinGW 后保持原 target 标记。generated-C bridge 默认使用随包 Clang 的相应模式，不能因用户 C++ compiler 是 `cl.exe` 就把其中 Clang builtin 原样交给 cl 编译。全部调用使用参数数组，并正确处理 MSVC 风格参数、依赖输出和路径。

MinGW 环境使用 GNU triple、compiler-rt、LLVM libunwind 和 `-stdlib=libc++`；Unicode startup 使用 `-municode`。Microsoft 环境使用 MSVC triple、MSVC headers／libraries，native C++ 默认 `/EHsc`、`/MD`，正常对象用 `lld-link` 或 `link.exe`；Unicode `wmain` 经对应 CRT startup 进入 runtime。不能把 MinGW driver 参数或启动库顺序移植到 MSVC 命令。Scoop 自身的 SEH 辅助库按对应环境构建，其选择不覆盖 native C++ EH runtime。

所选工具首次需要时，沿用现有工具探测：编译小型 header/object probe，检查机器、指针／`long` 宽度、实际 CRT／C++ headers 及符号引用；最终链接首次需要时才准备 startup／运行库。MSVC 开发环境可由显式 SDK／toolset 根或标准 Build Tools 环境定位。不能只凭可执行文件名、`libmsvcrt.a`／`msvcrt.lib` 文件名或 Clang triple 判断 CRT；不同环境中的同名库含义可能不同。

探测结果随既有正常输入更新记录复用。缓存命中先于源码读取、预处理、工具探测和链接准备；不逐次回读命中产物，不新增来源审计、生产凭据或工具授权层。library 编译不要求尚未使用的最终链接依赖。

### 2.3 宿主与交叉编译

| 构建／运行 Scoop 的宿主 | LLVM-MinGW 两个 CRT target | MSVC＋UCRT target |
| --- | --- | --- |
| 现有 macOS/AArch64 | 宿主 LLVM 工具＋对应 target 资源 | 宿主 clang-cl／LLD＋显式 Microsoft SDK／toolset 资源 |
| 现有 Linux/amd64 | 同上 | 同上 |
| Windows/amd64 | Windows LLVM 工具＋对应 target 资源 | 默认目标，native C++ 可选 cl／clang-cl |

先从现有宿主打通 Windows 交叉编译，在真实 Windows 执行验证；再提供 Windows 原生 Scoop 包。交叉编译不执行目标 `.exe` 作为必经配置步骤，第三方库用上游的 cross-build 配置。运行测试显式交给 Windows runner；Wine 可作开发辅助，不能替代 SEH、GC、线程和 I/O 的 Windows 验收。

Windows 版 Scoop 自身可沿用 UCRT 的 Rust `x86_64-pc-windows-gnullvm` 与相容的宿主 LLVM；这是宿主程序的构建选择，不改变它能生成的三个 Scoop target，也不改变 Windows 默认选择 MSVC＋UCRT。[Rust 官方平台说明](https://doc.rust-lang.org/rustc/platform-support/windows-gnullvm.html)明确 gnullvm 使用 UCRT／LLVM，并支持 native 与 cross 构建；发行脚本固定实际采用的 Rust 版本。

当前 `build_llvm.py` 只构建宿主 LLVM library，需要扩展为安装所需 Clang／LLD 工具；Windows 构建保留 MinGW 的 LLVM DLL 路径，不能把当前 Darwin rpath／Linux `.so` 规则照搬。LLVM 22.1.8 的 [llvm-shlib 构建文件](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/tools/llvm-shlib/CMakeLists.txt)已有 MinGW DLL 分支。Windows 启动时必需的宿主 DLL 放到实际引用它的 executable 相邻目录；不能等程序启动后再修补尚未满足的加载依赖。

维护脚本还需处理真实的宿主差异，例如当前 Rust 文本后端准备脚本的 `fcntl.flock`、POSIX archive 权限、程序后缀与文件替换；使用平台原有锁／文件 API 保留普通并发协调。Rust 主命令的取消／进程退出和外部子命令查找也须有 Windows 实现，不能仅增加 Cargo target 就宣布原生宿主完成。

维护机可使用上游构建所需的 shell／MSYS2、CMake、Ninja、Rust 等。MinGW 包配齐开发资源，用户不必另装 MSYS2／GCC；默认 UCRT Microsoft 环境需要 Build Tools 的 C++／SDK 组件或显式提供的等价开发资源，不要求完整 Visual Studio IDE。随包的 clang-cl 本身不包含 Microsoft SDK／STL。交叉编译可用 clang-cl 配合显式 SDK；原生 cl／link.exe 或特定 MSVC 工具流程交给 Windows 构建机，不承诺在 macOS／Linux 直接执行。

## 3. CRT、链接和 native 互操作

### 3.1 `native.cxx` 保持启用开关

`native.cxx` 继续为 Boolean：省略／`false` 表示当前 Cone 不声明 C++ 需求，`true` 表示需要所选 target 已确定的 C++ 环境。无需扩展成 `"msvc"`／`"libc++"` 选项；否则会产生 target 与另一选项各自选择 ABI 的重复来源。

```toml
[native]
cxx = true
```

MSVC target 对应 MSVC ABI／STL，两个 LLVM-MinGW target 对应 MinGW ABI／libc++。实际 `cl.exe`／`clang-cl`／`clang++` 路径在构建工具配置中选择，并检查满足 target 合同。Clang 的 [MSVC ABI 支持](https://clang.llvm.org/docs/MSVCCompatibility.html)和 [clang-cl 模式](https://clang.llvm.org/docs/UsersManual.html#clang-cl)说明 compiler 名称本身不能决定标准库。

跨目标 native 源码、include 和逻辑库继续通过现有 `when = { env = "..." }` 选择。`cxx_flags` 保持当前 Cone 的实际 driver 参数，不能覆盖 target／CRT／C++ ABI。MSVC 的 `/std:` 与 MinGW 的 `-std=` 等参数由各自编译器接受，不把编译器方言误当作可通用的文本。此设计不新增任意选项翻译或多维工具链配置语言。

只有使用 C ABI 的独立 DLL，不会因其内部用 C++ 编写就强制消费者开启 `cxx`；编译 C++ wrapper、调用 C++ 接口或需要补齐 C++ 链接运行库时才声明需求。当前 Cone 的 `false` 不取消依赖的 C++ 要求。现有 Darwin／Linux 的默认行为及 musl 的当前不支持范围保持。

### 3.2 依赖闭包和现成 MSVC 库

现有 `native_cxx: bool` 可以继续经 LIR foundation、slib reader 传播到 linker，因为完整 target 已经唯一决定所需 C++ 环境。最终链接先按既有规则确认全部 `.slib` target 相同，再合并是否需要 C++；不为此另加一套可与 target 矛盾的 C++ family 字段。实际 compiler／SDK／flags 的 producer 配置仍服务更新与必要的兼容检查。

MSVC 与 LLVM-MinGW 的 `.slib` 混用直接按 target mismatch 诊断，并指出依赖路径；源依赖可重新为目标构建，既有 native 对象不能改标签。裸 native `.lib`／`.a` 仍按正常对象、ABI 和实际依赖消费，不伪造 Scoop target metadata。独立 DLL 仅暴露 C ABI 时，其内部 C++ 运行库由 DLL 自己部署；这个正常 FFI 边界不等于允许跨 target 消费 Scoop 产物。

MSVC 模式必须实测两条正常路径：cl 编译 native wrapper＋MSVC `.lib`，clang-cl 编译 wrapper＋同一 `.lib`。wrapper 内可调用 C++ 类、STL、RTTI 和 native exception；对 Scoop 暴露既有 `extern "C"` 接口。不能把“支持 MSVC”缩减成只支持别人已经提供 C 导出的 DLL，也不因此新增 Scoop 的 C++ 语法／直接模板 FFI。

MSVC toolset 版本、`/MD`／`/MT`、debug CRT、`_ITERATOR_DEBUG_LEVEL` 和 LTO 格式都会影响组合。默认配套使用 release `/MD`，Scoop 的 debug profile 不自动改成 `/MDd`。`/MT` 或 debug native 库需要整套匹配的 native／runtime 配置，不能用 `/NODEFAULTLIB` 隐藏冲突后当作兼容；首期默认资源只承诺 `/MD`。Microsoft 要求同次链接的模块使用匹配的 runtime library 选项，见 [/MD、/MT 文档](https://learn.microsoft.com/en-us/cpp/build/reference/md-mt-ld-use-run-time-library?view=msvc-170)。

普通 MSVC COFF `.obj`／`.lib` 可用经验证的 lld-link 或 link.exe。MSVC `/GL`／`/LTCG` 不是普通 COFF 互操作承诺，须用匹配的 Microsoft 工具流程；首期不支持的输入明确诊断。不能要求所有 compiler 版本字符串相等来代替真实 ABI 规则，亦不能承诺任意新旧 MSVC 版本可混用；版本边界见 [Microsoft C++ 二进制兼容规则](https://learn.microsoft.com/en-us/cpp/porting/binary-compat-2015-2017?view=msvc-170)。

### 3.3 启动、运行库与边界

默认程序动态依赖所选系统 CRT。MinGW 使用配套 startup、compiler-rt 和 libc++ 环境；Microsoft 使用其 CRT startup、MSVC STL／VCRuntime 配套。Scoop runtime 与其自身展开辅助库按所选环境准备。辅助库静态链接不等于无 CRT，也不消除 Win32 或实际 MSVC runtime DLL 依赖。

generated-C、用户 native、runtime、startup 与内置库必须符合选定 native 环境。C++ 环境是配套配置的一部分，不能单改 C++ compiler 然后沿用另一环境的全部 C startup／headers。实际符合 C ABI 且不依赖不相容支持库的对象仍按正常链接规则使用，不仅按生产者名称拒绝。

MinGW 最终链接沿用 Clang driver；Microsoft 使用选定的 clang-cl／cl 链接驱动及 lld-link／link.exe。Scoop 添加实际对象、metadata 和所需库，不复制一套任意启动顺序；Unicode startup 之后才进入 Scoop coordinator，正常 native 初始化／终止由对应 CRT 负责。

现代 MSVC 的 `msvcrt.lib` 是搭配 UCRT／VCRuntime 的启动库名，不能据此认定它导入旧 `msvcrt.dll`，见 [Microsoft CRT 库对应关系](https://learn.microsoft.com/en-us/cpp/c-runtime-library/crt-library-features?view=msvc-170)。跨 DLL 或不相同运行库边界时，分配与释放留在原 owner，不共享未经约定的 `FILE*`、CRT fd、locale 或 C++ 对象。C++ 异常在 native 一侧处理，不能穿越 Scoop FFI；普通裸 native archive 不附加 Scoop 来源认证。

`captureErrno` 只捕获所选 CRT 的 `errno`，不能拿来取得 `GetLastError()`／`WSAGetLastError()`。平台 C wrapper 在对应 Windows API 返回后立即取得原始错误码，再进行其他调用；分别保留错误域，不把 Windows 错误伪装成 POSIX errno。

COFF 读取与链接增加 `.obj`／`.o`、普通 `.a`／`.lib` 和 import libraries；区分短 import object、普通成员与 `__imp_` 引用。按实际库类型解释文件，不仅按扩展名猜测。显式 roots、目标 sysroot、目标 provider 的既有顺序不变，不能回退到宿主库；DLL 运行期的加载位置属于正常部署合同。[LLD 的 Windows 支持](https://lld.llvm.org/windows_support.html)已经覆盖静态库、import library 和 PE 链接，不另写 PE linker。

## 4. 编译器与 GC／EH 的共同 Windows 实现

### 4.1 Win64 ABI

新增 Win64 分类器，与 SysV amd64 分开；三个 Windows target 复用共同规则，各自以配套 C 编译器互调验证。平台采用 LLP64：C `long` 为 32 bit、指针 64 bit；Scoop `Long` 仍是固定 `int64_t`。C bridge 用固定宽度声明和实际目标 headers，不把现有 LP64 假设带入，也不把共同 C ABI 推导成相同 C++ ABI。

C ABI 遵循 [Microsoft x64 calling convention](https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention)：前四个参数位置使用对应整数／浮点寄存器，caller 提供 32-byte shadow space，调用栈按 16 bytes 对齐；没有 SysV red zone。普通 C aggregate 的直接／间接分类、隐藏结果参数和变参按完整签名处理，以配套 Clang 互调验证。

Scoop ABI 使用相同 machine calling convention，独立确定语言值如何分解：scalar 保持直接；GC-free 小 aggregate 采用 Win64 分类；其他 aggregate 间接。interface 参数仍显式展开为 AS1 object 与 AS0 itab 两个标量参数；Windows interface 结果改用带精确扫描信息的间接 storage，不默认套用当前双 carrier 返回。非 ZST 间接结果的隐藏指针占据首个参数位置；Win64 的 sret 使用 RCX／RAX，现行规范的 RDI 条款只适用于 SysV。

间接参数仍是独立的按值 storage。LLVM 参数属性必须符合 Win64 实际 lowering，不能机械复制 SysV `byval` 属性；caller、callee、callback、runtime helper、Scoop ABI extern 和跨 Cone 消费同一份计划。修改这些规则前同步实现规范 2.4，并用 interface 返回期间移动 GC、混合参数和隐藏 sret 专测闭合。

### 4.2 COFF／PE 与元数据

在 `NativeObjectFormat`、codegen、slib 对象读取和 final-link 中增加 AMD64 COFF／PE 支持，包括实际使用的地址／相对地址 relocation、COMDAT、weak／external 符号、section 保留和 unwind 数据。COFF object 与最终 PE image 是两个读取边界，不能把 ELF reader 改后缀复用。

Scoop metadata 和 stackmaps 使用 COFF 可表达的 section 与明确边界符号；在 image 注册层通过 PE 映射范围取得实际地址。处理 section 合并、ASLR relocation、COMDAT 选择与丢弃，保留代码必须同时保留其 safepoint 和 EH 信息；禁止 ICF 把仍需区分身份的 callable 合并。只读取当前注册和链接需要的数据，不新增每次启动的产物生产复核。

`.pdata`／`.xdata` 为 SEH frame 信息，不等同于 `.eh_frame`。startup／anchor 汇编增加 Windows 寄存器、shadow space 和 `.seh_*` 描述，保留栈探测等 compiler-rt 引用。不能沿用当前 `x86_64_anchor.S` 的 SysV 寄存器和 ELF directives。

调试信息使用 Windows CodeView／PDB 路线：LLVM 对象与 Clang native 对象采用相容配置，由 LLD 产生最终 PDB；输出位置纳入既有构建结果。调试信息不代替 GC stackmap 或 SEH frame 表，release 的必要 GC／EH 信息不能随 debug 信息一起删除。

### 4.3 异常

Scoop 自身采用 LLVM libunwind 的 Windows SEH 后端，保留 exception class、强 GcHandle、catch／rethrow／cleanup 与 FFI 禁止异常穿越的合同。Windows personality 增加符合 SEH 调用签名的薄入口，经 `_GCC_specific_handler` 接入 Scoop 的 LSDA 处理；Scoop 类型匹配不使用 C++ RTTI。MSVC target 中，这个辅助库须用相应的 CRT／headers 配套构建，并与 MSVC machine triple 的 LLVM emission 一起验证。

该路线有 LLVM 22.1.8 的 [Unwind-seh.cpp](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/libunwind/src/Unwind-seh.cpp)作为依据，仍需最早用实际 Scoop personality＋`invoke`／`landingpad` 产物验证。不能把 Linux 的 `_Unwind_Exception` 私有布局、LSDA `0x9b` 编码或 landing-pad 寄存器假设直接冻结到 Windows；按实际 COFF emission 更新运行时规范 5.1～5.2 的目标条款。

Scoop 的 `_Unwind_*` 只绑定这一套实现，不再拉入重复定义这些接口的 GCC unwinder。MSVC C++ 的 exception handler 则由 VCRuntime 提供；它和 Scoop personality 都依托系统 SEH，但处理不同语言的异常，不能以“只允许一个 unwinder”为由删除 MSVC 运行库。native C++ 在自己的边界内捕获后返回，Scoop 异常也不穿越 native FFI；硬件 SEH 不转换为 Scoop Throwable。W0 必须验证两类 handler 同处一个 executable 的正确组合；有 lowering 缺口先解决，不能关闭异常后宣布 target 可用。

### 4.4 精确栈扫描

Windows frame decoder 按 SEH frame 信息重建被扫描 frame 的上下文，并用 LLVM stackmap 的真实 RSP／RBP location 定位根。优先使用系统 `RtlVirtualUnwind`，不新增私有 Windows unwinder。普通 stackmap 的寄存器编号仍按 LLVM 的编号解释，不能换成 Windows `CONTEXT` 字段序号。

当前 amd64 reader/runtime 使用 `FP = SP + stack_size - 8` 等 SysV frame 规则，Windows 必须有独立实现，不能只在 target match 中接受新名字。anchor 保存扫描所需的调用位置、栈与非易失寄存器上下文；NativeSafe 冻结、callback 重入、park、正常和异常恢复继续遵循既有 GC 协议。特别验证大栈帧、寄存器 spill、嵌套 native callback 与 unwind 途中的移动 GC。

## 5. Runtime 与内置库

### 5.1 Runtime 平台层

保留 C runtime，不要求改写为另一种语言。将实际需要的 pthread／POSIX 操作收束到现有 platform 层，Windows 使用线程／同步／TLS、`VirtualAlloc`／`VirtualFree`、栈范围查询和系统计时等 Win32 实现；不为三个 OS 建通用插件框架。两 CRT 共用 Win32 OS 层，CRT 差异集中在真实需要差异的 stdio、locale、数值解析、启动和终止处。

优先实现现有有限的 mutex、condition、once、thread join 与 TLS 生命周期合同，不让 runtime 为使用 pthread API 而强制依赖一个额外 DLL；第三方库确实需要的 winpthreads 可按其构建合同静态配套。锁等待仍执行现有 GC 协调，不能把 Windows wait 包成未经声明的 GCLeaf。

stdout／stderr 继续遵守既有字节输出合同，明确处理 CRT text mode 的换行转换。Unicode argv 从 UTF-16 startup 取得，转换为严格 UTF-8 后复用当前 managed 参数构造；不经过 ANSI code page。非配对 surrogate 走明确的启动参数解码失败路径，不能静默替换。`scoop.platform` 的原生路径需要另行保留完整 UTF-16，不能由严格 `String` 代替全部 OS 路径。

正常返回保留 CRT 标准终止，core `exit` 与失败立即退出保持“刷新后结束进程、不补做 managed finally／release”的现有语义。明确 Windows 的完整退出码表示，不照搬 POSIX signal／8-bit wait-status 解析。数值解析使用线程安全的独立 C locale，不修改进程全局 locale 来掩盖两 CRT 的函数差异。

### 5.2 平台库、asyncio 与 native 依赖

| 范围 | Windows 工作 |
| --- | --- |
| `scoop.core`／`scoop.std` 与语言测试 | 复用源码，先用完整 Windows runtime／GC／异常打通 |
| `scoop.platform` | Win32 文件、路径、目录、环境、进程、线程、时钟、熵、Winsock、console；分别建模 HANDLE、SOCKET 与 CRT fd |
| 路径与错误合同 | 原生路径可无损保存 UTF-16；明确盘符／UNC／根路径、权限差异，新增真实 Windows 错误域；先修订平台规范 |
| `scoop.asyncio` | 为现有 `PlatformEventLoop` 增加 IOCP／overlapped I/O 后端，非 overlapped 操作用已有 NativeWork 边界；保留真实完成前不得释放借用的规则 |
| GUI 接入 | 复用窗口设计已定义的 WindowsEvent 唤醒合同；I/O 服务与消息循环独立，不在本任务顺带实现窗口库 |
| OpenSSL、PCRE2、nghttp2、压缩、URL 等 | 用上游 Windows 配置按完整 target 构建静态库，普通应用消费预编译资源 |

当前 asyncio 已使用 Darwin libdispatch、Linux io_uring／liburing，libuv 已移除；Windows 设计不重新引入另一套 libuv 公共 EventLoop。IOCP completion、取消成功和实际缓冲区退休是不同事实；late completion、close、host 注销和移动 GC 按[已有后端设计](../asyncio-backends/DESIGN.md)共用规则。Windows event 与消息循环的接缝见[窗口设计第 4 节](../windowing/DESIGN.md#4-与-asyncio-的接入及-windows-唤醒接缝)。

### 5.3 Rust 文本后端是必须单独闭合的依赖

`sysroot/native/text_backend` 当前依赖 Rust `std`，不是可在两个 CRT 间任意复用的纯对象集合。保留现有 C API 和“由文本后端分配、由文本后端释放”的 owner 规则，继续 `panic = "abort"`，Rust panic 不进入 Scoop EH。

MSVC＋UCRT 从 Rust `x86_64-pc-windows-msvc` 构建，匹配默认动态 CRT 配置；LLVM-MinGW＋UCRT 从 `x86_64-pc-windows-gnullvm` 构建；LLVM-MinGW＋MSVCRT 从 `x86_64-pc-windows-gnu` 的匹配 MSVCRT 路线开始资格验证。最后一项不是已确认兼容 LLVM-MinGW 的承诺：Rust GNU target 默认 native 依赖包含 GCC 相关库，不能只更换 `CC` 就把这些默认项视为已解决。

准备阶段以实际 `staticlib` 的 native 依赖和最终程序为依据，验证 std、compiler builtins、import libraries 与本 target 运行库的组合。若固定 Rust 版本的预编译 GNU std 带入不可兼容的 GCC runtime，则从同版 Rust 源码构建匹配配置的 std／后端；这个依赖闭合必须在发布 MSVCRT 全部系统库之前完成，不临时混入另一 target 的文本库或重复 `_Unwind_*` 实现。是否需要这一维护构建，在 W0 用真实文本后端小程序确定并补充准确命令。

W0 已验证 Rust `1.99.1`、LLVM-MinGW `20260616`／LLVM `22.1.8`：三种 Rust target 的实际文本 staticlib 均可与对应 native 环境和 Scoop SEH 链接运行；GNU／MSVCRT 组合不需要重建 std，文本 archive 不定义第二套 `_Unwind_*`，最终程序未导入 libgcc DLL。复现命令见 [W0 Rust 文本实验](../../spikes/windows-targets/README.md#rust-text-dependency-closure)。此结果只适用于该固定组合，升级 Rust 或 native 配套时按正常维护验证重新确认。

## 6. 仓库落点与实施顺序

| 阶段 | 主要落点与交付 | 验收后才能声明的能力 |
| --- | --- | --- |
| W0 工具链与 ABI qualification | 固定 LLVM／MinGW 与 MSVC／SDK 验证环境；三个 target 的 C／C++、Rust 文本后端小程序；SEH／statepoint 最小实验 | 工具链路线和关键后端机制可行，不开放正式 target |
| W1 编译／链接闭环 | `compiler/codegen/target-data`、`abi-plan`、LIR target、COFF reader、`compiler/toolchain` registry／probe／final-link、`compiler/linker` | 三个完整 target 的 `.slib` 和 PE 链接；target 不匹配诊断 |
| W2 完整 core runtime | `runtime/src/platform`、线程／callback／初始化协调、SEH personality、启动／argv、精确 roots | 各 target 的 core／std、异常、线程、FFI、移动 GC 全链路 |
| W3 系统库 | platform Windows 实现、IOCP、Rust 文本后端和第三方 native 组件、对应规范 | Windows 内置库的公开行为和真实 consumer |
| W4 原生宿主与发行 | Rust Windows host、`.exe` 命令发现、进程／退出／路径、LLVM DLL、SDK 选择、维护和打包脚本 | Windows 宿主配齐对应开发资源后可构建；现有宿主可按 target 交叉编译 |

W0 先验证默认 MSVC＋UCRT 的 C++ 库消费及 Scoop SEH 组合，同时以 LLVM-MinGW＋UCRT 验证另一套 native 环境，再验证 MSVCRT 配套；不等完整移植后才检查环境差异。W1、W2 是同一最小可用编译器闭环的连续步骤，不把缺少异常或 GC 的半成品作为正式支持发布。系统库尚未移植时不把全部系统库标成 Windows 可用。

工具链实现沿用当前有限 target registry 和对应投影；新增 Windows 配置分支，不引入任意 target 文件、provider 插件或能力资格凭据。`scripts/build_llvm.py` 扩展宿主工具构建；增加职责单一的 Windows 目标资源准备脚本，并接入 `prepare_sysroot.py`、各真实 native 准备脚本和 `package_toolchain.py`。按现有格式把固定输入、版本、CPU／OS 基线与实际组件许可证写入发行资料。

三个新增 target identity 各自带版本。`native.cxx` 的布尔 manifest／产物表示不因本设计升级。增加 COFF／Windows 数据表示若确实改变 wire 格式，按既有格式升级规则处理；不因写下设计就提前修改 runtime ABI 或全局 schema，也不改变现有三个目标的缓存身份。

## 7. 必要验收

- **CRT 与发行**：按三个完整 target 编译 stdio、locale、errno、TLS、native constructor／destructor 程序；在显式验收中检查实际 imports。构建测试分别提供声明的 SDK／toolset；应用运行测试不依赖开发环境，只部署其实际需要的运行库，不把 UCRT 自带误当作 MSVC C++ DLL 自带。
- **MSVC C++**：分别用 cl、clang-cl 编译 C++ wrapper 消费真实 MSVC 静态 `.lib` 和 DLL import library，覆盖 STL、RTTI、内部异常和 static 初始化；Scoop 仅调用 C wrapper。验证缺 SDK、CRT linkage 不匹配和不支持的 LTCG 输入的正常诊断。
- **ABI 与产物**：Clang C 和 Scoop 双向互调，覆盖前四／第五参数、整数／浮点混合、各尺寸 aggregate、sret、interface 参数／结果、callback 和跨 Cone；验证 COFF／import library、保留与裁剪后的 metadata、ASLR 和异常表关联。
- **runtime／GC／EH**：完整 throw/catch/finally/rethrow、初始化失败、native callback、线程退出、park、TLS、多层栈与大栈帧；定向 GC stress 覆盖正常返回和 unwind 恢复，不把全量普通库测试放在 stress 模式。
- **平台与库**：Unicode 路径／argv、盘符和 UNC、标准流字节与重定向、进程退出、文件／SOCKET 生命周期；IOCP 取消及迟到完成、GUI host event 注销和 buffer 保活；每个 native 组件真实使用所选 CRT 构建并消费。
- **构建与缓存**：三个 target 在同一安装和同一项目内交替构建；特别覆盖两个 UCRT target 的产物不可混用。未变化输入复用，修改 native 输入正确更新，错误 CRT／工具链环境得到诊断；保持源码和工具存在，不以隐藏、删除它们验收缓存。
- **测试位置**：普通功能放所属 Cone 的 `scoop.test`；CLI、链接／ABI、产物和必须外部控制的进程行为用专门 fixture。两 CRT 在真实 Windows 执行；既有三目标运行与改动范围相称的回归。

最早需要实测的三个问题是 Win64 statepoint／SEH 与自定义 personality 的组合、精确根的 frame 几何、MSVCRT Rust 文本后端的 native 依赖闭合。这些是具体实现验证项；本文不把它们伪称为已经验证通过，也不以新增通用验证框架替代实验。
