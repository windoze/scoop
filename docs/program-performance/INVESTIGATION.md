# Scoop 程序运行性能：优化方向与实施路线调研

调研日期：2026-10-23。源码基线：`eee1cbbf7add9f415413fc47b46ea6eb85777968`，包含 M62。目标是生成程序的性能；编译速度只作为优化的工程成本记录，相关专项另见 [编译器性能](../COMPILER-PERFORMANCE.md)。

2026-10-24 修订：将测量与归因基础设施前置为独立交付；PGO 保留为后续独立专项，按实际优化消费者与收益安排，不与全程序优化／ThinLTO 绑定排期。另以源码及本机 release 产物核对父接口查表和 smart cast 成功路径，见 §5.2.1；该检查不包含性能计时。

本文对照语言、运行时、实现三份规范，核查实际优化代码、M31／M34／M56／M58／M62 的设计和测量，并查阅 Go、OpenJDK、Kotlin/Native、LLVM 的一手资料。本文没有新运行四种语言的对比基准；引用的 Scoop 时间都是既有实验，不能据此判定当前已经达到对手。下述方案为后续实施建议，不改变当前规范、公开 ABI 或默认构建配置。

## 1. 结论与目标

**建议把“达到或超过 Go／Java／Kotlin/Native”作为一条完整的程序性能主线推进。** Scoop 的值类型、泛型单态化、原生代码和静态依赖图提供了竞争基础，但还需要把这些信息贯穿到内联、内存访问、GC 和库实现。仅继续调整内联阈值、添加几个 LLVM pass，无法覆盖现有的主要差距。

推荐的核心组合是：

1. 更完整的跨函数／跨 Cone 内联、效果分析、去虚化和特化，使高层抽象能消失。
2. class／iterator 标量替换、跨函数逃逸分析和部分逃逸分析，使常用代码少产生真实对象。
3. 托管数组的边界检查消除、有界循环分块和 SIMD，使普通安全代码也能生成高质量内存循环。
4. 更便宜的分配、写屏障和 GC 元数据访问，加上寿命驱动的晋升与并发 major，兼顾吞吐和暂停。
5. 协程 frame／adapter、聚合值 ABI、集合和文本算法优化，补齐真实应用中的主要成本。
6. 全程序优化／ThinLTO、代码布局和产物裁剪，争取跨抽象层的剩余收益；PGO 在具备明确消费者和代表负载后独立推进。

**先交付可复现测量、热点归因与优化决策诊断，再持续用它们指导上述优化。** 这套能力服务编译器、runtime 和库开发，也服务未来 PGO，但不以 PGO 为验收前提。开发者根据实测改进通用静态优化，与编译器读取特定程序的 profile 来选择该次构建的优化，是两种独立的使用方式。普通 release 构建的性能应始终单独验收。

这些优化存在协同，也存在竞争。分配消除可能减少 survivor 的收益；更激进内联可能增加指令缓存压力；压缩引用可能提高缓存命中率却增加地址解码；并发 GC 会占用业务 CPU。因此，“全部考虑并完成必要实现”应落成上述能力体系和经过验证的策略选择，不能等同于将所有策略同时默认开启。

**本轮优化是一个长期、分批试验和交付的过程。** 成本低且收益比较稳定的方案，在验证后可直接纳入 release 默认设置；成本过高或收益不稳定的方案，通过独立开关或优化等级决定是否启用、启用哪一级。具体默认值随测量结果逐步调整，配置交付原则见 §11.1。

沿用 [M56 的桌面用途](../milestone56/DESKTOP-GC.md)：低分配成本、短暂停、低配置机器和实际内存占用仍是主要约束；同时增加数值计算、文本处理和并发服务负载，检验综合竞争力。是否领先必须分别比较稳定吞吐、应用尾延迟、内存和启动，不能用一个混合总分掩盖短板。

## 2. 当前基础及真正的缺口

| 层次 | 已有能力 | 本次核实的剩余问题 |
| --- | --- | --- |
| 语言表示 | struct／enum／tuple 按值，泛型单态化，引用／指针 Option niche，值元素数组连续存储 | 对象头 16 bytes；interface 16 bytes；含引用 enum 变体使用独占槽；这些表示有真实内存成本 |
| MIR 优化 | 去虚化、内联、局部 box／closure 消除交替执行，最多三轮；intrinsic 另有必要归一化 | 不是通用跨过程分析；分配消除只登记 Box／ClosureAlloc；带 EH 的函数被整体排除；没有 class 字段替换和冷路径物化 |
| 跨 Cone 优化 | 泛型具体化；M62 发布并消费部分普通顶层小函数的可移植 HIR 正文 | 普通正文导入范围受限，不等于任意成员、构造器、循环、Context、EH、suspend 都能跨库展开 |
| LLVM | SROA、范围约束、SCCP、GVN、memcpyopt、DSE 等；NoGc 有循环和向量优化；有界 Managed 内层支持整数／Boolean 数组 | 数组初始化及单位步长 checked 访问已打通；一般 CFG／循环变换、浮点、引用元素和未展开调用仍有限制 |
| GC 集成 | AS1 non-integral pointer，精确根，RS4GC，条件 poll，只在慢路径物化 poll roots | site 与逻辑恢复标记较早限制变换；机器 tail duplication 关闭；这限制了默认 O2/O3 能直接复用的范围 |
| 物理产物 | 逐 callable LLVM module／对象，Strong／ODR 选择，已有私有 closure 支持裁剪 | LLVM 通常看不到其他 callable 的定义；尚无全程序优化、PGO 或 ThinLTO 路线 |
| 调用 | 小于等于 16 bytes 的 GC-free 聚合值可按目标直接传递；接口拆成 object／itab | 24-byte V3 仍间接传递；含引用聚合值仍间接；真实动态函数视图还可能经过 Any bridge |
| 协程 | 无 site 的同步路径、状态机前同步内联、尾部 completion 转交 | 真挂起路径和未能消除的同步完成路径仍有 frame／adapter；跨库、EH 和更一般的生命周期尚未优化 |
| 分配与收集 | TLAB，1 MiB nursery，首次存活晋升，可扩展 region，STW 并行 mark/update，整块年轻区退役 | bump 后仍调用完成 helper；plan/copy/reclaim 仍串行；没有 survivor、并发 major；屏障、side metadata 和扫描局部性仍有成本 |
| 标准库 | 连续值容器、String 字节读取／clone 的部分 IR 快路、预编译系统库 | HashMap 逐槽线性探测；String 字符长度／随机字符读取扫描 UTF-8；集合访问、扩容、回调和迭代器仍可继续优化 |

主要代码证据：

- [MIR 调度](../../compiler/mir-lower/src/optimization.rs)、[分配候选与 EH 限制](../../compiler/mir-lower/src/optimization/allocation.rs)、[内联限制](../../compiler/mir-lower/src/optimization/inline/policy.rs)。
- [可移植普通正文范围](../../compiler/hir/src/inline_body.rs)、[普通顶层函数导入](../../compiler/hir-lower/src/imported_generics/inline.rs)。
- [LLVM 管线](../../compiler/codegen/src/statepoint/policy.rs)、[Managed 有界 SIMD 资格](../../compiler/codegen/src/statepoint/bounded_loops.rs)、[逐 callable 发射选择](../../compiler/codegen/src/emission.rs)。
- [Scoop ABI 分类](../../compiler/codegen/abi-plan/src/scoop.rs)、[typed closure 与动态 bridge 的不同调用路径](../../compiler/lir-lower/src/function/call/dispatch.rs)。普通 typed closure 调用已有 typed invoke，不能将全部函数值调用都描述为 Any 装箱。
- [GC 各阶段](../../runtime/src/gc/collector.c)、[block 元数据](../../runtime/src/gc/heap_internal.h)、[分配完成](../../runtime/src/gc/allocation.c)、[批量 mark 队列](../../runtime/src/gc/mark_queue.c)。
- [HashStorage](../../sysroot/src/scoop.std/src/collections/hash_storage.scoop)、[ArrayList](../../sysroot/src/scoop.core/src/array_list.scoop)、[UTF-8 访问](../../runtime/src/strings.c)。

既有数据说明优化空间很大，但也说明不能只看分配数：

