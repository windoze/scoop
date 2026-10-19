# Minor／full 暂停工作量：M58 调研

本文属于 [M58 设计](DESIGN.md)，承接 M56 的[桌面短暂停调研](../milestone56/DESKTOP-GC.md)。2026-10-19 的实验基线为 `04d28e763`；设计整理基于包含 M55 的 `c3b8942db`，本轮涉及的 GC 实现未变化。下面的两个 runtime 优化仅在仓库外实验副本中验证，尚未实现为生产能力。

## 1. 先区分三个时间

以一次成功取得 collector 身份的请求为单位：

| 口径 | 当前实现边界 | 用途与遗漏 |
| --- | --- | --- |
| `stop_wait_ns` | 调用 `scoop_gc_stop_world()` 至返回 | 包含协调、等待其他 mutator 停稳；不是纯粹执行某一条 poll 的时间 |
| `pause_ns` | 停稳后开始，至 `record_pause()` 前 | 包含 heap／roots 锁、GC 主体；结束点在解锁和 `resume_world()` **之前** |
| 请求至 collector 返回 | 调用前后计时 | 包含握手、主体和恢复调用；不保证 GUI 线程已经得到 CPU，也不等于一帧的延迟 |

`collector.c` 的阶段字段只覆盖 root、remembered、mark、plan、copy、update、reclaim、VM return。begin/reset、pool 准备、pin／TLAB 退役、live chunks 释放等留在其余时间中。逐事件计算 `pause − 各已计阶段之和` 才能观察这部分；不能相减几个阶段的独立中位数。

minor 晋升预留失败后，当前实现会在**同一次停顿**内重新 full mark，最后只递增 full 次数；`promotion_fallbacks` 与该事件必须一起解释。加入另一线程正在进行的收集、未取得 collector 身份的请求，也不能混入成功 collector 的阶段统计。

对于 60 fps，16.67 ms 是整个帧的时间，并非 GC 可独占的预算。minor 短暂停、full 尾部、分配触发频率、正常业务耗时和峰值 RSS 应分别报告；降低 GC 次数不自动说明单次暂停缩短。

## 2. 当前暂停里具体做什么

| 阶段 | Minor | Full | 当前可定位的额外工作 |
| --- | --- | --- | --- |
| 准备 | 退役 TLAB，遍历 block，重置年轻 block | 重置全部参与收集的 block、旧分配 cursor／free runs | 遍历会经过旧 block 元数据，但 minor 的 worker 计数数组只清零本轮年轻 block 数量，不能描述成清零全堆数组 |
| 根 | stackmap、native roots、静态根、pin／handle 等 | 同左 | 活跃线程和根数量均影响成本；移动后还要回写 |
| Remembered set | 遍历所有 region 的 card bytes，再扫描 dirty card 覆盖的旧引用槽 | 无此阶段 | 无引用大对象、全零 card 也要逐 byte 检查；发生搬迁时 update 又做一次 |
| Mark | 只追踪可达年轻对象；默认一 worker | 追踪整个存活图；大堆默认至多四 worker | 图宽度、引用槽数、工作任务决定并行收益，不能只按 heap bytes 预测 |
| Plan／copy | 为可移动年轻对象预留旧代目标并复制 | 选择稀疏 region；准备目标空洞、预留、复制，失败可放弃普通压缩 | source forwarding 表每 small block 另用 32 KiB；准备失败也可能已做大量扫描 |
| Update | 根、dirty old slots、年轻存活对象的出站引用 | 根和全体存活对象的出站引用 | M56 已并行化 live update；root 与 remembered 仍由 coordinator 完成 |
| Reclaim | 晋升／退役年轻 block、处理 target | sweep、release hook、重建空闲行 | 当前 small sweep 检查每个 8-byte word；全死或已搬空年轻 source 也逐对象退役 |
| VM／收尾 | 扫 region、清 card、更新统计等 | 还会 discard 空页／unmap 空 region | minor 不做 ordinary region 的 full discard，但并非完全不遍历 region；card reset 仍按 region 容量清零 |

