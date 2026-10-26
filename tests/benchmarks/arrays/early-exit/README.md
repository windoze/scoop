# 数组搜索与提前退出基准

复用 `../../interfaces/phases.c` 的阶段计时，初始化在计时前完成。`SCOOP_BENCH_COUNT` 为正的数组长度（通常 65536，短路径为 1），`SCOOP_BENCH_KIND` 选择：

| kind | 操作 |
| --- | --- |
| 0 / 1 / 2 / 3 | Long 搜索，末项／未命中／首项／中间命中 |
| 4 | 返回本轮读取值的阈值搜索 |
| 5 | 两个 Int 数组首次不相等 |
| 6 | 首次命中前逐元素写入目标数组 |
| 7 | 严格 Double 顺序累加，在 sentinel 处 break |
| 8 | 搜索命中后构造对象，保存原数组、索引和元素快照 |
| 9 | 已有直线求和对照 |
| 10 | NoGC 整数内核对照 |
| 11 | 同一数组首次不相等，完整遍历 |

通常执行 128 轮；短数组及首项命中执行 100 万轮。用相同源码构建基线和候选 release 程序，再用 `tests/benchmarks/compare.py` 串行 AB／BA 对照。阶段时间来自 stderr，stdout 是必须一致的结果；分配／GC 诊断与正常计时分开。检查内联进 `exercise` 的实际执行路径，不能只检查独立函数。

正确性与优化合同由对应 compiler-correctness／GC 定向 fixture 承担。原始产物与样本放在 `tmp/program-performance/early-exit-arrays`，摘要见性能文档。
