# M42 实施进度

目标范围以 [设计](DESIGN.md) 和 [标准基础库规范](../specs/SCOOP-STDLIB-SPEC.md) 为准。本文记录实际完成的增量，不代表 M42 整体已验收。

## M42-1：基础接口、比较器与列表

日期：2026-10-13。新增普通 `scoop.std` Cone，唯一直接依赖为 core。

- core 增加普通 Collection 接口，List 继承该接口；Array、MutableArray、ArrayList 自动沿原有关系取得 Collection 能力。保留 MutableList.add 返回 Unit 与原有 live iterator 行为。
- collections 再发布原集合实体，提供 isEmpty/isNotEmpty、emptyList/listOf/mutableListOf/arrayListOf、toList/toMutableList。ListSnapshot 只有只读接口，使用独立 backing；浅复制保留元素的值／引用语义。
- comparisons 提供 Comparator、comparatorOf、naturalOrder/reverseOrder、compareBy、thenBy、reversed。反向比较交换实参，避免对 Long 最小值取负；次级 selector 只在主比较相等时调用。
- 修正文档对 core package 的描述：当前 core 源码位于默认根 package，public import 使用真实根 binding，不从 Cone 名合成限定名称。显式 core 版本声明使用产物搜索，fixture 按现有 JSON 用例先构建 core 产物。

实现全部为普通 Scoop 源码，没有 compiler、driver/linker、IR/meta 或 runtime 改动。尚未发现需修复的底层实现缺陷。

### 验证与证据

使用重建后的配套 `scoop`、`scoopc`、`scoop-link`。本机目标为 `aarch64-apple-darwin`，工作目录为 `target/m42/foundation`；所有命令的 TMPDIR 都显式设为仓库内 `target/m42/scratch`。Scoop 源码按现有格式编写，各批测试前运行 `git diff --check`；未修改 Rust/C/Python 实现，无无关语言的 lint 或全量测试。

新增 `tests/fixtures/m42-foundation` 七个正式 fixture：Collection 的 core-only 消费；列表工厂、快照、值元素与嵌套 Option；比较器组合与回调异常；独立 core/std 产物构建、删除源码后的下游编译与 artifact-only link；缺失 std 依赖、Compare bound、不变泛型的精确 negative 诊断。四个运行用例均覆盖普通运行和 moving GC。

本机已覆盖通过 7 个新 fixture 和 15 个受影响的原有 fixture，没有未闭合失败。为减少重复测试，三个报告合并记录本批证据：

| 报告（仓库 target/m42 下） | 有效通过项 |
| --- | --- |
| foundation-components-report.json | lists、comparisons 两项；该轮其他失败随后修正 |
| foundation-regression-report.json | Collection、artifact-only 两项和 15 项 core 回归；三个新负例当时尚未填入诊断期望 |
| foundation-negative-report.json | 填入已核对的精确诊断后，三个新负例全部通过 |

执行入口均为 `python3 tests/run_fixtures.py --work-dir target/m42/foundation`。第一组使用 `--suite tests/fixtures/m42-foundation --jobs 4`；回归组使用 `--jobs 4`，选择五个此前待闭合的新例，以及 `m26-lists-*`、`m26-list-reject-*`、`m26-array-construction-basic`、`m26-array-construction-cross-cone`、`m26-interpolation-basic`、`m26-interpolation-aggregate-order`、`m29-container-decoding-sequences`。最后使用本组 suite 和三个负例的 exact name filter，`--jobs 3`，结果 3/3 通过。既有回归覆盖 List/Array 的接口、迭代、GC、跨 Cone、错误诊断，以及 StringBuilder 插值和 sequence codec。

本批不宣称 Linux 或整个 M42 已验收；GNU/musl 在后续本组完整定向验收中执行。

### 构建目录维护

开始实施前删除旧测试可执行文件及旧 incremental 缓存，保留配套 CLI，target 从约 39 GB 降至 33 GB。重建当前 CLI 后继续保留必要编译缓存；后续在无运行中构建时清理已完成 fixture 的临时产物，保留报告。

## M42-2：无序 Map／Set

日期：2026-10-13。新增 Map/MutableMap、Set/MutableSet、Map.Entry、HashMap/HashSet、hashMapOf/hashSetOf，以及 NoSuchElementException/ConcurrentModificationException。Map 提供 keys/values/entries 的只读 live view、缺失处理和 containsValue 扩展；键和值的 Option 层级保持完整。Set 复用 HashMap 的不替换插入，重复 add 不产生修改。

哈希存储使用线性探测和三态槽，entry 保存完整缓存 hash 与 typed key/value；rehash 复用 hash，remove/clear 重置 managed 槽。iterator 引用容器，不持有旧 backing 快照。普通写入与结构修改分开计数：替换 value 不使 iterator 失效，回调重入替换仍会被外层操作检测；计数回绕时更换局部 epoch 对象。实现文件按协议、views、存储、map、set、修改检测拆分，最大文件 127 行。

