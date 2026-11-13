# 发行组件精简与 Windows 工具资源拆分设计

日期：2026-11-09。

状态：实现与三平台打包已完成。宿主 bin／LLVM 合并、运行资源筛选、归档内别名共享、目标库精简和 Windows native 拆分均已接入。本轮已验证范围及 Windows 收尾时保留的限制见[验收记录](VALIDATION.md)。

依据：[实现规范](../specs/SCOOP-IMPL-SPEC.md) 1、2.7、2.8、2.22，[Windows targets 设计](../windows-targets/DESIGN.md)，以及 [AGENTS.md](../../AGENTS.md) 的范围、缓存与验证约束。语言语义、对象模型和 GC 契约继续遵守[语言规范](../specs/SCOOP-SPEC.md)与[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)。

## 1. 目标与边界

将 Scoop 宿主命令及其必需的 LLVM／LLDB 运行资源作为一个可安装组件交付；目标库包只交付应用消费的链接产物及必要元数据；Windows 的宿主工具、公用 SDK 头文件和目标专属 CRT／C++ 资源分别归属明确的位置。

本轮作出以下决策：

1. 合并 `bin-<host>` 与 `llvm-<host>`，保留 `bin-<host>` 名称，停止生成独立的 `llvm-<host>` 包。
2. LLVM 的完整开发 SDK 留在维护构建 prefix；发行阶段按工具运行和应用编译的实际需要选取文件。
3. `lib-base-<target>` 与 `lib-gui-<target>` 不再携带第三方库开发头、源码或完整 native 工具链。保留 `.slib`、对象、链接库、runtime 索引及许可证。
4. 两个 LLVM-MinGW 目标增加独立的 `native-<target>` 包，保存各自的目标开发资源。公用宿主工具和公共 MinGW 头文件随 Windows `bin` 包提供一次。
5. Windows 相同宿主二进制需要多个加载位置时，在同一 `bin` 归档内使用硬链接共享内容，保留实际需要的文件名和加载路径。

实现使用现有发行入口、目标目录、工具发现、native 调用和归档机制。不新增安装器、包管理协议、通用资源去重服务或逐文件发布凭据。普通构建不读取发行清单来验证文件内容，缓存命中仍只按输入更新和结果存在与否复用。

本轮不改变 `.slib` 格式、runtime ABI、Windows 三种 native 环境、GUI 依赖划分或支持的宿主／目标集合。现有的源码依赖、外部 C/C++ 工具、显式 native sysroot 和 runtime 源码构建继续使用各自正常输入；精简发行包不删除维护 sysroot 的构建资源。

## 2. 参考产物与问题来源

参考目录为 `/Volumes/N/homes/chenxu/scoop-dist/2026-11-08`。盘点覆盖 16 个组件包：全部 Windows 二进制组件，以及 macOS AArch64、Linux GNU 的 bin、LLVM、base 和 GUI 包。macOS bin 中记录的版本为 `0.0.0-bundle.20261108`，源码 revision 为 `ca56c85d40f51f05fbf6377e91e95ffde45d0dae`。这是历史样例，不替代实施后的候选包验证。

下表均为解压后逻辑文件大小，单位为 MiB；不能直接相加作为压缩包的下载节省量。

| 位置 | 样例结果 | 原因 |
| --- | --- | --- |
| 每个宿主 LLVM 包的 `libexec/llvm/include` | 3,217 个文件，约 64.4 MiB | `assemble_llvm` 复制完整开发 prefix |
| macOS／Windows GUI 包的 include | 每份约 22.5 MiB | GUI 组装显式复制 Skia 头文件树 |
| Linux GUI 包的 include | 约 29.9 MiB | 复制 Skia、GTK、tray 等开发头 |
| MinGW MSVCRT／UCRT base 包的 `toolchain` | 约 431.8／492.4 MiB | 整体复制 LLVM-MinGW prefix，只过滤部分其他架构目录 |
| MinGW UCRT 包的 `lib/clang/22/lib/linux` | 约 59.6 MiB | 上游输入中的其他平台 compiler-rt 被直接带入 |
| Windows LLVM 包内相同二进制内容 | 多余副本合计约 526.0 MiB | DLL 按每个 executable／Python 模块目录反复复制 |
| Windows bin 与 LLVM 包的同路径 DLL | 约 87.3 MiB | 两包都提供 `bin/libLLVM-22.dll`、`libc++.dll`、`libunwind.dll` |

