# 开发 sysroot 与发行准备

维护脚本与 fixture runner 共用 [`compiler/target-data/targets.toml`](../compiler/target-data/targets.toml) 的目标和宿主事实；目录读取不运行编译器。第三方配置名仍由各自 recipe 按机器／native 环境选择，实际使用的配置进入普通更新记录。

这些脚本供维护仓库和制作发行包时手动运行，不属于 `scoop` 命令，也不由 driver、常规测试或 CI 自动调用。Python 脚本要求 Python 3.11 或更新版本；OpenSSL 解包使用 Python 3.12 或更新版本。

多个 target 可以并行准备到同一个 sysroot。公共 runtime 头文件共用 sysroot 内的更新记录，并通过已有文件锁协调发布；各 target 的 Cone、runtime 产物与工作目录仍独立。

发行配置集中在 `release.toml`。当前 Darwin 基线为 macOS 27.0.1／SDK 27.0，Linux 构建环境为 Ubuntu 26.04.1、glibc 2.43／musl 1.2.5；只在相应环境完成验证后声明支持，不推定更老系统可用。LLVM 固定 22.1.8，CPU 使用 AArch64 generic／x86-64。工具构建时写入默认 Darwin 最低版本，普通命令的 `MACOSX_DEPLOYMENT_TARGET` 覆盖贯通 Scoop 对象、C bridge、native、runtime 与最终链接；预编译 runtime 的最低版本仍必须兼容。OpenSSL 准备使用同一设置。

宿主 Rust 命令保留 rustc 对应目标的默认 CPU 基线，Darwin/AArch64 使用 Apple Silicon 平台特性，Linux/amd64 使用 x86-64；Scoop backend 的 generic CPU 设置不传给宿主 Cargo 构建，以免移除 Apple 平台依赖所需的 AES／SHA2 等特性。

OpenSSL 的 Darwin/AArch64 构建也保留 Apple Clang 的平台特性，不传 `-mcpu=generic`；x86-64 仍显式使用对应 CPU 基线。Windows 打包在 MSVC 开发环境中运行，Rust build script／proc macro 的 linker 使用 `VCToolsInstallDir` 下的 x64 `link.exe`，避免 Git／MSYS 的同名工具。Linux 收集共享库时保留已有 `LD_LIBRARY_PATH`，同时优先查找选定 LLVM 的库；随包 Python 等依赖仍复制进发行包并改写为相对加载路径。

五个命令（`scoop`、`scoopc`、`scoop-link`、`scoop-lsp`、`scoop-pm`）的 `--version` 显示相同发行版本与源码 revision。开发构建读取默认版本和当前 Git revision；发行构建显式设置 `SCOOP_RELEASE_VERSION` 与 `SCOOP_SOURCE_REVISION`，不在运行时读取 Git 或发行清单。

Windows 安装包将主驱动命名为 `sco.exe`，其版本信息使用 `sco`；其他可执行程序保持原名，macOS／Linux 仍用 `scoop`。Cargo package 和内部输出名继续为 `scoop`／`scoop.exe`，`package_toolchain.py` 组装时只重命名 Windows 主驱动。维护脚本的 `--scoop` 接受实际路径：开发构建可传 `.../release/scoop.exe`，安装包验证传 `.../bin/sco.exe`；外部命令仍以 `scoop-` 开头。

`build_llvm.py` 从 [固定的 LLVM 22.1.8 上游输入](../sysroot/vendor/llvm-project/22.1.8/README.md) 构建宿主 LLVM／Clang 动态库，仅启用 AArch64／X86 后端，并安装 Rust／LLDB 构建所需的 llvm-config、头文件、Clang resources 和 CMake 资料，以及 Clang／Clang++／clang-cl、LLD、LLVM 归档／资源工具和 dsymutil、llvm-objcopy、llvm-strip、llvm-symbolizer、llvm-nm、llvm-readobj。安装的 llc 供后端正确性 fixture 使用，确保测试与编译器使用同一套 LLVM 补丁。版本信息使用固定的上游版本，不附加源码解包目录上层仓库的 Git revision。关闭未使用的压缩、XML、编辑器、HTTP 和插件依赖，不移除 GC／EH 或 statepoint 后端。默认工作位置为 `tmp/llvm/<host>`，可用 `--source`、`--build-dir`、`--prefix` 选择其他位置。

Windows RC 构建使用 LLVM 22.1 的 `llvm-windres` 和 `llvm-cvtres`；前者是 LLVM RC 的 windres 入口，用于明确选择目标 Clang 预处理器并取得头文件依赖。默认从已选 Clang 的相邻目录查找，再检查所选 native prefix 的 `bin` 和 PATH，只有实际声明 RC 且需要更新时才调用。`build_llvm.py` 安装这两个工具，外部工具也必须可在宿主执行。

在 Darwin／Linux 开发树中使用此 SDK 时，将 `LLVM_SYS_221_PREFIX` 指向安装根，并为 Cargo 设置同一目录的动态库搜索路径，例如 `RUSTFLAGS="-C link-arg=-Wl,-rpath,${LLVM_SYS_221_PREFIX}/lib"`。发行脚本已使用相同配置；仅设置编译期 prefix 不能保证运行时加载该 LLVM。更新 LLVM 后重新构建 Scoop 命令并准备系统库，fixture 的 `--llc` 也选择此 SDK 的工具。

Darwin 的 `scoop` 与 `scoop-link` 最终链接使用已打包的 `libexec/llvm/bin/lld -flavor darwin`，包含 C++ 依赖时同样直接调用 LLD；无需新增 `ld64.lld` 别名。开发树没有随包 LLD 时，依次使用 `LLVM_SYS_221_PREFIX/bin/lld` 或 PATH 中的 `lld`，也可通过 `--linker /path/to/lld` 显式选择。Apple Clang 继续承担 native 编译及 SDK 选择；最终链接不回退到系统 ld。更换链接器不消除 compact unwind 的 personality 数量限制。

