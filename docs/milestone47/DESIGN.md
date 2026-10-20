# M47 设计：HIR 查询与接口实现生成器

状态：设计完成，待实施。日期：2026-10-15。调研基线：`e548b9ebe`。M46 正在其他任务中实施；本文以 [M46 主设计](../milestone46/DESIGN.md)和 [Serialize／Option 合同](../milestone46/SERIALIZE.md)及其完成后的实现为迁移基线，不实施或重新安排 M46。

依据：[语言规范](../specs/SCOOP-SPEC.md) 8.5、9.4～9.6、11.13，[编译器与产物规范](../specs/SCOOP-IMPL-SPEC.md) 2.1、2.2、2.7、2.11、2.17，以及[路线图](../ROADMAP.md)。本文规定功能边界、编译编排、试点迁移和验收；[同步协议与语义查询](PROTOCOL.md)规定进程交互、HIR 查询与方法体源码片段。本次只修改设计；各批实施前先同步相应 spec。

## 1. 本期交付与边界

M47 只新增两个基础能力：

1. 通用的 HIR 查询接口，向 generator 提供现有类型、成员、注解、默认值和正文等语义信息，供后续 LSP 复用。编译器内部有实际需要时也可重构为调用同一接口，查询实现复用已有语义规则。
2. generator 的同步交互流程。driver 启动宿主可执行程序，建立 stdin／stdout 双向管道；generator 逐次查询 HIR，最后一次返回 `MethodId -> 方法体源码文本`。driver 使用现有 parser 得到普通 AST，再交回正常 HIR lowering。

encoder／decoder 是这两个能力的落地试点：把 M46 更新后的合成策略迁入独立的 `scoop-codec-generator`，使 `Serialize` 完全按普通 annotation class 绑定和保存。试点中的字段策略、codec 选择和源码组织归 generator；不能为迁移预先扩展 AST、名称绑定、类型系统或默认值语义。实际缺口的处理见 4.1 和 5.2。

总原则：driver 仅使用 generator 填充指定 interface 的缺失方法实现。GenerateWith 是标准 annotation class，其所修饰的实体照常显式声明全部继承；interface requirement、方法签名与分派关系在生成前确定。生成源码只接受普通词法作用域、名称解析、重载、访问域、类型和 effect 规则，不能修改声明表面或重跑已经完成的用户源码决议。

现有 AST 是语法结构，不具有已绑定 callable／类型／成员引用能力。本设计不新增这种能力，也不增加引用绑定旁表、特殊源码标记、HIR 回填、访问豁免或 generator 专用的 default 操作。查询返回的实体 ID 用于语义查询和确定待填方法，生成正文中的名称仍由普通 HIR lowering 解析。

进程交互每次最多有一个未完成查询，双方阻塞读写。M47 不实现 LSP 服务、编辑器增量分析、异步 IO 调度、进程池或重入生成。首个 generator 使用 Rust 实现；协议输出为文本，不要求 generator 依赖 Rust AST 数据结构。

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

interface 数组中的每个字符串都是一个独立的类型选择配置。driver 按数组顺序，复用普通类型语法入口将每个元素解析为恰好一个完整类型，再通过已有类型绑定入口，在被修饰 owner 的词法上下文中解析 import、类型参数、限定名和透明 alias。之后与该 owner 已有的完整 interface application 比较并选定实际 typed 引用；不能按显示字符串相等、FQN 回退或隐式添加继承来完成匹配。generator 名称须非空；数组元素为空字符串、类型语法错误、非 interface、裸泛型模板、错误实参数量或未继承应用均定位到相应数组元素。

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

`scoop` 把有效绑定随 single-Cone 编译请求交给 `scoopc`；`scoopc` 只启动宿主工具，不定位或构建上游源码。构建目标为 Linux、宿主为 Darwin 时，generator 仍是 Darwin 程序。生成器返回普通 Scoop 源码片段，代码布局和 ABI 继续由后续 stage 决定。

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
driver 读取 GenerateWith，选择已有接口；依次查询／生成方法体源码
  ↓
driver 调用现有 parser，将片段解析为普通函数体 AST
  ↓
hir-lower 按原声明上下文检查并 lower 生成正文
  ↓
