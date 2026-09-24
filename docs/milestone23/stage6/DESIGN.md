# M23-6 设计：跨 Cone layout、typed ABI 与 ZST

M23-6 将共有默认引用的访问 witness 纳入 `hir/cross-cone-interface/16`：原三字段的 public-only witness 退役，新的四字段 product 依次保存发布 callable、direct call domain、长度恰为 0 或 1 的 slot domain 列表以及 target access domain。所有域使用同一源码域格式：tag 1 为空域，tag 2 的 field 1 为约束的严格有序、无重复数组，空数组表示 universal；约束各为两字段 product，field 0 的 tag 依次为 1=Cone、2=source file、3=lexical source nominal、5=subclasses of source nominal，field 1 保存对应 typed identity，tag 4 保留不用。subclass 约束直接保存 Concrete/GenericTemplate 的 SourceNominalId，不要求机器 exact、布局或实例化。公开默认值仍须逐项证明 universal target 与完整公开调用域；解码出受限源码域本身不授予查找、调用或机器能力。默认正文及六类引用只保留这一份 witness，继承默认值还须按实际 provider 声明和发布声明分别重放域与包含关系。源码域的 canonical 解码、身份解析、集合比较及来源路径成本沿用 artifact 的累计预算；重复、乱序、空域携带约束、slot 数量错误、未知 tag 和旧格式均拒绝。section 仍为十字段，不新增 source-authority 证明表；旧 major、profile、HIR fingerprint 与缓存随之失效并重建。

2026-09-23 共有属性声明约定：M23-6 的共有 property/accessor 声明使用 `hir/cross-cone-interface/16`。section 的十字段结构保持，field 4 改为恰含 field 1=公开查找记录、field 2=必要支持声明记录的 closed product；两组按 typed property declaration 排序，组内及跨组重复均拒绝。共有 PropertyDeclarationRecord 的八字段依次为 declaration、owner、own binder、extension receiver、value type、完整 getter/setter identity 与各自源码实现形式、representation 与 declared visibility。accessor 集合使用 ReadOnly 的 tag 4 和 ReadWrite 的 tag 5，getter/setter 各保存 typed id 与源码实现形式；旧 tag 1、3 退役，public setter capability 的 tag 2 不用于该声明集合。公开 property record 是三字段 product，依次保存共有声明、getter lookup access 与 setter public access；声明须为 public，read-only 的 setter access 固定为 Restricted。公开查找信息与源码声明分离，受限 setter、private/internal/protected property 以及 owner 受限的 declared-public property 均保留完整共有声明，不因此产生公开查找或机器能力。必要 property 从共有 nominal 的完整成员关系闭合，其类型和 binder 引用沿同一必要 nominal 闭包保留；不加入无关顶层属性。所有共有 property 的 getter/setter 必须在同一 callable 声明表精确覆盖，完整保存参数名与类型、结果、effects、modality、declared visibility 和实际 slot relations；getter 可见性与 logical property 一致，只有公开 lookup 指定的 accessor 进入 public 分区。producer 从同一次 sealed HIR 投影；普通 reader 从共有声明、同一 foundation 的 typed property/accessor key 与 definition origin 验证 owner、role、签名、representation 和完整覆盖，支持查询仍限于当前 provider。旧 section major、旧 property array/record、缺失或重复 property/accessor、错误 owner/role/type、私有 accessor 被提升为 public lookup 均拒绝；profile、HIR fingerprint 和缓存同步重建。该扩展不改变 runtime ABI，也不开放后续 generic、native 或 ODR 物化。

普通 `.slib` reader 在共有 property 与 accessor 的身份、owner、签名和 definition origin 关联完成后，重放 setter 的有效查找域包含检查。声明 visibility 与完整词法 owner 链逐项取交集；private 使用实际 source file 或 typed lexical owner，protected 使用实际 class 声明的 subclass 域。包含关系读取同一 provider 的共有 nominal 声明及完整继承关系，保留 generic 与 source-only owner，不以缺少机器 exact/layout 为由跳过检查。object/companion 的词法上下文沿其共有 supertype 关系参与 subclass 判断。公开和必要支持属性使用同一校验，owner 受限时不能只对 visibility 修饰符排序，也不能把 protected 与 internal 视为全序。缺失来源、错误 owner、用于包含判断的继承链循环以及更宽 setter 域均拒绝；查询、词法遍历和继承遍历计入 reader 的累计预算，不新增访问证明 wire 表，也不授予查找或机器能力。 writer 的 SignatureDependency 同时遍历共有 callable/property 的公开与支持分区，binder、receiver、参数、结果和 property type 使用与 reader 一致的 wire path；依赖类型按实际 typed origin 归属，受限 setter 的 Unit 返回值也必须保留在引用闭包中。

2026-09-23 共有调用声明约定：M23-6 的共有 callable 声明使用 `hir/cross-cone-interface/16`。section 的十字段结构及 nominal 分区保持；field 3 改为恰含 field 1=公开查找记录、field 2=必要支持声明记录的 closed product，各组按 typed callable declaration 排序，组内及跨组重复均拒绝。共有 CallableDeclarationRecord 保存 declaration、owner、own binder、extension receiver、完整参数名及类型、result、effects、modality、declared visibility 与实际 typed slot relations，共十字段；词法 owner 与 definition origin 继续查询同一 foundation，不另复制来源记录。公开记录是恰含 field 1=该声明、field 2=public lookup access 的二字段 product，要求声明确为 public 且查找/slot 合同一致；支持记录不产生 public binding 或查找能力。支持函数、source constructor 与 enum variant constructor 从共有 nominal 的完整声明关系闭合，保留 private、internal、protected 及 owner 受限的 declared-public 成员，排除 object 隐式初始化构造器和生成 adapter；被签名、binder bounds 引用的本地 nominal 沿同一必要声明闭包保留，不加入无关顶层声明。参数/default 协议与同一份 callable 声明逐项关联，不能用公开可见性过滤掉必要支持签名。producer 与普通 reader 共用 typed declaration/parameter 关系和完整签名验证；支持查询限于当前 provider，其他 provider 的签名仍只通过其公开接口解析。旧 major、旧单 array callable 表、旧九字段 callable 记录、错误 owner/角色、缺失或重复支持声明均拒绝，profile、HIR fingerprint 与缓存同步重建。该扩展只补全源码 metadata，不授予 generic、native 或 ODR 物化能力。

共有源码参数协议和默认值必须精确覆盖公开查找声明及必要支持声明中的所有 source callable；accessor 没有命名实参协议，仍由同一 callable 声明保存完整签名。不能把非公开参数改成 Required、删除默认正文或省略继承来源。必要源码声明集合从公开表面出发，沿完整 nominal 的词法子声明、继承、字段、成员签名与参数协议，以及每个默认正文的实际 typed 引用和原始定义 provider 迭代闭合；默认值中实际需要的本地顶层 function、property、nominal 进入对应支持分区，并递归保留它们自己的参数协议和默认值。正文内 local/generated callable 只由其实际附着的正文声明闭合，不提升为顶层支持记录。producer 与普通 reader 分别从真实 HIR 和已解析的共有声明/正文重算这一有根集合；缺失记录、多余记录及只能互相引用却不可从公开根到达的支持环都必须拒绝，不能让待验证支持表自行成为根。共有参数、正文、来源、类型、引用、访问域和预算规则对两类声明一致；公开查找资格与机器物化资格仍分别由公开关系和既有阶段能力决定。section 的字段与格式不变，不新增平行的源码证明表。

2026-09-23 类型事实归属约定：M23-6 的 exact fact 归属沿与 exact identity 相同的 typed nominal 输入计算。普通 struct、enum、class、interface 使用实际 source nominal key 的 origin；compiler 表示的整数、Boolean、String 在定义图中查询其真实 nominal owner，在导入图中使用对应 ImportedHirNominal 保存的 provider。Defined/Imported 只选择引用形式，不得据此把 source-defined 类型的归属固定为当前 Cone 或 CORE。Unit、Any 使用规范规定的内建 canonical declaration key 的 origin；tuple、function、Ptr、FunPtr 等结构类型不伪造 source nominal provider，继续按既有结构类型与 ODR 规则处理。local/dependency exact facts 的分类与缺失支持检查共用此查询，既不从 FQN 或显示名补身份，也不建立专用 core 事实入口；本次修正不改变 fact wire schema 或 runtime ABI。

2026-09-23 当前共有声明约定：M23-6 的共有 nominal 声明表使用 `hir/cross-cone-interface/16`。section 的十字段结构不变；field 2 改为恰含 field 1=公开查找记录、field 2=必要支持声明记录的 closed product，两组都按 typed source owner 排序，组内及跨组重复均拒绝。每个 nominal record 保留原八个字段，并新增必需 field 9 的完整声明信息：field 1=modality、field 2=declared visibility、field 3=全部声明构造器、field 4=全部声明成员、field 5=全部直接词法子类型；它们与原有 kind、binder、supertypes、完整 source shape 合成唯一的源码声明合同；词法 owner 与 definition origin 从同一 foundation 的 typed 声明 key 和 subject 记录取得，不在声明表再复制来源记录。原构造器、成员和 nested binding 字段继续表示公开查找关系，必须是完整声明关系中有效的公开子集；支持记录的这些查找关系必须为空。必要支持查询限于声明所属 provider；其他 provider 的签名仍只能通过其公开接口引用这些实体。object 隐式初始化入口保留既有 typed constructor identity，但不属于源码构造器声明，也不进入构造器查找。支持数据沿当前公共声明的继承、完整字段类型及声明的完整词法子树闭合；被引用的子类型递归保留自身完整声明，不引入无关的顶层 private 声明，保留 private、internal、protected 和 source-only 声明，但不产生 public binding、机器根或执行能力。producer 从同一次 sealed HIR 投影；普通 reader 通过已有 identity graph、definition origin、typed owner 和声明关系验证，并与 foundation 中属于这些 nominal 的构造器、成员和子类型 keys 对齐，拒绝遗漏的声明关系；公开与支持查询共用同一份 source shape、modality 与访问数据，不另存 nominal 来源证明副本。旧 `/1`～`/6`、旧 array 表形状、缺失声明信息、重复或错 owner 的记录均拒绝；profile、HIR fingerprint 和缓存随版本同步失效并重建。该共有声明扩展不改变 runtime ABI，也不提前开放 generic、native 或 ODR 物化。

2026-09-23 共有声明字段约定：此前由 `hir/cross-cone-interface/4` 引入、现由 `/7` 保留的 class source shape 为 `{ 0: 7, 1: declared_fields }`，object source shape 改为 `{ 0: 8, 1: value, 2: declared_fields }`；旧 class tag 1、object tag 5 退役，其余 shape tag 保持。字段使用共有的 typed field id 与完整 signature type，保留声明顺序、私有 backing/delegate 字段和泛型 binder 引用，不含基类字段前缀。object 字段必须属于从实际 object id 派生的 backing class。public、nested support、普通声明合同、representation join 与外来 signature 引用闭包使用相同字段信息；必要的私有声明引用只作支持数据，不加入 public lookup。旧 section major、旧 shape tag 和 optional 旧版本混入均拒绝，profile 与 HIR fingerprint 同步变化并要求重建 artifact/cache。该完整源码信息用于共有语义与机器依赖判定，本身不授予 layout、ODR 或 native 执行能力。

直接 `scoopc build` 在同一依赖快照上先检查显式 direct/support 的共有 summary，再决定是否补入默认 core artifact；实际 target、累计资源预算、输出隔离与 Compile/Link 闭包共用既有校验。显式提供 core 时不访问默认 sysroot，错误或重复显式输入不触发回退；single-file 的依赖输入限制保持。该发现顺序不改变 compiler protocol 或机器接口能力。

隐式 Array application 的前端入口与显式泛型应用遵守同一 M23-7 能力边界：普通依赖模式下的 `vararg` 参数及数组字面量在缺少本地数组声明表示时，分别于关键字与完整字面量报告 `SCOOP_HIR_CROSS_CONE_GENERIC_REQUIRED`，不进入要求本地 intrinsic arena 的构造路径。定义侧完整数组声明继续使用既有解析与物化规则，不为 core 增加授权豁免；具体职责见实现规范 2.2。

2026-09-23 当前 native C 投影约定：native witness 四字段记录新增必需 NativeBoundaryCAbiV1，封闭表达 SourceRepresentation、UInt64Field 和 NullablePointer，并保存实际 typed field/variant 引用。前端从完整声明角色正规化，依赖查询和 reader 按同一表示与完整字段闭包重放，不再使用 CoreNativeBoundaryNominal 固定身份。HIR foundation 升级为 /3，旧 native field 33 退役、新 field 34 必需，M24 的 HIR major 预留顺延为 /4；详细编码和验证见实现规范 2.11。C 投影不改变 Scoop aggregate ABI 或 runtime C 入口。

2026-09-23 GC handle Scoop ABI 修正：PinnedPtr/GcHandle 的 C 透明表示不适用于 Scoop 调用。Scoop canonical ABI 从实际 nominal 字段重放普通 aggregate，并按共有规则间接传参/返回，固定 core 身份不能省略 witness 或字段闭包；具体合同见语言规范 14.1、14.2 与实现规范 2.11。

2026-09-23 当前 Link requirement 约定：所有外来 strong 选择在共有步骤中按实际 provider/symbol/typed owner 解析，取消 core proof 与独立 core owner 参数。已有 verified dependency owner 集合同时服务外部 callable、descriptor、registration support 和普通 callable，按实际 capability/subject 保持用途互斥与 relocation 完整覆盖。最终 requirement 的旧 CoreStrong tag 2 退役，tag 8 保存 provider/typed strong owner，object-definition fingerprint 的扁平 target sum 使用独立 tag 13；link-identity-closure 升级为 /2，旧版本/tag 拒绝，profile、Code fingerprint、writer/reader 和固定向量同步。完整合同见实现规范 2.11。

2026-09-22 native intrinsic 迁移记录：NativeBoundaryNominalShape 使用新 tag 4 保存完整共有 NominalIntrinsicRepresentationV1，生产与依赖投影不再丢弃 family。HIR identity-foundation 升级为 /2，旧 field 30 退役，field 33 保存完整 native 类型表；三十二字段集合为 1～29、31～33，旧字段/版本及 optional 混入拒绝。标量 canonical ABI 重放按实际 typed 声明的 intrinsic family 计算，不再重建 Integer/Boolean 的 CORE 身份，Unit 保持语言内建 identity；Option 与 GC handle 随后改为显式 typed C 投影，当前 HIR foundation 为 /3、native field 为 34，见本文首段。具体表示、覆盖与版本合同见实现规范 2.11。

2026-09-22 当前外部引用约定：String 和初始化 callable 使用共有 typed provider/definition 记录，禁止从 CORE 常量恢复 provider。strong production 的旧 field 1 退役，field 12 保存共有外部引用；旧 callable tag 1 退役，tag 3 直接保存完整 SelectedDependencyLirCallableV1，TypeDescriptor tag 2 保留已有含 provider 的载体。两种 TD/dispatch schema 统一使用显式 provider 的 DependencyExternal，CoreExternal tag 不复用；immortal registration 使用新的 tag 3 provider/exact 引用。strong-production /3、/4 升级为 /5、/6，cross-cone-layout-abi 与 cross-cone-layout-link-closure 升级为 /2。旧格式和 optional 版本混入均拒绝；local/runtime/absent、String 表示、runtime C ABI 和用途分区保持，详细编码与验证见实现规范 2.11、2.12。

2026-09-22 初始化服务 ABI 迁移记录：LIR 删除 CoreLirBridge 及 Core/NotCore 外层，strong production 的旧 field 10 退役，新增 field 11 的 0/1 array，直接保存完整共有 CallableAbiRecordV1。strong-production 在该批从 `/1`、`/2` 分别升级为 `/3`、`/4`，随后按上面的外部引用约定继续升级为 `/5`、`/6`；后者保留 V2 layout reference schema；旧字段、旧 tag、旧版本和交叉 profile payload 均拒绝。MIR 实际角色决定 ABI 有无，MIR/LIR 导入投影和 callable 选择按所选实际 provider 验证 typed target、body、symbol、definition 和完整签名，不追加 CORE 来源条件；calling convention、GC effect、root plan、预算、registration 与用途分区保持。详见实现规范 2.12，runtime C ABI 与 String 表示不变。

2026-09-22 当前 HIR 协议定义约定：`core-bootstrap-interface/3` 删除 `CoreHirInterfaceBranchV1`。section 的旧 field 1 及 Core/NotCore tag 退役，保留 field 2 output、field 3 direct surface，新增 field 4 长度 0/1 的完整 definitions array；由实际 Defined/Imported 决定发布，不按 CORE 身份或输出种类选择分支。String 按实际非泛型 class 与 exact nominal 关系解析，intrinsic 按其 canonical 声明 origin 查询对应可达 provider 的完整类型角色。旧版本、旧字段、多份定义、缺失/冲突角色和错误关系均拒绝；typed 引用、effect、签名、成员、预算和后续布局/registration 检查保持。完整合同见实现规范 2.12；LIR/Link 外层另行迁移，runtime C ABI 与 String capability kind 不变。 MIR production 同时删除基于 CORE 坐标判定初始化角色缺失、多余或必须 library 的分支；角色有无交给相邻 HIR 实际定义核对，MIR 的唯一性、source function、完整签名覆盖、entry 归属与实现检查保持。

2026-09-22 当前 nominal intrinsic 约定：共有 source shape 以新增 tag 6 保存完整 `NominalIntrinsicRepresentationV1`，public、source contract、nested support 与独立 representation 使用相同 family、声明 kind 和 binder 合同；不再将 intrinsic 退化为空 struct 或普通 class。`hir/cross-cone-interface` 升级为 `/3`，旧 major 拒绝并重建产物。公开常量与 vararg reader 沿自身完整 typed 类型引用查询实际 provider，核对 intrinsic kind 及元素类型关系，删除这些消费者的 trusted-core 查询；不以同名、同布局或全局候选扫描替换类型 id。最小 native source witness 保持既有 wire，完整规则见实现规范 2.11。

2026-09-22 当前 Unit binding 约定：protected callable 与 dispatch slot 使用语言内建 Unit identity，删除仅为取 Unit 而传入的 imported core 协议；setter result、slot signature 和完整 exact key 的一致性检查保持。其他 intrinsic/source nominal 仍必须查询实际声明与 provider，不能套用 Unit 例外，见实现规范 2.11。

2026-09-22 当前整数正规化约定：源码intrinsic调用完成共有成员候选及参数/effect检查后，HIR只保留封闭typed operation、conversion和操作数；导入声明不物化成本地函数，后续IR和default body不保留冗余callee。整数default wire删除该字段，旧格式要求重建；exact identity、witness及extern/callback bytes保持。导入成员复用共有源码调用探测与winner commit，完整规则见实现规范2.10。

2026-09-22 当前intrinsic声明约定：共有CallableSourceEffectsV1直接保存Intrinsic(IntrinsicFunctionKind)，不以标志或core专用operation表补全kind，不携带provider授权或annotation字符串。closed-sum wire的field 0为实现tag，仅Intrinsic具有field 1=完整typed kind；旧unsigned leaf要求重建artifact/cache。整数kind的GC effect与Ordinary execution由builder和reader共同验证。常量及静态initializer从共有callable目录取得整数语义；method/infix同时按typed声明id读取canonical源码名称、source参数名和infix属性，解析结果不依赖本地FunctionId。删除ImportedCoreProtocols中的重复compiler-operation载体；源码名称只用于匹配，不用于反推kind，非const实例成员仍须接入普通跨Cone调用选择。完整规则见实现规范2.10。

2026-09-22 当前外部描述符约定：String的旧外部描述符桥不再复制target/symbol/definition；它直接保存与通用layout选择、foundation投影和LIR arena相同的完整ExternalTypeDescriptor。共有wire为四字段closed product：1=provider、2=target、3=expected_symbol、4=required_definition；旧隐含CORE的三字段格式拒绝，artifact/cache须重建。reader在共有identity graph中解析provider和exact target，由实际provider重新推导symbol/definition并逐字核对完整record，definition的owner/role/primary symbol使用同一关系校验。String协议外层继续从well-known明确引用选择记录并验证所需CORE provider；不能把其他外部描述符按provider归入String角色。不再保留StrongExternalTypeDescriptorBridgeV1、core专用TD identity helper或CoreExternalBuildError。

2026-09-22 当前callable生产约定：LIR初始化服务、普通调用桥和通用layout/ABI发布共用实际callable关联：从typed StrongCallableDefinitionOwner取得同一MIR strong记录、实际物化root与对应LIR body，并核对exact签名。普通调用与初始化调用使用同一canonical ABI投影，统一检查GC effect、calling convention及逻辑参数数量，再构造完整CallableAbiRecordV1；初始化角色额外限定function、ordinary、无receiver。通用layout/ABI继续重放自己的layout/physical证明，但不再重复查找MIR/LIR函数；其resource meter在共有查找前按相同表长度计量，不免除预算。投影失败返回携带typed target的共有错误，不按core名称或symbol字符串补目标。此批合并生产实现，不改变角色外层wire和public可见性。

2026-09-22 当前LIR清理约定：初始化发布、外部调用与普通依赖共用CallableAbiRecordV1及ExternalCallableRootPlan；普通调用桥wire为declaration与完整ABI record组成的两字段product，旧七字段格式拒绝。provider显式输入共有identity/definition推导，初始化外层角色仍明确保存。

2026-09-22 当前清理约定：MIR production 删除独立 CoreMirBridge 与字段1；所有 strong callable 记录使用必需 CallableRole，初始化声明与实现只保存在共有记录中。HIR 协议、MIR foundation 与 LIR 投影按同一 typed 实现和签名核对，旧记录格式拒绝并重建；详见实现规范的当前约定。

M23-6当前约定：MIR初始化服务和普通依赖共用一个完整的`SelectedExternalMirSet`及typed选择引用。初始化角色显式保存，typed bridge与签名检查后进入共有集合，不保留core专用MIR借用凭证或泛型协议sidecar。strong输入使用共有consumer/覆盖/引用校验；旧wire与LIR协议仅在投影边界按角色区分，String与ABI契约保持完整。以`SCOOP-IMPL-SPEC.md`的共有外部调用约定为准。

LIR初始化服务与普通依赖使用同一`SelectedExternalLirSet`，完整保存provider、typed declaration/target、canonical ABI、calling convention、root plan、symbol及definition；选择角色与MIR使用同一语义枚举。MIR strong输出只保留一张完整外部callable根表，LIR共用选择覆盖、角色、签名、GC effect、参数/结果exact type查询和物理ABI分类，不再保留core专用callable集合或借用凭证。初始化服务与普通调用只在旧metadata角色物化时区分。String的foundation投影返回已有的完整`ExternalTypeDescriptor`，lowering显式接收Local/External描述符输入，所有测试也经过同一输入；不保留仅测试可用的runtime String替代分支。上述合并不改变String表示、内部函数源码可见性或旧wire契约。

共有依赖闭包将完整 SelectedExternalMirSet 一次投影为 SelectedExternalLirSet，按每条记录的实际 provider 查询已验证 artifact，保留 Ordinary/InitializationCycle 的既有用途角色。初始化服务与普通 callable 共用 consumer、provider、typed declaration/implementation、完整签名、canonical ABI、definition 及集合覆盖检查；不能先跳过初始化角色，再由 core 包装对象补入第二份选择。初始化 ABI 缺失、角色与真实目标不符、签名不一致或错误 provider 均由共有投影拒绝。String 的 TypeDescriptor 同样由实际 source nominal 与 provider 进入共有 descriptor 查询，核对有限 shape-support、exact type、symbol 和 definition；该查询也适用于当前已发布完整 shape-support 的普通 nominal，不授予额外机器物化资格。删除 driver 的独立 core LIR callable/descriptor 投影适配，保留前端解析的语言角色。此迁移不改变 wire、runtime ABI 或源码可见性。

协议导入保留实际 definition-origin 的 provider 与完整 typed callable 引用；其 provider 来自已验证的共有声明 metadata，不由导入 artifact 的身份补齐。driver 在依赖闭包完成后只保存前端所需的 ImportedCoreInputs，不再保存 ValidatedTrustedCoreArtifact 或另一份 Compile/Link view。初始化服务由共有闭包核对所选 provider、source function、完整源签名与 InitializationCycle 角色，再借用该 provider 已验证的 MIR exact signature 加入同一 selected set；driver 不重新拼接 String/Unit 签名。String descriptor 使用导入 nominal 自带的 source provider 与身份。缺失协议、错误目标、签名不一致、provider 不可达或重复选择均由共有入口拒绝。前端仍按语言规则选择默认协议库，internal 服务的源码可见性、wire 与 runtime ABI 不变。

MIR type bridge 的本地类型导出必须包含由当前 provider 拥有的语言内建 Unit/Any，即使它们没有普通源码声明 arena 或 nominal representation-support 记录。HIR facts 先完整发布当前 provider 拥有的 Unit/Any 固定语义，不能仅依赖普通 nominal roots 或源码签名触发它们。MIR 生产器从同次 sealed MIR 的 typed source-exact 引用与 HIR exact facts 投影其固定表示，并核对语言内建 nominal identity 及声明 provider；Unit 保持 ZST/GcFree，Any 保持根类型的 reference 语义，MIR 使用无成员、无 base 的 abstract class 表示和空 class dispatch schema，并由 LIR 生成既有 abstract-reference 布局；其 class-kind identity 不变。外来 Unit/Any 继续由依赖类型表提供，不能在每个 consumer 中重复发布，也不能将这一规则推广为 CORE 来源的任意类型豁免。

HIR selected 目标以封闭分支区分语言内建类型与普通源码 nominal：普通分支必须同时借用已验证的表示和继承记录；Unit/Any 分支按固定 typed declaration identity 及其真实 origin 核对 terminal provider，并借用该 provider 的完整 exact facts，分别保持 ZST/GcFree 与 reference/managed 语义，不补造源码表示或继承记录。内建分支可承载 Signature、Representation、TypeTest 和 ShapeSupport 目标存在性检查；实际 shape-support 及机器使用仍由既有完整闭包验证。构造、成员、槽、单例和直接继承仍须各自真实声明与关系，不因内建身份获得操作资格。每次实际使用、语义边、来源、依赖可达性、selected 精确覆盖和累计预算仍经过共有检查；该区分不增加 wire tag 或新的来源证明表。

MIR callable inventory 复用共有 nominal 表示与继承闭包检查 owner、receiver、参数和结果；依赖 source-only nominal 的未物化声明保留完整源码接口，不要求不存在的机器正文，可物化声明缺失及实际使用的能力门保持错误。

有限 shape-support 的装箱也从值类型的实际 typed 声明读取全部接口，并生成同一套 adjust thunk/itable；不能把 Integer/Boolean 的 intrinsic 表示当作没有接口。String 的 intrinsic 表示保持其源码 class 的 vtable/itable 要求，使用相同的 owner kind、receiver、slot 与 target 校验。LIR shape-support 的 source-kind 检查允许 struct 对应普通字段表示、intrinsic scalar 或固定 Unit value；具体 intrinsic family 仍由完整 HIR/MIR/layout join 核对，不能从 CORE 来源推断。

完整 core 源码及新增值类型、接口、类和初始化声明的闭合子集必须通过同一 MIR/LIR export 组装路径；不得用测试手工补入基础类型表代替实际 producer 输出。此修正不改变语言行为、runtime ABI 或 wire 结构。

core 是可由用户修改、扩展和重建的普通 library Cone。源码层面的特殊处理仅限于前端识别 `@Intrinsic`，并把它正规化为既有 typed IR，以及 desugar 通过普通声明引用使用基础库提供的类型和函数。sysroot 是默认查找位置，不是信任边界；源码目录、输出位置、相同 coordinate 或用户修改过的 core 不需要授权 token。metadata 解码、typed identity 一致性、依赖闭包、ABI、缓存失效和 slib fingerprint 使用所有 Cone 共用的规则。不得为 core 另建来源防伪、slot 授权、receipt 信任链或重复 pipeline；既有专用实现须合并或删除，旧文档的冻结条款不阻止此次清理。

当前优先目标与验收改为[core 普通 library 清理](../CORE-LIBRARY.md)。暂停扩展六类default来源授权、防篡改证明及独立source-authority框架。已有typed语义和普通metadata保持可用，专用证明链按实际依赖删除或合并。

版本：1.2（core 普通 library 与重复检查清理；2026-09-21）

依赖：M23-5

上位设计：[M23 系列设计](../DESIGN.md)

规范依据：

- [语言规范](../../specs/SCOOP-SPEC.md) 第 3.3、4.7、7.4、9.1、10、12.5、13.4～13.10、14 章；
- [Runtime 规范](../../specs/SCOOP-RUNTIME-SPEC.md) 第 2.1～2.8、3.1～3.3、3.6、4.2 节；
- [实现大纲](../../specs/SCOOP-IMPL-SPEC.md) 第 2.2～2.6、2.9、2.11 节；
- [M23 总设计](../DESIGN.md) 第 3.1～3.6、4.4～4.6、5.3、6.1、9.4～9.5、10 章；
- [M23-2](../stage2/DESIGN.md) 的 exact identity、`ExactOwnerRoot`、native witness、target projection 与 canonical ABI 编码；
- [M23-3](../stage3/DESIGN.md) 的 strong production、有限 shape-support、definition/image proof 与双 view；
- [M23-5](../stage5/DESIGN.md) 的 semantic world、access provenance、selected metadata 与 cross-Cone object-use 分区。

本文中的 runtime ABI 指 M23 基线：TypeDescriptor 最后一个字段是 `diagnostic_name`，callable body 使用 v1 identity，三层 metadata 的 `outer_schema` 均为 1。规范中已提前写出的 M24 `release_hook`、callable-body-v2 与 schema 2 不属于本阶段。

## 0. 结论

M23-6 把已有的本地类型表示变成可独立验证、可跨 Cone 消费的接口。成功解析到外部 nominal 之后，consumer 必须取得定义方的完整语义、layout、ABI、scan、TypeDescriptor 与 dispatch 证明，才能产生 machine use。

1. 新增 `cross-cone-layout-strong/1` production profile。它在 M23-5 inventory 上新增 HIR type/inheritance、MIR type bridge、LIR layout/ABI 和 Link-only layout-use closure 四条 section，并将强定义语义升级为 `strong-production/6`，以表达普通依赖的 TD/dispatch 引用；仍拒绝全部 ODR production。完整声明与实际使用信息按 3.1.1 合入共有 metadata，不新增独立来源授权 section。
2. persistent identity 与 extern/callback contract bytes 保持；native-boundary witness 按当前 intrinsic family 与 typed C 投影合同显式升代，旧字段和版本退役。一般 layout 服务只能由新 required section 构造，不从 native witness 推出通用 layout、scan 或 dispatch。
3. HIR 输出每个 concrete type 完备的 `gc_free`、value `ZstStatus` 与继承/slot 语义；MIR 输出表示无关的类型、构造器、成员、slot 与生成 helper 关系；LIR 独占 target layout、Scoop ABI 与递归 scan 的生产权。
4. 外部实体保持定义 Cone 的 Strong ownership。consumer 可以检查和在本地类型中内联外部 value 的表示，但不能重新定义其 body、layout constant、scan、TD、dispatch table、registration 或初始化 storage。
5. 定义 Cone 为每个可跨 Cone 引用的 param-free source nominal 预物化完整、有限的 `BoxedValue`（仅 value）、`CoroutineStep`、`CoroutineSlot` shape-support。closure 从 source subject 展开一次，不递归把 helper 当作新 source subject。
6. 通用 `ValueStorageLayout` 使用 `ZeroSized | NonZero`，zero-sized 分支不能携带 ref scan；未装箱 value layout 与五类 managed instance shape 分离。可分配操作只能接收排除 `AbstractRef` 的 refined descriptor。
7. Scoop ABI 保留全部 logical exact type，并用 `ElidedZst` 删除物理 payload。`UnitVoid` 与用户 ZST result 分开；全部 direct、invoke、virtual/interface、adapter 与 Scoop extern 复用同一分类和 logical-to-physical 映射。
8. ZST 的求值、异常、构造和方法语义不消失。需要地址的 local/parameter/value `this` 使用独立 token，static token 沿已有 persistent storage contract；每次 boxing 仍有 fresh managed identity。
9. `Array`/`MutableArray<ZST>` 保留 logical size、bounds、求值和 index iteration，allocation 不随 length 增长，不能生成 zero-stride scan、payload copy 或逐元素 token。
10. 开放 closure 完备的 param-free 跨 Cone 构造、value 投影、member/accessor、object value、继承、virtual/interface dispatch、`is/as` 与 protected access。protected bridge 是带用途证明的元数据通道，不是把 protected 声明提升成 public wrapper。
11. 通用表示算法和 compiler/layout/object 测试本阶段覆盖 generic/structural shape；需要独立 Nominal/Structural ODR materialization 的生产请求仍留 M23-7。`Array<T>`、tuple、function、pointer 不因布局简单获得 Strong 特赦。
12. 本阶段产出双 view 有效的多 Cone `.slib`，验证 local runtime 表示操作；真实 multi-image startup、artifact-only program-link 和多 Cone moving-GC 分别留 M23-8、M23-9、M23-11。

