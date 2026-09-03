# M18 设计：callable 表面补齐

版本：0.2（草案）

对应`docs/ROADMAP.md`的M18。目标是在M16统一constraint/overload内核与M17完整source argument protocol之上，补齐三组互相耦合的callable表面：完整operator/infix、property-like `invoke`以及`?.method()`。

M18不会为这些语法分别建立解析捷径。operator operand、infix右值和property-like invoke都必须转成M16/M17的candidate-local调用模型；winner确定后再统一落实receiver、显式实参、default与`vararg`的求值顺序。`LocalConcreteHir`及其后的stage只能看到完整typed call、temporary、branch与assignment，不能看到source operator或property-like候选。

## 0. 关键决策与范围

- operator能力只来自已经验证的源码`operator`声明。HIR使用封闭`OperatorKind`，调用时按typed role筛选；不得按名称、FQN、宿主类型或intrinsic symbol猜测；
- 除Scoop收紧为member-only的`equals`外，operator可由member或extension提供。top-level/local函数只有带extension receiver时才能标记`operator`或`infix`；
- primitive、String、Array/MutableArray与Ptr中能由Scoop声明表达的operator都迁到core源码表面。HIR先按普通候选规则选择core声明，随后才可按其已验证的typed intrinsic kind正规化为专用节点；
- `infix`是独立的typed modifier，要求member/extension与恰好一个required、非`vararg`参数。`operator infix fun invoke`可使property-like callable参与中缀调用；
- property-like调用分两步：先得到唯一property/value，再只对其静态类型解析一次`operator invoke`。函数类型调用仍是M11的内建callable-value call；`FunPtr`仍不可调用；
- 安全方法调用严格展开为Option分支。outer receiver只求值一次，property read、显式实参、default与`vararg`只在`Some`分支执行，结果始终再包一层`Option`；
- `++`/`--`、复合赋值及下标读写先建立typed place plan，receiver/index/右值均只求值一次。计划在HIR内完全正规化，不能把source AST留给MIR重放；
- `&&`/`||`、`===`/`!==`、`=`, `?:`, `!!`, `is`/`as`及安全导航本身保持不可重载；`==`/`!=`继续服从M14的member-only equals语义；
- M18接受并导出`componentN`与`iterator` operator identity。class位置解构在本里程碑消费`componentN`；`for`/range core类型仍在M22，属性委托operator协议仍在M21；
- context parameters已经从M18撤出，见第9节；本设计不为它预先固定函数类型、调用决议或ABI结构。

## 1. 语言表面

### 1.1 operator声明

```kotlin
struct Vec2(val x: Int, val y: Int) {
    operator fun plus(other: Vec2): Vec2 =
        Vec2(x + other.x, y + other.y)

    operator fun unaryMinus(): Vec2 = Vec2(-x, -y)
    operator fun get(index: Int): Int =
        if (index == 0) x else y
}

operator fun Vec2.times(scale: Int): Vec2 =
    Vec2(x * scale, y * scale)

val a = Vec2(1, 2) + Vec2(3, 4)
val b = -a
val c = a * 2
val x = c[0]
```

合法operator可以是generic或suspend，也可以同时标记`infix`。override必须保留operator/infix flag；普通同名函数不会自动取得约定能力。

M18支持以下封闭角色：

| 类别 | `OperatorKind` | 参数与结果约束 |
| --- | --- | --- |
| 一元 | `UnaryPlus`, `UnaryMinus`, `Not` | 0个参数 |
| 更新 | `Inc`, `Dec` | 0个参数；结果`<: receiver` |
| 算术 | `Plus`, `Minus`, `Times`, `Div`, `Rem` | 1个参数 |
| 区间 | `RangeTo`, `RangeUntil` | 1个参数 |
| 包含 | `Contains` | 1个参数；返回`Boolean` |
| 下标 | `Get`, `Set` | get至少1项；set至少2项且最后是非vararg写入值；set返回`Unit` |
| 调用 | `Invoke` | 任意参数；完整复用M17调用形态 |
| 复合赋值 | `PlusAssign`, `MinusAssign`, `TimesAssign`, `DivAssign`, `RemAssign` | 1个参数；返回`Unit` |
| 比较 | `CompareTo` | 1个参数；返回`Int` |
| 相等 | `Equals` | M14规则：member、1参、`Boolean`、非generic、非suspend |
| 约定 | `Component { index: NonZeroU32 }`, `Iterator` | 0个参数 |