Windows LLVM 包内，`libLLVM-22.dll` 的 5 份内容相同，每份约 85.1 MiB；`libclang-cpp.dll` 的 3 份内容相同，每份约 68.0 MiB。Python DLL、LLDB library／`_lldb.pyd` 等也有可共享的内容。文件内容相同不表示可以删除所需的加载名称，实施采用第 7 节的别名处理。

两个 MinGW prefix 的 4,333 个 include 文件中，仅 `include/_mingw.h` 不同。这一统计包含 MinGW SDK、Clang resource headers 和附带 Python 的开发头；其中 MinGW 的 `include` 本身有 3,750 个文件、约 103.4 MiB。三类资源分别处理：MinGW 公共头共享，Clang 头使用选定宿主 LLVM 的配套版本，附带 Python 开发头不进入发行包。

原始清单和内容比对结果保存在仓库外，不将逐文件报告或样例归档提交到文档目录。

## 3. 组件组成与安装组合

归档名称继续使用 `scoop-<version>-<component>.tar.xz`。

| component | 维度 | 内容与依赖 |
| --- | --- | --- |
| `bin-<host>` | 每宿主 | Scoop 命令、外部命令、所需 LLVM 工具及动态库、LLDB／Python、编译器资源、tooling、安装入口、发行说明和对应许可 |
| `lib-base-<target>` | 每目标 | 非 GUI 系统 `.slib`、native 链接库、四种 runtime 配置各一份 `.a`／`.lib` 与 `index.cbor`、对应许可 |
| `lib-gui-<target>` | 有 GUI 的目标 | GUI `.slib`、Skia 及其他 GUI 链接输入、对应许可；依赖同目标 base |
| `lib-profiling-<target>` | 每目标，可选 | 全部系统 Cone 的 Profiling/full 与 Release/full 两套 `.slib`、位置映射、profiling runtime 及 compiler-rt profile archive；依赖同目标 base/native，GUI native 输入复用 gui |
| `native-<target>` | 两个 MinGW 目标 | CRT 配置头、启动对象、Windows／CRT 导入库、C++ 目标库及必要 compiler-rt；通过同版本 Windows bin 取得公共工具和头文件 |
| `sources` | 全局 | 内置 Cone 源码、清单、自有 C／C++／Rust 胶水、资源、公开 Scoop FFI 头和示例 |
| `vendor` | 全局 | 全部上游源码、说明、构建补丁与许可，含 LLVM／LLDB；按需安装 |

`bin`、`lib-base`、`lib-gui`、`lib-profiling`、`native`、`sources` 和 `vendor` 归档顶层都是安装根内容，合并解压后可直接使用。`vendor` 保持维护归档用途，不通过安装它补足运行工具的资源。

profiling 组件单独下载、安装及移除，保存到 `lib/<target>/profiling/`，不改变普通构建的库选择。宿主工具包提供配套 `llvm-profdata`；compiler-rt 只为实际目标构建 profile runtime，随该目标 profiling 组件发布。完整采集与反馈合同见 [profiling 设计](../profiling/DESIGN.md)。

默认安装组合为宿主 `bin`、全局 `sources` 和默认目标 `lib-base`；需要 GUI 时追加同目标 `lib-gui`。Windows 默认 MSVC UCRT 仍由本机 Build Tools／Windows SDK 提供 native SDK，不新增空的 `native-msvc` 包。选择任一 MinGW 目标时，同时安装该目标的 base 与 native；追加 GUI 不重新安装 native 资源。