Linux／macOS 交叉编译 Microsoft target 使用配套宿主 Clang 和显式 `msvc.toml` 开发资源，布局见 [Windows 设计](../docs/windows-targets/DESIGN.md#22-安装布局与选择)。大小写敏感文件系统需要 SDK 文件可按源码的实际 include 拼写和链接库名定位；可在维护准备时为 `Windows.h`／`windows.h` 等差异补齐符号链接。Windows 上准备的 runtime schema 7 索引及相邻对象可直接传到其他宿主，配套 `.slib` 无需重编。

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

先构建配套的 `scoop`、`scoopc`、`scoop-link` 和依赖解析使用的 `scoop-pm`，并用以下维护脚本准备所需目标的 OpenSSL、nghttp2、IDNA／字符集、URL 和压缩后端。ABI 或产物格式更新时，这些命令须使用同一份当前源码构建；仅更新编译器会使旧 package manager 拒绝新产物。Linux 还需要按 `build_llvm_unwind.py --help` 准备同目标的 unwind。然后准备开发 sysroot，例如：

```sh
python3 scripts/build_nghttp2.py --target aarch64-apple-darwin
python3 scripts/build_text_backends.py --target aarch64-apple-darwin --build-dir ../scoop-work/text/aarch64-apple-darwin
python3 scripts/build_uriparser.py --target aarch64-apple-darwin --build-dir ../scoop-work/uriparser/aarch64-apple-darwin
python3 scripts/build_skia.py --target aarch64-apple-darwin --build-dir ../scoop-work/skia/aarch64-apple-darwin --gn /path/to/gn
python3 scripts/build_compression.py --target aarch64-apple-darwin --build-dir ../scoop-work/compression/aarch64-apple-darwin
python3 scripts/build_pcre2.py --target aarch64-apple-darwin --build-dir ../scoop-work/regex/pcre2-darwin
python3 scripts/prepare_sysroot.py \
  --target aarch64-apple-darwin \
  --scoop "$CARGO_TARGET_DIR/release/scoop" \
  --work-dir "$PWD/tmp/sysroot/aarch64-apple-darwin"
```

Windows graphics／GUI 使用相同的 `build_skia.py` 入口。对 Microsoft UCRT、LLVM-MinGW UCRT 和 LLVM-MinGW MSVCRT 分别指定完整 `--target`、独立 `--build-dir`、`--gn` 与对应 `--cc`；Microsoft 使用 clang-cl 及当前 x64 SDK／MSVC 开发环境，toolset 与 SDK 版本取自该环境的 `VCToolsVersion`／`WindowsSDKVersion`，不在发行配置中固定，MinGW 使用所选 CRT prefix 的 clang。输出为 Ganesh D3D12、DirectWrite、段落和内嵌 ICU 数据的静态资源，不需要应用分发旁置 ICU 数据文件。完整 Windows 打包会按同一配置准备并收集这三套 Skia 资源。

Linux GNU 的 GUI 使用 GTK 4／libadwaita，支持 Wayland 和 X11；窗口标题栏、圆角和阴影由 GTK／libadwaita 绘制，内容仍使用 Skia Ganesh GLES，字体使用 Fontconfig／FreeType。需要 GTK 4.18 以上、libadwaita 1.4 以上，以及 EGL、GLES、Fontconfig、FreeType、zlib、X11 和 xkbcommon 开发包。Skia 构建还需要已有的 GN、Ninja、Clang／Clang++、llvm-ar 和 pkg-config。先准备 GTK 开发资源，再使用固定 vendor 源码构建 Skia：

```sh
python3 scripts/prepare_gtk.py --build-dir ../scoop-work/gtk/x86_64-unknown-linux-gnu
python3 scripts/build_skia.py --target x86_64-unknown-linux-gnu \
  --build-dir ../scoop-work/skia/x86_64-unknown-linux-gnu --gn /path/to/gn \
  --cc /path/to/llvm/bin/clang --cxx /path/to/llvm/bin/clang++
```

`prepare_gtk.py --development-prefix /path/to/usr` 可选择单独解压的开发包目录；其 pkg-config 路径优先于系统目录，缺少的系统头文件和运行库仍从本机开发环境取得。脚本将头文件和七个链接输入（包括原生输入区域使用的 Cairo）放入目标 sysroot，输入未变化时直接复用。完整打包使用相同入口，`package_toolchain.py --gtk-prefix /path/to/usr` 传递该选择；GNU GUI 组件包含链接输入及许可，程序运行时使用系统安装的 GTK/libadwaita。musl 目标不包含 GUI。

独立的 `scoop.tray` 通过 StatusNotifierItem／DBusMenu 提供 Linux 托盘，额外使用发行版维护的 libdbusmenu-glib 0.4 开发包（Debian／Ubuntu：`libdbusmenu-glib-dev`），不依赖 GTK3。单独执行 `python3 scripts/prepare_tray.py --build-dir ../scoop-work/tray/x86_64-unknown-linux-gnu`；`--development-prefix /path/to/usr` 同样支持解压的开发包。它只准备 `include/tray`、libdbusmenu 链接输入和许可，不改变 `scoop.window` 的依赖。完整打包由 `package_toolchain.py --tray-prefix /path/to/usr` 选择来源。消费托盘的程序运行时需要系统 `libdbusmenu-glib.so.4` 和支持该协议的桌面宿主；未使用托盘的 window／UI 应用无需此额外运行库。注册成功不保证宿主显示了图标，Linux 不提供原生托盘矩形查询。

随后按普通 `prepare_sysroot.py` 流程构建 GUI Cones。Linux 完整打包使用 `--llvm-prefix` 的 Clang 准备 GNU Skia，发行只收集静态库，头文件留在维护 sysroot；运行时动态使用以上系统图形库，musl 目标不包含 GUI。显示后端由 `SCOOP_WINDOW_BACKEND=wayland`／`x11` 选择，默认优先环境中的 Wayland。测试和运行限制见 [Linux GUI 记录](../docs/linux-gui/PROGRESS.md)。

`--sysroot` 默认为仓库的 `sysroot`，`--runtime-root` 默认为仓库的 `runtime`；可显式选择 `--cache-dir`、`--scoopc` 和现有 native 工具参数。维护脚本默认将工作目录放在仓库已忽略的 `tmp` 下，避免占用 HOME 所在磁盘；也可显式指定其他磁盘。构建缓存、中间产物和辅助程序的 Rust target 目录保存在工作目录或显式 `CARGO_TARGET_DIR`，不要选择绝对路径 `/tmp`。目标输出为 `sysroot/lib/<triple>`，公开头文件为 `sysroot/include`。

脚本读取 `src/*/Cone.toml` 的生产依赖确定顺序，逐个执行普通 release library 构建；测试源码不进入发布库，各库的 lock 保存到工作目录的 `locks/`，不写入随包源码。`runtime_builder` 是独立于 Cargo 主 workspace 的脚本内部辅助程序，复用已有的 O2 runtime 编译与索引实现，不安装到发行包的 `bin`。每个目标的四种 runtime 配置各输出 `index.cbor` 和一份 `runtime.a`（Mach-O／ELF）或 `runtime.lib`（COFF）；C 编译的独立对象只留在临时目录，缓存与安装包不保留 `objects/*.o`。准备过程不重新读取归档以核对摘要或复验产物；重复运行使用已有构建记录，输入发生变化时更新对应结果。runtime 与公开头文件先复制完整输出，再替换准备目录；复制失败保留原目录。

`--runtime-only` 只准备公开头文件和四种 runtime 配置，不选择或构建系统 Cone，也不要求 `--scoop`。发行打包现在构建并打包系统 `.slib`，不再使用这个选项；它仅用于只需要 runtime 与头文件的开发场景。

`build_nghttp2.py` 从 vendor 的固定 1.70.0 tag 归档构建纯静态协议库，保留完整上游核心，不构建应用、工具、测试或文档。使用同一目标部署／CPU 配置；工作目录默认为 `tmp/nghttp2/<triple>`，支持 `--build-dir`、`--prefix`、`--cc`。更新判断先于解包和 CMake，命中不读取或复验 archive。llhttp 9.4.3 的发布 C 源码由普通 HTTP Cone 编译，不另加工具或命令。三个 Windows target 使用 Ninja；Microsoft 在 x64 SDK／MSVC 开发环境中使用 clang-cl／cl 和 `/MD`，两个 LLVM-MinGW target 通过 `--cc` 指定其 CRT prefix 的 Clang。可用 `--build-dir E:/scoop-work/nghttp2/<target>` 将构建文件放在另一磁盘。

`build_openssl.py` 使用固定 OpenSSL 3.5.9 的上游构建。Windows 的 Microsoft target 在 x64 SDK／MSVC 开发环境中以 clang-cl／cl、原生 Windows Perl、NASM、NMake 构建 `VC-WIN64A`，使用动态 CRT `/MD`。两个 MinGW target 必须通过 `--cc` 选择各自 CRT prefix 的 Clang，并使用完整的 MSYS Perl、GNU Make 和 Unix shell 构建 `mingw64`；`--perl`、`--make`、`--shell` 可显式指定维护工具。`--jobs` 控制 GNU Make 并发，NMake 串行执行。构建目录可用 `--build-dir E:/scoop-work/openssl-msvc`；目标资源位于 `sysroot/lib/<triple>/openssl`，普通应用无需 Perl／Make／MSYS。输入未变化时在解包和工具调用之前复用；`--rebuild` 可显式重新准备。

`build_text_backends.py` 准备独立 Rust `scoop_text_backend`（字符集转换／IDNA／Punycode／NFC／Unicode 编辑基础操作）。Rust 依赖从固定 vendor 归档离线构建，需要支持所选目标的 Rust 工具链；普通应用不执行 Cargo。字符集按用户决定缩减为 43 个名称；原 GNU 文本构建链与发行资源已移除。必须显式给出仓库外 `--build-dir`；支持目标 `--cc`、资源目录 `--prefix` 与 `--jobs`。静态库放在 `sysroot/lib/<triple>`，头文件放在其 `include` 子目录，直接使用现有 native 搜索路径。实际 Rust 工具链的 `COPYRIGHT-library.html`、Apache-2.0／MIT 许可保存在目标目录的 `rust-std` 中，发行时收集到 `share/licenses`。这些文档必须随维护构建工具链提供；缓存命中不探测工具链或读取许可内容。固定依赖的源码与许可由 vendor 随工具链分发，不携带构建出的命令行程序。

Windows 文本后端使用 `release.toml` 固定的 Rust 1.99.1，先用 `rustup target add x86_64-pc-windows-msvc x86_64-pc-windows-gnu x86_64-pc-windows-gnullvm` 安装对应标准库。三个完整 Scoop target 分别选择 MSVC UCRT、GNU MSVCRT、GNU-LLVM UCRT 的 staticlib，统一发布为 `.a`；例如 `python scripts/build_text_backends.py --target x86_64-pc-windows-msvc-ucrt --build-dir E:/scoop-work/text-msvc`。构建只调用 Cargo／Rust，沿用参数 `--cc` 不会触发 C 编译器调用。实际 Windows 系统库由 charset／IDNA Cone 声明，CRT 和 Scoop 展开库仍由完整目标提供。

`build_uriparser.py` 以相同布局准备 uriparser 1.0.2，仅启用窄字符静态库，关闭工具、文档、测试及宽字符 API 的构建。需要 CMake 和所选目标 C 编译器；使用仓库外 `--build-dir`，其余目标、编译器与并行选项同上。三个 Windows target 使用 Ninja，Microsoft 在 x64 SDK／MSVC 开发环境中选择 clang-cl／cl 和 `/MD`，两个 MinGW target 显式选择各自 prefix 的 Clang。输入未变化时直接复用，实际更新后在维护工作区重建，再发布库与头文件。

`build_compression.py` 使用 CMake 依次准备 Zstandard 1.5.7、zlib-ng 2.3.3 和 Brotli 1.2.0；参数与目录布局同上。只发布静态库和头文件，不构建命令行程序、测试、benchmark 或并行压缩线程池；zlib-ng 保留原生命名前缀和运行时 CPU 检测。内存、同步与异步 API 共用同一个 `scoop.compression` Cone 和同一套后端。Windows 的目标编译器、Ninja、动态 CRT 与 E: 工作目录配置同 nghttp2；zstd 的 C++ 探测使用同一个编译器 prefix。zlib-ng 补丁在 Windows 通过 Git 应用，Unix 使用 patch。所有后端均先检查更新，命中时不启动工具；同一工作目录的并发准备使用已有文件锁串行协调。

`build_pcre2.py` 从固定 PCRE2 10.49 源码准备 `libpcre2-8.a` 和 `include/pcre2.h`，启用 Unicode 与 JIT，只构建 8-bit 静态引擎；不构建 POSIX wrapper、工具、上游测试或 benchmark。必须显式指定仓库外 `--build-dir`，目标和 C 编译器参数同上。输入未变化且输出存在时直接复用；PCRE2 和 SLJIT 许可随现有 vendor／发行流程收集。该 C 库独立于 Rust 文本后端。

`build_skia.py` 为两个 Darwin 目标准备 `libscoop_skia.a` 和 `include/skia`，使用固定 Skia 及配套 HarfBuzz、ICU、PNG／JPEG 依赖。需要 GN、Ninja、Xcode 和 Apple Clang；`--gn` 选择已有 GN，`--sdk` 可指定 Xcode SDK 名称，默认 macosx。CPU 与最低部署系统来自发行配置；工作目录和目标资源按其他 native 准备脚本的方式显式选择。当前开发验证使用 Xcode macOS 27.2 SDK、最低部署 27.0.1。源码、选项及必要部署补丁见 [vendor 说明](../sysroot/vendor/skia/README.md)。普通应用只消费预编译 Cone 和资源；完整 Darwin 发行准备同样接受 `package_toolchain.py --gn ...`，发行包含所需静态库及许可，头文件保留在维护 sysroot。

PCRE2 的三个 Windows target 在 Windows 构建，使用原生 Ninja；Microsoft 环境以 `--cc` 选择 clang-cl 或 cl，并在所选 x64 SDK／MSVC 开发环境运行，使用 `/MD`。两个 MinGW target 分别传入对应 CRT 的 LLVM-MinGW `bin/clang.exe`，不能共用另一 CRT 的 prefix。构建目录可放到其他盘，例如 `--build-dir E:/scoop-work/pcre2-msvc`。PCRE2 的生成头和静态 archive 发布到完整 target 的 sysroot 目录，普通 Cone 不执行准备脚本。Windows 构建与脚本测试使用支持 UTF-8 build file 的 Ninja；可用 `ninja -t wincodepage` 确认。

`generate_public_suffix.py` 将 `sysroot/vendor/publicsuffix/public_suffix_list.dat` 的固定 PSL 规则、分区和版本注释生成普通 C 静态数据，供 HTTP Cone 打包。只有维护者更新快照时显式执行；脚本不下载数据，日常构建直接使用提交的 `.inc`。同步保留上游许可和同 revision 的测试向量，运行公共后缀的库测试及正式消费者；分发沿既有 vendor／license 收集路径携带这些资料。

开发 sysroot 准备完成后，设置 `SCOOP_SYSROOT="$PWD/sysroot"`，再运行相关的正式 CLI fixture 和库测试。Cargo 输出目录中的命令也遵守显式参数、环境变量、执行文件所属安装根的选择顺序，不再回退到编译时仓库。普通 build／run／test 和两个 link 命令默认使用目标 runtime 索引；`--runtime-root` 只用于明确需要源码构建的开发请求。准备 sysroot 不要求构建发行 LLVM、压缩归档或运行全套测试。

维护脚本共用 `compiler/target-data/targets.toml` 的目标与宿主事实。CMake 的 Apple 架构名及 C/C++ CPU 参数从所选架构产生，Darwin 不隐含 arm64；OpenSSL Configure 和 Rust 标准库仍选择各自实际需要的 recipe／Rust target。宿主工具默认目标与发行包携带的目标资源分别配置，fixture 的执行能力也不从发行组合推导。

需要重新配置已有 CMake 构建目录时，若其缓存的 `CMAKE_<LANG>_COMPILER` 与本次选择不同，准备脚本先删除 `CMakeCache.txt` 和 `CMakeFiles`（等同 `cmake --fresh`）；否则 CMake 会自行清空缓存并丢弃脚本传入的其余 `-D` 选项。编译器未变化时仍使用普通增量构建。

## 单一入口构建发行组件包

发行组件布局和 Windows native 资源拆分见[分发设计](../docs/distribution/DESIGN.md)。宿主 bin 已合并 LLVM／LLDB 运行资源，开发 SDK 仅留在维护 prefix；目标库只分发链接产物，两个 MinGW SDK 的目标数据由 native 包提供。

开发与发行 sysroot 均准备 `runtime`、`runtime-stress`、`runtime-startup-stats`、`runtime-stress-startup-stats` 四组目标 runtime，每组包含 `index.cbor`。默认构建使用 `runtime`，不编入 GC stress 或启动统计支持；`scoop --enable-gc-stress build/run/test/link` 选择 stress 配置，随后由 `SCOOP_GC_STRESS_MOVE` / `SCOOP_GC_STRESS_MINOR` 决定运行时是否开启。`--enable-startup-stats` 独立选择启动统计配置，编入后每次启动直接向 stderr 输出记录。两个开关可以组合，独立 `scoop-link` 同样支持。普通 fixtures 不传这些开关，相关专测显式选择。当前 runtime index schema 为 9，旧索引需要重新准备；`.slib` 的采集配置也需使用配套编译器重建。

可选 profiling 系统库通过同一准备入口生产，显式传入目标对应的 LLVM 22.1 compiler-rt profile archive：

```sh
python3 scripts/prepare_sysroot.py \
  --target aarch64-apple-darwin --scoop target/debug/scoop \
  --profiling --profile-runtime /path/to/llvm/lib/clang/22/lib/darwin/libclang_rt.profile_osx.a
```

该模式在 `lib/<target>/profiling/instrumented/` 生成 Profiling/full 库，在 `profiling/release/` 生成 Release/full 库；覆盖目标支持的全部系统 Cone，包括 GUI。另在 `profiling/` 下准备四种 runtime 配置及其 `profile.a`。`--runtime-only` 可只准备所选普通或 profiling runtime。native、base、GUI 的第三方资源沿原准备步骤复用，脚本不下载工具或自动生产缺失的 native 依赖。

`scoop build --profile profiling` 自动选择训练系统库和 profiling runtime，启动统计写入独立采集文件；显式 `--enable-startup-stats` 时仍向 stderr 输出原统计。维护生产可以用 `--system-library-root` 显式选择库目录。普通 release 链接拒绝含 Generate 机器码的 `.slib`，完整模板与是否采集是独立配置。`build_profile_runtime.py --target ... --source <LLVM 源码> --output <profile.a> --cc <目标 Clang>` 只构建目标 profile archive；musl 另需 `--musl-include`，Microsoft 目标沿 /MD 合同生产。完整打包自动调用该入口及 profiling 准备，再生成 `lib-profiling-<target>` 归档和 manifest 引用；复用 `--prepared-sysroot` 时须已有两套完整库、映射和 runtime。宿主提供 `scoop-llvm-profdata`，目标组件携带 compiler-rt 许可。安装与 PGO 命令的实施范围见 [profiling 记录](../docs/profiling/PROGRESS.md)。

`package_toolchain.py` 是发行组件包的唯一入口：取得固定 LLVM 输入、按 lockfile 构建 VS Code 扩展、构建 LLVM、五个 release 宿主命令与独立的 scoop-example、scoop-bundle、scoop-api，准备该宿主支持的全部 native／runtime 资源，并用本次编译器为各发行目标构建系统库 `.slib`，随包复制内置 Cone 源码、vendor、Scoop 编程 skill 和 post-install 脚本，最后按宿主／目标／全局维度生成下述组件归档。打包不生成 API 文档；文档由安装后的配套工具现场生成。无需事先逐个执行其他准备或扩展打包脚本。默认工作位置为 `tmp/package/<host>`，全部归档集中于以版本命名的 `tmp/dist/scoop-<version>/`；用 `--work-dir`、`--output-dir` 可选择外置盘目录。脚本不自动运行测试。

Linux GNU 的 native 与系统库默认使用本次准备或通过 `--llvm-prefix` 选择的 LLVM Clang；GUI 的 native 警告选项要求 Clang。`--gnu-cc` 仍可显式覆盖编译器。musl 使用自己的 `--musl-cc`，不会复用 GNU Clang 配置。

已显式准备的系统资源可通过 `--prepared-sysroot <目录>` 直接组装，跳过 native 后端与 runtime 的重复构建；宿主命令、示例与归档仍走同一流程。该目录应包含本宿主交付的各目标 native／runtime 资源、源码、vendor 和头文件，并包含各目标已经构建的系统 `.slib`；这些 `.slib` 按 GUI 划分打进对应 target 的 `lib-base-<target>`／`lib-gui-<target>` 组件包。还需保留维护准备产生的目标许可材料，例如 `lib/<target>/rust-std/`。已安装发行包将这些许可收集到 `share/licenses/`，从安装目录复用资源时需补齐维护输入布局。缺失文件按普通 I/O 报错，不加入生产过程复核。

Windows 在 x64 MSVC／Windows SDK 开发环境中运行，提供 `--mingw-msvcrt <prefix>` 与 `--mingw-ucrt <prefix>`，二者使用 `release.toml` 固定的 LLVM-MinGW 版本。UCRT prefix 同时用于 GNU-LLVM Rust 宿主和可选的宿主 LLVM 构建；Scoop 应用默认目标仍为 MSVC UCRT。默认完整准备还需要 `--windows-perl` 选择原生 Windows Perl，`--msys-perl`、`--msys-make`、`--msys-shell` 选择 MinGW OpenSSL 的完整 MSYS Perl、GNU Make 和 shell，以及 PATH 上的 NASM、NMake、CMake、Ninja、Cargo 和 npm。使用 `--prepared-sysroot` 时不调用这些 native 构建工具。

MinGW OpenSSL 构建将所选 compiler 和 MSYS shell 的目录加入子进程 PATH，compiler 目录优先，使 Make recipe 能找到同一 MSYS 安装中的 `rm`、`mkdir` 等工具；不修改用户全局 PATH。

例如，已有 LLVM 与含系统 `.slib` 的三目标 sysroot 时可运行 `python scripts/package_toolchain.py --llvm-prefix E:/scoop-work/llvm/installed --prepared-sysroot ./sysroot --mingw-msvcrt E:/llvm-mingw-msvcrt --mingw-ucrt E:/llvm-mingw-ucrt --work-dir E:/scoop-work/package --output-dir E:/scoop-work/dist --jobs 3`。Windows 发行产出三个 target 的 `lib-base-<target>` 组件包（各含非 GUI 预编译 `.slib`、native 库、runtime、每目标的 unwind archive）和 `lib-gui-<target>` 组件包（GUI `.slib` 与 Skia），两个 MinGW 目标另产出 `native-<target>`（CRT 配置头、启动对象、导入库、C++ 库和 compiler-rt），以及 `libexec/llvm/bin` 中带宿主 Clang／LLD／RC 工具和配套 resource headers 的 bin 组件包。Microsoft SDK／MSVC toolset 从使用者的开发环境发现，不复制维护机的 SDK 路径配置。

构建机需要 Python 3.12+ 及其开发文件、Git、SWIG 4、Ninja、Rust/Cargo、C/C++ 开发工具链、CMake、Make、Perl、Node.js 22.18+ 和 npm。npm 依赖、首次缺失的 LLVM 归档以及 Tree-sitter WASM 工具链需要联网取得；其余 native 上游输入已在 vendor 中。已有 WASI SDK／Binaryen 可按 [tooling 说明](../tooling/README.md) 选择。

`--vscode-extension /path/to/scoop-language-support-<version>.vsix` 可复用已经构建的当前扩展，跳过 npm 安装和扩展构建；未指定时仍自动构建。这样重试工具链打包时无需重新下载 Tree-sitter CLI。

```sh
python3 scripts/package_toolchain.py --jobs 6
```

上例用于 Darwin。Linux 还要提供 `--musl-include /usr/include/x86_64-linux-musl`，并安装 `musl-gcc`、`patchelf` 和适用于 LLVM libunwind 的 Clang／Clang++；Rust 工具链需要同时具有 `x86_64-unknown-linux-musl` 目标。可通过 `--gnu-cc`、`--musl-cc`、`--unwind-cc`、`--unwind-cxx` 明确选择工具。Linux 宿主发行同时包含 GNU 与 musl 目标资源。

musl 的 C 开发环境也需要 Linux UAPI 头。Debian／Ubuntu 的 `musl-gcc` 默认搜索目录可能缺少 `linux/mman.h`；此时可在工作目录准备仅包含 `linux`、`asm`、`asm-generic` 的头文件搜索根，以编译器包装脚本添加该根，并通过 `--musl-cc` 选择包装脚本。这些目录可指向系统已安装的对应 Linux 头；保留 musl 自己的 libc 头文件搜索顺序。

`--download-dir` 指定可跨工作目录复用的外部依赖原文件目录，优先于 `SCOOP_DOWNLOAD_DIR`，默认仓库 `tmp/downloads`；相对路径按调用 cwd 解析。自动选择先复用该目录固定条目，再用 vendor 的已有输入，均缺失才下载；命中不发送网络请求或重新计算摘要。LLVM 按 `llvm-project/<version>/<filename>` 保存原归档，工作目录的 `inputs` 只存解包副本及更新记录。LLVM-MinGW ZIP 和所选 CPython LICENSE 也复用此目录；补丁只应用于工作副本。也可传入已有的 `--llvm-source` 和 `--llvm-archive`，显式路径不存在时直接报错。已经准备的匹配版本宿主 LLVM 可通过 `--llvm-prefix` 复用；省略时脚本在自己的工作目录构建。`--version` 和 `--revision` 可显式设置发行标识，默认采用配置版本与当前 Git revision，存在未提交修改时标注 `+modified`。这些参数也写入安装根的 `VERSION`。

同一 release 的全部归档输出到以版本命名的目录（`tmp/dist/scoop-<version>/`），按维度分为组件包；每个归档内直接是安装根内容，并集解压到同一安装根（如 `~/.scoop`）即为完整安装，不需要 `--strip-components`：

- `scoop-<version>-bin-<host>.tar.xz`：每宿主一份，包含 `bin/`（五个命令，Windows 主驱动为 `sco.exe`）、`commands/`、`VERSION`、`post-install.sh`／`post-install.ps1`、`tooling/`（VSIX 与 scoop-programming skill）、`share/scoop/`（toolchain.toml、install/targets.list、各 `<target>.list` 与 `<target>.gui.list`、README、RELEASE-NOTES）及 Scoop 自身许可。 同一包还包含 `libexec/llvm/` 中选定的 Clang／LLD／LLVM 工具、Clang builtin headers 和实际动态依赖，`libexec/lldb/` 中的 LLDB／Python，公开的 `scoop-` 前缀调试工具及 formatter 和许可；不包含 LLVM SDK 头、静态库、CMake exports、llvm-config 或 llc。

- `scoop-<version>-sources.tar.xz`：全局一份，包含 `src/`（全部内置 Cone 源码、清单、胶水和资源）、`native/text_backend/`、`include/` 和 `share/scoop/examples/`。
- `scoop-<version>-lib-base-<target>.tar.xz`：每 target 一份，与构建宿主无关，包含 `lib/<target>/` 中非 GUI 系统 Cone 的预编译 `*.slib`、对应 native 静态库、`openssl/`、`nghttp2/`、`unwind/`（非 Darwin）与四种 `runtime*/` 配置，以及该 target 的 native 组件与 rust-std 许可。Darwin/AArch64 宿主发行 AArch64／x86_64 两个 Darwin 目标的 lib 包，Darwin/x86_64 宿主发行 x86_64 目标，Linux/GNU amd64 宿主同时发行 GNU／musl 两套目标。
- `scoop-<version>-lib-gui-<target>.tar.xz`：该 target 有 GUI Cone 时每 target 一份，包含 GUI Cone（`scoop.window`、`scoop.graphics` 及所有依赖它们的 Cone，由 `package_toolchain.py` 按 Cone 依赖图划分）的预编译 `*.slib`，以及 Skia、GTK／libadwaita、tray 等 GUI native 链接库和许可；须与同 target 的 lib-base 包一起安装。
- `scoop-<version>-native-<target>.tar.xz`：仅两个 MinGW 目标，提供 `lib/<target>/toolchain` 内的 CRT／libc++ 配置头、启动对象、Windows／CRT 导入库、C++ 静态库及 compiler-rt builtins；不包含 executable。公共 MinGW／libc++ 头由 Windows bin 的 `libexec/mingw/include` 提供一次。MSVC 继续使用本机 SDK 或显式 msvc.toml，不发空的 native 包。
- `scoop-<version>-vendor.tar.xz`：全局一份，包含 `vendor/` 中全部上游源码、固定版本说明、许可及编译补丁；LLVM 原始归档和 LLVM／LLDB 补丁也归此包。按需安装，默认开发组合不下载。所有组件的归档顶层均为安装根内容。

同一 release 的包按相同版本号成组安装，不跨版本混装。

```text
~/.scoop/
  VERSION               发行版本、源码 revision 与宿主平台（bin 包）
  post-install.sh       Unix 安装后文档生成入口；Windows 为 post-install.ps1（bin 包）
  bin/                  scoop、scoopc、scoop-link、scoop-lsp、scoop-pm（bin 包）；
                        scoop-lldb 等 LLVM 启动器（bin 包）
  commands/             scoop-example 与其他外部子命令（bin 包）
  lib/<target-triple>/  预编译 *.slib、native 链接库、四种 runtime 配置（lib-base-<target> 包）；
                        GUI *.slib、Skia 等 GUI native 库（lib-gui-<target> 包）
  include/              公开 Scoop C 头文件（sources 包）
  src/                  全部内置 Cone 源码（sources 包）
  vendor/               native 上游输入和补丁（sources 包）
  native/               Rust 文本后端维护源码（sources 包）
  libexec/scoop/include/unwind/  Linux／Windows 的公共链接探测头（bin 包）
  libexec/mingw/include/         Windows 公共 MinGW／libc++ 头（bin 包）
  libexec/llvm/         选定 LLVM 工具、builtin headers 和动态依赖（bin 包）
  libexec/lldb/         LLDB、DAP、argdumper、平台进程调试 helper 及随包 Python（bin 包）
  share/scoop/          toolchain.toml、install 清单、README、RELEASE-NOTES（bin 包）；
                        示例（sources 包）、debugger formatter（bin 包）、各组件许可
  docs/                 安装后由 post-install 生成的离线 API 文档
  tooling/              scoop-language-support-<extension-version>.vsix、scoop-programming/（bin 包）
```

同一 release 的组件包按相同版本号成组安装，依次解压到同一安装根后运行 `sh ~/.scoop/post-install.sh`，Windows 运行 `& "$HOME/.scoop/post-install.ps1"`。脚本使用 POSIX shell／PowerShell 直接调用随包编译器，不要求用户安装 Python、Rust、Cargo、CMake 或 vendor 构建工具；LLDB 所需的配套 Python 仍随 bin 包提供。post-install 只负责为已安装的目标生成离线 API 文档，不再构建系统库；它对每个所选目标先检查 `lib/<target>/` 已随对应 `lib-base-<target>` 组件包安装（含系统 `.slib`），缺失时报错并指出应安装的 `scoop-<version>-lib-base-<target>.tar.xz`；已安装 `lib-gui-<target>` 时同时为 GUI Cone 生成文档。

`--target <triple>` 可重复选择其他已安装的随包目标，`--all-targets` 处理清单中全部已安装的目标，跳过未安装的目标。支持 `--target`、`--all-targets`、`--jobs` 及 `--work-dir`；默认工作目录为安装根的 `build/post-install`，保留普通构建缓存和 lock，重复运行复用未变化的结果。文档生成失败返回非零状态，修正实际错误后重试同一命令。

安装对应 `lib-base-<target>`（及需要 GUI 时的 `lib-gui-<target>`）组件包，Windows MinGW 同时安装对应 `native-<target>` 后，`~/.scoop/bin/scoop` 默认使用相邻的 `~/.scoop/lib/<target-triple>/*.slib`，不因 debug profile 或空缓存重编系统库。显式 `--sysroot`／`SCOOP_SYSROOT` 选择目标库与 Scoop 资源根，宿主 LLVM 工具及公共头由实际执行文件所属安装根提供；source locator 保持原有规则。安装及 VSIX 使用步骤见[随包说明](distribution/README.md)。

`sysroot/commands` 整目录进入安装包，保留程序的可执行权限；空目录也保留，`.gitkeep` 不打包。`scoop [--sysroot path] somecommand <args>` 直接执行所选安装根的 `commands/scoop-somecommand`，后续参数全部原样交给该程序。

`tooling/scoop-programming/` 原样进入 dist bin 组件包的同名目录，包含 `SKILL.md`、界面元数据和参考资料；`share/scoop/toolchain.toml` 的 `programming_skill` 字段记录其安装根相对路径。skill 引导 AI 使用配套示例、库文档和 CLI，不需要额外构建步骤，也不写入任何 AI 工具的技能目录或用户配置。内容与维护说明见 [tooling README](../tooling/README.md#scoop-编程-skill)。

脚本读取宿主命令的实际动态依赖，将所需非系统库复制到 `libexec/llvm/lib`，为实际引用这些私有库的 Darwin 文件设置相对加载路径。目标资源和许可说明按实际组件收集；`--jobs` 同时控制独立组件的并行压缩数，每个归档在完整写入后才替换同名文件，全部归档成功后才写本次组件清单。安装方法、系统开发环境要求及升级说明随包放在 `share/scoop`。

Windows 的宿主 DLL 通过同一归档内的硬链接放在实际使用它们的 `bin`、外部命令目录及 `libexec/llvm/bin`；公共 LLVM／LLDB／Python 的 DLL 只在同一个 bin 归档内共享，不与目标资源跨包链接。LLDB／Python 的依赖优先从所选 Python 安装查找，包含 Conda 的 `Library/bin`，再回退到环境 `PATH`；仅收集实际导入的 DLL 及其传递依赖。LLVM、libc++／libc++abi、libunwind、compiler-rt、MinGW runtime 与 Rust 的许可随包收集，版本与目标信息进入组件清单。

打包从 `sysroot/src/*/Cone.toml` 生成目标生产清单：显式非空 `sources` 中至少一项满足该目标的条件才进入清单；缺省或显式空表仍由编译器处理。选中库依赖该目标未提供的内置库时报告错误。清单中的 Cone 目录按依赖顺序保存在 `share/scoop/toolchain.toml` 的各目标 `cones` 数组，`default_target` 记录宿主默认目标。同一选择写入 `share/scoop/install/targets.list`（本 release 为该 host 提供的目标，默认目标在首行）及 `<target>.list`（逐行 Cone 目录），供 post-install 的文档生成直接读取。Linux GNU 的 GUI Cone 进入 GNU 清单，未提供 musl 源码的 GUI Cone 不进入 musl 清单，不需要空后端。

post-install 使用配套 `scoop`／`scoopc`，显式选择上述目标清单，以 `--no-deps --deny-doc-warnings` 为所选目标生成共同站点。嵌套测试／辅助 Cone 不进入发行清单。工作目录下的 `doc-target` 和普通 `cache` 保留中间结果；文档直接生成到安装根的 `docs/<target>`，保留普通文档更新记录，重复执行复用未变化的文档。文档失败使本次 post-install 返回非零状态，不将失败报告为安装成功。

安装根的 `docs/index.html` 列出已经准备的目标，具体站点位于 `docs/<canonical-target-triple>/`。追加准备其他目标时保留已经生成的站点。源码原文嵌入 HTML，搜索和库内资源均在本地；指向仓库 `docs/`、`scripts/`、`tests/` 的 Markdown 链接在组装时改为本次源码 revision 的 GitHub 链接，作为设计及维护文档的外部参考。`.slib` 不保存完整源码。手工验收时检查安装后的目标及库清单、跨库链接、源码页、离线搜索与重复生成的复用情况；文档能力的详细范围见 [M60 设计](../docs/milestone60/DESIGN.md)。

手工验证发布包时，将同一版本的 bin、sources 与所选 lib 组件归档（MinGW 还需同目标 native）依次解压到同一个新目录，可使用带空格路径；保留源码、原工具和原产物。确认 lib 归档包含该目标的系统 `.slib`、所有包没有 `docs/`，再执行 post-install。运行五个命令的版本查询，检查实际加载的私有 LLVM，再用空缓存完成含 native／泛型／GC 的应用、crypto／TLS、普通库测试和两个 link 入口，重复构建检查复用。Linux 还应覆盖 GNU 与 musl 的既有链接模式。常规 fixture 使用已准备的开发 sysroot，不执行打包或解压步骤。

打包脚本的归档布局与失败保留检查使用 `python3 -m unittest discover -s scripts/tests`；运行前对改动的 Python 脚本执行与 fixture runner 相同版本的 Ruff 格式化及 lint。完整包验收还应确认 `VERSION` 与命令版本一致、VSIX 内包含扩展代码及两个 WASM，并使用解压目录中的工具完成冷构建／运行及再次复用。

## GitHub Release

[Release workflow](../.github/workflows/release.yml) 在推送 `vMAJOR.MINOR.PATCH`（可带预发布和 build metadata）tag 时执行，也可通过 `workflow_dispatch` 指定已有 tag 重跑。三个构建任务使用 `xcode-27`（macOS 27.0.1 AArch64）、`windows-2025`（x64）和 `ubuntu-26.04`（x64），按 `release.toml` 的固定版本准备 LLVM、Windows Rust 与 LLVM-MinGW，并复用匹配的 LLVM／LLDB SDK 缓存。

每个任务调用同一个 `package_toolchain.py`，产出本宿主的 `bin-<host>` 组件包及该宿主发行目标的 `lib-base-<target>.tar.xz`／`lib-gui-<target>.tar.xz`；全局一份的 `sources` 与 `llvm-sources` 由其中一个任务产出。随后在带空格的独立安装目录中合并解压组件包、执行 post-install 并编译运行程序；这些验证产物不进入归档。三平台全部成功后才向对应 GitHub Release 上传全部组件包，同一 release 的附件列表即以版本命名的完整集合，发布前使用 draft；带预发布标识的 tag 发布为 prerelease，普通版本更新 latest。重跑同一 tag 会更新同名附件。

官网安装脚本 `install.sh` 位于独立仓库 `scoop-lang.org`；它读取 `https://release.scoop-lang.org/latest.txt` 的组件索引，并从同一发布站点下载本平台归档。`--release <日期目录>`（别名 `--version`）选择指定 release。`curl -fsSL https://scoop-lang.org/install.sh | sh` 将组件包解压到 `~/.scoop` 并执行 post-install（生成文档），成功后更新 shell rc。额外准备参数放在 `--` 后，例如 `sh install.sh --release 2026-11-09 -- --jobs 2`。Windows 按索引下载所选组件后运行随包 PowerShell 入口；MinGW 目标还需要对应的 `native` 包。

## 独立示例命令

`python3 scripts/build_scoop_api.py` 构建无 LLVM 执行依赖的 `scoop-api` 并安装到开发 sysroot 的 `commands/`；使用 workspace lockfile，可指定 `--sysroot` 与 `--target-dir`。完整发行流程也包含该命令。API 数据由原有 `scoop doc`／post-install 随同页面生成，查询不触发生成。用法和格式见 [API 查询](../docs/scoop-doc/API.md)。

`python3 scripts/build_scoop_pgo.py` 构建并安装独立 `scoop-pgo`，同样使用 workspace lockfile，接受 `--sysroot` 与 `--target-dir`。正式入口为 `scoop pgo instrument/optimize`，默认自动选择 profiling 组件中的训练或反馈库；训练数据与普通构建缓存分开。开发时可用 `--llvm-profdata` 指向配套 LLVM 22.1，完整发行流程会安装该工具。使用方式见 [PGO 命令](../tooling/scoop-pgo/README.md)。

`python3 scripts/build_scoop_example.py` 仅构建 `tooling/scoop-example` 并将命令安装到开发 sysroot，不调用编译器。它使用独立 Cargo lockfile；新增示例与正文位于 `sysroot/share/scoop/examples`，已有库示例保持原文件。可用 `--sysroot` 安装到另一份配套工具链，用 `--target-dir` 指定构建缓存。完整打包入口自动完成这一步，并复制所有主题材料。内容维护和独立测试见[阅读器说明](../tooling/scoop-example/README.md)。


## 应用打包外部命令

`python3 scripts/build_scoop_bundle.py` 独立构建 `tooling/scoop-bundle` 并安装到开发 sysroot 的 `commands/`，不构建 LLVM。`--sysroot` 与 `--target-dir` 分别选择安装位置和构建缓存。完整工具链打包自动包含该命令及其 Rust 依赖许可；应用本身的打包用 `scoop bundle`，配置与示例见 [工具说明](../tooling/scoop-bundle/README.md)。各格式的系统打包工具由维护环境显式准备。

## 调试器发行资源

`package_toolchain.py` 在 LLVM／Clang SDK 后调用 `build_lldb.py`，固定 LLVM 22.1.8，
启用 Python，构建 lldb、lldb-dap、lldb-argdumper 和 debugserver／lldb-server。
Darwin 开发安装在 CMake 调整加载路径后重新签名 debugserver，并保留调试 entitlements。
源码兼容修补保留 Scoop 的 DW_AT_name 与精确 linkage identity，将同源文件的独立编译单元全局量放回当前栈帧，并补齐 Linux/amd64 静态程序的原生 TLS 定位；修补文件也进入 llvm-sources 包。
可用 `--debugger-prefix` 复用匹配的已构建调试器，`--debugger-python` 指定构建时所用 Python；
只有 LLVM SDK 时仍正常构建 LLDB，要求 SDK 包含 Clang headers／CMake targets／clang-cpp、构建用 `llvm-tblgen` 以及 TableGen／Support／Demangle／Debuginfod 静态库；`build_llvm.py` 会安装这些资源。
Linux 构建的 LLDB 安装 RUNPATH 同时包含所选 Python 的 `LIBDIR`；复用既有 LLDB 时，打包也会从选定 Python 查询该目录以收集 libpython。共享库复制后改写为相对路径，附带解释器与其他 ELF 命令统一使用 Scoop 宿主命令的系统 loader，不保留 Homebrew 的绝对 interpreter 路径。

`package_debugger.py` 收集匹配的 Python 标准库、原生模块和解释器，排除用户 site-packages、
测试、Tk／IDLE 与 pip；包中不依赖用户 Python。Python 许可证随包保留，安装中缺失时可用
`--python-license` 明确提供。所有实际非系统动态依赖统一收集并改为相对加载路径；
Darwin 对最终文件重新签名，保留 debugserver 的 entitlements。
发行包公开 `bin` 中的 LLVM／LLDB 命令统一带 `scoop-` 前缀，不保留无前缀别名；例如 `scoop-lldb`、`scoop-lldb-dap`、`scoop-dsymutil` 与 `scoop-llvm-symbolizer`。构建 SDK 和安装根内私有 `libexec` 目录保留上游命名。
`bin/scoop-lldb` 和 `bin/scoop-lldb-dap` 通过相对安装根设置 Python 与 helper 路径，`scoop-lldb` 自动加载 formatter。
Windows 对应的入口是 `bin/scoop-lldb.exe` 和 `bin/scoop-lldb-dap.exe`，由原生启动程序设置随包 Python 路径，保留 Unicode 参数、标准流与子进程退出码；无需 POSIX shell。Windows LLDB 与宿主 LLVM 使用同一个 LLVM-MinGW UCRT C/C++ 工具链，独立运行 `build_lldb.py` 时以 `--cc`／`--cxx` 指定它。维护构建需 SWIG 4、Python 开发文件和 Ninja；应用使用发行包无需这些构建工具。

随包 LLDB 优先使用 PE 的匹配 PDB，避免 LLVM-MinGW CRT 的 DWARF 遮蔽 Scoop 程序的调试信息；没有匹配 PDB 时保留 DWARF 回退。相关修正在 `patches/lldb-scoop-source-names.patch`，与源码名称、类型布局和线程 TLS 修正一起应用。

本地调试器专项验证使用启用 Python 的真实 LLDB 构建：将打包目录移动到含空格的路径，
以仅保留系统 PATH 的环境启动 LLDB，验证 formatter、Python 标准模块和动态库加载。
未移走或隐藏原工具；通过 dyld 加载记录确认没有引用原 Homebrew／构建目录的私有动态库。
Linux 的同类检查还包括 ELF loader 记录、GNU／musl 实际 DAP 会话及静态 TLS；可重复入口见[调试器验收说明](../tooling/vscode-scoop/test/debugger/README.md)，本轮结果见[实施记录](../docs/debugging/IMPLEMENTATION.md)。

发行完成后输出 `packages-<host>.list`，只列出本次成功生成的组件文件名。Release workflow 据此准备上传目录，避免重复构建时把已退役的旧组件重新发布。

发行 base／GUI 包不复制第三方开发头、源码或完整工具链；runtime 的 index.cbor 和 objects/*.o 必须成组保留，Linux GUI 继续包含实际链接用共享库。最终链接默认从 bin 读取公共 unwind 头，从所选 sysroot 读取目标 libunwind.a；显式 --unwind-prefix 和 runtime 源码准备仍使用各自完整维护 prefix。

发行目录同时包含 `manifest.toml`、本宿主 `manifest-<host>.toml` 与 `manifest-shared.toml`。本地可直接使用完整 manifest；CI 合并各宿主片段及一份全局片段后发布完整索引。格式与发布规则见 [发行索引 schema 1](../docs/distribution/MANIFEST.md)。
