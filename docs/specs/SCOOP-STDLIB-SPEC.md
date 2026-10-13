# Scoop 平台无关标准基础库规范

状态：M42 目标规范，正在实施；已实现范围和实际验证见 [进度记录](../milestone42/PROGRESS.md)。日期：2026-10-13。

本文件规定 `scoop.std` 的公共库行为；语法、类型和 core 规则以 [语言规范](SCOOP-SPEC.md) 为准，产物与运行机制分别遵守 [实现规范](SCOOP-IMPL-SPEC.md) 和 [运行时规范](SCOOP-RUNTIME-SPEC.md)。实施次序与验证见 [M42 设计](../milestone42/DESIGN.md)。本文件不是已经交付这些 API 的声明。

## 1. 库边界与公共约定

### 1.1 Cone 与源码

提供普通 library Cone `scoop:scoop.std:0.1.0`，源码位于 `sysroot/lib/scoop.std/`，唯一直接依赖为 `scoop:scoop.core:0.1.0`。用户在 `Cone.toml` 显式声明依赖，按语言规范 12.2 使用 sysroot 默认定位；不隐式依赖或默认导入 std。single-file 模式继续只有 core dependency。

| package | 责任 |
| --- | --- |
| `scoop.collections` | 集合接口、实现、工厂与急切求值的集合操作 |
| `scoop.comparisons` | Comparator、自然顺序及比较器组合 |
| `scoop.sequences` | 惰性序列与 iterator 适配 |
| `scoop.text` | 文本操作、数值文本转换、Hex |
| `scoop.bytes` | 堆内字节构建与读取、字节序 |
| `scoop.io.encoding` | Base64；包名不表示提供系统 I/O |
| `scoop.random` | Random 抽象与显式种子 PRNG |
| `scoop.time` | Duration、Instant、Clock、TimeSource、TimeMark |
| `scoop.datetime` | 日历、日期、时间、固定 UTC offset 及相关运算 |
| `scoop.uuid` | Uuid 的表示、转换与显式 RNG 生成 |
| `scoop` | Result 及 Option 组合扩展 |

全部实现必须是普通 Scoop 源码，可以调用公开 core API。std 不声明 `@Extern`／`@Intrinsic`，不携带 native source，不访问 core internal binding，不引入 compiler/runtime 特判、专用 helper 或额外 ABI。既有 core API 背后的分配、GC、UTF-8 等实现不因此复制到 std。合法 Scoop 程序若暴露底层实现缺陷，须先修复底层、建立独立回归，再继续实现库；不能以库侧 workaround 代替修复。

core 不依赖 std。现有 Iterator、Iterable、List、MutableList、ArrayList、StringBuilder、Option、编码协议与运算符接口保持原实体；`scoop.collections` 可通过普通 `public import` 再发布 core 的集合类型。JSON 继续是独立 Cone；M42 不要求迁移 JSON 或让 std 依赖 JSON。

### 1.2 命名、类型与失败

类名、工厂、扩展名尽量采用 Kotlin 的常用拼写，例如 HashMap、LinkedHashMap、mapOf、mapNotNull、getOrNull、sortedWith、Random、Duration、Instant、LocalDate、Uuid。以下 Scoop 规则优先：

- 集合类型保持 invariant，不增加 `in/out/*`、追溯 conformance、SAM 转换或 generic virtual method。需要方法级泛型的操作使用普通泛型扩展或 final method。
- size、index、容量、计数与比较结果继续使用 Long；日历年月日、纳秒的小范围分量、radix 和 RNG bitCount 使用 Int。`Comparator<T>.compare(a, b): Long` 与 core Compare 的返回口径一致。
- 缺少值用 `Option<T>`。名称中的 `OrNull` 表示返回 Option，不引入另一种 null；`Some(None)` 与 None 必须区别。
- 各模块的回调为普通非 suspend 函数值，可以分配和抛异常。需要挂起的 caller 可先求值其输入，但同步库不隐式提供挂起回调或调度。
- 参数非法抛 IllegalArgumentException，位置越界抛 IndexOutOfBoundsException。`NoSuchElementException`、`ConcurrentModificationException` 由 collections 定义，`NumberFormatException` 由 text 定义；均为普通 Exception 子类，使用 `message: String` 构造并调用既有 `Exception(Some(message))`。
- `OrNull` 只转换自身描述的缺失／格式失败，不吞用户回调异常、无效参数、分配失败或其他故障。整数和时间运算需要精确结果时显式检查溢出，不能依赖 core 整数 wrapping。
- 容量只受实际表示和分配限制；不增加固定条目数、步骤数或跨库资源预算。

只读接口约束可调用的操作，不代表对象不可变。`toX` 快照是浅复制：值元素复制值，引用元素保留引用。无元素能力约束的容器不自动获得 Equals、Hash、ToString 或 codec；字符串拼接通过显式 transform 或 ToString bound。

## 2. 集合与比较

### 2.1 复用 core 的公共访问协议

语言规范 11.10 增加普通 `Collection<T> : Iterable<T>`，只含 `size: Long`。现有 List 改为继承 Collection，仍提供 get；Array、MutableArray、ArrayList 经原有 List 关系获得该接口。不能在 std 定义 Collection 后要求 core 的 List 反向继承。

Collection 不要求 T 可比较；`isEmpty/isNotEmpty` 是普通扩展，按值查找的 Iterable 扩展单独要求 `T : Equals<T>`。保留 MutableList 的现有 `add(value): Unit`，M42 不引入与它返回值冲突的 MutableCollection 总父接口。List 的索引、快照与原有 live iterator 语义不变。

新集合名 `scoop.collections.Set<T>` 与 core 的下标赋值接口 `Set<Index, Value>` 是不同实体。core 源码当前使用默认根 package，std 内部通过 `import Set as IndexSet` 区分赋值接口；用户按普通 import 优先级选择，编译器不按类型参数数量消歧。

### 2.2 Map、Set 与视图

接口的最小交付表面如下；大部分 bulk 和组合操作由泛型扩展提供，不引入 generic interface method。retainAll 需要访问目标 Set 的等价关系，作为 MutableSet 的普通非泛型成员。

| 类型 | 必须提供的成员 |
| --- | --- |
| `Map<K, V>` | `size: Long`、`get(key): V?`、`containsKey(key): Boolean`、`keys: Set<K>`、`values: Collection<V>`、`entries: Collection<Map.Entry<K, V>>`、按 entry 的 iterator |
| `MutableMap<K, V> : Map<K, V>` | `put(key, value): V?`、`set(key, value): Unit`、`remove(key): V?`、`clear(): Unit` |
| `Set<T> : Collection<T>` | `contains(value): Boolean`，实现 Contains<T> |
| `MutableSet<T> : Set<T>` | `add(value): Boolean`、`remove(value): Boolean`、`clear(): Unit`、`retainAll(elements: Iterable<T>): Boolean` |
| `Map.Entry<K, V>` | 普通 struct，两个 public val 字段 key/value，按既有 struct 规则解构 |

Map 实现 `Get<K, Option<V>>`、`Iterable<Map.Entry<K,V>>` 与 `Contains<K>`，operator contains 委托 containsKey；MutableMap 另实现 core 的下标赋值接口 `Set<K, V>`（使用上述 IndexSet alias）。Map 的 K/V、Set 的 T 在接口层不要求 Hash/Equals，具体实现确定 key/element 的等价关系。containsValue 是单独要求 `V : Equals<V>` 的扩展。

`get/put/remove` 的 None 表示原来没有键；当 V 是 Option<U> 时，原值 None 表示为 Some(None)。`set` 丢弃 put 的旧值。Set.add 仅新增时返回 true，remove 仅确实移除时返回 true。存在等价 key 时 put 替换 value，保留最早存入的 key 对象；Set.add 不替换原元素。

