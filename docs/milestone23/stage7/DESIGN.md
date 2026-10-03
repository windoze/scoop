# M23-7 设计：跨 Cone 泛型、ODR 与泛型委托扩展属性

状态：实现中（2026-09-27 开始）；2026-09-29 按 [M23-6a 共同 HIR 设计](../stage6a/DESIGN.md) 修订职责和实现顺序。依赖 M23-6 的实际产物基线及已于 2026-10-01 [验收的 M23-6a](../stage6a/ACCEPTANCE.md)。共同 HIR 前置条件已落实，本阶段继续完成机器闭包、ODR 与委托运行；已有工作保留在 [进度记录](PROGRESS.md)。

本阶段消费 M23-6a 的共同声明、正文、调用决议与具体化结果，完成跨 Cone 泛型的机器定义／引用闭包、ODR 合并和每个 receiver application 共享一次初始化的泛型委托扩展属性。下游仍只凭上游 `.slib` 编译；完成标准包括实际源码、产物消费、链接运行和移动 GC。统一类型表示、导入默认值、解构、上下文推断及 source-only gate 的修正归 6a，不再作为本阶段逐语法扩充 imported 支持的实现顺序。

权威合同为 [语言规范](../../specs/SCOOP-SPEC.md) 3.2、8.5、9.1.1、9.2、12.5，[运行时规范](../../specs/SCOOP-RUNTIME-SPEC.md) 2.2、2.7、2.8，以及 [实现规范](../../specs/SCOOP-IMPL-SPEC.md) 2.13。本文细化 [M23 总设计](../DESIGN.md) 3.4、4.3、7、10，修订其中以整组成员集合相等判定 ODR 的旧要求；既有 specialization、group/member 身份及 mangler 保持。

## 1. 范围与实现基线

### 1.1 本阶段能力

| 能力 | 成功后的行为 |
| --- | --- |
| 泛型函数 | top-level、extension、local function 及 final generic method 可从依赖模板实例化；显式实参、`_`、推导、class/interface 与 `value`/`ref` bound 使用同一前端 |
| 泛型名义类型 | class、struct、enum、interface 的完整 application 保留真实字段、父类型、构造器、成员、属性、默认实现、exact RTTI 与 ABI |
| 模板内部实现依赖 | 可引用定义处合法的 private/internal helper、类型与属性；名称查找仍只公开原来的 public binding |
| 函数值与生成实体 | 实例化正文中的 lambda/local function、capture、callable reference、function adapter、coroutine frame/step/slot/start 形成完整的 typed 定义与引用 |
| 泛型委托扩展属性 | getter/setter 共用由 property origin 与完整 receiver arguments 决定的 lazy storage、cell、failure root 和初始化实现 |
| 产物与 ODR | Strong 与 ODR 共用正式生产、读取和对象处理路径；重复 member 必须定义一致，同组独立 helper 可以按实际需要分别发射 |

同一泛型既可使用定义 Cone 已知的类型，也可用下游定义的 class、struct、enum、ZST 或含 managed reference 的类型实例化。core 中的泛型声明沿用同一路径，包括 `Option`、数组与协程协议；不恢复 core 专用物化入口。

本阶段不开放 generic alias、interface method 自有类型参数、virtual generic method、polymorphic function value、polymorphic recursion、运行期 dictionary 或类型擦除。现有 FFI 声明和 generated bridge 的类型替换、ABI 与 requirement 必须完整；一般 native provider 查找仍归 M23-10。

### 1.2 已有能力与修订后交接

下表保留历史实现落点，M23-6a 负责把其中的本地／导入语义路径收敛为共同模型。本阶段继承该结果；已有泛型 fixture 同时是 6a 迁移回归和本阶段机器／运行验收，不重复实现已完成的语言规则。

| 当前入口 | 已有能力 | 6a 交接后 M23-7 的工作 |
| --- | --- | --- |
| [`identity/entity/odr.rs`](../../../compiler/identity/src/entity/odr.rs) | 四类 specialization、group/member、role/discriminator 的 typed key | 复用实体身份；按实际 member 建立定义记录 |
| [`cross_cone_interface/section.rs`](../../../compiler/hir/src/cross_cone_interface/section.rs) | 声明、binder、默认值、正文及依赖引用 | 消费 6a 统一的导出投影与编解码，补齐实际物化和委托所需记录，不另建正文模型 |
| [`hir-lower/concretize.rs`](../../../compiler/hir-lower/src/concretize.rs) | 已有本地／导入具体化实现 | 6a 合并查询与固定点后，检查本阶段所需实例的实际 Strong/ODR 定义闭包 |
| [`concretize/callables.rs`](../../../compiler/hir-lower/src/concretize/callables.rs) | bound-call 具体化 | 消费 6a 已选 conformance、完整实参与 exact 目标，接通真实机器定义和派发 |
| [`globals.rs`](../../../compiler/hir-lower/src/globals.rs) | 普通 extension delegate，显式拒绝 generic delegate | 增加真正的 source template 与 concrete specialization，替换拒绝分支 |
| [`lir/foundation/cone.rs`](../../../compiler/lir/src/foundation/cone.rs) | 完整 Strong 输出，明确拒绝 ODR | 正式输出直接持有完整 canonical foundation 与 Strong/ODR 定义，不再借用 OdrFree 包装 |
| [`codegen/emission/objects.rs`](../../../compiler/codegen/src/emission/objects.rs) | 已支持对象集合，分离 callable 与非 callable 对象 | 扩展现有分区、weak 定义和 metadata 发射；不重新搭建多对象基础设施 |
| [`artifact_production/layout.rs`](../../../compiler/driver/src/artifact_production/layout.rs) 与 [`layout_compile_closure/read.rs`](../../../compiler/slib/src/layout_compile_closure/read.rs) | 真实产物组装、共有 Compile/Link 读取及结果复用 | 接入模板、ODR 定义目录和跨 artifact 重复定义检查 |

M23-2 已有 identity 并不表示模板正文或 ODR 机器能力已实现。M23-6 的泛型相关 compiler/unit 测试也不能代替本阶段的独立 `.slib` 发布和消费。

## 2. 两项必须先修订的合同

### 2.1 ODR 比较实际重复的 member

现有 `FunctionAdapterIdentity` 以 `StructuralType(FunctionShape(target))` 为 group，但静态 adapter 的 member identity 同时包含 source 与 target signature。Cone A 可能只需要 `S1 → T`，Cone B 只需要 `S2 → T`。两者共享函数形状 T，拥有不同 adapter member，都是合法程序。要求两个 group 的全部 member set 相等会错误拒绝这种组合。

同类问题也存在于按需生成的 equality、box、coroutine support。为了补齐一个 group 而为每个新类型继续生成所有 coroutine helper，还可能使 `R → Continuation<R> → Continuation<Continuation<R>>` 无穷展开。

因此采用以下规则：

1. group 继续表示规范规定的语义归属，member 继续表示独立的可合并定义；不增加新的 specialization variant。
2. 每个 artifact 列出自己实际发射的 member。相同 member 的 key、ABI 和完整定义必须一致；独立 member 的集合取并集。
3. 每个实际操作和已发射定义的必要引用必须闭合。实际 TD 的完整 layout/scan/dispatch、callable 的 EH/stackmap、初始化单元的状态和根不能省略。
4. 同一 artifact 内，一个 member 只定义一次；多个 artifact 的等价定义才可由 native linker 合并。不能把重复 strong 定义混入 ODR。
5. 不允许因同组已有另一个 member，便接受缺失 body、错误 relocation、不同 ABI 或错误初始化状态。组相同本身不构成定义相等。

这一调整保留现有生成实体的 root 规则，避免为使用集合制造新 identity，也不要求编译器预生成所有可能的 source-to-target adapter。

### 2.2 支持声明与链接可见性

generic body 的 private/internal 实现依赖来自定义处已经解析的 typed 引用。依赖为参数自由 helper 时，定义 Cone 必须实际发射该 helper 及执行所需的类型、存储和初始化；下游只有签名与外部定义引用，不复制其正文。

普通顶层 stored property 统一在声明阶段生成实际 getter/setter，涵盖 private/internal 的静态初值。源码泛型正文中的属性读写、复合赋值及嵌套 callable 调用原访问器；访问域和 RHS 求值顺序沿既有前端规则处理。访问器本身持有 backing 存储与必要的初始化 ensure，作为参数自由 hidden support 在提供方发射。这样提供方、下游及再次发布的实例具有相同调用目标，全部类型实参共用原状态、初始化单元和 GC root。不得只在导入转换中把直接存储访问改成调用，导致同一 specialization 的两边正文不同；也无需为这一源码能力新增一套全局存储导入表示。

`TemplateSupportHidden` 只表示普通的跨对象可链接、native hidden 定义。前端的实际 template 引用和原声明可见性足以决定保留该符号；不生成逐边授权表、访问收据、来源包装或“同次编译”证明。若同一 subject 同时需要公开机器导出，保留 `ConeStrong`，不以 hidden 缩窄其他合法使用。源码 public lookup 仍由原有 binding 表决定。

generic helper、template-owned lambda/local function 或含宿主 binder 的 constructor/member 需要在消费方替换，才随模板导出。是否字面出现 type parameter 不能代替词法归属：捕获外层值的局部函数仍由其 enclosing application 物化。

## 3. 消费 M23-6a 的共有 HIR

### 3.1 导出根与唯一数据来源

模板根来自公开 generic callable、generic nominal 的可调用/继承表面、generic extension property，以及这些根真实引用的支持模板。generic nominal 的完整 source shape、modality、binder bounds、字段、variant、父类型、成员和 property 继续保存于共有声明表，不再复制到一张“泛型声明表”。

下列执行数据全部采用 6a 的共同语义节点和原身份。已有数据直接迁移；尚需补齐的委托或物化内容进入同一模型，不产生另一套导入表示：

| 数据 | 必需内容 |
| --- | --- |
| callable body template | 原 typed callable declaration/generated lexical owner、完整 typed body、局部值与 capture、已有条件约束 |
| constructor/common initialization | 原 nominal/constructor identity、delegation target 及求值序列、primary field 写入、按源码顺序的 field/delegate/`init` 操作、secondary body |
| generic delegate template | 原 extension property identity、effective delegate type、`by` body、可选 provide role、get/set typed target 及条件约束 |

abstract slot、intrinsic 声明和 source extern 继续使用各自已有 implementation 分支，不为它们填空 body。参数自由 source helper 只增加必要的共有支持声明及真实机器导出。未使用的本地实例、arena 索引、AST、resolver scratch 和失败候选不进入模板。

外来指针与布局 intrinsic 使用实际声明的 owner、binder、签名及 effects 参与普通候选选择和约束求解。`Ptr<T>` 的结构表示仍关联真实 core 泛型 owner，成员的 owner 与方法类型参数分别绑定；已选 intrinsic 直接正规化到既有指针/布局节点，不产生空模板或虚构机器函数。`addressOf` 保留源码原 place，泛型 pointee 沿现有 `Ptr` 条件传播 GC-free 要求；布局查询仅要求值类型，待代换后求布局。默认值和共有泛型正文复用同一组 typed 节点、局部 selector 与 raw global 引用，保持实际求值顺序、方法 `this` 副本和 ZST 地址语义。该能力不增加 wire tag 或 runtime ABI。

显式 `Ptr<T>(raw)` 的外来入口由普通 lookup 找到的实际 `core_ptr` 类型提供，参与同层函数与名义构造候选选择。导入别名和固定 application 的 typealias 保留既有推断、遮蔽及具体性规则；后者是非参数化候选。与当前 Cone 共用特殊构造检查，保留 `raw: ULong`、unsafe、常量非零和递归 GC-free 契约，并直接产生已有 `PtrFromNonZeroULong` 节点。共有泛型正文及默认值保存该节点与 pointee 类型，下游可用自己的 GC-free 值类型实例化并再次发布。不新增普通 constructor 声明、来源证明、wire tag 或 runtime 检查。

模板可引用 direct 或 support provider 的声明。只保存原 typed target 和实际 provider；re-export 不复制正文、不改 origin。一次 reader 得到的不可变模板和声明供后续查询复用。模板内已经绑定的引用不制造消费方 public lookup observation，也不重新枚举 hidden 名称；只有消费方实际源码 lookup 才进入原候选记录。

消费方源码中的限定类型路径从当前与 direct dependency 的同一公开包集合选择最长前缀，再沿实际 typed owner 解析。普通类型、泛型 application 和 alias 共用短名入口；同包冲突、最长前缀遮蔽及仅供支持使用的依赖边界遵守普通 lookup 规则。导入收集产生的不可变包类型索引供后续 lowering 复用，模板仍只保存已绑定的 typed 类型，不保存源码包路径或另加 wire 字段。

exact、star、public import 在选中 nominal owner 后沿其真实 public 静态 namespace 继续，原 provider 仅作 support 时也保留终点公开绑定。direct package index 决定源码入口，不能用终点 provider 的角色再次过滤合法嵌套成员。产物 reader 保留普通绑定、类型及引用检查，删除重复的 direct-provider 资格检查和专用输入；不追加外层 lookup 路径或新的证明记录。该规则自 `hir/cross-cone-interface/39` 起启用。

公开 typealias 的目标通过实际类型签名或直接 typed alias 边记录，外部 `AliasTarget` 只承担目标引用和实体归属检查，不再要求或保存别名专用的名称来源证明。源码的普通名称查找、可见性、类型实参和循环检查保持；已解析目标可来自可见类型的静态嵌套命名空间。该别名规则自 `hir/cross-cone-interface/38` 起启用；当前共有 HIR 格式为 `/42`，旧 `/41` 及更早产物与缓存重建，不改变 runtime C ABI、对象布局或 GC 契约。

