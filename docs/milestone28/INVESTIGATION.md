# M28 调研与工具链探针

日期：2026-10-05。对应 [设计文档](DESIGN.md)。本记录区分当前代码事实、已经执行的探针和后续验收要求，不是 M28 完成记录。

## 1. 本机环境

| 项目 | 实际观察 |
| --- | --- |
| host | Linux x86_64，glibc 2.43 |
| Scoop LLVM 后端 | `/usr/lib/llvm-22/bin/llvm-config`、`llc` 报告 22.1.2 |
| PATH 中的 LLVM | Homebrew `llvm-config`、`clang`、`clang++` 为 21.1.8 |
| Clang 22 | `/usr/lib/llvm-22/bin/clang` 不存在；构建 Scoop 后端不要求它存在 |
| C 工具链 | `/usr/bin/gcc` 15.2.0，GNU ld 2.46 |
| musl driver | `/usr/bin/musl-gcc`，调用 GCC 并加载 `/usr/lib/x86_64-linux-musl/musl-gcc.specs` |
| musl 开发文件 | `/usr/include/x86_64-linux-musl`、`/usr/lib/x86_64-linux-musl` |
| musl loader | `/lib/ld-musl-x86_64.so.1` |
| LLVM 源码 | `../llvm-project-22.1.2.src`，版本文件确认 22.1.2，包含 `libunwind`、`runtimes` 和 CMake 支持文件 |
| 独立 libunwind | `../libunwind`，版本 `1.9.0-pre`，commit `d9e7b9a20a00d2c86b37981b7368469d18941b31` |

编译 Scoop 时应显式设置：

```sh
export LLVM_SYS_221_PREFIX=/usr/lib/llvm-22
```

`musl-gcc -dumpmachine` 在本机输出 `x86_64-linux-gnu`；`-print-file-name=libc.a` 也不能代替实际链接输入检查。验证 libc 选择要查看 wrapper/specs、真实 headers、CRT 和最终 interpreter/dependencies。

glibc 最低部署版本由目标开发环境决定，本机的成功不代表已验证旧 glibc 发行版。此阶段先定义工具链输入和可用平台，不把本机 2.43 无意写成语言 ABI 的最低版本。

## 2. 两个 libunwind 的选择

| 比较项 | LLVM libunwind | 独立 libunwind |
| --- | --- | --- |
| Scoop 所需的 `_Unwind_*` | 提供；配合 Scoop 自己的 personality | 提供；当前源码需开启相应构建选项 |
| Linux amd64、后续 arm64 | 在官方 DWARF 支持范围内 | 同样支持 |
| 远程进程/core dump/较广架构覆盖 | 不是当前选择它的理由 | 更适合此类调试和额外平台需求 |
| 当前源码构建系统 | CMake，通过 `runtimes` 只启用 libunwind | README 推荐 Meson；Autotools 标为 deprecated；该源码的 CMake 主要面向 Visual Studio |
| 本机 glibc/musl 静态库 | 两套均已从原始源码构建成功 | glibc 可构建，当前 musl Meson 路径需要构建脚本修正 |

选择 LLVM 版本的主要依据是需求匹配和本次实际构建结果，而不是“同名库与 LLVM 后端天然绑定”。双方 Level I API 都可以承担 Scoop 的展开；不把它们各自低层 `unw_*` context/cursor 布局视为可互换 ABI。

LLVM 文档列出 Linux x86_64/ARM64 DWARF 支持，并说明实现主要提供 local unwinding；它的 CMake 对 C++ 源文件使用 `-fno-exceptions`/`-fno-rtti`，避免依赖 C++ 标准库。[LLVM 22.1.2 概述](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.2/libunwind/docs/index.rst)、[构建配置](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.2/libunwind/CMakeLists.txt)。

M28 固定使用给定的 22.1.2 源码便于复现；未来可以独立升级 unwinder，但要重新验证实际 EH/链接行为。Darwin 保持系统 libSystem provider，本阶段不替换它。无需为两个 Linux unwinder 同时建立选择框架。

### 独立项目的已知本机问题

在原始 `../libunwind` 上使用 Meson 1.12.1、静态库和 `-Dcxx_exceptions=enabled`：

