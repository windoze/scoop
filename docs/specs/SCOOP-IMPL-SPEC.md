# Scoop 实现大纲

版本：0.2（草案）

配套文档：`SCOOP-SPEC.md`（语言规范）、`SCOOP-RUNTIME-SPEC.md`（运行时规范）。本文引用其章节号。

## 1. 总体技术路线

- 编译器实现语言为 **Rust**，LLVM 绑定使用 **inkwell**（feature `llvm22-1`）；个别 inkwell 未覆盖的 LLVM 子系统（statepoint / stackmap 等）可降落到 `llvm-sys`；
- 使用 **LLVM 22.1** 作为编译器后端（与当前 Rust 工具链自带的 LLVM 版本一致）；
- 使用 **LLVM stackmap** 生成 GC 所需的 metadata；
- 使用 **LLVM landingpad** 作为 exception 的基础设施（Windows / catchpad 留待后续）；
- runtime library 使用 **C 语言**，M9/M13 基线包含一个单代、非移动的 **Immix GC** 以及其他必须的 runtime 功能；M15加入精确根与moving compaction，分代及parallel/concurrent collector留待后续；
- GC register root 的操作使用平台相关汇编。

## 2. 编译器 pipeline

**模块边界**：stage 之间只通过 IR / meta crate 交换数据——AST、HIR、MIR、LIR 的定义（含各自的 `.slib` meta 格式）独立成 crate，作为 stage 之间的通道。每个 stage crate 只负责把输入变成输出，只依赖其输入/输出的 IR crate，不了解、不依赖上游 stage 的实现；stage 之间的编排由 driver 负责（见 2.7）。

### 2.1 parser / AST

解析源代码，为每个源文件生成语法树。

lexer 对坏字符及可恢复的字面量错误继续扫描；parser 分别以顶层声明、类型成员、块内语句为同步边界，在一次解析中收集同一文件的多个独立诊断。诊断按源码顺序输出；只要存在任一诊断，恢复得到的残缺 AST 必须整体丢弃，不得进入 HIR。

### 2.2 HIR

负责 desugaring、type check 和 overload resolution；综合上游 Cone 的 HIR export representation；解析每个表达式/子表达式的 type，解析每个 callable 的 target。输出不是一个同时容纳parameterized与concrete节点的`Module`，而是按消费者严格隔离的两个IR：

- **`ExportHir`**：只供下游Cone的HIR阶段消费。它包含源码可见的导出语义表面（包括非generic concrete声明的签名/成员/属性）、generic声明与template body、导出的`const val`、调用处实例化的默认表达式，以及这些template所引用但不一定源码可见的类型化依赖闭包。它不包含本Cone局部产生的concrete实例体；
- **`LocalConcreteHir`**：只供本Cone的MIR阶段消费。它包含本Cone需要发射的全部non-generic及fully instantiated type/function/body；其中不允许出现type parameter、parameterized type/body或待完成的实例化请求。

二者必须是不同的Rust输出类型，并使用互不兼容的实体id家族，例如`ExportTypeId`/`ExportFunctionId`与`ConcreteTypeId`/`ConcreteFunctionId`。禁止用同一个arena index、type alias、共享`TypeId`或运行期tag区分两侧实体。一个源码声明同时需要导出语义接口和本地实现时，HIR显式生成两个实体并保存类型化映射；不能让两个消费者读取同一节点的不同字段。

`ExportHir`中的param-free导出声明不等于`LocalConcreteHir`实体：前者是跨Cone语义接口，后者是本地可执行实例。下游HIR确实需要前者来完成非generic调用、继承/接口检查、构造、重载、默认参数和const求值，但绝不能读取本Cone的concrete实例body。反之，本Cone MIR不得读取generic template；HIR必须先对实例化需求做固定点闭包、完成类型替换并生成结构完备的`LocalConcreteHir`实例体。

**所有编译期错误都在 HIR 层报告**，之后的 stage 不再做源代码错误处理。

HIR 负责解析所有type parameter：确定每个generic调用的具体类型实参（含对上游Cone `ExportHir` template的实例化请求），对调用图和类型依赖做固定点闭包，并在`LocalConcreteHir`中生成完成替换的concrete实例体。MIR不接收实例化需求清单，也不读取template后自行替换`TypeParam`。

每个`ConcreteTypeId`索引的实体都必须携带非可选的`gc_free: bool`。concrete enum实体还必须为声明顺序中的每个concrete variant携带非可选的`gc_free: bool`，enum自身的flag恒等于全部variant flag的逻辑AND。不存在“未知”的concrete类型，也不得使用`Option<bool>`、默认false、延迟回填或MIR/LIR递归字段来补齐该属性。尚未特化的generic template不是concrete type，可以在`ExportHir`中携带类型化GC-free条件，但该条件不是flag。导出的param-free concrete语义类型可在`ExportHir`中复制同样完备的属性供下游类型检查；它使用export侧id，不与本Cone的`ConcreteTypeId`共享身份。

调用与值构造的泛型推导对整组实参执行固定点约束求解，不得按从左到右的一次遍历决定成败。依赖期望类型的实参可延迟到其他实参完成绑定后再检查；为每个实参产生的 desugaring 语句必须分开缓存并最终按源码实参顺序拼接，类型检查顺序不得改变运行期求值顺序。

