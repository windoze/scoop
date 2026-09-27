# Scoop 语言规范

静态存储与初始化失败根按其实际值类型引用 layout/scan。当前 Cone 只发射自身拥有的布局与扫描定义；外来类型的静态根复用共有依赖查询取得的完整 value-layout 和 scan 记录，保留实际 provider、typed identity、定义与 relocation，不因本地持有该类型的值而重发射 foreign Strong。layout/scan 指纹节点引用已经解析的实际记录，不要求该类型在当前 Cone 定义；指纹补丁目标仍须属于当前产物。MIR 必须携带生成失败根所需的实际 Any 声明，LIR 不再缺省重建固定 core 身份。static-storage 语义记录新增 field 32 保存 layout provider，完整记录使用 fields 1～32；语义投影使用 fields 1～10 与 32。共有 strong-production 两种格式升级为 /11、/12，旧 /9、/10 产物和缓存重建。runtime C ABI、String 表示、初始化状态与失败缓存语义不变，不引入 ODR 或多 image 启动。

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

产物 reader 的 MIR 类型和 callable 校验使用同一次 HIR 类型基础与继承图构建。按真实依赖顺序解析完整 MIR 类型、callable 和 dispatch 后，在同一 provider 的借用范围内核对类型表示、有限 shape、方法、构造器、object、派发、equality 与初始化契约；不再为类型和 callable 分别完整重建未变化的 HIR。后续 LIR 消费完整的 MIR 结果，保留各自需要的格式、引用、签名、ABI 与对象检查。此调整不改变 wire、profile、runtime C ABI 或 String 表示，也不增加来源工厂、资格状态或缓存凭证。

完整 LIR 输出已保存 canonical foundation，codegen 不从同一未变化的模块再次构造该 foundation。LIR 在既有 Strong 能力边界检查 ODR 后，只追加当前 Cone 的 Strong 定义，不再次扫描未变化的 callable/ODR 记录。production 的符号表直接使用本次生成的定义表，对象分区直接使用 production 的符号记录；这些数据在 production 组合边界检查后，codegen 按 typed definition、atom 和 symbol 解析每项实际发射引用，复用完整记录，不重建整份预期表或再次完整比较。C bridge 与 C layout 入口只检查实际 C ABI、callback 声明及所需类型关系；callback 指令的 typed 引用与操作结果、Scoop CFG、dispatch、safepoint 与 root plan 在对象代码生成边界检查；不在每个无关入口重复完整验证。外部产物读取的格式与引用检查保持，wire、fingerprint 字段和 runtime ABI 不变。

产物的 identity graph 从 manifest 的当前 producer、实际直接依赖和已经读取的依赖实体构成；不无条件注册 CORE 身份，也不为 CORE 设置单独的重复过滤规则。CORE 与普通 provider 的声明使用相同的 typed 引用解析，缺失依赖、身份冲突及非法引用由共有格式与引用检查报告。默认 core 依赖仍由正常构建与前端依赖发现加入 manifest；本项不改变 wire 或 runtime ABI。

代码生成在入口对本次完整且不可变的 LIR、目标 profile 和 executable 入口完成一次必要验证，然后按实际定义分成函数与非函数对象。各成员直接消费同一 LIR，不因发射另一个对象再次完整遍历类型、ABI、CFG、GC roots、safepoint 或身份表；LLVM 变换后的 IR 和新生成的对象字节仍在各自边界检查。generated-C 源码入口同样不在内部 helper 重复整模块验证。这一职责调整不改变产物格式、runtime C ABI、String 表示或链接语义。

普通 library Cone 导出的无类型参数 struct 可以直接作为下游的参数、返回值和嵌套字段类型，并以真实声明身份解析其成员。final 成员的默认参数、命名参数和 operator 与本地调用使用相同语言规则，private 成员仍不可由外部直接访问；同布局或同名类型不能替代声明身份。实现和产物版本见 [实现规范](SCOOP-IMPL-SPEC.md)。

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。 依赖查询直接使用真实 `ConeIdentity` 与 callable、property、type-alias 的类型化声明 ID；选择集合按这些 ID 保存完整接口和实际依赖路径。删除独立 world/projection/selection 品牌、仅为品牌服务的计数器和错误，以及从声明 ID 再映射到局部 u32 的三套重复表。候选选择依照当前依赖目录中的实际声明与表示，不要求由同一查询实例铸造；直接依赖的名称可见性、转导出路径、实际 provider 和引用完整性继续按共有规则检查。HIR 依赖目录、候选和已选声明只保存实际 provider 与类型化声明引用，不逐项复制 artifact 坐标/fingerprint 凭证；provider 身份直接来自已导入的共有 foundation，不再提供独立 certificate 或重算坐标身份。直接依赖与传递依赖使用同一输入数据和查询实现，直接依赖集合只决定当前源码可见的 package/public binding；不以分离的输入、视图或 seed 包装授予枚举资格。转导出保留实际 binding route，删除逐候选的 provider 凭证、重复 terminal 声明及其包装；候选选择使用已解析的声明目录，不重放未变化的 route 与 provider 证明。HIR→MIR 使用同次编译的依赖快照及完整声明：共有依赖选择在关联实际 MIR 定义时核对一次目标、逻辑签名和 GC effect，lowering 随后按实际 provider 与 typed target 消费同一记录，不重复比较未变化的声明和签名，也不再次比较来源凭证。缺失定义及最终 MIR 输出的结构、类型与外部引用检查仍在各自边界保留。普通构建依赖记录与缓存 fingerprint 继续承担定位和失效职责。MIR 外部 callable 引用保存实际 provider 与类型化声明，不另映射到带会话品牌的局部编号；调用位置保留 GC effect。MIR 类型与 LIR layout/ABI 选择按实际 provider 和 typed target 直接返回完整记录，不先铸造并验证中间 handle；依赖闭包、类型、ABI、物理引用和 GC 契约仍在其消费边界检查。同一声明或 target 在另一选择集合中是否存在，按实际目录查询决定，不依据集合生成顺序或计数器。该进程内数据简化不改变 wire/profile、实体身份或 runtime ABI。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

Strong production 的两种表示使用 `/11`、`/12`，删除初始化专用 ABI field 11 和外部服务表 field 12，section 只保留 field 2～9 的八字段 product。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

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

标准库的其他部分（集合框架、IO、并发工具、序列库等）不在本规范范围内。

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
- 上下文参数：context parameters（见 8.3）；
- 注解与反射以外的元编程语法（反射仅保留语法级支持，见 2.2）。

### 2.2 排除的内容

以下 Kotlin 功能**不属于** Scoop：

- 平台对接：
  - `expect` / `actual`（multiplatform）；
  - `@JvmStatic` / `@JvmField` / `@JvmOverloads` / `@Throws` 等一切 JVM 互操作注解；
  - `external` 声明（FFI 通过 `@Extern` 注解机制提供，见第 13 章）；
  - Java 互操作语义（SAM 转换、平台类型 `T!` 等）。
- 运行期反射：`KClass`、`::class` 的运行期反射 API、`kotlin.reflect` 体系（单态化泛型 + 无 JVM 运行期使其不成立）。
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

- 泛型在编译期**单态化**实例化：每个具体类型实参生成一份专门的代码。
- function、class、struct、enum与interface都可以声明类型参数。generic class/struct/enum的constructor或variant、base/interface application、字段与成员都可以使用宿主类型参数，generic interface的父interface与成员也可以使用宿主类型参数。每个fully specialized nominal application生成独立的concrete identity和成员实现；class还生成对象布局、TypeDescriptor与分派表，struct/enum生成完整value layout与GC-free/扫描信息，interface生成独立TypeDescriptor与itable key identity。
- Scoop没有预定义`Self`类型、associated type或“当前实现者类型”的隐式占位符；`Self`也不是关键字，若出现在源码中只按普通名称解析。generic/interface契约若需要表达某个类型关系，必须用显式nominal type application或显式type parameter表示，编译器不执行`Self := 实现类型`替换。
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
- 类型实参必须同时满足参数的全部上界；class/interface关系按普通继承、显式conformance及完整application identity判断，value type同样只按显式声明的interface实现判断。泛型推导把上界纳入整组约束求解，不得先任选一个类型、再把bound失败降为警告或回退为`Any`。
- receiver为有界type parameter时，成员候选只来自唯一class上界、interface上界及其继承闭包；不加入`Any`成员或实际类型未在bound中声明的能力。generic template中的bound member在实例化时解析为concrete direct/virtual/interface call；单态化不需要runtime dictionary，但不取消actual concrete type本来具有的动态分派语义。
- 每个合法且参数完整的exact class/interface application都是普通reference type，可以直接作为变量、参数、返回值、字段、cast目标和upper bound。Scoop不引入`dyn`/trait object语法、object-safety分类或可空witness；所有合法interface成员仍可经concrete、exact interface或bounded receiver调用。
- interface方法现阶段不能声明自己的type parameter；`interface I { fun <T> f(value: T) }`在声明处即为编译错误。interface宿主可以generic，例如`interface I<T> { fun f(value: T) }`，完整application `I<String>`中的方法可正常itable分派。这是method-level generic dispatch ABI尚未定义的功能边界，不是允许声明后再限制调用形态的object-safety规则。未来开放时必须同时支持interface与bounded receiver调用。
- non-interface generic method必须non-virtual。class generic method必须语义为final；generic method不能声明为open/abstract/override，不能实现或覆盖vtable/itable slot。struct/enum方法本来即为final。调用根据exact receiver application与完整method argument使用direct dispatch，运行期派生class不能override目标。
- class/struct/enum泛型宿主上的泛型成员函数同时拥有两组类型参数：宿主类型实参由接收者的exact静态application确定，方法类型实参由整组调用实参推导；两组实参共同组成该方法的单态化身份，顺序固定为“宿主类型实参在前、方法类型实参在后”。两组参数使用不同semantic identity，方法参数不得与宿主参数重名。调用点显式列表只写method自身参数，可整组省略，或覆盖全部位置并在待推断位置写`_`；两组参数的bound一起验证。interface方法现阶段没有第二组参数。
- top-level、local与extension generic function，以及上述non-interface generic method，都可以使用inline upper bound与`where`。generic method的callable reference必须由期望函数类型唯一确定method全部实参，得到的是某个concrete函数值；Scoop没有first-class polymorphic function value。
- 所有type parameter declaration都不能写`in`/`out`。callable参数与返回类型在推导中的方向由constraint solver处理，不通过声明点variance修饰符表达；普通函数类型自身的参数逆变/返回协变继续按8.1.1处理，它不是nominal generic application之间的variance。
- `is` / `as` / `as?`不擦除generic argument，generic nominal目标必须是参数完整的exact application。例如`Box<Int>`与`Box<String>`、`I<Int>`与`I<String>`是不同检查目标，前者不会仅因`Int <: Any`匹配`Box<Any>`。generic body中的type parameter在普通单态化后引用concrete TypeDescriptor；不存在裸generic、star或projected runtime descriptor。
- 单态化必须结构上保证实例化闭包终止。同一generic callable递归SCC中的每个调用环，把宿主参数与callable参数组成的完整向量代回起点后必须逐项保持identity；普通直接/互递归因此复用同一concrete实例。参数替换非identity的环属于当前不支持的polymorphic recursion，在template定义检查时报错。非递归调用边仍可任意变换实参。编译器不得用递归深度、实例数量或超时阈值决定源码是否合法。

