# 小型 TLAB 分配完成内联

2026-10-24，在 `5fb20e6ac` 的 nursery 独占块登记之上，复用完整 LIR 分配布局并内联小型分配的完成步骤。两部分均作为 release 默认能力，条件见[简略清单](ENABLED-OPTIMIZATIONS.md)；没有新增公开开关或等级。

release 从完整本地 TypeDescriptor 或 typed array layout 取得已确定的 alignment 与 release policy，跨 LLVM 物理对象分区保留这些事实。请求大小为现有编译期常量、位于 16～128 B、布局完整且无 release hook 时，成功 bump 之后直接清零、设置对象头、登记精确大小、发布位图并更新四项原子计数。当前最大托管对齐为 16 B，对齐后仍至多 128 B；line mask 同字合并，跨字仅补一次登记。starts 在其他对象元数据之后作 release 发布，单写者 atomic load／OR／store 保留此前对象的位。

debug、动态／更大请求和缺少完整布局的外部／动态 TD 使用共享完成入口；缺少事实时保留 TD 加载。已知有 release hook 的分配消除不可能成功的年轻区分支，继续使用原旧代慢路径。失败 bump 保留 collecting slow path、poll、live set 与 relocation；没有省略清零、初始化、统计或 GC。内联复用本次取得的 allocation context，成功路径不再调用完成 helper 或再次读取 TLS，但每次分配最初的 TLS 访问仍存在。

首次试验只按请求大小展开，物理分区中的 TD 仍为外部引用，导致已知 alignment 未折叠、清零仍调用 helper，Darwin text 增长 13.7%。最终实现直接消费已有 LIR 布局，消除重复 shape／hook 读取及相关分支，将 Darwin text 增长降至 8.0%；不增加持久摘要或多套布局策略。

runtime ABI contract 从 15 升至 **16**，metadata ABI 仍为 9，spec 与兼容性向量同步。所有 profile 使用相同七指针 context：cursor、limit、block start、size units、starts、occupied lines、counters。三目标实测 context 从 16 增至 56 B，但利用 mutator 原有 padding，`sizeof(ScoopGcMutator)` 仍为 256 B，counter 大小／偏移仍为 32／128 B。对象、接口、TD 与精确 roots 布局不变；新增指针均为非托管元数据。旧 ABI 产物与 sysroot 需配套重建，本轮已用正式准备脚本完成。

## 定向验证

- 本机 fmt、workspace Clippy，以及 25 项 Rust／C 测试通过：runtime ABI 5、slib 兼容 8、分配 codegen 2、数组 codegen 8、runtime 元数据与统计各 1。Linux fmt、workspace Clippy、相同分配／数组及 C 测试共 12 项通过。
- Darwin、Linux GNU、Linux musl 各运行 allocation-accounting、allocation-threads、runtime-allocation 三组正式专测，每目标 6 个 profile 变体、22 次进程调用。覆盖 debug／release、正常与移动／minor stress、16／128／136 B、引用字段、装箱、数组、线程计数，以及连续分配跨 line、位图字和 TLAB refill。没有运行全量验收。
- 三目标基准的完整 MIR／LIR 逐字节相同，8 个内核的 poll／live 计划不变；9 种测量负载的 16 项 GC 诊断指标逐项相同。每侧另以 0、1、2、3、63、64、65、4097 次输入核对全部 8 个模式的独立数学结果。
- IR 验证发布次序、对齐、跨字合流和 fallback ABI；机器码确认 16／24／48／128 B 成功快路径无完成 helper 或 memset／bzero 调用，136 B 与数组对照仍调用共享入口。最终工具重建的完整程序与根 `.slib` 均与三目标测量候选逐字节相同。

## 运行结果

复用[真实短命分配基准](../../tests/benchmarks/allocation/nursery/README.md)。双方从相同源码构建，分别使用配套工具、runtime 和从当前源码重新编译的 core。M3 Ultra／LLVM 22.1.8；nuc12 i7-12700H／LLVM 22.1.2，Linux 固定 CPU 4；release 基础 CPU、一个 GC worker，一对预热后七对 AB／BA。计时区间包含分配、字段初始化、统计和收集，GC 诊断另跑。

| 负载与迭代次数 | Darwin 耗时变化 | GNU 耗时变化 | musl 耗时变化 |
| --- | ---: | ---: | ---: |
| 16 B 空对象，2000 万 | −56.28% | −55.26% | −55.12% |
| 24 B 对象，2000 万 | −59.25% | −52.04% | −49.40% |
| 48 B 对象，2000 万 | −56.43% | −41.93% | −46.28% |
| 128 B 对象，1000 万 | −12.64% | −9.52% | −37.63% |
| 536 B 数组，含初始化，100 万 | −5.49% | **+2.38%** | +0.43% |
| 136 B 对象，1000 万 | −1.11% | −3.53% | −0.93% |
| 带 release hook 的旧代对象，100 万 | −3.20% | −1.47% | −1.09% |
| 无循环内分配的整数对照，2000 万 | −0.60% | −0.13% | +0.01% |

24 B 阶段中位数分别为 155.646→63.433、145.698→69.872、148.379→75.081 ms；含启动的整进程墙钟分别降低 54.36%／49.99%／44.82%。GNU 数组另外延长至 500 万次，一对预热后九对 AB／BA，阶段 290.086→294.379 ms，仍回退 **1.48%**；保留首次与复测结果，不将此项记为收益。仅一次分配的整进程中位数约 14.8／6.1／15.1 ms，变化 −0.64%／−0.73%／+0.07%；极短阶段不用于推断稳定的单次成本。

## 空间与编译成本

| 项目 | Darwin | GNU | musl |
| --- | ---: | ---: | ---: |
| text | +58,100 B（+7.98%） | +41,408 B（+4.76%） | +41,408 B（+4.41%） |
| 完整程序文件 | +51,536 B（+0.41%） | +40,960 B（+0.33%） | +40,960 B（+0.34%） |
| 根 `.slib` | −212 B | −1,248 B | −1,252 B |
| core `.slib` | +23,318 B | −34,056 B | −33,934 B |
| 冷根 Cone 编译墙钟 | +2.12% | −0.61% | **+6.48%** |
| 编译 CPU | +1.67% | +0.17% | +1.82% |
| 编译峰值 RSS | −0.17% | +0.00% | +0.07% |

编译成本独立测量：保留预编译 core／runtime，只清根 Cone 产物，一对预热后五对 AB／BA，包含正常编译与链接，不代表整套 sysroot 重建成本。静态接口表和类型常量没有增长；stackmap 减少 384 B，GNU／musl 对应 metadata section 同样缩小 384 B，EH frame 变化 −8／+824／+824 B。运行 RSS 中位数 Darwin 增加 0～96 KiB，Linux 普通负载不变，旧代模式降低 96／68 KiB；不把小幅波动解释为内存优化。

当前选择以 128 B 上限和完整布局条件控制展开成本。16／24／48 B 的分配收益跨三目标稳定，支持受限的 release 默认策略；代码段增长、GNU 数组回退和 musl 编译墙钟增长均保留为实际成本。其后已交付 ARM64 的[分配 context 首次使用复用](ALLOCATION-CONTEXT.md)；更大或动态分配、清零与构造合并、已清零 TLAB 及批量登记继续独立设计和测量。这是 Scoop 自身 A/B 结果，不构成对 Go／Java／Kotlin Native 综合性能的结论。原始日志、逐次结果、IR 与机器码保留在两机忽略的 `tmp/program-performance/tlab-inline/`。
