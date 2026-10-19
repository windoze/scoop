# M55 设计：泛型函数约束与成员调用要求

状态：实施中。更新：2026-10-19。M55-1 负向接口条件与条件重载、M55-2 companion 投影与 JSON 默认入口已完成本机验证并同步规范；M55-3 成员要求与结构断言也已完成本机验证；M55-4 组合与 Linux 完整验收尚待完成，实际进度见 [PROGRESS.md](PROGRESS.md)。第 10 节保留初步审查问题、待定边界与现有设计的改进机会，未完成部分仍是目标。

现有基础：[语言规范](../specs/SCOOP-SPEC.md)第 3.2、8.6、9.1.3、9.5、11.11、11.13、12.4、13.2～13.3、13.9 节、[实现规范](../specs/SCOOP-IMPL-SPEC.md)第 2.11、2.13 节、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)第 1、2.7 节与[路线图](../ROADMAP.md)。已有设计见 [M14](../milestone14/DESIGN.md)、[M20](../milestone20/DESIGN.md)、[M29](../milestone29/DESIGN.md)与 [M41](../milestone41/DESIGN.md)。这些文档尚未纳入 M55 的全部变更；实施前须按第 10.1 节同步权威合同，不能把本设计目标当作既有行为。

## 0. 范围与决定

M55 扩展已有的泛型约束系统。普通 `where`、正向 class／interface bound 和 `value`／`ref` 已存在，本期增加以下能力：

1. `where T.Companion : ValueEncoder<T>`：约束实际类型的 companion，并允许泛型正文通过该契约调用它。
2. `where T : !Debug`：表达静态类型不满足某个完整 interface bound，参与普通泛型函数重载。
3. `where requires(x: T) { x.debug(): String }`：要求类型自身提供指定的公开成员调用，也支持公开 companion 成员与公开构造器。
4. 同形泛型函数可以由不同条件集合区分。先判断适用性，再使用既有最具体候选规则；泛型正文在定义处选定的普通重载不在单态化时重选。
5. 成员要求使用统一的公开成员表面，不接收调用点的自由函数、扩展、import 或访问权限。同一函数声明和完整类型实参必须得到同一套调用绑定。

新条件只用于有自身类型参数的顶层、局部、扩展函数和 non-interface final 成员函数。宿主／外层参数可被约束中的类型引用，但不能由内层函数的 `where` 追加约束。普通非泛型成员、构造器声明、名义类型、typealias 和 extension property 的声明约束不扩展；构造器可以成为要求的目标。

本期不增加一般 associated type、负向 interface 实现声明、类模板偏特化、条件 conformance、任意表达式要求或运行期约束分派。需要调用点定制的实现时，使用普通显式函数参数或 codec 参数。

## 1. 语法与条件含义

### 1.1 声明表面

```scoop
fun <T> encode(value: T, output: Encoder): Unit
    where T.Companion : ValueEncoder<T> =
    T.Companion.encode(value, output)

fun <T> render(value: T): String
    where T : !Debug,
          requires(x: T) { x.describe(): String } =
    value.describe()

fun <T> construct(text: String): T
    where requires { T(input: String) } =
    T(text)

fun <T> staticText(): String
    where requires { T.someStaticMethod(): String } =
    T.someStaticMethod()
```

这里使用 core 已有的 `ValueEncoder<T>` 与非泛型 `Encoder`，不另外引入名为 `Encoder<T>` 的接口。`Debug` 是本设计示例中的普通用户接口。

语法骨架如下；命名／spread 规则、方法类型实参与类型引用沿用现有规则，实参位置的类型标注仅属于要求语法：

```ebnf
WhereClause      = "where" Constraint { "," Constraint } ;
Constraint       = Subject ":" Bound | MemberRequirements ;
Subject          = TypeParameter [ "." "Companion" ] ;
Bound            = "value" | "ref" | Type | "!" Type ;
MemberRequirements = "requires" [ "(" [ VirtualParameters ] ")" ]
                     "{" Requirement { Separator Requirement } [ Separator ] "}" ;
VirtualParameters = Identifier ":" Type { "," Identifier ":" Type } ;
Requirement      = [ EffectModifiers ]
                   ( RequiredMethodCall ":" Type | RequiredConstruction [ ":" Type ] ) ;
RequiredMethodCall = Identifier "." MethodName [ TypeArguments ] "(" [ Arguments ] ")"
                | TypeParameter "." "Companion" "." MethodName
                  [ TypeArguments ] "(" [ Arguments ] ")" ;
RequiredConstruction = TypeParameter "(" [ Arguments ] ")" ;
Arguments        = Argument { "," Argument } ;
Argument         = [ Identifier "=" ] [ "*" ]
                   ( Identifier | Identifier ":" Type ) ;
Separator        = Newline | ";" ;
```

`requires` 仅在 `where` 条目起始处且后接 `(` 或 `{` 时作为上下文关键字；普通同名函数、变量，以及 `where requires : I` 中的同名类型参数不受影响。没有虚拟参数表时，`requires { ... }` 与 `requires() { ... }` 等价。`EffectModifiers` 只允许第 3.3 节的既有效果标记。要求块必须非空，不是表达式 block；方法要求的结果类型必填，构造要求的结果固定为 `T`，可省略 `: T`，显式标注时必须为该类型。

所有条目取合取，顺序不影响条件含义。负向 bound 也可写成 `fun <T : !Debug> ...`，与 `where T : !Debug` 等价；只允许半角 `!`，`!Debug` 不是普通类型。没有 `or`、一般 Boolean 条件或 `!requires`。

### 1.2 Companion 投影

`T.Companion` 是既有 companion 的依赖投影。它要求实际静态类型直接拥有 public companion，不能用基类的 companion 代替。投影类型和值使用这个拼写；调用位置可写 `T.method(...)`，固定归一化为 `T.Companion.method(...)`。不支持依赖的 `T.Factory` 或任意 `T.Nested`。

| 条件或使用 | 含义 |
| --- | --- |
| `T.Companion : I<T>` | 存在 public companion，并满足完整接口上界 |
| `T.Companion : Base<T>` | 存在 public companion，并满足普通 class 上界 |
| `T.Companion : ref` | 只要求存在 public companion |
| `T.Companion : !I<T>` | 存在 public companion，且不满足该接口上界 |
| `T.Companion : value` | companion 是引用 singleton，声明处拒绝矛盾条件 |
| 要求 `T.make(...)`／`T.Companion.make(...)`，或 receiver 类型为 `T.Companion` | 同时建立 public companion 的存在条件 |
| 仅在参数／结果／类型实参位置写 `T.Companion` | 不隐式增加存在条件；需有上述条件支持 |

每个主体独立应用正向 bound 规则：同一主体至多一个 class 上界，可有多个 interface 上界；kind 与正向名义上界仍互斥。`T : value` 可以与 `T.Companion : I<T>` 组合，负向及成员要求也可以与 kind bound 组合。

