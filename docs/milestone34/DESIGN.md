# M34 设计：接口与调用优化、可扩展并行标记 Immix

状态：实施中。本文件规定 M34 的目标行为、实现边界与验收出口，不表示相应代码已经完成。三份规范已按第 10 节先行修订，实际能力与版本迁移见 [实施记录](PROGRESS.md)。本次不回写历史里程碑的实施和性能记录。

日期：2026-10-09。

基线：已完成 M33 的仓库。依赖 M15 的精确根与 moving GC、M23 的分离编译和 artifact-only program-link、M24 的 release hook、M27 的 Context、M28 的三个 target、M31 的优化配置与 nursery，以及 M33 的原子类型、NativeSafe 握手和 DirectC。

关联文档：[路线图](../ROADMAP.md)、[语言规范](../specs/SCOOP-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)、[编译器与产物规范](../specs/SCOOP-IMPL-SPEC.md)、[M31 性能报告](../milestone31/PERFORMANCE.md)。

## 0. 目标与取舍

Scoop 面向正常的桌面和服务器计算机。默认优化目标是实际程序的执行效率、吞吐与合理停顿；允许用适量额外内存、只读元数据和空闲空间换取更少的查表、调用、同步和重复扫描。不把资源受限设备的最小表示作为默认设计目标，也不以减少一个机器字为由保留高频路径上的线性查找。

内存换性能仍须观察整体效果：同时记录运行时间、GC 工作量、峰值与稳定 RSS、代码大小，避免局部少一条指令却显著增加分配或缓存压力。公开语义、精确 roots、原子性、异常与 FFI 契约继续成立。

M34 的完成范围如下：

| 方向 | 交付 |
| --- | --- |
| 接口表示 | 普通 interface value 携带 object 与 itab，调用直接取 slot；完成转换、布局、GC、原子引用、协程与跨 Cone ABI 迁移 |
| MIR 优化 | 函数内 receiver 实际类型传播、唯一目标去虚拟化、对本 Cone 已有正文按调用点成本选择小函数自动内联 |
| 调用与循环 | managed safepoint 的内联检查与冷慢路径；小型 GC-free 值的直接 ABI；按值 C-layout aggregate 的 DirectC |
| 堆容量 | 多 region 按需扩容、统一地址索引、region card table，移除单个 1 GiB arena 的总容量限制 |
| 移动与释放 | 选择性跨 region evacuation，空页归还与空 region 解除映射，保留 Immix 的原地 line 复用 |
| 收集并行性 | STW 并行 mark，复用标记产生的存活对象集合进行引用更新；搬迁、release hook 首版仍由 coordinator 执行 |
| 分配热路径 | 保留 TLAB，移除每次分配对多个全局统计计数器的争用，补齐精确汇总 |
| 集合存储 | 提供与 T 等大且始终可被 GC 扫描的 `MaybeUninit<T>`；ArrayList 使用普通 `MutableArray<MaybeUninit<T>>`，移除容量状态的 Option 开销 |

当前分代已经存在。M34 延续 nursery、minor、首次存活晋升旧代和 remembered set，不把这些记为新增功能。并行 mark 指 world 停止后的多个 GC worker，不引入与 mutator 同时运行的并发 mark 或并发搬迁。

本里程碑有意修改接口和小值的物理 ABI；debug/release 必须使用同一新 ABI，不能把接口宽度或调用分类变成优化模式开关。第 12 节列出明确后置的工作。

## 1. 已核对的实现基线

| M33 现状 | 本次要解决的问题 | 代码依据 |
| --- | --- | --- |
| interface 与 class 都使用单个 managed pointer；接口调用从 TD 找表 | 每次动态调用都有额外查询；当前 lookup 线性遍历接口项 | [dispatch.rs](../../compiler/lir-lower/src/function/call/dispatch.rs)、[rt.c](../../runtime/src/rt.c) |
| Direct / FinalOverride 已直接调用，其他方法主要按静态 receiver 决定派发 | 尚未利用构造、复制与 CFG 合流得到的唯一实际类型 | [calls.rs](../../compiler/mir-lower/src/body/calls.rs)、[MIR CallTarget](../../compiler/mir/src/control_flow/calls.rs) |
| 每个 callable 分别生成 LLVM module/object；release 已有 machine O2 和函数内优化 | 开启 O2 本身不能实现普通跨函数内联 | [emission.rs](../../compiler/codegen/src/emission.rs)、[policy.rs](../../compiler/codegen/src/statepoint/policy.rs) |
| 每个 managed 入口和循环回边都发射完整 runtime poll 调用 | 无 GC 请求的紧循环仍付出调用和 anchor 建立成本 | [memory.rs](../../compiler/codegen/src/function/memory.rs)、[collection.c](../../runtime/src/thread/collection.c) |
| 非空普通 struct、tuple、tagged enum 一律 indirect | 小值跨调用会产生不必要的临时存储与装载 | 实现规范 2.4 |
| DirectC 仅覆盖无 errno 捕获的固定参数标量签名 | 按值 C-layout struct 仍经过独立 storage bridge | [c_bridge.rs](../../compiler/codegen/src/c_bridge.rs)、实现规范 2.4～2.5 |
| 单次映射 1 GiB；32 KiB block、128 B line、512 B card；默认 1 MiB nursery | 总堆不能继续增长，空 block 的回收不等于归还 OS | [heap_internal.h](../../runtime/src/gc/heap_internal.h)、[heap.c](../../runtime/src/gc/heap.c)、[reclamation.c](../../runtime/src/gc/reclamation.c) |
| 编译器内联使用单个预偏置 card-table 指针 | 多 region 必须同时改变 generated code 的地址寻址合同 | [memory.rs](../../compiler/codegen/src/function/memory.rs) |
| 全局 mark worklist；mark bit、line_live 和 block 计数非原子；full 更新阶段重新追踪图 | 不能只创建多个线程调用现有 marker；标记之后还有串行扫描成本 | [collector.c](../../runtime/src/gc/collector.c)、[heap_objects.c](../../runtime/src/gc/heap_objects.c) |
| TLAB 快路径仍经 GC-leaf helper 登记对象，并更新多个全局原子计数器 | 多 mutator 分配竞争统计 cache line | [allocation.c](../../runtime/src/gc/allocation.c)、[statistics.c](../../runtime/src/gc/statistics.c) |
| ArrayList backing 为 MutableArray<Option<T>> | 对 Int 等无 niche 的值增加 tag、padding 与分支；这是包装布局成本，并非逐元素装箱 | [array_list.scoop](../../sysroot/lib/scoop.core/src/array_list.scoop) |

现有普通函数值仍为 closure 对象引用，typed invoke 使用原签名；经 Any 等运行期转换选择的动态函数适配才需要 erased Any 边界。M34 不把所有 lambda 调用误归为动态装箱路径。

## 2. 携带 itab 的接口值

### 2.1 表示与 identity

受支持的三个 64-bit target 统一使用以下内存表示，size 16、alignment 8：

```text
InterfaceValue<I> {
    object: managed pointer,   // offset 0, LLVM AS1
    itab: metadata pointer,    // offset 8, LLVM AS0
}
```

这里的 I 是静态、完整的 interface application。itab 直接保存该对象实际 TD 中与 I 对应的只读 slot table 首址，复用现有 `ScoopItableEntryV1.slots`；它不指向 managed 对象，也不需要每个接口值分配一张新表。不同 generic application 和变型 bridge 仍使用各自已经解析的表。

class、object、Any、String、Array 和普通函数值继续使用一个 managed pointer。对象头保持 16 bytes；值类型转 interface 仍按既有语义装箱，不借此次优化把任意值塞进接口的两个机器字。

