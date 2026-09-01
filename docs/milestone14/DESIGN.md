# M14 设计：泛型类型、上界约束与接口化

版本：0.1（草案）

对应 `docs/ROADMAP.md` 的 M14。目标：统一generic nominal type的template/application/concrete模型，补齐 invariant generic class并整改既有generic struct/enum/interface表示；把已有generic member function收紧并补全为明确的non-virtual generic method模型；落地 interface 类型上界与 `where` 子句，使 function、class、struct、enum 与 interface 的 generic template 可以只依赖上界做成员解析；把字符串化、哈希和值相等从历史 `Any` 固定方法槽迁移到 `ToString` / `Hash` / `operator fun equals`；同时完成一次 IR 输出完备性整改，清除目前由下游按上下文、字符串或并行字段猜测必要信息的路径。

M14 完成后，class/struct/enum/interface都使用同一原则下、按declaration kind隔离的generic template/application/concrete模型；普通用户声明的`class C<T>`与core的`Array<T>` / `MutableArray<T>`使用同一套generic class解析、约束、单态化和跨Cone export模型，数组只在representation与专用操作上是intrinsic，不在类型身份或泛型规则上另建特例。`Any`仍是所有类型的根，但没有成员；vtable不再保留固定前三槽，`TypeDescriptor`也不承担通用equals/hash/toString分发。M15 moving compaction因而不再背负按对象地址实现的默认字符串化或哈希语义。

## 0. 范围与关键决策

- M14统一generic nominal type模型：把既有generic struct/enum/interface与新增的generic class全部改为“template declaration → 完整type application → local concrete specialization”三类typed identity。`class C<T>`补齐constructor推导、generic base/interface、宿主成员解析与完整单态化；class/struct/enum的type parameter在M14全部为invariant，interface继续使用既有声明点variance规则，非interface声明点`in`/`out`留待后续；
- M14把generic method作为正式功能闭环：class/struct/enum member可以声明自己的type parameter与bound，但必须non-virtual；class generic method语义上必须为final，任何generic method都不能是open/abstract/override、不能实现interface slot，也不进入vtable/itable。宿主参数与方法参数使用不同typed identity并共同形成concrete callable；
- M14 落地 `T : Interface` 和 `where T : Interface`。同一类型参数可以有多个 interface 上界，并统一适用于function、class、struct、enum与interface；M14 不引入 class 上界、交叉类型值或使用点类型投影；
- `value` / `ref` kind bound 与 interface 上界继续互斥。同一类型参数一旦有任一 interface 上界，就不能再有 `value` / `ref` 约束；
- generic template 中对有界类型参数的成员调用在 HIR 解析为类型化的 bound member，不生成运行期 dictionary/witness 参数。实例化时由 HIR 将它解析为 concrete direct / virtual / interface call；`LocalConcreteHir` 不允许保留 type parameter 或未决 bound call；
- `ToString` 和 `Hash` 是普通 nominal interface，所有类型都必须显式adopt并提供实现，编译器不为任何值类型自动生成这两个interface的conformance或成员。“参数类型等于当前完整concrete value type”的equals仍可条件派生。Scoop没有内建`Self`类型，interface也没有隐含的实现者类型参数；
- `==` 只考虑左操作数的成员 `operator fun equals`；值类型另有编译器生成的同类成员候选。`Any` 没有 equals，缺少可用成员时直接编译错误；`===` 仍是不可重载的引用 identity 比较；
- `print` / `println` 变为普通 generic core 函数 `fun <T : ToString> ...`。String、当前已落地的基本类型及其他显式adopt `ToString`的类型均通过同一 bound 路径调用，不在 HIR/MIR 按类型名分支；
- `@Intrinsic`扩展到core type声明，使Int/UInt/Boolean/String以及generic `Array<T>` / `MutableArray<T>`等compiler-represented类型在源码中显式给出nominal声明与成员/interface表面；固定表示与依赖类型实参的表示族都从HIR起使用独立typed representation，绝不伪装成零字段struct/class；
- `Array<T>` / `MutableArray<T>`复用普通generic class的类型身份、bound、成员解析与单态化；现有独立compiler-built-in array type identity在M14退役，不能与core class application双轨并存；
- 删除 `Any` 固定三槽、地址哈希与地址字符串化是 M14 主目标的一部分，不在附加整改中重复列项；
- 本里程碑附带修复 M13 后审计出的其余 IR 不完备路径。每项都以“上游结构化地产生完整信息、下游只消费”为验收标准，禁止保留 fallback、反向扫描或按 symbol/name 特判。

### 0.1 generic能力盘点

本表是M14与ROADMAP backlog之间的覆盖索引。表中“既有”表示M14不得破坏且须纳入新typed identity/concretizer回归，不表示可以继续保留旧IR捷径。

| 能力 | M14处理 |
| --- | --- |
| top-level generic function、显式/推导type argument、重载 | 既有能力；M14加入interface bound/`where`并迁入新Export/LocalConcrete边界 |
| local/extension generic function与concrete callable reference | 既有能力；M14加入bound并回归closure/suspend组合，默认参数仍属M7通用callable backlog |
| class/struct/enum generic method | M14补齐为2.8的non-virtual完整模型 |
| interface method-level type parameter | 当前定义处拒绝；动态分派与跨Cone ABI进入backlog |
| generic class/struct/enum/interface及generic继承/实现 | M14统一template/application/concrete identity并完成layout/dispatch |
| interface declaration-site variance | 既有能力；M14迁移到完整application identity并保留variance bridge |
| class/struct/enum declaration-site variance | backlog；M14 invariant |
| interface upper bound、多个bound、F-bound、`where` | M14 |
| class upper bound与表达式交叉类型 | backlog |
| use-site `in`/`out` projection | backlog，独立于声明点variance |
| star projection与capture conversion | backlog；不得用擦除或`Any`代替 |
| exact generic RTTI及`is`/`as`/`as?` | M14只支持完整application |
| generic callable recursion与实例化闭包终止 | M14接受identity recursive SCC；一般polymorphic recursion进入backlog |
| generic `typealias` | backlog；需透明展开、cycle与export设计 |
| generic extension property | backlog，依赖property基础能力 |
| nested/inner generic type与generic class companion作用域 | backlog，依赖nested/object基础能力 |
| 显式实参`_`占位/部分推断 | backlog；M14仍为整组省略或整组给出 |
| fresh-variable、postponed argument及projection参与的完整constraint solver | backlog；M14只扩展当前固定点求解器 |
| runtime dictionary/shared generic body | backlog中的可选未来优化，不得作为correctness fallback |
| polymorphic function value、generic lambda、HKT、associated type、const generic、用户specialization | 当前语言明确不引入 |

## 1. 端到端语言形态

```scoop
interface ToString {
    fun toString(): String
}

interface Hash {
    fun hash(): Int
}

fun <T : ToString> render(value: T): String = value.toString()

fun <T> renderPair(first: T, second: T): String
    where T : ToString =
    first.toString() + ", " + second.toString()

struct Point(val x: Int, val y: Int) : ToString {
    override fun toString(): String =
        "Point(x=" + x.toString() + ", y=" + y.toString() + ")"
}

class User(val id: Int) : ToString, Hash {
    override fun toString(): String = "User"
    override fun hash(): Int = id
    operator fun equals(other: User): Boolean = id == other.id
}

class RenderBox<T : ToString>(val value: T) : ToString {
    override fun toString(): String = value.toString()
}

struct RenderPair<T : ToString>(val first: T, val second: T) : ToString {
    override fun toString(): String =
        first.toString() + ", " + second.toString()
}

enum RenderState<T : ToString> : ToString {
    Value(T),
    Empty

    override fun toString(): String = when (this) {
        Value(value) -> value.toString()
        Empty -> "Empty"
    }
}

fun main() {
    println(Point(20, 22))
    val a = User(42)
    val b = User(42)
    println(a == b)
    println(a.hash())
    println(RenderBox(Point(20, 22)))
    println(RenderPair(Point(1, 2), Point(3, 4)))
    println(RenderState.Value(Point(5, 6)))
}
```

这个例子固定以下边界：

- `render` 的 template body 只从 `ToString` 上界取得 `toString`，不依赖某个实际类型或 `Any`；
- `RenderBox<T>`是普通generic class；其字段、constructor、interface实现和方法体在`RenderBox<Point>`实例化时统一替换，不能为class保留另一套非泛型路径；
- `RenderPair<T>`与`RenderState<T>`分别是generic struct/enum；上界参与template body检查、显式`ToString`实现与构造推导，fully specialized value type再取得完整layout与GC-free flag；
- `Point` 显式adopt并实现 `ToString`；它的两个字段可比较，因此仍可自动获得条件派生的 `equals(Point)`。这两种能力来源彼此独立；
- `User` 的三个行为都是普通成员/interface 实现，没有 TypeDescriptor 特权槽；
- `println(a == b)` 先按 `User` 的成员候选解析 equals，再把 Boolean 通过 `ToString` 打印；
- 把 `a` 显式上转为 `Any` 后不能调用 `toString`、`hash` 或 `==`；若需要这些能力，静态类型必须保留相应 interface。

