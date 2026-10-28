# 开发 sysroot 与发行准备

这些脚本供维护仓库和制作发行包时手动运行，不属于 `scoop` 命令，也不由 driver、常规测试或 CI 自动调用。Python 脚本要求 Python 3.11 或更新版本；OpenSSL 解包使用 Python 3.12 或更新版本。

发行配置集中在 `release.toml`。当前 Darwin 基线为 macOS 27.0.1／SDK 27.0，Linux 构建环境为 Ubuntu 26.04.1、glibc 2.43／musl 1.2.5；只在相应环境完成验证后声明支持，不推定更老系统可用。LLVM 固定 22.1.8，CPU 使用 AArch64 generic／x86-64。工具构建时写入默认 Darwin 最低版本，普通命令的 `MACOSX_DEPLOYMENT_TARGET` 覆盖贯通 Scoop 对象、C bridge、native、runtime 与最终链接；预编译 runtime 的最低版本仍必须兼容。OpenSSL 准备使用同一设置。

五个命令（`scoop`、`scoopc`、`scoop-link`、`scoop-lsp`、`scoop-pm`）的 `--version` 显示相同发行版本与源码 revision。开发构建读取默认版本和当前 Git revision；发行构建显式设置 `SCOOP_RELEASE_VERSION` 与 `SCOOP_SOURCE_REVISION`，不在运行时读取 Git 或发行清单。

Windows 安装包将主驱动命名为 `sco.exe`，其版本信息使用 `sco`；其他可执行程序保持原名，macOS／Linux 仍用 `scoop`。Cargo package 和内部输出名继续为 `scoop`／`scoop.exe`，`package_toolchain.py` 组装时只重命名 Windows 主驱动。维护脚本的 `--scoop` 接受实际路径：开发构建可传 `.../release/scoop.exe`，安装包验证传 `.../bin/sco.exe`；外部命令仍以 `scoop-` 开头。

`build_llvm.py` 从 [固定的 LLVM 22.1.8 上游输入](../sysroot/vendor/llvm-project/22.1.8/README.md) 构建宿主动态库，仅启用 AArch64／X86 后端，并安装 Rust 构建所需的 llvm-config、头文件和 CMake 资料，以及 Clang／Clang++／clang-cl、LLD 和 LLVM 对象／归档／资源工具。关闭未使用的压缩、XML、编辑器、HTTP 和插件依赖，不移除 GC／EH 或 statepoint 后端。默认工作位置为 `tmp/llvm/<host>`，可用 `--source`、`--build-dir`、`--prefix` 选择其他位置。

Windows RC 构建使用 LLVM 22.1 的 `llvm-windres` 和 `llvm-cvtres`；前者是 LLVM RC 的 windres 入口，用于明确选择目标 Clang 预处理器并取得头文件依赖。默认从已选 Clang 的相邻目录查找，再检查所选 native prefix 的 `bin` 和 PATH，只有实际声明 RC 且需要更新时才调用。`build_llvm.py` 安装这两个工具，外部工具也必须可在宿主执行。

Windows x64 宿主使用 LLVM-MinGW `20260616` 的 UCRT 环境与支持 UTF-8 build file 的 Ninja（1.11+，`ninja -t wincodepage` 输出 UTF-8），以 `--cc`、`--cxx` 分别选择该 prefix 中的 `clang.exe`、`clang++.exe`。脚本安装 `.exe`、LLVM／Clang DLL 与 import library，并将其 C++／unwind DLL 放在同一 `bin`；Rust 编译使用 `x86_64-pc-windows-gnullvm` 与安装根的 `LLVM_SYS_221_PREFIX`。这些是宿主开发工具，与应用 target 的 Microsoft／MinGW、MSVCRT／UCRT 选择独立。首次完整构建包括 Clang／LLD；重复执行只在输入更新或所需输出缺失时构建。

发射 Windows target 的宿主 LLVM 必须包含 [statepoint 返回地址修正](../sysroot/vendor/llvm-project/22.1.8/patches/llvm-windows-statepoint-return-pc.patch)。`build_llvm.py` 在需要构建时通过 Git 将补丁应用到所选外部 source tree，已应用时保留原文件；未变化命中不读取补丁或源码内容。补丁将 stackmap 标签放在 WinEH NOP 之前，保留必要的 EH padding。更新宿主 LLVM 后重新构建配套 Scoop 命令和 Windows 系统库；不复用此前发射的错误 stackmap。

