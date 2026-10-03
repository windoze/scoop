# M27 Task-local Context 设计

状态：设计完成，待规范同步与实现

日期：2026-09-07

依赖：M10 coroutine、M13 foreign-thread managed callback、M15 moving GC、M22 typed cleanup、M23-11 多 Cone 与 `.slib`总验收、M25 自有异常 ABI

## 0. 结论

M27保留Kotlin context parameters的主要源码形态，但把底层语义改成**运行期解析、动态作用域、属于logical task的Context**：

```scoop
interface DatabaseService {
    fun load(id: UserId): User
}

context(database: DatabaseService)
fun loadUser(id: UserId): User = database.load(id)

fun run(database: DatabaseService): User =
    context(database) {
        loadUser(UserId(42))
    }
```

本里程碑固定以下语义与架构不变量：

- `context(name: T)`声明一个context requirement和声明体内的不可变参数名；canonical exact static type `T`就是key，参数名不参与key identity；
- `context(value) { body }`是compiler-known结构化表达式，不是普通函数调用。它按`value`的canonical exact static type安装一个binding，body结束时严格LIFO恢复；
- M27只做**exact type match**。`context(Impl())`不满足`context(service: Service)`；调用者必须先通过变量类型、显式转换或其他既有方式把value静态定型为`Service`；
- contextual declaration进入时按声明顺序lookup一次：具名结果保存为普通不可变local，匿名项执行同样的presence check但不建立local。函数内部后来安装的同型binding不会改变本次activation已经取得的参数；
- lookup未绑定时抛普通、可捕获的`MissingContextException`。runtime不做subtype、variance、generic compatibility、反射或动态scope链搜索；
- context requirement是导出语义契约，但M27不让它参与overload selection、类型推断、普通函数类型、mangle或机器ABI；未经标注的中间调用链不需要转发hidden参数；
- binding属于logical task而非OS thread。普通调用与同一coroutine内的direct suspend调用共享TaskContext；`startCoroutine`从当前有效binding建立独立child TaskContext；
- 挂起不是scope退出。resume在frame所属TaskContext下继续，离开driver时恢复resumer原Context，真正穿过scope exit edge时才恢复binding；
- 普通closure不隐式捕获整个Context；如果它引用已声明的context参数，则只按现有closure规则捕获那个普通local。`foreignCallback` registration另行捕获binding snapshot，并为每次invocation建立独立调用Context；
- 所有保存binding、旧binding、snapshot或current TaskContext的地址都必须是GC可扫描、可重定位的managed ref/root，不能擦成`void *`或未登记TLS裸指针；
- 跨Cone key identity从`PersistentExactTypeId`确定；机器热路径使用进程内slot，但slot不进入源码语义、`.slib` semantic identity、mangle或fingerprint；
- M27不引入静态effect row、effect inference、effect polymorphism或编译期“binding一定存在”证明。

本文把约束分为两类：使用“必须/不得”的内容是语义或架构不变量；第7节的索引结构、fanout、字段布局、helper拆分与slot编码只是针对当前实现的**参考方案**。只要保持上述不变量、typed IR边界和验收结果，后续可以替换物理实现。

## 1. 源码表面

### 1.1 contextual declaration

语法：

```text
context-parameter-list ::= "context" "(" context-parameter
                           ("," context-parameter)* ")"
context-parameter      ::= (Identifier | "_") ":" type
contextual-declaration ::= context-parameter-list eligible-contextual-declaration
```

`context-parameter-list`是一个且至多一个最外层declaration prefix；重复`context(...) context(...)`不等价于合并list。它位于被修饰declaration及其既有annotation/modifier整体之前，prefix与declaration之间允许普通换行，但不能跨显式statement terminator。parser不得把非法target先接受为任意`declaration`后留给后端猜测。

示例：

```scoop
context(database: DatabaseService, logger: Logger)
fun serve(id: UserId): User {
    logger.info("loading user")
    return database.load(id)
}

context(_: Tracer)
fun tracedWork() {
    nestedContextualCall()
}
```

M27允许context parameters修饰：

- top-level、member、extension或local named function，包含当前语言原本允许的ordinary、`suspend`、generic、abstract和interface method形态；M27不借此放宽interface method-level generic、generic virtual/override等既有禁令；
- 没有backing storage、initializer或delegate的accessor-only property；一个property-level context list同时约束其全部已有getter/setter，并在每次accessor invocation入口分别取得。

M27不允许它修饰class/interface/object/struct/enum声明、constructor、`init`、lambda、anonymous function、function type、stored/delegated property、`@Extern`或`@Intrinsic`声明。constructor和native entry的所有权/调用边界不同，intrinsic又可能在HIR/LIR展开而没有唯一source implementation entry；context function type会立即把requirement带入类型与HOF规则，均不在本里程碑混入。

因此contextual abstract/interface property在M27只能由带相同context contract的accessor-only property实现；stored/delegated property、constructor property及其implicit accessor不能满足该obligation。这个限制与普通non-contextual property obligation的实现集合有意不同，必须在override诊断中直接说明，不能静默丢掉context contract。

context parameter规则：

- 参数标签在IR中使用`Named(name) | Unnamed { ordinal, span }`封闭表示。具名参数在同一list、该声明的ordinary value parameter及property setter parameter命名空间中必须唯一；`_`可重复但不建立可引用名称，诊断以稳定的源码序号标识；
- canonical exact type相同的两个context parameter非法。generic template定义处先拒绝正规化后已经相同的parameterized key recipe；不同recipe在某个concrete substitution后合并到同一exact key时，只拒绝该具体化并在调用或显式实例化处诊断；
- 类型必须在最终concrete application中是non-null managed ref。class、interface、object、Array等nominal ref application以及fully typed ordinary/suspend managed function value可以使用；struct、enum、tuple、`Option<T>`、`Ptr`、`FunPtr`和其他value/raw/code carrier不能直接使用。这里的managed function value不等于M27明确排除的“携带context requirement的context function type”；
- generic declaration可以在context type内部引用其type parameter，但Export HIR必须证明所有合法substitution都保持ref category；Export template保存typed key recipe，每个fully concrete contract application（包括没有body的abstract/interface owner application）都必须完成substitution、得到完整exact key并再次执行duplicate-key检查，implementation body随后才建立LocalConcrete entry plan；
- transparent `typealias`先展开，因此alias和target是同一个key；不同generic application是不同key；
- public/protected contract中的context type遵守9.1.5与M23的signature exposure规则，不能借context metadata泄漏不可见类型；
- context name只在函数body/expression body或property accessor body中可见，不进入receiver/type clause、annotation argument、property type或ordinary parameter default expression。M17 default在caller侧展开，而context参数到callee entry才取得；M27不建立第二套default隐式传值规则。

### 1.2 参数取得时点

context parameter看起来并表现为参数：每次进入source-level concrete function body或property accessor时，生成的prologue按源码顺序对每个required exact type lookup一次。具名结果保存为普通immutable local；`_`只完成presence check并丢弃临时结果，但binding仍由当前TaskContext正常保活。