## 2. interface 上界与 `where`

### 2.1 AST 与语法

类型参数与约束使用结构化 AST：

```text
TypeParamDecl {
    name: Ident,
    variance: Variance,
    inline_bound: Option<TypeBound>,
    span: Span,
}

TypeBound = Kind(TypeParamKindBound) | Upper(TypeRef)

WhereClause {
    constraints: Vec<TypeConstraint>,
    span: Span,
}

TypeConstraint {
    parameter: Ident,
    bound: TypeBound,
    span: Span,
}
```

`where` 位于声明头之后、函数 body或类型 body之前：

```scoop
fun <T> stringify(value: T): String where T : ToString = value.toString()

struct Pair<T>(val first: T, val second: T)
    where T : ToString

enum MaybePrintable<T> where T : ToString {
    Value(T),
    Empty
}

interface Encoder<T> where T : ToString {
    fun encode(value: T): String
}

class PrintableBox<T : ToString>(val value: T) : ToString {
    override fun toString(): String = value.toString()
}
```

直接 bound 与 `where` 进入同一 ordered constraint list；parser 不做名称解析或“interface”判断，只保留每个参数名、bound 与精确 span。`where` 中未知参数、重复约束、非法 bound kind及上界不是 interface都在 HIR 诊断。M14将同一套类型参数与约束语法开放给function、class、struct、enum与interface；class不允许声明点`in`/`out`，该限制由HIR在variance modifier的准确span诊断，而不是让parser丢失信息。

### 2.2 声明合法性

HIR 对每个 generic 声明执行以下检查：

1. inline bound 与所有 `where` constraint按源码顺序归属到唯一的 type parameter；
2. `Upper(TypeRef)` 必须解析为完整的 interface application，保留 interface typed id与全部类型实参；裸 class、struct、enum、函数类型、`Any`、另一 type parameter以及未完整应用的generic interface都不是 M14 合法上界；
3. 同一参数可以有多个不同的 interface 上界，但不能重复同一个正规化后的 interface application；
4. `value` / `ref` 最多出现一次，且与任意 interface 上界互斥；
5. 上界中的类型实参可以引用当前声明作用域内的 type parameter，因此 `T : Comparable<T>` 合法；所有引用仍须满足目标 interface 自身的arity、variance与bound；
6. interface 继承闭包中重复到达的同一上界按typed identity去重。循环interface继承仍由既有继承环检查诊断，不能靠遍历深度上限截断；
7. generic class/struct/enum的类型参数在M14必须是invariant；`in`/`out`是已识别但当前不合法的声明，并给出指向后续non-interface variance能力的正式诊断；
8. 无约束参数的语义仍是接受任意类型，但这不等于隐式拥有一个可调用成员的 `Any` 上界，因为 `Any` 没有成员。

M14 不把多个上界物化为一个可写入变量的交叉类型。它们只构成 type parameter 的能力集合；普通表达式的类型仍必须是现有的单一 `TypeId`。

### 2.3 类型实参满足关系与推导

类型实参 `A` 满足 `T : I<X...>`，当且仅当把当前 substitution 应用于整个上界后，`A` 是该 concrete interface application 的子类型：

- class/interface按普通继承、实现与声明点型变规则判断；
- struct/enum与class一样，只按源码显式声明的interface conformance判断；条件派生的equals是成员，不产生interface conformance；
- tuple/Unit不能在源码中声明implements列表，因此M14不满足任何普通interface上界，也不能因元素成员同形而被视为实现某个interface；
- 装箱不是“满足上界”的额外规则。值类型本身实现 interface 后可以作为 `T`，只有在实际需要interface ref表示时才按既有 O(1) 规则装箱；
- `Any` 不因保存过某个实现者就满足该interface。静态类型已经丢失的能力不能由运行期猜回。

泛型推导继续对整组实参做固定点求解。上界是求解约束的一部分，而不是推导结束后的可选警告：

- 推导先收集等式、子类型、期望类型和声明bound约束，再统一求解；
- 不得在多个候选中任意选择一个“碰巧满足bound”的类型，也不得用 `Any` 填补未绑定参数；
- 显式类型实参与推导结果走同一套bound验证；
- generic class/struct/enum构造与generic function调用使用同一个固定点求解器。构造调用可以从显式类型实参、constructor/variant实参和期望类型推导宿主参数；类型标注、字段类型、基类与interface列表中的generic nominal application必须写出完整实参，不能出现裸generic type或部分application；
- 诊断必须同时指出不满足的具体实参、声明处bound及失败的interface application；不能检查字段形状后替类型补出未声明的conformance。

### 2.4 bounded member resolution

receiver 静态类型为 type parameter 时，候选只来自其interface上界及这些interface的继承闭包：

- 不加入 `Any` 成员，不加入实际类型“将来可能有”的成员，也不加入扩展函数作为成员fallback；
- 每个候选携带声明它的 `ExportInterfaceMethodId`、完成上界type argument替换后的签名及来源bound；
- 同一**完整interface application**经多条继承路径到达时按application typed id合并；同一template的`I<Int>`与`I<String>`绝不能因declaration id相同而合并。override链只保留最派生声明；来自互不相关interface或不同application的声明即使文本签名相同也保持不同候选，不能按名字合并。若普通overload resolution无法选出唯一目标，则报歧义并列出bound来源；
- interface方法只能使用interface宿主的类型参数，M14不允许方法自身再声明type parameter。因而每个合法interface member都具有可进入itable的完整签名，经interface静态类型与bounded receiver调用时走同一套候选/分派规则，不存在调用时再做“object safety”过滤；
- suspend、visibility、unsafe及普通overload applicability规则与直接interface receiver一致。

`ExportHir` template 中的调用目标使用专门的typed bound member引用，而不是function name或暂定slot：

```text
BoundCallableRef {
    receiver_parameter: ExportTypeParamId,
    bound: ExportInterfaceApplicationId,
    member: ExportInterfaceMethodId,
    instantiated_signature: ExportFunctionTypeId,
}
```

当本Cone或下游Cone实例化template时，HIR用concrete substitution验证bound，并把每个 `BoundCallableRef` 解析为普通 `ConcreteCallableRef`：value/final成员为direct，open class成员为virtual，interface静态类型为interface dispatch。这里的“单态化静态分发”指不传递runtime generic dictionary；它不取消actual concrete type本来具有的virtual/interface动态分派语义。

`LocalConcreteHir` 中禁止出现 `BoundCallableRef`、`TypeParam`、待验证constraint或“稍后选实现”的字段。MIR只消费已经完成的concrete call target。

### 2.5 generic nominal type

generic class不是数组专用能力；generic struct/enum/interface也不能因为早于M14已经可用，就继续保留一套较弱的IR表示。M14必须对class、struct、enum与interface统一打通声明、完整application、成员/构造、layout/dispatch identity与跨Cone实例化的闭环：