源码入口：[collector.c](../../runtime/src/gc/collector.c)、[heap_objects.c](../../runtime/src/gc/heap_objects.c)、[mark.c](../../runtime/src/gc/mark.c)、[remembered.c](../../runtime/src/gc/remembered.c)、[evacuation_plan.c](../../runtime/src/gc/evacuation_plan.c)、[block_sweep.c](../../runtime/src/gc/block_sweep.c)、[reclamation.c](../../runtime/src/gc/reclamation.c)。

## 3. Minor：从全死扩展到混合存活

### 3.1 方法

Apple M4 Pro／macOS 27.0.1，LLVM／Clang 22.1.8。使用真实 runtime、真实 OS 线程／VM、现有测试的 synthetic AArch64 stackmap 和 native roots；单 mutator，固定一 worker，普通非 stress 收集。runtime `-O2 -Wall -Wextra -Werror`；测试入口禁用函数内联，以保持 synthetic managed anchor 位于独立的栈帧，runtime 本身仍正常优化。

每事件分配 8192 个 32-byte 含引用节点，即 256 KiB，随后显式 minor；不改变默认 1 MiB nursery 阈值。保留 0／128 MiB 无引用旧数组，以及一份 8192 槽的旧引用数组。链模式由 native root 连接所选节点；card 模式每 16 槽存一个年轻对象并执行真实写屏障，恰有 512 个年轻对象存活，但 dirty cards 覆盖 8192 个旧引用槽。每轮前显式 full 清理上一轮晋升垃圾，**该 full 与分配不计入 minor 事件时间**。事件计时包围测试 helper，包含 synthetic anchor 的发布／撤销，不是生成代码 poll 快路径的耗时。

三版本：基线；仅年轻 small block 整块退役；整块退役加无引用 large region 的 remembered-scan 跳过。全部构建、显式 heap verification 和针对性正确性测试完成后计时；每格预热一进程，再交替正反顺序运行五个独立进程，每进程 100 个事件。下表是 500 个事件合并的 nearest-rank p50／p95，单位 µs；同进程事件相关，不是 500 次独立实验。未固定 affinity，本机观测时间粒度为 1 µs。

### 3.2 结果

| 存活／引用方式 | 无引用旧数组 | 基线 p50／p95 | 只整块退役 | 再跳过无引用 card |
| --- | ---: | ---: | ---: | ---: |
| 0 | 0 MiB | 63／66 | 1／2 | 1／2 |
| 0 | 128 MiB | 129／135 | 69／73 | 3／3 |
| 约 1%，链 | 0 MiB | 69／78 | 12／15 | 12／13 |
| 约 1%，链 | 128 MiB | 201／212 | 145／157 | 13／15 |
| 1/16，链 | 0 MiB | 101／108 | 37／40 | 38／43 |
| 1/16，链 | 128 MiB | 229／243 | 171／180 | 38／42 |
| 全存活，链 | 0 MiB | 555／586 | 482／506 | 501／524 |
| 1/16，旧数组持有年轻引用 | 128 MiB | 285／299 | 226／243 | 97／108 |

这是**请求到返回**，单 mutator 的握手接近零，不能用于推断多 mutator 的 stop-wait 尾部。代表性的 p99／max：128 MiB＋约 1% 链为基线 225／242 → 组合原型 22／33 µs；card 模式为 312／329 → 116／123 µs；全存活为 616／949 → 539／559 µs。尾部是本轮样本观测，不是暂停上界。

阶段计数在三版本间一致：约 1% 为 82 个 traced objects／2624 promoted bytes，1/16 为 512／16384，全存活为 8192／262144；card 模式每事件仍检查 8192 个旧引用槽。未发生晋升失败或计时区间内 full。每轮校验存活节点数量、内容、链／数组引用、晋升代别及旧数组端点；最后清根 full 后 live count 为零。

两个原因可以独立定位：

