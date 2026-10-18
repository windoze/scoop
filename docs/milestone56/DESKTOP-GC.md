# M56 补充设计：桌面程序的短暂停与内存占用

日期：2026-10-19。依据用户补充的主要用途：PC／桌面程序，含 60 fps GUI，兼顾低配置电脑；大数据／吞吐型负载较少。本文件是 [M56 设计](DESIGN.md)第 6 节的补充。区分当前实现、已测结果和后续候选；候选不自动成为现行 runtime 合同或本期实现要求。

## 1. 取舍与时间口径

优先级调整为：**低分配成本、短暂停及其尾部、较小实际内存占用，最后才是批量回收吞吐**。吞吐改善不能抵偿明显增加的暂停或驻留内存。60 fps 一帧约 16.67 ms，布局、事件、绘制和操作系统调度都要使用这段时间，不能将整帧分配给 GC。

工程评估先观察普通交互负载的 minor 是否大部分在 1 ms 内、较差样本能否控制在 2 ms 左右；这是待低配置实机和应用校准的性能目标，不是保证、语言合法性规则或 runtime 配额。full 必须单独报告，不能用大量短 minor 稀释少量长 full。当前大图 full 实测仍有数十毫秒，不宣称任意堆大小下维持 60 fps。

必须区分以下指标：

- 握手等待：GC 请求至所有需要参与的线程停稳，包含到达 poll／离开不合作 native 区间的延迟。
- 已停稳暂停：当前 `pause_ns`；它在 RUNNING 发布之前结束，不等于完整阻塞时间。
- 请求至恢复：从本次成功收集请求到 RUNNING／collector 返回的直接观测；与前两项分别报告，不相加各阶段中位数。
- 应用进度：逐帧耗时、超出 16.67 ms 的帧比例、分配突发耗时，以及分配后首次触页的成本。必须计入恢复后的缺页／清零，不将其移出 STW 就当作成本消失。

逐次记录 minor／full、实际复制字节、roots／remembered／mark／plan／copy／update／reclaim／VM 归还；报告样本数量、p50、p95、最大值，样本足够且分布适合时再给 p99。诊断只在实验副本或显式诊断中记录，禁止每次生产分配读时钟。小量进程样本的中位／最大值不能冒充 GC 事件 p95。峰值 RSS、回收后的 RSS、managed committed／mapped、side metadata、线程栈分别看；虚拟地址预留不是等量物理常驻。

## 2. 当前已经有的能力与限制

默认 nursery 为 1 MiB、TLAB 为 32 KiB；minor 和小于 4 MiB committed 的 full 默认一个 worker，其余默认至多四个。worker 按需创建并复用。M56 B1 减少分配时的重复 TD 查询；D2 复用同一 worker pool 并行回写移动对象的引用，未增加第二套 collector。无引用对象不扫描出站槽、零搬迁跳过 update 已经存在。