- `class C<T...>`、`struct S<T...>`、`enum E<T...>`分别形成export侧generic template。constructor property、struct field、enum variant payload、implemented interface、成员签名和成员body都可以引用宿主type parameter；
- `interface I<T...>`同样形成export侧generic template；父interface application与方法签名/body可以引用宿主type parameter，并继续执行既有声明点variance位置检查；
- class额外允许generic base application及base constructor argument：`class D<T>(value: T) : B<T>(value), I<T>`在template检查期保留完整的base/interface application。每个fully specialized `D<A>`再解析到唯一的`B<A>`与`I<A>` concrete identity；继承环和override在应用substitution后仍必须成立；
- 继承/implements闭包只对完全相同的interface application去重。同一实现者可以在类型上到达同一template的不同application；它们保留不同TypeDescriptor/itable key和不同成员obligation，声明点variance关系按既有bridge规则单独 materialize。若多个obligation需要无法由Scoop overload/override规则同时表达的实现（例如只按返回类型区分），在实现检查处诊断，不能任选一个application或擦除实参合并slot；
- class/struct/enum application在M14全部invariant。同一template的两个application只有所有实参相等时才是同一类型；struct/enum不同application之间没有subtyping，class仍可通过普通继承使`D<A>`成为其已解析base application的子类型；
- class/struct constructor与enum variant构造均支持完整显式实参或整组推导。显式列表必须覆盖宿主全部参数；没有足够约束唯一求出所有参数时在HIR报错，不以`Any`或bound默认填补；
- constructor与enum variant constructor不声明自己独立的type parameter；构造调用中的类型实参始终对应nominal host。未来的secondary constructor也只复用class参数，不形成generic method式的第二组参数；
- generic宿主上的普通成员先用receiver静态类型确定宿主实参，再检查成员调用；class/struct/enum成员自身也generic时，单态化身份按spec 3.2固定为“宿主实参前缀、方法实参后缀”。两组参数使用不同typed id，不能按名称或裸索引拼接；interface成员自身的type parameter按2.7在定义处拒绝；
- fully specialized class是reference type，`gc_free`恒为`false`，但对象field/base layout、payload scan、vtable、itable与方法ABI仍按concrete实参产生；
- fully specialized struct的`gc_free`由全部concrete field递归计算；fully specialized enum的每个variant分别产生非可选`gc_free: bool`，enum自身等于全部variant flag的AND。generic template只能携带typed GC-free predicate，不能携带未知/可选flag；
- fully specialized interface application是ref type，`gc_free`恒为`false`；它没有实例payload layout，但必须拥有独立concrete TypeDescriptor、父interface闭包、method signature与itable key identity，不能把不同实参擦除为同一个interface；
- `is` / `as` / `as?`对generic nominal type检查完整concrete application identity：`Box<Int>`、`Box<String>`、`I<Int>`与`I<String>`都是不同目标，不擦除type argument。generic body中的`is T` / `as T`在实例化后直接引用替换所得concrete type；M14不存在可表达“任意application”的裸`Box`或`Box<*>`目标；
- recursive generic class引用可以经过ref边界，例如`class Node<T>(val next: Option<Node<T>>)`。struct/enum的递归value layout也必须最终经过ref边界。concretizer先intern concrete identity再展开字段/variant/成员依赖，并以typed instantiation state诊断无限value layout或非法实例化循环，不能使用递归深度上限或FQN缓存止损。

Export与local-concrete实体严格隔离，且每一种nominal declaration都使用自己的id家族：

```text
ExportNominalApplication =
    Class(ExportClassApplicationId)
  | Struct(ExportStructApplicationId)
  | Enum(ExportEnumApplicationId)
  | Interface(ExportInterfaceApplicationId)

ExportClassApplication {
    template: ExportGenericClassId,
    arguments: NonEmptyVec<ExportTypeId>,
}

ExportStructApplication {
    template: ExportGenericStructId,
    arguments: NonEmptyVec<ExportTypeId>,
}

ExportEnumApplication {
    template: ExportGenericEnumId,
    arguments: NonEmptyVec<ExportTypeId>,
}

ExportInterfaceApplication {
    template: ExportGenericInterfaceId,
    arguments: NonEmptyVec<ExportTypeId>,
}

ConcreteTypeKind =
    Class(ConcreteClassId)
  | Struct(ConcreteStructId)
  | Enum(ConcreteEnumId)
  | Interface(ConcreteInterfaceId)
  | ...
```

每一组`ExportGeneric*Id`、`Export*ApplicationId`与`Concrete*Id`都是互不兼容的typed id。export application携带完整实参，local concrete type则只引用已经完成替换的concrete entity；当前`Type::Struct(StructId, Vec<TypeId>)`、`Type::Enum(EnumId, Vec<TypeId>)`、`Type::Interface(InterfaceId, Vec<TypeId>)`以及不带application identity的class表示都必须退出export HIR，不能用“声明id + 参数Vec”同时冒充template、application与concrete实例。

每个concrete entity非可选地携带local-concrete侧的kind-specific typed origin、全部concrete arguments及已替换的内容：class为field/base/interface/member闭包，struct为field/interface/member闭包，enum为variant/interface/member闭包，interface为parent/member/variance闭包。`ConcreteClassOriginId` / `ConcreteStructOriginId` / `ConcreteEnumOriginId` / `ConcreteInterfaceOriginId`与对应的`ExportGeneric*Id`是互不兼容的类型；concretizer内部显式建立映射，不能把export arena id直接塞进`LocalConcreteHir`。class的`ConcreteClassRepresentation`还必须是区分ordinary/intrinsic的sum type，而不是`is_intrinsic: bool + Option<...>`；ordinary分支携带普通class layout输入，intrinsic分支携带该表示族要求的完整concrete参数。

本Cone和下游Cone使用同一个HIR concretizer，以kind-specific `(ExportGeneric*Id, concrete arguments)`作为typed memo key，产生对应concrete id及其依赖闭包。`ExportHir`只导出template/application及类型化依赖，`LocalConcreteHir`只保存本Cone实际需要发射的fully specialized type与函数；MIR不能读取任何generic nominal template或自行替换宿主参数。

### 2.6 `Array<T>` / `MutableArray<T>`迁移

M14在core中以generic intrinsic class声明数组类型：

```scoop
@Intrinsic("core_array")
class Array<T>

@Intrinsic("core_mutable_array")
class MutableArray<T>
```

源码声明提供nominal identity、invariant参数、显式interface列表与可声明的普通成员；数组字面量、内联元素区分配、`size`、下标读写、越界检查和快照转换继续是registry登记的typed intrinsic operation。它们绑定到上述声明与concrete application，不通过名称识别一个旁路的built-in数组类型。

intrinsic type registry必须描述固定类型与generic表示族：

```text
IntrinsicTypeSpec {
    kind: IntrinsicTypeKind,
    declaration_kind: IntrinsicDeclarationKind,
    parameters: IntrinsicTypeParameterSchema,
    representation: IntrinsicTypeRepresentation,
    declaration_contract: IntrinsicTypeDeclarationContract,
}

IntrinsicTypeRepresentation =
    Fixed(FixedIntrinsicRepresentation)
  | GenericFamily(GenericIntrinsicRepresentation)

GenericIntrinsicRepresentation =
    Array { element_parameter: IntrinsicTypeParameterIndex }
  | MutableArray { element_parameter: IntrinsicTypeParameterIndex }
```

`core_array`与`core_mutable_array`的schema都精确要求一个无bound、invariant参数。registry验证arity、variance、bound、target kind、成员/intrinsic operation签名与provider唯一性；测试authority只放宽provider来源，不放宽generic shape。

完成迁移后，export侧的`Array<X>`就是普通`ExportClassApplicationId`，local侧则是`ConcreteClassId`，其`ConcreteClassRepresentation::Intrinsic`完整携带`Array { element: ConcreteTypeId }`或`MutableArray { element: ConcreteTypeId }`。当前HIR/MIR中独立的`Type::Array(T)` / `Type::MutableArray(T)`类型身份必须删除；数组专用expression可以保留，但每个节点必须直接携带来源/目标concrete array class，不能由expected type、operand type或元素expression重建数组种类与元素类型。

LIR为每个fully specialized intrinsic array class application建立唯一`ArrayTypeId`及完备`ArrayType`记录，其中一次性给出nominal kind、element storage type、stride/alignment、递归element scan及目标TypeDescriptor。`ArrayAlloc` / `ArrayLen` / `ArrayGet` / `ArraySet` / `ArrayClone`都显式引用该id；LIR value type只保留managed-pointer物理形态，不构造第二份`LirType::Array(T)`身份。codegen严禁遍历`ArrayAlloc`、按`(element layout, scan)`去重或从operand/result type提取元素信息，只能按`ArrayTypeId`机械消费元数据。`ArrayClone`引用的是目标application id，runtime以其TypeDescriptor分配新对象并只复制对象头之后的size/padding/elements，不能整体memcpy后沿用来源descriptor。

### 2.7 没有`Self`与object safety分类

Scoop中的interface是普通nominal reference type，不是Rust trait、Swift protocol或另一个需要再转成existential/object的声明类别：

- `Self`不是Scoop关键字、预定义类型、associated type或“当前实现者类型”的隐式占位符。源码中出现`Self`时只按普通名称解析；core未定义该名称，规范和IR也不得赋予它宿主替换语义；
- 每个合法且类型实参完整的`interface I<A...>`天然可以作为变量、参数、返回值、字段、cast目标和generic upper bound；没有`dyn I`之类的第二种类型，也不计算`is_object_safe`/`requires_sized_self`之类的flag；
- interface成员签名只能引用普通nominal type、函数类型及interface宿主的type parameter。若契约需要表达“参数与某个实现类型相同”，必须显式参数化，例如`interface EqualTo<T> { operator fun equals(other: T): Boolean }`，实现者写`struct Point : EqualTo<Point>`；HIR不做隐式`Self := Point`替换；
- 当前不允许interface方法声明自己的type parameter，例如`interface I { fun <T> f(value: T) }`在HIR定义检查时报错。generic interface本身完全支持，例如`interface I<T> { fun f(value: T) }`；形成`I<String>`后方法签名完整并可经itable调用；
- 上一条是当前method-level generic dispatch ABI尚未定义的语言子集边界，不是object-safety判定。编译器不能接受声明后再根据receiver形态决定是否可调用。未来若开放interface generic method，必须同时定义跨Cone specialization/itable ABI，并保证经concrete、interface和bounded receiver三种形态都可调用，不能引入“该interface不能作为值类型”的分类。

