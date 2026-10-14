# M44 性能记录

本文件只记录实际完成的测量；其余设计场景和三目标验证继续见 [PROGRESS](PROGRESS.md)。

## 当前图基线

源码基线 `44a3c0bf`，编译器为 M43 完成后的固定 release 工具（摘要在报告中），
runtime 仅增加线程测试观测点，尚未优化 relocation。主机 Apple M4 Pro、64 GiB，
macOS 27.0.1／AArch64，LLVM 22.1.8、Apple clang 21.0.0；生成程序为 release。
固定 4 个 mark worker，普通 GC 策略与默认 block／nursery 配置，关闭 moving／minor
stress 和 full-only。两次对照必须使用同一输入和观测配置。

使用现有 wide／array／chain 程序，每图 131071 个节点、每进程主动 full 五次，
预热一次后测七次进程，共每图 35 次有效 collection。计时期间无本任务的构建、
测试或清理；[原始报告](validation/graphs-baseline-darwin.json.gz)包含命令、工具摘要、
全部预热和正式样本、阶段时间、worker CPU、RSS／映射量及进程吞吐。

| 图 | 搬迁数范围 | update 中位／p95／最大（ms） | 请求到 RUNNING 中位／p95／最大（ms） | 进程耗时中位（ms） |
| --- | --- | --- | --- | --- |
| wide | 0 | 3.442／3.815／3.938 | 5.311／5.927／5.938 | 51.148 |
| array | 0 | 6.607／7.095／7.100 | 9.207／9.933／9.942 | 73.343 |
| chain | 0 | 3.289／3.513／3.528 | 7.040／7.281／7.301 | 56.885 |

这里的零搬迁由实际 moved_objects 证明。尚未测量 candidate，不能将阶段中位数相减
作为优化收益。35 次 collection 的 p95 只描述此批样本，不能证明稳定尾延迟。
三个发布时间点的区间不包括随后广播／解锁／调度，不代表 mutator 实际恢复延迟；
测试 hook 和时钟读取有成本。阶段墙钟可能重叠，worker CPU 单独记录。

复现方法见 [基准说明](../../tests/benchmarks/m44/README.md)。固定 binary／runtime
保存于 target/m44/graphs-baseline-darwin，后续与 candidate 同机交替测量。

## 零搬迁 update 分支：Darwin 同机交替对照

完成本批编译与测试后，对上述固定 baseline 和仅修改 collector 零搬迁分支的
candidate 交替运行；每图每个 binary 预热一次、正式七次进程、每次五轮主动 full。
源码 commit 加构建时差异、全部运行顺序和原始数据见
[对照报告](validation/graphs-no-move-darwin.json.gz)。这是重新交替测得的 baseline，
不与上一节分开运行的数据计算加速比。

| 图 | update 中位：基线 → candidate（ms） | 请求到 RUNNING 中位：基线 → candidate（ms） | candidate p95／最大（ms） | 进程耗时中位：基线 → candidate（ms） |
| --- | --- | --- | --- | --- |
| wide | 3.660 → 0 | 5.605 → 1.878 | 1.972／1.990 | 53.242 → 34.245 |
| array | 6.913 → 0 | 9.431 → 2.504 | 2.580／2.667 | 73.664 → 39.137 |
| chain | 3.563 → 0 | 7.547 → 3.903 | 4.010／4.027 | 60.239 → 41.811 |

两侧各 105 轮有效 full 的搬迁数均为零；candidate 的 update 时间每次均为零，
与源码中根／remembered／retained-set relocation 整体不执行相符。每次 checksum
和 collection 记录检查通过。每图七次进程耗时范围分别为：wide 基线 51.609–54.482 ms、
candidate 33.846–34.806 ms；array 基线 73.175–74.217 ms、candidate 38.506–39.254 ms；
chain 基线 59.537–61.024 ms、candidate 41.730–42.391 ms。本批差异超过这些运行间波动。

这只证明当前主机三类无搬迁 full 图的收益，不代表有搬迁、线程实际恢复、Linux 或
代码生成模块整理的结果。minor／回退／pin／release 的正确性由本批独立 runtime
测试及正式 CLI 组合验证，性能尚须按设计补齐。

## 引用位置复用、条件写回与 GC-free 扫描省略