泛型 interface 的 HIR 类型必须同时携带 interface id 与完整类型实参；实现列表同样保存已解析的 interface 应用而非裸 id。HIR 按声明点 `in` / `out` 计算子类型关系并递归校验类型参数的使用位置。MIR 为每个实际使用的具体 interface 应用建立独立的单态化 interface 实体、TypeDescriptor 与 itable 身份；不得把不同类型实参的应用擦除到同一个 interface id。

M14起，class/struct/enum/interface全部进入统一generic nominal单态化管线，并且每个declaration kind使用独立id家族：`ExportGenericClassId`/`ExportClassApplicationId`/`ConcreteClassId`、`ExportGenericStructId`/`ExportStructApplicationId`/`ConcreteStructId`、`ExportGenericEnumId`/`ExportEnumApplicationId`/`ConcreteEnumId`、`ExportGenericInterfaceId`/`ExportInterfaceApplicationId`/`ConcreteInterfaceId`。template、带完整export type argument的application与fully specialized local entity三层id在Rust类型上互不兼容；禁止继续使用`Type::Struct(StructId, Vec<TypeId>)`、`Type::Enum(EnumId, Vec<TypeId>)`、`Type::Interface(InterfaceId, Vec<TypeId>)`或`ClassId + Option<Vec<TypeId>>`混合表示declaration/application/concrete实体。每个concrete entity非可选地携带generic origin、全部concrete arguments及已替换的field/variant/base/parent/interface/member闭包；class使用ordinary/intrinsic representation sum且`gc_free=false`，struct/enum按concrete payload生成完整`gc_free`与扫描信息，interface application以`gc_free=false`形成独立TypeDescriptor与itable key identity。class/struct/enum构造推导使用同一固定点求解器；class/struct/enum参数现阶段全部invariant，interface按声明点variance检查，裸或部分nominal application在HIR诊断。

M14起，`ExportHir`为每个type parameter保存结构化interface upper-bound列表，bound中的interface id、类型实参与来源span均非可选；generic template中对有界receiver的调用保存typed `BoundCallableRef`。本Cone或下游Cone的HIR实例化必须把它解析为普通concrete callable及完整direct/virtual/interface信息；`LocalConcreteHir`不得含bound placeholder、未验证constraint或runtime witness参数。`ToString` / `Hash`只使用源码显式声明的普通interface conformance，不存在conditional predicate；conditional equality则以typed条件和派生目标保存在export侧，generic template按当前bound环境做结构化证明，fully specialized实例由HIR求出最终结论并生成具体成员，不能把未知当false/true或交给MIR尝试。

M14起，non-interface generic method使用独立的`ExportGenericMethodId`与`ExportGenericMethodApplicationId`；application结构化地携带kind-specific owner host（param-free nominal declaration或generic nominal application）与method自身实参，不能用一个无分组的type-argument Vec同时冒充两组参数，也不能假设generic method必然属于generic owner。HIR在声明处拒绝generic method的open/abstract/override/interface-slot形态，完成宿主substitution、method实参推导与全部bound检查后生成direct `ConcreteCallableId`；`LocalConcreteHir`中的origin非可选地保留generic method id、kind-specific `ConcreteClassId`/`ConcreteStructId`/`ConcreteEnumId` owner和非空method arguments，宿主实参只从该complete owner实体读取，不再平行保存一个可能为空或错位的Vec。callable reference同样必须先形成该concrete application，不能输出polymorphic closure或让MIR从expected function type补实参。

HIR concretizer必须在模板图上分别维护nominal/callable/predicate的typed instantiation state。generic callable递归SCC中，每个环组合后的owner+callable参数替换必须为identity；合法普通递归在exact concrete key处复用已经intern的`InProgress`实体，非identity环作为polymorphic recursion在HIR诊断。不得用递归深度、实例数量、worklist上限、FQN visited set或编译超时保证终止。`is`/`as`/`as?`目标也必须引用完整concrete nominal application及其TypeDescriptor，不能擦除type argument或为尚未支持的star projection制造通配descriptor。

M14还允许registry批准的core struct/class使用`@Intrinsic`省略源码representation。registry声明同时区分fixed representation与generic representation family，并完整规定declaration kind、type-parameter arity/variance/bound及声明contract。HIR必须把这类声明正规化为封闭typed `IntrinsicTypeKind`，把每个generic intrinsic application正规化为携带完整concrete参数的typed family representation，并在`ExportHir`/`LocalConcreteHir`各自的id家族中显式携带；不能表示为空字段普通类型。implements、成员与用户可见constructor仍来自源码语义表面；literal、boxing、allocation等hidden construction entry使用独立compiler entity，不进入overload候选。`Array<T>`/`MutableArray<T>`使用普通generic class application identity，旧的独立built-in array type identity必须删除；数组专用expression可以保留，但直接引用concrete class及element type。intrinsic kind/application同时确定GC-free/representation classifier所需的语义输入，具体layout仍由LIR按target生成。

此外归属 HIR 的语义工作：

