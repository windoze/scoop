# M23-6：普通编译器职责与过度设计清理

运行时类型转换的失败构造使用前端解析的实际异常类型与 constructor 引用，并沿共有的类型、callable、ABI 和 Link 路径消费。删除由 Cast 反向投影的独立 CastFailure call-site、RuntimeOperationDependency role，以及 reader 对同一目标再按 compiler protocol 进行资格判断的通道；普通源码调用的位置、参数、结果与 typed 引用检查保留。共有 HIR 格式更新为 `hir/cross-cone-interface/27`，原 call-site reason tag 2 与 external-reference role tag 9 退役，不复用；旧产物、profile fingerprint 与缓存重建。该调整不改变转换失败抛出 ClassCastException 的语言行为、runtime C ABI 或 String 表示。

引用上行转换在 HIR 中用显式 `ReferenceUpcast` 节点保存内部表达式及目标类型，不能直接改写构造、调用或局部读取的原始类型。MIR 使用已有 `Retype`，不分配对象、不改变引用身份；构造器仍按实际所属 class 分配。默认值正文使用新 expression tag 58 保存同一操作，tag 44 继续退役；共有 HIR 格式更新为 `hir/cross-cone-interface/27`，旧产物与缓存重建，不改变 runtime C ABI。

外来接口与动态调用沿共有类型和 callable 查询消费真实定义。删除 external callee 必须 Direct 的阶段限制后，由完整 typed receiver、所属 dispatch 表、槽位置、声明签名和实际 ABI 表达调用；不能为此添加新的来源工厂、dispatch 凭证或完整语义重放。

共有导出绑定包含 enum 变体的真实 typed ID，nominal 的 `nested_bindings` 同时列出其静态命名空间中的嵌套类型、object value 与 enum 变体；变体归属由实际声明确定，不能误作包级值。`hir/cross-cone-interface/27` 更新该格式语义，旧 `/23` 及更早产物与缓存重建；既有 tag 不复用，不保留双轨 reader，runtime C ABI 和 String 表示保持。

enum 模式和变体测试不是构造器调用：删除默认值引用集合中仅为这些操作保存的构造器访问记录及 reader 对该记录的要求，直接消费正文已有的实际 variant、owner 与字段引用。保留实际构造表达式的构造器关系、前端可见性和类型检查；`hir/cross-cone-interface/27` 同步语义并要求旧 `/22` 及更早产物重建，不建立替代凭证。

Strong producer 的两种产物表示从完整 LIR 各计算一次类型、safepoint、immortal 与本地初始化语义，digest 与 registration 直接复用这些结果；初始化仅在 digest 身份可用后补入实际依赖定义。外部初始化引用在解析时完成 definition 与物理导入核对，后续由全局唯一的 local unit 引用表达使用归属，不保存额外 consumer 状态或重复选择核对。descriptor 和 dispatch 直接保存 LIR 中已有的外部 typed 引用，不反向重新 materialize 外部对象或再次比较完整 ABI。实体与 definition role 的匹配只查询实体种类和角色，不借用虚构的 CORE provider；实际 definition、symbol、relocation、ABI 和 GC 检查仍由各自消费边界负责。此清理保持产物字段、指纹内容和 runtime C ABI 不变。

canonical ABI 导出复用同次完整 IR 的实际签名和 callable definition，删除重复逐参数 layout ID 表及只为该表存在的重放入口。共有 reader 以真实声明和 MIR lowered signature 核对 ABI，dispatch 按实际 receiver 查询表示。`lir/cross-cone-layout-abi/3` 退役 callable field 5，其余字段编号保持；旧 `/2` 产物与缓存重建，profile 和内容 fingerprint 按格式正常更新。tuple 的字段与参数使用完整结构身份和已有存储算法，不为嵌套值补造独立 nominal、layout/TD definition 或来源记录。字段布局消费完整存储数据，C-layout 的嵌套合同与 enum niche 的实际指针种类仍保留；删除以字段内嵌数据为由追加 layout 符号依赖并再次比较完整记录的通道。

MIR 组装先排除已经由普通 callable 表保存的实际声明，再生产需要独立 lowering 的定义；不为同一函数重建第二份绑定、完整验证后再丢弃。boxing 与 dispatch 的共有查询同时借用当前 Cone 的普通记录和依赖记录。每个定义只生成一次，签名、GC effect 与 typed target 沿完整 IR 和已有表消费，最终表继续拒绝重复定义。

