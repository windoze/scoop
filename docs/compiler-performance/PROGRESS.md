# 编译速度与 `.slib` 尺寸优化

开始日期：2026-11-01。源码基线 `427ef37cccc67ef4b95942ef31ea7c51ec73410c`。先测量实际编译与当前产物，再按收益安排实施；不降低语言检查、生成代码的优化等级、GC／ABI 正确性或调试能力。每轮完成后更新本文、清理无用临时产物、提交并推送，再合并 `origin/main`。

按最新安排，当前集中处理 `.slib` 尺寸：每次改动只做必要的定向验证，尺寸方面的大幅优化全部完成后统一运行全量 fixture。随后再集中处理编译速度，同样在该阶段的大项完成后统一全量验收。发现的已有错误一并修复，不以基线问题为由排除检查。

已有工作见[编译性能修复](../COMPILER-PERFORMANCE.md)和[存储精简与压缩](../slib-storage/DESIGN.md)。本任务从这些优化之后的当前版本继续，不将过去的收益再次计入。

## 基线与方法

本机为 Apple M3 Ultra、macOS 27.2、Rust 1.99.0、LLVM 22.1.8。第一、二轮配对计时显式设置 `LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22`；PATH 中默认 LLVM 已是 23，不用于这些对照。第二轮完整验收发现该安装缺少仓库所需的 LLVM 补丁，随后改用 `scripts/build_llvm.py` 构建的私有 22.1.8 SDK，并配置同一 SDK 的运行时 rpath。下文保留原环境内的成对测量，不把切换 LLVM 的影响算入结构优化收益。编译器与 Scoop 库均使用 release。

以真实的 `scoop.std` 和 `scoop.http.client` 源码执行 `scoop build --release --emit-slib`。每次源码重编译使用独立的新缓存和输出目录，复用同一套预编译依赖；随后用原命令测量输入未变的缓存命中。三组旧／新交替执行，报告中位数并保留全部样本。操作系统文件缓存保持正常，不将其称为整机冷启动；采样运行与正式计时分开。`/usr/bin/time -l -p` 记录墙钟和最大单进程 RSS，计时不包含事后产物比较。

首次未采样观测为标准库 13.74 秒、HTTP 客户端 102.78 秒；正式交替样本更稳定，以下收益均以同组对照计算，不用首次较慢的数值放大收益。采样中标准库约 50%、HTTP 客户端约 60% 的主线程样本落在对象生成；元数据分区分别约占总样本的 26%／23%，其中反复建立机器代码生成 pass 占明显成本。

当前 Darwin/AArch64 sysroot 的 29 个库合计 419.13 MiB、238,683 个归档成员：对象 171.75 MiB、LIR 130.28 MiB、HIR 44.66 MiB、manifest 41.87 MiB、MIR 16.80 MiB，其余为 ar header／padding。该清单只读现有产物，库集合不同于此前 25 库报告，不能直接作跨报告尺寸增减比较。

## 第一轮：静态元数据直接由 LLVM 汇编器发射

只有数据、别名及函数声明的分区使用 LLVM 的目标 AsmPrinter／MC；不为每个小对象建立无实际函数可处理的指令选择、寄存器分配及机器函数优化流水线。目标配置、物理分区、符号、TLS、COMDAT、DWARF、module assembly 和原生对象处理保持原合同。函数对象继续使用完整流水线，无新增依赖、用户开关或自行实现的对象编码器。规范见实现规范 2.5。

| 实际源码重编译 | 原版中位数 | 新版中位数 | 耗时降低 | 原版／新版范围 |
| --- | ---: | ---: | ---: | --- |
| `scoop.std` | 11.649 s | 10.364 s | 11.0% | 11.581～11.739 / 10.332～10.375 s |
| `scoop.http.client` | 95.492 s | 88.899 s | 6.9% | 94.471～96.355 / 84.745～89.549 s |

