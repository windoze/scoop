# Scoop Runtime 规范

2026-10-07，M31 实施中，见 [M31 设计](../milestone31/DESIGN.md)和[实施记录](../milestone31/PROGRESS.md)。M31 的 nursery／minor GC 见 3.9，写屏障见 3.6，ODR 与最终 image 选择见 2.8；历史版本记录不代表待完成项目已经实现。

2026-10-07，M30 已实现并通过验收：binary32/binary64 标量、线程浮点环境、字符串转换和必要的数学后备复用既有 GC、Scoop/C ABI 与链接边界；Float/Double 不提供 Hash 后备，128 位类型仅作调研。契约见 6.1 与 [M30 设计](../milestone30/DESIGN.md)，实际平台范围和测试结果见[验收记录](../milestone30/ACCEPTANCE.md)。

2026-10-05，M29 设计将 generic companion 改为每个完整宿主类型各有一个 singleton，类型和初始化状态按宿主 application 区分，见 2.2、2.7 及 [M29 设计](../milestone29/DESIGN.md)。此项已在 M29 首批实现并通过三平台正式 fixture；M21/M23 的历史设计不改写，旧的共享 companion 规则由本次修订取代，初始化状态机及 C ABI 沿用既有协议。

2026-10-06，M29 将编码改为 companion/普通 codec 的 `Encodable<T>.encode(value, encoder)`，与 `Decodable<T>` 一样保留真实 codec receiver。Unit 使用独立的 UnitEncoder / UnitDecoder，数据的固定类型身份、零大小值布局与返回 ABI 保持。协议、core、JSON 入口和双向自动派生已完成，三平台全部适用 fixture 已覆盖；实际测试方式与结果见 [M29 验收记录](../milestone29/ACCEPTANCE.md)。旧实例编码的验收不计作新协议验收；分层与格式迁移见实现规范 §2.17。

共有名义声明保存 `@NoGC` 值类型契约及在原形参域内推导的 GC-free 指针条件，该字段自 `hir/cross-cone-interface/43` 起启用。仅在签名、别名、父类型或嵌套 application 中使用依赖类型，也须满足同一契约；泛型替换继续传播尚未闭合的条件。旧 `/42` 及更早产物与缓存重建；完整字面量来源、默认值规则、runtime C ABI、对象布局和 GC 契约保持。详见实现规范 §2.2。

`for` 在 Export HIR 前展开为普通调用、接口适配、Option 操作和循环，共有 HIR 撤销专用 For 与 portable binding-plan 编码，statement tag 9 退役且不复用，该变更自 `hir/cross-cone-interface/40` 起启用。迭代协议、求值顺序、ABI 与 GC 规则保持，由实际类型与 callable 记录表达。

静态嵌套 import 与 re-export 保留原 provider 的 typed 实体及机器定义；support provider 的合法公开成员可通过已选 owner 消费，不重新发射其 Strong 定义。该规则自 `hir/cross-cone-interface/39` 起启用；本项不改变 runtime C ABI、对象布局、初始化或 GC 契约。

公开 typealias 的目标通过实际类型签名或直接 typed alias 边记录，外部 `AliasTarget` 只承担目标引用和实体归属检查，不再要求或保存别名专用的名称来源证明。源码的普通名称查找、可见性、类型实参和循环检查保持；已解析目标可来自可见类型的静态嵌套命名空间。该别名规则自 `hir/cross-cone-interface/38` 起启用；该批次共有 HIR 格式为 `/42`，旧 `/41` 及更早产物与缓存重建，不改变 runtime C ABI、对象布局或 GC 契约。

旧 identity-only 产物 profile、平行来源 reader 和发布/Link 凭证策略退役；三个完整生产 profile 升为 `/3`，descriptor 只保留实际必需 section 清单，旧产物与缓存重建。此调整不改变 runtime C ABI、String 表示、初始化或 GC 语义；类型、对象范围和实际引用检查仍在对应消费边界完成。具体格式见实现规范 2.6 与 M23-2 设计 8.3。

静态存储与初始化失败根按其实际值类型引用 layout/scan。当前 Cone 只发射自身拥有的布局与扫描定义；外来类型的静态根复用共有依赖查询取得的完整 value-layout 和 scan 记录，保留实际 provider、typed identity、定义与 relocation，不因本地持有该类型的值而重发射 foreign Strong。layout/scan 指纹节点引用已经解析的实际记录，不要求该类型在当前 Cone 定义；指纹补丁目标仍须属于当前产物。MIR 必须携带生成失败根所需的实际 Any 声明，LIR 不再缺省重建固定 core 身份。static-storage 语义记录新增 field 32 保存 layout provider，完整记录使用 fields 1～32；语义投影使用 fields 1～10 与 32。共有 strong-production 两种格式当前为 /13、/14；在静态根的 /11、/12 之后增加实际 callable 正文的 canonical LIR 摘要（实现规范 §2.5），旧产物和缓存重建。runtime C ABI、String 表示、初始化状态与失败缓存语义不变，不引入 ODR 或多 image 启动。

普通 catch 的绑定必须可以像其他引用值一样离开 handler：匹配 native payload 后，在绑定变量前物化一次 managed 异常对象，后续返回、存储和捕获使用该对象；native unwind record 仍按既有 cleanup 规则释放。初始化 catch 复用这次物化，不再次复制。每次 throw 仍创建独立 native payload，runtime C ABI 不变。

跨 Cone 值类型装箱使用实际 provider 发布的 BoxedValue TypeDescriptor、接口表和 adjust thunk。值的接口引用、父接口及调用签名沿完整 typed IR 传递，消费者不复制外来实现。ZST 装箱仍产生独立对象身份，非零与含引用 payload 保留原有布局、canonical ABI、scan 和移动 GC 规则；这项消费能力不增加 runtime C ABI、String 表示或元数据验证策略。

整数和 Boolean 沿相同装箱路径使用实际声明的 TD 与接口表；primitive 的机器表示不承担声明身份或来源资格。runtime 不接收前端的来源文件编号，也不据此决定调用或分派是否合法。

编译器只为实际装箱和分派选取相应的外部 TD 及接口定义；未使用的 primitive 声明不会扩大 runtime 登记或机器依赖集合。

运行时类型转换的失败构造使用前端解析的实际异常类型与 constructor 引用，并沿共有的类型、callable、ABI 和 Link 路径消费。删除由 Cast 反向投影的独立 CastFailure call-site、RuntimeOperationDependency role，以及 reader 对同一目标再按 compiler protocol 进行资格判断的通道；普通源码调用的位置、参数、结果与 typed 引用检查保留。共有 HIR 格式更新为 `hir/cross-cone-interface/30`，原 call-site reason tag 2 与 external-reference role tag 9 退役，不复用；旧产物、profile fingerprint 与缓存重建。该调整不改变转换失败抛出 ClassCastException 的语言行为、runtime C ABI 或 String 表示。

引用上行转换在 HIR 中用显式 `ReferenceUpcast` 节点保存内部表达式及目标类型，不能直接改写构造、调用或局部读取的原始类型。MIR 使用已有 `Retype`，不分配对象、不改变引用身份；构造器仍按实际所属 class 分配。默认值正文使用新 expression tag 58 保存同一操作，tag 44 继续退役；共有 HIR 格式更新为 `hir/cross-cone-interface/30`，旧产物与缓存重建，不改变 runtime C ABI。

跨 Cone 动态调用沿现有 vtable/itable 及对象 TypeDescriptor 执行；接口查找键使用真实 exact interface 的 TD，槽序号仅在其所属表内有效。外来 class/interface 与本地类型共用调用约定、GC roots 和异常边界，不因 provider 为 core 而取得额外资格，也不改变 String 表示或 C runtime ABI。 成员 setter 同样使用实际 accessor ABI 与 dispatch；含引用属性写入由 provider 的普通 setter 执行已有写屏障，ZST 和大值参数沿 canonical ABI 传递。

本地接口继承依赖接口时，父接口与各槽继续引用真实 exact type 和声明身份。接口表中的本地与外来默认实现使用同一 canonical ABI，值类型装箱按同一 receiver 转换规则调用；接口继承不新增 runtime 表、来源资格或验证重放。

抽象 slot 的物理 entry 使用实际所选 abstract 声明的既有 pure-virtual trap 及完整 ABI；slot 的根声明与 trap 声明可以不同，receiver 沿真实 base/interface 关系适配，调用错误仍有明确 fatal 出口。该关系由编译器完整产物提供，runtime 不反向重建声明或新增来源、配额和重复元数据检查。HIR 接口及 type-semantics 格式按实现规范升级并要求旧产物重建，runtime C ABI、String 表示和 GC 契约保持不变。

跨 Cone 的限定 `super<I>` 调用沿实际默认实现的普通 direct ABI 执行，接收者转换与值装箱、ZST 消除、大值返回和 GC effect 使用共有规则。运行时不再执行一次接口槽选择，也不增加特殊入口或验证；源码默认参数中的同类调用经产物消费后保持同一目标。现有 C 调用约定、String 表示与元数据格式不变。

删除仅由测试实现的默认值operation-typing、nested ABI及root/origin语义工厂和其证明数据、平行验证入口与专用测试。正式reader继续使用共有声明表、完整typed模板、类型与binder检查、来源位置、局部数据流及真实引用一致性检查。局部数据流直接借用模板与共有字段查询，删除重复body input及authority适配器；nested descriptor保留实际类型化身份、parent/path与binder数据，删除独立Standalone证明模式。语言操作规则由前端负责，不在IR/meta crate再复制实现。此清理不改变wire字段、profile版本、runtime C ABI或String表示。

参数自由依赖class通过共有名义声明取得真实身份、modality、直接父类型和声明序字段，引用类型按GC契约传播，递归引用字段不展开成递归值布局。HIR保留完整声明及解析后的字段、父类型，不重建同名本地声明。外来class构造仍是对真实constructor的typed调用；HIR→MIR消费已选MIR绑定中的ClassInitializer角色，使用共有class分配路径创建对象，再将该对象作为receiver调用实际provider的初始化实现。构造表达式返回分配的对象，物理initializer返回Unit；普通返回class的函数不走构造分支。本地与外来initializer共用Callee表示、参数求值顺序和GC处理。 core默认导入层的构造调用和静态限定名同时查询普通Type/Value binding；用户新增的class、enum和typealias与其他依赖使用相同声明路径，不限于预设内建名字。此能力不增加来源凭证、core分支或runtime ABI，沿用现有类型、MIR绑定及LIR布局格式。 M23-6的真实class运行在单个最终镜像中链接实际产物及runtime；LLVM stackmap段按各对象贡献的完整v3 blob逐个读取，长度由已有count和对齐决定，保留每份格式、记录唯一性与精确PC检查，不增加展开预算。该读取不要求M23-8的多镜像启动或M23-9的program-link。

默认值引用的访问证明字段退役属于 HIR 产物格式变更：`hir/cross-cone-interface/30` 要求旧 `/24` 及更早产物和缓存重建。runtime 不读取这类前端访问证明；删除它不改变 C 调用约定、String 表示、类型布局或 GC 扫描契约。

共有导出绑定包含 enum 变体的真实 typed ID，nominal 的 `nested_bindings` 同时列出其静态命名空间中的嵌套类型、object value 与 enum 变体；变体归属由实际声明确定，不能误作包级值。`hir/cross-cone-interface/30` 更新该格式语义，旧 `/23` 及更早产物与缓存重建；既有 tag 不复用，不保留双轨 reader，runtime C ABI 和 String 表示保持。

跨 Cone enum 构造与默认值展开只改变前端取得完整构造信息的路径。runtime 继续使用现有 tag/niche、payload、ZST 与 scan 表示；不新增外部构造服务、来源检查或 C ABI 分支。

enum 模式和变体测试继续消费实际 tag/niche、payload 布局与 GC scan 元数据；删除默认值正文外的重复构造器访问记录只影响 HIR 产物引用集合及其版本，不改变 runtime C ABI、String 表示或对象范围检查。

ABI 元数据不再为每个逻辑参数保存独立 layout ID；参数与结果的完整 canonical 类型、传递方式和 GC effect 保留，结构字段的 scan 组成继续进入实际外层布局。此格式清理不改变 runtime C 调用约定、String 表示、对象范围检查、GC roots 或登记语义。

跨 Cone struct 主构造器保留源码 Managed 合同与实际 NoGC 值构造入口的区别；次构造器使用其实际 lowering 签名及 GC effect。调用方按完整 MIR lowering 记录和 LIR canonical ABI 传递参数、结果及 roots，ZST 的机器消除不改变源码求值。该接入不改变现有 runtime C 调用约定、String 表示或 GC 契约。

跨 Cone tuple 等结构类型继续使用既有值布局和 canonical ABI；其 nominal 叶、逻辑参数、ZST 及 GC roots 不因签名组合而丢失。结构签名解析不改变 runtime C ABI、String 表示或扫描规则。

MIR 输出在 HIR→MIR 边界完成一次整模块结构、类型与实际外来 callable 检查，并同时保留已生成的 canonical foundation、共有依赖选择和完整物化引用。通用 MIR 输出保留完整泛型实体；Strong profile 的 ODR 能力门仍在其消费入口检查，并共享已有 canonical foundation。driver、MIR production 组装和 MIR→LIR 直接消费同一完整输出，不再从未变化的 Module 重建第二份 foundation、重复验证外来调用或重跑整模块检查。production 与模块之间仍核对实际 callable、入口和初始化关系；直接借用已有签名记录，不构造第二份预期桥表。外部新产物的格式、引用、ABI 与对象检查继续由 reader 负责。此清理不增加凭证、状态机、wire 字段或 profile 版本，不改变 runtime C ABI、String 表示或后续里程碑范围。

跨 Cone 的参数自由 class 继承直接使用依赖产物中的完整声明、真实基类、字段、接口与 dispatch selection。共有成员查询保存声明本身及其可见性，public、protected 和必要支持声明不改造成另一份 public 声明；前端按实际词法类和接收者静态类型执行访问、覆写及默认值规则。基类构造在已分配的派生对象上调用实际 provider 的 initializer，保持基类先于派生类的初始化顺序及移动 GC 接收者跟踪。继承的 virtual family 保留原 typed slot identity，派发表中的外来实现直接引用实际 callable；本地覆写只替换对应槽，未覆写的基类和接口选择完整传递。构造、super、成员与 getter/setter 沿共有调用、布局、ABI 和 relocation 路径消费，派生类再次发布后仍可由后续 Cone 使用。以上使用既有源码与机器声明格式，不增加来源资格、独立证明、平行来源表或后续里程碑能力。依赖虚槽的 lookup 域由 HIR lowering 按共有声明的 visibility 与 typed owner 计算，保留 protected 与外层 owner 约束；产物不重复保存该域，reader 不重放访问语义。只有本次实际物化的 descriptor 和派发表产生物理外部引用；依赖类的查询数据本身不形成 relocation。受保护的嵌套类型通过实际 owner 的 child 声明引用参与限定名查询，不能要求 public binding 或将其重建为本地声明。前端分别检查全部词法 owner 的有效访问域和 protected 成员自身的接收者规则；嵌套类型中的 public 成员不会额外要求接收者属于访问者的子类。嵌套构造、成员、enum 变体与 object 继续使用共有 typed 选择及初始化路径，访问错误与签名泄露在前端诊断。 已发布的可物化 nominal 声明必须同时具有完整 dispatch 与有限 BoxedValue/CoroutineStep/CoroutineSlot 支持，包括受保护嵌套类型和实际表示所需的支持声明；producer 与 reader 从同一共有声明闭包取得这些需求，public binding 不控制机器支持的生成。source-only 声明及仅存在于当前私有实现、未进入发布声明闭包的类型不因此成为发布根。 reader 的声明查询不再区分“仅当前 provider 支持查询”模式；本地与依赖的签名、父类型、accessor 和 variant 都按实际 typed ID 查询完整声明，保留 kind、arity、owner 与依赖范围检查。

继承的实际产物验收同时覆盖 ZST 字段与交错参数/返回值、大值构造与虚调用的 indirect/sret ABI、含 managed 引用的聚合参数和基类/派生类字段、次构造器的基类先行初始化及 object 单例继承。派生类再次发布后，由后续 Cone 继续派生并执行 super 与虚调用；普通和移动 GC 运行使用同一完整产物。上述场景不增加新的机器表示或 runtime ABI。跨 Cone 初始化中的 inherited backing field 由共有 property、实际字段身份与声明关系解析，前端检查 getter/setter 可见性及字段就绪；不能用同名字段或调用 accessor 代替直接存储访问。完整 HIR 的初始化字段引用区分本地 class application 与实际依赖字段，concretize 后均成为共有的 ConstructorReceiver 字段读写。基类完成后外来存储已就绪，computed/delegated property、不可见 setter、未就绪自有字段与 receiver 逃逸继续拒绝。现有产物已携带所需 property/field 引用，此项不增加 wire 字段或来源表。

setter 的有效访问域包含关系由前端在声明处检查。共有 reader 保留 property/accessor 的 typed 身份、owner、签名、effect、声明位置和 public binding 一致性检查，删除第二套访问域集合运算、逐属性继承遍历及其专用测试。继承环和引用闭合仍在共有继承图边界检查；已检查的完整声明不附加来源或操作资格，也不因 selected 去重而反复重新证明源码访问。此清理不改变 wire、profile、runtime C ABI 或 String 表示。

primitive 与 String 的接口默认方法使用前端识别的实际声明及已解析的接口实现参与普通成员查找。成员查找自身解析依赖中的父接口，不依赖此前是否发生过类型转换。重建 core 时的源码调用和下游的产物消费遵守同一候选选择、可见性、参数及 effect 规则；值接收者沿既有装箱路径适配，String 使用其既有引用表示。不得因这些类型使用内建表示而遗漏合法接口默认方法，也不为它们重建固定 provider 或增加后端资格。验收覆盖 Long、Boolean、String 的独立源码调用及跨 Cone 混合调用，保持现有 wire、runtime C ABI 与 String 表示。

跨 Cone 的覆写按真实声明关系继承默认参数。默认来源保留原声明的 typed root、定义路径、正文和引用；下游再次发布时只关联新的参数位置，不把原声明改造成本地函数或重新解析其源码。未求值的继承模板也保留完整源码位置记录；位置收集复用同一模板遍历，不触发机器物化。来自同一定义的菱形继承只保留一个默认来源，互不相关的默认来源在覆写定义处报冲突；参数名仍由调用点静态声明决定。实际省略参数时复用共有的依赖默认值实例化，并把接收者显式转换或装箱为模板所需类型，默认值内部的动态分派、显式参数求值顺序及 GC effect 保持。参数自由的导入模板已经在提供方完成类型和 effect 检查，未改变的正文不参与本地泛型约束重放。该功能沿用已有完整默认值产物格式和共享 reader，不新增来源凭证、模板工厂、wire tag 或 runtime ABI；验收包括覆写、菱形继承、命名参数、ZST、大值参数与再次发布后的真实消费。

产物 reader 的 MIR 类型和 callable 校验使用同一次 HIR 类型基础与继承图构建。按真实依赖顺序解析完整 MIR 类型、callable 和 dispatch，并在每个 provider 的实际依赖范围内检查类型基础与父类型引用；随后为整个不可变依赖闭包构建一次继承图及槽关系，各 provider 共用这张图核对类型表示、有限 shape、方法、构造器、object、派发、equality 与初始化契约。不得因处理另一个 provider、类型或 callable 而完整重建已检查的上游继承图；继承环、实际声明、槽及依赖引用检查继续保留。后续 LIR 消费完整的 MIR 结果，保留各自需要的格式、引用、签名、ABI 与对象检查。此调整不改变 wire、profile、runtime C ABI 或 String 表示，也不增加来源工厂、资格状态或缓存凭证。

完整 LIR 输出已保存 canonical foundation，codegen 不从同一未变化的模块再次构造该 foundation。LIR 在既有 Strong 能力边界检查 ODR 后，只追加当前 Cone 的 Strong 定义，不再次扫描未变化的 callable/ODR 记录。production 的符号表直接使用本次生成的定义表，对象分区直接使用 production 的符号记录；这些数据在 production 组合边界检查后，codegen 按 typed definition、atom 和 symbol 解析每项实际发射引用，复用完整记录，不重建整份预期表或再次完整比较。C bridge 与 C layout 入口只检查实际 C ABI、callback 声明及所需类型关系；callback 指令的 typed 引用与操作结果、Scoop CFG、dispatch、safepoint 与 root plan 在对象代码生成边界检查；不在每个无关入口重复完整验证。外部产物读取的格式与引用检查保持，wire、fingerprint 字段和 runtime ABI 不变。

依赖 companion 与 static nested 访问复用共有 reader 的静态命名空间和原 typed binding，前端在值遮蔽之后沿实际 owner 逐段解析限定类型与表达式。转发调用、属性和默认参数使用实际 object receiver，并沿普通 singleton ensure/root 路径消费；限定 host 和 const 访问不额外初始化。限定或直接导入的 companion 属性赋值、复合赋值与自增复用共有 getter/setter 路径，保存一次实际接收者，在右值之前完成接收者求值与初始化。验收覆盖命名 companion、Companion 别名、转发、跨 facade、类型别名及可见性错误，由真实源码产生四 Cone 产物后链接运行。此项补齐真实声明关系及其消费路径，不引入来源凭证。

`hir/cross-cone-interface/30` 补齐 object 的声明种类：source-shape 的旧 Object tag 8 退役，新 tag 9 保留 field 1=value、field 2=声明序字段，新增 field 3=`Standalone(1)` 或 `Companion(2)`；host 沿已有声明 key 的 typed owner 查询。命名 companion 发布名称与 `Companion` 两个普通 type/value binding，object 的公开方法和属性进入自身静态 binding 表。共有命名空间在本 owner 无同名 binding 时，沿已声明的 companion 关系转发其直接 binding；不复制成员声明、不用名称或初始化 metadata 推断 companion。旧 `/28` 产物与缓存重建，退役 tag 不复用，runtime ABI 不变。

产物的 identity graph 从 manifest 的当前 producer、实际直接依赖和已经读取的依赖实体构成；不无条件注册 CORE 身份，也不为 CORE 设置单独的重复过滤规则。CORE 与普通 provider 的声明使用相同的 typed 引用解析，缺失依赖、身份冲突及非法引用由共有格式与引用检查报告。默认 core 依赖仍由正常构建与前端依赖发现加入 manifest；本项不改变 wire 或 runtime ABI。

代码生成在入口对本次完整且不可变的 LIR、目标 profile 和 executable 入口完成一次必要验证，然后按实际定义分成函数与非函数对象。各成员直接消费同一 LIR，不因发射另一个对象再次完整遍历类型、ABI、CFG、GC roots、safepoint 或身份表；LLVM 变换后的 IR 和新生成的对象字节仍在各自边界检查。generated-C 源码入口同样不在内部 helper 重复整模块验证。这一职责调整不改变产物格式、runtime C ABI、String 表示或链接语义。