实际 HIR 调用在依赖 MIR 记录可用后完成一次调用根与逻辑签名核对，普通函数、accessor 与 constructor 共用该边界。MIR 和 LIR 的依赖集合分别从实际 typed 调用加入相应 callable 与 ABI 引用，沿共有 provider 查询消费已有定义；不得因旧函数表不含 constructor 而拒绝合法调用，也不在前置阶段和布局阶段重复核对同一调用。

共有 nominal 声明直接保存 struct 主构造器的 typed declaration ID，供前端按实际语言角色检查 `@NoGC` 调用；不能从参数形状、字段布局或 provider 身份推断主构造器。主构造器继续保留源码 Managed、物理 NoGC 的既有合同；值构造本身不分配，`@NoGC` 的参数、结果与局部值仍须 GC-free，managed 次构造器仍禁止调用。`hir/cross-cone-interface/27` 在 `NominalDeclarationDetailsV1` 新增 field 8：空数组表示无值主构造器，单元素数组保存其 constructor ID；有构造器的 struct 必须明确该引用，引用必须属于同一 nominal 的声明集合，其他 nominal 不得填写。旧 `/21` 及更早格式退役并要求重建，既有 tag 不复用，profile 与内容 fingerprint 正常更新；MIR/LIR callable 格式和 runtime ABI 不变。

跨 Cone struct 构造器使用实际 provider 与 Strong callable target 进入共有请求内选择；其完整逻辑／物理签名来自已发布定义，不能伪装为普通函数 ID 或重建固定 core 身份。主构造器的源码 Managed 合同与实际 NoGC 值入口分别保留。Strong 外部根不重复携带 function/accessor 资格 ID，LIR 使用现有 layout/ABI 与物理 import 物化调用，并复用于 dispatch；不增加构造器专用来源工厂、授权外层或平行导出表。

代码生成在入口对本次完整且不可变的 LIR、目标 profile 和 executable 入口完成一次必要验证，然后按实际定义分成函数与非函数对象。各成员直接消费同一 LIR，不因发射另一个对象再次完整遍历类型、ABI、CFG、GC roots、safepoint 或身份表；LLVM 变换后的 IR 和新生成的对象字节仍在各自边界检查。generated-C 源码入口同样不在内部 helper 重复整模块验证。这一职责调整不改变产物格式、runtime C ABI、String 表示或链接语义。

普通 final 成员与其他 callable 共用真实 provider、typed declaration 和 implementation 查询；移除“普通导出不能被 dispatch/类型闭包引用”的旧分区限制及其身份名单。MIR 直接导出补全 GC effect，LIR dispatch 复用实际 ABI 并只保存 implementation/body 引用。无 namespace 导入的成员调用不伪造 binding，不在选择集合中另存用于认证的 import 路径。格式升级、旧 major 退役和完整消费规则见 [M23-6 设计](DESIGN.md)；runtime C ABI、String 表示与 GC 契约保持。 struct 字段读取直接保留真实 field ID 并使用共有布局；计算属性消费实际 getter 引用及 ABI，均无额外来源或资格外层。 成员 setter 与 getter 使用同一属性声明、真实 accessor 和普通 dispatch，不建立写入授权外层；可见性、值类型与 GC 写屏障仍按各自语言和内存规则处理。

本次来源框架清理覆盖所有 Cone：旧 HIR foundation/declaration transcript 及逐层 `Bound*` 包装只由测试工厂消费，应删除相关构造器、编码器、绑定链和专用测试。保留 `source_authority` 中仍被生产调用的普通声明数据、默认参数数据和身份查询；以共有源码编译、reader、链接与运行回归确认清理结果。