两库三组新旧产物全部逐字节相同，完整 `.slib` 仍为 12,054,970 和 85,426,772 bytes。本轮是编译速度收益，没有尺寸收益。输入未变的缓存命中中位数约 20～26 ms，未变化的缓存路径没有新增工作。RSS 中位数分别为 1,448,673,280 → 1,482,440,704 bytes、12,054,560,768 → 12,136,710,144 bytes；未观察到内存下降，不将小幅差异解释为内存优化。

验证：本机格式化、workspace all-targets Clippy 与 release 编译通过。新增独立测试以 LLVM 完整流水线为参照，在七种目标、debug／release 两种配置下逐字节比较数据对象，覆盖初始化数据、BSS、TLS、别名、外部函数／数据引用、COMDAT 与 DWARF。标准库完整生产另作逐字节对照。

首轮本机后端为 388 项通过、1 项失败：`windows_statepoints_record_the_return_pc_before_seh_padding` 得到 16 而非 15 的返回 PC。原因是 Homebrew LLVM 缺少仓库已有的 `llvm-windows-statepoint-return-pc.patch`，不是本轮新增的数据发射路径。第二轮已构建包含该补丁的 LLVM，原断言及完整 workspace 验收均通过；Windows 原生运行不在该交叉目标对象测试的验证范围内。

首轮 Linux 在 `nuc12` 的独立目录 `~/repos/scoop-compiler-speed-20261101` 使用 Rust 1.99.0、LLVM 22.1.2，格式检查、release workspace all-targets Clippy 通过；后端 401 项通过，当时排除了上述 LLVM 补丁检查。第二轮已换用配套的补丁版 22.1.8，后续验收不再排除此项。Linux 覆盖实际原生运行与对象合同，新增逐字节对照同样通过。

正式 CLI 的 `generic-reader`、`runtime-images`、`direct-c/artifacts`、`interfaces/artifacts`、`gc-correctness/native/tls-threads` 五组全部通过：20 个 fixture、23 个模式组合、151 次实际进程调用，含 5 份专用 golden 对照。覆盖预编译泛型消费、初始化顺序与失败、跨 profile 的 C ABI 和接口产物、TLS／回调／线程及定向 moving GC。普通验收复用同一 sysroot 和共享 Cone cache；并未执行全仓库全部 fixture。

## 第二轮：删除派生定义数据并合并符号使用

`cone-production` 不再保存可由 LIR foundation 的 typed definition plans、atoms 和 symbol requests 恢复的 canonical definitions。Compile 从已经读取的记录生成所需数据；Link 只取定义归属和两个 trap 符号，不构建完整 codegen symbol surface。删除对应重复 decoder，保留生产阶段需要的检查。

链接目录不再保存 typed owner 已确定的完整符号名。ODR 使用已有 member／callable body key 找到真实实体，保留各目标的原生符号前缀；使用表按目标符号合并 source members，同库目标引用定义目录，外部名称每个不同符号只保存一次。目录下标只复用名称，仍按原有全程序符号与 ODR 规则选择定义。读取后同名使用共享字符串存储。

本轮不改变压缩算法、级别、物理分区、原生对象、runtime ABI 或源码身份。生产 profile 升为 `cross-cone-generic/8`，`cone-production/16`、`strong-production/26`、`link-identity-closure/20` 同步升级；旧产物按正常版本与更新规则重建。

| 真实 release 产物 | 优化前 | 优化后 | 尺寸减少 |
| --- | ---: | ---: | ---: |
| `scoop.core` | 20,102,590 bytes | 18,486,580 bytes | 8.0% |
| `scoop.std` | 12,054,970 bytes | 11,324,054 bytes | 6.1% |
| `scoop.http.client` | 85,426,772 bytes | 77,481,064 bytes | 9.3% |
| Darwin/AArch64 全部 29 库 | 439,479,908 bytes | 403,881,794 bytes | 8.1% |

