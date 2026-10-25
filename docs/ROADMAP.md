# Scoop 实现路线图

旧 identity-only 产物 profile、平行来源 reader 和发布/Link 凭证策略退役；三个完整生产 profile 升为 `/3`，descriptor 只保留实际必需 section 清单，旧产物与缓存重建。此调整不改变 runtime C ABI、String 表示、初始化或 GC 语义；类型、对象范围和实际引用检查仍在对应消费边界完成。具体格式见实现规范 2.6 与 M23-2 设计 8.3。

静态存储与初始化失败根按其实际值类型引用 layout/scan。当前 Cone 只发射自身拥有的布局与扫描定义；外来类型的静态根复用共有依赖查询取得的完整 value-layout 和 scan 记录，保留实际 provider、typed identity、定义与 relocation，不因本地持有该类型的值而重发射 foreign Strong。layout/scan 指纹节点引用已经解析的实际记录，不要求该类型在当前 Cone 定义。MIR 必须携带生成失败根所需的实际 Any 声明，LIR 不再缺省重建固定 core 身份。static-storage 语义记录新增 field 32 保存 layout provider，完整记录使用 fields 1～32；语义投影使用 fields 1～10 与 32。共有 strong-production 两种格式当前为 /13、/14；在静态根的 /11、/12 之后增加实际 callable 正文的 canonical LIR 摘要（实现规范 §2.5），旧产物和缓存重建。runtime C ABI、String 表示、初始化状态与失败缓存语义不变，不引入 ODR 或多 image 启动。

普通 catch 的绑定必须可以像其他引用值一样离开 handler：匹配 native payload 后，在绑定变量前物化一次 managed 异常对象，后续返回、存储和捕获使用该对象；native unwind record 仍按既有 cleanup 规则释放。初始化 catch 复用这次物化，不再次复制。每次 throw 仍创建独立 native payload，runtime C ABI 不变。

M23-6 的值类型接口验收使用真实源码产生并消费产物，覆盖外来 struct/enum、父接口、默认方法、ZST、大值及含引用 payload 的装箱与分派；同时覆盖修改并重建 core 后新增值类型的下游使用。HIR/MIR/LIR 保留真实声明和完整接口关系，普通与移动 GC 运行验证实际 provider 的 TD、接口表及 canonical ABI。intrinsic 值类型也必须通过共有声明取得接口关系，不能依赖本地 core 定义或用空接口表跳过。

primitive 验收包含整数、Boolean 到 `Any` 和已声明接口的转换、拆箱、默认参数与下游再次发布，以及修改 core 的 intrinsic 类型接口后重建并消费。删除 MIR 装箱的 Defined core 要求和 intrinsic 类型的来源编号包装；保留前端声明规则、真实类型身份及实际 ABI。

该接入保留既有物化边界：声明可按需查询并缓存，未使用 primitive 与未实例化默认值不产生机器依赖；实际装箱与函数变型适配必须有完整声明、接口和 provider 定义。

独立 function/adapter 的 Structural ODR 发布仍属 M23-7；M23-6 对该请求验收能力诊断，primitive 正例覆盖本阶段允许的实际产物与运行路径。

整数 `div/rem` 通过共有依赖声明解析 `ArithmeticException`，并与强制 `as` 的失败分支共用普通类型、constructor、ABI 与 relocation 路径。只有实际运行点物化异常表示；未使用的默认表达式不产生机器依赖。删除 Managed 整数运算的旧无条件布局门和 MIR 的 Defined core 要求，不另存来源或操作证明。验收覆盖正常除法/余数、signed 边界、除零 catch、默认参数、修改 core 后的默认构造器适配，以及产物的下游链接和移动 GC 运行。

运行时类型转换的失败构造使用前端解析的实际异常类型与 constructor 引用，并沿共有的类型、callable、ABI 和 Link 路径消费。删除由 Cast 反向投影的独立 CastFailure call-site、RuntimeOperationDependency role，以及 reader 对同一目标再按 compiler protocol 进行资格判断的通道；普通源码调用的位置、参数、结果与 typed 引用检查保留。共有 HIR 格式更新为 `hir/cross-cone-interface/30`，原 call-site reason tag 2 与 external-reference role tag 9 退役，不复用；旧产物、profile fingerprint 与缓存重建。该调整不改变转换失败抛出 ClassCastException 的语言行为、runtime C ABI 或 String 表示。

引用上行转换在 HIR 中用显式 `ReferenceUpcast` 节点保存内部表达式及目标类型，不能直接改写构造、调用或局部读取的原始类型。MIR 使用已有 `Retype`，不分配对象、不改变引用身份；构造器仍按实际所属 class 分配。默认值正文使用新 expression tag 58 保存同一操作，tag 44 继续退役；共有 HIR 格式更新为 `hir/cross-cone-interface/30`，旧产物与缓存重建，不改变 runtime C ABI。

M23-6 的实际消费覆盖外来接口类型、父接口、class 虚方法、接口默认方法与动态覆写；真实 provider 源码产生方法和 dispatch 表，下游只消费完整产物，独立与组合 fixture 锁定 HIR/MIR/LIR 并链接运行。 同一产物消费路径须覆盖成员属性赋值、复合赋值、前后缀更新、class/interface setter dispatch，以及 ZST、大值和含引用属性；反例包含不可见 setter、只读属性和不匹配的值类型。

删除仅由测试实现的默认值operation-typing、nested ABI及root/origin语义工厂和其证明数据、平行验证入口与专用测试。正式reader继续使用共有声明表、完整typed模板、类型与binder检查、来源位置、局部数据流及真实引用一致性检查。局部数据流直接借用模板与共有字段查询，删除重复body input及authority适配器；nested descriptor保留实际类型化身份、parent/path与binder数据，删除独立Standalone证明模式。语言操作规则由前端负责，不在IR/meta crate再复制实现。此清理不改变wire字段、profile版本、runtime C ABI或String表示。

参数自由依赖class通过共有名义声明取得真实身份、modality、直接父类型和声明序字段，引用类型按GC契约传播，递归引用字段不展开成递归值布局。HIR保留完整声明及解析后的字段、父类型，不重建同名本地声明。外来class构造仍是对真实constructor的typed调用；HIR→MIR消费已选MIR绑定中的ClassInitializer角色，使用共有class分配路径创建对象，再将该对象作为receiver调用实际provider的初始化实现。构造表达式返回分配的对象，物理initializer返回Unit；普通返回class的函数不走构造分支。本地与外来initializer共用Callee表示、参数求值顺序和GC处理。 core默认导入层的构造调用和静态限定名同时查询普通Type/Value binding；用户新增的class、enum和typealias与其他依赖使用相同声明路径，不限于预设内建名字。此能力不增加来源凭证、core分支或runtime ABI，沿用现有类型、MIR绑定及LIR布局格式。 M23-6的真实class运行在单个最终镜像中链接实际产物及runtime；LLVM stackmap段按各对象贡献的完整v3 blob逐个读取，长度由已有count和对齐决定，保留每份格式、记录唯一性与精确PC检查，不增加展开预算。该读取不要求M23-8的多镜像启动或M23-9的program-link。

M23-6 清理默认值逐引用访问证明及仅测试使用的平行完整验证入口。保留前端定义处与继承后的可见性检查、完整 typed 正文和引用索引；producer 直接投影，reader 在边界检查真实引用和跨表关系。默认引用 field 3 退役，`hir/cross-cone-interface/30` 要求旧 `/24` 及更早产物、profile fingerprint 和缓存重建；跨 Cone 默认值实际展开与运行仍是验收要求。

共有导出绑定包含 enum 变体的真实 typed ID，nominal 的 `nested_bindings` 同时列出其静态命名空间中的嵌套类型、object value 与 enum 变体；变体归属由实际声明确定，不能误作包级值。`hir/cross-cone-interface/30` 更新该格式语义，旧 `/23` 及更早产物与缓存重建；既有 tag 不复用，不保留双轨 reader，runtime C ABI 和 String 表示保持。

M23-6 的 enum 模式验收覆盖实际跨 Cone 的 unit／payload 模式、嵌套 tuple、guard、穷尽性、类型别名和定义处默认值，并由第三个 Cone 消费后链接运行。模式与变体测试保留正文中的真实声明引用，不追加构造器访问资格；引用集合更新使用 `hir/cross-cone-interface/30`，旧 `/22` 及更早产物重建。

canonical ABI 导出复用同次完整 IR 的实际签名和 callable definition，删除重复逐参数 layout ID 表及只为该表存在的重放入口。共有 reader 以真实声明和 MIR lowered signature 核对 ABI，dispatch 按实际 receiver 查询表示。`lir/cross-cone-layout-abi/3` 退役 callable field 5，其余字段编号保持；旧 `/2` 产物与缓存重建，profile 和内容 fingerprint 按格式正常更新。tuple 的字段与参数使用完整结构身份和已有存储算法，不为嵌套值补造独立 nominal、layout/TD definition 或来源记录。

M23-6 的结构签名与字段验收包含实际 tuple 参数／结果、嵌套 tuple 字段、全 ZST 字段、enum tuple payload、成员／getter 及默认值的跨 Cone 编译、发布与运行。普通 callable 已生成的记录直接用于后续 MIR 组装、boxing 和 dispatch，不再生成一份重复绑定后丢弃。

共有 nominal 声明直接保存 struct 主构造器的 typed declaration ID，供前端按实际语言角色检查 `@NoGC` 调用；不能从参数形状、字段布局或 provider 身份推断主构造器。主构造器继续保留源码 Managed、物理 NoGC 的既有合同；值构造本身不分配，`@NoGC` 的参数、结果与局部值仍须 GC-free，managed 次构造器仍禁止调用。`hir/cross-cone-interface/30` 在 `NominalDeclarationDetailsV1` 新增 field 8：空数组表示无值主构造器，单元素数组保存其 constructor ID；有构造器的 struct 必须明确该引用，引用必须属于同一 nominal 的声明集合，其他 nominal 不得填写。旧 `/21` 及更早格式退役并要求重建，既有 tag 不复用，profile 与内容 fingerprint 正常更新；MIR/LIR callable 格式和 runtime ABI 不变。

M23-6 的源码消费验收包含跨 Cone struct 主／次构造器、命名／默认参数、ZST 与大值返回 ABI 的独立及组合用例。验收需由真实源码生成 provider 产物、下游生成完整产物并完成对象链接与运行；构造器按真实 typed target 消费既有 MIR/LIR 定义。

MIR 输出在 HIR→MIR 边界完成一次整模块结构、类型与实际外来 callable 检查，并同时保留已生成的 canonical foundation、共有依赖选择和完整物化引用。通用 MIR 输出保留完整泛型实体；Strong profile 的 ODR 能力门仍在其消费入口检查，并共享已有 canonical foundation。driver、MIR production 组装和 MIR→LIR 直接消费同一完整输出，不再从未变化的 Module 重建第二份 foundation、重复验证外来调用或重跑整模块检查。production 与模块之间仍核对实际 callable、入口和初始化关系；直接借用已有签名记录，不构造第二份预期桥表。外部新产物的格式、引用、ABI 与对象检查继续由 reader 负责。此清理不增加凭证、状态机、wire 字段或 profile 版本，不改变 runtime C ABI、String 表示或后续里程碑范围。

跨 Cone 的参数自由 class 继承直接使用依赖产物中的完整声明、真实基类、字段、接口与 dispatch selection。共有成员查询保存声明本身及其可见性，public、protected 和必要支持声明不改造成另一份 public 声明；前端按实际词法类和接收者静态类型执行访问、覆写及默认值规则。基类构造在已分配的派生对象上调用实际 provider 的 initializer，保持基类先于派生类的初始化顺序及移动 GC 接收者跟踪。继承的 virtual family 保留原 typed slot identity，派发表中的外来实现直接引用实际 callable；本地覆写只替换对应槽，未覆写的基类和接口选择完整传递。构造、super、成员与 getter/setter 沿共有调用、布局、ABI 和 relocation 路径消费，派生类再次发布后仍可由后续 Cone 使用。以上使用既有源码与机器声明格式，不增加来源资格、独立证明、平行来源表或后续里程碑能力。依赖虚槽的 lookup 域直接复用原声明已保存且读入检查的合同，保留 protected 与外层 owner 约束，不重建继承图。只有本次实际物化的 descriptor 和派发表产生物理外部引用；依赖类的查询数据本身不形成 relocation。受保护的嵌套类型通过实际 owner 的 child 声明引用参与限定名查询，不能要求 public binding 或将其重建为本地声明。前端分别检查全部词法 owner 的有效访问域和 protected 成员自身的接收者规则；嵌套类型中的 public 成员不会额外要求接收者属于访问者的子类。嵌套构造、成员、enum 变体与 object 继续使用共有 typed 选择及初始化路径，访问错误与签名泄露在前端诊断。 已发布的可物化 nominal 声明必须同时具有完整 dispatch 与有限 BoxedValue/CoroutineStep/CoroutineSlot 支持，包括受保护嵌套类型和实际表示所需的支持声明；producer 与 reader 从同一共有声明闭包取得这些需求，public binding 不控制机器支持的生成。source-only 声明及仅存在于当前私有实现、未进入发布声明闭包的类型不因此成为发布根。 reader 的声明查询不再区分“仅当前 provider 支持查询”模式；本地与依赖的签名、父类型、accessor 和 variant 都按实际 typed ID 查询完整声明，保留 kind、arity、owner 与依赖范围检查。

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

M23-6 的实际跨 Cone 验收包含 struct 类型、普通 final 成员、字段与计算属性读取、默认参数（含定义处字段引用及下游再次发布）、命名参数、operator、ZST 和含引用值。独立与组合 fixture 通过真实源码编译发布，再由下游仅凭产物消费；三个 Cone 的实际对象必须完成适用的链接和运行。共有 callable 查询支持 dispatch 引用普通成员，不能用历史 metadata 分区或来源资格限制合法使用。其余 M23-6 能力与清理仍按 [阶段设计](milestone23/stage6/DESIGN.md) 完成后验收，不以这部分通过代替整个里程碑。

M23-6 的框架清理以生产调用链为依据：移除未被共有 reader 使用的 HIR foundation/declaration transcript、来源绑定层及专用证明测试，保留实际声明查询、默认参数实例化和正常产物错误回归。真实源码发布与下游消费继续作为验收依据。

类型生产器直接返回完整的共有 type section；MIR 使用已有继承边、槽序、typed 实现目标及成员引用。类型物化和构造器选择查询已有共有 nominal/callable 声明，不另行投影来源 foundation、完整 protected 声明、构造器签名或参数协议来重复证明同次产出。删除这些副本的生产工厂、凭证外层、只用于副本的 reader 及测试。必要的类型、引用、继承、ABI 与格式检查保留在实际消费边界，未变化的数据复用已有检查结果。

所有可见性的 nominal、callable、property、参数、默认值和定义环境由共有源码接口完整保存。protected 成员仍以 typed 引用参与实际 MIR callable 选择，其可见性和签名读取同一声明；构造器使用共有 nominal 的 constructor 引用及对应 callable，不在 inheritance record 再保存一份 payload。generic 词法 owner 不因访问域查询而要求 machine exact type。名义类型的 lookup/inheritance/slot 三份派生域不再保存和重验。

`CrossConeTypeSemanticsSectionV1` 保留 field 1、2、3、8，field 4～7 退役；`NominalInheritanceInterfaceV1` 保留 field 1～4、7～9，field 5、6 退役，退役字段不复用。成员引用的 Constructor tag 2 随重复构造器通道退役，实际 constructor 始终使用共有 typed 声明。HIR `cross-cone-type-semantics/8`、required inventory、profile 与内容 fingerprint 同步更新，旧产物和缓存需重建；不保留旧来源副本的双轨兼容，不改变 runtime C 调用约定或 String 表示。

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。 依赖查询直接使用真实 `ConeIdentity` 与 callable、property、type-alias 的类型化声明 ID；选择集合按这些 ID 保存完整接口和实际依赖路径。删除独立 world/projection/selection 品牌、仅为品牌服务的计数器和错误，以及从声明 ID 再映射到局部 u32 的三套重复表。候选选择依照当前依赖目录中的实际声明与表示，不要求由同一查询实例铸造；直接依赖的名称可见性、转导出路径、实际 provider 和引用完整性继续按共有规则检查。HIR 依赖目录、候选和已选声明只保存实际 provider 与类型化声明引用，不逐项复制 artifact 坐标/fingerprint 凭证；provider 身份直接来自已导入的共有 foundation，不再提供独立 certificate 或重算坐标身份。直接依赖与传递依赖使用同一输入数据和查询实现，直接依赖集合只决定当前源码可见的 package/public binding；不以分离的输入、视图或 seed 包装授予枚举资格。转导出保留实际 binding route，删除逐候选的 provider 凭证、重复 terminal 声明及其包装；候选选择使用已解析的声明目录，不重放未变化的 route 与 provider 证明。HIR→MIR 使用同次编译的依赖快照及完整声明：共有依赖选择在关联实际 MIR 定义时核对一次目标、逻辑签名和 GC effect，lowering 随后按实际 provider 与 typed target 消费同一记录，不重复比较未变化的声明和签名，也不再次比较来源凭证。缺失定义及最终 MIR 输出的结构、类型与外部引用检查仍在各自边界保留。普通构建依赖记录与缓存 fingerprint 继续承担定位和失效职责。MIR 外部 callable 引用保存实际 provider 与类型化声明，不另映射到带会话品牌的局部编号；调用位置保留 GC effect。MIR 类型与 LIR layout/ABI 选择按实际 provider 和 typed target 直接返回完整记录，不先铸造并验证中间 handle；依赖闭包、类型、ABI、物理引用和 GC 契约仍在其消费边界检查。同一声明或 target 在另一选择集合中是否存在，按实际目录查询决定，不依据集合生成顺序或计数器。该进程内数据简化不改变 wire/profile、实体身份或 runtime ABI。

版本：0.2（草案）

配套文档：`docs/specs/SCOOP-IMPL-SPEC.md`（pipeline 与各 stage 职责）、`AGENTS.md`（编码准则）。

## 1. 策略：纵向主线 + 显式语言子集

不采用"逐 stage 完整实现"的横向推进，而是先打通最小端到端主线，再逐里程碑扩大语言子集。理由：

- **风险前置**：statepoint/stackmap、landingpad、GC pin/handle、suspend 状态机是本项目的新链路，越晚碰代价越大；
- **IR 设计需要下游反馈**：每个 stage 输出的真实需求由其消费者发现，孤立地"完整"实现某个 stage 几乎必然返工；
- **测试基建一次到位**：fixture runner、golden dump、negative fixture 断言在第一个里程碑建好，后续特性直接落入现成框架。

与 AGENTS.md"不留占位符"准则的调和：每个里程碑定义一个**显式的语言子集**，子集内的规则完整实现（结构完备、无 TODO 分支）；子集外的语法由 parser/HIR 报**正式的不支持诊断**（有位置、有信息）——这是面向用户的错误报告，不是代码中的占位分支。随着里程碑推进，这些诊断逐一消除。

## 2. 里程碑

每个里程碑都是全链路可运行的（parser → HIR → MIR → LIR → codegen → 可执行文件）。

### M0 技术 spike ✅（2026-08-21 完成）

- ~~独立一次性程序验证 inkwell 的 statepoint + stackmap + landingpad 全链路（不进主线代码）~~——`spikes/llvm-gc/`（独立 workspace），inkwell 0.10 + LLVM 22.1.8 验证通过：`rewrite-statepoints-for-gc` 正常改写、`.o` 含 `__llvm_stackmaps` 与 `gcc_except_tab`。该历史spike把pointer发射为address space 0，只证明metadata/EH通道打通，没有验证AS1 root、`gc.relocate`或root machine location；这些契约由M15的专用LLVM 22.1 qualification与artifact测试建立；
- ~~建立 fixture runner、golden dump 设施与 CLI 骨架~~——`compiler/driver/tests/fixtures.rs`（insta 快照），`scoopc` CLI 拆为 lib + 薄 bin（clap），冒烟 fixture `tests/fixtures/language/smoke/hello.scoop` 端到端通过。

### M1 hello world ✅（2026-08-22 完成，设计见 `docs/milestone1/DESIGN.md`）

顶层函数、`String` 字面量、`fun main`、调用 runtime 的 print。GC 用 always-leak 实现（分配即 malloc、不回收）；单 Cone；不插 statepoint。

### M2 值类型基础 ✅（2026-08-22 完成，设计见 `docs/milestone2/DESIGN.md`）

struct / tuple、字段访问、`val` / `var`、if / while、结构相等。

### M3 泛型与 Option ✅（2026-08-22 完成，设计见 `docs/milestone3/DESIGN.md`）

单态化、`Option<T>`、`T?`脱糖与`?.` / `?:` / `!!`（spec第7章）。M3首版将Option作为编译器内建，M4已迁移为`scoop.core`的真正泛型enum；M13边界修订后，export generic template、HIR生成的local concrete实例与MIR实体使用三类独立typed id，MIR不再直接读取template完成单态化。

### M4 enum 与模式匹配 ✅（2026-08-22 完成，设计见 `docs/milestone4/DESIGN.md`）

enum变体、when扩展模式、守卫、穷尽性、解构声明与`..`（spec第4、5章）。同时建立了sysroot框架（`sysroot/lib/scoop.core`），`Option`与`print`/`println`的硬编码定义正式迁移入core库；M13将tagged enum的扫描表示修订为pure-value共享payload、ref-bearing独占slot及不读取tag的固定ref偏移，可递归嵌入struct / tuple / class。

### M5 数组 ✅（2026-08-27 完成，设计见 `docs/milestone5/DESIGN.md`）

`Array<T>` / `MutableArray<T>`（M5暂为编译器内建，M14迁移为core generic intrinsic class）、字面量与推导规则、下标读写、`size`、构造函数形式互转（memcpy 快照）；越界 trap（M8 改异常）。数组 TD 携带递归元素扫描，支持含引用的 struct / tuple 与 tagged enum 内联元素。

### M6 引用类型层级 ✅（2026-08-28 完成，设计见 `docs/milestone6/DESIGN.md`）

class / 继承 / interface / 方法、vtable / itable 分派、装箱（spec 3、4.4、9.1；impl spec 2.9）。方法级 final / open / abstract 语义完整落地：类开放不隐式开放成员，final 调用 direct，open / abstract 调用 virtual，final override 保留继承槽。落地后顺带解锁：数组的 `toArray` / `toMutableArray` 方法形式、core 的 `class StringBuilder` 声明、`add<T>` 依赖的 `toString` 分发基础。

### M7 函数重载 ✅（2026-08-28 完成，设计见 `docs/milestone7/DESIGN.md`）

顶层函数与方法的 overload resolution（候选集分层 + 可应用性 + MSC，按 Kotlin 规范）；`print` / `println` 已迁移为 `scoop.core` 的普通重载定义。M7 首版由三个 `@Intrinsic` 原语支撑，M12 已把 `write` 迁为 Scoop ABI extern，只保留临时的值到字符串转换 intrinsic。

### M8 异常 ✅（2026-08-30 完成，设计见 `docs/milestone8/DESIGN.md`）

