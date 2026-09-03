# M17 设计：命名参数、默认参数与 `vararg`

版本：0.2（草案）

对应`docs/ROADMAP.md`的新M17。目标是在M16统一调用决议内核上加入完整的source argument protocol：命名映射、调用处默认表达式实例化、`vararg`与spread，并让普通函数、local/member/extension generic callable、class/struct构造和enum variant构造使用同一套规则。

M17同时关闭M2“struct字段默认值/命名构造”、M4“enum variant默认值只接受literal”以及M7“默认参数/vararg参与重载”的backlog。次构造函数本身、`init`/body property及完整operator/context parameter属于后续里程碑；但它们将来必须复用M17的参数与调用协议，不能再设计第二套constructor argument规则。

## 0. 关键决策

- 默认表达式是callable source interface的一部分：在定义处完成解析、绑定与类型检查，其直接引用必须在该callable的每个合法调用处都可用；winner及完整type arguments确定后，才在实际缺省的调用处hygienic展开、求值。它不是声明时缓存的值，也不是在callee函数体入口内统一补值；
- 所有默认表达式经同一个typed template实例化器处理，不按literal、call、intrinsic、suspend或其他表达式kind分流；实例化器只负责完整替换、前置参数绑定和通用表达式来源，不识别任何具体intrinsic；
- receiver先求值，全部显式实参按调用源码顺序求值，然后才按形参声明顺序物化vararg及实际使用的默认表达式；最终call按形参顺序传值；
- 参数名、default和vararg是source-level callable metadata。`ExportHir`必须完整携带；`LocalConcreteHir`与MIR中的call已经是全量位置参数，不得让下游补齐；
- 命名映射针对每个候选独立完成，并在候选层选择与constraint solving之前形成typed map；不能先挑一个同名声明的参数名公共重排；
- generic inference只从receiver、显式实参、允许的外层expected type及声明constraint取得信息。未使用的默认表达式不求值，实际使用的默认表达式也不能反向“猜出”缺失type argument；
- `vararg x: T`的callee参数实际类型是`Array<T>`。位置vararg/spread物化新数组；命名传入完整数组直接使用。C的`...`仍不支持；
- 函数类型不保存参数名/default/vararg。取得声明引用后必须按完整`Array<T>`/普通参数函数类型调用，不生成隐式减元适配器；
- 所有语法错误和调用不适用均在HIR前端给稳定诊断；MIR/LIR/codegen不认识“缺省参数”概念。

## 1. 语言表面

### 1.1 普通、generic与成员函数

```kotlin
fun connect(
    host: String,
    port: Int = 443,
    retries: Int = defaultRetries(),
) { ... }

fun <T> collect(first: T, vararg rest: T): Array<T> { ... }

class Logger(val prefix: String) {
    fun write(message: String, loc: SourceLocation = getCurrentSourceLocation()) { ... }
}

connect("example.com")
connect("example.com", retries = 2)
connect(retries = 2, host = "example.com")
collect(1, 2, 3)
collect(1, *more)
```

top-level、local、member、extension、ordinary/suspend及non-virtual generic method都使用同一参数语法。generic owner参数与method自身参数仍按M14分组；默认表达式可以引用两组type parameter，但只在完整concrete application确定后实例化。

### 1.2 构造器与enum variant

```kotlin
struct Endpoint(
    val host: String,
    val port: Int = 443,
)

struct Values(vararg val values: Int)

class Request(
    val path: String,
    val headers: Array<String> = [],
)

enum Event {
    Message(val text: String, val retry: Int = 0),
    Batch(vararg val values: Int)
}

val a = Endpoint(port = 8443, host = "localhost")
val b = Event.Message("hello")
val c = Event.Batch(1, 2, 3)
```

- class/struct主构造参数与构造函数式enum variant字段允许default、命名调用和一个`vararg`；
- `vararg val values: T`形成的属性/字段类型是`Array<T>`。对struct/enum，这会按普通递归GC-free规则使含ref数组字段的concrete value不是GC-free，不允许保留原元素类型或按名称特判；
- block式enum named-field variant没有参数声明语法，继续要求所有字段用名字提供，不增加default/vararg；构造函数式variant用于需要这些能力的情况；
- positional enum variant没有字段名，不能使用命名实参；若未来为其加入`vararg`语法，必须先回spec定义，不在M17从类型形状猜测；
- 次构造函数尚未实现；后续加入后，其普通参数天然使用本里程碑协议。

