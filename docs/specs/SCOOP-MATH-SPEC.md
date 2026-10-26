# Scoop 数学库规范

状态：首版已实现。日期：2026-10-26。验收范围与限制见 [实施记录](../math/PROGRESS.md)。

本文件定义普通库 `scoop.math` 的公共合同。存储与算法取舍见 [math 设计](../math/DESIGN.md)，使用方法见[可运行示例](../../sysroot/src/scoop.math/examples/README.md)。语言、对象和产物分别遵守 [语言规范](SCOOP-SPEC.md)、[运行时规范](SCOOP-RUNTIME-SPEC.md)、[实现规范](SCOOP-IMPL-SPEC.md)；命名遵守 [库 API 风格](SCOOP-API-STYLE.md)。

## 1. 范围与依赖

提供显式依赖的 library Cone `scoop:scoop.math:0.1.0`，源码目录为 `sysroot/src/scoop.math/`，公共 package 为 `scoop.math`。生产直接依赖 core 和 std；测试依赖 `scoop.test`。core、std 不反向依赖 math，math 不进入 prelude，也不改变 single-file 模式的默认依赖。

功能范围参考 Java SE 26 的 [Math](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/lang/Math.html) 与 [java.math](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/math/package-summary.html)，按 Scoop 类型和调用习惯组织：

| 领域 | 本库内容 |
| --- | --- |
| 定宽整数 | 大小比较、绝对值、受检算术、定向整除及余数、乘法高位 |
| 浮点 | 常量、初等函数、舍入、融合乘加、相邻值与指数操作 |
| 任意精度整数 | `BigInteger`、算术、位运算、模运算、平方根、素性测试和素数搜索 |
| 任意精度十进制 | `BigDecimal`、显式精度、舍入、scale、比较和文本转换 |
| 小型配置 | `MathContext` struct 与 `RoundingMode` enum |

不提供 `Math` 静态工具 class。标量函数直接导入 package，大数操作为成员与运算符。既有 Float/Double 分类、bits 转换、整数基础运算、集合比较器继续使用原实体。

随机源及 PRNG 保持在已有 `scoop.random` 中；math 没有 `random()`、默认 RNG、seed、熵源或 RNG 实现。素数相关 API 需要抽样时显式接收既有 `Random`，不改变该接口，也不让 std 依赖 math。

首版不包含矩阵、复数、分数、统计分布、任意精度超越函数、Decimal128 存储类型、Java 序列化兼容层或第二套 `StrictMath` API。

## 2. 共同约定

- `BigInteger`、`BigDecimal` 是普通、final、不可变的 managed class。所有运算返回结果，不修改接收者；`var x; x += y` 通过既有 `plus` 与重新绑定工作，不声明 `plusAssign` 等就地更新成员。
- 两类显式实现同类型的 `Equals<T>`、`Compare<T>`、`Hash`、`ToString`，以及 `Add<T,T>`、`Sub<T,T>`、`Mul<T,T>`、`Div<T,T>`、`Rem<T,T>`、`UnaryPlus<T>`、`UnaryMinus<T>`、`Increment<T>`、`Decrement<T>`。`compareTo` 返回 Long 的 -1、0、1。不建立 Number、Numeric、ArithmeticProvider 等新层次。
- 计数、位位置、移位距离、十进制 scale 和 precision 使用 Long；radix、signum 和素性置信参数使用 Int。标量指数函数 `getExponent`/`scalb` 使用 Int，足以覆盖 binary32/binary64 的指数域。
- 已定型数值不隐式提升。大数间不自动混算，大数与定宽整数也不隐式混算；使用构造器或显式转换。普通数字字面量不取得 BigInteger/BigDecimal 类型。
- 除零、算术结果无法按入口合同表示、需要舍入但指定 `Unnecessary` 时抛 `ArithmeticException`；非法参数抛 `IllegalArgumentException`；文本格式错误复用 `scoop.text.NumberFormatException`。
- `parseOrNone`/`toBigIntegerOrNone`/`toBigDecimalOrNone` 只转换约定的文本失败；非法 radix、用户提供的 List/Random 实现抛出的异常和分配故障继续传播。
- 大数不设人为位数、位数增长或运算次数上限。实际长度、scale、索引和容量计算须避免 wrapping；表示溢出与实际分配失败分别沿算术合同和既有分配合同处理。
- 字节输入采用 `List<Byte>`，输出采用独立 `Array<Byte>` 快照；字节序复用 `scoop.bytes.ByteOrder`。不得保留外部可变输入的别名。
- 结果允许复用输入实例、常量或既有不可变存储；数值 API 不承诺 `===` 的结果。跨线程发布仍遵守语言规范 1.2，普通 `val` 不建立 happens-before。