try / catch / finally / throw，landingpad 落地（runtime spec 第 5 章）。四条 trap 路径已全部改接真实异常：`!!` → `UnwrapException`、数组越界 → `IndexOutOfBoundsException`、`as` → `ClassCastException`、整数除零 → `ArithmeticException`（spec 11.7 已同步新增后者）。M8首版的`scoop_rt_throw`使用`__cxa_allocate_exception` + 拷贝；M25已在不改变语言语义和显式CFG的前提下用Scoop自有exception record与Level-I personality替换该实现。

### M9 真 GC ✅（2026-08-30 完成，设计见 `docs/milestone9/DESIGN.md`）

真 GC 替换 always-leak：Immix 核心（32KB block / 128B line、bump 分配、free-line 复用、标记-区域回收，1 GiB mmap arena）；statepoint 打开（GC strategy + `rewrite-statepoints-for-gc` + safepoint poll + stackmap，runtime v1 暂用经对象起点校验的保守栈扫描）；对象头扩为 16B（td + gc_word）；递归 TD 扫描描述；pin（对象头标志位）与 GcHandle 表落地；`scoop.core.gc` 包；写屏障卡片表（预偏置指针，为分代预留）；M1–M8 全部 fixture 在真 GC 下原样通过。M12 已把公开 pin/handle API 迁为 `@Unsafe` 普通 core 函数，仅保留 raw runtime boundary intrinsic。

### M10 协程 ✅（2026-08-31 完成，设计见 `docs/milestone10/DESIGN.md`）

命名`suspend`函数/方法、完全类型化的suspend状态机变换、`Continuation`与最小启动/挂起原语（spec 8.2、11.9；impl spec 2.3）。已完成MIR CFG化、HIR concrete化后再做MIR状态机变换、direct / virtual / exact interface hidden ABI、真实挂起/同步完成/失败恢复、异常物化及跨挂起`catch` / `finally`，并以强制GC验证嵌套frame/adapter链和递归扫描。高层协程构建器与调度器仍属标准库；M10以core的`SuspendTask` / `SuspendRegistration`适配器打通无lambda前置依赖的端到端闭环。

### M11 函数类型、函数值与 closure ✅（2026-08-31 完成，设计见 `docs/milestone11/DESIGN.md`）

已完成ordinary / suspend function type、lambda、匿名函数、局部函数、callable reference与捕获closure的全链路实现（spec 8.1；impl spec 2.2–2.4）。跨stage固定“HIR concrete化 → MIR closure conversion → coroutine transform”顺序；generic callable value、静态/动态函数型变adapter与suspend hidden ABI均保持完全类型化。M11采用类似Java lambda的保守capture边界，但以Scoop显式声明的不可变性为准，不推导effectively final：只允许捕获`val`、参数、`this`等不可重新绑定的binding。captured value type按concrete layout内联于closure，不隐式生成shared cell或boxing。closure/adapter都有独立TypeDescriptor与完整GC扫描描述，并已覆盖异常、真实挂起和强制GC。core已在M10的`SuspendTask` / `SuspendRegistration`协议上增加函数值形态适配重载。

M11 同时补齐 M7 预留的局部函数候选层，并把 managed 函数值与 native `FunPtr` 明确分开：只有下一里程碑在 `FunPtr<F>` 期望位置处理合格的顶层 `::name`，lambda/closure 不自动变成 native callback。

### M12 FFI 注解族 ✅（2026-08-31 完成，设计见 `docs/milestone12/DESIGN.md`）

已完成 `@Extern` / `@NoGC` / `@Unsafe` / `@Safe` / `@CLayout` / `@CallingConvention` / `@Global` / `@ThreadLocal` / `@InteriorMutable`、`value` / `ref` kind bound、显式调用类型实参以及 `Ptr` / `FunPtr` 的全链路实现（spec 第 13、14 章）。C ABI 通过编译器生成且带静态布局断言的canonical C bridge交给已验证system C compiler profile分类；Scoop ABI保持普通Scoop typed signature与direct ref，不经过C storage bridge（M15在不改变该ABI的前提下把caller实现修订为typed native-borrowed transition）。extern function/global/TLS、GC-free本地存储、CLayout struct双向传值、raw pointer操作、同线程同步静态 `@NoGC` callback、native root frame和core GC/output boundary迁移均已有独立IR golden与端到端fixture。

M12 只实现普通、非挂起的 FFI：`@Extern` 与 `suspend` 互斥，挂起函数也不能在 `FunPtr` 上下文中解析为原生地址。两种情况都由 HIR 直接诊断；不生成 wrapper，也不向外暴露 M10 hidden continuation ABI。`FunPtr<F>` 复用 M11 的正式函数类型与中性 `::name` 语法，但通过期望类型选择独立的 native ABI resolution。C ABI 只接受 GC-free C-FFI-safe值并经过 C bridge；Scoop ABI复用 typed managed ABI直接传 ref，跨 safepoint由 native root slot保活与更新。M12 callback只支持同步同线程的静态 `@NoGC` target；managed closure保活、异步调用和foreign-thread入口明确延后到M13。

### M13 多线程 GC 与 foreign-thread managed callback ✅（2026-09-01 完成，设计见 `docs/milestone13/DESIGN.md`）

M13 已将 M9 的单 mutator runtime升级为**多 mutator、stop-the-world、collector仍单线程且不移动**的基线，并补完 M12 明确延后的 GC-aware managed closure反向回调。它不引入语言级线程库，而是让 `pthread_create` 等带 opaque context的 C API 能安全地在 foreign thread上执行普通 Scoop closure。HIR输出同时完成`ExportHir` / `LocalConcreteHir`的typed-id隔离；fully specialized type及每个enum variant携带完备`gc_free: bool`，tagged enum采用GC-free共享slot、ref-bearing独占slot，niche仅用于与`Option<ref/Ptr/FunPtr>`结构同构的enum。

- runtime 建立显式 thread attach/detach、TLS thread state、栈边界、per-thread TLAB/native-root链和全局线程登记表；主线程、runtime创建的线程及 foreign thread使用同一套注册实体；
- GC 请求通过入口/回边 safepoint poll、native/runtime入口和线程状态完成 STW handshake。C ABI outbound call在多线程模式下发布 caller roots并进入 native-safe状态；Scoop ABI direct-ref callee只有在登记 native roots的显式入口参与协调，collector不得在其普通 native指令区间移动或扫描未知裸指针；
- 分配器、Immix block/free-line元数据、card table、handle/pin表、全局根与 native-root登记改为多线程安全。M13 只要求单线程 collector完成 STW tracing/sweep；精确根与moving compaction进入M15，parallel/concurrent marking和分代仍留 backlog；
- 新增 managed callback registration协议：Scoop侧以 ordinary、非 suspend closure和编译器生成的 typed invoke adapter注册一个 runtime-owned opaque token；token内部用 `GcHandle` 保活 closure，C侧只持有静态 C ABI trampoline与 GC-free context pointer；
- foreign callback入口执行 attach-if-needed → enter managed → invoke adapter → leave managed → detach-if-owned。参数/返回值必须是 C-FFI-safe，closure body可正常分配、触发 GC和调用 managed代码；这条路径独立于 M12 的静态 `FunPtr` / `@NoGC` target路径，不得通过放宽 `FunPtr` 来源规则实现；
- callback token具有显式 retain/release和一次性 ownership transfer规则；异常在反向边界内捕获并转换为 status/受管异常handle，绝不穿越 C frame。首个端到端 fixture用 `pthread_create`：registration分别保留worker ownership与join-observer ownership；创建失败释放两者，成功后新线程调用捕获 closure、释放worker ownership并detach，`join`侧以仍存活的observer读取完成/异常、按约定重新抛出后执行最终release；
- 任意 closure导出首版只支持具有显式 `void *user_data` / context槽的 C API；没有 context参数的 callback API仍只能使用 M12 静态 `FunPtr`，动态 executable trampoline/slot registry不在本里程碑；
- continuation完成/恢复状态改为原子协议，使普通 managed callback在 foreign thread中恢复既有 continuation时不产生数据竞争；调度器、`launch` / `async`、结构化并发、取消和 suspend FFI仍不属于 M13；
- 验收使用确定性barrier同时覆盖：多个mutator分配并强制GC、一个线程阻塞在native-safe C ABI调用、一个Scoop ABI direct-ref callee跨runtime入口使用native root、foreign thread反复attach/invoke/detach，以及callback token的retain/release、create失败和异常路径；
- M13只保证runtime/GC与callback token本身的线程安全，不把未同步的普通managed可变状态竞争定义为安全行为；跨线程共享数据必须由native同步原语或后续标准库memory model约束。

### M14 泛型类型、上界约束与接口化 ✅（2026-09-02 完成，设计见 `docs/milestone14/DESIGN.md`）

- generic nominal type统一模型：普通用户class/struct/enum/interface使用各自的ExportHir template → typed application → LocalConcreteHir specialization身份；既有generic struct/enum/interface迁出`declaration id + type args`旧表示，新增invariant generic class的构造推导、generic base/interface、宿主成员解析及单态化layout/TD/vtable/itable完整闭环；`is`/`as`/`as?`按完整application identity工作，不擦除type argument或接受裸generic目标；
- non-virtual generic method闭环：class/struct/enum method可声明自己的类型参数与bound；class generic method必须final，任何generic method都不能open/abstract/override、实现dispatch slot或进入vtable/itable。宿主参数前缀与method参数后缀使用不同typed identity，共同参与推导、callable reference、单态化及跨Cone template输出；
- 泛型上界约束：`T : Interface` 与 `where` 子句（spec 2.1/3.2 的既有语法落地）、有界类型参数上的方法解析（bounded method resolution）；
- `@Intrinsic`扩展到compiler-represented core type：Int/UInt/Boolean/String在core源码中显式声明ToString/Hash/equals等nominal能力；`Array<T>` / `MutableArray<T>`迁移为使用普通generic class身份的generic intrinsic representation family，删除独立built-in array type identity。固定表示与表示族都由typed kind/application提供，不伪装成零字段普通类型。生产模式只允许sysroot provider；compiler test可通过内部`CompileOptions`按input/Cone allowlist授权，且只放宽来源检查；
- `ToString` / `Hash` 接口落地（spec 11.11）：所有类型都通过普通implements/override显式adopt，不生成值类型派生conformance；`print` / `println` 改造为 `fun <T : ToString> print(v: T)`（单态化静态分发，退役 M7 的 `Any.toString()` 分发形态）；
- equals 的 operator fun 化（成员限定，spec 11.11）：class 的 `==` 走 `equals` 运算符，值类型的条件派生 `==`；vtable 前三槽（Any 方法）拆除；
- 受益方：M26 字符串插值可直接使用普通`add<T : ToString>`；同时退役现有按对象地址实现的过渡`Any.hashCode`/`toString`，避免把地址稳定性带入M15 moving collector。
- 附加完成M13后发现的IR完备性整改：MIR expression携带非可选类型；intrinsic、compiler-generated exception与function type canonical mapping全部类型化；LIR call完整携带target/signature/result/effect，pointer null保留provenance，layout/TypeDescriptor/dispatch只用typed identity连接；用sum type消除native global、foreign callback、caller root与enum field中的非法组合。所有信息由上游结构化地产生，删除下游按context、arena反扫、FQN/symbol或并行字段猜测/补齐的路径。Any typed method/fixed-slot问题随本里程碑主线拆槽自然消失，不作为独立附加项重复实现。

### M15 精确根、statepoint relocation 与 moving compaction ✅（2026-09-03 完成，设计见 `docs/milestone15/DESIGN.md`）

M15在M13的多mutator STW与M14清理后的对象语义之上，把GC从“只会mark、地址永远不变”推进为**单代、STW、单线程collector的moving Immix**。本里程碑首先是正确性门：所有managed ref都必须来自可枚举、可更新的root/field slot，不能继续依赖“旧地址碰巧还能用”。分代、parallel/concurrent collection不与moving一起引入。

- M15首个且唯一强制target为macOS/AArch64（Apple Silicon、Mach-O、LLVM stack map v3）；其他target在codegen前明确拒绝，不允许退回非移动/保守模式。请求级opaque typed target selection从完备capability选择runtime platform bundle；M23-2进一步把其中LIR-target、backend、C-bridge、runtime-build与final-link证明拆为五个不可互相推导的projection。Mach-O image、Darwin thread/VM及AArch64 frame/anchor分为可组合组件，通用stackmap parser、root visitor与collector不得包含平台分支。新增平台复用已有维度，只登记profile并补缺失组件；
- 编译器后端固定为与当前stable Rust一致的LLVM 22.1，本路线不包含LLVM升级。DarwinAArch64 profile固定使用22.1标准SelectionDAG/TargetMachine pipeline：GC pointer不进入vreg，post-RA `FixupStatepointCallerSaved`禁止callee-saved register root，`DeoptLiveIn`与透传原始LLVM backend option均禁止；因此managed GC root的产出契约是可写`Indirect [SP/FP + offset]`，object检查只是验证该后端不变量的防御性断言，不负责从任意LLVM产物中猜测能力；
- runtime从dyld已fixup的进程内`__LLVM_STACKMAPS,__llvm_stackmaps`精确解析并登记record，以每个parked线程的return address和受检stack location定位root；moving collection不允许回退到保守栈扫描。全局根、immortal/stable external object、native/compiler root slot、`GcHandle`、对象字段、数组/enum/tuple/closure/coroutine frame中的引用都必须通过统一的可改写slot visitor更新；
- runtime入口按类型隔离为generated managed薄入口、Scoop ABI native-borrowed入口、runtime internal实现及foreign callback gateway：只有managed薄入口捕获直接caller的PC/SP/FP；native-borrowed入口只扫描冻结caller roots和callee登记slot，不能把C frame冒充managed frame；
- outbound native-safe/native-borrowed在transition薄入口保留唯一零`gc-live`/relocate statepoint并捕获冻结segment的精确anchor，实际native machine call为普通nounwind调用。LIR完备caller-root plan只负责transition所在的顶层generated frame及含ref result slot；冻结segment的外层managed frame从transition anchor按stack map更新，不能要求callee反推任意外层caller liveness。返回managed后顶层值全部从caller-root/result slot reload，不能同时消费普通`gc.relocate`；
- `Managed` pointer发射为LLVM address space 1，raw/code/metadata保持address space 0。函数入口及每条循环回边由LIR显式输出带`SafepointId + StatepointLiveSet`的poll，codegen不得补插或重算roots；普通call的集合还必须包含LLVM 22.1会自动列入`gc-live`的可移动direct实参载体，并为RS4GC不会递归发现的aggregate实参拆叶。codegen验收必须检查`rewrite-statepoints-for-gc`之后的IR，而不只检查`gc.statepoint`和stackmap section存在：跨普通poll/call存活的managed ref必须产生并使用正确的`gc.relocate`结果。LLVM当前不可表达的exceptional relocation不作为依赖：managed invoke把两个后继活跃ref与可移动实参spill到带edge-role的显式compiler root frame，由codegen直接发射零`gc-live`的statepoint invoke，normal/unwind各自reload并pop，不得携带exceptional `gc.relocate`；
- M13建立的managed/raw/code/metadata pointer provenance继续保留；未pin的interior/derived pointer不得跨safepoint，必要时从relocated base重新计算。每个statepoint使用确定的typed id，post-RS4GC verifier与Mach-O/AArch64 artifact测试共同锁定root count、relocate dominance、return PC和受支持location kind；
- collector为被移动对象建立forwarding关系，将未pin存活对象evacuate到新line/block，再重写全部roots与heap引用。`PinnedPtr`指向的对象地址保持不变，pinned对象的出站引用仍须更新；`GcHandle`的generation/identity不变但slot内容更新到新地址。解除pin后，对象可在后续collection移动；
- Scoop永久不支持GC finalizer、析构回调或对象复活；M15在不可达判定、moving与reclaim中不调用managed用户代码。M24在不放宽这条边界的前提下增加同步GC-free release hook，专门用于兜底释放native resource；
- block header、free-block/free-line node、object-start、精确allocation size与forwarding等collector元数据全部迁出GC arena；可变长String/Array及large object不得从TypeDescriptor fixed size或block span反推复制长度；
- 增加专用**moving GC stress mode**：禁用TLAB/threshold绕过，使每次managed allocation都进入slow path并在分配新对象前执行一次完整moving compaction；collector内部的evacuation allocation不得递归触发stress collection。除pinned对象外，每个可移动存活对象都应在该轮取得不同地址，避免启发式evacuation因“这次没搬”掩盖悬空引用；
- stress mode完成全部root/heap slot重写并验证forwarding闭包后，清除旧object-start记录并用固定非法pattern poison完整旧副本。只要一个源block在本轮evacuation后不再含任何live/pinned对象，就立即`mprotect(PROT_NONE)`并在该stress进程余下生命周期内隔离、不重新交给allocator；为此block header、链表和free-list节点等collector元数据必须移到block外。含pinned对象而不能整块保护的block仍poison其中已迁出的旧副本；
- 验收强制覆盖：普通local/parameter/phi、含ref aggregate、递归对象图、数组/tagged enum、异常catch/materialize、closure与interface dispatch、协程挂起frame、`GcHandle`、pin/unpin、Scoop ABI native root reload，以及M13 foreign-thread callback/多mutator组合。测试必须断言未pin对象地址确实改变、所有合法引用仍指向同一identity，并以poison/`PROT_NONE`使故意保留的旧裸地址确定性失败；
- M1–M14全部fixture必须在普通moving模式下回归；选定的GC/FFI/closure/coroutine组合fixture必须在“每次allocation compact”的stress mode下通过。只有能生成stackmap、但runtime不消费或不更新root，不算完成M15。

### M16 统一约束系统与重载决议 ✅（2026-09-03 完成，设计见 `docs/milestone16/DESIGN.md`）

- 统一普通/local/member/extension函数、generic nominal构造、enum variant、operator与callable reference的候选及applicability入口；single-candidate不再绕过统一检查；
- 以candidate-local fresh variables、结构化equality/subtyping/kind/interface bound及postponed arguments替换M3/M7/M14分散的固定点绑定；lambda/callable reference/`None`/空数组/嵌套generic构造在同一session完成；
- MSC改为与本次actual inference隔离的pairwise fresh-variable forwarding constraint system，删除“比较推断后concrete type arguments”的简化；
- 失败候选不产生永久HIR实体，winner原子地产生唯一typed callee、完整owner/callable concrete arguments和argument adaptation；`LocalConcreteHir`不得含inference variable、constraint或export placeholder；
- 完成invariant nominal generic application、function type variance与bound范围；整数literal widen在对应语言能力落地时扩展同一solver。原context parameter规划由M27的运行期exact-match模型取代，不进入solver。

### M17 命名参数、默认参数与 `vararg` ✅（2026-09-04 完成，设计见 `docs/milestone17/DESIGN.md`）

- AST/parser正式区分位置、命名、spread与尾随lambda实参，以及required/default/vararg parameter；所有callable/constructor按候选独立映射；
- 默认表达式作为callable source interface在定义处完成绑定、类型检查及调用域覆盖检查，导出default只能引用export/re-export实体而不携带private/internal hidden dependency closure；winner及完整type arguments确定后，只在实际缺省处经同一实例化器hygienic展开。receiver先求值，显式实参按源码顺序求值，随后default按声明顺序求值；所有concrete expression都具有完备、类型隔离的definition/evaluation origin，default机制不识别具体intrinsic；
- `vararg T`的实际参数类型是`Array<T>`；位置element/spread产生fresh array，命名whole-array直接使用；当前invariant Array要求spread精确匹配`Array<T>`；
- 重载补齐候选参数名shape filter、“更少实际default”与“无vararg”优先规则；default/empty vararg不为generic inference虚构约束；
- `ExportHir`完整携带parameter calling shape、hygienic default template、definition origin及包含真实typed目标与定义位置的kind-specific引用；定义处与继承后的调用域覆盖检查由前端完成，产物不附加证明记录；local/export default使用不同id且不向下游stage泄漏，winner commit后`LocalConcreteHir`/MIR只见完整位置参数与普通typed array assembly；
- 覆盖普通/local/member/extension/generic function与method、class/struct主构造、constructor-style enum variant、abstract/interface/default inheritance及Scoop ABI extern；C `...`仍不支持。

### M18 callable 表面补齐（设计见 `docs/milestone18/DESIGN.md`）

- 完整operator声明角色、表达式映射、infix优先级与property-like `invoke`，全部复用M16/M17的candidate-local constraint、参数映射、MSC与求值协议；primitive/String/Array/Ptr能力由普通core operator声明提供，winner后才正规化为typed intrinsic；
- `++`/`--`、复合赋值与多参数下标使用typed place plan保证receiver/index/右值各求值一次；`LocalConcreteHir`前消除source operator与place计划；
- `?.method()`按Option分支lower，实参/default/vararg只在Some分支执行；结果始终再包一层Option，不展平`Option<Option<T>>`；
- `componentN`支持class位置解构并导出typed role；`iterator`与range operator表面进入M18，`for`/range core类型仍由M22消费。属性委托operator随M21定义reflection-free协议。

### M19 构造与初始化（设计见 `docs/milestone19/DESIGN.md`）

- class primary constructor补齐普通参数；class body加入带显式类型/initializer的stored property与按源码交错执行的`init`，并以field readiness和受限`InitializingThis`禁止读取未初始化字段或发布半初始化对象；
- class/struct secondary constructor复用M16/M17的候选、generic host推导与完整参数协议，委托图在HIR验证唯一typed target、termination和cycle；
- class构造改为一次exact allocation后在同一receiver上依次执行base、primary field、body initializer / `init`与secondary body；废除递归拼接继承字段的flattened constructor捷径；
- `super.method()`只在direct base member层决议并强制direct dispatch；constructor / initializer保持非挂起，普通suspend caller的显式构造实参仍可挂起；
- moving GC下initializing receiver和已写ref字段沿普通root/relocation传播，未写payload在首个safepoint前全零。

### M20 泛型类型系统第二阶段（设计见 `docs/milestone20/DESIGN.md`）

- 正式固定class/struct/enum/interface的全部nominal type parameter为invariant；删除interface旧variance bridge，不提供declaration/use-site `in`/`out`、star projection或capture conversion；
- generic算法通过callable自身type parameter与bound表达；application变换使用显式`map`/重建，未知application使用非generic interface或显式erased wrapper；expected type支持直接构造目标`Option<T>`等exact application；
- upper bound扩展为至多一个class加多个interface，bound均为参数完整的exact application；bounded receiver在实例化后解析为普通direct/virtual/interface target；
- 调用显式type argument list允许用`_`逐项继续推断；`_`复用M16同一candidate-local constraint/MSC内核，并在LocalConcrete HIR前完全消失；runtime、RTTI、dispatch和跨Cone metadata保持exact-only。

### M21 属性、对象与可见性（设计见 `docs/milestone21/DESIGN.md`）

