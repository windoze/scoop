# scoop.math

普通数学库 `scoop:scoop.math:0.1.0`，显式依赖后用 `import scoop.math.*` 或单独导入函数。提供八种整数的受检算术、Float/Double 数学函数、不可变 `BigInteger` / `BigDecimal`、值类型 `MathContext` / `RoundingMode`。

从[可运行示例](examples/README.md)开始；完整边界见[数学库规范](../../../docs/specs/SCOOP-MATH-SPEC.md)，算法及优化说明见[设计](../../../docs/math/DESIGN.md)。

## 选择入口

| 需求 | 入口 |
| --- | --- |
| 定宽整数算术，溢出必须报错 | `addExact`、`multiplyExact`、`powExact` 等具体类型重载 |
| 明确整数商的取整方向 | `floorDiv` / `floorMod`、`ceilDiv` / `ceilMod` |
| 浮点初等函数和 IEEE 边界 | `sqrt`、`log1p`、`fma`、`nextUp`、`scalb` 等 |
| 超过 64 位的精确整数 | `BigInteger`，支持算术、位操作、字节、模运算和素数 |
| 精确十进制金额或显式有效数字 | `BigDecimal`；`MathContext` 控制有效数字，`setScale` 控制小数位 |

两种大数都是 final、不可变的普通 managed class。`a + b` 返回结果，`var a; a += b` 重新绑定变量；输入对象和已发布的数字数组不会被修改。大数与标量不隐式混算，先用构造器或 `toBigInteger` / `toBigDecimal` 转换。

BigDecimal 的 `==`、排序和 hash 按**数值**，忽略 scale；`sameRepresentation` 比较 coefficient 与 scale。默认 `/` 要求有限、精确的十进制商，`1 / 3` 抛算术异常；用 `div(other, MathContext(...))` 显式舍入。`fromDouble(0.1)` 得到十进制 `0.1`，`fromDoubleExact(0.1)` 保留二进制浮点的精确数值。

普通三角、指数和对数函数采用目标系统 libm 的精度保证，不承诺所有平台最后一位相同或 Java Math 的统一 ULP 界。Float 入口按 binary32 运算，BigDecimal 到 Float 的转换也直接按目标精度舍入。

随机源继续由 `scoop.random.Random` 提供，素数 API 显式接收它。math 没有自带 RNG；大数算法不承诺常数时间执行。

## 构建与测试

在仓库根目录完成[开发 sysroot 准备](../../../scripts/README.md)后：

```sh
export SCOOP_SYSROOT="$PWD/sysroot"
scoop test --release sysroot/src/scoop.math
scoop test --release sysroot/src/scoop.math/consumer-tests
```

第二条消费已发布的 `scoop.math.slib`，包括泛型运算、接口视图、集合、共享存储及显式 GC 后的运算。Linux 最小定向确认使用 `--filter platformSmoke`，无需运行完整套件。

库内的除法向量由 Python 整数生成；十进制向量由 Python decimal / libmpdec 4.0.1 生成，固定 seed 为 20261026。正幂先以足以容纳完整系数的 2000 位精度计算，再舍入一次；负幂用一除以该精确正幂，在目标精度舍入。平方根用 libmpdec 的正确舍入结果。向量随源码保存，测试运行不依赖 Python、Java 或联网。

大整数乘除算法参考 num-bigint 0.5.1；实际参考 revision、许可与移植范围见 [上游说明](../../vendor/num-bigint/README.scoop.md)。
