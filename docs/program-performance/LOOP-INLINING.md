# 循环内联优先级与传值调用估价

当前的收益、热度、累计增长和正文尺寸选择由[调用点内联评分](CALL-SITE-INLINING.md)接续；下面保留本项交付时的设计与测量。

相对 `e3a88dd6c`，普通 MIR 内联优先把累计增长额度用于更深的循环，并及时使用展开后暴露的 exact receiver；循环中的执行成本上限同时考虑参数和返回值在调用两侧的值快照。统一适用于本地、具体化泛型和已导入的跨 Cone 正文，release 默认启用，没有按类型或函数名称设置例外，也没有增加诊断代码或用户开关。

## 选择规则

复用现有 CFG 支配分析计算自然循环深度。同一 header 的多个 latch 合并计一层，EH 的分析节点不成为实际输出块；不可归约环至少取得循环优先级。同深度按原 block／statement 顺序选择，再按原调用位置展开，求值顺序不变。初始 caller 增长仍为 `min(size / 2 + 128, 384)`，跨轮累计扣减，不返还化简后的额度，保留六层展开限制。

选择器重新分析当前 caller 时，直接复用普通去虚拟化的完整签名与 dispatch 规则。已知 receiver 可以立即参与本次内联选择，不再等下一轮才利用刚展开的事实。入选项保存完整调用及其 receiver／unbox／实参适配；未入选的调用沿原路径处理。已有调用图和展开祖先继续限制递归，不改变未知动态目标的分派。

循环执行上限为 `min(12 + 4 × boundary + opportunities, 72)`，其中 `boundary = 6 + 2 × (全部参数复制权重 + 返回值复制权重)`；沿用现有复制权重：普通 struct／tuple／enum 为 3，interface 为 1，intrinsic Float／Double／Char 等标量为 0。已证明的常量化简和 exact receiver 机会仍分别提供原有的 12／8 权重。这是选择启发式，不代表实际机器周期或固定发生的复制次数。极小正文、非循环规则和完整代码增长计价保持；同步 suspend 内联共用执行估价，仍保留初始常数 96 和原调用点次序。

冷 throw／trap 分支原本已有较低执行权重，本次没有将其重新算作新增优化；它们仍完整计入体积。MIR／LIR 格式、接口表、跨 Cone、`.slib`、调用 ABI、精确 GC roots 与严格浮点语义不变。此次对照直接消费相同的预编译系统库。

## 实际调用变化

- 用户 values 中的 `Complex.times` 原为 execution 51、limit 40，额度尚足；现按相同正文成本取得 limit 72，主循环调用消失。
- 用户 dispatch 中的 `ListIterator.next` 原在下一轮取得直接目标时只剩 20 额度，无法承担 growth 61；现及时取得目标，入选时剩 104，execution 67 小于 limit 68。`ArrayList.get` 随后以 execution 19、growth 13 入选，迭代器分配在既有字段替换中消失。
- 三目标 MIR 和实际机器码均确认上述变化。未知 `Shape.area` 保留接口调用；`ArrayList.checkIndex` 和底层 `MutableArray.get` 仍保留。初始化分支中的部分 `MutableArray.set` 留作调用，有限额度优先用于反复遍历。
- strmap 的主要库正文缺口和 GC 成本仍在，不把这次内联选择当作该负载的完整优化。

## 定向验证

两机格式化／workspace Clippy 通过，各通过 69 项 MIR 优化与 4 项同步 suspend 测试，覆盖多 latch／EH 循环深度、优先级、复合参数／返回值、递归、累计增长、冷分支、快照、装箱及闭包消除。相关中间轮次断言改为验证新的同轮展开合同，保留未知调用和递归边。

Darwin／GNU／musl 通过 `ordinary-member-inlining`、`inlining-growth`、`inlining-semantics`、`small-values`，共 24 个 variant、66 个进程。增长专测增加多分支初始化后的接口集合遍历和双字段浮点值运算，同时断言热调用消除与未知分派保留。成员专测覆盖反向 provider／consumer profile、泛型、装箱与 moving／minor GC；未运行完整 fixture 验收。

## 性能与成本

使用用户 `/tmp/scoop-bench` 的原始 dispatch、values、strmap 源码，输入副本及日志保留在仓库忽略的 `tmp/program-performance/loop-inlining/` 和 `priority-review/sources/`；Linux 原始记录在 `nuc12` 的同名目录。旧／新编译器消费同一套当前 sysroot，不混入前一项 GC 改动。运行测量为每个程序一次预热、三对交错独立进程，关闭 GC stress／诊断，逐次核对输出；表中为整进程墙钟中位数。

| 负载 | Darwin：旧 → 新 | GNU：旧 → 新 | musl：旧 → 新 |
| --- | --- | --- | --- |
| dispatch | 1.836543 → 1.601506 s（−12.80%） | 1.596038 → 1.118603 s（−29.91%） | 1.602399 → 1.221995 s（−23.74%） |
| values | 0.186716 → 0.123362 s（−33.93%） | 0.154999 → 0.139183 s（−10.20%） | 0.173568 → 0.153619 s（−11.49%） |
| strmap | 3.126972 → 3.174506 s（+1.52%） | 4.190937 → 4.034872 s（−3.72%） | 6.546625 → 6.548757 s（+0.03%） |

dispatch／values 每对样本均改善。strmap 的 Darwin 三对中两对改善、一对回退，musl 每对仅回退约 0.03%～0.33%；保留这些结果，不将其称为稳定收益。

| 目标 | 负载 | 代码段变化 | 相关静态数据变化 | 完整文件变化 |
| --- | --- | ---: | ---: | ---: |
| Darwin | dispatch | −540 B | −312 B | −560 B |
| Darwin | values | −8 B | −248 B | −288 B |
| Darwin | strmap | −44 B | +40 B | 0 B |
| GNU | dispatch | −512 B | −320 B | −600 B |
| GNU | values | −64 B | −280 B | −296 B |
| GNU | strmap | −64 B | +24 B | 0 B |
| musl | dispatch | −512 B | −336 B | −600 B |
| musl | values | 0 B | −280 B | −304 B |
| musl | strmap | −64 B | +40 B | 0 B |

静态数据按相关节汇总：Darwin 为两个 `__const` 加 `__llvm_stackmaps`，GNU 为 `.data.rel.ro.scoop`，musl 为 `.scoop_metadata`；不是仅统计接口表。GNU 的动态重定位节另外减少 48／24／0 B。对象布局与精确根规划契约保持，内联产生的正文及 site 变化均包含在上表中。

编译成本直接运行 `scoopc build`，使用相同输入和 std／core，排除 dump 与最终链接；一对预热后取三对交错中位数。墙钟变化如下，编译峰值 RSS 变化范围为 −0.15%～+0.86%。

| 负载 | Darwin | GNU | musl |
| --- | ---: | ---: | ---: |
| dispatch | +0.37% | +0.57% | +0.21% |
| values | +0.04% | −0.12% | +1.70% |
| strmap | +0.40% | +0.12% | −1.49% |

两项主要热调用负载持续获益，本组样本的代码与文件未增长，编译成本很小，因此沿用 release 默认能力。内部小成员正文导出、minor 脏区／survivor、callable 与 stackmap 的可达性裁剪继续独立推进。
