# 接口引用循环的页表查询提升

本项接续[有界引用循环查询提升](CARD-REGIONS.md)，以 `4238a5309` 为基线。最终仅在 ARM64 release 默认允许独立的 GC mapping 查询跨过保留在原处的接口栈重取；x86 试验未取得稳定收益，继续采用原区域选择规则。没有新增公开开关或等级。

## 设计与边界

接口值从 canonical storage 重建时，object 使用 volatile load，避免 LLVM 将移动 GC 前的 aggregate SSA 重新转发回来。此前这类操作会排除整个区域，接口复制、父接口投影和接口初始化因而仍在每个元素写入后查询四级页表。

先修订实现规范 2.19 和运行时规范 3.6，再扩展已有区域选择：只接受非原子的 AS1 引用重读，其地址必须来自当前函数入口的 alloca，可经常量 GEP 或仅在这些栈地址间选择的 select／phi。重读留在原位置，保持次数与相互顺序；待提升查询的输入仍由原有稳定根槽分析约束，不能依赖 volatile 的结果。含堆、全局、参数、不明确地址、动态 GEP、volatile store、用户原子或调用的情况保留原操作。

其余条件沿用原方案：实际 CFG 是单入口、单 latch、无提前退出的 1～64 次内层，查询每轮必经且目标不变。只移动独立的 acquire 页表／immutable prefix 加载及必要地址运算，发生在本块 poll／恢复之后，不跨下一次 safepoint。原 card store、精确 roots 与 interface object／itab 重建保持。

这里减少的是写屏障的 GC 页表查询。既有 Child→Parent 投影仍使用固定 itab 偏移，继承方法仍直接取槽，没有重新引入接口目录查找；短尾和不合格写入继续查询 mapping。接口值、对象、静态表、`.slib` 字段与各 profile 的布局／调用 ABI 不变。

## 定向验证

- 两机均通过最终 12 项后端定向测试，覆盖原有有界整数／严格浮点循环、栈重取来源的正反例、三个目标的 LLVM safepoint 验证与物理站点／根数量，以及 interface call／invoke／恢复组合。x86 用例明确断言含 volatile 的区域不提升查询；ARM64 保留两次栈重取的顺序和位置。格式化与 workspace Clippy 通过。
- Darwin ARM64、Linux GNU／musl 各通过 [reference-array-roots](../../tests/fixtures/gc-correctness/poll/reference-arrays/fixture.toml) 与 [conditional-interface-roots](../../tests/fixtures/gc-correctness/conditional-interfaces/fixture.toml)，每目标两变体、10 次成功进程调用。覆盖跨 64 KiB 的旧数组持有年轻引用、泛型非零父表偏移、装箱、default／override、smartcast、协程恢复和 moving／minor。最终收窄平台策略后，Linux 额外定向确认 conditional-interface-roots。
- 组合验证发现基线已有的倒计数识别崩溃：在确认指令为 add／sub 前，读取了 `gc.result.i32` 的 token 操作数。基线和候选均可复现；最小 LLVM 回归先失败，再通过先检查 opcode 修复。正常整数调用结果和非减一回边保持原 IR，不新增证明或重复语义检查。
- 三目标基准 LIR 逐字节相同，235 条 TD／接口表／布局记录、16 个函数的 poll／live 集合及 22 种负载的 15 项分配／GC／脏卡／扫描计数保持。最终 ARM64 程序、根 `.slib` 与 LIR 等同已测候选，最终 GNU／musl 等同基线；16 种模式输出均一致。

ARM64 完整块中的接口复制、父接口数组投影、接口初始化，内层分别从 36／39／36 条指令降至 20／22／18 条。四级查询由每 64 元素执行 64 次变为一次，内层不再含这组 acquire load；栈重取与逐次 card store 保留，短尾仍有查询。引用循环仍未取得 SIMD 资格。

## 运行结果与平台选择

双方编译相同[引用数组基准](../../tests/benchmarks/arrays/references)，release、相同预编译 core 与 runtime。M3 Ultra／LLVM 22.1.8 和 nuc12 i7-12700H／LLVM 22.1.2；Linux 固定 CPU 4。每模式一对预热、五对 AB／BA，GC 统计另跑；长数组 65,536 元素、128 轮，短模式与首项命中 10 万轮。下表是内核墙钟中位数变化，负数表示降低。

| 操作 | ARM64 最终采用 | GNU 试验／九对复测 | musl 试验／九对复测 |
| --- | ---: | ---: | ---: |
| 泛型 interface 复制 | −44.88% | −13.00%／−9.12% | +8.22%／+6.05% |
| Child 数组投影到 Parent | −43.65% | +6.03%／+2.21% | −0.27%／+1.01% |
| interface 分配与初始化 | −8.07% | +0.42%／+0.18% | −0.56%／+0.31% |

ARM64 三项整进程耗时分别降低 20.49%、18.64%、6.72%。x86 试验虽然把复制／投影内层从 38／40 条指令降到 20／22 条，实际运行没有稳定的共同收益，因此没有采纳该扩展。最终 x86 基准程序和根 `.slib` 恢复为基线的相同字节；保留试验数据，不把减少指令等同于性能收益。

ARM64 未命中的 MaybeUninit 清零首轮 +11.07%，两对预热、九对复测后为 +0.48%；String 复制为 −11.91%／−2.27%；class 复制为 +2.61%／+2.37%。三者实际被调用函数的指令序列归一化后相同，样本范围重叠，地址布局与运行波动尚未拆清；不把这些变化归为本项优化收益。其他长对照为 −0.95%～+1.14%，六类单元素模式的最大绝对回退是 class 初始化的 0.03 ns／次，首项命中没有回退。

GNU 试验的末项身份搜索首轮 +13.32%、复测 +4.35%；短投影 +4.55%／−3.52%。musl 短清零 +6.59%／+1.86%。这些随未采纳的 x86 试验一并保留，不进入最终默认产物的收益结论。

## 成本与记录

| 最终成本变化 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| text | +12 B | 0 B | 0 B |
| 完整程序文件 | 0 B | 0 B | 0 B |
| 根 Cone `.slib` | +16 B | 0 B | 0 B |
| 编译墙钟 | +1.92% | +0.17% | +0.66% |
| 编译 CPU | +1.57% | +0.74% | +0.66% |
| 编译峰值 RSS | +0.15% | +0.18% | +0.20% |

编译测量使用最终工具，只编译根 Cone、消费相同预编译 core，关闭 dump、独立输出，一对预热后五对 AB／BA，不含最终链接。GNU／musl 的程序、根 `.slib` 与 LIR 和基线完全相同。没有新增静态接口表、对象字段、GC root 或 runtime 资源。

运行数据、编译样本、产物、基线崩溃及回归日志保存在两机忽略的 `tmp/program-performance/interface-card-regions`；`final` 保留全目标试验，`accepted` 保存最终平台策略产物。没有全量验收，也没有重建未变化的系统库或 runtime。已同步[优化开启条件清单](ENABLED-OPTIMIZATIONS.md)。