### 1.3 默认声明与override

```kotlin
interface Drawable {
    fun draw(width: Int = 10)
}

class Shape : Drawable {
    override fun draw(width: Int) { ... }
}
```

- 普通有体函数、abstract函数、interface方法和允许bodyless的intrinsic/extern声明都可提供默认表达式；
- override不能写新的默认表达式，使用静态声明视图继承的唯一default source；
- override的parameter name不参与签名。`d: Drawable`上的命名调用使用`Drawable.draw`的名字，`s: Shape`使用`Shape.draw`可见的名字；动态dispatch只选择body；
- override的vararg形态必须一致。仅把`Array<T>`参数在override中改写成/移除`vararg`是定义处错误，避免同一个virtual slot对不同静态视图暴露不一致的调用协议；
- 同一override obligation从多个不相关父声明继承到多个默认表达式时，类型定义必须形成唯一default source，否则诊断冲突。通过不同静态interface引用调用时仍各自使用该interface声明的默认来源。

默认值与参数名不参与重载签名；`fun f(x: Int)`和`fun f(x: Int = 0)`重复。`vararg x: T`的实际参数类型是`Array<T>`，因此不能与同位置`x: Array<T>`仅凭vararg modifier形成重载。

## 2. 默认表达式语义

### 2.1 定义处解析

默认表达式在callable signature lowering阶段、声明方词法环境中完成：

- 名称、成员、overload、intrinsic与可见性全部绑定为HIR typed entity；对需要导出的default，绑定结果随后正规化为带调用域覆盖证明的export-interface reference；
- 当前callable的receiver、owner/callable type parameters和此前value parameters可见；
- 当前参数自身及后续参数不可见，不能靠调用处命名顺序改变；
- 表达式必须可赋给普通参数类型；vararg显式default必须可赋给实际类型`Array<T>`；
- generic template下的表达式以type parameter和`BoundCallableRef`保留，按M14规则证明定义对所有合法application成立；
- 默认表达式失败是声明处错误，不等到第一次调用或某个concrete instance才发现。

作为source interface，default template还必须在定义处通过**调用域覆盖检查**。令`CallDomain(C)`为callable `C`的全部合法源码调用位置，`AccessDomain(E)`为template直接绑定实体`E`的可访问范围，则每个引用必须满足：

```text
CallDomain(C) ⊆ AccessDomain(E)
```

这里的“直接绑定实体”包括普通/成员/extension callable、已选择的overload、constructor、property accessor、operator目标及引用到的nominal type；检查发生在const folding、desugaring和其他优化之前，不能通过先内联一个private const值来绕过。callable自身的receiver、owner/callable type parameter及前置value parameter是其interface输入，不作为外部依赖参与此检查。

直接推论是：

- exported `public` callable的default只能引用其下游可见的`public`或经`public import` re-export的实体；不能引用声明Cone的`private`/`internal`实现；
- `internal` callable可以引用覆盖整个Cone调用域的`internal`实体；private/member/local callable同理，只要被引用实体覆盖该callable的全部合法调用域；
- default调用的public函数，其函数体内部仍可使用private实现；调用域检查只约束default template直接绑定的interface实体，不递归公开callee body；
- 违反覆盖关系是default声明处错误，诊断同时指出default中的引用位置、被引用实体的可见域和声明callable的调用域。

调用处展开是hygienic typed expansion，不是复制原始AST后在调用方重新做名称解析。template保存定义处选定的typed entity identity；调用方的import、extension、局部同名声明或后来新增的overload都不能改变其含义。由于全部引用已经满足调用域覆盖，default不携带private/internal hidden dependency closure。

### 2.2 挂起性

- suspend callable的默认表达式可以调用suspend fun；发生缺省的调用点本身已经要求合法挂起上下文；
- 普通函数、class/struct constructor与enum variant default必须处于明确的non-suspend context，不能含挂起调用；
- 调用普通constructor时，suspend caller显式写出的参数仍属于caller表达式，可以先挂起并求值，然后进入同步构造；
- `@Extern`仍不能与`suspend`组合，因而extern默认表达式也必须non-suspend。

### 2.3 通用实例化与表达式来源