投影可用于已受约束函数的签名、默认值和正文。先登记整个声明的参数及 companion 存在条件，再解析引用，不让书写顺序影响合法性。投影不反向推断宿主；例如只提供 `T.Companion` 实参且无法从其他信息确定 `T` 时，调用方须显式写 `f<User>(...)`。

实际类型确定后，投影替换为已有的真实 companion application。命名 companion 的别名、透明 typealias、不同泛型宿主实参的独立身份，以及同一实例跨 Cone 共享 singleton，均沿用现有规则。条件查询不执行初始化；只有实际求值的 `T.Companion` 访问调用已有 ensure。M55 不自动合成 companion，也不改变显式 codec API 或既有合成规则。

### 1.3 负向接口条件

首版负向条件只接受完整 interface application，例如 `T : !Debug`、`T : !Equals<T>`。不支持排除 class、exact value type、另一类型参数或 kind；`!Any`、`!T`、`!value`、`!ref` 均非法。

判定采用三种结果，不能把已有 `is_subtype` 的 `false` 直接取反：

| 已知事实 | `T : !Debug` 的结果 | 能否使用该候选 |
| --- | --- | --- |
| 静态实参已确定，按完整接口关系不满足 `Debug` | 成立 | 可以 |
| 静态实参已确定且满足 `Debug`，或环境有 `T : Debug` | 不成立 | 不可以 |
| 环境已有同一负向事实 | 成立 | 可以 |
| `T` 仍未知，环境没有可推出的负向事实 | 未知 | 不可以 |

判断基于静态类型与完整接口继承关系。`Base` 不满足 `Debug` 可以让 `foo<Base>(value)` 使用负向候选，即使 `value` 的运行期派生类型满足它；该条件不提供运行期类型排除、smart cast 或禁止以后增加接口实现的保证。`Nothing` 依既有底类型规则满足正向上界，不能用“没有普通成员”推导其满足负向条件。

部分 application 同样可能未知：`Box<U>` 实现 `I<U>` 时，未知 `U` 不能让 `Box<U> : !I<String>` 成立。可以复用名义继承的确定关系，例如已知 `J : I` 时，`T : !I` 可推出 `T : !J`，反向不成立；仅因一个 class 上界没列出接口，不能断言其未知子类型也没有接口。

负向条件不产生类型推断的猜测值。普通参数、receiver、显式类型实参、允许的期望类型和正向 bound 先完成原有推断，不能为通过 `!Debug` 改选 `Any` 或换掉已经确定的实参。重复完整条件、直接正负矛盾及可由普通继承确定的矛盾在声明处报错。

### 1.4 约束组合

**各类约束可以组合使用，companion 投影与负向接口条件也可以直接组合。** 投影决定被约束的主体，`!` 决定接口条件的方向；因此下面的声明合法：

```scoop
fun <T> withoutEncoder(value: T): String
    where T.Companion : !ValueEncoder<T> = "<unencoded>"
```

该条件表示实际 `T` 存在 public companion，且该 companion 不满足 `ValueEncoder<T>`。没有 public companion 时条件不成立；companion 的接口关系仍未知时，也不能使用该候选。

同一个 `where` 还可以混合对不同主体的条件、同一主体的正负接口条件，以及成员调用要求。例如，要求一个值类型能够解码、companion 不提供编码接口，并且值本身具有 `describe` 方法：

```scoop
fun <T> decodeAndDescribe(decoder: Decoder): String
    where T : value,
          T.Companion : ValueDecoder<T>,
          T.Companion : !ValueEncoder<T>,
          requires(x: T) { x.describe(): String } =
    T.Companion.decode(decoder).describe()
```

各项按合取共同成立，顺序不影响含义。`T` 与 `T.Companion` 的条件独立判断；同一主体的正负条件可以针对不同接口，若同时要求 `ValueEncoder<T>` 和 `!ValueEncoder<T>` 则是矛盾。组合继续遵守已有 kind／正向名义 bound 互斥、接口继承和三态判定规则。

## 2. 条件重载与定义处绑定

以下代码固定本期的核心行为：

```scoop
interface Debug { fun debug(): String }

fun <T> foo(value: T): String = "general"
fun <T> foo(value: T): String where T : !Debug = "opaque"

fun <T> bar(value: T): String = foo(value)
fun <T> baz(value: T): String where T : !Debug = foo(value)

fun <T> foo1(value: T): String where T : !Debug = "opaque"
fun <T> good(value: T): String where T : !Debug = foo1(value)
```

`bar` 选择第一个 `foo`，`baz` 选择第二个，`good` 合法。下面的声明在定义处报无匹配错误，说明无法建立所需的负向事实：

```scoop
fun <T> bad(value: T): String = foo1(value)
```

即使以后实例化为 `bar<Plain>`，且 `Plain` 不满足 `Debug`，内部仍调用第一个版本。直接以确定的 `Plain` 调用 `foo` 则可以选第二个版本。带 `T : Debug` 的调用方不能选择负向版本。编译器不自动给调用方追加 bound，也不把失败留到单态化时再重选。

候选按现有词法／成员／import 层级收集，在层内完成普通推断与条件检查，再做最具体候选比较。某一层没有适用候选时，继续遵守现有下一层查找规则。

最具体比较继续使用声明间转发：源声明参数作为 rigid skolem，源条件成为已知事实，尝试满足目标声明的参数和条件。对相同参数形状，负向版能转发给无约束版，反向不能，因此负向版更具体。不是给所有带 `!` 的函数增加固定权重，也不按条件条数排序。

两个互不相关的负向重载若同时适用，而原参数规则不能区分，结果仍为歧义。正向上界沿用现有继承推导；负向只增加相同条件与直接名义关系；成员要求只转发替换后相同的完整契约，不增加一般逻辑蕴含、成员参数逆变或结果协变的条件排序。

## 3. 成员调用要求的封闭范围

### 3.1 为什么统一使用公开成员

若 `requires` 查询调用点可见的自由函数或扩展，两个消费方可以给同一 `T` 添加不同重载，导致同一个 `render<T>` 绑定不同实现。这会破坏“函数声明 + 完整类型实参”确定泛型实例的规则。

只冻结泛型定义处的访问域仍留下转发问题：同样的 `x.format(text): String`，一个 helper 只看到 public 重载，另一个 helper 还看到更具体的 private 重载；后者甚至可能因所选重载返回类型不同而不满足要求。因此本期统一查询 public 成员，而不是按每个 helper 的访问权限构造候选集。

固定查询范围包括真实成员、继承成员、接口成员和合法的公开构造器。private／protected／internal 成员不参与，即使声明方或调用方有权普通调用它们。companion 本身也必须 public。这里不要求实际数据类型自身为 public：隐藏具体类型仍可经 `T` 抽象传入并提供公开操作。

筛选使用成员／companion／构造器自身的声明可见性。实际类型已经有 typed identity，不通过 import 重新寻找其名称，也不将私有宿主的有效访问域当作成员自身的 private 标记。

要求中出现的类型名称、interface、typealias 和注解身份在定义处按普通规则解析并绑定。普通源码中的自由函数、扩展、protected 访问与显式已知类型的私有 companion 访问保持现有规则，只是不成为这类可转发的要求。

