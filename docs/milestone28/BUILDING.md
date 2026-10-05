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

LLVM 22.1.2 在本机 musl 动态模式的完整 `_Unwind_Backtrace` 中暴露了 ELF 索引未命中后的越界扫描。脚本将 `libunwind` 和 `runtimes` 复制到 build 目录的 `source/`，应用 [最小兼容补丁](../../scripts/patches/llvm-libunwind-eh-index-miss.patch)，再在 `objects/` 构建；用户的 LLVM 源码保持不变。补丁让已存在的 FDE 搜索表未命中直接返回，不再假设 `.eh_frame` 必有零终止记录；没有索引时保留上游原路径。该修复不改公开 `_Unwind_*` ABI，实际 archive 内容继续作为最终链接输入。源码版本变化导致补丁不再适用时，脚本报错，应重新核对上游实现后更新补丁。

验证脚本默认使用 `gcc`、`musl-gcc`（可用 `--gnu-cc`、`--musl-cc` 覆盖），为 glibc PIE、musl 静态、musl PIE 三种配置运行真实 LLVM cleanup/resume/catch 测试，检查 ELF interpreter、静态程序的动态依赖与 link map 中的 EH provider。它覆盖 personality 和 Level I 展开，不代替完整异常对象生命周期、GC 或语言 fixture。

## 编译器 CLI 与库产物

三个命令需一起重建，父子协议当前为 3：

```sh
cargo build -p scoop -p scoopc -p scoop-linker --bins
target/debug/scoopc build sysroot/lib/scoop.core \
  --target x86_64-unknown-linux-gnu --cc /usr/bin/gcc \
  --out-slib target/core-gnu.slib
target/debug/scoopc build sysroot/lib/scoop.core \
  --target x86_64-unknown-linux-musl --cc /usr/bin/musl-gcc \
  --out-slib target/core-musl.slib
```

`scoopc build` 接受 `--cc`、`--native-sysroot`；`scoop build/run/link` 与
`scoop-link` 还接受 `--unwind-prefix` 和 `--link-mode static|dynamic`。
`--sysroot` 是 Scoop core 布局，`--native-sysroot` 是 C 开发环境；musl wrapper
使用已安装的 specs/headers 时无需填写后者。纯 library 构建不读取 unwind archive。
Linux 未指定 cache 时先取绝对 `XDG_CACHE_HOME/scoop`，其次为 `$HOME/.cache/scoop`。

正式 library fixture 验证从源码构建 core、父子工具选择、C bridge、产物发布及再次命中缓存：

```sh
python3 tests/run_fixtures.py --suite tests/fixtures/m28-linux/cli-library \
  --target x86_64-unknown-linux-gnu --work-dir target/m28-fixtures/gnu
python3 tests/run_fixtures.py --suite tests/fixtures/m28-linux/cli-library \
  --target x86_64-unknown-linux-musl --work-dir target/m28-fixtures/musl
```

## ELF 程序与独立链接

准备两套 unwind prefix 后，可直接构建本阶段的分配、移动 GC 与异常程序：

```sh
target/debug/scoop build tests/fixtures/m28-linux/cli-program/program \
  --target x86_64-unknown-linux-gnu -o target/program-gnu
target/debug/scoop build tests/fixtures/m28-linux/cli-program/program \
  --target x86_64-unknown-linux-musl -o target/program-musl
target/debug/scoop build tests/fixtures/m28-linux/cli-program/program \
  --target x86_64-unknown-linux-musl --link-mode dynamic -o target/program-musl-pie
SCOOP_GC_STRESS_MOVE=1 target/program-gnu
SCOOP_GC_STRESS_MOVE=1 target/program-musl
SCOOP_GC_STRESS_MOVE=1 target/program-musl-pie
```

gnu 默认是 PIE；musl 默认是无动态依赖的静态 executable，显式 dynamic 模式使用
musl loader。两种 libc 的 `.slib` 和 runtime index 分开保存，不能交叉消费。
程序 fixture 包含 debug/release 构建、运行及 `scoop-link` 对既有 `.slib` 和 runtime
index 的重链接；重链接进程不需要 LLVM 开发环境：

```sh
python3 tests/run_fixtures.py --suite tests/fixtures/m28-linux/cli-program \
  --target x86_64-unknown-linux-gnu --work-dir target/m28-program/gnu
python3 tests/run_fixtures.py --suite tests/fixtures/m28-linux/cli-program \
  --target x86_64-unknown-linux-musl --work-dir target/m28-program/musl
```

动态原生库的组合验证使用同一 runner：

```sh
python3 tests/run_fixtures.py --suite tests/fixtures/m28-linux/native-dso \
  --target x86_64-unknown-linux-gnu --work-dir target/m28-native/gnu
python3 tests/run_fixtures.py --suite tests/fixtures/m28-linux/native-dso \
  --target x86_64-unknown-linux-musl --work-dir target/m28-native/musl
```

该用例显式选择 dynamic 模式，覆盖 SONAME、无 SONAME 库、版本化依赖、
constructor、动态 TLS、DSO 调用静态 archive、重链接及错误版本不覆盖旧输出。
Linux 的同名导出遵循 ELF 平坦命名空间；不能同时实现的显式库绑定会报符号冲突。

`tests/fixtures/m28-linux/deep-frames` 覆盖 2,048 层递归中的移动 GC、间接调用、
零尺寸值、16 字节对齐的大聚合和溢出到栈的标量参数，以及 600 层异常 cleanup。
同一 runner 可分别选择两种 libc；musl 另有显式 PIE 变体。

大量 fixture 可先用 `cargo build --release -p scoop -p scoopc -p scoop-linker --bins`
构建优化版工具，再给 runner 传入 `--scoop target/release/scoop`
`--scoopc target/release/scoopc --scoop-link target/release/scoop-link`。
这只优化 Rust 编译器工具自身，与被测 Scoop 程序的 debug/release 选择分开。
更广的功能组合及 macOS 回归状态见 [实施记录](PROGRESS.md)。

## 构建目录管理

开发验证可以设置 `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`，减少大型 workspace 的中间产物。使用同一组设置完成一批验证，避免生成多套重复缓存。

定期查看 `du -sh target`。unwind 安装和真实展开测试通过后，可以删除 `target/llvm-unwind` 的构建中间文件，保留 `sysroot/native` 中的 headers/archive；不得清理仍在运行的构建或测试目录。最终 macOS/AArch64 回归在 `m3u.0d0a.com:~/repos/scoop` 中进行，临时目录也须位于该仓库下。
