# M16 设计：统一约束系统与重载决议

版本：0.1（草案）

对应`docs/ROADMAP.md`的新M16。目标是在不增加新调用语法的前提下，替换M3/M7以来分散的generic inference、单候选直降、overload applicability、lambda postponed check与M14 bound检查路径，建立一个供所有源码callable和构造表达式使用的统一HIR调用决议内核。

M16完成的是**当前已经进入语言的类型系统**：完整concrete generic application、interface声明点variance、函数类型variance、kind/interface bound、值类型装箱、lambda/callable reference与non-virtual generic method。use-site/star projection、capture conversion、context parameter和定宽整数字面量widen尚未进入当前类型系统，不在M16伪造半成品；以后只能扩展本里程碑建立的constraint relation，不能另建旁路resolver。

## 0. 关键决策与范围

- 所有命名函数调用、成员/扩展调用、函数值调用以外的callable reference resolution、class/struct构造、enum variant构造及当前已经实现的operator调用共用同一套候选、约束和诊断框架；
- 每个候选使用自己的fresh inference variables、参数映射、postponed argument状态与诊断trace。不得先按某个候选降低实参，再把结果套到其他候选上；
- generic applicability与MSC是两个constraint system：前者回答“本次调用能否使用这个候选”，后者回答“声明A能否把本次提供的参数转发给声明B”。MSC不得比较本次调用已经推断出的concrete arguments；
- single-candidate调用不再绕过overload模块。候选唯一只意味着无需MSC，不意味着可以跳过映射、bound、postponed argument、expected type或统一诊断；
- solver内部允许存在尚未固定的临时变量，但成功结果必须一次性给出唯一callee、完整concrete type arguments与全部实参类型；不允许把`Option<TypeId>`、待验证bound或“稍后选callee”输出给HIR消费者；
- M16维持当前精确位置实参数量；M17加入命名/default/vararg。M16的solver消费候选已经完成的semantic argument map，使M17只替换“源码实参如何映射”的前置步骤，不改写constraint与MSC内核；
- M16不改变源码求值顺序，也不提前产生MIR/LIR新调用形态。所有candidate probe都是编译期事务，不执行或复制源码副作用。

## 1. 现状问题与退役路径

当前实现存在四条彼此不完全一致的路径：

1. M3/M14固定点推断按`Vec<Option<TypeId>>`逐步绑定type parameter，主要处理等式式的nominal结构；
2. M7 overload先为每个候选推断一次concrete arguments，再比较替换后的parameter types；这不是fresh-variable MSC；
3. 只有多个同层候选时进入`overload`模块，单候选保留旧的arity/type diagnostic直降路径；
4. lambda、`None`、空数组、nested generic constructor及callable reference各自维护expected-type postponement入口。

这些路径在简单调用上结果一致，但加入generic owner + generic method、function/interface variance、默认参数与vararg之后会出现结构性分歧。M16退役以下假设：

- `arg_count == param_count`可以在候选外统一检查；
- 同一个source argument对所有候选都具有相同期望类型和lowering结果；
- 推断后的`T = Int`可以代表generic声明本身参与MSC时的具体程度；
- 单候选失败可以由callee-specific旧代码报错而无需候选trace；
- generic type argument是一个可按owner count切分的无类型`Vec`。

旧M3/M7/M11/M14测试继续作为兼容基线，但golden应改为新的typed call result；旧resolver代码必须删除，不能保留为“fast path”或失败fallback。

## 2. 统一调用决议模型

### 2.1 调用入口与候选视图

HIR把所有声明侧目标转换为只读的`CallableView`。它不是新的全局实体identity，只是对既有typed id的统一查询结果：

```rust
struct CallableView {
    target: CallableSource,
    receiver: ReceiverShape,
    owner_parameters: Vec<TypeParameter>,
    callable_parameters: Vec<TypeParameter>,
    value_parameters: Vec<ValueParameter>,
    return_type: TypeId,
    effects: CallableEffects,
    dispatch: SourceDispatch,
}

enum CallableSource {
    Free(ExportFunctionId),
    Local(LocalFunctionId),
    Method(ExportMethodId),
    Constructor(NominalConstructorId),
    Variant(EnumVariantConstructorId),
}
```

示意名称不要求与最终代码逐字一致，但必须满足：