普通应用继续消费预编译系统库。post-install 只为已安装目标生成文档；本轮不取消源码、示例和离线文档的现有分发方式，也不使 post-install 构建缺失资源。

目标 `.slib`、runtime 与 native 机器库仍不依赖生产宿主路径。官方宿主／目标组合沿用现有 `targets.toml`；本轮不扩展将 Windows SDK 组件直接安装到其他宿主的发行组合。已有跨宿主编译继续使用可在该宿主运行的工具和显式提供的完整外部 SDK，不依赖 Windows executable。

## 4. 宿主工具包

### 4.1 文件选择

`build_llvm.py` 继续生成构建 Scoop 和 LLDB 所需的开发 SDK，包括头文件、`llvm-config`、CMake exports、SDK 静态库和后端测试使用的 `llc`。发行组装不再递归复制整个 prefix，而使用有限的工具与资源清单。

LLVM 源码包保存构建输入；prefix 中的生成头、CMake exports 和机器库不必搬入源码包以维持一份预构建 SDK。需要开发 SDK 时由维护构建生成。

| 资源 | 发行处理 |
| --- | --- |
| Scoop 五个宿主命令、`scoop-example`、`scoop-bundle` | 保留当前公开名称、命令目录和版本信息 |
| Clang／clang++／clang-cl、LLD 各实际入口 | 保留供已有 native 编译和链接流程使用的入口及其正常别名 |
| LLVM 对象、归档和资源工具 | 保留 `llvm-ar`／ranlib、readobj、nm、objcopy、strip、symbolizer、dsymutil；Windows 保留 rc／windres／cvtres |
| LLDB／lldb-dap、argdumper、平台 debug server | 保留现有调试功能使用的程序、Python 模块和 formatter |
| LLVM／Clang／LLDB 动态库和传递依赖 | 从实际保留的 executable、DLL 和 Python 扩展取得，复制非系统依赖 |
| `lib/clang/<major>/include` | 保留选定 Clang 的内建头和配套 include 文件；它们是应用编译及调试表达式解析的输入 |
| LLVM／Clang SDK include、CMake exports、SDK 静态库和 import libraries | 不进入发行包，继续留在开发 prefix |
| `llvm-config`、`llvm-tblgen`、`llc` | 不进入普通工具包；开发构建和编译器正确性测试继续使用维护 SDK |

公开 LLVM／LLDB 命令仍使用 `scoop-` 前缀，私有工具保留上游名称。保持现有工具选择覆盖能力，不能因包合并改变 `--cc`、`--cxx`、`--linker` 的优先级。

### 4.2 组装顺序

先将所有宿主程序、外部命令、LLDB 和 Python 原生模块放入一个 bin 组装根，再收集实际依赖。依赖根包含保留的私有 LLVM 工具，不能仅从 `scoopc` 的依赖推导整个工具包内容。

Darwin 继续处理相对 rpath、dylib install name 和必要签名；Linux 继续处理每层私有依赖的相对 RUNPATH 及配套 ELF interpreter。当前 `prefix_libraries` 对完整 prefix 的假定随组装调整，不能使未处理的库保留构建机加载路径。

原始 SDK 文件不在发行加工时修改。文件先复制到本次组装根，完成加载路径调整和签名，再建立发行所需的别名。

## 5. 目标库包与必要头文件

base／GUI 包按现有 Cone 划分和具体 native 库清单复制链接产物，停止复制目标下的 OpenSSL、nghttp2、文本、URL、压缩、PCRE2、Skia、GTK 和 tray 开发头文件。

“只保留链接产物”有以下必要组成，不使用仅允许三种扩展名的统一过滤器：

