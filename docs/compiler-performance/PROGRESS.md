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

## 尺寸阶段统一验收（进行中）

共享字典提交后，macOS 完整 release workspace 的 105 个测试组、6,347 项测试全部通过，没有失败、忽略或过滤。fixture runner 规则测试 80 项完成、3 项平台条件跳过，构建脚本测试 32 项完成、6 项平台条件跳过。AArch64 正式 CLI 全量执行 4,327 个 fixture，首次为 4,282 项通过、25 项平台不适用、20 项失败，共 4,502 个 variant、13,374 次进程调用。两个 Darwin 目标的系统库和四种 runtime 均已按 schema 6 准备。

已闭合其中 15 项：六个动态库用例的独立 link 命令遗漏公共 `link_args`，没有使用 build 的显式 C compiler／SDK 配置；补齐后保持原有计划指纹相等、链接、运行及五份专用 plan golden 断言。七个错误诊断用例同步主线已修复的错误恢复行为，移除派生的“未知名字”重复错误，保留首个类型／初始化错误及位置；不可调用整数现在断言明确的不可调用诊断。另同步一项 manifest 错误的新版源码片段显示。工具版本检查的一次失败来自本轮并行 Cargo 测试重建了部分 CLI，统一构建五个工具后闭合；后续 CLI 验收使用同一组工具，并与其重建错开。以上 15 项定向复测全部通过，覆盖 15 个 variant、63 次进程调用。

其余五项也已闭合。第三轮把物理 ODR 可用性混入 LIR 语义指纹，但优化会增删内部 callable、layout、descriptor 和 scan；真实 core 的 debug／release 产物相差 174 条非私有记录，不能据此拒绝语义与 ABI 相容的组合。本轮删除这项投影，优化前后 core 的 HIR／MIR／LIR 指纹现在全部相同，原有链接定义与 ABI 检查继续生效。

源码编译缓存改用已有 Code 指纹跟踪对象生产变化，并采用与 child 请求相同的完整 direct／support 依赖投影。只看直接依赖不足：中间库的 Code 可以保持不变，而下层库已经改变可用实现。新增正式 CLI 用例保持源码与直接依赖的预编译产物不变，只切换间接依赖的优化 profile，确认顶层在 debug／release 下都重新编译，下一次构建恢复命中。机器代码变化也会使相关源码缓存更新；Code 不成为预编译消费者的语义相等条件，不增加独立可用性摘要、目录扫描或命中复核。profile 升为 `cross-cone-generic/15`，cache key domain 升为 v11。

release workspace all-targets Clippy、264 项 slib、75 项 scoop 和 4 项真实泛型／ODR 测试完成；更新固定向量及配套工具后仅复测原失败项。12 个定向 fixture、15 个 variant、113 次进程调用全部通过，包括原来失败的五项、间接依赖缓存、泛型发布／读取与 ODR 选择。ODR 选择用例同步检查依赖 Code 改变后的重编译、重新链接和新产物的后续命中，不再错误要求消费者保持旧的物理成员集合。AArch64 29 库与四种 runtime 重新准备完成；独立检查确认全部 165,487 个原生对象及 296,503,136 bytes 原始内容、230,044 条 ODR 记录与物理 bundle 保持一致。总尺寸为 258,702,533 bytes，指纹改变后的 manifest 压缩波动比第九轮少 14 bytes，不计为新的优化收益。

AArch64 首次全量的 20 项失败均已闭合，没有重跑整套 fixture。macOS x86_64 使用修复后的 profile 15 与配套五个 CLI 完成全量：4,328 个 fixture 中 4,303 项通过、25 项平台不适用，没有失败，共 4,525 个 variant、13,417 次进程调用、55 份 stage／plan golden。两个 Darwin 目标的 29 库与四种 runtime 均已更新为 profile 15。

Linux 修复后完整 release workspace 的 105 个测试组、6,377 项测试全部通过，没有失败、忽略或过滤；包含此前遇到临时脚本执行 ETXTBSY 的原测试。fixture runner 规则测试 80 项完成、3 项平台条件跳过，构建脚本测试 32 项完成、5 项平台条件跳过。GNU／musl 的系统库与四种 runtime 均已准备完成，两个目标正在执行全量 CLI。Windows 首批四项 runtime 测试失败定位为验收脚本漏配测试要求的 unwind 路径，已补齐三套 C／unwind 工具配置；修复后 release workspace all-targets Clippy 与五个 CLI 构建通过，完整 Rust 测试与 profile 15 系统库准备继续执行。新批次发现 23 项 HIR 快照失败，逐项确认仅为 Windows checkout 将预期文件转换成 CRLF，内容在恢复 LF 后全部相等；该问题继续修复和定向复测。

原始报告保留在仓库外，已经完成的 macOS 全量与定向 fixture 工作目录及过期编译缓存已清理；正在运行的其他平台验收继续保留其工作目录。

## 后续优先级

尺寸优化按用户补充要求，优先精简结构、删除不必要或重复的信息；压缩只作为最后的补充，且不采用 CPU／内存消耗过大的算法。

1. 完成尺寸阶段在 macOS、Linux、Windows 适用目标上的统一全量 fixture 与 workspace 验收，修复所有遇到的实际失败；随后切换到编译速度大项。优先实测对象生产中的重复查找与独立 LLVM context 的并行发射，保持确定性和全部 GC／ABI 合同。
2. 当前完整 29 库的 HIR／MIR／LIR 字符串表投影仅省约 1.05～1.06 MiB，其中 LIR 约 0.04～0.08 MiB。注册记录 ABI 去掉重复 group／member hash 输入的投影约省 1.93 MiB，该估算尚保留旧索引，仅用于候选排序。跨三层 metadata 的重复 ID 原始字节上限为 7.03 MiB，尚未扣映射开销；全局 union 会让只请求 HIR 的读取载入大量 LIR 专用 ID，先保留阶段局部读取。这些候选后置。manifest 的全量 ID 表此前反而增加约 2.10 MiB，不采用。

已排除直接复用 LLVM 22.1 的对象发射 pass manager：即使 AsmPrinter／MCStreamer／MachineModuleInfo 有逐模块收尾，带 GC statepoint 与异常展开的第二个函数模块仍残留前一模块的栈映射记录；仅数据流水线在 COFF debug → 无 debug 切换时也残留字符串表空间。独立的连续模块字节对照发现两者，实验实现均已撤下，未进入生产。本轮仅补齐数据发射入口自身的 LLVM pass 注册，并将测试顺序调整为先调用数据入口，避免测试依赖完整代码生成的先行初始化。格式检查、workspace all-targets Clippy 和七目标两配置的数据对象对照通过；不为撤下的实验记录性能收益。

本机原始命令、采样、逐轮计时和验收日志保留在仓库外的 `/Volumes/Data/home/chenxu/scoop-performance-results/compiler-20261101`；Linux 日志在 `~/repos/scoop-work/compiler-20261101`。工作缓存与对照二进制暂放本仓库 `tmp/compile-speed-20261101`，只保留下一轮实际需要的内容。