下文表格中的类型集合表示分别声明的具体重载，不是新泛型约束或数值子类型。签名清单省略实现体；正式源码必须具有完整实现。

## 3. 定宽整数函数

记 `S = Byte / Short / Int / Long`，`U = UByte / UShort / UInt / ULong`，`I = S / U`。每个重载的 operand/result 使用同一具体类型，另行列出的乘法与转换入口除外。

| 签名族 | 合同 |
| --- | --- |
| `abs(value: S): S` | 最小负数保持原值，其余返回数学绝对值，符合已有 wrapping 整数 |
| `absExact(value: S): S` | 最小负数抛 ArithmeticException |
| `signum(value: S): Int` | 返回 -1、0、1 |
| `min(a: I, b: I): I`、`max(a: I, b: I): I` | 按该类型的数值次序选择 |
| `clamp(value: I, min: I, max: I): I` | 闭区间钳制；min > max 非法 |
| `addExact(a: I, b: I): I`、`subtractExact(a: I, b: I): I`、`multiplyExact(a: I, b: I): I` | 数学结果超出目标类型范围时抛出；无符号减法下溢同样报错 |
| `incrementExact(value: I): I`、`decrementExact(value: I): I` | 对加减一检查溢出 |
| `negateExact(value: S): S` | 对取负检查溢出 |
| `powExact(value: I, exponent: Long): I` | 非负整数幂，0^0=1；负指数或结果超出目标范围时抛 ArithmeticException |
| `divideExact(a: S, b: S): S` | 向零截断，检查除零与 MIN / -1；不要求余数为零 |
| `floorDiv(a: S, b: S): S`、`ceilDiv(a: S, b: S): S` | 商分别向负无穷、正无穷取整；MIN / -1 沿 core 返回 MIN |
| `floorDivExact(a: S, b: S): S`、`ceilDivExact(a: S, b: S): S` | 同上，但 MIN / -1 抛出 |
| `floorMod(a: S, b: S): S`、`ceilMod(a: S, b: S): S` | 分别对应向下、向上的数学整数商；非零余数分别与除数同号、异号 |
| `toIntExact(value: Long): Int` | 检查可表示性后收窄 |
| `multiplyFull(a: Int, b: Int): Long` | 完整有符号 64 位乘积 |
| `multiplyFull(a: UInt, b: UInt): ULong` | 完整无符号 64 位乘积 |
| `multiplyHigh(a: Long, b: Long): Long` | 数学有符号 128 位乘积的高 64 位 |
| `multiplyHigh(a: ULong, b: ULong): ULong` | 数学无符号 128 位乘积的高 64 位 |

全部除法/余数入口的零除数均抛 ArithmeticException。MIN / -1 的两种 Mod 返回 0；除这一商不可表示的 wrapping 例外外，满足 `a = q*b+r`，按数学整数理解该等式。余数实现不能先以定宽乘法计算 `a-q*b` 导致错误。

例如 `floorDiv(-7,3) == -3`、`floorMod(-7,3) == 2`、`ceilDiv(-7,3) == -2`、`ceilMod(-7,3) == -1`。不增加 Java 的混合 Int/Long 参数重载；调用方显式选定宽度。

## 4. Float / Double 函数

### 4.1 表面

`E`、`PI`、`TAU` 为 Double `const val`，分别取对应数学常数最近偶数舍入的 binary64 值。Float 使用显式转换。记 `F = Float / Double`，下表均提供两种精度的具体重载。