核心关系为：

```text
source access proof + complete type/inheritance interface
    -> committed exact external use
    -> selected MIR representation/dispatch relation
    -> selected LIR layout/ABI/scan/descriptor proof
    -> metadata-only dependency | verified physical Strong use

same size/alignment != same exact type
same physical signature != same callable contract
external layout knowledge != permission to emit an external definition
ExactOwnerRoot == SourceCone != exemption from dependency ODR requirements
```

## 1. 范围、基线与阶段边界

### 1.1 当前实现基线

截至 M23-5，仓库已具备若干可复用的本地表示，但尚未形成一般跨 Cone authority：

| 位置 | 已有能力 | 本阶段工作 |
| --- | --- | --- |
| `compiler/lir/src/abi.rs` | `AbiZst`、Direct/Indirect/ElidedZst、logical-to-physical 参数映射 | 绑定 persistent exact type 与 imported layout proof，统一全部调用入口 |
| `compiler/lir/src/type_descriptor.rs` | checked value/array storage、五类 instance shape 与扫描约束 | 形成跨 Cone required wire、closure validator 和 refined external descriptor |
| `compiler/lir/src/metadata.rs`、`compiler/lir-lower/src/metadata/layouts/` | 本地 aggregate/enum/field layout | 收口 raw size/alignment 构造，接入定义方布局与继承 prefix |
| `compiler/lir/src/production/shape_support/` | core 的有限 shape-support proof | 合并到所有定义 Cone 共用的形状表，移除 core 专用查询分支 |
| `compiler/hir/src/visibility.rs`、`compiler/hir-lower/src/visibility.rs` | lookup/inheritance/slot domain 与本地 protected 规则 | 导出 persistent inheritance surface，生成跨 Cone receiver witness |
| HIR/MIR/LIR 的 `cross_cone_*` 模块 | M23-5 core-closed callable 子集与 selected bridge | 增加一般类型、构造、slot/dispatch 和 object-value selection |
| `compiler/lir-lower/src/function/expression.rs`、`runtime/src/rt.c` | 现有 box/unbox/array 执行路径 | 消除旧 payload/size/scan 多源调用与固定 header-offset 假设 |
| `compiler/codegen/src/function.rs` | 本地地址存储发射 | 在 LIR 明确 token place，codegen 不再由 LLVM 空类型猜 ZST |

本文的“首次冻结”指可跨 artifact 复用的通用契约，不表示重新实现所有本地算法。现有内部类型只有经过新 proof 构造器验证后才能进入导出表；Rust DTO 已存在不等于它已是稳定 wire。

### 1.2 生产成功矩阵

| 使用 | M23-6 结果 | 必要证明 |
| --- | --- | --- |
| M23-5 已成功的 const、top-level/extension callable | 继续成功 | 旧 route/bridge 原样保留 |
| param-free public struct/enum 参数、返回、构造、模式、copy update | 成功 | public representation + MIR shape + LIR layout/ABI |
| param-free class allocation、base constructor、member/accessor | 成功 | constructor access、完整 object shape、initializer bridge |
| param-free interface/default/virtual dispatch、`is/as` | 成功 | 完整 ancestry、slot contract、唯一 implementation 与 TD |
| public object/companion value及 runtime property | 成功 | provider ensure/value/accessor target 与初始化 ownership |
| subclass 中合法 protected member/constructor/nested type 使用 | 成功 | inheritance route、用途专属 access witness 与完整所需 bridge |
| imported value 嵌入本地 field/enum/capture | 成功 | representation closure；新生成实体的 ownership 也须通过 Strong gate |
| generic nominal/callable application，generic delegated extension | M23-7 能力诊断 | 不生成 partial LocalConcrete/MIR |
| 独立 tuple/function/raw/native-pointer TD 或其他 Structural ODR entity | M23-7 能力诊断 | 不因物理布局简单改成 Strong |
| suspend external call、需要 `ContinuationShell`/`CoroutineStart` 的操作 | M23-7 能力诊断 | hidden ABI 依赖 generic nominal application |
| direct foreign source `@Extern`/native storage 使用 | M23-10 能力诊断 | 完整 native closure 尚未开放 |

所有“成功”格还要求传递物化闭包不包含 M23-7/10 能力。param-free declaration 不保证其字段、签名、默认参数或 generated dependency 也是 param-free；gate 必须检查实际闭包，不能只检查声明的 type-parameter count。

发布源码接口与发布机器接口分别按其实际需求闭合。普通 library 可以保留完整 generic 或暂不可物化的源码声明；只有完整机器依赖闭包满足本阶段能力的声明进入 layout/ABI/dispatch 导出与有限 shape-support。未被实际物化的声明不能仅因存在于源码接口就使整个 library 失败，也不能通过删除其公开类型、继承、成员或默认值信息来绕过 gate。实际本地使用、可发布机器根或下游使用一旦需要 generic/Structural ODR 或 native 能力，仍在该使用处报告 M23-7/10 诊断，不能发出部分机器接口、空实现或替代定义。此规则对 core 和普通 Cone 相同，包括 `IntRange : Iterable<Int>` 这类无自身类型参数但依赖 generic application 的声明。

M23-6 的 HIR nominal type 导出先从同一次 Export/LocalConcrete HIR 计算表示与继承的必需集合。依赖包括全部自身字段、enum payload、class base、interface ancestry、公开或 protected 构造器参数，以及实际 virtual/interface slot 的参数、结果与 execution；generic application，或依赖泛型/挂起 ABI、native 定义的 slot，使该声明保留为 source-only，并沿本地 nominal 依赖传递。引用类型环按有限图求闭包，不能因遍历顺序误判，也不能通过截断继承、字段或 slot 消除依赖。完整源码根、kind、binder、modality、supertype、成员、构造器与默认值仍保留；只有闭合的参数自由根进入 representation、fact 和 exact inheritance 表。候选表与从声明计算的必需集合逐项覆盖，可物化根缺失或 source-only 根混入均拒绝。该计算不按 CORE 身份或 source 路径分支，不授予 callable body、独立 Structural ODR、native、shape-support 或实际跨 Cone use 能力；这些仍由完整机器闭包与对应阶段能力门检查。

M23-6 的 nominal 表示与继承物化闭包直接读取共有 nominal/callable 声明；它是有预算的临时查询结果，不保存第二份来源、资格或 exact 清单。公开记录和必要支持记录使用同一完整字段、enum payload、supertypes、公开或 protected 构造器参数及实际 slot 签名；getter/setter 与普通函数同样参与。generic application、未绑定 binder、generic/suspend/native slot 将所属 nominal 标记为 source-only，沿当前 provider 的 nominal 依赖反向传播；环按有限图求不动点，结果不依赖声明顺序。structural 签名递归检查子类型，但不因此授予独立 Structural ODR 物化。HIR producer 与 artifact reader 重放同一算法，保持完整源码声明；形状需求只从当前 exporter 的 DeclaredCurrent 公开根中选择闭合子集，不能因 source-only 声明存在而要求不存在的 shape-support。外来 nominal 仍由其实际 provider 的机器接口及消费闭包验证；本地临时集合不授予外来机器使用、访问权或后续里程碑能力。Link 入口接收已验证的 Compile artifact，先核对完整 artifact fingerprint、实际 Cone、target 与 compatibility，再借用其中的共有声明和 LIR bridge；调用方不能另行传入候选声明替代该来源。Link 用自己的累计预算重算形状需求，不从待验 LIR 形状表反推需要集合，也不增加持久化资格字段。

共有 nominal 物化查询必须保留各条依赖的语义角色：普通字段、enum variant 字段、完整直接继承类型、公开或 protected 构造器、实际 virtual/interface 槽。查询只借用同一 canonical nominal/callable 声明；字段保留原 typed field identity，callable 保留完整原声明、签名与 effect，不能把 private 字段、重复类型位置或 accessor 槽归并后丢弃。generic owner 不进入本阶段的参数自由物化遍历，private/internal 构造器及非槽普通成员不自动产生类型机器需求；构造器/槽所属的参数自由 nominal 必须存在于同份声明集合，不能静默忽略悬空 owner。槽需求同时来自声明自身的 slot relation 和共有 nominal 的实际 dispatch selection；值类型或 final 类的接口实现即使没有自己的物理槽，也必须按被选择的完整 typed callable 加入签名与 effect 判定，不能仅因 slot_relations 为空而漏掉实现。相同实现被多个槽选择时，保留原声明的完整参数序列；实际选择关系仍由原 nominal metadata 单独验证。现有 source-only 传播与后续 selected 语义边生成应复用此查询，遍历、回调及类型递归使用同一累计预算；它不保存新 wire 表，不授予来源访问或机器资格。

M23-6 的自动 nominal 物化根以完整共有声明的表示与继承闭包为准；内部声明与公开声明使用同一算法，source-only owner 不因出现在 source arena 中就生成本地类型、构造器、成员或 slot 正文。自动成员与构造器根的签名若依赖 source-only nominal，也不单独预物化；实际本地调用、构造及已求值正文中的类型需要仍通过同一 typed 请求和固定点队列产生完整实例。generic callable application 的来源记录不是独立发射根，未求值 default 或未实例化 generic body 中的记录不能触发物化。对象、companion、singleton value、published root、initialization unit 与 failure root 使用各自显式的 Export→LocalConcrete 映射，不转换 source arena 下标冒充 concrete id；lazy 初始化仅随真实对象需要进入闭包，eager 全局初始化仍是必需根。编译器协议的完整输出以已验证的 typed 角色引用作为明确请求根，经过相同队列，不能依赖整个 core 声明表的预物化。nominal type 生产先按共有声明筛选完整表示与继承集合，再核对所需根的 Export/LocalConcrete exact pair；source-only 声明无需伪造 concrete 实例。

### 1.3 表示覆盖与 ODR gate

本阶段完整定义和验证 `Array<ZST>`、`Option<ZST>`、`Phantom<A/B>`、tuple/function/pointer 的表示，不据此开放这些 application 的 production ownership。

- compiler/layout tests 可直接构造 fully concrete typed input，验证算法、IR、wire constituent 与 object emission；不能伪装成已通过 production profile 的 artifact。
- runtime 单 image harness 可验证 box、array、scan、token 的真实行为；不得将它报告为多 Cone startup/program-link 验收。
- production 继续应用 M23-3 §10.2～10.3：任何 ODR record、body、symbol、definition node 均拒绝，哪怕只有一个 producer。
- source-owned aggregate 的嵌套结构只为计算外层 shape 时，可用无 persistent identity 的 transient layout value；结果吸收进外层 layout/scan。该例外不生成独立 TD、dispatch、box、callable 或 addressable entity，不授予 nested generic application 的源码执行能力。

Stage 7 只补物化和一致性证明，不能再修改本阶段冻结的零尺寸、field offset、scan 或 typed ABI 规则。

### 1.4 不属于本阶段

不新增源码语法、泛型能力、target、C register classifier、runtime release hook、friend visibility、field/array-element `addressOf`、动态 image、最终 linker 或正式 `scoop run`。M23-4 locator/cache/调度和 M23-5 名称候选顺序保持既有契约。

## 2. crate 与输入边界

```text
scoop-hir       type facts / inheritance interface / selected use / access witness
scoop-hir-lower 语义检查、receiver proof、完整外部请求与 winner commit
scoop-mir       representation-neutral type/callable/slot bridge
scoop-mir-lower 外部构造、ensure、dispatch 和 helper 的机械 lowering
scoop-lir       layout/ABI/scan/descriptor DTO、proof、wire 与 verifier
scoop-lir-lower 唯一 target layout producer、外部 proof 消费与 root/place plan
scoop-codegen   完整 LIR -> LLVM/object，不推导语言行为或缺失布局
scoop-slib      section/profile、原子 closure 验证、双 view 与 publication
scoopc          selected metadata 投影、stage 编排
scoop           accepted profile/cache key 更新
runtime         既定 shape ABI 下的 box/array/scan 执行与防御验证
```

不新增 stage 实现间依赖。导入算法需要的通用验证逻辑放在对应 IR crate；不能让 `scoop-slib` 调用 `hir-lower` 或 `lir-lower` 来重编译上游。target-aware layout 的纯数据验证与重放 API 由 `scoop-lir` 提供，lowering 调用相同构造器。

ordinary callable 的 canonical ABI 校验对 core 与普通 provider 无差别执行。旧 callable bridge 借用当前 artifact 与其实际可达依赖的 native-boundary witness，重复 owner 必须完整一致；layout profile 在完整本地和依赖 section 通过后，按 exact type 查询唯一的 ManagedValue layout，不回退到 witness。两条路径使用 LIR 共有的 direct/indirect/ZST 分类，逐项核对 logical signature、GC effect、参数次序与结果；缺失、重复或不匹配的 layout 拒绝，查询与签名复制持续使用当前 artifact 的预算。此处共有 ABI 重放不代替 native-boundary 专用外来类型入口与固定 core 身份识别的后续迁移。

native-boundary reader 在同一 validated identity graph 中解析本地与外来声明，统一重建 owner、kind、参数个数、binder 及成员关系；不保留 external-core 的零字段/Reference 特许恢复分支。完整性查询共享当前和依赖图的 canonical field、variant 与 variant-field records，依赖 key 以共享引用读取，不加入本地定义 inventory。缺少 canonical 声明或遗漏实际成员必须失败，闭包查询使用调用方预算。generic 声明的结构解析不授予 generic application 执行或 ODR 物化能力；profile 的既有 gate 继续检查。producer 的外来类型输入按下述共有 world 规则闭合；后端 typed 表示与通用 layout/ABI 查询的连接仍须按清理设计完成。

共有 struct source shape 在 `cross-cone-interface/4` 中精确为 `{ 0: 3, 1: source-order fields, 2: NominalCLayoutPolicyV1 }`；policy 使用 type-semantics 的既有闭合编码，由实际 HIR `@CLayout` 属性投影。公开 source shape、nominal source contract 与 nested source support 共享该结构，representation join 必须同时比较字段与 policy。intrinsic source shape 使用新增 tag 6 明确保存完整 family，并与 representation 比对，不能使用普通 struct/class shape 代替。旧 `/1`、`/2`、`/3` section 及旧两字段 struct shape 直接拒绝并重建产物，profile fingerprint、inventory 和 HIR fingerprint 随之更新，不回改 native-boundary witness 或 C ABI。本次扩展只补齐声明事实，不以 source shape 授予 layout、scan 或物化能力。

native-boundary producer 直接借用共有 dependency world，不再接收 `CurrentArtifactOnly/TrustedCore` sum 或 `ImportedCoreNativeBoundaryTypes`。外来 owner 必须解析到实际 provider 的 canonical source key；共有 v4 source shape 提供 intrinsic family、完整 struct CLayout/字段或 enum variant/字段，成员 key 来自同一 provider。非公开但已有 native witness 的声明可沿已持有 typed 引用继续闭合，Reference 由真实声明 kind 决定；共有 source shape 与已有 native witness 同时存在时逐项一致。缺失或不一致立即失败，不按 CORE、名称或空字段补默认记录。本地和外来 shape 都进入同一 signature-type 传递遍历，支持跨 direct/support provider 的字段/variant 闭包并按实际 owner 规范化；不把该最小 witness 作为一般 layout、lookup 或物化能力。

compiler protocol 的 constituent 仅验证实际声明与角色关系，不额外要求当前 export、声明 owner 或导入 foundation 的来源为 CORE。已有 identity/definition-origin 检查、完整 signature 与 binder、constructor/generated-adapter source、enum kind/member owner、effect 和固定角色完整性继续执行，普通 provider 的相同声明经同一路径验证；缺失或不一致不能以来源身份豁免。Unit 保留既有语言内建身份。producer/reader 删除重复来源资格，typed 引用编码与前端 intrinsic 使用边界不变；完整 operation 表按下一段退役，HIR 的 Core/NotCore 外层按本节 `/3` 修订迁移；其余协议投影及 String/初始化服务的 LIR/Link 外层继续共有化。

完整 intrinsic operation 表从 protocol product 的 field 9 退役，HIR `core-bootstrap-interface` 的 `/2` 修订使协议 product 只接受 field 1～8，当前 `/3` 同时使用本节开头规定的 definitions section，旧 `/1`、`/2` capability 与旧九字段 payload 拒绝；field 9 保留为退役编号。共有 callable effects 是发布 intrinsic kind 的唯一记录，普通 Compile 与 layout-profile 均在实际共有 callable/public-source 验证路径中检查 source owner、own binder、参数/结果与 execution，所用语言类型角色来自已解析的实际 provider 依赖闭包，不能由 CORE 常量或名称补出。验证按已有 callable 表逐项执行，重复 kind、角色缺失/冲突和错误签名拒绝，查询与临时记录沿用同一预算。前端完整 intrinsic 声明检查和固定语言协议角色继续保留；没有实际导入消费者的完整 operation 表不作为新的授权或执行目录。

输入链扩展为：

```text
ValidatedArtifactClosure<Compile>
    -> ValidatedCrossConeSemanticClosure
    -> ImportedTypeSemantics + ImportedInheritanceSurface
    -> HIR winner + CrossConeUseSet
    -> SelectedMirTypeBridgeSet
    -> local MIR + SelectedLirLayoutAbiSet
    -> complete local LIR + external typed definitions
```

`SelectedMirTypeBridgeSet`、`SelectedLirLayoutAbiSet` 是经过 dependency closure 检查的 branded handle，不接受裸 table、symbol 或任意 exact-id 列表。MIR 无法读取 default/import/access 语法，LIR 无法读取 Export HIR body，codegen 无法重新查询全部 dependency。

## 3. profile、section 与 wire 演进

### 3.1 新 production profile

新增：

```text
org.scoop-lang.slib-profile/cross-cone-layout-strong/1
```

其 required inventory 从 M23-5 `cross-cone-semantics-strong/1` 出发，移除 `org.scoop-lang.lir/strong-production/5`，替换为 `/6`，再加入下表前四项；`code_requirement`、`runtime_requirement`、publication、decode-cost model、extra-section policy 和 Link proof policy 原样继承，`odr = RejectAll`。

| capability | location | required_for | sinks |
| --- | --- | --- | --- |
| `org.scoop-lang.hir/cross-cone-type-semantics/2` | HIR | Compile | Hir |
| `org.scoop-lang.mir/cross-cone-type-bridge/1` | MIR | Compile | Mir |
| `org.scoop-lang.lir/cross-cone-layout-abi/2` | LIR | Compile | Lir |
| `org.scoop-lang.lir/cross-cone-layout-link-closure/2` | LIR | Link | Code + LinkValidationOnly |
| `org.scoop-lang.lir/strong-production/6` | LIR | Compile、Link | Lir + Code + RuntimeImage |

`cross-cone-type-semantics/2` 承载 type facts、完整表示与继承接口，复用 `cross-cone-interface/16` 的普通声明、参数和默认值 metadata。HIR type、MIR type bridge、LIR layout/ABI 三条 Compile section 的完整 canonical inner bytes 分别进入对应 layer contribution；Link-only section 仅以 semantic physical-import projection 进入 Code，member/range/patch 信息只作 LinkValidationOnly。strong-production/6 沿用强定义 section 自身的三个 sink。

所有 source Cone、core、single-file 与 cache 产物最终使用新 profile、四条新增 section 及 strong-production/6；空集合显式编码。compiler compatibility 与 cache key 共用新 profile fingerprint，旧 completed dependency 必须重建，不做内存升级。section major 与 outer schema 是不同版本维度；本阶段仍使用 M23 outer schema 1 和既有 runtime ABI。

#### 3.1.1 共有 metadata 与实际使用闭包

普通与 layout 产物中的 canonical C layout 合同同时覆盖实际 native 使用和已物化的本地 CLayout 表示。读取方在共有声明及实际依赖闭包验证后，使用同一物理物化 exact type 清单与共有 nominal 的 CLayout policy 收集表示根，再按实际 provider 的完整字段类型、aligned/packed 和 target 重算合同；native-only witness 闭包仍只由真实 extern/callback 使用决定。额外表示根不从候选 C layout 合同反推，不新增 native callable 或 witness。普通 Compile reader 在每个 provider 的可达依赖均可借用时完成这一重放，缺失、额外或非 canonical 合同仍按完整集合拒绝；无 native 调用的合法 CLayout 可以发布并从最终 bytes 重读。该关联共用原 artifact 预算，不修改 wire、runtime C ABI 或 persistent identity。

MIR 的普通 source function/accessor 集合从同一份共有 HIR 声明独立重建：纳入签名及 owner 均满足无参数物化条件的公开 Scoop ordinary callable、已物化继承接口的 protected 成员、实际 slot root 与选中的源码 implementation，以及必要 logical property 中可访问的 getter/setter；Storage/Constant accessor 不产生函数 binding，Body/AbstractSlot 保留正文或 fatal trap。实际抽象成员保留其自身 typed declaration，不能将 derived abstract override 换回原 slot 声明。继承到的外来声明按 canonical key 的实际 provider 留在依赖侧，不复制成本地 body。producer 与 reader 复用受累计预算约束的共有查询算法；查询只返回已有 declaration 的临时借用索引，不新增 source authority、期望表示表或 wire transcript。reader 从共有声明逐项重放 receiver、参数、结果、execution、GC effect 与 lowering role；普通函数、accessor 和 trap 的 semantic/lowered signature 必须相同，trap slot 必须属于该声明已验证的实际 slot relations。object 成员的逻辑 receiver 使用源码 object exact，不能换成 backing class exact。既有 ordinary bridge 的完整集合独立按实际 provider scope 的原有 nominal classifier 与共有 accessor 实现形式重放，并精确核对 implementation 与 exact signature，不扩张旧签名白名单；某个 nominal 的字段布局仍为 source-only，不会取消它在旧 ordinary bridge 中已经合法的函数签名，也不能因此为它补造新的 type-bridge binding 或 descriptor。两个分区分别保证完备，只对实际重叠的源码 callable 核对并去重；跨分区重复、缺失/额外 source binding、错误 provider/signature/GC/role 均拒绝。constructor、有限 generated helper、dispatch、初始化和实际 selected-use 仍分别完成各自的 source/cross-stage join，单独通过 source callable 校验不构成完整 machine capability。

共有 property 声明完整保留每个 accessor 的源码实现形式。`hir/cross-cone-interface/16` 的 property 声明仍为八字段，field 6 的 accessor 集合改用 tag 4=ReadOnly、tag 5=ReadWrite；field 1 为 getter，ReadWrite 另有 field 2 为 setter。每个 accessor 是恰含 field 1=typed accessor id、field 2=实现形式的二字段 product，实现形式采用 unsigned 1=Storage、2=Constant、3=Body、4=AbstractSlot。旧身份单项形式的 tag 1、3 退役，tag 2 不复用；public lookup 的 capability 及 setter 可见性含义不变。setter 若存在，身份与实现形式必须同时存在；getter/setter 可各自为 Storage 或 Body，不能由 property 的 RuntimeAccessor 总类推断它们都有正文。Const 必须是只读 Constant getter；AbstractSlot property 的 accessor 全部为 AbstractSlot，RuntimeAccessor 不含 Constant 且不能全部为 AbstractSlot；各 accessor 的 AbstractSlot 形式还须与同一共有 callable 声明的 Abstract modality 一致，Storage/Constant 必须没有 dispatch slot。producer 从同一次 Export HIR 的实际 getter/setter 实现投影，MIR callable 生产与 reader 通过该共有声明区分需要正文的 Body/AbstractSlot 与直接读取的 Storage/Constant，不枚举 MIR 候选来补来源。此变更不序列化正文或 LocalConcrete id，不授予私有/受限 accessor 额外访问权，不改变 persistent identity、runtime ABI 或源码行为；旧 section major、profile/fingerprint 与缓存必须重建。

MIR selected 的递归图重放复用已有 type-bridge 闭包算法：在各组成表完成共有声明关联后，按实际 provider 借用完整 types、callables、dispatch、object、shape、初始化合同和旧 callable 分区，从本地完整导出语义边与共有 HIR 实际类型位置查询所得根闭合。该步骤消费原七字段 section 的 initialization-use 与 selected transport，先完成身份及 canonical 集合校验，再将完整计算结果与 selected 精确比较；不从 selected、本地 MIR 类型查找表或物理产物清单补根。依赖图只接受同一显式可达闭包的 provider，拒绝重复 provider、重复 typed target、旧 callable 分区重叠、缺失/额外 selected 及冒领本地目标；索引、节点、边和深度继续使用同一累计预算。只读图视图与重放结果不授予机器消费资格，也不替代 initialization-use 的逐次 HIR 来源、完整 HIR use/access 与后续 LIR/Link 关联；这些条件仍须在最终原子发布前全部成立。该重放不新增 wire、来源 factory 或平行授权记录。

MIR type bridge 的类型表示与有限 shape-support 从同一产物的共有 HIR 声明重放。reader 按所有权先解析原七字段中的 types 和 shape-support，保留其余尚待检查的 callable、dispatch、object、initialization 与 selected transport；不复制或重新解码 types，不把这一中间状态作为完整机器消费资格。各 provider 只借用 manifest 中真实可达依赖的已检查类型组成表，沿原 identity graph 和累计预算完成 HIR facts/representation/inheritance 与 MIR 的关联。源码类型 inventory 来自完整共有 representation 闭包及当前 provider 拥有的 Unit/Any；逐项核对 source/generated origin、GC/ZST、完整字段和 variant 的声明序、exact 字段类型、CLayout、InteriorMutable、class modality、base/interface 与 object backing。有限 helper 的源码根从同一 public surface 和共有 materialization closure 取得，shape-support 与相应 MIR helper 必须精确覆盖；无关或 foreign helper、遗漏源码类型、额外类型和跨表差异均拒绝。此关联不增加来源授权 section、调用方 source factory 或平行声明副本；原 MIR section wire 不变，callable、实际 selected use、LIR 与最终双 view 的完整校验继续作为发布条件。

共有 struct 源码形状完整保留声明的 `@InteriorMutable` 标志。`StructSourceShapeV1` 的既有 tag 3 改为四字段 closed product：field 0=tag、field 1=完整声明序字段、field 2=CLayout policy、field 3=interior_mutable（canonical unsigned 0 或 1）。普通与必要支持声明、嵌套 protected 源码支持均使用这一份形状；标志不由字段大小、GC/ZST、CLayout、可见性或 provider 身份推断，未标注声明显式为 0。producer 从同一次 sealed Export HIR 的 struct attributes 投影，读取方保留该标志供普通类型查询和 MIR 表示关联使用，不能默认补齐缺失字段。缺字段、旧二/三字段格式及非 0/1 标志均拒绝。共有接口升级为 `hir/cross-cone-interface/16`，承载嵌套源码形状的类型接口同步升级为 `hir/cross-cone-type-semantics/2`；required inventory、profile fingerprint、HIR fingerprint 与缓存同步失效并重建。该变更不修改已有语言的内部可变性规则、persistent identity 或 runtime ABI，不增加独立来源授权记录。

共有类型位置同时保留已物化表示和生成语义的必需类型依赖。`hir/cross-cone-interface/16` 在既有 type_sites 和类型中增加 tag 6=FieldStorage、tag 7=EnumVariantFieldStorage、tag 8=ConstructorInitializerResult、tag 9=InitializationCycleMessage；各为三字段 product，field 0 为 tag，field 1 分别为 PersistentFieldId、PersistentEnumVariantFieldId、实际 CallableMaterialization、PersistentInitializationUnitId，field 2 为完整 exact。这些位置直接归属于原有字段、variant payload、constructor 或初始化单元，不另存来源授权记录。LocalConcrete class field 与 struct field 一样直接保留其持久化字段身份，不由字段名或 arena 下标重建。producer 只遍历真实物化类型闭包内的字段与 payload；class initializer 的结果按语言语义为 Unit，实际初始化单元的循环错误消息按其已解析协议使用 String。未物化 enum、未展开默认正文和仅作查找的类型身份不产生记录。读取方沿同一 foundation 检查 typed owner、真实 provider 与 definition origin，生成位置还须核对 constructor/初始化角色及相邻 MIR 的实际依赖。共有 external references 在完成位置、来源与 foreign nominal 分发校验后，可临时查询完整的实际外部类型依赖；该查询不增加 wire 表，也不单独授予机器层消费能力。MIR producer 必须独立从同一次已封存 LocalConcrete 的真实物化闭包重算类型根，并与共有记录查询结果完整相等；缺项或多项都拒绝，不能从 MIR 查找表或候选 selected 反推和补齐。所有记录继续参加完整 foreign nominal 分发、重复位置与累计预算检查；旧 major、未知 tag、错误 owner、错误生成角色和缺失身份均拒绝，profile 与 fingerprint 同步重建。

共有 HIR 外部引用保留同一次已物化 LocalConcrete 的表达式、签名与声明类型使用。`hir/cross-cone-interface/16` 的 ExternalHirReferenceV1 仍为六字段 closed product，必需 field 6 为 type_sites 数组；每项改为以 field 0 为 tag 的封闭和类型。tag 1=Expression，field 1～5 依次为实际 CallableMaterialization 正文根、根内 expression_index、ConcreteExpressionOrigin、表达式类型角色、完整 exact；角色 unsigned 1～5 仍为 Value、SizeOf、AlignOf、TypeTest、ArrayElement。tag 2=CallableSignature，field 1～3 为实际 CallableMaterialization、签名位置、完整 exact；签名位置为 field 0 的 tag 1=Receiver、tag 2=Parameter（field 1 为去除 receiver 后的声明序 index）、tag 3=Result。tag 3=LocalValue，field 1、2 为 PersistentLocalValueId 与完整 exact。tag 4=BackingStorage、tag 5=DelegateStorage，field 1、2 为既有 typed PropertyOwner 与完整 exact。各分支字段必须完整，禁止以可选字段、虚构 expression_index 或 arena 序号拼接声明位置；旧五字段 type-site item、旧 major、未知 tag 及额外字段均拒绝，profile、HIR fingerprint 与缓存同步重建。

producer 从实际函数和构造器的 receiver、声明序参数、结果，正文内 source-backed local value 及实际 global storage 投影声明分支，保留 private 签名、未读取的参数/局部变量、全局声明和初始化相关 generated callable 的类型。局部捕获沿既有 PersistentLocalValueId 指回同一声明，重复观察只能在 exact 完全一致时合并；synthetic temporary 由其实际表达式/生成签名承载，不伪造源码声明。仅作前端操作声明的 Intrinsic 函数不产生签名根，其实际操作由表达式记录承载。GenericSourceMetadata 的未展开默认正文不加入该集合。数组按 variant、完整 typed 位置和表达式角色严格排序，组内重复位置及冲突 exact 拒绝；外部引用 role 8=ExecutableTypeDependency 当且仅当 nominal target 拥有非空 type_sites 时出现，包含已物化实体的签名与存储依赖。