```scoop
context(current: Logger)
fun example(next: Logger) {
    current.write("outer")
    context(next) {
        current.write("still outer")
        anotherContextualCall() // 新callee取得next
    }
}
```

这条规则带来以下确定行为：

- 某个parameter即使最终没有被body读取，declaration仍明确要求它，进入body前缺失就抛异常；
- 同一activation内的parameter identity稳定，不会因嵌套`context(...)`而隐式变化；
- ordinary closure引用parameter时，捕获的是该immutable local；不引用时不会因为创建位置而捕获TaskContext；
- suspend declaration的parameter若跨挂起存活，按普通managed local进入exact coroutine slot；
- abstract declaration及无body的interface member只保存contract，没有可执行prologue；任何实际source body（包括interface default body）都在其唯一implementation entry执行lookup。

prologue按一次source implementation activation插入，而不是按machine callable插入：this-adjust thunk、variance/callable-reference adapter、跨Cone bridge等只转入唯一body entry，不能重复lookup。suspend body也只在initial state执行prologue；resume state从frame中的已取得local继续，不重新读取当前binding。

### 1.3 exact type key

key由context requirement或binding expression的**canonical exact static type**唯一确定，而不是：

- 参数/变量名称；
- expression的运行期TypeDescriptor；
- 某个兼容父类或interface；
- typealias拼写；
- runtime generic variance或subtype搜索结果。

```scoop
interface Service
final class ServiceImpl : Service

context(service: Service)
fun useService() {}

fun examples(impl: ServiceImpl) {
    context(impl) {
        useService() // 运行时缺少Service；这里只安装ServiceImpl
    }

    val service: Service = impl
    context(service) {
        useService() // 命中Service
    }
}
```

binding body中出现哪些contextual call不能反向影响`value`的类型推断。否则binder需要知道任意深调用、FFI和separate compilation之后的requirement集合，等价于提前引入本里程碑明确排除的静态Context传播。

exact match的代价是同一exact type只能表示一个context角色。需要同时区分primary/replica database之类的同型角色时，必须声明不同的nominal wrapper或capability view；transparent typealias不足以创建新key。

### 1.4 `context(value) { ... }` scope

语法：

```text
context-scope-expression ::= "context" "(" expression ")" block
```

它是compiler-known表达式，body是词法block而不是lambda。M27一次只绑定一个value；多个不同类型使用嵌套scope：

```scoop
context(database) {
    context(logger) {
        serve()
    }
}
```

求值与退出规则：

1. `value`先在外层Context中完整求值一次并确定canonical exact static ref type；body不向它提供expected context type，整个Context scope收到的外层expected result type也只传给body/result，不传给`value`；
2. value正常完成后才安装binding；安装必须failure-atomic，body不能观察半完成状态；
3. body在新binding下执行一次，body的normal result是整个表达式结果；
4. 同exact type的内层scope shadow外层scope，退出后恢复外层值；原先unbound时恢复unbound；
5. normal fallthrough、`return`、`break`、`continue`和exception unwind只要真正离开scope，都必须恰好恢复一次；
6. 仅仅挂起不离开scope，不恢复binding；resume后仍在同一TaskContext与同一scope状态继续；
7. body中的`try`/`finally`继续使用8.7既有规则；Context cleanup只加入同一typed cleanup stack，不建立平行控制流模型。

body不建立callable或suspension boundary：位于`suspend` callable时，它直接继承所在body的挂起能力；位于ordinary callable时，它仍不能调用suspend callable。ordinary/suspend两个`context`函数overload不参与这条规则。

`context(value) { ... }`的token序列与“调用名为`context`的函数并传trailing lambda”相同。parser必须在生成普通Call/Lambda前按本形态建立专用AST；无receiver的完整形态保留给Context。`)`与`{`之间采用现有control-flow header的换行/terminator规则；qualified `obj.context(...) { ... }`及不满足本shape的标识符使用仍可按普通语法处理。

### 1.5 与Kotlin表面的有意差异

M27保留声明前缀、参数名和binding block的形态，但不复制Kotlin的call-site implicit resolution：

- context requirement不参与overload candidate applicability或MSC；两个声明若只差context list，仍是重复声明；
- override/interface implementation的context list是导出contract。每个concrete owner/callable application完成全部substitution和alias canonicalization后，required type的数量、顺序和canonical identity必须exact-match；参数名可按ordinary override参数规则不同；
- property-level context list分别投影到getter和已有setter的accessor contract，并按各自slot验证。若一个owner继承到ordinary signature相同而context contract不同的多个function/accessor slot，则该owner application冲突；不能按声明顺序选一个，也不能让单个override同时冒充两份不同contract；
- call site不静态检查当前是否有binding，也不插入隐式参数；callee entry在运行期lookup，缺失时抛异常；
- context list只枚举本declaration entry显式执行的lookup，不要求覆盖callee的context list，也不从body/call graph推导、传播或闭包化；无context list的函数可以调用contextual declaration，缺失只在后者entry动态抛出；
- M27没有explicit context argument、`contextOf<T>()`、context function type或Context-aware callable conversion；
- `context(value)`只按exact static type绑定一个key，不执行compatible-type resolution，也不接受同层多value list。

这些差异是保持普通ABI、透明中间调用链和O(1) exact lookup的基础，不是parser限制。

### 1.6 effect与ABI分类

- Context不增加ordinary/suspend function的机器参数，不改变函数类型、mangle、vtable/itable slot、C ABI或Scoop ABI；
- context list仍进入Export HIR declaration contract、override验证、`.slib` semantic fingerprint和诊断metadata。修改它是source contract变化，但不必是machine calling convention变化；
- prologue lookup可能走missing异常分支，因此contextual concrete body不是`@NoGC`；`context(value)`安装可能分配/GC，也不是`@NoGC`；二者均不得出现在release-safe、const或其他禁止managed effect的body；
- payload对象的方法仍按普通签名调用，可以是ordinary或suspend method。Context lookup本身不拥有、捕获或暴露continuation；
- context requirement不参与M16类型推断，generic type argument仍只能由ordinary args、receiver、expected type和explicit type args决定。
- contextual declaration的callable reference是相同ordinary/suspend function type；创建reference时不lookup或snapshot Context，调用该function value进入目标body时才从invocation所属TaskContext执行prologue。reference resolution与MSC同样忽略context list。

## 2. Task与动态作用域语义

### 2.1 ownership模型

Context属于logical task，不属于OS thread、native stack frame或词法closure：

```text
logical task
  └── TaskContext
        ├── exact-type bindings
        └── structured restore state

OS thread
  └── ScoopThreadState.current_task_context  // 当前执行入口，也是GC root
```

同一个TaskContext同一时刻只能由一个执行者驱动。普通函数调用、virtual/interface调用、普通closure invocation以及同一coroutine内部的direct suspend调用都共享当前TaskContext，不发生fork。

binding重绑定与bound对象自身mutation必须区分：parent和child后续`context(value)`互不影响，但继承到的managed object仍是同一个对象；其线程安全继续服从普通数据竞争规则，Context不复制对象也不提供同步。

### 2.2 execution enter与binding restore

必须区分两套操作：