#### 3.2.1 非泛型透明 `typealias`

跨 Cone 的 `Alias(...)` 与本地别名遵循相同规则：先按真实 typed target 展开，再对目标声明的构造器执行普通候选决议。外来 alias、本地指向外来类型的 alias 及其链式组合保留目标的构造器身份、默认参数、访问域与 GC effect，不生成转发构造器。

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
- 跨 Cone 非泛型 enum 使用相同的变体构造、import/typealias、期望类型和默认参数规则；值构造按实际声明身份生成，不以 provider 来源授予额外资格。
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
- tuple 不支持在源码中显式声明implements列表，也不支持命名字段，因此不实现`ToString`或其他普通interface。需要命名或实现interface请使用struct。

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
- binding、`_`、rest补位与`else`均为wildcard；fixed-width integer是有限域，实现以已出现literal singleton与符号化other partition计算覆盖，不实际枚举`2^W`个值。若不同无guard literal确已覆盖全部bit pattern，则无需wildcard；否则missing witness必须是该exact signed/unsigned数学次序中的真实缺失值，unsigned witness始终使用`u`后缀以保证可按subject type重新解析。String是无限开放域，有限literal arm仍必须有覆盖余值的wildcard；Char/Float/Double literal coverage由其进入已实现子集时另行规定；
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
- 变体名可省略 enum 类型前缀（`E.`），编译器按被匹配值的类型解析；存在歧义时需写全限定名。
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
- lambda 参数支持 4.6 的解构模式。传入的单个参数值作为该pattern的subject且只处理一次；解构失败不产生运行期分支，参数静态类型必须能按该模式解构，否则是编译错误。投影、component调用、hidden temporary、异常与挂起语义均遵守4.6，不改变函数的源码/函数类型元数或该参数对应的单个typed ABI classification entry。
- lambda 的值是 body 最后一个表达式的值；期望返回 `Unit` 时最后一个表达式的值被丢弃。匿名函数使用普通函数的返回规则。
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

### 8.3 上下文参数（context parameters）

- 支持 Kotlin 的 context parameters 语法：`context(ctx: Ctx)` 声明函数所需的隐式上下文，调用处由编译器从作用域解析。
- 上下文参数参与重载决议与类型推断，规则与 Kotlin 一致。

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
- 依赖候选期望类型的lambda、匿名函数、callable reference、裸enum variant（包括`None`）、空数组、整数字面量和嵌套generic构造作为postponed argument处理。求解器先用其余约束推进固定点，再用候选给出的完整期望类型检查postponed argument；lambda body结果只决定候选是否适用，不提供额外的“按lambda返回类型优先”规则。
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
- `return`不能退出property initializer、`init`或constructor body；`throw`合法。所有这些体及delegation都是ordinary、safe、non-suspend上下文；可以调用普通函数、分配、触发GC，也可以构造尚未执行的suspend task，但不能立即调用suspend函数。`startCoroutine`本身是同步普通builder，其启动计算不成为尚未完成的初始化步骤。

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

- `object O`同时声明一个nominal ref type与唯一singleton value，二者identity类型化且不同。object不能声明type parameter或primary/secondary constructor，可以继承一个class并实现interface；base constructor后按9.1.1执行property/delegate/`init`。`O`不是普通constructor，`O()`非法；
- 依赖 Cone 中的 object 遵守同一规则：类型位置引用原 nominal，值位置引用原 singleton value；二者来自同一声明时不构成值查找歧义。每次值访问先确保提供方的初始化单元成功，再读取其已发布根。转导出、默认参数展开、成员访问和下游再次发布都保留原实体身份，consumer 不创建第二个实例或初始化单元；
- class/struct/enum/interface/object至多有一个`companion object`，省略名称时为`Companion`。companion是独立、非generic singleton，不捕获host instance/primary parameter/type parameter，也不按generic host application复制；内部generic function仍可自行声明type parameter。`Host.member`可在无冲突时forward到companion，`Host.Companion.member`或显式名称始终明确；companion member不进入instance lookup或继承；
- 跨 Cone 的 companion、嵌套声明和转发按实际 host 的静态命名空间解析。`Host.Factory`、`Host.Companion`、导入别名及类型别名最终引用同一个声明与 singleton value；`Host.member` 只查该命名空间实际导出的转发 binding，再使用成员声明所属 object 的 receiver。限定路径本身不初始化 host，const 读取也不触发单例初始化。限定或直接导入的 companion 属性赋值、复合赋值与自增均先求值并保存一次实际 object receiver，再求值右值并调用对应 getter/setter。可见性、值遮蔽和不向 instance lookup 转发的规则与本地声明相同；

`hir/cross-cone-interface/30` 补齐 object 的声明种类：source-shape 的旧 Object tag 8 退役，新 tag 9 保留 field 1=value、field 2=声明序字段，新增 field 3=`Standalone(1)` 或 `Companion(2)`；host 沿已有声明 key 的 typed owner 查询。命名 companion 发布名称与 `Companion` 两个普通 type/value binding，object 的公开方法和属性进入自身静态 binding 表。共有命名空间在本 owner 无同名 binding 时，沿已声明的 companion 关系转发其直接 binding；不复制成员声明、不用名称或初始化 metadata 推断 companion。旧 `/28` 产物与缓存重建，退役 tag 不复用，runtime ABI 不变。
- body可以声明static nested class/struct/enum/interface/object。nested declaration没有implicit outer receiver或outer type parameter；需要关联时显式声明参数。generic outer名称可作为owner qualifier而不构成裸generic application。`inner class`、anonymous/local object/type及implicit outer capture不支持；
- ordinary top-level stored/delegated property可以是`val`或`var`、可以包含managed ref，使用compiler-managed hidden storage/accessor并进入global root表；它不可`addressOf`。`@Global`/`@ThreadLocal`仍只表示13.6的显式可寻址GC-free raw storage，`@Extern`仍只表示C data symbol；这些storage形态不能带普通accessor/delegate或与ordinary property混用；
- 需要runtime求值的top-level property使用`StaticInitialState::ZeroedForRuntimeUnit`：其完整storage先以canonical zero/null carrier登记，在全部Cone的image/stackmap/type/root/init metadata、GC与主线程就绪后、`main`前由对应unit exactly once求值并写入。只有无需执行Scoop代码、无需读取ordinary property且可直接编码为目标静态数据的literal/内建纯常量表达式、immortal String ref及Option `None` shorthand可省略unit；这些声明必须改用`StaticInitialState::EncodedStaticValue`，不能仅因为最终bits为零冒充前一分支。Encoded状态包含恰为storage allocation extent的canonical target-representation template（padding、ZST token及managed-ref位置为零）和按pointer offset排序的typed immortal relocation；非null managed ref只能重定位到已登记immutable String对象的精确object start，`None`不产生relocation，不允许任意heap/interior ref。链接与runtime在执行任何managed代码前验证template、relocation与实际初值一致；验证完成后该storage无需cell/unit即可作为合法初值读取。优化器不得因事后fold而改变有可观察求值的初始化语义。文件之间没有source order；二进制判等与同Cone排序唯一使用kind-specific `PersistentInitializationUnitId` bytes。其canonical declaration/specialization key已经编码origin `ConeIdentity`、owner chain、package、kind与name，只有file-private owner再加入标准化Cone-relative source identity；canonical Cone coordinate与declaration path只形成独立的稳定诊断path，不参与第二套unit hash。identity不依赖输入枚举、session arena id、re-export路径或host绝对路径，runtime不得退回table index或可读path。多Cone顺序见12.3；访问另一个unit会先ensure目标。HIR只对该unit自有且经脱糖展开的initializer/delegate expression、object base argument与`init`body中的直接typed unit引用形成依赖图并报告结构环，不递归进入被调用的普通function/constructor/default/dynamic/FFI body；这些间接环由runtime gate检测。startup失败则`main`不执行；
- object/companion在首次非const访问时线程安全、同步初始化；static nested declaration或const引用不初始化外层。每个runtime unit状态为Uninitialized、Initializing(owner/dependency stack)、Initialized或Failed(rooted Throwable)。成功singleton只在完整初始化后release发布；失败不发布、记忆异常且不重试。同线程或跨线程wait-for环抛出`message`含稳定unit path的`IllegalStateException`；该异常若未在initializer内被普通`try`捕获才使unit失败。其他线程以可参与safepoint的方式等待terminal state；
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
- declaration signature中的type、receiver、base/bound、annotation type及default直接绑定实体必须同时覆盖该declaration的direct access/call domain与其承担的更宽slot contract domain。visibility在const folding、companion forwarding和desugaring前检查；M17 default保留已绑定的kind-specific typed引用；前端只在定义处和继承导致调用域变化时检查覆盖关系；
- class primary constructor需要modifier/annotation时写显式`constructor`关键字；无modifier的class header/explicit primary及class/struct secondary constructor均为internal。class primary property parameter可声明member visibility/override，普通parameter不可。class隐式零参数constructor为internal并与owner domain取交集；public class需要显式`public constructor`才提供public construction API。struct primary constructor与enum variant constructor属于固定public representation entry，只与owner domain取交集且不能单独声明visibility；
- property-level custom annotation不传播到accessor/backing/delegate storage；explicit accessor可单独标注。`@Unsafe`/`@Safe`可用于constructor/accessor并进入调用contract；`@NoGC`只在完整signature/body确实GC-free的explicit accessor或struct secondary constructor合法。class/object receiver为ref，不能满足NoGC。`@Extern`/`@Global`/`@ThreadLocal`及`@CallingConvention`仍限各自13章target；普通property/object/constructor不能伪装为native symbol。
- constructor的`@Unsafe`/`@Safe`控制该声明的参数缺省表达式、`this`/`super`委托表达式及secondary body；overload选择完成后才检查所选constructor的safety，不因safe调用上下文改选其他候选。class/object的property initializer、delegate与`init`属于共同初始化声明，使用独立safe上下文，不继承任一constructor的注解；需要unsafe操作时使用显式`@Unsafe` block。

