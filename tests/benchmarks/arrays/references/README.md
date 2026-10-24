# 引用数组基准

复用 `../../interfaces/phases.c` 的阶段计时。`SCOOP_BENCH_COUNT` 为正的数组长度，通常为 65536，短路径取 1；`SCOOP_BENCH_KIND` 选择：

| kind | 操作 |
| --- | --- |
| 0 | 不同 Node 对象的字段顺序求和 |
| 1 / 15 | 按引用身份搜索末项／首项 |
| 2 | class 引用复制 |
| 3 | 泛型父接口值复制，混合 class 与 boxed struct |
| 4 | 泛型 Child 接口逐元素投影到第二个 Parent 表 |
| 5 / 6 | String／捕获引用的闭包复制 |
| 7 / 8 | 可空 class 引用读取求和／复制 |
| 16 / 17 / 18 | 可空引用求和：全 None／全 Some／固定种子伪随机混合 |
| 9 | `MaybeUninit<Node>` 清零 |
| 10 / 11 / 14 | class 引用／未初始化 wrapper／接口数组的分配与初始化 |
| 12 / 13 | NoGC 整数内核／已有标量数组求和对照 |

通常执行 128 轮；长度小于 256 及首项命中执行 10 万轮。除模式 10／11／14 测量的分配与初始化外，输入和目标数组均在阶段计时前准备。Node 读取使用逐项独立对象；接口包含多继承、泛型完整实例和装箱后的值类型。可空读取默认交替 Some／None；伪随机模式在初始化时用固定种子生成 32-bit LCG 高位选择，不在被测读取内核中生成随机数。阶段后读取目标数组，stdout 结果必须一致。

用同一源码、相同预编译 sysroot 和两版 release 工具构建，再用 `tests/benchmarks/compare.py` 做串行 AB／BA 对照。阶段时间来自 stderr，GC 诊断单独运行。检查 `exercise` 中的实际内联副本，不能仅凭独立函数推断成本。

正确性、优化合同及 GC 由对应 compiler-correctness／GC fixture 定向覆盖。原始数据放在 `tmp/program-performance/reference-arrays`，摘要见性能文档。