M17不为任何默认表达式内容建立特殊路径。exported callable使用`ExportDefaultExpr`，非导出callable使用HIR内部独立的`LocalDefaultExpr`；两者的每个节点都只保存声明方已经解析完成的hygienic typed body与`DefinitionOrigin`。template尚未进入一次具体求值，不能为它伪造或可选地附加`EvaluationOrigin`。winner commit后，HIR以统一的`DefaultInstantiationContext`展开实际缺失的参数：

- 完整替换owner/callable type parameter；
- 把对前置value parameter的引用绑定到已经物化的parameter value，而不是原始AST表达式；
- 保留每个template节点的`DefinitionOrigin`；
- 以发生缺省的call expression自身的`EvaluationOrigin`作为此次实例化的求值来源；
- 将结果作为调用处的普通concrete expression交回统一lowering，后续call、构造、allocation、throw、suspend和intrinsic均走各自已有的typed路径，不重新执行名称查找或overload resolution。

每个concrete expression都必须具有同一种完备来源结构：

```rust
struct ConcreteExpressionOrigin {
    definition: DefinitionOrigin,
    evaluation: EvaluationOrigin,
}
```

两个字段类型不同且都不可缺失。普通源码表达式的两者指向同一个源码表达式所对应的typed来源；默认template实例的`definition`仍指向声明中的具体节点，`evaluation`则来自发生缺省的call。不得通过覆盖clone后节点的span来冒充实例化，也不得使用`Option<EvaluationOrigin>`让consumer猜测回退位置。

`EvaluationOrigin`按表达式求值关系传播，而不是机械取当前文件中的物理span：若发生缺省的call本身来自另一个default实例，它已经携带上层的evaluation origin，内层default继续使用该origin，因此自然得到最外层实际调用位置。进入另一个普通函数体或稍后执行的lambda/local-function body时，则使用该body内表达式自己的普通evaluation origin；优化、内联或协程拆分不能改变它。

任何需要观察“本表达式从哪个源码位置求值”的现有或未来语言设施，都只能读取`ConcreteExpressionOrigin.evaluation`。默认参数模块不调用这类设施，也不判断其typed identity。`getCurrentSourceLocation()`只是第一个使用者：

```kotlin
fun trace(
    message: String,
    location: SourceLocation = getCurrentSourceLocation(),
) { ... }
```

普通intrinsic lowering读取该节点的`evaluation`并生成`SourceLocation`常量；它不需要知道这个来源是否由default实例化产生。调用`trace("x", explicit)`时没有缺失参数，整个template都不会实例化。default内调用普通helper时，helper body里的位置intrinsic读取helper body表达式的来源；这同样由通用函数体边界决定，不是为该intrinsic另设规则。

### 2.4 Export与失效

导出callable的default template、parameter name、calling shape、definition origin及已经绑定的export-visible entity identity进入`ExportHir`，随后进入`.slib` HIR metadata。default template不携带private/internal hidden dependency closure，也不把调用方需要重新解析的名称文本作为语义输入；其body只能引用带非可选调用域覆盖证明的kind-specific export-interface reference及callable自身的interface参数。普通`ExportTypeId`/`ExportCallableId`可能还索引generic body依赖闭包中的隐藏实体，不能直接冒充这种refined reference。下游在自己的调用点通过同一个通用实例化器生成`ConcreteExpressionOrigin`；export template与concrete instance使用不同IR类型，因此前者不需要以可选字段假装已经存在evaluation origin。

修改参数名、default body、default绑定目标或vararg形态都会改变调用接口metadata，并使依赖Cone的HIR缓存失效。把default所引用实体的可见域收窄到不再覆盖callable调用域，是被引用声明与default声明之间的HIR错误。链接ABI仍是完整实际参数列表，不能以“machine signature没变”为由复用旧调用结果。

## 3. 实参映射

### 3.1 AST形态

调用参数不能继续是`Vec<Expr>`：

```rust
struct CallArgument {
    name: CallArgumentName,
    spread: SpreadSyntax,
    expression: Expr,
    span: Span,
}

enum CallArgumentName {
    Positional,
    Named(Ident),
}

enum SpreadSyntax {
    Plain,
    Spread(Span),
}
```

这些是语法上确实可选的sum，不用`Option<Ident>`和另一个bool组合非法状态。annotation argument已有自己的literal-only模型，不与普通call argument共用，避免注解规则被spread/lambda污染。

parameter AST同样显式区分：