同一对象的不同接口视图具有相同引用 identity。`===` / `!==`、pin 和 handle 针对 object 分量，不比较 itab。复制接口值复制两个分量，不复制对象。接口仍是普通 reference type，不新增源码类型类别或转换语法。

空接口也使用同一双字表示，itab 可沿用其合法的空 slot table，即 null；是否为有效引用由 object 判断。`Option<I>` 及现有同构 niche enum 与 I 同为 16 bytes：None 为 `{null, null}`，Some 保存完整接口值；不能仅测试 itab。嵌套 Option 的区分和 tagged enum 的全零 inactive 存储规则继续成立。

### 2.2 构造、转换与动态调用

| 操作 | 所需行为 |
| --- | --- |
| 确定实际类型的对象转 I | 直接引用已有的该类型／I 的 itab；不执行运行期搜索 |
| 未知实际类型的 class/Any 转 I | 根据对象实际 TD 取得 I 的表一次，组成接口值 |
| I 转相同 exact I | 复制原接口值 |
| I 转父接口 J、其他合法接口或变型视图 | 为目标 exact J 取得相应表；不能仅复制 I 的 itab |
| I 转 Any 或合法 class | 保留 object，按语言要求完成类型检查 |
| `is` / `as` / `as?` | 保留原类型关系与失败行为；转换成功后产生完整目标表示；检查与表查询可合并，不能查两次同一事实 |
| 接口普通参数、结果、字段、数组元素和捕获 | 保存／传递完整双字值 |
| 接口方法调用 | 从已有 itab 读取已确定 slot 的函数地址，不再从 object TD 搜索接口表 |

未知转换可以继续使用现有 TD 接口项查询，首版不另建全局转换缓存。查询以匹配的 entry 是否存在判断成功；空接口的合法 null slot table 不能误判为转换失败。M34 的承诺是每次接口调用不查表；反复从 Any 重建接口视图的程序仍可能支付转换成本。

**接口值的 ABI 与 dispatch slot 的 receiver ABI 分开规定。** 普通参数／返回中的 I 使用双字值；动态接口调用用 itab 选择入口后，slot 的隐藏 receiver 仍传 object 分量，其余参数和结果按新 Scoop ABI 传递。这样，签名与 effect 兼容的 class 实现仍能直接作为表项，不为每个普通实现多加一次转发调用。

需要完整接口 this 的 default body、变型适配和装箱值使用实际 typed adapter。adapter 属于确定的 concrete type／interface 表项，可以用对应的静态表重建 `{object, itab}`，再按 default body 的逻辑签名调用；装箱值 adapter 仍按值复制 payload。NoGc value implementation 对 Managed slot 的适配保留入口 poll 与必要 roots，不通过强制转换抹去 effect。

LIR 明确记录这种 receiver projection 与完整的入口签名；不能因底层都含 pointer 就把普通双字接口参数、单字 class receiver 和 slot receiver 当成同一 ABI。直接调用 default/helper 或去虚拟化后的入口，也必须使用该 callee 自身的已确定签名。

### 2.3 GC、原子引用与系统边界

GC scan 只记录 offset 0 的 object leaf；itab 不进入 root set、不标记、不搬迁。接口落在 struct、tagged enum、数组、closure、Context、异常 payload、native root region 或 coroutine frame 内时，仍通过普通递归 scan 描述这一 leaf。搬迁只更新 object，原 itab 保持有效；TD 与表的生命周期由现有 image/ODR 合同保证。

LLVM 中必须保留 AS1 与 Metadata provenance 的区别，不能把整个接口值转成两个整数再依赖保守扫描。跨 safepoint 的 SSA 接口值从更新后的 object 和原 metadata 分量重建；内存中的双字接口按所属 place 原地更新 object。引用写屏障覆盖实际写入的 managed leaf／含引用范围。

Context 的内部擦除 binding 槽也保留单字 object，使用独立 generated identity 的普通 managed-pointer 表示。接口 scope 保存 object，以 exact static key 取回后 NoGC 重建相应 itab；不把该内部槽伪装为源码空接口。

`AtomicRef<I>` 的隐藏原子槽仍只保存一个 object pointer。store/CAS 提取 object；load、exchange、compareAndExchange 得到 object 后，按静态 I 取得 itab 并构造返回视图，整个重建为 NoGC 操作。CAS 继续按对象身份比较。不能使用两个独立原子字段表示一个接口，也不要求所有 target 支持 128-bit CAS。普通接口读写仍遵守语言的数据竞争规则。

GcHandle 的保活对象、显式 pin 与 scoped pin frame 同样只保存 object；解析为接口值时重新取得目标表。Scoop ABI native 参数／结果则遵守完整接口 ABI，native 跨 safepoint 保存接口时登记 object slot 或整个递归 value region，并在之后重读。C ABI 仍禁止 managed interface。

必须覆盖 hidden `Continuation<R>` 等接口参数、协程保存和恢复、绑定方法引用、函数变型 adapter、Any 动态 invoke 的接口参数／返回转换。不能只修正源码中显式写出的接口变量。

## 3. MIR 去虚拟化与小函数内联

### 3.1 已知实际类型与唯一目标

优化发生在完整 concrete MIR 上。规范中的 exact type 是静态类型身份，不自动等于对象唯一的动态类型；open class 参数和一次 `is Base` 检查都不能单独证明对象就是 Base。

第一批分析采用函数内数据流：不可达、未知、已知实际类型；构造、已知 box/closure 和 final class 提供事实，局部复制和合法引用转换传播事实，分支合流只有类型一致时保留，循环求不动点。对可变字段、未知返回值、取址后的可变 local 保守处理；不能跨未知写入保留过期的内存事实。

构造事实来自最派生对象的实际 allocation TD 或完整构造结果；base/this initializer 不分配新对象，不能把其 receiver 的运行时类型收窄成 initializer 所属的基类。构造期间的虚方法调用仍按实际最派生 TD 选择目标。

对 virtual/interface call，按实际类型和原 typed slot 取得既有表项，把调用种类与 callee 一起改为 Direct，同时完成 receiver 适配。不得重新按函数名解析，也不能仅修改 CallKind 而保留接口声明／基类声明作为 callee。

唯一实际类型是首版必须覆盖的条件。对于分析已经能完整列举的有限候选，若所有候选映射到同一 callable 且 receiver 适配一致，也可以直接调用；无法证明完整性的集合退回动态派发。单例候选传播是必要完成门，多类型集合精化不阻塞该批交付。

当前 Cone 只出现一个实现，不构成全程序唯一性的证据。库消费者仍可新增子类或实现；分离编译时只使用调用点事实及已发布的 final/继承合同。跨 Cone 已知类型的直接调用可以引用现有 ExternalCallableUse，无需取得其实现正文。

装箱方法可以先直接调用原表项 adapter；devirtualization 不自动消除 box 或绕过按值 this。去虚拟化不意味着 NoGC、nounwind 或非 suspend，原求值顺序、异常边、Context 与 continuation 契约均须保持。

### 3.2 自动内联的首版范围与调用点策略

内联按调用点决策。同一个函数可以在循环内或传入常量的调用点被内联，在其他调用点继续保留调用；不以源码行数或一个全局的“函数够小”标记替代判断。这里的正文成本与第 5 节小值 ABI 的字节大小分别衡量不同问题，不共用阈值。

#### 3.2.1 正文可用性与正确性

在 MIR 层处理目标已确定、当前 Cone 已有完整 concrete 正文的普通 callable，包括本地单态化实例、从依赖的泛型正文在本地物化的实例及符合条件的适配函数。范围按正文是否可用确定，不按声明最初属于哪个 Cone 决定。这样可以在保持现有每 callable 一个 LLVM object 的组织下，获得 getter、小 helper、具体接口方法和泛型组合中的收益。

