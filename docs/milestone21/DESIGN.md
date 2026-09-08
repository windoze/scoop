# M21 设计：属性、对象、初始化与可见性

版本：0.2（草案）

对应`docs/ROADMAP.md`的M21。M21在M19的构造/初始化顺序、M18的property-like调用分区和M20的exact generic模型之上，补齐完整property语义、`object`/`companion object`、interface default implementation、顶层初始化、`const val`与可见性，并把这些能力以typed identity贯穿Export HIR、LocalConcrete HIR、MIR、LIR和runtime启动协议。

Scoop不背负JVM字段零值、反射property descriptor或class-initializer ABI的兼容负担。本里程碑因此明确删除`lateinit`：源码不能声明“静态类型为`T`、运行时却可能尚无`T`”的property。需要延后取得值时，声明`var value: Option<T>`或`var value: T?`；省略initializer的这种形态精确定义为`None`初始化，读取结果始终仍是`Option<T>`，由普通`when`、`?:`或`!!`处理。编译器不生成隐藏initialized bit、特殊getter检查或`UninitializedPropertyAccessException`。

## 0. 关键决策与范围

- property是“声明type + getter + 可选setter”的逻辑实体，不等同于field。stored、accessor-only、delegated、const和extern/raw storage使用互斥的typed representation；accessor-only内部再让每个getter/setter以`Body`/`AbstractSlot`封闭sum表达，完整容纳interface的混合default/abstract形态；
- 所有非局部property都必须写显式type。initializer不参与导出签名推断，getter/setter也不能各自形成不同的property type；
- 普通stored property必须有initializer。唯一省略initializer的stored形态是无accessor的`var p: Option<T>`/`var p: T?`，等价于在同一源码初始化位置写`= None`。`lateinit`永久不进入语言；
- 带initializer的class/object property一定拥有backing field；无initializer而带accessor的property一定是computed property且没有field。是否生成field不根据accessor body里是否碰巧出现`field`反推；
- custom accessor与property delegate都是ordinary、non-suspend callable边界。访问property永远同步；异步获取必须暴露`suspend fun`；
- delegate协议不携带`KProperty`、反射对象或property name。可选`provideDelegate()`不接收正在初始化的owner，`getValue`/`setValue`只接收普通、已完成初始化的receiver；
- interface method与property accessor可以提供default body。class hierarchy实现优先，其次选择唯一most-specific interface default；互不相关的多个default必须显式override。`super<I>.member`只direct调用明确父interface的default；
- top-level与nested `object`、每个nominal至多一个`companion object`都是非generic singleton ref type。companion不捕获宿主实例或宿主type parameter，也不按generic宿主application复制；
- 普通top-level stored/delegated property由编译器管理、可包含managed ref并进入global root表；`@Global`/`@ThreadLocal`只表示M12的显式可寻址GC-free raw storage，不再用来描述普通property backing storage；
- top-level runtime initializer在`main`前完成；object/companion在首次需要其非const值或成员时同步、线程安全地初始化。二者共用exactly-once initialization cell、失败记忆与循环检测；
- `public`/`internal`/`private`/`protected`形成显式access domain。所有允许visibility的声明默认`internal`，对外API必须逐项显式写`public`；`internal`精确定义为Cone内，top-level `private`为文件内，member `private`为声明type词法域，`protected`仅用于class成员/constructor；
- M21同时开放static nested nominal/object声明，因为companion与member visibility需要同一owner模型。`inner` class及隐式outer receiver仍不进入本里程碑；
- property、accessor、object、singleton value、backing/delegate/global storage、initialization cell、interface slot、default body与visibility witness分别使用不同typed id，不以名称、FQN、symbol或arena平行下标互相恢复。

## 1. 统一 property 模型

### 1.1 源码形态

以下形态构成M21的property表面：

```kotlin
class Account(initial: String) {
    val id: String = initial

    var displayName: String = initial
        get() = field
        private set(value) { field = value }

    val label: String
        get() = "account"

    var selected: Account?            // 等价于 = None

    val cached: String by cache()
}

val <T> Box<T>.content: T
    get() = unpack(this)
```

每个非局部property必须显式写`: T`。property自身不能声明callable式value parameter、default/`vararg`或返回类型；extension property可以在`val`/`var`之后声明自己的type parameter列表，见1.6。

合法representation由语法直接决定：

| 源码形态 | representation | backing storage |
|---|---|---|
| `val/var p: T = expr`，可跟custom accessor | stored | 一个field或global slot |
| 无initializer、无accessor的`var p: T?`/`Option<T>` | stored，synthetic `None` initializer | 一个field或global slot |
| `val p: T get() ...` | accessor-only read-only | 无 |
| `var p: T get() ... set(...) ...` | accessor-only read-write | 无 |
| `val/var p: T by expr` | delegated | delegate field/global/local，不是`p: T` field |
| `abstract val/var p: T` | accessor-only，全部为abstract slot | 无 |
| interface中省略部分accessor body | accessor-only，每个slot独立default/abstract | 无 |
| `const val p: T = constExpr` | compile-time const | 无runtime storage |
| 合法`@Extern val/var p: T` | native storage view | 外部C symbol |

同一声明不能同时使用initializer、`by`、`abstract`与computed accessor。`const`和`@Extern`不能带accessor或delegate。property名称在同一owner内唯一，不能按type、mutability或accessor signature重载；public/internal top-level property的owner namespace是package，file-private top-level property的owner还包含source file，因而不同文件可各自声明同名private property。function与property仍可同名，并沿M18的function-like/property-like分区决议。

### 1.2 不提供 `lateinit`

`lateinit var service: Service`是稳定的编译错误。Scoop没有平台null、JVM默认字段值或需要兼容的reflection flag，因此不允许把缺失状态藏在一个静态非Option type后面。

延后赋值写成：