当前 Cone 在 SemanticHir 完成后，从公开声明、默认值与泛型正文确定导出支持闭包，并独立收集实际物化需求；二者由 6a 的共同查询连接，不用导出可见集合替代机器根。完成 LocalConcrete HIR 后，从实际已物化的泛型名义 application 及实际导出的泛型成员的 receiver、参数和结果类型沿表示依赖，把当前 Cone 所需的源码名义声明补入同一支持集合，并闭合其字段、成员和模板引用；普通函数正文中用于泛型 payload 或共有成员 ABI 的私有类型也必须有完整表示依赖。只有支持根增加时才扩展源码投影及对应的 shape support plan，最终不可变结果由 HIR 类型语义、MIR/LIR 布局和正式共有 section 复用。支持声明保留原 typed identity 和可见性，不生成 public binding；所有本地私有物理声明、未调用模板、未求值默认值和整个类型 arena 仍不构成共有机器根。该结果是当前编译的数据投影，不新增产物字段或来源资格。

### 3.2 默认值与泛型正文共用节点

普通函数、默认值、泛型正文、构造初始化和词法 callable 使用 M23-6a 的共同 typed expression、statement、pattern、local、capture 与替换操作。不同根记录保存各自的参数与 owner：

```text
ExportDefaultTemplate
    parameter origin + existing calling protocol + shared HirBody

ExportGenericBody
    source callable / initialization owner + shared HirBody + predicates
```

源码产生和产物解码产生同一语义正文。编解码可机械转换索引与持久引用，不能创建 `GenericExpression`、`ImportedExpression` 或独立默认值执行器；默认参数也不能伪造 generic body 的 owner。必要的格式、类型连接和引用检查在 reader 边界完成，语言规则只由共同 `hir-lower` 实现。

模板表达式必需保存已解析的类型、target、effect 和定义位置；类型允许声明作用域内的 binder。调用节点保存已选声明、宿主与 callable 两组符号化类型实参、已提交的 dispatch 方式和完整实参求值计划。只有完整替换后才产生 persistent exact application，不能给 `T` 或 `_` 伪造 exact ID。

bound member 节点保存实际 class/interface bound 与原 slot/callable identity。实例化后只完成规定的 exact dispatch 选择，不用实际类型额外出现的方法重新参与 overload resolution。

消费方按原 typed declaration 从共同 HIR 查询直接取得所需正文，不先转换为独立 imported template，不分配当前 Cone 的同名声明。源码声明、符号化 application 与最终 concrete function 保持各自的 typed ID；同一具体化队列按原定义与完整实参处理所有请求，最终 materialization 保留定义方身份。来源只影响外部定义关联和发射归属，不改变语义节点或替换流程。

constructor、field、variant、property accessor、local value、loop target 和 callback registration 分别使用其原 typed ID。loop/cleanup、`try`/`finally`、enum pattern、callable reference、receiver adaptation 与 source location 都沿用既有 HIR 语义。

lambda/anonymous function 的模板正文保留按定义处捕获顺序排列的类型表，正文读取以该表中的位置引用闭包输入；该位置是当前正文内的 ABI 索引，不是新的全局实体身份。创建嵌套闭包时，捕获来源明确区分当前局部值与外层闭包输入。局部具名函数按现有 ABI 将捕获值作为前置参数，正文投影直接引用这些真实参数。不得将 request-local `BindingId` 写入产物，也不得把闭包输入误当作当前函数尚未定义的局部值。

导入闭包创建表达式保存原 generated body、定义路径、有序类型实参和本次捕获来源，正文的捕获槽按顺序关联定义处的真实 binding。捕获源表达式保留首次引用的定义位置，求值位置使用创建表达式的实际上下文。局部函数的前置捕获参数是原值的别名，继续被闭包或内层局部函数捕获时保留同一局部值身份；关联结果不依赖函数遍历顺序。捕获值可以属于词法外层的 application，环境字段复用 HIR 已解析的原值身份，MIR 不以 context 必须相等重新限制捕获。具体化复用导入 callable 队列，并生成既有 concrete lambda/anonymous 实体；其 materialization 沿原 enclosing callable 或 initialization application，环境布局、invoke、函数值调用及 GC 继续走普通 lowering。再次发布使用同一共有闭包节点，不增加外来闭包 wire、运行时或本地伪源码声明。

导入的 callable reference 保留定义方的完整 invoke key、词法父模板和有序宿主实参；已选目标区分实际模板 application 与普通依赖 callable use。命名函数、局部函数、绑定成员及绑定扩展复用现有 concrete 函数引用与 MIR invoke，普通依赖目标直接引用提供方代码，不在消费方伪造源码函数。实际引用目标随创建表达式进入既有可执行依赖集合，尚未实例化的模板引用不成为机器根。局部函数的捕获值按原前置参数 ABI 传入；绑定接收者在创建表达式处只求值一次，保留值拷贝或引用拷贝，以及原 virtual/interface 派发。环境字段、局部值身份和 GC 扫描沿既有闭包路径处理。

消费方源码中的 `::name`、`receiver::member` 和绑定／未绑定扩展引用按语言规范 8.1.4 解析。每个普通查找层同时收集本地和依赖声明，使用同一函数签名约束与最具体候选比较；泛型引用须由期望函数类型和实际宿主接收者确定完整实参，没有期望类型时仅唯一非泛型候选可推导。函数引用保留声明的全部参数，不套用默认参数或展开 vararg；函数类型型变仍由独立值适配处理。选中的依赖目标保存原 callable use 或完整 application，创建处的 invoke 使用本地词法 identity，不能复制外来源码函数。invoke 的模板父节点是结构路径所在的最近词法 callable，共有导出与本地具体化使用同一归属，不能把嵌套引用统一挂到最外层函数。绑定 receiver 只在创建处求值一次并按值保存，成员仍保留原 virtual/interface 派发；再次发布使用既有共有引用节点、真实目标与 import/re-export 关系。

引用创建不是普通调用现场：源码查找完成后，模板保存已解析目标及定义位置，invoke 的实际目标沿现有可执行依赖集合发布；不为创建表达式伪造实参或调用记录，也不重放已经完成的名字查找。

原语 intrinsic 成员没有独立机器函数，绑定引用在 HIR 保留已选声明与正规化的 intrinsic kind，MIR 的 invoke 复用普通运算降低，包括整数除法异常；共有模板仍发布原成员引用，消费时从已解析声明恢复该运算，不制造 Strong 函数依赖。

默认参数展开到可执行正文时，函数引用使用实际创建点的词法 root、新的结构路径和调用方完整宿主实参；仅在默认值模板之间代换时，继续保留原定义身份并替换其宿主实参。已选目标、捕获内容以及原定义与求值位置保持不变。局部函数引用的捕获关系按目标函数前置捕获参数所属的 application 解析，invoke 归属创建点不改变目标函数的绑定或捕获值身份。再次发布后的完整正文保留这个 invoke identity，不把原默认参数的非泛型或其他泛型作用域混入当前正文的 application。

导入的 bound member 与直接成员引用使用定义处已经选择的声明。class bound 的目标沿原成员 application 降低；interface bound 保留完整接口 application、原成员与 dispatch slot、接收者参数及替换后的签名，待接收者具体化后从实际 conformance 选取实现。普通调用与绑定函数引用共用该选择：值类型直接调用其实际方法，class 实现保留原 virtual 派发，接口接收者或留给派生类的 abstract obligation 使用原接口槽。选中的实际实现属于接口默认方法时，接收者按该方法的精确接口 owner 进行装箱或引用转换；不得把所有 bound 调用统一改成装箱和接口调用。外来普通类型和 core primitive 的实现同样来自共有声明中的 conformance，不复制为本地源码声明，不在 MIR 重新做成员查找。primitive 的 conformance 实现按已使用的 bound 接口加载，普通算术或成员解析不因此选择全部 primitive 方法。 已经替换为具体 primitive 的导入 bound 调用同样按需保留其实际声明与 conformance，不能依赖消费方再次执行源码候选选择。 模板正文转发到泛型 callable 时，已经具体化的名义 bound 实参也在创建 application 时保留所需声明；不重做已完成的语言约束检查。需要的实际目标进入既有可执行依赖集合，未选中的声明仍只作为模板支持。

源码声明中的名义 bound 使用共同的 nominal application 和原 typed 声明；class 上界与接口上界不再按本地／外来类型分支保存。允许同一类型参数组合来自不同 Cone 的接口及 class 上界；单一 class 上界、重复接口、kind bound 互斥、完整实参和可访问性规则由 6a 的同一约束过程处理。本阶段消费该结果及替换后的实际关系，不另存来源专用 bound 集合。

bound callable 的 receiver 使用完整 `SignatureTypeKey`，可以是定义处 binder，也可以是默认参数展开后已经替换的 nominal application 或结构类型；bound、原 member/slot 和完整函数签名继续保留。默认值中的 bound 调用按同一目标选择降低，再次发布时不把具体 receiver 伪造成 binder。正文与引用目录使用同一个完整 bound target，并收集 receiver、接口与签名中的实际类型引用。默认值展开生成的临时语句及控制节点使用本次求值位置，内部表达式继续保留提供方定义与实际求值位置；默认值求值的 effect 错误定位到本次调用处，不得把提供方的裸字节偏移绑定到消费方文件。该 payload 的 field 1 从专用 binder 对变更为类型 key，HIR interface 升至 `/37`，reader、required inventory、profile fingerprint 与缓存同步迁移，旧 major 需重建。

共有名义声明中的参数自由构造函数按实际可物化签名导出对应 MIR 构造 binding，包括泛型正文所需的 private/internal 构造函数。源码可见性由 HIR 候选选择和定义处访问检查决定，不能再用 public/protected 过滤已经生成的构造函数 ABI。消费者引用定义 Cone 的实际 Strong 构造实现，不另发同名定义；producer 和 reader 共用同一构造声明选择。

未装箱值的 TypeDescriptor 不携带接口运行时表；其完整 conformance 仍保留在 HIR 和 MIR 类型关系中。泛型值只有实际产生 box 时才导出 payload 的物理 dispatch schema，并由该 box 的 TD 引用；单纯的 bound 直接调用不请求 box 或 adjust helper。LIR 发布与 reader 对未装箱值使用空运行时分派，对实际 box 要求完整接口表及目标，因此同一 payload 的直接使用与装箱使用可以贡献不同 helper 成员而保持共同值布局和 TD 一致。

派生相等复用语言规范 11.11：外来泛型 struct、enum 从共有字段及 variant 的真实 identity 生成完整比较正文，替换字段类型并按声明顺序短路；enum 先检查 tag，再读取 active payload。字段的本地、外来及 bound 成员共用普通相等选择，显式同类型 `equals` 取代派生候选，其他重载不屏蔽它。模板与绑定引用只保留完整 owner type，仍使用已有 `DerivedEquality` generated key；泛型及结构 owner 按 exact type 发射 ODR helper，参数自由外来名义 owner 引用定义 Cone 的 helper。派生正文没有源码词法根；表达式保留派生需求或字段诊断的位置，reader 从当前物化产物解析 helper 的 generated key，从位置所属产物检查真实文件、context 和 span；提供方模板无须预先物化消费方的 exact helper，位置也无须归属不存在的 helper 源码声明。完整 exact owner 的 materialization 使用 `NoSubstitution`。不可比较的字段给出原字段／variant 路径，不引入额外接口、wire 节点或 runtime 相等槽。真实产物验收覆盖独立 struct／enum、嵌套 payload、显式重载、ZST、大值与引用字段、默认值、函数引用及再次发布。

模板中的局部具名函数声明不产生运行时语句。实际直接调用复用原 typed callee 及完整类型实参，把已解析捕获表达式按原顺序放在显式参数之前，并进入 6a 的共同具体化队列；不为声明标记生成 Unit 占位表达式，也不把 provider 的源码函数复制为当前 Cone 的 `FunctionId`。

无自身类型参数的局部声明保持 `PersistentFunctionId`，其继承实参通过 enclosing callable application 或 initialization application 表达；局部 generic 声明另外保留自身实参组。initializer 中的生成正文沿原 generated callable、unit 与 property/type 关系取得声明所属 Cone，不增加可按名字导入的 initializer binding。词法正文从已有正文表读取，不成为可按名字导入的源码接口。该正文的 binder 已在定义处检查，消费端保存有序替换参数及已有条件约束；它们与需要参与源码推断的声明参数使用不同表示，不能伪造一组无约束声明参数来填充接口。

构造初始化表按原名义声明保存公共序列与各构造器：class 的 common initialization 只保存一次，primary 保留 base delegation 和参数到字段的写入，secondary 区分 `this` 与 terminal `super` 委托，并保留其后执行的正文；struct 区分 primary 值构造与 secondary 委托。委托实参、字段初始化、`init` 和次构造正文共用含局部值表、typed statements 与实际结果列表的执行片段；纯语句片段的结果列表为空，不填充假的 Unit。构造参数用原源码位置对应的 `Parameter` selector，初始化接收者用 `This` selector，字段访问继续使用原 typed field identity。class 的委托复用同一个已分配接收者，common sequence 中的 stored/delegate 写入与 `init` 按源码顺序执行。shape、参数调用协议和 bounds 仍由既有共有声明表提供。

消费方以原普通或泛型名义 owner 查找构造候选，宿主实参复用现有推断、参数协议和 bound 检查。选定的构造保持原 constructor identity 与实际 owner application，执行片段进入已有具体化队列，最终产生正常 struct constructor 或 class initializer。`this` 与泛型基类委托均保留同一已分配接收者，公共初始化只在 terminal 构造执行；未调用模板不成为机器根。构造生成的参数读取、字段写入与委托调用保留原定义位置，并复用提供方已有的 constructor execution context；声明所在文件上下文不代替执行上下文。

