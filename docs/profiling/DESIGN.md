# Profiling、PGO 与 IR template 导出设计

状态：首版实现完成，实施记录见 [PROGRESS.md](PROGRESS.md)，原生验证及性能边界见 [VALIDATION.md](VALIDATION.md)。设计日期：2026-11-13；验收日期：2026-11-14。

本文规定运行信息采集、反馈优化、系统库组件和命令入口。`profiling`、`--export-ir`、`--profile-use`、`scoop pgo` 和安装器的 profiling 组件选项已在实现分支交付。首版反馈会改变现有 cost 决策，实际性能仍需对照验证；验收记录保留已观察到的回归及后续改进方向。

[下一阶段规划](NEXT-STEPS.md)明确后续采用“保留默认内联、PGO 只追加”的策略，并安排上下文反馈、分支权重和带类型检查的去虚拟化。下文第 7 节描述首版当前实现，后续策略尚未实施；[缺省 release 分析](RELEASE-ANALYSIS.md)保留已有测量及独立静态优化建议。

依据：[语言规范](../specs/SCOOP-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[路线图](../ROADMAP.md)。已有基础见[调用点内联评分](../program-performance/CALL-SITE-INLINING.md)、[CPU 采样归因](../program-performance/CPU-PROFILING.md)与[调试信息实现](../debugging/IMPLEMENTATION.md)。实施各阶段前，先同步对应权威规范；本文不把尚未实现的行为记为现有能力。

## 1. 目标与主要决定

Scoop 已有依据 cost 决定是否内联、如何处理字符串等优化的结构。下一步先让程序提供真实执行频次，再把这些事实送回现有决策位置。

本设计包含四个相互配合、可以分批交付的能力：

| 能力 | 作用 | 默认行为 |
| --- | --- | --- |
| `profiling` build profile | 生成经过 release 优化、带运行期计数的程序 | 采集函数、调用点与控制流计数，输出必要 runtime 指标；默认保留行表 |
| `--export-ir=off\|default\|full` | 控制 `.slib` 中供消费方优化使用的正文范围 | 普通构建使用 `default`，与 debug/release/profiling 正交 |
| 独立命令 `scoop-pgo` | 编排训练、合并、反馈构建，复用参数 | `scoop pgo instrument` 构建并运行；`scoop pgo optimize` 生成新的 release binary |
| 可选 profiling 系统库组件 | 单独下载、安装每个 target 的完整模板库及采集资源 | 系统 `.slib` 全部使用 full；训练选带探针的库，反馈选配套无插桩库 |

采集位置及其含义由 Scoop 定义，原始计数文件与合并复用 LLVM InstrProf、compiler-rt 和 `llvm-profdata`。反馈首先进入已有 MIR cost 模型，不以实现 ThinLTO、通用去虚拟化或新的优化框架为前置。

PGO 数据只影响优化收益判断。类型、可见性、ABI、异常、GC、别名与逃逸等正确性条件仍由现有静态规则决定；训练中未观察到某条路径，不构成删除它的语义依据。

首版不实现自动挑选训练负载、自动调参、分布式训练、跨平台 CPU 采样器或每次调用的完整时间追踪。平台 CPU 采样与已有符号化工具继续用于解释耗时。

## 2. 实施前的基础与缺口

| 实施前的实现 | 可以复用的部分 | 本次补齐的部分 |
| --- | --- | --- |
| [BuildProfile](../../compiler/scoop/src/build/request.rs) 只有 Debug/Release；[OptimizationMode](../../compiler/lir/src/optimization.rs) 表达优化模式 | 构建编排、release 优化、独立输出目录 | 增加 Profiling；采集配置独立于优化模式传递 |
| [调用点热度](../../compiler/mir-lower/src/optimization/inline/policy/heat.rs) 使用静态控制流 | 普通点为 1，冷出口为 1/8，循环热度为 4/16/32；已有评分和增长限制 | 使用匹配的动态频次替代静态估计，保留缺失时的回退 |
| [优化报告](../../compiler/mir/src/optimization_report.rs) 保存观察时的 MIR 坐标 | 决策理由、cost 分项、源位置和内联来源 | 增加数据来源及覆盖诊断；临时 block/statement 编号不能成为持久采集键 |
| [普通正文导出判断](../../compiler/hir/src/inline_body.rs) 与 [模板生产](../../compiler/hir/src/production/default_templates/generic.rs) | 泛型/default 模板格式、typed 引用、支持声明与 helper 闭包 | 增加显式导出策略；区分完整序列化能力与当前内联器的处理能力 |
| `--debug-info line-tables`、symbols 观察及平台采样已经可用 | 执行行和 LLVM 内联帧归因 | profiling 选择合适默认值；这些信息不能代替可回灌的执行计数 |
| GC 总计数与 `--enable-startup-stats` | 现有指标及计时位置 | 独立文件输出、统一采集生命周期，后续补 GC 事件时间线 |
| [外部命令分发](../../compiler/scoop/src/cli/external.rs) | `commands/scoop-<name>`、原始 argv、进程状态和标准流转发 | 安装 `scoop-pgo`；让它消费正常 build 的机器输出 |

当前导出普通正文时已经移除了旧的 96 节点过滤。`default` 应描述当前结构支持范围，不重新引入一个与消费方 cost 模型脱节的尺寸阈值。

## 3. Build profile 与底层接口

### 3.1 三种 profile

| Build profile | 优化模式 | 运行期采集 | 默认 debug info |
| --- | --- | --- | --- |
| debug | Debug | 关闭 | full |
| release | Release | 关闭 | none |
| profiling | Release | 开启 | line-tables |

`profiling` 使用 release 的优化与运行语义；探针必然带来时间、内存和代码大小开销，训练程序的速度不能直接当作 release 性能。`--debug-info` 仍可显式覆盖默认值。

内部保持 `OptimizationMode::Debug/Release`，将 `BuildProfile::Profiling` 映射到 Release，另用封闭的配置表达 `Off`、`Generate`、`Use`。配置经正常编译请求传到负责阶段；不得靠环境变量在各 stage 内分别猜测模式，也不能让现有 `== Release` 判断遗漏 profiling 的优化。

反馈构建采用 `release + Use`，不增加第四个名为 pgo 的 build profile。`Generate` 与 `Use` 互斥；首版 `--profile-use` 要求 release，debug 或 profiling 与它组合时报参数错误。普通 `scoop build` 的默认 profile 保持 debug。

以下为拟定的低层使用方式：

```sh
scoop build . --profile profiling -o target/training/app
SCOOP_PROFILE_DIR=target/profile-run ./target/training/app

scoop build . --release \
  --profile-use target/pgo/merged/profile.json \
  -o dist/app

scoop build path/to/library --release --emit-slib --export-ir full
```

`--profile-use` 接收 Scoop 的反馈数据索引，其中引用 LLVM indexed profile 和 Scoop 位置映射；它不把任意 `.profdata` 当作 Scoop MIR 数据。低层 `scoopc` 接收同样含义的具体配置；manifest、训练流程和数据目录管理属于上层工具。

`SCOOP_PROFILE_DIR` 是运行期输出位置，不进入编译键。直接运行 profiling 程序而未设置它时，使用当前工作目录下的 `scoop-profile/`，每次运行创建独立目录，避免覆盖其他进程的数据。程序继承环境后启动的已插桩子程序也各自产生文件。

行表只对本次实际编译的正文生效；不能通过应用层选择 profiling 给已经预编译的库补出调试信息或探针。

### 3.2 阶段边界

```mermaid
flowchart LR
    Source[源码与已选依赖] --> MIR[语义 lowering 完成后的 MIR]
    MIR --> Plan[固定位置身份与采集计划]
    Plan --> Opt[现有 MIR 优化]
    Opt --> LIR[LIR 与计数位置映射]
    LIR --> LLVM[LLVM InstrProf lowering]
    LLVM --> Train[profiling 程序]
    Train --> Raw[profraw 与 runtime 指标]
    Raw --> Merge[llvm-profdata merge]
    Merge --> Facts[匹配后的 typed 频次事实]
    Facts --> Opt
```

图中的反馈来自前一次训练。反馈编译先在同一 MIR 边界匹配数据，再执行优化；不在最终 program-link 中隐藏重新编译，也不让 IR/meta crate 依赖上游 stage 的实现。

## 4. `.slib` 的 IR template 导出策略

### 4.1 对外语义

使用 `--export-ir` 命名，值为 `off/default/full`。它控制额外的优化正文导出，避免用 `prune=off` 表达“全部导出”造成反向理解。

| 内容 | off | default | full |
| --- | --- | --- | --- |
| 公开声明、签名、类型、ABI 与正常机器定义 | 保留 | 保留 | 保留 |
| 泛型单态化、默认实参及其局部 callable 等语义必需模板 | 保留 | 保留 | 保留 |
| 供优化使用的普通非泛型正文 | 不额外导出 | 沿用当前可移植正文选择及 helper 依赖闭包 | 导出本 Cone 所有有 Scoop 正文的函数及所需闭包 |
| 无 Scoop 正文的 extern、抽象声明 | 不制造正文 | 不制造正文 | 不制造正文 |

`full` 包括普通函数、方法、构造器、属性访问器，以及正文引用的 private/internal helper、局部函数、闭包和必要声明。泛型宿主或泛型成员继续使用原有泛型模板路径。编译器 intrinsic 的既有语义和降低角色照常携带，不为没有源码正文的实体伪造模板。

“全部”要求能完整表达合法正文的控制流、捕获、Context、协程与异常结构。不能仅因为当前普通内联器暂时不能处理这些结构，就在 full 导出阶段丢弃正文。实现中应复用并补齐已有模板表示，不另建第二套语言语义检查体系。

### 4.2 off 的准确边界

off 关闭的是**通过普通导出正文进行的跨 Cone 优化**，不是禁止一切跨 Cone 内联：

- 外部泛型仍必须在消费方物化；物化后的本地函数可以被正常优化和内联。
- 默认实参仍按语言语义展开，不能为了关优化而删掉必需模板。
- 主值构造器等已有明确 lowering role 的实体，可以由既有角色和字段信息生成优化正文，不依赖普通 IR template 导出。

因此不能把 off 的帮助写成“完全禁止跨 Cone 内联”。若将来确实需要禁止所有来源的跨 Cone 展开，应另行定义消费方优化选项及其边界，本项不顺带增加该开关。

当前 `inline/crossinline/noinline/reified/tailrec` 修饰符的语义以语言规范为准，不能借导出选项悄悄赋予它们新的优化承诺。

### 4.3 导出、导入与执行定义各自负责什么

导出负责提供完整正文。消费方仍只为实际使用的声明准备优化正文；full 不要求预先具体化或降低库中的全部函数。内联器分别判断正文是否可处理、是否满足递归/异常/GC/初始化约束，以及调用点收益是否值得增长代码。

现有普通正文导入阶段也使用可移植性过滤。实施时应把它同 default 的导出策略分开：full 正文不能在导入时又被按 default 集合丢弃；沿完整模板路径处理已使用正文，再由 MIR 判断是否可以、是否值得展开。需要直接操作 provider 全局存储或初始化门的访问器仍调用 provider 入口：当前导入正文的存储模型不能表达这类展开，优化报告标为 unsupported-body；full 仍完整导出其模板，不为内联另建一套外来存储 ABI。常量引用保留真正的 provider 和 callable/property 归属。

取得 private helper 的模板不会让它变成源码可见。普通外来 Strong 定义仍属于 provider；消费方可展开其正文，但不能因模板存在而额外发射该 Strong 的机器定义。泛型物化与 ODR 继续遵守现有所有权和选择规则。

模板存在本身不增加运行时 roots，不使未使用函数、字符串常量或机器成员保活。为实际展开而产生的物理引用仍走现有产物和链接依赖通道。

导出模式对 debug/release/profiling 的语义一致，进入产物生产配置和正常缓存更新输入，不进入语言实体身份或目标 ABI。`default` 首版沿用当前集合；以后如调整，必须有库体积、编译成本和优化机会的实测依据。

## 5. 采集的数据及其含义

### 5.1 首批计数

| 数据 | 准确定义 | 首个用途 |
| --- | --- | --- |
| 函数进入次数 | 固定 MIR 边界上的逻辑 callable 调用次数，包含被内联的实例 | 调用点频率分母、函数覆盖情况 |
| 调用点次数 | 执行到该逻辑调用点的次数，在可能抛出的调用开始前计数 | 内联收益与候选排序 |
| 基本块与必要边计数 | 指定控制流位置被执行的次数；显式区分分支后继与异常出口 | 分支概率、循环频次、冷路径判断 |
| runtime 总指标 | 复用已有分配/GC 计数，以及有明确边界的启动阶段计时 | 分辨分配压力、GC 与启动成本 |

块入口次数不能自动当作块内所有调用的次数：前面的调用可能抛出、退出或不返回。只在已有控制流证明等价时合并计数位置；否则保留必要调用点或边计数。

函数次数是逻辑执行次数，不是物理栈帧数量，更不是 CPU 时间。循环回边与入口计数可给出总迭代次数和平均值，不能据此声称已有完整 trip-count 分布。

首版采用 64 位计数，在 InstrProf lowering 中明确选择原子更新，保证正常多线程训练不会因竞争丢计数；不沿用 LLVM 的非原子默认更新。原子代价单独测量；有具体热点证据后再考虑局部合并或线程本地方案。溢出、无法恢复的流量关系等沿 LLVM/读取边界的实际诊断处理，不把错误数值当作精确事实。

### 5.2 后续观测

GC 事件时间线作为采集能力的下一批：记录单调时间、GC cycle、STW slice、phase 与相关计数，区分 rendezvous 等待、实际暂停和 worker CPU。一个增量周期可以包含多次暂停，不能用整个周期墙钟代替单次 STW 延迟。

事件时刻取自实际 world phase 发布和既有 collector 阶段。phase 同时关联 cycle 与 slice；重启可能在一个 slice 内跨越两个周期，plan/copy 和 reclaim/vm-return 按嵌套区间解释。收集期间只复制既有标量到固定 8,192 条 native 缓冲，退出时写出已发布前缀并报告 dropped；不在暂停中写文件、扫描堆或产生托管分配。总指标继续覆盖完整运行，时间线截断单独表达。具体时间边界见运行时规范 §7。

分配站点、实际分配类型/字节数，以及间接调用目标分布在有对应优化消费者时加入。分配采集必须覆盖 codegen 的 inline nursery fast path；只拦截 C runtime 分配入口会漏记。被优化消除的分配不算实际分配。

首版不在每个函数入口/出口计时，不对每次分配采完整栈。native/runtime 耗时和物理调用栈继续由平台采样解释。runtime 事件不保存托管对象地址、不增加 GC roots，也不改变对象头布局。

## 6. 稳定身份、计数映射与 LLVM 接入

### 6.1 逻辑采集位置

在必要语义 lowering、具体化和 CFG 构造完成后，首次可选 MIR 优化之前，为有完整正文的 callable 建立采集计划。generation/use 必须使用同一个边界和编号算法。

现有 pipeline 在构造协程 frame 之前已有 `inline_suspend_calls`，采集位置必须早于这一轮内联，不能只在 `run_current` 前编号。原始 callable 的标记随 hidden ABI 与状态机变换移动；变换中新生成的 callable 在其首次具有完整正文时建立自己的计划，保留已经携带的原正文标记。插入标记时同步更新 `pre_coroutine_call_sites` 的实际语句位置；handler 的块入口计数位于必要的 landing pad 指令之后，不破坏 LLVM EH 首指令要求。

位置使用以下信息：

- 既有 typed 声明、callable body、materialization/implementation 身份，保留 exact 类型实参和目标相关条件。
- 该正文在固定边界上的结构化位置序号与种类，例如 entry、call、branch successor。
- 规范化正文/CFG 和计数布局的 fingerprint，用于判断旧数据能否解释当前位置。

这里的 fingerprint 只做正文和格式匹配。它不复用包含插桩/反馈配置的整个产物 code fingerprint，也不成为来源授权、真实性证明或缓存命中审计。

arena 中的 FunctionId/BlockId、显示 FQN、源码行号和机器 PC 都不能单独作为跨构建键。源码位置用于显示；重载、泛型实例、同一行的多个调用、外来模板和编译器生成 callable 必须按实际 typed 身份区分。

### 6.2 随优化维护映射

采集计划属于独立的编译数据。优化必须能够保存、移动、复制或删除对应标记，探针本身不计入用户代码的 cost，也不能被当成托管分配、GC safepoint 或语言可见副作用。

- 内联保留原调用点标记；克隆正文保留被调用方的逻辑来源。callee 进入计数不能因为物理调用消失而丢失，也不能在保留调用时重复计算。
- 块拆分、合并、常量折叠和循环变换维护计数位置关系。已证明不可达的位置可标为静态消除；不能把“没有发射计数器”默认为“训练观察到零”。
- 原位置已被纯计算替代但其逻辑执行仍需要计数时，保留相应采集标记，不要求保留原调用或原计算。
- 无法可靠映射的局部位置标为 unavailable，相关消费者回退并报告覆盖缺口，不猜测次数。所支持变换的映射正确性是插桩实施的必要验收项。

首版聚合相同逻辑位置在不同内联副本中的次数，同时保留物理记录到内联来源的映射。它不宣称提供完整上下文敏感 PGO；不能给每个内联副本重复套用整个 callee 的总次数。未来若需要按调用链区分，另增加实际需要的上下文维度。

探针尽量在 MIR 优化之后、LLVM 代码生成时物化。LLVM 仍需要遵守计数操作的实际执行语义；“不计入 cost”不意味着可以假定插桩完全不扰动后端优化或运行性能。

### 6.3 物理计数记录

Scoop 当前按 callable 生成 LLVM module/对象。采用每个实际发射 callable 自己持有物理计数记录的方式，记录包含其保留位置及内联副本的位置；旁置映射把每个 counter index 关联回逻辑位置和正文 fingerprint。

LLVM 的 record name 使用稳定的机器正文身份编码，record hash 描述这份物理计数布局。反馈阶段先通过训练时保存的映射还原逻辑位置，再与当前优化前的正文匹配；不要求两次构建产生相同的优化后 CFG 或物理 counter 编号。

同一 ODR 机器正文的计数、profile data 和相关名字必须与其代码采用一致的选择/裁剪规则。被链接器丢弃的副本不计数，未引用的 profile metadata 不得把死函数全部保活。Strong/provider 归属保持原规则；不为了给跨 Cone 内联副本计数而新增外来 Strong 定义。

每个计数记录与映射只在明确边界验证一次。随产物保存的映射是解释采集数据所需的正常编译结果，不能从 symbols 观察或函数显示名反推。

### 6.4 工具和数据格式

代码生成使用 LLVM 22.1 的 `llvm.instrprof.increment` 等必要 intrinsic，并在配套 pipeline 中执行 InstrProf lowering。链接目标对应的 compiler-rt profile runtime，生成 `.profraw`；使用同一套工具链中的 `llvm-profdata merge` 生成 `.profdata`。

保留 LLVM 的原始和 indexed 格式，仅增加 Scoop 必需的位置映射与数据索引。不能把自定义 MIR counter 布局直接交给 LLVM 的 IR-PGO use pass 并假定它能够理解；首批反馈由 Scoop MIR 消费，后续按需要将已匹配的分支概率转换为合法 LLVM `!prof` 信息。

indexed profile 的解码放在已经依赖 LLVM 的 `scoopc` 适配边界，通过最小 C++/C FFI 使用 LLVM InstrProfReader。`scoop-toolchain` 只负责路径与资源选择；`scoop-pgo` 调用配套工具，不直接链接 LLVM。给 mir-lower 的是纯 typed 频次事实；MIR/meta crate 不依赖 LLVM reader 或重建语言语义，优化循环也不反复读取 profile 文件。

LLVM 22.1.8 的 `llvm-profdata show --show-format=json/yaml` 对 instrumentation profile 明确报不支持，不能将它作为现成的 JSON 读取接口，也不解析默认的人类输出。便利命令需要的逻辑统计由同一份 `scoopc` reader 通过拟定的 `scoopc profile-summary <profile.json> --message-format json` 提供；该入口只读数据索引和计数，不编译源码。首版 merge 不启用删除零记录的 sparse 模式，保留“已覆盖但计为零”与缺失记录的区别。

格式/schema 不支持、文件损坏与 I/O 失败是输入错误；正常的正文变化或某个函数没有记录是可回退情况。原始格式不承诺跨 LLVM 版本兼容；工具升级后不增加历史格式兼容层，需要时重新训练。

## 7. 数据如何进入现有 cost 模型

### 7.1 第一批反馈规则

保留现有静态收益、代码增长、复杂度、caller 规模和合法性判断。反馈构建先执行普通 release 的默认内联、去虚拟化和配套化简，再对剩余调用及新暴露的机会使用反馈追加内联。冷计数或嵌套 callee 的低相对频率不能否决默认阶段已经完成的展开，新增热点也不能提前占用默认候选的增长额度。比较基准须使用相同源码、目标、语义选项及模板集合。

两阶段各沿用最多三轮，复用同一会话的稳定模板、分析、内联 ancestry 和累计 caller／region 增长记录，进入反馈阶段时不重置已经消耗的增长量。普通 release 和采集构建只执行默认阶段。反馈阶段只替换尚待选择位置的热度来源，并复用现有排序与增长规则；最后的字符串优化仍使用相同来源的块频次。本次不调整普通 release 的全局阈值，也不实现内联上下文的精确计数传播。

当 caller entry 次数大于零，且分子/分母覆盖相同的执行范围时：

```text
relative_frequency = call_count / caller_entry_count
F = clamp(relative_frequency, 1/8, 32)
```

首版沿用当前静态热度的上下界校准评分，不直接把训练绝对次数乘进收益。原始计数和未截断比例仍完整保留；分支概率也使用原始比例。增长仍采用既有代码体积限制，PGO 不建立新的通用资源预算。

同一 caller 中，一个调用点训练执行 1,000,000 次、另一个只执行 10 次，会得到不同的频率；把整个训练重复十次不会改变两者的相对收益。全局函数次数用于覆盖和报告，首版不再额外引入一套全局热度乘数。

| 数据状态 | 处理 |
| --- | --- |
| 位置和布局匹配，计数可用 | 使用动态热度，报告 `profile` 来源 |
| caller 已进入，特定调用点确实计为零 | 作为观察到的冷点参与收益判断；保留合法可执行路径 |
| caller entry 为零 | 不计算 0/0；报告未执行，局部比例使用静态回退 |
| 缺记录、正文改变、位置 unavailable | 该位置使用现有静态热度并记录原因 |
| 只观测到部分内联/库执行范围 | 只对同一覆盖范围内的分子分母作推断；不能冒充完整函数计数 |

数据记录中的零、缺失和不匹配必须是可区分状态。不能把未插桩预编译库中的函数当作从未执行，也不能从训练样本证明动态目标永远唯一。

### 7.2 决策报告

复用 `--emit optimizations`，增加逻辑采集位置、计数来源、原始分子/分母、实际使用的热度、匹配/回退原因。保留原有是否有正文、是否合法、是否值得内联及实际增长原因，不能让“没有 profile”遮住“没有可用模板”。

默认阶段的选择继续报告静态热度，`heat.profile` 为 null，不计为反馈消费者；反馈阶段报告实测或回退原因。两阶段轮次连续编号，只有所有实际执行阶段都观察到无改写轮次时才报告 `converged`。匹配记录数、实际消费及最终性能分别判断，保留默认内联不等于冻结其他 IR 变换或承诺所有负载加速。

构建成功摘要报告：参与反馈的源码 Cone、预编译依赖覆盖、匹配函数/位置数、过期或缺失数，以及实际消费反馈的优化种类。记录数只是覆盖指标，不宣称等于程序耗时覆盖率。

显式低层 `--profile-use` 可在缺数据时完成静态回退。便利命令固定为正常 build 附加 `--profile-use-required`，把“至少有一项匹配事实”作为本次反馈请求的要求；由构建链路在最终链接/发布之前检查，没有匹配事实则失败并提示重新训练。不能等 binary 已经覆盖输出后才由包装命令撤回成功，也不能把完全未使用数据的构建报告为完成 PGO。

## 8. runtime、退出与发布产物

### 8.1 运行期输出

计数文件、runtime 指标和错误信息使用不同通道。计数和指标写入采集目录；业务 stdout/stderr 不被统计 JSON 替换。直接使用现有显式 GC/startup 统计开关时，仍按其原合同输出。

profiling runtime 复用已有计数和计时位置，只补必要采集输出。GC 的完整 snapshot 目前涉及锁、region 遍历和 RSS 查询，不应在每个热点事件上调用；事件路径记录当时已有的标量数据，结束或显式输出时再整理。

启动时间以现有 runtime 测量边界为准，不能把 root gateway 之前的统计称作包含动态加载器、C 启动和完整程序生命周期的总时间。新增 GC 时间线同样明确各字段单位与边界。

### 8.2 flush 与进程状态

compiler-rt 的默认 atexit 不能覆盖 Scoop 的全部结束路径：当前显式退出最终会进入 `_exit`，fatal 路径也可能直接 abort。需要在 runtime 规范中明确：

- 正常 main 返回：在现有线程/GC 收尾完成、运行指标仍可读取时输出，计数正常 flush。
- Scoop 显式 exit 和可正常处理的未捕获异常终止：在最终 `_exit` 前尝试 flush，不改变原来的退出语义或等待原本不会等待的后台线程。
- 显式 flush 与 atexit 共用一次写出控制，避免同一批计数因两条路径重复合并。
- fatal abort、强制杀死或进程崩溃：不保证生成完整数据，不在任意信号上下文执行复杂文件写入或 GC 遍历。
- native-app：使用其实际初始化与宿主进程退出路径，补对应采集收尾；不假定它经过普通 executable 的 main wrapper，不顺带增加卸载协议。

业务非零退出码与 profile 文件是否可用分别记录。正常写出的有效数据可以用于反馈，`instrument` 仍返回被训练程序的非零状态。采集写入失败明确报告；业务成功但未得到所需训练数据时，便利命令整体失败。

函数计数的热路径不调用可能分配或 safepoint 的 Scoop/runtime 函数。元数据与计数数组使用 native 静态存储，不保存 managed 指针。确需新增 runtime 入口或 image 字段时同步 ABI/格式规范，不通过临时后门参数传递。

### 8.3 反馈二进制必须去掉探针

训练用 `.slib` 的机器码包含探针。反馈阶段必须按 `Release + Use` 重新构建需要优化的源码 Cone，使用常规更新键区分它与训练产物，不能只更换最终链接选项。

普通发布/反馈链接不得静默选入仍带 Generate 机器码的预编译库。发现此类输入时，要求选择对应的无插桩库或从源码重建；不能把“成功链接 compiler-rt”当作去掉探针。

profiling 可以链接用户显式选择的无插桩第三方预编译库，报告该部分机器正文未覆盖；默认系统库则必须按第 10.1 节选择专用 profiling 组件，不静默回退到普通系统库。消费方实际展开的模板仍可在本次编译中采集。full 只增加正文可用范围，既不自动重建 provider，也不保证其所有函数最终被展开。

## 9. `scoop-pgo` 的命令与流程

### 9.1 安装与职责

实现为独立的 `scoop-pgo` 可执行程序，安装到 `<SYSROOT>/commands/`，公开入口为 `scoop pgo`。Windows 对应 `sco pgo` 和 `scoop-pgo.exe`。建议源码放在 `tooling/scoop-pgo/`，沿现有外部命令的构建、打包和许可证收集流程发布。

命令负责配置复用、正常 build 调用、训练进程、数据合并与结果说明；不实现 HIR/MIR/LIR 变换，不解析项目语言语义，也不另建构建缓存。

内部调用配套主驱动的正常 `build --message-format json`，从本次成功记录取得 actual target、root、输出、采集映射和实际构建设置；不解析人类日志，不打开 `.slib` 推导这些信息。build 失败时不运行旧 executable。必要的 build 参数解析/规范化只为这两个真实调用方作最小复用，避免再维护一套不同默认值。

配套驱动定位不走任意 PATH 搜索。为支持 `scoop --sysroot X pgo` 及共享 commands 的实际调用，外部命令分发向子进程传递父驱动绝对路径 `SCOOP_DRIVER` 与解析后的 `SCOOP_SYSROOT`；仅设置子进程环境，不改用户全局环境。直接运行独立命令时，可用 `--scoop`、`--sysroot` 指定，缺省按命令安装根定位 `bin/scoop` 或 `bin/sco.exe`。这两项分发补充在实现前写入实现规范，不假设当前分发已经传递它们。

### 9.2 日常入口

```sh
# 首次使用时单独安装当前 target 的组件；使用提供该组件的发行版本。
scoop-install component add profiling --target aarch64-apple-darwin

# 构建并运行第一份训练输入。
scoop pgo instrument . -- data/train-small.json

# 同一构建配置下追加训练；构建输入未变时复用已有结果。
scoop pgo instrument . -- data/train-large.json

# 复用构建配置和数据，生成不带探针的优化程序。
scoop pgo optimize -o dist/app
```

命令语法：

```text
scoop pgo instrument [root-input] [build-options...]
                     [--data-dir DIR] [--no-run] [-- program-args...]
scoop pgo optimize   [root-input] [build-options...]
                     [--data-dir DIR] [-o BINARY]
```

root 选择沿现有 manifest/workspace/single-file 规则，必须得到一个可执行根；多 root 时由现有 package 选择收窄。optimize 省略 root 时使用保存的选择，不重新猜一个训练对象。

`--data-dir` 默认是调用工作目录中的 `target/pgo/`，两个命令相同；不搜索“最近一次训练”。不同项目、target 或需要独立管理的训练集合使用不同目录。`--target-dir` 仍是正常构建输出/缓存选项，不暗中改变这个训练数据目录。

| 设置 | instrument | optimize |
| --- | --- | --- |
| profile 与插桩模式 | 固定 Profiling/Generate | 固定 Release/Use |
| export-ir | 对本次生产的源码 Cone 默认 full，可显式选择三档 | 默认复用训练配置；变更需要新训练集合 |
| 默认系统库 | 可选组件中的 Profiling/full 库 | 同组件中的无插桩 Release/full 库 |
| target、root/member、generator、依赖和 native 构建设置 | 使用正常 build 参数并保存解析结果 | 默认复用，显式调整按下节处理 |
| debug info | 默认 line-tables | 默认恢复 release 的 none；显式配置可覆盖 |
| executable 输出 | 数据目录的 `instrumented/` | 数据目录的 `optimized/`，或显式 `-o` |
| 程序参数 | 仅 `--` 后的原始 argv | 不运行程序，也不接收训练 argv |

用户不需要传 LLVM pass 名、profile runtime 路径、计数文件名、`--profile-use` 或 `--profile-use-required`。与工具固定模式冲突的 profile、profile-use、emit-slib 等 build 参数直接报错，不靠选项出现顺序覆盖。

### 9.3 数据目录与参数复用

```text
target/pgo/
  build.json
  builds/<build-id>/<ordinal>.profile.json
  instrumented/app
  runs/<run-id>/
    run.json
    <process-run-id>/counts.profraw
    <process-run-id>/runtime.jsonl
  merged/
    counts.profdata
    profile.json
    summary.json
    state.json
  optimized/app
  optimization.json
```

实际可执行文件扩展名按 target 处理。build-id 复用正常构建结果的 link fingerprint 以区分位置映射，run-id 只用于文件不覆盖；两者不成为新的实体身份或产物真实性凭据。

`build.json` 保存版本、原构建 cwd、已选择 root/member、target、配套工具/sysroot、语义相关 build 设置和当前训练输出位置。路径按现有平台路径编码保存；相对 build 路径在首次构建时按原 cwd 解析，不能因 optimize 换了工作目录而改变含义。每次训练另外记录其运行 cwd、argv、进程状态和数据路径，不持久化整份环境或 stdin。

工具固定的 generation/use、输出路径和默认 debug-info 属于阶段设置，不能把 instrument 的值盲目重放到 optimize。jobs、cache、target-dir、输出和诊断设置可显式覆盖；工具/sysroot/预编译库位置可为反馈阶段选择配套的无插桩资源，依赖兼容性沿正常构建与数据匹配判断。

同一目录不能混入另一个 root、target、导出策略或不同语义构建选项的训练。需要比较这些配置时使用另一个 `--data-dir`。源码正常修改仍允许继续训练，保留各次实际映射；反馈只聚合同当前正文匹配的记录，不把旧 counter index 套到新正文上。

本地构建配置与 profile 数据索引分开。`merged/profile.json` 使用相对引用记录 indexed profile、位置映射、target 和必要格式版本，便于作为低层 `--profile-use` 输入；它不复制源码、整个依赖图或缓存凭据。`summary.json` 给出可读的函数/位置统计及覆盖，runtime 时间指标独立保留，不并入计数比例。

### 9.4 instrument

1. 解析本次 root 和设置；首次训练保存设置，追加训练核对目录所代表的配置。沿正常 PM、源码依赖、缓存与工具链流程构建 Profiling/Generate。
2. 仅接受本次成功 build 返回的 executable 与 profile map，保存实际位置映射；编译失败不更新成功构建记录、不运行旧程序。
3. 创建独立 run 目录，为训练子进程设置 `SCOOP_PROFILE_DIR`。Scoop runtime 使用 LLVM buffer 写出计数，并在目录内为每个进程创建独立子目录；不使用 `LLVM_PROFILE_FILE` 模板，因此外部 LLVM 路径设置不影响收集位置。原始程序参数、stdin/stdout/stderr 原样传递，不经 shell 拼接。
4. 等待程序退出并保存真实状态。由目标 profile runtime 写出的完整 raw 文件进入本次合并输入；缺失、格式损坏或输出失败明确诊断，不把退出码零当作已有数据。
5. 使用配套 `llvm-profdata merge` 合并本目录已完成训练的有效记录，生成 indexed profile、Scoop 索引和摘要。正常写出数据的非零业务退出同样可以提供样本，命令仍传回业务失败状态。

默认追加而不清空旧样本。重复训练按实际计数累加，先合并整数计数再计算比例，不对每次运行的百分比作简单平均；首版不增加权重调参接口。

`--no-run` 完成构建和映射保存，打印可执行文件及采集目录的运行方式，供需要另行启动的程序或非本机可执行 target 使用。外部运行结束后将采集文件放回指定 run 目录，再执行 optimize；没有数据时不继续反馈。工具不隐式选择模拟器、远程执行器或 native-app 宿主。

### 9.5 optimize

1. 读取选定目录的构建设置和训练文件；必要时重新合并，包括 no-run 后产生的外部训练数据。输入文件缺失/损坏或合并失败时停止，不静默使用先前的旧 merged 结果。
2. 按保存的 root、target 和构建设置调用正常 Release/Use 构建，传入 `merged/profile.json` 并要求至少一项匹配。当前源码和依赖照常判断更新，匹配不上的局部数据交给静态回退。
3. 构建链路先完成匹配要求及编译/链接检查，再发布成功结果；便利命令据此报告匹配范围和实际反馈消费者。没有任何匹配事实、编译失败或链接失败均遵守正常失败不覆盖规则。
4. 成功后输出新的 binary 路径及 `optimization.json`。默认路径与训练程序、普通 release 输出分开；不自动运行业务负载或替用户判断性能是否提升。

同一数据目录的修改用普通文件锁串行协调，不建立后台任务服务。合并及索引通过临时文件在全部成功后发布；不对旧缓存进行内容审计。工具摘要写 stderr，机器可读结果保存在上述 JSON 文件，程序标准流保持原样。

### 9.6 预编译库与覆盖范围

命令为默认系统依赖自动选择已安装的 profiling 组件。该组件的系统 `.slib` 固定为 full，应用的 `--export-ir` 只影响本次生产的源码 Cone；应用选 full 不能恢复第三方预编译库已经剪掉的模板，也不能给其机器码补探针。系统库和第三方库的覆盖分别报告。

instrument 使用组件中的 Profiling/full 系统库，optimize 自动切换到同组件的 Release/full 库，无须用户传两套库路径。默认系统库组件缺失时给出对应 target 的安装命令，不在线下载、不重建安装 sysroot、不静默降为部分系统库采集。

首批反馈优化本次重新编译的源码及在其中展开的系统库正文。仍保留为外部调用的系统函数使用配套 Release/full 预编译机器码；该机器码没有探针，也没有依据当前应用的 profile 重新优化。对 provider 独立机器正文做 PGO，需要从其源码经正常 Cone 构建产生新的库，属于后续明确的库反馈构建能力，不能靠复制外来 Strong 或读取 full 模板在链接时补做。

训练与反馈应保持同一目标、语言/ABI 配置和模板导出策略。机器模式从 Generate 切到 Use 属于必要差异，不要求二者产物 fingerprint 相同；是否可用由实际逻辑正文和计数布局匹配决定。

## 10. 构建、缓存与工具链配套

### 10.1 可单独安装的 profiling 系统库组件

沿用[发行组件设计](../distribution/DESIGN.md)和[发行索引](../distribution/MANIFEST.md)，增加每 target 一份的可选组件：

```text
component: lib-profiling-<target>
archive:   scoop-<version>-lib-profiling-<target>.tar.xz
installer: scoop-install component add profiling --target <target>
```

组件不进入普通默认安装，可在同一版本安装后追加下载、列出和移除；首次安装也可显式选中。归档顶层仍是安装根内容，与普通 base/gui/native 组件并存。同一个组件内提供训练和反馈所需的配套库，避免让用户再选择另一个用于关闭探针的组件。

| 组件内容 | 构建设置 | 用途 |
| --- | --- | --- |
| 训练系统 `.slib` | Profiling/Generate、export-ir=full、line-tables | 全部系统 Scoop 正文具备实际采集能力，保留完整模板 |
| 配套无插桩系统 `.slib` | Release/Off、export-ir=full | 反馈编译有同样的模板范围，最终机器码无探针；不冒称已应用用户 PGO 数据 |
| 训练位置映射 | 与训练库的实际记录和计数布局对应 | 无须重建系统库即可解释 raw 数据并反馈 |
| profiling runtime 和目标 compiler-rt profile 库 | release 优化及必要观测设置 | 收集指标、正常写出计数、提供正确目标的链接输入 |

两组 `.slib` 从同一 release、同一目标的相同系统库源码生成，**全部使用 `export-ir=full`**。配套 Release/full 库是本设计建议的必要搭配：普通系统库仍可采用 default，单靠它不能保证保留训练时已有的全部跨 Cone 候选；直接重用训练库又会把探针带进最终程序。

该 target 发行的系统 Cone 全部纳入，两组集合一致，包括该 target 提供的 GUI Cone。只增加这一个可选库组件；既有 base/gui/native 的第三方 native 链接资源继续复用，不重复打包 Skia、OpenSSL、SDK 等。安装 profiling 需要同 target 的 base 和原有必要 native 组件；使用 GUI 库时仍按正常要求安装 gui 组件，不因只安装 profiling 就自动下载 GUI native 资源。

拟定安装布局：

```text
<SYSROOT>/lib/<target>/
  *.slib                         普通系统库，维持既有选择
  runtime*/                      普通 runtime 配置
  profiling/
    instrumented/*.slib          Profiling/full 系统库
    release/*.slib               Release/full 系统库
    instrumented/*.slib.profile.json  与训练库同名的计数位置映射
    runtime*/                    profiling runtime、对象索引及 profile.a（compiler-rt）
```

采集元数据是组件的正常内容，不依赖用户安装 vendor 或在本机生成。位置映射在首次形成训练数据集时随本次构建结果保存，后续仍可解释原训练数据；不在每次缓存命中时重新读取全部系统库或复核其内容。

系统库选择由 build 配置决定，在依赖选择阶段确定具体路径，再进入现有缓存/编译流程：

| 构建 | 默认系统 `.slib` 位置 | runtime |
| --- | --- | --- |
| debug 或普通 release | `lib/<target>/` | 原有配置 |
| profiling | `lib/<target>/profiling/instrumented/` | 组件中的 profiling runtime，链接目标 profile 库 |
| release + profile-use | `lib/<target>/profiling/release/` | 原有无插桩 runtime，不链接 profile 库 |

安装 profiling 不改变普通 debug/release 的默认选择。不能把上述目录都作为无序搜索路径后任取同身份 `.slib`，也不能在一个系统依赖闭包内静默混用训练库与普通库。显式第三方依赖仍采用用户选择；维护生产所需的源码/依赖/runtime 输入继续走既有低层路径，不以“组件已经安装”限制组件自身的构建。

系统库的 Cone 坐标和版本保持相同，变化的是本次绑定到的本地预编译路径。scoop 将已选系统库根传入现有 PM/依赖准备接口，PM 不读取运行计数或决定优化；profile 不成为新的 ConeIdentity 或 lock 中的 package 版本。

发行 manifest 的 `targets.<target>` 新增 `profiling = "lib-profiling-<target>"` 引用，对应归档仍使用已有 path/size/SHA-256 字段和首次下载检查。不存在该引用表示该 release/target 尚未提供组件；存在但本机未安装时，profiling/PGO 命令提示安装。日常构建使用已解析的本地路径，不读取发行归档作验证。

Scoop 维护脚本负责预编译两组库、组装 runtime/映射及发布组件。独立 `scoop-install` 仓库同步组件选择、追加安装、移除、升级保留选择与 target 移除行为；官网/bootstrap 转交组件选择。安装过程只取得预编译资源，不要求 Cargo/CMake 或现场编译系统库。移除组件只移除其专用目录，普通库、源码和其他组件保持可用。

### 10.2 编译配置与缓存

需要进入正常编译更新输入的内容包括：采集模式/布局版本、IR 导出策略，以及 Use 模式所选反馈数据的更新状态。profile 文件作为实际编译输入参与更新判断；命中且结果存在时直接复用，不重读内容、重新合并或探测工具来证明缓存仍可信。便利命令只在训练集合实际变化或合并结果缺失时执行合并。

计数映射是 Generate 编译所需结果之一，随相应编译结果保存；反馈决定改变机器码，因此不能按纯观察开关处理。运行参数、采集输出目录和本次 runtime 指标不进入源码编译键。

产物记录实际机器码是否含 Generate 插桩，以及所用导出策略，供正常消费和最终链接选择；Use 影响代码更新输入，但其产物的机器码不带采集探针。共享 HIR/泛型/default 模板仍是未插桩的语义正文，不嵌入训练计数或某次运行的状态。

profiling 的 C/native 编译仍选择 release 优化设置；用户 C/C++ 源码不自动成为 Scoop MIR 反馈位置。需要对 native 代码做 PGO 时，另按真实工具链能力接入，不能将其计数混作 Scoop callable。

### 10.3 工具与各模块接线

当前 LLVM 打包脚本只包含常用 clang/lld/符号工具，未打包 `llvm-profdata`，也未构建 profile runtime。实施需补：

- LLVM 22.1 对应的 `llvm-profdata` 随宿主 bin 提供；compiler-rt profile 目标库随上述 profiling 组件提供。只构建实际需要的组件。
- profiling runtime 的必要计时/输出配置、对象索引和最终链接选择，复用已有 runtime 构建入口，不机械展开所有布尔设置的组合。
- `scoop-pgo` 宿主可执行程序、命令安装路径、配套版本及实际依赖许可证。
- Generate 产物、记录、计数数组与 profile runtime 在 Mach-O、ELF、COFF 下的链接和裁剪合同。

主要接线位置如下；stage 实现仍只经相应 IR 数据通信：

| 位置 | 改动职责 |
| --- | --- |
| `compiler/scoop`、`compiler/protocol`、`compiler/pm` | profile/导出/反馈请求，已选系统库根的依赖绑定，正常 build 参数及结果记录 |
| `compiler/hir`、`compiler/hir-lower` | 导出选择、完整模板与使用处导入 |
| `compiler/mir`、`compiler/mir-lower` | typed 采集位置/事实、优化中映射维护、cost 消费 |
| `compiler/lir`、`compiler/lir-lower` | 将实际计数位置和必要来源完整传向后端 |
| `compiler/codegen`、`compiler/driver` | InstrProf 生成、计数读取适配与 summary 机器入口、结果编排 |
| `compiler/slib`、`compiler/linker`、`compiler/toolchain` | 必需产物配置、记录选择、目标 profile 资源与最终链接 |
| `runtime`、`tooling/scoop-pgo`、维护/发行脚本 | 采集生命周期，训练/反馈流程，独立组件生产和安装配套 |

工具定位必须使用 Scoop 配套 LLVM；不能因 PATH 中另一个 `llvm-profdata` 恰好可执行就混用格式。实际原生目标、LLVM 版本和完整流程验证结果见 [VALIDATION.md](VALIDATION.md)。

## 11. 实施顺序与验收

### 11.1 分批交付

| 批次 | 交付内容 | 完成条件 |
| --- | --- | --- |
| A：计数原型 | 单函数/跨对象/内联/ODR 的 LLVM InstrProf 接线，固定 MIR 位置与映射 | 已知次数正确，原子多线程计数正确，死代码裁剪不被破坏，Generation/Use 可在同一逻辑位置匹配 |
| B：导出策略 | off/default/full、模板表示补齐、producer/importer 分工、构建键 | off 保留语义必需模板；full 不因当前内联限制丢正文；consumer 不复制外来 Strong |
| C：profiling | 第三种 build profile、控制流/调用计数、runtime 总指标、退出写出、独立系统库组件 | 单独安装组件后能直接构建/运行并解释系统库计数；两组库均为 full；默认 release 没有新增探针 |
| D：首个 PGO 闭环 | typed 数据读取、调用点与字符串热度反馈、`scoop-pgo instrument/optimize`、覆盖诊断 | 两条命令完成训练与新 binary 生成；参数复用正确；真实负载有决策变化与性能对照 |
| E：后续观测 | GC phase/slice 时间线；按消费者需要增加目标/分配分布 | 指标边界准确，采集成本已测量，具体消费者能够使用 |

这些批次遵循依赖关系，允许独立交付 B 与 C 的已完成部分。A 需要先确认物理记录所有权、计数映射和 pass 顺序；不能仅靠 `opt --print-passes` 中出现 InstrProf 就宣称方案闭环。

### 11.2 必要正确性验证

- 用确定负载覆盖分支、循环、递归、异常、同块中途抛出、同一 callee 的多个调用点；核对计数含义，不能只检查文件存在。
- 覆盖优化前位置到内联/拆块/折叠后的映射、不同泛型实例及同一源行多个位置；验证旧正文数据局部失效和真实零计数。
- 对 off/default/full 验证泛型/default 的必要组合，以及普通方法、访问器、构造器、private helper、局部 callable；验证 full 正文可导出但仍因合法性/cost 保留调用的情况。
- 验证跨 Cone mixed coverage、Strong 归属、ODR 选取与计数、dead-strip、常量/GC roots，以及反馈结果未混入训练机器码。
- 覆盖多线程计数、main 返回、显式 exit、未捕获异常、native-app 生命周期、写入失败和训练非零状态。fatal/kill 只验证明确的失败/数据缺失行为，不要求不可能保证的完整 flush。
- 命令验证 argv/空字符串/平台路径、workspace 唯一 root、配置复用、追加和合并、no-run、失败不运行旧程序/不发布新成功结果，以及正常缓存命中和实际输入变化。
- 发行/安装专项验证组件缺失提示、单独追加下载安装、两组 full 模板、系统库实际计数、训练/反馈自动选库、GUI native 资源复用，以及移除组件后普通构建仍正常。使用真实输入和正常安装状态，不用破坏缓存证明复用。

普通库行为用 Cone 测试。命令行、进程控制、IR 优化、产物、链接/ABI 等使用必要 fixture；不把 profiling 变成普通 API 测试的新全量模式，不通过移走源码或工具制造缓存验收。

目标验收覆盖实际支持的 Darwin、Linux GNU/musl、Windows MSVC/MinGW 组合，每个宣称支持的目标都需完成原生 instrument → merge → optimize → run。阶段记录只标实际通过的目标，其他目标明确待验证，不由单一宿主推定。

### 11.3 收益与开销验证

分别测量普通 release、profiling 和 PGO release。记录训练插桩的墙钟/CPU/RSS、代码与数据体积；比较 default/full 的库体积、生产成本和 consumer 编译成本。正式性能对照关闭采集，使用相同语义输入、预热与交错运行。

PGO 收益至少包括一个同函数不同调用点热度使现有评分改变的代表用例，并使用独立于训练输入的负载检查泛化。对内联变化、代码增长、GC/分配和最终性能作对应解释；没有净收益或有退化的结果照常记录，不能以“成功读入 profile”替代优化有效性。

### 11.4 规范与使用材料

实施前按批次同步实现规范中的 profile、stage 配置、`.slib` 模板、编译/链接、观察、外部命令和发行组件合同；runtime 的指标、写出时机或新增入口同步运行时规范。发行设计/manifest、独立安装器和官网同批更新 profiling 组件选项。语言语义不因导出/PGO 改变，涉及其引用时核对语言规范。

能力交付时，按[编程材料更新工作流](../scoop-programming/UPDATE-WORKFLOW.md)同步 CLI 帮助、编程 skill 的命令/性能指导、必要可运行示例、scripts/tooling 与发行说明。上述材料已随首版实现同步。

验证记录仅保存范围、结果、失败闭合与限制；原始 profile、训练 binary、报告和计时日志放在仓库外，不提交进设计目录。

## 12. LLVM 参考

- [InstrProf increment intrinsic](https://llvm.org/docs/LangRef.html#llvm-instrprof-increment-intrinsic)：前端定义计数位置及 lowering 入口。
- [Instrumentation Profile Format](https://llvm.org/docs/InstrProfileFormat.html)：raw/indexed 格式、记录与计数布局。
- [Clang instrumentation profiling](https://clang.llvm.org/docs/UsersManual.html#profiling-with-instrumentation)：生成/合并/使用流程、输出文件模板及 atomic 更新。

这些资料说明可复用的 LLVM 能力。Scoop 的 MIR 位置含义、跨 Cone 正文归属、GC 契约及命令行为仍由本设计与对应规范决定。