以非递归调用为首版对象；属于递归 SCC 的 callee、未取得正文的外部 Strong、native 边界、callback/gateway 和特殊协议入口保留调用。一般跨 Cone 普通非泛型正文导入、递归的有限展开、协程 frame elision 和 managed allocation 消除留待后续。

内联必须重新分配 caller 内的 local、value、block、临时存储及后续 site 身份，重接 return 与异常出口，保留按值副本、SourceLocation、构造发布、finally 和 Context 效果。实参仍按原求值顺序各求值一次，不能因形参未使用或分支被化简就删除实参中的副作用。不能把 callee 的物理 root/frame plan 一起复制；MIR 尚未定稿 GC sites，新增站点由后续 LIR 统一生成。

分配、可能抛异常或 Managed effect 本身不构成拒绝内联的理由，NoGC 也不意味着纯函数或不会抛异常。对首版无法完整重映射的特定 EH/Context/协程控制流保留原调用，不产生不完整输出；支持范围由实际的控制流和效果重写能力决定。

#### 3.2.2 调用点收益与成本

使用简单的 MIR 加权成本估计正文，标量运算、字段访问、aggregate 复制、分配、调用与异常控制流不视为等价操作。该估计用于比较优化机会，不预测精确 CPU 周期，也不复刻完整 LLVM 成本模型。

优先估计代入已知实参、传播常量及删除已证明不可达分支之后的成本。例如 `calculate(x, checked)` 中即使检查分支很长，调用 `calculate(x, false)` 仍可能只留下 `x + 1`，应有机会按化简后的小正文内联。首版只做能静态证明的局部化简，保留必须求值的表达式与副作用，不执行函数效果或为试探收益反复运行完整优化管线。

| 调用点事实 | 策略中的作用 |
| --- | --- |
| getter、简单字段运算、很短的转发 wrapper | 调用成本占比较高，优先消除调用与转发 |
| 调用位于循环中 | 用循环结构提高收益估计；没有 PGO 时不声称已测得实际热度 |
| 常量实参、已知 receiver 或函数实参 | 计入能证明的分支折叠、去虚拟化或直接调用机会；不能消除的动态调用继续计入成本 |
| 小型值参数和结果 | 考虑按值复制、临时存储可被后续局部优化消除的机会，不放松独立 place 与别名语义 |
| caller 已较大、调用点多或存在多层 wrapper | 计入累计代码增长及指令缓存压力，避免逐点看起来划算却整体膨胀 |
| 大量 live values、复杂 CFG/EH 或较大的错误处理分支 | 提高成本估计，关注寄存器压力与 spill；不能删去的分支仍计入代码大小 |

首版用局部静态信息和少量明确的成本类别完成判断，不引入全程序热度推断、通用分析框架或新的函数效果系统。

#### 3.2.3 三档策略与膨胀控制

| 档位 | 首版策略 | 初始调参量级 |
| --- | --- | --- |
| 极小正文 | 满足正确性条件后积极内联，例如 getter、简单算术和短 wrapper；仍受 caller 累计增长限制 | 可从相当于 8～12 个简单 MIR 操作的阈值试起 |
| 普通小正文 | 结合化简后成本、循环位置、常量实参、后续直接调用机会和累计增长选择 | 基础阈值可从相当于 30～50 个简单 MIR 操作试起，再按调用点调整 |
| 较大或复杂正文 | 默认保留调用；若常量化简后已落入前两档，按化简结果重新判断 | 不设置一个仅凭调用频繁就无限放宽的阈值 |

这些数值是实现时的初始实验区间，不是已测得的最佳参数，也不是每个 MIR 节点权重相同的计数；与 LLVM 自身内联阈值没有数值对应关系。默认参数由第 11.3 节的运行时间、代码大小、spill 与编译时间结果校准，记录实际权重和所选阈值。

除了单次内联成本，还要限制同一 caller 的累计代码增长与嵌套内联链的展开规模，极小函数也不能绕过这一限制。达到局部增长上限时保留调用；按稳定顺序选择调用点，避免正文遍历顺序偶然变化造成无意义的结果波动。这些都是优化启发式，不影响程序合法性、语言语义或产物兼容性，不建立跨阶段的通用资源预算。

debug 可保留较少优化，ABI 与语言行为不变。首版不要求增加源码属性或公开新的调参接口；复用现有优化配置与测试入口提供内联开启／关闭的对照。

#### 3.2.4 执行顺序

建议顺序：

```text
LocalConcrete HIR
  -> 完整 MIR：CFG、dispatch、closure/adapter、协程执行 ABI
  -> receiver 类型传播与去虚拟化
  -> 按调用点成本选择内联，控制累计代码增长
  -> 局部常量/CFG 清理，再做一次 receiver 传播与去虚拟化
  -> LIR 布局/调用 ABI/poll/roots
  -> LLVM 函数内优化、最终 GC 发射计划、RS4GC、machine code
```

首版在 mir-lower 输出完整 MIR 后以独立模块执行，优化配置由 driver 明确传入。scoop-mir 仍只定义数据；不为这两项优化建立新的通用 IR、pass 插件或全程序分析框架。program-link 继续只消费产物，不承担隐藏重编译。

## 4. 轻量 safepoint

managed 入口和每条实际循环回边继续提供 poll 机会；把正常路径改为内联读取 GC 请求状态、条件分支，只有慢路径才调用现有 runtime 协调入口、发布 anchor、完成 park 和 relocation。

首版复用 world phase、GC epoch 与本线程 observed epoch，使用明确定义的原子读取；不能让普通 load、readonly 标注或循环优化把检查提到循环外。正常路径不获取 world lock、不广播、不建立和拆除 runtime anchor，也不为该 poll 无条件物化显式 root 临时存储。确切的 TLS/epoch 读取与 target 指令在 LLVM 22.1 上验证，属于 generated-code/runtime ABI。

`MANAGED_PENDING` 的首次入口必须经有锁激活，再允许执行 managed body；不能因为 world 为 RUNNING 就跳过该动作。NativeSafe 返回、callback 进入与 NativeBorrowed runtime 入口继续使用 M33 的握手，不用轻量 poll 替换它们。

LIR 表达条件 poll、慢路径 site 与完整 live set。只有实际可能停顿的慢路径发射 statepoint/stackmap，合流后的引用选择慢路径更新值或快路径原值。若 ABI spill 无法避免则如实记录，不以“零 spill”作为未经验证的承诺。无限纯计算循环仍须响应另一个 mutator 的 GC 请求。

## 5. 小值 ABI 与 DirectC aggregate

### 5.1 Scoop 小值与接口参数

LIR 在完整逻辑签名之外保存物理 parts、来源 offset、provenance、alignment 与返回约定。直接多分量值不能被塞入现有单 scalar 分支；每个 caller、callee、函数值 invoke、dispatch adapter、Scoop ABI extern 和跨 Cone 签名使用同一计划。

接口及其 niche enum 是必须支持的直接双字值：普通参数按 object、itab 展开，结果为对应双分量直接结果；object 保留 AS1。隐藏 slot receiver 使用 2.2 的单字 projection，不套用普通接口参数展开。

首版小值覆盖 size 不超过 16 bytes、alignment 不超过 8 的非 ZST GC-free 普通 struct、tuple、tagged enum；更大或含 managed leaf 的一般 aggregate 继续使用既有 Indirect，接口值作为已定义的专门直接表示除外。小值的 source layout、完整复制和地址可观察语义不变。

