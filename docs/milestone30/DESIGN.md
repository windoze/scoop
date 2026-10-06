# M30 设计：Float / Double 与 128 位数值调研

状态：设计已记录，尚未实施。本次只修改文档；本文的完成门尚未执行。

日期：2026-10-06。

基线：已完成 M29 的仓库。依赖 M22 的定宽整数、literal fit 与 const，M23 的共有 HIR / 跨 Cone / `.slib`，M26 的字符串和容器，M28 的三种 target，以及 M29 的 annotation、静态描述和显式 codec。

对应[路线图](../ROADMAP.md)、[语言规范](../specs/SCOOP-SPEC.md) 9.1.2、9.3.2、9.4、11.2.2、11.11、11.13、13.8，[实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.18 和[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) 6.1。工具链实测及 128 位类型的可行路线单列于[调研记录](INVESTIGATION.md)。历史 milestone 文档保留原文，当前行为以同步后的 spec 为准。

## 0. 目标与边界

M30 正式交付 IEEE 754 binary32 / binary64 浮点值，完成源码、普通库、各级 IR、产物消费、链接与运行闭环。`Float` 与 `Double` 是 canonical nominal type，`Float32` 与 `Float64` 分别是透明 alias。

已确定的语义选择：

| 项目 | M30 决定 |
| --- | --- |
| 普通比较 | IEEE `== != < <= > >=`；NaN 无序，正负零相等 |
| 全序比较 | 显式 `isTotallyOrdered(belowOrEqualTo = ...)`，采用 IEEE `totalOrder`，包含全部 NaN 位型及正负零 |
| `compareTo` | Float/Double 不提供；浮点关系运算直接表达为 typed comparison |
| `Hash` | Float/Double 不实现；不因基本类型身份自动补齐 |
| 数值转换 | 显式转换；不做一般数值隐式提升 |
| 原始表示 API | 本次不公开 `toBits/fromBits`、byte sequence 或 `transmute` |
| Int128 / UInt128、Float128 | 本次仅调研，不纳入正式实现及完成门 |

正式范围包括十进制字面量、基础算术与分类、显式转换、const、字符串化，以及它们与泛型、默认参数、pattern、annotation、容器、GC、FFI、跨 Cone 和 JSON 的组合。不增加 FloatRange、PartialEq/PartialOrd/Eq/Ord 接口层次、通用数学包、任意精度数字、硬件 rounding mode API 或 Map key 专用规则。

目标用法如下，代码片段展示 M30 完成后的接口：

```scoop
val rate: Float32 = 0.1
val elapsed: Double = 1.5e2
val adjusted: Double = elapsed + rate.toDouble()

val nan: Double = Double.NaN
val equal: Boolean = nan == nan                              // false
val before: Boolean = nan < 0.0                              // false
val ordered: Boolean = nan.isTotallyOrdered(belowOrEqualTo = nan) // true
val zerosEqual: Boolean = -0.0 == 0.0                        // true
val zerosOrdered: Boolean = (-0.0).isTotallyOrdered(belowOrEqualTo = 0.0) // true
```

## 1. 类型、字面量与推断

### 1.1 类型身份与表示

core 增加两个显式 public 的 intrinsic struct，公开成员同样显式 public；声明没有普通字段或公开 primary constructor。它们显式 adopt `ToString`，不 adopt `Hash`。与整数一样，表示由 typed representation 绑定到真实的源码 nominal owner，不能按短名称、FQN、相同位宽或结构形状认定一个类型是浮点。

| canonical type | alias | IEEE 格式 | 有效精度 | exponent / fraction | size / alignment |
| --- | --- | --- | --- | --- | --- |
| Float | Float32 | binary32 | 24 bits | 8 / 23 bits | 4 / 4 bytes |
| Double | Float64 | binary64 | 53 bits | 11 / 52 bits | 8 / 8 bytes |

表中布局适用于现有 Darwin/AArch64、Linux/amd64 glibc 与 musl。每个位型都合法，包含 signaling NaN；不是只允许有限值的包装类型。alias 与目标在成员查找、泛型具体化、companion、布局、RTTI、mangle、ABI 和 ODR 上完全相同。

### 1.2 十进制语法

接受小数、指数和 `f/F` 后缀；至少出现其中一项：

```scoop
1.0
.5
1e3
1.25E-4
1f
1_024.5_0e+2F
```

- 小数点后必须有数字；`1.` 不是浮点 token。`1..2` 保持整数 range，`1.toDouble()` 保持整数 member access。
- 指数 `e/E` 后允许一个正负号，必须跟十进制数字。`_` 仅能放在同一数字段的两个数字之间，不能紧邻小数点、指数标记、符号或后缀。
- 本次不接受十六进制/二进制浮点或 `d/D` 后缀；已有的 `0x1f` 仍是整数。错误后缀、缺少指数数字及不合法 separator 在 lexer/parser 报到相应 token。
- 一元正负号保留为 AST 运算；`-0.0`、`-0f` 得到负零。解析器不能用整数零或普通空 struct 代替它们。

### 1.3 目标精度与 candidate-local fit

`f/F` 后缀固定为 Float；无后缀浮点字面量在 exact Float / Double 上下文中直接按目标格式舍入，无上下文默认 Double。规则用于 assignment、return、argument、默认值和 call/operator receiver，沿既有 8.6 fixed point、候选隔离和 winner commit 工作；普通 MSC 之后仍并列时，默认 Double commit 优先。浮点 literal 的有限候选只有两种 core 表示，不引入对任意用户类型的反向枚举。

AST 保存十进制有效数字、指数、后缀与来源。HIR 不能在探测候选前丢掉原始十进制精度，也不能让失败 probe 留下已提交的 f32/f64 值。直接舍入是可观察语义，例如：

```scoop
val direct: Float = 1.000000059604644775390625000001
val intermediate: Double = 1.000000059604644775390625000001
val roundedTwice: Float = intermediate.toFloat()
```

`direct` 是位型 `0x3f800001`，`roundedTwice` 是 `0x3f800000`。同一规则适用于 const、annotation、默认实参及 JSON 的 Float 解析。

舍入为最近偶数。有限字面量在目标精度下溢出为 Infinity 时诊断；下溢到 subnormal 或零合法。字面量不必能精确表示，例如 `0.1` 可直接适配 Float。算术/显式转换产生 Infinity 则按第 2 节正常执行，不能误套字面量溢出诊断。

已定型 Float 与 Double 之间不隐式转换，整数与浮点也不隐式混算。`val x: Float = 1` 是类型错误，使用 `1f`、`1.0` 或 `1.toFloat()`；`1f` 不能适配 expected Double。透明 alias 不属于跨类型转换。

## 2. 算术、转换与浮点环境

### 2.1 基础操作

每种表示提供 `unaryPlus/unaryMinus/inc/dec`、`plus/minus/times/div/rem/equals`、`isNaN/isInfinite/isFinite`、`isTotallyOrdered` 和第 2.2 节的转换。除比较/分类结果为 Boolean 外，算术 operand 与 result 都是 owner exact type；`inc/dec` 等价于同精度加/减 1。复合赋值与自增沿现有 place 单次求值协议。

`+ - * /` 每一步按目标精度最近偶数舍入，保留 subnormal。除零、溢出和无效运算产生 IEEE 的 Infinity / NaN，不抛 `ArithmeticException`：`1.0 / 0.0` 为正 Infinity，`0.0 / 0.0` 为 NaN。所有浮点表示操作，包括 `div/rem`，均可标记 `@NoGC`，不构造源码异常。

`%` 采用 C `fmod` / LLVM `frem` 的语义，商向零截断；余数符号跟随被除数。有限值对 Infinity 取余得到原值，Infinity 对有限值取余或对零取余得到 NaN；精确为零的余数保留被除数符号。不得使用 IEEE `remainder` 或 SoftFloat 的 `f128_rem` 代替该语义。

一元 `+` 是位型 identity，一元 `-` 翻转 sign bit，包括 NaN 和零。ordinary 算术产生的 NaN sign/payload 不保证在平台、优化及 const folding 之间一致；存储、传参、返回、复制和同类型转换保留既有位型。totalOrder 比较当前位型，不为尚未计算的 NaN 结果预测 payload。

### 2.2 显式转换

八种现有整数及两种浮点均提供 `toFloat()`、`toDouble()`；Float/Double 提供到八种整数的 `toInt8/toInt16/toInt32/toInt64`、`toUInt8/toUInt16/toUInt32/toUInt64`。这些是数值转换，不是位重解释。

| 方向 | 规则 |
| --- | --- |
| integer → Float/Double | 从源数学值直接按目标精度最近偶数舍入；区分 signed / unsigned |
| Float → Double | 有限值精确，保留零的符号 |
| Double → Float | 最近偶数舍入，允许溢出到 Infinity 或下溢 |
| 同类型浮点转换 | 保留全部 bits |
| Float/Double → signed integer | 向零截断，再饱和到目标 MIN/MAX；NaN → 0 |
| Float/Double → unsigned integer | 向零截断，再饱和到 0/MAX；NaN → 0 |

跨浮点格式转换的结果为 NaN 时，不固定 payload 映射。转换不抛异常，窄整数必须按自身范围饱和：例如 `1000.0.toInt8()` 得到 127，不能先转 Long 再按整数 wrapping 缩窄。既有 integer → integer 的模 `2^W` 规则保持。

### 2.3 默认环境与优化

采用 LLVM 默认浮点环境：round-to-nearest, ties-to-even、异常 trap 关闭、保留 subnormal。启动及线程 attach 建立此前提；native 代码改变环境后，应在重新进入 managed 代码前恢复。M30 不公开 fenv，不读取异常标志，也不引入每次运算或 FFI 的环境保存器。

编译器不得添加 `fast/nnan/ninf/nsz/reassoc/contract` 等放宽语义的 flag；不能把 `x * 0`、`x == x`、`!(a < b)` 等按实数恒等式折叠，也不能把独立乘加自动融合为一次舍入。普通 LLVM 优化仍可执行能保持本节语义的变换。

## 3. 普通比较、显式全序与 Hash

### 3.1 IEEE 运算符

| 情况 | `==` | `!=` | `<` | `<=` | `>` | `>=` |
| --- | --- | --- | --- | --- | --- | --- |
| 任一 operand 是 NaN | false | true | false | false | false | false |
| `-0.0` 与 `+0.0` | true | false | false | true | false | true |
| 其他值 | 按数值 | 相等取反 | 按数值 | 按数值 | 按数值 | 按数值 |

Infinity 参与正常次序。两个有限值的比较不使用误差容限；数值算法需要的近似判断由调用方按问题定义。

现有 operator 规则用 `compareTo(): Long` 的符号实现四种关系，但任何一个 Long 结果都无法表示“四种关系全部 false”。因此 Float/Double 的 `< <= > >=` 是由实际 typed representation 识别的直接比较，不生成 `compareTo` 调用；core 不为浮点另外提供 `compareTo`。其他类型继续沿原有 operator 规则。

`==` 仍调用 Float/Double 的成员 `operator equals`，`!=` 取反。已合法的泛型调用、alias、struct/tuple/enum 派生相等使用同一规则；含 NaN 的值可以不等于自身。派生相等不能改成字节比较、存储身份检查或统一 NaN 的值相等。也不增加 `Any.equals` 或根据装箱/静态类型改变浮点相等的规则。

需要 `compareTo` 的既有泛型协议不会因此自动接受 Float；需要对浮点排序的代码显式使用下一节的方法，不扩建整套比较 interface。

### 3.2 IEEE `totalOrder`

分别在两个类型上提供普通 public 方法：

```scoop
public fun isTotallyOrdered(belowOrEqualTo: Float): Boolean
public fun isTotallyOrdered(belowOrEqualTo: Double): Boolean
```

这是两个 owner 上各自的签名，不是一个 owner 同时接受两种精度。方法是 `@NoGC` typed intrinsic，返回 IEEE 754 的非严格 `totalOrder(a, b)`：

```text
negative NaNs < -Infinity < negative finite < -0 < +0
              < positive finite < +Infinity < positive NaNs
```

正 NaN 中 signaling 在 quiet 前、payload 从小到大；负 NaN 中次序反转，quiet 在 signaling 前、payload 从大到小。相同位型对自身返回 true。该方法不把所有 NaN 合并到末尾，也不合并正负零。

需要严格先后谓词的算法使用：

```scoop
fun precedes(a: Double, b: Double): Boolean =
    !b.isTotallyOrdered(belowOrEqualTo = a)
```

不能直接把非严格 `<=` 谓词作为严格排序比较器。方法命名采用 Swift 风格；无需同时增加三路 `totalCompare`、新的比较结果 enum 或额外 equality API。

实现无需浮点运算。对于 W 位 IEEE 表示，以 unsigned 整数读取 bits，令 `signMask = 1 << (W - 1)`：

```text
key(bits) = ~bits                 if sign bit is set
            bits ^ signMask      otherwise
totalOrder(a, b) = key(a) <= key(b)  // unsigned comparison
```

`~` 限定在 W 位内。该变换在现有 binary32/binary64 编码上同时实现数值次序、正负零次序及完整 NaN 次序；不触发 signaling NaN 运算、不调用 libm，也不依赖公开 bit-conversion API。

### 3.3 不实现浮点 Hash

Float/Double 与 alias 都不 adopt `Hash`、不提供 `hash()`。同步修订语言规范中“每个基本类型都必须提供 Hash”的旧要求；Boolean、Char、八种整数和 String 仍显式实现 Hash。

这项选择与 IEEE 相等、显式全序彼此独立。提供 totalOrder 不表示普通 `==` 成为等价于位型相等的运算，也不表示 Hash bound 自动满足。没有新的 Map key 禁令、特殊容器检查或装箱后 Hash fallback。

编译器自己的常量表、metadata、fingerprint 可以按 kind + bits 做 Eq/Hash；它们比较的是编译数据，不是向 Scoop 程序提供浮点 Hash。以后若有具体需求，再单独设计安全的位/字节转换或具有明确相等规则的类型；M30 不预置这些 API。

## 4. 常量与现有语言能力

### 4.1 companion 常量与 const evaluator

两个 companion 均提供下列同类型 `const val`，读取不触发 singleton 初始化：

| 名称 | 含义 | Float bits | Double bits |
| --- | --- | --- | --- |
| NaN | 正 quiet NaN | `7fc00000` | `7ff8000000000000` |
| POSITIVE_INFINITY | 正无穷 | `7f800000` | `7ff0000000000000` |
| NEGATIVE_INFINITY | 负无穷 | `ff800000` | `fff0000000000000` |
| MAX_VALUE | 最大正有限值 | `7f7fffff` | `7fefffffffffffff` |
| MIN_VALUE | 最小正 subnormal | `00000001` | `0000000000000001` |
| MIN_NORMAL | 最小正 normal | `00800000` | `0010000000000000` |

`MIN_VALUE` 不是最负有限值；后者是 `-MAX_VALUE`。这些是固定 binary32/binary64 值，不随 host `long double` 改变。core 可以用合法字面量及封闭 const 运算定义它们，不增加公开原始 bits 构造器。

const evaluator 扩展现有 typed intrinsic 集合，覆盖浮点算术、比较、分类、totalOrder 和显式数值转换；不执行普通用户函数、`toString`、codec 或任意属性 getter。`1.0 / 0.0` 在 const 中合法，整数除零仍是定义错误。浮点 const 算术产生 NaN 时选择表中的 canonical quiet NaN；位型 identity、sign 翻转及 totalOrder 仍按明确的输入 bits 工作。

选用独立于 LLVM 的 `rustc_apfloat` 实现目标精度的解析与计算，使用其目标舍入、`c_fmod` 和 bits 接口，不使用 host `f32/f64` 的普通解析或算术充当语言求值器。依赖版本与维护情况见调研记录；实现阶段锁定经验证版本，不让 HIR 获得 LLVM 依赖。

### 4.2 默认值、annotation 与 pattern

默认参数、字段 initializer、global/static initializer、const 导出及下游默认值展开都保留完整浮点类型、bits 和既有的定义/求值来源；负零不能因为“与零相等”被重新编码为正零。

M29 的 annotation 参数类型增加 Float/Double，值仍限于 literal、带符号 numeric literal 与同类型 const 引用；没有新的 CTFE 调用机制。参数按声明顺序存储 typed constant，读取跨 Cone annotation 时不按十进制文本再次舍入。

浮点 literal pattern 使用 subject 的 exact precision 与 IEEE 相等。两个不同文本若舍入成同一非 NaN 数值，覆盖同一个 singleton；正负零同样覆盖一个值，无 guard 的重复分支按既有 unreachable 规则诊断。浮点 pattern 列始终保留需 wildcard/`else` 覆盖的剩余域，不能因浮点格式的 bits 数有限而枚举其全域，也不能从 `isNaN()` guard 推导穷尽。递归 tuple、struct 与 enum payload 复用同一规则。

泛型具体化、扩展方法、callable reference、closure、default 与跨 Cone 调用仅需把两种表示接入现有类型通道；不新增浮点专用泛型 bound。对外的 member、ToString 与 codec 仍按实际声明和 interface application 选择，不能把没有普通字段的 intrinsic struct 当作空值、恒等的空 struct 或空 record codec。

## 5. 字符串化与 JSON

### 5.1 `ToString`

有限值按输入精度产生可读回相同位型的最短有效十进制数字。Float 使用 binary32 shortest conversion，不能先扩成 Double 再调用 binary64 formatter。最短数字由成熟的 Ryu C 实现生成，Scoop 的小型适配层固定文本形状：

- 对非零值，用规范化十进制科学指数 e 判断：`-3 <= e < 7` 用定点，否则用科学记数法。
- 输出包含小数点和至少一位小数；不补其他多余的小数零。科学形式小数点前恰有一位，指数使用小写 `e`、无正号、无多余前导零。
- `0.0`、`-0.0` 保留零的符号；NaN 一律为 `NaN`，Infinity 为 `Infinity` / `-Infinity`。文本不保存 NaN sign/payload。

例如 `1f` 为 `1.0`，`0.1f` 为 `0.1`，`1e7` 为 `1.0e7`，`1e-3` 为 `0.001`，`1e-4` 为 `1.0e-4`。输出不受进程 locale 影响，不提供格式精度、区域化或通用 format API。

runtime 后备只接受 scalar 并返回普通 String；Ryu 数字转换本身不分配 managed 对象，最终 String 分配沿现有路径。源码 `toString` 是普通 Managed core/helper 调用，不属于 const intrinsic。源码字符串插值、print/println 和泛型 ToString bound 直接复用该实现。

### 5.2 单值协议与 codec

M29 的 SingleValueEncodingContainer / SingleValueDecodingContainer 分别增加：

```scoop
public fun writeFloat(value: Float): Unit
public fun writeDouble(value: Double): Unit

public fun readFloat(): Float
public fun readDouble(): Double
```

这两个精度必须分开，格式实现才能选择直接解析 Float，而不被协议强制经过 Double。所有已有实现一并补齐方法并重新构建 itable；不为旧实现添加掩盖缺失能力的默认方法。

Float.Companion 显式实现 `Encodable<Float>` / `Decodable<Float>`，Double.Companion 对应 Double；body 只调用同精度 single-value 方法。alias 复用同一 companion。record/enum 自动派生按实际字段类型选择该 codec；泛型与核心容器继续显式传入元素 codec，不扩大 M29 的自动派生机制。

例如 `Json.decode("0.1", Float.Companion)` 直接得到 Float；含浮点字段的 struct 仍使用自己的显式 companion codec，类型无需实现 Hash。

### 5.3 JSON 数值规则

JSON number 节点继续保留原始文本。现有整数解析保持精确，不经浮点中间值；整数 codec 继续拒绝带小数或指数部分的 token。Float/Double codec 则接受合法 JSON 整数、小数和指数 token，直接舍入为所需精度。

编码仅接受有限值，复用第 5.1 节格式并保留负零。NaN / Infinity 抛带当前 path 的 EncodingException；语法解析不接受这些名称。目标精度解析溢出到 Infinity 时抛 DecodingException，subnormal 和下溢到带符号零合法。字符串、Boolean 和 null 不强制转换为浮点，字段缺失/default/Option 规则保持。

解析后备使用固定 C locale 的 `strtof_l` / `strtod_l`，分别直接解析 binary32 / binary64；不用 Ryu 的实验性或受限位数 parser，也不把任意长 JSON 数字先压入整数。实现时按三个 target 核实 locale API、正确舍入与链接要求，不调用 `setlocale` 改变进程全局状态。

JSON 库通过内部 Scoop-ABI extern 声明调用 runtime helper，无需把内部 core member 暴露为公共 API。输入已经由 JSON parser 验证语法，后备只做实际转换、完整消费与溢出判断；不能把 `ERANGE` 直接当成错误，因为合法下溢也可能设置它。String 不带终止 NUL，临时 C 缓冲区按实际长度申请并在调用内释放；None 表示不能得到合法结果，分配失败沿既有 fatal allocation failure。

后备借用 String，不回调用户代码、不进入 managed allocation，不从 C 抛 Scoop 异常；返回 Option<Float>/Option<Double> 并由普通 JSON body 构造 DecodingException。含 ref 的 Scoop-ABI 入口仍使用既有 Managed 边界，Option 结果按实际 sret 适配。源码没有新增 String 解析、字节 view 或 native buffer 所有权接口。

## 6. GC、布局、ABI 与数学库

### 6.1 统一值布局

Float/Double 作为普通 GC-free scalar 进入各值容器：

- Array/MutableArray 连续 inline 存储；element stride 分别为 4/8，长度和索引仍为 Long。
- struct、tuple、enum、class field、closure capture、协程 frame 和静态存储使用实际 size/alignment；混合引用字段按原有 scan/barrier 处理。
- Option<Float>/Option<Double> 使用现有 tagged enum；所有 NaN 位型合法，不能占用任何浮点位型表示 None。
- 装箱使用真实 Float/Double TypeDescriptor 和正常 box payload offset；从 Any 做检查/拆箱仍按 nominal identity，不能因为位宽相同与 Int/Long 互换。
- safepoint、异常展开、挂起/恢复和移动 GC 中的浮点 live value 沿普通 spill/frame 保存。它们不是 GC root，旁边的 managed reference 仍须正确保活和 relocation。

不修改 TypeDescriptor 的字段或 scan schema，不增加浮点对象头、特殊数组种类或新的 GC allocation path。

### 6.2 Scoop / C ABI

Scoop scalar 参数和返回值在 LLVM 签名中直接使用 `float` / `double`，由现有目标 ABI lowering 放入对应寄存器；aggregate 继续沿既有间接参数/结果规则。不要把浮点包装为空 struct，也不要为复用整数代码把其函数签名改成 i32/i64。

canonical C storage 增加 C `float` / `double`。extern 函数、global/TLS、`@CLayout` 混合 struct、Ptr/FunPtr、addressOf/load/store 与 foreignCallback 全部通过现有 generated-C bridge 连接；C 编译器继续负责 C 参数/结果的寄存器和 aggregate 分类。Scoop 自有 indirect ABI 与 C aggregate ABI 不能混淆。

所有 IEEE 位型在 FFI 入站合法，参数/返回/复制保存原始 bits，不添加对 NaN 的验证或正规化。需要观察 signaling NaN/payload 的 fixture 可在 C 中用同宽 unsigned integer 和 `memcpy` 构造、读回值，不要求新增 Scoop transmute。

### 6.3 libm 与 native support

LLVM 22.1 探针已确认 f32/f64 基础加减乘除可在现有目标生成机器浮点指令，`frem` 则会导入 `fmodf/fmod`。二者属于 NoGC native leaf；需要补齐已有 native-support 表和产物链接依赖，不能只确保 runtime 的 C 源码能编译。

| target / 链接配置 | M30 数学依赖 |
| --- | --- |
| Darwin/AArch64 | 实际 SDK 的 libSystem / system math exports |
| Linux/amd64 glibc 动态 PIE | glibc 配套 libm，沿现有最终链接方式 |
| Linux/amd64 musl 静态 executable | musl 配套静态库与归档顺序；保留现有 native runtime 支持依赖 |
| Linux/amd64 musl 动态 PIE | musl 配套动态 libc/math 链接 |

现有 Linux 链接命令已有 `-lm` 等支持项，实施时核实实际输入与符号闭包；Darwin 继续直接使用已选择的 system linker/SDK。不得混入 host 的不同 libc 或 LLVM 版本库，也不因 Float128 调研无条件添加 compiler-rt/libquadmath。

M30 不新增 sin/cos/log/exp、fma、sqrt 等数学 API；这不影响 `%` 对基础 libm 的必要使用。128 位基础算术 helper 与完整数学函数的职责区别见调研记录。

## 7. 各阶段改动与产物

按现有 IR 数据通道增加两种实际表示，不预建任意格式浮点框架：

| 层次 | 必要改动与输出不变量 |
| --- | --- |
| AST / parser | 共用十进制浮点 literal payload；表达式、annotation、pattern 保留来源和未舍入原值 |
| HIR / hir-lower | 绑定实际 intrinsic nominal owner，候选期直接目标舍入；完整 typed constant/operation；const、比较和转换全部在明确的封闭集合内 |
| MIR / mir-lower | 传播 typed kind，保持单次求值与 CFG；浮点除零无整数 throw 分支，派生字段相等不走身份捷径 |
| LIR / lir-lower | F32/F64 scalar、目标布局/ABI、常量/phi/内存/返回/静态初值完整；GC 只处理引用部分 |
| codegen | 精确 bits 常量、IEEE fcmp、正常浮点指令、饱和转换与 totalOrder 位操作；不再查名字或重新推断源码语义 |
| identity / meta / slib | typed kind、constant bits、C storage、实际调用与 ABI 正常持久化，旧产物按受影响版本重建 |
| core / runtime / json | 两个真正的 primitive 声明与公开成员、Ryu/解析后备、单值 codec、精确目标 JSON 解析 |
| toolchain / linker | 完成 fmod 支持符号、正确目标 math provider 与四种链接配置的真实运行 |

现有主要接入点包括 [整数 AST](../../compiler/ast/src/integer.rs)、[HIR 数值表示](../../compiler/hir/src/integer.rs)、[operator 决议](../../compiler/hir-lower/src/expr/operators.rs)、[MIR operator lowering](../../compiler/mir-lower/src/body/operators.rs)、[LIR 类型](../../compiler/lir/src/types.rs)、[目标布局](../../compiler/lir/src/target.rs)、[C storage](../../compiler/identity/src/entity/c_abi.rs)、[LLVM 类型](../../compiler/codegen/src/llvm_types.rs) 与 [native support](../../compiler/lir/src/production/c_bridge_support.rs)。浮点定义使用自己的小模块，integer kind 和源码 nominal id 不混用；只抽取本次真实共用的转换/常量逻辑。

常量使用类似 `F32(u32) | F64(u64)` 的封闭结构，必须携带实际类型；不能用一个宿主 f64、`Option` 类型字段或“以后根据上下文补齐”的值。LLVM 常量从同宽 integer bits 形成浮点 bitcast/精确常量，不用接受宿主 f64 的便利 API 建立统一常量路径。

LLVM 映射固定如下：

| Scoop 操作 | LLVM / 等价展开 |
| --- | --- |
| `+ - * / %` | `fadd/fsub/fmul/fdiv/frem`，无 fast-math flags |
| unary `-` / `+` | `fneg` / identity |
| `== != < <= > >=` | `fcmp oeq/une/olt/ole/ogt/oge` |
| integer → float | 按 signedness 的 `sitofp/uitofp`，直接目标精度 |
| float → integer | `llvm.fptosi.sat` / `llvm.fptoui.sat`，直接目标位宽 |
| Float ↔ Double | `fpext/fptrunc` |
| totalOrder / 分类 | 同宽 integer bitcast、位操作与 unsigned 比较 |

饱和 intrinsic 若需要展开，必须得到同样的 NaN、Infinity 和边界结果，不能使用越界可产生 poison 的裸 `fptosi/fptoui`；浮点常量与运行期转换共享这些语言规则。

Export HIR、MIR、LIR 及其 meta codec 都要完整编码新增实际 kind，source shape、默认值、annotation 与静态初值保存 raw bits。compiler-side 常量相等/哈希按 bits，不能把正负零合并或让 NaN 破坏元数据的自反性。reader 在正常产物边界检查 kind/位宽、类型引用和 ABI，后续 stage 复用已验证数据。

实际编码变更时提升受影响 section/profile 版本并使旧缓存重建，single-value 协议变更后相关 provider/consumer 一起重编。字段号和 tag 在实施时分配，本设计不预占；不新增 sidecar、来源证明、重复全图验证或跨阶段资源预算。runtime metadata ABI 4 不因增加两个无引用 scalar 自动升级。

## 8. 实施顺序

每批先更新必要 spec，再修改实现；完成代码变更后先 fmt/lint，随后运行有针对性的 fixture 和测试。一个阶段输出新增 kind 时，相应数据结构与必要 match 分支必须一起闭合，不能用空分支、TODO 或假造整数 carrier 让 pipeline 暂时接受不完整值。

| 批次 | 交付内容 | 本批可验证结果 |
| --- | --- | --- |
| A：标量纵向闭环 | 两种类型及 alias、literal/bits 常量、IR/meta/标量 ABI、基础存储和 Ryu ToString | 源码声明、调用、返回、打印两种精度；正负零和 nominal/alias 身份正确 |
| B：数值语义 | 算术、分类、IEEE 比较、totalOrder、全部显式转换、const evaluator 和 companion 常量 | NaN/Infinity/subnormal、舍入与饱和边界；const 与动态结果符合各自明确契约 |
| C：源码组合 | 完整 candidate-local fit、默认值、annotation、递归 pattern、派生相等与泛型调用 | 双重舍入回归、NaN 结构相等、来源诊断及 negative/golden |
| D：值容器与 FFI | arrays/aggregate/boxing/frame、C storage、global/TLS/Ptr/FunPtr/callback、native fmod 支持 | 混合布局、跨 safepoint/挂起存活和真实 C 双向调用 |
| E：序列化 | 单值协议、companion codec、目标精度十进制解析、JSON 组合与路径错误 | 有限值按位往返、整数精确性、拒绝非有限值和正确处理负零/下溢 |
| F：产物与总验收 | 跨 Cone、重导出/alias/ODR、独立 artifact-only link、四种链接配置及回归 | 正式 CLI 全链路运行，记录实际命令、结果与修复 |

Int128/UInt128、Float128 的调研不插入上述实现依赖，不把选择四倍精度库、增加目标或建立通用数学框架变成 M30 的验收条件。

## 9. 验收矩阵

围绕实际源码与产物闭环组织独立、组合、negative 和 HIR/MIR/LIR golden，不按修改文件数量堆重复测试。

| 类别 | 必需覆盖 |
| --- | --- |
| literal / 类型 | 小数、指数、后缀、separator、range/member 消歧；无上下文 Double、expected Float、固定 f/F、候选失败隔离、直接舍入及溢出位置 |
| 算术 / 环境 | 两种精度的正常值、正负零、normal/subnormal 边界、最大有限值、Infinity/NaN；fmod 的符号与特殊输入，默认无融合/无 fast-math |
| 比较 / totalOrder | 全部 IEEE 谓词，NaN 与自身，正负零；正负 signaling/quiet NaN 与多个 payload 的已知次序、相同位型及严格谓词；const/动态/generic/派生字段一致 |
| 转换 | 八种整数到两种浮点及反向；实际 MIN/MAX 邻域、unsigned 上界、NaN/Infinity、负数到 unsigned、窄类型直接饱和、f32/f64 双重舍入 |
| const / 源码组合 | companion 常量、const 依赖、导出默认值、annotation bits、递归 pattern、零的重复覆盖和穷尽诊断；String 插值与 ToString bound |
| 值存储 / GC | inline arrays、Option、mixed struct/tuple/enum、class field、boxing/unboxing、closure 与协程 frame；GC/异常/挂起期间保留浮点 bits 和相邻引用 |
| C / Scoop ABI | scalar 参数/返回、混合 @CLayout、extern global/TLS、Ptr stride/load/store、FunPtr 与双向 callback；C memcpy 制作 NaN payload 并观察完整传递 |
| JSON | 两种精度的有限值往返、原始长数字与小数/指数、直接 f32 舍入、正负零、subnormal/underflow、overflow 与路径错误；NaN/Infinity 拒绝、整数不丢精度、locale 独立 |
| 跨 Cone / 链接 | 依赖 core、公开 alias/转导出、generic/default/annotation 常量、不同文件的 ODR、artifact-only link/run、fmod imports、四种目标链接配置 |
| negative / golden | 错误 literal、混合数值类型、fixed suffix 与 expected 冲突、不存在的 Float.compareTo/hash、Hash bound 不满足、缺失单值协议实现；typed float kind/operation/ABI dump |

totalOrder 测试使用手工排列的 IEEE 分类与 payload 样本，不把同一个 key 变换复制进测试作为唯一 oracle。转换测试使用边界两侧可表示值；格式化/解析测试包括第 1.3 节的直接舍入反例。源代码不需要公开原始位操作就能通过普通 C fixture 检查全部位型。

正式完成门包括：

1. 按仓库约定先 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，再完成相应 workspace 测试；Python 有改动时先执行规定版本的 ruff。
2. 构建 `scoop`、`scoopc`、`scoop-linker`，运行 fixture runner 自身测试及 `python3 tests/run_fixtures.py --all`。每个目标记录实际完整运行范围，不以 `cargo test` 替代文件 fixture。
3. Darwin/AArch64、Linux/amd64 glibc 动态、musl 静态与动态实际链接并运行；核实目标 SDK/libc/math 支持与 artifact-only 消费。跨 target 的 llc 输出仅作为工具链探针，不算该目标运行通过。
4. 记录实际 spec/产物版本变更、测试命令与结果；未通过项继续修复，不用本设计或探针结果代替完成记录。

本次文档提交不宣称上述完成门已通过，也不提前生成验收结果。后续实施记录应以真实命令输出为依据。

## 10. 128 位调研结论与后续入口

Int128/UInt128 有清晰的后端路线：LLVM `i128` 加减乘可展开为较窄指令，除余和部分转换依赖 `__*ti3` 等 compiler-rt/libgcc builtins。当前 AST/常量值域、整数 kind、mangle/meta、Scoop/C ABI 和链接支持尚未形成 128 位闭环；不能只把 width enum 加一项就宣称完成。后续必须单独定义字面量范围与默认推断、全链路常量、转换和平台 ABI。

Float128 应明确指 IEEE binary128，不能使用 C `long double` 作跨平台替身。LLVM 有 `fp128` IR，不代表三个目标都有可用的基础 helper、数学库、十进制转换或一致的 C ABI；Darwin/AArch64 尤其缺少可直接采用的 `__float128` C 边界和现成 quad helper 集合。

后续可比较两条路线：先在具备 binary128 库与 ABI 的目标交付；或采用 SoftFloat 一类软件实现，以显式 128 位存储和 pointer/out-parameter helper 在各平台统一。compiler-rt 解决基础算术/转换的一部分，libquadmath/glibc math 解决另一部分数学与文本能力，不能彼此等同。原始探针、准确性/部署边界和未验证事项见[调研记录](INVESTIGATION.md)。M30 不选择或集成上述任一路线。
