# M47 设计：接口实现生成器与编码合成迁移

状态：设计完成，待实施。日期：2026-10-15。调研基线：`e548b9ebe`。M46 正在其他任务中实施；本文以 [M46 主设计](../milestone46/DESIGN.md)和 [Serialize／Option 合同](../milestone46/SERIALIZE.md)及其完成后的实现为迁移基线，不实施或重新安排 M46。

依据：[语言规范](../specs/SCOOP-SPEC.md) 8.5、9.4～9.6、11.13，[编译器与产物规范](../specs/SCOOP-IMPL-SPEC.md) 2.1、2.2、2.7、2.11、2.17，以及[路线图](../ROADMAP.md)。本文规定功能边界、编译编排、试点迁移和验收；[同步协议与语义查询](PROTOCOL.md)规定进程交互、实体引用、查询与方法体 AST。本次变更只包含设计和路线图；各批实施前先同步相应 spec。

## 1. 本期交付与边界

M47 把“为已继承 interface 的缺失方法提供实现”做成可扩展的编译器能力。首个实际使用者是独立的 `scoop-codec-generator`：把 M46 更新后的 encode／decode 合成算法整体迁出编译器，保留 M46 的语言行为，并使 `Serialize` 完全按普通 annotation class 绑定和保存。

总原则：driver 仅使用 generator 填充指定 interface 的缺失方法实现。GenerateWith 是标准 annotation class，其所修饰的实体照常显式声明全部继承；注解解析、参数绑定、类型系统、方法选择、访问域和正文检查都使用普通机制。不为 generator 增加其他特殊约定、隐式声明、专用语法或检查豁免。

1. generator 是宿主可执行程序，由 `scoop` 准备执行配置、`scoopc` 启动和管理。首个 generator 使用 Rust 实现，不动态加载 Rust 插件，不要求使用 Scoop 自举。
2. driver 建立 stdin／stdout 双向管道，发送生成任务；generator 串行查询当前编译的 HIR 语义信息，最后一次返回 `MethodId -> BodyAst`。
3. 每次最多有一个未完成的查询。双方使用阻塞读写；driver 在普通循环中回答查询，收到最终结果后继续编译。无需异步 runtime、IO 多路复用、后台查询服务或重入编译。
4. interface requirement、实现类型、完整方法签名与分派关系在调用 generator 前确定。generator 只填正文，不改变已有类型的继承关系或方法签名，不向现有类型或包注入其他声明。
5. 正文按原方法的词法 owner、receiver、参数、泛型 binder 和 effect 检查；得到普通 typed HIR 后进入已有后续阶段。已经完成的源码类型解析和重载决议不重跑。
6. 共享语义查询契约和实现可供未来 LSP 复用。M47 不实现 LSP、编辑器增量分析或不完整源码恢复，也不把 LSP 的会话调度引入 generator。
7. encoder／decoder 的字段策略、codec 选择、构造计划和 `Serialize` 解释都归 generator。编译器提供普通类型、成员、注解、默认值和正文查询，不能保留一个替 generator 做这些决定的专用查询入口。
8. `Serialize` 保留在 `scoop.core`，名称、四个参数与 M46 的用法保持。编译器只执行普通注解声明、参数、target、repeatability 等规则。

本期不迁移结构 equality、委托、accessor、构造初始化、closure／coroutine／ABI bridge 等其他合成。GenerateWith 的两个参数分别为普通 `String` 和 `Array<String>`，不扩展 annotation 值域或语法；不增加通用源码宏、顶层声明生成、任意运行期类型／函数值、任意编译期 Scoop 执行、generator 递归调用或新的 runtime 机制。方法体中的局部变量、闭包及其正常 lowering 仍按已有语言规则处理。

## 2. 如何为一个类型的不同 interface 指定 generator

### 2.1 通用声明入口

GenerateWith 是 `scoop.core` 中的普通 annotation class，标在实际实现类型上。一条注解为同一个 generator 指定一组接口，一轮生成它们的缺失实现；同一个类型可以重复标注，为其他接口选择不同程序。interface 声明本身不绑定生成策略。

```scoop
@Target([AnnotationTarget.CLASS])
@Repeatable
public annotation class GenerateWith(
    val generator: String,
    val `interface`: Array<String>
)
```

两个参数都没有默认值，按普通注解规则检查、绑定和保存。`interface` 使用普通数组字面量，即使只选择一个接口也写作 `["I<T>"]`；直接传入 String 按普通参数类型规则报错。`interface` 是现有关键字，因此声明和具名实参使用普通反引号转义，实际参数名称仍为 interface；不为这个注解放宽关键字或命名参数语法。