每个位置沿 exact key 的结构子类型保留全部实际 foreign nominal；跨 nominal 的同一位置必须保存完全相同的记录。reader 使用同一 identity graph 与真实可达依赖重算这一分发集合，验证完整有序集合、target/provider、表达式 definition/evaluation 来源，以及声明位置在当前 foundation 的 callable、local-value 或 property 身份及源码归属；NoSubstitution 以外的物化继续由既有 ODR capability gate 拒绝。声明分支沿既有 canonical key 与 definition origin 查询，不重复保存一套来源证明。委托属性的生成 accessor 在进入其真实 callable 源码上下文后生成合成 thisRef、storage read 与协议调用；正文构造结束或诊断退出均恢复外层上下文，不能让合成操作沿用外层 File 求值来源，也不放宽 reader 的正文归属检查。source-point 完备步骤继续从同一 interface 的 call_sites 和 Expression 分支收集 definition/evaluation span 端点，与已有声明/default 来源点合并；真实 UTF-8 源码生成本地映射，依赖来源只使用已有 canonical source record。位置、canonical key、fanout、哈希、排序、扫描与临时分配使用累计预算。类型记录本身不授予名字查找、访问、调用、TD 或机器资格；binding witness、call_sites、实际 type/member/slot/use 与后续 MIR/LIR selected join 仍分别验证，不从 MIR、候选 selected、layout 或 primitive arena 反推根，也不新增调用方 source factory 或平行来源表。

共有HIR外部引用在 `hir/cross-cone-interface/16` 中直接保存实际调用位置：ExternalHirReferenceV1 的原 field 1～4 保留 origin、typed target、roles 和 canonical binding witnesses，新增必需 field 5 的 call_sites 数组；不增加来源授权 section 或平行 transcript。每个 call site 是六字段 closed product，依次为实际 CallableMaterialization 正文根、该根内的 expression_index、既有 ConcreteExpressionOrigin、完整逻辑实参 exact type 序列、result exact type 和本记录 witness 表中的非空 canonical u32 index 集合。extension receiver 保留为第一个逻辑实参；重复参数、Unit 和 ZST 均不得省略。数组按正文根与 expression_index 严格排序，同一位置不能重复声明调用；target 与 terminal provider 只从所属外部引用取得，逐次 route 用索引关联共有 witness，不复制授权事实。普通 Function/Accessor 的 ConcreteSelectedUse 必须恰有非空 call_sites，其他用途或 target 不得冒领调用位置；const/type-alias 的既有源码协议保持。producer 直接遍历 sealed LocalConcrete 的实际节点并保留每次来源，未展开默认值和未物化正文不产生记录。reader 从同一 foundation 与实际依赖验证 definition/evaluation 的来源、正文与 source context 归属、每次 route、逻辑签名和实际 MIR Strong callable 根；多个位置使用同一目标时仍逐项验证。已有 HIR/MIR/LIR selected 与 Link 覆盖继续精确关联。解码、身份解析、来源、索引、排序、遍历和分配使用累计预算，缺字段、旧四字段格式、缺失或额外调用、乱序、重复位置、错 provider/root/context、越界 witness、错误签名和 ODR 根均拒绝。旧 `/10` artifact 与缓存重建；outer schema、persistent identity 和 runtime ABI 不变。

本节替代旧的独立 HIR source-authority transport 草案。`cross-cone-type-source-authority/1` 不进入 required inventory，不实现其四域封装或额外授权链，也不把它改名或嵌入其他 section 继续作为第二份来源证明。此前为该草案定义的独立 transcript 不构成普通类型使用或发布的前置条件；必要信息合并到实际拥有相应声明、类型、参数、默认正文或 selected use 的共有 metadata。现有 constituent 可以复用其完整 typed 数据与校验算法，不能保留相同事实的独立授权副本。

producer 从同一次 sealed Export/LocalConcrete HIR 及真实 winner-commit 结果生成这些 metadata；普通查询和 reader 使用同一数据模型。bytes-only reader 不需要源码、编译器 token、调用方另行提供的来源 factory，或一份与发布表平行的自证 transcript。reader 验证 provider 所声明的完整语义与机器产物的一致性，不认证源码目录或要求 provider 证明其声明未被用户修改。

layout profile 的共有 HIR 声明读取直接消费同一 artifact 的 identity、foundation、production 与 public/source metadata。它与既有 ordinary reader 共用 definition source、nominal、property、callable、alias、参数/default、const、public route 和 external reference 校验；每个 provider 只使用其显式依赖闭包中已验证的声明，继续消费 artifact 的累计预算。完成这些共有检查只证明声明与引用闭合，新 type semantics、MIR/LIR 和最终 object 的关联仍须分别完成，不能将解码成功或生产侧对象代替这些检查。

共有校验必须保留以下信息与约束：

- identity、definition origin 和 source context 只使用同一 artifact 与依赖闭包中的完整 typed 记录。local declaration 的 key origin、词法 owner、definition subject 和 provider 一致；external 引用按实际 provider 唯一解析。SourceNominal、ExactType、Function、GenericFunction、Constructor、Property、Accessor、DispatchSlot 与不同生成实体不得混用；不得从 FQN、符号、同布局或“唯一匹配项”补身份。
- 普通 nominal metadata 保存真实 kind、modality、own binder、bounds、supertypes、完整字段/variant 及必要 member/constructor/child 关系。private backing field、object backing class、setter 与默认正文支持声明不能因不参与 public lookup 而丢失必要类型或语义。source-only generic 声明保留完整源码合同，不伪造 exact、representation、inheritance 或机器定义。
- GC/ZST facts 由完整表示结构重算并与 fact 表一致；source value shape、MIR 表示无关 shape、LIR field/layout/scan 逐阶段按 typed owner 和 exact type 关联。字段与 enum variant 保持声明顺序；普通 canonical 集合按 typed key 排序且拒绝重复。布局不能反推源码 nominal kind、generic 参数或访问权限。
- constructor、callable、property 与 accessor 共享真实声明的参数名、参数顺序、值类型、结果、effects、modality 和访问域。getter/setter 的 logical property 与 role 由 typed key 决定；参数/default/vararg 协议与同一声明一致，零参数声明也完整。protected nested support 沿真实词法关系保留所需子树及完整源码合同，不能把 owner 受限的 declared-public 成员改标为 protected。
- inheritance、virtual family、interface conformance、slot schema 与实际 implementation 选择来自同一次已解析的 HIR 关系。完整接口声明序、typed override 抑制、Function/Getter/Setter 角色、abstract/concrete/default 状态、effects、参数/结果及访问域仍按第 4、5、8 节重放；不能按名称重新选择实现，也不能以另一项可适用实现代替实际选择。
- 默认值只保存一份完整 typed 正文、参数协议、definition origin、binder、六类引用、receiver occurrence、nested callable/capture 和访问域。继续复用普通默认值的正文、类型、data-flow、effect、引用闭包与调用域检查；继承默认值分别保留原 provider 与发布声明的域。不得为了 reader 校验另建一套平行 source-default proof。
- 实际 HIR use 保留 winner-commit 产生的稳定 occurrence、typed target、lookup/access/receiver 上下文及依赖边；去重后的 selected 集合与这些真实 use 闭合。MIR/LIR 的 selected、semantic import 和 physical use 按实际消费继续关联验证。声明存在、进入候选集、provider 可达或 source-only metadata 可见，都不能代替实际使用，也不能授予机器执行能力。
- 相邻表、所属 provider、public/inheritance 可见面、完整需要集合和跨阶段引用保持互斥与覆盖校验。缺失、额外、错 owner/provider、错误角色、访问域不合法、错误签名、布局/scan 不一致和不支持的物化均拒绝。canonical 解码、identity 注册、图遍历、排序、嵌套正文与结果分配共用累计预算；失败不交付部分可查询的 checked closure。

当前 Cone 的形状物化需求由共有公共声明和实际本地使用产生：按 export binding 的实际 exporter 选择 DeclaredCurrent 声明，再应用第 1.2 节的完整物化闭包 gate。HIR 保存必需的 LocalShapeSupportPlan；MIR 接收同一 typed source 集合并核对实际 source/exact 与 helper；LIR 核对实际 producer 的完整 layout、scan、descriptor 和 registration。alias、reexport 和仅作源码 metadata 的声明不自动增加本地机器根。所有 Cone 共用同一计划和 reader 校验，compiler protocol 的 Defined/Imported 角色不决定形状需求。

layout profile 的 HIR 类型基础读取从共有 nominal/callable 声明重算当前可物化集合，并从完整字段、variant、继承和公开/受保护构造参数闭合必需 exact facts；Unit/Any 由其既有定义 owner 保留。struct/enum/tuple 的 GC/ZST 事实继续使用共有递归算法，引用值作为 managed leaf，不以对象字段判断值的 GC-free 或大小。external nominal 只查询实际 provider 已验证的事实，缺失、额外、本地冒领外来事实、错误 intrinsic family 和 by-value cycle 均拒绝。representation 的 owner、来源位置、词法 owner、visibility、完整字段/variant、CLayout policy、base 和 object backing 与同一共有声明及事实关联，预算贯穿身份、集合、递归和比较；这些检查不替代 dispatch、protected/default、selected use 或机器产物闭合。

### 3.2 不改义的既有 section

- identity-foundation 的布局/scan/dispatch key 继续只证明 identity；本阶段新 payload 引用它们，不重复声明同 kind/id。
- `NativeBoundaryTypeDefinitionRecordV1` 只服务原 extern/callback witness，不通过它提供一般 field/scan/TD 查询。
- core与其他Cone共用通用MIR/LIR shape-support表。每个source root均由当前provider的typed source声明及完整类型、layout、descriptor记录验证，完整section核对独立source-root集合的精确覆盖。不得因provider为core而要求空表、跳过验证或委托旧core表；MIR查询与依赖选择直接读取同一通用表。LIR production字段8直接保存所有producer共有的有限shape-support计划数组，删除Core/NotCore包装；source、closure root与definition owner必须属于实际producer，reader从独立source集合完整重算，不能为通用section补齐缺失记录或提供第二份root来源。MIR CoreMirBridge的重复shape_support_roots及wire字段2已删除；本地物化直接消费HIR提交的完整source声明并验证MIR实体，reader从共有公共声明推导LIR义务。
- M23-5 最大 core-closed callable export 集不变，ordinary dependency 的该子集仍走旧 MIR/LIR bridge 和旧 Link 分区。
- 新 callable bridge 只承载上述旧集合以外、现在可证明的 target；MIR与LIR的所有外部callable在各自stage共用一个实体、typed id和arena，MIR输出与sealer复用引用及重复implementation校验，LIR同一body不得重复；旧、新metadata分区从实体的明确选择角色投影。完整类型证明可以被两类 bridge 共用。callable分区优先检查保留的core初始化协议bridge，其次检查M23-5 ordinary bridge，剩余target才进入新bridge；shape的layout、scan、TD与registration不因出现在production形状计划中而被整组排除，按共有shape-link校验实际provider与definition；新开放的 core member/constructor/shape use 若不属于旧 bridge 的固定集合，也走新 bridge，不能借此扩大旧集合。
- 所有外部TD在LIR中统一为`ExternalTypeDescriptor`及一个arena，必需保留实际provider、exact、symbol和definition；Local/External引用只表达本地定义与外部引用。String角色由well-known明确引用保存，既有协议wire仅投影该角色；其他外部TD（包括core普通类型）必须通过通用layout selection，V1缺少对应layout selection时明确拒绝，V2保留实际provider并重放完整semantic/physical selection。codegen统一发射和校验外部描述符，foundation与layout投影共用实体，不维护第二套core TD。通用ExactDispatch从callable ABI实际provider推导Local/DependencyExternal引用，core普通dispatch不降为旧协议CoreExternal。
- `strong-production/5`与`strong-production/6`共有的字段8改为直接shape-support计划数组，旧Core/NotCore tagged sum拒绝读取，已有产物必须重建。新profile只生产/要求V2；两版保持top-level十字段及identity、definition plan、digest DAG、协议bridge、image plan结构，V2另将TD/dispatch语义中无法表示ordinary provider的引用sum版本化；runtime registration/image的C ABI不改变。

本阶段不提升 container/outer schema，不改 `persistent-v1` mangler，不重分配既有 tag。新增 mandatory section 使旧 reader fail closed。

`strong-production/6`中的三个版本化constituent固定为：

```text
StrongTypeDescriptorRefV2 =
    Local(PersistentExactTypeId)                      // tag 1
  | CoreExternal(PersistentExactTypeId)               // tag 2
  | DependencyExternal { provider, exact }           // tag 3

StrongTypeDispatchCallableRefV2 =
    Local(PersistentCallableBodyId)                  // tag 1
  | CoreExternal(PersistentCallableBodyId)            // tag 2
  | Runtime(RuntimeFunction)                         // tag 3
  | DependencyExternal { provider, body }            // tag 4

OptionalStrongTypeDescriptorRefV2 =
    Absent                                           // tag 1
  | Local(PersistentExactTypeId)                      // tag 2
  | CoreExternal(PersistentExactTypeId)               // tag 3
  | DependencyExternal { provider, exact }           // tag 4
```

前述既有variant的payload逐byte复用V1实际编码，包括optional Absent的`{0:1,1:0}`；新增variant是`{0:tag,1:provider,2:exact_or_body}`。body是`PersistentCallableBodyId`，必须反向join同provider已验证的Strong owner、ABI export与definition，不能只比较symbol。TD parent、itable interface key、vtable/itable entry及引用它们的type-registration semantic plan统一使用V2，不保留另一个可矛盾的V1 plan。普通形状的 DependencyExternal 必须由共有 layout/ABI selection 和 Link import 逐项证明；String 与初始化角色使用同一 provider/typed target 引用，并按实际已选 descriptor/callable 与 definition 校验，重复来源不得互相补齐缺失证明；不允许把真实parent写成Absent再由sidecar补齐。core原ref仍按旧分区使用CoreExternal，新增core能力按3.2的分区规则使用新ref。descriptor/dispatch dependency fingerprint使用同一完整V2 relation；旧variant的canonical bytes保持，新variant追加tag，不能遗漏provider或把foreign target编码成local。

其他strong-production constituent以及runtime registration的typed key/record不变。Compile/Link handler先按capability解码V1或V2，再取得不可降格的validated production view；旧Link identity section只消费其既有子集与同一member全集，新physical use由11.3覆盖。profile拒绝同时出现两个strong-production版本，避免两套definition authority。

V2另扩展initialization dependency的验证域：wire仍是原顺序/排序契约下的`PersistentInitializationUnitId`序列，不改变id或record字段；内存证明改为`LocalUnit(ref) | DependencyExternalUnit { provider, unit_ref }`。旧V1只能解析本Cone producer unit的规则保留。V2的foreign id必须命中同一显式dependency closure中已选中的provider unit、对应initialization descriptor与required definition，producer unit表仍只含本地定义，不能复制foreign canonical unit record冒充本地。missing/错provider/跨closure dependency在artifact commit前拒绝；不允许省略真实edge来让旧validator通过。

### 3.3 编码约定

本文新 record 使用 M23-2 Wire CBOR v1 closed product：field 从 1 开始按文中声明顺序编号，sum 使用 field 0 的 tag，payload field 从 1 开始；新增 sum 的 tag 按列出顺序从 1 递增。所有引用既有类型的地方复用原编码，不重新给 `Effect`、`GcEffect`、exact key、symbol request、definition plan、scan fingerprint 或 canonical ABI 编号。

table 按 kind-specific typed 主键的 canonical bytes 严格递增；set 排序去重，重复输入拒绝而非 reader 自动修复。参数、字段、variant、base prefix、slot position 是有序语义序列，不按名称重排。各 constituent 使用 foundation 的 text/bytes/count 限额和 deterministic logical budget；checked arithmetic、depth/cycle 与展开成本检查在分配前完成。

文中 `Checked`、`Selected`、`Ref`、`Witness` 表示构造完成后的内存类型，不把 arena index 或“已验证”布尔值写入 wire。wire 只保存重建这些证明所需的 typed id 与 canonical facts；reader 重建后才返回 handle。

layout profile 的继承图与 nominal 访问域从同一共有声明重放：所有源码 nominal（含 source-only 泛型）保留词法 owner 与来源；只有已闭合表示的 nominal 进入 exact 继承图。reader 逐项核对继承表的完整 owner 集合、modality、直接 base 和 interface，再复用共有算法拒绝缺失父节点、final base、错误 kind 与继承环，并核对声明 lookup/inheritance 域和 object/backing 关系。依赖按实际 provider 及其已登记身份解析，临时图与来源记录计入当前累计预算；图仅在读取作用域内借用，不增加 wire、来源凭证或发布资格，也不替代 slot、protected/default 和 selected use 的后续闭合。

共有 inheritance 声明校验从 nominal 的完整 constructor/member/child 关系及 callable/property 的真实 visibility 确定必需集合。Public/Protected constructor 按 typed id 与同一共有 callable 的 owner、参数名和顺序、binder、result、effects、modality、slot 及 definition origin 逐项关联；Protected constructor 同时闭合 protected declaration 中的同 id 记录。protected member 集合包含真实 Protected function/property/accessor/nested nominal，setter 按自身 visibility 判定，不把 declared-public 的受限 owner 成员改标为 Protected。缺失、额外、错 owner、签名或访问来源不一致均拒绝；所有查询、比较与集合分配继续消费原 artifact 的累计预算。这些关联检查与继承图重放一同在 layout reader 中执行，仍不代替完整 slot/default/selected 与机器产物验证。

layout reader 的受限源码参数协议复用完整声明集合收集规则：从已与共有声明关联的 protected callable/constructor、递归 nested support 与 inheritance constructor 取得全部非 accessor owner，按 typed id 去重，并要求协议表与该集合精确相等。每条协议必须同当前 provider 的共有 source interface 逐项相等，包含参数顺序、名称、完整 signature type、Required/Default/VarargEmpty/VarargDefault、vararg element type、默认模板 owner/position 及逐参数 definition origin；不按公开查找资格省略必要支持协议。默认 key 集合继续同协议中的实际引用精确闭合，不能把默认参数改为 Required 或用零参数记录补缺。收集、排序、比较、递归和查询使用同一累计预算；该步骤只关联已有 metadata，不新增源码证明表，也不替代默认正文、访问域与实际 selected use 的验证。

layout reader 的 inheritance slot 合同逐项关联共有声明：root 与 target 的 typed identity、实际 owner、完整 signature/effect、modality 和 access source 继续复用既有 slot identity、receiver、domain、schema 与 abstract-obligation 验证。实际 implementation 必须与同一 owner 的共有 dispatch selection 精确相等，完整槽集合与已验证的 schema 一致；不得根据名称、可适用签名或另一项 default 重新选择，也不能从正在检查的 implementation 字段反推。getter 与 setter 分别核对，final override、reabstract 与 inherited default 均保留同一次 resolved HIR 的实际结果。源码层 generic 声明保存完整源码选择，M23-6 concrete slot 只消费已物化的 param-free owner。身份解析、索引、签名投影、图遍历与比较计入同一累计预算；完整 selected-use 与 MIR/LIR 证明仍须继续闭合。

layout reader 的受限默认模板逐项关联同一 provider 已验证的共有 default metadata：owner/position、definition root/path/origin、局部 binding、完整正文、result、suspend、binder、receiver 与前序 value parameter 必须一致。六类引用的 typed target 与 definition origin 精确闭合；expression-use 集合从同一正文重新收集，拒绝缺失、多余、重复或错误 occurrence。访问 witness 的 direct/target 域来自共有 reference，完整 root slot 集合来自共有 callable 的 slot relations，逐槽 domain 从已验证声明及继承图重放；generic source profile 依据真实 nominal owner 与 source access constraints 判定，不能因函数自身 generic、当前 Cone 为 core 或候选 witness 自报而改变。默认正文、数据流、receiver 和访问合法性继续由共有声明 reader 验证，受限投影不建立第二份来源授权链。definition-source 集合与类型 section 的实际引用闭合，并关联同一共有 origin。所有重放与比较使用原 artifact 的累计预算，不增加 wire 字段，也不替代实际 selected use 和机器产物验证。

layout reader 的 protected 声明全集从共有 nominal、callable、property 的实际 declared visibility 与 owner 关系产生，包含 source-only generic owner；不能只遍历已物化的 inheritance exact 节点。完整表须与该集合精确相等。每个 property 的逻辑类型、getter/setter、representation、slot 集合及实际 setter 来源与共有声明逐项关联；nested source interface 的 kind、modality、binder、supertypes、constructor/member/child 集合与完整 source shape 同样逐项关联，并递归验证 support 子树中的 callable、constructor、property、const 与 enum variant。完整 source identity 和 definition origin 沿同一 provider 的共有 foundation 解析，所有递归、比较、查询和分配共用预算。param-free nested 继续闭合真实 inheritance/representation，generic source 保留完整合同且不产生 exact 节点；参数/default、访问域和实际 selected use 仍须经过各自的完整检查。静态 nested nominal 即使自身无 binder，若完整词法 owner 链含 generic owner，其构造器仍只保留完整 source/default 合同，不进入要求 param-free access source 的 inheritance constructor 集合；其独立的表示与布局不因此缺失。producer 与 reader 使用同一声明关系判断该边界，不能由构造参数恰好不含 binder 授予泛型访问能力。nested const 直接使用 source support 内已有的完整 const 值，并同共有 property 的类型、representation、真实 object owner 和 foundation 来源关联；字面值种类按实际类型 provider 的 intrinsic family 验证。只在该 property 同时位于普通 public 表时要求与 public const 值一致；非 public support 不伪造 public lookup，也不为常量值另建来源副本。该关联不新增 wire 字段或第二份来源授权表。

共有 nominal 的实际派发选择纳入 `hir/cross-cone-interface/16`：`NominalDeclarationDetailsV1` 在既有 field 1～6 后增加必需 field 7，保存按 typed slot id 严格排序、无重复的完整 `dispatch_selections` 数组；每项为 field 1=PersistentDispatchSlotId、field 2=实际选择的两字段 product。选择复用既有源码选择格式：tag 1=Abstract 无 payload，tag 2=Concrete 与 tag 3=InterfaceDefault 在 field 1 保存完整 typed Function/Getter/Setter declaration。owner 由 enclosing source nominal 唯一确定，不复制 exact owner、来源 proof 或授权 transcript。producer 从同一次 sealed HIR 的 virtual family、FinalOverride、interface member override 与已解析 MethodApplication 投影，保留实际实现；generic owner 的普通 method 仍保存 Function/Accessor identity，source-only 选择不要求 exact 或机器物化。layout reader 将可物化 owner 的完整槽集合及每项选择与 inheritance contract 精确关联，并继续按共有 callable、继承图与访问合同核对其合法性。外来实现目标进入共有 external HIR reference 的 InheritanceDependency（新 tag 7），按实际 provider 与 typed declaration 验证依赖闭包，不伪造 source-name binding witness；槽 identity 继续通过真实声明、dispatch order、继承与 schema 图关联。缺失 field 7、旧六字段记录、重复或乱序 slot、错误角色、缺失/额外选择以及以另一合法实现替换已选目标均拒绝。空集合显式编码；producer、decoder、resolver 和所有图/集合校验共用累计预算。旧 `/9` 产物及缓存重建，profile、inventory 与 HIR fingerprint 同步更新；outer schema、persistent identity 和 runtime ABI 不变。

共有 nominal 声明的 dispatch 顺序属于声明 metadata：`hir/cross-cone-interface/16` 在 `NominalDeclarationDetailsV1` 的原 field 1～5 后增加必需 field 6，保存 `NominalDispatchOrderV1`。tag 1 的 NonVirtual 无 payload，仅用于 struct/enum；tag 2 的 Class 在 field 1 保存按当前 owner 的方法声明序排列、无重复的 virtual-family slot id，包含该 owner 的 override 引用；class/object 均使用此分支。tag 3 的 Interface 在 field 1 保存有序、无重复的 direct-parent source signature，field 2 保存有序、无重复的 `{ slot, overrides }` 成员，后者复用既有 InterfaceSourceMemberV1 的两字段结构，overrides 仍为 canonical typed slot 集合。generic source 声明保留相同的完整 source 顺序，不为此建立 exact node。parent 与共有 supertypes、slot 与共有 callable 的同 owner 关系须完整相等，不能通过排序填补缺失顺序。class vtable 从已验证 base prefix 开始，按声明序追加尚未继承的 family；interface schema 继续按 direct-parent 声明序作后序遍历，按 typed override 关系抑制被覆盖成员。所有生成、解码、索引、集合比较和图遍历计入原预算，构造器、protected、default 与实际实现选择继续使用原共有声明及闭包，不另建来源授权表。旧 `/8` 产物与缓存重建；outer schema 和既有 identity 不变，Compile capability inventory 与完整 HIR contribution 同步变化。

## 4. HIR：type facts 与继承接口

### 4.1 type semantics section

```text
CrossConeTypeSemanticsSectionV1 {
    exact_facts: CanonicalVec<ExactTypeFactsV1>,
    representation_support: CanonicalVec<NominalRepresentationSupportV1>,
    inheritance: CanonicalVec<NominalInheritanceInterfaceV1>,
    protected_declarations: CanonicalVec<ProtectedDeclarationInterfaceV1>,
    protected_source_interfaces: CanonicalVec<ProtectedSourceInterfaceV1>,
    protected_defaults: CanonicalVec<ProtectedDefaultTemplateV1>,
    definition_sources: CanonicalVec<ExportDefinitionSourceV1>,
    selected: CanonicalVec<SelectedExternalTypeUseV1>,
}

ExactTypeFactsV1 {
    exact: PersistentExactTypeId,
    kind: Value { zst: ZeroSized | NonZero } | Reference,
    gc: GcFree | ContainsManagedReferences,
}
```

field7 `definition_sources`精确等于fields2～6全部inline definition origin的去重并集，包括representation/access、inheritance constructor与slot root/implementation、递归nested source support（含setter及const）、source parameter、default root/local/body/reference来源。reader穷尽typed constituent，不从候选field7反推使用集合；缺失与额外来源均拒绝。继承合同中的foreign origin保留真实provider，并由type-section独立authority逐项核对其已验证foundation/provider及实际使用关系；不能套用旧public section整表`current_cone`约束，也不能把foreign origin改写为当前Cone。field1的exact facts和field8的typed selected关系没有inline origin；selected的源码provenance另由其独立use authority重放。该闭包本身只证明来源完整性，不授予source lookup或selected资格。

fields1～3的顶层inventory由本provider的独立checked source/support全集决定；foreign nominal的facts、representation和inheritance完整顶层record只从其terminal provider的checked section读取，不能复制到consumer表。本地inheritance record可保存真实foreign slot root/implementation合同，graph借用terminal的foreign edges；struct等递归GC/ZST事实也从terminal checked facts取得。Tuple等没有唯一source owner的structural exact事实可由本Cone必要support inventory保存，ownership不能按ExactTypeKey形状猜Cone；M23-7的独立Strong物化gate不阻断本阶段对本地transient/组合语义所需的结构事实重放。完整section同时对照本地inventories、显式source_roots与跨provider递归selected闭包。

source_roots本身只证明source identity和词法闭包，Concrete与GenericTemplate均可作为source-only根；只有被独立required_representation_owners或local_inheritance_edges要求的Concrete根必须同时闭合facts、representation与inheritance。source-only根不取得concrete target token，selected不能只凭其源码存在性通过。

完整 type-section 与同 provider 的 public 十表共同经过共有 semantic validator；checked public view 不能由 raw DTO 或单表单独制造。公开 final 成员的 declaration/provider/receiver 必须关联实际 public 合同，protected/inheritance 合同由完整类型 metadata 闭合，重叠处逐项一致。local inventory、source roots、fact ownership、表示与继承关系使用 3.1.1 规定的共有声明和 identity 数据，按完整集合与 graph 关系校验；producer 从 sealed HIR 构造这些记录，reader 从同一 artifact 和依赖闭包取得它们，不需要独立 source-authority section 或编译期内存 token。

这些事实覆盖本 Cone 导出的 param-free exact subject，以及其必须供下游检查的表示/继承 support；generic template 不能伪装成 concrete facts。`Reference` 的 `gc` 固定为 ContainsManagedReferences，描述的是该引用值；对象内部 `object_scan` 可以为空，二者不能混淆。`Value/ZeroSized` 必须是 GcFree。enum 每个 variant 的 gc flag另随 representation record保存，并重放 `gc_free(enum) = AND(gc_free(variant))`，不是对ContainsManagedReferences取AND。

value `ZstStatus` 在 HIR 完成：只有 Unit 或全 ZST 的普通非 CLayout struct/tuple 为 ZeroSized，intrinsic scalar/pointer 与 enum 不从空字段推断。MIR 机械转写；LIR 验证 status 与 layout 一致，不能用 `size == 0` 为缺失 HIR 信息补值。

`NominalRepresentationSupportV1` 的主键是 param-free source nominal 的 `PersistentTypeId`，保存 kind-specific 源码表示：ordinary struct 的声明序 `{ field, exact/signature type }` 和 CLayout policy，enum 的声明序 variant/payload 与 variant gc，class 的已解析 base 及声明序 backing/delegate field，object 的 backing-class 关系，intrinsic 的 typed representation family。本阶段不导出generic template的representation support，尤其不能为依赖binder的enum variant填入无条件concrete gc flag。字段仍可用既有`SignatureTypeKey`引用fully concrete的generic application；这些引用不能包含binder，选中请求闭包若需要ODR生产仍由M23-7 gate拒绝。generic template的表示实例化与对应物化证明留M23-7，不能增加Unknown/default flag。

其wire固定为 `{ 1: owner, 2: declaration_access, 3: shape }`。field type只保存一个`SignatureTypeKey`，不平行保存可矛盾的exact/signature字段；concrete消费通过已验证binder-free转换取得exact id。shape的tag1～6依次为：`Struct { fields, c_layout_policy }`、`Enum { variants }`、`Class { base, declared_fields }`、`Interface`、`Object { backing_class, declared_fields }`、`Intrinsic { representation }`。field是`{ field_id, value_type }`的declaration-order product，enum variant是`{ variant_id, fields, gc }`；source struct field与class backing/delegate field沿用既有`PersistentFieldId`，但必须经不同的checked wrapper隔离，并逐项验证foundation `FieldIdentityKey`中的`Declared`与`PropertyBacking`/`PropertyDelegate`角色；enum-variant-field使用独立的`PersistentEnumVariantFieldId`，三种字段用途不能混用。这里不新增persistent id家族或改变既有identity wire。CLayout policy与intrinsic representation复用对应IR的封闭语义模型并显式wire映射，不以annotation文本、短名或bool替代。

普通 public struct/enum 的 public representation 必须逐项等于 M23-5 source shape。class 的 private/internal backing field 仅供布局重放，不加入 `members`、import、default binding 或 source field lookup。读取 shape-support 的 authority 与源代码访问 field 的 authority 是两种不同 handle。

CLayout policy使用独立sum：tag1为`Ordinary`，tag2为`CLayout { contract }`，field1的contract复用HIR语义product`{ aligned, packed }`（field1、2）；各`HirCLayoutValue`使用无payload的sum，tag1～6依次为`Natural/A1/A2/A4/A8/A16`。intrinsic source representation family使用tag1～7的`Integer { signedness, width }/Boolean/String/Array/MutableArray/Ptr/FunPtr`；Integer的field1、2分别保存HIR signedness和width，二者均为无payload的sum，tag分别为`Signed=1/Unsigned=2`和`W8=1/W16=2/W32=3/W64=4`。family记录source nominal的封闭表示类别，实际generic application的element/pointee/function type仍由既有exact/signature key提供；不能从family省略应用参数或取得ODR生产能力。这些是新section constituent的显式编码，不修改native-boundary的CLayout/ABI编码。

