# M23-6：普通编译器职责与过度设计清理

代码生成在入口对本次完整且不可变的 LIR、目标 profile 和 executable 入口完成一次必要验证，然后按实际定义分成函数与非函数对象。各成员直接消费同一 LIR，不因发射另一个对象再次完整遍历类型、ABI、CFG、GC roots、safepoint 或身份表；LLVM 变换后的 IR 和新生成的对象字节仍在各自边界检查。generated-C 源码入口同样不在内部 helper 重复整模块验证。这一职责调整不改变产物格式、runtime C ABI、String 表示或链接语义。

外来参数自由 struct 的源码类型引用使用依赖中实际的 typed nominal 声明及完整字段类型；名称仅用于查找和诊断。HIR 保留其完整成员、继承、构造器和字段声明，参数、结果、局部存储及嵌套字段使用同一类型。LocalConcrete/MIR 可以保存计算值表示所需的完整字段，同时保留真实定义 Cone；这些表示记录不是当前 Cone 的源码声明或机器定义。成员与构造器调用仍引用定义方 callable，layout、TD、ABI 和 relocation 由共有依赖选择取得；不能复制外来函数正文或在消费 Cone 重新发布其 Strong 定义。实际跨 Cone 类型使用无需来源凭证、工厂资格或重新认证 provider。此接通使用已有 wire 类型和布局记录，不改变 runtime C ABI 或 String 表示。

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。

嵌套声明的源码 payload 只保留实际声明身份与完整源码接口，不重复携带可由该身份推导的 inheritance exact、representation owner 或泛型/非泛型资格标志。是否存在机器表示由同一产物的实际 representation、inheritance、MIR 与 LIR 表表达；源码完整但尚未物化的声明仍可正常发布。reader 在这些表的消费边界检查引用与表示一致性，不因解析嵌套源码声明再次完整验证同一表示。原 payload 的 field 3 退役且不得复用；HIR `cross-cone-type-semantics` capability 升至 5，旧产物按版本规则重建，runtime C ABI 不变。producer 的类型事实与表示投影必须覆盖共有声明接口中实际需要的私有存储与嵌套支持类型，不能只依据 public binding 根的局部列表。

本设计落实最新版 AGENTS.md，并明确修正旧目标和设计中保留通用预算、来源证明、任意成本模型及重复完整验证的条款。清理覆盖 compiler、slib、runtime 和所有相关规范、格式、测试，不限于 core；将 core 专用机制改成所有 Cone 共用的机制不等于删除过度设计。

