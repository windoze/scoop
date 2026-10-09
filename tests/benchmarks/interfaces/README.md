# 接口调用与转换成本

provider 提供四个普通公开循环函数，consumer 从独立 Cone 调用，保证接口参数的未知实际类型不会因 caller 内联而消失。循环保留 Managed safepoint，所有模式对同一个 UInt 输入执行相同的两百万次变换，输出最终值。

| `SCOOP_BENCH_KIND` | provider 中的调用点 |
| --- | --- |
| 0 | final Cell 参数转成 Reader，receiver 的实际类型已知 |
| 1 | Reader 参数，实际类型未知 |
| 2 | Any 参数在循环前转换一次，再调用多次 |
| 3 | Any 参数在每轮调用前转换一次 |

`SCOOP_BENCH_COUNT` 可以覆盖循环次数。`phases.c` 只读取环境变量和系统时钟，不依赖任一版 runtime 内部布局；stderr 第一行是 kernel 的墙钟与进程 CPU 纳秒。开启 `SCOOP_GC_STATS=1` 后，第二行保留该进程的真实 GC 统计。开始计时前已分配 Cell，全部循环结束后才打印 checksum。

先用 C11／O2 编译 `phases.c` 并打包为 `libm34_interfaces.a`，再使用配套的 `scoop`／`scoopc` 按 release 构建 provider 到 `artifacts/dev.m34/interface-bench-provider/0.1.0/cone.slib`。consumer 使用 `--cone-path artifacts --library-path <native archive directory>` 构建；两次构建都指定同一 sysroot／runtime、目标与普通缓存。`--emit all --dump-dir <directory>` 保留 MIR／LIR，结合目标 `llvm-objdump` 检查调用和转换的实际位置。

最终 M33／M34 对照按模式各七轮，逐轮交换顺序，关闭继承的 GC stress／worker 覆盖。它同时反映接口表示、MIR 优化和条件 poll 的累计变化；不把总加速归给单一优化。完整参数、命令、全部样本与机器码在 [M34 性能记录](../../../docs/milestone34/PERFORMANCE.md)。