| 签名族 | 范围 |
| --- | --- |
| `abs(x: F): F`、`signum(x: F): F` | 绝对值与保留零符号的符号函数 |
| `min(a: F, b: F): F`、`max(a: F, b: F): F`、`clamp(value: F, min: F, max: F): F` | 显式数学选择语义，见 4.2 |
| `sin(x)`、`cos(x)`、`tan(x)`、`asin(x)`、`acos(x)`、`atan(x)` | 参数/结果为 F；角度用弧度 |
| `atan2(y: F, x: F): F` | 包含象限和 signed zero 的反正切 |
| `sinh(x)`、`cosh(x)`、`tanh(x)` | 双曲函数，参数/结果为 F |
| `exp(x)`、`expm1(x)`、`log(x)`、`log10(x)`、`log1p(x)` | 指数、自然/十进制对数及近零专用入口，参数/结果为 F |
| `pow(x: F, y: F): F`、`sqrt(x: F): F`、`cbrt(x: F): F`、`hypot(x: F, y: F): F` | 幂、根与避免不必要中间溢出的二范数 |
| `toRadians(x: F): F`、`toDegrees(x: F): F` | 单位转换 |
| `floor(x: F): F`、`ceil(x: F): F`、`rint(x: F): F` | 向下、向上、最近偶数取整，仍返回浮点 |
| `round(x: Float): Int`、`round(x: Double): Long` | 最近整数，恰好一半时朝正无穷；饱和返回整数 |
| `fma(a: F, b: F, c: F): F` | 对数学 `a*b+c` 只舍入一次 |
| `ieeeRemainder(x: F, y: F): F` | 使用最近偶数整数商的 IEEE 余数，不是 `%` |
| `copySign(magnitude: F, sign: F): F` | 复制 sign bit，其余 magnitude bits 不变 |
| `nextUp(x: F): F`、`nextDown(x: F): F`、`nextAfter(x: F, direction: F): F` | 相邻可表示值 |
| `ulp(x: F): F`、`getExponent(x: F): Int`、`scalb(x: F, scaleFactor: Int): F` | 间隔、无偏指数与按二的整数次幂缩放 |

`Float` 的入口在 binary32 上定义，不统一采用 Double 计算后转回 Float。`nextAfter` 的两个参数同型，不采用 Java 的 Float/Double 混合签名。`getExponent` 对零/subnormal 返回最小 normal 指数减一，对 NaN/Infinity 返回最大 normal 指数加一。

### 4.2 特殊值、比较与舍入

遵守语言规范 11.2.2：最近偶数浮点环境、无 trap、渐进下溢、无可观察异常标志。浮点定义域错误产生 NaN，溢出产生有符号 Infinity；不抛整数算术异常。

- `min`/`max` 任一参数为 NaN 时返回 NaN；两种零之间分别选择 -0、+0。这是函数的明确合同，与 Scoop 默认浮点比较的“NaN 在末尾、两种零相等”不同，也不改变通用 `minOf`/`maxOf`。
- `clamp` 先检查两端不是 NaN，并按包含 `-0 < +0` 的数值次序检查边界；`min=+0,max=-0` 非法。合法边界下等价于 `min(max(value, min), max)`，value 为 NaN 则返回 NaN。
- `abs` 清除 sign bit；`copySign` 仅更换 sign bit，二者保留 NaN 的其余 bits。`signum` 对非零有限值和无穷返回相应 ±1，对零返回原零，对 NaN 返回 NaN。
- `sqrt(-0)` 为 -0；负非零输入为 NaN。`cbrt` 保留负号。`log(±0)` 为负无穷，负非零输入为 NaN；`log1p(-1)` 为负无穷，小于 -1 为 NaN。
- `floor`/`ceil`/`rint` 保留零的符号，并传递无穷和 NaN。`rint(2.5)` 为 2，`rint(3.5)` 为 4。`round(2.5)` 为 3，`round(-2.5)` 为 -2；NaN 返回 0，越界/无穷饱和到目标整数边界。
- `nextAfter` 的两输入数值相等时返回 direction，包括零符号；任一 NaN 返回 NaN。`nextUp(-0)` 为最小正 subnormal，`nextDown(+0)` 为最小负 subnormal；向内的无穷邻值为最大同号有限值，向外保持原无穷。
- `ulp(±0)` 为最小正 subnormal，`ulp(±Infinity)` 为正无穷，`ulp(NaN)` 为 NaN；结果对输入符号不敏感。
- `ieeeRemainder` 的有限 x 与无限 y 返回 x；无限 x、零 y 或 NaN operand 返回 NaN；零余数沿 x 的符号。`fma` 不能分解成普通乘法和加法，也不能以 fma 反向重写用户的独立乘加。