语言规范 9.4 与现有 M35 实现已支持 annotation class 的一维 `Array<String>` 参数，包括空数组、字符串常量元素、尾随逗号、元素来源以及跨 Cone metadata。M47 直接复用现有 parser、注解数组绑定和产物表示，无需新增字符串数组支持。

```scoop
@GenerateWith(generator = "my.encoder", `interface` = ["Encodable<User>"])
@GenerateWith(generator = "my.decoder", `interface` = ["Decodable<User>"])
public class UserCodec : Encodable<User>, Decodable<User> {}
```

源码 parser 不识别 GenerateWith 的名字或参数含义，只产生普通 annotation AST；HIR 使用普通参数／target／Repeatable 规则绑定它。之后 driver 通过普通 core 命名空间查到 GenerateWith 的实际声明，读取正常 metadata 中对它的应用，解释 generator 名称与 interface 数组中的各个字符串，选择要填充的既有接口实现。导入别名、generator 的常量字符串及数组中的字符串常量元素都沿普通注解规则工作；不在 parser／HIR 中增加 GenerateWith 专属的 core role、schema 校验或注解节点。

CLASS 沿用覆盖名义声明的规则；driver 只对实际承载实现的 class／struct／enum／object（包括 companion）建立任务。interface 本体不能用此入口替实现者指定策略，配置在没有相应实现对象时报告不适用。重复应用沿现有 Repeatable 行为保留各自来源，具体接口重复规则见 2.2。

codec 试点在一条注解中列出两个接口，由工具链注册的同一个程序在一次请求中处理两个方向：

```scoop
public struct User(val id: Long, val name: String?) {
    @GenerateWith(
        generator = "scoop.codec",
        `interface` = ["Encodable<User>", "Decodable<User>"]
    )
    public companion object : Encodable<User>, Decodable<User> {}
}
```

注解必须放在真正实现接口的 companion／codec 上，不能标在 User 外层并隐式转发到 companion。继承列表必须显式保留；单有 GenerateWith 不会让类型实现某个接口。Encodable／Decodable 和其他用户 interface 都使用这一通用机制。

interface 数组中的每个字符串都是一个独立的类型选择配置。driver 按数组顺序，复用普通类型语法入口将每个元素解析为恰好一个完整类型，再通过现有语义查询，在被修饰 owner 的词法上下文中解析 import、类型参数、限定名和透明 alias。之后与该 owner 已有的完整 interface application 比较并选定实际 typed 引用；不能按显示字符串相等、FQN 回退或隐式添加继承来完成匹配。generator 名称须非空；数组元素为空字符串、类型语法错误、非 interface、裸泛型模板、错误实参数量或未继承应用均定位到相应数组元素。

空数组沿普通注解规则合法，表示这条配置没有选择接口，不建立生成任务；类型的接口实现完整性仍照常检查。数组自身保留顺序与重复元素，driver 再按 2.2 检查接口绑定是否重复或冲突。

类型字符串的解释发生在普通注解绑定之后，不把它变成 annotation 类型实参，不新增 InterfaceRef 参数类别、专用 annotation AST 或 parser 分支。选定后的 typed 引用用于任务与后续查询；原注解仍保存普通 `String` 与 `Array<String>`，不改变其参数类型或普通 metadata。

### 2.2 方法选择与部分实现

首先完成现有 override、继承方法与 interface default 选择。已有合法实现的方法不进入任务；错误的显式 override、重复签名、冲突 default 等仍按普通规则诊断。generator 不承担修复这些错误或改变候选选择的职责。

对仍缺正文、按普通规则需要具体实现的 method，编译器建立唯一的方法声明、固定签名与 requirement 映射。任务中的 `MethodId` 指向这个实现方法，另外保存它满足的实际 requirement 及完整 interface application。generator 可以返回全部或部分任务方法；未返回的方法最终仍按普通实现完整性规则处理，不承诺有下一轮生成或备用生成器。

abstract 类型继续遵守原有抽象 requirement 规则，不为允许保留的抽象方法强行运行 generator。属性、存储与 accessor 的合成沿现有路径，不因为本功能将 property requirement 自动改成字段生成任务。