- 字符串插值脱糖（spec 6.2）、`?.` / `?:` 脱糖（spec 7.3）、`for` 脱糖（spec 11.8）等；
- 把普通/挂起函数类型正规化为全局唯一的类型化 `FunctionTypeId`，完整保留挂起性、参数与返回类型；每个 lambda、匿名函数、局部函数及 managed callable reference 都有独立的类型化实体 id。AST 的 `::name` / `receiver::name` 保持中性的函数声明引用表达式，不提前编码 managed/native 运行时类别；HIR 完成双向类型检查、局部函数可见性、重载目标选择与捕获分析，再由期望类型类别直接构造 managed callable reference 或 `FunctionAddress`，不能先构造前者再转换为后者。capture 列表只允许不可重新绑定的 binding，对外层词法局部 `var` 的任何引用直接诊断。captured value type保持 concrete layout并内联进入 closure environment，不生成 hidden box/shared cell，也不把 closure 擦除为 `Any`、裸函数符号或无类型代码指针；
- 所有成员方法/计算属性 getter 的 `this` 及扩展函数的 extension receiver，在 HIR 中都是隐含的、不可重新绑定的按值参数（spec 3.3），不得用指向调用方 binding 的 place 表示其语义。value receiver 复制完整值，ref receiver 复制 ref value；后者的 HIR 类型仍是完整 ref type，不降级成 raw pointer。捕获 `this` 时捕获的就是该参数值；对 value-type `this` 执行 `addressOf(this)` 则对该 method activation 内物化的私有副本取址；
- 函数值调用解析为独立的 callable-value call target；命名调用、函数声明引用表达式的 managed resolution 与 `FunPtr` native-address contextual resolution 是三个不同的决议入口。没有期望类型时只允许进入 managed 分支；只有 spec 13.10 允许的顶层 `@NoGC` 普通函数引用能在明确的 `FunPtr<F>` 期望类型下直接解析为 `FunctionAddress`，普通函数值之间的型变转换则保留为显式 typed coercion；
- 在 callable 签名、调用目标与 override / interface 实现关系中保留 `suspend` 标志；以显式、可嵌套的上下文状态检查挂起调用只出现在挂起函数或已登记的协程构建器中，不能把“当前无函数”当作默认允许。进入顶层属性、object/companion、实例属性、delegate、`init`、构造函数/构造委托及普通属性访问器等声明自身拥有的初始化/访问体时，必须压入带原因的 forbidden context；离开后恢复调用者上下文，所以 suspend caller 的显式构造实参仍可挂起。MIR 不为这些初始化入口或属性访问器生成 frame / continuation ABI；
- 完成 FFI 注解的目标与共存检查：`@Extern` 与 `suspend` 互斥，无论 `abi` 取值为何都在 HIR 报编译错误；函数声明引用表达式的 `FunPtr` contextual resolution 同样拒绝挂起函数及一切只能产生 managed closure 的形态。该检查必须发生在单态化、closure 转换、协程变换及 extern 符号发射之前；
- FFI 类型检查必须按 ABI 分流：`classify_c_ffi_type` 只接受具有稳定 C 表示的 GC-free 类型；`classify_scoop_abi_type` 复用普通 Scoop typed ABI，并允许规范支持的 direct ref。`String` 等 ref出现在 C ABI 是诊断，出现在 Scoop ABI 不能被偷偷改写为 `PinnedPtr`、handle或 byte storage；两种 classifier及 `is_gc_free` 使用不同缓存/结果类型，不能合并成一个布尔值。extern global 只进入 C data ABI 分支，不能伪装成 Scoop ABI direct call；
- M13 的 managed callback registration是独立的类型化决议入口，不复用 `FunPtr` native-address resolution：HIR验证唯一`ForeignCallbackCore`，并仅在该core contract内部允许其deferred callback-signature参数`F`出现在`FunPtr<F>`字段/辅助签名；每个实际应用仍要求native函数类型显式、ordinary、非suspend且完全具体化，不形成通用`function` kind bound。context index和`Reusable`/`OneShot` mode必须为编译期常量，被选参数精确为`Ptr<Unit>`，全部native参数/返回值C-FFI-safe。删除context参数后得到的ordinary concrete `FunctionTypeId`是closure的真实expected type；core声明中的`callback: Any`不产生装箱。输出使用独立 `ForeignCallbackRegistrationId`，不能把 closure改写成 `FunPtr`、`Ptr<Unit>`或无类型 runtime call；
- `const val` 在 HIR 做常量表达式求值与依赖环检查；其值进入可供下游 Cone 使用的 HIR meta，不生成 runtime initializer。普通/挂起 call、构造、分配及普通属性读取均不能进入 const expression IR；
- 对非 `Unit` 块体执行组合式控制流分析，证明所有可达路径均以有值 `return` 或 `throw` 结束；`finally` 的必退出路径覆盖 try/catch 的待执行结果（spec 第 8 章）；
- 默认参数值的调用处实例化（spec 8.5）；`getCurrentSourceLocation` 在缺省参数中的常量化（spec 11.12）；
- 装箱/拆箱的插入（spec 4.4.4 的 O(1) 规则）；
- `when` 的穷尽性检查（spec 第 5 章）。

### 2.3 MIR