- 统一logical property/accessor模型，覆盖stored/computed/extension/interface/delegated property、自定义getter/setter、override与typed place；property不再等同于field；
- 永久删除`lateinit`与隐藏未初始化状态。无accessor的`var p: Option<T>`/`var p: T?`可省略initializer，语义精确等价于在该初始化位置写`= None`；其他stored property仍必须完整初始化；
- reflection-free delegate协议不传`KProperty`/名称：可选`provideDelegate()`与必需`getValue(thisRef)`/`setValue(thisRef, value)`形成独立typed role；
- top-level/static nested `object`与non-generic companion使用线程安全exactly-once gate，只在完整初始化后发布；top-level runtime property在`main`前初始化，失败记忆、直接/间接循环与moving-GC root契约一次锁定；
- interface function/property accessor支持default body，按class hierarchy优先与唯一most-specific interface选择；冲突要求显式override，`super<I>`只direct调用direct superinterface default；
- 默认visibility改为`internal`，对外API逐项显式写`public`；四种visibility以typed access domain贯穿候选、override、signature exposure、M17 default的前端检查及`.slib`中的实际声明可见性。M12的raw `@Global`/`@ThreadLocal`与普通managed top-level property正式分离；
- 支持static nested nominal/object声明；`inner`/anonymous/local object、generic delegated extension及class/interface delegation仍不在本里程碑。

### M22 循环、值模式与定宽整数 ✅（2026-09-07 完成，设计见 `docs/milestone22/DESIGN.md`）

- `for`、不带标签的`break`/`continue`与typed loop target；统一while/for header，控制转移按目标cleanup深度穿越catch/finally/suspend状态，所有回边继续满足M15 poll契约；
- public exact `Iterator<T>`/`Iterable<T>`协议、Array/MutableArray迭代器，以及四个不同nominal identity的普通core `IntRange`/`UIntRange`/`LongRange`/`ULongRange`、`until`/`downTo`/`step`；
- struct命名字段副本更新，固定base只求值一次、RHS源码顺序与声明序重建；任何enum目标都是稳定编译错误，必须通过`when`匹配后显式重建；
- binding pattern与match pattern分流、命名字段递归subpattern、sound pattern-matrix完备性/witness，以及由import或唯一expected enum application驱动的通用裸variant；
- 八种signed/unsigned定宽整数、candidate-local literal fit、显式转换与全宽layout/C ABI；`Int`/`UInt`固定32位且`Int32`/`UInt32`为alias，`Long`/`ULong`固定64位且`Int64`/`UInt64`为alias；无上下文literal采用`Int → Long`/`UInt → ULong`默认阶梯，算术采用定义良好的wrapping并显式处理LLVM division/shift边界；既有64位source/core契约整体迁名为`Long`/`ULong`，包括Array size/index（上限仍`INT64_MAX`）、Hash、integer及String的compareTo、integer shift count、SourceLocation、`@CLayout`的aligned/packed与其他M22触及的API，内部machine metadata继续使用独立typed scalar；String length/index/slice尚未进入实现子集，由M26首次以`Long`表面引入；
- 当前可执行profile仍要求64位data/code pointer、全零null carrier及合法地址逐bit往返；`Ptr<T>`/`FunPtr<F>`迁为无公开representation field的compiler-represented family，阻断解构/copy update伪造；`Ptr`的raw/to与`sizeOf`/`alignOf`暂用`ULong`，element offset暂用`Long`，且只保留typed unsafe nonzero-ULong入口并要求pointee GC-free；`FunPtr`不提供源码constructor或integer转换，裸pointer固定非零、null只由`Option`的niche表示；platform-native integer及这些临时底层surface的最终迁移留待后续设计；
- 为固定宽度/Kotlin整数拼写及普通用户别名提供top-level非generic透明`typealias`；alias只有声明/可见性身份，不产生第二个类型/layout/RTTI/ABI。四种range本身不是alias；generic alias及真实跨Cone编码仍留后续。

### M23-1 source表面、parser与当前编译单元lookup ✅（2026-09-09 完成）

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage1/DESIGN.md`。

- 实现`package`、exact/star `import`、`as`和`public import`的AST/parser、文件头恢复与当前编译单元内package lookup；跨Cone target尚未开放时必须显式诊断，不伪造artifact或留下半成品AST节点。
- 完成门：新语法的parser golden、negative/组合fixture与旧fixture全量回归。

### M23-2 persistent identity与`.slib` wire基础

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage2/DESIGN.md`。

- 冻结Cone及kind-specific persistent identity、exact/source/unit/callable application/body/runtime/safepoint id、当前generic body所需的最小ODR group/member identity、`PersistentV1` mangler与session remap。callback source site使用允许binder的`PersistentCallbackRegistrationId`，fully concrete materialization使用`{ registration, CallableMaterializationContext }`的`PersistentCallbackApplicationId`；generic delegated initializer/ensure的local generic owner使用`EnclosingInitializationApplication { unit }`。Initialization generated template只引用声明级unit，application unit只进入materialization context并决定实际body root。box/coroutine step/slot/start统一按`ExactOwnerRoot`归属；该root不豁免helper依赖的generic nominal application/ODR能力。M23-3/6先预物化param-free source nominal的非callable有限shape-support closure，start 到 M23-7 随实际 ODR 定义与引用闭包加入；consumer不得替定义Cone发Strong定义。
- 统一冻结`ByteSpan`、`DomainSeparatedCborHash`与仅供callable body使用的`RuntimeEncode`；`.slib`冻结deterministic container、三层schema-1 metadata envelope、typed member directory、实际输入边界检查、基础Graph/Compile reader与schema演进规则。三条`identity-foundation/1`保存可重算的exact canonical payload；native witness只覆盖extern/callback边界的source nominal闭包，Scoop extern的`GcEffect::{Managed, NoGc}`与ordinary/suspend effect分离，不把foundation冒充通用layout服务。
- M23-2只持久化`ValidatedLirTargetSelection { lir_target, backend }`；请求级registry原子解析`ResolvedTargetProfile`的`lir_target/backend/c_bridge_toolchain/runtime_build/final_link`五个projection。canonical C signature只描述generated-C source storage，不持久化完整target C classifier；LLVM candidate绑定`ValidatedBackendProfile`，generated-C candidate绑定`ValidatedCBridgeToolchainProfile`。bridge recipe使用producer-independent `GeneratedBridgeUnitId`，实际定义使用producer-specific `GeneratedBridgeAtomId`，LIR/ODR relocation引用unit并由object verifier从atom规范化回unit。
- member envelope从本阶段起允许任意数量、任意已登记producer的`LinkObject`以及opaque/required blob；成员用途不依赖文件名、扩展名、顺序或object数量。后续语义payload按独立section/capability version加入，不在尚无verifier时宣称最终Link view完成。

### M23-3 single-Cone artifact与core分离 ✅（2026-09-16 完成）

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage3/DESIGN.md`。

- 引入`Cone.toml`、source discovery、library/executable entry sum及manifest/protocol边界；`scoopc`每次只消费显式上游`.slib`闭包并产生当前Cone `.slib`，不搜索、递归、构建runtime或最终链接。
- `scoop.core`成为trusted独立library artifact；先闭环core-only编译，其他Cone dependency稳定拒绝到M23-5。实现compiler侧per-Cone image producer、strong-only六类registration/image digest、member-aware definition/undefined requirement、当前Scoop/generated `LinkObject` verifier与基础Link view，使本阶段成功artifact已通过Compile/Link双view；core中可跨Cone引用的param-free source nominal同时验证并物化M23-2冻结的有限shape-support closure。production profile必须拒绝任意ODR group/member/body/symbol，直到M23-7具备完整证明。single-file synthetic request固定为一个source、executable、core-only Cone dependency，其`.slib`只能作为local executable root artifact。

### M23-4 resolved build graph与调度 ✅（2026-09-16 完成）

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage4/DESIGN.md`。

- `compiler/scoop` orchestration library解析exact locator和静态无环、同`group:name`单版本的DAG，拥有cache与dependency-first子进程调度。M23-6 将原 Compile/Link 双重完成门禁收缩为父进程的归档/摘要一致性检查，完整 typed IR 与对象由实际编译器消费者读取。
- 以recording artifacts和core-only真实节点完成chain/diamond/cycle、ambiguous locator、stale dependency、cache失效与child失败传播；本阶段不提前开放跨Cone源码名称。

### M23-5 多Cone名称语义 ✅（2026-09-19 完成）

总体设计见`docs/milestone23/DESIGN.md`，阶段详细设计见`docs/milestone23/stage5/DESIGN.md`。

- 落地direct/support closure、cross-Cone exact/star/alias import、re-export、public/internal/private access provenance、default、non-generic alias与selected HIR/MIR/LIR metadata；新增cross-Cone strong profile，成功machine-use只开放已解析基础类型构成的 const 及签名完全由这些参数自由类型构成的非generic top-level/extension callable或property accessor。凡需尚未具备的cross-Cone layout/dispatch或物化实现的使用稳定拒绝到M23-6，不由consumer临时发Strong定义；receiver-dependent protected、generic或direct source-extern能力分别拒绝到M23-6、M23-7或M23-10，不产生残缺IR。
- 完成direct/transitive可见性、split package、链式re-export、negative lookup observation及semantic cache失效矩阵；每个成功用例仍产生双view有效artifact。

### M23-6 跨Cone layout、typed ABI与ZST

本地 class、struct 和 enum 实现参数自由的依赖接口时，前端直接消费共有接口声明的完整父接口、typed slot、签名、默认实现和访问域。HIR 的 conformance 引用实际接口类型及本地/外来槽声明，目标为本地方法 application 或共有依赖 callable；不得为复用本地检查而复制外来函数声明、正文或生成同名替身。MIR 的 dispatch 表保留本地函数或实际外部 callable 引用，默认实现与抽象槽 trap 沿定义方原有 target 解析，LIR 使用现有 canonical ABI、外部定义与 relocation 路径。override、缺失实现、默认方法冲突、setter 能力和签名/effect 规则在同一前端检查中完成；类型、成员和 dispatch 独立及组合场景须经真实源码产物消费和单 image 普通/移动 GC 运行验收。

本地 interface 可以继承参数自由的依赖接口。父边保留实际 TypeId，override 关系保留实际本地或外来槽声明；继承的成员按父接口声明顺序进入完整槽表，菱形继承按声明身份去重，被覆盖的槽按已解析 override 关系消除。显式成员及当前 this 的隐式成员查找沿本地与依赖声明的同一父图进行，本地和外来候选共同执行语言规定的适用性与最具体选择；不能以声明存储位置决定优先级，也不能将外来成员复制为本地声明。该接口再次发布后，下游按实际父类型、槽与 provider 消费，保持 canonical ABI、默认方法、属性和装箱语义。

抽象 dispatch 目标与具体实现一样保留实际声明：Export HIR 的抽象 conformance 携带真实方法 application 或外来 callable 引用，source selection 携带所选 abstract 声明，完整 slot contract 携带该声明的 owner、signature、effect、modality 与访问域。最近的 class 抽象声明以及更具体 interface 的抽象 override 均压制原默认实现；不把原槽声明伪装成所选目标。MIR、LIR 和 Compile/Link 按该 typed target 取得真实 trap、ABI 与 relocation，不再扫描所有 callable、重建整份继承图或沿继承链反向推测抽象目标。槽身份与所选声明身份可以不同，双方仍须满足实际继承、签名和访问合同。正常路径复用完整记录，不增加来源凭证或第二套证明表。

限定 `super<I>` 通过当前 owner 直接列出的真实接口类型查找成员，本地接口和依赖接口使用同一候选决议、参数默认值与可见性规则。普通 `super` 对 class 成员的候选限制只用于实际基类接收者；限定 `super<I>` 使用 I 的接口成员集合，不能因共用 DirectSuper 调用方式而被该限制排除。调用选中的具体默认方法或 getter/setter 时，HIR 保存实际声明及强制 direct 调用方式；接收者沿共有引用转换或值装箱路径适配，MIR/LIR 按该声明的 canonical ABI 与 provider 定义发射，不再次进入接口表。抽象目标、非直接父接口、错误参数与初始化期间的调用仍在前端诊断。导出的默认参数正文用既有 `DirectSuperMethodCall` 节点保留该语义，下游展开继续调用同一真实目标。正文与引用集合复用同一 callable 引用转换，完整保留实际方法 owner，不分别重建不同的引用记录。同一 provider 的同一 typed callable 被 direct 和 dispatch 多次使用时，MIR 依赖记录与物理定义只选择一次，各调用点仍保留各自的派发方式。此项不新增来源资格、证明表、wire tag 或 runtime ABI；验收覆盖普通与 ZST 接收者、属性读写、命名及默认参数、大值结果和再次发布后的消费。具有隐式接收者的成员正文仍按普通值查找规则消费依赖属性、object 与 enum 变体；这些值与本地值使用同一个解析结果，不能归入函数或类型的非值阻断层。

前端在名称查找、override coverage 和 signature exposure 的负责位置执行访问域检查；成功后直接保留 typed 声明引用、实际继承/override 关系及最终 lookup/slot 域。删除 `LookupAccessWitness`、`OverrideAccessWitness`、`PropertyOverrideAccessWitness`、`SignatureExposureWitness` 及仅携带这些记录的候选资格状态。后续候选物化、IR 与产物生成不复制访问域证明，不用 witness 的有无替代实际声明关系。可见性错误、protected 接收者规则、签名泄露检查和独立 setter 槽规则保持；不改变 wire、profile、runtime C ABI 或 String 表示。

外来 object 的实际消费须覆盖类型和值导入、普通成员、初始化中的读取、转导出或默认参数，以及再次发布后的下游消费。源码访问调用提供方的实际 ensure 并读取同一 published-root，链接与移动 GC 验证唯一实例和正常根登记；不复制 foreign storage，也不以额外来源资格替代该闭环。 共有 HIR `/28` 使用新的 SingletonValue 角色 tag 7 保留真实求值位置，旧 `/27` 产物和缓存重建；MIR、LIR 与 runtime 继续消费既有 object/unit/storage 格式。

清理 concretizer 入口的整模块迭代语义重放及其独立 IR/meta 验证实现，删除仅服务该通道的伪造 HIR 测试和修改工厂。保留真实源码的 iterator 选择、类型、effect、默认值、解构和循环控制回归；正常格式与引用检查仍由实际产物边界负责。

跨 Cone 的 `throw`、语句及表达式形式的 `catch` 使用前端已解析的实际 `Throwable` 声明，沿共有依赖类型查询取得继承关系并执行同一子类型规则；导入协议不允许跳过检查。同名普通 class 不能替代该实体。异常构造、默认参数、调用、布局、TD 和展开使用共有路径，保留调用求值顺序、catch 顺序与 finally 语义；此能力不增加协议资格、独立证明、wire 字段或 runtime ABI。 公开存储属性的 getter 与可公开调用的 setter 必须按实际 owner 和声明类型生成普通 callable body，即使 provider 的正文没有引用该属性；名义类型、顶层属性与 core 使用同一规则。参数自由且可物化的属性自动导出实际 body。泛型或 source-only owner 仅用于签名和表示查询时，不自动实例化其普通方法或 accessor；实际调用仍通过同一 typed 请求生成完整实例，dispatch 所需的方法继续随其实际类型物化。

跨 Cone 的强制 `as` 沿实际 `ClassCastException` 声明查询完整异常类型，并选择该声明的零参数 constructor 或默认参数适配入口。成功检查保留原对象身份，失败通过普通 class 分配、外部 initializer 调用和 throw 执行；别名、interface 与装箱值沿同一类型与 ABI 路径处理。默认参数中的转换在实际展开时进入相同路径，未求值模板不触发机器物化。异常类的表示仍依赖后续泛型能力时，前端在输出 LocalConcrete HIR 前对实际执行点给出已有 layout-required 诊断；不输出缺少所需表示的成功 IR。MIR 直接消费完整协议中的 typed 声明引用，不再建立丢弃外来声明信息的 Core/Imported 资格投影。所选 constructor 使用共有依赖 callable 集合，保留真实逻辑/物理签名、GC effect、ABI 和 relocation，不增加转换调用的证明记录、wire 字段或 runtime ABI。零参数默认值 adapter 以已有 GeneratedCallable 实体及 ClassInitializer 表示进入共有 callable 导出，不能只发布协议引用而遗漏定义。MIR/LIR selected callable 记录本身是机器依赖引用；reader 按 provider、typed target、定义、签名和传递依赖检查其完整性，不要求隐式调用另附一份 HIR 操作资格记录。

共有 HIR 类型位置的结构、foreign nominal 分发、真实 provider 与定义/求值位置在 HIR reader 边界检查一次。后续物化查询和 MIR/LIR 消费同一未变化的 typed 记录，不重新完整检查这组 HIR 关系，不建立额外验证状态或凭证；实际类型表示、签名、ABI、对象与传递引用仍由相应边界检查。外部字节重新读入或相关数据发生变化时重新验证受影响部分。这一职责调整不改变 wire、内容 fingerprint 的字段组成或 runtime C ABI。

M23-6 删除没有实际 producer/consumer 的 `ScoopRuntimeCoreBindingsV1`、`ScoopProgramDescriptorV1`、固定 String capability ID 及其 LLVM 布局镜像和专用测试，不把它们改名后保留。未使用的 program/core magic `0x53434f4f50505247`、`0x53434f4f50434f52` 退役且不复用。实际发射的 image、root entry、type/callable、storage、immortal、initialization 和 safepoint 记录继续使用既有格式；这项删除不改变实际 runtime C ABI、String 表示或其 fingerprint。M23-8/9 的多 image 启动与 program-link 仍留在后续阶段，按实际入口和引用需要定义数据，不提前冻结新的 program record 或另建 String 授权表。

协议数据清理删除重复 String source/exact capability 和独立 definitions 外层，`hir/core-bootstrap-interface/4` 直接保存完整 typed 角色。旧 section field 4 退役，field 5 保存实际本地产物定义；类型身份、初始化服务与 ABI 继续通过共有路径消费，旧 `/1`～`/3` 产物与缓存重建。

String descriptor 使用完整 MIR 中实际声明的 source exact identity，沿共有 descriptor 查询、layout selection、physical import、registration 和 Link relocation 消费。删除独立 String bridge 与固定角色的 descriptor 恢复通道，不以 provider 坐标或协议来源豁免普通引用检查。Strong production `/7`、`/8` 退役原服务表中的 TD tag 2；旧产物与缓存重建，String 表示及 runtime C ABI 不变。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

Strong production 的两种表示当前使用 `/13`、`/14`：删除初始化专用 ABI field 11 和外部服务表 field 12，section 保留 field 2～9，并由新增 field 13 保存实际 callable 正文的 canonical LIR 摘要，共九字段。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

总体设计见 [M23 设计](milestone23/DESIGN.md)，详细设计见 [M23-6](milestone23/stage6/DESIGN.md)，清理范围见 [清理设计](milestone23/stage6/CORE-AUTHORITY-CLEANUP.md)。

- 完成共有 `ValueStorageLayout`、canonical Scoop ABI、scan/TypeDescriptor、ZST payload elision、boxing/address/static token、Array/Ptr 与 C 边界；完成 param-free 跨 Cone 类型、constructor/member/object、继承、dispatch/protected 和有限 shape-support 的实际发布与消费。
- core 作为普通 library 使用相同构建、缓存、依赖、类型查询和产物消费。完成 canonical ABI、native boundary、compiler protocol、String/初始化及 Link 路径中的专用资格清理。已完成部分核对生产调用链并复用。
- 外来 enum 构造直接使用共有声明、参数与默认值，覆盖限定名、直接导入、别名、期望类型及第三 Cone 默认值展开；不要求额外外部机器构造函数。
- 默认值源码位置复用共有 HIR 接口的完整正文和实际使用记录；删除为位置收集保留的旧默认值来源生产器、平行表与专用测试，不新增替代工厂。
- 清除 compiler/slib/runtime 的通用资源预算、计费与配额，删除成本策略与 profile/fingerprint/runtime ABI 的绑定。保留实际范围、整数溢出、非法环与 GC 契约检查。
- 清除额外来源授权、防伪、重复证明和测试专用来源工厂；默认参数、名称解析和语言规则由前端负责，IR/meta 与 reader 不再另建语言语义实现。Compile/Link 共享同一不可变语义结果；发布与 runtime 热点不反复完整重放。
- producer、reader、linker、wire/profile、版本、fingerprint、fixture、golden 和文档同步。退役 tag 不复用，不兼容产物重建。保留实际 C ABI、String 表示和 typed identity。
- 共有默认值和受限声明只保存一份 typed 协议与正文；退役 type-semantics field 5～7 及嵌套源码 payload 的 field 3，capability /8 要求旧产物重建。
- 正式 `scoopc` 发布与 `scoop` 依赖消费接入共有 `CrossConeLayoutStrong` reader；语义会话直接接收完整类型、ABI 与真实对象，移除旧 M23-5 的重复提交和 Link 消费路径，旧依赖重建。
- 正式发布直接保存同次编译完成的归档和普通摘要，回读核对 bytes；删除旧 M23-5 writer 与双版本发布包装，外部产物继续通过共有 reader 检查。

- 构建完成节点、prebuilt 与 cache 复用普通 manifest 摘要和不可变归档；父进程不重新读取完整语义/对象或全部依赖。编译器消费边界保留完整类型、ABI、格式与 Link 检查，缓存失效和 child 结果核对继续执行。
- 真实源码生成完整 `.slib`，由本阶段跨 Cone Compile/Link 路径消费，并完成适用链接与运行；覆盖 core 修改、扩展、重建、下游使用，以及类型、成员、dispatch、ABI、ZST 的独立与组合场景。手工 metadata 和证明反例不能替代验收。
- **M23-6 已完成并验收（2026-09-27）**：实际功能、七项清理和生产调用链核对已完成，变更按功能提交；启用真实配套编译器的全仓测试 5084 项通过。详见 [实际产物验收记录](milestone23/stage6/ACCEPTANCE.md)。ODR、multi-image startup、artifact-only program-link 按 M23-7/8/9 的原阶段安排。

### M23-6a 统一 HIR 语义模型、产物消费与具体化（[详细设计](milestone23/stage6a/DESIGN.md)，已完成）

- 插入 M23-6 与 M23-7 之间；既有 Stage 7 实现与真实 fixture 作为迁移基线，后续阶段以 6a 完成门为共同前置条件。
- 当前与依赖声明使用相同的 typed nominal/application、成员、字段、conformance 和正文；源码与 `.slib` 解码产生同结构 HIR，Export HIR 是共同语义图的导出投影，LocalConcrete HIR 保持独立 ID 和无 binder 的完成边界。
- 合并完整候选／参数／上下文推断、默认值展开和具体化，删除 Imported 类型／操作、导入专用执行器、transport→imported template 及本地 arena 假设；保留原实体、可见性、定义位置和求值顺序。
- 按完整 application 和实际需求闭合物化，修复普通宿主的封闭泛型父类型、整数范围 iterator 去重、外来类型解构和函数值默认参数；上游普通存储、初始化和实现不复制。
- 验收同一导出图内存／wire 消费等价、合法声明位置变化、模板与实参来自不同 Cone、真实再次发布和适用的链接／移动 GC；同步实际格式迁移，删除被替代路径，不能只增加共同 facade。
- **M23-6a 已完成并验收（2026-10-01）**：共同声明、正文、调用与具体化已切换，既有语义缺口和收尾求值顺序问题已修复。完整 workspace 首轮 5294 项通过、17 项旧快照／断言失败；核对更新后，使用文件完全相同的配套编译器严格复验这 17 项，5311 项全部覆盖通过、无忽略项。详见 [实际验收](milestone23/stage6a/ACCEPTANCE.md)。

