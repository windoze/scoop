# M28：Linux glibc / musl amd64 支持

状态：实施中，完成情况见 [实施记录](PROGRESS.md)。调研日期：2026-10-05。

本阶段新增 `x86_64-unknown-linux-gnu` 和 `x86_64-unknown-linux-musl`，保持现有 Darwin/AArch64。`amd64` 与 `x86_64` 在本文中指同一架构；CLI/LLVM canonical triple 统一使用 `x86_64`。profile name 遵循现有不含下划线的标识符规则，使用 `x86-64`；后续 Linux arm64 的 canonical triple 使用 `aarch64`。

建议采用 **LLVM 项目的 libunwind 22.1.2**，源码取自 `../llvm-project-22.1.2.src`。Scoop 只消费 Itanium Level I `_Unwind_*` 接口，继续使用 M25 的异常记录和 personality。独立项目 `../libunwind` 的额外架构、远程展开及调试能力不进入本阶段。

本设计的依据包括三份规范、现有 target/toolchain/runtime/linker 实现和本机小型编译运行探针。环境、命令、实测结论及其局限见 [调研记录](INVESTIGATION.md)。这些探针不等同于 Scoop 已完成 Linux 支持。

当前可用的工具链准备命令见 [构建说明](BUILDING.md)。

## 1. 交付范围

### 1.1 支持矩阵

| target | 本阶段的输出 | 用途 |
| --- | --- | --- |
| `aarch64-apple-darwin` | 保持现有 Mach-O 行为 | 原平台回归 |
| `x86_64-unknown-linux-gnu` | 动态链接 libc 的 ELF PIE | Linux glibc 默认模式 |
| `x86_64-unknown-linux-musl` | 静态 ELF executable，默认模式 | 无目标 libc 动态加载器依赖的程序 |
| 同上，显式选择动态链接 | ELF PIE，使用 musl loader | 原生共享库及动态 TLS 的正式覆盖 |

静态/动态是 final-link 配置，不是新的源码语言模式，也不各建一套 `.slib` target。Linux Scoop 对象及 runtime 对象按可用于这两种最终链接的 PIC 配置生成；具体缓存是否能共用取决于实际 flags 和输入。

交付包含正式 `scoop build/run`、single-file、single-Cone `scoopc`、多 Cone `.slib` 生产与消费、artifact-only link，以及现有语言能力的组合运行。GC、异常、FFI、callback、协程、初始化、release hook 和 M27 Context 都必须可用。

本阶段不增加 Linux arm64 执行支持、32 位/x32 ABI、Windows、Scoop DSO/动态加载、JIT、远程栈展开或通用交叉工具链发行系统。glibc 全静态及 musl static PIE 不作为完成条件。编译器自身可以是链接系统 LLVM 的 glibc Rust 程序；生成 musl 程序不要求先把编译器和 LLVM 编成 musl。

### 1.2 设计约束

沿用现有 typed identity、IR、GC 和异常契约。新增抽象必须同时服务现有 Darwin 和新增 Linux 的实际差异；不新增 target 插件系统、来源授权、产物证明链或通用预算。必要检查分别留在源码/IR、对象读取、最终链接和已加载内存边界，同一未变化事实不逐阶段重放。

本文件描述 M28 的目标行为。各功能先按第 10 节修订对应规范，再修改实现；实施记录区分已验证能力与尚待完成的工作。

## 2. 实施起点的差距

下表记录 M27 结束时的移植起点；M28 当前完成情况见实施记录。

| 位置 | M27 固定假设 | M28 所需变化 |
| --- | --- | --- |
| `compiler/toolchain/src/registry.rs` | 唯一 Darwin/AArch64，host 仅检查 arch/OS | 增加两个 Linux target；明确 libc，组成完整的各阶段配置 |
| `compiler/lir/src/target*.rs`、`target/contract.rs` | 唯一布局、Mach-O 名字前缀 | 增加 amd64 布局和 ELF 符号规则，保留完整 typed target |
| `compiler/lir/src/c_bridge_{toolchain,invocation}.rs`、toolchain | Apple Clang、SDK、deployment 参数 | 区分 driver、libc 开发文件及平台特有部署事实 |
| `compiler/codegen/src/target.rs`、`artifact/` | AArch64 后端、指令、帧和 Mach-O | X86 后端、amd64 帧契约、ELF/EH 对象处理 |
| codegen 的 metadata、atom boundary、TLS 发射 | Darwin section、前置 `_`、TLV descriptor | ELF section/symbol/COMDAT/TLS 表达 |
| `compiler/slib/src/link_object/` | 大量 reader/finalizer 直接使用 Mach-O 类型 | 复用共有语义逻辑，分离对象格式及架构 relocation 读取 |
| `compiler/linker/src/` | Apple ld、TBD、dyld ordinal、Mach-O fixup/map | ELF 原生输入、CRT、系统库、链接动作及最终 ELF 事实 |
| `runtime/src/platform/` | 已有 image、OS/VM、frame 三层，但只有 Darwin 实现 | ELF image、Linux OS/VM、amd64 frame/入口汇编 |
| `compiler/scoop/src/runtime_build/` | SDK/Clang resource 和 Darwin 源文件 | 目标 libc 的编译输入、依赖及独立 runtime 缓存 |
| Rust 集成测试、`tests/fixture_runner/cli.py` | host gate、xcrun、Darwin flags/golden | 显式 target 测试矩阵和对应 native 工具 |