keys/values/entries 是随 Map 变化的只读 live view，不复制全表；entries 每次取得的是独立 Entry 值，已返回的 value 不会随以后替换而变动。entries 使用 Collection 而非 Set，避免要求任意 V 具有比较能力。不提供 mutable entry、iterator.remove 或可写子视图。Map views 保留所属 Map 的等价关系和迭代顺序。

`getValue` 缺失时抛 NoSuchElementException。`getOrDefault/getOrElse/getOrPut` 只把外层 None 视为缺失；getOrPut 缺失时调用 producer 一次，成功后执行一次普通 put 并返回生成值。producer 若自行修改同一个 Map，按上述顺序执行，最终 put 可以覆盖其插入；producer 抛出时不进行最后的 put。

### 2.3 无序、插入顺序与排序顺序

| 类别 | 接口／实现 | 等价关系 | 遍历顺序与复杂度 |
| --- | --- | --- | --- |
| unordered | `HashMap<K, V>`、`HashSet<T>` | K/T 同时实现 Equals<自身> 和 Hash | 不承诺顺序；查找/插入/移除平均 O(1)，碰撞最坏 O(n) |
| insertion-ordered | `LinkedHashMap<K, V>`、`LinkedHashSet<T>` | 与 Hash 系列相同 | 第一次插入顺序；平均操作 O(1)，遍历 O(n) |
| sorted | `SortedMap`／`MutableSortedMap`、`SortedSet`／`MutableSortedSet`；实现 `TreeMap<K, V>`、`TreeSet<T>` | comparator.compare(a,b) == 0 | comparator 升序；查找/插入/移除最坏 O(log n)，遍历 O(n) |

不再增加含义模糊的 OrderedMap/OrderedSet。Hash 系列即使某次运行呈现稳定顺序，也没有顺序保证。LinkedHash 系列更新既有值、重复 add 和内部扩容不改变位置；删除再插入移至末尾，不提供 access-order/LRU 模式。

Hash 的库合同为：key 的 equals 是等价关系，相等 key 有相同 hash；key 在容器内期间这两项行为不变。Float/Double 仍无 Hash，不因 M42 自动获得 hash。Tree 系列无需 Hash，比较器须给出稳定全序／全预序；排序相等不要求再满足 Equals。自然排序使用 core Compare，允许 Float/Double 及 M41 的 NaN／正负零等价类。以上用户方法的数学性质由作者保证，编译器不证明。

`Comparator<T>` 是普通 interface，唯一成员 `compare(left: T, right: T): Long`。`comparatorOf((T,T)->Long)` 提供函数适配；`naturalOrder<T : Compare<T>>()`、`reverseOrder`、`compareBy`、`thenBy`、`reversed` 是普通 generic function/extension。reversed 交换实参，不对任意 Long 结果直接取负。Tree 构造器显式接收 Comparator；自然排序工厂带 Compare bound，不要求构造器拥有独立的类型参数。

SortedMap/Set 暴露 comparator 与 `first/last` entry/element 的 OrNull 形式；另提供 lower/floor/ceiling/higher 查询，分别使用 `<`、`<=`、`>=`、`>` 边界。空容器返回 None。范围筛选返回快照，首版不做共享 subMap/subSet 可写 view。

SortedMap 的具体命名为 firstEntryOrNull/lastEntryOrNull、lowerEntry/floorEntry/ceilingEntry/higherEntry，结果为 Entry?；SortedSet 对应 firstOrNull/lastOrNull、lower/floor/ceiling/higher，结果为 T?。MutableSortedMap/Set 同时继承对应的只读 Sorted 接口与 MutableMap/Set，所有 exact application 保持一致。

范围快照使用 `SortedMap.subMap(fromKey, toKey, fromInclusive = true, toInclusive = false)` 和 `SortedSet.subSet(fromElement, toElement, fromInclusive = true, toInclusive = false)`，返回对应只读 Sorted 接口并保留原 Comparator。两端按 Comparator 顺序解释；from 大于 to 抛 IllegalArgumentException，相等时仅两端都 inclusive 才可能包含该等价类。范围构造不修改来源。sortedMapOf/sortedSetOf 返回新的 TreeMap/TreeSet；toSortedMap/toSortedSet 返回不暴露 Mutable 接口的独立快照。每组均提供显式 Comparator 与要求 Compare bound 的自然顺序重载。

### 2.4 工厂、转换与集合运算

交付 `emptyList/listOf/mutableListOf/arrayListOf`、`emptySet/setOf/mutableSetOf/hashSetOf/linkedSetOf/sortedSetOf`、`emptyMap/mapOf/mutableMapOf/hashMapOf/linkedMapOf/sortedMapOf`。Map 工厂接收现有 tuple `(K,V)` 的 vararg；不额外复制一套 Pair。命名参数、空工厂期望类型推导、泛型 bound 全部沿普通规则。

listOf 返回独立只读快照，mutableListOf 返回新的 ArrayList；setOf/mapOf 的只读快照及 mutableSetOf/mutableMapOf 默认保持插入顺序。默认 Set/Map 工厂要求 Hash+Equals；sorted 工厂使用显式 Comparator 或自然 Compare bound。重复 Map key 最后一个 value 胜出，保持第一次 key 的位置；Set 保留第一次等价元素。

`toList/toMutableList/toSet/toMutableSet/toMap/toMutableMap/toSortedSet/toSortedMap` 创建独立容器；默认 Map/Set 按输入迭代顺序形成 insertion order。`toList` 的结果与源后续修改无关，不等同于把 MutableList 上转为 List。只读快照不暴露可强转回去修改同一底层容器的 Mutable 接口。

交付 addAll、putAll、removeAll、retainAll、union、intersect、subtract。默认集合运算以 Equals+Hash 建立 LinkedHashSet，保留左输入顺序，union 继续追加右侧新元素；即使来源是 TreeSet，也按结果容器的 Equals+Hash 解释等价关系。需要自定义排序等价关系时，显式提供目标集合，按目标 contains/add 执行。批量操作逐元素提交，异常保留成功前缀，不隐含事务。

Iterable 型批量输入先取得本次元素快照，再修改目标，保证 addAll(self)、putAll(self)、removeAll(self)、retainAll(self) 不依赖失效 iterator；快照期间失败不修改目标。MutableList.addAll 返回 Unit，MutableSet.addAll 返回是否新增元素；removeAll/retainAll 返回是否删除元素，putAll 返回 Unit。List 按值删除要求 Equals，Set 使用目标自己的等价关系；不为它们增加共同的可变父接口。

MutableSet.retainAll 由实现按自身等价关系建立输入 membership，再只移除不保留的原元素；保留元素的位置、对象与未变化时的 iterator 有效性不变。Hash 系列使用 Equals+Hash，Sorted 系列使用原 Comparator，自定义 Set 对其规则负责。该方法不声明额外的类型参数；仅有 contains/remove 的通用扩展无法为任意 Set 推导元素等价关系，因此不以默认 Equals 偷换目标语义。

### 2.5 迭代、异常和存储

新增 Map/Set、它们的 views、ArrayDeque 和 PriorityQueue 的 iterator 不做全量快照。创建和 next 不隐式复制容器；首次 None 后保持耗尽。创建后发生成功结构修改，尚未耗尽的 iterator 后续 next 抛 ConcurrentModificationException。替换 Map 的现有 value 不属结构修改，下一次读取可看到新值；新增/删除 key、非空 clear 属结构修改。重复 Set.add、未命中 remove、空 clear 不使 iterator 失效。