### M23-7 跨 Cone generic、ODR 与 generic delegated extension ✅（2026-10-01 完成，[详细设计](milestone23/stage7/DESIGN.md)，[实际验收](milestone23/stage7/ACCEPTANCE.md)）

M23-6a 的共同 HIR 前置条件已经验收；本阶段的实际机器定义、ODR 和泛型委托运行闭环已完成。源码与产物沿共同语义和具体化路径进入机器输出。

- 消费 6a 的共有 `.slib` 模板、共同具体化与完整类型事实，完成泛型函数/名义类型、constructor/member/default/bound、hidden support 的机器定义／引用和 generic delegate 运行；复用当前 MIR/LIR、对象集合、registration、reader 和发布路径。参数自由 source nominal 的 start 由定义方补齐，generic application 按实际引用闭合，不递归生成全部 helper。
- 普通顶层 stored property 统一生成声明方访问器，泛型正文、嵌套 callable、构造、默认参数和 delegate initializer 共用原状态、初始化单元与 GC root；纯静态 GC-free 存储访问器保留 NoGc 合同。普通属性、泛型 enum payload、含引用大值与 ZST 的再次发布、重复实例合并及移动 GC 已有真实产物回归；本地和外来 Strong 类型描述符的对象引用统一为同一 exact type 定义，`link-identity-closure/6` 替代 `/5`。
- 外来 core 的 `Option` 短写、泛型 enum 变体、默认值与静态 `None` 已接通共有声明和具体化路径；限定／导入／上下文构造、空安全运算、转换、含引用大值和 ZST 均有再次发布与移动 GC 回归。调换 core 变体顺序并移走源码后，下游仍按实际协议身份消费与运行。
- 数组模板、转换构造与函数 vararg 调用已接通实际 core owner、共有类型与普通物化；数组读写、转换成员／构造、别名与同名重载、默认值、复制 identity、函数引用和迭代成员有再次发布及移动 GC 回归。导入泛型调用复用整组实参的约束固定点，后续实参可为前面的空数组、裸变体、lambda 和函数引用提供上下文，求值顺序保持源码序。数组 instance／element scan 与派发表保留各自真实身份，递归 scan 的完整字节归原对象定义；具体范围与验证见 [进度记录](milestone23/stage7/PROGRESS.md)。
- `for` 在 Export HIR 前展开为普通调用、接口适配、Option 操作与 while，当前共有 HIR 为 `/43`，专用 For 和 portable binding-plan 编码已撤销。数组与本地／外来泛型迭代器、普通默认表达式、逐轮捕获、跳转和 finally 已有真实再次发布与移动 GC 回归；普通宿主的泛型父类型与整数范围迭代、消费者源码对外来类型的直接解构、外来函数值默认参数已由 6a 的共同 HIR 修正，本阶段已完成相应的机器闭包、ODR 与委托运行验收。
- 跨 Cone 首次取得参数自由 `@NoGC` 源函数的 C 地址已接通：定义方发布 C-safe 函数的 storage bridge，消费方使用原 typed callable 并按实际取址生成 C trampoline。公开、私有模板支持、精确重载、默认值、再次转导出、大值及指针写回均完成真实产物链接、C 调用和移动 GC 验证；自动属性访问器不生成此桥。storage bridge 使用共有 MIR 的 role 12（当前 type bridge 为 `/6`），C 调用 ABI 保持。
- 函数静态／动态适配已覆盖指针装箱、嵌套函数、lambda 结果、接口继承、私有 tuple 签名和 generic delegate initializer，全部通过再次发布、真实链接和移动 GC。closure 使用精确 FunctionShape parent 及固定 Any 动态 invoke，下游目标集合不改变共同 TD；类型关系随 cone-production `/3` 和 runtime metadata ABI 2 发布。原有真实产物回归继续通过。
- 跨 Cone 协程已接通实际 core 协议、状态机／函数型变、完整隐藏 ABI 和参数自由 source exact 的有限 start 发布；共有 MIR type bridge 升至 `/6`。再次发布、真实挂起／恢复、异常与 finally、值类型 task、24-byte 含引用结果以及调换 core 槽声明顺序均通过普通与移动 GC 运行。关闭更新并启用实际配套编译器的最终全仓 **5333 项**通过，无失败、忽略或警告。
- 保留四类 specialization 和既有 group/member identity；按重复 member 的完整 ABI、canonical LIR、对象/EH/stackmap 判等，独立 helper 的成员集合取并集。旧“同组全部成员必须相同”规则会拒绝不同源签名到同一目标类型的合法 adapter，现按实际定义与引用修订；不增加授权、预算或证明体系。不同源签名到共同目标类型的两组真实 sibling adapter 已完成独立成员并集、共同定义／ABI／对象比较、实际 TD 和派发地址合并，普通与移动 GC 均通过；不同 exact type 的 TD 保持不同址。
- 重建 core 专项直接修改数组 iterator 正文并新增泛型 class／方法／函数及私有 helper，移走源码后完成实际消费、再次发布、转换构造、Option／函数引用和移动 GC；下游观察到提供方实际正文的新结果。
- 真实泛型依赖缓存已验证正文、私有 helper、默认值、约束和实参类型变化的重编译范围；core 与未变节点命中缓存，6 轮重复输入的编译调用列表为空且产物字节一致。
- 已删除无生产调用的旧 Strong 专用 writer、driver 收窄错误和 HIR 遗留错误项，格式测试复用已有 canonical archive 构造器。最终逐项验收、运行与格式证据见 [验收记录](milestone23/stage7/ACCEPTANCE.md)。
- 正式产物沿原完整 layout 路径切换到 `cross-cone-generic/1`，模板、定义目录、逐 member fingerprint 与缓存同步迁移；required section 按实际 payload 分步升级，不预填尚未实现的模板或保留平行发布器。完成门包含 provider 源码移走后的下游编译，以及现有单 image 验收入口的真实链接、地址合并、委托初始化/失败共享和移动 GC；生产多 image 启动与正式 program-link 仍留给 M23-8/9。

### M23-8 runtime multi-image registry与启动

详细设计见 [M23-8](milestone23/stage8/DESIGN.md)，状态：已完成并验收（2026-10-02）；实际运行、格式迁移与完整回归见 [验收记录](milestone23/stage8/ACCEPTANCE.md)。

- 依赖已验收的 6a 共同 HIR 与 Stage 7 完整机器产物，直接消费实际 image/root entry 和六类 registration；不重新解释 HIR、具体化模板或补发 helper。
- 以 `scoop_rt_run_program(images, image_count, root_entry)` 接通生产启动，统一 initialization registration 与 coordinator 参数，删除旧 coordinator 副本。登记全部 image 后形成 canonical Kahn/eager 顺序，每次 no-throw gateway 独立执行 EntryPending/epoch 握手；root catch 先物化 managed 异常再结束 native catch。
- 扩展现有 stackmap parser，消费全部 LLVM v3 blob，按实际 registration、owner、原始 PC 与规范化 payload 核对 ODR 重复。GC 复用唯一静态 root/immortal/TD/PC 索引；不重算 RuntimeImage/Graph/ODR 全图或增加来源凭证。
- 按实现规范 2.14 同批迁移 metadata ABI 3、generic profile /2、初始化 production/object 格式与缓存。完成门包含实际 3+ Cone image、Strong/ODR 地址与独立 member 并集、eager/lazy、failure/cycle、函数值/协程、ZST/大值/引用、moving GC、受控线程竞争和损坏 metadata；正式启动对象、runtime-build 与 artifact-only program-link 留给 M23-9。

### M23-9 基础 artifact-only program-link

详细设计见 [M23-9](milestone23/stage9/DESIGN.md)，状态：已完成并验收（2026-10-02）；真实产物链接、运行矩阵与完整回归见 [验收记录](milestone23/stage9/ACCEPTANCE.md)。

- 继承已验收 Stage 8；独立 Link reader 复用原 foundation 的 identity/ABI、production/import 和对象完成读取，不再重放完整 HIR/MIR 或具体化模板。新增只补 runtime 数据 alias 的 `link-support/1`，required inventory／profile fingerprint 更新，旧产物和缓存重建；现有语言与 runtime ABI 保持。
- `compiler/linker` 提供库及低层 `scoop-link`，全新进程只凭 `.slib` 闭包、普通 runtime 对象索引和明确 target/toolchain 工作。`scoop` 编排 runtime 源文件／头文件构建与内容缓存；program-link 只编译自己生成的 startup C，不读 Scoop/runtime 源码、不加载 LLVM。
- 正式 C main 引用原 image/root 并调用 `scoop_rt_run_program`，String 以实际 TD 同地址 alias 接入。直接使用明确系统 linker、libSystem 和固定 flags，不依赖 `cc` 默认搜索或 archive；SourceExtern 统一合并声明合同并从实际对象／系统 export 解析，不设 core API 白名单；缺失符号与合同冲突同样报错。
- 复用逐 member ODR 和原对象检查，final verifier 只核对链接新产生的 entry、绑定、地址合并、fixup 与 section 保留；Stage 8 负责实际加载地址／权限与精确 stackmap。没有 runtime/program 来源凭证、平台认证或重复完整语义重放。
- 完成门包含多 Cone/multi-object、ODR member 并集、eager/lazy/failed/cycle、moving GC、exception gateway、ZST/大值/引用、重建 core、runtime cache 失效和原子输出。新 native library 输入的定位与供应留到 M23-10，umbrella CLI 总迁移留到 M23-11；本阶段不实现 final-link cache。

### M23-10 一般 native 输入与链接闭包

详细设计见 [M23-10](milestone23/stage10/DESIGN.md)，状态：已完成并验收（2026-10-02）；真实产物、功能组合、完整回归与分批提交见 [验收记录](milestone23/stage10/ACCEPTANCE.md)。前置条件为已验收的 M23-9。

- 继承共同 HIR 已选的 native 签名／effects 和 MIR/LIR 的实际 ABI requirement；直接、默认值、泛型实例和依赖调用共用前端规则。补齐外来 global／TLS、GC intrinsic、callback 协议和泛型 FunPtr 的共有消费；program-link 只解析已有 requirement 对应的 native 输入。
- 复用 M23-9 的 extern、对象、startup 与最终检查，增加 direct object/archive/dylib/framework 的明确候选解析；archive 在有限符号工作集中选择成员并独立交给 ld，dynamic 保留 export/load/re-export，从已选 binding 生成标准链接 stub 以保证重名 export 下的显式选库，并检查实际 two-level binding。输入快照、map/trace、全局数据／TLS 和实际 RPATH 均进入对应链接职责。
- 普通 native 输入不要求 Scoop producer 来源凭证或通用 verifier/effect 框架；删除逐 selection/contribution/binding 的证明外层及未选 archive 成员的完整效果拒绝。不为不存在的调用方新增 blob handler、Cone 内 C/C++ producer 或 whole-archive 模式；unknown Link-required capability 继续诊断。
- native 候选经 `.slib` 已有 typed requirement 和显式 `--library-path` 定位，不直接注入 raw `.o`、archive、linker option 或 script；全部 `LinkObject` 按原目录参与，非 link blob 永不误入。plan/profile 使用新的 v2 内容规则，实际 install name/RPATH 纳入摘要，既有 `.slib`、native ABI 和 runtime metadata ABI 3 保持；本阶段不实现 final-link cache。
- 完成门包括移走源码后的独立进程链接、真实成员链／回边／同名、动态绑定、CLayout/global/TLS、Scoop ABI、callback、跨 Cone 默认值／泛型与 moving GC，以及合同冲突、输入变化、损坏／确定性、最终检查和原子输出。M23-11 再迁移公开 CLI 与历史 fixture。

### M23-11 umbrella CLI、single-file mode与总验收

详细设计见 [M23-11](milestone23/stage11/DESIGN.md)，状态：已完成并验收（2026-10-03）；前置为已验收的 M23-10。完整 Rust 5197 项、正式 CLI 2067 个 fixture／2157 个变体均通过，分批实现、迁移与清理见 [验收记录](milestone23/stage11/ACCEPTANCE.md)。

- 新增薄 `scoop` bin，复用实际 DAG／snapshot／cache、配套 `scoopc` child、runtime-build 和 program-link。build/run 共用完整构建，显式 link 只定位已有 `.slib`，消费必需的 runtime 对象索引和 native 输入，不读源码或探测 LLVM／compiler。
- build/run 可省略 root，此时使用当前目录 `Cone.toml`；显式文件 root 使用 reserved synthetic identity、logical `main.scoop`、唯一 source 与 core-only Cone dependency，不发现相邻 manifest、Scoop/C/C++ 或 blob。构建 profile 仅 debug/release，默认 debug，支持 `--profile`／`--release`；默认输出为 `target/<canonical-target-triple>/<profile>`，`--target-dir` 替换 target 根，`-o` 可覆盖 build 的最终文件。profile 先预留选择与布局，优化延后实现，当前相同编译设置可复用缓存。
- run 共用 build 的稳定输出发布，再执行同次结果，保留公开产物并清理私有执行副本；覆盖原子物化、并发发布、argv/cwd/env/stdio 和真实 exit/signal。native requirement 仍由显式 library search roots 解析。
- 补齐 primary／note 的 canonical source 和缓存 warning；子进程协议升至 2，一次真实编译可输出 AST、Export／LocalConcrete HIR、MIR、LIR 文件。普通缓存命中零 child，显式 dump 对指定源码节点重新编译一次；不增加 dump cache、第二条语义管线或完整 Compile/Link 重放。
- 旧 driver fixture runner 已在 `c02bd245c` 删除，历史端到端覆盖现已恢复。M1–M22/M25 的 534 份源码均保留，480 份原合并式端到端 fixture 全部经 Python runner 接通正式 CLI，其余阶段 fixture 明确归属并补语言规则的 CLI 覆盖；另保留 M0 smoke。最终 3040 份 Scoop 源码未归属数为零。FFI companion 由声明式步骤显式构建，再仅通过 `--library-path` 参与链接。
- 将现有 Rust fixture infra（含阶段 golden、program-link/native 编排）迁到统一 Python infra；源码注释或附加 TOML 用同一 schema 表达执行条件、步骤、断言和变体，新增 fixture 原则上只加数据，不写 case-by-case 分支。覆盖恢复后删除原 Rust runner、专用 helper、入口与不再使用的依赖，保留普通 Rust 内部单元测试；以仅改 fixture 数据的新增用例验证扩展性，完整验收同时运行 Rust 测试与 Python suite。
- 总验收覆盖 6a 的共同语义／wire／声明位置／双向泛型矩阵及 Stage 7–10 的 ODR、初始化、native、corruption/reproducibility、cache 和 moving-GC/exception/closure/coroutine/FFI 组合；修复实际产物路径暴露的共同实现缺口，删除仅供运行验收的临时 link/startup 旁路。`.slib` 和 runtime metadata ABI 3 保持，不实现 final-link cache。

### M24 GC-free release hook ✅（2026-10-04 完成，[设计](milestone24/DESIGN.md)）

2026-10-04 完成普通与泛型 release hook 的源码编译、参数自由 A→B→C 依赖、跨 Cone 模板与 ODR、聚合 native ABI、GC 根与真实挂起、产物检查和缓存失效闭环。最终无更新模式的正式 CLI 全量验收 2,158/2,158 通过，Rust workspace 5,244 项及 Python runner 29 项测试全部通过。实际命令、各批检查与最终报告见 [M24 验收记录](milestone24/ACCEPTANCE.md)。

- 普通 final class 可声明至多一个不可显式调用的 `release { ... }`。无 managed this，只可只读本 owner 的合格 backing field；generic/static nested 合法，Throwable、object、intrinsic/generated class 与值类型不能声明。
- `ReleaseValue` 在 GC-free 基础上排除 handle/pin/callback 等实际表示。定义方在既有 NoGc 分析中推导 `ReleaseCallability` 和泛型条件，沿共有 callable 接口传递；默认值与 operator/accessor/for/vararg 展开一并检查，依赖方不重放完整 helper 调用图。
- TypeDescriptor 追加静态 hook；完整构造的正常结果路径设置内部 ready，失败与 base/this delegation 不设置。collector 区分逻辑死亡和 moving 旧副本，Dead 对象在 poison/复用/unmap 前原子 clear/test 并同步调用，不复制 payload 或建立队列。
- 直接 C leaf 保留显式 Unsafe 与正常 native contract，在 release 中不做线程 transition；普通调用协议保持。显式 close/release 先将字段置 inert 再释放资源；hook 不保证及时性、顺序、执行线程、退出时调用或 native release 成功。
- 完整 generic profile 从已实现的 `/2` 升至 `/3`，三层 outer schema 升至 2，foundation 从 3/1/2 升至 4/2/3；TD 固定部分 144→152 bytes，callable body 统一 v2，runtime record prefix ABI 保持 3。共有 HIR、MIR/LIR type/layout、production/link section 与 runtime contract 的精确迁移见设计第 4.3 节，旧产物与缓存重建。
- 分批完成共同 ABI、最小 native owner、源码规则与参数自由依赖、泛型与 ODR，并通过总验收；测试使用正式 CLI 和公共 Python fixture schema，覆盖 small/large、构造失败、显式关闭、moving、A→B→C 与双 consumer ODR。各边界只承担必要检查，不增加来源资格、证明包或重复验证。
- ByteBuffer、外部内存压力记账、公开 arm/disarm、full finalizer、对象复活和异步 cleaner 不进入 M24。M26 改用 GC 堆上的 List/ArrayList 与 parts 构建字符串；off-heap ByteBuffer 及外部内存反馈另行排期。

### M25 自有异常 ABI 与 libc++abi 退役 ✅（2026-09-05 完成，设计见 `docs/milestone25/DESIGN.md`）

- 以 Scoop 私有 exception record、稳定 `exception_class`、per-thread caught 栈和 begin/end/rethrow 协议替换 `__cxa_*`，异常 payload 继续按值复制并作为 stable external object root 接受 moving GC 更新；
- 实现只接受 LLVM 22.1 catch-all/cleanup 封闭 LSDA 子集的 `scoop_eh_personality`，直接通过 Itanium Level I `_Unwind_*` 完成 search、landing-pad install、resume 与 record 删除，不借用 C/C++ personality；
- HIR/MIR/LIR 的异常语义与显式 CFG 保持不变，codegen只替换personality和runtime symbol；suspend handler继续先物化为managed `Throwable`并结束native catch，record不可跨线程或挂起点；
- Darwin/AArch64最终链接删除`-lc++abi`且不添加显式`-lunwind`，由默认`libSystem`解析unwind接口；C ABI/Scoop ABI FFI的异常边界不扩大；
- 以decoder/runtime/GC/协程组合测试及Mach-O依赖/导入符号检查验收，最终程序不得出现`__cxa_*`、gxx/gcc personality、C++ terminate或`libc++abi.dylib`依赖。

### M26 List、字符与 parts 字符串构建 ✅（2026-10-04 完成，[设计](milestone26/DESIGN.md)）

- 增加 `Array<T>(size, init)` / `MutableArray<T>(size, init)` 完整初始化构造，保留固定长度、值元素内联与 GC 布局；initializer 的顺序、异常、ZST 与移动 GC 同批落地。
- 增加普通 public `List<T>` / `MutableList<T>` / final `ArrayList<T>`，全部 invariant。List 是只读访问接口；Array/MutableArray 只实现 List，ArrayList 实现全部增删操作，以 GC 堆 `MutableArray<Option<T>>` 几何扩容。快照转换与按当前索引读取的 iterator 具有明确语义。
- 落地 32-bit Unicode scalar `Char`；String 的 length/get/slice 按标量值计数，byteLength 单独表示 UTF-8 字节数。补齐字符字面量、模式/常量/ABI、Iterable<Char>、safe 字符快照、unsafe UTF-8 字节导入与 safe 字节导出。
- StringBuilder 以普通 core `ArrayList<String>` 保存 parts，add 立即完成 ToString，build checked 求和并集中分配/复制最终 String，保留 builder 供重复 build 与继续追加。runtime 只接收具体 backing/有效前缀，不管理容器。
- 完成单行/raw 多行 f-string 与普通 raw 字符串；插值在 HIR 按实际 core callable 脱糖，保持表达式与 toString 的交错顺序、异常和挂起语义。经正式 CLI、跨 Cone/ODR、artifact-only link/run、GC stress 与 negative/golden 验收。
- ByteBuffer、off-heap storage、borrow/view、close 及外部内存压力反馈整体延期；本里程碑不依赖 M24 release hook，也不扩展为通用集合/ownership 框架。旧 M16 字符串设计不作为实现依据。
- 正式总验收通过：全 workspace 5,252 项测试、公共 fixture runner 32 项测试、全部 2,242 个文件 fixture；M26 新增 84 个 fixture，覆盖源码与产物编译、跨 Cone/ODR、异常、协程和 moving GC，详见设计第 11 节。

### M27 Task-local Context（已完成，[设计](milestone27/DESIGN.md)，[验收记录](milestone27/ACCEPTANCE.md)）

- `context(name: T)` 声明命名函数或计算/abstract property 的 requirement；`context(value) { ... }` 是词法 block 表达式，每次绑定一个非空 managed ref。key 为 canonical exact static type，alias 展开后相同；derived binding 不隐式满足 base/interface requirement。
- 实现入口按源码顺序取得一次参数 local，未使用/匿名项同样要求存在，缺失抛可捕获的 `MissingContextException`。requirement 进入导出与 override contract，不参与重载、MSC、推断、函数类型、mangle 或普通 ABI。
- 真实的 normal/return/break/continue/exception 退出沿既有 typed cleanup 恢复；挂起保留 scope。普通/direct suspend 调用共享 task，`startCoroutine` fork child；实际 resume 进入 frame task 并在全部 driver 出口恢复 resumer。
- 普通 closure 只捕获实际词法 local。`foreignCallback` 注册时保存不可变 binding root，每次 invocation fork 独立 task，覆盖同步重入与并发 Reusable；同步 outbound FFI 保持当前 task。
- 首版使用 TaskContext 与小型 immutable radix node 两种 GC 对象，scope mark 直接保存旧 root；无需独立 undo 链或 snapshot 堆对象。current/previous、frame、临时节点与 callback handle 全部沿 M15 精确扫描、relocation 和 barrier。首次 task 在现有 gateway poll 后建立，继续使用既有分配入口。
- `ContextKey` 是既有 `PersistentExactTypeId` 的独立 typed wrapper；每 body/key 的 slot cell 随原 callable registration/associated atoms 与 Strong/ODR 发射。metadata ABI 3→4，保留 image 六类 table；不增加独立 key-use family 或 program/core descriptor，进程 slot 不进入持久 identity/fingerprint。
- 已按同步纵向闭环、完整源码契约、泛型/跨 Cone、协程、callback、总验收六批完成。三份规范已分别同步到 language 8.3、runtime 第 9 章、impl 2.16；完成门包含正式 CLI、artifact-only 链接、negative/golden、跨线程与 moving GC。静态 effect、公开 Context API、scheduler/取消及 allocator 改造不进入本里程碑。
- 正式总验收通过：全 workspace 5,270 项测试、公共 fixture runner 32 项测试、全部 2,314 个文件 fixture；其中 M27 新增 72 个用例，覆盖独立与组合功能、错误诊断、跨 Cone、跨线程和 moving GC。清理重复 driver 测试与产物修改矩阵，保留独有的引用／ABI 错误覆盖，具体耗时和覆盖依据见验收记录。