Class shape的`base`复用既有`OptionalSignatureType`编码，`Absent`表示无class base，`Present`只接受binder-free的nominal或nominal application signature；其exact class关系在inheritance closure中重放。Object shape的`backing_class`是`PersistentTypeId`，必须逐项等于`GeneratedNominalKey::ObjectBackingClass { object: owner }`派生的identity，不能借此把object的source identity当作backing class identity。

### 4.2 inheritance surface

```text
NominalInheritanceInterfaceV1 {
    owner: PersistentExactTypeId,
    modality: Final | Open | Abstract | Interface,
    direct_base: NoClassBase | ClassBase { exact },
    direct_interfaces: CanonicalVec<PersistentExactTypeId>,
    domains: NominalAccessDomainsV1,
    constructors: CanonicalVec<InheritanceConstructorInterfaceV1>,
    slots: CanonicalVec<InheritanceSlotContractV1>,
    protected_members: CanonicalVec<ProtectedDeclarationRefV1>,
    slot_schemas: CanonicalVec<InheritanceSlotSchemaV1>,
}
```

该表是普通 public lookup surface之外的第二条接口。protected declaration record 携带与同类 public record 相同的完整 owner、source signature、source parameter/default interface、effect、modality 和 property/accessor 关系，但使用独立的 inheritance access proof，不能 cast 成 `PublicLookupAccessV1`。

`InheritanceConstructorInterfaceV1`使用三字段product：field1为`PersistentConstructorId`，field2为真实`DeclarationAccessSourceV1`，field3为完整`NominalSourceCallablePayloadV1`。它只保存当前param-free Class/Struct上声明为Public或Protected的构造入口，owner无binder、constructor own binder及slot关系为空；enum construction仍使用typed variant plan。Private/Internal constructor供provider本地初始化或委托使用时，由其body及Strong support闭包保存，不进入该foreign source构造surface。该product复用source signature叶子，不取得普通public lookup；Protected constructor须与同id的protected declaration逐项join。ctor参数/default协议与真实源参数表闭合，重叠的旧public元数据也须同值，不生成第二个声明identity。

继承表按owner exact严格递增，constructor序列按constructor id递增；完整record继续平铺field1～4的edges及field5～9的domains、constructors、slots、protected_members、slot_schemas。独立定义侧inventory决定所需owner、constructor及protected member集合，并提供真实callable signature/access/modality和已解析的slot实现选择；不得用正在读取的表反推这些全集或实现选择。slot合同须恰好覆盖该owner所有schema的slot并集，root和target各自与完整source callable合同join，再核对override实现选择。protected member引用与完整protected声明表的owner逐项相等，constructor由独立constructors字段承载。这些source/inheritance表的checked结果仍需在完整section中同default coverage、表示、selected及provider来源闭合后才能授予生产资格。

`NominalAccessDomainsV1` 分别保存 effective lookup、inheritance 和 slot contract 所需的域。persistent domain 是 `Empty | Conjunction(constraints)`，空 conjunction 表示 universal；constraint 仅为 `Cone(ConeIdentity)`、`File(SourceIdentity)`、`LexicalOwner(kind-specific nominal id)`、`SubclassesOf(exact class)`。约束按 tag/id 严格排序，owner intersection、不可居住性与包含关系由 reader 重放，不保存 session ClassId 或 visibility 数字大小。

每个protected/support声明都保存 `DeclarationAccessSourceV1 { declared_visibility, lexical_owners, definition_origin }`：visibility是独立closed enum `Public=1 | Internal=2 | Private=3 | Protected=4`，owner链按outer→inner顺序保存typed nominal id，origin复用Stage5 `ExportDefinitionSourceV1`。reader从foundation的真实owner chain/source与这份declared visibility重算域，不能只比较producer给出的两个计算结果。`NominalAccessDomainsV1`的三个字段依次为lookup、inheritance、slot；用途不同的域不能互转。

语言9.1.5的receiver限制只属于目标method/property/accessor自身声明的protected权限；其effective lookup中的外层nominal protected约束仍只用于词法域重放。不能把nested类型的普通成员receiver代入外层class的SubclassesOf条件。getter与setter分别按各自access source判断是否需要protected receiver证明。

nominal的lookup由声明visibility与全部owner域重放；class的inheritance沿既有本地语义：Final为Empty，Open/Abstract为lookup与`SubclassesOf(self)`的交集；interface的inheritance为其lookup，struct/enum/object等不可继承nominal为Empty。nominal本身不是callable slot，因此该record的slot字段固定Empty，reader拒绝非Empty；它不保存member域的并集，也不覆盖任何member合同。每条`InheritanceSlotContractV1`仍保存自己的完整root-slot域，通过原始声明source独立重放。

`ProtectedDeclarationInterfaceV1`的tag1～4依次为`Callable`、`Constructor`、`Property`、`NestedNominal`，每个variant field1为对应kind-specific声明id、field2为`DeclarationAccessSourceV1`、field3为完整接口payload。Callable/Constructor payload按顺序保存owner、own binder list、receiver、parameter shapes、result、effects、modality、source interface、slot relations；constructor own binder固定为空，signature scope压缩规则复用Stage5。Property保存owner、value type、getter、`ReadOnly | ReadWrite { setter, setter_access }`、representation与slot relations；NestedNominal保存source nominal id及其inheritance/representation表引用。protected owner必须是class；其visibility为Protected，随owner收窄后的effective domain仍须允许该inheritance路径。不可lookup的internal concrete slot只出现在`InheritanceSlotContractV1`的support target关系，不借此表取得protected声明身份。

`ProtectedDeclarationRefV1`使用同一tag1～4及field1的kind-specific id作为声明表的canonical key；Callable仅接受Function、GenericFunction、Accessor，Constructor和NestedNominal分别保持`PersistentConstructorId`和`SourceNominalId`。完整record直接使用field0的tag及field1～3的四字段sum，不在field1外再嵌套已有三字段record。声明表和ref集合均按该key的canonical bytes排序、唯一；reader拒绝乱序，不修补。定义侧独立提供完整protected声明ref集合，source table验证先精确对照全集，再逐条验证source identity/access/signature及nested support，最后闭合property的getter和真实Protected setter；private/internal setter不要求也不得伪造为Protected callable。public property的protected setter可作为独立Callable根存在，不要求伪造对应Protected Property。该checked source table只证明来源、全集和accessor关系，不独立授予继承选择、default覆盖或生产资格。

Callable分支的声明字段复用既有`CallableTemplateOrigin`编码，但只接受Function、GenericFunction与Accessor；Constructor只使用独立constructor分支的typed id。Callable/Constructor payload为field1～9的product：`owner, type_parameters, receiver, parameters, result, effects, modality, source_interface, slot_relations`。owner使用`SourceNominalId`，binder、parameter shape、result、effects及modality分别复用Stage5对应constituent编码。这里是source signature：nominal member的implicit this只由owner提供，`receiver`使用既有`OptionalSignatureType::Absent`，reader拒绝Present；它与slot/MIR exact callable signature的显式receiver字段属于不同层。`source_interface`是独立closed sum：tag1为无payload的`AccessorNoSourceInterface`；tag2、3、4分别为Function、GenericFunction、Constructor，field1为对应typed declaration id并须等于外层声明。它引用本节protected source-interface表，不能被旧public source-interface索引解释。`slot_relations`为按canonical slot id排序且去重的序列，不表示dispatch位置；constructor固定为空，abstract/open callable须关联真实slot。generic source metadata的binder保留，class generic method按语言既有规则固定Final且不能关联slot；本阶段concrete生产仍受param-free gate限制。

Property payload是field1～6的product：`owner, value_type, getter, mutability, representation, slot_relations`；声明字段使用`PersistentPropertyId`，owner使用`SourceNominalId`，getter/setter使用独立`PersistentPropertyAccessorId`。mutability的tag1是无payload的`ReadOnly`，tag2是`ReadWrite`，field1、2分别为setter与完整`DeclarationAccessSourceV1`。representation复用Stage5 `PropertyRepresentationV1`，但class-owned protected property不能为Const；AbstractSlot必须携带slot关系。reader由foundation property/accessor key重放同一logical property及getter/setter role，再与source逻辑type、mutability、representation逐项join。getter的protected访问源与property一致；setter保留实际声明源，并按语言9.1.5及既有本地实现比较effective lookup域的包含关系，不比较visibility整数。protected setter须在独立protected callable表中闭合；private/internal setter不由该表取得protected callable身份。getter及可访问setter的完整source签名/effect和slot关系在相应表中继续闭合。

NestedNominal payload为field1～3的product：`source_nominal, source_interface, support`。source_interface使用独立`ProtectedNestedSourceInterfaceV1`，field1～9依次为`kind, modality, type_parameters, supertypes, constructors, members, children, source_shape, source_support`；kind/binder/supertype/source shape复用Stage5叶子constituent，modality复用`NominalInheritanceModalityV1`的closed wire并与kind及真实source声明逐项join，不能从kind或source key猜测generic class的Final/Open/Abstract。constructors是typed constructor id集合，members是独立的Function/GenericFunction/Property typed ref集合（tag1/2/3、field1为对应id），children是`SourceNominalId`集合。各集合与source_support按typed declaration key的canonical bytes严格排序且唯一；source_support key是与记录同tag的二字段sum（field0为tag，field1为typed declaration id），不含access/payload。完整section从定义侧权威source全集投射source_roots，source-only根只闭合source identity/词法链，不生成exact inheritance node或concrete能力。不使用`CanonicalPublicMemberRefsV1`或`PersistentExportBindingId`，不生成不存在的普通export binding。

source_support在nested constituent内部闭合：独立closed sum的tag1～4分别为Callable、Constructor、Property、NestedNominal，每条field1为相应typed declaration、field2为真实`DeclarationAccessSourceV1`、field3为完整source payload；nested child递归携带自己的完整source_interface。成员、constructor、child以及property accessor refs必须逐项命中同一容器中kind和owner均相等的support记录。support可保留declared-public/internal/private/protected的真实来源，effective域仍与全部nominal owner相交；记录存在性不授予lookup资格，也不能把declared-public但owner受限的member伪称Protected塞入外层protected声明表。callable signature、source parameter/default、effect、modality、property/accessor关系均由其source payload与独立protected source/default表闭合，不复用旧public表解释这些引用。

support的Callable/Constructor payload复用本节九字段source signature constituent，并由独立support wrapper验证实际nominal owner及声明access；interface owner按既有语言规则允许InterfaceDefault/Abstract，private helper只能Final且无slot，interface method不能有own binder。外层Protected Callable仍只接受class声明，不能因此接收InterfaceDefault。support Property payload为tag1的`Runtime { interface }`或tag2的`Const { value }`：field1分别保存六字段source property constituent，或既有`ExportConstValueV1`四字段const constituent（property、type、typed value、definition origin）。Runtime保留真实getter/setter访问源并验证owner种类；Const仅允许object/companion owner，property identity及origin必须与外层support相等，不携带伪getter、setter或slot。这里复用的是const值语义，不经旧public property interface取得资格。

Nested support的Callable分支另接受既有`CallableTemplateOrigin::VariantConstructor` typed variant id；其九字段payload使用独立`NominalSupportSourceInterfaceUseV1`，tag1～4逐项对应上述accessor/function/generic-function/constructor协议，新增tag5的field1为`PersistentEnumVariantId`。variant的own binder为空、receiver为Absent、执行为Ordinary、modality为Final且无slot；host binder来自对应enum，参数及顺序逐项等于同一enum source shape的variant字段，result为该enum source type/application。reader必须用foundation `EnumVariantIdentityKey`与source enum owner证明identity和访问源，并把tag5协议同该variant的source parameter/default接口join；不能伪造nominal/function `SourceDeclarationKey`或把variant当普通constructor。外层Protected Callable/Constructor及`ProtectedSourceInterfaceUseV1`仍拒绝variant，不扩其closed wire。

两种support分支共享上述完整source层：tag1为`ParamFree { inheritance_exact, representation_owner }`（field1、2分别为`PersistentExactTypeId`和`PersistentTypeId`），必须指向同一source nominal在inheritance/representation表中的记录；tag2为无payload的`GenericTemplate`，保留generic source identity、binder与owner语义，但不持有concrete表引用，也不授予M23-6 concrete生产能力。两分支均须独立验证canonical source key、完整词法owner链和source_support闭包；source接口constituent不得携带或转换成普通public lookup资格。GenericTemplate及处在generic词法owner中的source property只保存并校验真实setter声明源、逻辑type、accessor合同和定义侧完整shape，checked结果显式为GenericSourceMetadata；它不伪造`SubclassesOf(exact)`，也不以visibility整数替代effective域比较。只有全部相关owner均有param-free来源的property才产生ParamFreeDomains证明并重放getter/setter effective域包含关系；generic实例化时的权限重放属于M23-7，当前M23-6 gate不得据source-only证明选择或物化该generic目标。

public property带protected setter时，旧public property仍保留`Restricted`，新表只以Callable分支登记该setter的`PersistentPropertyAccessorId`和protected access；不把整个property改成protected，也不向旧public callable表添加setter。consumer先选中public getter/property，再通过两表的同一property/accessor key取得setter witness。

`InheritanceSlotContractV1` 保存：既有 `PersistentDispatchSlotId`、声明 owner、function/getter/setter 的 typed declaration origin、完整 signature/effect、slot contract domain，以及 `Abstract | Concrete(target) | InterfaceDefault(target)`。每条 slot 与 canonical slot key 关联；override 引用原 slot identity，不以相同方法名或新 owner 重造 base slot。

slot合同wire为field1～7的product：`slot, declaration_owner, declaration, signature, domain, implementation, declaration_access`。本阶段source owner为param-free `PersistentTypeId`；declaration使用`Function/Getter/Setter`（tag1/2/3、field1为对应typed id），getter与setter须分别匹配foundation accessor key及slot role。signature为`{ exact_signature, effects }`（field1、2），复用既有`ExactCallableSignature`与`CallableSourceEffectsV1`编码，双方execution必须相等，receiver必须存在且精确对应source owner。function参数与foundation的binder-free source signature逐项join；getter无显式参数，setter恰有一个参数并返回Unit。implementation为tag1的`Abstract`或tag2/3、field1含target的`Concrete/InterfaceDefault`。target为field1～5的`{ declaration, owner, signature, modality, declaration_access }`，保留实现自身的source owner、完整签名/effect、modality与source access，不能用root-slot receiver冒充实际实现receiver；target不能为Abstract，InterfaceDefault分支只接受interface default modality。原始slot与target的参数/result/execution以及安全性、GC、operator、infix合同逐项相等；intrinsic与普通Scoop body的实现类别可以不同，source extern不进入dispatch support。这里的GC相等比较的是两个source声明的合同，沿用本地override的source attributes相等规则；5.2允许的NoGc value目标到Managed入口只发生在生成的boxing/dispatch adjust层，由两份lowered signature表达，不能据此接受GC source合同不同的override。调用的实际receiver/boxing或dispatch adjust由5.3的typed实现关系承接。

`NominalInheritanceInterfaceV1`按上述顺序编码field1～9，新增的field9为独立`slot_schemas`；合同表`slots`仍按slot id的canonical bytes排序。`InheritanceSlotSchemaV1`是`{ role, slots }`（field1、2）：role为`ClassVtable`（tag1）或`Interface { interface_exact }`（tag2、field1），slots是无重复且保留provider语义顺序的`PersistentDispatchSlotId`序列。schema表按role的canonical bytes严格排序。该序列只定义该exact owner及typed table语境中的dispatch顺序，不保存byte offset、LLVM类型或程序级全局ordinal；同一slot可以出现在多个interface schema，不能为slot合同附加一个缺少table语境的position。interface自身schema与class/value/object实现该interface的schema逐项join同一provider的合同及顺序；derived的ClassVtable保留base完整slot序列为prefix，override只替换已有slot的实现关系，不改变slot identity或位置。MIR按每条序列的顺序枚举position，再依5.3逐项join合同signature与implementation。

必要的 internal concrete slot 可以作为不可 lookup/override 的继承 support 保留，使下游 table 保留其已有实现。private member 永不进入继承/dispatch；public owner 不得留下下游不可实现的 hidden abstract obligation。internal/private concrete owner 中的 public override 可以填充更宽 slot，但不因此成为普通 foreign lookup target。

公开继承入口及其传递 base/interface/slot closure由定义方完整导出。protected nested type 同样保留自己的 identity 与 owner chain，通过合法 subclass scope取得访问证明；不能通过 `public import` 再导出成普通 binding。

### 4.3 protected access 与 default

HIR 依次证明：

1. base/interface 来自合法 source route，或者是已授权 inheritance relation 的传递 support；support artifact 不因此成为普通 import 来源。
2. 当前词法访问位置处于声明 class 或其 subclass body；有效 owner domain 同时满足。
3. 显式 receiver 的 exact 静态 class 是当前访问 subclass 或其 subclass。仅“运行时对象可能是 derived”不够；`Base` 静态 receiver 不获准访问 Derived 作用域中的 protected member。
4. constructor delegation、implicit `this`、explicit receiver、qualified `super` 分别产生用途专属 witness，不用一个可任意复用的 `is_protected_allowed` 标志。
5. property 先选 getter/logical property，随后检查 setter domain。setter 不可见立即报 assignment 错误，不回退其他 overload。

词法检查沿foundation验证后的owner链查找实际授权的class/subclass，witness保存这一个exact access class；它可以是static nested或companion的外层class，不要求最内层owner本身为class。显式receiver仍须是该access class或其子类，且nested/companion不因此获得implicit outer `this`；这一回放沿用语言9.1.5及M21设计5.1的既有授权范围。域相交保留所有独立约束：两个`SubclassesOf`不能仅因class单继承且彼此无subtype关系就判为Empty，因为合法词法owner链可能分别提供两个授权class。只有Cone/File/词法owner约束本身矛盾，或某个必需的subclass区域在已验证且封闭的词法区域内不可能满足时，才归约为Empty；包含关系也须按同一词法/继承语义证明，不能将wire次序当作visibility大小。

object依语言9.1.3直接继承class时，其body也是9.1.5的subclass访问scope，能使用自己的`this`或满足同一规则的显式object receiver访问继承protected成员。witness分别保存source object exact与generated backing-class exact；两者须通过该object的representation record、foundation `ObjectBackingClass { object }` canonical key和exact key逐项join，不能把source object id当作backing class id。继承/静态receiver关系沿source object语义边验证，物理class身份由该checked对应取得。protected声明owner仍只允许source Class；没有直接继承相应class的companion只能沿外层class取得授权，并且没有implicit outer `this`。

inherited protected callable 的 default 继续在定义处解析，使用 M17 的 kind-specific export-interface ref 和完整 call-domain coverage。consumer 只在合法 winner 提交后实例化；不把 private/internal hidden dependency装入 default，也不把 protected reference转换成 universal public witness。M23-5 的 public default wire 原样保留，新的 protected source-interface record单独引用相同表达式语义 constituent。

具体使用独立 `ProtectedDefaultTemplateV1`：field1～10和field12逐项复用Stage5 `ExportDefaultTemplateV1`的key、definition root/path、locals、纯body、result、suspend、binder mapping、receiver、前置参数与origin；field11替换为`ProtectedDefaultReferenceSetV1`。不得把整个旧template/ref-set直接复用，因为旧witness只允许public/universal domain。

新的reference set仍是六个按`(target, definition_origin)`排序的kind-specific集合：callable、constructor、type、global、singleton、field；target类型逐项复用Stage5，record为`{ target, definition_origin, witness, uses }`。同一target的不同origin分别保留；相同target与origin只允许一条record，即使witness或uses不同也视为重复，不靠合并修补。witness采用下述封闭sum；其ParamFree分支保留`{ owner, direct_call_domain, slot_call_domains, target_domain }`，domains使用本节persistent域；slot_call_domains是按slot id严格递增的`{ slot: PersistentDispatchSlotId, domain: PersistentSlotContractDomainV1 }`二字段product序列，逐项覆盖该source callable关联的全部root-slot，constructor及不能参与dispatch的generic callable保持为空。uses精确保留每个附着于Expression的实际引用occurrence的`{ expression_index, receiver_use }`，按expression_index及receiver_use的canonical编码排序且唯一。expression_index是body按wire字段序、source sequence序前序遍历的Expression节点u32序号，不是arena index或新persistent identity；receiver_use是`None | ImplicitThis | Explicit { receiver_expression_index } | ConstructorDelegation`。

局部type、statement binding、pattern、iterator protocol及其他非Expression节点的引用不伪造expression_index，也不生成uses元素。reader仍由完整typed body visitor逐个重放其target、definition/evaluation origin、词法访问scope和实际receiver；protocol或binding receiver只可来自已验证typed plan，不能一律按NoReceiver处理。六域reference闭包必须精确等于Expression引用与这些metadata引用的并集；只有metadata引用的record允许uses为空，但仍须通过全部target、来源和整个default call-domain coverage检查。空uses不得作为跳过依赖、访问或coverage的条件。

`ProtectedDefaultAccessWitnessV1`的tag1为ParamFree：field1～4依次为owner、direct_call_domain、slot_call_domains、target_domain；direct与target均使用`PersistentLookupDomainV1`，slot使用独立`PersistentSlotContractDomainV1`，每个domain必须由实际source/slot合同独立重放，不能把wire DTO当作domain来源。tag2为GenericSourceMetadata，仅有field1 owner；owner在两分支都必须等于完整template key的owner。GenericSourceMetadata只在已验证source的完整词法owner链包含GenericTemplate且独立定义侧将该完整default分类为generic metadata时成立，不能由wire tag、outer qualifier或callable自身binder列表自证。定义侧从完整source/default语义闭包决定profile，reader逐项join该预期分类；artifact不能任意选metadata分支降级应可验证的param-free default。依语言9.1.3，static nested没有outer type parameter：直接source owner为Concrete、外层generic只作为qualifier且实际domain与传递依赖均可证明时，仍使用ParamFree；实际需要未具体化generic owner/default域时才保留metadata。param-free nominal下仅函数自身generic时仍用ParamFree，不允许借metadata分支跳过可证明的coverage。

默认值 source profile 由每个参数的完整 sealed source 独立投影，并以 `CanonicalDefaultSourceProfilesV1` 保存；每条记录恰为 field 1 的完整 protected default key 与 field 2 的 profile sum（tag 1=ParamFree、tag 2=GenericSourceMetadata，均无 payload），表按 key 严格递增并精确覆盖所有默认正文，包括没有引用的正文。producer 可以排序，reader 拒绝乱序、重复、未知 tag/identity 与缺失或额外字段；解析后的表仍不授予 profile authority。分类检查直接 nominal owner 是否为 generic，以及发布声明的实际调用域和同一次默认正文六类引用的原始调用/槽/目标域是否保留 generic subclass 约束；仅外层 generic qualifier 不构成 metadata 理由，仅 callable 自身 binder 也不构成 metadata 理由。generic enum variant 使用 enum 自身参数状态，不能遗漏其 owner。每个参数分别分类，不能用某个参数的依赖替代同 owner 其他参数的来源。该清单与正文、参数省略表使用同一预算生产，完整 defaults 事务仍须重放每个域、receiver、来源与传递依赖，并逐项核对分类；ODR/执行 gate 保持独立。

profile reader 的来源前置条件是完整六类 target-domain 绑定：以同一 BoundNominalDefaultDeclarationsV1、同一 provider 注册表和原预算依次完成 Type、Constructor/Global/Singleton/Field、Callable 的逐 occurrence 重放，不接受调用者拼接任意独立凭证。声明绑定同时保存独立重放的 publishing_call_domain 与原 provider direct_call_domain；前者来自完整 template key 的真实发布声明，后者来自 definition root，两者不能因继承或空引用而混用。profile 表先精确覆盖已绑定的完整模板集合，随后逐项读取已验证的直接 nominal owner binder、发布域和全部 occurrence 的原始调用/槽/目标域，重放本节两分支分类并相等比较。任一覆盖、域或分类错误不交付结果；所有模板、查询及空表使用同一资源预算。只有成功结果实现 profile semantic authority，查询须接受并消耗调用者 BudgetMeter；raw canonical 表保持纯来源数据。此证明只闭合 profile 来源，其他 body operation、receiver、capture/ABI、继承代换及调用域 coverage 义务继续由完整 defaults 事务验证。

默认操作类型的共有协议查询直接使用已导入的 typed 语言角色：Unit、Boolean、所有整数宽度和符号、String、Throwable、ForeignCallbackState，以及 Array、MutableArray、Option、ForeignCallback 的 generic identity。只识别真实 generic id，已识别应用精确校验一项实参；未知 identity 不按名称、布局或 arity 推断角色。查询与实参复制均纳入 defaults 原预算，复制以既有迭代类型变换引擎的 Copy 分支完成，保留全部 source binder 和函数 effect，不进行实例化或补造 exact type。此查询只提供操作校验所需的类型输入，不证明应用实参合法性、声明访问或执行能力，也不引入额外 core 来源资格门槛。

nominal 操作 shape 由 BoundNominalSourceContractsV1 的实际 artifact 来源重放：nominal type 核验 kind 与自身 arity；普通 struct 与 enum variant 保留声明顺序的完整字段类型；struct field、variant 和 variant field 的 typed key 必须属于所声称的真实 owner，字段位置由 source 顺序独立取得；singleton 则沿实际 object-value key 回到 source object。泛型实参按声明顺序映射至 nominal 自身 binder，使用既有受预算的一次代换，不吞掉开放 binder、不捕获 static nested 的外层参数。struct 字段不可变，intrinsic representation 不能被当作空普通 struct 构造。来源表查询、每次类型复制、输出分配和代换共享原预算，缺少实际 provider 记录、错误 owner/kind/arity 或资源不足均失败；此来源查询不替代参数 bound、receiver、可见性及执行检查。

nominal callable 操作来源沿同 artifact 已绑定的 Function、GenericFunction 或 PropertyAccessor 记录读取完整 payload，不能由表达式自报参数或结果反推。concrete 成员允许从实际声明补出省略的 owner self type；generic 成员必须提供直接 owner 的完整应用，static nested 不捕获外层 qualifier 参数。owner 与 callable own binder 的 arity 分别核验后一次代换，返回独立 receiver、空 lexical captures、完整参数、result 和 execution effect；vararg 保持已验证的 Array 参数表示。构造操作沿已核验的 source constructor、variant 或零参 adapter typed 关系回到同份 nominal/参数来源；零参 adapter 的原参数必须逐项为 Default、VarargDefault 或 VarargEmpty，Required 立即拒绝。空参数列表仍计费，来源查询与代换共用原预算。该输入层不提前授予 bound、receiver、访问、正文 default 展开或 adapter 执行资格。

完整default先从独立checked callable source取得owner/profile证明，即使body没有外部引用、六域reference set全部为空，也不得跳过该分类。reference访问重放仅接收实际typed occurrence、真实receiver上下文及该source证明；独立authority先验证target来源和词法/receiver规则，再返回与同一checked inheritance graph关联的target域，不能读取witness所声称的target域作为预期值。每个实际occurrence随后与wire witness逐项join，并验证完整direct及每个root-slot调用域的coverage。

两分支都重放完整body、binder、source origin、typed target、receiver及源码合法性。GenericSourceMetadata只推迟无法定义的concrete-domain包含证明，不保存假exact或Empty/Universal域，且不产生concrete access资格；所有selected、default展开和物化入口必须显式拒绝该分支。其存在只使GenericTemplate的源元数据在本节完整保存，M23-6的param-free执行门限不变。metadata proof与可执行的ParamFree coverage proof使用不同checked类型，不提供隐式转换。

`ProtectedDefaultExpressionUseV1`是二字段product：field1为`expression_index: u32`，field2为`ProtectedDefaultReceiverUseV1`。receiver_use的tag1、2、3、4依次为None、ImplicitThis、Explicit、ConstructorDelegation；只有Explicit分支另有field1 `receiver_expression_index: u32`，其余为仅tag的sum。canonical uses按expression_index及receiver_use的tag/内部index严格递增，producer拒绝重复，reader拒绝重复和逆序，不修补输入；序号范围、receiver种类与完整body的精确对应由独立visitor回放。`ProtectedDefaultSlotCallDomainV1`的field1、2为slot与domain，canonical表按typed slot id严格递增；同slot重复即使domain相同也拒绝。以上集合仅是传输数据，空集合不证明无receiver或无root-slot，完整template必须对照实际body及source callable的独立全集。

receiver_use从实际typed节点作唯一投射：member field/method（含direct-super）引用的receiver若为该模板receiver local，使用ImplicitThis；其他实际receiver expression使用其完整body前序编号的Explicit。type、global、singleton及无receiver的ordinary call引用使用None。Stage5的ClassInit表示普通构造表达式，不证明ConstructorDelegation；在没有真实delegation typed节点的本节default body中声称该tag必须拒绝。metadata引用保留其完整typed statement/iterator/binding上下文供独立source、origin、domain及receiver校验，不通过None抹除真实receiver。精确body/ref-set闭包只证明出现关系，不能单独产生访问、default展开或selected资格。


reader重放完整body/reference闭包、definition-before-use及各receiver的静态type；每个occurrence必须命中相同typed target并满足4.3的词法/receiver规则。target domain必须覆盖direct call domain和每一个slot call domain；domain不是两个可比较visibility整数，也不能只覆盖当前某一个consumer调用点。default provider、参数位置、完整binder映射和definition/evaluation origin仍按Stage5规则验证。继承或扩大override调用域时重新检查coverage，不可把protected dependency藏进public default；constructor默认表达式仍遵守初始化receiver禁用规则。

public与protected default的receiver合同均来自独立原provider声明：正文receiver及其This local须在provider binder frame中精确匹配该声明的source receiver。继承默认值仍保存Base（或Base<T>）的receiver，不能要求其等于发布override的Derived receiver，也不能把正文receiver重写成Derived来通过验证。独立继承来源验证同时接收完整provider到发布owner的binder映射，核对实际override边、代换及发布receiver到代换后provider receiver的合法关系，包括正文未使用的binder。直接定义使用恒等binder映射，provider的binder shape和receiver必须分别等于已检查的发布声明合同。public callable接口的receiver字段仅表示extension receiver；成员默认值的隐含This必须由已检查的nominal owner与host/own binder frame核对，不能把该字段的Absent误当成成员没有receiver。constructor与enum variant的provider receiver固定不存在。protected/default共有metadata的上述查询和raw receiver/local比较共享本次预算；旧public语义入口保持既有预算边界和wire格式，不从候选receiver、候选mapping或witness反推预期来源。

普通 `.slib` reader 还须从共有 callable 声明及 source parameter 协议重建默认值的定义方与发布方合同。definition path 的 ordinal 按定义方参数表中实际带默认值的参数顺序计算，不按参数位置或候选正文推断。分别比较原定义方的参数前缀、receiver 和结果，并以完整 binder 映射逐项比较全部发布参数，包含当前参数之后的部分和未使用 binder；直接默认值必须使用恒等映射。原定义方与发布方的执行 effect、完整参数数及当前位置均须一致。该校验与类型接口默认值共用受累计预算约束的逻辑，只消费已有声明和协议，不引入第二份 wire 合同；override 唯一性、访问域、正文操作及引用闭包仍由相应校验负责。