### 3.2 允许的调用形态

| 形态 | 例子 | 检查内容 |
| --- | --- | --- |
| 实例方法 | `x.debug(): String` | `x` 的声明类型为本函数参数 `T` 或其 companion 投影 |
| companion 方法 | `T.someStaticMethod(): String`、`T.Companion.parse(s): T` | 两种限定形式都只查询 public companion 与其中的 public 成员 |
| 构造 | `T(arg: String)`、`T(s): T` | class／struct 的合法 public 普通构造入口，结果固定为 `T` |

`T.someStaticMethod()` 中的 `T` 必须绑定到本函数的类型形参；它表示 companion 调用，不表示实例方法，也不展开为任意静态命名空间查找。要求与普通泛型正文都先把它归一化为 `T.Companion.someStaticMethod()`，再使用完整契约。实际类型的同名 nested type、属性或消费方扩展不能改变这个已经绑定的目标种类；显式已知类型的普通 `Host.member` 继续按原规则查找。

虚拟参数只有名称和完整类型，无默认值、`vararg`、存储、求值和捕获。调用实参可以引用参数表中的虚拟参数，也可以在当前位置直接写 `arg: String`，表示一个静态类型为 `String` 的虚拟实参。内联名称只是当前位置的说明，不要求 callee 形参同名，也不建立可在别处引用的 binding；改名不改变条件。它们都不用于猜测外层函数尚未确定的类型实参。

需要检查命名调用时使用 `=`，如 `T(text = arg: String)`；需要 spread 时使用 `*`，如 `T(*items: Array<String>)`，继续遵守普通参数映射规则。仅写 `T(arg: String)` 是一个位置实参的构造要求，按普通调用规则判断该 String 实参能否选出唯一构造器；正常赋值适配与其余默认参数仍可使用。类型标注仅属于要求语法，不进入普通运行期实参表达式。

以下写法归一到同一个构造要求：

```scoop
requires { T(arg: String) }
requires() { T(input: String): T }
requires(value: String) { T(value): T }
```

普通方法的结果仍须写出，如 `requires { T.someStaticMethod(): String }`；构造器的结果天然是所构造的 `T`。要求检查不执行方法或构造，也不初始化 companion。方法自身的完整类型实参列表、`_` 与普通方法实参推断规则继续适用。

```scoop
fun <T> tagged(value: T, text: String): String
    where requires(x: T, s: String) { x.format(prefix = s): String } =
    value.format(prefix = text)
```

实际方法可以使用默认参数或 `vararg`，只要所写调用形态适用。成员重载先按普通参数规则选出唯一目标，再检查结果可赋给声明的结果类型；结果类型不参与择优。目标方法需要自己的类型实参而仅凭结果才能确定时，要求中也必须显式提供。

构造要求不为 interface、abstract class、object、enum 整体或 intrinsic-only 类型制造新入口。要求块也不接受自由函数、扩展、函数值属性、getter／setter、运算符表达式、嵌套调用和任意执行语句。`x.plus(y)` 只描述一个方法调用，不单凭名称获得 `operator` 能力。

需要 outer 实例或运行期捕获值的构造不能满足要求；无捕获局部 class 可按公开构造入口满足。仅有成员要求不能形成 callable reference，使用普通 lambda 包装调用。具体边界见第 10.2.2～10.2.3 节。

### 3.3 调用效果

要求需要足够的信息让泛型正文完成正常类型与效果检查。默认要求为 safe、非 suspend、Managed 调用；每项调用前可使用既有效果标记：

| 标记 | 契约 |
| --- | --- |
| `suspend` | 目标同为 suspend，正文调用须处于正常挂起上下文 |
| `@NoGC` | 完整调用满足既有 NoGC 条件，包括所需默认值、vararg 和 singleton 访问 |
| `@Unsafe` | 允许 unsafe 目标，正文使用该契约需要正常 unsafe 上下文 |

未写 `@NoGC` 不对外承诺 NoGC，即使某个实际方法更严格；safe 要求不能由 unsafe 目标满足。这里只接受表中标记，不开放任意 annotation 或新 effect 系统。非法组合沿用普通 callable 规则，结果与效果失败都不使查询改选另一个成员重载。

```scoop
suspend fun <T> readAsync(value: T): String
    where requires(x: T) { suspend x.read(): String } =
    value.read()

@NoGC
fun <T : value> count(value: T): Int
    where requires(x: T) { @NoGC x.count(): Int } =
    value.count()
```

第二例仍遵守已有 generic NoGC 的 GC-free 条件传播，`value` 本身不保证 GC-free。companion 是引用 singleton，不能用 `@NoGC` 要求绕过已有 receiver 与初始化限制。成员的 Context、异常、默认参数和实际分派按现有合同执行，查询本身不执行这些行为。

### 3.4 正文绑定与转发

每项要求形成独立的类型化调用契约。正文中的调用使用契约的 receiver／参数类型、命名／spread 形态和效果，并把返回值视为声明的 `R`。真实表达式正常求值并适配到契约类型；虚拟变量不会进入正文或生成存储。

要求只承诺所写形态。例如要求了 `x.format(prefix = s)`，不自动承诺 `x.format(s, mode)`；另一种调用需独立要求或已有名义 bound。多个要求适用于正文同一调用时，按普通参数规则选择；仅结果或效果不同不能择优。

这里比较的是归一后的调用形态：`T.method` 与 `T.Companion.method` 可以互换，内联类型实参与参数表中的虚拟实参可以互换，构造结果 `: T` 的省略不产生区别。普通正文仍传实际表达式，例如要求 `T(arg: String)` 后使用 `T(text)`，不把要求中的类型标注当作运行期实参。

已有名义 bound 的适用成员优先于成员要求；没有适用的 bound 成员时才选择要求契约，然后再按原规则考虑作用域扩展。定义处保留选中的目标种类，具体化不能切换路径。仅在一个 interface 上界下能调用某个成员，也不自动满足一个结构调用要求：实际子类型可能增加更具体的重载，故转发要求只认相同完整契约。

保留名义 bound 优先：若它选中 Managed 成员，同形结构 NoGC 承诺不能改变该目标效果；可用只携带同一结构要求的普通 helper 调用。第 10.2.1 节给出组合边界。

```scoop
fun <T> describe(value: T): String
    where requires(x: T) { x.describe(): String } = value.describe()

fun <T> forward(value: T): String
    where requires(y: T) { y.describe(): String } = describe(value)
```

虚拟变量名不同不影响转发。无此要求的泛型函数不能把未知 `T` 交给 `describe`；静态具体类型则直接查询其公开成员。

实际类型确定后，按要求声明的实参类型选择真实目标一次，并保留参数映射、默认值／vararg 计划、结果适配和效果。正文恰好传入更窄的值不触发第二次重载选择。默认值始终来自选中成员原定义的已绑定模板，且只在真实调用执行时求值。

### 3.5 失败与递归

