# Scoop math 库设计

日期：2026-10-26。状态：设计完成，待实现。

公共 API 与边界行为见 [math 规范草案](../specs/SCOOP-MATH-SPEC.md)。本设计以 Java 数学库的功能范围为参照，使用 Scoop 自己的值类型、运算符接口、原生编译及 GC 模型。优化管线在另一分支推进；本文按其 scalar、value type、icall/vcall 优化方向组织库代码，不将该分支尚未在本 checkout 验证的收益写成已实现事实。

## 1. 核心决定

1. 新增显式依赖的 `scoop.math` Cone。整数与大数运算主体使用普通 Scoop；超越函数复用目标 libm。无需新增语言语法、编译器识别的 math 类型或 runtime 大数 ABI。
2. `BigInteger`、`BigDecimal` 均为不可变 final class。数值很大时共享不可变 magnitude，常见小值直接保存在 class 的内部 value enum 中。
3. `MathContext` 是 struct，`RoundingMode` 是 enum；商余数返回 tuple。内部进位、借位、舍入判定等短命数据使用 scalar/struct/tuple，不为每一步构造 class。
4. 复用现有 Add/Sub/Mul/Div/Rem、Equals、Compare、Hash 等协议。直接调用、泛型单态化和真正动态的接口调用使用同一份实现，不建立平行的“快速 API”。
5. BigDecimal 默认按数值相等，scale 仍可观察；`sameRepresentation` 比较表示。运算默认精确，舍入要求显式 context 或 scale/mode。
6. 不实现随机数模块。素性测试和素数生成接收已有 Random，不使用隐藏全局随机源。

这里的“不可变”包括对象引用的 magnitude 内容，不能仅把一个仍可由外部修改的 List 字段声明为 val。既有实例可以共享，不承诺新对象身份；安全发布之后所有读操作均不写入对象或其存储。

## 2. Java 功能映射与明确差异

参考基线为 Java SE 26，不声称 Java API、序列化格式或 JVM 行为兼容。

| Java 表面 | Scoop 设计 |
| --- | --- |
| Math 的静态函数及常量 | `scoop.math` 顶层函数及 E/PI/TAU；包括 Exact 算术、定向整除和 powExact |
| 大部分超越函数仅 double | 同时提供 Float/Double；使用各自精度的运算及后备 |
| unsignedMultiplyHigh(long,long) | 使用实际无符号类型的 multiplyHigh(ULong,ULong) |
| IEEEremainder | lowerCamelCase 的 ieeeRemainder |
| Math.random、BigInteger 隐藏的随机源选择 | 无 math.random；需要随机性的数论调用显式传入既有 Random |
| BigInteger、BigDecimal | 保留名称与不可变 class 模型，使用运算符及明确转换 |
| divideAndRemainder、sqrtAndRemainder 的二元素数组 | 两个结果的 typed tuple |
| MathContext class、RoundingMode enum class | 无托管字段的 struct 与普通 enum |
| BigDecimal.equals 区分 scale | 默认数值相等；sameRepresentation 独立表达 scale 敏感比较 |
| BigDecimal 的 32 位 scale 和 precision | Long，符合 Scoop 的计数口径，不引入 Java 的指数范围上限 |
| BigDecimal(double) 与 valueOf(double) | fromDoubleExact 与 fromDouble 明确区分精确二进制值和最短十进制文本 |
| BigDecimal 限精度 pow 的近似误差合同 | 对数学结果一次舍入，不能直接照搬 Java 的最终误差宽限 |
| StrictMath 的固定算法结果 | 首版没有独立 StrictMath，也不承诺跨 libm 位级结果一致 |
| parallelMultiply、Java Serializable、已弃用整数舍入码 | 不复制；乘法功能、文本/字节转换及 RoundingMode 已覆盖本期职责 |

Java 是 API 范围和对照数据来源，不是 Scoop 上游实现源码的默认移植来源。接口名相同也不能自动套用 Java 的相等、装箱、泛型擦除、数组结果或异常模型。

调用形状如下；这些是待实现 API 的使用示意：

```scoop
import scoop.math.*

fun <T : Add<T, T>> twice(value: T): T = value + value

val amount = BigDecimal.parse("19.95")
val doubled = twice(amount)
val third = amount.div(BigDecimal(3L), MathContext.Decimal128)
val rounded = third.setScale(2L, RoundingMode.HalfEven)

val a = BigDecimal.parse("1.0")
val b = BigDecimal.parse("1.00")
val equal = a == b                       // true
val same = a.sameRepresentation(b)       // false

val large = BigInteger.parse("340282366920938463463374607431768211456")
val (quotient, remainder) = large.divideAndRemainder(BigInteger(97L))
```

