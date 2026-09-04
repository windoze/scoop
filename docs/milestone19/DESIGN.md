# M19 设计：构造与初始化

版本：0.2（草案）

对应 `docs/ROADMAP.md` 的 M19。目标是在 M16 统一调用决议、M17 完整 source argument protocol、M18 callable 表面之上，补齐 class / struct 的构造闭环：普通 primary parameter、secondary constructor、class stored body property、`init`、基类构造委托与 `super.method()`，并把初始化顺序、半初始化对象边界、异常及 moving GC 契约一次锁定。

M19 不是给现有单主构造路径增加几个语法分支。当前 MIR 把整条继承链的字段值递归收集后一次 `ClassInit`，从未调用基类构造体；该捷径无法表达 `init`、body property initializer、secondary body、异常和同一对象 identity。M19 必须把它替换为“一次分配 exact concrete object，再在同一 receiver 上逐级调用 typed constructor initializer”的模型。所有源码构造目标、委托目标和 `super` 方法目标都在 HIR 唯一确定；MIR 以后不能按 class 名、参数个数或字段布局重建构造语义。

## 0. 关键决策与范围

- class primary constructor 参数可以是普通参数，也可以用 `val` / `var` 同时声明属性；普通参数只在基类委托、body property initializer 和 `init` 中可见，不进入实例布局或普通成员作用域；
- M19 的 body property 是 class body 中带显式类型、带 initializer 的 stored `val` / `var`。计算属性、自定义accessor、delegate、Option-typed initializer omission、extension/interface/object property仍归M21；M21明确不引入`lateinit`。struct继续没有body stored state；
- class 支持任意数量 secondary constructor；struct 支持必须委托到 primary constructor 的 secondary constructor。constructor 不声明自己的类型参数，参数与调用完整复用 M17 的 required / default / `vararg`、命名 / spread、求值顺序和 candidate-local mapping；
- 构造器形成有向委托图。class 有 primary 时每个 secondary 必须经 `this(...)` 终止于 primary；无 primary 时必须经 `this(...)` 链终止于一次 `super(...)`；struct 必须终止于 primary。自环与多节点环都是定义处错误；
- class allocation 在调用处全部构造实参完成后发生且只发生一次。primary / secondary / base constructor initializer 都在同一 object identity 上执行；基类 initializer 不重新分配、不重写最派生 TypeDescriptor；
- base 初始化先于 derived 自有字段。primary property store 先于 class body；body property initializer 与 `init` 按 class body 源码顺序交错执行；secondary body 在其委托目标完整返回后执行，`this` 链的 body 从最内层向最外层执行；
- 初始化上下文中的 `this` 是 HIR 内部的受限 `InitializingThis`，不是可逃逸普通值。它只允许直接访问已经初始化的字段；不能被传参、返回、存储、捕获、装箱、转换、取 callable reference，也不能作为普通 / extension / virtual / interface / `super` 方法 receiver。由此从结构上禁止发布半初始化对象和基类构造期间的虚调用；
- `super.name(args...)` 只表示直接基类成员方法调用：以 direct base application 建候选，只收集 class member，不收集 extension / property-like 候选，winner 以 direct dispatch 调用实际基类实现。`super` 本身不是表达式；`super.field`、`super::name`、`super<T>` 和 interface-qualified super 不在 M19；
- 所有初始化体、constructor body 与 delegation 都是 ordinary safe managed 上下文，不能 `suspend`，也不能带 `@NoGC` / `@Extern` / operator / infix / modality。显式构造实参仍属于 caller；因此 suspend caller 中 `C(awaitValue())` 合法，`awaitValue()` 完成后才进入同步构造；
- 分配后的完整对象 payload 在任何 safepoint 前清零，initializing receiver 是普通精确 GC root。尚未写入的 managed field 只能以全 0 非引用形态被扫描，不能用未初始化字节、poison 或 `Option<Type>` 掩盖字段状态；
- constructor / field / initialization step 都使用独立 typed id。primary、secondary、class constructor、struct constructor、constructor application、concrete initializer 与普通 `FunctionId` 互不冒充，不能用 FQN、参数个数或 arena 平行下标恢复身份。

## 1. 语言表面

### 1.1 primary parameter、body property 与 `init`

```kotlin
open class Entity(val id: Int)

class User(
    rawName: String,
    val id: Int,
    var enabled: Boolean = true,
) : Entity(id) {
    val name: String = normalize(rawName)

    init {
        println("user-init")
        enabled = enabled && name != ""
    }

    val label: String = name + id.toString()
}
```

