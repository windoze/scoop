# M23-6：普通编译器职责与过度设计清理

静态存储与初始化失败根按其实际值类型引用 layout/scan。当前 Cone 只发射自身拥有的布局与扫描定义；外来类型的静态根复用共有依赖查询取得的完整 value-layout 和 scan 记录，保留实际 provider、typed identity、定义与 relocation，不因本地持有该类型的值而重发射 foreign Strong。layout/scan 指纹节点引用已经解析的实际记录，不要求该类型在当前 Cone 定义；指纹补丁目标仍须属于当前产物。MIR 必须携带生成失败根所需的实际 Any 声明，LIR 不再缺省重建固定 core 身份。static-storage 语义记录新增 field 32 保存 layout provider，完整记录使用 fields 1～32；语义投影使用 fields 1～10 与 32。共有 strong-production 两种格式升级为 /11、/12，旧 /9、/10 产物和缓存重建。runtime C ABI、String 表示、初始化状态与失败缓存语义不变，不引入 ODR 或多 image 启动。

普通 catch 的绑定必须可以像其他引用值一样离开 handler：匹配 native payload 后，在绑定变量前物化一次 managed 异常对象，后续返回、存储和捕获使用该对象；native unwind record 仍按既有 cleanup 规则释放。初始化 catch 复用这次物化，不再次复制。每次 throw 仍创建独立 native payload，runtime C ABI 不变。

运行时类型转换的失败构造使用前端解析的实际异常类型与 constructor 引用，并沿共有的类型、callable、ABI 和 Link 路径消费。删除由 Cast 反向投影的独立 CastFailure call-site、RuntimeOperationDependency role，以及 reader 对同一目标再按 compiler protocol 进行资格判断的通道；普通源码调用的位置、参数、结果与 typed 引用检查保留。共有 HIR 格式更新为 `hir/cross-cone-interface/30`，原 call-site reason tag 2 与 external-reference role tag 9 退役，不复用；旧产物、profile fingerprint 与缓存重建。该调整不改变转换失败抛出 ClassCastException 的语言行为、runtime C ABI 或 String 表示。

引用上行转换在 HIR 中用显式 `ReferenceUpcast` 节点保存内部表达式及目标类型，不能直接改写构造、调用或局部读取的原始类型。MIR 使用已有 `Retype`，不分配对象、不改变引用身份；构造器仍按实际所属 class 分配。默认值正文使用新 expression tag 58 保存同一操作，tag 44 继续退役；共有 HIR 格式更新为 `hir/cross-cone-interface/30`，旧产物与缓存重建，不改变 runtime C ABI。

外来接口与动态调用沿共有类型和 callable 查询消费真实定义。删除 external callee 必须 Direct 的阶段限制后，由完整 typed receiver、所属 dispatch 表、槽位置、声明签名和实际 ABI 表达调用；不能为此添加新的来源工厂、dispatch 凭证或完整语义重放。

值类型装箱同样消费真实声明及其完整接口关系。普通 struct/enum 与 intrinsic 值类型不能因声明来自依赖而丢失接口或要求本地 core；实际 provider 的 boxed TD、接口表、adjust thunk 通过共有定义与 relocation 路径使用。保留真实声明 ID、接口继承、payload 布局和 GC 信息，不将其包装成来源资格或另建证明表。

清除 MIR primitive 装箱对 Defined core 的依赖，复用完整 HIR 中的实际值类型声明。`IntrinsicTypeDeclaration` 中的来源编号只曾用于前端重复声明诊断，不应传至后端或由外来表示伪造；删除该包装并直接保存 typed kind，前端从当前源码上下文取得诊断位置。普通 source 文件编号、声明身份与签名继续保留。

保留按实际使用建立物化闭包的规则。查询结果本身不是机器使用，不得将未使用 primitive、失败候选或未实例化默认参数批量加入布局、TD 或 Link 依赖；真实装箱与函数变型适配继续获得完整声明和接口信息。

此次清理不提前开放独立 function/adapter 的 Structural ODR 发布；这类真实请求继续在 M23-6 得到能力诊断，验收不能通过改变实体归属绕过 M23-7 边界。

共有导出绑定包含 enum 变体的真实 typed ID，nominal 的 `nested_bindings` 同时列出其静态命名空间中的嵌套类型、object value 与 enum 变体；变体归属由实际声明确定，不能误作包级值。`hir/cross-cone-interface/30` 更新该格式语义，旧 `/23` 及更早产物与缓存重建；既有 tag 不复用，不保留双轨 reader，runtime C ABI 和 String 表示保持。

enum 模式和变体测试不是构造器调用：删除默认值引用集合中仅为这些操作保存的构造器访问记录及 reader 对该记录的要求，直接消费正文已有的实际 variant、owner 与字段引用。保留实际构造表达式的构造器关系、前端可见性和类型检查；`hir/cross-cone-interface/30` 同步语义并要求旧 `/22` 及更早产物重建，不建立替代凭证。

