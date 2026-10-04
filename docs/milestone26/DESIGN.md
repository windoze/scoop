# M26 设计：List、字符与 parts 字符串构建

状态：实现中；数组初始化、List、Char、String 与 StringBuilder 已完成，插值继续按第 9 节实施。

日期：2026-10-04。

依赖：M22 的 Long、循环与值模式，M23 的普通泛型/interface 与跨 Cone 产物闭环，以及 M25 的异常 ABI。当前工具链继续使用已完成 M24 的产物与 runtime 基线，但 M26 的功能不依赖 release hook。

对应 [路线图 M26](../ROADMAP.md)；权威行为分别位于 [语言规范](../specs/SCOOP-SPEC.md)第 6、10.6、11.2.1、11.4、11.6、11.10 节、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)第 2.4、6 节和[实现规范](../specs/SCOOP-IMPL-SPEC.md)第 2.15 节。

## 0. 范围与交付目标

M26 形成以下完整能力链：

```text
Array / MutableArray 按长度初始化
    → List / MutableList 普通接口
    → ArrayList 普通泛型实现
    → StringBuilder 保存 String parts
    → f-string 按普通调用脱糖

Char 与 UTF-8 标量操作
    → String 索引、切片、字符/字节快照
    → parts 最终拼接与插值输出
```

公开能力包括三个列表类型、数组初始化构造、Char、String 的字符操作与转换、StringBuilder 和单行/raw 多行插值。必须通过正式 CLI 从源码编译、消费独立 `.slib`、链接并运行，不能只交付接口声明或宿主测试中的替代容器。

ByteBuffer、off-heap storage、borrow/view、close、外部内存压力反馈及相关 release-safe 入口整体移到后续待排期事项。本里程碑也不扩展成 Collection/Set/Map 层次、可变 iterator、subList、COW、通用未初始化存储或 ownership 框架。

## 1. 关键决策

| 项目 | 决策 |
| --- | --- |
| 数组 | 继续定长；增加完整初始化构造，增长由容器替换 backing |
| List | 只读访问接口，允许观察其他 alias 的修改 |
| MutableList | 在 List 上增加写入、追加、插入、按索引移除与清空 |
| ArrayList | 普通 final generic class，GC 数组承载元素，几何扩容 |
| 泛型关系 | 全部 invariant，沿既有 exact nominal application 与 itable |
| 空槽 | `Option<T>`；有效元素是 Some，空闲槽是 None |
| Char | 一个 Unicode 标量值，32-bit GC-free 表示 |
| String 索引 | length/get/slice 按标量值；byteLength 单独表示 UTF-8 字节数 |
| parts | 私有 ArrayList<String>；add 保存引用，build 最终集中复制字节 |
| runtime | 只处理分配、UTF-8 与复制，不认识 ArrayList/StringBuilder 字段 |

String 的 Unicode 标量索引口径已由用户明确选择。它不等同于 grapheme cluster：组合附加符、ZWJ 等各自占一个标量位置，不自动 normalization。

## 2. 数组初始化基础

增加两个 registry 规定的构造候选：

```text
Array<T>(size: Long, init: (Long) -> T)
MutableArray<T>(size: Long, init: (Long) -> T)
```

它们和既有 `Array(source)` / `MutableArray(source)` 共用普通名称查找、参数映射、generic inference、alias 与 overload 选择。参数名固定为 size/init，支持尾随 lambda；数组字面量仍只创建 Array 或 MutableArray。

```scoop
val squares = Array<Long>(4L) { i -> i * i }
val slots = MutableArray<String?>(8L) { _ -> None }
val characters = MutableArray<Char>(3L) { _ -> '雪' }
```

全部实参先按源码顺序各求值一次。负 size 在调用 initializer 前抛 IllegalArgumentException；零 size 不调用 initializer，但不能删掉取得函数值的实参求值。其余依次调用 init(0L)、init(1L) 等，每个索引一次。initializer 为 ordinary 函数值，可以分配或抛出，但不能挂起。第 i 次失败后不执行后续调用，也不回滚已发生的用户副作用。