三角函数的无穷输入为 NaN；反三角函数按实数定义域及带符号零处理。`atan2` 的零/无穷象限与 C/Java 数学库约定一致。`pow(x, ±0)` 为 1，包括 x 为 NaN；其余 NaN 参数返回 NaN。负的有限非零底数只有整数指数产生实数结果；奇数整数指数保留负结果。零、无穷和无限指数按照 Java Math.pow 的特殊值表处理，尤其 `pow(1, NaN)` 与 `pow(±1, ±Infinity)` 为 NaN；不能直接暴露不同的 C pow 特例。`hypot` 任一参数为无穷时返回正无穷，即使另一参数为 NaN；否则 NaN 传播。

### 4.3 精度与可移植性

位操作、选择、取整、相邻值以及 IEEE 要求精确舍入的 `sqrt`、`fma`、`ieeeRemainder`、`scalb` 遵守各自数学合同。`scalb` 按精确缩放后一次舍入处理，允许合法上/下溢，不能通过先构造一个已溢出/下溢的浮点幂实现。

其他初等函数首版使用所支持目标的 libm。其有限结果精度采用对应 libm 的保证；**不承诺统一的最大 ULP 误差、Java Math 的逐项精度界或 StrictMath 的跨平台位级一致性**。API 文档须说明这一边界；同一算法的 NaN payload 也不作跨目标承诺。特别值规则仍须满足 4.2，不能把精度自由解释成放宽 NaN、零、定义域或 fma 的语义。

`expm1`、`log1p`、`hypot` 使用相应成熟算法，不能替换为直接 `exp(x)-1`、`log(1+x)`、`sqrt(x*x+y*y)`。角度转换按该精度的乘法完成：弧度转换乘最近舍入的 π/180，角度转换乘最近舍入的 180/π，不保证往返精确。两种目标精度均使用直接确定的系数。

不启用 fast-math，不引入隐式近似模式或浮点 Context。普通数学函数不自动进入语言的 const-call 集合。

## 5. BigInteger

### 5.1 构造、查询与转换

`BigInteger` 表示任意精度有符号整数；不存在 NaN、无穷、负零或固定字宽。companion 提供普通只读值 `ZERO`、`ONE`、`TWO`、`TEN`。

| 入口 | 合同 |
| --- | --- |
| `BigInteger(value: Long)`、`BigInteger(value: ULong)` | 精确构造，涵盖 Long.MIN_VALUE 和 ULong.MAX_VALUE |
| `BigInteger.parse(text: String, radix: Int = 10)`、`parseOrNone(...)` | 接受可选 ASCII 正负号及 radix 2..36 的 ASCII 数字；字母不区分大小写 |
| `String.toBigInteger(radix: Int = 10)`、`toBigIntegerOrNone(...)` | 同一解析合同的扩展 |
| 八种整数的 `toBigInteger(): BigInteger` | 从数学值精确转换，不经可能截断的 Long |
| `BigInteger.fromByteArray(bytes: List<Byte>, order: ByteOrder = BigEndian)` | 按二进制补码读取；空输入非法；允许冗余符号扩展 |
| `BigInteger.fromMagnitude(signum: Int, bytes: List<Byte>, order: ByteOrder = BigEndian)` | 按无符号 magnitude 读取；signum 限 -1/0/1，signum=0 时 magnitude 必须为零；空 magnitude 为零 |
| `val signum: Int` | -1、0、1 |
| `bitLength(): Long`、`bitCount(): Long`、`lowestSetBit(): Long` | 按 5.3 的无限补码模型查询 |
| `toByteArray(order: ByteOrder = BigEndian): Array<Byte>` | 最短有符号补码；必要时保留符号保护字节，零为单字节 0 |
| `toMagnitudeByteArray(order: ByteOrder = BigEndian): Array<Byte>` | 绝对值的最短无符号字节，零为单字节 0 |
| `toByte/Short/Int/Long/UByte/UShort/UInt/ULong()` | 与 core 一致，取模 2^W 后按目标类型解释 |
| 对应的 `toXExact()` | 只在数学值落入目标类型范围时返回，否则抛 ArithmeticException |
| `toFloat()`、`toDouble()` | 从整数直接按目标格式最近偶数舍入，可溢出为无穷 |
| `toString(): String`、`toString(radix: Int): String` | 默认十进制；radix 2..36，小写字母；无正号、前缀或多余前导零 |