本批仍无底层宿主实现改动。算法资料与许可核对见设计 3.2。

### 验证与证据

先运行 `git diff --check`，再执行：

```sh
TMPDIR="$PWD/target/m42/scratch" python3 tests/run_fixtures.py \
  --suite tests/fixtures/m42-hash \
  --work-dir target/m42/foundation --jobs 4
```

本机 `aarch64-apple-darwin` 最终 7/7 fixture、24 个进程通过，报告另存为 `target/m42/hash-report.json`。五个运行用例均包含普通和 moving GC 执行，两个 negative 精确验证缺少 Hash／Equals bound 的位置与信息。覆盖同 hash 碰撞、跨删除槽查找、增长／墓碑清理、原 key 保留、live view、Entry 独立值、永久耗尽与失效 iterator、hash/equals 异常及重入、producer 语义、嵌套 Option、零大小 key、命名 set、引用释放，以及与独立数组模型逐次对照的 600 次操作。

复用同一工作目录时，切换 suite 前先清理已完成的 `cases` 子目录，避免旧测试留下的显式 std 产物优先于新源码；保留共享编译缓存和具名报告。源码变化时同步 runner 的 sysroot 副本。

## M42-3：插入顺序集合、快照与批量操作

日期：2026-10-13。新增 LinkedHashMap/LinkedHashSet、linkedMapOf/linkedSetOf，以及默认 mapOf/setOf、mutableMapOf/mutableSetOf、emptyMap/emptySet。Map 和 tuple Iterable 均有独立的 toMap/toMutableMap，Iterable 有 toSet/toMutableSet；只读快照使用不实现 Mutable 接口的独立包装。新增 List/Set 的 addAll/removeAll/retainAll、Map.putAll、union/intersect/subtract。

LinkedHashMap 复用原哈希索引和修改计数，以数组槽中的双向链接保持顺序；替换保留最早 key 与位置，删除重插移到末尾。每次查找后提交不再调用用户方法。HashMap 增加的内部提交函数同时由原 public 操作与 LinkedHashMap 实际使用，没有新增底层实现。

规范先修正 MutableSet.retainAll：它需要目标自己的等价关系，因此成为接口的普通成员，不强加 Equals/Hash bound 或引入泛型虚方法。Hash/Linked 实现按相同哈希等价规则建立 membership，自定义实现和后续 Sorted 实现分别保留其规则。批量输入先取快照；之后逐元素提交，失败保留成功前缀。

### 验证与证据

本机 `aarch64-apple-darwin` 新增 9 个 linked fixture，连同受内部提交和 MutableSet 接口调整影响的 7 个 hash fixture 已覆盖通过。普通运行用例同时执行 moving GC；本批无 Rust/C/Python 实现变更，测试前运行 `git diff --check`。

- `target/m42/linked-components-report.json`：`--filter 'm42-linked-*' --filter 'm42-hash-*' --jobs 4`，13 项通过；两个只读 negative 的预期诊断随后填入。覆盖增长顺序、原 key 保留、删除重插与空槽复用、600 次独立模型对照、异常／重入、live views、快照、批量自引用、成功前缀、值类型与引用释放。
- `target/m42/linked-final-report.json`：本组 suite 中 exact filter 选择 artifacts、bulk 和 readonly 两项，4/4 通过。补齐只读 Map/Set 的精确拒绝诊断、通过 MutableSet 接口使用自定义等价规则的 retainAll，以及移除源码后的 std 产物消费和独立链接运行。

两轮均使用 `TMPDIR="$PWD/target/m42/scratch" python3 tests/run_fixtures.py --work-dir target/m42/foundation`。仍没有发现需要修复的底层宿主实现缺陷。M42-4 至 M42-12 和 Linux GNU/musl 定向验收尚未完成。

## M42-4 前置修复：开放结构比较的展开来源

日期：2026-10-13。Tree 的 AVL 结构 fixture 首次在普通库内具体化多个泛型方法，产物成功生成后，下游读取因 HIR expression 的 evaluation context 属于另一个方法而失败。已提取只依赖 core 的最小复现：同一泛型 class 的两个方法分别比较 `T?`，在 provider 中具体化这两个方法，再让 consumer 加载 provider 产物。旧编译器在 consumer 构建时出现相同的 CallSites/Evaluation/Context 错误；与 Collection 接口无关。

原因是按类型复用开放派生比较正文后，内联复制仍保留第一次派生时的来源。修复 HIR lowering，让每次内联及嵌套比较使用本次操作的完整 definition/evaluation origin 与源码 span；默认参数继续通过原有复制路径传播求值来源。未放宽产物校验，未改变 callable 身份、产物格式、runtime ABI 或库算法。实现规范 2.2 明确这一既有来源要求。