```kotlin
class Application {
    var service: Service?       // 初始化步骤在这里写入 None

    fun install(value: Service) {
        service = Some(value)
    }

    fun run() {
        val current: Service = service!!
        current.start()
    }
}
```

规则如下：

- 省略initializer的stored shorthand只接受`var`，且解析后的最外层type必须是core的exact `Option<U>` application；`T?`只是同一type的语法糖；
- shorthand产生真正的typed `None<U>`值并执行普通field/global store。field从该store之后就绪，不存在“已声明但未初始化”的runtime状态；
- `val p: T?`没有后续可写路径且不表达有用状态，仍要求显式initializer；`var p: T`也必须显式初始化或改写为computed/abstract property；
- 若无initializer但存在任一custom accessor，则声明按computed property解释，不获得上述shorthand。需要optional backing field和custom accessor时必须明确写`= None`；
- generic type parameter即使某个实例化恰好是`Option<U>`也不能触发shorthand；声明表面必须已经解析到最外层`Option` nominal identity，不能在单态化后改变property representation；
- `!!`失败继续抛`UnwrapException`，这是普通Option unwrap，不是late-init专用异常。

### 1.3 stored property、backing field与accessor

带initializer的class/object property进入M19 common initialization sequence；optional shorthand把synthetic `None` store放在同一声明位置。primary-constructor property仍由constructor参数store初始化，且只拥有default field accessor，不能在参数列表内附带accessor body。

stored property的implicit getter读取backing field；`var`的implicit setter写field并走普通write barrier。custom accessor语法为：

```kotlin
var count: Int = 0
    get() = field
    protected set(next) {
        field = normalize(next)
    }
```

- getter没有显式参数，结果必须可赋给property type；block body必须在所有正常路径返回该type；
- setter恰好有一个不可重新绑定的参数，省略名称时为`value`，显式名称只改变accessor body中的binding；setter返回`Unit`，不能写显式返回类型；
- `field`是只在当前stored accessor直接body中可见的contextual backing-field place。它不是property、local或first-class reference，不进入nested lambda/local function的词法capture；
- `val`不能声明setter。`var`可以只customize一个accessor，另一个使用implicit body；
- initializer先完成field store，之后才可能从普通程序访问accessor。M19的constructor/property initializer/`init`上下文对`this.p`及bare member `p`只允许已就绪backing field的direct访问，不调用custom/default accessor；computed/delegated property在initializing receiver上不可访问；
- backing field没有源码名称、visibility或独立annotation target，只能由所属initializer、accessor和M19 initialization checker通过typed id访问。

### 1.4 computed、abstract与interface property

interface以外，computed `val`必须提供getter，computed `var`必须同时提供getter与setter。class中的property可使用与method相同的`final`/`open`/`abstract`/`override`规则：普通property默认final，`abstract`无initializer/delegate/accessor body且把全部required accessor正规化为abstract slot，`override`默认open，`final override`关闭后续覆写。

interface不允许stored field、initializer、delegate或`init`。它可以声明：

```kotlin
interface Named {
    val name: String

    val displayName: String
        get() = name

    var enabled: Boolean
        get() = true
        set(value) { record(value) }
}
```

interface中每个required accessor独立分类：有body的是default implementation，声明中没有提供body的则是abstract accessor slot。因此无body的`val`形成一个abstract getter；无body的`var`形成abstract getter与setter；只写其中一个body时，另一个仍是abstract。property override要求名称和property type exact一致；`var`可以override `val`并额外提供setter，`val`不能override `var`。每个被覆写accessor的suspend、safety、GC effect与operator等contract必须匹配；property accessor本身永远non-suspend。

class/object/value type用一个显式`override` property满足interface或base obligation。stored、computed或delegated representation均可作为实现，只要所需accessor、visibility和type完整。value type仍不能实现`var`property，因为setter会引入可变`this`语义；它可以用computed `override val`或继承无需修改value的interface default getter。

### 1.5 property read、write与place

property read与write分开做typed resolution，但一旦选中逻辑property就不能因某个accessor不可见或缺失而退回较低候选层：

- read要求getter存在且可访问；write要求同一property存在setter且setter可访问；
- explicit receiver先求值一次。getter没有源码value argument；setter在receiver后求值右值一次，再以property type传入；
- member property层先于extension property层；不可适用的extension候选不阻断下一层，member同名property已经选中后则不会因缺setter改找extension；
- extension property在其可见extension scope中按receiver applicability与M16的MSC选择；普通member property不能重载；
- `?.property`只在`Some`分支调用getter并将结果再包一层Option，不展平已有Option；safe write不提供新语法；
- assignment、`++`/`--`与复合赋值复用M18 typed place plan。一个property place非可选地携带一次receiver evaluation以及已经解析的getter/setter target；MIR不再次按名称配对accessor；
- M18 property-like `invoke`先执行这套getter resolution，再对所得值执行一次`invoke`resolution。extension property作为既有第5/6 c-level来源插入，不改变分区顺序；
- M21仍不引入property reference类型或`::property`。`::name`继续只引用function declaration。

### 1.6 extension property与generic约束

extension property只允许在top level声明，且没有backing field：

```kotlin
val String.lastIndex: Int
    get() = size - 1

val <T> Box<T>.content: T
    get() = unpack(this)
```

它可以是computed property，或在非generic形态使用共享的top-level delegate。不能写普通initializer，也不能是`abstract`/`open`/`override`。extension receiver是不可重新绑定的隐含按值参数；ref/value receiver、capture和`addressOf(this)`沿用spec 3.3。

generic extension property的全部type parameter必须能仅由receiver exact静态type和声明bound唯一确定。property read没有显式type-argument list，不能用expected result type补出只出现在返回type中的参数；setter value也不能让read/write得到不同application。成功后getter/setter按完整receiver application单态化，LocalConcrete HIR前不保留type parameter。