生成绑定属于实际实现类型，数组中的每个元素建立一条 `(owner, interface application) -> generator` 绑定；同一注解选定接口的缺失方法合并成一个任务集合。选定的 interface application 必须存在于 owner 已声明的继承闭包中；注解不新增 conformance，也不能通过变型转换推造一个新的继承应用。绑定保留完整类型实参，`I<A>` 与 `I<B>` 是不同绑定，可以列在同一数组中；开放类型中的参数按 owner 的真实 binder 绑定，具体化时不重新选择 generator。

每条绑定覆盖选定 interface 及其父接口带入的 requirement，不扩展到该 owner 的其他无关接口。比如 owner 已继承 `Combined<T> : Encodable<T>, Decodable<T>`，可以为两个实际父接口分别选择 generator，也可以为 Combined 的完整 requirement 集合选择同一个 generator。读取这些 requirement 仍保留其实际声明身份，不复制成新声明。

同一 owner、同一规范化 interface application 的重复配置报错，包括同一数组内重复、不同 GenerateWith 应用间重复，以及 alias 指向同一实际应用的情况；即使 generator 相同也不静默覆盖。错误定位到后一个数组元素并关联前一个。不同应用可能覆盖同一实现方法，沿现有实现关系去重，只产生一个 MethodId：相同 generator 合并任务并保留相关绑定与 requirement；不同 generator 则在后一配置处报告冲突并关联前一处，不按顺序试探。已有合法手写／继承／default 实现的方法先被排除，不为无需生成的方法报告这种冲突。

配置不从基类或 interface 声明继承。基类已经生成的普通方法可以按正常方法规则继承；这不使派生类自动获得为新方法运行相同 generator 的配置。

### 2.3 宿主工具注册

`scoop.codec` 默认注册到随工具链分发的宿主程序 `scoop-codec-generator`；这只提供“名称→程序”的工具配置，不自动为任何 interface 或实现类型创建生成绑定。首版通过可重复的 CLI 选项提供自定义绑定，`scoop build/run/test` 和 `scoopc build` 使用同一含义：

```text
--generator my.generator=/absolute/path/to/my-generator
```

一个名称至多有一个显式绑定；显式绑定可替换工具链的同名默认项。相对路径在接收 CLI 的进程中按调用者 cwd 解析一次，之后传递已有的宿主路径表示。源码 annotation 不能指定路径。首版不新增 Cone manifest 的插件包格式、下载器、Rust 构建器或独立依赖解析器；第三方程序由使用者准备后注册。

`scoop` 把有效绑定随 single-Cone 编译请求交给 `scoopc`；`scoopc` 只启动宿主工具，不定位或构建上游源码。构建目标为 Linux、宿主为 Darwin 时，generator 仍是 Darwin 程序。生成器返回目标无关的 AST，代码布局和 ABI 继续由后续 stage 决定。

只有出现实际任务时才要求对应程序可用。没有绑定、程序缺失、无法启动或协议不相容，均在该生成请求处明确报错，不回退到编译器内置 codec 合成。

## 3. HIR 流程中的接入

### 3.1 先确定声明，再生成正文

```text
解析源码，建立声明、interface 与类型签名
  ↓
普通继承／override／default 选择，保留待填充的接口方法
  ↓
为待生成实现建立 MethodId、完整签名与 requirement 映射
  ↓
继续普通源码 body、全部普通 annotation、构造和默认值的绑定
  ↓
driver 读取 GenerateWith，选择已有接口；依次查询／生成 BodyAst
  ↓
hir-lower 按原声明上下文检查并 lower 生成正文
  ↓
已有完整性、effect 等检查 → 完整 HIR → MIR／LIR／codegen
```

生成钩子放在现有 encode／decode 正文合成的位置附近：普通源码正文已处理，必要声明与注解已按原规则绑定，最终 HIR 尚未封装。普通继承处理建立既有方法签名并记录仍缺失的实现；driver 到生成点才读取 GenerateWith，将配置匹配到其中的实际 interface application。未被填充的必要方法在最终完整性检查中诊断。不能为解析这个注解另设提前绑定流程，也不能为读取注解反复绑定同一应用。

当前实现的定位点是 `hir-lower/src/pipeline/run.rs` 中的 `check_inheritance`、`lower_derived_encoding_bodies` 和 `lower_derived_decoding_bodies`，以及 `driver/src/request/preflight/current_hir.rs` 的 current-Cone lowering 调用。M46 可能调整这些目录和函数；实施按职责接入，不要求 M46 保持调研时的函数名。

这是 HIR lowering 内部的正常合成步骤，不发布一个供下游消费的“半成品 HIR”阶段。待生成正文只属于前端正在构建的状态；成功返回的 HIR 必须保存全部必要的完整 typed body。