1. 0 MiB／全死的 reclaim p50 从约 62 µs 变为低于 1 µs；1/16 链从约 69 降至 5 µs。已移动的 source 也能整块退役，所以收益不局限于零存活。
2. 128 MiB／约 1% 链，基线 remembered p50 约 65 µs，update 约 67 µs；组合原型分别约 0 和 3 µs。因为存在搬迁，同一份完全无引用旧数组的卡表原先扫描了两遍。card 对照仍保留必要扫描，remembered／update 约为 33／45 µs，没有把真正的旧到新引用漏掉。

全存活时 mark／copy／update 支配暂停，整块退役只能省 source 的 sweep；组合原型比单独退役略慢，没有证据把这点差异归因于某个具体机制。不能把全死时的倍数外推到全存活图，也不声称本实验改善了端到端分配吞吐或 RSS：分配、重置 full、下一轮 refill 不在这一计时区间。旧的连续分配／RSS 对照仍见 [DESKTOP-GC 5.1–5.2](../milestone56/DESKTOP-GC.md#51-本机整块回收原型结果)。

### 3.3 建议实施的边界

**优先级最高：年轻 small block 整块退役。** 仅普通 minor；block 仍是 young，且 live bytes 为零或已被合法选择为 evacuation source。后者的存活对象已经复制、所有根和引用已经回写，选择器已排除含 pin 的 block。release-hook 对象按现行合同预晋升，不在这条年轻快速路径中。先累计 promoted bytes，再走现有 `scoop_heap_release_block()`；复用时由现有 activation 清空 starts／marks／pins／size 等表。不提前销毁 forwarding，不跳过 stress poison／quarantine，不推广到可能需要 release hook 的旧对象。source 与 target 必须分开处理。

**同批可独立实施：跳过无引用大对象的卡表遍历。** 在 large region 入口检查 active old head 的既有 TD：`object_scan == NULL` 则无需访问卡表。使用已发布对象的类型事实，不按数组名判断，不重新查 TD registry。标记与回写复用同一个枚举入口，因此两次都受益；真正含引用的数组保留原流程。此原型没有省掉卡表分配或收尾清零，不能报成 footprint 收益。

这两项不需要新 GC 算法、写屏障、年龄层或公开 ABI。正式实施先在运行时规范 3.9 写明退役条件与复用职责，再做实现。实验组合原型通过现有 `nursery_test`、`parallel_update_test`、`nursery_fallback_test`，覆盖 pin／handle、release、1／2／4／8 worker 的别名引用回写、stress full 与晋升预留回滚；另外三版本各跑八格显式 heap verification。尚缺正式源码 fixture、GNU／musl、真实多 mutator 和 GUI 突发验收。

## 4. Full：并行 mark 后仍剩下什么

### 4.1 同机基线分解

同一基线 runtime，每进程建立 1,048,576 个 32-byte 节点及一份 8 MiB 引用数组；先 full 将图整理到旧代，再测一次普通 full。单链模式也保留等尺寸、内容为 null 的引用数组，以控制存储规模，因而不是纯单链并行度测试。宽图由数组直接引用独立节点；稀疏模式在整理后清掉 15/16 数组槽，数组本身仍存活。四 worker 的池已预热，不能代表第一次 full。

每格预热一次、七个独立进程，交替 worker 顺序；以下各字段分别取中位，单位 ms，不能把行内中位数相加推导精确总时间。

| 图 | workers | 请求到返回 [min, max] | mark | plan | copy | update | reclaim | VM |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 单链＋空数组 | 1 | 27.918 [27.643, 28.451] | 16.794 | 5.235 | 0 | 0 | 5.831 | 0.008 |
| 单链＋空数组 | 4 | 24.916 [24.413, 26.488] | 13.984 | 5.202 | 0 | 0 | 5.786 | 0.009 |
| 宽图，全存活 | 1 | 28.458 [27.957, 29.348] | 17.393 | 5.191 | 0 | 0 | 5.824 | 0.009 |
| 宽图，全存活 | 4 | 19.079 [18.816, 19.852] | 7.958 | 5.230 | 0 | 0 | 5.825 | 0.012 |
| 宽图，保留 1/16 | 1 | 22.201 [22.089, 22.418] | 4.709 | 5.122 | 0.653 | 5.131 | 6.211 | 0.244 |
| 宽图，保留 1/16 | 4 | 16.626 [16.302, 16.981] | 1.598 | 5.119 | 0.652 | 2.529 | 6.305 | 0.236 |

全部存活为 1,048,577 个对象、约 40 MiB 对象存储；稀疏图为 65,537 个对象、约 10 MiB 对象存储，成功压缩两个 source region。这些 bytes 含对象头，未计 GC metadata。minor 原型没有应用于这张 full 表；这是瓶颈调查，**不是 full 优化前后对照**。

宽图 mark 可明显并行，plan 与 reclaim 基本不随 worker 数减少。即使四 worker，该合成图的 full 仍可能占去一整帧；持续增大池不能解决串行部分，也可能争用 GUI 的 CPU。七个事件不足以报告可信的 full p95／p99。

### 4.2 零搬迁也可能花费压缩准备时间

密集图的 `last_full_source_regions == 0`、copy／update 为零，**不代表从未尝试压缩**。当前代码仅在最初选中的 source 数为零时早退；选出 source 后，先扫目标 block、退役死对象并建立 free runs，再为 live objects 预留目标，可能最后失败并回滚。

在独立诊断副本重放宽图全存活样本：选中一个候选 source，选择约 30 µs，`prepare_evacuation_targets()` 约 3846 µs，live 遍历／预留约 1178 µs，预留 2024 个 move 后失败；没有发布复制或 forwarding，正常放弃压缩。诊断仅用于确认路径，不混入上表计时。这解释了该负载约 5 ms 的 plan，不能把它误报成复制了 5 ms。

适合接下来的局部改进：

| 顺序 | 方案 | 约束与需要验证的收益 |
| --- | --- | --- |
| 1 | 对只有一个 source、明显没有足够既有目标空间的情况，先做必要容量检查 | 根据真实空闲 block／line 和现行“占用新目标 region 数少于释放 source 数”的条件，计算可用容量上界；连上界都不够才跳过尝试。不能把总空闲 bytes 足够当成一定能放下，仍保留原预留与回滚；不建立额外规划框架 |
| 2 | sweep／目标准备按 starts bitmap 的非零位枚举对象，跳过空 bitmap word | 现在逐 8-byte word 检查 starts；可用 `ctz` 枚举置位，沿用原 marked／pin／release／size 逻辑，源位图会被清位时先取当前 word 的快照。不增加另一份对象列表 |
| 3 | 少量有界工作存储复用、单 worker 直接执行、没有 update task 时跳过空 phase | 先分开测冷池与热池；当前首次线程创建和 counters 扩容在 STW 内。避免每轮唤醒不参与 minor 的闲置线程，也不永久保留任意高水位内存 |
| 4 | 应用已有空闲点主动收集／及时释放大型文档对象 | 只改变可控的触发时机，不承诺分配压力下绝无 full；保留紧急路径，不新增调度服务或“低延迟模式”状态机 |

前两项需要新的独立对照，本文不宣称已经消除 5 ms 或给出 full 加速比。继续并行化更多阶段应晚于这些检查；同步 release hook、巨大根集合和存活图本身的遍历仍可能决定尾部。metadata footprint 及增量 major 的进入条件沿用 [DESKTOP-GC 3–4](../milestone56/DESKTOP-GC.md#3-优先调查的局部改动)，不因本次图测试直接升级为并发 collector。

## 5. 实施后的验收重点

按 dead／约 1%／1/16／全存活，链／宽图，稀疏／密集 dirty cards，pin／release，普通／晋升失败，首次／热池组织对照。分别报告事件级 stop-wait、pause、请求到返回和 full 失败尝试；连续分配另测吞吐、refill、RSS 和空闲后再次打开的成本。用有输入／布局／绘制的实际帧循环观察漏帧，不能把上述显式收集 harness 叫作 GUI benchmark。

本轮未使用 nuc12，没有远端临时修改。实验代码、runtime 对照副本、私有编译器／sysroot 和原始结果均在仓库外；调研结束后清理，只保存本文的范围、结果与限制。