#### 9.1.6 GC-free release block

普通final class可以声明至多一个受限的release block：

```scoop
final class NativeOwner(
    private var handle: Option<Ptr<UInt8>>,
) {
    release {
        when (handle) {
            Some(value) -> @Unsafe { nativeFree(value) }
            None -> {}
        }
    }
}
```

`release { ... }`是class member位置的contextual语法，不是名为`release`的方法。普通`fun release()`仍可作为显式释放API存在。release block没有名称、参数、返回类型、visibility、annotation或modifier；它不进入lookup、overload、override、vtable/itable、callable reference或反射表面，源码也不能直接调用。block正常完成结果固定为`Unit`，`return`、`throw`与挂起均非法。

- owner必须是普通、可实例化的final class。`open`/`abstract`/`sealed` class、interface、struct、enum、annotation class、intrinsic class、`object`/companion及compiler-generated class均不能声明release block；`Throwable`的任意子类也不能声明，因为异常runtime会按值复制异常payload。合法owner可以是static nested或generic final class，也可以继承一个不带release block的base class，但release语义不继承、不覆写且不形成hook chain；
- release block没有普通`this`，而是在独立的reclaiming-receiver上下文中定型。它只可direct读取本owner自己声明、确有backing storage且fully resolved exact type满足`ReleaseValue`的primary/body stored property；该读取始终绕过getter并直接load backing field，即使stored property另有explicit accessor，结果也是只读release value。inherited、computed/accessor-only或delegated property、任何getter/setter调用、method、extension、`super`及任何managed field均不可用；field write、receiver传参/返回/存储/捕获/装箱/cast/取址、root/handle/pin及把当前对象转回managed ref均为编译错误；
- `ReleaseValue(T)`要求fully resolved exact `T`为GC-free，且其递归表示不含GC/root/callback capability。M24按typed core identity封闭排除`PinnedPtr`、`GcHandle`、`FunPtr`、`ForeignCallback`及其aggregate/`Option`包装；它们虽为GC-free值，却可能固定、保活或回调managed对象。普通integer、`Ptr<Unit>`、pointee也满足`ReleaseValue`的`Ptr<T>`、相应`Option<Ptr<T>>`及只由release value组成的aggregate合法；
- generic owner可以导出release template。body所读取字段若依赖type parameter，编译器为每个fully specialized application验证typed `RequiresReleaseValue`条件；不能证明的application是编译错误。未被release body读取的managed字段不影响合法性；
- release block隐式满足比普通`@NoGC`更严格的release-safe effect，但本身不是`@NoGC` annotation target。所有local、temporary、expression、field read、参数与非`Unit`结果必须满足`ReleaseValue`；不得分配managed对象、触发safepoint/GC、抛异常、挂起、调用ordinary managed callable、Scoop ABI extern或virtual/interface/indirect dispatch，执行managed/native线程状态转换，操作root/handle/pin/thread/GC入口或回调Scoop。它可以调用经传递验证为release-safe且目标静态可确定的`@NoGC` Scoop helper，包括top-level function与GC-free value-type的direct method；callee既有NoGC body不得包含C extern或runtime transition，M24不生成release-context clone。release block还可以直接调用签名C-FFI-safe且全部参数/非`Unit`结果满足`ReleaseValue`的C ABI extern leaf；后者不经过managed线程transition且不得foreign unwind或回调Scoop。普通`@NoGC`拼写本身不足以证明release-safe；
- `@Unsafe`规则不因release上下文放宽。C ABI extern与raw pointer操作仍须写在`@Unsafe` callable/block中；release block中访问`@ThreadLocal`非法，因为执行线程不属于语言契约；
- 分配时对象的内部release-ready状态为false。只有最外层constructor正常返回、构造表达式即将发布完整对象时，生成代码才把它设为true；构造失败对象不运行release block，构造途中已经取得的unmanaged resource必须由构造代码显式清理。该状态不是源码property或通用field-initialization bitmap，源码/FFI不能读取或修改；
- collector只在release-ready对象已被证明逻辑死亡且其storage即将真正reclaim时，通过exact TypeDescriptor登记的静态hook同步执行该block。marked/live/pinned/handle-rooted对象不执行；moving的from-space旧副本不是逻辑死亡且绝不执行，ready状态随存活对象搬迁；hook返回前storage保持完整可读，返回后立即失效。详细ABI与顺序见runtime spec 2.1、2.2和3.8；
- hook是best effort：不保证何时发生GC、对象间顺序、执行线程、native release结果或进程退出时调用；shutdown不做全堆finalization pass。但一次正常collection已经决定回收一个ready对象时，必须在poison、复用或unmap其storage之前尝试一次且至多一次；
- 程序仍应提供显式`close`/`release`并用`try/finally`管理资源。显式释放应先把handle字段置为`None`等合法inert state，再释放取出的资源，使以后可能发生的hook成为no-op。语言不自动生成close，也不公开arm/disarm或手动调用hook的能力。

release block不提供GC finalizer语义：不能观察managed对象图、复活对象、依赖另一个hook的顺序或承担及时释放、事务、锁、flush等正确性责任。

### 9.2 委托

- property delegate使用reflection-free协议。角色必须显式声明为ordinary、non-suspend、non-generic `operator fun`，不得带default/`vararg`：可选`provideDelegate(): D2`、必需`getValue(thisRef: R): T`，以及`var`必需`setValue(thisRef: R, value: T): Unit`。它们是与9.3普通operator不同的typed role；无role的同名函数不参与；
- `R`对class/object member是owner type，对extension是extension receiver，对top-level/local是Unit。先求值`by`表达式一次，再可选调用一次无owner参数的provideDelegate并把effective delegate存入hidden field/global/local；之后每次access读取delegate并调用唯一get/set target。协议没有`KProperty`、property name或annotation metadata；需要这些值必须在`by`表达式中显式传入；
- 非局部delegate必须显式声明property type。local delegate省略type时，先在没有result expected type的条件下选出唯一`getValue`role，再以其concrete结果作为property type；`var`的`setValue`必须接受同一type，不能从多个get/set组合反向猜type或让setter改变getter结果；
- class/object delegate storage进入9.1.1 common sequence/readiness和GC scan；top-level与non-generic extension delegate进入9.1.3 eager unit；generic extension delegate按9.1.1为每个exact receiver application建立独立的program-wide lazy unit，第一次访问该application时才求值`by`表达式并发布effective delegate，不能因最终程序已知使用点而改成eager startup。`by`表达式与可选`provideDelegate`按定义处已绑定的typed template在consumer Cone替换receiver type arguments，不重新解析name、import或operator。该specialization的ODR group整体包含effective delegate storage、managed root descriptor、init cell、failure root、initializer/ensure entry与init descriptor；多个Cone产生同一specialization时必须共享并整体coalesce这一组成员，re-export不复制它。local delegate是不可重新绑定的hidden local。struct/enum member不能delegated。delegate调用同步，可分配、GC、抛异常但不能挂起；
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

支持自定义注解与注解使用语法；不内置平台相关注解。注解不携带运行期反射能力（见 2.2），编译期处理（如编译器插件）由实现定义。语言核心注解（`@Intrinsic` / `@NoGC` / `@Extern` 等）见第 13 章。

---

## 10. 数组

Scoop 内置两个数组类型（引用类型，属于核心库）：

- `class Array<T>`：不可变数组（长度固定，元素不可写）；
- `class MutableArray<T>`：可变数组（长度固定，元素可写）。

它们是由core源码提供nominal identity、由`@Intrinsic`提供representation family的invariant generic class，使用与普通`class C<T>`相同的类型application、约束、成员解析和单态化规则。编译器可以为字面量、内联元素区、下标和转换保留typed专用操作，但不得再建立一个与core class声明平行的数组类型身份。

### 10.1 值类型元素的内存保证

当 `T` 是值类型（struct / enum / tuple / 基本类型）时，`Array<T>` 与 `MutableArray<T>` 保证：

- 元素**不装箱**；
- 元素在内存中**连续布局**（在满足 pack/align 约束的前提下）。