已完成 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，随后 `cargo test -p scoop-hir-lower equality`（33 项通过）、`cargo test -p scoop-hir-lower executable_type_sites`（4 项通过，两组包含同一个新回归），并重建三个配套 CLI。新增 HIR 回归检查每个实际依赖调用与类型使用的来源；独立正式 fixture 覆盖同一 class 的重复 Option 比较、默认参数、provider 内具体化与下游继续具体化、移除源码后的产物消费及 moving GC。

本次实际修改了编译器，因此最终验证范围按设计 7.1 增加该问题的独立回归和受影响的相等／默认参数用例。修复仅涉及 lowering 的来源信息，不据此扩大到无关 runtime、ABI 或完整仓库测试。

本机 CLI 验证证据：

- `target/m42/equality-regressions-report.json`：10 个选择项中的 9 项通过；包括 `evaluation-order-derived-binding`、`generic-equality-defaults`、`source-defaults-derived-generic-defaults`、`source-defaults-derived-default-combinations`、`source-defaults-derived-default-order`、`m33-equality-derived-nominal-artifacts`、`m33-equality-derived-field-semantics`、`m42-tree-structure` 和 `m42-tree-artifacts`。原有 fixture 的 debug/release 变体照常执行。新增回归中的嵌套 tuple 情况暴露下述另一个缺陷，随后单独提取。
- `target/m42/equality-origins-report.json`：来源问题的独立 fixture 最终 1/1、6 个进程通过；命令为 `TMPDIR="$PWD/target/m42/scratch" python3 tests/run_fixtures.py --suite tests/fixtures/m42-regressions/derived-equality-origins --work-dir target/m42/foundation --jobs 1`。拆分独立问题后重新运行格式化、全工作区 clippy 和新增 HIR 单测，均通过。

来源问题的修复已通过本机定向验收；Linux 留在后续本组定向验证中。当前 Tree 实现尚未提交，M42-4 不标记完成。

## M42-4 前置修复：嵌套 enum 的派生比较

普通 `fun <T : Equals<T>> same(left: T?, right: T?): Boolean = (left, left) == (right, right)`，以 Int 调用，在 MIR 验证阶段被拒绝：VariantPayloadProject 缺少同一 stable value 上对应 VariantTest 的 true-edge 支配。最小复现为独立 core-only 程序，日志为 `target/m42/equality-repro/nested.stderr`。它不需要两个方法或跨 Cone 复用即可触发，与上述来源问题独立。

根因是开放派生比较重复展开值字段投影，没有让 tag 检查与后续 payload 使用同一个已保存的 enum 值。MIR lowering 现在在短路条件实际到达时保存嵌套 enum 字段，当前短路分支内对应的 test 与 payload 使用同一 immutable local；嵌套 payload 的取值保留在外层成功分支内，离开分支即结束复用。复用只匹配从不可变 local 出发的 typed value 字段／payload 路径，不合并 class 字段、调用或任意相同文本的表达式。MIR 与 codegen 的原有支配校验不变，没有产物格式、runtime ABI 或 std workaround 改动。

独立 `m42-regression-nested-derived-equality` 覆盖 tuple、struct、嵌套 Option、不同字段与不同 payload，以及 Int/String 的 debug/release 与 moving GC。已执行格式化、全工作区 clippy 和 `cargo test -p scoop-mir-lower`，118 项通过。重建三个 CLI 后，正式定向回归 4/4、6 个变体、30 个进程通过：新增嵌套比较回归、此前的展开来源产物回归、`generic-equality-defaults` 和 `m33-equality-derived-field-semantics`；报告为 `target/m42/nested-equality-report.json`。命令使用 `--filter` 选择这四项，`--work-dir target/m42/foundation --jobs 4`，没有运行无关完整套件。Linux 留待本组统一定向验证。

## M42-4 完成：B-tree SortedMap／SortedSet

日期：2026-10-13。按用户要求，TreeMap／TreeSet 使用 B-tree，替换未提交的 AVL 实现。非根节点保存 7..15 个键值，条目内联存储于普通 typed array，叶节点不分配子数组；节点内二分查找，比较完成后才进行分裂、借位、合并或根收缩。TreeSet 复用 TreeMap，不增加 compiler/runtime helper。算法与容量说明见设计 3.2。

交付 SortedMap／MutableSortedMap、SortedSet／MutableSortedSet、自然／显式 Comparator 工厂、边界查询、保留 Comparator 的只读转换与范围快照。Comparator 等价覆盖时保留最早 key；value 替换不使 iterator 失效，结构修改使其失效，耗尽后不复活。删除／合并／clear 清空失效条目及子引用，即使 iterator 仍持有旧节点也释放已删除 payload。

