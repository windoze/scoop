# 可选 survivor 阶段

基线为 `7efeeb5b6`，已经包含 nursery 自适应容量和分块搬迁计划。本项针对跨过一次 minor、随后很快死亡的对象，减少它们提前进入旧代所产生的内存和 full collection 成本。

## 开启条件与布局

普通 debug／release 默认关闭；`SCOOP_GC_SURVIVOR=1` 开启一个 survivor 阶段，`0` 关闭。它与 `SCOOP_GC_NURSERY_MAX_MIB` 独立，moving／minor stress 和 full-only 保留原有收集策略。该选项控制实际 GC 策略；测量沿用现有 GC 统计，分配与屏障的生成规则保持，不增加诊断代码或 GC roots。

- 每轮选择时，survivor 总 block 容量以当时 nursery 的四分之一为上限，并计入下轮占用；nursery 仍按原有规则自适应调整。先在原 source 选择遍历中保留首次存活、无 pin 且至少半满的块；剩余配额可承接稀疏块中的存活对象，配额不足时直接晋升。
- 下一次 minor 中，仍至少半满的 survivor 原地晋升，稀疏块沿原搬迁路径集中到旧代。含 pin 的年轻块原地晋升；large／release-hook 对象仍预晋升。full 晋升全部年轻存活对象。
- 代龄与本轮保留状态只占 block 原有 padding。survivor 不接受新分配，也不进入旧代 free-run 清单；对象、TLAB、region generated-code prefix、runtime ABI 17、metadata ABI 9 和 `.slib` 合同保持。
- 原地保留和目标预留均先于复制及 forwarding 发布。预留失败撤销本轮目标和保留标志，仍从完整原图转 full；对象计数扣除上轮 survivor，晋升量只计实际进入旧代的字节。容量自适应继续使用全部年轻存活量。

原有“minor 后全部年轻对象都已晋升”的清卡假设已解除。借当前 remembered set 和年轻存活对象回写，只保留实际 old→survivor 边所需的卡，包括本轮晋升对象的边；STW 内暂记为 2，收尾恢复为 1 并重建已有卡组清单。mutator 和生成代码仍只写 1，mutator 运行期间卡值只有 0／1，不增加全旧堆扫描。

没有搬迁时仍跳过 roots 更新；存在原地 survivor 时继续完成必要的 heap 边扫描。没有 survivor 的零搬迁路径保持原有跳过行为。普通引用回写与需要保留卡的回写共用同一内联实现，在 collector 选择入口，关闭时不为每个引用读取 survivor 开关。

## 测量方法

三个目标分别以相同 release 工具、源码和预编译库对照旧 runtime、新 runtime 默认关闭及显式开启。每组一次预热、三对 AB／BA 进程运行；另以已有 GC 统计运行一对，核对已知输出。它们是独立进程的描述统计，包含启动与退出，不当作应用内尾延迟或统计显著性结论。

原始 trees／strings／strmap 使用用户 `/tmp/scoop-bench` 的仓库内副本；[有限缓存负载](../../tests/benchmarks/survivor-cache.scoop) 保留 8192 个最近创建的记录并替换八百万次，用来观察中等寿命对象。长期存活树与频繁更新字符串映射分别保留为对照，不能把减少晋升直接等同于缩短总耗时。

初版对全部 survivor 进行复制，在有限缓存中减少内存但增加耗时；最终改为密集块原地保留。初版产物、探索样本和最终 `v2` 样本分别保存在两机的 `tmp/program-performance/survivor/`，不把初次加载的探索计时作为最终收益。

## 运行效果与默认策略

下表为旧 runtime → 开启 survivor 的耗时中位数，单位为秒；括号为候选相对基线的变化，负值表示耗时减少。

| 负载 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | --- | --- | --- |
| trees | 5.53398 → 5.68226（+2.68%） | 7.18460 → 7.39676（+2.95%） | 8.63008 → 8.40799（-2.57%） |
| strings | 0.30445 → 0.31807（+4.48%） | 0.29456 → 0.32161（+9.18%） | 0.71556 → 0.73993（+3.41%） |
| strmap | 2.86793 → 3.08330（+7.51%） | 3.23941 → 3.20965（-0.92%） | 5.11785 → 5.22196（+2.03%） |
| 有限缓存 | 0.08995 → 0.07965（-11.45%） | 0.12926 → 0.10338（-20.02%） | 0.18280 → 0.11572（-36.69%） |

对应峰值 RSS 中位数如下，单位 MiB：

| 负载 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | --- | --- | --- |
| trees | 474.12 → 392.61 | 455.07 → 355.50 | 453.90 → 350.04 |
| strings | 44.52 → 42.00 | 41.50 → 38.92 | 34.72 → 34.21 |
| strmap | 35.12 → 34.77 | 33.48 → 32.47 | 30.25 → 29.79 |
| 有限缓存 | 34.84 → 13.16 | 32.46 → 17.75 | 30.21 → 17.54 |