类型别名固定泛型宿主实参时，构造推断使用别名的完整 application；普通构造继续使用调用上下文的期望类型，不以被调用类型自身覆盖该约束。

数组转换构造从实际 core 数组 owner 取得 binder，提供 `Array<T>(source: MutableArray<T>)` 或 `MutableArray<T>(source: Array<T>)` 候选，并与普通同名函数进行同层 MSC。声明准备及固定 alias 类型位于共同候选环境，候选各自求解和提交；显式实参、`_`、期望结果、导入别名和本地／外来 typealias 均复用普通构造规则。最终表达式直接使用现有 `ArrayClone` 和完整目标 application，保留新对象 identity、logical size 及元素快照；默认值、泛型正文和下游再次发布不增加格式分支。

### 3.3 默认参数的边界

public default 仍只能直接引用覆盖其完整调用域的实体；generic body 可以引用定义处合法的 narrower 实现。二者共享节点不共享访问规则。private generic helper 自己的默认值按该 helper 的实际调用域检查，不能把正文依赖资格传播给 public default。

显式实参、默认参数、`vararg` 和前置参数引用继续按 8.5 求值。默认值中嵌套泛型调用的 binder 随外层模板替换；实际省略参数时才展开默认值。定义位置和求值位置分别保留，实例化失败报告消费方使用点，并附 provider 中的约束/声明位置。

当前与依赖 callable 使用 6a 的同一推断过程：先处理无需上下文的实参，再以既有 partial solver 的完整 hint 检查待定输入，直到固定点；字面量默认阶梯仅在没有其他进展时启动。后续元素／形参可为前面的空数组、裸变体、lambda 或函数引用确定类型，命名与位置写法不改变推断能力。每次无 hint 的种子尝试使用候选事务，失败状态不泄漏，全部输入检查成功且参数唯一后才提交。表达式及其 desugaring sink 始终保留源码索引，随后仍按源码序求值、形参序展开默认值与 vararg；宿主参数、方法参数和外层已解析 binder 保持原身份。

数组字面量与 `vararg` 的隐式 application 使用实际 core 协议 owner，与显式数组类型共用普通泛型表示。依赖调用映射保留元素、spread 的源码索引与命名整数组的完整值；显式求值和形参物化沿既有顺序进行。数组 literal／assembly、下标、写入、长度和转换节点在共有模板中直接替换完整类型与操作数，Export assembly 不依赖仅容纳本地声明的 class application id。成员选择、转换、循环、异常及移动 GC 均复用正常管线，真实产物覆盖空数组、复制 identity、ZST、大值和含引用元素。 动态 assembly 的溢出常量归实际 callable，递归 shape scan 的子程序合入原 primary atom；完整引用与对象边界使用已有物理定义检查，具体职责见实现规范 2.4。

`for` 在前端选定实际 iterator operator 和唯一 exact `Iterator<T>` conformance 后，直接展开为普通调用、接口适配、Option 操作与带 typed LoopId 的 while。泛型 binder、值装箱、每轮 binding 捕获、默认值和控制转移均复用已有节点与物化过程；外来 core、普通 provider 和消费方本地类型使用同一路径。专用 For 及 portable binding-plan 结构随生产入口移除，共有正文仅保存展开后的必要类型、callable、操作数和语句。

共享可移植表达式直接保存原 `EvaluationOrigin`，使已经在泛型函数或构造委托内展开的默认值保留该正文中的实际求值位置。消费泛型正文时恢复这条记录；消费默认参数模板时仍按本次省略参数的使用点替换求值位置。两者使用同一表达式转换，不从定义位置或模板声明位置猜测求值位置。

generic body 自身的 `current_source_location` 使用该正文中的语义求值位置，不因某个 consumer 首先实例化而变成调用点。实例化诊断链只用于诊断，不进入 body、常量、symbol 或 ODR fingerprint。

### 3.4 条件约束与闭包

复用已经存在的 nominal bounds、`RequiresGcFreePointee`、NoGC 条件和 `CLayoutFieldRequirement::CFieldSafeAndNonZst`。条件保存实际 binder/type 与声明位置；完整替换后由现有算法判定。新增闭包不能替代 FFI 的 C-safe、ZST、pointee 或调用约定规则。

producer 从实际 typed body 收集必须导出的支持引用和 source records。reader 检查记录、引用范围、binder 作用域、owner、签名连接与编码完整性；不重跑名字查找、重载决议、访问域证明或全量类型检查。已完成的同一引用与类型关系不在每个下游 stage 再完整检查一遍。

## 4. 消费方的 HIR 具体化

### 4.1 接入现有工作队列

M23-6a 已提供同结构的语义声明／正文和统一查询。当前 Cone 的 `SemanticHir` 与依赖 `ExportHir` 通过该查询进入同一个 concretizer；请求不再分成 Local/Imported 语义分支。不能先把外来声明复制成“本地同名函数”来复用本地索引，也不能恢复独立 imported template。

工作队列区分 callable application、nominal application 和 delegate specialization。key 分别复用 `CallableApplicationKey`、kind-specific nominal application 及 generic delegate unit key；每个 key 在当前编译中只生成一个 concrete 实体。排队中的构造状态只存在于 concretizer 内部，最终输出不含 pending request 或可选的必要类型信息。

处理顺序为：

1. 共有 lookup 和 M16/M17 solver 选定声明，求出宿主参数与 callable 参数的完整实参，验证所有 bound。
2. 根据实际类型操作、调用和 constructor/member 使用请求模板；没有执行用途的源码声明不触发 machine body。
3. 在定义处已解析的模板上替换类型、receiver、局部值与 capture，完成默认参数、`vararg`、bound dispatch 和条件约束。
4. 把正文新发现的实际 application 加入同一队列，已有实例直接复用。
5. 完成所有实例和 concrete type facts，输出独立的 `ExportHir` 与 `LocalConcreteHir`。只有后者进入 MIR。

普通外来参数自由 target 仍是 external reference。若上游恰好已经产生 `f<Int>`，消费方也不能把它当作新的参数自由源码声明；本阶段仍按同一模板生成本次需要的 ODR member，跨 artifact 去重由定义比较与链接完成，不另建预编译实例选优缓存。

### 4.2 类型与派发

generic nominal 的字段、variant payload、base/interface application、constructor 和成员使用完整替换结果。绑定到实际 integer/Boolean/String 或用户 nominal 的 conformance 查询同一共有声明，不依赖 core 位于本地 arena。

消费方目录以已有 `NominalDeclarationOwner` 同时索引参数自由声明与泛型模板，保留原 canonical identity 与共有声明。导入类型携带完整有序实参，使用现有 `TypeId` 表示已解析 application；不向本地源码名义 arena 复制声明。相等性、签名推断及替换均递归处理这些实参。concretizer 以原声明和 concrete 实参唯一登记表示，再由现有 exact type builder 产生 application 和 ODR group；provider 已物化哪些实例不影响消费方的实例集合。参数自由外来表示与消费方物化的泛型表示按其真实 exact identity 选择 Strong/ODR，不增加泛型专用发布入口或重复 wire 声明表。

M23-6a 已按完整 application 和实际需求移除旧 source-only gate。普通声明的字段、签名、父类型或接口包含封闭泛型 application 时，本阶段直接消费其完整具体化结果，生成实际 Strong 定义及其中属于 ODR 的泛型表示；自动根、构造器导出和 exact facts 不再逐例增加允许规则。core 异常类中的 `Option<String>` 与用户声明遵循相同规则。

字段 type site 以所属名义 exact type 和原 typed field ID 共同标识位置，允许同一泛型字段出现在多个 application 中。`/34` 的 tag 6、7 增加 field 3=所属 exact type；同一位置的类型唯一，原字段 owner 必须与 exact nominal 的源声明一致。外来泛型表示保留原 provider 字段与定义位置，实际字段用途属于本次物化。

generic host 的普通方法和 accessor 具有 exact owner；generic method 另有 own arguments，二者顺序和身份沿用语言 3.2。interface default、abstract override、`super<I>`、protected receiver、属性 setter 权限、constructor readiness 继续在前端按实际声明检查。

导入成员的候选先沿接收者的实际继承闭包取得声明所属的完整 application，再把宿主实参固定到 solver 的 owner 参数组；显式类型参数、`_` 与实参推断只绑定 callable 参数组。声明之间的最具体候选比较同样保留两组参数，按原 forwarding 约束处理，不使用本次调用已经推断的实参。请求内的成员 application 保存所属类型和方法自身实参，宿主实参从所属类型读取，不能把两组实参合并保存后依赖位置重新猜测；只有执行正文替换时才临时按“宿主在前、方法在后”生成替换列表。实例仍通过已有 typed template/application handle 和 `CallableApplicationKey` 进入同一具体化队列。

泛型成员的普通调用和强制 `super` 调用保留不同的调用方式。泛型宿主上的普通虚方法、接口方法及访问器复用具体方法的 direct/virtual/interface dispatch；方法自身有类型参数时保持 final/direct。类的虚表、类和值类型的接口表可引用外来普通定义，或当前消费方从模板物化的具体方法；抽象槽沿现有参数完整的抽象方法与 trap 路径处理。共有模板的再次发布保留这项选择，不能把普通动态调用重新导出成强制直接调用。

泛型扩展属性的 getter 与 setter 保留原 `PersistentPropertyAccessorId`，按 property binder 的声明顺序建立各自的 callable application。消费方使用 getter 的完整签名和现有约束求解器，仅从 receiver 静态类型选择逻辑属性并确定类型实参；该选择供读取、赋值、复合赋值和自增共用，setter 右值与 expected result 不参与重新推断。只写属性时不因借用 getter 签名而物化 getter 正文；实际使用的访问器进入原泛型正文队列，定义处绑定、独立可见性、effect 和 receiver 求值顺序保持。再次发布保存实际访问器声明及类型实参，不把扩展访问器改造成普通泛型函数或名义类型方法；此项复用已有 HIR interface 格式。

protected 访问区域使用普通或泛型声明的原 typed nominal owner；词法类和声明类的继承关系不依赖 machine exact type。显式 receiver 的静态类仍须是提供访问上下文的类或其子类，构造器、方法、属性 setter 与 protected override 共用原可见性和槽覆盖规则。泛型 application 的实参替换及不变性继续由类型检查负责，访问检查不制造擦除后的类型或额外访问证明。产物以原声明 visibility 和 owner 表达访问规则，`InheritanceSlotContractV1` 不再重复保存 domain field 5，槽根和目标的 owner field 2 也退役；完整 receiver application 提供原宿主和实际参数。reader 保留身份、签名和实现引用检查，不重放已完成的访问域语义。

泛型抽象成员只有共有声明，没有共享执行正文。消费方从已验证声明取得实际 owner、宿主 binder、完整 receiver/参数/结果、effect 和原 definition origin，建立与本地抽象方法相同的参数局部值，进入既有 abstract trap lowering；不能为通过正文导入而制造空的共有 body record。抽象 setter 的隐式参数使用原 accessor 声明位置，普通参数继续保留自己的源码位置。默认实现继续消费真实模板正文，接口继承和抽象 override 保留实际所选声明。同一具体方法同时被调用根和接口表引用时，MIR 复用一个函数与实例记录。导入 struct/enum 在存储和父接口完成后，沿与 class 相同的 source dispatch selection 建立完整接口实现；实际装箱和 adjust thunk 使用该完整 conformance，不从方法名重建选择。

接口声明的槽契约与实际机器表项分开表示：接口 record 保存必需的原 slot、槽位置和完成替换的完整签名，不能保存虚构的自身 itable。类和值类型的实际 itable 保留必需实现，并按接口槽契约校验身份、顺序与完整签名。槽契约只引用自身签名类型；只有实际目标产生 callable 依赖，根默认正文没有被调用或被更具体的抽象声明压制时不物化。普通接口与泛型 application 共用此结构；源码覆写选择仍来自 HIR，不在 MIR/meta 增加第二套选择算法。

共有 callable binding、dispatch 和 exact callable ABI 的目标统一为既有 Strong owner 或 typed ODR callable member，转换为同一 `CallableBodyKey`；不增加新的实体 ID 或泛型专用发布表。实际 application 的虚表与接口表保留原 slot identity、完成替换的 slot signature、真实目标及 receiver adjustment。LocalConcreteHir 保留替换后的直接接口与完整实现集合，接口自身保留直接父接口；MIR 类型记录表达直接继承，物理派发表保留完整实现。来源来自完成的 callable materialization，物理 ABI 与定义来自实际 MIR/LIR；reader 只检查这些既有记录间的必要关联。继承、接口默认实现和抽象 trap 继续使用原 lowering 角色，不能以空派发表代替非空 generic dispatch。

value/ref、GC-free、enum variant facts、ZST 和 `Option` niche 在 concrete 输出中必须完备。别名先展开为原 exact target，不产生新实例；不同 nominal arguments 即使 ABI 相同也保留不同 exact identity、TD 和 ODR member。

core 的 `Option` 语法使用既有 imported protocol 的 typed owner/variant/payload 角色，具体 application 与用户 enum 共用声明解析和具体化。外来泛型 variant 的源码构造共用普通候选约束求解，直接生成实际 variant 值，不为其补造模板函数正文。泛型类型名在 `E.V` 中表示声明命名空间；unit variant 与 `None` 的静态初值沿原常量 image 路径保留。验收同时覆盖限定／导入／contextual 构造、`T??`、安全调用顺序、Elvis／unwrap／cast、可变 Option 属性和下游本地 payload 类型。

### 4.3 终止性

