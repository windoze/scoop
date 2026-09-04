# M20 设计：泛型约束与部分类型实参

版本：0.2（草案）

对应`docs/ROADMAP.md`的M20。M20在M14的exact generic application与M16的统一constraint/MSC内核之上，增加class upper bound和调用点`_`部分类型实参推断，并正式收敛Scoop的nominal generic模型：class、struct、enum与interface的全部类型参数永久invariant，不实现declaration-site `in`/`out`、use-site projection、star projection或capture conversion。

这个决定不是暂时搁置一种方便语法，而是表示与实现边界：Scoop以真值类型、完整单态化和exact RTTI为核心。generic算法通过callable自身的type parameter与bound表达；需要改变容器/包装类型实参时显式`map`/重建；需要保存未知application时使用源码可见的非generic interface或用户定义的type-erased wrapper。编译器和runtime不隐式制造existential类型、generic view、dictionary或按运行期application选择的单态化分支。

## 0. 关键决策与范围

- 所有nominal type parameter固定invariant，源码声明不能写`in`/`out`；同一template的两个application只有全部实参逐项相等时才存在由该template产生的赋值关系；
- type application只接受参数完整的普通类型列表。`G<out T>`、`G<in T>`与`G<*>`均非法；裸`G`和少写参数仍非法；
- 普通class继承和interface conformance不受影响，但其目标必须是声明中写出的exact application。例如`D : B<String>`只建立`D <: B<String>`，不会推导`D <: B<Any>`；
- value向`Any`或已实现interface的普通装箱规则不变，但装箱不会改写外层generic application。`Box<Value>`不能通过装箱成为`Box<I>`；程序必须先把单个value显式转换为`I`，再构造`Box<I>`；
- `Option<T>`、`Iterator<T>`、`Iterable<T>`、`Continuation<T>`、`SuspendTask<T>`与`SuspendRegistration<T>`全部invariant，不保留core专用variance；
- 普通函数类型的参数逆变/返回协变继续按spec 8.1.1处理。它是concrete function value之间的typed adapter关系，不是nominal application variance；
- upper bound扩展为“至多一个class bound + 零个或多个interface bound”。所有nominal bound都是参数完整的exact application；
- 调用/构造的显式type argument list允许用`_`逐项继续推断，但列表仍须覆盖callee自己的全部参数位置；
- M20不增加runtime representation、TypeDescriptor种类、dispatch table或GC协议；所有成功调用在HIR结束前仍必须得到完整concrete type arguments和唯一typed target。

## 1. 统一 nominal invariance

### 1.1 声明与application

合法声明不携带variance modifier：

```kotlin
class Box<T>(val value: T)
struct Pair<A, B>(val first: A, val second: B)
enum Option<T> {
    Some(T),
    None
}
interface Iterator<T> {
    fun next(): Option<T>
}
```

以下声明与类型均为编译错误：

```kotlin
interface Producer<out T>
class Sink<in T>
Box<out Animal>
Box<in Cat>
Box<*>
```

parser可以为恢复保留非法token及span，但成功AST/HIR中的nominal type parameter不需要`Variance`字段，type application argument也不需要`Exact/Out/In/Star` sum；它就是一个arity完整的`TypeRef`列表。

### 1.2 类型关系

对同一nominal template `G`：

```text
G<A1, ..., An> <: G<B1, ..., Bn>
    iff A1 == B1 && ... && An == Bn
```

该规则不区分owner或argument kind：

```kotlin
// Cat : Animal

Box<Cat>       !<: Box<Animal>
Option<Cat>    !<: Option<Animal>
Box<Int>       !<: Box<Any>
Array<Cat>     !<: Array<Animal>
Iterator<Cat>  !<: Iterator<Animal>
```

普通nominal关系仍在application外层生效：

```kotlin
open class Base<T>
class Derived : Base<String>()

val base: Base<String> = Derived() // 合法
// val other: Base<Any> = Derived() // 非法
```

interface conformance同样保留exact参数。一个类型可以分别实现`I<Int>`与`I<String>`；二者拥有不同obligation、TypeDescriptor/itable key与成员ABI，不能按template id合并。

### 1.3 替代variance的API模式

只消费generic值的API优先把操作本身写成generic callable：

```kotlin
fun <T> printAnimals(values: Array<T>)
    where T : Animal {
    for (value in values) {
        printAnimal(value)
    }
}
```

这样`Array<Cat>`可直接调用，不复制、不装箱，也不需要`Array<out Animal>`。interface bound同样允许对value element生成exact specialization：

```kotlin
fun <T> printAll(values: Array<T>)
    where T : ToString
```

需要真正改变application时使用显式转换：

```kotlin
val animals: Array<Animal> = cats.map { it as Animal }
val result: Option<Animal> = catResult.map { it as Animal }
```