文本不接受空串、仅符号、空白、下划线、进制前缀、指数或非 ASCII 数字。非法 radix 抛 IllegalArgumentException；其余文本失败抛 NumberFormatException。字节输入的结构非法抛 IllegalArgumentException。BigInteger 不经浮点中转做任何整数解析或转换。

### 5.2 算术与数论

| 入口 | 合同 |
| --- | --- |
| `+ - * / %`、一元 `+ -`、`++ --` | 精确整数运算；除法向零截断，余数与被除数同号；自增/减返回新值 |
| `abs(): BigInteger`、`min(other)`、`max(other)` | 数值选择；相等时 min/max 返回接收者 |
| `divideAndRemainder(other: BigInteger): (BigInteger, BigInteger)` | 一次求商、余数，满足 a=q*b+r 且 abs(r)<abs(b) |
| `pow(exponent: Long): BigInteger` | 非负整数幂，0^0=1；负指数抛 ArithmeticException |
| `sqrt(): BigInteger`、`sqrtAndRemainder(): (BigInteger, BigInteger)` | 对非负数返回 floor(sqrt(n)) 与 n-root²；负数抛 ArithmeticException |
| `gcd(other: BigInteger): BigInteger` | 非负最大公约数，gcd(0,0)=0 |
| `mod(modulus: BigInteger): BigInteger` | modulus 必须正，返回 [0,modulus) |
| `modPow(exponent: BigInteger, modulus: BigInteger): BigInteger` | 正 modulus；负指数先取模逆，逆不存在则抛出；modulus=1 返回 0 |
| `modInverse(modulus: BigInteger): BigInteger` | 正 modulus，存在时返回 [0,modulus) 内的逆；modulus=1 返回 0，否则不存在逆时抛出 |

非正 modulus 与不存在的模逆为 ArithmeticException。`%` 与 `mod` 不能互换。`sqrtAndRemainder` 满足 `0 <= remainder < 2*root+1`。大整数除法不存在定宽的 MIN / -1 溢出。

### 5.3 位操作

提供 infix `and(other)`、`or(other)`、`xor(other)`、`andNot(other)`、`shl(distance: Long)`、`shr(distance: Long)`，以及 `inv()`、`testBit(index: Long): Boolean`、`setBit(index)`、`clearBit(index)`、`flipBit(index)`。结果除 testBit 外均为 BigInteger；不增加位运算的 compiler operator 或 `ushr`。

数学模型为无限符号扩展的二进制补码；内部无需采用补码存储。右移是算术右移，负距离反转方向，距离不按机器字宽取模。Long.MIN_VALUE 的距离不能先直接取负而 wrapping：足够远的右移直接返回 0 或 -1，实际左移仅在结果长度/分配无法表示时失败，零的任意移位仍为零。

bitLength 是去掉符号扩展位后的最小位数：0 和 -1 均为 0；bitCount 统计不同于符号位的位数，-1 为 0；lowestSetBit 对零返回 -1，否则返回最低 1 位的位置。位 index 必须非负，否则抛 IllegalArgumentException；没有人为最高位限制。

### 5.4 素性与素数搜索

```scoop
fun BigInteger.isProbablePrime(random: Random, certainty: Int = 100): Boolean
fun BigInteger.nextProbablePrime(random: Random, certainty: Int = 100): BigInteger
fun BigInteger.Companion.probablePrime(
    bitLength: Long, random: Random, certainty: Int = 100
): BigInteger
```

以上为归属于 BigInteger/companion 的签名表示，不要求实际以 extension 实现。certainty 必须大于零，bitLength 至少为 2，否则抛 IllegalArgumentException。`isProbablePrime` 对小于 2 的整数返回 false；不把负数隐式取绝对值。`nextProbablePrime` 返回严格大于接收者的正 probable prime，因此小于 2 的输入返回 2。