跨 Cone 的 struct 值与成员调用使用实际 provider 的布局和 canonical ABI。ZST 保留逻辑 receiver、参数和结果，物理调用可以省略其存储；普通值保留规定的 direct/indirect/sret 传递，含引用的值保留完整 scan 和 safepoint root 信息。dispatch 使用实际 callable body 引用，不因函数位于某张 metadata 表而获得或失去调用资格。此项不改变 runtime C 调用约定、String 表示及初始化、登记、GC 语义；产物格式见 [实现规范](SCOOP-IMPL-SPEC.md)。

runtime 消费完整类型布局、scan、ABI 与对象引用，不依赖编译器中的独立 source transcript 或绑定凭证。默认值依赖改用完整 typed 声明后，HIR 接口版本及缓存 fingerprint 按实现规范更新，不改变 runtime C ABI 或加入运行期来源检查。删除仅供测试的来源重建和证明框架不改变 runtime C ABI、String 表示或 GC 契约，也不增加新的 runtime 验证入口。 编译器以共有接口收集默认值源码位置后，独立的旧默认值来源模型不再保留，runtime 继续消费同一类型和调用表示。

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。 依赖查询直接使用真实 `ConeIdentity` 与 callable、property、type-alias 的类型化声明 ID；选择集合按这些 ID 保存完整接口和实际依赖路径。删除独立 world/projection/selection 品牌、仅为品牌服务的计数器和错误，以及从声明 ID 再映射到局部 u32 的三套重复表。候选选择依照当前依赖目录中的实际声明与表示，不要求由同一查询实例铸造；直接依赖的名称可见性、转导出路径、实际 provider 和引用完整性继续按共有规则检查。HIR 依赖目录、候选和已选声明只保存实际 provider 与类型化声明引用，不逐项复制 artifact 坐标/fingerprint 凭证；provider 身份直接来自已导入的共有 foundation，不再提供独立 certificate 或重算坐标身份。直接依赖与传递依赖使用同一输入数据和查询实现，直接依赖集合只决定当前源码可见的 package/public binding；不以分离的输入、视图或 seed 包装授予枚举资格。转导出保留实际 binding route，删除逐候选的 provider 凭证、重复 terminal 声明及其包装；候选选择使用已解析的声明目录，不重放未变化的 route 与 provider 证明。HIR→MIR 使用同次编译的依赖快照及完整声明：共有依赖选择在关联实际 MIR 定义时核对一次目标、逻辑签名和 GC effect，lowering 随后按实际 provider 与 typed target 消费同一记录，不重复比较未变化的声明和签名，也不再次比较来源凭证。缺失定义及最终 MIR 输出的结构、类型与外部引用检查仍在各自边界保留。普通构建依赖记录与缓存 fingerprint 继续承担定位和失效职责。MIR 外部 callable 引用保存实际 provider 与类型化声明，不另映射到带会话品牌的局部编号；调用位置保留 GC effect。MIR 类型与 LIR layout/ABI 选择按实际 provider 和 typed target 直接返回完整记录，不先铸造并验证中间 handle；依赖闭包、类型、ABI、物理引用和 GC 契约仍在其消费边界检查。同一声明或 target 在另一选择集合中是否存在，按实际目录查询决定，不依据集合生成顺序或计数器。该进程内数据简化不改变 wire/profile、实体身份或 runtime ABI。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

Strong production 的两种表示当前使用 `/13`、`/14`：删除初始化专用 ABI field 11 和外部服务表 field 12，section 保留 field 2～9，并由新增 field 13 保存实际 callable 正文的 canonical LIR 摘要，共九字段。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

对象摘要中的 relocation 按已解析目标的实际实体身份与 definition role 编码。Strong callable、layout、scan、类型描述符、派发表、类型登记、静态存储及其登记、初始化 cell 与 descriptor 统一使用既有 runtime target tag 1；ODR callable 和 shape 使用既有 tag 14 及原 member。同一已解析目标在定义 Cone 和消费 Cone 必须得到相同字节，泛型正文对普通 object 静态根的引用不能因再次发布而改变对象摘要。实体、role 和符号的对应关系复用共有 target 查询，provider 与声明目标仍保留在普通依赖记录中，承担既有引用、ABI 和符号检查。原普通依赖 callable tag 11 与 shape tag 12 均退役且不复用，原服务 tag 2、13 保持退役。完整 shape 引用正规化将 link-identity-closure 从 /6 升至 /7，旧产物和缓存重建；runtime C ABI、persistent identity 与 ODR 合并规则不变。

M23～M30 历史实现（M31 的内容 ODR 判等由 2.8 取代）：ODR callable-body 的 ABI/LIR 摘要由最终 LIR 产生，产物读取复用 production 中的值，并与实际 body ObjectDefinition 和所属 normalized stackmap 组合为该 member 的定义摘要。此项只补齐逐 member 链接比较需要的内容；callable registration 的 body-definition 字段继续保存 ObjectDefinition，runtime record layout、GC 和 C ABI 不变。Strong production field 13 记录保持原编码，ODR 的必需 ABI 字段随完整泛型 profile 发布，具体编码见实现规范 2.5。

core与其他library Cone使用相同image、registration与ABI检查。runtime和linker只关心实际typed表示及调用契约，不检查core源码来自哪个目录，不消费core专用授权token或缓存receipt；用户重建core后按普通依赖fingerprint更新产物。

每个 Cone image 的依赖表与该 artifact 的完整直接依赖集合一致，按 ConeIdentity 严格排序，拒绝自身、重复、缺失或额外项；core image 同样使用实际集合。编译器、artifact reader 和 Link object verifier 在登记与 fingerprint 前闭合这项关系，不能将普通依赖移出 image 表。此规则复用已有 descriptor 字段和 RuntimeImage 编码，不改变 C ABI，也不在 M23-6 执行多 image startup。

String TypeDescriptor、初始化循环异常服务及其 registration 由编译器和 linker 的共有类型/callable/definition 闭包提供，well-known 角色关联实际 typed 目标与 provider，不依赖 Core/NotCore 或 CoreExternal 资格。移除 core 专用发布与 Link 闭包不得省略布局、canonical ABI、GC/root plan、symbol/definition 归属及完整 relocation 验证；core image 与普通 image 遵守同一规则。runtime 的实际表示、调用与登记 C ABI 不因本次清理改变。编译阶段的迁移与验收见实现规范 2.12 和 M23-6 的 core 专用资格清理设计。

String 的外部 TypeDescriptor 与 type-registration relocation 使用普通 provider/exact 引用。删除编译器的独立 String bridge 不改变对象头、字符串数据布局、instance-shape tag 或 C 调用约定。 HIR `/4` 同时删除重复 String source/exact capability 和 definitions 外层，runtime 继续消费共有 MIR/LIR 中完整的类型、布局、ABI 与 registration；前端协议角色不成为运行期来源凭证。

归档调度与缓存属于构建管理职责，不能在父进程中再次执行完整对象与 runtime metadata 验证。编译器消费和实际 Link 对象边界继续检查类型、ABI、对象范围、scan、registration 与 relocation；runtime 按这些完整数据执行动态对象范围和 GC 契约检查，构建摘要不作为 runtime 资格凭证。

版本：0.6（草案）

配套文档：`SCOOP-SPEC.md`（语言规范）。本文引用其章节号。

## 1. 概述与范围

source `for` 与解构计划在 HIR lowering 完成语义检查并在 Export HIR 前展开；本地与外来迭代器复用普通调用、装箱／引用转换、Option 操作和循环。泛型值迭代器使用实际物化后的 BoxedValue 布局，引用迭代器保持已有引用表示；runtime 只消费已有调用、异常、对象及 GC 数据，C ABI、String 表示、扫描记录和 GC 契约不变。

Runtime 是编译产物的支撑层，职责包括：

- 对象模型与内存布局（对象头、TypeDescriptor、装箱）；
- GC 与编译器生成代码之间的**契约**（safepoint、根集合、handle 表）；
- Scoop ABI FFI 的 runtime functions（第 4 章，本文重点）；
- 异常抛出与展开；
- GC-free release hook与unmanaged resource兜底释放；
- 核心类型的表示级后备（String 编码/拼接、Array 分配/复制等）；List、ArrayList 与 StringBuilder 的容器算法由普通 core 源码实现；
- 进程启动、线程注册与终止。

明确**不在**本文范围：

- 第 3 章 M31 nursery + Immix 之外的替代 GC 算法，以及 parallel/concurrent collector；
- 协程调度器、集合、IO 等标准库内容；
- 普通 outbound C ABI callee 的内部实现；spec 14.3 的 managed callback 是经 runtime gateway 重新进入 Scoop 的独立反向边界；
- 编译器 intrinsic（`@Intrinsic`，spec 13.1）由编译器生成实现，不经 runtime。

---

## 2. 对象模型与内存布局

### 2.1 引用对象

每个引用类型对象带有对象头 `ScoopObjectHeader`，其中包含指向 `TypeDescriptor` 的指针（类似 vpointer，spec 10.4）。除本文明确冻结的pin/release-ready位及16-byte ABI外，对象头中其余GC状态的分配由collector实现决定。

M23-6的class instance以完整base instance作为prefix：继承字段offset不变，derived字段在base的完整size（含tail padding）之后追加并完成自身alignment/tail padding；同Cone与跨Cone使用同一规则，不复用base tail padding。abstract class也保留完整FixedObject layout供derived继承和initializer使用，但编译器的class allocation target必须具有实际ConcreteClass modality；不能把abstract class的base布局与interface/纯reference identity的AbstractRef混为一类。runtime仍只按最派生TD的完整allocation/scan处理实际对象，不分配或扫描独立base子对象。

对象分配与语言构造是两个层次。M19 class construction只分配一次exact concrete对象；header从分配完成起始终指向最派生TypeDescriptor，base/`this` constructor initializer在同一对象上执行，不分配base子对象或替换header。runtime不运行constructor、不在对象header/payload保存逐field“正在初始化”状态，也不根据TypeDescriptor补字段默认值；初始化顺序与访问限制完全由编译器静态保证。唯一相关的生命周期位是M24为hook-bearing class定义的`RELEASE_READY`：它只表示整个对象已经正常完成最外层构造、可在逻辑死亡时运行release hook，不表示任一field的就绪状态。M21的object/global exactly-once cell是独立的runtime side metadata，只管理完整initializer的并发执行与发布，不改变任一property的源码type或field表示（见2.7）。

分配入口在返回managed pointer前必须清零完整payload并原子登记header、object-start与精确size；`RELEASE_READY`初值也必须为0。constructor中途发生safepoint时，collector按完整class扫描描述处理该对象：已写field包含合法值，未写managed leaf保持全0并不形成引用。最外层constructor正常返回后、构造表达式把receiver发布为普通值前，生成代码以保留其他GC位的`memory_order_release`无safepoint RMW设置ready；构造失败对象的ready保持0，只按普通可达性回收，不调用release hook、rollback或其他用户代码。该操作与源码不可发布的initializing receiver共同保证hook只观察完整对象。

### 2.2 TypeDescriptor

M29的静态类型描述见语言规范9.6，它属于编译器/`.slib`类型接口，不是本节的运行期TypeDescriptor。字段名、字段类型列表和annotation不发射为反射枚举表，wire名称只是普通String常量。`Encodable<T>`与`Decodable<T>`均使用普通interface ABI和真实companion/codec receiver；encode另接收普通T参数，decode返回T，两个方向均有普通itable slot。分派由codec对象决定，不按数据的运行时TypeDescriptor寻找companion或编码实现，不增加类型级工厂、metatype或构造器注册表。

companion按完整宿主application分别使用既有exactly-once gate；`Box<Int>.Companion`与`Box<String>.Companion`具有不同exact type、TypeDescriptor和对象，各自扫描替换类型参数后的成员。普通codec、元素依赖及EncodeFunction/DecodeFunction闭包按正常对象/函数值表示保活。编码的数据实参、codec receiver、解码临时值、容器和结果沿原value layout、根、write barrier与异常路径处理；值类型codec适配到接口时使用普通装箱与receiver调整，目标数据不因编码获得接口。class解码目标必须执行正常构造及release-ready发布，runtime不按TypeDescriptor填字段或补default。JSON算法由普通Scoop库实现，本里程碑不改变对象头、扫描记录、runtime metadata ABI或C入口。

M23-6a 的共同 HIR 按原声明与完整 application 产生类型事实，MIR/LIR 保留对应 exact identity、实际 provider、Strong/ODR 归属及布局。源码与依赖产物的类型表示统一不改变对象布局、TypeDescriptor、GC 或初始化契约；runtime 消费实际生成的记录，不因声明来自另一 Cone 重新判定语言类型或复制定义方状态。新实例的必要 facts 由编译阶段完成，运行时继续检查动态对象范围、状态和 GC 要求。

每个**runtime-materialized concrete exact type**有且只有一份编译器生成的`TypeDescriptor`。materialized精确定义为该exact type进入某Cone的LIR layout/type closure或param-free exported LIR bridge；只存在于尚未替换的Export HIR template/binder中的type不提前产生descriptor。它包括每个单态化exact nominal实例，也包括进入LIR的tuple、managed function、raw/native pointer等结构exact type；后者即使layout相同也按各自`PersistentExactTypeId`区分。descriptor至少包含：

- 类型标识（`is` / `as` 检查用）；
- 稳定的 UTF-8 类型名（与 TypeDescriptor 同生命周期，用于未捕获异常等运行时诊断）。producer、artifact reader与final-link verifier必须从已验证exact-type key按M23设计3.1重算`CanonicalExactTypeDiagnosticName`，并以 checked 长度计算避免表示溢出；transparent alias、import/re-export alias、使用点qualifier、mangle或consumer本地display spelling均不得改变bytes。最终runtime metadata不携带可反演的exact-type key，因此runtime本身只验证该span非空、只读、长度落在已登记的只读范围，把 bytes 作为已验证的诊断内容消费，不在 runtime 重建或解析 canonical exact-type key；不能从 `PersistentExactTypeId` digest 反推名称或按名称判断类型；
- 版本化`ScoopTypeInstanceShapeV1`：只描述带对象头的managed instance，并以封闭`FixedObject | BoxedValue | InlineBytes | InlineArray | AbstractRef`区分固定对象、装箱值、String、array与不可分配的reference identity；未装箱值的`ValueStorageLayout`是独立LIR契约；
- **递归引用扫描描述**：`RefScan`普通节点记录相对调用方明确给出的base之字节偏移，sequence节点把多个扫描作用于同一base；array节点是自包含的`Array { length_offset, first_element_offset, stride: NonZeroU64, element: NonEmptyRefScan }`，从同一object base读取length并定位首元素。TypeDescriptor的`object_scan`永远相对managed object start；shape中的`inline_scan`只相对box payload或单个array element起点。BoxedValue必须把非空inline scan按payload offset平移成object scan；InlineArray固定使用`length_offset == 16`、`first_element_offset == inline_offset`和shape中的nonzero stride，不能假设首元素总在24。ZST或其他GC-free payload/element的两个scan都为`None`，不得编码`stride == 0`后按length重复空扫描。tagged enum允许完全不含managed ref的variant复用pure-value payload区；每个直接或间接含managed ref的variant拥有互不重叠的连续slot，构造时把inactive slot清零，因此所有ref-bearing variant中的ref leaf直接合并为固定偏移，不存在按tag分派的扫描节点；
- **enum 扫描不读取 tag**：tagged enum的所有潜在ref位置都位于ref-bearing variant的独占slot，按普通固定偏移检查；inactive独占slot必须为全0，pure-value共享区不进入扫描。niche表示的managed-ref enum整体是一个普通引用位置，`Ptr` / `FunPtr` niche不是managed root。没有出站引用的节点可用空指针表示。LIR meta 的`RefScan`提供该信息（见 impl spec 2.4）；
- 父类型信息（接口、父类）；
- 虚分派结构：内嵌 **vtable 指针**与 **itable 数组**（itable 以接口 TypeDescriptor 指针为键）。普通类型的vtable只含实际需要virtual dispatch的class方法，slot从0开始且允许为空；closure的vtable第0槽为2.6规定的固定动态invoke；`Any`没有方法或固定前缀。`ToString` / `Hash`及声明operator equals的interface均走普通itable；装箱值类型的表项指向this调整thunk（impl spec 2.9）。TypeDescriptor的类型名只用于诊断，runtime不得据此合成用户可见字符串、哈希或相等语义。
- **release policy**：M24起物理descriptor末尾带一个nullable静态release-hook entry。它是typed `None | SynchronousGcFree { hook }` policy的最终编码；非null时只允许普通final class的exact descriptor，并须指向与该exact owner双向登记的release-safe thunk。runtime不得按类型名、字段、方法或annotation猜测policy（spec 9.1.6；本文3.8）。

generic nominal application始终invariant。每个fully specialized application以自身TypeDescriptor及替换完成的exact base/interface closure参加`is`/`as`；runtime不按nominal template或type argument subtyping合成其他application关系，不解析UTF-8类型名、比较mangled symbol前缀或把同template的application擦除为相等。Scoop没有projected/star generic descriptor、wildcard lookup或runtime generic dictionary。

M23以后TypeDescriptor地址是program-wide materialized exact type identity的一部分。同一个generic/structural specialization被多个Cone发射时，其descriptor、vtable/itable、scan/layout constant与相关adapter必须属于同一个canonical ODR group并由linker真正coalesce；同persistent exact type/group id出现两个地址、或不同exact type id共享一个地址都是fatal metadata error，runtime不得按结构相等、类型名、相同layout或“先登记者”合并。param-free nominal类型由定义Cone强定义；含consumer-local type argument的新application及跨Cone重复materialize的structural exact type由consumer发射ODR定义。

同一`PersistentExactTypeId`的canonical diagnostic bytes必须完全相同。TypeDescriptor的canonical LIR definition按值包含该span，descriptor `ObjectDefinition`/`descriptor_fingerprint`因此覆盖其length与bytes；span指向的只读diagnostic atom也必须由对应strong definition plan覆盖或作为type ODR group显式member进入object/ODR fingerprint。artifact reader与final linker从exact key重算并验证；runtime把通过验证的span视为诊断用opaque bytes，只做生命周期、只读range及同identity逐byte一致性的防御检查，绝不从它反推type identity。

M22的八种integer value按1/2/4/8 byte精确存储并使用target自然对齐；嵌入aggregate/array/CLayout或装箱时不扩成统一8-byte payload。canonical identity固定为`Int8`/`Int16`/`Int`/`Long`的i8/i16/i32/i64与`UInt8`/`UInt16`/`UInt`/`ULong`的u8/u16/u32/u64；`Byte`/`Short`/`Int32`/`Int64`与`UByte`/`UShort`/`UInt32`/`UInt64`分别是对应identity的transparent alias，不能产生多份TypeDescriptor或按alias名称选择布局。C ABI精确映射为`int8_t`/`int16_t`/`int32_t`/`int64_t`与`uint8_t`/`uint16_t`/`uint32_t`/`uint64_t`；因此`Int`不是C `long`，`Long`也不随平台C `long`变宽。当前runtime target profile同时要求data/code pointer恰为64位、两类null carrier全零，并保证合法非null地址与各自内部64位raw carrier逐bit往返；build/C bridge作宽度静态断言，target artifact test验证null表示与往返能力。M22暂不引入platform-native integer：原有64位pointer/size源码surface暂时用`Long`/`ULong`，即`Ptr(raw: ULong)`及`toULong(): ULong`承担raw往返、元素offset用`Long`，`sizeOf`/`alignOf`返回`ULong`；`FunPtr`仍不开放integer构造或accessor。不满足profile者不是可运行target，runtime不能截断/扩展pointer伪造结果。runtime metadata中的版本、tag、id、offset、size、count、epoch与state等`u32`/`u64`/`size_t`标量是各自独立的内部typed carrier，不是源码`Int`/`UInt`/`Long`/`ULong`值，不得为复用源码operator、boxing或FFI规则而伪装成任一源码integer。

ZST的`ValueStorageLayout`保存logical size 0、严格大于0的alignment与空value scan；不同exact ZST即使layout相同也保留不同persistent identity与TD地址。它的TypeDescriptor使用`BoxedValue { ZeroSized }`：`inline_size == 0`且两个scan为空，但`inline_offset = alignUp(16, inline_alignment)`，managed box的`minimum_size`仍是按instance alignment规范化后的非零值。runtime不得把ZST解释为缺失layout、未知size或“无需descriptor”。需要地址identity的source/static place遵守spec 4.7与2.8的1-byte token规则；token既不进入value layout size，也不充当box payload。

M24起`ScoopTypeDescriptor`的精确字段顺序为`type_id, instance_shape, object_scan, parent, vtable, itables, itable_count, diagnostic_name, relation_kind, related_type_count, function_result, release_hook, related_types`；M23保留2.6的函数签名固定字段与尾部参数表，M24在固定部分末尾、参数表之前追加hook并bump runtime ABI fingerprint与`.slib`的HIR/MIR/LIR三层wire schema，旧artifact必须重建。2.8 带 prefix 的八类 runtime metadata record 在 M24 继承 ABI 3，M27 按第 9 章统一使用 `abi_version == 4`及各自精确字段/size；TypeDescriptor本身没有prefix，不能把metadata record版本误当成它的布局版本或据此接受旧TD。hook的C ABI固定为`void (*)(void *object_start)`。shape顺序仍为`u32 instance_kind, u32 inline_storage_kind, u64 minimum_size, u64 instance_alignment, u64 inline_offset, u64 inline_size, u64 inline_stride, u64 inline_alignment, inline_scan*`（M23基线C spelling见M23设计6.1，追加字段见M24设计4.1）。合法矩阵如下：`FixedObject`的minimum是含header和尾padding的exact allocation、alignment至少8且inline字段全零/null；`BoxedValue`按`inline_offset = alignUp(16, inline_alignment)`与`minimum_size = alignUp(inline_offset + inline_size, max(8, inline_alignment))`保存未装箱value layout，stride为0，object scan是inline scan的checked平移（References offset相加、Sequence递归、Array的length/first offset相加而element child不变）；`InlineBytes`固定minimum/offset 24、alignment 8及1-byte inline size/stride/alignment；`InlineArray`固定minimum/offset为`alignUp(24, inline_alignment)`，非ZST element要求`inline_size == inline_stride > 0`，ZST element要求size/stride为0，object scan只在element scan非空时使用`Array { length_offset: 16, first_element_offset: inline_offset, stride: inline_stride, element: inline_scan }`；`AbstractRef`的minimum/alignment/inline字段与两个scan全零/null且任何allocation均fatal。`minimum_size == 0`因此只表示AbstractRef，不表示ZST。除普通final class的`FixedObject`外，`release_hook`必须为null；`Throwable`继承闭包、intrinsic/compiler-generated class及其他被spec 9.1.6排除的owner即使shape为FixedObject也必须为null。target profile给出非零`maximum_managed_alignment`与`maximum_managed_object_size`；layout超过alignment上限必须在LIR阶段拒绝，任意allocation超过object-size上限必须失败，allocator不得降级、wrap或截断。descriptor/scan/release-policy、C/LLVM `sizeof/offsetof` 及对象格式在编译/读取边界验证；runtime 只补充加载地址、实际 scan 与 GC 契约检查，分工见 3.8。