尤其不能仅打开 Linux target gate。现有 ELF 不可用的地方还包括 startup C 的 `__DATA_CONST`、`_main`、native library 文件后缀、String 间接返回 shim、CommonCrypto SHA-256 和 runtime 注册时保存的线程栈边界。

## 3. 平台结构

### 3.1 分解职责，保留封闭的支持列表

继续由 `scoop-toolchain` 编排完整 target，stage 只接收其输入 IR 中的必要投影，不依赖上游实现 crate。使用已有模块增加以下有实际用途的区分，不新建平行 registry：

| 维度 | 负责的事实 | 后续 Linux arm64 的改动 |
| --- | --- | --- |
| 架构 / 过程 ABI | LLVM backend、data layout、调用参数、frame、DWARF 寄存器、机器 relocation | 增加/复用 AArch64 对应实现 |
| OS | 线程栈、VM、loader 运行环境 | 复用 Linux 实现 |
| libc 环境 | glibc/musl headers、CRT、库、loader、unwind 编译输入 | 为 arm64 提供对应路径和产物 |
| 对象格式 | ELF/Mach-O section、symbol、relocation 表、COMDAT、TLS 表示 | 复用 ELF，选择 AArch64 relocation 模块 |
| 最终链接模式 | PIE/静态、链接脚本、系统库、实际 linker 参数 | 复用 Linux 链接编排 |

这些维度用于内部组合，不允许用户任意拼出一个“看似支持”的 target。registry 只返回已实现的三个平台组合；Linux AArch64 请求仍给出明确的 target 不支持诊断。

推荐在现有 crate 内形成如下边界，按实际文件规模拆分：

```text
toolchain: registry + c_driver + linux/darwin tool discovery + final_link
codegen:   target + object/{elf,macho} + arch/{x86_64,aarch64}
slib:      shared link-object records + object/{elf,macho} + relocation/{...}
linker:    shared program/native plan + elf/macho input and output handling
runtime:   platform/image/{elf,macho}.c
           platform/os/{linux,darwin}.c
           platform/arch/{x86_64,aarch64} frame and entry implementations
           platform/profiles/ complete bundle selection
```

`runtime_sources` 拆为共有源文件清单和实际 platform 组件清单。glibc/musl 共用 Linux/amd64 runtime 源码，区别主要由编译输入和最终链接表达。通用 collector、thread、callback 和 Context 不散布 arch/libc 条件编译。

现有 `scoop_darwin_aarch64_managed_frame_ops` 等名称在确有跨 OS 复用时改为相应架构/ABI 名称；Darwin 与 ELF 的汇编符号拼写留在格式适配处。不能假定 Darwin AArch64 的每个过程 ABI 细节都等同于 Linux AAPCS64。

### 3.2 target 和名字

- canonical triple 使用表 1 中的三个完整拼写；Linux `gnu`/`musl` 不省略。可在 CLI 输入处接受 `amd64` alias，但 canonical 化后不传播 alias。
- Linux 默认 target 根据编译器的 host triple/libc 环境选择；不能只看 `std::env::consts::{ARCH,OS}`。显式 `--target` 优先。
- `musl-gcc -dumpmachine` 在本机仍返回 `x86_64-linux-gnu`，不能据此否定其 musl 配置。driver 选择必须结合明确配置、实际 headers/链接输入及小型输出探针。
- 新增独立的 Linux glibc/musl typed target profile id：`linux-x86-64-gnu/1`、`linux-x86-64-musl/1`。即使部分布局相同，也不互用 `.slib`、core、runtime 或 native bridge 缓存。
- ELF logical symbol 到 symbol-table bytes 使用恒等映射，保留现有非空/NUL/LLVM escape 的输入约束；不加 `_`，不折叠已有 `_`。C `asm` label、startup、native requirement、runtime alias 和对象 reader 使用同一规则。

## 4. amd64 ABI 与 LLVM 后端

### 4.1 布局与 Scoop ABI

两个 Linux target 都采用 little-endian LP64、64 位 data/code/managed pointer、16 字节调用栈对齐，保持现有整数、ZST、String、对象头及最大显式 16 字节对齐规则。LLVM data layout 从 22.1 TargetMachine 实际取得并与声明的 target 契约匹配，不能复制 Darwin 的 `m:o` layout。

