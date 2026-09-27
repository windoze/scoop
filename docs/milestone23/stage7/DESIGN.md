# M23-7 设计：跨 Cone 泛型、ODR 与泛型委托扩展属性

状态：实现中（2026-09-27 开始）。依赖 [M23-6 实际产物验收](../stage6/ACCEPTANCE.md)。

本阶段让下游仅凭上游 `.slib` 完成泛型实例化，生成可以与其他 Cone 重复实例合并的真实对象，并开放每个 receiver application 共享一次初始化的泛型委托扩展属性。完成标准包括实际源码、产物消费、链接运行和移动 GC。

权威合同为 [语言规范](../../specs/SCOOP-SPEC.md) 3.2、8.5、9.1.1、9.2、12.5，[运行时规范](../../specs/SCOOP-RUNTIME-SPEC.md) 2.2、2.7、2.8，以及 [实现规范](../../specs/SCOOP-IMPL-SPEC.md) 2.13。本文细化 [M23 总设计](../DESIGN.md) 3.4、4.3、7、10，修订其中以整组成员集合相等判定 ODR 的旧要求；既有 specialization、group/member 身份及 mangler 保持。

## 1. 范围与实现基线

### 1.1 本阶段能力

| 能力 | 成功后的行为 |
| --- | --- |
| 泛型函数 | top-level、extension、local function 及 final generic method 可从依赖模板实例化；显式实参、`_`、推导、class/interface 与 `value`/`ref` bound 使用同一前端 |
| 泛型名义类型 | class、struct、enum、interface 的完整 application 保留真实字段、父类型、构造器、成员、属性、默认实现、exact RTTI 与 ABI |
| 模板内部实现依赖 | 可引用定义处合法的 private/internal helper、类型与属性；名称查找仍只公开原来的 public binding |
| 函数值与生成实体 | 实例化正文中的 lambda/local function、capture、callable reference、function adapter、coroutine frame/step/slot/shell/start 形成完整的 typed 定义与引用 |
| 泛型委托扩展属性 | getter/setter 共用由 property origin 与完整 receiver arguments 决定的 lazy storage、cell、failure root 和初始化实现 |
| 产物与 ODR | Strong 与 ODR 共用正式生产、读取和对象处理路径；重复 member 必须定义一致，同组独立 helper 可以按实际需要分别发射 |

同一泛型既可使用定义 Cone 已知的类型，也可用下游定义的 class、struct、enum、ZST 或含 managed reference 的类型实例化。core 中的泛型声明沿用同一路径，包括 `Option`、数组与协程协议；不恢复 core 专用物化入口。

本阶段不开放 generic alias、interface method 自有类型参数、virtual generic method、polymorphic function value、polymorphic recursion、运行期 dictionary 或类型擦除。现有 FFI 声明和 generated bridge 的类型替换、ABI 与 requirement 必须完整；一般 native provider 查找仍归 M23-10。

### 1.2 已有能力与实际缺口

| 当前入口 | 已有能力 | M23-7 的必要改动 |
| --- | --- | --- |
| [`identity/entity/odr.rs`](../../../compiler/identity/src/entity/odr.rs) | 四类 specialization、group/member、role/discriminator 的 typed key | 复用实体身份；按实际 member 建立定义记录 |
| [`cross_cone_interface/section.rs`](../../../compiler/hir/src/cross_cone_interface/section.rs) | 完整声明、binder、参数、默认值、定义位置及依赖引用 | 加入可具体化的泛型正文、构造初始化序列和 delegate template |
| [`hir-lower/concretize.rs`](../../../compiler/hir-lower/src/concretize.rs) | 本地固定点具体化、独立 concrete ID、外来参数自由引用 | 在同一工作队列中消费外来模板；去除依赖本地声明 arena 的假设 |
| [`concretize/callables.rs`](../../../compiler/hir-lower/src/concretize/callables.rs) | 本地 bound-call 具体化 | actual 类型和 bound 来自本地或依赖时均通过共有声明及 conformance 查询 |
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

`TemplateSupportHidden` 只表示普通的跨对象可链接、native hidden 定义。前端的实际 template 引用和原声明可见性足以决定保留该符号；不生成逐边授权表、访问收据、来源包装或“同次编译”证明。若同一 subject 同时需要公开机器导出，保留 `ConeStrong`，不以 hidden 缩窄其他合法使用。源码 public lookup 仍由原有 binding 表决定。