沿用已有 generic callable/constructor 调用图及 SCC 规则，检查环上的完整参数替换为 identity；普通直接和互递归复用已登记实例，非递归边允许变换实参。外来模板保留执行该检查所需的原 typed call edges，消费方只补实际替换及新形成的关系，不发明递归深度或实例数量限额。

值布局和继承环保留既有局部检查；managed reference 字段只需要引用表示，不递归展开其 referent 的值布局或全部方法。MIR helper 也从实际操作请求，处理生成类型不自动请求该类型的全部 coroutine helper。

## 5. 定义归属与完整引用

### 5.1 复用现有 root

| 实体 | 归属 |
| --- | --- |
| 参数自由 source nominal/callable、普通 property/object storage | 原声明 Cone 的 Strong；必要支持定义可为 native hidden |
| fully specialized nominal application | `SpecializationKey::Nominal` |
| generic callable、宿主 application 上的 constructor/member/accessor | `SpecializationKey::Callable`，使用完整 application key |
| generic delegate storage 和 initialization | `SpecializationKey::DelegatedProperty` |
| tuple、function、raw/native pointer 的结构形状 | `SpecializationKey::StructuralType` |
| closure/coroutine frame 及其本体 helper | enclosing callable 或 initialization materialization |
| box、coroutine step/slot/start、derived equality | 既有 `ExactOwnerRoot` |
| static/dynamic function adapter 及 environment | 既有目标 `FunctionShape` 的 Structural root，source signature 仍属于 member key |
| dispatch/boxing adjust | implementor/payload 的既有 root；slot 和目标实现是 typed 依赖 |

`Unit` 派生相等可由多个 Cone 独立请求，使用其 exact type 的 `StructuralType` ODR 组；
普通 `Unit` 类型、布局与装箱继续使用原名义归属。该选择只改变派生相等 helper 的
materialization，不新增生成身份或运行时入口。

函数值适配的参数逆变与结果协变复用普通 subtype 转换，包括 `Ptr`／`FunPtr` 装箱、引用上行和嵌套函数型变；指针类型参数与原生函数签名保持不变。closure、动态类型检查及 bridge 调用是函数形状描述符的实际需求根，LIR 按完整 exact function identity 发射 Structural ODR 描述符及签名所需的类型引用。closure 的 parent 指向精确函数形状，vtable 第 0 槽固定为保留挂起性及参数个数的 Any 动态 invoke；参数解包、调用原 typed invoke 和结果装箱由普通 managed CFG 实现。目标 adapter 执行相反方向的转换，运行时按完整签名检查型变。TD 和 bridge 不得随消费者所用的检查目标集合变化；提供方无需预见下游类型。普通正文、默认参数和 generic delegate initializer 共用这条路径，泛型仍保持单态化。 函数签名及函数型变所需的 interface 直接父关系通过共有类型登记的必需 field 29 发布，使用 `Absent | Signature | Interface` 封闭表示；当前 cone-production/3、历史 strong-production/16 与 runtime metadata ABI 2 同步拒绝旧布局产物。泛型引用实参消除保守装箱时保留显式上行转换及操作数原始类型，捕获字段仍按其实际声明类型读取。 实际 closure、adapter 及动态检查的函数签名所引用的私有 source nominal 进入共有表示闭包，包括 tuple 或嵌套函数中的类型；这些用途由原 HIR 遍历一次收集，无关私有声明继续保持本地。

物理 producer 和 semantic owner 分开。普通 Strong 引用携带真实定义方；ODR 引用携带 group/member。reader 可以记录某个物理候选所在 provider，但该候选位置不进入 ODR identity 或 canonical relocation。

MIR 生成类型计划区分本 Cone 的普通定义与既有 ODR 生成类型，保留原 location、nominal、exact 和 group；LIR 不把后者降为 Cone-owned。实际 box、step/slot 进入原类型表示表，参数自由有限支持和按需 application/结构 helper 共用表示投影。payload 语义引用沿 tuple 展开到实际 nominal/application 叶子，function 和 pointer 采用已有引用或指针表示，不要求结构 payload 独立发布名义类型记录。装箱 adjust 及目标成员从真实 callable root 取得完整 Strong/ODR subject、签名和 GC effect，不从生成名称或第一次使用的 Cone 推断归属。reader 复用 MIR 已完成的生成角色和字段检查，将按需 helper 纳入表示清单，并核对 payload GC 与直接接口关系，不要求未使用的支持族成员。

多个 Cone 物化同一 exact application 或结构类型的 box、step/slot 时，MIR 类型与 dispatch 查询复用完整内容相同的 ODR 记录。记录构造时按实际 origin 与 payload exact type 确定 Strong／ODR 归属，后续索引复用该结果；参数自由名义类型及其有限 helper 的重复 Strong 定义、同一 identity 下内容不同的记录继续拒绝。此修复不新增 wire 字段或改变归属规则。

box payload 或 inline array element 所需的结构 value layout 与 scan 由 LIR 实际发射，归已有 `StructuralType` group；已在 MIR 出现的 group 沿用原记录，首次需要的 group 由 LIR 生产。共有布局使用原 tuple/qualified-pointer 表示与 value constituent，不要求嵌套结构元素逐个物化。含引用 box 的 inline scan 必须连接真实 payload layout，不能只保存一个没有定义的 scan identity。

装箱 adjust 的外来目标直接使用同次 selected callable 的完整物理签名，Direct/Interface 调用方式不承担声明签名查询。外来参数自由值的 box 与 adjust 由原 provider 提供，conformance 查询数据本身不增加消费方执行根；本地值和实际泛型 application 的装箱实现继续按需物化。

共有布局与 callable ABI 语义闭包保存 `(provider, target)`，允许同一 ODR definition plan 在不同 Cone 各有一份记录。指定 provider 的引用必须保留该定义位置，普通 Strong target 和同一 provider 内的重复仍拒绝；跨产物内容兼容性继续由共有 ODR member 合并入口检查，不在语义索引中先选 winner 或重算内容摘要。

生成 dispatch 导出时，从同次 LIR 槽的 Local/External 引用取得实际物理 provider，再以 `(provider, callable target)` 查询已有 ABI。读取时从产物已保存的 dispatch ABI 引用取得相同位置，并以 MIR 槽的实际 target 查询完整 ABI；查询后仍核对保存的正文引用与签名。本地槽使用当前 Cone 的记录，外部槽保留已经选定的依赖记录；其他 Cone 中存在同一 ODR member 的记录不构成调用歧义。既有槽签名、ABI 和物理引用检查保持，跨产物完整定义比较继续只由原 ODR member 合并入口承担。

descriptor 的父类型或接口已在当前 MIR 表示清单与 LIR instance layout 中定义时，直接使用当前 Cone 的 Local TD 引用。依赖中同时存在该 ODR descriptor 不改变这个已经确定的物理引用；未在当前产物定义的类型继续解析实际依赖记录。

### 5.2 每次物化的必要闭包

“按需发射”不意味着可以产生残缺记录：

- nominal/structural type 的实际 TD、layout、scan、diagnostic bytes、父类型和完整 dispatch 表相互对应；表中每个 target 必须解析到已有 Strong 或实际 ODR 定义。
- 实际 box/adjust、function adapter、coroutine helper 包含自己产生的 environment、字段、type/scan、callable、registration 和常量；引用其他 root 使用明确 typed edge。
- 每个 callable 覆盖实际 body 及其 LSDA、FDE/CIE、compact unwind、safepoint 和 LLVM stackmap。不能只比较 `__text`。
- 每个被取址的 specialization-owned string/constant/diagnostic atom 保持原 owner 与 stable structural path，不落入首次使用 Cone 的私有常量池。
- 所有外部引用都在当前产物或显式依赖闭包中有完整目标；错误 owner、缺失目标、错误 ABI 和 dangling relocation 均拒绝。

普通方法属于自己的 callable application，不因某个 consumer 多调用一个方法而改变 nominal TD 的定义。一个 group 中新增独立 helper 也不会改变已经存在 member 的定义摘要。

### 5.3 参数自由类型的有限支持

M23-6 已发布的 box、coroutine step/slot 继续由 source nominal 的定义 Cone 提供。M23-7 为可跨 Cone 请求的参数自由 source exact 补齐 `CoroutineStart<R>`、canonical ABI、callable registration 及其真实 `Continuation<R>`/`SuspendTask<R>` 依赖。continuation 派发直接引用实际具体化的 `resume`／`resumeWithException` 声明及其槽；旧 `ContinuationShell` 只重复保存这两个签名，并无独立执行正文，予以删除。generated callable 的旧 tag 10 退役，不再生成或复用。

这些依赖 application 只产生其表示、dispatch 和实际被调用的方法，不递归补齐所有新 application 的 start。对 nominal application 和结构类型，helper 按实际使用物化，并仍归既有 root；member 并集规则使不同使用集合可以正常链接。

缺失定义方的参数自由 helper 是产物错误，不能由消费方生成第二个 Strong 或伪造 Structural root。

reader 在共有 MIR 与实际 core 协议相接的边界核对有限 shape-support 根各自的 start，并检查 start 的 task/completion 参数及挂起 callable 的隐藏 continuation 引用实际 core 协议类型。其他签名、step、归属和 ABI 不变量复用 MIR 表已经完成的验证，不重新执行源码语义或布局检查。

core 定义处按协议成员名称和签名选择 typed declaration，不限制 `Continuation` 的两个方法的源码顺序。具体化、状态机和派发表均保留实际方法槽；调换声明顺序后重建 core 的真实产物必须仍能恢复值与异常。

协议实例由共同 HIR 具体化工作队列从实际 core 声明请求；LocalConcrete HIR 的完整协程协议集合不依赖 core 是本地定义还是外来产物。intrinsic 使用其真实签名与 kind，抽象协议成员使用既有成员实例。源挂起 callable 的语义签名与内部 continuation/step ABI 分开保留，外来调用仍由原 MIR 状态机处理。MIR 派发槽使用相同的内部签名；抽象接口槽也从实际结果类型的协议实例取得 continuation 类型，不要求额外生成可执行正文。有限支持根包含实际共有表示所需的私有声明，继续排除无关私有类型；协议自身新生成的 nominal application 只补齐直接表示与调用依赖。

## 6. MIR 与 LIR 的统一生产

MIR 输入仍只有完整 LocalConcrete HIR 与依赖 MIR 数据。它转置 exact type、实际 target、dispatch、constructor initializer 和 initialization application，并在原有 lowering 中生成 closure/coroutine/helper；不接收模板或补做泛型推导。

现有外来 callable、type/layout/descriptor 记录扩展为明确的定义引用：

```text
DefinitionRef<kind> = Strong { provider, kind_specific_subject }
                   | Odr { group, kind_specific_member }
```

这里只统一归属表达，不把 callable、type、layout、storage 的 ID 合并成通用整数。实际 typed 目标、logical signature、canonical/physical ABI、GC root plan 和符号仍由各自记录完整持有。当前产物内定义与依赖引用保持明确分支。

LIR 继续使用 M23-6 的完整 value storage、scan、TypeDescriptor、dispatch 和 ABI 算法。Unit/ZST 的逻辑参数仍存在，物理参数省略按同一 canonical contract；generic substitution 不成为另一套 ABI 分类器。含引用 value 的 by-value copy、indirect/sret、boxing temporary 和 moving roots 沿既有路径处理。

当前 `OdrFree*Foundation` 只能用于历史 Strong 输入的限制检查，不再包围正式 generic 输出。正式生产持有原 canonical foundation、完整定义、实际依赖和 registration；删除被替代且无生产调用的 Strong 专用装配分支，不引入 Pending/Verified/Authorized 等资格状态机。

MIR 的共有机器输入直接消费 `DependencyMirOutput` 所持的唯一 canonical foundation，不要求调用方再提供另一份 foundation。每个函数物化根保留既有 `CallableSignatureSubject` 的 Strong/ODR 分支；参数自由导出、entry 和 compiler protocol 查询仍选择各自的 Strong 目标，不因同模块还有 ODR 函数而失败。旧 core bootstrap bridge 的 bytes 合同不变，其 Strong callable 表只覆盖 canonical foundation 中的 Strong 子集；ODR 函数继续使用原完整 signature、application 和 member 表。历史 Strong 产物读取与写入仍在各自产物边界拒绝 ODR。

泛型正文与默认值复用同一控制流节点展开。`try`、顺序 `catch`、`finally` 和 `throw` 直接保留原定义处的完整 typed 结构、catch local selector 与求值位置，继续使用现有异常、清理和 GC lowering；消费方不重做名字查找，不另建泛型异常实现。

共有 LIR 输出保存实际 producer 与唯一 canonical foundation，物化定义图直接区分 Strong plan 和既有 ODR member plan。源 callable 的 group/member 由 MIR 的实际物化记录传入；LIR 不重复发布 HIR/MIR 已有的 member，而为其 callable/safepoint registration 建立属于同一 group 的新 member。每个物理定义保留自己的 kind-specific primary symbol，不能为取得 plan 而把 ODR body 改成 consumer Strong。callable 的 trap 字符串、运行时 scan 与 EH/stackmap 关联 atom 从该 body 的真实 plan 和稳定局部路径产生；定义边界 symbol 沿用所属 plan 的 linkage。历史 Strong section 的限制在读取或生产该 section 时检查，共有 LIR lowering 不借用 OdrFree 输出。 普通 callable bridge 只核对其实际导出项的 body、符号、Strong definition plan 与 primary atom；完整物理定义和关联 atom 的验证保留在共有产物边界，不为读取参数自由子集重建整张 Strong symbol 表。 从已完成读取验证的 foundation 构造共有输出时，直接保留已验证的 producer 和 canonical 数据，不再次遍历归属。