List/ArrayList 原有按索引迭代规则保持语言规范 11.10.3；不同容器的规则明确区分。修改检测只保障串行使用的契约，不提供线程同步。iterator 的失效状态不能因内部版本计数回绕而恢复有效。

单次 put/add/remove 调用用户 hash/equals/comparator 时，在完成选择前不提交容器结构修改；用户异常保持容器自身结构有效。用户回调重入修改正在操作的同一容器，外层在继续使用旧查找状态之前检测修改并抛 ConcurrentModificationException；重入操作及其他用户副作用不回滚。批量操作遵守前缀提交规则。

容量存储复用 MutableArray/MaybeUninit。退出有效范围的 managed 槽必须清零；保留槽位索引或树链接不能留下已移除元素的强引用。不能使用移动对象裸地址作 hash、持久 key 或节点 identity。具体值类型元素保留内联存储，除显式接口／Any 转换外不强制逐元素装箱。

### 2.6 Deque 与优先队列

`ArrayDeque<T>` 为实现 Collection<T> 的普通 final class，提供 size、isEmpty、addFirst/addLast、removeFirst/removeLast、removeFirstOrNull/removeLastOrNull、firstOrNull/lastOrNull、clear 和从头到尾的 iterator。用环形数组实现，两端插入/删除摊还 O(1)。空的非 OrNull remove 抛 NoSuchElementException。栈和队列直接使用这组 API，不增加只有改名意义的 Stack/Queue 实现。

`PriorityQueue<T>` 为实现 Collection<T> 的普通 final class，接收并公开 comparator，提供 add、peek/peekOrNull、removeFirst/removeFirstOrNull、clear、size 和 iterator；自然顺序工厂 `priorityQueueOf(vararg elements)` 要求 Compare bound，另有显式 Comparator 重载。堆顶为最小元素，peek O(1)、add 摊还 O(log n)、remove 最坏 O(log n)；动态数组单次扩容 O(n)。同优先级不保证稳定性。iterator 不保证排序顺序，排序输出须使用显式 sorted 操作或在副本上重复移除。

PriorityQueue.add 返回 Unit，空队列的 peek/removeFirst 抛 NoSuchElementException。插入与移除先通过 Comparator 规划 sift 路径，全部比较成功后再提交元素移动；比较失败不留下破坏堆性质的半次 sift。Deque 两端 add 返回 Unit，其非空读取/移除返回 T。

## 3. 集合算法与惰性序列

### 3.1 急切集合操作

以 Iterable 为主要 receiver，不假定它可重复遍历或有廉价索引。以下操作均在 M42 交付：

| 组 | API |
| --- | --- |
| 转换 | map、mapIndexed、mapNotNull、filter、filterNot、filterNotNull、flatMap、flatten、partition |
| 遍历与归约 | forEach、forEachIndexed、onEach、fold、reduce、reduceOrNull、count、any、all、none |
| 查找 | find、first/firstOrNull、last/lastOrNull、single/singleOrNull、elementAt/elementAtOrNull、contains、indexOf |
| 组合与截取 | zip、withIndex、take、drop、takeWhile、dropWhile、chunked、windowed、reversed |
| 分组与去重 | groupBy、groupByTo、associate、associateBy、associateWith、distinct、distinctBy |
| 输出与目的地 | joinToString、mapTo、filterTo、flatMapTo、toCollection |
| 排序与查找 | sorted、sortedDescending、sortedBy、sortedWith、sort、sortWith、binarySearch、minOrNull、maxOrNull、minWithOrNull、maxWithOrNull |

map/filter 等返回新 List；indexed 回调索引和 withIndex 的 index 为 Long。indexOf 无结果返回 -1L；查找的 OrNull 返回外层 Option。single 在零项或多项时分别抛 NoSuchElementException 或 IllegalArgumentException；singleOrNull 在这两种情况下返回 None。reduce 空输入抛 NoSuchElementException。any/all/none/find/first 等遇到确定结果立即停止，不预先遍历或计算 size。

withIndex 使用普通 `IndexedValue<T>(index: Long, value: T)` struct；zip/partition 复用 tuple。flatten 针对 `Iterable<Iterable<T>>`，由于 invariant，不假定 List<List<T>> 能上转为该类型；这类输入可用 flatMap 显式取得内层 Iterable。toCollection/mapTo/filterTo/flatMapTo 为 MutableList 和 MutableSet 分别提供普通重载，返回同一目标接口值，不依赖不存在的 MutableCollection。min/max 空输入返回 None，同值时保留首次结果；sortedBy 的 key selector 每元素求值一次后再排序。

计数参数非负，chunked/windowed 的 size/step 必须正；windowed 默认 step=1、partialWindows=false，窗口结果是独立浅快照。zip 按左后右取值，任一输入耗尽即结束；可能为判断终止从左侧多消费一项，此顺序不隐藏。take(0) 不取得上游 iterator 或调用回调。

groupBy/associate/distinct 默认使用 LinkedHashMap/LinkedHashSet，key 要求 Equals+Hash。groupBy 保留首次 key 次序及组内输入次序；associate 的重复 key 遵守 Map 工厂覆盖规则。按值 contains/indexOf 等只要求 Equals；排序只要求 Compare 或显式 Comparator。joinToString 无 transform 的重载要求 ToString，不回退到 Any。

排序稳定；非原地版本返回快照。`MutableList.sortWith` 先复制、排序后再顺序写回，比较器抛异常时不写回；自定义 setter 抛异常可以留下写回前缀。实现不以重复 List.get 构造低效的 linked-list 算法。binarySearch 接收 List、半开范围与自然/显式比较器；范围外失败，命中返回任一相等项下标，未命中返回 `-(insertionPoint + 1)`，计算必须正确覆盖 Long 边界。输入已按同一比较器排序是调用方前提，不每次重新验证全序。

具体调用约定：first/last/single 及其 OrNull 同时提供无参数与 predicate 重载；find 等于 firstOrNull(predicate)。elementAt 的负数或越界 index 抛 IndexOutOfBoundsException，elementAtOrNull 返回 None；负数不取得 iterator。count/any/none 提供无参数与 predicate 重载，all 接收 predicate。onEach 急切执行 action 后返回原 Iterable。fold 使用显式 initial，reduce 使用首元素作为 initial。indexed 回调参数顺序为 `(index: Long, element: T)`；计数无法用 Long 表示时抛 ArithmeticException。

groupBy/groupByTo/associateBy 提供仅 keySelector 与额外 valueTransform 的重载；groupByTo 的目标为 MutableMap<K, MutableList<V>>，使用目标的 key 等价语义，不额外要求 Hash。groupBy 返回只读 Map<K,List<V>>，分组 List 也是独立只读快照；associate 系列同样返回只读独立 Map。withIndex、zip、chunked、windowed 返回独立 List；partition 返回两个独立 List 组成的 tuple，true 组在前。目的地操作 toCollection/mapTo/filterTo/flatMapTo/groupByTo 先取得一次输入快照，再顺序执行回调和写入，支持输入与目标别名；快照失败不改目标，回调／目标写入失败保留成功前缀。flatMapTo 还在每次写入前取得该次 transform 返回的内层 Iterable 快照，允许内层来源与目标相同。

joinToString 的参数为 separator=", "、prefix=""、postfix=""、limit=-1、truncated="..."，以及可选的显式 transform（无 transform 重载要求 ToString）。limit 使用 Long，-1 表示不限，其他负数非法；计数参数非负的通常规则不限制此 sentinel。超过 limit 时最多额外读取一项以判断是否添加 truncated，不转换该项；truncated 和普通元素一样由 separator 分隔。所有急切算法在回调异常时直接传播，不吞成 OrNull。

