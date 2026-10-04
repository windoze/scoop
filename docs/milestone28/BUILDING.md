# M28 本机构建与验证

从仓库根目录运行。当前已提供 LLVM libunwind 准备和真实异常展开测试；完整 Scoop Linux CLI 的完成状态见 [实施记录](PROGRESS.md)。

## LLVM 与 unwind 准备

Scoop 后端使用 LLVM 22.1，本机配置为：

```sh
export LLVM_SYS_221_PREFIX=/usr/lib/llvm-22
mkdir -p target/tmp
export TMPDIR="$PWD/target/tmp"
```

从用户提供的 LLVM 22.1.2 源码分别构建 glibc、musl 静态 PIC unwinder：

```sh
python3 scripts/build_llvm_unwind.py \
  --source ../llvm-project-22.1.2.src \
  --target x86_64-unknown-linux-gnu
python3 scripts/build_llvm_unwind.py \
  --source ../llvm-project-22.1.2.src \
  --target x86_64-unknown-linux-musl \
  --musl-include /usr/include/x86_64-linux-musl
python3 scripts/check_linux_unwind.py
```

默认构建目录为 `target/llvm-unwind/<triple>`，安装目录为 `sysroot/native/<triple>/unwind`，包含 `include/unwind.h` 和 `lib/libunwind.a`。这些产物不提交 Git。构建过程的临时文件也位于相应 build 目录内；可以通过 `--build-dir`、`--prefix` 覆盖路径。C/ASM 和 C++ compiler 分别使用 `--cc`、`--cxx` 指定的 Clang，默认查找 PATH 中的 `clang`、`clang++`。Clang 不要求与 Scoop 后端版本相同。本机 musl include 路径在其他发行版需按实际安装调整。

构建脚本仅消费本地源码，通过 CMake 的 `runtimes` 入口构建并安装 `libunwind`，不下载源码，也不覆盖系统 unwinder。它为两种 libc 使用独立目录；普通 `scoop build` 不负责构建第三方源码。

验证脚本默认使用 `gcc`、`musl-gcc`（可用 `--gnu-cc`、`--musl-cc` 覆盖），为 glibc PIE、musl 静态、musl PIE 三种配置运行真实 LLVM cleanup/resume/catch 测试，检查 ELF interpreter、静态程序的动态依赖与 link map 中的 EH provider。它覆盖 personality 和 Level I 展开，不代替完整异常对象生命周期、GC 或语言 fixture。

## 构建目录管理

开发验证可以设置 `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`，减少大型 workspace 的中间产物。使用同一组设置完成一批验证，避免生成多套重复缓存。

定期查看 `du -sh target`。unwind 安装和真实展开测试通过后，可以删除 `target/llvm-unwind` 的构建中间文件，保留 `sysroot/native` 中的 headers/archive；不得清理仍在运行的构建或测试目录。最终 macOS/AArch64 回归在 `m3u.0d0a.com:~/repos/scoop` 中进行，临时目录也须位于该仓库下。
