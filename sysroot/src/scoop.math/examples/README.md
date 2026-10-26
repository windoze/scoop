# 数学库使用示例

先按[运行准备](../../../README.md#运行准备)安装或准备包含 `scoop.math` 的 sysroot。每个文件首行的 `//! dep` 已声明依赖，可以单独运行；以下命令在本目录执行。

```sh
scoop run --release scalars.scoop
scoop run --release integers.scoop
scoop run --release decimals.scoop
scoop run --release primes.scoop
```

| 示例 | 内容 |
| --- | --- |
| [scalars.scoop](scalars.scoop) | 受检加法、溢出异常、floor/ceil 整除、钳制、浮点范数和舍入 |
| [integers.scoop](integers.scoop) | 128 位以上整数、泛型加法、商余数、进制、字节序、平方根和模逆 |
| [decimals.scoop](decimals.scoop) | 金额计算、有效数字与小数位、pow/sqrt、相等/hash、浮点构造和格式 |
| [primes.scoop](primes.scoop) | 复用显式 Random、生成指定长度 probable prime、素性检查和模幂 |

## 加入自己的 Cone

在应用 `Cone.toml` 中声明普通依赖，不需要单独配置 libm：

```toml
[dependencies]
"scoop:scoop.math" = "0.1.0"
```

再导入所用实体：

```scoop
import scoop.math.BigDecimal
import scoop.math.MathContext
import scoop.math.RoundingMode
```

## 定宽整数与浮点

`addExact(Long.MAX_VALUE, 1L)` 抛 `ArithmeticException`。普通整数运算仍遵守 core 的 wrapping 规则；要检查溢出时明确调用 Exact 函数。

`floorDiv(-7L, 3L)` 为 -3，配套余数为 2；`ceilDiv` 为 -2，配套余数为 -1。两组都满足数学整数关系 `a = q*b+r`。Float / Double 重载不混用类型，例如 `hypot(3f, 4f)` 返回 Float。

`rint` 按最近偶数返回浮点，`round` 在中点朝正无穷返回整数，所以 `rint(2.5)` 为 2.0，`round(2.5)` 为 3。需要单次舍入的乘加使用 `fma`。

## BigInteger

`integers.scoop` 从十进制文本读取 2^128。`divideAndRemainder` 一次返回 `(quotient, remainder)`；`sqrt` 返回向下取整的整数平方根。`toByteArray` 返回独立补码快照，`fromByteArray` 接受明确字节序；只需要绝对值字节时选 `toMagnitudeByteArray`。

位操作使用无限符号扩展的补码语义，所以 `BigInteger(-3L) shr 1L` 为 -2。`%` 的非零结果与被除数同号；需要 `[0, modulus)` 内的正模结果时使用 `mod`。

`twice<T : Add<T,T>>` 展示普通泛型运算。无需引入数值基类；BigInteger 和 BigDecimal 都显式实现现有运算符接口。

## BigDecimal：有效数字与 scale

`MathContext(16L)` 规定最多 16 位**有效数字**，默认 HalfEven。`setScale(2L, HalfEven)` 规定两位**小数位**。例如金额示例计算 `19.95 * 3 = 59.85`，税额精确为 `4.48875`，舍入到两位小数为 `4.49`。

默认运算精确：`1 / 8` 可表示为 `0.125`，`1 / 3` 抛异常。指定 context 的除法得到 `0.3333333333333333`。`MathContext.Unlimited` 也要求精确，不能用它消除非终止除法错误。`Unnecessary` 在舍入将改变数值时抛异常，丢弃全零尾数合法。

`"1.0"` 与 `"1.00"` 数值相等，作为 HashMap key 只占一个位置，但其 scale、`toString()` 和 `sameRepresentation` 仍可区分。`toString()` 能恢复同一表示；`toPlainString()` 不使用指数，只保证数值往返。

通过文本构造十进制输入：`"0.1".toBigDecimal()`。已有浮点值时，`fromDouble` 采用其最短往返文本；`fromDoubleExact` 展开实际二进制值，因此例子中的后者输出 `0.1000000000000000055511151231257827021181583404541015625`。两个入口都有用途，应按输入含义选择。

## 素数与随机源

`primes.scoop` 使用 `Random.seeded` 让演示路径可复现；math 接收这个已有随机源，不创建隐藏 RNG。`probablePrime(128L, random, 64)` 保证结果恰为 128 位。`certainty` 为正数，在随机位独立均匀的前提下，固定合数的 Miller–Rabin 误判概率不超过 `2^-certainty`；不能把任意自定义 Random 自动视为满足这一前提。示例不用于密钥生成。
