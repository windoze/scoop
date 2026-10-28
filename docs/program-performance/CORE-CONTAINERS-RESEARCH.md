# ArrayList、StringBuilder 与 HashMap 效率调研

2026-10-29，基线 `1de3cd2eb01067d91d1b51fc505e49aac9abbb1d`。

调研覆盖普通库正文、HIR 插值下降、MIR 内联与范围证明、最终 ARM64 机器码，以及相关分配和 GC 合同。本轮调研没有修改生产实现或规范；下面明确区分当时的行为、实测结果和后续建议。ArrayList 与 StringBuilder 的后续实现见 [优化记录](ARRAYLIST-STRINGBUILDER.md)。

结论：ArrayList 的存储表示合理，优先处理追加快路径与扩容初始化的生成质量；StringBuilder 适合将字节缓冲与 primitive 重载一起推进；HashMap 可以采用 Swiss table 式分组探测，但需要同时处理指纹分布、缓存 hash、删除和现有回调语义，不能只替换状态数组。

## 1. 测量范围

本机为 Apple M3 Ultra，macOS 27.0.1；编译器及相关 core/std 从上述源码构建，LLVM 22.1.8，普通 release，runtime 显式使用同一源码版本。已有开发 sysroot 与当前 ABI 不匹配，因此探针通过普通根 overrides 选择 core/std 源码，全部模式共用这两份构建结果和一个 runtime；没有逐项重建或复制 sysroot。

22 个模式各预热一次、测量七次，逐轮反转模式顺序；表中为函数内阶段墙钟中位数，构建、输入准备及最终输出不进入阶段。GC 统计另行运行，不混入主计时。所有进程正常退出，输出校验和一致；列表、map 与字符串模式还执行结果检查。控制字节两种算法的校验和相同。

这是本机基线与局部算法可行性测量，尚未实施完整 Swiss map 或字节缓冲 builder，不能把下列不同工作负载之间的比值当作改造收益。未进行 Linux/x86_64 性能测试或全量功能验收。

## 2. ArrayList

### 2.1 已经合理的部分

[ArrayList](../../sysroot/src/scoop.core/src/array_list.scoop) 使用 `MutableArray<MaybeUninit<T>>`，有效长度单独保存。`MaybeUninit<T>` 与 T 等大小、等对齐，不附加初始化位、Option tag 或逐元素对象。`ArrayList<Long>` 的槽为 8 B，引用为 8 B，接口为 16 B；普通值类型按实际布局内联。

初始容量可指定，默认从零开始，第一次增长为 8，随后倍增。追加摊还 O(1)，插入、删除和 clear 为 O(n)。快照成员已直接捕获 backing 和逻辑长度，不再逐项经过未知 List 接口 getter；这部分已有优化和三目标验证，见 [ArrayList 快照](ARRAYLIST-SNAPSHOTS.md)。

完整 GC 按 backing capacity 扫描引用槽；删除和 clear 必须释放失效引用，不能只减少 elementCount。大量预留但长期低占用的引用列表会保留容量空间及扫描范围。这是当前普通数组的合同，不应为列表另建 initialized-length GC 对象模型。

### 2.2 当前生成代码中的明确开销

**追加快路径仍进入完整扩容函数。** `add` 已在探针的追加循环中内联，但 `ensureRoom` 被 MIR 以 `not-profitable` 保留。实际机器码每次追加都调用它，随后再检查 storage 的索引边界。`ensureRoom` 即使不扩容，也先建立 176 B 栈帧、保存六对寄存器并检查入口 poll；调用者循环还有自己的 poll。问题不在泛型装箱，而在把罕见扩容正文与每次追加的检查放进同一个较大 callable。

**扩容中的条件复制未获得实际可用的分块路径。** 初始化正文是 `index < count ? old[index] : uninit()`，循环上界为新容量。当前范围证明为完整块生成 `nextCapacity <= old.size` 的 guard；真实扩容满足相反关系，因此本探针中虽然存在向量机器码，实际扩容走带逐项 poll/检查的标量路径。不能只凭产物中出现向量指令就判定该负载被优化。