- free/local/method/constructor/variant使用互不兼容的source id；
- generic owner参数与callable自身参数结构化分组；
- receiver、dispatch、suspend/effect及return type是必需信息，不从target name或宿主arena反推；
- constructor/variant的“返回类型”是完整nominal application，不能用名称加待推断实参表示。

函数值调用不是声明重载：callee表达式先定型为唯一`FunctionTypeId`，随后按函数类型的精确位置参数检查并产生`CallableValueCall`。它可以复用constraint relation和参数适配，但不进入声明候选层或MSC。

### 2.2 候选层与预过滤

候选层按spec 8.6及当前可用作用域构建：

- 无显式receiver：可见局部函数 → 隐含`this`成员 → 当前编译单元顶层 → core隐式导入；
- 有显式receiver：真实成员 → 当前可见extension；
- constructor/variant由类型或变体名称解析直接建立自己的候选层；
- callable reference按M11的managed/native上下文先分支，再在对应类别中建立声明候选；`FunPtr`分支不会混入managed closure候选。

每层先执行不需要降低表达式的shape filter：

- 显式type argument数量是否与callable自身参数数量一致；
- receiver类别、static/bound形态是否允许；
- M16的source argument count是否与source value parameter count精确相等；
- callable reference目标能否形成期望的ordinary/suspend/native类别；
- M17加入的参数名、spread及vararg结构检查由同一filter接口返回候选自己的argument map。

随后在该层分别运行applicability。取第一个至少有一个**可应用候选**的层；仅有同名但shape/type/bound都不适用的高优先级声明不会遮蔽下一层。各失败层仍保留trace，以便最终没有任何候选时报告最相关原因。多Cone带来的显式import、同package、star import和re-export只增加层，不改变filter/applicability/MSC。

### 2.3 候选自己的参数映射

M16的源码只有精确位置实参，但仍先产生候选专属的semantic map：

```rust
struct CandidateArgumentMap {
    receiver: ReceiverInput,
    parameters: Vec<ParameterInput>,
    source_order: Vec<SourceInputId>,
}

struct ParameterInput {
    parameter: ValueParameterId,
    inputs: NonEmptyVec<SourceInputId>,
}
```

M16中每个parameter恰有一个input；`NonEmptyVec`表达当前完成形态。M17将`ParameterInput`升级为封闭sum：explicit single、default template、vararg parts/empty/default，但solver继续从每个input与parameter type生成约束，不接触名称重排算法。

receiver是独立字段，因为它参与extension generic inference与MSC，却不属于源码圆括号中的value parameter，也不能被default/vararg替代。`source_order`只用于最终commit winner时发射求值顺序，不能拿parameter顺序反推。

### 2.4 Fresh variable与constraint种类

每次probe候选都创建独立的`InferenceSessionId`；候选拥有的每个未显式固定type parameter得到唯一`InferenceVariableId`。owner参数若已由receiver完整application确定则作为concrete binding进入，未确定的constructor host参数也创建自己的owner变量。两组id不能互换。

constraint至少包含：

- `Equal(A, B)`：invariant nominal argument、显式type argument及必须完全相等的结构；
- `Subtype(A, B)`：普通实参到参数、函数返回到期望位置、interface variance与装箱；
- `CallableShape(actual, expected)`：lambda/anonymous/callable reference的ordinary/suspend、arity、parameter与return关系；
- `Kind(variable, Value|Ref)`：M12 kind bound；
- `Implements(variable, InterfaceApplication)`：M14 interface upper bound；
- `ConcreteApplication(template, arguments)`：确保constructor/variant结果形成完整nominal application。

`constrain_subtype(A, B)`按类型结构递归：

- 同一invariant class/struct/enum/Array application逐项产生`Equal`；
- 同一interface application按声明点`out`同向、`in`反向、不变等式展开；
- function type按参数逆变、返回协变，并要求suspend标志和arity匹配；
- nominal继承、interface实现及value-to-ref boxing使用现有typed关系；
- `TypeParam`只能在candidate template probe中转为当前session的fresh variable，不得越过session或进入成功输出。

当前没有union/existential/projection。求解需要某组lower bounds的共同上界时，只能选择HIR当前可表达的唯一最小解；多个互不可比较的最小解、需要尚不存在的交叉类型或无法保持value representation时均为无唯一解，不能无条件回退`Any`。

