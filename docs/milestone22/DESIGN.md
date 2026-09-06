# M22 设计：循环、值模式与定宽整数

版本：0.2（草案）

对应`docs/ROADMAP.md`的M22。M22在M16的统一constraint/MSC、M18的operator与`iterator`/`componentN` typed role、M20的exact invariant application、M21的typed access domain以及M25的自有异常ABI之上，补齐四条长期欠账：`for`/`break`/`continue`，struct副本更新，递归模式与裸enum变体，以及完整定宽整数/区间。

本里程碑首先是语义与IR正确性门。当前enum穷尽性只按variant名称计数，可能把`V(true)`误当作覆盖整个`V(Boolean)`并让MIR删除最后一个条件；当前`while`又把condition setup复制到循环尾，直接加入`continue`会跳过下一轮setup。M22必须先消除这两类不可靠结构，再扩展语法，不能只让parser接受新形式。

## 0. 关键决策与范围

- `Int`与`UInt`在所有Scoop target上永久固定为32位，`Int32`/`UInt32`分别是其透明别名；`Long`与`ULong`永久固定为64位，`Int64`/`UInt64`分别是其透明别名。四个canonical源码身份与Java/Kotlin的同名整数宽度一致，不由target决定；
- M22不引入platform-native integer。当前唯一可执行target profile仍要求data pointer与code pointer都恰为64位、两类null都对应内部carrier的全零位模式，并显式保证合法非null地址与内部carrier逐bit往返；原来借用64位`Int`/`UInt`的pointer offset、pointer integer carrier、`sizeOf`/`alignOf`等底层源码surface暂时改用`Long`/`ULong`。未来支持其他指针宽度或引入native-width类型时，再单独修订这些surface与FFI契约，不能让普通`Int`/`UInt`随target变宽。这不新增integer与`FunPtr`的源码转换；裸`Ptr`/`FunPtr`值固定非null，null只由`Option<Ptr/FunPtr>.None`表示；
- M22加入top-level、非generic、透明`typealias`。别名有自己的声明/可见性身份，但展开后不产生新的类型、layout、TypeDescriptor、重载签名或mangling身份；generic `typealias`仍留在backlog；
- 整数`+ - *`、一元负号、`inc`/`dec`和位运算采用按位宽定义的二进制补码wrapping语义。除零抛`ArithmeticException`；有符号`MIN / -1`返回`MIN`，`MIN % -1`返回`0`，不得把LLVM poison暴露为语言行为；
- 整数字面量在HIR winner commit前是candidate-local literal constraint，不是提前定型的`Int`。无后缀的无上下文默认阶梯为`Int`，超出后为`Long`；`u/U`阶梯为`UInt`，超出后为`ULong`；`L`与`UL`分别精确固定为`Long`与`ULong`。expected exact整数类型可接收domain内可表示的字面量；不存在非字面量的隐式整数转换或混宽运算；
- M22只实现不带标签的`break`/`continue`以及`for`；目标是同一callable内的最内层循环。`do-while`、带标签的`break`/`continue`/`return`不进入本里程碑；
- `for`只消费M18已有的普通`iterator` operator候选。其结果必须唯一满足某个exact `Iterator<T>` conformance；`next()`及`Option.Some/None`来自一次验证后保存的typed core contract，不按名称/FQN查找；
- `val`/`var`、lambda参数与`for`变量使用递归、不可失败的binding pattern；其中除内建`Unit` literal外的普通裸标识符恒为新binding，不查询enum/import/value，只有按subject exact enum解析成功的显式variant shape才按refutable pattern拒绝。`when`使用允许literal/variant的refutable match pattern，并保留enum subject下的variant-first裸名解析。`for (Some(x) in xs)`不是过滤语法，而是编译错误；
- 穷尽性统一使用递归constructor/pattern-matrix算法。enum variant的payload、Boolean、Unit、tuple和struct都递归参与覆盖；guard不贡献覆盖；缺失诊断给出一个稳定witness；
- 副本更新只支持具有声明字段的exact struct。任何enum目标（包括命名字段variant）都是稳定编译错误，必须用`when`匹配并显式重建；编译器不生成active-variant检查或`IllegalStateException`路径；
- 表达式位裸variant只由普通可见候选或唯一expected exact enum application引入，不扫描全程序全部enum；`Option`不再拥有另一套硬编码表达式解析；
- `IntRange`/`UIntRange`/`LongRange`/`ULongRange`及iterator由普通public core源码实现，四者都是不同的真实nominal type，不新增range runtime ABI。`CharRange`等到`Char`进入已实现子集后再加入；
- 为保持既有64位source/core API的值域，原来使用`Int`的通用契约整体迁为`Long`，包括Array size/index、`Hash.hash()`结果、integer及String的`compareTo`结果、shift count、`SourceLocation`、`@CLayout`与callback index等；原来使用`UInt`并依赖64位值域的契约（包括pointer raw与size surface）相应迁为`ULong`。ROADMAP已把尚未进入实现子集的String length/index/slice排在M24；这些API首次实现时直接使用`Long`，M22不为尚不存在的surface制造占位intrinsic。Array公开上限继续是数学上的`INT64_MAX`，不因新`Int`缩窄，也不要求core新增整数边界companion常量。编译器/runtime内部的enum tag、statepoint id、allocation size、count与offset继续使用各自独立的typed machine metadata，不能为了迁移源码surface而改成`Long`/`ULong`；
- alias、literal variable、字段名、source `for`、copy-update plan、未解析控制目标和pattern coverage plan都必须在`LocalConcreteHir`前消失或正规化为结构完备的typed节点。MIR不得重做名称解析、类型推断、variant选择或穷尽性判断。

## 1. 定宽整数与透明别名

### 1.1 类型身份

语义上的integer representation是封闭结构：

```text
IntegerKind = Integer { signedness: Signed | Unsigned, width: W8 | W16 | W32 | W64 }
```

它是core nominal struct声明的typed intrinsic representation，不取代nominal declaration/application identity。HIR/MIR中的一个integer type同时能到达唯一core struct实体和非可选`IntegerKind`；下游不能从短名称、操作数位宽或intrinsic字符串反推signedness。

core以如下身份提供八种表示：

| 表示 | canonical源码声明 | 透明别名 |
| --- | --- | --- |
| signed 8 | `Int8` | `Byte` |
| signed 16 | `Int16` | `Short` |
| signed 32 | `Int` | `Int32` |
| signed 64 | `Long` | `Int64` |
| unsigned 8 | `UInt8` | `UByte` |
| unsigned 16 | `UInt16` | `UShort` |
| unsigned 32 | `UInt` | `UInt32` |
| unsigned 64 | `ULong` | `UInt64` |

core中的alias声明精确为：

```kotlin
public typealias Byte = Int8
public typealias Short = Int16
public typealias Int32 = Int
public typealias Int64 = Long
public typealias UByte = UInt8
public typealias UShort = UInt16
public typealias UInt32 = UInt
public typealias UInt64 = ULong
```

因此`Int`与`Int32`不能形成两个overload，`Long`与`Int64`也不能形成两个overload；unsigned同理。在同一候选集中只按alias拼写区分的声明是重复签名。诊断、dump与mangling使用core为该identity登记的canonical拼写，alias拼写仅保留在source origin中用于诊断。

M22实现的`typealias`子集为：

```kotlin
public typealias Bytes = Array<UInt8>
private typealias Count = Int
```

- 只允许top-level、非generic alias；右侧必须是参数完整、已经解析的普通类型，可以是nominal application、tuple或函数类型；
- alias声明有独立`TypeAliasId`、visibility与source origin；使用处在类型检查前透明展开到目标type id；
- alias可出现在所有类型位置，也可作为构造器、companion或static nested member的type qualifier；最终候选仍属于真实目标实体；
- alias不产生新的nominal application、constructor、成员、layout、TypeDescriptor、RTTI、boxing种类、FFI classifier或单态化实例；
- alias展开图中的直接或间接环是定义处错误；完全展开目标type tree中每个被引用声明的effective access domain都必须覆盖alias自身的effective domain，并保存M21 signature-exposure witness。该规则同样约束internal alias，不能借它把file-private目标泄漏到Cone其他文件；
- Export HIR保存“alias声明→typed目标”的语义接口；真实`.slib`编码、跨Cone环检测与re-export由M23落地。generic alias继续按既有backlog拒绝。

### 1.2 字面量词法与定型

整数词法接受十进制、`0b`/`0B`二进制、`0x`/`0X`十六进制和位于数字之间的`_`。不引入八进制。后缀为：

| 后缀 | literal domain | 无其他约束时的默认类型 |
| --- | --- | --- |
| 无 | 任一能表示该值的signed整数 | 值适合32位时为`Int`，否则为`Long` |
| `u`/`U` | 任一能表示该值的unsigned整数 | 值适合32位时为`UInt`，否则为`ULong` |
| `l`/`L` | signed 64位 | `Long` |
| `uL`/`UL`及大小写组合 | unsigned 64位 | `ULong` |

`_`不能紧邻radix前缀、后缀或位于首尾；前缀后至少有一个合法数字。lexer保存无符号magnitude、radix、suffix和完整span，不再直接解析成`i64`。超过数学值`2^64 - 1`的magnitude在词法阶段给出可恢复诊断；其余范围判定在HIR候选中完成。