本机正式验收：`TMPDIR="$PWD/target/m42/scratch" python3 tests/run_fixtures.py --suite tests/fixtures/m42-tree --work-dir target/m42/foundation --jobs 4`，9/9 项、9 个变体、35 个进程通过；报告为 `target/m42/btree-report.json`。运行前完成 `git diff --check` 和普通 std Cone 构建。

- 独立结构 probe 在测试用的 std 副本中运行：升序、降序和置换序列的逐次插删，每步检查占用范围、有序性、所有叶深相同、parent/槽位一致性、无重复节点与闲置 child 清空；256 项形成至少三层树，另以 2048 项验证插删的比较次数上界。正常及 moving GC 均通过。
- 功能与组合覆盖随机模型对照、lower/floor/ceiling/higher、范围端点和逆序比较器、NaN／正负零、无 Hash/Equals 的 key、Comparator 异常及重入。满节点插入遇到比较异常时不提前分裂，已有 iterator 仍有效。
- 带 managed 引用的较大 struct 经节点移动仍保持字段内容；持有失效 iterator 时，单节点 clear／remove 的 payload 可回收。另覆盖 readonly 与缺失 Compare bound 的编译错误，以及删除 core/std/消费者源码后的 artifact-only link。

本批库代码全部为普通 Scoop。先行修复的两个独立编译器缺陷分别见上文；不因它们扩大到无关完整测试。M42-5 至 M42-12 与 Linux GNU/musl 本组定向验收仍待完成。

## M42-5 进行中：ArrayDeque 已完成

日期：2026-10-13。ArrayDeque 以普通 typed 环形数组实现两端增删、首尾 OrNull、clear 与头尾顺序迭代。增长只复制有效区间；退出有效范围的 MaybeUninit 槽清零。iterator 持有容器而非旧 backing，结构修改失效、空操作保持有效、首次 None 后永久耗尽。与 PriorityQueue 共用的 ElementBuffer 只封装实际使用的 typed 槽读写和增长容量计算，没有新增集合接口或 native 能力。

本机通过 `m42-queue-deque-basic` 与 `m42-queue-deque-model`，正常与 moving GC 均执行：包括嵌套 Option、空队列异常、两端交错操作、1500 步模型对照、多轮环形绕回、扩容，以及与 HashSet 组合的广度优先遍历。证据保留于 `target/m42/queues-functional-report.json`；同轮 PriorityQueue 与组合 GC 验证见后续记录。先完成普通源码构建与 `git diff --check`，无底层宿主改动，不运行全仓库套件。

### PriorityQueue 已完成

普通数组二叉堆支持显式 Comparator、自然顺序／显式比较器工厂、peek、removeFirst 及 OrNull、clear、size 和失效检测 iterator。插入先查找目标祖先，删除先计算下降路径，全部比较成功后才提交搬移；路径由二叉堆索引保存，无额外路径数组。每次用户比较返回后检查修改计数，异常不提交本次调整，重入修改保留副作用并拒绝继续使用旧状态。有效槽外及时清零；迭代器不保持旧 backing。

本批正式验证共 7 个 fixture 均通过，全部使用 `TMPDIR="$PWD/target/m42/scratch"`、`--work-dir target/m42/foundation`，按以下实际改动范围运行：

- `--suite tests/fixtures/m42-queues --jobs 4`：当时的 5 个功能／模型／GC fixture 全通过，5 个变体、20 个进程。`target/m42/queues-functional-report.json`。堆以 2400 步有重复值模型插删及最终有序排空验证优先级和比较次数上界；覆盖自定义无 Hash/Equals/Compare 的元素、嵌套 Option、满容量时比较异常、删除比较异常、重入 add/clear 与 iterator 规则。
- 同 suite 中以 `--filter` 选择 `m42-queue-artifacts` 和 `m42-queue-natural-bound`，`--jobs 2`：2/2、8 个进程通过。`target/m42/queues-artifacts-report.json`。负例锁定自然工厂缺少 Compare 的消息及源码位置；组合 artifact-only 用例移除 core/std/消费者源码后链接运行，含 managed struct、Comparator closure、Deque→PriorityQueue 流水线与 moving GC。
- 将 GC 用例加强为连续移除 40 个含引用与 tuple 字段的 struct、逐项对照 Deque 与 Heap 后，仅重跑 `--suite tests/fixtures/m42-queues/gc --jobs 1`：1/1、4 个进程通过。`target/m42/queues-gc-report.json`。删除及 clear 分别确认 payload 回收，扩容前创建的失效 iterator 仍被持有；正常和 moving GC 均通过。

没有 compiler/runtime 修改。已清理各轮 case 临时产物并保留报告与共享 cache；未运行完整仓库测试。M42-5 的急切集合算法、稳定排序和二分尚待完成，后续批次及 Linux GNU/musl 本组定向验收保持原范围。

### 急切转换、遍历、窗口与目的地操作已完成