M24 `.slib`继续使用v1 container，HIR/MIR/LIR三层wire schema version全部固定为2，任一v1或版本不一致的artifact都不升级读取；runtime ABI fingerprint与manifest composite identity-ABI fingerprint随TypeDescriptor布局及callable-body-v2 identity一起改变。其他persistent identity与`persistent-v1` mangling schema不变。2.8 prefixed metadata record 自身在 M24 使用 ABI version 3，M27 升为 4；这些版本维度不得互相替代。

TypeDescriptor 的 canonical LIR definition 保存 `None | SynchronousGcFree { release_hook_callable_id }`；object definition 将实际 hook relocation 正规化为同一 typed body id，ASLR 地址不进入 fingerprint。reader/linker 检查该物理引用与已生成 policy 一致。runtime 由 exact type id 得到预期 ReleaseHook body id，再检查加载后的 callable entry；它复用已登记的布局和既有 ODR 判等结果，不重新计算 body、descriptor 或调用图摘要。

scan program v1的word编码固定为：null表示empty；普通节点为`[count, offset...]`且`0 < count < UINT64_MAX-1`；sequence为`[UINT64_MAX-1, child_count, child_ptr...]`且`child_count >= 2`；array为`[UINT64_MAX, length_offset, first_element_offset, nonzero_stride, nonempty_element_scan_ptr]`。canonical `RefScan` normal form规定：References offset按无符号byte offset严格递增且无重复；Sequence不含None或直接Sequence child，先把同层References合并为一个offset并集，再将包含该合并节点在内的全部已规范化child统一按`(child ScanFingerprint, canonical child bytes)`严格递增、去重，零/单child分别折叠为None/该child；Array的element是已规范化的NonEmptyRefScan。producer 在 fingerprint/发射前规范化，reader 在产物读取边界检查 canonical 形式与摘要；runtime 在登记边界检查实际节点范围、结构、错对齐、越界及成环 graph，复用未变化的规范化结果，不重新展开计算 scan hash。scan 的 offset、stride、count 按实际对象范围使用 checked 算术；共享 DAG 通过节点或节点对记忆化避免重复比较，active path 检查非法环。删除节点、word、展开次数与逻辑 canonical-byte 配额，不因任意成本超额 abort，也不将遍历策略或成本常量写入 runtime ABI fingerprint。

scan的canonical typed bytes使用2.8的scalar/count规则且没有pointer：`None = u32(0)`；`References = u32(1) || u64(count) || each u64(offset)`；`Sequence = u32(2) || u64(child_count) || each canonical child`；`Array = u32(3) || u64(length_offset) || u64(first_element_offset) || u64(stride) || canonical element`。`ScanFingerprint = SHA-256(ByteSpan("scoop-scan-v1") || canonical typed bytes)`；共享物理 child 的语义编码保持不变，编码长度做溢出检查；比较不必重新展开整棵语义树。static storage的None scan使用canonical `[0]`非null sentinel；该sentinel不是合法Recursive root，TypeDescriptor empty scan按shape矩阵使用null。

访问域、override 和签名可见性由前端检查，runtime 不消费 lookup/override/signature-exposure witness；删除这些 HIR 记录不改变 runtime C ABI、String 表示或 GC 契约。

本地类型实现外来接口时，dispatch 表直接引用定义方的完整 callable 与 canonical ABI，值类型装箱 thunk 保留实际 payload/receiver 适配和 GC 根。该消费能力使用现有 C ABI 与 String 表示，不复制外来方法定义或新增启动协议。

不可变 TypeDescriptor、instance shape 与 scan 在负责其输入的边界完成完整验证。M23-6 的编译器验证 typed LIR，外部产物 reader 验证完整静态表示；runtime 正常分配、装箱、数组和 GC 路径消费这些已验证常量，只检查当前 pointer/TD、对象范围、动态 length/size 溢出、payload 对齐及具体 GC/root 契约。完整静态交叉核对保留为 `scoop_shape_validate`，并可用 `SCOOP_VERIFY_METADATA=1` 编译 runtime，在操作入口显式启用；不为此增加静态登记表或指针缓存，也不提前实现 M23-8 的多 image 登记。M23-8 引入外部静态 image 的实际登记边界时，完整检查在该边界进行一次，结果供正常操作复用。

### 2.3 装箱

值类型装箱为堆对象：对象头 + 对齐填充 + 按值存储的payload（spec 4.4.4）。分配、复制和拆箱只消费TypeDescriptor的`BoxedValue` shape，不能固定假设payload在`+16`：payload位于`inline_offset`，allocation使用`minimum_size`，inline scan相对payload而object scan相对object start。compiler/runtime内部ABI以两个互斥入口实现LIR的`BoxPayload::{ZeroSized, NonZero { source_place }}`：`scoop_rt_box_zst(td)`只接受`BoxedValue/ZeroSized`且没有payload pointer；`scoop_rt_box_value(td, source_place)`只接受`BoxedValue/Inline`及满足其size/alignment的non-null readable、跨该入口地址稳定的caller place。NonZero value的inline scan非空时，caller必须先把完整value写入该place，再以TD的inline scan push覆盖它的native recursive-region root；push本身NoGC，且必须发生在调用入口及其managed-entry handshake/park之前。该root保持到entry handshake、allocation和从经collector更新后的同一place复制payload全部完成，`scoop_rt_box_value`返回后才由caller按LIFO pop；root entry 的 scan 指针直接取该 TD 的 `inline_scan`，runtime 比较活动 root 的 place 和 scan 指针，不再展开比较相同静态 scan；不能等进入入口后补登记。空scan可省略region frame。不能在push前后缓存payload中的AS1 ref或改从旧副本复制。对应拆箱入口为不接收result pointer的`scoop_rt_unbox_zst(obj, expected_td)`与接收non-null writable destination的`scoop_rt_unbox_value(obj, expected_td, destination)`；两者都先验证exact TD identity与shape。size、alignment和scan只从已验证TD读取，调用者不得再传一份可能矛盾的size/scan。旧的`box(td, payload, size, scan)`形态不属于M23 ABI。ZST payload不占字节，但box仍分配普通非零对象、登记object start/size并获得新的ref identity，不能返回共享singleton或ZST address token；ZST unbox只产生typed logical value，需要可观察的value-type `this` place时另行物化token，不能把位于对象末尾的零字节payload地址暴露出去。装箱赋予对象identity但不赋予通用`==`：相等按表达式静态引用类型声明的成员operator equals分派；`Any`或未声明equals的interface不能使用`==`。TypeDescriptor没有通用结构相等入口。

所有 Scoop 方法 receiver 都按值传递（spec 3.3）：ref-type receiver 复制 managed ref value，因而仍指向同一对象；value-type receiver 复制完整值。装箱值经 interface 分派调用值类型实现时，box 只是 payload 的存储来源；dispatch thunk 用 payload 的值初始化 value-type `this`。thunk 可以在不可观察时借用 payload 地址作为 ABI 优化；若 unsafe/interior-mutable 路径能观察或修改存储，必须先复制 payload，不得把 box 内部地址暴露为 `this`。

### 2.4 `String` / `Array` 布局

- `String`：对象头后在 offset 16 保存独立 typed `u64` physical byte count，内联 UTF-8 字节数据从 offset 24 开始（spec 11.4）。M26 的 `byteLength: Long` 读取该 count；`length: Long`、get index 与 slice boundary 按 Unicode 标量值计数，不能把 physical byte count 直接当作字符数。byteLength 与 length 都位于 `0..=INT64_MAX`，边界实参保留完整 Long 输入域。String 数据始终为合法 UTF-8，不要求结尾 NUL，也不新增字符索引表、off-heap 指针或 owner 字段。physical count 为 allocation/copy/runtime 使用的 machine scalar，不是源码 ULong 值。
- `Array<T>` / `MutableArray<T>`：对象头后在offset 16保存一个独立typed `u64` physical count，offset 24是未考虑元素过对齐时的data boundary，实际元素区起点为`alignUp(24, alignOf<T>())`，object allocation alignment至少为`alignOf<T>()`。源码可见`size`、`get`/`set` index与iterator index均为`Long`（i64）；实际logical size限于`0..=INT64_MAX`，index实参仍可取任意`Long`值并由普通bounds语义拒绝负数或越界值。physical count只是为allocation、copy和runtime scan服务的machine scalar，不是源码`ULong`字段。非ZST值元素不装箱且以nonzero `sizeOf<T>()` stride连续布局；ref元素使用pointer stride。ZST元素使用封闭`ZeroSized { alignment }`分支，任意logical size的exact allocation size都等于元素区起点，不计算`size * 0`、不物化逐元素token，也不调用payload `memcpy`；clone/互转仍分配fresh目标对象并复制logical size。bounds与整数index迭代仍按spec 10章执行。只有元素子扫描非空时TypeDescriptor才使用带length/data offset及nonzero stride的array scan；ZST/GC-free元素直接为`None`。因此collector工作量不随ZST array length增长，也不会反复访问同一地址；tagged enum元素的inactive variant slot同样保持全0。

variable allocation 公式固定为：String `alignUp(24 + byte_count, 8)`；Inline array `alignUp(inline_offset + count * inline_stride, instance_alignment)`；ZeroSized array 恰为 `minimum_size == inline_offset`。literal/assembly/vararg 的 logical count 由非负 component count 经 checked 加法得到；M26 的按长度构造先在 managed CFG 拒绝负 Long，再把非负 logical count 转为内部 typed u64；clone/互转先验证 source exact array TD、physical count 与 side metadata 一致且不超过 `INT64_MAX`。内部入口收到非法 count 或布局是不变量错误，不在 native frame 中构造源码异常。乘加/alignUp 均使用 checked u64，结果须可转 target size_t 且不超过 maximum_managed_object_size；溢出、对象过大和资源耗尽沿用 fatal allocation failure。collector 扫描前用 side metadata exact size 检查 count、first offset 与 stride 的动态覆盖范围。

M26 按长度初始化使用现有 array TD 与固定物理 count。分配时完整清零存储，在首次 initializer 调用前建立正确对象头、count 和 managed root；初始化中 collector 可按完整数组长度扫描，因为尚未写入的所有 ref leaf 都为零，已有 tagged enum 的扫描也不依赖未写入的 tag。零字节不是任意 T 的语言值：generated CFG 只写本次 initializer 的完整结果，普通源码不能读取或获得正在初始化的数组，成功退出才发布完整结果。Array ref、initializer closure、捕获与临时返回值按既有精确根/statepoint 规则更新；每次 callback 后从最新 ref 计算目标槽，不能缓存跨 safepoint 的元素地址。initializer 由 managed 调用执行，异常按普通 CFG 展开，native allocator 不调用用户 closure。ZST 仍执行全部 initializer，只省略物理元素 store。

`ArrayList<T>` 和 `StringBuilder` 使用普通 class 布局与字段扫描，不增加 runtime instance-shape kind。ArrayList 的有效 size 属于 owner 字段；其 `MutableArray<Option<T>>` backing 的 physical count 始终是容量，collector 扫描完整容量，并忽略 None 的零 ref leaf。增长前后两份数组按普通根保活；移除/clear 的失效槽写入完整 None，使用既有 typed store/屏障路径。`Char` 的 inline 表示是 4-byte、按 target u32 自然对齐的合法 Unicode scalar，GC-free；Option<Char> 不借用本里程碑未定义的新 niche。

### 2.5 `Option` 的 niche 表示

引用类型的全0机器字表示`None`；`Ptr`/`FunPtr`的内部data/code-pointer carrier为全0时同样只表示对应`Option`的`None`（spec 7.4）。裸reference/`Ptr`/`FunPtr`值均维持非零不变量，因而`Some(payload)`不可能与`None`碰撞。GC扫描时必须识别niche编码，不得把全0当作有效引用追踪。

### 2.6 函数值与 closure

managed 函数值是普通引用对象，不是原生函数指针。每个 concrete closure 实例包含编译器控制的 invoke entry 与零个或多个不可变捕获字段；语言只允许捕获不可重新绑定的 binding，不存在 compiler-generated shared cell。closure、函数类型型变 adapter 与 suspend closure frame 都必须有各自的 TypeDescriptor 和完备的递归引用扫描描述。

M23-7 的固定动态 invoke 将 runtime metadata ABI 升至 2。`ScoopTypeDescriptor` 保留原 128-byte 前缀，在 offset 128、132、136 分别追加 `relation_kind: u32`、`related_type_count: u32` 和函数结果 TD 指针；有序相关 TD 指针紧随 144-byte 固定部分。kind 0 表示无附加关系，count 和结果必须为零；1／2 表示 ordinary／suspend 函数形状，相关类型为声明顺序的参数；3 表示 interface，相关类型为按 exact ID 排序的直接父接口且结果必须为零。接口父引用不可为空，运行时沿这份既有继承图执行函数型变所需的 subtype 查询，不产生派发表槽或新的调用 ABI。函数形状的空结果／参数 TD 明确表示 Any。整个描述符及尾部参数表属于同一个只读对象 atom，大小为 `144 + count * 8`，采用 checked 算术；没有额外登记表或 runtime 分配。对象头、实例布局、扫描规则和普通调用 ABI 保持不变，旧 metadata ABI 的产物与缓存需要重建。

- invoke entry、TypeDescriptor 及其他代码/metadata 指针不是 managed 引用，不进入 GC 扫描描述；captured value type按 concrete layout直接内联在 closure对象中，其中的引用必须按普通字段递归扫描；该内联存储没有独立对象头、identity或额外TypeDescriptor，不构成 boxing；
- concrete closure 的 TypeDescriptor 以 parent 记录其 exact function type descriptor。function type descriptor 保留挂起性、有序参数和结果的运行时类型引用，`Any` 使用明确的空引用；值类型引用其实际 box descriptor，嵌套函数引用其函数形状 descriptor。运行期 `is` / `as` 按参数逆变、结果协变检查完整签名；引用继承和接口关系复用普通 TD，指针、native pointer 和其他值的精确类型不因物理布局相同而等价；
- 每个 closure 的 vtable 第 0 槽是固定的动态 invoke：保留参数个数和挂起性，将显式参数和结果以 `Any` 传递。该 managed bridge 按 closure 的精确源码签名解包参数、调用普通 typed invoke 并装箱结果；嵌套函数值使用相同的动态适配。检查通过后的目标 adapter 负责相反方向的参数装箱和结果解包。suspend bridge 继续使用 `Continuation<Any>`／`CoroutineStep<Any>` 的既有 hidden ABI。静态调用仍使用原 typed invoke；此动态调用边界不改变泛型单态化；
- closure 的 TD、动态 invoke 及完整签名只依赖其本身的 identity，不随消费者的检查目标签名变化。不同 Cone 增加适配需求时发射独立 adapter，不能修改共同 TD 或要求提供方预见所有下游函数类型；
- 无捕获 closure 可以由编译器放入静态只读对象或复用单例，但其对象头和 TypeDescriptor 仍须满足普通引用对象契约；
- closure 的分配、调用与回收不需要新增 runtime API，走现有 managed 分配、statepoint 与动态分派设施；
- 本节对象不得直接当作 `FunPtr` 交给原生代码。spec 13.10 的 `FunPtr` callback 是独立的 GC-free 原生地址；spec 14.3 的 GC-aware closure 回调则必须先通过 runtime 注册/保活协议。

### 2.7 Initialization unit 与 singleton 发布（M21；M29 修订）

跨 Cone singleton 访问调用对应完整singleton identity的同一个ensure入口，再读取其已登记的published root。参数自由object/companion复用定义方的Strong记录；M29的generic companion按完整宿主application物化并沿既有ODR合并，多个consumer不能各自保留独立状态。对象引用离开读取点后遵守普通GC root规则；失败缓存、循环检测、发布顺序及C调用约定沿用本节契约。

**M29 泛型 companion 修订。** 每个实际使用的generic companion application拥有独立的cell、initializer/ensure、published root、failure root及initialization registration。`Box<Int>.Companion`初始化成功或失败不改变`Box<String>.Companion`的状态，二者只有显式代码依赖才互相ensure；同一application跨Cone和image只初始化一次。即使T未出现在成员布局中，状态也不能按相同布局或函数正文合并。直接宿主application与companion声明共同决定这些记录的typed identity；未具体化模板没有对象、cell或初始化执行。

同一generic companion application的对象类型、初始化支持和可变状态按普通ODR归属一致物化。多个image的引用必须由linker合并到相同记录与地址，registration及root按原规则只登记一次。runtime直接消费编译器生成的普通exact type和unit记录，不根据类型名或类型参数构造singleton，也不增加运行期泛型对象工厂。宿主实例构造、类型查询和const读取不隐式初始化companion，实际非const访问才进入本节lazy gate。

每个需要runtime求值的top-level property及每个实际singleton拥有独立、GC-free的`ScoopInitializationCell` side metadata；companion以完整宿主application区分singleton。cell不位于managed object内，也不是TypeDescriptor字段。状态为`Uninitialized`、`Initializing(owner_thread, dependency_stack)`、`Initialized`或`Failed`。cell只控制整个generated initializer entry恰好执行一次：ordinary stored property在Initialized状态下始终已有声明type的合法值，`var p: T?`省略initializer时写入的就是普通`None`；runtime不得为property另建late-init bit或读取检查。

零字节普通属性或委托值仍保留其 initializer、ensure、cell 和求值副作用；初始化登记关联原属性拥有的 `StaticPlaceToken` 存储。该角色只用于实际 value byte size 为零的存储，其 allocation extent 与空 scan 沿已有 ZST 规则处理；非零字节值仍使用实际 backing/delegate 角色。此关系不增加 runtime 状态或 C ABI。

singleton winner线程先按2.1/3.1普通分配对象，只把initializing receiver保存在generated managed frame的精确root中；base/common initializer全部成功后，才把ref以release语义写入已登记的published global root slot并把cell转为Initialized。loser线程以acquire观察terminal state后从published slot重新读取；不得在Initializing时读取临时ref、pin对象或把旧地址缓存在cell。moving collector只更新普通root slot。

initializer抛异常时，singleton不发布，top-level storage不被视为可读；runtime把managed Throwable写入单独登记的failure root slot、转为Failed并唤醒waiter。本次和以后ensure都重新抛出该failure且不重试；已发生副作用不回滚，未发布object按普通可达性回收。

同线程重入由dependency stack检测并产生带稳定unit path的cycle结果。跨线程等待由init coordinator登记wait edge；加入新edge形成cycle时，该访问产生同类结果。generated ensure把path复制为普通managed `String`后交给编译器协议已经解析的core cycle thrower；该target在core内构造并抛出`message`包含完整cycle的`IllegalStateException`。它若未被initializer内的普通`try`捕获，才使当前unit失败并逐步唤醒其他waiter。runtime不按名称查找或构造core对象，也不认识该thrower的symbol。无环等待必须使用与3.5 epoch/STW handshake兼容的park；不能持有未登记managed pointer睡眠，不能busy-spin阻止safepoint。

每个Cone image在2.8的typed descriptor中提供init-unit span；unit identity与初始化排序键只使用kind-specific `PersistentInitializationUnitId`，ODR去重另使用其group/member，不再另造“Cone coordinate前缀字符串key”。该persistent id的definition key已经包含origin `ConeIdentity`或specialization identity；coordinate/declaration path只放在独立`diagnostic_path`用于cycle/错误显示，不参与unit identity或排序，但仍作为普通 descriptor/RuntimeImage 内容被验证；M31 不将诊断路径内容加入跨实现 ODR 判等。descriptor还必须关联cell、generated initializer/ensure entry、no-throw startup gateway以及其published/property与failure storage registration。所有storage都进入2.8的static-storage descriptor span；`scan_kind == Recursive`者才按3.3成为GC root，init descriptor中的opaque状态不能代替root登记。

generic delegated extension specialization 是 lazy unit，其 storage、cell、failure root、initializer/ensure 与初始化登记按既有角色属于同一 ODR group，每次物化必须完整。M31 由 linker 选择兼容定义并将全部引用解析到唯一 storage/cell/registration，runtime 按实际 semantic id、group/member 与关键地址验证后只登记一次；相同 id 地址不同仍 fatal。独立 helper 可来自不同 image，初始化正文及其自身 GC/EH 数据随正文选择；不比较各 producer 的机器实现内容。`by` 与 provide 不接收某次访问的 receiver；get/set 先按普通调用求值参数，再经 ensure 取得 effective delegate，失败与等待沿用本节状态机。

M23-8 起上述 coordinator API 直接接收 `const ScoopInitializationUnitDescriptorV1 *`，其参数就是六类表中实际登记的 unit record。旧 coordinator descriptor、独立 `id` symbol 与相应对象/摘要计划删除；schedule 使用已有 Eager=1/Lazy=2 tag，diagnostic path 使用 byte span。state/result code、cell 与失败发布语义保持，热点只检查实际 record 引用和本次状态，不能重复验证会改变的初态。

### 2.8 M23 多 image 登记与启动

**M31 修订：**ODR 只在普通依赖定义一致的前提下比较 typed key 与共享 ABI，不比较不同 producer 的 LIR、机器码、EH 或 stackmap。artifact-only program-link 先选定每个兼容定义，再将 callable 正文、EH、stackmap、callable/safepoint registration 和 Context cell 等实现附属数据作为同一物理选择保留；runtime 只看到所选实现。不同优化级别不能让同一 body 使用另一份实现的栈图，也不能产生两个 TD、static storage 或初始化状态。独立 helper 仍可取并集，不要求整个泛型 group 选择同一 producer。

M31 的 `.slib` 保存各 Cone 的完整候选登记数据与物理归属，最终链接按所选定义生成每 Cone 的 image descriptor 和 pointer 表。落选正文及其私有站点不进入最终对象或登记集合；共享登记只在实际选中 provider 的 image 中列一次。image identity、依赖顺序和初始化协议保持；选择结果与最终 image 内容进入现有链接缓存和内容摘要，不改写输入 `.slib`。本节历史版本中“每个输入 image 已固定全部登记表、native linker 独立 weak coalesce”的做法由此取代，具体产物分工见实现规范 2.19。