### 2.5 Applicability固定点

候选probe按以下阶段运行：

1. 建立fresh variables，写入receiver已知owner bindings及完整显式type arguments；
2. 对所有不依赖候选expected type的receiver/实参做无副作用type probe，加入`actual <: parameter`；
3. 加入kind/interface bounds并反复传播等式、上下界和结构化application约束，直到固定点；
4. 对已获得完整expected type的postponed argument进行候选局部检查；它们产生的新约束回到同一固定点；
5. 验证每个变量具有唯一concrete解，所有bound成立，所有argument adaptation可明确决定；
6. 产生`ApplicableCandidate`，完整携带source target、owner/callable concrete arguments、candidate argument map、参数/返回concrete types与所需coercions。

probe不能向module永久登记generic instance、lambda entity、closure capture、hidden local或diagnostic。每个候选使用事务式scratch state；只有winner被commit一次。上下文无关表达式的纯类型结果可以缓存，但boxing/function coercion、lambda body entity和desugaring statements必须按winner参数类型最终生成，不能复用另一个候选的适配结果。

若一个postponed lambda在不同候选下获得不同参数类型，每个probe独立检查arity、suspend及body返回兼容性。lambda返回只可淘汰不适用候选；Scoop不实现`OverloadResolutionByLambdaReturnType`，因此不能在多个仍适用且同样具体的候选之间作为额外tie-break。

### 2.6 外层expected type

M16明确区分“参数给argument的expected type”和“整个call expression的外层expected type”：

- 前者是候选applicability的一部分，用于lambda、`None`、空数组、callable reference及nested constructor；
- 后者可以在callable目标已经不依赖返回类型区分时，固定仅出现在返回结果中的type parameter，例如唯一可见的`fun <T> empty(): Box<T>`用于`val x: Box<Int> = empty()`；
- generic class/struct/enum constructor的外层expected application可以预绑定宿主参数；
- 若两个候选只有借助不同return type才能决定谁胜出，调用保持歧义。return type不进入函数签名，也不进入MSC forwarding parameter list。

实现可以在candidate session中暂存外层expected constraint，但必须在MSC前标记它是否影响了candidate之间的存留；若影响结果且候选无法由receiver/source arguments独立区分，则按上述规则报告return-dependent ambiguity，而不是静默选中。

### 2.7 最具体候选（MSC）

对同一层的每对applicable候选`A`、`B`分别检查`A → B`和`B → A`：

1. 为`A`的声明type parameters创建fresh变量并施加`A`的声明bound；
2. `B`的type parameters作为待求解变量并施加`B`的声明bound；
3. 对本次调用实际提供的每个非default参数，加入`A.param[k] <: B.param[k]`；extension receiver始终视为提供的参数；
4. 求解成功表示`A`可以把这些值转发给`B`，即`A`至少同样具体。

M16精确元数下所有参数都参与。M17按候选argument map排除该候选使用default补入的参数，并加入两个Kotlin式附加规则：更少实际default者优先，仍相同时无vararg者优先。

选择顺序：

- 唯一单向支配其他候选者胜出；
- 对互不支配或互相支配的剩余集合，非参数化声明优先；generic owner已由receiver固定但method自身非generic时，不应误判为parameterized callable；
- 应用当前里程碑已定义的附加规则后仍不唯一，报告歧义。

MSC不读取candidate为当前actual arguments求得的concrete type argument，不比较return type，不用symbol name/discriminator决定胜负，也不因某候选先被arena遍历到而稳定化歧义。

### 2.8 Callable reference

managed callable reference从期望`FunctionTypeId`取得完整参数/返回约束；每个声明候选建立自己的fresh session，目标函数的默认值与vararg调用约定不进入函数类型，因此reference始终代表完整实际参数列表。没有期望类型时仍遵循M11：只有唯一可自行确定的非generic候选可形成managed函数值。

generic callable reference必须从receiver和期望函数类型唯一确定全部owner/callable参数，再产生一个concrete callable application。若多个reference候选仍适用，不使用普通调用的“default数量”或vararg便利性；它们在函数类型中已经被抹去，不能把声明调用糖重新带入closure ABI。

`FunPtr<F>` contextual resolution复用constraint relation验证签名，但候选类别继续受M12限制；M16不得先生成managed reference再转换，也不得因统一resolver而放宽generic/suspend/member/closure地址禁令。