```rust
enum ParameterSyntax {
    Required,
    Default { expression: Expr, equals_span: Span },
    Vararg {
        modifier_span: Span,
        default: VarargDefaultSyntax,
    },
}

enum VarargDefaultSyntax {
    EmptyWhenOmitted,
    Expression { expression: Expr, equals_span: Span },
}
```

constructor property/field另保留`val`/`var`类别，但复用`ParameterSyntax`，不复制default字段。parser保留每个name、`=`、`vararg`和`*`的精确span。

### 3.2 位置与命名混合

对每个候选按源码顺序扫描参数：

1. 维护下一个声明位置`p`和是否进入`NamedOnlyTail`；
2. 普通位置实参在非vararg位置映射到`p`并推进；在vararg位置成为该参数的一个element；
3. 命名实参若名字恰是当前位置`p`且此前映射保持声明顺序，可以映射后继续允许后续位置实参；
4. 命名实参一旦跳到其他参数、跳过一个默认参数或离开vararg位置，进入`NamedOnlyTail`，后续出现未命名实参使该候选不适用；
5. 参数不能被重复绑定；未知名字使候选shape不匹配；
6. 扫描结束后，未绑定required参数使候选不适用，default/vararg按各自omission规则补齐。

因此：

```kotlin
fun f(a: Int, b: Int = 0, c: Int = 0) {}

f(1, b = 2, 3)       // 合法：b仍处于自己的声明位置，3映射c
f(1, c = 3)          // 合法：b使用默认；进入named-only tail
f(c = 3, a = 1)      // 合法：全部按名
f(c = 3, 1)          // 非法：跳到c后不能再用位置实参
f(1, 3)              // b = 3；不能在位置序列中隐式跳过b去绑定c
```

block式enum named-field variant从开始即处于named-only模式，所有字段必须恰好绑定一次。

### 3.3 `vararg`映射

```kotlin
fun emit(prefix: String, vararg values: Int, suffix: String = "") {}

emit("[", 1, 2, 3, suffix = "]")
emit("[", *values, suffix = "]")
emit(prefix = "[", values = values, suffix = "]")
```

- 位置扫描到vararg后，所有后续未命名普通/spread实参都属于vararg；其后的参数只能按名提供；
- 每个普通元素产生`actual <: T`，每个spread当前要求`actual == Array<T>`；可出现多个spread并与普通元素混合；
- 命名`values = array`或`values = *array`提供一个完整、精确`Array<T>`，不能再混合其他values输入；
- 未提供任何输入时：若vararg声明显式default，则使用default；否则产生空array；
- named whole-array形式不复制；位置element/spread形式始终物化fresh array，唯一`*array`也复制，避免`===`观察到来源alias；
- 一个parameter list只能声明一个vararg。`vararg`不是C ABI的`...`，也不接受非Array iterator/sequence自动展开。

由于`Array`当前invariant，spread只接受精确`Array<T>`。M17不隐藏一个`Array<S> -> Array<T>`逐元素转换；use-site projection落地后应由同一个constraint relation重新定义可接受的projected source，而不是在vararg路径按element name猜类型。

### 3.4 候选层预过滤

M16的candidate layer选择升级为：

- 显式type argument arity、命名参数集合、重复name、spread可能性与receiver类别先形成候选自己的shape结果；
- 只有shape可映射的候选进入type applicability；
- 一个层必须至少含一个最终applicable候选才会阻止查找下一层；
- 同名高层成员没有某个命名参数、arity不匹配或类型不适用时，不会遮蔽合法extension；
- 无候选时诊断展示最优先相关层中的shape/type失败，不把“未知参数名”误报成unknown function。

## 4. Generic inference与重载

### 4.1 Constraint来源

argument map完成后，M16 solver按形态加入约束：

- explicit single：`actual <: parameter`；
- vararg element：`actual <: element_type`；
- vararg spread/named array：`actual == Array<element_type>`；
- default/implicit empty vararg：不从表达式具体值反向绑定type parameter；
- receiver与外层expected application按M16规则加入；
- declaration kind/interface bound始终加入。

例如：

```kotlin
fun <T> empty(values: Array<T> = []): Array<T> = values

val a: Array<Int> = empty() // T由外层expected type确定
val b = empty()             // 错误：default []不能自行猜T
```

default template已经在generic定义环境中检查正确，但只有目标type arguments全部确定后才可实例化。禁止先把`[]`武断定为`Array<Nothing>`/`Array<Any>`再反推T。