普通 `.slib` reader 在默认值声明合同、provider 类型作用域和访问 witness 检查后，必须沿原 artifact 的累计预算重放正文与六类引用的精确闭包。遍历覆盖全部 local 类型、嵌套 callable/capture、绑定计划、receiver、表达式与语句；每个实际直接引用按完整 typed target、definition origin 和声明调用域 witness 匹配唯一记录。缺失引用、同 target 的错误来源、重复用途未合并以及正文未使用的额外记录均拒绝；不能仅验证外部依赖集合或把候选引用表本身当作正文需求。错误保留模板 key、引用类别及具体 occurrence，验证失败不交付 source-interface checked 状态。该检查复用共有默认正文遍历和闭包校验，不新增 wire 证明表，也不代替目标访问、操作类型、override 或 nested ABI 的独立验证。

默认值引用 object 属性的 backing field 时，外部引用来源与 binding root 查询沿已有 FieldIdentityKey、ObjectBackingClass generated nominal key、源码 object 及 logical property 的 typed 关系解析；生成字段的存在不等于缺少源码根。必须核对生成角色、源码 object kind、property 的直接词法 owner 及实际 provider 一致性，然后使用该源码 object 的 binding root；不得将其他生成字段泛化为可公开访问的源码字段。查询只确定来源和路由根，具体字段访问仍须经过声明域与 receiver 校验。

共有 HIR foundation 投影必须在具体 callable-reference 物化之前，从 sealed Export HIR 的实际 source callable-reference descriptor 收集 invoke key 和 definition origin，连同已有 callable parent 身份及准确 source points 一起发布。普通与 layout 生产入口使用同一投影和累计预算；只补齐源码身份，不由此产生机器定义或扩大物化能力。普通 `.slib` reader 在默认值 provider 合同和完整正文引用闭包之后，必须从实际正文建立借用的嵌套 callable occurrence 索引，并在同一累计预算内核对每个 local function、lambda、anonymous function 与 callable-reference descriptor 的身份。查询使用该 occurrence 来源所属实际 provider 的共有 foundation 和同一已验证 identity graph，保留源码 local function 的 lexical scope/source/path，以及生成 callable 的精确 typed role、structural path 和 lexical parent；不能由候选引用表、仅同名声明或 core 来源代替正文归属。父级必须与实际 source context 一致，enum variant 父级还须覆盖 descriptor 的来源范围。沿真实 parent/owner 链计算全部词法 type parameter 数，包含 ABI 中未出现的参数，static nested nominal 开始新作用域；descriptor owner arity 与显式 body arguments 均须精确匹配。缺失 artifact key、错误角色/来源/路径/父级/context、伪造 binder 数和耗尽预算均拒绝，并保留模板 key 与 occurrence site。该检查与已有类型接口共用同一身份查询，不增加 source-authority wire 表，也不代替 callable target domain、capture/ABI、receiver 或操作类型检查。

普通 `.slib` reader 对默认值的 constructor、global、singleton 与 field 引用同样独立重放 target domain。路由只读取原 artifact 的共有 foundation 与同一已验证 identity graph：source constructor 及其零参数 adapter 保留实际 constructor，variant 保留实际 enum owner，global 绑定 top-level logical property，singleton 绑定源码 object，struct field 绑定表示 owner，class/object backing 或 delegate field 绑定实际 logical property。必须核对 typed role、实际 provider、source owner 与 applied owner root/arity，再从共有声明 visibility 和完整词法 owner 链推导域；tuple field 的访问域为 universal，其合法索引仍由操作类型检查负责。public witness 不能代替这些声明查询，生成字段也不能因来源是 core 或拥有机器定义而获得额外权限。所有查询与比较共用原累计预算，缺失声明、错误来源或角色、owner 不匹配与伪造目标域均拒绝。已有类型接口默认值复用同一 identity 路由实现，不复制 source-authority wire 表；该访问重放不代替 constructor 参数合同、具体字段类型/索引、receiver 权限及操作类型校验。

普通 `.slib` reader 在默认值引用闭包和嵌套身份检查之后，必须从完整正文的实际 callable occurrence 重放目标访问域。命名函数、generic function、getter/setter 与绑定成员沿 typed key 路由到实际 provider 的共有 callable 声明，访问域由声明 visibility 与完整词法 owner 链取交集；accessor 的 owner 链来自同一 foundation 的 logical property key，可见性仍取 accessor 自身。local function 必须存在于该正文的实际嵌套索引，lambda/anonymous function 必须附着于当前 occurrence 的 descriptor。callable-reference invoke 必须按当前正文节点选择 descriptor，再沿其真正 named/local/bound target 重放权限；相同 invoke 身份在多次展开中的每个 descriptor 均独立检查，generated invoke 不能充当命名源码目标，local reference 的声明与实际 callee 必须一致。derived equality 的 Unit/tuple 沿完整类型域处理，struct/enum 只取真实 nominal 声明域并检查 kind 与 arity，应用参数仍是独立类型访问需求。每次出现须按完整 typed target 和 definition origin 匹配唯一引用记录，目标域与 witness 精确相等；缺失 provider、错误角色、正文归属、局部声明、来源和伪造访问域均拒绝，错误保留模板 key、引用序号及 occurrence。共有查询继续使用原 foundation、同一 identity graph 和累计预算，不新增 wire 证明表，也不代替 callable 签名、receiver、override/slot、nested ABI 与操作类型检查。

普通 `.slib` reader 还须独立重放每个默认值发布声明与原定义声明的完整调用域，包括正文没有引用的模板。direct domain 取实际声明 visibility 与全部词法 owner 域的交集；根槽沿自身有效域，override 的槽域取声明 visibility 域，不能被实现 owner 收窄。实际 override 关系由共有 nominal 的完整继承边、真实成员声明及代换后的完整参数/结果签名重建，参数名不参与匹配，execution effect、operator 与 infix 合同须一致；private 成员不继承，跨 provider 的 internal 成员不能成为 override 来源。每条 generic 继承边使用已有受预算的一次类型代换，保留不同实参的同一 interface、未使用 binder 和 static nested 的独立作用域，循环、错误 arity 与错误 typed slot 关系均拒绝。非 virtual 的 final/值类型 interface 实现即使没有自身物理槽，仍承担继承契约的槽调用域；interface override 即使拥有自身 slot identity，也按实际继承关系认定为 override。仅在最近匹配的 class 声明同为 protected 时继承其槽域，遇到已扩大为 public 的中间 override 不得回退到更早的 protected 根。provided slot domain 必须覆盖全部实际 inherited slot domain。原默认值定义声明不得是 override；继承模板的原定义声明必须在真实 ancestor 关系中，且完整 provider binder mapping 精确匹配实际继承应用。每类引用 witness 的 owner、direct/slot 域须等于独立重放值，target 域须覆盖整个发布调用域；仍需单独验证继承默认来源唯一性、参数省略协议、receiver、操作类型与 nested ABI。上述重放仅消费同一 artifact/provider 闭包及共享累计预算，不新增 wire 授权记录或机器物化能力。

默认值的外部依赖仍完整保留语言内建 Unit/Any 的 typed nominal target、实际 provider 与 DefaultDependency role。Unit/Any 不经过源码名称选择，也没有普通源码声明的导出 binding，因此只由 DefaultDependency、SignatureDependency、ConstType 组成的这类记录使用空 binding witness；不得为它们伪造导入 route。该规则仅识别语言固定的 Unit/Any typed identity，不能按 CORE 来源、同名或相同表示推广到其他声明。生产端和 reader 使用同一 source-name role 判定；alias、re-export、concrete selected use 以及其他默认值目标仍按原规则要求实际 binding witness。实际 provider、canonical identity、完整默认值引用闭包及类型/调用访问域的检查保持必需；空 binding witness 只表示没有源码名称选择，不是权限证明。共有 external reference 的 role tag 保持不变；Unit/Any 的上述源码依赖不携带 call_sites，不新增授权表。

普通 `.slib` reader 在默认值 provider 类型作用域及正文引用闭包校验后，必须从共有 nominal 声明独立重放每个类型引用的目标访问域，并与唯一引用记录中的 target domain 精确比较。遍历覆盖 generic application 的模板与全部实参、tuple、函数参数和结果，以及 raw pointer/function pointer 的完整子类型；binder 仅在此前已验证的真实 provider/local frame 中视为 universal。每个源码 nominal 按 typed identity 的实际 provider 读取声明 visibility、definition origin 与完整词法 owner 链，不能从 public witness、机器 exact/layout、core 来源或同名/同表示类型推断权限。Unit/Any 沿既有语言内建身份处理；正规化的 pointer wrapper 读取可达 compiler protocol 中唯一的实际 Ptr/FunPtr typed 角色，并核对共有声明的 intrinsic family 与 arity。缺失或冲突角色、不可达 provider、错误类型形状及目标域不匹配均拒绝。查询、结构遍历、约束合并与比较共享原 artifact 的累计预算，不新增 wire 证明表；此项不代替其他五类目标、receiver 权限、override 及操作类型校验。

普通 `.slib` reader 必须继续在原 provider 的 binder frame 中重放全部默认值 local 类型及正文类型，包含未使用 local；local selector 的结构路径须属于该默认值的 definition path。正文中的局部函数签名仅在其自身真实非空 type parameter 组存在时增加一层 frame，arity 从同一已验证 identity graph 的 Function/GenericFunction canonical key 取得，不能由 descriptor、capture 或出现的 binder 反推。共有 source contract view 复用同一完整正文遍历、作用域规则和累计预算，类型接口默认值与普通 reader 不保留两套实现。该类型检查不代替操作类型关系、局部函数 parent/ownership、nested ABI、数据流及引用访问闭包。

普通 `.slib` reader 的共有 definition-source 表逐项按 source identity 中的实际 provider 选择已验证 foundation：当前来源使用当前 artifact，外来来源只使用该 artifact 的可达依赖闭包。在同一闭包中出现但不可达的 provider 不得提供位置依据；当前 artifact 保留的外来 source/context 副本也不能替代真正定义方。每个来源必须匹配选中 provider、真实 source/context 关系及声明的起止字节位置，查询共享当前解码的累计预算。该位置校验由普通 reader 和类型接口来源绑定共用；它仅证明位置存在，不授予默认值继承、声明访问或执行资格。参数等要求由当前声明提供的来源仍在各自声明合同中检查，不能借全局来源表接受外来参数。

默认值模板根的定义来源由共有 HIR foundation 校验。普通 `.slib` reader 按模板实际 source 的 Cone 从当前 artifact 或可达依赖闭包选择定义方 foundation，要求 typed root 确实属于该 artifact，且完整 canonical key 与同一已验证 identity graph 一致。Function、GenericFunction 和 Constructor 必须匹配其真实 callable context；EnumVariantConstructor 必须来自源码 nominal，并匹配其 nominal context。模板、声明和 context 的 source 必须一致；继承默认值保留原定义根，不能以发布 owner 或相同签名代替。所有查询、key 比较和来源比较共用当前解码的累计预算；缺失根、错误 provider、文件或 context 均拒绝，不引入独立来源表或调用方提供的默认来源凭证。

public与protected候选默认值还必须从独立authority取得原provider的完整参数表及该definition path对应的参数位置。借用合同必须结构上携带存在的当前参数，越界位置不能产生合同；其位置和总参数数须与发布声明一致。原始prefix local与result在provider frame中分别精确匹配此前参数和当前参数，随后逐项核对完整参数表（包含当前参数之后的位置）的provider到发布type代换。不能因A、B在继承映射中都变为T而允许把provider的A改成B；不能从候选local/result拼出预期provider参数。声明来源绑定保留这份借用合同以供后续完整authority组合，且不因此跳过override唯一性与body/reference-access验证。protected查询、原始比较和全部代换使用同一个预算，public保持旧wire与预算边界。

protected source-interface表额外保存其default template集合及definition_sources精确闭包，使用独立索引空间；不能把protected key插入旧public source-interface/default表。表的source parameter形状和省略类别复用Stage5规则，template引用由当前protected表的checked key/index解释。

protected source-interface的owner全集由已验证的protected declaration surface（递归包含Nested的完整source_support）和inheritance constructor surface投射，不由candidate协议/default表反推；各source-use中AccessorNoSourceInterface不生成协议行。跨surface出现同一constructor时，其完整source signature、access、binder、effects及source-use必须同值，才能共用一条协议；同identity的不同payload不得覆盖或合并。Public inheritance constructor使用其payload明确携带的独立协议，重叠的旧public source接口仍须同值。这个全集仅证明source metadata支持，private/internal成员不因协议存在而获得lookup资格。

protected source-interface表是按`CallableTemplateOrigin`的canonical编码排序且唯一的record序列；每条record的field1、2为owner与声明序parameters。owner只允许Function、GenericFunction、Constructor、VariantConstructor，Accessor使用无source-interface协议。parameter的field1～4为name、value_type、calling、definition_origin；calling的tag1～4依次为Required、Default、VarargEmpty、VarargDefault，字段与Stage5叶子wire相同，但default位置只由独立`ProtectedDefaultTemplateIndexV1`解释。semantic key使用独立`ProtectedDefaultTemplateKeyV1 { owner, parameter_position }`（field1、2），不引入新persistent id，不向旧public key/index提供隐式转换。顶层protected_defaults按该key排序，source-interface的每个省略位置恰好引用同owner/position的template，全部template都须可由该独立source表到达；definition_sources由完整section对全部来源取精确闭包。读取方逐项对照已验证source callable签名、真实省略/vararg类别和parameter origin；不借旧public callable interface作为protected签名authority。

在本阶段，access bridge 指“source target → checked inheritance/receiver witness → MIR external target”的关系。只有已有 `DispatchAdjust`/`BoxingAdjust` 等语义确实要求时才生成 thunk，并使用既有 generated identity；不为绕过 visibility 新增 public wrapper 或新 persistent id 家族。

### 4.4 HIR selected set

HIR 类型接口的生产入口必须借用同一 artifact 的共有 foundation、已验证 identity graph 与 HIR interface，以及实际可达依赖的同类元数据，不能固定发布空的 selected 表。Signature、Representation 与 TypeTest 三类类型用途从共有 external reference 的实际 type_sites 逐次读取：调用签名与 constructor initializer result 贡献 Signature，类型测试保留 TypeTest，其余值、存储及 sizeOf/alignOf 等用途贡献 Representation；初始化循环消息保留其实际 String 参数依赖。完整 exact 及源码位置继续保留在原 type_sites 中，持久化 target 只对实际 provider 与 nominal exact/use kind 去重。Signature 和 TypeTest 均需要同一类型的 Representation；闭包从实际类型用途及当前类型导出所需的参数自由名义声明继续沿完整字段、enum payload、直接继承及 constructor/slot 签名递归，包含 private storage 和无自身物理槽的实际 interface implementation。每个 provider 的共享声明决定其 source-only 边界，不能通过外来 selected 或同名类型绕过；Unit/Any 仅沿固定语言身份处理。共有 Compile reader 独立重算这三类用途并与候选表的对应分区精确比较，拒绝缺项、额外项、错误 provider 及仅在无关 artifact 中存在的依赖。查询、递归、去重及比较使用同一累计预算，不增加来源 wire 表；这项类型依赖闭合不代替各次 source lookup/access、receiver/default/support parent 证明，以及其余六类操作用途和最终 Compile/Link 资格。

`SelectedExternalTypeUseV1` 保存 terminal provider、exact/declaration target 与封闭的 use kind：`Signature`、`Representation`、`Construct`、`MemberCall`、`SlotCall`、`TypeTest`、`SingletonValue`、`Inheritance`、`ShapeSupport`。涉及 declaration 的分支携带相应 kind-specific declaration ref；slot 分支携带 exact receiver 与 slot；inheritance 分支携带当前 derived owner 和 direct base edge。

selected record固定为二字段product：field1 `terminal_provider: ConeIdentity`，field2 `typed_use`。typed_use的tag1～9依次对应上述九类：Signature/Representation/TypeTest/ShapeSupport各以field1保存exact；Construct的field1、2为构造结果的source exact与typed declaration；MemberCall为实际静态receiver exact与typed callable declaration；SlotCall为实际静态receiver exact与persistent slot；SingletonValue为source object exact与`PersistentObjectValueId`，不得用generated backing identity替代source object；Inheritance为当前derived exact与直接继承edge。构造declaration的tag1、2分别为Constructor和EnumVariant（field1为各自typed id）；member declaration复用`InheritanceCallableDeclarationV1`的Function/Getter/Setter叶子（tag1/2/3），不包含generic template/application或generated实现。直接edge的tag1、2分别为ClassBase和Interface，field1为其target exact，没有NoBase分支。完整record按canonical编码严格排序、唯一，reader拒绝重复和乱序；仅取得这些typed id不构成selected资格。

该persistent集合只按terminal provider与完整typed target去重，不保存或任意挑选某一个source/root/parent provenance。独立checked committed-use authority从全部实际typed HIR roots和递归semantic edges重算精确target集合，完整section逐项对照，再重放每一个真实source使用的lookup/access、receiver、definition/evaluation来源及每一条派生support edge的semantic parent。相同target被多个source或parent使用时，所有证明仍须成立；wire不能以去重为由跳过其中任何一次使用。Representation和ShapeSupport使用同样规则，不伪造import route。候选selected表不能反推实际roots/parents或自证terminal provider，GenericSourceMetadata default不能产生selected、展开或物化入口。

`CrossConeUseSet` 对 type、constructor、member/accessor、slot、TD、object ensure/value 增加独立 typed request 家族，不把所有请求塞入 callable id。source use 的 lookup/access provenance 保留在 HIR；由 lowerer 产生的表示/dispatch support edge 携带选中语义 parent，不伪造 import route。

capability gate 在 winner commit、default expansion 和 persistent materialization 前完成整个请求闭包。缺 source access 是语言错误，缺必需 section/record 是 artifact 错误，闭包需要 ODR/native 能力则使用相邻阶段诊断。成功 `LocalConcreteHir` 只包含 local body 与 external complete ref，不复制 provider param-free body。

## 5. MIR：表示无关的 type/callable bridge

### 5.1 section 结构

```text
CrossConeMirTypeBridgeSectionV1 {
    types: CanonicalVec<ParamFreeMirTypeExportV1>,
    callables: CanonicalVec<ParamFreeMirCallableBindingV1>,
    dispatch: CanonicalVec<ParamFreeMirDispatchSchemaV1>,
    object_values: CanonicalVec<ParamFreeMirObjectValueV1>,
    shape_support: CanonicalVec<ParamFreeMirShapeSupportV1>,
    initialization_uses: CanonicalVec<SelectedExternalInitializationUseV1>,
    selected: SelectedDependencyMirTypeSetV1,
}
```

所有 export 由 provider 的当前 HIR/LocalConcrete 与 MIR 输出逐项 join 产生；re-export Cone 只保存 selected relation，不复制 terminal provider 的 export。

`mir-lower` 提供单一的完整 export 组装入口，按同一次 HIR/MIR 输入依次生产 source 与有限 helper type、普通 callable/constructor、object 初始化 callable、boxing adjust、derived equality、dispatch 和 shape-support 六张组成表。依赖 type/callable/schema 通过共有只读索引参与关系检查，不复制到本地 export；调用者显式传入已经提交的 initialization-use 表，组装时继续核对 local unit 的实际物化 root 与 typed provider 关系。所有组成生产、索引、合并、排序与检查共用一次预算。旧 callable bridge 已发布的实际 typed implementation 只在旧表保留，组装器逐项核对 signature 后将其从新 callable 表排除；不按 core、名称或 ABI 形状猜测分区。重复 implementation、错 Cone 输入或缺失依赖均返回完整错误，不返回部分组成表。此入口产出完整 export 数据，最终七字段 section 的 source/selected 闭包和双 view artifact 检查继续按本节执行，不增设来源授权框架。

`ParamFreeMirTypeExportV1` 保存 `{ exact, origin, facts, representation, base_and_interfaces }`。origin是`SourceNominal(PersistentTypeId) | GeneratedNominal { nominal, role }`，generated role与foundation canonical key逐项相等，不伪装成source声明。representation 是 MIR 自有的 closed sum：scalar/intrinsic、struct、enum、class、interface、object backing 与 generated helper；field、variant、base、capture 使用对应 typed id 和 exact type。它包含可重放布局的完整顺序和 CLayout policy，但没有 byte offset、LLVM type、stride、scan 或 ABI pass mode。

该新 constituent 的 product 使用 field1～5，顺序与上述五项一致；type 表按 exact id 的 canonical bytes 严格递增。producer 可以对已验证 record 排序，reader 必须拒绝重复及乱序输入，不能替 artifact 修补。origin 的 tag1 是 `SourceNominal`（field1 nominal），tag2 是 `GeneratedNominal`（field1 nominal、field2 完整既有 `GeneratedNominalKey`）。exact key 必须逐项等于 `Nominal(origin.nominal)`，source 分支只能引用 parameter-free source declaration；generated 分支必须匹配同一 canonical generated key。`ObjectBackingClass` 的 key 由 HIR foundation 提供并保留在已验证 identity graph 中，其余有限 helper 同时要求存在于 MIR foundation，不能把 HIR-owned backing class 强行补进 MIR 的生成类型表。

facts 为 `{ kind, gc }`（field1、2）。kind 的无 payload sum tag1～3 为 `ZeroSizedValue/NonZeroValue/Reference`，gc 的 tag1、2 为 `GcFree/ContainsManagedReferences`。ZST 必须 GC-free，reference 必须含 managed reference；integer/boolean intrinsic 固定为 nonzero GC-free，空 ordinary struct 为 ZST，enum 保留 tag 因而 nonzero，enum 整体 GC-free 等于全部 variant GC-free 的合取。非空 aggregate 的完整事实由 HIR/LocalConcrete join 提供，不能按“字段数为零”分类 intrinsic，或以 type constituent 的局部核验代替该 join。

representation 的 tag1～10 依次为 `Intrinsic/Struct/Enum/Class/Interface/ObjectBacking/BoxedValue/CoroutineStep/CoroutineSlot/Object`。Intrinsic 的 field1 保存固定 param-free family；Struct 的 field1～3 为声明序 fields、CLayout policy、interior-mutable 的 0/1 unsigned 值；Enum/Step/Slot 的 field1 为语义序 variants；Class 的 field1、2 为 class kind 与 declared fields；Interface 无 payload；ObjectBacking 的 field1 为 declared fields；BoxedValue 的 field1 为唯一 payload field；Object 的 field1 为 backing exact。class kind 的 tag1～3 为 `Final/Open/Abstract`。field product 为 `{ field: PersistentFieldId, value: PersistentExactTypeId }`；variant product 为 `{ variant: PersistentEnumVariantId, fields, gc }`，其 fields 使用 `{ field: PersistentEnumVariantFieldId, value: PersistentExactTypeId }`。field/variant 序列不按 id 重排；重复 id、错误 owner 和 variant-field owner 均拒绝。

固定 param-free intrinsic 的 tag1～4 为 `Unit/Integer/Boolean/String`；Integer 的 field1、2 为 signedness 和 width，tag 与 4.1 一致。generic intrinsic family 不进入该 sum。CLayout policy 的 tag1 为 Ordinary，tag2 为 CLayout，后者 field1 保存 `{ aligned, packed }`；值的 tag1～6 为 `Natural/A1/A2/A4/A8/A16`。这只是源 policy，不携带目标布局；空 CLayout struct 被拒绝。base-and-interfaces 的 field1 保存 `None`（tag1）或 `Base`（tag2、field1 exact），field2 保存 provider direct-interface exact 的 canonical 集合，按 exact id 严格递增。该集合不承载 dispatch 顺序；后者独立来自 4.2 的 slot schema。base 只可属于 Class、Object 或 ObjectBacking，目标必须是 source class exact；interface 目标必须是 source interface exact，重复、乱序和自身关系拒绝。完整继承闭包及 object/backing 两侧关系的一致性由 section 的 HIR/MIR join 核验。

BoxedValue 的 payload exact 与 `BoxedValue { payload }` key 逐项相等，field id 必须由既有 box payload field key 派生。Step/Slot 的 variant 与 payload field id 复用既有 generated enum keys，严格保留 7.2 的两 variant 顺序。三个 helper 的 subject 只能是 source-root exact，不能递归触发生成闭包。closure environment、frame 及 callable/continuation adapter 的执行型 shape 在本 constituent 明确按相邻阶段 gate 拒绝；不以任意 Class 表示伪装导出，也不为这些 shape 新造 machine 字段。单条 record 与 table 的 decoded→validated 路径均在 identity 解析、集合分配和语义遍历前消耗相应预算。这里只建立可核验的 type constituent；完整 section、selected closure 与 production artifact 资格仍必须满足本章其余各节和 11.2。

`ParamFreeMirShapeSupportV1` 是上述有限闭包的表示无关索引，其新 product 的 field1～5 固定为 `{ source_nominal, exact, boxed, coroutine_step, coroutine_slot }`。后两项是必需的 helper exact；boxed 的 tag1 为 `Available`（field1 helper exact），tag2 为无 payload 的 `ReferenceNominalRequiresNoBox`。value 必须取 Available，reference 必须取后一分支；不得用可选项表示缺失能力。source nominal 与 exact 必须指向同一完整 source type export；三个 helper exact 分别 join 同一 types 表的 generated key、representation、payload exact 与 GC facts，Step 的 Completed 和 Slot 的 Value 的 GC facts 必须等于 source，另一个 variant 必须 GC-free。helper 不能充当 source，错误 role 或不同 source 的同形 helper 不能替代。

shape-support表按source nominal的canonical bytes严格递增。所有producer（包括core）的每项source必须属于当前provider；完整section用独立重建的required source-root集合核验精确覆盖，不能从本表反推所需全集。通用MIR表统一保存source、exact、box、coroutine step与slot关系，producer及reader共用相同helper角色、GC与typed归属校验；删除Core/Ordinary root来源枚举及section内第二份core shape缓存。缺少source、helper或所需表项均报共有错误，不能另建或从旧CoreMirBridge补齐第二份root清单。该 MIR 五字段索引不复制 7.2 的 LIR 八 role product，后者另行证明布局、scan、TD、registration 与 definition。

record 之间的查询可以借用本地完整表及已验证 dependency 表组成的只读索引，索引不编码进本地 export wire。type、callable、schema 索引分别使用 exact、Strong target、owner exact 的独立 key 空间；相同 key 出现在两个输入表中也必须拒绝，不能以“内容相同”吞并重复 authority。dispatch 的 base prefix、继承 interface schema 与 implementation 查询均使用这个完整视图，而待导出的 canonical schema 表只保存当前 provider 的 records。索引只承载已验证 constituent 的关系查询，不自行授予 dependency selection；完整 section 必须另将每个借用来源 join 到终端 provider proof，不能把任意表拼装为 Selected。

MIR 定义必需的 source-join 协议，由 `mir-lower` 从当前已验证 HIR/LocalConcrete 与实际 MIR 输出独立投射，reader 则从同一已验证 HIR/基础身份闭包重建对应合同。协议提供当前 provider、独立的 type/callable/dispatch/object export 全集、required source roots、逐项完整预期 record 及已提交 initialization-use 全集；这些查询不能从待校验的 transport table 实现。MIR 检查每张 canonical inventory 的精确覆盖、每条完整 record 的逐字段相等、source root 的当前定义 Cone 及有限 helper 闭包。该 source join 不代替终端 provider/selected 闭包，也不赋予单独的表或 source proof 生产 artifact 资格；最终七字段 section 必须同时持有两类证明。

实际 source-join 适配器只接收同一次 sealed HIR/MIR 生产输入、借用依赖表与已提交 initialization-use；构造接口不接收待验证 export candidate。它复用共有生产器独立投影完整预期记录，所需有限 shape roots 直接取 LocalConcrete 的源码物化计划，并与投影的 shape 表精确核对。candidate 经独立解码或组装后，继续调用 MIR 共有 `validate_sources`，逐表拒绝缺项、额外项及完整记录漂移。source 投影、inventory 分配与后续核对使用同一预算；不复制 candidate 来充当预期，也不将这份只读投影升级为终端 provider、committed-use 或完整 section 证明。

完整 MIR section 的实际 source 适配器从同次 LocalConcrete 初始化单元读取两个生成函数的完整逻辑签名和 GC effect，并与 sealed MIR 的物化根逐项对应；单元集合不能从候选 section 反推。外部类型用途取实际经过 HIR→MIR 转置的 source-exact 关系，只保留外来 nominal 的实际 provider；外部 callable 按同次 ordinary selected 的完整 provider、declaration、implementation、signature 关系排除已在旧分区的用途，其余实际用途进入新闭包。初始化服务仍按既有语义角色参加共有调用验证，不重复放入新的 nominal-member 分区。该适配器实现已有 section source 接口，完整 section 继续完成独立 export 重放、初始化合同和 terminal dependency 闭包；不新增 wire、来源授权记录或 CORE 豁免。

完整 section 的语义引用使用独立 `MirTypeBridgeTargetV1`：tag1～6 依次为 `Type(exact)`、`Callable(Strong owner)`、`Dispatch(owner exact)`、`Object(value id)`、`ShapeSupport(source nominal)`、`InitializationUnit(unit id)`，均在 field1 保存既有 typed id/owner。该 sum 只是待闭合的语义目标，不是 Selected handle。selected record 是 field1、2 的 `{ terminal_provider, target }`；表按 provider 后 target 的 canonical bytes 严格排序且唯一，reader 不从该表反推 required use 全集。当前 checked source 的已提交用途及本地 exports 的全部语义边共同决定闭包，terminal provider 必须提供自己的完整 local export；facade 的 selected relation 不能代替 terminal export。旧 core/ordinary callable 分区按3.2优先核验，重复登记或仅凭 foundation signature 伪造旧 selected proof 均拒绝。

语义边收集逐项覆盖 representation 的字段、variant、base/interface 与 Object backing，callable 的两份完整 signature、role 与 generated relation，dispatch 的 owner/base/interface provider、原 slot declaration、实现与调用 signature，object 的 source/backing/ensure/unit，有限 shape family 的全部 exact，以及显式 initialization-use 的 local/dependency unit 与 object cause。它不读取 foreign body 推断调用图，也不把 property accessor cause 无条件再登记到新 callable 分区。字段/variant 的嵌套 tuple 只在 transient storage 上下文递归到 nominal 叶子；其它 structural/generic 应用以及 signature/独立目标中的 tuple 仍在 M23-7 gate 拒绝。每个 exact 查找、边、节点、排序与嵌套深度共享预算，重复的语义边归为集合，而 wire selected 的重复输入始终是错误。收集器只返回完整待闭合边集；source、终端 provider、旧 bridge 与实际初始化函数证明全部 join 之前不能构造 Selected。

完整 section 的 source 协议还独立提供 committed external-use 全集和当前 local initialization-unit 全集及两种 role 的逻辑签名；不从 selected wire 或待验证 unit 查询结果生成预期集合。unit proof 在内存中区分 `ProducerEmitted` 与 `ReaderSemanticReplay`：前者借用 Strong sealer 的真实 emitted roots，后者只由 checked HIR unit 来源、canonical generated roles 与 MIR foundation/Strong signature surface 重建语义合同。两者都验证当前 provider、ordinary Managed、无 receiver/参数及 Unit exact；reader 不声称已取得 machine body。现有 unit key 已唯一决定两个 role，七字段 wire 不另保存冗余 unit-role 表。

MIR section 保留 terminal dependency section 的借用及本地 source-join 结果，只有完整 export/required-use 递归闭包校验成功才构造本层 selected set。相同 terminal section 经 diamond 重复到达只访问一次；同 provider 的另一份输入 authority 不能覆盖或合并。canonical selected relation 仅包含实际需要的 foreign 目标，缺项、多项、错 provider、把本地目标写成 selected 或跨 request 使用 handle 均拒绝。该证明限于 MIR 语义；外部 machine arena 的最终资格还必须由同一 artifact closure 的 LIR/Strong V2/profile 验证取得，不从 MIR section 单独授予。