## 3. HIR数据边界

### 3.1 Export侧

M16不把solver session写入`ExportHir`。Export侧继续只保存完整声明语义：

- kind-specific callable source id；
- owner与callable type parameter分组、结构化bounds；
- receiver、value parameter、return、suspend/effect/dispatch信息；
- callable reference及bound member实例化所需的typed dependency relation。

所有字段都足以由下游HIR创建自己的candidate session；下游不能读取上游某次调用推断出的bindings，也不能复用本Cone的arena index。M17增加parameter calling-shape和default template，不改变这一消费者方向。

### 3.2 LocalConcrete侧

成功commit产生结构完整的concrete call：

```rust
struct ResolvedCall {
    target: ConcreteCallTarget,
    owner_arguments: ConcreteOwnerArguments,
    callable_arguments: ConcreteCallableArguments,
    arguments: Vec<Expr>,
    return_type: TypeId,
    evaluation: EvaluationOrder,
}
```

实际IR可以按call kind拆成多个struct/sum，不能机械照抄一个“大而全”节点；不变量是：target、dispatch/effect、两组generic identity、参数、return type及源码求值计划均非可选且互相一致。非generic调用使用封闭的`NonGeneric`分支，而不是空Vec被下游猜成非generic。

`LocalConcreteHir`禁止出现`InferenceVariableId`、candidate、constraint、postponed flag、export id或待实例化request。MIR只消费winner及其concrete body/nominal闭包。

## 4. hir-lower模块划分

现有`overload.rs`不继续增长为单文件总控。建议按职责拆分：

- `call_resolution/candidates.rs`：作用域层、typed source view与shape prefilter；
- `call_resolution/arguments.rs`：M16精确位置映射；M17在此加入命名/default/vararg；
- `call_resolution/constraints.rs`：constraint与fresh session数据；
- `call_resolution/relations.rs`：结构化subtype/equality约束生成；
- `call_resolution/solver.rs`：固定点、唯一解与bound验证；
- `call_resolution/applicability.rs`：candidate事务及postponed argument循环；
- `call_resolution/specificity.rs`：pairwise forwarding与tie-break；
- `call_resolution/diagnostics.rs`：失败trace排序与render；
- `call_resolution/commit.rs`：只对winner产生HIR实体、coercion及源码顺序statements。

这是一组语义边界，不要求每个文件长度相同；小而紧密的类型可以合并，但不能把candidate lookup、solver、MSC和diagnostic再次缠成一个超长模块。

## 5. 与现有功能的组合

### 5.1 generic nominal与generic method

- class/struct/enum constructor共享host inference；variant不创建独立type parameter；
- generic owner普通方法从receiver取得完整owner application；generic method另建method fresh variables；
- bounded receiver调用把`BoundCallableRef`作为声明约束来源，winner concrete化后不保留witness；
- generic recursion仍按M14的typed substitution SCC规则验证，resolver只记录winner实例化边。

### 5.2 lambda、closure与suspend

- ordinary/suspend expected function type严格分离；候选不能借expected type把普通lambda静默变为suspend；
- candidate probe不提交capture；winner commit才执行一次capture analysis和closure entity创建；
- suspend call的合法上下文检查发生在winner明确后、MIR coroutine transform之前；候选的suspend标志仍参与signature applicability，但不成为仅凭挂起性重载的依据。

### 5.3 operator与intrinsic

M14的`==`先求值lhs/rhs一次，再把typed values交给普通member candidate入口；operator-specific签名合法性在声明期完成，调用期不另建equals resolver。M16不顺带开放其他operator名称，但未来operator expansion必须进入同一入口。

compiler intrinsic/core contract可以在candidate选择后把唯一typed target正规化成专用HIR节点；registry不能在overload之前按函数名截获调用，也不能跳过generic/bound/unsafe检查。

### 5.4 FFI

`@Extern`声明参与普通source overload；winner确定后再检查/携带其typed ABI与effect。unsafe context不通过隐藏候选改变MSC。C/Scoop ABI classifier只验证已经concrete的完整签名，不能反过来帮助猜type arguments。

## 6. MIR/LIR/codegen影响

