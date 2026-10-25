# 已采纳优化与开启条件

本表记录本次[程序性能优化主线](PROGRESS.md)已采纳的方案，详细设计、验证、收益与成本见各项链接。后续采纳新方案或调整默认策略时同步更新。

“release 默认”指编译该 Cone 时使用 `scoop build --release` 或 `scoop build --profile release`，并满足表中的适用条件；独立编译器使用 `scoopc build --profile release`。CLI 默认 profile 为 debug。目前下列优化没有各自独立的公开开关或用户可设等级，编译器内部选项不等于 CLI 参数。

相关 API 调整：[String 的字符扫描名称](STRING-API.md)在全部 profile 生效，无开关；`charLength`／`getCharAt` 保持 O(n)，`byteLength` 保持 O(1)，内部存储与算法保持。

## 程序优化

| 已采纳方案 | 开启条件 | 适用范围与主要限制 |
| --- | --- | --- |
| [minor 脏卡组清单](DIRTY-CARD-GROUPS.md) | 所有 profile 默认，配套 runtime ABI 17，旧产物须重建 | 每 64 张卡共用 16 B 节点，首次写入登记，STW 只枚举／清零脏组；原有批量卡标记保留完整登记。side metadata 约 0.049%，写屏障固定成本与真实程序回退见记录。 |
| [小正文导出与跨 Cone 主值构造器内联](BODY-EXPORT.md) | release 默认；扩展成员正文需新版 provider，主值构造可消费既有角色 | 普通字段写入、值／构造表达式和 callable-owned 字符串沿用有界导出及统一成本选择；字符串引用 provider 原对象。secondary／class 初始化、缺少正文或成本不合格的调用保留，profile 间 ABI 一致。 |
| [循环内联优先级与传值调用估价](LOOP-INLINING.md) | release 默认 | 更深循环优先使用累计增长；及时消费已知 receiver，复用完整调用适配；循环执行上限计入参数／返回值快照，最高 72。总增长、六层展开、非循环规则和完整冷分支体积限制保持。 |
| [minor 局部块遍历与连续清卡跳过](MINOR-BLOCKS.md) | 新版 runtime，所有 profile 默认 | minor 的准备、汇总、搬迁选择和收尾只枚举年轻块／新目标；可用 region 直接索引，连续清卡由 `memchr` 跳过。对象／TLAB／ABI 不变；卡表自身的遍历与清零仍保留，尚无 survivor。 |
| [普通 poll 单次 phase 读取与有界叶函数省入口](ACTIVE-POLLS.md) | 新编译器生成的所有 profile 默认 | 普通 Managed poll 不读取 TLS／mode／epoch；既有有界叶函数省去入口 poll，效果仍为 Managed。root／初始化 gateway 保留首次激活，循环与慢边精确 roots 保持。 |
| [启动 site 的 owner 验证合并](STARTUP-OWNER-JOIN.md) | 新版 runtime，所有 profile 默认 | 每个 site 在首次 stackmap 合并时只解析一次 owner；重复 ODR 记录复用验证结果，仍比较完整地址与 payload，全部对应关系在 managed 执行前完成。 |
| [poll 的本线程 relaxed 读取](POLL-OWNER-READS.md) | Gateway 完整检查中，所有 profile 默认 | 全局 epoch／phase 保持 acquire，所属线程的 mode／observed epoch 使用 relaxed；普通 Full poll 已由上项进一步简化。runtime 跨线程发布与停稳协议保持。 |
| [普通内联增长额度调整](INLINE-GROWTH.md) | release 默认 | 初始额度为 `min(size / 2 + 128, 384)`，统一适用于本地、泛型及跨 Cone 正文；总上限、深度与跨轮累计限制保持，同步 suspend 初始常数仍为 96；执行估价的后续调整见循环内联项。 |
| [跨 Cone 普通方法与 getter 内联](MEMBER-INLINING.md) | release 默认；提供方需导出合格小正文 | 按完整签名与通用 HIR／MIR 成本选择，保留 receiver 与 typed accessor identity；泛型沿既有具体化路径，未知动态调用及缺少正文的旧库保留入口。 |
| [Scoop 符号在镜像内绑定](LOCAL-SYMBOLS.md) | Darwin 最终链接默认，所有 profile | 从动态导出移除 Scoop 持久符号，直接绑定 callable 与 metadata；已有相容 `.slib` 可重链接，普通 native C 导出保留。 |
| [完整父接口表内嵌与目录共享](INTERFACE-PROJECTION.md) | 所有 profile，统一布局／ABI | 单继承完整前缀、多继承固定偏移；目录复用内嵌片段，菱形保留完整父表。 |
| [父接口直接投影与继承方法直接取槽](INTERFACE-PROJECTION.md) | 所有 profile | 已有 Child 视图且静态知道父关系；泛型按 exact 实例处理。首次 Any 取表等动态路径另行查询。 |
| [smartcast 视图保留与块内 itab 复用](INTERFACE-PROJECTION.md) | release 默认 | 复用已取得的接口视图；动态表缓存仅保存 metadata，遵守局部值写入、取址与块边界的失效规则。 |
| [接口 SSA 快照的直线区间复用](INTERFACE-VALUES.md) | x86_64 release 默认 | 复用刚写入 canonical storage 的完整接口值及接口 niche enum；调用、poll、别名操作和控制流边界结束复用。ARM64 保持原重读路径。 |
| [接口检查与取表合并](INTERFACE-CHECKS.md) | release 默认 | 有非空接口视图消费者的 `is`／`as`／`as?`；普通控制流上复用成功检查，写入、非共同合流与异常入口使其失效。 |
| [循环中稳定来源的接口查询复用](INTERFACE-QUERY-REUSE.md) | release 默认 | 未取址、未写入的参数或所有 CFG 环外单次定义的局部引用；首次实际查询后复用 exact interface 的成功／失败 entry，含 Boolean 与空接口，不增加对象副本或 GC roots。 |
| [普通 class／局部游标字段替换](CLASS-SCALARS.md) | release 默认 | 创建与使用可分析、不逃逸且无 release hook；字段转为 typed locals，保留引用快照与 GC 语义。 |
| [字段替换指导内联](ITERATOR-INLINING.md) | release 默认 | 可替换 class 只剩一个 receiver 调用时共同估价；继续遵守原内联成本与累计增长限制。 |
| [调用点异常路径特化](EXCEPTION-PATHS.md) | release 默认 | exact receiver／closure 和已有完整正文证明异常边不可达后，按实际剩余正文选择内联；保留正常清理、真实异常、返回快照与精确 roots。未知效果和残余 EH 保留调用。 |
| [异常区域中的局部分配消除](EXCEPTION-SCALARS.md) | release 默认 | box／closure／无 release hook 的 class 在正常和异常路径均满足来源、支配与循环存活证明时替换为 typed locals；保留真实 catch／finally、返回快照及异常出口 roots。 |
| [有界整数／Boolean 数组初始化](ARRAY-INITIALIZATION.md) | release 默认 | initializer 展开后无调用／分配；64 次有界分块，合格的无 safepoint 内层交由 LLVM 向量化。 |
| [数组边界检查消除与分块](ARRAY-BOUNDS.md) | release 默认 | 稳定上界、非负起点、唯一加一更新；直接范围证明或长度 guard，保留 checked 回退与短尾。 |
| [循环范围指导数组访问内联](ARRAY-INLINING.md) | release 默认 | 用已证明的范围去掉候选正文中的不可达检查后计价；不提高全局阈值。 |
| [严格浮点数组循环](FLOAT-ARRAYS.md) | release 默认 | Float／Double 访问与逐元素运算复用有界路径；顺序归约保留累加顺序，禁止隐含重排与融合。 |
| [含条件分支的数组循环](CONDITIONAL-ARRAYS.md) | release 默认 | 单入口、单 latch、无环正文；合流保留共同事实，整数内层可向量化，严格浮点按实际合法性处理。 |
| [x86 最内层循环入口对齐](CONDITIONAL-ARRAYS.md) | x86 后端默认，无独立 profile 开关 | 向 LLVM 设置 32 字节优先对齐，实际应用由后端决定；其他架构不受此 x86 设置影响。 |
| [数组搜索与提前退出循环](EARLY-EXIT-ARRAYS.md) | release 默认 | 只复制继续迭代的区域，共享 return／break／失败出口；保留首次命中、异常前缀与本轮值。 |
| [引用数组范围证明与有界循环](REFERENCE-ARRAYS.md) | release 默认 | 单字引用、双字 interface、相应可空值与 MaybeUninit；保留写屏障和精确 roots，尚未开放引用 SIMD。 |
| [卡表置脏的幂等原子写入](CARD-STORES.md) | 所有 profile 与 runtime 默认 | 二值 card 使用 relaxed atomic store 1，省去原子读改写，清零仅在 STW；合格 release 循环可按卡标记合并规则处理。 |
| [nursery 独占块的分配登记](NURSERY-METADATA.md) | 使用新版 runtime，所有 profile 默认 | 已保留的年轻块范围直接登记，以原子 load／OR／store 保留已有位；共享旧代仍用原子读改写，保留完整清零与发布顺序。 |
| [已知分配布局特化](TLAB-COMPLETION.md) | release 默认 | 从完整本地 TD／typed array layout 复用对齐与 release policy，跨物理对象分区消除重复加载和分支；缺少事实时保留原路径。 |
| [小型 TLAB 分配完成内联](TLAB-COMPLETION.md) | release 默认，配套 runtime ABI 16 | 16～128 B 常量请求、完整布局、无 release hook；成功 bump 后内联清零、登记和计数，动态／更大请求沿共享完成入口。 |
| [分配 context 的首次使用复用](ALLOCATION-CONTEXT.md) | ARM64 release 默认 | 调用内首次需要 TLAB 时取址，跨循环／GC／catch 只复用稳定的 AS0 context 地址；可变字段继续重读。x86、debug 保留原路径，完成 helper 内部 TLS 访问不变。 |
| [确定初始化前的尾部清零消除](ALLOCATION-ZEROING.md) | release 默认；对齐后对象大小 ARM64 ≤128 B、x86 ≤64 B | 已知布局、无 release hook 的小型 TLAB 分配；沿确定初始化路径省去完全覆盖的无引用尾部零写入，保留对象头、引用槽、padding 及原发布顺序。 |
| [有界引用循环的页表查询提升](CARD-REGIONS.md)，含[接口栈重取扩展](INTERFACE-CARD-REGIONS.md) | release 默认；保留接口栈重取的扩展仅 ARM64；数组基址定位规则适用于所有 profile | 常数有界、目标不变且查询每轮必经，保留原 roots。ARM64 可保留已确认来自本函数栈槽的 volatile 引用重读，同时提升独立查询；x86 仍排除含 volatile 的区域。 |
| [有界引用循环的卡标记合并](CARD-BATCHES.md) | release 默认 | 无 safepoint、2～64 次、每轮必经且槽步长为 0／8／16 字节；退出后、下次 poll 前每流最多 3 次卡标记，保留全部元素写入和精确 roots。 |
| [有界引用读取的四倍部分展开](REFERENCE-UNROLL.md)，含[条件读取扩展](CONDITIONAL-REFERENCE-UNROLL.md) | release 默认 | 最多四个无环块、同一 latch、4～64 次且可被四整除、最多 24 条正文指令的只读内层；保留空值分支、浮点顺序、poll、短尾与精确 roots，不扩大引用 SIMD。 |