- M62 的同机 V3 对照为原版本 1.309 s、自动内联 0.170 s、手工展开 0.170 s、禁止浮点融合的 C 0.134 s。主要损失曾来自未能内联；修复后仍有可定位的差距。这不是跨语言综合分数。[M62 验收](../milestone62/ACCEPTANCE.md)
- M58 的局部 box／scope closure 样本分别从约 32／40 MB 分配降至 280 bytes；但某个 each 版本进一步消除 closure 后，耗时较前一版本增加约 15.6%。必须同时查看机器码、运行时间、内存和暂停。[M58 性能第 13 节](../milestone58/PERFORMANCE.md#13-局部装箱作用域闭包与嵌套遍历)
- M62 的 16 MiB nursery 在高存活多线程样本中提高吞吐，却完全改变 minor/full 路径，并恶化部分短帧延迟；4 MiB 某样本出现约 382 MiB RSS 峰值。简单扩大 nursery 不能代替晋升策略设计。[M62 候选评估](../milestone62/EVALUATION.md)

## 3. 对手已经具备什么

比较必须锁定版本、配置和目标平台。按本次读到的官方资料，当前基准至少应覆盖 Go 1.27、JDK 25 LTS 与 JDK 27，以及当时稳定的 Kotlin/Native 2.4 系列；具体 patch 版本在实际测量时固定。实验特性另列，不能记入默认配置。

| 对手 | 需要正面应对的能力 | 对 Scoop 的启示 |
| --- | --- | --- |
| Go | 逃逸分析与栈分配、低成本并发运行时、PGO；Go 1.24 引入 Swiss Table，1.26 默认 Green Tea，1.27 增加按大小特化的分配入口 [O1]、[O2]、[O3]、[O4] | 原生 AOT 本身不是优势终点；分配、哈希表和 GC 缓存局部性需要一起优化 |
| Java／HotSpot | 分层 JIT、运行时类型反馈、内联／去虚化、逃逸分析与标量替换；成熟 GC、压缩引用、紧凑字符串及对象头 | 需要 AOT 特化和 PGO 对应其动态信息优势；在对象图负载上不能假定 Java 一定更费内存 |
| Kotlin/Native | LLVM 后端、自定义分页分配器；2.4.0 起 CMS 为默认选择，当前文档描述非分代并发 mark/sweep | 不能继续把它当成旧内存模型或纯 STW 对手；Scoop 的值表示优势必须落实为更少分配和更好的循环代码 |

几个容易误读的事实：

- Go PGO 文档给出的约 2%～14% 是 **Go 1.22 的代表程序结果**，不是 Scoop 的预期加速。Green Tea 的 10%～40% 是 GC 开销下降预期，不能写成整程序提速；Go 1.27 小分配入口“最高约 30%”同样不等于应用提升 30%。[O1]、[O2]、[O3]
- Go 1.27 的可移植 SIMD 包仍标为实验特性。它可以作为调优对照，默认组不能暗中使用。[O3]
- HotSpot 的常规逃逸优化主要消除可标量替换的分配，不能笼统写成“Java 把所有不逃逸对象搬到栈上”。Graal 的部分逃逸分析则支持只在逃逸路径分配，两种实现要区分。[O5]、[O6]
- JDK 25 将 compact headers 转为正式特性，JDK 27 才默认启用；JDK 26 的 G1 改进减少了写屏障同步。ZGC 使用不同的引用和屏障方案，不能把 G1／压缩引用／ZGC 的最佳指标拼成一个不存在的 Java 配置。[O7]、[O8]、[O9]
- Kotlin/Native 使用 tracing GC；与 Objective-C ARC 互操作不意味着其自身采用 ARC。其 release binary cache、Latin-1 字符串等选项的默认与实验状态也应分别记录。[O10]、[O11]

Java 稳态峰值需要充分预热后比较；启动和短命命令行程序则从进程创建开始独立比较。Scoop 可以争取同时保持启动和 footprint 优势，并在稳定负载上通过 AOT＋PGO 获得接近动态编译的代码质量；变化很大的运行时类型分布仍是 JIT 的强项。

从机制上判断，Scoop 最有希望率先取得优势的是连续值数据、单态化数值／集合和短命工具；最需要投入的是高阶业务代码中的剩余分配、密集对象图的内存效率，以及长期运行应用的 full GC 尾部。这是选题判断，尚不是已经测得的语言排名。

## 4. 历史暂缓项重新归类

以 [M31 后续清单](../milestone31/DESIGN.md#9-明确留待后续)、[M56 扩展](../milestone56/DESIGN.md)、[M58 设计](../milestone58/DESIGN.md)及 [M62 评估](../milestone62/EVALUATION.md)为来源，按当前代码更新状态：

| 历史方向 | 当前状态 | 本轮结论 |
| --- | --- | --- |
| 自动内联、去虚化 | 已有本地／泛型及有限跨库实现 | 扩展到 EH、成员、构造器及全程序／profile 类型反馈 |
| managed allocation 消除 | 已有局部 box／closure、class／游标字段替换，以及最后一个 receiver 调用的联合内联估价 | 继续处理未命中的创建／调用、containment、跨调用和部分逃逸 |
| coroutine frame elision | 已有同步正文内联和尾转交 | 继续处理实际挂起协议及未消除的立即完成路径 |
| bounds-check elimination | 已有路径约束与单位步长数组循环证明，动态长度关系使用 guard／原 checked 路径 | 继续处理未展开调用、复杂条件和更广索引关系 |
| 激进 unroll／SIMD | NoGc 已有；Managed 已支持有界整数 SSA、标量数组初始化与合格 checked 访问 | 继续扩展一般内存循环、严格浮点和目标特化 |
| 跨 callable module 重组、LTO／ThinLTO | 尚未建立 | 单独设计全程序优化和后端物理产物生成顺序 |
| 测量与归因基础设施 | 已有专项 benchmark、GC 统计、可复现对照及按需 MIR 决策报告；CPU／调用栈归因仍需补足 | 提前独立交付，先支持瓶颈定位、回退解释和通用静态优化；按实际需要补充测量能力 |
| PGO | 尚未建立 | 后续独立排期；先选有实测依据的内联／去虚化／布局消费者，再按需要扩充分配寿命反馈 |
| survivor／多代龄／自适应晋升／pretenuring | 尚未实现 | 基于真实寿命分布实现晋升策略，不以单次 nursery 调参替代 |
| 并行 GC | mark 已交付，update 也已交付 | 剩余是 plan/copy/reclaim 串行部分和队列效率，不从头重做并行 mark |
| 并发／增量 major、并发搬迁 | 尚未实现 | 并发 major 纳入主线；并发搬迁为更深层的独立设计 |
| arena／region 扩容与归还 | M34 已交付 | 不重复排期；继续优化 metadata、稀疏图和归还成本 |
| 24／32-byte HFA | 小值目标分类已有，Scoop 前置条件受限 | 在真实不可内联调用上重新评估，包含内部 ABI 和含引用聚合值 |
| 父接口表缓存／String hash 缓存 | M62 控制实验未采用 | 当前父接口优化采用完整父表内嵌及直接投影，并消除动态路径的重复查表，见 §5.2.1；其他缓存方案按真实复用分布决策，不将一次负结果视为永久排除 |
| nursery 放大／HashMap 起点改掩码 | M62 未采用 | nursery 转向寿命策略；HashMap 转向探测算法、布局和检查消除 |
| jump-threading 等额外 LLVM pass | M58 样本未形成稳定收益 | 在新的 Managed 根计划及真实负载上重评，不直接全局开启 |
| 源码调试信息 | 仍不完整 | 补足采样和内联归因所需的信息；不是生成程序提速本身 |

历史文档中的“留待后续”只说明当时范围。例如旧待办仍出现单个 1 GiB arena 的描述，而当前代码已是可扩展 region；实施依据应以当前规范和代码为准。

## 5. 编译器主线：让高层信息持续有效

### 5.1 扩展内联，并加入实际消费者需要的跨过程分析

先支持普通成员、getter、构造器、回调包装及带 catch/finally 的 CFG 克隆。代表消费者是 `ArrayList.get`、迭代器、`forEach`、`use`／清理作用域、codec 和协程包装。M62 的顶层小正文导入只是起点，不能靠提高统一阈值覆盖这些结构性缺口。

将内存读写、参数捕获、返回别名、具体类型和可能挂起等事实用于真实优化消费者：

- 非内联 helper 不保留参数时，调用方仍可继续分析局部对象；读取 getter 不修改 backing 时，重复 load 和检查可以消除。
- SCC 内求解需要传播的事实；callee 改变后只使相关 caller 的分析失效。当前三轮循环可以逐步改为受影响函数的工作列表，代码增长选择仍只影响优化，不决定程序是否合法。
- `NoGc`、不抛异常、不挂起、无内存写入、无同步和不捕获是不同事实。`@GCLeaf` 可以修改 native 内存，不能据此附加 `readonly`／`nosync`。
- 跨 Cone 摘要只保存消费者实际需要的数据，依赖变化沿现有缓存更新规则失效；不建立新的证明、收据、资格或重复语义校验体系。

### 5.2 全程序去虚化、调用点特化和受保护的快速路径

分三层落实：局部已知 exact type → 静态依赖闭包内的可达类型／调用图 → profile 指示的热门类型。前两层可以产生无条件 direct call；第三层只能生成“TD／目标比较成功时执行特化正文，否则走完整原分派”的分支。

可特化的对象包括常量参数、小型固定长度、closure 目标、具体 receiver、常见 Option／enum 分支。它们对集合、编解码和 callback 密集代码尤其重要。限制重复 clone 和代码增长，并以真实命中率／机器码大小选择。

“静态依赖图”不自动意味着全部运行时类型已知：native 创建、公开入口、跨 image 对象、callback 和可被外部访问的类型必须进入可达性分析。没有闭合时保留原分派。PGO 的类型频率只是提示，不是语言类型规则；这种带 fallback 的 AOT 特化不需要先建设 JIT／deoptimization。

### 5.2.1 接口视图复用与 smart cast 成功路径

本节数字是修改前的调研基线。当前已完成[完整父表投影](INTERFACE-PROJECTION.md)及[接口检查与取表合并](INTERFACE-CHECKS.md)，设计、产物合同和验证见各交付记录；下面的历史查找次数不代表新实现。

当前接口值保存 `{ object, itab }`。同一 exact interface 的复制保留这两个分量，调用直接读取已有 itab 的方法 slot。需要从 Child 取得 Parent 视图、且不知道具体 class 时，lowering 从 object 重新读取 TD，再调用 `scoop_rt_itable_lookup` 线性扫描该实际类型的 itable 目录；不是沿父接口链逐层寻找。该 helper 不缓存结果，现有 Child 视图也不保存其他接口的表。将转换后的 Parent 视图存入局部值后，其 itab 能正常复用；已知具体 class 的部分转换使用静态目录索引，无需线性扫描。

smart cast 已有正确的路径类型收窄：命中分支中的引用读取变成 typed view／Retype，值类型读取使用必要的 Unbox，不为每次普通成员读取重复生成同一个 `is`。但是 `scoop_rt_is_instance` 只返回 Boolean，不返回已找到的接口表；目标为 interface 时，后续读取仍可能重新构造接口视图。每次读取产生的临时 receiver 目前也没有统一复用同一对象、同一目标接口的转换结果。

2026-10-24 使用本机现有 release 工具在 Darwin ARM64 编译一个普通源码探针，检查 MIR、LIR 和最终机器码，并运行核对成功／失败分支和派生类覆写结果。各函数通过参数接收对象；Parent 声明两个方法，Child 继承 Parent 并声明自身方法。以下数字是函数成功路径中保留的 helper 调用次数，不是时间或加速比；不计调用者建立参数视图的成本。

| 源码形态，每次调用两个 Parent 方法 | `is_instance` 次数 | `itable_lookup` 次数 | 当前结果 |
| --- | --- | --- | --- |
| 参数已经是 Parent | 0 | 0 | 直接使用参数携带的表，仍有两个接口间接调用 |
| 参数是 Child，直接调用继承方法 | 0 | 2 | 每次调用分别建立 Parent 视图 |
| 参数是 Child，先保存 `val parent: Parent = value` | 0 | 1 | 转换一次，两个方法复用 parent 的表 |
| 参数是 Any，`is Parent` 命中后直接调用 | 1 | 2 | 类型检查与取表分离，两个 receiver 分别查表 |
| 参数是 Any，`is Parent` 命中后保存 Parent 局部值 | 1 | 1 | 后续方法复用表，但类型检查的查找结果仍未复用 |
| 参数是 Any，`is Child` 命中后调用两个 Parent 方法 | 1 | 4 | 每次调用依次建立 Child 和 Parent 视图，未消除中间取表 |

同一探针中，`is Concrete` 收窄到 final class 后，两个方法被现有内联优化消除，成功分支化简为常量结果；`is Base` 收窄到 open class 后仍保留两个虚调用，派生类覆写生效。后者只知道 Base 的子类型约束，不能据此直接调用 Base 实现。因此现状是已有类型收窄与已知具体类型的优化，但缺少接口视图复用及类型检查结果与取表的联合优化，不能概括为“smart cast 分支完全没优化”。

当前不依赖 PGO 的实施顺序已调整为：

1. 已知 Child → Parent 通过完整内嵌父表直接投影，第一次转换就免查目录；继承调用直接使用 Child 表中的对应物理槽。动态路径按同一对象值、同一 exact interface 复用静态 itab，保留有用的 Child 视图和当前 managed 引用，不跨可变 binding 的赋值使用旧结果。
2. 已完成接口类型检查与成功路径取表合并：新增返回静态目录项的 NoGC 查询，覆盖 `is`／`as`／`as?` 及否定临时值；纯 Boolean 检查保持原成本。成功路径按数据流复用 metadata，不额外保活对象。空接口匹配不以 slots 非空判定。runtime contract 15、实现及三目标验证见上述记录。
3. 继续利用 final class、确定的分配来源或已证明唯一的调用目标去虚化和内联；仅收窄到 interface／open class 时保留必要动态分派。
4. 测量完整内嵌的静态表、机器码和编译成本；确有明显膨胀时评估固定父表指针共享。对于仍需动态查找的非父接口转换，再评估目录和调用点缓存。M62 的历史实验不排除本轮布局优化。

主要证据：[runtime 类型检查与查表](../../runtime/src/rt.c)、[接口视图构造与已知 class 快路](../../compiler/lir-lower/src/function/interfaces.rs)、[已有 itab 的方法分派](../../compiler/lir-lower/src/function/call/dispatch.rs)、[smart cast 读取](../../compiler/hir-lower/src/expr/smart_casts/reads.rs)、[MIR 已知类型读取](../../compiler/mir-lower/src/body/known_reads.rs)、[已知具体类型传播](../../compiler/mir-lower/src/optimization/flow.rs)。源码、原始 stage dump 与机器码留在仓库外，文档只记录检查范围和结论。

### 5.3 从局部 box 消除走向 class 标量替换与部分逃逸分析

第一步已交付[普通 class／局部游标字段替换](CLASS-SCALARS.md)，三目标的 Point／Cursor 分配消失；随后完成[字段替换指导内联](ITERATOR-INLINING.md)，真实 ArrayList 反复遍历的 next 得以展开，迭代器分配消失。创建尚未展开、多个引用调用和逃逸等情况仍保留。后续按实际调用与字段状态推进以下其余能力，不能将局部结果视为部分逃逸或全程序消除已经完成。

建议依次实现：

1. **普通 class／iterator 字段替换。** 无 release hook 的局部小对象，构造器及实际字段操作可见时，把字段状态转成 typed locals／SSA。保留构造时的调用、异常和求值顺序；循环游标必须保留每轮状态。
2. **字段敏感的 containment 与跨过程逃逸。** 局部对象包含其他候选时传播其逃逸；调用只读取对象、不保留引用时，可生成传字段的特化版本。不能因“返回给 caller”就永远放弃 caller 内的消除。
3. **合流与部分逃逸。** 多个路径产生的字段通过 phi 合并；仅在冷分支真正发布对象时物化，常见路径保持值表示。处理别名、循环动态实例和 EH 后，再扩展复杂对象图。
4. **固定小数组／临时 buffer。** 常量索引可逐元素替换；动态索引和大对象再考虑栈存储，不把所有不逃逸对象机械改为 alloca。

例：循环中创建临时 `Point`，通常只读取其坐标，少数错误路径将它加入诊断列表。当前 all-or-nothing 分析会保留分配；部分逃逸可以在发布路径创建对象，并保存那一时刻的正确字段值。物化只负责对象存储，不能重新执行构造器的副作用。

身份比较、取址、pin、release hook、原子访问和跨线程发布分别决定是否可替换。某些局部身份关系可以静态折叠，但不能合并本应独立的对象。新增 safepoint 前必须保活物化所需引用。Graal 的 PEA 是成熟方向参考，而非可直接移植到 Scoop 的实现。[O6]

### 5.4 栈分配与区域分配

这两项应纳入深度优化，但优先级低于彻底消除对象：

- 非逃逸对象的地址确实被内部调用需要、无法标量替换时，可研究栈对象或 caller 提供存储。含引用字段仍须精确扫描、更新；栈地址不得伪装成当前只接受 heap／immortal 的普通 managed 对象。
- 真实拥有共同生命周期的一批临时对象可研究 region allocation。释放整个 region 前必须没有外向逃逸；和挂起、EH、循环、native borrow 组合后仍要闭合生命周期。
- 栈大小、递归和 coroutine suspension 会限制适用性；大数组不应无条件进入线程栈。原有 heap 路径保留。

这些方案需要明确内部表示和扫描合同，不要求引入新的用户所有权类型系统，也不以通用区域框架作为 class 消除的前置。

## 6. 托管内存循环与 GC 根计划

### 6.1 先打通普通数组循环

已交付[有界整数／Boolean 数组初始化](ARRAY-INITIALIZATION.md)及[普通数组访问的边界检查与分块](ARRAY-BOUNDS.md)：同一稳定数组长度提供静态证明，显式 count／其他数组通过长度 guard 进入 64 次完整块；条件失败仍执行原 checked 正文。复用支配分析覆盖内联到外层循环的访问，保持异常前缀和 poll／roots，三目标普通读取、变换和复制已获益；短循环回退与编译／空间成本一并记录。综合求和中未展开的 getter 仍因 caller 增长额度保留，后续应结合实际检查消除后的净成本处理，而非直接提高全局阈值。以下路线仍用于更广的数组遍历。

第一组目标应是普通 Managed 代码中的整数／浮点数组遍历、拷贝、变换、搜索和聚合：

1. 展开数组／容器的实际 getter/setter，传播不变 length、backing 与归纳变量范围。
2. 对可证明合法的访问消除逐元素检查；存在动态边界时用 guard 选择优化循环，不满足 guard 就执行保留原行为的循环。
3. 把循环分为含正常 poll 的外层和有界内层；每次外层 poll 后重新取得更新后的对象基址，derived pointer 只在无 safepoint 的内层使用。
4. 在内层启用 LICM、load/store 消除、向量化、适量 unroll、memcpy/memmove/fill 等惯用形式识别；保留尾块和重叠复制语义。
5. 再扩展嵌套循环、简单分支、多个数组、引用写入和 bulk barrier。

边界检查不能随意提前抛异常。例如原循环会先写入前几个元素再越界，预检若直接抛错会改变可观察状态；guard＋原循环 fallback 能保持这一顺序。零次循环不能因优化而读取数组或执行原本不会触发的检查。

别名关系来自真实来源、对象不等／区间检查和调用效果。值类型只保证值副本独立，其中的引用字段仍可能指向同一对象；`val` 也不代表对象不变。适当传递 TBAA、alias scope、capture 和 memory effects，必须逐种操作有正确依据。

### 6.2 整数、严格浮点与 CPU 特化

Scoop 整数 wrapping 语义不能因优化被添加不成立的 `nsw/nuw`；除零、`MIN / -1` 和 shift masking 保持。严格浮点逐元素 SIMD 通常可行，但任意重结合 reduction 和自动 FMA 会改变结果；优先 LLVM 可支持的有序 reduction，以及保留每元素舍入次数的并行运算。[O12]

如实际数值库需要显式 FMA、向量类型或允许重结合的操作，再设计清楚的库 API 和语义，不能将全局 fast-math 作为达到性能目标的捷径。

基线覆盖 ARM64 NEON 与 x86-64 基础目标，再对确认受益的内核提供 CPU multiversioning。AVX2／AVX-512／SVE 等必须同时考虑目标可用性、代码大小和低核数设备；不能把本机 `-march=native` 产物当作所有发布目标的表现。

### 6.3 深层前提：解除 site 表示对普通优化的过早限制

当前 RS4GC 已在普通优化之后运行；问题不是简单把这个 pass 再往后挪，而是更早的唯一 site、逻辑恢复标记和预先准备的根计划使普通变换不能自由复制／合并调用。

建议建立一次专门的后端设计：在语言 CFG、EH 和主要优化完成后，根据实际保留的调用／poll 形成最终物理 site 与根信息；受控 CFG 克隆生成新的 typed site，源码位置可保留共同来源。继续由 RS4GC 处理最终实际活跃的 AS1 引用，不另写一套 LLVM 指针活跃性复核器。LLVM 官方也建议在大部分优化阶段保留抽象 GC 模型，较晚转为显式 relocation。[O13]

这将为更一般的 loop versioning、unswitch、PRE、hot/cold splitting 及后续 LTO 提供基础。机器 tail duplication 和固定 outgoing argument area 则是另外的后端问题：需要实际机器 site／PC、stackmap、unwind 合同支持后才能解除，不能认为 IR 方案自动解决它们。

第一阶段数组内层不含 safepoint，可以先独立交付；不应等待整个后端重构才能获得数组收益。涉及内存访问的有界块仍可能缺页或被抢占，静态块长不提供墙钟暂停保证。

### 6.4 poll、根存储与派生地址

GC 对代码质量的影响还包括保存／重载引用、寄存器压力和入口检查。当前条件 poll 已只在慢路物化根，优化应针对剩余成本：

- 在最终实际使用点缩短引用存活区间，复用不重叠的 root slots，消除多次保存在相同位置的同一引用；EH、native transition、Context 及隐式清理仍可能保持对象存活。
- 优先把 derived pointer 表示为基对象加可重算 offset，在 safepoint 后重算地址。更一般的 base／derived relocation 或寄存器根需要扩展真实 stackmap 消费与机器上下文保存，不能将当前 base=derived 的假定静默放宽。
- 对频繁进入的小函数和计算循环，比较更少的线程局部 poll 状态读取、无请求快路的分支布局以及已有 leaf 激活检查。进一步的单请求位或保护页 poll 属于独立 runtime／平台设计，必须与 native 返回及多线程 epoch 协调一致，不能仅比较一条 load 的吞吐。
- 将异常热路径的显式根、聚合值重载和内联后的剩余 frame 成本一起归因；不能为省根而将可能抛出、回调或 GC 的调用误标成普通 leaf。

这些工作可与 §6.3 分开实现。主要验收是实际 spill/reload、栈帧、root slots 和运行性能，以及另一个 mutator 请求 GC 时的响应；并非简单追求最少 stackmap 记录。

## 7. 测量基础设施、PGO 与发布优化

### 7.1 测量与归因：提前独立交付

目标是回答“时间和内存花在哪里、为什么没有优化、修改后是否真正改善”。测量数据首先供开发者判断问题和改进通用实现，无需应用构建消费 profile。例如 [M62 的 V3 内联成本修正](../milestone62/PROGRESS.md#intrinsic-标量的内联复制成本)通过对照和机器码发现 intrinsic 标量被误计为 aggregate copy，修正后普通 release 构建直接受益；这已经形成了测量指导静态优化的闭环。

早期范围以现有 benchmark、GC 统计和成熟平台工具为起点，分批交付：

| 能力 | 首批用途与产出 | 后续按真实问题补充 |
| --- | --- | --- |
| 可复现对照 | 固定程序、输入、输出、工具链与构建选项；保存独立进程样本，对照耗时、吞吐、分配、RSS 和尾延迟 | 扩展真实应用和跨语言负载；区分启动、稳态与空闲后突发 |
| CPU 与调用归因 | 复用平台采样工具和 LLVM 符号化能力，补足函数／源码位置与可用的调用栈，定位热点及间接调用 | 内联栈、单态化实例、bridge／adapter 归因；按平台能力增加 PMU 与等待／调度分析 |
| 分配与 GC 观测 | 复用已有分配总量及 GC 阶段统计，区分分配量、存活量、堆容量与 RSS；关联暂停和应用进度 | 为寿命／晋升问题增加类型及分配点采样、存活分布；为并发问题增加必要的时序事件 |
| 优化决策诊断 | 在已调查的内联、分配消除和循环优化上说明成功或未命中的原因，关联源码与小范围 IR／机器码 | 随新增优化补足诊断，区分机会缺失、证明条件不足和成本选择 |

已交付[按需 MIR 决策报告](OPTIMIZATION-REPORT.md)：复用观察缓存，提供内联与局部分配消除的实际决定、成本和已有声明／内联来源；真实 ArrayList next 的共同估价已由报告解释。三目标的生成产物不变，默认编译与显式收集的扰动分别保留。这完成一项具体诊断消费者，后续采样和其他 pass 的诊断继续独立推进。

源码行表、必要的内联信息和可展开调用栈直接服务归因，完整调试器功能不作为首批前置。现有对象大小／类型元数据也不等于已经具备分配点与寿命统计；这些信息需要针对实际问题设计采样。分配点采样、详细 GC 事件和优化诊断应按需启用，测量其扰动，不在每次分配中默认回溯栈或记录全部事件。平台不支持的硬件计数应如实标记，不能拼成跨平台一致的指标。

独立验收是：在代表程序中找到可复现瓶颈，解释相关优化决策，完成一个通用改进，并在不使用 PGO 的普通 release 构建中验证端到端收益及回退情况。首批只需让具体问题完成这个闭环，后续优化不必等待表中所有增强能力。报告保留结论和复现条件，原始样本留仓库外；复用已有工具和脚本，不为未来 PGO 预建通用采集平台、事件框架或固定一套万能 profile 格式。

### 7.2 PGO：后续独立的编译反馈能力

PGO 将具体程序的运行数据作为构建输入，自动影响该次编译的优化选择。它仍可以是 AOT 静态编译；与前一节的区别在于谁消费数据、是否影响特定程序的构建，而不在于有没有静态生成机器码。测量能力可以在完全没有 PGO 的情况下持续发挥价值，PGO 则复用其中适合自身需求的归因和采集基础。

PGO 不要求自研所有 profiling 工具，也不要求每一种优化重新实现；可以采用 LLVM 成熟的格式、合并工具和已有优化消费者。[O14] 但 CPU 热点采样不天然提供精确边计数、间接调用目标／类型分布或对象寿命。每个消费者必须明确所需信息，必要时再增加相应插桩或采样转换，不能把“已有性能报告”等同于“已有可直接使用的 PGO 输入”。

建议在以下条件具备后安排专项：

1. 已有代表应用及可复现的训练／独立评估负载，可以检验输入变化和不同生命周期下的表现。
2. 存在成熟的优化消费者，实测瓶颈来自静态信息不足的选择，例如热门调用边、动态目标或冷热布局；缺少 EH 内联、class 标量替换等变换能力时，先补实际变换。
3. 针对该消费者的小范围反馈原型在独立评估上有稳定净收益，足以承担训练、额外构建、体积及 profile 维护成本。

这不是要求全部静态优化完成后才能研究 PGO；只是不将它默认排进早期必做阶段。候选消费者包括热调用边内联、受保护的去虚化、冷热分离、循环选择和函数布局，每次按已有证据选择，不必一开始成套实现；分配点寿命反馈单独评估。

需要区分后端与高层反馈：LLVM profile 能帮助对应的后端消费者，但当前重要内联发生在 MIR。若要影响 MIR 选择，还需将可归因的热调用信息提供给 MIR；仅给最后的 C linker 增加 profile flag 不会改善已经错过的高层机会。测量阶段保留所需源码／符号信息，具体 profile 导入和匹配合同到 PGO 实施时再设计。

训练集与评估集分开，包含启动、常见交互、长时间运行和突发。profile 不匹配或过时时退回正常优化；profile 内容不能改变程序是否合法。profile 与目标 CPU／优化选项是普通构建输入，缓存仍先做更新判断，命中直接复用，不增加命中产物检查。

### 7.3 全程序优化与 ThinLTO

建议先完成 Scoop 层的全程序可达性、特化、内联和 DCE，再加入 LLVM ThinLTO。两者互补：MIR 保留对象、泛型、协程语义；LLVM 处理跨函数标量和机器相关机会。ThinLTO 通过 bitcode summary 与选择性导入减少 Full LTO 的编译成本，但它不是给当前纯机器对象增加一个 linker flag。[O14]

需要明确以下实际工作：

- 由 driver／链接编排提供选中的语义／优化输入，优化 stage 只经 IR crate 交换完整数据；codegen 不读取上游实现来重新解析语言。
- `.slib` 可选择携带所需优化正文或 pre-RS4GC bitcode；普通机器对象消费仍可工作。复用现有 Strong／ODR 身份和定义选择，不把不同语义定义因机器码相同而合并。
- 所选 LLVM 优化结束后生成最终 roots、RS4GC、机器码和 EH／stackmap／callable registration。导入、clone、internalize 和 DCE 后，附属数据必须属于最终物理定义。
- runtime C 可以先试独立的 LTO，再评估 Scoop 与少量 native helper 的联合优化；当前 `-fno-lto` 和预编译资源选择需要相应修改。runtime 的 PGO 同样按 §7.2 独立评估，不作为 LTO 的前置。不能绕过 FFI 效果或依赖消失的副作用。
- 实际 ABI／bitcode producer 兼容性按已有格式和版本规则表达；保持缓存命中直接复用，不引入新的来源认证或生产 replay。

默认增量开发构建和充分优化的发行构建可以使用不同优化成本，源码语义和公开 ABI 保持一致。内部专用签名只能用于已闭合的调用集合；外部调用和取址入口保留规范签名。

### 7.4 聚合值、函数值与尾调用

- **HFA 和聚合参数。** 重新评估 24／32-byte HFA、参数拆分、返回值直接构造、消除中间副本。M62 的 2.2% 是 C translation units 控制实验，不能决定完整 Scoop ABI 收益；先选择确实不能内联的数学／几何／codec 调用。
- **含引用聚合值。** 研究将其拆成标量与独立 AS1 leaves 传递，避免一律内存往返；每个 leaf 必须进入正确的 roots／relocation，不能把引用封进 LLVM aggregate 就假定 RS4GC 会处理。
- **动态函数适配。** 普通 typed invoke 已有。重点优化经 `Any`、结构化函数转换或 variance 产生的 adapter／bridge 链；exact signature 可直接走 typed 路径，转换只在实际需要时执行。环境＋typed entry 的其他表示需要先处理函数 identity、GC 和 FFI 语义。
- **内部调用约定。** 全程序闭合后，可为非导出、未取址函数采用适合 Scoop 的寄存器约定或专用签名；C ABI 与公开 Scoop ABI 的边界保留适配。避免为每个小函数增加一层新的 trampoline。
- **tail call／递归。** 源码尾递归转循环可先在 MIR 完成；一般机器 tail call、frame pointer 省略、poll 状态专用寄存器，需要另行处理栈遍历、异常、callback 和寄存器压力。不能直接删除现有后端限制。

## 8. GC 与内存主线

### 8.1 让分配接近必要的写入成本

TLAB 已存在，但成功 bump 后仍有清零、对象头、精确大小、starts／line metadata 和统计更新。建议测量每分配的指令、原子操作和 cache miss，再逐项推进：

- 固定大小／已知 TD 分配专门化，消除重复的动态 shape 读取、分支和通用 helper；选择少量高频尺寸或在调用点展开，控制 text 增长。Go 1.27 的尺寸专门化说明这是正常且仍有收益的方向。[O3]
- 合并清零与构造写入、采用已清零 TLAB 或批量初始化；必须满足对象被 GC 观察前引用槽有效、payload／padding 和 MaybeUninit 的现有语义。不是无条件取消 zeroing。
- 同一无 safepoint 区间的分配／计数／登记可研究合并，但并发查询、对象发布、pin 和 native 访问依赖的事实必须在所需边界成立。不能直接把现有原子字段改成非原子。
- 将大对象清零／复制与后续首次触页一起测量。无界操作不能伪装成 GCLeaf；需要响应优化时研究分块与部分初始化期间的精确扫描。

### 8.2 写屏障、remembered set 与页映射

当前屏障使用四级 page map 的 acquire 查询和 card atomic OR，M56 已有单 block 内的有限复用。进一步方向是：

1. 证明目标处于本次无 safepoint 初始化区间、确属 young 且未发布时，消除不需要的代际屏障；release／large 等预晋升对象单独处理。
2. 合并同 card 的写入、循环批量写入与范围屏障；遇到可能抛出或发布的边界，必须已登记此前写入。
3. 在区间内提升不变 metadata 查询；之后才评估对齐 region、分层索引、已分配地址区间等更便宜的地址到 card 映射。改变堆放置有碎片和地址空间成本。
4. 按真实 young／dirty 集合或 bitmap 摘要缩小 minor 扫描范围，避免年轻工作量固定时暂停仍随稳定旧堆容量增长。

以后并发标记的 SATB／增量更新屏障与 old-to-young barrier 是不同职责，消除规则必须分别成立。JDK 26 的双 card table 是减少并发争用的参考，不应在 Scoop 尚无对应并发消费者时照搬一套双表。[O8]

### 8.3 晋升策略：survivor、反馈和 pretenuring

首次存活即晋升会使中等寿命对象污染旧代；高存活图则可能为一次收集复制过多。建议依次比较：

- 一个 survivor 阶段／简单年龄阈值，针对活过少数 minor 后死亡的对象。
- 根据实际存活 bytes、晋升失败、分配速率和目标 live set 调整 nursery／晋升；策略局限于 GC 收集决策，不变成全阶段资源预算。
- 先用存活分布测量指导通用晋升策略；按分配点识别长寿命对象并直接进入旧代，是进一步的 pretenuring 优化。运行时在线反馈与构建时 PGO 分别评估，后者按 §7.2 后续排期；反馈错误只影响性能，保留正常可回收性。
- 对几乎全存活的批次评估原地晋升／保留 block，避免无收益复制，但同时测量夹带垃圾和碎片。

survivor 并非高存活问题的统一答案：对象全部长寿时多复制一轮更差。必须覆盖低／中／高存活、稀疏旧到新引用、单／多 mutator、pinned 对象和长期稳定内存。目标是降低总分配服务成本、full 频率和尾延迟，而不是增加代数。

### 8.4 mark／copy 的局部性、并行度与元数据

当前 marker 已有 64 个对象的批量任务和数组分片，不能把“批处理”当作从未实现。仍可研究按 block／page 聚集扫描、同布局对象成组处理、预取、局部 work queue／批量 steal，减少队列锁、全局 pending 争用和随机 metadata 访问。Green Tea 的启示是调度单位和数据局部性一起改变，不是简单给 mark 循环加 SIMD。[O2]

Scoop 的 Immix block 含不同尺寸对象，不能直接套用 Go 同 size-class page 的扫描内核；按实际对象密度、布局和引用 mask 决定是否分组。共享 scan 描述可增加常见 flat-reference 快路，确有收益时由同一描述生成扫描代码，避免另一套布局来源。

plan/copy/reclaim 仍有串行部分。预留目标空间后，可将无重叠复制任务交给既有 worker pool，再汇合发布 forwarding／回写和回收；先测 copy 是否占主导，避免让带宽饱和、少核调度或串行规划抵消收益。局部并行化可以与并发 major 独立交付。

按当前结构计算，每个 32 KiB small block 的 `size_units` 为 8192 bytes，四份 bitmap 为 2048 bytes，两份 line bitmap 为 64 bytes，共 **10,304 bytes，约占 block 容量 31.4%**；搬迁 source 的 forwarding 表另占 32 KiB。这是结构容量核算，不是 RSS 测量。[元数据分配](../../runtime/src/gc/heap.c)

可研究空 block 的闲置 metadata 归还、连续分配 side tables、按对象起点稀疏表示、forwarding offset 压缩及与大小类别相关的表示。优先减少确实保留的内存，再选择查询／分配热路的压缩；不因节省表大小而增加每次指针访问的高成本搜索。

### 8.5 将并发 major 纳入正式主线

若目标包括大 live set 下的交互和尾延迟，长期只使用 STW full 会形成结构性限制。并行 mark/update 可以缩短暂停，但工作量仍随存活图增长；面对 Go、Kotlin/Native CMS 和 Java 低延迟 GC，建议明确安排并发 major，而不再将它永久留作无日期的“以后”。

优先研究的形态是：短 STW 根处理 → SATB 或增量更新维持下的并发 old-generation mark → 短 remark／终止处理 → 可并发的 sweep，必要压缩仍单独安排。先选择一种完整标记不变量，不叠加多个 collector 框架。

真正需要解决的合同包括：

- native 与 Scoop 的所有引用写入、原子引用、对象发布、handle／pin、静态根和线程生命周期都进入同一标记协议。
- 并发 major 期间若允许 minor 移动年轻对象，mark 队列、SATB 缓冲和 old-to-young 关系必须同步更新；不能在后台持有会失效的裸对象地址。
- sweep 的元数据退休、TLAB／page-map 读者和地址复用必须协调；无法跟上分配时需要明确的正常 GC 调度和退化路径，并实测其尾部。
- 当前 release hook 在 STW 中执行，不能静默改为后台线程执行。可将无 hook 的回收独立处理；hook 延迟、线程和资源可见性如需变化，先修改语言／runtime 合同。
- 并发 mark 不自动消除压缩暂停。继续 full 搬迁整个相关图，仍可能造成长 update；应同时评估低频压缩、分区 remembered set 和受影响区间更新。

**并发搬迁是下一层独立项目。** 它需要读屏障或等价的对象转发机制、原子引用／pin／FFI 协作和更复杂的回收终止；可以参考 ZGC，但不能把现有 STW copy 放到后台就算完成。[O9] 是否采用由不可消除的大图尾延迟决定。若普通并发 mark/sweep 已满足桌面目标，额外 load barrier 可能不值得成为默认成本。

## 9. 协程、库算法与表示

### 9.1 协程真实完成路径

在 M58 基础上继续跨函数传播“不会挂起”，处理跨 Cone、普通动态目标已知和 EH 的同步路径。对确实会挂起的函数，研究：frame live-slot 精简与复用、减少反复保存／恢复、共享 adapter 代码、tail wrapper 穿透，以及只在需要时保留长期存储。

“运行时同步完成”优化需要专门协议设计：callee 可能在返回前同步 resume，也可能在另一个线程竞争 resume，甚至保留 continuation。不能先传栈 continuation，看到 Suspended 后再复制到堆。可行方案必须从调用开始就保证唯一完成、发布与 lifetime；如采用新的内部 fast-result／延迟 frame 协议，应保留公开边界适配和原取消／Context／异常行为。

真实受益消费者为立即就绪的异步读写、无等待的任务组合和多层业务包装。对象池不是默认答案：会延长引用存活，增加跨线程重用／ABA／清理成本。已有 frame 能标量替换或不分配时，优先消除。

任务调度属于 core／asyncio 等实际库：优化 ready queue、批量唤醒、避免无意义跨线程切换、背压和 buffer 复用；依据真实工作窃取／I/O 消费者实施，不另造与现有任务树平行的调度运行时。

### 9.2 集合、文本和数据处理

| 方向 | 具体候选 | 必须保持的行为 |
| --- | --- | --- |
| HashMap／HashSet | byte control metadata、Swiss-style 分组探测、fingerprint、SIMD／word-at-a-time 查找、小表快路；评估 resize 尾延迟 | hash/equals 一致、删除／墓碑、负 hash、迭代与回调重入；不能直接删除 ModificationCounter |
| ArrayList／Deque／builder | 扩容的 bulk copy、零初始化与尾部清理合并、容量估计、迭代器标量替换、减少中间容器 | 按值快照、GC-safe 清槽、元素顺序和异常；capacity 仍是当前 backing 的扫描范围 |
| String／UTF-8 | ASCII 与分块 SIMD 快路、字符计数、搜索／比较；长文本反复查询的索引和 hash 复用 | Unicode scalar 索引、嵌入 NUL、非法输入策略、独立快照和线程安全 |
| JSON／文本解析 | 批量字节分类、减少每字符 helper、直接解析到具体值、减少临时 String／容器 | 数字／Unicode／重复字段／错误偏移等实际规范；正确性相同后才比较速度 |
| I/O／协议 | 读取／写入合并、减少 native transition、buffer 重用及已有 slice/view 的直接消费 | borrow/pin 生命周期、部分读写、取消、背压和资源释放 |
| 数值与排序 | 数据局部性、专业算法、目标 SIMD、成熟数学／压缩／加密后端 | 同精度、同结果合同；第三方后端性能另列，不能全部归因编译器 |

HashStorage 已缓存每个条目的 hash；不能把“缓存条目 hash”重新算作新优化。String 查询键的 hash 复用是另一个问题。M62 side-cache 实验中，短字符串一次使用明显变慢，长字符串十次复用受益；应优先在实际重复查询的操作层复用，再决定是否改变所有 String 对象的布局。

String 的 `length` 扫描 UTF-8，随机字符 `get` 从头定位。编译器可对不变文本做 LICM，库可优先使用已有 byte cursor 的 StringIterator；真正大量随机索引再比较稀疏索引／缓存计数的存储成本。普通 `map`／`filter` 的融合也须保留副作用和异常顺序，不能将所有 eager 链无条件改成 lazy 或单循环。

### 9.3 更紧凑的数据表示

这些方向不能因涉及 ABI 就永久排除，但应各自有对象尺寸和缓存行为的证据：

- **紧凑对象头／TD 编码。** 评估合并状态或以紧凑类型索引替代部分指针；保留唯一 exact TD、pin／release-ready 和 FFI 布局。JDK 27 的 8-byte header 说明“小对象很多时 Java 必然更臃肿”已经不是可靠假设。[O7]
- **压缩 managed 引用。** 在堆内存储压缩值，寄存器中使用正常指针，可能显著减少对象图／数组带宽。需处理堆地址范围、immortal、AtomicRef、扫描、Option niche 和 ABI；避免将堆上限或解码成本隐藏。与某些并发搬迁指针编码是替代或需要联合设计的方案。
- **含引用 enum 更紧凑布局。** 当前独占变体槽避免 GC 读 tag，但大 union 可能浪费空间。研究共享引用槽或按 tag 扫描时，必须定义 tag／payload 的有效过渡、MaybeUninit、复制和并发 GC 观察规则；其语言布局条款也要先修订。
- **字段重排、冷热拆分、AoS／SoA。** 优先用于内部、未暴露布局和地址的类型／专业集合；`@CLayout`、基类前缀、连续数组和公开取址合同不能被破坏。值类型已连续内联，不需要重新包装成引用才能优化。
- **活跃前缀／稀疏容器扫描。** capacity 远大于 size 时可能有意义，但 GC 当前只知道完整 MaybeUninit backing。优先普通分段容器等既有表示；真要减少扫描范围，需有通用且可正确发布的存储／scan 合同，不能让 collector 按 ArrayList 字段名特殊处理。

## 10. 代码布局及最深层候选

先用已有物理引用关系推进未使用函数／TD／scan／adapter 的裁剪、可见性收敛、函数排序和冷热分离。M62 已减少 Darwin 内部边界导出，仍应分别测 `.text`、可装载只读数据、链接信息、文件体积、启动和 RSS，不能把符号表缩小全部记作常驻内存节省。

| 候选 | 价值与定位 | 采用前提 |
| --- | --- | --- |
| 安全 ICF／共享泛型机器正文 | 控制单态化与内联产生的体积，提高指令缓存命中 | 保留可观察的函数地址、类型／ODR identity、TD 和初始化；不同身份可共享物理实现不代表语义合并 |
| BOLT 等链接后布局优化 | 大型程序的分支与指令缓存；优先 Linux | LLVM 22 BOLT 面向 x86-64／AArch64 ELF；不能当作 macOS 通用方案。Scoop 的自定义 callable PC 范围、stackmap、LSDA／unwind 必须随代码变化正确更新 [O15] |
| 自动预取／循环 interchange／tiling | 大数组、矩阵、图算法和内存带宽 | 真实数据依赖、别名、异常顺序和 GC 区间合法；先用 LLVM 已有能力，不自建通用多面体编译框架 |
| 分区／区域回收与并发压缩 | 降低大 live set 下 full 的最长暂停 | 完整 remembered／load barrier／pin／native 合同，独立评价额外 mutator 成本 |
| JIT、OSR、deoptimization | 动态类型分布频繁变化时可追随当前热路径 | 先证明 AOT＋PGO＋guarded specialization 仍有关键差距；需要代码缓存、重建 frame／虚拟对象、GC/EH/调试集成，不能仅接上 LLVM ORC 就宣布完成 |

JIT 应保留为可研究的技术分支，而不是达到本目标的必选前置。Scoop 的主要优势更适合先集中在完整的 AOT 优化体系；对稳定部署的程序，全程序特化与 PGO 可以争取大部分关键机会，对瞬时变化很大的应用则应承认需要单独验证。

## 11. 推荐实施顺序与交付单位

以下是依赖顺序，不预先占用新的里程碑编号。每个交付单位均应有真实源码／产物／运行闭环，不能先建设通用优化框架再等待未来消费者。

| 阶段 | 可交付内容 | 主要依赖 | 首要验收 |
| --- | --- | --- | --- |
| A：测量与归因基础 | 先交付可复现对照、CPU 归因、已有分配／GC 指标和相关优化诊断；逐步扩展跨语言基准 | 现有程序、benchmark 与成熟工具；无 PGO 依赖 | 找出并解释真实瓶颈，验证通用优化在普通 release 构建中的收益 |
| B1：抽象消除 | 完整父表投影、接口视图复用与 smart cast 成功路径、成员／构造器／EH 内联、class／iterator 标量替换、相应跨 Cone 正文 | 现有 typed 读取与 MIR 优化 | 已知父接口首次查表及动态重复查表消除；for／forEach、use、codec 等接近手写循环；对象分配与效果一致 |
| B2：数组内核 | BCE、别名／范围信息、有界托管内存循环、严格 SIMD | 现有 poll 分块；B1 暴露访问更有利 | 普通 Managed 数组接近相同算法 C／对手；跨块 GC 正确 |
| B3：低成本 runtime／库 | 固定分配、屏障合并、metadata 精简；HashMap／String／bulk copy | 已定位热点 | CPU 与 RSS 双向收益、短帧尾延迟无显著退化 |
| C1：跨过程与 PEA | 效果／逃逸摘要、containment、冷路径物化、调用点特化 | B1；CFG／EH 完整性 | 未内联 helper 和稀有逃逸不再迫使热路径分配 |
| C2：GC 演进 | survivor／晋升反馈、局部性和剩余并行阶段；并发 major 设计及实现 | 真实寿命／阶段测量；写屏障合同 | 稳定旧堆、分配突发、低核数及长时运行的尾部 |
| C3：静态发布优化 | 可达性／全程序优化、后置物理 site、ThinLTO | B 阶段提供真实受益代码；GC 后端合同；无 PGO 依赖 | 普通优化构建的端到端改善及可接受的编译／体积成本 |
| D1：深层表示 | 不可内联 ABI、含引用聚合、协程协议优化、紧凑布局 | C1／C3 和相关 ABI 设计 | 几何／对象图／异步负载，独立跨产物与移动 GC |
| D2：条件性分支 | 栈／区域分配、并发搬迁、压缩引用、BOLT、JIT | 相应热点仍显著，具体设计已闭合 | 解决可复现短板；完整报告新增常态成本 |
| P：PGO（后续按条件排期） | 所选消费者的采集、profile 导入、优化反馈和构建支持 | 测量基础、成熟消费者、代表性训练／评估负载；满足 §7.2 的启动条件 | 相对无 PGO 基线的独立评估净收益、输入变化适应性及训练／构建成本 |

A 提前开始、分批交付，并持续支持 B／C／D，不要求测量能力全部完成才开始优化。P 是独立排期项，表中位置不表示必须等 D 完成；测量、普通静态优化及 LTO 均不以 PGO 为前置，也不因已有采样能力就默认提前启动 PGO。并发 major 应在 B 阶段形成详细设计，避免最后才发现现有屏障／FFI 合同难以演进。

每项开始实现前更新真正受影响的规范：

| 改动 | 规范重点 |
| --- | --- |
| 测量与归因 | 实际新增的调试信息、诊断选项与 runtime 统计合同；复用现有 benchmark／工具不扩展语言语义或 ABI |
| 内联／WPO／逃逸 | 实现 2.2～2.5、2.6～2.8、2.13、2.19；语言 3.3、4.4、8.1／8.2 的行为保持 |
| PGO | 到专项实施时规定 profile 构建输入、归因、消费者与失配回退；保持正常缓存复用及无 profile 构建 |
| 数组循环／site 演进 | 实现 2.4、2.5、2.19；runtime 3.2／3.3；语言 10、11.2、11.4 |
| ABI／对象／enum／引用表示 | 语言 4、7.4、13～14；runtime 2、3.2～3.4；实现 2.4～2.6 |
| 分配／并发 GC | runtime 3.1～3.9、4、5、8～9；语言 1.2、9.1.6、11.14、13～14；实现 GC 写操作与 roots 条款 |
| 库算法 | 对应库规范及其普通行为测试；只有确实改变 core／FFI 合同时才扩展编译器或 runtime |

### 11.1 长期试验与配置交付原则

优化主线会持续比较多种方案，逐项验证、分批交付，并根据真实负载修正取舍。各项方案按以下原则决定如何提供给用户：

- **release 默认启用。** 成本低、在代表负载和支持目标上收益比较稳定的方案，验证正确性及必要组合后即可进入默认设置，随实现进展持续交付。
- **独立开关或等级。** 成本过高、收益依赖特定场景或表现不稳定的方案，提供单独开关或有实际意义的等级选择。默认配置采用适合普遍负载的关闭状态或较保守等级，使用者可为自己的应用选择更高投入。
- **同时衡量收益与成本。** 成本包括编译／链接耗时与峰值内存、产物体积、运行时额外 CPU／内存，以及必要的 profiling／训练成本；收益同时看吞吐、尾延迟、启动和实际内存占用。记录适用负载及回退情况，以实测决定默认归属。
- **等级表达投入和策略。** 等级表示分析深度、优化投入或激进程度，不保证等级越高程序就越快。开关与等级控制优化选择，各配置均遵守语言语义和正确性合同；优先复用现有 profile／配置机制，具体选项随实际方案确定。
- **保留对照并持续调整。** 性能报告同时保留默认 release 基线和所选开关／等级的完整配置。可选方案在成本降低、收益趋于稳定后可以进入默认配置；默认方案出现稳定回退时也应重新评估。每项交付简要记录当前默认值、可选设置、收益依据和已知成本。

## 12. 如何证明达到了目标

### 12.1 工作负载与两条对照轨道

| 类别 | 初始来源／新增代表程序 | 主要指标 |
| --- | --- | --- |
| 数值与内存循环 | 现有 primitive、aggregate、small-values、codegen；新增数组变换／归约／矩阵小内核 | ns/op、指令、分支、cache miss、带宽、向量化 |
| 值／接口／高阶函数 | collections、containers、interfaces、局部 class／closure、EH 作用域 | 吞吐、间接调用、allocations/op、bytes/op |
| 对象寿命与 GC | allocation、survivors、old-graph、region churn、parallel allocation | GC CPU、复制／晋升、请求与恢复延迟、稳定／峰值 RSS |
| 文本与数据 | String、HashMap、真实 JSON／charset 的端到端读写 | bytes/s、Unicode 正确性、分配、长短输入和重用分布 |
| 协程与并发 | 同步／真挂起／尾包装、任务树、实际 asyncio／HTTP 流 | completion/s、每任务分配、线程扩展、p95／p99 响应 |
| 桌面与生命周期 | 文档／树模型更新、布局或真实 GUI 集成、启动—交互—空闲—重开 | 输入到完成延迟、漏帧、空闲后突发、RSS 回落、启动 |

已有 [benchmarks](../../tests/benchmarks/README.md) 是良好起点，但尚不足以支持四语言综合结论。机制轨道保持同一算法、数据、数值精度及可观察结果；产品轨道采用各语言正常且高效的 API／库，包含 Java primitive arrays、Go value slices 等最佳实践，避免故意制造装箱劣势。第三方 native 算法占主导的结果单独解释。

配置分两组：默认优化发布构建；各语言合理调优后的构建。当前先建立无 PGO 的 Scoop 基线；PGO 交付后新增 Scoop PGO 对 Go PGO 等调优配置，并保留无 PGO 结果。Java 充分预热后另列 G1／ZGC 或 Graal，Kotlin/Native 使用相应 release 与 GC 配置。每个配置给出完整结果，不能逐项挑选不同 GC 的最优值再拼接；支持 PGO 不作为开始跨语言比较的前提。

### 12.2 测量方法

- 固定工具链、输入规模、输出、CPU／线程数与内存条件。Darwin ARM64、Linux amd64 各自比较；GNU／musl 不混算，跨平台和跨语言只比较共同支持的目标。
- 先构建和完成正确性验证，再独立计时；不并行运行编译、fixture 或另一组 benchmark。预热、随机／交替次序、多个独立进程及足够长的测量窗口，保存原始样本于仓库外。
- Java 稳态使用 JMH 或等效的 fork／预热设计；Go 用 `testing.B` 等成熟工具；Scoop 和 Kotlin/Native 使用可检查输出的等效 harness。检查常量折叠、死代码消除、提前算好结果和错误的计时边界。[O16]
- GC 性能不启用 stress；移动／minor stress 只承担定向正确性。统计本身可能有开销，生产形态与诊断形态分开对照。
- GC 报告分别记录 stop-wait、已停稳工作、允许恢复和实际 mutator／应用进度；不能相加不同阶段的中位数推导 p99。按进程和事件说明样本量，长尾要用足够事件验证。
- 延迟服务基准固定并报告 offered load、吞吐、错误率和背压；只看已经完成请求的耗时可能漏掉排队。桌面模型不等于真实 GUI，需补真实帧循环与低配置实机。
- 冷启动区分首次启动、系统页缓存已热的进程重启；RSS 区分稳定、峰值、空闲回落及下一次触页。堆容量、虚拟映射和 RSS 分别报告。

### 12.3 达标口径

对每一个对手、每一种固定配置，分别计算每项速度比 `对手耗时 / Scoop 耗时`；等工作量的吞吐比与之对应。先报告类别，再按事先固定的权重计算几何平均 `exp(sum(weight * log(ratio)))`。不平均毫秒，也不把 RSS／启动／尾延迟混进吞吐分数。

建议将“综合运行速度达到”定义为代表套件的速度比至少 1.0，且统计区间支持这一判断；区间无法区分时记为接近或结果未定。将“明显超过”作为更高目标：几何平均至少 1.10，置信区间下界高于 1.0，同时公开每个显著回退项。1.10 是建议的项目验收幅度，不是预测的可实现加速。

吞吐达标之外，桌面主目标还要求典型交互的尾延迟与内存达到预先确定的应用目标；任何类别出现稳定的大幅回退都应单独处理。M56 的 minor 约 1～2 ms 只能继续作为待实机校准的工程目标，不能把它称为所有堆大小和调度条件下的保证，也不能拿它代替 full 与实际应用响应。

采用顺序为“单项机制 → 独立程序 → 真实应用 → 组合版本”。做一次关闭某优化的对照识别其净贡献；无需穷举全部优化组合。优化间收益不能相乘推算最终总加速，优化减少一个瓶颈后必须重新采样。

正确性测试遵守仓库现有分工：普通 API／行为放 Cone，编译错误和必要产物／ABI／GC／优化专测使用 fixture 或小型 typed IR 断言；不新增全量快照、来源审计、通用成本计费或生产语义 replay。性能门槛只用于项目验收和回归判断，不决定 Scoop 程序是否合法。

## 13. 一手资料

资料读取于 2026-10-23。LLVM 机制尽可能引用 22.1 文档；网站中的后续版本能力不自动代表当前本机 LLVM 已支持。外部报告数字仅说明该技术在其原实现中的价值，不是 Scoop 收益预测。

- [O1：Go PGO](https://go.dev/doc/pgo)；[Go GC guide](https://go.dev/doc/gc-guide)。
- [O2：Green Tea 设计](https://go.dev/blog/greenteagc)；[Go 1.26 默认启用及变化](https://go.dev/doc/go1.26)。
- [O3：Go 1.27 release notes](https://go.dev/doc/go1.27)。
- [O4：Go Swiss Tables](https://go.dev/blog/swisstable)。
- [O5：HotSpot 性能机制与逃逸分析](https://docs.oracle.com/en/java/javase/25/vm/java-hotspot-virtual-machine-performance-enhancements.html)。
- [O6：Graal compiler／partial escape analysis](https://www.graalvm.org/latest/reference-manual/java/compiler/)。
- [O7：JEP 519，紧凑对象头正式化](https://openjdk.org/jeps/519)；[JEP 534，JDK 27 默认启用](https://openjdk.org/jeps/534)。
- [O8：JEP 522，G1 降低同步及写屏障成本](https://openjdk.org/jeps/522)。
- [O9：JEP 439，Generational ZGC 的读写屏障和搬迁](https://openjdk.org/jeps/439)。
- [O10：Kotlin/Native binary options](https://kotlinlang.org/docs/native-binary-options.html)；[内存管理官方文档源码](https://github.com/JetBrains/kotlin-web-site/blob/master/docs/topics/native/native-memory-manager.md)。后者用于核对 CMS 和分配器，网页读取超时后改读同一官方仓库。
- [O11：Kotlin/Native 与 Objective-C ARC](https://kotlinlang.org/docs/native-arc-integration.html)。
- [O12：LLVM 22.1 vectorizers](https://releases.llvm.org/22.1.0/docs/Vectorizers.html)。
- [O13：LLVM 22.1 statepoints](https://releases.llvm.org/22.1.0/docs/Statepoints.html)。
- [O14：LLVM ThinLTO](https://clang.llvm.org/docs/ThinLTO.html)；[Clang profile 使用文档](https://clang.llvm.org/docs/UsersManual.html#profile-guided-optimization)。
- [O15：LLVM 22 BOLT README](https://github.com/llvm/llvm-project/blob/release/22.x/bolt/README.md)。
- [O16：OpenJDK JMH](https://github.com/openjdk/jmh)。

[O1]: https://go.dev/doc/pgo
[O2]: https://go.dev/doc/go1.26
[O3]: https://go.dev/doc/go1.27
[O4]: https://go.dev/blog/swisstable
[O5]: https://docs.oracle.com/en/java/javase/25/vm/java-hotspot-virtual-machine-performance-enhancements.html
[O6]: https://www.graalvm.org/latest/reference-manual/java/compiler/
[O7]: https://openjdk.org/jeps/534
[O8]: https://openjdk.org/jeps/522
[O9]: https://openjdk.org/jeps/439
[O10]: https://kotlinlang.org/docs/native-binary-options.html
[O11]: https://kotlinlang.org/docs/native-arc-integration.html
[O12]: https://releases.llvm.org/22.1.0/docs/Vectorizers.html
[O13]: https://releases.llvm.org/22.1.0/docs/Statepoints.html
[O14]: https://clang.llvm.org/docs/ThinLTO.html
[O15]: https://github.com/llvm/llvm-project/blob/release/22.x/bolt/README.md
[O16]: https://github.com/openjdk/jmh