- MIR call lowering只接收唯一concrete target及完整ordered arguments；删除任何按callee name、generic Vec长度或argument type重新分类target的路径；
- M16不改变MIR ABI、LIR call模型或runtime函数；差异只体现在HIR选择出更正确的callee/application；
- generic实例、closure/adapter、constructor body只为winner登记，失败候选不应出现在MIR golden或最终symbol table；
- `EvaluationOrder`在HIR body中已经由statements/hidden locals结构化体现时可不作为单独runtime字段，但dump必须能证明实参只lower一次且顺序未被constraint probe改变。

## 7. 诊断

每个candidate保留结构化失败原因：

- shape：type argument count、arity、receiver/callable category；
- inference：变量无约束、多解、冲突、无法表达的LUB；
- constraint：某个`actual <: parameter`失败及递归路径；
- bound：kind/interface bound与实际type argument；
- postponed：lambda arity/suspend/body return、callable reference歧义、expected nominal缺失；
- specificity：并列候选之间双向forwarding结果。

最终诊断按“所选最高相关层 → 源码声明顺序”稳定展示完整source signature与一条主失败原因；`--dump`/golden可显示更完整trace。禁止以debug字符串比较或遍历顺序选一个“最接近”候选。

## 8. 测试计划

### 8.1 solver unit

- equality、上下界传播、invariant nominal、interface in/out、function parameter contravariance/return covariance；
- owner/callable两组变量、显式绑定、kind/interface多bound、唯一解/无解/多解；
- 多轮固定点：后一个实参先固定变量，前面的`None`/空数组/nested constructor随后完成；
- 无可表达LUB、value/ref装箱边界与多个互不可比较interface；
- pairwise generic forwarding结果不依赖当前call推断出的concrete arguments。

### 8.2 HIR与overload

- single/multi候选走同一路径并产生一致diagnostic结构；
- local/member/top-level/core/extension层在高层无适用候选时正确继续；
- generic与non-generic、generic对generic、generic owner普通method、generic method及bounded receiver的MSC；
- lambda/anonymous/callable reference在候选expected type下postpone，失败候选不泄漏entity/capture；
- constructor/variant与普通函数使用相同推导结果；
- 外层expected type只完成唯一目标的返回参数推断，不按return type打破overload歧义；
- managed reference与`FunPtr`上下文严格隔离。

### 8.3 golden与回归

- AST无变化；HIR golden锁定完整concrete target、两组type arguments、coercion、source evaluation order；
- MIR中只出现winner实例，失败候选无symbol/body；
- negative fixture逐候选显示arity、bound、lambda及MSC原因；
- M1–M15全部fixture通过，尤其M3 generic、M6 dispatch、M7 overload、M10 suspend、M11 closure/callable reference、M12 FFI及M14 generic nominal/bound组合。

fixture建议使用`tests/fixtures/m16-call-resolution/`，solver unit按子模块放置，避免把所有case堆进一个测试文件。

## 9. 实现顺序与验收门

1. 抽取typed `CallableView`和候选层，统一single/multi candidate入口；
2. 建立fresh variable、constraint relation、fixed-point solver与结构化失败；
3. 迁移generic function/constructor/method及M14 bound；
4. 迁移postponed lambda、`None`、空数组、nested constructor和callable reference；
5. 替换MSC为pairwise fresh forwarding；
6. winner-only commit、诊断、golden与全量回归；
7. 删除旧binding/inferred-concrete MSC与所有single-candidate旁路。

M16只有在所有现有callable/constructor入口都通过同一resolver、MSC不再读取当前调用推断结果、失败candidate不产生永久HIR实体、成功`LocalConcreteHir`不含任何未解变量或export placeholder时完成。仅新增一个solver但保留旧调用路径，不算完成。

## 10. 明确不做

1. 命名参数、默认参数、`vararg`及其MSC附加规则——M17；
2. context parameters、完整operator/infix与property-like callable——后续callable语义里程碑；
3. use-site`in`/`out`、star projection、capture conversion及projection-aware LUB——后续泛型里程碑，但必须扩展本solver；
4. 定宽整数字面量类型与Widen MSC规则——随数值类型里程碑；
5. `OverloadResolutionByLambdaReturnType`、builder inference、SAM conversion、receiver function type；
6. 部分显式type argument的`_`、polymorphic function value、higher-kinded type、associated type或runtime generic dictionary；
7. 多Cone/import层本身；后续只能添加candidate source layer与读取`ExportHir`，不能替换M16算法。