有限小数域使用已知的确定性测试；一般整数采用试除和 Miller–Rabin。对于一个固定合数，且传入 Random 能提供独立均匀随机位时，抽取独立均匀底数并执行足够轮次，使误判概率不超过 2^-certainty；这一条件不被表述为任意自定义 RNG 的保证，也不是素数生成结果的后验概率保证。probablePrime 设置最高位以保证恰有 bitLength 位；不承诺生成素数间均匀分布。

抽样复用 Random 的字节/整数接口和局部 rejection sampling；不新增随机源。实现不承诺常数时间运算，也不接管 `scoop.crypto` 的密钥生成。

## 6. MathContext 与 RoundingMode

```scoop
public enum RoundingMode {
    Up, Down, Ceiling, Floor, HalfUp, HalfDown, HalfEven, Unnecessary
}

public struct MathContext(
    val precision: Long = 0L,
    val roundingMode: RoundingMode = RoundingMode.HalfEven
)
```

两者均为普通值类型，显式实现同类型 Equals、Hash 和 ToString。MathContext 没有托管引用字段。precision=0 表示不限制精度并要求精确结果；precision>0 表示十进制有效数字上限，不是小数位数。

struct 的公开字段和副本更新不能承诺构造时已验证 precision。所有接收 context 的公共运算在入口检查 precision>=0，负数抛 IllegalArgumentException，即使本次数值恰好无需计算；验证后内部算法传递同一事实，不逐层重复检查。这里是一个标量参数检查，不添加工厂状态、隐藏有效性标志或新语言约束。

companion 提供具名配置 `Unlimited`、`Decimal32`、`Decimal64`、`Decimal128`，precision 分别为 0、7、16、34，roundingMode 均为 HalfEven。后三者只选择有效数字，**不是 IEEE decimal32/64/128 编码，也不施加这些格式的指数范围**。

| 模式 | 丢弃部分非零时的方向 |
| --- | --- |
| Up | 远离零 |
| Down | 朝零 |
| Ceiling | 朝正无穷 |
| Floor | 朝负无穷 |
| HalfUp | 最近，恰好一半远离零 |
| HalfDown | 最近，恰好一半朝零 |
| HalfEven | 最近，恰好一半使保留末位为偶数 |
| Unnecessary | 不允许改变数值，否则抛 ArithmeticException |

没有全局、线程或 Task-local 的隐式 MathContext，也没有浮点异常状态/粘性标志。一个运算的 context 不存入 BigDecimal，不影响后续运算。

## 7. BigDecimal

### 7.1 数值模型、构造与观察

数值为 `unscaledValue × 10^(-scale)`，unscaledValue 是整数，scale 为 Long。允许负 scale；没有 NaN、无穷或有符号零。输入的尾随十进制零保留在表示中。companion 提供 scale=0 的 `ZERO`、`ONE`、`TWO`、`TEN`。

| 入口 | 合同 |
| --- | --- |
| `BigDecimal(value: Long)`、`BigDecimal(value: ULong)` | 精确整数，scale=0 |
| `BigDecimal(unscaledValue: BigInteger, scale: Long = 0L)` | 精确指定表示 |
| `BigDecimal(unscaledValue: Long, scale: Long)` | 常用小 coefficient 的精确构造 |
| `BigDecimal.parse(text: String)`、`parseOrNone(text)` | ASCII 十进制和可选 e/E 指数，保留表示 |
| `String.toBigDecimal()`、`toBigDecimalOrNone()` | 同一解析合同的扩展 |
| 八种整数的 `toBigDecimal()`、`BigInteger.toBigDecimal()` | 精确转换为 scale=0 |
| `BigDecimal.fromFloat(value: Float)`、`fromDouble(value: Double)` | 解析原精度的最短往返十进制文本；例如 fromDouble(0.1) 得到十进制 0.1 |
| `BigDecimal.fromFloatExact(value: Float)`、`fromDoubleExact(value: Double)` | 精确展开二进制数值；例如 fromDoubleExact(0.1) 与十进制 0.1 不相等 |
| `val scale: Long`、`val signum: Int` | 已有的廉价状态；零的 signum=0 |
| `unscaledValue(): BigInteger`、`precision(): Long` | 返回不可变整数与 coefficient 的十进制位数；零的 precision=1 |
| `ulp(): BigDecimal` | coefficient=1、scale 与接收者相同 |