M31 将 metadata ABI 从 4 升至 5，删除六类 registration 公共 identity 中仅用于内容判等的 `definition_fingerprint[32]`，保留 linkage、semantic id 与 ODR group/member。`OdrDefinition` 以及仅用于回填该公共字段的 `StrongRegistration` 摘要节点一并退出；记录内容由已有 RuntimeImage／Code／Artifact 摘要覆盖，不增加替代正文摘要。仍有实际消费者的 body、descriptor、layout、scan 和 normalized-stackmap 字段保留，用于所选实现自身的格式、引用和 GC 检查。共同结构缩短 32 bytes，callable record 由 208 变为 176 bytes，其余五类登记按同样删除位置更新；TypeDescriptor、对象头与 managed reference 表示不因此改变。canonical record 同步删除该公共字段；修改 RuntimeImage 编码域及受影响 section/profile，旧产物和缓存重建。ABI 5 的共同 prefix 与 identity 共占 120 bytes；safepoint、callable、type、immortal、static-storage、initialization-unit record 大小依次为 200、176、208、152、264、320 bytes。RuntimeImage 内容编码域为 `scoop-runtime-image-v2`，runtime ABI contract 为 9，受影响的产物 section 版本见实现规范 2.19。以下 ABI 2～4 的字段与摘要公式保留为迁移记录，M31 不再消费其中退役字段。

M23-8/9 继承 M23-6a 的共同 HIR 和 M23-7 的完整机器定义闭包。登记与启动不接收 AST、模板或待推断类型，不补造缺失 callable、scan、TD 或初始化服务；这些缺口在编译／产物消费边界报告。六类 registration、普通与 ODR 定义的去重和动态 GC 检查沿本节实际数据执行，不另建来源专用路径。

M23-7 已验收的产物包含每 Cone 的 image、六类 registration 和 executable root entry；此前运行验收使用单 image 适配入口。M23-8 的详细设计见 [阶段设计](../milestone23/stage8/DESIGN.md)，编译器职责与格式迁移见实现规范 2.14；正式 artifact-only program-link 仍由 M23-9 完成。

M23-8 的私有 C 启动入口为 `int scoop_rt_run_program(const ScoopImageDescriptorV1 *const *images, uint64_t image_count, const ScoopRootEntryDescriptorV1 *root_entry)`。调用方只提供实际产物中 image 与唯一 root entry 的静态引用，不另发射 program/core descriptor。image 集合非空且恰为 root 的完整依赖闭包；输入枚举顺序不承担初始化语义。runtime 在加载后的 image/dependency 记录上构造一次语言规范 12.3 的 canonical Kahn 顺序，各 Cone 的 eager unit 再按 persistent unit ID 排序；不根据名称、符号前缀或归档成员顺序发现记录。该入口在一个进程中只调用一次，正常完成返回 0，未捕获异常仍按语言规范 11.7 打印类型名并 abort。

M23-9 的正式链接按 [阶段设计](../milestone23/stage9/DESIGN.md) 生成 C main、静态 image pointer array 与唯一 root 引用，直接调用上述入口。runtime 自身不提供 C main，也不要求 program/core descriptor；该阶段保持当时的 metadata ABI 3、初始化、gateway 和 GC 行为，M27 的 ABI 与 task 入口扩展见第 9 章。`scoop_td_String` 由实际 typed TD 的等地址 alias 满足，不增加 TD 或 pointer slot。program-link 在文件边界检查入口、绑定、ODR 地址与 section 保留，本节继续负责实际加载后的权限、registry 和精确 stackmap；两边不重复重算语言布局或整个 RuntimeImage/Graph/ODR。

M23-10 的普通 native object、archive 成员与进程装载时引入的 dylib/framework 按 [阶段设计](../milestone23/stage10/DESIGN.md) 参与 FFI 符号解析，不成为新的 Scoop logical image，不增加 registration、program descriptor 或 runtime metadata ABI。native C 的全局数据与静态初始化 TLS 仍为 GC-free 外部存储；Darwin TLV bootstrap 不登记 managed root，也不等同于本节禁止的运行时新增 Cone。SourceExtern 的 C native-safe、Scoop native-borrowed、caller-root、回调 attach 和异常边界继续按第 3～5 章执行。链接器检查实际选中对象和装载绑定，不要求 runtime 再次扫描 native 文件、确认 producer 身份或重放 link plan；外部实现遵守 FFI 契约仍由作者负责。

M28 的 Linux ELF image adapter 从 `dl_iterate_phdr` 取得当前 executable 的 `PT_LOAD` 与 load bias，并从隐藏 linker symbols `__scoop_stackmaps_start/end` 取得完整 stackmap blob 范围；不读取磁盘 ELF 或运行时 section headers。所有 Scoop logical images 仍由 startup 显式传入，native DSO 不成为 Scoop image。权限检查将加载段与 `/proc/self/maps` 的实际映射求交，因此动态 loader 完成 relocation 后的 RELRO 会被视为只读；`/proc/self/maps` 是本阶段 Linux runtime 的环境要求，不能只凭 ELF section 名或 `PT_GNU_RELRO` 推断权限。查询在启动 image 边界完成，GC 使用得到的静态范围快照。动态 PIE 的 metadata/stackmap 进入 RELRO；静态 musl 的已重定位不可变记录放入只读 `PT_LOAD`，不能依赖 musl static 启动过程施加 RELRO。metadata ABI 保持 4。

共有 runtime 对 callable entry 检查实际首 byte 位于可读、可执行且不可写的映射，并按 registration 验证地址身份；不把 AArch64 的 4-byte 指令宽度/对齐应用到 amd64。机器指令入口的架构约束在 compiler/object reader 边界承担，不由 runtime metadata 通用代码再次解析指令。

M23-11 的公开 `scoop build/run/link` 复用上述正式启动对象和 `scoop_rt_run_program`，不增加 CLI 专用 runtime 入口、registration 或第二份初始化顺序。`run` 在新的程序进程内执行 binary；默认当前 `Cone.toml`、debug/release 构建 profile、输出布局、缓存、诊断、argv 转交和退出状态由工具层处理，详见实现规范 2.7 及 [阶段设计](../milestone23/stage11/DESIGN.md)。两种构建 profile 当前沿用相同 runtime 构建设置，不按 profile 名切换 GC／检查／异常行为；后续优化设置由实际 runtime-build 输入表达。CLI 统一消费当前 metadata ABI 及 GC／FFI／异常合同和 native 装载规则，历史 fixture 迁移同样消费实际多 image 启动。

登记只检查当前加载边界的格式、实际地址、引用、唯一性、扫描与初始化契约。编译器、reader 和 linker 已完成的语言、布局、ABI 与 ODR 选择不在 runtime 重放；runtime 不重算 RuntimeImage／Graph／ObjectDefinition fingerprint，也不验证两份不同优化正文是否等价。实际 stackmap 的规范化核对用于确认所选实现的 PC、owner 与 root locations。完整不可变记录检查一次，同址重复引用与 GC 热点直接复用结果。

空接口仍保留以真实 interface TD 为键的 itable entry，其无槽派发表沿既有 codegen 表示为 null；空 vtable 同理。runtime 检查非空 dispatch pointer 的只读起点，不拒绝合法的 null 空表，也不从地址或接口名称推测槽数。实际槽清单、空表与接口契约的一致性由编译器及对象 reader 验证。

M23-6 删除没有实际 producer/consumer 的 `ScoopRuntimeCoreBindingsV1`、`ScoopProgramDescriptorV1`、固定 String capability ID 及其 LLVM 布局镜像和专用测试，不把它们改名后保留。未使用的 program/core magic `0x53434f4f50505247`、`0x53434f4f50434f52` 退役且不复用。M23-8/9 使用上述普通 C 参数及实际引用，String 继续通过实际 `scoop_td_String` 定义接入，不另建授权表。

M23-7 实际发射的私有 metadata ABI 版本为 2；M23-8 切换为 3，明确区分统一 initialization descriptor 参数与旧 coordinator 参数，旧产物和缓存重建。带 prefix 的 record 以 `{ u64 magic; u32 abi_version; u32 struct_size; }` 开头；image/entry/storage/immortal/init/type/safepoint/callable 的 magic 依次为 `0x53434f4f50494d47`、`0x53434f4f50454e54`、`0x53434f4f5053544f`、`0x53434f4f50494d4d`、`0x53434f4f50494e49`、`0x53434f4f50545950`、`0x53434f4f50535054`、`0x53434f4f5043414c`。M27 起要求 `abi_version == 4`，`struct_size` 与共享 header 的 `sizeof` 精确一致，reserved fields 为零。callable record 按第 9 章扩为 208 bytes，其他 record 保持原字段顺序与 size；TypeDescriptor 使用 M24 的 152-byte 固定部分加相关类型尾表。消费边界先确认 prefix 所在内存范围，再检查 prefix，最后读取完整 record。Darwin/AArch64 使用 little-endian、64-bit pointer 和 natural 8-byte struct alignment；C header 与 LLVM 布局由实际产物测试核对。

image 字段顺序为 `prefix, canonical {group bytes, name bytes, version bytes, ConeIdentity}, runtime_image_fingerprint[32]`，随后是直接依赖 identity 及 static storage、immortal object、initialization unit、type registration、safepoint registration、callable registration 六组 pointer/count。共有 reader 检查 coordinate、实体 identity、依赖关系和记录格式；后续阶段复用解析结果。启动入口只接收本节规定的 image pointer/count 和 root entry pointer，不复制 dependency、String、类型或初始化表。

六类registration record均内嵌固定`{ u32 linkage_kind; u32 zero; semantic_id[32]; odr_group_id[32]; odr_member_id[32]; definition_fingerprint[32] }`。外层record type把`semantic_id`分别解释为`PersistentStaticStorageId`、`PersistentImmortalObjectId`、`PersistentInitializationUnitId`、`PersistentExactTypeId`、`PersistentSafepointSiteId`或`PersistentCallableBodyId`；strong记录的ODR字段为零，ODR记录的group/member非零。static-storage record另按声明序含`scan_kind, initial_state_kind, writable_base, byte_size, allocation_extent, required_alignment, scan_program, scan_fingerprint, layout_fingerprint, initial_template byte span, initial_relocations*, initial_relocation_count`；child relocation record固定为`{ u64 pointer_offset; const ScoopImmortalObjectDescriptorV1 *target; }`。`byte_size`是logical payload size，`allocation_extent`必须恰为`max(byte_size, 1)`。immortal record含`object_start, object_size, required_alignment, type_registration*`；init record含`schedule, diagnostic_path, cell, storage registration*, failure-root registration*, initializer/ensure callable id与entry、startup gateway callable id/definition fingerprint/pointer`；type registration含`runtime_type_id, TypeDescriptor*, descriptor/layout fingerprint`；safepoint registration含`SafepointId, site role, root-pair count, owner callable id, normalized stackmap fingerprint`；callable registration含`body_definition_fingerprint, entry, context_keys*, context_key_count`；每个 Context key use 为 `{ exact_key[32], u64 *slot_cell }`，entry 的 function-pointer typedef只作地址identity，不能用该擦除类型发起调用。完整字段顺序与C spelling由共享`scoop_runtime_metadata_v1.h`及M23设计6.1锁定，不允许加可空尾字段模拟版本协商。

这些digest字段不各自定义临时算法：per-Cone source/layout/scan字段分别由typed DAG的`SourceSignature`/`Layout`/`Scan`节点写入；TypeDescriptor的`descriptor_fingerprint`与每个callable record的`body_definition_fingerprint`由对应atom的`ObjectDefinition`节点写入；root/Eager-init gateway fingerprint是同一gateway callable的同一个ObjectDefinition digest在另一patch site的逐byte镜像，必须等于其callable record中的body digest；Lazy-init gateway fingerprint是固定全零tagged encoding，不是graph slot；safepoint normalized字段由`StackmapRecord`写入；registration identity 中的 definition 字段由 strong record 的 `StrongRegistration` 或该 ODR registration member 自身的 `OdrDefinition` node 写入；image字段由`RuntimeImage`写入。strong registration精确使用M23设计3.4的`scoop-strong-registration-v1`公式：对应canonical record强制Strong linkage、零ODR group/member并只把own definition slot归零，其他声明的上游digest保留，direct input按`DigestKind tag + DigestNodeId`排序编码。`LirDefinition`与`ObjectSupport`可以只存在verifier index。每个object slot固定32 bytes、无relocation、初始为零且只有一个writer；同一node写入多个镜像slot时最终bytes必须一致。M23-7 的 ODR 摘要采用实现规范 2.13 的逐 member 算法，不把整组成员集合写入每条记录；旧 group digest owner 退役。该变化启用此前尚未发布的 ODR 生产，不改变 Strong 编码、metadata C 字段顺序、prefix 或实际 runtime C 调用约定。

M31 的最终 image 只列出本 Cone 被 program-link 选中的 Strong/ODR producer record，普通 external reference 不重复登记；table 仍为 record pointer span。输入 artifact 保存候选表与物理归属，最终 descriptor 和表由 program-link 在选择后生成；runtime 不按 archive 名称、扩展名或顺序发现 image。六类 semantic id 分别建立 id/record-address 索引，callable 建立 body-id/entry 双向映射；entry 非空、位于 executable segment，不同 body 不得共用 entry。同一 ODR 登记只保留实际 provider 的记录，独立 helper 允许来自不同 image。所选 callable 集合完整覆盖实际保留的 Scoop body、NoGC body、adapter/trampoline 与 root/init gateway；无 safepoint 的 body 也须登记。普通 native extern、runtime C body 和其他 native producer 仍按其自身对象/ABI 合同消费，不伪造 Scoop callable registration。

safepoint还必须独立建立非零`SafepointId <-> PersistentSafepointSiteId`双向map；这里与上一段runtime type一样要求u64/full key全程序一一对应。不同full site key映射到同一u64时，即使owner、ODR group或return PC不同，也必须在用该ID匹配raw stackmap record之前fatal，不能让后续PC判等消歧。

既有 image 与 registration 的 pointer/count pair 即使 count 为 0 也指向类型正确的 addressable sentinel；M27 新增的 `context_keys/context_key_count` 使用明确的空分支，count 为 0 时 pointer 必须为 null；消费边界对非零 span 验证其 count 与实际 section/segment 范围一致、乘加不溢出、alignment、non-null及所在loaded segment权限。v1没有per-Cone code-range字段；entry/gateway pointer必须经callable map解析为声明的body、位于最终程序任一executable segment，并且该callable record出现在要求的producer image表（strong）或同一已验证ODR owner闭包的producer表中，不能声称验证一个不存在的“Cone text range”。storage的整个allocation range必须落在writable segment，descriptor/coordinate/scan/initial-template/relocation metadata必须为只读。零尺寸storage要求`scan_kind == None`和canonical空scan，但仍通过独立1-byte writable identity token提供non-null地址；除已验证的同一ODR member外，任意两个storage allocation range不得重叠。

static初值的C tag封闭为`ZeroedForRuntimeUnit=1`与`EncodedStaticValue=2`。Zeroed分支要求template为长度0的typed sentinel、relocation count为0及typed sentinel，并在registry commit与任一managed代码前逐byte验证完整`allocation_extent`为零；只有initialization-unit的ordinary/delegate/published storage、init/root-entry failure storage可用该分支。不能以undef、未初始化BSS假设或“initializer很快会运行”代替。Encoded分支要求没有initialization/root-entry descriptor把它作为storage/failure slot，`initial_template.length == allocation_extent`且template range只读。producer/reader检查每个padding、ZST token byte和Recursive managed-pointer leaf均为零，并检查实际storage的非pointer-leaf初始bytes与template一致；runtime只检查加载后的范围、实际引用与GC关系。即使Encoded template恰好全零也不得改用Zeroed tag。

runtime先收集并验证全部immortal-object记录，再验证Encoded relocation sequence。记录按`pointer_offset`严格递增且无重复；offset须8-byte对齐、checked落在logical `byte_size`内并恰好命中Recursive scan展开出的一个leaf。`scan_kind == None`禁止任何relocation，storage与template内容一致性由producer/reader检查；`scan_kind == Recursive`要求每个实际非null leaf恰有一项record、每项record也恰对应该leaf，record target必须解析到本程序已登记且位于只读segment的immutable immortal object，leaf bits须逐bit等于其精确`object_start`。其余leaf必须为null；immortal interior pointer、未登记静态地址及任意heap ref一律fatal。link/object verifier还要求writable atom在每个非null leaf恰有一条exact-width、zero-addend、指向同一typed immortal object-start的relocation，且child record的target pointer relocation指向同一immortal registration；链接对象读取边界验证初值内容与 relocation；runtime 接入时检查实际地址范围和 GC 根关系，复用已验证且未变化的初值、类型及 scan，不再次完整重放静态内容。显式raw `@Global`/`@ThreadLocal`与只读immortal object不属于这两个storage初值分支。

`RuntimeImageFingerprint`的canonical record不是raw struct bytes，而是封闭`StaticStorage | ImmortalObject | InitializationUnit | TypeRegistration | SafepointRegistration | CallableRegistration` sum。最外层sum tag就是record kind的唯一编码；每个variant payload先编码不含kind的`{linkage, semantic id, ODR group/member-or-zero, definition fingerprint}`，再依次编码：storage的scan kind、`OwnStorageAtom`、logical size/allocation extent/alignment、`EmptyScan | ScanProgram(scan fingerprint)`、scan/layout fingerprint，以及`ZeroedForRuntimeUnit | EncodedStaticValue { allocation-extent template byte span, sequence<{pointer offset, target PersistentImmortalObjectId}> }`初值sum；immortal的`OwnImmortalAtom`、size/alignment与target exact type registration id；init的schedule、diagnostic path bytes、`OwnInitializationCell`、storage/failure storage id、initializer/ensure callable id及其`CallableEntry` role、`None | UnitGateway(startup gateway body id, gateway fingerprint)`；type的runtime type id、`TypeDescriptorAtom(exact type id)`、descriptor/layout fingerprint；safepoint的64-bit id、site role、root-pair count、owner callable id与normalized stackmap fingerprint；callable的body definition fingerprint与`OwnCallableEntry(body id)`。canonical encoder统一使用`u32/u64`小端、32-byte id/digest原字节、`u64 length + bytes` byte span、`u64 count + elements` sequence、无padding的声明序product和`u32 tag + payload` sum。固定tag为：record kind按上述顺序为1..6；linkage `Strong=1, Odr=2`；scan `None=0, Recursive=1`；static initial state `ZeroedForRuntimeUnit=1, EncodedStaticValue=2`；schedule `Eager=1, Lazy=2`；scan-program choice `Empty=0, Program=1`；gateway choice `None=0, Unit=1`；typed role `OwnStorageAtom=1, OwnImmortalAtom=2, OwnInitializationCell=3, CallableEntry=4, UnitGateway=5, TypeDescriptorAtom=6, RootGateway=7, OwnCallableEntry=8`；site role `ManagedPoll=1, ManagedCall=2, ManagedInvoke=3, NativeSafeTransition=4, NativeBorrowedTransition=5`。Encoded canonical bytes内联template内容并把每个C target pointer替换为typed immortal semantic id；template/relocation-array地址、target record地址和storage中ASLR后的pointer bits均不进入key。每张表按`(semantic id, linkage, ODR group, ODR member)`byte序严格递增并分别编码count，direct dependency identity也按digest bytes递增、去重并编码count；unknown/reserved tag一律拒绝。StrongRegistration fingerprint复用own-definition置零后的这份完整sum bytes，不在前面再写第二份record-kind tag。

M24起每个machine body的persistent identity为`SHA-256(ByteSpan("scoop-callable-body-v2") || canonical(CallableBodyKey))`。`CallableBodyKey`使用`u32`小端tag与声明序字段：`Strong=1 { owner }`、`Odr=2 { member }`、`RootGateway=3 { root_cone, main_body_id }`、`InitializationStartupGateway=4 { unit_id }`、`ReleaseHook=5 { owner_exact_type_id }`；unknown tag拒绝。M23的v1 domain及四variant artifact随runtime ABI/schema不兼容而整体重建，不能在同一程序混用。main body、root gateway、initializer、ensure、startup gateway与release hook因此都是不同body。root/init gateway、release hook及每个safepoint的owner都必须解析到callable registration；runtime可由type registration的`PersistentExactTypeId`、v2 domain与tag 5重算该TD唯一允许的release-hook body id，再要求descriptor函数地址逐bit等于该callable entry。所有raw function address只用于验证/调用，不进入canonical identity或hash。

`normalized_stackmap_fingerprint`也不能hash raw LLVM section slice。每个已匹配registration的record规范化为声明序`{format_version=3, site_id, safepoint_id:u64, owner_callable_id, site_role:u32, root_pair_count:u32, instruction_offset:u32, stack_size:u64, locations, live_outs}`；location是`Register=1 {size:u32, dwarf_register:u32}`、`Direct=2 {size, dwarf_register, signed_offset_bits:u64}`、`Indirect=3 {size, dwarf_register, signed_offset_bits:u64}`或`Constant=4 {size, value_bits:u64}`，live-out为`{dwarf_register:u32, size:u32}`。raw顺序和两个sequence count都保留；i32 offset/constant先符号扩展为i64 two's-complement bits，`ConstantIndex`必须有效并用pool原始u64值规范化成同一个Constant tag，因此pool index、pool顺序及未引用entry不进入hash。Register offset、Constant register、reserved/flags/padding必须为零，unknown kind拒绝。fingerprint精确为`SHA-256(ByteSpan("scoop-stackmap-record-v1") || canonical(record))`。function address经`owner_callable_id`的callable map取得并必须逐bit等于raw function table address；该ASLR地址及`return_pc`不入hash，但`instruction_offset`与`stack_size`进入。当前Darwin/AArch64与Linux/amd64 profile另要求location数为`3 + 2 * root_pair_count`、前三项是8-byte Constant且每对root是相同的8-byte SP/FP Indirect可写slot；registration中的site/body/role/root count逐项相等。

producer/reader把canonical key中的pointer替换为上述own atom/scan/cross-registration/callable role，绝不输入ASLR地址：跨record pointer必须解析到目标kind-specific registration，scan内容必须与fingerprint一致；link verifier另用relocation证明pointer指向typed symbol，runtime重验地址映射、segment/producer关系及ODR duplicate相等。TypeDescriptor内部pointer语义由descriptor fingerprint承诺，runtime只作shape、scan和已登记type relation的防御性检查。RuntimeImage hash stream依次编码`ByteSpan("scoop-runtime-image-v1")`、runtime ABI、target profile、canonical Cone record、sorted direct dependencies及按record-kind顺序的六个独立table sequence；自身slot、任意raw link-object或embedded-blob bytes、ASLR地址与尚未定义的 program 级数据排除。typed digest DAG要求runtime-image node显式依赖record key中出现的definition/layout/scan/descriptor/gateway/callable/stackmap节点；object内容本身由逐成员code/artifact fingerprint和link verifier覆盖，不因一个`.slib`恰好含几个object而改变RuntimeImage算法。

普通内容 fingerprint 用于产物一致性和缓存失效，不承担 String 资格或完整程序来源证明。删除没有生产用途的 program/core binding key、Graph fingerprint 编码及逐层反向重放要求；编译、Link 和 runtime 各自只验证当前消费边界需要的格式、符号、ABI、地址范围与 GC 契约。