抽象 callable 的 trap 从结构化终止语句到 MIR `Trap` 终结符直接携带诊断文本，LIR 只生成 body 所属的既有 C 字符串 atom。消息不进入托管 String 常量池，不产生 immortal object 或登记；移除以普通 runtime call 和托管 String 参数表示同一 trap 的旧路径。多个 Cone 物化同一泛型抽象成员时，其常量沿 body 的 ODR plan 合并，runtime 的对象范围检查保持不变。

实际托管 String 常量复用 MIR 已保留的 `ImmortalObjectKey`，由原 callable materialization 取得既有 ODR group；常量对象与 immortal registration 分别使用 `ImmortalObject`、`RegistrationRecord` role，不建立另一套常量池或按文本合并身份。共有 physical content 表的 String 投影使用 tag 7，ABI 输入为对象 ID、String exact type、尺寸与对齐，LIR 输入再加入 UTF-8 内容；登记投影使用既有 runtime record kind 2，包含对象 ID、符号、尺寸、对齐及 String type-registration ID。登记的 ObjectDefinition 保留 group/member 和真实 typed relocation；其最终 ODR definition 使用既有 member 算法，以登记对象、对应 immortal object 的 ObjectDefinition 及登记 LIR 摘要为直接输入。对象读取结果及对象摘要在后续目录构造中复用，不能再次解析或重算。同 member 经共有合并后只登记一次，不同 application、定义位置或 callable 的同文字符串保持独立身份和地址。

物理依赖选择直接使用 MIR 的实际类型、callable、dispatch 与初始化单元引用，保留各自 provider 和 typed target；初始化选择只传入所需的 unit 引用，不要求先构造整个参数自由类型导出 section。完整依赖记录仍来自同一批已读取产物，不能用手工补造 descriptor 或省略实际引用来降低测试要求。

## 7. 对象发射与可复现性

扩展现有 `EmittedConeObjectSetV2` 所在的对象集合实现，并按其新用途整理命名。每个 `ObjectDefinitionPlan` 只绑定到一个实际 `SlibMemberId`；一个对象可以含多个 plan，一个 group 可以跨对象。目录逻辑 key 仍由排序后的真实 unit set 形成。

发射遵循以下约束：

1. 每个具有物理定义的 ODR member 只有既有 kind-specific primary；有 `cb`、`td`、`ss` 等 primary 的成员不另发 `od` alias。声明与定义使用相同 linkage、visibility 和 ABI。
2. Darwin/AArch64 使用 `weak_odr` 保留实际要求的定义，生成代码仍按 native linker 的逐 symbol 合并工作；不假设 Mach-O 有原子 COMDAT。
3. TD、static storage、init cell、registration 和取址常量保持地址意义；禁止用 `unnamed_addr`、common、constant merge 或 ICF 合并不同 identity。不同 ZST storage 保持各自 token。
4. image 和其六类 pointer table 由当前 Cone Strong 持有；table 可以引用共享的 ODR registration atom。table 本身不进入某个 ODR group。
5. 边界符号、关联 EH/stackmap、relocation、digest patch 沿用当前集合式发射与验证。函数对象只依赖其 canonical body 和完整声明，不能因同模块其他函数是否可见而得到不同优化结果。
6. member 顺序、capture/field 次序、内部 label 和 safepoint site 从已有 typed identity 与规范化 body 生成。consumer 的 arena 分配、输入顺序、绝对路径和首次使用点不得改变语义对象。

单成员 LLVM 模块中，只有实际定义的 ODR 函数和 global 使用 `weak_odr`；指向其它对象中该定义的声明使用 LLVM 必需的普通 external declaration，其语义 symbol request 仍为 `OdrWeak`，不能改成可缺失的弱引用。registration 的 runtime identity 写入实际 group/member，image 的 Strong pointer table 直接使用这些 registration 的既有符号请求。

机器对象 reader 从 external section definition 读取实际 Strong/weak 标志，再与已有 plan 逐项比较。局部 weak definition、weak undefined reference、缺失或未计划的定义仍被拒绝；所有真实 range、relocation 和 patch 继续使用同一对象索引。物理符号计划保留 foundation 已解析的实际 definition owner，ODR relocation 的逻辑目标为 member，物理 producer 只用于定位对象。 外来 shape 引用分类直接接收实际 consumer identity 与已有的完整 physical imports；生产端和 reader 使用同一入口，不为分类另建导出 section 或重新选择依赖。

同一 member 的 canonical LIR 和规范化对象在同一 target/backend 下必须一致。重排物理对象分片可以改变 Code/Artifact fingerprint，但不改变 ODR member identity 和 definition fingerprint。测试分别比较 canonical 内容与物理目录，不要求不同 Cone 的整个 `.slib` bytes 相同。

ODR 定义引用的空 span、空 scan 等非 null sentinel 也按所属 group 的稳定角色发射为实际 member 或对应定义的关联 atom；不能复用以消费 Cone 派生的私有 sentinel。只有 ABI 明确允许为空的字段才使用原有 null/zero 分支。

generated-C bridge 继续以 producer-independent unit 表达 recipe，实际对象引用当前 producer 的 bridge atom；既有 verifier 核对后将 relocation 规范化回 unit。此例外不扩展到任意 consumer-local helper 或 native symbol。

跨 Cone 首次取得参数自由 `@NoGC` 函数的 C ABI 地址必须闭合真实 storage bridge、trampoline 及它们对原函数的引用。M23-6a 的实际 CLI 对照中，本地 `FunPtr<(Int) -> Int> = ::increment` 和消费方普通 `increment(41)` 都可发布；提供方只定义 `increment` 时没有 C storage bridge，消费方直接取其地址尚不能发布。本阶段应补齐实际机器地址的发布、需求与引用关系，保留原源码函数和桥的 typed identity／ABI／归属；不得复制外来 Strong 函数体或以假源码 wrapper 代替这一关系。验收覆盖提供方已经取地址与消费方首次取地址、再导出与源码移走、默认／泛型正文使用，以及真实 C 调用该地址的返回值。

定义 Cone 从共有 callable 表选择参数自由、ordinary、NoGC、顶层 Scoop 函数，只有其参数与结果满足已有 C-safe 规则时才发布实际 storage bridge；私有模板支持与公开函数使用同一选择。桥沿既有 generated identity 保留原函数，MIR binding 同时保存源码签名和实际指针参数／Unit 结果签名。普通不满足 C-safe 的函数仍可发布，不为它们生成桥。源码与模板中的 FunctionAddress 共用已选 callable target，消费方引用定义方桥、生成本次 generated-C trampoline；提供方已经取址时复用同一桥，不因使用点不同追加 Strong 定义、身份或来源记录。

## 8. ODR 摘要、检查与合并

### 8.1 逐 member 记录

manifest 的 ODR 目录按 group、member 的 bytes 排序，只记录各组本次实际发射的物理定义。每个成员包含原 role、ABI 摘要和完整 definition 摘要；identity key、canonical LIR 和物理 materialization 继续保存于其既有所属层，目录不复制另一份正文。

```text
OdrAbiFingerprint = DomainSeparatedCborHash(
    "scoop-odr-member-abi-v1",
    { 1: group, 2: member, 3: role, 4: canonical_member_abi })

OdrDefinitionFingerprint = DomainSeparatedCborHash(
    "scoop-odr-member-definition-v1",
    { 1: group, 2: member, 3: role,
      4: canonical_lir_leaves_by_atom,
      5: definition_input_leaves_by_digest_node,
      6: stackmap_leaves_by_site })
```

三个 leaf 集合都是必需的 canonical array。实际物理 member 的 canonical LIR 与 object leaves 非空；没有 safepoint 时 stackmap array 可以为空。ABI payload 按 role 保存实际函数签名、布局、存储或 registration 合同。

field 5 通常保存实际 ObjectDefinition leaves；static-storage registration 同时保存该登记所需的 layout 和 scan leaves。每项仍为 `{1=DigestNodeId, 2=digest}`，节点的种类及用途来自已有 digest graph，按节点 ID 排序。ODR 节点因此允许直接引用 Layout/Scan；对象节点继续表示其自身实际字节及 relocation。初始化登记汇总 registration、cell 和 descriptor 的对象 leaves。此扩展只服务已有静态存储登记的实际字段，不递归汇总普通 callee 或其他名义类型的定义。

纯语义的 `GeneratedNominal` identity/member record 继续保存于对应 IR foundation，完整生成类型保存在 MIR/LIR metadata；它本身不进入 manifest 的物理定义目录，不生成 ABI 摘要、空 `od` marker、ObjectDefinitionPlan 或虚构 range。其实际 TD/layout/scan/callable 各自作为物理 member 检查。source/key 存在与实际机器定义分开，缺少真正需要的物理成员仍是错误。

不再生产旧 `scoop-odr-abi-v1`、`scoop-odr-definition-v1` 的整组摘要，也不另加 group union hash。Code/Artifact fingerprint 已覆盖整个产物目录；同一 group 的成员是否兼容由成员比较和引用检查决定。

### 8.2 复用现有 digest 计算

最终 LIR 的 `StructDef` 与 `EnumDef` 必需保存实际 `PersistentExactTypeId`，lowering 从 MIR 已解析的 exact-type record 直接传入；完整 key 仍由同一份 `LirMeta.exact_types` 保存。session-local 的 `StructDefId` / `EnumDefId` 只用于访问当前表示表，canonical LIR、ABI 和全局引用使用记录中的持久身份。名称、arena 排列、相同布局或字段集合不能替代该身份；表示、扫描和 C ABI refinement 的后续填写必须保留它。泛型实例使用其完整 application 身份，生成 enum 使用已有 generated exact-type record。

函数正文的 canonical LIR leaf 使用 `scoop-lir-definition-v1`，从最终 `Function` 的实际 ABI、GC effect、指令、正常/异常控制流与 root plan 计算。块按入口可达 CFG 的固定后继顺序规范化，local/temp 按正文首次出现顺序编号；未使用的 arena 槽、诊断名称和不可达块不进入这个语义投影，实际发射的所有对象 atom 仍由 ObjectDefinition 覆盖。类型、函数、存储、桥和 safepoint 使用已有 typed persistent identity，call target/signature/root-scan 等函数局部表在使用位置编码实际值；不编码 arena ID、dump、LLVM 文本或最终摘要补丁。

代码生成按实际发射的块和指令顺序，以局部值首次使用或定义的次序分配函数栈槽；同一指令先读取操作数，再处理结果定义。该顺序复用既有 use/def 遍历，未引用的局部槽最后处理，不依赖源码或导入模板的 arena 编号。局部值重新编号不能改变同一 ODR 正文的栈偏移及对象摘要；这项代码生成修正不改变 LIR 编码、摘要格式或 runtime ABI。

对象摘要中的 relocation 按已解析目标的实际实体身份与 definition role 编码。Strong callable、layout、scan、类型描述符、派发表、类型登记、静态存储及其登记、初始化 cell 与 descriptor 统一使用既有 runtime target tag 1；ODR callable 和 shape 使用既有 tag 14 及原 member。同一已解析目标在定义 Cone 和消费 Cone 必须得到相同字节，泛型正文对普通 object 静态根的引用不能因再次发布而改变对象摘要。实体、role 和符号的对应关系复用共有 target 查询，provider 与声明目标仍保留在普通依赖记录中，承担既有引用、ABI 和符号检查。原普通依赖 callable tag 11 与 shape tag 12 均退役且不复用，原服务 tag 2、13 保持退役。完整 shape 引用正规化将 link-identity-closure 从 /6 升至 /7，旧产物和缓存重建；runtime C ABI、persistent identity 与 ODR 合并规则不变。

production section 新增必需 field 13，保存按 callable-body ID 排序的 records：Strong 使用 `{1=body, 2=canonical LIR fingerprint}`，ODR 必需追加 `3=OdrAbiFingerprint`。此排序独立于 foundation 为解析引用使用的拓扑顺序，并精确覆盖全部 callable-body；其中普通 Strong/ODR 正文与 lowering 已生成的 root/init gateway 均按实际 `Function` 编码，不用空记录或入口计划替代 gateway 正文。body 的实际 Strong/ODR 分类及 ODR group/member/role 直接来自 foundation，wire 不复制身份字段；reader 拒绝 Strong 携带 ODR ABI 或 ODR 缺少 ABI。producer 从同一最终 LIR 计算一次，reader 检查集合、引用、排序和 fingerprint 格式后复用；物理对象仍独立按实际内容验证。当前两种 Strong reference schema 的 production capability 分别由 `/11`、`/12` 升至 `/13`、`/14`，旧产物重建，ODR 发布限制保持；ODR 必需分支随完整泛型 profile 发布，当前格式为下述 cone-production/3，不改变 Strong callable record 编码。

callable-body member 的 ABI payload 固定为 `{1=GC effect, 2=ScoopAbiSignature}`，与 canonical Function 共用 GC effect、调用约定、参数和返回值的 direct/indirect/ZST、物理类型、布局及 scan 编码。ABI 计算只读取这些完整表示，不遍历函数正文或 CFG；group/member/role 使用该 body 的实际 ODR member key，不能将 adapter 等函数角色统一改为 CallableBody。已有 refined `CallableOdrMemberId` 在构造/解析时保留该 key 的 group 和 role，wire 仍只编码原 member ID；body key 因此可直接复用已解析关系，不要求上游 member key 在 LIR 本地表再次发布，也不再查找或解码原 key。经验证的 canonical callable record 用完整 Strong/ODR sum 保存分类，ODR 分支必需包含 group/member/role 和 ABI 摘要，不能在后续 stage 补猜。