primary parameter 使用与普通参数相同的类型、default 与 `vararg` 语法，其中 `val` / `var` 决定是否同时声明 stored property。普通 primary parameter：

- 可用于 class header 的 base constructor arguments、body property initializer 和 `init`；
- 不是 field，不能通过 `this.rawName` 或构造完成后的 member function 读取；
- 与所有 primary parameter 一样是不可重新绑定的 constructor local；`var` 只表示 mutable property，不表示参数 binding 可重新赋值；
- 若是 `vararg T`，作用域中的实际类型为 `Array<T>`；只有写了 `val` / `var` 才把该数组值存入字段。

class body stored property 的 M19 形态固定为：

```text
(`val` | `var`) name `:` Type `=` expression
```

类型和 initializer 都不能省略。initializer 必须可赋给声明类型，且处于 ordinary、safe、non-suspend 初始化上下文。property 与 primary-constructor property 一起进入同一个实例 field namespace、继承检查、布局和递归 GC scan；`val` 构造完成后只读，`var` 使用现有 field assignment / barrier 路径。

同一class的primary parameter名称必须唯一；body property不能与带`val`/`var`的primary parameter、同class其他field或inherited field重名，但可以与普通primary parameter同名。common initialization中的无receiver名称先解析到该词法parameter，字段必须用`this.name`显式访问；因此`val x: T = transform(x)`的右侧读取parameter，而`this.x`在该initializer中仍是会被readiness拒绝的self read。secondary parameter同样可按普通词法规则遮蔽field，body中用`this.name`显式访问字段。

一个 class 可以有多个 `init { ... }`。body property initializer 和 `init` 是 class-owned common initialization sequence；method / secondary constructor 声明本身不执行，也不切断相邻初始化项的源码顺序。`return` 不能退出 initializer、`init` 或 constructor body；`throw` 正常终止本次构造并沿普通异常边传播。

struct 仍只把 primary constructor 的 `val` 参数作为完整值布局，不接受普通 primary parameter、`var` field、`init` 或 body stored property。计算属性不占存储，连同 accessor 在 M21 定义。

### 1.2 secondary constructor

```kotlin
class User(rawName: String, val id: Int) {
    val name: String = normalize(rawName)

    constructor(id: Int) : this("anonymous", id) {
        println("secondary-inner")
    }

    constructor() : this(0) {
        println("secondary-outer")
    }
}

open class Base(val value: Int) {
    constructor(text: String) : this(parse(text)) {}
}

class Derived : Base {
    val doubled: Int = value * 2

    constructor(value: Int) : super(value) {}
    constructor(text: String) : super(text) {}
}

struct NonZero(val value: Int) {
    constructor() : this(1) {}
}
```

secondary constructor 语法为：

```text
constructor(parameters) (`:` (`this` | `super`) arguments)? block
```

规则：

- 参数不能写 `val` / `var`，不直接声明字段；允许 required、default 和一个 `vararg`，但不能声明独立 type parameter 或返回类型；
- constructor default可以引用host type parameter与此前constructor参数，但没有源码`this` receiver，也不能访问实例field；class allocation发生在调用处全部default物化之后；
- body 必须是 block，不能省略或使用 expression body；constructor 不能写 `suspend`、`operator`、`infix`、`open`、`final`、`abstract`、`override` 或 M12 callable annotation；可见性与 constructor annotation 随 M21；
- class 显式声明 primary constructor 时，所有 secondary 必须直接或间接 `this(...)` 到 primary，不能直接 `super(...)`，且 delegation 不能省略；
- class 没有 primary constructor时，secondary 可以 `this(...)` 到另一个 secondary，或者以 `super(...)` 终止；省略 delegation 等价于 `super()`：存在direct base时进入完整overload/default检查，没有显式base时正规化为编译器根初始化；
- struct secondary 必须显式 `this(...)`，并最终到达唯一 primary；不存在 `super` 或省略 delegation；delegated primary 先形成完整 immutable value，secondary body 随后只可读取其字段和执行验证 / 副作用，不能修改字段；
- `this(...)` / `super(...)` 只在 delegation clause 中合法，不是普通 call expression，也不能出现在 body 的第一条语句来模拟委托；
- secondary body 只能在目标 constructor 完整正常返回后执行。若目标抛异常，当前 body 不执行；若当前 body 抛异常，外层 delegating body 也不执行。