固定元数operator可以声明default，因为operator语法仍会提供相应operand；`get`、`set`和`invoke`使用M17的default/`vararg`能力，其中`set`最后一个写入参数不能是`vararg`。`componentN`在声明校验时把正整数索引存入typed variant，使用点不能重新解析函数名。

属性委托的`provideDelegate` / `getValue` / `setValue`不属于本表。M21定义其reflection-free协议后再增加对应角色，M18不能仅按名称提前接受。

### 1.2 表达式映射

HIR使用下表建立调用形态，但不复制AST文本：

| 源码 | 概念调用 |
| --- | --- |
| `+a`, `-a`, `!a` | `a.unaryPlus()`, `a.unaryMinus()`, `a.not()` |
| `a + b`, `a - b`, `a * b`, `a / b`, `a % b` | `a.plus(b)`等 |
| `a..b`, `a..<b` | `a.rangeTo(b)`, `a.rangeUntil(b)` |
| `a in b`, `a !in b` | `b.contains(a)`，后者再对`Boolean`结果取反 |
| `a[i, j]` | `a.get(i, j)` |
| `a[i, j] = value` | `a.set(i, j, value)` |
| `a(args...)` | 函数值调用，或一次`a.invoke(args...)` |
| `a < b`, `a <= b`, `a > b`, `a >= b` | `a.compareTo(b)`的`Int`结果与0比较 |
| `a == b`, `a != b` | M14/11.11的member equals目标，`!=`对结果取反 |

operator调用按显式receiver规则先求值receiver，再求值实参。因此`item in container`先求值`container`、再求值`item`；这是语言规范锁定的可观察顺序。其他二元调用按概念调用的receiver、operand顺序求值，default与`vararg`继续使用M17协议。

`a(args...)`至多应用一次invoke convention。若所选`invoke`返回另一个可调用值，必须显式写`a(args...)(more...)`；resolver不能递归寻找下一层`invoke`。

### 1.3 infix

```kotlin
infix fun Set.union(other: Set): Set = ...

operator infix fun Command.invoke(argument: String): Result = ...

val all = left union right
val result = command "build"
```

声明要求：

- 必须具有dispatch或extension receiver；当前`this`上的中缀调用必须显式写`this name rhs`；
- 恰好一个required、非`vararg`参数；带default的参数不是required，因此不合法；
- 可以是generic或suspend；`operator`与`infix`是正交modifier；
- property-like中缀调用要求最终选中的`invoke`同时具有`operator`与`infix`角色。

所有infix name共用一个左结合优先级，不支持用户声明fixity。完整优先级从高到低为：postfix、prefix、cast、乘法、加法、range、infix name、Elvis、`in`/`is`、比较、相等、`&&`、`||`、赋值。parser golden必须用无括号组合锁定每层边界。

### 1.4 安全方法与property-like调用

```kotlin
val maybeClient: Client? = ...
val reply: Reply? = maybeClient?.send(buildRequest())

struct Handler(...) {
    operator fun invoke(message: String): Reply = ...
}

val handler = Handler(...)
val direct = handler("ping")
val fromField = service.handler("ping")
```

`maybeClient?.send(...)`仅在`Some`分支读取payload、解析并执行调用。若`send`返回`Reply?`，整个表达式类型是`Reply??`；不能因为两层底层表示都是Option而展平。

property-like调用先读取`handler`或`service.handler`，再把结果作为invoke receiver。显式type arguments、命名/spread与尾随lambda全部属于invoke，不属于property读取。M18可用的property-like来源是local/parameter/capture、global、primary-constructor property与已有field；M21增加普通/extension property、object和companion后，只扩展property candidate source，不改变本设计的分区算法。

