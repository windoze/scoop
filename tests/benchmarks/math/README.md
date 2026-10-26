# 数学库规模基准

消费已准备 sysroot 中的 `scoop.math.slib`，复用接口基准的时钟与环境参数读取，不新增测量 runtime。

```sh
scoop build --release tests/benchmarks/math -o /path/to/work/math-bench
SCOOP_BENCH_KIND=14 SCOOP_BENCH_COUNT=1000 SCOOP_GC_STATS=1 /path/to/work/math-bench
```

| kind | 运算 |
| --- | --- |
| 0 | Small BigInteger，通过 `Add<T,T>` 泛型累计 |
| 1 | Small BigDecimal，Decimal64 金额累计 |
| 10–17 | 256、1024、2048、3072、4096、8192、16384、32768 位整数乘法 |
| 20–27 | 同样大小的整数因子构造被除数，再求商余数 |
| 30–34 | 7、16、34、100、1000 位十进制除法和文本转换 |
| 101 以上 | `kind - 100` 个 32 位 limb 的乘法，用于测量阈值附近的规模 |

`SCOOP_BENCH_COUNT` 必须显式选择合理次数；大数测试不使用时钟 helper 的默认两百万次。输出 checksum，stderr 输出 kernel 墙钟和 CPU 纳秒；可选 GC 统计是整个进程的分配、字节和收集数据。输入与循环内数值随 index 变化，结果被实际消费。

交替运行基线/候选版本并检查 checksum 相同，再比较多次中位数。乘法阈值仅是实现选择，不是合法输入限制。大数类仍允许结果对象分配，不能从一个热路径推断整库零分配。实际目标、工具 revision 和测量结果见 [math 实施记录](../../../docs/math/PROGRESS.md)。
