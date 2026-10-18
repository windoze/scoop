# 开发 sysroot 与发行准备

这些脚本供维护仓库和制作发行包时手动运行，不属于 `scoop` 命令，也不由 driver、常规测试或 CI 自动调用。Python 脚本要求 Python 3.11 或更新版本；OpenSSL 解包使用 Python 3.12 或更新版本。

发行配置集中在 `release.toml`。当前 Darwin 基线为 macOS 27.0.1／SDK 27.0，Linux 构建环境为 Ubuntu 26.04.1、glibc 2.43／musl 1.2.5；只在相应环境完成验证后声明支持，不推定更老系统可用。LLVM 固定 22.1.8，CPU 使用 AArch64 generic／x86-64。工具构建时写入默认 Darwin 最低版本，普通命令的 `MACOSX_DEPLOYMENT_TARGET` 覆盖贯通 Scoop 对象、C bridge、native、runtime 与最终链接；预编译 runtime 的最低版本仍必须兼容。OpenSSL 准备使用同一设置。

三个命令的 `--version` 显示相同发行版本与源码 revision。开发构建读取默认版本和当前 Git revision；发行构建显式设置 `SCOOP_RELEASE_VERSION` 与 `SCOOP_SOURCE_REVISION`，不在运行时读取 Git 或发行清单。

`build_llvm.py` 从 [固定的 LLVM 22.1.8 上游输入](../sysroot/vendor/llvm-project/22.1.8/README.md) 构建宿主动态库，仅启用 AArch64／X86 后端，并安装 Rust 构建所需的 llvm-config、头文件和 CMake 资料。关闭未使用的压缩、XML、编辑器、HTTP 和插件依赖，不移除 GC／EH 或 statepoint 后端。默认工作位置为 `tmp/llvm/<host>`，可用 `--source`、`--build-dir`、`--prefix` 选择其他位置。

```sh
python3 scripts/build_llvm.py --jobs 6
python3 scripts/build_llvm_unwind.py --target x86_64-unknown-linux-gnu \
  --cc clang-22 --cxx clang++-22
python3 scripts/build_llvm_unwind.py --target x86_64-unknown-linux-musl \
  --cc clang-22 --cxx clang++-22 --musl-include /usr/include/x86_64-linux-musl
```

两个 LLVM 准备脚本在复制输入、补丁、工具探测和 CMake 之前检查输入 metadata 与所需输出；保存首次构建实际 depfile 中的头文件输入，更新时仍由 CMake 的普通增量构建处理。命中不读回 archive、对象或 LLVM 动态库。libunwind 的三个 Linux 链接模式可按 [M28 说明](../docs/milestone28/BUILDING.md) 单独验证。

先构建配套的 `scoop`、`scoopc`、`scoop-link`，并用 `build_openssl.py`、`build_nghttp2.py`、`build_text_backends.py` 准备所需目标的 OpenSSL、nghttp2 和 IDNA／字符集后端。Linux 还需要按 `build_llvm_unwind.py --help` 准备同目标的 unwind。然后准备开发 sysroot，例如：

```sh
python3 scripts/build_nghttp2.py --target aarch64-apple-darwin
python3 scripts/build_text_backends.py --target aarch64-apple-darwin --build-dir ../scoop-work/text/aarch64-apple-darwin
python3 scripts/build_uriparser.py --target aarch64-apple-darwin --build-dir ../scoop-work/uriparser/aarch64-apple-darwin
python3 scripts/prepare_sysroot.py \
  --target aarch64-apple-darwin \
  --scoop "$CARGO_TARGET_DIR/release/scoop" \
  --work-dir "$PWD/tmp/sysroot/aarch64-apple-darwin"
```

`--sysroot` 默认为仓库的 `sysroot`，`--runtime-root` 默认为仓库的 `runtime`；可显式选择 `--cache-dir`、`--scoopc` 和现有 native 工具参数。维护脚本默认将工作目录放在仓库已忽略的 `tmp` 下，避免占用 HOME 所在磁盘；也可显式指定其他磁盘。构建缓存、中间产物和辅助程序的 Rust target 目录保存在工作目录或显式 `CARGO_TARGET_DIR`，不要选择绝对路径 `/tmp`。目标输出为 `sysroot/lib/<triple>`，公开头文件为 `sysroot/include`。

脚本读取 `src/*/Cone.toml` 的生产依赖确定顺序，逐个执行普通 release library 构建；测试源码不进入发布库。`runtime_builder` 是独立于 Cargo 主 workspace 的脚本内部辅助程序，复用已有的 O2 runtime 编译与索引实现，不安装到发行包的 `bin`。准备过程不重新读取对象以核对摘要或复验产物；重复运行使用已有构建记录，输入发生变化时更新对应结果。runtime 与公开头文件先复制完整输出，再替换准备目录；复制失败保留原目录。