binarySearch 的签名为 `List<T>.binarySearch(element, fromIndex=0, toIndex=size)` 与 `List<T>.binarySearch(element, comparator, fromIndex=0, toIndex=size)`；自然顺序要求 Compare。fromIndex>toIndex 抛 IllegalArgumentException，任一端不在 0..size 抛 IndexOutOfBoundsException；半开空范围合法。

### 3.2 Sequence

`Sequence<T>` 为普通 invariant interface，提供 `operator iterator(): Iterator<T>`。通过 `sequenceOf`、`emptySequence`、Iterable.asSequence、Iterator.asSequence、`generateSequence(seed: T, next: (T)->T?)` 和 `generateSequence(next: ()->T?)` 构造。

Iterable.asSequence 每次迭代委托原 Iterable，不强行保证来源可重复。Iterator.asSequence 与无 seed 的 generator 为 once-only；第二次取得 iterator 抛 IllegalStateException。带 seed 的 generator 每个 iterator 独立从 seed 起步；初始 seed 不执行 next，之后 None 表示结束。T 自身是 Option 时，Some(None) 是合法元素。

交付惰性 map/mapIndexed/mapNotNull、filter/filterNot/filterNotNull、flatMap/flatten、onEach、take/drop/takeWhile/dropWhile、zip/withIndex、distinct/distinctBy、chunked/windowed，以及第 3.1 节适用的终结操作、toList/toSet/toMap/groupBy/associate/joinToString。`sorted/sortedWith` 在首次消费时收集全部上游并排序；不能用于无限输入。其余算子的内存需求跟随实际行为，distinct 保存已见 key，windowed 保存窗口，不宣传所有 Sequence 都是 O(1) 内存。

Sequence 的 flatten 接收 Sequence<Sequence<T>>；flatMap 的 transform 返回 Sequence<R>，Iterable 来源通过 asSequence 显式适配。不依赖 invariant application 的隐式上转或根据名字合并 Iterable/Sequence 两个协议。

终结操作包括 3.1 的查找、遍历、归约、计数、极值、partition、reversed、分组／关联和目的地操作，以及 toList/toMutableList、toSet/toMutableSet、tuple entry 的 toMap/toMutableMap、joinToString；返回值和约束沿用急切版本。Sequence.flatMapTo 的 transform 同样返回 Sequence<R>。sorted/sortedDescending/sortedBy/sortedWith 返回延迟排序的 Sequence，稳定性和 selector 求值次数沿用集合排序。binarySearch 与原地 sort/sortWith 只属于 List／MutableList，不为 Sequence 增加对应操作。

构造中间操作不取得 iterator、不执行用户回调。每个 iterator 有自己的进度和缓存；短路操作不调用无关后缀回调。next 抛异常后该迭代器进入失败终止状态，再次 next 抛 IllegalStateException，不重新执行已经失败的回调。操作不提供事务回滚。M42 不提供 suspend sequence builder、yield、资源 close 协议或异步 Flow。

库提供的 Sequence 在首次 next 时才取得所需上游 iterator，包括 Iterable.asSequence 对源 iterator 的调用和 sorted 的收集；once-only 在调用自身 iterator 时即占用唯一迭代机会。首次 None 后始终返回 None，不再次访问来源。take(0) 不取得上游 iterator，即使请求或消费结果的 iterator 也如此；负数量与非法窗口参数在创建操作时报告。上述生命周期同样是自定义 Sequence 实现须遵守的协议，不依赖编译器注入。

## 4. 文本与数值文本转换

### 4.1 文本 API

遵守 core String 的合法 UTF-8、Unicode 标量索引与 Long 计数。交付 startsWith/endsWith、contains、indexOf/lastIndexOf、substring（委托 slice）、substringBefore/After、removePrefix/Suffix、replace/replaceFirst、split、lines/lineSequence、trim/trimStart/trimEnd、padStart/padEnd、repeat、isBlank/isNotBlank、lowercaseAscii/uppercaseAscii。

默认匹配区分大小写、按完整标量序列精确匹配，不做 normalization。空 needle 在合法起点匹配；indexOf 默认从 0 起，lastIndexOf 默认从 length 起。非法 startIndex 抛 IndexOutOfBoundsException；无匹配返回 -1L。substringBefore/After 未命中默认返回原串，允许显式 missingDelimiterValue。replace 使用从左到右不重叠匹配；空旧串在每个标量边界插入一次，包括首尾，不形成无限循环。

split 接受非空 String delimiter 或 Char delimiter；String delimiter 为空抛 IllegalArgumentException，避免隐含另一套按字符拆分 API。limit 默认 0 表示不限，负数非法，正数最多返回 limit 段、最后一段保留余下内容；保留首尾和连续分隔产生的空段，空输入得到单个空串。lines 识别 CRLF、LF、CR，CRLF 作为一次换行，保留尾部空行。

trim/isBlank 使用固定 Unicode 17.0 White_Space 集合，并提供显式 predicate 版本。所需小表随 Scoop 源码携带，不读取系统 locale 或 Unicode 数据文件。大小写转换仅提供名字明确的 ASCII 版本，其他标量保持不变；完整 Unicode 大小写、normalization、grapheme、collation 和正则另行设计。上述搜索及遍历不得通过反复 O(n) 的 String[index] 实现二次方扫描；使用 iterator 或一次快照。

padStart/padEnd 的目标长度和 repeat 次数非负；padding 是一个 Char。低于已有长度时返回等内容 String。输出字节大小检查实际溢出和分配界限，不提供任意长度预算。基础格式化复用 f-string、StringBuilder、数值 toString 和补齐，不加入 printf／通用格式 DSL。

搜索和切片索引均为 Long：`indexOf(value, startIndex=0)` 找从 startIndex 开始的首次匹配，`lastIndexOf(value, startIndex=length)` 找起点不超过 startIndex 的末次匹配（允许重叠）。startsWith/endsWith、contains、indexOf/lastIndexOf、substringBefore/After 同时提供 String 和 Char 参数；replace/replaceFirst 提供 String→String 与 Char→Char 重载。substring 的 endIndex 默认 length，substringBefore/After 的 missingDelimiterValue 默认 receiver；removePrefix/Suffix 接收 String。split 的 limit、padStart/padEnd 的 length 和 repeat 的 count 使用 Long，paddingChar 默认为空格。

trim(predicate) 先从左向右检查前缀，再从右向左检查剩余后缀，不重复检查已删除字符；trimStart/trimEnd 分别使用对应方向。isBlank(predicate) 在首次不满足 predicate 的字符处停止，空串为 true。lineSequence 在首次消费时建立字符迭代器，每次只生成下一行，支持重复迭代；空串和结束换行后的尾部各产生空行。输出 UTF-8 字节数算术超过 Long.MAX_VALUE 抛 ArithmeticException；可表示的实际分配仍遵守 core 的分配边界，不增设人为长度上限。

### 4.2 数值与布尔解析

交付八种定宽整数的 `toInt8/.../toULong` 及各自 OrNull 形式，另有 `toFloat/toDouble` 与 OrNull、`toBooleanStrict/toBooleanStrictOrNull`。整数支持 `radix: Int = 10`，范围 2..36；字母数字不区分 ASCII 大小写，允许一个前导 `+`，signed 另允许 `-`。不自动 trim，不接受 `_`，也不识别类型后缀或 radix 前缀；数值超出目标范围为解析失败。提供整数 `toString(radix)`，signed 使用负号与绝对值的数学数字，不输出二进制补码文本。