struct/tuple 根据 exact size、alignment、字段 offset 和 scalar leaves 构造物理分类输入，复用 5.2 的目标 aggregate 分类工具，保留整数、浮点和混合寄存器类别；不能为了实现方便把所有浮点小值都强制放入整数寄存器。tagged enum 按固定 tag 和 GC-free payload 存储区域分类，共享 payload 作为整数 byte 区域处理，分类不能随运行期 variant 改变。分类产生可直接传递的值时采用 DirectParts，要求 MEMORY/间接传递时保留其实际约定；每个 logical aggregate 的寄存器分配和回退作为整体确定。

物理 parts 可含目标需要的 scalar/coercion carrier，并明确每一部分覆盖的存储范围；尾部填充使用确定值，读取不得越过 exact storage。重建保留浮点字段原始 bits，不引入浮点重算或 fast-math。这里复用的是布局到机器签名的工具，不会把普通 Scoop 类型标记为 C-FFI-safe；超过首版小值范围的 Scoop aggregate 也不会仅因 C ABI 支持 HFA 就自动改变表示。

callee 只在操作需要 place 时重建独立存储，不能把 caller 的 storage 当成按值参数的可观察别名。ZST 仍求值且不传 payload；异常记录与一般含引用 aggregate 保持各自完整根和返回规则。Scoop ABI native shim 根据实际 carrier 签名实现，不把源码同形 C struct 当成自动兼容。

### 5.2 DirectC 适用，但必须按 C ABI 分类

M33 已将固定参数的标量 C extern 直接发射为 native symbol 调用；M34 将无 errno 捕获的合法按值 C-layout struct 参数／结果纳入 DirectC。小 aggregate 的寄存器传递是重要收益，但 DirectC 也可以使用目标要求的栈参数、byval 或隐藏 sret，不以“全部走寄存器”为定义。

不能仅因 Scoop 和 C 类型同样“小”，就把某个 Scoop physical signature 套用于 C 边界。C classifier 只消费已有 C-FFI-safe projection、C layout 与完整函数签名：

- Linux/amd64 GNU 与 musl 按 SysV AMD64 的 eightbyte 分类、INTEGER/SSE 与 MEMORY 等实际规则处理；寄存器不足时按整个 aggregate 的 ABI 规则分配，不能逐字段任意 spill。
- Darwin/AArch64 按该 target 的 AAPCS64/Darwin 规则处理，包括 HFA、普通 aggregate、栈参数及间接结果；四个 Double 的 HFA 即使超过 16 bytes，也不能简单套用 Scoop 小值阈值。
- 零扩展、符号扩展、coercion、padding、alignment、nested struct 和隐藏参数顺序必须进入完整物理签名。普通 Scoop struct 不因此取得 C-FFI-safe 资格；managed interface 也不能经过这一入口。

在现有 C ABI projection、LIR call plan 与 codegen 中实现上述两套目标 ABI 分类，GNU/musl 共享 SysV 规则，复用布局／scalar 分类工具；不建立通用 FFI 插件或运行期 libffi 调用路径。以所选 target C compiler 编译的真实函数作为互调基准，不能只用编译器自己生成的 caller/callee 相互证明正确。

DirectC 继续与 NativeSafe/GCLeaf 两轴独立：普通 C extern 保留 roots 与线程握手，GCLeaf 保持 M33 的无回调／无阻塞／无 safepoint 契约。`captureErrno = true` 首版继续用既有 StorageBridge；callback、FunPtr 与 native global/TLS 的 ABI 不在本批顺带扩大。合法 C-layout aggregate 的普通正向 extern 调用不能靠错误诊断回避分类。

## 6. 可扩展的 region 堆

### 6.1 Region、block、line 与大对象

```text
Heap
  -> 普通 Region：独立 VM mapping + side metadata + card table
       -> 32 KiB Block
            -> 128 B Line
  -> 大对象 mapping：按实际大小覆盖一个或多个地址索引单元
```

Region 是 VM 映射、扩容、集中搬迁与归还 OS 的单位；block/line 继续承担 Immix 分配与碎片复用。普通 region 首版以 16 MiB 为起始参数，大小为 block 与 OS page 的公倍数；这是可调运行时策略，不是对象大小、总堆大小或 ABI 中的语义上限。

移除全堆 `GC_BLOCK_COUNT`、单一 base/end 和固定 1 GiB 大小假设。block、free span/run、TLAB、forwarding 和 nursery 分配记录都携带所属 region；全局 block 身份不能由会复用的裸局部 index 代替。metadata 保存在 managed storage 外，不能因空页归还而丢失仍需要的对象起点、大小或卡片信息。

先复用可用 block/line 和保留的空 region，再在 slow path 按需申请 region。正常分配阈值仍决定何时 GC，不能每次请求 region 都盲目 full，也不能因 nursery refill 竞争次数达到某值就报告 OOM。超过旧 1 GiB 总容量后仍能继续分配；实际地址计算和映射失败走明确出口。

大于普通 block 能力的对象使用独立、按实际大小对齐的 mapping/span，容量可以超过普通 region。精确大小与数组长度检查延续原合同；不为了 region 固定大小新增单对象上限。对象在未移动时也可于死亡后独立解除映射。

映射记录保留原始 mapping base/length 以及对齐后的可用范围。若通过额外预留取得对齐，要释放首尾多余页或在最终释放时包含它们，不能遗失原始映射。

### 6.2 地址索引与写屏障

使用按地址 chunk 索引的稀疏 radix page map，将任意 heap object/card 地址映射到 region 或大对象 metadata；普通 block 定位再由 region 内 offset 得出。层级覆盖 target 的有效 uintptr_t 地址范围，按实际映射分配节点，不分配覆盖整个虚拟地址空间的平坦表，不在线性 region 列表上执行每次 mark 或 store。

增加 region 时先建立 metadata 和 card storage，再 release 发布地址索引；无锁读取使用对应的 acquire 合同。撤销映射及索引只在 STW 且全部 GC worker 已结束、TLAB 和相关缓存已失效之后进行。generated code 不跨 safepoint 保存可被回收的 region metadata 地址。

每个 region/large span 有按实际地址覆盖的 512-byte card table。单槽屏障从地址索引取得 metadata，计算局部 card，并原子置脏；范围屏障覆盖整个写入范围，相交的索引单元逐段处理。两者有限时间、NoGC、无分配、无 park。

替换 `scoop_gc_card_table + (address >> 9)` 的单 arena 预偏置合同；codegen、runtime/native stores、Context、clone、aggregate copy 和 AtomicRef 同步迁移。最终默认路径须具有内联的单槽寻址和标卡，不能把正常 heap store 退化为 region 线性查找或带锁查询。标量／GC-free store 继续省略屏障。

### 6.3 选择性跨区搬迁

full 标记完成后，优先选择能腾出整个 region 的低存活、无 pin 区域作为 source；把对象集中复制到未选为 source 的目标区域。目标可以复用已有密集区域的空间或使用保留／新增 region，但不能把对象重新放回本轮准备释放的 source。

高存活区、含 pin 的区继续支持原地 Immix line 复用；含 pin 的 region 仍可释放完全空闲的其他页。M34 不采用每轮复制整个 live heap 的固定双半空间，也不强制所有 full 都为证明 moving 而复制对象。普通 full 按收益决定是否 evacuation；显式 moving stress 单独要求发生 eligible movement。

搬迁前预留本批全部目标空间，包括对齐损耗。预留失败时缩小 source 集合或执行非移动回收；minor 晋升不足仍从尚未破坏的完整原图转 full。不得在部分 root 已改写后当作“没有搬迁”继续运行。

复制与 forwarding 建立后，更新全部精确 roots、存活对象出站引用和 runtime 引用。跨 region 搬迁不会免除引用更新，也不让 minor 的 old→young cards 自动成为 full 的完整入边索引。接口只更新 object 分量。完成更新前不覆盖、discard 或 unmap source。

