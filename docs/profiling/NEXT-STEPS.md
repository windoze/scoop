# PGO 下一阶段规划

日期：2026-11-14。状态：第一批中的“保留默认内联、反馈只追加”已实现；精确上下文反馈及其余批次仍待实施。现有能力及测量分别见 [DESIGN.md](DESIGN.md)、[VALIDATION.md](VALIDATION.md) 和 [RELEASE-ANALYSIS.md](RELEASE-ANALYSIS.md)。实施各批次前先更新受影响的权威规范；本文不把尚未落地的建议行为写成已交付能力。

**已实现的内联基础。** 默认 release 的内联策略较保守，选中的小函数即便位于冷路径，展开成本通常也有限。当前 PGO 以默认内联为基础，只追加机会，不因训练频率较低而否决默认策略已经完成的内联。这里的基准必须使用相同源码、目标、语义选项与 IR 模板范围，不能把普通 default 库和 PGO full 库之间的候选差异算成反馈效果。

当前实现没有把已展开正文重新还原为调用的 pass；首版问题是反馈构建重新选择时没有保留默认选择。现在先按现有轮次完成默认内联、去虚拟化和配套化简，再对剩余调用及新暴露的机会使用反馈。两阶段复用稳定模板、已有分析和累计代码增长记录，不重置已消耗增长量，不编译两份 binary 来比较或回放选择。只取 `max(static_heat, profile_heat)` 或“静态／反馈任一通过”不足以保证这一点：反馈排序仍可能让新增候选提前占用增长额度。普通静态常量折叠和不可达代码消除继续生效；保留内联选择不等于冻结 IR。

Long hash 的三层默认展开已保住，原来约 61% 的退化在同轮复测中不再复现，热 `mix` 调用仍可追加内联。未训练的 Boolean hash 仍慢约 19%；其反馈标记使现有标量循环识别拒绝该区域，最终 LIR 没有普通 release 的 poll 分块。第一批剩余工作需同时处理上下文反馈及标记对既有优化的干扰，在保留计数对应关系的前提下完成必要的透明处理或消费后清理；不能仅凭“保留默认内联”宣称全部默认优化都已保持。

**典型 PGO 优化与 Scoop 的取舍。** PGO 提供的是频率、类型或值分布；实际转换仍由普通编译器完成。高概率足以选择一个带检查的快速路径，不能代替类型、别名、异常或 GC 正确性条件。

| 优化 | 主要收益 | 所需观测与当前缺口 | 建议顺序 |
| --- | --- | --- | --- |
| 保留默认内联、按上下文追加内联 | 保住小 helper 的默认展开，把额外代码用于高频调用 | 已有逻辑计数和 `inlined_at`；反馈读取与评分尚未完整保留上下文 | 第一批 |
| 分支概率与基本块布局 | 热分支尽量顺序落入，冷块远离主路径；帮助 LLVM 选择布局和部分转换 | 已有 then/else 与 block 计数；尚未映射到优化后分支并生成 LLVM `!prof` | 第二批 |
| 接口／虚调用的带检查去虚拟化 | 高概率类型走直接调用并争取内联、常量传播和字段消除 | 新增调用点的接收者精确类型分布；复用 TD、slot 解析和现有直接调用转换 | 第三批主项 |
| 闭包／回调的间接调用提升 | 热目标直接调用，进一步暴露闭包正文和捕获值使用 | 新增实际目标分布；保留环境和原调用 ABI，不能只凭代码地址推断捕获值 | 第三批后续复用 |
| 常见参数值的函数特化 | 为常见 mode、flag、长度等创建小型专用版本，删去其内部判断 | 新增实际有消费者的参数值分布；现有静态常量传播可消费 guard 后的事实 | 后续按负载选择 |
| `memcpy`／`memset` 等尺寸特化 | 常见小尺寸走常量长度复制／清零，大尺寸保留原实现 | 新增 memory intrinsic 的尺寸分布；LLVM 22 已有对应 pass | 后续优先试验项 |
| 循环展开、剥离、分支外提与版本化 | 为高频循环选择合适展开量；例如 stride=1 或固定短长度的专用循环 | 现有边计数可给平均迭代量；分布、stride 等需定点采集，均值不能冒充固定次数 | 有代表性循环后实施 |
| 冷代码外提、部分内联 | 热入口展开，较大的错误处理或罕见分支保留在外部 helper | 分支频率已有基础；需处理 live-in/out、异常、GC roots 和新增调用成本 | 晚于简单布局与调用特化 |
| 函数排序与热代码聚集 | 改善 I-cache、iTLB 和指令页局部性；启动可按首次调用顺序布局 | 直接调用点计数可构造部分加权调用图；间接边需目标分布，首次调用顺序目前未采 | 单独的链接／布局批次 |