- glibc `unwind` target 编译成功，但即使关闭 `minidebuginfo/zlibdebuginfo`，该 commit 的无条件 `dependency()` 探测仍会找到压缩库；实际链接探针带入了 liblzma。
- musl configure 在 `_Unwind_Resume` 检测处失败，系统 GCC 的 `libgcc_eh.a` 引用 musl 不提供的 `_dl_find_object`。
- 后面的 `getcontext` 探测还会要求额外 libucontext，而 x86_64 local implementation 已有自己的汇编 context 保存入口。

在临时源码副本中去掉这些不适用于该 local x86_64 构建的依赖后，musl 静态库及最小 catch 探针成功。这只能证明有可行的修正方向，不表示原始源码无需补丁；没有修改用户的 `../libunwind`。切换到 LLVM 方案后，不再把这些补丁作为 M28 的交付前置条件。

## 3. LLVM libunwind 的可复现构建

已执行的构建使用原始 `../llvm-project-22.1.2.src`、Homebrew Clang/Clang++ 21.1.8、CMake 和 Unix Makefiles。两套 `unwind` target 均成功，无需 Clang 22、LLD 或 C++ runtime 库。

下面从 Scoop 仓库根目录运行，以新临时目录保存两套构建。musl 的系统 include 路径是本机布局，移到另一台机器时按其安装位置调整；不要把它们硬编码进编译器。

```bash
M28_SOURCE="$(realpath ../llvm-project-22.1.2.src)"
M28_BUILD="$(mktemp -d /tmp/scoop-m28-llvm-unwind.XXXXXX)"
M28_CC="$(command -v clang)"
M28_CXX="$(command -v clang++)"
M28_RESOURCE="$("$M28_CC" -print-resource-dir)"

M28_COMMON=(
  -G "Unix Makefiles"
  -S "$M28_SOURCE/runtimes"
  -DLLVM_ENABLE_RUNTIMES=libunwind
  -DCMAKE_BUILD_TYPE=Release
  -DLLVM_INCLUDE_TESTS=OFF
  -DLLVM_INCLUDE_DOCS=OFF
  -DLIBUNWIND_INCLUDE_TESTS=OFF
  -DLIBUNWIND_INCLUDE_DOCS=OFF
  -DLIBUNWIND_ENABLE_SHARED=OFF
  -DLIBUNWIND_ENABLE_STATIC=ON
  -DLIBUNWIND_ENABLE_THREADS=ON
  -DLIBUNWIND_ENABLE_CROSS_UNWINDING=OFF
  -DLIBUNWIND_HIDE_SYMBOLS=ON
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON
  -DCMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY
  "-DCMAKE_C_COMPILER=$M28_CC"
  "-DCMAKE_CXX_COMPILER=$M28_CXX"
  "-DCMAKE_ASM_COMPILER=$M28_CC"
)

for M28_LIBC in gnu musl; do
  M28_EXTRA=()
  if [[ "$M28_LIBC" == musl ]]; then
    M28_INCLUDES="-nostdinc -isystem $M28_RESOURCE/include -isystem /usr/include/x86_64-linux-musl"
    M28_EXTRA+=(
      "-DCMAKE_C_FLAGS=$M28_INCLUDES"
      "-DCMAKE_CXX_FLAGS=$M28_INCLUDES -nostdinc++"
      "-DCMAKE_ASM_FLAGS=$M28_INCLUDES"
    )
  fi
  cmake "${M28_COMMON[@]}" "${M28_EXTRA[@]}" \
    -B "$M28_BUILD/$M28_LIBC" \
    "-DCMAKE_INSTALL_PREFIX=$M28_BUILD/install-$M28_LIBC" \
    "-DCMAKE_C_COMPILER_TARGET=x86_64-unknown-linux-$M28_LIBC" \
    "-DCMAKE_CXX_COMPILER_TARGET=x86_64-unknown-linux-$M28_LIBC" \
    "-DCMAKE_ASM_COMPILER_TARGET=x86_64-unknown-linux-$M28_LIBC"
  cmake --build "$M28_BUILD/$M28_LIBC" --target unwind -j 4
done
```