canonical 解析名为 toInt8/toInt16/toInt/toLong、toUInt8/toUInt16/toUInt/toULong，并提供 core alias 对应的 toByte/toShort/toInt32/toInt64、toUByte/toUShort/toUInt32/toUInt64 转发名，各有 OrNull。unsigned 拒绝包括 -0 在内的负号输入。只有 sign 而无数字时失败；普通字符在指定 radix 下是否合法决定其数字含义，例如 0x 在 radix=36 中是数字 33，radix=16 中失败，不把 x 当作前缀标记。整数 toString(radix) 必须显式传入 radix，字母数字使用小写，零输出 0，无正号和前导零；无参数 toString 仍使用原 core 方法。

浮点接受 ASCII 十进制 `[+-]?(digits(\.digits?)?|\.digits)([eE][+-]?digits)?`，以及精确拼写 NaN、Infinity、+Infinity、-Infinity。十进制结果直接按目标 Float/Double 精度最近偶数舍入，不通过 Double 再转 Float；有限输入舍入后成为无穷时为解析失败，略大于 MAX_VALUE 但仍舍入到 MAX_VALUE 的输入成功。underflow 可得到 subnormal 或带符号零。禁止 hex float、后缀、空白、尾随内容和 locale 变体；+NaN/-NaN 也不接受。即使数值明显溢出、下溢或全零，仍检查完整语法。NaN 使用 core NaN 常量位型；显式 Infinity 返回相应无穷。

布尔只接受 true/false。失败的非 OrNull 版本抛 NumberFormatException；无效 radix 仍抛 IllegalArgumentException。解析实现全用 Scoop；不能直接绑定既有 JSON 内部的 runtime 浮点转换 helper。必要的精确十进制到二进制运算是 text 的私有算法，不导出通用 BigInteger/Decimal 或 math 包。

## 5. 内存字节与基础编码

### 5.1 字节构建与读取

复用 core Byte（Int8）保存八位 bit pattern，输入采用 List<Byte>，结果采用独立 Array<Byte> 或 MutableArray<Byte>，不引入另一套 intrinsic ByteArray。`ByteArrayBuilder` 是 GC 堆上的 growable final class，提供 size、append(Byte)、append(List<Byte>)、clear、toByteArray、toMutableByteArray，以及 writeInt8/UInt8/Int16/UInt16/Int/UInt/Long/ULong/Float/Double。

`ByteArrayReader` 在构造时取得输入浅快照，持有 position、remaining；提供对应 read 方法、readBytes(count)、skip(count)。position 只能通过受检 seek 改变，范围 0..size。多字节读写显式接收 `ByteOrder.BIG_ENDIAN/LITTLE_ENDIAN`，不设 native-order 默认值；浮点按 toBits/fromBits 保存位型。短输入和非法区间抛 IndexOutOfBoundsException，负数量抛 IllegalArgumentException；失败的单次读取不移动 position。

builder 输出为独立快照，后续 append/clear 不改变结果；append(List) 先取得本次输入快照，再一次提交，允许同源输入与 getter 抛异常。write 固定位宽操作先保证空间再写入，不对外留下半个数值。容量增长和清零使用普通 core 存储；不提供 direct/off-heap buffer、Ptr、pin、borrow/view、显式 close 或 file/socket stream。

builder 使用无参数构造；size: Long，append／write／clear 返回 Unit，toByteArray 返回 Array<Byte>，toMutableByteArray 返回 MutableArray<Byte>。字节长度相加超出 Long.MAX_VALUE 时抛 ArithmeticException。reader 使用 `ByteArrayReader(input: List<Byte>)` 构造，size、position、remaining 均为只读 Long 属性；seek(position: Long)、skip(count: Long) 返回 Unit，readBytes(count: Long) 返回独立 Array<Byte>。零长度读取和跳过允许位于末尾。read/writeInt8 与 UInt8 无字节序参数，其他定宽方法必须显式传入 order: ByteOrder。

### 5.2 Hex 与 Base64

text 提供 `toHexString`、`hexToByteArray/hexToByteArrayOrNull`；默认输出小写，可显式 uppercase，输入接受大小写 ASCII hex 且长度必须偶数。不接受空白、分隔符和 0x 前缀。空输入合法，错误报告第一个非法标量位置。

签名为 `List<Byte>.toHexString(uppercase: Boolean = false): String`、`String.hexToByteArray(): Array<Byte>` 与返回 Array<Byte>? 的 OrNull 版本。非法输入抛 IllegalArgumentException，message 含从零开始的非法标量 index；若只有结尾缺少半个字节，index 为字符串末尾位置。输出大小超出 Long 表示范围时抛 ArithmeticException。

`Base64.Default`、`Base64.UrlSafe` 分别使用 RFC 4648 的标准和 URL-safe 字母表；encode/decode 接收显式输入。`PaddingOption.REQUIRED/ABSENT/OPTIONAL` 控制 padding：encode 在 REQUIRED/OPTIONAL 时写 padding，ABSENT 不写；decode 分别要求规范 padding、禁止 padding、接受二者。默认 REQUIRED。

Base64 为不可变 final class，构造私有，通过两个预定义实例和 `withPadding(option: Base64.PaddingOption): Base64` 取得配置。PaddingOption 为 Base64 的嵌套 enum；paddingOption 是只读属性。`encode(source: List<Byte>): String`，`decode(source: String): Array<Byte>`，`decodeOrNull(source: String): Array<Byte>?`。完整三字节分组不需要 `=`，所有模式均接受空输入；OPTIONAL 只接受完整规范 padding 或完全省略，不能省略所需两个 `=` 中的一个。编码结果长度溢出 Long 时抛 ArithmeticException。

decode 拒绝混用字母表、非法字符、空白、错误长度、错误位置/数量的 `=` 和非零 pad bits；不增加 MIME 换行模式。非法输入抛 IllegalArgumentException，decodeOrNull 返回 None；所有结果为独立字节快照。实现只读显式数据，不进行 I/O。

## 6. Random 与 Uuid

### 6.1 RNG 抽象

`scoop.random.Random` 为普通 interface，唯一必需原语 `nextBits(bitCount: Int): Int`，要求 0..32；结果的低 bitCount 位为生成位，其余为零，32 位可以是负 Int。0 返回 0 且不推进状态。实现方负责提供约定的随机位；不要求它使用特定熵源。

其余能力用普通 default method 或扩展实现：nextBoolean、nextInt、nextLong、nextUInt、nextULong、nextFloat、nextDouble、nextBytes。整数提供完整值域、until、from/until 重载；区间是半开且 from < until，参数错误在消费 RNG 前失败，跨度计算不得 signed overflow。有界整数采用 rejection sampling，不能简单取模产生偏差。nextFloat/Double 分别使用 24/53 位生成 [0,1) 网格上的均匀值；浮点范围采样和其他分布不在 M42。

有界整数固定使用对应位宽的 unsigned 完整抽样：令 span 为半开区间的 unsigned 长度，threshold 为该位宽 wrapping 的 `(-span) % span`，丢弃小于 threshold 的样本 r，返回 from 加 `r % span` 的目标位型。完整无边界重载直接返回全部随机位。nextBoolean 使用一个随机位；nextInt/UInt 使用一次 32-bit 请求。此规则同时确定拒绝分支的调用次序，避免不同 target 选择不同采样实现。

nextBytes 提供新数组和写入 MutableArray<Byte> 区间两种形式；边界先检查，非法请求不消费 RNG。random/randomOrNull、shuffle/shuffled 接收显式 Random，shuffle 使用无偏 Fisher–Yates。未知大小 Iterable 的单元素选择用 reservoir sampling，不先全量收集；空输入非 OrNull 抛 NoSuchElementException。

上述 next 系列作为 Random 的普通 default method 提供。`nextBytes(size: Long): Array<Byte>` 拒绝负 size（IllegalArgumentException）；`nextBytes(array: MutableArray<Byte>, fromIndex: Long = 0, toIndex: Long = array.size): MutableArray<Byte>` 填充半开区间并返回原数组，越界抛 IndexOutOfBoundsException，范围反向抛 IllegalArgumentException，空范围不消费 RNG。RNG 抛异常时保留已写入的前缀，不承诺回滚。

