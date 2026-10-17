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

先构建配套的 `scoop`、`scoopc`、`scoop-link`，并用 `build_openssl.py` 准备所需目标的 OpenSSL。Linux 还需要按 `build_llvm_unwind.py --help` 准备同目标的 unwind。然后准备开发 sysroot，例如：

```sh
python3 scripts/prepare_sysroot.py \
  --target aarch64-apple-darwin \
  --scoop "$CARGO_TARGET_DIR/release/scoop" \
  --work-dir "$PWD/tmp/sysroot/aarch64-apple-darwin"
```

`--sysroot` 默认为仓库的 `sysroot`，`--runtime-root` 默认为仓库的 `runtime`；可显式选择 `--cache-dir`、`--scoopc` 和现有 native 工具参数。维护脚本默认将工作目录放在仓库已忽略的 `tmp` 下，避免占用 HOME 所在磁盘；也可显式指定其他磁盘。构建缓存、中间产物和辅助程序的 Rust target 目录保存在工作目录或显式 `CARGO_TARGET_DIR`，不要选择绝对路径 `/tmp`。目标输出为 `sysroot/lib/<triple>`，公开头文件为 `sysroot/include`。

脚本读取 `src/*/Cone.toml` 的生产依赖确定顺序，逐个执行普通 release library 构建；测试源码不进入发布库。`runtime_builder` 是独立于 Cargo 主 workspace 的脚本内部辅助程序，复用已有的 O2 runtime 编译与索引实现，不安装到发行包的 `bin`。准备过程不重新读取对象以核对摘要或复验产物；重复运行使用已有构建记录，输入发生变化时更新对应结果。runtime 与公开头文件先复制完整输出，再替换准备目录；复制失败保留原目录。

开发 sysroot 准备完成后，设置 `SCOOP_SYSROOT="$PWD/sysroot"`，再运行相关的正式 CLI fixture 和库测试。Cargo 输出目录中的命令也遵守显式参数、环境变量、执行文件所属安装根的选择顺序，不再回退到编译时仓库。普通 build／run／test 和两个 link 命令默认使用目标 runtime 索引；`--runtime-root` 只用于明确需要源码构建的开发请求。准备 sysroot 不要求构建发行 LLVM、压缩归档或运行全套测试。