只接收**本Cone**的`LocalConcreteHir`以及**上游Cone的MIR meta**（见下）。其输入类型签名不得接受`ExportHir`，负责：

- 为HIR已经生成的每个concrete function/type建立MIR实体并做name mangling；MIR不执行generic type substitution，不接受`TypeParam`，也不从template生成实例体；
- 每个MIR expression（包括desugared及MIR synthetic节点）直接携带非可选`MirTypeId`；创建节点与填入类型是同一操作。LIR只能消费该类型，不得按expected/context、operand、使用点或目标storage反向重建expression type；
- 原样传播`LocalConcreteHir`中每个concrete type及enum variant的非可选`gc_free` flag。MIR自身创建closure/frame/adapter等synthetic concrete type时，创建该实体的同一操作必须同时生成完整flag；不能先留下缺失值再由全模块扫描补齐。后续enum布局、NoGC边界与FFI classifier消费这些已定稿flag，不得分别重做一套“是否间接含ref”的递归判断；
- 为每个 call 标注 virtual / interface / direct call 类型；
- 方法/扩展调用必须先以 receiver value 初始化完整类型的隐含 receiver 参数；MIR 不得让方法体持有调用方 binding place。该规则对 value/ref receiver 相同，只是参数值的类型与 layout 不同。后续可以做 copy elision，但若 value receiver 的 `@InteriorMutable`、`addressOf(this)` 或其他 unsafe 操作可观察存储，则必须保留独立的 method-local storage；
- 为每个具体类型建立 vtable / itable（见 2.9）。本 Cone 的类型实现上游接口或继承上游类时，表结构、槽位布局与 TypeDescriptor 符号取自上游的 MIR meta；
- 把结构化 HIR 降为类型化 CFG：调用从嵌套表达式中按源码求值顺序正规化出来，控制边与异常 unwind 边显式化，`try` / `catch` / `finally` 的 cleanup 路径在 MIR 固定。LIR 只负责把这些边映射为目标相关的 landingpad 结构；
- **concrete HIR输入 → closure转换 → foreign callback adapter → 协程变换**：输入中的所有callable与function type已经concrete；MIR再把lambda、匿名函数和callable reference转为显式closure类、invoke body与按值capture字段，把函数类型型变转为typed adapter closure；局部函数的direct call可以使用带显式不可变capture参数的lifted function，取得callable reference时才物化closure。每种synthetic closure/adapter都有独立实体id、TypeDescriptor、完整`gc_free`属性与递归引用扫描描述；captured value type直接使用其concrete layout，不得用FQN、`Any`、identity-bearing box、opaque environment或统一函数指针回退；
- closure 的首字段语义上是由编译器控制的 invoke entry，其余字段按确定顺序保存 capture。普通 closure invoke 使用 `(closure, source args...) -> R` 的 managed ABI；绑定 virtual/interface reference 的 invoke body 必须在调用时执行动态分派。已知不逃逸或立即调用的 closure 可以在后续优化中消除，但 MIR 的未优化基线必须先有完整、可扫描的结构；
- 对每个实际使用的 `ForeignCallbackRegistrationId`，MIR在closure conversion之后生成独立的 typed storage adapter：输入为closure ref、删除context后的C-FFI-safe args storage、result storage与exception root slot，内部按 concrete `FunctionTypeId` 调用closure。adapter在返回C前catch-all、物化managed异常并返回typed status；adapter、registration和M12静态NoGC callback bridge使用三类不同实体/id；前者是可分配、可GC、内部可抛但对C nounwind的managed入口，不能误标为NoGC；
- 对 closure 转换后的每个 concrete suspend callable，把 CFG 在挂起调用后切分为恢复状态，计算跨挂起点活跃的值并生成堆上 frame；每个挂起点生成与其结果类型精确匹配的 `Continuation<T>` 实际类型。frame、continuation adapter 与内部完成结果均有独立的类型化 id、TypeDescriptor 与递归引用扫描描述，不允许用 FQN 或 `Any` 作为实体/结果回退；
- suspend callable 的内部 ABI 是 `(source args..., Continuation<R>) -> CoroutineStep<R>`，其中编译器内部值 enum `CoroutineStep<R>` 只有 `Completed(R)` / `Suspended` 两个变体。立即完成走 `Completed`；返回 `Suspended` 后只能由传入的 continuation 恢复。该 ABI 只用于编译器生成的 Scoop 托管调用，不用于 extern 声明或 `FunPtr`，MIR 不生成 FFI wrapper。变换完成后 MIR output 不再含源码级 suspend callable 或 suspend call；
- extern call保留类型化的 ABI/effect：C ABI 进入独立 native bridge实体；Scoop ABI 直接保存普通 managed call目标签名及 direct-ref参数，不生成 C storage bridge。两类实体 id不能混用，也不能在 MIR 后续通过 symbol string重新猜 ABI；
- `LocalConcreteHir`交给MIR的intrinsic已经是封闭typed kind，compiler-generated异常已经通过完备`CompilerExceptionCore`给出类型与constructor callable；MIR不得按intrinsic/runtime symbol、class short name或FQN检索实体，也不得以panic/fallback补齐缺项；
- 恢复失败在对应恢复状态入口重新注入为 `throw`，沿原调用点的 unwind / cleanup 边传播。挂起不是作用域退出，不能触发 `finally`；跨挂起的 pending return / exception / cleanup 动作必须作为有判别的 frame 字段保存，不能依赖未初始化槽或原生栈状态；
- M13 将编译器生成的safe continuation adapter和frame lifecycle改为类型化原子状态机：MIR显式表示claim/completing/latched/consumed及`running/suspended/completed`转换，使用acq_rel CAS取得唯一完成/驱动权，并保留payload store-before-release、acquire-before-read关系；不得把这些操作伪装成普通字段读写或按函数名留给codegen猜测；
- 把 `when` 模式匹配降级为 decision tree / 跳转序列；
- 输出 MIR type/function list，其中不再包含任何 generic 和 suspend（诊断信息除外）。