### 4.2 Applicability

候选可应用必须同时满足：

- argument map合法，每个required参数有输入；
- type arguments完整、唯一并满足所有bounds；
- explicit、spread、lambda/callable reference都能在各自parameter expected type下成立；
- 每个实际使用的default template能在该concrete application中完成替换；
- suspend/unsafe/FFI上下文合法。

默认表达式本身的运行期body不在candidate probe中commit；只有winner实例化。若定义阶段已经保证template对所有合法type arguments成立，candidate不会因“试着lower默认body失败”才被淘汰。

### 4.3 MSC附加规则

pairwise forwarding只比较本次调用**实际提供**的参数；某候选依靠default填入的参数不参与其转发parameter列表。extension receiver始终参与。

对互相可转发、且普通specificity/非参数化规则仍无法唯一选择的候选：

1. 统计本次调用实际使用的显式default template数量，较少者优先；implicit empty vararg不计入default数量，带显式default的vararg缺省计入；
2. 仍并列时，没有vararg parameter的候选优先于有vararg的候选；是否本次恰好传了一个Array不改变声明的vararg性质；
3. 命名或位置写法本身、参数名称文本、default表达式内容都不参与优先级；
4. 仍不唯一即歧义。

示例：

```kotlin
fun f(x: Int) = 1
fun f(x: Int, y: Int = 0) = 2
f(1) // 第一个候选使用0个default，胜出

fun g(x: Int) = 1
fun g(vararg x: Int) = 2
g(1) // non-vararg胜出
```

仅default/参数名不同而source parameter types完全相同的声明在定义处已经是重复，不会留到调用点比较。

## 5. 求值与正规化

### 5.1 两阶段求值

winner确定后，HIR commit构造两组临时值：

**显式阶段**：

1. receiver；
2. 圆括号内实参从左到右；
3. 尾随lambda；

每个显式expression只lower一次到独立temporary/sink。即使命名映射把它送到较早形参，也不移动其执行位置。

**参数物化阶段**按形参声明顺序：

- explicit single引用已求值temporary并做winner所需coercion；
- positional vararg根据已求值element/spread temporaries生成fresh `Array<T>`；
- named vararg引用完整array temporary；
- missing vararg生成empty array或实例化显式default；
- missing regular parameter实例化default template；
- required missing在applicability阶段已经失败，不可能进入commit。

最后按声明顺序发出完整call。例：

```kotlin
fun f(x: Int = dx(), y: Int, z: Int = dz()) {}
f(z = ez(), y = ey())
```

运行顺序固定为`ez()`、`ey()`、`dx()`，然后调用`f(x=dx结果, y=ey结果, z=ez结果)`；`dz()`不执行。

若两阶段求值发生在`while`条件中，产生的temporary/sink属于条件的typed setup区域，并在每次条件检查前重新执行；不得提升到循环外，也不能把普通调用产生的sink误判为空安全脱糖而拒绝。

### 5.2 vararg array assembly

HIR增加一个完全类型化的普通array物化操作，而不是未决call argument：

```rust
struct ArrayAssembly {
    element_type: ConcreteTypeId,
    parts: Vec<ArrayAssemblyPart>,
    result_type: ConcreteClassId,
}

enum ArrayAssemblyPart {
    Element(Expr),
    CopyArray(Expr),
}
```

进入该节点的Expr都是此前temporary读取，不再含源码调用顺序。`result_type`必须是精确`Array<T>`application；MIR可先计算总长度、分配一次并按parts顺序复制。长度/分配失败沿用普通Array构造的错误契约，不允许退化为native临时buffer或泄漏malloc。

该节点表达“构造一个Array值”，不是default/vararg placeholder，可以作为以后普通array concat优化的共享基础。sole spread仍使用`CopyArray`并产生fresh identity；named whole-array不产生节点。

### 5.3 suspend与异常

- 显式实参或suspend default发生挂起时，此前已求值temporaries、receiver与已物化参数进入M10 frame；恢复后从下一个求值步骤继续；
- 任一显式/default/array assembly抛异常时，callee body尚未进入；已完成表达式不重跑；
- vararg array由GC管理，不需要finalizer/cleanup；moving GC按普通Array root规则更新temporaries；
- HIR求值顺序必须通过statement/temporary结构表示，不能仅靠Vec约定让MIR猜。