上述优化没有统一的“执行概率超过 99% 就开启”规则。一个只调用一次的位置即使训练中 100% 命中，也未必值得增加检查；某个高频位置即使只有 90%，若能消除较多工作也可能值得。选择应同时考虑执行频率、命中比例、检查及失败路径成本、实际可内联／可化简的正文，以及现有代码增长限制。具体阈值随代表负载测量确定，不照抄其他编译器的常数。

冷代码外提和部分内联首先用于原来未展开的大函数及 PGO 新增候选，不借此把默认已经展开的小函数重新变回调用。

内存分配的冷热分类、按存活期预晋升等也是典型反馈优化，但需要分配位置与存活期关联，不能从函数调用次数或当前 GC 汇总推出来。它们会涉及 Scoop 的分配与收集策略，暂不纳入这轮 PGO 交付。普通字符串长度投影属于已找到的默认 release 改进，继续独立实施，不以新增 PGO 能力为前置。

**接口调用特化：首个主要的新消费者。** 假设某个高频接口调用点的接收者绝大多数为类型 `T`，可以生成下列编译器内部逻辑；这不是新增 Scoop 语法：

```text
receiver, arguments = evaluate_original_operands_once_in_original_order()
if exact_type(receiver.object) == T:
    result = direct_call(resolved_implementation_for_T, receiver, arguments)
else:
    result = original_interface_call(receiver, arguments)
```

检查成功的分支可以进入已有内联、常量传播、局部对象字段替换等优化，收益常常来自这些后续化简。即使训练中只出现 `T`，失败分支仍保留完整接口语义；新类型、未训练实现或分布反转时照常执行。这里不需要 JIT、去优化时的栈状态重建或运行期重新编译。

Scoop 的 interface value 已是 `{ object, itab }`，调用直接读取已有 slot，通常没有每次重新查询接口目录的开销。因此不能预先把“省掉接口查表”计入收益；应测量省掉间接调用及展开实现后的实际效果。当前可复用 [MIR 静态分派解析](../../compiler/mir-lower/src/optimization/dispatch.rs)、[LIR 接口调用与参数适配](../../compiler/lir-lower/src/function/call/dispatch.rs) 和既有对象 TD。

类型检查和目标检查分别表达不同事实：

- **精确类型检查**使用对象 TD 与目标 TD 的身份。最终 image 中 exact type 的 TD 唯一，足以在成功分支建立完整的动态类型事实。普通 `is T` 包含子类型，不能自动改成固定 `T` 实现调用；子类型可能 override。继承和完整泛型 application 必须保持现有规则。
- **实际调用目标检查**比较当前 slot 目标与预期函数／adapter，可以在多个类型共享同一实现时复用一个直接调用路径。它只证明调用目标相同，不证明接收者具有同一个 exact type，因此不能据此进行某个具体类型专有的字段投影或拆箱。
- **itab 地址不作为类型身份**。运行时规范 2.1 允许接口菱形投影的等价表片段具有不同地址；应使用 TD 或实际 slot 目标。box、default method 和 effect 适配继续走已有 typed adapter，不能将其视为同签名裸指针直接替换。

guard 和两条调用路径在 MIR 中形成，随后交给正常 LIR ABI、异常边和精确 roots 生成。接收者和参数只求值一次，失败路径不重算有副作用的表达式；跨 safepoint 的 managed 引用按既有机制更新。只有本次实际依赖中的类型／callee 可以被正常解析和引用；有 full 模板时争取展开，没有可用正文时仍可评估直接调用，无法完整表达目标与适配时保留原调用。