构建期间仅在子进程环境中将显式选择的 compiler 目录放到 PATH 前面，不修改用户全局环境。安装必须包含实际依赖的 `libclang-cpp.dll`，否则 PATH 上的 ESP32 等另一套 Clang DLL 可能被 Windows 加载并导致入口点错误。新构建完成后运行安装目录中的 llvm-config 与 Clang；命中缓存时不重复运行。需要排查当前会话的工具选择时，检查 PATH、`CC`／`CXX`、`CFLAGS`／`CXXFLAGS`、`CPATH`／`LIBRARY_PATH` 和 `LLVM_SYS_221_PREFIX`，不以清空全局配置代替正确安装依赖。

```sh
python3 scripts/build_llvm.py --jobs 6
python3 scripts/build_llvm_unwind.py --target x86_64-unknown-linux-gnu \
  --cc clang-22 --cxx clang++-22
python3 scripts/build_llvm_unwind.py --target x86_64-unknown-linux-musl \
  --cc clang-22 --cxx clang++-22 --musl-include /usr/include/x86_64-linux-musl
```

两个 LLVM 准备脚本在复制输入、补丁、工具探测和 CMake 之前检查输入 metadata 与所需输出；构建后从实际 Make depfile 或 Ninja 依赖记录保存头文件输入，更新时仍由 CMake 的普通增量构建处理。命中只检查保存的输入 metadata 和输出存在，不执行 Ninja 或读回 archive、对象或 LLVM 动态库。libunwind 的三个 Linux 链接模式可按 [M28 说明](../docs/milestone28/BUILDING.md) 单独验证。

Windows 使用 `build_windows_unwind.py --target <完整目标> --source <LLVM-22.1.8-source> --cc <目标-Clang> --build-dir E:/scoop-work/unwind/<target>` 准备静态 LLVM libunwind。Microsoft 在 x64 SDK／MSVC 开发环境中使用同目录的 clang-cl 和 `/MD`；两个 LLVM-MinGW 目标使用各自 CRT prefix 的 Clang。脚本在工作目录中复制所需上游源码，Microsoft 配置应用限定为 Clang x64／静态 Level-I SEH 的 CMake 补丁，并保留 C unwind 函数的异常标记，关闭库自身的 C++ 异常／RTTI。头文件与 canonical `lib/libunwind.a` 发布到 `sysroot/lib/<target>/unwind`，可用 `--prefix` 覆盖；普通编译直接使用这些资源。命中更新记录时不复制源码、启动 CMake／Ninja或检查 archive 内容。

先构建配套的 `scoop`、`scoopc`、`scoop-link`，并用以下维护脚本准备所需目标的 OpenSSL、nghttp2、IDNA／字符集、URL 和压缩后端。Linux 还需要按 `build_llvm_unwind.py --help` 准备同目标的 unwind。然后准备开发 sysroot，例如：

```sh
python3 scripts/build_nghttp2.py --target aarch64-apple-darwin
python3 scripts/build_text_backends.py --target aarch64-apple-darwin --build-dir ../scoop-work/text/aarch64-apple-darwin
python3 scripts/build_uriparser.py --target aarch64-apple-darwin --build-dir ../scoop-work/uriparser/aarch64-apple-darwin
python3 scripts/build_compression.py --target aarch64-apple-darwin --build-dir ../scoop-work/compression/aarch64-apple-darwin
python3 scripts/build_pcre2.py --target aarch64-apple-darwin --build-dir ../scoop-work/regex/pcre2-darwin
python3 scripts/prepare_sysroot.py \
  --target aarch64-apple-darwin \
  --scoop "$CARGO_TARGET_DIR/release/scoop" \
  --work-dir "$PWD/tmp/sysroot/aarch64-apple-darwin"
```