此构造不接收可观察的半初始化数组。分配与逐元素写入是生成 CFG 内部过程，完整结果只在全部初始化成功后返回；Array 的内部初始化也不要求公开 set。GC 根据完整 array TD/count 扫描，未填充存储的 ref leaf 保持零；清零仅满足扫描要求，不把任意位型解释成合法 T。initializer 调用前后通过现有 roots/statepoint 保存数组、closure 与返回值，重新计算元素地址。

ZST 数组也调用每个索引的 initializer，只省略数据区写入。元素数量、分配乘加和对齐采用既有 checked 规则；对象过大、溢出或资源耗尽沿现有 fatal allocation failure。没有公开的 `uninitialized<T>` 或可变长度数组对象。

## 3. List 与 MutableList

### 3.1 公共协议

```scoop
public interface List<T> : Iterable<T> {
    public val size: Long
    public operator fun get(index: Long): T
}

public interface MutableList<T> : List<T> {
    public operator fun set(index: Long, value: T): Unit
    public fun add(value: T): Unit
    public fun add(index: Long, value: T): Unit
    public fun removeAt(index: Long): T
    public fun clear(): Unit
}
```

关系为 `ArrayList<T> <: MutableList<T> <: List<T> <: Iterable<T>`。List 的只读性只约束经该静态类型可调用的操作；不复制对象、不冻结对象，也不承诺元素深度不可变。

```scoop
val values: MutableList<Int> = ArrayList<Int>()
values.add(10)
val view: List<Int> = values
values.add(20)
val snapshot = view.toArray()
values[0L] = 30
// view[0L] is 30; snapshot[0L] is 10.
```

三个类型及继承的 Iterable/Iterator 均保持 invariant。`List<Derived>` 不能赋给 `List<Base>`，需要改变元素类型时由用户显式复制或转换。`List<Int>` 通过 exact interface 调用 get 仍返回 Int；只有把单个值显式/按普通 O(1) 规则放入 `List<Any>` 等位置才会装箱。

### 3.2 操作语义

| 操作 | 前置条件 | 成功结果 |
| --- | --- | --- |
| get(index) | `0 <= index < size` | 返回该位置的 T |
| set(index, value) | 同上 | 替换元素，返回 Unit，size 不变 |
| add(value) | size 可继续增长 | 追加一项，返回 Unit |
| add(index, value) | `0 <= index <= size` | 插入一项，后缀右移，返回 Unit |
| removeAt(index) | `0 <= index < size` | 返回旧元素，后缀左移 |
| clear() | 无 | 删除所有元素，size 变零 |

越界统一抛 IndexOutOfBoundsException，且在容器修改前发生；receiver/value/index 等实参已经按普通调用规则完成求值。size 使用 Long，取值范围为 `0..=INT64_MAX`。负构造容量是 IllegalArgumentException，实际增长与分配失败沿第 2 节。

这组接口不对任意 T 假定 equals、Hash 或 ToString。M26 不加入按值 remove、contains、结构相等和列表字符串化；不借 Any 建立默认能力。自定义 List/MutableList 实现遵守上述公开语义，编译器只执行普通 interface 检查，不通过类型名放行 core 容器。

### 3.3 与数组和快照的关系

Array<T> 和 MutableArray<T> 都实现 List<T>，经该接口继承 Iterable<T>。MutableArray 自身的 set 继续可用，但其固定长度无法满足 MutableList 的增删契约，因此不声明该 conformance，也不提供总会失败的增删成员。

数组 size 需要在 core 源码中成为真实 `public override val size: Long` getter，供普通属性/接口分派消费。getter 调用表示级数组长度 intrinsic；它需要读取 managed receiver，因此源码 getter/member 不标注 NoGC。旧的按 `.size` 拼写处理的入口不能代替接口实现。get/set/clone 继续使用原有 typed intrinsic 操作。

core 提供 `List<T>.toArray()` 与 `List<T>.toMutableArray()` 普通 generic extension，读取一次 size，再按索引顺序创建完整快照。现有数组转换成员在普通候选规则下优先，保持 fresh-array 的浅复制语义。自定义 List getter 的异常在 managed 代码中传播，不保证并发快照。M26 不新增 listOf/mutableListOf、Collection 或 subList；空列表通过 ArrayList 构造获得。