### 1.3 primary 是否存在与 base header

显式 `class C(...)`（包括 `class C()`）声明 primary constructor。普通 class 既没有显式 primary 也没有 secondary 时，HIR 合成 public 零参数 primary；声明至少一个 secondary 且 header 没有 `(...)` 时，class 没有 primary。registry 批准的 intrinsic class 不因省略源码 representation 自动获得零参数 constructor。

AST 必须保留每个 supertype 是否带 constructor argument list，不能再以“带括号就是 base、不带括号就是 interface”提前分类。HIR 解析 nominal kind 后应用：

- 有 primary 的 derived class 在 header 以 `Base(args...)` 委托；base class 没写时隐式委托编译器根类 `Any`；
- 无 primary、仅有 secondary 的 class 在 header 只写 bare base type，每条 terminal secondary 以 `super(args...)` 初始化它；header 上同时写 `Base(args...)` 是错误；
- interface 不能带 constructor arguments；class 最多有一个 direct base，其他 supertype 必须是 interface；
- 普通 class 若得到隐式零参数 primary，则 header 的 `Base(args...)` 属于该 primary。若 base 没有可用的零参数 constructor，不能由编译器补值或跳过 base 初始化。

### 1.4 `super` 成员调用

```kotlin
open class Base {
    open fun render(prefix: String = "base"): String = prefix
}

class Derived : Base() {
    override fun render(prefix: String): String =
        super.render(prefix) + ":derived"
}
```

`super.name<TypeArgs>(arguments...)` 使用 direct base application 的有效成员层完成 M16/M17 candidate resolution。它支持 overload、generic method、named/default/`vararg`/spread/尾随 lambda，并遵守 receiver 后显式实参、再 default/`vararg` 的顺序；receiver 是当前 `this` ref value，不重复求值。

只收集 class member declaration / inherited implementation；extension、property-like invoke、字段读取和 interface default 都不参与。目标是 base 静态视图中该 virtual family 的实际实现，最终调用强制 direct，即使同一对象最派生类型覆写了该方法也不能重新进入 vtable。abstract 且不存在 concrete base implementation的目标不可调用。

`super` 不是一等值，不能裸用、赋值、传参、捕获、safe-call、下标、invoke 或形成 callable reference。初始化上下文中也不能用 `super.method()` 绕过 `InitializingThis` 的禁止调用规则。

## 2. 初始化语义

### 2.1 class 的精确顺序

对源码构造表达式 `C(arguments...)`，顺序固定为：

1. 以 M16/M17 选择唯一 constructor 与完整 host type arguments；按源码顺序求值全部显式实参，再按参数声明顺序物化 `vararg` 和实际使用的 default；
2. 分配一次 exact concrete `C` 对象，写入最派生 TypeDescriptor，清零完整 payload，并立即把 initializing receiver 纳入精确 root；
3. 进入所选 constructor initializer。每条 `this` / `super` edge 都先按该 delegation clause 的源码顺序求值显式实参，再物化目标 constructor 的 default / `vararg`，随后在同一 receiver 上 direct 调用目标 initializer；
4. terminal primary 先完成 direct base constructor，再按 primary parameter 声明顺序把带 `val` / `var` 的参数写入本 class 字段；terminal `super` secondary 先完成 direct base constructor；根类没有 base 调用；
5. 执行本 class 的 common initialization sequence：body property initializer 和 `init` 按 class body 源码顺序交错运行；
6. 若 terminal 是 secondary，执行其 body；每次返回上一层 `this` delegation 后，继续执行该层 secondary body，直到最外层；
7. 最外层 constructor 正常返回后，构造表达式才产生对象引用。

因此继承链整体顺序是 base-before-derived；每一级的 primary properties 先于该级 body items。base constructor 的 secondary body 也是 base 初始化的一部分，必须在 derived 自有字段前完成。每个 class 的 common initialization sequence 在一次构造中恰好执行一次，不能因 `this` 链重复，也不能因多个 MIR constructor body复制而漏掉。

调用点显式实参属于 caller，而 delegation / property / `init` / constructor body 属于定义方。前者可在 suspend caller 中挂起；一旦进入步骤 2，整个构造链同步运行，不产生 coroutine state。initializer 内普通 managed call仍可分配、触发 GC 或抛异常。

### 2.2 字段就绪与名称作用域

HIR 在 lower class initialization 时维护非可选的 `InitializedFieldSet`：

