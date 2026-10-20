# M60 设计：`scoop doc` 与多 Cone API 文档

状态：实施中，进度见 [PROGRESS.md](PROGRESS.md)。设计日期：2026-10-20。依据：[调研与建议](../scoop-doc/INVESTIGATION.md)、[语言规范](../specs/SCOOP-SPEC.md) §9.4、§12、§13.1、[实现规范](../specs/SCOOP-IMPL-SPEC.md) §2.1、§2.2、§2.7、§2.12，以及 [M52 工具链分发](../milestone52/DESIGN.md)。设计复核基线：`abf17c32`。

本期把调研收敛为可实施的工具链功能：一个或多个源码 Cone 经正常 parser／HIR 检查后，生成可离线浏览、交叉链接和搜索的 HTML API 站点。本文确定完整设计与验收范围；已实施及尚未完成的部分分别记录在实施记录中。

## 1. 目标与范围

采用调研中的三项主线：复用现有 Cone 依赖编排，复用编译器前端取得真实声明与完整类型，以接近 KDoc 的注释表达说明。文档生成是编译器的输出用途，不建立第二套名称解析、类型检查或源码发现规则。

| 范围 | M60 决定 |
| --- | --- |
| 用户入口 | 新增 `scoop doc`；支持一个或多个显式源码 Cone，省略时使用当前目录 |
| 文档输入 | `/** ... */`、连续 `///`、可选的 Cone 根目录 `README.md`；只选择目标的生产源码 |
| API 提取 | 复用完整 HIR；呈现源码声明、泛型、推断类型、property、扩展与 re-export |
| 文档控制 | 同批交付普通 core 注解 `DocHidden`、可重复的 `DocAlias` |
| 输出 | Cone／package 导航、类型及函数组页面、成员锚点、源码页、离线搜索 |
| 多 Cone | 各 root 独立合法，站点层汇总；相容共享依赖只生成一次 |
| 预编译依赖 | 继续正常参与语义检查；未选中源码的依赖不生成完整文档 |
| 更新复用 | 文档独立记录输入与结果；与机器产物分别判断缺失和更新 |
| 诊断 | 文档问题默认 warning，提供 `--deny-doc-warnings`；语言错误仍失败 |
| 实际交付 | 先完成并验证全部功能，再为现有 sysroot 的所有 Cone 补充文档注释，并把全部内置库文档纳入工具链打包 |

本期不包含文档示例执行、workspace／glob 输入、single-file 文档模式、package 文件头说明、`[doc]` 配置、Deprecated 语言语义、用户模板／渲染插件或 annotation processor。可独立分发的编译器文档模型及从 `.slib` 重建完整文档留待后续；内置库 HTML 文档按新增要求纳入本期发行包。

## 2. 当前基础与调研修正

调研日期为 2026-10-17，其代码基线与 M52 状态不能直接当作当前实现。复核后的接入点如下：

| 当前位置 | 已有能力与本期改动 |
| --- | --- |
| [CLI](../../compiler/scoop/src/cli/args.rs) | 仍只有 Build／Run／Test／Link；新增独立 Doc 参数，避免直接带入全部 build／link 选项 |
| [构建图](../../compiler/scoop/src/graph.rs) | 每图一个 root，拒绝 executable 依赖及图内版本冲突；保留这些规则，在外层组合文档任务 |
| [lexer](../../compiler/parser/src/lexer.rs)、[AST](../../compiler/ast/src/declarations.rs) | 注释仍作为 trivia 丢弃；增加文档片段、附着关系与准确位置 |
| [HIR 完成入口](../../compiler/hir-lower/src/lib.rs) | `finish_output` 仍生成 ExportHir、LocalConcreteHir 与 native-boundary 数据；首版复用，不绕过语义完成 |
| [driver](../../compiler/driver/src/request/preflight/production.rs) | 已有 HIR 后停止的 observation 路径；增加结构化文档输出，不能解析文本 dump 充当接口 |
| [源码记录](../../compiler/hir/src/source_record.rs) | `SourceRecord` 不包含完整源码；文档正文与源码页使用本次编译读取的文本 |
| [core 注解](../../sysroot/src/scoop.core/src/serialization_annotations.scoop) | 已有 Target／Repeatable、typed identity、静态参数和 use-site，可承载文档注解 |
| [Intrinsic 检查](../../compiler/hir-lower/src/annotations.rs) | 类型路径仍有 `annotations.len() != 1` 限制；需与函数路径一起改为准确的组合规则 |
| [manifest](../../compiler/manifest/src/semantic/parse.rs) | 拒绝未知字段，尚无 `[doc]`；M60 不增加该配置 |
| M52 | 已完成；源码现位于 `sysroot/src`，目标资源位于 `sysroot/lib/<canonical-target-triple>`，普通消费默认选择预编译库 |