低成本且收益稳定的方案优先进入 release 默认设置；需要独立开关或等级的方案，在采纳时补充准确选项、默认值、等级含义和适用条件。所有 profile 继续遵守相同的语言、对象、GC 与产物 ABI 契约。PGO、更广范围的屏障合并和 bulk barrier 仍在后续工作中，不计入本表已采纳项。

[动态接口目录循环展开](INTERFACE-DIRECTORY.md)已作定向测量，因短目录与 smartcast 回退暂未采纳；当前仍使用原逐项查询。

## 测量与诊断

这些能力按需使用，不改变优化的开启状态。

诊断／测量插桩以显式生成开关为边界；关闭时不得进入最终程序，不能以运行时分支代替代码排除。

| 已交付能力 | 开启方式 | 作用 |
| --- | --- | --- |
| [启动阶段测量](STARTUP-STATS.md) | `scoop build/run/test/link --enable-startup-stats` 或 `scoop-link --enable-startup-stats`；所有 profile 默认不编入 | 七个启动阶段和已有索引计数；关闭时预处理排除计时、报告、诊断字符串与专用计数访问。切换只改变 runtime／最终链接，GC stress 可独立组合。 |
| [预构建程序 A/B 测量](../../tests/benchmarks/README.md) | 手动运行 `python3 tests/benchmarks/compare.py`；GC 诊断加 `--gc-stats` | 交错运行相同输入，核对输出并记录墙钟、CPU、RSS；GC 诊断与普通计时分别测量。 |
| [跨 Cone 普通成员专项负载](../../tests/benchmarks/inlining/members/README.md) | 按说明构建 provider／consumer；`SCOOP_BENCH_KIND` 取 0～5，`SCOOP_BENCH_COUNT` 指定次数 | 用户 struct／class 方法、getter、primitive hash、未知接口与标量对照，复用已有 phase 计时及 A/B 工具。 |
| [动态接口目录专项负载](INTERFACE-DIRECTORY.md) | 按 [interfaces 构建说明](../../tests/benchmarks/interfaces/README.md)手动构建；`SCOOP_BENCH_KIND` 取 9～17，`SCOOP_BENCH_COUNT` 指定次数，再用 `compare.py` 对照 | 区分真实装箱／集合目录、命中位置、泛型及短目录未命中、Boolean 和混合查询；保持原静态投影对照。 |
| [MIR 优化决策报告](OPTIMIZATION-REPORT.md) | `scoop build --release --emit optimizations --dump-dir DIR`；`--emit all` 也包含 | 输出实际内联／分配消除的成本与原因；未请求时不保存逐项事件。 |
| [函数级 CPU 采样归因](CPU-PROFILING.md) | 所有 profile 按需 `--emit symbols --dump-dir DIR`；`--emit all` 也包含；手动使用 sample／perf 和 `symbolize.py` | 关联当前及已加载依赖的真实正文、实例和声明；可用 `--binary` 解析实际别名及 sample 地址。不生成指令行表或 PGO 输入。 |