### 3.2 driver 与 lowerer 的分工

HIR lowering 入口接收 driver 提供的生成钩子。到达生成点时，lowerer 交给钩子已有的接口／缺失方法描述、普通注解 metadata 和借用的语义查询接口。driver 按 GenerateWith 配置选择目标、组装任务，负责子进程、消息收发、查询转发和收集结果，返回 AST；它不持有或直接改写 lowerer 的内部 arena。

查询接口由编译器侧实现，进程调度和生成配置的解释留在 driver。interface 的各个字符串元素使用 parser 的通用类型解析入口产生普通 TypeRef AST，再由共享语义查询按 owner 上下文解析；该入口复用原类型语法，不认识 GenerateWith。任务、查询和正文的数据契约放在 AST／HIR 数据通道及协议模块；hir-lower 不依赖 parser 的实现、driver、进程协议实现或 generator 的算法 crate，也不解释注解中的 generator／interface 配置。driver 返回后，由 hir-lower 使用已有方法正文入口完成绑定与检查。

同一 Cone 内按 generator 名称分组，一组任务使用一个进程、一个 `generate` 请求；组内按实际声明顺序排列，owner 内的绑定按注解应用及数组元素顺序排列。一条 GenerateWith 的全部接口进入同一个任务的 bindings，缺失方法取并集并按 MethodId 去重，不为每个数组元素单独启动进程或发送 generate。相同 owner 可以出现在不同组中，各组只得到对应接口绑定覆盖的缺失方法；同一 owner 的其他同名 generator 配置也并入该任务。首版依次执行这些组，不建立进程池。driver 收齐各组结果后再填充正文，因此任何一组查询都看到同一批已确定的声明和已有正文，不能依赖另一 generator 的执行顺序。

查询不会触发其他 generator，也不会因为读取尚待生成的方法而递归编译。互递归方法通过预先建立的签名相互引用。填充后的普通 body lowering 可以执行其本来需要的类型应用、默认模板展开和局部声明处理，但不能反向改变已经选定的用户源码调用。

### 3.3 结果处理

driver 在协议边界检查消息、ID 种类及结果格式；hir-lower 检查返回方法属于本批待填充集合、没有重复或覆写已有正文，并执行普通语义检查。错误 AST 的返回类型、参数、访问域、泛型约束和 effect 错误必须得到正常编译诊断，不能因为来自 generator 就视为已经 typecheck。

生成结果使用原方法声明的 typed identity。可执行文件路径、generator 名称、请求编号和进程执行顺序都不参与源码方法 identity。正文变化按既有语义／代码指纹影响产物。

## 4. 可复用的语义查询模块

共享层面向实体、类型和已绑定语义，返回结构化数据，不返回 LSP 的 URI／position 查询结果、Markdown hover 或 CompletionItem。已有 `scoop-hir::static_type_shape`、静态注解／构造描述和 `ImportedSemanticWorld` 是数据访问的基础。

本期先整理实际合成所需的查询契约与只读投影；涉及子类型、成员访问、类型实参替换等语义的查询，调用 hir-lower 已有规则。不要在协议 crate、HIR/meta 或 generator 中再实现一套 Scoop 类型系统。将来 LSP 通过同一前端分析接口使用这些查询，再做源码位置和编辑器结果转换。

| 所在模块 | 本期责任 |
| --- | --- |
| `scoop-ast` 的正文数据 | 普通 body AST 和结构化的已绑定名称引用；没有 generator 专属的 default／codec 操作，见协议文档 |
| `scoop-hir` 的查询数据与现有静态视图 | 任务、完整签名、声明／应用类型、成员、注解、default 和 typed body 的只读描述 |
| `scoop-hir-lower` 的查询适配 | 将当前构建上下文与依赖视图接到共享契约，复用原有语义规则；接收并 lower BodyAst |
| 新 `scoop-generator-protocol` | JSON-RPC 消息、同步分帧和 generator 客户端封装；不包含类型判定或编码策略 |
| `scoopc` driver | 读取普通 GenerateWith 配置并选择已有接口；启动、阻塞查询循环、失败诊断、回传正文及记录实际工具输入 |
| 新 `scoop-codec-generator` | 查询编译器数据，解释 Serialize，选择 codec，生成 encode／decode AST |

既有 `scoop-protocol` 负责 `scoop` 与 `scoopc` 的 single-Cone 构建协议，不能把两种消息混在一起。它只增加传递 generator 工具配置及实际工具使用结果所需的数据。新协议与独立 generator 都有当前实际调用，不为尚未实施的 LSP 新建服务器、增量数据库或额外进程。

