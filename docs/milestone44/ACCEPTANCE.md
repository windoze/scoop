# M44 验收核对

> 原始日志、逐次 JSON 报告和压缩归档已移出版本库。本页保留验收范围、结果与限制；文中的历史报告名称仅标识当时的验证批次。

状态：M44 已完成，Darwin 全量与 Linux GNU／musl 适用组合验收通过。日期：2026-10-14。

范围依据 [设计](DESIGN.md)第 8～9 章。实施及失败修复历史见 [PROGRESS](PROGRESS.md)，
性能原始记录与测量限制见 [PERFORMANCE](PERFORMANCE.md)。下文分别列出开发期专测、
最终集成与三目标适用覆盖；原始失败及成功复验均保留。

最终源码与选择核对基于 `29f7f1da`：当前
4053 项发现结果与 Darwin 全量覆盖逐项一致；GNU／musl 最终各选择 1189 项，分别
有 1163／1167 项适用，名称与平台适用性均与准备清单一致。两组在 nuc12 使用固定
release 工具完成，适用项全部通过。固定工具构建后的 Rust 变更仅为两个 `cfg(test)`
模块中的期望修正；当前 runtime 改动与最终性能归档的四份源码一致。终态记录
保留实际执行的源码树与工具摘要，本机和 Linux 的 compiler／runtime／sysroot／tests
源码树完全一致；覆盖索引逐项核对最终结果。

## 交付项

| 项目 | 当前实现与证据 | 结论 |
| --- | --- | --- |
| M44-1 基线与时间口径 | 运行时规范 3.5／3.9 保留既有 pause／stop_wait 含义，增加 STOPPING／COLLECTING／RUNNING 观测；握手测试覆盖发布顺序，Darwin 与 [Linux](PERFORMANCE-LINUX.md) 各场景的最终同条件对照已闭合 | 已完成 |
| M44-2 无搬迁跳过更新 | [collector](../../runtime/src/gc/collector.c)在规划／复制及 fallback 完成后检查本轮 moved_objects；只跳过 relocation，回收／晋升／release／分配状态失效／线程恢复共用后续路径；[独立专测](../../runtime/tests/no_move_update_test.c)覆盖零搬迁及移动后下一轮，见最终 collector 组 | 三目标适用组合通过 |
| M44-3 减少实际更新工作 | [slot visitor](../../runtime/src/gc/collector.c)复用本次 object_meta 结果并条件写回；[forwarding](../../runtime/src/gc/forwarding_internal.h)与[claim](../../runtime/src/gc/evacuation.c)只为新地址重新定位，GC-free 对象跳过出站扫描；[损坏 forwarding 专测](../../runtime/tests/forwarding_validation_test.c)与反汇编保留必要失败出口和一次定位证据；最终平台调用方式和本机回归见记录 | 三目标适用组合通过 |
| M44-4 完整 GC 代码生成 | [function/gc](../../compiler/codegen/src/function/gc/)持有分配、字段／数组／aggregate 写入、原子操作、发布和 poll；失败 CAS 只在成功分支执行屏障；372 项测试、真实写入 fixture与机器码对照通过；Linux 四程序独立根产物逐字节相同 | 机器码及三目标运行组合通过 |
| M44-5 runtime 边界 | [gc_binding](../../runtime/src/gc_binding/)解释根、stackmap、对象 scan 与线程字段；[mutator](../../runtime/src/gc/mutator_internal.h)拥有分配状态并保留原 TLAB 前缀；[普通 runtime 操作](../../runtime/src/gc.h)收拢实际分配／复制／对象检查，操作回归通过；完整边界的本机性能与机器码已核对 | 边界审阅及三目标生命周期组合通过 |
| M44-6 组合与平台 | 工作区覆盖包含 27 个目标的 5629 项成功测试及 2 项 doctest，49 项公共规则通过；正式 CLI 全量覆盖包含 4037 项通过、16 项平台不适用，按名称和适用性核对全部 4053 项；Linux 结果见下一节 | 最终集成、平台及性能证据闭合 |

## 最终平台覆盖

| 目标与范围 | 选择 | 通过 | 平台不适用 | 变体 | 进程步骤 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Darwin/AArch64，正式 CLI 全量 | 4053 | 4037 | 16 | 4561 | 17020 |
| Linux GNU，设计第 9 章适用组合 | 1189 | 1163 | 26 | 1399 | 6971 |
| Linux musl，设计第 9 章适用组合 | 1189 | 1167 | 22 | 1404 | 7001 |