**零初始化重复。** [动态数组分配](../../compiler/codegen/src/function/gc/arrays/dynamic.rs) 已取得清零存储；ArrayList 构造随后又执行 `uninit()` 初始化循环。Long 构造机器码保留了这第二遍写零，完整块每 64 项 poll 一次。扩容则再次写入新容量的空尾部。这也影响 HashStorage 的状态与 wrapper 数组初始化。

**clear 仍有可改进的循环与屏障。** 独立 `ArrayList<Long>.clear` 逐项读取字段、检查 backing 长度并执行 poll；本探针没有变成 memset 或 64 项分块。引用版本写入已知零值后仍标记卡片。[heap_array_element_store](../../compiler/codegen/src/function/gc/writes.rs) 的引用变化 guard 目前仅覆盖 stride 大于 16 B 的单引用元素，8 B 引用槽沿无条件屏障路径。可以优先研究“静态已知全部引用为 null”的通用消除，不必给所有短元素增加动态旧值比较。

### 2.3 本机基线

前三项均包含构造和填充；读取、clear、快照的列表在计时前构造。元素数为 1,048,576，GC 分配列是独立统计进程的总分配，包含少量启动及输出开销。

| 工作负载 | 阶段中位数 | 总分配 |
| --- | ---: | ---: |
| `ArrayList<Long>()`，逐项追加 | 4.424 ms | 16,777,744 B |
| `ArrayList<Long>(n)`，逐项追加 | 2.752 ms | 8,388,768 B |
| `MutableArray<Long>(n) { it }` | 0.520 ms | 8,388,736 B |
| ArrayList 顺序求和 | 0.558 ms | 输入在计时前准备 |
| ArrayList 的独立数组快照 | 0.679 ms | 输入在计时前准备 |
| `ArrayList<Long>.clear()` | 0.520 ms | 无阶段分配 |
| `ArrayList<Cell>.clear()`，各槽引用同一对象 | 1.026 ms | 无阶段分配 |

预留容量相对从零追加在此负载少用约 38% 时间、约一半分配；与直接数组初始化的差距仍明显。数组构造的语义和列表追加不同，此对照只定位额外成本，不是可以直接替换所有列表的证据。

在 65,536 项列表上做 64 次头部插入/移除对，移动约 8,388,608 个槽，耗时 4.372 ms；复杂度来自必要的后缀移动。后续优化应关注连续移动的生成质量，不能把头部队列操作变成 ArrayList 的常数时间承诺。

### 2.4 建议

1. 将无需扩容的容量检查留在小正文中，真实分配/复制进入单独的增长 helper。检查新的内联决策和追加循环，不直接提高全局内联阈值。
2. 复用已分配为零的事实，识别无副作用的零 initializer；对条件复制采用路径相关范围证明，或在普通库正文中把有效前缀复制与空尾部分开。当前 [范围事实](../../compiler/mir-lower/src/optimization/poll_chunks/checks/facts.rs) 对整个循环上界产生的 guard 是具体切入点。
3. 让稳定 backing、稳定长度和已知 null 写入进入已有范围、分块及屏障优化。连续复制/移动可复用普通数组能力；重叠移动需要 memmove 语义，引用复制需要完整 GC roots 和必要的范围屏障，不能给任意 initializer 或任意 T 直接套 memcpy。
4. 在本来已知结果数量的生产调用点使用现有 initialCapacity。对一般 Iterable 不提前遍历来估数，也不为预留容量增加用户 getter 的无关副作用。

## 3. StringBuilder 与 primitive 重载

### 3.1 当前成本在哪里

[StringBuilder](../../sysroot/src/scoop.core/src/string_builder.scoop) 保存 `ArrayList<String>`。`append(String)` 只保存引用；泛型 `append<T : ToString>` 立即转换再保存。泛型已经单态化，具体 primitive 不需要统一装箱到 ToString 接口。

`join_parts`（基线中的 `runtime/src/string_parts.c`） 先 checked 累计字节数，再一次分配结果并逐段 memcpy；分配后重新从 backing 取引用，单片段直接返回原 String。它没有逐次 `result = result + part`，一次完整构建为 O(partCount + bytes)。已有大 String 只在最终汇总时复制，是此设计的优势。