value进入引用抽象时先在元素边界显式装箱：

```kotlin
val item: Display = Point(1, 2) as Display
val items: Array<Display> = [item]
```

需要长期保存“类型实参未知”的对象时，API作者必须定义不携带该参数的interface或显式erased wrapper：

```kotlin
interface InspectableBox {
    fun inspect(): String
}
```

这让装箱、能力丢失和动态分派发生在源码可见边界，而不是由`Box<*>`隐式引入。

### 1.4 Expected type与LUB

expected type可以让构造直接选择目标exact application：

```kotlin
fun findAnimal(): Option<Animal> = Some(Cat())

val value: Option<Animal> = if (ready) Some(Cat()) else None
```

这里HIR先由返回/变量类型固定`Option<Animal>`，再检查`Cat <: Animal`作为`Some` payload；不会先构造`Option<Cat>`后做application转换。

已经形成的application不能借expected type改变实参：

```kotlin
val cats: Option<Cat> = findCat()
// val animals: Option<Animal> = cats // 非法
```

无expected type的LUB只有在同template实参逐项相等时保留该application。`Box<Cat>`与`Box<Dog>`不会合流为`Box<Animal>`；若二者都是class ref，可沿普通共同父类型退到`Any`，若是value application则按普通规则装箱整个值。编译器不在LUB中偷偷插入`map`、逐元素copy或type erasure。

## 2. Class upper bound

### 2.1 语法与结构

class bound沿用现有inline/`where`语法：

```kotlin
open class Node<T>(val value: T)
interface Named {
    fun name(): String
}

fun <T> render(node: T): String
    where T : Node<String>, T : Named =
    node.name()
```

每个type parameter的nominal bound set为：

```text
NominalBounds {
    class: None | One(ClassBound),
    interfaces: Vec<InterfaceBound>,
}
```

- class bound至多一个，可以同时有多个不同interface bound；源码顺序只用于诊断与golden；
- bound必须是参数完整的exact class/interface application；`Node`、`Node<*>`与`Node<out T>`均非法；
- value type、函数类型、`Any`与另一type parameter不能作为nominal bound；
- class bound蕴含`ref`，但interface bound仍可由显式实现它的value type满足；
- `value`/`ref` kind bound与任何nominal bound互斥；多个class bound即使彼此存在继承关系也非法；
- `T : Base<T>`等F-bound继续合法；bound cycle沿M14的结构化规则检查，不用深度/超时截断。

### 2.2 Member resolution与actual验证

bounded receiver的候选来自唯一class bound、全部interface bound及它们的继承闭包。class member按普通visibility/modality规则进入候选；多个bound形成编译期能力集合，不产生可作为表达式静态类型的intersection type。

actual type必须同时满足全部bound：

- class actual通过普通继承到达完全相同的class application；
- class/value actual通过显式conformance到达完全相同的interface application；
- 不允许用同template不同实参之间的关系满足bound；
- 推导把bound作为constraint固定点的一部分，不能先选类型再把bound失败降级为警告或`Any`。

Export HIR中的bound member保存typed `BoundCallableRef`；本Cone或下游Cone实例化后解析为完整direct/virtual/interface target。LocalConcrete HIR不含bound placeholder、witness/dictionary参数或待验证constraint。

## 3. `_`部分类型实参

### 3.1 语法

```kotlin
fun <A, B> pair(first: A, second: B): Pair<A, B> = Pair(first, second)

val p = pair<Int, _>(1, "value")
val q = Pair<_, String>(1, "value")
val r = receiver.convert<_, String>(input)
```

call/constructor显式type argument使用独立AST：

```text
CallTypeArgument = Explicit(TypeRef) | Infer { span }
```

它不能与nominal type application共用带可空type的节点；后者不存在`_`位置。

### 3.2 Arity与归属

- 显式列表存在时，长度必须等于callee自身type parameter arity；`f<Int>()`不能以隐含尾部`_`补齐；
- 每个`Explicit`固定候选对应变量，每个`Infer`创建candidate-local fresh inference variable，并与整组省略时的变量进入同一固定点；
- class/struct constructor与enum variant列表对应nominal host参数；constructor/variant不声明第二组参数；
- generic method的显式列表只对应method自身参数，receiver的host application已经exact确定，不重复列出；
- `_`不能出现在type annotation、nested type application、bound、base/interface、`is`/`as`目标、函数类型或export metadata；
- `_`不影响运行期求值顺序，也不是值实参placeholder。

### 3.3 求解与结果

fixed entry、`_`变量、receiver、普通实参、expected type、postponed argument及全部bound共同进入M16的candidate-local constraint session。成功候选必须唯一解出每个`_`；未解、多解或bound失败指向该`_`的span并展示其他fixed位置。

