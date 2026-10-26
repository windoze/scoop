# 搬迁复制复用已预留元数据

基线为 `74ad6bf0d`，包含自然循环内联、接口 SSA 和普通 GC 覆盖登记消除。本项只处理复制阶段仍逐对象进入通用分配登记的问题；编译器、程序源码、接口布局及四个系统库固定，仅替换 runtime。

全部目标预留成功后，搬迁计划已保存目标 block、精确大小与范围。此时 mutator 已停稳，mark worker 已完成，update worker 尚未发布，coordinator 是目标元数据的唯一写者。小对象复制直接复用这些事实和原有登记 helper，省去块种类、大小、边界、起点的重复检查及通用入口调用。occupied／starts 用 atomic load、OR、store 保留既有位，line_live／marks 直接 OR；精确大小、位图和 live bytes 完成后 release 发布 starts，再发布 forwarding 和更新存活清单。

新块、survivor 和旧代空洞沿同一阶段规则，不引入第二套布局或持久状态。共享 mutator 分配仍使用原子读改写，并行标记保留首次置位；large 对象沿原入口。完整预留失败仍不复制、不发布目标对象或 forwarding。通用小对象入口不再需要 marked 参数，已同步删除该参数及无调用者的分支。

所有 profile 默认采用，无独立开关、测量代码、对象字段、根或计划项增长。runtime contract 19、metadata ABI 9 和生成代码接口保持；重新构建 runtime 并链接即可。系统库复用用于隔离实验变量，不构成保留历史兼容分支的要求。

## 正确性与机器码

两机先执行格式化、release Clippy 和 C 警告检查，再各通过 7 项 native 定向回归：复制后的存活清单与元数据、并行 roots／remembered slots 更新、bitmap sweep、完整预留失败、晋升失败回退、survivor 生命周期和 nursery 块复用。扩展原边界测试，在真正复制后、引用回写前检查完整字节、精确大小、对齐、starts、mark 和每个 occupied／live line；新增 9,152 B、16 B 对齐对象跨多个位图字，原 small／large／pin 组合保持。相关回写测试覆盖 1／2／4／8 worker 及普通、heap 验证、stress 构建。

三目标各通过 `gc-correctness/interface-ssa` 和 `gc-correctness/poll/reference-arrays`，各 3 个变体、12 次进程调用，覆盖 debug／release、moving／minor、父接口投影、数组复制和可选 survivor。独立统计确认接口用例各实际搬迁 10,648 B，数组用例各执行 194～201 次 minor 并复制数 MB。没有运行完整验收。

八程序的 MIR、LIR 和符号映射相同。每目标四种 runtime 的 92 个对象中，只有 allocation、evacuation_plan、heap_objects 三个对象变化；其余逐字节相同。普通及仅 startup-stats 的对象没有覆盖诊断函数和相关错误文本，stress 两种配置仍保留。该边界没有因本项改变。

最终复制循环不再调用 `scoop_heap_record_small_object`，也不执行其 occupied／starts 的原子读改写；原通用入口含两处 ARM64 `ldset`／`ldsetl` 或 x86_64 `lock` 静态指令，其中 occupied 操作在逐 line 循环内。复制所需的普通位图更新与 release 发布仍存在，不能把静态指令数直接当成每对象周期收益。ARM64 的普通 TLAB 完成函数机器指令保持，小对象初始化函数只在共享分配慢边少一条参数赋值。

| 目标 | 完整规划函数指令数 | 通用小对象登记指令数 |
| --- | ---: | ---: |
| ARM64 Darwin | 203→252 | 86→59 |
| x86_64 GNU | 233→300 | 119→79 |
| x86_64 musl | 232→300 | 119→79 |

应用代表函数的指令数量保持，链接地址及部分反汇编显示名变化；接口复制热循环没有新增工作。没有把既有接口 SSA 的复制链、根恢复或栈访存收益重复计入。

## 测量方法与限制

M3 Ultra／LLVM 22.1.8 与 nuc12 i7-12700H／LLVM 22.1.2 实机。正常 release、不启用 stress；每目标一对预热、五对 AB／BA，GC 统计另跑一对。Linux 计时及 GC 统计固定在四个物理核 CPU 4、6、8、10，频率未锁；编译成本使用普通调度。没有从 ARM64 外推 x86_64。