collections 提供 List／Iterable 的 random(random)／randomOrNull(random)、MutableList.shuffle(random): Unit 与 Iterable.shuffled(random): List。List 只抽取一个 Long 下标；Iterable 依次对第 n≥2 个元素请求 nextLong(n)，结果为零时替换候选，计数溢出抛 ArithmeticException。空或单元素选择不消费 RNG，Some(None) 不表示空输入。shuffle 从末尾向前对剩余 n≥2 个元素请求 nextLong(n)，与所选下标交换；异常保留已完成的修改。shuffled 返回独立只读浅快照。sequences 提供 random／randomOrNull 终结操作和惰性 shuffled；后者第一次 next 才收集有限输入和消费 RNG，重迭代按来源与同一 RNG 的当前状态执行，不新增随机源或重置种子。

没有 Random.Default、无种子构造、隐式全局生成器、系统熵读取或自动使用时钟播种。后续系统库通过普通 Random 实现接入；M42 不引入 entropy provider registry 或插件层。

### 6.2 确定性生成器

提供 `Xoshiro256StarStar(seed: Long)`，使用 SplitMix64 从 seed 的完整 64-bit pattern 连续产生四个初始 ULong 状态，再执行公开 xoshiro256** 算法；运算按 ULong modulo 2^64，旋转与移位显式实现。`Random.seeded(seed)` 返回该算法实例。

同一算法、seed 与 API 调用序列在所有支持 target 上结果相同。一个非零 nextBits 调用消费一个 64-bit 输出，取最高 bitCount 位放入 Int 低位，其余输出丢弃；nextLong/ULong 依次消费两个 32-bit 请求，先高后低。浮点分别消费一次 24-bit 或先 26-bit 后 27-bit 请求；字节填充每项消费一次 8-bit 请求。rejection 消费的额外请求按公开采样算法确定。后续算法替换使用新的具名实现，不静默改变此具名生成器的向量。

此生成器不提供密码学随机性。一般 Random 不承诺可重复，公开 seeded 类型和调用组合才具有上述可重复合同；实例可变状态不提供并发同步。

### 6.3 Uuid

`Uuid` 是普通 struct，以两个 ULong 保存 128 位，全部位型均合法；显式实现 Equals<Uuid>、Hash、ToString 和 Compare<Uuid>，按高位再低位的 unsigned 字典序比较。提供 NIL、MAX、fromLongs/toLongs、fromByteArray/toByteArray、parse/parseOrNull、toHexString/toHexDashString。字节表示固定 network order，恰好 16 bytes；标准文本为 8-4-4-4-12，parse 同时接受精确 32 hex 和 36 字符带横线形式，接受大小写，不接受前后空白、花括号或 URN 前缀。toString 使用小写带横线形式。

public val 字段名为 mostSignificantBits、leastSignificantBits，类型均为 ULong；普通主构造与副本更新不限制任何位型。companion 的 fromLongs 接收两个 Long，toLongs(): (Long, Long) 返回相同位型的 signed 载体；fromByteArray 接收 List<Byte>，toByteArray 返回独立 Array<Byte>。非法长度与 parse 格式失败抛 IllegalArgumentException；parseOrNull 仅将格式失败转为 None。NIL／MAX 是 companion 的普通 val，分别保存全零／全一位型，不读取时钟或随机源。

`Uuid.random(random: Random)` 生成 RFC 9562 v4，将调用方提供的 16 bytes 设置规定的 version/variant bits。无隐式 RNG，不承诺密码学强度或绝对不碰撞。时间型 UUID、name-based hash UUID 和系统 UUID 服务留待相应库；表示和解析仍允许其他版本的位型。

## 7. Result 与 Option 扩展

`scoop.Result<T, E>` 是普通 invariant enum，variant 为 `Success(T)`、`Failure(E)`，不限制 E 为异常。提供 isSuccess/isFailure、getOrNull/errorOrNull、getOrElse、fold、map/mapError、flatMap、recover、onSuccess/onFailure。只有被选择的分支调用回调，map 等不自动捕获回调异常；onSuccess/onFailure 返回原结果。

`getOrThrow` 只适用于 `E : Throwable`；`runCatching(block)` 返回 Result<T,Throwable>，捕获普通可抛出的 Throwable，不捕获 runtime fatal、进程信号或终止。结果方法通过普通模式匹配实现，不改变 try/throw、Option 或 Nothing 的语言规则。采用 Kotlin 的 Result/Success/Failure/getOrThrow 命名，但保留 Scoop 的显式错误类型 E；不新增隐式异常转换。

runCatching 保存普通 catch 物化后的 managed 异常；getOrThrow 与直接 throw 使用同一既有异常契约。运行时规范 5.1～5.4 的 payload 复制和 catch 物化仍适用，不额外承诺 throw 前后保持对象 identity；动态类型、值字段和 payload 中引用字段的浅复制语义保持。

Option 增加 map/flatMap/filter/fold/getOrElse/orElse、toResult(errorProducer)，Result 提供 toOption。Some(None) 保留层次；懒缺省只在缺失／失败分支调用一次。无约束的 T/E 不因此取得 Equals、Hash、ToString 或编码能力。

isSuccess/isFailure 是只读 Boolean 属性。Result 的 getOrElse 接收 `(E)->T`，fold 按 `onSuccess: (T)->R, onFailure: (E)->R` 接收分支；map 只改变成功类型，mapError 只改变错误类型，flatMap 接收 `(T)->Result<R,E>`。recover 接收 `(E)->T` 并返回 Result<T,E>，恢复成功时保存 Success，回调异常直接传播。Option.fold 按 `onSome: (T)->R, onNone: ()->R` 接收分支；getOrElse、orElse 的缺省 producer 分别返回 T、Option<T>，toResult 的 errorProducer 返回 E。扩展均在 scoop package 中，不改变 core 的默认导入。

## 8. 时间、时钟与日期

### 8.1 时间域和表示原则

| 类型／接口 | 所表达的含义 |
| --- | --- |
| Duration | 有正负号的精确经过时长，纳秒精度 |
| Instant | wall clock 时间轴上的绝对时刻，以 Unix epoch 为基准 |
| Clock | wall clock 来源接口；不是另一个 Instant 数据类型 |
| TimeSource | monotonic 时间源接口；不携带 UTC epoch |
| TimeMark | 某一个 TimeSource 内的单调读数；只能在同源内比较／相减 |
| LocalDate、LocalTime、LocalDateTime | 尚未绑定 offset 的民用日期／一天内时间／二者组合 |
| UtcOffset、OffsetDateTime | 固定 UTC 偏移及附带偏移的日期时间 |
| DatePeriod、DateTimePeriod | 日历单位变化，与精确 Duration 分离 |
| YearMonth、Month、DayOfWeek | 年月与日历分量 |

Duration、Instant 和需要范围不变量的日期／时间类型使用普通 immutable final class，构造与工厂检查后保存 canonical 字段，只公开 val 和计算属性；显式实现相应 Equals/Compare/Hash/ToString。原因是当前 struct 的字段公开且有副本更新，不能只凭 private constructor 声称原始字段不变量不可被绕过。M42 不新增不透明 struct、时间 intrinsic 或运行期反复全量验证。DatePeriod、DateTimePeriod 和 TimeMark 可用普通 struct 保存本已合法的分量；Uuid 的全部位型合法，适合 struct。

