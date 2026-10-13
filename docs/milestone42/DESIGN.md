# M42 设计：平台无关标准基础库

状态：正在实施，分批交付与实际验证见 [进度记录](PROGRESS.md)。基线 `89da1b35`。日期：2026-10-13。

依据：[语言规范](../specs/SCOOP-SPEC.md)、[标准基础库规范](../specs/SCOOP-STDLIB-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)与[路线图](../ROADMAP.md)。公共 API 行为以标准基础库规范为准；本文规定实现组织、依赖、交付批次和验证范围。

## 0. 目标与已确认范围

在 core 之上交付普通 `scoop.std` Cone，覆盖集合、集合算法、文本、堆内字节、编码、惰性序列、Result/Option 组合、RNG、UUID 和时间日期数据。命名靠拢 Kotlin，类型关系和执行语义遵守 Scoop。

用户已确认的边界：

1. Map/Set 同时交付 unordered、insertion-ordered 和 sorted 三类；分别使用 Hash、LinkedHash、Tree/Sorted 命名，不用一个 Ordered 混指两种顺序。
2. math 本次不做，后续作为独立包；BigInteger/Decimal 同样留给后续。排序和数字解析中的必要私有算术不扩展成公共 math 模块。
3. Random 纳入本期，抽象 RNG，提供可重复的显式种子实现；系统熵由以后的系统库提供，没有隐式全局 RNG。
4. 时间范围包括 Duration、Instant、wall clock 与 monotonic clock 的来源接口和数据模型，以及常用日期/时间类型与运算。实际获取时间、系统时区与定时/调度由后续系统库实现。
5. 正则、完整 Unicode、压缩、密码学、更多格式库和持久化集合列为后续，不作为 M42 验收条件。
6. **所有库实现应能全部用普通 Scoop Cone 写出。合法 Scoop 代码若触发底层缺口，说明底层有缺陷，必须先行修复；不允许在库里绕过。**
7. **实现过程中没有修改底层实现时，最终只运行这组 Cone 的定向测试，不运行完整测试套件。** 修改底层时按实际影响补充验证，具体规则见第 7 章。

## 1. 当前基础与增量

| 已有能力 | M42 使用方式 |
| --- | --- |
| core Iterable/Iterator、List/MutableList、ArrayList | 保留原实体；新增普通 Collection 接口提供共享 size/iterator 协议 |
| MutableArray/MaybeUninit、值内联与精确 GC | 哈希表、环形队列、堆和文本转换的普通存储基础 |
| Equals、Hash、Compare、Get/Set 等接口 | 显式表达 key、元素及运算能力，不回退到 Any 或对象地址 |
| String/Char、标量迭代、UTF-8 字节快照 | 文本、编码与解析，不重写 String 表示 |
| 泛型、函数值、普通接口与跨 Cone 单态化 | 扩展算法、Comparator/Random、Sequence 和 codec 组合 |
| Encodable/Decodable 及独立 scoop.json | std 显式 codec 直接用于现有格式库，不增加编译器合成规则 |
| 源码/产物构建、artifact-only link、三种 target | std 与真实下游 Cone 的正式 CLI 验收 |

现有 StringBuilder 和核心序列 codec 使用 ArrayList，因此本期不把 ArrayList 搬出 core。Collection 的加入是小范围普通 Scoop API 演进，不是新编译器能力。List 的 add 返回 Unit、Iterator.next 返回 Option、Long index、invariant 泛型均保持。

当前 struct 的字段公开，支持直接副本更新；为需要 canonical 分量和受检构造的 Duration/Instant/LocalTime/LocalDate 等选择 immutable final class。显式 Equals/Hash 表达内容语义，不借用引用 identity；不为了复刻 Kotlin value class 增加 opaque value、校验钩子或 intrinsic。Uuid 的所有 128-bit 位型都合法，继续使用普通 struct。

## 2. Cone、包与依赖

新增 `sysroot/lib/scoop.std/Cone.toml`，语义内容为：

```toml
schema = 1

[cone]
group = "scoop"
name = "scoop.std"
version = "0.1.0"
kind = "library"

[dependencies]
"scoop:scoop.core" = "0.1.0"
```