## 3. 已有基础与依赖方向

| 现有能力 | math 中的使用 |
| --- | --- |
| core 八种整数、Float/Double、bits 转换 | 32 位 limb、64 位中间积、浮点位操作与精确转换 |
| struct/enum/tuple 与单态化 | 数值配置、进位和舍入数据、商余数、带 bound 的泛型运算 |
| Array/MutableArray 的内联值元素 | 连续未装箱 UInt magnitude 与局部工作区 |
| Equals/Compare/Hash 和运算符接口 | 直接/泛型/动态调用共用实现，接入 std 集合 |
| String、StringBuilder、ASCII 迭代 | 解析和输出；避免反复 O(n) 的 String[index] |
| std NumberFormatException、ByteOrder、Random | 复用已有错误、字节序和随机源实体 |
| 普通 C ABI、GCLeaf、native 链接闭包 | libm 标量调用，不传递 managed 数据 |
| 预编译 sysroot 与 scoop.test | 库发布、应用消费及库行为验收 |

依赖为 `math -> std -> core`，math 直接使用 core 也显式声明 core dependency；测试依赖单独列出。math 不依赖 platform、crypto、JSON 或一个新的 Rust runtime。

std 的 [DecimalMagnitude](../../sysroot/src/scoop.std/src/text/decimal_magnitude.scoop) 只服务私有十进制到浮点解析，操作集合很小。它不是可直接提升为 BigInteger 的完整实现：缺少有符号算术、通用除法、位语义、不可变发布及公开转换。保留这一局部实现，不借本任务移动 std 的数值文本 API 或制造依赖环。

## 4. 存储模型与不可变边界

### 4.1 大整数内部表示

两个公共 class 共用 Cone 内部的值 enum，其结构形状为：

```scoop
internal enum IntegerStorage {
    Small(val value: Long),
    Large(val negative: Boolean, val magnitude: Array<UInt>)
}
```

这只是内部存储，不形成第二套公共数值类型或 compiler role。

| 对象 | 私有/公开字段 |
| --- | --- |
| BigInteger | private val storage: IntegerStorage |
| BigDecimal | private val coefficient: IntegerStorage；public val scale: Long |

Small 覆盖完整 Long，包括 Long.MIN_VALUE；不以某个合法整数充当“大数”sentinel。Large 使用 little-endian base-2^32 magnitude，最高 limb 非零，并且数学值不能放入 Small。零仅用 Small(0)；Large 的 negative 不表达负零。内部构造/运算结束时规范化一次，后续只读运算复用不变量，不在每个循环或 accessor 重扫 magnitude。

选择 UInt limb 是因为 Scoop 已有完整 UInt/ULong 运算。单 limb 的 `a*b + destination + carry` 在正确进位范围内最大为 2^64-1，可用 ULong 精确计算；不需要新增 UInt128 或编译器的“大整数乘加”指令。64 位 multiplyHigh 也可由 32 位分块实现。

BigDecimal 的小 coefficient 直接存 Long，不额外嵌套一个 BigInteger 对象。其 unscaledValue() 按需构造 BigInteger wrapper，large magnitude 可以安全共享。BigInteger 传入 BigDecimal 构造时同样共享已发布的 IntegerStorage，不深复制数字。

### 4.2 工作区与发布

- 使用局部 MutableArray<UInt> 进行长加、长乘、除法及进制转换；写入过程中不把它作为某个已返回数值的存储。
- 完成后按实际 limb 数生成一次 Array<UInt> 快照。语言规范 10.4 已明确 MutableArray→Array 复制，不能把转换宣称为零复制 freeze，不能通过 Ptr cast 改变 nominal 类型。
- 只需直接逐项生成的结果可以一次构造 Array；不把每一个内部运算先包装成临时 BigInteger，再拆回 limbs。
- 工作区的使用长度、进位和索引用普通局部标量或小 struct 保存。没有全局可变 scratch、对象池、跨任务共享 builder 或公开 MutableBigInteger。
- 传入 List<Byte> 的外部可变内容在构造期间读取并形成自有表示；其后修改输入不影响结果。toByteArray 返回独立快照。
- 不设置无同步的 lazy hash/precision/string cache。bitLength、hash 等按实际成本计算；若未来有真实热点，再单独评估缓存的收益和同步成本。

