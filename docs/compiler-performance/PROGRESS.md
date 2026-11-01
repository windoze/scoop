# 编译速度与 `.slib` 尺寸优化

开始日期：2026-11-01。源码基线 `427ef37cccc67ef4b95942ef31ea7c51ec73410c`。先测量实际编译与当前产物，再按收益安排实施；不降低语言检查、生成代码的优化等级、GC／ABI 正确性或调试能力。每轮完成后更新本文、清理无用临时产物、提交并推送，再合并 `origin/main`。

按最新安排，`.slib` 尺寸与编译速度已确认的大项优化均已落地，本次完成统一验收后收尾，不再展开小幅优化。编译速度优先处理了影响大且 debug／release 共有的路径；各轮只做必要的定向验证，阶段末每目标执行一次全量，发现的已有错误一并修复后定向闭合。

已有工作见[编译性能修复](../COMPILER-PERFORMANCE.md)和[存储精简与压缩](../slib-storage/DESIGN.md)。本任务从这些优化之后的当前版本继续，不将过去的收益再次计入。

## 基线与方法

本机为 Apple M3 Ultra、macOS 27.2、Rust 1.99.0、LLVM 22.1.8。第一、二轮配对计时显式设置 `LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22`；PATH 中默认 LLVM 已是 23，不用于这些对照。第二轮完整验收发现该安装缺少仓库所需的 LLVM 补丁，随后改用 `scripts/build_llvm.py` 构建的私有 22.1.8 SDK，并配置同一 SDK 的运行时 rpath。下文保留原环境内的成对测量，不把切换 LLVM 的影响算入结构优化收益。编译器使用 release 构建；第一至第九轮测量 Scoop release，从第十轮开始分别测量 Scoop debug／release，保留各自默认调试信息。

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

没有本地 primary 定义的 ODR 目录条目由真实依赖闭包满足，不能借用无依赖关系的兄弟库。私有 RegistrationRecord 只记录实际本地定义，随选定函数对象消费，避免要求 debug／release 具有相同 safepoint 集合。第三轮曾将可复用 ODR 实现的可用性与 ABI 并入 LIR 指纹；尺寸阶段统一验收发现它会错误拒绝混合优化 profile，已在下文修正为使用完整依赖的既有 Code 指纹更新源码缓存，不新增缓存检查或目录副本。

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

## 第四轮：导出的 ODR 定义使用同一共享物理实现

核对实际身份模型后，取消第三轮对导出对象的额外保留：`ObjectDefinitionPlanKey::odr` 只有共享 OdrMemberId 与 primary role，不含 producer。导出的 provider 选择语义／ABI 记录，实际机器对象由既有 ODR 目录及链接选择确定。删除约 100 行导出筛选代码，所有完整、内容相同的 ODR 对象统一复用实际依赖；原导出、共享 ID、symbol 和 ABI 不改写，不增加转发数据或新的格式版本。

| 真实 release 产物 | 第三轮后 | 本轮后 | 尺寸减少 |
| --- | ---: | ---: | ---: |
| `scoop.core` | 18,486,580 bytes | 18,486,580 bytes | 0.0% |
| `scoop.std` | 10,830,330 bytes | 9,894,862 bytes | 8.6% |
| `scoop.http.client` | 74,656,670 bytes | 68,789,944 bytes | 7.9% |
| `scoop.http.server` | 44,638,684 bytes | 39,199,366 bytes | 12.2% |
| Darwin/AArch64 全部 29 库 | 385,553,960 bytes | 348,050,608 bytes | 9.7% |

本轮再省 37,503,352 bytes（35.77 MiB）。43,561 个省去的对象占 26,786,912 bytes 压缩 payload，其余来自目录与归档头；165,487 个保留对象逐字节相同，省去对象均在新版真实传递依赖中找到同内容、同完整 bundle 的物理定义。两轮 ODR 合计删除 73,109 个对象、减少 53.24 MiB，接近先前未落地的候选上限。加上第二轮结构精简，29 库从 439,479,908 降到 348,050,608 bytes，累计减少 87.19 MiB（20.8%）。

扩展真实泛型库测试，要求库导出的共享 definition 在本库没有物理副本时，仍保留原 provider 和 definition ID，并由依赖中的实际 primary 定义满足。该断言先在第三轮实现上失败，简化后 4 项泛型生产／消费测试通过，继续覆盖 debug 信息、混合优化模式、完整私有记录和依赖更新。release workspace all-targets Clippy、29 库及四种 runtime 准备成功。正式 CLI 定向为 23 个 fixture、25 个 variant、170 次进程调用全部通过，增加 runtime 初始化、循环／失败恢复与注册表选择覆盖。本轮没有运行全量 fixture，也不以单纯的对象减少宣称编译提速。

## 第五轮：HIR／MIR 共用成员内 ID 表

将已有的 LIR 存储索引用于 HIR／MIR，同一 metadata 成员的已知 section 共用按 bytes 排序的 32-byte 值表。三层统一 outer schema 4、同一套 writer／reader，删除 HIR／MIR 的特殊分支；未知扩展继续保存 opaque payload。全局 typed ID、逻辑投影、原生对象和 Zstd level 3 保持原规则，表索引不进入实体身份计算。profile 升为 `cross-cone-generic/10`，复合 ABI 与既有缓存 key 按格式版本正常更新。

| 真实 release 产物 | 第四轮后 | 本轮后 | 尺寸减少 |
| --- | ---: | ---: | ---: |
| `scoop.core` | 18,486,580 bytes | 18,111,610 bytes | 2.0% |
| `scoop.std` | 9,894,862 bytes | 9,222,612 bytes | 6.8% |
| `scoop.http.client` | 68,789,944 bytes | 64,958,328 bytes | 5.6% |
| `scoop.http.server` | 39,199,366 bytes | 37,027,406 bytes | 5.5% |
| Darwin/AArch64 全部 29 库 | 348,050,608 bytes | 333,462,792 bytes | 4.2% |

以 `d11b966154` 的工具和完整 29 库为基线，实际再省 14,587,816 bytes（13.91 MiB）。HIR 46,829,937 → 33,654,840 bytes、MIR 17,613,076 → 16,200,346 bytes，与先前独立投影完全一致；LIR 存储大小未变，manifest 摘要与 ar padding 只有少量变化。165,487 个原生对象没有增删，全部逐字节相同。尺寸阶段累计从 439,479,908 降到 333,462,792 bytes，减少 101.11 MiB（24.1%）。

一次交替对照用于检查明显回退：标准库源码重编译 11.531 → 9.961 s，HTTP 客户端 103.451 → 86.094 s；最大单进程 RSS 分别为 1,482,506,240 → 1,424,506,880 bytes、11,736,580,096 → 11,568,119,808 bytes。缓存命中为 23.03 → 23.31 ms、26.32 → 29.11 ms。该单组观测没有显示新增建表带来的明显回退，不据此宣称稳定提速；速度阶段再作成组对照。

本机 release workspace all-targets Clippy 通过，`.slib` 的 259 项测试完成覆盖；格式升级引起的三处固定向量／版本用例更新后仅定向复测。另有 6 项 capability、7 项缓存 key、4 项真实泛型生产／消费测试通过，缓存固定向量同步更新。存储测试覆盖三层共享 ID、opaque 扩展、借用 payload、表长度／顺序和索引边界；兼容测试覆盖旧 schema 1～3 与混合版本。修复原有元数据错误信息硬编码 schema 1 的问题，实际报告当前版本。正式 CLI 的 11 个定向 fixture、13 个 variant、91 次进程调用全部通过；29 库与四种 runtime 准备成功，没有运行全量 fixture。跨平台的阶段末验收继续准备，Windows 正在本任务目录构建配套开发 SDK，补齐现有安装缺少的 LLVM 头文件；Windows 原生 fixture 尚未运行。

## 第六轮：ODR 目录与 LIR 共用 ID 表

将原来单独的 `single-cone-production` manifest section 移入 `link-identity-closure/21` 的字段 6，让 group、member 和 ABI 值复用 LIR 已有的成员内 ID 表。删除原 section、独立包装类型与 reader，manifest 没有必需 section；Code／RuntimeImage 继续使用顶层摘要。profile 升为 `cross-cone-generic/11`。逻辑 ODR 目录、实际依赖约束、ABI 合并与注册记录选择规则不变。

| 真实 release 产物 | 第五轮后 | 本轮后 | 尺寸减少 |
| --- | ---: | ---: | ---: |
| `scoop.core` | 18,111,610 bytes | 17,735,026 bytes | 2.1% |
| `scoop.std` | 9,222,612 bytes | 9,038,934 bytes | 2.0% |
| `scoop.http.client` | 64,958,328 bytes | 63,694,368 bytes | 1.9% |
| `scoop.http.server` | 37,027,406 bytes | 36,168,704 bytes | 2.3% |
| 全部 29 个系统库 | 333,462,792 bytes | 326,130,108 bytes | 2.2% |