`--sysroot` 默认为仓库的 `sysroot`，`--runtime-root` 默认为仓库的 `runtime`；可显式选择 `--cache-dir`、`--scoopc` 和现有 native 工具参数。维护脚本默认将工作目录放在仓库已忽略的 `tmp` 下，避免占用 HOME 所在磁盘；也可显式指定其他磁盘。构建缓存、中间产物和辅助程序的 Rust target 目录保存在工作目录或显式 `CARGO_TARGET_DIR`，不要选择绝对路径 `/tmp`。目标输出为 `sysroot/lib/<triple>`，公开头文件为 `sysroot/include`。

脚本读取 `src/*/Cone.toml` 的生产依赖确定顺序，逐个执行普通 release library 构建；测试源码不进入发布库，各库的 lock 保存到工作目录的 `locks/`，不写入随包源码。`runtime_builder` 是独立于 Cargo 主 workspace 的脚本内部辅助程序，复用已有的 O2 runtime 编译与索引实现，不安装到发行包的 `bin`。准备过程不重新读取对象以核对摘要或复验产物；重复运行使用已有构建记录，输入发生变化时更新对应结果。runtime 与公开头文件先复制完整输出，再替换准备目录；复制失败保留原目录。

`build_nghttp2.py` 从 vendor 的固定 1.70.0 tag 归档构建纯静态协议库，保留完整上游核心，不构建应用、工具、测试或文档。使用同一目标部署／CPU 配置；工作目录默认为 `tmp/nghttp2/<triple>`，支持 `--build-dir`、`--prefix`、`--cc`。更新判断先于解包和 CMake，命中不读取或复验 archive。llhttp 9.4.3 的发布 C 源码由普通 HTTP Cone 编译，不另加工具或命令。三个 Windows target 使用 Ninja；Microsoft 在 x64 SDK／MSVC 开发环境中使用 clang-cl／cl 和 `/MD`，两个 LLVM-MinGW target 通过 `--cc` 指定其 CRT prefix 的 Clang。可用 `--build-dir E:/scoop-work/nghttp2/<target>` 将构建文件放在另一磁盘。

`build_openssl.py` 使用固定 OpenSSL 3.5.9 的上游构建。Windows 的 Microsoft target 在 x64 SDK／MSVC 开发环境中以 clang-cl／cl、原生 Windows Perl、NASM、NMake 构建 `VC-WIN64A`，使用动态 CRT `/MD`。两个 MinGW target 必须通过 `--cc` 选择各自 CRT prefix 的 Clang，并使用完整的 MSYS Perl、GNU Make 和 Unix shell 构建 `mingw64`；`--perl`、`--make`、`--shell` 可显式指定维护工具。`--jobs` 控制 GNU Make 并发，NMake 串行执行。构建目录可用 `--build-dir E:/scoop-work/openssl-msvc`；目标资源位于 `sysroot/lib/<triple>/openssl`，普通应用无需 Perl／Make／MSYS。输入未变化时在解包和工具调用之前复用；`--rebuild` 可显式重新准备。

`build_text_backends.py` 准备独立 Rust `scoop_text_backend`（字符集转换／IDNA／Punycode／NFC／Unicode 编辑基础操作）。Rust 依赖从固定 vendor 归档离线构建，需要支持所选目标的 Rust 工具链；普通应用不执行 Cargo。字符集按用户决定缩减为 43 个名称；原 GNU 文本构建链与发行资源已移除。必须显式给出仓库外 `--build-dir`；支持目标 `--cc`、资源目录 `--prefix` 与 `--jobs`。静态库放在 `sysroot/lib/<triple>`，头文件放在其 `include` 子目录，直接使用现有 native 搜索路径。实际 Rust 工具链的 `COPYRIGHT-library.html`、Apache-2.0／MIT 许可保存在目标目录的 `rust-std` 中，发行时收集到 `share/licenses`。这些文档必须随维护构建工具链提供；缓存命中不探测工具链或读取许可内容。固定依赖的源码与许可由 vendor 随工具链分发，不携带构建出的命令行程序。

Windows 文本后端使用 `release.toml` 固定的 Rust 1.99.1，先用 `rustup target add x86_64-pc-windows-msvc x86_64-pc-windows-gnu x86_64-pc-windows-gnullvm` 安装对应标准库。三个完整 Scoop target 分别选择 MSVC UCRT、GNU MSVCRT、GNU-LLVM UCRT 的 staticlib，统一发布为 `.a`；例如 `python scripts/build_text_backends.py --target x86_64-pc-windows-msvc-ucrt --build-dir E:/scoop-work/text-msvc`。构建只调用 Cargo／Rust，沿用参数 `--cc` 不会触发 C 编译器调用。实际 Windows 系统库由 charset／IDNA Cone 声明，CRT 和 Scoop 展开库仍由完整目标提供。

