# 已知小型 box 的实际分配

当前已扩展到所有 profile 和所有已知 box 布局，旧 box C 入口已退役。下面保留最初的小型 box 阶段数据；后续实现、回归和独立成本见 [Runtime 职责收敛](RUNTIME-RESPONSIBILITIES.md)。

基线为 `7d2452435`。release 的 typed BoxValue 直接消费已选 shape 的完整实例 size／alignment；实例不超过 128 bytes 时复用普通 TLAB、清零和 collecting slow path，再按实际 offset／alignment 复制完整 payload。ZST 每次仍分配独立对象，不构造零尺寸 payload。较大 box、debug 及原 C ABI 后备保持。

含引用源 place 使用已有 statepoint live set，分配后从移动回写的存储重新复制；生成路径不进入 box C 入口，因此省去其额外 native region frame、动态 shape／place 检查、managed anchor 和入口 poll。循环与函数既有 poll 保持。复制后复用值写屏障，不假定慢路径一定返回年轻对象；packed、padding、enum 父整数解箱及对象身份保持。新增实例布局只是既有 shape 的 LIR 内存投影，未增加 `.slib` 字段、runtime ABI、对象字段或诊断代码。

两机格式化／Clippy 和 8 项 codegen 单测通过，覆盖小型生成路径、GC relocation、值屏障、较大共享后备、普通 runtime call 不可冒充 typed box。扩展已有 box／unbox fixture，三目标各两个 profile、6 次进程调用通过：ZST 身份、packed 与可观察 padding、含引用 nested payload、MaybeUninit、跨 Cone struct box、整型 enum 与父整数、错误 cast，以及 moving／minor stress。新增测试 Cone 的 package 声明准备问题已修正，未改变语言规则。

基准将结果存入 256 槽的外部数组，装箱真实逃逸；输入建立和最终读取在计时区间之外。一次预热、三对 AB／BA，每轮一百万次，中位数 ns／次为旧 → 新：

| box 实例 | Darwin ARM64 | GNU x86-64 | musl x86-64 |
| --- | ---: | ---: | ---: |
| ZST，16 B | 17.089 → 3.671 | 18.568 → 4.580 | 19.404 → 4.903 |
| Long，24 B | 20.736 → 3.930 | 23.471 → 7.859 | 31.702 → 5.317 |
| 两引用＋Long，40 B | 35.308 → 7.354 | 36.594 → 10.256 | 43.290 → 11.420 |
| 宽值，144 B，保留后备 | 24.308 → 23.903 | 28.816 → 28.860 | 45.054 → 45.445 |

三平台机器码确认前三项无 box C 调用，引用项无 native region push／pop；宽值仍保留一次 `scoop_rt_box_value`，其小幅波动不计作收益。十万次各项分配字节和 minor 次数完全保持：1,606,328／2,406,328／4,006,328／14,406,328 B，minor 1／2／3／13；其中 6,328 B 来自输入及输出。引用项 root_slots 228→222，其他项不变，没有把减少调用误报为减少实际装箱。

text 增加 960／960／896 B，完整文件变化 0／0／+4,096 B。三对独立冷 consumer 编译墙钟 −1.38%／−0.54%／−0.04%，CPU +1.13%／−0.53%／+0.14%，峰值 RSS −1.37%／−0.10%／−0.08%；库和 runtime 使用相同输入。该有界策略进入 release 默认，不增加公开开关。

原始日志、GC 数据和机器码在仓库外 `scoop-performance-results/2026-10-26/runtime-visible/box-allocation`，构建在 `tmp/program-performance/box-allocation`，源码为 `tests/benchmarks/scalars/boxing.scoop`。
