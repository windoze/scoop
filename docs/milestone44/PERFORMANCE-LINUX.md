# M44 Linux 性能记录

状态：整体优化对照完成；runtime 边界的 minor 更新回退尚未关闭，不能据此宣称 M44 完成。
Darwin 结果见 [PERFORMANCE](PERFORMANCE.md)，最终验收见 [ACCEPTANCE](ACCEPTANCE.md)。

## 来源与口径

主机 nuc12，Intel Core i7-12700H、14 核／20 线程、约 61 GiB，Linux x86_64 GNU，
LLVM 22.1.2。工具和程序为固定 release 副本，runtime C 为 O2、无 LTO。
[准备记录](validation/linux-preparation.json.gz)保存编译器、runtime 来源及机器信息。

- 整体对照：`44a3c0bf` 的未优化 runtime → `f300090b` 的生产 runtime；当前工具构建于
  `af49910f`，期间编译器行为没有变化。基线使用与其 runtime 源清单匹配的 M43 工具。
- 边界对照：`ed5624b4` 已完成 P0 引用更新优化、尚未拆分 runtime → 同一当前 runtime。
  不把 P0 的收益归给模块整理。
- 全部场景在同机交替运行，每配置每侧预热一次、正式七次进程；图每进程五轮，
  以下图阶段统计各含 35 次 collection。计时期间没有本任务的构建、测试或清理。
- wide／array／chain 为 131071 节点、4 marker；compact 为 180 万对 node／leaf、
  保留 1/16、4 marker；minor 为 4096 年轻节点和同数 leaf、1／4 marker。
- 测量使用既有测试 hook，生产分配／写入／poll 快路没有新增时钟。请求到 RUNNING
  指 T2−T0，尚不包括线程实际重新推进；进程耗时还包含构图和验证。阶段墙钟不能简单求和。

完整命令、运行顺序、binary／工具摘要、构建输入、预热和正式样本分别保存在
[整体对照](validation/performance-initial-gnu.json.gz)、
[边界对照](validation/performance-boundary-gnu.json.gz)。
两份压缩包是文件名到原始文件文本的 JSON 映射。
[工作量核对](validation/performance-audit-gnu.json.gz)逐对检查 15 个计数字段，
包含 live／traced／moved、映射量、mark 槽、晋升、脏卡、旧代槽及 source／target。
整体、边界和下述对齐实验共 18 组图对照全部保持对应轮次工作量一致。

## 整体优化对照

| 场景 | update 中位（ms） | 请求到 RUNNING 中位（ms） | 当前请求 p95／最大（ms） | 进程 CPU 中位（ms） |
| --- | --- | --- | --- | --- |
| wide | 5.707 → 0 | 8.456 → 2.658 | 2.990／2.991 | 102.204 → 70.623 |
| array | 10.600 → 0 | 13.879 → 3.177 | 3.684／4.424 | 139.910 → 84.430 |
| chain | 5.758 → 0 | 11.408 → 5.552 | 6.841／8.266 | 92.685 → 62.460 |
| compact full | 16.596 → 12.021 | 92.630 → 88.881 | 92.306／100.699 | 6908.378 → 4949.676 |
| minor，1 marker | 0.550 → 0.274 | 1.455 → 1.192 | 2.168／2.190 | 22.045 → 20.383 |
| minor，4 marker | 0.550 → 0.275 | 1.468 → 1.238 | 2.116／2.160 | 24.359 → 25.748 |

三个零搬迁场景的实际 moved 均为 0，当前 update 每轮均为 0。compact 每轮
225002 个存活对象，从 6 个 source region 集中到 1 个 target，搬迁 187720–187815 个；
每对轮次的实际数目相同。minor 每轮追踪 8192 个年轻对象，搬迁 7027 个，晋升
229376 bytes，扫描 65 张脏卡和 4096 个旧代槽，没有额外 full 或 promotion fallback。