外部只读指语义声明与已作出的判断不会被查询改变。编译器内部可复用或缓存已知类型应用与依赖投影，不要求为每次查询深拷贝全部 HIR。未生成正文由查询的明确状态表达，不能把它伪装成空 body 或无类型表达式。

## 5. encoder／decoder 的迁移

### 5.1 迁移的是算法

独立程序同时处理 Encodable 与 Decodable 任务，并共享字段配置、结构查询和 codec 选择的公共部分。它按普通依赖命名空间取得实际 core 声明引用，后续使用 typed ID；编译器无需知道哪一个任务表示 encode 或 decode。

迁移时给 M46 实际请求缺省正文的 companion／codec 添加对应的 GenerateWith，包含 core 中的普通 helper、库源码、示例和 fixture。只请求 encode 时数组只列编码接口；两个方向使用 scoop.codec 时列在同一数组中，也可通过两条注解选择不同 generator。已有完整手写实现不需要添加配置。相比 M46，仅有空的接口继承声明不再隐式触发 codec 合成；这个显式请求的源码变化属于 M47，编解码行为继续保持 M46 合同。不保留 interface 级默认配置或 compiler-known codec 自动触发作为第二条路径。

| 现有职责 | M47 归属 |
| --- | --- |
| 识别 interface requirement、登记方法签名 | 普通继承／实现选择；driver 仅按 GenerateWith 筛选缺失方法集合 |
| struct／enum／tuple／class 的可合成形状与方向限制 | codec generator |
| 字段 codec 的依赖、递归 this、companion、容器和 tuple 组合选择 | codec generator，使用普通类型关系与成员查询 |
| Serialize 的 rename／required／emitNull／ignore 解释与策略诊断 | codec generator |
| 字段读取、presence 分支、variant 分支、container 调用与返回 | codec generator 生成普通 AST |
| 已绑定默认值的分支组合与 AST 构造 | generator；读取普通默认值正文，保留引用与求值顺序 |
| 默认值定义处的绑定、普通调用默认参数、访问／类型／effect 检查与 HIR 构造 | 编译器的已有公共语义路径 |
| Option 的 null 表示、各 container 的运行行为、JsonValue | M46 的普通 core／JSON 实现 |

调研时的迁移入口包括 `class/inheritance/coding/`、`encoding/`、`decoding/` 和 `user_annotations/serialization.rs`。M46 完成后按这一职责表复核所有新增加的分支，一起迁移；不能从 M29 的旧实现另起一份而遗漏 M46 行为。

禁止把迁出改成“外部程序请求编译器生成 codec，再原样返回”：共享查询中没有 `selectFieldCodec`、`serializeOptions`、`planDecodeRecord` 或 `generateEncodeBody`。编译器返回字段类型、构造/default 引用、普通注解等事实，由 generator 作出相应决策。

### 5.2 必须保持的 M46 行为

| 行为 | generator 需要保留的事实与顺序 |
| --- | --- |
| 实现选择 | 两个方向独立；合法手写／继承／default 优先；不替错误 override 兜底 |
| codec 优先级 | 使用 M46 完成时的既有优先级；完整字段 codec 注入优先，歧义保持报错，递归 this 与普通 companion／helper 组合保持 |
| 原声明类型 | 同时查询原成员绑定类型与应用后的实际类型；`val x: T` 不因 T 后来取 Option 就获得缺省／省略，`val x: T?` 保留该能力 |
| 编码 | 字段至多读取一次；ignore 不读取；先决定 None 是否省略，再建立 child、求值 codec、编码完整字段值 |
| None 省略 | 仅按 M46 条件判断：原声明外层 Option、没有 required／emitNull，且缺失可直接恢复 None；不执行 default 或推断任意表达式等价性 |
| 解码 | key 存在时交给完整字段 codec；缺失时显式 default 优先，再考虑原声明 Option 的 None；显式 null 和错误值不触发 default |
| default 时机 | 先读取已提供字段并完成容器检查，再按声明序补缺失值，允许引用前参，每次构造至多执行一次，最后调用普通构造器 |
| 形状与访问域 | struct、现有合格 final class、enum 的命名／位置 payload、tuple／Unit 沿原范围；普通字段／property／constructor 可见性保持 |
| wire 外形 | 普通 enum 外层单 key、tuple 精确长度、M46 Option null 表示及手写 codec 组合保持 |

