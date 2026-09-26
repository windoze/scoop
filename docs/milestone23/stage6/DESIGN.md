# M23-6 设计：跨 Cone layout、typed ABI 与 ZST

静态存储与初始化失败根按其实际值类型引用 layout/scan。当前 Cone 只发射自身拥有的布局与扫描定义；外来类型的静态根复用共有依赖查询取得的完整 value-layout 和 scan 记录，保留实际 provider、typed identity、定义与 relocation，不因本地持有该类型的值而重发射 foreign Strong。layout/scan 指纹节点引用已经解析的实际记录，不要求该类型在当前 Cone 定义；指纹补丁目标仍须属于当前产物。MIR 必须携带生成失败根所需的实际 Any 声明，LIR 不再缺省重建固定 core 身份。static-storage 语义记录新增 field 32 保存 layout provider，完整记录使用 fields 1～32；语义投影使用 fields 1～10 与 32。共有 strong-production 两种格式升级为 /11、/12，旧 /9、/10 产物和缓存重建。runtime C ABI、String 表示、初始化状态与失败缓存语义不变，不引入 ODR 或多 image 启动。

普通 catch 的绑定必须可以像其他引用值一样离开 handler：匹配 native payload 后，在绑定变量前物化一次 managed 异常对象，后续返回、存储和捕获使用该对象；native unwind record 仍按既有 cleanup 规则释放。初始化 catch 复用这次物化，不再次复制。每次 throw 仍创建独立 native payload，runtime C ABI 不变。

引用上行转换在 HIR 中用显式 `ReferenceUpcast` 节点保存内部表达式及目标类型，不能直接改写构造、调用或局部读取的原始类型。MIR 使用已有 `Retype`，不分配对象、不改变引用身份；构造器仍按实际所属 class 分配。默认值正文使用新 expression tag 58 保存同一操作，tag 44 继续退役；共有 HIR 格式更新为 `hir/cross-cone-interface/28`，旧产物与缓存重建，不改变 runtime C ABI。

默认值中的参数自由 class 构造与 struct 构造共用实际 constructor 引用、参数实例化和调用选择；引用上行转换保留在正文中，实例化后继续使用相同的 typed 操作。

虚调用读取当前 `ScoopTypeDescriptor` 的真实布局：runtime metadata V1 在 64 位生产目标上的 vtable 偏移为 88（type_id、完整 instance_shape、object_scan 和 parent 之后）。删除旧三标量 descriptor 前缀的偏移算法；这修正消费者读址，不改变 C ABI 或已发射 descriptor 格式。

共有导出绑定包含 enum 变体的真实 typed ID，nominal 的 `nested_bindings` 同时列出其静态命名空间中的嵌套类型、object value 与 enum 变体；变体归属由实际声明确定，不能误作包级值。`hir/cross-cone-interface/28` 更新该格式语义，旧 `/23` 及更早产物与缓存重建；既有 tag 不复用，不保留双轨 reader，runtime C ABI 和 String 表示保持。

默认值中外来 enum 的模式、变体测试和普通 enum 遵循同一引用规则：正文保存真实 variant 与 owner，类型引用仍检查，构造器集合只收录实际构造表达式。删除为匹配已有值而要求的重复构造器访问记录；`hir/cross-cone-interface/28` 同步引用集合语义，旧 `/22` 及更早产物重建，body tag 与 runtime ABI 保持。真实验收包含第三 Cone 实例化带模式的默认值并链接运行。

跨 Cone enum 变体构造使用共有 nominal/variant 声明、源码参数协议与默认值模板。unit、位置参数、block 命名字段和构造函数式变体遵守与本地相同的参数规则；exact/star/alias import、透明 typealias 和唯一 expected enum type 均解析到真实 variant ID，并服从普通名称遮蔽。前端完成参数与默认值求值后输出包含 owner 类型、variant ID 和声明序参数的完整构造表达式；后续阶段沿共有 enum 表示与 ABI 生成值，不要求不存在的外部机器构造函数，也不创建额外来源资格。默认值中的构造保留相同 typed 引用，可由第三 Cone 直接消费。

外来 enum 的 `when` 使用与本地 enum 相同的 variant 优先查找、字段模式、guard 与穷尽性规则。qualified 名称先走共有类型解析并核对实际声明身份；HIR 模式保存真实 enum 类型和 variant ID，concretization 按已有完整声明转换为普通 concrete enum 模式。payload 读取沿既有 MIR/LIR 的 tag/niche 与字段布局实现，不复制 provider 定义，也不新增 matcher 凭证或独立格式。默认值正文中的模式沿共有模板格式保留同一 typed 引用。

组合值的 ZST 判定必须包含其实际外来字段：本地 tuple/struct 不能因字段事实保存在依赖表中就默认字段非零。producer 从完整 concrete 值表示取得依赖字段的 ZST 结果并在本次投影内复用；依赖事实仍保留实际 provider 引用，不复制为本地定义。

跨 Cone 非泛型 enum 的参数、结果与嵌套值表示由共有依赖声明查询解析。HIR 保留实际 nominal、variant、payload field 身份及完整字段类型；concretization 仅建立消费者使用的值表示，定义与 dispatch 仍引用实际 provider。variant 次序、payload offset、tag/niche、GC scan 与 canonical ABI 使用既有 enum 布局算法，不从名称或相同布局恢复身份，也不为值传递增加来源资格表。验收使用源码产物中返回并再次传入的 enum，覆盖独立 tagged 值及 tuple/ZST 组合。

Strong producer 的两种产物表示从完整 LIR 各计算一次类型、safepoint、immortal 与本地初始化语义，digest 与 registration 直接复用这些结果；初始化仅在 digest 身份可用后补入实际依赖定义。外部初始化引用在解析时完成 definition 与物理导入核对，后续由全局唯一的 local unit 引用表达使用归属，不保存额外 consumer 状态或重复选择核对。descriptor 和 dispatch 直接保存 LIR 中已有的外部 typed 引用，不反向重新 materialize 外部对象或再次比较完整 ABI。实体与 definition role 的匹配只查询实体种类和角色，不借用虚构的 CORE provider；实际 definition、symbol、relocation、ABI 和 GC 检查仍由各自消费边界负责。此清理保持产物字段、指纹内容和 runtime C ABI 不变。

canonical ABI 导出复用同次完整 IR 的实际签名和 callable definition，删除重复逐参数 layout ID 表及只为该表存在的重放入口。MIR callable 的 GC 检查沿结构类型递归使用已有 nominal facts；tuple 的完整参数、receiver 与结果按实际组成保留，不要求 nominal 导出。共有 reader 以真实声明和 MIR lowered signature 核对 ABI，dispatch 按实际 receiver 查询表示。`lir/cross-cone-layout-abi/3` 退役 callable field 5，其余字段编号保持；旧 `/2` 产物与缓存重建，profile 和内容 fingerprint 按格式正常更新。tuple 的字段与参数使用完整结构身份和已有存储算法，不为嵌套值补造独立 nominal、layout/TD definition 或来源记录。

字段布局以完整 `ValueLayoutConstituentV1` 传递 exact 身份、目标、存储、对齐与 scan；嵌套 tuple 从实际元素递归计算并复用结果，不要求独立 layout definition。C-layout 的嵌套合同、enum niche 的真实指针种类分别随实际表示传入。字段与实例中的布局数据是内嵌值，不构成 Link 符号引用；语义依赖图只沿实际 descriptor、dispatch、shape-support 和机器 relocation 引用闭合，读取边界继续核对完整字段身份、顺序、布局与 GC 事实。

依赖默认值的 tuple 字面量与字段投影直接实例化为完整 HIR，类型沿同一共有签名查询取得。读取边界已核对的字段索引、元素类型和默认正文在实例化时直接使用，不另以 core 类型或单一 nominal 形式限制参数。

MIR 组装先排除已经由普通 callable 表保存的实际声明，再生产需要独立 lowering 的定义；不为同一函数重建第二份绑定、完整验证后再丢弃。boxing 与 dispatch 的共有查询同时借用当前 Cone 的普通记录和依赖记录。每个定义只生成一次，签名、GC effect 与 typed target 沿完整 IR 和已有表消费，最终表继续拒绝重复定义。

实际 HIR 调用在依赖 MIR 记录可用后完成一次调用根与逻辑签名核对，普通函数、accessor 与 constructor 共用该边界。MIR 和 LIR 的依赖集合分别从实际 typed 调用加入相应 callable 与 ABI 引用，沿共有 provider 查询消费已有定义；不得因旧函数表不含 constructor 而拒绝合法调用，也不在前置阶段和布局阶段重复核对同一调用。

共有 nominal 声明直接保存 struct 主构造器的 typed declaration ID，供前端按实际语言角色检查 `@NoGC` 调用；不能从参数形状、字段布局或 provider 身份推断主构造器。主构造器继续保留源码 Managed、物理 NoGC 的既有合同；值构造本身不分配，`@NoGC` 的参数、结果与局部值仍须 GC-free，managed 次构造器仍禁止调用。`hir/cross-cone-interface/28` 在 `NominalDeclarationDetailsV1` 新增 field 8：空数组表示无值主构造器，单元素数组保存其 constructor ID；有构造器的 struct 必须明确该引用，引用必须属于同一 nominal 的声明集合，其他 nominal 不得填写。旧 `/21` 及更早格式退役并要求重建，既有 tag 不复用，profile 与内容 fingerprint 正常更新；MIR/LIR callable 格式和 runtime ABI 不变。

跨 Cone struct 主／次构造器使用共有声明目录及真实 constructor ID，保留所属 nominal、参数协议、默认值作用域和访问关系。请求内 MIR 选择按 provider 与 Strong callable target 查询已有完整记录；普通直接 callable 与需要独立 lowering 的记录使用同一引用域。主构造器的逻辑 Managed 签名与实际 NoGC 值构造签名分别用于源码语义和机器调用；次构造器使用其真实 lowering 合同。Strong 外部根携带实际 implementation 与完整物理签名，不另保存 function/accessor 资格 ID。LIR 从共有 layout/ABI 选择解析构造器定义、symbol 和 relocation，与实际 MIR 根关联后使用共有 callable materialization；ZST 实参先按源码顺序求值，再依 canonical ABI 消除机器槽位。构造器继续使用原 M23-6 callable/selected 字段，直接 callable 继续使用原字段，各定义只发布一次，不增加 wire tag、profile 版本或来源凭证。

MIR 输出在 HIR→MIR 边界完成一次整模块结构、类型与实际外来 callable 检查，并同时保留已生成的 canonical foundation、共有依赖选择和完整物化引用。通用 MIR 输出保留完整泛型实体；Strong profile 的 ODR 能力门仍在其消费入口检查，并共享已有 canonical foundation。driver、MIR production 组装和 MIR→LIR 直接消费同一完整输出，不再从未变化的 Module 重建第二份 foundation、重复验证外来调用或重跑整模块检查。production 与模块之间仍核对实际 callable、入口和初始化关系；直接借用已有签名记录，不构造第二份预期桥表。外部新产物的格式、引用、ABI 与对象检查继续由 reader 负责。此清理不增加凭证、状态机、wire 字段或 profile 版本，不改变 runtime C ABI、String 表示或后续里程碑范围。

产物的 identity graph 从 manifest 的当前 producer、实际直接依赖和已经读取的依赖实体构成；不无条件注册 CORE 身份，也不为 CORE 设置单独的重复过滤规则。CORE 与普通 provider 的声明使用相同的 typed 引用解析，缺失依赖、身份冲突及非法引用由共有格式与引用检查报告。默认 core 依赖仍由正常构建与前端依赖发现加入 manifest；本项不改变 wire 或 runtime ABI。

代码生成在入口对本次完整且不可变的 LIR、目标 profile 和 executable 入口完成一次必要验证，然后按实际定义分成函数与非函数对象。各成员直接消费同一 LIR，不因发射另一个对象再次完整遍历类型、ABI、CFG、GC roots、safepoint 或身份表；LLVM 变换后的 IR 和新生成的对象字节仍在各自边界检查。generated-C 源码入口同样不在内部 helper 重复整模块验证。这一职责调整不改变产物格式、runtime C ABI、String 表示或链接语义。

外来参数自由 struct 使用依赖中实际的 typed nominal 声明和完整字段类型。HIR 保留其完整成员、继承、构造器及字段声明，参数、结果、局部存储和嵌套字段使用同一实体。LocalConcrete/MIR 保存计算值表示所需的字段及真实定义 Cone；成员与构造器引用定义方 callable，layout、TD、ABI 和 relocation 由共有依赖查询取得，不在消费 Cone 重新定义外来函数或 Strong 产物。

普通 final 成员按 receiver 的真实 nominal 声明查找，并通过共有调用路径传递隐式 receiver、参数、结果和 GC effect。默认参数、命名参数与 operator 使用同一候选决议。成员调用直接保存 typed 声明引用；实际做过 namespace 导入的调用同时保留该次查找路径，选择集合不再聚合另一份 import 路径用于认证调用。 成员属性读取与写入先选择同一真实 property；setter 通过该属性的 typed accessor ID 取得，不从同名属性、相同布局或 provider 来源推断。赋值、复合赋值和更新使用共有 place 与 callable 路径，保持 setter 可见性、单次求值、动态分派、ZST/大值 ABI 和含引用写屏障。验收由源码发布产物后跨 Cone 调用、再次发布并链接运行。 接口 dispatch 签名直接消费完整 typed callable 参数与结果；自动生成或抽象 accessor 不要求普通函数的命名参数源码协议，内部参数显示名不承担实体身份或调用选择职责。

`hir/cross-cone-interface/28` 为无 namespace binding 的声明调用保留 `SourceDeclaration` reason（tag 3，只有 field 0 的 map）；既有 tag 1/2 保持。普通直接 callable 的 MIR 导出保存完整 GC effect，`mir/cross-cone-param-free-bridge/2` 的 field 1～3 保持 declaration、implementation 和 exact signature，新增 field 4（unsigned 1=Managed，2=NoGC）。旧 major 退役，profile、fingerprint 和缓存同步重建；runtime C ABI 与 String 表示不变。

MIR 的普通直接 callable 与需要独立 lowering 表示的 callable 按 typed implementation 共用借用查询，dispatch 和 boxing 直接读取实际记录。每个定义只保存一份导出；删除禁止跨历史分区引用的资格限制和专用身份名单，保留重复定义、类型、签名、effect 与 provider 一致性检查。

清理旧 HIR source transcript 路径：删除 `TypeFoundationSourceAuthorityV1`、`TypeDeclarationSourceAuthorityV1` 及其仅供测试的构造、编码、绑定和反向重建入口，删除依附这些入口的 `Bound*` 证明链和专用测试，以及没有生产消费者的 nominal 来源快照、依赖事实和继承边重复表及其编解码器。共有 reader 已消费的声明表、实际默认参数数据、typed identity 查询与必要格式检查继续使用原有生产路径。未进入正式产物的旧测试格式直接退役，不引入双轨 reader 或新资格包装。

LIR dispatch 同样查询实际 provider 的普通或专用 lowering callable ABI，校验调用约定、完整签名与 receiver 表示后保留 typed implementation/body 引用；每个 slot 不再保存一份完整 callee ABI。布局依赖闭包沿共有导出查询真实类型，普通记录只编码于其原表。reader 在布局完成后先检查普通 ABI，再供 dispatch 和后续 Link 消费，不因表的位置重复重建或验证同一 ABI。

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。 依赖查询直接使用真实 `ConeIdentity` 与 callable、property、type-alias 的类型化声明 ID；选择集合按这些 ID 保存完整接口和实际依赖路径。删除独立 world/projection/selection 品牌、仅为品牌服务的计数器和错误，以及从声明 ID 再映射到局部 u32 的三套重复表。候选选择依照当前依赖目录中的实际声明与表示，不要求由同一查询实例铸造；直接依赖的名称可见性、转导出路径、实际 provider 和引用完整性继续按共有规则检查。HIR 候选和已选声明只保存实际 provider ID，不逐项复制 artifact 坐标/fingerprint 凭证；HIR→MIR 使用同次编译的依赖快照及完整声明，核对实际定义和签名，不再次比较来源凭证。普通构建依赖记录与缓存 fingerprint 继续承担定位和失效职责。MIR 外部 callable 引用保存实际 provider 与类型化声明，不另映射到带会话品牌的局部编号；调用位置保留 GC effect。MIR 类型与 LIR layout/ABI 选择按实际 provider 和 typed target 直接返回完整记录，不先铸造并验证中间 handle；依赖闭包、类型、ABI、物理引用和 GC 契约仍在其消费边界检查。同一声明或 target 在另一选择集合中是否存在，按实际目录查询决定，不依据集合生成顺序或计数器。该进程内数据简化不改变 wire/profile、实体身份或 runtime ABI。

M23-6 的实现范围以最新版 AGENTS.md 为准：完成类型布局、canonical ABI、dispatch、ZST、跨 Cone 消费与产物发布；同时删除编译器、slib、runtime 中额外的来源授权、防伪、资格认证、通用资源预算与计费、重复证明和仅为这些机制存在的框架。此约定修正历史设计中的冲突条款，将 core 专用机制推广到所有 Cone 不构成完成清理。正常类型、可见性、格式、引用、依赖环、缓存、ABI、内存范围与 GC 契约继续保留。

配套：三份 `docs/specs/SCOOP-*.md`、[M23 总设计](../DESIGN.md) 与 [清理设计](CORE-AUTHORITY-CLEANUP.md)。本文定义当前目标；历史实现记录不构成继续保留废弃机制的要求。已完成的部分按生产调用链核对后复用。

## 0. 结论

构建管理与编译器消费是两个明确边界。`scoop` 的发现、完成节点和缓存只保留不可变归档快照及普通 manifest 摘要；检查容器、成员范围与 hash、版本/profile、Cone/target、依赖 fingerprint、缓存键和子进程结果，不解码 HIR/MIR/LIR 或重建 Link 对象。已读 prebuilt 快照复用同一摘要，不能在每完成一个节点时重放该节点及全部依赖。`scoopc` 或实际 Link 消费者在读取外部产物时完成 typed 引用、ABI、对象与 relocation 检查，Compile/Link 共享这份完整结果；同次编译的输出直接发布。摘要不能替代 IR 或对象，也不承担来源或操作资格。此职责调整不改变 wire、runtime C ABI 或 String 表示。

String descriptor 使用完整 MIR 中实际声明的 source exact identity，沿共有 descriptor 查询、layout selection、physical import、registration 和 Link relocation 消费。删除独立 String bridge 与固定角色的 descriptor 恢复通道，不以 provider 坐标或协议来源豁免普通引用检查。Strong production `/7`、`/8` 退役原服务表中的 TD tag 2；旧产物与缓存重建，String 表示及 runtime C ABI 不变。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

初始化单元直接携带所需的本地或外来 callable。共有 MIR 依赖选择从这些单元收集实际 provider 与函数引用，driver 不再另走一次初始化服务选择；MIR 不保存 Local/ImportedUnused/Imported 的配对状态，也不逐单元再核对一份全局来源记录。MIR/LIR selected 表不复制初始化角色、不限制只有一个该角色，也不提供服务专用添加入口。声明发布需要的语言角色继续保留，实际 target、签名、GC effect、ABI 与依赖关系由共有记录及消费边界检查。该进程内简化不改变 wire、runtime C ABI 或初始化与失败缓存语义。

Strong production 的两种表示使用 `/11`、`/12`，删除初始化专用 ABI field 11 和外部服务表 field 12，section 只保留 field 2～9 的八字段 product。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 registration reader 完成结构与引用检查后直接返回完整 registration production 数据，不保留仅用于限制编码或导出资格的中间凭证包装。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

LIR descriptor 依赖是明确的 typed IR 引用。reader 验证可达 provider、exact type 的实际 descriptor 导出、递归依赖闭合及物理定义，不要求 descriptor 先出现在 HIR 的 value-layout 或 shape-support 物化根中，也不从协议表重新证明其来源。显式 descriptor relation 与从源码类型使用得到的 layout/shape 根共同闭合；遗漏必要依赖、错误 provider/类型与不一致物理合同仍拒绝。

共有 Link import 表可以保留没有实际 relocation 的完整类型或 callable 引用；这类声明不生成虚构的 machine use。Link 只对实际 relocation 生成 requirement，并验证目标、provider、symbol、ABI 和定义一致性。删除“每个已声明 import 必须至少出现一次”的附加证明，以及初始化服务的专用豁免；缺失实际引用、错误定义和损坏对象仍由共有 verifier 拒绝。

M23-6 把已有的本地类型表示变成可独立验证、可跨 Cone 消费的接口。成功解析到外部 nominal 之后，consumer 必须取得定义方的完整语义、layout、ABI、scan、TypeDescriptor 与 dispatch 信息，才能产生 machine use。

1. 新增 `cross-cone-layout-strong/2` production profile。它在 M23-5 inventory 上新增 HIR type/inheritance、MIR type bridge、LIR layout/ABI 和 Link-only layout-use closure 四条 section，并将强定义语义升级为 `strong-production/12`，以表达普通依赖的 TD/dispatch 引用；仍拒绝全部 ODR production。完整声明与实际使用信息按 3.1.1 合入共有 metadata，不新增独立来源授权 section。
2. persistent identity 与 extern/callback contract bytes 保持；native-boundary witness 按当前 intrinsic family 与 typed C 投影合同显式升代，旧字段和版本退役。一般 layout 服务只能由新 required section 构造，不从 native witness 推出通用 layout、scan 或 dispatch。
3. HIR 输出每个 concrete type 完备的 `gc_free`、value `ZstStatus` 与继承/slot 语义；MIR 输出表示无关的类型、构造器、成员、slot 与生成 helper 关系；LIR 独占 target layout、Scoop ABI 与递归 scan 的生产权。
4. 外部实体保持定义 Cone 的 Strong ownership。consumer 可以检查和在本地类型中内联外部 value 的表示，但不能重新定义其 body、layout constant、scan、TD、dispatch table、registration 或初始化 storage。
5. 定义 Cone 为每个可跨 Cone 引用的 param-free source nominal 预物化完整、有限的 `BoxedValue`（仅 value）、`CoroutineStep`、`CoroutineSlot` shape-support。closure 从 source subject 展开一次，不递归把 helper 当作新 source subject。
6. 通用 `ValueStorageLayout` 使用 `ZeroSized | NonZero`，zero-sized 分支不能携带 ref scan；未装箱 value layout 与五类 managed instance shape 分离。可分配操作只能接收排除 `AbstractRef` 的 refined descriptor。
7. Scoop ABI 保留全部 logical exact type，并用 `ElidedZst` 删除物理 payload。`UnitVoid` 与用户 ZST result 分开；全部 direct、invoke、virtual/interface、adapter 与 Scoop extern 复用同一分类和 logical-to-physical 映射。
8. ZST 的求值、异常、构造和方法语义不消失。需要地址的 local/parameter/value `this` 使用独立 token，static token 沿已有 persistent storage contract；每次 boxing 仍有 fresh managed identity。
9. `Array`/`MutableArray<ZST>` 保留 logical size、bounds、求值和 index iteration，allocation 不随 length 增长，不能生成 zero-stride scan、payload copy 或逐元素 token。
10. 开放 closure 完备的 param-free 跨 Cone 构造、value 投影、member/accessor、object value、继承、virtual/interface dispatch、`is/as` 与 protected access。protected 调用保存实际声明、接收者和访问上下文，由前端按语言可见性检查；不增加授权包装，也不把 protected 声明提升成 public wrapper。
11. 通用表示算法和 compiler/layout/object 测试本阶段覆盖 generic/structural shape；需要独立 Nominal/Structural ODR materialization 的生产请求仍留 M23-7。`Array<T>`、tuple、function、pointer 不因布局简单获得 Strong 特赦。
12. 本阶段产出双 view 有效的多 Cone `.slib`，验证 local runtime 表示操作；真实 multi-image startup、artifact-only program-link 和多 Cone moving-GC 分别留 M23-8、M23-9、M23-11。

核心关系为：

```text
resolved declaration + complete type/inheritance interface
    -> committed exact external use
    -> selected MIR representation/dispatch relation
    -> selected LIR layout/ABI/scan/descriptor records
    -> metadata-only dependency | verified physical Strong use

same size/alignment != same exact type
same physical signature != same callable contract
external layout knowledge != permission to emit an external definition
ExactOwnerRoot == SourceCone != exemption from dependency ODR requirements
```

## 1. 范围、基线与阶段边界

### 1.1 当前实现基线

截至 M23-5，仓库已具备若干可复用的本地表示，但尚未形成完整的一般跨 Cone 消费路径：

| 位置 | 已有能力 | 本阶段工作 |
| --- | --- | --- |
| `compiler/lir/src/abi.rs` | `AbiZst`、Direct/Indirect/ElidedZst、logical-to-physical 参数映射 | 关联 persistent exact type 与实际导入布局，统一全部调用入口 |
| `compiler/lir/src/type_descriptor.rs` | checked value/array storage、五类 instance shape 与扫描约束 | 形成跨 Cone required wire、closure validator 和 refined external descriptor |
| `compiler/lir/src/metadata.rs`、`compiler/lir-lower/src/metadata/layouts/` | 本地 aggregate/enum/field layout | 收口 raw size/alignment 构造，接入定义方布局与继承 prefix |
| `compiler/lir/src/production/shape_support/` | core 的有限 shape-support 数据 | 合并到所有定义 Cone 共用的形状表，移除 core 专用查询分支 |
| `compiler/hir/src/visibility.rs`、`compiler/hir-lower/src/visibility.rs` | lookup/inheritance/slot domain 与本地 protected 规则 | 导出完整继承关系，前端按实际接收者检查 protected 访问 |
| HIR/MIR/LIR 的 `cross_cone_*` 模块 | M23-5 core-closed callable 子集与 selected bridge | 增加一般类型、构造、slot/dispatch 和 object-value selection |
| `compiler/lir-lower/src/function/expression.rs`、`runtime/src/rt.c` | 现有 box/unbox/array 执行路径 | 消除旧 payload/size/scan 多源调用与固定 header-offset 假设 |
| `compiler/codegen/src/function.rs` | 本地地址存储发射 | 在 LIR 明确 token place，codegen 不再由 LLVM 空类型猜 ZST |