## 4. ArrayList 存储与迭代

### 4.1 构造与不变量

公开构造为 `ArrayList<T>(initialCapacity: Long = 0L)`，结果为 size 等于零的普通 final class。初始存储由第 2 节的 MutableArray 构造产生；容量不代表有效元素。公开表面只有协议成员，容量与 backing 留在实现内部。

```text
storage: MutableArray<Option<T>>
elementCount: Long

0 <= elementCount <= storage.size
[0, elementCount)             = Some(value)
[elementCount, storage.size)  = None
```

T 本身可为 Option<U>，有效的 None 元素编码成 Some(None)。String 的 Option 使用已有空指针 niche，适合作为 parts 存储；其他 T 的 Option 可能增加 tag/padding。`ArrayList<Unit>` 也使用普通 Option<Unit>，不为它引入另一套 ZST vector 表示。

GC 扫描 backing 的完整容量，不依赖 ArrayList 的 elementCount。set 覆写完整 Some，removeAt 清空原尾槽，clear 将旧有效前缀全部写成 None，避免旧引用继续保活。值类型按已有 inline copy/store 处理，没有用户复制回调、析构或统一 Any 存储。

### 4.2 增长与修改

首版可以从零容量增长到 8，再按 2 倍增长；比例属于可替换策略，不是产物兼容性或源码常量。实现先 checked 计算 required size，在增长乘法超过 Long 上限时回退 required；实际字节大小由已有数组分配检查决定。

扩容读取当前 storage/count 为普通 local，创建更大且全部初始化的数组，复制有效前缀，完成后替换 owner 字段。旧数组身份和长度不变。新值先写入有效位置，再提交新 count；插入/删除按方向搬移完整 Option<T>。正常完成后满足 4.1，不在其他 stage/runtime 中重复重放这些容器不变量。

get/set 为 O(1)，append 摊还 O(1)，中间插入/移除和 clear 为 O(size)。clear 可保留容量；GC 不承诺立即回收旧 backing/被移除对象。并发同步、自动缩容和公开 reserve/trim 不进入本里程碑。

### 4.3 iterator 的修改可见性

core 的 Array、MutableArray、ArrayList 使用普通按索引 iterator，可共享一个 internal 实现。iterator 保存 `owner: List<T>`、`nextIndex: Long`、`exhausted: Boolean`；不缓存 ArrayList backing。

每次 next 的顺序是：若已耗尽则 None；否则读取当前 size，索引越界时置耗尽并返回 None；若还有元素，先成功读取 owner[nextIndex]，再把索引加一并返回 Some(value)。getter 抛出时索引不前移。因为最大可读索引小于 size 且 size 不超过 INT64_MAX，成功递增不溢出。

串行修改的效果是当前索引语义：set 对以后读取可见；在首次 None 前 append 的元素可以被观察；插入或删除会改变后续索引对应的元素，允许因此重复或跳过；clear 后下一次 next 返回 None。已经返回 None 的 iterator 不因后来追加而恢复。iterator 不提供同步，也不添加 modification counter 或 ConcurrentModificationException。

## 5. Char 与 UTF-8

### 5.1 Char 的完整语义

Char 的值域为 U+0000..U+D7FF 与 U+E000..U+10FFFF，内联表示为按 target u32 自然对齐的 4 byte，GC-free。它是独立 intrinsic struct，不是 UInt alias；没有字段解构、公开 primary constructor 或隐式整数转换。

字符字面量支持直接写入的标量及第 6 章统一转义，例如 `'雪'`、`'😀'`、`'\u0000'`、`'\u{1F600}'`。转义后必须恰为一个标量；空字符、多标量、surrogate、超过最大值、物理换行和未闭合字面量都在原 span 报错。

`Char.code: Int` 取得编号；普通 extension `Int.toChar()` 检查范围后构造 Char，非法值抛 IllegalArgumentException。code、同类型 equals/compareTo 与内部 unchecked 构造由封闭 scalar intrinsic 实现；unchecked 构造只供 core 在满足域条件后使用，并显式 NoGC/Unsafe。Char 的 toString 编码一个标量，hash 返回编号的 Long；不提供数字运算或 CharRange。