默认值不能重新输出成一段在 codec 词法环境中按名称解析的源码。generator 读取普通默认值的已绑定正文，把其中的表达式、语句和局部声明组合成普通 AST，保留实际声明引用，将前参引用连接到已求得的局部值，并为复制的局部绑定重新分配身份。driver 不提供 default 展开、构造计划或 HIR 回填操作，BodyAst 也不增加专用 default 节点；具体边界见协议 5.2。

默认值的外部引用仍在生成方法的普通访问域中检查，SourceLocation 不改变权限。语言规范 8.5 已要求 default 引用的外部实体覆盖 callable 的全部合法调用位置，因此保留定义处的名称／重载选择不需要赋予 codec 额外可见性。直接 None 的识别通过普通已绑定表达式与实际 variant ID 完成，不在编译器增加序列化专用布尔标记。

Option、Array、MutableArray、ArrayList、Unit 和 tuple 所需普通 helper 的选择算法归 generator；这些数据类型自身的编译器表示职责保持原有归属。生成器也不得把可空字段改成绕过完整 Option codec、直接调用元素 codec 的专用路径。

### 5.3 完成时删除旧路径

删除编译器内的 codec 策略合成入口、方向专用待生成队列和专用 Serialize 配置／诊断。清理只为这些逻辑存在的 core role、schema 校验、metadata 字段和 helper；被普通语言语义共同使用的能力保留并复用。

测试可在迁移期间对照 M46 基线，但最终生产构建只能走独立 generator。程序缺失或失败必须可观察；不能保留可自动启用的进程内 fallback，也不能在测试中绕过正式 driver 启动路径。

## 6. Serialize 成为普通 annotation class

最终声明继续是 M46 的普通 core 声明：

```scoop
@Target([
    AnnotationTarget.FIELD,
    AnnotationTarget.PROPERTY,
    AnnotationTarget.ENUM_VARIANT
])
public annotation class Serialize(
    val rename: String = "",
    val required: Boolean = false,
    val emitNull: Boolean = false,
    val ignore: Boolean = false
)
```

编译器对它执行与用户 annotation class 相同的处理：解析实际声明、检查字符串／布尔参数和 use-site、补齐静态默认值、检查重复，保存普通 annotation application、参数值及来源。导入别名、re-export 和跨 Cone 使用仍按实际 annotation identity 关联。

generator 按实际 Serialize 声明身份读取已正规化参数，解释四项配置，检查 ignore 参数冲突、emitNull 的原声明 Option 限制、参与字段和 variant 的 wire 名冲突，以及合成目标的 storage／computed／位置字段限制。没有注解与四项默认配置等价，不读取“调用处是否显式填写参数”的额外状态。

这明确改变专用策略诊断的发生位置：只有请求本方向的自动生成时，才解释本方向涉及的 Serialize 策略。没有生成任务、全部由手写方法实现时，Serialize 只作为普通静态 metadata 存在；编译器不额外跑序列化检查。普通 annotation 参数类型、target 和重复错误仍照常诊断。

generator 无法通过看到私有字段而取得访问权限，读取元数据与合法生成字段访问是两件事。返回正文仍受 codec 定义处的普通访问域约束。Serialize 不创建 companion、增加 conformance、授予访问能力或产生运行期 annotation 对象。

M46 已负责 SerialName／Transient 到 Serialize 的迁移；M47 不再实施一次旧注解迁名，也不恢复旧 Option 格式。本期变化是合成执行位置和专用解释职责。

## 7. 构建、缓存与产物

### 7.1 工具准备与 core 构建

Rust workspace 增加协议库和 codec generator 可执行目标，正式开发／分发构建把它与 `scoop`、`scoopc`、`scoop-linker` 一同准备。generator 依赖 AST／HIR 查询契约和协议 SDK，不依赖 hir-lower、LLVM 或 Scoop 程序运行时。

编译 core 本身也使用这个宿主程序查询当前 core 声明；它在 core `.slib` 生成前可独立构建和启动，避免 bootstrap 循环。当前 core 与依赖 core 使用相同的 typed 声明查询，不为 generator 复制一张 Serialize 或容器的内建身份表。

### 7.2 缓存输入

generator 是当前 Cone 的编译工具输入。缓存合同按后续 [M48 设计](../milestone48/DESIGN.md) 收敛：记录本次实际调用的 generator 名称、协议／工具版本、所选程序的修改时间与大小及显式选项；程序路径用于本地定位和更新判定，不进入源码实体 identity。不要求每次构建完整读取或哈希 generator 二进制。