本文定义可跨 artifact 消费的格式与契约，复用已有本地算法。同次编译直接导出完整 typed IR；外部 reader 在格式、引用、类型与 ABI 边界完成必要检查，不以额外证明构造器或发布凭证控制导出。

### 1.2 生产成功矩阵

| 使用 | M23-6 结果 | 所需信息与检查 |
| --- | --- | --- |
| M23-5 已成功的 const、top-level/extension callable | 继续成功 | 旧 route/bridge 原样保留 |
| param-free public struct/enum 参数、返回、构造、模式、copy update | 成功 | public representation + MIR shape + LIR layout/ABI |
| param-free class allocation、base constructor、member/accessor | 成功 | constructor access、完整 object shape、initializer bridge |
| param-free interface/default/virtual dispatch、`is/as` | 成功 | 完整 ancestry、slot contract、唯一 implementation 与 TD |
| public object/companion value及 runtime property | 成功 | provider ensure/value/accessor target 与初始化 ownership |
| subclass 中合法 protected member/constructor/nested type 使用 | 成功 | 实际继承关系、前端可见性检查与完整调用表示 |
| imported value 嵌入本地 field/enum/capture | 成功 | representation closure；新生成实体的 ownership 也须通过 Strong gate |
| generic nominal/callable application，generic delegated extension | M23-7 能力诊断 | 不生成 partial LocalConcrete/MIR |
| 独立 tuple/function/raw/native-pointer TD 或其他 Structural ODR entity | M23-7 能力诊断 | 不因物理布局简单改成 Strong |
| suspend external call、需要 `ContinuationShell`/`CoroutineStart` 的操作 | M23-7 能力诊断 | hidden ABI 依赖 generic nominal application |
| direct foreign source `@Extern`/native storage 使用 | M23-10 能力诊断 | 完整 native closure 尚未开放 |

所有“成功”格还要求传递物化闭包不包含 M23-7/10 能力。param-free declaration 不保证其字段、签名、默认参数或 generated dependency 也是 param-free；gate 必须检查实际闭包，不能只检查声明的 type-parameter count。

发布源码接口与发布机器接口分别按其实际需求闭合。普通 library 可以保留完整 generic 或暂不可物化的源码声明；只有完整机器依赖闭包满足本阶段能力的声明进入 layout/ABI/dispatch 导出与有限 shape-support。未被实际物化的声明不能仅因存在于源码接口就使整个 library 失败，也不能通过删除其公开类型、继承、成员或默认值信息来绕过 gate。实际本地使用、可发布机器根或下游使用一旦需要 generic/Structural ODR 或 native 能力，仍在该使用处报告 M23-7/10 诊断，不能发出部分机器接口、空实现或替代定义。此规则对 core 和普通 Cone 相同，包括 `IntRange : Iterable<Int>` 这类无自身类型参数但依赖 generic application 的声明。

M23-6 的 HIR nominal type 导出先从同一次 Export/LocalConcrete HIR 计算表示与继承的必需集合。依赖包括全部自身字段、enum payload、class base、interface ancestry、公开或 protected 构造器参数，以及实际 virtual/interface slot 的参数、结果与 execution；generic application，或依赖泛型/挂起 ABI、native 定义的 slot，使该声明保留为 source-only，并沿本地 nominal 依赖传递。引用类型环按有限图求闭包，不能因遍历顺序误判，也不能通过截断继承、字段或 slot 消除依赖。完整源码根、kind、binder、modality、supertype、成员、构造器与默认值仍保留；只有闭合的参数自由根进入 representation、fact 和 exact inheritance 表。候选表与从声明计算的必需集合逐项覆盖，可物化根缺失或 source-only 根混入均拒绝。该计算不按 CORE 身份或 source 路径分支，不授予 callable body、独立 Structural ODR、native、shape-support 或实际跨 Cone use 能力；这些仍由完整机器闭包与对应阶段能力门检查。

M23-6 的 nominal 表示与继承物化闭包直接读取共有 nominal/callable 声明；它是临时查询结果，不保存第二份来源、资格或 exact 清单。公开记录和必要支持记录使用同一完整字段、enum payload、supertypes、公开或 protected 构造器参数及实际 slot 签名；getter/setter 与普通函数同样参与。generic application、未绑定 binder、generic/suspend/native slot 将所属 nominal 标记为 source-only，沿当前 provider 的 nominal 依赖反向传播；环按有限图求不动点，结果不依赖声明顺序。structural 签名递归检查子类型，但不因此授予独立 Structural ODR 物化。HIR producer 与 artifact reader 重放同一算法，保持完整源码声明；形状需求只从当前 exporter 的 DeclaredCurrent 公开根中选择闭合子集，不能因 source-only 声明存在而要求不存在的 shape-support。外来 nominal 仍由其实际 provider 的机器接口及消费闭包验证；本地临时集合不授予外来机器使用、访问权或后续里程碑能力。Link 入口接收已验证的 Compile artifact，先核对完整 artifact fingerprint、实际 Cone、target 与 compatibility，再借用其中的共有声明和 LIR bridge；调用方不能另行传入候选声明替代该来源。不从待验 LIR 形状表反推需要集合，也不增加持久化资格字段。

共有 nominal 物化查询必须保留各条依赖的语义角色：普通字段、enum variant 字段、完整直接继承类型、公开或 protected 构造器、实际 virtual/interface 槽。查询只借用同一 canonical nominal/callable 声明；字段保留原 typed field identity，callable 保留完整原声明、签名与 effect，不能把 private 字段、重复类型位置或 accessor 槽归并后丢弃。generic owner 不进入本阶段的参数自由物化遍历，private/internal 构造器及非槽普通成员不自动产生类型机器需求；构造器/槽所属的参数自由 nominal 必须存在于同份声明集合，不能静默忽略悬空 owner。槽需求同时来自声明自身的 slot relation 和共有 nominal 的实际 dispatch selection；值类型或 final 类的接口实现即使没有自己的物理槽，也必须按被选择的完整 typed callable 加入签名与 effect 判定，不能仅因 slot_relations 为空而漏掉实现。相同实现被多个槽选择时，保留原声明的完整参数序列；实际选择关系仍由原 nominal metadata 单独验证。现有 source-only 传播与后续 selected 语义边生成应复用此查询，它不保存新 wire 表，不授予来源访问或机器资格。

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

迭代与绑定计划的语义检查属于前端：选择实际 iterator/component、验证签名及 effect、构造局部数据流和 typed 循环目标后，concretizer 直接代换并展开完整计划。删除 `validate_iteration_plans` 这条没有产物 reader 生产调用的整模块重放通道及其独立语义实现，不把它移成新工厂或另一份证明。真实语言错误、generic 展开、继承/boxing、默认值及 break/continue 的回归继续保留。

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

ordinary callable 的 canonical ABI 校验对 core 与普通 provider 无差别执行。旧 callable bridge 借用当前 artifact 与其实际可达依赖的 native-boundary witness，重复 owner 必须完整一致；layout profile 在完整本地和依赖 section 通过后，按 exact type 查询唯一的 ManagedValue layout，不回退到 witness。两条路径使用 LIR 共有的 direct/indirect/ZST 分类，逐项核对 logical signature、GC effect、参数次序与结果；缺失、重复或不匹配的 layout 拒绝，此处共有 ABI 重放不代替 native-boundary 专用外来类型入口与固定 core 身份识别的后续迁移。

native-boundary reader 在同一 validated identity graph 中解析本地与外来声明，统一重建 owner、kind、参数个数、binder 及成员关系；不保留 external-core 的零字段/Reference 特许恢复分支。完整性查询共享当前和依赖图的 canonical field、variant 与 variant-field records，依赖 key 以共享引用读取，不加入本地定义 inventory。缺少 canonical 声明或遗漏实际成员必须失败，generic 声明的结构解析不授予 generic application 执行或 ODR 物化能力；profile 的既有 gate 继续检查。producer 的外来类型输入按下述共有 world 规则闭合；后端 typed 表示与通用 layout/ABI 查询的连接仍须按清理设计完成。

共有 struct source shape 在 `cross-cone-interface/4` 中精确为 `{ 0: 3, 1: source-order fields, 2: NominalCLayoutPolicyV1 }`；policy 使用 type-semantics 的既有闭合编码，由实际 HIR `@CLayout` 属性投影。公开 source shape、nominal source contract 与 nested source support 共享该结构，representation join 必须同时比较字段与 policy。intrinsic source shape 使用新增 tag 6 明确保存完整 family，并与 representation 比对，不能使用普通 struct/class shape 代替。旧 `/1`、`/2`、`/3` section 及旧两字段 struct shape 直接拒绝并重建产物，profile fingerprint、inventory 和 HIR fingerprint 随之更新，不回改 native-boundary witness 或 C ABI。本次扩展只补齐声明事实，不以 source shape 授予 layout、scan 或物化能力。

native-boundary producer 直接借用共有 dependency world，不再接收 `CurrentArtifactOnly/TrustedCore` sum 或 `ImportedCoreNativeBoundaryTypes`。外来 owner 必须解析到实际 provider 的 canonical source key；共有 v4 source shape 提供 intrinsic family、完整 struct CLayout/字段或 enum variant/字段，成员 key 来自同一 provider。非公开但已有 native witness 的声明可沿已持有 typed 引用继续闭合，Reference 由真实声明 kind 决定；共有 source shape 与已有 native witness 同时存在时逐项一致。缺失或不一致立即失败，不按 CORE、名称或空字段补默认记录。本地和外来 shape 都进入同一 signature-type 传递遍历，支持跨 direct/support provider 的字段/variant 闭包并按实际 owner 规范化；不把该最小 witness 作为一般 layout、lookup 或物化能力。

整数 `div/rem` 通过共有依赖声明解析 `ArithmeticException`，并与强制 `as` 的失败分支共用普通类型、constructor、ABI 与 relocation 路径。只有实际运行点物化异常表示；未使用的默认表达式不产生机器依赖。删除 Managed 整数运算的旧无条件布局门和 MIR 的 Defined core 要求，不另存来源或操作证明。验收覆盖正常除法/余数、signed 边界、除零 catch、默认参数、修改 core 后的默认构造器适配，以及产物的下游链接和移动 GC 运行。

compiler protocol 的 constituent 仅验证实际声明与角色关系，不额外要求当前 export、声明 owner 或导入 foundation 的来源为 CORE。已有 identity/definition-origin 检查、完整 signature 与 binder、constructor/generated-adapter source、enum kind/member owner、effect 和固定角色完整性继续执行，普通 provider 的相同声明经同一路径验证；缺失或不一致不能以来源身份豁免。Unit 保留既有语言内建身份。producer/reader 删除重复来源资格，typed 引用编码与前端 intrinsic 使用边界不变；完整 operation 表按下一段退役，HIR 的 Core/NotCore 外层、重复 String capability 及 definitions 外层按下述 `/4` 修订删除；String 和初始化服务使用共有 LIR/Link 记录。

Lowerer 直接保存已解析的 `ImportedCoreProtocols`，Concretizer 直接借用 Export HIR 的 `CoreProtocols`。删除仅包裹同一数据的 `ImportedCoreLoweringAuthority` 与 `CoreConcretizationAuthority`；本地 arena 引用和外来持久声明引用仍保持类型区别，不重新包装为阶段资格。此进程内简化不改变 wire、实体身份或 runtime ABI。

完整 intrinsic operation 表从 protocol product 的 field 9 退役，HIR `core-bootstrap-interface` 的 `/2` 修订使协议 product 只接受 field 1～8，当前 `/4` 直接保存完整协议角色，旧 `/1`～`/3` capability 与旧九字段 payload 拒绝；field 9 保留为退役编号。共有 callable effects 是发布 intrinsic kind 的唯一记录，普通 Compile 与 layout-profile 均在实际共有 callable/public-source 验证路径中检查 source owner、own binder、参数/结果与 execution，所用语言类型角色来自已解析的实际 provider 依赖闭包，不能由 CORE 常量或名称补出。验证按已有 callable 表逐项执行，重复 kind、角色缺失/冲突和错误签名拒绝，前端完整 intrinsic 声明检查和固定语言协议角色继续保留；没有实际导入消费者的完整 operation 表不作为新的授权或执行目录。

HIR 的 `org.scoop-lang.hir/core-bootstrap-interface/4` 直接保存完整 `CoreCompilerProtocolSurfaceV1`，删除重复的 `RuntimeCoreCapabilityV1::String` 与 `CompilerProtocolDefinitionsV1` 外层。section 的 field 2、3 继续保存 output contract 和 direct public surface，field 5 以长度为 0 或 1 的 array 保存本产物定义的协议角色；旧 field 1、4 及旧资格分支 tag 1、2 退役，不复用，旧 `/1`～`/3` 产物与缓存重建。String 的 source identity 只在已有 fundamental type 角色中保存，exact identity 由该真实声明的 `ExactTypeKey::Nominal` 得到；不再维护第二份 source/exact 记录或比较两份投影。前端完成 intrinsic 识别与语言声明检查，后续阶段直接消费完整 typed 角色、共有声明与实际 provider；reader 保留格式、引用种类、签名、effect、成员归属和跨表一致性检查。初始化循环服务的实际函数、String 参数、Unit 结果及 canonical ABI 继续沿共有 callable 路径使用。此修订不改变 String 表示或 runtime C 调用约定。

M23-6 删除没有实际 producer/consumer 的 `ScoopRuntimeCoreBindingsV1`、`ScoopProgramDescriptorV1`、固定 String capability ID 及其 LLVM 布局镜像和专用测试，不把它们改名后保留。未使用的 program/core magic `0x53434f4f50505247`、`0x53434f4f50434f52` 退役且不复用。实际发射的 image、root entry、type/callable、storage、immortal、initialization 和 safepoint 记录继续使用既有格式；这项删除不改变实际 runtime C ABI、String 表示或其 fingerprint。M23-8/9 的多 image 启动与 program-link 仍留在后续阶段，按实际入口和引用需要定义数据，不提前冻结新的 program record 或另建 String 授权表。

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

MIR 类型与 LIR layout/ABI 选择保存经过依赖闭包检查的完整记录，直接按实际 provider 和 typed target 查询；类型身份不由 symbol 或同布局推导，不另以选择集合的品牌决定使用资格。MIR 消费完整 typed HIR，LIR 消费完整 MIR，codegen 消费完整 LIR，各 stage 不回头重做源码解析。

## 3. profile、section 与 wire 演进

### 3.1 新 production profile

新增：

```text
org.scoop-lang.slib-profile/cross-cone-layout-strong/2
```

其 required inventory 从 M23-5 `cross-cone-semantics-strong/2` 出发，移除 `org.scoop-lang.lir/strong-production/11`，替换为 `/12`，再加入下表前四项；`code_requirement`、`runtime_requirement`、publication、extra-section policy 和实际 Link 数据要求沿用共有规则；旧 decode-cost model 字段 3 退役，`odr = RejectAll`。

| capability | location | required_for | sinks |
| --- | --- | --- | --- |
| `org.scoop-lang.hir/cross-cone-type-semantics/7` | HIR | Compile | Hir |
| `org.scoop-lang.mir/cross-cone-type-bridge/1` | MIR | Compile | Mir |
| `org.scoop-lang.lir/cross-cone-layout-abi/3` | LIR | Compile | Lir |
| `org.scoop-lang.lir/cross-cone-layout-link-closure/2` | LIR | Link | Code + LinkValidationOnly |
| `org.scoop-lang.lir/strong-production/12` | LIR | Compile、Link | Lir + Code + RuntimeImage |

`cross-cone-type-semantics/7` 承载 type facts、完整表示与继承接口，复用 `cross-cone-interface/28` 的普通声明、参数和默认值 metadata。HIR type、MIR type bridge、LIR layout/ABI 三条 Compile section 的完整 canonical inner bytes 分别进入对应 layer contribution；Link-only section 仅以 semantic physical-import projection 进入 Code，member/range/patch 信息只作 LinkValidationOnly。strong-production/12 沿用强定义 section 自身的三个 sink。

所有 source Cone、core、single-file 与 cache 产物最终使用新 profile、四条新增 section 及 strong-production/12；空集合显式编码。compiler compatibility 与 cache key 共用新 profile fingerprint，旧 completed dependency 必须重建，不做内存升级。section major 与 outer schema 是不同版本维度；本阶段仍使用 M23 outer schema 1 和既有 runtime ABI。

#### 3.1.1 共有 metadata 与实际使用闭包

普通与 layout 产物中的 canonical C layout 合同同时覆盖实际 native 使用和已物化的本地 CLayout 表示。读取方在共有声明及实际依赖闭包验证后，使用同一物理物化 exact type 清单与共有 nominal 的 CLayout policy 收集表示根，再按实际 provider 的完整字段类型、aligned/packed 和 target 重算合同；native-only witness 闭包仍只由真实 extern/callback 使用决定。额外表示根不从候选 C layout 合同反推，不新增 native callable 或 witness。普通 Compile reader 在每个 provider 的可达依赖均可借用时完成这一重放，缺失、额外或非 canonical 合同仍按完整集合拒绝；无 native 调用的合法 CLayout 可以发布并从最终 bytes 重读。不修改 wire、runtime C ABI 或 persistent identity。

跨 Cone struct 字段读取按接收者的实际声明解析名称，并在 HIR 保留 typed field identity 与完整接收者类型。字段在具体化时映射到同一声明的字段位置，后续布局和 ABI 继续使用共有依赖表示；不得用同名或同布局替代身份。计算属性通过其真实 getter 声明进入共有 callable 路径，保留可见性、GC effect 和返回类型；固定表示字段不为读取额外生成函数。生产验收覆盖普通及嵌套值类型、ZST 字段、计算属性、成员组合和下游再次消费。依赖默认值中的字段和 callable 使用定义时保存的 typed 声明与完整类型，实例化不重新要求公开 namespace 导入路径，也不再次证明模板引用集合。

默认值依赖按定义处已解析的 typed target 保存，`DefaultDependency` 不要求消费 Cone 再取得 namespace 导入路径。定义处实际发生过的查找路径可以保留；生产器不为没有名称查找的字段、成员或支持声明补造 witness，也不保存全体依赖的第二份导入路径表。Unit、Any 与其他声明遵循同一规则。共有 reader 继续检查 provider、引用、类型、默认值正文及实际声明关系，不从这份路径信息授予调用资格。

共有 HIR `cross-cone-interface/28` 明确此默认值依赖合同，保留默认值既有字段及 `SourceDeclaration` tag 3，不增加或复用 tag；`/21` 及更早版本退役，旧产物和缓存须重建。profile fingerprint 按实际 descriptor 更新，runtime C ABI、String 表示与 GC 契约保持。读取边界核对源码上下文及位置后，实例化复用同一不可变记录，保留定义位置与调用处求值位置。

MIR 从完整 HIR 声明与实际物化正文生成 callable 导出。公开声明、继承接口需要的 protected 成员、实际 slot root 与选中的源码 implementation，以及 logical property 的 getter/setter 均按其语言用途处理；Storage/Constant accessor 不产生函数 binding，Body/AbstractSlot 保留正文或 fatal trap。抽象 override 保留自身 typed declaration，外来声明按实际 provider 留在依赖侧，object 成员的逻辑 receiver 使用源码 object exact。

无参数的普通函数、extension、final nominal 成员和需要正文的 accessor 使用普通 callable 表，保留完整 exact signature 与 GC effect；其余成员、constructor、trap 和生成的适配函数使用相应的 lowering 记录。两类记录通过同一个 typed target 查询，实际重叠的源码 callable 只导出一份。某个 nominal 的字段布局仍为 source-only，不会取消已经合法的普通函数签名，也不能为它补造 type-bridge binding 或 descriptor。reader 在产物边界检查声明、实际 implementation、签名、GC effect 和 lowering role 的一致性；dispatch、selected closure 和初始化消费已经检查的记录，不复制签名或重复建立来源证明。缺失定义、重复 typed target 及签名不一致仍是产物错误。

共有 property 声明完整保留每个 accessor 的源码实现形式。`hir/cross-cone-interface/28` 的 property 声明仍为八字段，field 6 的 accessor 集合改用 tag 4=ReadOnly、tag 5=ReadWrite；field 1 为 getter，ReadWrite 另有 field 2 为 setter。每个 accessor 是恰含 field 1=typed accessor id、field 2=实现形式的二字段 product，实现形式采用 unsigned 1=Storage、2=Constant、3=Body、4=AbstractSlot。旧身份单项形式的 tag 1、3 退役，tag 2 不复用；public lookup 的 capability 及 setter 可见性含义不变。setter 若存在，身份与实现形式必须同时存在；getter/setter 可各自为 Storage 或 Body，不能由 property 的 RuntimeAccessor 总类推断它们都有正文。Const 必须是只读 Constant getter；AbstractSlot property 的 accessor 全部为 AbstractSlot，RuntimeAccessor 不含 Constant 且不能全部为 AbstractSlot；各 accessor 的 AbstractSlot 形式还须与同一共有 callable 声明的 Abstract modality 一致，Storage/Constant 必须没有 dispatch slot。producer 从同一次 Export HIR 的实际 getter/setter 实现投影，MIR callable 生产与 reader 通过该共有声明区分需要正文的 Body/AbstractSlot 与直接读取的 Storage/Constant，不枚举 MIR 候选来补来源。此变更不序列化正文或 LocalConcrete id，不授予私有/受限 accessor 额外访问权，不改变 persistent identity、runtime ABI 或源码行为；旧 section major、profile/fingerprint 与缓存必须重建。

实际 Box、Unbox、is 与 as 操作的形状依赖由共有 HIR external reference 中的 Expression type site 查询：只接受 TypeTest 或 BoxedValue 角色，且该位置完整 exact 必须恰为当前 reference 的 source nominal，结构类型的 nominal 成分不能冒领结构类型的 helper。查询从同一 identity graph 核对真实 provider，按完整 typed source id 去重，并由 HIR selected、MIR producer 与共有 MIR/LIR reader 共用；普通签名、仅存储、SizeOf/AlignOf、未展开默认值和未物化泛型正文不产生形状用途。MIR 将这些实际根关联到提供方 ShapeSupport，闭合原 source exact、boxed、coroutine-step/slot 等有限 helper；LIR 将其关联到同一 provider 的 ShapeSupport，并沿现有完整合同闭合 layout、descriptor、dispatch 和 scan。不能只保留 ManagedValue layout、从候选 selected 或外部 descriptor arena 反推 HIR 根，也不能在 consumer 重新定义提供方 helper。缺失、额外、错误 provider/source 与结构类型成分冒领均由完整集合比较拒绝；这一关联不改变 wire 或 runtime ABI，实际访问、初始化用途、物理引用与最终 Compile/Link 校验继续按原完成门核验。

MIR 的依赖闭包按实际 provider 和 typed target 查询完整类型、callable、dispatch、object、shape 与初始化记录。普通 callable 导出保存完整 exact signature 和 GC effect，与 constructor、adjust 等需要不同 lowering 表示的记录共用查询；同一 implementation 只保存一份。dispatch 和 selected 可以引用普通导出中的实际成员，不受历史 section 分区限制。生产侧从已物化使用及导出语义边生成闭包；reader 在外部字节边界检查 canonical 次序、唯一 provider/target、引用可达性及 selected 完整性，后续阶段复用这些完整记录。闭包不反向推导源码使用，也不承担额外来源或操作资格证明。

MIR type bridge 的类型表示与有限 shape-support 从同一产物的共有 HIR 声明重放。reader 按所有权先解析原七字段中的 types 和 shape-support，保留其余尚待检查的 callable、dispatch、object、initialization 与 selected transport；不复制或重新解码 types，不把这一中间状态作为完整机器消费资格。各 provider 只借用 manifest 中真实可达依赖的已检查类型组成表，沿原 identity graph 完成 HIR facts/representation/inheritance 与 MIR 的关联。源码类型 inventory 来自完整共有 representation 闭包及当前 provider 拥有的 Unit/Any；逐项核对 source/generated origin、GC/ZST、完整字段和 variant 的声明序、exact 字段类型、CLayout、InteriorMutable、class modality、base/interface 与 object backing。有限 helper 的源码根从同一 public surface 和共有 materialization closure 取得，shape-support 与相应 MIR helper 必须精确覆盖；无关或 foreign helper、遗漏源码类型、额外类型和跨表差异均拒绝。此关联不增加来源授权 section、调用方 source factory 或平行声明副本；原 MIR section wire 不变，callable、实际 selected use、LIR 与最终双 view 的完整校验继续作为发布条件。