winner commit一次性生成完整concrete arguments、唯一typed callee、source argument mapping、default/vararg materialization及必要的普通coercion。失败候选不能永久登记generic instance、lambda/default实例或constructor application。`_`、inference variable和候选constraint均不得进入LocalConcrete HIR。

## 4. Constraint、MSC与LUB

M20不新增另一套resolver，只扩展M16既有session：

1. shape filter验证显式列表总arity，并把`Explicit/Infer`绑定到候选自己的参数空间；
2. 收集ordinary equality/subtyping、nominal argument equality、kind/class/interface bound与postponed argument constraint；
3. 固定点唯一确定全部type arguments；
4. 检查每个actual、default、vararg及expected result；
5. winner原子commit完整concrete target。

MSC仍从source declaration创建与当前actual inference隔离的fresh variables，通过普通subtyping和nominal invariance做pairwise forwarding。它不能读取两个候选在本次调用中猜出的type arguments，也不能因某个`_`更早求出而偏好候选。M17的“较少实际default”“无vararg”及非参数化优先规则保持原顺序。

LUB遵守1.4。不同generic arguments不会合成新的application；这项限制同时适用于reference/value owner、array literal、`if`/`when`/`try`与solver lower bounds。

## 5. IR、runtime与跨Cone边界

### 5.1 AST / HIR

- nominal `TypeParameter`只保存name、bounds及span，不保存variance；
- nominal application只保存arity完整的`Vec<TypeRef>`；`in`/`out`/`*`在parser/HIR恢复路径诊断，不能进入合法type arena；
- call type argument使用`Explicit/Infer`封闭sum；
- Export HIR为class/interface bound使用不同typed id，并以`None | One`结构表达class bound；
- exact class/struct/enum/interface application id与template id类型不兼容；不同kind application id仍互不兼容；
- candidate-local inference variable只存在于`hir-lower`事务，winner commit后全部消去。

### 5.2 LocalConcrete / MIR / LIR

LocalConcrete HIR只包含：

- fully substituted exact type/application；
- 完整concrete callable/constructor target；
- 已解析的direct/virtual/interface dispatch；
- ordinary box/unbox/function-type adapter等既有coercion。

它不包含`_`、inference variable、bound placeholder、projection/capture/view或待实例化请求。MIR/LIR不感知部分推断，也不添加generic dispatch种类。

每个exact generic application继续拥有独立layout/TypeDescriptor/itable key。`is`/`as`只比较参数完整的exact target及其普通base/interface closure；不存在template-only、star或view descriptor。value application之间没有隐式重建，class application之间没有source/target member bridge。

### 5.3 Export metadata与未来`.slib`

Export HIR meta新增class bound结构；`_`只存在于source call，winner commit后导出/本地实例仍只含完整concrete arguments。成功导出仍只含普通generic template、exact application与typed dependency closure。MIR/LIR meta继续导出exact dispatch/layout信息，不增加view row、capture branch、existential ABI或value reconstruction relation。

M20锁定供未来下游Cone消费的`ExportHir`结构：class/interface bound保持typed id，generic template不含`_`，本Cone的`LocalConcreteHir`实例只含完整实参。真实`.slib`打包/reader、依赖图与下游Cone编译由M23整体实现；M23读取上游template后必须独立运行相同的`_`求解与bound验证，再生成完整本地实例。stage之间仍只通过各自IR/meta crate通信，不读取上游stage实现crate。

## 6. 诊断与恢复

至少提供以下稳定诊断：

- type parameter declaration出现`in`/`out`；
- type application出现`in`/`out`/`*`，并建议generic callable、exact interface或显式wrapper；
- 裸generic、arity不足/过多或constructor显式列表归属错误；
- class bound重复、与kind bound冲突、target不是class或application不完整；
- interface/class bound中的application实参不完整或actual不满足exact关系；
- `_`出现在非call type位置、显式列表未覆盖全部arity、某个`_`未解/多解/bound失败；
- `G<S>`赋给`G<T>`时明确指出nominal invariance和首个不同argument，而不是只报普通type mismatch；
- 无expected type的不同application分支无法形成预期generic LUB时，显示双方exact application及建议annotation/显式转换。

parser以当前type/call argument list为恢复边界；HIR对非法declaration/candidate保持事务隔离。存在任一diagnostic时不输出残缺Export/LocalConcrete module；MIR/runtime不承担source错误恢复。

## 7. 测试计划

### 7.1 Invariance

- class/struct/enum/interface的不同argument application全部不能互相赋值；
- reference subtype、value实现interface、`Any`装箱均不能穿透外层application；
- ordinary generic base与interface conformance只到达声明的exact application；
- `Option<Cat>`/`Option<Animal>`、`Iterator<Cat>`/`Iterator<Animal>`与`Array<Cat>`/`Array<Animal>`negative fixture；
- declaration/use-site `in`/`out`、star及nested star的parser/HIR诊断与恢复；
- `is`/`as`/generic catch区分完整application，不存在通配target。