无后缀literal不能靠expected type变成unsigned；需要写`u`或显式转换。`L`/`UL`把domain限制为对应的64位type，因此`val x: Int8 = 1L`非法。负号仍是一元operator；为表示signed最小值，HIR允许AST上的`unaryMinus`直接作用于无`u`integer literal时，把magnitude `2^(W-1)`作为一个candidate-local边界事实提交。括号不形成语义节点，源码空白/注释不影响该关系。例如`val x: Int = -2147483648`与`val y: Long = -9223372036854775808`合法，而对应正值不适合各自type。无expected type的`2147483648`默认`Long`，`4294967296u`默认`ULong`；native-width integer不参与默认阶梯。

在M16 solver中，literal持有一个候选私有的可表示type集合：

- assignment/return/argument/call-or-operator receiver等位置给出exact expected integer type时，值可表示即产生`LiteralCommit`，否则该候选失败；
- literal作为integer representation intrinsic或本里程碑四个core range member的receiver时同样可延迟：HIR只枚举`IntegerTypeCore`中的八个canonical owner，再在假设owner上执行普通core-member决议。因而`1 + smallInt8`、`1 and smallInt8`与`1..smallInt8`都可由另一operand约束到`Int8`；不能先把receiver提交为`Int`再寻找成员。该封闭规则不枚举任意用户/extension member，也不把反向receiver推断推广给普通类型；
- 若固定点结束仍无expected type，无后缀literal按magnitude提交为`Int`或`Long`，`u`literal按magnitude提交为`UInt`或`ULong`；`L`/`UL`分别直接提交为`Long`/`ULong`。因此`id(1)`推导`Int`，`id(2147483648)`推导`Long`，`id(1u)`推导`UInt`，`id(4294967296u)`推导`ULong`；
- overload同时包含该literal按上述阶梯得到的默认类型与其他可表示类型时，默认exact目标更具体；只剩多个非默认可表示目标时互不支配并报告歧义，不采用阶梯之外的“最小能容纳位宽”作为隐藏排序；
- literal fit只是一次构造该type常量的能力，不建立`Int8 <: Int16`或数值coercion。已经定型的变量、call result、const引用与aggregate字段之间没有隐式宽化、窄化或signedness转换。

winner commit后literal成为封闭的`IntegerConstant::{Signed8(u8), Signed16(u16), Signed32(u32), Signed64(u64), Unsigned8(u8), Unsigned16(u16), Unsigned32(u32), Unsigned64(u64)}`之一；payload保存恰好W个低位raw bits，kind由variant得到，不能构造`Int8 + 任意高位u64`或kind/bits width不一致的状态。对应nominal type由下述total core map取得，不再平行存一个可矛盾的kind。同一typed payload还用于const值、annotation argument、default metadata、global/static initializer与下游constant image，不能让这些旁路继续保存裸`i64`。literal variable、magnitude约束与候选集合不得进入Export HIR的body接口或`LocalConcreteHir`。

### 1.3 运算、溢出与转换

内建integer的二元算术、逐bit运算和比较只在两个operand是同一canonical integer type时适用；literal可以按1.2直接提交到该type。shift是唯一例外：左operand/result保持其integer type，count固定为canonical `Long`，以保留M22前`Int`所提供的64位count契约。用户operator仍走M18普通决议，不继承primitive规则。

- `+`、`-`、`*`、一元`-`、`inc`、`dec`按`2^W`取模；一元`+`是同类型identity。signed结果按W位二进制补码解释，因此`-MIN == MIN`；
- signed/unsigned `div`与`rem`在除数为0时抛`ArithmeticException`。signed `MIN / -1 == MIN`、`MIN % -1 == 0`；其他signed余数与被除数同号；
- `and`、`or`、`xor`、`inv`直接作用于W位bit pattern；signedness只影响结果解释，不改变bit；
- shift的count参数为`Long`，实际count取其低`log2(W)`位。signed `shr`为算术右移，signed `ushr`与unsigned `shr`为逻辑右移，`shl`丢弃移出位；任何count都不能触发LLVM overshift poison；
- 比较按kind选择signed或unsigned次序；`compareTo`返回canonical `Long`的`-1/0/1`，`equals`仍要求同一type；
- HIR const evaluator、普通执行、default/annotation常量与优化后的结果必须逐项一致。源码显式整数方法调用只有在typed registry精确解析到对应representation intrinsic时才可进入const evaluator；同名用户callable与`toString`等普通方法仍不是常量表达式。常量`div`/`rem`除零是const定义错误；普通表达式即使operand为literal也保持运行期异常语义；
- unary plus/minus、inc/dec、add/sub/mul、compare/equals、bit、shift与conversion是GC-free且不抛异常，其core intrinsic声明必须显式带`@NoGC`，registry也把它们登记为typed NoGc target；这是language spec §13.1允许`@Intrinsic`与`@NoGC`共存的封闭例外。div/rem可能构造异常，声明不得带`@NoGC`且registry登记为Managed。MIR使用已经解析的`ArithmeticException`构造目标显式展开除零分支，并在signed `MIN/-1`路径直接产生规定结果；只有其余安全路径进入primitive div/rem。codegen不得为wrapping运算添加`nsw`/`nuw`，也不得自行补异常控制流。

每种integer提供固定宽度命名的`toInt8()`/`toInt16()`/`toInt32()`/`toInt64()`与`toUInt8()`/`toUInt16()`/`toUInt32()`/`toUInt64()`，并可提供`toByte()`/`toShort()`/`toInt()`/`toLong()`等源码友好名称的转发member。转换不抛异常：先把数学值按`2^targetWidth`取模，再按目标signedness解释；这同时定义identity、narrowing、signed/unsigned跨族和widening。alias返回类型仍是同一个目标identity。

### 1.4 layout、ABI与core能力

八种表示的size分别为1/2/4/8 byte，alignment使用target对相应整数标量的自然对齐；struct/tuple/enum/array/boxing/CLayout均消费该layout，不能继续把所有integer扩大为8 byte。

C ABI classifier精确映射到`int8_t`/`int16_t`/`int32_t`/`int64_t`与`uint8_t`/`uint16_t`/`uint32_t`/`uint64_t`；C bridge负责目标ABI要求的窄整数实参/返回extension与static assertion。Scoop ABI保留源码exact宽度。alias不形成第二种ABI分类。当前target profile必须验证data/code pointer均为64位、两类LLVM AS0 null的内部carrier均为全零，且合法非null data/code地址可经对应内部64位carrier逐bit往返；这只是`Ptr`/`FunPtr`表示与ABI的target qualification，不开放从任意同size integer构造pointer，尤其不改变非null `FunPtr`的来源限制。M22明确不据此宣称`Long`/`ULong`是platform-native type；它们只是当前64位profile上临时承接既有底层源码surface的固定宽度整数。

M22同时修正M12旧pointer surface的两处矛盾。`Ptr<T>`与`FunPtr<F>`分别改由`@Intrinsic("core_ptr") public struct Ptr<T : value>`及`@Intrinsic("core_fun_ptr") public struct FunPtr<F>`声明compiler-represented data/code-pointer family；二者都没有源码可见raw field或普通primary constructor，因而不能用字段读取、解构或copy update绕过pointer契约。`core_ptr`除源码value kind bound外还要求每个concrete pointee递归GC-free；generic模板中的未知T形成并传播typed `RequiresGcFreePointee`条件，不能把`T : value`当成证明。registry只为`Ptr<T>(raw: ULong)`提供`@NoGC @Unsafe`的call-shaped construction entry，立即正规化为typed `PtrFromNonZeroULong`；其前置条件是`raw != 0uL`，编译期常量零直接诊断，其他违反前置条件的行为未定义。公开integer extraction改为`Ptr<T>.toULong(): ULong`，`load`/`store`的element offset及`plus`/`minus`的offset改为`Long`，`sizeOf<T>()`/`alignOf<T>()`改为`ULong`；`PinnedPtr.raw`等实际地址carrier同样迁为`ULong`。这些迁名保留原64位值域；未来native-width设计至少必须重新审视这一组显式底层API，其他暂用`Long`/`ULong`的source surface是否迁移也在届时逐项决定。`FunPtr`没有任何源码constructor或integer accessor，`FunPtr<F>()`及所有带argument写法都失败，非null值只来自language spec §13.10的合法函数地址入口。全零data/code carrier只表示`Option<Ptr/FunPtr>.None`或inactive/zeroed storage，保证普通`Some(payload)`永不与`None`碰撞；可空FFI字段、参数与返回值等全部边界位置必须改用`Option`。本段明确取代M12的旧raw-field/null-constructor规则。

八个canonical integer struct、表1.1的全部alias与四个range type都显式声明`public`；所有用户可调用的integer/range成员也显式`public`，sysroot不依赖默认visibility。每个integer nominal声明通过普通core源码提供`equals`、`ToString`、`Hash`、同类型算术/比较能力与显式转换；既有`Hash.hash(): Int`统一迁为`Hash.hash(): Long`，保持原64位结果契约。`and`/`or`/`xor`/`shl`/`shr`是普通`public infix` member，`inv()`是普通零参数member；signed类型另有`public infix ushr`，unsigned的`shr`已经是逻辑右移且不另设`ushr`。这些名称不带`operator` modifier，也不扩展language spec §9.3.1的operator表。`Int8`/`Int16`/`Int`的字符串化可先sign extend到`Long`，`UInt8`/`UInt16`/`UInt`可先zero extend到`ULong`，再复用迁到`Long`/`ULong` owner的既有64位runtime后备；M22不新增按type name分派的format/hash runtime入口。