- `.slib`、普通对象、静态库及目标平台实际使用的导入库。
- 四种已支持 runtime 配置各一份 `index.cbor` 和它引用的 `runtime.a`／`runtime.lib`，保持索引内相对布局；不交付独立 runtime `.o`。
- Linux GUI 当前使用的 GTK／libadwaita／Cairo／tray 等 `.so` 链接输入。程序仍使用系统桌面运行库，本轮不改变其动态链接合同。
- 各已交付组件的许可证与版权通知，继续放在 `share/licenses`。

移除发行头文件不改变开发 sysroot：维护准备仍生成完整 native 资源，再构建系统 `.slib`。从源码重建这些内置库的开发者继续运行相应准备脚本；不承诺只解压精简后的 lib 包即可重建第三方后端。

### 5.1 Unwind 头与 archive 分别选择

现有 Linux／Windows final-link probe 编译包含 `unwind.h` 的程序，Windows final-link 还检查 `<unwind-prefix>/include/unwind.h`。因此必须同步拆开链接 archive 与 probe include 的定位。

选定 LLVM libunwind 的公开头及其所需配套头在 bin 包中保存一份，位置为 `libexec/scoop/include/unwind/`；由发行入口已经取得的 LLVM 源码准备，不能用另一套系统 unwind 声明替代。各目标的 archive 仍是 `lib/<target>/unwind/lib/libunwind.a`，属于 base 包。

默认 final-link 分别使用宿主工具包的共享头目录和所选 Scoop sysroot 的目标 archive。显式 `--unwind-prefix` 继续使用该 prefix 自己的头和库；源码 runtime 构建继续使用已有的维护资源定位。开发环境需要头文件时沿现有准备布局取得，不要求先制作发行归档。

解析结果保存实际使用的 include 和 archive 位置，并进入现有调用与更新记录。链接探测的机器、CRT、unwind 和实际链接输入检查保留，未变化时复用；本轮不把它扩展成对全部发行库的扫描。

## 6. Windows 公共资源与目标资源

### 6.1 安装布局

下面的 `<target>` 为任一完整 MinGW target，`22` 为当前固定 LLVM 主版本：

```text
<install>/
  bin/                                      Scoop 与公开的 scoop-* 工具
  libexec/llvm/bin/                          配套 Clang、LLD、LLVM 工具
  libexec/llvm/lib/clang/22/include/          配套 Clang 内建头
  libexec/lldb/                             LLDB、helper 和配套 Python
  libexec/scoop/include/unwind/              共享 libunwind 公开头
  libexec/mingw/include/                     公共 MinGW C／Win32 头
    c++/v1/                                 公共 libc++ 头
  lib/<target>/
    *.slib                                  base／GUI 包
    runtime*/                               base 包
    unwind/lib/libunwind.a                   base 包的 Scoop unwind provider
    toolchain/                              native 包，无 executable
      include/_mingw.h                      该 CRT 的实际配置头
      include/c++/v1/__config_site           该目标的 libc++ 构建配置
      x86_64-w64-mingw32/lib/                CRT startup、导入库及 C++ 目标库
      lib/clang/22/lib/windows/              所需的目标 compiler-rt
```

`toolchain` 继续作为目标 SDK 数据目录名，其含义不再包括驱动程序。`bin` 包拥有 `libexec/mingw`；native 包拥有相应目标的 `toolchain`。base／GUI 不再复制这两部分，也不通过跨归档硬链接取得它们。

因此只使用 MSVC 的 Windows 安装也会获得一份公共 MinGW 头文件。本轮将公共资源合并进 bin，不额外增加独立的 common SDK 包；体积记录同时列出默认 MSVC 和追加 MinGW 的实际安装组合。

### 6.2 公共头与目标配置

公共 C／Win32 和 libc++ 头来自本次固定 LLVM-MinGW 版本。`_mingw.h` 按真实 CRT 分别提供；`__config_site` 虽然在样例中相同，仍是 C++ 构建配置，随目标保存。公共 include 中不再放置这两个配置文件，避免引号 include 优先命中公共目录中的另一份配置。