`build_uriparser.py` 以相同布局准备 uriparser 1.0.2，仅启用窄字符静态库，关闭工具、文档、测试及宽字符 API 的构建。需要 CMake 和所选目标 C 编译器；使用仓库外 `--build-dir`，其余目标、编译器与并行选项同上。三个 Windows target 使用 Ninja，Microsoft 在 x64 SDK／MSVC 开发环境中选择 clang-cl／cl 和 `/MD`，两个 MinGW target 显式选择各自 prefix 的 Clang。输入未变化时直接复用，实际更新后在维护工作区重建，再发布库与头文件。

`build_compression.py` 使用 CMake 依次准备 Zstandard 1.5.7、zlib-ng 2.3.3 和 Brotli 1.2.0；参数与目录布局同上。只发布静态库和头文件，不构建命令行程序、测试、benchmark 或并行压缩线程池；zlib-ng 保留原生命名前缀和运行时 CPU 检测。内存、同步与异步 API 共用同一个 `scoop.compression` Cone 和同一套后端。Windows 的目标编译器、Ninja、动态 CRT 与 E: 工作目录配置同 nghttp2；zstd 的 C++ 探测使用同一个编译器 prefix。zlib-ng 补丁在 Windows 通过 Git 应用，Unix 使用 patch。所有后端均先检查更新，命中时不启动工具；同一工作目录的并发准备使用已有文件锁串行协调。

`build_pcre2.py` 从固定 PCRE2 10.49 源码准备 `libpcre2-8.a` 和 `include/pcre2.h`，启用 Unicode 与 JIT，只构建 8-bit 静态引擎；不构建 POSIX wrapper、工具、上游测试或 benchmark。必须显式指定仓库外 `--build-dir`，目标和 C 编译器参数同上。输入未变化且输出存在时直接复用；PCRE2 和 SLJIT 许可随现有 vendor／发行流程收集。该 C 库独立于 Rust 文本后端。

PCRE2 的三个 Windows target 在 Windows 构建，使用原生 Ninja；Microsoft 环境以 `--cc` 选择 clang-cl 或 cl，并在所选 x64 SDK／MSVC 开发环境运行，使用 `/MD`。两个 MinGW target 分别传入对应 CRT 的 LLVM-MinGW `bin/clang.exe`，不能共用另一 CRT 的 prefix。构建目录可放到其他盘，例如 `--build-dir E:/scoop-work/pcre2-msvc`。PCRE2 的生成头和静态 archive 发布到完整 target 的 sysroot 目录，普通 Cone 不执行准备脚本。Windows 构建与脚本测试使用支持 UTF-8 build file 的 Ninja；可用 `ninja -t wincodepage` 确认。

`generate_public_suffix.py` 将 `sysroot/vendor/publicsuffix/public_suffix_list.dat` 的固定 PSL 规则、分区和版本注释生成普通 C 静态数据，供 HTTP Cone 打包。只有维护者更新快照时显式执行；脚本不下载数据，日常构建直接使用提交的 `.inc`。同步保留上游许可和同 revision 的测试向量，运行公共后缀的库测试及正式消费者；分发沿既有 vendor／license 收集路径携带这些资料。

开发 sysroot 准备完成后，设置 `SCOOP_SYSROOT="$PWD/sysroot"`，再运行相关的正式 CLI fixture 和库测试。Cargo 输出目录中的命令也遵守显式参数、环境变量、执行文件所属安装根的选择顺序，不再回退到编译时仓库。普通 build／run／test 和两个 link 命令默认使用目标 runtime 索引；`--runtime-root` 只用于明确需要源码构建的开发请求。准备 sysroot 不要求构建发行 LLVM、压缩归档或运行全套测试。

## 单一入口构建完整安装包