intrinsic登记仍是封闭且一对一的。登记key取`int8`/`int16`/`int`/`long`与`uint8`/`uint16`/`uint`/`ulong`：type name为`core_<key>`，representation-level member为`<key>_<operation>`，conversion为`<source-key>_to_<target-key>`。`operation`集合固定为所有kind共有的`unary_plus/unary_minus/inc/dec/add/sub/mul/div/rem/compare_to/equals/and/or/xor/inv/shl/shr`，以及signed kind独有的`ushr`。unary/`inv`零参数并返回owner；算术和逐bit二元参数/结果为owner；shift参数为`Long`、结果为owner；`compare_to`参数为owner、结果为`Long`；`equals`参数为owner、结果为`Boolean`；conversion零参数并返回target kind。typed登记使用互斥的`NoGcIntegerOperation`与`ManagedIntegerDivRem`类别：只有`div`/`rem`可进入后者，其余上述operation及全部conversion只能进入NoGc target，并与源码`@NoGC` presence逐项验证，不能再平行保存一个可能矛盾的effect字段。既有`core_int`/`core_uint`与`int_*`/`uint_*`名称迁为32位`Int`/`UInt` owner，64位owner使用新增`core_long`/`core_ulong`与`long_*`/`ulong_*`；alias转发成员使用普通body，不重复登记。HIR立即把字符串正规化为typed operation/conversion；不能让多个声明共享一个provider，也不能把登记字符串或未类型化effect传给MIR/codegen。

源码integer value与编译器内部enum tag、statepoint id、array allocation size、layout offset及component count等machine metadata使用不同typed kind。即使它们最终都是LLVM integer，也不能把内部计数器迁成`Long`/`ULong`来取得源码operator、boxing或FFI语义；源码API的迁名与内部表示迁移是两件独立的事。

### 1.5 既有64位source契约迁名

M22必须对compiler-recognized core/annotation surface做一次完整typed inventory：凡是M22前以canonical `Int`/`UInt`表达、契约值域确实是64位的既有声明，都迁名为`Long`/`ULong`；不能因某个实际值通常很小就让它随`Int`缩到32位，也不能靠alias、隐式conversion或backend extension维持表面兼容。至少包括：

| 契约类别 | M22后的source type |
| --- | --- |
| `Array`/`MutableArray`的`size`、`get`/`set` index与iterator index | `Long`；上限仍为`INT64_MAX` |
| `String.compareTo`；M24新增的length/size、index与slice boundary | `Long`；后者在M22尚未进入实现子集，物理UTF-8 byte count仍是独立machine scalar |
| `Hash.hash()`、所有integer `compareTo`结果、所有integer shift count | `Long` |
| `SourceLocation`的line/column | `Long` |
| `@CLayout`的`aligned`/`packed`参数及其他既有FFI annotation/index参数（例如callback `contextIndex`） | `Long` |
| `Ptr`的element offset与`plus`/`minus` | `Long` |
| `Ptr` raw constructor/`toULong`、`sizeOf`/`alignOf`及源码可见64位地址carrier | `ULong` |

这张表列的是source/core API，不规定对应HIR/MIR arena index、layout byte offset、allocation size、source span storage或runtime table count的宿主实现类型。那些值继续使用按职责区分的typed `u32`/`u64`/`size_t`或封闭newtype；不得为省事统一替换成源码`Long`/`ULong`。相反，用户显式声明的普通`Int`/`UInt`值以及M22新定义的32位integer/range API保持32位，不能把源码中所有`Int`token无条件批量替换为`Long`。

## 2. 循环、迭代与区间

### 2.1 `break` / `continue`

M22新增以下成功语法：

```kotlin
while (condition()) {
    if (done()) break
    if (skip()) continue
    work()
}

for (item in source()) {
    use(item)
}
```

- 不带标签的`break`/`continue`分别指向当前callable内词法最内层循环；循环外使用是编译错误；
- lambda、匿名函数与局部函数建立新的callable边界。即使文本上嵌在循环体内，其中的`break`/`continue`也不能跳出外层循环；
- 两者都是jump statement，不产生正常结果；其所在路径不参与外围控制表达式的结果类型合并，表现为semantic bottom，但不构造`Nothing`类型的表达式。`while`/`for`正常落空的语句结果为`Unit`；
- 成功位置限于块内完整语句、不带花括号的`if`分支体以及`when`箭头后的单条分支体。`value ?: break`、argument/initializer/receiver/operator operand、`(break)`及对应的`continue`形式属于未来的一般jump expression，M22必须给出稳定的未支持诊断；
- `break`跳到循环exit；`continue`跳到循环header：`while`重新执行完整condition setup和condition，`for`重新调用一次`next()`；
- 循环体内的`try`/`catch`/`finally`按照5.3执行离开目标之前的全部cleanup。finally正常结束后恢复原transfer；只有实际离开当前finally的return/throw/break/continue才替换旧transfer。在finally内部被catch或只退出其内层循环的控制流不会覆盖外部pending transfer；
- 本里程碑parser对`break@label`、`continue@label`、labeled loop与`do-while`继续给出正式的未支持诊断，不建立半实现的label id。

### 2.2 `for`协议与求值顺序

core contract为：

```kotlin
public interface Iterator<T> {
    public fun next(): Option<T>
}

public interface Iterable<T> {
    public operator fun iterator(): Iterator<T>
}
```

全部visibility必须显式写出；sysroot不享有M21规则之外的特权。两个application保持M20的exact invariance。

对`for (pattern in iterableExpr) body`，HIR按以下顺序工作：

1. 在循环外求值`iterableExpr`一次并保存；
2. 使用M18 `OperatorKind::Iterator`走普通member/extension candidate-local决议，调用winner一次并保存结果；
3. 在winner返回type的完整base/interface/bound闭包中收集core `Iterator<T>` exact application，去重相同diamond路径后必须恰有一个；零个报告协议不满足，多个不同`T`报告元素类型歧义；
4. 将结果按已证明的conformance物化为该exact `Iterator<T>`一次。value iterator需要装箱时也只装箱一次；
5. 每轮header通过已经验证的core `Iterator<T>.next` typed slot调用一次，以表示无关的typed `Some`/`None` variant分流；`Some`payload进入不可失败binding plan后执行body，`None`退出；
6. 普通落到body末尾或`continue`均回到下一次`next()`，`break`直接退出。

概念展开仍可写为：

```kotlin
val __iterator: Iterator<T> = iterableExpr.iterator()
loop {
    when (val __step = __iterator.next()) {
        Some(__element) -> {
            val pattern = __element
            body
        }
        None -> break
    }
}
```

但`loop`、temporary、`Some`/`None`名称都不是源码可见实体。实现必须使用hygienic typed local、loop id、variant id与interface slot。用户声明的同名变量、type或variant不能改变展开。

`iterator()`可以像M18其他operator一样是suspend；只有处于suspend callable且通过普通effect检查时才可选择，挂起点发生在首次迭代之前且该调用仍只执行一次。core `Iterator.next()`本身固定ordinary，M22不定义async iterator协议。

每次成功迭代建立一个新的不可重新绑定binding scope；closure捕获的是该轮binding的值，而不是后续迭代会覆写的共享隐藏`var`。binding/destructure中的component调用可分配、抛异常，并在suspend上下文中按其已选effect挂起；跨这些点的iterator与payload使用普通moving-GC root规则。

### 2.3 统一binding plan

`val`/`var`、lambda参数与`for`变量共用一个`IrrefutableBindingPlan`：

- 一个plan只接收一个完整subject并将其保存到immutable hidden temporary；lambda的一个composite pattern仍对应一个logical source参数、一个函数类型参数与一个typed Scoop ABI classification entry，annotation描述这个完整subject，不按叶binding flatten；物理entry仍可分类为`ElidedZst`/`Direct`/`Indirect`，closure environment与suspend continuation是各自独立的hidden参数；
- binding name、`_`与`..`不失败。除4.3内建`Unit` literal外，binding上下文任意深度的普通裸标识符恒建立新binding，不按subject exact enum、import或既有value做variant/value lookup；`val None = o`因此合法。`Unit`/`()`仍是literal并拒绝。只有显式`E.V`、`V()`、`V(...)`或`V { ... }`等shape按subject exact enum解析成功时才作为refutable variant拒绝；解析为struct的同形pattern仍可进入irrefutable plan；
- tuple/struct位置模式按位置从左到右递归。命名字段先解析为exact typed field id：完整shape及与coverage/layout共享的field mapping按声明顺序，运行期action entries保留源码显式字段顺序并逐项depth-first执行；`..`补出的wildcard只有shape、没有运行期动作；
- class位置解构复用M18 `componentN`普通operator resolution，每个实际位置选择唯一typed target；class没有声明式总元数，因此该模式不能含`..`，写出的N个位置精确调用`component1`至`componentN`。每次component结果先保存到新的immutable hidden temporary，再递归处理对应子pattern；
- subject、内建投影结果与component结果的hidden temporary全部immutable。lambda/for的用户binding不可重新绑定；`var`解构声明也只有最终用户可见叶binding是mutable，不能把共享subject或中间temporary建成`var`；
- 运行期严格按上述source-order、depth-first action顺序执行。component抛异常时保留此前外部副作用并跳过后续action；component挂起时把subject和全部已完成temporary保存在协程状态中，正常恢复后先保存本次结果再继续，不能重跑subject或已完成的投影/component；异常恢复等价于在原call点throw并跳过全部后续action。在ordinary上下文选择`suspend componentN`是effect错误。