### M28 Linux glibc / musl amd64（已完成，[设计](milestone28/DESIGN.md)，[验收记录](milestone28/ACCEPTANCE.md)）

- 新增 `x86_64-unknown-linux-gnu` 动态 PIE，以及 `x86_64-unknown-linux-musl` 默认静态 executable、显式动态 PIE；保持 Darwin/AArch64 回归。Linux arm64、glibc 全静态及 musl static PIE 留待后续。
- 架构 ABI、OS/VM、ELF/Mach-O 和 libc 工具链分别承担实际差异，复用已有完整 target projection；不按 libc 复制 compiler/runtime，也不增加 target 插件框架。
- Linux 采用分别为 glibc、musl 构建的 LLVM libunwind 22.1.2 静态库，Scoop personality 与精确 GC 契约保持。支持 LLVM 生成的 cleanup-only LSDA；不可变 metadata 在 PIE 和静态程序中均须实际只读，musl 主线程栈增长沿既有线程发布协议处理。
- 验收覆盖正式 CLI、跨 Cone/ODR、artifact-only、FFI/TLS/异常、moving GC、Context/协程及缓存目标隔离，已完成 Linux 三种链接配置及真实 macOS/AArch64 回归。
- 全部适用文件 fixture 验证通过：glibc 2,295 个、musl 2,297 个、Darwin 2,314 个；glibc 全量中的 8 份旧产物指纹修正后普通复验通过。Linux workspace 5,318 项、Darwin workspace 5,290 项及 Python runner 38 项通过，原始运行与修复说明见验收记录。

### M29 静态类型描述与序列化基础设施 ✅（2026-10-06 完成，[设计](milestone29/DESIGN.md)，[验收记录](milestone29/ACCEPTANCE.md)，[实施记录](milestone29/PROGRESS.md)）

- 为每个类型提供编译期结构描述，统一源码与`.slib`中的字段名称、类型、annotation、variant及构造关系。复用共有HIR及typed identity，不建立运行期反射表或扩展runtime TypeDescriptor；通用源码编译期查询/遍历语法后续单独设计。
- 增加编译期annotation class及字段/variant/property注解；SerialName和Transient提供默认编码的最小定制，其余用户注解保留为静态数据。
- companion或普通codec显式实现`Encodable<T>` / `Decodable<T>`；encode接收显式数据值，decode返回目标T。两个方向都有真实codec receiver和普通itable，合法手写/继承/default实现优先，缺少的方法在HIR中按T的shape合成为普通typed body。数据继承不传递codec关系，不增加Self、static成员或类型级工厂requirement。
- companion按完整宿主application分别具有类型、singleton状态及exactly-once初始化；可直接使用宿主类型参数与bound，同一application跨Cone沿ODR共享状态，不同实参各自独立。此项修订M21的共享companion选择，普通static nested作用域保持；历史milestone设计保留原文。
- 泛型的两个方向都显式传入codec，数据类型无需编码bound。`Box<T>.Companion`把元素codec交给普通encoder/decoder helper，宿主具体化不自动提供裸T的字段能力。支持struct、enum、tuple及原范围内的final class；核心容器保持无bound用途，撤销容器/tuple条件编码，Unit使用普通UnitEncoder/UnitDecoder。
- keyed/unkeyed/single-value协议分离类型与格式；普通JSON库通过String输入输出验收，encode/decode入口均显式接收codec。字段按声明类型选定codec，不回退到基类companion或运行期类型查找。不扩入Map、Float/Double、ByteBuffer或循环图/开放多态框架。
- 2026-10-06协议修订已完成，设计与三份spec同步，按功能迁移并提交。全部适用fixture覆盖：Darwin 2,490项、glibc 2,471项、musl 2,473项，每个平台均含176项M29用例；Darwin使用普通完整`--all`，Linux使用完整更新轮、快照审阅与定向复验，原始结果见验收记录。既有实例编码和条件conformance的实现/测试保留为历史，不计作新协议验收。

### M30 Float / Double 与 128 位数值调研（已完成，[设计](milestone30/DESIGN.md)，[实施记录](milestone30/PROGRESS.md)，[验收](milestone30/ACCEPTANCE.md)，[调研](milestone30/INVESTIGATION.md)）

- 正式交付两个GC-free值类型：Float/Float32为IEEE binary32，Double/Float64为binary64；后两种固定宽度拼写分别是透明alias。覆盖十进制literal、目标精度const、基础算术/分类、显式转换和ToString，以及泛型、默认值、annotation、pattern、数组/aggregate、boxing、协程与moving GC。
- M30 最初交付 IEEE 默认比较与 isTotallyOrdered。后续 M41 将默认比较改为等价类全序，新增 Compare / compareTo 与显式 IeeeComparison / ieeeCompare，删除旧 totalOrder API；当前语义见语言规范 11.2.2。Float/Double 仍不实现 Hash。
- literal、const与JSON按实际目标精度直接舍入，避免先Double再Float；运行期不使用fast-math。数值转换显式，浮点转整数向零截断并按目标位宽饱和，NaN转0。公开位/字节转换与transmute留待具体需求，不作为本次totalOrder的前置条件。
- 两种companion实现普通Encodable/Decodable，single-value协议增加Float/Double read/write；JSON保持整数精确解析，浮点只编码有限值，保留负零，按路径报告非法值与overflow，允许合法subnormal/underflow。字符串化使用成熟的最短往返转换，C ABI以float/double通过既有generated-C bridge接入。
- 复用既有IR/meta、布局、GC和native-support机制；`frem`产生的fmodf/fmod覆盖Darwin、glibc动态以及musl静态/动态实际链接与运行。三份spec已同步实际契约与格式版本，见实现规范2.18；完成门为独立/组合/negative/golden、workspace与正式CLI/fixture验收。
- Int128/UInt128与Float128仅调研：记录i128所需compiler-rt/libgcc helper、binary128与long double的差异，以及compiler-rt、libquadmath、SoftFloat的职责和可行路线。不添加128位类型、IR占位或runtime依赖，也不把它们的实现纳入M30完成门。
- 2026-10-07实现与验收完成：5355项Rust测试覆盖通过，44项公共runner测试在Darwin与Linux通过；Darwin全部2543项适用fixture通过，另7项不适用。Linux完成M30专项、受影响组合与GNU/musl各11项收尾普通复验，范围与原始报告见验收记录。按功能分批提交，清理无关固定摘要，保留实际类型、IR、ABI、GC、符号与运行验证。

### M31 首批优化、ODR 合并与 nursery（已完成，[设计](milestone31/DESIGN.md)）

- 将 debug/release 的实际优化配置贯通 umbrella、child 协议、scoopc、producer 与缓存。release 使用 LLVM machine O2 及经过精确 GC/EH 验证的函数内 IR pass；generated-C 独立切换 O0/O2，runtime 沿用当前默认 O2。首批覆盖常量/复制传播、已知 enum/Option 分支、死路径与冗余存取清理、无效标量写屏障及可安全消除的 root 临时存储。
- ODR 在普通依赖确认同一定义后，只比较完整 typed key 与共享 ABI；删除 LIR/机器码/EH/stackmap 的专用 definition 判等，不增加优化前正文指纹。优化差异进入生产/代码/链接缓存，真实模板、const、类型和 ABI 变化仍按普通 semantic fingerprint/stale 规则失效。
- artifact-only program-link 明确选择兼容定义，正文及其 EH、stackmap、registration、Context cell 等附属数据同选，再生成最终 image 表；独立 helper 仍取并集。混合优化实例共享唯一 TD、storage、callable 和初始化状态；metadata ABI 4→5 及实际受影响格式同步迁移。
- 在现有 arena 中增加按 block 划分的 nursery，复用 per-thread TLAB、STW、精确 roots 与 moving Immix。minor 只扫描完整根、旧代脏区及年轻存活图，第一次存活即晋升旧代，不引入 survivor 区。补齐中小对象跨 line 分配、精确 size 与旧代晋升，不能继续将所有超过 64 bytes 的对象送入独占 large span。
- remembered set 消费卡表；编译器与 runtime/native 的所有引用写入遵守范围屏障，多 mutator 使用原子标卡，GC-free/scalar 写入省略。pin 地址稳定、带 release hook 对象首版直接旧代分配；晋升空间不足在完整原图上转 full，显式 `gc.collect()` 保持 full。
- 分六批实施：配置/基线、ODR/物理选择、首批优化、分配/屏障、nursery 闭环、总验收/性能。完成门为三 target 的 debug/release、正式 CLI 与 artifact-only 混合优化、minor/full moving、FFI/多 mutator/Context/release 组合，以及可复跑的运行/编译/代码大小/GC 性能报告。
- 自动跨函数内联、通用去虚拟化、循环边界检查优化、LTO/PGO、逃逸分析、协程 frame elision、survivor、并行/并发 collector、arena 扩容及编译速度优化留待后续。2026-10-07 实现与本阶段验收完成：Darwin 全部 2552 项适用 fixture 通过，另 7 项不适用；GNU/musl 的 127 项定向选择分别为 112/115 项普通通过，其余按平台不适用，NUC 另完成重点组合原生复验。运行效率、机器代码大小、GC 收益及回退见[性能报告](milestone31/PERFORMANCE.md)，实际测试范围与分批提交见[验收记录](milestone31/ACCEPTANCE.md)和[实施记录](milestone31/PROGRESS.md)。

### M32 Any / Nothing 的源码定义与顶底类型闭环（已完成，[设计](milestone32/DESIGN.md)，[验收](milestone32/ACCEPTANCE.md)）

- core 正式声明 `@Intrinsic("core_any") public abstract class Any {}` 与 `@Intrinsic("core_nothing") public final class Nothing {}`，提供普通 public binding、nominal identity 和跨产物声明；删除 Any 在缺少源码声明时的兜底。
- 保留 Any 的顶类型、装箱与无成员语义；补齐 Nothing 的底类型关系、结果合并、无正常返回调用、检查／转换，以及异常／finally 和挂起的正确边界。
- 复用现有 intrinsic、IR/meta、runtime 类型关系及普通依赖通道，完成函数值、泛型、Option/数组、A→B→C 再次发布和 artifact-only 链接运行；按实际编码迁移受影响版本与缓存。
- 分四批完成源码根、底类型求值、组合与产物、正式回归。以真实 core 构建、独立／组合／negative／golden、正式 CLI 与 GC/debug/release 验收为完成门。
- 2026-10-07 实现与验收完成：Rust 工作区 5,328 项测试覆盖在首轮及定向复验后闭合，44 项公共 runner 测试通过；Darwin 全部 2,594 项适用 fixture 普通通过，另 7 项不适用。NUC 上 GNU/musl 各 67 项定向 fixture 普通通过，其中三个 target 的 M32 专项各 42 项；实际范围和去重后的报告见验收记录。

### M33 平台库的编译器、driver 与 runtime 前置能力（已完成，[设计](milestone33/DESIGN.md)，[验收](milestone33/ACCEPTANCE.md)）

- 提供平台库需要的编译器与 runtime 能力：NativeSafe 握手、GCLeaf 与标量直接 C ABI、作用域数据借用、计数 pin、受检 UTF-8 与 C 字符串、四种 main／argv／退出码、NativeSafe 输出及 errno 捕获。
- 完成 Cone 内 C/C++ 源码、目标条件源码选择、native 系统库、sysroot 中已声明 scoop 依赖的默认定位；C++ 支持 Darwin/GNU，musl 明确拒绝。
- `==` / `!=` 使用普通成员 `operator fun equals`，Map 的 key 比较使用独立的普通 `Equality<T>.equalTo` 接口；结构比较不派生接口，两项能力分别显式提供。上述接口拆分由 M41 的 Equals/operator equals 统一方案替代；后续有序 Map 的比较能力见 M41。M33 提供五种内存序的原子类型及线程退出诊断，不扩大为平台库公开 API 或新的通用框架。
- 按设计第 12 节拆分功能提交，完成三份规范、独立与组合 fixture、负例及各阶段转储；最小平台 Cone 在三个 target 上完成源码、产物、独立链接、文件操作及并发 GC 闭环，musl 另有 PIE 验收。
- 2026-10-09 总验收完成：Rust 工作区 5,394 项和公共 runner 44 项通过；Darwin 的 2,850 项适用 fixture 覆盖闭合，其中 1,977 项普通比较、873 项审阅后的完整快照更新执行，另 16 项不适用。Linux GNU／musl 的 M33 专项分别为 260／255 项普通通过，各另通过 6 项必要旧回归；保留实际选择范围与逐项成功来源，避免重复全量测试。既有泛型结构比较正文选择问题单独记录，未扩展 Equality 或 M33 的验收范围。

### M34 接口与调用优化、可扩展并行标记 Immix（已完成，[设计](milestone34/DESIGN.md)，[实施记录](milestone34/PROGRESS.md)，[验收](milestone34/ACCEPTANCE.md)）

- 面向正常桌面和服务器，以执行效率、吞吐和合理停顿为目标，允许适量内存换性能；同时测量 RSS、GC 工作量和代码大小。规范先行、两地 M33 基线、必要的 LLVM／C ABI 实验及各批实现已完成，实际结果见实施记录与验收。
- interface value 改为 object + itab 的双字表示，将查询移到接口构造／转换处；动态调用直接读取 slot。完成 identity、Option niche、字段／数组、精确扫描、AtomicRef、Scoop ABI native、closure／continuation 与跨 Cone 适配，slot 的隐藏 receiver 保留明确的单字 object projection。
- 在完整 MIR 上进行 receiver 实际类型传播、唯一目标去虚拟化和本 Cone 已有正文的小函数自动内联；内联按调用点的化简后成本采用三档策略，控制 caller 累计代码增长，包含在本地物化的外部泛型实例。不得把当前 Cone 只有一个实现当成全程序闭合。
- 增加内联 safepoint 检查与冷慢路径、小型 GC-free 值的直接 ABI，并把合法 C-layout aggregate 的正向 extern 调用扩展为按目标 C ABI 分类的 DirectC。
- 在 M31 已有分代 Immix 上改为按需增长的多 region、稀疏地址索引和分区 card table；选择低存活区域跨区搬迁，保留高存活／pin 区域的 line 复用，完成空页 discard 与空 region unmap。移除固定 1 GiB 总堆限制，大对象不受普通 region 大小限制。
- region 与屏障稳定后增加 STW 并行 mark、任务分块和正确的全局终止判定，保留存活对象集合供引用更新；首版复制、引用回写与 release hook 仍由 coordinator 执行。分配统计改为线程局部累计与精确汇总。
- 提供与 T 等大、全零可扫描的 `MaybeUninit<T>`，通过合法值包装与 unsafe assumeInit 维护初始化契约；ArrayList 使用普通 `MutableArray<MaybeUninit<T>>`，删除／clear 清零引用。StringBuilder 继续持有 `ArrayList<String>` 复用扩容，仅适配 build 的 backing 消费边界。复用现有数组 shape 与按容量扫描的规则，单独测量低占用大容量的 GC 成本。
- 已按设计分十批实施，迁移到 runtime ABI contract 12／metadata ABI 8，新 ABI 的 debug/release 保持互通，旧 ABI 产物明确拒绝并要求重建。完成三个 target 的源码／产物／链接／运行、独立 C compiler 互调、moving／多 mutator／并行 mark、真实扩容与内存归还，以及可复跑的分阶段性能报告；并发 GC、survivor、全程序优化、通用逃逸分析和 frame elision 留待后续。
- 2026-10-10 总验收完成：Rust 工作区初跑 5,404 项中的 61 项失败均在两机定向闭合，后续最终 codegen 的 Darwin 363／GNU 378 项与公共 runner 44 项通过。正式 CLI 按实际运行来源去重，Darwin 2,904 项通过、16 项不适用；GNU 1,317 项通过、30 项不适用；musl 503 项通过，均无未闭合失败。Darwin 完整初跑与后续迁移、GNU／musl 受影响选择及严格复验分别记录，没有重复无关全量；两机共 28 组最终同机性能对照保存全部七轮样本与收益、回退。

### M35 when、Elvis 返回、尾随 lambda 与注解数组（已完成，[设计](milestone35/DESIGN.md)，[实施记录](milestone35/PROGRESS.md)，[验收](milestone35/ACCEPTANCE.md)）

- 普通 when 与显式 case 模式共存；subject 声明/解构、路径 smart cast、多条件、guard、覆盖与正常落空。
- return/throw 表达式与 Elvis 的正常结果合并，沿用异常、Context 与协程清理。
- 尾随 lambda 的最后形参映射，覆盖 default、命名、vararg、泛型及普通调用入口。
- 一维标量 annotation 数组及完整类型，贯通默认值、静态描述与跨 Cone 产物。
- 每项功能独立 fixture 与组合验收，按批次验证和提交，最终完成正式 CLI 回归。
- 2026-10-10 实现与验收完成：Darwin Rust 5,437 项、Linux Rust 5,468 项及两机公共 runner 44 项通过；正式 CLI 按实际来源去重，Darwin 3,049 项、GNU 344 项、musl 306 项通过，分别另有 16／20／17 项不适用。三个 target 均覆盖全部 131 个 M35 新用例；Linux 聚焦实际后端变化与平台组合，保留额外通过记录，未完成候选不计通过。HIR cross-cone interface／type semantics 为 72／29，runtime ABI 保持不变。

### M36 编译器输入与 IR 的类型完备性（已完成，[设计](milestone36/DESIGN.md)，[实施记录](milestone36/PROGRESS.md)，[验收](milestone36/ACCEPTANCE.md)）

- 收紧调用目标、固定元数 intrinsic、when、return 与最终 MIR 临时态边界；保留已有求值顺序、异常、协程与 GC 合同。
- 保留函数／enum 等窄身份，分离 declared 与 C/Scoop 表示专用数据，core protocol 改为具名 typed 字段。
- NativeBorrowed 结果发布与 codegen 调用准备一次构造；parser 修饰符与 literal pattern 使用完整数据。
- 按功能提交，本机针对性验证后在 Linux 完整验证；不新增语言功能、认证、预算或通用状态机。
- 2026-10-11 实现与验收完成：12 项功能分别提交；Linux 完整 Rust 5,474 项及最后修复的 29 项定向测试通过，最终不同测试覆盖 5,475 项，两机公共 runner 各 44 项通过。普通 CLI 按实际来源去重，Darwin 236 项、GNU 3,061 项、musl 188 项通过，GNU 另有 30 项不适用；GNU 使用正式全量加失败补验，三个 target 均覆盖全部 26 个 M36 新用例。wire section 编号／版本与 runtime ABI 保持不变。

### M37 语言语法与类型能力补齐（已完成并验收，[设计](milestone37/DESIGN.md)，[验收记录](milestone37/ACCEPTANCE.md)）

基线为 `ec9d8849d`（M36）。按功能纵向完成并逐项提交：兼容修饰符、Unicode/反引号标识符、单语句控制体、do/标签/跳转、声明推导、函数引用与 receiver 函数值、泛型/嵌套/局部 alias、inner/局部/匿名类型、sealed/接口委托、流敏感 smart cast、整型枚举及静态注解扩展。21 项功能分别提交；三个目标的 680 个 M37 fixture 均通过，Linux 完整 Rust 5,553 项通过，GNU 全量覆盖 3,741 项通过、30 项不适用。完整验证范围及产物兼容性见验收记录。

`inline`、`crossinline`、`noinline`、`reified`、`tailrec` 只解析后完全忽略。interface 方法自身泛型、任何泛型虚方法和外层词法局部 `var` 捕获为明确排除项；non-local control flow 暂缓。

### M38 TypeInfo、结构化调试输出与 CLI 参数解析（已完成并验收，[设计](milestone38/DESIGN.md)、[验收](milestone38/ACCEPTANCE.md)、[进度](milestone38/PROGRESS.md)）

- 在 M46／M48 完成后交付 TextWriter／StringWriter、typeInfo／annotationKey、typed 声明与注解描述、Inspectable 普通访问及合成、DebugPrinter，以及独立 scoop.cli 的 schema／argv／稳定 Decoder／help。
- 52 项新增正式 fixture 在三个目标全部通过，包含真实跨 Cone 产物、组合 codec／debug、精确负例及 moving GC。当前 Rust 覆盖本机 5,305／Linux 5,341 项；正式 CLI 本机 4,141、GNU 4,130、musl 4,126 项通过，完整轮次与受影响补验均已闭合。
- 修正 Linux manifest 大小写，并将 GC 指针查询改为实际使用链及共享 CFG 位置，CLI 基本组合恢复默认超时。逐功能提交，原始日志在仓库外，只保留验收概要；后续按用户要求推进 M45。

### M39 目标平台边界重构（已完成并验收，[设计](milestone39/DESIGN.md)，[验收记录](milestone39/ACCEPTANCE.md)）

- 保持 Darwin/AArch64、Linux GNU/musl x86-64 三个现有 target，不新增架构支持。
- 分离公共 typed 数据、目标目录、codegen ABI 规划与 LLVM 实现、工具链执行及对象架构规则。
- driver 编排布局／签名准备、ABI 规划和完整 LIR 构造；依赖 ABI 直接映射复用。
- 11 项功能分别提交并完成验证；Linux Rust 5,576 项、GNU CLI 3,741 项、musl CLI 3,739 项通过，本机定向 CLI 83 项通过。现有 wire、runtime ABI 与 golden 保持，构建产物按阶段清理。

### M40 fixture 精简与回归验证职责收敛（已完成并验收，[设计](milestone40/DESIGN.md)，[验收记录](milestone40/ACCEPTANCE.md)）

基线为 `b59294fbc`。普通功能保留编译、运行和精确诊断，阶段结构由 typed IR 及 9 项专测负责；原生链接计划仅精确比较相关输入与绑定。fixture 内容由约 8.29 GB 减至 16.00 MB，snapshot 声明由 12,257 降至 57，4,939 份 Scoop 源码保持原样。产物持有 CLI 改为同步 artifact-only 链接；内部字节持有在 Rust 函数边界测试，不改变生产代码或产物格式。按功能提交后完成三平台适用范围验收：Darwin 3,755、GNU 3,741、musl 3,739 项通过，无未闭合失败；具体来源、补验和清理记录见验收文档。

### M41 运算符接口与浮点默认比较（已完成并验收，[设计](milestone41/DESIGN.md)，[验收记录](milestone41/ACCEPTANCE.md)）

- 通过普通 core 运算符接口及显式 conformance 支持泛型操作约束，保留普通成员和作用域扩展调用。
- 以 Equals/equals 替换 Equality/equalTo；Float/Double 默认比较按正负零与全部 NaN 的等价类构成全序，提供显式 IEEE 四态比较。
- 按功能先更新 spec、实现并验证后提交。Rust 完整覆盖 5,584 项通过；Linux 正式 CLI 完整覆盖 GNU 3,785 项、musl 3,783 项通过，无未闭合失败。本机定向回归、完整运行与补验来源、产物差异审阅及清理见验收记录。

