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

## 后续优先级

尺寸优化按用户补充要求，优先精简结构、删除不必要或重复的信息；压缩只作为最后的补充，且不采用 CPU／内存消耗过大的算法。

1. 继续核对三层 metadata 的实体记录，优先处理仍有显著收益的重复内容。LIR 生产计划和链接目录的主要重复已在第二轮删除。普通 LIR 字符串表投影仅使 core／std／HTTP client 的 LIR 成员减少约 0.22／0.04／2.08 MiB，占完整归档更低，暂不为它增加格式变更；已有 HIR 来源索引的收益也较小。
2. 减少真实依赖闭包内重复的 ODR 物化与持久化记录。已按 manifest 的真实传递依赖、完整物理对象 bundle 的 group／member／role／ABI 匹配，排除含 Strong、bridge 或 image 主定义的对象。29 库中有 73,230 个对象、38,987,871 bytes（37.18 MiB）压缩 payload 可由已有依赖中的候选提供，约占当前整包的 9.65%；不计额外 metadata／归档头收益。该结果仍是结构分析，尚未落地；实施需保持 typed 导入、ABI、调试与发布合同。
3. 共享对象压缩字典已暂缓，实验实现从工作树撤出，未提交／启用。离线投影使用原始对象 bytes、Zstd level 3 和每库 64 KiB 字典，标准库对象存储 4,328,545 → 2,466,108 bytes，HTTP 客户端 35,465,193 → 21,513,353 bytes，已计入字典自身；完整归档估算减少 15.45%／16.33%。逐对象解压后与原 bytes 一致，实验代码只完成格式化和 release workspace all-targets Clippy，尚未完成正确性和真实产物验收。待结构与重复信息的大项处理后再评估，不能将投影记为落地收益。

已排除直接复用 LLVM 22.1 的对象发射 pass manager：即使 AsmPrinter／MCStreamer／MachineModuleInfo 有逐模块收尾，带 GC statepoint 与异常展开的第二个函数模块仍残留前一模块的栈映射记录；仅数据流水线在 COFF debug → 无 debug 切换时也残留字符串表空间。独立的连续模块字节对照发现两者，实验实现均已撤下，未进入生产。本轮仅补齐数据发射入口自身的 LLVM pass 注册，并将测试顺序调整为先调用数据入口，避免测试依赖完整代码生成的先行初始化。格式检查、workspace all-targets Clippy 和七目标两配置的数据对象对照通过；不为撤下的实验记录性能收益。

本机原始命令、采样、逐轮计时和验收日志保留在仓库外的 `/Volumes/Data/home/chenxu/scoop-performance-results/compiler-20261101`；Linux 日志在 `~/repos/scoop-work/compiler-20261101`。工作缓存与对照二进制暂放本仓库 `tmp/compile-speed-20261101`，只保留下一轮实际需要的内容。