实际产物为各 build 的 `lib/libunwind.a`。需要安装 headers/archive 时，用对应 build 的 `install-unwind` target 安装到已指定的私有 prefix；不要覆盖系统 unwinder。此入口与上游的 [runtimes 构建说明](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.2/libunwind/docs/BuildingLibunwind.rst) 一致。

`CMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY` 避免 bootstrap 时依赖尚未构建的 unwind runtime，但也意味着部分 configure 检测不证明最终可链接；必须继续做下一节的目标 libc 链接运行。musl 编译出现上游把 `-nostdinc++` 传给部分 C/ASM 命令的 unused-argument warning，构建成功；没有通过引入 glibc headers 修补 musl 构建。

## 4. 异常展开探针

用 `/usr/lib/llvm-22/bin/llc` 为 gnu/musl 生成 ELF：外层 catch-all，中间函数包含 catch-all/cleanup 两类 landingpad，实际异常路径执行 cleanup 后 `resume`，外层捕获并调用 `_Unwind_DeleteException`。C harness 直接创建带 Scoop exception class 的 native unwind record；personality 直接编译仓库现有的 `runtime/src/eh_personality.c`，没有修改它。

链接使用目标 GCC/musl-gcc、LLVM libunwind 自带的 `include/unwind.h` 和对应静态 archive，显式 `-nodefaultlibs`，随后提供 `-lc -lgcc`；保留 `-pthread`、`-Wl,--eh-frame-hdr`。这是 Level I/personality 探针，不运行完整 Scoop 异常对象生命周期或 GC。

| 配置 | 实际运行 | 最终动态依赖 |
| --- | --- | --- |
| glibc，`-fPIE -pie` | 输出 `cleanup`、`caught`，退出 0 | 仅 `libc.so.6` |
| musl，`-static -fno-pie -no-pie` | 同上 | 无 `DT_NEEDED` |
| musl，`-fPIE -pie` | 同上 | 仅 `libc.so` |

三套 link map 均未引入 `libgcc_eh` 或 `libgcc_s`，没有 libc++/libstdc++/libc++abi 依赖。PIE 的 CRT 可出现 weak `__cxa_finalize`；这不是 C++ EH 调用。后续应保留输入归属和真实符号类别，不按 `__cxa_` 前缀误拒绝正常 CRT。

### cleanup-only 的独立发现

最初的中间函数只有 cleanup landingpad，没有 catch-all。LLVM 22.1.2 为其 `.gcc_except_table` 发出 `ff ff 01 ...`，即省略 type table；现有 personality 固定要求 `0x9b`，因此三套配置均报告：

```text
unsupported LSDA (unsupported type-table encoding)
```

保留 catch-all 与 cleanup 的现有封闭编码形态后，三套探针通过。这说明 unwinder 的选择不是该失败的原因。主设计要求在实现时补齐真正 cleanup-only 的省略 type-table 分支，并同步 spec/codegen object decoder/runtime decoder；不能把通过的混合探针写成“所有异常形态已经通过”。

## 5. LLVM amd64 statepoint 与 ABI 探针

使用显式 statepoint/relocate 的最小 LLVM 函数，一条 managed root 跨越 call，设置全部 frame pointer、禁 tail call 和 `noredzone`。两个 Linux triple 在 `llc -O0`、`-O2` 下都成功发射 ELF64-x86-64。

`llvm-readobj --stackmap` 的关键输出一致：

```text
LLVM StackMap Version: 3
stack size: 24
Record ID: 7, instruction offset: 17
Indirect [R#7 + 8], size: 8
Indirect [R#7 + 8], size: 8
```

对应 prologue 为 `push rbp; mov rsp, rbp; sub 16, rsp`。因此 `RBP=callsite_RSP+stack_size-8`；callsite 的精确返回位置是 17，而不是 AArch64 的固定 4 字节对齐位置。函数地址 relocation 为 `R_X86_64_64 f + 0`，call relocation 为 `R_X86_64_PLT32`，EH frame 使用其正常 PC-relative relocation。

另一个 `sret({i64,i64})` + `byval({i64,i64})` + scalar 参数函数观察到：返回地址从 RDI 接收并放回 RAX；byval 副本从 `[RBP+16]` 读取，scalar 使用 RSI。这个差异直接影响 Scoop ABI native shim，不能仅把 Darwin 参数寄存器名字换成 amd64 名字。