当 `T` 是引用类型时，数组存储引用。

当`T`是ZST时，`Array<T>`/`MutableArray<T>`使用专门的zero-sized element storage：对象仍保存普通ref identity与精确`size: Long`，元素区起点仍按`alignOf<T>()`对齐，但任意长度都不分配element payload bytes。所有literal/assembly/spread输入仍按源码顺序求值并计算逻辑元素数。`get`先按普通调用规则求值receiver与index，再检查`0 <= index < size`并返回该exact ZST值；`set`先按普通调用规则依次求值receiver、index与RHS，随后执行bounds check，成功时不写物理字节。因而即使index越界，RHS的副作用或异常也不能因ZST被跳过。不同index表示不同逻辑元素，但不承诺不同物理地址；现有`addressOf`不能用于array元素。

ZST array的iterator必须保存array ref与`Long` index，以`index < size`终止并按1递增；不得用element pointer是否到达end判断进度。`Array`/`MutableArray`互转和clone仍分配新的array对象并保留size，所以结果ref identity与源不同，但不执行payload `memcpy`。物理分配大小恰为对齐后的元素区起点，与size无关；size仍须位于数学区间`0..=INT64_MAX`，因此不能用“分配字节数很小”绕过长度、assembly求和或迭代index的overflow检查，也不要求core新增整数边界companion常量。M23公开表面没有接受任意signed length的array constructor：literal与spread/vararg assembly只从非负元素/component count经checked求和得到size；clone/互转则先验证source exact array TypeDescriptor、side metadata与`0 <= source.size <= INT64_MAX`一致，再读取该source size作为目标logical count。未来若增加length-based core API须另行规定其源码前置条件与异常。

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
- 互转总是分配新对象并复制logical `size`，结果是与原数组相互独立的快照：之后对原数组的修改不影响转换结果，反之亦然。非ZST `Inline`元素复制完整inline payload（实现可用`memcpy`）；`ZeroSized`元素没有payload，不调用`memcpy`，但这不允许复用原对象或丢失size。
  - 不能像 Rust 那样转交（move）内存块：Scoop 没有 move 语义，转交意味着清空原 `MutableArray`，与引用语义冲突。
  - 也不能仅改写对象头复用原存储：在 LLVM + GC 的实现中，每个引用类型对象的头中带有 TypeDescriptor（类似 vpointer），就地改写它会破坏 GC 的状态，因此转换必须分配新对象并复制数据。

### 10.5 操作

- 下标访问 `a[i]`通过普通成员`operator fun get(index: Long): T`；`MutableArray`通过`operator fun set(index: Long, value: T): Unit`支持下标赋值`m[i] = v`。这些声明可以由intrinsic提供表示级实现，但候选选择、泛型实例化与operator identity遵守9.3，不建立按`Array`类型名放行的第二套解析规则。
- `vararg T`的spread和普通形参`Array<T>`都要求exact `Array<T>`；需要改变element type时，调用方显式逐元素构造/转换目标array。
- `size: Long` 属性；实现 `Iterable<T>`，可用于 `for` 循环。

---

## 11. 最小核心库

核心库只包含核心语法运行所必需的类型与函数。命名空间为 **`scoop.core`**（默认导入 `scoop.core.*`，以及 `scoop.core.Option.*`，见 7.2）。

**核心库中的类除单独标明外均为 `final`**，不可继承（包括 `Array` / `MutableArray` / `String` / `StringBuilder` 等）。

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

### 11.3 `Unit`

0 元 tuple 的类型名与值构造器（见 4.3）。

### 11.4 `String`

- 引用类型，immutable，UTF-8 语义（编码细节由实现定义）。
- 语言层支持 `+` 拼接、索引/切片、`length`（或 `size`）、比较等核心操作；索引、长度与切片边界固定使用`Long`，保留64位范围。当前实现子集已有拼接与比较；ROADMAP排在M26的索引、切片和length/size首次实现时直接采用上述`Long`签名。
- 实现内容相等的成员`operator fun equals(other: String): Boolean`与内容相关`Hash`；`toString()`返回自身（`ToString`的恒等实现）。这些能力均是String的具体core contract，不来自`Any`或TypeDescriptor缺省槽。

### 11.5 `Option<T>`

```
enum Option<T> {
    Some(T),
    None
}
```

见第7章。`Option`与全部nominal generic一样保持invariant；niche表示只属于每个exact `Option<T>`。`scoop.core.Option.*`默认引入。

### 11.6 `StringBuilder`

字符串插值（第 6 章）的脱糖目标：

```
class StringBuilder {
    fun add(part: String): StringBuilder
    fun <T : ToString> add(part: T): StringBuilder   // 插入 part.toString()
    fun build(): String
}
```

也可由用户代码直接使用。

### 11.7 异常

- `Throwable`（引用类型，可被 `throw` / `catch`）及其最小子类：
  - `Exception(message: String?)`：通用异常基类；
  - `UnwrapException`：`!!` 失败时抛出（见 7.3）；
  - `ClassCastException`：`as` 失败时抛出；
  - `ArithmeticException`：整数除零等算术错误；
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

`CrossConeTypeSemanticsSectionV1` 保留 field 1、2、3、8，field 4～7 退役；`NominalInheritanceInterfaceV1` 保留 field 1～4、7～9，field 5、6 退役，退役字段不复用。成员引用的 Constructor tag 2 随重复构造器通道退役，实际 constructor 始终使用共有 typed 声明。HIR `cross-cone-type-semantics/8`、required inventory、profile 与内容 fingerprint 同步更新，旧产物和缓存需重建；不保留旧来源副本的双轨兼容，不改变 runtime C 调用约定或 String 表示。

- generic class可以继承`Throwable`；其每个exact application都是不同异常类型并拥有不同TypeDescriptor。`catch (e: Error<Int>)`只接收该exact application及普通派生class，`catch (e: Throwable)`仍可接收全部application；不存在`Error<*>`式通配catch。

### 11.8 迭代与区间

迭代操作、解构、挂起性、GC effect 与循环跳转的语言规则由前端在解析实际声明及构造 typed plan 时检查。已经完成的计划在 concretization 中直接代换引用并展开，不要求对整个 HIR 重新执行名称、类型、默认值或局部数据流检查；正常产物消费仍检查编码和实际引用。

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

integer range有四个canonical nominal type：`public final class IntRange : Iterable<Int>`、`public final class LongRange : Iterable<Long>`、`public final class UIntRange : Iterable<UInt>`与`public final class ULongRange : Iterable<ULong>`。四者是不同的nominal type，`LongRange`/`ULongRange`不再是alias。constructor与表示属性为core-internal，外部代码不能直接构造不满足step/方向不变量的实例；类型、分配、构造与成员本身仍遵守普通class规则，不是intrinsic或runtime opaque type。`CharRange`等到Char进入已实现子集后另行定义。

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

- `startCoroutine` 是最小协程构建器：启动 `task.run()` 后立即返回 `Unit`。若 task 在启动调用内完成，则返回前调用 `completion.resume(value)` 或 `completion.resumeWithException(exception)`；若 task 挂起，则在最终完成时调用。completion 恰好收到一次完成通知。
- `suspendCoroutine` 调用 `registration.register(continuation)`。registration 可以同步恢复 continuation，也可以保存它并在 `register` 返回后恢复；前者使 `suspendCoroutine` 在当前调用栈内继续，后者使其真正挂起。`register` 在尚未完成 continuation 时抛出的异常等价于 `suspendCoroutine` 在调用点抛出该异常。
- `register` 已同步完成 continuation 后又抛出属于状态协议错误，`suspendCoroutine` 以 `IllegalStateException` 失败；该 continuation 随即失效，之后不能再次成功完成。
- `SuspendTask` / `SuspendRegistration` 是不依赖 lambda 与函数引用的最小协议。函数类型 overload 由 core 中的普通 Scoop 适配器包装为这两个 interface 后调用同一底层原语，不另建 continuation 状态机；两种入口具有完全相同的同步完成、真实挂起、异常与单次完成语义。
- `launch`、`async`、dispatcher 等高层 API 可以在这些原语上由标准库提供，但不得改变 8.2 的单次完成与异常语义。
- core 原语不提供队列、线程切换或事件循环；调度器与取消不属于核心库。

### 11.10 数组

`Array<T>` 与 `MutableArray<T>`，见第 10 章。

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
- `scoop:single-file:0.0.0`是另一reserved coordinate，用户manifest不得声明它。`scoop build/run`收到basename扩展名精确为`.scoop`、跟随symlink后目标为已存在regular file的operand时，构造`kind = executable`、source set恰好为该文件、logical source path恒为`main.scoop`、direct Cone dependency恰好为trusted core的typed synthetic projection；symlink cycle、dangling link或最终目标非regular file是输入错误，resolved host path不进入identity。显式文件operand即使位于某Cone目录中也不读取相邻`Cone.toml`、其他`.scoop`、C/C++ source或blob；它不接受其他Cone dependency。该`.slib`可缓存，并可作为产生它的build/run或显式`scoop link --root-slib`的唯一executable root；但不能作为可分发artifact发布、作为dependency或被manifest artifact locator引用；
- manifest Cone的source identity是`(ConeIdentity, normalized Cone-relative src path)`；single-file source使用相同pair形态，但第一项固定为由reserved `scoop:single-file:0.0.0`计算的`ConeIdentity`，第二项固定为`main.scoop`。host绝对路径、CLI relative/absolute/symlink spelling、inode、mtime、目录枚举顺序与临时输出路径不进入语义identity；只有语言允许跨文件同名的file-private/hidden实体才把该source identity加入其declaration key。single-file artifact/cache key另外包含source content digest、core semantic/code fingerprints、compiler/schema/target/toolchain，不因共用reserved identity而碰撞；

### 12.3 静态exact依赖图