各场景当前 RSS 中位依次为 15.21、16.26、15.02、164.39、7.44、7.46 MiB；
对应基线为 15.24、16.23、15.42、164.45、7.42、7.42 MiB。完整 RSS／映射量、
worker CPU、各阶段分布均在原始记录中。minor 的整体暂停尾部和四线程进程 CPU
没有随 update 中位一同改善，不以局部收益推导稳定尾延迟或所有程序的吞吐。

## 实际 mutator 进度与微基准

计算／分配各覆盖 1／4 个 mutator，共 56 个正式进程。每 1024 次真实迭代发布进度，
以 T1 前最后进度与 T2 后更大迭代数的时间记录间隔；该间隔包含采样批次和 OS 调度。
每线程计算一亿次或分配两百万次，collector 使用 4 marker。

| 场景／线程 | 实际进度间隔中位（µs） | 当前间隔 p95／最大（µs） | T2 到下一进度中位（µs） | 吞吐中位（百万迭代/s） |
| --- | --- | --- | --- | --- |
| 计算／1 | 31.38 → 29.82 | 68.01／263.05 | 2.69 → 2.58 | 468.97 → 481.45 |
| 计算／4 | 55.80 → 57.08 | 94.77／380.37 | 2.68 → 2.73 | 1571.02 → 1553.73 |
| 分配／1 | 605.97 → 633.17 | 696.22／809.25 | 21.14 → 21.32 | 24.55 → 25.64 |
| 分配／4 | 581.73 → 608.71 | 676.65／1644.02 | 18.35 → 18.23 | 47.83 → 47.25 |

单线程／四线程实际 runtime 分配字节分别为 48000104／192000176，两侧完全相同。
单线程两侧均为 45 次 minor／retry；四线程 minor 为 187–191／187–193，retry 为
191–193／187–197。retry 是次数，不是锁等待时间；这些小样本不支持稳定吞吐改善的结论。

四程序 allocation／arrays／old-graph／primitive 各自独立编译，两侧根 `.slib`
逐字节相同，函数集合、LIR、生成指令数与长度相同，最终 binary 因 runtime 不同而不同。
七次进程耗时中位分别为 13.208→11.378、19.676→18.623、17.569→18.042、
16.650→15.912 ms；范围重叠，不按这些短进程的单个中位数宣称稳定改善。

## runtime 边界回退与未采用的实验

独立边界对照保留全部 P0 优化，两侧零搬迁 update 均为 0。wide／array／chain
请求中位为 2.625→2.737、3.340→3.318、5.506→5.529 ms。
compact update 为 11.734→12.019 ms；配对差值中位 +324 µs，35 对中 28 对增加。
minor 的回退更稳定：

| 场景 | 拆分前 → 拆分后 update 中位（µs） | 配对差值中位（µs） | 增加的配对数 |
| --- | --- | --- | --- |
| minor，1 marker | 251.709 → 273.207 | +21.15 | 31／35 |
| minor，4 marker | 251.513 → 273.337 | +22.89 | 30／35 |

重复对照为 251.28→272.63 µs；将双方都固定在 CPU 4 的 P-core 后仍为
249.83→269.27 µs，各七次进程的中位数均表现为拆分后更慢，因此不能仅归为调度噪声。

[诊断与对齐实验](validation/performance-alignment-gnu.json.gz)包含实际 cache object
反汇编、relocation、符号地址、重复／固定 CPU 测量和完整六组场景。八个更新热函数
的指令字节在拆分前后相同；relocation 唯一差异是 scanner 的冷错误出口转至 binding
的 fatal。图的根 `.slib` 与 native helper object 也相同。callback 地址从
`0x879d0` 变为 `0x80b70`，说明布局变化值得检查，但不能据此声称某一 CPU 微架构
机制已被证明。