Linux 两组各四个 job，同时执行正确性验收，没有同时做性能测量。两组均正常退出，
无失败或待补项；完整报告与日志见原始归档，
名称、适用性、变体和步骤汇总均经覆盖核对。
选择包含 GC／ABI／根／写入／FFI／线程与生命周期、产物消费、平台和真实测试库组合，
每目标包括全部 105 项 M43 与三个 M44 fixture。Linux 是设计要求的适用组合，
不是全仓 4053 项套件；报告的 `complete=false` 表示定向选择，不表示执行未结束。

## 正确性矩阵

以下为设计第 9 章各范围的专测落点；最终集成与平台组合的覆盖范围见上一节。

| 范围 | 专测／组合与已归档证据 |
| --- | --- |
| 零搬迁、回收与历史计数 | no_move_update_test：无 source 的 full、死亡对象、ready／unready release、pin 的 minor、nursery／TLAB 失效、移动后连续零搬迁；no-move CLI 组 |
| 晋升回退、预留回滚与空洞复用 | evacuation_fallback_test、nursery_fallback_test 与现有 nursery／region 组；最终 31 项 collector 测试包含对应 Rust 驱动 |
| 完整引用更新与精确表示 | [moving-references](../../tests/fixtures/m44-moving-references/)和 [gc-writes](../../tests/fixtures/m44-gc-writes/)覆盖 roots、环、旧代字段／数组、未移动含引用对象、GC-free leaf／数组；引用组合组另含 MaybeUninit、region、Context 与并行值 |
| 根、FFI、线程与生命周期 | collector 根与握手专测；mutator 组及普通 runtime 操作组各 7 项、15 变体、79 步，含 native-scoop-abi-moving-roots、nursery/native/Context/copy、parallel-threads／values |
| pin／handle／release | collector 的 pins、moving、reclamation、no-move、fallback 组；回收与 pin 的 CLI 组；实际 minor 基准核对 pinned 节点到移动节点的引用 |
| 托管写入与发布 | gc-writes 在 debug／release 下运行普通、moving stress、minor stress；字段、数组、含引用值复制、构造、clone、AtomicRef store／exchange／成功与失败 CAS；codegen 专测检查内存序、屏障与 poll |
| 显式验证及错误出口 | collector 驱动的 verify_metadata 配置、moving stress，损坏 forwarding 的真实对象注入／失败消息，初始化与对象表示专测；操作测试覆盖索引保留首轮失败与成功复验对应关系 |
| 产物消费与平台 | 工作区及正式 CLI 全量已完成并逐项核对；Darwin CLI 共 4561 个变体、17020 个进程步骤，包含无源码消费和 artifact-only 链接。首轮产物损坏用例的过期偏移已修复并复验，保留全量原始记录和完整覆盖。GNU／musl 使用最终源码的适用组合全部通过，见独立覆盖记录 |

## 性能与边界核对

以下源码审阅基于 runtime `f300090b` 和当前 codegen；最终 forwarding 调用方式仅由平台层选择，
不改变此处已经收拢的职责。源码审阅证明实现边界，不替代下文的运行和机器码证据。