### 6.4 内存归还 OS

复用现有 ThreadVmOps 平台边界增加 discard 与 release mapping 能力，分别在 Darwin 与 Linux 实现。区分两层回收：

- region 内整页已无存活对象时，可以 discard 物理页，保留地址及必要 side metadata；任何跨页活对象覆盖的页都不能 discard。
- region 完全空闲且无 pin、TLAB、forwarding、worker 或 runtime 引用时，从分配结构和地址索引移除，再解除整个 mapping；不把仅加入 free list 计为 OS 归还。

discard 后的内容不能作为仍有效的对象状态读取；重新使用时按对象初始化合同清零。不同 OS 的建议性 discard 不保证立即减少某一 RSS 采样，报告必须分别记录调用成功、discarded bytes、unmapped bytes 与实际 RSS。瞬时峰值 RSS 不会因之后释放而回落，不能拿峰值计数验证当前占用。

首版保留少量空 region 供 nursery refill 和 evacuation，普通空闲缓存初值为一个 region；其余在 full 后释放。根据增长／收缩循环实测调整保留策略，避免每轮释放后立即重新映射。pin 只限制其对象所在存储，不阻止其他空 region 归还。

## 7. STW 并行 mark 与存活集合复用

### 7.1 Worker 与阶段边界

复用 M33 的单 coordinator 取得 collector 独占权和 world 停稳协议。停稳后才启动多个 GC worker；worker 不作为 mutator attachment，不执行用户 managed code，不参与 NativeSafe 返回或 callback 生命周期。使用 runtime 内部常驻 pthread worker，退出时由 coordinator 停止并 join。

worker 数依据可用 CPU 和工作量选择；小 minor 默认单 worker，大图允许 coordinator 与 worker 一起标记。首版评估 1/2/4/8 worker，不承诺任意图都随核心数线性加速。队列使用简单的本地分批任务和带锁 work stealing 即可，不要求无锁 deque。

collector 对 region 集合和 roots 的稳定视图承担独占所有权，worker 只能访问本轮已发布数据；不能让每个 worker 对每个对象重取同一 heap lock。新增 reader/marker 操作明确其阶段前提，不把原来隐含“只有一个调用者”的 `_locked` 函数直接多线程复用。

### 7.2 标记的并发正确性

对象 mark bitmap 使用原子 test-and-set；只有首次成功者登记对象并产生扫描任务。large object 的标记同样是原子的。共享子图、环和多 root 不能重复增加存活统计。

line_live 使用原子位操作，或由 worker 独占的 block 归约阶段统一产生；首版选择原子 OR 配合 worker-local 计数归并。block live bytes、movable bytes、扫描计数和存活对象清单不再由多个线程修改原来的非原子字段。任务发布／获取使用正确同步，heap 不变不等于队列和 side metadata 可以数据竞争。

大引用数组按元素区间拆任务，嵌套 scan 仍使用现有精确描述；一个数组不能独占单个 worker 扫完整片大图。普通小对象按批次处理，GC-free 数组没有引用扫描任务。长链等缺少图并行性的工作负载如实报告。

终止判定覆盖所有队列、正在扫描的任务与尚在发布的子任务。子任务先记入未完成工作再发布，父任务只有在子任务全部发布后才完成；协调者确认全局无在途任务才进入下一阶段。不能以自己的队列为空或一次窃取失败判定完成。

minor 仍只依据完整根、脏旧区与年轻图工作，不借并行化遍历整个旧代弥补屏障。pin 固定在移动规划前生效；标记期间不改写用户对象引用。

### 7.3 后续阶段与失败处理

保留每个 worker 首次标记得到的存活对象列表。完成 mark 和归并后建立移动计划；首版串行复制，再按列表对每个存活对象当前副本扫描一次并更新出站引用，同时更新所有 roots 和稳定外部 payload。

由此去掉当前 full 在更新阶段重新发现整个可达图的 worklist 遍历；仍然必须扫描需要修正的引用槽，不能把“复用存活集合”写成“无需第二次访问引用”。minor 只消费对应年轻集合和 remembered set。

worker 全部结束前，coordinator 不得移动、释放、清空卡片或销毁 metadata。release hook 继续由 coordinator 按原来的逻辑死亡／ready 规则执行，旧副本退休不重复执行 hook。首版不并行用户 release hook，也不并行 evacuation 或引用回写。

worker 创建失败在标记开始前退回一个 worker；任务队列扩容失败不能丢任务，进入现有明确的 runtime 分配失败出口。保持一份参数化的串行／并行实现，worker=1 不另建一套 collector。

## 8. 分配统计与 nursery 热路径

TLAB fast path 保留对象清零、有效 TD、精确大小与对象起点登记；这些事实在 GC 可观察对象前必须成立。先消除共享统计争用，不以省略必需登记制造分配收益。

分配对象数、nursery 对象数与累计分配 bytes 改为线程局部累计；GC 内统计为 worker-local，阶段结束时归并。普通分配不再对数个全局 cache line 执行 atomic RMW。

现有 `scoop_rt_gc_stats`、minor 旧代对象计数和 debug metrics 的语义必须保留。首版使用 cache-line 分隔的每线程累计计数：本线程维护数值并以 relaxed atomic store 发布，诊断用 atomic load 求和；这些单写者计数不需要 atomic fetch-add。线程 detach 时在 registry 生命周期保护下将其累计值并入已退出线程总账，再移除登记，避免漏记和双记。

collection 在 STW 下记录累计分配快照与回收后的对象数；之后的当前对象数由该基线加新增分配推导，minor 同时保留两代的正确计数。查询与 collection 基线更新、detach 总账归并按已有锁顺序同步，不能读到半次归并。不能无同步读取运行线程的普通局部变量，也不能把当前对象数静默改成“上次 GC 的存活数”。统计开关只控制额外诊断，不改变分配／回收正确性所需计数。

保留首次存活晋升，无 survivor 或自适应代龄。nursery 容量与 region 保留量作为独立策略参数测量；没有证据时不把 nursery 大小绑定为整个 region。指标至少区分分配、晋升、复制、脏卡、full fallback，以及每轮 collection 的实际 worker 数。

## 9. MaybeUninit 与 ArrayList 的紧凑容量存储

### 9.1 类型表示与初始化规则

在 core 中增加公开、invariant、无 kind bound 的 intrinsic value type `MaybeUninit<T>`，通过真实声明和封闭 intrinsic 登记提供表示与基本操作。它具有独立的完整类型身份，按普通值语义复制，可用于局部变量、字段、数组和泛型正文。M34 的实际消费方为 ArrayList，StringBuilder 通过持有 `ArrayList<String>` 复用其存储与扩容；编译器只处理该存储原语，不按 ArrayList 的类型名识别特殊布局。

`MaybeUninit<T>` 的 size、alignment 与 T 相同，直接内联保存 T 的存储，不增加 tag、初始化位或额外对象。T 为引用类型时，wrapper 仍是值类型；T 为第 2 节的 interface 时，wrapper 保存相同的 16-byte 存储。T 为 ZST 时 wrapper 同样为 ZST，并保留普通求值规则。

首版 `uninit()` 将整个存储清零，包括引用槽和 padding。这是一个合法的 `MaybeUninit<T>`，不表示已经构造合法的 T；例如零引用不能作为 String 使用。`uninit()` 不调用 T 的构造器、不要求 Default，也不使用 LLVM undef/poison 表示尚未写入的内容。这个原语不允许在 managed 引用槽中保留任意未初始化字节。

### 9.2 最小操作与编译器契约