Strong producer 的两种产物表示从完整 LIR 各计算一次类型、safepoint、immortal 与本地初始化语义，digest 与 registration 直接复用这些结果；初始化仅在 digest 身份可用后补入实际依赖定义。外部初始化引用在解析时完成 definition 与物理导入核对，后续由全局唯一的 local unit 引用表达使用归属，不保存额外 consumer 状态或重复选择核对。descriptor 和 dispatch 直接保存 LIR 中已有的外部 typed 引用，不反向重新 materialize 外部对象或再次比较完整 ABI。实体与 definition role 的匹配只查询实体种类和角色，不借用虚构的 CORE provider；实际 definition、symbol、relocation、ABI 和 GC 检查仍由各自消费边界负责。此清理保持产物字段、指纹内容和 runtime C ABI 不变。

canonical ABI 导出复用同次完整 IR 的实际签名和 callable definition，删除重复逐参数 layout ID 表及只为该表存在的重放入口。共有 reader 以真实声明和 MIR lowered signature 核对 ABI，dispatch 按实际 receiver 查询表示。`lir/cross-cone-layout-abi/3` 退役 callable field 5，其余字段编号保持；旧 `/2` 产物与缓存重建，profile 和内容 fingerprint 按格式正常更新。tuple 的字段与参数使用完整结构身份和已有存储算法，不为嵌套值补造独立 nominal、layout/TD definition 或来源记录。字段布局消费完整存储数据，C-layout 的嵌套合同与 enum niche 的实际指针种类仍保留；删除以字段内嵌数据为由追加 layout 符号依赖并再次比较完整记录的通道。

MIR 组装先排除已经由普通 callable 表保存的实际声明，再生产需要独立 lowering 的定义；不为同一函数重建第二份绑定、完整验证后再丢弃。boxing 与 dispatch 的共有查询同时借用当前 Cone 的普通记录和依赖记录。每个定义只生成一次，签名、GC effect 与 typed target 沿完整 IR 和已有表消费，最终表继续拒绝重复定义。

实际 HIR 调用在依赖 MIR 记录可用后完成一次调用根与逻辑签名核对，普通函数、accessor 与 constructor 共用该边界。MIR 和 LIR 的依赖集合分别从实际 typed 调用加入相应 callable 与 ABI 引用，沿共有 provider 查询消费已有定义；不得因旧函数表不含 constructor 而拒绝合法调用，也不在前置阶段和布局阶段重复核对同一调用。

共有 nominal 声明直接保存 struct 主构造器的 typed declaration ID，供前端按实际语言角色检查 `@NoGC` 调用；不能从参数形状、字段布局或 provider 身份推断主构造器。主构造器继续保留源码 Managed、物理 NoGC 的既有合同；值构造本身不分配，`@NoGC` 的参数、结果与局部值仍须 GC-free，managed 次构造器仍禁止调用。`hir/cross-cone-interface/30` 在 `NominalDeclarationDetailsV1` 新增 field 8：空数组表示无值主构造器，单元素数组保存其 constructor ID；有构造器的 struct 必须明确该引用，引用必须属于同一 nominal 的声明集合，其他 nominal 不得填写。旧 `/21` 及更早格式退役并要求重建，既有 tag 不复用，profile 与内容 fingerprint 正常更新；MIR/LIR callable 格式和 runtime ABI 不变。

跨 Cone struct 构造器使用实际 provider 与 Strong callable target 进入共有请求内选择；其完整逻辑／物理签名来自已发布定义，不能伪装为普通函数 ID 或重建固定 core 身份。主构造器的源码 Managed 合同与实际 NoGC 值入口分别保留。Strong 外部根不重复携带 function/accessor 资格 ID，LIR 使用现有 layout/ABI 与物理 import 物化调用，并复用于 dispatch；不增加构造器专用来源工厂、授权外层或平行导出表。

MIR 输出在 HIR→MIR 边界完成一次整模块结构、类型与实际外来 callable 检查，并同时保留已生成的 canonical foundation、共有依赖选择和完整物化引用。通用 MIR 输出保留完整泛型实体；Strong profile 的 ODR 能力门仍在其消费入口检查，并共享已有 canonical foundation。driver、MIR production 组装和 MIR→LIR 直接消费同一完整输出，不再从未变化的 Module 重建第二份 foundation、重复验证外来调用或重跑整模块检查。production 与模块之间仍核对实际 callable、入口和初始化关系；直接借用已有签名记录，不构造第二份预期桥表。外部新产物的格式、引用、ABI 与对象检查继续由 reader 负责。此清理不增加凭证、状态机、wire 字段或 profile 版本，不改变 runtime C ABI、String 表示或后续里程碑范围。