类型生产器直接返回完整的共有 type section；MIR 从其中已有的继承边、槽序和 typed 实现目标取得类型与 dispatch 信息。类型可物化性查询复用已有共有 nominal/callable 声明，不为同一次产出重新投影全部声明表。删除另行投影的来源 foundation、独立证明外层、只供旧测试消费的平行名义声明/参数/属性表，以及仅为比较同次结果而重建公开声明和 callable 表的工作。生产器仍从完整 HIR 生成全部实际记录，外部 reader 继续在明确边界检查格式、身份、引用、继承、类型与 ABI。删除仅用于重复证明的名义类型 lookup/inheritance/slot 三域副本；可见性仍由完整 typed 声明、词法 owner 和实际继承关系决定。`NominalInheritanceInterfaceV1` 的 field 5 退役，保留 field 1～4、6～9；HIR `cross-cone-type-semantics/6`、required inventory、profile 与 HIR fingerprint 同步更新，旧产物重建。未进入实际产物的旧辅助编码不保留兼容分支，runtime C ABI 与 String 表示不变。

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。 依赖查询直接使用真实 `ConeIdentity` 与 callable、property、type-alias 的类型化声明 ID；选择集合按这些 ID 保存完整接口和实际依赖路径。删除独立 world/projection/selection 品牌、仅为品牌服务的计数器和错误，以及从声明 ID 再映射到局部 u32 的三套重复表。候选选择依照当前依赖目录中的实际声明与表示，不要求由同一查询实例铸造；直接依赖的名称可见性、转导出路径、实际 provider 和引用完整性继续按共有规则检查。HIR 候选和已选声明只保存实际 provider ID，不逐项复制 artifact 坐标/fingerprint 凭证；HIR→MIR 使用同次编译的依赖快照及完整声明，核对实际定义和签名，不再次比较来源凭证。普通构建依赖记录与缓存 fingerprint 继续承担定位和失效职责。MIR 外部 callable 引用保存实际 provider 与类型化声明，不另映射到带会话品牌的局部编号；调用位置保留 GC effect。MIR 类型与 LIR layout/ABI 选择按实际 provider 和 typed target 直接返回完整记录，不先铸造并验证中间 handle；依赖闭包、类型、ABI、物理引用和 GC 契约仍在其消费边界检查。同一声明或 target 在另一选择集合中是否存在，按实际目录查询决定，不依据集合生成顺序或计数器。该进程内数据简化不改变 wire/profile、实体身份或 runtime ABI。

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

跨 Cone 的 `throw`、语句及表达式形式的 `catch` 使用前端已解析的实际 `Throwable` 声明，沿共有依赖类型查询取得继承关系并执行同一子类型规则；导入协议不允许跳过检查。同名普通 class 不能替代该实体。异常构造、默认参数、调用、布局、TD 和展开使用共有路径，保留调用求值顺序、catch 顺序与 finally 语义；此能力不增加协议资格、独立证明、wire 字段或 runtime ABI。 公开存储属性的 getter 与可公开调用的 setter 必须按实际 owner 和声明类型生成普通 callable body，即使 provider 的正文没有引用该属性；名义类型、顶层属性与 core 使用同一规则。参数自由且可物化的属性自动导出实际 body。泛型或 source-only owner 仅用于签名和表示查询时，不自动实例化其普通方法或 accessor；实际调用仍通过同一 typed 请求生成完整实例，dispatch 所需的方法继续随其实际类型物化。

跨 Cone 的强制 `as` 沿实际 `ClassCastException` 声明查询完整异常类型，并选择该声明的零参数 constructor 或默认参数适配入口。成功检查保留原对象身份，失败通过普通 class 分配、外部 initializer 调用和 throw 执行；别名、interface 与装箱值沿同一类型与 ABI 路径处理。默认参数中的转换在实际展开时进入相同路径，未求值模板不触发机器物化。异常类的表示仍依赖后续泛型能力时，前端在输出 LocalConcrete HIR 前对实际执行点给出已有 layout-required 诊断；不输出缺少所需表示的成功 IR。MIR 直接消费完整协议中的 typed 声明引用，不再建立丢弃外来声明信息的 Core/Imported 资格投影。所选 constructor 使用共有依赖 callable 集合，保留真实逻辑/物理签名、GC effect、ABI 和 relocation，不增加转换调用的证明记录、wire 字段或 runtime ABI。零参数默认值 adapter 以已有 GeneratedCallable 实体及 ClassInitializer 表示进入共有 callable 导出，不能只发布协议引用而遗漏定义。MIR/LIR selected callable 记录本身是机器依赖引用；reader 按 provider、typed target、定义、签名和传递依赖检查其完整性，不要求隐式调用另附一份 HIR 操作资格记录。