## 2. operator决议与正规化

### 2.1 声明期验证

parser只保留`operator`/`infix`各自的modifier span、名称与参数形态，不判断角色。HIR signature phase一次完成：

1. 检查receiver形态、合法名称、参数元数、default/`vararg`位置与返回类型；
2. 把名称正规化为`OperatorKind`，把`componentN`索引解析为`NonZeroU32`；
3. 检查generic、suspend、extern与intrinsic组合；
4. 把operator/infix作为override signature contract匹配；
5. 只有完整成功的声明才进入`ExportHir`或本地signature表。

不能用`Option<OperatorKind>`表示“写了operator但校验失败”的半成品；失败声明不能进入body lowering。合法的普通非operator函数仍可使用相同名称，因此`None`只表示真实没有operator modifier。

`inc`/`dec`的返回subtyping与generic bound一起证明；若对template无法证明则在声明处报错，不能留到某个application碰运气。`iterator`在M18只验证零参数角色；M22在具体`for`使用点继续要求结果满足11.8的`Iterator<T>`协议。

### 2.2 调用候选

operator call构造普通`CallableView`并附加`required_operator: OperatorKind`。成员、extension scope、owner/method type parameter及M17参数metadata都来自同一view。角色filter在shape prefilter阶段发生，随后完整复用：

- receiver与显式type argument constraint；
- named/default/`vararg` source mapping；
- postponed lambda、callable reference、`None`与空数组；
- bound与fixed-point inference；
- pairwise fresh-variable MSC；
- suspend、unsafe与NoGC调用效果检查。

`equals`入口继续只建立member/derived候选，不收集extension；其他operator先搜索member分区，再逐extension作用域搜索。operator调用不建立property-like候选，避免约定经间接invoke满足。

### 2.3 core operator迁移

M18删除`expr/operators.rs`中“先看Int/UInt/Boolean/Ptr/Array类型再决定语法可用性”的能力判断。core源码至少显式声明：

- Int/UInt的合法一元、算术、余数与`compareTo`，Boolean的`not`；
- String的`plus`以及进入当前语言子集的比较/下标operator；
- `Array<T>.get`、`MutableArray<T>.get/set`；
- `Ptr<T>.plus/minus`作为真正operator成员；
- 已有primitive、String与Ptr的equals继续使用普通member operator。

表示级运算仍可使用`@Intrinsic("int_add")`等封闭typed intrinsic。顺序固定为“core声明验证 → 普通operator overload winner → intrinsic正规化”。专用HIR节点携带`PrimitiveUnaryKind`、`PrimitiveBinaryKind`、`ArrayAccessKind`或`PointerIntrinsic`以及完整concrete type，不能保留source函数名。缺少合法core声明时core contract整体失败，编译器不能自行补候选。

### 2.4 下标与多参数

AST把单个`index`改为非空`indices`列表，read与assignment共用相同的索引语法事实：

```text
IndexExpr { receiver, indices: NonEmptyVec<Expr>, span }
IndexTarget { receiver, indices: NonEmptyVec<Expr>, span }
```

read把全部index作为位置source input交给`get`；write把index映射到`set`的前缀参数，右值强制映射到最后一个非`vararg`写入参数。前缀索引参数仍可按M17使用default/`vararg`，右值不能被`vararg`吞掉。Array/MutableArray winner可正规化为现有typed Index节点；用户类型保持普通method/extension call。

### 2.5 typed place与更新

parser把可赋值源码形态保存在`PlaceExpr::{Name, Field, Index}`，不把任意Expr留给HIR事后猜测。HIR建立仅在lowering内部存在的计划：

```text
ResolvedPlacePlan =
    Direct { materialization, read, write_capability }
  | Indexed { receiver_temp, index_temps, get, set_capability }

WriteCapability =
    ReadOnly
  | DirectWrite(ResolvedTarget)
  | OperatorSet(ResolvedCall)
```

该计划证明并缓存：