包划分按标准基础库规范 1.1。源文件按实际功能拆分，例如 collections 下区分接口、hash、linked、tree、deque、heap、factories 和 transforms；time 下区分 duration、instant、source、parse，datetime 下区分 calendar、date、time、offset 和 period。使用普通目录，不另建插件、provider 或构建生成框架。

依赖方向：std → core；JSON 和后续系统/业务库按需依赖 std/core。本期 JSON 实现无需变化，组合 fixture 的应用 Cone 同时声明 std/json。在 std 的 collections 包用明确的 public import 再发布 core Collection/List 等；不批量再次发布整个 core prelude。

`scoop.std` 无默认导入、无隐式依赖，不改变 single-file 入口。示例和 fixture 使用真实 manifest；库安装到 sysroot 后可省略 std 的 path locator，但坐标与版本仍显式。上述显式 core 版本声明按语言规范 12.6 使用产物搜索，需要先构建 core 并提供 artifact search root。core 源码当前位于默认根 package，collections 再发布使用 `public import List` 等实际 binding，而非从 Cone 名推导 `scoop.core.List`。std 的 `.slib` 是普通 target-specific 产物，平台无关指源码/API 能力，不意味着跨 target 共用机器码。

## 3. 实现要求

### 3.1 全部通过 Scoop 实现

允许：普通 class/struct/enum、泛型扩展、接口、函数值、core 的公开数组/字符串/Option/MaybeUninit/运算符等 API，以及这些 API 已有的 runtime 实现。

禁止：std 内声明 Extern/Intrinsic、新 C/C++ 文件、直接调用 JSON 或 core 的 internal native binding、为某个 std 类型新增 compiler-known role、按 FQN 特判、绕过正常 `.slib` 消费、在 host 侧先算出结果当作库实现。静态格式表、PRNG 常量和 Unicode White_Space 小表是正常源码数据。

实施遇到问题时按下列顺序处理：

1. 先区分普通编译错误、库算法错误与合法程序的底层缺陷。文档未允许的 variance、泛型虚方法等不因 Kotlin 存在就变成 Scoop 缺陷；使用既有语言表达相同库职责。
2. 底层缺陷提取为最小普通 Scoop 用例，定位到相应 stage、core 或 runtime；测试验证实际失效行为，不把整套库复制进底层测试。
3. 先修复底层并运行其必要回归，再继续原库功能。修复独立提交/记录，关联暴露问题的 M42 用例。
4. 若触及语言、runtime 或 pipeline 契约，先修对应 spec，再修实现；普通实现缺陷不凭空扩大语言范围。
5. 不以“先完成库”为由留下库侧替代分支，也不为假想需求提前改底层。

std 的浮点解析尤其遵守此要求：现有 JSON 的 runtime parser 是 internal 路径，不可直接复用其 native symbol。text 使用普通 Scoop 实现精确十进制转换，目标结果直接舍入到 Float/Double。所需多精度 limb 操作只作为该算法的私有实现，不能变成公开 BigInteger 或通用 math 框架。

解析先按有效位与十进制指数判定数量级，能确定 overflow/underflow 时直接得到规范要求的失败／带符号零；不能对明显不需要精确展开的巨大指数盲目构造 10 的幂。必要的精确后备按实际输入和目标精度计算，不用任意字符串长度或循环次数作为数值是否合法的条件。

### 3.2 集合存储与算法