删除 CORE 来源资格、Core/NotCore 包装、重复 operation/签名投影和凭证外层。语言角色只保存实际需要的 typed 声明引用、签名、effect、可见性与依赖关系。普通 metadata 发布和消费这些声明；intrinsic 正规化后不再从来源获得执行资格。internal 服务不因迁移加入 public lookup。

HIR 的 `org.scoop-lang.hir/core-bootstrap-interface/4` 直接保存完整 `CoreCompilerProtocolSurfaceV1`，删除重复的 `RuntimeCoreCapabilityV1::String` 与 `CompilerProtocolDefinitionsV1` 外层。section 的 field 2、3 继续保存 output contract 和 direct public surface，field 5 以长度为 0 或 1 的 array 保存本产物定义的协议角色；旧 field 1、4 及旧资格分支 tag 1、2 退役，不复用，旧 `/1`～`/3` 产物与缓存重建。String 的 source identity 只在已有 fundamental type 角色中保存，exact identity 由该真实声明的 `ExactTypeKey::Nominal` 得到；不再维护第二份 source/exact 记录或比较两份投影。前端完成 intrinsic 识别与语言声明检查，后续阶段直接消费完整 typed 角色、共有声明与实际 provider；reader 保留格式、引用种类、签名、effect、成员归属和跨表一致性检查。初始化循环服务的实际函数、String 参数、Unit 结果及 canonical ABI 继续沿共有 callable 路径使用。此修订不改变 String 表示或 runtime C 调用约定。

### 2.4 String、初始化与 Link

String descriptor 使用完整 MIR 中实际声明的 source exact identity，沿共有 descriptor 查询、layout selection、physical import、registration 和 Link relocation 消费。删除独立 String bridge 与固定角色的 descriptor 恢复通道，不以 provider 坐标或协议来源豁免普通引用检查。Strong production `/7`、`/8` 退役原服务表中的 TD tag 2；旧产物与缓存重建，String 表示及 runtime C ABI 不变。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

Strong production 的两种表示升级为 `/9`、`/10`，删除初始化专用 ABI field 11 和外部服务表 field 12，section 只保留 field 2～9 的八字段 product。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 registration reader 完成结构与引用检查后直接返回完整 registration production 数据，不保留仅用于限制编码或导出资格的中间凭证包装。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

LIR descriptor 依赖是明确的 typed IR 引用。reader 验证可达 provider、exact type 的实际 descriptor 导出、递归依赖闭合及物理定义，不要求 descriptor 先出现在 HIR 的 value-layout 或 shape-support 物化根中，也不从协议表重新证明其来源。显式 descriptor relation 与从源码类型使用得到的 layout/shape 根共同闭合；遗漏必要依赖、错误 provider/类型与不一致物理合同仍拒绝。

共有 Link import 表可以保留没有实际 relocation 的完整类型或 callable 引用；这类声明不生成虚构的 machine use。Link 只对实际 relocation 生成 requirement，并验证目标、provider、symbol、ABI 和定义一致性。删除“每个已声明 import 必须至少出现一次”的附加证明，以及初始化服务的专用豁免；缺失实际引用、错误定义和损坏对象仍由共有 verifier 拒绝。

String、初始化服务及 descriptor、callable、selected、registration 接入共有记录。删除固定 CORE provider、专用 bridge、独立授权及 core requirement/owner/proof 通道。所有外来 strong 引用按实际 provider 和 typed target 查询 definition、ABI、symbol 与 relocation；GC 登记与初始化依赖仍完整。runtime/native/target 的不同调用契约保持，用途分区互斥且完整覆盖实际 undefined relocation。

M23-6 删除没有实际 producer/consumer 的 `ScoopRuntimeCoreBindingsV1`、`ScoopProgramDescriptorV1`、固定 String capability ID 及其 LLVM 布局镜像和专用测试，不把它们改名后保留。未使用的 program/core magic `0x53434f4f50505247`、`0x53434f4f50434f52` 退役且不复用。实际发射的 image、root entry、type/callable、storage、immortal、initialization 和 safepoint 记录继续使用既有格式；这项删除不改变实际 runtime C ABI、String 表示或其 fingerprint。M23-8/9 的多 image 启动与 program-link 仍留在后续阶段，按实际入口和引用需要定义数据，不提前冻结新的 program record 或另建 String 授权表。