调研中 `sysroot/lib/scoop.core/src/...` 等旧源码路径在本设计中改用 `sysroot/src/...`。随包源码存在，不改变构建图选中的 prebuilt 节点；文档命令也不能隐式重建整套 sysroot。

## 3. 命令、输入与多 root

```sh
scoop doc
scoop doc ./libs/model
scoop doc ./libs/model ./libs/client --open
scoop doc ./libs/client --no-deps
scoop doc ./libs/model --document-private-items --deny-doc-warnings
scoop doc ./libs/model --target x86_64-unknown-linux-gnu --output-dir ./site
scoop doc ./sysroot/src/scoop.core ./sysroot/src/scoop.std --no-deps
```

### 3.1 命令合同

- root 参数是有序的零个或多个 Cone 目录／`Cone.toml`。省略时使用调用者 cwd 的 `Cone.toml`，与 build／run 一样不向父目录搜索。重复选择同一相容 Cone 不生成重复条目。
- 默认生成所有显式 root，以及各依赖图中已选择为源码节点的依赖文档。`--no-deps` 只缩小文档集合；依赖仍按原规则参与类型检查。显式列出的 Cone 始终属于文档集合，即使它也是其他 root 的依赖。
- 源码用途固定为 `CompilationPurpose::Production`。沿用 `sources/when` 与 canonical target 选择，不加入测试源码、测试依赖或另一平台的互斥输入。
- library 默认生成公开 API；executable 默认包含非公开的非局部声明。`--document-private-items` 将本次所有源码 Cone 切换到后者，不包含函数内局部声明。
- 默认站点入口是调用者 cwd 下的 `target/doc/<canonical-target-triple>/index.html`；`--target-dir` 替换其中的 target 根，`--output-dir` 直接覆盖站点目录。相对路径均从调用者 cwd 解析，多 root 不以第一个 root 的目录为隐含基准。
- 复用 `--target`、`--sysroot`、`--cone-path`、`--cache-dir`、`--scoopc`、`--message-format` 及源码依赖生产实际需要的 native 工具选择。文档根没有最终链接步骤，不引入 runtime／最终链接专属选项。
- `--open` 只在站点成功发布后打开入口；打开失败报告实际错误和已生成的入口路径，不撤销文档结果。`--deny-doc-warnings` 使文档 warning 阻止站点发布，不改变普通编译 warning 的政策。

一次命令只选一个 target。自定义输出目录仍记录 target；对同一目录换 target 是一次完整站点更新，不混用两套 API。

### 3.2 多 root 与身份冲突

每个 root 保留自己的 `ResolvedBuildGraph` 与 direct dependency 环境。两个 executable 可以同时生成文档；不能造一个把两者都当依赖的 synthetic Cone。图内的环、版本冲突、可见性和 executable 限制继续由现有规则检查。

站点允许不同 root 分别依赖某库的不同版本，按完整 ConeIdentity 区分页面和搜索项。相同 identity／target 的任务只有在源码、依赖选择及实际语义输入相容时才能合并；同 identity 的冲突输入在发布前报错，不能后写覆盖。复用已有输入记录与语义选择，不增加产物内容复核。

显式源码 root 与另一图中的同 identity prebuilt 依赖可以在输入／接口相容时共享文档链接，但不改变后者的编译来源。链接必须跟随该 root 实际解析到的声明，不能只凭 group:name 或显示名称连接到站点中的任意版本。

## 4. 注释语法、归属与位置

### 4.1 词法与附着

文档块以 `/**` 开始、以第一个 `*/` 结束，沿用非嵌套块注释规则；`/***` 开头的块仍为普通注释，`/**/` 是空文档块。文档行以恰好三个 `/` 开始；`////` 与 `//!` 保持普通注释。字符串及字符字面量内部不识别这些前缀。

文档只采用前置附着：同一语法声明位置中，附着于随后完整声明，包括它的 annotations 和 modifiers。空白、空行与普通注释可位于文档和声明之间；不得越过 package、import、另一完整声明、语句或当前列表／块的结束边界。插在表达式或类型内部的片段没有声明目标，不能漂移到后续 API。