浮点构造的非有限输入抛 IllegalArgumentException。最短文本构造保留该文本得到的 scale；精确构造去掉展开中非必需的尾零，零为 ZERO，负零也不保留符号。

解析语法为可选正负号，随后 `digits[.digits]` 或 `.digits`，可接 `[eE][+-]?digits`；小数点前后至少一侧有数字，因此 `1.` 合法。只接受 ASCII 数字，不接受空白、分隔符、后缀、NaN 或 Infinity。scale 等于小数位数减指数，按数学整数计算后检查 Long 范围；格式或最终 scale 无法表示属于 NumberFormatException/None，不让指数累加 wrapping。前导零不计 precision，小数尾零计入 scale。

### 7.2 相等、顺序与哈希

**默认相等按数学数值，忽略 scale。** `BigDecimal.parse("1.0") == BigDecimal.parse("1.00")` 为 true，compareTo 为 0，hash 相同。`sameRepresentation(other: BigDecimal): Boolean` 另行比较 coefficient 和 scale；该例返回 false。所有零的默认相等与 hash 一致。

这是相对 [Java BigDecimal](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/math/BigDecimal.html) 的明确设计差异，避免 HashMap 与有序容器因默认相等规则产生不同的数值集合。它不消除可观察的 scale，数值相等的对象仍可有不同文本和 scale 相关运算结果。

hash 必须按去除尾零后的逻辑数值计算，但不能因此修改对象或要求构造一个规范化 BigDecimal。特别是 scale=Long.MIN_VALUE 时，去尾零后的逻辑指数仍须正确处理，equals/compareTo/hash 不能因为规范化 scale 溢出而报算术异常。比较也不能为两个相距很远的 scale 无条件构造巨大 10 的幂。

### 7.3 算术与精度

| 入口 | 合同 |
| --- | --- |
| `+ - * / %`、一元 `+ -`、`++ --` | 默认精确；`/` 不使用接收者 scale 或隐式 context 舍入 |
| `plus(other, context)`、`minus(other, context)`、`times(other, context)`、`div(other, context)` | 同名普通重载，按显式有效数字与模式舍入 |
| `div(other, scale: Long, roundingMode: RoundingMode)` | 结果固定为指定 scale，按模式舍入 |
| `round(context: MathContext): BigDecimal` | 将本值舍入到指定有效数字 |
| `abs(): BigDecimal`、`min(other)`、`max(other)` | 保留所选值的表示；min/max 数值相等时选择接收者 |
| `divideToIntegralValue(other, context: MathContext = MathContext.Unlimited)` | 数学商向零取整数；有限 precision 不足以容纳整数商时抛出，不靠舍入改变整数商 |
| `divideAndRemainder(other, context: MathContext = MathContext.Unlimited): (BigDecimal, BigDecimal)` | 返回上述整数商 q 与精确余数 a-q*b |
| `rem(other, context)` | context 仅限制整数商，余数本身不再次舍入，可以超过 context.precision |
| `pow(exponent: Long): BigDecimal` | 非负整数幂，0^0=1；负指数抛 ArithmeticException |
| `pow(exponent: Long, context: MathContext): BigDecimal` | precision>0 时也允许负指数；负指数的零底数抛出；precision=0 时同精确版本 |
| `sqrt(context: MathContext = MathContext.Unlimited): BigDecimal` | 负数抛出；无限精度要求有限精确十进制平方根，有限精度按模式舍入 |

所有二元入口的 other 均为 BigDecimal；除显式 tuple 外结果均为 BigDecimal。精确除法在约分后的分母仅含因子 2、5 时有有限结果；否则抛 ArithmeticException，例如精确 `1/3`。`%` 使用向零整数商，余数与被除数同号，不是正模运算。

有限 precision 的合同是对数学精确结果执行一次所选舍入，包含 `pow` 和 `sqrt`。实现可以用保护位、余数和误差区间避免物化全部精确中间值，但不能把反复按最终 precision 舍入的近似当作同一合同。Unnecessary 只在确实需要改变数值时失败；丢弃全零尾数合法。