- **task execution enter/leave**：OS thread开始或结束驱动某个logical task时，临时替换`ScoopThreadState.current_task_context`，离开driver后恢复调用者原Context；
- **binding push/restore**：源码真正进入或退出`context(value) { ... }`时，修改当前TaskContext中的有效binding。

因此挂起只会使当前线程leave task execution，不会pop源码binding。resume先enter frame所属TaskContext，再从挂起点继续；真正穿过scope exit edge时才restore。

execution enter必须严格LIFO。保存的previous TaskContext本身也是managed ref，必须位于collector可原地更新的root slot中；不能只放在未扫描的C local或第二个TLS裸缓存。

### 2.3 root task与启动

M23的eager initializer和最终`main`由多个独立managed gateway调用，中间C coordinator回到native-safe。M27必须在第一个eager gateway前增加受检context bootstrap：

1. program/image/context metadata先完成native validation与slot解析；
2. 主线程attach后，在仍为native-safe且`current_task_context`为空时调用唯一受限的runtime Context entry constructor。它只消费已验证的Context runtime support，以显式可回写native root保护分配中间值，完整建立empty root TaskContext后先安装到ThreadState，再进入任何Scoop managed gateway；
3. root跨全部eager gateway、gateway间native-safe区间和最终root gateway保持存活并接受GC relocation；
4. `main`完成且不再进入managed代码后，经受检teardown清空；detach验证没有残留current Context或execution entry。

initializer中的contextual call因而具有普通语言语义：没有同一initializer词法scope提供的binding时抛`MissingContextException`，而不是“没有TaskContext”的runtime invariant错误。

foreign thread attach本身处于native-safe，可以尚未安装Context；任何后续managed gateway必须先通过同一受限entry constructor安装empty entry Context或下述callback invocation Context，返回native并准备detach时恢复为空。该constructor不是可调用的Scoop body，不执行用户代码、lookup、callback或普通managed dispatch；这是允许`current_task_context == null`的唯一分配入口。

### 2.4 coroutine fork与resume

`startCoroutine`是当前核心语言中明确的logical child boundary：

- task、completion及其他ordinary实参先在caller Context中按源码顺序求值；
- 真正调用task body前，从caller当前**有效binding**建立独立child TaskContext，child restore history为空；
- task body与最终`completion.resume`/`resumeWithException`都在child Context下运行；
- immediate completion、真实挂起、恢复失败及completion自身抛出都必须最终leave execution entry并恢复调用者Context；
- caller之后退出外层`context(value)`不影响已建立的child；child重绑定也不写回parent。

每个compiler-generated resumable coroutine frame必须有非可选、immutable的`task_context` managed-ref field，并进入普通TypeDescriptor RefScan。frame发布给continuation前完成该字段初始化。每个resume adapter在M13 atomic claim成功后：

1. 从frame取得所属TaskContext；
2. enter并保存resumer原Context；
3. 驱动到再次`Suspended`、成功完成或失败完成；
4. 在全部出口leave并恢复resumer Context。

resumer当前恰好绑定同exact type不能覆盖frame Context。嵌套resume形成普通LIFO序列，例如callback Context A进入coroutine Context B，resume返回后恢复A，callback返回后再恢复其外层Context。

本规则不改变hidden continuation calling convention：TaskContext由frame持有，不给ordinary或suspend callable额外增加context参数。

### 2.5 closure

普通closure不捕获创建位置的TaskContext。两种情况必须区分：

```scoop
context(logger: Logger)
fun captureParameter(): () -> Logger = { logger } // 捕获entry local

fun deferCall(): () -> Unit = {
    contextualLog() // invocation时由contextualLog自己的prologue lookup
}
```

第一种只复用现有lexical closure capture；第二种在closure实际invocation所属TaskContext中解析callee requirement。不能仅因closure在`context(value)`内部创建，就给它添加隐式snapshot field。

### 2.6 FFI与managed callback

同步C ABI或Scoop ABI outbound调用不切换logical task。native-safe/native-borrowed区间不清空`current_task_context`；该slot继续作为GC可更新root，因此同一同步调用链返回后观察到原binding。ordinary function ABI没有hidden Context参数，foreign frame无需理解Context。

现有`foreignCallback(...)` registration是明确的Context snapshot边界：

- registration成功前捕获当前有效binding的immutable snapshot，token通过独立GC handle保活；retain共享snapshot，final release/token destruction在active lease归零后同步释放closure、snapshot与failure handle，不引入GC finalizer；
- OneShot和Reusable的每次invocation都从snapshot建立新的TaskContext，restore history为空；并发invocation不得共享可变TaskContext；
- native gateway attach线程并取得closure/snapshot handle roots后，先用受限runtime entry constructor从snapshot建立invocation Context并安装；只有安装完成后才进入generated managed adapter调用closure，并在normal、异常转换和全部失败出口leave；已有outer Context的同步重入也先建立独立invocation Context并以execution guard暂存outer；
- callback内部的`context(value)`只影响本次invocation，不污染snapshot、其他并发invocation或下一次调用；
- `MissingContextException`由既有callback catch-all物化为token failure/managed exception handle，不得展开穿越C frame。

M12静态`FunPtr` target本来就必须`@NoGC`，因而不能使用Context；M27不改变其能力。

## 3. Exact-type key、slot与artifact契约

### 3.1 三种身份不得混用

至少区分：

```text
PersistentExactTypeId      // M23既有exact type身份
PersistentContextKeyId     // 由exact type确定的Context key身份
ContextKeyRef              // 各IR crate自己的local/external typed引用
PersistentContextKeyUseId  // 一个machine-code owner对一个concrete key的materialized use
ContextSlotCellRef         // 由该use唯一拥有、与owner同linkage的cell
ContextSlotId              // 当前进程内lookup索引
```

`PersistentContextKeyId`使用独立、versioned、domain-separated canonical encoding，并非直接type alias裸digest。下面只是编码示意，实际hash/tag拼接随wire spec固定：

```text
PersistentContextKeyId = H(
    "scoop-context-exact-type-key-v1",
    PersistentExactTypeId
)
```

一对canonical exact type/key是一一对应关系，但typed wrapper保证context key、key use、cell、type、field、dispatch slot和机器slot不能混用。不得以FQN、parameter label、link symbol、host path、输入顺序或arena ordinal作为identity/fallback。

transparent alias展开到同一key；`Repository<User>`和`Repository<Order>`为不同key。context parameter名字只进入source interface/诊断metadata，不改变key。

### 3.2 context match plan

HIR对每个requirement和binding site产生非可选typed plan：

```text
DeclaredContextRequirement {
    parameter_label: Named(name) | Unnamed { ordinal, span },
    key: Symbolic { ref_type_expr, key_recipe }
       | Concrete { exact_ref_type, persistent_key },
    access/origin metadata,
}

AppliedContextRequirement {
    declaration_requirement: Local | External checked ref,
    exact_ref_type,
    persistent_key,
}

ConcreteEntryRequirement {
    applied_requirement: AppliedContextRequirementRef,
    action: BindNamedLocal(ConcreteLocalId) | CheckPresenceOnly,
}

ContextBinding = Template { value_ref_type_expr, key_recipe }
               | Concrete { value_exact_ref_type, persistent_key }
```