现有仅在`val` lowering中特判class component解构的路径必须抽出，不能给lambda与`for`各复制一份resolver。

### 2.4 四种整数range

core提供普通`public final class IntRange : Iterable<Int>`、`UIntRange : Iterable<UInt>`、`LongRange : Iterable<Long>`与`ULongRange : Iterable<ULong>`。四者都是不同的nominal class，不是彼此的alias；各自的constructor与表示字段为core-internal，用户只能由下述public range函数取得，从而不能构造零step或方向/端点不一致的状态。这仍是普通class/constructor/member语义，不是compiler intrinsic或runtime opaque type。iterator实现也可以是internal class，但对外返回exact public interface。`Int32`/`Int64`等整数alias自然复用其目标owner对应的range，不另建`Int32Range`/`Int64Range`等身份。

每个canonical integer owner都声明public `rangeTo`、`rangeUntil`、`until`与`downTo` member，参数为同一owner，返回type按下表固定。窄整数在普通core body内扩为其32位同signedness owner；64位owner不经过32位range：

| endpoint owner | range结果与元素type |
| --- | --- |
| `Int8`、`Int16`、`Int` | `IntRange : Iterable<Int>` |
| `Long` | `LongRange : Iterable<Long>` |
| `UInt8`、`UInt16`、`UInt` | `UIntRange : Iterable<UInt>` |
| `ULong` | `ULongRange : Iterable<ULong>` |

以下是精确签名schema而非可直接编译的class body；实际普通core方法必须各自提供实现，不能因省略body而变成intrinsic或abstract：

```text
IntRange
  kind: public final class
  constructor/storage: core-internal
  implements: Iterable<Int>
  members:
    public override operator fun iterator(): Iterator<Int>
    public infix fun step(value: Int): IntRange
    public operator fun contains(value: Int): Boolean

UIntRange
  kind: public final class
  constructor/storage: core-internal
  implements: Iterable<UInt>
  members:
    public override operator fun iterator(): Iterator<UInt>
    public infix fun step(value: UInt): UIntRange
    public operator fun contains(value: UInt): Boolean

LongRange
  kind: public final class
  constructor/storage: core-internal
  implements: Iterable<Long>
  members:
    public override operator fun iterator(): Iterator<Long>
    public infix fun step(value: Long): LongRange
    public operator fun contains(value: Long): Boolean

ULongRange
  kind: public final class
  constructor/storage: core-internal
  implements: Iterable<ULong>
  members:
    public override operator fun iterator(): Iterator<ULong>
    public infix fun step(value: ULong): ULongRange
    public operator fun contains(value: ULong): Boolean
```

这些range operator是owner member，不是extension，因此候选层、遮蔽和override行为由language spec §9.3的member规则唯一决定。internal iterator class的`next`仍以显式`public override`满足public interface slot，但effective domain受owner限制。

同一canonical integer type的两个operand支持：

| 形式 | 方向与端点 | 结果 |
| --- | --- | --- |
| `a..b` | 升序、闭区间；`a > b`为空 | 按endpoint owner使用上表对应range |
| `a..<b` | 升序、右开；`a >= b`为空 | 同上 |
| `a until b` | 与`a..<b`相同 | 同上 |
| `a downTo b` | 降序、闭区间；`a < b`为空 | 同上 |

窄signed operand在选中的core函数体内显式扩为`Int`，窄unsigned扩为`UInt`，因此两个已定型为`Int8`的endpoint形成的range元素类型是`Int`；这不是语言级隐式conversion，也不允许混宽operand。`Long`/`ULong` endpoint始终形成对应64位range，不先缩窄。literal可按1.2直接提交到另一个operand的type。

`range step k`返回相同range type并保留方向与端点；四个range的step与contains参数分别为自己的元素type，step要求`k > 0`，否则在求值时抛新增core `IllegalArgumentException`。默认step为对应type的1。`contains`仅在值位于端点范围并与first的步长对齐时返回true；差值与步长判断必须使用不会发生源码整数overflow的比较/余数结构。

iterator必须以显式“是否还有下一个元素”状态结束：产出当前端点后，先判断下一步是否会越过边界或溢出，再决定结束或更新current；不能依赖wrapping后恰好命中sentinel。四种range分别覆盖完整32/64位端点；从signed数学最小值到最大值的range、endpoint等于`2^64 - 1`的`ULongRange`单元素range以及最大端点上的`downTo`都不会死循环或漏掉最后一个值。M22不借此引入`MIN`/`MAX` companion常量。

`Array<T>`与`MutableArray<T>`在M22补上普通public `Iterable<T>` conformance及`public override operator fun iterator(): Iterator<T>`；既有array `size`、`get`/`set` index与iterator index从旧canonical `Int`迁名为canonical `Long`，逻辑长度上限继续为`INT64_MAX`，按既有bounds与moving-GC规则访问。range/array iterator、`until`、`downTo`、`step`与`contains`都来自core源码和普通operator/infix/member决议，不增加编译器按类型名识别的语法能力。

## 3. 副本更新

语法为非空字段列表：

```kotlin
val moved = point.{ x: point.x + 1, y: nextY() }
```

共同规则：

- `baseExpr`先求值且仅求值一次，结果保存于typed temporary；base必须是具有声明字段的exact struct value；enum、class、interface、tuple、basic type、intrinsic value family与函数值非法；
- 每个更新项必须是直接字段名；列表至少一项且不能重复。更新表达式在确认目标后按源码从左到右各求值一次，并检查可赋给字段exact type；
- 最终按字段声明顺序构造完整新值；未更新字段从base temporary读取，已更新字段使用对应temporary。原值与其storage从不原地修改；
- 任何base/update表达式的异常或挂起遵守普通求值规则，已经发生的外部副作用不回滚；跨调用存活的base及含ref更新值进入正常root/relocation plan；
- `opt?.{ ... }`继续非法；必须先用`when`显式拆包。

字段只由base exact struct identity与源码字段名确定，不用更新表达式类型反向选择目标。HIR在建立field candidate、lower RHS或commit plan之前确认base是exact declared struct；enum及其他非struct目标立即产生同一类稳定诊断。即使某个enum命名字段variant恰好包含全部所写字段，也不建立variant候选。

HIR内部的`CopyUpdatePlan`原子保存base temporary、exact struct application、经definition检查的typed field identity、声明顺序field mapping、源码顺序RHS与结果type。winner与类型检查完成后，它只展开为普通temporary、field projection与compiler-only raw `StructConstruct`；`LocalConcreteHir`不保留字段字符串或source `CopyUpdate`节点。该路径不产生`VariantTest`、`VariantPayloadProject`、`VariantConstruct`或compiler-generated throw。

供`for`、`when`等enum consumer使用的两个表示无关primitive，其typed契约固定为：`VariantTest(operand, variant) -> Boolean`，以及`VariantPayloadProject(operand, variant_field) -> exact field type`。其中`variant`同时封闭保存exact enum identity与已经按该enum定义检查过范围的variant index，`variant_field`再封闭保存该typed variant与已经按同一variant payload检查过范围的field index；MIR与LIR分别拥有本stage的两种typed reference，相邻lowering只做穷尽id映射，调用点不能散落`enum id + u32`、裸field index、名称或FQN回退。两个primitive的operand必须是引用所指向的exact enum；projection的结果type只能由该field定义导出。

`VariantPayloadProject`只允许在对同一value与同一typed variant执行的`VariantTest`为true的控制流分支支配下消费。这个条件是IR合法性的一部分，而不是依赖producer“通常先检查”的约定；MIR module validation与LIR module validation都必须拒绝错误enum、越界variant/field、错误结果type或缺少对应true-edge支配证明的projection。MIR primitive不暴露tag、null或payload offset；LIR取得concrete representation后才把test与projection机械解释为tagged discriminant或niche/null操作，并保留managed/raw/code pointer provenance。

## 4. 模式与enum变体

### 4.1 binding pattern与match pattern

M22正式区分两种上下文，而不是笼统称为“赋值位置”。parser在两种位置都产出同一封闭`PatternSyntax` AST；下列文法描述结合subject type并完成name/shape解析后的成功HIR语义分类。binding位置若写literal或显式enum variant shape，仍先保留完整AST与span，再在构造`BindingPattern`时产生稳定诊断，而不是由parser提前删掉：

```text
BindingPattern = Binding | Wildcard | Positional(BindingElement*)
               | Struct(FieldBindingElement*)
BindingElement = BindingPattern | Rest
FieldBindingElement = Field(BindingPattern) | FieldRest

MatchPattern   = Binding | Wildcard | Literal
               | EnumVariant(VariantPayload) | Positional(MatchElement*)
               | Struct(FieldMatchElement*)
MatchElement   = MatchPattern | Rest
FieldMatchElement = Field(MatchPattern) | FieldRest
VariantPayload = Unit | Positional(MatchElement*)
               | Fields(FieldMatchElement*)
```