Clock/TimeSource 为供外部实现的普通接口，只有 `Clock.now(): Instant` 和 `TimeSource.markNow(): TimeMark` 的来源合同。M42 不提供 Clock.System、TimeSource.Monotonic 的系统实现、隐式 now、elapsedNow、measureTime、sleep、timer 或调度。下面所有具体数据处理仅依赖传入值，不调用来源接口读取“现在”。后续系统库负责这些取时操作及其平台行为。

### 8.2 Duration

内部使用 floor-normalized `(seconds: Long, nanosecondsOfSecond: Int)`，后者处于 0..999999999，数学值为 seconds + nanos/10^9 秒。例如 -1ns 为 (-1,999999999)。所有值有限，不提供 Infinity 或 NaN sentinel。

提供 ZERO、seconds/milliseconds/microseconds/nanoseconds 的受检工厂与对应 Int/Long 扩展函数，以及分/时/天单位构造。one day 精确为 86400 秒。交付 plus/minus、unaryMinus、times(Long)、div(Long)、compareTo、absoluteValue、inWholeNanoseconds/Microseconds/Milliseconds/Seconds/Minutes/Hours/Days、toComponents、parse/parseOrNull/toIsoString/toString。

整数除法按纳秒截断趋零，inWholeX 也向零；除零和结果越界抛 ArithmeticException，不饱和、不 wrapping。中间表达式溢出不能错误拒绝实际可表示的最终结果，不要求底层 128-bit 类型。工厂纳秒调整允许负数和跨秒值，先按数学值规范化再判断范围。

ISO 文本接受可选整体负号以及按 D/H/M/S 顺序的 `P...DT...H...M...S`，至少一个分量，只有秒允许 1..9 位小数，各整数非负；不接受 years/months/weeks、混合分量符号或空白。输出统一为 `[-]PT<绝对值整秒>[.<去尾零小数>]S`，零为 PT0S。负极值的输出和解析不能先做有符号 abs 导致溢出。

普通 companion 工厂为 `seconds(value: Long, nanosecondAdjustment: Long = 0)`、milliseconds/microseconds/nanoseconds/minutes/hours/days(value: Long)，对应 Int/Long 的同名零参数扩展函数。seconds 工厂先规范化调整量，再检查最终秒值；所有不可表示结果抛 ArithmeticException。公开只读 seconds、nanosecondsOfSecond；`toComponents(): (Long, Int)` 返回这两个 floor-normalized 分量，重建时将纳秒分量显式转为 Long 传给 seconds 工厂。inWholeX 与 absoluteValue 是只读属性。parse 拒绝格式或范围错误（IllegalArgumentException），parseOrNull 返回 None；其它异常不隐藏。文本仅接受大写设计符；存在 T 时其后必须有时间分量，不接受整体加号。

### 8.3 Instant 与 wall clock

Instant 采用 Unix/POSIX epoch seconds 与 0..999999999 nanos：1970-01-01T00:00:00Z 为零，一天按 86400 秒处理，不表示闰秒 23:59:60，也不实施 UTC-SLS smear。可表示范围为第 8.5 节 LocalDate.MIN 的 UTC 零时至 LocalDate.MAX 的 UTC 当日末纳秒。

交付 MIN/MAX、fromEpochSeconds(seconds,nanosecondAdjustment)、fromEpochMilliseconds、epochSeconds、nanosecondsOfSecond、toEpochMilliseconds、plus/minus(Duration)、minus(Instant): Duration、parse/parseOrNull/toString。负 epoch 的秒／毫秒转换向负无穷取整；与 Duration 的整单位截断规则区分。超出 Instant 域或结果表示时失败，不能 wrapping。

parse 接受第 8.5 节 ISO 日期和时间，必须携带 Z 或固定 offset；输出转换到 UTC，以 Z 结尾。没有 offset 的 LocalDateTime 不能隐式解释为 UTC 或本地时间。wall clock 可以被校时，不承诺不同 Clock.now 读数单调；求耗时应使用同源 TimeMark。

fromEpochSeconds 的两个参数均为 Long，nanosecondAdjustment 缺省为零；先规范化再检查域，参数无效抛 IllegalArgumentException。fromEpochMilliseconds 接收 Long。plus/minus、toEpochMilliseconds 和与 LocalDateTime 的转换越界抛 ArithmeticException；parse 超出域视为非法文本，parseOrNull 返回 None。

### 8.4 单调时间与 TimeMark

TimeMark 保存 `source: TimeSource` 与 `reading: Duration`，reading 是相对于该来源任意原点的有限坐标，不是 Unix 时间。供系统库从已经取得的读数构造，也可由用户数据源生成。source 使用普通引用身份，只用于判断读数是否同源，不是授权凭证、跨进程 ID 或注册表项。

提供 plus/minus(Duration)、minus(TimeMark): Duration、compareTo。减法和顺序比较要求 source 引用相同，不同来源抛 IllegalArgumentException，即使 reading 相同也不能相减。相等为 source 相同且 reading 相同；不同源为 false。不提供默认 Hash、持久化 codec 或跨来源排序。

来源实现必须返回以自身为 source 的读数，非递减且已处理其 native tick 的单位、wrap 和重置；不同启动／重置后的时间域须使用不同来源对象。M42 的数据操作不侦测系统启动、不读取 native tick。保留 TimeMark 会保活 source，按普通 GC 处理。

不存在隐式 Instant ↔ TimeMark 转换。一个 wall/monotonic 样本配对只能给出调用者自行解释的相关信息，不证明两个时间域永久同步；M42 不增加自动校准模型。

### 8.5 日期、时间与固定偏移

采用 proleptic Gregorian calendar，包含 year 0，year 范围 -999999999..999999999。Month 使用 JANUARY..DECEMBER，DayOfWeek 使用 MONDAY..SUNDAY；monthNumber 为 1..12，ISO weekday 为 1..7。

Month／DayOfWeek 是普通 enum，分别提供 number／isoDayNumber、of(Int) 受检工厂，以及按上述编号的 Equals/Compare/Hash 和大写变体名称 ToString。LocalDate(year: Int, monthNumber: Int, dayOfMonth: Int) 与 YearMonth(year: Int, monthNumber: Int) 在构造时验证；两者另提供 Month 参数重载、month 和 yearMonth（LocalDate）属性。plusDays/plusMonths/plusYears 接收 Long，fromEpochDays 接收 Long 并拒绝域外参数（IllegalArgumentException）；已有日期的偏移越界为 ArithmeticException。日期减 DatePeriod 直接反向应用总月数及 days，不要求先构造逐字段取负的 period，因此不会因 Long.MIN_VALUE 的中间取负而错误接受 wrapping 结果。

- LocalDate：受检 year/month/day 构造，MIN/MAX、year/monthNumber/dayOfMonth/dayOfYear/dayOfWeek、toEpochDays/fromEpochDays、plusDays/plusMonths/plusYears、daysUntil、DatePeriod 加减、parse/parseOrNull/toString。epoch day 0 为 1970-01-01，使用 Long；daysUntil 为 other 减 receiver 的日数。
- LocalTime：hour 0..23、minute/second 0..59、nanosecond 0..999999999；提供 from/toNanosecondOfDay、分量、比较、加减 Duration（按 24 小时回绕）与 ISO 文本。拒绝 24:00、闰秒和带日期/offset 的输入。
- LocalDateTime：组合 LocalDate/LocalTime，plus/minus(Duration)、DatePeriod/DateTimePeriod 运算、日期时间分量、与显式 UtcOffset 的 toInstant 转换、parse/parseOrNull/toString。按民用 86400 秒日进行纯值计算，不隐式调用时区规则。
- UtcOffset：totalSeconds 范围 -64800..64800，即 ±18:00；受检分量工厂与 parse，接受 Z、±HH:MM、±HH:MM:SS，±18 小时时分钟秒必须为零。输出零为 Z，非零省略为零的秒。+00:00/-00:00 均规范化为零，不表示“未知 offset”。
- OffsetDateTime：LocalDateTime 与 UtcOffset，提供 toInstant、保留 instant 的 offset 转换、保留 local fields 的 offset 替换及 ISO 文本。相等使用 local fields 和 offset，按 instant 比较须显式 toInstant，避免默认排序与相等语义冲突。
- YearMonth：受检 year/month、lengthOfMonth/isLeapYear、plusMonths/plusYears、atDay、parse/parseOrNull/toString；LocalDate 暴露对应年月。