实际再省 7,332,684 bytes（6.99 MiB）。manifest 从 34,019,893 降至 17,311,716 bytes，LIR 从 98,141,174 增至 107,516,675 bytes，合并后的总存储明显减少；HIR／MIR 大小保持不变。独立 CBOR／Zstd 脚本逐项确认 230,044 条 ODR 记录与全部语义指纹相同，165,487 个原生对象无增删、存储及解压后的 296,503,136 bytes 均相同。相对尺寸阶段原始 439,479,908 bytes，累计减少 108.10 MiB（25.8%）。

一次对照用于检查明显回退：标准库源码重编译 11.778 → 10.201 s，HTTP 客户端 100.234 → 83.326 s；最大单进程 RSS 分别为 1,442,398,208 → 1,440,251,904 bytes、11,563,958,272 → 11,547,656,192 bytes。缓存命中为 22.51 → 21.54 ms、24.63 → 24.42 ms。该单组观测未见明显回退，不据此宣称稳定提速；速度阶段再作成组对照。

release workspace all-targets Clippy、257 项 slib 测试、4 项真实泛型／ODR 测试、1 项实际 core 导出与依赖链接测试通过。7 项缓存 key 检查完成，版本变化对应的固定向量更新后只复测该项；profile descriptor 也使用独立 CBOR 与域分隔 SHA-256 同步核对。新增目录字段缺失／错误字段号检查，真实非空目录往返与旧 capability 拒绝继续覆盖。修复原有 manifest／container 诊断仍硬编码 schema 1 的错误，改为报告当前格式版本。正式 CLI 的 11 个定向 fixture、13 个 variant、91 次进程调用全部通过；29 库准备成功、四种 runtime 复用，没有运行全量 fixture。

## 第七轮：成员目录删除重复 key 与对象目标字段

成员目录从五字段精简为 ID、角色、存储长度和摘要四字段，生产端仍使用完整 stable key 生成同一个 SlibMemberId。Scoop／generated-C 对象的角色不再重复保存 target、object format 和已无消费方的 verifier capability，统一使用 manifest 已确定的目标契约。普通 Native 对象使用独立角色保存后续链接确实需要的规范化源路径；三种 metadata 各唯一一份的要求在目录读取边界检查。容器／manifest 更新为 4，profile 为 `cross-cone-generic/12`，三层 metadata outer 仍为 4。

| 产物 | 优化前 | 优化后 | 缩减 |
| --- | ---: | ---: | ---: |
| `scoop.core` | 17,735,026 bytes | 17,261,458 bytes | 2.7% |
| `scoop.std` | 9,038,934 bytes | 8,886,662 bytes | 1.7% |
| `scoop.http.client` | 63,694,368 bytes | 62,608,680 bytes | 1.7% |
| `scoop.http.server` | 36,168,704 bytes | 35,605,210 bytes | 1.6% |
| 全部 29 个系统库 | 326,130,108 bytes | 320,272,772 bytes | 1.8% |

实际再省 5,857,336 bytes（5.59 MiB），全部来自 manifest，后者从 17,311,716 降至 11,454,380 bytes。独立 CBOR／Zstd 对照确认全部 165,574 个成员的 ID 与存储 payload 不变，包括 165,487 个原生对象及其解压后的 296,503,136 bytes、87 个 metadata payload；133 条 Native 源路径完整保留，230,044 条 ODR 记录与物理 bundle 不变。HIR／MIR／LIR／RuntimeImage 指纹相同，Code 指纹按更新后的成员目录编码正常变化。相对尺寸阶段原始 439,479,908 bytes，累计减少 113.68 MiB（27.1%）。

单组回退检查中，标准库源码重编译 12.594 → 10.645 s，HTTP 客户端 84.204 → 85.026 s；最大单进程 RSS 分别为 1,448,804,352 → 1,409,400,832 bytes、11,506,401,280 → 11,368,710,144 bytes。缓存命中为 20.87 → 20.78 ms、24.26 → 27.29 ms。HTTP 源码耗时基本持平，本轮不据单组结果宣称稳定提速；多组交替对照仍留到速度阶段。

release workspace all-targets Clippy、258 项 slib 测试、4 项真实泛型／ODR 测试和 7 项缓存 key 检查完成。格式变化的 Link／member／profile 固定向量已用独立 CBOR 与域分隔 SHA-256 核对；修复一个不完整 CBOR 测试样本，更新缓存 key 向量后只复测失败项。正式 CLI 的 15 个定向 fixture、18 个 variant、110 次进程调用全部通过，增加了 Native 产物消费、源路径、诊断与缓存覆盖。29 库准备成功，四种 runtime 复用，没有运行全量 fixture。

Windows 的正式 LLVM 22.1.8 SDK 已按仓库四个补丁构建完成，`612b41a79a` 的五个 release 命令在该 SDK 下编译并成功启动；不再需要的 LLVM 源码和构建中间目录已清理。这是后续平台验收的环境准备，尚未作为本轮 Windows 原生功能通过的依据。

## 第八轮：由目录直接定位连续 payload

`.slib` 使用 16-byte 固定头、压缩 manifest 和按成员 ID 排序的连续 payload。目录中的存储长度已经足够定位成员，因此删除逐成员 ar header、物理名称、固定文件属性、重复长度与对齐填充；文件和内存 reader 继续按需读取并缓存结果。容器／manifest 更新为 5，profile 为 `cross-cone-generic/13`，metadata outer 仍为 4。普通 native `.a` 的格式和处理独立保留。

| 产物 | 优化前 | 优化后 | 缩减 |
| --- | ---: | ---: | ---: |
| `scoop.core` | 17,261,458 bytes | 16,443,534 bytes | 4.7% |
| `scoop.std` | 8,886,662 bytes | 8,627,045 bytes | 2.9% |
| `scoop.http.client` | 62,608,680 bytes | 60,753,726 bytes | 3.0% |
| `scoop.http.server` | 35,605,210 bytes | 34,640,135 bytes | 2.7% |
| 全部 29 个系统库 | 320,272,772 bytes | 310,254,160 bytes | 3.1% |

实际再省 10,018,612 bytes（9.55 MiB），容器包装由 10,019,076 降为 464 bytes，manifest、HIR、MIR、LIR 和对象的存储总量分别保持不变。独立对照确认 165,574 个成员记录及 payload、230,044 条 ODR 记录、五种语义指纹均相同；其中 165,487 个原生对象解压后的 296,503,136 bytes 不变，133 条 Native 源路径保留。相对尺寸阶段原始 439,479,908 bytes，累计减少 123.24 MiB（29.4%）。

单组回退检查中，标准库源码重编译 11.943 → 10.217 s，HTTP 客户端 83.653 → 87.791 s；最大单进程 RSS 分别为 1,417,035,776 → 1,429,651,456 bytes、11,314,331,648 → 11,438,047,232 bytes。缓存命中为 21.90 → 21.66 ms、24.84 → 26.04 ms。HTTP 单组耗时增加 4.9%，没有据此判断稳定变化；尺寸阶段只确认功能与实际存储收益，速度阶段使用多组交替对照。

release workspace all-targets Clippy、260 项 slib 测试、4 项真实泛型／ODR 测试通过。7 项缓存 key 检查完成，profile 更新对应的固定向量修正后仅复测该项；profile descriptor 使用独立 CBOR／SHA-256 核对。边界测试覆盖紧凑布局、空目录、旧 magic、溢出、截断、只读取实际请求的成员，以及文件 reader 的缓冲区复用。七个针对旧 `.slib` ar 结构的列举步骤及其 14 份过时快照退役，真实 Native `.a` 构建和所有消费、链接、运行断言保留。正式 CLI 的 19 个定向 fixture、22 个 variant、121 次进程调用全部通过；29 库准备成功、四种 runtime 复用，没有运行全量 fixture。

## 第九轮：对象使用每库共享压缩字典

结构大项完成后，对 ODR 去重后的原生对象统一训练并编码。继续使用已有的 Zstandard 1.5.7、level 3，不引入新的编译器依赖或高等级压缩。按成员 ID 排序后均匀选择至多 2,048 个样本，每个最多 4 KiB；至少 64 个样本且累计达到 256 KiB 才训练，样本最多 8 MiB、字典最多 64 KiB。27 个库生成字典，两个小库保留普通 frame。

对象摘要改为原生 bytes 的 SHA-256，Code 指纹不再包含压缩长度。同内容对象在不同库字典下仍能复用完整 ODR bundle 和既有对象缓存，不另存第二份摘要。生产先完成内容摘要与去重，再一次批量压缩；发布缓存直接使用已有的原生 bytes。字典作为独立的 ObjectDictionary 成员保存，reader 只有实际请求对象时才加载字典并复用解压上下文。metadata 的压缩与按需读取不变。容器／manifest 升为 schema 6、profile 升为 `cross-cone-generic/14`，link-member 指纹 domain 升为 v2。