主要问题是大量细碎片段、primitive 转换产生的临时 String，以及重复快照：

- 整数即使只有几个十进制字符，也有 String 对象头及对齐成本，并占一个引用槽；片段需要保活到最终拼接或 clear。
- Char 同样经过临时 String。Boolean 例外：当前 `toString()` 返回固定的 `"true"` / `"false"`，没有逐次转换 String 分配。
- 每次多片段 `toString()` 都重新扫描和复制；未修改时重复调用可考虑缓存不可变结果，现有规范不要求每次结果身份不同。
- 一边追加、一边保存每个前缀快照，本来就会输出 O(n²) 总字节；只换成字节缓冲也消除不了这项必要输出量。

32,768 次 append 后做一次 toString 的基线如下，各行输入不同，不能直接相减当作格式化或重载的净收益：

| 追加内容 | 阶段中位数 | 最终 UTF-8 字节 | 总分配 |
| --- | ---: | ---: | ---: |
| 重复已有 8 B String | 0.282 ms | 262,144 | 786,888 B |
| 递增 Long：0～32,767 | 1.249 ms | 152,730 | 1,726,056 B |
| 重复 Char：`雪` | 0.862 ms | 98,304 | 1,671,624 B |
| 交替 Boolean | 0.334 ms | 147,456 | 672,200 B |

此处 Long 与 Char 的每个转换 String 均占 32 B，32,768 项对应 1,048,576 B 临时 String 分配，另有 parts 容量与最终结果。Byte buffer 加真正的 primitive 格式化重载能去掉这部分临时对象；新 buffer 本身的分配和复制仍须计入最终对照。

1024 个片段共 8 KiB，内容不变地调用 toString 2048 次，耗时 5.707 ms、总分配 16,843,064 B。逐次追加一个字节并生成快照，4096 次耗时 17.334 ms；后者同时包含越来越长的 parts 扫描和输出复制。

### 3.2 primitive 重载有用，但需要真正改变数据路径

只增加 `append(Int)` 并在里面写 `parts.add(value.toString())`，不能消除主要成本。建议沿 [现有后续设计](STRING-BUILDER-INTERPOLATION.md) 使用普通 `MutableArray<UInt8>`、byteCount 和几何增长，随后增加普通 primitive 重载：

| 类型 | 建议的数据路径 |
| --- | --- |
| Long、ULong | 复用现有十进制 writer，格式化一次到最多 20 B 的 GC-free 栈缓冲，再复制有效区间到 builder；省掉临时 String。 |
| Int、UInt、8/16 位整数 | 通过精确的有符号/无符号扩展复用上项。Scoop 没有隐式整数转换，小整数要有对应重载或明确的转发。 |
| Char | 复用 UTF-8 width/word，直接写实际 1～4 B，省掉临时 String。 |
| Boolean | 写固定文本。当前转换本来不分配，主要收益是减少 parts 管理及后续引用扫描。 |
| Float、Double | 后续复用现有 Ryu 和 Scoop 浮点文本规则；先保留泛型转换，不能另外实现一套结果不同的格式化。 |
| 用户 ToString、显式泛型调用 | 保持普通转换语义；用户转换恰好执行一次，转换期间的重入及异常按现有顺序处理。 |

[integer_decimal.scoop](../../sysroot/src/scoop.core/src/integer_decimal.scoop) 已经使用 20 B 栈缓冲与 digit-pair writer，不需要为 builder 重写整数转换算法。首版还有一次很小的栈缓冲复制，应与“直接写入最终 String”区分。

字节缓冲使 clear 可仅重置长度，消除对 parts 的引用保活，toString 直接复制有效前缀。它也增加已有大 String 的一次 append 复制，扩容还要复制 buffer；单片段失去当前直接返回原 String 的优势，clear 后可能保留历史峰值字节容量。正式对照必须包括这些负载，不能预设全场景获益。

### 3.3 编译器需要配套什么