declaration contract不引用body local：non-generic source/export declaration持有concrete exact type/key，generic Export template持有结构完备的`ContextExactTypeRecipe`/`ContextKeyRecipe`。每个fully concrete callable/owner contract先建立`AppliedContextRequirement`并完成pairwise duplicate检查，因此abstract/interface requirement也能完整存在而没有entry action。只有LocalConcrete implementation body再建立`ConcreteEntryRequirement`并可能引用`ConcreteLocalId`。generic body的binding site也保存template recipe，不能在substitution完成前伪造`PersistentContextKeyId`。这些variant不能用平行optional字段拼接；若两个原本不同的requirement recipe具体化后合并为同一key，该contract application失败。

generic binding expression的static type同样必须由bound证明始终属于ref category；否则在template定义处拒绝，不能等某次碰巧为ref的instantiation才接受。

concrete requirement与binding只有`PersistentContextKeyId`相等时才命中。runtime不接收source type relation，也不调用`is_instance`、itable lookup或generic solver。binding expression的body、callee集合和当前已安装binding都不参与plan构造。

### 3.3 slot解析与间接cell

生成代码不得把artifact-local ordinal或最终程序某次排列直接烘成Context语义。每个实际发射lookup/binding操作的registered machine callable按concrete key产生一个`ContextKeyUse { owner_callable_body, key, exact_type_ref, cell }`；同一body中的多个site复用它。`PersistentContextKeyUseId`由稳定的typed owner body identity与`PersistentContextKeyId`共同确定。use record与cell都是该body的typed associated atom/ObjectDefinition role，并与owner body物化在同一个`LinkObject`：strong body使用对应的strong associated definition；ODR body沿用该body既有的`OdrMemberId`和materialization relation，绝不为use或cell新建第二个ODR member。程序登记期把全部合法use按key分组并解析到同一`ContextSlotId`，再向各use独立拥有的只写一次cell发布该slot：

```text
PersistentContextKeyUseId = H(
    versioned context-key-use domain,
    PersistentCallableBodyId,
    PersistentContextKeyId,
)
```

- cell初始为明确unresolved encoding；登记成功后只发生一次publish，已发布值在进程余下生命周期不改变、也不复用；
- 同一key由不同body使用、因而拥有多个cell是合法的；这些use必须引用同一`PersistentExactTypeId`，最终取得同一slot；
- 同一generic/structural callable可由多个Cone重复materialize时，其use record与cell作为该body既有ODR member的associated atom共同coalesce；所有body relocation因此指向相同semantic cell identity。final link后同一`PersistentContextKeyUseId`仍有不同record/cell地址说明ODR失败，必须拒绝；
- 同一`(owner_callable_body, key)`出现多条未coalesce use、一个cell被不同use拥有、同一use重复publish，或相同key关联不同exact type，均是稳定metadata错误；
- cell只保存GC-free机器标量，不是managed root；slot数值对源码不可观察，不进入persistent id、semantic fingerprint、mangle或diagnostic identity；
- verifier通过typed owner/use/cell、ODR relation与object relocation定位cell，不能只凭cell地址、TypeDescriptor地址或symbol碰运气；
- slot空间耗尽，或managed execution开始后仍读取unresolved cell，是稳定runtime/metadata错误，不退化成missing binding。

`ContextKeyUse`是M23 registration prefix下新增的封闭record family，沿用既有单向digest DAG而不得自创循环：canonical LIR/ObjectDefinition是上游leaf；strong use record的`definition_fingerprint`由`StrongRegistration`产生，在计算该record的normalized object bytes时把自身fingerprint slot归零；ODR use record 按 M23-7 的逐 member 规则使用自身 registration member 的 `OdrDefinitionFingerprint`，其内容通过实际 typed 引用关联 owner body；计算该 member 的 normalized object bytes 时把自身 fingerprint slot 归零。`RuntimeImage`最后消费完成的use registration。cell在object中只以canonical unresolved初始bytes参与ObjectDefinition/ODR/registration验证；启动登记写入的live slot值永不进入fingerprint、cache key或后续object revalidation。

Context没有独立source key declaration或单独的全局definition record；全局key从exact type确定，runtime materialization是上述code-owner use。generic/abstract contract若未产生concrete code，不制造cell。具体table分片与record字段布局留给wire spec；硬约束是typed关系完整、无nullable尾字段、合法的同key多use与非法的同owner duplicate可结构化区分，也不在lookup时临时解析type关系。

### 3.4 descriptor与wire版本

M23 当前实际发射的 `ScoopImageDescriptorV1` 是 exact-sized，只覆盖既有 registration 种类。M23-6 已删除没有生产用途的 program/core binding 结构；本文不再假设这些结构、其 Graph digest 或验证包装是已有 ABI。M27 实施时以届时实际运行的启动和链接格式为基线：

- image 或已实现的 program 启动数据增加 Context 字段时，按正常规则升级相应格式和 runtime ABI；不得直接在 exact-sized V1 尾部追加字段；
- 登记实际 code-owner 的 Context key use、cell 所属 callable 与必要的 ODR 关系。Context runtime support 的类型、布局、scan 和 native entry 由实际 program-link 产物表达，使用普通定义、要求和 typed relocation；
- program 级 Context 支持只生成一份，用户 Cone 不重复生成这些定义。bootstrap/teardown 由 C runtime 调用其 native entry，不为这些调用伪造 managed callable body 或 registration；
- 同步升级实际受影响的 HIR、MIR、LIR 和 `.slib` 格式。HIR 保存 symbolic requirement 和 key recipe，后续完整 IR 保存具体 key 和 Context 操作；
- 缓存 fingerprint 覆盖实际语义、布局、object bytes 和 relocation 的变化；RuntimeImage 只包含本 image 实际生成的记录。启动登记写入的 cell slot 值不参与持久身份或产物 fingerprint；
- 在实际消费边界检查类型、引用、布局、GC 和初始化顺序，复用未变化的 IR 及检查结果。删除预设的 program digest 证明链、独立回填状态机及回填后重放整份 object 验证的要求；
- Context cell 在关系解析完成后、managed initializer 运行前初始化。其字段、table 与版本在实现相应功能时按具体需要确定，不提前冻结未实现的结构。

## 4. Runtime正确性契约

### 4.1 managed ref与moving GC

无论使用何种索引结构，以下要求都是硬约束：