本节不遍历 callable body，因而不会把 body 内的旧 ordinary/core 调用重新登记为新边。slot declaration、dispatch 的 source implementation 及 adjust 的 source target 必须由 canonical source key 证明为 nominal member：直接 owner 是 param-free type，source receiver 不是 extension receiver；generated dispatch target 则仅接受已支持的 adjust/derived role。object ensure 仅指向同 unit 的 generated ensure。它们与旧 TopLevel/Extension callable 分区不相交；普通 accessor 的 initialization cause 仍只贡献既有 unit relation。将旧 callable 显式放入本节 export/selected，或把顶层/extension callable 伪装为 slot/adjust target，均拒绝。

M23-6 的 MIR 外来类型来源投影从同一次 sealed LocalConcrete HIR 的实际物化根重算，不能把 MIR source-exact 表或候选 selected 集合当作源码使用证明。根包括实际函数与构造器签名、正文局部值和表达式、已物化的存储及名义表示、显式 shape-support 要求；表达式中的类型测试、sizeOf/alignOf 及函数签名等非结果类型同样保留。闭包沿原 HIR 的字段、variant payload、继承、类型实参与结构化类型子项查询，不扫描 primitive/exact type arena 来增加根，未物化的默认正文与无关 source-only 声明不产生依赖。class constructor 配对的 initializer 保留其 Unit 返回类型；实际初始化 unit 的 ensure 消息按现有 lowering 合同使用同一 HIR String 类型，不能因源码没有字面量而省略。外来 nominal 的 exact id 和 provider 分别读取原 HIR 完备类型关系与共有 canonical 声明 key；只在最后对实际 provider/typed target 去重。查询、遍历、索引及分配消耗同一累计预算；本次查询是临时物化类型闭包，不另存来源表，也不代替各次使用的访问、绑定、support parent、完整 selected-use 与最终 artifact 重放。

### 5.2 callable 与 constructor

`ParamFreeMirCallableBindingV1` 保存 `{ source_or_generated_origin, implementation, semantic_signature, lowered_signature, lowering_role }`。implementation 只接受既有 `StrongCallableDefinitionOwner`；lowering role 为 ordinary、class initializer、struct value constructor、accessor、dispatch adjust、boxing adjust、object ensure/value 等已有 typed role，不根据 name 推断。

新 binding product 按上述顺序编码 field1～5。两份 signature 都使用新 constituent `{ exact, gc_effect }`（field1、2）：exact 原样复用既有 `ExactCallableSignature`，gc effect 是独立无 payload sum，tag1 为 `Managed`，tag2 为 `NoGc`。旧 exact-signature wire 不变。普通 callable、accessor 与 pure-virtual trap 的两份 signature 相等；class initializer 仅按本节规则转换 receiver/result；adjust 保留目标语义 signature 与 slot 调用 signature，并逐项检查参数、result、suspend 及 GC effect 的合法关系。foundation 的 callable signature 锁定 implementation 的 lowered exact signature；GC effect 另与 HIR 合同及实际 MIR function metadata join，不能从同形 LLVM 函数类型补出。

origin 的 tag1～4 是 `Function/Constructor/Accessor/Generated`：前三者的 field1 为相应 typed id，Generated 的 field1、2 为 generated callable id 与完整既有 canonical key。lowering role 的 tag1～10 是 `Ordinary/ClassInitializer/ValueConstructor/Accessor/DispatchAdjust/BoxingAdjust/ObjectEnsure/ObjectInitializer/PureVirtualTrap/DerivedEquality`，tag11 为 `PrimaryValueConstructor`，field1 同样为 owner exact。ValueConstructor 用于 struct 次构造，PrimaryValueConstructor 明确表示只组装已求值字段的主构造 leaf。Ordinary/Accessor 无 payload；ClassInitializer/ValueConstructor/DerivedEquality 的 field1 为 owner exact；两个 Adjust 的 field1 为所调用的 Strong target；ObjectEnsure/ObjectInitializer 的 field1 为 object/companion initialization unit；PureVirtualTrap 的 field1 为原 dispatch slot。constructor owner必须逐项等于源码 owner chain 的最内层 nominal；trap implementation可以是该slot的原 declaration owner，或保留同一slot的abstract class override；后者的源码声明必须属于实际receiver class，该class须沿已导出的直接基类链严格继承root receiver，双方参数/result/execution签名保持一致，实际abstract选择仍由同次HIR/MIR生产核对。object value读取仍用5.4的已授权storage plan，不新造一个返回object的initializer身份。

源码 NoGc signature 的receiver、参数及结果必须全部GC-free。两个Adjust允许GC-free value目标保持NoGc，而接收interface/class ref的wrapper为Managed；Managed目标不能适配为NoGc调用。PrimaryValueConstructor 的源码合同固定Managed，lowered固定NoGc，两份exact signature相同且无receiver、返回owner，参数必须逐项等于struct完整声明字段的exact类型；这一无安全点内部leaf可以组装含引用的值，不授予源码NoGC调用资格。其余角色的lowered NoGc signature同样要求GC-free；除adjust与上述主构造角色外不改变GC effect。constructor/accessor/object initializer/ensure/derived equality均为ordinary同步角色；object ensure/initializer保持Managed。callable canonical表按Strong implementation的既有kind/id顺序保存，reader拒绝重复或乱序；两份签名与role均不可缺失。该constituent核对canonical identity、signature和type facts；slot选择、真实body和GC effect的完整authority仍须经HIR/MIR生产join取得。

source signature 与 lowered signature不能合成一个字段：class constructor 的源码结果为 class value，MIR initializer 取得同一个 initializing receiver并返回 Unit；enum variant construction 可以只有 representation operation而没有独立 machine body，使用专用 construction plan，不虚构 callable definition。

class construction 固定为 exact allocation一次、同一 receiver direct调用 initializer；base/`this` delegation 不分配、不改 header，最派生 TD 从 allocation 起保持。abstract class可有供 derived调用的 initializer，但不能构造 allocation target。跨 Cone构造保留 base-before-derived、共同初始化一次、异常不发布结果及每次 call 后 receiver relocation。

普通 member、extension、value constructor和adjust thunk都执行按值 receiver/参数语义。`@InteriorMutable` 或 `addressOf(this)`可观察时必须有方法局部 copy；即使 physical ABI使用 pointer，也不能把 caller/box内存变成方法的可修改 `this`。

MIR source constructor 的必需集合直接从共有 nominal/callable 声明的参数自由物化闭包查询，限于当前 provider 的公开或 protected 构造器；完整声明中的 private/internal、generic/source-only owner 与 object 隐式初始化入口不因存在于源码表而成为构造器导出根。查询仅返回有预算的借用索引，producer 与 reader 共用，不新增 wire 来源清单。reader 从同一共有声明逐项核对完整语义签名、GC effect 与实际 owner：class initializer 把源码返回值转换为同一 owner 的 initializing receiver，并返回 canonical Unit；struct 唯一 primary 的参数类型序列与完整声明字段序列相同，既有 constructor duplicate-signature identity 保证同一 owner 下该签名唯一，其 lowered 角色为 PrimaryValueConstructor、GC effect 为 NoGc；其余 struct constructor 保持 ValueConstructor 与源码 GC effect。缺失、额外、错 owner、错参数、错 receiver/result、错误主次角色或 GC 漂移均拒绝。枚举变体仍使用 representation construction plan，generated callable、dispatch、初始化、selected 和完整 LIR/artifact 闭包继续按各自合同核验。

派生 equality 的共有 MIR reader 从同一 HIR foundation 的原有 DerivedEquality application key 查询源码身份，并与已核验的 Strong callable/signature 表关联实际函数投影；后者只确定哪项源码 application 已有机器定义，不能反推源码资格。仅当前 provider 的已闭合 struct、enum 及 canonical Unit 进入类型桥接集合，私有未导出 owner、无关声明与未物化的默认正文 application 不增加导出根。每个实际导出的 application 必须保留原 generated identity 与 exact owner，semantic/lowered 签名均为 ordinary Managed、receiver 与唯一参数同为 owner、返回从共有声明及真实可达依赖按 Boolean intrinsic 角色查询的唯一 typed exact，lowering role 必须为 DerivedEquality。缺失源码 application、缺失或额外 binding、owner、签名或 GC 漂移均拒绝；Strong 定义集合继续与同一 MIR foundation、LIR 和对象定义关联，源码 application key 本身不代表 machine body。该查询复用已有 identity 记录和临时预算索引，不新增来源表、caller factory 或 wire 字段。派生 equality 正文的 receiver 与唯一参数使用原 This、Parameter(0) local selector，但其定义站点是编译器合成；reader 仅在 local owner 的原 generated key 确为 DerivedEquality 时不要求这两个值的 definition origin，并拒绝为它们附加伪造的源码记录。其他源码局部值继续核对真实 owner 和来源锚点，不能按 generated owner 一概豁免。codegen 对 tagged enum 字段重放存储时，零大小字段必须保持 canonical offset 0，不加 variant payload 起点；非零字段继续按 slot 与相对偏移校验，全部字段的类型、大小、对齐、GC 属性和 scan 仍逐项检查。实际 initialization-use、selected-use 与最终 artifact 闭包仍须独立核验。

### 5.3 dispatch schema 与 table 构造

`ParamFreeMirDispatchSchemaV1` 按 exact owner 保存 class vtable schema 与按 exact interface排序的 itable schema。每条 entry包含 `{ slot, position, slot_signature, implementation }`；implementation是 `AbstractObligation { declaration, trap_target, receiver_adaptation } | DirectStrongTarget | InterfaceDefaultTarget | AdjustThunkTarget`。abstract分支沿用当前MIR的typed pure-virtual trap body，使用实际abstract声明对应的Strong callable identity与完整signature；interface obligation使用原interface declaration，class再次抽象化可使用保留同一slot的derived abstract override；它有明确fatal出口，不留下null/未解析function，也不授予源码direct call权限。

schema wire固定为field1～3的`{ owner, vtable, itables }`；vtable是tag1的无payload `NoClassVtable`或tag2、field1为有序entry序列的`ClassVtable`。itable为`{ interface_exact, entries }`，表按interface exact严格递增；interface provider保留以自身exact命名的schema。entry按上述四项编码，position为table内的typed u32序号，必须逐项等于语义序列的0至长度减1，不是persistent id或全程序ordinal。slot signature复用5.2的完整`{ exact, gc_effect }`。implementation按上述顺序使用tag1～4；Abstract的field1、2分别复用既有`DispatchDeclarationOwner`与`StrongCallableDefinitionOwner`，field3保存receiver adaptation；后三个分支的field1保存Strong target。DirectStrongTarget与InterfaceDefaultTarget另以field2保存无payload的`Identity`（tag1）或`ReferenceDispatch`（tag2）receiver adaptation。

Direct receiver adaptation沿实现规范2.9的同一object身份规则：Identity要求slot与target完整签名相等；ReferenceDispatch仅允许receiver不同，其参数/result/execution/GC完全相同，且从当前schema owner通过显式base/interface/Object→backing边分别到达两个reference receiver。两条规范路径由验证器重建：先取最短，再对同长路径的完整exact-id序列取canonical bytes最小者；不在wire保存任选路径。receiver相等时必须使用Identity，因此同一关系只有一种编码。真实value payload变换仍必须选择AdjustThunkTarget并由既有generated key与两份完整signature证明。值类型采用interface default时也保留BoxingAdjust：semantic receiver是default body的interface，lowered receiver是key指定的目标itable interface，body仅重解释同一个box；payload到default interface的可达关系由schema验证，不能以Direct/Identity分支绕过。

itable的slot signature receiver固定为该表的interface exact，class vtable则保留原slot declaration receiver；因此derived class base prefix保持完整签名不变，而继承interface的provider/consumer共享当前interface的调用签名。原declaration与该调用签名仅receiver可不同，须由当前interface到声明interface的typed路径证明。生产lowering须按这份table调用签名建立interface call ABI；当前本地callee signature不能仅因receiver物理形状相同就替代table合同。AbstractObligation也携带上述闭合receiver adaptation，把当前table receiver适配到实际trap receiver，保留该声明的Strong identity和完整lowered signature；不把derived class的再次抽象化强制回退为基类body。

reader核对每个slot的原declaration callable signature、所选target的完整lowered signature以及adjust的目标semantic/lowered关系。schema表按owner exact canonical排序，并统一重放base prefix、interface provider的slot/signature序列及abstract obligation；这种MIR内部闭合检查不替代HIR source slot schema的原始顺序与override选择join。Object和ObjectBacking沿class-like vtable规则处理；有限BoxedValue/CoroutineStep/CoroutineSlot shape support不另造source dispatch schema，box的descriptor dispatch从payload语义schema与既有boxing adjust机械构造。

ObjectBacking schema调用同一object自身声明的成员时，receiver验证允许直接的backing→source object适配；必须同时匹配已验证的`ObjectBackingClass { object }` origin、source object nominal及其`Object { backing }` representation。该适配不改变两个exact identity，也不加入继承图，不产生双向继承环；其余receiver仍沿原继承图重放规范路径。

interface provider之间的MIR独立核验只比较父子表仍共同保留的slot：共同slot的非receiver签名与相对顺序必须一致，全部保留的继承slot仍在当前新增slot之前。子接口可按typed override抑制父槽，不能要求每个父槽继续出现在子表。被抑制集合、完整父接口声明序及新增slot是否合法，由同次HIR完整source schema与MIR逐项join证明；组成表通过不替代这一完整发布条件。

- derived vtable保留完整 base prefix；既有 slot的 position保持，override只替换 target；新增 virtual family按当前owner的方法声明序首次出现时追加，保持现有MIR语义顺序。
- interface保留“继承slot在前、当前声明slot随后”的schema顺序和去重规则；按typed override关系抑制被覆盖的旧成员后，其余槽保持相对顺序，getter/setter分别处理。consumer调用携带interface TD + schema内position；不存在程序级global slot ordinal。按id查找的wire record table可以canonical排序，但position必须保留provider的语义序列，不能按id重排物理table。
- 每个 concrete owner必须填满 obligation；abstract class可以保留 obligation，但 LIR allocation仍拒绝 abstract identity。
- HIR完成“最近 class concrete override → 删除较不 specific interface candidate → 唯一 default或诊断”的选择；MIR只验证并机械构造，getter/setter分别选择。
- `super`、`super<I>`保存强制 direct target；不会因为 callee属于 open family重新走 table。
- value实现 interface时 table entry指向按 exact implementor/slot/target产生的 adjust thunk；ZST thunk产生 typed value与独立需要的 this token，无 payload load。

slot调用签名与实现签名不同的任何合法情况必须由已有 typed adaptation relation闭合，不能用 LLVM function-pointer bitcast消除差异。没有语义允许的 adaptation时，在 HIR拒绝 override。

MIR dispatch 的共有 reader 在完整 HIR 继承、槽 schema 与源码选择重放后，独立取得当前 provider 的必需 owner 集合、原始槽顺序和每个实际目标；object/backing 共享同一源码选择，内建 Any 仅按其 canonical typed identity 保留空 class vtable。槽调用签名从共有 HIR 根声明投影，interface 表只替换 receiver；抽象 class 入口沿真实 base 链选择最近的同槽抽象声明，interface obligation 使用该槽的原声明，不能从候选 trap target 反推。引用 receiver 采用 canonical Identity/ReferenceDispatch；值类型 interface 表机械选择既有 BoxingAdjust identity，并核对其实际源码 target、语义签名与 Managed wrapper 签名。缺失或额外 owner、table、slot、适配入口，顺序、位置、目标种类、receiver、effect、GC 或 generated key 漂移均拒绝。共有 MIR transport 在 dependency-first callable 解析时使用同一依赖 type/callable/schema 索引验证 dispatch 一次，后续直接借用 canonical 结果；独立源码重放和所有索引、签名比较、继承遍历继续消耗原 artifact 的累计预算，不新增来源副本、caller factory 或 wire 字段。有限 BoxedValue 的 MIR base-and-interfaces 复用同一次源码类型导出中 payload 的直接接口集合，不能把机器 class 的传递接口闭包写入该字段；实际 descriptor/itable 仍沿 payload schema 保留完整闭包。有限 helper producer 必须取得 sealed source shape root 对应的源码类型记录，缺失或 origin 不符立即拒绝。derived equality、实际 initialization-use、selected、LIR 和双 view artifact 的剩余义务仍分别完成。

### 5.4 object value 与初始化

`ParamFreeMirObjectValueV1` 保存 object-value identity、backing exact class、provider-owned unit、ensure callable和value读取入口/已授权storage关系。consumer按普通语义先 ensure，再取得值；不复制 singleton allocation、init cell、failure root或 backing storage。top-level/delegated property继续经唯一 accessor；“有布局”不授权直接读取 private backing field。

该object product按上述顺序固定field1～5；value为`PersistentObjectValueId`，backing为exact id，unit为既有初始化unit id，ensure为既有Strong callable owner。read-plan是tag1的`PublishedSingletonRoot`，field1保存source object exact；MIR只保留该已授权读取关系，LIR由同一source nominal机械派生既有`StaticStorageKey::singleton_published_root`，不在MIR引入offset或物理storage id。reader将value的canonical source key、source Object representation、generated backing key、Object/Companion unit、GeneratedCallableKey::Initialization(Ensure)和ObjectEnsure binding逐项join；没有返回object的虚构initializer。object表按value id严格递增且唯一，provider从canonical source key取得。

Strong MIR sealer必须逐项将每个local initialization unit的ensure/initializer函数与该unit的既有 `GeneratedCallableKey::Initialization` 两个role join，要求两者均为真实emitted Strong body、普通Managed、无receiver/参数并返回同一Unit exact。HIR-owned source callable materialization和MIR-generated relation按其真实来源读取，不要求HIR生成的initializer另出现在MIR-generated delta。sealed materialization plan保存这条完整typed关联，不能只保存unit arena id后让LIR猜函数。该局部函数证明不代替foreign unit的同一terminal provider、真实descriptor/cell和definition closure；后者仍由完整section及11.2闭合。

本阶段关闭 artifact中的所有 typed edge与registration关系，不执行全图 eager startup。M23-8会在登记全部image之后按这些既有unit关系启动，不回到名称解析补依赖。

当前initializer中的显式external ensure通过`SelectedExternalInitializationUseV1 { local_unit, provider, dependency_unit, cause }`记录；cause是`ObjectValue(object_value_id) | PropertyAccessor(accessor_id) | InitializationSupport(unit_id)`，必须由已提交的typed ensure语义产生。LIR selected set保留同一edge，strong-production/6按3.2验证foreign unit，不要求它出现在本地unit arena。这里只记录现有语义的真实ensure dependency，不读取foreign body做跨Cone调用图推断；普通external callable内部自己的ensure继续由provider负责。image/runtime使用canonical unit id解析这些edge，相关真实descriptor/cell relocation按11.2验证。

initialization-use product的field1～4按上述顺序保存；cause是tag1/2/3、field1分别为object-value/accessor/unit id的closed sum。canonical表按local-unit、provider、dependency-unit、cause的typed key顺序保存，拒绝重复。local-unit必须属于当前consumer，provider必须是dependency-unit的真实定义Cone且不是consumer。ObjectValue cause必须对应同一Object/Companion unit；PropertyAccessor cause必须对应同一top-level/extension property unit，或同一object/companion owner的成员property；InitializationSupport必须等于该dependency-unit。param-free表拒绝generic delegated application unit。该constituent验证identity及关系，完整section仍必须把每条use与已提交的typed ensure语义/真实MIR调用逐项join，不能由canonical key匹配推断foreign body依赖。

MIR object value 与两个初始化 callable 的 reader 校验直接读取同一 artifact 的共有 HIR nominal 声明、已重放的表示和原有 initialization-unit foundation keys。参数自由 object/companion 的必需集合来自这些源码数据；每个 owner 恰有一个对应 Object/Companion unit，普通属性初始化与 generic delegated unit 不参与该对象索引。object value 的 typed id、source exact、backing exact、实际 provider、published-root read plan 与 ensure target 必须逐项对应；initializer 和 ensure 的 canonical generated key、lowering role、ordinary/Managed、无 receiver/参数及 canonical Unit 返回值同时核验。缺项、额外对象或初始化入口、相同 owner 的冲突 unit 与任何关系漂移均拒绝，source-only/generic 或无关 private 源码对象不因其 foundation key 存在就成为机器根。对象 transport 在 callable 解析后通过本地 canonical callable 表验证一次；后续完整 section 直接使用该 owned canonical 结果，仍必须完成其它 generated callable、dispatch、实际 initialization-use、selected、LIR 与双 view artifact 闭包。本步骤不新增 wire 字段、来源副本或 caller factory。

共有 MIR reader 从同一 HIR foundation 的原 initialization-unit keys 关联完整 Strong callable/signature 表中的实际初始化定义，成对重放 Initializer 与 Ensure 的原 generated identity、实际 provider、ordinary 无 receiver/参数及 canonical Unit 返回值。只有存在实际 Strong 定义的参数自由 unit 进入已验证 unit 集合；源码 key、默认正文或未物化 object 本身不能制造机器根。任何已发布初始化 role 都必须具有原 source unit 和对应的另一个 role，两个签名分别同 MIR foundation 精确匹配；缺失 source/role、错误 provider、generic delegated application、签名或 callable role 漂移均拒绝。重放结果使用既有 MirTypeBridgeInitializationUnitV1 和 ReaderSemanticReplay，语义签名为 Managed；它不声称拥有 ProducerEmitted body，实际 emitted body、GC、LIR unit registration 与对象定义仍由 Strong/LIR 完整关联核验。共有 Compile 闭包持有完整已重放 unit 集合，查询、hash、排序与分配使用原累计预算；不新增 wire 字段、来源表或 caller factory。unit 的 source owner 只表示身份归属，不自动产生 MIR type/layout 导出边；unit 合同的类型依赖来自其完整调用签名。实际 ObjectValue 使用仍独立闭合 source/backing 类型、ensure 与 unit，私有对象的本地 descriptor、初始化 body 和 storage 仍由完整 Strong/LIR 产物验证，不能为了验证本地 unit 扩大类型导出根。实际 external initialization-use 与 selected 仍须从已提交的 typed 使用重放，不能从本地 unit 列表推断 foreign body 依赖。

## 6. LIR：完整 layout 与 scan

### 6.1 section 与 authority

```text
CrossConeLayoutAbiSectionV1 {
    layouts: CanonicalVec<ExactLayoutExportV1>,
    descriptors: CanonicalVec<ExactDescriptorExportV1>,
    dispatch: CanonicalVec<ExactDispatchExportV1>,
    callables: CanonicalVec<ExactCallableAbiExportV1>,
    shape_support: CanonicalVec<ParamFreeShapeSupportExportV1>,
    selected: SelectedDependencyLayoutAbiSetV1,
}

SelectedDependencyLayoutAbiSetV1 {
    semantic_uses: CanonicalVec<LayoutAbiDependencyV1>,
    physical_imports: CanonicalVec<ExternalShapeLinkImportV1>,
}

LayoutAbiDependencyV1 {
    provider: ConeIdentity,
    target: LayoutAbiSemanticTargetV1,
}

LayoutAbiSemanticTargetV1 =
    Layout(PersistentLayoutId)                  // tag 1
  | Descriptor(PersistentExactTypeId)           // tag 2
  | Dispatch(PersistentDispatchTableId)         // tag 3
  | Callable(StrongCallableDefinitionOwner)     // tag 4
  | ShapeSupport(PersistentTypeId)               // tag 5

ExactLayoutExportV1 {
    layout: PersistentLayoutId,
    exact: PersistentExactTypeId,
    target: TargetProfileWireId,
    role: RepresentationRole,
    body: Value { storage: ValueStorageLayout, representation: ExactRepresentationLayoutV1 }
        | Instance { shape: TypeInstanceShape, representation: InstanceRepresentationV1 },
    scan: PersistentScanId,
    definition: StrongShapeDefinitionV1,
}
```

`selected`是field1/2分别保存`semantic_uses`与`physical_imports`的closed product；`LayoutAbiDependencyV1`的field1/2分别保存provider与target，target按上列tag编码且payload在field1。semantic表按`(provider, target canonical bytes)`严格递增。每项provider必须不是consumer，并精确命中显式dependency closure中唯一terminal provider的对应五张表；内存中的selected entry保留该terminal section引用和request-local brand，不能由wire relation、裸id或单张constituent table直接构造。

producer与reader都从同一份已提交MIR→LIR typed selection和完整本地LIR记录独立重算semantic闭包：layout递归跟随base/field/variant/array等内嵌layout引用；descriptor跟随value/instance layout、parent/interface TD及vtable/itable；dispatch跟随interface/owner type与每个target callable ABI；callable跟随receiver/parameter/result layout；shape-support跟随八个role对应的layout/descriptor/helper记录。metadata-only读取保留在`semantic_uses`即可，不因此产生relocation。每个跨provider递归edge都进入真实terminal provider，依赖section中的转发selected关系不能代替terminal记录；环、同target多provider、当前Cone回指、缺失/额外关系及旧core/M23-5分区冒充新selection均拒绝。

两条路径在计算闭包前都完成共有跨阶段校验：生产侧将本地五张 export 表与同一次实际 MIR→LIR 输出关联，reader 将其与同 artifact 的完整 MIR type/callable/dispatch metadata、Strong production、layout/ABI 和实际 object use 关联。physical_imports 必须精确对应机器使用及必要 object/init support；五类一般 import contract 重新绑定到依赖闭包中的同一 terminal section 记录。单表构造成功或 import 已解析不能代替完整覆盖与相邻阶段一致性检查；reader 不要求另外提供编译期 source authority 或平行授权 transcript。

`physical_imports`就是11.2定义的canonical semantic-import projection，按`(provider, subject canonical bytes)`严格递增且唯一。它由实际machine use、strong-production/6引用及已授权object/init support独立收集，再与对应terminal semantic记录、Strong definition及旧分区逐项join；semantic use可以没有physical import，physical import不能只有symbol或definition而没有完整semantic/support authority。该数组与Link section field1及Code contribution逐byte相等，selected的brand和terminal引用不编码。

layout 表以 `PersistentLayoutId` 为主键，同一 exact 可以有不同 representation role 的多项。`TargetProfileWireId`、`RepresentationRole` 原样复用 foundation 的 `LayoutKey`：ManagedValue/CValue/NativeFunctionPointer 必须匹配 Value body，ManagedObject 必须匹配 Instance body；scan key 的 layout/role 同样重放。target 必须等于同 artifact 已验证 LIR projection，不能仅比较可读名称。`StrongShapeDefinitionV1` 复用 M23-3 的 semantic-id/definition-plan/symbol product。每项 layout/scan/descriptor引用 foundation中的既有 key；definition必须在 provider strong production中有唯一primary atom。

普通引用值的 Value body 是 managed pointer大小/对齐和单个 managed leaf；它指向的对象 field layout属于独立 ManagedObject layout 的 Instance body及object scan。`InstanceRepresentationV1`的tag1～5依次为`ClassObject { base_prefix, declared_fields, complete_fields }`、`BoxedPayload { payload_exact, value_layout }`、`InlineBytes`、`InlineArray { element_exact, element_storage }`、`AbstractReference`。class base prefix与新增字段只在ClassObject中拥有authority；complete_fields是从base prefix和declared_fields机械拼出的全序投影，reader逐项核对，不能自由提供一份不同的flattened list。不能把空 class误判为 size 0引用值。

scan identity的role矩阵保持既有规则：ManagedValue、CValue、NativeFunctionPointer均配InlineValue；普通ManagedObject配ManagedObject；intrinsic array的ManagedObject layout配ArrayElement，其canonical scan描述单element。array descriptor的object scan另从该element scan与length/data offset/stride派生，不能把element scan当作object-relative scan，也不能为方便改写已冻结ScanKey。BoxedPayload的inline scan来自payload value layout，object scan来自checked平移。

consumer只能通过 `ExternalExactLayoutRef`取得完整事实，并在本地 aggregate中重放外层布局。外部 layout/scan constant、TD和table全部保持 external definition；本地内联 field offsets不构成复制外部addressable constant的许可。

### 6.2 storage 与 field layout

M23-6 的共有 Compile reader 直接从已核验的 MIR 类型组成表重算 LIR layout 导出，不接受调用方提供另一份预期表示或来源 factory。每个实际导出的参数自由 nominal 及有限 helper 必需 ManagedValue 与 ManagedObject 两种角色，CLayout struct 另需 CValue；已核验的 ObjectBacking 仅为对应 object 提供完整字段和继承形状，不独立产生布局角色。集合从 MIR 表示决定，不从候选 layout 或 foundation layout 清单反推。无关私有本地定义继续属于完整 Strong production，但不加入导出根。重放使用同一 artifact 的 typed identity、LIR foundation、实际 target 和可达依赖的已检查 layout，按声明序计算 field、variant、base prefix、ZST、GC scan 与既有 CLayout 合同；引用环经 managed value 表示终止，按值或基类循环拒绝。Unit/Any 的固定语言角色保持，其他 intrinsic 均来自已检查的实际 MIR 表示，不按 CORE 来源放行。每条 wire layout 的完整 identity、role、representation、storage/shape、scan 和 definition 与重算结果逐项一致，缺失、多余、乱序及错 provider/target 均失败。图遍历、查询、排序和分配沿用原累计预算。该状态只证明布局组成表，后续 callable ABI、TD/dispatch、实际 selected-use、Strong V2 和 Compile/Link 双视图仍须全部关联后才能发布或消费；原 wire、版本及 runtime ABI 不变。

```text
ValueStorageLayout =
    ZeroSized { alignment: NonZeroPow2 }
  | NonZero { size: NonZeroU64, alignment: NonZeroPow2, scan: RefScan }

ArrayElementStorage =
    ZeroSized { alignment: NonZeroPow2 }
  | Inline { stride: NonZeroU64, alignment: NonZeroPow2, scan: RefScan }

FieldStorage =
    ElidedZst { exact, offset: ByteOffset, alignment: NonZeroPow2 }
  | Stored { exact, offset: ByteOffset, layout: NonZeroValueLayoutRef }
```

这些 sum具有封闭 checked constructor；raw wire不能直接实例化 `NonZeroPow2`、scan或 field ref。现有 `ValueStorageLayoutV1::Inline` 对应这里的 NonZero，内部命名可保留；新 wire只使用本节规定的 variant语义，不能直接序列化 Rust enum。

普通 struct/tuple按声明序计算：ZST field的canonical offset为0，不推进 cursor；nonzero field按其alignment对齐cursor，再checked加size；outer alignment取全部field最大alignment，最后checked tail padding。空ordinary struct与Unit是0/1；全ZST aggregate为0/max-alignment。class以完整base instance size作为新增field cursor，不复用base tail padding，继承field offset保持不变；object alignment至少8，并包含16-byte header。这是本阶段统一的class ABI冻结：替换当前flatten全部base/derived fields再布局的做法，本地和外部base都使用同一prefix算法。仅含Int8的base size为24时，derived首个nonzero字段最早从offset24开始，不能占用旧算法的offset17；新profile重建与layout golden同步迁移。

`ExactRepresentationLayoutV1` 穷尽 scalar、qualified pointer、struct、tuple、tagged enum、niche enum与intrinsic value family；class/interface/function等reference value使用managed qualified pointer分支。每个field/variant记录持久 typed field/variant identity、exact type与对应storage，source field sequence逐项匹配MIR。Instance的ClassObject保存 `NoBase | BasePrefix { exact, layout, byte_size, alignment }`，证明prefix和provider导出完全相等。

enum继续使用已有tag宽度与分配规则；不新增discriminant elision。tagged enum保存tag、pure-value共享区和每个ref-bearing variant独占连续slot；construction清零全部value/padding/inactive slots后再写active内容。niche只适用于规范7.4的封闭同构形状；managed-ref niche有managed scan，raw/code-pointer niche无managed scan。`Option<ZST>`保持非零tagged layout。