callable-body member 的 definition 使用该正文 primary atom 的唯一 canonical LIR leaf、body ObjectDefinition 节点的唯一 object leaf，以及按 site ID 排序的全部所属 normalized-stackmap leaves；没有 safepoint 时最后一项为空。ObjectDefinition 已覆盖该函数全部关联 atom，汇总不再次解析 EH 或 stackmap。生产与产物读取共用这一计算入口，并复用 production field 13；普通 callee、TD 等引用继续只保存 typed identity，不递归纳入目标定义摘要。Strong body 仍使用其既有 ObjectDefinition；callable registration 的 `body_definition_fingerprint` 对两种 owner 均保留该 ObjectDefinition，不改写为 ODR member definition。

实际启用泛型名义类型的物理生产时，production 增加必需 field 14，保存当前发射的 ODR layout、scan、TD 和 dispatch table 的 canonical 内容摘要，格式为按 member ID 严格递增的 `{1=member, 2=canonical LIR fingerprint, 3=ABI fingerprint}`。group、role、primary atom 与定义计划直接复用 foundation 的已解析记录，不在 wire 重复；该表精确覆盖上述四类实际 ODR 定义。完整泛型格式由 `cone-production/1` 升至 `/2`，同一生产结构的历史 Strong 格式由 `/13` 升至 `/15`；历史 Strong 表为空且继续拒绝 ODR，不增加发布路径。

名义类型 application 的 group 记录由 HIR foundation 从已有 concrete exact type 表发布，与 callable application group 合并去重；后续 stage 沿已有 typed 引用使用它，不在每层重复发布。

shape 的 canonical LIR 从最终 LIR metadata 计算一次：固定 layout 保存 size/alignment、有序 field offset/access alignment、CLayout、内部可变性和实际表示；变长 array 保存元素 exact type、物理类型、完整 instance shape 与最大长度；boxed/abstract instance layout 使用其已有完整 instance shape，不伪造固定字段表。scan 保存实际递归程序；TD 保存 runtime type、instance layout/shape、inline scan、parent、dispatch identity 和诊断字节；dispatch 保存按 ABI 顺序排列的真实 callable 目标。arena index、消费 Cone、显示用 layout 名称和首次使用位置不参与摘要，local/external 引用均归一为同一 typed target。shape ABI 使用相同物理合同，排除 TD 诊断文本；它不能仅散列 member key、符号或大小而遗漏字段偏移。reader 检查表的集合、排序和引用后复用摘要，不重做源码语义或布局。

四类 shape 的 ObjectDefinition 使用现有 relocation 归一化与实际 atom 范围，包含 descriptor 的诊断和 itable directory 等关联 atom。其 ODR definition 各汇总自身一个 LIR leaf、一个 object leaf，stackmap 为空。类型注册先取得 TD ObjectDefinition 与 layout 摘要，再写入其规范化对象内容并计算注册自身的 ODR member；仅自身最终 definition 槽归零。Strong 类型注册保留原算法。最终摘要目录和 image 使用实际 Strong/ODR 分支，普通 TD/dispatch 引用不递归散列目标成员，合法递归类型不会造成摘要环。

类型摘要在生产与读取共用的一个入口内按上述依赖次序计算，保留实际验证过的 registration 与最终 leaves；删除仅为跨函数传递而建立的 registration-object/dependency 包装链。相同对象 bytes 与已解析引用在此计算中只读取核对一次，不在每个内部计算步骤重新验证完整对象。

保留 `DigestKind::OdrDefinition = 8`。旧 owner variant `OdrDefinition(OdrGroupId) = 8` 退役，新增 `OdrMemberDefinition(OdrMemberId) = 11`；后者映射到 kind 8。其他 owner tag 与 patch field role 不变，不复用退役编号。

每个 ODR node 只汇总该 member 的 LIR、object 和 stackmap leaves。registration 的 `definition_fingerprint` 来自该 registration member；callable 的 `body_definition_fingerprint`、TD 的 descriptor 字段继续来自对应 ObjectDefinition。计算 registration 的对象 leaf 时，其自身最终 ODR slot 归零；已有 body/layout/scan 等上游字段保留。

callable 与 safepoint registration 的 canonical LIR 直接投影已有实际 production plan：callable 使用 `{0=6, 1=body, 2=entry symbol request}`，safepoint 使用 `{0=5, 1=site, 2=runtime safepoint id, 3=owner body, 4=site role, 5=root-pair count}`，在 `scoop-lir-definition-v1` 域计算。这里不编码物理 member、patch intent、最终摘要或 producer。注册 ABI payload 为 `{0=record kind, 1=实际 ABI version, 2=实际 record byte size}`；kind 沿用 runtime 的 callable=6、safepoint=5，当前 version=2、size 分别为192和232。它描述注册记录自身的格式，函数调用签名另由 callable-body member 的 ABI 覆盖。

逐 member definition 的 LIR leaf 编码为 `{1=atom, 2=fingerprint}`，object leaf 为 `{1=digest node, 2=fingerprint}`，stackmap leaf 为 `{1=site, 2=fingerprint}`。callable registration 各有一个自身 primary 的 LIR/object leaf，stackmap array 为空；safepoint registration 同样各有一个自身 LIR/object leaf，并含该 site 唯一的已规范化 stackmap。ODR safepoint 对象摘要须先代入其实际上游 normalized-stackmap 字段，并记录该直接输入，再计算自身 member definition；不能沿用 Strong 的全零 provisional record 对象摘要。callable registration 对象已包含实际上游 body-definition，最终 ODR 汇总不再次把 body node 当作自身 object leaf。

共有注册摘要结果区分 `StrongRegistration` 和 `OdrDefinition`，补丁写入与 RuntimeImage 编码使用实际 owner 及对应 digest kind。image 表中的注册指针按期望的 typed entity/registration role 核对已解析目标的实际 primary definition、symbol 与 owner，不重新派生 Strong-only definition；其余 relocation 宽度、形式、零 addend 和所属范围检查保持。manifest `/2` 的 field 5 只投影六类注册中的 Strong 子集，ODR registration 与 callable-body 等物理定义统一进入必需 field 11。field 4 继续包含全部实际注册 identity；ODR 摘要不能写入 Strong 字段，也不能从完整成员目录中遗漏。Strong registration 自身的编码和摘要算法保持；manifest 与 profile 的格式变化进入 Code/Artifact fingerprint，RuntimeImage 仍由实际注册、runtime ABI 与 target 投影计算。

共有生产查询按 typed subject 与物理角色取得 foundation 中的实际 Strong/ODR plan。源 callable 的 subject 使用已解析的 callable-body key，其他物理 member 使用原 ODR key，不重复发布上游 member 或重新解码其 key。ODR registration 的 ObjectDefinition 节点依赖实际写入该记录的上游字段；其 ODR node 只汇总自身 LIR、对象和所属 stackmap leaves。对象间的摘要边仅表达这些实际补丁依赖，image 继续汇总六类实际 registration node。

函数的 ObjectDefinition 规范化 primary text 及全部实际关联 atom：runtime scan、取址常量、LSDA、EH frame、compact unwind 与 LLVM stackmap。关联条目按 atom ID 排序；内部 label 引用转换为所属 atom 与相对 offset，外部引用使用既有 typed target，不能把 section 起点或物理 member 写入摘要。已有 stackmap record 的规范化结果继续作为函数的直接输入，不重做解析。 Mach-O compact unwind 与关联数据中的本地 UNSIGNED 指针按实际 section ordinal 和对象内目标地址定位所属 atom，编码 atom 相对 offset 后清除物理地址；其余真实符号引用继续保留原 relocation 形式与语义 addend。目标不在该定义的实际 atom 范围内时拒绝。

函数正文摘要先于 callable registration 对象摘要计算。ODR registration 的对象 payload 保留真实 group/member，将 body-definition 字段代入刚计算的正文摘要，只把自身最终 definition 槽归零；对应直接输入为该 body ObjectDefinition。入口使用已验证的真实 relocation，不能根据 body ID 重建 Strong 引用。Strong registration 保留原有置零字段和独立正文依赖合同。此前只接通 ODR 对象读取的 verifier `/2` 退役，完整关联对象摘要使用 `/3`，旧对象和缓存重建；runtime C ABI 及摘要字段宽度保持。

普通调用和 TD 关系只在 canonical relocation 中保存 typed 目标，不能加入目标 ODR digest 的递归依赖。跨 member 的真实关系在对象/引用边界检查，正常互递归不会形成 digest 环。每个实际 patch 仍有唯一写入者、精确 atom/range/width，初始为零，计算完成后一次回填。

### 8.3 合并算法与错误边界

共有 reader 完成每份 artifact 的格式、identity、ABI、对象和实际引用检查后，合并器仅做新增工作：

1. 按 `(group, member)` 建立定义索引；相同 key 的完整 ABI、definition 与 canonical key 必须一致。
2. 将兼容定义加入物理候选列表，独立 member 加入并集；不先选 winner 再忽略其他定义。
3. 对实际 Strong/ODR 引用核对目标存在、kind、signature/layout、linkage 和 provider 范围；拒绝相同 symbol 被不同 owner 认领。
4. 检查 type、callable 和 initialization 所需的关系完整，返回普通完整定义和候选数据，供当前验收入口及后续 linker 使用。

不同 definition 是产物/链接错误，报告两个 Cone、group/member、角色以及首个不同的 ABI/LIR/object/stackmap 部分。它不是源码 bound 错误，也不允许交给 native linker 任意选择。

这一步接在共有 closure 的各产物 symbol/object 读取之后，覆盖没有彼此直接依赖的 sibling。完整 canonical key 借用已有 identity graph，物理候选直接关联已验证 primary definition 的 Cone、`SlibMemberId` 和 symbol。结果随共有 closure 保留，不另建发布器或 wire 表；诊断只读取现有内容叶子，不重算已验证摘要。

## 9. 泛型委托扩展属性

### 9.1 source template 与 application

`val/var <T> Receiver<T>.p: U by expression` 的全部 binder 必须只由 receiver 的 exact 静态类型和 bound 唯一确定。result expected type、setter RHS、runtime receiver class 和使用 Cone 不参与推导或 specialization key。

消费方可以为依赖中声明的泛型名义类型定义自己的扩展属性。receiver 的 binder 遍历同样覆盖导入的 class、struct、enum、interface 及其嵌套实参，不要求把外来声明复制为本地类型。

源级 `by` 没有某次访问的 receiver 值，不能使用该值的 `this`、字段或 identity。它可以使用已经声明的类型参数及定义处可见实体；先求值 `by`，再可选调用一次无 receiver-argument 的 `provideDelegate`，保存 effective delegate。get/set role 按已有协议接收实际 `thisRef`，保持 ordinary、non-suspend、non-generic、无 default/vararg。

依赖声明的 delegate 方法和扩展通过普通候选、typed role、可见性、参数替换和 winner commit 解析。同一成员层中的本地与外来方法一起做最具体候选比较；扩展在各自作用域层内采用相同规则，import alias 保持原 typed role。生成访问器和 initializer 直接保存已有的 imported call；局部委托复用已选 callable、实际参数类型和 effect，在每个使用点生成调用。内部已经降低的 receiver/参数保持原求值顺序，不再为 required-only 协议生成默认参数或多余参数临时变量。

重载平局中的非泛型优先只考察 callable 自己的 binder；泛型名义宿主中的普通方法不会因实现需要单态化而变成泛型方法。泛型构造器仍按所属名义声明的类型参数参与已有构造器比较。

source HIR 增加独立 generic delegate template，不能把它先降成 `ManagedGlobal`。具体化后使用本地 typed `GenericDelegateStorageSpecializationId` 关联完整 unit 和 storage；持久身份直接复用已有 `InitializationUnitKey::GenericDelegatedExtensionApplication { property, receiver_arguments }` 及 storage role，不建立第二套 persistent specialization key。

模板保存所属 property、effective delegate 的符号化类型和声明级 initialization unit。源级 ensure、storage read/write 使用明确的 template 引用及按 property binder 顺序排列的非空类型实参；这些节点只参与普通的类型替换，不查询运行时接收者。具体化以 property 与完整实参组复用同一 specialization，再把读写变为该 specialization 的普通全局存储操作。initializer 与 ensure 仍由 unit 关联各自的 generated callable，源码模板本身不成为参数自由初始化根；非泛型 unit 的现有根选择保持。

委托存储节点由访问器和初始化模板产生。普通源码及默认值对属性的访问继续使用既有 accessor 调用；独立 default template 不能直接读写委托存储或发起其内部 ensure。

委托模板通过 6a 的共同 HIR 查询按原 property／template 身份读取，源码与解码使用同一记录，不增加来源专用的 consumer-local 模板实体。initializer 直接走共同正文替换，ensure 以初始化协议实现的明确种类表示，不伪装成缺失的源码正文。具体化后的 storage specialization 使用独立具体实体 ID；application unit 仍以原 property 与完整 exact arguments 唯一确定。

receiver arguments 按 property binder 声明序排列；透明 alias、re-export、getter/setter 使用方式及不同 receiver 对象都不改变 key。不同 arguments 各有独立 delegate 与失败状态。

initializer/ensure 的 `GeneratedCallableKey::Initialization` 仍只引用声明级 extension-property unit 与各自 role；application unit 只进入 `CallableMaterializationContext::InitializationApplication`。其中的 local generic function 使用既有 `EnclosingInitializationApplication`。不能把具体 receiver arguments 写回 source template identity，也不能让 getter、setter、initializer 和 ensure 共用 callable ID。

MIR type bridge 的 source initialization 表继续表示可从 provider 直接引用的参数自由 Strong 单元；泛型 application 的实际 unit、两个 callable body 和存储由本次 MIR materialization 与 LIR registration 保留，不进入该 source 表。产物 reader 使用同一完整 identity graph 核对 application unit、declaration role 与实际 ODR 成员的关系，不把 application 伪装成 provider 的参数自由初始化服务。