D1 的最终无额外插桩矩阵支持保留 1 MiB。单 mutator、短命负载的 1／4／16 MiB 候选，七个进程的最大已停稳暂停中位为 0.241／0.957／3.664 ms，峰值 RSS 中位为 13.5／17.1／32.7 MiB；这不是事件 p95。16 MiB 在全存活批次中有吞吐优势，涉及少做 minor／直接触发 full，不能据此作为 GUI 默认。详见 [D1 结果](PROGRESS.md#d1nursery-容量与-refill-取舍)。

该矩阵没有搜索小于 1 MiB 的容量，保留现值不表示已经找出所有桌面负载的最优值。第 5.1 节的短命对象实验显示当前低存活 minor 主要耗在逐槽回收；先消除这部分工作，再以真实 GUI 的对象寿命比较更小容量，并同时观察收集频率、过早晋升与分配成本。

D2 对普通稀疏 region full 的四 worker 对照将请求至返回中位从 41.874 ms 降至 34.297 ms，仍超过一帧。这类大图不是典型每帧工作，但证明仅加 worker 不能解决 full 暂停；需要减少串行工作和必要扫描量。完整工作集、验证方式与单 worker 对照见 [D2 结果](PROGRESS.md#d2搬迁后的并行引用回写)。

普通 full 已只选择稀疏且可获得净 region 节省的搬迁方案；限制一次搬迁的字节数仍不能保证暂停，因为任何搬迁都可能需要更新整个存活图的相关槽，mark 也仍遍历全部可达对象。首次存活即晋升可能使“只活过一轮”的对象进入旧代；若真实桌面寿命数据证明它支配 full 次数，再考虑简单的 survivor 方案，不能从全存活微基准直接推出应增加代龄机制。

## 3. 优先调查的局部改动

| 顺序 | 已确认的代码成本 | 最小候选与正确性边界 | 取舍／验收 |
| --- | --- | --- | --- |
| 1 | `block_sweep.c` 对每个 small block 检查 4096 个 word 位置，对死亡对象执行 release 查询和逐对象元数据清理；普通 nursery 对象没有 release hook | 对已无存活对象，或所有存活对象已搬走且无 pin 的年轻 small block，在全部引用回写完成后整块退役；复用时仍完整清空旧元数据。pinned、部分保留、release 所在旧代、stress poison／quarantine 保持现有路径 | 优先测低存活 minor；同时看下一轮 refill 和分配时间，防止只移动成本。年轻对象资格来自实际分配规则，不能按 TD 名称猜测 |
| 2 | minor 的 begin／mark 归并／source 选择／reclaim 遍历全部 block；remembered 遍历旧堆 card，结束时清全 region card 数组 | 先测固定年轻工作量、逐渐增加稳定旧堆；优先按现有 bitmap 成组跳过零项，以及跳过确定无引用的大对象的 remembered 扫描。若仍支配暂停，再维护明确的 young／dirty 集合 | 不先为每次引用写入增加一个全局锁或全局原子；不能漏跨 card 对象、AtomicRef、旧到新写入和异常退出。full 原路径不变 |
| 3 | small block 的精确大小与多个 bitmap、搬迁 forwarding 表，以及逐存活对象 move plan，带来真实内存与清零成本 | 优先在 full 后释放保留空 region 的闲置 side metadata，按现有首次激活路径重建；评估共享一块连续 metadata 存储。随后才评估稀疏 forwarding 或用可编码 offset 缩小表项 | 保持精确对象边界、pin、published starts 与 forwarding 生命周期；不要把压缩表查询成本转移到每次分配／引用访问。分别测空闲回落与下一轮突发 |
| 4 | full 的 VM discard／unmap 和 page-map prune 在 STW 内；空页 discard 已合并相邻区间 | 先消除无变化区域重复扫描和保留不必要 metadata。只有 VM 调用实测支配尾部时，才评估将已从可分配集合移除的独立空 region 延后归还 | 现有代码已经合并连续 discard，不能把它当成新优化。不能在恢复后对可能重新分配的页做异步 discard；延后归还会增加 RSS、状态与生命周期复杂性，首版不加后台 scavenger |
| 5 | 大堆第一次 full 在停稳后创建 marker 线程；`mark_pool_run` 即使本轮只有一个 worker 仍广播 phase，已创建的空闲线程会被唤醒后再睡眠；单链图并行度低 | 先评估单 worker 直接执行、跳过无用广播；引用回写没有排入任何槽任务时也不必启动空 phase。再单独比较首次与热池 full。必要时复用有限的已有工作存储；实际有空闲阶段时可用现有显式收集预热，不在启动时无条件建满线程 | 测一／二／四 worker、先 full 后 minor 的序列，以及低核／其他应用占用 CPU 时的尾部；保持阶段切换和失败回退正确，不永久保留高水位工作内存 |
| 6 | `NativeBorrowed` 长操作和同步 release hook 可以延长握手或回收 | 先定位慢 native 调用，使用现有合法 pin／NativeSafe 路径或带 roots 的分块处理；资源在应用已有 close／dispose 路径及时释放，release hook 做轻量兜底 | `NoGC` 不是有界时间证明。任意同步 release hook 不能承诺短暂停；移到后台会改变生命周期、顺序及完成语义，不作为简单优化悄悄实施 |

前两项减少必要工作的次数，第三项同时针对 footprint。没有证据时不调整全堆触发阈值、不加 survivor／多级年龄、不增加统一自适应调参器。分块 poll 沿用 [设计 3.2](DESIGN.md#32-poll-的吞吐与响应合同)：只对每次迭代工作有界的 loop 选择编译期内层上限；选择达到吞吐平台的较小块，响应尾部优先，不能仅以平均迭代时间推导硬上界。

大对象清零、clone／大范围复制也要纳入分配突发和握手测量；不得通过删除清零、暴露半构造对象或将无界操作标成 leaf 改善表面计时。OS 缺页、线程失去 CPU 等同样可能延迟到达 poll。LLVM 的 safepoint 文档也将正常情况下的响应与严重 paging 下的最坏情形区分，不能把静态迭代数当作墙钟保证。[LLVM：Garbage Collection Safepoints](https://llvm.org/docs/Statepoints.html)

尤其不能只为取得 C1 的向量化收益，把长时间计算改标 `@NoGC`：没有分配不代表不会拖延其他线程发起的 STW。对 Managed 计算，更合适的后续方向是带外层 poll 的有界分块，保持正常调用者的 GC 合同。

### 3.1 可直接核算的 metadata

当前每个激活过的 small block（32 KiB）有 8192 bytes 的 `size_units`、四份各 512 bytes 的 starts／marks／pins／scanned bitmap，以及两份各 32 bytes 的 line bitmap，共 **10,304 bytes，约为 block 容量的 31.4%**，尚未计 block struct、malloc 元数据、cards、page map、工作队列或对象头。被选为搬迁 source 时，forwarding 的 4096 个指针另占 32 KiB；它在该轮结束释放。统计来自当前 `heap.c`／`evacuation_sources.c` 的结构尺寸，不是 RSS 实测。

full 至少保留一个可复用的空普通 region；若该 16 MiB region 的 512 个 block 都激活过，仅上述常驻 side tables 就可能保留约 5.03 MiB。现有物理页 discard 不会释放这些单独 malloc 的表。这里比简单缩小虚拟 region 大小更有针对性；但全面更换 bitmap／对象尺寸表示会影响分配和查询热路径，必须有实测后再采用。

### 3.2 最终程序的链接信息

对 C1 的实际 Darwin release 测试程序读取 Mach-O headers、符号表和 export trie：文件共 17,492,984 bytes，`__TEXT` 为 1,081,344 bytes，其中 `__text` 指令区为 730,340 bytes；`__LINKEDIT` 达 13,697,024 bytes，包含 5,684,496 bytes 的 export trie 和 5,924,736 bytes 的字符串表。因此不能把约 17.5 MB 文件大小解释为同等大小的机器码或常驻内存。

该程序有 76,560 个导出符号，其中 27,536 个 `bs` 和 27,535 个 `be` 是 Scoop atom 起止边界。当前 codegen 在 Mach-O 对象中保留这些 external alias，以实现对象边界和 weak／ODR 物化；这是链接输入的实际用途，但不自动证明所有符号都需要保留为最终程序的动态导出。

后续优先评估：在完成本次程序的普通符号解析与 ODR 选择后，将只用于产物消费／静态链接的内部边界设为局部或隐藏，并减少最终映像不需要的符号表内容。需要保留实际 native 导出、回调、跨 image 引用与 loader 的绑定语义；不能简单隐藏所有 Scoop 符号，也不删除精确 GC 所需的 stackmap／注册数据。先用普通链接可见性规则与现有最终映像验证确认可行性，再测启动、文件大小和 RSS。上述 Darwin 候选目前仅完成结构测量，没有实现或宣称缩减收益。

GNU 另检查了真实分配基准的 release 产物：文件为 12,822,024 bytes，静态 `.strtab`／`.symtab` 分别为 6,157,549／2,245,272 bytes，均无 `SHF_ALLOC`；没有 Scoop 动态导出。仅对实验副本执行 LLVM 22.1.2 的 `llvm-strip --strip-unneeded`，文件缩到 4,419,056 bytes，约减少 65.5%。原件和副本均完成两百万次分配，checksum `1999999000000`、45 次 minor、0 次 full、1,114,160 copied bytes 一致；该轮只验证结果，没有采纳并行验收期间的耗时。被删除的主要是未装载的静态链接信息，因此这是分发／磁盘体积收益，未证明 RSS 降低。正式采用应放在最终程序验证之后的 release 输出步骤，并补足异常展开、native 绑定与诊断覆盖；`.slib` 对象仍保留消费所需符号。本期未将 strip 加入生产链接流程，也不将 ELF 结果外推到 Mach-O。

## 4. 为什么暂不直接改成增量／并发 GC

增量标记能把工作分散到多帧，但必须在两次扫描之间允许 mutator 改图，维护新的标记不变量和写屏障，并处理 roots、队列、最终收尾、晋升和搬迁的一致性。Scoop 现有 old-to-young card barrier 不能直接当成增量标记屏障。将现有 mark 循环暂停后恢复业务，并不是正确的增量 collector。

Unity 的官方说明也明确：增量模式主要分散停顿，会增加写屏障成本；引用变化过多时可能退回整轮收集。这支持“需单独论证”的判断，不代表 Scoop 可以直接套用 Unity 的非压缩 collector。[Unity：Garbage collection modes](https://docs.unity3d.com/6000.0/Documentation/Manual/performance-incremental-garbage-collection.html)

.NET 的低延迟模式通过减少前台 full 干扰实现交互性，但文档指出它可能增加堆大小和碎片，内存压力仍可能触发收集。对低配置桌面，长期抑制 full 不是免费方案。[.NET：Latency modes](https://learn.microsoft.com/en-us/dotnet/standard/garbage-collection/latency)

因此 M56 先完成上述低复杂性调查和现有分代 STW 优化。如果在代表性桌面 live set 上，减少重复工作后 full 的存活图遍历仍持续造成掉帧，再单独设计增量 major 标记；明确额外写屏障、内存与最终暂停代价。该触发条件是实测问题，不将并发 collector 变成当前交付前置。

## 5. 额外实验范围与后续验收

额外机制实验使用真实 Scoop 产物和仓库外 runtime 副本，保持 1 MiB nursery、相同 LLVM 产物和计算结果。固定短命分配量，对照保留 0／16／128 MiB 无引用旧数组，分别看 minor 与初始化 full；诊断副本记录逐次阶段，生产形态副本核对完整分配速度与 RSS。该工作负载只隔离稳定旧堆与年轻垃圾，不是 GUI 帧率基准，不包含 renderer、OS 输入、磁盘或真实应用寿命分布。

最终桌面评估应补：稀疏／密集旧到新写入，1%／中等存活率，少量大数组，pinned buffer，close／release，单链和宽图，启动／热池／长时间空闲后的突发；在低配置双核或限制本进程 CPU 的环境中核对。通过应用已有帧循环记录漏帧率，并把窗口关闭后的 RSS 回落与下一次打开的延迟一起比较。只有源码级实现、正确性回归与相应平台验证闭合后，才把候选写成默认能力。

### 5.1 本机整块回收原型结果

基线为 D2 runtime，候选只在 `reclamation.c` 增加整块退役分支，其余路径相同。真实 release Scoop 程序分配一千万个 24-byte 对象，验证值总和 `49999995000000`、最后一个对象及保留旧数组的内容；初始化旧数组并显式 full 后才开始 kernel 计时。每种配置先预热一次，再交替运行七个独立进程；全部构建和正确性验证先完成。未固定 affinity，`SCOOP_GC_STATS=1`，无 stress。

所有对照的分配量、root slots、traced objects、229 次 minor 和零复制字节均一致；0 MiB 组总计一次初始化 full，16／128 MiB 组两次，未计入下表的 minor 分位数。该 kernel 在 minor 点没有年轻存活对象，主要隔离死亡 nursery 的回收，不能代表有存活图的搬迁。

| 保留无引用旧数组 | 无额外插桩 kernel，基线 → 原型，中位 [最小, 最大] ms | 峰值 RSS 中位，基线 → 原型 MiB | 诊断 minor 请求至 collector 返回，p50／p95／p99／max，基线 → 原型 µs |
| ---: | --- | --- | --- |
| 0 MiB | 174.952 [173.916, 178.817] → 115.298 [115.123, 116.295] | 12.72 → 12.64 | 251／396／402／427 → 3／3／6／16 |
| 16 MiB | 177.848 [176.904, 185.545] → 119.941 [118.740, 122.747] | 28.84 → 28.78 | 266／288／304／442 → 13／15／18／42 |
| 128 MiB | 192.305 [191.684, 193.841] → 134.324 [133.076, 137.116] | 140.84 → 140.80 | 321／358／359／371 → 75／77／81／92 |

每个诊断格包含七个进程共 1603 个 minor 事件，分位数采用合并样本的 nearest rank；同一进程的事件相关，不能当作 1603 次独立运行或响应上界。本机此次时钟观测为 1 µs 粒度。诊断在独立副本里仅记录收集边界，结束后输出；中位 kernel 与无插桩组接近，但 0 MiB 基线出现过较慢进程，保留其尾部样本，没有剔除。吞吐和 RSS 以无额外插桩组为准。

阶段数据支持两个不同结论：0 MiB 基线 reclaim 的事件 p50 约 249 µs，原型约 1 µs；原型保留 128 MiB 旧数组时，remembered scan 的 p50 约 69 µs，已成为主要成本。其 `old_reference_slots` 仍为零，说明“没有要追踪的引用”并不等于“没有遍历 card 的成本”。kernel 计时包含回收后继续分配，未把下一轮 refill 成本藏到计时区间外。单 mutator 的停稳等待几乎为零，该数据不覆盖多线程不合作调用导致的握手尾部。

原型另用显式全堆验证构建运行 12 组既有引用更新程序（1／2／4／8 worker × normal／minor／moving）和六组真实含引用 nursery 程序（1／8 mutator × 三种存活率），全部通过；覆盖移动、pin、大数组分片、原子引用、循环图和全存活。它仍是实验副本，没有修改生产 runtime，也未完成该候选的正式跨平台验收。建议将整块年轻区退役作为下一项局部 GC 改进，随后测无引用大对象 card 跳过；不据此扩大 nursery 或引入后台 collector。