开发与发行 sysroot 均准备 `runtime`、`runtime-stress`、`runtime-startup-stats`、`runtime-stress-startup-stats` 四组目标 runtime，每组包含 `index.cbor`。默认构建使用 `runtime`，不编入 GC stress 或启动统计支持；`scoop --enable-gc-stress build/run/test/link` 选择 stress 配置，随后由 `SCOOP_GC_STRESS_MOVE` / `SCOOP_GC_STRESS_MINOR` 决定运行时是否开启。`--enable-startup-stats` 独立选择启动统计配置，编入后每次启动直接向 stderr 输出记录；未传时预处理完全排除计时和报告代码。两个开关可以组合，独立 `scoop-link` 同样支持。普通 fixtures 不传这些开关，相关专测显式选择。旧 schema 5 runtime 索引需要重新准备，`.slib` 和 runtime ABI 不变。

`package_toolchain.py` 是完整安装包的唯一入口：取得固定 LLVM 输入、按 lockfile 构建 VS Code 扩展、构建 LLVM、五个 release 宿主命令与独立的 scoop-example、准备该宿主支持的全部 native／slib／runtime 资源、为全部内置库生成 API 文档，随包复制 Scoop 编程 skill，最后生成工具包与 LLVM 源码包。无需事先逐个执行其他准备或扩展打包脚本。默认工作位置为 `tmp/package/<host>`，归档位于 `tmp/dist`；用 `--work-dir`、`--output-dir` 可选择外置盘目录。脚本不自动运行测试。

构建机需要 Python 3.12+、Git、Rust/Cargo、C/C++ 开发工具链、CMake、Make、Perl、Node.js 22.18+ 和 npm。npm 依赖、首次缺失的 LLVM 归档以及 Tree-sitter WASM 工具链需要联网取得；其余 native 上游输入已在 vendor 中。已有 WASI SDK／Binaryen 可按 [tooling 说明](../tooling/README.md) 选择。

```sh
python3 scripts/package_toolchain.py --jobs 6
```

上例用于 Darwin。Linux 还要提供 `--musl-include /usr/include/x86_64-linux-musl`，并安装 `musl-gcc`、`patchelf` 和适用于 LLVM libunwind 的 Clang／Clang++；Rust 工具链需要同时具有 `x86_64-unknown-linux-musl` 目标。可通过 `--gnu-cc`、`--musl-cc`、`--unwind-cc`、`--unwind-cxx` 明确选择工具。Linux 包同时包含 GNU 与 musl 目标资源。

musl 的 C 开发环境也需要 Linux UAPI 头。Debian／Ubuntu 的 `musl-gcc` 默认搜索目录可能缺少 `linux/mman.h`；此时可在工作目录准备仅包含 `linux`、`asm`、`asm-generic` 的头文件搜索根，以编译器包装脚本添加该根，并通过 `--musl-cc` 选择包装脚本。这些目录可指向系统已安装的对应 Linux 头；保留 musl 自己的 libc 头文件搜索顺序。

LLVM 源码与归档优先复用 vendor 的固定版本位置；缺失时在工作目录的 `inputs` 下下载官方固定版本归档并解包。也可传入已有的 `--llvm-source` 和 `--llvm-archive`，显式路径不存在时直接报错。已经准备的匹配版本宿主 LLVM 可通过 `--llvm-prefix` 复用；省略时脚本在自己的工作目录构建。`--version` 和 `--revision` 可显式设置发行标识，默认采用配置版本与当前 Git revision，存在未提交修改时标注 `+modified`。这些参数也写入安装根的 `VERSION`。

工具包 `scoop-<version>-<host>.tar.xz` 内直接是安装根内容，可直接解压到 `~/.scoop`，不需要 `--strip-components`。Darwin 包包含 Darwin/AArch64 目标，Linux/GNU amd64 包同时包含 GNU／musl 两套目标；每套均包含全部生产 Cone 的 release `.slib`、native 静态库／头文件和两种 runtime。LLVM 源码包单独保留有版本名称的顶层目录。

```text
~/.scoop/
  VERSION
  bin/                 scoop、scoopc、scoop-link、scoop-lsp、scoop-pm
  commands/            scoop-example 与其他外部子命令
  lib/<target-triple>/  *.slib、native 库／头文件、四种 runtime 配置
  include/             公开 Scoop C 头文件
  src/                 全部内置 Cone 源码
  vendor/              native 上游输入和补丁
  native/              Rust 文本后端维护源码
  libexec/llvm/lib/     宿主工具所需动态库
  docs/                离线 API 文档
  share/               示例主题、发行说明、组件清单及许可证
  tooling/             scoop-language-support-<extension-version>.vsix、scoop-programming/
```