### M42 平台无关标准基础库（已完成，[详细设计](milestone42/DESIGN.md)，[规范](specs/SCOOP-STDLIB-SPEC.md)，[验收记录](milestone42/ACCEPTANCE.md)）

- 增加普通 `scoop:scoop.std:0.1.0` Cone，显式依赖 core，按 Kotlin 习惯组织 collections/comparisons/sequences/text/bytes/io.encoding/random/time/datetime/uuid 与 Result；不改变 single-file 的 core-only 依赖或默认导入。
- 集合同时包含 unordered HashMap/HashSet、insertion-ordered LinkedHashMap/LinkedHashSet、sorted TreeMap/TreeSet 与 Sorted 接口，另有 Deque、PriorityQueue、工厂/快照、集合算法和惰性 Sequence。core 仅补普通 Collection 公共接口，保留原 List/ArrayList/StringBuilder 的依赖与行为。
- 补齐文本、数值解析、堆内字节读写、大小端、Hex/Base64、typed Result/Option 组合、显式 RNG 抽象与 seeded PRNG、Uuid 及显式 codec。
- 时间包含 Duration、Instant、Clock/TimeSource 来源接口、同源 TimeMark、LocalDate/LocalTime/LocalDateTime、固定 UtcOffset/OffsetDateTime、YearMonth 和日历 period；本期仅数据、运算与处理，无系统取时、熵、时区数据库或调度。
- 所有实现必须由普通 Scoop Cone 与公开 core API 完成，不新增 native 后备、intrinsic、compiler/runtime 特判。合法源码遇到底层缺陷时先独立修复和回归，再继续库实现，不保留库侧绕行。
- 验收按实际修改范围：未改底层实现时，最终只跑 M42 这组 Cone 的三目标定向测试及必要的 core API 定向回归，不运行完整测试套件；底层修改按影响补回归，共享 ABI/GC/产物变化才据实际影响扩大验证，详见设计第 7 章。
- math（含 BigInteger/Decimal）、regex、完整 Unicode、压缩、密码学、更多格式、持久化集合及系统库独立排期，不成为本里程碑的验收前置。
- M42-1～12 全部交付，TreeMap／TreeSet 使用内联键值的 B-tree。三目标各通过本组 130 个 fixture、139 个变体；含必要既有回归后 Darwin／GNU 各覆盖 150 项、musl 149 项。五个实际底层缺陷均独立修复及回归，没有运行无关全量套件。

### M43 测试库与 `scoop test`（已完成，[设计](milestone43/DESIGN.md)、[验收](milestone43/ACCEPTANCE.md)）

- 交付普通 `scoop.test` Cone、断言、显式测试容器与方法、每用例实例及 BeforeTest/AfterTest 生命周期。
- 增加同 Cone 测试源码、测试依赖与构建用途，由 `scoop test` 静态发现、构建、过滤、列出并串行运行，保留准确的生命周期失败与进程结果。
- 以 `scoop.std` 作为首个实际落地应用，在 `sysroot/lib/scoop.std/tests/` 编写同 Cone 单元测试，通过 `scoop test sysroot/lib/scoop.std` 验收测试库与命令的完整流程。
- 已按功能分批更新规范、实现、验证并提交；三个现有 target 各通过 167 项 fixture，包含 105 项 M43 与 62 项必要既有回归，覆盖正式 CLI、产物消费、进程边界与 GC 组合。
- integration-test 等待 multi-cone workspace 定型后另行设计，不纳入本里程碑。

### M44 GC 引用更新优化与实现边界隔离（已完成，[设计](milestone44/DESIGN.md)、[验收](milestone44/ACCEPTANCE.md)）

- 根据本轮实际搬迁结果省去零搬迁时的 relocation，保留回收、晋升、pin/release 和验证职责。
- 复用引用的单次对象定位结果，仅在地址改变时写回；无托管引用对象继续参与存活与回收，但省去出站引用扫描工作。
- 收拢 codegen 的完整 GC 分配、写入、发布与 poll 操作，以及 runtime 的 GC 操作、Scoop 适配和 mutator 私有状态，保持现有静态快路径与必要 ABI／同步契约。
- 先建立当前同机基线和完整停顿口径，再按功能分别提交优化与模块迁移；通过实际性能对照、共享路径集成回归、三目标和无源码产物消费验收。
- M44-1～6 全部交付。Darwin 工作区通过 5629 项测试及 2 项 doctest，正式 CLI 全量通过 4037 项；Linux GNU／musl 适用组合分别通过 1163／1167 项。零搬迁、实际移动、minor、线程进度和四程序性能对照完成，回退修复、原始失败及测量限制均保留。

### M45 平台库、确定性资源释放与 native IO buffer（已完成，[设计](milestone45/DESIGN.md)，[验收记录](milestone45/ACCEPTANCE.md)）

- 在 M38 完成后实施；首先修复泛型扩展回调的参数／返回类型联合推断，再接入普通 `scoop.platform` Cone。
- core 提供 Disposable、普通／挂起 use 与 GC-free pressure 提示；platform 提供 native buffer、同步 IO、文件、进程、线程、网络、系统时间与熵。
- M45-1～10 按顺序完成并逐功能提交，已交付推断修复、系统字符串／路径／错误、资源协议、NativeByteBuffer、同步 IO、文件／目录、环境／系统信息、时钟、系统熵、线程／同步原语、进程／管道及阻塞 DNS／TCP／UDP；真实下游应用完成 buffer → 文件 → 子进程管道 → TCP → 文件的传输与独立链接。过程见[实施记录](milestone45/PROGRESS.md)。
- 2026-10-16 三目标验收完成：平台库各 99 项在 debug／release／debug moving GC 通过；正式 CLI 组合 Darwin／GNU／musl 分别 335／337／342 项通过，另有 7／5／0 项不适用；Linux 完整 Rust 5350 项闭合。本机稳定后集中运行 Linux，最后的测试期望修改仅补验受影响用例；原始日志在仓库外，只保留验收概要。

### M46 序列化基础协议与 DTO 合成（已完成，[设计](milestone46/DESIGN.md)）

- 按 M46-1～6 完成稳定解码视图、null 查询、编码共享容器、统一 Serialize 与 Option 默认值、不可变 JsonValue，以及 API／Cone 离线数据形状验收。
- core 与格式库保持普通 Scoop 实现；字段规则在 HIR 生成普通 typed body，不增加专用 runtime ABI、JSON IR 或运行期反射。
- M46-1～6 已分批实现、提交并验收；四组离线基线及组合映射见 [覆盖记录](milestone46/COVERAGE.md)。Linux GNU 全量 4061 项通过、30 项不适用；musl 本期及必要回归 224 项通过，Rust 单元／集成测试 5665 项与 2 项 doctest 通过。
- 每项功能先同步规范、实现和针对性验证，再提交；先本机测试，再在 Linux 完整验证。M46 已完成，后续按用户要求严格先完成 M48，再推进 M38。
- 分批执行、性能修复、失败闭合与验收概要见[实施记录](milestone46/PROGRESS.md)。

### M47 HIR 查询与接口实现生成器（通用机制已接入，codec 迁移待实施，[设计](milestone47/DESIGN.md)、[协议](milestone47/PROTOCOL.md)）

- 从完整声明与签名提供通用 typed 语义查询，以同步宿主 generator 返回普通方法体源码，driver 经正式 parser／HIR 补齐既有接口实现；M46 codec 策略作为迁移试点。
- 2026-10-21 补充[可恢复语法／LSP 公共基础](language-service/DESIGN.md)与[调研](language-service/INVESTIGATION.md)：共用源码坐标、同一 grammar、语义上下文、查询与源码关联；编辑态未知不进入成功 HIR。
- 固定编译视图、严格片段入口、共享查询及真实 generator 双向协议已随公共基础接入；可恢复 CST、编辑态语义、LSP 与引用／编辑能力也已完成 A～E 交付，验证范围和当前限制见[实施记录](language-service/PROGRESS.md)。codec 迁移仍按 M47 设计推进。

## 3. 备注

- 里程碑内的特性验收标准：独立 fixture、组合 fixture 和准确的 negative 诊断。普通功能用例验证编译与运行；各 stage 的结构由少量代表性的 golden 或 typed IR 精确断言覆盖，产物、ABI、GC、链接与优化专测保留对应合同（见 AGENTS.md 编码准则）。
- 普通功能 fixture 不附加无关的产物、core 或 native 输入固定指纹；缓存、确定性及产物消费一致性优先比较本次产生的值。结构快照只对明确的易变摘要字段做保留引用关系的归一化，完整类型、ABI、GC、相关符号和运行断言继续保留；规则见实现规范第 1 节及 fixture runner 说明。
- 2026-10-05 M29后续修订：`Box<Int>.Companion`与`Box<String>.Companion`改为不同类型、不同singleton，companion可使用宿主T；成员、初始化及跨Cone物化按完整宿主application处理。M21设计§3.2及M23相关历史设计不回写，以[M29设计](milestone29/DESIGN.md)和当前language 9.1.3/runtime 2.7/impl 2.17为当前规则；M29完整实现与实际验证见[验收记录](milestone29/ACCEPTANCE.md)。
- 2026-10-06 M29后续修订：`Encodable<T>`与`Decodable<T>`统一由companion/普通codec对象实现，数据类型继承不决定编码策略。移除旧实例编码、编码bound及容器/tuple条件接口，采用显式codec组合；当前语言9.5/11.13、runtime 2.2、impl 2.17与[M29设计](milestone29/DESIGN.md)同步，旧实施记录保留，修订后的实现与验证已完成。
- 里程碑顺序可按实现中发现的依赖调整，但 M0 不推迟、M3 不晚于任何依赖 `Option` 的特性。
- 2026-08-28 顺序调整：字符串插值由 M6 后移至 M12（低优先级语法糖）；引用类型层级提前为 M6，新增 M7 函数重载；原 M8–M12 顺延为 M8–M13。其后（同日）再调整：新增 M12"泛型上界约束与接口化"（ToString/Hash/equals，spec 11.11 已定稿），字符串插值顺延为 M13、多 Cone 顺延为 M14。
- 2026-08-31 顺序调整：在 FFI 前新增 M11“函数类型、函数值与 closure”，先完成 lambda/callable reference/closure conversion，使 FFI 直接复用正式函数类型；原 M11–M14 顺延为 M12–M15。
- 2026-08-31 顺序调整：在 FFI 后新增 M13“多线程 GC 与 foreign-thread managed callback”，补完 managed closure反向回调和 foreign-thread runtime入口；原 M13–M15 顺延为 M14–M16。
- 2026-09-01 顺序调整：在接口化之后新增M15“精确根、statepoint relocation与moving compaction”，以强制relocation stress mode前置验证managed ref/root契约；字符串插值与多Cone顺延为M16/M17。
- 2026-09-03 顺序调整：撤回原M16字符串插值设计并延后原M17多Cone；新M16先统一fresh-variable constraint solving与overload resolution，新M17再落地命名/default/vararg完整实参协议。其余M2/M4/M6/M7/M14基础backlog及多Cone、字符串能力按上节后续顺序重新排期。
- 2026-09-04 编号确定：原“后续顺序”七项依次编号为 M18 callable表面、M19构造与初始化、M20泛型类型系统第二阶段、M21属性/对象/可见性、M22基础语言能力、M23多Cone与`.slib`、M24字符串底层与插值；各项详细范围仍须spec先行并单独设计。
- 2026-09-04 M20设计决定：nominal generic统一为invariant，撤销既有interface variance并明确不引入projection/star/capture；M20只新增class upper bound与调用点`_`部分类型实参，泛型抽象由generic callable、exact interface及显式转换表达。
- 2026-09-04 M21设计决定：Scoop不提供`lateinit`；延后初始化必须显式使用`Option<T>`/`T?`，其中无accessor的`var Option<T>`省略initializer等价于`None`。同时固定logical property/accessor、reflection-free delegate、interface default、singleton/global exactly-once初始化与typed access domain。
- 2026-09-04 M21可见性修订：默认visibility由Kotlin式public改为internal；public API、public interface contract、public override与public constructor都要求源码显式标记，`main`仍可internal。sysroot不享有默认public特权，计划导出的core API也必须显式标记。
- 2026-09-04 新增M25“自有异常ABI与libc++abi退役”：保留LLVM landingpad与Level I unwinder，以Scoop record/personality/catch状态替换C++ ABI层；M8–M10对应实现选择由M25设计取代。
- 2026-09-05 完成M25：Scoop runtime自有record/personality/caught栈与moving-GC external payload生命周期落地；object与最终Mach-O门禁锁定LLVM 22.1封闭LSDA、八个Level-I导入及`libSystem` provider，生成程序不再链接`libc++abi`。
- 2026-09-05 M22设计决定（后续修订）：`Int`/`UInt`永久固定32位并以`Int32`/`UInt32`为alias，`Long`/`ULong`永久固定64位并以`Int64`/`UInt64`为alias；无上下文literal采用32位优先、越界升至64位的默认阶梯。为避免同时改变既有API值域，原来使用64位`Int`/`UInt`的array、String、Hash、compareTo、shift、SourceLocation、`@CLayout`参数、pointer/size等source/core契约整体迁名为`Long`/`ULong`，Array上限仍为`INT64_MAX`，internal machine metadata不伪装成源码integer。四种整数range均为真实nominal type。当前target仍须有64位data/code pointer及对应内部carrier逐bit往返能力，裸`Ptr`/`FunPtr`固定非零并由`Option`唯一承载null；platform-native integer留待后续。M22其余范围仍为无标签break/continue与for、exact Iterator协议、递归pattern matrix和typed cleanup target；label、do-while、CharRange与generic alias继续留后续。
- 2026-09-05 M23设计决定：Cone与source package分离，以canonical `group:name:version`及按kind隔离的persistent typed id表达跨artifact identity，v1只消费exact、静态、无环resolved graph。`package`、exact/star/alias/public import与re-export只扩展既有typed resolver层；target-specific确定性`.slib`显式打包三层metadata、native object与闭合export surface，下游完成generic concretization并以完整ODR group coalesce。跨artifact ABI同时固定ZST为零payload/typed elision/显式address token，并为Array采用zero-sized element分支而非零stride通用路径。最终静态程序通过唯一program descriptor登记全部Cone image及runtime metadata后按canonical dependency order初始化；package registry/版本求解、动态加载、generic typealias、interface方法级泛型与跨版本ABI不属于M23。
- 2026-09-07 M23工具/容器边界补充：新增`cargo`式umbrella binary `scoop`负责多Cone图、cache和依赖顺序，`scoopc`收缩为只消费显式上游`.slib`闭包的single-Cone compiler，最终binary由独立artifact-only program-link产生。`.slib`改以typed member directory表达任意多个link object、C/C++ object或其他opaque blob，不再把`code.o`/`bridge.o`、扩展名或object数量写成格式假设；所有可链接object统一走`LinkObject`，全体link object只共同提供一个typed image descriptor，Graph/Compile/Link view与runtime/final-input provenance分别闭合。
- 2026-09-08 M23拆分与单文件模式：原M23按依赖拆为M23-1…M23-11，依次落实source语法、persistent identity/`.slib` container、single-Cone/core边界、build graph、多Cone名称语义、ZST/ABI、generic/ODR、runtime registry、基础program-link、native闭包/hardening与最终CLI。正式`scoop build/run <file>`把指定文件作为固定reserved identity、唯一source、core-only Cone dependency的synthetic executable Cone；不发现旁边manifest/源码，native FFI只由artifact已有typed requirement经显式library search root解析。历史fixture在M23-11切到该正式路径并删除旧`scoopc`直编直链/core拼接旁路。
- 2026-09-07 顺序调整：M24改为GC-free release hook，采用“完整构造后ready、逻辑死亡且真正reclaim前同步调用TypeDescriptor hook”的直接模型，不采用payload复制或异步queue；原M24字符串范围整体移至M26，并补入Char、safe字符互转、unsafe byte互转及off-heap growable ByteBuffer。既有已完成M25编号保持不变。
- 2026-10-04 M26 重新设计：采用 List/MutableList/ArrayList 与 String parts 主线，保留 Char 与字符串能力并明确 Unicode 标量索引。ByteBuffer 及所有 off-heap 配套设计移至后续待排期；三份规范与 M24 后续边界同步修订。
- 2026-10-05 完成 M27 Task-local Context：六批功能、GC 生命周期回归与总验收全部通过；完整 CLI 以无更新模式通过 2,314 个 fixture，runtime metadata ABI 4 及当前 section/profile 已同步到三份规范。
- 2026-10-04 M27 设计修订并同步三份规范：替换旧 8.3 的 Kotlin 静态解析，采用 exact-type 动态 Context；以 scope 保存旧 root、callable 内 cell 列表和现有 managed gateway 完成恢复、链接与启动，删除旧草案的独立 undo/key-use/program support 机制。实现与功能验收尚未开始。
- 2026-09-07 新增M27“Task-local Context”：保留Kotlin-like的`context(name: T)`/`context(value) { ... }`表面，以canonical exact static type为key，结合结构化动态binding和logical-task传播重写原spec 8.3 context parameters。首版不做子类型兼容解析或静态effect row；ordinary ABI保持不变。实现细节区分架构不变量与参考方案，物理索引布局不作为长期契约。

### M49 异步 I/O 与结构化并发基础（已完成，[设计](milestone49/DESIGN.md)、[规范](specs/SCOOP-ASYNCIO-SPEC.md)、[验收](milestone49/ACCEPTANCE.md)）

- 在普通 core 源码中实现 Job、Deferred、CoroutineScope、失败传播与协作取消，复用既有 continuation、Context 和原子能力。
- 先补齐 platform 的跨挂起 buffer 借用和 Libuv 错误域，再以独立 `scoop.asyncio` Cone 接入 libuv 的事件循环、定时等待、TCP、UDP、DNS 和文件操作。
- 显式关闭、实际进度、取消后收尾及普通跨 Cone 消费组成验收闭环；TLS／HTTP 不在本期范围内。发现底层缺陷先独立修复和回归。

### M50 编译器职责清理（已完成，[设计](milestone50/DESIGN.md)、[验收](milestone50/ACCEPTANCE.md)）

- 删除自产 IR／对象／产物的重复语义、布局和内容复核，以及来源证明、只读封存和无生产使用者的验证 API。
- 恢复普通 native 对象、C++ 动态库、COMMON、初始化和归档成员的目标工具链行为；runtime 直接使用已生成的静态数据，保留实际 GC／内存访问和链接所需的引用、身份、范围。
- 同步旧设计与测试；Darwin 逐批定向验证，Linux Rust 4,956 项、GNU 完整 CLI 4,160 项与 musl 专项 88 项通过，收尾改动和首轮失败均已定向闭合。

### M51 密码、证书与 TLS 集成（已完成，[设计](milestone51/DESIGN.md)、[进度](milestone51/PROGRESS.md)、[验收](milestone51/ACCEPTANCE.md)）

- 以固定 OpenSSL 3.5.9 LTS 源码和三目标静态资源交付普通 `scoop.crypto`、`scoop.tls`、`scoop.tls.asyncio` Cone。
- 完成内存密码与密钥、X.509 与 JWS／JWK、有限 BIO 共享 TLS 引擎、同步流和 M49 异步适配，支持 TLS 1.2／1.3、双向背压、取消与显式关闭。
- 目标条件 native include 和短路右侧检查在所属层独立实现或修复；使用现有 FFI、buffer、Job 和关闭合同，不增加 runtime 专属入口。
- 三目标 debug／release／moving GC 均通过 42 项库测试与 7 项生产产物消费者测试；Linux Rust 4,956 项及 2 项 doctest、GNU 完整 CLI 4,164 项、musl 相关 CLI 149 项通过。

### M52 工具链分发与预编译 sysroot（已完成，[设计](milestone52/DESIGN.md)、[验收](milestone52/PROGRESS.md)）

- 统一安装目录与仓库 sysroot 布局：宿主工具放 bin，内置 Cone 源码迁到 src，libuv／OpenSSL／Ryu／LLVM 等第三方源码集中到 vendor；三个现有目标的 `.slib`、runtime、OpenSSL 和 Linux unwind 资源放入 `lib/<canonical-target-triple>`，公开 C 头独立交付。
- 普通 build／run／test／link 默认消费预编译目标资源，空缓存首次使用也不重建内置 Cone／runtime；仓库准备脚本复用普通 Cone 构建、runtime 编译库和 M48 缓存，`scoop` 命令与 driver 不承担发行准备或打包编排。
- 随包提供受控 LLVM 22.1 宿主依赖，保留明确的平台 C 开发环境要求；M47 延后，不作为本期前置。支持安装位置搬迁，收敛 runtime 索引路径和实际 ABI／部署兼容条件。
- 提供打包脚本，生成 Darwin/AArch64、Linux/GNU amd64 两类宿主包，覆盖 Darwin、GNU、musl 三目标；打包流程只供手工测试和发布，不作为常规验证步骤，也不接入常规 CI、workspace 测试或 fixture runner。常规功能回归使用开发 sysroot，最终归档的解压、搬迁和运行按需手工验证。保留源码和工具，不恢复 M48／M50 已退役的验证机制，原始验收资料放仓库外。

### M53 HTTP/1.x 与 HTTP/2 协议库（已完成，[设计](milestone53/DESIGN.md)、[规范](specs/SCOOP-HTTP-SPEC.md)、[验收](milestone53/PROGRESS.md)）

- 复用 M49 异步 I/O、M51 TLS 与 M52 目标资源交付，提供普通 HTTP 协议引擎和异步流适配；支持 HTTP/1.0、1.1、HTTP/2、TLS ALPN、明文 prior knowledge 与可选 h2c Upgrade。
- 字段保留字节、重复项与顺序，参数和编码由调用者处理；接收兼容策略按协议和连接角色配置，HTTP/1 明确采用 llhttp 9.4.3，并接受已知方法集合限制。
- nghttp2 1.70.0 只构建静态协议库，通过既有 `native.libraries` 按 archive 成员选择；排除应用、示例、工具、测试和文档目标，不重复编入上游对象。
- 完成两个普通 Cone 的三目标 debug／release／moving GC 验收：每目标 50 项内存协议测试、26 项异步测试及 6 组独立消费者通过；包括 TLS 单流取消、双向背压、协议切换、并发关闭与原生对端互操作。Darwin／GNU 宿主包搬迁后，三目标 HTTP 消费、缓存复用和两个 link 入口均通过；本期不做函数级裁剪。

### M54 统一 HTTP client 与配套基础库（已完成，[设计](milestone54/DESIGN.md)、[验收](milestone54/PROGRESS.md)、[客户端规范](specs/SCOOP-HTTP-CLIENT-SPEC.md)）

