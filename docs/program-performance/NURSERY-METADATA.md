# nursery 独占块的分配登记

本项以 `9b9748e42` 为基线，接续分配消除后的真实分配热点。对显式发布短命对象的程序进行 CPU 采样，发现通用小对象登记、重复 block 基址调用和位图原子读改写仍占用较多时间；本项只优化已保留范围的年轻块登记。

普通 nursery block 由一个 mutator 独占写入，moving stress 的独立年轻块也只有当前分配线程初始化；取得空间到完成登记之间没有 safepoint。先修订运行时规范 3.1，再让该路径复用已确认的大小、对齐、空闲与块种类，用 relaxed atomic load／OR／store 更新 occupied-line，最后以 release store 发布 starts。每次更新保留同一字中已经发布的其他位，查询方仍以 acquire 读取 starts；晋升、退役和清零遵守原 STW／TLAB 失效协议。

共享旧代块可能由多个 mutator 在保留空间后并发初始化，继续使用原子读改写和原检查；collector 的登记路径保持。两类调用者共用一个私有元数据写入实现，并内联原有 block 基址访问器。分配完成入口、TLS 读取、全部对象字节清零、对象头、精确大小与分配计数仍然存在，没有新增分配区、批量发布或布局选择框架。

对象、block metadata、TLAB、roots、TypeDescriptor 和 `.slib`／ABI 格式均未改变。编译器没有修改，旧 Cone 产物可以重用并链接新版 runtime；本次普通／stress sysroot 只刷新 runtime，未重建系统 Cone。

## 定向验证

- 本机与 Linux 各通过四项 C runtime 专测，覆盖连续 TLAB 分配共用位图字、大小与对齐边界、line／card、统计查询与线程退出、共享旧代分配、位图 sweep、pin／release，以及装箱、数组、ZST 和移动回写。新增用例连续发布 64 个对象后逐个核对大小、对象头、清零和此前 starts 位的保留；格式化与 workspace Clippy 通过。
- Darwin ARM64、Linux GNU／musl 各通过 [allocation-accounting](../../tests/fixtures/gc-correctness/allocation/accounting/fixture.toml) 与 [allocation-threads](../../tests/fixtures/gc-correctness/allocation/threads/fixture.toml)，每目标四个变体、20 次进程调用，覆盖 debug／release 和普通／moving／minor。没有全量验收。
- 三目标基准的 MIR、LIR 与根 Cone `.slib` 均逐字节相同；小对象、宽对象和旧代对照保留实际分配。每侧五种模式的八组短输入通过独立结果核对；六种计时负载的输出及 16 项分配／GC 计数一致，包括分配字节、minor／full、晋升、复制、roots、脏卡和扫描量。
- 实际完成入口进入年轻块分支后，不再调用通用 `record_small_object` 或 block 基址函数；ARM64 使用普通 load／store 和 starts 的 `stlr`，x86 使用寄存器 OR／位设置后存回，没有位图 `lock` 操作。共享旧代／collector 的通用登记仍保留 ARM64 `ldset` 与 x86 `lock`。检查覆盖实际调用路径及分支。

## 运行与空间成本

双方用同一编译器、相同[分配基准](../../tests/benchmarks/allocation/nursery/README.md)和预编译 core，分别链接旧／新 runtime，release、基础目标、`SCOOP_GC_WORKERS=1`。每模式一对预热、七对 AB／BA，GC 诊断另跑；Linux 固定 CPU 4。环境为 M3 Ultra／LLVM 22.1.8 和 nuc12 i7-12700H／LLVM 22.1.2。下表是计时区间墙钟中位数变化，负数表示耗时下降。

| 负载 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| 24 字节对象，2000 万次 | −33.50% | −53.45% | −51.11% |
| 48 字节对象，2000 万次 | −28.16% | −51.24% | −48.10% |
| 64 个 Long 的数组，100 万次，含初始化 | −3.64% | −29.66% | −23.73% |
| 带 release hook 的旧代对象，100 万次 | −4.21% | −4.44% | +0.30% |
| 无分配整数循环，2000 万次 | +0.16% | −0.17% | +0.04% |

24 字节分配区间由 225.13／302.45／304.57 ms 降至 149.71／140.79／148.90 ms，整进程分别下降 31.25%／52.42%／48.73%。48 字节的整进程下降 26.63%／50.28%／46.07%，数组下降 3.67%／27.70%／20.31%。musl 旧代对照的整进程 +0.31% 保留，不归为已解决的回退。单次分配的区间分别为 2000→2000、1619→1439、854→811 ns，整进程变化 −0.24%～−0.02%；这类短区间不外推为稳定的单对象成本。

小／宽／数组负载的分配量仍约为 480／960／536 MB，minor 仍为 459／919／520 次；旧代负载约 24 MB、一次 full，统计和输出没有因优化减少。所有计时负载的峰值 RSS 中位数变化在 −68 KiB～+16 KiB。

| 成本变化 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| 程序 text | +716 B | +192 B | +256 B |
| 完整程序文件 | −16 B | −56 B | −24 B |
| 根 Cone `.slib` | 0 B，内容相同 | 0 B，内容相同 | 0 B，内容相同 |
| runtime 冷构建墙钟 | +0.46% | −0.72% | −1.20% |
| runtime 冷构建 CPU | +0.34% | −0.40% | −0.64% |
| runtime 冷构建峰值 RSS | −0.04% | +0.02% | −0.21% |

冷构建单独调用既有 runtime builder，含正常工具探测、依赖预处理和 C 编译，预热一对后五对 AB／BA；三目标基线墙钟中位数约 4.42／4.04／4.15 s。没有修改 Scoop 编译 pipeline，没有增加对象或 block 元数据字段、GC roots。原始日志、采样、机器码和逐次报告保留于两机忽略的 `tmp/program-performance/tlab-allocation`。

本项作为所有 profile 使用的新版 runtime 默认实现，无独立开关或等级，已进入[简略清单](ENABLED-OPTIMIZATIONS.md)。完整分配完成内联、清零与构造合并、已清零 TLAB 和批量登记继续分别测量；本项的单写者证明不适用于共享块或新增的并发写入者。