**MIR meta**：每个 Cone 的 MIR 同时输出一份 metadata，随 `.slib` 导出（见 2.6），内容包括：各单态化实例的符号、generic来源与具体类型实参，各导出类型的 vtable / itable 结构、TypeDescriptor 符号与 name mangling 结果。类型布局不在其中——布局由 LIR 生产、经 LIR meta 导出（见 2.4）。`ExportHir` generic template、`LocalConcreteHir` concrete实例与MIR实体是三类实体，分别使用不同的类型化id；跨层关系只能通过显式typed mapping表达。

### 2.4 LIR

接收**本 Cone** 的 MIR output，以及**上游 Cone 的 LIR meta**（见下），负责：

- 为每个 type 生成布局信息（struct/enum/tuple 布局、typed intrinsic fixed/family representation、`@CLayout` 的 pack/align、`Option` 同构形态的 niche 编码——spec 7.4），并生成布局完备的递归引用扫描描述：普通引用偏移、同起点 sequence、数组元素子扫描。intrinsic type按typed representation sum穷尽lower，不按FQN、空字段或使用它的operation识别；generic Array/MutableArray分支直接消费representation携带的concrete element type，并在函数lowering前为每个application建立唯一typed `ArrayTypeId`及完备的kind、element storage、stride/alignment、element scan和TypeDescriptor记录。所有数组指令显式携带该id；LIR/codegen不得扫描allocation、按layout/scan去重descriptor，或从operand/result type反推element/kind。tagged enum直接消费MIR已定稿的逐variant `gc_free` flag：pure-value variant复用一个payload区，每个非GC-free variant分配独立连续slot；LIR不得重新递归判定“是否含ref”。完整值的构造先清零再写active payload，所有独占slot中的ref leaf可直接合并为固定偏移，LIR/runtime不得保留按tag分派的扫描路径。需要上游布局的场景：本 Cone 的类继承上游类（继承字段的偏移）、跨 Cone 嵌套的值类型（上游 struct/enum/tuple 嵌入本地类型、作为数组元素、按值传参的 ABI）；
- statepoint 的落地形态（M9 定稿）：LIR 保持 statepoint 无关的指令形态，由 codegen 给每个 function 设置 GC strategy（`statepoint-example`）并执行 `rewrite-statepoints-for-gc` pass，同时在函数入口与循环回边插入 safepoint poll（详见 codegen 的实现与注释；statepoint 的"插入职责在 LIR"是早期表述，以此为准）；
- C ABI extern降为 GC-leaf storage bridge call；Scoop ABI extern降为使用普通 Scoop typed signature的 managed direct call，direct ref进入 statepoint root集合。只有 C ABI bridge拥有 C type tree/`@CLayout` static-assert描述；Scoop ABI不得经过 byte storage bridge，也不得插入隐式 pin/unpin；
- M13 启用多 mutator时，LIR必须把managed ref、raw data pointer、code pointer与metadata pointer保留为不同pointer provenance，直到root liveness/codegen完成；不能因LLVM最终都使用opaque `ptr`而提前合并。outbound C ABI及可能无界执行的NoGC区间发布live caller-root leaves并进入native-safe，持有direct ref的Scoop ABI native调用进入native-borrowed；`ManagedSafepoint`、`NoGc`、`NativeSafe`、`NativeBorrowed`必须是不同call effect，不能只用一个`GcLeaf`布尔值表达。caller-root frame以可寻址value storage加递归scan descriptor表达含ref aggregate并spill live SSA temp；tagged enum使用spec 7.4的固定ref偏移，不读取tag。Scoop ABI含ref返回槽在push前清零并一并发布，native返回后在leave可能park之前写入、恢复managed后reload。未初始化slot/raw pointer不得混入；
- 每个LIR direct/indirect/runtime/extern call及invoke共享同一typed call模型。void/direct/indirect-result分别使用互不兼容的target/signature id家族；每个target实体原子地包含destination、layout完成后的完整signature/return convention及上一条的四分effect，callsite variant再结构化地要求无result、必需out或必需result storage。不能使用`out: Option<_>`或另放一份可能不一致的result scan；dispatch slot保存typed callable与对应target/signature。codegen不得根据symbol、实参或result temp推断/创建函数声明；
- LIR中的pointer null必须携带`Managed`/`Raw`/`Code`/`Metadata` provenance；`FunPtr` null、managed niche与metadata链尾不能合并成默认Raw null。layout、well-known String、TypeDescriptor parent/interface及dispatch entry全部通过typed id/ref连接，display/link symbol只用于最终发射，不作为semantic identity；
- foreign callback adapter作为managed entry保留GC strategy/statepoint，C trampoline、signature descriptor与删除context后的args/result storage描述作为独立反向bridge数据交给codegen。continuation原子状态降为显式`AtomicLoad/Store/CompareExchange`或等价LIR指令并携带内存序；
- 把 MIR 已显式化的 normal / unwind / cleanup CFG 机械映射为 landingpad + personality function；`throw` 接到 runtime 入口。对 suspend function 的 handler，进入可挂起的 Scoop catch/finally 代码前必须把 ABI 异常物化为 managed 对象并结束原生 catch，任何原生 EH 状态都不得跨挂起点；
- 输出 LIR type/function list，不再包含任何 Scoop 特有的内容，可以机械翻译成目标 IR 或其他格式。