共有 struct 源码形状完整保留声明的 `@InteriorMutable` 标志。`StructSourceShapeV1` 的既有 tag 3 改为四字段 closed product：field 0=tag、field 1=完整声明序字段、field 2=CLayout policy、field 3=interior_mutable（canonical unsigned 0 或 1）。普通与必要支持声明、嵌套 protected 源码支持均使用这一份形状；标志不由字段大小、GC/ZST、CLayout、可见性或 provider 身份推断，未标注声明显式为 0。producer 从同一次 sealed Export HIR 的 struct attributes 投影，读取方保留该标志供普通类型查询和 MIR 表示关联使用，不能默认补齐缺失字段。缺字段、旧二/三字段格式及非 0/1 标志均拒绝。共有接口升级为 `hir/cross-cone-interface/28`，承载嵌套源码形状的类型接口同步升级为 `hir/cross-cone-type-semantics/7`；required inventory、profile fingerprint、HIR fingerprint 与缓存同步失效并重建。该变更不修改已有语言的内部可变性规则、persistent identity 或 runtime ABI，不增加独立来源授权记录。

共有类型位置同时保留已物化表示和生成语义的必需类型依赖。`hir/cross-cone-interface/28` 在既有 type_sites 和类型中增加 tag 6=FieldStorage、tag 7=EnumVariantFieldStorage、tag 8=ConstructorInitializerResult、tag 9=InitializationCycleMessage；各为三字段 product，field 0 为 tag，field 1 分别为 PersistentFieldId、PersistentEnumVariantFieldId、实际 CallableMaterialization、PersistentInitializationUnitId，field 2 为完整 exact。这些位置直接归属于原有字段、variant payload、constructor 或初始化单元，不另存来源授权记录。LocalConcrete class field 与 struct field 一样直接保留其持久化字段身份，不由字段名或 arena 下标重建。producer 只遍历真实物化类型闭包内的字段与 payload；class initializer 的结果按语言语义为 Unit，实际初始化单元的循环错误消息按其已解析协议使用 String。未物化 enum、未展开默认正文和仅作查找的类型身份不产生记录。读取方沿同一 foundation 检查 typed owner、真实 provider 与 definition origin，生成位置还须核对 constructor/初始化角色及相邻 MIR 的实际依赖。共有 external references 在完成位置、来源与 foreign nominal 分发校验后，可临时查询完整的实际外部类型依赖；该查询不增加 wire 表，也不单独授予机器层消费能力。producer 从同次完整 typed IR 汇集实际类型用途；共有 HIR reader 核对完整 foreign nominal 分发、重复位置、typed owner 与 provider 后，物化查询及 MIR/LIR 复用同一记录。后续阶段继续核对各自实际类型表示、签名、ABI 与传递引用，不为重复证明 HIR 使用来源而重算和比较整份类型根；旧 major、未知 tag、错误 owner、错误生成角色和缺失身份均拒绝，profile 与 fingerprint 同步重建。

共有 HIR 外部引用保留同一次已物化 LocalConcrete 的表达式、签名与声明类型使用。`hir/cross-cone-interface/28` 的 ExternalHirReferenceV1 仍为六字段 closed product，必需 field 6 为 type_sites 数组；每项改为以 field 0 为 tag 的封闭和类型。tag 1=Expression，field 1～5 依次为实际 CallableMaterialization 正文根、根内 expression_index、ConcreteExpressionOrigin、表达式类型角色、完整 exact；角色 unsigned 1～5 仍为 Value、SizeOf、AlignOf、TypeTest、ArrayElement。tag 2=CallableSignature，field 1～3 为实际 CallableMaterialization、签名位置、完整 exact；签名位置为 field 0 的 tag 1=Receiver、tag 2=Parameter（field 1 为去除 receiver 后的声明序 index）、tag 3=Result。tag 3=LocalValue，field 1、2 为 PersistentLocalValueId 与完整 exact。tag 4=BackingStorage、tag 5=DelegateStorage，field 1、2 为既有 typed PropertyOwner 与完整 exact。各分支字段必须完整，禁止以可选字段、虚构 expression_index 或 arena 序号拼接声明位置；旧五字段 type-site item、旧 major、未知 tag 及额外字段均拒绝，profile、HIR fingerprint 与缓存同步重建。

producer 从实际函数和构造器的 receiver、声明序参数、结果，正文内 source-backed local value 及实际 global storage 投影声明分支，保留 private 签名、未读取的参数/局部变量、全局声明和初始化相关 generated callable 的类型。局部捕获沿既有 PersistentLocalValueId 指回同一声明，重复观察只能在 exact 完全一致时合并；synthetic temporary 由其实际表达式/生成签名承载，不伪造源码声明。仅作前端操作声明的 Intrinsic 函数不产生签名根，其实际操作由表达式记录承载。GenericSourceMetadata 的未展开默认正文不加入该集合。数组按 variant、完整 typed 位置和表达式角色严格排序，组内重复位置及冲突 exact 拒绝；外部引用 role 8=ExecutableTypeDependency 当且仅当 nominal target 拥有非空 type_sites 时出现，包含已物化实体的签名与存储依赖。

每个位置沿 exact key 的结构子类型保留全部实际 foreign nominal；跨 nominal 的同一位置必须保存完全相同的记录。reader 使用同一 identity graph 与真实可达依赖重算这一分发集合，验证完整有序集合、target/provider、表达式 definition/evaluation 来源，以及声明位置在当前 foundation 的 callable、local-value 或 property 身份及源码归属；NoSubstitution 以外的物化继续由既有 ODR capability gate 拒绝。声明分支沿既有 canonical key 与 definition origin 查询，不重复保存一套来源证明。委托属性的生成 accessor 在进入其真实 callable 源码上下文后生成合成 thisRef、storage read 与协议调用；正文构造结束或诊断退出均恢复外层上下文，不能让合成操作沿用外层 File 求值来源，也不放宽 reader 的正文归属检查。source-point 完备步骤继续从同一 interface 的 call_sites 和 Expression 分支收集 definition/evaluation span 端点，与已有声明/default 来源点合并；真实 UTF-8 源码生成本地映射，依赖来源只使用已有 canonical source record。类型记录本身不授予名字查找、访问、调用、TD 或机器资格；binding witness、call_sites、实际 type/member/slot/use 与后续 MIR/LIR selected join 仍分别验证，不从 MIR、候选 selected、layout 或 primitive arena 反推根，也不新增调用方 source factory 或平行来源表。

共有 HIR 外部引用在 `hir/cross-cone-interface/28` 中保存实际源码调用位置。ExternalHirReferenceV1 的 field 1～4 为 origin、typed target、roles 和名称解析路径，field 5 为 call_sites。每个 call site 的 field 1～5 为实际 CallableMaterialization 正文根、expression_index、ConcreteExpressionOrigin、完整逻辑实参 exact type 序列和 result exact type；field 6 区分 SourceBinding 与 SourceDeclaration，field 7 保存适配前的 SourceCallReceiver。SourceBinding 使用 tag 1，引用实际名称导入路径的非空 canonical u32 index 集合；成员与实际声明调用使用 tag 3 的 SourceDeclaration，直接关联 typed declaration 与 provider，不要求额外导入路径。原 CastFailure reason tag 2 退役。生成的异常构造使用完整 typed 操作和真实声明引用，不复制一份源码 call-site 或独立协议证明。

extension receiver 为第一个逻辑实参，重复参数、Unit 和 ZST 均不得省略。call_sites 按正文根与 expression_index 严格排序；一个位置只记录一次调用。producer 从完成默认参数展开和物化后的 typed IR 收集记录，未物化正文不产生使用。reader 检查格式、引用、表达式归属和调用签名，并将已经确认的调用与 MIR/LIR selected 及 Link 引用关联；不得用额外证明记录替代实际声明和完整签名。缺字段、重复位置、越界路径索引、错误 provider、声明或签名均拒绝。版本早于本 capability 的 artifact 与缓存需要重建；runtime ABI 不变。

本节替代旧的独立 HIR source-authority transport 草案。`cross-cone-type-source-authority/1` 不进入 required inventory，不实现其四域封装或额外授权链，也不把它改名或嵌入其他 section 继续作为第二份来源证明。此前为该草案定义的独立 transcript 不构成普通类型使用或发布的前置条件；必要信息合并到实际拥有相应声明、类型、参数、默认正文或 selected use 的共有 metadata。现有 constituent 可以复用其完整 typed 数据与校验算法，不能保留相同事实的独立授权副本。

类型生产器直接返回完整的共有 type section；MIR 使用已有继承边、槽序、typed 实现目标及成员引用。类型物化和构造器选择查询已有共有 nominal/callable 声明，不另行投影来源 foundation、完整 protected 声明、构造器签名或参数协议来重复证明同次产出。删除这些副本的生产工厂、凭证外层、只用于副本的 reader 及测试。必要的类型、引用、继承、ABI 与格式检查保留在实际消费边界，未变化的数据复用已有检查结果。

所有可见性的 nominal、callable、property、参数、默认值和定义环境由共有源码接口完整保存。protected 成员仍以 typed 引用参与实际 MIR callable 选择，其可见性和签名读取同一声明；构造器使用共有 nominal 的 constructor 引用及对应 callable，不在 inheritance record 再保存一份 payload。generic 词法 owner 不因访问域查询而要求 machine exact type。名义类型的 lookup/inheritance/slot 三份派生域不再保存和重验。

`CrossConeTypeSemanticsSectionV1` 保留 field 1、2、3、8，field 4～7 退役；`NominalInheritanceInterfaceV1` 保留 field 1～4、7～9，field 5、6 退役，退役字段不复用。成员引用的 Constructor tag 2 随重复构造器通道退役，实际 constructor 始终使用共有 typed 声明。HIR `cross-cone-type-semantics/7`、required inventory、profile 与内容 fingerprint 同步更新，旧产物和缓存需重建；不保留旧来源副本的双轨兼容，不改变 runtime C 调用约定或 String 表示。

producer 从同一次 sealed Export/LocalConcrete HIR 及真实 winner-commit 结果生成这些 metadata；普通查询和 reader 使用同一数据模型。bytes-only reader 不需要源码、编译器 token、调用方另行提供的来源 factory，或一份与发布表平行的自证 transcript。reader 验证 provider 所声明的完整语义与机器产物的一致性，不认证源码目录或要求 provider 证明其声明未被用户修改。

layout profile 的共有 HIR 声明读取直接消费同一 artifact 的 identity、foundation、production 与 public/source metadata。它与既有 ordinary reader 共用 definition source、nominal、property、callable、alias、参数/default、const、public route 和 external reference 校验；每个 provider 只使用其显式依赖闭包中已验证的声明，完成这些共有检查只证明声明与引用闭合，新 type semantics、MIR/LIR 和最终 object 的关联仍须分别完成，不能将解码成功或生产侧对象代替这些检查。

非泛型 typealias 的声明、目标实体归属、public 可见性和外部引用在共有 HIR 声明边界检查。别名展开随后只处理实际 typed alias 目标、缺失目标及循环，不保存或查询逐边授权表，不重建依赖的 import/re-export 路径。已完成依赖的展开结果直接参与当前 Cone 的查询，共享最终 SignatureTypeKey；不能为同一别名链重复重走全部依赖。M23-6 完整 reader 保留展开结果供前端名称解析使用，生产与读取都不依赖测试专用来源工厂。

共有 param-free 签名解析递归使用实际 nominal 声明和既有 ExactTypeKey，保留 tuple 顺序、函数 effect、指针所指类型与 native calling convention。普通 nominal 查询不依赖当前 core 协议是本地还是导入。此查询只提供完整类型身份，实际 callable、布局、ABI 和依赖关系使用既有记录；不新增来源、授权或重复证明表。

`Alias(...)` 验收包含实际 provider 导出的别名、本地指向外来 nominal 的别名以及链式组合。前端在候选状态中展开实际类型，沿共有查询选中目标的构造器；命名／默认参数、ZST、大值 ABI 和下游再次发布使用相同的真实声明。源码产物由第三个 Cone 消费并链接运行，失败候选不得遗留消费记录。不新增格式字段或 runtime ABI。

共有校验必须保留以下信息与约束：

- identity、definition origin 和 source context 只使用同一 artifact 与依赖闭包中的完整 typed 记录。local declaration 的 key origin、词法 owner、definition subject 和 provider 一致；external 引用按实际 provider 唯一解析。SourceNominal、ExactType、Function、GenericFunction、Constructor、Property、Accessor、DispatchSlot 与不同生成实体不得混用；不得从 FQN、符号、同布局或“唯一匹配项”补身份。
- 普通 nominal metadata 保存真实 kind、modality、own binder、bounds、supertypes、完整字段/variant 及必要 member/constructor/child 关系。private backing field、object backing class、setter 与默认正文支持声明不能因不参与 public lookup 而丢失必要类型或语义。source-only generic 声明保留完整源码合同，不伪造 exact、representation、inheritance 或机器定义。
- GC/ZST facts 由完整表示结构重算并与 fact 表一致；source value shape、MIR 表示无关 shape、LIR field/layout/scan 逐阶段按 typed owner 和 exact type 关联。字段与 enum variant 保持声明顺序；普通 canonical 集合按 typed key 排序且拒绝重复。布局不能反推源码 nominal kind、generic 参数或访问权限。
- constructor、callable、property 与 accessor 共享真实声明的参数名、参数顺序、值类型、结果、effects、modality 和访问域。getter/setter 的 logical property 与 role 由 typed key 决定；参数/default/vararg 协议与同一声明一致，零参数声明也完整。protected nested support 沿真实词法关系保留所需子树及完整源码合同，不能把 owner 受限的 declared-public 成员改标为 protected。
- inheritance、virtual family、interface conformance、slot schema 与实际 implementation 选择来自同一次已解析的 HIR 关系。完整接口声明序、typed override 抑制、Function/Getter/Setter 角色、abstract/concrete/default 状态、effects、参数/结果及访问域仍按第 4、5、8 节重放；不能按名称重新选择实现，也不能以另一项可适用实现代替实际选择。
- 默认值只保存一份完整 typed 正文、参数协议、definition origin、binder、六类引用、receiver occurrence 与 nested callable/capture。原 provider 与发布声明属于实际参数关系；前端完成定义处及继承导致调用域变化时的语言检查。reader 检查编码、typed 引用、owner/binder 范围和正文与引用索引一致性，不再保存或重建逐引用访问域证明。
- 实际 HIR use 保留 winner-commit 产生的稳定 occurrence、typed target、lookup/access/receiver 上下文及依赖边；去重后的 selected 集合与这些真实 use 闭合。MIR/LIR 的 selected、semantic import 和 physical use 按实际消费继续关联验证。声明存在、进入候选集、provider 可达或 source-only metadata 可见，都不能代替实际使用，也不能授予机器执行能力。
- 相邻表、所属 provider、public/inheritance 可见面、完整需要集合和跨阶段引用保持互斥与覆盖校验。缺失、额外、错 owner/provider、错误角色、访问域不合法、错误签名、布局/scan 不一致和不支持的物化均拒绝。失败不交付部分可查询的 checked closure。

当前 Cone 的形状物化需求由共有公共声明和实际本地使用产生：按 export binding 的实际 exporter 选择 DeclaredCurrent 声明，再应用第 1.2 节的完整物化闭包 gate。HIR 保存必需的 LocalShapeSupportPlan；MIR 接收同一 typed source 集合并核对实际 source/exact 与 helper；LIR 核对实际 producer 的完整 layout、scan、descriptor 和 registration。alias、reexport 和仅作源码 metadata 的声明不自动增加本地机器根。所有 Cone 共用同一计划和 reader 校验，compiler protocol 的 Defined/Imported 角色不决定形状需求。

layout profile 的 HIR 类型基础读取从共有 nominal/callable 声明重算当前可物化集合，并从完整字段、variant、继承和公开/受保护构造参数闭合必需 exact facts；Unit/Any 由其既有定义 owner 保留。struct/enum/tuple 的 GC/ZST 事实继续使用共有递归算法，引用值作为 managed leaf，不以对象字段判断值的 GC-free 或大小。external nominal 只查询实际 provider 已验证的事实，缺失、额外、本地冒领外来事实、错误 intrinsic family 和 by-value cycle 均拒绝。representation 的 owner、来源位置、词法 owner、visibility、完整字段/variant、CLayout policy、base 和 object backing 与同一共有声明及事实关联，这些检查不替代 dispatch、protected/default、selected use 或机器产物闭合。

### 3.2 共有数据迁移与既有 section 边界

- identity-foundation 的布局/scan/dispatch key 继续只证明 identity；本阶段新 payload 引用它们，不重复声明同 kind/id。
- `NativeBoundaryTypeDefinitionRecordV1` 只服务原 extern/callback witness，不通过它提供一般 field/scan/TD 查询。
- core与其他Cone共用通用MIR/LIR shape-support表。每个source root均由当前provider的typed source声明及完整类型、layout、descriptor记录验证，完整section核对独立source-root集合的精确覆盖。不得因provider为core而要求空表、跳过验证或委托旧core表；MIR查询与依赖选择直接读取同一通用表。LIR production字段8直接保存所有producer共有的有限shape-support计划数组，删除Core/NotCore包装；source、closure root与definition owner必须属于实际producer，reader从独立source集合完整重算，不能为通用section补齐缺失记录或提供第二份root来源。MIR CoreMirBridge的重复shape_support_roots及wire字段2已删除；本地物化直接消费HIR提交的完整source声明并验证MIR实体，reader从共有公共声明推导LIR义务。
- M23-5 已开放 callable 子集的语义继续保留；其有效 nominal 分类与 MIR/LIR 关系验证复用共有实现。core 专用发布与 Link 分区按补充设计删除，不能为保留旧分区而拒绝合法的共有 provider 查询。
- 新 callable bridge 只承载上述旧集合以外、现在可证明的 target；MIR与LIR的所有外部callable在各自stage共用一个实体、typed id和arena，MIR输出与sealer复用引用及重复implementation校验，LIR同一body不得重复；旧、新metadata分区从实体的明确选择角色投影。完整类型证明可以被两类 bridge 共用。callable 的有效既有 bridge 与通用 bridge 按明确 subject 能力分工，初始化服务迁入共有选择，不再优先进入 core 专用协议 bridge；shape的layout、scan、TD与registration不因出现在production形状计划中而被整组排除，按共有shape-link校验实际provider与definition；新开放的 core member/constructor/shape use 若不属于旧 bridge 的固定集合，也走新 bridge，不能借此扩大旧集合。
- 所有外部TD在LIR中统一为`ExternalTypeDescriptor`及一个arena，必需保留实际provider、exact、symbol和definition；Local/External引用只表达本地定义与外部引用。String 角色由 well-known typed 引用保存，其 wire、选择与验证使用共有 descriptor 路径；其他外部TD（包括core普通类型）必须通过通用layout selection，V1缺少对应layout selection时明确拒绝，V2保留实际provider并重放完整semantic/physical selection。codegen统一发射和校验外部描述符，foundation与layout投影共用实体，不维护第二套core TD。通用ExactDispatch从callable ABI实际provider推导Local/DependencyExternal引用，core普通dispatch不降为旧协议CoreExternal。
- `strong-production/11`与`strong-production/12`共有的字段8改为直接shape-support计划数组，旧Core/NotCore tagged sum拒绝读取，已有产物必须重建。新profile只生产/要求V2；有效的 identity、definition plan、digest DAG 与 image plan 继续共用；专用协议 bridge 字段和包装按补充设计删除并同步 inventory/fingerprint，不能以旧 top-level 字段数冻结阻止迁移。V2 的 TD/dispatch 外部引用统一显式保存实际 provider；runtime registration/image的C ABI不改变。

本阶段不为清理来源资格改变 container/outer schema 或 `persistent-v1` mangler，不重用退役 tag。删除专用包装、字段或 variant 时同步对应 capability/profile inventory 与 fingerprint；不兼容产物拒绝并要求重建。旧 core 专用通道不因历史冻结条款继续生产。

`strong-production/12`中的三个版本化constituent固定为：

```text
StrongTypeDescriptorRefV2 =
    Local(PersistentExactTypeId)                      // tag 1
  | DependencyExternal { provider, exact }           // tag 3
// Retired tag 2 is not emitted or accepted by the completed profile.

StrongTypeDispatchCallableRefV2 =
    Local(PersistentCallableBodyId)                  // tag 1
  | Runtime(RuntimeFunction)                         // tag 3
  | DependencyExternal { provider, body }            // tag 4
// Retired tag 2 is not emitted or accepted by the completed profile.

OptionalStrongTypeDescriptorRefV2 =
    Absent                                           // tag 1
  | Local(PersistentExactTypeId)                      // tag 2
  | DependencyExternal { provider, exact }           // tag 4
// Retired tag 3 is not emitted or accepted by the completed profile.
```

前述保留 variant 的 payload 复用既有编码，包括 optional Absent 的 `{0:1,1:0}`；退役的 CoreExternal 不进入新 profile；新增variant是`{0:tag,1:provider,2:exact_or_body}`。body是`PersistentCallableBodyId`，必须反向join同provider已验证的Strong owner、ABI export与definition，不能只比较symbol。TD parent、itable interface key、vtable/itable entry及引用它们的type-registration semantic plan统一使用V2，不保留另一个可矛盾的V1 plan。DependencyExternal 使用实际 provider 与完整 typed target 查询共有的 layout/ABI 和物理定义；String 的 TD 与 type-registration 引用沿同一导入记录解析。已在导入边界检查的定义直接供对象 relocation 与 fingerprint 使用，不从协议服务表重复恢复 owner 或证明来源。初始化 callable 也按实际声明引用解析；不允许把真实 parent 写成 Absent 再由额外记录补齐。core 与其他 provider 的外部引用均使用 DependencyExternal；reader 不以默认 CORE 值补齐退役格式。descriptor/dispatch dependency fingerprint使用同一完整V2 relation；旧variant的canonical bytes保持，新variant追加tag，不能遗漏provider或把foreign target编码成local。

除补充设计明确退出的专用 constituent 外，既有 strong-production 与 runtime registration 的 typed key/record 继续复用。Compile/Link handler先按capability解码V1或V2，再取得不可降格的validated production view；旧Link identity section只消费其既有子集与同一member全集，新physical use由11.3覆盖。profile拒绝同时出现两个strong-production版本，避免两套definition authority。

V2另扩展initialization dependency的验证域：wire仍是原顺序/排序契约下的`PersistentInitializationUnitId`序列，不改变id或record字段；内存证明改为`LocalUnit(ref) | DependencyExternalUnit { provider, unit_ref }`。旧V1只能解析本Cone producer unit的规则保留。V2的foreign id必须命中同一显式dependency closure中已选中的provider unit、对应initialization descriptor与required definition，producer unit表仍只含本地定义，不能复制foreign canonical unit record冒充本地。missing/错provider/跨closure dependency在artifact commit前拒绝；不允许省略真实edge来让旧validator通过。

### 3.3 编码约定

本文新 record 使用 M23-2 Wire CBOR v1 closed product：field 从 1 开始按文中声明顺序编号，sum 使用 field 0 的 tag，payload field 从 1 开始；新增 sum 的 tag 按列出顺序从 1 递增。所有引用既有类型的地方复用原编码，不重新给 `Effect`、`GcEffect`、exact key、symbol request、definition plan、scan fingerprint 或 canonical ABI 编号。

table 按 kind-specific typed 主键的 canonical bytes 严格递增；set 排序去重，重复输入拒绝而非 reader 自动修复。参数、字段、variant、base prefix、slot position 是有序语义序列，不按名称重排。各 constituent 按实际输入范围、格式字段宽度与 checked 算术读取；分配使用真实容量并处理失败，图遍历检查非法环，不设置 text/bytes/count 或展开成本配额。

文中用于数据完整性和 typed 引用解析的内存类型不写入 wire；wire 保存实际声明、typed id、类型、布局与 ABI 数据。正常产物边界完成必要检查后，后续消费复用同一完整数据，不再包装操作资格或重建来源证明。

layout profile 的继承图与 nominal 访问域从同一共有声明重放：所有源码 nominal（含 source-only 泛型）保留词法 owner 与来源；只有已闭合表示的 nominal 进入 exact 继承图。reader 逐项核对继承表的完整 owner 集合、modality、直接 base 和 interface，再复用共有算法拒绝缺失父节点、final base、错误 kind 与继承环，并核对声明 lookup/inheritance 域和 object/backing 关系。依赖按实际 provider 及其已登记身份解析，图仅在读取作用域内借用，不增加 wire、来源凭证或发布资格，也不替代 slot、protected/default 和 selected use 的后续闭合。

共有 inheritance 记录保留实际继承边、dispatch schema、slot 合同及 protected 成员 typed 引用。成员索引从 nominal 的完整 member/child 关系和 callable/property 的实际 visibility 得到，setter 按自身 visibility 判定；不能将 declared-public 的受限 owner 成员改标为 Protected。构造器直接查询共有 nominal constructor 引用及对应 callable 的 owner、签名、effect、默认值与可见性，不再建立或闭合第二份构造器表。产物边界拒绝缺失、额外或错 owner 的真实成员引用，并保留实际继承图、slot、默认值、ABI 和链接检查。

layout reader 直接使用共有 HIR 中完整的声明、参数协议与默认值。受限可见性成员和嵌套声明都通过实际 typed owner 查询同一记录，不另建 protected 声明、嵌套支持副本、参数协议表或默认值表。

layout reader 的 inheritance slot 合同逐项关联共有声明：root 与 target 的 typed identity、实际 owner、完整 signature/effect、modality 和 access source 继续复用既有 slot identity、receiver、domain、schema 与 abstract-obligation 验证。实际 implementation 必须与同一 owner 的共有 dispatch selection 精确相等，完整槽集合与已验证的 schema 一致；不得根据名称、可适用签名或另一项 default 重新选择，也不能从正在检查的 implementation 字段反推。getter 与 setter 分别核对，final override、reabstract 与 inherited default 均保留同一次 resolved HIR 的实际结果。源码层 generic 声明保存完整源码选择，M23-6 concrete slot 只消费已物化的 param-free owner。完整 selected-use 与 MIR/LIR 的类型、签名及布局引用仍须闭合。