同样，本文后续所谓“当前完整value type”只是规范描述，不是`Self`类型。对非generic `Point`，派生签名为`operator fun equals(other: Point): Boolean`；对generic `Box<T>`的export template，参数类型是显式application `Box<T>`，实例化`Box<Int>`后参数类型为`Box<Int>`。tuple的派生候选直接引用该concrete tuple type。每个签名在所属IR层都携带真实typed type/application id，不保存`Self`占位符。

### 2.8 non-virtual generic method

M14把此前“能够解析部分generic member调用”的实现补成完整语言规则。这里的method仍有普通instance receiver；non-virtual只表示callee由receiver的静态类型和完整类型实参确定，并不表示把method改成没有`this`的static function。

```scoop
class Box<T>(val value: T) {
    final fun <U : ToString> renderWith(other: U): String = other.toString()
}

struct Pair<T>(val first: T, val second: T) {
    fun <U> mapFirst(mapper: (T) -> U): Pair<U> =
        Pair(mapper(first), mapper(second))
}
```

规则如下：

- 顶层、局部与extension generic function继续使用普通direct单态化；class、struct与enum的member可以额外声明method type parameter。M14的`TypeBound` / `where`规则同样用于这些callable，不能只对top-level function或nominal host生效；
- callable自身声明的type parameter始终invariant，不能写声明点`in`/`out`；声明点variance只属于nominal type parameter，函数参数/返回位置通过普通constraint solving体现方向；
- class generic method必须语义为final。显式`open`、`abstract`、`override`以及隐含open的override形态均在声明处报错；struct/enum method本来即为final。generic method不能占据或覆盖vtable/itable slot，也不能作为某个非generic interface method的“每次调用再选一个实例”的实现；
- 通过base静态类型调用继承到的generic method时，直接调用声明该final method的concrete实例。运行期对象的派生class不能替换该目标，因此不会出现“generic virtual dispatch”；
- receiver先确定全部宿主实参；调用处的显式`<...>`只对应method自身参数，并且仍须全部给出或全部推导。推导器随后以“宿主substitution + method实参 + 全部bound”为一个固定点求解问题；method参数不得与宿主参数重名；
- method默认参数表达式、closure body、suspend transform与`@NoGC`条件若引用两组参数，均在同一个concrete application中完成替换。默认参数/`vararg`本身仍受M7通用callable backlog约束；M14不为generic method另造一个缩水版调用协议；
- generic method的callable reference必须由期望函数类型唯一确定method全部实参。绑定reference按值保存receiver，未绑定member reference仍不在当前语言子集；结果总是一个concrete函数值，不存在携带未解析type parameter的polymorphic function value；
- overload resolution先以宿主substitution后的source signature和fresh method parameter参与候选比较，再产生唯一concrete application。不得先按某个猜测实例生成符号后再用碰撞结果决定overload；两个source declaration即使特化后machine signature相同，仍有不同semantic identity和稳定discriminator。

export与local-concrete identity必须结构化地区分method template、完整application与发射实体：

```text
ExportGenericMethodApplication {
    method: ExportGenericMethodId,
    owner: ExportGenericMethodOwner,
    method_arguments: NonEmptyVec<ExportTypeId>,
}

ExportGenericMethodOwner =
    ParamFree(ExportNominalDeclRef)
  | Applied(ExportNominalApplication)

ConcreteMethodOrigin {
    method: ConcreteGenericMethodOriginId,
    owner: ConcreteMethodOwner,
    method_arguments: NonEmptyVec<ConcreteTypeId>,
}

ConcreteMethodOwner =
    Class(ConcreteClassId)
  | Struct(ConcreteStructId)
  | Enum(ConcreteEnumId)
```

`ExportGenericMethodId`不能与top-level/local generic function id、nominal template id或interface method id混用。`ConcreteGenericMethodOriginId`又是local-concrete侧独立的typed provenance identity，不是export template引用；同理，generic free function使用`ConcreteGenericFunctionOriginId`。两侧的映射只存在于HIR concretizer内部，MIR的输入类型无法构造或读取export id。`ExportNominalDeclRef`与`ExportNominalApplication`在真实IR中还须按class/struct/enum拆成kind-specific variant/id；前者覆盖non-generic owner，后者覆盖generic owner，不能要求所有generic method都恰好位于generic type中。concrete侧直接引用完整`ConcreteMethodOwner`；该owner实体本身非可选地携带origin与全部宿主实参，因此不再平行保存一个“可能为空”的owner argument Vec。非空`method_arguments`同样完整。`LocalConcreteHir`中的body/signature/captures/default-expression请求已经完成替换，MIR只接收普通direct `ConcreteCallableId`。

### 2.9 单态化闭包与终止

按exact application memoize只能去重已经见过的实例，不能阻止`f<T>() -> f<Option<T>>()`产生无限的新实例。M14重建统一concretizer时必须同时建立可终止规则：

- 同一generic callable递归SCC内，任一调用环把宿主与callable参数组成的完整参数向量代回起点后，必须与起点参数逐项相同（允许参数改名和跨多个template的identity传递）；这类普通/互递归在concrete key进入`InProgress`后直接引用已intern的identity；
- 调用环若改变完整参数向量，就是polymorphic recursion，在HIR template检查阶段报错并展示组成该环的call site与参数变换。不能等worklist增长到任意深度后abort，也不能用实例数量上限改变程序是否合法；
- 非递归边可以任意变换实参，例如`outer<T>`单向调用`inner<Option<T>>`；只要不处于返回原template的环中，就只会为每个入口产生有限闭包；
- nominal value-layout recursion继续使用2.5的独立规则；callable SCC、nominal layout SCC和conditional-equality SCC是不同typed状态机，不能共享一个“visited name”集合。

未来若要接受能够证明终止的更一般polymorphic recursion，需要单独定义termination proof及跨Cone实例化契约，已记入ROADMAP；M14只接受上述结构上可判定且不依赖资源上限的子集。

## 3. `ToString` 的显式adoption

### 3.1 intrinsic core type声明

compiler-represented类型不能再依靠名称特判或compiler内部的隐式interface表。M14把既有`@Intrinsic`扩展到type target，使core源码成为基本类型、String与数组等类型的nominal语义表面：

```scoop
@Intrinsic("core_int")
struct Int : ToString, Hash {
    @Intrinsic("int_equals")
    operator fun equals(other: Int): Boolean

    @Intrinsic("int_to_string")
    override fun toString(): String

    override fun hash(): Int = this
}
```

`@Intrinsic("core_int")`表示Int的representation、literal lowering、ABI和内部构造机制由编译器提供；`core_array`/`core_mutable_array`则表示由具体element type参数化的数组representation family（2.6）。implements列表和成员仍按普通语言规则进入名称解析、override检查、generic bound与dispatch。方法可以有普通Scoop body，也可以像示例中的equals/toString一样以另一个已登记function intrinsic提供body。

intrinsic type不是“恰好没有字段的普通类型”：

- AST允许被登记表批准的`@Intrinsic` struct/class省略primary-constructor/field列表；普通零字段struct仍写作`struct Marker()`；
- HIR将声明正规化为`IntrinsicTypeKind::{Int, UInt, Boolean, String, Array, MutableArray, ...}`等封闭typed variant，并按2.6把generic intrinsic application正规化为携带完整concrete实参的representation family；不能构造`Struct { fields: [] }`或`Class { fields: [] }`后让下游看名称修正；
- intrinsic struct不参与普通零字段struct的derived equals、解构、size或默认constructor规则。它的equals/ToString/Hash必须由声明中的成员/interface明确提供；
- 编译器只合成literal、boxing、runtime allocation等不进入源码候选集的hidden construction entry。`Int()`、数值转换或其他用户可调用constructor必须显式声明，不能以“必要constructor”为理由绕过HIR API；
- `ExportHir`保存export侧intrinsic type kind、完整type-parameter schema、显式语义接口及成员，`LocalConcreteHir`保存独立concrete id、固定表示或已完全应用的generic representation family，以及非可选`gc_free`。LIR对该sum type做穷尽layout lowering；没有stage根据FQN、空字段、annotation字符串或操作数猜representation/type argument；
- 同一机制允许compiler-represented value type、reference type及generic representation family：Int/UInt/Boolean是scalar value，String是变长managed class，Array/MutableArray是按element type特化的变长managed class。annotation target的共同点是opaque compiler representation，不是都采用struct布局或固定layout。

生产编译默认只允许已指定的sysroot/core输入提供intrinsic声明。为测试提供内部authority策略：