M31 的 callable/safepoint registration 继续保存真实 linkage、group/member、body/site owner 及各自实现所需字段；公共 definition 字段和 OdrDefinition/StrongRegistration 节点按本节 ABI 5 删除。RuntimeImage 根据本次选择后的实际记录及保留的 body/stackmap 等内容计算，不能把不同实现的登记混在一起，也不将全部记录伪装成 Strong。各记录的 typed identity 和物理入口关系在原读取及加载边界检查。

M31 的 safepoint registration 只包含普通 LLVM 优化后实际保留的站点。普通 call/poll 的 root-pair count 来自 RS4GC 前最终 managed SSA 等值分组：多个逻辑 leaf 可共享同一物理根，已知 null/不可移动常量不产生 relocation pair。LIR 逻辑 live-set 仍完整；编译器按实现规范 2.19 在 RS4GC 前后核对 typed leaf 分组，runtime 继续按这份实际登记与 stackmap 检查可写 slot，不从源级 leaf 数量推定机器根数。invoke 与 native transition 仍是零 gc-live，分别使用已有 compiler/caller root frame。

root entry不是裸Scoop function pointer。`ScoopRootEntryDescriptorV1`保存root Cone、persistent main body、ordinary `() -> Unit` source signature fingerprint、由root image producer表拥有的已登记failure root，以及gateway body id/fingerprint和精确`uint32_t(void)` C-callable gateway。gateway ID 由 producer/reader 从 `RootGateway { root_cone, main_body_id }` 推导，runtime 核对其实际登记引用，pointer逐bit等于该body的callable registration entry，两个位置的ObjectDefinition digest逐byte相等。gateway以自己的body identity登记内部managed call/异常路径全部safepoint，再调用namespaced main：成功返回0；未捕获异常必须在 generated landing pad 内使用已有 MaterializeException 物化为 managed 对象，发布 failure root 并结束 native catch 后返回 1；不能在 failure slot 保存 BeginCatch 的 native payload 地址；其他值fatal。

init schedule在IR中是封闭`EagerStartup { gateway } | LazyAccess`。Eager 的 startup gateway body ID 由 producer/reader 从 `InitializationStartupGateway { unit_id }` 推导，fingerprint非零且等于对应callable body digest，pointer逐bit等于该entry；Lazy的gateway id/fingerprint全零且pointer为null。两种schedule的cell、storage/failure registration及initializer/ensure body id/entry都必须non-null/nonzero并由当前strong producer或unit同一ODR group拥有；initializer/ensure pointer逐bit等于各自callable registration entry。eager init只能由no-throw startup gateway从C调用，普通initializer/ensure entry仅供generated managed代码；lazy访问仍经普通ensure抛向Scoop caller。ensure已在自身catch中发布managed failure并将unit转为Failed；eager gateway只平衡外层catch并返回1，不能再次物化或覆盖失败对象。status为1却没有Failed状态或有效failure root属于runtime invariant error。由此任何startup异常都不会跨C ABI frame。

主线程attach后处于native-safe且没有活动managed栈段。C startup coordinator对**每次**init/root gateway调用都执行独立native→managed transition wrapper：在本次wrapper栈底push `NativeToManagedBoundary`，LIFO保存previous mode/boundary，发布封闭`EntryPending`状态（gateway尚未执行，因此段内没有managed frame）；在既有thread registry mutex保护下协调managed mode发布与3.5的GC phase/epoch检查，需要时按空的新栈段状态park，外层frozen segment与native roots仍须扫描，不能把C PC作为managed anchor。compiler 根据既有 RootGateway/InitializationStartupGateway body kind 强制入口 poll，以自己的 body/site id 发布准确 anchor，并将段切为`ActiveManagedSegment`，随后才执行普通managed body。walker必须在本次boundary停止，不扫描C coordinator。root gateway在managed段内完成异常物化、failure-root发布及EndCatch，eager gateway复用ensure已发布的failure；两者只返回GC-free status；wrapper返回后以release语义发布native-safe，按同一epoch/snapshot协议移除boundary并恢复外层记录，collector可能读取的record不可提前销毁。每个eager调用和root调用各自enter/leave，不能用attach时的一条boundary覆盖多个调用；嵌套callback继续独立建段并LIFO恢复。

init `failure_root`与root-entry `failure_root`是封闭role的专用static storage，不是任意合法root：当前profile精确要求8-byte logical/allocation size、8-byte alignment、`Recursive`、canonical `References { offsets:[0] }`（word program `[1,0]`）、`StaticInitialState::ZeroedForRuntimeUnit`，并在任何managed代码前保存全零null ref且没有template/relocation payload。它不能兼作property/published/ZST token或不同unit/root的failure slot；只有同一已验证ODR identity可以重复引用。init `cell`固定size 16/alignment 8，必须位于writable segment并初始恰为`{state=Uninitialized(0), owner_thread=null}`；不同unit的cell互不重叠，也不与任何storage range重叠。普通init storage的persistent key必须绑定该unit及property/published role，不得与另一unit共享registration或range，也不能等于本unit failure root。runtime在coordinator或gateway可能写入前建立并验证`cell address -> unit`与`storage id/address -> unit`反向map。

gateway返回后，native reporter只保存稳定的failure slot地址；在既有world/thread mutex保证GC不会移动对象的短临界区中读取该slot并取得只读TD与diagnostic span，随后停止访问managed对象。不得把异常地址缓存到native局部后跨等待或handshake使用。报告输出已有类型名及必要的startup unit path并abort，不调用用户toString、属性或reflection。

String 由前端解析为实际 typed class，MIR/LIR 与 Link 使用同一 provider 的 exact type、TypeDescriptor、registration 和 relocation。现有 C ABI 的 `scoop_td_String` 表示 runtime 所需的 String descriptor 地址；链接使用实际声明的定义，不从固定 CORE identity 重建它，也不按名称或同布局替换类型。对象头为 16 bytes，length offset 为 16，inline bytes offset/minimum 为 24，alignment 为 8；`InlineBytes` 的 size/stride/alignment 为 1，object/inline scan 为空。保留这些真实表示和边界检查，不单列重复的 String 布局/scan 副本、capability-kind digest 或 program/core binding 证明。

后续 program-link 将保活实际 image 集合；当前每个 image 已强引用其六类 metadata producer table。M23的Cone image是最终可执行文件内的logical image，不是独立Mach-O/dylib；不支持运行期静态metadata注册、`dlopen`、卸载或global destructor。Darwin final-link 将对象的 `__LLVM_STACKMAPS,__llvm_stackmaps` 合并映射到 `__DATA_CONST,__llvm_stackmaps`（`-rename_section`），由 dyld 在只读数据段完成函数地址修正后保护为只读；直接把原段的初始权限改为只读会被 linker 作为 text relocation 拒绝。含指针的不可变 metadata 使用 `__DATA_CONST,__const`。runtime 仍以当前 VM protection 验证，不能从段名推定权限。Darwin linker会把各object的`__LLVM_STACKMAPS,__llvm_stackmaps`串接成多个完整v3 blob而非合成一个header；parser必须逐blob按count/对齐算出精确消费长度并解析至section末尾。ODR重复site只在full site/body owner/group/member、最终链接/ASLR relocation后的**原始return PC**、canonical location/live-out payload与fingerprint全等时去重，不做`PC-4`、nearest-symbol或range修正。M23 Darwin final-link profile禁止`-dead_strip`，因为descriptor引用不能保活无符号stackmap section；缺失任一 safepoint registration 对应 raw record，或 raw record 缺少实际 site/callable owner 时在 startup 前 fatal；已登记 NoGC callable 可以没有 safepoint。

上述去重不放宽固定宽度碰撞门禁：非ODR或不同full site key复用同一`SafepointId`或原始return PC均fatal。

---

## 3. GC 契约

参考实现由 M9 的单代非移动 Immix 和 M13 的多 mutator STW 演进而来；M15～M30 的已实现基线为单代、STW、单线程 collector 的 moving Immix。M31 设计在相同 arena 中增加 nursery，并以 minor GC 晋升到 Immix 旧代，见 3.9；仍采用 STW、单线程 collector 和多个 mutator。root、safepoint、native 借用、pin、handle 与 release hook 的语义同时约束 minor/full GC。现有 Darwin/AArch64、Linux/amd64 glibc 和 musl 使用精确 frame/location adapter，不允许保守扫描回退。

### 3.1 分配入口

分配分为两条通道：

- **managed 快速通道**：编译器生成的代码**不使用 handle**，直接取得裸指针。标准形态是 TLAB 碰撞指针（bump pointer）内联序列；成功后由不含safepoint、不取得heap/world锁的GC-leaf `scoop_runtime_finish_tlab_alloc`完成清零、对象头与object-start/精确allocation-size side metadata登记，缓冲区耗尽时才落入 slow path（`scoop_runtime_alloc_slow`，可能触发 GC）。编译器必须在 safepoint保留完整root信息，包括post-site live leaf与普通managed call的可移动实参leaf；M15 moving collector精确消费statepoint stack map并更新其location，生成代码在调用后只使用`gc.relocate`结果（impl spec 2.4/2.5）。managed 分配是最高频操作，其成本必须保持摊销 O(1)，不经 handle 表。
- **Scoop ABI native 通道**：外部实现没有 stack map；它可以直接借用传入的 managed ref，但在调用可能触发 GC 的 runtime入口前，必须先把仍需使用的引用登记为 native root slot（见 4.2）。需要把新对象长期带出 native frame时可使用 handle；需要稳定裸地址时使用 pinned allocation。

本章未细分时出现的“native root”“root slot（链）”只是4.2封闭root frame链的简称，不表示只支持direct slot：直接ref使用`DirectSlots`，含managed leaf的内联value/aggregate必须使用`RecursiveRegion`。collector及每个可能GC的native-borrowed入口都必须穷尽匹配两种variant。

M13 起每个已attach线程持有独立TLAB；slow path在同步的heap元数据下从Immix free-line run或新block切出互不重叠区间。STW开始后全部TLAB失效，GC结束后各线程在下一次分配时重新refill。内联bump与上述GC-leaf finish helper共同构成fast path，并必须在返回前清零完整对象、初始化对象头，并以原子方式同时登记object-start与精确normalized allocation size，保证多mutator分配与构造中途safepoint都可安全扫描。只登记start而没有size不是合法的已发布对象状态。TLAB具体尺寸可调，但managed分配返回裸指针、摊销O(1)、不经handle表的契约不变。

### 3.2 safepoint 模型

- managed（编译器生成）代码：statepoint poll（impl spec 2.4/2.5）。函数入口及每条循环回边在LIR中已经是带完备root plan的显式poll，不由codegen补插；M22的普通循环尾与continue edge都必须经过对应header poll。managed ref在LLVM中使用address space 1，普通poll/call的post-site live leaf及可移动实参leaf由statepoint/`gc.relocate`描述。含ref aggregate必须先拆为独立leaf，不能把aggregate alloca当作隐式stack region；
- managed `invoke`：LLVM当前异常边relocation不能作为语言实现基础。编译器在invoke前把normal/unwind后继仍活跃的ref leaf及可移动实参leaf写入独立compiler root frame，每项携带normal/unwind reload角色；codegen直接发射零`gc-live`的显式statepoint invoke，两个后继只从各自所需slot reload并在继续控制流前pop，不得产生exceptional `gc.relocate`；
- outbound native transition：native-safe/native-borrowed调用前发布只描述transition顶层generated frame的完备caller-root frame，并以唯一`SafepointId`把transition入口发射为零`gc-live`/relocate statepoint；平台薄入口在C prologue前捕获该record的PC/SP/FP。transition入口本身可能park并使root被改写，所以入口返回后必须先从caller-root storage重新构造含ref实参，不能把入口前求值的AS1 SSA值传入native callee。顶层frame只从caller-root storage reload，冻结segment的外层managed frame从该anchor按精确stack map扫描；实际native machine call为nounwind普通调用，不跨native frame反向unwind。返回值含ref时使用调用前已清零并登记的result storage，在native-borrowed返回后、leave可能park前写入，回到managed后连同其他顶层live root只从slot reload；
- Scoop ABI FFI：函数体内**无** safepoint poll，GC 只能在其显式调用 runtime或回调 managed代码时于该入口内部发生；跨越这些入口的 direct ref必须位于 native root slot（spec 14.3）；
- C ABI callee不接收 managed ref，也不得直接调用 Scoop GC；M12 单 mutator实现可把它视为纯 GC leaf。M13启用多mutator后，outbound caller必须在调用前发布live roots并进入native-safe，使其他线程发起的STW GC无需等待一个可能阻塞的C调用。C代码若持有4.3注册所得的静态trampoline与cookie，可以经这个**独立反向边界**进入managed callback；这不使原C callee获得direct ref或Scoop ABI能力，外层caller roots继续由native-safe transition保活。

M15的macOS/AArch64 runtime只更新stack-resident managed roots。固定的LLVM 22.1 backend profile通过SelectionDAG默认spill与post-RA statepoint fixup保证每个GC base/derived location都是可写的8-byte `Indirect [SP/FP + offset]`；runtime不保存、unwind或改写register root。stackmap前三个constant location、deopt location与live-out不属于GC root pair；通用parser必须能区分这些区域，DarwinAArch64 profile只在GC root位置拒绝`Register`/`Direct`/constant等非约定形态。

M28 的 Linux/amd64 继续采用上述 stack-only root 规则，DWARF SP/FP 分别为 7/6。入口汇编从尚未修改的 `[RSP]` 读取精确 return PC，以 `RSP+8` 发布 caller 的 callsite SP，以 RBP 发布 FP，追加到 C implementation 参数后 tail-jump；不能把入口自己的 return-address 槽算进 caller SP。LLVM stackmap 的 `stack_size=N` 不含 caller return-address 槽：`FP=SP+N-8`，`[FP]` 是 saved RBP，`[FP+8]` 是 return PC，上一帧的 callsite SP 为 `FP+16`。仅接受位于 `[SP, FP)` 的完整 8-byte root slot，不把 frame record 当 root；base/derived 必须相同。generated 函数固定 frame pointer、禁用 red zone 和 tail call；精确 GC 保持原 return PC，不应用异常 call-site 的 PC 调整。

stackmap 通用 decoder 只检查非零、已知的固定 frame size；对齐属于 frame adapter。Darwin/AArch64 要求 `N % 16 == 0`，Linux/amd64 要求 `N % 16 == 8`，后者与调用点 SP 的 16-byte 对齐一致。不得在共有 v3 decoder 中硬编码 AArch64 的 frame-size 余数。

### 3.3 根集合

- M27 的线程 Context root：`ScoopThreadState.current_task_context` 是可回写 managed slot，在 native-safe、挂起驱动退出及 gateway 之间仍被扫描。切换时保存的 previous Context、scope 保存的旧 binding root、coroutine frame 和 callback 快照同样使用本章已有的精确 root/RefScan；具体契约见第 9 章。
- 栈根：managed caller在statepoint处的live root location、managed invoke显式compiler root frame、Scoop ABI调用点保持的caller roots，以及native callee显式登记的可更新root slot链。M19的initializing receiver从class allocation返回起就是普通managed root，跨base/`this` initializer、property/`init` managed call与异常边时按同一规则relocate；不存在constructor专用handle或不可移动区。runtime以精确return PC查stack-map record并把每个location解析为可写slot；不存在保守栈扫描fallback；
- 静态storage root：ordinary top-level property/delegate storage、object/companion published singleton slot、root entry及initialization failure slot。LIR/codegen为每个compiler-owned static storage输出2.8的typed descriptor、非可选`scan_kind`与封闭`StaticInitialState`；`ZeroedForRuntimeUnit`在任一managed代码前使完整extent为零，`EncodedStaticValue`则只允许Recursive leaf为null或经typed relocation指向已登记immutable object-start，两者都在registry commit后携带完备scan登记为可写root。`None`明确表示GC-free且禁止relocation，只参与storage/init identity验证。零尺寸payload强制为`None`，但仍拥有独立的1-byte writable identity token；该token在两种variant中的canonical byte均为零，GC绝不扫描。spec 13.6显式`@Global`/`@ThreadLocal` raw storage仍是另一种FFI representation；
- stable/immortal对象：活跃的Scoop exception record中按值复制的对象payload作为动态stable external object登记，其内部ref slot可更新；编译器生成的静态String等只读immortal object必须有精确地址/size/TD登记。M15只允许GC-free payload的只读immortal对象；未登记的heap外地址不能出现在managed slot；
- handle 表与 pinned 对象（`GcHandle` 保活其引用对象；pinned 对象作为根被扫描，见 3.4）。

M23以后不再引用固定`scoop_image_*`符号；runtime 只遍历 2.8 启动参数强引用的实际 image 集合。static-storage/immortal/type/init/safepoint/callable全部使用2.8带prefix、typed registration identity及kind-specific字段的v1 record；不能再接受旧`{base, scan}`、`{start, size, td}`by-value table、裸TypeDescriptor/SafepointId span或未经登记的function address。count是唯一权威，空span使用addressable sentinel。runtime必须先对全部image完成DAG、table、range、TD与callable双向identity、ODR与duplicate验证并建立static metadata registry，才初始化GC heap或执行任一managed initializer；不能验证一个image后立即运行其副作用。

普通不同 storage/unit semantic id 必须全程序唯一；不同 identity 的 storage allocation range 与 immortal range 分别不得重叠，TD/header 必须匹配，只读 immortal object 不得含 managed 出站引用。M31 只登记 program-link 已选择的定义，仍检查 record variant、typed identity、owner、实际 base/entry/TD/cell、size/alignment、scan、初始化引用与 stackmap 的一致关系。若 image 中存在同一 ODR record 的重复引用，只能复用同一记录地址及其已经验证的内容；同 id 对应不同地址、不同 body 对应同 entry 或存储范围重叠仍是错误。不得以跨 producer 的 definition fingerprint 或 stackmap 内容相同作为 ODR 前提。

### 3.4 保活机制（两级）

- **pin（对象头标志）**：保活且阻止移动。pin 标志位于对象头，当前64位源码surface上的`PinnedPtr.raw: ULong`即对象地址（spec 14.1），pin/unpin 是 O(1) 的对象头读写，**不经 handle 表**。带 pin 标志的对象作为 GC 根被扫描（其出站引用必须被追踪），但自身不移动。这个源码`ULong`是本版本的临时carrier，后续与native integer的归属关系另行决定；不允许runtime将任意`ULong`当作已验证的pinned object地址。
- **`GcHandle`（handle 表）**：保活，不阻止移动。handle 表是 runtime 私有结构；64位host使用带generation的`(generation, slot)` opaque编码，0保留为null niche，slot复用时generation递增，避免已释放handle错误命中新对象。源码`GcHandle.raw: ULong`只是该固定64位opaque token的ABI carrier；generation、slot与live state仍是runtime内部typed metadata，不获得源码`ULong`算术语义。GC 移动对象后负责更新表项。

只在一次 Scoop ABI 调用内、且不跨 safepoint使用的 direct ref不需要 pin/handle。需要稳定裸地址时优先 pin；需要长期保活且允许移动时使用 `GcHandle`；仅需跨 native callee内的 safepoint时优先使用 native root slot，避免改变对象移动属性或建立长期 handle。

### 3.5 线程

线程创建/销毁时向 GC 注册/注销。M13建立的多 mutator协议使用带单调GC epoch的合作式 stop-the-world handshake，并至少区分 managed、native-safe、native-borrowed、parked与collector状态：managed线程在入口/回边 poll或managed runtime入口发布平台定义的top managed anchor后停顿；C ABI outbound call发布顶层caller roots并冻结当前managed栈段后进入 native-safe，collector无需等待其返回；持有 direct ref的 Scoop ABI native code属于 native-borrowed，只在登记 native roots的显式 runtime/managed入口或返回边界参与协调。每次managed/native重入都在LIFO transition链中划分managed栈段。活动managed段从top anchor按精确stack map walk；每个冻结段的顶层frame由caller-root storage表示，外层frame从进入native前由零root transition statepoint捕获的anchor继续精确walk到该段boundary。collector不扫描native stack，也不从当前native PC或C frame反向猜测managed入口。collector 不得在未握手的普通 native-borrowed指令之间移动对象或扫描仍在变化的native栈。进入协调点时，当前线程anchor、transition anchor、compiler caller-root frame与native root slot链必须可扫描、可更新。compiler caller-root entry由value storage地址和递归scan descriptor组成；tagged enum的固定ref偏移可直接扫描，因为inactive variant slot按spec 7.4恒为全0。含ref的Scoop ABI返回storage从push前的全零值开始登记，在native返回后、leave可能park前写入结果，并在恢复managed后reload。

进入managed、离开native-safe或离开native-borrowed必须与epoch发布形成无丢失握手：转换方发布mode后复查phase/epoch，观察到`Stopping`或epoch变化时在执行第一条managed指令前park；collector先release发布`Stopping`再以acquire读取线程mode。不得把线程按native-safe计为quiescent后又允许它未经复查进入managed。

M23的C startup coordinator遵守2.8逐次gateway transition协议。attach本身不构成永久managed段；首次managed指令前的`EntryPending`是已证明为空的新段，允许epoch握手但不允许扫描native frame。gateway首次poll建立自己的精确anchor后才转为活动managed段，返回时先退出该段再继续native-safe coordinator。这个空段优化只用于无managed输入的root/eager gateway。现有callback wrapper进入后需要解析closure handle并发布native roots，仍以活动managed段参与握手，collector等待其真实poll；不能把已携带managed参数的callback准备阶段当成空段跳过。native transition冻结外层段，callback的进入/恢复继续使用同一mode、boundary和深度链。

foreign thread在进入任何 managed代码前必须 attach，建立 TLS thread state、栈边界、TLAB和空 native-root链；离开最后一个 managed callback后由拥有本次 attachment的入口 detach。重复使用的长期 foreign thread可以显式保持 attachment，但不得在 runtime shutdown后重新进入。

Linux OS 组件通过当前线程的 `pthread_getattr_np`/`pthread_attr_getstack` 获取实际栈范围，并通过 `mmap`/`mprotect` 提供同一 VM 操作。glibc 与 musl 复用该组件，寄存器/帧规则属于 architecture/ABI 组件；后续 Linux arm64 不复制 Linux OS 层。musl 主线程栈范围可能随实际调用深度增长，不能永久冻结 attach 时的范围；当当前线程发布的 boundary/anchor/native transition 超出已知范围时重新查询 OS，确认新范围涵盖所需地址及仍活跃的旧段后，在现有 world/park 同步边界发布。collector 只读取线程已发布的快照，不替其他线程查询栈，也不在 parked 期间修改其范围。