没有匹配公开成员、成员歧义、结果不兼容或效果不兼容，使外层候选的该项要求不成立。诊断保留具体原因。被调用声明自身的类型错误、非法递归、损坏产物或 I/O 错误不能被吞成候选不适用；不能通过尝试编译任意函数正文来判断要求。

本期禁止递归的要求判定：活动要求声明链再次进入同一要求声明即报错，即使本次类型实参不同。普通函数正文的递归不因此被禁止；条件检查与正文实例化是不同过程。

成员契约落实后新增的真实调用边须进入既有泛型递归 SCC 与实参替换检查。定义处暂不能确定的边在此时检查；不能借成员要求形成参数持续变化的递归实例，也不引入深度、实例数量或时间预算。

## 4. 声明身份与单态化一致性

### 4.1 同形声明的规范化

现有重复签名只比较类型参数数量、receiver 和值参数类型。本期将规范化条件纳入泛型函数的重复签名及持久化声明身份，并统一用于当前 Cone、局部声明、成员、import 和 re-export 冲突判断。

规范化遵循以下规则：

- 类型引用使用真实 typed identity；声明内参数按 binder 位置进行 alpha-normalization，保持宿主／外层和函数自身参数组的区别。
- inline 与 `where` 合并，typealias 完全展开，条件按规范形式组成集合。要求块拆成逐项契约，不保留块分组、未使用的虚拟参数、虚拟变量名或变量复用关系。
- 空参数表省略、`T.method`／`T.Companion.method`、内联类型实参／参数表引用、构造结果 `: T` 的省略均先归一，不形成额外条件身份或重载。
- 要求记录目标种类、receiver、方法名、显式类型实参形态、调用参数类型及顺序、命名／spread、结果和效果。未写实参与显式传入默认值不是同一形态。
- 源码位置与条件书写顺序不参与身份。参数名、函数返回类型、默认值或函数自身的 suspend 标记仍不能单独区分外层重载。
- 规范化后的相同条件重复出现时在声明处诊断；不同声明的规范化签名完全相同则仍是重复声明。不会为了合并声明求任意逻辑等价。

成员要求中的结果类型属于被要求的调用契约，因而参与该条件的身份；这不等于允许普通函数只按返回类型重载。仅结果不同的两个要求也不能用于正文的返回类型择优。

先按规范化条件值确定函数声明身份，再由该声明和规范化要求位置派生要求的 typed identity。父声明 key 不引用自己尚未产生的要求 ID，避免身份构造循环；要求不使用 function ID 或 interface slot ID 代替。

### 4.2 保持实例 key 的含义

实例仍由原函数声明、完整宿主／外层实例及完整函数类型实参确定。`CallableApplicationKey` 和既有 ODR 分组不添加调用点作用域、消费 Cone、选中方法的额外 specialization 参数或隐式 dictionary。

对一个未变化的程序与依赖集合，要求绑定可表示为：

```text
绑定结果 = resolve(已绑定的要求定义, 完整类型实参, 对应类型的公开成员表面)
```

公开成员表面由类型声明及继承关系决定，不由 consumer 的自由函数、扩展或访问权限决定。同一实例跨 Cone 得到同一绑定；已有 virtual／interface 调用仍保留正常运行期分派，ODR 一致性不要求把它们全部改为 direct call。

两个重载因声明条件不同而具有不同声明身份，这是用户明确选择的函数重载。不能让同一声明、同一类型实参在不同调用点产生不同正文，再由 linker 比较内容、增加 symbol 或选择其中一个来掩盖问题。

需要局部定制时，显式传入实现即可：

```scoop
fun <T> renderWith(value: T, debug: (T) -> String): String = debug(value)
```

这使用已有函数值语义，定制是普通值参数，不会改变 `renderWith<T>` 的实例身份。

## 5. 当前实现落点

以下是本次设计调研的代码入口，不表示已有代码具备新能力。

| 入口 | 现状 | M55 修改责任 |
| --- | --- | --- |
| [AST 声明](../../compiler/ast/src/declarations.rs)、[泛型 parser](../../compiler/parser/src/decl/generics.rs) | bound 只有 kind／正向类型，`where` 左侧只有参数名 | 区分条件主体、负向类型与专用要求语法，保留准确 span |
| [HIR 泛型声明](../../compiler/hir/src/declarations/generics.rs)、[约束解析](../../compiler/hir-lower/src/types/constraints.rs) | `TypeParamBounds` 表达 kind 与正向名义 bound 的互斥 | 保留原不变量，增加独立负向、companion 条件与成员契约 |
| [调用约束](../../compiler/hir-lower/src/call_resolution/constraints.rs)、[适用性](../../compiler/hir-lower/src/call_resolution/applicability.rs) | 候选独立推断并检查既有 bound | 新条件检查、未知状态、失败原因与普通推断衔接 |
| [声明转发](../../compiler/hir-lower/src/call_resolution/specificity.rs)、[MSC](../../compiler/hir-lower/src/call_resolution/specificity/selection.rs) | 源参数固定，目标 fresh，成对判断可转发 | 传入源条件事实，按同一规则检查目标条件 |
| [重复签名](../../compiler/hir-lower/src/declarations/callables/signature_comparison.rs)、[身份签名](../../compiler/identity/src/entity/signature.rs) | 参数形状参与判重，条件不参与 | 统一扩展后的规范化签名及依赖投影类型键 |
| [持久化函数](../../compiler/hir-lower/src/persistent_functions.rs)、[导出合并](../../compiler/hir-lower/src/persistent_export_bindings/merge.rs) | 生成声明 key 并合并普通 overload 表面 | 采用同一条件签名，避免本地合法而导入冲突 |
| [类型](../../compiler/hir/src/types.rs)、[类型替换](../../compiler/hir-lower/src/types/substitution.rs)、[companion 查询](../../compiler/hir-lower/src/types/qualified/applied.rs) | 参数类型存在，companion 路径要求宿主已知 | Export HIR 的依赖投影、存在性与实际 application 替换 |
| [成员查询](../../compiler/hir-lower/src/expr/members.rs)、[HIR 调用](../../compiler/hir/src/body.rs)、[成员具体化](../../compiler/hir-lower/src/concretize/methods.rs) | 普通成员、扩展与已有 bound callee 分开处理 | 公共固有成员探测、独立要求引用与真实调用落实 |
| [跨 Cone binder](../../compiler/hir/src/cross_cone_interface/binders.rs)、[callable key](../../compiler/identity/src/entity/callable.rs) | 保存既有 bound 与宿主／函数实例身份 | 扩展声明和模板数据，保持实例 key 的原含义 |
| [泛型递归](../../compiler/hir-lower/src/effects/generic_recursion.rs) | 在已知调用图中检查完整参数替换 | 补入要求落实后确定的调用边，拒绝非 identity 递归 |

不把 AST／HIR／identity crate 扩展为另一套类型检查器。规范化数据和格式属于 IR／identity；条件语义、调用选择和具体化属于 `hir-lower`。

## 6. 实现方案