generic helper、template-owned lambda/local function 或含宿主 binder 的 constructor/member 需要在消费方替换，才随模板导出。是否字面出现 type parameter 不能代替词法归属：捕获外层值的局部函数仍由其 enclosing application 物化。

## 3. 共有 HIR 中的模板

### 3.1 导出根与唯一数据来源

模板根来自公开 generic callable、generic nominal 的可调用/继承表面、generic extension property，以及这些根真实引用的支持模板。generic nominal 的完整 source shape、modality、binder bounds、字段、variant、父类型、成员和 property 继续保存于共有声明表，不再复制到一张“泛型声明表”。

需要新增的内容只有目前缺失的可替换执行数据：

| 数据 | 必需内容 |
| --- | --- |
| callable body template | 原 typed callable declaration/generated lexical owner、完整 typed body、局部值与 capture、已有条件约束 |
| constructor/common initialization | 原 nominal/constructor identity、delegation target 及求值序列、primary field 写入、按源码顺序的 field/delegate/`init` 操作、secondary body |
| generic delegate template | 原 extension property identity、effective delegate type、`by` body、可选 provide role、get/set typed target 及条件约束 |

abstract slot、intrinsic 声明和 source extern 继续使用各自已有 implementation 分支，不为它们填空 body。参数自由 source helper 只增加必要的共有支持声明及真实机器导出。未使用的本地实例、arena 索引、AST、resolver scratch 和失败候选不进入模板。

模板可引用 direct 或 support provider 的声明。只保存原 typed target 和实际 provider；re-export 不复制正文、不改 origin。一次 reader 得到的不可变模板和声明供后续查询复用。模板内已经绑定的引用不制造消费方 public lookup observation，也不重新枚举 hidden 名称；只有消费方实际源码 lookup 才进入原候选记录。

当前 Cone 在 Export HIR 完成后、LocalConcrete HIR 生成前收集一次共有声明与模板正文闭包，并随不可变的 Export HIR 输出保留该结果。公开声明、默认值与泛型正文实际引用的私有具体类型进入同一组 shape support roots；HIR 类型语义、MIR/LIR 布局和正式共有 section 复用这一结果，不重新从 public binding 子集收集根。未引用的私有声明不因此进入共有支持表。该结果是当前编译的数据投影，不新增产物字段或来源资格。

### 3.2 默认值与泛型正文共用节点

将当前默认值 transport 中确有复用的 typed expression、statement、pattern、binding、capture 和 substitution 操作整理为同一组 HIR 内部模块。默认值和泛型正文使用不同根类型：

```text
ExportDefaultTemplate
    parameter origin + existing calling protocol + TemplateBody

ExportGenericBody
    source callable / initialization owner + TemplateBody + predicates
```

不复制一套 `GenericExpression`、`GenericStatement` 和完整 reader，也不以默认参数伪造 generic body 的 owner。共同节点的序列化仍只承担格式和引用检查，语言规则在 `hir-lower` 完成。

模板表达式必需保存已解析的类型、target、effect 和定义位置；类型允许声明作用域内的 binder。调用节点保存已选声明、宿主与 callable 两组符号化类型实参、已提交的 dispatch 方式和完整实参求值计划。只有完整替换后才产生 persistent exact application，不能给 `T` 或 `_` 伪造 exact ID。

bound member 节点保存实际 class/interface bound 与原 slot/callable identity。实例化后只完成规定的 exact dispatch 选择，不用实际类型额外出现的方法重新参与 overload resolution。

消费方把实际需要的外来 callable body 转换为独立的 imported template 数据，保留原 typed declaration、定义位置和完整正文；该转换复用默认值的节点替换。imported template、调用的符号化 application 与最终 concrete function 使用各自的 typed ID，不分配当前 Cone 的 `FunctionId` 或复制源声明身份。具体化队列同时处理本地与 imported 请求，最终 callable materialization 仍引用定义方声明及完整 exact arguments。

constructor、field、variant、property accessor、local value、loop target 和 callback registration 分别使用其原 typed ID。loop/cleanup、`try`/`finally`、enum pattern、callable reference、receiver adaptation 与 source location 都沿用既有 HIR 语义。