layout reader 使用共有 HIR 中已检查的 typed 默认值正文、参数位置与定义环境，不生成受限默认值副本或访问资格 profile。正常调用和默认值展开直接查询实际声明及引用；表达式 receiver 直接取自同一正文，不为它重建另一张证明索引。

共有源码接口完整保存 protected 声明和 source-only generic owner 的成员、构造器、property、const 与 enum variant。类型 reader 使用共有声明中的 kind、modality、binder、supertypes、shape、visibility 和 typed owner，不再递归重验 protected/nested support 副本。泛型词法 owner 下的静态嵌套 nominal 仍保留完整源码和默认值；M23-6 仅按实际表示与签名选择 param-free 机器构造入口，不能从构造参数不含 binder 推断泛型 owner 已物化。常量值直接读取共有 property 的完整 const 值，并按其真实类型与 provider 处理，不复制来源或伪造 public lookup。

共有 nominal 的实际派发选择纳入 `hir/cross-cone-interface/28`：`NominalDeclarationDetailsV1` 在既有 field 1～6 后增加必需 field 7，保存按 typed slot id 严格排序、无重复的完整 `dispatch_selections` 数组；每项为 field 1=PersistentDispatchSlotId、field 2=实际选择的两字段 product。选择复用既有源码选择格式：tag 1=Abstract 无 payload，tag 2=Concrete 与 tag 3=InterfaceDefault 在 field 1 保存完整 typed Function/Getter/Setter declaration。owner 由 enclosing source nominal 唯一确定，不复制 exact owner、来源 proof 或授权 transcript。producer 从同一次 sealed HIR 的 virtual family、FinalOverride、interface member override 与已解析 MethodApplication 投影，保留实际实现；generic owner 的普通 method 仍保存 Function/Accessor identity，source-only 选择不要求 exact 或机器物化。layout reader 将可物化 owner 的完整槽集合及每项选择与 inheritance contract 精确关联，并继续按共有 callable、继承图与访问合同核对其合法性。外来实现目标进入共有 external HIR reference 的 InheritanceDependency（新 tag 7），按实际 provider 与 typed declaration 验证依赖闭包，不伪造 source-name binding witness；槽 identity 继续通过真实声明、dispatch order、继承与 schema 图关联。缺失 field 7、旧六字段记录、重复或乱序 slot、错误角色、缺失/额外选择以及以另一合法实现替换已选目标均拒绝。空集合显式编码；旧 `/9` 产物及缓存重建，profile、inventory 与 HIR fingerprint 同步更新；outer schema、persistent identity 和 runtime ABI 不变。

共有 nominal 声明的 dispatch 顺序属于声明 metadata：`hir/cross-cone-interface/28` 在 `NominalDeclarationDetailsV1` 的原 field 1～5 后增加必需 field 6，保存 `NominalDispatchOrderV1`。tag 1 的 NonVirtual 无 payload，仅用于 struct/enum；tag 2 的 Class 在 field 1 保存按当前 owner 的方法声明序排列、无重复的 virtual-family slot id，包含该 owner 的 override 引用；class/object 均使用此分支。tag 3 的 Interface 在 field 1 保存有序、无重复的 direct-parent source signature，field 2 保存有序、无重复的 `{ slot, overrides }` 成员，后者复用既有 InterfaceSourceMemberV1 的两字段结构，overrides 仍为 canonical typed slot 集合。generic source 声明保留相同的完整 source 顺序，不为此建立 exact node。parent 与共有 supertypes、slot 与共有 callable 的同 owner 关系须完整相等，不能通过排序填补缺失顺序。class vtable 从已验证 base prefix 开始，按声明序追加尚未继承的 family；interface schema 继续按 direct-parent 声明序作后序遍历，按 typed override 关系抑制被覆盖成员。构造器、protected、default 与实际实现选择继续使用原共有声明及闭包，不另建来源授权表。旧 `/8` 产物与缓存重建；outer schema 和既有 identity 不变，Compile capability inventory 与完整 HIR contribution 同步变化。

## 4. HIR：type facts 与继承接口

嵌套声明的源码 payload 只保留实际声明身份与完整源码接口，不重复携带可由该身份推导的 inheritance exact、representation owner 或泛型/非泛型资格标志。是否存在机器表示由同一产物的实际 representation、inheritance、MIR 与 LIR 表表达；源码完整但尚未物化的声明仍可正常发布。reader 在这些表的消费边界检查引用与表示一致性，不因解析嵌套源码声明再次完整验证同一表示。原 payload 的 field 3 退役且不得复用；HIR `cross-cone-type-semantics` capability 升至 5，旧产物按版本规则重建，runtime C ABI 不变。producer 的类型事实与表示投影必须覆盖共有声明接口中实际需要的私有存储与嵌套支持类型，不能只依据 public binding 根的局部列表。

实际直接调用的 receiver 在参数适配之前保留静态类型。Export 与 LocalConcrete HIR 的普通 Call、ImportedDependencyCall 使用必需的 SourceCallReceiver：NoReceiver 或携带完整静态类型的 Receiver；top-level 函数及构造调用使用前者，extension 与 nominal member 使用后者。类型代入、default 复制和导入物化必须映射原类型，不能从适配后的首参数或声明 owner 补出。共有默认表达式的 Call 同时保存该信息：原 tag 44 退役，新 tag 57 的 field 1、2、3 分别为原 callee、arguments、receiver；receiver 采用 tag 1 无 payload 的 NoReceiver 或 tag 2、field 1 保存完整 signature type 的 Receiver。共有实际 call-site 在原 field 1～6 后新增必需 field 7，采用相同 receiver 和类型化 exact id。reader 从已验证声明独立核对 receiver 的有无与使用种类，逻辑参数及结果仍逐次按适配后签名验证；原始 receiver 另参与类型关系、成员选择及访问闭包，不授予访问或机器资格。拒绝缺失、错误角色、无实参的 receiver 与 runtime 构造携带 receiver。此变更将共有接口升级为 hir/cross-cone-interface/28，仅保存类型关系的类型接口同步升级为 hir/cross-cone-type-semantics/7；旧 major、旧 Call tag 和旧六字段 call-site 拒绝，inventory、fingerprint、cache 与 golden 同步重建。runtime ABI 和 persistent entity identity 不变。

共有 selected 的 MemberCall 用途从实际 SourceBinding call-site 逐次推导。完成同一次调用的逻辑签名核对后，nominal Function、Getter、Setter 分别保留完整 typed 声明；accessor 的角色从真实 provider 的 canonical PropertyAccessorKey 读取。receiver 使用参数适配前保存的静态 exact，沿实际共有 nominal 声明的 class base 与 interface 边证明其可到达声明 owner；每次调用先验证，再对完整用途去重，不能因同一目标已有正确调用而省略其他位置。选中记录的 provider 是成员声明所属 provider，不能使用 receiver 所属 provider；当前 Cone 的派生类型调用外来基类或接口成员也遵守此规则。原始 receiver 与声明 owner 的 Signature/Representation 及传递需求分别闭合；构造调用、top-level 与 extension 不产生 MemberCall。source object exact 不由 backing exact 替代，generic/source-only 类型继续遵守 M23-7 物化边界。producer 与 Compile/Link reader 使用同一从共有来源推导的集合，候选表在这一用途分区必须精确相等；缺失、额外、错误 provider、声明角色或 receiver 均拒绝。此检查不替代成员访问、slot/dispatch、MIR 实现与机器布局闭包，不新增 wire 字段或版本。

实际 extension 调用的原始 receiver 同样须独立重放类型可应用性，不能因适配后的首个逻辑参数已匹配声明而省略。expected receiver 从实际 provider 的共有 extension 声明取得，actual receiver 使用 call-site 保留的适配前 exact；按语言规范 3.1、3.2、8.1.1、8.6 核对 exact 相等、到 Any 的上行关系、真实 nominal class/interface 继承，以及同挂起性和元数下函数参数逆变、结果协变。tuple、原生 pointer 与 native function pointer 不按元素或签名生成隐式型变，整数也不因位宽或表示相似建立子类型关系；generic/source-only 物化继续受 M23-7 限制。每次调用分别验证，失败保留原调用位置、actual receiver 与 expected exact；构造及无 receiver 的 top-level 调用保持各自角色，extension 不生成 MemberCall。两端类型与其传递需求仍从共有来源完整收集，派生关系不能用同名、同布局或 provider 标签替代。producer 与 Compile/Link reader 复用同一检查；函数类型关系的重复子查询只在本次调用中复用，跨调用不跳过校验。此项不替代实际适配操作、装箱/函数 adapter、访问或机器 ABI 闭包，不新增 wire 字段或版本。

不含泛型参数的普通 top-level 与 extension function/accessor 的 exact nominal 签名分类同时包含语言内建 Unit 和 Any；两者使用固定 canonical typed declaration identity，provider 沿该 declaration key 的真实 origin 取得，不要求补造普通 nominal 声明记录，也不能按名称、布局或当前 provider 代替身份。Any 在 receiver、参数、结果以及默认值实例化中始终保留自身 exact type，参数适配前的实际类型与普通上行转换仍分别保存和核对。已选择调用必须闭合共有 Signature/Representation 用途、实际 MIR implementation、完整 logical signature、LIR ManagedValue layout 与 canonical ABI，Compile/Link reader 按同一入口重放；相同物理 ref 表示不能替代 Any 与其他 nominal 的语义身份。此分类仅补齐已有共有表示的语言内建类型，不授予成员、构造、generic/ODR、结构签名或 native 调用资格，不新增 wire 字段或版本。

共有 canonical Scoop ABI 正规化从已经注册的 exact key 识别语言内建 Any，按目标的 managed pointer 布局计算其大小、对齐、GC 属性和 direct 参数/结果形式，并保留完整 Any exact identity；Any 同样参与既有非空 managed-reference niche 判断。该固定语言表示不依赖 native-boundary witness 或普通 nominal arena，计算本身不产生 provider definition、成员或布局消费资格，实际可达依赖及 ManagedValue 布局仍须由共有产物闭包核对。缺失 exact key 继续拒绝；其他同名 nominal 仍查询自身声明。C ABI 中按值传递 Any 继续按既有 managed-reference 规则拒绝。

### 4.1 type semantics section

```text
CrossConeTypeSemanticsSectionV1 {
    exact_facts: CanonicalVec<ExactTypeFactsV1>,
    representation_support: CanonicalVec<NominalRepresentationSupportV1>,
    inheritance: CanonicalVec<NominalInheritanceInterfaceV1>,
    // Fields 4, 5, 6, and 7 are retired.
    selected: CanonicalVec<SelectedExternalTypeUseV1>,
}

ExactTypeFactsV1 {
    exact: PersistentExactTypeId,
    kind: Value { zst: ZeroSized | NonZero } | Reference,
    gc: GcFree | ContainsManagedReferences,
}
```

共有源码接口完整保存所有必要声明、参数协议、typed 默认值正文和定义环境。protected、private 与默认值支持声明使用同一记录。type-semantics section 不再复制 protected 声明、受限参数协议、默认值或来源并集；field 4～7 退役，field 1、2、3、8 保持原编号。前端完成语言与可见性检查，reader 核对编码、typed 引用和 owner/binder 关系，不建立另一套来源资格或完整语义重放。此次格式变化纳入 `cross-cone-type-semantics/7`，旧产物需重建，runtime C ABI 不变。

源码位置由共有接口的 definition_sources、call_sites、type_sites 与已有全局定义记录汇集，不能为收集位置重建独立默认值正文、参数协议或访问证明。删除 NominalDefaultSourceProductionV1、DefaultSourceBodyProductionV1 及旧 DefaultSourceTemplate／References／Access 数据模型、适配器和专用测试；生产路径直接复用共有接口的完整默认值及位置收集。必要的来源位置、typed 引用、可见性、参数与 binder 规则仍保留。旧模型已不属于正常产物字段，此清理不增加格式分支或改变 runtime ABI；内容 fingerprint 依实际产物数据计算。

默认值引用是普通依赖索引：六类记录各保留真实 typed target 与 definition origin，正文与索引在读取边界核对一次。定义处的名称、类型、effect 与调用域覆盖规则由前端负责；继承默认值遇到类型代换或调用域扩大时检查实际变化，未变化的事实直接复用。producer 不重建访问域，reader 不再分别重放 type、value、callable 与 direct/slot 的访问证明；产物仍检查实际 provider、typed 引用、owner/binder 范围、局部值范围及跨表一致性。字段、构造器与全局值引用在共有 reader 边界对照已解析声明的实际 owner、种类和作用域；局部函数引用必须对应正文携带的声明。复用已验证的身份图与正文索引，不重新推导访问域。默认引用 record 采用两字段 map，field 1=target、field 2=definition_origin；旧 witness 的 field 3 退役且不复用。共有接口升级为 `hir/cross-cone-interface/28`，旧 `/24` 及更早产物与缓存重建，profile 与内容 fingerprint 同步更新。运行时 C ABI、String 表示及必要 GC 契约不变。

删除没有生产实现者的完整 HIR semantic-authority 总入口、默认引用 envelope 平行验证器和公共类型支持凭证；正常 reader 使用已有各表的格式、引用与类型边界检查。仅验证旧凭证构造、访问域重放和证明状态的测试随之删除，实际源码的可见性错误、默认值继承与跨 Cone 展开验收继续保留。

删除仅由测试实现的默认值operation-typing、nested ABI及root/origin语义工厂和其证明数据、平行验证入口与专用测试。正式reader继续使用共有声明表、完整typed模板、类型与binder检查、来源位置、局部数据流及真实引用一致性检查。局部数据流直接借用模板与共有字段查询，删除重复body input及authority适配器；nested descriptor保留实际类型化身份、parent/path与binder数据，删除独立Standalone证明模式。语言操作规则由前端负责，不在IR/meta crate再复制实现。此清理不改变wire字段、profile版本、runtime C ABI或String表示。

参数自由依赖class通过共有名义声明取得真实身份、modality、直接父类型和声明序字段，引用类型按GC契约传播，递归引用字段不展开成递归值布局。HIR保留完整声明及解析后的字段、父类型，不重建同名本地声明。外来class构造仍是对真实constructor的typed调用；HIR→MIR消费已选MIR绑定中的ClassInitializer角色，使用共有class分配路径创建对象，再将该对象作为receiver调用实际provider的初始化实现。构造表达式返回分配的对象，物理initializer返回Unit；普通返回class的函数不走构造分支。本地与外来initializer共用Callee表示、参数求值顺序和GC处理。 core默认导入层的构造调用和静态限定名同时查询普通Type/Value binding；用户新增的class、enum和typealias与其他依赖使用相同声明路径，不限于预设内建名字。此能力不增加来源凭证、core分支或runtime ABI，沿用现有类型、MIR绑定及LIR布局格式。 M23-6的真实class运行在单个最终镜像中链接实际产物及runtime；LLVM stackmap段按各对象贡献的完整v3 blob逐个读取，长度由已有count和对齐决定，保留每份格式、记录唯一性与精确PC检查，不增加展开预算。该读取不要求M23-8的多镜像启动或M23-9的program-link。

field 1～3 的内容由本 provider 的完整 typed HIR 与共有声明决定。foreign nominal 的 facts、representation 和 inheritance 从实际 provider 读取，不复制为 consumer 拥有的声明；本地 inheritance record 可以引用真实 foreign slot root/implementation，图查询使用 provider 的继承边。递归 GC/ZST 事实同样查询实际定义。Tuple 等结构类型可以作为当前编译实际需要的 support 保存，不能按 exact key 的形状猜测 Cone。共有 reader 检查声明引用、类型关系与 selected 依赖；不另存来源全集或证明链。

源码声明的保留范围由实际名称、类型、默认值和布局依赖决定。generic source-only 声明保存完整 typed identity、binder 和词法 owner，具体机器表示仅由实际物化需要决定；不建立独立来源根凭证。

type section 与同 provider 的共有源码接口在正常产物边界检查字段、typed 引用、类型及 ABI 关系。成员、构造器与默认值均解析到共有声明，reader 不对 protected/constructor 副本重新执行同一检查；缺失或错误的真实声明引用仍拒绝。

这些事实覆盖本 Cone 导出的 param-free exact subject，以及其必须供下游检查的表示/继承 support；generic template 不能伪装成 concrete facts。`Reference` 的 `gc` 固定为 ContainsManagedReferences，描述的是该引用值；对象内部 `object_scan` 可以为空，二者不能混淆。`Value/ZeroSized` 必须是 GcFree。enum 每个 variant 的 gc flag另随 representation record保存，并重放 `gc_free(enum) = AND(gc_free(variant))`，不是对ContainsManagedReferences取AND。

value `ZstStatus` 在 HIR 完成：只有 Unit 或全 ZST 的普通非 CLayout struct/tuple 为 ZeroSized，intrinsic scalar/pointer 与 enum 不从空字段推断。MIR 机械转写；LIR 验证 status 与 layout 一致，不能用 `size == 0` 为缺失 HIR 信息补值。

`NominalRepresentationSupportV1` 的主键是 param-free source nominal 的 `PersistentTypeId`，保存 kind-specific 源码表示：ordinary struct 的声明序 `{ field, exact/signature type }` 和 CLayout policy，enum 的声明序 variant/payload 与 variant gc，class 的已解析 base 及声明序 backing/delegate field，object 的 backing-class 关系，intrinsic 的 typed representation family。本阶段不导出generic template的representation support，尤其不能为依赖binder的enum variant填入无条件concrete gc flag。字段仍可用既有`SignatureTypeKey`引用fully concrete的generic application；这些引用不能包含binder，选中请求闭包若需要ODR生产仍由M23-7 gate拒绝。generic template的表示实例化与对应物化证明留M23-7，不能增加Unknown/default flag。

其wire固定为 `{ 1: owner, 2: declaration_access, 3: shape }`。field type只保存一个`SignatureTypeKey`，不平行保存可矛盾的exact/signature字段；concrete消费通过已验证binder-free转换取得exact id。shape的tag1～6依次为：`Struct { fields, c_layout_policy }`、`Enum { variants }`、`Class { base, declared_fields }`、`Interface`、`Object { backing_class, declared_fields }`、`Intrinsic { representation }`。field是`{ field_id, value_type }`的declaration-order product，enum variant是`{ variant_id, fields, gc }`；source struct field与class backing/delegate field沿用既有`PersistentFieldId`，但必须经不同的checked wrapper隔离，并逐项验证foundation `FieldIdentityKey`中的`Declared`与`PropertyBacking`/`PropertyDelegate`角色；enum-variant-field使用独立的`PersistentEnumVariantFieldId`，三种字段用途不能混用。这里不新增persistent id家族或改变既有identity wire。CLayout policy与intrinsic representation复用对应IR的封闭语义模型并显式wire映射，不以annotation文本、短名或bool替代。

普通 public struct/enum 的 public representation 与 M23-5 source shape 保持一致。class 的 private/internal backing field 保存于完整声明 metadata，供布局计算使用，不加入普通 public member lookup；布局查询与源代码成员访问分别遵守各自用途的规则。

CLayout policy使用独立sum：tag1为`Ordinary`，tag2为`CLayout { contract }`，field1的contract复用HIR语义product`{ aligned, packed }`（field1、2）；各`HirCLayoutValue`使用无payload的sum，tag1～6依次为`Natural/A1/A2/A4/A8/A16`。intrinsic source representation family使用tag1～7的`Integer { signedness, width }/Boolean/String/Array/MutableArray/Ptr/FunPtr`；Integer的field1、2分别保存HIR signedness和width，二者均为无payload的sum，tag分别为`Signed=1/Unsigned=2`和`W8=1/W16=2/W32=3/W64=4`。family记录source nominal的封闭表示类别，实际generic application的element/pointee/function type仍由既有exact/signature key提供；不能从family省略应用参数或取得ODR生产能力。这些是新section constituent的显式编码，不修改native-boundary的CLayout/ABI编码。

Class shape的`base`复用既有`OptionalSignatureType`编码，`Absent`表示无class base，`Present`只接受binder-free的nominal或nominal application signature；其exact class关系在inheritance closure中重放。Object shape的`backing_class`是`PersistentTypeId`，必须逐项等于`GeneratedNominalKey::ObjectBackingClass { object: owner }`派生的identity，不能借此把object的source identity当作backing class identity。

### 4.2 inheritance surface

```text
NominalInheritanceInterfaceV1 {
    owner: PersistentExactTypeId,
    modality: Final | Open | Abstract | Interface,
    direct_base: NoClassBase | ClassBase { exact },
    direct_interfaces: CanonicalVec<PersistentExactTypeId>,
    slots: CanonicalVec<InheritanceSlotContractV1>,
    protected_members: CanonicalVec<ProtectedDeclarationRefV1>,
    slot_schemas: CanonicalVec<InheritanceSlotSchemaV1>,
}
```

该表保存实际物化类型的继承边、构造入口和 dispatch 合同。protected 声明携带与同类 public 声明相同的完整 owner、source signature、parameter/default interface、effect、modality 和 property/accessor 关系。名称解析依据声明可见性、词法 owner 和实际继承关系决定访问，不将记录存在性包装成额外的访问凭证。

构造器完整签名、参数、默认值、effect、可见性和 owner 保存在共有 callable 声明中，nominal 的 constructor 引用保持其 typed identity。MIR 按实际物化 owner 与签名选择 Public/Protected 构造入口，enum 构造继续使用 typed variant plan。inheritance record 的旧 field6 和 `InheritanceConstructorInterfaceV1` payload 退役；private/internal 构造器所需的本地初始化与委托实现仍由正常 body/Strong 依赖保存。

继承表按 owner exact 严格递增；record 平铺 field1～4 的 edges 及 field7～9 的 slots、protected_members、slot_schemas，field5、6 退役且不复用。producer 从完整 typed HIR 与共有声明构造必要继承和 dispatch 数据。slot 合同恰好覆盖该 owner 所有 schema 的 slot 并集，root 和 target 指向实际 typed callable，override 使用已解析的实现选择。MIR 直接消费这些关系并查询共有成员与构造器声明。

名义类型不再编码 lookup、inheritance、slot 三份派生访问域。可见性查询直接使用声明 visibility、typed lexical owner 和继承关系；generic 词法 owner 保留其实际 `SourceNominalId`，不因查询访问规则而要求不存在的 machine exact type。实际 slot 合同仍保存其 root 声明的访问域。persistent domain 是 `Empty | Conjunction(constraints)`，空 conjunction 表示 universal；constraint 为 `Cone(ConeIdentity)`、`File(SourceIdentity)`、`LexicalOwner(kind-specific nominal id)`、`SubclassesOf(exact class)`，按 tag/id 严格排序，不保存 session ClassId 或 visibility 数字大小。

每个 protected/support 声明都保存 `DeclarationAccessSourceV1 { declared_visibility, lexical_owners, definition_origin }`：visibility 是独立 closed enum `Public=1 | Internal=2 | Private=3 | Protected=4`，owner 链按 outer→inner 顺序保存 typed nominal id，origin 复用 Stage5 `ExportDefinitionSourceV1`。这些字段用于实际名称解析、诊断和声明引用检查；后续阶段复用已完成的检查，不重新构造并比较三份名义类型访问域。

语言9.1.5的receiver限制只属于目标method/property/accessor自身声明的protected权限；其effective lookup中的外层nominal protected约束仍只用于词法域重放。不能把nested类型的普通成员receiver代入外层class的SubclassesOf条件。getter 与 setter 分别按实际声明检查 protected receiver 规则。

nominal 的 lookup 由声明 visibility 与全部 owner 的可见性共同决定；Final class、struct、enum、object 不可继承，Open/Abstract class 与 interface 的继承遵守既有语言规则。这些规则由负责类型检查的实现按实际查询处理，不将派生结果存入名义类型记录，也不保存固定为空的 nominal slot domain。每条 `InheritanceSlotContractV1` 保留实际 root-slot 域及声明引用，用于必要的 slot 一致性检查。

`ProtectedDeclarationInterfaceV1` 及其独立 callable/constructor/property/nested payload 不再进入 type section。普通可见性、签名、effect、参数和默认值直接读取共有源码声明；删除仅用于建立和重验这些副本的工厂与语义入口。

继承表中的 `ProtectedDeclarationRefV1` 是实际成员索引：tag1 为 Callable，tag3 为 Property，tag4 为 NestedNominal，field1 保存对应 kind-specific typed id；Constructor 的旧 tag2 退役且不复用。Callable 继续区分 Function、GenericFunction 和 Accessor。引用集合按 canonical key 排序且唯一；成员声明从共有 callable/property/nominal 表解析，MIR 仅按需要选择具有完整机器签名的目标，不通过引用取得额外来源资格。

共有 nested nominal 声明保留 `SourceNominalId`、binder、词法 owner、父类型、成员、子类型与 source shape。source-only 的泛型声明和未物化非泛型声明仍可完整发布。嵌套访问、protected receiver 与 getter/setter 可见性由前端按实际声明处理；泛型实例化仍属于 M23-7。const 值和 enum variant 参数使用各自真实 typed 声明及共有 source/default 记录，不建立第二套 nested support 容器。

