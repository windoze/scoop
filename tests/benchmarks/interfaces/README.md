# 接口调用与转换成本

provider 提供普通公开循环函数，consumer 从独立 Cone 调用，保证接口参数的未知实际类型不会因 caller 内联而消失。循环保留 Managed safepoint，各模式从同一个 UInt 输入开始，默认循环两百万次，输出最终值。

| `SCOOP_BENCH_KIND` | provider 中的调用点 |
| --- | --- |
| 0 | final Cell 参数转成 Reader，receiver 的实际类型已知 |
| 1 | Reader 参数，实际类型未知 |
| 2 | Any 参数在循环前转换一次，再调用多次 |
| 3 | Any 参数在每轮调用前转换一次 |
| 4 | ChildReader 参数，每轮调用两次父接口 Reader 的方法 |
| 5 | Any 参数，每轮 `is Reader` 成功后调用两次 Reader 方法 |
| 6 | Any 参数，每轮 `is ChildReader` 成功后调用两次父接口 Reader 的方法 |
| 7 | MultiReader 参数，通过第二个父接口 Reader 调用两次 |
| 8 | DiamondReader 参数，通过菱形共同父接口 Reader 调用两次 |
| 9 | 装箱 Int 的 Hash 视图，15 项目录中的较早位置 |
| 10 | 装箱 Int 的 Add<Int, Int> 视图，目录中段 |
| 11 | 装箱 Int 的 Mul<Int, Int> 视图，目录末段 |
| 12 | 装箱 Int 查询 Add<Long, Long>，完整 exact 泛型未命中 |
| 13 | 无接口对象查询 Hash，空目录 |
| 14 | Cell 查询 Hash，单项目录未命中 |
| 15 | ArrayList<Int> 的 Collection<Int> 视图及 size getter |
| 16 | 装箱 Int 的纯 Boolean `is Mul<Int, Int>`，保留完整类型关系入口 |
| 17 | 装箱 Int 与 Cell 交替查询 Hash，成功／失败混合 |

模式 0～3 每轮变换一次，模式 4～8 每轮变换两次；只在相同模式、相同次数下比较编译器版本。ChildReader 保留自己的 `tag` slot，避免把空接口转换的零成本误算为查表优化。

模式 9～17 使用真实 core 装箱和集合实现，构造均在计时前完成。目录位置以当前产物核对，不是语言保证；这些模式不会为测量构造大量虚假接口。数值累加遵循 Int 的环绕语义，模式 13／14 的校验和为零。

`SCOOP_BENCH_COUNT` 可以覆盖循环次数。`phases.c` 只读取环境变量和系统时钟，不依赖任一版 runtime 内部布局；stderr 第一行是 kernel 的墙钟与进程 CPU 纳秒。开启 `SCOOP_GC_STATS=1` 后，第二行保留该进程的真实 GC 统计。开始计时前已分配 Cell，全部循环结束后才打印 checksum。

先用 C11／O2 编译 `phases.c` 并打包为 `libm34_interfaces.a`，再使用配套的 `scoop`／`scoopc` 按 release 构建 provider 到 `artifacts/dev.m34/interface-bench-provider/0.1.0/cone.slib`。将 consumer 复制到工作目录的 `consumer/`，产物放在同一工作目录的上述 `artifacts/` 路径；consumer 清单显式引用它，并使用 `--library-path <native archive directory>` 构建；两次构建都指定同一 sysroot／runtime、目标与普通缓存。`--emit all --dump-dir <directory>` 保留 MIR／LIR，结合目标 `llvm-objdump` 检查调用和转换的实际位置。

最终 M33／M34 对照按模式各七轮，逐轮交换顺序，关闭继承的 GC stress／worker 覆盖。它同时反映接口表示、MIR 优化和条件 poll 的累计变化；不把总加速归给单一优化。完整参数、命令、全部样本与机器码在 [M34 性能记录](../../../docs/milestone34/PERFORMANCE.md)。

后续接口视图优化使用新增的模式 4～8。分别用基线／候选编译器构建相同源码，再用 [compare.py](../compare.py) 的双方环境覆盖固定 `SCOOP_BENCH_KIND` 与 `SCOOP_BENCH_COUNT`，执行相同输出检查和交替对照。工具记录进程总时间；`phases.c` 的 kernel 时间作为原始 stderr 保留，二者不能混用。

`parentView`、`secondaryView`、`diamondLeft`、`diamondRight` 是公开的真实接口返回边界，供机器码检查零偏移和非零偏移的父表投影。它们不是循环测速结果；继承调用和接口值构造分别验证。