**LIR meta**：每个 Cone 的 LIR 同时输出各导出类型的布局与递归引用扫描描述，随 `.slib` 导出（见 2.6）。布局及扫描描述只在定义它的 Cone 计算一次，下游直接消费、不重算，保证全程序一致；结构上不能用可缺失字段把 tagged enum 或聚合元素的扫描责任推迟给 codegen。

### 2.5 codegen

只接收**本 Cone** 的 LIR output，负责：

- 将 LIR output 机械翻译成目标 IR（本阶段为 LLVM IR），然后用 LLVM 编译成 `.o`；
- 生成每个具体类型的 `TypeDescriptor`（runtime spec 2.2：类型标识、实例大小、递归引用扫描描述、父类型与普通vtable/itable）；`Any`没有方法或固定槽，TypeDescriptor不含通用equals/hash/toString入口；
- 展开登记表中归属 codegen 的 `@Intrinsic`（见 2.10）；
- 普通（非 suspend）extern 声明的符号发射与 calling convention 属性（spec 13.4）、`addressOf` 的 lvalue 语义（spec 13.10）；C ABI生成/编译 storage bridge，Scoop ABI直接发射 managed external call并接受 direct ref。codegen 依赖 HIR 已排除 suspend extern，不识别或发射 hidden continuation FFI ABI。
- M13 按`(C signature, context index)`为实际导出的 managed callback生成/复用静态 C ABI trampoline：按真实C签名编组storage、排除context参数、携带signature descriptor调用runtime callback gateway；trampoline地址与runtime-owned opaque cookie配对使用，不生成每个closure实例的可执行代码。异常时trampoline按C返回类型产生全零值。foreign-thread attach/detach、generation cookie、异常handle及token retain/release由runtime实现，codegen只消费LIR中已定型的adapter/trampoline描述。多mutator下的card mark必须发射atomic monotonic store/RMW；

codegen **不需要任何上游 meta**：上游信息已逐层吸收进本 Cone 的 LIR（布局经 LIR meta、符号经 MIR meta），对上游函数/TypeDescriptor 的引用一律发射为外部符号，链接期解析。两个链接层规则：

- **重复实例去重**：不同 Cone 可能各自单态化出同一个实例（如两个 Cone 都实例化上游的 `foo<Int>`），同名实例符号必须以 `linkonce_odr` / COMDAT 形式发射，由 linker 去重（前提是 name mangling 全程序一致）；
- **符号可见性**：`internal` 符号可本地化，但被导出的泛型体 / 默认参数表达式引用的 `internal` 符号必须保留可链接名字（spec 8.5、12.5）。

### 2.6 `.slib` 打包

把 codegen 产出的 `.o` 与 metadata 打包成 `.slib`（spec 12.5）。metadata 包括三层：`ExportHir`（供下游HIR，既含导出的concrete语义接口，也含generic template及其依赖闭包）、MIR meta（供下游MIR，见2.3）、LIR meta（供下游LIR，见2.4）。`LocalConcreteHir`是本Cone内的瞬时stage输出，绝不写入`.slib`，下游也不能反序列化它。reader对三层metadata分别返回不同的输入类型，不提供把export id直接转换成本地concrete id的无检查接口。

### 2.7 build driver

不属于编译器 stage，但为必需组件：

- 依赖图解析与无环检查（spec 12.3）；
- 上游 `.slib` metadata 变化时触发下游重编译（spec 12.5）；
- 调度各 Cone 的编译与最终链接。

driver还拥有intrinsic声明的authority配置，默认且生产模式只能是`CoreOnly`。driver为sysroot及每个编译输入provider分配不可由源码伪造的typed `IntrinsicProviderId`；M17接入多Cone后该id显式映射到Cone identity，不使用路径或包名充当身份。compiler unit/golden/fixture测试可通过内部`CompileOptions`传入`AllowListedForTesting { providers: Set<IntrinsicProviderId> }`：只对列出的provider跳过“必须来自sysroot”检查，不授权其依赖或其他输入，也不放宽registry name、target、shape/signature、annotation共存及provider唯一性。该配置不来自源码、环境变量、`Cone.toml`或稳定CLI，并且不写入HIR作为可影响语义的bool；intrinsic实体本身仍携带定义provider的typed relation，测试provider产出的`.slib`只能在消费方显式授权同一provider时加载，普通编译拒绝。