- direct base 的全部字段在 base initializer 正常返回后就绪；
- 本 class 的 primary properties在 synthetic store 后按声明顺序就绪；
- 每个 body property 只有自己的 initializer 正常求值并完成 store 后才就绪；
- `init` 可以读写 inherited field、primary property和此前已经就绪的 body property；它不能读写当前步骤之后的字段；
- self read、forward read、通过 `this.name` 绕过顺序以及提前写 later `var` 都是定义处错误。编译器不能用分配时的全零字节当作合法 Scoop 默认值；
- secondary body 开始时本 class 与全部 base field 已就绪；它可以按普通 mutability规则读字段、写 `var`，但仍受 2.3 的 initializing-receiver限制。

primary parameter 作用域覆盖 header base delegation与 common initialization sequence。普通成员方法、secondary constructor 的 parameter/default/delegation/body都看不到 primary parameter；它们只能看到真正 stored property。secondary parameter 在自身 default 的前序参数规则、delegation 和 body 中可见，词法 binding 优先于同名 field；字段必须显式写 `this.name` 消歧。

primary base delegation只可读取primary parameter、owner type parameter与普通外部声明；带属性的parameter在此也读取parameter value，因为field尚未写入。secondary delegation只可读取该secondary自己的parameter及普通外部声明。两种delegation都没有`this` / field scope。

所有 class field identity 在 initializer lowering 前完成声明和类型解析，但“已声明”不等于“已初始化”。readiness 使用独立 typed state检查，不能靠从成员表暂时隐藏 later field、靠 arena 下标比较或在后端插入 null check实现。

### 2.3 `InitializingThis`、完成与失败

为落实 spec 8.2 “不得发布部分初始化对象”，class / struct constructor、class property initializer 与 `init` 使用独立上下文：

```text
ReceiverContext = OrdinaryThis | InitializingThis(InitializedFieldSet)
```

`InitializingThis` 只允许作为已经就绪 field 的 direct read / mutable write receiver。它不能转换为普通 `Expr`，因此从类型结构上排除：

- 作为实参或返回值、写入 local / global / field / array / aggregate；
- 被 lambda / anonymous / local function 捕获；
- boxing、upcast、`is` / `as`、`===`、operator / invoke receiver、callable reference或 `addressOf`；
- ordinary member、extension、virtual、interface 或 `super` method call，包括裸 member-call拼写。

读取一个已经就绪的 field 后，所得值是普通值，可以参与调用和存储；例如 `println(name)` 合法，但 `println(this)` 不合法，`this.handler()` 中 receiver 是已就绪 field 时可先正规化为 field read再调用。该边界不依赖 whole-program escape analysis或“final method大概不会泄漏”的猜测；未来若需要 init-safe callable，必须以新的 typed effect单独设计。

任一步骤抛异常时，后续 field / init / body与外层 secondary body都不执行，构造表达式不产生值；此前外部副作用不回滚。因为 `InitializingThis` 不可发布，失败对象只能从 constructor root变为不可达并由 GC 回收，不需要析构、回滚写入或 runtime invalid-object状态。struct 构造失败同样不产生值。

### 2.4 origin 与只求值一次

- constructor 调用及 delegation 都完整复用 M17 materialization：每个显式实参一次、全部显式实参先于 default、最终按参数顺序传递；失败候选不产生 initializer、temporary 或 default实例；
- delegation default 的 evaluation origin 是该 `this(...)` / `super(...)` clause；外层 `C(...)` 的 source location不会覆盖内层 delegation；
- property initializer、`init` 和 constructor body的普通表达式同时以自身源码节点作为 definition / evaluation origin。generic实例化或 common sequence复制不能把它们改成构造调用点；
- compiler-generated allocation、primary-property store、common-sequence edge与隐式 `super()`也必须有完整 synthetic origin，分别锚定 constructor / property / class declaration，不能使用空 span或从相邻表达式回退；
- receiver、constructor parameters及已写字段跨 allocation / call / exception edge存活时，沿普通 M15 live-set与relocation规则传播，恢复后不能重复 delegation argument或 initializer。

## 3. constructor identity 与决议

### 3.1 typed entity family

Export HIR 至少区分：

