# M58：短暂停 GC、局部分配消除与循环 poll 分块

状态：实施中，分项进度与验证见 [PROGRESS.md](PROGRESS.md)。2026-10-19 从远端 `main` 的 `c3b8942db` 建立 `codex/m58`；当前在主仓库直接实施，M56 的完成范围与验收记录保持原样。

## 1. 目标与范围

Scoop 主要面向 PC／桌面应用，包括 60 fps GUI。本期优先改善分配速度、GC 暂停及响应尾部，同时控制低配置电脑上的 footprint。16.67 ms 是整帧时间，不能全部留给 GC；吞吐提升也不能抵消明显更长的停顿或更高的常驻内存。

本期承接四项调研，落实为四组可独立验证的改动：

| 分组 | 要解决的问题 | 最小交付 |
| --- | --- | --- |
| M58-A：minor／full 暂停工作量 | minor 做与年轻存活量无关的扫描；full 在压缩失败前做昂贵准备，sweep 逐 word 扫描 | 年轻 small block 整块退役、无引用大对象跳过 remembered scan；full 必要容量早判与 starts bitmap 枚举 |
| M58-B：函数内逃逸分析与局部分配消除 | exact 类型已知的 box 仍分配；高阶函数内联后 closure 环境仍存在 | 统一迭代内联、去虚拟化、来源／逃逸分析、box 与 closure 替换，清理无用闭包支持产物 |
| M58-C：尾部与同步协程包装消除 | 真正尾调用仍生成自身状态；普通内联发生在 frame／adapter 生成之后 | 同结果类型的尾部 completion 转交；在 suspend ABI／状态机转换前内联小型已知同步正文 |
| M58-D：带外层 poll 的循环分块 | 每轮 poll 限制轻量循环吞吐与向量化 | 简单有界计数循环分块，外层 poll；先保守标量，再验证受限内层向量化 |

详细依据分别见 [GC 暂停工作量](GC-PAUSE-WORK.md)、[逃逸分析](ESCAPE-ANALYSIS.md)、[装箱与协程包装](ALLOCATION-ELISION.md)、[循环分块](POLL-STRIP-MINING.md)。M58-B 先以 box 打通分析与替换，再闭合 forEach 等内联后的 closure 消除，两者属于首批交付，不先搭建没有消费者的通用分析平台。

本期不以全程序逃逸分析、普通对象栈化、部分逃逸重物化、continuation 池或并发 collector 为前置。简单 class 标量替换是同一分析的后续实际消费者，按收益证据扩展，不要求一次覆盖所有局部分配。

## 2. 基线与已有证据

本轮实验基于 `04d28e763`，并在 `c3b8942db` 上复核相关源码：M55 未修改本轮涉及的 GC、MIR 普通优化、coroutine lowering、LIR safepoint 和 LLVM poll 策略实现。测量仍标注原基线，不视为最新 main 的重新计时；M55 的逐 callable 声明裁剪已有独立验收，不纳入 M58 的收益。

| 方向 | 已确认的证据 | 结论的边界 |
| --- | --- | --- |
| Minor | 单 mutator、256 KiB 年轻对象、约 1% 存活、128 MiB 无引用旧数组：请求到返回 p95 212 → 15 µs；单独整块退役为 157 µs | 仓库外 runtime 原型，500 事件／格；不代表多线程握手、GUI 或连续分配吞吐 |
| Full | 约 40 MiB 密集存活图，四 worker 仍约 19–25 ms；约 5 ms plan 可来自最终失败的压缩准备 | 基线瓶颈分解，没有 full 优化后数据；七次事件不足以报告 p95／p99 |
| 局部装箱 | 真实 release MIR 的构造已内联，box／别名／类型检查／拆箱仍存在 | 确认优化机会，尚无逃逸分析 pass 的前后性能结果 |
| 局部 closure | 标准库 forEach 只在循环内调用 action，现有 devirt 可选择 typed invoke；LIR 仍为全部 closure class 建立 TD／layout | 源码审计与设计；尚无 closure 消除原型、产物裁剪或性能结果 |
| 同步协程 | leaf 已无 frame，wrapper／outer／Task.run 仍各有 frame 与 adapter 路径 | 无 site 的现有 Immediate 路径已工作；需要前置消除调用点 |
| 循环分块 | LLVM 22.1.8 中 AS1 数组、有界内层 SIMD、两个不同 site 的 relocation／stackmap 可以共存 | 手写 IR 的 poll 只计数；与旧真实 runtime 搬迁原型尚未合成正式源码编译闭环 |