`~/.scoop/bin/scoop` 默认使用相邻的 `~/.scoop/lib/<target-triple>/*.slib`，不因 debug profile 或空缓存重编系统库。显式 `--sysroot`／`SCOOP_SYSROOT` 和 source locator 仍按原有规则生效。安装及 VSIX 使用步骤见[随包说明](distribution/README.md)。

`sysroot/commands` 整目录进入安装包，保留程序的可执行权限；空目录也保留，`.gitkeep` 不打包。`scoop [--sysroot path] somecommand <args>` 直接执行所选安装根的 `commands/scoop-somecommand`，后续参数全部原样交给该程序。

`tooling/scoop-programming/` 原样进入 dist 工具包的同名目录，包含 `SKILL.md`、界面元数据和参考资料；`share/scoop/toolchain.toml` 的 `programming_skill` 字段记录其安装根相对路径。skill 引导 AI 使用配套示例、库文档和 CLI，不需要额外构建步骤，也不写入任何 AI 工具的技能目录或用户配置。内容与维护说明见 [tooling README](../tooling/README.md#scoop-编程-skill)。

脚本读取宿主命令的实际动态依赖，将所需非系统库复制到 `libexec/llvm/lib`，为实际引用这些私有库的 Darwin 文件设置相对加载路径。目标资源和许可说明按实际组件收集；归档在完整写入后才替换同名文件。安装方法、系统开发环境要求及升级说明随包放在 `share/scoop`。

文档使用本次配套 `scoop`／`scoopc`，显式选择 `sysroot/src/*/Cone.toml` 的全部生产库，以 `--no-deps --deny-doc-warnings` 为每个发行目标生成共同站点。嵌套测试／辅助 Cone 不进入发行清单。工作目录下的 `documentation/<target>`、`doc-target` 和普通 `cache` 保留更新结果；重复打包复用未变化的文档，随后复制到组装目录。站点更新记录只保留在工作目录，不放入发行包。所有目标的文档成功后才开始归档，文档失败保留上一次完整归档。

安装根的 `docs/index.html` 提供目标选择，具体站点位于 `docs/<canonical-target-triple>/`。Darwin 包包含 Darwin 站点；Linux 包包含 GNU／musl 两个站点。源码原文嵌入 HTML，搜索和资源均在本地；`.slib` 不保存完整源码。手工验收时检查解压后的目标及库清单、跨库链接、源码页、离线搜索与重复生成的复用情况；详细范围见 [M60 设计](../docs/milestone60/DESIGN.md)。

手工验证发布包时，将归档解压到新的目录，可使用带空格路径；保留源码、原工具和原产物。运行五个命令的版本查询，检查实际加载的私有 LLVM，再用空缓存完成含 native／泛型／GC 的应用、crypto／TLS、普通库测试和两个 link 入口，重复构建检查复用。Linux 还应覆盖 GNU 与 musl 的既有链接模式。常规 fixture 使用已准备的开发 sysroot，不执行打包或解压步骤。具体结果记录在 [M52 实施记录](../docs/milestone52/PROGRESS.md)。

打包脚本的归档布局与失败保留检查使用 `python3 -m unittest discover -s scripts/tests`；运行前对改动的 Python 脚本执行与 fixture runner 相同版本的 Ruff 格式化及 lint。完整包验收还应确认 `VERSION` 与命令版本一致、VSIX 内包含扩展代码及两个 WASM，并使用解压目录中的工具完成冷构建／运行及再次复用。

## 独立示例命令

`python3 scripts/build_scoop_example.py` 仅构建 `tooling/scoop-example` 并将命令安装到开发 sysroot，不调用编译器。它使用独立 Cargo lockfile；新增示例与正文位于 `sysroot/share/scoop/examples`，已有库示例保持原文件。可用 `--sysroot` 安装到另一份配套工具链，用 `--target-dir` 指定构建缓存。完整打包入口自动完成这一步，并复制所有主题材料。内容维护和独立测试见[阅读器说明](../tooling/scoop-example/README.md)。