- M23固定三层工具边界：umbrella binary `scoop`负责root-input分流、locator、resolved DAG、cache与调度；`scoopc`每次只编译一个当前Cone并产生该Cone的`.slib`；program-link是只消费已验证artifact的独立stage。`scoop build`和`scoop run`共用这条完整pipeline；`run`只在build/program-link成功后执行binary，不是另一种编译或解释模式。三层不能用共享的未持久AST/IR或隐式进程状态绕过`.slib`边界；
- M23只接受最终链接前已经完整解析的静态Cone图。依赖边必须无环；同一resolved graph中同一`group:name`只能出现一个version，同一`ConeIdentity`只能对应一组一致的semantic fingerprints。cycle、多个version、同identity不同artifact或dependency coordinate不匹配都是构建错误；
- `executable`不能成为另一个Cone的dependency。一次程序构建恰有一个executable root，其余节点都是library；library单独构建时不需要executable root；
- core可从任意普通manifest目录作为`scoop`构建根；当前根已经定义core时不读取默认sysroot、不加载另一份core，也不注入self edge。graph、源码快照、缓存和产物返回均使用普通Manifest library节点。
- `scoop`为除core自身外且未显式声明core的每个Cone注入12.6的core direct dependency；core artifact使用普通依赖输入与一致性检查，sysroot仅提供默认locator。除该边外不存在隐式dependency；
- single-file resolved graph恰好由trusted core与唯一synthetic executable root组成，不运行manifest locator发现；其源码对非core Cone的import按普通不可达诊断。这不禁止`@Extern`产生的逻辑native library requirement；该requirement只能由`scoop`/program-link经显式library search root解析，不是Cone dependency，也不能用无typed来源的raw object/archive输入替代；
- dependency path、manifest枚举与输入顺序不影响结果。canonical topological order使用dependency-first的Kahn顺序，并在每个ready set按`(group UTF-8 bytes, name UTF-8 bytes, canonical version)`取最小者；
- 对 manifest root，`scoop`先只读解析 source manifest 与 prebuilt `.slib` 的 manifest summary；single-file root 使用 12.2 的固定语义记录。编译 child 启动前先验证完整构建图，再按依赖顺序处理节点。父进程检查产物的 envelope/hash、profile、Cone/target、依赖 fingerprint、缓存键和 child 结果，保留不可变归档与摘要，不为调度或缓存重放完整语义与对象。`scoopc` 或实际 Link 消费者在读取外部产物时取得完整 HIR/MIR/LIR 与对象并完成对应检查；同一消费中的 Compile/Link 复用结果。source cache miss 在全部上游就绪后调用一次 `scoopc`；源码依赖变化时重编译，无源码可重建时报告 stale dependency，不能把旧 typed identity 接到新 metadata；
- 每个`.slib`记录编译时direct dependency的`ConeIdentity`及HIR/MIR/LIR semantic fingerprint。编译manifest-backed Cone时，`scoopc`必须显式获得当前manifest声明的全部direct `.slib`及它们递归引用的其余transitive support `.slib`；direct/support角色由typed manifest edge验证，不从命令行顺序或路径推断。single-file请求不接受额外Cone依赖，其core artifact可由普通构建输入指定，sysroot仅提供默认路径。缺失、额外不可达、重复identity、同identity不同fingerprint、stale edge、cycle、自环、同一`group:name`多version、executable dependency、伪造core authority或target/backend/schema/ABI不兼容都必须在parse当前源码前失败；`scoopc`只验证调用者给出的封闭artifact graph，不搜索、不修复且不重编译任何上游节点；
- runtime在执行任何managed initializer前登记整个图的image、stackmap、TypeDescriptor、static storage/root、immortal object与initialization-unit metadata。eager top-level initialization按上述Cone顺序执行，每个Cone内再按9.1.3的`PersistentInitializationUnitId` bytes排序；object/companion及generic delegated extension等lazy unit只登记、不进入eager loop。直接读取另一个unit仍先ensure目标，可以使该目标早于其普通排序位置执行；
- `scoopc`为typed metadata support加载完整transitive artifact closure，独立program-link stage为最终链接另行验证并加载同一闭包，但这不使间接依赖自动成为源码候选。源码可到达性只由12.4的current Cone、direct dependency surface、re-export和core prelude决定；
- library root以完成的`.slib`为最终产物，父进程核对归档与构建结果。executable root同样先完整产生自身`.slib`，实际 program-link 消费时读取并检查完整依赖与对象。`scoop`的registry必须原子解析不可部分构造的`ResolvedTargetProfile { lir_target: LirTargetProfile, backend: ValidatedBackendProfile, c_bridge_toolchain: ValidatedCBridgeToolchainProfile, runtime_build: ValidatedRuntimeBuildProfile, final_link: ValidatedFinalLinkProfile }`，并证明五个projection的triple、object format、deployment、calling convention、pointer/storage ABI、compiler output与link input相容；不得先读取host默认值再补齐。完整product只由driver后续编排持有，不写入M23-2 `.slib`或下发给不需要它的stage。M23-2只构造并持久化前两项组成的`ValidatedLirTargetSelection`，不能伪造完整profile；`lir-lower`只接收`lir_target`，Scoop codegen接收`lir_target + backend`，generated-C producer接收`lir_target + c_bridge_toolchain`，runtime-build接收`lir_target + c_bridge_toolchain + runtime_build`，program-link接收`lir_target + final_link`及已验证的其他stage产物。后三项由后续required capability冻结，并分别进入Code、RuntimeArtifact/RuntimeImage与ResolvedLinkPlan fingerprint。之后runtime-build才从受信任runtime source set按这些精确projection构建任意非空数量的verified relocatable object，形成`ValidatedRuntimeArtifact`。M23不接受外部prebuilt runtime bundle、raw `.a`或runtime-object cache。program-link只消费Link-purpose已验证的完整transitive artifact closure、该runtime artifact、精确link projection与输出路径，不读取Cone/runtime source manifest、locator、cache或编译残留IR。最后一次`scoopc`不顺带生成program descriptor、runtime输入或最终binary；
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

#### 12.4.2 import可到达性与re-export

普通exact/star import selector只能解析：

1. 当前Cone中按visibility允许当前file访问的声明；
2. manifest direct dependency的public lookup binding；
3. 该direct dependency已经解析并公开的re-export binding；
4. trusted core direct dependency的public/prelude binding。

每个成功解析的import target都非可选地保存最终typed实体origin及一个非空、canonical排序去重的来源集合：`CurrentCone`表示从当前Cone声明解析，`DirectDependency { provider ConeIdentity, export binding persistent id, witness hops }`表示由某一direct dependency的已验证公开表面赋予访问权；implicit core也使用后一分支。链式re-export只延长`DirectDependency` witness，不改变target的`ConeIdentity`或entity identity。同一origin经钻石图到达时合并target并保留全部合法source witness，不能按首次加载路径任选一个；source按provider coordinate/identity、export binding id及逐跳persistent witness排序，session-local id、路径长度与artifact加载顺序不参与判等。transitive `.slib`虽可作为template/layout/link support加载，但未由direct dependency re-export的public binding不会自动成为源码候选。

- `public import`的每个最终target都必须至少有一个source且全部source都是`DirectDependency`；`CurrentCone` target不能用于public import/re-export。同一origin可以由多个direct dependency surface共同授权并把全部witness写入snapshot。它也不能导出internal、private或protected target。target本身可以是该dependency的re-export，因此re-export可以成链；
- re-export只建立destination package/name到origin typed实体的公开binding，不生成wrapper、forwarder、第二个TypeDescriptor、第二个typealias target、generic body或storage，也不扩大target member的visibility；
- public star在编译当前Cone时展开为逐项、已经解析的API snapshot；下游不重新执行上游的文本glob。target集合变化会改变当前Cone的re-export metadata/fingerprint；
- 同一typed origin经重复import、多个star或钻石re-export路径到达同一层时合并为一个target并union其排序后的source witness集合；删除一条路径会确定性改变snapshot/fingerprint，但只要另一合法路径保留就仍可访问。不同origin即使package/name/signature文本相同也不合并：非overloadable实体产生歧义；function/extension可进入同一overload层，但展开后签名相同仍是冲突；
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

`CapabilityId`固定为`{namespace, name, nonzero u32 major}`：namespace总长1…255 ASCII byte且满足`[a-z][a-z0-9-]{0,62}(\.[a-z][a-z0-9-]{0,62})*`，name总长1…63 byte且满足`[a-z][a-z0-9-]{0,62}`。M23内建typed id精确为`org.scoop-lang.target-profile/darwin-aarch64/1`、`org.scoop-lang.backend-profile/llvm-22-1/1`、`org.scoop-lang.c-bridge-toolchain-profile/darwin-aarch64-apple-clang/1`、`org.scoop-lang.object-format/mach-o-relocatable/1`、`org.scoop-lang.link-object/scoop-lir/1`与`org.scoop-lang.link-object/generated-c-bridge/1`；target/backend/C-bridge toolchain/object format虽复用该三字段wire envelope，在类型上不能彼此或与verifier capability互换。manifest `CompatibilityRecordV1`的field 5/6精确为`TargetProfileWireId/TargetProfileFingerprint`，field 7/8为`BackendProfileWireId/BackendProfileFingerprint`；reader只在两对都由registry验证后构造`ValidatedLirTargetSelection { lir_target, backend }`。target profile仅覆盖LIR可观察的layout、Scoop ABI、native symbol normalization及“C边界必须经generated bridge”策略；backend profile仅覆盖Scoop LIR到LLVM/Mach-O的受检codegen。实际C compiler、SDK/deployment、generated-C template/flags、runtime build与最终link action不属于这两个`/1` contract；C-bridge toolchain以独立profile id/fingerprint进入M23-3 production proof，不能伪装成target或backend字段。

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

