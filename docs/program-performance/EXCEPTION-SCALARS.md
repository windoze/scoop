# 异常区域中的局部分配消除

本项继续 B1 抽象消除，基线为 `fafcdf8e1`。[调用点异常路径特化](EXCEPTION-PATHS.md)已经能展开纯 callback 的 `use`，但调用方只要存在 catch／finally，原分配分析便跳过整个函数。真实带 catch 循环因而仍每轮创建一个闭包，并通过资源对象访问字段。

## 实现与开启条件

- **release 默认启用**，沿用已有局部分配消除及共同内联估价，没有新增公开开关或等级。debug 保留原分配；所有 profile 的对象、接口、异常、GC 和跨 Cone／`.slib` ABI 一致。
- 复用 box／closure／无 release hook 的 declared class 来源、使用和循环分析。仅在分析图中，为有隐式 unwind 的块增加正常完成点；异常边从原入口出发，局部定义只在正常完成点杀死旧别名的存活。跨块使用必须由创建块的正常完成点支配，同块仍按语句顺序判断；显式 Throw／Rethrow 保持位于语句之后。共享的块入口支配查询保持原语义。
- 实际 MIR 块、调用、throw、catch／finally／resume 和 poll 不移动。字段写入在原位置变成 typed local 赋值，读取保持独立快照；异常发生在更新前后时，处理器分别观察旧值和新值。box payload 与捕获仍在原创建点按原顺序复制。
- 正常和异常出口沿已有 LIR 精确 roots 保存、移动回写。被消除的对象不成为额外根，未初始化字段不伪造 null，不增加栈对象、native 异常槽或持久效果摘要。优化报告复用同一分析的实际保留原因，停止整函数的 `exception-flow` 拒绝分类。

创建点不能支配异常使用、别名跨创建迭代存活、外来／多来源合流、身份／地址观察、release hook 和未知引用使用继续保留。块内异常边按入口保守处理，不另外搭建逐语句异常效果分析；一般 EH 内联和跨调用部分物化仍在后续范围。规范见[实现规范 2.19](../specs/SCOOP-IMPL-SPEC.md)，实现见 [allocation/control.rs](../../compiler/mir-lower/src/optimization/allocation/control.rs)。

## 定向验证

- Darwin 与 Linux 各通过 66 项 MIR 优化测试；新增正常／异常字段替换、创建前 unwind 的支配拒绝，以及后续赋值不能遮蔽上一轮别名的回归。修改后先执行 fmt／workspace Clippy，Python 使用 Ruff 0.16.10；旧 EH 一律保留的断言已按新合同调整。
- 三目标各通过两组专测、3 个 variants：`exception-path-inlining` 覆盖相反 profile 的 provider／consumer、真实回调失败与未知调用保留、清理失败、已知成功的 use 在 catch 内消除；`local-class-scalar-roots` 覆盖字段更新前后抛出、聚合／接口引用、构造中途 GC、嵌套 finally／rethrow 与返回快照。每目标另通过 core consumer Cone 的三项资源行为测试。没有全量验收或预编译 sysroot 重建。
- roots 专测分别运行 moving 和 minor 模式。真正的被调用函数先 GC 再抛出，调用方 LIR 的异常根包含完整 Payload 扫描偏移 `0,8,16,24`，itab 不作根；旧快照与当前字段分别保留。Holder、Constructed 与 box 的分配实际消失，其残留 local 声明没有进入根集合。finally 中返回快照与新字段值保持两份 live root。本机独立诊断确认 428 次 full collection、70536 B 实际复制。
- 基准四种写法在 1、2、3、4、7、64、65、1000000 次均与独立计算结果一致，fixture 另覆盖零次。两种带 catch 的新内核均无 class／closure 分配及普通调用，仍有 3 个 landing pad、2 个 LIR poll；guarded use 机器指令从 205／207／207 条降至 109／111／111 条。三目标的新 use 与直接写法在去除符号／地址重定位差异后指令相同。

## 运行与成本

使用同一份[资源基准](../../tests/benchmarks/abstractions/resources/README.md)、预编译 sysroot 与 runtime。M3 Ultra／LLVM 22.1.8，以及 nuc12 i7-12700H／LLVM 22.1.2；Linux 固定 CPU 4。每种模式预热一对，再执行七对 AB／BA，GC 统计独立运行。长阶段执行 20000000 轮，在中间一轮抛出预建异常；阶段包含资源创建、循环与最后读取，异常对象创建和输出位于阶段之外。

| 目标 | 带 catch 的 use，基线 → 新实现 | 耗时变化 | 带 catch 的直接写法，基线 → 新实现 |
| --- | --- | --- | --- |
| ARM64 Darwin | 228.145 → 16.794 ms | −92.64% | 34.682 → 16.752 ms |
| x86_64 GNU | 296.192 → 12.868 ms | −95.66% | 14.326 → 11.454 ms |
| x86_64 musl | 308.736 → 11.634 ms | −96.23% | 14.478 → 13.058 ms |

三目标的 guarded use 分配由 640000152 B 降到固定 128 B，minor collection 从 612 次降为零，扫描 root slots 从 23868 降为零。guarded direct 从固定 152 B 降为 128 B。普通无异常 use／direct 仍各分配 104 B，长阶段变化 −0.34%～+0.10%。整进程 guarded use 的基线／新实现耗时比为 7.58／15.87／11.57；本机峰值 RSS 中位数约 10.70→9.28 MiB，Linux 观测值保持不变。

保留 Linux 两种新内核的实际时差：尽管指令除重定位外相同，阶段仍落在 11.45～13.06 ms，不能据此给源码抽象添加固定成本。单轮 guarded use 为 13000→13000／9829→9787／17340→16455 ns；普通 musl direct 短样本为 1476→1643 ns（+167 ns），不把短区间百分比解释为稳定收益或回退。

| 增量成本 | ARM64 Darwin | x86_64 GNU | x86_64 musl |
| --- | ---: | ---: | ---: |
| text | −880 B | −960 B | −960 B |
| 根 `.slib` | −32532 B | −39196 B | −39342 B |
| 完整程序 | −25744 B | −12408 B | −8312 B |
| 资源基准编译墙钟 | +0.30% | −2.17% | −1.37% |
| 引用数组对照编译墙钟 | +0.36% | +1.47% | +0.30% |

编译成本为直接 `scoopc build` 的六对交错样本，首对预热排除。编译峰值 RSS 变化 −1.25%～−0.11%；每版重复产物一致，引用数组对照在两版之间逐字节相同。基准 LIR 的 TD／布局记录由 53 项减为 49 项，减少已不需要的闭包物理支持；语义实体与公开布局契约保持。

该有限范围的收益与成本支持 release 默认策略，已加入[简略开启条件清单](ENABLED-OPTIMIZATIONS.md)。结果是同一 Scoop 程序的前后对比；跨语言综合目标继续沿主线推进。清理了本机 94 份过时的已完成增量编译会话，保留每个 crate 的最新与正在工作数据。

原始测量、IR／机器码、构建与失败闭合日志位于两机 `tmp/program-performance/exception-scalars`，不提交原始归档。