`scoopc` 的成功响应返回本次实际使用的工具输入，由 `scoop` 保存到普通构建记录。查找条目时，只根据实际使用列表检查当前绑定、程序输入状态和选项是否变化；实际使用的绑定／程序缺失或输入改变时更新相应 Cone，之后保存新的使用列表。不增加独立生成器缓存，不重放缓存内容、来源或生成过程的验证。命中后不启动 generator，也不复制或重新读取已有生成产物。

旧条目缺少所需输入记录时按新的缓存格式失效。未使用的注册项不影响已完成结果；下游读取已生成方法不重跑上游 generator。缓存与下游验收保留源码和工具，通过实际调用记录证明复用，不再要求移除源码或 generator 后仍能消费成功。

v1 的生成输入是任务和编译器查询结果。使用者实现的 generator 应由这些输入确定输出；本期不提供额外文件输入、环境变量依赖或网络输入的构建契约，也不另建沙箱、来源授权或查询计费机制。需要外部资源的扩展应先设计对应的普通构建依赖声明，不能默默复用不完整缓存。

### 7.3 `.slib` 与下游

生成后的 encode／decode 是普通方法。Export HIR 保存已完成的正文和必要支持声明，MIR／LIR 与后续产物按现有规则生成。下游只消费／实例化这些正文，不重新运行上游 generator、不再次解释 Serialize、不重新选择字段 codec。

下游自己声明新的 codec 并请求缺失方法时，才为自己的 Cone 调用 generator；读取外来 DTO 的字段、注解和默认值使用已保存的依赖数据，不重跑上游源码检查。泛型 codec 在定义处生成一次，具体化不按类型实参再次运行生成器或重判原声明 Option 事实。

GenerateWith 与 Serialize 都按普通 annotation metadata 保存在各自实际 target 上，GenerateWith 的参数保持 `String` 与 `Array<String>`，沿现有数组表示保留元素顺序与来源。driver 对当前生成任务取得的 interface application 引用只是编排结果，不要求新增持久化的 annotation 值类别或第二张生成配置表。已有继承与普通方法数据照常保存；下游不为了消费方法而重新解释上游配置字符串。

不增加专用 codec 配置表、generator 输出产物格式或 runtime 表。构建请求字段或旧专用 metadata 清理若实际改变 wire，则升级对应 section／machine protocol；普通注解声明、算法迁移和普通正文不单独提升 AST／HIR／MIR／LIR 或 runtime ABI。具体版本在实施时接续 M46 的最新编号，不预占并行工作中的版本号。

## 8. 实施分批与规范同步

本期依赖 M46 的基础协议和字段合成合同完成。M47 不要求先交付 M38 TypeInfo、LSP、M45 的 Scoop 进程 API 或其他尚无实际必要的前置；Rust driver 使用已有宿主进程能力。

| 批次 | 交付 | 同批验证 |
| --- | --- | --- |
| M47-1 | 同步 spec；声明普通 GenerateWith，复用现有字符串数组注解参数，整理通用缺失方法任务、查询数据、已绑定引用与 AST 接入契约 | 普通数组注解绑定、显式继承、多接口任务合并、方法签名提前可见、继承/default/override 选择不变、任务去重与冲突诊断、查询与正文接口的必要结构检查 |
| M47-2 | 独立协议库、同步客户端与 driver 查询循环；CLI 注册及 machine request 传递，完成通用功能的正式 CLI 闭环 | 用户 interface 全部／部分生成；真实子进程多次查询后提交正文；帧拆分、EOF、错误 ID、失败退出和错误 AST；宿主与 target 分离 |
| M47-3 | 将 M46 两个方向的策略迁入 codec generator；core 绑定；Serialize 完全移除编译器专用解释 | 完整 M46 合成回归、普通注解／跨 Cone 读取、default 绑定和求值顺序；删除旧路径后仍能正式 CLI 编译运行 |
| M47-4 | 工具分发／core bootstrap、实际工具输入缓存、泛型与 `.slib` 消费；清理只为旧合成存在的数据 | core 构建、工具变更失效、缓存命中不重跑、下游复用已有方法不调用上游 generator、三目标必要组合验收 |

各批提交必须保持相应入口完整可用，不以空 body、返回未实现错误的占位分支或自动 fallback 作为交付。