variant tag是little-endian `u32`，product按声明顺序编码。后两个gateway不能冒充其调用的main/ensure。M24把**全部**machine body统一切到`CallableBodyKeyV2`与domain`scoop-callable-body-v2`，保留前四个tag并增加`ReleaseHook { owner: PersistentExactTypeId }=5`，公式仍是`SHA-256(ByteSpan(domain) || RuntimeEncode(key))`；M24的HIR/MIR/LIR outer schema、identity-foundation capability major分别升为HIR 4、MIR/LIR 2，引用它们的artifact profile升为2，而container、persistent identity schema、`persistent-v1` mangler与runtime metadata record prefix仍为1。旧M23 artifact整体重建，M24 artifact不得混留body-v1。`PersistentCallableApplicationId`、`OdrGroupId`及以CallableApplication/GeneratedCallable作discriminator的primary member不因body版本改变；body id、其safepoint及以CallableBody/SafepointSite作discriminator的派生member、registration与ODR fingerprint全部重生。

只有声明的native extern、validated runtime artifact函数及由非Scoop LIR producer生成的native body/bridge不属于callable-body集合，它们使用各自typed identity与目录或final-input verifier capability。layout id由exact type、target profile与representation role派生；storage/object id由typed owner declaration或specialization、stable definition path与封闭生成role派生，内容变化进入definition fingerprint而不另造content identity；safepoint site id由callable body id与CFG site role/ordinal派生。TypeDescriptor直接以`PersistentExactTypeId`登记，不另设与exact type竞争的type identity。凡concrete exact type进入LIR layout/type closure或param-free exported LIR bridge就必须runtime-materialize一份TD registration；只存在于未替换Export HIR template/binder中的type尚不materialize。非nominal exact type以exact id建立12.5的`StructuralType` ODR group；nominal exact type及以它为shape owner的box、coroutine step/slot/shell/start一律沿`ExactOwnerRoot`回到source Cone或Nominal specialization，不能为nominal exact type另造`StructuralType`组。最终程序中每个materialized exact type恰有一个TD地址，是否materialize不改变语言type identity。

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

四个specialization variant的wire tag固定为1…4。全部argument/application必须使用persistent exact type identity；没有owner/application/argument时使用对应variant的typed空分支，不能靠空vector推断entity kind。member role只区分同一group内的member，不进入`SpecializationKey`或`OdrGroupId`。`OdrMemberRole`的tag 1…16固定为`CallableBody, GeneratedNominal, Layout, ScanProgram, TypeDescriptor, DispatchTable, DispatchAdapter, StaticStorage, ImmortalObject, InitializationCell, InitializationDescriptor, RegistrationRecord, DiagnosticBytes, AddressTakenConstant, ObjectSupport, ReleaseHook`；`ReleaseHook=16`在identity schema v1中已有唯一语义且只允许`ExactType` discriminator，但M23 HIR/MIR/LIR schema v1与artifact profile必须拒绝产生/消费它，M24 schema/profile v2才启用。

role/discriminator合法性之外，reader还必须从discriminator完整canonical key沿typed owner关系回溯到本group唯一root：Callable application逐字段等于group key；Nominal的exact application、DelegatedProperty的unit与StructuralType的非nominal exact type分别匹配自身origin/arguments。任意别组typed id、producer Cone、symbol、object分片、相同layout或first use均不能建立provenance。多个Cone产生同一specialization时发射相同group，且相同role/member provenance具有相同`OdrMemberId`与ODR linkage：nominal group完整包含由该nominal application产生的layout、TypeDescriptor、vtable/itable、scan及相关box/adjust member；callable group包含由该callable application产生的body、closure/coroutine/adapter；delegated-property group包含其storage/root/cell/failure/init/ensure/descriptor；structural-type group包含materialized layout/scan/box/TypeDescriptor。specialization-owned registration、address-taken string/constant/diagnostic bytes及其他支持定义都必须进入同组，不能直接指向consumer-local private定义，也不能增加未列入上述封闭sum的content-group旁路。唯一的受验证例外是generated-C bridge：canonical LIR/ODR只引用producer-independent的`GeneratedBridgeSemanticTarget { unit }`，实际object relocation才绑定当前producer的local strong primary atom并由object verifier正规化回unit；它是外部semantic dependency，不成为当前specialization group成员。一个group可以通过typed ref引用另一既有canonical group，但不能把自身产生的runtime identity漏在组外。

M24 generic release hook恰好使用其owner的Nominal specialization group及`{ role=ReleaseHook, discriminator=ExactType(owner application) }`，validator逐字段核对group origin/arguments；hook body使用`CallableBodyKeyV2::ReleaseHook(owner)`的`cb` primary和`OdrWeak`，另有同组唯一`RegistrationRecord/CallableBody(body id)`供`cr`使用，不得再造`CallableBody` member或`od` alias，且不得有safepoint/`sr`。param-free hook不构造ReleaseHook ODR member；body、registration与TD沿同一个exact source subject统一取得`ConeStrong`，或在M23-7完整hidden-support proof存在后统一收窄为`TemplateSupportHidden`。

各producer记录相同的完整ODR member set、ABI fingerprint与`OdrDefinitionFingerprint`；link前必须比较definition fingerprint，ABI相同不足以合并。每个`OdrMemberId`的canonical LIR/constant payload以`scoop-lir-definition-v1`形成独立leaf，最终ODR digest按稳定的typed definition-node与safepoint-site key覆盖完整member set、这些LIR leaf、归一化object atom/typed relocation及stackmap leaf，且不得反向进入LIR own-layer。canonical relocation target是封闭sum；bridge分支只能保存`GeneratedBridgeUnit { id }`，不得保存producer-specific `GeneratedBridgeAtomId`。每个producer的object verifier核对真实`br` symbol确实属于`GeneratedBridgeAtomKey { producer=current Cone, PrimaryEntry(unit) }`后才把relocation正规化为该unit；缺失/多个primary、跨producer atom或直接把atom写入canonical definition均拒绝。`SlibMemberId`、object range/offset与物理分片只定位当前artifact中的leaf，绝不进入跨Cone ODR canonical内容或排序；否则相同specialization会仅因producer Cone不同而无法coalesce。LIR另携带typed digest DAG与单writer patch plan；每个node只观察声明的传递依赖，hash视图把node自身、peer、descendant与无关slot归零，不能把已经完成的上游leaf统一归零或按遍历时机决定输入。Darwin/Mach-O按每个member的deterministic primary-key policy发射同名`linkonce_odr`/`weak_odr` symbol：callable/layout/scan/TD/table/storage/registration等优先使用`cb/ly/sp/td/dt/ss/...`的kind-specific key，`od(OdrMemberId)`只用于没有kind-specific primary的封闭fallback role，同member不得同时发射两种primary。完整definition验证保证任意winner等价。最终程序必须验证同一exact type只有一个TypeDescriptor地址且不同exact type没有被constant merge/ICF折到同一地址，不能只合并函数而留下重复TypeDescriptor或static storage。

独立 `identity-foundation` artifact profile（旧 `/1`、`/2`）退役。删除只供旧测试使用的 `IdentityFoundationMetadata`、`IdentityFoundationArtifact` writer，以及 `DecodedIdentityFoundations`、`IdentityCheckedFoundations`、`StructurallyValidatedFoundations` 和 native-boundary/commit 外层组成的平行 reader。三层基础 identity payload、真实依赖身份解析、类型/ABI/GC 契约与完整 Strong 产物 reader 保留；测试直接使用共有容器或完整生产 reader，不保留只有 identity、没有实际编译输出的产物路线。

生产 profile descriptor 只编码必需 section 清单：field 1=id、2=required_manifest、3=required_hir、4=required_mir、5=required_lir。原 field 6～9 及独立 `ArtifactValidationPolicy` 退役，不复用；删除仅服务于旧 profile 或未来占位的 availability policy、publication class、Link proof policy 与 ODR policy 数据。完整生产产物的 Code/RuntimeImage fingerprint 必须 Available，由 manifest 读取规则检查；ODR 在本阶段的 Strong 输入边界拒绝，optional/unknown section 按实际 purpose 与 registry 规则处理。`single-cone-strong`、`cross-cone-semantics-strong`、`cross-cone-layout-strong` 的 major 均升为 3，旧 `/1`、`/2` 产物和缓存重建。profile fingerprint 继续覆盖这个实际格式描述，runtime C ABI 与 String 表示不变。

M23-3 的 strong-only production profile 为 `org.scoop-lang.slib-profile/single-cone-strong/3`。除三层 identity-foundation payload 外，它要求 Manifest `org.scoop-lang.manifest/single-cone-production/1`、HIR `org.scoop-lang.hir/core-bootstrap-interface/4`、MIR `org.scoop-lang.mir/core-bootstrap-bridge/1` 以及 LIR `org.scoop-lang.lir/strong-production/11`、`org.scoop-lang.lir/link-identity-closure/3`；code/runtime-image fingerprint 都必须为 `Available`，Compile 与 Link 消费边界拒绝 ODR group/member/body/symbol。旧 identity-only profile 退役，生产消费要求完整编译数据。发布使用同次编译的完整 typed IR 和产物汇总，不对当前产物及全部依赖再分别执行完整 Compile/Link 读取，也不增加发布凭证。M23-6 正式发布使用下述完整跨 Cone profile；ODR 仍留在 M23-7。

本地 class、struct 和 enum 实现参数自由的依赖接口时，前端直接消费共有接口声明的完整父接口、typed slot、签名、默认实现和访问域。HIR 的 conformance 引用实际接口类型及本地/外来槽声明，目标为本地方法 application 或共有依赖 callable；不得为复用本地检查而复制外来函数声明、正文或生成同名替身。MIR 的 dispatch 表保留本地函数或实际外部 callable 引用，默认实现与抽象槽 trap 沿定义方原有 target 解析，LIR 使用现有 canonical ABI、外部定义与 relocation 路径。override、缺失实现、默认方法冲突、setter 能力和签名/effect 规则在同一前端检查中完成；类型、成员和 dispatch 独立及组合场景须经真实源码产物消费和单 image 普通/移动 GC 运行验收。