产物 reader 的 MIR 类型和 callable 校验使用同一次 HIR 类型基础与继承图构建。按真实依赖顺序解析完整 MIR 类型、callable 和 dispatch 后，在同一 provider 的借用范围内核对类型表示、有限 shape、方法、构造器、object、派发、equality 与初始化契约；不再为类型和 callable 分别完整重建未变化的 HIR。后续 LIR 消费完整的 MIR 结果，保留各自需要的格式、引用、签名、ABI 与对象检查。此调整不改变 wire、profile、runtime C ABI 或 String 表示，也不增加来源工厂、资格状态或缓存凭证。

完整 LIR 输出已保存 canonical foundation，codegen 不从同一未变化的模块再次构造该 foundation。LIR 在既有 Strong 能力边界检查 ODR 后，只追加当前 Cone 的 Strong 定义，不再次扫描未变化的 callable/ODR 记录。production 的符号表直接使用本次生成的定义表，对象分区直接使用 production 的符号记录；这些数据在 production 组合边界检查后，codegen 按 typed definition、atom 和 symbol 解析每项实际发射引用，复用完整记录，不重建整份预期表或再次完整比较。C bridge 与 C layout 入口只检查实际 C ABI、callback 声明及所需类型关系；callback 指令的 typed 引用与操作结果、Scoop CFG、dispatch、safepoint 与 root plan 在对象代码生成边界检查；不在每个无关入口重复完整验证。外部产物读取的格式与引用检查保持，wire、fingerprint 字段和 runtime ABI 不变。

依赖 companion 与 static nested 访问复用共有 reader 的静态命名空间和原 typed binding，前端在值遮蔽之后沿实际 owner 逐段解析限定类型与表达式。转发调用、属性和默认参数使用实际 object receiver，并沿普通 singleton ensure/root 路径消费；限定 host 和 const 访问不额外初始化。限定或直接导入的 companion 属性赋值、复合赋值与自增复用共有 getter/setter 路径，保存一次实际接收者，在右值之前完成接收者求值与初始化。验收覆盖命名 companion、Companion 别名、转发、跨 facade、类型别名及可见性错误，由真实源码产生四 Cone 产物后链接运行。此项补齐真实声明关系及其消费路径，不引入来源凭证。

`hir/cross-cone-interface/30` 补齐 object 的声明种类：source-shape 的旧 Object tag 8 退役，新 tag 9 保留 field 1=value、field 2=声明序字段，新增 field 3=`Standalone(1)` 或 `Companion(2)`；host 沿已有声明 key 的 typed owner 查询。命名 companion 发布名称与 `Companion` 两个普通 type/value binding，object 的公开方法和属性进入自身静态 binding 表。共有命名空间在本 owner 无同名 binding 时，沿已声明的 companion 关系转发其直接 binding；不复制成员声明、不用名称或初始化 metadata 推断 companion。旧 `/28` 产物与缓存重建，退役 tag 不复用，runtime ABI 不变。

产物的 identity graph 从 manifest 的当前 producer、实际直接依赖和已经读取的依赖实体构成；不无条件注册 CORE 身份，也不为 CORE 设置单独的重复过滤规则。CORE 与普通 provider 的声明使用相同的 typed 引用解析，缺失依赖、身份冲突及非法引用由共有格式与引用检查报告。默认 core 依赖仍由正常构建与前端依赖发现加入 manifest；本项不改变 wire 或 runtime ABI。

代码生成在入口对本次完整且不可变的 LIR、目标 profile 和 executable 入口完成一次必要验证，然后按实际定义分成函数与非函数对象。各成员直接消费同一 LIR，不因发射另一个对象再次完整遍历类型、ABI、CFG、GC roots、safepoint 或身份表；LLVM 变换后的 IR 和新生成的对象字节仍在各自边界检查。generated-C 源码入口同样不在内部 helper 重复整模块验证。这一职责调整不改变产物格式、runtime C ABI、String 表示或链接语义。

普通 final 成员与其他 callable 共用真实 provider、typed declaration 和 implementation 查询；移除“普通导出不能被 dispatch/类型闭包引用”的旧分区限制及其身份名单。MIR 直接导出补全 GC effect，LIR dispatch 复用实际 ABI 并只保存 implementation/body 引用。无 namespace 导入的成员调用不伪造 binding，不在选择集合中另存用于认证的 import 路径。格式升级、旧 major 退役和完整消费规则见 [M23-6 设计](DESIGN.md)；runtime C ABI、String 表示与 GC 契约保持。 struct 字段读取直接保留真实 field ID 并使用共有布局；计算属性消费实际 getter 引用及 ABI，均无额外来源或资格外层。 成员 setter 与 getter 使用同一属性声明、真实 accessor 和普通 dispatch，不建立写入授权外层；可见性、值类型与 GC 写屏障仍按各自语言和内存规则处理。

本次来源框架清理覆盖所有 Cone：旧 HIR foundation/declaration transcript 及逐层 `Bound*` 包装只由测试工厂消费，应删除相关构造器、编码器、绑定链和专用测试。保留 `source_authority` 中仍被生产调用的普通声明数据、默认参数数据和身份查询；以共有源码编译、reader、链接与运行回归确认清理结果。