| 真实 release 产物 | 第八轮后 | 本轮后 | 尺寸减少 |
| --- | ---: | ---: | ---: |
| `scoop.core` | 16,443,534 bytes | 12,559,032 bytes | 23.6% |
| `scoop.std` | 8,627,045 bytes | 7,322,274 bytes | 15.1% |
| `scoop.http.client` | 60,753,726 bytes | 50,710,800 bytes | 16.5% |
| `scoop.http.server` | 34,640,135 bytes | 29,169,712 bytes | 15.8% |
| 全部 29 个系统库 | 310,254,160 bytes | 258,702,547 bytes | 16.6% |

实际净省 51,551,613 bytes（49.16 MiB）。对象 frame 从 141,427,455 降到 88,026,902 bytes，新增字典共 1,769,472 bytes；两者合计的净收益与离线投影完全一致。manifest 因摘要内容变化增加 79,468 bytes，以上实际收益已扣除。87 个 metadata payload、165,487 个原生对象解压后的全部 296,503,136 bytes、133 条 Native 源路径及 230,044 条 ODR 记录均保持一致。HIR／MIR／LIR／RuntimeImage 指纹不变，Code 指纹按新的原生内容投影正常变化。相对尺寸阶段起点 439,479,908 bytes，累计减少 180,777,361 bytes（172.40 MiB，41.1%）。

对全部 29 库的真实对象进行三组交替微观测量，解压使用生产 reader 的 frame 检查、分配及上下文复用路径。每次校验生成大小与实际 writer 相同、解压内容逐字节相同；以下为各库中位数求和。

| 对象处理 | 无字典 | 共享字典 |
| --- | ---: | ---: |
| 字典训练 | 无 | 2.044 s |
| 压缩 | 1.348 s | 0.914 s |
| 解压 | 0.576 s | 0.279 s |

训练和压缩合计从 1.348 增至 2.958 s，不能只用压缩部分宣称生产提速。各库训练中位数为 22.5～92.9 ms；相对于已加载对象语料，训练期间最大的新增峰值 RSS 为 18,006,016 bytes（17.2 MiB）。独立测量程序包含输入和压缩结果后的最大进程峰值为 142,295,040 → 153,354,240 bytes，不将它等同于完整编译器内存。保留 level 3 与小字典的实际成本符合本轮尺寸补充目标。

真实 CLI 的单组回退检查：标准库源码重编译 11.845 → 10.310 s、HTTP 客户端 84.129 → 84.724 s；最大单进程 RSS 分别为 1,414,840,320 → 1,377,255,424 bytes、11,514,134,528 → 11,043,667,968 bytes。缓存命中为 22.25 → 21.65 ms、25.84 → 25.17 ms。HTTP 时间基本持平，单组结果不作为稳定提速结论；编译速度阶段另作公平预热后的多组交替测量。

release workspace all-targets Clippy、265 项 slib 测试、4 项真实泛型／ODR 测试、7 项缓存 key 检查完成；格式升级的固定向量独立核对后仅复测原失败项。新增覆盖确定性训练、所有对象内容、跨字典 Code 指纹、普通小批对象、坏 frame／缺失字典、字典唯一性以及文件 reader 的延迟读取与并发。29 库重新准备成功，四种 runtime 复用。正式 CLI 的 10 个定向 fixture、13 个 variant、85 次进程调用全部通过。

收尾时发现 deployment 的辅助脚本仍假设旧 ar 包装，且不能使用对象字典解压。一并修正为从紧凑容器的真实 CBOR manifest 定位成员，使用可选字典解压，要求同时检查 Scoop 和 Native 对象；runtime、最终 executable 与缓存行为断言保留。开发依赖使用近期维护的 `cbor2==6.1.5`，不手写重复 CBOR reader。3 个 deployment 相关 fixture、4 个 variant、9 次进程调用通过，并使用真实 asyncio 库的 Scoop／Native 对象验证字典解压分支。本轮未运行全量 fixture；到此开始尺寸阶段的统一完整验收。

## 尺寸阶段验收记录

共享字典提交后，macOS 完整 release workspace 的 105 个测试组、6,347 项测试全部通过，没有失败、忽略或过滤。fixture runner 规则测试 80 项完成、3 项平台条件跳过，构建脚本测试 32 项完成、6 项平台条件跳过。AArch64 正式 CLI 全量执行 4,327 个 fixture，首次为 4,282 项通过、25 项平台不适用、20 项失败，共 4,502 个 variant、13,374 次进程调用。两个 Darwin 目标的系统库和四种 runtime 均已按 schema 6 准备。

已闭合其中 15 项：六个动态库用例的独立 link 命令遗漏公共 `link_args`，没有使用 build 的显式 C compiler／SDK 配置；补齐后保持原有计划指纹相等、链接、运行及五份专用 plan golden 断言。七个错误诊断用例同步主线已修复的错误恢复行为，移除派生的“未知名字”重复错误，保留首个类型／初始化错误及位置；不可调用整数现在断言明确的不可调用诊断。另同步一项 manifest 错误的新版源码片段显示。工具版本检查的一次失败来自本轮并行 Cargo 测试重建了部分 CLI，统一构建五个工具后闭合；后续 CLI 验收使用同一组工具，并与其重建错开。以上 15 项定向复测全部通过，覆盖 15 个 variant、63 次进程调用。

其余五项也已闭合。第三轮把物理 ODR 可用性混入 LIR 语义指纹，但优化会增删内部 callable、layout、descriptor 和 scan；真实 core 的 debug／release 产物相差 174 条非私有记录，不能据此拒绝语义与 ABI 相容的组合。本轮删除这项投影，优化前后 core 的 HIR／MIR／LIR 指纹现在全部相同，原有链接定义与 ABI 检查继续生效。

源码编译缓存改用已有 Code 指纹跟踪对象生产变化，并采用与 child 请求相同的完整 direct／support 依赖投影。只看直接依赖不足：中间库的 Code 可以保持不变，而下层库已经改变可用实现。新增正式 CLI 用例保持源码与直接依赖的预编译产物不变，只切换间接依赖的优化 profile，确认顶层在 debug／release 下都重新编译，下一次构建恢复命中。机器代码变化也会使相关源码缓存更新；Code 不成为预编译消费者的语义相等条件，不增加独立可用性摘要、目录扫描或命中复核。profile 升为 `cross-cone-generic/15`，cache key domain 升为 v11。

release workspace all-targets Clippy、264 项 slib、75 项 scoop 和 4 项真实泛型／ODR 测试完成；更新固定向量及配套工具后仅复测原失败项。12 个定向 fixture、15 个 variant、113 次进程调用全部通过，包括原来失败的五项、间接依赖缓存、泛型发布／读取与 ODR 选择。ODR 选择用例同步检查依赖 Code 改变后的重编译、重新链接和新产物的后续命中，不再错误要求消费者保持旧的物理成员集合。AArch64 29 库与四种 runtime 重新准备完成；独立检查确认全部 165,487 个原生对象及 296,503,136 bytes 原始内容、230,044 条 ODR 记录与物理 bundle 保持一致。总尺寸为 258,702,533 bytes，指纹改变后的 manifest 压缩波动比第九轮少 14 bytes，不计为新的优化收益。

AArch64 首次全量的 20 项失败均已闭合，没有重跑整套 fixture。macOS x86_64 使用修复后的 profile 15 与配套五个 CLI 完成全量：4,328 个 fixture 中 4,303 项通过、25 项平台不适用，没有失败，共 4,525 个 variant、13,417 次进程调用、55 份 stage／plan golden。两个 Darwin 目标的 29 库与四种 runtime 均已更新为 profile 15。

Linux 修复后完整 release workspace 的 105 个测试组、6,377 项测试全部通过，没有失败、忽略或过滤；包含此前遇到临时脚本执行 ETXTBSY 的原测试。fixture runner 规则测试 80 项完成、3 项平台条件跳过，构建脚本测试 32 项完成、5 项平台条件跳过。GNU／musl 的系统库与四种 runtime 均已准备完成，两个目标各执行完整 4,328 个 fixture，均无失败。GNU 为 4,285 项通过、43 项平台不适用，共 4,504 个 variant、13,264 次进程调用、41 份 stage／plan golden；musl 为 4,282 项通过、46 项平台不适用，共 4,499 个 variant、13,223 次进程调用、41 份 stage／plan golden。