日期：2026-10-13。交付 map/filter/flatMap/partition 系列、遍历与归约、first/last/single/elementAt 与 OrNull、contains/indexOf、截取与反转、zip/withIndex、chunked/windowed、List/Set 目的地重载及 joinToString。所有操作遵守单次输入遍历；短路后不读取无关后缀，take(0) 与负数 elementAtOrNull 不取得 iterator。目的地操作先快照输入，flatMapTo 还逐次快照内层来源，支持目标别名并保留异常前的成功前缀。新建结果使用只读浅快照。

本机 `m42-algorithms-transforms`、`terminals`、`windows`、`join` 均已通过普通运行与 moving GC；首轮报告保留为 `target/m42/algorithms-first-report.json`。窗口模型覆盖长度 0..11、窗口 1..5、步长 1..7、两种 partialWindows 共 840 组；其他用例覆盖嵌套 Option、短路读取和回调次数、空输入、非法数量、目的地自身／内层别名、prefix 提交和有限 join 对无限输入的停止。新增 `m42-algorithms-string-bound` 拒绝无 ToString 又未提供 transform 的调用，精确锁定源码位置及诊断。后续定向报告与整个 M42-5 收尾记录见下文。

先完成普通 std Cone 构建、源码风格检查与 `git diff --check`。这批只修改 Scoop 源码和相关规范／fixture，没有底层改动，不运行完整测试套件。

### 分组、关联与去重已完成

交付 groupBy/groupByTo、associate/associateBy/associateWith、distinct/distinctBy。默认结果使用 LinkedHash 容器，保留首次 key 次序和组内次序；覆盖重复 key 时保留原 key、采用末次 value。groupBy 和 associate 返回不可强转回 Mutable 接口的独立 Map，groupBy 的每个组也是只读快照。groupByTo 使用目标容器的等价关系，不强加 Hash，并在成功转换该项 value 后才创建或追加组。

`m42-algorithms-grouping` 已通过正常与 moving GC：验证两组 valueTransform 重载、首项保留、现存目标分组追加、以无 Hash/Equals 的 Key 配合 TreeMap、自定义回调异常及前缀结果。证据为 `target/m42/algorithms-first-report.json` 中的 grouping 项。`m42-algorithms-hash-bound` 在元素已实现 Equals 的前提下，精确验证普通 groupBy 拒绝缺失 Hash 的 key；通过记录在 `target/m42/algorithms-final-report.json`。没有底层实现修改。

### M42-5 完成：稳定排序、二分与算法组合验收

交付 sorted/sortedDescending/sortedBy/sortedWith、sort/sortWith、binarySearch 和自然／显式 Comparator 的 min/maxOrNull。归并仅索引私有 ArrayList，先遍历复制调用者；相等时取左项，selector 每项执行一次。原地排序完整比较结束才写回，比较失败不改来源，自定义 setter 失败按规范留下写回前缀。二分使用半开区间及防溢出的中点、负插入点计算，默认 toIndex 取 receiver.size。

`m42-algorithms-sorting` 验证稳定性、只支持顺序遍历的 List、逆序／自然排序、首次并列极值、NaN／正负零、511 项置换模型及 O(n log n) 比较上界；虚拟 Long.MAX_VALUE 大小的 List 覆盖极限命中、空区间与 Long.MIN_VALUE 的未命中编码。`natural-bound` 精确拒绝没有 Compare 的自然排序。`artifacts` 将含引用 struct 的稳定排序、窗口、分组、默认参数二分与 join 组合，删除 core/std/消费者源码后独立 link，并验证链接计划一致及 normal/moving 运行结果。

本机算法测试现有 10 个 fixture 全部闭合，未重复运行此前已通过且未修改的用例。两轮均使用 `TMPDIR="$PWD/target/m42/scratch"`、`--work-dir target/m42/foundation --jobs 4`：

- 首轮 `--suite tests/fixtures/m42-algorithms` 当时选择 6 项，5 项通过、22 个进程；`target/m42/algorithms-first-report.json`。sorting fixture 中对不具备 Equals 的 class 使用 Option 相等是普通源码错误，修正为模式匹配后定向重跑，没有绕过合法程序的底层缺陷。
- 收尾在同 suite 使用 `--filter m42-algorithms-sorting --filter m42-algorithms-artifacts --filter 'm42-algorithms-*-bound'`，5/5、5 个变体、16 个进程通过；`target/m42/algorithms-final-report.json`。所有运行类 fixture 都包含正常与 moving GC。

结合此前 Deque/PriorityQueue，M42-5 的计划交付已完成。全部新增库仍为普通 Scoop Cone；本批没有 compiler/runtime 修改，没有运行无关完整套件。已保留报告与共享 cache，并清理完成的 case 及单次检查产物。M42-6 至 M42-12、Linux GNU/musl 的本组最终定向验收仍待完成。