继续使用已有 Scoop ABI 分类：scalar/ref/raw/code pointer/niche 值直接传递；非空 aggregate 使用现有 typed 间接 ABI；ZST 消去 payload。**LLVM 的 `byval` 参数不是“机器上传一个 C 指针”的承诺。**

本机 22.1.2 探针观察到：amd64 `sret` 地址使用 RDI，后续 scalar 参数顺移；一个 `byval({i64,i64})` 参数从 callee 的 `[RBP+16]` 开始读取，实际副本位于栈上传参区。现有调用者、定义、dispatch 和 statepoint 的 `sret`/`byval`/`align` 属性必须一致。Scoop native shim 必须按具体物理签名适配，不能把普通 C struct 按值传参或显式 pointer helper 直接冒充 Scoop ABI。

`scoop_rt_string_get` 和 `scoop_rt_string_slice_bounds` 的 amd64 shim 按 RDI 间接返回地址及后续参数调用现有 storage helper，并按 SysV `sret` 规则保留返回地址结果。不要复用 Darwin 将 x8 搬到 x0 的汇编。C ABI 调用继续通过 generated-C bridge，让目标 C compiler 处理 SysV aggregate 分类、packing 和返回规则。

### 4.2 后端配置

显式初始化 LLVM X86 backend，使用通用 x86-64 CPU 基线，不采用本机 `native`/AVX 特性。LLVM 后端仍固定 major/minor 22.1，C compiler/linker 不必与该版本号一致。

managed code 保持 statepoint v3、AS1、全部 frame pointer、禁 managed tail call；amd64 增加 `noredzone`。runtime C 使用 frame pointer、禁 sibling-call 优化及 unwind tables。不启用会改变 frame/metadata 存活关系的 LTO、ICF 或 section GC。

对象校验复用 stackmap/LSDA 的共有解码，架构模块负责寄存器和帧关系。amd64 指令变长，不能复制 AArch64 的 `return_pc - 4` 或假定所有 call 都长 5 字节。确有必要校验指令边界时，在 codegen 内使用已有 LLVM 的解码能力；最终 artifact-only linker 不因此依赖 LLVM，也不另写通用 x86 反汇编器。

## 5. 精确栈根与 Linux runtime

### 5.1 managed anchor 与 frame

入口汇编在任何 C prologue 前捕获 generated caller：

```text
entry_rsp      = RSP on entry to the runtime stub
return_pc      = *(uintptr_t *)entry_rsp
callsite_sp    = entry_rsp + 8
frame_pointer  = RBP
DWARF RBP      = 6
DWARF RSP      = 7
```

随后把三个值交给现有 signature-matched C implementation。当前 0～3 个整数/pointer 参数的入口加上三个 anchor 参数仍可放进六个 SysV 参数寄存器，能够用薄 tail jump 保留 generated caller 的 return address。按实际入口签名维护清单；若新增参数超出寄存器，显式实现栈参数传递，不能套用会覆盖原栈的宏。assembly 需带正确 CFI、ELF function type/size 和非执行栈声明。

对本阶段固定帧 profile，令 `S=callsite_sp`、`F=RBP`、`N=stackmap.stack_size`：

```text
F = S + N - 8
saved_caller_rbp = *(uintptr_t *)F
caller_return_pc = *(uintptr_t *)(F + 8)
caller_callsite_sp = F + 16
```

探针中 `N=24`、`F=S+16`，GC root 为 `Indirect [DWARF RSP + 8]`。这与 AArch64 的 `F=S+N-16` 不同。amd64 的 N 不必是 16 的倍数；应验证其与调用栈对齐、保存 RBP 和完整帧范围的真实关系。

GC root 仍只允许 8 字节可写 `Indirect [RSP/RBP+offset]`，base/derived 遵守现有同址合同。按架构计算可用 spill 范围，排除保存的 RBP/return address；保留加法溢出、对齐、线程栈及 managed boundary 检查。原始 return PC 必须精确命中 stackmap，不调整 PC、不扫描任意 C frame，不用 libunwind 的普通栈遍历替代 moving GC 的可写 slot 定位。

O0/O2、深调用、高寄存器压力、超过六个参数、间接调用、invoke、16 字节对齐 aggregate 都要验证此帧合同。生成代码不使用动态 alloca；若实际 LLVM 输出出现不同的合法帧形态，先调整该架构合同和 adapter，再开放对应配置。

### 5.2 OS/VM 与可增长的主线程栈

Linux OS 层使用 `pthread_getattr_np` + `pthread_attr_getstack` 获取当前线程范围，销毁临时 attr；VM 使用 `mmap(MAP_PRIVATE|MAP_ANONYMOUS)`、`mprotect` 和实际 `sysconf(_SC_PAGESIZE)`。不硬编码 4 KiB page，也不把偏好 arena 地址改成覆盖已有映射的 `MAP_FIXED`。