组装通过明确的上游目录和配置文件选择完成，不在编译器中加入逐文件合并层。公共头是否可以共用在维护准备或相关测试中检查一次；升级 LLVM-MinGW 时若出现新的实际配置差异，先调整相应文件归属。不能在日常编译时比较两套完整头文件，也不能无条件采用 UCRT 配置服务 MSVCRT。

上游 prefix 附带的 Python、Python 开发头、BusyBox、clangd、clang-tidy、LLDB 等不进入 native 包。Clang resource headers 使用宿主 LLVM 的一份配套资源，不再复制 MinGW prefix 中的第二套。其他机器与其他 OS 的 compiler-rt、不属于当前功能的 sanitizer／fuzzer／profiling 工具资源不因位于同一 prefix 而发行。

### 6.3 目标机器资源

各 native 包保留本目标正常 C/C++ 应用所需的 CRT 启动对象、Windows／CRT 导入库、libc++／libc++abi 及实际需要的支持库。保留 Windows API 的正常 SDK 库集合，不能按测试恰好引用的几个库裁剪开发能力。SDK 构建说明、CMake／libtool 开发资料和未使用的 C++ module 构建资料不随这些链接输入复制。

compiler-rt 按当前所需功能与目标机器选择，包括 `libclang_rt.builtins-x86_64.a`；不能将整个 `lib/clang` 作为目标库复制。Scoop 最终链接继续显式选择本目标的 Scoop unwind provider，不再为这条链接路径复制 MinGW prefix 内另一套未使用的 unwind archive。

需要分发的目标 DLL 只依据已经支持的实际动态链接需求选择；不能将宿主 UCRT 的 DLL 放进 MSVCRT native 包，也不能将目标 DLL 纳入宿主 DLL 的共享集合。当前 MinGW 最终链接静态选择的 C++ 支持库不因此改为动态库。

### 6.4 编译、资源编译与最终链接

默认 generated-C、native C/C++、RC 预处理和最终链接使用同一组已解析资源：宿主 driver／linker、目标 SDK 根、公共 MinGW include、目标配置 include、Clang 内建 include、compiler-rt 目录和目标库搜索路径。

宿主工具及其公共 include 从实际执行文件所属安装根定位；所选 Scoop `--sysroot` 决定目标库及默认 `lib/<target>/toolchain`。两者分别保存实际位置，不从某个 target 的目录反向寻找宿主 executable，也不把开发机绝对路径写进目标包。

头文件顺序必须使 Clang 的内建头按原合同生效，并使目标配置优先于公共 SDK 内容。C++ 标准头和 `__config_site` 使用显式目录，不能让宿主 driver 猜出另一套 libc++ 配置。RC 预处理消费相同的公共／目标 include，保留用户 manifest 的 include 规则。

最终链接改用宿主 Clang／Clang++ 与其配套 LLD，不再从 `<native-sysroot>/bin` 找 driver。`--sysroot` 选择目标 CRT；编译器运行库也必须显式指向该目标的 compiler-rt。仅改 driver 路径而保留 Clang 默认 resource 搜索不够：当前宿主 LLVM 的构建不包含 MinGW target builtins。

实现可用 Clang 的 `-resource-dir <target-sdk>/lib/clang/22` 选择目标运行库，并显式提供宿主 Clang 的内建 include；C++ 与 CRT 目录沿同一选择加入调用。`-rtlib=compiler-rt`、`-unwindlib=none`、`-stdlib=libc++`、Unicode startup 及 MSVCRT／UCRT 的现有差异继续落实到实际命令。编译、C++、RC 与最终链接不得各自推导不同的资源根。

显式选择的完整外部 MinGW prefix 继续使用自身 SDK 与 compiler-rt，不混入随包的公共 SDK 头；Microsoft target 继续使用其 Build Tools／SDK 发现或 `msvc.toml`。这些输入都归入现有 Windows native 调用记录，不新增通用 SDK 插件协议。

新增路径和参数作为普通工具调用的更新输入。若调用记录结构发生变化，同步演进相应本机记录或请求协议版本；不为安装目录调整改变 `.slib` 或 runtime ABI，也不在命中时重新扫描 SDK。