- receiver/index的typed temporary及固定源码求值顺序；
- read结果类型；
- 是否能直接写回，或下标set的唯一typed目标；
- 读写所需的boxing/adaptation以及异常/挂起效果。

prefix/postfix update要求计划可读写：先读旧值，解析并调用`inc`/`dec`，再适配写回。prefix结果是新值，postfix结果是旧值。正规化结果是temporary、普通call与assignment；`LocalConcreteHir`不保留Update节点。

对`lhs op= rhs`，以同一个place read type和同一个尚未提交的rhs source同时探测`opAssign`与`op`：

- 只有`opAssign`适用：调用并要求返回`Unit`；不写回，所以只读`val`也合法；
- 只有`op`适用：要求write capability，且结果可赋给place，再执行写回；
- 两组都有适用winner：报告歧义；
- 两组都无winner：合并显示两组结构化失败。

receiver和index先于rhs求值，且各一次；rhs即使参与两个候选组的类型检查，也只在winner commit后lower一次。`a[i] += rhs`的get与set共享同一组receiver/index temporary，不能重新执行getter、index表达式或property lookup。

### 2.6 保留的内建操作

- `&&`/`||`直接生成control-flow短路，不查找`and`/`or`；
- `===`/`!==`直接生成reference identity比较；
- `==`/`!=`复用M14的member equals与value conditional derivation；
- `compareTo`返回的exact Int与0比较使用已验证的primitive typed op，不再次进行用户overload；
- `in`/`!in`的外层取反使用exact Boolean typed op；
- assignment、Option运算、type test/cast不可重载。

这些内部后处理只消费operator contract已经保证的exact core type，不构成新的名称决议入口。

## 3. property-like `invoke`

### 3.1 三种调用入口

HIR明确区分：

1. `Type::Function`值：`CallableValueCall`，由函数类型直接给出参数与结果；
2. nominal值或其他非函数表达式：建立一次`Invoke` operator member/extension候选；
3. `name(args)`或`receiver.name(args)`：function-like与“property read + invoke”按c-level分区竞争。

函数值入口具有最高结构优先级，不与extension `operator invoke`歧义；它仍只接受函数类型元数规定的精确位置实参。显式`f.invoke(args)`在`f`为函数类型时正规化到同一入口。`FunPtr`在两种写法下都诊断为不可调用。

### 3.2 两段候选与c-level

property-like候选是结构化组合，不是伪造函数：

```text
PropertyLikeCandidate {
    property: ResolvedPropertyRead,
    invoke: CallableCandidate,
    priority: CallableLevel,
}
```

先在对应scope/receiver层选择property identity与result type，再以该type收集一次invoke。组合优先级取两部分中的较低级别。显式receiver的最终分区严格是：

1. member function-like；
2. member property + member invoke；
3. 当前extension scope的extension function-like；
4. member property + extension invoke；
5. extension property + member invoke（M21增加来源）；
6. extension property + extension invoke（M21增加来源）。

每个scope层完成上述c-level后才进入下一层。M16的“第一个含适用候选的最终分区”继续有效：仅存在同名但property读取或invoke不适用的组合，不会截断下一分区。

最近词法scope中的callable-value binding保留M11的硬遮蔽：Function类型或静态类型至少暴露一个可见invoke operator的binding先成为唯一local property-like层；若实参不适用，直接针对该binding报错。不可调用的普通binding不参与callable候选，也不遮蔽同名函数调用。

### 3.3 参数与求值

property-like调用的type arguments、全部`CallArgument`与尾随lambda原样交给invoke的M17 mapper；property不能消费这些参数。实际顺序是：

1. 外部显式receiver（若有）；
2. property read/getter，得到invoke receiver；
3. invoke的显式实参按源码顺序；
4. invoke的`vararg`/default按参数声明顺序物化；
5. invoke body。

即使当前field read没有副作用，也必须固定此顺序，为M21计算属性复用。property result只物化一次；suspend invoke跨挂起时按普通call receiver保活。失败候选的property getter和argument都不产生运行期求值。

### 3.4 callable reference边界