**musl 主线程栈不能只在 attach 时查询一次。** 本机递归探针中，musl 初始报告 135168 字节，深调用后报告 339968 字节，当前 frame 已低于第一次记录的 low；glibc 同一测试报告稳定的约 8 MiB 范围。现有 `thread.c` 的一次性快照会误拒绝合法 musl 程序。

在 managed anchor、gateway/callback boundary 及 outbound transition 的发布边界复用同一个栈范围确认函数：地址在缓存范围内走快速路径；低于缓存 low 时，由**当前所属线程**重新查询 OS 范围，确认实际地址后更新。更新与既有 world/park/native-safe 发布协议同步，collector 只读已发布快照；不在每个 root/每帧重新查询，不由 collector 调用 `pthread_self()` 来猜其他线程的栈。异常值仍须诊断，不能直接用传入 SP 扩大合法范围。

### 5.3 已加载 ELF 与 metadata

ELF image adapter 使用 `dl_iterate_phdr` 获取主 executable 的 load bias、`PT_LOAD` 和权限。stackmap 的起止地址由 final link 定义的 hidden linker symbols 直接提供；不依赖运行时存在 ELF section header、磁盘上的 executable 或 `.symtab`。Scoop Cone image 仍是一个最终 executable 内的 logical image，不新增 DSO 注册协议。

动态 PIE 下，含 relocation 的不可变 metadata 在 loader 修正后必须只读；Linux adapter 用加载段和实际 VM 映射权限核对范围，不能只凭 section 名或 `PT_GNU_RELRO` 宣称只读。Linux 的 `/proc/self/maps` 查询只发生于已有启动 image 检查边界，不进入 GC 热路径；缺少所需映射信息须明确诊断，作为本阶段 Linux 运行环境要求记录。