## 7. Windows 宿主二进制共享

合并后的 bin 归档同时包含 Scoop 命令、私有 LLVM 工具、LLDB 和 Python，因此依赖收集可以在一个组装根内复用已选文件。

对一个已选非系统 DLL，首次需要时复制内容，后续实际需要同一 DLL 的目录建立硬链接。保持 executable 相邻 DLL 和 Python 扩展需要的加载位置，使绝对路径启动 Scoop、LLDB launcher 及子工具不额外依赖用户 PATH。Windows 基线系统 DLL／API-set 仍不复制。

`liblldb.dll` 与 `_lldb.pyd`、LLDB argdumper、Clang 工具别名和 LLDB 的 Clang 头目录按实际来源处理。一个原生文件确实承担多个名称时共享同一内容；若所选 LLDB 构建产生独立的 Python 扩展，则分别保留，不能假定所有 `_lldb.pyd` 都等于 `liblldb.dll`。

依赖发现沿用当前 imports 读取和已选来源映射。不同来源的同名 DLL 继续执行现有冲突判断，确认实际相同后方可共享；不按文件名合并不同版本，不对所有安装内容建立通用内容寻址仓库。

硬链接仅在本次组装根中建立，原 SDK／Python 安装始终先复制。tar 中首个成员保存文件数据，后续别名引用同一归档内已经出现的成员。LLDB 共享头目录在 Windows 上用普通目录及逐文件硬链接表达，不依赖需要额外权限的目录符号链接。POSIX 现有的相对符号链接继续保留。

Windows 原生 `tar` 解压与 Python 使用的解包方式均纳入验收。硬链接建立失败按实际归档／文件系统错误报告，不静默退回数百 MiB 的重复副本。用户复制完整安装目录后即使复制工具将硬链接展开，文件仍须正常工作；这只影响复制后的空间占用。

## 8. 发行组装、许可与安装

`package_toolchain.py` 继续是唯一入口。流程为准备维护输入与目标产物，组装完整 bin、sources、vendor 和各目标 lib／native，收集许可，再生成本次组件归档。源码拆分及共享下载目录见 [PACKAGING-DESIGN.md](PACKAGING-DESIGN.md)。`--prepared-sysroot` 继续消费维护者已准备的完整 sysroot；精简后的发行目录不自动成为 native 后端的维护输入。

每个文件由所属组件提供。不同组件可以创建相同父目录，但不覆盖同一个实际文件；尤其 LLVM DLL 不再同时出现在两个归档中，目标 `toolchain` 不再由 base 与 native 重复提供。许可随实际组件收集：宿主 LLVM／LLDB／Python 和公共 SDK 许可随 bin，CRT／C++／compiler-rt 许可随 native，系统库及第三方后端许可随相应 lib。不能在移除源码或开发文件时遗漏其已发行机器库的版权通知。

归档继续采用暂存写入和成功后替换。一次发布只上传本次明确生成的组件列表；重复组装同一版本时，停止发行的旧 `llvm-<host>` 文件不得因目录 glob 被再次上传或纳入下载索引。退役附件与远端索引的清理属于显式发布步骤，不在普通构建中执行。

同一次安装的组件取自同一发行，按所选 target 与可选 GUI／vendor 组合安装。独立的 `scoop-install` 将各版本放入 `toolchains/<version>-<host>`，升级时保留所选组件与旧版本，保留共享的用户包缓存及自定义命令；构建缓存按工具链发行身份区分。不保留旧包名别名或两套发行模式。

post-install 继续按已安装 base／GUI 选择文档来源。缺少 MinGW native 资源时，由实际使用该资源的工具发现报告目标及所需组件；不通过新建逐文件安装验收清单让普通命令先完整扫描目录。