| 操作 | 语义与调用条件 |
| --- | --- |
| `MaybeUninit<T>.uninit()` | safe；产生全零 wrapper |
| `MaybeUninit<T>.initialized(value: T)` | safe；实参按普通规则求值一次，保存完整合法的 T |
| `value.assumeInit(): T` | 标记 `@Unsafe`；调用者必须保证该 wrapper 已包含合法 T，按 Scoop 值语义复制返回，不移动或清空原 wrapper |
| 将变量、字段或数组槽赋值为新的 wrapper | 使用普通赋值和数组 setter；写入 `uninit()` 即重置存储并清除原引用，无独立 reset/write intrinsic |

只有前三项需要新的 typed intrinsic。`assumeInit()` 使用现有 unsafe context 检查；在未满足前提时调用属于 unsafe 契约违反，不提供运行期初始化检查或新异常。类型中没有初始化标记，不能通过测试全零实现 `isInitialized()`；合法 T 的表示也可能全零，初始化事实由使用者的不变量保证。

三个 intrinsic 本身只进行零化、复制和受前提约束的取值，不分配、不回调用户代码。实参求值中的分配、异常与 safepoint 仍保留；含引用 application 也不能因此绕过既有 NoGC 的类型与 effect 条件。

wrapper 不隐式转换为 T，不从 T 自动派生字段访问、相等或其他读取 payload 的操作。初始化后再取出仍使用原 T 的语义。`Option<MaybeUninit<T>>` 不继承 T 的零值 niche，按普通 tagged enum 处理；`MaybeUninit<Option<T>>` 则与该 Option<T> 的存储等大，已初始化的 None 仍是合法 payload。嵌套 wrapper 逐层遵守同一规则，不添加隐藏状态。

MIR/LIR 保留 wrapper 的完整类型、零构造、合法值包装和 unsafe 取值语义；存储相同不允许提前抹掉 wrapper 的有效值规则。尤其不能对 wrapper 套用 T 的 nonnull、有效 enum tag 或 niche 假设。布局相同也不自动意味着同一调用 ABI 或 C-FFI-safe：按 wrapper 自身的值类型分类和第 5 节规则生成签名，普通含引用 aggregate 继续遵守既有间接 ABI。泛型和跨 Cone 产物保存同一表示与操作契约，不另建初始化状态证明或全程序分析。

### 9.3 GC 扫描与普通数组复用

`MaybeUninit<T>` 复用 T 的 value/inline scan，GC-free 当且仅当 T 为 GC-free。合法 wrapper 的 managed 引用槽始终为 null 或有效引用；GC 正常标记、搬迁和回写非零引用。接口的 itab 仍是 metadata，不参与扫描。字段、数组、closure、native region 和跨 safepoint 临时值沿普通根与写屏障通路处理，不因“可能未初始化”就省略 roots。

这一方案依赖现有的固定引用槽扫描：null 槽直接跳过，tagged enum 的扫描不读取 tag。全零安全指正常对象头和正确扫描描述覆盖下的 value 存储，不允许把对象头或 TD 一起抹零。写入、复制、重置 wrapper 时，按现有值存储规则保证 safepoint 处每个引用槽有效；不向 GC 暴露半写的指针，也不需要 GC 查询初始化状态。

ArrayList backing 改为普通 `MutableArray<MaybeUninit<T>>`，数组的 logical size 就是 capacity；ArrayList 自己的 `elementCount` 表示有效 T 的数量。初始化形式为 `MutableArray<MaybeUninit<T>>(capacity) { _ -> MaybeUninit<T>.uninit() }`。每个数组元素都是合法 wrapper，普通数组“所有逻辑元素已初始化”的规则继续成立。

复用现有 InlineArray shape：logical size 位于 offset 16，元素起点为 `alignUp(24, element_alignment)`，非 ZST stride 为 T 的 exact size。分配范围与完整数组扫描都按 capacity 确定；不增加独立 InlineBuffer shape、供 GC 使用的 initialized length 或容器专用 runtime 操作。minor 继续使用现有 remembered set／dirty-card 范围；不能把这一点表述为按 ArrayList.elementCount 扫描。

普通数组的逐索引 initializer、异常和副作用规则保持。对已证明冗余的全零写入可复用现有清零和局部优化，测量中记录实际初始化成本，不为 ArrayList 增加绕过一般初始化语义的分配入口。

### 9.4 ArrayList 迁移与 StringBuilder 复用

ArrayList 继续是普通 core class，其 public API、索引异常、迭代期间串行修改行为、浅复制和不装箱规则保持。库代码维护 `[0, elementCount)` 内均为已初始化 T、剩余位置均为全零 wrapper 的不变量；容量增长、边界检查、插入删除与元素计数都在普通 Scoop 代码中完成。

追加先通过 `initialized(value)` 和数组 setter 保存完整元素，再增加 elementCount；getter 在索引检查后进入局部 unsafe context 调用 `assumeInit()`。插入、删除和扩容按普通数组读写复制 wrapper；移动过程中允许暂时保活重复的合法引用。source、destination 和临时值按普通 roots 保活，任一 safepoint 都可扫描当前存储，不要求额外发布 GC 前缀。

移除元素后将退出有效范围的尾槽重置为 `uninit()`；clear 将原有效范围逐槽清零，再使 elementCount 为零。不能只减少计数而留下旧引用，因为 GC 仍会访问 backing 的全部引用槽。异常路径沿现有语言行为处理，不能让未初始化 T 成为可读列表元素。

`ArrayList<Option<T>>` 使用 `MaybeUninit<Option<T>>`，None 仍是合法列表元素；消除的是容量状态额外包的一层 Option。Int 元素按 4-byte payload 保存，interface 元素按第 2 节的 16-byte 值保存，ZST 只保留逻辑数量与求值行为。

StringBuilder 继续直接持有 `parts: ArrayList<String>`。`add(String)` 调用 `parts.add`，generic add 先完成一次 toString 再追加；容量增长、存储初始化与扩容复制统一由 ArrayList 负责。StringBuilder 不另建容量数组、元素计数或扩容算法。

`build()` 沿用 `coreStringJoinParts(parts.backing, parts.size)` 的调用组织，只适配现有拼接边界：ArrayList 的 internal backing 变为 `MutableArray<MaybeUninit<String>>`，相应修改 core 声明、runtime 元素描述与文档。`scoop_rt_string_join_parts` 仍只负责计算总字节数、分配并拼接由 ArrayList 保证已初始化的 String 前缀；分配后从更新后的 backing 重读引用，不管理列表容量，也不额外复制一份普通 String 数组。该边界使用新的元素类型身份。公开 String 的 UTF-8 表示、Unicode scalar length/index 和 StringIterator 语义不变。

### 9.5 成本与范围

本方案消除容量状态的额外 tag/padding，复用已有数组布局与扫描机制。对含引用 T，完整 GC 扫描的槽位数仍随 capacity 增长，空闲槽即使全零也有访问成本；GC-free T 没有元素引用扫描任务。第 11.3 节分别测量紧凑布局收益、初始化成本及大容量低占用时的 GC 扫描成本，不宣称获得只扫描有效前缀的收益。

首版以三个基本操作和现有赋值完成可用能力，不扩展任意 managed 原始指针、字段级部分初始化、运行期初始化位图或新的存储框架。按有效前缀扫描作为有测量依据后再考虑的独立优化，不阻塞 M34。

## 10. 规范、ABI 与实现落点

### 10.1 实施前的规范修订清单