相邻文档行组成一个片段；其间没有普通 token 的多个片段按源码顺序以空段落拼接。去掉 `///` 后至多一个分隔空格；块注释去掉首尾标记、公共缩进和行首装饰性 `*`，保留正文相对缩进、空行和代码内容。CRLF 归一化及去前缀过程保留到原 UTF-8 byte span 的映射。

无目标片段在 `scoop doc` 中产生文档 warning；普通 build 仍按注释处理。未终止块注释继续是所有命令都必须报告的词法错误。

### 4.2 声明归属

| 位置 | 归属与显示 |
| --- | --- |
| 顶层／成员类型、函数、typealias、annotation class、object／companion | 对应真实声明，不复制到实例化结果、别名或派生类型 |
| struct 字段、class property、enum variant | 对应字段／logical property／variant；variant payload 参数在其签名下说明 |
| 普通参数、主构造参数 | 对应完整源码参数；带 val／var 的主构造参数同时保留参数与字段／property 的真实关联 |
| 类型声明前的文档 | 正文属于类型；`@constructor` 描述主构造器，`@param` 描述其参数，`@property` 描述其字段／property |
| 主构造 val／var 参数前的文档 | 正文属于其字段／property；构造参数说明通过所属构造器的 `@param` 表达，不自动复制 |
| 次构造器 | 独立 constructor 条目及参数说明 |
| getter／setter | 属于对应 accessor，在 property 页内呈现，不另造同名普通函数 |
| 局部声明 | 保留语言解析所需的正常归属；首版不生成局部 API 页面 |

`@param` 与 `@property` 各自绑定真实目标，不能按同名自动互相补充。直接附着说明与 owner 上的 tag 同时描述一个目标时，直接说明优先，重复说明给出文档 warning，避免静默覆盖。

### 4.3 Markdown 与 tag

正文使用 Markdown，首个普通段落作为摘要。支持下列 tag；tag 只在 Markdown 普通正文的行首识别，围栏、缩进代码块和行内代码里的 `@`、方括号保持代码文本。

| Tag | 绑定与约束 |
| --- | --- |
| `@param name` | 当前 callable／constructor／variant 的完整源码值参数；类型参数名也可作为目标，绑定到原 binder |
| `@property name` | 当前类型实际声明的字段／logical property，不顺着任意同名继承成员猜测 |
| `@constructor` | 当前类型的主构造器；没有主构造器时诊断 |
| `@receiver` | 实际扩展 receiver；普通值参数不充当 receiver |
| `@return` | callable／getter 的结果说明；不推导或改变结果类型 |
| `@throws Type` | 解析实际可见异常类型并呈现说明；不增加 checked exception 或抛出集合分析 |
| `@see target` | 符号链接或显式普通 URL，沿相同链接规则处理 |
| `@since text` | 原样展示版本说明，不参与 Cone 版本选择 |

未知 tag、缺失必需参数、目标不存在或不适用、重复的单值说明均给出准确位置的文档 warning。重复 `@param`／`@property` 按目标判断；多个不同目标的 `@throws`、`@see` 按源码顺序保留。

README.md 存在时作为 Cone 首页正文，不存在时生成 API 导航首页。README 没有声明的词法作用域，只使用普通 Markdown 链接，不把未定义的引用链接猜成符号。相对文件链接与图片按 README 所在目录解析，仅复制实际引用的本地资源；不递归发布整个目录。缺失资源属于文档诊断，实际读取／写入失败按 I/O 错误报告；已引用资源加入文档更新输入。

## 5. `DocHidden`、`DocAlias` 与 Intrinsic

两项均作为 `scoop.core` 中的普通公开 annotation class 提供，静态参数、导入、use-site、重复与跨 Cone 保存使用 §9.4 的既有机制。

| 注解 | 定义与目标 |
| --- | --- |
| `DocHidden` | 无参数、不可重复；允许 Class、AnnotationClass、EnumVariant、Field、Property、Function、Constructor、TypeAlias |
| `DocAlias(val name: String)` | 使用 `@Repeatable`；目标与 DocHidden 相同；只增加文档搜索词 |

`Class` 已涵盖 class／struct／enum／interface／object，不扩展 AnnotationTarget。普通参数、receiver、Type 使用位置及独立 getter／setter 不是这两个注解的目标；property 页仍完整显示 accessor 能力和可见性。错误 Target、非法参数或重复非 Repeatable 应用是普通编译错误。