类型生产器直接返回完整的共有 type section；MIR 使用已有继承边、槽序、typed 实现目标及成员引用。类型物化和构造器选择查询已有共有 nominal/callable 声明，不另行投影来源 foundation、完整 protected 声明、构造器签名或参数协议来重复证明同次产出。删除这些副本的生产工厂、凭证外层、只用于副本的 reader 及测试。必要的类型、引用、继承、ABI 与格式检查保留在实际消费边界，未变化的数据复用已有检查结果。

所有可见性的 nominal、callable、property、参数、默认值和定义环境由共有源码接口完整保存。protected 成员仍以 typed 引用参与实际 MIR callable 选择，其可见性和签名读取同一声明；构造器使用共有 nominal 的 constructor 引用及对应 callable，不在 inheritance record 再保存一份 payload。generic 词法 owner 不因访问域查询而要求 machine exact type。名义类型的 lookup/inheritance/slot 三份派生域不再保存和重验。

`CrossConeTypeSemanticsSectionV1` 保留 field 1、2、3、8，field 4～7 退役；`NominalInheritanceInterfaceV1` 保留 field 1～4、7～9，field 5、6 退役，退役字段不复用。成员引用的 Constructor tag 2 随重复构造器通道退役，实际 constructor 始终使用共有 typed 声明。HIR `cross-cone-type-semantics/8`、required inventory、profile 与内容 fingerprint 同步更新，旧产物和缓存需重建；不保留旧来源副本的双轨兼容，不改变 runtime C 调用约定或 String 表示。

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。 依赖查询直接使用真实 `ConeIdentity` 与 callable、property、type-alias 的类型化声明 ID；选择集合按这些 ID 保存完整接口和实际依赖路径。删除独立 world/projection/selection 品牌、仅为品牌服务的计数器和错误，以及从声明 ID 再映射到局部 u32 的三套重复表。候选选择依照当前依赖目录中的实际声明与表示，不要求由同一查询实例铸造；直接依赖的名称可见性、转导出路径、实际 provider 和引用完整性继续按共有规则检查。HIR 依赖目录、候选和已选声明只保存实际 provider 与类型化声明引用，不逐项复制 artifact 坐标/fingerprint 凭证；provider 身份直接来自已导入的共有 foundation，不再提供独立 certificate 或重算坐标身份。直接依赖与传递依赖使用同一输入数据和查询实现，直接依赖集合只决定当前源码可见的 package/public binding；不以分离的输入、视图或 seed 包装授予枚举资格。转导出保留实际 binding route，删除逐候选的 provider 凭证、重复 terminal 声明及其包装；候选选择使用已解析的声明目录，不重放未变化的 route 与 provider 证明。HIR→MIR 使用同次编译的依赖快照及完整声明，核对实际定义和签名，不再次比较来源凭证。普通构建依赖记录与缓存 fingerprint 继续承担定位和失效职责。MIR 外部 callable 引用保存实际 provider 与类型化声明，不另映射到带会话品牌的局部编号；调用位置保留 GC effect。MIR 类型与 LIR layout/ABI 选择按实际 provider 和 typed target 直接返回完整记录，不先铸造并验证中间 handle；依赖闭包、类型、ABI、物理引用和 GC 契约仍在其消费边界检查。同一声明或 target 在另一选择集合中是否存在，按实际目录查询决定，不依据集合生成顺序或计数器。该进程内数据简化不改变 wire/profile、实体身份或 runtime ABI。

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

整数 `div/rem` 通过共有依赖声明解析 `ArithmeticException`，并与强制 `as` 的失败分支共用普通类型、constructor、ABI 与 relocation 路径。只有实际运行点物化异常表示；未使用的默认表达式不产生机器依赖。删除 Managed 整数运算的旧无条件布局门和 MIR 的 Defined core 要求，不另存来源或操作证明。验收覆盖正常除法/余数、signed 边界、除零 catch、默认参数、修改 core 后的默认构造器适配，以及产物的下游链接和移动 GC 运行。

删除 CORE 来源资格、Core/NotCore 包装、重复 operation/签名投影和凭证外层。语言角色只保存实际需要的 typed 声明引用、签名、effect、可见性与依赖关系。普通 metadata 发布和消费这些声明；intrinsic 正规化后不再从来源获得执行资格。internal 服务不因迁移加入 public lookup。

Lowerer 直接保存已解析的 `ImportedCoreProtocols`，Concretizer 直接借用 Export HIR 的 `CoreProtocols`，删除单字段 `ImportedCoreLoweringAuthority` 和重复的 `CoreConcretizationAuthority` 投影。本地与外来声明引用的类型区别继续保留，不新增替代包装、wire 字段或 runtime ABI。