## M42-6 进行中：Result 与 Option 扩展已完成

日期：2026-10-13。普通 invariant Result<T,E> 使用 Success/Failure，E 可为非 Throwable。已交付状态属性、两侧 OrNull、getOrElse/fold、map/mapError/flatMap/recover、onSuccess/onFailure、toOption、受 Throwable bound 约束的 getOrThrow，以及显式捕获 Throwable 的 runCatching。Option 的 map/flatMap/filter/fold/getOrElse/orElse/toResult 保留 Some(None) 层次，缺省 producer 仅在缺失时调用一次。所有普通组合操作直接传播回调异常。codec 随 M42-12 交付。

本机新增 5 个正式 fixture 全部闭合：basic、option、exceptions、throw-bound、artifacts。覆盖非异常 typed error、成功与失败分支短路、不同结果／错误类型变换、含引用 payload、嵌套 Option、集合组合、Throwable 直接子类与自定义异常子类、精确 bound 负例；artifact-only 用例移除 core/std/消费者源码后独立链接运行。所有运行类均执行普通和 moving GC。

证据保留为 `target/m42/results-first-report.json` 的 basic/throw-bound/artifacts 三项，以及 `target/m42/results-final-report.json` 的 option/exceptions 两项（2 个变体、8 个进程）。正式命令均使用 `TMPDIR="$PWD/target/m42/scratch" python3 tests/run_fixtures.py --suite tests/fixtures/m42-results --work-dir target/m42/foundation`；首轮 `--jobs 4`，收尾只选择两项受修改 fixture，`--jobs 2`。初始测试误以为 catch 与 throw 前的对象具有相同 identity；core-only probe 与运行时规范 5.1～5.4 均确认现有复制／物化行为。修正测试为动态类型和 payload 浅复制检查，普通 Result 包装仍检查引用保持；没有修改底层或库代码来绕过问题。

已执行源码风格检查、`git diff --check` 和普通 std Cone 构建。无 Rust/C 修改，不运行完整测试套件；已清理本轮 case、诊断 probe 及单次负例目录，保留报告与共享 cache。Sequence 及后续阶段保持原交付范围。

### Sequence 构造与迭代生命周期已完成

交付独立的 Sequence<T> 协议、emptySequence/sequenceOf、Iterable/Iterator.asSequence、两种 generateSequence 与 toList。普通 iterator factory 在首次 next 时建立来源；每次迭代独立保存进度，once-only 在请求 iterator 时占用。延迟 wrapper 统一保持永久耗尽与失败终止，第一次失败传播原异常，后续 next 抛 IllegalStateException，不重跑来源或回调。Sequence 按语言规范 11.8 的普通 operator iterator 规则直接用于 for，无需修改编译器，也不让 Sequence 隐式继承 Iterable；toList 通过内部普通适配复用集合算法。

`m42-sequences-foundation` 与 `m42-sequences-lifecycle` 在普通和 moving GC 下全部通过，2 个变体、8 个进程，证据 `target/m42/sequences-foundation-report.json`。命令为 `TMPDIR="$PWD/target/m42/scratch" python3 tests/run_fixtures.py --suite tests/fixtures/m42-sequences --work-dir target/m42/foundation --jobs 2`。覆盖延迟创建来源、for、重复 Iterable、交错 seeded iterator、Some(None) seed、消费前的 once-only 预约、源 factory／generator 异常不重试，以及第一次 None 后不访问会恢复产值的底层 iterator。已构建普通 std Cone、检查源码格式并清理完成的 case；惰性算子、其余终结操作与组合产物验证继续实施。

### Sequence 惰性算子与排序已完成

交付 map/mapIndexed/mapNotNull、filter/filterNot/filterNotNull、flatMap/flatten、onEach、take/drop/takeWhile/dropWhile、zip/withIndex、distinct/distinctBy、chunked/windowed、sorted/sortedDescending/sortedBy/sortedWith，以及 joinToString。每个算子的状态随 iterator 独立建立；windowed 在下次 next 才滑动或跨过间隙，当前输出不提前消费后续元素，输出窗口为独立只读快照。排序在首次消费时完整收集并稳定排序，后续读取使用本次结果。共用延迟 wrapper 保证回调或上游失败后不重试。

`m42-sequences-pipeline/windows/failures/sorting` 全部通过正常与 moving GC：无限 generator 经 filter/drop/take 的精确调用次数；take(0) 不占用上游 once-only iterator；嵌套 Option、空 inner flatten、zip 左侧多读约定、去重保留首项；480 组有限窗口模型、无限重叠窗口与大间隙短路；首个 next 才收集排序、selector 每项一次及相等 key 的稳定性。异常组合覆盖 map/filter、flatMap 内层、zip 右侧、distinct selector、window 上游、Comparator 和排序 selector，每次第一次异常传播、再次 next 不重试。