M15的platform bundle由object-image、OS thread/VM与architecture/ABI frame三个完备组件组成；GC arena reservation、page protection与线程栈边界都只经OS/VM组件，通用heap/collector不直接调用`mmap`/`mprotect`。通用runtime只经该bundle把top anchor与每一帧return PC/stack-map location转换为可写root slot，不读取Mach-O/x29等平台细节。macOS/AArch64基线强制generated/runtime frame pointer、禁止managed tail call，并要求return PC原值精确命中record；不得使用`PC-4`、最近函数、symbol或地址范围猜测。任何缺失record、越界location、未知DWARF register或当前target不支持的GC-root location kind都是fatal metadata错误；生产编译器应已在object验证阶段把这种情况报告为compiler/toolchain invariant failure，runtime检查只防御错误链接、损坏或非Scoop产物。每个由generated managed code直接调用且可park/collect的managed runtime symbol必须通过薄入口先发布`{ return_pc, callsite_sp, frame_pointer }` opaque anchor，再调用平台无关实现；runtime内部不得重入该入口并把C frame冒充managed frame。Scoop ABI native实现调用的是另一组native-borrowed入口：它验证已发布的caller/native roots并参与握手，但不捕获C caller或伪造managed anchor。

### 3.6 屏障

M30 的单代 collector 尚不消费 card table；M31 的 minor GC 必须消费旧代脏卡，保证每个 old→young 引用可被找到。向 managed heap 写入可能含引用的数据后，必须在下一个可 park/collect 边界或向其他线程发布该对象前，标记实际被写引用槽所在的 card；连续 aggregate/array copy 标记整个可能含引用的写入范围，不能只标对象头或范围起点。普通 scalar、GC-free value 和零长度写入不产生屏障。首版允许保守标记 young 目标或不含 young 值的引用写入，collector 仅消费旧代脏区；不要求热路径检查每个 RHS 的代龄。

编译器的字段、含引用 value/enum/array 写入，runtime 的 Context、array/box clone/copy，以及 Scoop ABI native 的 managed heap 引用写入遵守同一范围语义。native 提供一个不分配、不 GC、不 park 的范围屏障入口；root registration、pin 或“刚分配”均不能替代它。构造器和复制操作中途可发生 GC，只有现有控制流能证明写入前目标一直在 nursery 且尚未发布时，才可省略该次屏障。

范围入口为 `void scoop_rt_gc_write_barrier(const void *destination, size_t bytes)`：写入完成后调用，非空范围必须完整落在现有 arena 内；零长度直接返回且不访问地址。入口以 relaxed atomic OR 标记范围内每张 512-byte card，不分配、不获取 heap lock、不参与 world handshake。编译器单个引用槽保持内联 monotonic OR，含多个引用槽的完整 value store 可保守标记首末引用之间的连续范围。此分配与屏障批次将 runtime ABI contract 6 升为 7，旧产物重建；其公共函数机器签名进入普通 runtime symbol registry。

多个 mutator 标记同一卡必须使用 atomic monotonic store/RMW；Context 的普通 byte store 必须迁移。引用写入与标记之间不得插入 safepoint，屏障不能被优化移到可能 park 或发布之后。STW handshake 建立 collector 读取引用与脏位的可见性；collector 在所有 mutator 停止后消费和清理脏位。脏卡通过 object-start、精确 size 与 scan metadata 找到所有相交对象中的引用槽，正确处理跨 card/line 的对象及大数组；根表和 stable external region 仍直接按精确 root 扫描，不伪装为 heap card。

### 3.7 moving collection与side metadata

M15 collector为选中的from-space对象建立arena外forwarding关系，把未pin live object复制到独立to-space，再通过统一可改写slot visitor更新全部root和heap字段。普通模式可按block占用率选择source，但只要存在可移动live object且to-space足够，一次显式full collection至少选择一个eligible source，不能退化为永久不移动的mark/sweep；stress模式必须移动全部未pin live object。pinned对象不移动但其出站ref仍更新；`GcHandle`只更新table entry，generation/slot identity不变。

block state、object-start、每个对象的精确normalized allocation size、line/free-run信息与forwarding均位于GC arena外。block header、free-block node或hole node不得存放在可能poison/保护的arena内。可变长String/Array及large object的复制长度直接读取allocation metadata，不能从TypeDescriptor fixed size、block span或payload内容反推。

M31 的 normal collection 在实际 root/heap slot 遍历与回写时检查 forwarding 和引用合法性，不为再次证明刚完成的扫描额外全堆遍历；minor 尤其不得调用全旧代 verification walk。全堆交叉验证、逐分配强制 moving、旧地址 `0xA5` poison 与 `PROT_NONE` quarantine 用于现有 stress 或显式验证模式。minor stress 必须执行真实 nursery、remembered set 与晋升路径，不能被旧 full-stress allocator 的独立分配路径替代；full stress 继续覆盖全部未 pin live object 的移动。回收前必须完成所有实际存活引用的更新，evacuation allocation 不得递归触发 collection。

### 3.8 finalizer与资源释放

Scoop 永久不支持执行 managed 代码、观察对象图或允许对象复活的 GC finalizer。M24 只增加语言规范 9.1.6 的同步 GC-free release hook，详细落点与验收见 [M24 设计](../milestone24/DESIGN.md)。

语言和 IR 以 `None | SynchronousGcFree { hook }` 表示 policy。C metadata 最后将其编码为 nullable `void (*)(void *object_start)`；raw 参数仅在调用期间有效，源码不能取得它。thunk 按静态布局读取本 owner 的合格字段，只执行 release-safe 操作，不具有 GC strategy、statepoint、stackmap、EH/personality、managed root frame 或 thread transition。字段类型与 effect 在编译边界完成，runtime 不重新做语言类型或调用图分析。

**ready 与调用顺序：**

- object header 保持 16 bytes；`gc_word` 的 `GC_RELEASE_READY_BIT = UINT64_C(4)`（bit 2），pin bit 仍为 `UINT64_C(2)`（bit 1）。分配时 ready 为 0；构造最外层正常出口使用保留其他位的 atomic OR/release 置 1，且不产生 safepoint。异常出口、base/`this` initializer 和普通返回对象的函数不执行该操作。常量和顺序纳入 runtime ABI。
- 标记、pin 与其他 header 操作保留不属于自己的位。evacuation 复制完整 header/payload，并保留 to-space 的 ready。from-space 中 marked 且已搬迁的旧地址只失去物理存储，绝不调用 hook。
- reclaim 必须先用现有 mark/forwarding 状态区分 `LiveInPlace`、`RelocatedCopy` 和 `Dead`。只有 `Dead` 才读取 hook/ready 并进入调用分支；不能用“这段 storage 要丢弃”或“没有 keep”代替逻辑死亡。
- 对 Dead 对象，null hook 与 ready=0 直接回收；null hook 与 ready=1 是局部 header/metadata invariant error。非 null hook 使用 atomic AND/acq_rel 清除 ready，旧值为 0 就跳过，旧值为 1 则在 header 和 payload 完整可读时同步调用一次。ready 在调用前已经清除，不增加第二个 claimed 状态、注册表或队列。
- hook 正常返回后才能删除 object-start/size、poison、复用 line/block 或 quarantine/unmap。small/large object 的普通与 stress 路径共用这一逐对象操作；已搬迁副本的 poison 仍直接进行。没有逐块调用 hook、遗漏 dead object 或额外全堆扫描。

当前 collector 为单线程 STW。ready publish/claim 的 release/acquire 配对保证初始字段可见；构造后的字段写入继续使用语言数据竞争规则及 3.5 的 STW handshake。显式 close 在 park 前完成的 inert-state 写入必须对 hook 可见；hook 不为有数据竞争的程序补充同步语义。它清理的是逻辑对象的一次资源状态，不保证不同 owner 指向同一 native resource 时只释放一次。

**执行上下文与 native 契约：**

hook 在现有 collector 的 heap/root 锁仍被持有、world 未恢复时执行。不得释放这些锁来进入 ordinary managed/native transition，不得分配 managed 对象、发起 GC、访问 root/handle/pin/thread API、等待已停顿 mutator 或依赖其进展。纯 native 释放可以使用其自身的同步，但不能形成对这些运行时锁或停顿线程的等待环。

允许的 Scoop helper 复用定义方已推导、由普通接口携带的 NoGc/release-safe 正文；不在 reclaim 时遍历调用图。helper 中不能有 C extern 函数调用或任何 transition。release block 的直接 C ABI extern 使用 raw leaf call 或既有纯 storage bridge；同一 extern 在 ordinary caller 中仍使用 native-safe。native 实现不得 foreign unwind、回调任何 Scoop entry、重新进入 runtime 或保留临时地址。编译器检查可见的类型、effect、ABI 和引用；外部 C 实现履行这些契约是 unsafe FFI 的责任，runtime 不反汇编它或建立回调资格系统。

**验证边界与 best effort：**

启动 registry 只补充加载后的必要检查：非 null hook 属于 FixedObject descriptor，按 owner exact type 得到的 ReleaseHook body id 在已有 callable map 中存在，entry 地址相同且可执行。Strong/ODR 的归属和实际地址复用既有登记规则；source final/Throwable、body effect、静态 offset、对象 relocation 与内容摘要分别由前端、lowering、reader/linker 检查，runtime 不重建或重复验证它们。GC 热路径直接使用已登记 descriptor，仅执行死亡分类和 ready 操作。

best effort 不保证何时再次 GC、对象间顺序、执行线程、native release 成功或退出时调用；但正常 collection 一旦决定回收 ready 对象，就必须在其 storage 失效前尝试一次。进程 abort、native fault、foreign unwind 或不返回的调用不会得到重试或恢复保证。shutdown 直接沿既有协议销毁运行时状态，不追加 collection，不为 live/uncollected/unready 对象补调 hook。确定性释放使用显式 close/release 与 try/finally，显式路径先写 inert state 再释放资源；不增加 arm/disarm、手动触发、排序、重试、后台执行器或 finalizer。

---

### 3.9 M31 nursery 与 minor GC

nursery 使用现有 arena 中独立标记的一组 block，每个 mutator 的 TLAB 从 nursery 取得；代龄、object-start、精确 size、forwarding 与脏区索引都保存在 arena 外。它不增加 managed reference tag、对象头字段或 GC stackmap 格式。分配返回前完成清零、头部与精确 allocation metadata 发布，原 GC-leaf fast path 契约保持。

普通中小对象进入 nursery；能放入普通 block 的对象不能继续仅因超过旧 `GC_SMALL_MAX == 64` 就各占一个 large span。普通分配与旧代晋升均须支持跨 line 的中小对象、准确标记占用 line 和完整 size metadata。无法放入普通 block 的大对象直接进入旧代 large-object 路径。带 release hook 的对象首版直接在旧代分配，沿 3.8 的 ready/reclaim 规则释放；不能因重置 nursery 漏掉 hook。

M31 的普通 block 保持 32768 bytes，前 128 bytes 不用于对象；普通 allocation 上限为 32640 bytes，按 descriptor alignment 对齐后跨 line 连续 bump。精确大小使用 `uint16_t` 的 8-byte units，覆盖单 block 中每个合法对象；对象起点的发布与全部相交 line 的占用标记支持同一 block 中不同 mutator 的独立 free run。TLAB refill 选择能容纳当前请求的连续 run，不能消耗过短的空闲 run。evacuation 与后续晋升使用相同大小、对齐及 line 占用规则。

minor 在现有 STW 协议内执行：

1. 停止全部 mutator，废止全部 TLAB，沿 3.3 的完整精确根及旧代脏区追踪 nursery 可达图；根指向旧对象不意味着递归扫描整个旧图。
2. 为年轻存活对象安排晋升。第一次 minor 存活即进入旧代，不设置 survivor 区、年龄计数或多级晋升。空间规划成功后才发布 forwarding 和修改引用；不足时释放未使用的目标预留、清理工作标记，从仍完整的原图执行 full GC，不允许半搬迁后退回普通分配或递归 GC。
3. 未 pin 的年轻存活对象复制到旧代，统一更新根、旧代脏槽和年轻存活对象内的引用。含 pinned 对象的 nursery block 在 STW 中转为旧代，其中存活对象可原地保留、死亡对象删除；全部留下的存活引用同样更新。pin 返回后已暴露的地址绝不改变。
4. 完成后没有存活对象留在本轮 nursery；回收死亡对象与旧副本、重置可复用年轻 block，清理已经消费的卡。下一轮分配才建立新 TLAB。旧代已不可达对象允许保留至 full GC；minor 不执行旧代回收。

nursery 满触发 minor；旧代／large-object 压力、晋升空间不足或显式 `gc.collect()` 触发 full GC。full 沿现有 Immix mark/evacuate/reclaim 收集两代，其年轻存活对象也进入旧代；空间不足时可原地保留合适 block，不遗失引用、不绕过既有分配失败出口。显式 full 在存在 eligible movable object 且 to-space 足够时仍满足 3.7 的移动要求。nursery 大小是内部性能参数，不进入语言语义或通用资源预算。

首版按 block 保存 Young/Old，普通 nursery 的触发容量为 1 MiB；`SCOOP_GC_STRESS_MINOR=1` 将其缩为一个普通 block，仍走相同 TLAB/refill、脏卡与晋升路径。`SCOOP_GC_STRESS_MOVE=1` 继续使用原 full-moving/quarantine 模式；同时设置时 full stress 优先。带 hook 的小对象使用旧代 free run，generated fast path 只接受 null hook；此批 runtime ABI contract 7→8。minor 的空间规划和 full evacuation 共用真实对象大小/对齐的 to-space 预留，预留失败前不复制或发布 forwarding；normal full 无可用 to-space 时保留原位对象，full stress 仍要求移动全部未 pin 存活对象。

诊断统计记录实际分配字节、nursery 分配、晋升、minor/full 次数、晋升失败转 full、脏卡、扫描引用槽与 STW 停顿。`scoop_rt_gc_debug_metrics` 复制这些计数供测试读取；`SCOOP_GC_STATS=1` 在正常程序退出时向 stderr 输出一行 JSON，供性能脚本采集。这些计数不决定程序是否合法，不进入对象 ABI 或产物身份。

性能对照使用诊断开关 `SCOOP_GC_FULL_ONLY=1`，将 nursery refill 原本请求的 minor 改为普通 full；分配布局、nursery 容量、屏障和显式 full 路径保持相同。该开关默认关闭，不开启 full-moving stress 的 quarantine 或重型验证，也不改变语言接口。JSON 另记录 minor/full 的累计停顿和八档停顿直方图，上界依次为 10、50、100、500、1000、5000、10000 微秒及无上界；停顿从 mutator 停止后开始，包含本次 mark/plan/update/reclaim，未包含 rendezvous 等待。附加统计保存在 runtime 内部，不改变 `ScoopGcMetrics` 的 C 布局。

minor 的正常工作随根、脏区和年轻存活图增长，不能扫描所有旧代对象或所有旧代引用来弥补屏障遗漏。测试必须覆盖仅从 old→young 可达的对象、跨 card aggregate/array copy、构造中途晋升、多 mutator、Context、native root/handle/pin、callback/冻结栈段，以及 minor 与 full 的交替。性能记录包括 minor/full 次数、年轻分配与晋升量、扫描脏区、停顿、吞吐及实际堆占用；不得只用旧 full stress 证明分代正确。详细实施与验收见 [M31 设计](../milestone31/DESIGN.md)。

## 4. Scoop ABI FFI runtime functions

对应 spec 第 14 章。以下固定 runtime 必须提供的功能与语义；M13 已定稿的 thread state、handle 与 callback token 编码继续按本章执行，M15增加精确root更新与moving语义；尚未列出的导出 C 函数签名在实现时由 runtime header 锁定。

### 4.1 对象分配

- runtime导出入口在类型和symbol上区分managed generated-code entry、Scoop ABI native-borrowed entry、runtime-only internal implementation与foreign-callback entry；不得由单一入口在运行期猜direct caller kind。以下本节分配API属于native-borrowed入口，managed TLAB slow path使用3.1所述独立managed入口；
- `scoop_runtime_alloc_pinned(type_desc, size) -> ScoopObjectHeader*`：**分配即固定**，直接返回裸指针。它用于确实要求稳定裸地址的 native / C ABI 场景，不是 direct-ref Scoop ABI 的默认通道。pin 标志在分配返回前已设置，因此即使分配过程触发了 GC，返回的指针也可以直接使用；其所有权与 `unpin` 时机必须由具体 API 契约明确（spec 14.1）。
- `scoop_runtime_alloc(type_desc, size) -> handle`：返回 GC handle（可移动），用于需要把新对象长期带出当前 native root frame的场景。
- FFI 代码没有 stack map，无法使用 managed 快速通道（见 3.1）。若新对象只在本次 native调用内使用，可把当前引用写入 native root slot；pinned 分配与 handle仍分别服务于稳定地址和长期保活。
- native-borrowed分配入口在第一个可能GC的动作前验证当前thread transition anchor及完整root frame链。它触发collection时扫描冻结managed segment顶层的caller-root frame、native实现登记的全部`DirectSlots | RecursiveRegion` frame，并从预先捕获的transition anchor精确扫描外层managed frame；不捕获当前C return address，也不跨native frame反向unwind；返回后native代码分别从slot或region base reload。

### 4.2 native root、pin 与 handle 操作

- native root链的frame kind是封闭`DirectSlots | RecursiveRegion`。`push_native_roots(slots, count)` / `pop_native_roots()`等价能力登记一组`void **` direct-ref slot；`push_native_region_root(base, byte_extent, scan)` / `pop_native_region_root()`等价能力登记`{non-null base, nonzero byte_extent, NonEmptyRefScan}`，供含内联managed leaf的caller/value storage使用。具体C结构可以内联携带`previous + kind + payload`，但必须是类型化runtime API，不能依赖C栈保守扫描猜测，也不能用`count == 0`或null scan伪装另一variant。
- 两种push/pop本身都不得分配、触发GC或回调managed代码；所有root frame共享一条严格LIFO链。RecursiveRegion的base/extent在frame生存期内地址稳定，scan必须是已canonical且通过2.2结构与范围检查的只读nonempty program；push先验证所有静态offset/alignment，collector再用同一recursive slot visitor相对base扫描并在移动后原地更新每个leaf，动态Array节点还须使length/乘加落在extent内。用于2.3 boxing时，extent/alignment/scan精确来自已验证BoxedValue TD的inline layout，`scoop_rt_box_value`要求链顶region frame与`source_place`及该TD三者逐项相符。
- generated caller在进入可能park的transition/managed-runtime入口前以release语义发布完整frame，入口/collector在把线程视为可扫描后以acquire观察；pop只能发生在入口返回并从更新后的region/slot reload之后。direct native代码跨safepoint后必须从slot reload，aggregate/value代码必须从region base重新读取，不能缓存旧managed leaf。
- generated managed caller使用的region push/pop经compiler-private `GeneratedNoGcLeaf`入口或等价内联序列完成；该入口只操作当前TLS root链，必须NoGC、nounwind、有限时间且不得执行allocation、park、handshake或callback，因此它不建立新的native transition，也不需要stackmap。源码与普通extern不能取得该symbol；generated CFG在随后的managed entry statepoint之前完成push，并在所有非fatal出口匹配pop。
- direct ref只在无 safepoint的同步借用区间内可以作为普通 C pointer缓存；不得把该副本保存到 root frame之外、全局存储或调用返回之后。

- pin / unpin：直接读写对象头的 pin 标志（`scoop_runtime_pin(ptr)` / `scoop_runtime_unpin(ptr)`），O(1)。
- `scoop_runtime_pin_handle(handle) -> ptr`：把传入的（可移动）handle 解析为当前地址并固定，用于 FFI 收到 `GcHandle` 参数又需要裸指针的场景。
- handle 校验：runtime 必须校验generation、slot与live状态；非法或stale handle按4.4视为fatal runtime ABI error。
- Scoop 侧的 `pin` / `unpin` / `getGcHandle` / `releaseGcHandle`（spec 14.1）是普通 core 函数，通过已声明的 runtime intrinsic 映射到上述能力。`PinnedPtr<T>` / `GcHandle<T>` 的 Scoop 参数与返回使用实际单字段 struct 的 indirect aggregate ABI；只有 C 边界采用 `UInt64` 透明表示。底层 runtime 继续使用既有 raw pointer/`uint64_t` 入口与 handle 编码，不为 Scoop struct 改写 runtime C ABI。

### 4.3 回调 Scoop closure

- M13 提供 managed callback registration协议。概念入口为 `scoop_runtime_callback_register(closure, adapter, signature, mode) -> cookie`：注册函数按 Scoop ABI直接接收 ordinary、非 suspend closure，为其建立 `GcHandle`，并在runtime registry中创建 opaque token slot。64位host的GC-free cookie在目标ABI保证可往返且保持canonical的非零payload位内编码generation和slot，只做`uintptr_t`/`void *`往返、从不解引用；超出该位预算的slot/generation不得分配。slot复用递增generation，因而可检测stale/use-after-final-release而无需永久泄漏C heap tombstone。
- token slot至少保存closure handle、首个异常handle、typed adapter、静态signature descriptor、`Reusable`/`OneShot` mode、owner/active计数和完成/失败状态；M27 另保存第 9 章的 binding-root snapshot handle，空快照用明确的空分支。C侧不得读取这些字段，也不得把cookie当地址解引用。
- callback ABI中的mode/state整数只是固定wire code，不具有Scoop nominal enum identity：`scoop_runtime_callback_register`的`uint32_t mode`中`0/1`分别表示header常量`SCOOP_FOREIGN_CALLBACK_REUSABLE`/`SCOOP_FOREIGN_CALLBACK_ONE_SHOT`及经core-contract验证的`Reusable`/`OneShot`；其他mode输入是fatal ABI error。`scoop_runtime_callback_state`的`uint32_t`返回中`0/1/2/3`分别表示header常量`SCOOP_FOREIGN_CALLBACK_REGISTERED`/`ACTIVE`/`COMPLETED`/`FAILED`及经验证的`Registered`/`Active`/`Completed`/`Failed`。compiler metadata必须原子保存每个code对应的exact typed variant并在边界穷尽转换；runtime返回其他state值属于ABI破坏，generated code必须fatal/trap，不能把该整数直接作为语言enum tag。
- 编译器为每个实际导出的 concrete函数类型生成 managed invoke adapter，并按`(C signature, context index)`生成/复用静态 C ABI trampoline。源码`contextIndex: Long`在HIR已验证为非负且可索引该签名，进入runtime/bridge metadata后是zero-based typed `CallbackParameterIndex(u32)`，不是源码`Long`；负数、超过`u32`、越界或非`Ptr<Unit>`槽均在生成trampoline前失败。trampoline identity只包含canonical C signature fingerprint与该index，不包含具体closure、registration、managed adapter或callback body id。trampoline按真实 C签名收参，移除被token占用的context参数，把其余值写入 C-FFI-safe args/result storage，再调用 C-callable `scoop_runtime_callback_invoke(cookie, signature, args, result)`；runtime不能用未类型化可变参数直接猜 managed invoke ABI。
- `scoop_runtime_callback_invoke` 执行 attach-if-needed → enter managed → 从 handle取得 closure并登记为root → 调用 adapter → leave managed → detach-if-owned。closure调用期间使用普通 managed ABI、statepoint和异常处理，可以分配及触发 GC；跨调用保存的不是 closure裸指针，而是 token中的 handle。
- M27 在同一流程中同时 resolve/root binding snapshot，typed managed adapter 在其入口 poll 后、用户 closure 前建立并进入独立 TaskContext；全部正常/异常出口先恢复 previous Context，再返回 C。它不在 native-safe 阶段分配 Context，也不新增另一套 attach/GC 协议；私有 adapter 参数变化见 9.4。
- token提供 `retain` / `release` 等价能力并明确 ownership transfer。普通值复制不增加owner；owner与active lease都为零后撤销handles并回收slot，之后调用属于 ABI错误。`Reusable`调用只增减active lease，持久owner由unregister后的调用者释放；`OneShot`入口原子claim并把一份worker owner转为active worker lease，出口消费。需要在`join`侧观察完成/异常时，observer必须预先retain并持有到读取状态后最终release；创建失败路径释放所有尚未转移的ownership。
- callback adapter必须在返回 C前捕获所有 Scoop异常，物化成managed对象并以status/受管异常handle报告失败；异常不得展开穿越 trampoline/C frame。token以first-wins保存首个失败，trampoline按真实C返回类型返回全零值；由API-specific同步点后的observer决定重新抛出。invalid/stale cookie、signature不匹配、one-shot重复调用或shutdown后调用无法安全映射为任意C API错误，统一视为fatal runtime ABI error。
- 首版只支持原生API具有显式 `void *` context/user-data槽的 callback；静态 trampoline和token分别占据 function pointer与context。缺少context槽的API不能导出任意closure，只能使用 spec 13.10 的静态 `FunPtr`，直到后续实现动态 executable trampoline或有限slot registry。