stackmap parser 继续支持同一输出段中串接的完整 v3 blob，保留精确 PC 和登记关系检查。现有 CommonCrypto SHA-256 调用需提供可移植实现；优先采用维护中的上游 C SHA-256 模块，例如 [Mbed TLS 的 SHA-256 模块](https://github.com/Mbed-TLS/mbedtls/blob/mbedtls-3.6/library/sha256.c)及其最小配置，固定版本并保留许可证，只编入所需源文件。它仅服务已有 canonical stackmap fingerprint，不引入完整 TLS 库、新摘要协议或额外来源检查。

## 6. ELF 发射、ODR 和 TLS

### 6.1 section 与最终权限

| 内容 | ELF 输入表达 | 最终要求 |
| --- | --- | --- |
| 代码 | `.text` / function sections | RX |
| 不含 pointer relocation 的只读常量 | `.rodata` / 适用的字符串 section | R |
| TD、scan、registration、image、startup image array 等不可变记录 | 专用 `.data.rel.ro.scoop.*` | loader/静态链接完成后 R |
| stackmap | `.llvm_stackmaps`，正确设置可重定位数据属性 | 最终 R，完整保留 |
| 可写 cell、ordinary static storage、failure roots | `.data` / `.bss` | RW |
| TLS template | `.tdata` / `.tbss`，TLS symbol | 由目标 TLS 模型解释 |

LLVM 原始 stackmap 含函数地址 relocation。PIE 不能把它作为只读输入直接留下 text relocation：在已有对象物化步骤修正 ELF section flags，再由小型 linker script 汇入 RELRO 范围；保留 `KEEP` 和明确起止符号，禁止 `DT_TEXTREL`。本机已验证此方式在 glibc/musl PIE 下得到只读映射。

静态 musl 另用输出布局：将已由静态 linker 完成 relocation 的 Scoop 不可变记录和 stackmap 放入 `READONLY` 输出 section，落在 R 的 `PT_LOAD`。**静态 musl 上单独发出 `PT_GNU_RELRO` 不够**，本机对应区域仍可写；改用只读输出 section 的探针已通过。不为此在 runtime 启动后任意保护整段 `.data`。

动态/静态脚本只收集明确的 Scoop metadata section，不改变可写初始化 cell、GC root storage 或 libc 私有数据的权限。脚本和属性处理属于 ELF/链接模式，不包含 amd64 指令知识，后续 arm64 复用。

### 6.2 `.slib` 与对象处理

复用现有 `object` crate 读取 ELF64 little-endian `ET_REL`，验证 `EM_X86_64`、section/symbol/relocation 边界和所需 ABI。共有 definition/registration/native requirement 逻辑不再要求参数必须是 `ValidatedDarwinArm64ObjectEnvelopeV1`。

对象格式负责 section、symbol size/binding/visibility、group 和 RELA 表；架构模块解释具体 relocation，包括符号目标、signed addend、写入宽度及 PC-relative 语义。覆盖实际 LLVM 和所选 C compiler 发出的绝对、PC-relative、PLT/GOT 与 TLS relocation；不把所有 relocation 当 64-bit pointer，不给 debug section 套用 loadable metadata 规则。

ELF atom boundary 优先使用真实 symbol/section extent 和现有 typed definition，必要边界标签由 ELF 发射模块提供。不要复制 Mach-O 的 nlist 重写、underscore 拼接及固定 TLV extent。对象物化、指纹补丁之后按最终字节检查一次相关对象事实；后续 consumer 复用读取结果。

只读数据 section 可以在第一个 atom 之前包含 canonical zero 字节，供共享空 template/relocation 哨兵使用；不能假定 LLVM 总把哨兵排在已命名 atom 后面。前缀逐字节检查为零，不能包含 relocation 来源；普通 atom 仍不重叠，哨兵引用仍按实际范围、对齐和非 atom 区域验证。可写数据与代码 section 继续要求从已归属 atom 开始。

### 6.3 ODR

保持 M23 的 exact type/body/storage identity 和已有 ODR 内容一致性语义。ELF 使用 `weak_odr` 和按实际 definition member 组织的 COMDAT；不新增 ODR 身份，也不假定一个跨对象语义 group 必须变成一个跨对象物理 COMDAT。

同一 member 的 associated atoms 必须随同一选中定义保留。callable 的 stackmap contribution 可继续作为完整 blob 保留，但函数地址 relocation 必须引用可合并的 canonical body symbol，不能引用被 COMDAT 丢弃的局部 section 地址。LLVM 22.1.2 的普通函数探针已观察到 `R_X86_64_64 f`；ODR 情况仍须专门验证。重复 raw records 只有在既有完整 identity、最终 PC 和 normalized payload 全等条件下去重。

Scoop 内部及跨 Cone 定义在最终 executable 中防止非预期 ELF symbol preemption，采用与静态消费相容的 hidden linkage。保留 TD、storage 和 callable 的地址身份，不启用 `unnamed_addr`/ICF 等可能合并不同实体的配置。native extern 按正常 ELF ABI 解析，不给所有外来符号强加 hidden。

### 6.4 TLS

`@ThreadLocal` 和 runtime `_Thread_local` 采用 LLVM/C compiler 的 ELF TLS。静态最终程序可由 linker 做对应 TLS relaxation；动态输入按其实际 TLS relocation 和 `STT_TLS` 表达。

ELF 没有 Darwin 的 24 字节 TLV descriptor，不能复用 `storage/tls.rs` 的 extent 和 `$tlv$init` 名字。TLS symbol 的值也不能当作普通已加载虚拟地址。raw TLS 保持 GC-free 规则，测试覆盖 Scoop 定义、extern C TLS、多个 OS thread、callback，以及两种 libc。

## 7. 工具链、libunwind 与最终链接

### 7.1 host 工具与目标 libc 分开

`LLVM_SYS_221_PREFIX=/usr/lib/llvm-22` 选择本机 22.1.2 后端；当前 PATH 中的 `llvm-config`/Clang 是 21.1.8，不能让 LLVM crate 自动选错版本。C compiler 的版本则独立：当前 GCC 15.2 和 Clang 21.1.8 均可作为待验收的 C/native 工具。

第一版正式 Linux C driver 建议以本机 GCC / musl-gcc 加 GNU ld 的组合落地，避免依赖尚未安装的 Clang 22 或 LLD；后端仍是 LLVM 22.1。generated-C 的现有 clang/SDK 调用封装改成封闭 driver 分支，GCC 不接收 Clang 的 `-target`、Darwin `-isysroot` 或 deployment flags。LLVM libunwind 的构建可单独使用支持目标 headers 的 Clang/Clang++。

复用 `--target`，新增以下普通工具链参数，并在第一批同步 CLI/protocol 文档：

| 参数 | 作用及缺省 |
| --- | --- |
| `--cc PATH` | 覆盖目标 C driver；Linux gnu 默认查找 GCC，musl 默认查找 musl-gcc；本阶段明确验收这两类 driver |
| `--native-sysroot PATH` | 覆盖目标 C 开发环境；未给出时使用已选 driver 的明确配置，musl wrapper 可直接使用本机分散安装的 headers/CRT，无需伪造统一 sysroot |
| `--unwind-prefix PATH` | 定位目标匹配的 headers/archive；默认查找 `<Scoop sysroot>/native/<canonical triple>/unwind`，缺失则诊断并提示构建命令 |
| `--link-mode static\|dynamic` | 仅用于最终 executable 链接；gnu 默认 dynamic，musl 默认 static；当前 gnu 的 static 请求明确诊断为不在本阶段支持矩阵中 |

`--sysroot` 继续代表 Scoop core 布局。`scoop` 编排完整配置，向 `scoopc` 只传 C bridge 所需的 driver/native 开发环境，向 program-link 传 startup/final-link 所需配置；纯 `.slib` 编译不要求链接器和 unwind archive。父子进程复用同一解析结果，不能自行发现另一个 libc。artifact-only 路径仍无需加载 LLVM。

解析后保存实际 driver、参数、工具查找路径和开发文件。`musl-gcc` 在本机是 shell wrapper，会调用 `REALGCC`/`x86_64-linux-gnu-gcc` 并读取 `musl-gcc.specs`；现有 `env_clear()` 需要补上已选择的必要执行环境。不能只记录 wrapper bytes 而忽略实际 GCC、specs、headers、CRT 和 libraries，也不能在某项缺失时静默退回 glibc。

### 7.2 选择 LLVM libunwind

LLVM libunwind 的 Linux x86_64/ARM64 DWARF 支持符合 Scoop 当前范围。其 C++ 实现不要求应用链接 libc++、libstdc++ 或 libc++abi；按上游关闭 RTTI/异常和 C++ 标准库依赖的配置构建，仅向 Scoop 提供 `_Unwind_*`。

分别建立 glibc、musl 的独立 build/install prefix，构建 PIC 静态 `libunwind.a`，保留 threads，关闭 cross-unwinding，保持 M25 Scoop personality。独立项目 `../libunwind` 的 `-Dcxx_exceptions=enabled` 选项不适用于 LLVM 项目；LLVM 使用其 `runtimes` CMake 入口，只启用 `libunwind`。

实施时在本机 musl PIE 的完整 backtrace 中复现 LLVM 22.1.2 的 FDE 索引未命中后无边界线性扫描。工具链准备脚本在私有源码副本应用局部补丁：存在搜索表时，其未命中即终止该 image 的查找；不扫描没有零终止的 `.eh_frame` 尾部。此问题与 amd64 的 Scoop 帧合同无关，修复保持公开异常 ABI。原始 LLVM 源码不修改，补丁与复现事实纳入本里程碑构建说明，继续用真实异常与 backtrace 测试验证。

unwind prefix 包含相匹配的 headers 和静态库。runtime 的 `<unwind.h>` 来自该选择；最终链接使用具体归档路径，不能靠无区分的 `-lunwind` 搜索碰巧选到系统另一实现。libunwind 版本是工具链依赖及缓存输入，不与 LLVM backend 的 major/minor 绑定成新的语言 ABI 条件。

避免 C driver 自动引入第二套 unwinder。Linux final-link 的默认库集合显式控制：所选 `libunwind.a` 提供 EH，所选 libc 提供 C/POSIX，确有需要的 `libgcc.a` 或 compiler-rt 只提供编译器算术 builtins；不能把 glibc 的 `libgcc_eh.a`、`libgcc_s.so` 当作 musl EH provider。本机独立 libunwind 的配置已实际遇到 `_dl_find_object` 未定义，表明这种混用不是假想风险。

源代码准备/第三方构建属于开发工具链准备，不是普通 `scoop build` 自动联网下载的步骤。artifact-only link 消费预构建 runtime/unwind，不读取这两个仓库的源码，不调用上游编译 stage。

### 7.3 ELF 程序链接

保留现有程序 identity、依赖拓扑、image/root gateway 和 artifact-only 输入闭包。Linux 路径使用目标 C driver 安排正确的 CRT 和系统 linker；明确 PIE/static、unwind、pthread、metadata script、`--eh-frame-hdr` 和输出路径，不照搬 Darwin `-e _main`/codesign/TBD 参数。

动态模式保留 `.eh_frame`、`.eh_frame_hdr`/`PT_GNU_EH_FRAME`，使用目标 loader，含指针 metadata 正确进入 RELRO。静态 musl 输出无 `PT_INTERP`、无 `DT_NEEDED`，所有符号由所选静态输入闭合；这不意味着程序不再依赖 Linux 内核及本阶段的 `/proc` 运行环境。

LLVM landingpad/LSDA 和 `_Unwind_Exception` 的大小、对齐、exception data registers 按目标验证；不重做 M25 语言异常语义。精确 GC 使用原始 return PC，而 LSDA 内定位 call-site 使用异常 ABI 规定的规则，两者不能混用。必须覆盖 catch、cleanup/resume、rethrow、未捕获异常、跨 Cone 和 callback 边界终止行为。

实测还发现一个需要修订的封闭 LSDA 条件：当前 personality 固定要求 type-table encoding 为 `0x9b`，LLVM 的 cleanup-only 函数正常会输出 `0xff`（省略 type table）。M28 在 spec、对象 decoder 和 runtime decoder 中加入这种实际形态：没有 type table 时只接受对应 cleanup/no-action 记录，不能读不存在的 type-table offset；catch-all 继续按现有 `0x9b` 规则。范围只覆盖 Scoop 使用的 catch/cleanup，不扩展为任意 C++ type/filter decoder。

Linux CRT 可能带有普通的 weak `__cxa_finalize` 引用，探针在 glibc/musl PIE 中均观察到；glibc 的 C `atexit` 实现还会引入 `__cxa_atexit`。这两个退出清理入口不表示 Scoop 使用 C++ 异常 ABI。生成的 Scoop/runtime 对象继续不导入 C++ EH 接口；最终产物检查区分 CRT 正常引用与 `__cxa_throw`、`__cxa_begin_catch`、C++ personality 等异常依赖，不能把 Darwin 的全前缀禁止规则机械套到 Linux CRT。

原生库查找延续显式 search roots 和歧义诊断：Linux 查找 `L.o`、`libL.a`、动态模式下的 `libL.so`；framework/TBD 是 Darwin 专属。用户输入与系统 libc 的 linker script 分开处理：CRT/libc 开发包的标准脚本交由所选 compiler/linker 处理，不为 Scoop 实现通用 linker-script 解释器。

ELF 动态符号按实际 SONAME、`DT_NEEDED`、symbol version 和 TLS 类型处理；不复制 Mach-O two-level namespace/ordinal 规则。普通 C/native DSO 的加载行为沿平台 ABI，Scoop 不承诺防止用户通过 loader 环境改变外部库解析。

显式 `lib` 约束仍须由该库自身提供匹配定义。如果已选 DSO 的同名默认导出使 ELF 链接顺序无法实现这组源声明的库绑定，报告原生符号冲突；不为它生成两级命名空间或运行时符号查找包装。DSO 的实际依赖闭包以普通动态输入交给 linker，并将原库目录写入 RUNPATH；共享库在运行时仍由系统 loader 加载。

最终验证只检查链接产生的新事实：目标/文件类型、loader/库依赖、实际符号闭合和所需地址唯一性、metadata 范围/权限、stackmap/EH 的保留、startup 引用。不重放 HIR/MIR 语义，也不为 ELF 新写一套全程序指令/来源证明系统。保留现有失败不覆盖旧输出的原子发布行为。

## 8. 产物与缓存

target、backend、C toolchain、runtime build 和 final-link 继续各自表达其负责的事实。对象格式新增 ELF relocatable 的 typed id，目标三元组/libc 必须进入当前兼容性选择；不根据 ELF `EI_OSABI` 猜测 glibc/musl，因为普通 ELF 对象不一定携带这种区分。

代码输出的 ABI/符号/metadata 规则属于 target/backend 契约；实际 C compiler、headers、CRT、unwinder 及 linker 属于相应构建输入和缓存。沿已有 fingerprint 机制加入实际新增输入，不建立来源授权或额外 compatibility receipt。

core artifact、普通 Cone cache、runtime cache 和 native bridge 按 canonical target 隔离。改换 musl headers、wrapper specs、libunwind archive 或编译 flags 必须使相关缓存失效。glibc/musl 混合 `.slib` 应在读取兼容性边界清楚报错，而不是拖到系统 linker。

复用现有 metadata C 布局，M27 的 runtime metadata ABI 4 不因 ELF 自动升版。保留 Darwin 的既有 `llvm-22-1/1` backend profile；为新增机器合同注册 `llvm-22-1-linux-x86-64/1`，两个 Linux libc target 共用这一后端配置，分别使用自己的 target 和 C toolchain profile。新的 backend contract 保存 X86 发射、帧/root 策略及实际 LSDA 编码集合，不能沿用含 Darwin 假设的旧 fingerprint。版本号相同的 LLVM 库不等于相同的完整 backend contract。

若确需改变某个持久 section 的字段/含义，明确升相应版本并要求重建；新增平台本身不要求盲目升级所有无关 wire schema。后续 Linux AArch64 可新增其 backend 配置，复用同一 registry 和阶段投影结构。

## 9. 实施顺序与验收

按可运行的纵向切片推进，先解决布局/帧/链接问题，再扩大功能覆盖，不先建设通用平台框架。

| 批次 | 实施内容 | 结束条件 |
| --- | --- | --- |
| 1 | spec、target/profile、host/toolchain 选择；LLVM libunwind 两套构建 | 两种 libc 的原生异常探针成功，错误工具链明确诊断 |
| 2 | X86 codegen、ELF object/reader、startup/link、Linux image/VM、amd64 anchor/frame | 正式 glibc CLI 从 core 到最小 Scoop 程序运行，并通过一次强制移动 GC |
| 3 | amd64 FFI/Scoop ABI shim、ELF TLS、完整异常、native 输入 | 独立 fixture 和 ABI/GC/异常组合运行；artifact-only 链接不读源码 |
| 4 | ODR、跨 Cone、registration/stackmap 合并及缓存 | 多 Cone 泛型/Context/static identity 唯一，重建及目标隔离正确 |
| 5 | 同一路径完成 musl static/PIE、静态 metadata 权限和栈增长处理 | 两种 musl 链接模式运行，深栈、外部线程和移动 GC 通过 |
| 6 | 全部 fixture 迁移、三平台回归与验收记录 | 各平台适用测试通过，没有通过整批 skip 掩盖能力缺口 |

每批 Rust/Python 变更按 AGENTS.md 先格式化和 lint，再执行相关测试。总验收运行 workspace tests、三个正式 binary 构建、公共 fixture runner 测试及完整文件 fixture；`cargo test` 不能代替正式 CLI。

### 9.1 关键测试矩阵

- **目标/产物**：alias canonical 化、glibc/musl 拒绝混用、core 和 runtime 缓存隔离、缺少 musl headers/CRT/unwind、错误架构/ELF 类别，以及 artifact-only link 不需要 `llvm-config`。
- **ABI**：scalar/ZST、16 字节及大 aggregate、`sret`/`byval`、栈参数、CLayout/packed、Scoop ABI 与 C bridge、String 两个 Option 返回后备。锁定真实 LLVM/对象结果，不以两端同时写错的手工 C shim 自证。
- **GC/frame**：两个 libc、O0/O2、深调用超过 musl attach 时初始栈映射、高寄存器压力、间接调用、invoke root frame、native-safe/borrowed、foreign thread callback、重入和多 mutator STW；启用 `SCOOP_GC_STRESS_MOVE=1`。
- **ELF/链接**：PIE 地址随机化、静态无 loader、metadata 真只读、完整 stackmaps、未使用函数和多个 object blob、COMDAT 重复实例、TD/storage/context cell 地址身份、原生 `.o`/`.a`/`.so`、TLS、loader/版本化符号。
- **语言组合**：跨 Cone 泛型、初始化成功/失败/循环、异常 catch/cleanup/rethrow、release hook、Char/List/StringBuilder、协程挂起/恢复、M27 Context fork/恢复及 callback 快照。
- **诊断/golden**：源码错误保留精确 span/message；平台特有错误断言目标和失败输入。HIR/MIR 复用平台无关快照，LIR/ABI/link plan 对真正不同的部分维护 target 变体，不全量机械复制快照。

fixture runner 增加显式 `--target` 和目标 C 工具配置，不能把 `platform.machine()` 等同于本次产物 target。glibc host 上必须能选择 musl；适用性过滤后再解析所需的 native 工具，避免 Darwin-only fixture 触发 Linux 的 xcrun 查询。Darwin flags 改为目标对应的明确命令参数，不塞进包含空格的单个工具路径字符串。

现有测试中只验证语言语义的 Darwin target 限制应移除；平台专属测试保留精确条件和原因。总报告分别统计执行、失败、不适用，不能把“全部不适用”当新平台验收成功。最终在真实 Darwin/AArch64 机器复跑原平台，本机 Linux 探针不能替代它。

## 10. 规范同步和 arm64 扩展边界

实施前同步以下位置，并核对交叉引用：

| 文档 | 章节及修改内容 |
| --- | --- |
| `SCOOP-SPEC.md` | §12.5 target/object profile、ELF ODR/native 库；§13.4 symbol normalization；§14.2 target-specific Scoop ABI，特别是 `byval` 与真实机器传参 |
| `SCOOP-RUNTIME-SPEC.md` | §2.8 ELF 加载/metadata；§3.2/3.5 amd64 stack roots、入口及栈范围发布；§5.2 cleanup-only LSDA；§5.5 LLVM libunwind provider 与 Linux CRT 符号边界；第 6 章 String 返回 shim；清理“只有 macOS/AArch64”的当前限制 |
| `SCOOP-IMPL-SPEC.md` | 第 1 章支持矩阵；§2.4/2.5 target/codegen/frame；§2.6 `.slib`；§2.7 toolchain/cache；§2.8 ELF final-link；§2.13/2.14 ODR 与 image |
| `ROADMAP.md`、runtime/fixture README | 新增 M28 范围/验收矩阵，修正已过时的构建入口和 host 限制 |

Linux arm64 后续应主要新增：registry 条目、AArch64 ELF ABI/relocation 和必要入口汇编、对应 libc/unwind 工具链输入、架构测试与 golden。Linux OS/VM、ELF image、metadata/link script、共有 `.slib`/ODR/native 链接编排和 libc 选择不应重写。

若本阶段的实现需要把整个 linker/runtime 按 `linux_amd64_glibc`、`linux_amd64_musl` 各复制一次，或把架构寄存器写入共有 reader/collector，说明拆分边界尚未完成。反之，不为尚未支持的 arm64 增加空分支、占位 profile 或假想使用者；用现有 Darwin 与两种 Linux libc 的实际复用检验结构。