证据为 `target/m42/sequences-operations-report.json` 中上述四项；该轮另两项终结操作见后续收尾记录。新增 compare-bound/hash-bound/string-bound/invariant 四项精确负例均通过，诊断包含缺失 Compare/Hash/ToString 和 Sequence<String> 不能隐式转换为 Sequence<Any>，记录在 `target/m42/sequences-artifacts-report.json`。普通 std Cone 构建、源码风格和 `git diff --check` 已通过；没有底层实现修改。

### M42-6 完成：终结操作与组合产物验收

Sequence 补齐查找／成员判断、遍历／归约／计数、partition/reversed、极值、分组／关联、List/Set 目的地操作及 List/Set/tuple-entry Map 的只读和可变 materialize。通过内部 Iterable 适配复用 M42-5 的实现，保留短路读取、目标别名快照、只读结果与前缀提交规则；不另写一套算法。binarySearch 和原地排序仍仅属于 List/MutableList。

`m42-sequences-terminals` 验证只能读取一次的无限来源在 first/any/none/all/find/elementAt/contains/indexOf 时正确停止，并覆盖各无参数／predicate 重载、空归约、partition、reversed、极值和有限 join。`destinations` 覆盖 List/Set 的转换及两套目的地重载、flatMapTo 内层别名、两套 groupBy/groupByTo/associateBy 与其他关联操作、分组顺序和重复 tuple key 覆盖。`artifacts` 组合含引用 struct、Result、无限 generator、mapNotNull/windowed/take、跨 Cone generic collect 和 Map materialize；移除 core/std/消费者源码后独立链接，正常与 moving GC 均通过，并检查短路仅处理五个来源元素。

Sequence 的 13 个正式 fixture 全部通过，三个报告分别对应 2 个基础、6 个算子／终结、5 个约束／产物用例；运行类全有 normal/moving 两次执行。报告依次为 `target/m42/sequences-foundation-report.json`（8 个进程）、`sequences-operations-report.json`（24 个进程）、`sequences-artifacts-report.json`（14 个进程）。后两轮命令使用相同 `TMPDIR="$PWD/target/m42/scratch"`、`--suite tests/fixtures/m42-sequences --work-dir target/m42/foundation --jobs 4`；第二轮 exact filter 选择 pipeline/windows/terminals/destinations/failures/sorting，第三轮选择 `m42-sequences-*-bound`、invariant、artifacts。没有重复运行已通过且未修改的基础用例。

结合 Result/Option 的 5 个 fixture，M42-6 共 18 个当前 fixture 均有明确通过证据，本批计划范围完成；文件按职责拆分，Sequence 实现的最大单文件为 63 行。没有 compiler/runtime 修改，未运行无关完整套件。已清理完成的 case、检查构建和四个诊断构建目录，保留报告与共享 cache。M42-7 至 M42-12 以及 Linux GNU/musl 的本组最终定向验收继续保持原范围。

## M42-7 进行中：文本搜索与处理已完成

日期：2026-10-13。交付 String/Char 搜索与 contains、前后缀、substring/before/after、removePrefix/Suffix、replace/replaceFirst、split、lines/lineSequence、默认／predicate trim 与 blank、padding/repeat、ASCII 大小写转换。统一使用 Unicode 标量与 Long 索引，保留 NUL，不做 normalization。KMP 前缀表复用于搜索、替换与切分；批量操作一次取得字符快照，片段直接从快照构造，避免反复 O(n) 的 String 索引和 slice。lineSequence 使用普通延迟 iterator，CRLF 合并并保留尾部空行；repeat 按二进制倍增，padding 复用 repeat，先检查实际 UTF-8 大小。

固定 White_Space 取自 Unicode 17.0 的 25 个标量，许可证随 std 保存；不依赖 locale 或系统数据文件。trim 的 predicate 顺序、lastIndexOf 的起点含义、split limit 与大小溢出行为已补明规范。

本机新增 search、replace-split、whitespace-padding、lines 四个 fixture 全部通过正常和 moving GC，4 个变体、16 个进程。命令：`TMPDIR="$PWD/target/m42/scratch" python3 tests/run_fixtures.py --suite tests/fixtures/m42-text --work-dir target/m42/foundation --jobs 4`；证据为 `target/m42/text-strings-report.json`。搜索以独立逐位置匹配模型核对每个合法 startIndex，含空 needle、重叠、NUL、补充平面标量与组合字符；另有重复前缀长输入。其他覆盖非重叠替换、空旧串所有边界、连续／尾部分隔与 limit、1024 段输入、全部 25 个 White_Space 及边界反例、predicate 顺序和异常、ASCII 保留非 ASCII、负数及 UTF-8 长度溢出、三类混合换行和 Sequence 短路／重复消费。