首个批次只为一个主要候选生成检查，不顺带建设任意长度的运行期 inline cache。多态位置后续可按实际收益加入第二候选，每次检查使用剩余分布，并控制累计代码增长。普通多态路径始终存在，目标分布改变不会影响结果正确性。

**反馈数据的最小扩展。** 首版入口、block、call 与边计数只能说明“执行了多少次”，没有记录 interface receiver 是什么，也没有记录间接调用最终去了哪里。新的 value profiling 按实际消费者增加，不能从同一 callee 的全局总次数猜测每个动态调用点的类型比例。

采集位置沿用已有逻辑 body／position 与内联来源。为需要特化且仍保留动态分派的位置记录总次数、主要候选及其次数和未列出的剩余量；有限候选或抽样结果必须说明精度，未列出的类型不等于没有发生。只采集所需的 receiver type、调用目标或特定尺寸，避免每个函数的所有实参都插探针。探针不能为了采集而阻止默认静态去虚拟化或内联。

进程内可以读取 TD 或函数地址，但反馈匹配必须还原到已有 `PersistentExactTypeId`／callable identity，不能跨运行比较 ASLR 地址，也不能使用 FQN 作为身份回退。TD 已有的 `type_id` 可以经当前构建映射关联 exact type；复用其现有身份和碰撞规则，不增加新类型身份体系。探针不保存 managed 接收者、不增加 GC roots，训练的计数资源沿已有 runtime 生命周期管理。

优先复用 LLVM/compiler-rt value profiling 与 `llvm-profdata` 合并。本机 LLVM 22 头文件已提供 `IPVK_IndirectCallTarget`、`IPVK_MemOPSize`、`IPVK_VTableTarget`；但 C++ vtable address point 不等于 Scoop TD／itab，不能直接假定该 kind 已支持 Scoop 的 receiver type。第三批先验证对应值记录及地址到稳定身份的映射；不能把任意类型 ID 塞进有尺寸分桶语义的 MemOPSize 记录。现有 [indexed reader](../../compiler/codegen/src/profiling/llvm.cpp) 只返回 `record.Counts`，同批必须扩展为读取实际使用的 value records，不能只加运行期采集而没有 typed 消费链路。

沿普通 schema／构建更新规则演进映射与系统 profiling 组件，必要时重建配套资源并重新训练；项目未发布，不另建旧数据兼容层。组件继续提供 Profiling/full 与 Release/full 两套库。`.slib` 模板不保存运行状态，值数据不绕过已有 Strong／ODR 归属，也不因存在 profile 映射而保活全部函数或 TD。

用户入口继续使用 `scoop pgo instrument` 和 `scoop pgo optimize`。本轮扩展由数据索引、构建结果和已选消费者承载，不要求用户手动拼接 LLVM value-profile 参数，也不把第二轮上下文训练设为默认前置。

**把已有计数用好。** 当前原始映射按内联副本保留 `inlined_at`，但 [Feedback](../../compiler/profile/src/feedback.rs) 配对后又按原 body／position 聚合，[热度使用处](../../compiler/mir-lower/src/optimization/inline/policy/heat.rs) 未查询内联上下文。第一批应同时保留汇总与实际副本范围：有匹配上下文时使用该上下文；优化中新产生的内联上下文没有实测数据时，用外层频率与内部相对频率形成明确标注的估计，或回退静态判断。不能把整个函数的总次数套到每个副本，也不要求先采完整动态调用栈。

第二批将可对应的真实边计数送到最终 LLVM `branch_weights`，并在需要时提供正确的物理函数入口计数。当前 [条件跳转发射](../../compiler/codegen/src/function.rs) 没有这条反馈接线。MIR 内联、取反、拆块、合并和化简后，权重必须仍对应实际 successors；无法对应时保留未知状态，不能把原始编号硬贴到新 CFG。零计数只影响优化选择，不将合法路径改成 unreachable。函数入口／函数布局使用独立机器正文的实际频率，不能使用包含所有内联副本的逻辑函数总次数。

Scoop 自定义 MIR counter 布局不等于 LLVM 自身 IR-PGO 布局，不能直接给 LLVM 的 profile-use pass 一个 `.profdata` 就期待它读懂。先通过 Scoop 的匹配与 typed IR 传递事实，再以标准 metadata 或实际适用的 pass 接入。单函数 LLVM module 可以先受益于基本块布局；跨函数聚集则由后续 final-link 批次解决，不要求 ThinLTO 先落地。