### 2.8 linker

收集所有 Cone 编译产生的 `.o`，与 runtime lib 链接，生成可执行程序。

### 2.9 虚/接口调用分派

- MIR 为每个具体类型建立 **vtable**（类层次分派）与 **itable**（接口分派）；标注 call kind 时，virtual / interface call 的 target 指向对应 table entry，direct call 指向具体函数符号。
- 表的内容由 MIR 定义，由 codegen 以数据形式发射，并从 `TypeDescriptor` 引用：TypeDescriptor 内嵌 vtable 指针与 itable 数组（见 runtime spec 2.2）。vtable从slot 0开始只包含真实virtual成员并允许为空；`Any`无成员，不预留equals/hash/toString前缀。`ToString`/`Hash`及interface operator equals走普通itable，open class equals走普通vtable。
- **装箱值类型的 this 调整**：值类型装箱后对象为 header + payload。vtable / itable 中对应装箱值类型的表项指向 MIR 生成的 **adjust thunk**；thunk 语义上从 payload 读取值，并用它初始化真正成员函数的按值 `this`。只有在不可观察的情况下，codegen 才可把它优化成对 payload 地址做 header 偏移后直接传递；需要取址或存在 interior mutability 时必须先复制到私有临时存储。
- **跨 Cone 的槽位识别**：初版 itable 采用（接口 TypeDescriptor 指针 → 方法表）的键值查找，调用点按接口 TypeDescriptor 地址查找，不需要跨 Cone 的全局槽位编号；槽位编号等优化留待后续。
- **interface没有Self/object-safety分类**：`Self`不进入AST/HIR内建type kind；若源码使用该标识，只走普通名称解析。每个合法`ExportInterfaceApplicationId`都能形成普通reference type与对应concrete TypeDescriptor/itable identity，不输出`object_safe`、`requires_sized_self`或类似flag。interface方法签名只能引用普通类型及interface宿主参数；HIR在声明处拒绝方法自身的type parameter，因此每个合法method都有固定itable slot/signature，经interface或bounded receiver均可调用，不能接受声明后再按receiver形态拒绝。未来开放method-level generic interface member前必须先定义跨Cone specialization/itable ABI；
- **其他泛型成员函数不参与虚分派**：class上的泛型成员必须为final，值类型成员本来即为final；open、abstract、override、覆盖既有virtual slot或充当interface实现都在HIR定义检查拒绝，因此所有合法调用一律标为direct。泛型宿主的参数与方法自身参数使用export侧不同typed id空间，单态化键按“宿主参数在前、方法参数在后”携带完整实参，并生成不再含该参数空间的`ConcreteCallableId`；不能按名称或裸索引拼接两组参数，MIR也不能通过跳过table insertion来掩盖一个上游仍标为virtual的非法组合。
- **结构化表达式降级**：AST 在表达式位置直接表示 `if` / `when` / `try`。HIR lower 先确定所有正常分支的共同结果类型，再分配一个类型完备的隐藏结果 local，在每个可正常结束的分支尾写入该 local，并把原有 HIR 结构化语句追加到表达式的 desugaring sink；表达式本身成为该 local 的读取。结果为 `Unit` 时无需结果 local，但仍须保留分支尾表达式的求值。HIR 保留这些结构化控制语句；MIR lower 在内部完成结构化控制降级后，统一输出基本块 CFG，不新增内嵌 CFG 的表达式节点；LIR 只接收该 CFG。

### 2.10 intrinsic 的分阶段处理

不同的 intrinsic 在其**信息就绪的最早 stage** 展开，而不是集中在 codegen：

- 编译器内置一张 **intrinsic 登记表**：`name → （typed intrinsic id、合法annotation target、展开stage/representation owner、完整声明shape/signature约束）`。type target的shape还必须以非可选结构给出declaration kind、type-parameter arity/variance/bound及fixed/generic-family representation；`Array<T>`/`MutableArray<T>`均要求一个无bound、invariant参数。target包括function/method及M14的compiler-represented struct/class。`@Intrinsic("name")`的字符串只存在于AST/HIR验证入口；name必须在表中且provider通过2.7的authority策略，否则HIR报编译错误。验证后的`ExportHir`/`LocalConcreteHir`保存封闭typed id/kind/application，不继续携带name字符串。
- **检查与展开分离**：无论在哪一阶段展开，类型检查、`value` / `ref` 约束检查、注解共存检查一律在 HIR 完成（与"所有编译期错误在 HIR 报告"一致）。
- 晚于HIR展开的intrinsic在`LocalConcreteHir`及后续IR中以“完全实例化的typed intrinsic/call target”形式存在；其type argument已全部确定，因此不构成generic节点，也不对MIR输入“不含generic”的不变量设例外。link symbol只能是该typed target的发射属性，不能反过来决定intrinsic kind或函数签名。
- 每个 intrinsic 恰好展开一次；其后的 stage 只看到普通 IR。

初始分类：

