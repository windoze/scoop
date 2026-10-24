# 按首次使用复用分配 context

2026-10-25，在 `6edfdeafd` 的小型 TLAB 完成内联之上，减少生成代码对 allocation context 的重复 TLS 取址。最终仅在 **ARM64 release 默认启用**，x86 和 debug 保留原路径；没有新增公开选项或等级，条件已列入[简略清单](ENABLED-OPTIMIZATIONS.md)。

context 地址与 attachment 在一次 managed 调用期间稳定，GC、refill、native 转换和同步 callback 不替换 mutator；真实挂起返回后的下次机器函数调用重新取得地址。codegen 用入口初始化为空的非逃逸 AS0 局部槽表达首次使用，各分配点仅在缓存为空时读取 TLS，交由 LLVM 提升为 SSA。正常分支、catch 与循环都复用同一个调用内缓存。cursor、limit、块元数据和计数内容仍读取当前状态，不把已经失效的 TLAB 范围跨 GC 保存。

首次 TLS load 使用 volatile 保留执行位置，避免 LLVM 把条件读取改成无条件读取加 select；它不是同步操作。最初的普通 load 试验在实际机器码中仍每轮调用 TLS 取址入口，未采纳。已知 release hook、超出普通对象上限或溢出的常量条件直接选择原 collecting slow path，不建立无用的 context 缓存。未执行分配的路径不调用 context 的 TLS 取址入口，仍可能承担少量局部状态与寄存器准备。

小型内联完成的成功快路在缓存命中后不再取址；更大／动态分配使用的完成 helper 内部 TLS 访问，以及 collecting slow path 内的 runtime 工作仍保留。poll 的原子读取、GC roots、relocation、清零、登记顺序和统计保持。runtime ABI 仍为 **16**、metadata ABI 仍为 **9**，不增加对象字段、runtime TLS 或 context 布局，不增加托管引用存活范围或根。

## 定向验证

- 两机 fmt／workspace Clippy 通过；最终各运行 10 项 codegen 测试，覆盖分配发布、debug／未知／大对象退路、首次使用的条件路径和数组。ABI、C runtime 与持久 IR 格式没有变化。
- 最终三目标各运行 allocation-accounting、allocation-threads、runtime-allocation 三组正式专测，每目标 6 个 profile 变体、22 次进程调用。debug／release、正常／移动／minor stress、线程 callback 与 attachment 重建均通过；新增共享数组保留真实分配的 catch 用例，覆盖先分配再抛出、catch 后分配、GC 后继续分配，以及跳过 try 内分配的路径。没有运行全量验收。
- 三目标策略试验的完整 MIR／LIR 逐字节相同，原 poll／live 计划保持，10 种测量负载的 16 项 GC 指标逐项相同；各侧对 8 个模式、8 组零／短输入另核对独立数学结果。IR 与机器码确认缓存命中跳过 TLS 调用，保持可变字段重读和原发布顺序。
- 最终 ARM64 程序、根 `.slib`、core 与测量候选逐字节相同；最终 GNU／musl 三类产物均与 `6edfdeafd` 基线逐字节相同，未把 x86 试验推入默认能力。

## ARM64 运行结果

使用[真实短命分配基准](../../tests/benchmarks/allocation/nursery/README.md)，双方配套工具、core 与相同 runtime，M3 Ultra／LLVM 22.1.8、release 基础 CPU、一个 GC worker。一对预热后七对 AB／BA，GC 诊断另跑。下列阶段包含真实分配、初始化、登记、统计与收集。

| 负载与次数 | 基线→采纳版本（ms） | 耗时变化 |
| --- | ---: | ---: |
| 16 B 空对象，2000 万 | 57.197→47.762 | −16.50% |
| 24 B 对象，2000 万 | 62.137→52.430 | −15.62% |
| 48 B 对象，2000 万 | 80.041→75.569 | −5.59% |
| 128 B 对象，1000 万 | 68.352→66.139 | −3.24% |
| 536 B 数组含初始化，100 万 | 101.901→102.106 | +0.20% |
| 136 B 对象，1000 万 | 92.251→90.502 | −1.90% |
| 带 release hook 的旧代对象，100 万 | 33.036→33.202 | +0.50% |
| 无循环内分配的整数对照，2000 万 | 6.713→6.777 | +0.95% |

16／24 B 的整进程墙钟分别降低 12.82%／12.10%。零次分配的进程中位数 16.111→16.592 ms（+2.99%），仅一次分配为 16.244→16.246 ms（+0.01%）；二者阶段均约 0.003 ms，启动占主导，不用于推断稳定的单次分配成本。运行 RSS 中位数变化为 0～+80 KiB，保留这些对照成本，不宣称所有负载加速。

GNU／musl 另在 nuc12 i7-12700H／LLVM 22.1.2、CPU 4 上按相同协议试验。16／24 B 仅改善约 2%～3%，48 B 分别回退 4.65%／4.55%，128 B 回退 4.24%／4.27%；数组变化 −1.39%／+0.13%，136 B 变化 +1.63%／−7.88%。这些结果不足以支持 x86 默认增加缓存判断与更长的寄存器存活，最终保留原取址方式；未新增供用户启用该试验的开关。

## 最终空间与编译成本

| 项目 | ARM64 Darwin | GNU | musl |
| --- | ---: | ---: | ---: |
| text | −1,516 B（−0.19%） | 相同字节 | 相同字节 |
| 完整程序文件 | 大小不变 | 相同字节 | 相同字节 |
| 根 `.slib` | +112 B | 相同字节 | 相同字节 |
| core `.slib` | −1,704 B | 相同字节 | 相同字节 |
| 冷根 Cone 编译墙钟 | +0.96% | +0.41% | −2.29% |
| 编译 CPU | +2.09% | +0.27% | −2.12% |
| 编译峰值 RSS | −1.06% | 约不变 | 约不变 |

编译成本以最终工具独立测量，保留预编译资源，只清根 Cone 产物；一对预热后五对 AB／BA，Linux 固定 CPU 4，包含编译与链接。ARM64 静态类型／接口表和 stackmap 大小不变，EH frame 缩小 8 B。小幅编译和对照波动不作为额外优化收益。后续已交付受限的[初始化尾部清零消除](ALLOCATION-ZEROING.md)；更广的线程视图共享、减少 helper 内部 TLS、其他清零合并与批量登记仍需独立测量。本项不构成跨语言综合性能结论，原始日志与试验产物保留在两机忽略的 `tmp/program-performance/allocation-context/`。