Char literal/const 引用进入 const/default/递归 pattern；普通 Char 方法不扩大 const-call 集合。覆盖算法使用已出现 singleton 和剩余标量集合，不能枚举整个域或生成 surrogate witness。C ABI 使用 uint32_t，外部实现负责满足标量值前置条件；Option/aggregate/array/boxing 走现有 exact 布局。

### 5.2 String 的计数、索引与切片

String 保持当前 inline UTF-8 对象布局，offset 16 是 byte count，offset 24 起是字节。内容必须为严格合法 UTF-8，可以包含 U+0000，不带隐含的 NUL 结尾要求。

```scoop
val text = "A雪😀"
// text.length == 3L
// text.byteLength == 8L
// text[1L] == '雪'
// text.slice(1L, 3L) == "雪😀"
```

length/get/slice 按 Unicode 标量值，byteLength 按 UTF-8 字节，参数及结果计数均用 Long。get 的合法区间是 `[0, length)`；slice(start, endExclusive) 要求 `0 <= start <= endExclusive <= length`，允许尾端空区间。其他情况由普通 core 代码抛 IndexOutOfBoundsException。

byteLength 是 O(1)，length 与索引定位需要扫描。String 实现普通 Iterable<Char>，iterator 保存 String ref 与 byte cursor，每次只解码下一标量，遍历总计 O(byteLength)，不反复调用字符下标，也不新增索引缓存或 String 的 List conformance。内容相等/hash 不做 normalization；合法 UTF-8 的字节字典序与标量字典序一致，现有 compareTo 可复用。

### 5.3 转换与 native 边界

| API | 行为 |
| --- | --- |
| String.toCharArray() | 返回独立 MutableArray<Char> |
| String.fromChars(chars: List<Char>) | safe companion 方法，字符快照编码为 String |
| String.toByteArray() | safe 地复制 UTF-8 字节到 MutableArray<Byte> |
| String.fromUtf8Unchecked(bytes: List<Byte>) | unsafe companion 方法，复制调用方保证合法的 UTF-8 序列 |

Array<Char>、MutableArray<Char>、ArrayList<Char> 都可经 List 参与 fromChars；字节版本同理。List 输入先由 managed core 取快照，native helper 不调用用户 getter。fromUtf8Unchecked 的 unsafe 条件针对本次实际读取形成的序列，非法输入不承诺被检测或抛出异常；不执行这种违例作为正向功能测试。输出字节没有破坏 String 不变量的能力，故 toByteArray 保持 safe。

所有转换复制存储，不 move、不共享可写 backing。字符读取 native leaf 返回 Option<Char>，slice 定位返回 Option<(Long, Long)> 的字节区间；None 由 managed core 转成源码异常。immutable String 的定位结果在后续复制时可复用，不重复全串校验。native 创建、编码、解码按既有精确 TD、roots 与返回值 ABI 执行。

## 6. StringBuilder 的 parts 实现

### 6.1 公开契约

公开零参数 StringBuilder 构造，以及以下方法：

```text
add(part: String): StringBuilder
add<T : ToString>(part: T): StringBuilder
build(): String
```

两种 add 都返回原 builder。generic add 立即调用一次 toString，随后把所得 String 追加到当前 parts；不保存原始对象或延迟调用。调用用户 toString 前不能缓存 parts 的旧 size/backing，因为用户可以重入同一 builder。toString 抛出时不追加该 part，用户自身已经造成的修改仍按正常语言语义保留。

build 不消费 parts，不 clear/close/freeze builder。重复 build 得到相同内容，后续追加不影响旧结果。空 builder 得到空串；相同不可变结果可复用引用，测试不能要求每次 build 都有 fresh identity。

### 6.2 数据与拼接

StringBuilder 只需一个私有 `ArrayList<String>`，有效长度来自列表。无需另外维护随 add 更新的总字节数，避免重复可变状态；build 时进行 checked 求和即可。空 String part 可以保存或在保持外部行为时省略。

