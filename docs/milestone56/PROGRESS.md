# M56 实施记录

状态：实施中，2026-10-19 开始。范围与完成条件以 [DESIGN.md](DESIGN.md) 第 9、10 节为准；未完成项不得以设计或历史性能数据代替验收。

## 交付检查表

| 项目 | 状态 | 验证／取舍 |
| --- | --- | --- |
| A：当前 release 工具、开发 sysroot 与真实程序基线 | 本机基线就绪 | 完整库 sysroot 准备成功；分配与拆箱基线已建立，后续按项目追加 |
| B1：内部 TD 验证结果复用、native 边界保留 | 本机通过 | 30 项 collector 测试、8 个 CLI 变体及完整分配对照；Linux 最终复验 |
| B2：unbox／clone 单次对象定位 | 本机通过 | 单次 exact-size 定位；非法地址／类型、large clone、移动与含引用值 |
| B3：String 有界表示 intrinsic | 本机通过 | 三个 typed intrinsic 与独立内部 NoGC leaf；UTF-8、越界、旧 extern 与移动 GC |
| C1：NoGC／无 site 循环优化及 Managed 保守优化 | 本机通过 | 三目标向量代码与完整 codegen 专测；Managed poll／EH／原子保留，Linux 运行待复验 |
| C2：局部 card 查询／mark 复用 | 本机通过 | 同对象复用 region，精确槽重复 mark 消除；跨 card／调用／原子／GC 边界覆盖 |
| 分块 poll 候选调研 | 机制实验完成 | 设计 3.2 补充有界内层、CFG／roots 接入要求与真实移动握手对照；尚未启用自动变换 |
| D1：nursery／refill 对照与策略取舍 | 待完成 | 1／4／8／16 MiB，存活率与 1–8 mutator；吞吐／内存／停顿 |
| D2：有搬迁 GC 并行 update 候选 | 待完成 | 串行 claim、槽分区、完整阶段屏障；单／多 worker 同义性及净收益 |
| E：声明构建、局部 box／协程消除、跨 Cone 内联 | 待评估 | 逐项记录设计规定的进入条件与实测，不以无依据延期替代调查 |
| 本机阶段验证 | 待完成 | 每批先格式化／lint，相关 crate／runtime 与正式 CLI fixtures |
| Linux 完整验收 | 待完成 | nuc12 独立 worktree：workspace、fixture runner 规则和完整 CLI；GNU／musl 合同 |
| 逐功能提交、临时资料与 target 清理 | 进行中 | 只提交本任务变更，原始日志在仓库外，不使用绝对 `/tmp` |

## 工作环境与验证策略

本机 Apple M4 Pro／macOS 27.0.1，LLVM 22.1.8；Linux 从 `nuc12:~/repos/scoop` 新建独立 worktree，并为本任务使用独立 target、TMPDIR 和测试缓存，不修改其他 agent 的 checkout／构建输出。开始时远端 HEAD 为 `3cbc339f8`，完整验收前将本次提交导入独立 worktree。开始时本机 target 为 4.8 GiB、远端原 checkout target 为 1.6 GiB；按阶段清理无用构建产物，避免每个小改动反复全量构建。B1 验证后删除 2.6 GiB 的旧 incremental 缓存，本机 target 从约 6.6 GiB 降至 4.0 GiB；本任务设置 `CARGO_INCREMENTAL=0`。

仓库外工作目录为 `../scoop-work/m56-20261019`，包含 scratch、基线／候选测量与原始日志；命令显式设置 TMPDIR。性能样本先完成构建，再串行交替运行。正常应用使用显式准备的开发 sysroot，runtime 专测继续用已有源码构建测试入口。

Linux 独立 worktree 已创建为 `/home/chenxu/repos/scoop-m56-20261019`，分支 `codex/m56-validation-20261019`，起点为 B2 提交 `91df2bc22`；后续仅向该分支导入本任务提交。独立工作目录为 `/home/chenxu/repos/scoop-m56-work-20261019`，target／scratch／日志和测试输出均使用该目录，sysroot 属于新 worktree。未修改主 checkout 或其他 worktree。

已有的 M47 两份文档和 M55 设计不属于本次提交。初始内容摘要保存在仓库外，结束时核对；不使用全仓 reset／checkout，也不删除／隐藏源码或工具来验收缓存。

## 实施与验收记录

### B1：复用已登记的 TD

先补充运行时规范 2.2。`value_shape` 直接消费 generated-code、对象头和内部已登记 TD，保留动态 kind／size／count／alignment／overflow 检查；公开 native `scoop_rt_alloc` 在进入内部实现前检查登记。没有新增 unchecked 公共入口、凭证或第二套尺寸算法。C 回归新增合法 native 分配控制组及未登记、null、不可读 TD 的拒绝覆盖。

