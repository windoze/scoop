# M62 设计：intrinsic 下降、跨 Cone 内联与 runtime 性能

状态：已完成，交付与实际验证范围见 [实施记录](PROGRESS.md)和 [验收记录](ACCEPTANCE.md)。设计日期：2026-10-23。核查基线：`f8df637f59211cfb7c7e5c29a5eaa25ceec970e0`；下文的问题描述、源码调查与原始测量均对应此基线，不表示修复后的当前状态。

本期处理已经确认的 intrinsic 下降遗漏、跨 Cone 小函数内联缺失，以及接口分派、poll、分配、GC 和最终映像中的性能成本。设计保留源码与产物证据、原分析的必要修正、候选方案和验收边界；实验收益不代表生产优化已完成，各用例耗时也不足以排出所有程序通用的瓶颈顺序。

依据：[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)、[语言规范](../specs/SCOOP-SPEC.md)、[M56 桌面性能取舍](../milestone56/DESKTOP-GC.md)与 [M58 设计](../milestone58/DESIGN.md)。

## 目标与范围

**必须保证 intrinsic function 在所有“类型已经确定”的上下文中，都能下降为对应的真正 intrinsic，包括所有静态上下文，以及经去虚化（devirt）可以确定具体类型与调用目标的上下文。** 这一保证独立于普通函数内联，不以修复 Int.plus 或某一个泛型用例作为完成标准；完整要求与验收矩阵见第 2.4 节。

| 范围 | M62 要求与边界 |
| --- | --- |
| intrinsic 下降 | 覆盖本地与跨 Cone、泛型特化、静态函数引用及 devirt 后确定的目标，沿已有类型化信息完成语义下降 |
| 普通小函数内联 | 支持符合条件的跨 Cone 非泛型函数参与内联，改善本地成本模型；保持身份、求值、异常、GC 与 ABI 合同 |
| runtime 热路径 | 优先减少 TD 重复查询，并独立评估有界 core leaf、poll 与 TLAB 完成路径；保留必要发布与握手职责 |
| 需实验决定的方案 | nursery 调整、父接口表布局、24 字节 HFA、最终导出精简和 HashMap/String 改进分别对照；本文不预先决定全部采用 |
| 既有性能目标 | 沿用 M56 的桌面短暂停和 footprint 取舍；与 M58 已有优化协作，不另建 collector、通用调参器或第二套优化框架 |

当前已确认的 imported intrinsic 遗漏和跨 Cone 普通内联缺失是两个独立问题。前者必须按 intrinsic 的具体语义消除遗漏，后者仍由普通内联策略控制展开范围。

## 问题总览

| 编号 | 已确认的问题或成本 | 证据与适用范围 |
| --- | --- | --- |
| 1 | 接口查表重复校验 TD 登记；部分继承接口调用重复查找父接口表 | 源码、现有 runtime 副本对照；集合遍历用例收益明显 |
| 2.1 | MIR 内联成本包含冷异常路径和未消除的别名拷贝 | 成本模型、现有数组及 V3 MIR；不是所有未内联调用的共同原因 |
| 2.2 | 跨 Cone 的普通非泛型函数不进入 MIR 内联候选集 | 两个独立库 Cone 的最小复现，最终机器码保留调用 |
| 2.3 | 泛型约束特化到外部 Int.plus 后，未恢复为整数运算 | 本地及导入泛型均复现；明确类型的直接整数运算正常 |
| 3 | Managed 小函数入口也有 poll，快路径含四次 acquire load | 源码及机器码；NoGC 函数不插入，TLS 地址已有函数内缓存 |
| 4 | TLAB bump 后仍逐对象调用分配完成函数 | 源码确认；包含清零、元数据发布、计数等工作 |
| 5 | 1 MiB nursery 在 trees 中造成高晋升量和大量 GC 工作 | 现有对照及补充诊断；16 MiB 同时改变 minor/full 触发模式 |
| 6 | Long.hash 等很小的 core extern 仍经过 NativeBorrowed 切换 | 源码及 hashLong 采样；不能将整个 String 操作族视为常数时间 |
| 7 | 24 字节 V3 被 Scoop ABI 的 16 字节限制挡在寄存器分类之前 | ABI 规划代码确认；目标 HFA 分类器本身已经存在 |
| 8 | 最终映像链接信息、weak 符号与绑定项较多 | 实际 Mach-O 统计及启动采样；启动百分比尚不足以精确归因 |
| 9 | HashMap 和 String hash 存在可见的逐次查找成本 | 源码确认；单项性能收益和数据布局取舍尚待对照 |