- 在 M53 协议库之上提供纯 Scoop `scoop.http.client`，统一 HTTP/1.x／HTTP/2、ALPN、连接池、流式 body、suspend、背压、完整生命周期超时与单请求取消。
- 独立交付 [IDNA](specs/SCOOP-IDNA-SPEC.md)、[URL](specs/SCOOP-URL-SPEC.md)、[字符集](specs/SCOOP-CHARSET-SPEC.md)和单个 [compression Cone](specs/SCOOP-COMPRESSION-SPEC.md)；IDNA 不要求 ICU，压缩采用 zstd，并支持 gzip／deflate／Brotli 的同步与异步流。
- 在所属 HTTP、crypto、TLS、asyncio／platform 库补齐方法／消息工具、摘要、mTLS／系统信任、文件操作、地址族竞速与代理，不在 client 中增加 workaround。完成重定向／重试、认证、Cookie、私有缓存、下载续传、multipart、SSE 与 HTTP/1.1 WebSocket。
- HTTP/3、HTTP/2 WebSocket、跨 origin H2 合并、证书固定／对端链检查接口、TLS 会话恢复控制／0-RTT、主动 H2 PING 保活留待后续；不引入占位接口。三目标库测试、正式消费者、网络组合与缓存复用验收已完成，实际版本、复用范围和失败闭合见验收记录。

### M55 泛型函数条件与成员调用要求（已完成，[设计](milestone55/DESIGN.md)、[验收](milestone55/PROGRESS.md)）

- M55-1：负向接口约束、三态关系、条件重载与规范化声明身份，保持普通泛型正文的定义处绑定。
- M55-2：public companion 的依赖投影与 bound，复用既有 singleton；增加 JSON 默认 codec 重载。
- M55-3：公开实例／companion 成员与构造调用要求，完整调用形态、效果和同契约转发；增加结构相等断言入口。
- M55-4：局部／成员泛型、默认模板、递归边及跨 Cone 组合验收；identity schema 2、metadata outer schema／language ABI 3，runtime ABI 不变。
- 2026-10-19 完成按功能提交及三目标 M55 104 项 fixture；GNU 全量 4305 项通过／33 项不适用，musl 定向及补充复验 183／5，五项首轮失败全部闭合。Rust workspace 全量与最后 64 项定向回归、runner 50 项通过。
- 逐 callable LLVM module 仅建立实际引用的声明；5000 小函数的同机单轮编译从 253.60 秒降至 64.96 秒，峰值内存未改善，测量边界见验收记录。

### M56 生成代码、GC 与 runtime 性能（已完成，[设计与调研](milestone56/DESIGN.md)、[进度](milestone56/PROGRESS.md)、[验收](milestone56/ACCEPTANCE.md)）

- 面向 PC／桌面与 60 fps GUI，分配速度、短暂停尾部和低配置机器的 footprint 优先于 GC 批量吞吐；局部改进与增量 collector 取舍见[补充设计](milestone56/DESKTOP-GC.md)。
- 优先复用 TD／对象表示操作的已有验证结果，以 typed intrinsic 消除常数时间 runtime 操作的 native transition，并扩展满足精确根与 site identity 合同的 release 优化。
- 对 nursery／TLAB、局部 card mark 和有搬迁 GC 的引用回写做真实工作负载对照；吞吐、GC 响应、内存、编译耗时与代码体积共同决定方案取舍。
- 记录编译声明重复、跨 Cone 小函数内联、装箱与协程对象消除的候选边界；分块 poll 等合同变更需单独验证，不作为首版前置。实现按功能提交，先本机聚焦验证，再做 Linux 完整验收。
- 2026-10-19 完成 A–D 实现与策略取舍，默认 nursery 保留 1 MiB；E／桌面后续候选完成调查。Linux Rust 4976 项、runner 50 项通过，GNU 全量 4220 项通过／33 项不适用，musl 合同组合 676／8；末轮单 worker 回写修正在三目标定向闭合。两端性能保留无明显收益和剩余回退，临时源码、工具、实验目录与远端 worktree 已恢复或清理。

### M57 普通类型与 companion 的条件 interface 实现（已完成，[设计](milestone57/DESIGN.md)、[验收](milestone57/PROGRESS.md)）

- 在普通类型或 companion 的 body 内声明 `override interface I<...> where ... { ... }`，where 必填且非空，复用宿主参数及已有关键字；首版支持正向名义／kind 条件与固定 `T.Companion` 投影。
- 实现块与宿主必须同 Cone，首版只允许写在原始 body 内；消费方、扩展、再次发布方和链接阶段不能追加实现。不同接口可以叠加，同一完整接口的独立实现可能重叠时在定义处报错，不进行特化排序。
- 首个闭环为 HashMap 默认序列化：同一 companion 分别按 K／V 的编码、解码能力实现两个接口，覆盖四种能力组合与嵌套容器；保留原集合定义、显式 codec 工厂和 wire 格式。
- 类型、字段、构造器、基类和初始化只有原来一份；泛型使用在定义处证明接口／成员条件，具体化生成普通接口表。覆盖继承、default、条件 codec 合成、查询循环及跨 Cone 一致性；整份类型的偏特化、全特化和接口实现特化暂不做。
- 2026-10-23 完成按功能提交及三目标 M57 验收。Darwin／GNU 全量 CLI 分别 4,206／4,190 项通过、17／33 项目标不适用，含补跑与定向闭合；两端 Rust 全量、GNU 26 个库测试 Cone 和 musl 30 项专项通过。产物接口格式为 81，runtime ABI 不变，详细范围见验收记录。

### M58 短暂停 GC、局部分配消除与循环 poll 分块（设计与调研完成，待实施，[设计](milestone58/DESIGN.md)）

- Minor／full 暂停工作量：年轻 small block 整块退役、无引用大对象跳过 remembered scan；full 压缩必要容量早判与 starts bitmap 遍历，按暂停阶段独立验证。
- 将内联、去虚拟化、函数内逃逸分析、box 与 closure 消除放入同一个有界优化循环；首批覆盖 forEach 内联后捕获环境替换及无用闭包支持产物清理，保留 identity、捕获与精确根语义。简单 class 字段替换后续按收益扩展，不以全程序分析或通用栈对象为前置。
- 全部挂起点为真正尾调用的协程直接转交 completion，省掉自身状态机；同步正文在 suspend ABI／状态机转换前内联。简单计数循环用外层 poll 覆盖有界内层，并单独验证受限向量化与移动 GC。
- 面向 60 fps 桌面与低配置 PC，同时验收分配、minor／full 与 stop-wait 尾部、RSS、编译成本和代码体积。本轮原型与机制实验保留原基线和限制，未计为生产优化；M56 的已完成范围不变。

### M59 crossterm 的 Scoop 移植与 PTY 交互验收（已完成，[设计](milestone59/DESIGN.md)、[规范](specs/SCOOP-TERMINAL-SPEC.md)、[验收记录](milestone59/PROGRESS.md)）

- 以 crossterm 0.29.0 为基线，提供普通 `scoop.terminal` Cone：Scoop 实现命令、样式、事件与增量解析，C11 shim 处理 Unix TTY、raw mode、尺寸、等待和 resize 通知。
- 首版覆盖现有 Darwin／GNU／musl 三目标；公共类型与内部平台源码边界允许后续 Windows 接入，本期不添加 Windows 实现或 manifest 占位。
- 参考 atto-ui 的 portable-pty＋vt100 测试 host，启动真实 Scoop 程序、维护虚拟屏幕并注入交互；补齐查询回包、精确 cell／协议断言、终端状态恢复和退出排空，沿用普通 fixture 步骤。
- 同步 read／poll 与查询共用单一输入通道；独立 `scoop.terminal.asyncio` 复用公共协议实现，使用实际异步 TTY、取消与关闭合同。
- 三目标各 15 项 M59 fixture、30 个变体、68 个进程通过；Linux 两种 libc 各 62 项终端单测、3 项异步输出单测和 11 项相关网络回归通过，GNU／musl 的 tmux 3.6 smoke 通过。GUI 终端抽查与协议支持的限制见验收记录。

### M60 `scoop doc` 与多 Cone API 文档（已完成，[设计](milestone60/DESIGN.md)、[验收记录](milestone60/PROGRESS.md)、[调研](scoop-doc/INVESTIGATION.md)）

- 新增 `scoop doc`，为一个或多个源码 Cone 生成可离线浏览的 API、源码页与搜索；各 root 保持独立合法依赖图，复用完整 HIR 和真实 typed 声明。
- 支持 `/** ... */`／`///`、有限 KDoc 风格 tag、DocHidden／DocAlias、可见性筛选及按原作用域解析的文档链接；覆盖泛型、property、扩展、继承与 re-export。
- 只使用所选 target 的 Production 源码；沿用 M52 的预编译依赖选择，不因随包源码存在而重建 sysroot，也不在首版新增 `.slib` 文档格式。
- 源码原文嵌入 HTML，使用站内相对行链接；`.slib` 不保存完整源码，源码不可用时省略展示及链接。
- 文档与机器结果分别判断更新，冷源码依赖正常生产时复用同次 HIR；多 Cone 站点统一处理身份、版本、搜索与链接，失败不发布半成品。
- 已补齐 19 个生产库与 7 个嵌套测试／辅助 Cone 的说明；打包流程为全部生产库生成各目标站点并安装到 SYSROOT/docs。文档示例执行留待后续。
- 三目标完整 CLI 验收闭合：Darwin 4,503、GNU 4,487、musl 4,484 项通过；双宿主完整打包、最终归档的文档与解压构建／运行、文档更新复用检查通过。真实浏览器离线交互的工具限制见验收记录。

### M61 Rust 文本后端与宽松许可静态分发（已完成，[设计](milestone61/DESIGN.md)、[进度](milestone61/PROGRESS.md)）

- 将 IDNA、Punycode、NFC 及字符集能力迁到固定 Rust 文本后端；按用户决定收敛到成熟 Rust 库可覆盖的编码，保留增量、BOM、错误偏移合同。
- 已明确 UTS #46 与旧严格 IDNA2008 的差异，完成 Darwin／GNU／musl 资源准备、适配、两个正式链接入口及运行，移除 GNU 文本链；发行收集 Rust 依赖与标准库通知。
- 字符集集合收敛到 43 个名称；readline 的字素、宽度、属性和大小写已迁入共同后端。测试仅验证 Scoop 自有行为，不新增底层补缺或上游 Unicode 兼容性／完备性测试。

### M62 intrinsic 下降、跨 Cone 内联与 runtime 性能（已完成，[设计](milestone62/DESIGN.md)、[实施记录](milestone62/PROGRESS.md)、[验收记录](milestone62/ACCEPTANCE.md)）

- 保证 intrinsic function 在所有类型已确定的静态上下文，以及经 devirt 确定具体目标的上下文中下降为对应 intrinsic；覆盖本地、跨 Cone、泛型特化和静态函数引用，不依赖普通内联阈值。
- 支持符合条件的跨 Cone 非泛型小函数参与内联，改进别名复制和冷异常路径的成本处理；消除已登记 TD 在接口分派热路径上的重复查询。
- 独立评估有界 core leaf、Managed 入口 poll、TLAB 完成路径、nursery 与回写、HFA 和最终映像链接信息；保留必要的发布、握手、异常与 ABI 语义。
- 保留现有调查的收益与限制，沿用 M56 的桌面暂停和 footprint 验收，与 M58 优化协作；候选方案不视为已实现，不仅凭 trees 调大默认 nursery。
- 各功能已独立提交；修正 intrinsic 标量复制成本后 V3 自动内联对照约改善 7.7 倍，普通 GC 移除逐槽 immortal 登记查询。Linux 已知十五个 fixture 失败均定向补测通过；按用户要求停止剩余完整 fixture，实际覆盖与限制见验收记录。

### 独立 Regex 库（已完成，[设计](regex/INVESTIGATION.md)、[规范](specs/SCOOP-REGEX-SPEC.md)、[实施记录](regex/PROGRESS.md)）

- 普通 `scoop.regex` Cone 使用 PCRE2-8 10.49 和薄 C 适配；独立 C 静态库随 sysroot 交付，不增加 Rust staticlib。
- 提供可复用模式、编号／命名捕获、惰性迭代、字面／模板／回调替换、分割与转义；公开位置使用 Long Unicode 标量索引，结果为独立快照，支持 close/use 和构造阶段可选 JIT。
- Darwin／GNU／musl 各 19 项库测试、3 项预编译库消费者测试及 API 文档检查通过；Darwin 发行组装与组装目录中的消费者验证通过。仅验证 Scoop 适配与公开行为，不新增 fixture、底层缺口或上游 Unicode 符合性测试。

## 4. 待补齐清单（backlog）

各里程碑"涵盖但只实现了部分"的事项，按来源里程碑整理。标注→的为目标里程碑（已知时）；未标注的待排期。

### 程序运行性能优化主线（调研完成，持续实施中）

- [优化方向与实施路线调研](program-performance/INVESTIGATION.md)以达到或超过 Go／Java／Kotlin/Native 的综合运行性能为目标，对照 M62 代码更新历史暂缓项，覆盖测量与归因、跨过程／全程序优化、部分逃逸、托管数组 SIMD、PGO／ThinLTO、协程与 ABI、GC 晋升／并发、内存表示和库算法。
- 测量基础设施提前独立交付：复用 benchmark、采样工具和 GC 统计，补足源码归因与优化诊断，先支持通用静态优化。PGO 为后续独立专项，在代表负载、成熟消费者和反馈原型净收益成立后排期；测量与全程序优化／ThinLTO 均不以 PGO 为前置。
- 优化按长期、分批试验和交付推进：成本低且收益比较稳定的方案可直接纳入 release 默认设置；成本过高或收益不稳定的方案通过独立开关或等级决定是否启用、启用哪一级。默认值随实测结果调整，保留默认与调优配置的对照。
- [已采纳优化与开启条件](program-performance/ENABLED-OPTIMIZATIONS.md)简要列出各项方案的默认 profile、适用条件及实际开关，后续采纳与策略变更时同步维护。
- [启动阶段测量](program-performance/STARTUP-STATS.md)已通过 `--enable-startup-stats` 提供；默认预处理排除全部新增计时和报告，三目标默认程序的代码段与无测量基线逐字节相同。统计记录七阶段与已有索引数量，开关只改变 runtime／最终链接，作为注册与 stackmap 优化的指导，不提前引入 PGO。
- [启动 owner 验证合并](program-performance/STARTUP-OWNER-JOIN.md)已按测量结果交付：首次 stackmap 合并时完成一次 owner 解析，重复 ODR 复用结果，全部必要检查仍在 managed 执行前完成。三个目标的代表启动降低约 2.7%～8.1%，完整文件与静态元数据不增长；全部 profile 默认，ABI 和预编译 Cone 保持。
- [普通 managed poll 与有界叶函数](program-performance/ACTIVE-POLLS.md)已交付：普通 Full poll 仅一次 acquire 读取 phase，省去 TLS／mode／epoch，既有有界叶函数省入口；Gateway 保留首次激活及[本线程 relaxed 读取](program-performance/POLL-OWNER-READS.md)。三个目标动态调用内核降低约 16.5%／27.5%／39.5%，静态空间缩小，定向 GC／ABI 与编译成本已验证，所有 profile 默认。
- [minor 局部块遍历与连续清卡跳过](program-performance/MINOR-BLOCKS.md)已交付：年轻块／新目标与可用 region 分别索引，保留原 block 大小和 ABI，连续清卡批量跳过。原始 trees 三目标耗时降低约 7.9%／12.0%／11.8%，仅每个 region 增加 8 B、全局增加 16 B；定向 GC、实际负载、默认产物消费与空间／构建成本已核对，所有 profile 默认。
- [循环内联优先级与传值调用估价](program-performance/LOOP-INLINING.md)已交付：有限增长优先用于深循环，及时使用展开后的 exact receiver，循环成本考虑参数／返回值快照。原始 dispatch／values 三目标分别降低约 12.8%～29.9%／10.2%～33.9%，代码不膨胀，编译变化 −1.49%～+1.70%；定向语义／ABI／GC 验证通过，release 默认。
- [小正文导出与跨 Cone 主值构造器内联](program-performance/BODY-EXPORT.md)已交付：普通构造／字段写入／callable-owned 字符串进入有界导出，主值构造按已有角色选择；strmap 的快照／主值构造及部分检查调用消失，三目标降低约 3.1%～4.9%，静态数据减少，生产库增长小于 1%。定向语义／GC／跨 profile、旧库回退及编译成本已核对，release 默认；成本不合格的调用保留。
- [minor 脏卡组清单](program-performance/DIRTY-CARD-GROUPS.md)已交付：只枚举、清理已登记的 32 KiB 组，side metadata 约 0.049%，统一 runtime ABI 17；三目标稀疏旧堆增长项消除，真实程序与写入固定成本的回退如实保留，定向正确性、编译与空间成本已验证。下一批继续处理 nursery 自适应扩容／缩容与 survivor／晋升年龄。callable／stackmap 裁剪随后处理真实可达性与登记保留，避免把优化推进转成测量设施的扩建。
- [普通内联增长额度](program-performance/INLINE-GROWTH.md)已统一调整初始常数 96→128，保留总上限与执行阈值；ArrayList.get 调用消失，三目标遍历减少约 16%～24%，代码增加 32～64 B，release 默认。按实际成本保留其他 helper，未采纳增长回收试验。
- [跨 Cone 普通方法与 getter 内联](program-performance/MEMBER-INLINING.md)已交付：相同的 HIR 结构和 MIR 成本选择，保留完整 receiver、typed accessor identity 与动态分派边界；三目标用户成员循环降低约 47%～73%，正文、代码与编译成本很小，release 默认启用。反向 profile、异常、泛型、装箱、GC roots 和旧库回退通过。
- [Darwin 的 Scoop 符号镜像内绑定](program-performance/LOCAL-SYMBOLS.md)已交付：所有 profile 的最终链接移除 Scoop 动态导出、weak binding 和直接调用 stub，相容预编译库可直接重链接。代表程序缩小 16.84%，零次循环进程降低 46.60%；跨 Cone／GC、稳态回退与链接成本已验证。成员内联、内联选择、poll／TLS 和 runtime 启动登记继续优先推进。
- [函数级 CPU 采样归因](program-performance/CPU-PROFILING.md)已交付：所有 profile 可按需输出 symbols，复用当前及已加载依赖的正文身份；sample／perf 与 LLVM 处理实际符号、同址别名和主镜像 PC。三目标程序产物不变，声明与执行位置的区别、编译／空间成本和采样扰动已记录；指令行表、内联栈与 PGO 继续按实际消费者推进。
- [实施记录](program-performance/PROGRESS.md)记录各项交付、必要验证和后续工作；已交付预构建程序的可复现 A/B 测量、[完整父接口表投影与视图复用](program-performance/INTERFACE-PROJECTION.md)、[接口检查与取表合并](program-performance/INTERFACE-CHECKS.md)、[普通 class／局部游标字段替换](program-performance/CLASS-SCALARS.md)，以及[字段替换指导内联](program-performance/ITERATOR-INLINING.md)和[按需 MIR 优化决策报告](program-performance/OPTIMIZATION-REPORT.md)。真实 ArrayList 反复遍历的 iterator 分配已消除；Darwin／Linux GNU／musl 的定向正确性、生成代码及性能对照完成，未命中边界、实测对照回退、编译与空间成本一并保留。后续按改动选择必要验证组，继续完善归因并改进已定位的静态优化热点。
- 已交付[有界整数／Boolean 数组初始化](program-performance/ARRAY-INITIALIZATION.md)及[普通数组边界检查与分块](program-performance/ARRAY-BOUNDS.md)：稳定长度与归纳变量直接证明，动态长度关系用 guard 选择完整块并保留原 checked 路径，覆盖真实内联的读取／变换／复制。三目标正确性、移动 GC、大数组收益、动态短数组回退与编译／空间成本已测量；更广的容器访问、复杂条件与元素类型仍待后续。
- [循环范围指导内联](program-performance/ARRAY-INLINING.md)已闭合综合求和 getter 的增长额度缺口：复用范围证明，实际去除不可达检查后计价，在不提高阈值的条件下形成无调用内层。三目标运行收益、短任务及布局敏感回退、编译与空间成本已记录；更广容器与跨过程范围仍待后续。
- [严格浮点数组循环](program-performance/FLOAT-ARRAYS.md)已交付：Float／Double 复用范围证明和有界分块，逐元素运算保留精度与舍入次数，顺序求和保持原累加顺序；显式关闭 LLVM 向量化 hint 的隐含重排许可。三目标定向正确性、移动 GC、长数组收益、单元素回退及编译／空间成本已记录，有限范围沿用 release 默认能力。
- [含条件分支的数组循环](program-performance/CONDITIONAL-ARRAYS.md)已交付：单入口、单 latch 的无环正文保留实际分支与合流快照，整数条件内核获得 SIMD，浮点限幅与掩码求和保留标量有界内层。x86 最内层入口对齐闭合了实测布局回退；三目标定向正确性、GC、长／短路径及编译／空间成本已记录，未交付收益不稳定的分支填充实验。
- [数组搜索与提前退出循环](program-performance/EARLY-EXIT-ARRAYS.md)已交付：只复制继续迭代的自然循环区域，原出口共享本轮值及其副作用，保留首次命中、异常前缀与 GC。三目标长内核获得收益，首项／短路径回退、实际内联代码、空间与编译成本已核对；沿用 release 默认能力，搜索 SIMD 和更广循环变换继续独立推进。
- [引用数组范围证明与有界循环](program-performance/REFERENCE-ARRAYS.md)已交付：读取、复制、可空值、MaybeUninit 和静态父表投影进入已有有界内层，保留原写屏障与精确 roots。三目标读取／搜索获益，Darwin 写入获益，x86 带屏障写入基本持平；旧数组持有年轻对象、并发 moving／minor、短路径及编译／空间成本已定向核对。重复卡标记、bulk barrier 和引用 SIMD 继续独立测量。
- [卡表幂等原子写入](program-performance/CARD-STORES.md)已交付：利用二值卡状态与 STW 清零，以 relaxed store 1 替代单槽和 runtime 范围屏障的 OR 1。三目标引用写入获益，GNU／musl 长复制与清零降低约 70%～82%；LIR、roots、脏卡计数及 ABI 保持，空间／编译成本与对照回退一并记录，作为所有 profile 与 runtime 的默认实现。
- [有界引用循环页表查询提升](program-performance/CARD-REGIONS.md)已交付：数组基址定位 mapping，release 将每轮必经、目标稳定的查询移至完整块进入边，保留逐次 card store 和精确 roots。三目标普通引用复制内核降低约 45%～80%；后续[接口栈重取扩展](program-performance/INTERFACE-CARD-REGIONS.md)在 ARM64 release 继续覆盖接口复制／父投影／初始化，前两项再降低约 44%。x86 试验未取得稳定收益，保持原规则；空间、编译成本和对照回退均有记录，无新开关。
- [接口 SSA 快照复用](program-performance/INTERFACE-VALUES.md)已交付：x86_64 release 在同一无 safepoint 直线区间复用刚写入的完整接口值，消除局部复制链，接口数组复制／父投影也因此满足原有页表查询提升条件。GNU／musl 内核降低约 36%～40%；ARM64 试验回退，最终产物保持基线相同字节。精确 roots、接口布局和 ABI 保持，成本、复测与未采纳试验已记录。
- [有界引用循环卡标记合并](program-performance/CARD-BATCHES.md)已交付：每轮必经、固定槽步长的完整块在正常退出后统一标记，保留元素写入，先于下一次 poll／发布，每流最多三次卡写入。三目标复制与清零获益，GNU／musl 接口复制／父投影降低约 43%～47%；LIR、精确 roots、脏卡集合与 ABI 保持，text 增加约 0.03%，沿用 release 默认设置。初始化、求和及短路径回退和编译成本已记录，更广屏障合并继续独立测量。
- [有界引用读取部分展开](program-performance/REFERENCE-UNROLL.md)已交付：依据函数级采样，对小型单块只读内层作四倍展开，三目标 release 默认启用；保持 poll、精确 roots、短尾及严格浮点顺序。较长字段求和阶段降低约 13.5%／3.6%／3.6%，text 增加 40／0／0 字节；拒绝条件、卡标记组合及编译成本已定向验证。
- 后续[条件引用读取展开](program-performance/CONDITIONAL-REFERENCE-UNROLL.md)已交付：最多四个无环块复用四倍策略，保留空值 guard 和标量合流。交替空值内核降低约 51%／29%／15%，text 增加 64／128／128 字节；全 Some 的 Linux 小幅回退、二倍对照、短路径和编译成本一并记录，沿用 release 默认设置。
- [调用点异常路径特化](program-performance/EXCEPTION-PATHS.md)已交付：按 exact 类型和已有完整正文证明异常边不可达，保留正常清理并按剩余正文选择内联；不扩大一般 EH 内联或持久摘要格式。三目标 `use` 内核与直接写法接近，长阶段耗时降低约 97.2%／98.9%／98.9%，闭包和资源分配消失；真实抛出、跨 Cone、返回快照、移动 GC、编译与空间成本已核对，沿用 release 默认能力。
- [异常区域中的局部分配消除](program-performance/EXCEPTION-SCALARS.md)已交付：在分析图中分开块入口与正常完成点，box／closure／class 在异常支配和循环存活证明成立时转为局部值，保留真实 EH、字段快照和异常出口 roots。三目标带 catch 的 use 阶段降低约 92.6%／95.7%／96.2%，静态空间缩小，编译墙钟 −2.17%～+1.47%；有限范围作为 release 默认项纳入开启条件清单。
- [nursery 独占块的分配登记](program-performance/NURSERY-METADATA.md)已交付：复用已保留范围和单写者关系，以原子 load／OR／store 保留已有位图状态及 starts 的 release 发布，共享旧代继续使用原子读改写。三目标小／宽对象分配区间降低约 28%～53%，GC 计数、IR 与根 `.slib` 保持，text 增加 192～716 B，runtime 冷构建墙钟 −1.20%～+0.46%；所有 profile 默认采用，无新开关。
- [小型 TLAB 分配完成内联](program-performance/TLAB-COMPLETION.md)已交付：release 复用完整 LIR 布局，合格 16～128 B 常量分配直接完成清零、登记与计数；其他请求保留共享入口，统一 runtime ABI 16。三目标 16／24／48 B 阶段降低约 42%～59%，text 增加约 4.4%～8.0%，完整程序增加约 0.33%～0.41%；GNU 数组回退、musl 编译成本和定向验证均已记录，受限范围纳入 release 默认清单。
- [分配 context 首次使用复用](program-performance/ALLOCATION-CONTEXT.md)已交付：ARM64 release 复用调用内稳定的 TLAB context 地址，保留 GC／refill 后的状态重读和异常路径。16／24 B 阶段再降低约 16.5%／15.6%，text 缩小约 0.19%；x86 宽对象试验回退，最终 GNU／musl 产物保持基线相同字节。三目标定向验证、零／单次路径和编译成本已记录，ABI 16 与对象／根布局保持，无新公开开关。
- [确定初始化前尾部清零消除](program-performance/ALLOCATION-ZEROING.md)已交付：在直线初始化路径中缩短单次清零，保留对象头、引用槽、padding、发布与 roots；release 默认接受 ARM64 ≤128 B、x86 ≤64 B。三目标 24／48 B 阶段降低约 3.4%～17.5%，ARM64 128 B 降低约 26.4%；较大 x86 试验未形成稳定收益，保持原指令。text 不增加，编译、对照回退和定向验证已记录，统一 ABI 16 与产物格式保持。
- [动态接口目录测量](program-performance/INTERFACE-DIRECTORY.md)已补齐：真实装箱／集合／短目录等 9 种负载在三目标验证，O2 边界测试覆盖 0～17 项。四种循环展开存在短目录或 smartcast 回退，本轮未采纳，保留原 runtime 查询和全部父表投影能力；测量入口及取舍已登记。
- [循环中稳定来源的查询复用](program-performance/INTERFACE-QUERY-REUSE.md)已交付：release 对参数或所有 CFG 环外单次定义的局部引用，首次实际检查后复用 exact interface 的命中／未命中 entry；含 Boolean、空接口和稳定来源的异常继续路径，不增加对象副本、roots 或 ABI。三目标主要目录负载降低约 13%～87%，静态／编译成本和短目录、布局敏感回退均已记录。
- 延续桌面短暂停与内存目标，按吞吐、应用尾延迟、RSS 和启动分别验收；调研不表示优化已实现或已完成跨语言性能比较。具体实施前先修订实际受影响的规范，不预先占用新的里程碑编号。