public property 的 protected setter 以真实 `PersistentPropertyAccessorId` 保存在共有声明中；property 的公开 getter 和受限 setter 保持各自可见性。调用端通过实际 property/accessor 关系选择 getter 或 setter，并消费其 typed 签名。

`InheritanceSlotContractV1` 保存：既有 `PersistentDispatchSlotId`、声明 owner、function/getter/setter 的 typed declaration origin、完整 signature/effect、slot contract domain，以及 `Abstract | Concrete(target) | InterfaceDefault(target)`。每条 slot 与 canonical slot key 关联；override 引用原 slot identity，不以相同方法名或新 owner 重造 base slot。

slot合同wire为field1～7的product：`slot, declaration_owner, declaration, signature, domain, implementation, declaration_access`。本阶段source owner为param-free `PersistentTypeId`；declaration使用`Function/Getter/Setter`（tag1/2/3、field1为对应typed id），getter与setter须分别匹配foundation accessor key及slot role。signature为`{ exact_signature, effects }`（field1、2），复用既有`ExactCallableSignature`与`CallableSourceEffectsV1`编码，双方execution必须相等，receiver必须存在且精确对应source owner。function参数与foundation的binder-free source signature逐项join；getter无显式参数，setter恰有一个参数并返回Unit。implementation为tag1的`Abstract`或tag2/3、field1含target的`Concrete/InterfaceDefault`。target为field1～5的`{ declaration, owner, signature, modality, declaration_access }`，保留实现自身的source owner、完整签名/effect、modality与source access，不能用root-slot receiver冒充实际实现receiver；target不能为Abstract，InterfaceDefault分支只接受interface default modality。原始slot与target的参数/result/execution以及安全性、GC、operator、infix合同逐项相等；intrinsic与普通Scoop body的实现类别可以不同，source extern不进入dispatch support。这里的GC相等比较的是两个source声明的合同，沿用本地override的source attributes相等规则；5.2允许的NoGc value目标到Managed入口只发生在生成的boxing/dispatch adjust层，由两份lowered signature表达，不能据此接受GC source合同不同的override。调用的实际receiver/boxing或dispatch adjust由5.3的typed实现关系承接。

`NominalInheritanceInterfaceV1` 编码七个字段：field1～4 和 field7～9，field5、6 退役且不得复用；field9 为 `slot_schemas`。合同表 `slots` 按 slot id 的 canonical bytes 排序。`InheritanceSlotSchemaV1` 是 `{ role, slots }`（field1、2）：role 为 `ClassVtable`（tag1）或 `Interface { interface_exact }`（tag2、field1），slots 是无重复且保留 provider 语义顺序的 `PersistentDispatchSlotId` 序列。schema 表按 role 的 canonical bytes 严格排序；它定义 exact owner 和 typed table 下的 dispatch 顺序，不保存 byte offset、LLVM 类型或程序级全局 ordinal。同一 slot 可以出现在多个 interface schema。derived ClassVtable 保留 base 序列为 prefix，override 只替换实现，不改变 slot identity 或位置；MIR 按序读取 signature 与 implementation。

必要的 internal concrete slot 可以作为不可 lookup/override 的继承 support 保留，使下游 table 保留其已有实现。private member 永不进入继承/dispatch；public owner 不得留下下游不可实现的 hidden abstract obligation。internal/private concrete owner 中的 public override 可以填充更宽 slot，但不因此成为普通 foreign lookup target。

公开继承入口及其传递 base/interface/slot 依赖由定义方完整导出。protected nested type 保留自己的 identity 与 owner chain，名称解析按实际 subclass scope 检查可见性；不能通过 `public import` 再导出成普通 binding。

### 4.3 protected access 与 default

前端依据语言规范 9.1.5 检查 protected：词法位置必须在声明 class 或其 subclass body，显式 receiver 的静态类型必须是当前访问 subclass 或其 subclass；只知道运行时对象可能为 derived 不满足规则。static nested、companion 和 object 的词法/继承关系按真实 typed owner 查询，不凭借 lookup 名称授予访问。property 读取与赋值分别检查 getter 和 setter 域，setter 不可见立即诊断。

constructor delegation、implicit this、explicit receiver 与 super 保留实际 typed target 和 receiver；成员使用的 identity、slot、签名、effect 与继承关系完整。必要 support 不能作为普通 import，public re-export 不能扩大 protected 可见性。

默认值在定义处完成名称解析、类型、effect、数据流和访问检查；继承默认值沿唯一实际 provider 及 binder mapping 保存。完整参数 metadata 区分 Required、Default、VarargEmpty、VarargDefault，default 正文保存参数位置、typed locals、结果、receiver、定义来源、必要嵌套 callable 及原声明引用。原始 scope、实际 owner arguments 和 definition/evaluation origin 在实例化中保留，不从候选签名或同名声明恢复。

M23-6 的机器使用限于成功矩阵中的完整 param-free 关系。generic 源码 metadata 保持完整而不提前物化；已有前端实例化与类型代换按真实用途复用，不引入只供测试的通用工厂或另一套语言语义实现。

### 4.4 HIR selected set

HIR 类型接口的生产入口必须借用同一 artifact 的共有 foundation、已验证 identity graph 与 HIR interface，以及实际可达依赖的同类元数据，不能固定发布空的 selected 表。Signature、Representation 与 TypeTest 三类类型用途从共有 external reference 的实际 type_sites 逐次读取：调用签名与 constructor initializer result 贡献 Signature，类型测试保留 TypeTest，其余值、存储及 sizeOf/alignOf 等用途贡献 Representation；初始化循环消息保留其实际 String 参数依赖。完整 exact 及源码位置继续保留在原 type_sites 中，持久化 target 只对实际 provider 与 nominal exact/use kind 去重。Signature 和 TypeTest 均需要同一类型的 Representation；闭包从实际类型用途及当前类型导出所需的参数自由名义声明继续沿完整字段、enum payload、直接继承及 constructor/slot 签名递归，包含 private storage 和无自身物理槽的实际 interface implementation。每个 provider 的共享声明决定其 source-only 边界，不能通过外来 selected 或同名类型绕过；Unit/Any 仅沿固定语言身份处理。共有 Compile reader 独立重算这三类用途并与候选表的对应分区精确比较，拒绝缺项、额外项、错误 provider 及仅在无关 artifact 中存在的依赖。不增加来源 wire 表；这项类型依赖闭合不代替各次 source lookup/access、receiver/default/support parent 证明，以及操作用途和最终 Compile/Link 资格。

共有 HIR selected 同时从当前可物化声明的完整直接父边重建 Inheritance 用途。derived 必须属于当前 provider；target 的实际声明 kind 决定 ClassBase 或 Interface，完整 typed derived、target exact 与 terminal provider 一并保留。每条当前直接边独立处理，相同父类型但不同 derived 不得合并；本地父边不产生 external selected，递归读取依赖声明的父边只贡献 Representation，不冒充当前声明的直接继承。父类或接口自身的完整表示与签名依赖继续沿原闭包展开。源码泛型或不能完整物化的声明不生成该用途，Unit/Any 内建身份不构成继承资格。producer 与共有 Compile reader 使用相同声明查询重建四类用途，Inheritance 与 Signature、Representation、TypeTest 的候选分区须精确相等，缺失、额外、错 derived、错边种类、错 provider 及不可达依赖均拒绝。声明来源、访问域、modality、override 与 slot 关系仍由原共有继承重放验证；该扩展不增加来源记录、wire 字段或机器资格，后续五类操作用途和完整 Compile/Link 验证继续属于发布条件。

HIR 中保留为 Cast 节点的普通与可选运行时类型转换显式保存实际 checked type；Export HIR、LocalConcrete HIR、泛型默认值实例化和默认正文运输均保留这一必需字段。结果仍分别为 T 或 Option<T>，读取方必须验证其与 checked type 的完整类型关系，不能从结果类型反推或默认补齐目标。默认表达式 Cast 的既有 tag 37 使用四字段 closed product：field 0=37，field 1=完整 operand，field 2=checked_type（SignatureTypeKey），field 3=optional（CanonicalBoolean）；缺少 checked_type 的旧格式拒绝。该目标参加原默认正文的类型、引用、访问域、来源遍历；MIR 直接消费实际目标，不从 Option payload 重新解析。

共有表达式 type_sites 的角色 1～5 保持含义，unsigned 6=BoxedValue，unsigned 7=SingletonValue（见 object value 与初始化规则）：Box 保存 operand 的完整 exact，Unbox 保存实际 payload 结果 exact；Cast 与 IsInstance 均以 TypeTest 保存完整 checked exact，可选转换不以 Option<T> 冒充目标。每次操作继续使用自身正文根、expression_index 和 definition/evaluation 来源，记录加入原 external reference 的完整 foreign nominal 分发。共有 HIR selected 查询从这些实际 TypeTest/BoxedValue 操作重建 ShapeSupport：只有整个操作 exact 为外来 source nominal 时，才对该实际 owner/provider 请求完整有限支持；结构类型的内部 nominal 仍参加原类型依赖，不能冒充整个操作的 helper owner。相同 helper 目标可去重，但各操作位置及原类型记录仍全部验证；本地类型不产生 external ShapeSupport。读取方独立重算并精确比较 ShapeSupport 分区，拒绝遗漏、多项、错 provider、错 payload 和借用结构成员身份。该步骤不授予构造、成员、槽或单例的源码操作资格，最终 MIR/LIR、object 与 Compile/Link 关联仍须完成。

上述转换字段与表达式角色更新将共有接口升级为 hir/cross-cone-interface/28，仅保存类型关系的类型接口同步升级为 hir/cross-cone-type-semantics/7；required inventory、profile fingerprint、HIR fingerprint 和缓存同步失效并重建。旧 major 不能混入 Compile 或 Link view；persistent identity、runtime C ABI 与 capability kind 不变。

运行时类型转换的失败构造使用前端解析的实际异常类型与 constructor 引用，并沿共有的类型、callable、ABI 和 Link 路径消费。删除由 Cast 反向投影的独立 CastFailure call-site、RuntimeOperationDependency role，以及 reader 对同一目标再按 compiler protocol 进行资格判断的通道；普通源码调用的位置、参数、结果与 typed 引用检查保留。共有 HIR 格式更新为 `hir/cross-cone-interface/28`，原 call-site reason tag 2 与 external-reference role tag 9 退役，不复用；旧产物、profile fingerprint 与缓存重建。该调整不改变转换失败抛出 ClassCastException 的语言行为、runtime C ABI 或 String 表示。

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

`mir-lower` 提供单一的完整 export 组装入口，按同一次 HIR/MIR 输入依次生产 source 与有限 helper type、普通 callable/constructor、object 初始化 callable、boxing adjust、derived equality、dispatch 和 shape-support 六张组成表。依赖 type/callable/schema 通过共有只读索引参与关系检查，不复制到本地 export；initialization-use 表由同一次 sealed HIR 的实际已物化调用独立投影，组装接口不接受调用者提供的用途表；组装时继续核对 local unit 的真实 MIR 物化 root 与 typed provider 关系。旧 callable bridge 已发布的实际 typed implementation 只在旧表保留，组装器逐项核对 signature 后将其从新 callable 表排除；不按 core、名称或 ABI 形状猜测分区。重复 implementation、错 Cone 输入或缺失依赖均返回完整错误，不返回部分组成表。此入口产出完整 export 数据，最终七字段 section 按本节组装实际依赖闭包，Compile 与 Link 复用共有语义读取结果，不增设来源授权框架。

`ParamFreeMirTypeExportV1` 保存 `{ exact, origin, facts, representation, base_and_interfaces }`。origin是`SourceNominal(PersistentTypeId) | GeneratedNominal { nominal, role }`，generated role与foundation canonical key逐项相等，不伪装成source声明。representation 是 MIR 自有的 closed sum：scalar/intrinsic、struct、enum、class、interface、object backing 与 generated helper；field、variant、base、capture 使用对应 typed id 和 exact type。它包含可重放布局的完整顺序和 CLayout policy，但没有 byte offset、LLVM type、stride、scan 或 ABI pass mode。

该新 constituent 的 product 使用 field1～5，顺序与上述五项一致；type 表按 exact id 的 canonical bytes 严格递增。producer 可以对已验证 record 排序，reader 必须拒绝重复及乱序输入，不能替 artifact 修补。origin 的 tag1 是 `SourceNominal`（field1 nominal），tag2 是 `GeneratedNominal`（field1 nominal、field2 完整既有 `GeneratedNominalKey`）。exact key 必须逐项等于 `Nominal(origin.nominal)`，source 分支只能引用 parameter-free source declaration；generated 分支必须匹配同一 canonical generated key。`ObjectBackingClass` 的 key 由 HIR foundation 提供并保留在已验证 identity graph 中，其余有限 helper 同时要求存在于 MIR foundation，不能把 HIR-owned backing class 强行补进 MIR 的生成类型表。

facts 为 `{ kind, gc }`（field1、2）。kind 的无 payload sum tag1～3 为 `ZeroSizedValue/NonZeroValue/Reference`，gc 的 tag1、2 为 `GcFree/ContainsManagedReferences`。ZST 必须 GC-free，reference 必须含 managed reference；integer/boolean intrinsic 固定为 nonzero GC-free，空 ordinary struct 为 ZST，enum 保留 tag 因而 nonzero，enum 整体 GC-free 等于全部 variant GC-free 的合取。非空 aggregate 的完整事实由 HIR/LocalConcrete join 提供，不能按“字段数为零”分类 intrinsic，或以 type constituent 的局部核验代替该 join。

representation 的 tag1～10 依次为 `Intrinsic/Struct/Enum/Class/Interface/ObjectBacking/BoxedValue/CoroutineStep/CoroutineSlot/Object`。Intrinsic 的 field1 保存固定 param-free family；Struct 的 field1～3 为声明序 fields、CLayout policy、interior-mutable 的 0/1 unsigned 值；Enum/Step/Slot 的 field1 为语义序 variants；Class 的 field1、2 为 class kind 与 declared fields；Interface 无 payload；ObjectBacking 的 field1 为 declared fields；BoxedValue 的 field1 为唯一 payload field；Object 的 field1 为 backing exact。class kind 的 tag1～3 为 `Final/Open/Abstract`。field product 为 `{ field: PersistentFieldId, value: PersistentExactTypeId }`；variant product 为 `{ variant: PersistentEnumVariantId, fields, gc }`，其 fields 使用 `{ field: PersistentEnumVariantFieldId, value: PersistentExactTypeId }`。field/variant 序列不按 id 重排；重复 id、错误 owner 和 variant-field owner 均拒绝。

固定 param-free intrinsic 的 tag1～4 为 `Unit/Integer/Boolean/String`；Integer 的 field1、2 为 signedness 和 width，tag 与 4.1 一致。generic intrinsic family 不进入该 sum。CLayout policy 的 tag1 为 Ordinary，tag2 为 CLayout，后者 field1 保存 `{ aligned, packed }`；值的 tag1～6 为 `Natural/A1/A2/A4/A8/A16`。这只是源 policy，不携带目标布局；空 CLayout struct 被拒绝。base-and-interfaces 的 field1 保存 `None`（tag1）或 `Base`（tag2、field1 exact），field2 保存 provider direct-interface exact 的 canonical 集合，按 exact id 严格递增。该集合不承载 dispatch 顺序；后者独立来自 4.2 的 slot schema。base 只可属于 Class、Object 或 ObjectBacking，目标必须是 source class exact；interface 目标必须是 source interface exact，重复、乱序和自身关系拒绝。完整继承闭包及 object/backing 两侧关系的一致性由 section 的 HIR/MIR join 核验。

BoxedValue 的 payload exact 与 `BoxedValue { payload }` key 逐项相等，field id 必须由既有 box payload field key 派生。Step/Slot 的 variant 与 payload field id 复用既有 generated enum keys，严格保留 7.2 的两 variant 顺序。三个 helper 的 subject 只能是 source-root exact，不能递归触发生成闭包。closure environment、frame 及 callable/continuation adapter 的执行型 shape 在本 constituent 明确按相邻阶段 gate 拒绝；不以任意 Class 表示伪装导出，也不为这些 shape 新造 machine 字段。这里只建立可核验的 type constituent；完整 section、selected closure 与 production artifact 资格仍必须满足本章其余各节和 11.2。

`ParamFreeMirShapeSupportV1` 是上述有限闭包的表示无关索引，其新 product 的 field1～5 固定为 `{ source_nominal, exact, boxed, coroutine_step, coroutine_slot }`。后两项是必需的 helper exact；boxed 的 tag1 为 `Available`（field1 helper exact），tag2 为无 payload 的 `ReferenceNominalRequiresNoBox`。value 必须取 Available，reference 必须取后一分支；不得用可选项表示缺失能力。source nominal 与 exact 必须指向同一完整 source type export；三个 helper exact 分别 join 同一 types 表的 generated key、representation、payload exact 与 GC facts，Step 的 Completed 和 Slot 的 Value 的 GC facts 必须等于 source，另一个 variant 必须 GC-free。helper 不能充当 source，错误 role 或不同 source 的同形 helper 不能替代。

shape-support表按source nominal的canonical bytes严格递增。所有producer（包括core）的每项source必须属于当前provider；生产侧从同次 MIR 已绑定的 source/helper 计划生成完整表；读取侧在类型组成表边界核对实际 HIR 需求、helper 和 GC 关系。通用MIR表统一保存source、exact、box、coroutine step与slot关系，producer及reader共用相同helper角色、GC与typed归属校验；删除Core/Ordinary root来源枚举及section内第二份core shape缓存。缺少source、helper或所需表项均报共有错误，不能另建或从旧CoreMirBridge补齐第二份root清单。该 MIR 五字段索引不复制 7.2 的 LIR 八 role product，后者保存并检查布局、scan、TD、registration 与 definition。

record 之间的查询可以借用本地完整表及已验证 dependency 表组成的只读索引，索引不编码进本地 export wire。type、callable、schema 索引分别使用 exact、Strong target、owner exact 的独立 key 空间；相同 key 出现在两个输入表中也必须拒绝，不能以“内容相同”吞并重复定义。dispatch 的 base prefix、继承 interface schema 与 implementation 查询均使用这个完整视图，而待导出的 canonical schema 表只保存当前 provider 的 records。索引承载 constituent 的关系查询；selected 引用按实际可达 provider 与 typed target 解析。

MIR 生产与读取共用已有的 `MirTypeBridgeDependencyViewV1`：它借用真实 provider 的完整导出表、初始化单元和普通 callable 定义。driver 从已解析依赖图提供可达 provider 的目录；section 不再保存 producer section 组成的递归依赖图，也不要求读出的依赖重新构造为 producer section。选择入口保留 provider 唯一性与实际 typed 引用闭包检查，manifest 依赖环检测复用共有图边界。读取结果可直接用于后续 MIR 类型、callable、dispatch、初始化和布局生产。

M23-6 的正式 `scoopc` 发布与 `scoop` 依赖消费统一使用 `CrossConeLayoutStrong`。共有 reader 一次读入完整 HIR 类型语义、MIR 类型与 callable、LIR layout/ABI/dispatch 及真实对象；语义会话直接导入这些记录的实体映射，Compile 与 Link 引用同一完整结果。旧 M23-5 的独立提交、Link 重开及两份发布证明链退出正常路径，不把新 reader 的数据反向构造为旧 producer section。此前格式的依赖需重建；section tag 不复用，runtime C ABI 和 String 表示不变。

初始化与存储导入直接查询实际 provider 的完整 Strong 记录，核对实际 definition、symbol 与 ABI；移除 support-source 资格回调、空授权对象和重复 MIR 使用证明。源码使用与 typed 依赖闭包继续在对应语义边界检查。

当前源码的类型查询图直接保留 HIR/MIR foundation 已生成的 canonical 实体记录，按真实 IR layer 合并已读依赖图；不对同次编译的 foundation 再做 wire 编解码和哈希重建。正式 LIR lowering 从该图及实际 Cone coordinate 建立完整类型诊断目录。记录合并仍检查 typed identity 冲突；reader 继续负责外部字节的格式、引用和循环检查。

MIR section 直接消费同一次 HIR→MIR 已生产的六张完整导出表、初始化单元记录和 typed 依赖使用。删除独立来源工厂、重复预期表、逐表比较回调及 source-join 凭证；生产侧不再次重建或完整验证已经检查的记录。读取边界按实际 HIR 声明、MIR 定义与依赖目录检查格式、类型、签名、effect、可见性、实体归属和引用关系，保留已检查组成表供后续使用，不重建 producer section。依赖选择仍检查实际 provider、完整 typed target 和引用闭包。

普通 callable 导出在首次生成时根据真实声明解析 exact signature，并与实际 Strong body 的签名关联。组装类型导出时直接使用这些完整记录，不再次从 HIR 枚举、分类和比较同一普通导出全集。组装在生产其他 lowering 记录前排除已有普通定义；合并后的最终表统一排序并检查重复定义，不在合并中途再构建一张仅供重验的临时 canonical 表。外部 reader 对新读入数据的类型、签名和定义检查保留；本调整不改变 wire/profile、runtime C ABI 或 String 表示。

生产侧的类型与 shape 依赖直接取 HIR 已保存的实际物化使用记录，不再次遍历全部 HIR 来证明同一记录。外部 callable 从同次 MIR 实际引用取得；ordinary 分区已有的 provider、declaration、implementation 和签名继续复用，其余用途进入类型桥接闭包。初始化使用从实际操作生成一次。元数据中的未物化默认正文和无关源码声明不产生机器依赖。

mir-lower 的导出、初始化记录和依赖使用是普通数据转换输出。section 只组装这些完整数据、共有依赖目录和 selected 引用，不要求调用者实现来源工厂，也不另外重放本地六表的完整语义。初始化服务按已有语言角色参加普通调用路径。

完整 section 的语义引用使用独立 `MirTypeBridgeTargetV1`：tag1～6 依次为 `Type(exact)`、`Callable(Strong owner)`、`Dispatch(owner exact)`、`Object(value id)`、`ShapeSupport(source nominal)`、`InitializationUnit(unit id)`，均在 field1 保存既有 typed id/owner。该 sum 只是待闭合的语义目标，不是 Selected handle。selected record 是 field1、2 的 `{ terminal_provider, target }`；表按 provider 后 target 的 canonical bytes 严格排序且唯一，reader 不从该表反推 required use 全集。当前 HIR/MIR 的实际用途及本地 exports 的全部语义边共同决定闭包，terminal provider 必须提供自己的完整 local export；facade 的 selected relation 不能代替 terminal export。普通 callable 与需要独立 lowering 表示的 callable 共用实际 typed target 查询；重复定义或引用未提供的定义均拒绝。

语义边收集逐项覆盖 representation 的字段、variant、base/interface 与 Object backing，callable 的完整 semantic/lowered signature、role 与 generated relation；两份签名相同时只收集一次，dispatch 的 owner/base/interface provider、原 slot declaration、实现与调用 signature，object 的 source/backing/ensure/unit，有限 shape family 的全部 exact，以及显式 initialization-use 的 local/dependency unit 与 object cause。它不读取 foreign body 推断调用图，也不把 property accessor cause 无条件再登记到新 callable 分区。字段/variant 的嵌套 tuple 只在 transient storage 上下文递归到 nominal 叶子；其它 structural/generic 应用以及 signature/独立目标中的 tuple 仍在 M23-7 gate 拒绝。重复的语义边归为集合，而 wire selected 的重复输入始终是错误。收集器返回完整边集，按实际 provider 定义、ordinary callable 和初始化单元解析后形成 selected 引用。

初始化单元在 MIR 物化时已经确定实际 initializer、ensure、完整逻辑签名与 GC effect；导出直接复制这些完整记录，不再投影第二套来源签名或保存 ProducerEmitted／ReaderSemanticReplay 证明状态。reader 从实际声明的 unit key 和同一产物的 callable 定义恢复相同数据，并检查真实 provider、ordinary Managed、无 receiver/参数及 Unit 返回值。实际机器定义与 relocation 在 Link 对象边界检查；两种消费使用同一记录类型，七字段 wire 不增加冗余 unit-role 表。

MIR section 保留当前完整导出、初始化记录及可达依赖导出表的借用。每个 provider 在目录中唯一；实际使用根和导出引用共同决定 selected 闭包，缺项、多项、错 provider、本地目标误写为 selected 和错误引用均拒绝。LIR 继续消费实际表示、ABI 和机器定义；不通过 MIR 来源凭证取得额外资格。

本节从实际 callable 签名和导出关系收集依赖，不遍历 foreign body 推断调用图。slot declaration、dispatch 的源码 implementation 及 adjust 的源码 target 必须是实际 nominal member，直接 owner 是无类型参数的类型，receiver 不得是 extension receiver；generated target 按其实际 adjust/derived role 处理。普通导出中的 final member 同样可以成为 dispatch 或 selected 的目标，查询直接借用该完整记录，不另存一份 binding 或维护旧分区身份名单。object ensure 仍只指向同 unit 的 generated ensure，顶层或 extension 函数不能冒充 nominal slot。

M23-6 的外来类型与 shape 使用由 HIR 在实际物化时保存为完整 typed 记录，覆盖函数与构造器签名、正文局部值和表达式、存储、nominal 表示及显式 shape 操作。类型测试、sizeOf/alignOf、构造 initializer 的 Unit 返回值及初始化 ensure 所用的 String 同样保留；未物化默认正文和无关声明不加入。MIR 直接使用这些记录及真实 provider，不从整个 primitive/exact arena 或 selected 表反推使用，也不再次完整遍历 HIR 以比较两份相同来源集合。

### 5.2 callable 与 constructor