使用新增 compact 场景，先分配 180 万对含引用 node／GC-free leaf，再保留 1/16 并
连接为环。两侧都包含上一批零搬迁优化、相同观测代码和源码输入；candidate 增加
位置复用、条件写回、GC-free 扫描省略及无效自指 forwarding 的直接检查。工具、
宿主和 4 worker 配置沿用上文。全部构建／测试结束后交替运行，预热各一次、正式
各七次进程，每进程五次增长／收缩，共每侧 35 个有效收缩 full。

[最终原始对照](validation/relocation-compact-darwin.json.gz)保存源码版本／差异、
binary／工具摘要、命令和全部样本。逐对逐轮核对 moved、source、target、live、traced
数完全一致：每次从 6 个 source region 集中到 1 个 target，225002 个存活对象，
实际搬迁 187720–187815 个对象。没有开启 moving stress。

| 指标 | 基线中位／p95／最大 | candidate 中位／p95／最大 |
| --- | --- | --- |
| update（ms） | 14.082／15.065／15.344 | 11.388／11.585／11.787 |
| 请求到 RUNNING（ms） | 56.155／62.952／64.424 | 53.894／57.936／58.629 |
| 进程耗时（s） | 2.901／2.958／2.958 | 2.659／2.693／2.693 |

update 中位约减少 19.1%，请求到 RUNNING 中位约减少 4.0%；七次进程耗时范围分别为
2.859–2.958 s 与 2.628–2.693 s。这是该具体工作负载的整体对照，不把三项代码改动的
收益分别归因；进程还包含构图、自动 minor、初始 full 和校验，不能把总时间变化全归
到已单列的五次 full。worker CPU、映射量、RSS 和吞吐均保留在原始报告中，阶段墙钟
可能重叠，p95 仍仅描述小样本。

另用完全相同的两个 binary 对 wide／array／chain 各交替七次进程，每次五轮 full，
恢复 131071 个节点的原输入。两侧各 105 轮实际搬迁和 update 均为零，三个请求到
RUNNING 中位分别为 1.931→1.923、2.355→2.339、4.128→4.089 ms，未见整体回退；
这些微小差异不作为零搬迁分支的新增收益。见[完整复测](validation/relocation-no-move-darwin.json.gz)。

[最终 binary 的反汇编](validation/relocation-machine-code.txt.gz)结合真实 fixture
验证定位复用、条件 store 和 GC-free 提前返回。此结果仍限 Darwin／AArch64；年轻图、
线程恢复延迟和 Linux 对照待后续批次，不能用当前结果替代。

## 编译器 GC 边界整理：生成代码与正常路径

本批只整理 codegen 职责。两侧使用同一当前 runtime（含前两批 update 优化）与
sysroot 副本，固定整理前和整理后的 release CLI，宿主、LLVM 和 4 worker 配置沿用
上文。复用 allocation、arrays、old-graph、primitive，覆盖分配、数组引用写入、字段
写入及纯计算循环 poll。编译／测试结束后，各预热一次，再按程序交替七次进程。
[原始对照](validation/codegen-comparison-darwin.json.gz)保存全部命令、源码／工具
摘要、checksum、墙钟／CPU、GC 统计、映射量／RSS、吞吐和运行顺序。

| 程序 | 基线进程中位／p95／最大（ms） | 整理后进程中位／p95／最大（ms） | 根 Cone 机器指令字节数（两侧相同） |
| --- | --- | --- | --- |
| allocation | 9.519／14.777／14.777 | 9.755／13.126／13.126 | 1408 |
| arrays | 15.006／20.699／20.699 | 15.236／18.476／18.476 | 8636 |
| old-graph | 14.317／19.962／19.962 | 14.019／17.732／17.732 | 6392 |
| primitive | 15.823／22.405／22.405 | 15.828／19.199／19.199 | 624 |

七次进程范围分别为：allocation 基线 8.921–14.777、整理后 9.179–13.126 ms；
arrays 14.659–20.699、14.360–18.476 ms；old-graph 13.871–19.962、13.723–17.732 ms；
primitive 15.720–22.405、15.515–19.199 ms。小程序含进程启动，单个 outlier 对 p95
影响很大；这些差异不构成加速或回退的证据，也不解释为单条 store／poll 成本。