配套：[语言规范 12.6](../../specs/SCOOP-SPEC.md#126-核心库)、[实现规范 2.12](../../specs/SCOOP-IMPL-SPEC.md#212-core-普通库的共有验证)、[运行时规范](../../specs/SCOOP-RUNTIME-SPEC.md)、[M23 总设计](../DESIGN.md) 和 [M23-6 设计](DESIGN.md)。

## 1. 边界与完成条件

保留并完成 M23-6 的类型布局、canonical ABI、dispatch、ZST、跨 Cone 实际消费和产物发布。core 是普通 library Cone，用户可修改、扩展和重建；sysroot 只是默认查找位置。前端负责 intrinsic 识别和语言规则，后续阶段只消费完整 typed IR、实际声明及 provider。

保留全局唯一且类型化的实体 ID、类型与可见性、格式/引用、依赖环、内容 fingerprint、缓存失效、符号/ABI、对象范围和 GC 契约。普通来源位置、依赖路由和声明引用有实际语义用途，不按名称机械删除；它们不承担授权、防伪、反篡改或资格证明职责。

## 2. 七组清理

### 2.1 canonical ABI

删除 `ConeIdentity::CORE` 的验证跳过、豁免和专用空表限制。从实际签名、exact type、target、布局及 provider 使用共有规则，空 expectation 自然成功。普通 callable bridge 与 layout profile 均核对真实 ABI，但同一不可变签名/布局不因消除豁免而多轮完整重算。

### 2.2 native boundary

删除 `TrustedCore`、`ImportedCoreNativeBoundaryTypes`、`CoreNativeBoundaryNominal` 及 external-core 专用恢复路径。共有依赖类型查询提供真实 nominal、字段/variant、参数、CLayout policy 和表示。后端消费正规化 typed representation，保留 FFI 的 C-safe、GC-free、布局、调用约定和根契约；Unit/Any 维持其语言内建身份。

### 2.3 compiler protocol

删除 CORE 来源资格、Core/NotCore 包装、重复 operation/签名投影和凭证外层。语言角色只保存实际需要的 typed 声明引用、签名、effect、可见性与依赖关系。普通 metadata 发布和消费这些声明；intrinsic 正规化后不再从来源获得执行资格。internal 服务不因迁移加入 public lookup。

### 2.4 String、初始化与 Link

String descriptor 使用完整 MIR 中实际声明的 source exact identity，沿共有 descriptor 查询、layout selection、physical import、registration 和 Link relocation 消费。删除独立 String bridge 与固定角色的 descriptor 恢复通道，不以 provider 坐标或协议来源豁免普通引用检查。Strong production `/7`、`/8` 退役原服务表中的 TD tag 2；旧产物与缓存重建，String 表示及 runtime C ABI 不变。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

Strong production 的两种表示升级为 `/9`、`/10`，删除初始化专用 ABI field 11 和外部服务表 field 12，section 只保留 field 2～9 的八字段 product。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 registration reader 完成结构与引用检查后直接返回完整 registration production 数据，不保留仅用于限制编码或导出资格的中间凭证包装。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

LIR descriptor 依赖是明确的 typed IR 引用。reader 验证可达 provider、exact type 的实际 descriptor 导出、递归依赖闭合及物理定义，不要求 descriptor 先出现在 HIR 的 value-layout 或 shape-support 物化根中，也不从协议表重新证明其来源。显式 descriptor relation 与从源码类型使用得到的 layout/shape 根共同闭合；遗漏必要依赖、错误 provider/类型与不一致物理合同仍拒绝。

共有 Link import 表可以保留没有实际 relocation 的完整类型或 callable 引用；这类声明不生成虚构的 machine use。Link 只对实际 relocation 生成 requirement，并验证目标、provider、symbol、ABI 和定义一致性。删除“每个已声明 import 必须至少出现一次”的附加证明，以及初始化服务的专用豁免；缺失实际引用、错误定义和损坏对象仍由共有 verifier 拒绝。

String、初始化服务及 descriptor、callable、selected、registration 接入共有记录。删除固定 CORE provider、专用 bridge、独立授权及 core requirement/owner/proof 通道。所有外来 strong 引用按实际 provider 和 typed target 查询 definition、ABI、symbol 与 relocation；GC 登记与初始化依赖仍完整。runtime/native/target 的不同调用契约保持，用途分区互斥且完整覆盖实际 undefined relocation。

未来 program descriptor 中的历史 C 字段 `core_image` 同样表示 String 声明的实际 provider，不再要求 reserved core coordinate。保持既有 C struct 布局、String 表示与 kind 常量；普通 core 修改或扩展不自动改变 runtime ABI。M23-8 的 startup 实现继续留在后续里程碑。

### 2.5 通用资源预算与计费

删除 `BudgetMeter`、累计 usage、logical heap/work、节点/边/复制字节和逐操作收费的参数、接口、错误、版本字段及专用测试。不保留改名后的 meter、unlimited 或空壳。

runtime scan 比较删除任意展开次数、逻辑字节配额及超额 abort；成本常量不再进入 profile、fingerprint 或 runtime ABI。实际 offset/count/length 使用 checked 运算并核对真实存储范围，非法循环由局部图检查处理，共享 DAG 比较按节点或节点对复用结果。分配失败正常报告，不以成本估算决定合法程序能否编译或运行。

### 2.6 重复证明与完整验证

共有源码接口保存所有必要声明的参数协议、typed 默认值正文和定义环境，protected、private 与默认值支持声明使用同一记录。type-semantics section 不再复制受限参数协议、默认值或来源并集；原 field 5、6、7 退役，现有 field 1～4、8 保持原编号。前端完成语言与可见性检查，reader 核对已有正文的编码、typed 引用与 owner/binder 关系，不再生成 ParamFree/GenericSourceMetadata 资格、逐正文访问证明或独立完整重放。源码完整但无运行时表示的类型可以参与默认参数声明；实际物化时按 typed 声明处理。此次格式变化纳入 cross-cone-type-semantics/5，旧产物需重建，runtime C ABI 不变。

同次编译使用完整 HIR/MIR/LIR 直接生产导出和对象，不反复逆向制造同一对象以证明来源或操作资格。外部产物在读取边界完成格式、typed 引用与跨层关系检查；Compile/Link 对同一字节快照和依赖复用该结果。Link 追加真实对象范围、符号、relocation、registration、patch 和 Code/runtime fingerprint 检查，不能删除必要检查或接受损坏产物。

M23-6 的正式 `scoopc` 发布与 `scoop` 依赖消费统一使用 `CrossConeLayoutStrong`。共有 reader 一次读入完整 HIR 类型语义、MIR 类型与 callable、LIR layout/ABI/dispatch 及真实对象；语义会话直接导入这些记录的实体映射，Compile 与 Link 引用同一完整结果。旧 M23-5 的独立提交、Link 重开及两份发布证明链退出正常路径，不把新 reader 的数据反向构造为旧 producer section。此前格式的依赖需重建；section tag 不复用，runtime C ABI 和 String 表示不变。

初始化与存储导入直接查询实际 provider 的完整 Strong 记录，核对实际 definition、symbol 与 ABI；移除 support-source 资格回调、空授权对象和重复 MIR 使用证明。源码使用与 typed 依赖闭包继续在对应语义边界检查。

当前源码的类型查询与 LIR 诊断目录直接接收 canonical HIR/MIR 实体记录及已读依赖图；不经自身 wire 编解码重建来源。实体层次、实际 provider 和 canonical key 仍完整保留，合并只检查实际 typed identity 冲突。

构建管理与编译器消费是两个明确边界。`scoop` 的发现、完成节点和缓存只保留不可变归档快照及普通 manifest 摘要；检查容器、成员范围与 hash、版本/profile、Cone/target、依赖 fingerprint、缓存键和子进程结果，不解码 HIR/MIR/LIR 或重建 Link 对象。已读 prebuilt 快照复用同一摘要，不能在每完成一个节点时重放该节点及全部依赖。`scoopc` 或实际 Link 消费者在读取外部产物时完成 typed 引用、ABI、对象与 relocation 检查，Compile/Link 共享这份完整结果；同次编译的输出直接发布。摘要不能替代 IR 或对象，也不承担来源或操作资格。此职责调整不改变 wire、runtime C ABI 或 String 表示。

layout reader 的外部形状合同保留选中记录的完整 typed 数据，读取结果直接拥有 section 和 Link 信息，不依赖临时 arena 或高阶回调。依赖查询期间的正常借用保留；不能用借用生命周期或只读回调额外限制已经完整的编译数据。

MIR 生产与读取共用已有的 `MirTypeBridgeDependencyViewV1`：它借用真实 provider 的完整导出表、初始化单元和普通 callable 定义。driver 从已解析依赖图提供可达 provider 的目录；section 不再保存 producer section 组成的递归依赖图，也不要求读出的依赖重新构造为 producer section。选择入口保留 provider 唯一性与实际 typed 引用闭包检查，manifest 依赖环检测复用共有图边界。读取结果可直接用于后续 MIR 类型、callable、dispatch、初始化和布局生产。

MIR section 直接消费同一次 HIR→MIR 已生产的六张完整导出表、初始化单元记录和 typed 依赖使用。删除独立来源工厂、重复预期表、逐表比较回调及 source-join 凭证；生产侧不再次重建或完整验证已经检查的记录。读取边界按实际 HIR 声明、MIR 定义与依赖目录检查格式、类型、签名、effect、可见性、实体归属和引用关系，保留已检查组成表供后续使用，不重建 producer section。依赖选择仍检查实际 provider、完整 typed target 和引用闭包。

初始化单元在 MIR 物化时已经确定实际 initializer、ensure、完整逻辑签名与 GC effect；导出直接复制这些完整记录，不再投影第二套来源签名或保存 ProducerEmitted／ReaderSemanticReplay 证明状态。reader 从实际声明的 unit key 和同一产物的 callable 定义恢复相同数据，并检查真实 provider、ordinary Managed、无 receiver/参数及 Unit 返回值。实际机器定义与 relocation 在 Link 对象边界检查；两种消费使用同一记录类型，七字段 wire 不增加冗余 unit-role 表。

LIR section 直接接收同次 MIR→LIR 已生产的完整五张导出表和 typed 依赖使用。lir-lower 按实际 MIR 使用、LIR external arena 与 Strong V2 初始化记录计算语义根，并核对物理引用的 provider、subject、symbol 和 definition；不重做 MIR source/export 全量验证，也不重新生成五张预期表。IR section 只负责导出关系与依赖引用闭合，不接收来源工厂或资格回调。reader 逐项检查新读入的组成表后保留这些表，随后解析 selected 和物理引用；不能再次传入另一套预期表，重编码并比较先前已经完成的同一检查。

非泛型 typealias 的声明、目标实体归属、public 可见性和外部引用在共有 HIR 声明边界检查。别名展开随后只处理实际 typed alias 目标、缺失目标及循环，不保存或查询逐边授权表，不重建依赖的 import/re-export 路径。已完成依赖的展开结果直接参与当前 Cone 的查询，共享最终 SignatureTypeKey；不能为同一别名链重复重走全部依赖。M23-6 完整 reader 保留展开结果供前端名称解析使用，生产与读取都不依赖测试专用来源工厂。

LIR 的依赖选择直接查询实际 provider 的完整 layout/ABI 五表导出记录。构建图提供可达 provider 集合，同次编译与产物 reader 使用同一入口；不要求从读取结果重造 producer section，不保存重复的递归 section 依赖图。选择仍检查 typed 目标、provider、target 和真实引用闭包，manifest 依赖环与语言布局环保留在各自职责边界。

Strong V2 的类型、布局、ABI、dispatch 与 registration 在实际组合边界核对一次，完成后直接保留普通完整 production section。后续 codegen、对象处理和发布使用该数据，不再通过 Pending/Replayed/Validated 凭证包装限制编码资格，也不复制五张导出表仅用于防止后续替换。provider 查询从实际 production 和导出记录取得 typed target、definition、symbol 与合同；已检查且未变化的依赖不反复重做整表关联。

同次编译完成的归档直接保存完整产物及其普通摘要；摘要由已有 manifest、语义 fingerprint、最终对象和生产记录产生，只记录后续构建与缓存实际需要的信息。原子发布直接写入这些归档 bytes，回读仅核对字节一致后替换目标，不重新创建语义会话或把当前产物和全部依赖再次交给 reader。外部输入的产物仍由共有 reader 检查格式、引用、ABI、符号和实际对象。删除已退出正式路径的 M23-5 归档 writer、双版本发布包装及其专用测试；发布接口只接收正常编译结果，不保留用于取得发布资格的 raw-bytes 重放入口。此清理不改变 wire、runtime ABI 或 String 表示。

M23-6 的 runtime 复用编译器或 artifact reader 已验证的静态类型、布局与 scan，分配、装箱、数组及 GC 不反复完整校验。动态对象范围、length/size、TD 归属、对齐和 GC 根仍检查；装箱根直接引用 TD 的 inline scan，不重发射和逐项比较第二份 scan。完整静态检查保留为 `scoop_shape_validate`，`SCOOP_VERIFY_METADATA=1` 可在 runtime 操作入口显式启用。M23-8 才引入多 image 登记边界，本阶段不为此新建 registry 或不可变指针缓存。

### 2.7 无生产用途的框架和重复实现

删除仅由测试实现/使用的 HIR/MIR/LIR 来源工厂、平行来源 reader、凭证状态机、重复数据表及适配层。旧 bootstrap stage 包装与 Core/NotCore 测试输入适配层同样删除；保留的真实源码测试直接调用共有 stage 入口。独立 dual-artifact certificate/reopen 框架没有生产调用者，删除后只保留实际使用的不可变产物快照。source-authority 收缩为名称解析、类型检查、默认参数实例化和跨 Cone 消费实际需要的声明/来源信息，不保留逐项授权和防伪证明。

语言规则由对应 stage 负责；IR/meta crate 负责数据及格式/引用不变量，reader 不再维护一套前端语义实现。已解析默认值保留完整 typed 正文、声明引用和定义环境。不得用新通用工厂、插件、安全框架或证明系统代替删除项。

## 3. 编排、wire 与文档

producer、reader、linker、wire/profile、版本、fingerprint、fixture、golden 及规范同步修改。格式改变按正常版本演进，退役 tag 不复用；旧产物明确重建，不做长期双轨适配。内容 fingerprint 与缓存记录继续保留实际用途，不绑定计费策略或来源授权。

保持实际 runtime C 调用约定、String 表示与 persistent identity。后续 ODR、multi-image startup、artifact-only program-link 不属于此清理。历史阶段文档如描述已退役策略，只可作为带有明确迁移说明的历史记录，不能继续作为生产入口要求。

## 4. 实施顺序与验收

先修订三份 spec、ROADMAP、M23 总设计、M23-6 设计与本清理文档，再按实际调用链清理实现。已完成项核对后保留，不重复实现；每批代码先格式化和 lint，随后测试，通过后按功能提交。

| 验收项 | 实际场景 |
|---|---|
| core 普通库 | 普通目录修改、扩展、重建，显式依赖优先，下游消费与缓存失效 |
| 类型与 ABI | 独立/混合 provider 的 layout、C-safe、完整签名、ZST、direct/indirect、GC/root plan |
| 成员与 dispatch | constructor、继承、interface/default、getter/setter、protected、真实 body/slot |
| String 与初始化 | 字面量、类型测试、属性/object 初始化、循环失败、真实 descriptor/callable/registration |
| 产物与 Link | 真实源码生成完整产物，consumer 只读产物；对象、符号、relocation、版本与内容检查 |
| runtime | 分配、boxing、array、GC 不重复展开已验证静态 scan；动态范围错误仍拒绝 |
| 清理结果 | 无生产调用依赖旧授权、通用计量、重复证明或测试专用工厂；不以改名或 unlimited 通过 |

语言错误 fixture 断言位置与信息，格式损坏和正常功能回归保留，删除仅服务旧机制的测试。手工 metadata、测试来源工厂和大量证明反例不能替代真实编译、跨 Cone 消费、适用链接运行。只有主设计第 15 节的原有功能、本文件七组清理及验收均完成、文档一致并按功能提交后，才能完成目标。