```text
ExportClassConstructorId
ExportStructConstructorId
ExportClassConstructorApplicationId
ExportStructConstructorApplicationId

ClassConstructorBody =
    Primary {
        base: BaseInitialization,
        primary_stores,
        common_initialization,
    }
  | Secondary {
        delegation: SecondaryDelegation,
        body,
    }

SecondaryDelegation =
    This { target, arguments }
  | Terminal { base: BaseInitialization, common_initialization }

BaseInitialization =
    Root
  | Super { target, arguments }
```

具体 Rust 结构可以切分，但必须使“`this` 分支不会执行 common sequence”“terminal 分支恰好带一份 common sequence”“每条 edge 有唯一typed target”成为类型可表达的不变量，不能使用 `Option<target> + bool runs_initializers` 拼装状态。

class / struct constructor source id、带完整 owner type arguments 的 application id、LocalConcrete initializer id以及普通 function / method id必须互不兼容。primary 也是真正 constructor candidate，有独立 id，不再用 `ClassId` / `StructId` 代替 owner。M17 的 `ExportParameterOwner`、default template key、调用域证明和 compiler exception core相应改为引用具体 constructor id / application；零参数异常构造目标仍是完整 concrete class constructor identity。

每个overloaded concrete constructor / initializer的link symbol由typed source constructor identity与owner specialization共同确定，并带稳定declaration discriminator；不能只编码class名或concrete参数类型，否则同signature错误恢复、不同source constructor或跨Cone实例会合并。

class field同样从“构造参数序号 / flattened index”中解耦：primary parameter、declared field 与 LIR layout field分别使用独立 id。带属性的 primary parameter显式关联一个 `ClassFieldId`；普通 primary parameter没有 field relation。body property直接拥有自己的 field id。base-first layout offset只由 LIR typed mapping给出，不能反向充当 source field identity。

### 3.2 overload、delegation 与 graph validation

`C(args...)` 的 nominal constructor candidate层包含该 class / struct 的全部可源码调用 constructor；abstract class constructor不进入普通 construction，但仍可作为 derived `super`目标。所有候选使用 M16/M17 同一 shape filter、constraint solving、MSC、default-count和vararg优先规则：

- secondary constructor只复用 host nominal type parameters；显式 `C<T>(...)` 的 type arguments只对应 host，constructor不增加第二组；
- generic nominal inference针对每个 constructor候选独立使用其参数、caller expected type与host bound；winner同时确定 exact nominal application和 constructor application；
- primary与secondary只按实际参数类型形成重载签名；名字、default、vararg marker和返回 host type不区分重载；
- `this(...)` candidate固定为当前 exact host application的constructor集合，`super(...)` candidate固定为 direct base application的constructor集合；二者仍做完整映射与applicability，不能按元数或源码顺序挑选；
- constructor callable reference继续不支持；`::C`不因出现多个constructor变成overload reference。

所有 delegation winner确定后形成每个 secondary 恰好一条 outgoing edge的图，再验证 termination与cycle。cycle诊断列出按源码edge组成的 constructor signature链；不能靠递归深度、运行期flag或 codegen递归崩溃发现。primary存在性、header base clause、abstract instantiation与重复signature也都在模板定义阶段检查，不能留到某个 concrete application。

### 3.3 generic 与跨 Cone

generic class / struct 的 property initializer、`init`、delegation和constructor body都是owner template implementation的一部分，可以引用host type parameter并在完整 application确定后实例化。constructor body不形成generic function实体；其单态化key是 kind-specific source constructor id加exact owner arguments。

`.slib` 的 Export HIR需要携带：

- constructor顺序、kind、source parameter interface、default templates、delegation target与body dependency closure；
- primary parameter到field的typed relation、declared body field的type/mutability/source order以及common initialization sequence；
- `super` call的direct-base typed target标记；
- generic initializer/body所需的typed依赖与definition origin。

字段名/type/mutability/order改变type layout与下游缓存；constructor signature/default/delegation改变source interface；initializer / `init` / body改变实现依赖。M23读取上游Cone时只能沿这些typed关系实例化或引用已发射constructor initializer，不能扫描class名称、method列表或link symbol重建。

## 4. IR 与 stage 边界

### 4.1 AST / parser

类型成员改为保留顺序的封闭枚举，而不是只收集 `methods`：

```text
ClassMember = StoredProperty | InitBlock | SecondaryConstructor | Function
StructMember = SecondaryConstructor | Function

PrimaryClassParameter {
    property: None | Val | Var,
    name,
    type,
    syntax,
}

ConstructorDelegation = This(arguments) | Super(arguments)
SupertypeSpec { type, constructor_arguments: None | Present(arguments) }
```