- effective binding只保存managed ref；内部unbound carrier必须保持`NullableManagedRef`一类provenance，不能转成Raw pointer；
- current TaskContext、binding storage、restore history、snapshot、context parameter local、scope mark/execution guard中的managed leaf和coroutine frame field全部出现在M15可扫描且可回写的root/TypeDescriptor计划中。现有C-heap callback token仍只保存`GcHandle`整数；closure/snapshot/failure对应的handle-registry object slot是可回写external root，token本身没有TypeDescriptor或RefScan；
- `ScoopThreadState.current_task_context`是显式GC root。TLS只保存`ScoopThreadState *`，不得另有collector未知的TaskContext cache；
- execution entry中保存的previous Context也由thread scanner访问并原地更新；
- 所有managed-ref store服从当前collector的checked write-barrier plan；Context安装/fork/snapshot的中间结果与binding实参在每个可能GC调用点都有完整native/compiler root，并在GC后reload。当前M15 lowering使用atomic monotonic card barrier，但具体barrier形态不是Context语义；
- restore后清掉不再需要的undo引用，使shadow前后的对象按可达性及时回收；
- lookup成功路径不得分配或触发GC；restore必须no-alloc、nounwind，可安全用于EH cleanup。安装、fork和snapshot允许分配/GC，但必须failure-atomic。

把managed ref放进`void *`数组、把TaskContext只存在TLS、依赖conservative C stack扫描、为每个slot临时创建`GcHandle`，或在GC后继续使用旧地址，都不满足M27。

### 4.2 并发与发布

TaskContext由logical task独占，同一时刻只允许一个driver修改；lookup/push/restore不应为共享可变表支付全局锁或逐slot原子操作。跨线程resume沿用M13 continuation/frame状态机的release publication与acquire claim，frame中的TaskContext及其已完成写入由该happens-before边发布。

child/snapshot可以共享immutable索引节点，但不得共享可变root/undo状态。callback每次invocation拥有独立TaskContext。若实现使用mutable page/tree，fork必须复制或建立正确copy-on-write ownership，不能让parent、child和并发callback相互覆盖binding。

slot cell的publish/read与thread start、metadata commit形成明确release/acquire关系；publish后不可改写。bound对象本身的并发访问仍由普通语言内存模型负责。

### 4.3 操作级契约

IR到runtime至少需要表达以下不同effect类别；名称和拆分方式不是固定ABI：

| 语义操作 | 必须保证 |
|---|---|
| `try_get(exact_key)` | 返回internal nullable managed ref；hit路径no-alloc、no-GC、nounwind；只比较/索引exact key |
| `push(exact_key, value)` | value已由HIR按key对应的exact静态ref type检查；runtime只接收managed-ref carrier，不读取或验证对象的运行期TypeDescriptor；可分配/GC；commit前failure-atomic；返回typed scope mark |
| `restore(mark)` | 线性消费mark并恢复其owner TaskContext的严格LIFO状态；no-alloc、no-GC、nounwind；非法/重复mark为runtime invariant错误 |
| `fork_current()` | 复制当前有效binding语义，child restore history为空；可分配/GC |
| `snapshot_current()` | 捕获immutable有效binding，不携带undo history；可分配/GC |
| `fork_snapshot(snapshot)` | 每次产生独立可变TaskContext；可分配/GC |
| `enter(next)` / `leave(guard)` | 保存/恢复thread current Context；no-alloc、严格LIFO并线性消费guard；guard内installed/previous ref可扫描 |

2.3/2.6的受限native-safe entry constructor是empty construction/`fork_snapshot`的专用runtime boundary：它在普通managed-entry invariant生效前以显式native roots持有输入handle、分配中间值和返回TaskContext，并在调用任何Scoop body前完成`enter`。它不能复用一个要求current Context已经存在的普通managed helper入口。

抽象`ContextScopeMark`至少绑定owning TaskContext identity与本次push安装的undo-top identity；restore要求thread current Context仍是owner且owner当前undo top恰为该项，然后才允许pop。只保存depth不够，因为两个TaskContext可处于相同depth。抽象execution guard同样绑定installed/previous Context与当前execution-entry top identity，leave只接受当前top。具体carrier可换，但其managed-ref leaf必须始终可扫描/重定位。

scope mark/guard必须是compiler-internal typed linear value，不能伪装成源码`Int`/`Long`或与其他machine scalar混用。mark跨挂起点存活时进入exact coroutine slot；`Suspended`返回边不消费mark，但会消费本次driver的execution guard。

## 5. 编译器分层

### 5.1 parser与AST

AST增加独立封闭节点：

```text
ContextParameterListSyntax
ContextParameterSyntax { name_or_underscore, type }
ContextScopeSyntax { value, body }
```

`ContextScopeSyntax.body`是block，不是lambda/call argument。parser在形成普通Call + trailing lambda前识别无receiver `context(expr) block`；declaration位置则要求冒号分隔的parameter list后紧跟合法declaration。

parser按当前declaration/block item边界恢复；非法target、缺type、malformed list/scope进入稳定语法或HIR诊断，不能先伪装普通调用再报“无匹配overload”。

### 5.2 HIR

HIR负责全部名称与类型语义：

- Export/LocalConcrete分别定义typed requirement/binding/key id和`Local | External` checked ref；generic Export使用typed exact-type/key recipe，concrete实体使用persistent key；recipe、persistent key与exact type使用不同newtype；
- context type使用已证明ref category的refined表示，不能保存`TypeId + bool`让下游补检；
- 完成parameter label/name scope、template与具体化两阶段duplicate exact-key检查、visibility/signature exposure、generic substitution、alias canonicalization、scope value独立定型、body result与`ControlOutcome`；
- Export HIR保存public/inheritance contract中的ordered declared-requirement list及typed exact key/recipe，但从不引用body local或entry action；generic template携带完整substitution recipe，hidden support不靠名称重解；
- HIR为每个fully concrete callable/owner contract生成ordered applied-requirement list并在此完成duplicate与override/slot比较，即使该contract没有body；
- LocalConcrete为每个contextual concrete body产生ordered entry snapshot plan；每项checked引用一个applied requirement，并带封闭的`BindNamedLocal(local) | CheckPresenceOnly` action。property getter/setter各自获得完备plan；
- 新增完备`CompilerContextCore`，至少绑定`MissingContextException` exact class/constructor及canonical type-name诊断所需实体。缺项不能进入MIR；下游不得按`scoop.core`名称查找；
- context list不进入overload/MSC，调用点也不产生hidden argument或static availability proof。

### 5.3 MIR与cleanup CFG

每个concrete contextual body先建立entry prologue：

```text
for requirement in source order:
    hit + BindNamedLocal -> initialize immutable context local
    hit + CheckPresenceOnly -> discard temporary result
    miss -> construct MissingContextException and throw
execute original body
```

lookup用typed hit/miss分支表达：hit边产生按requirement静态类型refine的managed ref，missing边走普通managed exception CFG；runtime不检查对象动态类型，不得表示成`void*`后由下游bitcast/unwrap。

HIR的structured `ContextScope`进入MIR cleanup plan，并在coroutine transform前正规化：

```text
evaluate value
mark = ContextPush(exact_key, value)
outcome = execute body

normal outcome:
    write exact typed hidden result place

every real scope-exit edge, in existing cleanup order:
    ContextRestore(mark)
    clear consumed mark storage when it has scanned managed leaves
    continue original Fallthrough / PendingTransfer / EH unwind
```