generic extension property首版只允许computed representation。delegated generic extension会为未来下游产生的每个specialization引入跨Cone静态storage与lazy initialization identity，留待M23拥有真实`.slib`实例归属后再设计；不能用一份type-erased delegate或runtime dictionary绕过。

## 2. Reflection-free property delegate

### 2.1 协议

M21为delegate增加三种独立于普通operator表的typed role。声明必须显式写`operator`，必须ordinary、non-suspend、non-generic，且不能使用default或`vararg`：

```kotlin
operator fun provideDelegate(): EffectiveDelegate
operator fun getValue(thisRef: R): T
operator fun setValue(thisRef: R, value: T): Unit
```

这些可以是delegate type的member或可见extension。`provideDelegate`可选；若不存在适用候选就直接使用原delegate值，若存在则必须选出唯一候选，其结果成为effective delegate。`val`要求唯一`getValue`；`var`还要求同一effective delegate上的唯一`setValue`。普通同名function若没有对应typed role不参与协议。

`R`由property位置确定：class member为声明class application，object/companion为其singleton type，extension property为extension receiver type，top-level与local delegated property为`Unit`。普通subtyping、boxing、generic owner substitution与M16 candidate-local resolution照常生效；getter结果必须可赋给声明type，setter以声明type传值。

非局部delegated property仍必须显式声明type。local delegated property可以省略type：先在没有result expected type的条件下选出唯一`getValue`role，再把其concrete结果type作为local property type；`var`的`setValue`必须接受同一type。不能根据多个get/set组合反向猜一个type，也不能让setter单独改变getter已经确定的type。

协议故意没有`KProperty`、property name、annotation数组或其他反射descriptor。需要名称、source location或注册key的delegate必须在`by`表达式中显式传入；编译器不为每次访问分配metadata对象。`provideDelegate()`也不接收owner，从而不会在class/object初始化期间发布受限`InitializingThis`。

### 2.2 初始化、storage与求值

`val/var p: T by expr`严格执行：

1. 在property声明对应的初始化位置求值`expr`一次；
2. 若选中`provideDelegate`，对步骤1的值调用一次；
3. 把effective delegate存入隐藏的typed storage并标记该初始化步骤就绪；
4. 此后每次read读取该delegate一次，物化普通receiver value并调用`getValue`；write再按源码顺序求值右值并调用`setValue`。

class member delegate使用hidden field并进入M19 common sequence/readiness/GC scan；object member同理。top-level与非generic extension delegate使用managed global root slot和M21 eager initializer。local delegate使用不可重新绑定的hidden local；它可按普通`val`capture规则被closure捕获。struct/enum member不能有delegate hidden state，仍只允许computed `val`。

delegate expression、`provideDelegate`及access调用都可能分配、抛异常和触发GC，但不能挂起。class/object delegate初始化失败按普通construction/singleton failure处理；top-level失败使startup失败；local失败只终止当前控制流。delegate不是特殊lazy primitive：`lazy { ... }`可由core/stdlib定义持有closure与状态的普通delegate type。

## 3. `object`、companion与nested declaration

### 3.1 singleton object

```kotlin
object Registry : Base(), Service {
    val entries: MutableArray<String> = MutableArray()
    init { registerDefaults(entries) }
    override fun start() { ... }
}
```

`object O`同时声明一个nominal ref type和一个singleton value；两者使用不同typed id。object不能声明type parameter、primary/secondary constructor或`inner` modifier，可以继承至多一个class并实现interface。base constructor先执行，随后stored/delegated property与`init`按源码顺序执行；整个过程复用M19的exact allocation、readiness、受限`InitializingThis`、异常和moving-GC规则。

引用`O`本身、读取/写入其非const property或调用member，都会先通过初始化gate取得已经完整构造的唯一ref。object的type可用于type annotation、interface/class relation和RTTI，但不存在普通`O()`构造。只引用其static nested type/declaration或`const val`不初始化外层object；访问nested object只初始化nested unit，除非其initializer显式引用外层。singleton identity在进程生命周期内稳定为“同一逻辑对象”，物理地址仍可被moving GC更新；`O === O`为true。

### 3.2 companion object

class/struct/enum/interface/object body至多声明一个companion：

```kotlin
class Box<T>(val value: T) {
    companion object Factory {
        fun emptyInt(): Box<Int> = Box(0)
        const val version: Int = 1
    }
}
```

省略名称时声明名为`Companion`；显式名称和`Companion`别名都可用于限定访问。companion是独立singleton type/value，不捕获宿主实例`this`，不能访问宿主primary parameter或type parameter，也不为每个`Box<T>`application复制。companion自身不能声明type parameter，但其中的普通generic function仍按M14 non-virtual规则工作。

`Box.Factory.emptyInt()`和无冲突时的`Box.emptyInt()`引用同一member；后者只是type-qualifier lookup中的companion forwarding，不生成static duplicate。companion member不进入`box.emptyInt()`实例查找，也不被derived class继承。type自身的nested declaration/constructor namespace优先；与companion forwarding同名而无法唯一分类时要求显式写companion名称。

读取companion内`const val`不触发初始化；其他member/property访问先初始化companion。宿主class的构造和companion初始化互不隐式触发，只有各自initializer中的显式引用形成依赖。

### 3.3 static nested declaration

M21允许class/struct/enum/interface/object body包含命名class/struct/enum/interface/object；它们是static nested declaration：

- nested type不带隐式outer receiver，不继承outer primary parameter或type parameter；需要关联时显式声明自己的type parameter并传值；
- generic outer通过声明qualifier引用nested类型，例如`Outer.Nested<Int>`；这里`Outer`是owner qualifier，不是裸generic application；
- nested declaration的effective visibility是自身visibility与全部owner access domain的交集；
- nested name形成owner-scoped nominal identity，不能用短名/FQN文本代替typed owner relation；
- `inner class`、匿名object expression、local object/type及捕获outer ref的constructor明确留在本里程碑之外。