本探针只证明最小 LLVM 形态。完整 Scoop 的 root plan、高压力、聚合值、indirect call、invoke 和跨线程 relocation 仍是 M28 正式测试。

## 6. Linux OS 与 ELF 只读 metadata 探针

### 6.1 线程与 unwind 基础设施

glibc PIE、musl PIE 和 musl static 的小型 C 程序均能调用 `pthread_getattr_np`、`pthread_attr_getstack`、`dl_iterate_phdr`。本机 page size 为 4096，native `_Unwind_Exception` 探针显示 size/alignment 为 32/16；这些实测值不能代替目标 profile 的实际 ABI 检查。

主线程递归约 80 层、每层触碰 4096 字节数组的另一个探针得到：

| libc | 初始 attr stack size | 深调用时 | 当前 frame 是否低于初始 low |
| --- | --- | --- | --- |
| glibc | 8376320 | 8376320 | 否 |
| musl | 135168 | 339968 | 是 |

具体字节数随环境变化；要保留的是“musl 的初始栈范围会过时”这一事实。musl 上游 `pthread_getattr_np` 对主线程映射的处理也与该观察一致。[musl 实现](https://git.musl-libc.org/cgit/musl/tree/src/thread/pthread_getattr_np.c)。设计采用发布 anchor/boundary 时的范围快速检查及必要时刷新，不对每个 GC root 做系统查询。

### 6.2 stackmap 权限

将真实 LLVM stackmap 对象的输入段设为可重定位数据，用以下最小脚本定义边界并放入动态 RELRO 布局：

```ld
SECTIONS {
  .scoop_stackmaps : ALIGN(8) {
    HIDDEN(__scoop_stackmaps_start = .);
    KEEP(*(.llvm_stackmaps))
    HIDDEN(__scoop_stackmaps_end = .);
  }
} INSERT BEFORE .data.rel.ro;
```

探针使用 `llvm-objcopy --set-section-flags .llvm_stackmaps=alloc,load,data` 准备输入，链接参数含 `-z relro -z now -z text`。生产实现应在既有 ELF 对象物化逻辑内设置属性，不因此要求 artifact-only link 安装 LLVM tools。

通过 `/proc/self/maps` 检查实际包含 stackmap 的映射：

| 模式 | stackmap 长度 | 实际权限 |
| --- | --- | --- |
| glibc PIE | 128 bytes | `r--p` |
| musl PIE | 128 bytes | `r--p` |
| musl static，仍只依赖 RELRO 布局 | 128 bytes | **`rw-p`** |

静态 musl 改为以下输出规则后得到 `r--p`，ELF section flags 为 `A`，没有 `W`：

```ld
SECTIONS {
  .scoop_stackmaps (READONLY) : ALIGN(8) {
    HIDDEN(__scoop_stackmaps_start = .);
    KEEP(*(.llvm_stackmaps))
    HIDDEN(__scoop_stackmaps_end = .);
  }
} INSERT BEFORE .eh_frame;
```

静态链接已经完成函数地址 relocation，因此可直接由只读 LOAD segment 承载。这是两种最终链接脚本需要区分的实际原因。正式脚本还要包括全部 Scoop 不可变记录、ODR section 和 startup image array；本次没有验证完整程序布局。[GNU ld READONLY 输出类型](https://sourceware.org/binutils/docs/ld/Output-Section-Type.html)。

## 7. 尚未完成的验证

本次没有修改编译器/runtime 实现，也没有运行完整 workspace 或 fixture 验收。当前正式 CLI 和 fixture runner 仍只接受 Darwin/AArch64；Linux 的 target/object/link/runtime 路径尚待实施。

还必须完成：cleanup-only decoder、完整 moving GC、ODR/COMDAT 及重复 stackmap、TLS、C bridge 和所有 native shim、多 Cone artifact-only 链接、初始化/协程/Context/callback/release 组合，以及真实 Darwin 回归。不能以这些小型探针替代第 9 节的完成条件。

实验构建和 harness 均放在临时目录，LLVM 与独立 libunwind 源码未改动，未安装或替换系统 unwinder。仓库交付只有本目录的设计与调研文档。
