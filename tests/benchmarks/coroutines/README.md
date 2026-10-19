# 协程包装对照

`synchronous.scoop` 的 `leaf`／`wrapped` 模式各执行 50,000 次 startCoroutine，并校验 completion 次数。wrapped 经过四层非尾部同步 suspend 包装，输出 `1250225000`；leaf 输出 `1250025000`。对照应使用相同 core、runtime 源码、release app 和编译器构建模式，分别构建到独立输出／缓存。

预热后交替运行，校验 stdout；开启相同的 `SCOOP_GC_STATS=1` 和 `SCOOP_GC_WORKERS=1`，记录整程序时间、分配 bytes、GC 次数／累计暂停及 RSS。可通过 exec 的 argv[0] 参数令两版输入完全相同。编译对照直接重编 `.slib`，避开应用缓存；记录 compiler 自身构建模式、峰值内存、text／文件和 `.slib` 大小。不要在计时期间并行构建／测试。

C2 对照基线已包含 C1 tail forwarding，因此不能把此结果当作尾转交单项收益。真实挂起及跨线程语义使用 `tests/fixtures/m58-coroutines`，这里不模拟等待。原始日志置于仓库外，方法与结果见 `docs/milestone58/PERFORMANCE.md` 第 10 节。