## 4. Interface default implementation

### 4.1 default与obligation

interface function有body即为default implementation，无body即为abstract obligation；property按accessor分别判断。private interface member必须有body，只是default body的helper，不产生itable slot、不被继承或override。

对每个concrete class/object/value type及每个exact interface application，HIR按typed slot执行：

1. class hierarchy中最近的concrete override获胜；abstract override可明确保持obligation并压制更早class implementation；
2. 否则在可达interface declaration中删除被更specific subinterface override的候选；
3. 剩余唯一concrete default获胜；只剩abstract声明则形成未实现obligation；
4. 多个互不相关concrete default冲突，owner必须声明显式override；不能按implements列表顺序、import顺序或link顺序任选；
5. getter/setter/function slot独立应用上述算法，但一个源码property override仍必须整体满足1.4的type/mutability规则。

concrete class不能留下abstract obligation；abstract class可以把完整typed obligation继续交给derived。itable entry可以direct指向class/value implementation、interface default body或必要的typed adjust thunk，不复制default源码body到每个实现者后再靠名称合并。

### 4.2 qualified interface `super`

显式override/default body可写：

```kotlin
override fun render(): String = super<Left>.render()
override val name: String get() = super<Named>.name
```

qualifier必须是当前owner显式列出的direct superinterface exact application，并且目标accessor/function在该interface上具有concrete default body。调用强制direct，不经itable，不收集extension/property-like候选，也不接受抽象目标。setter default可通过`super<I>.p = value`调用。

该语法只在普通member/accessor body中合法；initialization context仍不能借它调用受限receiver。`super<Base>.member`不能替代M19的class `super.member`；裸`super<I>`、callable reference、safe/index/invoke形态仍非法。interface default body也可以只对其direct superinterface使用同一语法，以显式组合父default。

## 5. 可见性与annotation

### 5.1 access domain

默认visibility为`internal`。省略modifier不是“暂时未知”或“继承上下文”，而是在AST→HIR时确定地正规化为`Internal`；setter omission是唯一不同规则，它继承所属property visibility。合法位置与含义为：

这是有意偏离Kotlin的安全默认：public声明会进入`.slib`语义表面、约束下游源码兼容性，并让generic template/default/dispatch layout成为跨Cone契约；Scoop没有必须靠默认public维持的平台或inline ABI负担。要求显式`public`使API surface可由源码检索、review和工具稳定审计，新增未标注声明不会意外扩大兼容承诺。

| visibility | top level | nominal/member/nested | 含义 |
|---|---|---|---|
| `public` | 是 | 是 | 所有依赖方可见，受owner domain约束 |
| `internal` | 是 | 是 | 当前Cone内可见 |
| `private` | 是 | 是 | top-level为当前source file；member为声明type及其nested lexical declarations |
| `protected` | 否 | 仅class member/nested/constructor | 声明class及其subclass body可见 |

visibility覆盖top-level nominal/function/property/object，以及nominal中的method/property/nested declaration/constructor；local declaration、parameter、`init`块和accessor parameter不能声明visibility。`main`按入口契约发现，不要求public，也不因internal而进入`.slib`导出表面。

enum variant与struct/enum表示字段继续是固定public representation surface，不在M21逐字段声明visibility。这不是default-public旁路：只有外层struct/enum本身显式`public`时，owner domain交集才会让这些表示成员对下游可见；选择公开该值类型即选择公开其可构造/匹配表示。

access不是一个可用整数比较的单轴枚举。HIR分别保存`DeclaredVisibility`、与owner相交后的`EffectiveLookupDomain`，以及virtual/interface override使用的`SlotContractDomain`；三者不能互换。普通member/nested的直接查找域恒为声明modifier与owner effective domain的交集，因此public member不能把internal/private owner作为名称泄漏；但internal/private concrete type中的显式`public override`仍可实现public slot，经base/interface静态类型分派时遵守slot contract。

候选收集只把当前访问点落在effective domain内、且能产生对应typed witness的声明送入applicability/MSC；同名但不可访问的声明保留用于诊断，不会单独让更高候选层变成“已有可应用候选”而遮蔽较低层。这个规则不改变setter narrowing：一旦可见logical property已经作为read/write来源被选中，缺失或不可访问的setter就是该property上的assignment错误，不再改选extension或其他较低层。

protected显式receiver还必须具有当前访问subclass或其子类的静态type；不能在subclass中通过任意`Base`实例读取protected member。companion/nested body没有隐式outer `this`，但可在满足同一receiver规则时访问。top-level、struct/enum/interface/object member不能声明protected。

private member不进入继承、override、vtable/itable或member candidate；同名derived member是新声明。internal open member只能在同一Cone override；下游Cone既看不到也不能替换其slot。

interface member同样默认internal。internal interface可以拥有internal abstract/default contract；public interface的abstract/default contract必须显式`public`，遗漏modifier得到internal后是定义错误，不能静默升级为public。private interface helper必须有body且不进入itable/override；protected interface member非法。这个约束保证一个可由下游实现的public interface不会携带下游不可见的hidden obligation。

同理，abstract member的slot contract必须覆盖owner的typed `InheritanceDomain`：public abstract/open class中的abstract member至少显式protected或public，使所有合法下游subclass都能实现；internal/private owner可以保留internal obligation。不能用下游不可见的abstract member把public type偷偷变成“事实上sealed”；受限继承要等待并使用正式`sealed`语义。已有body的internal open member不形成未实现obligation，下游只会继承该实现且不能override。

### 5.2 override、setter与signature exposure

override省略visibility时也严格得到internal，不继承或复制base declaration的visibility。override coverage比较声明visibility所形成的slot contract，而不是已经被concrete owner收窄的direct lookup domain；它必须覆盖每个被覆写slot，不能收窄。因此实现public contract必须写`public override`，即使实现class本身是internal/private；该member的直接名称访问仍受owner限制。实现protected contract也必须显式选择足以覆盖它的visibility，允许显式扩大。primary-constructor/body/computed/delegated property和method遵守同一规则。