- HashMap 使用普通数组支持的哈希表，存储完整 typed key/value 与缓存 hash；扩容复用缓存 hash，不再次调用用户 hash。选择成熟的 open addressing 策略，查找终止由实际空槽／已检查的表范围保证，无“最大探测次数”语义配额。
- LinkedHashMap 在实际 entry 中保存插入次序链接，扩容与替换保持顺序。Set 复用相应 Map<T,Unit> 或同一内部存储算法，复用必须是实际调用，不能再造集合插件层。
- TreeMap 使用 B-tree，将多个 typed key/value 保存在同一节点中，减少小对象与指针跳转；TreeSet 复用相同树逻辑。查找先执行 Comparator，分裂、借位和合并使用已确定的位置，不在半修改状态调用用户代码。Sorted 接口支持边界查询而非只增加一个排序后的 iterator。
- ArrayDeque 使用环形 MutableArray<MaybeUninit<T>>；PriorityQueue 使用二叉堆，先比较并规划 sift 路径、再提交移动。删除及时清除 managed 槽，移动元素按普通值复制；不使用对象裸地址或新 runtime payload。
- 新容器的结构修改检测局限于该容器。可使用计数加仅在回绕时更换的普通引用标记，避免失效 iterator 因回绕复活；不引入全局版本 registry 或资源计费。
- 用户 hash/equals/comparator 的异常和重入必须在访问过期内部位置前处理。单次修改在完成用户比较前不提交结构；对多元素批量操作只承诺成功前缀。
- 快照集合使用不实现 Mutable 接口的普通包装与独立 backing；不能把可变对象上转一次就声称不可变快照。
- 排序使用稳定 O(n log n) 算法，首选迭代 merge sort；随机采样使用无偏 rejection/reservoir/Fisher–Yates。算法有明确输入、输出和局部状态，不引入框架式策略注册。