`ParamFreeMirCallableBindingV1` 保存 `{ source_or_generated_origin, implementation, semantic_signature, lowered_signature, lowering_role }`。implementation 只接受既有 `StrongCallableDefinitionOwner`；lowering role 为 ordinary、class initializer、struct value constructor、accessor、dispatch adjust、boxing adjust、object ensure/value 等已有 typed role，不根据 name 推断。

新 binding product 按上述顺序编码 field1～5。两份 signature 都使用新 constituent `{ exact, gc_effect }`（field1、2）：exact 原样复用既有 `ExactCallableSignature`，gc effect 是独立无 payload sum，tag1 为 `Managed`，tag2 为 `NoGc`。旧 exact-signature wire 不变。普通 callable、accessor 与 pure-virtual trap 的两份 signature 相等；class initializer 仅按本节规则转换 receiver/result；adjust 保留目标语义 signature 与 slot 调用 signature，并逐项检查参数、result、suspend 及 GC effect 的合法关系。foundation 的 callable signature 锁定 implementation 的 lowered exact signature；GC effect 另与 HIR 合同及实际 MIR function metadata join，不能从同形 LLVM 函数类型补出。

origin 的 tag1～4 是 `Function/Constructor/Accessor/Generated`：前三者的 field1 为相应 typed id，Generated 的 field1、2 为 generated callable id 与完整既有 canonical key。lowering role 的 tag1～10 是 `Ordinary/ClassInitializer/ValueConstructor/Accessor/DispatchAdjust/BoxingAdjust/ObjectEnsure/ObjectInitializer/PureVirtualTrap/DerivedEquality`，tag11 为 `PrimaryValueConstructor`，field1 同样为 owner exact。ValueConstructor 用于 struct 次构造，PrimaryValueConstructor 明确表示只组装已求值字段的主构造 leaf。Ordinary/Accessor 无 payload；ClassInitializer/ValueConstructor/DerivedEquality 的 field1 为 owner exact；两个 Adjust 的 field1 为所调用的 Strong target；ObjectEnsure/ObjectInitializer 的 field1 为 object/companion initialization unit；PureVirtualTrap 的 field1 为原 dispatch slot。constructor owner必须逐项等于源码 owner chain 的最内层 nominal；trap implementation可以是该slot的原 declaration owner，或保留同一slot的abstract class override；后者的源码声明必须属于实际receiver class，该class须沿已导出的直接基类链严格继承root receiver，双方参数/result/execution签名保持一致，实际abstract选择仍由同次HIR/MIR生产核对。object value读取仍用5.4的已授权storage plan，不新造一个返回object的initializer身份。

源码 NoGc signature 的receiver、参数及结果必须全部GC-free。两个Adjust允许GC-free value目标保持NoGc，而接收interface/class ref的wrapper为Managed；Managed目标不能适配为NoGc调用。PrimaryValueConstructor 的源码合同固定Managed，lowered固定NoGc，两份exact signature相同且无receiver、返回owner，参数必须逐项等于struct完整声明字段的exact类型；这一无安全点内部leaf可以组装含引用的值，不授予源码NoGC调用资格。其余角色的lowered NoGc signature同样要求GC-free；除adjust与上述主构造角色外不改变GC effect。constructor/accessor/object initializer/ensure/derived equality均为ordinary同步角色；object ensure/initializer保持Managed。callable canonical表按Strong implementation的既有kind/id顺序保存，reader拒绝重复或乱序；两份签名与role均不可缺失。该constituent核对canonical identity、signature和type facts；slot选择、真实body和GC effect的完整authority仍须经HIR/MIR生产join取得。

source signature 与 lowered signature不能合成一个字段：class constructor 的源码结果为 class value，MIR initializer 取得同一个 initializing receiver并返回 Unit；enum variant construction 可以只有 representation operation而没有独立 machine body，使用专用 construction plan，不虚构 callable definition。

class construction 固定为 exact allocation一次、同一 receiver direct调用 initializer；base/`this` delegation 不分配、不改 header，最派生 TD 从 allocation 起保持。abstract class可有供 derived调用的 initializer，但不能构造 allocation target。跨 Cone构造保留 base-before-derived、共同初始化一次、异常不发布结果及每次 call 后 receiver relocation。

普通 member、extension、value constructor和adjust thunk都执行按值 receiver/参数语义。`@InteriorMutable` 或 `addressOf(this)`可观察时必须有方法局部 copy；即使 physical ABI使用 pointer，也不能把 caller/box内存变成方法的可修改 `this`。

MIR source constructor 的必需集合直接从共有 nominal/callable 声明的参数自由物化闭包查询，限于当前 provider 的公开或 protected 构造器；完整声明中的 private/internal、generic/source-only owner 与 object 隐式初始化入口不因存在于源码表而成为构造器导出根。查询仅返回借用索引，producer 与 reader 共用，不新增 wire 来源清单。reader 从同一共有声明逐项核对完整语义签名、GC effect 与实际 owner：class initializer 把源码返回值转换为同一 owner 的 initializing receiver，并返回 canonical Unit；struct 唯一 primary 的参数类型序列与完整声明字段序列相同，既有 constructor duplicate-signature identity 保证同一 owner 下该签名唯一，其 lowered 角色为 PrimaryValueConstructor、GC effect 为 NoGc；其余 struct constructor 保持 ValueConstructor 与源码 GC effect。缺失、额外、错 owner、错参数、错 receiver/result、错误主次角色或 GC 漂移均拒绝。枚举变体仍使用 representation construction plan，generated callable、dispatch、初始化、selected 和完整 LIR/artifact 闭包继续按各自合同核验。

派生 equality 的共有 MIR reader 从同一 HIR foundation 的原有 DerivedEquality application key 查询源码身份，并与已核验的 Strong callable/signature 表关联实际函数投影；后者只确定哪项源码 application 已有机器定义，不能反推源码资格。仅当前 provider 的已闭合 struct、enum 及 canonical Unit 进入类型桥接集合，私有未导出 owner、无关声明与未物化的默认正文 application 不增加导出根。每个实际导出的 application 必须保留原 generated identity 与 exact owner，semantic/lowered 签名均为 ordinary Managed、receiver 与唯一参数同为 owner、返回从共有声明及真实可达依赖按 Boolean intrinsic 角色查询的唯一 typed exact，lowering role 必须为 DerivedEquality。缺失源码 application、缺失或额外 binding、owner、签名或 GC 漂移均拒绝；Strong 定义集合继续与同一 MIR foundation、LIR 和对象定义关联，源码 application key 本身不代表 machine body。该查询复用已有 identity 记录和临时索引，不新增来源表、caller factory 或 wire 字段。派生 equality 正文的 receiver 与唯一参数使用原 This、Parameter(0) local selector，但其定义站点是编译器合成；reader 仅在 local owner 的原 generated key 确为 DerivedEquality 时不要求这两个值的 definition origin，并拒绝为它们附加伪造的源码记录。其他源码局部值继续核对真实 owner 和来源锚点，不能按 generated owner 一概豁免。codegen 对 tagged enum 字段重放存储时，零大小字段必须保持 canonical offset 0，不加 variant payload 起点；非零字段继续按 slot 与相对偏移校验，全部字段的类型、大小、对齐、GC 属性和 scan 仍逐项检查。实际 initialization-use、selected-use 与最终 artifact 闭包仍须独立核验。

### 5.3 dispatch schema 与 table 构造

实际源码消费使用共有外来 interface/class 声明，HIR 保留完整父接口、成员签名与 provider 的 dispatch 顺序。选中的调用带有直接、虚调用或接口调用方式及表内槽位置；MIR/LIR external callee 使用该方式和实际 canonical ABI，不再限制为 Direct。接口查找使用真实接口 TD，class 虚调用保留 base prefix 的槽位置；外来实现继续由 provider 定义；MIR 接口槽直接保存完整参数、结果与 GC 签名，删除只为签名创建的不可达本地函数。真实跨 Cone 调用、覆写和接口默认方法的链接运行必须覆盖这些路径，手工 metadata 验证不替代功能验收。

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

MIR dispatch 的共有 reader 在完整 HIR 继承、槽 schema 与源码选择重放后，独立取得当前 provider 的必需 owner 集合、原始槽顺序和每个实际目标；object/backing 共享同一源码选择，内建 Any 仅按其 canonical typed identity 保留空 class vtable。槽调用签名从共有 HIR 根声明投影，interface 表只替换 receiver；抽象 class 入口沿真实 base 链选择最近的同槽抽象声明，interface obligation 使用该槽的原声明，不能从候选 trap target 反推。引用 receiver 采用 canonical Identity/ReferenceDispatch；值类型 interface 表机械选择既有 BoxingAdjust identity，并核对其实际源码 target、语义签名与 Managed wrapper 签名。缺失或额外 owner、table、slot、适配入口，顺序、位置、目标种类、receiver、effect、GC 或 generated key 漂移均拒绝。共有 MIR transport 在 dependency-first callable 解析时使用同一依赖 type/callable/schema 索引验证 dispatch 一次，后续直接借用 canonical 结果；不新增来源副本、caller factory 或 wire 字段。有限 BoxedValue 的 MIR base-and-interfaces 复用同一次源码类型导出中 payload 的直接接口集合，不能把机器 class 的传递接口闭包写入该字段；实际 descriptor/itable 仍沿 payload schema 保留完整闭包。有限 helper producer 必须取得 sealed source shape root 对应的源码类型记录，缺失或 origin 不符立即拒绝。derived equality、实际 initialization-use、selected、LIR 和双 view artifact 的剩余义务仍分别完成。

### 5.4 object value 与初始化

`ParamFreeMirObjectValueV1` 保存 object-value identity、backing exact class、provider-owned unit、ensure callable 和 published-root 读取关系。consumer 按普通语义先 ensure，再读取提供方的根；对象、初始化状态、失败根和 GC 登记均由提供方持有。类型、成员、转导出与默认参数使用原 typed 声明引用；top-level/delegated property 继续经实际 accessor，字段访问仍服从语言可见性。

object product 的 fields 1～5 依次为 `PersistentObjectValueId`、backing exact、初始化 unit、Strong ensure callable 和读取计划。读取计划沿用 tag 1 `PublishedSingletonRoot`，field 1 保存 source object exact；表按 value id 排序且唯一。MIR 用完整 object 记录生成普通外来 ensure 调用与 typed 全局读取，LIR 通过同一 nominal 的 `StaticStorageKey::singleton_published_root` 查询实际依赖存储定义、符号和布局。外来静态存储不进入当前 Cone 的定义、初始化或 GC 登记表。reader 在输入边界核对 value、source/backing 类型、provider、unit、ensure 签名和根的关系，后续复用完整结果；不增加来源授权、凭证或返回 object 的第二种初始化入口。object product 的现有 wire tags 和 runtime C ABI 保持不变。 默认模板中的本地与外来 object value 使用同一引用表保留真实 value ID 和定义位置；对象类型与值共享可见域，默认值及继承默认值的域检查复用已有类型引用，不再缓存或重查第二份对象可见域。

`cross-cone-interface/28` 的共有 Expression type site 新增角色 tag 7 `SingletonValue`；既有角色 1～6 不变，旧 `/27` 产物重建。记录原 object exact 与真实求值位置，关联同一 object 声明的 value ID。仅实际读取产生 HIR SingletonValue selected use 和 MIR Object 依赖；若其 root 是当前已物化 Initializer，则把实际 provider object 记录中的 unit 保存为 ObjectValue 初始化边。consumer 不从名字、相同布局或候选 selected 表补身份；后续使用已经完整解析的记录。 object/companion 的属性初始化和 init 块位于其实际主构造正文时，按该构造器的 typed owner 关联同一 Object/Companion unit；这仍是单例声明自身的直接初始化，不展开普通构造器或外来函数的调用图。

Strong MIR sealer必须逐项将每个local initialization unit的ensure/initializer函数与该unit的既有 `GeneratedCallableKey::Initialization` 两个role join，要求两者均为真实emitted Strong body、普通Managed、无receiver/参数并返回同一Unit exact。HIR-owned source callable materialization和MIR-generated relation按其真实来源读取，不要求HIR生成的initializer另出现在MIR-generated delta。sealed materialization plan保存这条完整typed关联，不能只保存unit arena id后让LIR猜函数。该局部函数证明不代替foreign unit的同一terminal provider、真实descriptor/cell和definition closure；后者仍由完整section及11.2闭合。

本阶段关闭 artifact中的所有 typed edge与registration关系，不执行全图 eager startup。M23-8会在登记全部image之后按这些既有unit关系启动，不回到名称解析补依赖。

属性初始化用途从实际已物化的 accessor 调用重放。共享 HIR 的原调用位置定位当前 generated Initializer root，提供方的原 property 声明与 source initialization-unit key 决定访问是否带直接 ensure；没有运行期 unit 的 image property、computed property、const 不虚构依赖。公开 managed 顶层属性的默认 getter/setter 必须生成实际 Body，并以既有 typed accessor identity 进入普通 callable 导出；静态映像后备也遵循此规则，但不为它生成运行期 unit 或 ensure。非公开直接存储与 const 保持原形式，setter 仍使用其自身的访问合同。独立 lambda/匿名函数的词法身份从真实父 accessor/initializer 的签名取得 binder 上下文，不能查询子闭包的 source signature 或用空 binder 兜底。普通 callable 的内部 ensure、独立 lambda/匿名函数正文与未展开 default 不加入调用者的初始化图；已展开 default 中直接执行的属性访问使用调用者的实际 root。默认模板中的 accessor 引用直接保留所属 property、accessor 的 typed ID 和定义作用域，按共有声明目录解析；定义时实际发生的名称查找路径可以保留，不要求消费方重新取得路径，也不另建 direct callable binding 表。实际调用及初始化边来自展开后的 typed 正文。生产端保留所选 accessor 的同一 typed unit 关系，再从 sealed HIR 正文逐次收集并规范去重；共有 Compile/Link reader 独立从已有 call-site、property 与 foundation 数据重算，与 MIR initialization-use 全集精确比较。缺失、额外、错 provider、错 accessor/unit、把普通函数或未物化 unit 冒充初始化 root 均拒绝，不能由候选 MIR 用途或 selected 表补根。读取只借用同一依赖闭包的原 metadata，不新增 wire、来源授权表或 foreign body 分析。ObjectValue 与显式 InitializationSupport 继续要求各自实际 typed 使用，不能因存在合法 canonical key 被接受。LIR 的完整 external initialization registration 边必须与已验证 MIR 用途按（local unit、provider、dependency unit）去重后的集合精确相等；getter/setter 等不同 cause 可合为同一条边，登记边本身的重复、缺失、额外或 provider 漂移均拒绝。再验证完整 unit definition、descriptor 和物理导入。依赖的初始化 startup gateway 保留其独立 CallableBodyKeyKind 与 unit registration 关系；目录核对同一 eager unit 的完整 gateway body/entry，不能将其冒充普通 Strong callable 或因 provider 含有 gateway 而拒绝整个依赖。ODR callable 仍由本阶段 gate 拒绝。

当前initializer中的显式external ensure通过`SelectedExternalInitializationUseV1 { local_unit, provider, dependency_unit, cause }`记录；cause是`ObjectValue(object_value_id) | PropertyAccessor(accessor_id) | InitializationSupport(unit_id)`，必须由已提交的typed ensure语义产生。LIR selected set保留同一edge，strong-production/12按3.2验证foreign unit，不要求它出现在本地unit arena。这里只记录现有语义的真实ensure dependency，不读取foreign body做跨Cone调用图推断；普通external callable内部自己的ensure继续由provider负责。image/runtime使用canonical unit id解析这些edge，相关真实descriptor/cell relocation按11.2验证。

initialization-use product的field1～4按上述顺序保存；cause是tag1/2/3、field1分别为object-value/accessor/unit id的closed sum。canonical表按local-unit、provider、dependency-unit、cause的typed key顺序保存，拒绝重复。local-unit必须属于当前consumer，provider必须是dependency-unit的真实定义Cone且不是consumer。ObjectValue cause必须对应同一Object/Companion unit；PropertyAccessor cause必须对应同一top-level/extension property unit，或同一object/companion owner的成员property；InitializationSupport必须等于该dependency-unit。param-free表拒绝generic delegated application unit。该constituent验证identity及关系，完整section仍必须把每条use与已提交的typed ensure语义/真实MIR调用逐项join，不能由canonical key匹配推断foreign body依赖。

MIR object value 与两个初始化 callable 的 reader 校验直接读取同一 artifact 的共有 HIR nominal 声明、已重放的表示和原有 initialization-unit foundation keys。参数自由 object/companion 的必需集合来自这些源码数据；每个 owner 恰有一个对应 Object/Companion unit，普通属性初始化与 generic delegated unit 不参与该对象索引。object value 的 typed id、source exact、backing exact、实际 provider、published-root read plan 与 ensure target 必须逐项对应；initializer 和 ensure 的 canonical generated key、lowering role、ordinary/Managed、无 receiver/参数及 canonical Unit 返回值同时核验。缺项、额外对象或初始化入口、相同 owner 的冲突 unit 与任何关系漂移均拒绝，source-only/generic 或无关 private 源码对象不因其 foundation key 存在就成为机器根。对象 transport 在 callable 解析后通过本地 canonical callable 表验证一次；后续完整 section 直接使用该 owned canonical 结果，仍必须完成其它 generated callable、dispatch、实际 initialization-use、selected、LIR 与双 view artifact 闭包。本步骤不新增 wire 字段、来源副本或 caller factory。

共有 MIR reader 从同一 HIR foundation 的原 initialization-unit keys 关联完整 Strong callable/signature 表中的实际初始化定义，成对重放 Initializer 与 Ensure 的原 generated identity、实际 provider、ordinary 无 receiver/参数及 canonical Unit 返回值。只有存在实际 Strong 定义的参数自由 unit 进入已验证 unit 集合；源码 key、默认正文或未物化 object 本身不能制造机器根。任何已发布初始化 role 都必须具有原 source unit 和对应的另一个 role，两个签名分别同 MIR foundation 精确匹配；缺失 source/role、错误 provider、generic delegated application、签名或 callable role 漂移均拒绝。读取结果使用共有 MirTypeBridgeInitializationUnitV1，直接保存两个 typed callable 和 Managed 签名；不保存额外证明状态。实际 body、GC、LIR unit registration 与对象定义在各自消费边界关联核验，后续直接使用这些完整结果。共有 Compile 闭包持有完整已重放 unit 集合，不新增 wire 字段、来源表或 caller factory。unit 的 source owner 只表示身份归属，不自动产生 MIR type/layout 导出边；unit 合同的类型依赖来自其完整调用签名。实际 ObjectValue 使用仍独立闭合 source/backing 类型、ensure 与 unit，私有对象的本地 descriptor、初始化 body 和 storage 仍由完整 Strong/LIR 产物验证，不能为了验证本地 unit 扩大类型导出根。实际 external initialization-use 与 selected 仍须从已提交的 typed 使用重放，不能从本地 unit 列表推断 foreign body 依赖。

## 6. LIR：完整 layout 与 scan

### 6.1 section 与依赖查询

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

`selected`是field1/2分别保存`semantic_uses`与`physical_imports`的closed product；`LayoutAbiDependencyV1`的field1/2分别保存provider与target，target按上列tag编码且payload在field1。semantic表按`(provider, target canonical bytes)`严格递增。每项provider必须不是consumer，并精确命中显式dependency closure中唯一terminal provider的对应五张表；内存中的 selected entry 保留实际 provider 的完整五表导出记录引用和本次选择索引。依赖查询直接接收共有的完整导出表；同次编译的 producer 与已读取产物使用同一数据入口，不要求 reader 重造 producer section 或来源工厂。

producer 根据已提交 MIR→LIR typed selection 构造 semantic 闭包；reader 验证外部产物的相同关系。两者查询同一类实际导出记录：layout 内嵌的 base/field/variant/array 存储数据不追加符号依赖，其完整性已在布局读取边界检查；descriptor 跟随value/instance layout、parent/interface TD及vtable/itable；dispatch跟随interface/owner type与每个target callable ABI；callable 的完整签名不另外引入 layout 物理依赖；shape-support跟随八个role对应的layout/descriptor/helper记录。metadata-only读取保留在`semantic_uses`即可，不因此产生relocation。每个跨provider递归edge都进入真实terminal provider，依赖section中的转发selected关系不能代替terminal记录；环、同target多provider、当前Cone回指、缺失/额外关系及旧core/M23-5分区冒充新selection均拒绝。

两条路径在计算闭包前都完成共有跨阶段校验：生产侧将本地五张 export 表与同一次实际 MIR→LIR 输出关联，reader 将其与同 artifact 的完整 MIR type/callable/dispatch metadata、Strong production、layout/ABI 和实际 object use 关联。physical_imports 必须精确对应机器使用及必要 object/init support；五类一般 import contract 重新绑定到依赖闭包中的同一 terminal section 记录。单表构造成功或 import 已解析不能代替完整覆盖与相邻阶段一致性检查；reader 不要求另外提供编译期 source authority 或平行授权 transcript。

LIR section 不再保存一份 producer section 组成的递归依赖图。driver 从已解析构建图提供真实可达 provider 的完整导出表集合；选择入口检查 provider 唯一性、target 和所需 typed 目标，布局关系闭包只遍历实际引用。manifest 的依赖环检测仍由共有图边界负责，不在每次选择中重走相同的 section 图或累计遍历深度。Strong V2 的预先选择与最终 section 使用相同目录，读取结果可直接参与 downstream lowering、registration 和发布。

`physical_imports` 是 11.2 定义的 canonical semantic-import projection，按 `(provider, subject canonical bytes)` 严格递增且唯一。它收集实际 machine use、strong-production/12 引用以及对象和初始化支持，再关联对应 provider 的完整语义记录、Strong definition 与既有分区。语义使用可以没有物理 import；物理 import 必须具有完整的实际 typed target、表示及定义信息，不能只凭 symbol 或布局相同建立关系。该数组与 Link section field 1 及 Code contribution 使用同一规范编码，不增设 support authority 或来源凭证。

layout 表以 `PersistentLayoutId` 为主键，同一 exact 可以有不同 representation role 的多项。`TargetProfileWireId`、`RepresentationRole` 原样复用 foundation 的 `LayoutKey`：ManagedValue/CValue/NativeFunctionPointer 必须匹配 Value body，ManagedObject 必须匹配 Instance body；scan key 的 layout/role 同样重放。target 必须等于同 artifact 已验证 LIR projection，不能仅比较可读名称。`StrongShapeDefinitionV1` 复用 M23-3 的 semantic-id/definition-plan/symbol product。每项 layout/scan/descriptor引用 foundation中的既有 key；definition必须在 provider strong production中有唯一primary atom。

普通引用值的 Value body 是 managed pointer大小/对齐和单个 managed leaf；它指向的对象 field layout属于独立 ManagedObject layout 的 Instance body及object scan。`InstanceRepresentationV1`的tag1～5依次为`ClassObject { base_prefix, declared_fields, complete_fields }`、`BoxedPayload { payload_exact, value_layout }`、`InlineBytes`、`InlineArray { element_exact, element_storage }`、`AbstractReference`。class base prefix与新增字段只在ClassObject中拥有authority；complete_fields是从base prefix和declared_fields机械拼出的全序投影，reader逐项核对，不能自由提供一份不同的flattened list。不能把空 class误判为 size 0引用值。

scan identity的role矩阵保持既有规则：ManagedValue、CValue、NativeFunctionPointer均配InlineValue；普通ManagedObject配ManagedObject；intrinsic array的ManagedObject layout配ArrayElement，其canonical scan描述单element。array descriptor的object scan另从该element scan与length/data offset/stride派生，不能把element scan当作object-relative scan，也不能为方便改写已冻结ScanKey。BoxedPayload的inline scan来自payload value layout，object scan来自checked平移。

consumer只能通过 `ExternalExactLayoutRef`取得完整事实，并在本地 aggregate中重放外层布局。外部 layout/scan constant、TD和table全部保持 external definition；本地内联 field offsets不构成复制外部addressable constant的许可。

LIR selected 闭包使用完整 MIR 引用及 manifest 中实际可达的依赖。reader 先检查布局与普通 callable ABI，再检查其余 callable、dispatch、descriptor 和 shape-support 记录；dispatch 同时查询两类已有 callable 记录，按实际 typed target 取得 ABI 与 body symbol，不在每个 slot 内另存一份 callable ABI。普通 callable section 在原有字段编码一次，布局导出上下文仅共享已经读取的记录。

类型用途通过 actual exact type 关联所属 provider 与当前 target 的 ManagedValue layout。selected 闭包沿现有 layout、descriptor、dispatch、callable 和 shape-support 引用检查缺失目标、重复目标、不可达 provider、错误 target profile 和 ABI 不一致。普通 callable 与其余 callable 共用引用规则，不能按旧表分区拒绝正常 dispatch 或 selected 用途。完整结果复用于后续物理引用与 Strong object 检查；发布时不重新解码未变化的记录或再建立操作资格。Compile 与 Link 仍检查各自消费的格式、符号、定义及 relocation 合同。