Instant.toLocalDateTime(offset) 与 LocalDateTime.toInstant(offset) 使用相反的 offset 运算并检查结果范围。UtcOffset 只是固定偏移，不是地区时区；M42 不提供 IANA TimeZone、系统本地时区、DST gap/overlap 推断或时区数据库。

DatePeriod 保存 years:Int、months:Int、days:Long，各分量可有符号；计算时先把 years/months 合成总月数一次应用，再应用 days。目标月份没有原 day 时夹到月末，例如 2024-01-31 + 1 month 为 2024-02-29。DateTimePeriod 组合 DatePeriod 与 Duration，先日历部分、后精确时长；加减可能不可逆，不将一个 month 换算成固定秒数。DatePeriod/DateTimePeriod 首版只提供分量构造、运算和显式 codec，不另加 period 文本解析 DSL。

日期文本使用 ISO 扩展形式：year 0000..9999 为四位；负年为负号及至少四位绝对值数字；大于 9999 的正年必须有 + 号。月份和日期两位。时间为 HH:mm:ss，可有 1..9 位小数；输出去掉小数尾零，无小数时不写点。LocalDateTime 用 T 连接，OffsetDateTime/Instant 必须带 offset；不接受空白、区域名或 locale 拼写。

日期年份的无符号输入必须恰好四位；显式正号要求数值大于 9999，负号要求非零负年且至少四位绝对值数字，拒绝 -0000。YearMonth 使用同一年份规则及两位月份，形式为 year-MM。DatePeriod 的 years/months/days 缺省为零，按原始分量判等和哈希；本期的 period 运算指应用到日期／日期时间，不另提供 period 间的合并或规范化规则。

无效构造参数与文本分别按 IllegalArgumentException／parseOrNull 返回 None；已合法值的算术或转换超出域抛 ArithmeticException。已验证的不变对象不在每个 getter、比较、序列化时重放日期合法性检查。

LocalTime(hour: Int, minute: Int, second: Int = 0, nanosecond: Int = 0) 的分量只读；fromNanosecondOfDay(Long) 接受 0..<86400000000000，toNanosecondOfDay() 返回同一范围。加减 Duration 在整数运算中按天取余，负极值时长也不能因中间取负而溢出。LocalDateTime(date: LocalDate, time: LocalTime) 另提供 year/monthNumber/dayOfMonth/hour/minute/second/nanosecond 分量构造（秒与纳秒缺省为零），暴露 date/time 与相应只读分量。DateTimePeriod(date: DatePeriod = DatePeriod(), time: Duration = Duration.ZERO) 按两个原始字段判等和哈希；减法与加法一样先日历、后精确时长。

UtcOffset.fromTotalSeconds(Int) 与 fromHoursMinutesSeconds(hours: Int = 0, minutes: Int = 0, seconds: Int = 0) 是受检工厂，提供 ZERO。分量形式要求小时在 -18..18、分钟和秒各在 -59..59，所有非零分量的符号相同，最终总秒数仍须在 ±64800 内；不接受相互抵消的符号或超范围分量。OffsetDateTime(dateTime: LocalDateTime, offset: UtcOffset) 提供 toInstant()、withOffsetSameInstant(offset) 和 withOffsetSameLocal(offset)；构造与保留 local 的替换不要求此值对应的 Instant 在 Instant 域内，显式转换时才检查。

## 9. Codec 组合

复用 core 的 Encodable/Decodable 值与容器协议，不引入运行期反射或新条件 conformance。新增容器与 Result 的 companion 工厂显式接收元素/key/value/error codec；无关用途不要求 codec bound。

既有 Array/MutableArray/ArrayList 的 codec 继续由 core 提供。std 需要 List 接口 codec 时使用普通 top-level `listEncoder/listDecoder` 工厂，返回 Encodable<List<T>>／Decodable<List<T>>；不假定 core List 已有可扩展的 companion，也不使 core codec 依赖 std。listDecoder 返回独立只读快照，listEncoder 可复用公开 encodeList。

List/Deque/Set/PriorityQueue 编码为 unkeyed 序列，Map 编码为 unkeyed 的二元素 `[key,value]` entry 序列，使非 String key 保持类型。LinkedHash 保存迭代顺序，Tree 按 comparator 顺序，Hash 不保证输出顺序。解码时 Map/Set 的重复等价 key/element 为 DecodingException，不静默丢弃数据。Tree/Queue decoder 显式接收 Comparator，比较器不编码。Sequence 必须先显式 materialize，不自动消费潜在无限输入。

Result 使用一个 keyed 字段 success 或 failure，值交给对应 codec，要求恰好一个字段。Uuid、Duration、Instant、LocalDate/Time/DateTime、YearMonth、UtcOffset、OffsetDateTime 采用本文件规定的 canonical String。DatePeriod 为 years/months/days 字段；DateTimePeriod 为 date/time 字段并复用其 codec。解析失败由 codec 转为带当前 path 的 DecodingException，其他异常照常传播。

TimeMark、Clock、TimeSource、Random、Iterator 不提供默认 codec；来源对象、执行状态与 RNG state 不作为可移植数据。JSON 通过它现有的显式 codec 入口消费 std codec，不需要识别 std 类型名或改变格式协议。

## 10. M42 之外

独立 math 包（含数学函数、BigInteger、Decimal）、正则、完整 Unicode 数据与算法、压缩、密码学、更多格式库、持久化不可变集合均另行排期。系统库负责文件/网络/进程/线程、熵、实际 wall/monotonic 取时、sleep/timer、时区数据库和 native resource。M42 不预置空实现或运行期“尚未支持”分支。

## 11. 参考来源

以下资料用于接口命名和成熟算法/格式的核对；本规范明确的 Scoop 差异优先，不承诺复制 Kotlin 的所有重载或平台行为。

- [Kotlin collections](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.collections/) 与 [LinkedHashMap](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.collections/-linked-hash-map/)：集合、工厂和插入顺序命名。
- [Kotlin sequences](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.sequences/)：惰性与终结操作命名。
- [Kotlin Random](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.random/-random/)：nextBits 与采样 API 命名；Scoop 要求显式来源。
- [xoshiro/splitmix 作者资料](https://prng.di.unimi.it/)：具名 PRNG 和 seed 扩展算法。
- [Kotlin time](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.time/)、[Instant](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.time/-instant/) 与 [kotlinx.datetime](https://kotlinlang.org/api/kotlinx-datetime/kotlinx-datetime/kotlinx.datetime/)：时间域和日历类型命名；Scoop 的有限 Duration、POSIX 时间尺度和无系统来源实现是显式差异。
- [Kotlin Result](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin/-result/) 与 [Uuid](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.uuid/-uuid/)：命名参考；Scoop 的 Result 额外保留 E。
- [RFC 4648](https://www.rfc-editor.org/rfc/rfc4648.html)：Base16/Base64 的字母表、padding 与 pad bits。
- [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.html)：UUID 位型与 v4 version/variant。
- [Unicode 17.0 UCD](https://www.unicode.org/Public/17.0.0/ucd/)：PropList 的 White_Space 小表。