HIR 的 `org.scoop-lang.hir/core-bootstrap-interface/4` 直接保存完整 `CoreCompilerProtocolSurfaceV1`，删除重复的 `RuntimeCoreCapabilityV1::String` 与 `CompilerProtocolDefinitionsV1` 外层。section 的 field 2、3 继续保存 output contract 和 direct public surface，field 5 以长度为 0 或 1 的 array 保存本产物定义的协议角色；旧 field 1、4 及旧资格分支 tag 1、2 退役，不复用，旧 `/1`～`/3` 产物与缓存重建。String 的 source identity 只在已有 fundamental type 角色中保存，exact identity 由该真实声明的 `ExactTypeKey::Nominal` 得到；不再维护第二份 source/exact 记录或比较两份投影。前端完成 intrinsic 识别与语言声明检查，后续阶段直接消费完整 typed 角色、共有声明与实际 provider；reader 保留格式、引用种类、签名、effect、成员归属和跨表一致性检查。初始化循环服务的实际函数、String 参数、Unit 结果及 canonical ABI 继续沿共有 callable 路径使用。此修订不改变 String 表示或 runtime C 调用约定。

### 2.4 String、初始化与 Link

外来 singleton 的读取使用共有依赖中的实际 nominal、object-value、初始化 callable 和 published-root。类型和值查找按各自 typed ID 解析；ensure 进入普通 callable 选择，静态根通过实际 provider 的存储定义消费。删除“已授权 storage”之类外层资格，只检查真实类型、unit、ABI、根与 relocation 关系；不复制实例、cell、failure root 或 GC 登记。 实际读取位置使用共有 `cross-cone-interface/30` 的 SingletonValue 类型角色（新 tag 7），用于 Object 依赖及当前 initializer 的直接 unit 边，不建立独立操作凭证；旧 `/27` 产物重建。

String descriptor 使用完整 MIR 中实际声明的 source exact identity，沿共有 descriptor 查询、layout selection、physical import、registration 和 Link relocation 消费。删除独立 String bridge 与固定角色的 descriptor 恢复通道，不以 provider 坐标或协议来源豁免普通引用检查。Strong production `/7`、`/8` 退役原服务表中的 TD tag 2；旧产物与缓存重建，String 表示及 runtime C ABI 不变。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

初始化单元直接携带所需的本地或外来 callable。共有 MIR 依赖选择从这些单元收集实际 provider 与函数引用，driver 不再另走一次初始化服务选择；MIR 不保存 Local/ImportedUnused/Imported 的配对状态，也不逐单元再核对一份全局来源记录。MIR/LIR selected 表不复制初始化角色、不限制只有一个该角色，也不提供服务专用添加入口。声明发布需要的语言角色继续保留，实际 target、签名、GC effect、ABI 与依赖关系由共有记录及消费边界检查。该进程内简化不改变 wire、runtime C ABI 或初始化与失败缓存语义。

Strong production 的两种表示使用 `/11`、`/12`，删除初始化专用 ABI field 11 和外部服务表 field 12，section 只保留 field 2～9 的八字段 product。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 registration reader 完成结构与引用检查后直接返回完整 registration production 数据，不保留仅用于限制编码或导出资格的中间凭证包装。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

LIR descriptor 依赖是明确的 typed IR 引用。reader 验证可达 provider、exact type 的实际 descriptor 导出、递归依赖闭合及物理定义，不要求 descriptor 先出现在 HIR 的 value-layout 或 shape-support 物化根中，也不从协议表重新证明其来源。显式 descriptor relation 与从源码类型使用得到的 layout/shape 根共同闭合；遗漏必要依赖、错误 provider/类型与不一致物理合同仍拒绝。

共有 Link import 表可以保留没有实际 relocation 的完整类型或 callable 引用；这类声明不生成虚构的 machine use。Link 只对实际 relocation 生成 requirement，并验证目标、provider、symbol、ABI 和定义一致性。删除“每个已声明 import 必须至少出现一次”的附加证明，以及初始化服务的专用豁免；缺失实际引用、错误定义和损坏对象仍由共有 verifier 拒绝。

String、初始化服务及 descriptor、callable、selected、registration 接入共有记录。删除固定 CORE provider、专用 bridge、独立授权及 core requirement/owner/proof 通道。所有外来 strong 引用按实际 provider 和 typed target 查询 definition、ABI、symbol 与 relocation；GC 登记与初始化依赖仍完整。runtime/native/target 的不同调用契约保持，用途分区互斥且完整覆盖实际 undefined relocation。

M23-6 删除没有实际 producer/consumer 的 `ScoopRuntimeCoreBindingsV1`、`ScoopProgramDescriptorV1`、固定 String capability ID 及其 LLVM 布局镜像和专用测试，不把它们改名后保留。未使用的 program/core magic `0x53434f4f50505247`、`0x53434f4f50434f52` 退役且不复用。实际发射的 image、root entry、type/callable、storage、immortal、initialization 和 safepoint 记录继续使用既有格式；这项删除不改变实际 runtime C ABI、String 表示或其 fingerprint。M23-8/9 的多 image 启动与 program-link 仍留在后续阶段，按实际入口和引用需要定义数据，不提前冻结新的 program record 或另建 String 授权表。

