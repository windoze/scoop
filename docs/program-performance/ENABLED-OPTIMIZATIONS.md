# 已采纳优化与开启条件

本表记录本次[程序性能优化主线](PROGRESS.md)已采纳的方案，详细设计、验证、收益与成本见各项链接。后续采纳新方案或调整默认策略时同步更新。

“release 默认”指编译该 Cone 时使用 `scoop build --release` 或 `scoop build --profile release`，并满足表中的适用条件；独立编译器使用 `scoopc build --profile release`。CLI 默认 profile 为 debug。除表中明确列出的运行时参数外，下列优化没有各自独立的公开开关或用户可设等级，编译器内部选项不等于 CLI 参数。

相关 API 调整：[String 的字符扫描名称](STRING-API.md)在全部 profile 生效，无开关；`charLength`／`getCharAt` 保持 O(n)，`byteLength` 保持 O(1)，内部存储与算法保持。

表中 ABI 版本注明各项引入时的合同；当前统一 runtime contract 为 **19**、metadata ABI 为 **9**，配套库／runtime／sysroot 按当前合同重建。

## 程序优化

| 已采纳方案 | 开启条件 | 适用范围与主要限制 |
| --- | --- | --- |
| [普通 GC 省去搬迁覆盖登记](RELOCATION-COVERAGE.md) | 新版普通 runtime，所有 profile 默认；重链接即可，系统库可复用 | 直接消费唯一且已更新为当前副本的存活清单，省去逐对象 claim、覆盖位图及其生命周期操作。现有 GC stress 构建或显式 heap 验证保留这些诊断；真实字段／roots 转发、pin、survivor 与 ABI 保持。 |
| [宽数组元素的重复引用屏障消除](REFERENCE-WRITES.md) | release 默认；重新生成实际 ArraySet 正文，现有四个系统库与 runtime 可复用 | stride >16 B、恰有一个 managed leaf 时，完整写入后仅为不同的非 null 引用标卡。单／双字批量路径、多引用范围屏障、字段、原子、native 和初始化保持原策略；不增加根或跨 GC 保存旧引用。 |
| [固定引用布局的数组扫描](ARRAY-SCANS.md) | 使用新版 runtime 重新链接，所有 profile 默认；无需重建 core | 根据实际 References child 直接枚举元素槽，省去逐元素递归解释；保留精确范围、访问顺序及原标记／搬迁 visitor。复杂 child 保留递归，scan／对象／根及 ABI 不变；各平台收益、短对照回退和 native 栈帧成本见记录。 |
| [年轻对象初始化屏障消除](YOUNG-INITIALIZATION.md)，含[数组初始化](ARRAY-INITIALIZATION-BARRIERS.md) | release 默认，沿既有分配优化配置；旧库需重建才能获益 | 实际 TLAB 成功后，未经过 GC／观察／外传的字段、typed box 复制及数组字面量／clone／assembly 初始化跳过屏障；字段跟踪未取址副本及单前驱直线后继，数组事实仅在当前 typed 操作内使用。慢分配、用户 initializer、原子写入、外部数组发布和 C 后备保留原屏障，无新增根、布局或 ABI。 |
| [TaskContext fork 的生成分配](CONTEXT-FORK.md) | 新编译器所有 profile；release 默认复用 TLAB，含 fork 的库正文需重建才能获益 | typed Fork 降为已知 24 B Alloc／HeapStore，snapshot 跨分配恢复后写入；独立任务身份、屏障及 ABI 保持。初始 root、旧 C 后备与绑定树继续共享 runtime；小访问器由下一项处理。 |
| [Context 小访问器生成路径](CONTEXT-READS.md) | release 默认生成 current／enter／leave；全部 profile 降低 snapshot 并遵守 runtime contract 19；旧 sysroot／产物须重建 | typed 操作读写原线程根槽，snapshot 判空后读实际 root 字段；ARM64 调用内惰性复用槽的 native 地址，x86_64 直接读取 TLS。每线程增加 8 B 地址，无新增 managed 引用或根；debug／invoke／普通 extern 和绑定树保留 C 后备。 |
| [初始化完成后的就绪快路径](INIT-READY.md) | 新编译器所有 profile 默认，配套 runtime contract 18；旧库／runtime／sysroot 须重建 | 真实 typed 普通 InitializationEnter 先 acquire 读 state，Initialized 跳过 runtime／TLS／锁；其余状态、invoke、普通 extern 和入口 poll 保留。cell 仍为 16 B，根仅在慢边恢复，后续对象地址按原 GC 契约重读。 |
| [String 字节复制与 Char 编码](STRING-COPIES.md) | 新 core 协议所有 profile 一致；release 默认复用 TLAB、调用点内联及常量传播，须重建配套 core／依赖 | typed 复制按真实布局分配并在 GC 后重取源，String 字节快照消除逐字节 initializer；Char 普通正文按调用点展开。长 memcpy、List getter 快照和旧 C ABI 保留；代码／编译成本与 Linux dispatch 对照回退见记录。 |
| [MIR 末尾局部复制传播](INTERFACE-SSA.md) | release 默认，沿现有 MIR 内联优化配置 | 内联与循环选择后传播未取址局部当前别名；保持重赋快照、合流、循环与 EH，不依赖变量名。ARM64 dispatch／接口复制明显改善，lambdas 等回退如实记录并继续精化根与 SSA。 |
| [调用期间保活与返回恢复分离](INTERFACE-SSA.md) | 新编译器所有 profile 默认 | 死 direct AS1 实参保留调用期间根，省去人工后继使用及恢复；byval、展开后输入、poll、EH 与 native 协议保持，按实际 RS4GC 结果计根。各平台收益和阶段性回退分别记录。 |
| [接口分量与重定位结果 SSA](INTERFACE-SSA.md) | 局部根槽提升为 release 默认；晚期 aggregate 分解及指针合流归并适用于所有 profile | 复用 SROA／mem2reg，普通优化结束后拆分聚合、归并相同输入的指针 PHI／select，再定稿 roots；真实 ret／store／实参消费 relocated object。取址／发布／间接 ABI 等存储保持，不增加对象或接口表大小。 |
| [String hash 有界范围与提前返回](STRING-HASH-RANGE.md) | 新 core 所有 profile 默认；release 按原调用点评分展开，需重建 core 及依赖者 | ≤32 B 使用普通标量正文并及时返回，9～32 B 省去 native hash／协调／caller roots；长输入保留原后备。原结果、表示和 ABI 保持，无新开关或测量代码；极短输入的成本见对照记录。 |
| [String 比较与短 hash](STRING-READS.md) | 普通 core 正文全 profile；调用点常量折叠和 ≤8 B 常量比较 release 默认，须重建 core | 同引用／长度／空串判断、初版有界 FNV-1a 和真实 typed 常量读取；hash 范围由上项继续扩展，长文本共享后备。保持内容、身份和位宽，无新增 ABI；ASCII 单独展开经对照未采纳。 |
| [已知小型 box 生成分配](BOX-ALLOCATION.md) | release 默认 | 真实 typed box、实例 ≤128 B；ZST／GC-free／含引用值复用 TLAB，GC 后重读 payload 并执行值屏障。保持实际分配及身份，大值和 debug 使用共享后备，无新对象字段或 ABI。 |
| [普通 String 拼接融合与热二元生成](STRING-CONCAT.md) | release 默认 | 同块单用链最多八段、短常量合并；热循环／短常量二元使用同一路径，其他点共享后备。checked 一次分配并在 GC 后重读输入，保留共享结果与副作用边界，无布局或 ABI 变化。 |
| [标量计算与 Boolean 字符串常量](SCALAR-BODIES.md) | 新 core 所有 profile 默认；release 按调用点展开 | Boolean equals／hash、整数 hash 为普通正文，热／常量点可化简，普通点可共享；Boolean.toString 不再分配结果 String，旧 C ABI 后备保留。 |
| [整数专用十进制格式化](INTEGER-DECIMAL.md) | 新 core／runtime 所有 profile 默认；release 按调用点评分展开 | 20 B 栈缓冲区一次格式化，实际长度分配／复制；八位分块与两位表，动态大正文共享，C ABI 后备保留，无 NUL／locale 依赖。 |
| [普通 helper 正文闭合与循环／pointer 移植](INTEGER-DECIMAL.md) | 各 profile 导出一致，release 默认消费 | 沿 typed 依赖队列递归导出／导入，复用已完成分析；保留独立局部 place、loop target、原 intrinsic 身份和 GC 契约，不改变收益阈值。 |
| [线程入口复用已校验状态](THREAD-STATE-REUSE.md) | 使用新版 runtime 重新链接，所有 profile 默认，无独立开关 | 18 处同步线程／roots／异常／初始化入口复用 managed 检查返回的状态；native 边界 TLS 取址七次降至四次，检查与原子内存序保持，无新增字段或 ABI。 |
| [搬迁时更新存活清单](LIVE-SET-RELOCATION.md) | 使用新版 runtime 重新链接，所有 profile 默认，无独立开关 | 复制时将稳定清单条目改为当前副本，引用回写省去 source forwarding 与重复元数据定位；计划项和清单不增大，真实字段／roots 仍正常转发，保留 pin、survivor 与预留失败语义。 |
| [调用点内联评分](CALL-SITE-INLINING.md) | release 默认；扩大跨 Cone 正文范围需重建 provider；诊断由 `--emit optimizations` 开启 | 缓存函数事实，按调用点静态热度、实参、结果用途和 caller 规模比较可消除开销与净增长；热循环可接受大正文，冷点可保留小正文。沿用递归、增长及 GC 限制，无试内联或新 runtime 代码。 |
| [不可达 callable 与 stackmap 裁剪](CALLABLE-PRUNING.md) | 最终链接所有 profile 默认；配套 `link-identity-closure/19`，旧输入须重建 | 沿完整物理符号引用选择，函数代码、EH、stackmap、登记与私有数据同选；元数据保留起点由下一项进一步收紧，native archive 候选引用保守保留目标。无需新增 runtime／诊断代码或对象字段。 |
| [类型／shape 元数据裁剪](TYPE-PRUNING.md) | 元数据生产与最终链接所有 profile 默认；沿用版本 19，旧粗分组库可直接重链接 | Strong TD 与同一 exact type 的登记合组，布局／scan／完整派发表独立划分；沿实际引用删除无入口类型及方法。静态存储、初始化、immortal 和必要登记保留；库体积与生产成本增加，consumer 构建、链接和启动受益。 |
| [一个 survivor 阶段与密集块原地保留](SURVIVOR.md) | 普通 debug／release 默认关闭；`SCOOP_GC_SURVIVOR=1` 开启 | 最多占 nursery 四分之一；首次存活的密集块原地保留，稀疏对象按配额复制，下一轮晋升。减少中等寿命对象的提前晋升；长寿图的扫描成本须按负载衡量，stress／full-only 保持原策略。 |
| [nursery 容量自适应](ADAPTIVE-NURSERY.md) | 普通 debug／release 默认，初始 1 MiB、上限 32 MiB；`SCOOP_GC_NURSERY_MAX_MIB=N` 调整上限，1 固定容量 | 按存活压力扩大、连续低存活时缩小，最近 full 的活跃堆决定基准；旧代压力独立。较高吞吐与内存、单次暂停的取舍见记录；stress／full-only 保持固定策略。 |
| [固定分块的搬迁计划](ADAPTIVE-NURSERY.md) | 所有 profile 默认，无独立开关 | 每块 256 项，保留完整预留、复制和失败回退；消除连续数组倍增复制，每块增加 16 B 链接／计数，无新增对象字段、roots 或 ABI。 |
| [minor 脏卡组清单](DIRTY-CARD-GROUPS.md) | 所有 profile 默认，配套 runtime ABI 17，旧产物须重建 | 每 64 张卡共用 16 B 节点，首次写入登记，STW 只枚举／清零脏组；原有批量卡标记保留完整登记。side metadata 约 0.049%，写屏障固定成本与真实程序回退见记录。 |
| [普通正文导出与跨 Cone 主值构造器内联](BODY-EXPORT.md) | release 默认；扩展成员正文需新版 provider，主值构造可消费既有角色 | 普通字段写入、值／构造表达式和 callable-owned 字符串沿用可移植结构选择；原 96 节点过滤由调用点评分项移除；字符串引用 provider 原对象。secondary／class 初始化、缺少正文或成本不合格的调用保留，profile 间 ABI 一致。 |
| [循环内联优先级与传值调用估价](LOOP-INLINING.md) | release 默认 | 更深循环优先使用累计增长；及时消费已知 receiver，复用完整调用适配；参数／返回值快照参与估价；原执行上限由调用点收益评分替代。累计增长、六层展开和完整冷分支体积限制保持。 |
| [minor 局部块遍历与连续清卡跳过](MINOR-BLOCKS.md) | 新版 runtime，所有 profile 默认 | minor 的准备、汇总、搬迁选择和收尾只枚举年轻块／新目标；可用 region 直接索引，连续清卡由 `memchr` 跳过。对象／TLAB／ABI 不变，保留可选 survivor 的年轻成员关系与所需卡。 |
| [普通 poll 单次 phase 读取与有界叶函数省入口](ACTIVE-POLLS.md) | 新编译器生成的所有 profile 默认 | 普通 Managed poll 不读取 TLS／mode／epoch；既有有界叶函数省去入口 poll，效果仍为 Managed。root／初始化 gateway 保留首次激活，循环与慢边精确 roots 保持。 |
| [启动 site 的 owner 验证合并](STARTUP-OWNER-JOIN.md) | 新版 runtime，所有 profile 默认 | 每个 site 在首次 stackmap 合并时只解析一次 owner；重复 ODR 记录复用验证结果，仍比较完整地址与 payload，全部对应关系在 managed 执行前完成。 |
| [poll 的本线程 relaxed 读取](POLL-OWNER-READS.md) | Gateway 完整检查中，所有 profile 默认 | 全局 epoch／phase 保持 acquire，所属线程的 mode／observed epoch 使用 relaxed；普通 Full poll 已由上项进一步简化。runtime 跨线程发布与停稳协议保持。 |
| [普通内联增长额度调整](INLINE-GROWTH.md) | release 默认 | 基础额度为 `min(size / 2 + 128, 384)`，统一适用于本地、泛型及跨 Cone 正文；调用点评分按热度使用一／二／四倍上限，六层深度和跨轮累计保持，同步 suspend 基础常数仍为 96。 |
| [跨 Cone 普通方法与 getter 内联](MEMBER-INLINING.md) | release 默认；提供方需导出合格可移植正文 | 按完整签名与通用 HIR／MIR 成本选择，保留 receiver 与 typed accessor identity；泛型沿既有具体化路径，未知动态调用及缺少正文的旧库保留入口。 |
| [Scoop 符号在镜像内绑定](LOCAL-SYMBOLS.md) | Darwin 最终链接默认，所有 profile | 从动态导出移除 Scoop 持久符号，直接绑定 callable 与 metadata；已有相容 `.slib` 可重链接，普通 native C 导出保留。 |
| [完整父接口表内嵌与目录共享](INTERFACE-PROJECTION.md) | 所有 profile，统一布局／ABI | 单继承完整前缀、多继承固定偏移；目录复用内嵌片段，菱形保留完整父表。 |
| [父接口直接投影与继承方法直接取槽](INTERFACE-PROJECTION.md) | 所有 profile | 已有 Child 视图且静态知道父关系；泛型按 exact 实例处理。首次 Any 取表等动态路径另行查询。 |
| [smartcast 视图保留与块内 itab 复用](INTERFACE-PROJECTION.md) | release 默认 | 复用已取得的接口视图；动态表缓存仅保存 metadata，遵守局部值写入、取址与块边界的失效规则。 |
| [接口 SSA 快照的直线区间复用](INTERFACE-VALUES.md) | x86_64 release 默认 | 发射阶段复用刚写入的完整接口值及其 niche enum，调用／poll／别名／控制流边界结束复用；后续所有目标的局部槽提升与跨 safepoint 数据流采用上列接口 SSA 能力。 |
| [接口检查与取表合并](INTERFACE-CHECKS.md) | release 默认 | 有非空接口视图消费者的 `is`／`as`／`as?`；普通控制流上复用成功检查，写入、非共同合流与异常入口使其失效。 |
| [循环中稳定来源的接口查询复用](INTERFACE-QUERY-REUSE.md) | release 默认 | 未取址、未写入的参数或所有 CFG 环外单次定义的局部引用；首次实际查询后复用 exact interface 的成功／失败 entry，含 Boolean 与空接口，不增加对象副本或 GC roots。 |
| [普通 class／局部游标字段替换](CLASS-SCALARS.md) | release 默认 | 创建与使用可分析、不逃逸且无 release hook；字段转为 typed locals，保留引用快照与 GC 语义。 |
| [字段替换指导内联](ITERATOR-INLINING.md) | release 默认 | 可替换 class 只剩一个 receiver 调用时共同估价；继续遵守原内联成本与累计增长限制。 |
| [调用点异常路径特化](EXCEPTION-PATHS.md) | release 默认 | exact receiver／closure 和已有完整正文证明异常边不可达后，按实际剩余正文选择内联；保留正常清理、真实异常、返回快照与精确 roots。未知效果和残余 EH 保留调用。 |
| [异常区域中的局部分配消除](EXCEPTION-SCALARS.md) | release 默认 | box／closure／无 release hook 的 class 在正常和异常路径均满足来源、支配与循环存活证明时替换为 typed locals；保留真实 catch／finally、返回快照及异常出口 roots。 |
| [有界整数／Boolean 数组初始化](ARRAY-INITIALIZATION.md) | release 默认 | initializer 展开后无调用／分配；64 次有界分块，合格的无 safepoint 内层交由 LLVM 向量化。 |
| [数组边界检查消除与分块](ARRAY-BOUNDS.md) | release 默认 | 稳定上界、非负起点、唯一加一更新；直接范围证明或长度 guard，保留 checked 回退与短尾。 |
| [循环范围指导数组访问内联](ARRAY-INLINING.md) | release 默认 | 用已证明的范围去掉候选正文中的不可达检查后，参与统一调用点评分。 |
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