核心库中 ArrayList 提供仅 internal 的 backing 访问，StringBuilder 取得当前 `MutableArray<Option<String>>` 和 size 作为一次调用的值。该路径不对任意用户传入的 List 开放，也不为容器新增公开 view、borrow 或 raw pointer。

表示级入口为：

```text
coreStringJoinParts(storage: MutableArray<Option<String>>, partCount: Long): String
```

helper 按 runtime spec 第 6 章完成：

1. 按已有 managed/native 协议保活 storage，检查本次有效前缀范围。
2. 读取 Some(String) 的 byte count，checked 求和并验证最终 String allocation size。
3. 分配一次最终 String；allocation 后重新从更新过的 storage 读取每个 part。
4. 顺序复制字节，设置完整结果后返回；复制期间不回调用户代码，不写已经发布的 String。

空/单 part 可以直接返回不可变 String。None 出现在有效前缀是内部错误，不变成用户 String 值。runtime 只认识具体数组表示和 String，不知道 ArrayList/StringBuilder 字段、不管理容量、不再次做完整 UTF-8/类型检查。

总工作为 O(P + B)，P 为 part 数，B 为输出字节数；扩容搬运引用，字符串字节只在最终拼接时复制。它会保持源 parts 存活，build 时源字符串与输出可同时存在。实现不额外创建 parts.toArray 快照，不为最终拼接引入 off-heap 缓冲区；普通 GC 对象已能承担全部所有权。

## 7. f-string 与字面量

实现单行 f-string、raw 多行 f-string，并补齐普通 raw 多行 String。单行字符串文本和 Char 支持 `\t`、`\b`、`\n`、`\r`、`\'`、`\"`、`\\`、`\$`、`\uXXXX` 与 `\u{...}`；每次 Unicode 转义直接产生一个合法标量。raw 内容不解释反斜杠。

只有源码文本中未转义的 `${` 开始插值；转义解码产生的 `$` 不重新作为插值起点。f-string 中 `$name` 非法，普通 String 中 `$` 始终是文本。表达式内部按普通 lexer/parser 处理注释、嵌套括号和字符串，保留原始 source span，不用文本替换拼出新源码。

概念展开：

```scoop
val result = f"name=${name}, value=${value}"
// Equivalent call order:
// StringBuilder().add("name=").add(name).add(", value=").add(value).build()
```

name 的求值与 add/toString 都完成后才开始 value。失败后不执行后续段或 build。插值表达式可以在原本允许的上下文挂起，builder 作为普通 managed ref 进入已有 coroutine slot。

parser 输出有序 Text/Expression AST part；HIR 绑定实际 core StringBuilder 与普通 add overload/build，在导出 generic/default body 前完成脱糖。shadow/import 不替换隐式目标；用户手写 StringBuilder 调用仍遵守普通名称查找。没有 ToString bound 的插值值产生普通调用诊断，不走 Any 或反射后备。f-string 不是 const val 表达式，即使只有纯文本。

## 8. 编译器与产物落点

| 层 | 必须完成的工作 |
| --- | --- |
| parser / AST | Char、统一转义、raw 字符串、有源位置的插值 part；错误恢复 |
| HIR lower | Char 常量/模式，按长度数组候选，真实数组 size override，普通 List 关系，f-string 普通调用展开 |
| HIR / meta | 完整 Char 表示与常量、ArrayGenerate 的 count/initializer/exact type；普通集合模板 |
| MIR lower | ArrayGenerate 的 managed 初始化 loop/异常/GC；其余沿普通 call、interface 与 coroutine |
| LIR / codegen | Char 精确 scalar/ABI，数组动态分配与初始化 store，字符串 helper typed call/root |
| runtime | UTF-8 定位/编码/复制、parts 拼接；复用原 array/String layout 与 GC |
| core | List/MutableList/ArrayList/iterator，Char/String surface，StringBuilder 与 snapshot extension |
| slib / driver / linker | 新 typed variant 的现有 codec/fingerprint/版本迁移，普通跨 Cone 实例化与链接 |