`Field(...)`包含显式`field: pattern`；字段shorthand在AST中精确展开为同名的普通pattern syntax，RHS仍按所在上下文的完整规则分类。binding上下文通常把它分类为同名binding，但名称`Unit`仍优先成为内建literal并被拒绝，必须写成`Unit: value`等显式不同绑定名；match上下文也先应用Unit literal特例，再对其他名称执行variant-first分类。`FieldRest`在同一字段列表至多出现一次且只能位于末尾。`EnumVariant`始终保存已经解析的variant id及unit/位置/具名字段三种互斥payload shape，不能把具名enum payload伪装成struct pattern。`Rest`不是可独立出现的pattern，只能出现在position/variant element list或字段列表中，并遵守language spec §4.6的数量与位置约束。源码`Positional`在subject type确定后分类：binding位置可成为tuple投影、struct投影或class component plan，match位置只能成为tuple/struct投影；class component只属于binding位置，不能把可能有副作用的`componentN`调用放入match或coverage算法，且其element list不允许`Rest`。`val`/`var`、lambda与`for`只接受递归irrefutable的`BindingPattern`；`when`的模式分支接受`MatchPattern`。一个外层tuple/struct不能用嵌套literal或variant绕过binding限制。单variant enum仍是sum constructor，M22不为它引入特殊的“声明式可失败解构”。

两种上下文对裸标识符的分类刻意不同。除4.3的内建`Unit` literal外，`BindingPattern`中的普通裸标识符在任何深度都直接形成`Binding`，绝不尝试解析同名enum variant、import或value；`Unit`/`()`仍按literal拒绝。只有`E.V`、`V()`、`V(...)`与`V { ... }`等显式shape按subject exact enum解析成功时，才会被识别为refutable variant并拒绝；解析为struct时仍是irrefutable struct pattern。`MatchPattern`先把`Unit`/`()`分类为内建literal；其他裸标识符再按subject exact enum查询，unit variant形成variant pattern，payload variant缺少shape时直接诊断，只有名称未命中时才形成catch-all binding。同名enum variant `Unit`因此必须限定为`E.Unit`或写出payload shape。lambda参数的parser文法按language spec §8.1.2为`PatternSyntax [':' Type]`，成功语义节点必须是`BindingPattern`；一个composite pattern保留一个logical source/function-type parameter identity和一个typed ABI classification entry，annotation属于完整subject，物理payload分类以及closure environment/suspend continuation按language spec §4.7/§8.2另行处理。

### 4.2 命名字段子模式

字段模式元素统一为：

```text
field             // 精确展开为 field: field；RHS按所在context分类
field: pattern    // RHS是完整递归子模式
..                // 仅末尾一次，补齐未列字段为wildcard
```

因此`S { f1: 0, nested: (x, _), ignored: _, .. }`在`when`中合法；`{ f1, f2: renamed }`中的`f1`仍精确展开为`f1: f1`，其RHS再按binding或match上下文分类。`{ Unit }`同理展开为`{ Unit: Unit }`并命中内建literal特例，不会建立名为`Unit`的binding。`_`不能作为字段key，但可作为`field: _`的RHS。字段名必须存在且不能重复；所有binding名称在整个pattern内唯一。未列出全部字段时仍必须以`..`结尾。

AST的`FieldPattern`保存`field: Ident`与boxed完整`subpattern`，不再保存只能表示rename的`Option<Ident>`。HIR先把每个源码字段解析为subject exact type中的封闭typed field identity：struct字段使用`AppliedStructFieldRef`，enum payload字段使用`AppliedEnumVariantFieldRef`（或类型上同样隔离的封闭sum），不得用可混用的裸index。随后同时建立两种共享同一identity的顺序：按声明顺序补全wildcard的完整field vector供shape检查、match coverage与layout映射使用；按源码显式字段顺序保存的binding action entries供运行期depth-first执行。`FieldRest`只向前一种表示补wildcard，不生成后一种action。MIR不按名称查字段，也不得从声明序vector反推运行期binding顺序。

### 4.3 表达式位裸variant

qualified `E.V(...)`始终按普通variant constructor解析；显式/default star import引入的variant继续进入普通可见候选。除此之外，HIR只在存在expected exact enum application `E<Args...>`时，额外建立该enum中同名variant的最低优先级contextual candidate：

- unit variant可作为name expression，payload variant可作为call candidate；
- contextual unit variant只在普通value-name lookup没有找到任何实体时启用。词法value binding是hard shadow：例如局部`val None: Int`存在时，`val x: Option<String> = None`报告普通类型错误，不回退到`Option.None`；
- expected type若还是candidate-local inference variable，bare enum variant（包括`None`）可以像lambda、空数组一样postpone，待固定点把它唯一绑定到exact enum application后再检查；
- expected为`Any`、interface、多个不可比较enum或最终未解变量时，不扫描所有enum猜目标，诊断要求写`E.V`或显式type annotation；
- payload variant的contextual layer服从M16/M18既有named-call、local-value shadow、receiver、import与property-like分区，只在普通resolver允许继续到下一候选层时参与。按语言规范9.3.4，最近词法作用域中同名的callable-valued local形成hard shadow；普通不可调用local不参与callable层，也不阻止继续检查普通callable及contextual enum层。其他普通同名可见callable若在既有层无可应用candidate，才进入contextual enum层；
- winner commit产生exact enum application、variant id、完整argument mapping和constructor target；LocalConcrete前不保留variant字符串或“以后看expected”的节点。

`scoop.core.Option.*`通过prelude的typed default-import条目工作；编译器不再写`Some`/`None`短名称特判。未来M23只需把同一候选层换成真实import metadata。

match pattern位variant仍由subject exact enum type解析。未解析为该enum variant的裸标识符保持catch-all binding；为防拼写错误，M22补上spec既有的非致命warning，并在诊断中建议variant全限定名或显式`_`/binding命名。该variant-first规则不进入4.1定义的binding上下文。

### 4.4 递归pattern matrix

每个无guard arm正规化为按subject type排列的一行constructor matrix：

- enum的每个variant是一个sum constructor，payload字段递归形成列；unit variant元数为0；
- tuple与struct各有一个product constructor，字段按位置/声明顺序展开；命名字段和`..`先补成完整vector；
- `Boolean`有`false`/`true`两个有限constructor，`Unit`有唯一constructor；
- binding、`_`、rest补位和`else`都是wildcard；
- fixed-width integer是有限literal域。实现把已出现的singleton literal与一个符号化`OtherInteger(kind, excluded)`constructor做区间/集合分割；仅当不同无guard literal数小于`2^W`时`OtherInteger`仍有实例，不得实际枚举整个值域。因此列全256个`Int8`值可以证明穷尽，少一个则给出真实缺失值；
- String是无限开放literal域，有限arm集合只有在路径上存在wildcard时才穷尽。稳定witness依次选择未被排除的`""`、`"a"`、`"aa"`等合法源码literal并按既有转义规则打印；Char/Float/Double literal coverage不在M22实现范围；
- guard行完全不加入coverage matrix，因为guard可能为false；它仍按源码顺序参与运行期匹配；
- enum coverage不仅要求出现每个variant，还要求该variant的payload子矩阵递归穷尽。例如`Some(0), None`不覆盖`Option<Int>`，`V(true), V(false)`才覆盖`V(Boolean)`；
- tuple/struct允许由多行组合覆盖，例如`(true, _), (false, _)`，不再要求存在单行全wildcard。

实现使用标准usefulness/specialization算法或等价的有限constructor递归，必须终止于typed pattern shape而非枚举运行期值。integer domain size与去重计数使用能表示`2^64`的内部宽度，signed witness按数学值从最小值到最大值选择首个缺失值并使用无后缀/一元负号源码形式，unsigned从0到最大值选择并始终打印`u`后缀；这样witness本身也能按subject exact type重新解析。其余顺序为enum声明顺序、Boolean `false`后`true`、product字段顺序，例如打印`V(false)`或`S { flag: false, .. }`。

match pattern允许unary minus直接作用于integer literal（括号不形成语义节点，空白或注释不改变规则），并以subject的exact integer type复用1.2的fit与signed `MIN`边界；不接受其他常量表达式伪装成literal pattern。带`u`形式按普通wrapping unary-minus语义折叠，因此例如UInt8 subject中的`-1u`与`255u`是同一个constructor。

HIR在类型检查后同时产出穷尽proof与typed decision plan；plan只含表示无关的variant/literal test、typed payload/field projection、binding、guard、arm与fail edge。显式`else`形成wildcard proof。只有持有proof的default edge才能标记为`Impossible`；MIR不得因为“最后一个arm”或“每个variant名字出现过”删除条件。非法/不穷尽的`when`不输出可供MIR误用的残缺plan。MIR保留typed `VariantTest`/`VariantPayloadProject`直到LIR取得tagged或niche layout。

## 5. IR与stage边界

### 5.1 AST / parser

AST新增或修订以下封闭节点：

```text
IntegerLiteralSyntax { magnitude, radix, suffix, span }
TypeAliasDecl { visibility, name, target, span }
Statement::For { pattern, iterable, body, span }
Statement::Break { span }
Statement::Continue { span }
Expr::CopyUpdate { base, NonEmpty<FieldUpdate>, span }
FieldPattern { field, subpattern: Box<Pattern>, span }
```