| 规范 | 必须同步的章节与内容 |
| --- | --- |
| 语言规范 | 第 3～4 章的 ref value／MaybeUninit 值表示与复制、4.4.2 identity、7.4 的双字 niche 与 wrapper 的非 niche 规则、9.1 的语义派发与可证明去虚拟化、10.6 的合法 wrapper 数组初始化、11.6／11.10 的 StringBuilder／容量存储、11.14 的 AtomicRef identity、13.1～13.3 的 MaybeUninit intrinsic/effect/unsafe、13.8 和 14.2～14.3 的 Scoop ABI／native roots |
| 运行时规范 | 2.1～2.5 的 interface value、MaybeUninit 零存储／value scan、普通数组复用与 niche；2.8 的 ABI 版本；3.1～3.3 的分配/poll/roots；3.4 的 pin/handle；3.5 的 coordinator/worker 与 pending 激活；3.6 的 region cards；3.7 的选择性搬迁与释放；3.9 的并行 mark 和原分代合同；第 6 章的 string_join_parts backing 契约 |
| 实现规范 | 2.3 的 MIR 优化顺序、2.4～2.5 的 DirectParts/DirectC/poll 与 GC provenance、2.6～2.8 的格式/缓存/产物链接、2.9 的接口 slot receiver 与 adapter、2.10 的 AtomicRef/MaybeUninit、2.13 的表与 adapter ODR、2.15 的集合存储与普通数组初始化、2.19 的优化与最终 roots；wrapper 有效值规则贯穿 IR 与 codegen |

把“所有 ref 都是单个机器字”“所有 niche enum 都是单 scalar”“所有非空 aggregate 都 indirect”“普通 full 必须移动一个对象”等旧表述逐项改成新合同；保留 moving stress 的独立覆盖。不要在设计之外留下仍被消费者采用的矛盾 ABI 条文。

### 10.2 兼容迁移

M33 基线是 runtime ABI contract 11、metadata ABI 7。M34 目标为 runtime ABI contract 12、metadata ABI 8；即使部分 C descriptor 的字节大小不变，双字接口、slot receiver、物理参数、poll 与 heap/card generated-code 合同也不兼容。MaybeUninit 的类型表示／intrinsic 编码与 string_join_parts backing 契约一并进入该版本迁移，数组继续复用原 shape。producer、reader、runtime public headers、启动对象、target/backend 合同及缓存必须一起更新。

按实际编码修改 MIR/LIR 的表示、调用计划、native ABI、metadata profile/section version 和语义指纹；具体 field/tag 在对应批次随实现登记，不为后续优化预留空字段。类型/声明的 persistent identity 不因优化或机器表示改变；layout、scan、physical signature 与真实 ABI 变化正常使依赖失效。

重建 core、runtime 与所有受影响的源码 Cone；无源码的旧 ABI 产物明确拒绝。新 ABI 的 debug/release 可以混用，旧薄接口与新双字接口不能通过 bitcast 或默认字段混用。开发中的中间批次不声明稳定发布 ABI，也不能复用不匹配的缓存。

itable、default/boxing/variance adapter 继续属于真实 provider 或既有 ODR group；跨 Cone 直接引用表地址要形成正常 typed definition/reference 和 relocation。选中的代码、表、EH、stackmap、Context 与 registration 保持一致，不因内联或 fat interface 增加新的来源认证、正文证明或重型重复验证。

### 10.3 主要代码位置

| 工作 | 落点 |
| --- | --- |
| 接口表示与转换 | mir/lir 数据、mir-lower dispatch/boxing、lir-lower exact_layouts 与 call、codegen value/roots、runtime 类型转换与 native headers |
| 去虚拟化／内联 | mir-lower 的独立优化模块、driver 的 typed optimization 配置；不放进纯数据 IR crate |
| ABI／DirectC | LIR ABI/call plan、lir-lower signature 分类、codegen call/return、C bridge 分类与 native fixture |
| 条件 poll | lir-lower safepoints、codegen statepoint/memory、runtime thread collection/managed entry |
| region／卡表／归还 | runtime gc heap/allocation/evacuation/reclamation、platform ThreadVmOps、codegen memory barrier |
| 并行 mark | runtime collector、heap_objects、scan task、worker 队列和统计 |
| 未初始化存储与容器 | core MaybeUninit／ArrayList／StringBuilder、HIR intrinsic／unsafe 检查、MIR/LIR wrapper 表示与 value scan、codegen 零构造／包装／取值、现有数组存储与 string_join_parts 边界 |
| 兼容与测量 | 共有 metadata/profile、slib、runtime build、正式 fixture、现有 tests/benchmarks |

## 11. 批次与验收

### 11.1 实施顺序

| 批次 | 交付与出口 |
| --- | --- |
| M34-1 规范与基线 | 保存 M33 revision 和同机性能数据；先更新三份规范、ABI 分类与版本迁移计划；验证接口双分量结果／invoke／AS1 relocation 及小值／C aggregate 的最小 LLVM/C 实验 |
| M34-2 双字接口 | 完成表示、转换、slot receiver、niche、GC、AtomicRef、native、closure/coroutine 和跨 Cone 闭环；动态调用无 lookup |
| M34-3 MIR 优化 | 唯一实际类型去虚拟化、按调用点化简后成本选择的三档内联与累计增长控制、再次局部清理；MIR golden 与 GC/EH/Context 行为成立 |
| M34-4 poll 与分配热路径 | 条件 poll、pending 激活、线程局部计数与精确汇总；多 mutator 紧循环／分配运行成立 |
| M34-5 小值与 DirectC | GC-free 小值 DirectParts、Scoop ABI shim，以及 C-layout aggregate 的 target C ABI 正向直接调用 |
| M34-6 多 region | 地址索引、按需扩容、large mapping、region card table；先用串行 collector 验证新堆模型 |
| M34-7 集中搬迁与归还 | source/target 选择、完整引用修正、空页 discard、空 region unmap、失败回退与 pin 组合 |
| M34-8 并行 mark | 原子首次标记、任务分块、全局终止、worker-local 汇总、存活集合复用；1 worker 与多 worker 同义 |
| M34-9 未初始化存储与容器 | MaybeUninit 的零构造／合法值包装／unsafe 取值、普通数组扫描与 roots 闭环；ArrayList backing 迁移，StringBuilder 复用 ArrayList 并适配拼接边界，值和引用元素组合 |
| M34-10 总验收 | 三 target 的实际功能/CLI/ABI/GC/性能报告、默认策略与真实版本清单；逐项记录完成状态 |

先完成 region/card/metadata 改造，再并行化 marker，避免为单 arena 临时写一套并行元数据。各编译器批次都在 roots 定稿前完成会改变调用和控制流的变换。功能实现按批次提交，测试围绕变化范围推进，不在每个小改动后机械重跑完整工作区。

### 11.2 必须覆盖的正确性用例