追加核查 ETXTBSY：原测试并发重放 30 轮未再失败，受控 fork 实验则稳定复现了可写脚本句柄被其他子进程继承时的相同错误，即使写入线程已经关闭自己的句柄。三个测试假编译器的脚本创建与更新统一交给独立子进程，待写入进程退出再执行，测试主进程不持有该可写句柄；脚本内容、可执行权限、协议与缓存断言保留。macOS 格式检查、相关 crate 的 release all-targets Clippy 与 27 项定向测试通过；Linux 格式检查、release workspace all-targets Clippy 通过，29 项定向测试连续执行 30 轮、共 870 次全部通过。受控实验中的临时文件与子进程已清理，原始日志保留在仓库外。

Windows 首批四项 runtime 测试失败定位为验收脚本漏配测试要求的 unwind 路径，已补齐三套 C／unwind 工具配置；修复后 release workspace all-targets Clippy 与五个 CLI 构建通过。新批次发现的 23 项 HIR 快照失败已全部闭合：Windows checkout 将预期文件转换成 CRLF，逐项还原 LF 后内容全部相等。为 HIR 快照目录增加 `*.hir.txt text eol=lf`，在 `core.autocrlf=true` 下核对全部 23 份 Git checkout 投影与原文件逐字节相等；Windows 格式检查、相关 crate 的 release all-targets Clippy 及原 23 项测试定向复测通过，未改动快照正文或比较断言。Windows fixture runner 规则测试 80 项完成、3 项平台条件跳过，构建脚本测试 32 项完成、9 项平台条件跳过。

另有 provider 实例删除用例稳定失败于重新发布：测试仍持有旧 consumer 的 reader，Windows 拒绝覆盖该路径。已将旧消费者检查限定在重建之前的作用域，保留 Code 变化、旧消费者失效与重建后成功链接的断言；macOS 格式检查、相关 crate 的 release all-targets Clippy 和该项复测通过，Windows 格式检查、release workspace all-targets Clippy 与该项定向复测也通过。完整 Windows release workspace 批次和文档测试现已结束，原始日志汇总为 6,198 项通过、上述 24 项失败；失败均已定向闭合，最终没有新增失败。原始汇总包含嵌套子进程结果；两项带 ignored 标记的子进程入口由父测试实际调用，嵌套结果中的三项过滤并非排除正常验收。MSVC、MinGW MSVCRT、MinGW UCRT 的系统库和四种 runtime 现均准备完成，MSVC 全量 CLI 继续执行；两个尚未启动的 MinGW 全量 CLI 合并到速度阶段末验收。

两项 `.slib` 长路径发布失败也已闭合。真实 Rust／tempfile 独立复现显示：父目录为 244 字符时，目标文件为 259 字符，临时文件达到 275 字符；父目录为 310 字符时同样失败。两者均在普通路径的发布步骤报 `os error 3`，使用 Windows 完整绝对父目录后成功。`.slib` writer 在实际发布时统一临时文件与目标的目录表示，仍返回请求的产物路径，不改变格式、实体身份或缓存命中规则。macOS 格式检查、release workspace all-targets Clippy 和真实发布失败保护用例通过；Windows 格式检查、release workspace all-targets Clippy、长路径首次发布／覆盖／失败保留与清理的定向测试及五个 CLI 构建通过。原两项 fixture 的三个 variant、22 次进程调用全部通过，保留间接依赖切换 profile 后的重建、定义处编译期环境值、运行结果与缓存命中断言。

另有八项 Windows core 布局 fixture 在重负载下达到既有 300 秒时限。当前任务的后台 Rust 编译／测试和系统库准备调低优先级后，后续同类用例陆续通过；CLI 构建、MSVCRT 准备等任务结束后，以单并发定向复测原八项，八个 variant、八次实际编译全部通过，用时 227.00～240.61 秒。没有修改时限或功能断言，不将验收并发调整计为编译器提速。至此 MSVC 全量已出现的十项失败全部闭合，全量继续执行。

原始报告保留在仓库外，已经完成的 macOS、Linux GNU／musl 全量与定向 fixture 工作目录及过期编译缓存已清理；另清理 macOS 第五轮缓存和旧基线工作目录、Linux 第二轮 fixture 缓存与 sysroot 工作目录，以及 Windows 长路径、超时复测工作目录和独立复现程序。SDK、必要的基线产物和正在运行的 Windows 验收继续保留。

## 第十轮：Scoop 对象在内存中交接

进入编译速度阶段后，先对真实 HTTP 客户端的 debug／release 分别采样。对象临时文件的系统调用占主线程样本约 12.29%／15.88%，包括发射、常量与边界处理、driver 再次读取及最终清理；不把空闲信号线程的样本加入分母。Scoop LLVM 对象现在直接生成拥有所有权的原生字节，在内存中完成 Mach-O／ELF／COFF 后处理，再移动给 driver 打包。同时移交已有 foundation，省去一次完整复制。外部 C 编译、真实对象缓存与发布仍按需要使用文件。没有改变对象划分、优化等级、调试级别、GC／EH／ABI 检查或产物格式。

以相同 profile 15 预编译依赖、同一路径和源码分别测量 debug／release，每个组合三组旧／新交替构建；每次源码编译使用新缓存，之后验证原命令命中且没有 child 编译。下表为 macOS M3 Ultra 的中位数；保留 debug 默认 full、release 默认 none。

| 实际源码重编译 | 原版 | 内存交接 | 耗时降低 | 原版／新版范围 |
| --- | ---: | ---: | ---: | --- |
| `scoop.std` debug | 9.064 s | 7.383 s | 18.6% | 8.430～9.072 / 7.346～8.814 s |
| `scoop.std` release | 9.717 s | 8.283 s | 14.8% | 9.608～9.850 / 7.624～8.323 s |
| `scoop.http.client` debug | 75.530 s | 61.438 s | 18.7% | 75.510～82.584 / 61.354～61.924 s |
| `scoop.http.client` release | 79.954 s | 67.171 s | 16.0% | 79.149～90.260 / 66.659～67.353 s |

Linux `nuc12` 使用同一份改动、补丁版 LLVM 22.1.8 和 GCC，再对标准库执行三组成对测量：debug 11.707 → 11.146 s（4.8%），release 12.971 → 12.269 s（5.4%）。文件系统与宿主的实际收益不同，不把 macOS 的降幅推广为所有平台结果。

内存代价也计入：macOS 三次运行观测到的最大单进程 RSS，标准库 debug／release 分别增加 30.7／6.3 MiB；HTTP debug 从 13,156,073,472 增至 13,453,574,144 bytes，约增加 283.7 MiB，release 从 11,270,127,616 增至 11,421,990,912 bytes，约增加 144.8 MiB。Linux 标准库峰值基本持平。缓存命中中位数在 macOS 为 20～23.5 ms、Linux 为 12～14 ms；所有命中均没有启动编译 child。

macOS 两库的六组 release 产物与 Linux 标准库的三组 release 产物，完整文件逐字节相同。macOS 单组覆盖 34,946 个 release 原生对象，Linux 为 4,290 个。全部 debug／release 对照的 HIR／MIR／LIR payload 和语义指纹不变；macOS debug 的 39,871 个对象在三组对照中，机器段、重定位和相对于所属 section 的符号位置全部一致。

debug 原始文件存在已有的不确定性：原版标准库连续两次编译有 6 个对象不同，HTTP 有 261 个；差异限于 DWARF 内容及其大小导致的后续 section 基址变化。逐项定位发现 managed 变量失效记录遍历随机 HashMap，使变量、位置表和行表的发射顺序改变。该问题不记作本轮新增行为，也不宣称 debug 产物完全相同；下一项变更立即修复此错误，然后继续共有路径的大项。

macOS 与 Linux 的 release workspace all-targets Clippy 均通过；后端定向测试分别为 22／37 项，覆盖七目标的对象处理以及 Linux 原生链接／运行，两端各有 5 项真实泛型／跨产物测试通过。Darwin AArch64、Darwin x86_64（Rosetta）、Linux GNU、Linux musl 各执行 3 个正式 CLI 用例，合计 12 个 fixture、16 个 variant、116 次进程调用，全部通过；覆盖泛型异常控制流、间接依赖 profile 切换后的缓存更新与定义处编译环境值。未重跑全量 fixture，Windows 的原有全量继续收尾。

本轮原始测量与对照报告保留在仓库外。已经清理每次测量的构建缓存，以及完成的定向 fixture 工作目录和缓存；仅保留后续调试确定性修复与性能对照需要的工具及代表产物。

## 第十一轮：修复已有调试产物不确定性

第十轮逐对象对照发现两个已有错误，一并修复。首先，managed 变量失效记录从随机 HashMap 遍历改为按有类型存储身份排序，保留同一存储内的声明顺序。其次，invoke 正常返回清理块已经具有跳转指令，后补充的 declare／value 记录必须插在该指令之前；LLVM 22.1 的 AtEnd API 会将它们放到终结指令之后，留下 Context 中未附着的记录。复用 Context 时，这些旧记录会污染后面的对象。