`Statement::Break`与`Statement::Continue`是完整的statement variant，不得另增`Expr::Break`/`Expr::Continue`、伪造`Nothing` type id或用缺失expression type表达控制转移。parser除普通块语句外，还在不带花括号的`if`分支体和`when`箭头后的单条分支体接受这两个statement，并把它们包装为相同的block statement结构；其他expression子位置以当前语法单元为恢复边界产生正式未支持诊断，残缺节点不得进入AST。这样`val x = if (done()) { break } else { 1 }`仍由唯一正常分支定型，而`value ?: break`在M22中明确失败。

所有出现整数token的AST入口（普通表达式、annotation argument及其他受限常量语法）都复用`IntegerLiteralSyntax`或等价的同一payload，不能保留平行的`AnnotationArg::Int(i64)`。

`for`的`in`、range `..`/`..<`与pattern rest `..`按所在语法位置解析；copy update在postfix `.` 后看到`{`时进入专用字段列表，不与block或member access混淆。parser保留非法label/generic alias的完整span并以当前statement/declaration/list为恢复边界；有diagnostic时仍按全局规则丢弃残缺AST。

### 5.2 Export HIR / LocalConcrete HIR

HIR新增以下内部能力：

- alias table以`TypeAliasId`保存visibility/origin/target，并在所有type/qualifier入口统一展开；
- integer literal variable作为M16 constraint atom，winner commit一次性产出exact nominal type与width-matched `IntegerConstant` variant；
- `IntegerTypeCore`以对八个`IntegerKind`的total map保存各自唯一canonical nominal owner；缺项、重复owner或owner kind不符都使core contract失败，不能只在registry Vec中假定“应该齐全”；
- `IterationCore`非可选地保存Iterator template、`next` interface slot、Option template及Some/None variant id；core contract缺失或visibility/signature不符时整个模块不能进入MIR；
- 每个source `for`另有不可缺失的`ForIterationPlan`，原子保存source temporary、唯一iterator call及result type、exact `Iterator<T>` application、具体conformance/boxing witness、specialized next slot、exact `Option<T>`与Some/None variant、element type、binding plan和`LoopId`。`IterationCore`不能代替该use-site witness；
- source loop建立不可跨callable的typed `LoopId`栈；break/continue在HIR即绑定目标，不以整数深度或nullable label表示；
- 组合式控制流分析使用`ControlOutcome::{Fallthrough, Return, Throw, Break(LoopId), Continue(LoopId)}`集合而非“是否落空”bool；sequence只把Fallthrough送入下一语句，分支取union，每个loop消费指向自己的Break/Continue，finally按实际离开目标决定是否替换进入它的outcome。该分析sum不等于MIR的normal `PendingTransfer`，其中Throw仍只沿异常边传播；
- `IrrefutableBindingPlan`供val/lambda/for共用，并原子保存单一typed subject、immutable hidden temporaries、声明序field shape mapping、源码序depth-first actions、typed component winner与叶binding mutability。整个递归plan的名称引入、field identity解析、component resolution、effect检查与type检查属于同一个compile-time lowering事务：全部成功时只commit到紧邻的拥有者事务；普通已选语句可由此进入scope与永久arena，但overload candidate中的lambda等嵌套plan必须继续保持candidate-private，直到外层MSC winner commit才进入永久状态。任一错误只把该路径选定的稳定诊断交给拥有者并回滚全部暂存实体，不得留下部分binding或残缺plan。`CopyUpdatePlan`、contextual variant candidate和pattern matrix同样只存在于HIR lowering事务；失败candidate不得污染永久arena；
- Export HIR为public alias、integer intrinsic representation、public Iterator/Iterable/range surface、pattern所需typed declaration shape提供M23可打包的完备接口。generic body里的for在template中保存完整`ForIterationPlan`的typed协议/loop结构，具体化只替换已有typed引用，不得重新按名称找core。

`LocalConcreteHir`可以保留结构化typed loop与已绑定`Break(LoopId)`/`Continue(LoopId)`，但source `For`必须已展开为一次iterator初始化、重复typed next/Option branch、binding plan和loop。while与for统一具有显式header区域；所有进入新一轮的边都指向header。它不包含alias type、literal variable、source字段名、unresolved variant、coverage matrix、source copy update或未绑定控制转移。

上述compile-time事务性不提供运行期回滚。成功编译后的binding plan一旦开始执行，subject只求值一次；component调用之前已发生的外部副作用在后续throw时保留，throw跳过剩余action。挂起保存subject与已完成的immutable temporary；正常恢复点位于该component call之后，异常恢复则等价于在原call点throw，两者都不能重新执行已经完成的步骤。

### 5.3 MIR CFG、cleanup与协程

MIR lowering为每个loop建立显式`header/body/exit`目标。`while`形态必须从“在body尾复制condition setup”改为：

```text
preheader -> header(condition setup; condition)
                  | true  -> body ----+
                  | false -> exit     |
continue -----------------------------+
```

for的header对应next call与Option branch。普通body fallthrough与continue都进入同一header；break进入exit。这样M17 safe-call/default/argument sink产生的condition setup每轮恰好执行一次。

当前return专用cleanup stack泛化为**正常控制转移**的abrupt routing。每个loop target记录建立时的cleanup depth；return/break/continue只执行当前位置到目标之间被退出scope的cleanup suffix。普通throw与M25 rethrow继续沿typed unwind/catch/cleanup edge传播，不能被改写成词法goto或丢失native exception record identity。正常路径内部pending transfer是封闭sum：

```text
PendingTransfer<R> = Fallthrough(ResumeTargetId)
                   | Return(R)
                   | Break(LoopExitTargetId)
                   | Continue(LoopHeaderTargetId)
```

每个离开active catch的正常edge恰好执行一次M25 `EndCatch`；每个跨越finally的edge恰好进入一次对应cleanup。target仍在同一scope内时不得误执行外层cleanup。执行finally期间保留旧pending transfer与“已进入cleanup”的cursor：内部被处理完的throw或只指向该finally内部loop的break/continue不替换旧值；只有实际离开当前finally的abrupt transfer才替换它，并从cursor之后继续路由，不能重新进入同一个finally。

closure conversion后再做coroutine transform的既有顺序不变。pending break/continue若跨可挂起finally，以tagged frame字段保存typed resume target；挂起本身不退出scope或触发cleanup。M10在suspend handler中已把native exception物化并结束catch后，frame使用扩展的`SuspendedPendingTransfer = PendingTransfer | Throw(ManagedThrowable)`；这不是普通throw/rethrow的替代路径。恢复后finally正常结束再分派原transfer，不能保存native block address、原生EH状态或未初始化payload。

最终MIR只有普通CFG branch/goto/throw、typed integer operation、表示无关的`VariantTest`/`VariantPayloadProject`与literal test、aggregate projection/construction及call；不含source loop、pattern matrix或copy update。integer div/rem已经展开为除零throw、signed边界结果与安全primitive operation；shift count已经mask并在窄操作数上显式转换为后端要求的宽度。decision plan的`Impossible` edge只能来自HIR proof。

MIR的typed variant reference由stage-local enum id与该定义构造时已经检查过范围的variant index组成；typed variant-field reference再封闭保存前者与已检查的field index，两者都不能单独从裸index构造。`VariantTest`的结果固定为canonical Boolean，`VariantPayloadProject`的结果固定为variant-field引用所指field的exact `MirTypeId`。module validation除检查operand与result外，还验证每个projection只在相同operand/variant test的true-edge支配区域中出现；此验证不读取或推测tag/niche layout。

M22同时封闭所有compiler-owned enum producer，而不只封闭上述consumer primitive。Export HIR中的variant constructor/default owner、需要exact generic application的`VariantConstruct`及unit-enum constant image都保存经对应definition store检查的variant/field ref；LocalConcrete与MIR逐stage重建自己的checked ref，不能平行携带enum id与裸ordinal。MIR enum definition非可选地保存该单态实例的canonical type arguments，`OptionCore`原子保存Some payload field与None variant；module validation据此检查construct result的完整enum application、payload arity/逐字段exact type，以及global constant image每个递归leaf的owner与payloadless约束。

compiler生成的callback与coroutine metadata也遵守同一规则。callback family原子保存mode、state与`Option<Throwable>` failure的全部typed role，bridge只保存属于该family的mode ref；`CoroutineStep<R>`与`CoroutineSlot<T>`分别保存Completed/Value payload field和Suspended/Empty variant。兼容保留的协程`EnumTag`/`EnumField` reader可以读取layout ordinal，但ordinal只能由这些metadata accessor导出，producer不得写死或按名称重建。

### 5.4 LIR / codegen / FFI