```text
IntrinsicDeclarationPolicy =
    CoreOnly
  | AllowListedForTesting { providers: Set<IntrinsicProviderId> }
```

- 默认始终为`CoreOnly`；该policy由driver/test harness通过内部`CompileOptions`传入HIR，不是Scoop源码注解、`Cone.toml`字段、环境变量或稳定用户CLI选项；
- `IntrinsicProviderId`是driver为每个编译输入provider分配的typed identity：M14区分sysroot与当前用户输入，M17扩展为每个Cone；它不是路径、包名或可由源码伪造的字符串；
- 测试模式按该identity精确授权provider，不是一个“允许所有用户文件”的全局bool；授权不经import或dependency传递；
- 开关只跳过“provider必须是sysroot”这一项。intrinsic name必须在typed registry中、annotation target与完整声明shape/signature必须匹配、每个intrinsic kind的provider必须唯一、注解共存与body规则全部照常验证；
- HIR output和后续IR不保存“测试模式”布尔值，合法声明在两种authority下产生完全相同的typed实体。由测试provider产生的`.slib`若被使用，消费编译也必须显式授权同一provider；普通生产编译拒绝它，避免test authority泄漏为依赖能力。

这样unit/golden/negative fixture可以构造最小intrinsic provider或单独测试错误分支，同时不能用内部开关掩盖registry、signature或IR完备性问题。

### 3.2 普通 core 声明

core 新增并由HIR按typed id验证：

```scoop
interface ToString {
    fun toString(): String
}

fun <T : ToString> print(value: T) = write(value.toString())

fun <T : ToString> println(value: T) {
    write(value.toString())
    write("\n")
}
```

`write(String)` 继续是 M12 已定稿的 Scoop ABI managed extern。`print` / `println` 是普通generic Scoop函数；其每个实际使用由HIR生成concrete body，MIR看到的只是已决议 `toString` call与 `write` call。

String 的 `toString()` 返回自身。M14 为当前编译器已落地的 Boolean、Int 与 UInt补齐core实现；后续加入的所有 spec 11.2 基本类型在进入可用语言子集时必须同时实现 `ToString`，不能恢复按type name的compiler fallback。数值文本格式沿用各基本类型的既有字面/打印约定；Boolean固定为 `true` / `false`。

基础类型成员本身必须有普通Scoop body。当前Scoop尚不能表达的数值格式化、String内容比较/哈希等representation-level操作，由`types.scoop`中的普通Scoop-ABI `@Extern` helper连接runtime；这些helper走既有extern declaration/call target机制，不登记为function intrinsic，也不进入任何capability-specific core结构。以后若相关算法能用Scoop表达，可以只替换core源码body并删除对应runtime helper，调用者和编译器IR不变。

这些声明不形成 `FormattingCore` 或其他编译器专用通道。`ToString` 是普通 interface，`print` / `println` 是普通 generic function，String/basic implementations 是 intrinsic type 上的普通 interface implementation；名称解析、bound 检查、overload resolution、conformance、单态化与调用 lowering 均复用语言的一般机制。

编译器只为 `@Intrinsic` 所声明的 opaque representation 或 Scoop 本身无法表达的最小底层操作保存封闭 typed kind。给 intrinsic type 新增一个可用 Scoop 实现的 interface、method 或 generic 能力，只修改 `scoop.core`，不增加 HIR well-known 字段、不扩展 MIR 特判，也不要求 runtime 登记。确实需要新底层 primitive 时，新增的是该 primitive 的 typed intrinsic lowering，而不是围绕它所服务的高层 interface 建立 capability bundle。

HIR只按普通声明解析 `ToString`：implements列表产生exact interface application，override检查产生exact interface member → implementation映射，generic bound保存普通interface application。`LocalConcreteHir`接收完全替换后的普通conformance与callable identity；MIR不得按接口名、成员名、返回类型或字段形状补找。若core未声明`ToString`，只有实际引用该名称的core/用户源码按普通unknown type规则报错，不建立编译器well-known错误分支。

### 3.3 adoption规则

- class/object/struct/enum都只能通过显式列出 `: ToString` 并提供合法 `override fun toString(): String` adopt；字段或payload是否实现`ToString`不会自动赋予宿主任何能力；
- generic nominal type可以显式adopt。若实现体需要调用字段的`toString()`，必须在相应type parameter上声明`ToString` bound；这由普通bounded member resolution检查，不是conformance predicate；
- tuple与Unit不能声明成员或implements列表，因此M14不实现`ToString`。需要字符串化时应使用显式adopt `ToString`的命名struct，或由调用者显式格式化各元素；
- 普通同签名 `fun toString(): String` 若没有在implements/override关系中实现 `ToString`，仍只是普通成员，不能满足bound、不能进入`ToString` itable，也不会被 `print` / `println` 接受；
- String和基础类型通过其intrinsic nominal声明显式adopt，行为由core源码中的普通实现决定。编译器不规定其他类型的文本格式、字段求值顺序或拼接策略；
- explicit implementation、this-adjust thunk与itable entry都使用既有普通typed id。不存在generated `ToString` implementation、conditional predicate或专用dispatch路径。

## 4. `Hash`

core 定义：

```scoop
interface Hash {
    fun hash(): Int
}
```

- `Hash` 是普通interface，不是operator convention，也不占据TypeDescriptor固定槽；
- String与已经可用的基本类型提供显式core实现。相等的值必须产生相同hash；算法可以随语言/runtime版本改变，不承诺跨版本数值稳定，但同一执行文件内必须确定且不得使用对象地址；
- Boolean使用0/1，当前word-sized整数按其值的bit pattern混合，String按UTF-8内容计算。测试锁定一致性与非地址依赖，不把某一String混合算法写成语言ABI；
- struct、enum、tuple、class、array、closure及box都没有自动Hash。用户类型必须显式列出 `Hash` 并实现 `hash()`；
- 不提供 `Any.hashCode`、identity hash或“缺实现时返回地址”。未来哈希容器以 `T : Hash` 与相等契约作为静态约束。

## 5. `operator fun equals` 与 `==`

### 5.1 operator声明

AST 为function/method增加独立的 `is_operator: bool`，parser把 `operator` 作为声明modifier处理，不把它保留为注解或名字的一部分。重复modifier、用于不支持的声明、与方法modality/override组合中的语法错误都指向modifier span。

M14 中可参与 `==` 的 equals 必须满足：

- 是名为 `equals` 的成员函数；顶层、局部与extension equals即使标记operator也不参与；
- 显式写 `operator`；恰好一个显式参数，返回 `Boolean`；
- ordinary、非generic、非suspend；
- 可以final/open/abstract/override，参数类型按普通overload与override规则处理。一个类型可以有多个不同参数签名的operator equals；
- interface可以声明operator equals，class/value type按普通 `override operator fun` 实现。operator flag是override contract的一部分，不能在实现处丢失。

其他operator名称的签名约束仍按spec 9.3逐步落地；M14只要求parser/AST使用通用modifier、HIR完整验证equals，不把所有Kotlin operator一并实现。

### 5.2 `==` / `!=` 决议

`lhs == rhs` 保证先求值lhs一次，再求值rhs一次，然后：

1. 从lhs静态类型收集成员operator `equals`；
2. 对value type再加入编译器派生的、参数类型等于lhs完整静态value type的`equals`候选（若存在）；
3. 使用普通成员overload applicability与MSC选择唯一函数；
4. 按该函数的direct/virtual/interface call kind调用。`lhs != rhs` 调用完全相同目标后对Boolean结果取反。

不进行以下fallback：引用地址比较、`Any.equals`、TypeDescriptor比较、extension operator、对左右操作数交换后重试。引用identity只能显式写 `===` / `!==`。

若lhs静态类型为 `Any`，没有候选，因而 `==` 是编译错误。若lhs静态类型为某interface，只有该interface继承闭包声明的operator equals可用；对象的运行期类型另有equals不能弥补静态契约缺失。

### 5.3 value type 条件派生

派生方法的签名固定为“参数类型等于当前完整value type”。例如：

```scoop
// struct Point(...)
operator fun equals(other: Point): Boolean

// generic template struct Box<T>(...)
operator fun equals(other: Box<T>): Boolean
```

这里的`Point`与`Box<T>`都是普通nominal type/application；不存在名为`Self`的特殊类型。`Box<Int>`实例化后的concrete方法参数非可选地为`Box<Int>`。