各 IR crate 拥有自己的 Char 常量/表示与数组生成数据，相邻 stage 穷尽转换；类型与 initializer 不是 Option 字段。ArrayList 使用 ordinary class 与 application identity，没有 intrinsic collection type 或新的实体认证关系。f-string 不进入 MIR/LIR，ArrayList/StringBuilder 不成为后端特殊指令。

Char 新增分支必须覆盖 const/default、pattern、array/aggregate/Option、boxing、C/Scoop ABI、GC-free/release-value 分类及产物读写。ArrayGenerate 必须同时支持本地与导入模板中的 binder，不能只在直接源码调用生效。

实现每批修改其实际影响的 schema/section version 与 fingerprint，重建旧 core/provider/consumer/cache；不预建一套 M26 独立格式。String/Array 对象头、普通 TD 与 M24 release hook 表示保持，新增 native signature 进入已有契约表。读取成功的静态语义/布局按现有边界复用，不在运行时重放前端规则。

## 9. 实施顺序

1. **数组初始化闭环**：两个构造、GC/异常/ZST、typed IR 与本地/跨 Cone fixture，一批完成。
2. **List 闭环**：公开协议、数组真实 size conformance、ArrayList 全部约定操作、快照与 iterator，覆盖 interface 分派与普通泛型产物。
3. **Char 与 String 闭环**：字符词法/常量/模式/ABI、标量索引与切片、按 byte cursor 的迭代、字符/字节快照与 unsafe 调用检查。
4. **StringBuilder 闭环**：普通 core parts、内部数组拼接 helper、立即转换、重复 build 与失败/重入行为。
5. **f-string 闭环**：单行/raw、多层词法、普通调用顺序、generic/default body 与协程组合。
6. **正式总验收**：本地与 artifact-only 编译/链接/运行、ODR、多 Cone、GC stress、全部 golden/negative 与既有回归。

每批交付可用能力和对应实际 fixture，不以 TODO body、总会失败的方法或测试专用替代实现衔接。数组、Char、String helper 的新增操作只承担实际表示需求；列表扩容、迭代策略与 builder 状态留在 core。

## 10. 验收范围

| fixture 组 | 独立与组合验收 |
| --- | --- |
| m26-array-construction | 零/负长度、实参顺序、逐索引一次、异常中止；ZST、过对齐值、含引用 struct/enum、初始化中 moving GC；named/default/alias/跨 Cone |
| m26-lists | 三层静态类型、数组只读 conformance、全部增删边界、跨多次扩容、Some(None)、清空旧引用、快照独立、实时 iterator 与耗尽；值/引用、generic/ODR |
| m26-char | ASCII/CJK/补充平面/NUL、surrogate 邻接边界、字面量转义错误；const、默认值、nested pattern witness、Option/array/aggregate/FFI |
| m26-strings | 标量数与字节数差异、组合字符、空/全区间/越界 slice、顺序迭代、safe char 快照、unsafe byte 导入与 safe 导出 |
| m26-string-builder | 空/单/大量 parts、跨扩容、原对象修改后转换结果稳定、toString 次数/顺序/抛出/重入、重复 build 与继续 add、moving GC |
| m26-interpolation | 单行/raw/普通 raw、转义美元、嵌套字符串/注释/插值、span 诊断、shadow、ToString bound、默认值、generic 与真实挂起 |
| m26-cross-cone | core/provider/consumer 独立构建、transitive 产物、同实例双 consumer ODR、artifact-only link/run、旧 schema/cache 拒绝与重建 |

每个新的编译错误规则有 negative fixture，断言位置和诊断；运行期负长度/索引通过可捕获异常 fixture 验证，不能将其改成编译期规则。unsafe UTF-8 的违例不作为可恢复运行期输入测试；其缺失 unsafe context 则是编译错误。GC 检查覆盖清空后不再保活旧元素和构造/拼接中 relocation，不建立新的通用计数框架。

AST/HIR/MIR/LIR golden 锁定实际新增结构与普通调用展开；测试围绕语言组合和产物边界，不为 List/ArrayList 新建一套 meta 语义检查器。

实现批次先执行格式化与 lint，再测试：