| 设计边界 | 实际调用与审阅结果 |
| --- | --- |
| 零搬迁及回退 | collector 的 moved_objects 分支位于完整 planning／copy／minor→full 循环之后；统计、验证、mark 清理、finish_collection、临时 pin 解除、锁释放与 resume_world 均在该分支之外；heap_begin_collection 每轮清零 moved_objects，计划执行只在复制和 forwarding 发布后递增 |
| 完整托管写入 | [heap_store](../../compiler/codegen/src/function/gc/writes.rs) 同时执行实际 store 与值屏障，字段和数组元素发射器委托它；[数组构造／复制](../../compiler/codegen/src/function/gc/arrays/)在 GC 模块内完成写入与最终范围屏障，递归值布局保留 interface、struct、enum、MaybeUninit 和 aggregate 的引用范围 |
| 原子与 GC-free 状态 | [atomics](../../compiler/codegen/src/function/gc/atomics.rs)持有实际 store／RMW／CAS 和屏障；CAS success 条件控制屏障，内存序沿调用参数传递。[普通 heap 指令](../../compiler/codegen/src/function/instruction/heap.rs)保留 GC-free 机器状态操作，符合设计允许 GC-free 值继续使用普通操作的边界 |
| 快路径与共享 ABI | TLAB 普通对象上限在 GC allocation 发射器；四级查表和 card 操作在 card 模块，page-map 为 acquire load、置脏为 monotonic atomic OR；ready 在 writes 中为 release，poll 的 acquire 读取在 poll 模块。没有把策略常量重新放回通用 LIR，实际 runtime 符号仍沿现有 ABI 目录共享 |
| 根与线程状态 | GC 核心通过 [gc_binding](../../runtime/src/gc_binding.h)访问真实槽位、冻结 registry 和握手；[threads 适配](../../runtime/src/gc_binding/threads.c)解释 ThreadState。[mutator 私有结构](../../runtime/src/gc/mutator_internal.h)保存 TLAB／block／计数，线程侧只持 opaque mutator，未引入第二份线程目录 |
| 普通 runtime 与堆 | [gc.h](../../runtime/src/gc.h)提供实际分配、检查和含 memcpy／范围屏障的完整复制；boxing／array clone 使用这些操作。[values 适配](../../runtime/src/gc_binding/values.c)持有堆锁、起点／exact type／大小检查；普通 runtime 不再包含 GC 私有头或自行组合私有锁／对象查询，拆箱到外部目的地的普通 memcpy 保留在调用方 |

- 无搬迁 wide／array／chain：最终边界对照每图每侧 35 轮实际 moved_objects=0、update_ns=0；collector 后续必要工作仍执行，独立正确性测试不以计时为分支依据。
- 普通 full 选择性搬迁：引用优化对照与最终边界对照保持逐对图和工作量计数。引用数组 mark 的初轮回退、机器码定位与修复后的完整对照均保留，未删除不利样本。
- minor／old→young／pin：1 worker、4 worker各每侧 35 轮，实际移动、晋升和 remembered 工作量逐对相同，见核对。
- 多线程分配与实际进度：56 个正式进程覆盖计算／分配及 1／4 mutator。记录 T2 后实际增长的迭代数，明确 1024 次采样粒度、调度影响与重试计数的限制，不把 RUNNING 发布当作已经恢复。
- 编译器／runtime 边界：生成代码和普通 runtime 操作反汇编与四程序对照证明对应已测路径没有额外热路成本；GC 核心不直接解释 thread 私有字段或 scan 编码，普通 runtime 不组合 GC 私有锁／对象查询。

上述列表链接为 Darwin／AArch64 同机结果。[Linux 性能记录](PERFORMANCE-LINUX.md)
已覆盖整体优化、独立 runtime 边界、实际进度及四程序对照。Linux 的整体 update 收益
成立，边界拆分后的 minor 回退已通过 x86-64 普通内联解决；对齐和 full/minor 分流等
失败实验均保留，未合入生产。GNU／musl 的最终适用功能验收全部通过。
不能由这些小样本推导所有程序的固定加速比或稳定尾延迟。

内联 forwarding 的隔离候选已恢复 GNU minor 更新中位、完成六组同机对照，并通过
Darwin 31 项 collector、Linux 387 项 codegen/runtime 及三 target 的三个 M44 fixture；
见 [GNU 记录](PERFORMANCE-LINUX.md#内联-forwarding-的-gnu-验证)。最终平台策略让 AArch64
保持外部调用，消除统一内联带来的数组 mark 回退；Darwin 六组最终对照已完成，见
[最终记录](PERFORMANCE.md#最终平台调用策略)。全部 runtime 对象逐字节核对与本机
31 项 collector／三个正式 fixture 通过，性能门与最终平台覆盖均已闭合。

## 完成结论

M44-1～6 全部完成：零搬迁省去 relocation，实际更新复用定位并减少无效写入／扫描，
codegen 和 runtime 的职责边界已收拢，既有同步、布局与 ABI 保持；本机集成、三目标
适用组合和同条件性能对照闭合。初轮失败、未采用的性能实验及成功复验映射保留，
不把阶段时间改善推广为固定吞吐或尾延迟保证。设计第 10 章的后续方向没有纳入本期交付。