已有完整性、effect 等检查 → 完整 HIR → MIR／LIR／codegen
```

生成钩子放在现有 encode／decode 正文合成的位置附近：普通源码正文已处理，必要声明与注解已按原规则绑定，最终 HIR 尚未封装。普通继承处理建立既有方法签名并记录仍缺失的实现；driver 到生成点才读取 GenerateWith，将配置匹配到其中的实际 interface application。未被填充的必要方法在最终完整性检查中诊断。不能为解析这个注解另设提前绑定流程，也不能为读取注解反复绑定同一应用。

当前实现的定位点是 `hir-lower/src/pipeline/run.rs` 中的 `check_inheritance`、`lower_derived_encoding_bodies` 和 `lower_derived_decoding_bodies`，以及 `driver/src/request/preflight/current_hir.rs` 的 current-Cone lowering 调用。M46 可能调整这些目录和函数；实施按职责接入，不要求 M46 保持调研时的函数名。

这是 HIR lowering 内部的正常合成步骤，不发布一个供下游消费的“半成品 HIR”阶段。待生成正文只属于前端正在构建的状态；成功返回的 HIR 必须保存全部必要的完整 typed body。

### 3.2 driver 与 lowerer 的分工

HIR lowering 入口接收 driver 提供的生成钩子。到达生成点时，lowerer 交给钩子已有的接口／缺失方法描述、普通注解 metadata 和借用的语义查询接口。driver 按 GenerateWith 配置选择目标、组装任务，负责子进程、消息收发、查询转发和收集源码结果。driver 将源码交给 parser，向 lowerer 返回普通函数体 AST 和诊断源码；它不持有或直接改写 lowerer 的内部 arena。

查询接口由编译器侧实现，进程调度和生成配置的解释留在 driver。interface 的各个字符串元素使用 parser 的通用类型片段入口产生普通 TypeRef AST，再由 hir-lower 已有类型绑定入口按 owner 上下文解析；parser 不认识 GenerateWith。生成正文使用同样的职责分工：parser 只解析文本，hir-lower 完成语义检查。任务和查询契约沿 HIR 数据通道共享，函数体沿现有 AST 数据通道传递；hir-lower 不依赖 parser 的实现、driver、进程协议实现或 generator 算法，也不解释 GenerateWith 配置。

生成正文的语义上下文来自待填方法：原文件的 package／import、owner、receiver、参数名称、类型参数和普通访问域。片段的文件名与位置只用于诊断，不另建作用域或授予权限。源码中通过正常名称、限定名和显式类型实参表达调用；查询得到的 ID 不能作为直接绑定表达式写回。新增正文照常执行名称解析与重载选择，不影响已完成的其他正文。

同一 Cone 内按 generator 名称分组，一组任务使用一个进程、一个 `generate` 请求；组内按实际声明顺序排列，owner 内的绑定按注解应用及数组元素顺序排列。一条 GenerateWith 的全部接口进入同一个任务的 bindings，缺失方法取并集并按 MethodId 去重，不为每个数组元素单独启动进程或发送 generate。相同 owner 可以出现在不同组中，各组只得到对应接口绑定覆盖的缺失方法；同一 owner 的其他同名 generator 配置也并入该任务。首版依次执行这些组，不建立进程池。driver 收齐各组结果后再填充正文，因此任何一组查询都看到同一批已确定的声明和已有正文，不能依赖另一 generator 的执行顺序。

查询不会触发其他 generator，也不会因为读取尚待生成的方法而递归编译。互递归方法通过预先建立的签名相互引用。填充后的普通 body lowering 可以执行其本来需要的类型应用、默认模板展开和局部声明处理，但不能反向改变已经选定的用户源码调用。

### 3.3 结果处理

driver 在协议边界检查消息、目标 MethodId 及结果格式；非任务方法、重复方法或覆写已有正文在接收时诊断。每个 source 必须是恰好一个带大括号的普通函数体，完整解析后不得剩余其他声明或文本。parser 报告词法／语法错误，hir-lower 沿已有正文入口报告名称、重载、返回类型、参数、访问域、泛型约束和 effect 错误，不增加第二套语义验证器。

driver 保留收到的原始文本作为本次编译的诊断源码，普通错误显示片段内的行列并关联 GenerateWith 应用。需要调试导出时复用已有源码／dump 通道保存片段；它们不进入普通源码扫描，不重新注册类或方法声明。片段使用规则见协议第 5、6 节。

生成结果使用原方法声明的 typed identity。可执行文件路径、generator 名称、请求编号和进程执行顺序都不参与源码方法 identity。正文变化按既有语义／代码指纹影响产物。

## 4. 通用 HIR 查询接口

共享层面向现有实体、类型和已绑定语义，返回结构化数据。generator、后续 LSP 和有实际需要的编译器内部代码使用同一查询契约；进程消息、MethodId 到上下文的映射、编辑器位置转换留在各自适配层。查询核心不依赖 generator 生命周期，也不返回 LSP 的 hover／CompletionItem。

已有 `scoop-hir::static_type_shape`、静态注解／构造描述和 `ImportedSemanticWorld` 是数据访问的基础。类型关系、成员访问与类型实参替换直接复用 hir-lower 的原有规则；必要时整理现有代码供查询与内部调用共同使用，不复制类型系统或全面重写前端。

| 所在模块 | 本期责任 |
| --- | --- |
| `scoop-hir` 的查询契约与现有视图 | 现有 typed 引用、签名、声明／应用类型、成员、注解、default 和正文的只读描述 |
| `scoop-hir-lower` 的查询适配 | 将当前构建上下文与依赖视图接到公共接口，复用已有语义规则；按原方法上下文 lower 普通函数体 AST |
| `scoop-parser`／`scoop-ast` | 使用原语法和 AST；必要时包装现有函数体／类型片段解析入口，不增加绑定引用或生成专用节点 |
| 新 `scoop-generator-protocol` | JSON-RPC 消息、同步分帧和客户端封装；结果仅包含目标 MethodId 与源码文本，不传输出 AST schema |
| `scoopc` driver | 读取普通配置、启动进程、转发查询、接收源码、调用 parser 并交回正文，复用已有诊断与构建输入记录 |
| 新 `scoop-codec-generator` | 使用查询数据解释 Serialize、选择 codec、生成普通 encode／decode 源码片段 |

既有 `scoop-protocol` 继续承担 scoop 与 scoopc 的 single-Cone 构建协议，只传递实际需要的工具配置和使用结果。M47 不实现 LSP 服务器、增量数据库或另一套编译器服务。

只读查询不改变声明或已完成的语义决定。现有类型应用和依赖投影可缓存复用，不为查询深拷贝全部 HIR。尚无正文、正文未发布等情况通过查询结果表达，不把半成品类型或空正文装入成功 HIR。

### 4.1 只补实际接入缺口

| 现有接入点 | 必要整理 |
| --- | --- |
| parser 公开完整文件解析，内部已有 `parse_block`／`parse_type_ref` | 包装通用片段入口，完整消费输入；只接收文本，不查询 HIR，不扩展语法或 AST |
| 当前 codec 有专用的正文合成接入点 | 整理为对既有 interface 缺失方法的通用补齐回调，复用签名、继承选择和正文检查 |
| 类型、成员、注解和 default 数据分布在已有视图中 | 向公共查询接口提供所需事实；缺少投影字段时补该字段，不重新建一份并行语义状态 |

额外缺口必须由当前接入或 codec 试点的真实用例证明，再沿原有机制作最小修补。不得仅为可能方便 generator 或未来 LSP，增加 AST 语义引用、专用默认值操作、任意 HIR 拼接或新的名称解析规则。

## 5. encoder／decoder 的迁移

### 5.1 迁移的是算法

独立程序同时处理 Encodable 与 Decodable 任务，并共享字段配置、结构查询和 codec 选择的公共部分。它通过普通依赖命名空间取得 core 声明，在查询与策略判断中使用 typed ID，最终用正常名称和语法生成源码。编译器无需知道哪一个任务表示 encode 或 decode。

迁移时给 M46 实际请求缺省正文的 companion／codec 添加对应的 GenerateWith，包含 core 中的普通 helper、库源码、示例和 fixture。只请求 encode 时数组只列编码接口；两个方向使用 scoop.codec 时列在同一数组中，也可通过两条注解选择不同 generator。已有完整手写实现不需要添加配置。相比 M46，仅有空的接口继承声明不再隐式触发 codec 合成；这个显式请求的源码变化属于 M47，编解码行为继续保持 M46 合同。不保留 interface 级默认配置或 compiler-known codec 自动触发作为第二条路径。

| 现有职责 | M47 归属 |
| --- | --- |
| 识别 interface requirement、登记方法签名 | 普通继承／实现选择；driver 仅按 GenerateWith 筛选缺失方法集合 |
| struct／enum／tuple／class 的可合成形状与方向限制 | codec generator |
| 字段 codec 的依赖、递归 this、companion、容器和 tuple 组合选择 | codec generator，使用普通类型关系与成员查询 |
| Serialize 的 rename／required／emitNull／ignore 解释与策略诊断 | codec generator |
| 字段读取、presence 分支、variant 分支、container 调用与返回 | codec generator 生成普通源码片段 |
| 缺失值分支与构造调用的源码组织 | generator；以现有语法和普通库能力实现，默认值迁移缺口按 5.2 核对 |
| 默认值定义处的绑定、普通调用默认参数、访问／类型／effect 检查与 HIR 构造 | 编译器的已有公共语义路径 |
| Option 的 null 表示、各 container 的运行行为、JsonValue | M46 的普通 core／JSON 实现 |

调研时的迁移入口包括 `class/inheritance/coding/`、`encoding/`、`decoding/` 和 `user_annotations/serialization.rs`。M46 完成后按这一职责表复核所有新增加的分支，一起迁移；不能从 M29 的旧实现另起一份而遗漏 M46 行为。

禁止把迁出改成“外部程序请求编译器生成 codec，再原样返回”：共享查询中没有 `selectFieldCodec`、`serializeOptions`、`planDecodeRecord` 或 `generateEncodeBody`。编译器返回字段类型、构造/default 引用、普通注解等事实，由 generator 作出相应决策。

### 5.2 M46 行为与需要核对的迁移缺口

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

表中行为是 codec 试点的迁移目标，不意味着任意已绑定 HIR 都能直接打印为等价源码。默认值查询可以用于查看前参依赖、类型与直接 None 等事实；它不提供可写回的绑定引用，也不替 generator 生成正文。

普通调用省略实参时，编译器已有默认参数机制保留定义处绑定。decoder 的运行时字段缺失、default 求值顺序及跨作用域引用，需要在 M46 完成后的真实用例上验证如何由普通源码表达。将 default 中的短名、重载调用或局部声明直接复制到 codec 作用域，不能视为语义已保持；可见性合法也不等于绑定相同。

这属于试点需要核对的具体缺口。先使用现有调用规则、普通源码和库能力实现；确实无法完成时，明确失败用例及原有机制缺失之处，再作必要的最小修补。本设计不预设默认值反编译器、AST 绑定引用、绑定旁表或 default 展开 RPC；不能靠这些新增能力宣称迁移已经闭合，也不能静默放弃 M46 的原有行为。

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

Rust workspace 增加协议库和 codec generator 可执行目标，正式开发／分发构建把它与 `scoop`、`scoopc`、`scoop-linker` 一同准备。generator 使用 HIR 查询契约和协议 SDK，输出普通文本；不要求依赖 scoop-ast、hir-lower、LLVM 或 Scoop 程序运行时。

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

通用查询和 generator 交互可以独立推进；codec 迁移以 M46 完成后的实现为基线。M47 不要求先交付 M38 TypeInfo、LSP、M45 的 Scoop 进程 API 或其他尚无实际必要的前置；Rust driver 使用已有宿主进程能力。

| 批次 | 交付 | 同批验证 |
| --- | --- | --- |
| M47-1 | 同步 spec；整理通用 HIR 查询接口和已有实现，声明普通 GenerateWith，复用字符串数组参数与既有方法签名 | 普通数组注解绑定、显式继承、多接口任务合并、任务去重与冲突诊断；查询结果与原有语义一致 |
| M47-2 | 同步协议、driver 查询循环、CLI 注册；返回源码文本，复用 parser 及正常正文 lowering，完成通用功能闭环 | 用户 interface 全部／部分生成；真实子进程查询后提交源码；语法与类型诊断定位到片段；错误 ID、EOF、失败退出与宿主／target 分离 |
| M47-3 | 用 M46 encoder／decoder 做迁移试点；核对 5.2 的实际缺口并作必要修补，策略外移且 Serialize 只走普通注解绑定 | M46 合成回归、普通注解／跨 Cone 读取、default 定义处绑定与求值顺序；生成源码可检查，不依赖新增 AST 语义引用或隐式 fallback |
| M47-4 | 工具分发／core bootstrap、实际工具输入缓存、泛型与 `.slib` 消费；清理只为旧合成存在的数据 | core 构建、工具变更失效、缓存命中不重跑、下游复用已有方法不调用上游 generator、三目标必要组合验收 |

各批提交必须保持相应入口完整可用，不以空 body、返回未实现错误的占位分支或自动 fallback 作为交付。

| 规范位置 | 实施前同步的内容 |
| --- | --- |
| 语言规范 9.4 | GenerateWith 的普通 annotation class 声明、`String`／`Array<String>` 参数、target 与 Repeatable；复用已有字符串数组支持，不扩展 annotation 语法或值域；Serialize 的普通检查边界 |
| 语言规范 9.5、11.13 | 实现类型显式选择 interface application 与 generator；迁移原隐式 codec 请求，保持 M46 编解码规则，说明策略诊断按实际生成方向发生 |
| 语言规范 9.6 | 静态描述通过工具查询用于 interface body 生成，仍无运行期反射或任意源码宏 |
| 实现规范 2.1、2.2、2.11 | 普通源码片段的解析与原方法上下文、必要的正文补齐接入、完整 HIR 和 typed identity；保持既有 AST 与语义规则 |
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
| 返回正文 | 非任务 method、错误 ID 种类、重复 method、已有正文覆写均拒绝；源码片段走普通词法／语法、名称、类型及 effect 检查；块外声明拒绝，遗漏必要方法保持正常未实现诊断 |
| 源码与调试 | 保存可读片段，诊断行列对应实际生成文本并关联 GenerateWith；参数、泛型名称与 import 沿原方法上下文工作；打印的 ID、注释或显示文件名不具有绑定语义 |
| 同步协议 | 真实程序按“任务→多次查询／响应→源码结果”完成；中文、转义及较大片段的分段管道读写正确；stderr 日志不污染协议；协议版本／帧／退出错误可定位 |
| Serialize 普通化 | 参数类型、target、重复仍走通用检查；策略错误来自实际 generator；手写 codec 不触发专用解释；同名不同 annotation、alias、re-export 与跨 Cone identity 正确 |
| M46 语义保持 | rename／required／emitNull／ignore、None 省略、原声明 T／T?、直接 None default、带副作用及引用前参的 default、完整字段 codec 优先、全部合成形状及错误路径 |
| 真实查询 | codec generator 自己作出字段配置、codec 和构造决策；查询提供原声明／应用类型、普通注解和 default 事实；同一内部查询接口可由进程适配层外的实际编译器调用使用，无 codec 计划或 default 展开入口 |
| 构建与缓存 | core bootstrap、CLI 注册、修改 generator 后失效、缓存命中不调用、无任务不启动、程序缺失／失败不回退 |
| 产物与运行 | 源码和工具保持存在，复用 DTO metadata 并生成本地 codec；再次构建／下游消费不重跑有效的上游生成任务；泛型、ZST／大值和托管引用按必要组合覆盖，不增加源码／工具隔离场景 |

先执行变更语言对应的格式化和 lint，再执行相关 Rust 测试、fixture runner 规则测试及正式 CLI fixture。将 generator binary 纳入 fixture 的正式工具准备，不能用内存测试后端代替端到端验证。最终覆盖 Darwin、Linux GNU、Linux musl 的适用组合；只有实际影响共享 ABI、GC 或产物基础时才据此扩大回归。

M47 完成的判据是：保持 M46 编解码行为的真实程序经独立 generator 编译运行；编译器中已无 Serialize 专用解释和 encode／decode 策略合成；普通用户 interface 也能通过同一同步协议补充方法实现。设计提交本身不声称这些验收已经执行。