**托管对象之间的字节复制。** append 需要 String → 已分配字节数组，增长需要数组前缀 → 新数组，toString 需要有效前缀 → 新 String。扩展已有 typed StringCopy/数组复制能力即可：分配前保活输入，分配后恢复引用，再形成字节地址；已分配对象间的复制在无 safepoint 区间完成，并登记真实写副作用。不能每次 append 都走有闭包和 pin 成本的 `withDataPointer`，也不能让可移动数组的裸指针跨分配存活。

**f-string 的两条路径要分开处理。** [普通插值下降](../../compiler/hir-lower/src/expr/interpolation.rs) 对超过八个有效片段使用真实 StringBuilder，并通过普通重载选择解析 append；primitive 重载可以自然覆盖这条路径。

[短插值下降](../../compiler/hir-lower/src/expr/interpolation/short.rs) 则直接调用 ToString，再组成 StringConcat 链。release 已融合最终结果分配；给 builder 加重载不会优化这条路径。后续可让已静态确定的真实 core primitive 格式化结果以固定大小值片段进入现有拼接融合，复用 core writer；保持表达式/转换顺序、跨挂起的值保存、用户 ToString 和所有源引用的保活。不能为编译器另写十进制/Ryu 算法，也不应为了使用新重载把短插值重新改回 builder。

字节缓冲落地前需同步语言规范 §11.6、实现规范 §2.15 和 runtime §2.4/§6 中固定 parts 存储的描述。primitive 重载、复制操作的 typed 合同与旧 join_parts 退役范围已有对应设计，实施时以最终所需的变化为准。

## 4. HashMap 与 Swiss table

### 4.1 当前已经是平坦存储

[HashStorage](../../sysroot/src/scoop.std/src/collections/hash_storage.scoop) 采用开放寻址和线性探测，状态 0/1/2 表示空/占用/删除，75% occupied 时清理或增长。它没有每个 entry 一个链表节点：K、V、缓存 Long hash 内联在 `MaybeUninit<HashEntry<K,V>>` 中。

明确的额外成本包括：

- 状态数组用 Int，每槽 4 B，探测逐槽进行。
- `start` 使用动态 `% capacity`；ARM64 实际保留 `sdiv + msub` 和可抛出的除零后备，没有自动变成掩码。采用 mask 前须让所有容量路径保持二次幂；当前接近上界的分支会选择 `INT64_MAX`，不能直接机械替换 `%`。
- 未命中插入先 [locate](../../sysroot/src/scoop.std/src/collections/hash_map.scoop) 再 vacant，从起点探测两次；中间若没有 rehash，可以复用第一次找到的空位/墓碑。
- 普通探测循环逐槽检查状态数组和元素数组边界，并执行 loop poll；基线没有把整段探测变成 SIMD。
- Long key 的实际路径仍有 hash helper、ModificationCounter.check 和 start 调用；hash/equals 后的版本检查也保留。编译器可研究对真实、可见且无写副作用正文的证明与内联，但 NoGC 不等于无副作用，不能删除任意用户回调的重入检查。

同时，LLVM 已经把 `storage.read(index)` 的完整 HashEntry 值投影简化：本机 locate 先读 hash，只在 hash 命中后读 key，没有在每个探测槽都复制 value。不能把源码的 struct 临时值直接算成完整 K/V 复制成本。

本机基线中，Long map 存 65,536 项、容量 131,072，查询索引使用奇数乘法置换；各查询模式执行 1,048,576 次。String 查询提前构造 `key-<index>`，复用存入 map 的同一批 String，因此没有计入重新构造 key 或不同对象同内容比较的成本。

| 操作 | 中位 ns/次 |
| --- | ---: |
| 从空表插入 65,536 个 Long key，含增长 | 44.94 |
| Long 命中查询 | 11.80 |
| Long 未命中查询 | 16.31 |
| Long 替换既有 value | 13.57 |
| Long 移除后重新插入同一 key，一对操作 | 40.75 |
| String 命中查询，复用 key 引用 | 22.31 |

这些是约 50% 占用率的基线，不代替高负载率、严重碰撞和积累墓碑的测量。