String 由前端解析为实际 typed class，MIR/LIR 与 Link 使用同一 provider 的 exact type、TypeDescriptor、registration 和 relocation。现有 C ABI 的 `scoop_td_String` 表示 runtime 所需的 String descriptor 地址；链接使用实际声明的定义，不从固定 CORE identity 重建它，也不按名称或同布局替换类型。对象头为 16 bytes，length offset 为 16，inline bytes offset/minimum 为 24，alignment 为 8；`InlineBytes` 的 size/stride/alignment 为 1，object/inline scan 为空。保留这些真实表示和边界检查，不单列重复的 String 布局/scan 副本、capability-kind digest 或 program/core binding 证明。

### 2.5 通用资源预算与计费

删除 `BudgetMeter`、累计 usage、logical heap/work、节点/边/复制字节和逐操作收费的参数、接口、错误、版本字段及专用测试。不保留改名后的 meter、unlimited 或空壳。

runtime scan 比较删除任意展开次数、逻辑字节配额及超额 abort；成本常量不再进入 profile、fingerprint 或 runtime ABI。实际 offset/count/length 使用 checked 运算并核对真实存储范围，非法循环由局部图检查处理，共享 DAG 比较按节点或节点对复用结果。分配失败正常报告，不以成本估算决定合法程序能否编译或运行。

### 2.6 重复证明与完整验证

共有 HIR 类型位置的结构、foreign nominal 分发、真实 provider 与定义/求值位置在 HIR reader 边界检查一次。后续物化查询和 MIR/LIR 消费同一未变化的 typed 记录，不重新完整检查这组 HIR 关系，不建立额外验证状态或凭证；实际类型表示、签名、ABI、对象与传递引用仍由相应边界检查。外部字节重新读入或相关数据发生变化时重新验证受影响部分。这一职责调整不改变 wire、内容 fingerprint 的字段组成或 runtime C ABI。

删除 `validate_iteration_plans` 及 concretizer 两个入口的整模块调用。该通道重新检查全部函数、构造器、默认值的迭代协议、类型、effect 和局部定义，没有实际 artifact reader 消费；其独立语义实现、伪造 HIR 测试和修改辅助函数一并退役。前端产生完整 `IterationCore`、`ForIterationPlan`、binding plan 与 typed loop target，后续直接消费；保留真实源码和正常产物的正确性回归。

共有源码接口保存所有必要声明的参数协议、typed 默认值正文和定义环境，protected、private 与默认值支持声明使用同一记录。type-semantics section 不再复制受限参数协议、默认值或来源并集；原 field 5、6、7 退役，现有 field 1～4、8 保持原编号。前端完成语言与可见性检查，reader 核对已有正文的编码、typed 引用与 owner/binder 关系，不再生成 ParamFree/GenericSourceMetadata 资格、逐正文访问证明或独立完整重放。源码完整但无运行时表示的类型可以参与默认参数声明；实际物化时按 typed 声明处理。此次格式变化纳入 cross-cone-type-semantics/6，旧产物需重建，runtime C ABI 不变。

源码位置由共有接口的 definition_sources、call_sites、type_sites 与已有全局定义记录汇集，不能为收集位置重建独立默认值正文、参数协议或访问证明。删除 NominalDefaultSourceProductionV1、DefaultSourceBodyProductionV1 及旧 DefaultSourceTemplate／References／Access 数据模型、适配器和专用测试；生产路径直接复用共有接口的完整默认值及位置收集。必要的来源位置、typed 引用、可见性、参数与 binder 规则仍保留。旧模型已不属于正常产物字段，此清理不增加格式分支或改变 runtime ABI；内容 fingerprint 依实际产物数据计算。

