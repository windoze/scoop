# 接口值的直线区间复用

本项以 `43a0bfb9c` 为基线，接续接口父表投影与引用循环优化。最终只在 x86_64 release 默认复用刚写入 canonical storage 的完整接口 SSA 快照，没有新增公开开关或等级。ARM64 的相同试验发生回退，最终产物与基线相同。

## 设计与边界

真实接口数组复制在 LIR 中经过多个内联局部值。此前每次读取局部接口都从 canonical storage 用 volatile object load 重建 `{ object, itab }`，即使两次读取之间没有 safepoint。复制链既保留了多组存栈／重读，也使 x86 的有界内层不能使用已有页表查询提升。

先修订实现规范 2.5 和运行时规范 2.2 的接口扫描合同，再在 FnEmitter 中保存刚写入的完整 SSA 值。按 canonical storage 指针索引，仅供同一实际 LLVM 基本块的直线区间使用，包含接口 niche enum。codegen 仍生成原存储写入；局部重赋替换该槽的快照，先前复制的值保持独立，不能把快照当成源局部的别名。

调用（包括 NoGc）、poll、分配、取址、raw memory、atomic、EH、不明确操作以及 LIR／实际 LLVM 块边界结束复用。无快照时继续原 volatile object 重读，跨 safepoint 的精确 leaf 回写和 object／itab 重建保持。不增加逻辑 live set、GC root、对象字段、接口表示或 `.slib` 字段，各 profile 使用同一布局和 ABI。

这项改动减少接口值搬运，不改变接口查找语义。已有 Child→Parent 仍按完整内嵌父表的固定偏移投影，继承调用仍直接取 Child 槽；Any 首次取得视图、无静态父关系转换等继续按既有规则查找。复制链消失后，x86 接口复制／父投影自然满足原有无 volatile 区域条件，四级 GC 页表查询移到每 64 元素块的入口，逐次 card store 保留。没有放宽 x86 的 volatile 区域规则，也没有采纳额外的普通根加载提升。

## 定向验证

- 两机均通过最终 16 项后端定向测试、格式化与 workspace Clippy。覆盖 debug／release、三个目标、旧副本的两个分量、重赋、各失效边界、canonical temp、call／invoke／恢复、LLVM safepoint verifier 与物理对象发射。
- Darwin ARM64、Linux GNU／musl 各通过 [reference-array-roots](../../tests/fixtures/gc-correctness/poll/reference-arrays/fixture.toml) 与 [conditional-interface-roots](../../tests/fixtures/gc-correctness/conditional-interfaces/fixture.toml)，每目标两变体、10 次成功进程调用。覆盖可空接口、泛型非零父偏移、装箱、default／override、smartcast、协程恢复，以及跨 64 KiB 旧数组持有年轻值的 moving／minor 组合。没有全量验收或重建未变化的系统库。
- 三目标 16 种模式输出相同，整个 LIR、235 条 TD／接口表／布局记录、16 函数的 poll／live 集合保持；22 种负载的 15 项分配／GC／脏卡／扫描计数一致。最终 ARM64 程序和根 `.slib` 等于基线，GNU／musl 等于已测快照候选，以产物一致性复用原计时与 GC 样本。

GNU／musl 完整块的接口复制内层从 38 条指令降到 10 条，父接口投影从 40 条降到 13 条；四组局部接口复制重读消失，内层不再查询四级页表。短尾和其他不合格写入仍执行原查询，引用 SIMD 仍未开放。接口初始化的合流后重读按边界保留。

## 运行结果与未采纳试验

双方编译相同[引用数组基准](../../tests/benchmarks/arrays/references)，release、同一预编译 core 与 runtime。M3 Ultra／LLVM 22.1.8 和 nuc12 i7-12700H／LLVM 22.1.2；Linux 运行固定 CPU 4。长数组为 65,536 元素、128 轮，短模式和首项命中为 10 万轮。首轮一对预热、五对 AB／BA；热点复测两对预热、九对 AB／BA。下表为内核墙钟中位数变化，负数表示降低。

| 操作 | GNU 首轮／九对复测 | musl 首轮／九对复测 |
| --- | ---: | ---: |
| 泛型 interface 复制 | −35.75%／−37.65% | −37.13%／−37.07% |
| Child 数组投影到 Parent | −38.59%／−38.80% | −39.78%／−40.72% |
| interface 分配与初始化 | −1.39%／−1.23% | −0.22%／−1.51% |

复测的复制／投影整进程耗时分别降低 GNU 17.94%／18.98%、musl 12.56%／14.39%。六类单元素模式中，最大绝对回退为 GNU 求和约 0.212 ns／次；接口复制和父投影的短模式均降低。

GNU 末项身份搜索首轮 +2.62%、复测 +7.03%（整进程 +2.69%），标量求和 +4.32%／+1.14%。实际内联搜索的完整块与短尾各六条指令、32 字节对齐均不变，代码位置变化；样本波动与布局影响尚未拆清，保留该回退，不把其他对照上的负差归为本项收益。

ARM64 快照候选虽将复制／父投影从 20／22 条指令减至 12／13 条，首轮却回退 39.18%／35.19%，九对复测仍为 +40.02%／+34.51%；初始化降低约 4%～5%。另试验用既有不逃逸根槽与有界区域分析将普通根加载独立移到入口，机器码确认读取已移出，但接口回退未解决，普通引用复制又出现明显回退，x86 没有稳定新增收益。因此两种 ARM64 试验和额外根加载扩展均未采纳，实验源码、产物与数据保留在忽略目录，未推送。

## 成本

| 最终变化 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| text | 0 B | −448 B | −448 B |
| 完整程序文件 | 0 B | 0 B | 0 B |
| 根 Cone `.slib` | 0 B | −472 B | −472 B |
| 编译墙钟 | −0.85% | −0.44% | −0.36% |
| 编译 CPU | −0.87% | −0.50% | −0.31% |
| 编译峰值 RSS | −0.81% | +0.18% | +0.21% |

编译测量仅构建根 Cone、消费同一预编译 core，关闭 dump、独立输出，一对预热后五对 AB／BA，不含最终链接。GNU 首组未固定 CPU 的样本在约 2.0～5.1 秒间波动；最终 Linux 编译对照均固定 CPU 4，原始数据保留。没有测得编译时间增长，也不把这些小幅负差解释为编译加速。Darwin 完成样本后，临时汇总脚本对两个不同产物 hash 的旧断言失败；改为该目标应有一个 hash，并从已保存的 12 个样本生成汇总，没有重复编译。

原始日志、样本与产物位于两机忽略的 `tmp/program-performance/interface-values`：`final` 为全目标快照试验，`root-loads` 为额外根加载试验，`accepted` 为最终平台策略，Linux 编译结论使用 `pinned-compilation`。已同步[开启条件简略清单](ENABLED-OPTIMIZATIONS.md)。