lambda/anonymous function 的模板正文保留按定义处捕获顺序排列的类型表，正文读取以该表中的位置引用闭包输入；该位置是当前正文内的 ABI 索引，不是新的全局实体身份。创建嵌套闭包时，捕获来源明确区分当前局部值与外层闭包输入。局部具名函数按现有 ABI 将捕获值作为前置参数，正文投影直接引用这些真实参数。不得将 request-local `BindingId` 写入产物，也不得把闭包输入误当作当前函数尚未定义的局部值。

### 3.3 默认参数的边界

public default 仍只能直接引用覆盖其完整调用域的实体；generic body 可以引用定义处合法的 narrower 实现。二者共享节点不共享访问规则。private generic helper 自己的默认值按该 helper 的实际调用域检查，不能把正文依赖资格传播给 public default。

显式实参、默认参数、`vararg` 和前置参数引用继续按 8.5 求值。默认值中嵌套泛型调用的 binder 随外层模板替换；实际省略参数时才展开默认值。定义位置和求值位置分别保留，实例化失败报告消费方使用点，并附 provider 中的约束/声明位置。

generic body 自身的 `current_source_location` 使用该正文中的语义求值位置，不因某个 consumer 首先实例化而变成调用点。实例化诊断链只用于诊断，不进入 body、常量、symbol 或 ODR fingerprint。

### 3.4 条件约束与闭包

复用已经存在的 nominal bounds、`RequiresGcFreePointee`、NoGC 条件和 `CLayoutFieldRequirement::CFieldSafeAndNonZst`。条件保存实际 binder/type 与声明位置；完整替换后由现有算法判定。新增闭包不能替代 FFI 的 C-safe、ZST、pointee 或调用约定规则。

producer 从实际 typed body 收集必须导出的支持引用和 source records。reader 检查记录、引用范围、binder 作用域、owner、签名连接与编码完整性；不重跑名字查找、重载决议、访问域证明或全量类型检查。已完成的同一引用与类型关系不在每个下游 stage 再完整检查一遍。

## 4. 消费方的 HIR 具体化

### 4.1 接入现有工作队列

本地模板与读出的外来模板各自保留原有身份，通过当前 concretizer 的实际查询入口取得声明和正文。不能先把外来声明复制成“本地同名函数”来复用本地索引。

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

generic host 的普通方法和 accessor 具有 exact owner；generic method 另有 own arguments，二者顺序和身份沿用语言 3.2。interface default、abstract override、`super<I>`、protected receiver、属性 setter 权限、constructor readiness 继续在前端按实际声明检查。

value/ref、GC-free、enum variant facts、ZST 和 `Option` niche 在 concrete 输出中必须完备。别名先展开为原 exact target，不产生新实例；不同 nominal arguments 即使 ABI 相同也保留不同 exact identity、TD 和 ODR member。

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
| box、coroutine step/slot、shell/start、derived equality | 既有 `ExactOwnerRoot` |
| static/dynamic function adapter 及 environment | 既有目标 `FunctionShape` 的 Structural root，source signature 仍属于 member key |
| dispatch/boxing adjust | implementor/payload 的既有 root；slot 和目标实现是 typed 依赖 |

物理 producer 和 semantic owner 分开。普通 Strong 引用携带真实定义方；ODR 引用携带 group/member。reader 可以记录某个物理候选所在 provider，但该候选位置不进入 ODR identity 或 canonical relocation。

### 5.2 每次物化的必要闭包

“按需发射”不意味着可以产生残缺记录：

- nominal/structural type 的实际 TD、layout、scan、diagnostic bytes、父类型和完整 dispatch 表相互对应；表中每个 target 必须解析到已有 Strong 或实际 ODR 定义。
- 实际 box/adjust、function adapter、coroutine helper 包含自己产生的 environment、字段、type/scan、callable、registration 和常量；引用其他 root 使用明确 typed edge。
- 每个 callable 覆盖实际 body 及其 LSDA、FDE/CIE、compact unwind、safepoint 和 LLVM stackmap。不能只比较 `__text`。
- 每个被取址的 specialization-owned string/constant/diagnostic atom 保持原 owner 与 stable structural path，不落入首次使用 Cone 的私有常量池。
- 所有外部引用都在当前产物或显式依赖闭包中有完整目标；错误 owner、缺失目标、错误 ABI 和 dangling relocation 均拒绝。

普通方法属于自己的 callable application，不因某个 consumer 多调用一个方法而改变 nominal TD 的定义。一个 group 中新增独立 helper 也不会改变已经存在 member 的定义摘要。

