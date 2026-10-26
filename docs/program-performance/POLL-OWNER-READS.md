# poll 的本线程原子读取

相对 `fe55cd2c2`，生成代码的 Full poll 保留全局 GC epoch、world phase 的 acquire 读取，将本线程 mode、observed epoch 改为 LLVM monotonic／C relaxed；EntryActivation 的 mode 同样使用 relaxed。所有 profile 使用同一规则。状态仍是原来的 16 B TLS 指针视图，没有新增参数、对象字段、GC roots 或 `.slib`／runtime ABI 版本。

## 同步依据与适用范围

核实 runtime 的 attach／detach、pending 激活、park、collection、native 转换、初始化等待及终止路径：只有所属线程写自己的 poll state。收集者读取其他线程的 mode／epoch，不代写这些字段。所属线程的生成代码不通过这两个字段取得其他线程发布的数据；读取遵守同线程先行写入和原子一致性。runtime 的 release／seq_cst 发布、对其他线程的 acquire／seq_cst 读取以及 NativeSafeReturning 握手均保持。

全局 epoch／phase 仍在每个 Full poll 位置读取。状态地址复用、pending 首次有锁激活、慢边、精确根与 relocation 保持。该规则按同步职责选择内存序，不依据函数或库的名字。相容旧库较强的 acquire 读取可以混用；重新编译其 callable 后才获得新指令。

本项不消除 TLS resolver，也不改变函数间的调用 ABI。未内联的普通 callable 仍可能在入口取一次 TLS；Darwin 的该成本需要单独处理。基准为隔离本项而固定同一套 core／runtime，所以其中预编译的 `Long.hash` 物理正文仍保留原四次 acquire；其已内联调用路径没有重新引入包装调用。

## 定向验证

- 两机格式化／workspace Clippy 通过。LLVM 定向测试覆盖三个目标的 Full／EntryActivation 共六种组合，精确检查共享 acquire、所属线程 monotonic、函数内单次 TLS 读取和只在慢边物化 roots，并完成 statepoint 改写。
- Darwin、GNU、musl 的 `poll-threads`、`poll-thread-roots`、`poll-codegen-roots` 均通过，共 18 个 variant、96 个进程；包括 debug／release、callback 线程、并发收集、等待循环、continue、aggregate／可空接口／closure roots、普通与 moving／minor GC、有界循环停止和恢复。
- 复用[普通成员基准](../../tests/benchmarks/inlining/members/README.md)，三目标两版、六模式、五种小输入共 180 个独立预期输出通过；18 组 GC 诊断的 16 项计数一致，均分配 136 B、未发生 collection。并发正确性由上述 GC 专测覆盖，普通吞吐不混入 stress 或 GC 诊断。
- 单项对照完成后，按依赖顺序重建并发布当前开发 sysroot 的全部系统库：Darwin 22 个、GNU／musl 各 21 个。三目标各补一次移动 GC 根存活冒烟，均通过；新 core 中 `Long.hash` 物理正文为 112 B／2 条 ldar，原来为 116 B／4 条。该维护重建同时使用已交付的普通内联额度，不计入下表隔离本项的单 Cone 编译成本。
- 检查 target 增量缓存，每 crate 保留最近两个 completed session，没有更旧 session 可删；未清除有效构建缓存或重跑全量验证。

## 指令、时间与成本

ARM64 每个 Full poll 由 `4 ldar` 变为 `2 ldar + 2 ldr`，同时省掉 observed 字段取址的 add。六个调用者各有入口／循环两处 Full poll，从 8 条 ldar 减为 4 条，各缩小 8 B。`Mixer.apply`、`Bias.apply` 的 EntryActivation 从 1 条 ldar 减为 0 条，仍读取 mode 并保留慢边及 TLS。

同机同 core／runtime，M3 Ultra／LLVM 22.1.8；NUC i7-12700H／LLVM 22.1.2，Linux 固定 CPU 4。每模式 2,000 万次，一组预热、7 组 AB／BA，内核墙钟中位数，单位 ms：

| 负载 | Darwin 前 → 后 | GNU 前 → 后 | musl 前 → 后 |
| --- | --- | --- | --- |
| struct 方法已内联 | 16.607 → 15.164（−8.69%） | 23.246 → 23.177（−0.29%） | 13.875 → 14.891（+7.32%） |
| class 方法已内联 | 14.924 → 14.870（−0.36%） | 10.459 → 10.679（+2.10%） | 9.606 → 9.597（−0.10%） |
| getter 已内联 | 15.295 → 15.287（−0.05%） | 10.421 → 10.385（−0.34%） | 9.528 → 9.512（−0.17%） |
| primitive hash | 59.596 → 60.783（+1.99%） | 58.491 → 58.461（−0.05%） | 55.358 → 55.353（−0.01%） |
| 未知接口调用 | 39.835 → 39.102（−1.84%） | 69.991 → 67.907（−2.98%） | 39.479 → 38.754（−1.84%） |
| 标量对照 | 14.885 → 14.916（+0.21%） | 10.444 → 10.451（+0.06%） | 9.002 → 9.023（+0.23%） |

不将 Linux 表中的波动归因为本项收益：两个 Linux 目标的六个计时内核及实际动态目标 `Mixer.apply`，地址和完整机器字节均相同。整个 `.text` 并非逐字节相同，其他成员有合法的读取顺序变化，但代码段大小不变。仅对三个回退样本追加 2 亿次、9 组交错测量：Darwin primitive 为 **619.456 → 614.644 ms（−0.78%）**，GNU class 为 **87.690 → 87.719 ms（+0.03%）**，musl struct 为 **175.222 → 177.294 ms（+1.18%）**；保留原样本与残余噪声，不重复全套测量或据此为函数添加特判。

| 成本 | Darwin | GNU | musl |
| --- | --- | --- | --- |
| provider／consumer `.slib` 增量 | −128／−76 B | −8／0 B | −8／0 B |
| 程序 `.text` 增量 | −152 B | 0 B | 0 B |
| 完整程序大小，两版相同 | 10,596,824 B | 12,411,080 B | 12,016,056 B |
| provider／consumer 编译墙钟变化 | −0.11%／−0.94% | −2.75%／−4.13% | −0.75%／+0.84% |
| provider／consumer 编译峰值 RSS 变化 | +1.27%／+0.39% | +0.27%／+0.19% | +0.25%／+0.23% |

Darwin stackmap 为 505,736 B、GNU metadata 为 2,319,008 B、musl metadata 为 2,318,576 B，前后均未增加。编译对照直接执行不请求 dump 的 `scoopc build`，排除最终链接，一组预热后取 5 组交错中位数；小幅墙钟变化不作为编译提速结论。该选择减少 ARM64 指令／排序开销，不增加执行步骤、元数据或独立策略成本，作为所有 profile 的默认代码生成规则，不新增开关。

原始数据、字节对比、复测和工具快照保存在两机忽略的 `tmp/program-performance/poll-order/`。