## 6. AST、HIR与stage边界

### 6.1 Parser/AST

- lexer加入`vararg`关键字及call argument位置的`*`；乘法、类型星号（未来projection）与spread由parser上下文区分；
- function parameter、constructor field parameter及constructor-style variant field复用`ParameterSyntax`；
- call、member call、constructor、variant与intrinsic-looking普通调用全部使用`Vec<CallArgument>`；
- trailing lambda转换为带最后source index的普通`CallArgument`，不在HIR另开参数通道；
- parser只检查局部语法：重复modifier、一个声明列表多个vararg等可直接确定规则；名称存在、参数重复映射和default type由HIR检查；
- AST dump保留位置/命名/spread/default/vararg原貌。

### 6.2 ExportHir

source callable parameter使用完备sum：

```rust
struct ExportValueParameter {
    name: String,
    calling: ExportParameterCalling,
    origin: DefinitionOrigin,
}

enum ExportParameterCalling {
    Required {
        value_type: ExportTypeId,
    },
    Default {
        value_type: ExportTypeId,
        expression: ExportDefaultExprId,
    },
    Vararg {
        parameter_type: ExportVarargParameterTypeId,
        omission: ExportVarargOmission,
    },
}

enum ExportVarargOmission {
    EmptyArray,
    Default(ExportDefaultExprId),
}
```

`ExportVarargParameterTypeId`索引一个由HIR原子建立的实体，其中同时包含element type和对应的精确`Array<element>`application；普通/default分支直接携带value type。consumer不从element type重新查找Array，也不会遇到一个平行`actual_type`与vararg信息不一致的组合。

`ExportDefaultExpr`包含hygienic typed template body、result type、逐节点definition origin、允许的type/value parameter引用及已经绑定的export-interface references。callable、type、property/accessor等不同实体继续使用不同的ref类型，例如`ExportDefaultCallableRef`与`ExportDefaultTypeRef`；每个ref结构化携带目标export id和非可选`ExportDefaultAccessWitness`，证明目标的访问域覆盖所属callable的调用域。不存在一个无类型`ExportDefaultEntityId`，也不能直接放入可能指向generic hidden dependency的普通`ExportCallableId`/`ExportTypeId`。这些ref只能由定义处覆盖检查成功的builder产生，因此`.slib`reader只消费完备结果，不重新验证、反推或补齐可见性。

default不存在使用`Required`表达，而不是`Option<Expr>`；vararg的两种omission也不能用空Option猜。

override默认继承使用独立typed relation指向唯一`ExportDefaultExprId`，不是复制文本或按方法名搜索。跨Cone读取时所有id重映射发生在HIR import层，不能把上游local concrete id写进default body。

### 6.3 HIR内部resolved plan

每个applicable candidate产生：

```rust
enum DefaultExprTemplateRef {
    Local(LocalDefaultExprId),
    Export(ExportDefaultExprId),
}

enum ResolvedParameterInput {
    Explicit(SourceInputId),
    Default(DefaultExprTemplateRef),
    Vararg(ResolvedVarargInput),
}

enum ResolvedVarargInput {
    Parts(Vec<VarargPart>),
    WholeArray(SourceInputId),
    Empty,
    Default(DefaultExprTemplateRef),
}
```

`LocalDefaultExprId`与`ExportDefaultExprId`类型不兼容；前者只存在于当前HIR invocation的source-interface工作集，后者属于可序列化的`ExportHir`。`DefaultExprTemplateRef`是HIR内部对同一实例化算法的封闭输入，不会进入任何stage输出。`Vec<VarargPart>`可以为空，因此`Empty`必须独立，避免“空Vec到底是缺省还是显式零元素”的猜测。winner commit后这些source-level计划全部消解为ordinary temporaries、`ArrayAssembly`及完整ordered call args。

### 6.4 LocalConcreteHir

- concrete function parameter只有name/local、实际concrete type与callee执行属性；default/vararg metadata不进入函数body输入；
- concrete call始终含与callee实际parameter count相同、按声明顺序排列的argument；
- default template已经替换全部type parameter并生成concrete expression；
- 每个concrete expression已经具有完备的`ConcreteExpressionOrigin`；实例化时使用的default plan/template id不再存在；
- 包括`current_source_location`在内的source-sensitive intrinsic已经通过普通intrinsic lowering读取`evaluation`并变成concrete值，不存在default专用节点或分支；
- `ArrayAssembly`已具有完整element/result concrete identity及typed parts；
- 不得出现`LocalDefaultExprId`、`ExportDefaultExprId`、`DefaultExprTemplateRef`、`SourceInputId`、named/spread语法、missing argument或inference variable。