- LIR scalar type扩展为`I8/I16/I32/I64`；expression、const、annotation/default metadata、global initializer与constant image中的integer constant都显式携带kind/type，不能再由`Value::IntConst`或任一未类型化`i64` metadata payload默认推断I64；signedness由typed operation/FFI classifier携带，因为LLVM integer type本身不编码signedness；
- arithmetic、Boolean compare、安全的division/remainder、已正规化的shift与conversion使用封闭`IntegerOp { kind, ... }`或等价typed variant。`compareTo`使用独立的`IntegerCompareTo { operand_kind, result_type: I64 }`（或同样不可错的结构），该I64结果对应canonical `Long`，不能按lhs宽度构造结果。shift count输入同样固定为`Long`/I64。codegen只机械选择LLVM指令；除零、`MIN/-1`与shift count normalization已经由MIR表达，不能在此按opcode补分支/掩码；
- layout、constant/global image、boxing、array element、enum payload、callback/C bridge及mangling全部消费exact integer kind；C type tree增加八个精确classifier，不允许退回`Int/UInt`两项；
- `docs/milestone23/DESIGN.md`第3.3节的persistent-identity mangler接管前，M22 compact type code与schema兼容边界由impl spec 2.3的`compact-v2`表唯一定义；artifact/cache使用封闭`ManglingSchemaIdentity::CompactV2`，M23对应独立的`PersistentV1`，不能只比较版本数字。alias先展开，internal machine scalar不得取得source compact code；
- HIR、MIR、LIR分别拥有自己的C-layout value、constant image与static-initial-state IR类型，并由相邻lowering穷尽转写typed id；不能跨crate type-alias或从bits/任意对齐整数重建语义。layout前没有通用`Zero`/裸`i64`常量旁路，layout后的LIR `EncodedStaticValue`只携带canonical allocation-extent bytes与typed relocation，`ZeroedForRuntimeUnit`不由encoded zero bits反推；
- C bridge type tree把void严格限制在function result，并为data pointer同时保存`OpaqueVoid | Object(CType)` pointee与direct/nullable storage shape：只有`Ptr<Unit>`使用`OpaqueVoid`，其他pointee必须是non-ZST portable C object type。C-layout struct与nullable data/code-pointer enum使用fully concrete refined LIR ref，struct field tree不递归内联；by-value struct dependency必须无环，pointer edge只需forward declaration。C extern/callback/global及C-layout field不再平行保存可矛盾的`LirType`；
- MIR的`VariantTest`/`VariantPayloadProject`在LIR取得concrete enum layout后，分别机械降低为tagged discriminant或niche/null test与对应payload projection；`for Option`与`when`共享该路径；
- LIR为上述primitive重新建立本stage专有的checked variant reference与variant-field reference，前者封闭保存`EnumDefId`与合法variant index，后者封闭保存前者与合法field index；MIR→LIR只映射这些typed identity，不重新解析名称。`VariantTest`直接产生`I1`；`VariantPayloadProject`只携带variant-field reference，结果必须是layout中该field的exact `LirType`，并且只可在匹配test的true-edge支配下消费。tagged表示机械比较discriminant并按variant slot/field offset投影；niche表示按目标variant选择null或non-null测试，payload variant的投影复用carrier值。managed、raw data与code pointer carrier始终保留各自provenance。checked constructor与LIR/module/codegen verifier必须拒绝错误enum、越界variant/field、错误结果type、错误pointee provenance或缺失支配关系；codegen不新增runtime ABI，也不补任何source语义分支；
- structured MIR/CFG MIR的`VariantConstruct`、LIR `EnumWrap`以及三阶段constant image直接携带各stage checked variant ref。相邻lowering只映射ref，MIR与codegen的统一module validation门必须在dump、layout或发射前拒绝错误owner、错误exact result、payload arity/type不符及带payload variant伪装成unit constant；
- callback runtime的mode/state `uint32_t`是独立wire code，不是enum tag。codegen只能经validated callback family把mode role编码为`0/1`，并把state的`0/1/2/3`穷尽解码为Registered/Active/Completed/Failed typed variant；任何其他返回值进入显式fatal/trap block，不能零扩展后直接写入语言enum storage；
- MIR CFG的每条循环回边，包括continue edge，必须经过LIR的typed managed poll；可共享header poll，但不能存在绕过poll的回边。root liveness覆盖iterator、range、payload、copy-update temporary及finally pending transfer；
- alias在HIR后没有运行期表示；range和array iterator是普通core nominal type/call，不加入LIR专用range instruction。

## 6. core、runtime与异常边界

M22修改`scoop.core`源码与core contract，但不新增必须由C runtime实现的主要ABI：

- 补齐八种integer representation及其普通public能力、transparent alias、public Iterator/Iterable、四种真实`IntRange`/`UIntRange`/`LongRange`/`ULongRange`、internal iterator实现、Array/MutableArray conformance；
- 把Ptr/FunPtr core contract迁为无公开representation field的intrinsic family，保留唯一typed unsafe `PtrFromNonZeroULong`入口并删除`FunPtr()`；`toULong`、`Long` offset与返回`ULong`的`sizeOf`/`alignOf`共同锁定临时64位底层surface，旧fixture中的裸零pointer改用`Option.None`；
- 新增普通managed `IllegalArgumentException`作为`Exception`子类，供非法range step使用。它不是compiler主动构造的异常，因此不进入`CompilerExceptionCore`；
- `Int8`/`Int16`/`Int`与`UInt8`/`UInt16`/`UInt`的ToString可在core先扩为`Long`/`ULong`后调用既有后备，equals/算术/layout由typed intrinsic实现；所有`Hash.hash()`返回`Long`，不增加按短名称选择的runtime switch；
- `Long`/`ULong`的`toString`继续由普通core member body调用迁名后的既有typed Scoop-ABI runtime后备，不把会分配/GC并返回`String`的格式化塞入`IntegerOperation`；integer equals统一降低为typed compare，不再以`scoop_rt_int_equals`/`scoop_rt_uint_equals`等旧owner命名helper作为生成代码契约，M22迁移完调用点后删除这些公开helper；
- wrapping overflow不抛异常、不分配，也不需要runtime helper。range终止和signed division edge在生成代码/core逻辑中显式处理；
- 新增value boxing与range/iterator对象都沿用现有TypeDescriptor、RefScan、statepoint、moving-GC及异常record协议。

## 7. 诊断与恢复

至少提供以下稳定诊断：

- malformed radix/underscore/suffix、magnitude超过`2^64 - 1`、literal不适合expected type、signed/unsigned后缀错误；
- 多个仅靠literal fit可用且无默认exact winner的overload歧义，显示每个候选与所需literal commit；
- mixed-width或mixed-signedness primitive运算，并建议显式`toX()`；
- generic/nested/循环typealias、alias环、alias target不可访问、alias导致重复overload；
- loop外或跨lambda/local function的break/continue、label与do-while当前未支持；
- `iterator` role缺失/歧义、返回type零个或多个不同`Iterator<T>` conformance、core协议损坏；
- for/lambda/val中的literal/显式variant shape等refutable pattern；componentN缺失/歧义、ordinary上下文的suspend component，以及字段/rest/重复binding错误；裸variant同名标识符本身不是binding位置的refutable pattern；
- copy update空列表、non-struct base（包括任何enum）、未知/重复struct字段与字段type mismatch；enum在字段候选或RHS lowering之前按统一非法base诊断拒绝，不存在variant候选或运行期mismatch路径；
- 裸variant无expected enum、错误expected type、import/context候选歧义及constructor argument失败；
- 不穷尽`when`显示稳定missing witness；guard不计覆盖时明确提示；疑似拼错的enum variant catch-all binding产生非致命warning；
- `Ptr`的非GC-free concrete pointee/未能传播并证明的generic predicate/`ULong`常量零构造、`FunPtr`任意constructor、访问不存在的Ptr/FunPtr raw field、对二者做解构/copy update、非函数FunPtr type argument及非法地址来源分别给出稳定诊断；不得回退成普通零字段struct构造；

parser按当前literal、字段列表、pattern、for header与statement同步，不让一个缺失逗号吞掉后续arm/statement。HIR的alias/literal/operator/variant/iterator/copy-update候选都采用事务式probe；存在error时不输出残缺Export/LocalConcrete module。warning不阻止输出，但必须进入fixture snapshot且拥有稳定severity/span。

## 8. 测试计划

### 8.1 integer与alias

- 八种kind及全部alias共享/区分正确type identity：`Int`/`UInt`为I32 owner且`Int32`/`UInt32`只作alias，`Long`/`ULong`为I64 owner且`Int64`/`UInt64`只作alias；alias constructor/qualifier、visibility、环和重复overload；
- 十进制/二进制/十六进制、underscore、四类suffix、每种min/max/越界及负最小值；
- `18446744073709551615u`按默认阶梯成为`ULong`，并在const、annotation、default metadata、global/static initializer与constant image中均保持`Unsigned64`的全64位raw payload；`18446744073709551616u`稳定报越界；
- 既有unsigned fixture中的常量改用`u`后缀（例如`const val UNSIGNED: UInt = 3u * 4u`），并保留无后缀literal不能由UInt expected type吸收的negative回归；
- assignment/return/generic/array/operator receiver/argument中的expected fit；`smallInt8..1`与`1..smallInt8`双向literal receiver/argument fit；无上下文`Int → Long`/`UInt → ULong`边界、`L`/`UL`精确类型、默认候选优先和非默认多候选歧义；
- 每种width的add/sub/mul/neg/inc/dec wrapping、bit/shift count、signed/unsigned compare、除零、`MIN/-1`与`MIN%-1`；所有unsigned kind的unary minus均有回归；const与runtime逐项相同；
- `Int8`/`UInt8`等任意宽度operand的`compareTo`在HIR/MIR/LIR golden与最终ABI中都返回canonical `Long`/I64，shift count也始终为`Long`/I64；
- 既有64位source contract的typed inventory逐项锁定：Array size/index与`INT64_MAX`、String `compareTo`、`Hash.hash()`、`SourceLocation`、`@CLayout(aligned/packed)`及callback `contextIndex`都使用`Long`，对应旧`UInt` carrier使用`ULong`；传入新32位`Int`/`UInt`不能静默适配。String length/index/slice由M24首次实现并直接采用`Long`，不属于M22验收面；
- 所有转换、equals、ToString、Hash、boxing、interface/generic单态化；
- struct/tuple/enum/array/CLayout的size/align/scan及C extern/global/callback bridge从`int8_t`到`uint64_t`往返；
- LLVM IR检查wrapping路径不含`nsw`/`nuw`，每个`sdiv`/`srem`只在除零与`MIN/-1`守卫后的safe block出现；target-profile artifact同时检查data/code pointer均为64位、两类null carrier全零，并分别验证合法非null AS0 data/code地址经内部carrier逐bit往返；
- `Ptr<GC-free value>`及generic deferred predicate正向、`Ptr<含ref value>`与`Ptr(0uL)`negative，以及`FunPtr<F>()`/任意argument构造negative；`Ptr(raw: ULong)`/`toULong`、`Long` offset和`sizeOf`/`alignOf: ULong`逐项锁定，32位`Int`/`UInt`不得作为隐式carrier。Ptr/FunPtr字段访问、解构与copy update也必须失败。`None`分别生成Raw/Code provenance的typed null，`Some`的payload保持裸pointer非零不变量。另覆盖非函数FunPtr type argument、合法顶层`@NoGC` callback及bare-vs-Option C ABI往返，并锁定HIR/LIR golden。