文档语义按真实 core annotation identity 识别：正常 import alias／re-export 有效，用户自定义同名注解没有特殊含义。隐藏与 alias 不改变语言名称、可见性、重载、调用效果、ABI 或机器代码生成。

- DocHidden 省略目标的 API 页面、成员目录与搜索项；隐藏容器也省略其包含的文档条目。派生类型不因此获得语言层注解继承。
- re-export 不能把原隐藏声明重新显示出来；标在一个 overload 上只隐藏该声明。`--document-private-items` 不覆盖 DocHidden。
- 隐藏字段／property 不删除构造器签名中的参数；可见签名引用隐藏类型时保留完整类型文字，省略不可用的链接。
- `@field:DocHidden` 只作用于实际字段，不向 logical property 传播；`@property:DocHidden` 作用于 property。backing storage 本来就不单独作为源码 API 显示。
- DocAlias 仅为实际呈现的条目添加关键词，不产生可 import 的名称。空白 alias 给出文档 warning 并不进入索引；相同关键词可在索引中合并，静态注解应用仍按原规则保留。
- 源码页展示所选文件原文；API 隐藏不承担源码内容删除。

实施前先在语言规范 §13.1 明确 DocHidden／DocAlias 与 Intrinsic 的组合例外。类型和函数两条检查路径都按真实注解身份排除文档注解后，继续检查既有 Intrinsic、NoGC、Unsafe 等合同；不能保留单纯的注解数量判断，也不能允许任意用户注解混入。源码文档注释不是 annotation，不受此组合限制。

```scoop
/**
 * A user account with a stable identifier.
 *
 * Use [Account.id] when storing references to this account.
 *
 * @property id The stable identifier.
 * @property name The display name.
 */
@DocAlias("user")
public struct Account(val id: Long, val name: String)

/** Returns twice the receiver value. */
public fun Int.twice(): Int = this * 2

@DocHidden
public fun generatedHelper(): Int = 0
```

## 6. 前端、文档模型与编排

### 6.1 职责分配

只新增一个实际承担文档语法整理、投影后处理与渲染的 `compiler/doc` crate（`scoop-doc`）。跨 stage 的纯数据放入现有 AST／HIR crate，不让文档 stage 调用 parser 或 hir-lower 的内部实现。

| 层次 | 本期职责 |
| --- | --- |
| `scoop` | 多 root 图、来源选择、依赖准备、更新记录、全站页面目录与最终发布、`--open` |
| parser／AST | 词法保留、声明附着、原始签名语法、片段和 source span；不另写源码扫描器 |
| hir-lower／HIR | 正常完整语义检查、注释到 typed 声明的关联、tag 目标与文档符号解析、完整文档投影 |
| `scoopc` driver／protocol | single-Cone 请求；编排文档输入整理、HIR 与输出，支持仅文档和生产附带文档两种意图 |
| `scoop-doc` | Markdown／tag 语法整理、展示筛选、链接路由、HTML、源码页、索引和静态资源 |

driver 在 parser 之后，按文档请求调用 `scoop-doc` 对已保留片段进行 Markdown／tag 语法整理；待解析引用及其位置作为 AST 侧纯数据交给 HIR。HIR 在原声明作用域中绑定引用，文档 stage 再消费完整结果呈现。这样无需在 hir-lower 中依赖文档实现，也不会把代码围栏中的文本误交给符号解析器。普通 build 不执行文档专有分析。

### 6.2 完整源码声明视图

文档内容来自源码声明及 Export 视图，不从 LocalConcreteHir、MIR 或机器符号反推。当前完整 HIR 路径仍生成 LocalConcreteHir 和 native-boundary 数据；接受其既有成本，不增加“跳过函数体”的新语义入口。

每个文档条目保留实际 typed declaration identity、词法 owner、完整类型／签名、可见性、原始文档及位置、已绑定引用、导出 binding 与 origin 关系。模型还必须覆盖：

- 未实例化泛型定义、binder／bound、函数条件及成员调用要求；不把不同具体化结果列成多个源码 API。
- 显式类型的作者写法，包括透明 typealias；省略的返回值／property 类型由完整 HIR 补齐，不展示待推断占位值。
- 参数名、默认值源码、`vararg`、扩展 receiver、context 参数、suspend／operator 等源码合同；不泄露 continuation、ABI 展开参数或默认值 helper。
- 主／次构造器、class property 与 getter／setter、struct 字段、enum variant／payload、companion、嵌套类型、annotation 声明及类型使用注解。
- 继承／接口／覆写的真实关系，以及语言实际提供的源码可见合成成员；内部 bridge、box helper、backing storage、初始化函数不构成 API。