### 6.5 MIR/LIR/codegen

- MIR为`ArrayAssembly`生成checked total length、普通managed Array allocation与顺序copy；含ref元素沿用Array扫描，写入Mutable staging或初始化区时遵守现有barrier/publication规则；
- 普通/default参数调用没有新MIR call ABI；callee始终接收完整参数；
- suspend default在MIR看到的只是普通concrete suspend call，进入既有coroutine transform；
- LIR/codegen不读取参数名、default或vararg，不生成Kotlin/JVM式`$default`bridge、bitmask或额外stub；
- FFI call在source protocol消解后才进入C/Scoop ABI lowering。Scoop ABI extern vararg实际是`Array<T>`参数；C ABI因`Array<T>`不是C-FFI-safe在HIR拒绝。C `...`没有任何表示。

## 7. 特殊声明与边界

### 7.1 Intrinsic/extern

- bodyless intrinsic/extern可以声明default，因为代码来自调用方；registry/ABI只验证完整实际参数签名；
- intrinsic default中的调用仍必须是普通可验证Scoop表达式，不能把未解析源码交给codegen；
- `@Extern(abi = "c")`不能声明language vararg，因为其实际类型为managed`Array<T>`；`abi = "scoop"`可声明，但目标native symbol必须真实接收普通Scoop Array参数；
- 任何形式都不代表C varargs，不生成ABI adapter猜测参数数量。

### 7.2 Callable reference与function value

```kotlin
fun f(x: Int, y: Int = 0) {}
val ref: (Int, Int) -> Unit = ::f

ref(1, 2) // 合法
ref(1)    // 错误
```

default不会让`::f`适配成`(Int) -> Unit`；vararg declaration在函数类型中是一个`Array<T>`参数。需要减元或便利调用必须显式写：

```kotlin
val adapted: (Int) -> Unit = { x -> f(x) }
```

这样default求值点是lambda body中的`f(x)`调用，而非调用`adapted`的位置；若希望传递外层location，必须按11.12显式增加并转发参数。

### 7.3 Default引用参数

```kotlin
fun read(buffer: Array<Int>, offset: Int = 0, length: Int = buffer.size) {}
```

`length`template保存对前置parameter identity的引用。调用处先完成全部显式表达式，再按parameter order建立`buffer`、`offset`、`length`binding。实例化不能把参数引用替换成原AST表达式，否则会重复副作用；必须读取已经求值/物化的temporary。

## 8. 诊断与错误恢复

至少覆盖：

- duplicate/unknown named argument、named-only tail后的position argument；
- required missing、too many arguments、spread to non-vararg；
- 多个vararg、vararg override mismatch、仅default/vararg modifier不同的重复签名；
- default引用自身/后置参数、default type mismatch、普通/constructor/extern default中的suspend call；
- default直接引用的实体不能覆盖声明callable的全部调用域，包括public default引用private/internal实体、缺少`public import`的跨Cone实体以及优化前不可见的const/property/operator目标；
- vararg element/spread/whole-array类型错误；
- generic type argument无法从空vararg/default推断；
- 多父default source冲突；
- function value使用named/default幻想或vararg元素式调用；
- C ABI language vararg与C `...`误用。

overload无匹配诊断按M16显示每个候选的mapping失败；parser恢复在`,`、`)`及下一个parameter边界同步，单个坏named/spread参数不能吞掉后续声明。

## 9. 测试计划

### 9.1 Parser/AST

- required/default/vararg/default-vararg、`vararg val`constructor field；
- positional/named混合、named tail、普通/multiple spread、named array、尾随lambda；
- call/member/constructor/variant全部dump source form；
- malformed`=`/`*`、重复modifier、多个vararg及恢复。

### 9.2 HIR与constraint

- 每种callable kind的parameter map、default template与winner-only实例化；
- generic default/vararg由显式element、spread、receiver和outer expected type推断；空/default不虚构binding；
- M16 fresh MSC加“更少default”“non-vararg”规则；
- lambda/callable reference作为named或vararg element继续postpone；
- override/interface default来源与静态参数名；
- exported default的表达式节点只含带非可选访问证明的kind-specific export-interface reference且不含hidden dependency closure；local/export template id严格分型，LocalConcreteHir不含source protocol。

