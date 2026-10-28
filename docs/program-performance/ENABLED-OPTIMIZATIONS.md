# 已采纳优化与开启条件

本表记录本次[程序性能优化主线](PROGRESS.md)已采纳的方案，详细设计、验证、收益与成本见各项链接。后续采纳新方案或调整默认策略时同步更新。

“release 默认”指编译该 Cone 时使用 `scoop build --release` 或 `scoop build --profile release`，并满足表中的适用条件；独立编译器使用 `scoopc build --profile release`。CLI 默认 profile 为 debug。除表中明确列出的运行时参数外，下列优化没有各自独立的公开开关或用户可设等级，编译器内部选项不等于 CLI 参数。

相关 API 调整：[String 的字符扫描名称](STRING-API.md)在全部 profile 生效，无开关；`charLength`／`getCharAt` 保持 O(n)，`byteLength` 保持 O(1)，内部存储保持。扫描、编码与后备职责迁移见 [Runtime 职责收敛](RUNTIME-RESPONSIBILITIES.md)。

表中 ABI 版本注明各项引入时的合同；当前统一 runtime contract 为 **25**、metadata ABI 为 **9**，配套库／runtime／sysroot 按当前合同重建。

## 程序优化

| 已采纳方案 | 开启条件 | 适用范围与主要限制 |
| --- | --- | --- |
| [按实际引用量划分标记片](GC-EVOLUTION.md#第五十三阶段按实际引用量划分标记片) | 新版 runtime，所有 profile 随现有增量策略生效；默认等级 1，minor 分片仍需 `SCOOP_GC_INCREMENTAL=2` | 复用实际槽计数，每 128 槽计一个单位，完整任务允许越过片末；缩短大数组中间片，保留无界收尾。部分默认尾部及等级 2 的 p95／CPU 代价见记录；不改变分配快路径，无新开关、对象、metadata 或 ABI。 |
| [Linux 空闲页延迟归还](GC-EVOLUTION.md#第五十一阶段linux-空闲页延迟归还) | Linux runtime 所有 profile 默认；重链接即可，无新开关 | 对已确认不含活对象的页使用 MADV_FREE，复用前仍初始化；空 region 仍按原规则 unmap。以延迟下降的 RSS 换取线程分配吞吐和较短 STW；Darwin 试验未确认收益，保留原策略。无对象／元数据／ABI 增量。 |
| [GC 队列的有界环形索引](GC-EVOLUTION.md#第四十九阶段gc-队列的有界环形索引) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | 队列容量为二次幂，使用掩码回绕；worker 遍历按实际人数回绕。保留锁、发布、终止与首尾消费顺序，消除队列动态除法。大图 STW 及部分线程吞吐改善，GNU 大图吞吐和线程尾部波动见记录，无存储／ABI 增量。 |
| [短 GC 阶段省去无效调度](GC-EVOLUTION.md#第四十八阶段短-gc-阶段省去无效调度) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | 空阶段直接结束，单参与者不发布或广播新阶段；固定预留／复制任务只启动所需参与者，动态标记／回写仍保留原并行度。大图及 GNU 多线程 STW 改善，整体吞吐基本持平；部分回退和非独占机器短测波动见记录，无对象／metadata／ABI 增量。 |
| [较高 GC 并行度的显式选择](GC-EVOLUTION.md#第四十七阶段较高-gc-并行度的显式选择) | `SCOOP_GC_WORKERS=9..16`；默认仍最多 8，所有 profile 同义 | 大引用数组以更多 CPU／私有统计空间换取较短 STW；小图与不同平台的分配吞吐可能回退，不提高默认上限。分片总工作量保持；native `ScoopGcMetrics` 扩至 16 槽，消费者须用新头重编，对象／generated-code ABI 不变。 |
| [编译器可见的引用扫描循环](GC-EVOLUTION.md#第四十六阶段编译器可见的引用扫描循环) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | 单引用数组复用共享范围算法，在 collector 热循环中展开标记过滤，省去逐槽调用和帧保存；动态／复杂扫描保持。增加约 3.5／14 KiB runtime 代码，改善大图 STW 及线程分配吞吐；树形吞吐、部分最大暂停及 runtime 编译成本回退见记录，无对象／metadata／ABI 增量。 |
| [region 紧凑引用元数据](GC-EVOLUTION.md#第四十五阶段region-紧凑引用元数据) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | 连续 head／starts 保留唯一状态和精确起点，minor 的合法旧引用避开完整 block 记录；省去逐 block 的 starts 分配。完整普通 region 增加约 4 KiB，未使用块多预留位图；大图及部分分配改善，线程尾部回退见记录，无对象／ABI 变化。 |
| [连续脏卡范围与置位起点枚举](GC-EVOLUTION.md#第四十四阶段连续脏卡范围与置位起点枚举) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | 相邻脏卡共用一次范围扫描，小对象仅枚举 starts 置位项；不跨清卡或卡组，保留精确槽裁剪、survivor 与并行回写所有权。GNU 大图吞吐和线程暂停改善，ARM64 基本持平且部分尾部回退；无分配快路径、对象、metadata 或 ABI 增量。 |
| [固定对象复用已验证布局](GC-EVOLUTION.md#第四十阶段固定对象复用已验证布局) | 新版普通 runtime 所有 profile 默认；GC stress 支持或 `SCOOP_GC_VERIFY_HEAP` 构建仍完整交叉校验 | 首次标记和 remembered 扫描复用分配已保证的固定对象／box 大小与对齐，省去逐对象 shape 校验调用；动态长度、原生分配、映像、拆箱与精确起点检查保持。无对象／metadata／ABI 增量，重链接即可。 |
| [完整引用回写复用平坦扫描](GC-EVOLUTION.md#第三十七阶段完整引用回写复用平坦扫描) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | 串行与并行回写直接消费复制后存活对象，共用原 binding 的完整对象扫描正文。平坦对象省去共享解释器入口，动态 visitor、局部范围、复杂 scan、survivor 与覆盖验证保持；少量代码增长，无对象／metadata／ABI 增量。 |
| [旧槽余量与少搬迁晋升](GC-EVOLUTION.md#第三十六阶段旧槽余量与少搬迁晋升) | `SCOOP_GC_OLD_SLOTS=3` 显式开启；默认仍为 2，所有 profile 同义 | 密集、等大小、精确网格的年轻块为旧槽保留相当于当前 nursery 的余量，优先原地转槽。树形分配可少复制；大随机更新图可能增加 RSS 和暂停，完整预留、survivor、pin 与失败回退保持，无对象／ABI 增量。 |
| [单引用数组的槽区间](GC-EVOLUTION.md#第三十五阶段单引用数组的槽区间) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | 实际 References child 恰含一个 offset 时，从槽地址一次计算合法下标并按 stride 调用原 visitor，省去逐元素 offset 遍历与范围分支。多引用／递归路径保留，无对象、metadata 或 ABI 增量；以少量代码和短线程回退换取大图 STW／吞吐收益。 |
| [旧槽按位图字预留](GC-EVOLUTION.md#第三十四阶段旧槽按位图字预留) | 新版 runtime 随既有 `SCOOP_GC_OLD_SLOTS=1..3` 生效，默认 2；所有 profile，无新开关 | 复用零空槽计数跳过满块，按精确网格缓存空起点；保留整块独占、完整预留、回滚和 full 重建。每个可用块增加 8 B native 记录，另有 248 B 静态表；无逐对象／ABI 增量，接受单线程及部分长尾代价以缩短大图／多线程 STW。 |
| [TLAB 已对齐范围比较](GC-EVOLUTION.md#第三十二阶段tlab-已对齐范围比较) | ARM64 release 默认；已知 8-byte 对齐、规格化尺寸为 16～32,640 B 的常量 | 直接比较请求与当前 `limit - cursor`，省去重复对齐和判空；原初始化、发布、计数及 collecting slow path 保持。x86 两个候选因树／单线程回退撤回，debug、动态或其他对齐沿原路径；无对象／ABI／配置增量，线程尾部代价见记录。 |
| [精确对象定位正文内联](GC-EVOLUTION.md) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | 当前 block 视图内共用对齐、活动状态、starts／large 发布和对象索引正文，省去逐引用调用与索引栈中转。保留全部精确起点检查、acquire 和 STW 边界；无对象／ABI 增量，大图吞吐改善，暂停和线程波动见阶段三十。 |
| [提高 GC 并行度并限制分片总处理量](GC-EVOLUTION.md) | 新版 runtime 所有 profile 默认最多 8 worker；候选块小于 4 MiB 仍串行；既有 `SCOOP_GC_WORKERS` 可强制选择，范围见阶段四十七 | 四个以上 worker 分享约 4,096 单位的单片处理量，完整收尾仍排空任务。缩短大图和线程 STW、改善深树／多线程吞吐，接受更多 CPU、私有统计空间及单线程短负载回退；无新对象／ABI／选项，详见阶段二十九。 |
| [GC 工作队列按等待需求通知](GC-EVOLUTION.md) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | 无等待者时跳过通知锁，新工作只唤醒一人，终止仍持锁广播；先登记等待者再重查队列，保留分片交接与 phase 参与规则。主要改善 ARM64 树／线程吞吐和线程 STW，大图回退见阶段二十八；无对象／ABI 增量。 |
| [完整对象的平坦扫描展开](GC-EVOLUTION.md) | 新版 runtime 所有 profile 默认；重链接即可，无新开关 | binding 共用一个偏移扫描正文，完整范围与固定 visitor 直接展开；保留复杂 scan、局部范围和原子首次标记。树／线程吞吐改善，部分暂停回退保留；无对象／ABI 增量，代码成本见阶段二十七。 |
| [互斥域的分配计数](GC-EVOLUTION.md) | 所有 profile 默认，配套 runtime contract 25；编译器／runtime／系统库一起重建 | nursery 与直接旧代分别只更新对象数和字节数，读取时合成总量；保留 atomic、detach、minor 基线与 starts 发布。无新状态或开关，主要减少分配指令；吞吐与暂停的分项对照见阶段二十六。 |
| [STW 与分配优先的 GC 默认组合](GC-EVOLUTION.md) | 新版 runtime 所有 profile 默认 `SCOOP_GC_OLD_SLOTS=2`＋`SCOOP_GC_INCREMENTAL=1`，两个选项独立 | 复用密集转槽、原地 full 与 full 分片；minor 分片继续显式等级 2。缩短大图与多线程尾暂停、提高树和线程分配吞吐，接受随机更新吞吐与部分应用间隔 p99 代价；无新算法／ABI，见阶段二十五。 |
| [full 搬迁目标并行预留](GC-EVOLUTION.md) | 需要 full 搬迁时默认使用（旧槽等级 0），沿现有 worker／小规模策略；`SCOOP_GC_WORKERS=1` 串行 | worker 独占完整目标块的空洞及复制组，保留单 source 容量预判、集中收益与统一回滚；逐对象不取锁，无新对象／ABI。缩短 STW，增加 CPU 和约 1 KiB 临时规划数组，四线程／部分 p99 代价见阶段二十四。 |
| [分配不再维护 line 位图](GC-EVOLUTION.md) | 全部 profile 默认，配套 runtime contract 24；编译器／runtime／系统库一起重建 | 普通分配只维护精确大小、starts 与原计数，collector 的唯一 live-line 供清扫／full VM 归还消费；context 40 B、每小对象块逻辑 metadata 减少 40 B。分配吞吐与代码量改善，线程／尾暂停回退见阶段二十三；无新开关。 |
| [密集年轻块直接转为旧代槽](GC-EVOLUTION.md) | `SCOOP_GC_OLD_SLOTS=2` 为所有 profile 默认；0 选择 region 整理，1 保留复制晋升；所有 profile 语义一致 | marker 汇总精确大小与槽网格；默认等级 2 在已有空槽不足时原地晋升，等级 3 另保留空槽余量。保留首次 survivor、pin、混合布局及失败回退。树和线程减少复制、CPU 与 RSS，大图尾暂停可能回退；每进程增加 248 B，ABI 23 不变，见阶段二十二。 |
| [worker 本地存活 line 位图](GC-EVOLUTION.md) | 新版 runtime 所有 profile 默认，无新开关 | 逐对象只更新本 worker 的块统计，标记完成后与字节计数一起合并；保留原子首次标记、分片与复制边界。每 worker／参与 block 额外 32 B 临时统计，无对象或 ABI 增量；吞吐、暂停及 RSS 的小幅回退见阶段二十一。 |
| [minor 分片标记与有界分配余量](GC-EVOLUTION.md) | `SCOOP_GC_INCREMENTAL=2`，默认关闭；0 保持完整收集，1 仍只分片 full | nursery 达目标容量后分片，片间最多使用目标容量的 1/8 额外 block；保存旧脏卡至最终重定位，OOM／显式 full 完整回退。降低大存活图的单次停顿，但增加吞吐、CPU、应用间隔 p95 和部分 RSS 成本；树形负载不推荐。各 profile 同义、ABI 23 不变，具体对照见阶段二十。 |
| [引用更新直接消费存活分块](GC-EVOLUTION.md) | 新版 runtime 所有 profile 默认，沿现有 worker 策略；单 worker 直接遍历 | 省去 coordinator 逐对象建批次和重复 scan 读取，数组任务完整结束后 reclaim；降低队列容量、CPU 和部分暂停。覆盖位图原子 claim 只编入原 stress／显式验证配置，ABI 23 不变；回退与测量边界见阶段十九。 |
| [minor 搬迁目标并行预留](GC-EVOLUTION.md) | 新版 runtime 所有 profile 默认，沿现有 worker／小规模策略；`SCOOP_GC_WORKERS=1` 串行 | 无 survivor 配额时按存活清单分块，worker 独占目标区；全体成功后复制，失败完整回滚。survivor 配额仍串行规划，full 并行见上项；无新 ABI；CPU、GNU full 尾部及部分 RSS 代价见阶段十八。 |
| [单次标记停顿内复用 region](GC-EVOLUTION.md) | 新版 runtime，所有 profile 默认；无新开关，ABI 23 不变 | 每 worker 32 项普通 region，最大额外 4 KiB；每次 begin／resume 失效，命中仍验证对象起点，large／未命中走页表。普通与增量暂停、十六线程／RSS 回退见阶段十七。 |
| [数组 owner 直接定位 mapping](GC-EVOLUTION.md) | ARM64 所有 profile 默认，配套 runtime contract 23；x86_64 保留原页表，无新开关 | 单引用 ArraySet 选择 small／large 的既有 mapping 前缀，省去四级页表；原卡合并、AS1 重定位及 pin 保持。普通容量不变，stress 保留 region 首 block。随机写入收益、整程序与尾暂停回退见阶段十六。 |
| [按目标 block 并行复制](GC-EVOLUTION.md) | 新版 runtime，所有 profile 默认，沿用现有 worker／小规模策略；`SCOOP_GC_WORKERS=1` 串行 | 每个目标 block 单写者，完整预留后复制、全部完成后更新 roots；每阶段固定 worker 参与数量；每目标块 48 B 临时记录，无对象或 ABI 增量。降低复制阶段、墙钟和部分尾暂停，树形及高线程数 CPU 代价见阶段十三。 |
| [旧代槽的批量清扫](GC-EVOLUTION.md) | 随 `SCOOP_GC_OLD_SLOTS=1..3` 启用，当前默认等级 2；所有 profile 一致 | nursery 资格已排除 release hook，按位图清除死亡槽；普通块、large 与 moving stress 保留原路径。没有新 metadata／ABI／选项，缩短清扫和 full 收尾，整体 p99 与 M3 CPU 的限制见阶段十五。 |
| [旧代精确大小槽与原地 full](GC-EVOLUTION.md) | 新版 runtime，`SCOOP_GC_OLD_SLOTS=1..3`，默认等级 2；所有 profile 语义一致 | 16～256 B 晋升对象复用死亡槽，full 按真实存活与新增压力触发；保留完整预留回滚和独占回写。降低大存活图 RSS 与最长停顿，可能增加 CPU、收集频率及应用间隔 p99；stress／full-only 沿原策略，无 ABI／对象头变化。 |
| [minor 旧代脏区并行扫描与回写](GC-EVOLUTION.md) | 新版 runtime，所有 profile 默认，沿用现有 worker 策略；单 worker 保留直接扫描 | 只发布活动旧代卡组，复用精确范围和原 pool；roots 先更新，保持 native alias、survivor 和增量 full 的独立清卡协议。无 ABI／对象增长，大存活集吞吐与 p99 改善，full 长暂停及部分线程负载回退见阶段十一。 |
| [已确认对象的直接 region 定位](GC-EVOLUTION.md) | 新版 runtime，所有 profile 默认；重链接即可 | 对齐 region 的保留首行保存 owner，collector 已验证 source／已预留 target 直接定位；普通 source 不重复查 pin，未知地址沿原页表。contract 23 让 moving stress 复用 owner，ARM64 数组写入见阶段十六；对象头不变。 |
| [目标头保存临时搬迁计划](GC-EVOLUTION.md) | 新版 runtime，所有 profile 默认；重链接即可 | 已预留但未发布的目标头保存 next／live entry，删除每对象 32 B native 计划；原图直到完整预留成功仍不变，复制后发布。无 ABI／对象增量，主要降低工作区和 RSS，尾暂停及 Linux 波动限制见阶段九。 |
| [小对象大小合入 GC 字](GC-EVOLUTION.md) | 所有 profile 默认，按当前 runtime contract 配套重建 | 阶段八删除每块 8 KiB 大小表，context 56→48 B、普通 block metadata 120→112 B；阶段二十三继续缩至 40／104 B。对象头仍为 16 B，pin／release／复制状态保持，large 沿原精确大小。两机收益与限制见记录，无新开关。 |
| [冻结图的直接引用回写](GC-EVOLUTION.md) | 新版 runtime，所有 profile 默认；重链接即可 | 完成 mark 和复制后直接消费源 GC 字，省去逐边 page map／mark／pin 复核；首次边界检查保持，survivor 查询当前目标代际，完整验证只编入 stress／heap 验证。无布局、ABI 或新配置，性能与暂停限制见阶段七。 |
| [源对象 GC 字转发](GC-EVOLUTION.md) | 新版 runtime，所有 profile 默认；重链接即可 | 完整复制后在不可见旧副本保存带标志的目标地址，删除源块 forwarding 表；阶段六将计划 48→32 B、block metadata 136→120 B，后由阶段八／九继续收敛。当前对象头、pin／release、预留失败和 ABI 保持。 |
| [清扫复用存活 line](GC-EVOLUTION.md) | 新版 runtime，所有 profile 默认；重链接即可 | 普通保留块只逐个退休死亡对象，复用已完成 mark／复制的 line 位图；完整搬出的无 pin 源直接退休。moving stress 保留 forwarding 检查与 poison，pin／release／空洞／失败回退保持，无额外元数据或 ABI。 |
| [full 分片标记](GC-EVOLUTION.md) | `SCOOP_GC_INCREMENTAL=1` 为所有 profile 默认；0 选择完整收集，2 额外分片 minor；重链接即可 | 旧代压力触发两代追踪，片间保留原图并恢复 mutator；现有卡组维护改写，精确 roots 每片重扫。完整收尾仍为 STW，显式 full／large／失败走完整回退；树分配与最长暂停改善，但随机更新吞吐和部分 RSS 回退，按负载选择。对象及 ABI 不变。 |
| [TLAB 本地块储备](GC-EVOLUTION.md) | 新版普通 runtime，所有 profile 默认；重链接即可 | 一次 heap lock 最多准备四块，后续切换复用已登记储备；GC／managed 退出／callback 返回统一失效。保持 nursery 上限、部分失败和精确计数；stress／full-only 原策略，状态大小与 ABI 不变。RSS／短负载尾部代价见记录。 |
| [密集年轻块原地晋升](GC-EVOLUTION.md) | `SCOOP_GC_PROMOTE_IN_PLACE=1`，默认关闭；无源计划跳过所有 profile 默认 | 复用 block 存活统计省去密集块复制、forwarding 和计划；稀疏搬迁、pin、survivor、失败回退保持。吞吐与复制量改善，但大存活集 full 尾暂停／RSS 回退，须按负载选择；不增对象或 ABI。 |
| [GC epoch 确认与通知分流](GC-EVOLUTION.md) | 新版 runtime 所有 profile 默认，无新开关；重新链接即可 | 同一 epoch 不重复 park 确认，停稳只通知 collector，mutator 只在真实状态变化时唤醒；保留连续收集、初始化与 NativeSafe 超时复查，不增对象／线程字段或 ABI。多线程收益、低线程数波动及暂停观测边界见记录。 |
| [最终可执行文件符号清理](EXECUTABLE-SYMBOLS.md) | release 默认；artifact-only link 默认；debug 或 `--keep-symbols` 保留 | Darwin linker `-x -S`、ELF `--strip-all`；仅在最终链接清理普通符号，保留动态符号及 GC／EH 数据。旁置 `.map` 与显式 symbols 观察提供地址归因，参与链接的 `.slib`／`.o` 保持。 |
| [ArrayList 普通快照正文](ARRAYLIST-SNAPSHOTS.md) | 新版 core 全 profile 提供成员，release 按已有调用点策略优化；core 及静态解析调用的依赖 Cone 需重建 | 捕获逻辑长度和 backing，以普通 initializer 复制已初始化前缀；静态 List／自定义 getter 保留原扩展。结合整字存储，保留按成本共享的调用及跨分配源根，无专用 intrinsic 或新开关。 |
| [MaybeUninit 整字原始存储](RAW-VALUE-STORAGE.md) | 新编译器所有 profile 同一表示，release 默认复用 SROA／向量化，无开关 | 按已有对齐以至多 64 位整数保存原始 bits，保留 padding、AS1 引用偏移及物理 ABI；重编译相关正文后受益。省去字节 gather／拼装，musl 短负载回退和测量限制见记录。 |
| [GC 地址整数的重定位时序](GC-ADDRESS-BITS.md) | 新编译器所有 profile 默认，无开关；受影响应用和库须重新编译 | 地址转换保留 AS1 operand 到 RS4GC 后，防止页表／card 使用旧地址位；不声明普通内存写入，有界无 safepoint 区域沿用原提升规则。临时操作不进产物，无新 ABI；Linux lambdas 的运行代价见记录。 |
| [搬迁复制复用已预留元数据](EVACUATION-PUBLICATION.md) | 使用新版 runtime 重新链接，所有 profile 默认，无独立开关 | 完整预留后的 STW 复制复用目标 block／大小／范围及 coordinator 单写者关系，省去通用登记调用、重复检查和位图原子读改写；共享分配、并行标记、large 和预留失败语义保持。删除无调用者的 marked 参数，无对象、roots 或 ABI 增长。 |
| [普通 GC 省去搬迁覆盖登记](RELOCATION-COVERAGE.md) | 新版普通 runtime，所有 profile 默认；重链接即可，系统库可复用 | 直接消费唯一且已更新为当前副本的存活清单，省去逐对象 claim、覆盖位图及其生命周期操作。现有 GC stress 构建或显式 heap 验证保留这些诊断；真实字段／roots 转发、pin、survivor 与 ABI 保持。 |
| [宽数组元素的重复引用屏障消除](REFERENCE-WRITES.md) | release 默认；重新生成实际 ArraySet 正文，现有四个系统库与 runtime 可复用 | stride >16 B、恰有一个 managed leaf 时，完整写入后仅为不同的非 null 引用标卡。单／双字批量路径、多引用范围屏障、字段、原子、native 和初始化保持原策略；不增加根或跨 GC 保存旧引用。 |
| [固定引用布局的数组扫描](ARRAY-SCANS.md) | 使用新版 runtime 重新链接，所有 profile 默认；无需重建 core | 根据实际 References child 直接枚举元素槽，省去逐元素递归解释；保留精确范围、访问顺序及原标记／搬迁 visitor。复杂 child 保留递归，scan／对象／根及 ABI 不变；各平台收益、短对照回退和 native 栈帧成本见记录。 |
| [年轻对象初始化屏障消除](YOUNG-INITIALIZATION.md)，含[数组初始化](ARRAY-INITIALIZATION-BARRIERS.md) | release 默认，沿既有分配优化配置；旧库需重建才能获益 | 实际 TLAB 成功后，未经过 GC／观察／外传的字段、typed box 复制及数组字面量／clone／assembly 初始化跳过屏障；字段跟踪未取址副本及单前驱直线后继，数组事实仅在当前 typed 操作内使用。慢分配、用户 initializer、原子写入、外部数组发布和 C 后备保留原屏障，无新增根、布局或 ABI。 |
| [TaskContext fork 的生成分配](CONTEXT-FORK.md) | 新编译器所有 profile；release 默认复用 TLAB，含 fork 的库正文需重建才能获益 | typed Fork 降为已知 24 B Alloc／HeapStore，snapshot 跨分配恢复后写入；独立任务身份、屏障及 ABI 保持。初始 root 与绑定树继续共享 runtime；旧 fork C 入口已退役，详见 Runtime 职责收敛。 |
| [Context 小访问器生成路径](CONTEXT-READS.md) | 新编译器所有 profile 默认，遵守当前 runtime contract；旧 sysroot／产物须重建 | typed 操作读写原线程根槽，snapshot 判空后读实际 root 字段；ARM64 调用内惰性复用槽的 native 地址，x86_64 直接读取 TLS。每线程增加 8 B 地址，无新增 managed 引用或根；restore 复用普通字段写入和屏障；真实 snapshot／绑定树 C 消费者保留，普通 extern 遵守其声明。 |
| [初始化完成后的就绪快路径](INIT-READY.md) | 新编译器所有 profile 默认，配套 runtime contract 18；旧库／runtime／sysroot 须重建 | 真实 typed 普通 InitializationEnter 先 acquire 读 state，Initialized 跳过 runtime／TLS／锁；其余状态、invoke、普通 extern 和入口 poll 保留。cell 仍为 16 B，根仅在慢边恢复，后续对象地址按原 GC 契约重读。 |
| [String 字节复制与 Char 编码](STRING-COPIES.md) | 新 core 协议所有 profile 一致；release 默认复用 TLAB、调用点内联及常量传播，须重建配套 core／依赖 | typed 复制按真实布局分配并在 GC 后重取源，String 字节快照消除逐字节 initializer；Char 普通正文按调用点展开。长 memcpy 和 List getter 快照保留；无消费者的旧 C 复制／Char 入口已由职责收敛批次退役。 |
| [MIR 末尾局部复制传播](INTERFACE-SSA.md) | release 默认，沿现有 MIR 内联优化配置 | 内联与循环选择后传播未取址局部当前别名；保持重赋快照、合流、循环与 EH，不依赖变量名。ARM64 dispatch／接口复制明显改善，lambdas 等回退如实记录并继续精化根与 SSA。 |
| [调用期间保活与返回恢复分离](INTERFACE-SSA.md) | 新编译器所有 profile 默认 | 死 direct AS1 实参保留调用期间根，省去人工后继使用及恢复；byval、展开后输入、poll、EH 与 native 协议保持，按实际 RS4GC 结果计根。各平台收益和阶段性回退分别记录。 |
| [接口分量与重定位结果 SSA](INTERFACE-SSA.md) | 局部根槽提升为 release 默认；晚期 aggregate 分解及指针合流归并适用于所有 profile | 复用 SROA／mem2reg，普通优化结束后拆分聚合、归并相同输入的指针 PHI／select，再定稿 roots；真实 ret／store／实参消费 relocated object。取址／发布／间接 ABI 等存储保持，不增加对象或接口表大小。 |
| [String hash 有界范围与提前返回](STRING-HASH-RANGE.md) | 新 core 所有 profile 默认；release 按原调用点评分展开，需重建 core 及依赖者 | ≤32 B 使用普通标量正文并及时返回，9～32 B 省去 native hash／协调／caller roots；长输入现已迁为共享 core 循环。原结果、表示保持，无新开关或测量代码；极短输入的成本见对照记录。 |
| [String 扫描、UTF-8 编码／校验及长 hash 正文](RUNTIME-RESPONSIBILITIES.md) | 新 core 所有 profile 默认；release 默认使用既有调用点内联与 64 次 poll 分块 | 普通算法共享 Scoop 正文，按真实 typed 操作读写和分配；窄块计数后累加为 Long。旧 C 算法及聚合 shim 退役，不改变公开语义、String 布局、快照和借用；无额外开关。 |
| [浮点文本适配与解析栈缓冲](RUNTIME-RESPONSIBILITIES.md) | 新 core／runtime 所有 profile 默认 | core 的有界格式正文调用现有 Ryu；typed StringCopy 构造结果。JSON／CLI 小于 256 B 的文本使用栈缓冲补 NUL，长输入保留动态缓冲与原 libc 解析，无新开关。 |
| [String 比较与短 hash](STRING-READS.md) | 普通 core 正文全 profile；调用点常量折叠和 ≤8 B 常量比较 release 默认，须重建 core | 同引用／长度／空串判断、初版有界 FNV-1a 和真实 typed 常量读取；hash 范围由上项继续扩展，长文本共享后备。保持内容、身份和位宽，无新增 ABI；ASCII 单独展开经对照未采纳。 |
| [编译依赖身份图复用](COMPILER-IDENTITIES.md) | 编译器所有 profile 默认，无新增选项 | 共享完整图与导入映射，单节点 key 索引、增量环队列、快速进程内哈希；保留引用／冲突检查，持久 ID、产物与 ABI 不变。 |
| [已知 box 生成分配](BOX-ALLOCATION.md)，含[后备收敛](RUNTIME-RESPONSIBILITIES.md) | 所有 profile 默认，配套 runtime contract 21；release 沿既有 TLAB 条件优化 | ZST／GC-free／含引用及大 payload 均使用已知布局，GC 后复制并保留必要屏障；debug／大对象仍用共享分配器，不进入专用 box C 包装。动态拆箱和真正的 native 根入口保留，无新开关。 |
| [普通 String 拼接融合与热二元生成](STRING-CONCAT.md) | release 默认 | 同块单用链最多八段、短常量合并；热循环／短常量二元使用同一路径，其他点共享后备。checked 一次分配并在 GC 后重读输入，保留共享结果与副作用边界，无布局或 ABI 变化。 |
| [短 f-string 就地展开](STRING-BUILDER-INTERPOLATION.md) | 新编译器所有 profile 展开；release 复用拼接融合 | 不超过八个有效片段，按序求值并立即转换，最后保存稳定引用并拼接；零／单段直接使用结果，2～8 段 release 一次分配。debug 保留二元后备，九段以上使用普通 StringBuilder；跨挂起与默认值保持，无新 ABI。 |
| [标量计算与 Boolean 字符串常量](SCALAR-BODIES.md) | 新 core 所有 profile 默认；release 按调用点展开 | Boolean equals／hash、整数 hash 为普通正文，热／常量点可化简，普通点可共享；Boolean.toString 不再分配结果 String，无消费者的旧 C 后备已退役。 |
| [整数专用十进制格式化](INTEGER-DECIMAL.md) | 新 core／runtime 所有 profile 默认；release 按调用点评分展开 | 20 B 栈缓冲区一次格式化，实际长度分配／复制；八位分块与两位表，动态大正文共享，无消费者的旧 C writer／ABI 已退役，无 NUL／locale 依赖。 |
| [普通 helper 正文闭合与循环／pointer 移植](INTEGER-DECIMAL.md) | 各 profile 导出一致，release 默认消费 | 沿 typed 依赖队列递归导出／导入，复用已完成分析；保留独立局部 place、loop target、原 intrinsic 身份和 GC 契约，不改变收益阈值。 |
| [线程入口复用已校验状态](THREAD-STATE-REUSE.md) | 使用新版 runtime 重新链接，所有 profile 默认，无独立开关 | 18 处同步线程／roots／异常／初始化入口复用 managed 检查返回的状态；native 边界 TLS 取址七次降至四次，检查与原子内存序保持，无新增字段或 ABI。 |
| [搬迁时更新存活清单](LIVE-SET-RELOCATION.md) | 使用新版 runtime 重新链接，所有 profile 默认，无独立开关 | 复制时将稳定清单条目改为当前副本，引用回写省去 source forwarding 与重复元数据定位；计划项和清单不增大，真实字段／roots 仍正常转发，保留 pin、survivor 与预留失败语义。 |
| [调用点内联评分](CALL-SITE-INLINING.md) | release 默认；扩大跨 Cone 正文范围需重建 provider；诊断由 `--emit optimizations` 开启 | 缓存函数事实，按调用点静态热度、实参、结果用途和 caller 规模比较可消除开销与净增长；热循环可接受大正文，冷点可保留小正文。沿用递归、增长及 GC 限制，无试内联或新 runtime 代码。 |
| [自然循环的内联增长分配](HOT-REGION-INLINING.md) | release 默认，无独立开关；可复用既有系统库 | 每个最内层自然循环分别累计，原基数按热度使用一／二／四倍上限，整个 caller 最多八倍；跨轮不返还。复用支配分析与原评分，诊断补充区域／总剩余；lambdas 三目标降低约 41%～43%，代码与编译成本、剩余 toArray 路径见记录。 |
| [不可达 callable 与 stackmap 裁剪](CALLABLE-PRUNING.md) | 最终链接所有 profile 默认；配套 `link-identity-closure/19`，旧输入须重建 | 沿完整物理符号引用选择，函数代码、EH、stackmap、登记与私有数据同选；元数据保留起点由下一项进一步收紧，native archive 候选引用保守保留目标。无需新增 runtime／诊断代码或对象字段。 |
| [类型／shape 元数据裁剪](TYPE-PRUNING.md) | 元数据生产与最终链接所有 profile 默认；沿用版本 19，旧粗分组库可直接重链接 | Strong TD 与同一 exact type 的登记合组，布局／scan／完整派发表独立划分；沿实际引用删除无入口类型及方法。静态存储、初始化、immortal 和必要登记保留；库体积与生产成本增加，consumer 构建、链接和启动受益。 |
| [一个 survivor 阶段与密集块原地保留](SURVIVOR.md) | 普通 debug／release 默认关闭；`SCOOP_GC_SURVIVOR=1` 开启 | 最多占 nursery 四分之一；首次存活的密集块原地保留，稀疏对象按配额复制，下一轮晋升。减少中等寿命对象的提前晋升；长寿图的扫描成本须按负载衡量，stress／full-only 保持原策略。 |
| [nursery 容量自适应](ADAPTIVE-NURSERY.md) | 普通 debug／release 默认，初始 1 MiB、上限 32 MiB；`SCOOP_GC_NURSERY_MAX_MIB=N` 调整上限，1 固定容量 | 按存活压力扩大、连续低存活时缩小，最近 full 的活跃堆决定基准；旧代压力独立。较高吞吐与内存、单次暂停的取舍见记录；stress／full-only 保持固定策略。 |
| [固定分块的搬迁计划](ADAPTIVE-NURSERY.md) | 所有 profile 默认，无独立开关 | 每块 256 项，保留完整预留、复制和失败回退；消除连续数组倍增复制，每块增加 16 B 链接／计数，无新增对象字段、roots 或 ABI。 |
| [minor 脏卡组清单](DIRTY-CARD-GROUPS.md) | 所有 profile 默认，配套 runtime ABI 17，旧产物须重建 | 每 64 张卡共用 16 B 节点，首次写入登记，STW 只枚举／清零脏组；原有批量卡标记保留完整登记。side metadata 约 0.049%，写屏障固定成本与真实程序回退见记录。 |
| [普通正文导出与跨 Cone 主值构造器内联](BODY-EXPORT.md) | release 默认；扩展成员正文需新版 provider，主值构造可消费既有角色 | 普通字段写入、值／构造表达式和 callable-owned 字符串沿用可移植结构选择；原 96 节点过滤由调用点评分项移除；字符串引用 provider 原对象。secondary／class 初始化、缺少正文或成本不合格的调用保留，profile 间 ABI 一致。 |
| [循环内联优先级与传值调用估价](LOOP-INLINING.md) | release 默认 | 更深循环优先使用累计增长；及时消费已知 receiver，复用完整调用适配；参数／返回值快照参与估价；原执行上限由调用点收益评分替代。累计增长、六层展开和完整冷分支体积限制保持。 |
| [minor 局部块遍历与连续清卡跳过](MINOR-BLOCKS.md) | 新版 runtime，所有 profile 默认 | minor 的准备、汇总、搬迁选择和收尾只枚举年轻块／新目标；可用 region 直接索引，连续清卡由 `memchr` 跳过。对象／TLAB／ABI 不变，保留可选 survivor 的年轻成员关系与所需卡。 |
| [普通 poll 单次 phase 读取与有界叶函数省入口](ACTIVE-POLLS.md) | 新编译器生成的所有 profile 默认 | 普通 Managed poll 不读取 TLS／mode／epoch；既有有界叶函数省去入口 poll，效果仍为 Managed。root／初始化 gateway 保留首次激活，循环与慢边精确 roots 保持。 |
| [启动 site 的 owner 验证合并](STARTUP-OWNER-JOIN.md) | 新版 runtime，所有 profile 默认 | 每个 site 在首次 stackmap 合并时只解析一次 owner；重复 ODR 记录复用验证结果，仍比较完整地址与 payload，全部对应关系在 managed 执行前完成。 |
| [poll 的本线程 relaxed 读取](POLL-OWNER-READS.md) | Gateway 完整检查中，所有 profile 默认 | 全局 epoch／phase 保持 acquire，所属线程的 mode／observed epoch 使用 relaxed；普通 Full poll 已由上项进一步简化。runtime 跨线程发布与停稳协议保持。 |
| [普通内联增长额度调整](INLINE-GROWTH.md) | release 默认 | 基础额度为 `min(size / 2 + 128, 384)`，统一适用于本地、泛型及跨 Cone 正文；后续按上述自然循环区域分配，六层深度和跨轮累计保持，同步 suspend 基础常数仍为 96。 |
| [跨 Cone 普通方法与 getter 内联](MEMBER-INLINING.md) | release 默认；提供方需导出合格可移植正文 | 按完整签名与通用 HIR／MIR 成本选择，保留 receiver 与 typed accessor identity；泛型沿既有具体化路径，未知动态调用及缺少正文的旧库保留入口。 |
| [Scoop 符号在镜像内绑定](LOCAL-SYMBOLS.md) | Darwin 最终链接默认，所有 profile | 从动态导出移除 Scoop 持久符号，直接绑定 callable 与 metadata；已有相容 `.slib` 可重链接，普通 native C 导出保留。 |
| [完整父接口表内嵌与目录共享](INTERFACE-PROJECTION.md) | 所有 profile，统一布局／ABI | 单继承完整前缀、多继承固定偏移；目录复用内嵌片段，菱形保留完整父表。 |
| [父接口直接投影与继承方法直接取槽](INTERFACE-PROJECTION.md) | 所有 profile | 已有 Child 视图且静态知道父关系；泛型按 exact 实例处理。首次 Any 取表等动态路径另行查询。 |
| [smartcast 视图保留与块内 itab 复用](INTERFACE-PROJECTION.md) | release 默认 | 复用已取得的接口视图；动态表缓存仅保存 metadata，遵守局部值写入、取址与块边界的失效规则。 |
| [接口 SSA 快照的直线区间复用](INTERFACE-VALUES.md) | x86_64 release 默认 | 发射阶段复用刚写入的完整接口值及其 niche enum，调用／poll／别名／控制流边界结束复用；后续所有目标的局部槽提升与跨 safepoint 数据流采用上列接口 SSA 能力。 |
| [接口检查与取表合并](INTERFACE-CHECKS.md) | release 默认 | 有非空接口视图消费者的 `is`／`as`／`as?`；普通控制流上复用成功检查，写入、非共同合流与异常入口使其失效。 |
| [循环中稳定来源的接口查询复用](INTERFACE-QUERY-REUSE.md) | release 默认 | 未取址、未写入的参数或所有 CFG 环外单次定义的局部引用；首次实际查询后复用 exact interface 的成功／失败 entry，含 Boolean 与空接口，不增加对象副本或 GC roots。 |
| [普通 class／局部游标字段替换](CLASS-SCALARS.md) | release 默认 | 创建与使用可分析、不逃逸且无 release hook；字段转为 typed locals，保留引用快照与 GC 语义。 |
| [字段替换指导内联](ITERATOR-INLINING.md) | release 默认 | 可替换 class 只剩一个 receiver 调用时共同估价；继续遵守原内联成本与累计增长限制。 |
| [调用点异常路径特化](EXCEPTION-PATHS.md) | release 默认 | exact receiver／closure 和已有完整正文证明异常边不可达后，按实际剩余正文选择内联；保留正常清理、真实异常、返回快照与精确 roots。未知效果和残余 EH 保留调用。 |
| [异常区域中的局部分配消除](EXCEPTION-SCALARS.md) | release 默认 | box／closure／无 release hook 的 class 在正常和异常路径均满足来源、支配与循环存活证明时替换为 typed locals；保留真实 catch／finally、返回快照及异常出口 roots。 |
| [有界整数／Boolean 数组初始化](ARRAY-INITIALIZATION.md) | release 默认 | initializer 展开后无调用／分配；64 次有界分块，合格的无 safepoint 内层交由 LLVM 向量化。 |
| [数组边界检查消除与分块](ARRAY-BOUNDS.md) | release 默认 | 稳定上界、非负起点、唯一加一更新；直接范围证明或长度 guard，保留 checked 回退与短尾。 |
| [循环范围指导数组访问内联](ARRAY-INLINING.md) | release 默认 | 用已证明的范围去掉候选正文中的不可达检查后，参与统一调用点评分。 |
| [严格浮点数组循环](FLOAT-ARRAYS.md) | release 默认 | Float／Double 访问与逐元素运算复用有界路径；顺序归约保留累加顺序，禁止隐含重排与融合。 |
| [含条件分支的数组循环](CONDITIONAL-ARRAYS.md) | release 默认 | 单入口、单 latch、无环正文；合流保留共同事实，整数内层可向量化，严格浮点按实际合法性处理。 |
| [x86 最内层循环入口对齐](CONDITIONAL-ARRAYS.md) | x86 后端默认，无独立 profile 开关 | 向 LLVM 设置 32 字节优先对齐，实际应用由后端决定；其他架构不受此 x86 设置影响。 |
| [数组搜索与提前退出循环](EARLY-EXIT-ARRAYS.md) | release 默认 | 只复制继续迭代的区域，共享 return／break／失败出口；保留首次命中、异常前缀与本轮值。 |
| [引用数组范围证明与有界循环](REFERENCE-ARRAYS.md) | release 默认 | 单字引用、双字 interface、相应可空值与 MaybeUninit；保留写屏障和精确 roots，尚未开放引用 SIMD。 |
| [卡表置脏的幂等原子写入](CARD-STORES.md) | 所有 profile 与 runtime 默认 | 二值 card 使用 relaxed atomic store 1，省去原子读改写，清零仅在 STW；合格 release 循环可按卡标记合并规则处理。 |
| [nursery 独占块的分配登记](NURSERY-METADATA.md) | 使用新版 runtime，所有 profile 默认 | 已保留的年轻块范围直接登记，以原子 load／OR／store 保留已有位；共享旧代仍用原子读改写，保留完整清零与发布顺序。 |
| [已知分配布局特化](TLAB-COMPLETION.md) | release 默认 | 从完整本地 TD／typed array layout 复用对齐与 release policy，跨物理对象分区消除重复加载和分支；缺少事实时保留原路径。 |
| [已知布局的 TLAB 完成统一生成](GC-EVOLUTION.md#第五十阶段已知布局的-tlab-完成统一生成)，[初始实现](TLAB-COMPLETION.md) | release 默认，当前 runtime ABI 25，无新开关 | 完整 nursery 布局的成功 bump 直接清零、设置对象头和大小、发布 starts 并更新 nursery 计数；含较大常量及动态尺寸，不再受 128 B 门槛限制。复用 allocation context，省去完成入口的调用和 TLS 重读；debug、未知布局及 collecting slow path 保留。 |
| [分配 context 的首次使用复用](ALLOCATION-CONTEXT.md) | ARM64 release 默认 | 调用内首次需要 TLAB 时取址，跨循环／GC／catch 只复用稳定的 AS0 context 地址；可变字段继续重读。x86、debug 保留原路径，完成 helper 内部 TLS 访问不变。 |
| [确定初始化前的尾部清零消除](ALLOCATION-ZEROING.md)，含[引用扩展](GC-EVOLUTION.md#第三十一阶段引用字段的确定初始化) | release 默认；对齐后对象大小 ARM64 ≤128 B、x86 ≤64 B | 已知布局、无 release hook 的小型 TLAB 分配；省去下一次 GC／观察／发布前被完整 typed store 覆盖的连续尾部，包含直接引用、接口、null niche 与无 padding 聚合。保留对象头、未证明字节及原发布顺序；慢路径和 debug 完整清零。 |
| [有界引用循环的页表查询提升](CARD-REGIONS.md)，含[接口栈重取扩展](INTERFACE-CARD-REGIONS.md) | release 默认；保留接口栈重取的扩展仅 ARM64；数组基址定位规则适用于所有 profile | 常数有界、目标不变且查询每轮必经，保留原 roots。ARM64 可保留已确认来自本函数栈槽的 volatile 引用重读，同时提升独立查询；x86 仍排除含 volatile 的区域。 |
| [有界引用循环的卡标记合并](CARD-BATCHES.md) | release 默认 | 无 safepoint、2～64 次、每轮必经且槽步长为 0／8／16 字节；退出后、下次 poll 前每流最多 3 次卡标记，保留全部元素写入和精确 roots。 |
| [有界引用读取的四倍部分展开](REFERENCE-UNROLL.md)，含[条件读取扩展](CONDITIONAL-REFERENCE-UNROLL.md) | release 默认 | 最多四个无环块、同一 latch、4～64 次且可被四整除、最多 24 条正文指令的只读内层；保留空值分支、浮点顺序、poll、短尾与精确 roots，不扩大引用 SIMD。 |

低成本且收益稳定的方案优先进入 release 默认设置；需要独立开关或等级的方案，在采纳时补充准确选项、默认值、等级含义和适用条件。所有 profile 继续遵守相同的语言、对象、GC 与产物 ABI 契约。PGO、更广范围的屏障合并和 bulk barrier 仍在后续工作中，不计入本表已采纳项。

[动态接口目录循环展开](INTERFACE-DIRECTORY.md)已作定向测量，因短目录与 smartcast 回退暂未采纳；当前仍使用原逐项查询。

## 测量与诊断

这些能力按需使用，不改变优化的开启状态。

诊断／测量插桩以显式生成开关为边界；关闭时不得进入最终程序，不能以运行时分支代替代码排除。

| 已交付能力 | 开启方式 | 作用 |
| --- | --- | --- |
| [启动阶段测量](STARTUP-STATS.md) | `scoop build/run/test/link --enable-startup-stats` 或 `scoop-link --enable-startup-stats`；所有 profile 默认不编入 | 七个启动阶段和已有索引计数；关闭时预处理排除计时、报告、诊断字符串与专用计数访问。切换只改变 runtime／最终链接，GC stress 可独立组合。 |
| [预构建程序 A/B 测量](../../tests/benchmarks/README.md) | 手动运行 `python3 tests/benchmarks/compare.py`；GC 诊断加 `--gc-stats` | 交错运行相同输入，核对输出并记录墙钟、CPU、RSS；GC 诊断与普通计时分别测量。 |
| [跨 Cone 普通成员专项负载](../../tests/benchmarks/inlining/members/README.md) | 按说明构建 provider／consumer；`SCOOP_BENCH_KIND` 取 0～5，`SCOOP_BENCH_COUNT` 指定次数 | 用户 struct／class 方法、getter、primitive hash、未知接口与标量对照，复用已有 phase 计时及 A/B 工具。 |
| [动态接口目录专项负载](INTERFACE-DIRECTORY.md) | 按 [interfaces 构建说明](../../tests/benchmarks/interfaces/README.md)手动构建；`SCOOP_BENCH_KIND` 取 9～17，`SCOOP_BENCH_COUNT` 指定次数，再用 `compare.py` 对照 | 区分真实装箱／集合目录、命中位置、泛型及短目录未命中、Boolean 和混合查询；保持原静态投影对照。 |
| [MIR 优化决策报告](OPTIMIZATION-REPORT.md) | `scoop build --release --emit optimizations --dump-dir DIR`；`--emit all` 也包含 | 输出实际内联／分配消除的成本与原因；未请求时不保存逐项事件。 |
| [函数级 CPU 采样归因](CPU-PROFILING.md) | 所有 profile 按需 `--emit symbols --dump-dir DIR`；`--emit all` 也包含；手动使用 sample／perf 和 `symbolize.py` | 关联当前及已加载依赖的真实正文、实例和声明；可用 `--binary` 解析实际别名及 sample 地址。不生成指令行表或 PGO 输入。 |