- struct/tuple逐字段比较，enum先比较tag，再只比较active variant的payload；字段按声明顺序短路；Unit恒为true；
- 字段“可比较”表示对 `field: F` 的两个值执行 `F == F` 能在HIR按普通operator resolution选出唯一成员/派生目标。Boolean/整数等intrinsic type与String都在core源码中声明普通成员operator；它们不进入`EqualityCore`之类的专用表。其他value type递归条件派生，class/interface必须声明可用operator。HIR不能按类型名临时放行基本类型或String；
- generic value type在`ExportHir`保存typed conditional-equality predicate；generic template只能从当前bound提供的operator能力结构化证明它，fully specialized实例再由HIR定稿。失败诊断给出第一个不可比较字段/variant路径；
- 用户声明参数类型为当前完整宿主application的同签名operator equals，或显式实现了参数替换后恰好相同的interface contract时，不再生成派生体。其他参数类型的equals overload不屏蔽该同类型派生候选；
- 派生方法是普通 `ConcreteFunctionId`，被box/interface thunk引用时仍按value-type `this`的按值规则工作。

### 5.4 boxing后的语义

boxing只改变表示与静态类型，不额外赋予相等能力：

- 原value静态类型下的 `==` 使用派生/显式value equals；
- box可以用 `===` 比较identity；两个分别创建的box通常identity不同；
- box以 `Any` 静态类型存在时不能写 `==`，因为 `Any` 没有equals；
- box以声明了operator equals的interface静态类型存在时，按该interface itable调用对应实现；只有签名确实满足该interface时，value实现/adjust thunk才进入itable；
- `as`/模式匹配取回value后再次使用value规则。

因此runtime不再通过TypeDescriptor为所有boxed value强制提供结构相等入口。

## 6. 分派表、TypeDescriptor 与 runtime 迁移

### 6.1 vtable / itable

- vtable从slot 0开始只包含实际需要virtual dispatch的class方法；不再预留 `Any.equals/hashCode/toString` 三槽；
- final class方法直接调用，不因名称为equals/hash/toString进入vtable。open/abstract operator equals按普通class override规则占槽；
- `ToString` / `Hash` 以及声明equals的interface按普通itable布局。value type的显式interface实现需要装箱分派时使用既有this-adjust thunk；
- slot identity、签名和实现目标都由MIR typed dispatch entity给出。slot编号是表内布局结果，不是方法语义identity；
- class继承与跨Cone MIR meta输出从新布局重新编号。M14是尚未稳定发布ABI前的整体切换，不提供旧三槽兼容层。

### 6.2 TypeDescriptor

TypeDescriptor继续保存类型identity、名称、size/alignment、GC扫描、父类及vtable/itable入口，但删除任何固定equals/hash/toString字段与语义。类型名只用于异常/诊断；不能据此合成用户可见的 `toString`。

String、boxed primitive、boxed aggregate、closure、array与普通class全部使用同一“零个或多个普通分派槽”规则。空vtable合法；codegen不得因表为空补入runtime默认函数。

### 6.3 runtime/core清理

- 删除 `scoop_rt_any_equals`、`scoop_rt_any_hashcode`、`scoop_rt_any_tostring` 及相关声明、测试和codegen签名特判；
- 保留/补齐由typed core primitive调用的int/bool/uint/string转换、内容相等和内容hash helper。它们是具体实现细节，不是Any fallback；
- `scoop_rt_string_identity`若仅用于旧String vtable槽则删除；String的 `ToString.toString` 可直接由compiler/core实现为返回this；
- runtime TypeDescriptor定义、静态String descriptor、测试手写descriptor及所有编译器生成descriptor同步移除固定三槽假设；
- M1–M13 fixture中依赖 `println(Any)`、class默认地址字符串或reference `==` identity的源码改为显式能力：保留具体 `ToString` 静态类型、实现interface，或把identity判断改为 `===`。这属于语义迁移，不以兼容fallback掩盖。

## 7. pipeline 设计

### 7.1 parser / AST

- 增加通用 `operator` function modifier、`TypeBound`、`WhereClause` 与每项独立span；
- `ClassDecl`与function/struct/enum/interface一样保存完整`type_params`；class/struct/enum的constructor或variant、base/interface列表与成员都在同一宿主type-parameter scope解析。parser保留non-interface type上的variance modifier，由HIR执行M14的invariant限制；
- `where`与inline bound可用于top-level/local/extension function及class/struct/enum method；method modifier AST必须保留足以在HIR区分final/open/abstract/override的结构，不能在parser看到generic就静默改成final；
- `@Intrinsic`允许登记表批准的struct/class省略普通representation声明；AST仍只保留annotation和源码语义成员，不制造伪字段或伪constructor；
- generic声明dump/golden保留inline bound与where的源码顺序；
- parser只检查局部语法与重复modifier，不判断bound目标种类或operator签名。

### 7.2 HIR

- `ExportHir`拥有export侧type parameter、generic class/struct/enum/interface template及application、constraint、conditional equality与 `BoundCallableRef`；每种nominal type的declaration/application/concrete specialization使用不同typed id，且所有export id与本Cone concrete侧隔离；
- AST/HIR没有`Self` type kind或object-safety flag。HIR按普通名称解析源码`Self`，在interface定义处拒绝method-level type parameter，并保证每个合法interface method都产生完整itable signature；
- HIR为non-interface generic method建立独立`ExportGenericMethodId`与完整application identity，执行non-virtual modifier/override检查、两组参数固定点推导、method bound检查及callable-reference具体化；interface method-level type parameter仍在定义处拒绝；
- HIR按driver传入的`IntrinsicDeclarationPolicy`验证provider authority，再把type/function intrinsic name一次性正规化为封闭typed kind；test allowlist不进入输出；
- HIR类型检查generic template时只使用声明bound暴露的能力；本Cone实例化和下游Cone实例化走同一个concretizer。generic nominal concretization按kind同时完成field/variant、base、interface、member和constructor替换闭包；
- concretizer分别维护nominal与callable的typed instantiation state；普通递归复用已经intern的concrete identity，参数变换不为identity的recursive callable SCC按2.9诊断polymorphic recursion，不能靠深度/实例数上限终止；
- `LocalConcreteHir`包含实例化后的class/struct/enum/interface实体、普通call target、派生函数体、完整implements/parent关系、intrinsic fixed/family representation与core typed identities，不含generic application、constraint节点或未解析宿主参数；
- 只有编译器会脱离普通源码调用主动构造的实体（例如`CompilerExceptionCore`中的异常类型与constructor）才形成非可选well-known typed结构。`ToString` / `Hash` / operator equals、`print` / `println`及其基础类型实现都通过普通声明、conformance和callable identity流动，不形成capability-specific core结构；
- 所有bound、operator、派生失败和core contract错误在HIR结束前报告。

### 7.3 MIR

- 只接收 `LocalConcreteHir`，为fully specialized generic class/struct/enum、普通interface实现及derived equals建立普通concrete MIR实体；不解释generic bound、nominal template或宿主type substitution；
- generic method到达MIR时已经是完整的direct concrete callable；MIR不得为其分配vtable/itable slot，也不得从`owner_type_param_count`、函数名或残留type argument重新拼装实例；
- normal call kind沿用HIR已解析target与concrete receiver信息，MIR只定稿direct/virtual/interface表实体；
- vtable/itable从零重新布局，移除Any synthetic members、fixed prefix、boxed aggregate universal equals及primitive universal toString生成路径；
- MIR meta导出typed slot及callable关系，不能把slot降成symbol string后让LIR/codegen重建。

### 7.4 LIR / codegen / runtime

- LIR消费MIR分派表与call签名，发射无固定前缀的descriptor/table；
- derived body中的字段比较、concat与core helper均是带完整签名/effect的普通call；
- codegen不再识别 `scoop_rt_any_*` 名字或假设“前三槽”；
- runtime只提供具体String/primitive操作和descriptor/itable lookup设施，不决定语言级operator或interface conformance。

## 8. 附加部分：IR 输出完备性整改

本节是M14验收门，不是可延期backlog。共同准则：必要信息由最早拥有该语义的stage生成，以非可选、类型化、互斥的数据结构向下传递；消费者不得通过上下文expected type、arena反向扫描、FQN/symbol字符串、并行Vec索引、`Option`组合或默认值补齐。

原审计中的“Any方法typed id丢失、MIR按短名映射固定槽”不在本节重复：第6节拆除整个机制后该路径自然消失。

### 8.1 MIR表达式保留完整类型

MIR表达式统一为：

```text
MirExpr {
    ty: MirTypeId,
    kind: MirExprKind,
}
```

`ty` 对所有源码表达式、desugared表达式和MIR synthetic表达式均为非可选；Unit、Nothing、null niche、box/unbox、array literal、enum payload、closure、callback及coroutine结果都不能例外。HIR→MIR转换在创建节点的同一操作填入类型。

LIR lowering删除 `expr_ty` 式递归重建、expected/context参数、空数组靠目标类型补元素类型、Unbox靠使用点猜payload等路径。若某个lowering操作需要storage/layout type，它只能读取表达式自身 `ty`及typed entity映射。

### 8.2 typed intrinsic 与完整exception core