Array<UInt> 没有逐元素 managed 引用；class 与含数组的 enum 仍包含普通 GC 引用，按既有布局扫描。不能把 Large 或包含它的 IntegerStorage 标注为 NoGC，也不能因本次恰好是 Small 就绕过其静态类型合同。

## 5. 普通 Scoop 数值内核

### 5.1 定宽与大整数

| 运算 | 首期算法 |
| --- | --- |
| 受检标量运算 | 使用现有 wrapping 算术及符号/进位关系判断，失败路径抛出；除法独立处理零与 MIN/-1 |
| Small/Small | 直接 Long 算术和精确溢出检测；成功时不创建 magnitude，溢出才提升 |
| magnitude 加减 | 从低位向高位传递 ULong carry/borrow；结果一次规范化 |
| 单 limb 乘除 | 64 位中间数，复用分组文本转换与普通运算 |
| 多 limb 乘法 | schoolbook 为小规模基线，规模达到实测交点时使用 Karatsuba；阈值只影响实现选择 |
| 多 limb 除法 | 正规化长除法（Knuth D），商与余数共用一次运算；单 limb 单独走快路径 |
| 有符号位操作 | magnitude 与无限补码语义之间局部变换，不要求对象永久存补码 |
| gcd / inverse | Euclid/扩展 Euclid，结果复用已有除法；局部变量迭代，不建立多态算法对象 |
| pow / modPow | 平方求幂；模幂每步约简，奇数 modulus 使用成熟 Montgomery 路径，偶数先走普通约简 |
| 整数平方根 | 整数 Newton 迭代，最后以平方与余数校正，不使用 Double 当最终判据 |
| 文本 | 按 radix 的可容纳整数块处理；2 的幂进制按位分组，避免逐个字符进行完整大除法 |
| 素性 | 小素数试除；64 位有限域采用已证明底数组合，一般值执行独立底数的 Miller–Rabin |