String 由前端解析为实际 typed class，MIR/LIR 与 Link 使用同一 provider 的 exact type、TypeDescriptor、registration 和 relocation。现有 C ABI 的 `scoop_td_String` 表示 runtime 所需的 String descriptor 地址；链接使用实际声明的定义，不从固定 CORE identity 重建它，也不按名称或同布局替换类型。对象头为 16 bytes，length offset 为 16，inline bytes offset/minimum 为 24，alignment 为 8；`InlineBytes` 的 size/stride/alignment 为 1，object/inline scan 为空。保留这些真实表示和边界检查，不单列重复的 String 布局/scan 副本、capability-kind digest 或 program/core binding 证明。

### 2.5 通用资源预算与计费

删除 `BudgetMeter`、累计 usage、logical heap/work、节点/边/复制字节和逐操作收费的参数、接口、错误、版本字段及专用测试。不保留改名后的 meter、unlimited 或空壳。

runtime scan 比较删除任意展开次数、逻辑字节配额及超额 abort；成本常量不再进入 profile、fingerprint 或 runtime ABI。实际 offset/count/length 使用 checked 运算并核对真实存储范围，非法循环由局部图检查处理，共享 DAG 比较按节点或节点对复用结果。分配失败正常报告，不以成本估算决定合法程序能否编译或运行。

### 2.6 重复证明与完整验证

前端在名称查找、override coverage 和 signature exposure 的负责位置执行访问域检查；成功后直接保留 typed 声明引用、实际继承/override 关系及最终 lookup/slot 域。删除 `LookupAccessWitness`、`OverrideAccessWitness`、`PropertyOverrideAccessWitness`、`SignatureExposureWitness` 及仅携带这些记录的候选资格状态。后续候选物化、IR 与产物生成不复制访问域证明，不用 witness 的有无替代实际声明关系。可见性错误、protected 接收者规则、签名泄露检查和独立 setter 槽规则保持；不改变 wire、profile、runtime C ABI 或 String 表示。

共有 HIR 类型位置的结构、foreign nominal 分发、真实 provider 与定义/求值位置在 HIR reader 边界检查一次。后续物化查询和 MIR/LIR 消费同一未变化的 typed 记录，不重新完整检查这组 HIR 关系，不建立额外验证状态或凭证；实际类型表示、签名、ABI、对象与传递引用仍由相应边界检查。外部字节重新读入或相关数据发生变化时重新验证受影响部分。这一职责调整不改变 wire、内容 fingerprint 的字段组成或 runtime C ABI。

删除 `validate_iteration_plans` 及 concretizer 两个入口的整模块调用。该通道重新检查全部函数、构造器、默认值的迭代协议、类型、effect 和局部定义，没有实际 artifact reader 消费；其独立语义实现、伪造 HIR 测试和修改辅助函数一并退役。前端产生完整 `IterationCore`、`ForIterationPlan`、binding plan 与 typed loop target，后续直接消费；保留真实源码和正常产物的正确性回归。

共有源码接口保存所有必要声明的参数协议、typed 默认值正文和定义环境，protected、private 与默认值支持声明使用同一记录。type-semantics section 不再复制受限参数协议、默认值或来源并集；原 field 5、6、7 退役，现有 field 1～4、8 保持原编号。前端完成语言与可见性检查，reader 核对已有正文的编码、typed 引用与 owner/binder 关系，不再生成 ParamFree/GenericSourceMetadata 资格、逐正文访问证明或独立完整重放。源码完整但无运行时表示的类型可以参与默认参数声明；实际物化时按 typed 声明处理。此次格式变化纳入 cross-cone-type-semantics/8，旧产物需重建，runtime C ABI 不变。

源码位置由共有接口的 definition_sources、call_sites、type_sites 与已有全局定义记录汇集，不能为收集位置重建独立默认值正文、参数协议或访问证明。删除 NominalDefaultSourceProductionV1、DefaultSourceBodyProductionV1 及旧 DefaultSourceTemplate／References／Access 数据模型、适配器和专用测试；生产路径直接复用共有接口的完整默认值及位置收集。必要的来源位置、typed 引用、可见性、参数与 binder 规则仍保留。旧模型已不属于正常产物字段，此清理不增加格式分支或改变 runtime ABI；内容 fingerprint 依实际产物数据计算。