本地 interface 可以继承参数自由的依赖接口。父边保留实际 TypeId，override 关系保留实际本地或外来槽声明；继承的成员按父接口声明顺序进入完整槽表，菱形继承按声明身份去重，被覆盖的槽按已解析 override 关系消除。显式成员及当前 this 的隐式成员查找沿本地与依赖声明的同一父图进行，本地和外来候选共同执行语言规定的适用性与最具体选择；不能以声明存储位置决定优先级，也不能将外来成员复制为本地声明。该接口再次发布后，下游按实际父类型、槽与 provider 消费，保持 canonical ABI、默认方法、属性和装箱语义。

抽象 dispatch 目标与具体实现一样保留实际声明：Export HIR 的抽象 conformance 携带真实方法 application 或外来 callable 引用，source selection 携带所选 abstract 声明，完整 slot contract 携带该声明的 owner、signature、effect、modality 与访问域。最近的 class 抽象声明以及更具体 interface 的抽象 override 均压制原默认实现；不把原槽声明伪装成所选目标。MIR、LIR 和 Compile/Link 按该 typed target 取得真实 trap、ABI 与 relocation，不再扫描所有 callable、重建整份继承图或沿继承链反向推测抽象目标。槽身份与所选声明身份可以不同，双方仍须满足实际继承、签名和访问合同。正常路径复用完整记录，不增加来源凭证或第二套证明表。

限定 `super<I>` 通过当前 owner 直接列出的真实接口类型查找成员，本地接口和依赖接口使用同一候选决议、参数默认值与可见性规则。调用选中的具体默认方法或 getter/setter 时，HIR 保存实际声明及强制 direct 调用方式；接收者沿共有引用转换或值装箱路径适配，MIR/LIR 按该声明的 canonical ABI 与 provider 定义发射，不再次进入接口表。抽象目标、非直接父接口、错误参数与初始化期间的调用仍在前端诊断。导出的默认参数正文用既有 `DirectSuperMethodCall` 节点保留该语义，下游展开继续调用同一真实目标。正文与引用集合复用同一 callable 引用转换，完整保留实际方法 owner，不分别重建不同的引用记录。同一 provider 的同一 typed callable 被 direct 和 dispatch 多次使用时，MIR 依赖记录与物理定义只选择一次，各调用点仍保留各自的派发方式。此项不新增来源资格、证明表、wire tag 或 runtime ABI；验收覆盖普通与 ZST 接收者、属性读写、命名及默认参数、大值结果和再次发布后的消费。具有隐式接收者的成员正文仍按普通值查找规则消费依赖属性、object 与 enum 变体；这些值与本地值使用同一个解析结果，不能归入函数或类型的非值阻断层。

M23-5 引入 `org.scoop-lang.slib-profile/cross-cone-semantics-strong/3` 作为多 Cone 语义产物的基线。当前该 profile 要求 HIR `org.scoop-lang.hir/cross-cone-interface/30`、MIR `org.scoop-lang.mir/cross-cone-param-free-bridge/2`、LIR `org.scoop-lang.lir/cross-cone-param-free-bridge/1` 与 Link 数据，并继续拒绝 ODR；M23-6 正式发布另包含完整类型和布局 section。共有 HIR 保存公开与必要支持声明、默认参数、常量、非泛型 alias、转导出路径及实际外部使用。名称查找按可见性枚举当前 Cone 和直接依赖；传递依赖按已经解析的 typed reference 查询。普通 callable 的声明、完整签名和 GC effect 由实际 provider 提供，final nominal 成员与顶层函数、extension 共用导出和消费规则。M23-6 的类型布局、构造器、成员、dispatch 与 protected 访问按各自语言及 ABI 规则完成；泛型物化与跨 Cone native 调用分别留在后续里程碑。格式 major 变化后旧产物与缓存需重建。

M23-6 的共有 HIR 接口 `/30` 保留 struct 的实际 `@CLayout`、`@InteriorMutable`、字段、成员及调用位置。公共和支持声明使用同一源码形状；布局按实际声明和 target 计算，不按类型名称、空字段或 core 身份补出策略。完整字段与版本规则见实现规范 2.6、2.11、2.12；runtime C ABI 与 String 表示保持。

跨 Cone struct 字段读取按接收者的实际声明解析名称，并在 HIR 保留 typed field identity 与完整接收者类型。字段在具体化时映射到同一声明的字段位置，后续布局和 ABI 继续使用共有依赖表示；不得用同名或同布局替代身份。计算属性通过其真实 getter 声明进入共有 callable 路径，保留可见性、GC effect 和返回类型；固定表示字段不为读取额外生成函数。依赖默认值中的字段和 callable 使用定义时保存的 typed 声明与完整类型，实例化不重新要求公开 namespace 导入路径，也不再次证明模板引用集合。

默认值依赖按定义处已解析的 typed target 保存，`DefaultDependency` 不要求消费 Cone 再取得 namespace 导入路径。定义处实际发生过的查找路径可以保留；生产器不为没有名称查找的字段、成员或支持声明补造 witness，也不保存全体依赖的第二份导入路径表。Unit、Any 与其他声明遵循同一规则。共有 reader 继续检查 provider、引用、类型、默认值正文及实际声明关系，不从这份路径信息授予调用资格。

共有 HIR `cross-cone-interface/24` 明确此默认值依赖合同，保留默认值既有字段及 `SourceDeclaration` tag 3，不增加或复用 tag；`/21` 及更早版本退役，旧产物和缓存须重建。profile fingerprint 按实际 descriptor 更新，runtime C ABI、String 表示与 GC 契约保持。读取边界核对源码上下文及位置后，实例化复用同一不可变记录，保留定义位置与调用处求值位置。

跨 Cone 名称、类型、成员和默认参数语义由实际声明及 typed IR 表达。独立的 foundation/declaration source transcript 及其逐层绑定结果不构成语言输入或调用资格；生产路径未使用的来源工厂、绑定包装和专用证明测试应移除。实际名称解析、可见性、默认值实例化与声明身份规则继续由对应前端实现负责。 默认值的源码位置和声明引用属于同一完整 HIR，不要求为位置收集另建一套默认值或访问凭证。

外部调用与布局引用使用实际 provider、typed target、完整 ABI、符号和定义记录。共有 Link 消费检查实际 undefined relocation 的符号、目标和覆盖关系；Code fingerprint 记录影响代码的依赖，member/range/offset 等物理位置按 Link 合同处理。同一次编译的完整 IR 和已验证依赖直接用于发布；不再保留独立 core requirement 闭包、来源资格或发布时分别重放 Compile/Link 的证明链。provider 拥有实际 body 和 Strong definition，re-export 仅引用已有声明。

独立program-link stage按`ConeIdentity`去重完整产物闭包的 Link 数据，按canonical Cone order与每个artifact内的`SlibMemberId`顺序只提取typed directory中的全部`LinkObject`恰好一次；已知Link-required extension只产生canonical native-library requirement，不识别的optional成员、诊断附件与opaque blob绝不提取，当前Link purpose需要但不识别的capability必须失败。stage按`ResolvedTargetProfile`中的`lir_target + final_link`精确projection稳定合并全部传递native link requirements，并验证已有Code/RuntimeArtifact产物的相容proof；不能因root Cone未直接声明某项FFI而漏掉上游link member的native dependency，也不能从host或backend profile推断link默认项。

Cone member的`DefinedLinkSymbolOwner`/`UndefinedSymbolRequirement`只覆盖带`SlibMemberId`的`.slib` object。native locator必须先把每个结果验证成封闭`CanonicalResolvedNativeInput::{DirectObject, StaticArchive, DynamicProvider}`；三种分支都携带完整request origins、target/kind、content或capability-pinned platform identity及非可选provider/verifier contract。program-link在调用native linker前有界解析、验证direct object与static archive的**全部候选member**；thin/external-member或nested archive、LLVM bitcode/LTO、unknown ordinary member，以及constructor/EH/TLS/producer-language metadata或autolink/directive未由capability闭合的候选一律拒绝。link后的member-load trace只能选择已preverify且由parent input与稳定member identity定位的candidate，不能在事后引入新object或contract。最终`FinalLinkInput`封闭区分Cone、program、runtime、native-static contribution、native-dynamic provider与target-synthetic；每个最终image definition由唯一`FinalLinkSymbolOwner`解释，其中native static owner精确为`{contribution, definition}`。`FinalUndefinedSymbolRequirement`的use origin只允许Cone、program、runtime、native-static或target-synthetic，并唯一解析到受控owner或带实际provider identity/contract的dynamic binding；dynamic provider只作后一种resolution，不是definition owner或undefined origin，也绝不能满足或interpose任何受控requirement。target默认shared library/framework同样走`DynamicProvider`而非target-synthetic。不能给program/runtime/native输入伪造`SlibMemberId`，也不能以“系统库”或文件名豁免。runtime artifact由`scoop`按`lir_target + c_bridge_toolchain + runtime_build`三个validated projection从固定受信任source set构建，并携带按typed object id排序的任意非空verified object collection及其合并contract/fingerprint；source/object数量和映射不是协议基数。program-link不读runtime source、不调用C compiler，也不接受prebuilt runtime bundle或raw `.a`路径。

每个 Cone artifact 的实际 `LinkObject` 联合贡献且恰好定义一个由其 identity 唯一命名的 logical `ScoopImageDescriptorV1`，manifest 的 `image_owner_member` 与实际定义一致。各产物保留完整类型、ABI、dispatch、初始化、root 和 relocation 数据。未来 M23-8/9 以这些真实产物接入多 image 启动与 program-link，按依赖顺序初始化并通过现有 no-throw gateway 进入 managed 代码；不提前冻结没有实际 producer/consumer 的 program/core binding 结构。