| 展开 stage | intrinsic | 理由 |
|---|---|---|
| HIR→LIR typed contract | intrinsic type declaration | HIR把源码nominal surface绑定到`IntrinsicTypeKind`；MIR原样传播typed kind，LIR按target穷尽生成representation/layout，不存在按名字展开的单一stage |
| HIR | `current_source_location` | 编译期常量，直接折叠为 `SourceLocation` 值实例（spec 11.12） |
| HIR | `Ptr` / `FunPtr` 构造、`ptr_*` / `address_of` / `size_of` / `align_of` 的源码调用 | 在 core contract 验证、重载决议和 unsafe / lvalue / 类型约束检查后正规化为类型化专用节点，不把 intrinsic 函数名传给下游 |
| HIR | `foreign_callback_register`及callback token retain/release/state/failure | 验证`ForeignCallbackCore`后直接生成类型化registration/token操作；registration以native函数类型和context index派生managed closure expected type，不按`Any`普通调用lower |
| LIR | `size_of` / `align_of`、`ptr_load` / `ptr_store` / 指针算术、`address_of` | 依赖 2.4 的具体布局以及 address-taken local / parameter 的稳定存储；降为布局常量、带对齐的 raw memory 指令和局部地址 |
| codegen | 算术/位运算（`int_add` 等）及 LIR raw pointer 指令 | 直接映射到目标 IR 指令；此时不再按 intrinsic 名称分派 |

### 2.11 IR输出完备性与typed identity

本节是所有stage的结构性约束，而不是debug assert约定。只要某项是下游正确转换所必需的信息，上游输出类型就必须使它非可选且类型化；禁止由下游根据context、arena内容、名称/符号、容器长度或默认值猜测/补齐。

- **expression type**：HIR与MIR的每个expression都直接携带非可选type id，包含Unit/Nothing、synthetic、box/unbox、array/enum、closure/callback/coroutine节点。LIR lowering不得提供`expr_ty`反推器或用expected type替代缺失类型；
- **function type canonical identity**：每个`FunctionTypeId`实体直接保存唯一`canonical_type: TypeId`，interning保证二者一一对应。Export与LocalConcrete使用各自id家族；消费者不得扫描type arena寻找反向映射或重新intern同形type；
- **compiler core identity**：只有编译器会脱离普通源码引用主动构造或调用的well-known实体，才在HIR一次验证并输出非可选typed结构。intrinsic type与function intrinsic分别使用封闭typed kind；provider authority只影响声明是否可进入HIR，不成为输出中的可选语义。`CompilerExceptionCore`至少完整给出`Throwable`及编译器主动生成的`UnwrapException`、`ClassCastException`、`ArithmeticException`、`IndexOutOfBoundsException`、`IllegalStateException`类型/constructor。缺项不能进入MIR。普通core能力不得因为由intrinsic type实现就建立专用旁路：`ToString`、`Hash`、operator equals、`print` / `println`及其实现全部使用普通interface、method、generic callable、conformance与call target实体；新增可由Scoop表达的intrinsic-type能力只修改core源码，不增加`FormattingCore` / `HashCore` / `EqualityCore`之类的编译器结构；
- **call**：LIR call/invoke必须统一引用typed target；target原子地保存destination、完整signature与`ManagedSafepoint`/`NoGc`/`NativeSafe`/`NativeBorrowed` effect。signature在layout完成后明确全部参数、calling convention及return convention；void/direct/indirect-result使用不同target/signature id家族和对应callsite variant，使缺失/多余out、storage或scan在类型上不可构造。direct、runtime、extern及dispatch slot遵守同一规则；codegen只机械声明/调用，不按symbol或callsite推导签名；
- **pointer provenance**：所有LIR pointer value（包括null）都携带`Managed`/`Raw`/`Code`/`Metadata` kind；不同kind间转换必须是显式instruction。opaque LLVM `ptr`不构成在LIR中擦除kind的理由；
- **layout/metadata连接**：layout、well-known String layout、TypeDescriptor parent/interface、vtable/itable entry及callable全部使用typed id/ref。跨Cone实体用区分local/external的typed引用，link symbol只是external entity的发射属性；名称只用于显示/诊断；
- **结构上排除非法组合**：read-only/mutable native global使用互斥access variant并分别携带完整bridge id；foreign callback有结果/无结果operation使用不同variant；caller root只能持有无法表示空扫描的`NonEmptyRefScan`；tagged enum field把type与offset放在同一`EnumFieldRepr`，不得使用平行Vec；call/function return不得使用`Void`/`Option<out>`一类可矛盾组合。

typed id按实体类别隔离：type、function type、generic function、fully substituted generic request、concrete function、runtime function、extern declaration、dispatch slot、layout与TypeDescriptor不能共享裸整数或type alias。跨stage映射必须显式输出typed relation；FQN、short name和link symbol都不能作为id失败后的回退。

## 3. 待明确事项

1. **异常穿越 FFI frame 的最终规则**（runtime spec 第 5/9 章的 TBD）。
2. **后续 GC 演进**：M9/M13基线为单代、非移动Immix与保守managed栈扫描；精确stackmap消费和moving evacuation已排入M15。分代/晋升及parallel/concurrent collector仍需另行设计，并与runtime spec 3.6的屏障契约同步。
3. **Windows 异常**（catchpad）与调试信息（line table 等）留待后续。