默认值引用是普通依赖索引：六类记录各保留真实 typed target 与 definition origin，正文与索引在读取边界核对一次。定义处的名称、类型、effect 与调用域覆盖规则由前端负责；继承默认值遇到类型代换或调用域扩大时检查实际变化，未变化的事实直接复用。producer 不重建访问域，reader 不再分别重放 type、value、callable 与 direct/slot 的访问证明；产物仍检查实际 provider、typed 引用、owner/binder 范围、局部值范围及跨表一致性。字段、构造器与全局值引用在共有 reader 边界对照已解析声明的实际 owner、种类和作用域；局部函数引用必须对应正文携带的声明。复用已验证的身份图与正文索引，不重新推导访问域。默认引用 record 采用两字段 map，field 1=target、field 2=definition_origin；旧 witness 的 field 3 退役且不复用。共有接口升级为 `hir/cross-cone-interface/30`，旧 `/24` 及更早产物与缓存重建，profile 与内容 fingerprint 同步更新。运行时 C ABI、String 表示及必要 GC 契约不变。

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

独立 `identity-foundation` artifact profile（旧 `/1`、`/2`）退役。删除只供旧测试使用的 `IdentityFoundationMetadata`、`IdentityFoundationArtifact` writer，以及 `DecodedIdentityFoundations`、`IdentityCheckedFoundations`、`StructurallyValidatedFoundations` 和 native-boundary/commit 外层组成的平行 reader。三层基础 identity payload、真实依赖身份解析、类型/ABI/GC 契约与完整 Strong 产物 reader 保留；测试直接使用共有容器或完整生产 reader，不保留只有 identity、没有实际编译输出的产物路线。

生产 profile descriptor 只编码必需 section 清单：field 1=id、2=required_manifest、3=required_hir、4=required_mir、5=required_lir。原 field 6～9 及独立 `ArtifactValidationPolicy` 退役，不复用；删除仅服务于旧 profile 或未来占位的 availability policy、publication class、Link proof policy 与 ODR policy 数据。完整生产产物的 Code/RuntimeImage fingerprint 必须 Available，由 manifest 读取规则检查；ODR 在本阶段的 Strong 输入边界拒绝，optional/unknown section 按实际 purpose 与 registry 规则处理。`single-cone-strong`、`cross-cone-semantics-strong`、`cross-cone-layout-strong` 的 major 均升为 3，旧 `/1`、`/2` 产物和缓存重建。profile fingerprint 继续覆盖这个实际格式描述，runtime C ABI 与 String 表示不变。

删除仅由测试实现/使用的 HIR/MIR/LIR 来源工厂、平行来源 reader、凭证状态机、重复数据表及适配层。旧 bootstrap stage 包装与 Core/NotCore 测试输入适配层同样删除；保留的真实源码测试直接调用共有 stage 入口。独立 dual-artifact certificate/reopen 框架没有生产调用者，删除后只保留实际使用的不可变产物快照。source-authority 收缩为名称解析、类型检查、默认参数实例化和跨 Cone 消费实际需要的声明/来源信息，不保留逐项授权和防伪证明。

语言规则由对应 stage 负责；IR/meta crate 负责数据及格式/引用不变量，reader 不再维护一套前端语义实现。已解析默认值保留完整 typed 正文、声明引用和定义环境。依赖默认值中的字段和 callable 使用定义时保存的 typed 声明与完整类型，实例化不重新要求公开 namespace 导入路径，也不再次证明模板引用集合。

默认值依赖按定义处已解析的 typed target 保存，`DefaultDependency` 不要求消费 Cone 再取得 namespace 导入路径。定义处实际发生过的查找路径可以保留；生产器不为没有名称查找的字段、成员或支持声明补造 witness，也不保存全体依赖的第二份导入路径表。Unit、Any 与其他声明遵循同一规则。共有 reader 继续检查 provider、引用、类型、默认值正文及实际声明关系，不从这份路径信息授予调用资格。读取边界核对源码上下文及位置后，实例化复用同一不可变记录，保留定义位置与调用处求值位置。不得用新通用工厂、插件、安全框架或证明系统代替删除项。

## 3. 编排、wire 与文档

producer、reader、linker、wire/profile、版本、fingerprint、fixture、golden 及规范同步修改。格式改变按正常版本演进，退役 tag 不复用；旧产物明确重建，不做长期双轨适配。内容 fingerprint 与缓存记录继续保留实际用途，不绑定计费策略或来源授权。

保持实际 runtime C 调用约定、String 表示与 persistent identity。后续 ODR、multi-image startup、artifact-only program-link 不属于此清理。历史阶段文档如描述已退役策略，只可作为带有明确迁移说明的历史记录，不能继续作为生产入口要求。

## 4. 实施顺序与验收

本地 class、struct 和 enum 实现参数自由的依赖接口时，前端直接消费共有接口声明的完整父接口、typed slot、签名、默认实现和访问域。HIR 的 conformance 引用实际接口类型及本地/外来槽声明，目标为本地方法 application 或共有依赖 callable；不得为复用本地检查而复制外来函数声明、正文或生成同名替身。MIR 的 dispatch 表保留本地函数或实际外部 callable 引用，默认实现与抽象槽 trap 沿定义方原有 target 解析，LIR 使用现有 canonical ABI、外部定义与 relocation 路径。override、缺失实现、默认方法冲突、setter 能力和签名/effect 规则在同一前端检查中完成；类型、成员和 dispatch 独立及组合场景须经真实源码产物消费和单 image 普通/移动 GC 运行验收。