| 类别 | 独立与组合覆盖 |
| --- | --- |
| 接口 | 同对象多接口 identity、class/Any/父接口转换、空接口、generic/variance、default、值类型/ZST 装箱、属性 slot、绑定方法引用 |
| 接口布局与 GC | Option 和嵌套 Option、tagged enum、数组/struct/closure/Context/native region、full/minor moving、跨 region、itab 不作为 GC root |
| 原子与协程 | AtomicRef<I> 的 load/store/exchange/CAS、两个不同实现、跨线程发布与 moving；hidden Continuation、同步完成与真实挂起 |
| MIR 优化 | 构造后 upcast、基类初始化期间的虚调用、复制链、同/异类型合流、循环赋值、final/open、override、box adapter；下游新增实现时仍正确动态派发；异常与调用次序 |
| 内联决策 | 小 helper/getter、同一 callee 在不同调用点的选择、常量实参删去大分支、多层 wrapper 与累计增长限制、递归 SCC 保留调用、本地物化的外部泛型可内联、外部无正文保留调用 |
| 内联语义 | 实参副作用及求值次序、值参数独立 place、取址／InteriorMutable、多出口与 SourceLocation、Managed 分配与 moving GC、throw/catch/finally、Context、特殊协议入口保留调用 |
| 小值 ABI | 1/8/9/16/17 bytes 边界、ZST、Float/Double bits、tuple/enum、参数寄存器耗尽、返回值、Scoop ABI native shim、跨 Cone 混合优化 |
| DirectC | C compiler 产生的参数／返回检查器；单字段、双整数、混合整数浮点、nested struct、HFA、超过寄存器范围和 sret；NativeSafe/GCLeaf 分别运行，errno bridge 回归 |
| poll | 没有分配的无限循环收到 GC 请求、多个 continue 回边、slow-path relocation 合流、pending/callback 入口、连续 GC epoch、NoGC 路径不新增 poll |
| region | 跨多个 region、实际 live heap 超过旧 1 GiB 总边界、跨 region 的链/环/数组、大对象超过普通 region、映射对齐首尾、地址重用与 stale metadata 排除 |
| evacuation | 高/低存活、多个 source 汇入目标、source 不再作为目标、pin 阻止整区释放、unpin 后释放、引用回写后 unmap、晋升／预留失败保全原图 |
| parallel mark | 共享子图、环、宽图与长链、大数组分块、空队列后新任务发布、worker 创建失败、连续 minor/full、多 mutator 与冻结 native roots |
| release 与归还 | ready/unready、构造失败、显式 close、移动不重复 hook；discard 后复用、增长收缩循环、其他 region 在 pin 存在时仍能释放 |
| MaybeUninit | 与 T 等大的 size/alignment、全零存储、合法值往返、复制后独立 place、重置引用、safe context 调用 assumeInit 的 negative fixture；Int/Float bits、引用、接口、含引用 struct/enum、Option 与嵌套 wrapper、ZST；不误用 nonnull/tag/niche 假设 |
| MaybeUninit 与 GC | 未初始化／已初始化 wrapper 的局部变量、字段、数组、closure 与 native region；minor/full moving、旧数组写入年轻引用、跨 region relocation；不把 wrapper 误判为 GC-free |
| 容器 | Int、引用、接口、含引用值、Option 元素和 ZST；扩容/插入/删除/clear、复制中途 GC、全容量扫描空闲零槽、不保活删除元素、StringBuilder 追加跨越多次 ArrayList 扩容／重复 build／继续追加及 JSON 组合 |
| 产物 | source/prebuilt、artifact-only、普通/泛型/ODR 表与 adapter、MaybeUninit 跨 Cone 实例化／传参与返回、旧 ABI 拒绝、debug/release 混用、真实 ABI 改变后的缓存失效 |

对已有语言错误及新增 MaybeUninit 的声明／unsafe 调用规则保留 negative fixture；对优化机会缺失不制造新的源码错误。assumeInit 的动态前提由调用方保证，不要求把 unsafe 契约违反变成运行期检测或编译诊断。golden 检查实际 callee、ABI、roots、scan 和必要控制流，不固定无关产物摘要。runtime 测试可用较小 region 触发边界，但不能据此冒称已经执行真实超过 1 GiB 的验收；后者在内存充足的正常主机单独运行并记录。

实现变更先执行对应格式化与 lint，再运行适当测试。正式完成门包含 Rust 工作区、公共 fixture runner 单元测试与实际文件 fixture；Darwin/AArch64、Linux/amd64 GNU 和 musl 的 debug/release、现有静态／动态链接组合分别记录范围。C aggregate ABI 必须由独立 C compiler 互调验证，GC 则由真实 moving/多线程运行验证，不能只看 LLVM verifier。

### 11.3 性能与结构性完成门

复用 tests/benchmarks 和现有 GC 诊断，补充少量真实程序及可复跑脚本，不创建通用性能平台。先保存同机 M33 基线，再记录每个完成批次；最终至少有以下对照：

| 工作负载 | 测量与必须能观察到的变化 |
| --- | --- |
| 高频接口 getter/iterator/codec | 已知 receiver 与未知 receiver 分开；转换一次调用多次和每次转换分开；动态调用路径无 itable lookup |
| 内联策略 | getter/短 wrapper、循环内调用、常量分支、大量调用点和多层展开；与相同其他优化配置下关闭 MIR 内联的结果对照，观察调用实际消失及 CFG 化简，记录运行时间、机器代码大小和编译时间，检查代表性热点的 spill |
| 小值 ABI | DirectParts 的参数/返回搬运与实际机器代码；保留未内联调用的独立测量，避免把 ABI 收益完全隐藏 |
| C aggregate call | 与相同原型的 C→C 调用及原 StorageBridge 对照；验证 native symbol 直接调用、无额外 bridge，分别统计 NativeSafe/GCLeaf |
| primitive 紧循环 | 正常回边无完整 runtime poll 调用；同时测另一个线程请求 GC 的响应，不能关闭 safepoint 得出成绩 |
| 单／多 mutator 分配 | 吞吐、全局 atomic RMW、refill、nursery/full 次数；详细统计关闭和开启都记录 |
| 大旧图／少量脏边 | minor 不随无关旧图完整线性扫描；增加 region 数不引入线性地址查询 |
| 大图 mark | 1/2/4/8 worker 的 mark 时间、总停顿与 worker CPU；宽图、大数组和长链分别报告 |
| 堆增长再收缩 | live/allocated/mapped/committed accounting、discarded/unmapped bytes、当前与峰值 RSS、映射次数、下次增长成本 |
| ArrayList 与应用组合 | MaybeUninit 元素 stride、分配 bytes、初始化／重复清零成本、遍历/修改吞吐，以及 StringBuilder/JSON 等真实组合的总收益；确认复用普通数组布局且无容量状态 tag |
| 大容量低占用列表 | 含引用元素的 capacity／elementCount、完整 GC 的引用槽访问数与扫描时间、clear 后的引用存活；与紧凑布局收益分开记录，GC-free 元素作为对照 |

GC 时间至少拆为停稳等待、root/remembered 扫描、mark、plan、copy、reference update、release/reclaim、VM 归还；记录 minor/full 次数、晋升/复制量、source/target/空 region 数和 pin 阻塞释放情况。并行 mark 的加速不能当成整个 pause 的同倍数加速。

记录 revision、CPU/OS/内存、target/toolchain、输入、运行次数、warmup、实际策略参数和全部样本；使用中位数与波动范围，GC 停顿记录足够样本后再报告 p95/p99，不从几个样本宣称稳定尾延迟。跨宿主不直接相除，容器／模拟与原生测量分列。

Scoop/Go 对照是解释性能的辅助数据，必须对齐工作量、数值宽度、生命周期与字符语义，例如 Scoop Int 对 Go int32、Unicode scalar 遍历对 rune 遍历。M34 完成不以“全面快于 Go”或预设倍数为门槛；结构性目标必须达到，关键工作负载的明显回退必须定位、修正或在默认策略中收敛，并公开记录仍有的取舍。

## 12. 明确留待后续

分代 ZGC 风格的着色指针、load barrier、并发 mark／并发搬迁；survivor、多级代龄和自适应晋升；并行 evacuation／引用回写；全程序 CHA/RTA、PGO 驱动的去虚拟化／内联、递归的有限展开、跨 Cone 普通正文导入和 LTO；通用逃逸分析、managed box/closure 分配消除、协程 frame elision；一般循环 bounds-check elimination、激进向量化；任意含引用 aggregate 的寄存器 ABI、DirectC errno 捕获和 callback ABI 扩展；按有效前缀缩小容量存储的 GC 扫描范围；String 随机字符索引缓存；调度器、平台库 API 和新 target。

这些项目不作为 M34 的隐藏前置条件。M34 以可用的新接口／调用 ABI、局部优化、可增长且能归还内存的分代 Immix、实际并行 mark 和可核对的性能报告完成。