non-`Unit` normal body result必须先写入现有structured-expression的exact typed hidden result place，再restore，最后向外读取；`Unit`只保留body effects与restore，不伪造storage。managed ref或含ref aggregate在cleanup期间进入完整root/liveness计划。`return`/`break`/`continue`及其payload复用M22 `PendingTransfer`/`CleanupCursor`，不另建Context专用pending tag；throw/rethrow保留M25 typed unwind/native record并沿EH cleanup edge传播，Context不能把它物化成普通pending值或替换record identity。

`ContextRestore`是按lexical depth插入现有cleanup stack的普通typed item，与`finally`、`EndCatch`及其他cleanup严格LIFO。context位于active catch内部时离开顺序为restore后`EndCatch`；catch位于context内部时为`EndCatch`后restore。内部catch未离开context scope时不得提前restore。push失败前scope未active；validator证明每个mark在每条真正退出路径恰恢复一次，嵌套次序正确，且suspend edge不restore。coroutine transform继续消费既有static continuation chain，并把跨挂起存活的result/pending payload/mark放入exact frame slot；mark成功consume后必须在继续原transfer/unwind前把含managed leaf的frame storage清为canonical null，不能让静态RefScan经stale mark继续保活旧undo/root链。普通SSA mark可用liveness结束表达；execution guard若存入scanned storage也遵守相同清除规则。不能由codegen扫描CFG猜cleanup。

MIR还需要typed `ContextFork`、`ContextSnapshot`和`ContextExecutionEnter/Leave`，并在coroutine metadata中非可选记录frame TaskContext field、start boundary与resume guard。validator必须证明prologue只由source implementation initial entry执行，所有resume state绕过它；每次`ContextExecutionEnter`成功后，guard在`Suspended`、completed、failure和unwind的每个driver出口恰消费一次，previous TaskContext root活到`Leave`完成。`Suspended`出口执行execution leave，但绝不执行binding restore。closure conversion只捕获实际context local；foreign callback registration非可选记录snapshot capture policy。

### 5.4 LIR与codegen

LIR为exact context key、scope mark、TaskContext ref、snapshot ref与runtime target使用各自typed family：

- entry lookup的hit leaf及scope lookup fast path是明确no-GC/nounwind operation；missing branch构造异常属于managed CFG；
- `ContextPush/Fork/Snapshot`是managed、may-GC operation，携带`SafepointId`与完整statepoint/native root plan；
- restore和enter/leave使用受约束no-GC/nounwind target family，不塞入任意symbol call；
- managed/null/raw/code/metadata pointer provenance分离，LLVM同为opaque `ptr`不构成擦除理由；
- coroutine frame及runtime-private managed object的RefScan从LIR metadata完整产生；callback registration metadata则完整描述closure/snapshot/failure `GcHandle`的所有权、retain/release与失败清理，不能为C-heap token伪造RefScan；
- codegen机械发射与owner body linkage一致的key-use record/cell、runtime calls和cleanup blocks，不按type name、slot值、TLS layout或runtime struct offset补语义；
- M27首版不内联TaskContext物理布局；以后优化仍从同一个typed LIR operation降低，并保留helper基线。

ordinary call signature、Scoop ABI、C ABI和hidden continuation ABI都不增加Context参数。

### 5.5 stage与artifact完备性

各stage输出从结构上排除：

- declared requirement缺parameter label/exact ref（或合法template recipe）/key，错误携带body local，concrete duplicate key或public contract缺access witness；
- applied contract缺到declared requirement的checked mapping/exact key，或没有在owner/callable application上完成duplicate/override验证；contextual concrete body缺到applied requirement的checked mapping、entry action或missing edge；`BindNamedLocal`缺local、`CheckPresenceOnly`却携带local均无法构造；
- scope缺value exact key、mark或restore，或用`Option<Cleanup>`表示待决定；
- coroutine frame缺TaskContext field，resume/start boundary缺execution guard；
- callback registration有closure但缺snapshot capture policy；
- LIR managed operation缺root plan，nullable managed carrier被标成Raw；
- metadata只有type name/cell/symbol而缺persistent exact type/key/fingerprint关系。

`.slib` reader验证后只交付typed view；HIR→MIR→LIR相邻stage显式映射本crate id，不共享裸id type alias，也不由下游扫描arena重建关系。

## 6. 异常与生命周期边界

### 6.1 `MissingContextException`

core新增final `MissingContextException(message: String?) : Exception`，由compiler core contract精确绑定。compiler在missing分支构造稳定message，至少标识required canonical exact type、context parameter label和所属declaration diagnostic path；匿名label使用`context parameter #n: T`形式，不能伪造普通名称。M27不为它增加独立`SourceLocation`字段，对象布局与生命周期沿用普通`Exception`。

异常按M25 ordinary managed throw处理，可以被用户`catch`。callback边界按M13现有exception materialization/status协议处理。runtime lookup helper不按字符串构造Scoop对象，也不让异常跨C frame展开。

### 6.2 无取消保证

当前规范不定义coroutine cancellation。永不恢复的continuation不会隐式执行`finally`，M27同样不宣称会运行Context cleanup。其frame、TaskContext、undo和binding整体不可达后由GC回收；没有外部可观察的全局binding需要另行pop。

### 6.3 runtime invariant failure

以下不是用户可恢复的missing错误：

- 除2.3/2.6所述native-safe受限entry constructor外，任何Scoop managed body、generated managed adapter或普通managed runtime entry开始执行时没有current TaskContext；
- unresolved/损坏slot cell或exact type/key metadata不一致；
- restore mark属于错误depth/TaskContext或破坏LIFO；
- execution entry非LIFO leave；
- callback/resume离开时未恢复原Context；
- collector发现未登记或provenance错误的Context managed pointer。

这些情况打印稳定runtime ABI诊断后终止，不能继续执行已损坏的binding状态。

## 7. 当前runtime的参考实现

本节说明一条可落地路线，不把fanout、字段offset、helper名称、节点数量或slot分配顺序升级为长期契约。

### 7.1 不能直接照搬64-slot managed page

当前Immix配置`GC_LINE_SIZE = 128`、`GC_SMALL_MAX = 64`；超过64B的managed object进入large-object路径并至少占用一个32KiB block。一个含64个managed ref和16B header的page约528B，直接照搬附件中的64-slot page会把每个稀疏Context页放大成一个large block。

这是当前实现必须处理的真实障碍。M27不能在不同时重写allocator的前提下使用该布局。可选路线包括小节点radix、专门受GC理解的variable-size small array表示，或先改进allocator；最后一项会显著扩大里程碑。

### 7.2 建议：immutable persistent radix tree

首版建议使用全部不超过64B的小节点persistent radix tree，例如每层2 bit、每节点4个managed ref：

```text
TaskContext
  root: ContextNode?
  undoTop: ContextUndo?
  undoDepth: u64

ContextNode
  child[4]: managed ref?  // 中间层为node，叶层为payload

ContextUndo
  previous: ContextUndo?
  previousRoot: ContextNode?

ContextSnapshot
  root: ContextNode?

ContextScopeMark                 // compiler aggregate，不一定单独分配对象
  owner: TaskContext
  installedUndoTop: ContextUndo
```

节点publish后immutable：