第二项通过真实 HTTP 库定位：仅固定遍历顺序后，35,111 个对象仍有 4 个不同。对同一份保存的 LLVM IR 各执行 8 次独立 llc，输出一致；两次完整编译的优化前 IR 则混入了其他函数与文件的变量记录。修正实际插入位置后，继续复用 Context，无需更换 LLVM、清洗输出或增加完整语义复核。

新增直接验证终结指令前位置的后端测试，以及同一路径上三次独立 `scoopc build` 比较完整 `.slib` 的正式 CLI 用例，覆盖 debug 与 release/full。旧工具在第二次编译即复现差异；最终 macOS、Linux 的 release workspace all-targets Clippy、各 6 项后端调试测试全部通过。Darwin AArch64／x86_64、Linux GNU／musl 共 4 个 fixture、8 个 variant、24 次进程调用通过，未运行全量 fixture。

最终真实标准库与 HTTP 客户端各连续编译两次，debug 完整文件逐字节相同，分别为 8,816,180 和 115,595,790 bytes。与第十轮代表产物独立对照，39,871 个原生对象的机器段、重定位和 section 相对符号位置均相同，全部 HIR／MIR／LIR payload 与语义指纹不变；变化只涉及调试信息。HTTP 默认 release 的完整 50,710,800-byte 产物仍逐字节相同。本轮记录为既有正确性问题的闭合，不另行宣称速度或尺寸收益。

临时定位代码已移除，原始采样、LLVM IR 差异与验证结果移至仓库外。清理旧定位工具、失败复现的构建缓存、重复产物和已完成的定向 fixture 工作目录；保留最终工具与代表产物作为后续并行代码生成的基线。

## 第十二轮：修复 Windows fixture 的长路径启动

Windows 已启动的全量验收又遇到三项 include 定义处产物用例。已有长路径发布修复使其成功生成产物，但使用较长的复测工作目录时，runner 启动 261 字符路径的 consumer 可执行文件出现 WinError 206。独立调用同一份已生成程序证明：只从命令行解析程序名会触发 MAX_PATH 限制，显式传递 CreateProcessW 的应用名则正常返回并输出正确结果。

Windows 进程入口现对绝对路径显式传递 executable，继续保留原参数、工作目录和 Job 中的后代回收；相对命令沿原搜索行为启动。新增从超过 260 字符路径实际启动程序的回归测试，修改前稳定复现 WinError 206。固定版本 Ruff 格式与 lint 通过；公共规则共 81 项，macOS 77 项通过、4 项平台不适用，Windows 78 项通过、3 项平台不适用，包含全部 4 项 Windows 原生进程测试。

三项原失败 include 用例在更长路径上定向复测，3 个 fixture、5 个 variant、20 次实际进程调用全部通过。原始失败、复现与成功日志保留在仓库外，两个已完成的复测工作目录已清理，共享验收缓存继续供原有全量使用；未启动新的全量 fixture。本项修复不计入编译速度收益。

Windows MSVC 原全量随后完成：4,328 个 fixture 中 3,436 项通过、878 项平台不适用、14 项首次失败，共 3,462 个 variant、9,680 次进程调用。14 项失败均已逐项对应成功复测记录：六项长路径相关失败、八项重负载超时；其中最后出现的 release-blocks 缓存失效用例也在已有发布修复下通过，覆盖 1 个 variant、16 次进程调用。没有遗留失败，也没有重跑全量；原始报告和失败闭合映射保存在仓库外。

## 第十三轮：修复共享字典压缩的不确定性

并行编译的重复对照发现，HTTP debug 产物偶尔相差 119 bytes。独立读取确认 35,111 个原生对象、三层 metadata、字典和语义指纹全部相同，差异只在一个压缩帧。使用同一个 135,064-byte 对象和 64 KiB 字典的受控 C 实验复现：Zstandard 1.5.7 的原始字典缓冲区与输入偶然相邻时，匹配路径改变，分别生成 60,235／60,116-byte frame。

生产端改用已有 safe API 创建独立拥有内容的预编译字典，与压缩上下文一起按批复用；小批次沿普通无字典路径压缩。仍使用 level 3、相同训练样本和 64 KiB 上限。受控实验中，拥有内容的字典在两种布局下均生成 60,235 bytes。原有确定性往返测试补充中、大输入，覆盖附着字典、复制匹配表与重新加载字典的实际路径。

macOS、Linux 的 release workspace all-targets Clippy 和各 4 项对象字典测试通过。最终四目标正式 CLI 的确定性、并行 Cone 与泛型异常控制流用例全部通过，验证范围在下一轮汇总。真实库的重复构建也恢复完整文件一致。

独立脚本进一步比较修复前后的五组代表产物：macOS 标准库 debug／release、HTTP debug，以及 Linux 标准库 debug／release。全部 53,212 个原生对象、248,722,408 bytes 原始内容、三层 metadata payload、字典和五种语义指纹保持相同；仅 6 个压缩帧改变。macOS HTTP debug 实际文件从 115,609,708 增至 115,647,593 bytes，其他四组分别增加 1,115～2,663 bytes。这是编码确定性修复，不作为尺寸或编译速度收益。

最小字典／对象／压缩帧复现与原始验证报告移至仓库外，临时 C 实验程序、失败产物及已完成的独立对照副本已清理。

## 第十四轮：在统一 jobs 上限内并行发射 callable

独立 callable 的 LLVM module 现在由多个 worker 发射，每个 worker 独占 Context 和 TargetMachine，只读共享 LIR、符号规划、声明索引及 GC effect 表。逐对象 passes、物理分区与 GC／异常展开合同保持；完成结果按原 canonical 身份排序，错误也按原顺序选择。所有已启动线程完成后再处理元数据分区，后者继续使用原串行路径。

`scoop build/run/test --jobs` 同时限制 Cone 调度与 Cone 内代码生成。父进程按就绪节点分配剩余 worker，所有活跃 child 的份额之和不超过指定值；单个就绪 Cone 可以取得全部空闲份额。直接 `scoopc build` 也接受 `-j/--jobs`。此执行参数不进入 machine CBOR、缓存 key、语言身份或产物格式，命中与等待锁的节点不占用编译份额。

采用同一组最终 release 编译器，比较 jobs=1 与宿主并行度（macOS 32、Linux 20）。每项执行三组交替顺序的源码重编译，使用相同路径与预编译依赖，每次独立清空构建缓存；随后检查一次热构建。下表为三次墙钟中位数，收益仅归于并行发射。

| 宿主与源码 Cone | Scoop profile | 单 worker | 并行 worker | 耗时减少 |
| --- | --- | ---: | ---: | ---: |
| macOS `scoop.std` | debug | 7.319 s | 6.485 s | 11.4% |
| macOS `scoop.std` | release | 8.280 s | 6.614 s | 20.1% |
| macOS `scoop.http.client` | debug | 63.868 s | 47.274 s | 26.0% |
| macOS `scoop.http.client` | release | 68.318 s | 44.320 s | 35.1% |
| Linux `scoop.std` | debug | 11.346 s | 10.002 s | 11.8% |
| Linux `scoop.std` | release | 12.254 s | 9.958 s | 18.7% |

36 次源码重编译均确认只有目标 Cone 启动编译 child；对应 36 次热构建均命中且没有 child。每种宿主／Cone／profile 的六份完整 `.slib` SHA-256 全部相同。热构建中位数为 macOS 19.29～24.36 ms、Linux 13.34～13.56 ms，未将它们计为本轮收益。

三次运行中最大的单进程 RSS，macOS 标准库 debug／release 分别增加 39.6／39.9 MiB；HTTP debug 从 13,298,155,520 增至 13,904,576,512 bytes，增加 578.3 MiB，release 从 11,384,602,624 降至 11,318,640,640 bytes。Linux 标准库 debug／release 分别增加 77.3／65.2 MiB。独立 LLVM context 带来可见的并行内存成本，使用同一个 jobs 上限控制，不另加压缩线程池。

macOS、Linux 的 release workspace all-targets Clippy 均通过；调度、后端与真实泛型／跨产物定向测试分别为 52／107 项通过。新增后端对照在七目标的 debug、release/full 下共执行 56 次对象发射，逐项比较完整 bytes、分区和生产记录。调度测试覆盖独立 Cone 的份额总和、单 Cone 使用空闲 worker、重复根与锁等待、canonical 错误选择和缓存 key 不受 jobs 影响。

正式 CLI 使用最终工具，在 Darwin AArch64、Darwin x86_64（Rosetta）、Linux GNU、Linux musl 各执行确定性、并行 Cone、泛型异常控制流三个 fixture，合计 12 个 fixture、20 个 variant、108 次进程调用，全部通过。确定性用例在同一路径上分别使用 jobs=1、4、2，检查 debug 与 release/full 的完整文件相同。没有重跑全量 fixture；Windows 原有全量的失败已在第十二轮记录中全部闭合，其工作目录也已清理。