算法移植以维护中的 [num-bigint](https://github.com/rust-num/num-bigint) 为主要参考，首期核对 0.5.1 的修正，并明确使用 32 位 limb 分支。其 [2026-07 发布记录](https://github.com/rust-num/num-bigint/blob/main/RELEASES.md) 包括单字内联存储及除法修复；实施时记录实际移植版本/commit，保留适用的上游许可与归属。借用成熟算法和边界修正，不移植 Rust trait、allocator、RNG 或平台配置框架。

这里选择把大数内核移入 Scoop，而非绑定整个外部大数库，直接服务用户要求的优化方向：运算和数组访问对 Scoop pipeline 可见，数值对象由正常 GC 管理，也无需为每个数值附加 native handle 和 release。首版不同时维护 native 与 Scoop 两套大数实现。

Toom-Cook、FFT、Burnikel–Ziegler 和并行乘法不是功能闭环的先决条件；只有首期规模基准显示具体瓶颈后再扩展算法。任何阈值均不进入语言合法性、产物兼容性或公开参数。

### 5.2 十进制

BigDecimal 的算术复用 IntegerStorage 内核，不经公开 class 接口反复创建临时 wrapper。Small coefficient 上的加法对齐和乘法先检查真实溢出；失败后提升，不能为维持 Small 丢失精度。

舍入集中为一个内部决定函数，输入包括符号、保留末位、丢弃部分相对半单位的位置和 sticky 信息，输出是否增加绝对值。RoundingMode 的 when 覆盖八种变体；常量 context 可被普通内联和标量化消去分支，不为每一种模式建立策略 class 或 icall。

具体职责如下：

1. 精确加减对齐 scale，乘法相加 scale；精确除法先约分，再判断分母是否仅含 2 和 5。
2. 限精度除法计算必要的有效数字与余数，舍入依据精确余数；不能先用 Double 估商再当作最终结果。
3. 大幅相差的 scale 可在限精度加减中通过数量级和 sticky 信息处理；相消时保留足够信息或回到精确路径，不凭“大数一定占主导”丢掉舍入边界。
4. sqrt 使用整数平方根/余数在目标十进制格点判定舍入；pow 以精确平方求幂作为正确性基线，避免把固定少量保护位当作正确舍入证明。若以区间精化加速，必须有精确中点/恰好可表示的退出判断，不能在舍入中点无限精化。
5. 对 final scale、位数和容量分别检查实际表示范围。比较、hash 与格式化的逻辑十进制指数可能需要比 Long 多一位；使用局部双字运算处理该差值，不创建新的公开 128 位类型或通用预算。
6. 数值 hash 使用去尾零后的逻辑 coefficient/exponent。stripTrailingZeros 需要返回一个可表示的新 BigDecimal，可以报 scale 溢出；hash/equals 不以能否构造该新对象为前提。
7. 精确浮点构造按 sign/exponent/significand 拆解为整数和二的幂；toFloat/toDouble 使用目标精度、余数及最近偶数舍入，避免 Double→Float 双重舍入。

scale 边界、全零尾数、进位增加 precision、负数定向舍入和精确/非终止除法都在公共合同中确定，不以将来的优化行为决定结果。

## 6. 浮点后备与调用成本

abs、min/max、符号、指数、相邻值、round 和可直接表达的受检/位运算使用普通 Scoop。调用成熟 libm 的范围是超越函数及 sqrt/fma/remainder/scalbn 等相应标量操作；Float 使用真实的 f32 入口。

首期使用目标已有 libm：Darwin 由系统数学库提供，Linux GNU/musl 使用各自目标资源。通过普通 extern 声明和 native 链接配置选择，不按平台分叉公共 API，也不增加编译器按名字识别 sin/sqrt 的逻辑。只有真实存在符号或 ABI 差异的地方才增加薄 C shim，不创建第二套数值 runtime。

这些调用只有固定大小的数值参数，不分配 Scoop 对象、不回调、不持有 managed 内存。符合语言规范 13.4.1 的具体入口使用 `@GCLeaf @Extern(abi = "c")`，由安全的普通 Scoop 函数包装；C extern 不标 `@NoGC`。不捕获 errno，不把 libm 的 errno 转成 Scoop 算术异常。GCLeaf 不等于 readonly/纯函数，也不保证所有 target 的误差一致。

额外的 Scoop 分支只实现规范明确要求的表面差异，例如 pow(1,NaN)、pow(±1,Infinity) 与 C 的区别，以及 NaN 传播和 signed zero 的选择。它不是一套补偿 libm 精度的自制超越算法。fma 必须调用融合语义入口；strict FP 下普通 `a*b+c` 仍保持两次舍入。

精度边界已在规范 4.3 明示。若未来实际需要逐位可复现的初等函数，再独立评估固定实现；不能把使用系统 libm 描述成已拥有 Java StrictMath 保证。维护中的 [Rust libm](https://github.com/rust-lang/compiler-builtins) 是可研究的固定实现来源，但本期不引入它或另加 backend 选择层；旧 rust-lang/libm 仓库已迁移，不能以旧仓库归档状态判断当前项目无人维护。

[GNU libc 的数学误差说明](https://www.gnu.org/s/libc/manual/pdf/libc.pdf)也将 sqrt/fma/rint 等有精确 IEEE 合同的运算与一般初等函数区分；这支持分别声明两类精度责任，而不是从“调用了 libm”推导出统一的正确舍入保证。

## 7. 与优化分支的协作

| 写法/场景 | 交给既有优化的机会 | 库必须自己保证的事实 |
| --- | --- | --- |
| carry/borrow、舍入状态、小 tuple | scalar propagation、SROA、寄存器返回及冗余复制消除 | 完整的 typed 值，进位与溢出正确 |
| MathContext/RoundingMode | 按值传递、常量分支化简 | context 显式，负 precision 在公共边界处理 |
| BigInteger/BigDecimal final 成员 | 去虚化及跨 Cone 小函数内联 | 公共对象不可变；不承诺所有 class 分配被消除 |
| `T : Add<T,T>` 等 | 单态化、具体调用及 intrinsic 恢复 | 显式 conformance，和直接操作相同语义 |
| Equals/Compare 接口视图 | 已知目标去虚化，未知目标正常 icall | 不用 Any 或函数表重新封装同一运算 |
| UInt 数组内核 | 内联访问、标量/循环优化 | 数组有效范围、真实 poll 和 GC roots 保持 |
| libm 的标量签名 | 既有直接 C ABI 与 GCLeaf 调用路径 | 原精度、native 调用合同和链接依赖准确 |

不要求新增专用 math pass、不将整个库强制 inline、不引入 boxed Number、不把 BigInteger 改成公开 struct，也不以“不可变 class 所以必定零分配”作为设计前提。

`@NoGC` 只用于现有规则确实允许的 GC-free scalar/value helper；抛异常的 Exact 入口、class 方法、数组内核不能批量标注。源码整数除法即使已做非零检查，也不能凭库侧推理改写其现有 effect。长 limb 循环保持普通 Managed 轮询，不能为了 vectorization 在库中自行屏蔽 GC 或长时间 pin 数组。

## 8. 实现结构与交付阶段

预期源码按实际职责拆分，不把全部大数算法放进一个 class 文件：

```text
sysroot/src/scoop.math/
  Cone.toml
  src/scalar/                 # Integer checks, floating wrappers and bit operations
  src/big_integer/            # Public type, arithmetic, bits, conversions and primes
  src/big_decimal/            # Public type, arithmetic, scale, rounding and text
  src/internal/integer/       # Storage, limbs, multiplication and division
  tests/                     # Public behavior and internal algorithm tests
  consumer-tests/            # Ordinary precompiled consumer test Cone
  examples/
```

目录是实现规划，不创建空 Cone、空方法或“尚未支持”分支。

| 阶段 | 可用交付 | 完成条件 |
| --- | --- | --- |
| A：标量 math | 全部定宽及 Float/Double API、native 依赖和示例 | 三目标预编译库能消费、链接、运行；特殊值和受检边界通过 |
| B：BigInteger | 不可变表示、完整算术/位运算/转换、sqrt、模运算和素数 API | 独立规则、组合行为和真实下游 Cone 通过；随机源来自 std |
| C：BigDecimal | MathContext/RoundingMode、精确及限精度运算、scale、相等/hash 和格式 | 全模式舍入、精确除法、pow/sqrt、边界和集合组合通过 |
| D：优化与分发验收 | 针对实际规模调节内核，接入优化分支测量，完整 sysroot 与文档 | 少量代表 IR/机器码检查和同机基准完成；所有公开入口已实现 |

每一阶段先对照本规范完成该组 API，再发布其生产实现。阶段 A 的数值接口不依赖后续大数阶段才能运行。B/C 不以新优化 pass 是否消除所有分配为功能阻塞条件。公共合同变动仍先修改本规范；本设计不修改另一分支的 pipeline 或 runtime 契约。

## 9. 验证与性能证据

### 9.1 行为测试

普通测试放在 math Cone 的 tests；预编译消费者的泛型运算、集合及文本往返也使用独立测试 Cone，不把它们改写成编译器 fixture。

| 类别 | 必须覆盖 |
| --- | --- |
| 定宽 | 各宽度两端、无符号下溢、MIN/-1、异号 floor/ceil、powExact、乘法高位与全积 |
| 浮点 | ±0、subnormal、最大有限值、±Infinity、多种 NaN、fma 与普通乘加不同的例子、round 的大数边界 |
| BigInteger | Small/Large 来回转换、Long.MIN/ULong.MAX、连续进位/借位、除法估商修正、负数位语义、极端移位 |
| 字节/文本 | 127/128/-128/-129 的符号边界、两种字节序、radix 2..36、非法输入及快照不可变 |
| 数论 | q*b+r、gcd 整除关系、模逆与负指数、平方根余数界、素数与 Carmichael/强伪素数样例 |
| 十进制 | 每种模式的正负 tie/非 tie、零尾数、9 的进位链、precision 与 scale 的区别、非终止除法 |
| 表示与比较 | 1.0/1.00、不同 scale 的零、HashMap 与有序集合一致、sameRepresentation、scale 两端的 hash/比较 |
| 十进制高阶 | sqrt/pow 的精确值、中点、相消、负指数及单次舍入，Float/Double 的双重舍入反例 |
| 组合 | `Add<T,T>` 泛型、Equals/Compare 视图、class 包含大数、数组、异常后输入未改变 |

Java BigInteger/BigDecimal 用于生成可审阅的对照向量；相等、Long scale、负数素性、文本指数拼写和 pow 舍入等已声明差异不强求 Java 一致。浮点精确合同用高精度/精确算术生成预期，超越函数记录目标 libm 的参考值与误差，不把任一 JDK 的最后一位当作所有平台的唯一答案。生成工具与版本记录随测试说明保存，运行库测试不依赖本机安装 Java 或联网。

概率测试使用固定输入和显式 seeded Random 以复现路径；用已知合数/素数及底数范围验证实现，不把“随机跑很多次没失败”当作概率界的证明。误差样本也不能升级为超越函数的全域正确舍入证明。

### 9.2 编译器、链接及 GC 专测

只为必要边界增加少量 fixture：拒绝继承 final class/修改只读状态的代表诊断、真实跨 `.slib` 导出的调用与 native 符号闭包、值配置/tuple 的 ABI、含 Large 存储对象在 moving GC 后继续计算。已经由通用规则覆盖的错误不按每个 math API 重复建组。

普通库测试与 fixtures 使用 release；debug/release、内联/去虚化对照只进入确有需要的编译器正确性/优化专测。GC stress 只用于共享 magnitude、存活 class、局部工作数组跨移动收集的定向测试，不用于全量 math 测试。源码和工具始终保留，不做“删除源码仍成功”验收。

### 9.3 基准与判读

首期基准覆盖 64 位内小值、256/1024/4096 位整数，以及 7/16/34/100/1000 位十进制的真实运算。记录时间、对象/字节分配和 GC，不只看一条算术指令。

- scalar 连续调用、带常量 context 的舍入和 multiplyHigh：检查普通内联/标量化，以及真实 libm 调用边界。
- 小 BigInteger 累加、小 BigDecimal 金额与比例计算：检查是否避免额外 magnitude 和嵌套 BigInteger 分配；仍允许公共结果 class 分配。
- 大整数乘除、模幂、十进制除法和字符串转换：检查算法复杂度、工作区复制和长循环，分别报告，不归因于同一种调用成本。
- 直接成员、单态化泛型、已知及未知 Equals/Compare 接口视图：确认已知目标能用现有优化，未知目标保持正确 icall。BigInteger 本身 final，不为测 vcall 人造一个可继承数值体系；vcall 的算法验收属于优化分支已有用例。
- provider/consumer 分离及同一 Cone 对照：只选代表操作验证跨 Cone 优化，不为每个方法复制冷/热构建测试。

不预设“零分配”或某个加速倍数。先固定工具链、目标、输入及优化分支 revision，再报告优化前后数据；原始日志放在仓库外，文档只保留范围、结果、未闭合问题和限制。

## 10. 本次文档变更边界

本次新增公共规范和实施设计，并在语言规范、std 范围和路线图中挂接设计入口。没有改动 Rust/C/Scoop 实现，也没有把库标记为已实现；因此本次仅执行文档链接与空白检查，不声称完成数值、GC、链接或性能验收。

后续实现遵守仓库的先格式化/lint、再定向验证流程。最终交付需包含三个现有目标的实际预编译库消费及测试，不将研究过某个上游或写好 API 清单等同于完成库。

## 11. 依据

- [语言规范：值类型、class、运算符、数组、数值与 FFI](../specs/SCOOP-SPEC.md)：4、9.1、9.3.6、10.4、11.2、11.11、13.2、13.4.1。
- [运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)：2.1～2.4、3.1、3.2、6.1；不改 managed 对象及 strict FP 合同。
- [实现规范](../specs/SCOOP-IMPL-SPEC.md)：2.7～2.9、2.18、2.19；普通库发布、调用与优化路径。
- [库 API 风格](../specs/SCOOP-API-STYLE.md)、[std 规范](../specs/SCOOP-STDLIB-SPEC.md)、[M62](../milestone62/DESIGN.md)。
- [Java Math](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/lang/Math.html)、[StrictMath](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/lang/StrictMath.html)：标量范围和精度/可复现性的区别。
- [Java BigInteger](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/math/BigInteger.html)、[BigDecimal](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/math/BigDecimal.html)：大数功能、表示及边界对照。
- [MathContext](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/math/MathContext.html)、[RoundingMode](https://docs.oracle.com/en/java/javase/26/docs/api/java.base/java/math/RoundingMode.html)：有效数字与舍入模式参照。
- [num-bigint](https://github.com/rust-num/num-bigint)、[发布记录](https://github.com/rust-num/num-bigint/blob/main/RELEASES.md)：可移植大整数算法及维护状态。
- [compiler-builtins/libm](https://github.com/rust-lang/compiler-builtins)：当前 Rust libm 的维护位置；作为研究资料，不是本期生产依赖。
