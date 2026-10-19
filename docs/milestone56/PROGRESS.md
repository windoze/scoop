# M56 实施记录

状态：实现与验收完成，2026-10-19。A–D 交付及 E／桌面调查范围以 [DESIGN.md](DESIGN.md) 第 9、10 节为准；候选能力不计为已实现。最终结果与清理见 [ACCEPTANCE.md](ACCEPTANCE.md)。

分支已从 `main` 独立迁移到 `codex/m56`；以下保留原验收环境和提交编号。M54 专属的 zstd 修复以完整补丁随迁移保存，当前分支的应用状态与迁移核对见 [验收第 5 节](ACCEPTANCE.md#5-从-main-独立迁移)。

## 交付检查表

| 项目 | 状态 | 验证／取舍 |
| --- | --- | --- |
| A：当前 release 工具、开发 sysroot 与真实程序基线 | 完成 | 三目标工具／16 Cone／runtime 资源与同机对照 |
| B1：内部 TD 验证结果复用、native 边界保留 | 完成 | 分配、非法 TD 拒绝、移动与 minor；两端性能复核 |
| B2：unbox／clone 单次对象定位 | 完成 | 非法地址／类型、large clone、移动与含引用值 |
| B3：String 有界表示 intrinsic | 完成 | 三个 typed intrinsic；UTF-8、越界、旧 extern 与移动 GC |
| C1：NoGc 且无 site 的循环优化及 Managed 保守优化 | 完成 | 三目标向量代码；poll／EH／原子保留，Linux ELF 分区问题闭合 |
| C2：局部 card 查询／mark 复用 | 完成 | 跨 card／调用／原子／GC 边界覆盖；Linux 未测到稳定加速 |
| 分块 poll 候选调研 | 完成调查 | 有界内层、CFG／roots 与真实移动握手实验；未启用自动变换 |
| D1：nursery／refill 对照与策略取舍 | 完成 | 保留 1 MiB；本机完整矩阵与 Linux 代表点支持该取舍 |
| D2：有搬迁 GC 并行 update | 完成 | 复用 pool；单 worker 恢复直接回调，保留剩余差异与完整区间 |
| E：声明构建、局部 box／协程消除、跨 Cone 内联 | 完成进入调查 | 阶段及真实程序测量完成，候选没有扩大为本期实现 |
| 桌面短 STW 补充调研 | 完成调查 | 年轻区退役原型、metadata／链接信息、旧堆成本与 Linux 两核限制复核 |
| 本机阶段验证 | 已通过 | 每批格式化／lint、相关 crate／runtime 与正式 CLI；最后补验 31 项 collector 和 17 个变体 |
| Linux 完整验收 | 已通过 | Rust 4976 项、runner 50 项；GNU 4220 通过／33 不适用，musl 676／8，原失败闭合 |
| 逐功能提交、临时资料与 target 清理 | 完成 | 实验源码与工具恢复；两端临时目录及远端 worktree 清理，既有用户文件保持 |

## 工作环境与验证策略

本机 Apple M4 Pro／macOS 27.0.1，LLVM 22.1.8；Linux 从 `nuc12:~/repos/scoop` 新建独立 worktree，并为本任务使用独立 target、TMPDIR 和测试缓存，不修改其他 agent 的 checkout／构建输出。开始时远端 HEAD 为 `3cbc339f8`，完整验收前将本次提交导入独立 worktree。开始时本机 target 为 4.8 GiB、远端原 checkout target 为 1.6 GiB；按阶段清理无用构建产物，避免每个小改动反复全量构建。B1 验证后删除 2.6 GiB 的旧 incremental 缓存，本机 target 从约 6.6 GiB 降至 4.0 GiB；本任务设置 `CARGO_INCREMENTAL=0`。

仓库外工作目录为 `../scoop-work/m56-20261019`，包含 scratch、基线／候选测量与原始日志；命令显式设置 TMPDIR。性能样本先完成构建，再串行交替运行。正常应用使用显式准备的开发 sysroot，runtime 专测继续用已有源码构建测试入口。

Linux 独立 worktree 已创建为 `/home/chenxu/repos/scoop-m56-20261019`，分支 `codex/m56-validation-20261019`，起点为 B2 提交 `91df2bc22`；后续仅向该分支导入本任务提交。独立工作目录为 `/home/chenxu/repos/scoop-m56-work-20261019`，target／scratch／日志和测试输出均使用该目录，sysroot 属于新 worktree。未修改主 checkout 或其他 worktree。

已有的 M47 两份文档和 M55 设计不属于本次提交。初始内容摘要在仓库外记录，收尾核对三份文件均未变化；不使用全仓 reset／checkout，也不删除／隐藏源码或工具来验收缓存。

## 实施与验收记录

### B1：复用已登记的 TD

先补充运行时规范 2.2。`value_shape` 直接消费 generated-code、对象头和内部已登记 TD，保留动态 kind／size／count／alignment／overflow 检查；公开 native `scoop_rt_alloc` 在进入内部实现前检查登记。没有新增 unchecked 公共入口、凭证或第二套尺寸算法。C 回归新增合法 native 分配控制组及未登记、null、不可读 TD 的拒绝覆盖。

验证：C 使用 LLVM 22.1.8 clang-format（4 空格、100 列），Rust `cargo fmt --all` 与 `cargo clippy --workspace --all-targets` 通过；`scoop-codegen` 的 30 项 runtime collector 测试通过，包含 normal／moving 表示用例与 native 边界。fixture runner 的 50 项规则测试通过；新增 `m56-runtime-allocation`、`m56-runtime-representation` 共 8 个 debug／release／moving／minor 变体通过，无 golden。首次分配用例误用要求 Node.equals 的 Option 比较，改为已知长度遍历后通过，没有改语言规则迁就测试。

真实程序对照使用同一 release 编译器和 core，分别显式选择原始 runtime 副本与当前源码。循环分配两百万个 24-byte Node，写入 1024 项保留环并累加数值；末尾检查保留值和 checksum `1999999000000`。原生 monotonic clock 只在循环外取时，诊断 `SCOOP_GC_STATS=1`。先完成构建再串行测量，一轮预热、七轮交替独立进程，不固定 affinity：

| 版本 | kernel 中位 [最小, 最大] ms | ns／分配 |
| --- | ---: | ---: |
| `28fab1812` runtime | 71.363 [69.767, 75.752] | 35.682 |
| B1 | 43.332 [42.524, 43.915] | 21.666 |

该工作负载约减少 39.3% 时间；全部样本均为 45 次 minor／0 次 full、48,008,432 allocated bytes、1,114,160 promoted／copied bytes，dirty cards／root slots／traced objects 等工作量也一致。这是包括循环、引用写入与实际 GC 的 kernel 成绩，不是所有应用的加速比；Linux 结果见[最终验收](ACCEPTANCE.md)。

环境准备：release 工具、core、runtime 和第三方 native 后端已准备。完整 sysroot 首次在尚未准备好的 IDNA header 处停止；依赖已随后补齐，继续复用已有结果准备其余 Cone。原始构建、测量 JSON 与 fixture 工作目录均在仓库外。

### B2：拆箱与克隆复用单次对象定位

先补充运行时规范 2.3。两个表示入口在原有 heap lock 内调用一次 exact-size 查询，在读取对象头前确认对象起点，并用所得尺寸校验动态 shape；删除重复的 object-start 查询，没有增加新查询接口或跨 safepoint 缓存。保留类型／枚举父类型匹配、长度与实际分配尺寸检查。非法地址仍明确 abort，诊断沿用 exact-size 查询。

格式化、workspace clippy 和 descriptor-driven representation 专测通过；该专测在 normal／moving 两种模式覆盖内部地址、null、非对象地址、未匹配类型及动态 size。正式 CLI 的 3 个 fixture 共 12 个变体通过，新增整数枚举按原身份与父整数拆箱、错误转换异常、large array 克隆独立性；原含引用／ZST 用例继续通过。完整开发 sysroot 已准备成功。

同一 release 编译器／core 对照 B1 与 B2 runtime：一个 `Any` 中的 Pair 在循环内拆箱两百万次并累加两字段，checksum `30000000`；一轮预热、七轮交替独立进程，构建完成后串行测量。B1 中位 112.989 ms [112.512, 114.101]，B2 102.124 ms [101.818, 102.482]，约减少 9.6%。全部样本 allocated bytes 为 224，无 collection／复制／晋升；该结果包含类型测试、锁与循环成本，不代表所有 Any 使用方式。Linux 结果见[最终验收](ACCEPTANCE.md)。

### B3：String 有界表示操作

先同步语言规范 11.4、运行时规范 2.4 和实现规范 2.10。core 的 byteLength、byte-at 和单标量 cursor 解码使用三个有完整签名的 typed intrinsic，HIR／MIR／LIR 沿已有分层传递，LLVM 保留 AS1。内部 NoGC leaf 使用独立的 `_leaf` 符号，复用原表示实现；原 Managed extern 符号及显式调用合同保持。索引检查继续保留，不扩大到扫描整串的 length、hash 或比较。

格式化与 workspace clippy 通过；相关 String 单元测试 24 项、intrinsic wire 测试 2 项通过，最终符号版本的 AS1／无 native transition 专测与 normal／moving C 表示专测也通过。C 专测首次缺少 String 实现的链接输入，补齐现有 strings／utf8 源文件后通过。release 工具和全部 16 个 Cone／runtime sysroot 已重建；新增 2 个 fixture 的 8 个 debug／release／moving／minor 变体，以及现有 m26-strings、m33-utf8 共计 17 个 fixture、26 个变体、64 个进程全部通过。覆盖旧 extern、跨 Cone、ASCII／NUL／多字节 UTF-8、非法索引、迭代、异常和移动 GC。

真实程序使用同一 320-byte／128-character String，对比 B2 与 B3 的 release 产物。一轮预热、七轮交替独立进程；三个 kernel 分别执行两百万次 byteLength、四千次 toByteArray、四千次字符遍历，checksum 均为 `19703556000`。统计开启，构建完成后串行计时：

| kernel | B2 中位 [最小, 最大] ms | B3 中位 [最小, 最大] ms |
| --- | ---: | ---: |
| byteLength | 33.725 [33.288, 34.080] | 5.405 [4.878, 5.500] |
| toByteArray | 21.103 [20.865, 21.536] | 1.987 [1.949, 2.061] |
| iterator | 20.679 [18.766, 21.611] | 3.132 [2.910, 3.182] |

三个 kernel 时间分别约减少 84.0%、90.6%、84.9%。所有样本均为 1 次 minor／0 次 full、1,638,496 allocated bytes、400 promoted／copied bytes、38 root slots、3 traced objects。收益来自这些频繁的小操作，不能外推为整个 String 库或应用的加速比；Linux 结果见[最终验收](ACCEPTANCE.md)。原始数据保存在仓库外。此阶段又清理 1.7 GiB 的旧 incremental 产物，target 约 7.0 GiB。

### C1：按 typed effect 选择普通优化管线

先同步实现规范 2.19。使用 LLVM 22.1 的逐函数 pass 入口：typed NoGc 且没有实际 site 的定义启用循环规范化、LICM、indvars、loop／SLP vectorization；其余定义加入 reassociate、关闭 PRE／load PRE／回边拆分的 GVN 和 BDCE。debug 保持原管线，Managed 每条回边 poll 不变，最终 roots 仍由原 finalization 定稿。没有全局替换 default O2、重新编号 site 或启用 fast-math。

格式化、workspace clippy、全部 227 项 codegen 测试通过，包含 runtime collector、invoke／unwind、实际 root count、机器 tail duplication、原子和握手。新增三目标专测确认 NoGc release 产生 `<4 x i32>`；debug 和无 site 的 Managed 控制组保持标量。严格浮点重结合负控制通过。新增 NoGC／Managed 循环 2 个 fixture 的 7 个变体通过，覆盖零迭代、非整向量尾部、整数回绕、重叠指针、嵌套 continue／break、try／catch／finally、原子发布与移动根；现有四 mutator 原子／GC fixture 的 2 个变体、12 个进程也通过。

同一 4096-element 数组分别进行 5000 轮转换，真实 Scoop NoGC 指针借用与正常 Managed 数组路径各自对照 B3；checksum `25188352`。一轮预热、七轮交替独立进程，保留统计和所有 poll：

| kernel | B3 中位 [最小, 最大] ms | C1 中位 [最小, 最大] ms |
| --- | ---: | ---: |
| NoGC 指针循环 | 5.366 [5.298, 5.557] | 1.104 [1.096, 1.190] |
| Managed 数组循环 | 88.396 [86.706, 92.611] | 87.437 [84.793, 113.719] |

NoGC kernel 时间约减少 79.4%；反汇编在对应 callable 确认两条 `mla.4s`，标量循环扩为带别名检查与尾部路径的向量循环（该函数约 14→40 条指令）。Managed 差异落在离散范围内，不能声称稳定加速；poll 仍主导此类短循环。两版可执行文件均为 17,492,984 bytes，`__TEXT` 均为 1,081,344 bytes，段对齐掩盖小函数的局部增长。全部样本无 GC；进程 allocated bytes 为 33,200，C1 单个耗时字符串更长的样本为 33,208，差异发生在 kernel 后的数字输出。当前应用构建约 3.43 s（与 fixture 构建并行，仅作观测，不能作为冷编译 A/B 结论）；编译开销的规模化对照与 E 合并记录，Linux 性能及完整验收见 [ACCEPTANCE.md](ACCEPTANCE.md)。

### C2：普通字段写入的局部屏障复用

先同步运行时规范 3.6、实现规范 2.5 与设计 5.3。只保留当前直线区间一个 LIR 对象的 region base／cards；同一对象跨 card 字段分别寻址和原子标记，只有精确相同槽重复写入才省 mark。任何 block 切换、local／raw 写入、调用（含 NoGc）、poll、原子访问或发布均丢弃状态。原数组／aggregate 范围与 AtomicRef 屏障继续独立，未引入 dirty 预读分支、跨 safepoint 缓存或对象 ABI 变化。单独模块保存该局部状态，复用现有扫描偏移和 card 算法。

格式化与 workspace clippy 通过；moving_gc 相关 16 项、原子 3 项通过，最终局部屏障 5 项专测再次通过。专测断言同槽、相邻槽与跨 card 的查询／mark 数、receiver 变化、NoGc／Managed 调用、原子和 block 边界。新增 2 个 fixture 的 8 个变体、现有 aggregate／array／clone 写入 2 个变体／12 个进程，以及四 mutator 原子并发 2 个变体／12 个进程全部通过；含正常、moving、minor、跨 card 大字段、成功／失败 CAS。

真实程序先将 Holder 提升为旧对象，再循环两百万轮，每轮写四个引用字段，末尾 checksum `10`。一轮预热、七轮交替独立进程，C1 中位 27.796 ms [25.905, 30.051]，C2 25.378 ms [24.917, 25.975]，约减少 8.7%。对应热 callable 的 acquire 指令静态数从 40 降为 16，构造函数从 20 降为 8；四个不同字段仍分别标记。所有样本均 allocated bytes 328、minor 0／full 1、root slots 41、copied bytes 0，唯一 full 在计时前。该无 GC 热循环的有限收益需与上述实际 GC 正确性覆盖一起理解；Linux 结果见[最终验收](ACCEPTANCE.md)。

### D2：搬迁后的并行引用回写

仓库外原型先比较 1／2／4／8 worker，确认整体收集收益后同步运行时规范 3.9，再接入现有 pool。新增独立 `update.c`：coordinator 串行更新 roots／remembered，定位当前副本并 claim 普通 scanned bitmap；稳定 live chunks 保存当前副本地址，普通对象按 64 个一批、数组按 1024 个元素分区。worker 只更新各自的槽，复用原 scan 与 forwarding 规则，完成屏障之后才验证／释放／reclaim／恢复。单 worker 直接串行，不建立 update 队列；没有新增线程池、公开 ABI、生产时钟或调参开关。

格式化、workspace clippy、31 项 collector 专测全部通过。新增 C 用例在 1／2／4／8 worker 下反复搬迁，覆盖 pinned 对象字段与 native root 槽别名、minor remembered 更新、嵌套值数组与完成后 shutdown；worker 创建失败用例增加了实际搬迁后的串行 update。runtime 的正式预编译资源已重新准备。新增真实 Scoop fixture 的 13 个变体通过，覆盖 debug／release、各 worker 数、moving／minor、空数组、大引用数组、pin、旧到年轻引用与 AtomicRef；现有四 mutator 原子并发和 aggregate／array／clone 写入各 2 个变体、12 个进程也在显式 4 worker 下通过。fixture 最初误用 GC-free pointee 才支持的 `withDataPointer`，改为既有 `pin`／`unpin` 后通过，未放宽语言规则。

性能使用 C 驱动真实 collector、宿主线程／VM 和合成 stack map，先构建正常堆并预热 pool，再计时一次完整 `gcCollect` 调用；计时包括返回 RUNNING 和返回调用者，不以阶段和代替。每例一轮预热、七轮交替独立进程；原型另测 claim／发布与 worker 扫描／等待，最终生产实现去掉诊断后重新与 C2 基线对照：

| 工作集，4 worker | C2 整体中位 [最小, 最大] ms | D2 整体中位 [最小, 最大] ms | update 中位 ms，C2 → D2 |
| --- | ---: | ---: | ---: |
| 16,384 节点＋共享环＋引用值数组，强制搬迁 | 3.164 [2.972, 3.349] | 2.560 [2.496, 2.742] | 1.064 → 0.517 |
| 262,144 节点＋共享环＋引用值数组，强制搬迁 | 47.779 [46.039, 49.087] | 38.543 [37.665, 39.308] | 16.234 → 7.366 |
| 1,048,576 个数组元素，保留 1/8 节点，普通稀疏 region full | 41.874 [41.052, 42.842] | 34.297 [33.875, 35.689] | 17.250 → 10.460 |

上述为首版 D2 的结果；收尾直接回调修正后的复测见[最终验收](ACCEPTANCE.md#32-搬迁后的引用回写)。整体耗时分别约减少 19.1%、19.3%、18.1%。单 worker 对应整体中位为 3.401→3.419、53.700→52.682、48.596→49.154 ms，区间重叠，没有测到明确退化或加速。两版大图峰值 RSS 中位均约 67.3 MiB，普通稀疏图约 123.9 MiB；每次检查全部存活节点的值、共享与链关系，checksum 一致。强制搬迁组包含 stress 自身的验证／quarantine 成本，普通稀疏组未启用 stress，不能把这些阶段成绩外推为全部应用吞吐。

原型的 2／4／8 worker 均有净收益；4 worker 大图的 claim／任务发布约 4.1 ms、扫描及等待约 3.1 ms，稀疏大数组约 6.5 ms 和 2.9 ms，串行准备已成为进一步扩展的明显限制。当前按已有 mark 策略选择 worker，保留默认小 minor／小堆串行；不为了局部扫描加速继续增加线程。Linux 最终性能与完整验收见 [ACCEPTANCE.md](ACCEPTANCE.md)；原始日志与实验副本仅在仓库外使用，已于收尾清理。

### D1：nursery 容量与 refill 取舍

完成 1／4／8／16 MiB × 1／2／4／8 mutator × 三种存活率的矩阵。真实 Scoop 程序共分配 600,000 个 32-byte 含引用节点，各 worker 保留最后一个、每 16 个保留一个链节点、或保留全部；末尾遍历验证节点数与值范围，分配值 checksum 为 `179999700000`。每种配置一轮预热、七轮交替独立进程，先完成所有构建再计时；统计开启、无 stress、不固定 affinity。只改 nursery 容量，32 KiB TLAB 与初始 16 MiB 整体阈值保持原值，后续整体阈值仍按原策略演进。

先在 C2 runtime 副本的 refill／退役／收集边界加入诊断，不在每次分配读取时钟，记录 refill 等待、退役未用字节、采样 committed 峰值和成功 coordinator 请求到恢复的总时长／最大值。对照发现诊断并非免费：1 MiB 的无插桩／插桩完成时间在单 mutator 低存活时为 12.088／12.884 ms，8 mutator 中存活时为 15.301／19.085 ms，不能用该矩阵宣称产品吞吐收益。于是以 D2 最终 runtime 重新生成全部四种**无额外插桩**的容量副本并重跑完整矩阵。下表为分配 kernel 加末尾显式 full GC 的完整样本中位 ms，避免将工作移到计时结束后；初始化、结果遍历和数字输出不计入：

| mutator | 保留模式 | 1 MiB | 4 MiB | 8 MiB | 16 MiB |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1 | 最后一个 | 12.167 | 12.464 | 12.664 | 13.303 |
| 1 | 每 16 个 | 15.405 | 15.677 | 16.390 | 21.550 |
| 1 | 全部 | 71.859 | 62.319 | 52.310 | 34.916 |
| 2 | 最后一个 | 9.118 | 11.171 | 12.737 | 12.300 |
| 2 | 每 16 个 | 12.090 | 14.432 | 16.311 | 20.949 |
| 2 | 全部 | 83.293 | 87.969 | 68.664 | 38.031 |
| 4 | 最后一个 | 8.638 | 10.804 | 11.801 | 11.547 |
| 4 | 每 16 个 | 12.354 | 14.400 | 16.433 | 19.792 |
| 4 | 全部 | 82.864 | 87.610 | 65.515 | 25.286 |
| 8 | 最后一个 | 12.217 | 11.397 | 11.700 | 11.964 |
| 8 | 每 16 个 | 16.149 | 16.014 | 16.538 | 19.925 |
| 8 | 全部 | 87.744 | 88.354 | 68.667 | 26.339 |

取舍：M56 保留 **1 MiB** 默认值。较大 nursery 对该全存活批次有收益，但未给短／中寿命输入带来稳定的全面改善，且停顿和内存代价明确。例如单 mutator 低存活从 1→4→16 MiB，最大停稳后 pause 的样本中位为 0.241→0.957→3.664 ms，峰值 RSS 为 13.5→17.1→32.7 MiB；8 mutator 中存活为 0.988→2.130→7.482 ms、16.2→23.2→41.5 MiB。高存活 16 MiB 组先碰到整体 full 阈值，均为 0 minor／2 full，复制字节为零；1 MiB 组有约 17–20 次 minor，并复制大量后来持续存活的对象。因此收益同时涉及收集种类与晋升，不能解释为纯 bump／refill 加速，也不足以为所有程序扩大默认容量。

诊断矩阵仅用来解释成本：8 mutator 低／中存活在 1 MiB 下退役未用空间约 2.3 MiB，4 MiB 约 0.6 MiB；同一业务总量被更多 STW 提前退休的 TLAB 分割。refill 锁等待是各线程累计量，不是墙钟关键路径占比；该样本没有足够证据要求批量领取 block，后者还会扩大尚未使用的容量占用，故不增加 refill 策略或自适应框架。请求到 RUNNING 的诊断口径覆盖成功 coordinator 的完整 collect，加入别人的请求不重复计数；另记录业务每 1024 次分配的最大进度间隔，不把它当作硬实时上界。Linux 代表点复核与策略结论见[最终验收](ACCEPTANCE.md#33-nursery-取舍)。

所有容量／诊断修改均在仓库外 runtime 副本，生产容量及 ABI 未改。此阶段另清理 834 MiB 的闲置 incremental 缓存，target 约 7.0 GiB。

### E：有条件优化的进入调查

先完成阶段测量，再决定是否扩大本期实现。编译实验为同一 D2 编译器的 C1 新管线／B3 原精确 release pass 串对照；输入包括 100／1000／5000 个公开小型 NoGC 算术函数，以及 100 个 class、1000 个字段读取函数。各做三轮交替冷构建，独立缓存／target，源码和工具始终存在；每次随后测真实缓存命中。临时编译器只记录阶段，并在外部 wrapper 中分流诊断，未改变 child protocol。以下为秒，中位数：

| 输入 | 原管线 CLI | C1 CLI [最小, 最大] | C1 backend 总计 | 声明 | 优化／RS4GC | machine emit | C1 CLI 峰值 RSS MiB | `.slib` MiB |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 个 NoGC 函数 | 1.104 | 1.109 [1.096, 1.118] | 0.149 | 0.014 | 0.008／0.001 | 0.094 | 461.7 | 1.97 |
| 1000 个 NoGC 函数 | 5.353 | 5.374 [5.303, 5.488] | 3.658 | 1.204 | 0.239／0.020 | 1.410 | 1263.5 | 19.47 |
| 5000 个 NoGC 函数 | 79.039 | 77.297 [74.990, 80.302] | 70.802 | 31.245 | 5.215／0.402 | 19.065 | 14171.3 | 97.71 |
| 100 class＋1000 个读取函数 | 8.679 | 8.725 [8.665, 8.754] | 5.890 | 1.444 | 0.294／0.148 | 2.447 | 1610.8 | 39.77 |

每项两管线的 `.slib` 大小相同，缓存命中中位约 4.8–6.4 ms。独立无插桩生产工具的 1000 函数冷构建三轮为 5.175 s [5.132, 5.885]，RSS 约 1288.6 MiB，与插桩组区间重叠；阶段数据用于定位，不能将微小时间差全部解释为管线收益。峰值为完整 CLI 进程树的系统测量，不能把全部内存归因于 LLVM 声明。C1 在这组无大循环的小函数输入上未测到明显编译成本／产物大小回退，实际循环运行收益另见 C1。

声明构建的增长已满足进一步优化的证据条件。建议后续先裁剪当前 callable 的 local／external callable 声明，保留 typed 目标、address-taken、foreign callback adapter、初始化及 metadata 使用；再按实际收益扩展 TD／global 的闭包，不能机械改为可缺项数组或改变逐 callable object 合同。这是独立的编译速度工作，不阻塞用户新指定的桌面 GC 主线；本期没有声称已完成声明裁剪或消除该内存峰值。

另外三组使用当前 release 工具、现有 sysroot、无临时编译器变换；一轮预热、七轮交替独立进程，先完成所有构建再计时，校验结果。只在循环外读取 native clock，`SCOOP_GC_STATS=1`，无 stress、不固定 affinity：

| 工作负载 | 对照 kernel 中位 [最小, 最大] ms | 总 allocated bytes／minor 次数 |
| --- | --- | --- |
| 一百万次 Long Pair 直接使用 | 1.025 [0.995, 1.034] | 192／0 |
| 同样 Pair 经局部 Any 装箱再立即拆箱 | 87.994 [87.430, 88.475] | 32,000,192／30 |
| 十万次同步 startCoroutine，无额外 suspend wrapper | 6.107 [6.028, 6.381] | 2,400,248／2 |
| 同上，1 层静态同步完成 wrapper | 12.361 [12.240, 12.461] | 13,600,248／13 |
| 同上，4 层静态同步完成 wrapper | 30.731 [30.527, 30.953] | 49,600,248／47 |
| 五百万次小整数变换，本 Cone 可内联 | 6.205 [6.199, 6.222] | 184／0 |
| 同一函数经普通 strong 跨 Cone 调用 | 12.443 [12.183, 13.572] | 184／0 |

这些是等价源码形态的机会对照，不是已实现的新优化成绩。box 用例证明局部可消除表示能同时减少分配和收集；不代表可删除 Any 身份、可变 boxed receiver 或跨挂起使用。协程组复用同一 Task／Completion，wrapper 都同步完成，新增分配包括现有 frame／adapter；没有做逐类别对象计数，也未据此声称真实挂起、异常、取消、Context 和跨线程恢复可以省略。跨 Cone 组只隔离普通小函数边界，不能代表复杂库调用；两程序 `__TEXT` segment 均为 1,064,960 bytes，文件为 17,301,464／17,302,552 bytes，provider `.slib` 为 79,522 bytes。协程 0／4 层的程序文件为 17,337,256／17,534,056 bytes，`__TEXT` 为 1,064,960／1,097,728 bytes；这些包含段对齐和 metadata，并非纯指令字节。

取舍：局部 box 和静态同步 wrapper 消除有明确分配收益，作为后续编译优化优先于通用逃逸分析；完整语义组合及候选实现对照尚未完成，不在本期加入新的协程／导入机制。跨 Cone 导入还需证明 private 引用、Context／初始化唯一性、ODR 选择、编译时间和实际增量体积，当前仅记录必要性，不修改产物格式。E 的范围是进入调查；A–D 仍是本期实现交付，未进入的候选明确保留在设计而不伪称完成。

阶段计时与旧 pass 开关涉及的四个 Rust 文件已按测量前备份逐字恢复，并确认与 HEAD 一致；生产 scoopc 二进制恢复。恢复后的格式检查和 workspace clippy 通过。原始日志、源程序、编译器副本及 runtime 原型均只在仓库外使用，已于收尾清理。

### 桌面短 STW 补充调研

按用户补充，M56 明确分配速度、暂停尾部和低配置机器的 footprint 优先于批量 GC 吞吐。代码核算发现 small block 常驻 side tables 为容量的约 31.4%，搬迁 forwarding 还会临时增加一份 block 大小；建议优先释放空闲 metadata，不能把虚拟 region 大小直接当 RSS。官方 Unity／.NET／LLVM 资料和局部候选的正确性边界已写入[补充设计](DESKTOP-GC.md)。

只在仓库外副本对普通年轻 small block 做整块退役，未改生产源码。真实 Scoop 一千万个短命对象的无插桩 kernel 在 0／16／128 MiB 无引用旧数组下分别从 174.952／177.848／192.305 ms 降到 115.298／119.941／134.324 ms，RSS 基本相同；独立诊断副本每格记录 1603 次 minor，保留 p50／p95／p99／最大值和 1 µs 时钟粒度，数据及限制见补充设计 5.1。显式全堆验证下，12 组引用／移动／pin／原子组合和六组多 mutator／存活率组合全部通过。此用例在 minor 点没有年轻存活图，原型还不是正式默认；后续先完成其跨平台与混合寿命验收，再考虑减少无引用大对象的 card 遍历。

### 最终组合回归与旧快照闭合

最终生产工具与重新准备的本机 sysroot 通过 M56 全部 10 个 fixture／48 个变体，以及 M26 String、M33 UTF-8 合计 15 个 fixture／18 个变体／56 个进程。Linux 已准备 GNU／musl 两套完整 16 Cone sysroot，进入完整 workspace 与正式 CLI 验收。

Linux workspace 首轮的 HIR lowering 有 1452 项通过、23 项失败；逐项比较确认全部仅多出 B3 的三个 String intrinsic 声明，被测源码的 HIR 没有变化。按仓库的结构测试纪律，将这 23 份早期快照缩到被测函数／类型声明、入口和泛型实例，保留完整正文、求值顺序、字段／variant 和类型关系；不再重复固定公共 core 的无关定义。没有改生产 dump 或编译行为，也没有直接将新增 core 行抄入全部期望。删去约 2500 行重复快照，沿用一个只服务现有测试的局部选择 helper。格式化与 workspace clippy 通过；本机 23 项受影响测试加 String intrinsic 签名拒绝专测全部通过。Linux 已仅补跑这些项及首轮尚未执行的 crate，不重复已通过的完整部分；该阶段覆盖 4975 项 Rust 测试、0 失败、0 忽略，50 项 fixture runner 规则全部通过；后续 ELF 新增专测使最终不同测试数增至 4976。driver 未完成部分改用保留 debug assertions／overflow checks 的优化 Rust 测试程序，具体范围见 [最终验收](ACCEPTANCE.md)。

Linux CLI 随后在 direct C aggregate／errno 的 NoGC 包装中暴露 C1 的选项遗漏：两处裸 `instcombine` 通过文本解析时默认开启固定点自检，LLVM 22.1.2 在一轮之后仍有可做的优化时会终止编译。上游明确将该文本默认值用于测试；普通 `InstCombineOptions` 默认不开启它。现将 NoGC 的两处调用与既有 Managed 管线一样显式设为 `instcombine<no-verify-fixpoint>`，保留正常有界变换、LLVM IR 验证和 site／root 检查，未改变语言／ABI 合同。[LLVM 22.1.2 文本管线解析](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.2/llvm/lib/Passes/PassBuilder.cpp#L1123)、[迭代与自检实现](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.2/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5552)。

该修正本机格式化／workspace clippy、28 项 statepoint 专测通过；重建三个 release 工具后，direct C values、errno values／artifacts、M56 loops／moving 合计五个 fixture／13 个变体／47 个进程通过。Linux 以独立工具输出目录通过同一 GNU 组合及 musl 的 direct C／loops／moving 三项组合，未替换仍在全量验收中的工具。E 的冷编译数字采于这项修正前，不将其当作修正后的精确编译耗时。

GNU 的 `m41-floating-ieee` 另暴露 ELF 后端跳转表遗漏：NoGC `when` 优化后生成 `.rodata`，其中四个 `R_X86_64_PC32` 引用函数内分支；旧代码只登记 `.data.rel.ro` 引用表，导致 orphan relocation。先补充实现规范 2.5，再用已有 AddressTakenConstant／StructuralPath 子键为非合并只读段登记独立范围；ODR 的 `.rodata.<primary symbol>` 同样归入 callable COMDAT。保留正常 switch 优化、实际重定位与指纹处理，不改 wire 格式或 runtime ABI。新增 GNU／musl 普通及 ODR 对象专测，原 IEEE fixture 增加泛型版本，覆盖 debug／release、normal／moving／minor。新增 fixture 分支首次使用了错误的泛型声明语序，按现有语法修正后通过；没有修改语言规则。

该修正本机格式化／workspace clippy、390 项 LIR 测试通过；codegen 原 233 项通过，新增对象专测修正 LLVM 可省略一个表项的断言并覆盖 ODR 段名后，10 项对象分区专测全部通过，合计 234 项不同测试闭合。三个 release 工具重新构建；IEEE、浮点跨 Cone 与 ODR 选择三个真实 CLI 用例通过。Linux 仅补验受影响对象与真实程序，不重复已通过的完整前端覆盖。

Linux 的扩大组合随后指出 `.rodata` 还会与已有边界的 CString 共享：直接登记整段会与它们重叠。后续修正只登记首个已有 atom 之前的后端前缀，空前缀不产生额外 atom；保留正常表内重定位，不将已有字符串或其边界合并进表。按 body 建立一次计划索引，避免每个对象重新遍历整个 Cone。专测补齐 GNU／musl、普通／ODR、仅表／表加字符串／仅字符串共 12 种机器输出组合，保留对齐空隙并检查重定位全部落在前缀内；本机格式化／workspace clippy 和 10 项对象分区测试通过。

GNU 全量另有五个 M54 压缩／HTTP 组合在链接时找不到 `ZSTD_trace_*`。核对固定的 zstd 1.5.7 源码后确认，上游仅在 ELF 自动启用这些未使用的可选弱 tracing hooks；不是压缩数据或 GC 运行错误。先补充压缩规范的构建条款，准备脚本为 zstd 显式传入 `ZSTD_TRACE=0`，沿用已有包配置和输入更新判断，未改变压缩算法、公开 API 或通用 native 弱引用解析。Ruff 0.16.10 格式化／检查通过；本机由实际准备函数重建私有 zstd 静态库，四个内存／同步流／异步流／DEFLATE 消息 fixture 的 8 个变体、24 个进程全部通过。Linux 的归档与真实消费者复验使用独立准备目录，不覆盖全量测试仍在使用的库。

桌面 footprint 另核查了实际 Mach-O：17,492,984 bytes 程序中 `__LINKEDIT` 为 13,697,024 bytes，76,560 个动态导出中约 55,000 个是 atom 边界。GNU 分配程序的实验副本经常规 strip 后，从 12,822,024 缩至 4,419,056 bytes，结果与 45 次 minor／复制量均一致；主要移除无 `SHF_ALLOC` 的静态符号信息。两项分别作为符号可见性与 release 输出的后续候选，未改现行链接／ODR 合同，也不把文件字节数当作 RSS；详细组成和限制见补充设计 3.2。

### D2 收尾：保留直接串行回调

Linux 正式测量发现首版 D2 的单 worker 大图整体为 73.648→80.007 ms；固定同一个物理核后仍可复现。单 worker 虽未启动 worker 队列，但引入了逐对象读取 visitor／context 的包装。仓库外原型恢复 coordinator 原有的 `relocate_live_object` 直接回调，由 update 入口在单 worker 时直接交给 retained-set 遍历；多 worker 继续使用原来的槽 visitor 与任务划分。只增加一个有实际调用的内部回调参数并删除原包装，不增加公开 ABI、线程或策略。

同机、先构建后交替七轮的三方对照中，大图单 worker 的原串行／首版 D2／直接回调为 70.124／75.003／71.493 ms，update 为 17.846／19.990／18.383 ms；普通稀疏图整体为 49.923／53.407／50.442 ms。原型显著减小了回退，但大图仍有约 2% 的剩余差异，不宣称完全消除。四 worker 大图和稀疏图仍有整轮收益；保留全部区间与异常样本，不将无源码变化阶段的差异都归因于回调。最终数据和针对性回归在验收文档闭合。

收尾改动通过 clang-format 与既有 C 编译警告检查；本机 31 项 collector 测试全部通过。为避免重建已清理的无关 Rust 缓存，用仓库外薄入口以 `rustc --test` 原样加载现有测试模块与平台 helper，保留全部 C 编译参数和测试断言。显式更新 runtime 资源后，分配与引用回写的两个正式 fixture／17 个变体也全部通过；未增加新的测试实现或永久入口。


### 最终闭合与清理

单 worker 收尾修正提交为 `ded61b00c`。Linux 原有 31 项 collector 测试与 GNU／musl 各 17 个正式 CLI 变体通过；随后重新生成受影响的 runtime benchmark 产物，并在全部构建结束后完成最终对照。GNU 全量及 musl 合同组合的去重结果、首轮超时原样补验、剩余性能差异与峰值内存见 [ACCEPTANCE.md](ACCEPTANCE.md)。

临时编译器源码和工具均已恢复，本机约 5.5 GiB 实验目录、远端约 135 GiB 工作目录及独立 worktree／分支／私有引用全部移除。收尾再清理约 6.9 GiB 本机 debug 缓存，保留最终 release 工具与 sysroot；既有 M47／M55 三份文件逐字未变。没有未闭合的实现或验收失败；分块 poll、年轻区整块退役等仍按设计作为后续候选。
