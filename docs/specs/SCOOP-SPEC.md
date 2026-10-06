# Scoop 语言规范

2026-10-05，M29 设计修订 companion 的泛型规则：每个完整宿主类型各有自己的 companion 类型与 singleton，companion 可使用宿主类型参数，见 9.1.3、9.5 和 [M29 设计](../milestone29/DESIGN.md)。此项已在 M29 首批实现并通过三平台正式 fixture；M21 及后续历史 milestone 设计保留原文，其中“泛型宿主共享非 generic companion”的规则由本次修订取代。

2026-10-06，M29 将编码协议修订为 `Encodable<T>.encode(value: T, encoder: Encoder)`，与 `Decodable<T>` 一样由 companion 或普通 codec 对象实现。数据类型不因 codec 的存在获得接口；泛型、容器和 tuple 的两个方向均使用显式 codec 组合，见 9.5、11.13。Unit 使用独立的 UnitEncoder / UnitDecoder，固定类型身份、零大小值布局与返回 ABI 不变。本次文档修订先于实现迁移；协议、core 和 JSON 入口已迁移，自动派生及其验收继续实施。既有实例编码和条件 conformance 的验收不计作新协议验收；实际进度见 [M29 实施记录](../milestone29/PROGRESS.md)。

共有名义声明保存 `@NoGC` 值类型契约及在原形参域内推导的 GC-free 指针条件，该字段自 `hir/cross-cone-interface/43` 起启用。仅在签名、别名、父类型或嵌套 application 中使用依赖类型，也须满足同一契约；泛型替换继续传播尚未闭合的条件。旧 `/42` 及更早产物与缓存重建；完整字面量来源、默认值规则、runtime C ABI、对象布局和 GC 契约保持。详见实现规范 §2.2。

`for` 在 Export HIR 前展开为普通调用、接口适配、Option 操作和循环，共有 HIR 撤销专用 For 与 portable binding-plan 编码，statement tag 9 退役且不复用，该变更自 `hir/cross-cone-interface/40` 起启用。迭代协议、求值顺序、ABI 与 GC 规则保持，由实际类型与 callable 记录表达。

经直接依赖选中的名义类型，其 public 静态嵌套类型、object、companion 与可导入成员按实际 typed owner 继续查找，包括原声明 provider 仅作为 support 的情况。exact、star 和 public import 使用同一规则；support provider 的包仍不加入源码可见包集合。产物只保存实际终点公开绑定及其既有转导出引用，reader 不重复要求终点 provider 是 direct，也不补造外层命名空间的来源证明。该规则自 `hir/cross-cone-interface/39` 起启用；runtime ABI 与 GC 契约不变。

公开 typealias 的目标通过实际类型签名或直接 typed alias 边记录，外部 `AliasTarget` 只承担目标引用和实体归属检查，不再要求或保存别名专用的名称来源证明。源码的普通名称查找、可见性、类型实参和循环检查保持；已解析目标可来自可见类型的静态嵌套命名空间。该别名规则自 `hir/cross-cone-interface/38` 起启用；该批次共有 HIR 格式为 `/42`，旧 `/41` 及更早产物与缓存重建，不改变 runtime C ABI、对象布局或 GC 契约。

静态存储与初始化失败根按其实际值类型引用 layout/scan。当前 Cone 只发射自身拥有的布局与扫描定义；外来类型的静态根复用共有依赖查询取得的完整 value-layout 和 scan 记录，保留实际 provider、typed identity、定义与 relocation，不因本地持有该类型的值而重发射 foreign Strong。layout/scan 指纹节点引用已经解析的实际记录，不要求该类型在当前 Cone 定义；指纹补丁目标仍须属于当前产物。MIR 必须携带生成失败根所需的实际 Any 声明，LIR 不再缺省重建固定 core 身份。static-storage 语义记录新增 field 32 保存 layout provider，完整记录使用 fields 1～32；语义投影使用 fields 1～10 与 32。共有 strong-production 两种格式当前为 /13、/14；在静态根的 /11、/12 之后增加实际 callable 正文的 canonical LIR 摘要（实现规范 §2.5），旧产物和缓存重建。runtime C ABI、String 表示、初始化状态与失败缓存语义不变，不引入 ODR 或多 image 启动。

普通 catch 的绑定必须可以像其他引用值一样离开 handler：匹配 native payload 后，在绑定变量前物化一次 managed 异常对象，后续返回、存储和捕获使用该对象；native unwind record 仍按既有 cleanup 规则释放。初始化 catch 复用这次物化，不再次复制。每次 throw 仍创建独立 native payload，runtime C ABI 不变。

运行时类型转换的失败构造使用前端解析的实际异常类型与 constructor 引用，并沿共有的类型、callable、ABI 和 Link 路径消费。删除由 Cast 反向投影的独立 CastFailure call-site、RuntimeOperationDependency role，以及 reader 对同一目标再按 compiler protocol 进行资格判断的通道；普通源码调用的位置、参数、结果与 typed 引用检查保留。共有 HIR 格式更新为 `hir/cross-cone-interface/30`，原 call-site reason tag 2 与 external-reference role tag 9 退役，不复用；旧产物、profile fingerprint 与缓存重建。该调整不改变转换失败抛出 ClassCastException 的语言行为、runtime C ABI 或 String 表示。

引用上行转换在 HIR 中用显式 `ReferenceUpcast` 节点保存内部表达式及目标类型，不能直接改写构造、调用或局部读取的原始类型。MIR 使用已有 `Retype`，不分配对象、不改变引用身份；构造器仍按实际所属 class 分配。默认值正文使用新 expression tag 58 保存同一操作，tag 44 继续退役；共有 HIR 格式更新为 `hir/cross-cone-interface/30`，旧产物与缓存重建，不改变 runtime C ABI。

非泛型外来接口与 class 使用真实 nominal 声明、继承边和成员签名参与类型检查。经外来 open/abstract class 或接口的成员调用遵守相同的覆写与动态分派规则；final 成员保持直接调用。成员查找保留派生接口的有效覆写；class 及其基类链已有的匹配实现优先于接口声明，不把二者当作独立重载。类型别名不改变接口或槽身份，跨 Cone 的同名、同布局类型仍不相等。 外来成员属性沿相同继承与覆写规则选择实际 getter/setter；普通赋值、复合赋值及前后缀更新遵守 9.3.3 的单次求值规则。`val` 不允许写回，setter 的可见性独立于 getter；读取公开属性不能扩大其 setter 的访问范围。

删除仅由测试实现的默认值operation-typing、nested ABI及root/origin语义工厂和其证明数据、平行验证入口与专用测试。正式reader继续使用共有声明表、完整typed模板、类型与binder检查、来源位置、局部数据流及真实引用一致性检查。局部数据流直接借用模板与共有字段查询，删除重复body input及authority适配器；nested descriptor保留实际类型化身份、parent/path与binder数据，删除独立Standalone证明模式。语言操作规则由前端负责，不在IR/meta crate再复制实现。此清理不改变wire字段、profile版本、runtime C ABI或String表示。

参数自由依赖class通过共有名义声明取得真实身份、modality、直接父类型和声明序字段，引用类型按GC契约传播，递归引用字段不展开成递归值布局。HIR保留完整声明及解析后的字段、父类型，不重建同名本地声明。外来class构造仍是对真实constructor的typed调用；HIR→MIR消费已选MIR绑定中的ClassInitializer角色，使用共有class分配路径创建对象，再将该对象作为receiver调用实际provider的初始化实现。构造表达式返回分配的对象，物理initializer返回Unit；普通返回class的函数不走构造分支。本地与外来initializer共用Callee表示、参数求值顺序和GC处理。 core默认导入层的构造调用和静态限定名同时查询普通Type/Value binding；用户新增的class、enum和typealias与其他依赖使用相同声明路径，不限于预设内建名字。此能力不增加来源凭证、core分支或runtime ABI，沿用现有类型、MIR绑定及LIR布局格式。 M23-6的真实class运行在单个最终镜像中链接实际产物及runtime；LLVM stackmap段按各对象贡献的完整v3 blob逐个读取，长度由已有count和对齐决定，保留每份格式、记录唯一性与精确PC检查，不增加展开预算。该读取不要求M23-8的多镜像启动或M23-9的program-link。

默认值的完整调用域规则由定义方前端执行；继承使调用域扩大或类型变化时再次检查实际变化。跨 Cone 产物只保存已解析的 typed 引用、定义位置和完整正文，不携带逐引用 owner/direct/slot/target 访问证明；reader 的引用、类型、owner/binder 与格式检查不重复实现前端可见性语义。

共有导出绑定包含 enum 变体的真实 typed ID，nominal 的 `nested_bindings` 同时列出其静态命名空间中的嵌套类型、object value 与 enum 变体；变体归属由实际声明确定，不能误作包级值。`hir/cross-cone-interface/30` 更新该格式语义，旧 `/23` 及更早产物与缓存重建；既有 tag 不复用，不保留双轨 reader，runtime C ABI 和 String 表示保持。

enum 模式匹配与变体测试只读取已有值，不调用构造器。默认值跨 Cone 展开保留真实 variant、owner 类型及字段模式，遵循相同的可见性、类型和穷尽性规则；不得因此要求正文外再携带构造器访问资格。

tuple 等结构值作为普通字段或无类型参数 callable 的参数、结果时，使用原有结构类型身份、求值与 GC 规则。声明合法性由前端检查，合法类型组合不因缺少独立 layout 或 TD 定义而被拒绝；该组合不引入新的 ODR 或程序链接能力。

跨 Cone 的类型声明明确携带 struct 主构造器的真实声明引用，不能从参数形状、字段布局或 provider 身份推断。GC-free 值的主构造器可以在 `@NoGC` 代码中调用；参数、结果与局部值仍须满足 GC-free 规则，managed 次构造器仍禁止调用。产物格式与版本规则见实现规范。

跨 Cone 的 struct 构造调用使用实际声明的构造器 ID、所属 nominal、参数协议和定义处默认值，参与普通候选选择、命名实参、访问及类型检查。主构造器与次构造器均消费 provider 的真实构造定义；零大小参数保留语言求值顺序，大值结果遵循 canonical ABI。不能以同名或同布局的本地构造器替代，也不复制外来函数体作为本地定义。

无类型参数的跨 Cone 签名可以包含 tuple、函数类型和指针类型，组合中的每个 nominal 仍引用实际声明。类型解析不能因该签名不是单个 nominal 而拒绝合法调用；语言 effect、FFI、GC 及当前物化阶段的规则继续适用。

MIR 输出在 HIR→MIR 边界完成一次整模块结构、类型与实际外来 callable 检查，并同时保留已生成的 canonical foundation、共有依赖选择和完整物化引用。通用 MIR 输出保留完整泛型实体；Strong profile 的 ODR 能力门仍在其消费入口检查，并共享已有 canonical foundation。driver、MIR production 组装和 MIR→LIR 直接消费同一完整输出，不再从未变化的 Module 重建第二份 foundation、重复验证外来调用或重跑整模块检查。production 与模块之间仍核对实际 callable、入口和初始化关系；直接借用已有签名记录，不构造第二份预期桥表。外部新产物的格式、引用、ABI 与对象检查继续由 reader 负责。此清理不增加凭证、状态机、wire 字段或 profile 版本，不改变 runtime C ABI、String 表示或后续里程碑范围。

产物 reader 的 MIR 类型和 callable 校验使用同一次 HIR 类型基础与继承图构建。按真实依赖顺序解析完整 MIR 类型、callable 和 dispatch，并在每个 provider 的实际依赖范围内检查类型基础与父类型引用；随后为整个不可变依赖闭包构建一次继承图及槽关系，各 provider 共用这张图核对类型表示、有限 shape、方法、构造器、object、派发、equality 与初始化契约。不得因处理另一个 provider、类型或 callable 而完整重建已检查的上游继承图；继承环、实际声明、槽及依赖引用检查继续保留。后续 LIR 消费完整的 MIR 结果，保留各自需要的格式、引用、签名、ABI 与对象检查。此调整不改变 wire、profile、runtime C ABI 或 String 表示，也不增加来源工厂、资格状态或缓存凭证。

完整 LIR 输出已保存 canonical foundation，codegen 不从同一未变化的模块再次构造该 foundation。LIR 在既有 Strong 能力边界检查 ODR 后，只追加当前 Cone 的 Strong 定义，不再次扫描未变化的 callable/ODR 记录。production 的符号表直接使用本次生成的定义表，对象分区直接使用 production 的符号记录；这些数据在 production 组合边界检查后，codegen 按 typed definition、atom 和 symbol 解析每项实际发射引用，复用完整记录，不重建整份预期表或再次完整比较。C bridge 与 C layout 入口只检查实际 C ABI、callback 声明及所需类型关系；callback 指令的 typed 引用与操作结果、Scoop CFG、dispatch、safepoint 与 root plan 在对象代码生成边界检查；不在每个无关入口重复完整验证。外部产物读取的格式与引用检查保持，wire、fingerprint 字段和 runtime ABI 不变。

产物的 identity graph 从 manifest 的当前 producer、实际直接依赖和已经读取的依赖实体构成；不无条件注册 CORE 身份，也不为 CORE 设置单独的重复过滤规则。CORE 与普通 provider 的声明使用相同的 typed 引用解析，缺失依赖、身份冲突及非法引用由共有格式与引用检查报告。默认 core 依赖仍由正常构建与前端依赖发现加入 manifest；本项不改变 wire 或 runtime ABI。

代码生成在入口对本次完整且不可变的 LIR、目标 profile 和 executable 入口完成一次必要验证，然后按实际定义分成函数与非函数对象。各成员直接消费同一 LIR，不因发射另一个对象再次完整遍历类型、ABI、CFG、GC roots、safepoint 或身份表；LLVM 变换后的 IR 和新生成的对象字节仍在各自边界检查。generated-C 源码入口同样不在内部 helper 重复整模块验证。这一职责调整不改变产物格式、runtime C ABI、String 表示或链接语义。

普通 library Cone 导出的无类型参数 struct 可以直接作为下游的参数、返回值和嵌套字段类型，并以真实声明身份解析其成员。final 成员的默认参数、命名参数和 operator 与本地调用使用相同语言规则，private 成员仍不可由外部直接访问；同布局或同名类型不能替代声明身份。实现和产物版本见 [实现规范](SCOOP-IMPL-SPEC.md)。

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。 依赖查询直接使用真实 `ConeIdentity` 与 callable、property、type-alias 的类型化声明 ID；选择集合按这些 ID 保存完整接口和实际依赖路径。删除独立 world/projection/selection 品牌、仅为品牌服务的计数器和错误，以及从声明 ID 再映射到局部 u32 的三套重复表。候选选择依照当前依赖目录中的实际声明与表示，不要求由同一查询实例铸造；直接依赖的名称可见性、转导出路径、实际 provider 和引用完整性继续按共有规则检查。HIR 依赖目录、候选和已选声明只保存实际 provider 与类型化声明引用，不逐项复制 artifact 坐标/fingerprint 凭证；provider 身份直接来自已导入的共有 foundation，不再提供独立 certificate 或重算坐标身份。直接依赖与传递依赖使用同一输入数据和查询实现，直接依赖集合只决定当前源码可见的 package/public binding；不以分离的输入、视图或 seed 包装授予枚举资格。转导出保留实际 binding route，删除逐候选的 provider 凭证、重复 terminal 声明及其包装；候选选择使用已解析的声明目录，不重放未变化的 route 与 provider 证明。HIR→MIR 使用同次编译的依赖快照及完整声明：共有依赖选择在关联实际 MIR 定义时核对一次目标、逻辑签名和 GC effect，lowering 随后按实际 provider 与 typed target 消费同一记录，不重复比较未变化的声明和签名，也不再次比较来源凭证。缺失定义及最终 MIR 输出的结构、类型与外部引用检查仍在各自边界保留。普通构建依赖记录与缓存 fingerprint 继续承担定位和失效职责。MIR 外部 callable 引用保存实际 provider 与类型化声明，不另映射到带会话品牌的局部编号；调用位置保留 GC effect。MIR 类型与 LIR layout/ABI 选择按实际 provider 和 typed target 直接返回完整记录，不先铸造并验证中间 handle；依赖闭包、类型、ABI、物理引用和 GC 契约仍在其消费边界检查。同一声明或 target 在另一选择集合中是否存在，按实际目录查询决定，不依据集合生成顺序或计数器。该进程内数据简化不改变 wire/profile、实体身份或 runtime ABI。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

Strong production 的两种表示当前使用 `/13`、`/14`：删除初始化专用 ABI field 11 和外部服务表 field 12，section 保留 field 2～9，并由新增 field 13 保存实际 callable 正文的 canonical LIR 摘要，共九字段。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

core 是可由用户修改、扩展和重建的普通 library Cone。源码层面的特殊处理仅限于前端识别 `@Intrinsic`，并把它正规化为既有 typed IR，以及 desugar 通过普通声明引用使用基础库提供的类型和函数。sysroot 是默认查找位置，不是信任边界；源码目录、输出位置、相同 coordinate 或用户修改过的 core 不需要授权 token。metadata 解码、typed identity 一致性、依赖闭包、ABI、缓存失效和 slib fingerprint 使用所有 Cone 共用的规则。不得为 core 另建来源防伪、slot 授权、receipt 信任链或重复 pipeline；既有专用实现须合并或删除，旧文档的冻结条款不阻止此次清理。

String 的 intrinsic 角色关联实际 typed 类声明；跨 Cone 使用时与其他类型共用依赖查询和产物消费，不产生独立描述符授权或来源资格。 协议数据直接保存这些角色；String 不再另存 source/exact capability，也不通过独立 definitions 外层授予资格。exact 类型来自同一真实声明的 canonical nominal key，初始化服务与其他函数使用共有签名和 ABI。HIR 格式 `/4` 及旧产物重建规则见实现规范 2.11。

版本：0.10（草案）

## 1. 概述

Scoop 是一门静态类型、编译到原生代码（LLVM 后端）的编程语言。其语法以 Kotlin 的**核心语法**为基础，目标是：

- 尽量兼容 Kotlin 核心语法（类、接口、函数、泛型、控制流、协程等），不追求逐条全兼容；
- 排除一切与 JVM / JS / Kotlin Multiplatform 平台对接相关的扩展；
- 引入真正的**值类型**（struct / enum / tuple），并围绕值类型扩展解构与 `when` 的能力；
- 用 `Option<T>` 取代平台式的空指针语义；
- 提供基于核心库实现的字符串插值。

实现策略：Scoop 的泛型采用**单态化（monomorphization）**实例化，不依赖运行期类型擦除。

### 1.1 范围

本规范只覆盖：

- Scoop 的核心语法；
- 支撑核心语法所需的**最小核心库**（见第 11 章）。

除 11.10 为字符串构建提供的最小 `List` / `MutableList` / `ArrayList` 外，标准库的其他部分（其他集合、IO、并发工具、序列库等）不在本规范范围内。

---

## 2. 与 Kotlin 的关系

### 2.1 包含的 Kotlin 核心语法

除本规范明确修改或排除的部分外，Scoop 尽量兼容 Kotlin 核心语法，包括但不限于：

- 声明：`class` / `interface` / `object` / `companion object` / `typealias` / 属性（`val` / `var`，含委托属性）/ 函数 / 扩展函数与扩展属性；
- 类特性：主构造函数与次构造函数、`init` 块、继承（单继承 + 接口实现）、抽象类、可见性修饰符（`public` / `internal` / `private` / `protected`）；
- 数据与函数：局部函数、lambda、匿名函数、函数类型、callable reference、默认参数、命名参数、可变参数（`vararg`）、中缀函数（`infix`）、运算符重载、尾递归（`tailrec`）；
- 泛型：类型参数、完整类型实参、上界约束、`where` 子句与调用点类型推断；Scoop不采用Kotlin的声明点型变、使用点投影或star projection（见3.2）；
- 控制流：`if` / `when` / `for` / `while` / `do-while`、区间与迭代、`break` / `continue` / `return`（含标签）、异常（`try` / `catch` / `finally` / `throw`）；
- 空安全运算符：`?.` / `?:` / `!!`（语义见第 7 章）；
- 类型运算符：`is` / `!is` / `as` / `as?`、智能转换（smart cast，见 2.3）；
- 协程：`suspend` 函数与挂起调用（见 8.2）；
- 上下文参数：保留 `context(name: T)` 表面，采用 task-local 的运行期精确类型解析（见 8.3）；
- 编译期注解、静态类型描述，以及由显式 interface conformance 请求的方法合成（M29，见 9.4～9.6、11.13）；不引入通用编译期执行语言。

### 2.2 排除的内容

以下 Kotlin 功能**不属于** Scoop：

- 平台对接：
  - `expect` / `actual`（multiplatform）；
  - `@JvmStatic` / `@JvmField` / `@JvmOverloads` / `@Throws` 等一切 JVM 互操作注解；
  - `external` 声明（FFI 通过 `@Extern` 注解机制提供，见第 13 章）；
  - Java 互操作语义（SAM 转换、平台类型 `T!` 等）。
- 运行期反射：`KClass`、`::class` 的运行期反射 API、`kotlin.reflect` 体系。9.6 的静态描述只在编译期消费，不提供运行期字段枚举、按名称取类型或反射构造。
- `inline class` / `value class`（Kotlin 的 JVM 值类）：被 Scoop 的原生值类型（第 4 章）取代。
- `data class`：被 `struct`（4.1）取代；解构等能力由 4.6 的内建解构提供。
- Kotlin 的 `enum class`：被 Scoop 的 `enum`（第 4.2 节）取代。
- 空指针语义：`null` 字面量与 `T?` 的运行期 null 表示（见第 7 章）。
- 普通字符串的 `$` 插值：只有 f-string 支持插值（见第 6 章）。

### 2.3 语义调整

- 引用相等 `===` / `!==` 只适用于引用类型（见 4.4.2）。
- 智能转换（smart cast）仅用于 `is` / `!is` 类型检查；对 `Option<T>` 不提供"判非空后收窄"式的智能转换（见第 7 章）。

---

## 3. 类型系统总览

Scoop 的类型分为两大类：

- **引用类型（reference type）**：`class`、`interface`、`object`、函数类型、数组类型（第 10 章）等。其值是指向堆对象的 managed **ref value**；对象具有 identity，复制 ref value 只复制引用而不复制对象。
- **值类型（value type）**：`struct`、`enum`、`tuple`。无 identity，immutable，复制时复制完整值（实现可自行优化为内联或间接 ABI，语义上不可观察）。

### 3.1 顶层与底层类型

- `Any`：所有类型（引用类型与值类型）的根类型。值类型向上转型为引用类型时发生**装箱**（见 4.4.4）。`Any` **没有任何成员方法**：值相等走 `==` 的运算符决议（见 11.11），字符串化与哈希是独立的接口（`ToString` / `Hash`，见 11.11）——不把 `equals` / `hashCode` / `toString` 挂在类型根上（那是 Java 的遗迹）。
- `Nothing`：所有类型的子类型，无实例。值类型可以向下转型到 `Nothing`（实际上不可达，仅类型系统规则）。`Nothing`作为源码可命名类型以及一般jump expression的完整落地属于后续语言子集；M22的`break`/`continue`不会仅为表达这一底层语义而构造`Nothing`类型的表达式。

### 3.2 泛型

声明来自当前 Cone、普通依赖或 core 产物，不改变类型 application、bound、调用推断、默认值、成员或模式的语言规则；源码名称可达性与可见性仍按 12.4、9.1.5 检查。泛型正文在定义处绑定名称、重载和成员契约，实际类型替换不能重新选择定义处未选中的重载。普通声明中的封闭 application（如 `Box<Int>`、`I<Int>`）是完整类型，不因其原定义为泛型而成为不可物化的 source-only 声明。实现边界及共同 HIR 见实现规范 2.2 与 [M23-6a](../milestone23/stage6a/DESIGN.md)。

- 泛型在编译期**单态化**实例化：每个具体类型实参生成一份专门的代码。
- struct、enum 和 tuple 的内联值布局必须有限。字段或 payload 经实际内联的泛型形参返回同一值类型声明、且途中没有引用或指针边界时，在声明处报错；改变环上的类型实参不能消除此错误。此规则同样适用于来自依赖的泛型包装器。没有存入字段／payload 的 Phantom 参数以及仅位于引用或指针之后的参数，不构成内联布局依赖。
- function、class、struct、enum与interface都可以声明类型参数。generic class/struct/enum的constructor或variant、base/interface application、字段与成员都可以使用宿主类型参数，generic interface的父interface与成员也可以使用宿主类型参数。每个fully specialized nominal application生成独立的concrete identity和成员实现；class还生成对象布局、TypeDescriptor与分派表，struct/enum生成完整value layout与GC-free/扫描信息，interface生成独立TypeDescriptor与itable key identity。
- Scoop没有预定义`Self`类型、associated type或“当前实现者类型”的隐式占位符；`Self`也不是关键字，若出现在源码中只按普通名称解析。generic/interface契约若需要表达某个类型关系，必须用显式nominal type application或显式type parameter表示，编译器不执行`Self := 实现类型`替换。M29的`Encodable<T>`与`Decodable<T>`同样遵守此规则。
- 泛型调用与泛型值构造的类型实参由整组实参共同约束，推导结果不得依赖实参声明顺序。依赖期望类型的实参（如 `None`、空数组或嵌套泛型构造）可以由任意其他实参先绑定类型参数后再完成检查；类型检查顺序不决定运行期求值顺序，显式实参与缺省表达式严格按 8.5.3 求值。
- 调用点可以写显式类型实参：`f<Int>(value)`、`Box<String>(value)`、`Enum.Some<Int>(value)` 与 `receiver.convert<String>()`。列表仍须覆盖callee自己声明的全部参数位置，但任一位置可以写`_`请求继续推断，例如`convert<Int, _>(value)`或`Pair<_, String>(first, second)`；显式类型与`_`产生的fresh variable进入同一个candidate-local constraint system。`_`只在调用/构造的显式type-argument list中合法，不是类型，不能出现在变量、字段、返回类型、上界、cast目标或nominal type annotation中。泛型宿主的方法调用只列method自己的参数，宿主application仍由receiver确定；整组省略时继续使用普通推断。
- generic type application在类型位置必须覆盖全部参数位置；不支持裸generic type或少写参数，每个位置都必须是普通完整类型。`G<out T>`、`G<in T>`与`G<*>`均不是Scoop类型语法。generic class/struct constructor及enum variant构造产生exact application，可以整组省略实参或用`_`部分推断。
- primary/secondary constructor与enum variant constructor不声明独立type parameter；构造调用中的显式/推导实参只对应nominal host。只有普通callable可以在generic owner参数之外再拥有一组callable参数。
- 因此不存在类型擦除，也没有 `reified` 的运行期需求（见 8.4）。
- class、struct、enum与interface的nominal type parameter一律不变（invariant），声明处不能写`in`或`out`。同一nominal template的两个application只有全部类型实参逐项相等时才存在由该template产生的赋值/子类型关系；`G<S>`不会仅因`S <: T`成为`G<T>`的子类型。class的普通继承及interface的显式conformance仍可把一个concrete类型映射到声明中写出的某个exact base/interface application，例如`D : B<String>`仍使`D <: B<String>`，但不推导`D <: B<Any>`。
- 不变性同时适用于reference与value type argument：编译器不会为generic application的赋值、返回、分支合流或参数传递自动重建value layout，也不会把value argument装箱后改成另一个application。需要改变容器/包装类型实参时，程序必须显式构造、`map`/复制，或先把单个value显式转换为共同class/interface再构造目标exact application。
- 继承与implements闭包只合并完全相同的interface application；同一template的`I<Int>`与`I<String>`始终是不同契约、不同RTTI/itable identity。一个类型可以在继承图中到达二者，但必须分别满足其成员obligation；若替换后的签名无法由普通overload/override规则同时实现，则在实现类型定义处报错，不能按template id或擦除后的文本签名任选其一。
- Scoop不提供use-site `in`/`out` projection、star projection或wildcard capture。需要只读/只写抽象时，优先让消费操作本身成为带bound的generic callable，例如`fun <T : Animal> consume(values: Array<T>)`；需要保存未知application时，必须声明显式的非generic interface或用户实现的type-erased wrapper。语言不会隐式制造existential类型、runtime generic dictionary或capture-open dispatch。
- 除类型上界外，类型参数还可以用 `value` / `ref` 约束限定为值类型或引用类型（见 13.9）。
- 类型上界在参数列表中写作`T : Bound`，或在声明头后的`where T : Bound`子句中给出。同一参数至多有一个class上界，并可同时具有多个不同interface上界；class上界保证实际参数是该exact class application的引用子类型，成员候选包括class及其继承闭包。class/interface上界都必须是参数完整的exact reference application；不接受value type、函数类型、`Any`或另一type parameter，也不产生可作为普通表达式类型的交叉类型。`value` / `ref` kind bound与任一nominal上界互斥（见13.9）。
- 类型实参必须同时满足参数的全部上界；class/interface关系按普通继承、显式conformance及完整application identity判断，value type实现interface须有显式声明。泛型推导把上界纳入整组约束求解，不得先任选一个类型、再把bound失败降为警告或回退为`Any`。11.13的编码与解码能力属于显式codec值，不为目标数据类型增加特殊bound或结构型conformance。
- receiver为有界type parameter时，成员候选只来自唯一class上界、interface上界及其继承闭包；不加入`Any`成员或实际类型未在bound中声明的能力。generic template中的bound member在实例化时解析为concrete direct/virtual/interface call；单态化不需要runtime dictionary，但不取消actual concrete type本来具有的动态分派语义。 class 上界中的属性同样参与查询：存储、计算属性和函数值属性使用完整上界实参及其继承替换；读取、赋值、复合赋值和安全访问保留原 receiver 的访问域检查，getter／setter 在定义处选定。该规则与声明位于当前 Cone 或依赖无关。
- 每个合法且参数完整的exact class/interface application都是普通reference type，可以直接作为变量、参数、返回值、字段、cast目标和upper bound。Scoop不引入`dyn`/trait object语法、object-safety分类或可空witness；所有合法interface成员仍可经concrete、exact interface或bounded receiver调用。`Encodable<T>`、`Decodable<T>`及其方法也使用普通interface语义。
- interface方法现阶段不能声明自己的type parameter；`interface I { fun <T> f(value: T) }`在声明处即为编译错误。interface宿主可以generic，例如`interface I<T> { fun f(value: T) }`，完整application `I<String>`中的方法可正常itable分派。这是method-level generic dispatch ABI尚未定义的功能边界，不是允许声明后再限制调用形态的object-safety规则。未来开放时必须同时支持interface与bounded receiver调用。
- non-interface generic method必须non-virtual。class generic method必须语义为final；generic method不能声明为open/abstract/override，不能实现或覆盖vtable/itable slot。struct/enum方法本来即为final。调用根据exact receiver application与完整method argument使用direct dispatch，运行期派生class不能override目标。
- class/struct/enum泛型宿主上的泛型成员函数，以及带宿主类型参数环境的companion上的泛型成员函数，都拥有两组类型参数：宿主类型实参由接收者的exact静态application确定，方法类型实参由整组调用实参推导；两组实参共同组成该方法的单态化身份，顺序固定为“宿主类型实参在前、方法类型实参在后”。两组参数使用不同semantic identity，方法参数不得与宿主参数重名。companion的宿主实参来自9.1.3规定的完整宿主application，不从方法实参反推。调用点显式列表只写method自身参数，可整组省略，或覆盖全部位置并在待推断位置写`_`；两组参数的bound一起验证。interface本身的方法现阶段没有第二组参数，其companion的方法属于普通object成员。
- top-level、local与extension generic function，以及上述non-interface generic method，都可以使用inline upper bound与`where`。generic method的callable reference必须由期望函数类型唯一确定method全部实参，得到的是某个concrete函数值；Scoop没有first-class polymorphic function value。
- 所有type parameter declaration都不能写`in`/`out`。callable参数与返回类型在推导中的方向由constraint solver处理，不通过声明点variance修饰符表达；普通函数类型自身的参数逆变/返回协变继续按8.1.1处理，它不是nominal generic application之间的variance。
- `is` / `as` / `as?`不擦除generic argument，generic nominal目标必须是参数完整的exact application。例如`Box<Int>`与`Box<String>`、`I<Int>`与`I<String>`是不同检查目标，前者不会仅因`Int <: Any`匹配`Box<Any>`。generic body中的type parameter在普通单态化后引用concrete TypeDescriptor；不存在裸generic、star或projected runtime descriptor。
- 单态化必须结构上保证实例化闭包终止。同一generic callable递归SCC中的每个调用环，把宿主参数与callable参数组成的完整向量代回起点后必须逐项保持identity；普通直接/互递归因此复用同一concrete实例。参数替换非identity的环属于当前不支持的polymorphic recursion，在template定义检查时报错。非递归调用边仍可任意变换实参。编译器不得用递归深度、实例数量或超时阈值决定源码是否合法。

#### 3.2.1 非泛型透明 `typealias`

跨 Cone 的 `Alias(...)` 与本地别名遵循相同规则：先按真实 typed target 展开，再对目标声明的构造器执行普通候选决议。外来 alias、本地指向外来类型的 alias 及其链式组合保留目标的构造器身份、默认参数、访问域与 GC effect，不生成转发构造器。

别名限定的构造调用保留目标 application 的全部固定实参，包括 `Alias.Variant(...)`；实参表达式不能重新推断并替换这些类型实参。即使调用结果被赋给 `Any` 或没有显式结果类型，payload 与构造参数仍按别名展开后的目标类型检查。

当前语言子集支持top-level、非generic透明alias，例如`public typealias UserId = UInt64`、`typealias Names = Array<String>`。右侧必须是声明点可访问、参数完整的普通类型，可以是nominal application、tuple或函数类型；alias不能声明type parameter、捕获外层type parameter或出现在nested/local位置。alias可以引用其他alias，但展开依赖图必须无环；直接或间接递归均为定义处错误。

alias只建立名称、declared visibility、source origin与导出实体，不建立新的type identity、nominal application、layout、TypeDescriptor、RTTI、boxing、单态化实例、overload差异或ABI分类。类型检查与codegen一律使用完全展开的目标type；分别以alias和目标声明的同形overload是重复签名。alias可用于目标本来允许出现的type及type-qualifier位置；`Alias(...)`、`Alias.Variant(...)`或static/companion member lookup先展开目标再执行普通决议，不产生额外候选。visibility默认仍为`internal`；完全展开目标type tree中每个被引用声明的effective access domain必须覆盖alias自身的effective domain。因此internal alias也不能暴露file-private目标，public alias只是同一signature-exposure规则的更宽特例。M22定义的这一top-level、non-generic alias在M23起可写入`.slib`并经普通import、`public import` re-export与链式re-export跨Cone使用；它的alias identity保持origin Cone，使用处仍展开到同一typed target。声明type parameter的generic alias以及nested/local alias仍不在当前语言子集中，不因M23的跨Cone打包能力而获得半成品形态。

### 3.3 参数传递、receiver 与 `this`

Scoop 的源码函数/方法调用一律是 **pass-by-value**：每个实参表达式求值一次，再以所得值初始化 callee 中不可重新绑定的形参 binding。这条规则不按 value type / reference type 分叉：

- 实参为 value type 时，被复制的是完整值；callee 不获得调用方 binding/place 的别名。
- 实参为 reference type 时，被复制的是 ref value（当前实现中是一个 managed pointer-sized value），不是其指向的对象。调用方与 callee 因此持有两份相等的引用值，指向同一具有 identity 的对象；重新绑定任一引用 binding 不影响另一份引用，但通过它们对 referent 所做的合法修改对另一方可见。这与 Java 的引用参数语义一致，不是 pass-by-reference。

成员方法、计算属性 getter 及扩展函数的 receiver 使用同一规则。调用 `receiver.method(args...)` 时，receiver 只求值一次，并按值初始化隐含的、不可重新绑定的 `this: T` 参数；`this` 不是调用方 receiver binding 的别名。因此：

- value-type `this` 是 receiver 完整值的 method-local copy；值类型字段不可写，`this` 自身也不能被重新绑定。
- ref-type `this` 是 receiver ref value 的 method-local copy；它不能被重新绑定，但可以按 referent 类型的普通成员规则读写同一对象。
- 方法返回的 closure 若引用 `this`，按 8.1.3 捕获的就是这个隐含参数的值：value-type `this` 复制完整值，ref-type `this` 复制引用值。两者都不保留调用方 receiver binding/place。
- 经装箱值的 interface 分派调用值类型实现时，dispatch thunk 语义上用 box payload 的值初始化 value-type `this`；box identity 不会成为该 `this` 的 identity。

实现可以在不可观察时消除复制，或按目标 ABI 把大型 value 间接传递，但 IR 语义仍必须是参数值而不是调用方 place。当 value receiver 直接或间接含 `@InteriorMutable` value、出现 `addressOf(this)`，或其他 unsafe 能力可能观察存储时，必须物化独立的 method-local copy；所得地址不得指向调用方 binding 或 box payload，且有效期不超过当前 method activation。

---

## 4. 值类型

### 4.1 struct

`struct` 定义一个聚合值类型。

```
struct S(val f1: Int, val f2: String = "")
```

规则：

- 字段在主构造函数中声明，与 Kotlin 类的构造函数属性语法一致，但**只能用 `val`，不允许 `var`**（struct 不可变，`var` 字段无意义）。
- **字段不支持可见性修饰符，declared visibility固定为public**；effective domain仍与struct owner取交集（9.1.5），因此默认internal struct的字段不会导出Cone。
- **immutable**：struct 实例构造完成后不可修改。
- 支持主构造函数与次构造函数（secondary constructor）。secondary参数不声明字段，必须通过`this(...)`直接或间接委托到唯一primary；具体参数、委托、初始化与失败规则见4.1.1和9.1.1。
- **不支持 `init` 块**。所有初始化逻辑必须位于构造函数中。
- 不支持继承其他类型，但可以实现 interface（见 4.4.3）。
- struct 没有 body 内可变状态；body 中可以声明成员函数、伴生对象（companion object 本身是引用类型的单例）与计算属性。

#### 4.1.1 构造

struct 只能通过构造函数构造：

```
val s1 = S(10)                 // 主构造函数，默认值可省略
val s2 = S(f1 = 10, f2 = "x")  // 命名参数
```

不提供 `S { f1: 10, ... }` 形式的 struct 字面量（该形式与尾随 lambda 存在解析歧义，已排除）。

struct可以在body声明secondary constructor：

```kotlin
struct NonZero(val value: Int) {
    constructor() : this(1) {}
}
```

secondary参数使用8.5的required/default/`vararg`协议但不能写`val`/`var`，也不声明自己的type parameter或返回类型。每个secondary必须以`this(...)`直接或间接终止于primary；省略delegation、使用`super`、自环或多constructor环都是定义处错误。委托先产生完整immutable struct值，再从最内层到最外层执行secondary body；body可以读取字段、执行同步副作用或`throw`，不能修改字段、`return`或发布尚未完成的`this`。任一delegation/body抛异常时整个构造不产生值。

#### 4.1.2 派生行为

struct 自动获得：

- 结构相等：`==` 按字段逐一比较（**条件派生**——仅当全部字段可比较时可用，见 11.11）；
- 不自动获得`ToString`或`Hash`；两者都必须在struct声明中显式adopt并实现（见11.11）；
- 解构（见 4.6）：可按字段顺序或按字段名解构；
- 副本更新表达式（见 4.5）。

### 4.2 enum

`enum` 定义一个带负载的代数数据类型（类似 Rust 的 enum），取代 Kotlin 的 `enum class`。

```
enum E {
    SimpleVariant,
    VariantWithValue(Int),
    CompositedVariant(Int, String),
    VariantWithNamedField {
        f1: Int,
        f2: String
    },
    VariantWithDefaults(val f1: Int, val f2: String = "hello")
}
```

规则：

- 变体（variant）形式：
  - 单元变体：`SimpleVariant`；
  - 位置参数变体：`VariantWithValue(Int)`，可携带多个值；
  - 命名字段变体：`VariantWithNamedField { f1: Int, f2: String }`；
  - 构造函数式命名字段变体：`VariantWithDefaults(val f1: Int, val f2: String = "hello")`，**规则同 struct 的主构造函数**：字段可带默认值，构造时可用命名参数。
- 两种命名字段形式（block 式与构造函数式）语义等价；构造函数式额外支持默认值。
- **variant及其字段不支持可见性修饰符，declared visibility固定为public**；effective domain仍与enum owner取交集（9.1.5）。
- **enum 自身不支持构造函数、不支持 `init` 块、不支持成员属性**；body 中只允许声明成员函数与伴生对象（变体的构造函数式声明是变体定义的一部分，不在此限）。
- 每个变体是一个构造器：`E.SimpleVariant`、`E.VariantWithValue(42)`、`E.VariantWithNamedField(f1 = 1, f2 = "x")`、`E.VariantWithDefaults(1)`、`E.VariantWithDefaults(f1 = 1)`。block 式命名字段变体只能以命名参数构造；构造函数式变体沿用 struct 主构造函数的参数规则，允许位置参数、命名参数及默认值。两者均不提供花括号构造形式（与 struct 字面量同样存在解析歧义）。
- 跨 Cone enum 使用相同的变体构造、import/typealias、期望类型和默认参数规则；泛型变体的宿主实参由显式实参、payload 与期望 enum application 按普通约束推导确定。`E.V` 中的裸泛型 `E` 是声明命名空间，不先构造缺失实参的类型。值构造按实际声明身份生成，不以 provider 来源授予额外资格。
- enum 拥有 generic companion 不改变上述变体规则：限定调用先按实际静态 binding 识别变体，只有目标属于 companion 时才要求完整宿主实参（9.1.3）。变体构造不访问或初始化 companion。
- **变体不是类型**：不能用作 `is` 的检查目标、变量类型或参数类型；判断与提取负载通过 `when` 模式（第 5 章）完成。
- 与 Kotlin enum class 的 entries 类似，变体名可以通过`import some.package.E.*`引入后不写前缀直接使用；`scoop.core.Option.*`由core prelude的typed default import引入（见第7章），`Some`/`None`不具有短名称特判。
- 表达式位的裸`V`/`V(...)`除普通可见候选外，还可由唯一的expected exact enum application `E<Args...>`引入：只在该enum内寻找同名variant。expected type仍未固定时，该构造与`None`、lambda、空数组一样进入8.6的candidate-local postponed检查；最终expected为`Any`/interface、多个enum或未解变量时，不扫描全程序猜测，必须写`E.V`或补type annotation。普通词法/import候选遵守既有分层并优先；contextual variant不能绕过遮蔽，也不能扩展为按返回类型选择普通函数。unit variant的contextual name只在普通value-name lookup没有找到实体时启用，词法value binding即使类型不适配也hard-shadow该回退；payload variant call继续服从8.6既有named-call与local-value shadow规则。
- 在`when`匹配处，变体名可以省略`E.`前缀，由subject的exact enum type解析（见第5章）；这与表达式位的contextual candidate是两个不同入口。
- 与 struct 一样：immutable、无 identity，可条件派生结构相等；`ToString`与`Hash`必须显式adopt并实现（见11.11）。
- 命名字段变体的字段构造后只读。
- enum 可以实现 interface（见 4.4.3）。
- 泛型 enum 允许，例如核心库的`enum Option<T>`（见7.2）；不同类型实参形成互不转换的exact application。

### 4.3 tuple

tuple 是匿名的积类型，无需声明。

```
val t1 = (1, "hello")                       // 类型自动推断为 (Int, String)
val t2: (Int, Float, String) = (42, 4.2, "world")
val u1: Unit = ()                           // Unit（0 元 tuple）字面量
val u2 = Unit                               // Unit 既是类型名也是该值的构造器
val s1 = (42,)                              // 1 元 tuple，类型 (Int,)
```

规则：

- tuple 类型记作 `(T1, T2, ...)`，tuple 字面量记作 `(e1, e2, ...)`。
- **0 元 tuple 写作 `()`**；其类型名为 `Unit`，`Unit` 既是类型名也是该值的构造器，`()` 与 `Unit` 等价。
- **1 元 tuple 必须写作 `(e,)`**（尾随逗号），类型记作 `(T,)`；`(e)` 是带括号的表达式 `e` 本身。消歧汇总：`()` = Unit；`(e)` = 括号表达式；`(e,)` = 1 元 tuple；`(e1, e2, ...)` = 多元 tuple。
- 元素通过解构（见 4.6）或位置访问：`val (a, b) = t1`、`t1._1`、`t1._2`（位置访问从 `_1` 开始）。
- tuple 是值类型：immutable、无 identity；当全部元素可比较时条件派生结构相等，Unit无条件满足结构相等（见11.11）。
- tuple 不支持在源码中显式声明implements列表，也不支持命名字段，不实现`ToString`。M29的编码与解码分别由普通`Encodable<(T1, T2, ...)>`、`Decodable<(T1, T2, ...)>`实现承担，见11.13；不为tuple添加companion或条件接口。需要数据值自身实现interface时使用命名struct显式声明。

### 4.4 值类型通用规则

以下规则适用于 struct、enum、tuple 三者。

#### 4.4.1 不可变性

值类型实例构造后不可修改。对具有字段名的struct，可通过副本更新表达式创建新值（见4.5）；enum与其他值类型必须显式重建。也可以把任意新值重新绑定到`var`变量（这是重绑定，不是原地修改）。

#### 4.4.2 无 identity

值类型没有 identity：

- 对值类型使用 `===` / `!==` 是**编译错误**；
- 相等判断一律使用 `==`（结构相等）。

值类型装箱为引用类型后，其装箱结果按引用类型规则处理（见 4.4.4）。

#### 4.4.3 实现 interface

值类型可以实现 interface，但**不得因此获得可变性**：

- interface是普通nominal reference type；任何合法且完整的interface application均可承载class ref或装箱后的value，不存在trait object转换或“该interface是否object-safe”的额外判定；
- 实现 interface 方法或只读属性必须使用 `override`；值类型方法和 computed `val` 的 getter 始终为 final，省略修饰符的 `override val` 同样正规化为 final，显式 open/abstract 属性仍是编译错误。它们不参与 vtable 分派，但装箱为 interface 后通过该 interface 的 itable 分派；
- 实现 interface 的成员函数、属性 getter 不得修改 `this` 的任何字段（值类型字段本来就不可写，此规则是自然推论）；
- 若 interface 契约要求可变行为（例如要求实现 `var` 属性的 setter），值类型实现它是**编译错误**。

跨 Cone 的值类型遵守同一接口关系：struct、enum 及具有 intrinsic 表示的值类型从实际声明取得其接口和父接口，赋值、参数、返回及显式转换均使用相同的子类型规则。导入不能丢弃接口关系；同名接口、同布局值类型或 provider 身份不能替代真实类型身份。

整数和 Boolean 的 intrinsic 表示不固定其接口集合。前端从真实声明解析 `ToString`、`Hash` 及用户新增接口，后续阶段使用已解析的表示和声明身份；装箱不能要求该声明在当前 Cone 定义，也不能将导入的接口关系置空。intrinsic 注解的名称、类型参数和签名规则仍由前端检查。

声明查询按实际类型关系和转换操作进行。类型 arena 中存在某个 primitive，或未求值的默认参数引用它，不表示当前源码已经使用其装箱或接口表示。

```
interface Describable {
    fun describe(): String
}

struct Point(val x: Int, val y: Int) : Describable {
    override fun describe(): String = f"(${x}, ${y})"
}
```

#### 4.4.4 装箱与引用类型

- 值类型向上转型为**任何引用类型**（`Any`、它实现的 interface 等）时，自动**装箱**为堆上的引用对象（类似 Java 的 `int` → `Integer`）。
- 装箱后的对象具有 identity（可用 `===` 比较）且immutable。装箱不额外赋予相等能力：`==`始终按装箱后表达式的**静态引用类型**查找成员operator equals；`Any`没有该成员，未声明equals的interface也不能比较。经`as`/模式匹配取回原value后才重新使用value type的结构相等规则。
- **auto-boxing 只发生在 O(1) 场景**：单个值的转换（赋值/初始化、函数实参、返回值等单点转换）允许自动装箱；数组字面量的元素位置等批量场景不做自动装箱，需要显式 `as`（见 10.3）。
- `is` / `as` / `as?` 可用于判断与取回装箱前的值类型；`as?` 失败时返回 `None`（见第 7 章）。
- 值类型在类型系统上也是 `Nothing` 的父类型（可向下转型，语义不可达）。

### 4.5 副本更新表达式

副本更新表达式基于一个既有struct值创建“修改了部分字段”的新值：

```
struct S(val f1: Int, val f2: Int)

val s1 = S(10, 20)
val s2 = s1.{ f2: 30 }    // 新值：f1 = 10, f2 = 30
var s3 = S(1, 2)
s3 = s3.{ f1: 84 }        // var 重绑定，不是原地修改
```

规则：

- 形式为非空`baseExpr.{ field: expr, ... }`。`.`之后的`{`进入专用字段列表，因此不需要新关键字；块内不能出现语句、rest或嵌套field path。
- `baseExpr`先求值且只求值一次，静态类型必须是exact declared struct value；enum（包括具有命名字段payload的variant）、class、interface、tuple、basic type、intrinsic value family与函数值均不支持。结果为完整新值，原值及其storage不发生原地修改。
- 更新字段不能重复。目标确定后，各RHS按源码从左到右各求值一次并必须可赋给对应exact field type；未提及字段从保存的base值复制，最终按字段声明顺序构造结果。任一RHS抛出或挂起时后续RHS不求值，已发生的外部副作用不回滚。
- 字段只由base的exact struct identity与源码字段名确定；未知或重复字段、以及RHS无法赋给对应exact field type，均为编译错误。RHS type不参与目标选择。
- enum不是副本更新目标；即使其某个命名字段variant恰好包含全部所写字段，也必须用`when`匹配并显式重建。编译器不为副本更新生成active-variant检查、payload投影或variant不匹配异常路径。
- 不支持穿透`Option`的写法（`opt?.{ f: 1 }`）：先用`when`拆包，再做副本更新。

### 4.6 解构声明（destructuring）

`val` / `var` 声明中可以使用解构模式一次绑定多个变量：

```
struct S(val f1: Int, val f2: String)
val s = S(10, "hello")
val (n, _) = s                           // struct 按字段顺序解构，_ 跳过

val t = (1, 2, 3)
val (x, y, z) = t                        // tuple 按位置解构

val t2 = (4, 5, s)
val (a, b, { f1, f2: renamedF2 }) = t2   // 解构可以嵌套
```

本节定义的是**binding pattern**：用于`val`/`var`声明、`for`循环变量和lambda参数，必须对输入静态类型递归不可失败。parser在这些位置与第5章共用完整pattern syntax；`BindingPattern`是结合subject type后的成功语义分类，不是parser删减后的另一套语法。因此literal或显式variant shape先形成完整AST，再由本节规则给出稳定的refutable-pattern诊断。第5章的`when`使用范围更大的**match pattern**，额外允许literal与enum variant。一个外层tuple/struct不能用嵌套的可失败子模式绕过本条；`for (Some(x) in values)`不是过滤语法。

binding上下文中，除`_`以及4.3定义的内建Unit字面量写法`Unit`/`()`外，语法上只有一个普通标识符的pattern在任意嵌套深度都恒为新的binding；它不按subject type查询enum variant，也不查询import或既有value。例如`val None = o`合法并声明名为`None`的新binding，即使`o`的类型拥有或当前作用域导入了`None` variant。`Unit`与`()`仍是literal而不是binding；binding pattern不接收literal，所以二者在`val`/`var`、lambda与`for`中拒绝。对enum variant而言，只有显式写出variant shape的`E.V`、`V()`、`V(...)`或`V { ... }`（包括其限定形式）并按subject exact enum解析成功时，才会被识别为refutable variant pattern并在这些binding位置的任意深度拒绝；同形语法若解析为struct pattern，仍按本节的irrefutable struct规则处理。第5章“enum variant优先于binding”的规则只属于match pattern，不适用于本节。

规则：

- **tuple/struct固定元数位置模式**`(p1, p2, ...)`按位置解构；每个位置可以是绑定名、`_`、`..`或递归binding pattern。不使用`..`时元数必须与被解构值一致。class component位置模式不适用该元数规则，单独按下文处理。
- **字段模式**由`field`、`field: subpattern`和末尾可选`..`组成。`field`精确等价于`field: field`，展开后的RHS仍按所在上下文的完整规则分类；因此在本节的binding上下文中通常建立同名binding，但字段名恰为`Unit`时，RHS仍是内建Unit literal并被拒绝，需要写成`Unit: value`等显式不同绑定名。第5章的match上下文先应用同一个Unit literal特例，再对其他名称应用variant-first规则；若字段的exact enum type恰有同名unit variant，shorthand会匹配该variant，需要catch-all binding时应显式写成`field: value`等不同名称。`field: renamed`绑定到`renamed`；`field: _`显式忽略该字段。冒号右侧是完整递归pattern，不再只是rename。`_`不能作为字段key，但可以作为RHS。字段不能未知或重复，整个pattern内的binding名称必须唯一。
- 每个显式字段名先解析为subject exact type中的typed field identity。完整pattern shape、第5章的coverage vector及后端layout映射一律按字段声明顺序；binding plan的运行期动作则按源码显式字段的书写顺序depth-first执行。末尾`..`只为未列字段补充shape/coverage所需的wildcard，不产生投影、temporary或binding动作。未列出全部字段时必须以`..`结尾；列全时可省略。字段模式可带type前缀（`S { f1: x, .. }`），前缀必须解析到subject的exact type。
- struct既可按主构造字段顺序位置解构，也可按字段名解构；tuple/struct投影是语言内建能力，不查找`componentN`。
- 普通class可以按位置使用9.3的`componentN` operator解构。每个实际位置按顺序选择并调用唯一typed operator；写出的N个位置精确调用`component1`至`componentN`。每次调用正常返回后，结果先保存到新的immutable hidden temporary，再depth-first处理对应子模式。class没有声明式总元数，因此该位置模式不能包含`..`；调用可以按普通规则抛出或在允许的上下文挂起，但pattern本身没有“匹配失败”分支。挂起调用以异常恢复时等价于在原调用点抛出，后续动作不执行；在不允许挂起的求值上下文中选到`suspend componentN`是effect错误。
- binding pattern的不可失败性递归成立：binding、`_`、rest补位不可失败；tuple/struct只有全部展开后的子模式不可失败时才合法；literal（包括`Unit`/`()`）与上述显式enum variant shape在binding位置均非法。lambda和`for`引入的binding不可重新绑定；`var`解构声明中只有用户可见的叶binding是mutable，subject、投影及component结果等hidden temporary始终immutable。
- 三种binding位置共享同一求值、投影与component计划；每个完整subject只求值一次并进入immutable hidden temporary，位置元素按源码从左到右、命名字段按源码书写顺序递归depth-first执行。任一component调用抛出时，已经发生的外部副作用不回滚，后续投影、调用和binding均不执行；调用挂起时保存subject与全部已完成的hidden temporary，恢复后从该调用返回之后继续，不重新求值subject或任何已完成步骤。parser可以为lambda参数消歧施加语法限制，但不能改变后续语义。

#### `..` 忽略其余字段

`..` 用于忽略连续的一组或所有其余字段（规则同 Rust）：

```
val t = (1, 2, 3, 4)
val (x, .., y) = t       // x = 1, y = 4
val (a, ..) = t          // a = 1

struct S(val f1: Int, val f2: Int, val f3: Int, val f4: Int)
val s = S(1, 2, 3, 4)
val S { f1: x, f2: y, .. } = s
```

- 在tuple、struct及enum variant的固定元数位置模式中：`..`忽略任意数量（含0个）的连续元素，可出现在开头、中间或结尾，但**至多出现一次**（出现两次会产生歧义，是编译错误）；使用`..`时不要求模式元数与值的元数一致。class component位置模式始终禁止`..`，不适用本条。
- 在字段模式中：`..` 只能出现在最后，表示忽略所有未列出的字段；**未列出全部字段时必须写 `..`**（列出全部字段时 `..` 可省略）。
- `..` 自身不绑定任何值。

#### `..` 与区间运算符的消歧

`..` 同时是区间运算符（`1..4`，见 11.8）。两者不会歧义：

- rest 模式只出现在**模式位置**（解构声明、when 模式分支、for 循环变量、lambda 参数）；
- 区间运算符只出现在**表达式位置**（普通表达式、when 的 `in 1..4` 分支等）；
- 模式位置的元素位只接受绑定名、字面量、`_`、`..`、嵌套模式，区间表达式本来就不是合法的模式元素。parser 在模式位置看到裸 `..` 即 rest；看到 `expr .. expr` 形式则报"模式中不允许区间表达式"。

### 4.7 零尺寸值类型（ZST）

零尺寸是concrete exact value type的结构属性，不是新的source kind；M23 v1的`ZstStatus`与target无关，target profile只决定其正alignment及外层ABI细节。`sizeOf<T>() == 0uL`的concrete value type称为ZST；它仍保留自己的nominal/structural identity、generic application、方法、构造过程与TypeDescriptor，不能因layout相同而与另一类型合并。

- `Unit`和空的普通非`@CLayout` struct固定为size 0、alignment 1；非空tuple及其他普通非`@CLayout` struct在每个元素/字段都是ZST时也是ZST，alignment取字段alignment的最大值且始终为正的2次幂。ZST字段不增加aggregate size，可以共享offset 0；wire/runtime模型仍保留大于1的ZST alignment以便组合layout与未来扩展，但M23不新增独立的源码over-alignment语法。M23也不对enum做“单值所以删除discriminant”的新优化：tagged enum至少保留现有tag，`Option<ZST>`必须保留可区分`None`/`Some`的tag；
- ZST必须是GC-free且递归scan为空；反向不成立。它没有可区分值的存储bit，但构造器、initializer、getter、函数调用、array literal元素、赋值RHS及用户方法的求值和副作用一律保留。复制ZST不复制字节，不得借此删除产生该值的求值；
- ZST值仍遵守4.4.2的“无identity”。需要`addressOf`的独立source place必须在其有效期内物化满足alignment的非null 1-byte **address token**；同时存活且语义上不同的address-taken place不得共址，生命周期不重叠时可复用。token只提供place地址，不把语言值的size改成1，也不允许读写该token作为payload。字段和array元素仍不在13.10现有`addressOf` lvalue集合内；
- Scoop typed ABI保留完整source signature中的exact ZST参数/结果identity，但machine ABI使用显式`ElidedZst`分类，不传递payload byte。参数表达式仍按顺序求值；callee仅在地址被观察时物化自己的place token。`Unit`的void result与其他ZST的elided typed result在IR中是不同variant，不能因物理签名相同而合并overload、symbol或callable identity；
- 装箱ZST仍分配带对象头的普通managed object，每次装箱产生普通ref identity；未装箱payload size为0、scan为空，但TypeDescriptor以runtime spec 2.2的`BoxedValue { ZeroSized }`保存对齐后的payload offset与非零minimum object allocation。不同exact ZST继续使用不同TypeDescriptor，即使最终对象大小相同。

---

## 5. `when` 表达式扩展

Kotlin 原有的`when`语法（等值匹配、类型匹配、区间、条件分支）原样保留。以下扩展对**enum、tuple与struct**生效。它们与4.6共享tuple/字段/rest结构语法，但这里使用可失败的match pattern，额外允许literal与enum variant递归出现在子模式中。class的`componentN`位置解构只属于4.6的binding位置，不进入match pattern或穷尽性算法。

`if`、`when` 与 `try` 都是表达式，也可在结果被丢弃的语句位置使用。作为值使用时，每个可正常结束的分支块以最后一个表达式的值作为该分支结果；空块或以非表达式语句结束的块结果为 `Unit`，以`return`/`throw`或8.7的`break`/`continue`结束的路径不参与结果类型合并。外层有期望类型时，每个正常分支结果必须是其子类型；否则取所有正常分支结果的唯一可表达最小上界，存在多个不可比较的最小共同上界时退化为 `Any`。依赖期望类型的分支结果可由其他分支先确定类型，分支检查顺序不影响结果。

M22实现子集把`break`/`continue`与既有`return`/`throw`统一视为jump statement，而不是一般expression。`break`/`continue`的成功语法位置只包括块内的完整语句、直接作为不带花括号的`if`分支体，以及直接作为`when`箭头后的单条分支体；即使外围`if`/`when`/`try`正在值位置使用，jump自身仍是终止该分支的语句。`value ?: break`、`f(break)`、`val x = break`、`(break)`以及把`break`/`continue`用作receiver、operator operand或其他子表达式均不属于M22成功语法，必须产生稳定的未支持诊断且不得进入成功AST。一般jump expression与源码可命名的`Nothing`一并留给后续子集。

值位置的 `if` 必须有 `else`；结果被丢弃时可以省略。值位置的 `when` 必须穷尽。`try` 的结果由正常完成的 try body 与各 catch body 共同决定；`finally` 的结果值始终丢弃，但其中实际离开当前finally的`return`/`throw`/`break`/`continue`仍按第8章覆盖先前路径。

**统一规则**：仅当 subject 的静态类型是 enum / struct / tuple 时，`when` 才按模式匹配解析（下称**模式 when**）；其余 `when` 一律保持 Kotlin 的表达式语义——分支条件是普通表达式（等值比较）、`is` 检查、`in` 区间等，穷尽性也遵循 Kotlin 自身规则（表达式形式须穷尽，语句形式不强制）。两种解释的适用 subject 类型不相交，因此同一分支写法不会产生二义结果：parser 在分支条件位置同时接受模式语法与表达式语法，由语义分析按 subject 类型裁定。

模式when适用两条全局规则：

- **穷尽性**：模式when（无论作为语句还是表达式）必须由递归pattern-matrix证明穷尽；不能证明时是编译错误并给出至少一个稳定missing witness，也可以显式加`else`。带guard的arm不贡献覆盖，因为guard可能为false。
- **match中的绑定优先**：本条只适用于`when`的match pattern，不改变4.6的binding上下文。分支模式中的裸标识符不引用既有变量；M22只允许literal直接匹配值，匹配既有const须改用guard，限定名只用于enum variant。4.3的内建`Unit`/`()`首先按Unit literal分类；enum即使声明同名`Unit` variant，也必须以`E.Unit`或相应显式payload shape匹配。除此之外，enum subject下先按其exact type查询同名variant：命中unit variant时形成variant pattern，命中带payload的variant时报告缺少显式payload shape的错误，只有名称完全未命中时才建立匹配一切的新binding（效果同`else`）并给出非致命warning以避免拼写错误。相反，4.6中除`Unit`字面量外的普通裸标识符不执行这一步variant查询，即使名称相同也恒为新binding。

穷尽性按以下typed constructor递归定义：

- enum的有限constructor集合是其全部variant，variant payload继续作为子列检查；出现variant名称本身不代表覆盖其全部payload；
- tuple与struct各有一个product constructor，字段按位置/声明顺序展开；命名字段与`..`先补全为完整wildcard vector；
- `Boolean`有`false`/`true`两个有限constructor，`Unit`有一个constructor；
- binding、`_`、rest补位与`else`均为wildcard；fixed-width integer是有限域，实现以已出现literal singleton与符号化other partition计算覆盖，不实际枚举`2^W`个值。若不同无guard literal确已覆盖全部bit pattern，则无需wildcard；否则missing witness必须是该exact signed/unsigned数学次序中的真实缺失值，unsigned witness始终使用`u`后缀以保证可按subject type重新解析。M26 的 Char 同样使用 literal singleton 与符号化剩余集合，其有限域只包含 Unicode 标量值，缺失 witness 不得落入 surrogate 区间，使用可重新解析的字符字面量。String是无限开放域，有限literal arm仍必须有覆盖余值的wildcard；Float/Double literal coverage由其进入已实现子集时另行规定；
- 多个arm可以组合覆盖product，例如`(true, _)`与`(false, _)`共同穷尽；`Some(0)`与`None`不穷尽`Option<Int>`；
- 运行期始终按源码first-match顺序工作：先匹配结构，成功后才求值guard，guard为false时从下一arm继续。穷尽proof不得改变该副作用顺序。

integer literal pattern还允许unary minus直接作用于literal；括号不形成语义节点，空白或注释不影响识别。它以subject的exact integer type复用11.2的fit、wrapping unary-minus与signed `MIN`边界，其他常量表达式不属于literal pattern。

实现可同时计算pattern usefulness，但任何优化只能消费已经类型化的proof；不得因为最后一个arm或每个variant名至少出现一次，就把其条件视为恒真。

### 5.1 enum 变体模式

```
val v: E = ...

when (v) {
    SimpleVariant -> print("simple")
    VariantWithValue(n) if (n > 42) -> print(n)   // 解构 + if 守卫
    VariantWithValue(n) -> print(n)               // 上一分支的回退
    CompositedVariant(_, s) -> print(s)           // _ 省略不关心的字段
    VariantWithNamedField { f1, f2: renamedF2 } -> {
        print(f1)                                 // 按名绑定
        print(renamedF2)                          // 字段重命名绑定
    }
}
```

规则：

- 匹配对象是 enum 值时，分支条件可以是**变体模式**：
  - 单元变体：直接写变体名；
  - 位置参数变体：`Variant(p1, p2, ...)`，参数位可以是绑定名、字面量（等值匹配）、`_`（通配）、`..`（忽略其余，规则见 4.6）或嵌套模式；
  - 命名字段变体：`Variant { f1, f2: subpattern, .. }`；`f1`是`f1: f1`的shorthand，冒号右侧可为binding、literal、`_`或任意嵌套match pattern；未列出全部字段时必须以`..`结尾。
- 变体名可省略 enum 类型前缀（`E.`），编译器按被匹配值的类型解析；存在歧义时需写全限定名。显式前缀遵守普通类型名称的包、import、嵌套声明和可见性规则。前缀直接命名泛型 enum 声明时，以 subject 的完整 application 确定类型实参，不要求在模式前缀重复写实参；前缀是 typealias 时，其展开后的完整类型必须与 subject 相同。
- enum穷尽性按全局pattern matrix递归检查variant payload；仅列出全部variant名称但payload覆盖不全仍不穷尽。

### 5.2 tuple 模式

```
val t = (1, 2, "hello")

when (t) {
    (x, _, s) if (x == 42) -> { print(x); print(s) }
    else -> { /* ... */ }
}
```

规则：

- 匹配对象是 tuple 时，分支条件可以是 **tuple 模式** `(p1, p2, ...)`；元素位规则同 4.6 的解构（绑定名、字面量、`_`、`..`、嵌套模式）。
- tuple 模式要求 subject 的静态类型就是 tuple；tuple 没有类型名，不能像 struct 模式那样作为类型测试使用。
- 不使用 `..` 时，模式的元数必须与被匹配 tuple 的元数一致，否则编译错误。
- tuple无sum variant，但各字段的有限constructor组合可由多个arm共同覆盖；是否需要`else`完全由全局pattern matrix决定，而不是只寻找单个irrefutable arm。

### 5.3 struct 模式

```
struct S(val f1: Int, val f2: Int, val f3: Int)

val s = S(1, 2, 3)

when (s) {
    S { f1: 0, .. } -> print("f1 is zero")              // 字面量匹配 + ..
    S { f1, f2: y, .. } if (f1 == y) -> print("equal")  // 绑定 + 重命名 + 守卫
    S { f1, .. } -> print(f1)                           // 兜底，穷尽
}

when (s) {
    (x, 0, _) -> print("positional")   // 位置模式同样可用
    (x, ..) -> print(x)
}
```

规则：

- struct 模式的形式与 4.6 的解构声明完全对齐：字段模式（可带类型名前缀）或位置模式；元素位可以是绑定名、字面量（等值匹配）、`_`、`..` 或嵌套模式，与 enum 变体模式、tuple 模式一致。
- 字段模式未列出全部字段时必须以 `..` 结尾（同 4.6，与 Rust 一致）。
- 被匹配值的静态类型就是该struct时，type前缀可省略（`(x, ..)`或`{ f1, .. }`均可）；多个struct arm可以通过字段子模式组合证明穷尽，不要求存在单个全wildcard arm。
- struct 模式要求 subject 的静态类型就是该 struct（统一规则）。对父类型（`Any`、interface、装箱后的值等）的类型判断使用 Kotlin 的 `is` 分支（`is S -> ...`），智能转换后可在分支体内按 4.6 解构。

### 5.4 守卫（guard）

- enum 变体模式、tuple 模式与 struct 模式之后都可以跟 `if (condition)`；条件中可以使用该模式绑定的变量。
- 守卫为假时继续匹配后续分支。

---

## 6. 字符串插值

Scoop 的字符串插值是语言特性，由核心库的 `StringBuilder`（见 11.6）实现。

### 6.1 语法

```
val n = 42
val s = f"the number is ${n}"
val s1 = f"""this is a multi-line string,
this is the second line and the number is ${n+1}"""
```

- `f"..."`：单行插值字符串；
- `f"""..."""`：多行插值字符串（保留换行，与 Kotlin raw string 规则一致，允许 `${...}`）；
- `${expr}`：任意表达式；`$name` 不允许，必须带花括号（避免歧义，统一脱糖规则）。
- `$` 后紧跟 identifier-start 时按非法 `$name` 诊断并提示 `${name}`；`${` 以外的其他 `$` 是普通字面字符。单行 f-string 中 `\$` 同样产生字面 `$`；raw multiline f-string 不处理反斜杠转义。
- **只有 f-string 支持插值**。普通字符串（`"..."`、`"""..."""`）中 `$` 是普通字符，不触发任何插值或脱糖——这与 Kotlin 不同，迁移 Kotlin 代码时需注意。

### 6.2 脱糖

插值字符串脱糖为 `StringBuilder` 的链式调用：

```
val n = 42
val s = StringBuilder().add("the number is ").add(n).build()
val s1 = StringBuilder().add("""this is a multi-line string,
this is the second line and the number is """).add(n + 1).build()
```

规则：

- 脱糖在编译早期完成；`${...}` 中的表达式按普通代码类型检查。
- M26 按源码顺序交错执行每段表达式与对应 `add`：前一表达式及其 `toString()` 完成后，才开始下一表达式。表达式或 `toString()` 抛出时，后续段和最终 build 不执行；挂起表达式仅在原上下文允许时合法，builder 按普通 managed local 跨挂起保活。
- 脱糖引用实际 core `StringBuilder` 声明及普通构造、add、build callable，使用 hygienic temporary；用户同名声明、alias 或 import 不替换脱糖目标，普通用户书写的调用仍遵守名称查找。通用 add 使用普通 `T : ToString` bound，不为插值增加 Any 字符串化回退。
- f-string 不属于 `const val` 的常量表达式，即使只有文本段；普通字符串和 raw 字符串仍可作为 String 常量。实现可以在合法性检查后优化纯文本结果，但不改变其 const 可用性。

M26 同时补齐普通 `"""..."""` 与 `f"""..."""` 的 raw 多行字面量。普通单行字符串、单行 f-string 的文本段与字符字面量共用转义集合：`\t`、`\b`、`\n`、`\r`、`\'`、`\"`、`\\`、`\$`、四位十六进制 `\uXXXX` 和一至六位十六进制 `\u{...}`。每个 Unicode 转义必须是合法标量值；surrogate code point、超过 U+10FFFF、缺失数字/终止符及未知转义都在原 source span 诊断。raw 字面量不解释反斜杠转义。转义解码产生的美元符号始终是文本，不重新识别为插值起点。`${...}` 内按普通表达式词法处理嵌套括号、字符串和注释，不通过字符串替换或独立重解析丢失源位置。
- `add` 对实现 `ToString` 的类型可用（见 11.6 与 11.11），插入其 `toString()` 结果。

---

## 7. 可空性与 `Option`

Scoop 没有运行期 `null`。可空语义由核心库的 `Option<T>` 表达。

### 7.1 `T?` 的脱糖

类型 `T?` 是 `Option<T>` 的语法糖，编译期脱糖：

```
val a: String? = ...        // 即 Option<String>
```

注意：**`T??` 不是 `T?`**。脱糖是逐层的：`T??` = `Option<Option<T>>`，与 `Option<T>` 是不同的类型，`Some(None)` 与 `None` 可区分。这与 Kotlin（`T??` ≡ `T?`）不一致。

### 7.2 核心库定义

```
enum Option<T> {
    Some(T),
    None
}
```

`Option<T>`是普通invariant enum，享有第4.2节与第5章的全部能力（解构、穷尽性检查等）。即使`S <: T`，`Option<S>`也不是`Option<T>`的子类型；需要改变payload类型时必须显式`map`/重建。构造处存在`Option<T>`期望类型时，`Some(value)`和`None`按4.2适用于所有enum的contextual variant规则绑定该exact application，因此常见返回与赋值不需要先产生另一个`Option<S>`。`scoop.core.Option.*`由core prelude的typed default import引入；这只增加普通可见候选，不构成Option专用resolver或短名称特判。

导入 core 时，`T?`、安全调用、Elvis、`!!` 与 `as?` 使用产物中已解析的 Option 声明及 variant/payload 角色；其具体类型仍沿普通泛型 enum 路径产生。源码名称查找与这些语法角色分离，同名用户 enum 不改变 `T?` 的含义。省略初值的普通可变 Option 属性及可直接编码的 `None` 初值继续遵守9.1.3的静态初始化规则。

### 7.3 空安全运算符的语义

- `a?.b`：脱糖为 `when (a) { Some(v) -> Some(v.b); None -> None }`（结果类型为 `Option<B>`，其中 `B` 是 `b` 的类型）。
- `a?.f(args...)`：脱糖为 `when (a) { Some(v) -> Some(v.f(args...)); None -> None }`。receiver `a`先求值且只求值一次；只有运行期进入`Some`分支后才会求值显式实参、物化`vararg`、执行缺省表达式并进入callee。方法、extension与property-like `invoke`的选择均在payload静态类型上按第8章完成。
- 安全调用**不展平**结果。若`f`返回`Option<R>`，`a?.f()`的类型是`Option<Option<R>>`；若返回`Unit`，结果是`Option<Unit>`。这遵守7.1的逐层`Option`语义，不采用Kotlin nullable type的幂等合并。
- `a ?: b`：脱糖为 `when (a) { Some(v) -> v; None -> b }`。
- `a!!`：`a` 为 `Some` 时取值；为 `None` 时抛出核心库异常 `UnwrapException`。
- 空安全表达式可以出现在`while`条件中；条件脱糖产生的临时绑定与其他条件求值步骤一起在每次条件检查时重新执行，不能提升到循环外。
- 不支持 `a?.b = c` 形式的赋值：`a?.b` 的结果是临时 `Option`，对其赋值无意义。
- `null` 字面量不存在；表示"无值"使用 `None`。
- 取值的常规方式是 `when` 解构：`when (s) { Some(v) -> ...; None -> ... }`。**对 `Option<T>` 不提供智能转换**：`isSome()` 之类的判断不会收窄类型（此类收窄的语义存在隐蔽问题，暂不提供；后续如引入会单独修订本节）。

### 7.4 enum 布局与 `Option` niche 保证

对具有**天然空位（niche）**的类型 `T`，编译器必须保证 `Option<T>` 不增加额外存储：`None`编码为该空位，`Some(v)`与`v`的表示相同。空位按定义不是合法的裸`T`值；所有安全构造、compiler-generated值与声明为返回裸`T`的FFI边界都必须维持该不变量，unsafe代码破坏它后行为未定义。这是语言的固定特性，而非可选优化。适用类型：

- **引用类型**：空位是全 0 的机器字（GC 不会产生该引用值）；
- **`Ptr<U>` 与 `FunPtr<F>`**：空位是内部raw pointer carrier的全0位模式；裸`Ptr`/`FunPtr`值必须非零，null pointer只编码为对应`Option`的`None`。

因此`Option<FunPtr<F>>`在`F`满足C ABI规则时可以直接表示可空函数指针；`Option<Ptr<T>>`只有在`T`本身有本章定义的portable C pointee representation，或`T == Unit`作为opaque `void *`例外时，才能直接出现在C ABI边界。`Option`的niche不绕过C-FFI-safe classifier，尤其`Option<Ptr<其他ZST>>`仍被拒绝。`Option`包裹的引用类型与裸引用布局相同，但引用类型本身仍不能进入C ABI。对其他类型`Option<T>`带tag，具体布局不保证。

niche表示的适用范围严格限于与上述`Option<T>`同构的enum：恰有两个variant，其中一个无字段，另一个恰有一个字段，且该字段的具体类型为引用类型、`Ptr<U>`或`FunPtr<F>`；variant名称、顺序以及字段采用位置或命名形式不影响判断。除此之外的enum一律使用tagged表示，不能从其他位模式、整数范围或用户不变量推导niche。

tagged enum的表示由tag、一个可选的**pure-value共享payload区**以及若干**ref-bearing独占连续slot**组成。递归检查后完全不含managed ref的多个variant可以复用同一块共享payload空间；每个直接或间接包含managed ref的variant则必须拥有自己的连续slot，不能与其他variant重叠。共享区或独占slot都按对应variant字段的正常值布局保存其全部字段。构造tagged enum时必须先把整个值（包括共享区、所有inactive ref-bearing slot和padding）清零，再写入tag及当前variant所用的payload；复制按完整值复制。GC只需无条件检查所有ref-bearing独占slot中的固定ref位置，全0 inactive slot不形成引用，扫描不读取tag、不选择variant。`Ptr` / `FunPtr`不是managed ref；在未采用niche的tagged enum中，包含它们但不含managed ref的variant仍属于pure-value共享区。

---

## 8. 函数

函数语法整体与 Kotlin 一致：默认参数、命名参数、`vararg`、扩展函数、中缀调用、运算符重载、lambda 与尾随 lambda、函数类型 `(A, B) -> R` 等。

- 控制流分析产生目标化outcome集合：`Fallthrough`、`Return`、`Throw`、`Break(LoopId)`与`Continue(LoopId)`，而不是单个“落空”bool。顺序语句只把`Fallthrough`送入下一语句，其他outcome原样传播；`if`/穷尽`when`合并可执行分支集合。每个loop消费指向自己的`Break`为该loop的正常退出、消费自己的`Continue`为回边；循环在没有更强证明时仍加入`Fallthrough`。因此`break`/`continue`不会逃出其callable，也不会被误当作函数返回。
- 非`Unit`函数的块体在callable边界不得留下`Fallthrough`，所有可达正常完成路径必须以有值`Return`结束；`Throw`也可终止路径。无法静态证明时是编译错误。
- `try`合并try body与各catch的outcome。执行`finally`时，其`Fallthrough`恢复进入finally前的outcome；任何实际离开当前finally的`Return`/`Throw`/`Break`/`Continue`替换旧outcome，目标位于finally内部并已由内层loop/catch消费的动作不替换。这样finally的target-aware outcome规则同时决定definite return、后续语句可达性与8.7的cleanup语义。

### 8.1 函数类型、函数值与 closure

#### 8.1.1 函数类型

普通函数类型写作 `(P1, P2, ...) -> R`，挂起函数类型写作 `suspend (P1, P2, ...) -> R`。零参数函数必须写作 `() -> R`；参数名、缺省值与 `vararg` 不是函数类型身份的一部分。

- 函数类型是**引用类型**。函数值可以携带捕获环境，具有 identity，由 GC 管理；它不是一个裸代码地址。
- 函数类型的身份由挂起性、参数个数、每个参数类型和返回类型共同决定。普通与挂起函数类型之间不存在子类型关系或隐式转换。
- 同为普通函数类型或同为挂起函数类型时，参数类型逆变、返回类型协变：若 `A2 <: A1` 且 `R1 <: R2`，则 `(A1) -> R1 <: (A2) -> R2`。多参数逐项应用同一规则。
- 赋值、传参或 `as` 把函数值适配到不同但兼容的函数类型时，实现可以产生一个 forwarding adapter 函数值；因此适配后结果与来源不保证 `===`。对函数类型的 `is` / `as` 仍按上述结构化子类型关系判断，不能退化成只比较参数/返回类型完全相等。
- 函数类型不保留形参名、缺省值或 `vararg` 调用约定。经函数值调用时只能按位置提供与类型元数相同的实参；声明中的 `vararg T` 在函数类型中表现为其实际参数类型 `Array<T>`。
- 函数值以 `f(args...)` 调用；`f.invoke(args...)` 是同一操作的显式写法。若 `f` 是挂起函数类型，调用点必须处于 8.2 允许的挂起上下文。
- 函数值不定义结构相等。可以用 `===` / `!==` 观察同一已保存函数值的引用 identity，但规范不保证对同一函数重复创建 callable reference 或无捕获 lambda 时得到相同 identity。

Scoop 当前不提供 Kotlin 的 receiver function type（`A.(B) -> R`）；扩展函数的 callable reference 按 8.1.4 显式表示为普通函数类型。

#### 8.1.2 lambda 与匿名函数

lambda 写作 `{ parameters -> body }`，挂起 lambda 写作 `suspend { parameters -> body }`。匿名函数写作 `fun(parameters)[: R] { body }`，挂起匿名函数写作 `suspend fun(parameters)[: R] { body }`，其中方括号表示返回类型可省略并由 body 推导。四种形式都会产生函数值，也都可以捕获外层词法环境。

- 每个逗号分隔的lambda参数在parser层使用`LambdaParameter = PatternSyntax [':' Type]`，随后必须按4.6验证为`BindingPattern`；这保留了refutable shape的完整span与稳定语义诊断。一个成功pattern恒表示一个logical源码参数与一个函数类型参数；经过typed Scoop ABI classification后，它也只产生一个对应的`ElidedZst`/`Direct`/`Indirect`参数分类entry，tuple、struct或class component等composite pattern不会按叶binding数量flatten。物理payload仍可按4.7省略或间接传递；closure environment与挂起调用的continuation是各自独立的hidden参数，不计入源码参数。type annotation属于完整subject，而不是某个叶binding。
- 有期望函数类型时，每个lambda parameter的完整subject type可由对应的一个期望参数类型给出；期望元数按source pattern数量匹配。无期望类型时，每个显式参数（包括composite pattern）都必须为完整subject写出type annotation。单参数 lambda 在期望元数为 1 且省略参数列表时隐式声明 `it`；无参数 lambda 使用 `{ body }`。
- lambda 或匿名函数已有显式参数类型时保留该类型，并按 8.1.1 的逆变检查其能否接收期望参数；匿名函数已有显式返回类型时保留该类型，并按协变检查结果。上下文只补全未标注的部分，随后按普通函数值规则适配，不能因直接写在实参位置而禁止既有函数类型型变。
- lambda 参数支持 4.6 的解构模式。传入的单个参数值作为该pattern的subject且只处理一次；解构失败不产生运行期分支，参数静态类型必须能按该模式解构，否则是编译错误。投影、component调用、hidden temporary、异常与挂起语义均遵守4.6，不改变函数的源码/函数类型元数或该参数对应的单个typed ABI classification entry。
- lambda 的值是 body 最后一个表达式的值；有期望结果类型时，按普通函数返回规则检查 subtype 并完成隐式适配，包括值装箱、引用上行转换与函数值型变，不要求结果类型逐字相同。期望返回 `Unit` 时最后一个表达式的值被丢弃。匿名函数使用普通函数的返回规则。
- lambda 中的裸 `return` 是编译错误。Scoop 不提供 Kotlin inline lambda 的 non-local return；需要提前返回时应使用匿名函数，其 `return` 只返回该匿名函数。
- `suspend` 必须显式写在 lambda 或匿名函数上；期望类型不会把普通 lambda 静默改为挂起 lambda。创建或保存挂起函数值本身不会挂起，只有调用其 body 时才检查挂起上下文。

lambda 参与重载决议时，每个候选先提供自己的期望函数类型，再据此检查参数与 body。lambda 的返回表达式可以验证候选是否适用，但不作为“仅按 lambda 返回类型选择重载”的额外优先规则；若完成既有重载优先级比较后仍有多个候选，则调用有歧义。

#### 8.1.3 捕获与局部函数

- closure、匿名函数与局部函数只允许捕获外层 `val`、参数、接收者 `this` 及其他**不可重新绑定**的 binding；对外层 `var` 的读取或写入都是编译错误。Scoop 已有显式 `val`，因此本规则不额外引入 Java 式“effectively final”流分析：需要捕获时应先显式绑定为 `val`。
- `this` 是 3.3 定义的隐含不可变参数，与显式参数使用同一条按值捕获规则：value-type `this` 复制完整值，ref-type `this` 复制 ref value。两者都不保留调用方 receiver binding/place；复制 ref value 不会复制其指向的对象。
- 上述“外层 `var`”特指 callable body 中作为自由变量引用的外层**词法局部 binding**。顶层/object 属性按其 global/singleton storage 访问；class 可变属性则通过被捕获的 `this` 或其他显式引用对象访问，二者都不会把局部 `var` 隐式提升成 cell。
- capture 一律按值发生在函数值创建时：引用类型复制引用，值类型复制完整值并直接内联保存于 closure environment，不为 captured value type 隐式生成 identity-bearing box、shared cell 或 `Any` 装箱。
- captured `val` 本身不能重新绑定；若其值是带可变状态的引用对象，仍可按该对象公开的普通成员规则修改 referent。需要多个 closure 共享可变状态时，程序必须显式捕获这样的引用对象；编译器不把局部 `var` 隐式提升成引用对象。
- capture 会把复制进 environment 的引用和值延长到 closure 不再可达；其中嵌套的 managed 引用由 GC 按字段类型递归扫描。
- 没有捕获的实现允许复用静态单例；实现也可以把不逃逸的 closure 消除或栈上展开，但这些优化不得改变 identity 被观察时的结果。

局部命名函数与普通函数使用相同的声明语法，并按上述规则捕获外层不可变 binding。局部函数名在其自身 body 及声明之后的词法作用域可见，因此允许直接递归；声明之前不可见，互相递归不能依赖后声明函数的前向可见性。局部函数可以直接调用，也可以通过 `::name` 取得函数值。

局部 generic 函数的直接调用按 3.2 单态化。取得其 callable reference 时，所有类型参数必须能由期望函数类型唯一确定；否则是编译错误，不存在“仍然 generic 的函数值”。

本版本不提供隐式 reference capture、`move` capture 或 capture list。以后增加新的 capture mode 时必须使用显式语法，并单独规定 lifetime、identity、并发和成本模型；不得静默放宽本节的 `var` 禁令。

#### 8.1.4 函数声明引用表达式（callable reference）

`::` 引入**函数声明引用表达式**。这是中性的源码语法：`::name` 本身既不表示 managed closure，也不表示 native code pointer。parser 只保留引用形态与名称；语义分析根据上下文的期望类型类别一次性决定其结果：

- 期望类型是普通/挂起函数类型时，表达式创建本节定义的 managed 函数值；
- 没有期望类型时，只尝试推导 managed 函数类型，绝不自动推导为 `FunPtr`；
- 期望类型是 `FunPtr<F>` 时，只有语法形态为无 receiver 的 `::name` 才进入 8.1.5、13.10 的 native-address contextual resolution；该路径不先创建 managed 函数值。

在 managed 函数类型上下文或无期望类型的推导上下文中，支持以下形式：

- `::name`：引用可见的顶层函数或局部函数；
- `receiver::member`：创建绑定接收者的成员函数引用，`receiver` 在创建时求值且只求值一次；
- `::extension`：创建未绑定扩展函数引用，其函数类型把扩展接收者作为第一个普通参数；`receiver::extension` 则创建绑定形式。

若目标是 virtual / interface 成员，绑定引用在每次调用时仍按已保存接收者做动态分派，不能在创建时固定为当时的具体实现。managed 分支中的重载目标由期望函数类型与普通重载规则共同确定；没有期望类型时，只有唯一的非 generic 候选才能自行确定函数类型，所得结果仍是 managed 函数值。

`receiver::member` 的 receiver 是创建点的普通表达式；其中读取某个局部 `var` 会立即取得当前值，随后 closure 不可变地保存该 receiver。这是源码显式的创建时快照，不是 callable body 对该 `var` 的自由变量捕获。

Scoop 当前不提供 `Type::member` 的未绑定成员引用或构造函数引用。函数声明名出现在普通值位置时不会隐式变成函数值，必须显式写 `::`；直接写 `name(args...)` 仍走命名调用与重载决议。

#### 8.1.5 与 `FunPtr` 的边界

managed 函数类型与 `FunPtr<F>`（13.10）是不同类别的值：前者是可捕获、可经 GC 移动的引用对象，后者是 GC-free 的原生代码指针值。二者没有一般性的子类型关系、转换或相同调用 ABI。

唯一共享的是 8.1.4 的 `::name` **源码语法**。在上下文期望类型明确为 `FunPtr<F>` 时，满足约束的顶层命名函数引用直接解析为原生 callback 地址；这是对函数声明引用表达式的 native-address contextual resolution，不是从 managed 函数值到 `FunPtr` 的值转换。该路径不创建 managed closure，也不允许把 lambda、匿名函数、局部函数、绑定引用或已存在的函数值转换为 `FunPtr`。

需要让 native代码在稍后或 foreign thread中调用 managed closure时，必须使用 14.3 的 callback registration协议。该协议导出的是“静态 C trampoline + GC-free opaque context/token”的组合，closure本身仍是 managed函数值并由 runtime保活；它不是到 `FunPtr` 的隐式或显式值转换，也不放宽本节规则。

```
@NoGC
fun increment(value: Int): Int = value + 1

fun references() {
    val managed: (Int) -> Int = ::increment
    val native: FunPtr<(Int) -> Int> = ::increment
    val inferred = ::increment                    // 无期望类型：managed (Int) -> Int
    val invalid: FunPtr<(Int) -> Int> = inferred // 编译错误：不能事后拆出 native 地址
}
```

### 8.2 `suspend` 函数

- `suspend` 修饰的函数是协程挂起函数，只能在另一个 `suspend` 函数或编译器认可的协程构建器内调用。`main` 本身必须是普通函数；从普通代码启动挂起计算使用 11.9 的 `startCoroutine` 或标准库构建器。
- 下列**声明自身拥有的运行期初始化上下文**都是非挂起上下文，不得包含挂起调用：顶层 `val` / `var` 的 initializer 与 delegate 表达式；`object` / `companion object` 的属性 initializer、delegate 表达式、`init` 块及基类/接口委托初始化；class 的属性 initializer、delegate 表达式、`init` 块、主/次构造函数体及构造委托；struct secondary constructor body / delegation；struct/enum 变体及构造函数的缺省表达式。全局初始化入口必须在进入 `main` 前同步完成；单例或实例初始化必须在对象可用前同步完成；它们都不能返回 `Suspended`或保存“尚未完成的初始化”。class/struct初始化中的`this`按9.1.1作为受限initializing receiver，只能直接访问已经初始化的字段，不能作为普通值发布、捕获或用于任何方法分派；该限制从结构上排除半初始化对象逃逸，而不是依赖whole-program escape analysis。
- **求值归属按词法位置确定，而不是按最外层构造语法确定**：在 suspend 函数中显式写出的调用实参仍处于调用者的挂起上下文，因此 `C(awaitValue())` 合法——`awaitValue()` 先完成，随后普通构造过程同步执行；构造函数定义处的缺省表达式和构造体内部则仍为非挂起上下文。把 suspend lambda / `SuspendTask` 对象保存进字段也不等于执行它，其函数体在以后实际调用时按自身的挂起性检查。
- 属性没有隐式挂起能力：普通 getter / setter、计算属性，以及委托属性的 `getValue` / `setValue` 协议必须是非挂起 callable；即使属性读取发生在 suspend 函数中，也不能通过普通属性访问暗中挂起。未来若引入 suspend property，必须另行定义语法、类型与调用规则。
- `const val` 的约束更强：它没有运行期初始化过程，只允许 9.1.2 定义的编译期常量表达式，因此不允许普通或挂起函数调用。9.1.2 封闭列出的整数表示 intrinsic call 是编译器常量运算，不属于这里的普通函数/方法调用。
- 挂起性是 callable 签名的一部分：override / interface 实现的挂起性必须与被覆写声明完全一致；`suspend (A) -> R` 与普通函数类型 `(A) -> R` 不兼容。挂起性不作为同名声明的重载区分项，参数列表相同而只相差 `suspend` 的两个函数是重复声明。
- 调用挂起函数可能在当前调用栈内立即产生 `R`，也可能保存当前计算并返回到协程启动者，随后经 `Continuation` 恢复。无论采用哪条路径，源码都只观察到一次普通的 `R` 结果或一次在该调用点抛出的异常；挂起本身不是返回、异常或 `finally` 的退出原因。
- 调用点之前已经完成的实参和子表达式只求值一次；恢复后从调用点之后继续，源码从左到右求值顺序不变。跨挂起点仍存活的局部变量、参数及待执行的控制转移必须被保留。
- `try`/`catch`/`finally`的语义跨挂起点保持不变：恢复失败等价于在原挂起调用点`throw`；仅仅挂起不会执行`finally`；跨挂起保存的待执行动作包括正常继续、return、throw以及8.7的break/continue。finally正常结束后恢复原动作；只有实际从当前finally向外离开的新控制转移才覆盖原动作，finally内部被catch或只退出其内层loop的转移不覆盖。每个被退出的finally仍恰好执行一次。
- `Continuation` 是单次完成协议：一个挂起点只能由 `resume` 或 `resumeWithException` 中的一个成功完成一次；编译器生成的 continuation 对重复完成抛出 `IllegalStateException`。本规范不定义协程取消；放弃且永不恢复一个 continuation 不会隐式执行 `finally`。
- 现阶段不支持 suspend FFI：挂起函数不得带 `@Extern`，其声明引用也不得在 `FunPtr` 上下文中解析为原生地址；M10 的 hidden continuation ABI 只用于编译器生成的 Scoop 托管调用，不是任何 FFI ABI。具体约束见 13.4、13.10 与 14.2。
- 具体的协程构建器（`launch`、`async` 等）、调度器与取消策略属于标准库，不在最小核心库范围内。最小核心库只提供 11.9 的启动、挂起与恢复原语。

### 8.3 Task-local Context（M27）

Context 是属于逻辑任务的动态绑定。源码保留 Kotlin-like 的参数声明，但在被调用声明的入口按**精确静态类型**查找；调用点不做隐式实参解析。完整实施设计见 [M27 设计](../milestone27/DESIGN.md)，运行时与编译器职责见 runtime spec 第 9 章、impl spec 2.16。

```scoop
interface Logger {
    fun write(message: String)
}

context(logger: Logger)
fun report(message: String) {
    logger.write(message)
}

fun run(logger: Logger) {
    context(logger) {
        report("ready")
    }
}
```

#### 8.3.1 声明与名称

`context(name: T, _: U)` 是至多一个、非空的声明前缀，位于该声明的 annotation/modifier 之前；允许换行，不跨显式分号。每项必须有名称或 `_` 及类型，不接受默认值、`vararg` 或参数修饰符。它可以修饰现有合法的顶层、成员、扩展及局部命名函数，包括 ordinary、suspend、generic、abstract/interface 方法；也可以修饰没有存储、initializer 或 delegate 的计算/abstract property。property-level list 同时适用于其 getter 和已有 setter，不能分别给 accessor 添加另一份 list。

不能用此前缀修饰名义类型、constructor、`init`、release block、lambda、匿名函数、函数类型、stored/delegated property，或 `@Extern`、`@Intrinsic`、`@NoGC` 声明。既有的 generic virtual、interface method-level generic、属性挂起性等限制不因 Context 放宽。

- 具名项在该 list、普通值参数与实际 setter 参数的名称空间内必须唯一；`_` 可重复，但不建立名称。名称只在函数或 accessor 正文内可见，作为普通不可重新绑定的 local；不在 annotation、类型子句或普通参数默认表达式中引入名称。默认表达式仍按 8.5.3 在 caller 求值。
- 每个 list 的 key 必须不同。透明 alias 展开后相同也算重复；泛型定义处检查已经相同的类型表达式，完整替换后检查新合并的 key。后一检查适用于每个具体 callable/owner application，包括无正文的 abstract/interface contract；错误报告在触发该 application 的位置。
- requirement 的类型与 binding 表达式的静态类型都必须是非空 managed ref：包括 class/interface/object、String、数组、`Any` 及 ordinary/suspend 函数值。值类型、`Option<T>`、`Ptr`、`FunPtr` 不能直接作为 key。先按普通规则装箱或转成某个引用类型后，可按该引用类型绑定，Context 本身不插入装箱或上转型。
- 泛型类型表达式须在定义处即可确定为 ref；例如 `Array<T>`、函数类型、`T : ref` 或具有 class 上界的 T。无约束 T 或只有 interface 上界的 T 不能保证这一点，因为 value type 也能实现 interface。13.9 的 bound 组合限制保持。
- public/protected contract 中的 context type 遵守 9.1.5 的签名可见性；名称不是 key，不影响类型身份。

#### 8.3.2 精确 key 与入口取得

key 就是 alias 展开后的 canonical exact static type；包含完整 nominal/function type 实参及函数挂起性，不读取对象的运行期 TypeDescriptor，也不做 subtype、variance 或 interface 搜索。不同函数类型即使可按 8.1.1 转换，仍是不同 key。同型多角色需要不同 nominal wrapper，透明 alias 不创建新 key。

```scoop
interface Service
class ServiceImpl : Service

context(service: Service)
fun useService() {}

fun needsServiceBinding(impl: ServiceImpl) {
    context(impl) { useService() }       // ServiceImpl does not supply Service.
}

fun withServiceBinding(impl: ServiceImpl) {
    val service: Service = impl
    context(service) { useService() }    // Binds the exact Service key.
}
```

不同 key 的 binding 可以同时存在；安装 ServiceImpl 不遮蔽已有的 Service。上例 needsServiceBinding 只在外层也没有 Service 时抛缺失异常。

每次进入一个 source implementation body 时，按 list 顺序查找一次，先全部取得再执行用户正文。具名结果保存为普通 immutable local，匿名项只检查存在；未使用项也必须取得。第一个缺失项抛 11.7 的 `MissingContextException`，消息标识声明、参数名或从 1 开始的匿名序号、canonical type。函数内的 catch 不包围这个入口过程，caller 可以捕获该异常。

该次 activation 取得的 local 不随内部同型重绑定而变化，新调用进入时才取得新绑定。suspend body 只在 initial entry 取得，跨挂起保留 local；resume 不重取。无正文声明只有 contract；this-adjust thunk、函数引用 adapter 等进入同一实现，不另执行一次 lookup。body 内的普通 closure 若引用该参数，只按 8.1 捕获这个 local。

#### 8.3.3 结构化绑定表达式

`context(value) { body }` 是 compiler-known 表达式，body 是词法 block，不是 lambda，不产生 callable 或 suspension boundary。无 receiver 的这一完整形态优先于普通调用加 trailing lambda；`obj.context(...) { ... }` 仍是普通调用。括号与 block 之间按既有 control-flow header 规则处理换行和分号。一次只接收一个 value，多项用嵌套 scope。

1. value 在外层 Context 中按普通规则完整求值一次，以包含 smart cast 结果的静态类型确定 key。body 的 requirement 或整个表达式的 expected result type 不反向决定 value 的类型；expected result type 只传给 body。
2. value 正常完成后安装 binding；安装失败不得提交部分状态。body 在新 binding 下执行一次，其正常结果是整个表达式的结果。
3. 同 key 的内层 binding 覆盖外层，退出后恢复原值或 unbound。fallthrough、return、break、continue、throw 只有真正离开该 scope 才恢复，按词法嵌套恰好一次，与 8.7 的 finally/catch cleanup 使用同一顺序。
4. 挂起不是退出，不恢复 binding；body 继承所在 callable 的挂起性。恢复后的真正退出仍按上一项处理。scope 内的 finally 在该 binding 下执行；位于 scope 外的 finally 在恢复后的 Context 下执行。

Context 安装可能分配/GC，入口缺失可能构造并抛异常；它们不能用于 const、release-safe 或 NoGC 正文。lookup 成功本身不调用用户代码。

#### 8.3.4 声明契约与调用

ordered context list 进入导出声明及 override/accessor contract。完成宿主与 callable 类型替换后，override/interface implementation 的数量、顺序与 exact type 必须全部相同，名称可以不同。继承同一普通签名的多个冲突 context contract 是该 owner application 的编译错误，不能任选一个。contextual property obligation 只能由具有相同 contract 的计算/abstract property 承接，stored/delegated/constructor property 的隐式 accessor 不能满足它。

context list 不参与 overload applicability、MSC、泛型推断、普通或 suspend 函数类型、mangle、dispatch slot identity 或机器实参。只差 context list 的声明仍是重复声明。未声明 requirement 的中间函数可以调用 contextual function，不推导或传播静态 effect row；binding 缺失只在实际进入有 requirement 的 body 时抛出。函数引用创建时不查找、不捕获 Context，其类型不增加 requirement，实际调用才进入目标 prologue。普通实参、receiver 和 caller-side default 的求值仍先于这个 prologue。

#### 8.3.5 任务、协程与回调传播

- 普通调用、virtual/interface/closure 调用与同一协程的 direct suspend 调用共享当前 TaskContext。普通 closure 不因创建于 binding scope 而隐式捕获整个 Context。
- `startCoroutine` 在普通实参求值后、执行 task body 前，从当前有效 binding 建立独立 child TaskContext；task 与最终 completion 通知均在 child 下执行。parent、child 以后的重绑定互不影响，已绑定对象的 identity 仍共享。两种 overload 的行为相同（11.9）。
- 真正驱动 resume 时进入 frame 所属 TaskContext，在再次挂起、完成或失败后恢复 resumer 原 Context。只投递同步完成 payload 而未驱动 frame 的 resume 路径不切换 Context；完成权竞争仍遵守 8.2。
- 同步 outbound FFI 保持逻辑任务。`foreignCallback` 注册时捕获有效 binding 的快照，每次 invocation 从快照建立独立 TaskContext；包括同步重入及 Reusable 并发调用（14.3）。callback 的缺失异常遵守原有 catch/status 边界。
- 所有 eager initializer 与 main 共享根任务，但 binding 仍只能由实际执行的词法 scope 提供。无 binding 的 initializer 调用 contextual function 同样抛 `MissingContextException`。
- Context 不提供取消、调度、线程同步或资源释放。永不恢复的 continuation 不隐式执行 scope cleanup；相关对象不可达后按普通 GC 回收。

### 8.4 `inline` / `crossinline` / `noinline` / `reified`

这些关键字**仅出于 Kotlin 兼容性保留，无实际作用**：

- Scoop 泛型是单态化的，类型信息在每个实例化处天然可用，不需要 `reified` 来保留类型实参；
- 编译器自行决定内联策略，`inline` / `noinline` / `crossinline` 被接受但可忽略（允许作为提示，但不保证语义）；
- 不依赖语义内联的 Kotlin 代码可以保留这些关键字而无需修改；依赖 inline lambda non-local return 的代码不兼容，必须按 8.1.2 改为匿名函数或重写控制流。

### 8.5 调用实参、默认参数与 `vararg`

#### 8.5.1 参数声明

- 普通value parameter可以是必需参数、带缺省表达式的默认参数，或`vararg`参数；一个参数列表至多有一个`vararg`。class/struct primary constructor、class/struct secondary constructor及构造函数式enum variant字段使用同一规则；`vararg val x: T`在primary构造结果中声明的属性/字段类型是`Array<T>`。secondary参数不能写`val`/`var`，只作为constructor local存在。
- `vararg x: T`中的`T`是**元素类型**；在函数体、构造体及函数签名的实际参数类型中，`x`的类型是`Array<T>`。因此只相差`vararg x: T`与普通`x: Array<T>`的两个声明具有相同参数类型，不能据此形成重载。函数类型同样只保留实际参数类型`Array<T>`，不保留`vararg`调用约定（见8.1.1）。
- `vararg`可以有显式缺省表达式，其类型必须是`Array<T>`；没有显式缺省且调用处没有提供任何vararg元素时，产生一个新的空`Array<T>`。
- 参数名、默认值及是否具有默认值不属于函数签名，不能仅靠它们区分重载；但参数名和`vararg`调用约定属于源码调用接口，必须保留到调用决议完成。
- 默认参数值表达式是callable源码接口的一部分，并在**定义处完成解析**：名称解析、overload选择、类型检查、可见性检查、挂起性检查及所引用实体身份都使用声明方上下文。它可以使用当前callable的类型参数、源码存在的隐含receiver以及此前声明的参数，不能引用自身或后声明的参数。constructor default可以使用host type parameter与此前constructor参数，但constructor在source interface中没有`this` receiver：class对象尚未分配，struct值尚未形成，不能从default访问任何实例field。
- default直接绑定的每个实体都必须在该callable的**全部合法调用位置**可访问，即callable调用域必须是该实体可访问域的子集。对导出的`public` callable，这意味着只能引用下游可见的`public`或经`public import` re-export的实体，不能引用声明Cone的`private`/`internal`实现；`internal`、private/member及local callable可以引用覆盖各自完整调用域的实体。该检查适用于已选择的callable/overload、constructor、property accessor、operator目标及nominal type，并在const folding与desugaring之前执行；不能靠编译期折叠绕过可见性。
- default正文自己引入的local function随该正文一起展开，其合法直接调用不要求调用方重新lookup这一词法声明。callee必须对应正文中实际声明的同一个local function；正文外的private/internal callable仍按完整调用域检查，不能仅因它也是local function或同名函数而豁免。
- `abstract`函数和interface方法可以声明默认值。`override`声明不得重新声明默认值；它按静态可见的override关系继承唯一的默认来源。若一个override位置从互不相关的父声明继承到无法唯一确定的默认来源，必须在类型定义处诊断，不能任选一个表达式。override的`vararg`形态必须与被覆写声明一致。
- override参数名不参与签名匹配；命名调用使用调用点静态接收者所见声明的参数名，动态分派只选择函数体，不重新映射实参或替换默认来源。
- 默认值之间的依赖不受声明顺序、文件顺序或function/constructor/variant类别影响。仅当已选定调用实际省略某个参数时，才建立到该参数默认值的展开依赖；显式提供参数的普通调用不建立该边。源码默认值展开依赖必须无环，自环或跨声明环在定义处报编译错误，即使外部尚未调用该声明；按参数位置区分节点，不能因同一个callable出现在链中便误判成环。普通callee正文中的递归调用不属于该展开依赖图。

#### 8.5.2 调用处实参映射

- 调用实参有位置实参`expr`、命名实参`name = expr`、vararg展开实参`*expr`和命名vararg实参`name = expr` / `name = *expr`。尾随lambda是语法上位于圆括号之后的最后一个显式实参，进入同一映射与求值过程。
- 实参到形参的映射针对每个overload候选独立完成。未知参数名、重复绑定、spread映射到非`vararg`、遗漏必需参数或多余实参都会使该候选不可应用；不能先按任意候选重排一份公共参数表。
- 在尚未进入“仅命名实参”尾部时，第`i`个实参可以是映射到第`i`个形参的位置实参，也可以写出该形参本来的名字。某个命名实参一旦跳过声明位置、或通过名字绑定到其他位置，后续实参必须全部使用名字。默认参数不能在一串位置实参中间被隐式跳过。
- 位置实参到达`vararg`位置后，后续未命名的普通/spread实参都属于该`vararg`；若其后还有其他形参，这些形参只能用命名实参提供。
- 命名实参直接映射到同名形参。对`vararg x: T`，`x = array`与`x = *array`都提供完整的`Array<T>`参数值，不能再为`x`提供其他元素；未命名位置中的多个普通元素和多个spread则可以混合。
- 函数值调用不具有声明参数名、默认值或`vararg`调用约定，只接受与函数类型元数完全相同的位置实参；需要保留声明侧便利语义时必须显式创建适配lambda（见8.1.1）。

#### 8.5.3 求值顺序与调用处实例化

一次已经选定目标的调用严格执行：

1. 若有显式receiver，先求值receiver且只求值一次；
2. 所有**显式实参表达式**按它们在调用源码中出现的顺序从左到右求值且各一次，不受命名映射后的形参顺序影响；尾随lambda位于圆括号内实参之后；
3. 按形参声明顺序物化完整参数值：复用已求值的显式结果、构造vararg数组，并对缺失参数在其声明位置实例化和求值缺省表达式；显式缺省表达式只在对应参数确实缺失时执行；
4. 按形参声明顺序把完整值传给callee，再进入函数体。

因此所有显式实参都先于任何缺省表达式求值；多个实际使用的缺省表达式按参数声明顺序求值。命名实参只改变“值属于哪个参数”，不改变其源码求值顺序。例如`f(y = n(), x = m())`先执行`n()`再执行`m()`，随后以`x = m()`的结果、`y = n()`的结果调用`f`；若只写`f(y = n())`且`x`有默认值，则先执行`n()`，再执行`x`的缺省表达式。

- 调用目标与完整type arguments确定后，缺省表达式在每个发生缺省的调用处以已经绑定的typed template进行hygienic展开并求值，不是在声明时计算或缓存；调用方显式提供参数时，对应缺省表达式完全不展开。展开结果进入普通表达式编译流程，但调用方不会对template中的名称、extension或overload重新做决议，因而调用方import、同名局部声明或后来新增的overload不能改变其含义。
- 这与单态化一致：缺省表达式可能依赖类型参数（如`fun <T> f(x: T, y: Array<T> = [])`），只有在调用目标与全部类型实参确定后才能生成具体代码。缺省表达式自身不能作为“猜出”尚未确定类型实参的来源；若显式实参、receiver、允许的期望类型及声明约束仍不能唯一确定实参，调用不成立。
- 缺省表达式可以引用此前参数，因为完整参数值按声明顺序物化；此前参数无论来自显式实参、vararg构造还是另一个缺省表达式都已经可用。
- 导出callable的default template只引用已经导出或re-export的typed实体，不携带private/internal hidden dependency closure。修改参数名、default body、绑定目标或vararg形态会改变其`.slib`接口metadata并使下游重新编译；被引用实体的可见域收窄到不再覆盖callable调用域时，声明方必须重新编译并报错。
- `suspend`函数的缺省表达式可以包含挂起调用；它在挂起调用点实例化，此前已经求值的receiver与显式实参必须按8.2保存。普通函数及所有构造器/variant的缺省表达式不能包含挂起调用；suspend caller显式写出的普通构造实参仍按8.2合法。
- 所有缺省表达式采用相同的实例化语义，不因表达式是literal、普通调用、构造、intrinsic或其他kind而改变。实例化后的每个表达式同时具有声明节点的**定义来源**和发生缺省的call expression的**求值来源**；前者用于定义处诊断与源码归属，后者供任何需要观察求值位置的语言设施使用，二者都必须存在且不能互相回退或覆盖。若该call expression本身来自另一个缺省表达式实例，其已有求值来源继续传入内层实例，因此无需识别具体表达式内容即可得到最外层实际调用来源。

#### 8.5.4 `vararg`值

- 普通位置元素`e`为`vararg x: T`贡献一个满足`type(e) <: T`的元素；spread表达式必须具有exact `Array<T>`类型。`Array<S>`即使`S <: T`也不能作为该spread，调用方需要显式逐元素构造/转换目标数组。
- 未命名vararg元素（包括spread）先各自按8.5.3求值，随后在形参物化阶段构造一个新的`Array<T>`；spread按元素顺序复制，可以与普通元素混合。即使唯一输入是`*array`也不与来源数组共享identity。
- 命名形式`x = array`或`x = *array`直接提供完整exact `Array<T>`值，不额外复制；它与“若干位置元素组成新数组”是两种不同的源码调用形态。
- 没有元素且没有显式默认值时构造新的空数组；有显式默认值时按普通缺省表达式求值并直接使用其结果。实现可以在identity不可观察时消除分配或复制，但不能改变`===`、异常、挂起点或求值顺序可观察到的结果。

### 8.6 调用决议与泛型约束求解

- 调用决议先按词法/成员/import优先级建立候选层；无显式receiver与extension scope的完整层序见12.4.3。每层内继续按9.3.4的function-like/property-like c-level分区；对每个最终分区完成调用形态预过滤与候选各自的类型可应用性检查，再只在第一个含有至少一个可应用候选的分区中求最具体候选。显式类型实参数量、命名参数是否存在、spread/receiver形态等不依赖表达式类型的检查属于预过滤；某个更高层仅有同名但形态、类型或bound不适用的声明时，不得无条件遮蔽合法的下一层候选。当前Cone、上游`.slib`、exact/star import与core prelude只决定候选来自哪一层，不改变后续算法。
- 每个候选拥有独立的实参映射、fresh inference variables和constraint system。receiver、非postponed实参、显式fixed/`_`类型实参、函数/nominal invariant relation及upper bound共同产生等式与子类型约束；generic owner参数、callable自身参数和待推断变量保持不同identity，不能压平成一组后再按长度或span反推。
- 选择变量的唯一解前，等式与上下界必须相互传播：由`L <: V`和`V <: U`继续按同一类型关系约简`L <: U`，声明的class/interface bound也参与该过程。例如`R : Reader<T>`与实参确定的`Holder<Int> <: R`通过`Holder<Int>`的实际`Reader<Int>`父类型确定`T = Int`。该过程只使用已有约束；单独的声明bound不能成为补齐未知变量的猜测值，不能先选一个临时解再把它当作新的已知事实。
- 依赖候选期望类型的lambda、匿名函数、callable reference、裸enum variant（包括`None`）、空数组、整数字面量和嵌套generic构造作为postponed argument处理。求解器先用其余约束推进固定点，再用候选给出的完整期望类型检查postponed argument；lambda body结果只决定候选是否适用，不提供额外的“按lambda返回类型优先”规则。
- 候选声明、receiver 或显式类型实参已经确定的完整形参类型可直接作为实参表达式的上下文，包括结构化表达式的分支及参数已标注的 lambda 正文；不必先丢弃已有上下文再尝试合成类型。尚未确定的形参仍按上述固定点处理。
- 后续实参同样可以提供上述类型上下文，例如 `pack(*[], 42)` 的空 spread 可由另一元素确定为 `Array<Int>`。命名整数组、普通参数、构造参数和成员调用遵守同一规则；没有可用约束的空数组、裸变体或函数引用仍须报错。推断固定点没有进展时才尝试可独立确定类型的延期实参，其中整数字面量按既有默认阶梯确定类型；具有完整参数和结果标注的匿名函数也可提供自身类型。失败尝试不提交表达式、诊断或局部状态，也不妨碍其他延期实参继续提供约束；运行期求值顺序始终遵守 8.5.3。
- constraint system必须同时满足kind/class/interface bound、函数类型型变、nominal application逐项相等、普通subtyping及装箱规则。一个候选只有在所有实例化参数得到唯一、可表达且满足bound的concrete解，并且全部显式与postponed实参都可赋给对应参数时才可应用；不得用`Any`、bound、默认false或任意首个类型补齐无解/多解变量。
- 外层期望类型可以在唯一callable目标已经不依赖返回类型选择时帮助固定只出现在返回结果中的类型参数，也可以为generic nominal构造提供宿主application；它不能使两个仅靠结果类型才能区分的overload变得合法或在多个候选间充当MSC比较项。普通函数签名仍不含返回类型，返回类型不同不能单独形成重载。
- 最具体候选使用独立于本次实际推断结果的pairwise forwarding constraint system：比较`A`是否至少与`B`同样具体时，把`A`的声明参数替换为fresh variables，再检查其每个由调用提供的参数（extension receiver也算）是否可按同一普通subtyping/nominal-invariance关系转发给`B`的对应参数，并同时加入双方声明bound。不能比较两边已经为当前调用猜出的concrete type arguments。
- 若唯一候选能转发给所有其他候选而反向不成立，则它胜出；互相可转发或互相都不能转发时，依次应用规范已有的附加规则：非参数化候选优先；M17起，在互相可转发的集合中实际使用更少默认值者优先，仍相同时无`vararg`者优先。命名/位置写法本身不参与优先级。仍不唯一即为歧义。
- 无外层expected type的分支、数组元素或其他LUB计算只有在同一generic template的全部类型实参逐项相等时才能保留该application；`G<A>`与`G<B>`不会合成为`G<LUB(A, B)>`。否则沿普通共同父class/interface/`Any`规则寻找上界，必要时装箱整个value。expected type存在时可让各分支直接按同一个exact target构造/检查，但不能把已经形成的不同application隐式转换到该target。多个互不可比较的nominal共同上界仍不产生交叉类型。
- 整数字面量按11.2持有candidate-local可表示type集合；assignment/return/argument/call-or-operator receiver等位置的exact expected integer type可提交一个可表示literal。literal receiver对integer representation intrinsic及11.8的四个core range member都适用：只枚举八个canonical integer owner，再在假设owner上执行普通core-member决议，不能在查找`and`/`shl`/`rangeTo`/`until`前抢先默认。该规则不枚举任意用户或extension member，也不扩展为普通类型的全局反向推断。fixed point后仍未被约束的无后缀literal按`Int` → `Long`、`u/U` literal按`UInt` → `ULong`选择第一个可表示其值的类型。普通MSC及上述附加规则仍并列时，默认阶梯产生的exact commit优于同族其他fit；多个非默认fit互不支配。literal fit不建立整数type间的subtyping/coercion，失败probe不得泄漏已提交type。
- 无匹配与歧义都是HIR编译错误。诊断必须列出所在候选层、每个相关候选的完整签名及其失败原因（形态映射、类型实参数量、未解变量、bound、实参类型或MSC并列），不能只报告“unknown function”或由下游根据缺失callee猜测失败原因。

### 8.7 循环控制

本规范在M22规定的循环控制子集是`while`、`for`与不带标签的`break`/`continue` jump statement；成功及失败语法位置由第5章新增段落与本节共同规定。`do-while`、带标签控制流及一般jump expression仍属于2.1所列完整语言的后续子集。

- `break`退出当前callable内词法最内层循环，`continue`进入其下一轮；函数、匿名函数、lambda和局部函数各自建立控制边界，不能跳到外层callable的循环。循环外使用是编译错误；
- `break`/`continue`不产生正常结果并终止当前路径；该路径不参与第5章的结果类型合并，语义效果等同于bottom，但jump自身不是`Nothing`类型的表达式。循环正常结束的语句结果为`Unit`；没有更强证明时仍按可能落空处理；
- while的`continue`重新进入完整condition求值入口，包括该condition产生的temporary、safe-call/default setup和挂起调用；for的`continue`进入下一次`next()`；
- 控制转移离开scope时，按从内到外执行所有应执行的catch结束动作和`finally`。finally正常完成后恢复原动作；只有实际离开当前finally的return/throw/break/continue才覆盖原动作，内部被处理的转移不覆盖。新动作继续执行尚未经过的外层cleanup，不能再次进入当前finally。挂起不是退出，不执行finally。

`for (pattern in source)`严格遵守11.8的typed迭代协议；`source`、`iterator()`各求值一次，每轮只调用一次`next()`。每个`Some`建立新的只读binding scope，因此closure捕获对应轮次的值，而不是一个跨轮次覆写的隐藏mutable slot。pattern必须满足4.6的irrefutable binding规则。

---

## 9. 其他语法要点

### 9.1 类与对象

- `class` / `abstract class` / `interface` / `object` / `companion object`均为引用类型。`object`/companion的singleton语义、初始化和generic边界由9.1.3明确规定，不继承Kotlin/JVM的class-initializer ABI。
- 类可以实现 interface，可以继承一个类；struct/enum 不能被继承，也不能继承类。
- 类与成员方法默认均为 `final`。只有 `open` / `abstract` 类可以被继承；类可被继承不代表其方法自动可覆写，普通基类方法必须显式声明为 `open fun` 才能首次被覆写。
- `abstract fun` 隐含 `open` 且没有函数体，只能声明在 `abstract class` 或interface中。interface function有body时是default implementation。覆写class/interface member必须写`override`；`override`默认继续open，可用`final override`终止。
- final 方法不得被覆写。静态接收者上已知的 final 方法调用使用直接分派；open / abstract 类方法调用使用 vtable 分派，interface 方法调用使用 itable 分派。final override 仍替换继承来的 vtable 槽，以保证经基类引用调用时到达该实现。
- `sealed`：`sealed class` / `sealed interface` 保留（引用类型的受限继承）；值类型的等价物直接使用 `enum`。

#### 9.1.1 对象与属性初始化

**property实体与源码形态：**

- property是“显式声明type + getter + 可选setter”的逻辑实体，不等同于field。所有非局部property必须写显式type；同一owner内property名称唯一，不能按type、mutability或accessor重载。同一Cone的package namespace内public/internal top-level property名称唯一；split package中不同origin Cone的同名实体保持不同identity，并在同时导入时按12.4诊断。file-private property的owner包含source file，不同文件可以同名。function和property可以同名并按8.6/9.3.4分区；
- `val/var p: T = expr`是stored property并一定拥有backing storage；accessor语法可以全部省略，也可以自定义getter/setter。`val p: T get() ...`是无field的computed read-only property；interface以外的computed `var`必须同时提供getter和setter。`abstract val/var`把全部required accessor正规化为abstract slot；interface中每个required accessor独立分类，有body为default、未提供body为abstract，因而允许getter/setter混合default/abstract。`val/var p: T by expr`是delegated property，见9.2；
- Scoop永久不支持`lateinit`。唯一可以省略initializer的stored形态是无custom accessor的`var p: Option<T>`/`var p: T?`，其语义等价于在同一初始化位置写`= None`。`val p: T?`、`var p: T`仍须提供initializer或合法computed/abstract body。若需要optional backing field与custom accessor，必须显式写`= None`；generic type parameter不能因某个实例恰为Option而改变声明representation；
- getter无显式参数并返回property type；setter恰有一个不可重新绑定的参数（缺省名`value`）并返回Unit。`val`不能有setter；`var`可只customize一个stored accessor。`field`只在stored accessor的直接body内表示typed backing-field place，computed/abstract/delegated property及nested lambda/local function中不可用；
- primary-constructor property只有implicit field accessor。class/object的stored/delegated property进入common initialization sequence；struct/enum不能拥有stored/delegated member state，但可以声明computed `val`；interface不能有field、initializer、delegate或`init`；
- class property与method使用相同的final/open/abstract/override规则。property override要求名称与type exact相同；`var`可以override `val`并增加setter，`val`不能override `var`。stored/computed/delegated表示都可实现property contract。value type不能实现要求setter的`var`contract；
- property read要求getter可访问，write要求同一property的setter可访问。选中logical property后不能因缺少/不可见setter退回较低候选。receiver与右值各求值一次；assignment、`++`/`--`和复合赋值使用9.3.3的typed place。`?.property`只在Some分支执行getter并把结果再包一层Option，不展平嵌套Option；
- extension property只在top level声明且没有backing field，可以是computed或delegated property。generic extension写作`val <T> Receiver<T>.p: U ...`；全部type parameter必须只由receiver exact静态type与bound唯一确定，不能用result expected type或setter value补推断。generic delegated extension的每个成功使用的exact receiver application拥有program-wide唯一、lazy exactly-once的effective delegate storage与失败状态；其`GenericDelegateStorageSpecializationId`只由origin `PersistentExtensionPropertyId`和完整receiver concrete type arguments决定，不包含result expected type、setter value、re-export路径或使用它的Cone。不同application互不共享storage，多个Cone使用同一application也只执行一次delegate初始化；

**constructor与body member：**

- class primary constructor参数可以是普通参数，或以`val`/`var`同时声明stored property。普通primary参数只在base constructor arguments、class body property initializer与`init`中可见，不进入对象布局，也不能从普通member function或secondary constructor读取。secondary constructor参数不得写`val`/`var`，不声明自己的type parameter或返回类型；它与primary constructor都使用8.5的完整source argument protocol；
- class body可以声明上述property及任意多个`init { ... }`。同一class的primary parameter名称必须唯一；body property不能与primary property或同class其他property重名。遇到inherited同名property必须形成合法显式override，不能静默field shadow。body property可以与普通primary parameter同名；common initialization中无receiver名称优先解析到parameter，property storage用`this.name`直接访问。secondary parameter同样可按普通词法规则遮蔽property；
- 显式`class C(...)`（包括空参数列表）声明primary constructor。普通class既无显式primary也无secondary时拥有隐式internal零参数primary，再与owner effective domain取交集；public class不会因此隐式获得public construction API。若body声明了secondary而header没有参数列表，则不存在primary。intrinsic type省略源码representation不因此获得普通零参数constructor；
- secondary constructor写作`constructor(parameters) : this(args) { body }`或`: super(args)`。class有primary时每个secondary必须经`this`直接或间接终止于primary，不能直接`super`；class无primary时每条链必须终止于一次`super`，省略delegation等价于`super()`：存在direct base时解析其constructor，没有显式base时正规化为编译器根初始化。每个secondary恰好一条delegation edge，自环或多节点环都是定义处错误；
- 有primary的derived class在header用`Base(args)`完成base delegation。无primary、仅有secondary的derived class在header只声明bare base type，由每个terminal secondary执行`super(args)`；两处不能同时提供base arguments。interface不能携带constructor arguments，class最多有一个direct base；
- primary base delegation arguments可以读取全部primary parameter（写`val`/`var`的参数此时也仍以parameter value读取），但不能使用`this`或任何instance field；secondary `this`/`super` delegation只能读取该secondary自己的parameter，也不能访问`this`/field。delegation target的default同样没有source receiver；
- primary与全部secondary共同构成nominal constructor overload set，使用8.6的candidate-local mapping、constraint与MSC。constructor签名只由实际参数类型区分；参数名、default、`vararg`marker及相同的host返回类型不区分重载。abstract class不能普通构造，但其constructor可以作为derived `super`目标；constructor declaration reference不受支持。

**class初始化顺序：**

1. 调用处先选择唯一constructor与完整nominal type arguments，再按8.5求值、物化全部实参；显式实参属于caller，因此可以在suspend caller中先挂起；
2. 分配一次exact concrete对象，写入最派生TypeDescriptor并清零完整payload；随后所有`this`/`super` constructor initializer共享该对象，不重新分配或替换identity；
3. 每条constructor delegation先按其自身源码顺序求值显式实参，再物化目标default/`vararg`，然后direct调用typed目标；
4. terminal primary先完成direct base constructor，再按声明顺序把带`val`/`var`的primary参数写入本class字段；terminal `super` secondary先完成base constructor；
5. body stored initializer、optional synthetic None store、delegate initialization与`init`按照它们在class/object body中的源码顺序交错执行。method、computed/abstract property、nested declaration和secondary constructor本身不执行，也不改变相邻初始化项顺序；
6. terminal secondary body在common initialization之后执行；返回每一层`this` delegation后，再从最内层到最外层执行其余secondary body；最外层正常返回后构造表达式才产生对象引用。

base class的全部constructor body与初始化项先于derived自有字段。每个class的common property/`init`序列在一次构造中恰好执行一次。任一步骤抛异常时，后续初始化项与外层secondary body不执行，构造表达式不产生值，已发生的外部副作用不回滚；失败对象不可达并由GC正常回收，没有析构或runtime回滚。

**字段就绪与半初始化receiver：**

- base initializer正常返回后base storage就绪；primary property在对应compiler store后就绪；body stored/optional/delegate property只在其initializer与storage write正常完成后就绪。initializer与`init`只能direct读写已经就绪的inherited/primary/earlier backing field；self/forward read、通过`this`绕过顺序及提前写later `var`都是定义处错误。computed/delegated accessor不能以initializing receiver调用，分配时的全零payload不是合法源码默认值；
- class/struct constructor、class property initializer与`init`中的`this`是受限initializing receiver：只可用于已经就绪字段的direct read与mutable write。它不能作为普通值传参、返回、存储、捕获、装箱、转换、比较、取址或形成callable reference，也不能作为ordinary/extension/virtual/interface/`super` method receiver。读取field后得到的值是普通值，可以正常参与调用；
- 上述限制对base constructor同样成立，因而构造期间不能通过virtual dispatch观察derived未初始化字段，也不能依赖whole-program escape analysis判断某个final helper“可能安全”。需要init-safe callable时必须另行引入typed effect；
- `return`不能退出property initializer、`init`或constructor body；`throw`合法。其中声明的局部函数与匿名函数按8.1.2建立自身的返回边界，`return`只返回该函数；lambda中的裸`return`仍非法，嵌套callable也不能捕获initializing receiver。所有这些体及delegation都是ordinary、safe、non-suspend上下文；可以调用普通函数、分配、触发GC，也可以构造尚未执行的suspend task，但不能立即调用suspend函数。`startCoroutine`本身是同步普通builder，其启动计算不成为尚未完成的初始化步骤。

**`super`成员调用：**

- 普通member body中的`super.name<TypeArgs>(args...)`只在direct base application的class member层按8.5/8.6决议，不收集extension、property-like或interface default候选。winner强制direct调用base静态视图中的具体实现，即使最派生对象覆写同一virtual family也不经vtable；abstract且没有具体base实现的目标不可调用；
- `super<I>.member`的interface default规则见9.1.4。`super`不是一等表达式；裸`super`、`super.field`、`super::name`、safe/invoke/index形式非法。初始化上下文也不能以任一super形式绕过受限receiver。

普通property accessor、delegate访问和object/global initializer都同步且non-suspend；可以分配、GC和抛异常。需要异步取得值必须显式暴露`suspend fun`，不能藏在getter或delegate协议中。

#### 9.1.2 `const val`

- `const val`只允许声明在top level、`object`或`companion object`中；必须有显式type和initializer，不能是extension/local、`var`、delegate或带accessor。其type必须是`Boolean`、基本数值类型、`Char`或`String`。
- initializer 必须是编译期常量表达式：字面量、对其他 `const val` 的引用、由它们组成且可在编译期确定结果的内建一元/二元运算，以及通过 typed integer intrinsic registry 精确解析到整数表示运算或整数转换的封闭 call。后一类包括源码显式的 `inc`/`dec`、`compareTo`/`equals`、`div`/`rem` 与 `toX` 等方法形式；只有 exact typed registry identity 才使 call 成为常量表达式，用户声明或仅同名的 callable 不获得该能力。所有实参都必须是常量表达式；常量 `div`/`rem` 的除数为零是 const 定义错误。const 依赖图存在循环是编译错误。
- 除上述封闭整数表示 intrinsic 外，函数/方法调用（包括 `toString`）、构造、普通属性读取、数组或其他对象分配、`throw` 及挂起调用都不是常量表达式。`const val` 不生成需要在程序启动或单例首次访问时执行的 runtime initializer；位于 `object` / `companion object` 中的 `const val` 引用本身不触发单例初始化。
- 导出的`const val`的type和值属于`.slib` HIR metadata，下游Cone在编译期直接消费；值变化会使下游编译缓存失效。const没有getter或可寻址storage，`addressOf(const)`非法；String常量使用已登记immortal表示。visibility在folding前检查。

#### 9.1.3 `object`、companion、nested declaration与全局初始化

**M29 泛型 companion 修订。** 本节取代 [M21 设计 §3.2](../milestone21/DESIGN.md) 中“不捕获宿主类型参数、所有宿主 application 共享一个 companion”的选择；历史设计保留原文。普通 static nested declaration 的作用域规则不在本次修订之内。

- `object O`同时声明一个nominal ref type与singleton value，二者identity类型化且不同。object不能自行声明type parameter或primary/secondary constructor，可以继承一个class并实现interface；base constructor后按9.1.1执行property/delegate/`init`。普通top-level/static nested object只有一个singleton；companion按下述完整宿主application区分singleton。`O`不是普通constructor，`O()`非法；
- 依赖 Cone 中的 object 遵守同一规则：类型位置引用原 nominal，值位置引用原 singleton value；二者来自同一声明/application时不构成值查找歧义。每次值访问先确保对应初始化单元成功，再读取其已发布根。转导出、默认参数展开、成员访问和下游再次发布都保留原实体身份；同一companion application跨Cone只对应一个逻辑对象，普通泛型物化及ODR必须合并其状态和初始化支持；
- class/struct/enum/interface/object至多有一个`companion object`，省略名称时为`Companion`。每个完整宿主类型拥有独立的companion类型与singleton：`Box<Int>.Companion`和`Box<String>.Companion`是不同类型、不同对象，各自拥有成员状态；同一`Box<Int>.Companion`的重复访问得到同一对象。即使companion不使用T、布局相同或没有字段，也不能合并不同宿主实参的对象或类型identity；
- companion的声明及成员可使用直接宿主的类型参数和已有bound，包括字段、基类/interface application、参数、结果、default和初始化表达式中的类型位置；例如`fun create(value: T): Box<T>`合法。它不捕获宿主实例、primary parameter或其他实例值，this始终是companion自身。companion没有另行书写的类型参数列表；内部普通generic function可声明自己的参数，按3.2与宿主参数分别绑定，不能重名；
- 实际引用generic companion的类型、值、成员或const时，宿主必须写出完整实参，例如`Box<Int>.Companion`、`Box<T>.Companion.create(value)`、无冲突时的`Box<Int>.create(1)`。T可以是当前作用域已声明的参数，随后正常单态化；缺少实参的`Box.Companion`/`Box.create(1)`及带`_`的宿主限定不形成完整companion引用，不从方法参数或期望结果推断宿主。const仍无runtime初始化，不能因此省略宿主实参。非generic宿主继续使用`User.Companion`/`User.create(...)`；
- 命名companion的`Box<Int>.Factory`与`Box<Int>.Companion`是同一application。透明类型别名如`typealias IntBox = Box<Int>`不产生新对象；`IntBox.Companion`与`Box<Int>.Companion`相同。裸generic名称仍可用作声明命名空间限定符，import只定位声明，不能代替形成companion类型/值所需的宿主实参；没有新增`Companion<Int>`或带类型实参的import语法。companion member不进入宿主instance lookup，不被派生class继承；
- 跨 Cone 的 companion、嵌套声明和转发按实际 host 的静态命名空间解析，保留完整宿主application。`Host.member`只查实际导出的转发binding，再使用成员所属companion application的真实receiver，不生成static副本。限定路径本身不构造或初始化host，const读取也不触发singleton初始化。companion属性赋值、复合赋值与自增均先求值并保存一次实际receiver，再求值右值并调用对应getter/setter；可见性、值遮蔽与本地声明相同；

`hir/cross-cone-interface/30` 补齐 object 的声明种类：source-shape 的旧 Object tag 8 退役，新 tag 9 保留 field 1=value、field 2=声明序字段，新增 field 3=`Standalone(1)` 或 `Companion(2)`；host 沿已有声明 key 的 typed owner 查询。命名 companion 发布名称与 `Companion` 两个普通 type/value binding，object 的公开方法和属性进入自身静态 binding 表。共有命名空间在本 owner 无同名 binding 时，沿已声明的 companion 关系转发其直接 binding；不复制成员声明、不用名称或初始化 metadata 推断 companion。旧 `/28` 产物与缓存重建，退役 tag 不复用，runtime ABI 不变。
上述为历史格式的演进记录；M29的companion宿主binder/application及初始化物化已在后续格式中补齐，见实现规范2.17。`/30`本身不包含这些语义，不能把该历史版本当作当前companion格式。

- body可以声明static nested class/struct/enum/interface/object，包括companion body中的普通嵌套声明。它们没有implicit outer receiver或outer type parameter；需要关联时显式声明自己的参数。generic outer名称可作为owner qualifier而不构成裸generic application；`Box.Nested`仍是独立声明，不因`Box<Int>`/`Box<String>`复制。只有companion按直接宿主application参数化，普通static nested声明仍是外层类型参数作用域的边界。`inner class`、anonymous/local object/type及implicit outer instance capture不支持；
- ordinary top-level stored/delegated property可以是`val`或`var`、可以包含managed ref，使用compiler-managed hidden storage/accessor并进入global root表；它不可`addressOf`。`@Global`/`@ThreadLocal`仍只表示13.6的显式可寻址GC-free raw storage，`@Extern`仍只表示C data symbol；这些storage形态不能带普通accessor/delegate或与ordinary property混用；
- 泛型正文对普通顶层属性的读写始终操作声明方的同一份状态，包括 private/internal 属性、静态初值与运行时初值。不同类型实参、不同消费 Cone、再次发布以及嵌套 callable 不复制该属性的 storage、初始化单元或 GC root。模板保留定义处已解析的属性访问，不使该属性进入消费方的源码可见域；
- 需要runtime求值的top-level property使用`StaticInitialState::ZeroedForRuntimeUnit`：其完整storage先以canonical zero/null carrier登记，在全部Cone的image/stackmap/type/root/init metadata、GC与主线程就绪后、`main`前由对应unit exactly once求值并写入。只有无需执行Scoop代码、无需读取ordinary property且可直接编码为目标静态数据的literal/内建纯常量表达式、immortal String ref及Option `None` shorthand可省略unit；这些声明必须改用`StaticInitialState::EncodedStaticValue`，不能仅因为最终bits为零冒充前一分支。Encoded状态包含恰为storage allocation extent的canonical target-representation template（padding、ZST token及managed-ref位置为零）和按pointer offset排序的typed immortal relocation；非null managed ref只能重定位到已登记immutable String对象的精确object start，`None`不产生relocation，不允许任意heap/interior ref。compiler/reader 在对象边界验证 template、非引用初值及 relocation；runtime 在首个 managed 代码前核对加载后的 GC leaf 与已登记 immortal object-start，复用未变化的静态内容检查。通过后该 storage 无需 cell/unit 即可作为合法初值读取。优化器不得因事后fold而改变有可观察求值的初始化语义。文件之间没有source order；二进制判等与同Cone排序唯一使用kind-specific `PersistentInitializationUnitId` bytes。其canonical declaration/specialization key已经编码origin `ConeIdentity`、owner chain、package、kind与name，只有file-private owner再加入标准化Cone-relative source identity；canonical Cone coordinate与declaration path只形成独立的稳定诊断path，不参与第二套unit hash。identity不依赖输入枚举、session arena id、re-export路径或host绝对路径，runtime不得退回table index或可读path。多Cone顺序见12.3；访问另一个unit会先ensure目标。HIR只对该unit自有且经脱糖展开的initializer/delegate expression、object base argument与`init`body中的直接typed unit引用形成依赖图并报告结构环，不递归进入被调用的普通function/constructor/default/dynamic/FFI body；这些间接环由runtime gate检测。startup失败则`main`不执行；
- object/companion在首次非const访问时线程安全、同步初始化；static nested declaration或const引用不初始化外层。每个runtime unit状态为Uninitialized、Initializing(owner/dependency stack)、Initialized或Failed(rooted Throwable)。成功singleton只在完整初始化后release发布；失败不发布、记忆异常且不重试。同线程或跨线程wait-for环抛出`message`含稳定unit path的`IllegalStateException`；该异常若未在initializer内被普通`try`捕获才使unit失败。其他线程以可参与safepoint的方式等待terminal state；
- 每个实际使用的companion application独立执行上述exactly-once协议：拥有自己的initializer、cell、published root与failure root，失败缓存和副作用互不串用。同一application跨Cone共享这一组状态；不同application只有显式初始化依赖才互相ensure。只构造`Box<Int>`不初始化其companion，只访问companion也不构造Box实例；单纯类型引用或静态描述查询不执行初始化。未具体化的companion模板没有运行期对象或cell；
- 上述exactly-once cell只管理`ZeroedForRuntimeUnit`完整initializer的发布，不表示property可以缺少声明type的值；`EncodedStaticValue`没有cell/unit，在全程序metadata验证成功时即已包含合法声明type值。Initialized storage始终包含合法值；9.1.1 Option shorthand的值是普通`None`。

#### 9.1.4 Interface default implementation

- interface function有body时提供default，无body时形成abstract obligation；property按getter/setter slot分别判断。private interface member必须有body且只作词法helper，不进入itable、继承或override；
- 对每个typed slot先选择class hierarchy中最近的实际方法声明；具体声明成为实现，抽象声明保留该声明的义务并压制接口default。没有class声明时，删除被更specific subinterface覆写的interface候选，唯一剩余default获胜，只剩abstract即保留实际抽象声明，多个互不相关default则必须显式override。getter/setter独立选target，但property整体仍满足9.1.1的type/mutability规则；
- concrete class/object/value type不能留下abstract obligation；abstract class可以保留。itable entry显式指向class/value implementation、interface default或typed adjust thunk，不能按implements列表顺序选择；
- 普通member/default/accessor body中的`super<I>.function(args)`、`super<I>.property`与`super<I>.property = value`只direct调用当前owner显式列出的direct superinterface exact application上的concrete default。抽象target、间接/非父qualifier、extension/property-like/callable-reference/safe形式非法；初始化上下文仍禁止。interface default body也只能这样选择自己的direct superinterface。

#### 9.1.5 可见性与annotation

- 默认visibility是`internal`。所有允许visibility的声明在省略modifier时都于HIR前确定地正规化为internal，不从owner/base/interface继承visibility；setter省略modifier按下条继承property visibility。该规则适用于top-level nominal/function/property/object及nominal中的method/property/nested declaration/constructor；local、parameter、`init`块与accessor parameter不能声明visibility。`main`按入口契约发现，不要求public，也不因internal进入`.slib`导出表面。top-level允许public/internal/private，其中internal表示当前Cone、private表示当前source file；member/nested允许public/internal/private，其effective domain还要与全部owner domain取交集。protected只允许class member/nested/constructor，表示声明class及subclass body可见；explicit receiver的静态type还必须是当前访问subclass或其子类；
- 继承class的object/companion在自身body与基类构造委托中同样作为subclass参与protected检查；当前访问owner及explicit receiver沿其typed backing-class关系判断继承，不改变其object词法owner或singleton identity。非subclass上下文、基类静态type或其他兄弟subclass的explicit receiver仍不得访问protected成员。
- protected检查保留词法嵌套上下文：当前nominal及其词法owner链中任一class（object/companion按typed backing class）都可提供subclass访问上下文。explicit receiver必须相对于同一个提供权限的class满足静态type约束。故class内private成员的词法域包含于该class的protected域，protected property可使用private setter；这不会使子类获得基类private setter的权限。
- effective domain中由外层nominal带入的protected约束只检查访问点的词法权限，不把nested类型实例的receiver当作外层class的receiver。仅当所访问的method/property/accessor自身声明为protected时，才相对于该成员的声明class执行explicit receiver静态type检查；public/internal/private成员即使位于protected nested类型中，也不会额外获得protected receiver限制。setter使用setter自身的visibility与logical property owner。
- 候选只有在当前访问点属于其 effective domain 后才进入 applicability/MSC，成功结果保留实际声明引用与访问域；同名但不可访问的声明只参与诊断，不会让该层遮蔽较低层。getter可见的logical property一旦选中后，setter缺失或不可见仍按9.1.1直接报assignment错误，不触发fallback；
- struct/enum字段与enum variant保持固定public representation visibility，但仍与owner domain取交集；只有外层value type显式public时才对下游公开表示。普通member/nested的direct lookup domain也是声明visibility与owner domain的交集。private member不被继承或override、不进vtable/itable；internal open member只可在同Cone override；
- interface member同样默认internal。internal interface可拥有internal abstract/default contract；public interface的abstract/default contract必须显式public，不能把遗漏modifier静默升级。private interface helper必须有body且不进itable/override，protected interface member非法；因此public interface不能携带下游不可见的hidden obligation；
- abstract member的slot contract必须覆盖owner的合法inheritance domain；public abstract/open class中的abstract member至少显式protected或public，internal/private owner则可使用internal obligation。下游不可见的abstract member不能用来把public type隐式变成sealed；已有body的internal open member不形成hidden obligation，下游继承但不能override；
- override省略modifier时仍为internal，不继承base visibility。coverage比较声明visibility形成的slot contract domain，而不是被concrete owner收窄的direct lookup domain；它必须覆盖全部被覆写slot，因而public contract需要`public override`，即使实现type是internal/private。此时直接名称访问仍受owner限制，经base/interface静态类型调用则服从public slot。显式`protected override`覆写protected槽时保留该槽原声明class的protected region，不把slot覆盖域重新锚定到实现子类；直接名称查找仍按实现声明的owner检查。此规则不允许protected实现收窄public槽。允许显式扩大。getter沿用property visibility；setter可声明不更宽的private/internal/protected visibility。选中property后setter不可见是assignment错误，不改选其他候选；
- declaration signature中的type、receiver、base/bound及annotation type必须覆盖该declaration的direct access/call domain。经base/interface调用时使用该静态声明的签名及其完整类型实参；override的slot coverage不把实现者的签名扩展为独立public API。因此internal/private实现者可为非public类型实现public泛型接口，公开接口声明与公开返回类型仍不能泄漏不可见类型。default直接绑定实体须覆盖实际展开该default的call domain。visibility在const folding、companion forwarding和desugaring前检查；M17 default保留已绑定的kind-specific typed引用；前端只在定义处和继承导致调用域变化时检查覆盖关系；
- class primary constructor需要modifier/annotation时写显式`constructor`关键字；无modifier的class header/explicit primary及class/struct secondary constructor均为internal。class primary property parameter可声明member visibility/override，普通parameter不可。class隐式零参数constructor为internal并与owner domain取交集；public class需要显式`public constructor`才提供public construction API。struct primary constructor与enum variant constructor属于固定public representation entry，只与owner domain取交集且不能单独声明visibility；
- property-level custom annotation不传播到accessor/backing/delegate storage；explicit accessor可单独标注。`@Unsafe`/`@Safe`可用于constructor/accessor并进入调用contract；`@NoGC`只在完整signature/body确实GC-free的explicit accessor或struct secondary constructor合法。class/object receiver为ref，不能满足NoGC。`@Extern`/`@Global`/`@ThreadLocal`及`@CallingConvention`仍限各自13章target；普通property/object/constructor不能伪装为native symbol。
- constructor的`@Unsafe`/`@Safe`控制该声明的参数缺省表达式、`this`/`super`委托表达式及secondary body；overload选择完成后才检查所选constructor的safety，不因safe调用上下文改选其他候选。class/object的property initializer、delegate与`init`属于共同初始化声明，使用独立safe上下文，不继承任一constructor的注解；需要unsafe操作时使用显式`@Unsafe` block。

#### 9.1.6 GC-free release block

M24 为普通 final class 增加至多一个 `release { ... }`，在遗漏显式释放时兜底清理 native resource。实现范围与跨 Cone 数据流见 [M24 设计](../milestone24/DESIGN.md)，实际检查与完成情况见 [M24 验收记录](../milestone24/ACCEPTANCE.md)。

```scoop
@Extern(name = "free")
fun nativeFree(address: Ptr<Unit>)

final class NativeOwner public constructor(
    private var handle: Option<Ptr<Unit>>,
) {
    public fun close() {
        val owned = handle
        handle = None
        when (owned) {
            Some(value) -> @Unsafe { nativeFree(value) }
            None -> {}
        }
    }

    release {
        when (handle) {
            Some(value) -> @Unsafe { nativeFree(value) }
            None -> {}
        }
    }
}
```

`release` 只在 type body 的 member 起始位置、后接 block 时作为 contextual keyword。`fun release()`、局部变量及普通参数中的同名 identifier 保持原义。release block 没有名称、参数、返回类型、visibility、annotation 或 modifier；不进入 lookup、overload、override、dispatch 或 callable reference。正常完成结果固定为 `Unit`；block 内的 `return`、`throw`、`try`、挂起及局部 callable 声明均为编译错误。`while`、`break`、`continue`、`when` 和值绑定沿用已有规则，但所有实际执行的操作须满足下述 effect。

**owner 与字段读取：**

- owner 必须是普通、可实例化的 final class，默认 final 也合法；`open`/`abstract`/`sealed`、interface、struct、enum、annotation class、intrinsic class、object/companion 及 compiler-generated class 均不能声明。`Throwable` 的继承闭包同样排除，因为异常记录会按值复制 payload。static nested 与 generic final class 合法；可以继承普通无 hook 基类并实现 interface，release 不继承、不覆写、不形成 hook chain。
- block 没有 managed `this`，只在独立的 reclaiming-receiver 上下文中读取本 owner 自己声明的 backing field。primary property 与 body stored property 均可读；即使 stored property 有 custom getter，此处仍直接读取 backing storage。computed、delegated、inherited property 及 primary constructor 的非 property 参数不提供这样的字段。局部绑定按普通词法规则遮蔽字段，不能用 `this` 绕过遮蔽。
- 字段读取取得普通值副本；字段赋值、复合赋值、更新及 `addressOf(field)` 均非法。对副本的值类型投影、解构、局部更新及合法 native pointer 操作遵守现有规则。源对象不能被传参、返回、存储、捕获、装箱、cast、比较、取址或取得 root/handle/pin；`super`、本对象的 getter/setter/method/extension 不能被调用。

**`ReleaseValue`：**

`ReleaseValue(T)` 是普通类型属性：exact `T` 必须 GC-free，并且其实际递归表示不包含已解析 compiler protocol 所指的 `PinnedPtr`、`GcHandle`、`FunPtr` 或 `ForeignCallback`。判定使用真实 typed declaration/representation，不使用 FQN、字段形状或 core 来源资格。它不建立新的源码 trait、annotation 或资源所有权类型。

Boolean、已实现的数值类型、Unit、只由合格字段组成的 struct/enum/tuple、相应 Option 均可用；enum 要求全部 variant 合格。`Ptr<Unit>` 可用，`Ptr<T>` 还要求 pointee `T` 满足同一条件，不能通过 `Ptr<GcHandle<...>>` 绕过限制。指针间接形成的合法递归类型按有限类型图求属性固定点，不能因回访节点就报错或无限展开。只检查参与表示的参数，phantom type argument 不额外受限；`sizeOf<T>()` 等纯布局查询也不因类型实参而产生运行时 `T` 值。

block 的字段读取、local、temporary、实参和结果都须满足该条件；未被读取的 managed 字段不影响 owner 合法性。generic release template 保存实际需要的 `RequiresReleaseValue` 条件，沿已有类型替换规则传播；闭合 application 即使只出现在签名、别名或字段中也必须满足，不能等到构造或回收才检查。依赖 `ref` bound 的不可能条件在 release 定义处诊断。`Ptr<Unit>` 和 integer 的合法性不证明其地址来源、生命周期或 ownership；通过 unsafe/native 代码访问 GC heap、隐藏 managed reference 或恢复源对象仍违反 unsafe/FFI 契约，编译器不增加通用来源追踪或防伪机制。

**release-safe 操作与调用：**

- release 是比普通 `@NoGC` 更窄的执行上下文，不是 `@NoGC` annotation target，也不新增公开 `@ReleaseSafe`。禁止 managed allocation/ref、boxing、String/Array/closure、异常、suspend、safepoint/poll、初始化 ensure、动态/接口/间接调用及 root/handle/pin/thread/GC runtime 操作。
- 直接调用的 Scoop helper 必须有实际 Scoop 正文、满足既有 NoGc 合同，并经定义方推导为传递 release-safe。普通顶层/扩展函数、值类型 direct method/accessor、值类型 secondary constructor 及编译器已有的纯存储 accessor 使用同一规则；值的 primary construction 和 enum assembly 仍为普通值操作。未标注且未由既有规则生成 NoGc 合同的 callable 不因正文看似简单而自动放行。任何 C/Scoop ABI extern 函数调用、native transition、TLS、managed 操作或捕获环境都会使 helper 不可用于 release；不为 helper 生成另一份 release-context body。
- 推导结果和泛型条件保存在已有 callable 接口中。依赖方直接消费该效果信息与真实 typed target，不重新读取非泛型 helper 源码或遍历其完整实现调用图；泛型正文仍在本次正常实例化中检查替换后的值和实际调用。没有额外调用资格、凭证或独立信任链。
- release block 本身可以在显式 `@Unsafe` 中直接调用 C ABI extern。签名须满足既有 C-FFI-safe，全部参数及结果还须满足 `ReleaseValue`；此调用直接使用 native ABI 或既有纯 storage bridge，不执行 managed/native transition。同一 extern 在普通代码中的调用仍走既有 FFI 协议。C 实现必须不展开异常、不回调任何 Scoop 入口、不操作 GC/root/handle/pin/thread runtime、不保留 hook 的临时地址，也不等待已停顿的 mutator 或依赖其进展。这是调用处承担的 unsafe native 契约，不能由 `@Extern` 拼写、`nounwind` 或对象文件符号扫描证明。
- effect 在普通名称查找、唯一目标选择和完整实参展开之后检查。默认参数、operator/accessor、解构、`for`、`vararg` 等产生的操作一并检查；不能因 effect 不合格而退回另一个重载。运行期整数 `/`、`%` 仍按 11.2 调用可抛异常的 Managed 运算，即使受 `if` 保护或除数为常量也不允许；已合法求值的 GC-free const 可直接使用。不新增路径证明或循环执行预算。
- `@Unsafe`/`@Safe` 嵌套规则不变。TLS、singleton 和需要 ensure 的普通全局属性不可访问；GC-free const、无 ensure 的合格普通存储 accessor、非 TLS 的 GC-free raw/native global 可按原可见性、unsafe 和数据竞争规则访问。native global storage bridge 只进行普通存储操作；`@ThreadLocal` 不能借 helper 或 bridge 绕过限制。

**生命周期：**

1. 分配时内部 `RELEASE_READY` 为 0；完整最外层 constructor 正常返回后、构造表达式产生对象值前，由生成代码置 1。base/`this` delegation 不发布，异常出口不发布；本地、依赖与泛型构造遵守同一规则。构造失败前取得的 native resource 由构造/调用方显式清理。已经独立构造成功的子对象保持自己的生命周期。
2. collector 在对象逻辑死亡且 storage 即将真正回收时，先原子清除 ready，再同步调用 exact TypeDescriptor 的 hook，返回后才能 poison、复用或释放 storage。live、pinned、被 root/handle 保活的对象不调用；moving 的旧副本不表示逻辑死亡，ready 随存活对象搬迁。ABI 与顺序见 runtime spec 2.1、2.2、3.8。
3. best effort 不保证 GC 时机、对象间顺序、执行线程、native release 结果或退出时调用；正常 collection 一旦回收 ready 对象，就必须在存储失效前尝试一次且至多一次。shutdown 不补做全堆释放。
4. 显式 `close`/`release` 与 `try/finally` 仍负责确定性释放。显式路径先将字段置为 `None` 等 inert state，再释放取出的资源；hook 以后仍可运行，但不再释放该资源。并发关闭和 native handle 别名由库自身的同步/ownership 契约处理。语言不自动生成 close，也不公开 arm/disarm、手动 hook、重试或异常传播接口。

release 不提供 GC finalizer、对象图访问、对象复活或及时释放保证；不能依赖它完成 flush、事务、锁或要求特定线程的操作。

### 9.2 委托

- property delegate使用reflection-free协议。角色必须显式声明为ordinary、non-suspend、non-generic `operator fun`，不得带default/`vararg`：可选`provideDelegate(): D2`、必需`getValue(thisRef: R): T`，以及`var`必需`setValue(thisRef: R, value: T): Unit`。它们是与9.3普通operator不同的typed role；无role的同名函数不参与；
- role 查找沿普通成员优先、扩展按作用域分层的可应用性与最具体候选规则进行。同一层中的本地声明和依赖声明共同比较；import alias 不改变声明的 typed role，也不使扩展覆盖已有可应用成员；
- `R`对class/object member是owner type，对extension是extension receiver，对top-level/local是Unit。先求值`by`表达式一次，再可选调用一次无owner参数的provideDelegate并把effective delegate存入hidden field/global/local；之后每次access读取delegate并调用唯一get/set target。协议没有`KProperty`、property name或annotation metadata；需要这些值必须在`by`表达式中显式传入；
- 非局部delegate必须显式声明property type。local delegate省略type时，先在没有result expected type的条件下选出唯一`getValue`role，再以其concrete结果作为property type；`var`的`setValue`必须接受同一type，不能从多个get/set组合反向猜type或让setter改变getter结果；
- class/object delegate storage进入9.1.1 common sequence/readiness和GC scan；top-level与non-generic extension delegate进入9.1.3 eager unit；generic extension delegate按9.1.1为每个exact receiver application建立独立的program-wide lazy unit，第一次访问该application时才求值`by`表达式并发布effective delegate，不能因最终程序已知使用点而改成eager startup。`by`表达式与可选`provideDelegate`按定义处已绑定的typed template在consumer Cone替换receiver type arguments，不重新解析name、import或operator。`by` 与 `provideDelegate` 不取得某次访问的 receiver 值；get/set 才接收实际 `thisRef`。普通赋值先求值 receiver 与 RHS，再进入 setter 的 ensure；复合赋值先执行 getter/ensure，再求值 RHS。该specialization的ODR group整体包含effective delegate storage、managed root descriptor、init cell、failure root、initializer/ensure entry与init descriptor；多个Cone产生同一specialization时必须共享并整体coalesce这一组成员，re-export不复制它。local delegate是不可重新绑定的hidden local。struct/enum member不能delegated。delegate调用同步，可分配、GC、抛异常但不能挂起；
- class/interface delegation `class C : I by impl`尚未定义，不因property delegate落地而继承Kotlin语义。

### 9.3 运算符重载与约定

#### 9.3.1 `operator`声明

参与运算符约定的函数必须显式写`operator` modifier；仅仅使用约定名称不会使普通函数成为运算符。除下述`equals`外，operator必须是成员函数或extension函数，可以是ordinary/suspend、generic或infix。operator标志属于override contract，override与被覆写声明必须完全一致。

HIR把通过验证的角色保存为封闭、类型化的operator identity；表达式决议不得再从函数名或签名反推能力。合法名称与声明约束如下：

| 角色 | 显式普通参数 | 额外约束 |
| --- | --- | --- |
| `unaryPlus` / `unaryMinus` / `not` | 0 | 返回类型不限 |
| `inc` / `dec` | 0 | 返回类型必须是receiver静态类型的子类型 |
| `plus` / `minus` / `times` / `div` / `rem` | 1 | 返回类型不限 |
| `rangeTo` / `rangeUntil` | 1 | 返回类型不限 |
| `contains` | 1 | 返回`Boolean` |
| `get` | 至少1个 | 返回类型不限；允许按8.5使用default/`vararg` |
| `set` | 至少2个 | 返回`Unit`；最后一个参数是写入值且不得为`vararg`，此前参数是索引 |
| `invoke` | 任意 | 使用完整8.5调用参数协议 |
| `plusAssign` / `minusAssign` / `timesAssign` / `divAssign` / `remAssign` | 1 | 返回`Unit` |
| `compareTo` | 1 | 返回`Long` |
| `equals` | 1 | 返回`Boolean`；见下述收紧规则 |
| `componentN`（`N`为正十进制整数） | 0 | 返回类型不限 |
| `iterator` | 0 | 返回值在`for`使用点满足11.8的`Iterator<T>`协议 |

固定元数角色的参数可以具有default，但operator语法提供的operand仍按8.5映射到对应参数；只有`get`、`set`和`invoke`能以`vararg`表达可变元数。名称不在表中、元数或返回约束错误、`component0`及非数字`component`名称都是声明处错误，不能以普通函数身份携带`operator`标志进入HIR。

属性委托所需的`provideDelegate` / `getValue` / `setValue`不是本表的`set`下标角色；它们按9.2形成三个独立typed role，不能仅按名称或本表的普通operator identity参与delegate协议。

`equals`是本规范对Kotlin约定的有意收紧：可参与`==`的声明必须是名为`equals`的**成员**`operator fun`，恰好有一个显式参数并返回`Boolean`，且不得为generic或suspend。顶层、局部与extension equals不参与`==`。成员可以重载，也可按普通规则声明为final/open/abstract/override；具体决议见11.11。

`equals`签名中的参数必须写普通显式类型。Scoop没有`Self`类型：若interface需要表达“与某个类型比较”，应写成例如`interface EqualTo<T> { operator fun equals(other: T): Boolean }`，实现者显式选择`EqualTo<Point>`等application；编译器不把interface中的任何名字隐式替换为实现者类型。

#### 9.3.2 表达式展开

除单独说明外，operator展开后使用8.6的普通显式receiver调用决议，只保留具有对应typed operator identity的成员/extension候选，并完整复用8.5的default、`vararg`、泛型与求值协议：

| 源码 | 概念调用 |
| --- | --- |
| `+a` / `-a` / `!a` | `a.unaryPlus()` / `a.unaryMinus()` / `a.not()` |
| `a + b` / `a - b` / `a * b` / `a / b` / `a % b` | `a.plus(b)` / `a.minus(b)` / `a.times(b)` / `a.div(b)` / `a.rem(b)` |
| `a..b` / `a..<b` | `a.rangeTo(b)` / `a.rangeUntil(b)` |
| `a in b` / `a !in b` | `b.contains(a)` / `!b.contains(a)` |
| `a[i1, ..., iN]` | `a.get(i1, ..., iN)` |
| `a[i1, ..., iN] = v` | `a.set(i1, ..., iN, v)` |
| `a(args...)` | function-value call，或`a.invoke(args...)`（见9.3.4） |
| `a < b` / `a <= b` / `a > b` / `a >= b` | `a.compareTo(b)`的`Long`结果与0比较 |
| `a == b` / `a != b` | 11.11的成员`equals`调用 / 对同一结果取反 |

receiver先于调用实参求值，因此`a in b`与`a !in b`按概念调用先求值`b`、再求值`a`；这是有意保留的Kotlin顺序。其他表项按书写的receiver再到operand顺序求值。`&&` / `||`仍是只接受`Boolean`的内建短路操作，`===` / `!==`仍是不可重载的引用identity比较；`=`, `?:`, `!!`, `is` / `as`及安全导航本身也不可重载。

operator调用只考虑function-like operator目标，不能再通过property-like `invoke`递归寻找某个同名operator。一次`a(args...)`至多应用一次`invoke`约定；若选中的`invoke`返回另一个可调用值，必须再写一组显式括号才能调用。

#### 9.3.3 自增、自减与复合赋值

`++` / `--`的operand必须是可读写place。编译器先把local/global/field/下标place中的receiver与index从左到右各求值一次，再执行：

- prefix `++a` / `--a`：读取旧值，调用`inc` / `dec`，把适配后的新值写回，并以新值为表达式结果；
- postfix `a++` / `a--`：执行同样的单次读、调用和写回，但表达式结果是写回前的旧值。

对`a op= b`（`op`为`+ - * / %`），候选探测同时考虑对应`opAssign`与普通`op`：

1. 仅`opAssign`可应用时，读取place值并调用它；不写回，因此只读`val`也可以作为receiver；
2. 仅普通`op`可应用时，place必须可写，调用结果必须可赋给place静态类型，再写回；
3. 二者都可应用时报歧义，不能擅自偏好其中一个；二者都不可应用时报完整候选失败；
4. 整个语句的结果为`Unit`。

下标place的读写分别通过同一静态receiver上的`get`与`set`operator选择；receiver、全部index及右操作数都只求值一次。所有这些展开在HIR形成类型化place/evaluation plan后正规化为普通temporary、call与assignment；MIR不得重新解析operator或复制源码子表达式。

#### 9.3.4 property-like `invoke`与候选分区

任意表达式`e(args...)`先定型`e`：若其类型是8.1的函数类型，执行内建函数值调用；否则只从其静态类型收集成员及可见extension `operator fun invoke`，并按普通调用决议。`FunPtr<F>`仍不提供Scoop侧`invoke`。

对名称调用，property-like callable表示“先读取该名称对应的值，再对结果应用一次上述invoke规则”。显式receiver调用的c-level分区顺序为：

1. member function-like callable；
2. member property-like callable + member `invoke`；
3. 各extension作用域中的extension function-like callable；
4. member property-like callable + extension `invoke`；
5. extension property-like callable + member `invoke`；
6. extension property-like callable + extension `invoke`。

M18之前已有的local/parameter/capture、global、primary-constructor property及普通field都可作为property-like来源；extension property/object/companion加入后插入同一分区，不改变算法。每个组合先完成property选择，再以其结果类型建立独立invoke候选；最终优先级取property与invoke两部分中较低者。调用的显式type arguments、命名/spread/尾随lambda只转发给`invoke`，不作用于property读取。

为保留8.1.3已确定的词法遮蔽，最近词法作用域中同名的函数类型binding，或静态类型具有至少一个可见`invoke`operator的binding，先形成唯一local property-like层并遮蔽同名函数声明；其调用形态/类型不匹配时针对该binding诊断。普通不可调用binding不参与callable层。其他层仍遵守8.6的“第一个含可应用候选的分区”，不能因存在同名但不可应用的property无条件阻断后续函数。

#### 9.3.5 `infix`

`infix`函数必须是成员或extension函数，且恰好有一个required、非`vararg`的显式普通参数；可以同时是generic、suspend及`operator`。不满足声明形态、用于top-level非extension或local非extension函数都是声明处错误。`infix`标志与`operator`一样属于override contract。

`lhs name rhs`等价于`lhs.name(rhs)`，要求直接function-like候选带`infix`，或property-like候选最终选中的`invoke`同时带`operator infix`；其余决议与求值规则不变。infix调用左结合，必须显式写receiver（当前`this`上调用写成`this name rhs`）。其优先级从高到低位于range与Elvis之间：postfix、prefix、cast、乘法、加法、range、infix name、Elvis、`in`/`is`、比较、相等、`&&`、`||`、赋值。不同infix名称没有自定义优先级。

普通class可以通过`componentN` operator支持位置解构；每个实际需要的位置独立解析对应operator并只求值被解构值一次。struct/tuple仍走4.6的内建解构，不查找`componentN`。`iterator`只定义11.8中`for`脱糖的入口；M18接受并类型化该operator声明，`for`及range core类型在相应基础语言里程碑实现。

### 9.4 注解

M29定义下列编译期注解规则；实现批次见[M29设计](../milestone29/DESIGN.md)。语言核心注解（`@Intrinsic` / `@NoGC` / `@Extern` 等）继续遵守第13章的独立规则，不内置平台相关注解，也不提供运行期annotation对象。

```scoop
public annotation class Description(val text: String)

@Description("Account data")
public struct Account(@Description("Stable identifier") val id: Long)
```

- annotation class是编译期声明，具有普通名称、typed declaration identity、可见性和import规则；不是可实例化的runtime class，不具有继承、interface、泛型参数、body或成员函数。参数为`val`，类型限于Boolean、String、Char和现有定宽整数；无参数声明可省略括号。
- 使用处采用`@Name(...)`或限定名称，沿普通符号解析选定实际声明。参数遵守位置/命名参数映射，可以有缺省常量；值限于上述类型的字面量、带符号整数字面量和已绑定的同类型`const val`。const val 的限定引用沿普通名称和完整宿主 application 规则，包括 `Box<Int>.Companion.NAME`；只读取已绑定的常量，不执行 singleton 初始化。不得执行任意函数、构造用户对象或把类型作为annotation值。整数范围、重复/缺失/未知参数及可见性错误在定义或使用处诊断。
- 自定义注解可标在名义类型、enum variant、struct/variant字段和class/interface的logical property上。主构造参数带`val`/`var`时注解属于该字段/property；普通值参数不因此成为可注解字段。M29不增加use-site target、可重复注解、注解继承、元注解执行或编译器插件API。同一target重复同一annotation声明是错误；不同注解按源码顺序保留。
- 注解的参数按声明序正规化为typed常量，包含已补齐的缺省参数。泛型application保留原声明的注解；不因具体化产生新的annotation声明，也不把宿主注解复制到字段、派生类、accessor或backing storage。logical property与实际存储的关系遵守9.1.1、9.1.5。
- 注解本身没有可执行副作用。普通用户注解仅进入9.6的静态描述；只有已规定语义的核心注解参与编译。导出的注解声明及应用保留实际类型/常量引用和必要依赖，读入`.slib`后不重新按短名称解释；这些数据不扩大普通源码可见性。

### 9.5 companion 与普通 codec 的编码、解码接口（M29）

两个方向都用显式目标类型参数表达，由companion或普通codec对象实现：

```scoop
public interface Encodable<T> {
    public fun encode(value: T, encoder: Encoder): Unit
}

public interface Decodable<T> {
    public fun decode(decoder: Decoder): T
}

public struct Identifier(val value: Long) {
    public companion object : Encodable<Identifier>, Decodable<Identifier> {
        public override fun encode(value: Identifier, encoder: Encoder): Unit =
            Long.Companion.encode(value.value, encoder)

        public override fun decode(decoder: Decoder): Identifier =
            Identifier(Long.Companion.decode(decoder))
    }
}

public fun <T> encodeTo(value: T, encoder: Encoder, codec: Encodable<T>): Unit =
    codec.encode(value, encoder)

public fun <T> decodeFrom(decoder: Decoder, codec: Decodable<T>): T =
    codec.decode(decoder)
```

- `Encodable<T>`与`Decodable<T>`都是普通invariant generic interface，T是普通显式类型参数。encode的数据参数和decode的结果直接写Identifier、`Box<E>`或tuple等完整类型；没有Self替换、associated type或static成员语法。
- `Identifier.Companion`实现这两个接口，Identifier实例不因此实现它们。两个方法的this都是codec；encode通过value取得待编码数据，decode返回目标值。实现选择、override、slot、可见性及异常均使用普通方法规则，缺省body的合成见11.13。
- `Identifier.Companion.encode(value, sink)`与`Identifier.Companion.decode(source)`都是普通成员调用；无冲突时可按9.1.3转发为`Identifier.encode(value, sink)`与`Identifier.decode(source)`。generic代码显式接收相应codec值；不增加`T.encode`/`T.decode`/`T.Companion`泛型查找、隐式codec参数或companion bound。
- companion随完整宿主application具体化，可声明`Encodable<Box<T>>`或`Decodable<Box<T>>`，但含T字段时仍须在定义处取得相应字段codec。需要调用方选择元素策略时，普通encoder/decoder方法显式接收元素codec并返回持有依赖的普通对象，见11.13.4；依赖不写入singleton的全局状态。
- class继承不继承companion。Base.Companion的`Encodable<Base>`或`Decodable<Base>`不会转移到Derived.Companion，也不会使数据实例获得接口。`Encodable<Base>`不能赋给`Encodable<Derived>`，解码方向同理；显式使用Base codec时，encode的数据实参仍允许普通Derived到Base的向上转换，表示按Base策略编码。运行时数据类型不改变所选codec；字段的自动选择也不向基类companion回退。
- 普通codec class自身继承的方法/default照常选择，参数和结果类型不会因数据类型继承自动改写。codec可以由companion、object或普通class/struct提供，可手写处理接口、基类或singleton目标；自动派生仍限于11.13的形状及普通访问规则。
- 不同interface application的成员按普通overload/override规则检查；尤其两个decode参数相同而结果不兼容时诊断冲突，不能仅按结果类型选择。两个方向可由同一codec或普通组合interface表达，不需要新增内建Codable标记或专用工厂requirement。

### 9.6 静态类型描述（M29）

每个合法类型都有可在编译期查询的结构描述。描述依托共有HIR的typed类型、声明、字段、variant和property关系；源码与依赖产物使用同一数据模型，不建立另一套类型系统。

| 描述对象 | 内容 |
| --- | --- |
| 所有类型 | 类型类别、完整类型表达式或exact identity；名义类型的声明身份和注解 |
| struct | 按声明序排列的字段：独立字段身份、名称、类型、注解、对应构造参数及其已有default引用 |
| enum | 按声明序排列的variant及其注解；各variant独立的有序payload字段、名称/位置、类型、注解及构造关系 |
| class | 直接base关系、本owner的存储字段及logical property；保留声明顺序、访问域、存储/计算/委托类别和构造参数映射 |
| object / companion | 自身的base、存储字段和logical property；companion保留完整宿主application并替换宿主binder，不把宿主实例字段当作自身字段 |
| interface | 父接口、logical property及其类型/注解；不伪造实例存储字段 |
| tuple / Unit | 有序元素，位置名从`_1`开始；Unit为空积；没有源码注解或名义字段声明 |
| 核心intrinsic类型 | 标明其实际表示类别；String、Array、指针等不能因没有普通源码字段就被当作空record |
| 函数、指针、类型参数 | 原有签名、pointee或binder/bound；不伪造字段，也不推断可序列化能力 |

泛型定义的字段类型可以引用其binder；具体化后得到完整字段类型，透明alias展开后与目标共享类型描述。递归引用以typed type reference表示，不递归复制无限树。描述的名称仅用于显示或生成字段名常量，不作为实体identity；enum字段名的作用域是所在variant。class的基类字段经base关系取得，不扁平化成当前owner新字段，编译器生成的delegate slot等也不冒充源码property。

M29的消费入口是编译器的共有HIR查询及其dump，11.13的方法合成是首个语言功能消费者。M29不增加源码可执行的`TypeInfo`值、任意编译期循环/CTFE或公开的通用按字段构造原语。描述必须随必要的`.slib`类型接口/模板保存；普通源码访问仍受9.1.5约束。没有运行期字段表、annotation对象、按名称查类型/字段/构造器的入口，也不扩展runtime TypeDescriptor。

---

## 10. 数组

Scoop 内置两个数组类型（引用类型，属于核心库）：

- `class Array<T>`：不可变数组（长度固定，元素不可写）；
- `class MutableArray<T>`：可变数组（长度固定，元素可写）。

它们是由core源码提供nominal identity、由`@Intrinsic`提供representation family的invariant generic class，使用与普通`class C<T>`相同的类型application、约束、成员解析和单态化规则。编译器可以为字面量、内联元素区、下标和转换保留typed专用操作，但不得再建立一个与core class声明平行的数组类型身份。

跨 Cone 的数组字面量、`vararg`、默认值和泛型正文使用同一规则。隐式数组类型从实际 core 协议取得泛型声明，与显式 `Array<T>`／`MutableArray<T>` 解析为同一 application；成员仍先按实际声明完成选择，再正规化为数组操作。依赖调用必须保留每个位置元素、spread 和命名整数组的区别，并按 8.5.3 的顺序求值及物化。产物中的数组节点保存完整元素类型、结果类型与既有 typed 操作，下游替换 binder 后复用普通布局、复制、越界和 GC 路径。

### 10.1 值类型元素的内存保证

当 `T` 是值类型（struct / enum / tuple / 基本类型）时，`Array<T>` 与 `MutableArray<T>` 保证：

- 元素**不装箱**；
- 元素在内存中**连续布局**（在满足 pack/align 约束的前提下）。

当 `T` 是引用类型时，数组存储引用。

当`T`是ZST时，`Array<T>`/`MutableArray<T>`使用专门的zero-sized element storage：对象仍保存普通ref identity与精确`size: Long`，元素区起点仍按`alignOf<T>()`对齐，但任意长度都不分配element payload bytes。所有literal/assembly/spread输入仍按源码顺序求值并计算逻辑元素数。`get`先按普通调用规则求值receiver与index，再检查`0 <= index < size`并返回该exact ZST值；`set`先按普通调用规则依次求值receiver、index与RHS，随后执行bounds check，成功时不写物理字节。因而即使index越界，RHS的副作用或异常也不能因ZST被跳过。不同index表示不同逻辑元素，但不承诺不同物理地址；现有`addressOf`不能用于array元素。

ZST array的iterator必须保存array ref与`Long` index，以`index < size`终止并按1递增；不得用element pointer是否到达end判断进度。`Array`/`MutableArray`互转和clone仍分配新的array对象并保留size，所以结果ref identity与源不同，但不执行payload `memcpy`。物理分配大小恰为对齐后的元素区起点，与size无关；size仍须位于数学区间`0..=INT64_MAX`，因此不能用“分配字节数很小”绕过长度、assembly求和或迭代index的overflow检查，也不要求core新增整数边界companion常量。literal与spread/vararg assembly从非负元素/component count经checked求和得到size；clone/互转验证source exact array TypeDescriptor、side metadata与`0 <= source.size <= INT64_MAX`一致，再读取source size。M26 的按长度构造另遵守 10.6；即使 T 是 ZST，也必须实际调用每个索引的 initializer，保留全部副作用与异常。

### 10.2 数组字面量

```
val a = [1, 2, 3]                       // Array<Int>
val m: MutableArray<Int> = [1, 2, 3]    // MutableArray<Int>
```

- `[v1, v2, ...]` 是数组字面量；它具体构造 `Array` 还是 `MutableArray` 由**上下文推导**（类型标注、参数类型等）。
- 当上下文同时允许二者（例如无显式类型标注）时，使用 `Array`。
- 空数组字面量 `[]` 必须有显式的类型上下文，否则编译错误。

### 10.3 数组字面量的类型推导与检查

- **无显式类型上下文**（如 `val v = [v1, v2, v3]`）：
  - 若任意元素是值类型，则其余所有元素的类型必须与之**完全相同**，否则是编译错误（值类型元素之间不做隐式向上合流，以保证 10.1 的内存布局保证成立）。
  - 若所有元素都是引用类型，则元素类型取全部元素类型的**最小上界（LOB）**，数组类型为 `Array<LOB>`；当前类型系统没有交叉类型，若存在多个互不可比较的最小共同上界，则 LOB 取 `Any`。当 LOB 为 `Any` 时编译器应给出警告（属实现细节，可延后实现）。
- **有显式类型上下文**：每个元素的类型必须是上下文元素类型的子类型。

```
interface I
class A(val f: Int) : I

val arrayOfA = [A(10), A(20)]        // Array<A>
val i: I = A(10)                     // 引用类型 auto upcast
val arrayOfI: Array<I> = [A(10), i]  // 每个元素都是 I 的子类型
```

- **值类型元素不参与 auto-boxing**（auto-boxing 仅限 O(1) 场景，见 4.4.4；数组字面量不是 O(1) 场景）：

```
struct S(val f: Int) : I

val j: I = S(10)                         // O(1) 场景，auto-boxing
// val bad: Array<I> = [j, S(10)]        // 编译错误：数组字面量中不 auto-box
val good: Array<I> = [j, S(10) as I]     // 显式装箱
```

### 10.4 不变性与转换

- `Array<T>`与`MutableArray<T>`遵守3.2的统一nominal不变性：即使`T` is-a `S`，`Array<T>`与`Array<S>`、`MutableArray<T>`与`MutableArray<S>`之间也没有subtyping。这保证每个exact application始终拥有确定的element layout，并使只读/只写API通过generic callable的bound表达，而不是通过容器projection表达。
- `Array<T>` 与 `MutableArray<T>` 之间**没有父子类型关系**，互转必须显式进行：
  - `m.toArray(): Array<T>`、`a.toMutableArray(): MutableArray<T>`；
  - 或以对方为参数的构造函数：`Array(m)`、`MutableArray(a)`。
- 转换构造的唯一必需参数名为 `source`，类型分别为 `MutableArray<T>` 和 `Array<T>`。显式类型实参、`_`、期望结果类型及固定 application 的 typealias 按普通构造规则确定 `T`；转换不改变元素类型，也不接受相同数组种类作为来源。普通名称查找取得实际数组声明后，转换候选与同层普通函数一起进行 8.6 的重载选择；导入别名及跨 Cone 使用保持同一规则。
- 互转总是分配新对象并复制logical `size`，结果是与原数组相互独立的快照：之后对原数组的修改不影响转换结果，反之亦然。非ZST `Inline`元素复制完整inline payload（实现可用`memcpy`）；`ZeroSized`元素没有payload，不调用`memcpy`，但这不允许复用原对象或丢失size。
  - 不能像 Rust 那样转交（move）内存块：Scoop 没有 move 语义，转交意味着清空原 `MutableArray`，与引用语义冲突。
  - 也不能仅改写对象头复用原存储：在 LLVM + GC 的实现中，每个引用类型对象的头中带有 TypeDescriptor（类似 vpointer），就地改写它会破坏 GC 的状态，因此转换必须分配新对象并复制数据。

### 10.5 操作

- 下标访问 `a[i]`通过普通成员`operator fun get(index: Long): T`；`MutableArray`通过`operator fun set(index: Long, value: T): Unit`支持下标赋值`m[i] = v`。这些声明可以由intrinsic提供表示级实现，但候选选择、泛型实例化与operator identity遵守9.3，不建立按`Array`类型名放行的第二套解析规则。
- `vararg T`的spread和普通形参`Array<T>`都要求exact `Array<T>`；需要改变element type时，调用方显式逐元素构造/转换目标array。
- `size: Long` 属性；M26 起通过普通 `List<T>` conformance 继承 `Iterable<T>`，可用于 `for` 循环。`Array<T>` 和 `MutableArray<T>` 都只实现 `List<T>`，不实现要求可增删元素的 `MutableList<T>`（11.10）。`MutableArray` 自身仍提供 `set`；定长数组不提供以运行期失败代替实现的 `add` / `removeAt`。

### 10.6 按长度初始化（M26）

M26 增加两个构造形式，均返回固定长度的新数组：

```text
Array<T>(size: Long, init: (Long) -> T)
MutableArray<T>(size: Long, init: (Long) -> T)
```

- `size`、`init` 是公开参数名；显式实参、named argument、尾随 lambda、泛型推导及 alias 沿普通构造与 8.5.3 的求值规则处理，与 10.4 的 `source` 转换候选共同参与重载选择。
- 先各求值一次全部实参，再检查 `size >= 0`。负数抛 `IllegalArgumentException`，不调用 initializer。长度为零仍求值 `init` 表达式，但不调用它。
- initializer 是 ordinary 函数值，依次以 `0L` 到 `size - 1L` 调用，每个索引恰好一次；支持分配与抛异常，不支持在 initializer 内挂起。返回值必须可赋给 exact `T`，使用普通函数返回规则。
- 只有全部元素成功初始化后，完整数组才能成为构造结果；initializer 不接收正在构造的数组。第 i 次调用抛出时传播该异常，不调用后续索引，不返回半初始化数组，也不回滚已经发生的外部副作用。
- 每个可被普通代码访问的元素都必须是合法的 `T`。未填充的内部存储不是可读的 `T`，不能把清零当作任意 `T` 的默认构造。初始化期间的 GC 规则见 runtime spec 2.4。
- 对象大小的乘加、对齐与目标地址范围均沿现有 checked allocation 规则；溢出、对象过大或资源耗尽沿现有 fatal allocation failure，不发生整数 wrapping。
- 此 API 不引入数组原地 resize、公开未初始化数组或原始元素指针；`ArrayList` 通过替换自己持有的数组实现增长。

---

## 11. 最小核心库

核心库只包含核心语法运行所必需的类型与函数。命名空间为 **`scoop.core`**（默认导入 `scoop.core.*`，以及 `scoop.core.Option.*`，见 7.2）。

**核心库中的类除单独标明外均为 `final`**，不可继承（包括 `Array` / `MutableArray` / `ArrayList` / `String` / `StringBuilder` 等）。

### 11.1 类型层级根

- `Any`：所有类型的根。**没有任何成员方法**（见 3.1 与 11.11）。
- `Nothing`：所有类型的子类型，无实例。这一条固定完整语言及其core/runtime metadata的未来契约；源码可命名的canonical实体、完整类型系统行为与一般jump expression由后续子集一并落地，M22不要求现行sysroot已提供可解析的`Nothing`声明，也不为jump statement提前物化该类型。

### 11.2 基本类型

均为值类型（struct 语义）：

- `Boolean`；
- 八种整数表示：8/16/32/64位二进制补码signed/unsigned标量。八个canonical源码声明分别是`Int8`/`Int16`/`Int`/`Long`与`UInt8`/`UInt16`/`UInt`/`ULong`；它们是八个不同的nominal type；
- 固定宽度与Kotlin风格名称通过3.2.1的透明alias对应：`Byte ≡ Int8`、`Short ≡ Int16`、`Int32 ≡ Int`、`Int64 ≡ Long`，以及`UByte ≡ UInt8`、`UShort ≡ UInt16`、`UInt32 ≡ UInt`、`UInt64 ≡ ULong`。`Int`/`UInt`在所有target上永久固定32位，`Long`/`ULong`永久固定64位；等价拼写不产生overload、RTTI、layout、mangling或ABI差异；
- 浮点：`Float`（f32）/ `Double`（f64）；
- `Char`。

core中的对应声明是`public typealias Byte = Int8`、`public typealias Short = Int16`、`public typealias Int32 = Int`、`public typealias Int64 = Long`、`public typealias UByte = UInt8`、`public typealias UShort = UInt16`、`public typealias UInt32 = UInt`与`public typealias UInt64 = ULong`。当前语言不定义platform-native integer；本版本中要求保留64位数值范围的已有source/core API显式使用`Long`/`ULong`，这不把二者定义为target-native type。

整数literal接受十进制、`0b`/`0B`二进制、`0x`/`0X`十六进制及位于两个有效数字之间的`_`，不接受八进制。后缀规则为：无后缀候选域是可精确表示magnitude的signed整数，无其他约束时按`Int` → `Long`选择第一个可表示值的默认类型；`u/U`候选域是unsigned整数，无其他约束时按`UInt` → `ULong`选择；`l/L`精确固定为`Long`；`uL/UL`及大小写组合精确固定为`ULong`。radix前缀后必须有数字，separator不能位于首尾或紧邻前缀/后缀。各进制literal先表示非负数学magnitude，不按bit pattern自动重解释；超过数学值`2^64 - 1`的magnitude非法。

literal在8.6 winner commit前持有candidate-local可表示type集合。exact expected同符号族integer type可在值可表示时直接提交；该能力也适用于call/operator receiver，不建立整数type间的一般subtyping或conversion。无后缀不能适配unsigned，带`u`不能适配signed；`L`/`UL`已分别具有exact `Long`/`ULong`类型，不参与其他integer expected-type fit。AST上的`unaryMinus`直接作用于无`u`literal时，二者作为完整负数学值检查边界，使每种signed `MIN`可表示；括号不形成语义节点，空白/注释不改变该关系。`-1u`则是普通unsigned `unaryMinus`。默认阶梯产生的exact commit在其他8.6规则后优于同族非默认fit；多个非默认fit仍歧义。

core整数的二元算术、逐bit运算和比较要求两个已定型operand为同一canonical type；literal可按上段直接提交。shift例外地要求左operand/result保持该integer type、count为canonical `Long`。不同width或signedness的非literal不隐式提升，必须显式转换。对宽度W：

- `+`、`-`、`*`、一元`-`、`inc`/`dec`及bit操作按`2^W`wrapping；一元`+`是同类型identity。signed结果按W位二进制补码解释，overflow不抛异常；
- `/`向零截断，`%`余数与被除数同号；除数为0抛`ArithmeticException`。signed `MIN / -1 == MIN`且`MIN % -1 == 0`，不能继承后端poison/trap；跨 Cone 运算使用前端解析的实际异常声明及零参数 constructor 或其默认参数适配入口，core 的声明可以位于普通依赖中；
- `and`/`or`/`xor`/`inv`逐bit工作。`shl`/`shr`/`ushr`的count为`Long`，有效count取其低`log2(W)`位；signed `shr`为算术右移，signed `ushr`和unsigned右移为逻辑右移；
- signed/unsigned比较分别使用数学有符号/无符号次序；`compareTo`统一返回canonical `Long`的`-1L/0L/1L`；
- const evaluator与运行期使用完全相同的width、wrapping、division和shift语义；const除零是定义错误，普通表达式仍按运行期异常执行。

每种integer提供到八种表示的显式`toInt8`/`toInt16`/`toInt32`/`toInt64`与`toUInt8`/`toUInt16`/`toUInt32`/`toUInt64`，并可提供alias拼写的转发名称。转换total且不抛异常：数学源值先模`2^targetWidth`，再按目标signedness解释bit pattern。每个进入已实现语言子集的基本类型必须同时提供同类型值相等、`ToString`与`Hash` core实现（11.11）；这些实现按owner的完整位宽工作，不经过装箱或`Any`分派。窄signed/unsigned owner的字符串化与hash可分别先无损扩展为`Long`/`ULong`并复用64位core fallback；`Long`/`ULong`的完整输入不得先截断为`Int`/`UInt`。

八个canonical integer struct及`Byte`/`Short`/`Int32`/`Int64`/`UByte`/`UShort`/`UInt32`/`UInt64` alias都在core显式声明`public`，用户可调用成员同样显式`public`。`and`/`or`/`xor`/`shl`/`shr`是普通`infix` member，`inv()`是普通零参数member；signed类型另提供infix `ushr`，unsigned的`shr`已经是逻辑右移且不另设`ushr`。这些bit名称不带`operator` modifier，不增加9.3.1的operator约定。

每个kind的representation intrinsic surface固定包括`unaryPlus`/`unaryMinus`/`inc`/`dec`、`plus`/`minus`/`times`/`div`/`rem`/`compareTo`/`equals`、上述bit members及到八个kind的转换；unsigned同样提供wrapping `unaryMinus`，所以`-1u`有定义。除`compareTo: Long`、`equals: Boolean`和shift count `Long`外，operand/result均为owner exact type。range members是普通core body，不属于该intrinsic集合。

除`div`/`rem`外，上述integer representation intrinsic均不分配、不抛异常，源码声明必须显式带`@NoGC`并登记为NoGc call target；`div`/`rem`因除零可能构造`ArithmeticException`，不得带`@NoGC`且登记为Managed。普通`toString`仍可分配并经普通typed Scoop-ABI core helper工作，不属于integer intrinsic operation集合。

#### 11.2.1 `Char`（M26）

`Char` 是具有独立 nominal identity 的 intrinsic struct，表示一个 Unicode 标量值：U+0000..U+D7FF 或 U+E000..U+10FFFF。它不是 UTF-8 byte、UTF-16 code unit 或用户感知的 grapheme cluster。内存表示为 32 位无符号 scalar，GC-free；没有普通字段或公开 primary constructor，也不与 Int/UInt 隐式互转。

- 单引号字符字面量在转义后必须恰好含一个标量值，例如 `'A'`、`'雪'`、`'😀'`、`'\u{1F600}'`；空、多标量、未闭合或含物理换行的字符字面量非法。转义规则与第 6 章一致，surrogate pair 不作为两次转义合成为一个字符。
- `Char.code: Int` 返回标量编号；普通 core extension `Int.toChar(): Char` 检查该编号属于上述域，否则抛 `IllegalArgumentException`。其他整数先显式转换为 Int；该转换仍遵守已有整数截断规则。
- Char 提供同类型 `equals`、`compareTo: Long`，以及 `ToString` 和 `Hash`；相等和次序按标量编号，`toString()` 编码为一个标量的 String，`hash()` 返回编号对应的 Long。没有隐式数字算术或 CharRange。
- Char 字面量和同类型 const 引用可以用于 `const val`、默认值与递归 literal pattern；Char 普通方法调用不因此扩大既有 const-call 集合。模式覆盖遵守第 5 章的标量值有限域。
- C ABI 使用 `uint32_t` 承载 Char，外部实现仍须保证入站值是合法标量；其余 by-value、array、aggregate、Option、boxing 与 FFI 布局沿各自既有规则。Char 的源码身份不能以同宽 UInt 代替。

### 11.3 `Unit`

0 元 tuple 的类型名与值构造器（见 4.3）。

### 11.4 `String`

String 是 immutable 引用类型，内部持有合法 UTF-8 编码的 Unicode 标量序列，允许 U+0000，不隐含结尾 NUL。M26 将以下表面作为首次实现的字符操作契约；已有 `+`、相等、比较与 hash 保持按内容工作。

| API | 语义 |
| --- | --- |
| `length: Long` | Unicode 标量值数量 |
| `byteLength: Long` | UTF-8 字节数 |
| `operator get(index: Long): Char` | 第 index 个标量值 |
| `slice(start: Long, endExclusive: Long): String` | 按标量索引取半开区间 |
| `iterator(): Iterator<Char>` | 按标量顺序迭代，普通 `Iterable<Char>` conformance |
| `toCharArray(): MutableArray<Char>` | 独立、可写的字符数组快照 |
| `String.fromChars(chars: List<Char>): String` | 普通 companion 方法；将字符序列编码为独立 String |
| `toByteArray(): MutableArray<Byte>` | safe 的 UTF-8 字节快照，Byte 按原始八位 bit pattern 保存 |
| `String.fromUtf8Unchecked(bytes: List<Byte>): String` | 标注 `@Unsafe` 的 companion 方法；复制调用方保证合法的 UTF-8 字节序列 |

`get` 要求 `0 <= index < length`，`slice` 要求 `0 <= start <= endExclusive <= length`，否则抛 `IndexOutOfBoundsException`。`slice(length, length)` 合法并返回空串。索引不接受隐式数值转换；所有上述计数与边界直接使用 Long。`"A雪😀".length == 3L`，其 byteLength 为 8L。组合字符分别计数，不执行 Unicode normalization，因此 `"e\u0301"` 与 `"\u00E9"` 不相等且长度不同。

现有内联 UTF-8 表示下 byteLength 为 O(1)，length 与索引定位需要扫描，按顺序 iterator 使用 byte cursor，遍历总计 O(byteLength)。String 不缓存逐字符索引表，也不因只读文本序列而增加 `List<Char>` conformance；普通 `Iterable<Char>` 已满足 for。

从 List 构造时先在 managed core 中按 11.10.3 的规则取得完整元素快照，再交给编码/复制后备，用户 getter 的异常不穿越 native frame。所有字符/字节数组互转都复制存储，后续修改源或结果数组不会修改 String。`fromUtf8Unchecked` 的前置条件是本次读取形成的完整序列为严格合法 UTF-8；违反是 unsafe 契约违例，不承诺可捕获异常，也不在每次后续 String 操作时重复验证。输出字节的 `toByteArray` 本身不要求 unsafe。带验证的通用字节解码及 ByteBuffer 留给后续设计。

内容相等的成员 `operator fun equals(other: String): Boolean` 和 Hash 基于 UTF-8 内容；compareTo 按标量字典序（合法 UTF-8 的字节字典序给出相同结果）；`toString()` 返回自身。这些能力不来自 Any 或 TypeDescriptor 缺省槽。结果内容与源容器独立，但不要求空串、完整 slice 或其他相同不可变 String 具有不同引用身份。

### 11.5 `Option<T>`

```
enum Option<T> {
    Some(T),
    None
}
```

见第7章。`Option`与全部nominal generic一样保持invariant；niche表示只属于每个exact `Option<T>`。`scoop.core.Option.*`默认引入。

### 11.6 `StringBuilder`

字符串插值（第 6 章）的脱糖目标。M26 以普通 core class 实现，内部使用 11.10 的 `ArrayList<String>` 保存 parts：

```
public class StringBuilder {
    public fun add(part: String): StringBuilder
    public fun <T : ToString> add(part: T): StringBuilder
    public fun build(): String
}
```

公开零参数构造创建空 builder，也可由用户代码直接使用。两种 add 都追加到当前末尾并返回同一 builder；generic add 在本次调用中恰好执行一次 `part.toString()`，完成后才追加结果，不保存原始对象或延迟转换。转换失败不追加该 part，已发生的用户副作用不回滚；转换期间对同一 builder 的重入修改按普通调用顺序生效，不能预先缓存旧 size/backing。

build 返回当前所有 parts 按顺序连接的 String 内容快照，空 builder 返回空串；它不清空、关闭或冻结 builder。重复 build 的内容相同，后续 add 不影响既有结果，不承诺结果引用身份不同。parts 不以固定槽数限制追加次数；增长、总 UTF-8 字节数与最终分配大小只受 10.6 的真实表示/分配边界限制。

parts 与 backing 均为普通 GC 对象。add 只追加 String 引用；build 对有效前缀求和字节数并分配最终 String，再复制每个 part 的字节，复杂度 O(partCount + totalByteLength)。不通过循环 String `+` 反复复制前缀，不需要 ByteBuffer、off-heap owner、release hook 或 close。具体拼接边界见 runtime spec 第 6 章与 M26 设计。

### 11.7 异常

- `Throwable`（引用类型，可被 `throw` / `catch`）及其最小子类：
  - `Exception(message: String?)`：通用异常基类；
  - `UnwrapException`：`!!` 失败时抛出（见 7.3）；
  - `ClassCastException`：`as` 失败时抛出；
  - `ArithmeticException`：整数除零等算术错误；
  - `MissingContextException(message: String?)`：final 异常，contextual declaration 入口缺少精确类型 binding 时抛出（8.3）；
  - `IndexOutOfBoundsException`：数组下标越界（见 10.5）；
  - `IllegalArgumentException(message: String? = Some("illegal argument"))`：实参值违反普通core API的运行期前置条件；11.8的非正range step使用该异常；
  - `IllegalStateException(message: String? = Some("illegal state"))`：运行期状态协议被破坏；默认参数保持既有零实参调用，M21 initialization cycle使用显式message报告稳定unit path。
- `try` / `catch` / `finally` / `throw` 语法与 Kotlin 一致。多个 `catch` 按声明顺序匹配；前一个 `catch` 的类型是后一个的父类型（含相等）时，后者不可达，是编译错误。
- `throw` 与 `catch` 的类型必须是 `Throwable` 的子类型。未捕获的异常导致进程终止（默认行为：打印异常类型名后 abort）。

跨 Cone 的 `throw`、语句及表达式形式的 `catch` 使用前端已解析的实际 `Throwable` 声明，沿共有依赖类型查询取得继承关系并执行同一子类型规则；导入协议不允许跳过检查。同名普通 class 不能替代该实体。异常构造、默认参数、调用、布局、TD 和展开使用共有路径，保留调用求值顺序、catch 顺序与 finally 语义；此能力不增加协议资格、独立证明、wire 字段或 runtime ABI。 公开存储属性的 getter 与可公开调用的 setter 必须按实际 owner 和声明类型生成普通 callable body，即使 provider 的正文没有引用该属性；名义类型、顶层属性与 core 使用同一规则。参数自由且可物化的属性自动导出实际 body。泛型或 source-only owner 仅用于签名和表示查询时，不自动实例化其普通方法或 accessor；实际调用仍通过同一 typed 请求生成完整实例，dispatch 所需的方法继续随其实际类型物化。

跨 Cone 的强制 `as` 沿实际 `ClassCastException` 声明查询完整异常类型，并选择该声明的零参数 constructor 或默认参数适配入口。成功检查保留原对象身份，失败通过普通 class 分配、外部 initializer 调用和 throw 执行；别名、interface 与装箱值沿同一类型与 ABI 路径处理。默认参数中的转换在实际展开时进入相同路径，未求值模板不触发机器物化。异常类的表示仍依赖后续泛型能力时，前端在输出 LocalConcrete HIR 前对实际执行点给出已有 layout-required 诊断；不输出缺少所需表示的成功 IR。MIR 直接消费完整协议中的 typed 声明引用，不再建立丢弃外来声明信息的 Core/Imported 资格投影。所选 constructor 使用共有依赖 callable 集合，保留真实逻辑/物理签名、GC effect、ABI 和 relocation，不增加转换调用的证明记录、wire 字段或 runtime ABI。零参数默认值 adapter 以已有 GeneratedCallable 实体及 ClassInitializer 表示进入共有 callable 导出，不能只发布协议引用而遗漏定义。MIR/LIR selected callable 记录本身是机器依赖引用；reader 按 provider、typed target、定义、签名和传递依赖检查其完整性，不要求隐式调用另附一份 HIR 操作资格记录。

共有 HIR 类型位置的结构、foreign nominal 分发、真实 provider 与定义/求值位置在 HIR reader 边界检查一次。后续物化查询和 MIR/LIR 消费同一未变化的 typed 记录，不重新完整检查这组 HIR 关系，不建立额外验证状态或凭证；实际类型表示、签名、ABI、对象与传递引用仍由相应边界检查。外部字节重新读入或相关数据发生变化时重新验证受影响部分。这一职责调整不改变 wire、内容 fingerprint 的字段组成或 runtime C ABI。

类型生产器直接返回完整的共有 type section；MIR 使用已有继承边、槽序、typed 实现目标及成员引用。类型物化和构造器选择查询已有共有 nominal/callable 声明，不另行投影来源 foundation、完整 protected 声明、构造器签名或参数协议来重复证明同次产出。删除这些副本的生产工厂、凭证外层、只用于副本的 reader 及测试。必要的类型、引用、继承、ABI 与格式检查保留在实际消费边界，未变化的数据复用已有检查结果。

所有可见性的 nominal、callable、property、参数、默认值和定义环境由共有源码接口完整保存。protected 成员仍以 typed 引用参与实际 MIR callable 选择，其可见性和签名读取同一声明；构造器使用共有 nominal 的 constructor 引用及对应 callable，不在 inheritance record 再保存一份 payload。generic 词法 owner 不因访问域查询而要求 machine exact type。名义类型的 lookup/inheritance/slot 三份派生域不再保存和重验。

`CrossConeTypeSemanticsSectionV1` 保留 field 1、2、3、8，field 4～7 退役；`NominalInheritanceInterfaceV1` 保留 field 1～4、7～9，field 5、6 退役，退役字段不复用。成员引用的 Constructor tag 2 随重复构造器通道退役，实际 constructor 始终使用共有 typed 声明。HIR `cross-cone-type-semantics/11`、required inventory、profile 与内容 fingerprint 同步更新，旧产物和缓存需重建；不保留旧来源副本的双轨兼容，不改变 runtime C 调用约定或 String 表示。

M23-7 将访问域计算保留在 HIR lowering；产物保存原声明 visibility、typed nominal owner、slot identity 与实际实现引用，不重复持久化访问域或在 reader 重放 protected/override 的可见性语义。`InheritanceSlotContractV1` 的重复 domain field 5 与 owner field 2 退役且不复用，保留 field 1、3、4、6、7；`InheritanceSlotTargetV1` 的重复 owner field 2 同样退役，保留 field 1、3、4、5。普通宿主的封闭泛型父类型使用原声明与完整 receiver application 查询继承和槽，签名 receiver 直接提供宿主及完整实参；HIR `cross-cone-type-semantics/11` 与 required inventory、profile、fingerprint 同步，旧产物和缓存需重建。必要的声明归属、引用、签名、effect、abstract target modality 和实际继承路径检查继续保留；不改变 runtime C ABI。

M23-7 的实际泛型存储将该 section 升至 `/9`，把已具体化 application 的类型事实接入原表；MIR 类型表示与 LIR 布局当前分别使用 `cross-cone-type-bridge/6` 和 `cross-cone-layout-abi/5`。普通字段与异常字段可以持有同一实际泛型表示，外部 initializer 的完整物理签名进入 MIR 类型登记；格式与阶段职责见实现规范 2.13，runtime C ABI 保持。

- generic class可以继承`Throwable`；其每个exact application都是不同异常类型并拥有不同TypeDescriptor。`catch (e: Error<Int>)`只接收该exact application及普通派生class，`catch (e: Throwable)`仍可接收全部application；不存在`Error<*>`式通配catch。

### 11.8 迭代与区间

迭代操作、解构、挂起性、GC effect 与循环跳转的语言规则由前端在解析实际声明时检查。`for` 随后展开为普通的 typed 调用、装箱／引用转换、Option 操作和 `while`；泛型具体化只代换这些既有节点，不重新执行名称、协议、默认值或局部数据流检查。当前 Cone 与依赖 core 产物按同一规则使用实际 `Iterator`、`next` 和 Option 角色，不以同名用户声明代替协议身份。正常产物消费仍检查编码和实际引用。

`for` 循环、区间表达式的最小支撑：

```
public interface Iterator<T> {
    public fun next(): Option<T>
}

public interface Iterable<T> {
    public operator fun iterator(): Iterator<T>
}
```

`next()`直接返回`Option<T>`：有元素返回`Some(v)`，耗尽返回`None`。`Iterator<T>`与`Iterable<T>`都是invariant exact application；所有public modifier显式写出，sysroot不绕过9.1.5。

对`for (pattern in source)`：

1. `source`先求值且只求值一次；
2. 按9.3普通operator规则选择唯一零参数`iterator()`并调用一次；其返回协议不参与挑选另一个低优先级operator；
3. winner返回type的完整base/interface/bound闭包在去重相同diamond路径后必须恰好到达一个exact `Iterator<T>` application；零个或多个不同application均为编译错误；
4. iterator值按该conformance物化一次，每轮经core `Iterator<T>.next` exact interface target调用一次；用户同名`next`、`Some`或`None`不参与；
5. `Some(value)`建立该轮新的只读scope，以4.6的irrefutable binding pattern绑定后执行body；`None`正常退出。普通body末尾与`continue`都进入下一次`next()`，`break`退出循环。

概念展开为（名称仅作示意）：

```
val __it: Iterator<T> = source.iterator()
while (true) {
    when (__it.next()) {
        Some(__value) -> {
            val pattern = __value
            /* body */
        }
        None -> break
    }
}
```

展开实际使用hygienic temporary、typed loop/interface/variant identity，不进行源码名称查找。`iterator()`若为suspend，只能在允许挂起的上下文选择，可在首轮前挂起但仍只调用一次；core `next()`固定ordinary。每轮binding都是新值，closure捕获对应轮次，不共享一个反复覆写的隐藏`var`。Array/MutableArray必须以普通public conformance实现`Iterable<T>`。

integer range有四个canonical nominal type：`public final class IntRange : Iterable<Int>`、`public final class LongRange : Iterable<Long>`、`public final class UIntRange : Iterable<UInt>`与`public final class ULongRange : Iterable<ULong>`。四者是不同的nominal type，`LongRange`/`ULongRange`不再是alias。constructor与表示属性为core-internal，外部代码不能直接构造不满足step/方向不变量的实例；类型、分配、构造与成员本身仍遵守普通class规则，不是intrinsic或runtime opaque type。`CharRange` 不进入 M26，留待后续单独定义。

`Int8`/`Int16`/`Int`的四个range member返回`IntRange`，`Long`的四个成员返回`LongRange`；`UInt8`/`UInt16`/`UInt`返回`UIntRange`，`ULong`返回`ULongRange`。对这八个canonical integer owner分别令`O`为owner自己的exact type、`R`为上述结果type；每个owner必须按以下schema逐一声明四个普通、非generic、非suspend member，参数名`endpoint`属于可被named argument观察的public API：

```text
public operator fun rangeTo(endpoint: O): R
public operator fun rangeUntil(endpoint: O): R
public infix fun until(endpoint: O): R
public infix fun downTo(endpoint: O): R
```

每个range type `R` 都精确提供`public override operator fun iterator(): Iterator<E>`、`public infix fun step(value: E): R`与`public operator fun contains(value: E): Boolean`：`IntRange`的`E`/`R`为`Int`/`IntRange`，`LongRange`为`Long`/`LongRange`，`UIntRange`为`UInt`/`UIntRange`，`ULongRange`为`ULong`/`ULongRange`。它们都是member而非extension。range对象及其表示属性不可变；每次成功调用`rangeTo`、`rangeUntil`、`until`、`downTo`或`step`都构造fresh range对象，所以`step`结果与receiver不是同一引用。`step`结果保留first、方向与开/闭端点语义。每次`iterator()`都返回一个从first开始的全新cursor，不同cursor的进度彼此独立；cursor一旦耗尽，之后每次`next()`都返回`None`。Array/MutableArray同样以`public override operator fun iterator(): Iterator<T>`实现Iterable；internal array iterator的index固定为`Long`，`next`以显式`public override`满足slot，其effective domain仍受owner限制。

`Int8`/`Int16`的endpoint在已选core函数体内显式、无损扩展为`Int`，`UInt8`/`UInt16`同理扩展为`UInt`，所以窄range元素分别是`Int`/`UInt`。`Long`/`ULong`端点不截断且分别保留在`LongRange`/`ULongRange`中。这不建立一般隐式conversion，两个已定型且类型不同的endpoint仍不能混用。

- `a..b`从a以1升序并包含b，`a > b`为空；
- `a..<b`与`a until b`从a升序且不包含b，`a >= b`为空；
- `a downTo b`从a以1降序并包含b，`a < b`为空；
- `IntRange`/`LongRange`/`UIntRange`/`ULongRange` 的`step n`分别要求`n: Int > 0`、`n: Long > 0L`、`n: UInt > 0u`与`n: ULong > 0uL`，否则求值时抛`IllegalArgumentException`。它保留原方向、端点包含性与first，只替换步长绝对值；
- `contains`的value参数与该range的element type精确相同，只在值位于端点范围且与first的step对齐时为true；实现差值/对齐判断时不能让源码整数overflow改变结果；
- iterator先判断下一步是否越过endpoint或发生机器溢出，再更新current；不得依赖wrapping sentinel，因而MIN/MAX端点也必须正确终止。

range、iterator、`until`/`downTo`/`step`/`contains`均为普通public core声明，除9.3 operator展开和上述for协议外没有按type name识别的编译器旁路。`..`与rest pattern的消歧见4.6。

### 11.9 协程原语

支撑 `suspend` 语义的最小 core 形态如下：

```
interface Continuation<T> {
    fun resume(value: T)
    fun resumeWithException(exception: Throwable)
}

interface SuspendTask<T> {
    suspend fun run(): T
}

interface SuspendRegistration<T> {
    fun register(continuation: Continuation<T>)
}

fun <T> startCoroutine(task: SuspendTask<T>, completion: Continuation<T>)

fun <T> startCoroutine(
    task: suspend () -> T,
    completion: Continuation<T>
)

suspend fun <T> suspendCoroutine(
    registration: SuspendRegistration<T>
): T

suspend fun <T> suspendCoroutine(
    registration: (Continuation<T>) -> Unit
): T
```

- core 协程接口必须具有上述成员名称与签名；接口内的方法声明顺序不属于协议要求。编译器保留所选实际声明与派发槽，重新构建 core 后的消费者使用该产物保存的关系。
- `startCoroutine` 是最小协程构建器：启动 `task.run()` 后立即返回 `Unit`。若 task 在启动调用内完成，则返回前调用 `completion.resume(value)` 或 `completion.resumeWithException(exception)`；若 task 挂起，则在最终完成时调用。completion 恰好收到一次完成通知。
- M27 起，`startCoroutine` 的两种入口均按 8.3.5 fork 当前有效 Context，task 与 completion 在 child 中执行，离开本次驱动后恢复 caller/resumer 的 Context。direct suspend 调用及 `suspendCoroutine` 的 registration 本身不 fork；只是投递同步完成 payload 的 resume 不取得另一份驱动权。
- `suspendCoroutine` 调用 `registration.register(continuation)`。registration 可以同步恢复 continuation，也可以保存它并在 `register` 返回后恢复；前者使 `suspendCoroutine` 在当前调用栈内继续，后者使其真正挂起。`register` 在尚未完成 continuation 时抛出的异常等价于 `suspendCoroutine` 在调用点抛出该异常。
- `register` 已同步完成 continuation 后又抛出属于状态协议错误，`suspendCoroutine` 以 `IllegalStateException` 失败；该 continuation 随即失效，之后不能再次成功完成。
- `SuspendTask` / `SuspendRegistration` 是不依赖 lambda 与函数引用的最小协议。函数类型 overload 由 core 中的普通 Scoop 适配器包装为这两个 interface 后调用同一底层原语，不另建 continuation 状态机；两种入口具有完全相同的同步完成、真实挂起、异常与单次完成语义。
- `launch`、`async`、dispatcher 等高层 API 可以在这些原语上由标准库提供，但不得改变 8.2 的单次完成与异常语义。
- core 原语不提供队列、线程切换或事件循环；调度器与取消不属于核心库。

### 11.10 数组与 List（M26）

`Array<T>` 与 `MutableArray<T>` 仍遵守第 10 章的固定长度与快照转换语义。M26 增加以下普通 core 声明，支持 `StringBuilder` 的 parts 存储，也可由用户代码直接使用；所有类型参数均遵守 3.2 的不变性。

#### 11.10.1 `List<T>` 与 `MutableList<T>`

```scoop
public interface List<T> : Iterable<T> {
    public val size: Long
    public operator fun get(index: Long): T
}

public interface MutableList<T> : List<T> {
    public operator fun set(index: Long, value: T): Unit
    public fun add(value: T): Unit
    public fun add(index: Long, value: T): Unit
    public fun removeAt(index: Long): T
    public fun clear(): Unit
}
```

`List` 是只读访问接口，不承诺对象或元素不可变，也不产生快照。`ArrayList<T> <: MutableList<T> <: List<T> <: Iterable<T>`；以同一对象建立的 `List<T>` alias 会观察到其他 alias 的修改。`List<Derived>` 不是 `List<Base>` 的子类型，所有转换仍须满足 exact application 规则。数组字面量继续构造 `Array` 或 `MutableArray`，不会因这组接口改为构造 `ArrayList`。

`get` / `set` / `removeAt` 要求 `0 <= index < size`，按索引插入的 `add(index, value)` 要求 `0 <= index <= size`；失败抛 `IndexOutOfBoundsException`。普通 receiver 与实参先按源码顺序求值，因此越界操作也不能跳过 value 的副作用。追加保持顺序；插入后原后缀右移；`removeAt` 返回被移除的值并使原后缀左移；`clear` 使 size 变为零。`set` 不改变 size。所有操作都支持 `T` 本身是 `Option<U>`，`None` 是合法元素，不表示列表中没有该位置。

这组无 bound 的接口不隐含任意 `T` 的相等、哈希或字符串化能力；M26 不添加依赖这些能力的 `contains`、按值 `remove`、列表结构相等或 `ToString`。按索引的读取、增删及顺序迭代均有完整实现。

#### 11.10.2 `ArrayList<T>`

`public final class ArrayList<T> : MutableList<T>` 是普通可实例化 generic class，具有公开构造形式 `ArrayList<T>(initialCapacity: Long = 0L)`。负 initialCapacity 抛 `IllegalArgumentException`；初始 size 总为零，预留容量不产生列表元素。所有接口成员显式以 public override 实现，size 只读。容量及 backing array 不属于公开表面。

实现使用 GC 堆上的 `MutableArray<Option<T>>` 和 `Long` 有效长度。有效前缀为 `Some(value)`，其余槽为 `None`；`T = Option<U>` 时列表中的 `None` 保存为 `Some(None)`。增长创建更大的数组、复制有效前缀后替换 backing，数组本身保持定长。移除和 clear 必须清空不再使用的槽，避免继续保活被移除的引用。值类型直接内联在 Option payload 中，不因容器或 exact `List<T>` 分派而装箱；Option 的 tag/padding 可能使 stride 大于 `sizeOf<T>()`，不保证与裸 T 数组完全同布局。

索引读写为 O(1)，追加摊还 O(1)，插入/删除为 O(size)，clear 为 O(size)。几何增长比例、初始实际分配时点与空闲容量回收属于实现策略；不得给列表设置任意固定元素数上限。逻辑 size 不超过 `INT64_MAX`，容量求和及实际分配沿 10.6 的溢出/失败规则。操作不对用户实参求值的副作用提供事务回滚，也不提供并发同步。

#### 11.10.3 快照与迭代

core 提供普通 generic extension `fun <T> List<T>.toArray(): Array<T>` 和 `fun <T> List<T>.toMutableArray(): MutableArray<T>`。调用时读取一次 size，按 `0L` 到 `size - 1L` 依次读取元素并创建独立数组；原列表和结果容器的后续修改互不影响，元素本身按值或引用浅复制。10.4 既有数组转换成员仍优先于 extension，保留其复制语义。自定义 List 的 getter 副作用、异常沿普通调用传播，不承诺并发快照。

core 的 Array、MutableArray 和 ArrayList iterator 持有 owner 引用、`Long` 索引与耗尽标志，不缓存 backing array。每次 `next()` 按调用时的 size 判断是否还有元素，成功读取后索引加一；首次返回 `None` 后永久耗尽。多个 iterator 各有独立位置。串行交错修改有明确的按索引行为：set 可被后续读取观察到；在首次 None 前追加的元素可以被遍历；插入/删除引起的位移可能使某元素重复出现或被跳过；clear 后的下一次读取耗尽。M26 不引入修改版本计数或 fail-fast 异常，iterator 也不提供并发同步。

### 11.11 相等、字符串化与哈希约定

- **`===` / `!==`（引用相等）**：identity 比较，仅适用于引用类型，不可重载（见 4.4.2）。
- **`==` / `!=`（值相等）** 的决议规则：
  - `lhs == rhs`先各求值一次，再只从lhs静态类型收集成员`operator fun equals`候选，按普通成员overload规则选择唯一目标；`lhs != rhs`调用同一目标后对结果取反。不存在交换左右操作数、extension、地址比较、`Any.equals`或TypeDescriptor fallback。
  - **值类型：条件派生的结构相等**——编译器可以额外提供一个参数类型等于lhs完整静态value type的`operator fun equals`候选：例如`Point.equals(other: Point)`，generic template `Box<T>`中则是`Box<T>.equals(other: Box<T>)`，实例化后得到`Box<Int>.equals(other: Box<Int>)`。这些都是普通typed nominal application，不存在`Self`占位符。当且仅当类型的所有字段（元素）**可比较**时生成：字段可比较表示对两个该字段静态类型的值执行`==`能选出唯一目标；基本类型具有核心实现，其他value type递归应用本规则。struct/tuple逐字段按声明顺序短路；enum先比较tag，再只比较active variant payload；Unit恒等。任一字段不可比较时，该派生候选不存在，诊断指出首个失败字段/variant路径。
  - 用户声明参数类型为当前完整宿主application的同签名`equals`时取代派生体；其他参数类型的equals overload不屏蔽该同类型候选。派生方法也是普通成员，遵守value-type`this`按值传递规则。tuple/Unit 的派生候选以 lhs 完整静态类型的有效访问域为准，tuple 保留全部元素类型的可见性约束；生成 helper 的所在文件或首次创建位置不增加源码访问限制。
  - **引用类型**：只使用该class/interface静态类型声明或继承的成员operator equals；不存在时是编译错误。`Any`没有成员，因而`Any == Any`非法；运行期对象另有equals不能补齐静态契约。需要identity比较时显式使用`===`。
  - equals 的决议只考虑成员函数（含编译器派生）；扩展函数不得参与——import不能改变某类型`==`的语义。
- **`ToString`（字符串化）**：接口`interface ToString { fun toString(): String }`。class/object/struct/enum都必须在声明中显式列出该interface并提供合法override；字段或payload实现`ToString`不会让宿主自动获得conformance。generic nominal type若在实现体中调用类型参数值的`toString()`，必须为相应参数声明普通`ToString`上界。tuple与Unit不能声明implements列表，因而不实现`ToString`。String与基础类型由core中的intrinsic nominal声明显式adopt，String实现返回自身。`print` / `println` 定义为`fun <T : ToString> print(v: T)`并经普通单态化bound call实现，不接受`Any` fallback，也不按成员同形或字段结构补齐conformance。
- **`Hash`（哈希）**：接口`interface Hash { fun hash(): Long }`。**没有任何缺省或派生实现**；基本类型与String由核心库提供内容相关实现，其他类型显式opt-in。相等的值必须产生相等hash；不以对象地址作为hash，也不承诺算法跨runtime版本保持相同数值。struct不自动获得哈希。

### 11.12 `SourceLocation` 与位置 intrinsic

```
struct SourceLocation(
    val file: String,
    val line: Long,
    val column: Long,
    val functionName: String,
    val typeName: String        // 不在任何类型内部时为空串
)

@Intrinsic("current_source_location")
fun getCurrentSourceLocation(): SourceLocation
```

- `getCurrentSourceLocation()` 返回所在表达式的标准**求值来源**所指示的源码位置（文件、行、列）与所处函数、类型的名称。该信息在编译期已知，不依赖调试信息；本intrinsic不建立独立的调用处传播机制。
- `file`保存可重现的canonical semantic source path，而不是host绝对路径或CLI operand spelling。manifest Cone中其形式为`group:name:version/src/...`；12.2的single-file Cone中恒为`scoop:single-file:0.0.0/main.scoop`。编译诊断可另行显示当次调用路径，但该display locator不改变`SourceLocation`的值。
- 典型用法是与 8.5 的缺省参数规则组合，在调试信息与诊断设施落地前提供廉价的 runtime diagnostic / tracing 机制：

```
fun trace(msg: String, loc: SourceLocation = getCurrentSourceLocation()) {
    // loc 是调用 trace 的位置，而非本函数内部
}
```

- 作为普通表达式出现在缺省参数template中时，它与template内其他表达式一样按8.5取得定义来源和求值来源；随后普通intrinsic语义读取求值来源，因而返回**最外层调用处**的位置，而不是default机制识别并重写本函数。多层函数转发时，每一层都必须以缺省参数继续转发`loc`（`fun warn(msg: String, loc: SourceLocation = getCurrentSourceLocation()) = trace(msg, loc)`），否则记录的是中间层的位置。
- 内联等优化（见 8.4）不得改变其结果：结果按源码中的调用处确定，与代码生成决策无关。

---

### 11.13 编码、解码与缺省实现（M29）

本节规定2026-10-06修订后的M29目标语义，现有实例编码实现仍待迁移；范围、合成示例和验收见[M29设计](../milestone29/DESIGN.md)及其实施记录。核心库在`scoop.core`提供：

```scoop
public interface Encodable<T> {
    public fun encode(value: T, encoder: Encoder): Unit
}

public interface Decodable<T> {
    public fun decode(decoder: Decoder): T
}

public annotation class SerialName(val name: String)
public annotation class Transient
```

两者都是由companion或普通codec对象实现的普通实例interface，this始终是codec。请求默认实现的声明分别列出对应完整interface application；encode的数据参数和decode的结果由T确定。目标数据类型不会因codec存在获得接口，编译器不自动添加companion或向另一声明转移conformance。两个方向独立，也可由同一codec同时实现；M29不新增内建Codable标记，普通组合interface沿原规则使用。

#### 11.13.1 实现选择与合成条件

- 先按普通override/default规则选择合法的用户实现或继承实现；缺少实现时，才为当前实现类型上的核心requirement合成普通方法。两个方向独立决定，手写一个不影响另一个。错误的显式override仍然报错；无关的合法overload不占用requirement。不生成interface中遍历运行期元数据的共享default body。
- 编译器只识别实际core协议声明的typed identity；用户同名interface或annotation没有特殊行为。用户subinterface可继承完整的`Encodable<T>`或`Decodable<T>`并增加其他requirement；编译器只补齐encode/decode，其他缺失方法仍是正常错误。
- 两个方向的被描述类型都是interface application中的T，不是codec的receiver类型。struct按源码字段处理，enum按variant/payload处理，tuple按元素处理。按11.13.4在定义处为每个参与字段分别确定`Encodable<F>`或`Decodable<F>`值，encode读取value的字段，不读取codec自身的依赖字段作为数据。缺失能力、歧义或不可构造形状报错，不生成运行期失败stub。
- 数据class的自动处理范围仍是普通final class且没有显式class基类。encode读取value所属目标owner的存储property；带自定义accessor或委托的property须显式处理。decode还要求目标有唯一primary constructor，所有参与状态来自其val/var参数；未参与的构造参数须有default，其他存储状态须明确Transient并能按既有规则初始化。字段读取和constructor调用都必须处于codec的正常访问域；合成不创建超出9.1.5的额外权限。
- computed/abstract property不作为存储字段参与派生。无参与字段的普通struct/class编码为record，不能把intrinsic类型当成空record。开放/抽象class、带class基类、singleton或其他不满足上述映射的目标使用手写codec；object可以实现任一方向，但自动构造不会尝试创建新的singleton。将编码移入codec不扩大class自动派生范围。
- 泛型目标如`Box<E>`可包含普通参数E，两个方向分别由显式`Encodable<E>`、`Decodable<E>`依赖提供字段能力，不要求E自身实现接口。companion可使用宿主参数并按9.1.3具体化，但不能等E具体化后再发现其codec。裸T没有可展开目标形状，不能仅凭声明`Encodable<T>`或`Decodable<T>`自动得到body。
- 合法递归类型先建立合成方法签名，再生成body；值布局和单态化终止性继续遵守3.2。已有接口实现选择在定义处完成，具体化不重新选重载、default或字段codec依赖。

核心Boolean、现有定宽整数、String、Char的companion分别实现具体的`Encodable<Scalar>`与`Decodable<Scalar>`；Unit使用普通object UnitEncoder与UnitDecoder。`Option<T>`、`Array<T>`、`MutableArray<T>`、`ArrayList<T>`及tuple的codec按11.13.4组合。标量、Unit、容器和tuple数据值不因此获得编码/解码接口；不再提供条件conformance，容器原有无bound用途保持。

List/MutableList没有唯一默认解码结果。核心库提供普通
`fun <T> encodeList(values: List<T>, element: Encodable<T>, encoder: Encoder): Unit`：
取得一个unkeyed容器，按values的普通迭代次序逐项取得child并调用
`element.encode(value, child)`，最后结束该容器。空列表同样结束容器；元素异常沿普通调用传播。该函数
只遍历逻辑元素，不编码容器的 capacity、backing 或空闲槽。

Any、函数、Ptr/FunPtr没有默认codec。用户可手写`Encodable<List<T>>`、
`Decodable<List<T>>`、`Encodable<Any>`或`Decodable<Any>`等实现；不从运行期类型名推断具体codec。

#### 11.13.2 容器协议与库边界

Encoder/Decoder提供三个入口，均为ordinary、可抛异常的非泛型interface方法：

```scoop
public interface Encoder {
    public val path: String
    public fun keyed(): KeyedEncodingContainer
    public fun unkeyed(): UnkeyedEncodingContainer
    public fun singleValue(): SingleValueEncodingContainer
}

public interface Decoder {
    public val path: String
    public fun keyed(): KeyedDecodingContainer
    public fun unkeyed(): UnkeyedDecodingContainer
    public fun singleValue(): SingleValueDecodingContainer
}

public interface KeyedEncodingContainer {
    public fun field(name: String): Encoder
    public fun end(): Unit
}

public interface KeyedDecodingContainer {
    public val keys: List<String>
    public fun required(name: String): Decoder
    public fun optional(name: String): Option<Decoder>
    public fun end(): Unit
}

public interface UnkeyedEncodingContainer {
    public fun element(): Encoder
    public fun end(): Unit
}

public interface UnkeyedDecodingContainer {
    public val hasNext: Boolean
    public fun element(): Decoder
    public fun end(): Unit
}

public interface SingleValueEncodingContainer {
    public fun writeBoolean(value: Boolean): Unit
    public fun writeLong(value: Long): Unit
    public fun writeULong(value: ULong): Unit
    public fun writeString(value: String): Unit
    public fun writeNull(): Unit
}

public interface SingleValueDecodingContainer {
    public fun readBoolean(): Boolean
    public fun readLong(): Long
    public fun readULong(): ULong
    public fun readString(): String
    public fun readNull(): Unit
}
```

一个Encoder/Decoder代表一个值；为该值选择一种container。每个field/element返回处理其值的子入口，按调用顺序完成子值，再继续下一个；单值container恰好读/写一个标量。容器end检查该层的完整性，根入口检查完整文档消费。数组下标及数量沿现有库使用Long。

path表示输入/输出数据位置，采用从空串根开始的JSON Pointer segment形式：字段名中的`~`、`/`分别转义为`~0`、`~1`，序列使用十进制索引。它是普通String，便于核心codec及合成分支构造带位置的错误，不包含运行期类型描述。核心库的EncodingException/DecodingException是Exception子类，构造参数为`path: String, message: String`，公开只读path；message沿既有Exception的`Option<String>`表示。

`optional(name)`只在key不存在时返回None；格式中的显式null仍返回Some(Decoder)，之后按字段codec处理。`keys`是输入数据中的键列表，不是类型描述。keyed读取不要求输入字段顺序与请求顺序相同；未知key可跳过但仍需是合法格式，重复key必须失败。unkeyed的element在耗尽时失败，end拒绝剩余元素；tuple据此检查长度。

类型相关调用留在合成body及普通泛型helper中：`codec.encode(value, child)`与`codec.decode(child)`；codec分别是满足`Encodable<F>`或`Decodable<F>`的普通值，F是字段的完整静态类型。不要求interface方法级泛型、Any中间树、runtime SerialDescriptor或反射字段访问。JSON、未来的二进制格式等由普通库实现这些interface，编译器只生成相同的类型侧代码。

#### 11.13.3 缺省数据模型与构造

| Scoop类型 | 缺省编码形状 |
| --- | --- |
| Boolean / 整数 / String | 对应单值；有符号/无符号分别经过Long/ULong，窄整数decode检查范围 |
| Char | 恰好一个Unicode scalar的String |
| Unit | 单值null；不借用Scoop的Option表示 |
| struct / 合格class | 按声明序写入字段的keyed record |
| tuple / Array / MutableArray / ArrayList | 按位置或索引写入的unkeyed sequence |
| enum（包括Option） | 恰好一个key的外层record；key为variant名；命名payload为record、位置payload为sequence、unit variant的payload为record且编码时为空 |

`SerialName`只更改record字段或enum variant的wire名称，不更改源码名称、类型identity或字段类型。它不能用于位置payload/tuple、type本身或computed property。`Transient`排除存储字段/property，不适用于type、variant或位置元素；合成decode时该字段必须能从已声明default或正常class初始化取得值。两者不能同时标在同一target。参与同一record的key、同一enum的variant wire名须唯一；检查仅使用该方向实际派生的成员。

编码总是写出参与字段，包括等于default的值；不为了省略字段而求值default或调用equals。解码按构造参数顺序读取并类型化已提供字段，字段没有default时必须存在，包括Option字段；有default时仅记录缺失。完成container检查后，按参数声明序对缺失项执行已在定义处绑定的default，允许引用前面的参数，并且每次构造至多执行一次。输入显式null、错误类型或越界值不能触发default。Transient参数也按该顺序取default，最后调用选定的正常primary/variant constructor。命名字段的输入顺序不改变这些规则。

外层“字段是否存在”的Option与字段本身的Option值是两层独立值。缺省Option编码按普通enum保留None/Some区分；不自动展平为nullable单值。例如JSON中None为`{"None":{}}`，Some(None)为`{"Some":[{"None":{}}]}`。更紧凑的nullable协议由显式codec定义。

enum解码检查外层恰好一个key，未知variant失败；构造选中的variant，不使用内存tag、niche或ordinal作为wire tag。class解码先取得全部构造实参，再执行普通分配及完整初始化，不分配后逐字段反射填充、不调用setter修补半成品。失败沿普通异常/GC规则清理，不返回部分对象。

默认语义编码树形值：共享引用可展开为多份值，decode不保留原对象identity。循环图、跨对象引用及开放多态的discriminator由显式codec定义，不属于自动派生数据模型。

M29以普通库的`Json.encode<T>(value: T, codec: Encodable<T>): String`与`Json.decode<T>(text: String, codec: Decodable<T>): T`完成可运行闭环。调用如`Json.encode(value, User.Companion)`与`Json.decode(text, User.Companion)`，T按普通实参推断；两个入口都要求显式codec，没有从T或运行时数据类型隐式寻找codec的重载。JSON实现负责语法、Unicode/转义、重复key、数值范围和完整输入消费；整数不经过Double转换，整数字段不接受带小数部分或指数部分的数字token。无Float/Double或Map的新增承诺；record已可覆盖对象形式，array覆盖序列。格式错误抛带数据路径的EncodingException/DecodingException；用户codec、default和constructor抛出的普通异常照常传播，不改写为default或空值。

#### 11.13.4 编码、解码依赖与泛型组合

合成`Encodable<R>`.encode或`Decodable<R>`.decode时，目标R必须具有已知的struct/enum/tuple或合格class形状，R中的类型参数可以尚未具体化。两个方向分别按以下顺序确定每个参与字段F所需的`Encodable<F>`或`Decodable<F>`值；选择在定义处完成并保留实际声明引用，不在具体化时重新匹配：

1. 当前codec class/struct的primary constructor中，以val保存且静态类型满足所需完整interface application的显式依赖。没有匹配才继续；多个匹配诊断歧义，不按参数名称或顺序任选。companion没有constructor，不扫描其任意property、Context或整个词法作用域寻找依赖。
2. F与目标R相同则使用this，支持当前方向的递归调用。
3. F是可明确命名的名义类型，且它的可见普通companion实现了所需的`Encodable<F>`或`Decodable<F>`，则使用该值。F为`Envelope<E>`等完整application时保留宿主实参；不向F的基类companion回退，仅有同名encode/decode/encoder/decoder方法也不构成conformance。
4. F为核心Option/Array/MutableArray/ArrayList时，递归取得相应元素codec并调用完整宿主companion上预定义的普通encoder/decoder方法，例如`Array<E>.Companion.encoder(element)`。Unit使用UnitEncoder/UnitDecoder，tuple按元素递归组合普通codec。其余缺失情况报错，要求显式注入或手写body；不能在实例化时猜测裸类型参数的companion。

同一字段类型可以复用同一依赖，不同字段需要不同策略时手写相应方法。两个不同binder在具体化后恰好相同不触发重新选择或新的歧义；body继续使用定义处选定的参数。generic用户类型不按名约定自动调用companion.encoder/decoder；调用方显式组合其codec并传入需要它的provider。

```scoop
public struct Box<T>(val value: T) {
    public companion object {
        public fun encoder(element: Encodable<T>): Encodable<Box<T>> =
            BoxEncoder(element)

        public fun decoder(element: Decodable<T>): Decodable<Box<T>> =
            BoxDecoder(element)
    }
}

public class BoxEncoder<E>(private val element: Encodable<E>) : Encodable<Box<E>>
public class BoxDecoder<E>(private val element: Decodable<E>) : Decodable<Box<E>>
```

两个helper缺少的方法分别合成：BoxEncoder从数据实参读取value字段并调用this.element.encode；BoxDecoder调用this.element.decode后正常构造`Box<E>`。入口可写`Json.encode(box, Box<Long>.Companion.encoder(Long.Companion))`或`Json.decode(text, Box<Long>.Companion.decoder(Long.Companion))`，也可使用普通companion转发。方法中的T来自宿主，helper的E由构造实参推断；encoder/decoder方法没有另行声明的类型参数。元素codec属于本次返回的对象，不写入companion字段；同一个数据类型可使用多个编码或解码策略，数据本身无需编码bound，其他用户声明的泛型约束保持原义。

Option/Array/MutableArray/ArrayList的encoder方法接收`Encodable<T>`并返回相应容器的编码器，decoder方法接收`Decodable<T>`并返回解码器。两个方向都由普通持有依赖的对象实现，数组只处理逻辑元素，Option保留enum形状。tuple没有companion，可由普通object/class/struct显式实现完整的tuple codec接口请求合成；自动组合tuple字段时，普通闭包分别由核心EncodeFunction或DecodeFunction适配，不增加tuple metatype或按arity命名的源码类型。

```scoop
public class EncodeFunction<T>(private val body: (T, Encoder) -> Unit) : Encodable<T> {
    public override fun encode(value: T, encoder: Encoder): Unit = body(value, encoder)
}

public class DecodeFunction<T>(private val body: (Decoder) -> T) : Decodable<T> {
    public override fun decode(decoder: Decoder): T = body(decoder)
}
```

这些codec、闭包及依赖都是普通typed值，按现有调用、分派、初始化和GC规则执行。encode按声明/元素顺序读取数据、取得child，再求值已选codec并调用；每个数据读取与child取得只执行一次。decode先取得实际存在的字段child，再求值codec；缺失的可选字段、Transient字段及未选中的variant不求值字段codec。组合在相应方法执行时进行，不提前成新的singleton初始化链。调用方显式传入的codec仍按原实参求值规则处理。没有全局codec registry、隐式witness参数或对运行期类型描述的查询。

---

## 12. Cone模块、package与库

M23按`docs/milestone23/DESIGN.md`第0.1/10节拆为M23-1…M23-11实现；本章仍规定全部子里程碑完成后的单一最终语义，不把迁移期的功能子集写成第二套语言规范。

### 12.1 基本概念

- **Cone是module、分发、依赖、编译与静态链接的基本单位；package只是源码namespace。** Cone不等于package，二者不能由名称或目录互相推导。一个Cone可以包含多个package，同一package也可以由多个Cone贡献声明；后一情形称为split package，不会把不同origin的实体合并为同一identity；
- manifest source Cone是包含`Cone.toml`与固定`src/`目录的独立目录；`scoop build/run <file.scoop>`则把指定的唯一文件构造为12.2的synthetic executable source Cone。binary Cone是该语义单元编译得到的`.slib` artifact。面向构建的`scoop`可以将manifest Cone的上游依赖定位到source Cone或已经验证的binary Cone，二者必须声明同一Cone identity；低层single-Cone compiler `scoopc`只消费已生成并显式传入的上游`.slib`，不跟随source locator或递归构建其他Cone；
- 文件路径不决定package。source file至多声明一个`package` header，省略时属于root package；跨package引用必须按12.4导入或使用合法qualified path；
- package/FQN只参与名称组织与诊断，不是type、callable、property或其他实体的全局identity。相同package/name来自不同Cone时保持不同typed origin，并在同一lookup层相遇时按12.4诊断，而不是按依赖或链接顺序任选一个。

### 12.2 Cone identity与`Cone.toml`

每个Cone具有canonical coordinate `group:name:version`。`group`与`name`各由一个或多个`.`分隔的lowercase ASCII segment组成，每段精确匹配`[a-z][a-z0-9-]*`；`version`是canonical SemVer 2.0.0文本，禁止多余`v`与非法前导零，合法pre-release/build metadata原样参与identity。不做trim、大小写折叠、Unicode归一化或路径别名解析，不满足grammar直接拒绝。M23所有CBOR-based wire identity统一使用`WireCborV1`（M23设计4.1冻结的RFC 8949 deterministic CBOR子集）和如下hash framing，不能把任意“canonical”encoder、Rust/C内存表示或分隔符拼接混入公式：

```text
ByteSpan(bytes) = little_endian_u64(bytes.len) || bytes
DomainSeparatedCborHash(domain, value) =
    SHA-256(ByteSpan(ASCII(domain)) || WireCborV1(value))

ConeIdentity =
    DomainSeparatedCborHash("scoop-cone-id-v1", ConeCoordinate)
```

长度转换必须checked到`u64`；domain不带NUL。在`DomainSeparatedCborHash`及其他直接拼接CBOR item的公式中，`WireCborV1(value)`已是唯一、自定界item，不再隐式套`ByteSpan`；仅当公式明确写`raw(id)`时，typed 32-byte id直接拼入，其他可变长raw片段一律使用`ByteSpan`。若专门公式显式写出`ByteSpan(WireCborV1(...))`则以该公式为准，12.5的`ArtifactFingerprint`即是这种有意的外层framing。runtime可重算的callable-body与runtime registration key使用另一套`RuntimeEncode`（little-endian `u32/u64`、显式count、product按声明序、sum以little-endian `u32` tag开头），不得用Wire CBOR替代。manifest同时保存canonical coordinate record与digest，reader必须重算。version属于identity，因此两个版本即使源码相同也不是同一声明/type origin。artifact内容另有`ArtifactFingerprint`，不能用它、package、FQN、manifest路径或session-local provider编号代替`ConeIdentity`。

manifest-backed生产source Cone的最小manifest schema为：

```toml
schema = 1

[cone]
group = "dev.example"
name = "my-lib"
version = "0.1.0"
kind = "library" # 或 "executable"

[dependencies]
"org.foo:bar" = { version = "1.2.3", path = "../bar" }
"org.acme:util" = { version = "2.0.0", artifact = "../artifacts/util.slib" }
"org.other:log" = "3.1.0"
```

- `schema`以及`[cone]`中的`group`、`name`、`version`、`kind`必需；`kind`只能是`library`或`executable`。拼错或未知的semantic field不能被静默忽略；
- dependency key是exact `group:name`，value必须给出exact version；字符串短式只省略locator。table value至多给一个`path` source-Cone locator或`artifact` `.slib` locator；二者都省略时，`scoop`在每个显式artifact search root下检查`<group>/<name>/<version>/cone.slib`，三项都使用canonical文本且`.`不拆目录。所有存在的候选必须具有相同完整`ArtifactFingerprint`，否则是ambiguous artifact错误；search-root顺序不能决定选择不同内容；
- locator只用于当前构建查找，相对路径以当前manifest为基准；它不进入Cone identity、实体identity、`.slib` metadata、初始化顺序或源码诊断identity。locator解析到的source/artifact coordinate必须与dependency key/version canonical相等；
- M23没有version range、`latest`、optional/dev/build dependency、feature、platform条件、dependency alias或manifest提供的native link option。M23的`scoop`也只把exact locator解析为12.3的resolved graph，不执行版本选择或冲突调停；`scoopc`仅验证当前manifest的semantic projection与命令行显式提供的binary dependency closure，不解析任何locator；
- `scoop build [root-input]` 与 `scoop run [root-input]` 允许省略 root：此时明确使用调用者当前目录的 `Cone.toml`，与显式传入该文件相同；缺失或无效时报告输入错误，不向父目录搜索，也不猜测某个 `.scoop` 文件。显式 root 继续按目录、`Cone.toml` 或单文件规则分类；低层 `scoopc build` 仍要求明确输入；
- `scoop:single-file:0.0.0`是另一reserved coordinate，用户manifest不得声明它。`scoop build/run`收到basename扩展名精确为`.scoop`、跟随symlink后目标为已存在regular file的operand时，构造`kind = executable`、source set恰好为该文件、logical source path恒为`main.scoop`、direct Cone dependency恰好为trusted core的typed synthetic projection；symlink cycle、dangling link或最终目标非regular file是输入错误，resolved host path不进入identity。显式文件operand即使位于某Cone目录中也不读取相邻`Cone.toml`、其他`.scoop`、C/C++ source或blob；它不接受其他Cone dependency。该`.slib`可缓存，并可作为产生它的build/run或显式`scoop link --root-slib`的唯一executable root；但不能作为可分发artifact发布、作为dependency或被manifest artifact locator引用；
- manifest Cone的source identity是`(ConeIdentity, normalized Cone-relative src path)`；single-file source使用相同pair形态，但第一项固定为由reserved `scoop:single-file:0.0.0`计算的`ConeIdentity`，第二项固定为`main.scoop`。host绝对路径、CLI relative/absolute/symlink spelling、inode、mtime、目录枚举顺序与临时输出路径不进入语义identity；只有语言允许跨文件同名的file-private/hidden实体才把该source identity加入其declaration key。single-file artifact/cache key另外包含source content digest、core semantic/code fingerprints、compiler/schema/target/toolchain，不因共用reserved identity而碰撞；

### 12.3 静态exact依赖图

- M23固定三层工具边界：umbrella binary `scoop`负责root-input分流、locator、resolved DAG、cache与调度；`scoopc`每次只编译一个当前Cone并产生该Cone的`.slib`；program-link是只消费已验证artifact的独立stage。`scoop build`和`scoop run`共用这条完整pipeline；`run`只在build/program-link成功后执行binary，不是另一种编译或解释模式。三层不能用共享的未持久AST/IR或隐式进程状态绕过`.slib`边界；
- M23-11 的公开命令、默认输出、诊断与观察输出见实现规范 2.7 及 [阶段设计](../milestone23/stage11/DESIGN.md)。`run` 的 `--` 后参数原样传给进程，不改变源码 `main(): Unit` 的入口合同或编译缓存；cwd、environment、stdio 和 exit/signal 保持普通进程语义。显式 `scoop link` 只消费 root／dependency `.slib`、runtime 对象索引及已有 native 文件，不构建缺失的源码依赖。stage dump 和本次诊断展示路径不是语言 IR 或产物身份；
- `build/run` 的构建 profile 仅有 `debug` 与 `release`，默认 `debug`；`--profile` 显式选择，`--release` 为 release 简写。默认输出目录为 `target/<canonical-target-triple>/<profile>`，`--target-dir` 可替换 `target` 根；manifest 模式以 Cone root 为基准，single-file 模式以调用者 cwd 为基准。两种 profile 当前沿用相同编译设置，优化与调试信息策略延后实现；profile 名称与目录不改变语言语义、实体 identity 或 runtime ABI，不能与既有 target／artifact profile 混同；
- M23只接受最终链接前已经完整解析的静态Cone图。依赖边必须无环；同一resolved graph中同一`group:name`只能出现一个version，同一`ConeIdentity`只能对应一组一致的semantic fingerprints。cycle、多个version、同identity不同artifact或dependency coordinate不匹配都是构建错误；
- `executable`不能成为另一个Cone的dependency。一次程序构建恰有一个executable root，其余节点都是library；library单独构建时不需要executable root；
- core可从任意普通manifest目录作为`scoop`构建根；当前根已经定义core时不读取默认sysroot、不加载另一份core，也不注入self edge。graph、源码快照、缓存和产物返回均使用普通Manifest library节点。
- `scoop`为除core自身外且未显式声明core的每个Cone注入12.6的core direct dependency；core artifact使用普通依赖输入与一致性检查，sysroot仅提供默认locator。除该边外不存在隐式dependency；
- 完整显式依赖发现结束仍未定位 core 时，`scoop build/run` 优先使用默认 sysroot 的 `lib/scoop.core` 源码目录；仅当该目录不存在时，读取 `artifacts/<canonical-target-id>/scoop.core.slib`。存在但无效的源码目录、manifest 或选中的产物均报告对应错误，不回退到另一来源。两种来源分别沿普通 source／prebuilt 节点处理，构建不向 sysroot 写入产物；直接 `scoopc` 与显式 `scoop link` 仍只消费已存在的 core 产物。
- single-file resolved graph恰好由trusted core与唯一synthetic executable root组成，不运行manifest locator发现；其源码对非core Cone的import按普通不可达诊断。这不禁止`@Extern`产生的逻辑native library requirement；该requirement只能由`scoop`/program-link经显式library search root解析，不是Cone dependency，也不能用无typed来源的raw object/archive输入替代；
- dependency path、manifest枚举与输入顺序不影响结果。canonical topological order使用dependency-first的Kahn顺序，并在每个ready set按`(group UTF-8 bytes, name UTF-8 bytes, canonical version)`取最小者；
- 对 manifest root，`scoop`先只读解析 source manifest 与 prebuilt `.slib` 的 manifest summary；single-file root 使用 12.2 的固定语义记录。编译 child 启动前先验证完整构建图，再按依赖顺序处理节点。父进程检查产物的 envelope/hash、profile、Cone/target、依赖 fingerprint、缓存键和 child 结果，保留不可变归档与摘要，不为调度或缓存重放完整语义与对象。`scoopc` 或实际 Link 消费者在读取外部产物时取得完整 HIR/MIR/LIR 与对象并完成对应检查；同一消费中的 Compile/Link 复用结果。source cache miss 在全部上游就绪后调用一次 `scoopc`；源码依赖变化时重编译，无源码可重建时报告 stale dependency，不能把旧 typed identity 接到新 metadata；
- 每个`.slib`记录编译时direct dependency的`ConeIdentity`及HIR/MIR/LIR semantic fingerprint。编译manifest-backed Cone时，`scoopc`必须显式获得当前manifest声明的全部direct `.slib`及它们递归引用的其余transitive support `.slib`；direct/support角色由typed manifest edge验证，不从命令行顺序或路径推断。single-file请求不接受额外Cone依赖，其core artifact可由普通构建输入指定，sysroot仅提供默认路径。缺失、额外不可达、重复identity、同identity不同fingerprint、stale edge、cycle、自环、同一`group:name`多version、executable dependency或target/backend/schema/ABI不兼容都必须在parse当前源码前失败；`scoopc`只验证调用者给出的封闭artifact graph，不搜索、不修复且不重编译任何上游节点；
- runtime在执行任何managed initializer前登记整个图的image、stackmap、TypeDescriptor、static storage/root、immortal object与initialization-unit metadata。eager top-level initialization按上述Cone顺序执行，每个Cone内再按9.1.3的`PersistentInitializationUnitId` bytes排序；object/companion及generic delegated extension等lazy unit只登记、不进入eager loop。直接读取另一个unit仍先ensure目标，可以使该目标早于其普通排序位置执行；
- `scoopc`为typed metadata support加载完整transitive artifact closure，独立program-link stage为最终链接另行验证并加载同一闭包，但这不使间接依赖自动成为源码候选。源码可到达性只由12.4的current Cone、direct dependency surface、re-export和core prelude决定；
- library root以完成的`.slib`为最终产物，父进程核对归档与构建结果。executable root同样先完整产生自身`.slib`，实际 program-link 消费时读取并检查完整依赖与对象。`scoop`的registry必须原子解析不可部分构造的`ResolvedTargetProfile { lir_target: LirTargetProfile, backend: ValidatedBackendProfile, c_bridge_toolchain: ValidatedCBridgeToolchainProfile, runtime_build: ValidatedRuntimeBuildProfile, final_link: ValidatedFinalLinkProfile }`，并证明五个projection的triple、object format、deployment、calling convention、pointer/storage ABI、compiler output与link input相容；不得先读取host默认值再补齐。完整product只由driver后续编排持有，不写入M23-2 `.slib`或下发给不需要它的stage。M23-2只构造并持久化前两项组成的`ValidatedLirTargetSelection`，不能伪造完整profile；`lir-lower`只接收`lir_target`，Scoop codegen接收`lir_target + backend`，generated-C producer接收`lir_target + c_bridge_toolchain`，runtime-build接收`lir_target + c_bridge_toolchain + runtime_build`，program-link接收`lir_target + final_link`及已验证的其他stage产物。后三项由后续 required capability 描述，分别进入 Code、RuntimeArtifact 与 ResolvedLinkPlan fingerprint；runtime 对象变化不反向改变已经发布的 per-Cone RuntimeImage。之后 runtime-build 按实际源码、toolchain 和 build rules 产生普通对象与符号/ABI 摘要，缓存按这些输入及内容失效，不建立来源资格或不可伪造包装。后续 program-link 消费共有 reader 的完整 Link 数据、这些 runtime 对象、精确 link projection 与输出路径；已验证且未变化的数据直接复用，外部对象输入保留格式、引用与 ABI 检查。最后一次`scoopc`不顺带生成program descriptor、runtime输入或最终binary；
- M23不提供registry下载、版本求解、lockfile、shared-library ABI、`dlopen`/`dlclose`、hot reload、image卸载或运行期新增Cone。

### 12.4 `package`、`import`、re-export与编译单元

#### 12.4.1 文件头语法

每个source file的顺序固定为：

```text
packageHeader? importHeader* declaration*

packageHeader  = package QualifiedName
importHeader   = public? import ImportSelector (as Identifier)?
ImportSelector = QualifiedName | QualifiedName . *
```

- `package`至多一次且必须先于所有import/declaration；import只允许出现在文件头。exact import可写`as`，alias只改变当前文件中的短binding名；star import不能写alias；
- 普通import只影响当前source file。`public import`同时建立当前文件的普通exact/star import，并在**当前文件package**下为当前Cone建立re-export binding；其destination name是`as` alias或target短名；
- `public`在这里是上下文关键字，只修饰import；不存在`internal import`或`private import`；
- import selector与qualified type path都先解析最长的可见package binding前缀，再沿static nested nominal、object或companion的typed owner edge查找；最长package前缀一旦选定便不回退到较短前缀重猜。exact import的终点必须是importable binding，star import的终点必须是importable namespace；不能把点连接的字符串直接当作FQN扫描全部artifact。M23的top-level value/function表达式仍通过import后的短名或普通receiver语法访问，不新增dependency-coordinate-qualified源码名称。
- 限定类型路径的 package 前缀同时考虑当前 Cone 与 direct dependency 的公开包（含 re-export）。同一最长包中的当前与外来类型在同层合并，按 typed origin 去重并诊断不同实体的同名冲突；仅含 value binding 的公开包仍是可见包，不能因其中缺少目标类型而回退。仅供模板、布局或链接使用的 support dependency 不增加可见包。路径末端的普通类型、泛型 application 和 typealias 使用与短名查找相同的类型实参、可见性及展开规则。

#### 12.4.2 import可到达性与re-export

普通exact/star import selector只能解析：

1. 当前Cone中按visibility允许当前file访问的声明；
2. manifest direct dependency的public lookup binding；
3. 该direct dependency已经解析并公开的re-export binding；
4. trusted core direct dependency的public/prelude binding。

每个成功解析的 import target 都保存最终 typed 实体 origin，并区分当前 Cone 声明和依赖声明。依赖 target 保存非空、canonical 排序去重的实际终点公开 binding 及其既有 re-export 引用；同一 origin 合并为一个 target，保留实际 binding 的引用集合，不按首次加载路径任选一个。引用按 provider identity、export binding id 及既有 re-export 关系排序，session-local id、路径长度与 artifact 加载顺序不参与判等。

源码 selector 只能从当前 Cone 或 direct dependency 的可见包开始；选中实际名义 owner 后，其 public 静态嵌套 namespace（含 object、companion）继续按 owner edge 查询。原 owner 位于 support provider 时同样适用，终点直接保留该 provider 的真实公开 binding；不把外层类型的名称路径改写成嵌套 target 的路径，也不生成额外访问证明。transitive `.slib` 的包不会因此自动成为源码候选。产物 reader 检查这些公开 binding 的归属、typed target、namespace、role、引用存在及既有 re-export 连续性，复用前端已经完成的源码查找，不再要求终点 binding 的 provider 必须是 direct。

- `public import` 的每个最终 target 必须是依赖中的公开 binding；当前 Cone 声明不能用于 public import/re-export，也不能导出 internal、private 或 protected target。终点可以来自依赖的 re-export，或由已选名义 owner 到达的公开静态 namespace；snapshot 保存实际绑定引用，re-export 可以成链；
- re-export只建立destination package/name到origin typed实体的公开binding，不生成wrapper、forwarder、第二个TypeDescriptor、第二个typealias target、generic body或storage，也不扩大target member的visibility；
- public star在编译当前Cone时展开为逐项、已经解析的API snapshot；下游不重新执行上游的文本glob。target集合变化会改变当前Cone的re-export metadata/fingerprint；
- 同一 typed origin 经重复 import、多个 star 或钻石 re-export 到达同一层时合并为一个 target，并合并排序后的实际 binding 引用。终点相同的静态 owner 遍历不复制外层路径；实际绑定集合或依赖变化按既有规则影响 metadata／fingerprint。不同 origin 即使 package/name/signature 文本相同也不合并：非 overloadable 实体产生歧义；function/extension 可进入同一 overload 层，但展开后签名相同仍是冲突；
- split package合法，但不授予跨Cone可见性。exact selector命中多个不同origin的非overloadable实体时报告包含Cone coordinate的歧义；`as`只作用于已经唯一解析的exact selector，不能靠alias从一个本身歧义的selector中任选；
- 两个local public declaration/re-export在同一destination namespace形成不可重载冲突时，当前Cone本身即为定义错误，不能发布带歧义的`.slib`；
- public API签名通过typed dependency closure引用但未re-export的外部public type仍可供下游类型检查、推导、layout与member检查使用，却不会自动获得可书写的短名。希望只依赖当前Cone的用户直接写出该名称时，API作者应显式`public import`。

#### 12.4.3 名称与调用候选层

无显式receiver的名称/调用候选层从高到低为：

1. 词法binding与local function；
2. 隐含`this`的真实member；
3. 当前文件的exact import，包括alias与`public import`；
4. 当前package中可访问的本地声明与re-export binding；
5. 当前文件的全部star import；
6. core `.slib`提供的typed prelude；
7. 4.2/8.6由唯一expected exact enum application产生的contextual variant fallback。

显式receiver先查真实member；extension scope再按exact import → current package → star import → core prelude分层。property-like `invoke`在每层内继续使用9.3.4的c-level分区，property读写继续使用9.1.1的typed accessor/place规则，constructor、variant、operator、callable reference与typealias qualifier继续进入各自既有typed入口。

对callable层，每层独立执行8.6的shape filter、candidate-local applicability与MSC，只选择第一个至少含一个适用候选的层；某个高层只有不适用同名callable时继续到下一层。callable-value hard shadow等既有词法规则不变。type、object、property name等非overloadable lookup在同层出现多个不同origin时直接歧义。visibility 在 applicability 前按 9.1.5 检查，不产生额外访问凭证；声明顺序、dependency枚举、re-export链长与artifact加载顺序都不能作为tie-break。

跨Cone普通lookup只枚举public lookup surface。public open/abstract owner所需的protected constructor/member、abstract obligation、interface default source与override relation属于独立inheritance/slot surface，只能由前端在合法 subclass/implementation 上下文按实际声明、继承关系和接收者类型检查后使用；generic template的internal/private hidden support也不进入普通import或名称候选。

前端在名称查找、override coverage 和 signature exposure 的负责位置执行访问域检查；成功后直接保留 typed 声明引用、实际继承/override 关系及最终 lookup/slot 域。删除 `LookupAccessWitness`、`OverrideAccessWitness`、`PropertyOverrideAccessWitness`、`SignatureExposureWitness` 及仅携带这些记录的候选资格状态。后续候选物化、IR 与产物生成不复制访问域证明，不用 witness 的有无替代实际声明关系。可见性错误、protected 接收者规则、签名泄露检查和独立 setter 槽规则保持；不改变 wire、profile、runtime C ABI 或 String 表示。

#### 12.4.4 编译单元与entry

- Cone是独立的编译/静态链接单元，不默认按每个`.scoop`文件生成一个可依赖的语言模块。manifest Cone中，`scoopc`只为当前Cone递归收集`src/`下扩展名精确为`.scoop`的regular file，以normalized Cone-relative `/` path的UTF-8 byte order排序；空source set、逃出Cone root的symlink、非UTF-8或归一化后重复的relative path都是构建错误。single-file mode是显式root-input例外：整个synthetic Cone恰好包含指定文件，它不会使每个普通source file获得可分发Cone identity；
- 同一Cone的全部source file一起建立语义环境，文件之间没有编译顺序。声明能否用短名访问仍由package/import与visibility决定，“同一Cone编译”不等于忽略namespace；
- `internal`精确表示origin Cone内可见；默认visibility及其他access domain见9.1.5；
- library不需要entry；其中名为`main`的普通声明不会因此获得entry linkage。executable root必须恰有一个top-level ordinary、non-generic、non-suspend、无参数且返回`Unit`的`main`；只在root Cone中发现，dependency中的`main`不参与竞争，entry可以保持internal；
- 最终程序从program metadata保存的typed root entry调用`main`；源码package或固定native符号名不决定entry。

### 12.5 编译产物 `.slib`

泛型采用单态化（见3.2），上游generic定义必须能在实际使用它的下游Cone完成实例化。因此每个Cone都由single-Cone `scoopc`产生target-specific、版本化且确定性的`.slib`，而不是只有`.o`/`.a`；executable Cone的`.slib`同样先成为完整、可独立验证的artifact，再由program-link和完整依赖闭包产生最终程序。v1 artifact至少包含：

- canonical manifest、Cone identity、exact direct dependency identity/fingerprint、language/runtime、target与backend各自的typed profile id/fingerprint、identity schema、封闭`ManglingSchemaIdentity`与三层wire-schema compatibility信息；其中M23-2 manifest保存的target选择精确称为`ValidatedLirTargetSelection { lir_target, backend }`，不是缺少后三个projection的`ResolvedTargetProfile`；
- 覆盖除唯一bootstrap `manifest.cbor`之外全部payload的canonical typed member directory；每个record包含可重算的`SlibMemberId`、不依赖host path或目录ordinal的typed stable key、`SlibMemberRole`、byte length与SHA-256；物理archive name仅由已排序目录的ordinal派生，不作为record/fingerprint中的第二真源；
- 供下游HIR使用的Export HIR metadata；
- 供下游MIR使用的符号、exact ancestry/conformance、dispatch与external-target metadata；
- 供下游LIR使用的layout、ABI、recursive scan与TypeDescriptor metadata；
- 任意数量的`LinkObject`、诊断附件及由capability标识的任意`ExtensionBlob`，以及re-export/prelude binding index、definition source table、ODR records、target-tagged transitive native link requirements与各层semantic/code fingerprint。

v1使用标准deterministic、self-contained normal `ar`容器，但archive只是字节容器，不是成员语义的第二来源；thin archive、外部文件引用、未由目录声明的symbol/long-name special member均拒绝。唯一固定的bootstrap名是首个成员`manifest.cbor`；其余物理成员严格按`SlibMemberId` bytes排序，名称是从0开始的ordinal按`m`加8位零填充十进制唯一派生，不保留输入basename或扩展名。canonical writer固定使用SysV/GNU short-name header：`!<arch>\n` global magic、`name/`加space的16-byte name、十进制零填space的mtime/uid/gid、八进制`100644` mode、无前导零十进制size、`` `\n`` trailer及奇数payload后的`0x0A` pad；reader拒绝其他等价但非canonical拼法。member/map顺序及非语义字段不得依赖producer host、build目录或临时文件名。

directory使用M23设计4.1冻结的唯一RFC 8949 deterministic CBOR codec。`SlibMemberRecord` map field固定为`1=id, 2=stable_key, 3=role, 4=byte_length, 5=sha256`；`MemberStableKey`和`SlibMemberRole`都是unknown tag即拒绝的v1封闭sum，variant tag精确为`HirMetadata=1, MirMetadata=2, LirMetadata=3, LinkObject=4, DiagnosticAttachment=5, ExtensionBlob=6`。stable key按相同tag分别携带无payload、`{verifier capability, logical key}`或`{capability, logical key}`，并与role capability逐byte配对；logical key为非空的capability-defined canonical opaque bytes，长度按真实输入和表示范围检查。role中HIR/MIR为Compile输入，LIR同时为Compile与Link输入，`LinkObject`为Link输入。member identity使用混合raw/CBOR framing：`SlibMemberId = SHA-256(ByteSpan("scoop-slib-member-v1") || raw(ConeIdentity) || WireCborV1(MemberStableKey))`；固定32-byte Cone id不套`ByteSpan`，CBOR item也不再套，不能改写为三个字符串拼接或`DomainSeparatedCborHash`。

`CapabilityId`固定为`{namespace, name, nonzero u32 major}`：namespace总长1…255 ASCII byte且满足`[a-z][a-z0-9-]{0,62}(\.[a-z][a-z0-9-]{0,62})*`，name总长1…63 byte且满足`[a-z][a-z0-9-]{0,62}`。M23-6 基线的内建 typed id 为`org.scoop-lang.target-profile/darwin-aarch64/1`、`org.scoop-lang.backend-profile/llvm-22-1/1`、`org.scoop-lang.c-bridge-toolchain-profile/darwin-aarch64-apple-clang/1`、`org.scoop-lang.object-format/mach-o-relocatable/1`、`org.scoop-lang.link-object/scoop-lir/1`与`org.scoop-lang.link-object/generated-c-bridge/1`；M23-7 将 Scoop LIR object verifier 升至 `org.scoop-lang.link-object/scoop-lir/3`，其余本段列出的 id 保持。target/backend/C-bridge toolchain/object format虽复用该三字段wire envelope，在类型上不能彼此或与verifier capability互换。manifest `CompatibilityRecordV1`的field 5/6精确为`TargetProfileWireId/TargetProfileFingerprint`，field 7/8为`BackendProfileWireId/BackendProfileFingerprint`；reader只在两对都由registry验证后构造`ValidatedLirTargetSelection { lir_target, backend }`。target profile仅覆盖LIR可观察的layout、Scoop ABI、native symbol normalization及“C边界必须经generated bridge”策略；backend profile仅覆盖Scoop LIR到LLVM/Mach-O的受检codegen。实际C compiler、SDK/deployment、generated-C template/flags、runtime build与最终link action不属于这两个`/1` contract；C-bridge toolchain以独立profile id/fingerprint进入M23-3 production proof，不能伪装成target或backend字段。

两个内建object verifier的logical key都精确为map `1=NonZeroU32 unit_count, 2=Digest256 unit_set_digest`。Scoop LIR digest使用domain `scoop-lir-object-unit-set-v1`与按bytes严格递增、去重、非空的`ObjectDefinitionPlanId` array；generated bridge digest使用domain `scoop-generated-bridge-object-unit-set-v1`与同序的`GeneratedBridgeUnitId` array。`GeneratedBridgeUnitId = DomainSeparatedCborHash("scoop-generated-bridge-unit-v1", GeneratedBridgeUnitKey)`，只标识跨Cone可复用的canonical C source recipe；其封闭key为outbound-function、global read/write/address、`CallbackTrampoline { c_signature: CanonicalCAbiSignatureFingerprint, context_parameter: CallbackParameterIndex }`或`StaticCallbackTrampoline { storage_bridge: PersistentGeneratedCallableId, c_signature: CanonicalCAbiSignatureFingerprint }`。前者的context parameter是zero-based typed `u32`，必须索引该canonical C signature中的`Ptr<Unit>` context槽；该unit不含具体closure、registration、managed adapter、callback body id或producer，因此严格按`(C signature, context index)`复用。静态trampoline按稳定的storage-bridge generated callable identity与C signature复用，不以arena id、symbol或callable body id区分；相同C签名的不同静态callback仍得到不同unit。跨Cone复用unit只表示复用recipe，绝不表示复用managed adapter或link symbol。generated-C模板允许引入的target-native helper必须是绑定target及完整C-bridge toolchain profile的typed requirement；当前闭合集合只含`Memcpy`，且该use只能来自同一production envelope证明的generated-C member，不能由Scoop LIR object或同名symbol冒充。

物理bridge definition另使用`GeneratedBridgeAtomId = DomainSeparatedCborHash("scoop-generated-bridge-atom-v1", GeneratedBridgeAtomKey)`，其中key为`{ producer: ConeIdentity, atom: GeneratedBridgeAtomRoleKey }`；atom role封闭为`PrimaryEntry { unit }`、`SignatureDescriptor { unit, signature }`、`ContextDescriptor { unit, context_index }`与仅作compile-time proof、不得materialize symbol的`StaticAssertSupport { unit, layout }`。每个实际producer Cone对每个所用unit恰好产生自己的`PrimaryEntry` atom，`br` mangled symbol以`GeneratedBridgeAtomId`为owner并固定`ConeStrong`；不同Cone可以共享unit/source recipe，但atom id和symbol必须不同。manifest保存完整unit-to-member及atom materialization relation，reader重算producer、index合法性、count/digest，要求每个unit恰出现一次、每个物理atom命中声明member且集合互斥，并与signature/layout record及support proof闭合。这样单object可含任意多unit而不突破logical-key上限；不能用生成ordinal、first use或临时文件名区分同capability的多个object。

`MemberPurposeSet` 的 v1 bit 为 `Graph=0x1, Compile=0x2, Link=0x4, Diagnostics=0x8`，其他 bit 拒绝。这些字段选择成员的实际消费用途，不是来源或操作资格。Graph 数据描述 identity、依赖、target/backend 与 envelope/hash；Compile 数据提供完整 HIR/MIR/LIR 与跨层 bridge；Link 数据追加非空 `LinkObject`、实际扩展处理结果及 image owner。完整产物可在同一依赖集合中同时提供 Compile 和 Link 数据，共有字段和检查结果复用；仅有 manifest 的摘要不能提供尚未读取的 IR 或对象。Diagnostics 是附加信息，不改变语义数据。发布需要 Compile 和 Link 数据均完整，不对对象数量、文件名、扩展名或 producer 语言增加假设。

`LinkObject`可由Scoop、generated bridge以及未来C/C++或其他已知producer生成；每个member由完整versioned `verifier_capability`选择验证契约。M23没有“任意C object”能力，未来producer必须登记自己的flags/defined/undefined/constructor/EH contract。v1 `ExtensionBlob.required_for`只允许空集或恰好`{Link}`：空集表示非语义opaque blob；Link-required capability必须已知，handler的唯一封闭输出为canonical native-library requirement set。handler不能产出object、raw bytes、argv、host path或linker script；所有可链接object无论来源都必须显式使用`LinkObject` role并经过同一member-aware门禁。Graph/Compile/Diagnostics bit、多bit组合、unknown bit或当前Link purpose所需的unknown capability都拒绝；Graph/Compile reader对unknown Link-only payload只验证envelope/hash并保持opaque。`DiagnosticAttachment`对Graph/Compile/Link固定optional，只能由显式Diagnostics consumer解释。不得把整个`.slib`、任意opaque blob或所有非metadata成员盲传给linker。

每个Cone的全部`LinkObject`联合贡献其Scoop code、native code与runtime metadata，并必须在联合验证后恰好定义一个由该`ConeIdentity`派生的hidden strong `ScoopImageDescriptorV1`。manifest以唯一`image_owner_member: SlibMemberId`指向定义该descriptor的`LinkObject`，但这不对其他object数量或分片方式施加基数约束；其余任何member都不得定义同一或另一Scoop image descriptor。definition range、digest patch、stackmap contribution、linker-visible defined symbol与undefined-symbol use都必须显式携带所属`SlibMemberId`及唯一typed owner/requirement，不得用object-local index、物理名或默认主object补猜。当前Darwin capability还必须验证member为Mach-O `MH_OBJECT`并拒绝`LC_LINKER_OPTION`/autolink等内嵌linker输入；未来C/C++ producer必须另行定义flags、defined/undefined contract、静态构造析构与异常边界，不能仅靠member role获准。

manifest中的mangler格式字段不是裸整数版本，而是封闭schema identity；当前唯一合法值是`ManglingSchemaIdentity::PersistentV1`，canonical manifest spelling为`persistent-v1`，对应`docs/milestone23/DESIGN.md`第3.3节定义的persistent-identity mangler。object member、cache key与artifact必须携带并比较完整identity；unknown identity一律拒绝，不能由linker猜测symbol格式。M23-2以前的compact mangler只是未发布编译器内部实现，必须随pipeline迁移直接删除，不能作为artifact输入或兼容分支保留。

`MemberFingerprint`精确定义为`DomainSeparatedCborHash("scoop-slib-member-content-v1", SlibMemberRecordV1)`；`LinkMemberFingerprint`换用domain `scoop-slib-link-member-v1`但编码同一份五字段record，且只适用于`LinkObject`与Link-required `ExtensionBlob`。capability/purpose已在role中编码一次，不重复附加派生字段。`ArtifactFingerprint`使用另一条明确的raw-framing公式：

```text
ArtifactFingerprint = SHA-256(
    ByteSpan("scoop-artifact-v1")
    || ByteSpan(WireCborV1(ArtifactManifestInputV1))
    || ByteSpan(raw(MemberFingerprint[0]))
       ...
    || ByteSpan(raw(MemberFingerprint[n-1]))
)
```

`ArtifactManifestInputV1`是与完整bootstrap manifest除`artifact_fingerprint`外字段完全相同、但结构上不存在该字段的独立closed product，不是把required field删除或清零后送入普通decoder。member fingerprint按directory的`SlibMemberId`顺序，每个固定32-byte digest仍按统一framing套`ByteSpan`；它们都不含派生物理archive name。writer最后增加fingerprint并重新执行`WireCborV1`编码，reader投影成上述input重算。manifest不记录自身member hash，archive header也不进入该fingerprint，从而不存在自引用。该fingerprint描述完整artifact envelope的一致性，不是发行者签名；optional blob变化会改变它，但不进入HIR/MIR/LIR semantic fingerprint、code fingerprint或最终link key，因此不得使下游重编译或重链接。code fingerprint必须覆盖按`SlibMemberId`排序的全部`LinkObject`、已知Link-required handler的canonical native-library requirement、defined/undefined/native requirement contract，且不使用物理archive name或ordinal作为语义输入。

required schema、language/runtime ABI、identity schema、完整`ManglingSchemaIdentity`以及`ValidatedLirTargetSelection`中的target/backend typed id与fingerprint必须exact compatible。所有view先限制archive/member资源并验证manifest、typed directory、每个payload完整性与hash；Graph随后只验证graph envelope，Compile才执行HIR/MIR/LIR wire decode、structural validation、typed remap、跨层bridge与semantic-world commit，Link才解码LIR verification surface并验证member-local definition/relocation、capability、全部object联合的image-owner唯一性。任何当前view所需步骤失败都丢弃整个artifact；损坏或不兼容artifact不能以名称、扩展名、host默认值或部分可读member继续执行，也不能把较弱view冒充较强view。

M23-6a 起，Export HIR 是已检查语义 HIR 的导出投影：源码产生与产物解码使用同一声明、类型和正文结构及原 typed identity。普通、默认、泛型、构造初始化与词法 callable 的正文共用节点，导出范围仍只包含下游需要的接口和支持闭包。消费者不重新解释 provider 源码，也不把外来声明复制成本地声明；普通外部实现保留定义方身份，实际 generic application 通过同一具体化进入独立的 LocalConcrete HIR。已发生的 wire major 记录只描述相应版本，后续实际变更按同一版本与缓存规则迁移，不能为保持旧格式而保留平行语义实现。

Export HIR metadata逻辑上区分：

1. **public lookup surface**：可供普通跨Cone lookup的显式public声明、public non-generic typealias与resolved re-export binding；
2. **inheritance/slot surface**：public可继承owner需要的protected constructor/member、slot/default/override contract与实际 typed 声明及继承引用；
3. **generic hidden support closure**：公开generic nominal/callable/property中依赖consumer type arguments的template-owned body、lambda/local function、predicate与其他必须在下游具体化的递归typed依赖；
4. **interface dependency closure**：上述表面的signature type、exact ancestry/conformance、annotation、const、default source与well-known core relation；
5. **source interface templates**：M17参数形态及只使用refined export-interface reference的hygienic default template；
6. **binding index**：package/name/namespace到typed root的映射，以及prelude/re-export provenance。

同一实体可以被多个closure引用，但wire definition只有一份并标明用途集合。inheritance-only和hidden-support实体没有普通binding index entry。语言visibility与native linkage是不同概念：只有依赖consumer type arguments的generic/template-owned body随closure交给下游具体化；origin Cone已经发射的param-free private/internal helper只携带typed signature、persistent external target与link requirement，不复制body，也不得在consumer中转成`LocalConcreteHir`重新发射。该helper可供下游specialization链接，但不能因此被import或re-export；default template仍不能携带private/internal hidden dependency closure。public `const val`值、exact definition/evaluation source与line information按其用途进入相应closure/source table。

每类可跨artifact引用的实体都使用kind-specific persistent typed identity；不同kind在内存与wire上都不得通过统一裸digest互相cast。identity是对domain-separated canonical definition key的SHA-256，key至少包含origin `ConeIdentity`、package、typed owner chain、实体kind、源码name与对应duplicate-declaration规则采用的normalized signature；signature使用alias展开后的persistent type refs与binder位置。每条wire identity record同时保存typed id与canonical key供reader重算验证。serialized table index只允许作为wire内压缩索引；reader必须验证并重映射到consumer session-local typed id。FQN、link symbol、arena id与archive顺序都不能作为identity缺失时的回退。re-export保留origin identity；non-generic typealias保留独立alias identity与typed target，使用时透明展开。

M23-2的identity-foundation payload为每类**由该层引入**的persistent identity保存`{ typed id, canonical key }`并由reader重算；同一identity只能由HIR/MIR/LIR中的一个最早声明层拥有，后续层引用它时不得复制声明。HIR还保存target-independent `SourceNativeExternalContractRecord`、callback registration record与一个只服务native-boundary闭包的最小source witness。HIR definition-origin表必须精确覆盖全部源码声明的nominal type，包括origin为reserved core的`String`、primitive与其他普通core源码声明；只有编译器拥有、没有普通源码声明的`CoreBuiltinNominal::{Unit, Any}`不得要求或携带definition origin，不能用`ConeIdentity::CORE`把整组core类型豁免。该witness的`NativeBoundaryTypeDefinitionRecordV1`精确为`{ owner, type_parameter_count, shape }`：owner是`Concrete(PersistentTypeId)`或`GenericTemplate(PersistentGenericTypeId)`；shape是`Reference`、`Struct { NotCLayout | CLayout { aligned, packed }, declaration-order fields { PersistentFieldId, SignatureTypeKey } }`或`Enum { declaration-order variants及其declaration-order fields }`。M23-6 保留完整 intrinsic family，并把 witness 升为必需四字段记录 `{ owner, type_parameter_count, shape, c_abi }`；`c_abi` 以 typed field/variant 引用表达 source representation、UInt64 字段投影或可空 pointer 投影，精确编码与拒绝规则见实现规范 2.11。它必须精确覆盖本artifact的source extern、callback registration/application及target contract引用的exact/signature type之传递source-nominal闭包，额外或缺失record都拒绝。validator以该witness及`ValidatedLirTargetSelection`重算C-FFI-safe、canonical C storage/layout及Scoop value size/alignment/shape/pass category；foundation中的一般layout/scan/dispatch record只有identity key，不能被提升成跨Cone ABI证明。LIR foundation field 1不是第三份exact-type identity声明表，而是按`PersistentExactTypeId`严格递增、无重复编码的runtime-materialized exact-type引用集合；每个引用必须解析到同一artifact已由HIR或MIR声明并完成canonical-key验证的exact type，旧的`{ id, key }`元素格式直接拒绝。若闭包跨direct dependency而当前profile缺少相应proof，M23-2的单artifact Compile必须以稳定的native-boundary-closure-required错误失败；M23-3只允许当前Cone与trusted core即可闭合的子集，M23-6才通过新的required通用layout/ABI/scan section完成跨Cone证明，且不改本段已冻结的extern/callback bytes。

callable declaration、concrete application与machine body是三个不同的kind-specific identity。`PersistentCallableApplicationId = DomainSeparatedCborHash("scoop-callable-application-id-v1", CallableApplicationKey)`，其中key固定为`{ origin, instantiation_owner, callable_arguments }`：origin的tag 1…4分别是普通function、generic function、constructor与property accessor declaration id；instantiation owner是`NoOwner=1 | ExactNominalOwner=2 | EnclosingCallableApplication=3 | EnclosingInitializationApplication=4 { PersistentInitializationUnitId }`；callable arguments是`NoCallableArguments=1 | Arguments=2 { non-empty exact type ids }`。普通method/constructor/accessor位于generic nominal时使用其**声明宿主**exact application及NoCallableArguments，generic method另带自身arguments；top-level generic function使用NoOwner+Arguments；generic extension-property accessor以receiver binder arguments作Arguments；local callable捕获普通外层callable substitution时使用EnclosingCallableApplication，若其lexical parent链穿过generic delegated initializer/ensure（包括其中lambda或anonymous callable），则必须使用指向同一source extension property与完整receiver arguments之`GenericDelegatedExtensionApplication` unit的EnclosingInitializationApplication；自身generic arguments仍独立保留，不能被外层receiver substitution吞并或复制。两项都不存在时不得构造application，直接使用declaration id。全部arity包括phantom binder，argument必须concrete；调用点receiver、动态派生类型、FQN与空vector都不能代替上述typed owner/argument，validator须沿lexical/materialization relation证明tag 3/4确为最近的enclosing root。

initializer/ensure的generated declaration identity与其concrete实现root也必须分层：`GeneratedCallableKey::Initialization { unit, role }`中的unit只接受声明级`TopLevelProperty | ExtensionProperty | Object | Companion`，generic delegated extension仍以声明级`ExtensionProperty` unit产生唯一template；`GenericDelegatedExtensionApplication` unit禁止直接写进template key，只能出现在`CallableMaterializationContext::InitializationApplication`中携带完整receiver substitution。实际initializer/ensure body及其中lambda、anonymous/callable-reference wrapper和named local function的emission root取该materialization context；template owner始终终止于声明级unit。不得为每组receiver arguments复制template，也不得丢掉context后把具体实现错归到声明Cone。

concrete exact type另由`PersistentExactTypeId = DomainSeparatedCborHash("scoop-exact-type-v1", ExactTypeKey)`标识，`ExactTypeKey`是封闭sum：param-free nominal declaration、generic nominal origin加非空exact arguments、非空tuple elements、ordinary/suspend managed function parameters/result、`Ptr<T>`的`RawPointer { pointee }`、以及`FunPtr<F>`的`NativeFunctionPointer { calling convention, parameters, result }`。`Unit`与primitive走core nominal identity；`T?`先脱糖为core `Option<T>`，non-generic alias先展开。`Ptr`不能伪装成普通nominal application，`FunPtr`不能伪装成managed function；M23 native function-pointer convention封闭为C，未来新增convention必须新增stable tag。template binder ref不是exact type，只有完全替换后才产生该id。tuple/managed function的相同有序结构跨Cone具有同一identity；不同结构或pointer provenance即使target layout相同也不能合并。wire按dependency-first保存`{ id, key }`并重算，不能保存session `TypeId`或display string。

runtime TypeDescriptor中的诊断类型名不是源码display spelling，而是从上述已验证key唯一派生的`CanonicalExactTypeDiagnosticName`。其规范ASCII grammar由M23设计3.1冻结：transparent alias先展开；nominal使用canonical origin coordinate、package、typed owner与声明名；application、tuple、ordinary/suspend function、raw data pointer及native function pointer按不同固定tag/delimiter递归打印，除`[A-Za-z0-9._-]`外的每个UTF-8 byte使用唯一大写十六进制percent escape。同一`PersistentExactTypeId`必须产生逐byte相同的UTF-8名字，import/re-export alias、使用点限定路径与consumer pretty-printer都不能改变它；producer/reader通过显式遍历展开实际typed key，并检查非法循环引用及真实分配失败；诊断名不另受逻辑展开成本或文本配额限制。这些bytes进入TypeDescriptor及其diagnostic atom的definition/ODR fingerprint，但runtime不得按该名字判断类型。

前述nominal source atom只适用于源码声明；compiler-generated nominal使用M23设计3.1封闭的generated-role tag与`PersistentTypeId`分支，v1 tag精确为`ClosureEnvironment=1, CallableAdapterEnvironment=2, CoroutineFrame=3, ContinuationAdapterEnvironment=4, CoroutineStep=5, BoxedValue=6, CoroutineSlot=7, ObjectBackingClass=8`；不能伪造源码owner/name或把arena ordinal、临时display name写入诊断身份。

layout、callable body、static storage、immortal object、initialization unit与safepoint site分别使用不同persistent id。M23每个由LIR定义并发射到当前Cone某个已验证`LinkObject`的Scoop callable body使用以下runtime-encoder identity；它不能改写成Wire CBOR hash：

```text
CallableBodyKey = Strong { owner }                         // tag 1
                  | Odr { member: CallableOdrMemberId }      // tag 2
                  | RootGateway { root_cone, main }          // tag 3
                  | InitializationStartupGateway { unit }    // tag 4
PersistentCallableBodyId =
    SHA-256(ByteSpan("scoop-callable-body-v1") || RuntimeEncode(CallableBodyKey))
```

variant tag是little-endian `u32`，product按声明顺序编码。后两个gateway不能冒充其调用的main/ensure。M24把**全部**machine body统一切到`CallableBodyKeyV2`与domain`scoop-callable-body-v2`，保留前四个tag并增加`ReleaseHook { owner: PersistentExactTypeId }=5`，公式仍是`SHA-256(ByteSpan(domain) || RuntimeEncode(key))`；M24 的 HIR/MIR/LIR outer schema 均升为 2，identity-foundation capability major 分别为 HIR 4、MIR 2、LIR 3（承接 M23-7 的 LIR `/2`），完整 `cross-cone-generic` artifact profile 由 M23-8 的 /2 升为 /3；container、persistent identity schema 和 `persistent-v1` mangler 保持 1，prefixed runtime metadata record 在 M24 继承 M23-8 的 ABI 3，M27 按实现规范 2.16 升为 ABI 4 并同步其实际 section/profile。旧M23 artifact整体重建，M24 artifact不得混留body-v1。`PersistentCallableApplicationId`、`OdrGroupId`及以CallableApplication/GeneratedCallable作discriminator的primary member不因body版本改变；body id、其safepoint及以CallableBody/SafepointSite作discriminator的派生member、registration与ODR fingerprint全部重生。

只有声明的native extern、validated runtime artifact函数及由非Scoop LIR producer生成的native body/bridge不属于callable-body集合，它们使用各自typed identity与目录或final-input verifier capability。layout id由exact type、target profile与representation role派生；storage/object id由typed owner declaration或specialization、stable definition path与封闭生成role派生，内容变化进入definition fingerprint而不另造content identity；safepoint site id由callable body id与CFG site role/ordinal派生。TypeDescriptor直接以`PersistentExactTypeId`登记，不另设与exact type竞争的type identity。凡concrete exact type进入LIR layout/type closure或param-free exported LIR bridge就必须runtime-materialize一份TD registration；只存在于未替换Export HIR template/binder中的type尚不materialize。非nominal exact type以exact id建立12.5的`StructuralType` ODR group；nominal exact type及以它为shape owner的box、coroutine step/slot/start一律沿`ExactOwnerRoot`回到source Cone或Nominal specialization，不能为nominal exact type另造`StructuralType`组。最终程序中每个materialized exact type恰有一个TD地址，是否materialize不改变语言type identity。

跨artifact layout wire以封闭`ValueStorageLayout::{ZeroSized { nonzero alignment }, NonZero { nonzero size, nonzero alignment, RefScan }}`表达，不能用裸`size == 0`配可选stride/scan让consumer补猜。ZST exact type的layout/TypeDescriptor仍按persistent identity登记；compiler-owned static ZST storage使用runtime spec 2.8的独立1-byte identity token。未装箱layout与box shape严格分离：前者logical size为0，后者以`BoxedValue.inline_size == 0`及非零minimum allocation表示，不能把0当managed object allocation size。Scoop ABI的`ElidedZst`参数/结果分类、array的zero-sized element storage分支及其layout fingerprint都必须进入LIR metadata和definition fingerprint，保证上下游不会以不同物理ABI解释同一signature。

compiler-owned static storage的跨artifact初值wire也是封闭`StaticInitialState::{ZeroedForRuntimeUnit, EncodedStaticValue { canonical allocation-extent template bytes, sorted typed immortal relocations }}`，不得用可空initializer或raw linker address补猜。前者只能对应9.1.3需要runtime写入的unit/published/failure storage；后者只能对应9.1.3明确可省unit的值。template内容与`{pointer offset, PersistentImmortalObjectId}`序列进入LIR/strong或ODR definition及RuntimeImage canonical key，C指针、ASLR后的storage bits与object-local relocation index不进入key；同一ODR storage的variant、template或typed target不同必须在native coalesce前失败。

本Cone为自身object建立的`LocalConcreteHir`函数体/类型实例只供本Cone MIR消费，不写入`.slib`。下游读取上游Export template后，对实际需要且尚不存在的exact generic application在**当前消费Cone**完成HIR concretization、MIR/LIR、layout与发射；定义Cone只负责预先导出param-free实体及自己已经materialize的application。

program-wide specialization key是区分语义实体种类的封闭sum：

```text
SpecializationKey = Nominal { origin: PersistentGenericTypeId, exact arguments }
                  | Callable { application: CallableApplicationKey }
                  | DelegatedProperty { origin: PersistentExtensionPropertyId,
                                        exact receiver arguments }
                  | StructuralType { exact_type: PersistentExactTypeId }
OdrGroupId = DomainSeparatedCborHash("scoop-odr-v1", SpecializationKey)
OdrMemberKey = { group: OdrGroupId, role: OdrMemberRole,
                   discriminator: OdrMemberDiscriminator }
OdrMemberId = DomainSeparatedCborHash("scoop-odr-member-v1", OdrMemberKey)
```

四个specialization variant的wire tag固定为1…4。全部argument/application必须使用persistent exact type identity；没有owner/application/argument时使用对应variant的typed空分支，不能靠空vector推断entity kind。member role只区分同一group内的member，不进入`SpecializationKey`或`OdrGroupId`。`OdrMemberRole`的tag 1…16固定为`CallableBody, GeneratedNominal, Layout, ScanProgram, TypeDescriptor, DispatchTable, DispatchAdapter, StaticStorage, ImmortalObject, InitializationCell, InitializationDescriptor, RegistrationRecord, DiagnosticBytes, AddressTakenConstant, ObjectSupport, ReleaseHook`；`ReleaseHook=16`在identity schema v1中已有唯一语义且只允许`ExactType` discriminator，但 M23 的 HIR/MIR/LIR outer schema 1 与完整 artifact profile 仍拒绝产生/消费它；M24 的 outer schema 2 与 cross-cone-generic/3 才启用，不能只按 profile 数字 2 判断。

role/discriminator 合法性之外，reader 还须从 discriminator 的 canonical key 和 typed owner 关系核对其唯一 group root：Callable 对应完整 application，Nominal 对应 origin 与 exact arguments，DelegatedProperty 对应原 property application unit，StructuralType 对应实际非名义 exact type。producer Cone、symbol、物理分片或相同 layout 不能替代实体归属。实际产生的 layout、TD、scan、dispatch、callable、closure/coroutine、静态存储、registration 和取址常量均按既有 root 与 member role 保存；generated-C bridge 仍只在 canonical LIR/ODR 中引用 producer-independent unit，物理 relocation 经核对后规范化回该 unit。

M23-7 的 ODR 按实际重复 member 判等。一个 artifact 记录本次发射的成员集合；不同 artifact 可以需要同组的不同独立 helper，例如从不同 source signature 转换到同一 target function shape 的 adapter。同一 `(group, member)` 的完整 key、ABI 与 definition 必须一致，兼容成员取并集，不要求整个 group 的成员集合相等。每个实际物化操作及已发射定义的必要引用仍须闭合；不能漏掉已使用的 TD/scan/dispatch、callable/EH/stackmap 或 storage/registration。generic delegated unit 的 storage、cell、failure root、initializer、ensure 和登记是每次物化的固定整体。具体规则见实现规范 2.13 与 M23-7 设计第 5、8 节。

M24 generic release hook 使用 owner 的 Nominal specialization group 及 `ReleaseHook/ExactType(owner application)` member，沿既有 owner key 核对归属；`CallableBodyKeyV2::ReleaseHook(owner)` 对应唯一 cb primary 与 OdrWeak，另有同组唯一 `RegistrationRecord/CallableBody(body id)` 对应 cr，不另造 CallableBody member、od alias 或 safepoint/sr。param-free hook 使用原 exact source subject 的普通 Strong 定义与链接可见性，不构造 ReleaseHook ODR member，也不增加来源或模板支持证明。不同 consumer 仍按共同 member 判等、独立 member 取并集；TD、hook 和登记的实际引用必须完整。版本与边界见 [M24 设计第 4 节](../milestone24/DESIGN.md#4-typedescriptor身份与产物)。

每个实际 ODR member 分别保存 `OdrAbiFingerprint` 与 `OdrDefinitionFingerprint`，使用 `scoop-odr-member-abi-v1` 和 `scoop-odr-member-definition-v1` 域。定义摘要包含 group/member/role、该成员的 canonical LIR、规范化 object/typed relocation 和关联 EH/stackmap leaves，不含其他独立成员、producer、`SlibMemberId`、物理分片或最终地址；旧整组摘要退役。Link 合并前必须比较所有重复 member 的完整定义，只有 ABI 相同不足以合并。摘要计算只依赖实际声明的上游输入，自身和非上游补丁槽归零；typed 调用目标按身份编码，不递归把 callee 的摘要纳入 caller，因此正常递归不形成摘要环。generated-C bridge 的规范化例外保持，不扩展到其他 consumer-local 符号。

Darwin/Mach-O 为每个 member 发射既有 kind-specific primary 和 `OdrWeak`，不假设原子 COMDAT；有 `cb/ly/sp/td/dt/ss/...` 的成员不得再发 `od` alias。实际要求保留的定义使用 `weak_odr`，不同 TD、storage 和 callable identity 禁止由 constant merge、ICF 或 `unnamed_addr` 合并。最终程序核对同一 exact type 的 TD、同一 storage/cell/body 的实际地址唯一性；两个 Cone 的 image 只需对共同 registration member 指向同一已合并记录，不要求各自表的成员集合相同。

独立 `identity-foundation` artifact profile（旧 `/1`、`/2`）退役。删除只供旧测试使用的 `IdentityFoundationMetadata`、`IdentityFoundationArtifact` writer，以及 `DecodedIdentityFoundations`、`IdentityCheckedFoundations`、`StructurallyValidatedFoundations` 和 native-boundary/commit 外层组成的平行 reader。三层基础 identity payload、真实依赖身份解析、类型/ABI/GC 契约与完整 Strong 产物 reader 保留；测试直接使用共有容器或完整生产 reader，不保留只有 identity、没有实际编译输出的产物路线。

生产 profile descriptor 只编码必需 section 清单：field 1=id、2=required_manifest、3=required_hir、4=required_mir、5=required_lir。原 field 6～9 及独立 `ArtifactValidationPolicy` 退役，不复用；删除仅服务于旧 profile 或未来占位的 availability policy、publication class、Link proof policy 与 ODR policy 数据。完整生产产物的 Code/RuntimeImage fingerprint 必须 Available，由 manifest 读取规则检查；ODR 在 M23-6 的 Strong 输入边界拒绝；M23-7 按实现规范 2.13 切换完整 generic profile，optional/unknown section 按实际 purpose 与 registry 规则处理。`single-cone-strong`、`cross-cone-semantics-strong`、`cross-cone-layout-strong` 的 major 均升为 3，旧 `/1`、`/2` 产物和缓存重建。profile fingerprint 继续覆盖这个实际格式描述，runtime C ABI 与 String 表示不变。

M23-3 的 strong-only production profile 为 `org.scoop-lang.slib-profile/single-cone-strong/3`。除三层 identity-foundation payload 外，它要求 Manifest `org.scoop-lang.manifest/single-cone-production/1`、HIR `org.scoop-lang.hir/core-bootstrap-interface/4`、MIR `org.scoop-lang.mir/core-bootstrap-bridge/1` 以及 LIR `org.scoop-lang.lir/strong-production/13`、`org.scoop-lang.lir/link-identity-closure/3`；code/runtime-image fingerprint 都必须为 `Available`，Compile 与 Link 消费边界拒绝 ODR group/member/body/symbol。旧 identity-only profile 退役，生产消费要求完整编译数据。发布使用同次编译的完整 typed IR 和产物汇总，不对当前产物及全部依赖再分别执行完整 Compile/Link 读取，也不增加发布凭证。M23-6 正式发布使用下述完整跨 Cone profile；ODR 仍留在 M23-7。

M23-7 的共有对象读取将 `link-identity-closure` 升至 `/7`，保留实际 ODR member 定义及必需引用，并将对象摘要中的本地和依赖目标统一为实际实体与 definition role，覆盖普通 callable、类型 shape、静态存储与初始化引用，Scoop 对象 verifier 升至 `/3`；production manifest 升至 `single-cone-production/2`，必需 field 11 保存本次发射的完整物理 ODR member 目录，原 field 5 保存 Strong registration 子集，field 4 仍保存全部实际注册 identity。目录的逐 member ABI/definition 与实际对象定义一一对应，Strong 产物的目录为空。对应 required inventory、profile fingerprint 与缓存同步迁移，旧 major 需重建。

正式 layout 产物沿同一发布与读取入口切换到 `cross-cone-generic/1`，原 layout-strong 产物需重建。共有语义与 ABI 查询直接使用完整 canonical HIR/MIR foundation；`OdrFree` 只限制历史 Strong 格式，不作为泛型输出的中间表示。必需 section 按实际 payload 分步升级，首条泛型函数闭环使用已经生产的 HIR interface `/32` 等 section、`cone-production/1` 及完整 ODR 目录；已落地的构造初始化模板将 HIR interface 升至 `/33`，区分泛型字段实例的位置表示再升至 `/34`，共享表达式保留原求值位置后升至 `/35`，实际名义类型 shape 内容由 `cone-production/2` 的必需 field 14 承载；共用该结构的历史 Strong production 升至 `/15`，继续使用空 shape 表并拒绝 ODR。后续委托等格式落地后同步升级 section、descriptor fingerprint 和缓存，不预写缺失正文或假空表。默认值中已替换的 bound receiver 以完整类型 key 保存，HIR interface 随该 payload 变更升至 `/37`，旧产物与缓存需重建。该迁移过程及最终完整 inventory 见实现规范 2.13 和 M23-7 设计第 10 节；所有语言与阶段验收要求保持。

实际泛型名义应用的成员与派发表保留原模板声明及完整实参，机器定义沿已有 ODR member 发布；接口默认方法、抽象槽和继承覆写在产物消费时保留同一语言语义。对应完整 callable 与派发格式使用 MIR `cross-cone-type-bridge/6`、LIR `cross-cone-layout-abi/5` 和 `cross-cone-layout-link-closure/3`，旧版本产物及缓存重建；字段、类型与 callable identity 及 runtime C ABI 保持，具体字段见实现规范 2.13。

本地 class、struct 和 enum 实现参数自由的依赖接口时，前端直接消费共有接口声明的完整父接口、typed slot、签名、默认实现和访问域。HIR 的 conformance 引用实际接口类型及本地/外来槽声明，目标为本地方法 application 或共有依赖 callable；不得为复用本地检查而复制外来函数声明、正文或生成同名替身。MIR 的 dispatch 表保留本地函数或实际外部 callable 引用，默认实现与抽象槽 trap 沿定义方原有 target 解析，LIR 使用现有 canonical ABI、外部定义与 relocation 路径。override、缺失实现、默认方法冲突、setter 能力和签名/effect 规则在同一前端检查中完成；类型、成员和 dispatch 独立及组合场景须经真实源码产物消费和单 image 普通/移动 GC 运行验收。

本地 interface 可以继承参数自由的依赖接口。父边保留实际 TypeId，override 关系保留实际本地或外来槽声明；继承的成员按父接口声明顺序进入完整槽表，菱形继承按声明身份去重，被覆盖的槽按已解析 override 关系消除。显式成员及当前 this 的隐式成员查找沿本地与依赖声明的同一父图进行，本地和外来候选共同执行语言规定的适用性与最具体选择；不能以声明存储位置决定优先级，也不能将外来成员复制为本地声明。该接口再次发布后，下游按实际父类型、槽与 provider 消费，保持 canonical ABI、默认方法、属性和装箱语义。

抽象 dispatch 目标与具体实现一样保留实际声明：Export HIR 的抽象 conformance 携带真实方法 application 或外来 callable 引用，source selection 携带所选 abstract 声明，完整 slot contract 携带该声明的 owner、signature、effect、modality 与原声明可见性。最近的 class 抽象声明以及更具体 interface 的抽象 override 均压制原默认实现；不把原槽声明伪装成所选目标。MIR、LIR 和 Compile/Link 按该 typed target 取得真实 trap、ABI 与 relocation，不再扫描所有 callable、重建整份继承图或沿继承链反向推测抽象目标。槽身份与所选声明身份可以不同，双方仍须满足实际继承、签名和访问合同。正常路径复用完整记录，不增加来源凭证或第二套证明表。

跨 Cone 的参数自由 class 继承直接使用依赖产物中的完整声明、真实基类、字段、接口与 dispatch selection。共有成员查询保存声明本身及其可见性，public、protected 和必要支持声明不改造成另一份 public 声明；前端按实际词法类和接收者静态类型执行访问、覆写及默认值规则。基类构造在已分配的派生对象上调用实际 provider 的 initializer，保持基类先于派生类的初始化顺序及移动 GC 接收者跟踪。继承的 virtual family 保留原 typed slot identity，派发表中的外来实现直接引用实际 callable；本地覆写只替换对应槽，未覆写的基类和接口选择完整传递。构造、super、成员与 getter/setter 沿共有调用、布局、ABI 和 relocation 路径消费，派生类再次发布后仍可由后续 Cone 使用。以上使用既有源码与机器声明格式，不增加来源资格、独立证明、平行来源表或后续里程碑能力。依赖虚槽的 lookup 域由 HIR lowering 按共有声明的 visibility 与 typed owner 计算，保留 protected 与外层 owner 约束；产物不重复保存该域，reader 不重放访问语义。只有本次实际物化的 descriptor 和派发表产生物理外部引用；依赖类的查询数据本身不形成 relocation。受保护的嵌套类型通过实际 owner 的 child 声明引用参与限定名查询，不能要求 public binding 或将其重建为本地声明。前端分别检查全部词法 owner 的有效访问域和 protected 成员自身的接收者规则；嵌套类型中的 public 成员不会额外要求接收者属于访问者的子类。嵌套构造、成员、enum 变体与 object 继续使用共有 typed 选择及初始化路径，访问错误与签名泄露在前端诊断。 已发布的可物化 nominal 声明必须同时具有完整 dispatch 与有限 BoxedValue/CoroutineStep/CoroutineSlot 支持，包括受保护嵌套类型和实际表示所需的支持声明；producer 与 reader 从同一共有声明闭包取得这些需求，public binding 不控制机器支持的生成。source-only 声明及仅存在于当前私有实现、未进入发布声明闭包的类型不因此成为发布根。 reader 的声明查询不再区分“仅当前 provider 支持查询”模式；本地与依赖的签名、父类型、accessor 和 variant 都按实际 typed ID 查询完整声明，保留 kind、arity、owner 与依赖范围检查。

继承的实际产物验收同时覆盖 ZST 字段与交错参数/返回值、大值构造与虚调用的 indirect/sret ABI、含 managed 引用的聚合参数和基类/派生类字段、次构造器的基类先行初始化及 object 单例继承。派生类再次发布后，由后续 Cone 继续派生并执行 super 与虚调用；普通和移动 GC 运行使用同一完整产物。上述场景不增加新的机器表示或 runtime ABI。跨 Cone 初始化中的 inherited backing field 由共有 property、实际字段身份与声明关系解析，前端检查 getter/setter 可见性及字段就绪；不能用同名字段或调用 accessor 代替直接存储访问。完整 HIR 的初始化字段引用区分本地 class application 与实际依赖字段，concretize 后均成为共有的 ConstructorReceiver 字段读写。基类完成后外来存储已就绪，computed/delegated property、不可见 setter、未就绪自有字段与 receiver 逃逸继续拒绝。现有产物已携带所需 property/field 引用，此项不增加 wire 字段或来源表。

setter 的有效访问域包含关系由前端在声明处检查。共有 reader 保留 property/accessor 的 typed 身份、owner、签名、effect、声明位置和 public binding 一致性检查，删除第二套访问域集合运算、逐属性继承遍历及其专用测试。继承环和引用闭合仍在共有继承图边界检查；已检查的完整声明不附加来源或操作资格，也不因 selected 去重而反复重新证明源码访问。此清理不改变 wire、profile、runtime C ABI 或 String 表示。

primitive 与 String 的接口默认方法使用前端识别的实际声明及已解析的接口实现参与普通成员查找。成员查找自身解析依赖中的父接口，不依赖此前是否发生过类型转换。重建 core 时的源码调用和下游的产物消费遵守同一候选选择、可见性、参数及 effect 规则；值接收者沿既有装箱路径适配，String 使用其既有引用表示。不得因这些类型使用内建表示而遗漏合法接口默认方法，也不为它们重建固定 provider 或增加后端资格。验收覆盖 Long、Boolean、String 的独立源码调用及跨 Cone 混合调用，保持现有 wire、runtime C ABI 与 String 表示。

跨 Cone 的覆写按真实声明关系继承默认参数。默认来源保留原声明的 typed root、定义路径、正文和引用；下游再次发布时只关联新的参数位置，不把原声明改造成本地函数或重新解析其源码。未求值的继承模板也保留完整源码位置记录；位置收集复用同一模板遍历，不触发机器物化。来自同一定义的菱形继承只保留一个默认来源，互不相关的默认来源在覆写定义处报冲突；参数名仍由调用点静态声明决定。实际省略参数时复用共有的依赖默认值实例化，并把接收者显式转换或装箱为模板所需类型，默认值内部的动态分派、显式参数求值顺序及 GC effect 保持。参数自由的导入模板已经在提供方完成类型和 effect 检查，未改变的正文不参与本地泛型约束重放。该功能沿用已有完整默认值产物格式和共享 reader，不新增来源凭证、模板工厂、wire tag 或 runtime ABI；验收包括覆写、菱形继承、命名参数、ZST、大值参数与再次发布后的真实消费。

限定 `super<I>` 通过当前 owner 直接列出的真实接口类型查找成员，本地接口和依赖接口使用同一候选决议、参数默认值与可见性规则。普通 `super` 对 class 成员的候选限制只用于实际基类接收者；限定 `super<I>` 使用 I 的接口成员集合，不能因共用 DirectSuper 调用方式而被该限制排除。调用选中的具体默认方法或 getter/setter 时，HIR 保存实际声明及强制 direct 调用方式；接收者沿共有引用转换或值装箱路径适配，MIR/LIR 按该声明的 canonical ABI 与 provider 定义发射，不再次进入接口表。抽象目标、非直接父接口、错误参数与初始化期间的调用仍在前端诊断。导出的默认参数正文用既有 `DirectSuperMethodCall` 节点保留该语义，下游展开继续调用同一真实目标。正文与引用集合复用同一 callable 引用转换，完整保留实际方法 owner，不分别重建不同的引用记录。同一 provider 的同一 typed callable 被 direct 和 dispatch 多次使用时，MIR 依赖记录与物理定义只选择一次，各调用点仍保留各自的派发方式。此项不新增来源资格、证明表、wire tag 或 runtime ABI；验收覆盖普通与 ZST 接收者、属性读写、命名及默认参数、大值结果和再次发布后的消费。具有隐式接收者的成员正文仍按普通值查找规则消费依赖属性、object 与 enum 变体；这些值与本地值使用同一个解析结果，不能归入函数或类型的非值阻断层。

M23-5 引入 `org.scoop-lang.slib-profile/cross-cone-semantics-strong/3` 作为多 Cone 语义产物的基线。当前该 profile 要求 HIR `org.scoop-lang.hir/cross-cone-interface/30`、MIR `org.scoop-lang.mir/cross-cone-param-free-bridge/2`、LIR `org.scoop-lang.lir/cross-cone-param-free-bridge/1` 与 Link 数据，并继续拒绝 ODR；M23-6 正式发布另包含完整类型和布局 section。共有 HIR 保存公开与必要支持声明、默认参数、常量、非泛型 alias、转导出路径及实际外部使用。名称查找按可见性枚举当前 Cone 和直接依赖；传递依赖按已经解析的 typed reference 查询。普通 callable 的声明、完整签名和 GC effect 由实际 provider 提供，final nominal 成员与顶层函数、extension 共用导出和消费规则。M23-6 的类型布局、构造器、成员、dispatch 与 protected 访问按各自语言及 ABI 规则完成；泛型物化与跨 Cone native 调用分别留在后续里程碑。格式 major 变化后旧产物与缓存需重建。

M23-6 的共有 HIR 接口 `/30` 保留 struct 的实际 `@CLayout`、`@InteriorMutable`、字段、成员及调用位置。公共和支持声明使用同一源码形状；布局按实际声明和 target 计算，不按类型名称、空字段或 core 身份补出策略。完整字段与版本规则见实现规范 2.6、2.11、2.12；runtime C ABI 与 String 表示保持。

跨 Cone struct 字段读取按接收者的实际声明解析名称，并在 HIR 保留 typed field identity 与完整接收者类型。字段在具体化时映射到同一声明的字段位置，后续布局和 ABI 继续使用共有依赖表示；不得用同名或同布局替代身份。计算属性通过其真实 getter 声明进入共有 callable 路径，保留可见性、GC effect 和返回类型；固定表示字段不为读取额外生成函数。依赖默认值中的字段和 callable 使用定义时保存的 typed 声明与完整类型，实例化不重新要求公开 namespace 导入路径，也不再次证明模板引用集合。

默认值依赖按定义处已解析的 typed target 保存，`DefaultDependency` 不要求消费 Cone 再取得 namespace 导入路径。定义处实际发生过的查找路径可以保留；生产器不为没有名称查找的字段、成员或支持声明补造 witness，也不保存全体依赖的第二份导入路径表。Unit、Any 与其他声明遵循同一规则。共有 reader 继续检查 provider、引用、类型、默认值正文及实际声明关系，不从这份路径信息授予调用资格。

共有 HIR `cross-cone-interface/24` 明确此默认值依赖合同，保留默认值既有字段及 `SourceDeclaration` tag 3，不增加或复用 tag；`/21` 及更早版本退役，旧产物和缓存须重建。profile fingerprint 按实际 descriptor 更新，runtime C ABI、String 表示与 GC 契约保持。读取边界核对源码上下文及位置后，实例化复用同一不可变记录，保留定义位置与调用处求值位置。

跨 Cone 名称、类型、成员和默认参数语义由实际声明及 typed IR 表达。独立的 foundation/declaration source transcript 及其逐层绑定结果不构成语言输入或调用资格；生产路径未使用的来源工厂、绑定包装和专用证明测试应移除。实际名称解析、可见性、默认值实例化与声明身份规则继续由对应前端实现负责。 默认值的源码位置和声明引用属于同一完整 HIR，不要求为位置收集另建一套默认值或访问凭证。

外部调用与布局引用使用实际 provider、typed target、完整 ABI、符号和定义记录。共有 Link 消费检查实际 undefined relocation 的符号、目标和覆盖关系；Code fingerprint 记录影响代码的依赖，member/range/offset 等物理位置按 Link 合同处理。同一次编译的完整 IR 和已验证依赖直接用于发布；不再保留独立 core requirement 闭包、来源资格或发布时分别重放 Compile/Link 的证明链。provider 拥有实际 body 和 Strong definition，re-export 仅引用已有声明。

独立 program-link stage 从显式 executable artifact 和完整依赖读取 Link 数据，按语言规范 12.3 的 canonical Cone order 与目录中的 `SlibMemberId` 顺序，将每个 `LinkObject` 恰好提取一次。root、image、定义、ABI、ODR 与真实 relocation 来自产物，Link 不重建 HIR 模板或重做语言语义；缺定义不能退回源码修补。M23-9 直接复用已有 foundation 身份／合同及 production、import 与对象记录，通过 `org.scoop-lang.lir/link-support/1` 的单字段 map 补齐 runtime 数据 alias，完整机器 ABI／布局仍原位读取；格式和 reader 职责见实现规范 2.8。diagnostic、opaque 和 unknown optional 成员不成为对象，当前 Link purpose 不认识的 required capability 必须失败。

Cone 的 defined/undefined 记录只覆盖带 `SlibMemberId` 的 `.slib` object。program、runtime 和 native 对象保留各自实际定义、引用及对象身份，不能伪造 Cone member。动态 provider 只满足已声明且 ABI 一致的外部绑定，不能替代受控的 Scoop/runtime/program 定义。M23-10 接受由已有逻辑 library requirement 定位的普通 native object、static archive 与 dynamic provider；这些外部文件不要求 Scoop producer 身份、专用 verifier 凭证或额外函数签名清单。消费边界检查实际格式、target、符号、存储、引用及必要 EH/TLS 事实。thin/nested archive、bitcode/LTO、隐式 autolink 和超出当前 target 输入范围的构造／析构等机制仍有明确错误，具体规则见实现规范 2.8 与 [M23-10 设计](../milestone23/stage10/DESIGN.md)。这不开放 Cone 内 C/C++ 源码编译或新的 `.slib` LinkObject producer。

所有源码 extern 按同一套完整声明合同和符号规则链接，不按 core 身份、函数名或所需功能设置特许清单。M23-9 从本次 runtime 对象与已选 SDK 系统 provider 的实际定义／export 解析引用；普通 Cone 也可以引用未被 core/runtime 使用的系统 export。声明间 ABI、GC effect、library、kind/storage 不一致或实际目标缺失均为错误；编译器产生的 runtime/target 需求与源码 extern 共用符号时还须合并其既有合同。普通外部对象不携带完整函数类型，不从符号表虚构 ABI 证明；外部实现遵守声明继续由 FFI 作者负责，core 也适用。M23-10 扩展新 library、archive 等输入的定位与供应，沿用同一 extern 合并／解析路径。

非空 `@Extern(lib = L)` 是逻辑库名，不是文件路径或 linker 参数。当前 Darwin target 对 `TargetDefault` 在每个显式 `--library-path R` 下检查 `R/L.o`、`R/libL.a`、`R/libL.dylib`、`R/libL.tbd` 与 `R/L.framework/L`；不去掉名字中已有的 `lib` 或扩展名，不递归扫描其他文件。已有显式 kind 只收窄对应的候选格式。相同内容及装载合同的重复候选合并，不同候选报歧义；目录枚举与 search-root 顺序不能决定选择。空 `lib` 只查询本次已引入的 runtime、native 对象及动态 export，不扫描目录寻找能提供某个符号的任意库。固定系统 provider 继续由 target 明确提供。

完整 extern 声明集合始终参与合同冲突与候选可用性检查；只有实际机器引用推动 archive 成员抽取。program-link 以实际 undefined 符号工作集闭合所需成员，包括成员之间及多个已提供 archive 之间的引用，选中的成员以独立对象交给系统 linker。同一归档内同名候选按物理成员顺序选择；重复被选的物理成员只加入一次，实际纳入的重复定义仍报冲突。归档循环引用由有限成员工作集处理，不要求用户重复列库或提供 whole-archive/group 参数。不被抽取的成员不贡献定义、引用、初始化或运行时效果；其容器、成员边界及候选符号索引仍须可正确读取。

一般 dynamic provider 是已有 native FFI 实现，不是可动态加载的 Scoop Cone。Darwin 按实际 install name、export、re-export 与 load-command 依赖解析；非 re-export 的依赖不会自动成为父库的公开 export。Darwin final-link 使用 two-level binding，并把每个实际 import 关联到确定的 provider；多个库中存在同名 export 不足以改变一个显式 library binding，默认命名空间中的多 provider 则报歧义。普通 native 文件的依赖不能触发 Scoop 源码或 C/C++ 编译。

Linux 动态模式读取 ELF `.so` 的 SONAME、DT_NEEDED、版本化导出与 TLS 类型，未版本化的源码符号匹配默认版本；DT_NEEDED 不使子库导出自动成为父库的公开接口。显式 `lib` 仍要求该库自身提供兼容定义；ELF 的平坦命名空间若使实际链接顺序无法同时满足这些绑定，报原生符号冲突，不模拟 two-level binding。库依赖按显式搜索目录、已有 DSO 的 RPATH/RUNPATH（含 `$ORIGIN`）及所选系统库目录读取。无 SONAME 的普通库沿用其文件名，最终程序使用正常 DT_NEEDED/RUNPATH 与系统 loader；不在运行时证明外部库字节或阻止平台正常的符号 interposition。静态程序只消费静态原生输入，framework/TBD 留在 Darwin。

`--library-path` 首先是链接时 locator。对于实际选中的 `@rpath/...` provider，M23-10 还从匹配该 install name 的明确目录生成必要 `LC_RPATH`，以使正常运行可以找到该库；这些实际写入 executable 的路径与 install name 属于装载语义，必须进入 link plan。纯输入 locator、临时快照路径和输出文件名仍不进入 identity 或 plan。此区别不影响 Cone/entity identity、三层语义 fingerprint 或 `.slib` 内容；不承诺复制／部署第三方库，也不在运行时证明 dylib 内容未改变。具体相对装载名范围及解析规则见 M23-10 设计第 5 节。

runtime 由 `scoop` 按明确的 target、C toolchain、源文件、头文件及构建规则产生任意非空普通对象集合；源码/object 数量不是协议基数。内容缓存与普通对象索引用于构建复用和进程交接，外部输入在消费边界检查 bytes、target、符号、引用与必要 ABI，不建立来源授权或独立信任容器。program-link 不读取 Scoop/runtime 源码，只可调用明确 C compiler 编译自己生成的固定 startup C 代码；不调用 Scoop codegen，不重编译 artifact bridge，不接受 raw archive 或额外对象注入。实际低层入口及索引见 [M23-9 设计](../milestone23/stage9/DESIGN.md)。

每个 Cone artifact 的实际 `LinkObject` 联合贡献且恰好定义一个由其 identity 唯一命名的 logical `ScoopImageDescriptorV1`，manifest 的 `image_owner_member` 与实际定义一致。各产物保留完整类型、ABI、dispatch、初始化、root 和 relocation 数据。M23-8 的 runtime 直接接收实际 image pointer 集合与 root entry，在加载边界完成一次全图登记，并按 12.3 的 canonical Kahn 顺序逐次经 no-throw gateway 初始化和执行 main；具体入口、线程与 GC 合同见运行时规范 2.8、实现规范 2.14 及 [阶段设计](../milestone23/stage8/DESIGN.md)。M23-9 负责正式 artifact-only program-link，不复制这些记录为另一份 program/core binding。

String 由前端解析为实际 typed class，MIR/LIR 与 Link 使用同一 provider 的 exact type、TypeDescriptor、registration 和 relocation。现有 C ABI 的 `scoop_td_String` 表示 runtime 所需的 String descriptor 地址；链接使用实际声明的定义，不从固定 CORE identity 重建它，也不按名称或同布局替换类型。对象头为 16 bytes，length offset 为 16，inline bytes offset/minimum 为 24，alignment 为 8；`InlineBytes` 的 size/stride/alignment 为 1，object/inline scan 为空。保留这些真实表示和边界检查，不单列重复的 String 布局/scan 副本、capability-kind digest 或 program/core binding 证明。 M23-9 的 Link support 只保存固定 runtime 数据符号到原 TD owner 的实际 alias 引用；最终地址相同，不复制 TD 或增加 pointer slot。

M23-6 已删除没有实际 producer/consumer 的 `ScoopRuntimeCoreBindingsV1`、`ScoopProgramDescriptorV1`、固定 String capability ID 及其 LLVM 布局镜像和专用测试。未使用的 program/core magic `0x53434f4f50505247`、`0x53434f4f50434f52` 退役且不复用。M23-8 规定 `scoop_rt_run_program(images, image_count, root_entry)` 直接消费实际 image/root 引用，统一初始化 registration 与 coordinator 参数；metadata ABI 升至 3，字段布局及 String 表示保持，格式迁移见实现规范 2.14。M23-9 生成同一入口的实际启动对象，不另建 program/core descriptor 或 String 授权表。

最终链接的普通缓存记录由实际输入产物、toolchain、target、链接选项与有序操作构成；输入变化使缓存失效。符号、ABI 和引用合法性由相应读取及链接边界检查，复用未变化的完整数据，不要求来源授权、资源预算或重复完整证明。`.slib` 不是动态加载格式，M23 不承诺跨不兼容 schema、target 或 compiler ABI 的稳定二进制接口。

HIR/MIR/LIR semantic fingerprint都是Merkle值：除本层canonical semantic projection外，还按typed support/re-export edge纳入origin artifact对应层fingerprint；保守实现可纳入全部direct dependency同层fingerprint。LIR fingerprint context非可选地包含target/backend两对typed id与fingerprint，所以任一契约变化都使LIR fingerprint变化。LIR semantic projection覆盖canonical definition、layout/ABI/scan与typed external relation，但明确排除`SlibMemberId`分配、object分片、boundary/range、patch offset与archive placement；这些物理验证字段不在LIR wire，而由codegen/object verifier在LIR定稿后写入artifact manifest的`ObjectVerificationProjection`，并由code/artifact fingerprint与object/link verifier覆盖。两者只通过member-independent typed plan/intent id连接，不能回写LIR。HIR缓存尤其必须覆盖lookup observation surface，包括空binding、全部不适用的高层候选、star snapshot与MSC rejected candidate，不能只记录最终winner；M23 v1在没有完整observation set实现前必须保守纳入全部direct dependency HIR fingerprint。相应semantic fingerprint变化使下游缓存失效；仅object/code变化至少使最终link失效，而只重分片不改变LIR语义不能迫使dependent重编译。导出`const val`值、default body/绑定target、re-export snapshot/witness集合、alias target、generic template/hidden closure、inheritance contract、layout或ABI的变化都必须进入对应fingerprint，不能因版本文本未变而复用旧typed metadata。

### 12.6 核心库

第11章的核心库是reserved library Cone **`scoop:scoop.core:0.1.0`**，以独立`.slib`提供，sysroot只提供默认查找位置；`scoop.core`同时是其源码当前使用的package和Cone `name`，这只是明确约定，不是package与Cone identity之间的语言推导规则。

前端解析 intrinsic 与 desugar 所需声明后，后续 IR、metadata reader 和 linker 只按完整 typed 引用、表示及实际 provider 处理。不得再以来源为 CORE、Core/NotCore 标签或专用可信类型载体授予协议、native-boundary、String/初始化服务或 Link requirement 资格；也不得因当前 Cone 是 core 而跳过 canonical ABI 重放等共有验证。所需声明的签名、effect、类型关系、源码可见性及依赖可达性继续验证，不能用同名或同布局对象代替已解析声明。具体 stage 职责见实现规范 2.12。语言是否合法不由逻辑 CPU、内存、节点、复制字节或任意工作额度决定；引用环、整数溢出及类型递归按具体语言和表示规则检查。默认参数的名称、类型、effect 与可见性在定义处由前端检查，产物保留实例化所需的完整 typed 正文和声明引用，不另外发布授权凭证。

- 除core自身外，每个Cone都具有到该exact core Cone的direct dependency。manifest可以像其他依赖一样显式声明core的`path`、`artifact`或search-root locator；未声明时注入默认edge。先解析全图的显式声明，已有core节点则复用，仅当仍无core节点时才读取默认sysroot源码位置。显式locator失败按普通依赖报错，不回退到sysroot；不同core来源的冲突使用同一coordinate/content唯一性检查；
- core自身不隐式依赖自身。core manifest中的显式dependency使用普通语法、locator和图校验，不在manifest parser或单Cone请求归一化阶段因当前Cone为core而拒绝。非core Cone仍具有到core的direct edge，因此core到普通源码库的依赖若形成回边，必须由12.3的共有cycle诊断拒绝，并保留显式声明位置与注入边来源；这一规则不允许绕过无环要求。用户可以在普通源码目录声明core coordinate、修改或扩展其源码，并重新构建library；`@Intrinsic`的识别与类型检查在前端完成，desugar引用解析后的普通声明，不要求sysroot来源授权；
- `scoop.core.*`与`scoop.core.Option.*`默认可见性来自core `.slib`中的typed prelude binding，不通过把core源码拼入用户AST、扫描package name或对`Some`/`None`写短名特判实现；
- core中的普通public API、generic template、non-generic alias、layout、TypeDescriptor与runtime binding遵守与其他library Cone相同的metadata、persistent identity和兼容检查。sysroot core与compiler的language/runtime、target及backend fingerprints不兼容时必须重建或拒绝，不能退回core与用户源码同单元编译。

---

## 13. 核心注解与 FFI

以下注解类定义于 `scoop.core`，随默认导入可用。它们修饰的约束大多在编译期检查，违反即为编译错误。

本章多处使用 **GC-free** 的概念：一个类型是 GC-free 的，当且仅当其完全确定的表示中不直接或间接包含任何 ref type（基本类型、`Ptr`、`FunPtr`、不含ref的`@CLayout` struct等都是 GC-free 的值类型）；一个函数是 GC-free 的，当且仅当其不读写、不创建任何 ref value（见 13.2）。GC-free布尔属性只属于不含未解析type parameter的concrete type或fully specialized generic type；每个这样的type实体都必须具有非可选的`gc_free: bool`，不存在“未知”或缺失状态。尚未完全特化的generic declaration不是concrete type，没有GC-free真假flag；编译器可以保存“哪些实参必须GC-free”的符号条件，但该条件不是flag。每个fully specialized enum的每个variant都必须具有非可选`gc_free: bool`，enum自身的flag恒等于所有variant flag的逻辑AND。

### 13.1 `@Intrinsic`

```
annotation class Intrinsic(val name: String)
```

- 用于function/method：该函数是compiler intrinsic，由编译器生成实现；**函数体必须省略**；`name`是intrinsic的编译器内部标识。method必须仍按普通member语法、visibility与operator签名规则声明，不能用extension形态绕过登记shape。

- 也可用于登记表明确允许的core struct/class，声明一个**intrinsic type**：该类型的representation、literal lowering、ABI及内部构造机制由编译器提供，源码声明其nominal interface与成员语义。例如：

```
@Intrinsic("core_int")
public struct Int : ToString, Hash {
    @Intrinsic("int_add")
    @NoGC
    public operator fun plus(rhs: Int): Int

    @Intrinsic("int_equals")
    @NoGC
    public operator fun equals(other: Int): Boolean

    public override fun toString(): String = coreLongToString(this.toLong())

    public override fun hash(): Long = coreLongHash(this.toLong())
}

@Extern(name = "scoop_rt_long_to_string", abi = "scoop")
internal fun coreLongToString(value: Long): String

@Extern(name = "scoop_rt_long_hash", abi = "scoop")
internal fun coreLongHash(value: Long): Long
```

- intrinsic type不声明字段/primary constructor，也不等价于零字段普通struct/class；不能据此派生零大小布局、全等equals、`Int()`字符串、字段访问、解构、copy update或公开零参数constructor。编译器合成的literal/boxing/allocation entry不进入源码候选集；用户可调用constructor或转换来自显式声明或 registry 封闭规定的入口：10.4 的数组转换、10.6 的数组按长度初始化与 13.10 的 `Ptr<T>(raw: ULong)` unsafe construction entry；
- intrinsic type的implements列表、普通成员body、override/operator规则与普通类型一致；单个成员也可像上例一样另用function intrinsic提供实现。初始intrinsic type至少覆盖八种canonical integer representation、Boolean、M26 的 Char、String、`Array<T>`/`MutableArray<T>`以及13.10的`Ptr<T>`/`FunPtr<F>` family；integer登记项必须封闭地给出signedness与8/16/32/64位width，alias本身不能再次登记为intrinsic type。`core_int`/`int_*`表示32位canonical `Int`，64位signed表示使用独立的`core_long`/`long_*`；unsigned同理区分`UInt`与`ULong`。登记表可按同一契约增加其他compiler-represented value/reference type；
- intrinsic type可以是generic，但其登记项必须完整规定declaration kind、type-parameter数量/bound及representation family；所有参数按3.2固定为invariant。`Array<T>`与`MutableArray<T>`各要求一个无bound参数；它们的每个fully specialized application仍是普通generic class application，只是对象布局、元素stride和GC扫描由携带concrete element type的typed intrinsic representation产生。不得同时保留普通class application与独立built-in array type两种identity；
- 前端识别core源码中的intrinsic annotation，并检查以下name、target、shape、signature与唯一性规则；测试可直接提供相同源码输入。此处理不建立后续stage的来源授权能力。

- 除非有单独说明，`@Intrinsic` 不能与其他任何注解共存。integer registry中除`div`/`rem`外的纯scalar operation与conversion是一个封闭例外：其声明必须同时带`@NoGC`；`div`/`rem`不得带。M26 的 Char code/相等/比较和 core 内部 unchecked code-point 构造均为 NoGC scalar operation，其中 unchecked 构造还必须标注 `@Unsafe`；公开 `Int.toChar` 在普通 core body 中先检查范围再调用。13.10列出的pointer intrinsic继续按该节例外组合`@NoGC`/`@Unsafe`。
- `name` 必须是编译器内置intrinsic登记表中的已知标识；未知`name`、错误annotation target、与登记shape/signature不符或同一intrinsic kind存在多个provider都是编译错误（用户不能声明自定义intrinsic）。各intrinsic在编译pipeline中的展开阶段由实现大纲规定。

### 13.2 `@NoGC`

```
annotation class NoGC
```

- 用于function/method及9.1.5允许的explicit accessor/struct secondary constructor：指明该callable不会/不应与GC有任何交互——有body的callable中不读写任何ref value，也不创建任何ref type实例。唯一无body的组合是`abi = "scoop"`的top-level `@Extern` function：此时`@NoGC`是由FFI作者承担的callee contract assertion，并使其`GcEffect`取`NoGc`；省略时取`Managed`。C ABI extern本身已由C边界契约固定为GC leaf，不接受`@NoGC`这一重复且易混淆的拼写。
- struct secondary constructor的`@NoGC`要求完整参数、构造结果、委托求值与body中的运行时值均为GC-free，`this`只能委托primary或另一个`@NoGC` secondary constructor；未标注的secondary即使body看似纯净也保持Managed合同。primary struct与enum variant的直接值组装可出现在`@NoGC`代码中，但实参求值与完整结果表示仍须满足GC-free约束。参数缺省表达式在caller求值，遵守caller的GC effect，不因callee的`@NoGC`而自动获得GC-free资格。
- 也可用于`struct`或`enum`，作为“该concrete value type必须GC-free”的静态契约。非generic声明在字段类型解析后立即验证；generic声明本身没有GC-free真假值，每个type parameter全部resolve后的实际类型分别验证。对fully specialized enum，契约同时要求enum整体及每个variant均为GC-free。`@NoGC`不能用于class/interface，因为它们是ref type。 当前声明与依赖声明的契约相同；完整 application 只出现在签名、别名、父类型或嵌套类型中也必须满足，不能等到访问字段或执行构造才检查。泛型使用把实际影响该契约的形参条件沿既有实例化关系传播，phantom 参数不因此成为 GC-free 条件。
- 编译期检查；不符合约束是编译错误。
- 无自定义正文、无需初始化 ensure 且值类型为 GC-free 的普通顶层存储访问器，只有读取或写入既有存储的操作，编译器生成的 callable 使用 NoGc 合同；这使同一属性的直接访问与跨 Cone 泛型访问保留相同 GC effect。带运行时初始化、含 managed ref 或有自定义正文的访问器不据此推导 NoGc；显式正文仍按其声明的 annotation 检查。
- generic `@NoGC` callable 可以在签名或 body 中使用类型参数；未特化的generic本身不被判为GC-free或非GC-free。每个实际影响参数、返回值、receiver、局部值或表达式表示的类型参数，都会形成“实例化实参必须GC-free”的类型化条件，并经generic调用链向外传播；只有type parameter全部解析后的具体实例才能用concrete type的GC-free flag验证并成为`@NoGC`实例。未参与运行时表示的phantom type parameter不产生条件。
- `T : value` 只保证实参是 value type，不保证其递归表示中不含 managed ref，因此不能代替上述 GC-free 条件；`T : ref` 则不可能满足该条件。当前没有单独的源码 bound 语法来声明 GC-free，条件由 `@NoGC` body及其调用图推导。
- 这样的函数可以安全地跨越 FFI boundary（例如作为 FFI 回调）。
- 该约束也意味着 `@NoGC` 的成员函数只能属于 value type：class method 有隐含的 `this` 参数，而 `this` 是 ref value。
- 8.3 的 contextual declaration 不得标注 `@NoGC`；`context(value) { ... }` 也不是 NoGC 操作。runtime 的无分配 lookup/restore leaf 不等于源码 `@NoGC`，它们仍读取或写入 managed ref。
- 9.1.6 的 release block 不是普通 callable 或 `@NoGC` target。定义方在已有 NoGc 检查上推导更窄的 release-call effect 与 `ReleaseValue` 条件，并通过普通 callable 接口供依赖使用；`@NoGC` 本身不保证没有 native transition、TLS 或 GC capability 操作。普通 NoGc 代码的既有调用语义不因 M24 改变。

```
@NoGC
fun add42(n: Int) = n + 42

@NoGC
fun <T> identity(value: T): T = value

@NoGC
struct NativePair(val x: Int, val y: UInt)

// 编译错误：字段直接包含ref
@NoGC
struct BadNativeValue(val text: String)

val number = identity<Int>(42)          // 合法：Int 是 GC-free
val text = identity<String>("managed") // 编译错误：String 是 ref type

// 编译错误：String 是 ref type，违反 NoGC 约束
@NoGC
fun addString(s: String) = s + "world"
```

### 13.3 `@Unsafe` / `@Safe`

```
annotation class Unsafe
annotation class Safe
```

- **unsafe callable**：C ABI `@Extern`函数，以及标注`@Unsafe`的function、9.1.5允许的constructor/accessor。Scoop ABI extern默认safe，但可显式加`@Unsafe`收紧调用条件（13.4、14.2）。
- **unsafe context**：标注`@Unsafe`的callable body，或标注`@Unsafe`的block。只有在unsafe context中才能调用unsafe callable、构造unsafe constructor或读写unsafe accessor；否则是编译错误。
- `@Safe`用于function、9.1.5允许的constructor/accessor和block，在unsafe context中重新引入safe约束。

### 13.4 `@Extern` 与 `@CallingConvention`

```
annotation class Extern(val lib: String = "", val name: String = "", val abi: String = "c")
annotation class CallingConvention(val name: String)
```

- `@Extern` 用于top-level、non-generic function：指明该函数是位于 `lib` 所指库中的 FFI function，符号名由 `name` 指定，`abi` 指定 ABI（见 13.8）。函数体必须省略。声明可以带普通Scoop默认参数；缺省表达式按8.5在定义处解析、调用处实例化，不进入native symbol的ABI，native调用始终接收完整参数列表。`vararg`在ABI中表现为一个普通`Array<T>`参数，因此只有该实际参数类型满足对应ABI classifier时才合法：C ABI因ref不安全而拒绝，Scoop ABI可以接受；这不表示支持C的`...`可变参数。
- `@Extern` function与member/local、generic或`suspend`均互斥，无论`abi`取值为何都在声明处报编译错误。尤其不能把generic extern的多个concrete ABI绑定到同一个native symbol。编译器不为这些非法声明生成receiver、type-argument或continuation bridge；M10的hidden continuation ABI不得作为外部符号ABI暴露。
- `@Extern` 也可用于**全局变量**（`val` / `var`），访问库中的全局符号；extern `var` 仍须带 `@Global` / `@ThreadLocal` 且 GC-free（见 13.6），这些注解可以组合。extern 变量当前只支持 C data ABI，显式写 `abi = "scoop"` 是编译错误；Scoop ABI 只定义函数调用边界。
- **按 ABI 分类的边界类型约束**：`abi = "c"` 的函数签名及 extern 变量必须满足 13.8 的 C-FFI-safe 约束，因而全部 GC-free；ref type 出现在这些边界上是编译错误。`abi = "scoop"` 的函数复用普通 Scoop typed ABI，可以按第 14 章直接传递 managed ref，不套用 C-FFI-safe classifier。
- Scoop ABI extern的source contract非可选地携带独立`GcEffect`：未标注`@NoGC`时为`Managed=1`，显式合法标注时为`NoGc=2`。该轴只约束native callee能否进入GC/runtime/managed callback并进入ABI contract/fingerprint，与ordinary/suspend函数类型effect正交；extern仍禁止`suspend`。它不改变caller边界种类，两个值都按14.2走`NativeBorrowed`与caller-root publication，`NoGc`不能降级为普通Scoop `NoGc` callsite。
- 不支持 C varargs（`printf` 式可变参数）；需要时用 wrapper 函数绕行。
- `abi = "c"`（默认）的 extern 函数是 unsafe function，只能在 unsafe context 中调用（见 13.3）；`abi = "scoop"` 的 extern 函数例外，调用点不要求 unsafe context（见第 14 章）。
- `@CallingConvention` 用于 function，标明 calling convention（如 `cdecl` / `stdcall` 等，具体含义由实现确定）；可与 `@Extern` 组合使用，指定 FFI function 的 calling convention。

每个合法extern声明先建立跨Cone、target-independent的**source native contract**。省略的`name`解析为声明名；source symbol保存非空、无NUL的annotation UTF-8 bytes，target正规化前不宣称它就是object symbol。contract非可选地记录逻辑library binding（空`lib`使用封闭`DefaultNativeNamespace`）、symbol kind（function、read-only/mutable data或read-only/mutable TLS）、source calling convention，以及完整source ABI。function ABI是封闭`SourceExternFunctionAbi::C { SourceCAbiFunctionSignature } | Scoop { SourceScoopAbiFunctionSignature, GcEffect }`；data/TLS只接受C-safe storage type且不携带calling convention。默认参数、形参名、`vararg`源码拼写和transparent alias不进入native签名：它们先按本节规则展开/物化。

M28 的 Linux glibc/musl amd64 目标分别使用 `org.scoop-lang.target-profile/linux-x86-64-gnu/1`、`org.scoop-lang.target-profile/linux-x86-64-musl/1`，对象格式为 `org.scoop-lang.object-format/elf-relocatable/1`，共享后端为 `org.scoop-lang.backend-profile/llvm-22-1-linux-x86-64/1`。ELF 的 logical symbol 与真实 object symbol bytes 相同；同样拒绝空、NUL 和首 byte LLVM escape，不折叠已有下划线。native library requirement 和 symbol identity 的 target 必须原样生产、读回与比较，不能默认改为 Darwin。两种 libc 的 `.slib`、runtime 与 core 产物不能混用。

LIR在`ValidatedLirTargetSelection`下把source contract唯一正规化为target-specific native contract。`NativeExternalSymbolKey { target_profile: TargetProfileWireId, native_link_symbol: bytes }`使同一logical symbol在不同target中不共享identity；当前Darwin/AArch64的`MachOExternalUnderscore`要求输入非空、无NUL且不以LLVM escape byte `0x01`开头，并逐byte映射为`0x5f || logical`，输出长度为输入长度加一，使用 checked 长度计算与实际分配错误处理。它不做Unicode、大小写或已有underscore折叠，因此`foo → _foo`、`_foo → __foo`，Scoop mangled symbol同样只前置一个underscore。Scoop LLVM object与generated-C bridge object verifier必须应用相同规则，`native_link_symbol`统一保存真实目标 object symbol-table bytes，不能一方保存logical C name、另一方保存`nlist` name。当前Darwin链接模型以这些bytes作为冲突键，因为object中的undefined reference不携带源码`lib`归属。

target-specific contract记录canonical native library requirement、symbol kind、`C | Scoop` ABI、target-normalized calling convention及完整签名/storage。C function的`CanonicalCAbiFunctionSignature`只保存`{ calling_convention, parameters: [{ source_exact_type, CanonicalCStorageType }], result: Void | { source_exact_type, storage } }`；`@CLayout`另保存含exact owner、byte size/alignment、aligned/packed policy和declaration-order `{ field id, offset, storage }`的`CanonicalCAbiLayout`。code pointer storage保存其`NativeFunctionPointer` exact type与direct/nullable storage provenance；该exact type已经唯一编码完整参数、结果及calling convention，不再冗余嵌入signature fingerprint。否则合法的“struct字段是接收该struct值的`FunPtr`”会形成layout fingerprint与signature fingerprint之间不可计算的哈希环。其fingerprint分别为`DomainSeparatedCborHash("scoop-c-abi-signature-v1", CanonicalCAbiFunctionSignature)`与`DomainSeparatedCborHash("scoop-c-abi-layout-v1", CanonicalCAbiLayout)`；native symbol id为`DomainSeparatedCborHash("scoop-native-link-symbol-v1", NativeExternalSymbolKey)`。这两者是canonical C**源码storage/layout contract**，刻意不保存target register class、integer extension、`byval`/`sret`或手写aggregate pass classifier。所有C function、global与callback统一由canonical generated-C source bridge交给`ValidatedCBridgeToolchainProfile`指定并验证的system C compiler完成真实C ABI lowering，Scoop侧bridge只使用已类型化storage ABI；缺少该toolchain capability时拒绝production，不能从host默认ABI补值。Scoop ABI不复用C pass mode；`CanonicalScoopAbiFunctionSignature`精确保存field 1 `ExactCallableSignature`、field 2逐参数`ElidedZst | Direct | Indirect` typed storage、field 3 `UnitVoid | ElidedZst | Direct | Indirect` result和field 4 `GcEffect`。field 4必须逐项等于source contract，不能从物理signature或callsite反推。

`NativeExternalContractFingerprint = DomainSeparatedCborHash("scoop-native-external-contract-v1", { symbol_id, contract })`，因此同symbol与同物理value shape但`GcEffect`不同的Scoop ABI extern仍是不同contract。M23 `.slib`导出当前Cone全部extern contract；import/re-export只传播原contract，不以本地声明重写。最终静态程序在调用native linker前，按native linker symbol bytes合并完整dependency closure：同一symbol只允许字段逐项相同的contract，完全相同者去重；library、kind/TLS/mutability、ABI、calling convention或完整签名（包括Scoop `GcEffect`）任一不同都是链接诊断，即使native linker本来会任选一个定义。诊断列出全部声明origin及首个不同字段。不同源码type只有在各自ABI的canonical native signature确实相同时才可合并；相同bit宽但pointer provenance、C storage/layout或Scoop exact type不同不能靠字符串/size碰巧相同放行。该检查覆盖extern call/global use及C bridge为这些extern目标产生的native undefined reference；每个此类symbol必须有且只有一个已验证contract。编译器生成的bridge入口、runtime entry及target toolchain support采用独立typed link requirement，不伪造源码extern声明，也不能通过任意symbol前缀豁免检查。`FunPtr` native-address target按13.10必须是Scoop-owned、非extern的`@NoGC`函数，继续走callable/object verifier；只有`FunPtr`作为extern签名中的C code-pointer type时才作为该signature的一部分。该闭包不验证外部二进制真实实现是否遵守声明，那仍是FFI作者责任。

### 13.5 `@CLayout`

```
annotation class CLayout(val aligned: Long = 0L, val packed: Long = 0L)
```

- 用于 struct，指定该结构体内部成员的 align/pack 规范。
- `aligned` / `packed` 的缺省值 `0L` 分别表示不增加struct最小对齐以及不限制field自然对齐；非零值只能是 `1L` / `2L` / `4L` / `8L` / `16L`，其他值是编译错误。当前64位target profile不接受更大的显式对齐；未来profile如需扩展必须先修订本节的source契约，不能截断或静默归一化。
- 带有此注解的 struct，其每个字段都必须递归具有稳定C表示、且字段的concrete type不能是ZST；因此它不能直接或间接包含任何ref type，也不能以空字段或零尺寸字段依赖C实现扩展。字段类型不依赖type parameter时在声明处检查；generic `@CLayout`字段依赖type parameter时，声明保存逐字段的`C-FFI-safe && NonZst`条件，每个fully concrete application在替换后检查。失败诊断位于该application/concretization点并同时指出原始字段路径；跨Cone导入不得提前接受、删除或重新解释该条件。无字段的`@CLayout`在声明处直接报错。

### 13.6 `@Global` / `@ThreadLocal`

```
annotation class Global
annotation class ThreadLocal
```

- 用于top-level `var`请求一个可寻址的raw global/TLS storage；两者互斥。该声明不再是9.1.3的ordinary property，不能带custom accessor/delegate，并继续只接受显式type与编译期static initializer；
- raw global/TLS必须GC-free，类型中不能直接或间接包含ref。`addressOf`只可用于这类本地raw storage或合法extern storage，不能取得ordinary property backing slot；
- ordinary top-level `val`/`var`不要求这两个annotation，可以含managed ref，由compiler-managed accessor、global root与initialization unit承载。一般成组全局状态仍建议用`object`表达；
- extern `var`仍必须用`@Global`/`@ThreadLocal`说明native symbol是否TLS并满足C-FFI-safe；extern `val`只读view不要求这两个annotation。

### 13.7 `@InteriorMutable`

```
annotation class InteriorMutable
```

- 用于 struct：标明该 struct value 内部的值可能会被改变，不能保证 immutability。常用场景：struct 带有 atomic 字段，或 FFI 会修改传给它的 struct 参数。
- 带有此注解的 struct 及其 value 只能在 unsafe context 中使用（见 13.3）。
- 该注解不为 value 引入 identity，也不改变按值复制边界：两个 copy 仍是两份独立存储。值类型成员方法仍按 3.3 取得独立的 `this` copy，不得因 ABI 指针传递而把调用方存储暴露给方法。

### 13.8 FFI ABI

Scoop 的 FFI 函数有两种 ABI：

- **C ABI**（`abi = "c"`，默认）：标准的 FFI function，由外部 lib / so / dylib / dll 提供。它对 Scoop 的类型系统和 GC 环境没有任何了解，也不能使用相关功能，用于直接引入外部库。参数与返回值必须是 C-FFI-safe：GC-free 且具有本章规定的稳定 C 表示。
- **Scoop ABI**：复用普通、非挂起 Scoop 函数的 typed machine ABI。ref 参数/返回值直接以 managed ref value 传递，value type按 Scoop 自身的 concrete ABI 传递；被调方能读取 TypeDescriptor，并可按第 14 章的 native-root 协议显式进入可能触发 GC 的 runtime 操作。它主要供 runtime 与 core 使用，不是通用 C library ABI。

C ABI callee本身始终是 GC leaf，不能直接接收managed ref或调用Scoop GC；多线程runtime下 caller仍须按 runtime spec 3.5 发布roots并切换到native-safe状态。C代码只有在持有14.3注册得到的静态trampoline与cookie时，才能经独立的反向边界进入managed callback；这不改变该C函数自身的参数ABI或赋予它Scoop ABI能力。Scoop ABI extern无论`GcEffect`为`Managed`还是`NoGc`，caller都发布可更新roots并切换到`NativeBorrowed`，返回后按epoch协议reload；effect只约束callee contract并参与native contract fingerprint。`NoGc`不得改写成无需root/transition的普通Scoop NoGC call。具体机器级序列由实现决定，但不得改变上述类型与GC契约（第14章）。

C没有跨当前支持profile可依赖的零尺寸object ABI。C-FFI-safe classifier因此只允许`Unit`作为函数返回并映射为C `void`；ZST不能作为C参数、callback参数、extern global/TLS或by-value result，也不能成为`@CLayout`字段。无字段的`@CLayout`、不依赖type parameter且已知含ZST/最终size为0的声明在声明处报错；依赖type parameter的字段按13.5在每个fully concrete application的concretization点检查，不能等到C bridge或native linker才失败。`Ptr<Unit>`仍是显式的`void *`/opaque handle例外；其他`Ptr<ZST>`可在Scoop unsafe代码中使用，但不能凭“pointer本身有C表示”自动获得一个不存在的C pointee object type。

ref type（如 `String`、`Array`、普通 class）不能出现在 C ABI 的边界上（13.4 的 C-FFI-safe 约束）；同一类型可以直接出现在 Scoop ABI extern 签名中。`PinnedPtr<T>` / `GcHandle<T>` 已是 GC-free 的显式边界值：它们适合 C ABI、跨调用保活或需要稳定裸地址的场景，不是 Scoop ABI direct-ref 调用的必经表示。

定宽integer在C ABI参数、返回、extern global/TLS及`@CLayout`字段中精确映射：`Int8`/`Int16`/`Int`/`Long`（即`Int8`/`Int16`/`Int32`/`Int64`）分别为`int8_t`/`int16_t`/`int32_t`/`int64_t`，`UInt8`/`UInt16`/`UInt`/`ULong`（即`UInt8`/`UInt16`/`UInt32`/`UInt64`）分别为`uint8_t`/`uint16_t`/`uint32_t`/`uint64_t`。transparent alias先展开；不依据Scoop拼写把它们推断为C的`int`/`long`/`intptr_t`。窄integer实参/返回的寄存器extension与aggregate pass分类完全由validated C-bridge toolchain编译的canonical `stdint.h` source signature决定，Scoop metadata不得持久化第二套手写target C register classifier，也不能假定Scoop typed ABI恰好等于目标C ABI。

### 13.9 `value` / `ref` 类型约束

`value` 与 `ref` 是上下文关键字，用作泛型的 type bound：

- `T : value`：`T` 必须是值类型（struct / enum / tuple / 基本类型）；
- `T : ref`：`T` 必须是引用类型（class / interface）。

```
fun <T : value> needValue(v: T) { ... }

needValue("hello")    // 编译错误：String 不是值类型
```

- 与类型上界语法同样可用于 `where` 子句。
- class/interface类型上界可直接写在参数上，也可写在声明头之后的`where`子句；同一参数至多有一个class上界，并可有多个不同interface上界。上界必须是参数完整的exact reference application，例如`T : Base<String>`或`where T : Producer<Animal>`；不接受value type、函数类型、`Any`或另一type parameter。class上界已蕴含`ref`，并把class成员及其继承闭包加入bounded receiver能力。
- `value` / `ref`约束与任一class/interface上界互斥：同一类型参数不能同时携带二者；同一个kind bound也不能重复出现在inline与`where`位置。多个class上界即使文本上存在继承关系也不允许，必须保留唯一class bound并把其余能力写成interface bound。
- 无约束的类型参数默认接受任何类型（与 Kotlin 一致）。

### 13.10 `Ptr` 与 `FunPtr`

二者定义于 `scoop.core`，是 FFI 的基础辅助类型，均为值类型。

当前可执行target profile要求C ABI data pointer与code pointer都恰为64位、两类null都对应内部carrier的全零位模式，并保证合法非null地址与编译器/runtime内部64位raw carrier逐bit往返；因此`Ptr`的公开integer往返surface暂时使用canonical 64位`ULong`。内部data/code pointer仍分别使用带Raw/Code provenance的typed carrier，不是源码`ULong`值，尤其不能据此给`FunPtr`开放integer访问。这只是target与表示契约，不新增隐式pointer/integer转换：`Ptr<T>(raw: ULong)`是显式unsafe构造，`FunPtr`没有integer构造或转换；非null `FunPtr`只能来自本节下述合法入口。任何不满足该宽度、null表示或往返能力的target都必须在生成IR前报unsupported target；未来支持此类target必须另行修订本节source surface、C-FFI representation/storage契约与runtime callback token契约，是否引入何种native integer以及迁移哪些surface届时逐项决定，不能把固定64位`ULong`静默当作另一宽度的pointer word。

#### `Ptr<T>`

`Ptr` 能容纳一个 raw pointer；`addressOf` 用于取一个变量的内存地址：

```
@Intrinsic("core_ptr")
public struct Ptr<T : value> {
    public operator fun equals(other: Ptr<T>): Boolean {
        @Unsafe {
            return this.toULong() == other.toULong()
        }
    }

    @NoGC @Unsafe
    @Intrinsic("ptr_to_ulong")
    public fun toULong(): ULong

    @NoGC @Unsafe
    @Intrinsic("ptr_cast")
    public fun <U : value> cast(): Ptr<U>

    @NoGC @Unsafe
    @Intrinsic("ptr_load")
    public fun load(): T

    @NoGC @Unsafe
    @Intrinsic("ptr_load_offset")
    public fun load(offset: Long): T

    @NoGC @Unsafe
    @Intrinsic("ptr_store")
    public fun store(value: T)

    @NoGC @Unsafe
    @Intrinsic("ptr_store_offset")
    public fun store(offset: Long, value: T)

    @NoGC @Unsafe
    @Intrinsic("ptr_plus")
    public operator fun plus(offset: Long): Ptr<T>

    @NoGC @Unsafe
    @Intrinsic("ptr_minus")
    public operator fun minus(offset: Long): Ptr<T>
}

@Unsafe
@Intrinsic("address_of")
public fun <T : value> addressOf(v: T): Ptr<T>
```

- `core_ptr`登记为一个以exact GC-free value pointee type参数化、以data pointer表示的compiler-represented value family；它没有源码可见field或普通primary constructor，不能被字段访问、解构或copy update。源码`T : value`只表达kind，不能证明GC-free：每个concrete `Ptr<T>` application还必须递归证明`T`为GC-free；generic template中出现`Ptr<T>`时保存并向实例化者传播该typed deferred条件，所有type argument具体化后失败即为编译错误。`equals`是普通core body；其余上述方法均为compiler intrinsic且都是unsafe function（见13.3）。按13.1的规则`@Intrinsic`通常不得与其他注解共存；此处是单独说明的例外：这些intrinsic允许与`@NoGC`/`@Unsafe`组合。
- registry只为显式`Ptr<T>(raw: ULong)`提供一个call-shaped、`@NoGC @Unsafe`的特殊construction entry；它不是普通struct constructor，也不能由representation field合成。该entry是从integer显式制造data pointer的唯一形态，要求unsafe context及`raw != 0uL`前置条件。编译期常量零直接诊断；运行期值违反该unsafe前置条件时行为未定义。它不建立隐式conversion，未对齐、越界、悬垂地址、算术结果变为零及生命周期均由unsafe调用者负责。
- 该 construction entry 由普通名称查找得到的实际 `core_ptr` 类型声明提供，声明来自当前 Cone 或依赖时规则相同；导入别名保留该绑定，普通同名声明按既有遮蔽与重载规则处理。`T` 可显式给出，也可由期望的 `Ptr<T>` 类型推断（包括 `_`）；指向完整 `Ptr<T>` application 的非泛型 typealias 固定其类型实参，并作为非参数化候选参与选择。唯一值参数是 `raw: ULong`，允许位置或命名传参，不接受 spread、默认值或隐式整数转换。泛型正文及默认值保留显式构造与 pointee 类型，按既有规则向实例化者传播 GC-free 条件。
- unsafe context 与编译期常量非零检查对已选 construction entry 生效；不得因这些检查失败而跳过更具体的构造候选，转而调用同名的普通函数。未选候选不产生上述诊断或求值。
- 源码中的 `pointer + offset` / `pointer - offset` 分别按 `ptr_plus` / `ptr_minus` 的契约处理；`offset: Long` 以**元素个数**计，其数学byte displacement为`offset`与`sizeOf<T>()`数值的乘积，与非ZST C object pointer算术一致。带 `offset` 的 `load` / `store` 使用相同的元素偏移语义。对ZST pointee，任意offset的物理byte displacement恒为0且所得pointer bit值不变；`load`产生该exact ZST值，`store`不写payload byte，但receiver、offset、value仍求值，且unsafe调用者仍必须保证pointer非null、满足alignment/lifetime并指向相应逻辑place。算法不得用ZST pointer值变化表达迭代进度；`Ptr<Unit>`若需要逐byte移动必须先使用语义上正确的`Ptr<UInt8>`，不能把opaque `void *`自动当byte pointer；
- `addressOf` 是 intrinsic，且带 **lvalue 约束**：实参必须是参数、局部变量、全局变量，或值类型成员方法的 `this`，取的是该 place 实际存储的地址；对临时值、字面量、计算结果等非 lvalue 表达式调用是编译错误。对 `this` 取址时指向 3.3 规定的方法局部副本，不是调用方的 value 或 box payload。
- 泛型正文中的 `addressOf` 保留原 place 与 exact pointee type；尚未具体化的值参数按 `Ptr<T>` 的既有规则传播 GC-free 条件，不要求在泛型定义处得到具体布局。经过普通名称查找和重载选择后才应用这一 intrinsic 规则；命名参数和导入别名不改变取址对象，不得取为普通按值调用准备的临时副本。
- `Ptr<T>` 自身是值类型，因此满足 `value` 约束，可以出现在要求 `T : value` 的位置（包括 `Ptr<Ptr<T>>`）。
- **null与可空指针**：裸`Ptr<T>`没有null值，内部data-pointer carrier的全零位模式保留给`Option<Ptr<T>>.None`及inactive/zeroed storage。FFI边界上的可空data pointer必须用`Option<Ptr<T>>`表示；声明返回裸`Ptr<T>`的native函数返回null属于契约违反。布局由niche保证（见7.4）。
- **`void*` 与 opaque 类型**：`void*` 及 C 的 opaque handle（不完全类型指针）统一用 `Ptr<Unit>` 表示。

#### `sizeOf` / `alignOf`

```
@NoGC
@Intrinsic("size_of")
public fun <T : value> sizeOf(): ULong

@NoGC
@Intrinsic("align_of")
public fun <T : value> alignOf(): ULong
```

- 返回 `T` 的大小 / 对齐（字节数），编译期求值；ZST返回size 0和严格大于0的alignment。手工内存管理（配合 C 的 `malloc` / `free` 等）时不能把`malloc(0)`结果当成可取址ZST place，需按4.7自行提供至少1 byte且满足alignment的token。
- `T : value` 是布局查询的类型约束；含引用字段的值类型也可查询布局，不增加 GC-free 条件。泛型正文及默认值保留被查询的类型，单态化后按具体类型的实际布局求值。声明来自当前 Cone 或普通依赖时遵守相同规则。

#### `FunPtr<F>`

`FunPtr<F>` 声明一个 FFI 可用的 callback 指针，`F` 是 8.1 定义的**普通、非挂起且完全具体化**的函数类型：

```
@Intrinsic("core_fun_ptr")
public struct FunPtr<F>
```

- `core_fun_ptr`登记为一个由exact函数类型参数化、以code pointer表示的compiler-represented value family；它没有源码可见field、primary representation constructor或integer accessor。不能由零字段普通struct规则推出zero-size layout或任意构造能力。
- `core_fun_ptr`不提供任何源码constructor；`FunPtr<F>()`、`FunPtr<F>(0u)`与`FunPtr<F>(123u)`都没有合法candidate。裸`FunPtr<F>`始终非null；需要null或可选字段时使用`Option<FunPtr<F>>`并以`None`初始化。这保证7.4中的全零niche不是合法payload，`Some(value)`与`None`始终可区分。
- 用户不能直接构造`FunPtr`值；在期望类型明确为`FunPtr<F>`的位置使用顶层函数声明引用`::name`，由编译器直接生成非null原生callback地址。`::name`是8.1.4的中性源码语法，不具有固有的`FunPtr`类型；因此`val callback = ::name`仍推导为managed函数值，要保存原生地址必须由类型标注、参数类型、返回类型等上下文提供`FunPtr<F>`期望类型。目标声明签名必须与`F`**精确相同**，不应用8.1.1的函数类型型变。14.3的`ForeignCallback.function`是编译器生成的另一种非null值，指向必须与opaque context cookie配对使用的managed-callback trampoline；它只能从已注册值读取，不能用构造器仿造。声明返回裸`FunPtr<F>`的native函数返回null同样属于契约违反。
- `F` 的每个参数与返回类型必须满足 C-FFI-safe 约束；`Unit` 只允许作为返回类型。`FunPtr` 描述的是 C ABI callback 地址，不是 Scoop ABI managed callable；函数类型 `F` 在这里仅描述 native signature，本身不会作为 managed 引用穿越边界。
- 可空函数指针用 `Option<FunPtr<F>>` 表示（niche 优化见 7.4）。
- native-address resolution 的目标必须是带 `@NoGC` 的普通顶层命名函数，且**不能是 generic、挂起、extern、成员或扩展函数**。lambda、匿名函数、局部函数、任何绑定引用以及已存在的 managed 函数值都不能作为非 null `FunPtr` 的来源。违反这些约束是编译错误：FFI 回调不得与 GC 交互，generic 函数没有单一具体符号，挂起函数只有编译器内部的 hidden continuation ABI，而 closure 还需要原生 ABI 中不存在的 managed 环境参数。编译器不自动生成 closure 或挂起 callback wrapper。
- `FunPtr` 不提供 Scoop 侧 `invoke`；它只用于传递/存储 native callback 地址。由`::name`得到的M12静态callback仅允许原生方在发起extern调用的同一已注册线程上同步调用；保存后异步、跨线程或在Scoop程序退出后调用不隐式获得安全性。M13只有14.3 registration返回的`ForeignCallback.function + context`配对具备对应token/attach协议，单独保存或调用其中的function而不携带仍存活的配对context同样非法。

```
// C 侧：int64_t compare_int(int64_t a, int64_t b,
//                         int64_t (*cmp)(int64_t, int64_t))

@Extern(lib = "sample", name = "compare_int")
fun compareLong(a: Long, b: Long, cmp: FunPtr<(Long, Long) -> Long>): Long

@NoGC
fun cmp(a: Long, b: Long) = if (a > b) { 1L } else { 0L }

@Unsafe
fun caller() {
    compareLong(10L, 10L, ::cmp)
}
```

---

## 14. Scoop ABI FFI

Scoop ABI（见 13.8）供能识别 Scoop 类型信息并与 GC 交互的外部函数使用，主要消费者是 runtime 与核心库的实现者。

### 14.1 GC 基础设施 `scoop.core.gc`

```
package scoop.core.gc

struct PinnedPtr<T : ref>(val raw: ULong)
struct GcHandle<T : ref>(val raw: ULong)

@Unsafe fun <T : ref> pin(v: T): PinnedPtr<T>
@Unsafe fun <T : ref> unpin(p: PinnedPtr<T>): T
@Unsafe fun <T : ref> getGcHandle(v: T): GcHandle<T>
@Unsafe fun <T : ref> releaseGcHandle(h: GcHandle<T>): T
```

- **`pin`**：将对象固定在 GC 堆上（不移动、不回收），返回 `PinnedPtr`——其 `raw` 就是对象的实际地址，pin 标志记录在对象头（见 runtime spec 3.4），可以直接交给 FFI 当裸指针使用。固定对象会影响 GC 效率且可能造成内存泄漏，固定时间应尽可能短。
- **`unpin`**：按地址清除 pin 标志并取回对象，O(1)；之后该对象可以正常参与 GC。
- **`getGcHandle`**：获取对象的 GC handle。handle 被视为对象的引用：对象存在未释放的 handle 时不会被回收，但 GC 可能在堆上移动它。一个对象可同时存在多个 handle，全部释放后才可能被回收。
- **`releaseGcHandle`**：释放 handle 并取回对象，不再阻止回收。
- `PinnedPtr` 与 `GcHandle` 是不同的类型，混用（如 `unpin` 一个 `GcHandle`）是编译错误。二者都是只含一个 `ULong`（即`UInt64`）字段的 GC-free 值类型，C ABI 与 `ULong` 一致，可以直接出现在 C ABI 签名中（13.4 的 C-FFI-safe 约束）。这一透明 C 表示不改变 Scoop typed ABI：二者仍按实际声明字段形成普通非空 struct，参数与返回按 14.2 使用 indirect aggregate；不能因 core 身份或 C 表示将其变成 Scoop 标量。
- 取舍：短期持有并需要裸指针时用 `pin`（O(1)，但阻碍 GC 移动）；长期保活且允许移动时用 `GcHandle`。
- Scoop ABI extern 的同步调用期间若只借用 direct ref，调用方不需要显式 pin 或 handle；被调方需要跨 safepoint或调用结束保存引用时才使用 14.3 的 native root、pin 或 handle机制。
- handle 取回对象时类型 `T` 来自 handle 的类型参数，编译器无法校验其真实性——这层正确性由 runtime 作者保证。

### 14.2 调用约定

Scoop ABI FFI 的 caller side（Scoop 托管代码一侧）必须生成 typed native-borrowed call 框架：

- 调用前把所有调用后live ref及direct-ref实参写入显式、可更新的caller-root frame；含ref返回值使用调用前已清零并一同发布的typed result storage；
- machine call仍由statepoint rewrite处理并带唯一ID，但其`gc-live`/`gc.relocate`必须为空；冻结managed栈段只以caller-root frame为真相来源，不能同时从另一份statepoint spill恢复引用；
- M12 单mutator实现不插线程状态转换；M13多mutator实现发布live caller roots并在调用期间进入`native-borrowed`，返回managed前检查GC epoch。它不得进入C ABI使用的`native-safe`，也不得省略direct-ref callee在显式runtime入口所需的native-root协议；
- native返回含ref结果时，caller在仍处于`native-borrowed`且本线程尚不能被视为quiescent时立即写入已发布result storage；leave/epoch握手后只从caller-root/result slot reload，再移除root frame；
- `@NoGC`只保证native callee不调用GC/runtime/managed callback；M15多mutator moving实现仍保留native-borrowed transition、caller-root publication与返回reload，不能把该边界降为无root的普通NoGC call；
- machine callconv 初版使用 LLVM 默认 callconv `0`；
- 调用点本身**不要求 unsafe context**（`abi = "scoop"` 的 extern 函数不是 unsafe function，见 13.4）。
- `@Extern` native callee不得把Scoop异常展开回generated caller；初版遇到这种unwind终止进程。源码可见的失败必须由managed函数/wrapper在native返回状态后构造并抛出，runtime-only的no-return throw入口不属于可由用户声明调用的Scoop ABI FFI surface。

Scoop ABI extern 的参数与返回值使用普通 Scoop typed ABI，不经过 C ABI storage bridge：ref value 是直接 managed pointer；aggregate/value return沿用普通 Scoop 函数的 typed return storage规则。Darwin/AArch64 与 Linux/amd64 profile 中，scalar、ref/raw pointer/function pointer 与 niche enum 直接传递；所有非空 tuple、ordinary struct、tagged enum 与异常记录都通过 caller-owned、按 exact layout 对齐的间接 storage 传递，间接返回 storage 位于所有源码参数之前；Unit 返回为 machine void，其他 ZST 参数/结果只保留 typed identity而不传payload。被调方必须按同一 physical signature 实现该 ABI，不能把同形 C struct 的按值参数/返回直接用作 shim，也不能假设 C 编译器会为它选择与 Scoop value ABI 相同的寄存器或栈位置。

这里的 Scoop ABI 仍是普通、单次进入并在返回前完成的 FFI 调用约定，不是 8.2 所述挂起函数的 hidden continuation ABI。`abi = "scoop"` 不放宽 `@Extern` 与 `suspend` 的互斥规则，也不提供自动 continuation / callback wrapper。

注意：以上是**用 LLVM 实现时**需要的策略（LLVM GC / statepoint 体系的术语），描述的是参考实现的代码生成要求，不是语言语义本身。

### 14.3 direct ref、native root 与 safepoint

- Scoop ABI FFI 函数的机器码中**没有 safepoint poll**（它可能是用其他语言写的）。传入的 direct ref 是调用期间的借用 managed value：在被调方尚未执行可能触发 GC 的 runtime 调用或回调 Scoop 代码前，可以直接读取，无需 pin；不得写入长期存储或在返回后继续使用。
- 若被调方需要让某个 direct ref 跨越可能触发 GC 的操作，必须先把它写入可寻址的 **native root slot** 并把对应 root frame登记到当前线程。含managed leaf的内联aggregate/value place不能被拆成登记后仍从旧aggregate读取的临时ref；它使用`RecursiveRegion { stable base, byte extent, NonEmptyRefScan }` frame，由collector以与TypeDescriptor相同的递归slot visitor原地更新。登记/移除两种root frame本身都不得分配或触发GC；操作返回后，被调方必须从slot或region base重新读取，不能继续使用登记前保存的裸指针/aggregate副本。
- root frame 必须按栈严格嵌套，并在所有正常/错误出口移除。需要把引用保存到本次调用之后时，使用 `GcHandle`；需要把稳定裸地址交给 C ABI 或跨 safepoint保持同一地址时，使用 `PinnedPtr`。两者都不是普通 direct-ref 参数的默认表示。
- GC 只在 managed safepoint或显式 runtime 入口协调线程；实现不得在 Scoop ABI native code的任意两条普通指令之间无握手地移动对象。未来的并行/并发 collector 必须把 native root frame 与线程握手纳入同一协议，不能通过要求所有 Scoop ABI 参数预先 pin 来回避该契约。
- `abi = "scoop"` 的调用点安全的前提是**被调方遵守上述契约**。这类底层 extern 声明按约定仅由 runtime / 核心库作者使用；违反借用、root frame或保活规则是 runtime ABI 错误。
- **GC-aware managed callback** 使用独立 registration协议：注册操作接收 ordinary、非挂起的 managed closure及与其 concrete函数类型匹配的 compiler-generated invoke adapter，返回 runtime-owned、GC-free的 opaque token。`CallbackRegistrationKey`固定包含lexical parent、source conversion `StructuralDefinitionPath`、允许binder的source C signature、context index、允许binder的managed signature shape与mode；`PersistentCallbackRegistrationId = DomainSeparatedCborHash("scoop-callback-registration-id-v1", CallbackRegistrationKey)`只标识这一个source registration。一次fully concrete materialization另以`CallbackApplicationKey { registration: PersistentCallbackRegistrationId, context: CallableMaterializationContext }`和`PersistentCallbackApplicationId = DomainSeparatedCborHash("scoop-callback-application-id-v1", CallbackApplicationKey)`标识。`NoSubstitution`只允许两份source signature均binder-free；否则普通generic callable与generic delegated initializer/ensure分别使用覆盖完整binder stack的`Application`与`InitializationApplication`。registration与application不可互换，同一registration可以产生多个application。token内部以 `GcHandle` 保活 closure；native代码不得缓存 closure裸地址。
- native侧通过静态 C ABI trampoline和显式 context/user-data槽携带该 token。MIR以callback application为主键保存exact managed signature与固定storage/status ABI；每个application拥有自己的managed adapter identity，不能因签名相同而合并。LIR才在当前target下得到canonical C source-storage signature，并保存`{ application, CanonicalCAbiSignatureFingerprint, GeneratedBridgeUnitId }`；签名与context index相同的多个application必须复用同一trampoline unit/source recipe，但各producer仍使用自己的strong bridge atom/symbol。trampoline进入 runtime后 attach尚未注册的 foreign thread、切换到 managed执行状态、从 handle重新取得 closure并调用 typed adapter；返回 native前恢复线程状态。参数与返回值必须满足 C-FFI-safe约束，closure body内 GC正常生效。
- token具有显式 retain/release与 use-after-release错误边界；callback抛出的 Scoop异常必须在反向边界内捕获并转换为 status/受管异常handle，不得展开穿越 C frame。运行时终止后调用 token是 ABI错误。首版只支持原生API提供显式 context/user-data槽的形态；无此槽的任意 closure导出需要后续动态 trampoline或 slot registry。
- 该协议不改变 `FunPtr`（13.10）：M12 的 `FunPtr` callback仍是静态、同步、同线程、`@NoGC`路径。M13 落地 registration、foreign-thread attach/detach和多 mutator STW协调；suspend closure与 suspend FFI仍不支持。

M27 为上述协议增加 8.3.5 的 Context 传播：注册成功前捕获当前有效 binding 的不可变快照，token 通过现有 handle 表保活非空快照；retain 共享同一快照。每次 invocation 在调用 closure 前从快照建立独立 TaskContext，正常返回和异常转换均恢复原 Context；Reusable 并发调用不共享可变绑定。token 的 owner/active lease 都结束后同时释放 closure、快照和 failure handle。同步 native 往返保持外层 Context，但经 callback 反向重入时使用 registration 快照。此变化不改变 `ForeignCallback<F>`、C trampoline 签名或源码的 context/user-data 参数；私有 managed adapter 的实现边界见 runtime spec 9.4。

M13 的 core 源码形态保持为：

```
enum ForeignCallbackMode { Reusable, OneShot }
enum ForeignCallbackState { Registered, Active, Completed, Failed }

struct ForeignCallback<F>(
    val function: FunPtr<F>,
    val context: Ptr<Unit>
)

@Unsafe
@Intrinsic("foreign_callback_register")
fun <F> foreignCallback(
    callback: Any,
    contextIndex: Long,
    mode: ForeignCallbackMode
): ForeignCallback<F>

@Unsafe
@Intrinsic("foreign_callback_retain")
fun <F> retainForeignCallback(callback: ForeignCallback<F>): ForeignCallback<F>

@Unsafe
@Intrinsic("foreign_callback_release")
fun <F> releaseForeignCallback(callback: ForeignCallback<F>)

@Unsafe
@Intrinsic("foreign_callback_state")
fun <F> foreignCallbackState(callback: ForeignCallback<F>): ForeignCallbackState

@Unsafe
@Intrinsic("foreign_callback_failure")
fun <F> foreignCallbackFailure(callback: ForeignCallback<F>): Throwable?
```

- `F` 必须在registration调用处显式给出，是ordinary、非挂起、完全具体化且逐项C-FFI-safe的native callback函数类型。`contextIndex`和`mode`必须为编译期常量；被选参数必须精确为`Ptr<Unit>`；
- `callback: Any`只是core声明无法表达“从`F`删除context参数”的占位，不执行装箱。HIR删除`F`中`contextIndex`对应的参数，以所得ordinary concrete函数类型检查managed closure；context cookie不作为实参传给closure，返回类型保持不变；
- 若这次调用位于generic template中，上一条“完全具体化”是对每个实际callback application替换后的要求；Export HIR的registration仍保存binder-capable signature和source site，而不能提前用某个first application的exact type或trampoline unit改写registration identity；
- `ForeignCallback<F>`是GC-free值，但不是单个C ABI聚合。调用native API时分别传`function`与`context`；有效值只能由`foreignCallback`产生，用户不能直接构造；
- `ForeignCallback`是compiler-validated core类型，其`F`可在core声明内作为deferred callback signature用于`FunPtr<F>`；每个实际应用仍必须具体化为合法函数类型。这不引入一般性的`function` kind bound，用户generic类型不能据此用未约束参数绕过`FunPtr`检查；
- `foreignCallback`与`retainForeignCallback`各产生一份逻辑ownership。普通值复制只是借用别名，不增加计数；每份ownership恰好release一次。`Reusable`由调用者在native API完成unregister并确认不再回调后释放；`OneShot`在native创建成功后把worker ownership转移给callback入口，创建失败则仍由调用者释放。join侧若需观察结果，必须在转移前另retain observer ownership；
- callback抛出时，trampoline按C签名返回全零值，token保存首个managed异常。完成同步后，observer用`foreignCallbackState` / `foreignCallbackFailure`读取并可在Scoop侧重新抛出；最终release通常置于`finally`。stale token、signature不匹配、one-shot重复调用或runtime终止后调用均为runtime ABI错误；
- `foreignCallback`只接受普通closure，不接受`suspend`函数值。普通callback可以捕获并调用`Continuation<T>.resume`；这仍不是suspend callback或suspend FFI。

### 14.4 示例：直接输出 managed `String`

Scoop 侧直接声明 managed-ref 签名；不需要 wrapper、pin 或 unsafe block：

```
@Extern(name = "scoop_rt_write", abi = "scoop")
fun write(message: String)
```

runtime 侧按 Scoop 的 `String` 对象布局直接接收引用：

```c
void scoop_rt_write(const ScoopString *message)
{
    fwrite(message->data, 1, message->len, stdout);
}
```

该函数只在调用期间读取 `message`，不分配、不调用可能触发 Scoop GC 的 runtime入口、不回调 Scoop代码，也不保存引用，因此无需 native root frame。若以后在写入前后增加任一可能触发 GC 的操作，必须先按 14.3 把 `message` 放入 native root slot，并在操作后重新读取更新后的值。

---

## 15. 明确排除（再次汇总）

为方便实现者，以下内容 Scoop 不支持：

其中GC finalizer是永久排除项，不是尚未实现：对象不可达时不会执行`finalize`、析构方法或任意managed回调，也不存在对象复活语义。native resource应显式`release`/`close`并用`try/finally`保证正常路径清理；9.1.6与runtime spec 3.8只提供同步、GC-free、非及时且不可复活的release hook作为遗漏清理的兜底。该hook只能附着于受限的普通final class；值类型复制、继承hook chain和managed `this`均不属于该机制。

| 排除项 | 替代方案 |
|---|---|
| `enum class` | `enum`（4.2） |
| `value class` / `inline class` | `struct`（4.1） |
| `data class` | `struct`（4.1）+ 内建解构（4.6） |
| `null` / 平台类型 `T!` | `Option<T>`（第 7 章） |
| `lateinit` / 隐藏未初始化property状态 | `var p: T?` / `var p: Option<T>`（省略initializer即为`None`，9.1.1） |
| 省略visibility即public | 省略visibility即internal；对外API显式写`public`（9.1.5） |
| 普通字符串的 `$` 插值 | f-string（第 6 章） |
| struct 字面量 `S { f: v }` | 构造函数 / 命名参数（4.1.1） |
| `expect` / `actual` | 无（单平台） |
| JVM 互操作注解与 SAM 转换 | 无 |
| 运行期反射 | 编译期/单态化机制 |
| GC finalizer、析构回调与对象复活 | 显式 `release` / `close` + `try/finally`；受限GC-free `release { ... }`仅作兜底（9.1.6） |
| struct 的 `init` 块 / `var` 字段 | 构造函数内逻辑 / `val` |
| 对值类型使用 `===` | `==`（结构相等） |
| generic declaration/use-site `in`、`out`、`*` | nominal generic application始终invariant；使用带bound的generic callable或显式非generic接口（见3.2） |
| `Option<T>` 的智能转换 | `when` 解构（7.3） |