String 由前端解析为实际 typed class，MIR/LIR 与 Link 使用同一 provider 的 exact type、TypeDescriptor、registration 和 relocation。现有 C ABI 的 `scoop_td_String` 表示 runtime 所需的 String descriptor 地址；链接使用实际声明的定义，不从固定 CORE identity 重建它，也不按名称或同布局替换类型。对象头为 16 bytes，length offset 为 16，inline bytes offset/minimum 为 24，alignment 为 8；`InlineBytes` 的 size/stride/alignment 为 1，object/inline scan 为空。保留这些真实表示和边界检查，不单列重复的 String 布局/scan 副本、capability-kind digest 或 program/core binding 证明。

M23-6 删除没有实际 producer/consumer 的 `ScoopRuntimeCoreBindingsV1`、`ScoopProgramDescriptorV1`、固定 String capability ID 及其 LLVM 布局镜像和专用测试，不把它们改名后保留。未使用的 program/core magic `0x53434f4f50505247`、`0x53434f4f50434f52` 退役且不复用。实际发射的 image、root entry、type/callable、storage、immortal、initialization 和 safepoint 记录继续使用既有格式；这项删除不改变实际 runtime C ABI、String 表示或其 fingerprint。M23-8/9 的多 image 启动与 program-link 仍留在后续阶段，按实际入口和引用需要定义数据，不提前冻结新的 program record 或另建 String 授权表。

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

- intrinsic type不声明字段/primary constructor，也不等价于零字段普通struct/class；不能据此派生零大小布局、全等equals、`Int()`字符串、字段访问、解构、copy update或公开零参数constructor。编译器合成的literal/boxing/allocation entry不进入源码候选集；任何用户可调用constructor或转换仍须显式声明，唯一例外是13.10封闭规定的`Ptr<T>(raw: ULong)` unsafe construction entry；
- intrinsic type的implements列表、普通成员body、override/operator规则与普通类型一致；单个成员也可像上例一样另用function intrinsic提供实现。初始intrinsic type至少覆盖八种canonical integer representation、Boolean、String、`Array<T>`/`MutableArray<T>`以及13.10的`Ptr<T>`/`FunPtr<F>` family；integer登记项必须封闭地给出signedness与8/16/32/64位width，alias本身不能再次登记为intrinsic type。`core_int`/`int_*`表示32位canonical `Int`，64位signed表示使用独立的`core_long`/`long_*`；unsigned同理区分`UInt`与`ULong`。登记表可按同一契约增加其他compiler-represented value/reference type；
- intrinsic type可以是generic，但其登记项必须完整规定declaration kind、type-parameter数量/bound及representation family；所有参数按3.2固定为invariant。`Array<T>`与`MutableArray<T>`各要求一个无bound参数；它们的每个fully specialized application仍是普通generic class application，只是对象布局、元素stride和GC扫描由携带concrete element type的typed intrinsic representation产生。不得同时保留普通class application与独立built-in array type两种identity；
- 前端识别core源码中的intrinsic annotation，并检查以下name、target、shape、signature与唯一性规则；测试可直接提供相同源码输入。此处理不建立后续stage的来源授权能力。

- 除非有单独说明，`@Intrinsic` 不能与其他任何注解共存。integer registry中除`div`/`rem`外的纯scalar operation与conversion是一个封闭例外：其声明必须同时带`@NoGC`；`div`/`rem`不得带。13.10列出的pointer intrinsic继续按该节例外组合`@NoGC`/`@Unsafe`。
- `name` 必须是编译器内置intrinsic登记表中的已知标识；未知`name`、错误annotation target、与登记shape/signature不符或同一intrinsic kind存在多个provider都是编译错误（用户不能声明自定义intrinsic）。各intrinsic在编译pipeline中的展开阶段由实现大纲规定。

### 13.2 `@NoGC`

```
annotation class NoGC
```

- 用于function/method及9.1.5允许的explicit accessor/struct secondary constructor：指明该callable不会/不应与GC有任何交互——有body的callable中不读写任何ref value，也不创建任何ref type实例。唯一无body的组合是`abi = "scoop"`的top-level `@Extern` function：此时`@NoGC`是由FFI作者承担的callee contract assertion，并使其`GcEffect`取`NoGc`；省略时取`Managed`。C ABI extern本身已由C边界契约固定为GC leaf，不接受`@NoGC`这一重复且易混淆的拼写。
- struct secondary constructor的`@NoGC`要求完整参数、构造结果、委托求值与body中的运行时值均为GC-free，`this`只能委托primary或另一个`@NoGC` secondary constructor；未标注的secondary即使body看似纯净也保持Managed合同。primary struct与enum variant的直接值组装可出现在`@NoGC`代码中，但实参求值与完整结果表示仍须满足GC-free约束。参数缺省表达式在caller求值，遵守caller的GC effect，不因callee的`@NoGC`而自动获得GC-free资格。
- 也可用于`struct`或`enum`，作为“该concrete value type必须GC-free”的静态契约。非generic声明在字段类型解析后立即验证；generic声明本身没有GC-free真假值，每个type parameter全部resolve后的实际类型分别验证。对fully specialized enum，契约同时要求enum整体及每个variant均为GC-free。`@NoGC`不能用于class/interface，因为它们是ref type。
- 编译期检查；不符合约束是编译错误。
- generic `@NoGC` callable 可以在签名或 body 中使用类型参数；未特化的generic本身不被判为GC-free或非GC-free。每个实际影响参数、返回值、receiver、局部值或表达式表示的类型参数，都会形成“实例化实参必须GC-free”的类型化条件，并经generic调用链向外传播；只有type parameter全部解析后的具体实例才能用concrete type的GC-free flag验证并成为`@NoGC`实例。未参与运行时表示的phantom type parameter不产生条件。
- `T : value` 只保证实参是 value type，不保证其递归表示中不含 managed ref，因此不能代替上述 GC-free 条件；`T : ref` 则不可能满足该条件。当前没有单独的源码 bound 语法来声明 GC-free，条件由 `@NoGC` body及其调用图推导。
- 这样的函数可以安全地跨越 FFI boundary（例如作为 FFI 回调）。
- 该约束也意味着 `@NoGC` 的成员函数只能属于 value type：class method 有隐含的 `this` 参数，而 `this` 是 ref value。
- 9.1.6的release block不是普通callable或`@NoGC` target；编译器对它及其可调用闭包验证更窄的release-safe effect，并以`ReleaseValue`而非仅`gc_free`约束所有运行时值。一个callable仅有`@NoGC` annotation并不自动获得从collector reclaim上下文调用的资格。

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

LIR在`ValidatedLirTargetSelection`下把source contract唯一正规化为target-specific native contract。`NativeExternalSymbolKey { target_profile: TargetProfileWireId, native_link_symbol: bytes }`使同一logical symbol在不同target中不共享identity；当前Darwin/AArch64的`MachOExternalUnderscore`要求输入非空、无NUL且不以LLVM escape byte `0x01`开头，并逐byte映射为`0x5f || logical`，输出长度为输入长度加一，使用 checked 长度计算与实际分配错误处理。它不做Unicode、大小写或已有underscore折叠，因此`foo → _foo`、`_foo → __foo`，Scoop mangled symbol同样只前置一个underscore。Scoop LLVM object与generated-C bridge object verifier必须应用相同规则，`native_link_symbol`统一保存真实Mach-O symbol-table bytes，不能一方保存logical C name、另一方保存`nlist` name。当前Darwin链接模型以这些bytes作为冲突键，因为object中的undefined reference不携带源码`lib`归属。

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
- 源码中的 `pointer + offset` / `pointer - offset` 分别按 `ptr_plus` / `ptr_minus` 的契约处理；`offset: Long` 以**元素个数**计，其数学byte displacement为`offset`与`sizeOf<T>()`数值的乘积，与非ZST C object pointer算术一致。带 `offset` 的 `load` / `store` 使用相同的元素偏移语义。对ZST pointee，任意offset的物理byte displacement恒为0且所得pointer bit值不变；`load`产生该exact ZST值，`store`不写payload byte，但receiver、offset、value仍求值，且unsafe调用者仍必须保证pointer非null、满足alignment/lifetime并指向相应逻辑place。算法不得用ZST pointer值变化表达迭代进度；`Ptr<Unit>`若需要逐byte移动必须先使用语义上正确的`Ptr<UInt8>`，不能把opaque `void *`自动当byte pointer；
- `addressOf` 是 intrinsic，且带 **lvalue 约束**：实参必须是参数、局部变量、全局变量，或值类型成员方法的 `this`，取的是该 place 实际存储的地址；对临时值、字面量、计算结果等非 lvalue 表达式调用是编译错误。对 `this` 取址时指向 3.3 规定的方法局部副本，不是调用方的 value 或 box payload。
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

Scoop ABI extern 的参数与返回值使用普通 Scoop typed ABI，不经过 C ABI storage bridge：ref value 是直接 managed pointer；aggregate/value return沿用普通 Scoop 函数的 typed return storage规则。当前 Darwin / AArch64 profile 中，scalar、ref/raw pointer/function pointer 与 niche enum 直接传递；所有非空 tuple、ordinary struct、tagged enum 与异常记录都通过 caller-owned、按 exact layout 对齐的间接 storage 传递，间接返回 storage 位于所有源码参数之前；Unit 返回为 machine void，其他 ZST 参数/结果只保留 typed identity而不传payload。被调方必须按同一 physical signature 实现该 ABI，不能把同形 C struct 的按值参数/返回直接用作 shim，也不能假设 C 编译器会为它选择与 Scoop value ABI 相同的寄存器或栈位置。

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

M13 的 core 源码形态为：

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
