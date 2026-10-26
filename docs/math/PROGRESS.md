# Math 实施记录

日期：2026-10-26。分支：`codex/math-library`。已合并远端 main `b42f17b7c`；底层来源修复单独提交为 `047cfeecf`。

## 交付

- `scoop:scoop.math:0.1.0`：八种整数的标量 API、Float/Double 数学函数、不可变 final BigInteger/BigDecimal、MathContext struct 与 RoundingMode enum。
- Small 保存完整 Long；Large 使用不可变 UInt magnitude。Small 加乘除及十进制位数统计使用标量路径，BigDecimal 不嵌套额外 BigInteger 对象。大数算法、宽 scale、舍入和转换使用普通 Scoop；只有标量初等函数调用 libm。
- 大整数包含正规化长除法、schoolbook/Karatsuba、不平衡乘法、Montgomery 模幂、整数平方根、无限补码位操作和显式 Random 的素数 API。二的幂进制直接按位分组。
- 十进制包含八种舍入、精确和有限精度算术、负指数、单次正确舍入的 pow/sqrt、Long scale 边界、数值相等/hash、直接 binary32/binary64 转换。有限精度 pow 以精确系数幂为基线，计算成本随实际中间结果增长。
- [四组可运行示例及中文说明](../../sysroot/src/scoop.math/examples/README.md)、[库说明](../../sysroot/src/scoop.math/README.md)、普通预编译消费者测试 Cone 和[规模基准](../../tests/benchmarks/math/README.md)。
- num-bigint 0.5.1 的参考 revision 与 MIT 许可存于 [vendor](../../sysroot/vendor/num-bigint/README.scoop.md)，由既有发行脚本收集。

## 底层缺口闭合

BigInteger 浮点转换返回导入的 Infinity 常量时，HIR 把 provider definition span 用作 consumer 控制结果赋值的 span；导出内联正文因文件与偏移不匹配而失败。

已先修实现规范及 HIR：控制结果的赋值/丢弃语句使用 evaluation span，常量自身仍保留 provider definition origin。回归覆盖 if、when、try/finally，并检查两个来源和最终 foundation 导出。随后正常编译库，未在 math 中改写常量或绕开产物导出。

## 验证

| 范围 | 结果 |
| --- | --- |
| `cargo fmt --all`、`cargo clippy --workspace --all-targets --release` | 通过 |
| HIR `m23_ordinary_dependency_constants` | 4 项通过 |
| Darwin math Cone `scoop test --release` | 39 项通过 |
| Darwin 预编译消费者 | 2 项通过：平台功能及存活存储组合 |
| Darwin moving GC、minor GC | 各只运行 `retainedStorageAfterCollection`，通过 |
| Linux GNU、musl | 各实际发布 `.slib`，独立消费者的 `platformSmoke` 通过 |
| API 文档 | `scoop doc --no-deps --deny-doc-warnings` 通过 |
| 四个单文件 examples | 均消费预编译库成功运行 |

数值测试包括各整数宽度边界、IEEE 特殊值与非平凡浮点样本、29 组独立整数除法向量、Karatsuba/不平衡乘法对照、奇偶模数、素数与伪素数、全舍入模式、56 组四种十进制运算参考向量、20 组平方根向量及双重舍入反例。Python decimal/libmpdec 4.0.1 的参考向量在生成时使用精确正幂；负幂在目标精度对其倒数舍入。测试运行不依赖参考工具。

消费者覆盖普通单态化泛型、未知 Equals/Compare 接口参数、MathContext 值参数、商余数 tuple、集合数值相等以及 class/数组保留共享存储。LIR 核对显示泛型加法内联到具体大数调用；未知接口仍为 `dispatch[Interface:0]`，IntegerStorage 的 Large 引用槽与 managed poll 保留。ARM64 机器码保留必要的分配/收集路径，没有把大数 class 宣称为零分配。

Linux 使用 nuc12 的独立工作目录。已有 sysroot 资源的 runtime ABI 与本分支不一致，已在该目录重新准备 core/std/test/math 及 runtime。GNU 使用 GCC，musl 使用 musl-gcc 与 Linux UAPI 头。依用户要求，Linux 只运行各一项定向功能测试，没有运行完整库、Rust workspace 或 fixture 套件。

## 性能样本与取舍

Apple M4 Pro、macOS 27.0.1、LLVM 22.1.8，release；工具为上述 main 加 HIR 修复。复用普通 GC，关闭 stress。乘法基线只把 schoolbook 分界提高到测试规模之外，其他整数实现相同；两种程序交替运行七轮并核对 checksum。以下为 kernel 墙钟中位数；原始时间、GC JSON、LIR 与机器码保存在仓库外的 math 工作目录。

| 乘法输入 / 次数 | schoolbook | 当前分界 64 limbs | 比值（基线/当前） |
| --- | ---: | ---: | ---: |
| 64 limbs / 10000 | 30.54 ms | 31.27 ms | 0.98 |
| 65 limbs / 10000 | 31.97 ms | 29.57 ms | 1.08 |
| 80 limbs / 10000 | 47.86 ms | 39.70 ms | 1.21 |
| 96 limbs / 10000 | 67.65 ms | 51.80 ms | 1.31 |
| 4096 bits / 1000 | 12.36 ms | 8.41 ms | 1.47 |
| 8192 bits / 1000 | 50.88 ms | 20.50 ms | 2.48 |
| 32768 bits / 1000 | 816.64 ms | 111.59 ms | 7.32 |

64 limbs 两边仍走同一 schoolbook 路径；小幅时间差不作为算法收益。65 limbs 起已有收益，保留 64 的分界；不把它固化为 API 或合法输入上限。256/1024 位也已测量，两边均使用 schoolbook。

Small 十进制位数统计改为标量计数后，10000 次 Decimal64 金额累计的七轮交替中位数从 7.057 ms 降到 1.604 ms，整个进程分配字节从 9,756,968 降到 560,712，minor collection 从 9 次降到 0 次。这是特定热路径的数据，结果 BigDecimal 对象仍分配。

其他五轮样本如下。分配字节来自整个进程，包含初始化；时间只覆盖 benchmark kernel。现有统计记录累计字节和收集次数，没有将存活对象数当成累计分配对象数。

| 负载 | 次数 | 中位数 | 分配字节 | minor / full |
| --- | ---: | ---: | ---: | ---: |
| Small BigInteger 泛型累计 | 10000 | 0.307 ms | 960496 | 0 / 0 |
| 256 位因子的整数除法 | 1000 | 0.493 ms | 741616 | 0 / 0 |
| 1024 位因子的整数除法 | 1000 | 1.803 ms | 1654768 | 1 / 0 |
| 4096 位因子的整数除法 | 1000 | 16.057 ms | 5314296 | 5 / 0 |
| 7 位十进制除法及格式化 | 1000 | 2.246 ms | 3561400 | 3 / 0 |
| 16 位十进制除法及格式化 | 1000 | 2.473 ms | 4169400 | 3 / 0 |
| 34 位十进制除法及格式化 | 1000 | 4.053 ms | 7554880 | 7 / 0 |
| 100 位十进制除法及格式化 | 1000 | 7.534 ms | 15306200 | 14 / 0 |
| 1000 位十进制除法及格式化 | 100 | 13.438 ms | 23819072 | 23 / 0 |

这些是本机代表负载，不是跨语言、跨机器或所有规模的性能保证。未增加专用 math 优化 pass、并行数值内核或新的 runtime ABI。初等函数继续采用目标 libm 的精度保证；本次交付不声称 StrictMath 位级可复现性。