`constructor` / `init`继续作为声明位置的contextual keyword。`super`只在 delegation target与 `super.name(...)`专用parser入口接受；不先解析成普通 `Var("super")`。parser保留 property / init / constructor 的完整span、body item顺序、delegation colon与target span，用于稳定诊断和origin。

恢复边界包括下一个 class / struct body item、constructor body `}` 和 type body `}`。必须覆盖缺失参数括号、`:` 后非 `this`/`super`、缺失 delegation argument list、property缺type/initializer、`init`缺block、constructor缺body及 malformed `super` postfix；存在diagnostic时残缺AST整体丢弃。

### 4.2 Export HIR

Export HIR显式保存 constructor / field / initialization实体及各自typed id；普通 class不再把 `representation = Vec<ConstructorField>` 同时当parameter list、constructor identity和field layout。intrinsic representation仍是互斥sum，registry-approved intrinsic type不能携带普通stored body property、`init`或secondary constructor。

所有 constructor parameter interface使用 M17封闭calling-shape metadata。default在定义处绑定，delegation在目标winner确定后保存全量位置参数template；初始化体与普通generic body一样保存typed implementation依赖，但不伪装为 public method或 function value。

`super.method()` 保存 direct-base application和唯一callable；不能只保存同名 method或普通virtual family后让MIR猜“这是super”。初始化readiness与 `InitializingThis`只属于 hir-lower检查状态，成功Export HIR中的field read已经是typed field identity，失败状态不输出。

### 4.3 HIR lowering内部计划

以下结构只存在于 hir-lower，不跨stage：

- `ConstructorCandidateSession`与 delegation graph builder；
- `ClassInitializationPlan`、`InitializedFieldSet`及source-order member cursor；
- `InitializingThis` receiver capability；
- primary parameter / field name分层scope；
- common sequence复制或共享的commit计划。

candidate probe继续clone candidate-local state；失败constructor或delegation不能永久分配application、default实例、body temporary或graph edge。完成全部声明、field type和constructor signature后再lower initialization body，保证 forward read能报告“尚未初始化”而不是“未知属性”。

### 4.4 LocalConcrete HIR

LocalConcrete HIR接收fully specialized constructor与初始化体。class construction、constructor initializer和struct construction使用独立typed实体；允许保留的目标语义是：

```text
ClassNew { class, initializer, arguments }
ClassInitializerCall { receiver, initializer, arguments }
StructConstructorCall { constructor, arguments }
DirectSuperMethodCall { receiver, callable, arguments }
```

也可以把最后一项正规化为携带强制direct target的普通method call，但不得丢失direct证明。LocalConcrete HIR中不允许出现 source constructor name、`this`/`super` delegation语法、未验证graph、body member顺序、未实例化default/vararg、`InitializingThis`、field readiness或“稍后运行common init”的flag。

每个 concrete class initializer结构上说明它是 `This` delegating还是 terminal；terminal非可选地携带已展开的base call、field store和common initialization，delegating分支不携带第二份common sequence。abstract class也生成可由derived `super`调用的initializer，但没有合法 `ClassNew`入口。

### 4.5 MIR

MIR删除当前“递归展开base delegation arguments、拼成flattened fields、一次 raw `ClassInit`返回”的constructor实现。新基线为：

- `ClassNew` 分配 exact class一次，建立initializing receiver local，随后 direct managed call所选initializer；正常返回该receiver，异常路径不产生结果；
- initializer是返回 `Unit` 的hidden callable，参数为同一managed receiver加完整source parameter values。base / `this`调用只调用另一个initializer，不分配；
- primary property与body property使用普通typed field store，base field offset映射按class hierarchy完成。每次store都经过现有barrier插桩点；不能因“通常是年轻对象”跳过，因为initializer中的safepoint可能移动或改变代际状态；
- initializing receiver、constructor ref参数与先前写入的含ref aggregate按普通managed call / invoke exceptional root规则进入live set；allocation或任意initializer call后只使用relocated值；
- struct primary构造仍可成为aggregate construction；secondary constructor降低为hidden value-returning callable，先取得delegated完整value、执行body，再返回该值。优化可以消除copy，但不能改变按值identity、异常或body顺序；
- `super.method()`在MIR已经是direct target，不能按callee的open/override metadata再次改成virtual call；
- constructor initializer全部non-suspend，不进入M10 coroutine transform；普通suspend caller在调用 `ClassNew`前已完成自己的挂起实参。