[结构记录](validation/codegen-machine-darwin.json.gz)确认两侧分别以不同 scoopc
摘要和编译缓存键实际编译根程序，未误用同一缓存产物。四组 LIR、根产物、完整 binary
及全部根 Cone 函数机器指令均逐字节一致；包含生成代码的 core 也分别重新编译。
因此这些程序没有因边界整理增加热路调用、TLS 查询或机器指令。LLVM 条件 poll、
屏障及原子内存序的精确约束由本批 codegen 测试覆盖；正式 CLI 另验证构造／copy／
clone／原子引用与 GC 的组合。本节限 Darwin／AArch64，不替代后续 Linux 结果。

## 完整 runtime 边界整理

本节对照已包含 P0 优化的固定基线与完成根、扫描、mutator 和普通 runtime 操作边界的
版本，不把早先 relocation 优化的收益算入模块整理。宿主、4 worker、release profile
及各程序输入保持一致；先完成构建／回归，再各预热一次并交替运行七次进程。
四个小程序使用此前 codegen 整理后的固定 binary 作基线；图使用 compact-final-darwin
的固定 binary。全部源码版本／差异、工具摘要、命令、binary 摘要和原始结果随报告保存。

初轮引用数组暴露回退：mark 中位 1.422→1.646 ms，请求到 RUNNING 2.341→2.566 ms，
见[首轮零搬迁报告](validation/boundary-zero-initial-darwin.json.gz)。这组差异在进程间
持续存在，因此没有按噪声忽略。[实际 runtime object 反汇编](validation/boundary-marker-machine.json.gz)
显示封装改变了 marker 的内联与指令布局。仅把 null 判断移回调用方的
[中间对照](validation/boundary-guard-zero-darwin.json.gz)仍未消除回退；最终让数组视图
只保存 count／elements，并在原有分段任务发布位置读取 stride，恢复原来的热路径结构。
没有改变扫描内容、1024 元素分段、GC 参数或增加编译器专用的强制内联属性。
[专项复验](validation/boundary-array-check-darwin.json.gz)后，再对最终版本执行完整的下列对照。

| 程序 | 基线进程中位／p95／最大（ms） | 最终边界版进程中位／p95／最大（ms） |
| --- | --- | --- |
| allocation | 9.644／12.632／12.632 | 9.540／12.032／12.032 |
| arrays | 14.919／17.440／17.440 | 15.336／17.549／17.549 |
| old-graph | 14.168／17.564／17.564 | 14.146／15.621／15.621 |
| primitive | 15.580／19.254／19.254 | 15.698／17.619／17.619 |

[四程序原始记录](validation/boundary-programs-darwin.json.gz)包含 CPU、吞吐、GC 次数、
映射量和 RSS；最小值分别为基线 9.078／14.601／13.832／15.442 ms、最终版
8.995／14.647／13.798／15.520 ms。各组范围相交，这些含启动成本的短进程不支持
边界整理带来加速的结论，也不能把小幅中位差异解释为单次分配或屏障成本。

| 图 | 基线请求到 RUNNING 中位／p95／最大（ms） | 最终版中位／p95／最大（ms） | 基线／最终版 update 中位（ms） |
| --- | --- | --- | --- |
| wide | 1.916／1.971／1.985 | 1.931／2.015／2.063 | 0／0 |
| array | 2.321／2.394／2.402 | 2.332／2.506／2.531 | 0／0 |
| chain | 4.105／4.191／4.205 | 4.136／4.297／4.298 | 0／0 |
| compact | 54.432／59.506／59.861 | 54.378／58.781／59.220 | 11.457／11.381 |

[最终零搬迁记录](validation/boundary-zero-darwin.json.gz)的每图每侧 35 轮均为
moved_objects=0、update_ns=0。array 的 mark 中位恢复为 1.401／1.408 ms，最小值
1.360／1.353 ms；请求到 RUNNING 范围为 2.240–2.402／2.257–2.531 ms，没有首轮
持续分离的分布。[最终普通搬迁记录](validation/boundary-compact-darwin.json.gz)每侧
35 轮均实际搬迁，每对轮次的 moved／live／traced／source／target 计数相同，仍为
6 个 source、1 个 target、225002 个 live 对象；update 中位及整体区间未见明显回退。
阶段 p95 来自 35 次收集，进程 p95 来自七次运行；均为小样本描述，不证明稳定尾延迟。

这些观测仍只覆盖 T0／T1／T2 与进程吞吐，没有测到各 mutator 实际恢复推进的时间。
minor 年轻图／old→young／pin、多线程分配和实际恢复、Linux 对照及最终正式全量验收
尚待完成，本节不能单独作为 M44 完成依据。