### 8.2 control、iteration与range

- while/for普通、嵌套和empty路径；break/continue绑定正确target，condition setup在continue后重新执行；
- member/extension/generic/suspend iterator operator，一次source/iterator求值，零/多Iterator conformance；
- Array/MutableArray、四种真实整数range与用户Iterable；array size/index/iterator index保持`Long`且在`INT64_MAX`边界不截断，每轮fresh binding及closure capture；
- tuple/struct/class component解构；命名字段故意逆声明序书写并嵌套有副作用component，以锁定源码序depth-first动作；throw保留先前副作用并跳过后续动作，挂起恢复不重复subject/已完成temporary/component；ordinary上下文选择suspend component为negative；
- lambda composite pattern在有/无expected type下的完整subject annotation；一个pattern只对应一个logical source/function-type参数及一个typed ABI classification entry，且不按叶binding flatten；`var`只让用户叶binding mutable，所有hidden temporary保持immutable；
- closed/open/until/downTo/step/contains的空、单元素、方向和alignment；MIN/MAX端点不溢出或死循环；非法step抛IllegalArgumentException；
- try/catch/finally内外break/continue、finally覆盖、每个EndCatch恰好一次；suspend finally恢复pending target；
- ordinary/continue回边都有poll，moving-GC stress下iterator/array/payload跨call被正确relocate。

### 8.3 copy update与pattern

- struct/generic struct/nested update的base一次、RHS源码顺序、声明顺序重建与原值不变；
- unit、位置payload与命名字段payload enum均锁定同一个non-struct base诊断；即使命名字段集合唯一匹配也不能建立copy-update plan、lower RHS或生成variant test/projection/construction/throw；
- 含ref字段的struct覆盖普通、异常、挂起与moving-GC路径，锁定base及更新temporary的root/relocation；
- 字段shorthand、literal/`_`/nested tuple/struct/enum subpattern、rest补全、重复/未知字段与binding；按typed field identity得到的声明序shape/coverage vector与源码序binding actions分别锁定；
- binding位置的`val None = o`及嵌套裸`None`建立新binding，`Unit`/`()`以及字段名为`Unit`的shorthand仍按literal拒绝（显式`Unit: value`可绑定），显式`E.None`/`None()`/payload variant shape在val/var/lambda/for中稳定报refutable；match位置的裸`None`仍按subject exact enum解析variant，并覆盖字段shorthand RHS恰与Unit literal或unit/payload variant同名的情况；
- enum payload递归覆盖、Boolean product组合、nested enum/tuple/struct、guard不计覆盖；enum/tuple/struct中的一个`Int8`列由全256个literal覆盖时可穷尽、少一个给真实witness，unsigned witness带`u`后缀，String开放域要求wildcard；
- 专门回归单variant `V(Boolean)`只有`V(true)`必须报缺失`V(false)`，且MIR不得把该arm无条件化；
- annotated return/argument/generic expected下的裸unit/payload variant、普通/import/context层优先级、无expected失败和Option无特判；
- missing witness与suspicious catch-all warning的span/text稳定。

新增fixture建议分为：

- `tests/fixtures/m22-integers/`；
- `tests/fixtures/m22-control/`；
- `tests/fixtures/m22-copy-update/`；
- `tests/fixtures/m22-patterns/`；
- 各目录的`errors/`与必要warning fixture。

AST、Export HIR、LocalConcrete HIR、MIR、LIR golden分别锁定source facts、typed winner、source plan消失、cleanup CFG和精确integer width。至少一个组合fixture串联fixed-width range → for destructuring → recursive when → copy update → try/finally continue/break → suspend/moving-GC stress；需要stress的fixture加入runner显式白名单。按本节明确迁移unsigned literal的旧fixture后，M1至M21及M25全部回归。

## 9. 实现顺序与提交门

1. 同步language/runtime/impl spec和ROADMAP，固定`Int`/`UInt`为32位owner、`Long`/`ULong`为64位owner，把全部既有64位source/core API迁名到`Long`/`ULong`并保持内部machine metadata独立，同时固定wrapping、binding/match分类与M22范围；
2. 落地非generic transparent alias、八种typed integer representation及Ptr/FunPtr intrinsic family迁移，先贯通layout/mangling/boxing/ABI与nonzero niche不变量而不增加新整数运算；
3. 改造literal AST与M16 candidate-local fit/default/MSC，随后补齐integer core operator、转换、const folding及LLVM poison边界；
4. 扩展字段subpattern，落地递归pattern matrix/typed decision plan，先修复现有enum误编译风险；
5. 实现contextual bare variant与copy-update HIR plan/正规化；
6. 加入typed loop target，重构while header与通用abrupt-transfer cleanup，完成普通及suspend finally/EndCatch组合；
7. 实现for协议、共享binding plan、Iterator/Iterable、Array conformance与range core；
8. 完成全部negative/warning/golden、ABI artifact、moving-GC组合及全量回归。

每个编号都是可独立提交且workspace保持可构建的切片。每批变更先执行`cargo fmt --all`与`cargo clippy --workspace`，再运行对应crate测试、stage golden和fixture；不能留下把旧`Type::Int/UInt`继续解释为64位的旁路、与新IntegerKind并行的未类型化integer通道、Option专用裸variant、return专用cleanup、按variant名计数的穷尽旁路或source plan泄漏到MIR。

M22只有在以下条件同时满足时完成：八种integer及alias在类型/layout/ABI/const/runtime中一致且total core map无缺项，`Int`/`UInt`不再残留64位解释；literal只在winner commit定型并遵守32→64默认阶梯且没有隐式数值conversion；既有array/String/Hash/compare/shift/SourceLocation/FFI annotation/pointer/size等64位source契约完整迁名为`Long`/`ULong`，Array上限仍为`INT64_MAX`，内部machine metadata没有伪装成源码integer；四种range具有独立nominal identity；全部wrapping/div/shift边界无LLVM poison；裸Ptr/FunPtr不存在全零构造或公开representation旁路且Option niche可区分；while/for每个continue走正确header和poll；break/continue跨catch/finally/suspend恰好执行所需cleanup；for只使用typed exact Iterator协议且source只求值一次；struct copy update的base一次、RHS源码序与声明序重建闭合，所有enum目标稳定报错且不生成state-check/throw路径；递归pattern matrix能给出sound proof/witness且MIR只信任该proof；所有source计划在LocalConcrete/MIR边界按本设计消失。

## 10. 明确不做

1. `do-while`、loop label、labeled break/continue/return及Kotlin inline lambda的non-local control flow；
2. async iterator、sequence/collection框架、iterator fusion或range loop专用优化；
3. platform-native integer：`ISize`/`USize`、`IntPtr`/`UIntPtr`是否分立、其与target profile的关系，以及哪些暂用`Long`/`ULong`的pointer、size/index或其他source surface最终迁移，都留待后续设计；不能改变普通整数自身的固定宽度，也不能在M22预判迁移范围；
4. `Char`/Float/Double及`CharRange`，以及任意宽度整数/BigInt；
5. 非字面量隐式数值提升、mixed-width arithmetic、C integer promotion模拟、checked/saturating arithmetic语法；这些可由未来显式API提供；
6. generic/nested typealias、跨Cone alias打包/re-export与跨Cone cycle检查；后者属于M23；
7. refutable/过滤式for pattern；需要过滤时在body中显式`when`/`continue`；
8. 全局扫描enum来猜裸variant、expected为Any/interface时的反向动态选择或按variant短名称恢复typed identity；
9. array `==`、完整smart cast、sealed exhaustiveness、or-pattern/range-pattern与一般lint框架；M22只补spec已要求的enum catch-all warning通道；
10. 实现任何enum或class copy update、为struct copy update生成in-place mutation、引入class `copy`约定或Option safe-update语法；
11. M23的真实`.slib`/import/re-export与M24的String byte API/字符串插值。M24依赖的是`UInt8`/`UByte`，不是signed `Byte`。
