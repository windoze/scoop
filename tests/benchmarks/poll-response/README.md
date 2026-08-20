# 纯计算循环的 GC 请求响应

`program.scoop` 的工作线程只计算整数和，不分配对象、不调用 native helper，保留普通 Managed 循环的 safepoint。独立 collector 线程重复请求 full GC，每次收集后 NativeSafe 等待 100 微秒。

复用 [FFI 基准](../ffi/README.md) 的 `runner.c`、`leaf.c`、callback 生命周期及 STOPPING／STOPPED／RESUMING 观测点，不修改生产 runtime。按该文档创建启用 `SCOOP_THREAD_TESTING` 的 runtime 副本，再将 Scoop 输入替换成本目录的 `program.scoop`。runner 的 `BENCH_MODE=2` 选择实际 Scoop callback，`BENCH_GC=1` 启用 collector，`BENCH_ITERATIONS=100000000` 指定每个工作线程一亿次循环；分别用 `BENCH_THREADS=1` 和 `4` 运行。

runner 独立校验每个线程的结果为 `count * (count + 1) / 2`。它报告从请求到全部线程停稳及从请求到恢复的 p50／p99／最大值，同时记录实际 collection 数；`ns_per_call` 字段在本用例表示每次整数循环的聚合墙钟成本。计时包含一次 callback attach／detach，测试观测回调也有开销，不将此结果称为无观测开销的生产延迟。

最终 M33／M34 对照使用两套各自匹配的工具、core 和 runtime，七轮交替运行，保存每轮报告与 GC 统计。正常无请求路径的吞吐由原有 `primitive.scoop` 单独测量；本用例用于确认轻量 poll 在其他线程请求 GC 时仍及时响应。