### 4.4 错误处理

- 无法分配runtime/GC元数据、非法或stale handle、native-root LIFO破坏、非法thread transition、callback cookie/signature错误及shutdown后重新进入均为fatal runtime ABI error，打印稳定诊断后终止。普通callback抛出的 Scoop异常不属于runtime内部失败，按4.3转为token失败状态。

### 4.5 线程状态

- M12 的 Scoop ABI outbound调用不切换线程状态（不插 `enter_native` / `leave_native`，spec 14.2）；native callee仍属于当前已注册 managed thread。M13 多 mutator实现按 3.5 区分 C ABI native-safe与 Scoop ABI native-borrowed，runtime/managed入口读取该线程的 managed/native root frame链并完成 safepoint握手。
- foreign callback是反向边界：未注册线程必须先经 `scoop_runtime_attach_foreign_thread` 等价入口建立 thread state，再由 callback gateway进入 managed；detach只能由拥有attachment且已退出所有 managed frame/native root frame的代码执行。
- 边界转换必须可嵌套并按LIFO恢复previous mode；只使用一个`native_depth`不能区分native-safe/native-borrowed，也无法正确处理“managed → C → same-thread managed callback → C”的重入链。native-safe返回、native-borrowed返回和callback enter在切入managed前都必须检查当前GC epoch。

### 4.6 Initialization coordinator

- 任何`enter`前，descriptor必须已经由2.8/3.3的全程序registry验证并intern。eager startup顺序是dependency Cone先于dependent Cone；互不依赖的ready Cone按canonical coordinate byte order；Cone内按`PersistentInitializationUnitId` bytes严格递增，coordinate/declaration path只用于显示。runtime独立验证program给出的canonical order。直接ensure仍可按普通依赖提前运行目标unit，cell/wait-edge语义不因此改变；generic delegated extension unit始终lazy，不进入eager loop。
- M21 的generated ensure先调用typed managed入口`scoop_rt_init_enter(descriptor)`。入口以acquire语义观察cell并返回封闭结果：当前线程获胜为`RunInitializer`，已经Initialized为`Ready`，既有失败为`Failed(rooted Throwable)`，同线程重入或跨线程wait-for cycle为`Cycle(GC-free stable unit path)`。无环等待按2.7、3.5进入safepoint-aware park，醒来后重新观察状态；生成代码不得自行spin、缓存cell字段或绕过coordinator。
- generated ensure对`Failed`从已登记failure root取得managed异常并重新`scoop_rt_throw`；failure root的物理槽可按`Any` managed ref登记，但该runtime入口只接受/返回已经物化的`Throwable`，不能把任意managed值写入此专用槽。对`Cycle`先把path复制为普通managed String，再调用HIR已经绑定的core compiler-protocol cycle thrower；该`String -> Unit` target负责构造并抛出`IllegalStateException(message)`。enter返回所需的managed ref/result storage与GC-free path metadata必须由typed call/result plan完整描述；path的runtime side storage至少存活到复制完成。coordinator不认识core名称、thrower/constructor symbol或对象layout。
- `RunInitializer`分支执行ordinary managed initializer body。成功后，生成代码先完成property storage写入，或以release语义写入singleton published root slot，再调用`scoop_rt_init_succeed(descriptor)`；该入口发布Initialized、移除dependency/wait edge并唤醒waiter。`Ready`分支只从已经登记的storage/root slot重新读取结果。
- initializer抛出的native exception record不能保存进cell，也不能跨线程共享。generated catch-all在catch仍active时调用5.4的`scoop_rt_materialize_exception(caught)`得到普通managed `Throwable`，随后调用`scoop_rt_init_fail(descriptor, throwable)`；该入口把throwable写入已登记的failure root slot，以release语义发布Failed，移除dependency/wait edge并唤醒waiter。随后当前访问从该managed对象重新`scoop_rt_throw`；以后所有访问从failure root读取并重新抛出同一managed对象，但每次都会建立新的native unwind record。
- `enter` / `succeed` / `fail`是编译器与runtime之间的typed内部ABI，不是源码FFI API。descriptor决定unit identity和对应root/storage，调用者不能提交任意裸地址；非法transition、descriptor/image不匹配或未持有winner资格属于4.4的fatal runtime ABI error。

---

## 5. 异常

跨 Cone 的 `throw`、语句及表达式形式的 `catch` 使用前端已解析的实际 `Throwable` 声明，沿共有依赖类型查询取得继承关系并执行同一子类型规则；导入协议不允许跳过检查。同名普通 class 不能替代该实体。异常构造、默认参数、调用、布局、TD 和展开使用共有路径，保留调用求值顺序、catch 顺序与 finally 语义；此能力不增加协议资格、独立证明、wire 字段或 runtime ABI。 公开存储属性的 getter 与可公开调用的 setter 必须按实际 owner 和声明类型生成普通 callable body，即使 provider 的正文没有引用该属性；名义类型、顶层属性与 core 使用同一规则。参数自由且可物化的属性自动导出实际 body。泛型或 source-only owner 仅用于签名和表示查询时，不自动实例化其普通方法或 accessor；实际调用仍通过同一 typed 请求生成完整实例，dispatch 所需的方法继续随其实际类型物化。

跨 Cone 的强制 `as` 沿实际 `ClassCastException` 声明查询完整异常类型，并选择该声明的零参数 constructor 或默认参数适配入口。成功检查保留原对象身份，失败通过普通 class 分配、外部 initializer 调用和 throw 执行；别名、interface 与装箱值沿同一类型与 ABI 路径处理。默认参数中的转换在实际展开时进入相同路径，未求值模板不触发机器物化。异常类的表示仍依赖后续泛型能力时，前端在输出 LocalConcrete HIR 前对实际执行点给出已有 layout-required 诊断；不输出缺少所需表示的成功 IR。MIR 直接消费完整协议中的 typed 声明引用，不再建立丢弃外来声明信息的 Core/Imported 资格投影。所选 constructor 使用共有依赖 callable 集合，保留真实逻辑/物理签名、GC effect、ABI 和 relocation，不增加转换调用的证明记录、wire 字段或 runtime ABI。零参数默认值 adapter 以已有 GeneratedCallable 实体及 ClassInitializer 表示进入共有 callable 导出，不能只发布协议引用而遗漏定义。MIR/LIR selected callable 记录本身是机器依赖引用；reader 按 provider、typed target、定义、签名和传递依赖检查其完整性，不要求隐式调用另附一份 HIR 操作资格记录。

共有 HIR 类型位置的结构、foreign nominal 分发、真实 provider 与定义/求值位置在 HIR reader 边界检查一次。后续物化查询和 MIR/LIR 消费同一未变化的 typed 记录，不重新完整检查这组 HIR 关系，不建立额外验证状态或凭证；实际类型表示、签名、ABI、对象与传递引用仍由相应边界检查。外部字节重新读入或相关数据发生变化时重新验证受影响部分。这一职责调整不改变 wire、内容 fingerprint 的字段组成或 runtime C ABI。

类型生产器直接返回完整的共有 type section；MIR 使用已有继承边、槽序、typed 实现目标及成员引用。类型物化和构造器选择查询已有共有 nominal/callable 声明，不另行投影来源 foundation、完整 protected 声明、构造器签名或参数协议来重复证明同次产出。删除这些副本的生产工厂、凭证外层、只用于副本的 reader 及测试。必要的类型、引用、继承、ABI 与格式检查保留在实际消费边界，未变化的数据复用已有检查结果。

所有可见性的 nominal、callable、property、参数、默认值和定义环境由共有源码接口完整保存。protected 成员仍以 typed 引用参与实际 MIR callable 选择，其可见性和签名读取同一声明；构造器使用共有 nominal 的 constructor 引用及对应 callable，不在 inheritance record 再保存一份 payload。generic 词法 owner 不因访问域查询而要求 machine exact type。名义类型的 lookup/inheritance/slot 三份派生域不再保存和重验。

`CrossConeTypeSemanticsSectionV1` 保留 field 1、2、3、8，field 4～7 退役；`NominalInheritanceInterfaceV1` 保留 field 1～4、7～9，field 5、6 退役，退役字段不复用。成员引用的 Constructor tag 2 随重复构造器通道退役，实际 constructor 始终使用共有 typed 声明。HIR `cross-cone-type-semantics/11`、required inventory、profile 与内容 fingerprint 同步更新，旧产物和缓存需重建；不保留旧来源副本的双轨兼容，不改变 runtime C 调用约定或 String 表示。

M23-7 将访问域计算保留在 HIR lowering；产物保存原声明 visibility、typed nominal owner、slot identity 与实际实现引用，不重复持久化访问域或在 reader 重放 protected/override 的可见性语义。`InheritanceSlotContractV1` 的重复 domain field 5 与 owner field 2 退役且不复用，保留 field 1、3、4、6、7；`InheritanceSlotTargetV1` 的重复 owner field 2 同样退役，保留 field 1、3、4、5。普通宿主的封闭泛型父类型使用原声明与完整 receiver application 查询继承和槽，签名 receiver 直接提供宿主及完整实参；HIR `cross-cone-type-semantics/11` 与 required inventory、profile、fingerprint 同步，旧产物和缓存需重建。必要的声明归属、引用、签名、effect、abstract target modality 和实际继承路径检查继续保留；不改变 runtime C ABI。

M23-7 的实际泛型存储将该 section 升至 `/9`，并沿当前 MIR `cross-cone-type-bridge/6`、LIR `cross-cone-layout-abi/5` 传递实际表示、完整派发及 Strong/ODR callable 定义。物理 shape 引用使用 `cross-cone-layout-link-closure/3`。异常字段复用这条布局与 GC 路径，外部 initializer 保留完整物理签名；registration 使用原 definition plan 所属的 Strong 或 ODR 摘要节点。格式与阶段职责见实现规范 2.13，本项不修改 runtime C ABI。

共享可移植表达式在 `hir/cross-cone-interface/35` 保存原 `EvaluationOrigin`，使泛型构造委托等正文中已展开的默认值保留实际求值位置；普通默认参数使用仍按本次使用点展开。格式与读取规则见实现规范 2.13 和 M23-7 设计第 10 节。此调整要求旧产物与缓存重建，不改变 runtime C ABI、布局或 GC 契约。

### 5.1 Scoop exception record 与抛出

- M25起异常runtime只建立在Itanium Level I unwind接口上，不使用C++ ABI。runtime私有的`ScoopExceptionRecord`包含恰好一个满足目标对齐要求的`_Unwind_Exception`、catch/rethrow/lifetime元数据，以及按对象TypeDescriptor大小和对齐保存的Scoop对象payload；各部分的具体offset不属于生成代码ABI，raw unwind pointer与payload之间只能经runtime入口转换。
- 每条新异常记录使用Scoop专属且稳定的`exception_class = 0x53434f4f50000000`（`"SCOOP\0\0\0"`）。`_Unwind_Exception.exception_cleanup`负责在记录最终删除时先撤销payload的stable external object root，再释放整条记录；unwinder私有字段只由unwind library读写，runtime和personality不得挪作catch状态。
- 抛出入口`scoop_rt_throw(obj)`要求对象头的TypeDescriptor shape为`FixedObject`，从其`minimum_size/instance_alignment`读取精确allocation size/alignment，分配record并把完整对象按值复制到payload，随后在任何可能展开或触发GC的动作前登记该payload为stable external object root，再调用`_Unwind_RaiseException`。`Throwable`层次若出现其他shape是fatal type/runtime invariant error。catch匹配读取本次payload copy；选中后为绑定变量物化一次可逃逸的managed对象，原managed对象无需pin；绝不能把原managed对象或普通GC heap地址直接解释成`_Unwind_Exception`抛出。
- `_Unwind_RaiseException`只会在没有handler或unwind错误时返回。runtime必须在仍可读取payload时打印稳定的`uncaught exception: <type name>`或unwind-failure诊断，删除异常记录并终止进程；不注册或调用C++ `std::terminate`。

### 5.2 Personality 与 landing pad

- LLVM 22.1 的封闭 LSDA 形态包含 catch-all 和纯 cleanup：LPStart 为 `DW_EH_PE_omit (0xff)`，call-site 为 ULEB128；有 catch-all 时 TType 为 `0x9b`，只有一个 null type entry；省略 TType（`0xff`）时不读取 type-table offset，所有 call-site 的 action 必须为零，只能表达 cleanup 或无 action。无 TType 的有效表在 call-site 表结束，不能从后续 section 字节推断 catch/action。此规则适用于 Darwin/AArch64 和 Linux/amd64；不增加 C++ typed catch/filter。
- 栈展开使用LLVM `invoke` / `landingpad` / `resume`和runtime自有`scoop_eh_personality`。M25的personality直接消费LLVM 22.1为当前Itanium target生成的LSDA，只接受编译器封闭输出的两类action：Scoop catch-all与cleanup；Scoop源码catch类型继续由landing pad之后的普通`scoop_rt_is_instance`分派完成，LSDA不携带C++ RTTI或Scoop TypeDescriptor类型表。
- search phase只允许Scoop `exception_class`的catch-all action成为handler；cleanup-only action不能提前终止search。cleanup phase在匹配call-site range时按ABI设置exception/selector数据寄存器与landing-pad IP：中间cleanup进入cleanup pad，`_UA_HANDLER_FRAME`进入已选中的catch pad。`resume`保持LLVM生成的`_Unwind_Resume`，不能用它实现源码重抛。
- personality必须验证version、action flag、LSDA header、pointer encoding、call-site范围和action链；当前target profile没有声明的encoding、typed catch/filter、损坏或越界表项均为fatal unwind错误，不能退回`__gxx_personality_v0`、`__gcc_personality_v0`或把任意非零action当作合法Scoop handler。
- 非Scoop exception不得被Scoop catch解释为managed对象。它进入生成Scoop EH区域属于不受支持的foreign unwind边界，runtime必须终止而不是交给`BeginCatch`；本条不改变5.5的FFI边界限制。

### 5.3 Catch 状态、结束与重抛

- landing pad先捕获opaque exception record/raw pointer，再由普通dispatch块调`scoop_rt_begin_catch(raw)`验证`exception_class`、把对应record压入当前`ScoopThreadState`的caught-exception栈并返回其payload managed ref。该ref指向heap外稳定对象，但其对象头、TypeDescriptor和出站引用遵守普通Scoop对象与3.3 stable external root契约。
- 普通catch选中分支后，在绑定源码变量前调用5.4的`scoop_rt_materialize_exception(caught)`一次。绑定变量是普通managed引用，可以返回、存入对象或被闭包捕获；不能把只在native catch活动期间有效的payload地址绑定给源码变量。native catch仍由既有cleanup路径结束，初始化失败处理直接复用该managed对象。
- 每条正常离开handler的路径必须调一次`scoop_rt_end_catch()`。入口只操作当前线程栈顶并弹栈：普通caught record立即调`_Unwind_DeleteException`，标记为rethrow的record则恢复为in-flight而不删除；同一active record不得重复`BeginCatch`。cleanup callback是撤销root和释放record的唯一最终出口；runtime以active registry/state保证只调用一次Delete，callback在解引用record前也按raw地址检查active membership并拒绝重复回调。释放后的raw header不再是private ABI或Level-I API的有效输入。begin/end/rethrow都不得分配managed对象、触发GC或把异常记录地址暴露给Scoop源码。
- `scoop_rt_rethrow()`只在存在active catch时合法。它在当前record上标记rethrow并对同一个`_Unwind_Exception`重新调用`_Unwind_RaiseException`；随后原handler的cleanup chain调用`EndCatch`时只弹出catch状态而不得删除record，外层`BeginCatch`重新接管同一payload identity。handler内抛出另一异常时，cleanup chain先结束旧catch，再以LLVM `resume`传播新record。
- Scoop不提供`exception_ptr`、跨线程exception record共享或C++式公开引用计数；语言可跨线程/挂起保存的是managed `Throwable`，不是native unwind record。

### 5.4 跨控制边界的异常物化

- `scoop_rt_materialize_exception(caught)`按caught payload的TypeDescriptor分配managed对象，保留新对象的`td`和`gc_word`，只复制对象头之后的payload。普通catch、初始化失败处理和suspend handler使用同一入口；每个选中的catch只物化一次。
- M10 的suspend handler不允许把`scoop_rt_begin_catch`建立的native catch状态跨挂起点保存。选中catch或进入可能挂起的finally前，生成代码调用`scoop_rt_materialize_exception(caught)`：按caught payload的TypeDescriptor分配managed对象，保留新对象已初始化的`td` / `gc_word`，只复制对象头之后的payload。复制期间exception payload仍登记为stable external root；复制完成后立即`scoop_rt_end_catch()`，catch local / pending exception改指向managed副本；若该managed副本跨挂起保存，还必须满足第8章的exact slot、发布与扫描契约。
- 恢复失败或物化后的继续传播从该managed对象重新调用`scoop_rt_throw`，因而创建新的native record。Scoop的throw-by-value语义不承诺两个native record相同；同一次未物化rethrow则按5.3保持原record/payload identity。
- M21 initializer failure也必须在离开winner线程的catch、写入共享Failed状态之前调用同一物化入口；coordinator只保存已登记为global root的managed `Throwable`，绝不保存`_Unwind_Exception`、catch payload地址或active catch状态。物化、`scoop_rt_init_fail`与重新抛出的顺序遵守4.6。

### 5.5 依赖与边界

- 生产runtime与生成对象的异常依赖只允许Itanium Level I `_Unwind_*`接口；不得导入C++ EH 的`__cxa_*`接口、`__gxx_personality_v0`、`__gcc_personality_v0`或C++ terminate符号。Linux CRT/libc 的`__cxa_finalize`与`__cxa_atexit`属于普通退出清理（包括C `atexit`的实现），不属于C++ EH，应在原生输入和最终ELF检查中区分。每个target profile必须显式选择兼容的unwind provider并以产物级符号检查验收；Darwin/AArch64由系统`libSystem`重导出libunwind接口，不添加`-lc++abi`，也不要求当前SDK不提供的独立`-lunwind`链接名。
- 内置异常的抛出点：除零（`ArithmeticException`）、`as`失败（`ClassCastException`）、`!!`失败（`UnwrapException`）、数组越界等（spec 10.5、11.7）由generated managed CFG构造异常并调用runtime-only no-return throw入口；不能藏进可能把异常展开出native frame的Scoop ABI FFI helper。range的非正step由普通Scoop core body抛`IllegalArgumentException`，不需要runtime专用入口。整数除零与类型转换失败均使用实际 provider 的异常 TD 和普通 constructor；允许其零参数调用通过默认参数 adapter 实现。保持原有 managed 分配、展开与移动 GC 契约，不改变 runtime C ABI。
- **边界规则**：异常不得穿越C ABI frame（行为未定义）；能否穿越Scoop ABI FFI frame取决于实现（FFI函数无landing pad，穿越意味着跳过外部语言代码——初版禁止，行为定为终止进程）。M25只替换Scoop进程内的异常runtime，不扩大FFI可展开边界。

## 6. 核心类型的运行时后备

core 的 checked 容量增长在 Long 计数溢出时调用无参数 NoGC 后备 `scoop_rt_allocation_overflow(): Unit`，沿既有 fatal allocation failure 终止。该入口不分配、不回调用户代码，也不引入新的异常或计数协议。

以 Scoop ABI FFI 函数形式实现；spec 14.4 的 `write(String)` 是不跨 safepoint直接借用 ref的最小范例，涉及分配的函数则按 4.2 登记 native roots：

- `String`：创建、拼接、内容比较、内容 hash、UTF-8 字节数、标量计数/定位/迭代、切片与字符/字节数组快照。读取定位的 native leaf 不抛异常：get 返回普通 `Option<Char>`，slice 定位返回普通 `Option<(Long, Long)>` 字节边界，core 对 None 构造并抛 IndexOutOfBoundsException。定位与复制之间 String 内容不变，已验证的边界可以直接复用；按长度分配后只复制完整 UTF-8 区间。
  两个 Option 返回入口按目标 `sret` 合同适配到共有 C storage helper：Darwin/AArch64 从 x8 取结果地址；Linux/amd64 从 RDI 取结果地址，普通参数随其后排列，并在 RAX 返回同一地址。不能按 C 编译器的 16-byte aggregate 直接返回规则改变 Scoop 的间接结果 ABI。