### 可恢复语法与 LSP（公共基础 A～E 已完成，扩展能力待排期）

- 已按[公共基础设计](language-service/DESIGN.md)交付保留全部源码的 CST、缺失／错误语法和完整 AST 适配；错误文件仍可查询，成功编译继续要求完整 HIR。
- 文档同步、诊断、结构查询、项目输入、编辑态语义、导航／receiver 成员补全／签名帮助、引用与实现索引、版本化重命名和固定标点修复均已交付，完整范围与验收见[实施记录](language-service/PROGRESS.md)。
- 当前按文件重解析、按实际受影响 Cone 保守失效，已按实测优化正文定位与成员候选计算，见[性能记录](language-service/PERFORMANCE.md)。局部重解析、正文级增量分析，以及 semanticTokens、inlayHint、格式化和 call／type hierarchy 独立安排。

### M54 明确后续的客户端能力

- QUIC／HTTP/3；HTTP/2 WebSocket／Extended CONNECT；跨 origin 的 H2 连接合并、证书固定／对端链检查、TLS 会话恢复控制／0-RTT、主动 H2 PING 保活。先在所属协议／TLS 库设计与补齐，再由 client 组合，不属于 M54 的验收范围。

### M48 缓存直接复用与编译职责收敛（已完成，[设计](milestone48/DESIGN.md)、[验收](milestone48/ACCEPTANCE.md)）

- 先判断输入更新，再读取源码、探测工具或准备链接；库与最终程序命中时直接复用已有结果。
- 删除缓存内容复核、生产 replay、输入镜像和文件系统来源审计；保留相应 stage 的源码语义、ABI、GC 与实际链接职责。
- 迁移全部历史源码隔离／缓存破坏验收，保留真实语言与运行覆盖；按当前 release 工具建立冷暖基线并在完成后复测。
- 集中完成重复测试、消费重放与旧 profile 清理；Darwin Rust 5,302、Linux Rust 5,338 项通过，正式 CLI Darwin 4,089／GNU 4,078／musl 4,074 项通过。同机五轮性能和最终调用／I/O 观察完成，结果见[进度](milestone48/PROGRESS.md)与[验收](milestone48/ACCEPTANCE.md)；后续按用户要求推进 M38。

### 从 M26 移出的后续事项

- off-heap NativeByteBuffer 与 I/O 边界归入 M45：连续独占容量、同步 borrow、显式 close、失败原子性和 M24 hook 兜底已在 M45-4 本机完成。容量变化共用一个 GC-free pressure 提示，首版 GC 有意忽略；不建立外部内存总账或主动收集协议。共享 view 等仍留后续；不阻塞 M26 的集合与字符串闭环。

### 来自 M1

- ~~always-leak GC → M9 替换~~（已完成）；
- ~~编译诊断的 parser 错误恢复~~（已完成：lexer 收集多个可恢复词法错误；parser 按顶层声明、类型成员和块内语句同步，存在诊断时不发布完整 AST；编辑态 CST、恢复声明与局部语义已按[公共基础设计](language-service/DESIGN.md)交付）；
- 正式单文件Cone编译/运行模式 → M23-11 `scoop build/run <file>`。

### 来自 M2

- struct字段默认值与命名参数调用 → M17；次构造函数 → M19；
- struct副本更新表达式 `s.{ f: v }`（spec 4.5）→ M22；
- 不带标签的`break`/`continue` jump statement、`for`循环与区间 → M22；`do-while`与带标签的控制流仍待后续；
- 源码可命名的底类型`Nothing`（含signature、generic application与cast）→ M32（已完成）；
- 一般jump expression（例如`value ?: break`、argument/initializer中的jump）仍待后续；M22以`ControlOutcome`表达jump路径的semantic bottom，M32补齐可命名的源码底类型，不扩展一般jump expression语法；
- 定宽整数族 `Int8/16/32/64`、`UInt*`（spec 11.2；M22修订为`Int`/`UInt`固定i32、`Long`/`ULong`固定i64）→ M22；
- 整数溢出语义 → M22（spec 11.2已固定wrapping、除法与shift边界）；
- 内建 print 重载 → M7 转为 core 普通重载（设计已含）。

### 来自 M3

- ~~`!!` 失败 trap → `UnwrapException`~~（M8 已完成）；
- ~~`while` 条件中禁用 `?.`/`?:`~~（M17 已完成：HIR 条件 setup 区域在首次检查及每条回边前重新执行）；
- ~~`f(None, 1)` 式"先 None 后绑定"的推断~~（已完成：函数重载、泛型 enum/struct 构造统一按整组实参固定点推导，延迟上下文实参且保持源码求值顺序）；
- ~~显式类型实参 `f<Int>(x)`~~（M12 已完成：parser以事务式 probe保持与比较运算符消歧，HIR按完整实参列表定型函数、构造、变体与成员调用）；
- ~~`value` / `ref` 类型约束（spec 13.9）~~（M12 已完成：所有 generic声明保留类型化 kind bound，定义点与具体实例化点均检查）；
- ~~非 Unit 函数返回的分支穷尽分析~~（已完成：按顺序块、`if`、穷尽 `when`、`try/catch/finally` 组合分析可落空路径）；
- ~~`if` 作为表达式~~（已完成：分支尾表达式定型并写入隐藏结果 local；无期望类型时计算可表达 LUB，无 `else` 的值位置诊断拒绝）；
- ~~泛型 **struct** 声明~~（已完成：字段/方法类型形参作用域、构造推断与嵌套应用全链路落地；M13边界修订要求HIR生成独立的fully specialized local-concrete struct实体后再交给MIR）。

### 来自 M4

- core与用户代码同单元编译 → M23-3 single-Cone artifact与core分离；
- ~~注解仅 `@Intrinsic` 且仅 sysroot~~（M12 已扩展为类型化 FFI 注解族，并统一完成参数、目标和共存检查）；
- 构造函数式变体的默认值只支持常量表达式 → M17改为完整spec 8.5“定义处解析、调用处实例化”；
- ~~`when` 的表达式形态（产生值）~~（已完成：模式绑定、守卫与穷尽检查沿用语句形态，正常分支尾值统一定型，支持嵌套控制表达式）；
- 命名字段模式的子模式（`S { f1: 0, .. }` 字面量匹配——ast::FieldPattern 需扩展）→ M22；
- 表达式位的裸变体名解析推广到所有 enum（当前仅 `Option` 的 `Some`/`None`；spec 4.2/5.1 的"上下文可确定类型时可省略前缀"在表达式位只对 Option 生效）→ M22；
- tuple/struct 的穷尽性按"穷尽模式组合"判定（当前要求 catch-all 或 `else`；spec 5.2/5.3 的组合判定是保守简化）→ M22；
- ~~tagged enum嵌入struct/tuple/class字段时精确扫描~~（M13修订：pure-value variant共享payload，含ref variant使用独占slot及固定ref偏移，扫描不读取tag）；
- `for`解构，以及lambda/`val`既有解构与M18 class `componentN`能力的共享binding plan → M22。

### 来自 M5

- ~~`toArray` / `toMutableArray` 方法形式~~（已完成：与构造函数形式共用 `ArrayClone`，保持 memcpy 独立快照语义）；
- `for` 循环与区间 `IntRange` 等（spec 11.8；含 `..` 区间运算符与 rest 的共存验证）→ M22；
- `String` 下标/切片 → M26；
- 数组按长度 initializer 构造与普通 List/MutableList/ArrayList → M26；
- 数组 `==` 语义（spec 缺口，需先回 spec 第 10 章补充）；
- ~~数组字面量混合引用类型的 LOB 推导~~（已完成：唯一可表达最小上界；多个互不可比较的最小共同上界退化为 `Any`；数组元素位禁止值类型 auto-box）；
- ~~数组越界 trap → 异常~~（M8 已完成）。

### 来自 M6

- 次构造函数、`init` 块、body 属性（非构造函数属性）、`super` 调用 → M19；这些声明自身拥有的初始化体固定为非挂起上下文（spec 8.2、9.1.1；M10 设计已锁定）；
- interface 的属性与默认实现 → M21；
- ~~泛型 interface 与声明点 `in` / `out` 变型~~（已完成：接口应用类型贯穿 AST/HIR/MIR，位置合法性与变型子类型关系在 HIR 检查；MIR 按具体实参生成独立接口 TypeDescriptor，并为引用/值 ABI 生成变型 itable bridge）；
- ~~`equals` / `hashCode` / `toString` 的用户覆写~~（M14 已按接口化设计完成：`equals` 走成员 `operator fun`，`ToString` / `Hash` 显式adopt，vtable不再保留Any固定前三槽）；
- companion object 与 `object` 声明 → M21；`sealed`、class/interface delegation（`class C : I by impl`）仍待排期，property delegation由M21覆盖；object/companion初始化与property delegate协议不得隐式挂起（spec 8.2、9.1.1、9.2）；
- 顶层属性与 object/companion 的精确初始化时机、跨文件顺序及循环初始化诊断 → M21（M12 只设计 GC-free 常量初始化的显式 `@Global` / `@ThreadLocal` 存储与无 initializer 的 extern global；通用属性语义仍需按 spec 9.1.1 在实现前定稿）；
- `const val`（仅顶层/object/companion，HIR 编译期常量求值与依赖环检查，不生成 runtime initializer；spec 9.1.2）→ M21；
- 可见性修饰符 → M21（`internal` 必须先于 M23-5 多Cone名称语义完成）；
- `?.` 后随方法调用（`a?.foo()`）→ M18；
- smart cast 完整 flow analysis（当前简化：仅不可变局部变量、仅 `is`/`!is` 与 `&&`）；
- 基类构造委托实参不可引用构造函数属性（`class B(val x: Int) : A(x)` 中 `x` 暂不可用于委托实参——hir-lower 在空作用域降级）→ M19；
- ~~class 字段按 8 字节槽索引的约定与连续 sub-8 字段布局冲突~~（已修复：LIR `HeapLoad` / `HeapStore` 携带自然布局的字节偏移，连续 `Boolean` 不再被错误扩为槽）；
- ~~泛型成员函数~~（M14 已补齐 class/struct/enum 的 final direct generic method、两组 typed argument identity、bound/推导/callable reference 与单态化闭包；interface 方法自身泛型及泛型虚方法明确排除，见 M37 和语言规范 3.2）；
- `Any` 的 core 库形态与 `Nothing` 的源码定义及编译器闭环 → M32（已完成，[设计](milestone32/DESIGN.md)，[验收](milestone32/ACCEPTANCE.md)）。

### 来自 M7

- ~~两个泛型重载推导出相同类型实参时，单态化实例按符号错误合并~~（已修复：以 `GenericFunctionId + concrete type args` 为实体键，重载实例符号带定义 discriminator）；
- 候选集分层：局部函数层已由M11落地；M16统一当前local/member/extension/top-level/core层的applicability，显式import/星号import层由M23-1建立本Cone结构、M23-5接入跨Cone surface；
- 泛型候选MSC改用fresh-variable约束系统、统一postponed argument与候选诊断 → M16；M20的`_`部分实参继续复用同一solver；
- ~~`write` 的 `@Intrinsic` 退役~~（M12 已直接声明 `@Extern(abi = "scoop") fun write(String)`，作为 managed ABI direct-ref入口）；
- ~~`print` / `println` 的 `Any.toString()` 过渡分发~~（M14 已改为 `fun <T : ToString> ...` 的普通generic bound调用，并拆除Any固定槽）；
- 歧义/无匹配诊断的候选明细展示 → M16；
- 默认参数/vararg的决议规则 → M17；完整运算符重载 → M18；原`context`参数语义 → M27按task-local Context重写。

### 来自 M8

- ~~try 的表达式形态（`val x = try {...}`）~~（已完成：try body / catch body 共同定型，结果写入发生在 finally 前，finally 值丢弃且 return / throw 仍覆盖待定结果）；
- catch 遮蔽降级为警告（当前为错误；待警告级别诊断基础设施）；
- ~~finally 内的路径分析~~（已完成：必退出的 finally 覆盖 try/catch 的返回、异常与正常继续路径；可落空 finally 保留原路径结果）；
- libc++abi异常实现依赖、自有Level I personality与catch状态 → M25；
- 异常穿越 Scoop ABI FFI frame 的规则（维持 runtime spec 第 5 章的暂定“初版禁止”）。

### 来自 M9

- 分代（nursery、首次存活晋升、remembered set 消费卡片表、代间引用检查）→ M31，见[设计](milestone31/DESIGN.md)；STW 并行 mark → M34（已完成，见[验收](milestone34/ACCEPTANCE.md)）；survivor/多级代龄、并行搬迁与并发 collector 仍留后续；
- 精确消费statepoint stackmap、root relocation与Immix evacuation/defragmentation → M15；
- arena 扩容、多 region、集中 evacuation、空页／空 region 归还与相应碎片策略 → M34（已完成，见[验收](milestone34/ACCEPTANCE.md)）；
- ~~多 mutator STW协调、线程注册/握手与线程安全分配/根表 → M13~~（已完成：pthread registry、合作式epoch握手、per-thread TLAB及同步heap/root/handle/pin元数据；STW 并行 mark 已由 M34 交付，并发 collector 仍待后续）；
- ~~tagged enum的精确扫描描述发射~~（M13修订：移除`SCOOP_REFS_ENUM`按tag分派，独占ref-bearing slot的固定偏移可与`SCOOP_REFS_SEQUENCE`及数组元素扫描组合）；
- ~~hir-lower 的泛型 struct 字段类型形参作用域~~（已完成：移除 core GC struct 按名识别 stopgap，泛型定义本身不进入 MIR，仅发射具体实例）；
- 其余定宽整数族与固定宽度alias → M22；既有64位`Int`/`UInt`实现迁为canonical `Long`/`ULong` identity，新canonical `Int`/`UInt`为32位，`Int32`/`UInt32`与`Int64`/`UInt64`分别作为对应透明alias。

### 来自 M10

- ~~suspend function type、函数引用与 lambda/closure~~（已由 M11 完成；core 保留 `SuspendTask` / `SuspendRegistration` 最小协议，并已增加函数值形态适配 overload）；
- `launch` / `async`、dispatcher、事件循环、结构化并发与取消属于标准库设计，不进入编译器最小 core；
- ~~continuation 的跨线程恢复状态协议 → M13~~（已完成：生成的adapter/frame以acquire/release/CAS发布并竞争完成；调度器和线程切换策略仍由后续标准库定义）；
- ~~suspend FFI ABI（→ M12 实现既定禁令）~~（已决策：现阶段不支持；`@Extern` 与 `suspend` 互斥，挂起函数的声明引用不能在 `FunPtr` 上下文中解析为原生地址，不得生成 wrapper 或暴露 M10 hidden continuation ABI）；
- frame elision、栈上 fast path、共享 adapter 代码等优化；当前优先保留完全类型化、可由 GC 扫描的显式 frame/adapter。

### 来自 M11（设计预留）

- 显式 mutable/reference/move capture 或 capture list：当前只允许按值捕获 `val`、参数、`this` 等不可重新绑定的 binding，不做 effectively-final 推导，不为外层词法局部 `var` 隐式 boxing。未来方案必须以源码可见的新语法明确 lifetime、identity、并发与 ABI 成本，不能直接放宽 M11 的 `var` 诊断。

### 来自 M12（设计预留）

- ~~managed closure导出、callback token、类型化 invoke adapter、foreign-thread attach/detach及 `pthread_create` 组合闭环 → M13~~（已完成；M12 的裸 `FunPtr` 仍只表示同步同线程的静态 `@NoGC` callback地址，managed callback必须使用配对的trampoline与opaque context）；
- 无显式 context/user-data槽的 C callback API所需动态 trampoline或有限 slot registry仍待后续；M13 不通过泄漏 closure或把 managed ref伪装成裸指针支持它们。

### 来自 M14（设计预留）

- class upper bound → M20；M14的upper bound只接受完整interface application；
- 可作为普通表达式静态类型的交叉类型；M14的多个interface bound只构成type parameter能力集合；
- interface 方法自身泛型及任何泛型虚方法明确排除（语言规范 3.2）；泛型 interface 宿主仍支持普通非泛型方法和固定签名动态分派，不再预留 method-level specialization/itable ABI；
- generic `typealias`：M22只落地top-level非generic透明alias；带type parameter/bound的alias、递归generic alias及跨Cone打包/re-export仍待后续。alias不产生新的nominal application、layout、TypeDescriptor或单态化身份；
- generic extension property；普通member/top-level property自身不允许method式type parameter。该能力随extension property基础语义落地，并须定义receiver参数如何参与推导及getter/setter单态化；
- static nested generic type作用域沿M21保持独立，不隐式继承宿主参数；generic companion由M29修订为可使用直接宿主类型参数、每个完整宿主application各有singleton。普通nested object不随外层实参复制；`inner` type及outer instance/ref捕获仍待后续；
- 显式type argument中的`_`占位及部分推断 → M20；当前只允许“整组省略并推断”或“整组完整写出”；
- 更一般的polymorphic recursion。M14只接受generic callable递归SCC中环上参数替换合成为identity的可判定子集，并在参数增长/变化的递归环上定义处诊断；未来放宽必须提供结构化termination proof，不能以worklist深度、实例数或超时充当语义；
- 当前完整application范围内的fresh-variable/postponed-argument constraint system与MSC → M16；M20的`_`部分实参沿用该solver；
- runtime generic dictionary、witness参数、反射式bound调用或共享generic body；M14仅实现单态化，后续只有在代码体积、动态加载或其他明确需求出现时再设计，不能作为缺失concrete信息的fallback；

### 来自 M15（设计预留）

- GC-free release hook → M24（2026-10-04 已完成，[设计](milestone24/DESIGN.md)与[验收](milestone24/ACCEPTANCE.md)；完整构造后 ready，逻辑死亡且真正 reclaim 前同步调用 TypeDescriptor hook，正式 CLI 全量验收通过）。

### 来自 M22（设计预留）

- platform-native integer另行设计：决定`ISize`/`USize`与`IntPtr`/`UIntPtr`是否分立、data/code pointer表示资格及target-dependent const/layout规则，再逐项决定M22临时使用`Long`/`ULong`的Ptr raw/to/offset、`sizeOf`/`alignOf`、Array/String size/index及其他相关surface是否迁到相应native type；普通`Int`/`UInt`与`Long`/`ULong`自身的固定宽度不随target改变，迁移范围不能在本里程碑预判或静默扩大。