| 规范位置 | 实施前同步的内容 |
| --- | --- |
| 语言规范 9.4 | GenerateWith 的普通 annotation class 声明、`String`／`Array<String>` 参数、target 与 Repeatable；复用已有字符串数组支持，不扩展 annotation 语法或值域；Serialize 的普通检查边界 |
| 语言规范 9.5、11.13 | 实现类型显式选择 interface application 与 generator；迁移原隐式 codec 请求，保持 M46 编解码规则，说明策略诊断按实际生成方向发生 |
| 语言规范 9.6 | 静态描述通过工具查询用于 interface body 生成，仍无运行期反射或任意源码宏 |
| 实现规范 2.1、2.2、2.11 | 普通 BodyAst 的已绑定引用与访问规则、前端内部待生成状态、完整 HIR 和 typed identity 合同；没有 default／codec 专用节点 |
| 实现规范 2.7 | 宿主 generator 配置、同步进程编排、缓存输入与失败诊断 |
| 实现规范 2.17 | 共享语义查询、codec 策略外移、普通注解与普通产物消费 |
| 运行时／标准基础库规范 | 核对交叉引用与 M46 行为保持；不增加 runtime ABI 或重做普通 codec／JSON 实现 |

## 9. 验收要求

验收分为功能行为、协议边界和产物组合；普通功能不保存整份 HIR 或查询报文快照。每项新语言错误需有准确位置与信息的 negative fixture，库行为优先复用 M46 已有测试。

| 组别 | 必须证明 |
| --- | --- |
| 通用扩展 | 一个用户 interface 含多个方法，generator 提供全部／部分实现；一条注解的多个接口由一次请求共同生成，共享方法只返回一次；同一类型也可为不同接口使用不同 generator，区分 I<A>／I<B>；与手写方法、父接口、普通 default、泛型参数组合，编译器没有 codec 专属允许名单 |
| 绑定规则 | 普通 `String`／`Array<String>` 参数检查、空／单元素／多元素数组、字符串常量元素和导入别名绑定；空数组不启动 generator；同名自定义注解不触发生成；空／非法类型字符串、非 interface、裸泛型模板、未继承应用、错误实参数量、数组内或跨注解的重复应用（含 alias）、重叠方法由不同 generator 接管均定位到实际元素；不新增继承，不从数据类型隐式转发到 companion |
| 声明稳定 | 普通源码可在生成前完成对这些方法的调用选择；互递归通过既有签名成立；generator 不向原类型增加 overload／字段、不改变其继承、不增加包级声明或修改方法签名 |
| 返回正文 | 非任务 method、错误 ID 种类、重复 method、已有正文覆写、非法访问、类型／effect 不匹配均拒绝；修改来源位置不能改变访问域；遗漏必要方法保持正常未实现诊断 |
| 同步协议 | 真实程序按“任务→多次查询／响应→结果”完成；大量字段或较大 AST 的分段管道读写正确；stderr 日志不污染协议；协议版本／帧／退出错误可定位 |
| Serialize 普通化 | 参数类型、target、重复仍走通用检查；策略错误来自实际 generator；手写 codec 不触发专用解释；同名不同 annotation、alias、re-export 与跨 Cone identity 正确 |
| M46 语义保持 | rename／required／emitNull／ignore、None 省略、原声明 T／T?、直接 None default、带副作用及引用前参的 default、完整字段 codec 优先、全部合成形状及错误路径 |
| 真实查询 | codec generator 自己作出字段配置、codec 和构造决策；使用原声明及应用类型、普通注解和已绑定 default；default 中的调用选择与局部声明在 AST 组合后保持语义；driver 没有返回 codec 生成计划或展开 default 的专用入口 |
| 构建与缓存 | core bootstrap、CLI 注册、修改 generator 后失效、缓存命中不调用、无任务不启动、程序缺失／失败不回退 |
| 产物与运行 | 源码和工具保持存在，复用 DTO metadata 并生成本地 codec；再次构建／下游消费不重跑有效的上游生成任务；泛型、ZST／大值和托管引用按必要组合覆盖，不增加源码／工具隔离场景 |

先执行变更语言对应的格式化和 lint，再执行相关 Rust 测试、fixture runner 规则测试及正式 CLI fixture。将 generator binary 纳入 fixture 的正式工具准备，不能用内存测试后端代替端到端验证。最终覆盖 Darwin、Linux GNU、Linux musl 的适用组合；只有实际影响共享 ABI、GC 或产物基础时才据此扩大回归。

M47 完成的判据是：保持 M46 编解码行为的真实程序经独立 generator 编译运行；编译器中已无 Serialize 专用解释和 encode／decode 策略合成；普通用户 interface 也能通过同一同步协议补充方法实现。设计提交本身不声称这些验收已经执行。
