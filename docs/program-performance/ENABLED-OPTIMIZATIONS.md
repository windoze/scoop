# 已采纳优化与开启条件

本表记录本次[程序性能优化主线](PROGRESS.md)已采纳的方案，详细设计、验证、收益与成本见各项链接。后续采纳新方案或调整默认策略时同步更新。

“release 默认”指编译该 Cone 时使用 `scoop build --release` 或 `scoop build --profile release`，并满足表中的适用条件；独立编译器使用 `scoopc build --profile release`。CLI 默认 profile 为 debug。目前下列优化没有各自独立的公开开关或用户可设等级，编译器内部选项不等于 CLI 参数。

## 程序优化

| 已采纳方案 | 开启条件 | 适用范围与主要限制 |
| --- | --- | --- |
| [完整父接口表内嵌与目录共享](INTERFACE-PROJECTION.md) | 所有 profile，统一布局／ABI | 单继承完整前缀、多继承固定偏移；目录复用内嵌片段，菱形保留完整父表。 |
| [父接口直接投影与继承方法直接取槽](INTERFACE-PROJECTION.md) | 所有 profile | 已有 Child 视图且静态知道父关系；泛型按 exact 实例处理。首次 Any 取表等动态路径另行查询。 |
| [smartcast 视图保留与块内 itab 复用](INTERFACE-PROJECTION.md) | release 默认 | 复用已取得的接口视图；动态表缓存仅保存 metadata，遵守局部值写入、取址与块边界的失效规则。 |
| [接口 SSA 快照的直线区间复用](INTERFACE-VALUES.md) | x86_64 release 默认 | 复用刚写入 canonical storage 的完整接口值及接口 niche enum；调用、poll、别名操作和控制流边界结束复用。ARM64 保持原重读路径。 |
| [接口检查与取表合并](INTERFACE-CHECKS.md) | release 默认 | 有非空接口视图消费者的 `is`／`as`／`as?`；普通控制流上复用成功检查，写入、非共同合流与异常入口使其失效。 |
| [普通 class／局部游标字段替换](CLASS-SCALARS.md) | release 默认 | 创建与使用可分析、不逃逸且无 release hook；字段转为 typed locals，保留引用快照与 GC 语义。 |
| [字段替换指导内联](ITERATOR-INLINING.md) | release 默认 | 可替换 class 只剩一个 receiver 调用时共同估价；继续遵守原内联成本与累计增长限制。 |
| [有界整数／Boolean 数组初始化](ARRAY-INITIALIZATION.md) | release 默认 | initializer 展开后无调用／分配；64 次有界分块，合格的无 safepoint 内层交由 LLVM 向量化。 |
| [数组边界检查消除与分块](ARRAY-BOUNDS.md) | release 默认 | 稳定上界、非负起点、唯一加一更新；直接范围证明或长度 guard，保留 checked 回退与短尾。 |
| [循环范围指导数组访问内联](ARRAY-INLINING.md) | release 默认 | 用已证明的范围去掉候选正文中的不可达检查后计价；不提高全局阈值。 |
| [严格浮点数组循环](FLOAT-ARRAYS.md) | release 默认 | Float／Double 访问与逐元素运算复用有界路径；顺序归约保留累加顺序，禁止隐含重排与融合。 |
| [含条件分支的数组循环](CONDITIONAL-ARRAYS.md) | release 默认 | 单入口、单 latch、无环正文；合流保留共同事实，整数内层可向量化，严格浮点按实际合法性处理。 |
| [x86 最内层循环入口对齐](CONDITIONAL-ARRAYS.md) | x86 后端默认，无独立 profile 开关 | 向 LLVM 设置 32 字节优先对齐，实际应用由后端决定；其他架构不受此 x86 设置影响。 |
| [数组搜索与提前退出循环](EARLY-EXIT-ARRAYS.md) | release 默认 | 只复制继续迭代的区域，共享 return／break／失败出口；保留首次命中、异常前缀与本轮值。 |
| [引用数组范围证明与有界循环](REFERENCE-ARRAYS.md) | release 默认 | 单字引用、双字 interface、相应可空值与 MaybeUninit；保留写屏障和精确 roots，尚未开放引用 SIMD。 |
| [卡表置脏的幂等原子写入](CARD-STORES.md) | 所有 profile 与 runtime 默认 | 二值 card 使用 relaxed atomic store 1，省去原子读改写，清零仅在 STW；合格 release 循环可按卡标记合并规则处理。 |
| [有界引用循环的页表查询提升](CARD-REGIONS.md)，含[接口栈重取扩展](INTERFACE-CARD-REGIONS.md) | release 默认；保留接口栈重取的扩展仅 ARM64；数组基址定位规则适用于所有 profile | 常数有界、目标不变且查询每轮必经，保留原 roots。ARM64 可保留已确认来自本函数栈槽的 volatile 引用重读，同时提升独立查询；x86 仍排除含 volatile 的区域。 |
| [有界引用循环的卡标记合并](CARD-BATCHES.md) | release 默认 | 无 safepoint、2～64 次、每轮必经且槽步长为 0／8／16 字节；退出后、下次 poll 前每流最多 3 次卡标记，保留全部元素写入和精确 roots。 |

低成本且收益稳定的方案优先进入 release 默认设置；需要独立开关或等级的方案，在采纳时补充准确选项、默认值、等级含义和适用条件。所有 profile 继续遵守相同的语言、对象、GC 与产物 ABI 契约。PGO、更广范围的屏障合并和 bulk barrier 仍在后续工作中，不计入本表已采纳项。

## 测量与诊断

这些能力按需使用，不改变优化的开启状态。

| 已交付能力 | 开启方式 | 作用 |
| --- | --- | --- |
| [预构建程序 A/B 测量](../../tests/benchmarks/README.md) | 手动运行 `python3 tests/benchmarks/compare.py`；GC 诊断加 `--gc-stats` | 交错运行相同输入，核对输出并记录墙钟、CPU、RSS；GC 诊断与普通计时分别测量。 |
| [MIR 优化决策报告](OPTIMIZATION-REPORT.md) | `scoop build --release --emit optimizations --dump-dir DIR`；`--emit all` 也包含 | 输出实际内联／分配消除的成本与原因；未请求时不保存逐项事件。 |
| [函数级 CPU 采样归因](CPU-PROFILING.md) | 所有 profile 按需 `--emit symbols --dump-dir DIR`；`--emit all` 也包含；手动使用 sample／perf 和 `symbolize.py` | 关联当前及已加载依赖的真实正文、实例和声明；可用 `--binary` 解析实际别名及 sample 地址。不生成指令行表或 PGO 输入。 |