全库共减少 35,598,114 bytes（33.95 MiB），419.12 → 385.17 MiB。238,596 个机器对象按稳定成员 key 对应后全部逐字节相同，HIR／MIR 的存储大小未改变；尺寸变化来自 LIR metadata，manifest 指纹更新仅有微小压缩差异。

基线核对发现，现有 sysroot 的 `scoop.graphics` 与当前源码不完全对应。用第一轮保存的旧编译器编译当前源码后，该库机器对象与新版完全一致，旧格式产物由 6,818,130 修正为 6,812,362 bytes；以上合计使用修正值，不把这 5,768 bytes 的旧产物差异计入本轮收益。其余 28 库的原有机器对象均与重建后的新版一致。

使用第一轮保存的优化后工具与第二轮工具作三组交替对照，两侧分别使用对应格式的相同依赖。源码重编译与缓存命中均使用真实 CLI；每次冷编译采用新构建缓存，计时结束后删除单轮缓存。

| 实际源码重编译 | 第一轮后中位数 | 第二轮后中位数 | 原版／新版范围 |
| --- | ---: | ---: | --- |
| `scoop.std` | 10.245 s | 10.253 s | 9.960～10.293 / 10.240～10.262 s |
| `scoop.http.client` | 87.036 s | 87.369 s | 85.942～87.377 / 85.638～90.929 s |

本轮未观察到稳定的编译提速；中位数分别增加约 0.1%／0.4%，不将格式精简推算为时间收益。缓存命中中位数分别为 20.79 → 20.73 ms、26.93 → 26.16 ms。最大单进程 RSS 中位数为 1,473,970,176 → 1,427,619,840 bytes、12,072,615,936 → 11,881,922,560 bytes，下降约 3.1%／1.6%。本轮主要收益是上述实际产物尺寸下降。

已完成 macOS 与 Linux 的系统库／四种 runtime 资源准备、release workspace all-targets Clippy。身份、LIR 和 `.slib` 三个 crate 的测试分别为 macOS 995 项、Linux 996 项通过；新增对照覆盖 typed ODR subject、三种对象格式的符号名前缀、裁剪后的定义投影、跨 member 的本地／外部使用合并及目录下标边界。换用补丁版 LLVM 后，macOS 完整 release workspace 的 105 个测试组、6,310 项测试全部通过，失败／忽略／过滤均为零。fixture runner 规则测试 80 项完成，3 项平台条件跳过；构建脚本测试 32 项完成，6 项平台条件跳过。

Linux compiler-correctness 全组首次为 341 项通过、20 项失败、31 项平台条件跳过。20 项均来自显式选择 Clang 22.1.2 后的外部 C 对象长度快照及 C++ 驱动诊断，与 fixture 的 GCC 配置不符；以 `/usr/bin/gcc` 重跑 native 子组后，126 项全部通过、27 项平台条件跳过，覆盖并闭合原有全部失败。未修改编译器或 golden 来适配这些工具链差异。

macOS 首次完整 CLI 验收为 4,295 项通过、25 项平台不适用、6 项失败，共 4,326 个 fixture。按用户要求，已有问题也一并闭合：

- deployment 检查脚本此前直接把已压缩的对象成员当作 Mach-O 读取。现在声明并使用实际 zstd 工具解压后检查部署版本；复测通过。
- 异常路径的 debug consumer 在原始基线同样读到了错误的 landingpad 值。仓库已有的 `llvm-landingpad-debug-values.patch` 修复该 LLVM 问题；包含全部四个所需补丁的 SDK 上，原有 debug／release、移动 GC 组合共 2 个 variant、8 次进程调用通过，未改断言。
- native 缺失符号诊断改为显示声明库名；动态 provider 的文本计划按 install name 稳定排序，保留真实绑定、依赖、链接顺序与 canonical 编码。更新相应的必要 golden，原生相关 173 个适用 fixture 全部通过，覆盖 193 个 variant、868 次进程调用、33 份专用 golden。
- LLVM SDK 补齐正确性 fixture 使用的 llc，并关闭从解包目录上层仓库误取 Git revision；文档补全开发构建的 rpath 配置，避免编译期选择 22.1、运行时却加载其他 LLVM。