getter visibility就是property visibility，不能单独修改。`var` setter可省略modifier继承property visibility，或声明一个不更宽的`private`/`internal`/`protected` visibility；`public set`只有property本身public时有意义。读取候选选中property后，setter不可见产生assignment诊断，不改选其他property。

public/internal/protected declaration的签名中出现的parameter、return、receiver、property、base/bound、annotation type及default直接绑定实体必须覆盖该声明的完整direct lookup domain，以及它承担的更宽slot contract domain。该统一规则继续支撑M17的`CallDomain ⊆ AccessDomain` witness；const folding、companion forwarding、default或implicit accessor都不能绕过visibility。普通function/accessor/default-interface body是实现依赖，可以调用同owner的narrower实体，不因此导出它们。

### 5.3 constructor与property modifier

primary constructor visibility/annotation使用显式`constructor`关键字：

```kotlin
class Token private constructor(val raw: String)
class UnsafeBox @Unsafe constructor(value: Ptr<Unit>)
```

class中无`constructor`关键字的普通header constructor使用默认internal。class/struct secondary constructor在`constructor`前接受visibility与annotation，省略时同样internal。class隐式零参数constructor也合成为internal，再与owner effective domain取交集；因此`public class C`不会顺带获得public construction API，需要写`public class C public constructor(...)`。struct primary constructor与字段共同构成固定public representation entry，只与owner domain取交集且不能单独声明visibility；enum variant constructor同理。object没有constructor visibility。

class primary property参数可在`val`/`var`前声明visibility与`override`，普通primary parameter不能带member visibility。struct/enum字段继续固定public，不能借M21 modifier改变value representation API。

property declaration上的custom annotation只属于逻辑property，不自动复制到accessor/backing/delegate storage。accessor可以在`get`/`set`前单独写annotation；parser保留完整目标与span，HIR按annotation class target验证。M12核心annotation矩阵扩展为：

- `@Unsafe`/`@Safe`可用于constructor和explicit accessor；调用constructor或读写对应accessor按普通safety context检查；delegation到unsafe constructor要求当前constructor同样unsafe；
- `@NoGC`可用于explicit accessor及struct secondary constructor，但仍必须通过完整GC-free body/signature检查。class/object receiver是ref，因此其accessor/constructor不能满足`@NoGC`；
- `@Extern`、`@Global`、`@ThreadLocal`只用于6.1的top-level native/raw storage；`@CallingConvention`只用于既有native callable；普通property、delegate、object或constructor不能伪装成FFI symbol；
- `@Intrinsic`只有registry明确列出的target合法，不因新增property/object语法自动扩大。

accessor的safety/GC effect属于accessor slot contract；override/default实现必须一致。property访问HIR直接携带该effect，不能到MIR再从annotation名称恢复。

## 6. 顶层property、`const val`与初始化

### 6.1 top-level分类

M21把M12临时`GlobalDecl`拆为语义互斥类别：

- ordinary stored top-level `val/var p: T = expr`及optional shorthand：compiler-managed hidden storage，可含ref，读写经property accessor，不可`addressOf`；
- ordinary computed/delegated top-level property：分别无property storage或只有hidden delegate storage；
- `const val`：只存在编译期值；
- `@Extern val/var`：外部C data symbol view，无initializer/accessor/delegate；
- `@Global var`/`@ThreadLocal var`：显式可寻址GC-free raw storage，保持M12 constant initializer与FFI约束，不是ordinary backing field。

因此“所有top-level `var`都必须带`@Global`/`@ThreadLocal`”的M12临时限制退役；限制只属于请求raw storage/address identity的声明。普通top-level mutableproperty允许managed type，但仍建议把成组全局状态封装在object中。ordinary property不能与三个storage annotation混用。

### 6.2 eager top-level initialization

每个需要runtime求值的ordinary top-level stored/delegated property形成独立typed `InitializationUnitId`，并拥有hidden storage、exactly-once cell和initializer callable。只有HIR已经证明无需执行Scoop代码、无需读取ordinary property且可直接编码为目标静态数据的literal/内建纯常量表达式、immortal String ref和optional `None` shorthand属于static-representable initializer，可在image中直接表示而不进入runtime执行图；不能仅因优化器碰巧fold出常量就删除有语言可观察求值的unit。M12 raw storage仍只接受其既有static initializer；computed与const也没有unit。

启动顺序为：runtime/GC与主线程就绪 → 登记本image全部managed global root和init unit → 按canonical key逐个ensure全部top-level eager unit → 调用`main`。同一Cone文件没有编译顺序，因此canonical key编码完整typed declaration identity：owner chain、package、declaration kind与name；只有file-private owner再加入标准化Cone-relative source identity以区分不同文件同名声明。key不使用输入枚举顺序、arena id或host绝对路径。独立initializer的副作用按该顺序可观察。M23加入多Cone后先按无环dependency graph初始化上游Cone，再给key前缀stable Cone identity并沿用同一Cone规则。

initializer直接读取另一个top-level property或singleton时，getter先ensure目标；因此真实执行可沿依赖边提前初始化目标，但每个unit仍恰好一次。HIR只为文本上属于该unit并经脱糖展开的property/delegate expression、object base argument和`init`body中的直接typed init-unit引用建立依赖图并对结构环给定义处诊断；不递归进入被调用的普通function、constructor/default body、virtual/interface call或FFI回调。后者造成的间接重入依赖运行期控制流，由6.4的gate确定性检测，不能靠不完整call-graph分析误报或假定不发生。

