# 局部分配对照

`local.scoop` 的 `boxes`／`closures` 模式各执行 1,000,000 次局部装箱或作用域闭包调用；`traversals` 执行 200,000 次 64 元素数组遍历，每次创建捕获本轮 offset 和共享 Total 的闭包。预期输出依次为 `1000000000000`、`500000500000`、`1280396800000`。对象字段写入和实际数组读取保留，不进入 M58 的纯整数循环分块。

用相同编译器构建模式、core、runtime 和 release app，对比一次普通优化且关闭分配消除与当前三轮优化。基线是局部功能关闭对照，不是历史版本的整体性能。编译器临时开关只用于生成独立工具，结束后恢复源码并重建。

固定 `SCOOP_GC_STATS=1`、`SCOOP_GC_WORKERS=1` 和 argv[0]，预热后交替运行至少十一对，校验输出并记录整程序耗时、分配、GC 次数／累计暂停和 RSS。独立重编 `.slib` 对照编译时间／内存及 text、文件大小；计时期间不并行构建或测试。原始数据置于仓库外，汇总见 `docs/milestone58/PERFORMANCE.md`。分配减少可能伴随根的活跃区间和机器代码改变，不能据此宣称每个遍历场景都加速。

`classes.scoop` 用于普通 class 字段替换的历史版本对照。复用 `../interfaces/phases.c`，编译为 `libm34_interfaces.a` 后通过 `--library-path` 链接。`SCOOP_BENCH_KIND=0/1/2/3` 分别运行 Point、局部 Cursor、真实 ArrayList 迭代和无对象的 Point 算法对照；`SCOOP_BENCH_COUNT` 决定外层次数。Cursor 与 ArrayList 每轮消费 16 个值，xorshift 状态阻止把循环简化为闭式结果。

使用同一 sysroot、runtime 与 release profile，分别生成旧／新版二进制及 MIR／LIR，再用 `compare.py` 交替运行。正常计时关闭 GC stress／统计，分配统计另作诊断；同时记录内核计时、整程序时间、实际分配和编译／产物成本。`count=100` 时 Point／scalar 输出 `2330009635`，Cursor／ArrayList 输出 `2254931208`，已经独立参考计算与运行核对。适用范围记入 `docs/program-performance/CLASS-SCALARS.md`；不把真实集合尚未消除的迭代器分配报告为已优化。原始文件保存在忽略的 `tmp/program-performance/class-scalars`。