只在独立源码副本将 callback 对齐到 64 bytes，可使 minor 恢复到约 252 µs；
完整对照却使 compact update 从 11.672 增至 12.858 ms，因此拒绝合入。
另一份副本在遍历前选择 full／minor callback，以去掉逐槽模式判断；
[分流实验](validation/mode-experiment-gnu.json.gz)的机器码确认 helper 已内联，
但 minor 为 253→276 µs、compact 为 11.664→12.988 ms，同样拒绝合入。
两种实验均未改变生产 runtime；原始不利结果完整保留。后续修复必须同时核对
full／minor、零搬迁、分配和实际进度，不能只挑 minor 的有利样本。

## 内联 forwarding 的 GNU 验证

第三份隔离副本只将 `scoop_heap_forward_object` 的原函数体从 evacuation.c 移入
GC 私有 heap_internal.h，声明为普通 `static inline`；没有强制内联或对齐属性。
源位置解析、mark／pin／source 判断、自指／缺失 forwarding 及 collection 状态检查
全部保留，ABI 和收集策略不变。当前生产文件尚未采用该改动。

[候选原始记录](validation/performance-forward-gnu.json.gz)包含两处源码、构建来源、
实际机器码和全部六组对照。[逐轮核对](validation/forward-audit-gnu.json.gz)确认各侧
仍为七次进程、35 轮，15 个工作量字段相同；实际 O2／无 LTO 的槽访问和 claim 函数
已消除对 forwarding helper 的外部调用，原有失败出口保留。

| 场景 | 拆分前 → 候选 update 中位（ms） | 请求到 RUNNING 中位（ms） |
| --- | --- | --- |
| wide | 0 → 0 | 2.747 → 2.738 |
| array | 0 → 0 | 3.251 → 3.227 |
| chain | 0 → 0 | 5.496 → 5.517 |
| compact full | 11.672 → 10.904 | 88.699 → 88.253 |
| minor，1 marker | 0.251 → 0.251 | 1.163 → 1.172 |
| minor，4 marker | 0.251 → 0.250 | 1.178 → 1.177 |

full 的配对 update 差值中位为 −752 µs，35 对中 31 对未增加；minor 两组回到拆分前
范围。但 full 请求 p95／最大由 93.133／97.253 变为 97.834／100.979 ms，
单 marker minor 的最大 update 也由 0.297 变为 0.415 ms，不能宣称尾延迟改善。

实际进度间隔中位按计算 1／4 线程、分配 1／4 线程依次为
29.51→30.67、54.38→54.47、628.02→622.00、596.04→613.13 µs；
四组进程吞吐范围均重叠。allocation／arrays／old-graph／primitive 进程耗时中位为
14.065→12.033、19.167→18.847、18.992→17.950、16.460→17.952 ms，
同样均在重叠范围内；例如 primitive 两侧为 13.967–18.857／15.353–18.245 ms。

[Darwin 隔离验证](validation/forward-validation-darwin.json.gz)的 31 项 collector
测试和三个正式 M44 fixture 通过，并已构建本机图／进度／四程序对照 binary。
已有 Rust 测试驱动只通过临时 cc wrapper 将 runtime 源码和 include 路径转向副本，
没有修改断言；逐命令记录与源码核对随报告保留。相同方式运行的
[Linux codegen/runtime 全组](validation/forward-codegen-gnu.json.gz)387 项通过；
[GNU／musl 正式 fixture](validation/isolated-fixtures-linux.json.gz)也各为三项、
六个变体、24 个执行步骤通过。

这些结果不替代 GNU／musl 最终全部适用组合。统一内联在 Darwin 引入的数组 mark 回退
已通过平台层选择调用方式解决，见 [最终平台策略](PERFORMANCE.md#最终平台调用策略)。
生产代码共用一份函数体，x86-64 仍使用本节已验证的普通内联；重新构建后，GNU 全部
94 个 runtime 对象及完整图程序与本节候选逐字节相同，证据见[产物核对](validation/forward-platform-validation.json.gz)。因此保留并复用本节六组原始测量；最终功能覆盖仍待完成。