实验方法、完整矩阵、失败条件与限制保留在各调研文档。M56 的历史直接值／装箱、零／多层包装、C 循环分块对照仅作为机会大小的依据，不能作为 M58 优化实现的加速比。

## 3. M58-A：先减少暂停中的无效工作

普通 minor 中，全死年轻 small block 和已经完成复制、引用回写的年轻 evacuation source 可以整块退役。pin、stress quarantine、release hook、promotion 统计与 activation 清空元数据的责任必须保持；target block 继续正常处理。大对象 remembered 枚举根据既有 TD 的 `object_scan` 跳过无引用对象，真正的旧到新引用仍扫描和回写。两项可分别合入、分别计时。

Full 先处理两处局部工作：在单 source 且既有目标容量上界都不足时，避免进入注定失败的昂贵准备；按 starts bitmap 的置位枚举对象，减少 sweep 和目标准备的空位检查。容量检查只用于否决不可能成功的尝试，不能替代实际对象预留／回滚。位图遍历保持 release、pin、size 与清位顺序，不新增全堆对象列表。

冷 worker 池、空任务阶段、region/card 收尾、根数量和释放回调另外计量。只有确实支配暂停才继续扩展；增加并行阶段或保留更大工作缓冲都需要同时报告低核数响应与 RSS。增量 major 仍按 M56 的[进入条件](../milestone56/DESKTOP-GC.md)评估，不把本期扩成新 collector。

## 4. M58-B：统一迭代与 box／closure 消除

可以开始，但起点应是**函数内、保守的分配来源与使用分析**。先复用现有内联／去虚拟化已经暴露的 MIR；通过局部复制和合法 `Retype` 传播候选，遇到返回引用、外部存储、未知调用、身份／地址观察则保留对象。第一版不发布跨 Cone 摘要，也不创建额外 IR／meta 验证层。

第一个消费者消除全部使用都可替换的 box：在装箱点保存 payload 快照，改写兼容的拆箱与类型检查，删除无用的引用别名，随后生成正常 LIR roots。含引用 payload 仍受精确 GC 保护。现有实际类型分析可用于类型判断，但它的合流会丢失不同来源，不能直接作为 may-escape 分析。

普通优化采用有界的 `devirt → inline → simplify → 逃逸／使用分析 → box／closure 消除 → simplify` 统一迭代。内联 forEach 后才可把 action 关联到具体 closure，下一轮解析并内联 invoke，再以捕获快照替换环境字段、删除分配；closure 消除后原先被它捕获的 box 又可能被下一轮消除。未知调用导致的旧保留结论随正文变化重新计算，不能只在全部内联结束后分析一次。

