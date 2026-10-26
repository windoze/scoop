# TaskContext fork 的生成分配

旧 fork C 入口现已退役，首次 root 建立仍由 runtime 负责。下面保留最初生成分配阶段的数据，当前全部 profile 与后备职责见 [Runtime 职责收敛](RUNTIME-RESPONSIBILITIES.md)。

基线为 `379a99939`，复用已有 TLAB、接口 SSA 和根恢复。预热后的 Darwin `sample` 中，主线程 1,130 个样本有 196 个位于 fork 及其分配、根登记，current／snapshot 合计 39 个，enter／leave 合计 58 个；三个主要等待中的后台线程未计入这些比例。此次先处理 fork，小访问器、绑定树和切换继续使用共享 runtime。采样是选择实现顺序的依据，不是每次操作的周期模型。

真实 MIR `ContextOperation::Fork` 在根计划之前降低为普通 managed Alloc 和 HeapStore。分配与 TD／scan 共用 generated TaskContext 的布局：24 B、对齐 8，root 在 offset 16。snapshot 只求值一次，作为分配后写入的实际输入保活并恢复；HeapStore 消费搬迁后的引用并执行既有屏障。空 snapshot 也分配独立 TaskContext，绑定树及其中对象的身份按原语义共享。这个输入不会被“返回后已死实参”规则误删，因为降低后的字段写入仍真实使用它。

所有 profile 使用同一基本操作，release 默认得到已有 TLAB 快路径。没有新增 runtime 符号、TLS 字段、隐藏参数、诊断代码或 ABI；runtime contract 18 保持。旧产物可继续使用，其中的 ContextFork 调用保留，重新编译包含该操作的正文才获得优化。初始 root 创建、旧调用和普通 extern 的 C 后备继续存在，current／snapshot、push／restore、enter／leave 也保持原协议。

两机先格式化与 workspace Clippy，再通过定向 LIR 根恢复和 C Context moving 测试。三目标各通过四组正式组合：协程 fork 的 debug／release、并发 callback、绑定释放时机及异常出口，共七次程序调用。复用并加强原协程用例，只有 minor 运行增加短命对象流，避免仅设置环境却未发生收集；debug 实际复制 77,672 B，release 发生 48 次 minor、15 次显式 full 并复制 18,584 B。moving 与 minor 分开运行，验证父子重绑定独立、挂起／恢复、泛型 binding、同步 completion、并发 callback 与消费后的释放。未执行完整验收。

性能使用现有 `benchmarks/coroutines/tail.scoop` 和 `synchronous.scoop` 的两百万轮版本，两方固定相同源码和 runtime，各自使用配套预编译系统库。每组一对预热、五对 AB／BA，普通 release，不开启 GC stress／统计；没有固定 CPU affinity。双方的 92 个 runtime 目标文件逐字节相同。下表为含启动与输出的墙钟中位数，单位秒：

| 程序路径 | Darwin ARM64，旧 → 新 | GNU x86_64，旧 → 新 | musl x86_64，旧 → 新 |
| --- | ---: | ---: | ---: |
| tail，同步完成 | 0.215237 → 0.173000 | 0.262215 → 0.216739 | 0.277237 → 0.226294 |
| tail，实际挂起／恢复 | 0.262818 → 0.221979 | 0.328508 → 0.279035 | 0.336209 → 0.280499 |
| synchronous，leaf | 0.142240 → 0.098596 | 0.161090 → 0.121657 | 0.169547 → 0.127292 |
| synchronous，四层 wrapper | 0.137376 → 0.096094 | 0.161520 → 0.113859 | 0.172031 → 0.127736 |

ARM64 耗时降低 15.54%～30.68%，GNU 降低 15.06%～29.51%，musl 降低 16.57%～25.75%。输出均按预期核对：前三项为 `2000001000000`，wrapper 为 `2000009000000`。两个架构均为实机数据，没有从另一平台外推或把首跑的 Darwin 冷启动延迟计入预热后结果。

最终机器码中，两个程序实际使用的 coroutine-start 入口都不再调用 `scoop_rt_context_fork`，成功分配路径直接完成 bump、登记及字段写入；TLAB 不满足时进入 `scoop_runtime_alloc_slow`，恢复 snapshot 后再写 root。入口 poll、context 切换、异常根和用户调用保持。每个被展开入口从 ARM64 151 条增至 251 条指令，x86_64 从 167 条增至 259 条；这些是完整函数指令数，不是动态执行次数。省去的是 fork 内部的调用边界、额外 managed anchor／native root 登记和重复线程状态获取，实际 TaskContext 分配仍在。

另行 GC 统计确认四模式的分配字节、minor／full 次数各平台前后一致。tail 约分配 672 MB、644 次 minor／1 次 full；synchronous 约分配 416 MB、398 次 minor／0 次 full。每轮仍创建独立 TaskContext，没有用减少任务数量取得收益，也没有新增总分配次数探针。三对冷 consumer 构建与体积对照如下：

| 成本 | Darwin ARM64 | GNU x86_64 | musl x86_64 |
| --- | ---: | ---: | ---: |
| 每个程序 text 增量 | +400 B | +384 B | +384 B |
| 完整程序文件增量 | 0 B | 0 B | 0 B |
| core.slib 增量 | +91,818 B（0.20%） | +134,694 B（0.25%） | +134,900 B（0.25%） |
| consumer 编译墙钟变化 | +4.89% | −0.63% | +0.07% |
| consumer 编译 CPU 变化 | +1.08% | −0.33% | +0.22% |
| consumer 编译峰值 RSS 变化 | +1.24% | +0.23% | +0.22% |

ARM64 编译墙钟增加保留记录，不把小幅负差宣称为编译加速。此有限降低沿用 release 默认分配策略，无独立激进等级；动态绑定树、TLS 访问器及更广的 Context 复用未在本次展开。原始日志、逐次测量和机器码保存在两机仓库外 `scoop-performance-results/2026-10-26/context-access`，临时输入与构建位于忽略的 `tmp/program-performance/context-access`。