`build_nghttp2.py` 从 vendor 的固定 1.70.0 tag 归档构建纯静态协议库，保留完整上游核心，不构建应用、工具、测试或文档。使用同一目标部署／CPU 配置；工作目录默认为 `tmp/nghttp2/<triple>`，支持 `--build-dir`、`--prefix`、`--cc`。更新判断先于解包和 CMake，命中不读取或复验 archive。llhttp 9.4.3 的发布 C 源码由普通 HTTP Cone 编译，不另加工具或命令。

`build_text_backends.py` 依次准备带严格转换修正的 libiconv 1.19、libunistring 1.4.2 和带 ContextO／Bidi／NFC 校验修正的 libidn2 2.3.8。必须显式给出仓库外 `--build-dir`；支持目标 `--cc`、资源目录 `--prefix` 与 `--jobs`。静态库放在 `sysroot/lib/<triple>`，头文件放在其 `include` 子目录，直接使用现有 native 搜索路径。只把库和头文件复制到目标资源，不携带构建出的命令行程序；完整源码、许可和补丁由 vendor 随工具链分发。

`build_uriparser.py` 以相同布局准备 uriparser 1.0.2，仅启用窄字符静态库，关闭工具、文档、测试及宽字符 API 的构建。需要 CMake 和所选目标 C 编译器；使用仓库外 `--build-dir`，其余目标、编译器与并行选项同上。输入未变化时直接复用，实际更新后在维护工作区重建，再发布库与头文件。

开发 sysroot 准备完成后，设置 `SCOOP_SYSROOT="$PWD/sysroot"`，再运行相关的正式 CLI fixture 和库测试。Cargo 输出目录中的命令也遵守显式参数、环境变量、执行文件所属安装根的选择顺序，不再回退到编译时仓库。普通 build／run／test 和两个 link 命令默认使用目标 runtime 索引；`--runtime-root` 只用于明确需要源码构建的开发请求。准备 sysroot 不要求构建发行 LLVM、压缩归档或运行全套测试。

## 手工生成工具包

`package_toolchain.py` 构建三个 release 宿主命令、准备该宿主支持的目标资源，并生成工具包与 LLVM 源码包。默认工作位置为 `tmp/package/<host>`，归档位于 `tmp/dist`；用 `--work-dir`、`--output-dir` 可选择外置盘目录。脚本不自动运行测试。

```sh
python3 scripts/package_toolchain.py --jobs 6
```

上例用于 Darwin。Linux 还要提供 `--musl-include /usr/include/x86_64-linux-musl`，并安装 `musl-gcc`、`patchelf` 和适用于 LLVM libunwind 的 Clang／Clang++；可通过 `--gnu-cc`、`--musl-cc`、`--unwind-cc`、`--unwind-cxx` 明确选择工具。Linux 包同时包含 GNU 与 musl 目标资源。

musl 的 C 开发环境也需要 Linux UAPI 头。Debian／Ubuntu 的 `musl-gcc` 默认搜索目录可能缺少 `linux/mman.h`；此时可在工作目录准备仅包含 `linux`、`asm`、`asm-generic` 的头文件搜索根，以编译器包装脚本添加该根，并通过 `--musl-cc` 选择包装脚本。这些目录可指向系统已安装的对应 Linux 头；保留 musl 自己的 libc 头文件搜索顺序。

LLVM 源码与归档默认取 vendor 的固定版本位置，也可传入 `--llvm-source` 和 `--llvm-archive`。已经用 `build_llvm.py` 准备的宿主 LLVM 可通过 `--llvm-prefix` 复用；省略时脚本在自己的工作目录构建。`--version` 和 `--revision` 可显式设置发行标识，默认采用配置版本与当前 Git revision，存在未提交修改时标注 `+modified`。

脚本读取宿主命令的实际动态依赖，将所需非系统库复制到 `libexec/llvm/lib`，设置相对加载路径。目标资源和许可说明按实际组件收集；归档在完整写入后才替换同名文件。安装方法、系统开发环境要求及升级说明随包放在 `share/scoop`。

手工验证发布包时，将归档解压到新的目录，可使用带空格路径；保留源码、原工具和原产物。运行三个命令的版本查询，检查实际加载的私有 LLVM，再用空缓存完成含 native／泛型／GC 的应用、crypto／TLS、普通库测试和两个 link 入口，重复构建检查复用。Linux 还应覆盖 GNU 与 musl 的既有链接模式。常规 fixture 使用已准备的开发 sysroot，不执行打包或解压步骤。具体结果记录在 [M52 实施记录](../docs/milestone52/PROGRESS.md)。