不再为本轮重复执行全量 CLI；尺寸阶段完成后统一验收。Linux 新工具链的 workspace 检查在 `scoop` crate 出现 77 项通过、1 项失败，原因是任务目录缺少 musl 的 LLVM unwind 开发资源。使用正式脚本补齐后，仅重跑该 runtime 测试并通过；Windows statepoint 返回地址测试、原生库名诊断 fixture 也分别通过。没有重新启动该轮全量检查。macOS 的 29 个系统库与四种 runtime 已使用配套 SDK 重新准备，供后续尺寸优化对照。

## 第三轮：复用真实依赖中的完整 ODR 对象

归档组装从已经读取的依赖目录中匹配完整 ODR bundle：group、member、role、shared ABI、存储长度与摘要均相同才省去当前副本。不读取依赖机器对象，不重新计算依赖摘要；函数正文、私有 GC／EH／注册记录和共享 dispatch 数组一起选择。同步删除本地物理成员、defined owners 及来自该成员的符号使用，保留其他对象对这些符号的引用。含 Strong、C bridge、image 或当前导出明确绑定的物理定义继续保留。

没有本地 primary 定义的 ODR 目录条目由真实依赖闭包满足，不能借用无依赖关系的兄弟库。私有 RegistrationRecord 只记录实际本地定义，随选定函数对象消费，避免要求 debug／release 具有相同 safepoint 集合。可复用 ODR 实现的可用性与 ABI 并入既有 LIR 指纹；依赖库内部不再生成某个实例时，消费者正常失效并重新编译。仅机器字节、调试信息或私有记录变化仍遵守原有更新规则，不新增缓存检查或目录副本。

本轮使用包含全部仓库补丁的 LLVM 22.1.8。先在合并主线后的 `4d4a5c8b98` 冻结工具并重新准备 29 库，基线仍为 403,881,794 bytes，再使用相同源码、目标、release 配置和 Apple Clang 重建。profile 升为 `cross-cone-generic/9`、`single-cone-production/10`；对象内容、压缩参数及 runtime ABI 不变。

| 真实 release 产物 | 第二轮后 | 本轮后 | 尺寸减少 |
| --- | ---: | ---: | ---: |
| `scoop.core` | 18,486,580 bytes | 18,486,580 bytes | 0.0% |
| `scoop.std` | 11,324,054 bytes | 10,830,330 bytes | 4.4% |
| `scoop.http.client` | 77,481,064 bytes | 74,656,670 bytes | 3.6% |
| `scoop.http.server` | 47,128,412 bytes | 44,638,684 bytes | 5.3% |
| Darwin/AArch64 全部 29 库 | 403,881,794 bytes | 385,553,960 bytes | 4.5% |

实际减少 18,327,834 bytes（17.48 MiB）。删除的 29,548 个对象共占 11,872,248 bytes 压缩 payload，其余收益来自相关目录、使用表及归档头。209,048 个保留对象与基线逐字节相同；每个删除对象都在新版真实传递依赖中找到完整 bundle 与内容相同的物理对象。基线离线候选为 73,127 个对象、36.91 MiB，尚未排除当前导出绑定；不将该上限当作落地收益。剩余绑定能否沿既有 ODR 选择简化仍需继续核对。

一次交替对照用于排查明显编译回退：标准库源码重编译 10.529 → 10.164 s，HTTP 客户端 85.566 → 85.073 s；缓存命中分别为 21.48 → 20.09 ms、24.80 → 24.43 ms。最大单进程 RSS 分别为 1,457,831,936 → 1,466,515,456 bytes、11,865,391,104 → 12,110,675,968 bytes，增加约 0.6%／2.1%。这是单组观测，不宣称稳定提速或内存下降；压缩算法与等级仍相同。