### 5.3 参数自由类型的有限支持

M23-6 已发布的 box、coroutine step/slot 继续由 source nominal 的定义 Cone 提供。M23-7 为可跨 Cone 请求的参数自由 source exact 补齐 `ContinuationShell<R>` 的 success/failure 与 `CoroutineStart<R>`、canonical ABI、callable registration 及其真实 `Continuation<R>`/`SuspendTask<R>` 依赖。

这些依赖 application 只产生其表示、dispatch 和实际被调用的方法，不递归补齐所有新 application 的 shell/start。对 nominal application 和结构类型，helper 按实际使用物化，并仍归既有 root；member 并集规则使不同使用集合可以正常链接。

缺失定义方的参数自由 helper 是产物错误，不能由消费方生成第二个 Strong 或伪造 Structural root。

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
      5: object_definition_leaves_by_digest_node,
      6: stackmap_leaves_by_site })
```

三个 leaf 集合都是必需的 canonical array。实际物理 member 的 canonical LIR 与 object leaves 非空；没有 safepoint 时 stackmap array 可以为空。ABI payload 按 role 保存实际函数签名、布局、存储或 registration 合同。

纯语义的 `GeneratedNominal` identity/member record 继续保存于对应 IR foundation，完整生成类型保存在 MIR/LIR metadata；它本身不进入 manifest 的物理定义目录，不生成 ABI 摘要、空 `od` marker、ObjectDefinitionPlan 或虚构 range。其实际 TD/layout/scan/callable 各自作为物理 member 检查。source/key 存在与实际机器定义分开，缺少真正需要的物理成员仍是错误。

不再生产旧 `scoop-odr-abi-v1`、`scoop-odr-definition-v1` 的整组摘要，也不另加 group union hash。Code/Artifact fingerprint 已覆盖整个产物目录；同一 group 的成员是否兼容由成员比较和引用检查决定。

### 8.2 复用现有 digest 计算

最终 LIR 的 `StructDef` 与 `EnumDef` 必需保存实际 `PersistentExactTypeId`，lowering 从 MIR 已解析的 exact-type record 直接传入；完整 key 仍由同一份 `LirMeta.exact_types` 保存。session-local 的 `StructDefId` / `EnumDefId` 只用于访问当前表示表，canonical LIR、ABI 和全局引用使用记录中的持久身份。名称、arena 排列、相同布局或字段集合不能替代该身份；表示、扫描和 C ABI refinement 的后续填写必须保留它。泛型实例使用其完整 application 身份，生成 enum 使用已有 generated exact-type record。

函数正文的 canonical LIR leaf 使用 `scoop-lir-definition-v1`，从最终 `Function` 的实际 ABI、GC effect、指令、正常/异常控制流与 root plan 计算。块按入口可达 CFG 的固定后继顺序规范化，local/temp 按正文首次出现顺序编号；未使用的 arena 槽、诊断名称和不可达块不进入这个语义投影，实际发射的所有对象 atom 仍由 ObjectDefinition 覆盖。类型、函数、存储、桥和 safepoint 使用已有 typed persistent identity，call target/signature/root-scan 等函数局部表在使用位置编码实际值；不编码 arena ID、dump、LLVM 文本或最终摘要补丁。

production section 新增必需 field 13，保存按 callable-body ID 排序的 `{1=body, 2=canonical LIR fingerprint}` records；此排序独立于 foundation 为解析引用使用的拓扑顺序，并精确覆盖全部 callable-body；其中普通 Strong/ODR 正文与 lowering 已生成的 root/init gateway 均按实际 `Function` 编码，不用空记录或入口计划替代 gateway 正文。producer 从同一最终 LIR 计算一次，reader 检查集合、引用、排序和 fingerprint 格式后复用；物理对象仍独立按实际内容验证。当前两种 Strong reference schema 的 production capability 分别由 `/11`、`/12` 升至 `/13`、`/14`，旧产物重建，ODR 发布限制保持；完整泛型 profile 最终仍统一使用本节设计的 cone-production/1。

保留 `DigestKind::OdrDefinition = 8`。旧 owner variant `OdrDefinition(OdrGroupId) = 8` 退役，新增 `OdrMemberDefinition(OdrMemberId) = 11`；后者映射到 kind 8。其他 owner tag 与 patch field role 不变，不复用退役编号。

每个 ODR node 只汇总该 member 的 LIR、object 和 stackmap leaves。registration 的 `definition_fingerprint` 来自该 registration member；callable 的 `body_definition_fingerprint`、TD 的 descriptor 字段继续来自对应 ObjectDefinition。计算 registration 的对象 leaf 时，其自身最终 ODR slot 归零；已有 body/layout/scan 等上游字段保留。

callable 与 safepoint registration 的 canonical LIR 直接投影已有实际 production plan：callable 使用 `{0=6, 1=body, 2=entry symbol request}`，safepoint 使用 `{0=5, 1=site, 2=runtime safepoint id, 3=owner body, 4=site role, 5=root-pair count}`，在 `scoop-lir-definition-v1` 域计算。这里不编码物理 member、patch intent、最终摘要或 producer。注册 ABI payload 为 `{0=record kind, 1=实际 ABI version, 2=实际 record byte size}`；kind 沿用 runtime 的 callable=6、safepoint=5，当前 version=1、size 分别为192和232。它描述注册记录自身的格式，函数调用签名另由 callable-body member 的 ABI 覆盖。

逐 member definition 的 LIR leaf 编码为 `{1=atom, 2=fingerprint}`，object leaf 为 `{1=digest node, 2=fingerprint}`，stackmap leaf 为 `{1=site, 2=fingerprint}`。callable registration 各有一个自身 primary 的 LIR/object leaf，stackmap array 为空；safepoint registration 同样各有一个自身 LIR/object leaf，并含该 site 唯一的已规范化 stackmap。ODR safepoint 对象摘要须先代入其实际上游 normalized-stackmap 字段，并记录该直接输入，再计算自身 member definition；不能沿用 Strong 的全零 provisional record 对象摘要。callable registration 对象已包含实际上游 body-definition，最终 ODR 汇总不再次把 body node 当作自身 object leaf。

共有注册摘要结果区分 `StrongRegistration` 和 `OdrDefinition`，补丁写入与 RuntimeImage 编码使用实际 owner 及对应 digest kind。image 表中的注册指针按期望的 typed entity/registration role 核对已解析目标的实际 primary definition、symbol 与 owner，不重新派生 Strong-only definition；其余 relocation 宽度、形式、零 addend 和所属范围检查保持。Strong manifest 注册表只接受 Strong 摘要；在完整 ODR member 目录和正式泛型 profile 接通前，不能将 ODR 值塞入旧表或静默丢弃。既有 Strong 编码和摘要向量保持不变。

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

## 9. 泛型委托扩展属性

### 9.1 source template 与 application

`val/var <T> Receiver<T>.p: U by expression` 的全部 binder 必须只由 receiver 的 exact 静态类型和 bound 唯一确定。result expected type、setter RHS、runtime receiver class 和使用 Cone 不参与推导或 specialization key。

源级 `by` 没有某次访问的 receiver 值，不能使用该值的 `this`、字段或 identity。它可以使用已经声明的类型参数及定义处可见实体；先求值 `by`，再可选调用一次无 receiver-argument 的 `provideDelegate`，保存 effective delegate。get/set role 按已有协议接收实际 `thisRef`，保持 ordinary、non-suspend、non-generic、无 default/vararg。

source HIR 增加独立 generic delegate template，不能把它先降成 `ManagedGlobal`。具体化后使用本地 typed `GenericDelegateStorageSpecializationId` 关联完整 unit 和 storage；持久身份直接复用已有 `InitializationUnitKey::GenericDelegatedExtensionApplication { property, receiver_arguments }` 及 storage role，不建立第二套 persistent specialization key。

receiver arguments 按 property binder 声明序排列；透明 alias、re-export、getter/setter 使用方式及不同 receiver 对象都不改变 key。不同 arguments 各有独立 delegate 与失败状态。

initializer/ensure 的 `GeneratedCallableKey::Initialization` 仍只引用声明级 extension-property unit 与各自 role；application unit 只进入 `CallableMaterializationContext::InitializationApplication`。其中的 local generic function 使用既有 `EnclosingInitializationApplication`。不能把具体 receiver arguments 写回 source template identity，也不能让 getter、setter、initializer 和 ensure 共用 callable ID。

### 9.2 固定的 unit 闭包

每次物化一个 delegate specialization 都必须产生以下成员或其明确的同一 specialization 定义引用：

- effective delegate storage 及 static-storage registration；即使 delegate GC-free 或 ZST 也保留真实 storage/token。
- GC-free initialization cell，初始为 Uninitialized；managed failure storage 及 root registration，初始为 null。
- initializer、ensure、二者的 callable registration，以及实际 EH、safepoint 和 stackmap。
- `LazyAccess` initialization descriptor/registration、稳定 diagnostic path 和全部 typed 引用。
- 本次 initializer/ensure 自身产生的 closure、常量、类型/scan 等必要支持；其他 nominal/callable root 以 typed edge 引用。

getter/setter 继续使用各自 callable application，二者引用同一 delegated-property group。只读取 `var` 不要求预生成未使用的 setter；unit 的固定状态成员仍不能少。Lazy unit 不生成 startup gateway，C 中相关 id/digest/pointer 保持规范的全零分支。

### 9.3 求值、失败与 GC

普通读取先求值 receiver，再进入 getter ensure，成功后读取 delegate 并调用 get role。普通赋值先求值 receiver、再求值 RHS，进入 setter 后 ensure，随后调用 set role。复合赋值按 receiver → getter/ensure → RHS → operator → setter 的原有顺序，receiver 只求值一次；prefix/postfix 更新同样复用已有展开。

ensure 复用 M21 的 RunInitializer/Ready/Failed/Cycle 路径。winner 在线程的 managed roots 中完成 `by` 与 `provideDelegate`，成功后写入已登记 storage 并发布 Initialized；失败写入相同 specialization 的 failure root 并发布 Failed，后续访问重新抛出同一失败且不重试。并发等待、同线程重入、跨线程 cycle 与异常传播使用现有 coordinator。

delegate/failure 中的 managed reference 由普通 root 和 scan 更新，中间值遵守 moving-GC 规则；不 pin、不放入 GC-free cell，也不在 ensure 前缓存待移动的 delegate 地址。非泛型 extension delegate 保持原来的 eager 语义。

## 10. wire、profile 与缓存迁移

以下是本设计待实施的版本，不表示当前代码已经支持。正式生产 profile 为 `org.scoop-lang.slib-profile/cross-cone-generic/1`。它从 M23-6 的完整 inventory 继承未变化项，替换以下项；descriptor 仍只有 id 与必需 section 清单。

| section/capability | 本阶段版本 | 变化 |
| --- | --- | --- |
| `org.scoop-lang.manifest/single-cone-production` | `/2` | 保留单 Cone 产物含义，完整 Strong/ODR materialization 与新增必需 ODR member 目录 |
| `org.scoop-lang.hir/cross-cone-interface` | `/34` | 原 field 1～10 保持；必需 field 11、12、13 分别承载 callable body、constructor initialization 与 delegate template；实际调用记录保存 application |
| `org.scoop-lang.hir/cross-cone-type-semantics` | `/9` | exact application 的完整 facts、继承和 actual type uses；不增加来源资格 |
| `org.scoop-lang.mir/cross-cone-type-bridge` | `/2` | exact specialized type/callable、生成实体与 Strong/ODR 定义引用 |
| `org.scoop-lang.lir/identity-foundation` | `/2` | 新的 member digest owner；拒绝旧 group owner tag 8 |
| `org.scoop-lang.lir/cross-cone-layout-abi` | `/4` | 布局、descriptor、dispatch 和 callable 的 Strong/ODR 定义引用 |
| `org.scoop-lang.lir/cross-cone-link-closure` | `/2` | 普通 callable requirement 扩展到实际 ODR target |
| `org.scoop-lang.lir/cross-cone-layout-link-closure` | `/3` | layout/descriptor/helper 的实际 ODR 引用 |
| `org.scoop-lang.lir/link-identity-closure` | `/4` | ODR definition、symbol、relocation 与 member-aware materialization |
| `org.scoop-lang.lir/cone-production` | `/1` | 取代 Strong production `/12`，统一表示完整 Strong/ODR 定义、六类 registration、image 与 digest plan |
| `org.scoop-lang.link-object/scoop-lir` | `/3` | 同一 Mach-O verifier 支持并核对实际 ODR 对象与 member 摘要 |

HIR identity-foundation `/3`、MIR identity-foundation `/1`、现有 compiler protocol、参数自由调用桥及 generated-C verifier 的 bytes 合同不变；它们不是另一条 generic 生产路径。三层 outer schema、callable-body-v1、persistent identity schema 和 `persistent-v1` 保持。LIR foundation `/2` 已用于本阶段，因此 M24 对应 major 顺延为 `/3`，其三层 outer schema 升代仍按 M24 设计。

HIR cross-cone-interface 的新增字段直接保存三张 canonical 表：field 11=按 typed callable owner 排序的 body records，field 12=按 nominal/constructor owner 排序的 common/delegation initialization records，field 13=按 extension property ID 排序的 delegate templates。每条记录引用共有声明、签名和定义位置，内含完整 body/sequence 与该记录实际需要的 predicates；无对应内容时表为空。禁止用空 body 表示 abstract、intrinsic 或缺失实现。

生产按功能分步迁移：`/31` 增加必需 field 11 与捕获表示，`/32` 为实际调用增加必需 application 字段，`/33` 增加必需 field 12，`/34` 增加必需 field 13；每次同步 reader、required inventory、profile fingerprint 与固定向量，拒绝旧 major。未完成的后续表不提前写入占位记录。callable body 是十字段 product：owner、locals、parameter indices、statements、result、effects、type parameters、predicates、definition origin、capture types。共享 expression tag 59 表示闭包输入读取；capture 的 source 为 `{0=kind, 1=index}`，kind 1 引用本地值表，kind 2 引用当前正文的捕获类型表。局部值及捕获索引分别在各自正文范围内解析，不能跨正文引用。

共有 external reference 的实际 call-site product 使用必需 field 8 保存实例化分支：`{0=1}` 表示直接调用，`{0=2, 1=PersistentCallableApplicationId}` 表示 application。该 ID 引用已有 canonical application 表，不复制 binder、实参或另一份签名。读取时将 application 的原声明、宿主与 callable 实参连接到 provider 声明，并检查完全替换后的实际参数与结果类型；泛型声明不能使用直接分支，application 不能引用另一声明。实际泛型调用和参数自由调用使用同一 occurrence、绑定记录和 source-location 路径。实例化正文中的调用保留 provider 的求值位置，按实际 root 的模板定义检查；普通消费方调用及默认值展开继续使用自己的求值位置，不要求所有调用位置都属于消费 Cone。

共有 external reference 的 `TemplateDependency` 使用新 role tag 10，保存模板中实际外来声明及类型引用；旧 tag 9 保持退役。该 role 不携带消费方 lookup observation 或绑定路径，只在 reader 边界核对正文引用、实际 provider 与声明记录的一致性。

manifest/single-cone-production `/2` 在原十字段 product 后增加必需 field 11 的 ODR 目录：group record 为 `{1=group, 2=members}`；member record 为 `{1=member, 2=role, 3=abi_fingerprint, 4=definition_fingerprint}`，均严格排序、无重复。materialization 仍在原对象投影中，用 member ID 关联，不把物理 offset 再复制进 ODR 目录。该 section 与 bootstrap manifest 是不同的 product；后者 field 11 继续保存 ArtifactFingerprint，不改其含义。

`link-identity-closure/4` 的 defined owner 增加 `{0=5, 1=OdrMemberId}`，原 Strong、generated-C、image、verifier boundary 的 tag 1～4 与内容保持。当前产物中的必需 ODR undefined requirement 使用 `{0=9, 1=OdrMemberId}`，退役 tag 2、8 不复用；规范化对象 relocation 使用 runtime target tag 14 后跟该 member 的 32 bytes，不加入本次发射它的 Cone，旧服务专用 runtime target tag 13 保持退役。实际 symbol plan 的 definition owner 可由 foundation 唯一恢复，不在 LIR 生产 section 再复制一份 owner key。`scoop-lir/3` 沿原对象读取器接受并核对实际 ODR weak definition，旧 verifier major 退役；未切换的 Strong profile 仍在既有边界拒绝 ODR foundation。

ReleaseHook role 16 仍只由 M24 启用，本阶段完整 profile 继续拒绝其语义成员。退役字段和 tag 不复用；旧 Strong profile 保持“不支持 ODR”的含义。正式工具链、core、provider 与 consumer 同批重建为新 profile；不把新字段作为旧 section 的 optional 扩展，也不保留长期双轨 reader/publisher。尚未迁移的真实回归入口必须在正式切换前接入共有路径，旧格式仅保留拒绝测试。

模板 body、支持声明、条件、source semantics 进入 HIR fingerprint；实例化结果及 exact dispatch 进入 MIR/LIR；成员定义与物理对象进入各自 Code/Artifact 投影。依赖仍使用已有内容 fingerprint 失效，不建立实例计费、来源证明或额外 cache 资格。普通 whitespace/debug 附件按现有语义/非语义区分处理；影响 source-location 值或实际正文的改动必须反映在内容中。

## 11. 读取、发布与诊断

构建管理继续只持有归档快照、manifest 摘要与依赖 fingerprint；`scoopc` 的共有 reader 消费实际 IR/对象。对同一输入，envelope、HIR、MIR/LIR、object 各自完成一次所需检查，随后复用完整记录。当前编译输出沿原有原子写入路径发布，不重新读取自己与全部依赖执行第二轮完整验证。

模板读入后的实例化执行正常前端工作；它不是重新验证整个 provider 源码。consumer 用实际 local type 代入产生的新关系必须检查，未变化的 provider 声明和布局直接复用。Link 合并只有重复定义与最终引用关系的新检查，不重跑 HIR。

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

1. **共有数据和格式。** 落地本文的 template body 共用节点、必需字段、逐 member 摘要和 profile；先用真实源码 round-trip 验证完整输入，旧格式稳定拒绝。
2. **第一条纵向主线。** public generic function 使用基础类型和 consumer-local struct，经过真实 provider `.slib`、HIR/MIR/LIR、ODR 对象、reader 和单 image 运行；先形成可用闭环。
3. **支持引用与调用语义。** 接入 private/helper、default/vararg、两组 binder、bound dispatch、local function/capture；修正本地/core arena 假设，统一诊断和 origin。
4. **泛型类型与派发。** 完成 class/struct/enum/interface、constructor/common initialization、属性、继承、ABI/ZST/scan/TD；全部复用共有依赖记录。
5. **生成实体和对象一致性。** 补全 function adapter、box/coroutine/helper 与有限参数自由 shape support；完成多对象、共同 member 比较、合法并集、EH/stackmap 和实际地址合并测试。
6. **泛型 delegate。** 增加 source template 和完整 LazyAccess application，接入已有 ensure/coordinator、getter/setter、失败根和 moving GC，运行跨 sibling 初始化测试。
7. **正式切换与收尾。** core、构建管理、driver、cache 和 fixture 统一新 profile；删除无调用的旧 Strong 专用装配路径，完成负例、golden、真实子进程、runtime 和文档回归。

步骤是同一生产管线的增量，不是可长期保留的七种 profile 或能力档位。每步输出必须结构完备；不能以暂时返回空 body、假类型、缺失 ABI 或消费者补发 foreign Strong 来通过阶段测试。

## 14. 完成门与后续交接

M23-7 只有在下列条件全部满足后完成：

- provider 源码产生完整模板产物；移走源码后，下游能用本地或依赖类型完成当前语言子集的泛型编译，并再次发布可消费产物。
- LocalConcrete HIR、MIR、LIR 没有未替换 binder、待完成请求、缺失表示或错误 foreign ownership；源码语义错误在前端结束。
- 重复 ODR member 的完整定义必须一致，独立 member 的合法并集通过；实际对象、关联记录、registration、引用和必要 unit 闭包均完整。
- 真实链接运行通过 TD/storage/cell 地址合并、generic delegate 初始化与失败共享、ZST/大值/引用 ABI、异常和移动 GC；没有以 metadata-only 测试替代执行结果。
- core、普通 provider、consumer、reader、publisher 和 cache 使用同一生产路径，无来源授权、通用预算、重复完整重放或仅为测试存在的平行工厂。
- 格式、fingerprint、fixed vector、negative、组合 fixture、三阶段 golden 和实际配套编译器的全仓回归同步通过。

M23-8 接收已经发射的六类 Strong/ODR registration，完成生产多 image 登记与启动；去重依据同一 registration member、定义内容与最终地址，不比较整组成员集合。M23-9 在共有 reader 和 ODR 合并结果上完成正式 artifact-only program-link 与最终产物检查。M23-10 继续处理一般 native provider，M23-11 完成 CLI/历史入口总迁移。它们复用本阶段真实 fixture，不补造缺失模板、ODR 定义或初始化状态。