默认值引用是普通依赖索引：六类记录各保留真实 typed target 与 definition origin，正文与索引在读取边界核对一次。定义处的名称、类型、effect 与调用域覆盖规则由前端负责；继承默认值遇到类型代换或调用域扩大时检查实际变化，未变化的事实直接复用。producer 不重建访问域，reader 不再分别重放 type、value、callable 与 direct/slot 的访问证明；产物仍检查实际 provider、typed 引用、owner/binder 范围、局部值范围及跨表一致性。字段、构造器与全局值引用在共有 reader 边界对照已解析声明的实际 owner、种类和作用域；局部函数引用必须对应正文携带的声明。复用已验证的身份图与正文索引，不重新推导访问域。默认引用 record 采用两字段 map，field 1=target、field 2=definition_origin；旧 witness 的 field 3 退役且不复用。共有接口升级为 `hir/cross-cone-interface/27`，旧 `/24` 及更早产物与缓存重建，profile 与内容 fingerprint 同步更新。运行时 C ABI、String 表示及必要 GC 契约不变。

删除没有生产实现者的完整 HIR semantic-authority 总入口、默认引用 envelope 平行验证器和公共类型支持凭证；正常 reader 使用已有各表的格式、引用与类型边界检查。仅验证旧凭证构造、访问域重放和证明状态的测试随之删除，实际源码的可见性错误、默认值继承与跨 Cone 展开验收继续保留。

删除仅由测试实现的默认值operation-typing、nested ABI及root/origin语义工厂和其证明数据、平行验证入口与专用测试。正式reader继续使用共有声明表、完整typed模板、类型与binder检查、来源位置、局部数据流及真实引用一致性检查。局部数据流直接借用模板与共有字段查询，删除重复body input及authority适配器；nested descriptor保留实际类型化身份、parent/path与binder数据，删除独立Standalone证明模式。语言操作规则由前端负责，不在IR/meta crate再复制实现。此清理不改变wire字段、profile版本、runtime C ABI或String表示。

参数自由依赖class通过共有名义声明取得真实身份、modality、直接父类型和声明序字段，引用类型按GC契约传播，递归引用字段不展开成递归值布局。HIR保留完整声明及解析后的字段、父类型，不重建同名本地声明。外来class构造仍是对真实constructor的typed调用；HIR→MIR消费已选MIR绑定中的ClassInitializer角色，使用共有class分配路径创建对象，再将该对象作为receiver调用实际provider的初始化实现。构造表达式返回分配的对象，物理initializer返回Unit；普通返回class的函数不走构造分支。本地与外来initializer共用Callee表示、参数求值顺序和GC处理。 core默认导入层的构造调用和静态限定名同时查询普通Type/Value binding；用户新增的class、enum和typealias与其他依赖使用相同声明路径，不限于预设内建名字。此能力不增加来源凭证、core分支或runtime ABI，沿用现有类型、MIR绑定及LIR布局格式。 M23-6的真实class运行在单个最终镜像中链接实际产物及runtime；LLVM stackmap段按各对象贡献的完整v3 blob逐个读取，长度由已有count和对齐决定，保留每份格式、记录唯一性与精确PC检查，不增加展开预算。该读取不要求M23-8的多镜像启动或M23-9的program-link。

外来 enum 构造和默认值展开直接消费共有声明的实际 variant ID、字段类型与源码参数协议，沿完整 typed HIR 生成值；不得为内联值构造新增机器 callable 资格、来源模板工厂或独立授权记录。

同次编译使用完整 HIR/MIR/LIR 直接生产导出和对象，不反复逆向制造同一对象以证明来源或操作资格。外部产物在读取边界完成格式、typed 引用与跨层关系检查；Compile/Link 对同一字节快照和依赖复用该结果。Link 追加真实对象范围、符号、relocation、registration、patch 和 Code/runtime fingerprint 检查，不能删除必要检查或接受损坏产物。

M23-6 的正式 `scoopc` 发布与 `scoop` 依赖消费统一使用 `CrossConeLayoutStrong`。共有 reader 一次读入完整 HIR 类型语义、MIR 类型与 callable、LIR layout/ABI/dispatch 及真实对象；语义会话直接导入这些记录的实体映射，Compile 与 Link 引用同一完整结果。旧 M23-5 的独立提交、Link 重开及两份发布证明链退出正常路径，不把新 reader 的数据反向构造为旧 producer section。此前格式的依赖需重建；section tag 不复用，runtime C ABI 和 String 表示不变。

初始化与存储导入直接查询实际 provider 的完整 Strong 记录，核对实际 definition、symbol 与 ABI；移除 support-source 资格回调、空授权对象和重复 MIR 使用证明。源码使用与 typed 依赖闭包继续在对应语义边界检查。