## 1 接口查表中的重复登记校验

`scoop_rt_itable_lookup` 每次先对对象 TD 和目标接口 TD 调用 `scoop_image_require_type`，两次登记查找均进入基于地址索引的 `bsearch`，之后才线性扫描 `obj_td->itables`。`scoop_rt_is_instance` 也重复检查 source TD，并在目标 TD 非空时检查目标 TD，但其后执行的是更一般的子类型判断，不能把它描述成与 itable lookup 完全相同的线性查表。

接口值可以携带当前接口表；当通过子接口值使用父接口成员、需要获得另一个接口视图而又没有已知具体类型时，仍会走运行时查表。因此，问题应限定为需要重新取得接口表的路径，不能泛化为每一次接口调用都执行两次 `bsearch`。

现有 runtime 副本 `rtfast` 的关键差异仅为删除 `scoop_rt_itable_lookup` 中的两次登记校验，没有移除线性扫描，也没有改 `scoop_rt_is_instance`。已有对照记录如下：

| 用例 | 原版本 | 去掉这两次校验 | 加速比 |
| --- | ---: | ---: | ---: |
| listIter / forEach | 约 2.56 s | 约 0.54 s | 约 4.7 倍 |
| mapFilter | 约 1.38 s | 约 0.39 s | 约 3.5 倍 |

这个对照支持“重复登记校验在这些用例中是重要瓶颈”。它没有单独量化线性扫描的成本，也没有测出删除 `is_instance` 校验的收益。

优先在已经完成镜像登记与引用检查、TD 生命周期稳定的内部路径复用既有事实；真正输入边界的必要检查应保留。这与 AGENTS.md 对“同一事实在明确边界验证后，若数据未变化，应复用结果”的要求一致。进一步可评估子接口表携带父表指针、继承 slot，或在接口转换点取得父表，避免热调用反复查找；这涉及接口表示和跨产物 ABI，需先明确契约。