原始配对测量、独立解压对照与 CLI 报告保存在仓库外。每次测量的构建缓存、已完成的定向 fixture 工作目录与缓存、旧实验工具及重复产物已清理；保留最终工具和每种 profile 的一份代表产物，供下一轮共有路径优化对照。

## 第十五轮：只为链接选中的对象写入缓存

此前每次 `.slib` 生产都会把全部原生对象预填充到 objects-v1，即使只请求库产物也执行数千至数万次文件创建和发布。现在直接发布完整容器，复用链接器已有的按需提取路径：完成可达性与完整 ODR 选择后，只为实际交给 native linker 的对象物化文件，已有对象文件直接复用。同步删除归档和成员中为预填充保留的第二份原生对象集合，以及仅用于该路径的 child 参数与 request 字段；外部 C 编译及其缓存照常工作。

使用上一轮与本轮最终 release 编译器，同一路径、相同源码和预编译依赖，jobs 固定为宿主并行度（macOS 32、Linux 20）。每项运行三组交替顺序的对照；每组先冷编译库，再以真实库产物冷编译并链接消费者。下表的两步合计是各组两条命令耗时之和的中位数，不是单条构建命令，也不是两个中位数相加。

| 宿主与源码 Cone | Scoop profile | 库编译前 → 后 | 库耗时减少 | 消费者冷构建前 → 后 | 两步合计前 → 后 | 合计耗时减少 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| macOS `scoop.std` | debug | 5.982 → 5.286 s | 11.6% | 1.931 → 2.022 s | 7.905 → 7.307 s | 7.6% |
| macOS `scoop.std` | release | 6.275 → 5.663 s | 9.8% | 1.814 → 1.925 s | 8.089 → 7.569 s | 6.4% |
| macOS `scoop.http.client` | debug | 46.455 → 39.566 s | 14.8% | 20.370 → 19.849 s | 67.123 → 59.415 s | 11.5% |
| macOS `scoop.http.client` | release | 43.923 → 38.392 s | 12.6% | 19.809 → 19.488 s | 63.755 → 58.087 s | 8.9% |
| Linux `scoop.std` | debug | 9.908 → 9.811 s | 1.0% | 2.363 → 2.393 s | 12.270 → 12.222 s | 0.4% |
| Linux `scoop.std` | release | 9.989 → 9.952 s | 0.4% | 2.274 → 2.289 s | 12.263 → 12.253 s | 0.1% |

macOS 标准库消费者的冷构建增加约 0.09～0.11 s，已经计入两步合计；收益没有只按库发布计算。Linux 这组标准库的时间差异很小，不视为明确加速，也不外推 Windows 性能。各项库编译后的对象缓存文件均降为零；macOS HTTP debug／release 的完整两步流程分别从 40,180／35,329 个对象文件降至 5,187／4,894 个。

全部 72 次源码冷构建均确认只启动目标 Cone 的一个编译 child。每种宿主／Cone／profile 的库和消费者分别比较六份完整 `.slib`，12 组 SHA-256 各组全部相同，产物尺寸不变。对应 72 次热构建均命中且没有编译 child，消费者的冷／热 link plan 指纹一致；36 次真实程序运行分别得到预期的 `2` 或 `true`。热构建中位数为 macOS 19.17～25.29 ms、Linux 12.67～15.20 ms，未计为本轮收益。

三次中最大的单进程 RSS，macOS 库编译的变化范围为 -9.1～+76.8 MiB，标准库消费者减少 6.5～11.2 MiB，HTTP debug／release 消费者减少 2.9／247.8 MiB；Linux 各项增加 0.1～1.1 MiB。该指标不代表整个进程树的同时驻留内存，也不据此宣称整体编译峰值下降。

macOS、Linux、Windows 的 release workspace all-targets Clippy 均通过；归档、字典、child protocol、调度与真实泛型／跨产物的定向 Rust 测试分别通过 43、43、23 项。调度测试本身为 Unix 专用，Windows 不将零项匹配算作通过。新增正式 CLI 用例覆盖 debug／release 的库冷／热发布不写对象缓存、消费者实际提取链接并运行、消费者热命中；旧工具在库发布后的空缓存断言处失败，确认该用例能够区分原行为。

Darwin AArch64、Darwin x86_64（Rosetta）、Linux GNU、Linux musl 同时覆盖直接 C、native C 与 ODR 支持缓存；Windows MSVC、MinGW/MSVCRT、MinGW/UCRT 覆盖静态库与 DLL。新增用例初稿的包名和版本约束配置已修正，仅复测对应失败项；七目标最终合计 22 次 fixture 执行、41 个 variant、249 次成功进程调用全部通过，未重跑全量 fixture。

原始配对测量、各目标验证日志和报告保存在仓库外。临时构建缓存、失败复现与已完成的 fixture 工作目录、重复产物和退役对照工具已清理；保留本轮最终工具和每种 profile 的一份库／消费者代表产物，供后续共有路径优化对照。

## 第十六轮：修复 LLVM 对象销毁顺序

Windows 的七目标并行对象对照暴露了既有原生崩溃。保存失败二进制后，GDB 捕获到 AArch64 TargetMachine 析构中的 `MetadataTracking::untrack` 访问已释放的调试 metadata；worker 原先先创建 TargetMachine、后创建 Context，Rust 的逆序析构使 Context 提前释放。该顺序问题已存在于第十四轮并行发射实现，发生与否受分配布局和调度影响。

callable 与元数据发射入口现先创建 Context、后创建 TargetMachine，使后者在正常返回、错误返回和栈展开时都先销毁。IR 输出入口按实际 callable／元数据阶段分别使用词法作用域，上一阶段完整释放后再进入下一阶段；数据对象对照测试也采用相同顺序。没有改变线程数、对象划分、LLVM passes 或产物合同。

macOS、Linux、Windows 的 release workspace all-targets Clippy 均通过；各平台的对象分区、GC／异常记录、IR 输出与数据对象对照 15 项、真实泛型与跨产物消费 4 项全部通过。其中既有并行对照在每个宿主覆盖七目标、debug 与 release/full、四次不同 jobs 的发射，比较完整对象 bytes、分区与生产记录。Windows 另以原始参数独立运行该对照三次，并使用捕获崩溃时的普通堆与 GDB 配置运行一次，均正常退出。

本轮作为已有正确性问题的修复，不记录编译速度收益，也未重跑全量 fixture。原始崩溃堆栈、带诊断输出的旧版运行结果和修复验证日志保存在仓库外；临时崩溃二进制及已完成的初步索引实验目录已清理。保留修复后的 macOS／Linux 工具，下一轮索引优化以它们为基线重新测量。

## 第十七轮：为 production 的 digest 查询建立索引

callable、safepoint、类型、静态存储、初始化、入口和 runtime image 的注册构造原先反复线性扫描完整 digest 节点表。现在为同一计划建立按完整 typed key 排序的节点位置索引，查询改为二分；节点本身仍按原 canonical 身份排序，索引不进入持久化数据。codegen 删除不再发射的 safepoint 时同步重建位置索引并清理输入边。构造入口也去掉一次已包含在 `validate_plan` 内的重复节点检测，保留原错误优先级。

以下对照的前后工具均包含第十六轮生命周期修复。两边使用相同路径、源码和预编译依赖，jobs 固定为 macOS 32、Linux 20，各执行三组交替顺序的冷构建。每组先编译库，再编译、链接实际消费者；两步合计取各组两条命令耗时之和的中位数。

| 宿主与源码 Cone | Scoop profile | 库编译前 → 后 | 库耗时减少 | 消费者冷构建前 → 后 | 两步合计前 → 后 | 合计耗时减少 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| macOS `scoop.std` | debug | 5.351 → 5.268 s | 1.5% | 1.999 → 1.988 s | 7.399 → 7.232 s | 2.3% |
| macOS `scoop.std` | release | 5.628 → 5.594 s | 0.6% | 1.818 → 1.792 s | 7.468 → 7.424 s | 0.6% |
| macOS `scoop.http.client` | debug | 38.262 → 36.362 s | 5.0% | 18.666 → 18.376 s | 56.928 → 54.879 s | 3.6% |
| macOS `scoop.http.client` | release | 37.690 → 35.397 s | 6.1% | 18.459 → 18.319 s | 56.079 → 53.694 s | 4.3% |
| Linux `scoop.std` | debug | 9.784 → 9.776 s | 0.1% | 2.383 → 2.370 s | 12.189 → 12.145 s | 0.4% |
| Linux `scoop.std` | release | 9.868 → 9.886 s | -0.2% | 2.275 → 2.284 s | 12.143 → 12.170 s | -0.2% |

明确收益集中在较大的 HTTP 客户端：库编译减少 5.0%／6.1%，连同消费者的两步合计减少 3.6%／4.3%。标准库的变化较小，Linux 的库编译差异不到 0.2%，不视为明确加速；不据此推断 Windows 性能。