当前源码的类型查询与 LIR 诊断目录直接接收 canonical HIR/MIR 实体记录及已读依赖图；不经自身 wire 编解码重建来源。实体层次、实际 provider 和 canonical key 仍完整保留，合并只检查实际 typed identity 冲突。

构建管理与编译器消费是两个明确边界。`scoop` 的发现、完成节点和缓存只保留不可变归档快照及普通 manifest 摘要；检查容器、成员范围与 hash、版本/profile、Cone/target、依赖 fingerprint、缓存键和子进程结果，不解码 HIR/MIR/LIR 或重建 Link 对象。已读 prebuilt 快照复用同一摘要，不能在每完成一个节点时重放该节点及全部依赖。`scoopc` 或实际 Link 消费者在读取外部产物时完成 typed 引用、ABI、对象与 relocation 检查，Compile/Link 共享这份完整结果；同次编译的输出直接发布。摘要不能替代 IR 或对象，也不承担来源或操作资格。此职责调整不改变 wire、runtime C ABI 或 String 表示。

layout reader 的外部形状合同保留选中记录的完整 typed 数据，读取结果直接拥有 section 和 Link 信息，不依赖临时 arena 或高阶回调。依赖查询期间的正常借用保留；不能用借用生命周期或只读回调额外限制已经完整的编译数据。

MIR 生产与读取共用已有的 `MirTypeBridgeDependencyViewV1`：它借用真实 provider 的完整导出表、初始化单元和普通 callable 定义。driver 从已解析依赖图提供可达 provider 的目录；section 不再保存 producer section 组成的递归依赖图，也不要求读出的依赖重新构造为 producer section。选择入口保留 provider 唯一性与实际 typed 引用闭包检查，manifest 依赖环检测复用共有图边界。读取结果可直接用于后续 MIR 类型、callable、dispatch、初始化和布局生产。

MIR section 直接消费同一次 HIR→MIR 已生产的六张完整导出表、初始化单元记录和 typed 依赖使用。删除独立来源工厂、重复预期表、逐表比较回调及 source-join 凭证；生产侧不再次重建或完整验证已经检查的记录。读取边界按实际 HIR 声明、MIR 定义与依赖目录检查格式、类型、签名、effect、可见性、实体归属和引用关系，保留已检查组成表供后续使用，不重建 producer section。依赖选择仍检查实际 provider、完整 typed target 和引用闭包。

普通 callable 导出在首次生成时根据真实声明解析 exact signature，并与实际 Strong body 的签名关联。组装类型导出时直接使用这些完整记录，不再次从 HIR 枚举、分类和比较同一普通导出全集。组装在生产其他 lowering 记录前排除已有普通定义；合并后的最终表统一排序并检查重复定义，不在合并中途再构建一张仅供重验的临时 canonical 表。外部 reader 对新读入数据的类型、签名和定义检查保留；本调整不改变 wire/profile、runtime C ABI 或 String 表示。

初始化单元在 MIR 物化时已经确定实际 initializer、ensure、完整逻辑签名与 GC effect；导出直接复制这些完整记录，不再投影第二套来源签名或保存 ProducerEmitted／ReaderSemanticReplay 证明状态。reader 从实际声明的 unit key 和同一产物的 callable 定义恢复相同数据，并检查真实 provider、ordinary Managed、无 receiver/参数及 Unit 返回值。实际机器定义与 relocation 在 Link 对象边界检查；两种消费使用同一记录类型，七字段 wire 不增加冗余 unit-role 表。

LIR section 直接接收同次 MIR→LIR 已生产的完整五张导出表和 typed 依赖使用。lir-lower 按实际 MIR 使用、LIR external arena 与 Strong V2 初始化记录计算语义根，并核对物理引用的 provider、subject、symbol 和 definition；不重做 MIR source/export 全量验证，也不重新生成五张预期表。IR section 只负责导出关系与依赖引用闭合，不接收来源工厂或资格回调。reader 逐项检查新读入的组成表后保留这些表，随后解析 selected 和物理引用；不能再次传入另一套预期表，重编码并比较先前已经完成的同一检查。