官网仓库维护 bootstrap，下载对应宿主的 `scoop-install` 并转交安装参数；安装器按 [manifest](MANIFEST.md) 读取组件关系，确保 native 包进入相应 Windows 组合，退役 LLVM 包不再被列出。安装页、手动安装示例和安装测试采用相同入口。安装完成只需将管理目录的公开 `bin` 加入 PATH。

## 9. 代码改动位置

| 位置 | 改动 |
| --- | --- |
| [package_toolchain.py](../../scripts/package_toolchain.py) | 合并宿主组装根和归档；按清单选择 LLVM 运行资源；精简 base／GUI；输出 native 包与本次归档列表 |
| [package_windows.py](../../scripts/package_windows.py) | 将整 prefix 复制拆为公共 MinGW 头和目标 SDK 产物选择 |
| [package_native.py](../../scripts/package_native.py) | 覆盖全部保留的 native 程序；沿实际依赖共享宿主库；处理合并根与相对加载路径 |
| [package_debugger.py](../../scripts/package_debugger.py)、[package_debugger_windows.py](../../scripts/package_debugger_windows.py) | 保留 LLDB／Python 所需文件、别名与资源关系，消除重复内容 |
| [windows_c.rs](../../compiler/toolchain/src/windows_c.rs) 及 [environment.rs](../../compiler/toolchain/src/windows_c/environment.rs) | 分别解析宿主工具、公共头、目标配置和 compiler-rt，并保存实际调用输入 |
| [windows_native.rs](../../compiler/toolchain/src/windows_native.rs)、[windows_resource.rs](../../compiler/toolchain/src/windows_resource.rs) | C/C++ 与 RC 使用同一组资源及 include 次序 |
| [Windows final-link](../../compiler/toolchain/src/final_link/windows.rs) 及其 [command](../../compiler/toolchain/src/final_link/windows/command.rs) | 使用宿主 driver／linker，明确 CRT 与 compiler-rt 选择 |
| [unwind.rs](../../compiler/toolchain/src/unwind.rs)、[Windows probe](../../compiler/toolchain/src/final_link/windows/probe.rs)、[Linux probe](../../compiler/toolchain/src/final_link/linux/probe.rs) | 分离共享 probe 头与目标 unwind archive；保留显式外部 prefix |
| [package_licenses.py](../../scripts/package_licenses.py) | 按新的组件归属收集已发行内容的许可 |
| [release.yml](../../.github/workflows/release.yml) | 更新安装／上传组件集合，移除安装后对 llvm-config 的要求，增加 Windows 目标组合验收 |
| [打包测试](../../scripts/tests/test_package_toolchain.py)及 native／debugger／post-install 测试 | 删除要求开发文件或完整 prefix 的旧断言，验证新组成与真实消费 |

官网同步涉及独立仓库 `scoop-lang.org` 的 `tools/publish_release_index.sh`、`install.sh`、`src/pages/docs/install.html` 和 `tools/tests/test_install.py`。本仓库设计不直接修改或发布该站点。

## 10. 规范同步与实施顺序

1. **同步合同。** 修订实现规范第 1 节的宿主／Windows 资源布局、2.7 的组件与发布合同、2.8 的必要链接资源和 unwind 选择、2.22 的 LLDB 分发。同步 Windows 与调试设计中仍描述旧布局的段落，以及 scripts README、随包 README／release notes。语言与 runtime 规范未改变的语义不重复改写。
2. **完成宿主包合并。** 保持维护 SDK 完整，完成运行文件选择、统一依赖收集、别名和 Windows 硬链接，验证宿主命令及 LLDB 可用。
3. **完成库包精简。** 移除开发头并接入共享 unwind 头，验证 release 应用、库消费和 post-install，不改变四种 runtime 的选择。
4. **完成 Windows native 拆分。** 组装公共／目标资源，统一 C、C++、RC 与链接选择，在三种 Windows 环境验证真实程序。
5. **完成候选发行验收。** 更新 CI、组件上传集合、安装文档及官网索引流程，重新生成候选包，记录体积与功能结果。