### 6.1 AST、声明检查与完整 HIR

AST 的 `WhereClause` 保存封闭的条目种类：普通类型条件和成员要求。类型条件主体区分自身参数与其 companion；负向是独立 bound 种类。要求使用专用虚拟参数与调用结构，不通过普通表达式 AST 接受全部语法再在后续猜测合法子集。

parser 接受可省略的虚拟参数表和专用的 `name: Type` 实参形态；方法结果必须显式给出，构造结果可省略。HIR 声明边界将静态调用简写归一为 companion 目标，将内联类型实参归一为 typed 参数位置，并为构造要求补全确定的 `T` 结果；所有写法共用后续契约，不保留缺失类型或另一套短语法实现。

HIR 保留 `TypeParamBounds` 对 kind／正向名义 bound 的结构约束。普通参数正向 bound 继续只有一份权威数据；函数条件集合补充负向接口、companion 主体条件及成员要求，不能把负向塞进原互斥枚举后错误禁止 `T : value, T : !I`。

| 数据 | 必须完整记录的信息 |
| --- | --- |
| 负向条件 | typed 主体、完整 interface application、诊断位置 |
| companion 条件／投影 | 宿主参数身份、存在条件、该主体的完整 bound；投影与 nominal type ID 分开 |
| 成员要求 | 独立 typed ID、目标种类、receiver／参数／结果类型、名称与实参形态、效果、定义来源 |
| 正文要求调用 | 要求引用、全部实际表达式及到契约的适配、完整结果类型和调用效果 |
| 已落实要求 | 真实 typed callee／constructor、完整实参、分派种类、参数映射、默认值／vararg 与结果适配 |

未知 companion 不能先伪造一个 nominal ID。要求调用不能表示成 `Option<FunctionId>` 为空的普通调用，也不能与 interface slot 混用 ID。普通 bound 调用、要求调用和普通 callable 保持不同封闭分支，每个分支自身具备后续所需的数据。

声明检查先建立所有 binder 与允许的投影，再解析全部类型引用，检查主体范围、public 表面规则、重复／矛盾条件、效果组合和签名可见性。投影必须加入类型遍历、替换、签名比较、alias 展开、依赖与访问检查；仅支持 `T.Companion`，不顺手开放任意投影路径。

### 6.2 推断、适用性与 MSC

继续使用既有 candidate-local inference session，不另建通用逻辑 solver。普通等式、子类型、上下界与延期实参先按既有固定点传播；宿主确定后，companion 的正向上界可继续提供普通名义关系。负向与成员查询不为外层参数制造新猜测。

负向关系查询局部返回成立／不成立／未知及原因。对于参数，读取定义环境中的明确事实；对于部分名义 application，保留接口实参比较的未知结果；对于确定类型，复用完整继承与 conformance。不能把“不知道有无关系”折成确定不存在。

成员要求查询只消费真实类型的 public 成员、继承／接口及构造器视图，复用普通调用形态和重载决议。普通参数适用性、唯一目标选择、结果与效果检查按语言规定的顺序进行，不调用词法 extension／自由函数或 property-like 路径。

泛型转发时以条件契约检查条件契约：源事实支持目标则保留目标要求引用；尚未知道实际类型不是重新搜候选的理由。MSC 使用同一事实环境和规则，避免“调用适用”和“谁更具体”各有一份不一致的判定逻辑。

失败候选不提交 callable 实例、默认值展开、singleton 初始化或代码物化。若所有候选失败，诊断按候选保留缺少负向事实、缺少 public companion、缺少成员、歧义、结果或效果不兼容的具体原因。

### 6.3 具体化与终止性

Export HIR 保存已经绑定的普通目标及完整要求契约。具体化先替换完整宿主／函数参数，取得真实 companion 或公开成员表面，再落实每个要求的唯一目标。

同一未变化 application 的成功要求结果在当前 lowering 过程中复用。正文、默认值和局部 callable 直接使用其 typed 结果，不重新搜索，也不根据某个正文表达式更窄的类型改选方法。仅检查要求不生成未使用成员的机器代码，不执行默认值或 ensure。

递归条件检查只维护当前活动的要求声明依赖链，遇到重复声明即报告语言禁止的递归。成员落实后保留原 callable 身份与完整实参替换，将新增边接入现有递归检查；未知目标带入的递归环返回同一声明时必须保持完整 application，不能先无限展开再依赖深度限制停止。正常同实例递归复用已有实体。

进入 `LocalConcreteHir` 前消去投影、要求调用、类型参数、推断变量和未决条件，只留下正常 direct／virtual／interface 调用、构造与 singleton 访问。MIR、LIR、codegen 不参与源码约束查询或重载选择。

### 6.4 跨 Cone、格式与缓存

Export HIR 与跨 Cone binder 保存新条件、投影和要求引用；类型、接口、默认值及实际调用的依赖沿已有路径收集。泛型正文调用的目标必须在 producer 与 consumer 中一致，隐藏的模板支持也不能进入普通 public lookup。

实现时同步修改重复签名、持久化声明 key、类型签名、binder 与泛型正文编码，以及实际嵌入这些记录的 metadata。按实际改动升级相应格式／身份 schema，producer、consumer 和 import/re-export 同步切换，并重建旧产物与缓存。设计阶段不预占 wire tag，也不把当前 `persistent-v1` 写成已经升级。

reader 只在既有边界做格式与 typed 引用检查；语言条件在所属 lowering 阶段检查，不在 IR reader、linker 或 runtime 重放成员查询。新声明内容进入现有依赖与更新判断，不增加产物内容审计、来源认证、收据或并行 reader。

runtime ABI、TypeDescriptor、GC、初始化 cell／root 和实际调用约定保持。约束查询不触发初始化；真实调用的分派、装箱、默认值、异常、suspend 和 FFI 行为依既有规则生成。

## 7. 实施顺序

各批代码实施前先完成第 10.1 节的对应规范同步；涉及第 10.2 节的问题时先确定规则并补入本设计及权威规范。JSON 默认 codec 重载与结构相等断言作为首批实际使用验收，具体范围见第 10.5 节；其余库改进是后续评估候选，不要求一次性迁移全部标准库。

| 批次 | 工作 | 可验收出口 |
| --- | --- | --- |
| M55-1 负向与条件重载 | AST／HIR、三态关系、声明事实、MSC、统一条件签名及相应跨 Cone 编码 | 本文 `bar`／`baz`／`good`／`bad`，具体调用、重载歧义、import/re-export；正式 CLI 编译运行与错误定位 |
| M55-2 Companion bound | 固定投影、public 存在性、签名／默认值／正文替换、既有 singleton 复用 | `ValueEncoder<T>` 调用、命名 companion、泛型宿主多实例、缺少 companion 与初始化计数；JSON 默认 codec 重载 |
| M55-3 成员要求 | 专用语法、完整要求引用、公开成员／构造器探测、调用形态与效果、正文落实 | 未实现接口但有成员的类型、构造、默认值／vararg、suspend／NoGC／unsafe、同契约转发；结构相等断言与原名义入口兼容 |
| M55-4 组合与产物收尾 | 局部函数／泛型方法、默认模板、递归边、依赖消费者、格式拒绝路径与完整回归 | 相同实例跨 Cone 行为一致；两组实际使用用例跨 Cone 消费；旧格式明确拒绝；正式工具编译、链接、运行并更新交付记录 |