top-level initializer是ordinary、safe、non-suspend上下文，可分配、GC和抛异常。启动期异常或循环使image initialization失败，打印普通未捕获异常并终止，`main`不执行；已经发生的外部副作用不回滚。

### 6.3 `const val`

`const val`只允许top-level、object或companion位置，必须显式type和initializer，不能是`var`、extension、delegate、accessor或local。type限于`Boolean`、基础数值、`Char`与`String`；initializer仅含literal、其他可见const及规范允许的内建纯一元/二元运算。

HIR在定义处解析并求值const依赖图，环是编译错误。函数/accessor调用、普通property/object读取、构造、分配、`throw`和挂起均非法。成功引用在HIR直接成为typed constant value，不调用getter、不触发object/companion initialization，也没有可供`addressOf`的storage。String常量继续使用已登记的只读immortal object表示；其限制遵守M15 root契约。

visibility在folding前检查。public const值及其type进入Export HIR/.slib接口metadata；下游直接消费并在值或绑定目标变化时失效缓存，不携带声明Cone的hidden initializer。

### 6.4 singleton gate、失败与循环

object/companion在第一次非const访问时lazy ensure。每个singleton与runtime top-level unit使用同一状态机：

```text
Uninitialized
Initializing(ownerThread, dependencyStack)
Initialized
Failed(rooted Throwable)
```

winner线程把状态变为Initializing后执行initializer；singleton先普通分配并只把ref保存在winner的managed root中，完整初始化成功后才以release语义发布到global root slot并转为Initialized，其他线程绝不能观察半初始化ref。失败时不发布对象，保存普通managed Throwable root并唤醒等待者；本次与以后访问都抛出该失败，副作用不重试。

同线程沿dependency stack重新进入Initializing unit立即抛`IllegalStateException`并把稳定unit path写入`message`。跨线程等待通过runtime init coordinator登记wait edge；形成wait-for cycle时最后一条边的访问同样抛出带完整cycle message的`IllegalStateException`；若该异常未被initializer内的普通`try`捕获，就沿既有异常边使当前unit进入Failed并唤醒其他线程。M21把core声明扩展为`IllegalStateException(message: String? = Some("illegal state"))`，已有零实参调用保持有效；compiler-generated exception contract按“存在可用的零实参调用”验证，不能继续要求源码参数列表长度为零。无环等待使用可参与safepoint/epoch handshake的park机制，不能持有裸managed pointer睡眠或busy-spin阻止STW。

这套cell只保证一个完整initializer的exactly-once发布，不代表property值可以“未初始化”。普通stored property在Initialized对象/单元中始终已有其声明type的合法值；Option shorthand的合法值就是`None`。

## 7. Runtime、GC与ABI契约

- compiler-managed top-level storage、delegate slot、singleton published ref和Failed exception slot都在managed执行前以非可选`RefScan`登记为可更新global root；moving collection直接改写slot；
- initialization cell的tag、thread token与wait metadata是GC-free runtime side metadata。它不把managed ref塞入opaque integer；singleton/failure引用位于单独登记的typed root slot；
- singleton allocation与M19 class allocation相同：完整payload先清零、header/size/object-start登记完成后返回，初始化期间receiver是普通精确stack root；发布不pin对象，后续移动只更新global root；
- delegate hidden field/global使用其concrete type的递归scan，property type本身不决定delegate storage布局；optional `None`继续服从M13固定ref-slot清零/niche规则；
- runtime提供typed init coordinator primitive或等价内部ABI：enter/park、publish-success、publish-failure与cycle path管理。enter结果是封闭sum：`RunInitializer`、`Ready`、`Failed(rooted Throwable)`或`Cycle(GC-free stable path)`；generated ensure负责从后两者重新抛出failure或构造带message的`IllegalStateException`，runtime不按名称寻找core constructor。生成代码仍执行全部Scoop initializer body；runtime不按property/object名称反射调用，不补字段值，也不运行accessor；
- 每个image导出init-unit descriptor table与count，record至少关联上述不依赖枚举/绝对路径的stable unit key、GC-free cell、成功storage/root描述及generated initializer entry。M21单image先锁定ABI；M23负责加入Cone identity、多image registration、依赖顺序和去重，不改变unit状态语义；
- startup必须在GC/root/stackmap/image metadata可用后才运行managed initializer。shutdown不重跑、回滚或析构singleton/global，也不把init failure交给GC finalizer。

## 8. IR与stage边界

### 8.1 AST / parser

AST保留property form而不预判语义：显式type、initializer/`by`/accessor list、property/accessor visibility与annotation、modality/override、extension receiver/type parameter、setter parameter、`const`及完整span。`lateinit`在modifier位置产生专门诊断；成功AST不需要`LateInit`字段。

property body使用封闭sum，不能用多个`Option`组成非法组合：

```text
PropertyBodySyntax =
    Initializer(Expr, AccessorSyntax)
  | OptionalOmitted
  | Computed(AccessorSyntax)
  | Delegated(Expr)
  | Abstract
  | ExternStorage
  | Const(Expr)
```

parser只根据token形成候选form并恢复；HIR验证`OptionalOmitted`的outer Option/mutability、computed accessor完备性和target legality。class/object body item继续保存源码顺序；nested/object/companion拥有显式owner节点。primary/secondary constructor增加独立visibility/annotation，primary property parameter增加member modifier，不能把它们塞进普通parameter syntax。

### 8.2 Export HIR / LocalConcrete HIR

Export HIR为每个逻辑property保存：owner、name、exact/template type、read-only/read-write capability、`DeclaredVisibility`、`EffectiveLookupDomain`、可能的`SlotContractDomain`、representation接口、getter和可选setter typed ref、override/interface slot relation、definition origin。三种access identity与getter/setter/property id互不兼容；read-write使用封闭sum而不是`is_var + Option<setter>`产生矛盾。