72 次源码冷构建均只启动目标 Cone 的一个编译 child；对应 72 次热构建均命中且没有编译 child，消费者冷／热 link plan 指纹一致。库与消费者共 12 组完整 `.slib`，各组六份 SHA-256 全部相同，尺寸不变；36 次实际运行均得到预期的 `2` 或 `true`。首次库构建还与第十五轮代表产物比较完整文件，确认生命周期修复也未改变产物。热构建中位数为 macOS 19.15～24.37 ms、Linux 12.92～14.75 ms，未计为本轮收益。

按各组三次测量中的最大单进程 RSS 报告，macOS 标准库 debug／release 增加 3.1／18.6 MiB，HTTP 库增加 19.4／221.2 MiB；HTTP 消费者分别减少 18.5 MiB、增加 72.9 MiB。Linux 各项变化为 -0.5～+0.4 MiB。索引只存储节点位置，不复制节点或它们的 key。

三个宿主的 release workspace all-targets Clippy 均通过，production、后端和真实泛型／跨产物定向 Rust 测试各 134 项通过。新增三项回归覆盖 key 排序与 canonical 排序不同、重复节点的原诊断，以及删除 safepoint 后索引和剩余输入边的一致性。后端继续在七目标、两配置和不同 jobs 下比较完整对象 bytes、分区与生产记录。

Darwin AArch64、Darwin x86_64（Rosetta）、Linux GNU、Linux musl、Windows MSVC、MinGW/MSVCRT、MinGW/UCRT 的正式 CLI 确定性与泛型异常控制流用例全部通过，合计 14 次 fixture 执行、21 个 variant、91 次进程调用，未重跑全量 fixture。独立于计时的六次 HTTP 库／消费者采样构建也保持完整产物一致，四次消费者运行成功。最终工具的采样显示元数据分区仍是 debug／release 共有的主要串行热点，消费者还存在明显的后端依赖数据读取和链接准备开销。

原始配对计时、采样和七目标验证报告保存在仓库外。已清理每次测量与采样的缓存、完成的 fixture 工作目录与缓存、重复产物及退役工具；保留最终工具、每种 profile 的一份库／消费者代表产物，以及下一轮继续使用的小型消费者源码。

## 第十八轮：并行裁剪与发射元数据对象

元数据分区在 callable 生产记录完成后，沿原来的二分树先准备不超过 jobs 份额的独立子树，再通过内存中的 LLVM bitcode 转交独占 Context／TargetMachine 的 worker，继续原有裁剪和对象发射。转交保留 module identifier、source filename、布局与调试 metadata，按原对象顺序汇集结果和错误；单 worker 继续使用原串行路径。每个对象仍独立建立发射 passes，原模块在完成子树准备后释放，不增加持久化数据、文件中转或新的并发开关。

以第十七轮最终工具为基线，使用相同源码、路径和预编译依赖；macOS jobs 为 32，Linux 为 20。各组合三组交替顺序对照，包含全部子树准备、bitcode 编解码与线程启动成本。每组先冷编译库，再冷编译、链接消费者；两步合计取各组两条命令耗时之和的中位数。

| 宿主与源码 Cone | Scoop profile | 库编译前 → 后 | 库耗时减少 | 消费者冷构建前 → 后 | 两步合计前 → 后 | 合计耗时减少 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| macOS `scoop.std` | debug | 5.177 → 4.862 s | 6.1% | 2.006 → 2.071 s | 7.183 → 6.918 s | 3.7% |
| macOS `scoop.std` | release | 5.568 → 5.243 s | 5.8% | 1.835 → 1.831 s | 7.403 → 7.069 s | 4.5% |
| macOS `scoop.http.client` | debug | 36.705 → 32.152 s | 12.4% | 18.560 → 17.833 s | 55.140 → 50.265 s | 8.8% |
| macOS `scoop.http.client` | release | 35.617 → 31.581 s | 11.3% | 18.416 → 17.767 s | 54.055 → 49.399 s | 8.6% |
| Linux `scoop.std` | debug | 9.799 → 9.089 s | 7.2% | 2.362 → 2.313 s | 12.162 → 11.397 s | 6.3% |
| Linux `scoop.std` | release | 9.890 → 9.166 s | 7.3% | 2.284 → 2.228 s | 12.174 → 11.390 s | 6.4% |

两种配置均有明确收益，较大的 HTTP 库编译耗时减少 12.4%／11.3%，连同消费者的两步合计减少 8.8%／8.6%。macOS 标准库 debug 消费者增加约 0.065 s，已包含在两步合计中；本轮没有测量 Windows 编译耗时。

72 次源码冷构建均只启动目标 Cone 的一个编译 child；对应 72 次热构建均命中且没有编译 child，消费者冷／热 link plan 指纹一致。库与消费者共 12 组完整 `.slib`，各组六份 SHA-256 全部相同，尺寸不变；36 次实际运行均得到预期的 `2` 或 `true`。热构建中位数范围为 macOS 18.90～24.42 ms、Linux 12.56～15.24 ms。

三次测量中的最大单进程 RSS，macOS HTTP debug／release 库分别减少 100.6／43.2 MiB，消费者减少 18.6／187.3 MiB；标准库增加 2.2～16.3 MiB。Linux 标准库的库编译增加 13.1～16.0 MiB，消费者增加 0.3～0.9 MiB。该指标不代表整个进程树的同时驻留峰值。

三个宿主的 release workspace all-targets Clippy 和各 20 项对象分区／数据对象／真实泛型与跨产物定向 Rust 测试均通过。新增纯元数据入口对照以并行发射开场，另在独立测试进程中先执行一次；它和已有混合对象对照共同覆盖七目标、debug 与 release/full 及不同 jobs，逐项比较完整 bytes、分区和生产记录。七目标的正式 CLI 确定性及泛型异常控制流全部通过，合计 14 个 fixture、21 个 variant、91 次进程调用。

原始测量与七目标报告保存在仓库外。配对构建缓存、临时消费者源码、重复产物、退役基线工具及已完成的定向 fixture 工作目录和缓存已清理；保留本轮五个工具用于阶段末验收。按用户最新要求，本次以已确认有显著收益的大项收尾，接下来完成统一验收并修复发现的问题。

## 编译速度阶段验收（进行中）

Linux 完整 release workspace 的 105 个测试组首次为 6,383 项通过、1 项失败，失败项为 `child::tests::production_runner_preserves_linux_tools_and_uses_private_scratch`。测试仍把 child 的第 4 个参数当成临时目录，第十五轮精简协议参数后该位置已是 jobs 数值。测试现按实际 `TMPDIR` 和独立指定的 scratch 根检查路径，保留 GNU／musl 工具环境、真实调用、目录存在与结束清理检查。macOS、Linux 的格式与 release workspace all-targets Clippy 通过；Linux 原失败用例的两个目标分支定向复测通过，未重跑完整 workspace。fixture runner 规则为 77 项通过、4 项平台条件跳过，构建脚本为 27 项通过、5 项平台条件跳过。Linux GNU／musl 的完整 CLI 验收及其失败项闭合现已完成。


macOS 完整 release workspace 的 105 个测试组共 6,353 项通过，无失败或忽略；fixture runner 规则为 77 项通过、4 项平台条件跳过，构建脚本为 26 项通过、6 项平台条件跳过。CLI 均使用各宿主冻结的最终五个工具，每目标只执行一次全量，原始失败记录保留，修复后只复测相关项。

Windows 完整 release workspace 原始汇总为 106 个测试组、6,229 项通过、零失败，包含嵌套子进程结果。两个标记 ignored 的子进程入口由父测试实际执行，三项过滤来自该次子进程的入口选择；没有排除正常测试。fixture runner 规则为 78 项通过、3 项平台条件跳过，构建脚本为 23 项通过、9 项平台条件跳过。同步本阶段收尾修复后，Windows 格式检查、release workspace all-targets Clippy 和 61 项包管理器测试均通过，包含新增的三个真实 HTTP 回归测试。最终五个 CLI 已重新冻结。按用户要求，MinGW UCRT 与 MSVC 先以每目标 2 个 fixture worker 并行验收；UCRT 完成后，MSVC 剩余用例提高到 8 个 worker。本阶段只验证正确性，不将调度变化计为编译提速。

MinGW MSVCRT、MinGW UCRT 两目标的首轮完整 CLI 均覆盖 4,322 个 fixture：每目标 3,444 项通过、878 项平台不适用、零失败，各有 3,475 个 variant、9,730 次进程调用和 1 项 stage／plan golden 检查。完整报告包含实际编译和运行；五工具版本及已有长路径修复相关用例均通过，不需要复跑。878 项不适用来自 fixture 的既有平台条件，未计入通过数。`constructor-this` Cone 在 MSVCRT、MinGW UCRT、MSVC 各有 10 项真实行为测试通过。