有限缓存的晋升量在三个目标均由 32,505,880 B 降到 263,192 B，复制量由 32,243,736 B 降到 146,432 B；minor 从 123 次增加到 143 次，full 从 3 次降到 1 次。最大单次暂停在这次独立 GC 样本中分别由 1.815／3.691／3.917 ms 降到 0.294／0.395／0.505 ms。少量更频繁的 minor 换来了更少的复制、晋升及 full。

trees 的晋升量由约 1.242 GB 降到 1.102 GB，复制量由约 1.259 GB 降到 0.886 GB，峰值 RSS 降低 17.19%／21.88%／22.88%；但 minor 从 297 次增加到 317 次，full 均为 12 次，Darwin／GNU 的总耗时仍有小幅回退。strings 的 full 次数和晋升量基本未降，额外一轮 minor 增加了扫描成本；strmap 的总耗时变化也不稳定。

**因此默认保持关闭**，为中等寿命对象或需要降低内存占用的负载提供显式选项。已默认交付的 nursery 自适应容量继续生效；survivor 不代替容量调整。

默认关闭的对照也单独运行；下表是新 runtime 相对旧 runtime 的耗时变化：

| 负载 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | --- | --- | --- |
| trees | +0.85% | +1.66% | -0.52% |
| strings | +0.55% | +2.14% | +0.53% |
| strmap | -0.79% | -1.38% | +1.63% |
| 有限缓存 | +1.17% | -4.32% | +1.02% |

这些样本范围为 −4.32%～+2.14%，不据此声明关闭时有收益；四负载在三个目标的分配字节、年轻分配字节、minor／full 次数和晋升量均与旧 runtime 相同。本机普通引用回写指令在归一化地址后与基线一致，没有 survivor 卡保留调用。

## 空间、构建与正确性

每个 block 仍为 144 B，每个 region 仍为 72 B；block 的 state／starts／forwarding 偏移仍为 20／32／88。堆私有状态仅增加 8 B：Darwin 为 832 → 840 B，GNU／musl 为 808 → 816 B。用户对象、每线程 TLAB 与 GC roots 均没有新增字段。

三个主负载每个目标的 consumer `.slib` 均逐字节相同，静态类型／stackmap 区域增量为零。text 增量分别为 Darwin 1,256 B、GNU 1,648 B、musl 1,536～1,600 B；完整文件增量分别为 112～128 B、56～4,144 B、72～4,176 B，页对齐造成部分文件跨过一个 4 KiB 边界。

编译器和库正文未变，编译成本对照只测本次改变的 runtime 冷构建，每个目标三对交替运行：

| 目标 | 构建墙钟（秒） | 变化 | 峰值 RSS 变化 |
| --- | --- | --- | --- |
| Darwin ARM64 | 4.622 → 4.569 | -1.15% | -0.07% |
| Linux GNU | 5.062 → 5.095 | +0.65% | +0.08% |
| Linux musl | 5.528 → 5.497 | -0.56% | -0.46% |

格式化与 `scoop-codegen --all-targets` Clippy 通过。定向 native GC 覆盖单／四 worker、无新写入的跨轮 remembered 边、零复制的密集块、晋升对象的新引用、混合代龄及容量上限、pin／full、对象计数、双 mutator 同卡写入、部分预留失败回滚和无 survivor 的零搬迁回归；Linux 另复验 full 容量不足的回退。

Darwin／GNU／musl 各通过 2 个正式 fixture、5 个变体、20 个进程：`nursery-block-reuse` 的 debug／release 与 minor stress，以及 `reference-array-roots` 的 moving／minor stress 和新增普通 survivor 运行。覆盖继承接口视图、装箱、引用／optional／String／closure 数组、MaybeUninit、native callback 与并发线程的精确根。四种预编译 runtime 配置均已更新，没有执行全量验收。

初次 musl 整体 sysroot 准备在无关 libuv 输入处因所选工具链缺少 `linux/errqueue.h` 终止；本项改用既有 musl wrapper 明确构建四种 runtime 并复用所需库，随后 consumer 构建、上述 GC 验证与性能对照全部完成，未修改无关 native 库。

功能提交后合并 `origin/main` 的 `b11f790e7`，runtime 与 codegen 无差异；合并点的共享默认值身份、派生比较来源及 Unit 语法／编码共 10 项前端检查通过。Unit 编码用例在默认 Rust 测试线程栈下溢出，使用 `RUST_MIN_STACK=16777216` 后通过，保留这一测试环境限制；未改生产配置或扩大为上游完整验收。