下表为最终版墙钟中位数变化，负值表示耗时降低。

| 程序 | ARM64 Darwin | x86_64 GNU | x86_64 musl |
| --- | ---: | ---: | ---: |
| trees | −0.92% | −7.05% | −3.92% |
| strmap | +0.16% | +0.21% | −0.14% |
| strings | −1.06% | +2.55% | +0.25% |
| dispatch | +2.00% | +2.12% | −0.31% |
| lambdas | −0.83% | −2.61% | +0.74% |
| List<Long> | +1.25% | −0.47% | +1.39% |
| 接口数组／父投影 | +1.04% | −1.14% | +0.73% |
| class 跨调用 | −1.25% | +2.19% | −4.28% |

trees 中位数依次为 3.692528→3.658496 s、5.994040→5.571591 s、7.203829→6.921671 s；峰值 RSS 中位数为 464.63→464.70 MiB、447.33→447.29 MiB、445.48→445.79 MiB。独立的一对阶段统计如下，不是五轮中位数，也不与 worker 重叠计时相加。

| 目标 | trees 复制阶段累计（s） | 复制耗时变化 | GC 总暂停（s） |
| --- | ---: | ---: | ---: |
| ARM64 Darwin | 0.467115→0.390985 | −16.30% | 1.974494→1.894842 |
| x86_64 GNU | 0.979235→0.660241 | −32.58% | 3.394730→3.074504 |
| x86_64 musl | 1.340626→1.126491 | −15.97% | 4.604739→4.392400 |

八程序各目标的 managed／nursery 分配字节、GC 次数、根扫描量、追踪对象、旧代引用槽及脏卡保持。trees 均分配 9,686,046,288 B，执行 297 次 minor／12 次 full、追踪 70,981,978 个对象并扫描 23,322 个根槽；复制字节最多相差 384 B。dispatch 和 class 对照没有 GC，接口数组仅一次 minor；这些程序的正负变化不能归因为复制登记。GNU strings 的回退等结果原样保留，不宣称所有负载普遍加速。

初版曾提前计算 starts mask，连带改变普通 nursery 分配的五条指令调度，已撤回；最终版本只保留必要的复制改动及无用参数删除。删参前 ARM64 接口数组五对对照一度回退 7.30%，但该程序仅一次 minor、复制 4,704 B，GC 总量约 20～30 μs，通常热循环指令相同。针对该异常仅追加一次 15 轮三组交替，其中第三组重复同一个基线二进制：中位数依次为 0.127340、0.126837、0.125565 s。回退没有稳定复现；保留原结果，不归因为复制登记，也不宣称已排除所有布局或运行波动。

## 空间与编译成本

| 目标 | 每程序 text 变化 | runtime 冷构建墙钟（s） | 编译 CPU 变化 | 编译峰值 RSS 变化 |
| --- | ---: | ---: | ---: | ---: |
| ARM64 Darwin | +84 B | 4.525→4.507 | −0.89% | −0.25% |
| x86_64 GNU | +80 B | 5.160→5.345 | +0.73% | −0.14% |
| x86_64 musl | +128 B | 6.000→5.630 | −0.61% | −0.32% |

ARM64 八程序完整文件大小保持；GNU 的 strings、musl 的 List<Long> 因文件对齐各增加 4,096 B，其余保持。没有对象元数据、静态接口表、根或搬迁计划增长。

runtime 冷构建复用同一 builder／vendor，每次使用独立空缓存，一对预热、三对正式交替；它不是整个 Scoop consumer 的冷编译时间。没有新增 compiler pass 或库正文编译工作。

原始采样、逐次计时、GC 数据、IR、机器码与验证日志位于两机仓库外 `scoop-performance-results/2026-10-26/evacuation-publication`；初版和删参前结果分别保留在 `initial`、`before-parameter-cleanup`，本机 `linux-results` 保存 Linux 副本。临时构建位于仓库忽略的 `tmp/program-performance/evacuation-publication`。