`@CLayout`只接受已通过source predicate的具体字段，按既有aligned/packed契约重放，并与canonical C layout逐字段一致。每个实际materialized的本地CLayout都必须由同一checked C storage算法生成canonical layout contract，即使该类型未出现在native call边界；该contract属于CLayout自身的表示依赖，不新增native callable、pass classifier或边界witness。不得从一般Scoop layout反推出C pass classifier。所有size、offset、stride、alignUp使用checked内部machine scalar并受target capability约束。

#### 6.2.1 完整 layout record 的 canonical 编码

`ExactLayoutExportV1`固定为field1～7依次保存`layout/exact/target/role/body/scan/definition`的product。body的tag1为`Value`，field1、2依次为本节的storage与representation；tag2为`Instance`，field1、2依次为checked shape与instance representation。layout与scan的id必须重算并逐项匹配foundation中的既有`LayoutKey/ScanKey`，不能只检查id存在。definition原样复用`StrongShapeDefinitionV1<PersistentLayoutId>`的既有map3：field1为`semantic_id`，field2为`definition_plan`，field3为`PersistentSymbolRequest`；semantic id必须等于record的layout id。provider、唯一primary atom及layout/scan各自的物理definition关系从同一foundation重建，保存在checked proof内，不扩展该旧product，也不据此授予import权限。

value representation的variant严格互斥，tag与payload固定如下；每项列出的payload从field1连续编码。

| tag | variant | payload |
| --- | --- | --- |
| 1 | Scalar | `kind` |
| 2 | QualifiedPointer | `kind` |
| 3 | Struct | `policy, interior_mutable, fields` |
| 4 | Tuple | `elements` |
| 5 | TaggedEnum | `tag_layout, pure_region, variants` |
| 6 | NicheEnum | `pointer_kind, variants, payload_variant` |
| 7 | IntrinsicValue | `family` |

Scalar kind的tag1为`Integer { signedness, width }`，tag2为无payload的`Boolean`；integer字段沿用4.1的既有tag。QualifiedPointer kind是无payload的sum，tag1～3为`Managed/Raw/Code`。String、class、interface、object、managed function及intrinsic array的value只使用Managed；`Ptr`只使用Raw，`FunPtr`只使用Code；完整pointee或signature沿record的同一exact key核验，不复制第二份可独立修改的identity或signature。IntrinsicValue family只有tag1的`Unit`，其storage固定为0/1；Integer、Boolean及各pointer family不能再通过IntrinsicValue编码。compiler-owned machine scalar没有source exact导出身份，不为它伪造一个Scalar分支；它仍可参与不产生独立persistent layout的内部geometry。

Struct policy的tag1为无payload的Ordinary，tag2为`CLayout { aligned, packed, canonical_c_layout }`，三个field依次保存5.1的closed CLayout value与既有canonical C layout fingerprint；reader逐字段比对该C layout，不把它当作一般Scoop representation的authority。`interior_mutable`只接受unsigned 0/1。nominal field product为`{ field: PersistentFieldId, storage: FieldStorage, access_alignment }`；field identity的canonical owner必须等于source/generated nominal owner，field exact从FieldStorage与被引用value layout逐项相等。字段保持MIR声明序，不能按id排序；owner、唯一性、GC事实与顺序都须在完整section的source join中核对。

Tuple element不是nominal declaration field，不派生或复用`PersistentFieldId`。它的product为`{ index: TupleElementIndexV1, storage: FieldStorage, access_alignment }`；typed index编码为unsigned，严格依次为0至元素数减1，element exact逐项等于该record的`ExactTypeKey::Tuple`。这一位置只在该tuple exact内有意义。tuple、generic nominal及pointer/function等structural表示的完整wire constituent不改变1.3的production gate。

TaggedEnum的`tag_layout`与`pure_region`均为`{ offset, byte_size, alignment }`。tag geometry从已验证target的既有enum tag layout重放，当前Darwin AArch64为0/8/8；这里不新增tag宽度规则。variant product为`{ variant: PersistentEnumVariantId, fields, slot }`，fields中的product为`{ field: PersistentEnumVariantFieldId, storage: FieldStorage, access_alignment }`，variant-field owner必须等于对应variant。slot的tag1为`SharedPure { byte_size, alignment }`，tag2为`Dedicated { offset, byte_size, alignment }`。GC-free variant只能使用SharedPure，其offset来自同一pure region；含managed leaf的variant只能使用独占Dedicated。pure region取全部GC-free variant的最大size及最大alignment，最大size本身不额外tail-pad；随后按语义variant顺序追加独占slot，最后才完成整个enum的tail padding。slot size/alignment、region、tag与field placements全部由同一次checked replay产生，reader拒绝任何独立矛盾的输入。

所有variant FieldStorage的Stored offset统一相对整个enum value起点，不能有的相对slot、有的相对object；ElidedZst无论variant位置均为canonical offset 0。tagged scan只组合独占slot中的managed leaves，并验证它们与整个value extent。NicheEnum的variants沿原语义顺序保存`{ variant, fields }`，fields复用上述typed variant-field product；必须恰有两个variant，一个无field，另一个恰有一个qualified pointer field，`payload_variant`必须精确指向后者。pointer kind逐项等于payload value layout的QualifiedPointer kind，payload field offset为0。niche与tagged由规范7.4的同一封闭判定重放，不能让producer在等价bytes之间任选编码。

InstanceRepresentation沿6.1的tag1～5，ClassObject的field1～3为`base_prefix/declared_fields/complete_fields`；base prefix的tag1为NoBase，tag2依次保存`exact/layout/byte_size/alignment`。BoxedPayload的field1、2为`payload_exact/value_layout`，InlineBytes无payload，InlineArray的field1、2为`element_exact/element_storage`，AbstractReference无payload。class fields使用nominal field product；complete fields只能从已验证base的完整projection与本class的declared fields机械拼接。box、array及class shape均从同一typed依赖重建并与body.shape逐字段相等；shape中的object-relative scan与record的scan role按6.1严格区分。

BoxedPayload只接受两种exact关系：value自身TD的ManagedObject layout与payload同exact，或该payload的既有`BoxedValue { payload }` generated exact；两者均引用同target的ManagedValue layout。前者保持runtime spec 2.2中包括Unit与其他ZST在内的value TD契约，后者保持有限shape-support helper身份。Unit的Value body只能是IntrinsicValue；这不排除其ManagedObject body为BoxedPayload。reference exact不能因为其value表示恰为一个pointer而选择BoxedPayload，完整section须与MIR的value/reference facts逐项join。

object沿既有HIR/MIR表示保留source object exact作为物理ManagedValue、ManagedObject与TD identity；`ObjectBackingClass { object }` exact只提供独立shape来源，不因此生成第二套物理定义。其ClassObject body从对应MIR ObjectBacking representation重放，declared field的owner必须是该key派生的backing nominal；普通class字段仍匹配自己的source nominal。完整section逐项验证Object→ObjectBacking key、base/fields与同一实际backing Class arena的关系，不能把两个exact合并或仅凭相同geometry接受其他object的backing。

单record的checked replay证明canonical identity、representation几何及所有scan范围，不替代完整section的HIR/MIR source join、export surface或selected dependency closure。Unit必须等于compiler固定的`CoreBuiltinNominal::Unit` exact，且该exact的Value body不能选择其他representation；Integer/Boolean/String/Array/MutableArray没有compiler固定的nominal identity，其kind/family必须由完整section与同一trusted-core receipt下的既有intrinsic binding逐项join，不能按名称或FQN重建identity，也不能仅凭任意nominal加family输入声称已经验证core authority。reader在解析typed identity、分配field/variant表与复制完整base projection前消费共享`BudgetMeter`；按by-value依赖与base active path拒绝cycle。完整record只持有closed storage/shape及已核验依赖，不通过裸size、默认offset、missing-key fallback或native-boundary witness补齐authority。

### 6.3 scan normal form 与预算

scan完全复用runtime spec 2.2，普通node的offset相对明确的base：

- References严格递增且无重复；Sequence flatten、删除None、合并同层References，并按 `(child fingerprint, canonical bytes)`排序去重。
- Array只能是 `{ length_offset, first_element_offset, nonzero_stride, nonempty_element }`，两个offset相对object base；GC-free/ZST element直接为None。
- tagged enum扫描所有独占ref-bearing slot，不读取tag；pure-value共享区不进入scan。
- box的inline scan相对payload，object scan通过checked offset平移；Array node平移length/first offset，不平移其element child。

五项限额固定复用：depth 64、distinct nodes 65536、distinct words 1048576、expanded nodes 1048576、canonical bytes 16777216。cycle检测使用active path；共享DAG的expanded cost按每条路径重计，用memoized checked subtree cost在展开前拒绝。producer正规化到fixed point，reader拒绝非canonical输入而非替它排序修补。

`ScanFingerprint`继续使用既有 `scoop-scan-v1` 与runtime canonical typed bytes，不换成Wire CBOR hash。layout/TD reader重算scan及所有offset边界，不能只验证digest长度或“scan id已存在”。

## 7. TypeDescriptor 与有限 shape-support

### 7.1 descriptor record

M23-6 的共有 Compile reader 从已核验 MIR 类型角色及继承关系、同一闭包已重放的 layout/dispatch 重建完整 TD 导出。ObjectBacking 仅供形状，其余实际类型各有一项 descriptor；class/object/String 使用实际基类，原始值类型、interface 与有限 coroutine helper 无 parent，box helper 按自身已验证基类关系处理。interface directory 精确来自已重放的 dispatch table 集合，按 interface exact id 严格递增；producer 在 LIR 封存前采用相同顺序，reader 不替候选数据排序或修补，表内 slot 顺序保持原 schema。parent/interface 引用从实际本地类型或可达依赖 descriptor 确定 Local/DependencyExternal 及 provider，不按 CORE、名称或同布局推断。value/instance layout、shape、object/inline scan、diagnostic name、definition 与 registration/fingerprint 均从共有 checked constituents、identity graph 和 foundation 重算；Strong semantic plan 必须使用真实完整的已重放 dispatch entries，不能构造虚假空表。原 producer 继续验证实际 registration plan，reader 的组成状态保留其余原始 wire，并在最终验证拒绝替换任何已检查表。该步骤不授予 selected-use、machine body 或对象资格；完整有序 TD wire、资源累计预算与后续 Strong/object/双 view 关联仍须全部通过。不改变 runtime C ABI、persistent identity、wire 字段或 capability kind；接口目录顺序通过既有 fingerprint 字段影响产物，并要求不满足 canonical 顺序的旧候选重建。

M23-6 的共有 Compile reader 从已核验的 MIR 类型角色与 dispatch schema、同一闭包已重放的 callable ABI/layout 重建 LIR dispatch 导出。除 ObjectBacking 仅供源码形状外，每个实际类型必须具有自身 vtable；普通值类型、interface 和 CoroutineStep/CoroutineSlot 的物理 vtable 按既有表示规则为空，class、object 与 String 使用原源码 class schema，BoxedValue 沿实际 payload 使用其完整 schema 与 boxing adjustment。只有具有实例 dispatch 的 class/object/String/BoxedValue 导出 schema 中的完整 interface table 集合；值类型的接口实现由对应 box helper 承载，不能从候选 TD/table 的存在与否反推集合。每条输入保留原 position、typed slot、完整调用签名、implementation kind 与 receiver adjustment，从唯一实际 provider 的 callable ABI 取得 body 引用；本地与外来 target 使用相同规则，不按 CORE 来源分支。共有 canonical 重放核对 target、签名、GC effect、receiver layout、foundation key 和 Strong definition；原 producer 在此结果上继续逐项核对实际 LIR slots 和物理 callable 引用。reader 按所有权保留其余 wire，完整有序 dispatch 表与重放结果必须一致，漏表、多表、重复、错序、目标或签名漂移均拒绝；后续验证也不能替换已检查的表。所有遍历、签名复制和集合操作计入原 artifact 预算。该组成表状态不代替 descriptor、selected-use、Strong registration 或机器对象关联，不修改 wire、capability 版本或 runtime ABI。

`ExactDescriptorExportV1` 保存 `{ exact, value_layout, instance_layout, shape, object_scan, ancestry, dispatch, diagnostic_name, definition, registration }`。两个layout引用分别指向该exact的ManagedValue与ManagedObject记录；shape/object_scan是跨record关系证明，必须与instance layout逐字段相等，不是可独立修改的第二authority。shape只接受下列checked sum；ancestry/table edge使用 typed external/local ref，不保存地址。

| shape | allocation/inline规则 | object scan |
| --- | --- | --- |
| FixedObject | 含header、完整fields和tail padding的非零exact allocation | 完整object-relative scan |
| BoxedValue | `inline_offset = alignUp(16, value alignment)`；minimum为`alignUp(offset + value size, max(8, alignment))` | inline scan checked平移 |
| InlineBytes | minimum/offset 24，instance alignment 8，element size/stride/alignment 1 | None |
| InlineArray | minimum/offset `alignUp(24, element alignment)`；ZeroSized与Inline分开 | 只有nonempty element scan才有Array node |
| AbstractRef | 所有instance/inline size、alignment、offset为0 | None，不可分配 |

`AllocatableTypeDescriptorRef`排除AbstractRef；box、array、class allocation再分别要求对应refined variant。运行时拒绝错误TD只是防御，正常LIR不可表达对AbstractRef分配。

abstract class仍有完整FixedObject instance布局供derived prefix和initializer使用，不等于interface/纯reference identity的AbstractRef。`ClassAllocationTarget`必须同时持有FixedObject shape proof与`ConcreteClass` modality proof；abstract class不能构造该target，但可提供`BaseInitializerTarget`。仅检查shape不是合法class construction证明。

`ExactDispatchExportV1`以`PersistentDispatchTableId`为主键，product固定为`{ table, owner_exact, role, entries, definition }`。role是`Vtable | Itable { interface_exact }`，必须匹配foundation DispatchTableKey；entries按物理position保存`{ position, slot, slot_signature, implementation, abi }`。implementation使用5.3的已验证target/adjust关系，abi引用同一local/external callable ABI export；abstract obligation只可出现在abstract owner允许的schema，物理entry必须指向同signature的既有trap target，concrete owner不得残留该分支。table及owner TD中的对应table ref、interface key、slot数和每项target逐字段相等；definition绑定同一Strong table atom。

`CanonicalExactTypeDiagnosticName`由已验证exact key重算，遵守总设计3.1的grammar、generated role和16 MiB checked展开预算。import/re-export/typealias拼写不参与。name bytes按值进入descriptor definition，关联只读atom由同一definition plan覆盖；不能让consumer替外部TD提供本地display string。

### 7.2 普通 Cone shape-support

M23-6 的共有 Compile reader 继续从已通过 HIR 声明关联的 MIR 有限 shape-support 源码根重建 LIR 八角色表。每个 source 的 canonical declaration key 从同一已验证 identity graph 按 typed id 查询，保留实际 provider；不扫描候选 LIR 表、foundation 清单或无关类型 arena 来补根。共有重放从已检查的 layout、TD 与 definition 重算 SourceNominal、ValueLayout、RefScan、TypeDescriptor、TypeRegistration、BoxedValue、CoroutineStep、CoroutineSlot，逐项验证 helper 的 generated nominal/variant/field identity、payload layout、ZST、GC scan 与 registration；helper 不再成为新的 source root。reference nominal 仅允许既有的无需装箱角色，value nominal 必须具有完整 box。完整有序 wire、provider、target 与八个角色必须精确一致，漏项、多项、重复、错序或换用其他类型的角色均拒绝。此步骤按所有权汇合五张已检查 LIR 导出表，保留原始 selected/physical transport；后续最终验证不得替换任何已检查表，Strong V2、实际使用、对象与双 view 关联仍须继续完成。所有源码 key 复制、索引、重放、排序、wire 比较与临时分配使用原 artifact 累计预算，不增加授权表、调用方 factory、wire 字段或 runtime ABI。

所有 producer 的 `ParamFreeShapeSupportExportV1` 共用 M23-3 八 role 的 closed product，语义和 field 顺序不变；provider 从实际 typed source 声明及同一产物的 MIR/LIR 组成表核对。core 与普通 Cone 使用同一完整表，不保留独立 core root/role 副本或空表豁免：

```text
SourceNominal / ValueLayout / RefScan / TypeDescriptor / TypeRegistration
BoxedValue / CoroutineStep / CoroutineSlot
```

source集合从已验证HIR public/inheritance接口的可跨Cone请求subject闭包独立重建，包括合法protected nested subject；不能从wire已有closure反向枚举“应有全集”。纯re-export不产生新的source obligation。

前五项和Step/Slot总是Available。BoxedValue对value为Available，对reference nominal只允许既有 `ReferenceNominalRequiresNoBox`。不能增加泛化的Unavailable/Unsupported reason来掩盖缺项。

Step为 `Completed(T) | Suspended`，Slot为 `Empty | Value(T)`；使用既有generated nominal/variant/field key、完整gc flag和tagged/niche规则。每个generated role包含自身layout、scan、TD、registration和definition proof；这些helper不能再次作为source root触发无限 `Box<Step<Slot<...>>>` 展开。

owner严格由 `ExactOwnerRoot(subject)`决定：source nominal回定义Cone，application/structural回ODR。consumer请求source-root helper时仅导入provider definition；缺失closure直接使artifact无效，不能本地补Strong或伪造Structural组。

`ContinuationShell`与`CoroutineStart`不在本closure中；它们依赖`Continuation<R>`/`SuspendTask<R>`，到M23-7完整ODR proof后才加入。box/interface语义需要的adjust thunk按MIR dispatch relation独立闭合，不把“非callable shape-support”当作免验证生成任意body的入口。

## 8. Scoop typed ABI

### 8.1 canonical contract

共有 Compile reader 在五张 LIR 导出组成表通过验证后，从已完成 HIR 来源与调用位置校验的普通 MIR bridge、同一 HIR callable 的源码 GC effect，以及实际可达依赖已重放的普通 LIR bridge 重建完整普通调用表。所有普通签名统一通过既有共有 nominal 表示查询及 canonical Scoop ABI 算法重算 receiver、声明序参数、结果、GC effect、Cdecl 与 caller root plan；同一 artifact 只建立一次有预算的临时表示索引，不制造 native witness 或新的类型导出根。已有 ManagedValue layout 的签名位置还须逐项匹配相同 target 的 canonical ABI；普通 bridge 原已允许的 source-only nominal 签名仍由共有声明独立重放，不因缺少 M23-6 的完整类型布局导出而丢失，也不因此取得 type-bridge、TD、构造或泛型物化能力。实际 Strong body、symbol、definition 和 primary atom 继续按同一 provider 校验；外部选择逐项匹配 MIR 的 provider、typed declaration、implementation 与 exact signature，再借用该 provider 的完整 canonical ABI。候选 LIR exports/selected 只参加完整有序 wire 比较，不用于发现源码根、补齐 provider 或生成资格；缺失、额外、重复、乱序、不可达 provider、表示缺失或冲突、签名及 ABI 漂移均拒绝。查询、克隆、排序与 canonical 编码使用原 artifact 的累计预算；已核验的普通 bridge 按所有权进入下一状态，剩余 type selected-use、初始化角色与 Strong V2 仍须分别闭合。复用既有两字段普通 bridge 与六字段 callable ABI 格式，不增加 wire、source factory、授权表或 CORE 例外。

共有 Compile reader 随后从已完成 HIR 协议关联的 MIR InitializationCycle 角色重放初始化服务 ABI。有无记录仅由实际角色决定；有角色时使用该 typed function 的完整 ordinary、无 receiver、String 参数与 Unit 结果签名，从已重放的本地及可达依赖 ManagedValue 布局计算完整 canonical ABI，并固定为语言协议要求的 Managed、Cdecl 和 managed caller root plan。callable body、symbol、definition 与唯一 primary atom 共用普通 callable 的物理验证；不得从待验 ABI、CORE 坐标或 symbol 名补目标或布局。Strong V2 的 field 11 在同一累计预算内与完整预期 0/1 array 精确比较，缺失、额外、错误目标、签名、GC/root plan 或 definition 均拒绝；通过后将 owned canonical ABI 保留在后续 Strong reader 状态中，其余字段仍须完成完整 section、registration 与 layout/selected 闭包验证。本步骤不增加 wire 字段、版本或源码资格载体，不改变 runtime ABI。

Strong V2 reader 的 digest 图由同一 artifact 的实际 LIR foundation、已校验的八字段 runtime registration 语义和源码 entry 角色重建，不接收调用方另给的预期 digest 图。待验节点先只向该 foundation 的已有 typed owner 解析，验证 identity、顺序、边、patch 与无环关系；该临时结果用于 registration 的相互引用校验，不成为最终图的依据。随后 producer 与 reader 共用相同的按角色投影算法，从 callable、safepoint、type descriptor/layout、immortal、static storage/scan、初始化 schedule、entry 和 image 逐项生成完整节点、直接输入和 patch 集合，并精确比较全图及十字段 Strong section。缺失、额外或替换的合法节点/边/patch 不能因仍然无环而被接受；external 初始化依赖不新增本地 digest 输入。类型与 callable 的源码/selected join 及实际 object 校验继续由完整闭包执行。解析、临时存储、哈希、排序、图遍历和比较使用同一累计预算；不增加 wire、来源证明或独立预期表。

共有 Compile reader 在布局、普通调用表和初始化服务 ABI 验证后，按同一 manifest 的依赖优先顺序继续重放完整 Strong V2 section。entry 来源取已关联的 MIR 输出协议，有限 shape-support 根取共有 HIR 公共与必要支持声明；依赖 descriptor/callable 的物理定义和初始化单元只向真实可达 provider 的同一 foundation 与已重放 registration 查询。外部 bridge 先按原 identity graph 解析，并逐项核对实际 provider 的定义、symbol 及完整 callable ABI；这一步只解析和验证物理合同，不以候选 bridge、registration 或物理清单生成 HIR selected 根。初始化的本地依赖定义从本产物已校验的 registration identity、foundation 和待重放 digest 图解析，外部目录只能携带其他 provider 的完整定义，不能由调用方替代本地定义；随后继续完成全部 registration 字段、canonical digest DAG 和十字段 section 的精确重放。无关私有物理定义同样重放，但不扩大导出或源码根。已检查的布局、普通 bridge、初始化 ABI 和完整 Strong 重放结果按所有权进入下一状态，不复制或重新解码原始 section，不公开 wire encoder 或发布转换；实际 selected-use、完整 layout join、object、Link 和最终双 view 校验仍是发布条件。所有目录构建、来源 key 复制、身份解析、克隆、排序和比较沿用原 artifact 的累计预算，不接收调用方 source factory、预期 digest 表或新的来源授权记录，不增加 wire、版本或 runtime ABI。

完整 Strong V2 重放的预算按实际算法的表关系计量：digest owner 只查询其对应的 foundation 表，patch 只查询 definition/atom，输入边使用节点索引；definition/symbol 组装、有限 shape 查询和各类 registration 分别计算实际索引、线性扫描与分配成本。不能把所有互不参与该操作的表相加后再作整体平方，也不能把全部 definition plan 再当作每类 registration 的数量。类型及初始化的局部重放继续覆盖自身的语义递归、物理字段和最终计划构建，公共索引、错误路径与克隆同样计入累计预算；默认资源上限不变。真实原始 core 与组合产物必须在该默认预算内通过，共有校验继续拒绝零预算和累计耗尽。

codegen 的 Scoop ABI 防御校验按共有存储规则重放普通 struct/tuple：零尺寸字段保留逻辑字段身份与对齐，canonical 字段偏移固定为 0，不对当前非零存储 cursor 插入 padding，也不贡献 GC scan；非零字段仍按实际 cursor 对齐并逐项核对偏移、access alignment 与引用位置，aggregate 最终大小继续按最大字段对齐取整。CLayout 字段仍遵守自身合同的物理布局规则。不得将引用或非零字段后的 ZST 当作具有后继物理偏移的字段，也不得通过跳过整个 aggregate 的布局/scan 校验来接受该情况；错误的 ZST 偏移或对齐、非零字段偏移、总大小与 scan 均拒绝。此修正不改变既有布局、wire、身份或 runtime ABI，只使后端校验与已定义的 ElidedZst 规则一致。

M23-6 的共有 Compile reader 从已完成 HIR 来源关联的 MIR callable binding 集合及已重放的本地/可达依赖 layout 独立重算完整 callable ABI 表。每项按实际 implementation 与 lowered exact signature 保留 receiver、声明序和重复参数、Unit result 的 layout 引用、GC effect，并复用 canonical Scoop ABI 算法决定 ZST、direct/indirect 参数和返回方式；公开源码函数、构造器、accessor、trap 与有限 generated callable 使用相同机制。每个位置只接受该 exact 的唯一同 target ManagedValue layout，缺失、重复 provider、错 target 或 layout 角色均拒绝，不按 CORE 来源跳过。body、definition、primary atom 与 symbol 从同一 LIR foundation 的 typed target 取得，wire 的完整六字段及整个有序集合须与重放结果逐项相等，不能以候选 callable 或 Strong registration 补充来源根。布局与 ABI 的中间状态按所有权保留其余原 wire，后续验证必须继续使用相同的已检查表，不能换入另一份预期记录。此状态只证明布局及 ABI 组成，TD/dispatch、实际 selected-use、Strong V2 registration、机器对象和最终双 view 仍须完整关联；读取全过程沿用原 artifact 的累计预算，wire、capability 版本、persistent identity 与 runtime ABI 均不改变。

layout profile 的 LIR 在封存前使用已验证 HIR/MIR identity graph 与 Cone coordinates，为全部实际 descriptor 生成 canonical diagnostic name；registration、导出表与 object bytes 消费同一实际名称。导出重放只能比较，不能在 producer 已封存后改名或接受 arena 显示名。查询与名称分配使用同一预算，关系或 coordinate 缺失即失败。

source exact 在 LocalConcrete → MIR 转置时保留实际 nominal provider，application 则保留匹配的 specialization record，结构类型使用独立归属分支。协议导入的 nominal provider 同样来自其已验证声明 key，不由协议发布方、consumer 或 CORE 常量推断。只有 provider 为当前 Cone 的 nominal 才进入本地 Strong shape 根；外部值可用于表示计算和签名，但只能引用定义方的 layout/descriptor。此关系与 exact key 一起完整校验，不以非泛型 nominal 默认本地所有。

LIR producer 从同一次 sealed MIR/LIR、完整 MIR export 组成表及实际 Strong V2 registration 组装五张 export 表。布局和 descriptor 覆盖 MIR source/support/helper 导出闭包中的实际物理定义；callable 的 lowered signature 按 exact type 查询唯一的本地或依赖 ManagedValue layout，receiver、重复参数与 Unit result 均保留。dispatch 将实际 LIR table 的物理 callable 与 MIR 的声明序 schema 逐项 join；BoxedValue 沿 typed payload 关系使用源码 value schema，CoroutineStep/CoroutineSlot 的无成员关系只允许实际空表，不从任意空候选表补默认实现。依赖只借用，五表使用同一 target、provider 与预算；完整 section 的 source/selected-use 和最终产物验证继续执行，不以 export 组装代替发布闭包。

完整 LIR section 的实际 source 适配器从同次 sealed MIR/LIR 与 Strong V2 registration 重新投影五张预期 export 表，按实际跨阶段 source-exact 引用取得外来 ManagedValue layout；lookup 必须唯一且属于该 exact 的实际 provider。额外 descriptor 与新 callable 用途取实际 LIR external arena，初始化 descriptor 用途取同次 registration 中已验证的外部单元依赖。物理 import 必须精确覆盖这些实际用途，并逐项匹配 provider、typed subject、symbol 与 definition；完整 section 继续核对 terminal contract。既有 String 角色和旧 callable 分区按明确的 LIR 引用及 origin 区分，不能按 CORE 身份分支；它们继续经过共有旧 bridge 校验，不重复加入物理 import。投影、比较、去重、排序及完整 section 重放共用预算，不新增 wire 或独立来源授权。

当前 provider 拥有的语言内建 Unit/Any 必须在 LocalConcrete HIR sealing 前显式保留，并在 HIR → MIR 转置时作为必需类型根沿共有 source-exact 路径处理；不能依赖额外源码引用才获得完整导出。该必需集合依据内建 typed declaration 的真实 origin 判定，外来 consumer 仍只为实际使用引入依赖引用。验收包括不添加类型使用的原始 core 源码基线，完整 MIR/LIR section、Strong V2 registration 和 LLVM 对象发射均须闭合。

M23-6 的 Cone image dependency 表使用当前 artifact 的完整直接依赖集合，包含显式与前端发现后纳入 manifest 的普通 provider，按 ConeIdentity 严格排序；生产入口必须显式传入该集合，不能从当前 Cone 是否为 CORE 推断空表或固定 core 单项。构造与读取拒绝 self-dependency、重复、缺失及多余依赖；Compile reader 从已验证 manifest 重建预期 image plan，Link reader 与生产 Code projection 逐项关联同一集合，再验证实际 object bytes、依赖 atom、RuntimeImage 与 Code fingerprint。仅因运行时尚未执行多 image startup，不能丢弃已经声明的依赖元数据。既有 image wire field、identity、runtime C ABI 与 fingerprint 编码不变，新的实际依赖通过已有字段进入 fingerprint；该修正不提前开放 M23-8 startup、M23-9 program-link 或后续 ODR 能力。

layout profile 的对象组装直接消费同次 V2 LLVM 发射结果，必须在释放临时对象目录前取得完整成员 bytes；成员绑定、C bridge envelope、relocation、digest patch、stackmap 与六类 registration 沿现有共有校验执行，type 和 initialization 使用 V2 引用语义。完整 undefined-use 分区先闭合，再计算 callable/descriptor/registration、runtime image 和 Code fingerprint，最终由既有 layout writer 组装 archive。已验证的 V2 production 结果只能按所有权传递，不能复制、降级或替换为未经 layout join 的 section。对象组装结果本身不构成 Compile/Link 发布凭证；最终发布继续要求从这些 bytes 完整重放两个视图与依赖闭包。

无关私有类型的本地 TD/layout/dispatch/body 继续进入完整 Strong production、registration、object 和 fingerprint 验证，不能仅为导出五表而扩大 HIR source roots。私有类型被公开字段、基类、签名或有限 helper 的传递闭包引用时，仍须完整导出；边界由 typed 关系决定，不能按 visibility 删除必需记录。有限 shape-support 计划的源码根及其 helper 的 MIR type 与实际物理定义必须齐全，ObjectBacking 保持既有 shape-only 关系。dispatch constituent 逐项验证 foundation/definition，完整 section 要求导出 TD 所引用的 vtable/itable 与 dispatch export 精确覆盖；Strong V2 join 逐项验证 export 的实际 registration，允许本地生产清单包含导出闭包外的完整私有定义。

HIR 来源根发现必须遍历已要求 nominal 的全部真实存储字段，包括 struct 字段、enum payload、class 字段和 object backing 字段，保持字段类型的完整 nominal/application、tuple、function 与 pointer 组成关系。被这些字段引用的本地声明进入同一 source/support 闭包，外来声明继续由实际 provider 提供；字段可见性不影响表示依赖。遍历使用同一预算并按 typed type/owner 去重，既不扫描函数正文扩充根，也不展开无关私有 sibling。generic 声明及其字段引用只产生源码依赖，实际 generic/structural 物化继续受原 ODR gate 约束。

`ExactCallableAbiExportV1` 保存 `{ target, canonical_signature, calling_convention, call_protocol, layout_dependencies, definition }`，按此顺序使用field1～6的product。target为既有`StrongCallableDefinitionOwner`，definition复用`StrongShapeDefinitionV1<PersistentCallableBodyId>`的三字段product；body id必须从`CallableBodyKey::strong(target)`重算，并与同provider的physical definition/唯一primary atom相等。calling_convention原样复用LIR的既有`Cdecl`编码。`call_protocol`是无payload的closed sum，tag1、2分别为`OrdinaryManaged/OrdinaryNoGc`，逐项匹配canonical signature的GcEffect。