参数、variant、accessor 等必须通过实际 owner 与 typed 关联取得，不按名字或 span 包含关系重建声明。无对应独立声明身份的展示子项沿用所属 typed 声明与实际参数位置，不发明 FQN 身份。

完整源码文本由 driver 传递本次已读取的文本，供签名片段及源码页使用。文档数据只进入本次输出及工具自身的本地缓存；`.slib` 不包含完整源码，也不为注释正文新增 section。`SourceRecord` 继续只保存身份、摘要及位置等元数据；普通 annotation 应用继续通过已有格式保存。

### 6.3 single-Cone 请求与冷依赖

文档是输出意图，源码用途仍为 Production。协议需要明确区分“正常生产并附带文档”和“仅产出文档”：后者在完整 HIR 与文档投影完成后结束，不生成当前节点的 MIR／LIR／机器对象，也不伪造 `.slib` 成功摘要。不能把现有要求非空 dump 的 observation-only 标志直接当作已支持文档。

复用现有协议 framing、诊断、版本检查及普通位置描述。扩展 request／response 时同步升级配套协议；返回完整文档结果的位置和站点组装所需目录信息，沿既有响应返回 warning。内部文档缓存格式随工具版本更新，不在本期承诺独立分发格式。

依赖准备遵守当前 `.slib` 消费边界：

| 情况 | 行为 |
| --- | --- |
| 源码 root 未被选为需生产产物的依赖 | 完整 HIR 后结束，无该 root 的机器生产或最终程序链接；其他图消费同 identity 的 prebuilt 不构成重编译理由 |
| 源码依赖缺少可用 `.slib` | 按既有规则正常生产，可能调用 backend／native 工具；若还需文档，从同一次 HIR 结果提取 |
| `.slib` 已命中、文档缺失或过期 | 补做文档所需前端工作，复用机器结果，不重编译／重链接有效输出 |
| 文档与所需依赖结果均命中 | 直接使用已有文档结果，不再次启动前端或准备无关工具 |
| 依赖选中 prebuilt | 按原规则消费；不因另有源码目录而切换来源 |

整个命令不因 executable 文档 root 而准备 runtime 或最终程序链接。`scoop-doc` 自身不依赖 LLVM，但现有 scoopc 可执行文件仍有 LLVM 运行依赖；不能宣称首版从空缓存处理全部源码依赖也无需 backend。

## 7. API 可见性、符号链接与站点组合

### 7.1 展示规则

library 的默认公开 API 包括公开可继承类型中供子类使用的 protected 成员。effective visibility 计入全部词法 owner；internal／private 容器中的 public 字段、variant 或成员不会因此对外公开。私有模式纳入非局部声明并明确标出可见性。

property 作为一项 API，展示 getter／setter 的实际能力与各自可见性；公开 property 的 private setter 可以显示其限制，不能被误画成公开可写。扩展函数／property 归属声明 Cone 和 package，同时提供 receiver 导航，不加入 core 类型的固有成员集合。

继承成员使用原声明和已确定的类型替换关系展示；覆写与继承可以导航回来源。首版不按同名方法自动继承说明。re-export 页面列出实际导出 binding 和原目标，不复制一份实体或重新解释其文档。

### 7.2 符号链接

支持 `[Type]`、`[Type.member]`、`[label][Type]` 以及普通 Markdown URL／引用链接。显式 Markdown 引用定义优先；行内代码与代码块不作为符号链接候选。

符号链接由 HIR 在原定义处的词法、import、core prelude 和 direct dependency 环境中解析。原文经 re-export 展示后仍使用原上下文；support dependency 不额外变成可见名称。渲染器只消费已解析的 typed 引用。

`[parse]` 若解析为同一可见函数 overload 集，链接到函数组页，各 overload 有独立 typed anchor；不任意挑一个声明。不相关实体的歧义仍给出文档 warning，首版不添加按参数表达式挑选 overload 的专用语言。

名称无法解析与页面未生成是两种情况：前者是文档 warning；后者可能来自 `--no-deps`、prebuilt、DocHidden 或可见性筛选，保留文本并说明目标没有文档，不产生必然失效的 URL。必要签名信息始终保留。