共有 Compile reader 在 LIR 语义依赖图之后，将 Strong V2 与同一 artifact 的五张导出表逐项关联，再从原物理 transport 的 provider 与 typed subject 查询 manifest 可达依赖的实际 definition、symbol 和七类完整合同。候选只指定待核对的合同，不产生源使用根；原数组必须严格 canonical，重复、本地冒领、不可达 provider、旧分区重叠、合同漂移和缺失 semantic selection 均拒绝。Strong 中所有实际 ancestry、itable、dispatch callable 与外来初始化依赖必须命中相应 semantic/physical 记录。初始化和 storage 查询由 reader 私有实现直接借用同一 MIR 导出、已重放 selected 关系及 Strong unit/storage；普通 accessor 不开放 backing storage，singleton 只关联其 published root，初始化支持不得跨到其他 unit 的 storage/failure root。查询接口只提供候选关系，不能作为来源资格；合同继续与实际 provider 计划逐字段核对。结果保留实际完整 production、导出记录和 typed 依赖，后续消费直接使用这些数据；实际 object 使用覆盖与 ABI 一致性仍由相应边界检查，不另外证明 HIR 来源资格。本步骤复用现有合同与 Strong join 算法，不新增 wire、来源 factory 或 CORE 例外。

Strong V2 的类型、布局、ABI、dispatch 与 registration 在实际组合边界核对一次，完成后直接保留普通完整 production section。后续 codegen、对象处理和发布使用该数据，不再通过 Pending/Replayed/Validated 凭证包装限制编码资格，也不复制五张导出表仅用于防止后续替换。provider 查询从实际 production 和导出记录取得 typed target、definition、symbol 与合同；已检查且未变化的依赖不反复重做整表关联。

### 6.2 storage 与 field layout

M23-6 的共有 Compile reader 直接从已核验的 MIR 类型组成表重算 LIR layout 导出，不接受调用方提供另一份预期表示或来源 factory。每个实际导出的参数自由 nominal 及有限 helper 必需 ManagedValue 与 ManagedObject 两种角色，CLayout struct 另需 CValue；已核验的 ObjectBacking 仅为对应 object 提供完整字段和继承形状，不独立产生布局角色。集合从 MIR 表示决定，不从候选 layout 或 foundation layout 清单反推。无关私有本地定义继续属于完整 Strong production，但不加入导出根。重放使用同一 artifact 的 typed identity、LIR foundation、实际 target 和可达依赖的已检查 layout，按声明序计算 field、variant、base prefix、ZST、GC scan 与既有 CLayout 合同；引用环经 managed value 表示终止，按值或基类循环拒绝。Unit/Any 的固定语言角色保持，其他 intrinsic 均来自已检查的实际 MIR 表示，不按 CORE 来源放行。每条 wire layout 的完整 identity、role、representation、storage/shape、scan 和 definition 与重算结果逐项一致，缺失、多余、乱序及错 provider/target 均失败。该状态只证明布局组成表，后续 callable ABI、TD/dispatch、实际 selected-use、Strong V2 和 Compile/Link 双视图仍须全部关联后才能发布或消费；原 wire、版本及 runtime ABI 不变。

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

单record的checked replay证明canonical identity、representation几何及所有scan范围，不替代完整section的HIR/MIR source join、export surface或selected dependency closure。Unit必须等于compiler固定的`CoreBuiltinNominal::Unit` exact，且该exact的Value body不能选择其他representation；Integer/Boolean/String/Array/MutableArray没有compiler固定的nominal identity，其 kind/family 必须由完整 section 与前端已解析的共有 intrinsic 声明/typed representation 逐项 join，不依赖 core receipt 或来源资格；不能按名称/FQN 重建 identity，也不能仅凭任意 nominal 加 family 输入跳过声明与表示一致性检查。按by-value依赖与base active path拒绝cycle。完整record只持有closed storage/shape及已核验依赖，不通过裸size、默认offset、missing-key fallback或native-boundary witness补齐authority。

### 6.3 scan normal form 与检查边界

scan 复用 runtime spec 2.2，普通 node 的 offset 相对明确的 base：

- References 严格递增且无重复；Sequence flatten、删除 None、合并同层 References，并按 `(child fingerprint, canonical bytes)` 排序去重。
- Array 是 `{ length_offset, first_element_offset, nonzero_stride, nonempty_element }`；GC-free/ZST element 为 None。
- tagged enum 扫描所有独占 ref-bearing slot，不读取 tag；pure-value 共享区不进入 scan。
- box 的 inline scan 相对 payload，object scan 通过 checked offset 平移；Array 平移 length/first offset，不平移 element child。

producer 规范化后直接供后续 stage 使用。reader 在外部产物进入时检查 canonical 格式、引用环、offset/alignment 与真实布局范围；runtime 在元数据登记时检查并复用结果。删除 depth/node/word/expanded-node/canonical-byte 通用配额及 ABI 成本常量，避免共享 DAG 重复工作的办法是节点/节点对记忆化，不是拒绝成本超额的合法元数据。

`ScanFingerprint` 继续使用 `scoop-scan-v1` 和 runtime canonical typed bytes；表示不因删除预算而改变。

## 7. TypeDescriptor 与有限 shape-support

### 7.1 descriptor record

M23-6 的共有 Compile reader 从已核验 MIR 类型角色及继承关系、同一闭包已重放的 layout/dispatch 重建完整 TD 导出。ObjectBacking 仅供形状，其余实际类型各有一项 descriptor；class/object/String 使用实际基类，原始值类型、interface 与有限 coroutine helper 无 parent，box helper 按自身已验证基类关系处理。interface directory 精确来自已重放的 dispatch table 集合，按 interface exact id 严格递增；producer 在 LIR 封存前采用相同顺序，reader 不替候选数据排序或修补，表内 slot 顺序保持原 schema。parent/interface 引用从实际本地类型或可达依赖 descriptor 确定 Local/DependencyExternal 及 provider，不按 CORE、名称或同布局推断。value/instance layout、shape、object/inline scan、diagnostic name、definition 与 registration/fingerprint 均从共有 checked constituents、identity graph 和 foundation 重算；Strong semantic plan 必须使用真实完整的已重放 dispatch entries，不能构造虚假空表。原 producer 继续验证实际 registration plan，reader 的组成状态保留其余原始 wire，并在最终验证拒绝替换任何已检查表。该步骤不授予 selected-use、machine body 或对象资格；完整有序 TD wire与后续 Strong/object/双 view 关联仍须全部通过。不改变 runtime C ABI、persistent identity、wire 字段或 capability kind；接口目录顺序通过既有 fingerprint 字段影响产物，并要求不满足 canonical 顺序的旧候选重建。

M23-6 的共有 Compile reader 从已核验的 MIR 类型角色与 dispatch schema、同一闭包已重放的 callable ABI/layout 重建 LIR dispatch 导出。除 ObjectBacking 仅供源码形状外，每个实际类型必须具有自身 vtable；普通值类型、interface 和 CoroutineStep/CoroutineSlot 的物理 vtable 按既有表示规则为空，class、object 与 String 使用原源码 class schema，BoxedValue 沿实际 payload 使用其完整 schema 与 boxing adjustment。只有具有实例 dispatch 的 class/object/String/BoxedValue 导出 schema 中的完整 interface table 集合；值类型的接口实现由对应 box helper 承载，不能从候选 TD/table 的存在与否反推集合。每条输入保留原 position、typed slot、完整调用签名、implementation kind 与 receiver adjustment，从唯一实际 provider 的 callable ABI 取得 body 引用；本地与外来 target 使用相同规则，不按 CORE 来源分支。共有 canonical 重放核对 target、签名、GC effect、receiver layout、foundation key 和 Strong definition；原 producer 在此结果上继续逐项核对实际 LIR slots 和物理 callable 引用。reader 按所有权保留其余 wire，完整有序 dispatch 表与重放结果必须一致，漏表、多表、重复、错序、目标或签名漂移均拒绝；后续验证也不能替换已检查的表。该组成表状态不代替 descriptor、selected-use、Strong registration 或机器对象关联，不修改 wire、capability 版本或 runtime ABI。

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

`CanonicalExactTypeDiagnosticName`由已验证exact key重算，遵守总设计3.1的grammar、generated role和checked 长度计算。import/re-export/typealias拼写不参与。name bytes按值进入descriptor definition，关联只读atom由同一definition plan覆盖；不能让consumer替外部TD提供本地display string。

### 7.2 普通 Cone shape-support

M23-6 的共有 Compile reader 继续从已通过 HIR 声明关联的 MIR 有限 shape-support 源码根重建 LIR 八角色表。每个 source 的 canonical declaration key 从同一已验证 identity graph 按 typed id 查询，保留实际 provider；不扫描候选 LIR 表、foundation 清单或无关类型 arena 来补根。共有重放从已检查的 layout、TD 与 definition 重算 SourceNominal、ValueLayout、RefScan、TypeDescriptor、TypeRegistration、BoxedValue、CoroutineStep、CoroutineSlot，逐项验证 helper 的 generated nominal/variant/field identity、payload layout、ZST、GC scan 与 registration；helper 不再成为新的 source root。reference nominal 仅允许既有的无需装箱角色，value nominal 必须具有完整 box。完整有序 wire、provider、target 与八个角色必须精确一致，漏项、多项、重复、错序或换用其他类型的角色均拒绝。此步骤按所有权汇合五张已检查 LIR 导出表，保留原始 selected/physical transport；后续最终验证不得替换任何已检查表，Strong V2、实际使用、对象与双 view 关联仍须继续完成。不增加授权表、调用方 factory、wire 字段或 runtime ABI。

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

共有 Compile reader 在布局验证后先读取普通 callable ABI，使后续 dispatch 可以按 typed target 查询实际记录。它使用已确认的 MIR exact signature、GC effect、实际可达依赖及共有 nominal 表示查询，按 canonical Scoop ABI 规则检查 receiver、声明序参数、结果、Cdecl 和 caller root plan。每个 artifact 的表示索引仅建立一次；已有 ManagedValue layout 的签名位置按该 target 核对，已合法的 source-only nominal 签名仍按真实声明表示处理。

实际 Strong body、symbol、definition 与 primary atom 按 provider 和 typed target 关联；外部选择匹配 MIR 的声明、implementation、签名及 GC effect，再复用 provider 的完整 ABI。缺失、重复、乱序、不可达 provider、表示冲突和 ABI 不一致均拒绝。检查后的普通记录与其余 callable 共用查询，供 dispatch、selected 闭包、初始化及 Strong object 关联直接使用。普通 bridge 仍在原有两字段 section 中保存，六字段 callable ABI 不按 slot 或消费用途复制。

共有 Compile reader 通过普通 MIR/LIR callable 关联检查初始化函数的实际声明、body、完整 canonical ABI、symbol 与 definition，复用同一 callable 导出结果。初始化语义的签名和 effect 由前端协议解析及 MIR 角色关联负责；LIR 不再从协议重投影初始化 ABI，不单独读取、重放或保存第二份服务 ABI，也不按角色拆分外部定义目录。

Strong V2 reader 的 digest 图由同一 artifact 的实际 LIR foundation、已校验的八字段 runtime registration 语义和源码 entry 角色重建，不接收调用方另给的预期 digest 图。待验节点先只向该 foundation 的已有 typed owner 解析，验证 identity、顺序、边、patch 与无环关系；该临时结果用于 registration 的相互引用校验，不成为最终图的依据。随后 producer 与 reader 共用相同的按角色投影算法，从 callable、safepoint、type descriptor/layout、immortal、static storage/scan、初始化 schedule、entry 和 image 逐项生成完整节点、直接输入和 patch 集合，并精确比较全图及十字段 Strong section。缺失、额外或替换的合法节点/边/patch 不能因仍然无环而被接受；external 初始化依赖不新增本地 digest 输入。类型与 callable 的源码/selected join 及实际 object 校验继续由完整闭包执行。不增加 wire、来源证明或独立预期表。

共有 Compile reader 在布局和普通调用表验证后，按同一 manifest 的依赖优先顺序读取完整 Strong V2 section。entry 取已关联的 MIR 输出协议，有限 shape-support 根取共有 HIR 公共与必要支持声明；依赖 descriptor/callable 的物理定义和初始化单元从实际可达 provider 的同一 foundation 与 registration 查询。初始化本地定义由本产物的 registration identity、foundation 和 digest 图解析，外部目录只携带其他 provider 的定义。reader 完成 registration、digest DAG 和八字段 section 的必要一致性检查，保留已读完整数据；不从物理目录反推源码资格，不重新解码和证明同一结果。Compile/Link 复用这份语义结果，Link 追加实际对象和 relocation 检查；同次编译的完整输出直接发布，不以再次完整读取产物及全部依赖作为发布门禁。

Strong V2 的引用按实际表关系检查：digest owner 只查询其对应的 foundation 表，patch 只查询 definition/atom，输入边使用节点索引。类型及初始化检查覆盖必需的 typed 引用、物理字段与计划关系；不记录或估算遍历、索引、分配的逻辑成本，也不设置资源配额。

codegen 的 Scoop ABI 防御校验按共有存储规则重放普通 struct/tuple：零尺寸字段保留逻辑字段身份与对齐，canonical 字段偏移固定为 0，不对当前非零存储 cursor 插入 padding，也不贡献 GC scan；非零字段仍按实际 cursor 对齐并逐项核对偏移、access alignment 与引用位置，aggregate 最终大小继续按最大字段对齐取整。CLayout 字段仍遵守自身合同的物理布局规则。不得将引用或非零字段后的 ZST 当作具有后继物理偏移的字段，也不得通过跳过整个 aggregate 的布局/scan 校验来接受该情况；错误的 ZST 偏移或对齐、非零字段偏移、总大小与 scan 均拒绝。此修正不改变既有布局、wire、身份或 runtime ABI，只使后端校验与已定义的 ElidedZst 规则一致。

M23-6 的共有 Compile reader 按实际 MIR callable binding 和已验证的本地／可达依赖布局检查完整 canonical ABI。nominal 值使用已有 ManagedValue 布局；tuple 递归组合各元素的存储与 scan，函数和指针使用目标 profile 中的实际表示，不要求独立 layout/TD definition。同一 exact 的查询结果在本次解析中复用，不重放已完成的 HIR/MIR 类型语义。完整 receiver、参数顺序、结果、GC effect 和 ZST/direct/indirect 传递必须一致，缺失声明、布局、错误 provider/target 或布局角色仍拒绝。body、definition 与 symbol 按实际 typed target 解析，reader 核对 callable field 1～4、6 及有序集合，保留已检查记录供后续消费；field 5 在 `cross-cone-layout-abi/3` 退役。runtime C ABI 和 String 表示不变。

layout profile 的 LIR 在封存前使用已验证 HIR/MIR identity graph 与 Cone coordinates，为全部实际 descriptor 生成 canonical diagnostic name；registration、导出表与 object bytes 消费同一实际名称。导出重放只能比较，不能在 producer 已封存后改名或接受 arena 显示名。关系或 coordinate 缺失即失败。

source exact 在 LocalConcrete → MIR 转置时保留实际 nominal provider，application 则保留匹配的 specialization record，结构类型使用独立归属分支。协议导入的 nominal provider 同样来自其已验证声明 key，不由协议发布方、consumer 或 CORE 常量推断。只有 provider 为当前 Cone 的 nominal 才进入本地 Strong shape 根；外部值可用于表示计算和签名，但只能引用定义方的 layout/descriptor。此关系与 exact key 一起完整校验，不以非泛型 nominal 默认本地所有。

LIR producer 从同一次 sealed MIR/LIR、完整 MIR export 组成表及实际 Strong V2 registration 组装五张 export 表。布局和 descriptor 覆盖 MIR source/support/helper 导出闭包中的实际物理定义；callable 直接保存同次完整 IR 已生成的 canonical ABI，保留完整 receiver、重复参数、结果和 GC effect。dispatch 将实际 LIR table 的物理 callable 与 MIR 的声明序 schema 逐项 join；BoxedValue 沿 typed payload 关系使用源码 value schema，CoroutineStep/CoroutineSlot 的无成员关系只允许实际空表，不从任意空候选表补默认实现。依赖只借用，五表使用同一 target、provider ；完整 section 的 source/selected-use 和最终产物验证继续执行，不以 export 组装代替发布闭包。

LIR section 直接接收同次 MIR→LIR 已生产的完整五张导出表和 typed 依赖使用。lir-lower 按实际 MIR 使用、LIR external arena 与 Strong V2 初始化记录计算语义根，并核对物理引用的 provider、subject、symbol 和 definition；不重做 MIR source/export 全量验证，也不重新生成五张预期表。IR section 只负责导出关系与依赖引用闭合，不接收来源工厂或资格回调。reader 逐项检查新读入的组成表后保留这些表，随后解析 selected 和物理引用；不能再次传入另一套预期表，重编码并比较先前已经完成的同一检查。

当前 provider 拥有的语言内建 Unit/Any 必须在 LocalConcrete HIR sealing 前显式保留，并在 HIR → MIR 转置时作为必需类型根沿共有 source-exact 路径处理；不能依赖额外源码引用才获得完整导出。该必需集合依据内建 typed declaration 的真实 origin 判定，外来 consumer 仍只为实际使用引入依赖引用。验收包括不添加类型使用的原始 core 源码基线，完整 MIR/LIR section、Strong V2 registration 和 LLVM 对象发射均须闭合。

M23-6 的 Cone image dependency 表使用当前 artifact 的完整直接依赖集合，包含显式与前端发现后纳入 manifest 的普通 provider，按 ConeIdentity 严格排序；生产入口必须显式传入该集合，不能从当前 Cone 是否为 CORE 推断空表或固定 core 单项。构造与读取拒绝 self-dependency、重复、缺失及多余依赖；Compile reader 从已验证 manifest 重建预期 image plan，Link reader 与生产 Code projection 逐项关联同一集合，再验证实际 object bytes、依赖 atom、RuntimeImage 与 Code fingerprint。仅因运行时尚未执行多 image startup，不能丢弃已经声明的依赖元数据。既有 image wire field、identity、runtime C ABI 与 fingerprint 编码不变，新的实际依赖通过已有字段进入 fingerprint；该修正不提前开放 M23-8 startup、M23-9 program-link 或后续 ODR 能力。

Link reader 复用同一不可变字节快照的共有 metadata 结果，核对 Link 特有物理导入与对象关系；完整检查顺序见 11.1。

layout profile 的对象组装直接消费同次 V2 LLVM 发射结果，必须在释放临时对象目录前取得完整成员 bytes；成员绑定、C bridge envelope、relocation、digest patch、stackmap 与六类 registration 沿现有共有校验执行，type 和 initialization 使用 V2 引用语义。完整 undefined-use 分区先闭合，再计算 callable/descriptor/registration、runtime image 和 Code fingerprint，最终由既有 layout writer 组装 archive。V2 production 的完整结果按所有权传递或共享借用；发布复用同次语义和对象检查结果，不对这些 bytes 及依赖分别完整重放 Compile/Link。外部新产物仍经共有 reader，写入回读核对实际 bytes 后原子发布。

无关私有类型的本地 TD/layout/dispatch/body 继续进入完整 Strong production、registration、object 和 fingerprint 验证，不能仅为导出五表而扩大 HIR source roots。私有类型被公开字段、基类、签名或有限 helper 的传递闭包引用时，仍须完整导出；边界由 typed 关系决定，不能按 visibility 删除必需记录。有限 shape-support 计划的源码根及其 helper 的 MIR type 与实际物理定义必须齐全，ObjectBacking 保持既有 shape-only 关系。dispatch constituent 逐项验证 foundation/definition，完整 section 要求导出 TD 所引用的 vtable/itable 与 dispatch export 精确覆盖；Strong V2 join 逐项验证 export 的实际 registration，允许本地生产清单包含导出闭包外的完整私有定义。

HIR 来源根发现必须遍历已要求 nominal 的全部真实存储字段，包括 struct 字段、enum payload、class 字段和 object backing 字段，保持字段类型的完整 nominal/application、tuple、function 与 pointer 组成关系。被这些字段引用的本地声明进入同一 source/support 闭包，外来声明继续由实际 provider 提供；字段可见性不影响表示依赖。既不扫描函数正文扩充根，也不展开无关私有 sibling。generic 声明及其字段引用只产生源码依赖，实际 generic/structural 物化继续受原 ODR gate 约束。

所有 producer 共用同次 MIR/LIR 的 canonical ABI 投影，包括当前 Cone 为 core 的情况。后续组装直接消费已有结果；新读入产物按实际签名、类型和 provider 检查 ABI，不固定使用 core foundation，也不因来源跳过检查。普通 callable 与 M23-6 lowered callable 使用相同的参数传递和外部调用校验。

`ExactCallableAbiExportV1` 保存实际 `target`、完整 `canonical_signature`、`calling_convention`、`call_protocol` 与 `definition`，沿用 field 1～4 和 field 6；原 field 5 的逐参数 `layout_dependencies` 退役，不再序列化或保留重复 layout 表。`target` 是既有 `StrongCallableDefinitionOwner`，`definition` 保留实际 provider、callable body 和物理符号的对应关系。ABI 记录不要求 tuple、函数类型或指针类型为了成为参数而拥有独立 layout/TD 定义。

本Strong callable export的目标拥有Scoop body；既有`NativeSafe/NativeBorrowed`继续只由native contract与相应callsite承载，不能给本export伪造native分支或Strong extern definition。Scoop extern的NoGc不等于本表的OrdinaryNoGc，直接source extern production gate不变。

canonical signature原样复用M23-2的 `CanonicalScoopAbiFunctionSignature`：field 1 exact signature、field 2 logical arguments、field 3 result、field 4 GcEffect；storage仍是exact type/byte size/alignment/scalar-or-aggregate的既有product。参数tag为ElidedZst=1、Direct=2、Indirect=3；result为UnitVoid=1、ElidedZst=2、Direct=3、Indirect=4。

生产端从同次 MIR/LIR 已生成的完整 canonical ABI 和实际 callable body 组装记录，与普通 callable 共用签名投影。reader 根据实际 MIR lowered signature 和已验证的共有布局检查新读入的 canonical ABI；结构类型递归查询实际 nominal 表示，不依赖逐参数 layout ID 外层。dispatch 需要 receiver 表示时沿共有布局查询取得，定义、签名、effect、GC 和物理符号关系仍须一致。调用签名本身不产生独立 layout 符号或 Link 依赖；实际布局、descriptor、dispatch、初始化与 relocation 使用各自真实引用。

当前Darwin/AArch64 classifier：scalar、qualified pointer、niche enum为Direct；非ZST tuple/ordinary struct/tagged enum/exception record为Indirect；ZST input为ElidedZst；Unit result为UnitVoid，其他ZST result为ElidedZst。不得按aggregate大小或system C classifier另选pass mode。

validated LLVM backend profile 的 frame-pointer=all 策略适用于全部 Scoop callable，包括 NoGc 函数；GC strategy 的设置仍由 typed GC effect 独立决定，NoGc 不得因 frame 策略而加入 statepoint GC。LLVM producer 与 rewritten-module verifier 必须同时应用和核对该 frame 策略，使 NoGc 次构造器等包含普通调用的函数保持 canonical frame，避免因遗漏 profile 属性而产生预定 associated-atom 闭包外的 unwind-only EH frame。managed 的 tail-call 与精确根规则继续按既有 profile 执行；associated section、CIE/FDE、compact unwind、LSDA 和对象范围的完整校验仍然必需，不能通过忽略额外 backend section 接受不完整定义。

### 8.2 physical signature与调用

BoxingAdjust 的 MIR receiver、对应 local 与正文必须保留实际接口类型，和既有 exact callable signature 逐项相同。不得通过 Any 擦除再豁免 nominal signature 校验；接口 receiver 的物理表示仍由共有 managed-reference ABI 处理。

physical参数顺序只计算一次：若result indirect，首参数为result storage；随后按logical顺序跳过ZST、发出direct value或indirect pointer。每个indirect参数有fresh exact caller storage，callee遵循按值语义。

LLVM definition、call、invoke、dispatch和Scoop extern在同一physical index使用 `byval(exact LLVM type) align N`；indirect result使用 `sret(exact LLVM type) align N`。显式statepoint wrapper把callee参数attribute平移到intrinsic参数 `5 + i`。codegen从同一checked signature计算，禁止维护第二张可独立修改的physical表。

全部ZST实参仍按源码顺序求值。callee只在观察参数地址时分配token；用户ZST result产生typed logical value，不借Unit sentinel丢失exact type。两个source callable物理签名相同也不能共享identity、symbol、override slot或ABI fingerprint。

### 8.3 GC与异常边界

跨 Cone 的 `throw`、语句及表达式形式的 `catch` 使用前端已解析的实际 `Throwable` 声明，沿共有依赖类型查询取得继承关系并执行同一子类型规则；导入协议不允许跳过检查。同名普通 class 不能替代该实体。异常构造、默认参数、调用、布局、TD 和展开使用共有路径，保留调用求值顺序、catch 顺序与 finally 语义；此能力不增加协议资格、独立证明、wire 字段或 runtime ABI。 公开存储属性的 getter 与可公开调用的 setter 必须按实际 owner 和声明类型生成普通 callable body，即使 provider 的正文没有引用该属性；名义类型、顶层属性与 core 使用同一规则。参数自由且可物化的属性自动导出实际 body。泛型或 source-only owner 仅用于签名和表示查询时，不自动实例化其普通方法或 accessor；实际调用仍通过同一 typed 请求生成完整实例，dispatch 所需的方法继续随其实际类型物化。