- lookup按`ContextSlotId`的2-bit digit走固定层数，不扫描scope链、不做hash或type query；缺节点/叶null即unbound；
- push path-copy目标路径，先构造新root与undo，全部成功后一次commit `root/undoTop/depth`；
- restore先验证mark owner与当前TaskContext一致、installedUndoTop恰为当前top，再把root换回undo保存的previousRoot并弹栈，不分配；
- child TaskContext与snapshot共享immutable root，fork只分配新TaskContext并令undo为空；child rebind通过path-copy隔离；
- tree height不进入source/IR ABI。此参考方案按slot范围增长root，无需为每个task预分配与全程序key总数成比例的dense table；是否把这一空间性质提升为后续表示的验收约束，应由profiling与runtime spec另行决定。

在16B object header下，4-ref node约48B，TaskContext/undo/snapshot也可控制在64B以内。具体字段压缩、digit方向和height cache由runtime与TypeDescriptor layout共同决定。

该方案代价是每次push分配O(tree height)个小节点。若profiling证明Context切换远多于task fork，可以换成mutable small-node tree加fork复制或其他受检表示；不得改变第4节操作契约与GC可见性。

### 7.3 runtime-private managed types

参考结构中的TaskContext/node/undo/snapshot都使用ordinary managed object与精确RefScan，而不是C heap对象内藏managed pointer。其exact TypeDescriptor由compiler-owned typed metadata提供，并在program semantic phase验证size、alignment、scan与`release_hook = None`后交给runtime helper。

采用本参考结构时，为现有`GeneratedNominalRole`增加domain-separated的Context runtime role（TaskContext、Node、Undo、Snapshot）。其identity只来自一个program-independent、无环的runtime type key：

```text
ContextRuntimeTypeKey {
    representation_version: ContextRepresentationVersion,
    role: TaskContext | Node | Undo | Snapshot,
}

PersistentTypeId = H(
    "scoop-context-runtime-type-v1",
    representation_version,
    role,
)

PersistentExactTypeId = canonical exact-nominal encoding(
    PersistentTypeId,
    NoTypeArguments,
)
```

`ContextRepresentationVersion`是wire/spec固定的显式版本，物理字段、layout或scan角色改变时必须升级。identity不得消费`GraphFingerprint`、`ContextRuntimeSupportFingerprint`、program/object digest、最终地址或由这些type反向决定的runtime ABI fingerprint；相反，runtime ABI和support fingerprint消费已经确定的representation version与type定义。

这些internal exact type仍加入M23既有的全程序`PersistentExactTypeId ↔ TypeDescriptor address`唯一性验证，以及从full key导出`runtime_type_id`后的全局collision map；不得为Context建立第二套type registry。最终program-link在既有`VerifiedProgramDescriptorObject`内生成由`ContextRuntimeSupport`拥有的strong associated atoms，定义并登记对应TypeDescriptor/scan；不同Cone不得各自产生一份strong descriptor。它们与source Context key完全分离，并由semantic/layout/scan/ObjectDefinition leaf进入support及final program fingerprint，不进入任一用户Cone的RuntimeImage。若改用另一物理结构，则升级representation version并使用与之对应的封闭role集合，而不是保留不存在的占位descriptor。

仅列出TypeDescriptor不足以让C helper知道字段位置。当前实现还必须二选一提供封闭、versioned的runtime访问契约：要么由共享C/LLVM header固定本版layout并逐字段验证`sizeof`/`offsetof`/RefScan，要么由Context runtime support record提供已验证的typed offset/shape或generated accessor。选定契约随runtime ABI fingerprint发布；以后替换物理表示时升级该版本即可。runtime不能手写未登记的伪TypeDescriptor、按type name寻找它们，或在没有layout/access contract时猜offset。

### 7.4 参考slot cell编码

可用64位cell的0表示unresolved，非零值编码`ContextSlotId + 1`，保留完整u32 slot域并避免与合法slot 0混淆。registry以release publish，lookup以acquire读取；上层始终使用typed wrapper，不把编码值当源码integer。

为可复现metadata诊断，初始program key可按`PersistentContextKeyId` bytes排序分配slot；该顺序不是语义保证，测试不得断言某个source type恒为具体slot数字。

## 8. 测试与验收

### 8.1 parser、HIR与negative fixture

- ordinary/suspend、top-level/member/extension/local、generic、abstract/interface function及accessor-only property context list；property-level list同时约束getter/setter；`_`参数使用稳定ordinal诊断；
- 非法declaration target、stored/initialized/delegated/constructor property、以这些property实现contextual property obligation、constructor、lambda/function type、extern/intrinsic、duplicate name/key、非ref key及default中引用context name稳定报错；
- `context(value) { ... }`生成专用AST；qualified同名method仍是ordinary call；malformed prefix/scope按正确边界恢复；
- exact match正反例：derived binding不满足base/interface requirement，显式上转型后命中；运行期object subtype不影响；
- binding value只接受其自身ordinary typing context：body中的requirement和整个scope的expected result type都不反向给它提供父类型或generic约束；无context list的中间函数调用contextual declaration时不传播callee requirement；
- typealias与target同key，不同exact generic application不同key；generic substitution必须最终为ref；两个不同key recipe具体化后合并为同一exact key时，在function调用/显式实例化及无body的abstract/interface owner application处都拒绝；
- 两个声明只差context list是duplicate；override context contract缺失、多余、乱序或type不同稳定报错；
- Context操作在`@NoGC`、release block、const及其他非法effect位置正确拒绝；
- contextual callable reference创建时不lookup/snapshot，调用时才在当前TaskContext执行目标prologue；Context scope位于suspend body时可直接挂起，位于ordinary body时不产生新的挂起能力；
- AST/HIR golden锁定requirement list、entry snapshot plan、persistent exact type/key、scope value独立定型、access witness及`CompilerContextCore`。

### 8.2 参数snapshot、scope与异常

- unbound contextual declaration在body前抛精确`MissingContextException`并可catch；按list源码顺序报告第一个missing；`_`和未使用参数仍执行requirement；
- entry取得后嵌套同型binding不改变既有parameter，新callee取得内层值；
- closure引用context parameter时按ordinary local capture，创建于scope但不引用parameter的closure不隐式捕获Context；
- scope value在外层Context下只求值一次；求值失败或安装失败时新scope未active；
- 同key多层shadow正确恢复；normal result、return、break、continue、throw、catch和多层finally的每条scope-exit edge只restore一次；
- context在active catch内部与catch在context内部两种嵌套分别锁定`restore → EndCatch`/`EndCatch → restore`，rethrow保持同一M25 record identity；伪造wrong-owner/wrong-top或重复mark/guard稳定触发runtime invariant错误；
- binding对象在slot中是唯一强引用时保持存活；restore后新值可回收，旧值只在undo/未消费mark期间保持存活；跨挂起mark消费后frame slot被清空，不额外延长旧root链生命周期。

### 8.3 coroutine与线程迁移