- `Array` / `MutableArray`：按spec 10.1的元素布局分配、读取`size`，以及`toArray` / `toMutableArray`的浅拷贝转换（spec 10.4）。源码可见bounds check、`IndexOutOfBoundsException`构造与throw都在generated managed CFG中完成，native helper不抛异常；若仅供受检代码使用的helper收到越界index则是fatal compiler/runtime invariant error。转换入口显式接收编译器已选定的目标concrete application TypeDescriptor，以该descriptor分配并保留新对象头，先验证source exact array TD/side metadata与logical size，再复制该size；`Inline`非ZST分支复制目标data offset之后的inline element payload，padding保持分配时的canonical zero；`ZeroSized`分支不调用payload `memcpy`。不得从来源对象、元素布局或类型名推断目标类型，也不得沿用来源descriptor；
- `StringBuilder.add/build` 是普通 core body；只有 build 的最终字符串聚合使用本节下述表示级 helper。List/MutableList 接口、ArrayList 扩容/位移/清空及 iterator 不增加容器 runtime ABI；
- 八种定宽integer的具体`ToString`/`Hash`后备，以及其他基本类型显式实现的`ToString`/`Hash`/operator equals所需后备（不提供`Any`或地址fallback）。M30的Float/Double只需要6.1所列后备，不因基本类型身份增加Hash。`Hash.hash()`与所有integer `compareTo`的源码结果都是`Long`（i64），不随operand宽度改成`Int`；窄signed/unsigned值可由core按规则sign/zero extend后复用64位后备。integer equals由typed intrinsic直接生成比较，既有`scoop_rt_int_equals`/`scoop_rt_uint_equals`在M22迁移调用点后退出公开runtime契约。alias在进入runtime前已经展开，runtime不按`Byte`/`Int64`等alias名称分派；
- `Iterator`/`Iterable`、四个独立nominal type `IntRange`/`UIntRange`/`LongRange`/`ULongRange`、Array iterator、range终止与非法step全部由普通Scoop core与生成代码实现；`LongRange`/`ULongRange`不再是前两者的alias，M22不新增range/iteration runtime ABI；
- 类型测试与装箱辅助：`is` / `as` 的exact TypeDescriptor比较、普通装箱/拆箱。

String 的两个定位后备遵守当前 core `Option` 的 Scoop ABI：`Option<Char>` 是 16-byte、8-byte 对齐的存储，tag 位于 offset 0、Char payload 位于 offset 8；`Option<(Long, Long)>` 是 24-byte、8-byte 对齐的存储，tag 位于 offset 0、两个 Long 位于 offset 8/16。当前 core 声明的 Some/None tag 分别为 0/1；这些是同一 core/native 实现的普通类型布局约定。Darwin/AArch64 的返回存储由 x8 传入，平台 shim 将该地址转交 C helper，不能使用宿主 C struct-return 分类代替 Scoop ABI。字符/字节输出数组由普通 core 初始化构造创建，分别以只前进的 byte cursor 和 byte index 填充；native 定位入口只借用参数，不分配或进入 GC；含 ref 的源码签名仍按既有 Scoop ABI Managed 边界处理。分配 String 的入口先登记输入快照的 native roots，分配后重取地址。

M26 的 parts 拼接后备只消费 `MutableArray<Option<String>>` 的有效前缀及 `Long` partCount，返回普通 String；source-level core 边界为 `coreStringJoinParts(storage, partCount): String`。StringBuilder 从自己私有的 ArrayList 取得当前 backing 与 size 作为本次调用实参，期间不执行用户代码；该内部存储访问不成为公开 List 方法或 borrow/view API。runtime 不读取 ArrayList/StringBuilder 字段，也不改变它们的状态。

具体入口 `scoop_rt_string_join_parts(const ScoopArray *storage, int64_t part_count)` 使用普通 Scoop ABI 的 Managed native contract；Option<String> 消费既有 nullable managed pointer niche。只在分配前检查有效前缀中的非空 String 并累计字节数，分配后的复制复用这个不变前缀。

该 helper 检查本次动态前缀范围，对所有 Some(String) 的 physical byte count checked 求和，经既有 String allocation shape 分配一次最终字节存储，再顺序复制 parts。空/单 part 可以直接复用不可变 String。None 出现在有效前缀属于内部不变量错误；这里不重复验证完整静态 TD、每个 part 的 UTF-8 或用户语言规则。native 入口及分配前按既有 caller-root/managed-anchor 协议保活 backing；allocation 后从被更新的 backing 重新读取 String ref 与 data 地址，不能跨 safepoint 缓存 part 指针。填充结果的循环不分配、不回调、不抛源码异常；如实现添加 poll，必须同时 root 结果并在 poll 后重新取得全部地址。最终 String 发布后不再写入其字节。

Char 编码、String 解码到 `MutableArray<Char>` 与 UTF-8 字节快照都沿同一精确 TD、分配、root 与复制路径。接受 List 的公开 companion 方法先在 managed core 中物化数组快照，不在 native helper 中遍历任意 interface 或调用用户 getter。fromUtf8Unchecked 信任调用方已经满足的 UTF-8 前置条件；toByteArray 是安全的复制，不返回可写 String view。M26 不引入 native buffer 所有权、显式 close、release hook 或外部内存压力记账。

### 6.1 Float / Double 后备与 ABI（M30）

语言行为见语言规范11.2.2、11.13与13.8，pipeline契约见实现规范2.18。支持的Darwin/AArch64与Linux/amd64 glibc、musl目标均使用IEEE binary32/binary64：Float的size/alignment为4/4，Double为8/8。它们是GC-free的inline scalar；scalar本身不进入扫描图，混合aggregate只扫描实际managed reference。数组、boxing、静态存储、generic实例、协程frame与moving GC沿既有精确布局和root规则。所有浮点位型均为有效值，不以NaN或负零为Option niche；Option使用既有tagged enum布局。

普通Scoop ABI按实际f32/f64标量传参/返回，aggregate继续使用既有间接ABI；不能把f32伪装成i32或把所有浮点统一扩为f64。C storage精确使用`float`/`double`，C aggregate、global/TLS、pointer与callback的适配由既有generated-C bridge完成。参数、返回、load/store及复制保存原始位型，包括signaling NaN；Scoop不额外检验或正规化外部传入的浮点值。

算术、普通比较、分类、显式转换与totalOrder均是不分配、不进入GC的操作。totalOrder由整数位操作比较完整表示，无需libm；浮点`div/rem`不构造ArithmeticException。LLVM将`frem`降为`fmodf`/`fmod`时，这些是已知不回调Scoop、不分配managed对象的NoGC native支持函数，纳入原有native-support与最终链接检查。Darwin由SDK的libSystem提供；Linux动态/静态配置使用各自目标libc配套的libm，不能混用host库或遗漏静态归档依赖。

生成代码以LLVM默认浮点环境为前提：最近偶数舍入、异常trap关闭、保留subnormal。正常启动和线程attach须建立该环境；native代码若改变环境，须在重新进入managed代码前恢复。语言不暴露浮点异常标志或动态rounding mode，也不要求每次运算或FFI调用包裹一套fenv保存/恢复。

字符串化使用两个精度独立的runtime后备，有限值按语言规范11.2.2形成最短有效十进制表示；不依赖进程locale，也不把Float先扩成Double再格式化。采用成熟的Ryu C转换实现并固定来源版本与许可证，Scoop适配层只负责规定的文本形状和String分配。后备接受scalar并返回普通String，属于Managed调用，沿既有String分配与native-root规则；不提供浮点hash或公开位转换后备。

JSON通过库内部的Scoop-ABI extern声明调用两个精度的十进制解析后备。输入是JSON parser已确认语法的原始number文本，不经Long/ULong或另一浮点类型中转。C实现以固定C locale的`strtof_l`/`strtod_l`直接得到f32/f64，并检查完整消费及结果是否溢出到Infinity；underflow产生的subnormal或signed zero有效，不能仅凭`ERANGE`拒绝。String不隐含NUL，必要的临时终止缓冲区按实际长度分配并在本次native调用内释放；沿既有allocation failure处理分配失败，不作为非法数字返回。

解析后备不回调用户代码、不触发managed分配或源码异常；它借用String并返回普通Option<Float>/Option<Double>，含ref的入口仍沿原有Managed Scoop-ABI边界。Option结果使用实际目标布局与sret适配，不按C struct-return猜测；None由JSON的普通Scoop代码转换为带path的DecodingException。该后备只解决JSON数值解析，不新增公开String解析API、外部buffer所有权或通用FFI框架。

上述能力不需要扩展TypeDescriptor、scan schema或runtime metadata ABI字段。新入口和实际native依赖按原有类型、符号与链接规则记录；只有实现确实改变的编译产物section升级兼容版本，不为未来Int128/Float128预占runtime契约。

## 7. 启动、线程与终止

主线程attach仅建立native-safe状态；每次eager gateway及最后root gateway分别按2.8建立boundary、完成epoch enter握手、执行mandatory gateway入口poll，并在GC-free status返回后完成leave与boundary回收。两次调用之间的C coordinator没有活动managed段，不能跨C frame扫描或展开。

M27 在第一个实际 gateway 的入口 poll 后、执行任何源码 initializer/main 前建立空 root TaskContext，保存在 ThreadState 中并跨后续 gateway 复用。Context 不改变 EntryPending/活动段的区分，不要求 native-safe 分配；结束时按 9.4 清空 root。

- 进程启动先建立纯 native runtime/thread/callback 基础状态，再按 2.8 接收实际 image pointer 集合和 root entry。先检查已加载范围内的 prefix/span，再收集全部六类 record，核对实际引用、地址唯一性、TD/scan、初始化初态与全部连续 stackmap v3 blob；在同一 image dependency 索引上形成语言规范 12.3 的 canonical Kahn 顺序。全部成功后一次性冻结静态 registry，初始化 heap/handle 并以 native-safe attach 主线程，随后逐次通过 eager gateway 和 root gateway 执行 managed 代码。登记失败前没有 managed 副作用，不能留下部分 GC root 或改写 cell；gateway 失败后后续 eager 与 main 不执行。此过程不读取已删除的 program/core descriptor，不重算 RuntimeImage/Graph/ObjectDefinition 摘要，不重放语言语义。异常报告按 2.8 的实际 failure root 和已有 world 同步读取只读类型诊断，随后 abort；任何 Scoop exception 不得穿越 C startup frame；
- object/companion不因metadata登记或其const读取而初始化；第一次非const访问通过2.7 gate同步ensure。initializer entry是编译器生成的ordinary managed callable，可以分配、触发GC或抛异常但不能挂起；runtime只协调状态、wait与发布，不按名称反射调用property/accessor，也不补initializer body；
- 线程：主线程启动时注册；runtime创建的线程及 foreign thread在首次进入 managed代码前 attach，在退出最后一个 managed入口且不再持有 runtime thread state时 detach（配合 3.5、4.3）；
- 终止：`main` 返回后runtime先进入`ShuttingDown`并拒绝新attach/registration；只有主线程以外无attachment、无活动callback且token ownership均已释放时才销毁GC状态。存在迟到线程/token时报告计数并终止，不在其仍可能进入时释放runtime。shutdown不运行GC finalizer，也不为release hook遍历全部live/uncollected对象；native resource仍须在正常控制流中显式释放。

## 8. 协程

`suspend` 的状态机变换、`CoroutineStep<T>`、frame 与各挂起点的 `Continuation<T>` adapter 全部由编译器生成（spec 8.2、11.9；impl spec 2.3）。这些实体都是普通 managed 对象/值：

- frame 与 continuation adapter 必须有普通 TypeDescriptor 和完备的递归引用扫描描述；frame 链由 GC 自然保活，不登记额外的 runtime root；
- `CoroutineFrameState`是pending控制流的唯一物理discriminator；runtime frame不另存target、cleanup cursor、continuation chain或第二套pending tag，这些都是编译器的GC-free静态metadata。frame只保存state和实际动态payload的exact `CoroutineSlot<T>`；所有slot从`Empty`/canonical zero初始化，对应live/pending payload必须在使其恢复/dispatch edge可消费之前写完，恢复方只在规定的acquire/claim成功后读取。frame TypeDescriptor必须递归扫描每个可能活动slot，包括含managed ref的aggregate；`Return(Unit)`不占动态payload slot，其他`Return(T)`保持exact `T`，异常slot只能保存5.4已经物化并`EndCatch`后的managed `Throwable`，不得保存`Any`、opaque bytes、native exception record或EH状态；
- continuation 的完成状态与 frame 的当前恢复状态存于 managed 对象字段。M10–M12 的最小实现是单线程协议；M13 把adapter claim/完成与frame `running/suspended/completed`转换升级为64位对齐原子状态机：winner以acq_rel CAS取得完成/驱动权，先写payload再release发布终态，读取方以acquire消费；等待短暂`Completing`状态的循环必须包含safepoint/backoff。该状态机的64位字段是compiler/runtime共享的独立typed atomic state carrier，不是源码`Long`属性，不得经普通integer operation读写。已attach线程可安全恢复既有continuation，重复完成仍抛`IllegalStateException`。调度器、队列和恢复后在哪个线程继续执行仍由后续标准库规定；
- hidden continuation ABI 仅存在于编译器生成的 Scoop 托管调用之间。runtime 不提供 suspend FFI 入口、extern trampoline 或 callback wrapper；`@Extern` 与 `suspend` 的互斥，以及挂起函数声明引用不能在 `FunPtr` 上下文中解析为原生地址，由 HIR 保证（spec 8.2、13.4、13.10）；
- runtime 提供第 5 章的 ABI 异常物化及 M27 第 9 章的 Context 操作；状态分派、完成权竞争与何时进入 frame Context 仍由生成的 driver/adapter 决定，runtime 不接管恢复、队列或调度；
- 调度器、事件循环与取消属于标准库。永不恢复的 continuation 只会按普通不可达对象被 GC 回收，runtime 不替它执行 cleanup / `finally`。

---

## 9. Task-local Context（M27）

语言行为以 language spec 8.3、11.9、14.3 为准，实施设计见 [M27 设计](../milestone27/DESIGN.md)。Context 只承担精确类型动态绑定、结构化恢复和任务传播；源码不能直接操作 runtime 的 TaskContext、key cell 或 snapshot。

### 9.1 表示与操作

首版使用一个可变 TaskContext root 和不可变的四叉 radix tree。两个内部 managed 类型分别为 `TaskContext { bindings }` 与 `ContextNode { child[4] }`；16-byte 对象头下分别为 24/48 byte，均适配当前 64-byte small-object 上限。node 的槽是有 managed provenance 的 nullable ref，中间层指向 node，叶层指向实际 payload；collector 只需递归扫描这些 ref，不执行动态类型查找。已发布节点除 GC relocation 外不可修改。

slot 为进程内 `u32`。登记完全部实际 key 后选择统一树高，最多 16 层；lookup 按 slot digit 索引，遇到空边即缺失，不随动态 scope 深度遍历链表。slot 数值、树高、fanout 和字段 offset 不属于语言或 `.slib` 身份；表示变更通过实际 runtime ABI/layout fingerprint 迁移。

| 操作 | 语义与 effect |
| --- | --- |
| try-get(key cell) | 返回 typed nullable managed ref；不分配、不 GC、不 unwind；miss 由生成的 managed CFG 构造并抛异常 |
| push(key cell, value) | 从 current root path-copy 新节点；旧 root 与输入在所有分配点保活；完成后提交一次 bindings 并返回 typed scope mark |
| restore(mark) | 恢复 mark 保存的旧 root；no-alloc、no-GC、nounwind，与现有 cleanup 同序 |
| snapshot-current | 取得当前不可变 root，空 Context 返回明确的空 root；no-alloc、no-GC |
| fork(root) | 分配一个独立 TaskContext 并持有同一 immutable root；可分配/GC，不复制 payload |
| enter(task) / leave(guard) | 保存/恢复当前线程 Context；no-alloc、no-GC、nounwind，严格词法 LIFO |

scope mark 只保存 owner TaskContext 与旧 root，由 compiler-internal typed aggregate 表示；嵌套与消费顺序由现有 cleanup CFG 保证。无需再维护 TaskContext undo 栈、depth、mark token registry 或独立 snapshot 堆对象。execution guard 是保存 previous Context 的另一种 typed value，不能与 scope mark 或源码整数混用。restore/leave 可做局部 owner/入口状态检查，不重放控制流或整棵树验证。

这些 leaf 仍读取或修改 ThreadState/tree，GC effect 不代表 memory purity。首版保留保守的 LLVM memory effect，不能标为 readnone/仅参数内存，也不能让 lookup/snapshot 跨 push、restore、enter 或 leave 错误合并或前移。

只有 push/fork/首次空 task 构造可分配。push 在提交前不修改可见 root，资源耗尽沿现有 fatal allocation failure；不会把 OOM 转成 `MissingContextException`。try-get/push 不读 payload 的 TypeDescriptor，不重复检查已经由 HIR 确定的静态类型关系。

### 9.2 GC 与并发

两个内部类型的 exact identity、完整 layout/scan 和 TypeDescriptor 由实际 core provider 生成并通过已有类型/布局产物导出；消费者引用原定义。runtime helper 需要的 descriptor 由 typed metadata 参数提供，使用与本版共享 C 布局一致的普通 TD，不手写伪 descriptor 或在启动恢复已退役的 ProgramDescriptor/CoreBindings。所有字段是普通扫描 ref，release hook 为 None。

current Context、previous guard、旧 binding root、push 中间节点与 value、取得的参数 local、frame field、callback snapshot 都必须出现在现有 root/RefScan 中。ThreadState scanner 不依赖当前 thread mode 来省略 current Context；TLS 只定位 ThreadState。所有 ref store 沿已有 checked write-barrier 路径，可能 GC 后重新取得对象地址；生成的 cleanup 在 restore 消费 mark 后立即将对应 frame slot 写回 Empty，以免仍可达的已完成 continuation 保留旧树。仅挂起不消费 mark，仍保留其完整扫描内容。

同一 TaskContext 的 binding 更新由其逻辑任务串行驱动。M13 的 frame/adapter release publication 与 acquire claim 同时发布 TaskContext 及其 binding；真正挂起后原 driver 只恢复本线程入口，不继续修改已交出的 task。同步 registration 的 latched completion 只投递 payload，不能同时启动第二个 driver。child 和 callback invocation 共享 immutable node，但各自持有可变 TaskContext；payload 对象本身仍遵守普通并发规则。M27 不新增 scheduler、task 锁或逐 lookup 全局锁。

### 9.3 key cell 与加载

canonical exact type 是唯一的 key 信息。IR 的 `ContextKey` 是对已有 `PersistentExactTypeId` 的独立 typed wrapper，不引入第二份 type hash、source key declaration 或 key-use 证明。机器 slot 是另一类型，绝不进入持久 identity。

每个实际使用 key 的 machine body 为该 key 发射一个 writable slot cell，同 body 多个 site 共用；cell 的 typed associated-atom identity 为 `(PersistentCallableBodyId, ContextKey)`。把该 body 的非空/空 cell-use 列表加入**现有 callable registration**，不增加第七类 image record 或另一张 program support 表。每项保存 exact key 与本 cell 的 typed relocation，cell 与 body 位于同一 LinkObject，沿其 Strong/ODR associated-atom 规则发射和合并。不同 body 使用相同 key 可以持有不同 cell。

metadata ABI 4 的 callable record 保存新增 cell-list 字段，并保持 exact size 检查。image 的六类 table 和启动函数参数保持，其他 record 的字段没有因此增加。旧 ABI 3 产物在 prefix 边界拒绝，不能在旧 record 尾部静默追加。

启动登记在既有全部 image/callable 收集过程中检查新增 span、cell 的可写范围和实际 body 归属；同址 ODR 引用复用已有去重结果。按 exact key 去重后分配进程 slot，一次发布给各 cell，再执行任何 managed gateway。cell 的 canonical object 初值为 0，live encoding 为 `u64(slot) + 1`，可以表示整个 u32 slot 域；所有登记成功前不修改 cell。

runtime 只处理实际加载地址、范围、cell 初态、重复归属及 slot 容量，不重新构造语言类型或 canonical hash，不复算 ObjectDefinition/ODR fingerprint。compiler/reader 在各自已有边界检查引用与内容；cell 初值、关联 key、owner 和 relocation 进入既有 object fingerprint，进程 slot 和已写入的 cell 值不进入 fingerprint/cache key。同一个 ODR body 的 cell 随 body 合并，不为 cell 再建独立 ODR member。

### 9.4 入口、协程与 callback

root/eager gateway 保持入口 poll 为第一条可握手操作。poll 后，生成代码在 current Context 为空时通过普通 managed allocation 建立空 TaskContext 并安装，再调用源码 body；后续 gateway 复用它。root 在 gateway 之间作为 ThreadState slot 保活。main 完成且不再进入 managed 代码后，在已有 thread/registry 同步下清空；detach 不得留下活动 Context entry。

foreign attach 可以保持空 current Context。callback gateway 按 4.3 进入已有活动 managed 段后，resolve closure 和 snapshot handles 并登记 native roots；typed adapter 执行自身入口 poll，然后 fork snapshot、enter invocation Context，再调用用户 closure。这个内部准备前缀允许尚未安装 TaskContext；它只分配/安装 Context，不执行 requirement 或用户 body。不增加可在 native-safe 任意分配的入口。

callback token 以 `Empty | RootHandle` 保存快照，handle 的空分支不尝试 resolve。非空 root、closure、failure 全部沿既有 handle 表管理；invocation 的 active lease 同时保活这些输入。注册不复制树，retain 不重新采样，final release 在 owner/active 都结束后释放全部 handle。私有 managed adapter 在已有 closure/storage/status 参数中增加 typed nullable snapshot-root 输入；外部 C trampoline 和 `ForeignCallback<F>` ABI 不变。adapter 的 catch/status 路径及正常出口均先 leave Context，再返回 C；快照取得/安装期间的临时 ref 也有完整 root plan。

私有 adapter 的物理参数顺序固定为 closure、nullable snapshot root、result storage、argument storage、exception output，返回既有 machine callback status。C gateway 在进入 managed 段后解析两个输入 handle，并同时登记 closure、snapshot、exception 三个 native root slot。snapshot 使用零 handle 表示空 root；注册本身是无 managed allocation、无 GC 的 leaf，沿原 handle 表保活当前 root。adapter 在 fork 成功后才创建 execution guard；其 catch materialization 若展开，先 EndCatch 再 leave。该私有边界变化进入 runtime ABI contract 6，runtime metadata ABI 4 的记录布局保持。

每个 resumable frame 保存非可选 TaskContext ref，并在发布 continuation 前初始化。start helper 在普通实参求值后 fork/enter child，task 与最终 completion 均在 child 下执行；adapter 只有成功取得实际驱动权才 enter frame Context。每次 driver 的 suspended、completed、failure 与 unwind 出口都 leave；仅 suspended 出口不执行源码 binding restore。跨挂起的 mark/参数沿 exact frame slot 保存；已消费的 mark slot 清空。completion 自身抛出也必须恢复入口，不能再次发送 completion 通知。

缺失 binding 是普通 `MissingContextException`；unresolved cell、错误入口或损坏 root 属于 compiler/runtime invariant failure。没有取消或 GC finalizer：永不恢复的 frame 不运行 cleanup，可达性结束后连同其 task/旧 root 正常回收。

---

## 10. TBD 清单

仍待后续里程碑补充：

- nursery／minor GC 与晋升已在 M31 设计中排期，见 3.9；其后的 survivor 策略、parallel/concurrent collector、arena 扩容与更多 target adapter 另行设计；
- off-heap ByteBuffer 的增长、borrow/view、close、失败原子性、external-memory pressure accounting、managed 侧 GC 反馈与 hook 路径只扣减的 release-safe 入口，整体留待 M26 之后另行排期；
- runtime functions 的完整签名表与错误处理矩阵；
- 异常穿越 Scoop ABI frame 的最终规则。