无实际改写时提前停止，先对照最多 1／2／3／4 轮。跨轮保留原始 caller cost、累计增长与展开历史，不能反复调用现有入口来重置限制。固定点只指当前规则与策略下不再变化，达到轮次上限不等于收敛，详见[统一迭代设计](ALLOCATION-ELISION.md#4-内联去虚拟化与分配消除的统一迭代)。

closure 消除要求所有使用都可替换，不仅是 NoEscape。Scoop 捕获不可重新绑定的 binding；值类型复制完整值，引用复制引用，保留创建点和求值顺序，捕获的引用对象仍可变。循环外创建、循环内多次调用是首批正例；身份／地址观察、剩余调用或发布仍保留。环境消失后，捕获值继续走普通精确 roots。

尾随闭包构成的作用域式 API／DSL 是代表负载，按真实惯用法验收：forEach、Result 操作与手写展开对照，观察高频轻量正文、多层包装的累计分配与调用成本。withLock／use 等还需要当前内联器尚未支持的 callee EH 克隆，列为后续有明确消费者的扩展；首批保留清理路径，不把增加轮次当作已覆盖所有 DSL。详见[实际场景与边界](ESCAPE-ANALYSIS.md#52-尾随闭包与作用域式-api-的实际覆盖)。

动态分配与静态支持产物分别验收。当前删掉 ClosureAlloc 不会自动避免所有 TD／layout 生成；迭代结束后，仅在相关实例、callable 引用及现行输出要求均已消失时省掉私有环境 TD／layout／scan 和无用 invoke／bridge。同 class 的其他实例、共享函数类型信息与实际 Strong／ODR 引用照常保留，MIR 记录保持完整。

这条路径不需要通用栈对象头、栈地址与堆引用混用、额外扫描协议或对象生命周期 ABI。后续简单 class 可使用字段局部值，按实际消费者扩展。具体转移规则、循环限制、闭包支持产物、构造器副作用和测试见 [ESCAPE-ANALYSIS.md](ESCAPE-ANALYSIS.md)。

## 5. M58-C：尾部转交与同步正文内联

优先实现简单 tail suspend 包装：函数的所有可达挂起点均为真正尾调用，exact 结果类型匹配，没有调用后的结果处理、catch／finally 或 Context／其他 cleanup 义务时，将原 completion 传给 callee，原样返回 `CoroutineStep`。callee 即使真正挂起，也无需本函数的状态机、frame 或 site adapter；允许普通计算前缀和多个尾调用分支。先有非尾部挂起再尾调用的函数仍需状态机。该优化不依赖内联，也不要求机器 tail call，详见[尾部转交](ALLOCATION-ELISION.md#23-真正尾部-suspend-调用直接转交-completion)。

在 typed MIR 正文及调用点登记完成后、`transform_suspend_abis()` 和 `coroutine::transform()` 之前，内联同一 Cone 内直接可见的小型同步 callee。初始叶子无 suspend site，不观察／发布 hidden completion；移除调用后同步更新 site registry，让 wrapper 继续走已有 Immediate 路径。普通 GC、参数求值、副作用与取消检查保持，不能把“不挂起”当成 NoGC 或纯函数。

这一前置阶段也允许有界交替目标解析／去虚拟化、同步正文内联和 site 清理，并将已确认同步的事实传播给受影响 caller。它不能由状态机生成后的普通优化多跑几轮替代；前置迭代完成后只生成一次正式状态机。

同步内联初版限制无 EH 的小 CFG，递归、动态／外部 suspend 调用与真正挂起路径保留。复用已有 clone／ID 重映射与代码增长选择；不通过把 `Continuation<T>` 强转成 `Continuation<U>`、传 null 或延迟 adapter 分配来省对象。一般逃逸分析不负责推导协程的一次完成协议。

同步内联后的 CFG 可以再次产生尾调用候选。无帧的 tail wrapper 仍可能返回 Suspended，不能因其不再有自身 site／frame 就把它当成同步叶子。尾转交在 ABI 转换时保持完整参数和原 step 返回，与本函数剩余 site 一起决定是否生成状态机；同步恢复、跨线程、失败与唯一完成协议保持。

## 6. M58-D：外层 poll 覆盖有界内层

初版只处理稳定边界、非负单调归纳变量、无调用／分配／异常边及复杂跳转的简单循环。`N` 是编译期的内层最大次数；末块采用 `min(N, n - i)`，不要求补满，不先计算可能溢出的 `i + N`。保留入口 poll；外层正常生成 site 和 root plan，内层的有界事实由实际 CFG 结构维持。

先只减少 poll 次数，再对满足条件的内层开启必要 LLVM 循环优化；当前 Managed 标量管线不会因为内层少了 poll 自动向量化。每块使用外层 poll 重定位后的 managed 基址，不能跨块保留旧 derived pointer，也不能通过给整个函数标 NoGC 绕过合同。

用 N=16／64／256 对照，优先吞吐已接近平台的较小块。N 是优化选择，不是墙钟暂停上界；必须联合观察另一个 mutator 发起 GC 时的 stop-wait 尾部。没有调用并不保证没有缺页或调度延迟，不加入运行时计时器或通用操作预算。

## 7. 实施顺序、规范与验收

本轮增加 [LLVM codegen 优化](LLVM-CODEGEN.md)：先补范围推导与 NoGc 标量优化，再按真实源码对照评估 memcpyopt、aggressive-instcombine、vector-combine 和小范围循环优化。普通优化始终先于最终 roots 和 RS4GC；该工作与 D 的内层资格分析分别验收，不扩大到 LTO 或通用 pass 调度。

LLVM 优化之后继续落实 [runtime C → IR 候选](ALLOCATION-ELISION.md)：重新核对已有实现，再优先评估静态布局拆箱、String byteLength／byteAt 与静态数组复制。每项先明确 fast path 可依赖的 typed 事实，再修改规范、实现、做真实移动 GC 与性能对照；复杂检查和运行时协调沿原慢路径。此项已纳入实施范围，尚未实现，不能把早期审计当作完成。

先实施 A 的两个 minor 改动和 B 的 box 消费者，两者独立；B 随后在同一迭代中补齐 forEach 闭包消除与无用支持产物清理。用 full 对照决定 A 的后续局部改动，并完成 C；D 先交付正确分块，再单独启用内层 SIMD。所有优化按功能提交，保留独立前后对照，不能用一个总体总分掩盖暂停或 footprint 回退。

实现前先更新对应规范：

| 改动 | 规范位置与要求 |
| --- | --- |
| 年轻块退役／full 工作量 | 运行时规范 3.9：退役、元数据复用、对象释放与搬迁责任；3.2／3.3 根与握手语义保持 |
| 逃逸分析、box／closure 替换 | 实现规范 2.19：统一迭代、分析失效、保守保留与 roots 前完成；2.3 及物理输出条款明确无用闭包支持项的选择；语言 3.3／4.4.4／8.1、runtime 2.3／2.6 的值、identity 和捕获语义保持 |
| 协程尾转交／前置内联 | 实现规范 2.3／2.19：明确 tail 的 step 转交和 source signature 阶段顺序；runtime 第 8 章区分无自身恢复状态与实际挂起，语言 8.2／8.3／11.9 的协议保持 |
| 循环分块 | runtime 3.2 与实现规范 2.19：允许外层 poll 覆盖已证明有界、无 safepoint 的内层，保留 continue 和移动根要求 |

分项实施时按上表先修改规范，再修改代码和引用方；尚未实施的策略保留在设计中，不提前写成当前规范行为。实际完成范围见实施记录。

验收矩阵应同时涵盖以下维度：

| 维度 | 报告与必要组合 |
| --- | --- |
| 分配 | 每操作次数／bytes、连续分配耗时、minor／full 触发次数、refill 与空闲后突发 |
| 暂停 | 逐事件 stop-wait、停稳后的 pause、请求到返回；minor／full／晋升失败、冷池／热池分开；尾部须有足够样本 |
| 内存 | 峰值与稳定 RSS、元数据、空闲后回落和重新打开成本；不能把延迟释放到计时外算作净收益 |
| 编译与产物 | release 编译时间／峰值内存、机器码体积、实际分配及 poll 数；优化 IR 断言保持小而精确 |
| 正确性 | 真实源码与产物闭环、移动 GC、pin／release、含引用聚合值、协程 race、异常／Context、循环末块与退出路径 |
| 平台与桌面 | Darwin／GNU／musl；单／多 mutator、低核数／低配置、实际输入／布局／绘制帧循环与漏帧率 |

普通语义 fixture 验证结果与诊断，优化专测才检查分配、site、roots 和必要机器码。文档修改本身无需重跑编译器全量测试；生产变更按仓库格式化／lint 和正式 CLI fixture 流程验收。

本轮调研未使用 nuc12。仓库外实验源码、runtime 对照、私有工具链／sysroot 与原始结果已清理；生产源码没有临时修改。后续若在 nuc12 实验，必须另建独立 worktree，结束时恢复并清理自己的改动和产物。