MIR输出只保留allocation、direct call、typed field access/store与普通CFG / exception edge，不包含source initializer item、delegation graph或readiness状态。

### 4.6 LIR / codegen / runtime

LIR为 `ClassNew`使用exact concrete class layout / TypeDescriptor完成普通managed allocation；payload清零与header / allocation side metadata登记必须在第一条可能safepoint的initializer指令前完成。对象从分配起就可被collector按完整class `RefScan`扫描和移动；尚未初始化的managed leaf为全0，inactive值聚合的既有清零规则继续成立。

constructor initializer、base call与 `super`方法都是普通typed managed direct call，沿既有void-return target、statepoint、invoke root frame与异常CFG。LIR/codegen不按 `ctor.` symbol、receiver class名或第一个参数重新分类；initializer使用独立typed target ref。field store使用layout给出的字节offset和正常write barrier。

runtime不增加constructor API或初始化状态位。managed allocator已有的“完整对象清零、header及side metadata原子登记后才返回”契约即为所需基础；失败对象由普通可达性回收。runtime不得执行initializer、判断初始化完成、阻止访问或根据TypeDescriptor补默认字段值。

## 5. 诊断与恢复

至少提供以下稳定诊断：

- class primary parameter缺类型、非法modifier；secondary parameter写 `val` / `var`、constructor带type parameter / return / callable modifier / annotation、缺body；
- body property缺显式type或initializer、initializer类型不匹配，或与primary property / 其他字段重名；struct / interface中的stored body property或 `init`；intrinsic nominal携带初始化成员；
- class有primary时secondary未 `this`或直接 `super`；无primary时header base带arguments；struct secondary缺 `this`或使用 `super`；
- `this` / `super` delegation无匹配、歧义、duplicate constructor signature、abstract class普通实例化；diagnostic列出constructor kind、完整signature、候选层和M16结构化失败原因；
- constructor delegation自环 / 多节点环，显示有序signature edge链；edge无target时不再运行cycle pass制造二次错误；
- field self / forward read或write，指出当前初始化步骤与尚未就绪field；primary parameter在method / secondary中不可见；
- `InitializingThis`逃逸、捕获、普通/extension/virtual/interface/`super`调用、cast/box/addressOf等，统一说明“initializing receiver cannot escape before construction completes”；
- 裸 `super`、无base的 `super.name`、不存在 / 不适用 / ambiguous base member、abstract target、`super.field`、`super::name`、safe或qualified interface super；
- initializer / constructor中的挂起调用或 `return`；异常本身合法，不产生“字段可能未初始化”的后续伪诊断；
- base header与secondary `super`组合错误、interface带constructor arguments、多个base class或base constructor缺失。

parser在每个type member恢复，HIR在无效field / constructor上保留typed invalid-declaration集合并跳过依赖body；不能用空body、Unit type、零参数target或首个overload作为占位继续输出模块。存在任一诊断时整个HIR输出失败，后续stage看不到残缺constructor graph。

## 6. 测试计划

### 6.1 parser / AST

- ordinary / `val` / `var` primary parameter与required/default/`vararg`排列；显式空primary与无primary的区别；
- class / struct secondary constructor、`this` / `super` / omitted delegation、body；
- class member中property / init / constructor / function交错顺序，bare / argument-bearing supertype保真；
- `super.method<T>(named = ..., *spread) { ... }`与所有禁止的 `super` postfix；
- 每种malformed参数、delegation、property、init、body及多错误member恢复。

### 6.2 constructor 与 delegation

- primary + 多个secondary直接/间接 `this`，无primaryclass的explicit/implicit `super`，struct secondary；
- primary/secondary overload、generic host inference/bound、named/default/`vararg`/spread及MSC；
- primary与secondary duplicate signature、缺失/歧义target、self/two-node/long cycle；
- abstract base constructor只可由super使用，intrinsic type不产生普通constructor；compiler exception zero-arg target仍完整；
- observable order锁定caller args → delegation args/default → base → primary stores → interleaved common items → inner/outer secondary body，每个表达式只执行一次。

### 6.3 body property、`init` 与安全边界