- contextual suspend function的entry parameter及scope binding经过immediate和真实挂起均稳定；同型shadow中挂起再resume不重跑prologue，旧entry local不变；仅挂起不restore，最终scope exit才restore；
- `finally`内嵌Context与挂起、外层已有pending return/break/throw的组合，使mark、non-`Unit` exact result/pending payload共同跨frame且按既有cleanup chain恢复；每个driver的execution guard在Suspended/completed/failure出口恰leave一次；
- direct suspend调用共享TaskContext；`startCoroutine` child继承有效binding但undo为空；parent退出scope后child仍可由新callee取得binding，child override不污染parent；
- 两个child彼此隔离，bound object identity仍共享；
- 同线程和foreign thread resume都进入frame Context。resumer有冲突binding时coroutine看到自己的值，resume返回后resumer值恢复；
- nested suspend/completion/callback→resume覆盖A/B/A/outer的LIFO切换；
- abandoned continuation不运行finally/Context cleanup，丢弃全部引用后相关TaskContext可由GC回收。

### 8.4 FFI与callback

- `managed → native → return`的普通同步outbound往返保持当前Context，object/Mach-O检查证明ordinary ABI无hidden参数；native代码若中途经`foreignCallback`反向调用managed closure，则该invocation看到registration snapshot，而不是把调用线程当时的outer current Context当作callback Context；
- scope内registration、scope外invocation仍看到registration snapshot；registration后的外层rebind不改变snapshot；
- Reusable并发invocation拥有独立TaskContext；一次callback内override不泄漏到下一次或另一线程；
- callback normal/throw/GC/OneShot/Reusable/token final release都恢复outer Context并释放snapshot handle；
- callback中的missing异常进入token failure，绝不展开穿越C frame。

### 8.5 metadata、GC与artifact

- context key-use record缺失、同owner重复、跨owner合法同key、type/key/fingerprint不一致、cell越权/重复/unresolved及slot exhaustion稳定覆盖；同一ODR body跨Cone重复materialize时use/cell随body共同coalesce，未coalesce地址分裂稳定失败；runtime slot不改变semantic/program fingerprint；
- 主线程root及无outer Context的foreign callback通过受限native-safe entry constructor建立首个TaskContext；故意让任意managed body在安装前执行则稳定fatal，bootstrap/callback每个分配点强制GC仍由显式root保护；
- 0、1、多个及跨参考radix边界的key、稀疏高slot和empty Context正确；测试不锁定fanout与具体slot号；
- moving-GC stress搬迁ThreadState current root、execution previous root、TaskContext/node/undo/snapshot、context parameter local、bound/previous value、suspended frame及callback handle-registry中的closure/snapshot/failure object slot，并验证root/slot原地更新；
- push/fork/snapshot每个可能分配点强制GC，验证中间root reload与write barrier；native-safe期间GC后同步返回仍读取正确binding；
- HIR/MIR/LIR `.slib` round-trip、损坏矩阵、schema/capability mismatch、object record/cell ownership及program registration顺序完整覆盖；
- MIR golden锁定entry prologue、missing edge、cleanup、frame Context、start fork、resume guard和callback snapshot；LIR golden锁定managed/no-GC target与root plan；LLVM/object测试锁定typed registration/cell与TypeDescriptor scan，不锁死runtime-private node offset。

## 9. 实现顺序与完成门

建议分六批落地：

1. 先同步三份权威spec，固定Kotlin-like source surface、exact-match语义、declaration contract、core exception和task/callback规则；
2. 扩展三层wire、`.slib`、object verifier、image/program descriptor与program registry，完成exact-type key验证和slot cell publish；
3. 落地ThreadState root/execution entry、empty root bootstrap、Context runtime操作及选定表示，先通过C runtime与moving-GC stress；
4. parser/HIR/MIR接入context list、entry snapshot与scope，复用typed cleanup CFG，打通ordinary同步程序与property accessor；
5. coroutine frame、`startCoroutine` fork和resume guard接入，完成真实挂起与跨线程迁移；
6. foreign callback snapshot/token/adapter接入，补齐跨Cone、artifact corruption、全量golden与组合回归。

每批按仓库约定先执行`cargo fmt --all`和`cargo clippy --workspace`，再运行对应unit/golden/fixture。M27只有在以下条件同时满足时完成：

- 三份spec、core contract、IR和runtime对同一Context语义一致；
- exact match不暗中退化为subtype/runtime-type查询，body不反向影响binding value定型；
- ordinary ABI无hidden Context参数，key无FQN/name/symbol/ordinal fallback；
- entry snapshot、全部scope exit、suspend/resume、start child和callback边界通过typed validator与E2E测试；
- moving GC能更新全部Context root/ref，lookup hit无分配，restore no-alloc/nounwind，push/fork/snapshot failure-atomic；
- `.slib`与program metadata跨Cone exact-type identity闭合，slot不进入semantic fingerprint；
- 不保留`TODO`、`unimplemented`、未覆盖分支或依赖debug assert才能成立的结构约束。

## 10. 规范同步清单

正式实现前按spec-first工作流同步：

- `SCOOP-SPEC.md`：第2.1节能力列表；以本文运行期exact-match语义重写8.3；在11.7登记`MissingContextException`；在property/override/default规则中加入context contract；在11.9固定`startCoroutine` child inheritance；在14.3固定`foreignCallback` snapshot/invocation语义；
- `SCOOP-RUNTIME-SPEC.md`：program/image exact-type key-use metadata与slot cell；ThreadState current root和execution entry扫描；Context操作effect；受限native-safe bootstrap/teardown；callback token的snapshot `GcHandle`所有权及handle-registry root；coroutine frame/resume enter/leave；
- `SCOOP-IMPL-SPEC.md`：parser contextual syntax；HIR requirement/key/core identity；MIR entry snapshot、cleanup与coroutine transform顺序；LIR provenance/root plan；`.slib` schema、object materialization及validator责任。

这些修改与对应schema/version代码同批提交，不能只更新一份规范或长期让milestone文档与权威spec分叉。

## 11. 明确不在 M27

1. static effect row、effect-use inference、effect annotation、effect polymorphism、purity boundary或compile-time availability proof；
2. compatible-type/subtype Context resolution、body-to-binder requirement反推、同层多candidate ambiguity或按运行期类型选择binding；
3. general algebraic/control effect、可观察continuation、resume/drop/clone/multi-shot handler或native stack capture；
4. 独立source `context key`声明、first-class `ContextKey<T>`值、local/member/generative key、运行期制造key或key variance；
5. 直接承载struct/enum/tuple/raw pointer等value payload，以及公开nullable/optional lookup；
6. `context(v1, v2) { ... }`多value binding、无结构set/remove及源码直接操作TaskContext；多个binding用嵌套scope；
7. Context key/binding枚举、反射、`contextOf<T>()`、explicit context argument或context function type；
8. 普通closure隐式snapshot、公开`captureContext`/`runWithContext` API或任意arity callback wrapper；
9. `launch`/`async`/scheduler/structured concurrency库、coroutine cancellation及abandoned continuation cleanup；
10. lookup hoist、known-binding消除、slot constant folding、TaskContext scalar replacement及其他优化；M27保留typed operation/helper基线；
11. 为采用附件中的64-slot page顺带重写Immix allocator；物理表示优化作为独立性能工作评估。