```sh
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
cargo build -p scoop -p scoopc -p scoop-linker --bins
python3 -m unittest discover -s tests/fixture_runner/tests
python3 tests/run_fixtures.py --all
```

如修改 Python runner，按仓库 AGENTS.md 先运行固定版本 Ruff。runtime C/GC 测试沿已有 harness；fixture 声明遵守 [schema 1](../../tests/fixture_runner/README.md)。各批次实际完成的验证记录在下节，里程碑总验收在全部功能完成后执行。

## 11. 实施记录

### 数组初始化

- 新增两个 size/init 构造候选，沿普通参数映射、类型推断、alias 与导入模板解析；保留既有 source 转换候选。
- Export/LocalConcrete HIR 使用完整 ArrayGenerate；MIR 生成负长度异常、清零分配、普通 initializer 调用及逐元素写入；LIR/codegen 复用既有 checked array layout 与 statepoint。
- IllegalArgumentException 的普通默认构造适配进入实际 core protocol；跨 Cone 按实际需要选择其构造。core-bootstrap-interface 升至 5，cross-cone-interface 升至 46。
- m26-array-construction 的 7 个正式 CLI fixture 均通过，锁定 9 份 HIR/MIR/LIR golden；覆盖初始化顺序、零/负长度、ZST、过对齐和含引用元素、异常中止、moving GC、泛型/default/alias 及删除源码后的产物链接。
- 已执行 cargo fmt 与全 workspace clippy；HIR、HIR lowering、MIR lowering、LIR lowering 和 slib 的相关单元测试通过。全 workspace 与全文件 fixture 总验收留到第 9 节最后一批。
- 原有 21 个数组转换 CLI fixture 回归通过：16 个诊断 fixture 与 5 个源码/多级产物/standalone link/moving GC fixture；同步新增候选诊断与 core 普通构造变化后的符号和阶段基线。

### List 与 ArrayList

- core 提供 invariant List/MutableList、完整 ArrayList 增删与清空、浅快照及共享 ListIterator；数组以真实 size/get override 实现 List，固定长度 MutableArray 不实现 MutableList。
- 数组长度和私有读取 intrinsic 只承接表示操作，公开 getter/get、接口槽与 iterator 沿普通源码调用。intrinsic class 允许无存储的 computed property；移除旧 `.size` 拼写旁路。
- ArrayList 使用 MutableArray<Option<T>>，checked 几何增长，移除/清空及时清除尾部引用；Long 容量溢出复用 runtime fatal allocation 路径。intrinsic 注册表按数据定义、注册和 wire 职责拆分。
- 实例化后的泛型扩展调用保留实际静态接收者的共有声明需求，修复私有 List 实现调用快照扩展时的产物缺失；源码可见性保持原声明。
- core-bootstrap-interface 升至 6、cross-cone-interface 升至 47、cross-cone-type-semantics 升至 14，相关 profile/fingerprint 与旧产物拒绝测试同步。
- m26-lists 的 10 个正式 fixture 全部通过，锁定 9 份阶段 golden；覆盖增删边界、跨多次扩容、Some(None)、Unit、值类型/装箱、旧引用回收、修改可见性与永久耗尽、自定义 size/get 顺序和异常，以及跨 Cone 的普通泛型实例与删除源码后链接，包含正常和 moving GC 运行。
- 执行 cargo fmt、全 workspace clippy；HIR 873、HIR lowering 1314、MIR lowering 114、LIR lowering 146 个单元测试通过，slib 590 个测试通过。7 个数组初始化 fixture 使用新 core 回归通过并同步 9 份阶段 golden。

### Char