### 7.3 先汇总目标，再生成链接

站点路径与锚点由 canonical Cone／typed declaration identity 确定，显示名独立做 URL／HTML 编码。package 导航键包含其 Cone；函数组由真实作用域与 overload 集组成，其组键不能代替各声明的身份。名称、反引号、split package、不同版本及 root 输入顺序均不能造成覆盖。

多 Cone 采用两步组装：先取得所有选中 Cone 的完整文档投影和经过筛选的页面目录，再依据本次全站目录生成相对链接、导航与搜索。这样后生成的 Cone、显式选择的 core 源码和菱形 re-export 都能指向唯一的实际目标，隐藏／缺页也有统一判断。

跨进程传递或缓存的投影属于实际渲染输入；页面目录变化时，只重新渲染其链接结果受影响的 Cone。不得为了后生成的目标重跑前一个 Cone 的类型检查，也不通过扫描生成 HTML 猜测声明和链接。

## 8. HTML、搜索与预编译库边界

站点包含总入口、Cone 首页、package 列表、类型页、函数组页、独立成员锚点与有可用文本的源码页。源码原文直接嵌入生成的 HTML，API 页面通过站点内相对链接进入源码页及行锚点，不使用指向原始源码文件的 `file://` 引用。源码页按 `SourceIdentity` 和已读文本生成，行锚点与声明 span 对应；不补扫未选中的源码目录。

渲染输入中没有对应源码文本时，不生成该源码页、源码展示区或源码链接；没有任何可用源码的 Cone 也不显示源码文件目录。源码身份和位置元数据不代表源码正文可用，不能从 `.slib` 拼接或伪造源码。这不改变正常编译对必需源文件的要求：选中的编译输入缺失仍报告实际 I/O 错误。

沿用调研的依赖方向：使用 pulldown-cmark 处理 Markdown 事件与 offset，使用 MiniJinja 和有限的内置模板生成 HTML；实际引入依赖时再核对维护状态、版本及仓库 Rust MSRV，不照抄调研日期的版本号。只保留一套 Markdown／模板实现，不建立用户模板系统。

HTML 正常转义标识符、签名、注释文本和源码。首版将 Markdown 中的原始 HTML 显示为文本；链接按明确的 URL 类型处理，代码示例只呈现、不执行。资源与站点一同保存，页面之间使用相对链接。

搜索至少覆盖 Cone／package、名称、声明种类、摘要及 DocAlias；重载结果可汇总到函数组，并能定位具体声明。公开 re-export 的名称按实际 binding 增加搜索入口，结果仍指向原声明，不创造新实体。索引通过普通静态脚本加载，在 `file://` 直接打开时可用，不依赖 HTTP fetch、CDN 或联网服务。禁用 JavaScript 时仍能沿页面导航浏览全部可见 API。

只有 prebuilt 的依赖正常用于类型解析，站点明确列出其未生成文档的状态；不把缺失可选文档升级为语言编译失败。即使 `.slib` 能提供签名与 annotation，也不能假称已有可恢复的说明正文或源码页。

M52 的 `sysroot/src` 可作为显式文档 root；处理 core 本身时沿用已有 bootstrap 根规则，不加载另一份 core。普通 `scoop doc` 保持独立命令，发行准备与文档生成由打包脚本编排。

### 8.1 内置库注释与发行文档

先完成本设计的全部工具功能及验证，再补充现有 sysroot 所有 Cone 的文档注释。逐库说明公开类型、函数、property、variant、扩展及实际使用合同；需要时说明参数、返回值、异常、资源生命周期、线程／异步要求和泛型约束，不以重复声明名称的模板文字充数。嵌套测试／辅助 Cone 也检查其源码声明说明，但不冒充发行内置库。

`package_toolchain.py` 沿既有生产库清单及该宿主支持的 target，在预编译资源准备完成后，调用本次配套 `scoop doc`，显式选择全部内置库源码 root，并以 `--no-deps` 保持完整的显式文档集合。每个 target 生成一个共同站点，跨库链接使用该 target 的实际声明目录，文档 warning 按失败处理。不能只复制开发机遗留站点，也不能因文档生成改选依赖或重建不需要的机器产物。