`@Intrinsic("name")` 的字符串只存在于AST和HIR core contract验证入口。验证后 `ExportHir` / `LocalConcreteHir` 使用封闭 `IntrinsicId` / `IntrinsicKind`；runtime helper种类也用typed enum，不把symbol字符串当作kind。MIR展开按该id匹配一次，未知值在Rust类型上不可构造。

`LocalConcreteHir` 的 `CompilerExceptionCore` 必须非可选地包含编译器会主动生成的全部异常类型及constructor callable：

- `Throwable`（catch-all/type relation）；
- `UnwrapException`；
- `ClassCastException`；
- `ArithmeticException`；
- `IndexOutOfBoundsException`；
- `IllegalStateException`。

每项使用concrete typed type/callable ref，可明确表示本地或上游定义；HIR验证继承与constructor签名。MIR禁止遍历class arena按short name/FQN寻找这些类型，也禁止找不到后panic或改用其他异常。

### 8.3 canonical function type关系

每个HIR侧 `FunctionTypeId` 必须直接、非可选地关联唯一canonical `TypeId`：

```text
FunctionType {
    canonical_type: TypeId,
    suspension: Suspension,
    params: Vec<TypeId>,
    result: TypeId,
}
```

interning同时创建/返回这对identity并保证一一对应；Export与LocalConcrete侧各使用自己的typed id家族。任何stage都不得扫描type arena寻找 `Type::Function(id)`来反推canonical type，也不得临时再intern一个“看起来相同”的type。

### 8.4 LIR call target、signature、result与effect

每个直接、间接、runtime、extern call及其invoke形态都必须携带：

- typed target：local/external callable、typed runtime function或typed dispatch slot；link symbol只是target实体中的最终发射字段；
- layout完成后的完整signature：全部参数（含hidden receiver/continuation/storage参数）、calling convention和return convention；
- 四分的 `CallEffect::{ManagedSafepoint, NoGc, NativeSafe, NativeBorrowed}`，不能用函数所属模块、symbol前缀或单一leaf布尔值推断；
- may-throw call使用同一call实体再附normal/unwind edge；`Call`与`Invoke`不能各自维护一套会漂移的签名推断。

target、signature、effect和return shape不是四个可彼此矛盾的平行字段，而是由return convention区分的typed target家族：

```text
CallSite =
    Void { target: VoidCallTargetId, args }
  | Direct { target: DirectCallTargetId, out: TempId, args }
  | IndirectResult { target: IndirectResultCallTargetId, storage: Value, args }

VoidCallTarget / DirectCallTarget / IndirectResultCallTarget {
    destination: CallDestination,
    signature: CorrespondingSignature,
    effect: CallEffect,
}
```

三个target/signature id家族在Rust类型上互不兼容；indirect-result signature自身携带result storage type与scan，不能再与callsite放一份平行scan。dispatch table的每个entry保存typed callable ref与对应target/signature id；indirect call引用typed slot id。codegen只按target signature构造LLVM function type与return ABI，不再从参数值、result temp、函数名或“若声明不存在就按调用点add_function”猜签名。runtime函数登记表也在LIR之前产生完整target；codegen不按symbol match返回类型。

### 8.5 pointer null保留provenance

LIR把无类型 `Value::NullPtr` / `ConstantValue::NullPtr` 替换为携带 `PointerKind` 的null：

```text
NullPointer(PointerKind::Managed | Raw | Code | Metadata)
```

`FunPtr` / callback地址null是Code，`Ptr`是Raw，managed niche `None`是Managed，metadata链尾是Metadata。`value_ty`不得统一返回Raw。所有pointer cast必须是显式instruction，不能靠同一个null常量抹掉provenance。

### 8.6 metadata只用typed identity连接

- LIR meta为每个layout分配 `LayoutId`，并输出非可选 `WellKnownLayouts`（至少含String）；String literal/concat/descriptor直接引用该id，不扫描 `layouts` 查找名字 `"String"`；
- TypeDescriptor parent、interface key、vtable/itable slot使用 `TypeDescriptorRef` / `DispatchEntry` / `CallableRef`。跨Cone引用用区分local/external的typed enum；external实体内部可以携带link symbol，但symbol不充当semantic id；
- `Vec<String>` 形式的vtable/itable内容退役。codegen不得根据runtime symbol猜slot函数签名；
- MIR单态化metadata分别使用`GenericFunctionSourceId`、`ParameterizedMethodSourceId`与`GenericMethodSourceId`；instance按variant携带对应source id。method instance还必须直接携带exact concrete owner，owner参数与method自身参数保持两组，其中语义上非空的参数组使用不可表示空值的容器。`source: String`与把两组参数压平的`type_args: Vec<Type>`均须退役；
- display name仍可作为诊断数据保留，但任何layout、subtyping、dispatch、core type或ABI决策都不能读取它。

### 8.7 消除可构造的非法组合

以下关系改为sum type或成对实体，从结构上排除缺失/错位：

- `NativeGlobalAccess::{ReadOnly { get, address }, Mutable { get, set, address }}` 取代 `mutable: bool + set_bridge: Option<_>`；bridge全部使用typed id；
- `ForeignCallbackOperation` 在MIR只读取统一`MirExpr.ty`，不得在kind中再平行保存一份`result_ty`；到LIR后，`Retain/State/Failure`结果型variant携带必需out，`Release`为无结果variant，不再使用 `operation + out: Option<_>`；
- caller root使用 `NonEmptyRefScan`，其constructor拒绝/无法表示 `None`、空offset与全空sequence；GC-free值不能形成 `CallerRoot`；
- tagged enum field表示为单个 `EnumFieldRepr { ty, offset }` 列表，取代 `fields` / `field_offsets` 两个平行Vec；variant slot与字段offset在构造时一并验证；
- call result按8.4的typed target/return sum表达；同样的互斥return convention复用于function declaration与return lowering，禁止`Void`签名配有out、value签名缺out或indirect signature另配一份不一致scan。

这些结构的serializer/dump/golden同样使用新类型；不得只在lowerer里assert两个旧字段“通常一致”。

## 9. 诊断与测试

### 9.1 parser / HIR negative

- inline bound、多个where constraint、generic class/struct/enum/interface template与独立application identity、F-bound与源码顺序dump；
- generic nominal arity错误、裸/部分application、constructor/variant实参无法完整推导、bound失败、class/struct/enum声明点`in`/`out`以及替换后非法base/interface/override；
- generic method的open/abstract/override/interface声明、试图实现或覆盖dispatch slot、方法参数与宿主参数重名、显式方法实参数目错误、method bound失败及无法由期望函数类型具体化的callable reference；
- generic recursive callable SCC中的identity参数传递正例，以及`f<T> -> f<Option<T>>`和互递归参数增长的定义处诊断；
- `Self`未定义时按普通unknown type诊断、存在同名用户类型时按该nominal identity解析且绝不替换为宿主；interface method-level type parameter在声明处拒绝，不产生“声明合法但某类receiver不可调用”的诊断分支；
- intrinsic type省略representation的合法core/test-provider形态、普通来源拒绝、按input allowlist授权、授权不传递，以及未知kind/错误target/重复provider/错误type-parameter schema或成员签名在测试模式下仍被拒绝；
- unknown type parameter、重复bound、class/value/function type作为upper bound、缺失interface实参、kind/interface混用；
- 显式/推导type argument不满足bound；条件派生equality失败时给出字段与variant路径；
- bounded receiver无成员、互不相关bound产生歧义、interface方法自身声明type parameter在定义处拒绝；同时正向覆盖generic interface宿主参数替换后经interface/bounded receiver调用；
- 同一generic interface的相同application经diamond路径去重、不同application保持独立；无法同时满足的成员obligation在实现处诊断，不按template id或文本签名误合并；
- operator equals缺 `operator`、参数数目/返回值错误、generic/suspend/extension equals、override flag不匹配；
- `Any == Any`、class无equals、interface未声明equals、无ToString类型传给print；
- 只有普通 `toString` 成员但未显式adopt `ToString`的类型不能满足bound、显式ToString/Hash实现签名错误。

所有错误在HIR报告源码span；MIR/LIR/codegen测试不接受“panic证明negative生效”。

### 9.2 HIR / MIR / LIR golden

- ExportHir锁定typed constraints、generic class/struct/enum/interface template与独立application identity、generic method template/application的宿主与方法参数分组、conditional equality、普通interface conformance、bound member target及canonical function type mapping；
- Export/LocalConcrete HIR反向检查不存在`Self` type variant、object-safety bool/enum或跳过itable的合法interface method；
- LocalConcreteHir锁定generic nominal application与bound call已消失，每个class/struct/enum/interface specialization分别具有完整field/variant/base/parent/interface/member闭包和非可选GC-free信息；generic method只留下带完整origin与两组concrete arguments的direct callable，不进入任何dispatch table；派生函数与implements完整，intrinsic fixed/family representation、exception/intrinsic/core identities非可选；
- MIR锁定每个expression有type、generic nominal specialization、derived body、direct/virtual/interface选择及零前缀vtable；generic callable metadata还须锁定三类typed source id、exact method owner与未压平的参数分组；
- LIR锁定typed call target/signature/result/effect、pointer null provenance、typedmetadata refs、non-empty caller root与结构化enum field；
- codegen测试明确禁止symbol-driven function type fallback、Any runtime特判和固定slot偏移。