HIR identity delta 保存实际 application unit 与 delegated-property group；application 的源码位置沿原 property 声明查找，不另造实例声明。MIR 保存 initialization application 的实际 generated callable 成员。初始化正文中的类型位置允许该 application context，生成函数的参数与结果仍由既有 signature join 检查。

### 9.2 固定的 unit 闭包

每次物化一个 delegate specialization 都必须产生以下成员或其明确的同一 specialization 定义引用：

- effective delegate storage 及 static-storage registration；即使 delegate GC-free 或 ZST 也保留真实 storage/token。
- GC-free initialization cell，初始为 Uninitialized；managed failure storage 及 root registration，初始为 null。
- initializer、ensure、二者的 callable registration，以及实际 EH、safepoint 和 stackmap。
- `LazyAccess` initialization descriptor/registration、稳定 diagnostic path 和全部 typed 引用。
- 本次 initializer/ensure 自身产生的 closure、常量、类型/scan 等必要支持；其他 nominal/callable root 以 typed edge 引用。

getter/setter 继续使用各自 callable application，二者引用同一 delegated-property group。只读取 `var` 不要求预生成未使用的 setter；unit 的固定状态成员仍不能少。Lazy unit 不生成 startup gateway，C 中相关 id/digest/pointer 保持规范的全零分支。

initializer 直接访问外来 object/property 时，以本次实际 application unit 保存初始化依赖。该 local unit 的声明可来自依赖 Cone；存在性由实际 MIR root 和已验证的 LIR 登记确认。它引用原参数自由 Strong 初始化服务，自身不进入外部服务目录，也不要求提供方预先物化该 application。

production 的 canonical shape 表同时覆盖实际 ODR storage、initialization cell 和 descriptor。storage 的内容由已计算的存储语义投影取得，排除使用 Cone 的 layout-provider 路由信息；cell 保存真实零初始状态和物理 ABI，descriptor 保存 schedule、unit、typed 指针目标及稳定诊断字节。登记摘要复用已经验证的对象 leaves，不再次解析同一存储或初始化对象。layout/scan 自身沿 exact type 的归属发射，与持有这些值的委托 application 分开。

### 9.3 求值、失败与 GC

普通读取先求值 receiver，再进入 getter ensure，成功后读取 delegate 并调用 get role。普通赋值先求值 receiver、再求值 RHS，进入 setter 后 ensure，随后调用 set role。复合赋值按 receiver → getter/ensure → RHS → operator → setter 的原有顺序，receiver 只求值一次；prefix/postfix 更新同样复用已有展开。

ensure 复用 M21 的 RunInitializer/Ready/Failed/Cycle 路径。winner 在线程的 managed roots 中完成 `by` 与 `provideDelegate`，成功后写入已登记 storage 并发布 Initialized；失败写入相同 specialization 的 failure root 并发布 Failed，后续访问重新抛出同一失败且不重试。并发等待、同线程重入、跨线程 cycle 与异常传播使用现有 coordinator。

同一失败指 failure root 中缓存的 managed 对象；普通 throw/catch 仍按 runtime 规范 5.2～5.4 复制异常 payload 并物化每次 catch 的绑定。不同访问的 catch 对象不要求 `===`。运行验收通过初始化次数、异常中引用字段的共享身份与内容，以及实际 failure storage 的合并确认失败共享，不能以改变异常复制语义满足测试。

delegate/failure 中的 managed reference 由普通 root 和 scan 更新，中间值遵守 moving-GC 规则；不 pin、不放入 GC-free cell，也不在 ensure 前缓存待移动的 delegate 地址。非泛型 extension delegate 保持原来的 eager 语义。

## 10. wire、profile 与缓存迁移

以下表格记录 M23-6a 已验收的 HIR 版本基线及本阶段物理产物格式。Stage 7 直接继承共同 HIR 的实际 section/inventory，不再另设 imported 模板格式。现有正式生产 profile 为 `org.scoop-lang.slib-profile/cross-cone-generic/1`。它沿用原完整 layout 产物的同一 producer/reader，替换 `cross-cone-layout-strong/3`；descriptor 仍只有 id 与当前实际必需 section 清单。

修订前第一条泛型函数纵向主线启用该 profile 时，使用已完成的 manifest `/2`、HIR interface `/32`、HIR type semantics `/8`、MIR type bridge `/1`、LIR layout ABI `/3`、callable link closure `/1`、layout link closure `/2`、link identity closure `/4`，并将原 Strong production `/14` 替换为 `cone-production/1`、Scoop 对象 verifier 升至 `/3`。这些编号记录既有迁移；6a 完成后按共同 HIR 和本阶段新增物理 payload 更新实际 section、descriptor fingerprint 与缓存；不为尚未生产的数据预写空表、引入临时 profile 或保留 layout-strong 双轨。profile 的启用不代替第 12、14 节的完整阶段验收。

`cone-production` 与历史 `strong-production` 是同一 production section 的替代格式，同一产物不能同时携带二者；即使旧项标为 optional 也按冲突格式拒绝。旧 layout-strong profile ID 直接退役，不通过新 profile 的名称或版本解析。

共有 HIR/MIR 的查询、native ABI、源码位置、类型表示及产物装配直接借用原 canonical foundation；reader 保存并共享本次已经验证的数据，不要求泛型产物经过 `OdrFree` 包装。历史 Strong 输入的限制仅在其自身格式边界检查。普通参数自由导出仍是完整 foundation 的对应子集；同一产物包含泛型 application 不改变其源声明、Strong ABI、字段布局或实际 provider。

实例化局部值的 definition origin 继续指向原模板源码。foundation reader 按 typed owner 从本地产物或实际可达依赖的已验证 canonical foundation 查找声明锚点；不得把外来声明复制为本地记录，也不重复验证未变化的 provider。锚点缺失、来源不符及源码位置范围检查沿用共有规则，不新增 wire 字段。

参数自由 callable 的签名可以使用已确定全部实参的名义 application；例如 `IllegalStateException` 构造器的 `Option<String>` 参数不要求调用者再做类型推断。共有 exact 签名分类递归处理这些实参并查询实际声明目录，保留 callable 的 Strong 归属；未知来源和未替换 binder 仍不能形成完整签名。

泛型函数、构造器和属性访问器的声明类型位置使用实际 application；核对其原声明与 root 一致，再按原签名检查 receiver、参数下标和 result 位置。泛型字段保留原名义 owner，class 初始化结果继续核对原构造声明、class/object owner 与定义位置；这两种位置均接受 generic 名义声明。局部值保留当前 materialization 与原模板源码位置，完全替换后的类型关系由已有 application、签名及 MIR/LIR 对接检查。

HIR→MIR 的调用对接按每个 call site 的真实 application 查消费方已有的 ODR callable-body 签名；普通直接调用继续关联真实外部 Strong 定义。完整逻辑参数、receiver、result 及当前 root 的存在性逐次核对，不向 provider 的 Strong 表索取消费方实例，也不新增实例或签名 wire 表。

调用签名的外部类型用途按 exact type 的真实声明归属收集；消费方本地类型可作为泛型实参；实际泛型表示或共有成员 ABI 所需的私有类型进入原共有支持声明表，其余普通函数体中的私有类型保持本地。复合签名中的外来类型、当前共有类型的表示闭包及实际字段和局部值中的外部类型依赖继续完整保留。

| section/capability | 既有基线／物理目标 | 变化 |
| --- | --- | --- |
| `org.scoop-lang.manifest/single-cone-production` | `/2` | 保留单 Cone 产物含义，完整 Strong/ODR materialization 与新增必需 ODR member 目录 |
| `org.scoop-lang.hir/cross-cone-interface` | `/43` | 保留名义实例化条件；Literal field 1 使用完整 typed 表达式，保留原定义与求值位置；捕获 field 4 分离读取值来源与原绑定；for 在 Export 前展开，撤销专用 For 与 portable binding-plan 编码；公开绑定引用不要求终点 provider 是 direct；AliasTarget 只保存实际 typed 引用；原 field 1～10 保持；必需 field 11、12、13 分别承载 callable body、constructor initialization 与 delegate template；实际调用记录保存 application，共享表达式保存原求值位置，bound receiver 保存完整类型 key |
| `org.scoop-lang.hir/cross-cone-type-semantics` | `/11` | exact application 的完整 facts、继承和 actual type uses；退役重复 slot domain field 5 与槽根／目标 owner field 2，以原声明和完整 receiver 查询泛型父类型 |
| `org.scoop-lang.mir/cross-cone-type-bridge` | `/6` | 协程启动 helper 使用 role tag 13，保留真实 task/continuation 参数及完整 ABI；挂起声明保存源码与内部 continuation/step 两个签名；参数自由 NoGC callback storage 使用 role tag 12，语义签名保留源函数、物理签名为结果及参数的 Ptr 序列并返回 Unit； 原类型表示表保存 application origin；callable 和实际 dispatch 使用 Strong/ODR 目标；槽种类 tag 3 保存 interface 的完整签名契约，与具有必需实现的物理表项分开 |
| `org.scoop-lang.lir/identity-foundation` | `/2` | 新的 member digest owner；拒绝旧 group owner tag 8 |
| `org.scoop-lang.lir/cross-cone-layout-abi` | `/5` | 布局、descriptor、dispatch 和 callable 的 Strong/ODR 定义引用；完整 callable ABI 保留实际 callable member |
| `org.scoop-lang.lir/cross-cone-link-closure` | `/2` | 普通 callable requirement 扩展到实际 ODR target |
| `org.scoop-lang.lir/cross-cone-layout-link-closure` | `/3` | layout/descriptor/helper/callable 的实际 Strong/ODR 物理引用 |
| `org.scoop-lang.lir/link-identity-closure` | `/7` | ODR definition、symbol、relocation 与 member-aware materialization；本地和依赖 relocation 使用相同的实体与 definition role |
| `org.scoop-lang.lir/cone-production` | `/3` | 取代完整 layout 路径的 Strong production `/14`，统一表示完整 Strong/ODR 定义、六类 registration、image、digest plan 与实际 shape 内容摘要 |
| `org.scoop-lang.link-object/scoop-lir` | `/3` | 同一 Mach-O verifier 支持并核对实际 ODR 对象与 member 摘要 |

本次 HIR 统一复用现有 typed identity、callable application、Strong/ODR 和 mangler，不因重构内存模型而任意改写身份。HIR identity-foundation `/3`、MIR identity-foundation `/1`、现有 compiler protocol、参数自由调用桥、generated-C verifier 及三层 outer schema 是既有基线；实际 bytes 未变时保持，发生变化时由 6a／本阶段按对应 section/schema 明确升级。LIR foundation `/2` 已用于本阶段，M24 的后续升代仍按其设计执行；旧编号不能强迫保留两套语义结构。

### 10.1 共同 HIR 的格式交接

声明、默认值、callable body、构造初始化与委托记录均继承 6a 的共同语义结构和编解码。Stage 7 只补齐实际定义／运行需要的数据，不能为了保留旧 `Default*` transport 或 imported template 而增加第二套正文 reader。根记录以原 typed owner 关联；共享正文保存完整 locals、parameters、statements/results、effects、binder、必要 predicates、定义／求值位置和 captures，局部与捕获 selector 各在其真实正文范围内解析。

构造初始化仍按原 nominal 保存一份 common sequence 和各 constructor 的 primary、secondary-this、terminal-super 等实际执行关系。委托实参、字段初值、init 与次构造正文使用共同执行片段；语句片段没有结果时不填充假的 Unit。源码声明、字段和构造器的 shape、签名、参数协议与 bounds 直接引用共同表，不复制第二份来源数据。

泛型 delegate 根保存原 extension property、effective delegate type、完整 initializer 正文和诊断位置；get/set、provide 与存储引用沿第 9 章实际 typed 关系关联。initializer 使用原 generated callable、与 property 对应的 binder 和共同正文。ensure 由既有 initialization unit 协议生成，不能在 wire 中伪造空 Scoop body。存储、ensure 与初始化写入使用同一 property application。

call site 保留原直接目标或完整 callable application，以及实际 receiver、参数和 source/evaluation origin；application 引用原声明及两组完整实参，不复制另一份签名。generic 目标不能伪装成参数自由直接目标。模板中的支持引用按原 provider／typed declaration 连接，不携带消费方源码 lookup 路径；默认展开和实例化只替换定义处已经选定的目标。

旧 HIR `/31`–`/40` 曾逐步加入正文、application、构造、字段类型位置、求值位置、委托和 bound receiver，并移除别名／静态 namespace 的重复来源检查及专用 For/binding-plan。`/41` 增加默认值捕获的原 callable、作用域与局部值定义，跨 Cone 展开和再次发布保持同一绑定。`/42` 让字面量模式复用完整表达式记录，避免从模板声明补造作用域。该序列仅记录历史迁移；6a 的共同节点取代其独立语义模型，不能要求新正文继续经历旧 transport→imported template。已退役的 statement tag 9、TemplateDependency role tag 9 和其他退役字段不复用；必要的旧格式拒绝和兼容性测试保留。

若共同模型沿用现有字节编码，reader 直接构造共同节点；若实际 payload 改变，同批升级 HIR section、required inventory、profile fingerprint 和缓存，重建 core/provider/consumer。不得把实际变更伪装成旧 section 的 optional 扩展，也不为各语法保留不同 profile。

### 10.2 机器目录与对象格式

