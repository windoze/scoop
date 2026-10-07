# M31 设计：首批优化、ODR 合并与 nursery

状态：实施中。已完成的功能、提交批次与实际验证见 [PROGRESS.md](PROGRESS.md)；本设计列出的待完成项目不视为已经通过。

日期：2026-10-07。

基线：已完成 M30 的仓库；依赖 M15 的精确根与 moving GC、M23 的跨 Cone/ODR/产物链接、M24 的 release hook、M27 的 Context、M28 的三个 target，以及 M30 的严格浮点语义。

对应[路线图](../ROADMAP.md)、[语言规范](../specs/SCOOP-SPEC.md) 12.3、12.5、14.3，[实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.19，以及[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) 2.8、3.6、3.9。历史 milestone 文档保留原文；M23-7 的正文内容 ODR 判等、M23-11 的 profile 仅影响输出布局和 M15 的单代 GC，由本设计与同步后的 spec 明确替代。

## 0. 目标与边界

M31 把已经可用的编译、产物消费、链接与运行闭环推进到可度量的优化基线，正式交付三项相互配合的能力：

1. debug/release 使用真实且可缓存的优化配置，完成首批函数内优化及必要的 Scoop 局部优化。
2. 同一 ODR 实例在普通依赖语义一致、ABI 兼容时允许使用不同优化实现；链接正确选择正文及其实现附属数据。
3. 在现有 Immix 中增加 nursery、minor GC、晋升和 remembered set，使短命对象的收集无需遍历完整旧代。

本里程碑不改变语言求值、数值、类型身份、初始化、异常与 FFI 行为。优化不能改变公开 ABI；分代不能改变 root、pin、handle 和 release hook 契约。具体编译时间、代码大小、吞吐与停顿的收益通过实测记录，不预先承诺倍数。

| 决策 | M31 范围 |
| --- | --- |
| Scoop release | LLVM machine O2 + 经过 GC/EH 验证的函数内 IR pass 组合 |
| ODR | 普通依赖确认同一定义，重复 typed key 只检查共享 ABI |
| ODR 正文摘要 | 删除专用 definition fingerprint，不增加优化前正文 hash |
| 物理实现选择 | artifact-only linker 明确选择，正文及其附属 GC/EH/登记数据同选 |
| 年轻代 | 现有 arena 内按 block 划分 nursery，复用 per-thread TLAB |
| 晋升 | 第一次 minor 存活即进入 Immix 旧代，无 survivor 区 |
| 特殊分配 | 真正大对象、带 release hook 的对象直接进入旧代 |
| 收集方式 | 多 mutator、STW、单线程 collector；显式 collect 保持 full |
| 完成条件 | 真实源码/产物/链接/运行、三 target 组合验收及可复跑性能报告 |

## 1. M30 的实际基线

以下是本次设计依据的当前实现，不是 M31 已完成的功能。

| 现状 | 对设计的影响 | 主要落点 |
| --- | --- | --- |
| Scoop TargetMachine 使用 O0；IR pipeline 为 SROA/mem2reg 后 RS4GC | 机器优化和 IR 优化需要分别接通及验证 | [target.rs](../../compiler/codegen/src/target.rs)、[statepoint/policy.rs](../../compiler/codegen/src/statepoint/policy.rs) |
| 每个 callable 分别生成 LLVM module/object，其他 callable 仅声明 | 单纯开启 O2 不会得到普通跨函数内联 | [emission.rs](../../compiler/codegen/src/emission.rs)、[object_partition.rs](../../compiler/codegen/src/object_partition.rs) |
| debug/release 当前使用相同生产配置，正式 CLI fixture 允许共享产物 | 修改实际 child 请求、producer 配置与缓存规则，不能只改输出目录 | [request.rs](../../compiler/protocol/src/request.rs)、[cache/key.rs](../../compiler/scoop/src/cache/key.rs)、[CLI fixture](../../tests/fixtures/m23-cli/basic/fixture.toml) |
| generated-C 固定 O0，runtime 默认已经 O2 | 两者分别表达，不把 runtime O2 误记为新增收益 | [c_bridge_invocation.rs](../../compiler/lir/src/c_bridge_invocation.rs)、[runtime compile](../../compiler/scoop/src/runtime_build/compile.rs) |
| LIR 在 roots/sites 定稿前已有局部布尔折叠和不可达 CFG 删除 | 扩展现有入口，避免另建通用优化 IR/SSA 框架 | [safepoints/mod.rs](../../compiler/lir-lower/src/safepoints/mod.rs)、[constants.rs](../../compiler/lir-lower/src/safepoints/constants.rs) |
| 临时 root storage 使用逐站点 alloca/volatile；清单严格匹配 LIR sites | 优化前后须明确实际 GC 发射计划，不能维持旧清单再机械要求相等 | [function/roots.rs](../../compiler/codegen/src/function/roots.rs)、[statepoint/manifest.rs](../../compiler/codegen/src/statepoint/manifest.rs) |
| ODR 比较 ABI、canonical LIR、object 与 stackmap 摘要 | 需要删除实现内容相等要求，同时处理 metadata 选择 | [ODR merge](../../compiler/slib/src/layout_compile_closure/lir_physical/odr/merge.rs) |
| GC 是单代 moving Immix；卡表有写入和清理，没有 remembered set 消费 | nursery 必须包含真正的 minor 闭环 | [collector.c](../../runtime/src/gc/collector.c)、[reclamation.c](../../runtime/src/gc/reclamation.c) |
| small 上限为 64 bytes；超过后每对象至少取得一个 32 KiB span | nursery 和晋升都必须修正中小对象路径 | [heap_internal.h](../../runtime/src/gc/heap_internal.h)、[allocation.c](../../runtime/src/gc/allocation.c) |
| 编译器连标量写入也标卡，Context 使用普通 byte store | 先明确范围屏障、引用写入覆盖与并发规则 | [memory.rs](../../compiler/codegen/src/function/memory.rs)、[task_context.c](../../runtime/src/task_context.c) |

32 KiB 是当前 large span 的分配与 committed accounting 粒度，不据此宣称每个对象的实际 RSS 恰为 32 KiB。当前小对象已有 TLAB bump；nursery 的主要收益目标是减少短命对象导致的全堆追踪，并改善现有中小对象分配。

## 2. 优化配置与缓存

### 2.1 公开 profile 与实际 producer 输入

保留 `scoop build/run --profile debug|release`，默认 debug，`--release` 为已有简写。低层 `scoopc build` 同样接受封闭的 debug/release 选择，默认 debug；其 profile 只选择实际编译配置，输出位置仍由已有参数决定。machine child 请求传递解析后的 typed optimization mode，不让子进程根据目录名或环境猜测。

| producer | debug | release |
| --- | --- | --- |
| Scoop IR | 必要 lowering、已有正确性所需 CFG 清理与 SSA/GC pipeline | §3 的首批局部与 LLVM 函数内优化 |
| Scoop machine code | O0 | O2 |
| generated-C bridge | O0 | O2，保留 ABI、浮点与 `-fno-builtin` 等必要选项 |
| runtime C | 当前默认 O2 | 相同的默认 O2 |
| program-link | 现有普通链接配置 | 同一 ABI/GC 链接合同，不开启 LTO/ICF |

实际配置沿 umbrella → graph/node build → child protocol → scoopc → 对应优化阶段与 producer 传递。配置应是当前两个模式及明确 pass/producer 设置的完备数据，不增加任意 LLVM/C flags 透传、插件 pipeline 或通用优化策略框架。

prebuilt dependency 保留它自己的优化配置，只要 target、ABI、runtime/GC 契约兼容即可参与构建。release consumer 不要求把没有源码的 debug dependency 重编译成 release。不同源 Cone 可以使用不同模式；一个 resolved graph 仍只选择某个 Cone 的一份实际 artifact，不把同 identity 的多份候选自动视为无歧义输入。

### 2.2 三类信息分开

- **身份与兼容性**：Cone、声明、完整 application、exact type、ODR group/member，以及真实 target/layout/calling convention/GC ABI。优化等级不加入这些 identity。
- **编译依赖语义**：导出的模板、默认值、const、名称查询与继承事实，以及 MIR/LIR 的导出接口、布局、扫描和 typed 引用。私有优化正文、frame/site/root plan 不参与下游 stale 判定。
- **实际生产内容**：优化 mode、具体 pass/flags、compiler/toolchain、对象及关联 metadata。它们进入当前 producer 的构建键、Code/Artifact fingerprint 和最终链接输入。

复用现有 HIR/MIR/LIR semantic projection 与依赖失效机制，不额外发布“优化前正文指纹”。真实模板或 const 改动仍使消费者失效；仅改变普通非泛型函数的机器实现，只要求更新该产物与最终链接。LIR 中影响兼容性的 backend 契约与 producer 优化设置分开，不能通过 backend fingerprint 把优化差异重新变成语义冲突。

缓存行为需要直接验收：

| 变化 | 预期 |
| --- | --- |
| 当前 Cone 从 debug 改 release | 使用另一实际编译键，重新产生代码；不能复用 debug 对象冒充 release |
| 同 profile、相同输入重复构建 | 正常命中编译与链接缓存 |
| 仅依赖优化/代码改变，consumer 的模式和依赖语义未变 | consumer 可复用编译结果，最终链接失效；普通 artifact 完整性仍验证 |
| 模板、default、const、布局或 ABI 改变 | 对应语义 fingerprint 变化，源码 consumer 重编译；无法重建的 stale artifact 拒绝 |
| 仅输出目录、argv 或诊断展示改变 | 不改变编译语义和代码缓存键 |
| runtime 实际源码/toolchain/flags 不变 | debug/release 可继续复用同一 runtime 对象缓存 |

不要为了满足旧 fixture 而继续固定 debug/release 产物摘要相等；也不能仅因两种配置在某个空函数上偶然生成相同字节，就认定优化配置没有生效。

## 3. 首批优化与精确 GC

### 3.1 正式任务清单

| 任务 | 实施边界 | 验证结果 |
| --- | --- | --- |
| 真实 release pipeline | LLVM machine O2 与经过验证的函数内 IR pass 分别接入 | 生产请求/缓存正确，实际 LLVM/object 有预期变化 |
| 常量与复制传播 | 复用已有 typed CFG；先覆盖真实 primitive、复制链及已知 enum/Option 分支 | 死路径被清理，副作用与异常求值保持 |
| LLVM 函数内简化 | 复用 SROA、mem2reg、InstCombine、CSE、DCE/DSE 等标准能力 | 冗余局部存取、表达式和死代码减少 |
| 去除无效写屏障 | scalar/GC-free 写入不标卡，引用及 bulk copy 按 §5 的范围协议处理 | 标量路径减少原子操作，old→young 不丢失 |
| root 临时存储清理 | 只处理为维持旧身份比较而存在的中间存储；保留真实发布根的可见性 | moving/EH/native 组合成立，再记录 spill/load/store 变化 |
| 性能基线 | 使用真实 Scoop 源码与既有 CLI 产生和消费产物 | 运行、编译、代码大小及 GC 指标可重复对照 |

Scoop 局部变换安排在 MIR 或 LIR root-plan 定稿前的已有入口。仅当操作数、别名与 effect 事实足够时传播；未知写入、取址、调用和 GC 对相关内存事实的失效规则必须正确。能由 LLVM 完成的函数内优化交给 LLVM，不为几项局部规则另建通用 SSA、别名分析或全程序优化器。

当前 `Direct`/`FinalOverride` 已是直接调用，不能计为 M31 新增去虚拟化。自动跨函数内联、已知 receiver 的进一步去虚拟化和循环 bounds-check elimination 留到后续；它们需要各自的模块组织、分析与验证，不由“O2”三个字符自动交付。

### 3.2 普通 IR 变换与 GC 发射的边界

采用以下顺序：

1. 完成语言 lowering 与 Scoop 的调用/CFG 清理，形成完整 typed LIR 和根需求。
2. 生成 LLVM IR，运行选定的普通函数内优化。首批从 SROA/mem2reg、指令简化、CSE 和死代码/死存储消除的组合验证；CFG 简化只允许已具备正确 site 处理的规则。
3. 在最后一次普通 IR 变换之后，复用 LIR typed facts、实际保留的 site 与 managed SSA leaf，形成当前实现的最终 ExpectedSafepoints/GC 发射计划。
4. 执行 RS4GC，再做既有 relocation/use、invoke/native transition 检查。
5. 运行标准 TargetMachine pipeline，检查实际 object、EH 和 stackmap，随后打包。

最终计划是实际代码的 metadata，不是跨 Cone ABI 或 ODR 判等信息。LIR 自身仍须完整且正确；普通优化已证明不可达的 site 不再出现在发射清单，活跃站点必须具有准确 owner/role 与完整根。不能先让 LLVM 随意删除/复制调用，再以旧 LIR 清单不匹配为由拒绝一切优化，也不能从生成出的 stackmap 倒推“应该有几个根”。

最终保留的 site 不重新编号，继续使用原 owner/role/ordinal 的 typed identity；删除首个或中间站点后，序号可以不连续。foundation reader 保留 owner、唯一性和 runtime mapping 完整性检查；连续编号仅约束优化前的完整 LIR。

NoGC invoke 使用标准 `nomerge` call-site 属性防止 machine O2 合并多个受保护调用，维持最终 EH 清单的对应关系；不可达删除继续按实际 LLVM 结果投影，不把 NoGC 当作 nounwind。

InstCombine 沿用默认的一轮处理，以 `no-verify-fixpoint` 关闭仅检查是否仍有优化机会的内部断言；不要求首批优化达到全局固定点，LLVM IR、GC/EH 和对象 verifier 继续运行。

首批不启用会复制/合并 GC site 的 inlining、loop unroll、vectorization 或 tail duplication。具体 pass 名称、顺序和 CFG 选项由 LLVM 22.1 的三 target qualification 冻结；这是一项有限的实现选择，不改变上述完成范围。未经验证不直接接入完整 `default<O2>`；如果某个 pass 无法满足 site 约束，使用能完成同类局部优化的受限组合，并在实施记录解释实际配置。

运行期保留每个 managed 入口及实际循环路径的 poll 覆盖；优化不能造成无握手机会的无限循环。外部可见 callable 的 GC effect 与调用边界不按某次优化结果变更。RS4GC 之后不再运行任意改写 site/call/GC liveness 的普通 IR pipeline。

### 3.3 根的正确性与临时存储

保留 AS1 managed provenance、精确可写的 stack-only base/derived location、frame pointer、精确 return PC、禁用 managed tail call 和不同 body ICF 的合同。Machine O2 必须通过现有 post-RA statepoint spill 路径，不能引入 runtime 不支持的 register root。

只为强制“一个旧 LIR leaf 对应一个独立物理槽”而存在的临时 alloca/volatile，应以真实 SSA use 与最终根计划替代；必要时记录等值 leaf 到实际根的关联。不能为保留旧内容摘要而阻止合法优化，也不能为了减少指令删除 GC 必需的保活或 reload。

已发布的 compiler root frame、caller root、native root、invoke 的 normal/unwind root 与 callback/frozen segment storage 可由 collector 改写；它们的写入、内存可见性、出入口顺序和 reload 必须保留。M31 不要求无条件删除所有 volatile，也不新增 runtime 重扫栈或保守根来掩盖错误。

重型交叉验证放在 qualification/stress；已经在明确边界验证且未变化的 LIR、对象和 scan 数据直接复用。

### 3.4 不得改变的语义

- 整数保持 wrapping、MIN/-1 与 masked shift 规则，运行期除零仍走既定异常；不能添加语言未授权的 nsw/nuw 或 poison 假设。
- Float/Double 保持 IEEE NaN、正负零、subnormal 与逐次舍入；不启用 fast/nnan/ninf/nsz/reassoc/contract，不以 FMA 改变既定结果。
- 参数/receiver/default 求值、异常和 finally、SourceLocation、Context、初始化 exactly-once 保持；优化删除代码须有真实无副作用依据。
- 值复制、取址和 interior mutability 保持独立 place 与别名语义；GC-free 不等于无副作用，NoGC 不自动意味着 readonly/readnone/noalias/nounwind。
- native transition、native root/handle/pin、callback、协程恢复与 release hook 继续遵守现有协议。栈上 SROA 不等于可以消除任意 managed heap allocation。

`inline/crossinline/noinline/reified` 继续按已有兼容提示处理；自动优化不新增语言规则。

## 4. ODR：同一定义的兼容实现

### 4.1 合并前提与 ABI 的范围

ODR 的输入前提是普通依赖图已确认使用同一定义：原 typed template origin、完整 callable/nominal application、exact arguments、版本与 dependency semantic fingerprint 一致。普通 source/stale/typed 引用检查负责这一点；ABI 相同不能把两个不同源定义变成同一实例。

在此前提下，同一 OdrMemberKey 的重复实现只比较 ABI：

| 属于共享 ABI | 属于各自实现 |
| --- | --- |
| 调用约定、参数/返回物理表示、receiver 协议 | 指令、私有局部变量与栈帧 |
| 对外 GC effect、native/unwind 边界合同 | 实际调用序列、安全点数量/位置与 roots |
| 共享 size/alignment、字段偏移、scan 语义 | 机器码、寄存器/栈槽分配 |
| vtable/itable 槽、目标身份与 receiver adjustment | LSDA/EH frame、具体 stackmap payload |
| TD、storage、initialization 之间的共享表示与引用合同 | 私有 Context cell、临时常量池等实现附属数据 |

不能把 canonical LIR、完整初值字节或 body/stackmap hash 重新塞进 ABI fingerprint，以另一名称恢复内容判等。共享 scan 与派发合同须兼容；实际 record 是否引用正确 layout/scan/entry 仍在自身的格式/引用验证中检查。

优化不进入 specialization、type、body 或共享状态 identity；同一实例不会因 debug/release 产生两个类型、singleton 或 initialization cell。任何只适用于特定 caller 的特化也不能冒用原通用实例 identity；本批不做这类克隆。

### 4.2 删除内容判等，保留必要验证

删除 `OdrDefinitionFingerprint`、仅为它生成的 LIR/object/stackmap leaves、专用补丁与“不同代码必须 ODR 失败”的测试。不要增加优化前语义正文 hash、等价性证明、来源授权或新信任链。

保留已有 Code/Artifact/RuntimeImage 等实际内容摘要，以及各自产物读取的格式、typed owner、引用、对象范围/重定位、ABI 和 GC 检查。同一函数的两份 stackmap 可以不同，但每一份都必须能正确描述它自己的机器码。普通 hash 验证负责文件完整性，不作为两份实现必须相等的理由。

本次同时调整将私有 LIR 或 backend 优化配置纳入 dependency semantic fingerprint 的路径。不能只删最后的 ODR 比较，却在更早的 stale edge 或 layout closure 合并阶段因优化差异拒绝输入。

### 4.3 物理选择与最终 image

原 Mach-O 逐 member weak coalesce 无法保证独立符号来自同一 producer。M31 在 native link 之前显式处理选择，Darwin 和 ELF 使用相同语义：

1. reader 各自验证 artifact 及其候选定义/引用闭包，保留既有 typed 物理索引。
2. 对真正共享的 primary member，在 key/ABI 兼容后按 canonical Cone 顺序选择第一个可用 producer。相同输入集合的选择不依赖命令行、路径或 archive 枚举顺序；首版不额外按优化等级评分。
3. callable 的正文、LSDA/EH frame、stackmap blob、callable/safepoint registration、Context key-use cell 及其他私有关联 atom 随正文同选。附属 site 的数量和 key 可以随实现不同，不在选择前要求各 producer 有相同清单。
4. 对有独立 identity 的 TD、scan、storage、初始化记录和 helper 继续按其 ABI 与必要引用闭包处理；没有独立共享语义的附属记录不单独竞选。不同 helper 可来自不同 Cone，不引入“整组必须同一 provider”的限制。
5. 移除落选物理输入，再从实际所选记录生成最终每 Cone 的 image descriptor 与六类 pointer 表，解析全部引用，运行既有最终对象/符号/ABI 验证。

codegen 保持每 callable 一个对象，并把需要同选的数据移入其关联物理输入；当前混在 noncallable metadata object 中、需要独立选择的 ODR 数据按真实选择边界分区。复用既有 definition/atom/member owner，不创建通用 cohort 或第二套泛型发布器；普通 Strong metadata 不做无意义的逐记录分碎。

产物保留完整候选 image/registration 的 typed 数据及物理归属；最终 image 表由 linker 的现有 native-object 输出路径产生，不重新编译 Scoop/LLVM、不读取源码或 runtime header。落选 site 不以 unresolved weak reference、空登记或 runtime 忽略额外 stackmap 的方式残留。落选正文独占的关联数据可以删除；有独立共享 identity 且仍被所选定义引用的数据必须保留。

每个 Cone 仍有逻辑 image，即使所选 producer 表为空也保留其依赖与初始化归属。共享登记在所选 provider 的 image 列一次，其他 image 通过普通引用使用同一地址。产物中的 RuntimeImageFingerprint 覆盖完整候选登记内容；最终 image 按实际所选表计算自己的内容摘要。输入产物摘要、最终 image 内容和选择结果进入已有 ResolvedLinkPlan/链接缓存，输入 `.slib` 不被改写；普通 eager 顺序与 lazy exactly-once 不随 provider 选择而改变。

GC 的最终 site/body/PC 唯一性在实际选择后检查。某个 ODR body 在 debug 有三个 site、release 有两个 site是合法输入；最终只能保留一份正文及其对应清单，不能让 A 的 body 配 B 的 roots。

### 4.4 登记格式与兼容迁移

metadata ABI 从 4 升至 5，删除六类 registration 公共 identity 的 `definition_fingerprint[32]`；linkage、semantic id、ODR group/member 保持。相应 canonical record 删除同一字段，不用零值占位，也不创建另一种正文 hash。

删除 OdrDefinition 和仅用于该公共字段的 StrongRegistration 摘要节点；记录内容由现有 RuntimeImage/Code/Artifact 覆盖。body/descriptor/layout/scan/normalized-stackmap 中仍有实际消费者的字段继续服务各自实现的读取或运行期检查，不参与跨实现 ODR 判等。

该公共字段删除使各类 registration 缩短 32 bytes，callable record 从 208 变为 176 bytes。更新 RuntimeImage 编码域、schema/required inventory/profile 和实际 C/LLVM/native-object 布局；退役字段/tag 不复用。对象头、TD、managed ref 和 stackmap v3 不因这一调整改变，具体版本清单随实际实现记录，旧 artifact/cache 重建，不保留旧内容判等双轨。

## 5. nursery、晋升与 remembered set

### 5.1 分区和对象分配

在已有 arena 内用 block side metadata 区分 nursery 与旧代，代龄与 block 的 free/active/pinned/evacuation 状态分开表达。TLAB 从 nursery 取得；正常分配仍是对齐后的 bump 与现有 GC-leaf finish，返回前完成清零、对象头、object-start 和精确 allocation size 发布。

普通对象只要能放入一个普通 block 的可用空间，就使用普通分配路径。无法装入的对象使用旧代 large span；首版不把 64 bytes 当作分代分界。为此必须同时调整：

- nursery 与旧代普通 allocator 支持跨 line 的连续对象，不能只改一个 size 阈值；
- 扩大旧 `uint8_t size_units` 表示，使其覆盖 block 内合法 allocation；例如 8-byte units 用 `uint16_t` 即可覆盖当前 block，精确数值仍由实际 allocation 记录；
- 标记所有被对象覆盖的 line，card 扫描可从中途地址定位包含它的对象；
- 晋升使用旧代普通 run/block 空间，不能把每个稍大的存活对象重新送入独占 large span；
- 内联 TLAB 条件、slow path、dynamic String/Array、copy/evacuation 与精确 size 校验使用同一表示合同。

首版 nursery 容量是内部 block/byte 参数，按 §8 的分配工作负载确定默认值并记录，不是对象合法性的任意上限。保留当前 arena 的容量与局部分配失败处理；不为分代引入跨阶段资源预算、计费或 arena 扩容框架。

### 5.2 minor 收集顺序

minor 复用现有 STW/epoch 和 slot visitor；collector 仍单线程。

1. **停止与根快照。** 等全部 mutator 按原协议停下，废止所有 TLAB。扫描完整根：managed stackmap、invoke/compiler/caller roots、native 两类 root frame、handle、pin、static storage、stable external object、线程 Context、callback 与冻结 managed 栈段。已登记的只读 immortal 仍只允许 GC-free 出站表示。
2. **年轻可达性。** 对根与旧代脏卡中的实际引用，只继续追踪 nursery 对象图。遇到普通旧对象不递归遍历其全部后继，旧→年轻边由 remembered set 提供。这里把旧代脏对象保守视为存活是允许的，minor 不回收旧代。
3. **晋升规划。** 对年轻存活对象安排旧代空间及 forwarding，包含精确对齐、跨 line 占用和 pin 例外。先完成真实空间规划，成功后才发布 forwarding 并回写根；不能仅凭总空闲字节假定碎片空间一定可用。
4. **复制和更新。** 未 pin 的年轻存活对象复制到旧代；更新全部实际根、脏槽和年轻存活对象中的引用。循环、共享引用和重复根只产生一个目标对象。复制保持完整 payload/header 与 ready/pin 位。
5. **回收和恢复。** 本轮所有年轻存活对象均已是旧代，删除年轻死亡对象和被搬走副本的 metadata，重置可复用 block 与已消费卡，按原协议恢复 world；新 TLAB 在下一次分配 refill。

第一次 minor 存活即晋升，因此没有 survivor/to-survivor 空间、代龄计数、晋升年龄阈值或循环保留的 young survivor 队列。代价是中等寿命对象可能较早进入旧代；只有性能数据证明需要时，再单独设计 survivor。

normal minor 的工作量应主要随根、旧代脏区和年轻存活图增长。可扫描必要的紧凑脏区索引，不能为寻找引用、回收或验证而枚举所有旧代对象/引用，也不能调用现有全堆 verification walk。

### 5.3 写屏障的精确范围

首版使用现有 card table 建立 remembered set。编译器只对可能含 managed ref 的 heap 写入标卡；允许保守标记年轻目标以及没有年轻 RHS 的引用写入，minor 消费时只处理旧代卡。暂不要求每次写入先读取 RHS 的代龄，避免把值分类或复杂慢路径放入所有 store。

| 写入 | 屏障规则 |
| --- | --- |
| scalar/GC-free/ZST、零长度 copy | 不标卡 |
| 一个 managed reference slot | 标记该目标 slot 所在卡 |
| 含引用 struct/enum/tuple 字段 | 覆盖所有被写 managed leaf 所在卡 |
| array/aggregate 的连续 bulk copy | 标记整个可能含引用的目标范围，可保守覆盖范围内非引用字节 |
| static/native root slot | 按原精确根扫描；不将 heap 外地址代入 card table |
| 已知未发布且未经过 safepoint 的 nursery 初始化 | 可基于当前控制流事实省略；无法证明则执行屏障 |

collector 根据 object-start、精确 size 与 scan 程序，扫描与脏卡相交的实际引用槽。对象可能从上一张卡开始，aggregate/array 可以跨多卡；大数组按相交范围找到元素与引用，不应因为一次槽写入就无条件扫描整个巨大数组。记录足够的脏 block/card 索引以跳过干净旧区，复用当前 GC metadata，不增加通用事件队列框架。

所有路径必须覆盖：普通字段、heap aggregate、引用数组、生成构造器、闭包/协程 frame、runtime Context、array clone/copy、boxed value payload，以及 Scoop ABI native 的 managed heap store。native 使用一个目标地址/字节长度的 GC-leaf 范围屏障入口；该入口不分配、不 park、不 GC。

屏障在写入之后、下一次 safepoint/park 或发布之前完成，中间不能插入 GC。pin 或 native root 只保证保活/地址，不代替屏障；对象可能在构造/copy 调用中途晋升，不能永久把“构造器的 this”视作年轻对象。

多 mutator 对 card 使用 atomic monotonic store/RMW，Context 的普通 byte store 必须迁移。STW handshake 提供 collector 对引用及脏位的可见性；清卡只发生在 mutator 停止时。minor 内部更新在恢复 world 前统一完成，全部年轻存活对象晋升后允许清理本轮脏卡；恢复后的新 store 重新标记。

### 5.4 pin、release hook 与晋升失败

**pin。** pin 调用继续保活并返回原地址，不在 pin 操作中悄悄搬迁对象。minor 对含 pinned 对象的 nursery block 在 STW 中转为旧代，保留该 block 中存活对象的地址，删除死亡对象并形成可复用 hole；所有留下的存活对象引用都更新。不能只把 block 改成 old 而漏扫其中年轻出站边。unpin 后允许后续 full 按正常 Immix 规则移动对象。

**release hook。** 带 hook 的对象首版直接分配在旧代，包括构造尚未 ready 的阶段。它们继续由 full reclaim 按 Dead/LiveInPlace/RelocatedCopy 和 ready 规则决定是否同步调用 hook；minor 不因丢弃副本调用 hook。构造失败、显式 close、已搬迁副本和 pin 的组合沿 M24 合同测试，不增加 finalizer、复活或异步释放队列。

**空间不足。** 晋升规划在发布 forwarding 和修改可见引用前失败时，释放未使用的目标预留、清理本轮工作标记，保持原图完整，在同一个 collector/STW 调度中改做 full。full 可回收旧代并把年轻存活 block 原地转旧，必要时仅在有实际 to-space 时搬迁；它不递归进入另一个 GC，不留下半更新的 roots。完成后按实际空间重试当前分配，仍不足则走现有明确失败出口，不无限 minor/full 重试或绕过 size/alignment 检查。

### 5.5 full、触发与 stress

- nursery 容量耗尽通常触发 minor；旧代或 large allocation 压力、晋升空间不足触发 full。
- 显式 `gc.collect()` 保持 full：收集两代，年轻存活对象进入旧代；存在 eligible source 且空间足够时保留既有 moving 要求。
- 旧代继续复用 Immix mark/evacuate/reclaim；minor 不为不可达旧对象执行 release hook。
- normal 路径在实际遍历中检查引用与 forwarding，不额外重放全堆验证；显式验证/stress 可对 remembered set 与全堆结果交叉检查。
- 保留 full moving stress；增加真实 nursery/minor stress，并覆盖 minor/full 交替。现有 stress 每次分配新 block、禁用普通 TLAB 的旁路，不能单独作为 nursery 验收。
- poison/quarantine 用来发现旧指针；测试配置要明确区分故意不复用 block 的验证行为与正常 nursery 重置，性能测试不运行在该压力模式。

nursery 不改变 managed ref、对象头或 stackmap v3，但写屏障完整性、TLAB/分配 helper 合同与对象 metadata 表示的变化必须进入普通 runtime/backend 兼容版本。旧单代产物的不完整 barrier 不能被新 runtime 静默当作分代正确的代码。

## 6. 实施落点与版本纪律

| 范围 | 主要修改位置 |
| --- | --- |
| 实际 optimization 配置 | protocol、scoop graph/build/cache、driver、codegen target/pass、generated-C producer |
| 依赖语义/代码拆分 | 既有 HIR/MIR/LIR semantic projection、backend/producer 配置与 cache dependency key |
| ODR ABI/物理选择 | identity 的原 key、slib 共有 reader/merge、codegen partition、artifact-only linker |
| 最终 GC 发射计划 | lir-lower 的现有根需求、codegen ExpectedSafepoints/RS4GC/object verification |
| nursery/minor/full | runtime/gc allocation、collector、evacuation、reclamation、block metadata |
| 屏障 | codegen heap/array stores、runtime Context/array/box、native runtime header/entry |
| 验证 | 正式 fixture runner、现有 GC/runtime/codegen 测试与小型性能用例 |

身份类型沿用原实体的实际职责；不把 `OptimizationMode`、物理 producer 或 private stackmap 引入 application/type identity。stage 仅经输入/输出 IR 通信，优化 pass 不依赖上游 stage 实现 crate。

版本按真实 wire/ABI 变化升级，包括 child 请求、producer 配置、manifest 物理索引、LIR production/GC code plan、image/registration、runtime 兼容合同和缓存。§4.4 已确定 metadata ABI 5；其余具体 field/tag 与 section major 在落地对应批次记录并更新 inventory/profile，退役 tag 不复用，不为仅计划的后续优化预留占位字段。

一次消费中的 Compile/Link 复用已验证不可变数据；新检查只针对实际变化的 ABI、物理选择或 GC 事实。不为“优化更安全”增加来源凭证、证明链、通用成本预算或整套第二验证器。

## 7. 实施批次

| 批次 | 交付 | 依赖与验收出口 |
| --- | --- | --- |
| M31-1 配置与基线 | 记录 M30 性能；贯通实际优化配置、producer 与缓存；普通/child CLI 一致 | 能分别构建两种模式；更新仅目录 profile 的旧 fixture |
| M31-2 ODR 与 image 选择 | 删除内容判等、拆开依赖语义和代码；正文附属数据同选；metadata ABI 5 与最终 image 生成 | artifact-only 混合模式、不同 site 集合、唯一共享状态通过 |
| M31-3 首批优化 | Scoop 局部清理、LLVM 函数内 pass、machine O2、最终 GC 发射计划及可安全消除的 root 临时存储 | 三 target debug/release 的语义、GC/EH/native qualification |
| M31-4 分配与屏障准备 | 中小对象跨 line 路径、精确 size、旧代晋升 allocator、完整范围 barrier 与并发标卡 | 独立分配/scan/card 边界和真实 store 组合通过 |
| M31-5 nursery 闭环 | nursery TLAB、minor、首存活晋升、pin 原地转代、hook pretenure、full fallback | remembered set、moving、空间不足与多 mutator/FFI 组合通过 |
| M31-6 总验收与性能 | 完整功能/CLI、混合优化/GC 组合、性能对照与实际版本清单 | 无必需项遗漏；报告真实收益、回退和未覆盖范围 |

M31-1 的配置通路可先验证 machine O0/O2，不能因此宣布完整 release 优化已完成。M31-4 可与编译器优化分开验证；M31-5 必须依赖真实屏障和晋升分配路径。最终默认启用时必须同时具备 ODR 兼容与 GC 正确性，不留下新 runtime 接受旧不完整 barrier 的中间兼容状态。

## 8. 验收与性能

### 8.1 正确性矩阵

正式验收使用 Darwin/AArch64、Linux/amd64 glibc、Linux/amd64 musl；musl 的现有静态/动态链接覆盖继续保留。debug 与 release 都执行适用功能、CLI、artifact-only link 和 GC 组合，复用现有 fixture 声明与 runner。

| 类别 | 必须覆盖 |
| --- | --- |
| 数值/控制流 | 整数边界/除零/shift；浮点 NaN/负零/subnormal/舍入；default 求值、分支、循环、异常/finally |
| 值与调用 | aggregate/enum/Option、复制与取址、interior mutability、闭包、协程、SourceLocation、Context |
| 配置/缓存 | 冷/热 build/run、普通/child 请求、source/prebuilt 混用、profile 切换、优化差异不制造 stale |
| ODR 正例 | 多 Cone 同 application 的 O0/O2，实际不同正文和 site 集合；独立 helper 并集；交换输入顺序仍稳定选择 |
| ODR 状态 | 相同 TD/函数/storage 地址、generic companion/delegate 单一初始化、release hook/Context 附属数据正确 |
| ODR 反例 | 同 key ABI 不同、真实 stale 模板/const/布局、普通 Strong 重复、未闭合物理引用、正文与 stackmap 配错 |
| nursery 图 | 全死、全活、链/环/共享；仅 old→young 可达；连续多轮 minor 后 full |
| 分配边界 | 64 bytes 两侧、跨 line/card、接近 block 上限、large、对齐和精确 size；晋升不退回逐对象大 span |
| store 覆盖 | 字段/aggregate/ref array、跨卡 copy、Context/array clone/box、构造中途 minor、Scoop ABI native store |
| 并发/边界 | 同一卡多 mutator 写入、native roots 两种 variant、handle、young/old pin/unpin、callback 与 frozen segment |
| 失败/释放 | 晋升空间不足 full fallback、构造异常 ready=0、close 后回收、hook 对象移动、分配失败不遗留半更新根 |
| GC 压力 | 实际 minor stress、full moving stress、交替收集与显式全堆交叉验证 |

针对优化的 golden dump 保留真实类型/控制流/ABI/GC 结构，不固定无关机器码摘要。对于“ABI 相同但对象或 stackmap 不同”的输入，正确测试应接受并运行；对于“某份 stackmap 描述错自己的代码”，仍必须拒绝。混合优化测试还要在移除源码和 LLVM 可用性后执行 artifact-only 链接，证明没有隐藏重编译。

实现阶段遵守仓库顺序：先对应格式化与 lint，再运行适当 Rust/runtime 测试、公共 runner 单元测试及正式文件 fixture；最终以三 target 实际报告记载范围。不能用 cargo test 代替正式 CLI，也不为纯文档设计伪造测试结果。

### 8.2 性能用例与记录

在 `tests/benchmarks/` 增加少量真实 Scoop 程序及可重复命令，复用现有 build/run/link 与 runtime 诊断/测试统计，不创建通用 benchmark 平台。至少包括：

| 工作负载 | 要回答的问题 |
| --- | --- |
| primitive 循环与局部计算 | LLVM 函数内和 machine O2 是否减少冗余工作 |
| aggregate/Option/enum 局部流转 | 复制、常量传播与分支清理是否生效 |
| scalar 与 reference array 写入 | 去除标量卡标记是否受益，引用屏障成本如何 |
| 大量短命小/中对象 | nursery 是否减少 full 次数与全堆工作 |
| 长寿旧图 + 短命分配，只有少量脏边 | 增大无关旧图时 minor 成本不随其对象/引用数线性增长 |
| 高存活率、pin、跨代写入 | 首次存活即晋升的代价及退化行为 |
| 泛型/跨 Cone、String/List/JSON 或 Context | 改善能否体现在真实功能组合中 |

记录编译器 revision、target/toolchain、实际 mode/pass、nursery 参数、输入规模、运行次数与环境。至少输出运行时间/吞吐、编译时间、exe/`.slib` 大小、分配量、minor/full 次数、复制/晋升量、扫描脏区与 GC 停顿分布；区分 committed accounting 与实际 RSS。

先保存 M30 基线，再分别比较“仅编译器优化”“仅分代”“两者组合”，避免把 runtime 本来就有的 O2 或测试环境变化记成收益。使用足够运行次数的中位数和波动范围；correctness CI 不以任意毫秒阈值决定程序是否合法。关键用例的明显回退需要解释或修正，指标不改善时先定位原因，不靠扩大工作量或删除正确性检查制造结果。

实施时采用同一 M31 revision 的四组控制：debug/full-only、release/full-only、debug/nursery、release/nursery。full-only 使用 runtime spec 3.9 的诊断开关，在相同 refill 容量请求普通 full；每个 profile 只编译一次，同一个 executable 分别运行两种收集方式。四组共同包含 ABI 5、跨 line 分配、Scoop 局部清理与必要 GC lowering，因此 compiler 因子只表示 debug→release 的额外优化，nursery 因子只表示 minor 与 full 的差异；整个 M31 与保留 M30 的比较另列。新增大旧图和高存活率/pin 用例检验退化行为。不同宿主或模拟环境各自建立基线，不把容器数据与 NUC 原生时间相除。

## 9. 明确留待后续

以下不纳入 M31 完成门：跨 callable LLVM module 重组与自动内联、全程序去虚拟化、通用逃逸分析、managed allocation 消除、协程 frame elision、循环 bounds-check elimination、激进 unroll/vectorization、LTO/ThinLTO、PGO、survivor/多级代龄、自适应晋升、parallel/concurrent GC、arena 扩容/多 arena，以及调试信息体系。

这些工作以后按实际瓶颈单独设计。M31 以可用的优化构建、可互换且正确配对的 ODR 实现、真实 nursery/minor 闭环和可核对的性能数据完成，不以额外授权、指纹证明或假想通用框架扩充验收门槛。