验证：C 使用 LLVM 22.1.8 clang-format（4 空格、100 列），Rust `cargo fmt --all` 与 `cargo clippy --workspace --all-targets` 通过；`scoop-codegen` 的 30 项 runtime collector 测试通过，包含 normal／moving 表示用例与 native 边界。fixture runner 的 50 项规则测试通过；新增 `m56-runtime-allocation`、`m56-runtime-representation` 共 8 个 debug／release／moving／minor 变体通过，无 golden。首次分配用例误用要求 Node.equals 的 Option 比较，改为已知长度遍历后通过，没有改语言规则迁就测试。

真实程序对照使用同一 release 编译器和 core，分别显式选择原始 runtime 副本与当前源码。循环分配两百万个 24-byte Node，写入 1024 项保留环并累加数值；末尾检查保留值和 checksum `1999999000000`。原生 monotonic clock 只在循环外取时，诊断 `SCOOP_GC_STATS=1`。先完成构建再串行测量，一轮预热、七轮交替独立进程，不固定 affinity：

| 版本 | kernel 中位 [最小, 最大] ms | ns／分配 |
| --- | ---: | ---: |
| `28fab1812` runtime | 71.363 [69.767, 75.752] | 35.682 |
| B1 | 43.332 [42.524, 43.915] | 21.666 |

该工作负载约减少 39.3% 时间；全部样本均为 45 次 minor／0 次 full、48,008,432 allocated bytes、1,114,160 promoted／copied bytes，dirty cards／root slots／traced objects 等工作量也一致。这是包括循环、引用写入与实际 GC 的 kernel 成绩，不是所有应用的加速比；Linux 尚待复验。

环境准备：release 工具、core、runtime 和第三方 native 后端已准备。完整 sysroot 首次在尚未准备好的 IDNA header 处停止；依赖已随后补齐，继续复用已有结果准备其余 Cone。原始构建、测量 JSON 与 fixture 工作目录均在仓库外。

### B2：拆箱与克隆复用单次对象定位

先补充运行时规范 2.3。两个表示入口在原有 heap lock 内调用一次 exact-size 查询，在读取对象头前确认对象起点，并用所得尺寸校验动态 shape；删除重复的 object-start 查询，没有增加新查询接口或跨 safepoint 缓存。保留类型／枚举父类型匹配、长度与实际分配尺寸检查。非法地址仍明确 abort，诊断沿用 exact-size 查询。

格式化、workspace clippy 和 descriptor-driven representation 专测通过；该专测在 normal／moving 两种模式覆盖内部地址、null、非对象地址、未匹配类型及动态 size。正式 CLI 的 3 个 fixture 共 12 个变体通过，新增整数枚举按原身份与父整数拆箱、错误转换异常、large array 克隆独立性；原含引用／ZST 用例继续通过。完整开发 sysroot 已准备成功。

同一 release 编译器／core 对照 B1 与 B2 runtime：一个 `Any` 中的 Pair 在循环内拆箱两百万次并累加两字段，checksum `30000000`；一轮预热、七轮交替独立进程，构建完成后串行测量。B1 中位 112.989 ms [112.512, 114.101]，B2 102.124 ms [101.818, 102.482]，约减少 9.6%。全部样本 allocated bytes 为 224，无 collection／复制／晋升；该结果包含类型测试、锁与循环成本，不代表所有 Any 使用方式。Linux 最终复验。

### B3：String 有界表示操作

先同步语言规范 11.4、运行时规范 2.4 和实现规范 2.10。core 的 byteLength、byte-at 和单标量 cursor 解码使用三个有完整签名的 typed intrinsic，HIR／MIR／LIR 沿已有分层传递，LLVM 保留 AS1。内部 NoGC leaf 使用独立的 `_leaf` 符号，复用原表示实现；原 Managed extern 符号及显式调用合同保持。索引检查继续保留，不扩大到扫描整串的 length、hash 或比较。

格式化与 workspace clippy 通过；相关 String 单元测试 24 项、intrinsic wire 测试 2 项通过，最终符号版本的 AS1／无 native transition 专测与 normal／moving C 表示专测也通过。C 专测首次缺少 String 实现的链接输入，补齐现有 strings／utf8 源文件后通过。release 工具和全部 16 个 Cone／runtime sysroot 已重建；新增 2 个 fixture 的 8 个 debug／release／moving／minor 变体，以及现有 m26-strings、m33-utf8 共计 17 个 fixture、26 个变体、64 个进程全部通过。覆盖旧 extern、跨 Cone、ASCII／NUL／多字节 UTF-8、非法索引、迭代、异常和移动 GC。

真实程序使用同一 320-byte／128-character String，对比 B2 与 B3 的 release 产物。一轮预热、七轮交替独立进程；三个 kernel 分别执行两百万次 byteLength、四千次 toByteArray、四千次字符遍历，checksum 均为 `19703556000`。统计开启，构建完成后串行计时：

| kernel | B2 中位 [最小, 最大] ms | B3 中位 [最小, 最大] ms |
| --- | ---: | ---: |
| byteLength | 33.725 [33.288, 34.080] | 5.405 [4.878, 5.500] |
| toByteArray | 21.103 [20.865, 21.536] | 1.987 [1.949, 2.061] |
| iterator | 20.679 [18.766, 21.611] | 3.132 [2.910, 3.182] |