已通过普通 std Cone 构建及源码风格／`git diff --check`；无底层修改，不运行完整测试套件。报告和共享 cache 保留，已清理完成的 case。整数、布尔、精确浮点解析及后续阶段继续按原范围实施。

### 定宽整数、radix 格式化与布尔解析已完成

交付八种 canonical 整数及八种 core alias 的 String 转换和 OrNull、八种整数的 toString(radix)、toBooleanStrict/OrNull 与 NumberFormatException。解析先校验 radix，严格处理 ASCII 数字和单一符号，使用目标上限的 quotient/remainder 在乘加前阻止溢出；完整覆盖 Long.MIN_VALUE 和 ULong.MAX_VALUE。signed 格式化使用 unsigned magnitude，不对最小 signed 数直接求绝对值；字母统一小写，无正号或多余前导零。OrNull 不吞非法 radix，unsigned 连 -0 也拒绝；高 radix 下的字母只按数字解释，不启用类型后缀或前缀语法。

本机 integers/radices/parse-errors/artifacts 四个 fixture 全部通过，4 个变体、18 个进程，所有运行类覆盖 normal/moving。证据为 `target/m42/text-integers-report.json`。命令使用相同 `TMPDIR="$PWD/target/m42/scratch"`、`--suite tests/fixtures/m42-text --work-dir target/m42/foundation --jobs 4`，exact filter 只选择本轮四项。覆盖各类型最小／最大值和越界一位、全部 alias、符号／前导零、独立二／八／十六／三十六进制极值向量、2..36 全 radix 代表值往返、四种 radix 的 Int8 全值域、严格布尔／数字语法、无效 radix 与 NumberFormatException。产物用例组合混合换行、Unicode key、split/trim/ASCII 规范化、整数解析／格式化、Sequence 与 Result；删除 core/std/消费者源码后独立 link，链接计划与运行结果正确。

本轮新增源码最长为 43 行。普通 std Cone 构建、源码风格与 `git diff --check` 通过；没有 compiler/runtime 修改，不跑完整套件。已清理完成的 case 和检查构建，保留报告与共享 cache。M42-7 的精确 Float/Double 解析尚未完成，M42-8～12 和 Linux GNU/musl 的本组最终验证仍待实施，不将本批通过等同于整个 M42 已验收。

### M42-7 完成：精确 Float／Double 文本解析

新增 toFloat/toDouble 与 OrNull，完整校验 ASCII 十进制语法、特殊值与指数，保留 signed zero，按目标精度直接最近偶数舍入。有限输入只有舍入到无穷才失败；略大于 MAX_VALUE 但仍舍入到 MAX_VALUE 的文本成功。公开规范同步明确该边界与 +NaN/-NaN 的拒绝行为。

实现由普通 Scoop 的十进制扫描、指数抵消、UInt limb 整数、精确商／余数舍入与目标类型缩放组成。有效位保留数量由 IEEE 格式中点的有限十进制长度推导，长尾只保存非零标志；全输入继续验证，不添加数值长度配额。没有使用 Double 中转 Float、JSON native parser、Extern/Intrinsic 或宿主 helper。新增库源码单文件最多 81 行。

正式 CLI 验证：

- float-grammar 覆盖合法语法、42 种非法形式、OrNull／NumberFormatException、特殊值、负零与有限溢出；float-long 覆盖 2048 位输入、巨大指数、指数与小数点抵消、远端 sticky、负号及明确的 Double→Float 二次舍入反例。两者普通／moving GC 通过，证据为 `target/m42/text-float-grammar-long-report.json` 的对应通过记录。
- float32／float64 各有 324 个独立向量，覆盖正负输入、中点两侧与奇偶 tie、零／subnormal／normal 边界、binade 进位、溢出阈值、超过有效位保留范围的长尾和固定种子随机十进制。期望由 `tests/generate_float_text_vectors.py` 以 Fraction 在 IEEE 相邻值间二分并比较精确距离生成；宿主浮点只用于输出已选定位型的字面量。脚本仅离线再生 fixture，不接入 runner 或库实现。
- 两组向量在 debug／release 下分别以普通／moving GC 运行通过。artifacts 增加 Sequence→浮点解析组合，移除 core／std／应用源码后独立链接运行通过。证据 `target/m42/text-float-vectors-artifacts-report.json`：3 fixture、5 variant、22 process 均通过。

Python 生成脚本经固定版本 Ruff 格式化与 lint；全部改动通过 `git diff --check`，普通 std 构建成功。初次生成脚本 lint 失败导致尚无源码的两个向量 Cone 构建失败，修正并生成向量后的正式验收均通过；没有库算法或底层失败。已清理完成的 case 与单次检查构建，保留报告及共享 cache。本批没有 compiler/runtime 修改，不运行无关完整套件。M42-7 已完成，M42-8～12 及 Linux GNU/musl 的本组最终验证继续实施。