implementation representation同样是sum：stored携带backing storage与initializer，accessor-only携带getter及可能setter的`AccessorImplementation::{Body, AbstractSlot}`，delegated携带effective delegate plan/storage，const携带typed value，extern/raw携带ABI storage。explicit abstract property正规化为全部required slot都是`AbstractSlot`，interface允许Body/AbstractSlot混合；concrete owner不能残留abstract slot。只有stored可构造`BackingFieldId`，只有delegated可构造`DelegateStorageId`，只有runtime初始化variant可构造`InitializationUnitId`。

visibility使用上述分离的typed domain与已经验证的kind-specific access witness。候选ref、override relation、signature exposure和M17 default reference分别携带适合自身用途的witness，不能把“曾经查到过名称”或一种domain proof当作另一种证明。Export只包含源码显式public且effective/contract允许导出的语义表面、generic template及M23所需依赖；internal/private实体不因link symbol存在而冒充export declaration。

object declaration、object type、singleton value、companion relation、published root与initialization unit使用不同id。generic host companion没有host argument Vec；结构上就是owner template关联的非generic singleton。nested type保存typed owner，不把owner拼进字符串后再解析。

HIR完成property/extension/delegate/operator/default conflict/visibility决议与generic concretization。winner commit后的LocalConcrete HIR只含concrete accessor target、field/global storage、delegate call和typed object/global ensure；ensure同时携带unit与已解析的cycle-exception constructor，不让后续按core名称查找。不含property候选、unresolved accessor、type parameter、interface conflict、late-init状态或visibility字符串。

### 8.3 MIR / LIR / codegen

- default/custom accessor成为普通concrete callable，但使用property getter/setter origin与隐含receiver signature；property read/write在HIR已正规化为field/global operation或唯一call target；
- interface function/property default body进入普通typed dispatch target。每个itable slot已经确定class implementation、interface default或adjust thunk；MIR不重新跑most-specific/conflict算法；
- delegated access在HIR展开为“读取一次delegate storage + 普通typed operator call”，source `by`与role name不进入MIR；
- object/companion access成为带`InitializationUnitId`、cycle-exception target和typed published-root ref的ensure/load operation。MIR把它展开为`RunInitializer`/`Ready`/`Failed(rooted Throwable)`/`Cycle(GC-free stable path)`结果及完整CFG；top-level init table保存generated callable和root plan，codegen不根据symbol前缀猜startup function；
- backing/delegate/class field沿用M19/M15 field layout、write barrier和relocation。ordinary managed global、singleton/failure slot进入LIR image root descriptor；raw/TLS/extern storage继续走M12独立representation；
- const在HIR消失为普通typed literal/immortal String ref；LIR没有`ConstPropertyRead`或runtime getter；
- visibility、annotations的source form、accessor shorthand、companion forwarding与nested qualifier均不进入MIR。需要linkage的信息由typed local/external target直接给出；
- initialization gate call可能park、GC和throw，必须使用普通managed call/invoke及完备statepoint/exception root plan。成功后只从已登记published slot reload singleton ref，不能保留gate前裸地址。

### 8.4 `.slib`与M23边界

M21先把可见性、property/accessor、default body、object/companion、const与init dependency所需的Export/MIR/LIR meta结构锁定；M23负责真实打包/reader与多Cone调度。公开generic accessor/default body的template dependency closure遵守既有规则；public default parameter仍只能直接引用其调用域可见实体。

下游实现/override public interface property需要完整slot、accessor contract和default-source identity；下游访问object/companion需要singleton external root/init entry；下游读取const需要值。metadata不得要求扫描上游method/property列表、FQN或symbol重建这些关系。private top-level和internal实体不进入下游source候选；上游object code仍可为自身实现依赖保留local symbol。

## 9. 诊断与恢复

至少提供以下稳定诊断：

- `lateinit`，并建议`var p: T? = None`或省略initializer的Option shorthand；
- 非Option/`val` stored property省略initializer，Option shorthand与custom accessor混用，initializer/`by`/abstract/computed form冲突；
- getter/setter元数、返回/参数type、缺失computed accessor、val setter、computed body使用`field`；
- property重名、未写/多写`override`、final/private目标、type或mutability不兼容、value type实现var；
- extension property有initializer、非法位置、type parameter不能仅由receiver确定、generic delegated extension；
- delegate role缺失/歧义、错误operator/modifier/arity/result、suspend/generic/default/vararg role，以及不同get/set effective delegate；
- object/companion声明type parameter/constructor、一个owner多个companion、singleton普通构造、companion访问host type parameter/instance；
- nested declaration隐式使用outer receiver/type parameter、`inner`/anonymous/local object当前不支持；
- interface stored/delegated property或`init`、private interface member无body、default冲突、qualified super qualifier非direct/目标abstract；
- visibility非法位置、重复modifier、setter更宽、override收窄、protected receiver不合法、public/internal签名泄漏narrower type/entity；
- constructor visibility/annotation语法错误、unsafe delegation contract、core annotation target/coexistence错误；
- const非法位置/type/expression/dependency环/visibility；ordinary property与raw/extern annotation混用；
- top-level/object直接初始化依赖环显示有序typed unit path；runtime间接循环显示稳定unit名称与依赖栈。

parser以当前accessor/property/body item、object/nested closing brace为恢复边界。非法property form不能降级成local binding或function；存在diagnostic时不输出残缺Export/LocalConcrete module。runtime只处理依赖普通执行才能发现的re-entry/cross-thread cycle，不承担静态property/type错误。

## 10. 测试计划

### 10.1 property与accessor

- class/object/top-level stored val/var、implicit/custom getter/setter、private setter、backing field与一次求值；
- Option shorthand对ref/value/nested Option/generic payload写入真实None，之后Some赋值及`when`/`?:`/`!!`；反向检查IR/runtime没有late-init bit或专用异常；
- computed class/struct/enum/interface/extension property，read/write place、safe read、property-like invoke、`++`与复合赋值；
- open/abstract/override/final property、var→val允许方向、accessor级interface obligation/default与value type restriction；
- generic owner property与generic computed extension在多个exact application单态化，receiver不能推导的type parameter报错。