`::name`只引用函数声明，不应用invoke convention，也不把property变成函数引用。要保存property-like值，应直接读取property；要保存绑定调用行为，应显式写lambda。M21若增加property reference，需要独立语法与类型，不得复用本设计的call candidate组合。

## 4. `?.method()`

### 4.1 AST与payload解析

field与method导航共用封闭枚举：

```text
Navigation = Direct | Safe
```

parser不再在看到`?.name(`时诊断未支持，而是产生`MethodCall { navigation: Safe, ... }`。连续postfix保持左结合：`a?.f().g()`等同于`(a?.f()).g()`，后一个`.`在`Option<R>`上解析；要继续穿透必须写`a?.f()?.g()`。

HIR要求safe receiver精确为`Option<T>` application。它先把receiver物化为temporary，再只在`Some(payload)`分支内以payload静态类型执行完整调用决议。普通member/extension、property-like invoke、explicit type arguments、default/`vararg`与suspend效果全部沿用已有入口。

### 4.2 分支求值与结果类型

概念计划是：

```text
receiver_tmp = evaluate(receiver)
when (receiver_tmp) {
    Some(payload) -> Some(payload.method(arguments...))
    None -> None
}
```

HIR不能先lower argument再把结果搬入分支；每个argument自己的sink、default expansion、`vararg` assembly以及property read必须从一开始就在`Some`分支中生成。`None`分支只构造typed None，不触发callee effect。

结果类型无条件为`Option<R>`：`R = Option<X>`得到`Option<Option<X>>`，`R = Unit`得到`Option<Unit>`。SomeWrap与NoneLiteral都直接携带完整application TypeId；不能用“nullable flatten”flag或后续expected type猜测层数。

### 4.3 suspend、异常与control-flow

- `Some`分支内的suspend call按M10保存receiver/payload、已求值参数和Option结果continuation状态；恢复不能重跑receiver或实参；
- receiver求值抛异常时不进入when；`Some`分支argument/default/callee抛异常时不构造Some；`None`分支不会产生这些异常；
- safe call位于`while`条件时，receiver与分支计划在每次condition setup中重跑，不能提升到循环外；
- 嵌套default中的safe call保留M17 definition/evaluation origin；desugared temporary与branch也具有完整origin；
- moving stress下跨allocation/suspend存活的Option payload、invoke receiver与argument temporary进入普通精确root计划。

## 5. IR与stage边界

### 5.1 AST/parser

M18的AST改动集中为封闭结构：

- `FunctionDecl`增加`infix: Option<InfixModifier>`，既有operator modifier保留独立span；
- `Expr::InfixCall`以及prefix/postfix Update；
- Binary/Unary op扩为完整source token角色，assignment增加`CompoundAssignOp`；
- `MethodCall.navigation`，Index/IndexTarget使用非空indices；
- 统一`PlaceExpr`供assignment/update，不接受任意expression当左值。

lexer补`%`, `++`, `--`, `+=`, `-=`, `*=`, `/=`, `%=`, `..<`, `in`, `!in`, `infix`。最长匹配先处理`..<`/`..`、`++`/`+=`/`+`等冲突；`in`在for header与普通表达式中的语法角色由parser节点区分。

### 5.2 ExportHir

所有function kind共享callable-level metadata：

```text
CallableModifiers {
    operator: Option<OperatorKind>,
    is_infix: bool,
}
```

该结构用于top-level extension、member、local/export generic callable，不能只放在class Method中。`OperatorKind`及`is_infix`属于`.slib`接口metadata；新增、删除或改变角色会使依赖Cone失效。generic exported operator的角色不依赖某次实例化，参数/结果约束必须在template signature阶段得到证明。

class解构需要导出`Component { index }`角色，consumer直接按typed index建立候选；不能扫描名称。`iterator`同样导出角色，但M18不导出尚未存在的range/loop专用协议。

### 5.3 HIR lowering内部计划

以下结构只存在于`hir-lower`，不属于IR crate：

- `CallableLevel`与property/invoke组合候选；
- `ResolvedPlacePlan`与compound/update的两组probe；
- safe-call branch-local `OptionMapPlan`；
- operator source token到`OperatorKind`的映射。