依据：[rt.c:192](../../runtime/src/rt.c#L192)、[登记检查](../../runtime/src/image/active.c#L38)、[地址查找](../../runtime/src/image/lookup.c#L12)、[接口表取得路径](../../compiler/lir-lower/src/function/interfaces.rs#L84)、[登记期类型关系检查](../../runtime/src/image/type_relations.c#L21)、[AGENTS.md](../../AGENTS.md)。

## 2 内联问题与 intrinsic 下降要求

### 2.1 本地内联成本受到冷路径和别名拷贝影响

成本模型对基本块求和，分配计 12，普通直接调用通常计 6，throw 等终结符计 8，声明或赋值还计入值复制成本。数组访问的异常构造路径因此会抬高整个 accessor 的成本。循环内普通阈值为 36；存在值类型参数复制时可加至 40，其他启发条件还会改变阈值，不能把 36 或 40 当成所有调用的统一上限。

内联选择前已经存在基于参数事实的常量折叠和不可达路径简化，不能说“计算成本前完全没有优化”。但这一步没有完整消除非恒定值的别名声明，`$dependency.*`、`$parameter.*`、`$inline.*` 等复制仍可能被计费。现有 V3 小函数 MIR 保留了大量这类中间值，数组访问与 V3 用例仍保留 accessor/helper 调用。冷异常路径和别名声明的计费会高估某些小函数，但不能用这一原因解释第 2.2、2.3 节的遗漏。

可先做必要的拷贝传播与死声明清理，再评估成本；对已经识别的冷异常路径降低权重，或将异常构造放入不内联的 cold helper。不能仅因一个块以 throw/unreachable 结尾就无条件把整块成本归零，也不能通过优化删除必要的越界语义。

依据：[成本与阈值](../../compiler/mir-lower/src/optimization/inline/policy.rs#L80)、[内联选择](../../compiler/mir-lower/src/optimization/inline.rs#L144)、[现有简化步骤](../../compiler/mir-lower/src/optimization/simplify.rs#L12)。

### 2.2 跨 Cone 非泛型小函数没有进入内联候选集

MIR 内联器的 `local_target` 仅识别 `Callee::User` 和 `Callee::Monomorphized`。普通跨 Cone 调用下降为 `Callee::External`，在成本计算之前就被排除。导入的外部 callable 引用、签名和 effect 不等于一个可供当前内联器展开的本地函数体。

最小复现把相同的一行整数加法函数分别放在本 Cone 和另一个库 Cone：本地 helper 成功内联；跨 Cone helper 在最终 ARM64 可执行程序里仍保留 `bl`。这是候选集与函数体可用性的问题，提高成本阈值无效。

通用修复方向是让符合条件的外部函数体可被消费者用于优化，并纳入现有内联选择，正确维护实体身份、依赖、ABI 与代码体积约束。这里不要求将所有外部函数都内联。

依据：[候选目标限制](../../compiler/mir-lower/src/optimization/inline/policy.rs#L6)、[导入 callable 引用](../../compiler/mir-lower/src/current.rs#L175)、[外部调用下降](../../compiler/mir-lower/src/body/imported_calls.rs#L25)。

### 2.3 泛型特化后遗漏 imported intrinsic 处理

`Int` 的加法方法实际名为 `Int.plus`，标注 `@Intrinsic("int_add")`。直接写明确类型为 `Int` 的 `a + b` 或 `a.plus(b)`，会正常产生整数加法；问题出现在泛型约束选择具体实现之后。

```scoop
fun <T> genericAdd(a: T, b: T): T where T: Add<T, T> = a + b

fun test(a: Int, b: Int): Int = genericAdd<Int>(a, b)
```

泛型实例化选中 core 中的外部 `Int.plus` 后，HIR 保留 Imported 调用，MIR 再把它变成 External 调用。现有“单态化后恢复 intrinsic 运算”的特殊处理只覆盖本地方法路径，外部路径没有对应处理。

release 最小复现得到以下结果，程序的六个入口均输出 42：

| 路径 | 优化后 MIR 与最终机器码 |
| --- | --- |
| 明确 Int 类型的 `a + b` | IntegerBinary add，最终为 `add`，无加法函数调用 |
| 明确 Int 类型的 `a.plus(b)` | 同上 |
| 本 Cone 非泛型加法 helper | 成功内联 |
| 其他 Cone 非泛型加法 helper | 保留真实调用 |
| 本 Cone 定义的泛型加法特化为 Int | 泛型函数体已内联，内部仍调用外部 Int.plus |
| 从其他 Cone 导入的泛型加法特化为 Int | 同上 |

两个泛型路径的最终调用目标都只做建栈帧、整数 `add`、恢复栈帧和返回。因此基础运算确实受到影响，但不能概括为所有 `Int` 加法都未内联。该目标是 NoGC 的普通 Scoop callable，不涉及第 6 节的 native 状态切换。

泛型特化后的 imported intrinsic 遗漏是第 2.4 节统一下降要求的一个已确认反例。修复应利用已有的类型化 intrinsic 信息恢复原始运算，可以独立于通用跨 Cone 内联完成，也不应依赖方法名称或链接符号猜测 intrinsic 身份。当前确认了额外调用，尚未量化它在原性能用例中的占比。

依据：[Int.plus 声明](../../sysroot/src/scoop.core/src/integer_int.scoop#L72)、[特化后的 Imported 分支](../../compiler/hir-lower/src/concretize/methods.rs#L221)、[本地 intrinsic 处理](../../compiler/mir-lower/src/body/calls.rs#L290)、[外部调用下降](../../compiler/mir-lower/src/body/imported_calls.rs#L25)。

### 2.4 类型确定后的 intrinsic 下降要求

**必须保证 intrinsic function 在所有“类型已经确定”的上下文中，都能下降为其对应的真正 intrinsic。** 这包括所有静态上下文，以及经 devirt 可以确定具体类型与调用目标的上下文。不能只修复 Int.plus 或某一种泛型导入路径。

这一要求贯穿调用解析、泛型特化、去虚化以及后续优化：只要已经具备确定 intrinsic 具体操作所需的类型与目标信息，就必须完成相应下降。若具体目标在较晚阶段才被确定，该阶段或其后续处理必须消费这个新事实；不能因早期已经将调用表示为 Imported、External、虚调用或间接调用而错过下降。

| 上下文 | 必须满足的行为 |
| --- | --- |
| 静态已知目标的直接调用、运算符语法和显式方法调用 | 对同一个 intrinsic 采用相同的语义下降，本地与跨 Cone 一致 |
| 本地或导入的泛型特化，以及具体类型上的约束实现选择 | 特化后选中的 intrinsic 必须恢复为对应操作，不能停留在普通 callable 调用 |
| 已能静态确定目标的函数引用或其他间接调用 | 消费已经确定的目标信息，进入对应 intrinsic 下降路径 |
| 经 devirt 确定目标的接口或虚方法调用 | 去虚化后继续完成 intrinsic 下降，不能仅改成对 intrinsic 包装函数的直接调用 |
| 内联、常量传播或后续优化进一步暴露具体类型与目标 | 新确定的 intrinsic 调用也必须完成下降，不能依赖某一次早期处理是否已经执行 |

表达式所在位置不构成例外：默认实参、属性访问器、初始化器、闭包正文和函数引用调用等静态上下文均适用。对已经确定的目标，debug／release 和普通内联开关不改变这条下降要求；优化配置可以影响是否推导出额外类型事实，但已经取得的事实必须被消费。

这里的“真正 intrinsic”按各 intrinsic 既有语义定义：例如 Int 加法生成整数加法 IR；需要目标指令或规定 runtime helper 的 intrinsic 则采用其相应实现。保留必要的异常、溢出、内存及 GC 语义，不要求所有 intrinsic 都对应单条机器指令。

下降应使用跨阶段、跨 Cone 保留的类型化 intrinsic 身份和操作信息，不受普通函数内联成本、函数大小阈值或外部函数体是否可内联的影响。不同路径确定同一目标后应得到一致的处理结果。

验收必须同时检查运行语义和优化后 IR／最终机器码，并覆盖上表的路径组合。除 Int.plus（int_add）外，应按现有 intrinsic 分类选取必要的代表用例；包含静态直接调用、本地与导入泛型、函数引用解析，以及 devirt 后才确定目标的情况。对仍无法确定唯一实现的动态调用，保留正确动态分派，不能按接口声明或名称猜测 intrinsic。第 2.3 节仅记录已完成的最小复现，本节完整覆盖是后续修复的验收要求。

### 2.5 LLVM 没有消除这些跨 callable 调用

当前 codegen 按 callable body 分别准备 LLVM module 并输出对象文件。上面的最小复现也确认最终链接产物仍保留调用。因此，不能期待当前 LLVM 编译流程自动弥补这些 MIR 层遗漏。这个结论针对现有流程，不等于 LLVM 本身不具备跨函数或跨模块优化能力。

依据：[逐 callable 输出对象文件](../../compiler/codegen/src/emission/objects/emit.rs#L57)。

## 3 Managed 小函数入口的 poll 成本

当前 safepoint 插入逻辑对 Managed 函数入口和指定循环头插入 poll，没有单独豁免无循环、无调用的有界叶子函数。NoGC 函数会直接跳过这一过程。

每个 poll 快路径读取全局 GC epoch、world phase、线程 mode、线程已观察 epoch，共四次 acquire load。macOS 上取得 TLS 地址也有成本，但 `cached_poll_state` 已把地址放在函数入口取一次并在函数内复用，循环中的每个 poll 并不重新调用 TLV getter。

GC 根的 materialize/restore 位于 poll 的慢分支。因而“GC 根在每一轮 poll 快路径上都溢出并重载”不成立。实际机器码仍可能因为寄存器分配、其他 Managed 调用和栈布局出现栈访问，应分别归因。

可优先评估有界叶子函数的入口 poll 消除，以及快路径状态读取的精简。单字标志、relaxed 快读和慢路径 acquire 都只是待验证设计，需要保证不会漏掉停止请求。跨函数传递 TLS 地址或更换 TLS 模型涉及目标平台和 ABI，不能作为已经可直接应用的通用方案。

依据：[poll 插入条件](../../compiler/lir-lower/src/safepoints/cfg.rs#L89)、[四次读取与慢路径](../../compiler/codegen/src/function/gc/poll.rs#L26)、[TLS 地址缓存](../../compiler/codegen/src/function/gc/poll.rs#L105)。

## 4 TLAB 分配完成路径仍有逐对象成本

指针 bump 已内联，但每个符合该路径的对象仍调用 `scoop_runtime_finish_tlab_alloc`。后者取得当前 mutator，检查 TLAB 与类型契约，清零对象，设置对象头，登记对象大小及 bitmap，并更新分配计数。普通 nursery 分配会更新四个计数；这些计数使用 relaxed 原子 load/store，不是四次原子 fetch-add。

block 定位优先复用 `mutator->allocation_block`，仅在地址不匹配时回退到 `pointer_block`，不能说每个对象都完整查找 block。`line_occupied` 使用 relaxed fetch-or；`starts` 使用 release fetch-or，与对象查询侧的 acquire load 配对。

可评估进一步内联已知 TLAB 的完成路径、删除已由编译器和分配入口确立的重复检查，以及合并确无并发读者的写入。但“TLAB 属于单一 mutator，因此元数据全部改为普通 store 或延迟到退役登记”不是已经证明正确的结论。单写者不意味着没有并发读者，`starts` 还承担对象发布职责。零初始化、精确对象边界及对象在下一次使用前的可识别性也必须保持。

依据：[分配完成与计数](../../runtime/src/gc/allocation.c#L196)、[bitmap 发布](../../runtime/src/gc/heap_objects.c#L67)、[对象起点读取](../../runtime/src/gc/heap_objects.c#L36)。

## 5 nursery 容量与高存活负载的交互

默认 nursery 固定为 1 MiB，初始 full 触发阈值为 16 MiB。refill 先检查 full 条件，再检查 nursery 是否达到容量。因此将 nursery 改为 16 MiB，可能使程序在下一次 minor 之前先触发 full。

`trees` 保留一棵长期存活树，并反复构造、遍历其他树。原分析中约 91% 的晋升比例得到补充诊断支持。2026-10-23 对现有两个 release 产物各执行一次，启用 `SCOOP_GC_STATS=1`，清除强制 full-only 和 GC stress 环境变量，得到下表。两个程序输出均为 `5373911`，分配字节数一致。

| 指标 | 1 MiB | 16 MiB |
| --- | ---: | ---: |
| 进程墙钟耗时，含启动 | 528.9 ms | 213.6 ms |
| minor 次数 | 159 | 0 |
| full 次数 | 13 | 19 |
| 分配字节数 | 171,965,472 | 171,965,472 |
| 晋升字节数 | 156,782,648 | 82,039,576 |
| 实际复制字节数 | 155,890,008 | 0 |
| 累计已停稳 pause | 366.328 ms | 61.359 ms |
| 其中 minor pause | 303.884 ms | 0 |
| 最大已停稳 pause | 7.937 ms | 3.453 ms |
| 累计 update 时间 | 150.940 ms | 0 |
| 峰值 RSS | 46.77 MiB | 40.44 MiB |

这是用于核对行为的一次诊断运行，不是多轮基准的中位数或尾延迟分位数。它支持“当前策略在这个负载中有大量晋升与搬迁工作”，但同时表明 16 MiB 版本的收益伴随着 minor 完全消失、实际复制为零，不能仅归因于 nursery 变大或 refill 次数减少。

还需修正两个指标解释：`promoted_bytes` 不等于实际复制字节数，原地改变代际也可能计入晋升；`update_ns` 是累计阶段统计，包含相关 minor/full，不能把约 151 ms 全部认定为 minor 回写。`pause_ns` 只统计已停稳阶段，也不等于 GC 请求到恢复的完整阻塞时间。

保留默认 1 MiB 的 M56 决策有桌面短暂停和内存占用依据。容量调整、按存活率调整或 survivor 机制都应作为独立候选，重新比较低、中、高存活率及多个 mutator，并分别报告 minor/full、停止等待、请求至恢复、复制量、RSS 和实际帧延迟。当前不支持仅凭 trees 把默认值改为 16 MiB。

依据：[容量常量](../../runtime/src/gc/heap_internal.h#L21)、[refill 的触发顺序](../../runtime/src/gc/allocation.c#L53)、[统计口径](../../runtime/src/gc/statistics.c#L69)、[M56 桌面取舍](../../docs/milestone56/DESKTOP-GC.md)、[M56 容量验收](../../docs/milestone56/ACCEPTANCE.md)。

## 6 小型 core extern 的 native 状态切换

`Long.hash` 最终调用 `abi="scoop"` 的 `coreLongHash`，计算本身只需固定次数的整数混合运算。当前 Scoop extern 下降为 NativeBorrowed 调用，经过 enter/leave、栈段与根帧处理以及线程状态更新；已有 hashLong 采样中这条路径明显可见。

这类开销不能仅靠给 Scoop extern 添加 `@NoGC` 消除。`@GCLeaf` 当前只适用于 C ABI extern，Scoop extern 的 NativeBorrowed 路径是另一项调用协议选择。对 Long/ULong/Boolean hash 等有界且不访问托管对象的操作，可优先评估 intrinsic 或合适的无状态切换 leaf 路径。

String 必须单独区分：equals、hash、按 Unicode scalar 计数的 length 可随输入长度线性增长，不能统称为常数时间，也不能仅因不分配就认定适合任意长时间的 leaf 执行。`String.compareTo` 当前已声明为 intrinsic，不能继续归入“未标注的普通 Scoop extern”。`byteLength` 与字符 length 也不是同一个操作。

依据：[Long hash extern](../../sysroot/src/scoop.core/src/types.scoop#L132)、[计算实现](../../runtime/src/rt.c#L66)、[Scoop extern 调用协议](../../compiler/lir-lower/src/function/call/native.rs#L23)、[GCLeaf 限制](../../compiler/hir-lower/src/annotations/functions.rs#L133)、[native 状态转换](../../runtime/src/thread/transitions.c#L113)、[String 声明](../../sysroot/src/scoop.core/src/strings.scoop#L27)、[字符 length 实现](../../runtime/src/strings.c#L11)。

## 7 V3 的间接 ABI 与复制

测试中的 V3 是三个 Double，共 24 字节。Scoop ABI 对普通 aggregate 先要求大小不超过 16 字节、对齐不超过 8，之后才调用目标分类器；V3 因此采用间接参数和间接返回。

目标分类器已经支持 Darwin AArch64 的 HFA，包括最多四个同类浮点字段。问题是 Scoop ABI 的前置限制使 24 字节 V3 到不了这个分类器，不能描述成“完全没有小 struct 寄存器传参或 HFA 支持”。

应先修复小函数内联，让调用边界和相应复制有机会消失。对于仍存在的真实调用，再评估放宽 Scoop ABI 的 HFA 条件。后者属于 ABI 契约变化，需要同步 producer/consumer 与产物记录，不宜作为单点机器码优化直接修改。

依据：[Scoop ABI 前置限制](../../compiler/codegen/abi-plan/src/scoop.rs#L16)、[AArch64 HFA 分类](../../compiler/codegen/abi-plan/src/aggregate.rs#L43)、[布局读取上限](../../compiler/codegen/abi-plan/src/layout.rs#L16)。

## 8 产物体积和启动成本

对现有 `bench` release 可执行文件重新统计：

| 项目 | 数值 |
| --- | ---: |
| 文件大小 | 25,859,800 bytes，约 25.86 MB |
| __LINKEDIT 文件大小 | 20,387,544 bytes，约 20.39 MB，占文件约 78.8% |
| nm 符号记录 | 117,584 |
| 其中 weak external | 58,723 |
| weak bind 表项 | 15,003 |

因此，“11.7 万个符号全部是 weak external”不成立。文件体积主要在链接信息，不代表同等规模的机器码或 RSS。weak/可抢占符号会使部分映像内部调用仍经 stub，但不能泛化为所有内部调用；跨 Cone 最小复现里也有直接 `bl`。

已有启动 trace 中可见 dyld 绑定工作以及 runtime 对镜像记录、stackmap 的 qsort，说明两类成本值得优化。不过导出的启动 profile 只有 31 个样本，不宜把“约 60%”作为稳定、精确的全局启动占比。

可优先减少最终映像不需要的动态导出和边界符号，区分仅供静态链接与真正跨 image 使用的符号。保持 ODR 合并、外部可见入口及精确 GC 元数据；随后单独比较冷启动、热启动、文件体积和 RSS。启动排序也应按实际数据不变量评估，不能无条件删除。

依据：现有 release 产物与启动采样（见实验材料说明）、[M56 最终程序链接信息分析](../../docs/milestone56/DESKTOP-GC.md)。

## 9 标准库的可见查找成本

HashMap 的槽位起点使用 `% capacity`；常规扩容采用倍增，但极端容量分支不保证二的幂。改成掩码需要先确立容量不变量，并保持负 hash、扩容边界与探测行为。

查找在用户定义的 hash/equals 回调前后执行 ModificationCounter 的 snapshot/check。这些检查承担检测回调重入修改集合的语义，不是可以随意删除的纯开销。应优先消除调用包装和临时值；只有编译器证明回调不可能修改集合时，才考虑消除检查。

states 与 elements 是两个数组，源代码访问各自携带边界检查，HashEntry 还缓存了已存储条目的 hash。应评估两次数组访问、检查消除与布局局部性，但不能未经对照就断言合并数组更快，也不能声称存储条目的 hash 每次都重算。

String hash 当前逐字节执行 FNV 式混合，结果不缓存在 String 中。查询键仍需计算 hash，长字符串重复查询可能受影响。是否缓存或更换算法，应比较重复使用率、对象大小、并发访问以及冲突分布，不能只比较哈希循环的单次吞吐。

依据：[HashMap 查找](../../sysroot/src/scoop.std/src/collections/hash_map.scoop#L34)、[扩容](../../sysroot/src/scoop.std/src/collections/hash_map.scoop#L67)、[存储与槽位计算](../../sysroot/src/scoop.std/src/collections/hash_storage.scoop#L6)、[修改检测语义](../../sysroot/src/scoop.std/src/collections/modification.scoop#L11)、[String hash](../../runtime/src/rt.c#L83)。

## 实施顺序与验收重点

以下按证据强度、局部修复范围和后续依赖安排，不是全局性能排名。

| 顺序 | 建议任务 | 关键验收 |
| --- | --- | --- |
| 第一批 | 消除已登记 TD 的重复热点查询；落实所有类型已确定上下文的 intrinsic 下降 | 保持类型判断与接口结果；复测 listIter/forEach/mapFilter；按第 2.4 节覆盖静态、本地与跨 Cone、泛型特化及 devirt 后的 intrinsic 下降 |
| 第二批 | 内联前复制清理与冷路径计费；支持跨 Cone 非泛型小函数参与内联 | 检查优化后 IR 和最终机器码；保留越界异常与异常展开；同时记录运行时间、编译时间和代码体积 |
| 第三批 | 有界 core leaf、Managed 叶子入口 poll、TLAB 完成路径 | 分别测小调用和分配；对涉及 GC 协议的改动验证多线程停止响应、对象发布与移动正确性 |
| 独立评估 | nursery/回写、24 字节 HFA、链接信息、HashMap/String | 各自建立对照；GC 按 M56 桌面矩阵验收，ABI 按跨产物契约验收，标准库保留回调与集合语义 |

每项应先独立测量，之后再测组合效果。内联会同时影响函数入口 poll、ABI 复制和 stub 调用，单项收益不可相加。必要的语言、runtime、ABI 或 pipeline 职责变化按仓库要求先更新对应 spec。

## 规范衔接

当前实现规范的 MIR 优化合同仍将普通内联限定在本 Cone 已有完整 concrete 正文的 callable，包含本地物化的外部泛型。M62 的跨 Cone 非泛型内联是对该范围的扩展；实现前需同步实现规范中的正文导入、阶段职责和产物数据约定，不能把工作隐藏到 program-link 或重新按源码名称解析。

intrinsic 的统一下降要求需同步到实现规范的调用解析、具体化、去虚化和优化合同。跨阶段仅通过对应 IR crate 传递完整的类型化操作信息，早期静态解析与晚期 devirt 得到同一目标时遵守同一语义；真实动态分派需要的 intrinsic callable 入口仍按实际引用保留。

运行时规范目前要求 Managed 入口 poll、既定的 acquire 检查及对象在分配返回前完成发布。若采用入口 poll 消除、状态字精简、分配发布变化或接口表表示调整，先明确修订对应 runtime／ABI 合同，再修改实现。必要的 GC 正确性约束不能仅凭单线程收益删除；默认 nursery 不因本次 trees 结果自动改变。

## 实验材料说明

原始性能材料来自仓库临时目录 `tmp/perf`：`bench/src/main.scoop` 是工作负载，`out` 为基线，`outfast` 与 `rtfast` 用于接口登记检查对照，`out16` 与 `rt16` 用于 nursery 容量对照；采样、启动 trace 和小型 MIR 用例是本设计的调查依据，不作为正式验收套件提交。

跨 Cone 最小复现由 provider、consumer、runner 三个独立 Cone 组成：provider 导出非泛型加法 helper 与受 Add 约束的泛型函数；consumer 比较直接运算、本地／导入非泛型 helper、本地／导入泛型特化；runner 以 19 和 23 调用六个入口，均输出 42。release 构建检查优化后 MIR／LIR 和最终 ARM64 机器码，结果见第 2.3 节。

第 5 节补充 GC 诊断在 2026-10-23 执行，启用 `SCOOP_GC_STATS=1`，清除强制 full-only 与 GC stress 环境变量；两个产物各运行一次。原始日志、完整 dump、产物和后续逐次报告保留为临时实验材料，新增归档放在仓库外；正式文档只记录范围、结果概要和必要限制。实现后按实际修复更新结果，不沿用调查时的状态作为验收结论。