- primary plain parameter用于base args / property initializer / init，但在method与secondary不可见；与body field同名时无receiver名称取parameter、`this.name`取field；
- inherited、primary、earlier body fields可读，self/later field读写拒绝；body `var`后续赋值与body `val`只读；
- property initializer和多个 `init`按源码交错，method / constructor声明不影响顺序；
- 每一种 `InitializingThis`合法field read/write与非法逃逸/capture/call/cast/addressOf；field value上的后续调用保持合法；
- property/base/init/secondary任一点throw时后续步骤不执行，catch到原异常且没有construction result。

### 6.4 `super.method()`

- base overload/generic/default/`vararg`选择；base的base继承实现；final/open/final-override direct目标；
- derived override调用super得到base实现，golden确认不走vtable；
- extension/property-like同名不参与，abstract/no-base/ambiguous/非法postfix诊断；
- 初始化上下文中的super调用被受限receiver规则拒绝。

### 6.5 golden 与端到端

AST / ExportHir / LocalConcreteHir / MIR / LIR golden分别锁定：

- constructor、application、field和initializer使用kind-specific typed id；
- source member顺序只形成typed common sequence，LocalConcrete以后无source `init` / delegation / readiness；
- class只分配一次，base / `this` initializer共享receiver，primary/common sequence恰好一次；
- `super`是direct call，当前flattened-constructor捷径完全消失；
- initializing receiver和已初始化ref字段跨allocation、managed call与异常边拥有完整root / relocation；未写field在scan前已清零。

新增fixture分为 `tests/fixtures/m19-constructors/`、`m19-initialization/` 与 `m19-super/`。每个目录同时包含独立positive、negative以及 generic + default + closure + suspend-caller + exception + moving GC组合fixture。stress必须在initializer中触发多次allocation和对象移动，断言同一most-derived identity、base / derived ref字段及secondary参数全部仍正确。M1至M18全部回归。

## 7. 实现顺序与提交门

1. 同步语言 / runtime / 实现spec，加入typed constructor / field / initialization identity，拆开parameter、field与layout index；
2. 完成primary普通参数、ordered type member、secondary / init / body property / supertype / `super.method` AST与parser恢复；
3. 把class / struct全部constructor接入M16/M17候选、default与generic nominal inference，完成duplicate、delegation target与cycle检查；
4. 实现class body property / `init` common sequence、field readiness、primary parameter scope与 `InitializingThis`限制；
5. 生成LocalConcrete class initializer，替换MIR flattened-constructor捷径为一次allocation + same-receiver base/this initializer call，补齐异常与GC roots；
6. 实现struct secondary constructor的typed value-returning链与validation body；
7. 实现 `super.method()` direct-base resolution、强制direct target与generic/default/dispatch组合；
8. 补齐全量golden、fixture、moving stress及M1至M18回归。

每个编号都是可独立提交的功能切片。完成一批变更后先执行 `cargo fmt --all` 与 `cargo clippy --workspace`，再运行对应crate测试、stage golden和fixture；全部通过并单独提交后才进入下一切片。每个提交必须保持workspace可构建、既有fixture通过，不保留旧flattened ctor与新initializer双轨、按名称找constructor / super、临时 `Option<FieldType>`、未检查 `this`逃逸或等待后续提交补齐的root plan。

M19只有在以下条件同时满足时完成：每个construction / delegation都引用唯一typed constructor；base / derived在同一object identity上按规范顺序各执行一次；body property与 `init`不能观察未初始化field；半初始化 `this`不能逃逸或动态调用；普通实参/default/exception/suspend-caller/moving GC保持既有契约；LocalConcrete HIR以后不再含source初始化计划，MIR不再递归拼接继承字段模拟构造。

## 8. 明确不做

1. M20的class upper bound与partial type argument；nominal variance、projection与capture conversion已确定不进入语言；
2. M21的计算/extension/interface/delegated property、自定义getter/setter、Option-typed initializer omission（不含`lateinit`）、object/companion、constructor/property visibility与annotation、interface default implementation；
3. interface-qualified `super<I>.method()`；它依赖M21 interface default implementation的冲突选择；
4. nested / inner class及捕获outer receiver的constructor；object / companion / top-level初始化顺序和循环仍由M21定义；
5. `sealed`、class/interface delegation `by`、property delegate协议与reflection参数；
6. async initializer、suspend constructor / accessor或“构造尚未完成”的future对象；初始化始终同步；
7. init-safe effect、constructor factory callable reference、placement new、用户自定义allocation、析构 / finalizer或失败构造回滚；
8. constructor inline、field-store消除、terminal-chain合并和new-object barrier elision等优化；未优化基线先保证identity、顺序、异常与moving GC正确。