本地 interface 可以继承参数自由的依赖接口。父边保留实际 TypeId，override 关系保留实际本地或外来槽声明；继承的成员按父接口声明顺序进入完整槽表，菱形继承按声明身份去重，被覆盖的槽按已解析 override 关系消除。显式成员及当前 this 的隐式成员查找沿本地与依赖声明的同一父图进行，本地和外来候选共同执行语言规定的适用性与最具体选择；不能以声明存储位置决定优先级，也不能将外来成员复制为本地声明。该接口再次发布后，下游按实际父类型、槽与 provider 消费，保持 canonical ABI、默认方法、属性和装箱语义。

抽象 dispatch 目标与具体实现一样保留实际声明：Export HIR 的抽象 conformance 携带真实方法 application 或外来 callable 引用，source selection 携带所选 abstract 声明，完整 slot contract 携带该声明的 owner、signature、effect、modality 与访问域。最近的 class 抽象声明以及更具体 interface 的抽象 override 均压制原默认实现；不把原槽声明伪装成所选目标。MIR、LIR 和 Compile/Link 按该 typed target 取得真实 trap、ABI 与 relocation，不再扫描所有 callable、重建整份继承图或沿继承链反向推测抽象目标。槽身份与所选声明身份可以不同，双方仍须满足实际继承、签名和访问合同。正常路径复用完整记录，不增加来源凭证或第二套证明表。

限定 `super<I>` 通过当前 owner 直接列出的真实接口类型查找成员，本地接口和依赖接口使用同一候选决议、参数默认值与可见性规则。调用选中的具体默认方法或 getter/setter 时，HIR 保存实际声明及强制 direct 调用方式；接收者沿共有引用转换或值装箱路径适配，MIR/LIR 按该声明的 canonical ABI 与 provider 定义发射，不再次进入接口表。抽象目标、非直接父接口、错误参数与初始化期间的调用仍在前端诊断。导出的默认参数正文用既有 `DirectSuperMethodCall` 节点保留该语义，下游展开继续调用同一真实目标。正文与引用集合复用同一 callable 引用转换，完整保留实际方法 owner，不分别重建不同的引用记录。同一 provider 的同一 typed callable 被 direct 和 dispatch 多次使用时，MIR 依赖记录与物理定义只选择一次，各调用点仍保留各自的派发方式。此项不新增来源资格、证明表、wire tag 或 runtime ABI；验收覆盖普通与 ZST 接收者、属性读写、命名及默认参数、大值结果和再次发布后的消费。具有隐式接收者的成员正文仍按普通值查找规则消费依赖属性、object 与 enum 变体；这些值与本地值使用同一个解析结果，不能归入函数或类型的非值阻断层。

外来 dispatch target 按实际 provider 的普通 callable 或布局 ABI 导出取得定义和 canonical ABI；已完成消费边界检查的物理引用直接复用，不再重复完整验证或要求普通 callable 在布局 ABI 表中再登记一次。装箱和 dispatch adapter 的目标签名在完整本地及依赖 callable 查询中关联；组成记录只检查自身签名与转换形状，不要求外来目标同时登记在本地 foundation，也不重复进行同一目标签名的完整验证。

先修订三份 spec、ROADMAP、M23 总设计、M23-6 设计与本清理文档，再按实际调用链清理实现。已完成项核对后保留，不重复实现；每批代码先格式化和 lint，随后测试，通过后按功能提交。

| 验收项 | 实际场景 |
|---|---|
| core 普通库 | 普通目录修改、扩展、重建，显式依赖优先，下游消费与缓存失效 |
| 类型与 ABI | 独立/混合 provider 的 layout、C-safe、完整签名、ZST、direct/indirect、GC/root plan |
| 成员与 dispatch | constructor、继承、interface/default、getter/setter、protected、真实 body/slot |
| String 与初始化 | 字面量、类型测试、属性/object 初始化、修改并重建 core 循环函数、失败缓存与引用复用、真实 descriptor/callable/registration、静态根参与移动 GC |
| 产物与 Link | 真实源码生成完整产物，consumer 只读产物；对象、符号、relocation、版本与内容检查 |
| runtime | 分配、boxing、array、GC 不重复展开已验证静态 scan；动态范围错误仍拒绝 |
| 清理结果 | 无生产调用依赖旧授权、通用计量、重复证明或测试专用工厂；不以改名或 unlimited 通过 |

语言错误 fixture 断言位置与信息，格式损坏和正常功能回归保留，删除仅服务旧机制的测试。手工 metadata、测试来源工厂和大量证明反例不能替代真实编译、跨 Cone 消费、适用链接运行。只有主设计第 15 节的原有功能、本文件七组清理及验收均完成、文档一致并按功能提交后，才能完成目标。