M42-2 具体采用线性探测、empty/full/deleted 三态槽和缓存完整 hash。已占用槽（含 deleted）达到容量的 3/4 后，下次插入清理墓碑或扩容；rehash 不调用用户代码。状态与 typed entry 分开存储，删除将 entry 的 MaybeUninit 重置为全零。三态与删除后继续探测的资料参考 [Abseil 官方设计说明](https://abseil.io/about/design/swisstables)；其工程使用 [Apache-2.0 许可](https://github.com/abseil/abseil-cpp/blob/master/LICENSE)。这里独立实现标量线性探测，没有移植 C++ 源码、SIMD/group 探测或引入 native 依赖。

M42-3 的 LinkedHashMap 复用 HashMap 的 key→Long 位置索引，另以 ArrayList 内联保存带前后链接的 entry。已删除槽切换为只含 free-list 索引的 enum variant，释放旧 key/value 引用；复用空槽不改变插入次序链。内部查找与无回调的提交步骤分开，LinkedHashMap 查找后只调用一次插入／删除提交，避免在顺序链修改过程中再次执行用户 hash/equals。iterator 仅保存共享存储和位置，不保存旧 backing 或节点值。

M42-4 采用最小度数 8 的 B-tree：非根节点常态保存 7..15 个键值，内部节点保存 keyCount+1 个子节点，所有叶节点深度相同。根允许 1..15 个键值；空集合没有根。每个普通 class 节点使用固定容量的 MutableArray<MaybeUninit<Map.Entry<K,V>>>，key/value 在数组元素内联，不逐项装箱为 Any/interface，也不为每个 entry 分配对象。保留一个溢出槽，内部节点子数组也保留对应槽；叶节点没有子数组。容量是私有实现参数，不是公共 API 或 ABI，后续可按真实性能数据调整。

节点内二分查找，完成所有用户比较后沿 parent 提交修改。插入导致 16 个键值时提升一个中位项、分裂为 8 与 7 项，必要时向根传播；删除内部项先用后继替换，叶节点不足 7 项时向有余量的兄弟借位，否则连同父分隔项合并，必要时向根传播。根空时下降一层。上述步骤不再调用 Comparator；单次操作最坏 O(log n)，完整中序遍历 O(n)。节点内移动具有固定上界，但大值类型的复制可能更贵，不宣称所有负载都优于二叉树。

删除、移动、合并后及时清空闲置 payload 与子引用；clear 遍历并清空所有节点，避免失效 iterator 持有旧节点时保活已删除值。iterator 保存节点与槽位，结构修改失效，单纯 value 替换可见，耗尽后永久返回 None。结构测试覆盖占用范围、排序、子节点数量、相同叶深、parent/位置一致性，以及连续分裂、两侧借位、合并与根收缩。实现参考 [Abseil 的 B-tree 设计说明](https://abseil.io/about/design/btree) 和 [Rust BTreeMap 的存储取舍](https://doc.rust-lang.org/std/collections/struct.BTreeMap.html)。两者分别使用 Apache-2.0 和 MIT/Apache-2.0；这里独立编写普通 Scoop 源码，不引入 native 依赖或移植其宿主实现。

M42-5 的 ArrayDeque 使用 head/count 与环形 typed buffer，增长时一次复制有效元素并归零 head。PriorityQueue 使用数组二叉堆；插入先确定目标祖先，移除先选择下降路径，再按索引路径提交移动，不在搬移期间调用 Comparator。两者复用实际使用的 MaybeUninit 槽存储与容量增长计算，不引入共同可变集合接口。iterator 读取容器当前存储，不保留扩容前的 backing。容器/API 资料参考 [Kotlin ArrayDeque](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.collections/-array-deque/) 与 [OpenJDK PriorityQueue](https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/util/PriorityQueue.html)；Kotlin 使用 Apache-2.0，OpenJDK 使用 GPLv2 with Classpath Exception。这里独立实现普通环形数组／二叉堆算法，没有移植宿主源码或增加依赖。

急切算法只取得一次上游 iterator，按需短路。windowed 复用 ArrayDeque 保存活动窗口，支持重叠与步长大于窗口的间隙；每个输出窗口复制为只读浅快照。稳定排序先取得普通 ArrayList 副本，再用两个私有数组列表逐层归并，相等时取左项；sortWith 完整排序后才写回调用者。sortedBy 先保存每个元素的 key，避免比较期间重算 selector。二分计算 midpoint 与负插入点时避免 Long 中间溢出。算法参考作者维护的 [algs4 MergeBU 说明](https://github.com/kevin-wayne/algs4/blob/master/src/main/java/edu/princeton/cs/algs4/MergeBU.java)（GPLv3-or-later）；采用标准自底向上归并思想并独立实现 Scoop 的双缓冲半开区间算法，未移植该 Java 源码。joinToString 参数命名参考 [Kotlin 文档](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.collections/join-to-string.html)，回调及 Option 行为遵守 Scoop 规范。

### 3.3 泛型 API 可实现性

interface method 不声明自身泛型。Map/Set 接口不对所有 K/T 强加 Hash，Hash 容器才具有 Equals+Hash bound；Tree 构造器显式接收 Comparator，自然顺序工厂在普通 generic callable 上声明 Compare bound。MutableSet.retainAll 是使用接口自身 T 的普通成员，由实现提供目标等价语义；其余适用批量操作保留泛型扩展。

Comparator/Random 的普通 interface 不依赖 SAM 自动转换。comparatorOf 接收真实函数值并返回普通实现；Random 的 derived sampling 使用 default body 或扩展。Sequence 的转换是带额外类型参数的 generic extension，iterator 状态在普通 class 中，避免捕获外层局部 var 或 suspend yield。

结果缺失保留嵌套 Option。例如 Map<String,Option<Int>> 的 get 返回 Option<Option<Int>>，producer、containsKey 和 decoder 均不能用 inner None 判断键缺失。需要相等、哈希、字符串化的扩展在声明处带 bound；不根据调用者具体类型在编译器中偷偷补齐能力。

### 3.4 时间与来源

wall clock 的绝对读数使用 Instant，来源接口叫 Clock；单调来源用 TimeSource，读数用 TimeMark。接口只约定后续提供者的方法，不附带系统实现；本期所有算法显式接收已取得的数据。

Duration 使用 floor-normalized 秒/纳秒和受检运算，Instant 使用 POSIX 时间轴，日历采用 proleptic Gregorian。月份变化与经过时长分离，日期加月夹至目标月末，转换到 Instant 必须显式给 UtcOffset。不同 TimeSource 的 mark 比较/相减失败，同源检查是操作前提，不是安全认证机制。

初次构造验证 canonical 分量，后续 getter/比较复用不变结果。文本解析根据语法验证输入，转换检查新的目标范围；不在每个时间操作中重放所有字段校验。不读系统 locale、时区数据库或 native 时间函数。

### 3.5 复用与新依赖

实现前核对选定成熟算法的原作者资料、维护状态与许可；优先移植必要算法到 Scoop，保留许可归属，不通过绑定 native 库绕过本里程碑的纯 Scoop 要求。M42 不预先增加 Cargo、C 或宿主运行时依赖。

新 std codec 复用 core 的 keyed/unkeyed/single-value 协议。Map 的任意 key 编码成 entry 序列；不能为了 JSON object 方便把 K 强制变成 String。来源、迭代器、RNG 状态不提供默认持久化协议。

## 4. 实施批次

每批先核对目标规范，完成真实源码与正式 CLI 闭环后再进入下一批。以下编号是实施顺序，均属于 M42 的交付范围，不把后半部分默认为延期。

| 批次 | 工作 | 完成条件 |
| --- | --- | --- |
| M42-1 | std manifest、包布局；core Collection 与 List 继承；最小 imports/re-exports；Comparator；列表工厂和快照 | application 显式依赖 std，源码构建与独立 `.slib` 消费成功；原 core-only 程序不需要 std |
| M42-2 | HashMap/HashSet、Map/Set 协议、views、工厂、操作异常 | 冲突/删除/扩容、嵌套 Option、GC 清理、能力约束与失效 iterator 完整覆盖 |
| M42-3 | LinkedHashMap/LinkedHashSet、默认 mapOf/setOf、批量/集合运算 | 覆盖首次位置、更新、删除重插、rehash 和构建器重复 key |
| M42-4 | SortedMap/SortedSet、TreeMap/TreeSet、自然/自定义比较器、边界查询 | 最坏复杂度的结构不变量测试；浮点 NaN/±0、Comparator 等价与异常路径通过 |
| M42-5 | ArrayDeque、PriorityQueue；急切集合算法、稳定排序、二分 | 真正队列/堆/分组/排序流水线可运行，回调顺序与边界完备 |
| M42-6 | Sequence 与全部适用算子/终结操作；Result/Option 扩展 | 无限输入短路、once-only、异常终止、嵌套 Option、typed error 组合通过 |
| M42-7 | 文本搜索/切分/处理、整数/浮点解析与基础格式化 | Unicode 标量、多字节边界、极值和准确舍入通过；无新 native parser |
| M42-8 | ByteArrayBuilder/Reader、大小端、Hex/Base64 | 快照、position 失败原子性、数值位型和格式标准向量通过 |
| M42-9 | Random、具名 seeded PRNG、无偏采样、shuffle、Uuid | 三 target 具名序列一致；RNG 注入、拒绝采样分支与 UUID bits 通过 |
| M42-10 | Duration、Instant、Clock/TimeSource/TimeMark | 负时间规范化、精度/溢出、同源/异源操作、POSIX 时间及 ISO 文本通过 |
| M42-11 | 日期、时间、offset、period、YearMonth 和转换 | 闰年/负年/月末/极值、显式 offset、日期周期与 Duration 区分通过 |
| M42-12 | 各类型 codec、std/json 组合、完整 std 下游应用与产物验证 | 本组 Cone 的正式定向验收闭合，按第 7 章确定是否需扩大到底层测试 |

M42-6..11 中没有尚未完成前置依赖的工作可以调整实现顺序；调整不减少交付范围。遇到底层缺陷，在对应批次前插入必要的底层修复，不把 workaround 算作库交付。

## 5. 验收矩阵

每项公共能力至少有独立 fixture；组合用例覆盖实际语言/库组合。普通功能只断言编译诊断与运行结果，不保存全量 AST/HIR/MIR/LIR、符号表或整个集合内部布局快照。

| 功能 | 正例／组合 | 失败与边界 |
| --- | --- | --- |
| 包与接口 | direct dependency、public import、core 再发布、用户自定义 Collection/Comparator | 无 std dependency、Set 名称冲突、非法 bound、invariant 类型不匹配 |
| Hash/LinkedHash | 大量碰撞、扩容后查找、相同 hash 不同 key、插入顺序、ZST、含引用 struct | 缺少 Equals/Hash 编译失败、用户 hash/equals 抛出、回调重入、删除清空后移动 GC |
| Tree/Sorted | 随机插删对照简单参考结果、边界查询、自定义相等排序、NaN/±0 | 空树、逆序 Comparator、同等价 key 替换、比较异常、极值比较不取负溢出 |
| views/iterator | live views、value 替换、Entry 值快照、重复读取耗尽、原 List 规则 | structural modification 失效、空 clear/未命中 remove 不失效、持有 iterator 时 GC |
| Deque/Queue | 环形绕回、增长、交替端操作、堆优先级与 value 内联 | 空容器、删除释放、比较异常时结构仍有效、不将 heap iterator 当排序输出 |
| 集合算法 | 稳定排序、重复 key 分组、once-only Iterable、显式 ToString/transform、Option 元素 | 长度/范围错误、single 多项、empty reduce、short-circuit 调用次数、回调异常保留前缀 |
| Sequence | 无限 generator + take、flatMap、distinct、window、跨 Cone generic 与 closure | take(0) 不触发上游、once-only 第二次迭代、失败 iterator 不重试回调 |
| Result/Option | Success/Failure、非 Throwable E、显式 getOrThrow、Some(None)、codec | 不合法错误 bound、回调异常不被 map 吞掉、失败分支 producer 只调用一次 |
| 文本 | 多字节 Unicode、组合字符、NUL、空 needle、空段、CRLF、补齐 | 非法 split delimiter/limit、范围错误、ASCII 转换不改其他标量 |
| 数字解析 | 八类整数 MIN/MAX、radix 2/10/36、Float/Double 中点舍入、subnormal、负零 | 超出目标范围、拒绝空白/后缀/非法 radix、有限浮点 overflow、避免 double rounding |
| 字节/编码 | endian 向量、负整数位型、NaN payload、快照、RFC Base64/Hex 向量 | 短读不移动 position、非法 padding/pad bits、混合字母表、空输入 |
| RNG/UUID | 已知 seeded 向量、显式自定义 Random、rejection/reservoir/shuffle、v4 bits、128-bit 往返 | 非法区间不消费源、零 bit 不消费、full-width span、空随机选择、非法 UUID 文本 |
| Duration/Instant | 正负 ns 规范化、epoch 前毫秒、round trip、极值/恰好抵消 | 除零、negate MIN、算术溢出、parse 范围失败、拒绝闰秒/缺失 offset |
| 单调时间 | 用户普通来源 class、同源差值/偏移、来源保活 | 不同源拒绝、不同域不隐式转换；没有系统取时实现 |
| 日历 | Gregorian 400 年周期、year 0/负年、月末夹取、offset 跨日、DateTimePeriod 顺序 | 无效闰日、24:00、超 ±18h、年月极值、拒绝地区时区和隐式本地时区 |
| codec | std + json、非 String key、Result、日期/UUID、显式 Comparator decoder | 重复 Map/Set key、非法值路径、未知 Result tag、来源类型无默认 codec |

Float/Double 的准确舍入验收使用已知边界向量与独立高精度参考生成的期望，不能只将自身输出喂回自身验证。PRNG 使用原算法已知向量和带脚本输入的 Random，拒绝采样分支作确定性检查，不以偶然统计波动决定测试通过。日历验证包含固定已知日期与一个完整 400 年周期的运算性质，避免仅用同一算法互相 round-trip 掩盖错误。

GC 用例针对容器含引用元素、扩容/删除、callback 异常和 iterator/closure 存活的真实边界；产物用例选代表泛型/跨 Cone 引用与 codec 组合，不对每一个文本函数重放完整链接/布局合同。

## 6. 集成与兼容性

core Collection 的普通接口增加会改变相关类型的 conformance 与 `.slib` 指纹，需要按已有依赖失效规则重建 core 和本次消费者；不预设 metadata schema 或 runtime ABI 升版。原 Array/List API 的元素类型、size、add 返回值及 List iterator 不变。若实现发现实际底层 bug，再按其自身合同判断迁移范围。

std 的下游集成 fixture 至少覆盖：

- 读取显式 String 数据，解析数值，以 Map 分组，以稳定排序输出；不借文件 I/O 制造无关依赖。
- ByteArrayBuilder 写入、Base64 往返、ByteArrayReader 读取，再通过 std codec/JSON 往返。
- 显式 seeded Random 生成数据/Uuid，Sequence 短路，Result 汇总成功/失败。
- 解析带 offset 的时间，转换 Instant，用 Duration 和 DatePeriod 分别运算，按日期聚合。
- 三个 Cone 的 provider/consumer/application，交付泛型容器与 codec，再仅凭 `.slib` 链接运行；包括 debug/release 组合的代表用例。

M42 不修改应用启动、main、CLI 的 core-only 单文件合同或系统库 API；这里的测试程序可实现普通 Clock/TimeSource/Random 以验证公开接口，但不能把测试 fixture 里的系统取时代码发布为 std 实现。

## 7. 验证范围与完成条件

### 7.1 按实际改动选择范围

用户明确要求：**实现过程中没有修改底层实现时，最终验证不运行完整测试套件，只运行针对这组 Cone 的测试。** 这条规则是 M42 的验收约定，不能因惯例要求 `cargo test --workspace` 或 `run_fixtures.py --all`。

| 实际修改 | 必须运行的验证 | 不自动要求 |
| --- | --- | --- |
| 仅本次设计与 spec Markdown | 差异、内部链接、章节引用和跨文档语义一致性检查 | 编译器构建、Rust/runtime/全部 fixture 测试 |
| std Cone、必要的普通 core API、库 fixture | M42 本组 Cone 的正式 CLI 定向套件；core API 涉及的原 List/Array/StringBuilder/codec 定向回归；下述三目标与产物组合 | 全工作区 Rust 测试、全部 runtime 测试、完整仓库 fixture |
| compiler、driver/linker、IR/meta/slib、runtime 等底层实现 | 先格式化/lint，再运行对应模块测试、独立最小回归、受影响既有 fixture，以及 M42 定向套件 | 与改动无关的重复全量验证 |
| 改变共享 ABI、GC、产物格式等确有全局影响的底层修复 | 在以上基础上按影响运行完整相关套件/目标；记录扩大范围的具体原因 | 无证据地把所有平台和所有历史验证一概重放 |
| 修改 fixture runner / 公共测试规则 | 先按 AGENTS.md 运行固定版本 ruff，再运行 runner 单元测试及受影响选择集 | 因只新增声明式 fixture 而运行 runner 全量实现测试 |

这里“底层实现”指编译器、构建/链接工具、IR/metadata/产物实现与 runtime 等宿主实现。普通 Scoop core 源码调整仍按 Cone 的 API 影响选择回归；如果它同时要求底层 Rust/C 修改，则进入相应底层分支。修复某个底层问题后，不能只记录库用例已通过而漏掉问题自身的回归。

### 7.2 本组 Cone 的定向验收

使用现有 fixture runner 的选择参数运行 `m42-*` 用例，新增公共规则前先核对 `tests/fixture_runner/README.md` 和实际 CLI。不为 M42 创建第二套 runner、不写假定存在的选择 flag。

- 支持目标：Darwin/AArch64、Linux/amd64 GNU、Linux/amd64 musl。每个目标只运行本组适用的定向集合，不因三目标覆盖变成整库 `--all`。
- 包括 std 源码编译运行、预建 `.slib` 消费、artifact-only link/run；代表用例覆盖 debug/release、generic ODR 与 moving GC。没有跨目标 native 数据的默认值或环境依赖。
- 每次记录实际命令、目标、选择集和结果；工具链不可用时记录缺少的目标，不把未运行声明为通过。
- M42-1 修改 core Collection 时，额外只选择原 List/Array、StringBuilder、核心 sequence codec 及接口继承/产物消费的必要回归；不由“core 被改过”自动推出全量测试。
- 对库源码不新增无关的 Rust lint/test；若某批实际修改 Rust/C/Python，则按相应语言规则先格式化/lint 再测试。

完成条件是各批真实库能力、negative 和组合用例闭合，公开文档与代码一致，实际采用的验证范围全部通过；不以占位实现、仅宿主算法演示或单个平台的推断替代交付。实施完成后再填写实际 PROGRESS/ACCEPTANCE，当前不预写成功数量。

## 8. 后续库清单

- 独立 math：通用数值函数、BigInteger/Decimal、其他数值类型。
- 独立 text/Unicode 扩展：完整分类与大小写、normalization、grapheme、collation、regex。
- 独立压缩、密码学、更多格式库；现有 scoop.json 继续按实际需要演进。
- 持久化不可变集合、可写 subview、LRU/access-order 等有明确使用需求后再设计。
- 系统库：真实 Clock/TimeSource 实现、系统 RNG/entropy、文件/网络/进程/线程、定时/调度、IANA 时区及 off-heap/native resource。

这些后续工作不向 M42 注入预留字段、虚构 provider、空类型或运行期失败占位。