### 9.3 求值顺序

独立fixture记录side effect序列并断言：

- receiver → 显式实参源码顺序 → default声明顺序 → body；
- named乱序不改变显式顺序；显式提供的default完全不执行；
- 多个spread/element各求值一次，sole spread产生fresh array；named whole array保持identity；
- default引用前参不重复其显式表达式；
- suspend default/explicit在挂起恢复后不重跑；异常阻止后续步骤与callee entry；
- moving stress mode下所有temporary、default结果与ArrayAssembly在allocation间保持有效。

### 9.4 通用origin与跨边界

- literal、普通call、构造、generic、local/member及suspend default全部通过同一个template实例化入口，且显式提供参数时不建立实例；
- 每个实例化节点同时具有正确的definition/evaluation origin，定义处诊断与调用处求值来源互不污染；
- 嵌套default实例继承call expression的evaluation origin，普通helper及稍后执行的lambda/local-function body建立自己的来源边界；
- ExportHir round-trip只保留default template及definition origin；LocalConcreteHir golden对每个表达式要求完备`ConcreteExpressionOrigin`，不含default plan/id；
- visibility negative覆盖public→private/internal、未re-export依赖、const-folding前不可见引用；positive覆盖internal/private/local callable的调用域包含关系；
- default实例化模块的依赖边界测试保证它不引用intrinsic registry或任何`IntrinsicKind`；
- 以`getCurrentSourceLocation`作为普通source-sensitive consumer做集成fixture，覆盖direct/default/显式location、多层转发及generic/member/suspend中的function/type name；
- Scoop ABI extern default/vararg成功，C ABI vararg拒绝。

fixture建议使用`tests/fixtures/m17-arguments/`、`m17-defaults/`和`m17-vararg/`三个语义目录，避免形成单个超长fixture；组合测试再覆盖generic overload、constructor、closure、suspend与GC。

M1–M16全部fixture必须通过；M4原literal-only enum default negative转为接受任意合法non-suspend表达式，并增加真正非法default的negative。

## 10. 实现顺序与验收门

1. parser/AST parameter与call argument封闭sum、dump和恢复；
2. default调用域覆盖检查、local/export template的typed id隔离、ExportHir parameter calling metadata与hygienic typed body，以及所有concrete expression共用的完备definition/evaluation origin；
3. candidate-specific named/vararg mapping并接入M16 layer/applicability；
4. generic constraints、default/vararg MSC附加规则与完整诊断；
5. winner commit的显式temporary阶段、parameter materialization与default实例化；
6. `ArrayAssembly`全链路及GC/suspend/异常测试；
7. constructor/variant/override/intrinsic/extern整合；
8. 删除M4 literal-default复制逻辑、所有`Vec<Option<Expr>>`default输出和callee-side补参设想；
9. 全量格式化、lint、stage golden及fixture回归。

M17只有在所有现有source callable/constructor使用同一argument map，所有默认表达式都作为定义处绑定的source interface通过调用域覆盖检查、不分kind地经过同一个typed template实例化器，能够使用generic/interface参数但不携带窄可见域隐藏依赖，通用definition/evaluation origin与求值顺序正确，vararg fresh/whole-array identity明确，且`LocalConcreteHir`/MIR完全看不到missing/named/default/spread protocol时完成。调用方重新解析AST、允许public default引用private/internal实体、只支持尾部常量default、为某个intrinsic设置default专用路径、只给非generic顶层函数补参数或通过callee `$default`stub实现，都不算完成。

## 11. 明确不做

1. secondary constructor、`init`、body property与`super`调用本身；
2. context parameters、receiver function type、SAM conversion、builder inference；
3. C varargs、反射式`callBy`、运行期按参数名调用或default bitmask ABI；
4. use-site/star projection下的`Array<out T>`spread；M17要求精确`Array<T>`；
5. `OverloadResolutionByLambdaReturnType`；
6. 为函数声明引用自动生成减元/default/vararg adapter；
7. 多Cone打包与import层实现；M17只把完整metadata和consumer边界准备好；
8. vararg的stack allocation、small-vector或zero-copy优化；未优化基线先保证fresh identity、GC与求值语义。