跨 Cone 的强制 `as` 沿实际 `ClassCastException` 声明查询完整异常类型，并选择该声明的零参数 constructor 或默认参数适配入口。成功检查保留原对象身份，失败通过普通 class 分配、外部 initializer 调用和 throw 执行；别名、interface 与装箱值沿同一类型与 ABI 路径处理。默认参数中的转换在实际展开时进入相同路径，未求值模板不触发机器物化。异常类的表示仍依赖后续泛型能力时，前端在输出 LocalConcrete HIR 前对实际执行点给出已有 layout-required 诊断；不输出缺少所需表示的成功 IR。MIR 直接消费完整协议中的 typed 声明引用，不再建立丢弃外来声明信息的 Core/Imported 资格投影。所选 constructor 使用共有依赖 callable 集合，保留真实逻辑/物理签名、GC effect、ABI 和 relocation，不增加转换调用的证明记录、wire 字段或 runtime ABI。零参数默认值 adapter 以已有 GeneratedCallable 实体及 ClassInitializer 表示进入共有 callable 导出，不能只发布协议引用而遗漏定义。MIR/LIR selected callable 记录本身是机器依赖引用；reader 按 provider、typed target、定义、签名和传递依赖检查其完整性，不要求隐式调用另附一份 HIR 操作资格记录。

含ref aggregate按已验证scan和typed storage拆为AS1 leaf，不用byte array擦除provenance。ordinary managed call/invoke复用M15 root plan：invoke前root frame同时覆盖normal/unwind存活leaf及可移动实参，两个后继reload并pop，不产生exceptional gc.relocate。

Indirect参数/结果storage、value receiver copy、外部field内联ref与dispatch receiver都参与同一活跃性/root plan。post-statepoint只使用relocated/reloaded值，不能从旧indirect temp缓存ref。

普通NoGc与native Scoop NoGc不是一种callsite。Scoop extern无论GcEffect值均保持NativeBorrowed/caller-root publication；C bridge继续NativeSafe。effect轴不改变ordinary/suspend signature identity，也不使direct source-extern能力提前开放。

## 9. ZST place、boxing 与 static storage

### 9.1 logical value与place

LIR区分 `LogicalZstValue { exact }`、`AddressableZstPlace { place, exact, alignment, lifetime }` 和nonzero storage。token需求在MIR保留、LIR定稿；codegen不由LLVM store size为0反向发明place。

parameter/local/value `this`真正取址时分配non-null、至少1-byte、满足alignment的token。有效期重叠的不同semantic place不得共址；重复取同place地址稳定。token不能标成可合并的 `unnamed_addr` 常量，也不能通过共享零地址/singleton实现；不重叠lifetime允许复用。普通SSA ZST、field和array element不自动取得token。

class 字段与 closure capture 的 ZST payload 读写在 MIR→LIR 阶段消除。lowering 先按既有顺序完整求值 receiver、capture initializer 和 assignment RHS，再使用同一 storage 分类区分零大小与非零存储；零大小读取显式产生携带 exact type 与 AbiZst 的 LogicalZstValue，零大小写入在保留求值后结束。不得向 codegen 传递 offset 0 的伪 payload HeapLoad/HeapStore，也不能让 codegen 根据空 LLVM aggregate 猜测语义。closure invoke pointer、非零字段和 compiler-owned state 继续使用各自已验证的偏移与指令；ZST 不改变对象头、继承 prefix、GC scan 或字段身份。

### 9.2 box/unbox执行路径

整数与 Boolean 的接口取自实际 intrinsic 类型声明，包含其普通 `ToString`、`Hash` 及用户扩展；前端完整解析后沿共有值类型表示进入后续阶段。MIR 按 payload 的 canonical 类型关联真实 struct 声明，不能通过 Defined core 协议取得额外操作资格。类型表示直接保存 typed intrinsic kind，删除来源编号包装；源码文件索引仍只用于正常源码位置与诊断。

前端按需保留查询结果；具体化只为实际 Box、value receiver 和函数变型适配请求相应 primitive 表示。未使用的 arena 条目、失败候选及未求值默认模板不成为物化根；默认参数实际展开时使用相同请求路径。

需要独立 Structural ODR adapter 的函数变型仍由 M23-7 开放发布；本阶段保持其 HIR/MIR 装箱输入完整，并验收明确的 ODR 能力诊断，不将此类 adapter 改成 Strong 来绕过阶段边界。

外来 struct/enum 的完整 HIR 包含真实声明、字段和解析后的接口；具体化保留父接口闭包，不能将它清为空表。相互引用的类型先登记真实 ID 再完成记录，以支持接口签名引用实现者和经 class 的间接递归。装箱沿既有来源 exact type 选择实际 provider 的 boxed TD 和 dispatch 定义；消费者只保存所需表示与类型关系，不复制外来方法或生成第二套 adjust thunk。验收用真实源码和隐藏 provider 源码后的产物消费，覆盖接口默认方法、父接口、ZST、大值、含引用 payload 及 core 扩展，并链接运行普通与移动 GC。

```text
BoxPayload = ZeroSized | NonZero { source_place }
UnboxResult = ZeroSized | NonZero { destination_place }

scoop_rt_box_zst(td)
scoop_rt_box_value(td, source_place)
scoop_rt_unbox_zst(object, expected_td)
scoop_rt_unbox_value(object, expected_td, destination_place)
```

接口严格按runtime spec 2.3：size/alignment/inline_offset/scan只从TD读取，删除旧 `box(td, payload, size, scan)` 和固定`+16`路径。ZST入口没有payload/result pointer；box仍分配TD规定的非零managed object并取得fresh ref identity，unbox先检查exact TD再产生logical value。

nonzero source place须地址稳定、对齐且在call前已写入完整值。inline scan非空时caller先经compiler-private NoGc leaf `PushRecursiveRegion`登记该temp，再进入box runtime和可能park的managed-entry handshake；root保持到分配、从collector更新后的同一temp复制及返回完成后才LIFO pop。所有非fatal出口配对；root entry 的 scan 直接加载实际 TD 的 inline scan，runtime 检查活动 root 的 place 和 scan 指针，不重新发射 callable 专用副本或展开比较静态 scan。空scan可以省略frame。完整静态 shape/scan 交叉检查由编译器和外部 reader 负责，runtime 的 `SCOOP_VERIFY_METADATA=1` 可显式重验；动态对象范围、长度溢出、TD 和 GC 契约始终检查。

box payload不能作为可观察的value `this`存储暴露；adjust thunk初始化独立方法局部值，ZST需要地址时另建token。moving GC依靠完整object scan与精确side-metadata size，不依赖payload非零。

### 9.3 static token与初值

compiler-managed ZST storage保留persistent storage/unit identity，logical `byte_size=0`、allocation extent=1、scan None、canonical token byte=0。两种静态初态仍严格区分：有runtime unit的storage/failure/published root使用ZeroedForRuntimeUnit；无unit的静态值使用EncodedStaticValue，即使bits全零也不改tag。

EncodedStaticValue的template恰覆盖allocation extent，padding/pointer leaf初始归零，immortal relocation按offset排序且只指向已登记immutable object-start。ZST token没有relocation，None scan使用既有static sentinel。ordinary property不因有token开放`addressOf`；本地raw global/TLS遵守原GC-free和lvalue规则，C-boundary ZST storage仍拒绝。

本阶段immortal仍限于既有String表示，immortal 的类型 registration 引用迁为 Local 或显式 provider 的 DependencyExternal，旧 CoreExternal 随补充设计退出；imported const String继续按Stage5在consumer生成自己的literal/immortal，不引用provider immortal地址。`StaticImmortalRelocationPlanV1`继续只解析本Cone immortal producer表，不能因通用layout API已存在便扩大为任意foreign immortal relocation。

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

## 11. 产物消费、Link 与 fingerprint

### 11.1 检查顺序与结果复用

```text
envelope / profile / target / byte ranges
  -> typed identities + direct/support dependency graph
  -> complete shared HIR/MIR/LIR metadata and reference relations
  -> layout / canonical ABI / dispatch / selected physical imports
  -> Link object ranges / symbols / relocations / registrations
  -> Code/runtime fingerprints + atomic publication
```

产物读取按明确边界完成检查并保留结果：envelope 验证长度、目录、版本和内容 hash；共有 HIR/MIR/LIR 解码验证格式、typed 引用与跨层关系；Link 在该结果上追加对象范围、符号、relocation、registration 和 Code/runtime fingerprint 检查。同一字节快照及其依赖不分别执行两轮完整 Compile、Link 语义重放。发布复用同次编译的完整 IR、已检查依赖与最终对象，写入后只核对实际写入内容，再原子替换目标。外部新输入仍须经过对应读取边界；结果改变时重新检查受影响部分。

共有 HIR 类型位置的结构、foreign nominal 分发、真实 provider 与定义/求值位置在 HIR reader 边界检查一次。后续物化查询和 MIR/LIR 消费同一未变化的 typed 记录，不重新完整检查这组 HIR 关系，不建立额外验证状态或凭证；实际类型表示、签名、ABI、对象与传递引用仍由相应边界检查。外部字节重新读入或相关数据发生变化时重新验证受影响部分。这一职责调整不改变 wire、内容 fingerprint 的字段组成或 runtime C ABI。

同次编译完成的归档直接保存完整产物及其普通摘要；摘要由已有 manifest、语义 fingerprint、最终对象和生产记录产生，只记录后续构建与缓存实际需要的信息。原子发布直接写入这些归档 bytes，回读仅核对字节一致后替换目标，不重新创建语义会话或把当前产物和全部依赖再次交给 reader。外部输入的产物仍由共有 reader 检查格式、引用、ABI、符号和实际对象。删除已退出正式路径的 M23-5 归档 writer、双版本发布包装及其专用测试；发布接口只接收正常编译结果，不保留用于取得发布资格的 raw-bytes 重放入口。此清理不改变 wire、runtime ABI 或 String 表示。

源码的名称解析、访问规则、类型检查和默认值定义检查由前端负责；已解析的默认正文按 typed 引用实例化。IR/meta crate 保证完整结构和格式，reader 验证引用、归属与跨层关系，不再逐级重演语言语义或维护另一套 source-authority。实际 provider、typed target、签名、effect、field/variant/slot、初始化 unit 和物理 definition 必须对应，错误或损坏产物拒绝。继承和按值环拒绝，managed reference 递归在引用 leaf 终止。

同次编译的完整 IR 直接生产导出、selected 与对象记录。只为重新制造候选表并比较其来源的工厂、source join、凭证状态机和重复投影退出正常路径；重型交叉核对仅在有价值的测试或显式验证模式中运行。内容 fingerprint 可用于确认同一快照、缓存失效和对象关系，不作为授权证明。

Link 复用已有语义结果，完整验证对象目录、unit/member 归属、实际 byte range、typed relocation target、符号唯一性、初始化/GC registration、digest patch 和最终 Code/runtime fingerprint。编译与 Link 共用的不可变记录不复制或重解码；只有 Link 特有数据进入其检查。写入失败保持原目标完整，成功后原子发布。

### 11.2 semantic dependency与physical use分开

编译只读取外部field offset、size或scan事实时，可以没有对provider layout constant的object relocation；只读 TD metadata 同样不伪造地址使用。反之call、TD address、parent/interface table、dispatch target、registration与object ensure的实际relocation必须逐条验证。

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

Layout/Type/Dispatch分别复用6.1/7.1的canonical semantic record，去掉definition/registration的后置digest槽；Scan保存完整scan tree而非仅digest；storage/unit projection逐字段复用strong-production/12对应semantic plan，不包含member/range或后置object/registration digest。它们不是任意bytes，reader按subject取得provider同一plan并逐字段比较。required definition由subject和owner重算，provider必须真实Strong定义它，consumer defined-symbol set必须不包含它。

新projection保留原字段编号：Layout恰为exact-layout record的fields1～6，Type恰为exact-descriptor的fields1～8，Dispatch恰为exact-dispatch的fields1～4；三者分别排除field7、fields9～10与field5的definition/registration后置槽。StaticStorage恰为strong-production static record的fields1～10与32（storage、symbol、layout、scan id、完整scan、scan kind、logical size、allocation extent、alignment、initial state，以及实际layout provider）；Initialization恰为unit record的fields1～8（unit、diagnostic path、schedule、storage、failure root、initializer、ensure、ordered dependencies）。这些语义字段均不含后置digest，已有initial-state、schedule及V2 dependency的wire保持不变；typed dependency 关系仍在 provider 同一 strong-production/12 计划中保存，并通过真实 unit 声明解析。每个projection复用完整record的对应字段编码。static-storage的完整record与projection同步新增field32，旧格式产物重建。

七种contract的新wire均使用field0保存tag：CallableAbi为`{0:1,1:canonical_signature,2:calling_convention,3:protocol}`，Scan为`{0:3,1:layout,2:role,3:canonical_scan}`，其余五种为`{0:tag,1:对应typed projection}`。import的五字段依次为provider、subject、expected symbol、required definition、contract；canonical数组按provider与subject的组合canonical bytes严格递增，不合并重复项。provider查询先复用subject的唯一Strong definition/primary/symbol规则，再与同provider的实际Strong production及完整semantic records核对。storage/unit查询协议只返回候选semantic plan：共有reader封闭持有具体查询实现，直接关联同一MIR导出、selected object/unit与Strong计划，不接受调用方来源工厂；import重放再将候选与provider实际unit及其storage/failure root逐字段join。该查询没有独立来源资格，逐次源用途及完整object使用覆盖仍由包含它的最终section负责证明，候选本身不授予机器import或selected资格。读取边界按实际 provider 的 ABI/layout/scan/TD/dispatch 记录建立并核对 contract，完成后结果保存选中合同的完整 typed 数据；后续查询复用该结果，不再将相同不可变记录重新绑定或检查整表。

共有 layout reader 返回拥有实际 section、物理 import 和 Link 结果的完整数据。建立依赖合同只在查询期间借用已完成 provider，返回数据不借用内部 arena，不通过高阶回调控制“使用资格”。构建图及后续 lowering 可直接保留和查询结果；编码仍采用上述七种合同的原字段和 tag，不改变 wire、fingerprint 或 runtime ABI。

requirements 沿用 M23-5 canonical relocation-use 结构和排序，并引用本 section import index；每个 actual relocation 恰有一项。metadata 或 registration 使用的 import 可以没有机器 relocation，不要求通过额外计数证明使用资格。初始化依赖按既有 unit id metadata 表示，不伪造 object relocation，也不修改 runtime ABI。Compile/Link import 表与 Code contribution 保留完整 typed 支持记录，各边界检查真实 provider、定义、ABI 和引用关系。object coverage 绑定全部最终 LinkObject 成员集合及 canonical use set，digest 使用 `DomainSeparatedCborHash("scoop-cross-cone-layout-object-coverage-v1", { verified_link_objects, relocation_uses })`，仅作 LinkValidationOnly。

本section的三字段编号依次为semantic_imports、requirements、object_coverage。每个requirement为`{1:canonical_relocation_use,2:import_index_u32}`，严格按既有`(member, containing_atom, offset_within_atom, target_slot)`排序且唯一；只从共有 strong dependency 分类后的 remainder 中按 target 规范化后的 expected symbol 匹配，已由其他 capability/subject 认领的 symbol 不得再次认领。未匹配的native/runtime候选留给后续既有分类，不得丢弃。coverage为`{1:verified_link_objects,2:relocation_use_set_digest}`，其hash preimage为`{1:verified_link_objects,2:canonical_relocation_uses}`，uses保留完整十字段而不含import index。最终对象证明必须与分类输入的同一verified relocation closure逐成员内容和完整binding join，不能仅以producer或member id相等替代。再同独立重建的requirements、全部最终objects和coverage digest精确比较；raw wire本身不授予分类或对象覆盖资格。

### 11.3 共有 requirement 与 relocation 分区

所有外来 Scoop strong requirement 使用实际 provider 和 typed target 进入共有 symbol/definition 闭包，String TD、初始化 callable 及其 registration 也使用该入口。删除 core 专用 requirement proof、独立 core owner 参数与先验证 core bridge 的步骤；不保留 CoreStrong 来源资格。

object undefined-use 按实际 capability/subject 分区：既有 callable、general type/dispatch/shape 以及 runtime/native/target 各保留必要语义，use site 互斥且联合精确覆盖所有 nonlocal undefined relocation。producer 身份不决定权限或分区；wire/profile 与 Code contribution 随专用通道退出同步迁移，旧格式不能静默升级。详见补充设计 2.5、3。

ObjectDefinition relocation规范化在既有tag1～11之后新增tag12 `DependencyShapeStrong { provider, subject }`；它只从新Link proof派生，不按symbol解析。该hash路径使用既有object-definition runtime scalar encoder：`u32(12) || provider.raw32 || u32(subject_tag) || subject_payload`；Callable payload复用既有StrongCallableDefinitionOwner runtime编码，其余payload是对应typed id的raw32。这里不是Wire CBOR，不套用3.3的map格式，也不改变只供callable-body identity使用的`RuntimeEncode(key)`契约。tag11继续仅表示M23-5的`DependencyStrong`。这是受新required capability保护的object-definition target扩展，不改persistent identity key或旧capability payload。

object verifier既检查undefined target，也检查provider实际TD/scan/table bytes、alignment、field offset、ABI adapter和typed relocation。单靠object symbol table不能证明Scoop signature；ABI一致性由source→MIR→LIR→emission关系和object evidence共同证明。

### 11.4 hash与cache

新Link section的Code贡献沿M23-3 `KnownLinkExtensionCodeContributionV1`：capability为新section id，payload精确为canonical `semantic_imports` array，空时也存在。Compile selected的physical projection、Link section和Code contribution逐byte三方相等。

layout、scan、LIR definition和registration沿既有 `scoop-layout-v1`、`scoop-scan-v1`、`scoop-lir-definition-v1` 与digest DAG算法；不改hash encoder。新semantic record覆盖exact identity、target layout projection、字段/variant/prefix、ABI、scan、TD name与dispatch relation。layout不依赖provider code digest，metadata/table间指向只用typed identity，避免互相引用TD/dispatch/function形成digest环。

runtime-image fingerprint通过既有strong registration/digest graph覆盖新定义。MIR/LIR semantic bytes不包含SlibMemberId、object range、atom placement或host路径；object重新分片只改变对应physical/code/artifact部分。

M23-4 cache继续保守纳入全部direct dependency各层fingerprint，不在本阶段按selected set裁剪。field、base prefix、slot、default/access域、ZST status、layout/scan/ABI或target变化必须使对应consumer层失效；provider body-only变化仍由provider code和后续program-link闭包跟踪，不复制body到consumer HIR。

## 12. 诊断

source错误仍在parser/HIR结束，并断言主span与必要的provider声明/字段路径note：

- 非法protected receiver/词法位置、不可见setter或constructor。
- final base继承、非法override、未实现abstract slot、冲突interface default。
- C ABI ZST参数/result/global/TLS/pointee、空/含ZST CLayout。
- 非lvalue addressOf、非法private backing access。
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

每条编译错误规则都有独立negative fixture并断言span/message，不以单一“大型失败程序”覆盖全部规则。既有普通 callable fixture 与新增 nominal/member fixture 一同覆盖共有查询，包含 direct member 同时参与 dispatch 的组合；不得以历史分区限制合法调用。

初始化运行验收使用实际编译的 core、provider、consumer 和下游产物，在隐藏上游源码后消费。修改并重建 core 的初始化循环函数，检查单例引用复用、传入的 String 循环信息、异常失败缓存和重复读取（按既有 throw 的 payload 复制规则，断言初始化次数与异常内容，不要求多次 catch 的指针相等）；组合场景包含外来值装箱、interface/default 调用及 ZST 字段。单 image 运行 harness 从实际 registration 记录取得静态 storage 与 scan 符号并登记 GC 根，使用产物中的真实初始化 descriptor 与 ensure 调用，在普通和移动 GC 模式运行。该验收不新增多 image 启动或 artifact-only program-link 能力。

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

- 四条新增 capability、strong-production/12 和新 profile fixed vectors；覆盖 empty/nonempty、unknown required、错 purpose 与旧 profile 拒绝。M23-2 foundation/extern/callback 及 persistent identity vectors 保持；按清理设计退役 HIR 协议定义和 strong production 的旧格式，并更新对应 vectors。共有 metadata 覆盖缺失、额外、重复、错 origin、错 access/receiver、错实际 use、错 provider 与跨表不一致，并通过仅 artifact bytes 的 prebuilt/cache 路径重放。V2 ordinary parent、itable key、dispatch target 的正反例经过真实 strong production wire round-trip；内部 constituent 测试不能代替完整 profile。
- 每个record去掉/增加/错tag/错kind/错owner/乱序/重复逐项拒绝；reader独立重放字段layout、scan、ABI和source-root obligation。
- scan/type-name 共享 DAG 正常处理；cycle、实际范围越界和长度溢出拒绝。合法递归 ref class 成功，by-value 环失败。删除仅验证预算与重复证明的测试。
- 别名/re-export spelling改变不改变exact TD name/ABI；改变base prefix、ZST exact identity、slot contract或scan offset改变对应fingerprint。
- metadata-only外部layout没有伪relocation也能成功；physical callable/TD/table use缺relocation、错definition或多归属失败。
- 三路undefined-use分区两两不交且并集完整；consumer定义foreign Strong、weak/ODR symbol、错误TD/scan/table bytes或关联diagnostic atom均失败。
- 相同semantic program换source枚举、dependency枚举、arena顺序、object分片不改变相应semantic fingerprint；physical/code/artifact变化遵守既有规则。
- producer field/base/interface/ABI变化触发consumer cache miss；失败child/invalid artifact不得发布cache；source/direct/support graph回归不受影响。
- consumer initializer读取provider object/property时保留external unit dependency；V2 round-trip验证provider与descriptor，缺unit、错provider、伪造local、省略真实edge逐项拒绝。callable内部provider自有ensure不被consumer复制。
- class prefix单独覆盖base尾padding、derived更高alignment、ZST own field和abstract base；同Cone与跨Cone布局逐字段一致。slot位置锁定声明序语义，不受wire table按id排序、import或dependency枚举影响。
- 运行M23-1～5及本地value/GC/constructor/property/FFI/closure/coroutine回归，检查stage crate依赖方向。

## 14. 实现顺序

以下 layout 主线与 [core 清理的实施顺序](CORE-AUTHORITY-CLEANUP.md#4-实施顺序与验收) 共同执行。优先打通共有 native-boundary 类型查询与 ABI 重放，再迁移 protocol、String/初始化及 Link 消费者，最后删除专用包装；不得扩展新的来源授权体系。实际功能、七组清理、必要验收及按功能提交都完成后才能结束 M23-6。

1. 固定本文profile/section/wire constituent及测试vector；添加required capability gate和拒绝旧profile路径。先让生产与 bytes-only reader 使用完整共有 metadata 校验入口，再开放对应源码能力。
2. 收口HIR concrete facts与persistent inheritance surface，完成protected/default的前端语言检查和source negative；格式变化按版本规则要求重建。
3. 实现MIR type/constructor/object/slot bridge与selected closure；以golden锁定没有foreign body复制、layout offset或generic template。
4. 收口LIR storage/refined shape、general layout replay、scan normal form和external exact arena；实现provider有限shape-support导出和consumer验证。
5. 接入通用ABI及logical-to-physical映射，验证call/invoke/dispatch/byval/sret/root plan，再开放对应param-free source成功格。
6. 完成box/unbox、ZST place/static、array/Ptr执行路径与C边界矩阵；删除被替代的旧size/scan/header-offset入口。
7. 完成新Link-only closure、共有 requirement 与完整 object coverage、definition规范化、Code贡献和双view发布；删除 core 独立 closure 并更新core、scheduler、cache accepted profile。
8. 完成独立/组合/negative/golden/corruption与回归矩阵，将多Cone运行场景交给M23-9/11复用。

每批代码变更完成后先 `cargo fmt --all`、`cargo clippy --workspace`，再运行相关test；最终运行完整workspace与runtime/fixture验证。不能用最终linker尚未实现为理由跳过本阶段object和双view证明。

## 15. 完成门

- [M23 过度设计清理设计](CORE-AUTHORITY-CLEANUP.md) 的七组清理与验收矩阵全部完成；不存在按 CORE 身份授予额外协议/native-boundary/bridge/Link 资格或跳过 canonical ABI 检查的路径；也不存在通用计量、来源凭证、重复完整重放及只为这些机制存在的生产接口。必要 identity、签名、GC、布局、可见性、依赖与 object 验证已迁入共有入口；仅改名或删除旧 token/receipt 不足以完成。

- 完整共有 HIR 声明、source/access/inheritance、实际 selected use、MIR relation、LIR layout/ABI/scan/TD 与 actual object use 形成可从最终 artifact bytes 重建的完整校验链；reader 不需要独立来源授权 section、源码、compiler token、symbol/FQN 或 host layout fallback。
- param-free跨Cone构造、value/member/object使用、inheritance/dispatch/protected成功矩阵全部能生成新profile双view有效artifact，未选中的foreign body不复制。
- 每个合法source subject在定义Cone拥有完整有限shape-support；consumer只引用external typed definition，全部ODR生产继续拒绝。
- ZST logical semantics、typed ABI、place/static token、box/array/Ptr/C边界及scan/TD矩阵全部锁定；codegen/runtime不再从size0或空LLVM struct猜语义。
- nonzero box、indirect aggregate和跨Conefield的managed provenance/root/relocation完整；不存在握手后才登记root或从旧ref副本复制的路径。
- 新required section/profile、strong-production/12的完整foreign TD/dispatch引用、fingerprint/cache迁移和共有 Link requirement 与完整 object coverage 落地；既有 identity、extern/callback 契约保持，退出的专用 capability/字段按补充设计显式迁移。
- 独立、组合、negative、各stage golden与corruption/determinism回归通过；文档明确M23-7/8/9/10/11交接，真实多Conemoving-GC不被提前宣称完成。