manifest/single-cone-production `/2` 在原十字段 product 后增加必需 field 11 的 ODR 目录：group record 为 `{1=group, 2=members}`；member record 为 `{1=member, 2=role, 3=abi_fingerprint, 4=definition_fingerprint}`。group 和组内 member 分别按 ID bytes 严格递增，无重复；目录可为空，每个已列出的 group 必须非空，两个摘要各为32 bytes。未知 role、纯语义 GeneratedNominal 和本阶段未启用的 ReleaseHook 不得作为物理目录条目。materialization 仍在原对象投影中，用 member ID 关联，不把物理 offset 再复制进 ODR 目录。该 section 与 bootstrap manifest 是不同的 product；后者 field 11 继续保存 ArtifactFingerprint，不改其含义。

目录由共有最终摘要结果构建，并与已有已验证对象索引中的全部 ODR primary definition 做一次成员集合核对；缺少摘要、多余条目或同一 member 重复均为错误。普通外部引用和仅存在于语义 metadata 的成员不进入目录。读取端先解码结构，再把 group/member/role/ABI/definition 与本次对象读取已重建的目录比较；不重新解析对象或重算已完成的摘要。Code 的 manifest 投影相应成为七字段 product，保留 field 1～6，增加同义 field 11，仍不包含 CodeFingerprint 自身。

目录沿既有 producer/reader 迁移，`single-cone-production/1` 退役；全部当前 production profile 的 required inventory、profile fingerprint 和缓存同步改用 `/2`。历史 Strong profile 的完整目录为空，其既有语义边界继续拒绝 ODR；正式 generic 产物复用同一目录和共有生产投影。后续物理角色沿同一摘要与目录入口接入，不增加另一套发布路径。

`link-identity-closure/7` 沿用 `/4` 的 defined owner 分支 `{0=5, 1=OdrMemberId}`，原 Strong、generated-C、image、verifier boundary 的 tag 1～4 与内容保持。当前产物中的必需 ODR undefined requirement 使用 `{0=9, 1=OdrMemberId}`，退役 tag 2、8 不复用；规范化对象 relocation 使用 runtime target tag 14 后跟该 member 的 32 bytes，不加入本次发射它的 Cone，旧服务专用 runtime target tag 13 保持退役。实际 symbol plan 的 definition owner 可由 foundation 唯一恢复，不在 LIR 生产 section 再复制一份 owner key。`scoop-lir/3` 沿原对象读取器接受并核对实际 ODR weak definition，旧 verifier major 退役；未切换的 Strong profile 仍在既有边界拒绝 ODR foundation。

ReleaseHook role 16 仍只由 M24 启用，本阶段完整 profile 继续拒绝其语义成员。退役字段和 tag 不复用；旧 Strong profile 保持“不支持 ODR”的含义。正式工具链、core、provider 与 consumer 同批重建为新 profile；不把新字段作为旧 section 的 optional 扩展，也不保留长期双轨 reader/publisher。尚未迁移的真实回归入口必须在正式切换前接入共有路径，旧格式仅保留拒绝测试。

模板 body、支持声明、条件、source semantics 进入 HIR fingerprint；实例化结果及 exact dispatch 进入 MIR/LIR；成员定义与物理对象进入各自 Code/Artifact 投影。依赖仍使用已有内容 fingerprint 失效，不建立实例计费、来源证明或额外 cache 资格。普通 whitespace/debug 附件按现有语义/非语义区分处理；影响 source-location 值或实际正文的改动必须反映在内容中。

## 11. 读取、发布与诊断

构建管理继续只持有归档快照、manifest 摘要与依赖 fingerprint；`scoopc` 的共有 reader 消费实际 IR/对象。对同一输入，envelope、HIR、MIR/LIR、object 各自完成一次所需检查，随后复用完整记录。当前编译输出沿原有原子写入路径发布，不重新读取自己与全部依赖执行第二轮完整验证。

现行生产只保留 Strong/ODR 共用的完整 layout writer。已经没有实际生产调用的旧
`SingleConeStrongArtifactInputV1`／`AssembledSingleConeStrongArtifactV1` 及 driver 的 Strong-profile
收窄错误分支、无构造点的 HIR `GenericOdrRequired` 错误项删除；早期底层格式用例直接复用已有 canonical archive 构造器，不为测试保留专用生产入口。
这项清理不改变当前产物格式、fingerprint 或 runtime ABI。

模板读入后直接走 6a 的共同具体化；名字、重载、访问域与默认来源不重新选择。consumer 实参产生的新类型关系及必要条件在对应边界检查，未变化的 provider 声明和布局直接复用。Link 合并只有重复定义与最终引用关系的新检查，不重跑 HIR。

| 错误类别 | 诊断位置与内容 |
| --- | --- |
| 不能推导、错误 bound、非法 `_`、不可访问调用 | 消费方源码的实际类型/调用位置；保留候选和 provider 声明说明 |
| 模板定义错误、非法 polymorphic recursion、default 环 | 定义处；跨模板信息标出原 typed 声明和 source position |
| 替换后的 NoGC、pointee、CLayout/ZST 条件失败 | 消费方实例化点，加定义方具体条件位置 |
| delegate binder 不能仅由 receiver 确定、协议不匹配、`by` 使用 receiver 值 | property 定义或实际使用点，按现有规则区分 |
| 模板 body 缺失、binder/owner/字段或 application 引用损坏 | artifact、section、typed subject 与 wire path，不伪装成源码错误 |
| 同 member 的 ABI/definition 冲突、缺失实际 relocation 目标 | 两个 artifact 和具体 member/role，指出不同内容或缺失目标 |
| 超出当前 native 能力的实际使用 | 维持 native requirement/capability 诊断，不伪装成 generic 不支持 |

已完成的泛型用例不再报告 `SCOOP_HIR_CROSS_CONE_GENERIC_REQUIRED`。错误不依赖 FQN、symbol 文本、当前仅有一个候选或 provider 来源来补齐实体。

## 12. 测试与验收矩阵

### 12.1 真实产物 fixture

沿用 `compiler/driver/src/request/preflight/end_to_end_tests` 和 `tests/fixtures` 的现有机制。每项能力至少有独立 fixture 与组合 fixture，negative 断言实际 span 和诊断；HIR/MIR/LIR golden 锁定真正输出。provider 生成 `.slib` 后移走源码，下游只接收显式产物；不得用手工模板或 metadata 工厂代替正式生产。

| fixture 组 | 必须覆盖 |
| --- | --- |
| generic functions | identity、嵌套调用、local/extension、final method、宿主加方法参数、推导/显式/`_`、class 加多个 interface bound、value/ref |
| generic nominals | class/struct/enum/interface、主次构造器、嵌套 payload、base/interface substitution、default method、abstract override、protected、property/setter |
| calls and defaults | named/default/vararg、receiver 与 RHS 顺序、provider 默认值中的 generic call、继承默认值、第三个 Cone 再发布后消费 |
| hidden support | private non-generic helper、private generic helper、内部类型/属性、definition-site overload；直接 import hidden 和 public default 引用 hidden 必须失败 |
| exact values and ABI | consumer-local type、ZST、24-byte 大值、含引用 struct/enum、tuple、Option/niche、array/vararg、exact cast/type test、boxing 与返回值再次传入 |
| generated functions | lambda/capture、local recursion、callable reference、static/dynamic adapter、generic coroutine 与返回类型 helper、异常/finally、移动 GC |
| native function addresses | 外来参数自由 NoGC 函数的首次 C 地址需求、已存在的 storage bridge、再导出与模板引用、真实 C callback 调用 |
| delegate read/write | val/var、provide、多个 receiver 对象共享、不同 application 隔离、re-export、setter 与复合赋值顺序、ZST/大值/引用 delegate |
| delegate failures | 第一次失败、跨 Cone 再访问同一失败、initializer 副作用只一次、直接/间接 cycle、现有 coordinator 的并发等待 |
| core generic use | 修改并重建 core 后的 Option/array/协程协议与新增泛型声明；普通目录的 core 与其他 provider 走同一路径 |

negative 还覆盖非 identity 递归替换、值布局/继承环、bound 不满足、interface 自有 generic method、virtual generic method、从 result/RHS 推导 property binder、delegate suspend/default/vararg role，以及所有仍被语言排除的形态。

### 12.2 ODR 与对象测试

- 两个 sibling Cone 实例化同一上游 `f<T>` 和 `Box<T>`：共同 member 的 key、canonical ABI/LIR、规范化对象、EH/stackmap 和摘要一致；源文件输入顺序、consumer 路径及物理分片变化不改变这些结果。
- 两个 sibling 使用同一目标函数类型、不同源函数类型的 adapter，或只请求不同 box/coroutine helper：组成员集合可以不同，合并成功；共同 TD/member 仍严格一致。这是整组相等旧规则的必要回归。
- 同组同 member 的 body 常量、layout/scan、vtable、initializer、registration、LSDA/FDE/compact-unwind、stackmap 或 typed relocation 改变：实际 reader/合并器必须拒绝。不同 group 即使机器代码相同也不按内容合并身份。
- 多对象 materialization 的重复/缺失 plan、越界或重叠 range、错误 boundary、缺失关联记录、重复 writer、错误 patch 宽度、ODR 引用消费 Cone 的私有 sentinel、退役 owner/tag/profile：按实际格式和引用错误拒绝。
- 单个产物遗漏已发射 TD/dispatch 或 delegated unit 的必要成员必须失败；另一个同组无关 member 不能掩盖缺失。
- 新 profile 和实际依赖缓存测试覆盖 body/hidden helper/default/条件变化、type arguments 变化、旧产物重建和 unchanged-input 命中，不测试不存在的授权或计费策略。

### 12.3 实际链接与运行

复用 M23-6 已有单 image 运行验收入口：从完整 reader 返回的真实对象与登记数据构建测试入口，链接真实 runtime 和系统 linker。入口只适配登记集合，不生成假的 template、ODR 定义或 program descriptor 产品。

至少包含 provider、两个 sibling 和使用二者的 consumer。实际运行检查：generic 计算结果；同 exact type 的 TD 地址与 dispatch target 合并；不同 exact type 不共址；generic delegate 的 storage/cell/失败状态共用；相同 application 初始化一次、不同 application 分开；ordinary 与 `SCOOP_GC_STRESS_MOVE=1` 下含引用值、closure/coroutine 和异常结果正确。

必要的元数据损坏测试使用显式检查模式。此运行验收不实现 M23-8 的生产多 image registry，也不把测试入口暴露为 M23-9 的 program-link。

每批代码变更先 `cargo fmt --all` 和 `cargo clippy --workspace --all-targets`，再跑对应 fixture/unit/golden。阶段结束构建真实配套编译器，设置 `SCOOP_TEST_PAIRED_SCOOPC` 后运行完整 workspace 与 runtime 测试；省略环境变量导致跳过真实子进程的结果不能作为验收。

## 13. 实现顺序

1. **接收 6a 基线。** 以共同 HIR、真实 encode/decode 等价和既有泛型 fixture 确认输入；本地／依赖类型、默认值、构造、bound、capture、数组和模式不再补导入专用语义。
2. **闭合实际定义。** 从同一具体化结果贯通 Strong/ODR MIR、LIR、ABI、layout、scan、TD、dispatch、对象与 reader，覆盖当前语言全部实际物化需求。
3. **生成实体和对象一致性。** 补全 function adapter、box/coroutine/helper 与参数自由支持服务；验证多对象、共同 member 比较、独立 member 并集、EH/stackmap 和实际地址合并。
4. **泛型 delegate。** 用 6a 的共同正文表达 source template 和完整 LazyAccess application，接入 ensure/coordinator、getter/setter、失败根和 moving GC，完成跨 sibling 初始化测试。
5. **正式产物与交接。** 在 6a 实际格式基线上升级新增的物理 payload、目录、摘要和 profile；重建 core/provider/consumer，删除旧 Strong 专用装配，完成真实子进程、runtime 与多阶段组合回归。

步骤是同一生产管线的增量，不形成按语法或来源划分的 profile／能力档位。每步输出必须结构完备；不能以空 body、假类型、缺失 ABI 或消费者补发 foreign Strong 通过测试。发现共同 HIR 缺口时修正 6a 的合同与共同实现，不恢复本阶段已取消的导入适配路线。

## 14. 完成门与后续交接

M23-7 只有在下列条件全部满足后完成：

- M23-6a 完成门通过；源码／产物使用同结构 HIR、共同查询与具体化，无来源专用语义类型、默认值执行器或 imported template 重建路径。
- provider 源码产生完整模板产物；移走源码后，下游能用本地或依赖类型完成当前语言子集的泛型编译，并再次发布可消费产物。
- LocalConcrete HIR、MIR、LIR 没有未替换 binder、待完成请求、缺失表示或错误 foreign ownership；源码语义错误在前端结束。
- 重复 ODR member 的完整定义必须一致，独立 member 的合法并集通过；实际对象、关联记录、registration、引用和必要 unit 闭包均完整。
- 真实链接运行通过 TD/storage/cell 地址合并、generic delegate 初始化与失败共享、ZST/大值/引用 ABI、异常和移动 GC；没有以 metadata-only 测试替代执行结果。
- core、普通 provider、consumer、reader、publisher 和 cache 使用同一生产路径，无来源授权、通用预算、重复完整重放或仅为测试存在的平行工厂。
- 格式、fingerprint、fixed vector、negative、组合 fixture、三阶段 golden 和实际配套编译器的全仓回归同步通过。

M23-8 接收已经发射的六类 Strong/ODR registration，完成生产多 image 登记与启动；去重依据同一 registration member、定义内容与最终地址，不比较整组成员集合。M23-9 在共有 reader 和 ODR 合并结果上完成正式 artifact-only program-link 与最终产物检查。M23-10 继续处理一般 native provider，M23-11 完成 CLI/历史入口总迁移。它们复用本阶段真实 fixture，不补造缺失模板、ODR 定义或初始化状态。