发行 SYSROOT 下的 `docs/index.html` 提供目标入口，站点位于 `docs/<canonical-target-triple>/`，包含全部内置库页面、内嵌源码、搜索和本地资源。Darwin 包生成 Darwin 站点，Linux 包分别生成 GNU／musl 站点；全部必要文档成功后才归档发行包，失败保留上一次完整归档。打包验收检查目标及 Cone 清单、跨库导航、源码与资源、离线搜索、更新复用和解压后的相对链接。`.slib` 仍不包含完整源码或注释正文。

## 9. 更新判断、失败与诊断

文档记录与机器编译记录分开，复用仓库既有“先判断更新，再读取内容”的原则。

| 输入或事件 | 更新范围 |
| --- | --- |
| 源码清单、源码内容状态、依赖语义选择、target 改变 | 对应 Cone 的前端文档投影及受影响链接 |
| README 或其实际引用资源改变／出现／移除 | 对应 Cone 首页与资源；不要求重跑源码 HIR |
| 文档工具／模板版本、private／hidden 展示相关输入改变 | 相应投影或渲染结果 |
| root 集合、`--no-deps`、页面可用性改变 | 全站目录、搜索及链接结果受影响的页面 |
| 仅 `--open` 改变 | 使用已成功生成的入口 |
| 仅 `--deny-doc-warnings` 改变 | 使用已有结构化诊断重新判断本次成功条件，不重跑未变化前端 |
| 所需文档输出缺失 | 补齐缺失结果；仍有效的机器产物直接复用 |

更新输入包含选中的 Cone／target／生产源码、普通文件状态、实际依赖结果、工具版本与输出选项。先检查记录和所需输出是否存在，再读取源码、README、资源、执行文档分析或探测实际需要的 native 工具。未选中 target 的源码和未进入 Production 的测试文件不使文档更新。

完整命中时直接使用已有页面和索引，不读回它们核对内容、摘要或生产过程。仅在重组站点实际需要时读取缓存的文档投影／目录，属于消费输入，不重做语义验证。注释变化会使文档更新；本期不额外设计“普通编译必定忽略注释变化”的缓存优化。

所有 root、文档诊断政策和必要输出均成功后才替换站点入口与完整结果；失败保留上一次成功站点，不发布半成品。删除／隐藏声明、移除 root 或修改版本后，旧页面和搜索项不留在新的站点集合中。复用普通临时输出、锁和发布机制；并发同一输出目录协调写入，不增加来源审计、内容复核或持久化同步协议。

parser／类型／注解规则错误仍作为正常 Error。文档 tag、悬空注释、歧义／不可解析符号链接等默认 Warning，附实际 Cone、SourceIdentity 与原 byte span；README／资源使用实际文件位置。human／JSON 使用同一结构化诊断，写 stderr，成功信息给出入口路径。

普通 build 不报告 Markdown／文档链接 warning；未写注释也不是编译错误。文档命令不能为了生成页面忽略正常函数体、默认值、泛型或可见性错误。缓存中保留文档 warning，后续切换诊断政策时不能丢失。

## 10. 实施顺序与规范变更

| 阶段 | 工作与出口 |
| --- | --- |
| M60-A：规范与文档前端 | 先确定注释、tag、身份及 Intrinsic 组合合同；添加 AST／HIR 纯数据、typed 链接与 core 注解；独立语法和诊断用例通过 |
| M60-B：单 Cone 闭环 | Doc CLI／single-Cone 协议、完整 HIR 投影、可见性／隐藏、HTML／源码页／搜索；真实命令可离线浏览 |
| M60-C：多 Cone 与更新复用 | 独立 root 图、共享依赖、生产附带文档、仅补文档、re-export／版本路由、全站目录、失败发布与增量更新 |
| M60-D：实际库与交付验收 | 先完成三目标／双宿主功能验证，再为现有 sysroot 所有 Cone 补注释，将所有内置库的三目标文档纳入 SYSROOT/docs 并验收发行包 |

各阶段先修改其对应规范，再改实现：

| 规范 | 必要修改 |
| --- | --- |
| 语言规范 §9 | 增加文档注释、附着和 tag 的有限合同；§9.4 定义两项普通 core 文档注解及 Target／Repeatable 行为 |
| 语言规范 §13.1 | 按真实 identity 增加 Intrinsic 与文档注解共存的明确例外，保持既有效果规则 |
| 实现规范 §2.1／§2.2 | AST 位置映射、文档请求的完整 HIR 输出、原作用域链接绑定、源码 API 投影 |
| 实现规范 §2.7 | Doc CLI、多 root 编排、Production 用途、协议输出意图、独立更新与发布、诊断、离线站点合同 |
| 实现规范 §2.12 | 明确文档沿既有 core／预编译选择，显式 core 源码 root 使用正常 bootstrap 规则 |