**实施批次与验收。** 下一阶段的确定交付范围先覆盖前三批，其他项目依据它们产生的数据和真实热点单独选择。

| 批次 | 交付内容 | 必要验证 |
| --- | --- | --- |
| 1：默认内联与上下文 | 默认阶段先执行；反馈只追加；复用已有内联副本计数；报告区分默认选择、实测反馈和估计 | 保住 Long hash 的完整默认展开；新增热点不能挤掉原有候选；同一 callee 的热／冷调用点和嵌套副本仍可区分；真实基准复测 |
| 2：分支与基本块布局 | 对实际对应的 CondBr 等发射合法 LLVM 权重，复用后端布局 | 高偏置分支、取反与合并、内联分支及未覆盖边；最终 metadata／机器布局和端到端时间；零计数路径仍正确 |
| 3：类型反馈与带检查去虚拟化 | 采集并读取所需 value profile；首个 interface／virtual 目标 guard；快路径进入正常内联，fallback 保留 | 多个调用点具有不同主要类型；训练 99:1、验证 1:99 及新第三类型；继承 override、泛型 application、跨 Cone 及实际使用的 adapter；参数副作用、异常与 moving GC 定向组合 |
| 后续 A：目标与值特化 | 复用第三批采集与 guard 链路处理适合的 closure／callback；以真实 memcpy／memset 或小型 mode/长度负载选一个值消费者 | 相同代码不同捕获值、常用值与其他值／边界值、尺寸分布变化；比较检查成本、代码增量与实际吞吐 |
| 后续 B：循环与整体布局 | 按热点选循环版本化、部分内联、冷块外提或函数聚集，分别交付 | 循环零次／短／长输入、alias 与 GC poll；EH／roots／链接平台合同；冷路径正确性、text 大小和指令缓存相关证据 |

新 profile 的匹配不等于产生收益。复用既有优化报告，记录位置及内联上下文、主要候选比例、guard 与默认路径、选择或拒绝原因和代码增长；默认阶段的静态展开不冒充 PGO 消费。比较使用相同模板集合和无探针 binary，保留未参与训练的输入，记录吞吐／延迟、编译时间、text、分配及必要的硬件采样。先在本机完成一个大项的语义和性能验证，再到其他原生目标验证代码生成、ABI 与运行；不为每个小改动重复完整六目标验收。

即使训练中某路径从未出现，也不据此删除边界／类型／异常检查、宣称对象不逃逸或任意取消 GC safepoint。guard 成功后能建立的事实由普通静态规则继续推导；循环 SIMD、拆箱和分配消除只在其既有正确性条件满足时发生。下一阶段不引入 JIT、通用 deoptimization、全量路径追踪或自动调参平台。

**调研依据。** 下列资料用于确认典型转换和 LLVM 可复用能力，不代表 Scoop 已启用相应 pass；具体 LLVM 实现以本项目使用的 22.1 为准。

- [LLVM 22.1.8 IndirectCallPromotion.cpp](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/Transforms/Instrumentation/IndirectCallPromotion.cpp)：基于 value profile 的目标选择、函数／vtable 比较与直接调用提升。
- [LLVM 22.1.8 PGOMemOPSizeOpt.cpp](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/Transforms/Instrumentation/PGOMemOPSizeOpt.cpp)：按常见尺寸生成 guarded memory intrinsic 版本。
- [LLVM Branch Weight Metadata](https://llvm.org/docs/BranchWeightMetadata.html)：分支权重、物理函数入口计数，以及 CFG 改写后的维护责任。
- [Clang Users Manual：PGO](https://clang.llvm.org/docs/UsersManual.html#profile-guided-optimization)：普通 instrumentation 与内联后 context-sensitive instrumentation 的区别。
- [GCC Optimize Options](https://gcc.gnu.org/onlinedocs/gcc/Optimize-Options.html)：profile-use 下的内联、值传播与 clone、循环转换、函数排序；speculative devirtualization 同样使用有条件的直接／间接路径。