### 4.2 可行的实现路线

Swiss table 的关键是独立、紧凑的 control bytes：full 槽保存 7-bit H2 指纹，特殊值表示 empty/deleted；H1 定位首组，一次筛选一组指纹，只对候选槽读取 entry 并做真正的 hash/equals 比较。empty 终止查找，deleted 不终止；可以使用三角数步长遍历组。

**Scoop 已有的整数能力足够做首个分组实现。** 一个 ULong 可打包 8 个控制字节，通过 xor、减法、取反和 mask 同时筛选。使用普通整字数组避免在热路径借用/pin Byte 数组。若从任意字节槽开始探测，需要处理跨字组读取/环绕；若选择对齐组，则应明确其首组与探测规则。它们不能在实现中含糊混用。

本轮另写了一个普通 Scoop 控制字节探针：4096 个组，每组 8 个 tag，执行 4,194,304 次变化的组/指纹查询，只返回该组是否存在匹配。逐字节版本为 **5.704 ns/组**，ULong SWAR 为 **0.894 ns/组**。机器码确认 SWAR helper 已内联，一次整字读取后为整数运算，没有 FFI 或 pin；逐字节版本则保留八次循环、边界检查和 poll。

这个约 6.4 倍的局部差异包含“表达为整字算法”与“绕开当前小数组循环生成限制”两方面，**不是 HashMap 将快 6.4 倍的预测**。探针没有实现候选槽枚举、完整查找、插入、删除、rehash 或用户回调。

hashbrown 0.16.1 可作为成熟参考：x86_64/SSE2 用 16 字节组，little-endian AArch64/NEON 用 8 字节组，还有按机器字宽度的 portable 实现。当前 M3 不应机械照抄 x86 的 16 槽策略。Scoop 暂无现成的 trailing-zero bit-count 公开操作；候选位枚举若成为热点，可以沿现有整数 intrinsic 通道补标准的 `countTrailingZeroBits`，再衡量专门的 group compare/mask 是否有必要。无需先建设一套通用 SIMD 类型系统。

portable match 的减法技巧允许少量额外候选位，不能把指纹匹配当成 key 相等；最终 hash/equals 必须保留。完整组的边界读取、尾部 padding/clone、零 mask 与特殊控制值都应按选定算法验证。

### 4.3 指纹质量是实际问题

Abseil 的设计明确要求 hash 在整个 bit 空间有良好分布。Scoop 整数 hash 已有 avalanche mixing，但 String 使用 FNV-1a。按当前精确公式计算 `key-0`～`key-255`：

| H2 取最高 7 位 | 不同 H2 数 | 最多 key 的同一 H2 |
| --- | ---: | ---: |
| 直接使用当前 String hash | 4 | 100 |
| 再应用现有整数 hash 的混合公式 | 109 | 6 |

这不是完整表的碰撞率，却足以说明不能直接把最高 7 位当作近似均匀指纹。建议比较容器内一次私有 mixing，保持公开 Hash 接口和 String.hash 结果；需要同时测小整数、短字符串、长字符串及用户自定义 hash 的计算成本和探测收益。相等 key 相同 hash 的要求保持，恒定 hash 等极端输入仍可能退化。

### 4.4 首版建议保留缓存的完整 hash

当前 rehash 只搬运已经缓存的 hash，不再调用用户 `hash()`。这使增长/重排阶段没有用户回调，吻合异常和重入合同。若为了节省 8 B 删除缓存，就要重新处理增长中的用户异常、重入修改、旧表有效性及重复 hash 成本。

首版可以保留 `HashEntry<K,V>.hash`，只在 control 中加 H2 做前置筛选。以 Long/Long 为例，不含对象头、尾部和增长空闲量：

| 存储选择 | 每个容量槽 |
| --- | ---: |
| 当前：24 B entry + 4 B state | 28 B |
| 保留完整 hash：24 B entry + 1 B control | 25 B |
| 去掉缓存 hash：16 B K/V + 1 B control | 17 B |