- 独立 intrinsic Char 沿普通名义 struct application 保留实际 core 身份，以四字节 GC-free 标量参与布局、装箱、泛型容器、C ABI 和指针读写；内部 code/构造转换在 MIR 显式表示，LIR 使用 i32 值。
- 实现字符字面量与统一 Unicode 转义、code、checked Int.toChar、比较、ToString/Hash，以及 const、默认参数、递归模式和跨 Cone 导出。覆盖分析只划分已出现的 singleton 和剩余标量，诊断不生成 surrogate 或非法转义。
- intrinsic struct 计算属性复用普通 accessor，Char 解构与构造得到正常诊断；按词法、常量二元运算和属性解析职责拆分模块。
- core-bootstrap-interface 升至 7、cross-cone-interface 升至 48、cross-cone-type-semantics 升至 15、cross-cone-type-bridge 升至 8、cross-cone-layout-abi 升至 7，codec、profile/fingerprint 与旧版本拒绝基线同步。
- 23 个 Char 正式 fixture 通过，包含 18 个负例与 18 份 AST/HIR/MIR/LIR golden；覆盖 scalar 边界、异常、同名声明遮蔽、嵌套值/Option、数组与列表、装箱、C aggregate/指针，以及删除源码后的跨 Cone 链接。正向程序同时在正常和 moving GC 下运行。
- 执行 cargo fmt 与全 workspace clippy；parser 438、HIR 874、HIR lowering 1314、MIR lowering 114、LIR lowering 146、slib 590 个单元测试通过。
- 原有数组初始化和 List 的 12 个正式回归 fixture 通过，18 份阶段 golden 随新增实际 core 类型同步。

### String

- 普通 core String 提供 length/byteLength、按 Unicode scalar 的 get/slice、Iterable<Char>、字符与 UTF-8 字节快照，以及 public companion 的 fromChars/fromUtf8Unchecked；计算属性回到实际 String class application 后复用共有属性查询。
- StringIterator 持有 String 和只前进的 byte cursor；toCharArray/toByteArray 使用普通完整初始化构造，List 输入先做普通数组快照。UTF-8 编解码与 Char.toString 共用小型 runtime 模块，切片定位结果只计算一次。
- 两个 Option 结果遵守已有 Scoop aggregate 返回 ABI，以平台 shim 接收 x8 返回存储；分配入口登记实际输入数组/String 的 native roots，并在分配后重取地址。未增加数组 TD 猜测、字符串索引缓存或新的 IR 指令。
- 原 types.scoop 按职责拆为 53 行的基本协议/alias/Boolean 文件、487 行 signed integers、468 行 unsigned integers，以及独立的 String 与 iterator 文件。
- 9 个正式 String fixture 全部通过，包含 5 个完整诊断负例与 18 份阶段 golden；覆盖空串、NUL、组合字符、全部 UTF-8 宽度边界、Long 越界、持续耗尽、独立快照、用户 getter 的顺序/异常、泛型 Iterable，以及删除 core/provider/consumer 源码后的独立产物链接和 moving GC。
- 执行 cargo fmt、全 workspace clippy、C 严格告警语法检查；HIR lowering 1314 和 toolchain 12 个单元测试通过。10 个已有数组/List/Char 正式正例回归通过，36 份阶段 golden 同步。

### StringBuilder

- 普通 core class 以私有 ArrayList<String> 保存 parts；泛型 add 在每次调用中立即执行一次 ToString，成功后追加，允许重入与异常的既有副作用保留。build 可重复调用，也可继续追加。
- 45 行 runtime 拼接入口只消费 backing 与有效前缀，checked 求和、一次分配并集中复制 UTF-8；分配前登记 backing root，分配后重取 parts。未加入容器布局、用户 callback、额外快照或新的 IR 指令。
- 修复定义 Cone 与消费 Cone 中同一泛型实例的 source extern 调用边界差异：两端均调用原 provider 的 Scoop 薄入口，保持 safepoint 身份和根记录一致；release 的直接 C leaf 保留既有路径。
- 6 个正式 fixture 全部通过，锁定 18 份阶段 golden；覆盖空/单/大量 parts、别名返回、立即转换、重复 build、继续追加、跨扩容重入与抛出、普通泛型/默认值，以及删除源码后的产物链接。正例包含正常和 moving GC 运行。
- 执行 cargo fmt、全 workspace clippy 与 C 严格告警语法检查；MIR lowering 114 个单元测试通过。
- 4 个既有 List/String 正式回归 fixture 通过，包含跨 Cone 与 moving GC；13 份阶段 golden 校验通过，更新其中 4 份因 core 新增公开类型而变化的 HIR 依赖指纹。