不改变 runtime ABI、GC、FFI 或 `.slib` 文档格式；M60 没有 runtime 规范变更需求。若实施发现确需改变持久化格式，必须先说明实际消费者与必要数据，再修改对应规范，不能顺带把完整源码／Markdown 放进现有 SourceRecord。

## 11. 验收矩阵与完成条件

每组围绕真实语言规则、命令行为和输出，保留独立 fixture 与组合 fixture。页面断言定位关键签名、目标链接与搜索项，不为全部库保存整站 HTML 或完整 IR 快照。

| 分组 | 必须覆盖的正例与反例 |
| --- | --- |
| CLI／输出 | 默认 cwd、目录／manifest、多 root、多个 executable、重复 root、输出路径、target、`--open` 成功／失败、错误输入 |
| 注释／位置 | 块／行／普通注释、空行合并、annotations／modifiers、悬空片段、package／import 边界、成员／参数／accessor、Unicode／CRLF、代码围栏／缩进代码、准确 tag／链接位置 |
| 源码 API | class／struct／两种 enum／interface／object、构造器／字段／property、私有 setter、泛型定义与 M55 条件、推断类型、默认参数、扩展、context／suspend、typealias、嵌套／companion、继承／覆写及可见合成成员 |
| 文档注解 | typed core identity、导入 alias／re-export、用户同名声明、Target／use-site、重复应用、Intrinsic 类型与函数组合、容器隐藏、单 overload 隐藏、私有模式、搜索 alias |
| 可见性／链接 | 全部 owner 的有效可见性、protected、split package、普通 Markdown 引用优先、overload 组、歧义／缺失目标、隐藏目标文字、跨 Cone／菱形 re-export、不同版本与同 identity 冲突 |
| 依赖／前端成本 | 源码与 prebuilt 混合、`--no-deps`、显式 core／std、冷源码依赖生产附带文档、机器命中只补文档、文档 root 不做最终链接、完整 HIR 仍拒绝正文类型错误 |
| target／Production | Darwin／GNU／musl 各自选中正确声明，不加载测试或互斥平台源码；默认与显式源码清单一致 |
| 更新／失败 | 完整命中、源码／README／资源／选项／依赖变化、缺失实际输入与输出、移除／隐藏条目、root 集变化、并发输出、警告升级、失败保留旧站点 |
| HTML／浏览 | 转义、原始 HTML 按文本呈现、源码嵌入 HTML 及站内相对行链接、源码不可用时省略展示与链接、离线搜索、DocAlias、无 JS 导航、本地资源、入口及成员链接不悬空 |

缓存验收始终保留源码、工具和原产物，通过真实调用、调用次数与生成结果证明复用；不使用删除／移走源码或工具的隔离实验，不加入缓存外部破坏恢复。

完成全部功能验证后，按 §8.1 为现有 sysroot 所有 Cone 补充文档注释；core 包括类型、集合／Option、泛型与 intrinsic，其他库覆盖各自公开 API 及真实跨库引用。按发行目标为所有内置库生成站点，检查可读签名、导航、源码与搜索，不能只验收无说明的 API 骨架。

新增 core 注解后，按[准备脚本说明](../../scripts/README.md)显式更新开发 sysroot，再运行真实依赖与文档验收；`scoop doc` 不代办 sysroot 准备，也不混用更新前后的 core 接口。

实现验证按仓库规则先执行 Rust fmt／clippy，再运行相关单元和完整正式 CLI fixture 验收；若修改 Python runner，先执行仓库指定版本 Ruff，再跑 runner 测试。三目标分别报告源码选择、依赖产物消费与相关 fixture；离线页面在 Darwin／Linux 宿主实际检查。单纯本设计文档修改只检查差异、引用与跨文档一致性，不作为实现测试通过的证据。

M60 完成需要 A～D 及内置库注释／打包文档全部闭合，并记录各目标的实际结果、失败修复和限制。文档示例执行、独立声明查询优化及可分发文档模型继续作为后续工作；其中“冷源码依赖也完全不走 backend”必须先解决前端依赖接口生产，不能从现有 HIR 观察出口推定已经具备。