验证完成 release workspace all-targets Clippy、`.slib` crate 259 项、4 项真实泛型生产／消费测试及 7 项缓存 key 测试。格式升级引起的 profile 描述与 cache key 固定向量已同步更新，只复测原失败项。新增测试覆盖完整私有 bundle、DWARF 保留、实际依赖关系以及 provider 实例消失后的正常重编译。正式 CLI 定向覆盖 11 个 fixture、13 个 variant、131 次进程调用，包括 ODR 选择、混合优化模式、泛型发布与读取、共享 LIR 选择及异常路径。未运行全量 fixture。

五个 generic-reader 用例原来的 20 份平台快照只列出 `m00000000` 起的连续编号，锁定了无语义意义的对象数量。改为由原生 `ar` 完整读取并匹配 manifest、合法成员名和非空对象目录；编译、预编译消费、链接与运行断言保留，五项复测通过。另修复合并主线后 runtime builder 的独立 Cargo.lock 遗漏 `scoop-source` 新依赖 `annotate-snippets` 的错误；正式脚本按 `--locked` 成功准备四种 runtime。

## 后续优先级

尺寸优化按用户补充要求，优先精简结构、删除不必要或重复的信息；压缩只作为最后的补充，且不采用 CPU／内存消耗过大的算法。

1. 继续检查当前导出绑定所保留的 ODR 副本，确认现有 typed ABI 与物理选择是否可以直接复用真实依赖。完整对象复用的第一部分已在第三轮落地；不为省略对象增加转发或重复归属目录。
2. 将现有 LIR 的 32-byte ID 表机制用于 HIR／MIR，保留 typed ID 和逻辑指纹。29 库基线的独立投影包含表和外层编码，并核对原始 CBOR 与 Zstd level 3：HIR 46,829,937 → 33,654,840 bytes，MIR 17,613,076 → 16,200,346 bytes，合计可省约 13.91 MiB。成员目录还保存了机器对象读取不再使用的逻辑 key，原基线去除该副本的 manifest 投影可省约 7.82 MiB，需按第三轮后的实际目录继续验证。manifest 本身的全量 ID 表反而增加约 2.10 MiB，不采用。普通 LIR 字符串表和旧 HIR 来源索引收益较小，继续后置。
3. 按用户最新补充，格式大项完成后实施共享对象压缩字典，再做尺寸阶段的统一全量 fixture。既有离线投影使用 Zstd level 3 和每库 64 KiB 字典，标准库对象存储 4,328,545 → 2,466,108 bytes，HTTP 客户端 35,465,193 → 21,513,353 bytes，已计入字典自身；这是此前对象集合的投影，不能当作落地收益。实施时实际测量压缩、解压耗时与峰值内存，避免高开销算法；同时让现有对象内容摘要独立于字典编码，维持跨库 ODR 内容复用，不另存一份重复摘要。

已排除直接复用 LLVM 22.1 的对象发射 pass manager：即使 AsmPrinter／MCStreamer／MachineModuleInfo 有逐模块收尾，带 GC statepoint 与异常展开的第二个函数模块仍残留前一模块的栈映射记录；仅数据流水线在 COFF debug → 无 debug 切换时也残留字符串表空间。独立的连续模块字节对照发现两者，实验实现均已撤下，未进入生产。本轮仅补齐数据发射入口自身的 LLVM pass 注册，并将测试顺序调整为先调用数据入口，避免测试依赖完整代码生成的先行初始化。格式检查、workspace all-targets Clippy 和七目标两配置的数据对象对照通过；不为撤下的实验记录性能收益。

本机原始命令、采样、逐轮计时和验收日志保留在仓库外的 `/Volumes/Data/home/chenxu/scoop-performance-results/compiler-20261101`；Linux 日志在 `~/repos/scoop-work/compiler-20261101`。工作缓存与对照二进制暂放本仓库 `tmp/compile-speed-20261101`，只保留下一轮实际需要的内容。