首版同容量节省约 10.7%，不应宣传成“整个条目只有 1 B 开销”。若更高负载率通过验收，可再比较 7/8 与当前 3/4 的空间/探测取舍；容量按二次幂跳变，实际内存收益取决于元素数所在区间。

还要保持现有普通语义：替换 value 保留原 key 对象；替换不让 iterator 结构失效；用户 hash/equals 抛出前不提交结构变化；回调重入后检查已有修改状态；删除清空整个退出使用的 wrapper，不能只改 control byte 留住引用。

[HashSet](../../sysroot/src/scoop.std/src/collections/hash_set.scoop) 直接复用 HashMap；[LinkedHashMap](../../sysroot/src/scoop.std/src/collections/linked_map.scoop) 的 positions 也依赖 HashMap 的内部 locate/slot 接口。分组改造需要同步这两个实际消费者，并保留 LinkedHashMap 的插入顺序。K/V 继续用普通托管数组和 typed GC scan；C/Rust 的通用原生 hashmap 不能直接接管 Scoop 的引用及回调合同。

## 5. 推荐推进顺序

| 顺序 | 工作 | 库与编译器的分工 |
| --- | --- | --- |
| 1 | ArrayList 追加快路径、零初始化与真实增长复制 | 库拆小热检查；MIR/后端复用零值事实、条件范围和现有分块，解决已确认的未命中。 |
| 2 | StringBuilder 字节缓冲 + 整数/Char 等重载 | 库复用 formatter；编译器补实际需要的 typed 字节复制与有效前缀来源，保持 GC 恢复和写副作用。 |
| 3 | HashMap 分组原型与完整迁移 | 先普通 SWAR、合适的 H2 mixing、缓存完整 hash；结合 mask 定位和单次探测，实测后选择组宽与负载率。 |
| 4 | 短 f-string 的 primitive 临时 String 消除 | 将已知 core formatter 的小值结果接入已有拼接融合，单独核对复杂度、代码增长和求值顺序。 |

步骤 1 同时惠及当前 builder 和其他列表消费者；HashMap 使用自己的 storage，不会仅因 ArrayList 变快而直接获益。其初始化、数组访问和屏障可以复用步骤 1 中真正通用的编译器改进。

每项实施前按实际变动同步 spec。普通行为放在所属 Cone 测试中；少量编译器专测检查实际热路径、边界诊断、跨 Cone 以及包含引用/接口/Option 的移动和分代 GC。性能覆盖空/短/长输入、已知/未知容量、重复 clear/快照、高负载/墓碑、自定义回调及大值；只对改动涉及的模式做三目标对照，不把微基准耗时设为正确性阈值。

## 6. 证据与复现资料

探针源码、构建缓存、最终 MIR/LIR、优化决策、机器码、原始样本及独立 GC 输出都在仓库外：

`/Volumes/Data/home/chenxu/scoop-performance-results/2026-10-29/core-containers-research/`

主要入口为 `probe/Cone.toml`、`probe/src/main.scoop`、`timing-summary.json`、`timings.json`、`gc-<mode>.stderr`、`array-list-inline-decision.json`、`h2-distribution.json` 和 `disassembly-named.txt`。C 计时 helper 复用仓库的 `tests/benchmarks/interfaces/phases.c`，仅由探针显式链接。所有 Scoop 源码先通过正式 formatter 和格式检查，再编译运行；编译器/PM/linker 的 release 构建通过。没有新增生产诊断入口、依赖或优化开关。

外部算法参考：

- [Abseil Swiss Tables Design Notes](https://abseil.io/about/design/swisstables)，本次读取设计正文。
- [hashbrown 0.16.1 group 选择](https://github.com/rust-lang/hashbrown/blob/v0.16.1/src/control/group/mod.rs)、[portable group](https://github.com/rust-lang/hashbrown/blob/v0.16.1/src/control/group/generic.rs)、[AArch64 group](https://github.com/rust-lang/hashbrown/blob/v0.16.1/src/control/group/neon.rs)、[raw table](https://github.com/rust-lang/hashbrown/blob/v0.16.1/src/raw/mod.rs)，本次核对本机 Cargo 缓存中的相同版本源码，未宣称它是当前最新版本。