### 9.3 core/runtime与端到端

- String、Boolean、Int、UInt的ToString；String/basic Hash的一致性；地址变化不影响字符串/hash；
- 普通generic class/struct/enum的显式/推导构造、嵌套application、bound与宿主成员/generic成员组合；generic method覆盖普通/泛型宿主、显式/推导method实参、method bound、继承后direct调用、suspend/closure/NoGC组合及expected-type callable reference，并反向检查不进入vtable/itable；class另覆盖generic base/interface及不同实参隔离的TD/vtable/itable，struct/enum另覆盖不同实参隔离的value layout、逐variant GC-free flag与递归扫描，generic interface另覆盖variance、父interface闭包及不同application的TD/itable key隔离；
- `is` / `as` / `as?`区分不同generic class/interface application，generic body中的type parameter测试在实例化后引用exact concrete TypeDescriptor，不存在擦除或裸generic RTTI；
- generic `Throwable`子类的每个application同样具有独立TypeDescriptor：exact application catch只匹配相应实例，catch其非generic基类仍可匹配全部实例；不因JVM擦除限制而禁止generic异常类；
- `EqualTo<T>`一类显式实现者类型参数契约经concrete/interface/bounded receiver调用；所有interface application都能用于变量、字段、参数、返回值与cast，不存在object-safety差异；
- 同一对象实现/继承`I<Int>`与`I<String>`时使用不同itable key，variance bridge不抹去application identity；能由不同参数overload满足的obligation正确分派，只按返回类型区分等不可表达冲突给出定义处诊断；
- `Array<T>` / `MutableArray<T>`从core generic intrinsic class取得唯一nominal identity，数组字面量、size、下标、转换与GC扫描继续通过typed representation工作；反向检查HIR/MIR不再存在独立built-in array type identity或按名称映射；
- struct/enum/tuple/Unit条件相等派生、嵌套generic、显式同类型equals覆盖派生及失败字段诊断；普通`ToString`只覆盖显式adoption；
- value/reference/interface equals、`!=`一次取反、short-circuit与左右求值各一次；`===`行为不变；
- final/direct、open/virtual、interface/itable、boxed value adjust thunk及跨generic bound组合；
- print/println只通过generic ToString，write仍为Scoop ABI NativeBorrowed call；
- try/catch、closure、suspend与foreign callback中调用derived/interface方法，验证call effect和异常边不丢失；
- 全量回归M1–M13 fixture，并更新所有明确依赖旧Any行为的fixture/快照。

附加整改为每个问题设置反向检查：仓库中不再存在MIR/LIR `expr_ty`猜测、HIR intrinsic payload字符串、异常class名称搜索、FunctionType arena反扫、无provenance null、`Vec<String>` dispatch table、并行enum field vectors及上述 `bool + Option`/`operation + Option`形态。仅新增注释或assert而保留旧表示不算完成。

## 10. 实现顺序与验收门

1. 同步语言/runtime/实现spec，加入统一invariant generic nominal type模型、interface bound、operator规则、intrinsic表示族与无Any固定槽契约；
2. parser/AST为class加入完整type parameter scope，并加入结构化bound、where、operator modifier与intrinsic type声明形态；
3. HIR为class/struct/enum/interface建立各自的export generic template/application/local concrete specialization typed-id家族，并为non-interface generic method建立独立template/application/concrete callable identity；迁移既有`StructId/EnumId/InterfaceId + args`与合并参数Vec表示，完成constructor/variant/method推导、field/variant/base/parent/interface/member替换、non-virtual检查和可终止实例化固定点；
4. 将`Array<T>` / `MutableArray<T>`迁移为core generic intrinsic class，删除独立built-in array type identity；driver/HIR同时加入默认core-only、测试按provider allowlist的内部authority策略；
5. HIR完成intrinsic fixed/family representation正规化、bound检查与bounded resolution；ToString/Hash/equals只通过普通core声明、interface conformance与operator resolution进入这些一般路径；
6. HIR生成conditional equality member与derived body，core显式ToString conformance及generic print/println走普通interface路径；
7. MIR拆除Any synthetic methods/三槽，重建普通typed dispatch；runtime删除地址fallback；
8. 按数据流顺序完成附加整改：HIR typed identity → MIR expression type → LIR call/metadata/provenance/非法组合 → mechanical codegen；
9. 更新core/runtime测试、stage golden、negative与组合fixture，执行格式化、lint和全量测试。

M14 只有在以下条件同时满足时完成：普通用户generic class/struct/enum/interface都以独立template/application/concrete typed identity打通声明、成员与单态化，class/struct/enum完成各自构造和layout，class额外完成继承与分派，interface额外完成variance与itable identity，且任一generic template确实能仅依赖interface bound通过独立/下游实例化；non-interface generic method完整支持两组参数、bound、推导、callable reference与direct concrete emission，所有virtual/override/interface-slot形态都在HIR定义处拒绝，实例化图对普通递归终止且对polymorphic recursion给出结构化诊断；exact generic application参与RTTI/cast而不擦除实参；AST到LIR不存在`Self`语义实体或object-safety分类，每个合法interface application及其全部成员都可经itable使用；`Array<T>` / `MutableArray<T>`使用同一generic class身份并且旧built-in type identity已删除；Int/String/Array等intrinsic type的能力来自core显式声明且固定/表示族representation全程typed，测试authority只放宽指定provider来源；`Any`在所有层都没有隐式方法或固定槽；print/hash/equality都不使用地址fallback；每项附加整改已删除对应的下游猜测路径，而不只是新增一份上游字段后仍保留旧fallback。

## 11. 明确不做

1. class upper bound与可作为普通表达式类型的交叉类型；M14的多个interface bound只形成type parameter能力集合；
2. non-interface generic type的声明点`in`/`out`。class参数必须先处理可能为不同size/alignment的value实参，struct/enum还必须定义不同concrete layout间是否存在转换；M14全部按invariant处理；
3. use-site `in` / `out` projection、star projection与capture conversion。未来设计必须分别规定projected member可读/可写签名、subtyping/推导、RTTI/cast、跨Cone metadata，并决定value-type application是禁止投影还是引入显式existential boxing；不能把`C<*>`当成擦除为`C<Any>`；
4. interface方法自身的type parameter；未来支持必须先定义跨Cone specialization/itable ABI，且不得引入`Self`、trait object或object-safety分类；
5. generic `typealias`的参数、bound、透明展开、循环诊断及跨Cone export；alias不得获得新的nominal/concrete identity；
6. generic extension property，以及nested/inner generic type、generic class companion对宿主参数的可见性。这些依赖尚未实现的property、nested type与object/companion基础能力；普通property本身不能凭空拥有每次访问才实例化的method type parameter。object/companion声明自身不形成generic nominal application，也不能声明宿主type parameter，但其中的普通method未来可以按2.8声明自己的non-virtual参数；
7. 显式类型实参中的`_`占位与“部分写出、其余推导”；M14继续只允许整组省略或整组完整写出；
8. 比2.9更一般的polymorphic recursion及termination proof；资源上限、worklist深度或编译超时不能充当语言规则；
9. 完整Kotlin fresh-variable/postponed-argument constraint system，以及加入projection后的MSC、LUB和overload比较；M14只扩展当前固定点求解器以处理宿主参数、method参数与upper bound，M7已有的简化比较backlog继续有效；
10. runtime generic dictionary、witness参数、反射式bound调用或以代码体积为目标的共享泛型body；M14仅实现单态化，不能把这些机制作为缺失concrete信息的fallback；
11. first-class polymorphic function value、generic lambda/匿名函数、higher-kinded type、associated type、const generic或用户可控specialization。它们不属于当前Kotlin核心兼容范围，也不因M14使用“完整泛型”一词而被隐式引入；
12. 自动派生ToString/Hash、identity hash、`Any`默认字符串化/相等；
13. extension equals参与 `==`，或把 `===` 开放重载；
14. locale/format specifier与StringBuilder优化（进入M16）；
15. 为旧固定三槽提供ABI兼容层；
16. M15的精确stackmap、`gc.relocate`与moving compaction；M14只确保其上游IR不再依赖地址语义并为完整call/root信息打好基础；
17. 让普通用户Cone声明自定义intrinsic，或把测试authority作为稳定CLI/manifest能力公开。