### 7.2 替代API与expected type

- `<T : Class>`和`<T : Interface>`generic consumer直接接收不同exact array/iterator application；
- value element经interface bound保持unboxed specialization；
- expected `Option<Animal>`下`Some(Cat())`、`None`和分支构造成功；已经形成的`Option<Cat>`赋值失败；
- 显式`map`/逐元素转换、value显式装箱后构造目标application；
- 无expected type的不同application LUB不合成新application。

### 7.3 Class bound

- 唯一class + 多interface bound、F-bound、继承member和actual验证；
- class bound的generic argument逐项exact；
- duplicate class、kind冲突、value/function/`Any`/type-parameter bound negative；
- bounded receiver的direct/virtual/interface target，以及`ExportHir` template到本地完整实例的边界golden；真实跨Cone实例化随M23验证。

### 7.4 `_`与overload

- top-level/local/extension/generic method/class/struct/enum constructor与variant中的fixed + `_`组合；
- `_`结合named/default/vararg/lambda/callable reference/expected type/postponed nested constructor；
- arity不足、非call位置、未解、多解和bound失败；
- single-candidate失败、MSC单向/双向/互不支配及完整candidate trace；
- 失败candidate不污染application/default/lambda arena。

### 7.5 Stage与export边界golden

AST / ExportHir / LocalConcreteHir / MIR / LIR golden锁定：

- nominal type argument与call `Explicit/Infer`节点分离；
- class/interface bound typed id及`None | One`class结构；
- inference variable和`_`在LocalConcrete前消失；
- TypeDescriptor、itable、layout与RTTI只引用exact application；
- `ExportHir`足以让未来下游完成`_`求解、bound检查和单态化，不含view/capture/value-conversion metadata；`.slib`往返与实际下游编译属于M23。

组合fixture至少覆盖class bound + `_` + constructor/default/vararg + closure + suspend caller + exception + moving GC，并在同一golden中锁定generic template的`ExportHir`与完整本地实例。真实跨Cone template实例化随M23补充。M1至M19全部回归。

## 8. 实现顺序与提交门

1. 同步language/runtime/impl spec及core声明，统一“所有nominal application invariant”的术语；
2. 删除interface variance bridge、variance AST/HIR字段和相关测试，确保exact RTTI/itable回归；
3. 完成call `Explicit/Infer` AST/parser与错误恢复；
4. 实现class bound的typed结构、定义检查、member resolution与actual验证；
5. 把`_`接入M16 candidate-local constraint、postponed argument、MSC及winner commit；
6. 补齐expected-type构造、invariant LUB和首个不同argument诊断；
7. 更新Export/MIR/LIR的exact-only跨Cone数据结构与export/local边界golden；`.slib`打包、reader及真实跨Cone golden留给M23；
8. 完成negative/组合fixture及M1至M19回归。

每批代码变更先执行`cargo fmt --all`与`cargo clippy --workspace`，再运行对应crate test、stage golden和fixture。不得保留“interface仍variant”的core例外、只供Array使用的projection、`Option`隐式payload widening、runtime wildcard descriptor或失败时回退`Any`。

M20只有在以下条件同时满足时完成：全部nominal声明/application使用同一invariant关系；class bound可与多个interface bound组合并完整保存在`ExportHir` template中；`_`覆盖所有generic callable/constructor入口并在HIR内完全消去；expected type足以支持直接构造目标`Option`等常见模式；RTTI、dispatch、layout和供M23打包的metadata保持exact-only；旧variance/projection/capture路径及metadata已经删除而不是成为不可达旁路。真实`.slib`往返和跨Cone实例化是M23的完成门，不提前把其driver/import/re-export子集塞入M20。

## 9. 明确不做

1. nominal declaration-site variance、use-site projection、star projection与capture conversion；这些是明确排除的语言能力，不进入后续backlog；
2. implicit existential/type erasure、runtime dictionary、共享erased generic body或JIT specialization；
3. generic application之间的隐式`map`、逐字段/逐元素重建、value boxing或class source-specialization bridge；
4. 可作为普通表达式静态类型的intersection/union type；multiple upper bounds只属于type parameter能力集合；
5. generic `typealias`、nested/inner generic type与generic class companion参数作用域；
6. interface method自身type parameter及跨Cone generic virtual slot；
7. first-class polymorphic function value；callable reference必须得到一个完整concrete application；
8. 一般polymorphic recursion；M14的递归SCC identity termination规则保持不变；
9. context parameter与整数literal widening；分别由后续里程碑扩展同一solver；
10. 为减少annotation而自动把已经形成的`G<S>`转换为`G<T>`；只能让expected type参与尚未定型的构造/调用推断。