每批完成对应语义的输入到运行闭环及必要格式迁移，不能以 parser 接受语法或某一阶段的快照替代交付。已决定排除的语法给出明确错误，不留占位实现。发现三份规范之间的冲突，先修订到同一合同再实施。

## 8. 验收与诊断矩阵

每个新增语言错误均有独立 negative fixture，检查错误位置与关键原因；成功用例以正式源码的诊断和运行结果为主。实现时在既有 fixture runner 下建立 M55 用例，不增加另一套 runner。

| 范围 | 独立用例与错误规则 | 必须覆盖的组合 |
| --- | --- | --- |
| 语法与声明范围 | inline／where 等价、逗号与多项要求、同名 `requires` 参数兼容；全角 `！`、一般否定类型、空块、自由表达式、非法声明位置拒绝 | 局部／扩展／final 泛型方法；自身与宿主参数分组，outer 参数不得被新增约束 |
| 要求简写 | `requires { T.someStaticMethod(): String }` 与 `requires { T(arg: String) }`；省略空参数表、构造结果；缺少方法结果、错误构造结果、引用未声明虚拟值拒绝 | 与长写法规范化相同、互相转发、重复签名；内联实参改名不要求 callee 形参改名；命名／spread；类型形参静态简写不落入同名 nested type／属性／扩展 |
| 负向真值 | 具体满足／不满足、未知不能作否定、`Nothing`、完整 interface application、重复与正负矛盾 | `Box<U> : I<U>` 对 `!I<String>` 的未知；继承方向；kind 与负向组合；静态基类不等于运行期类型 |
| 约束组合 | `T.Companion : !ValueEncoder<T>` 成立、已实现接口而不成立、缺少 public companion 而不成立、关系未知而不适用 | 与 `T : value`、companion 的 `ValueDecoder<T>` 正向条件及 `requires` 同时使用；不同主体分别判断，同一主体相反的完整接口条件拒绝；跨 Cone 保留整个条件集合 |
| 重载选择 | 原始四种调用结果、只有负向候选时无匹配、两个无蕴含关系的负向候选歧义 | `bar<Plain>` 保持第一版；直接 `foo<Plain>` 选第二版；`T : Debug` 拒绝负向版；不扩大到 `Any` |
| 声明身份 | alias／参数改名／条件换序仍重复；不同条件形成独立重载 | 当前 Cone、局部函数、成员、精确／star import、钻石 re-export 采用同一规则 |
| companion 类型 | public 存在性、正负 bound、命名 companion alias；缺少／非 public／继承冒充／无条件投影拒绝 | `T : value` 与 companion bound；签名和默认参数引用；仅投影形参不能反推宿主；完整宿主 phantom 实参身份 |
| companion 生命周期 | 只检查要求不初始化；实际求值按既有 ensure 初始化一次 | 同宿主跨 Cone 一份状态，不同完整宿主实参独立；默认值与异常初始化沿原路径 |
| 固有成员表面 | 未实现接口但拥有 public 方法可满足；继承／接口成员可满足 | 自由函数、局部重载、扩展、函数值属性不能补齐；private／protected／internal 成员不因访问权限参与 |
| 隐藏类型与可见性 | 私有具体类型可经 `T` 提供 public 操作；参数／结果类型名称按定义处绑定 | 两个 helper 的可见域不同仍使用同一公开候选；增加 private 同名重载不改变要求 |
| 调用形态 | 显式结果、命名参数、默认参数、vararg／spread、方法类型实参；无匹配／歧义／错误结果拒绝 | 要求不执行默认值；真实调用只求值一次；正文更窄参数不改选；不按结果或效果选择第二个重载 |
| 构造要求 | public class／struct 构造、普通重载与参数映射 | 非 public、abstract、interface、object、enum 整体、intrinsic-only 入口拒绝；constructor default 与 unsafe；按第 10.2.2 节定稿覆盖 inner、捕获／不捕获值的局部 class 与显式工厂 |
| 效果 | 普通／suspend、safe／unsafe、Managed／NoGC 契约及非法组合 | value 不等于 GC-free；NoGC 默认值／vararg／receiver；真实挂起调用；Context 和异常保持原合同；第 10.2.1 节的名义 bound 与 NoGC 要求组合 |
| 泛型正文 | 相同要求转发、无要求的调用方失败、不同形态需额外要求 | 按第 10.2.1 节定稿覆盖 bound／要求优先级；接口成员可调用不自动推出结构要求；局部 closure、默认模板、宿主与方法双 binder |
| 函数引用 | 受约束函数 `::foo` 的期望类型推断、条件适用性与重载选择 | 按第 10.2.3 节定稿区分要求调用与 `value::method`／`::T`；默认参数、vararg、普通 lambda 包装及跨 Cone 消费 |
| 派生成员与库使用 | 既有派生 `equals` 参与成员要求；JSON 默认 codec 与结构相等断言 | struct／enum／tuple、泛型值类型、字段不可比较、隐藏类型与跨 Cone；保留显式 codec、原名义 bound 调用链及负向入口所需完整事实 |
| 循环与错误 | 要求直接／间接循环、改变实参的循环拒绝；声明正文错误不吞掉 | 普通同实例递归允许；要求引入的非 identity 泛型递归拒绝；跨 Cone 同样检查 |
| 单态化一致性 | 两个 consumer 各自添加不同同名扩展，消费同一泛型实例 | 有固有成员时绑定同一实现，无固有成员时都不能补齐；经第三 Cone re-export 后保持；运行输出及必要 ODR 身份断言 |
| 产物与阶段 | 新条件、投影、要求、默认模板跨 Cone 往返；旧格式明确拒绝 | LocalConcrete 输出无未决结构；普通调用／动态分派／装箱／suspend 继续由后端消费；正常增量更新与复用 |

少量代表性 AST／HIR／LocalConcrete 与身份测试可用 typed 断言验证新结构及必要引用，不为每个语言用例保存整份 IR 或符号表。跨 Cone 一致性需要实际编译、链接和运行；ODR 身份断言集中在专门产物用例，不恢复正文内容复验。

诊断应区分“确定不满足”与“泛型环境缺少事实”。例如无约束 `bad` 报 `no matching function found`，并定位调用与候选的 `T : !Debug`；缺少 public companion、成员歧义、结果或效果不符分别给出原因。语法错误定位非法 token，重复／矛盾条件同时指向相关条目，递归错误展示实际声明链。

实施后的验证遵循仓库约定：先 `cargo fmt --all` 与 `cargo clippy --workspace --all-targets`，再运行相应 Rust 测试、必要 workspace 回归及正式 CLI fixture。正式 CLI 前构建 `scoop`、`scoopc`、`scoop-linker`，按[准备脚本说明](../../scripts/README.md)显式准备开发 sysroot；runner 与期望按 [fixture schema 1](../../tests/fixture_runner/README.md)。只有修改 Python 时才执行对应 Ruff 格式化／检查。