三个 kernel 时间分别约减少 84.0%、90.6%、84.9%。所有样本均为 1 次 minor／0 次 full、1,638,496 allocated bytes、400 promoted／copied bytes、38 root slots、3 traced objects。收益来自这些频繁的小操作，不能外推为整个 String 库或应用的加速比；Linux 待复验。原始数据保存在仓库外。此阶段又清理 1.7 GiB 的旧 incremental 产物，target 约 7.0 GiB。

### C1：按 typed effect 选择普通优化管线

先同步实现规范 2.19。使用 LLVM 22.1 的逐函数 pass 入口：typed NoGc 且没有实际 site 的定义启用循环规范化、LICM、indvars、loop／SLP vectorization；其余定义加入 reassociate、关闭 PRE／load PRE／回边拆分的 GVN 和 BDCE。debug 保持原管线，Managed 每条回边 poll 不变，最终 roots 仍由原 finalization 定稿。没有全局替换 default O2、重新编号 site 或启用 fast-math。

格式化、workspace clippy、全部 227 项 codegen 测试通过，包含 runtime collector、invoke／unwind、实际 root count、机器 tail duplication、原子和握手。新增三目标专测确认 NoGc release 产生 `<4 x i32>`；debug 和无 site 的 Managed 控制组保持标量。严格浮点重结合负控制通过。新增 NoGC／Managed 循环 2 个 fixture 的 7 个变体通过，覆盖零迭代、非整向量尾部、整数回绕、重叠指针、嵌套 continue／break、try／catch／finally、原子发布与移动根；现有四 mutator 原子／GC fixture 的 2 个变体、12 个进程也通过。

同一 4096-element 数组分别进行 5000 轮转换，真实 Scoop NoGC 指针借用与正常 Managed 数组路径各自对照 B3；checksum `25188352`。一轮预热、七轮交替独立进程，保留统计和所有 poll：

| kernel | B3 中位 [最小, 最大] ms | C1 中位 [最小, 最大] ms |
| --- | ---: | ---: |
| NoGC 指针循环 | 5.366 [5.298, 5.557] | 1.104 [1.096, 1.190] |
| Managed 数组循环 | 88.396 [86.706, 92.611] | 87.437 [84.793, 113.719] |

NoGC kernel 时间约减少 79.4%；反汇编在对应 callable 确认两条 `mla.4s`，标量循环扩为带别名检查与尾部路径的向量循环（该函数约 14→40 条指令）。Managed 差异落在离散范围内，不能声称稳定加速；poll 仍主导此类短循环。两版可执行文件均为 17,492,984 bytes，`__TEXT` 均为 1,081,344 bytes，段对齐掩盖小函数的局部增长。全部样本无 GC；进程 allocated bytes 为 33,200，C1 单个耗时字符串更长的样本为 33,208，差异发生在 kernel 后的数字输出。当前应用构建约 3.43 s（与 fixture 构建并行，仅作观测，不能作为冷编译 A/B 结论）；编译开销的规模化对照与 E 合并记录，Linux 性能及完整验收仍待完成。

### C2：普通字段写入的局部屏障复用

先同步运行时规范 3.6、实现规范 2.5 与设计 5.3。只保留当前直线区间一个 LIR 对象的 region base／cards；同一对象跨 card 字段分别寻址和原子标记，只有精确相同槽重复写入才省 mark。任何 block 切换、local／raw 写入、调用（含 NoGc）、poll、原子访问或发布均丢弃状态。原数组／aggregate 范围与 AtomicRef 屏障继续独立，未引入 dirty 预读分支、跨 safepoint 缓存或对象 ABI 变化。单独模块保存该局部状态，复用现有扫描偏移和 card 算法。

格式化与 workspace clippy 通过；moving_gc 相关 16 项、原子 3 项通过，最终局部屏障 5 项专测再次通过。专测断言同槽、相邻槽与跨 card 的查询／mark 数、receiver 变化、NoGc／Managed 调用、原子和 block 边界。新增 2 个 fixture 的 8 个变体、现有 aggregate／array／clone 写入 2 个变体／12 个进程，以及四 mutator 原子并发 2 个变体／12 个进程全部通过；含正常、moving、minor、跨 card 大字段、成功／失败 CAS。

真实程序先将 Holder 提升为旧对象，再循环两百万轮，每轮写四个引用字段，末尾 checksum `10`。一轮预热、七轮交替独立进程，C1 中位 27.796 ms [25.905, 30.051]，C2 25.378 ms [24.917, 25.975]，约减少 8.7%。对应热 callable 的 acquire 指令静态数从 40 降为 16，构造函数从 20 降为 8；四个不同字段仍分别标记。所有样本均 allocated bytes 328、minor 0／full 1、root slots 41、copied bytes 0，唯一 full 在计时前。该无 GC 热循环的有限收益需与上述实际 GC 正确性覆盖一起理解；Linux 最终复验。