### 10.2 delegate

- member/object/top-level/non-generic extension/local delegated val/var；member/extension role、可选provide、owner receiver与Unit receiver；
- delegate expression/provide/get/set副作用顺序各一次，异常路径、closure capture与moving GC后hidden storage更新；
- class delegate在common sequence中与stored property/`init`交错，不能经provide发布InitializingThis或提前访问computed/delegated member；
- reflection-free golden确认没有name/KProperty参数或metadata allocation；generic delegated extension稳定拒绝。

### 10.3 object、default与visibility

- top-level/nested/named/default companion，type/value identity、forwarding、generic host不复制companion、const访问不初始化；
- object base construction、field/init/delegate顺序、失败不发布、重复访问同一identity；
- class hierarchy优先、most-specific interface default、unrelated diamond conflict、abstract suppression及`super<I>` function/getter/setter direct call；class/ref与boxed value itable；
- default-internal覆盖top-level/member/constructor/interface/override与main入口；public/internal/file-private/member-private/protected访问矩阵、owner/inheritance domain、public owner hidden-abstract诊断、internal/private class显式public override、setter narrowing、override widening、signature exposure与M17 default access witness；
- static nested generic type使用自己的参数且不捕获outer；inner稳定拒绝。

### 10.4 initialization、runtime与stage golden

- independent top-level initializer按stable declaration key，含不同文件同名private property；直接依赖提前ensure但各一次，startup先于main；
- direct dependency cycle HIR diagnostic；ordinary call造成的same-thread runtime cycle、两个thread wait cycle、其他thread无环等待；
- object initializer成功publication的release/acquire可见性，失败异常memoization，等待线程收到同一failure；
- initializer/delegate/accessor中分配、异常、强制moving GC；published singleton/global/delegate/failure root全部relocate；未发布半初始化object不可从其他线程观察；
- AST / ExportHir / LocalConcreteHir / MIR / LIR golden锁定全部typed id、representation sum、access domain/default slot/init unit/root descriptor，并证明LocalConcrete以后不存在候选、lateinit、未决visibility或delegate role；
- 与constructor/default/vararg/operator/closure/suspend caller/FFI global/exception/generic class组合；M1至M20及M25已有fixture全量回归。

## 11. 实现顺序与提交门

1. 先同步language/runtime/impl spec与ROADMAP，固定lateinit排除、Option shorthand、property representation、visibility与init状态机；
2. 扩展lexer/parser/AST的visibility、完整property/accessor/object/companion/nested/constructor表面和恢复，成功AST不含lateinit；
3. 把visibility omission正规化为typed `Internal`，再建立AccessDomain、signature exposure、override visibility和M17 witness统一检查；同步把sysroot计划公开的core API逐项标成`public`，不设置core专用default-public旁路；
4. 重构现有field/global为逻辑property + typed accessor/storage sum，完成stored/computed/Option shorthand及M18 place/property-like接入；
5. 完成interface property/default body、slot级冲突选择和qualified `super<I>`；
6. 完成extension/generic extension property与reflection-free delegate role/hidden storage；
7. 完成object/companion/static nested owner模型与M19初始化复用；
8. 把M12临时global拆成ordinary property、const、raw/TLS/extern，生成global root与top-level init unit；
9. 扩展core `IllegalStateException`的default-message constructor，并实现runtime init coordinator、startup table、failure/cycle/concurrency与moving root验证；
10. 更新Export/LocalConcrete/MIR/LIR/codegen typed结构、M23系列metadata边界、negative/golden/组合fixture和全量回归。

每批代码变更先执行`cargo fmt --all`与`cargo clippy --workspace`，再运行对应crate test、stage golden和fixture。不得保留“field就是property”的旧旁路、按accessor body猜backing field、Kotlin/JVM式lateinit检查、反射delegate参数、按implements顺序选default、visibility只在parser保存字符串、object无同步裸global或MIR按名字重建accessor/slot/init target。

M21只有在以下条件同时满足时完成：所有property form进入同一logical property/accessor模型；Option shorthand始终生成真实None且语言/runtime不存在lateinit状态；stored/accessor-only/delegated/const/extern-raw互斥完备，且每个accessor的Body/AbstractSlot状态结构完备；interface default和property obligation对每个exact application得到唯一typed target；object/companion只发布完整初始化的单例并在线程、异常、循环和moving GC下正确；top-level initializer在main前exactly once；visibility/access domain贯穿候选、override、default与export；LocalConcrete HIR以后不含source property候选或未决状态，MIR/LIR只机械消费唯一typed target、storage、dispatch和init plan。

## 12. 明确不做

1. `lateinit`、平台null、隐式非Option零值、隐藏initialized bit、读取时late-init专用异常或reflection flag；
2. property reference / `KProperty` / delegated property reflection metadata与JVM accessor命名ABI；
3. custom getter/setter挂起、async initializer、suspend object construction；需要异步值使用`suspend fun`；
4. `inner class`、匿名object expression、local type/object及implicit outer capture；M21只有static nested declaration；
5. generic delegated extension property的跨Cone specialization storage；等待M23明确实例归属，不使用erasure fallback；
6. interface method自身type parameter或generic virtual/default slot；M20/M14限制不变；
7. class/interface delegation `class C : I by impl`；property delegate在M21完成，class delegation仍单独待排期；
8. `sealed`闭包、完整smart-cast、context parameter、typealias与annotation processor/plugin；
9. property/accessor ABI稳定承诺、反射枚举或动态symbol lookup；M23只承诺typed `.slib` metadata与内部mangling一致；
10. singleton析构、finalizer、失败rollback或initializer重试；资源仍显式`release`/`close`。