这些计划可以引用AST span用于诊断，但winner commit后必须完全消费。失败session不得分配永久FunctionApplication、Default实例、property getter call或temporary；winner仍原子地产生唯一实体闭包。

### 5.4 LocalConcreteHir

本里程碑不改变函数类型或call argument的结构。普通用户operator/invoke正规化为现有MethodCall/Call；primitive/array/pointer winner可成为相应typed intrinsic expression；update与safe call成为temporary、assignment与control-flow。

`LocalConcreteHir`结构上不允许出现：

- source operator/infix/update/compound token；
- property-like组合候选或尚未执行的property read；
- safe navigation节点或尚未lower的argument sink；
- 未确定operator kind、name-based intrinsic或inference variable；
- `ResolvedPlacePlan`与`OptionMapPlan`。

### 5.5 MIR/LIR/codegen/runtime

- operator/infix/property-like/safe-call不增加MIR source语义；MIR只lower普通call、typed primitive/array/pointer op、branch与assignment；
- 所有新增temporary按既有异常边、suspend frame与GC live-set规则传播，不建立operator专用root计划；
- LIR call target继续引用完整typed signature；codegen不能通过函数名或symbol重新判断operator/invoke；
- core primitive operator intrinsic扩展现有typed intrinsic表，并在既定stage展开；
- runtime只提供String/异常等真正需要的后备函数，不参与operator或invoke lookup，因此M18不修改runtime查找协议。

## 6. 诊断与恢复

至少提供以下稳定诊断：

- `operator`用于无receiver函数、未知角色、错误参数数目/返回类型、非法`vararg` set value、`component0`或溢出索引；
- `infix`用于无receiver、参数非1、default/`vararg`参数，或override flag不匹配；
- operator表达式无匹配/歧义时显示展开角色、候选完整签名以及argument/bound失败；
- compound assignment同时存在`opAssign`与`op` winner、fallback不可写/返回不可赋值，或update operand不是place；
- get/set元数、index类型与多index失败，且右值失败明确标记set value位置；
- 非函数值没有invoke、invoke overload歧义、local callable-value硬遮蔽后的参数失败，或递归invoke被禁止；
- safe call receiver不是Option、`Some`分支方法无匹配，或链式`.`错误静态落在Option上。

parser在modifier、infix rhs、index逗号/`]`、operator token与下一statement边界恢复。存在诊断时整体丢弃残缺AST，不让占位节点进入HIR。

## 7. 测试计划

### 7.1 parser/AST

- 全部operator token、prefix/postfix、compound、`..<`/`..`最长匹配；
- infix与算术/range/Elvis/is/in/comparison/equality/boolean的无括号precedence golden；
- member/extension的operator与infix modifier排列；
- safe method、property-like member call、multi-index read/write与合法PlaceExpr；
- 每种malformed形态及多错误恢复。

### 7.2 operator与place

- 每个`OperatorKind`至少一个positive和声明negative，以及member/extension/generic/suspend组合；
- member优先于extension、角色filter排除普通同名函数、single candidate仍走完整applicability；
- `in`断言receiver先求值的右到左顺序，其余operator断言receiver/operand顺序；
- get/set多index、default/`vararg` index、Array/MutableArray typed intrinsic winner；
- prefix/postfix结果区别，local/global/field/index place，receiver/index/rhs各一次；
- `opAssign` only、`op` fallback、二者歧义、val receiver与不可写fallback；
- class `componentN`解构只求值subject一次；struct/tuple仍内建；iterator只验证声明角色；
- core Int/UInt/Boolean/String/Array/Ptr经普通operator声明选择，移除声明会触发core contract失败。

### 7.3 property-like/infix