| 精确运算 | 首选 scale |
| --- | --- |
| 加、减 | max(a.scale,b.scale) |
| 乘 | a.scale+b.scale |
| 除 | a.scale-b.scale，有限精确商必要时增加 scale |
| 平方根 | ceil(a.scale/2)，精确结果必要时增加 scale |
| 整数幂 | a.scale*exponent；exponent=0 返回 ONE；允许的负指数按正幂取倒数的除法规则选择表示 |

数学精确结果可在 precision 内表示时，采用最接近首选 scale 的允许表示；精确除法不为填满 precision 添加无意义零。必须舍入时保留规定有效数字，发生最高位进位时同步调整 scale。例如 `999.9` 按 precision=3、HalfUp 得到 `1.00e3`。

整数商的首选 scale 同普通除法，但允许减少 scale 来精确表示整数；有限 precision 只接受能在该精度内精确表示的整数商。余数采用精确减法的表示规则。无 context 的一元正负和 abs 保留 scale；inc/dec 等价于精确加减 scale=0 的 ONE。

scale 公式按数学整数理解，不以 Long wrapping 求值；最终规定表示的 scale 超出 Long 时抛 ArithmeticException。中间计算需要更宽范围时使用局部整数算法，不扩大语言整数种类。即使最终数值为零也不默默改变这些已规定的 scale；`stripTrailingZeros` 的零归一化例外见下节。

### 7.4 scale、转换和格式

| 入口 | 合同 |
| --- | --- |
| `setScale(scale: Long, roundingMode: RoundingMode = RoundingMode.Unnecessary)` | 保持数值或显式舍入到目标 scale |
| `scaleByPowerOfTen(exponent: Long)` | 精确乘 10^exponent，coefficient 不变，scale 减 exponent |
| `movePointLeft(distance: Long)`、`movePointRight(distance: Long)` | 精确移动小数点，负距离反转；结果 scale 为 max(移动后的 scale,0)，必要时补 coefficient 零 |
| `stripTrailingZeros()` | 去掉 coefficient 的十进制尾零并降低 scale；任何零返回 ZERO；非零结果 scale 不可表示则抛出 |
| `toBigInteger()`、`toBigIntegerExact()` | 分别向零截断、要求没有小数部分 |
| 八种整数的 `toX()`、`toXExact()` | 分别先向零取整数再按目标宽度截断、同时要求整数性与范围可表示 |
| `toFloat()`、`toDouble()` | 由精确十进制数学值直接最近偶数舍入到目标格式，不通过另一浮点格式；允许无穷与有符号下溢零 |
| `toString()` | 规范文本，能由 parse 恢复相同 coefficient 与 scale |
| `toPlainString()` | 不用指数；负 scale 会展开为整数，往返只保证数值 |
| `toEngineeringString()` | 使用三的整数倍指数，往返保证数值，不保证原 scale |

所有结果为新值或可安全复用的不可变值。格式化不用 locale；指数写小写 `e`、省略正号和多余前导零。toString 在 scale>=0 且调整指数 `precision-scale-1 >= -6` 时用定点，否则用一位整数数字的科学形式；零按 precision=1 应用同一规则。精度和指数计算避免定宽溢出。

例如 `parse("1.00").toString()` 为 `"1.00"`，`parse("1e3").toString()` 为 `"1e3"`，后者 toPlainString 为 `"1000"`；`parse("-0.00").toString()` 为 `"0.00"`。toEngineeringString 在需要科学形式时将指数改为三的倍数，并调整小数点，不自动丢弃原有有效数字。

## 8. 与现有语言、库及产物的边界

math 不增加 literal 后缀、隐式转换、compiler intrinsic 名单、runtime 数值对象或专用 GC 扫描协议。大数 class、内部值类型、数组和返回 tuple 均沿普通源码编译、单态化、接口分派及 `.slib` 发布路径。

标量 native leaf 只处理 C ABI 的基本数值，不接受 managed class、数组或 context。native 输入与目标库沿实现规范 2.7～2.8 的普通闭包交付；依赖更新和缓存复用没有 math 专用规则。

std 的私有十进制到浮点 helper 仍属于其文本转换实现，不提升为公开 BigInteger，也不为共用算法引入 std→math 依赖。现有 JSON 的数字合同保持原样；BigDecimal 的接入需使用明确的普通 codec，不能通过自动转 Double 丢失精度。