MSVC 调整并发前正常保存了完整的逐项结果：2,331 项通过、878 项平台不适用，尚未完成的 1,113 项标记为 interrupted。仓库外的一次性脚本保留这些结果，以原运行器和相同源码、冻结工具、sysroot 及共享缓存按 8 路并发接续未完成项；原报告、原日志和接续清单分别保留。未重跑已通过用例，也没有修改 fixture 规则或断言。MSVC 的剩余验收继续执行。

Darwin AArch64 首轮完整 CLI 为 4,290 项通过、25 项平台不适用、8 项失败，共 4,514 个 variant、13,391 次进程调用。已逐项闭合这些失败：

- 主线允许构造期间使用普通 `this` 后，`@NoGC` 构造结果用例漏记 receiver local／value 的两条既有类型诊断，现补齐精确期望。原来拒绝继承计算属性初始化的用例已经不符合语言规则，将其迁入 `constructor-this` Cone，以真实 provider 产物和实际运行断言验证继承 getter 求值；删除退役的 negative fixture，当前集合因此为 4,322 项。
- hash、linked、queue、tree 四个 GC 用例不再单独重编 core 后混用另一组预编译 std。它们直接消费配套 sysroot；保留原 profile、stress runtime 选择、正常／搬迁运行及实际回收断言。GC stress 不改变普通库产物身份，不需要为该选项重编 core。
- `object-files` 改为检查库发布不预填充对象缓存，保留真实源码变更、选中对象提取、重复链接不改写文件和程序结果检查。删除人为破坏对象缓存后的恢复验收；实现规范的链接章节也同步去掉遗漏的生产期预填充描述，与第十五轮及当前实现一致。
- macOS 验收解释器补装仓库 `tests/requirements-dev.txt` 已固定的 `cbor2==6.1.5`，闭合 deployment 检查的环境缺项，没有增加项目依赖。

修复经过 Python 格式与 lint、TOML 解析及差异检查；两组定向 CLI 共 9 项、10 个 variant、37 次进程调用全部通过，迁移后的构造 Cone 10 项真实行为测试全部通过。原始全量与复测日志、报告保存在仓库外；已完成的 Darwin AArch64 fixture 工作目录清理后，继续其余目标验收。

Linux GNU 首轮完整 CLI 为 4,273 项通过、43 项平台不适用、7 项失败，共 4,494 个 variant、13,241 次进程调用。七项均为上述构造语义、四个 GC 用例和对象缓存测试问题；修复后六个保留用例、6 个 variant、28 次进程调用全部通过。迁移后的 `constructor-this` Cone 在 GNU、musl 及两种 Darwin 架构各有 10 项真实行为测试通过。GNU 全量与复测工作目录已经清理，保留共享缓存供 musl 验收使用。

Darwin x86_64 首轮完整 CLI 为 4,296 项通过、25 项平台不适用、1 项失败，共 4,520 个 variant、13,408 次进程调用。唯一失败为 `terminal-input`：同一个已生成程序在 Rosetta 下重复运行 12 次，有 6 次在 full GC 后漏掉一次性 `SIGWINCH` 处理器调用。未删除 `gcCollect()`、信号断言或 Rosetta 覆盖。

独立 C 程序使用真实 PTY、kqueue 和相同信号处理模式，在 `madvise(MADV_DONTNEED)` 与异步 resize 并发时复现该错误；ARM 正常，Rosetta 失败。`MADV_FREE` 同样失败，不采用。Darwin 的 ThreadVmOps 改由 `mach_vm_behavior_set(VM_BEHAVIOR_DONTNEED)` 提交相同建议，并显式保留原 `madvise` 对非页对齐起始地址的拒绝，避免 Mach 自动向下取整。页归还仍可延迟，重用前初始化、相邻存活页、失败重试和 GC／runtime ABI 均遵守原合同。

新增原生回归测试直接链接实际 Darwin VM 实现，以 30 次异步一次性信号交付覆盖 handler、signal mask、页重用、相邻页和无效输入。测试在可用时实际探测并运行 Rosetta；本机两个架构均完成。使用旧实现编译同一个最终测试稳定失败，新实现通过。格式检查、release workspace all-targets Clippy 和该 Rust 测试通过。只重新准备两种 Darwin 目标的四种 runtime 配置；GC／终端定向 CLI 每目标为 80 项通过、3 项平台不适用、159 个 variant、531 次进程调用。补齐地址对齐检查后的页回收／终端复测每目标为 14 项通过、1 项平台不适用、18 个 variant、74 次进程调用，最终终端程序每目标连续 12 次运行通过。本项为已有正确性错误修复，不计入编译提速收益。

Linux musl 首轮完整 CLI 为 4,275 项通过、46 项平台不适用、1 项失败，共 4,494 个 variant、13,214 次进程调用。唯一失败发生在 `pm-static-registry-build` 读取静态注册表的包索引时，HTTP 客户端返回 `Peer disconnected`，尚未进入编译。受控 HTTP/1.0 服务器在发送带 Content-Length 的响应后，等客户端复用连接再关闭它，稳定复现相同错误。

注册表 JSON 与归档 GET 共用一个局部处理函数：仅在完整响应头到达前遇到 EOF、连接重置或断管时，用新连接重试一次。普通连接继续复用；第二次失败、HTTP 状态错误和正文截断照常报告，未增加依赖、重新求解或缓存命中时的网络请求。新增三项真实 TCP 测试分别验证关闭时序恢复、只重试一次，以及 HTTP 错误／截断正文不重试。macOS、Linux 的格式检查、release workspace all-targets Clippy 和各 62 项包管理器测试通过。两种 Darwin 与 GNU／musl 的静态注册表消费及五工具版本检查共八项定向 CLI 全部通过；musl 又用三个独立目录重复完整下载／构建／锁定缓存流程，均通过。

原始全量报告、失败日志、独立复现及定向复测记录保留在仓库外。已经退役的尺寸／速度对照工具、重复库和缓存清理约 20.5 GiB；Darwin 验收及诊断工作目录在修复完成后清理，保留 SDK、配套 sysroot 与后续验收所需工具。 Linux 全量、定向复测及退役基线目录也已清理；LLVM 安装保留，已不再使用的 LLVM／unwind 构建目录与源码压缩包删除。四个非 Windows 目标的全量失败均已闭合，Windows 验收继续进行。

## 后续优先级

尺寸优化按用户补充要求，优先精简结构、删除不必要或重复的信息；压缩只作为最后的补充，且不采用 CPU／内存消耗过大的算法。

1. 尺寸阶段已收尾，编译速度阶段已落地 Scoop 对象内存交接、独立 LLVM Context 的 callable 并行发射、链接选中后的对象按需落盘、production digest 查询索引和元数据分区并行化，并修复实际遇到的调试信息、压缩、Windows 路径及 LLVM Context 生命周期错误。按用户最新要求，完成七目标统一验收和发现问题的修复后结束本次任务；两个 MinGW 目标此前尚未执行的全量 CLI 一并完成。依赖读取与链接准备的后续研究，以及下列较小的尺寸候选，留待后续优化。
2. 当前完整 29 库的 HIR／MIR／LIR 字符串表投影仅省约 1.05～1.06 MiB，其中 LIR 约 0.04～0.08 MiB。注册记录 ABI 去掉重复 group／member hash 输入的投影约省 1.93 MiB，该估算尚保留旧索引，仅用于候选排序。跨三层 metadata 的重复 ID 原始字节上限为 7.03 MiB，尚未扣映射开销；全局 union 会让只请求 HIR 的读取载入大量 LIR 专用 ID，先保留阶段局部读取。这些候选后置。manifest 的全量 ID 表此前反而增加约 2.10 MiB，不采用。

已排除直接复用 LLVM 22.1 的对象发射 pass manager：即使 AsmPrinter／MCStreamer／MachineModuleInfo 有逐模块收尾，带 GC statepoint 与异常展开的第二个函数模块仍残留前一模块的栈映射记录；仅数据流水线在 COFF debug → 无 debug 切换时也残留字符串表空间。独立的连续模块字节对照发现两者，实验实现均已撤下，未进入生产。本轮仅补齐数据发射入口自身的 LLVM pass 注册，并将测试顺序调整为先调用数据入口，避免测试依赖完整代码生成的先行初始化。格式检查、workspace all-targets Clippy 和七目标两配置的数据对象对照通过；不为撤下的实验记录性能收益。

本机原始命令、采样、逐轮计时和验收日志保留在仓库外的 `/Volumes/Data/home/chenxu/scoop-performance-results/compiler-20261101`；Linux 日志在 `~/repos/scoop-work/compiler-20261101`。工作缓存与对照二进制暂放本仓库 `tmp/compile-speed-20261101`，只保留下一轮实际需要的内容。