验证保留真实源码与工具，以调用和输出验证复用；不删除源码、工具或原产物来构造隔离验收。原始日志放仓库外，完成后只记录实际范围、结果与限制。本设计不附虚构的通过记录。

## 9. 备选路线与后续边界

| 路线 | 判断 |
| --- | --- |
| 调用点自由查找的任意表达式 `requires` | 会让同一实例依赖临时重载／扩展环境，本期不采用 |
| 把所选实现作为隐藏实例参数或 dictionary | 可以显式表达多种实现，但改变实例身份与调用模型，不符合本期目标 |
| 冻结定义处全部自由函数／扩展候选 | 理论上可确定化；需要保存候选环境并处理不同定义之间的契约转发，本期没有必要引入 |
| 每个要求各按定义处 private／internal 可见性查找 | 单个实例可固定，但同形要求跨 helper 不保证成立或选中相同方法，因此采用统一 public 表面 |
| 公共固有成员／companion／构造器要求 | 由类型声明决定，可转发、可跨 Cone，作为本期方案 |
| 显式函数／codec 值参数 | 用于需要局部策略的普通程序，不依赖新的约束机制 |

参考 [Kotlin generic constraints](https://kotlinlang.org/docs/generics.html#generic-constraints)的普通 bound 表面，以及 C++ 的 [requires expression](https://eel.is/c++draft/expr.prim.req)和[依赖调用候选规则](https://eel.is/c++draft/temp.dep.candidate)。C++ 涉及实例化上下文与 ADL 的跨翻译单元规则不能直接作为 Scoop 的实例一致性方案；本期也不照搬任意表达式合法性探测。Scoop 的权威行为以本文对应的三份规范为准。

## 10. 设计审查与现有设计改进

本节记录 2026-10-18 对本设计、现有泛型实现、codec、测试库及 M47 的审查。统一公开成员表面、负向关系三态判定、普通重载在定义处绑定、泛型实例身份不随消费环境变化，是应保留的基础。以下区分实施前须闭合的问题、建议的库改进和不能据此替代的现有能力；不表示已经实现或验证通过。

### 10.1 权威规范与路线图尚未同步

审查时，语言规范没有原稿引用的第 3.2.2 节，实现规范没有第 2.20 节，路线图也没有 M55 条目。语言规范第 9.5 节仍明确排除 `T.encode`／`T.decode`／`T.Companion` 泛型查找和 companion bound。文首已改为引用实际存在的基础章节，不能据此认定新增行为已进入规范。

实施前按功能同步以下内容：

- 语言规范：新条件的声明范围、依赖投影、负向关系、要求调用、条件重载与声明身份，以及本节闭合后的效果／构造／函数引用规则；同步第 9.5 节的旧排除项和相关交叉引用。
- 实现规范：Export HIR 的条件与要求引用、具体化边界、规范化身份及跨 Cone 消费职责。按实际编码改动升级格式，不预设尚不存在的章节或 wire tag。
- 路线图：增加 M55 的范围、实施批次和真实使用验收；状态保持未实施。runtime 继续沿用既有调用、GC 和 singleton 初始化合同，必要时同步说明与引用，不为源码约束增加 runtime ABI。

### 10.2 已闭合的语义边界（2026-10-19）

#### 10.2.1 名义 bound 优先与更强效果承诺

第 3.4 节当前优先选择名义 bound 的适用成员，这会遮掉成员要求中更强的效果。例如：

```scoop
public interface Countable {
    public fun count(): Int
}

@NoGC
fun <T : Countable> countFast(value: T): Int
    where requires(x: T) { @NoGC x.count(): Int } =
    value.count()
```

按当前草案，正文先绑定 `Countable.count`，其静态合同为 Managed，因此 `countFast` 在定义处不能通过 NoGC 检查。要求已经限制实际类型提供 NoGC 调用，却没有让该调用使用要求契约；相比只声明结构要求，增加名义 bound 会使这一用法失败。

定稿保留名义 bound 优先，上述组合按 Managed 拒绝；结构调用可放入只使用同一要求契约的普通 helper。要求的 NoGC 承诺不能套到另一个 bound 目标上，也不能在具体化时因效果失败而重选重载。

验收至少比较仅有结构 NoGC 要求、仅有名义 bound、两者同时存在三种情况，并覆盖实际 NoGC value 实现与普通 Managed 实现。先确定预期诊断和调用目标，再修改实现。

#### 10.2.2 构造要求与隐式运行期环境

public 构造器不一定只需要显式参数。语言规范第 9.1.3.1～9.1.3.2 节允许 inner class 依赖 outer 实例、局部 class 捕获不可变运行期值；这些值不属于完整类型身份。若 `requires { T(...) }` 借用查询位置的隐式 receiver 或局部环境，同形要求在不同 helper 中就可能得到不同适用性，类型实参也不足以生成实际构造调用。

首版排除需要隐式 outer 或局部捕获值的构造要求，不从调用方补环境。需要这些值时，使用已有的显式工厂参数或绑定构造器引用，例如传入 `() -> T`。没有运行期捕获的局部 class 仍按其公开构造入口检查，不能仅因类型名称是局部或非 public 就拒绝。

验收覆盖 inner、捕获值的局部 class、无捕获的局部 class，以及调用点恰好拥有所需 outer 仍不改变要求结果的组合。显式工厂保持普通求值、捕获和构造语义，不增加隐藏实例参数。

#### 10.2.3 要求调用不等于函数声明引用

`requires(x: T) { x.read(): String }` 可以由 `read(prefix: String = "")` 满足；该要求保证零实参调用可用，却不能保证 `value::read` 的类型为 `() -> String`。现有 callable reference 使用完整声明签名，不保留省略默认参数或 vararg 展开的调用协议；要求中的结果适配也不能被当成目标声明的原始返回类型。

首版仅通过成员要求支持实际调用，不据此直接形成 `value::method`、`T::method` 或 `::T`。需要函数值时使用普通 lambda，例如在明确的 `() -> String` 上下文中写 `{ value.read() }`。若名义 bound 或已知具体声明本身支持函数引用，继续按原规则处理，不因存在额外要求改变该引用。

这与引用带约束的普通泛型函数是两件事：`::foo` 仍须通过期望函数类型确定完整实参，并执行新条件适用性与普通重载选择。验收分别覆盖这两类引用，包含默认参数、vararg、结果适配、suspend 与跨 Cone 泛型引用；不得将调用契约悄然包装为另一种声明引用。

#### 10.2.4 既有派生 `equals` 的成员表面

结构相等断言要求 `requires(x: T, y: T) { x.equals(y): Boolean }` 能使用既有 struct／enum／tuple 派生比较。实施前须明确这些派生成员进入公开成员要求视图的规则，包括隐藏宿主的成员声明可见性与 tuple 的有效访问域；不能只枚举手写源码方法而遗漏已有普通成员调用可用的能力。

复用既有派生比较的条件、身份和调用实现，不新增另一套比较派生或 `Equals<T>` conformance。成员不可用时，报告实际字段／payload 的失败原因；手写同签名成员、泛型字段替换及跨 Cone 声明沿原规则处理。要求查询遵守第 6.2～6.3 节的阶段职责，不为失败候选提交实例，也不生成未使用成员的机器代码。

### 10.3 现有库的简化与改进机会

| 优先级 | 现有入口 | M55 可支持的改进 | 保留的职责 |
| --- | --- | --- | --- |
| 高 | [JSON API](../../sysroot/src/scoop.json/src/json.scoop) | 增加 companion bound 重载，允许 `Json.encodeToString(value)`、`Json.decodeFromString<User>(text)`；正文转发到现有显式 codec API | 自定义 codec、格式实现、异常和完整目标类型推断规则 |
| 高 | [相等断言](../../sysroot/src/scoop.test/src/equality.scoop) | 增加结构调用入口，让已有派生 `equals` 的 struct／enum／tuple 直接用于 `assertEquals`／`assertNotEquals` | 原名义入口、失败数据、SourceLocation 默认值、可比较性与派生成员规则 |
| 中 | [集合查找](../../sysroot/src/scoop.std/src/collections/find.scoop)、[比较器](../../sysroot/src/scoop.std/src/comparisons/comparator.scoop) | 为 `indexOf`、`contains`、`naturalOrder` 等评估结构能力入口，减少仅为调用算法而声明接口或包装类型 | 显式 comparator、原泛型调用链与普通 `.equals()`／`.compareTo()` 语义 |
| 中 | [DebugPrinter](../../sysroot/src/scoop.core/src/debug.scoop)、[CLI](../../sysroot/src/scoop.cli/src/cli.scoop) | 增加 `T.Companion : ValueInspector<T>` 的直接格式化入口，以及带 decoder bound 的 `Cli.parser<T>(...)` 普通泛型工厂 | ValueView、显式 inspector／codec、schema 与解析行为；不在构造器上增加本期排除的约束 |
| 中 | [M47 codec 生成](../milestone47/DESIGN.md) | 将公开 companion 的默认 codec 调用收敛为普通受约束库 helper，生成正文调用该 helper | 字段展开、注解策略、显式依赖优先级、递归 this、构造与默认值处理 |

例如在现有 `Json` 中增加以下重载，即可复用原有实现：

```scoop
public fun <T> encodeToString(value: T): String
    where T.Companion : ValueEncoder<T> =
    encodeToString(value, T.Companion)
```

解码方向同样以 `T.Companion : ValueDecoder<T>` 转发。两个方向独立，不要求同时实现；显式传入 codec 的既有重载继续允许目标没有 companion，或选择与 companion 不同的策略。companion 仍只在实际求值时初始化。

### 10.4 迁移边界与兼容性

**名义 bound 不能整体替换为成员要求。** 第 3.4 节不允许从 `T : Equals<T>` 自动推出结构调用要求，因此直接修改 `assertEquals` 的约束会使已有 `fun <T : Equals<T>> check(...)` 等调用链失效。建议保留原名义重载，再为非 conformance 类型增加如下条件的入口：

```scoop
where T : !Equals<T>,
      requires(x: T, y: T) { x.equals(y): Boolean }
```

实际具体类型按正／负条件选择相应入口；原名义泛型调用继续走已有重载。新的结构泛型调用方必须携带目标需要的完整事实：仅有 `requires equals` 并不能推出 `!Equals<T>`，不能靠单态化时重选补齐。若内部算法需要共用实现，可显式传入比较函数，使用已有函数值语义。

**方法要求不产生 operator 能力。** 迁移集合算法时，结构路径正文使用 `.equals()`／`.compareTo()` 等显式方法调用；不能因名称相同就让 `==`、`<` 等操作通过检查。现有运算符接口、派生比较、名义 conformance 和动态接口分派仍各自承担原职责。`HashMap<K, V>` 等名义类型上的约束也不能直接改成 M55 函数级要求，结构相等不提供哈希实现或自动产生 `Hash` conformance。

**容器 codec 工厂不能直接删除。** 当前 [Option](../../sysroot/src/scoop.core/src/option.scoop)、[Array／MutableArray](../../sysroot/src/scoop.core/src/arrays.scoop) 的 companion 提供接收元素 codec 的工厂，没有实现容器自身的 `ValueEncoder`／`ValueDecoder`。M55 不提供条件 conformance，也不允许普通成员给宿主参数追加约束，因此不能直接把 `Option<T>.encoder(element)` 改成约束宿主 T 的无参成员。可按真实使用增加有自身类型参数的普通 helper，但这不自动解决任意嵌套容器的递归 codec 选择；未存储参数、显式策略和无 bound 的容器用途继续保持。

**M47 的结构生成与默认值迁移仍然必要。** M55 不能枚举字段、读取 Serialize 策略，或表达运行时缺哪个字段就省略哪个构造实参。M47 第 5.2 节关于默认值定义处绑定、前参依赖和缺失分支求值顺序的迁移缺口仍须独立解决。公开 companion helper 只能简化对应默认分支；原有可合法访问的 private／internal companion、显式字段 codec 注入及其优先级不能被 public-only 要求静默替换。

**静态 fallback 不能替换运行期类型判断。** [断言渲染](../../sysroot/src/scoop.test/src/failure.scoop)中的 `Any is ToString` 检查运行期对象；负向 bound 只判断静态类型，不能等价替换这一路径。缺少 companion 也不满足 `T.Companion : !I`，因为该条件同时要求 public companion 存在。需要覆盖没有 companion 的静态后备路径时可考虑无约束重载；无约束泛型 helper 在定义处选中的后备版本不会随具体化自动改选。

### 10.5 首批真实使用验收

JSON 默认 codec 重载与结构相等断言是本期建议优先落地的两组实际使用场景，已对应到第 7～8 节。它们用于验证新增语言能力和既有调用组合，不要求同时完成第 10.3 节的全部库迁移。

1. **JSON 默认 codec 重载**：实际编码／解码普通 DTO，覆盖两个方向独立实现、显式自定义 codec、命名 companion、缺少 public companion 的诊断、已带 companion bound 的泛型转发，以及跨 Cone 调用。验证约束查询不初始化 singleton，实际调用遵守一次初始化；显式 codec 路径的结果保持。
2. **结构相等断言**：覆盖 struct、enum、tuple 和泛型值类型的既有派生比较、成功与失败断言、不可比较字段诊断、隐藏类型和跨 Cone 消费。原 `Equals<T>` 泛型调用链继续可用；新负向结构入口必须检查完整事实，不能给类型补接口。保留 `SourceLocation` 默认值求值位置和原失败渲染行为。

这些用例使用现有正式 CLI fixture runner，并遵守第 8 节的诊断与组合覆盖要求。审查阶段只记录目标与边界，不附运行通过记录。