非泛型 typealias 的声明、目标实体归属、public 可见性和外部引用在共有 HIR 声明边界检查。别名展开随后只处理实际 typed alias 目标、缺失目标及循环，不保存或查询逐边授权表，不重建依赖的 import/re-export 路径。已完成依赖的展开结果直接参与当前 Cone 的查询，共享最终 SignatureTypeKey；不能为同一别名链重复重走全部依赖。M23-6 完整 reader 保留展开结果供前端名称解析使用，生产与读取都不依赖测试专用来源工厂。

LIR 的依赖选择直接查询实际 provider 的完整 layout/ABI 五表导出记录。构建图提供可达 provider 集合，同次编译与产物 reader 使用同一入口；不要求从读取结果重造 producer section，不保存重复的递归 section 依赖图。选择仍检查 typed 目标、provider、target 和真实引用闭包，manifest 依赖环与语言布局环保留在各自职责边界。

Strong V2 的类型、布局、ABI、dispatch 与 registration 在实际组合边界核对一次，完成后直接保留普通完整 production section。后续 codegen、对象处理和发布使用该数据，不再通过 Pending/Replayed/Validated 凭证包装限制编码资格，也不复制五张导出表仅用于防止后续替换。provider 查询从实际 production 和导出记录取得 typed target、definition、symbol 与合同；已检查且未变化的依赖不反复重做整表关联。

同次编译完成的归档直接保存完整产物及其普通摘要；摘要由已有 manifest、语义 fingerprint、最终对象和生产记录产生，只记录后续构建与缓存实际需要的信息。原子发布直接写入这些归档 bytes，回读仅核对字节一致后替换目标，不重新创建语义会话或把当前产物和全部依赖再次交给 reader。外部输入的产物仍由共有 reader 检查格式、引用、ABI、符号和实际对象。删除已退出正式路径的 M23-5 归档 writer、双版本发布包装及其专用测试；发布接口只接收正常编译结果，不保留用于取得发布资格的 raw-bytes 重放入口。此清理不改变 wire、runtime ABI 或 String 表示。

M23-6 的 runtime 复用编译器或 artifact reader 已验证的静态类型、布局与 scan，分配、装箱、数组及 GC 不反复完整校验。动态对象范围、length/size、TD 归属、对齐和 GC 根仍检查；装箱根直接引用 TD 的 inline scan，不重发射和逐项比较第二份 scan。完整静态检查保留为 `scoop_shape_validate`，`SCOOP_VERIFY_METADATA=1` 可在 runtime 操作入口显式启用。M23-8 才引入多 image 登记边界，本阶段不为此新建 registry 或不可变指针缓存。

### 2.7 无生产用途的框架和重复实现

删除仅由测试实现/使用的 HIR/MIR/LIR 来源工厂、平行来源 reader、凭证状态机、重复数据表及适配层。旧 bootstrap stage 包装与 Core/NotCore 测试输入适配层同样删除；保留的真实源码测试直接调用共有 stage 入口。独立 dual-artifact certificate/reopen 框架没有生产调用者，删除后只保留实际使用的不可变产物快照。source-authority 收缩为名称解析、类型检查、默认参数实例化和跨 Cone 消费实际需要的声明/来源信息，不保留逐项授权和防伪证明。

语言规则由对应 stage 负责；IR/meta crate 负责数据及格式/引用不变量，reader 不再维护一套前端语义实现。已解析默认值保留完整 typed 正文、声明引用和定义环境。依赖默认值中的字段和 callable 使用定义时保存的 typed 声明与完整类型，实例化不重新要求公开 namespace 导入路径，也不再次证明模板引用集合。

默认值依赖按定义处已解析的 typed target 保存，`DefaultDependency` 不要求消费 Cone 再取得 namespace 导入路径。定义处实际发生过的查找路径可以保留；生产器不为没有名称查找的字段、成员或支持声明补造 witness，也不保存全体依赖的第二份导入路径表。Unit、Any 与其他声明遵循同一规则。共有 reader 继续检查 provider、引用、类型、默认值正文及实际声明关系，不从这份路径信息授予调用资格。读取边界核对源码上下文及位置后，实例化复用同一不可变记录，保留定义位置与调用处求值位置。不得用新通用工厂、插件、安全框架或证明系统代替删除项。

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