每批 Rust／Python 代码修改按仓库要求先格式化和 lint，再运行相关测试。文档、测试和调用方围绕最终单一布局演进，不以临时兼容分支作为完成状态。

## 11. 验收

### 11.1 组件与归档

在打包专测中检查新组件集合、文件归属和必要别名。使用小型输入覆盖组装选择，候选发行再检查真实归档；这些检查不成为运行时或缓存命中的工作。

- 不生成独立的 `llvm-<host>` 包；合并 bin 不含 LLVM SDK include、CMake exports、SDK 静态库和仅用于维护的工具。
- base／GUI 不含第三方开发头或 target 工具链；runtime index、归档、必要 `.so` 和许可均保留。
- MinGW native 包没有 executable、Python、BusyBox、其他架构／OS 的运行库；公共头只由 bin 提供一次，目标配置与库各自对应 CRT。
- 相同宿主 DLL 和实际别名在 bin tar 中只保存一份数据；所有硬链接目标均位于同一归档，解包后链接关系与文件内容可用。
- 保留真实输入变化、普通缺失输入、失败不发布及已有并发协调覆盖；不删除、移走或隐藏源码、工具或原产物来证明后续阶段独立。

### 11.2 实际安装与功能

| 范围 | 代表性验证 |
| --- | --- |
| 三个发行宿主 | 在各平台解压；绝对路径运行 Scoop 各命令和内置外部命令；代表性安装覆盖带空格路径及目录复制，检查实际加载与调用路径。系统 tar 的中文路径行为不作为本轮打包验收项 |
| base 单独安装 | release 的真实系统库 consumer、运行和测试；空应用缓存直接消费预编译库；post-install 生成非 GUI 文档 |
| 追加 GUI | GUI 程序编译、链接和可用环境中的运行；追加 GUI 文档；Linux 保留系统桌面依赖 |
| 三种 Windows native 环境 | generated-C、native C/C++、C++ 标准库与原生异常、Unicode startup、RC 图标／版本资源、已支持的 DLL 链接与加载 |
| CRT 与机器选择 | 通过真实 header／object／link probe 和 PE imports 验证所选 CRT，native 配置缺失或选择错误给出正常诊断 |
| Unwind | Linux 两目标与 Windows 三目标的正常链接、Scoop 异常及栈诊断；显式外部 unwind prefix 继续可用 |
| 调试 | release 加 full 调试信息，生成对应 dSYM／debug／PDB；LLDB／DAP 启动、断点、formatter 和 Python 模块导入，Windows 原生 tar 解包后重复覆盖 |
| 多目标组合 | 同时安装两个 MinGW SDK 后交替构建，确认共享头不会使 CRT、C++ 配置或 builtins 串用；沿用 Darwin 两目标与 Linux GNU／musl 的发行组合 |
| 更新复用 | 目标／公共头或实际工具发生变化时更新相关结果，未变化时复用；布局变化不在热路径引入全量目录或内容检查 |

普通行为使用既有 Cone 测试；归档组成、安装路径、工具加载、PE／调试产物等用打包测试和必要的 CLI／ABI fixture。普通验收使用 release，调试合同通过显式 debug-info 选项覆盖；GC stress 只在确有 GC 正确性需求的既有专测中运行。

### 11.3 体积记录与完成条件

记录每个新组件的压缩体积、tar 内实际文件数据总量、文件／别名数量，以及 Windows 解压后的共享情况。比较时按相同宿主和目标安装组合统计，避免将包拆分造成的转移当作减少。

526.0 MiB 等历史重复量只用于确定工作优先级，不作为任意压缩比例门槛。保留必要 `.slib` 和 native 库，不为追求数字修改产物格式、改变链接语义或裁掉已支持功能。

上述文件归属、实际安装功能和发布组件集合全部闭合后方可标记完成。验收文档只保存范围、概要结果、已修复失败与真实平台限制；详细日志、逐文件清单和归档放在仓库外。