- arbitrary expression、local/parameter/capture/global/current member/explicit field invoke；
- function value优先且extension invoke不参与，FunPtr两种调用写法都拒绝；
- c-level六分区中M18可实现的前四项，M21预留项用纯candidate unit test锁定插入点；
- local callable binding硬遮蔽，不可调用binding不遮蔽；
- type arguments、named/default/`vararg`/spread/trailing lambda全部转发给invoke；
- property read → arguments → defaults → body顺序，property result只求值一次；
- `operator infix invoke`、普通infix member/extension、左结合与suspend组合；
- invoke返回invoke对象时不自动二次调用。

### 7.4 safe method与端到端组合

- Some/None方法、extension、generic、property-like invoke以及function-type explicit invoke；
- None分支不执行property read、explicit argument、default、`vararg` assembly或callee；
- `R`、`Option<R>`、`Unit`分别得到`Option<R>`、`Option<Option<R>>`、`Option<Unit>`；
- 连续`.`/`?.`链的静态类型与错误位置；
- `while` condition setup每轮执行、nested default origin与异常提前退出；
- suspend Some路径同步完成/真实挂起/恢复，None路径不挂起；
- moving stress下receiver、property value、`vararg`与payload经allocation/call/invoke仍正确。

### 7.5 golden与回归

AST/HIR/LocalConcreteHir/MIR/LIR golden分别锁定：

- typed `OperatorKind`而不是名称；
- property-like最终变成普通call，safe call最终变成branch，update最终变成temporary/write；
- core intrinsic只在winner后出现typed kind；
- `LocalConcreteHir`不存在source operator、property-like候选、place plan或safe-call plan。

新增fixture分为`tests/fixtures/m18-operators/`、`m18-invoke/`和`m18-safe-call/`。每个目录同时包含独立规则fixture以及generic + default + closure + suspend + exception + GC组合fixture。M1至M17全部回归。

## 8. 实现顺序与提交门

1. 同步语言/实现spec与core contract边界，加入完整`OperatorKind`及callable modifier metadata；
2. 完成operator/infix token、precedence、multi-index、place与safe method AST，补齐parser恢复；
3. 实现property-like invoke的c-level分区、函数值边界与infix resolution；
4. 实现operator声明验证、普通resolver入口及全部表达式映射；
5. 实现typed place、update/compound与multi-index get/set；
6. 实现`?.method()`的branch-local lowering以及suspend/异常/while/default origin组合；
7. 迁移core primitive/String/Array/Ptr operator，删除旧type-based capability旁路；
8. 补齐全量golden、fixture、moving stress及M1至M17回归。

每个编号都是可独立提交的功能切片。完成后先执行`cargo fmt --all`与`cargo clippy --workspace`，再运行对应crate测试、stage golden和fixture；全部通过并单独提交后才进入下一切片。每个提交都必须保持workspace可构建、既有fixture通过，不能保留双轨resolver、临时占位分支或等待后续提交修复的IR缺口。

M18仅在以下条件同时满足时完成：所有operator/infix/property-like调用进入M16/M17同一resolver；safe method与place严格只求值一次；core operator能力来自普通声明；`LocalConcreteHir`/MIR完全看不到source operator、property-like候选、place或safe-call plan。仅让parser接受语法、只支持primitive运算、按函数名识别invoke/operator、在MIR重放AST或展平Option结果，都不算完成。

## 9. 明确不做

1. context parameters：已从M18撤出，待按独立方案重新设计；本设计不规定其语法细节、函数类型身份、solver输入、closure/FFI边界或ABI；
2. M19的secondary constructor、`init`、body property与`super`初始化；
3. M20的use-site/star projection、capture conversion与partial type argument；
4. M21的普通/extension property声明、object/companion、property delegate及`provideDelegate/getValue/setValue`协议；本里程碑只消费已有field/global/value；
5. M22的`for`/`break`/`continue`实现、IntRange等core range类型与`until/downTo/step`库；M18只交付range operator表面和typed iterator角色；
6. M23的跨Cone读取与import层实现；M18只定义必须导出的operator/callable metadata；
7. receiver function type、SAM conversion、builder inference、implicit conversion或多阶段自动invoke；
8. C varargs以及operator内联、temporary消除等优化；未优化基线先保证identity、求值、异常、挂起与GC正确。