本Strong callable export的目标拥有Scoop body；既有`NativeSafe/NativeBorrowed`继续只由native contract与相应callsite承载，不能给本export伪造native分支或Strong extern definition。Scoop extern的NoGc不等于本表的OrdinaryNoGc，直接source extern production gate不变。

canonical signature原样复用M23-2的 `CanonicalScoopAbiFunctionSignature`：field 1 exact signature、field 2 logical arguments、field 3 result、field 4 GcEffect；storage仍是exact type/byte size/alignment/scalar-or-aggregate的既有product。参数tag为ElidedZst=1、Direct=2、Indirect=3；result为UnitVoid=1、ElidedZst=2、Direct=3、Indirect=4。

本section增加的是每个storage的完整layout/scan来源和可调用definition证明，不增加另一套extern signature编码。`layout_dependencies`是field1～3的product：`receiver, parameters, result`。receiver为tag1的无payload`NoReceiver`或tag2的`Layout`（field1为`PersistentLayoutId`）；parameters为保持声明顺序的layout id序列，result为单个layout id，即使UnitVoid也必须保存Unit的layout引用。所有位置均引用同target的ManagedValue记录，逐项与exact signature及storage相等；重复type保留重复位置，不能改成去重集合。receiver在source logical signature中独立保存，降低时按既有规则作为第一个logical input；不能静默遗漏。

当前Darwin/AArch64 classifier：scalar、qualified pointer、niche enum为Direct；非ZST tuple/ordinary struct/tagged enum/exception record为Indirect；ZST input为ElidedZst；Unit result为UnitVoid，其他ZST result为ElidedZst。不得按aggregate大小或system C classifier另选pass mode。

validated LLVM backend profile 的 frame-pointer=all 策略适用于全部 Scoop callable，包括 NoGc 函数；GC strategy 的设置仍由 typed GC effect 独立决定，NoGc 不得因 frame 策略而加入 statepoint GC。LLVM producer 与 rewritten-module verifier 必须同时应用和核对该 frame 策略，使 NoGc 次构造器等包含普通调用的函数保持 canonical frame，避免因遗漏 profile 属性而产生预定 associated-atom 闭包外的 unwind-only EH frame。managed 的 tail-call 与精确根规则继续按既有 profile 执行；associated section、CIE/FDE、compact unwind、LSDA 和对象范围的完整校验仍然必需，不能通过忽略额外 backend section 接受不完整定义。

### 8.2 physical signature与调用

BoxingAdjust 的 MIR receiver、对应 local 与正文必须保留实际接口类型，和既有 exact callable signature 逐项相同。不得通过 Any 擦除再豁免 nominal signature 校验；接口 receiver 的物理表示仍由共有 managed-reference ABI 处理。

physical参数顺序只计算一次：若result indirect，首参数为result storage；随后按logical顺序跳过ZST、发出direct value或indirect pointer。每个indirect参数有fresh exact caller storage，callee遵循按值语义。

LLVM definition、call、invoke、dispatch和Scoop extern在同一physical index使用 `byval(exact LLVM type) align N`；indirect result使用 `sret(exact LLVM type) align N`。显式statepoint wrapper把callee参数attribute平移到intrinsic参数 `5 + i`。codegen从同一checked signature计算，禁止维护第二张可独立修改的physical表。

全部ZST实参仍按源码顺序求值。callee只在观察参数地址时分配token；用户ZST result产生typed logical value，不借Unit sentinel丢失exact type。两个source callable物理签名相同也不能共享identity、symbol、override slot或ABI fingerprint。

### 8.3 GC与异常边界

含ref aggregate按已验证scan和typed storage拆为AS1 leaf，不用byte array擦除provenance。ordinary managed call/invoke复用M15 root plan：invoke前root frame同时覆盖normal/unwind存活leaf及可移动实参，两个后继reload并pop，不产生exceptional gc.relocate。

Indirect参数/结果storage、value receiver copy、外部field内联ref与dispatch receiver都参与同一活跃性/root plan。post-statepoint只使用relocated/reloaded值，不能从旧indirect temp缓存ref。

普通NoGc与native Scoop NoGc不是一种callsite。Scoop extern无论GcEffect值均保持NativeBorrowed/caller-root publication；C bridge继续NativeSafe。effect轴不改变ordinary/suspend signature identity，也不使direct source-extern能力提前开放。

## 9. ZST place、boxing 与 static storage

### 9.1 logical value与place

LIR区分 `LogicalZstValue { exact }`、`AddressableZstPlace { place, exact, alignment, lifetime }` 和nonzero storage。token需求在MIR保留、LIR定稿；codegen不由LLVM store size为0反向发明place。

parameter/local/value `this`真正取址时分配non-null、至少1-byte、满足alignment的token。有效期重叠的不同semantic place不得共址；重复取同place地址稳定。token不能标成可合并的 `unnamed_addr` 常量，也不能通过共享零地址/singleton实现；不重叠lifetime允许复用。普通SSA ZST、field和array element不自动取得token。

class 字段与 closure capture 的 ZST payload 读写在 MIR→LIR 阶段消除。lowering 先按既有顺序完整求值 receiver、capture initializer 和 assignment RHS，再使用同一 storage 分类区分零大小与非零存储；零大小读取显式产生携带 exact type 与 AbiZst 的 LogicalZstValue，零大小写入在保留求值后结束。不得向 codegen 传递 offset 0 的伪 payload HeapLoad/HeapStore，也不能让 codegen 根据空 LLVM aggregate 猜测语义。closure invoke pointer、非零字段和 compiler-owned state 继续使用各自已验证的偏移与指令；ZST 不改变对象头、继承 prefix、GC scan 或字段身份。

### 9.2 box/unbox执行路径

```text
BoxPayload = ZeroSized | NonZero { source_place }
UnboxResult = ZeroSized | NonZero { destination_place }

scoop_rt_box_zst(td)
scoop_rt_box_value(td, source_place)
scoop_rt_unbox_zst(object, expected_td)
scoop_rt_unbox_value(object, expected_td, destination_place)
```

接口严格按runtime spec 2.3：size/alignment/inline_offset/scan只从TD读取，删除旧 `box(td, payload, size, scan)` 和固定`+16`路径。ZST入口没有payload/result pointer；box仍分配TD规定的非零managed object并取得fresh ref identity，unbox先检查exact TD再产生logical value。

nonzero source place须地址稳定、对齐且在call前已写入完整值。inline scan非空时caller先经compiler-private NoGc leaf `PushRecursiveRegion`登记该temp，再进入box runtime和可能park的managed-entry handshake；root保持到分配、从collector更新后的同一temp复制及返回完成后才LIFO pop。所有非fatal出口配对；runtime只验证root已活跃，不在入口后补登记。空scan可以省略frame。

box payload不能作为可观察的value `this`存储暴露；adjust thunk初始化独立方法局部值，ZST需要地址时另建token。moving GC依靠完整object scan与精确side-metadata size，不依赖payload非零。

### 9.3 static token与初值

compiler-managed ZST storage保留persistent storage/unit identity，logical `byte_size=0`、allocation extent=1、scan None、canonical token byte=0。两种静态初态仍严格区分：有runtime unit的storage/failure/published root使用ZeroedForRuntimeUnit；无unit的静态值使用EncodedStaticValue，即使bits全零也不改tag。

EncodedStaticValue的template恰覆盖allocation extent，padding/pointer leaf初始归零，immortal relocation按offset排序且只指向已登记immutable object-start。ZST token没有relocation，None scan使用既有static sentinel。ordinary property不因有token开放`addressOf`；本地raw global/TLS遵守原GC-free和lvalue规则，C-boundary ZST storage仍拒绝。

本阶段immortal仍限于既有String表示，`ImmortalObjectTypeRegistrationRefV1`保持Local/CoreExternal范围；imported const String继续按Stage5在consumer生成自己的literal/immortal，不引用provider immortal地址。`StaticImmortalRelocationPlanV1`继续只解析本Cone immortal producer表，不能因通用layout API已存在便扩大为任意foreign immortal relocation。

## 10. Array、pointer 与 C 边界

### 10.1 ZST array

array type仍由core generic nominal提供identity，typed `ArrayTypeId`非可选地携带exact owner、mutable/immutable kind和element storage。offset16的count是内部u64 machine metadata，源码size/index是Long；logical count必须在`0..=INT64_MAX`。

- `data_offset = alignUp(24, element alignment)`，ZST allocation恰为该offset，与count无关；Inline allocation按checked `alignUp(data_offset + count * stride, instance alignment)`。
- get按receiver→index求值，再检查bounds，成功产生exact ZST；set按receiver→index→RHS求值，再检查bounds，成功不写payload。越界不能跳过RHS。
- literal/assembly/spread/vararg保留每个part的求值与checked计数；不会因为element size为0删除producer或把length溢出隐藏成小allocation。
- clone/互转验证source exact array TD、physical count与side metadata一致，再分配fresh target并复制logical size；ZST不发payload memcpy或write barrier。
- iterator固定保存array ref和Long index，比较`index < size`、每次加1；禁止pointer-end/stride-progress实现。
- GC-free/ZST element直接None scan，collector工作量不随logical count增长；含ref Inline使用真实data offset和nonzero stride的Array scan。

String、Inline array的所有乘加/alignUp同时检查u64、target size_t和maximum_managed_object_size。非法内部count/layout或溢出沿既有fatal invariant/allocation路径，不按名称虚构IllegalArgumentException；不新增length-based源码constructor。

### 10.2 pointer

unsafe `Ptr<ZST>` plus/minus/load/store offset的byte displacement恒为0，pointer bits不变；receiver、offset、value仍求值。load/store不访问payload但继续要求non-null/alignment/lifetime和合法逻辑place。`Ptr<Unit>`是opaque void pointer，逐byte arithmetic必须显式使用`Ptr<UInt8>`。

LIR lowering 在全部操作数按源码顺序求值后，按共有存储分类处理 pointee：ZST load 产生携带 exact identity 与 `AbiZst` 的显式 `MakeZstValue`，store 只保留值的求值，偏移直接复用原 pointer。非零 raw load/store 必须携带完整 `AbiValue`，与读写值类型一致；`PtrOffset` 的步长为 `NonZeroU64`，不能表达零步长。codegen 重放这些存储合同后发射操作，不从空 LLVM aggregate 推断或补造 ZST 语义。本条不增加运行时有效性检查或放宽 unsafe 调用者的有效地址责任。

本节是通用表示规则；Ptr exact type和相关generic callable的生产物化仍受1.3 ODR gate约束。不得以“pointer只占8 bytes”为理由本地Strong发TD或绕过specialization。

### 10.3 C ABI source规则

HIR在source边界统一检查，不能推迟到LIR/generated C/native linker：

| 边界 | 规则 |
| --- | --- |
| Unit result | 唯一void例外 |
| ZST by-value parameter/result/callback | 拒绝，包括Unit参数 |
| extern global/TLS ZST | 拒绝 |
| 空CLayout或具体ZST字段 | 拒绝 |
| `Ptr<Unit>`、`Option<Ptr<Unit>>` | opaque data pointer例外 |
| `Ptr<其他ZST>`及其Option | 拒绝C pointee |

generic CLayout的binder-dependent字段保留 `CFieldSafeAndNonZst { signature_type, declaration_field_path }` predicate，条件进入template fingerprint。无字段或与binder无关的非法字段在定义处报错；concretization检查在M23-7接入生产，本阶段在typed constituent测试锁定。不能把待替换条件提前编码为true，或在consumer丢弃字段路径。

C端真实寄存器/aggregate lowering继续由validated generated-C toolchain完成。M23-2的canonical C storage/layout、extern contract与callback bytes保持；general layout proof只能与它们交叉验证，不能反向扩充旧native witness的授权范围。

## 11. closure 验证、Link proof 与 fingerprint

### 11.1 原子验证顺序

```text
envelope/profile/target/resource checks
  -> foundation identities + direct/support role
  -> HIR shared declaration metadata / type facts / access / inheritance / actual selected-use closure
  -> MIR source-to-implementation / slot / helper relations
  -> LIR layout replay / scan normal form / ABI / TD
  -> complete source-root support obligation
  -> selected request closure and external arena commit
  -> final object verification + Link-only use closure
  -> Compile/Link equality + publish
```

继承环和by-value representation环拒绝；通过managed reference的递归class合法，layout重放在reference leaf停止，不沿对象图无限展开。所有provider来自同一target-compatible显式artifact closure。prebuilt/cache验证只能读取该closure内的required section，不得要求原始source、compiler进程内token或调用方注入authority factory。记录存在、id匹配或digest相同都不能替代逐字段关系证明。

普通 callable source-interface 的读取阶段也必须验证每份默认正文的 definite assignment、局部变量可变性、循环控制与完整 binding plan/action 顺序。struct pattern 的字段序号从当前声明及显式依赖闭包中已验证的共有 nominal metadata 取得，并核对 typed field owner 和 generic arity；不能信任正文自报的字段顺序、按名称恢复字段或使用 core 专用来源入口。字段查询索引与递归正文遍历使用同一 artifact 的资源预算，失败不能进入后续 const、bridge 或 identity commit 阶段。此项与完整默认值的 provider、类型、effect、nested callable 和引用域校验共同组成完整入口，不代替其他检查。

错误前不发布partial world、arena、artifact或cache entry。MIR/LIR自身不报告新的源码visibility/overload错误；不完整selected集合属于compiler/artifact invariant。

### 11.2 semantic dependency与physical use分开

编译只读取外部field offset、size或scan事实时，可以没有对provider layout constant的object relocation；metadata-only TD authority也不伪造地址使用。反之call、TD address、parent/interface table、dispatch target、registration与object ensure的实际relocation必须逐条验证。

`SelectedDependencyLayoutAbiSetV1`按6.1的field1/2完整保留semantic use与physical import；新Link section只保存后者。其`semantic_imports`必须与Compile selected field2逐byte相等，不得从requirements或symbol表反推、补齐或裁剪：

```text
CrossConeLayoutLinkClosureSectionV1 {
    semantic_imports: CanonicalVec<ExternalShapeLinkImportV1>,
    requirements: CanonicalVec<ExternalShapeUndefinedUseV1>,
    object_coverage: ExternalShapeObjectCoverageV1,
}

ExternalShapeLinkImportV1 {
    provider: ConeIdentity,
    subject: ExternalStrongShapeSubjectV1,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
    contract: ShapeLinkContractV1,
}
```

`ExternalStrongShapeSubjectV1` 精确采用下表，tag 是本新增sum的编号，不修改被引用identity的kind/tag：

| tag | variant | payload |
| --- | --- | --- |
| 1 | Callable | `StrongCallableDefinitionOwner` |
| 2 | Layout | `PersistentLayoutId` |
| 3 | Scan | `PersistentScanId` |
| 4 | TypeDescriptor | `PersistentExactTypeId` |
| 5 | DispatchTable | `PersistentDispatchTableId` |
| 6 | TypeRegistration | `PersistentExactTypeId` |
| 7 | StaticStorage | `PersistentStaticStorageId` |
| 8 | StaticStorageRegistration | `PersistentStaticStorageId` |
| 9 | InitializationCell | `PersistentInitializationUnitId` |
| 10 | InitializationDescriptor | `PersistentInitializationUnitId` |

每项wire是 `{ 0: tag, 1: payload }`，定义和symbol role从variant唯一派生。7～10只可由provider已导出的object-value/initialization support relation选择；不得通过它们枚举私有storage，也不能引用任意其他unit的cell/failure root。ordinary property access依然只使用accessor，不以此公开backing storage。已经由 strong production 外部引用或 ordinary callable selection 记录的 subject，按实际 capability/subject 从共有依赖索引验证，不能在 layout 接口重复认领。foreign immortal不是本sum的variant，其现有String/constant路径遵守9.3。

`ShapeLinkContractV1`精确分为七个variant：`CallableAbi { canonical_signature, calling_convention, protocol }`、`Layout { record }`、`Scan { layout, role, canonical_scan }`、`Type { descriptor_projection }`、`Dispatch { table_projection }`、`StaticStorage { storage_projection }`、`Initialization { unit_projection }`，tag按此顺序为1～7。subject1～5分别只能匹配contract1～5；subject6复用Type、7/8复用StaticStorage、9/10复用Initialization。

Layout/Type/Dispatch分别复用6.1/7.1的canonical semantic record，去掉definition/registration的后置digest槽；Scan保存完整scan tree而非仅digest；storage/unit projection逐字段复用strong-production/6对应semantic plan，不包含member/range或后置object/registration digest。它们不是任意bytes，reader按subject取得provider同一plan并逐字段比较。required definition由subject和owner重算，provider必须真实Strong定义它，consumer defined-symbol set必须不包含它。

新projection保留原字段编号：Layout恰为exact-layout record的fields1～6，Type恰为exact-descriptor的fields1～8，Dispatch恰为exact-dispatch的fields1～4；三者分别排除field7、fields9～10与field5的definition/registration后置槽。StaticStorage恰为strong-production static record的fields1～10（storage、symbol、layout、scan id、完整scan、scan kind、logical size、allocation extent、alignment、initial state）；Initialization恰为unit record的fields1～8（unit、diagnostic path、schedule、storage、failure root、initializer、ensure、ordered dependencies）。这些语义字段均不含后置digest，已有initial-state、schedule及V2 dependency的wire保持不变；typed dependency proof仍在provider同一strong-production/6计划中保存，不能由wire unit id重造。每个projection复用完整record的同一字段编码和逐字段重放，只改变新product的字段数，不改变旧完整record bytes。

七种contract的新wire均使用field0保存tag：CallableAbi为`{0:1,1:canonical_signature,2:calling_convention,3:protocol}`，Scan为`{0:3,1:layout,2:role,3:canonical_scan}`，其余五种为`{0:tag,1:对应typed projection}`。import的五字段依次为provider、subject、expected symbol、required definition、contract；canonical数组按provider与subject的组合canonical bytes严格递增，不合并重复项。provider查询先复用subject的唯一Strong definition/primary/symbol规则，再与同provider的实际Strong production及完整semantic records核对。storage/unit查询协议只返回候选semantic plan：实现须由完整section封闭持有，先证明导出的object-value/initialization support关系，import重放再将候选与provider实际unit及其storage/failure root逐字段join；候选本身不授予import或selected资格。完整section仍须将已构造import的五类一般contract重新绑定到同一terminal section的实际ABI/layout/scan/TD/dispatch表，不能凭provider与subject相等接受来自另一份同ID语义表的record。

requirements沿用M23-5 canonical relocation-use结构和排序，并引用本section import index；每个physical import至少一个use，每个actual relocation恰有一项。object coverage绑定全部最终LinkObject成员集合及canonical use set，digest使用 `DomainSeparatedCborHash("scoop-cross-cone-layout-object-coverage-v1", { verified_link_objects, relocation_uses })`，仅作LinkValidationOnly。

本section的三字段编号依次为semantic_imports、requirements、object_coverage。每个requirement为`{1:canonical_relocation_use,2:import_index_u32}`，严格按既有`(member, containing_atom, offset_within_atom, target_slot)`排序且唯一；只从共有 strong dependency 分类后的 remainder 中按 target 规范化后的 expected symbol 匹配，已由其他 capability/subject 认领的 symbol 不得再次认领。未匹配的native/runtime候选留给后续既有分类，不得丢弃。coverage为`{1:verified_link_objects,2:relocation_use_set_digest}`，其hash preimage为`{1:verified_link_objects,2:canonical_relocation_uses}`，uses保留完整十字段而不含import index。最终对象证明必须与分类输入的同一verified relocation closure逐成员内容和完整binding join，不能仅以producer或member id相等替代。reader在共享预算内先同Compile完整physical-import表重放，再同独立重建的requirements、全部最终objects和coverage digest精确比较；raw wire本身不授予分类或对象覆盖资格。

### 11.3 三路 relocation分区

最终object undefined-use集合被构造时分成三个互斥集合：

1. M23-3原core/intra-Cone/generated/native/runtime/target分区；
2. M23-5原ordinary core-closed callable分区；
3. M23-6新增general callable/type/dispatch/shape分区。

三者并集精确覆盖所有nonlocal undefined relocation。旧section继续验证自己的子集，不把新subject塞进`CoreStrong`或旧callable-only target。

ObjectDefinition relocation规范化在既有tag1～11之后新增tag12 `DependencyShapeStrong { provider, subject }`；它只从新Link proof派生，不按symbol解析。该hash路径使用既有object-definition runtime scalar encoder：`u32(12) || provider.raw32 || u32(subject_tag) || subject_payload`；Callable payload复用既有StrongCallableDefinitionOwner runtime编码，其余payload是对应typed id的raw32。这里不是Wire CBOR，不套用3.3的map格式，也不改变只供callable-body identity使用的`RuntimeEncode(key)`契约。tag11继续仅表示M23-5的`DependencyStrong`。这是受新required capability保护的object-definition target扩展，不改persistent identity key或旧capability payload。

object verifier既检查undefined target，也检查provider实际TD/scan/table bytes、alignment、field offset、ABI adapter和typed relocation。单靠object symbol table不能证明Scoop signature；ABI一致性由source→MIR→LIR→emission关系和object evidence共同证明。

### 11.4 hash与cache

新Link section的Code贡献沿M23-3 `KnownLinkExtensionCodeContributionV1`：capability为新section id，payload精确为canonical `semantic_imports` array，空时也存在。Compile selected的physical projection、Link section和Code contribution逐byte三方相等。

layout、scan、LIR definition和registration沿既有 `scoop-layout-v1`、`scoop-scan-v1`、`scoop-lir-definition-v1` 与digest DAG算法；不改hash encoder。新semantic record覆盖exact identity、target layout projection、字段/variant/prefix、ABI、scan、TD name与dispatch relation。layout不依赖provider code digest，metadata/table间指向只用typed identity，避免互相引用TD/dispatch/function形成digest环。

runtime-image fingerprint通过既有strong registration/digest graph覆盖新定义。MIR/LIR semantic bytes不包含SlibMemberId、object range、atom placement或host路径；object重新分片只改变对应physical/code/artifact部分。

M23-4 cache继续保守纳入全部direct dependency各层fingerprint，不在本阶段按selected set裁剪。field、base prefix、slot、default/access域、ZST status、layout/scan/ABI或target变化必须使对应consumer层失效；provider body-only变化仍由provider code和后续program-link闭包跟踪，不复制body到consumer HIR。

## 12. 诊断

source错误仍在parser/HIR结束，并断言主span与必要的provider声明/字段路径note：

- 非法protected receiver/词法位置、不可见setter或constructor；
- final base继承、非法override、未实现abstract slot、冲突interface default；
- C ABI ZST参数/result/global/TLS/pointee、空/含ZST CLayout；
- 非lvalue addressOf、非法private backing access；
- 实际请求依赖M23-7 ODR或M23-10 native能力。

artifact错误以capability/table/typed key/field path报告：缺record或source obligation、错owner/provider、forged gc/zst、layout/scan/ABI不一致、slot重复/错position、非法TD shape、required capability/profile/target不符、physical-use coverage缺项或重叠。报告期不按FQN补查来源。

target表示上限由LIR报告明确target/layout错误；内部array count、side metadata、box root-frame或runtime shape违反是fatal invariant，不转成源码异常。错误排序沿现有source顺序与typed key canonical顺序，provider路径只作diagnostic decorator。

## 13. 测试与验收矩阵

### 13.1 独立fixture与组合fixture

生产fixture采用provider→consumer，另加facade re-export和diamond；provider源码在consumer编译时不可见。建议目录 `tests/fixtures/milestone23_stage6/`，每项同时检查HIR/MIR/LIR golden、selected metadata和双view artifact；运行行为在单image harness验证，并标出待M23-9/11复用的真实多Cone运行断言。

| 主题 | 独立positive | 组合与negative |
| --- | --- | --- |
| 外部value | 空struct、嵌套ZST、混合integer/ref struct、enum | re-export/alias、嵌入本地class、模式/copy update；错field/variant identity |
| ABI | 多个ZST参数、用户ZST/Unit result、nonzero indirect | mixed参数与sret、default求值、throw/invoke、member/dispatch；错exact/pass/effect |
| constructor | public构造、protected base initializer、abstract base | base-before-derived、init异常、moving receiver；不可见/abstract allocation |
| dispatch | class override、interface/default、getter/setter | diamond/default冲突、super direct、value boxing；缺slot、错position、窄override |
| visibility | subclass implicit/explicit receiver、protected nested | Base静态receiver、同级subclass、非subclass、support-only import、setter拒绝 |
| object/property | provider singleton/companion、runtime accessor | 跨facade访问、ensure identity、delegated storage；consumer重复storage/initializer |
| support | provider未在自身body使用的公开value仍导出完整support | 缺任一role、错owner、递归helper展开、consumer重发Strong、shell/start提前加入 |

每条编译错误规则都有独立negative fixture并断言span/message，不以单一“大型失败程序”覆盖全部规则。旧M23-5 narrow callable fixture应保持旧分区，用新增nominal/member fixture命中新分区。

### 13.2 表示、GC与address测试

- Unit、空struct、全ZSTtuple/struct为0/正alignment；不同exact ZST有不同identity/TD。内部typed case覆盖alignment>1及超过target上限。
- Option<ZST>保留tag；enum含ref variant独占slot、inactive清零、scan不读tag；niche ref与raw/code pointer扫描分开。
- 五种TD逐variant验证C/LLVM sizeof/offsetof、shape round-trip和所有非法组合；AbstractRef不可形成allocation LIR。
- nonzero box包含ref及over-alignment，验证caller recursive root先于handshake、GC更新同一temp、异常cleanup配对；ZST box/unbox无payload pointer且每次fresh。
- address-taken parameter/local/value this的non-null、alignment、有效期及同时存活place不共址；重复同place稳定，field/element仍不可取址。
- static token extent1/logical0/scanNone；ZeroedForRuntimeUnit与全零EncodedStaticValue区分；错误template长度、relocation leaf/target/order以及跨identity range重叠拒绝。
- Array<Unit>、MutableArray<Phantom<T>>的0/1/大合法count、literal/spread/assembly、越界set RHS副作用/异常、index iterator、clone fresh identity；ZST无copy/barrier/length相关扫描。
- 含ref、alignment>8的array element使data offset不等于24，检查scan位置与relocation；篡改length offset/first offset/stride分别失败。
- String/InlineArray/ZSTArray在INT64_MAX、u64乘加、alignUp、target size_t及对象上限边界的成功/失败；不引入signed-length源码API。
- unsafe Ptr<ZST>正负offset bits不变、load/store求值保留；C ABI逐项测试10.3矩阵和generic CLayout deferred predicate。

generic/structural cases使用1.3规定的typed test harness；对应production请求另有M23-7拒绝fixture，不能通过削弱profile让positive harness结果冒充生产成功。

### 13.3 wire、object、cache与健壮性

- 四条新增 capability、strong-production/6 和新 profile fixed vectors；覆盖 empty/nonempty、unknown required、错 purpose 与旧 profile 拒绝。M23-2 foundation/extern/callback 及 persistent identity vectors 保持；按清理设计退役 HIR 协议定义和 strong production 的旧格式，并更新对应 vectors。共有 metadata 覆盖缺失、额外、重复、错 origin、错 access/receiver、错实际 use、错 provider 与跨表不一致，并通过仅 artifact bytes 的 prebuilt/cache 路径重放。V2 ordinary parent、itable key、dispatch target 的正反例经过真实 strong production wire round-trip；内部 constituent 测试不能代替完整 profile。
- 每个record去掉/增加/错tag/错kind/错owner/乱序/重复逐项拒绝；reader独立重放字段layout、scan、ABI和source-root obligation。
- scan/type-name共享DAG、cycle、深度/展开/bytes预算边界在分配前失败；合法递归ref class成功，by-value环失败。
- 别名/re-export spelling改变不改变exact TD name/ABI；改变base prefix、ZST exact identity、slot contract或scan offset改变对应fingerprint。
- metadata-only外部layout没有伪relocation也能成功；physical callable/TD/table use缺relocation、错definition或多归属失败。
- 三路undefined-use分区两两不交且并集完整；consumer定义foreign Strong、weak/ODR symbol、错误TD/scan/table bytes或关联diagnostic atom均失败。
- 相同semantic program换source枚举、dependency枚举、arena顺序、object分片不改变相应semantic fingerprint；physical/code/artifact变化遵守既有规则。
- producer field/base/interface/ABI变化触发consumer cache miss；失败child/invalid artifact不得发布cache；source/direct/support graph回归不受影响。
- consumer initializer读取provider object/property时保留external unit dependency；V2 round-trip验证provider与descriptor，缺unit、错provider、伪造local、省略真实edge逐项拒绝。callable内部provider自有ensure不被consumer复制。
- class prefix单独覆盖base尾padding、derived更高alignment、ZST own field和abstract base；同Cone与跨Cone布局逐字段一致。slot位置锁定声明序语义，不受wire table按id排序、import或dependency枚举影响。
- 运行M23-1～5及本地value/GC/constructor/property/FFI/closure/coroutine回归，检查stage crate依赖方向。

## 14. 实现顺序

1. 固定本文profile/section/wire constituent及测试vector；添加required capability gate和拒绝旧profile路径。先让生产与 bytes-only reader 使用完整共有 metadata 校验入口，再开放对应源码能力。
2. 收口HIR concrete facts与persistent inheritance surface，完成protected/domain/default witness和source negative；保留旧public section字节。
3. 实现MIR type/constructor/object/slot bridge与selected closure；以golden锁定没有foreign body复制、layout offset或generic template。
4. 收口LIR storage/refined shape、general layout replay、scan normal form和external exact arena；实现provider有限shape-support导出和consumer验证。
5. 接入通用ABI及logical-to-physical映射，验证call/invoke/dispatch/byval/sret/root plan，再开放对应param-free source成功格。
6. 完成box/unbox、ZST place/static、array/Ptr执行路径与C边界矩阵；删除被替代的旧size/scan/header-offset入口。
7. 完成新Link-only closure、三路object coverage、definition规范化、Code贡献和双view发布；更新core、scheduler、cache accepted profile。
8. 完成独立/组合/negative/golden/corruption与回归矩阵，将多Cone运行场景交给M23-9/11复用。

每批代码变更完成后先 `cargo fmt --all`、`cargo clippy --workspace`，再运行相关test；最终运行完整workspace与runtime/fixture验证。不能用最终linker尚未实现为理由跳过本阶段object和双view证明。

## 15. 完成门

- 完整共有 HIR 声明、source/access/inheritance、实际 selected use、MIR relation、LIR layout/ABI/scan/TD 与 actual object use 形成可从最终 artifact bytes 重建的完整校验链；reader 不需要独立来源授权 section、源码、compiler token、symbol/FQN 或 host layout fallback。
- param-free跨Cone构造、value/member/object使用、inheritance/dispatch/protected成功矩阵全部能生成新profile双view有效artifact，未选中的foreign body不复制。
- 每个合法source subject在定义Cone拥有完整有限shape-support；consumer只引用external typed definition，全部ODR生产继续拒绝。
- ZST logical semantics、typed ABI、place/static token、box/array/Ptr/C边界及scan/TD矩阵全部锁定；codegen/runtime不再从size0或空LLVM struct猜语义。
- nonzero box、indirect aggregate和跨Conefield的managed provenance/root/relocation完整；不存在握手后才登记root或从旧ref副本复制的路径。
- 新required section/profile、strong-production/6的完整foreign TD/dispatch引用、fingerprint/cache迁移和三路Link coverage完整；既有identity、extern/callback bytes、旧capability语义保持。
- 独立、组合、negative、各stage golden与corruption/determinism回归通过；文档明确M23-7/8/9/10/11交接，真实多Conemoving-GC不被提前宣称完成。
