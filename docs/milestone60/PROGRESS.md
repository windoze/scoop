# M60 实施记录

状态：实施中。开始日期：2026-10-21。基线：`abf17c32`；分支：`codex/m60`。范围与完成条件见 [DESIGN.md](DESIGN.md)。

## 分项状态

| 分项 | 状态与验证 |
| --- | --- |
| 设计与路线图 | 已提交；已核对本地引用、M52 当前状态及源码布局 |
| M60-A：文档前端与注解 | 已完成；文档注释、Markdown／tag、普通 core 注解、typed HIR 视图与正式命令诊断通过三目标专项验证 |
| M60-B：单 Cone 闭环 | 已完成；渲染输入、协议／driver、HTML／源码页／搜索、CLI、类型链接与继承／覆写展示通过三目标专项验证 |
| M60-C：多 Cone 与更新复用 | 已完成；独立 root、共享依赖、文档缓存、页面复用及版本／并发／资源组合通过三目标专项验证 |
| M60-D：实际库及跨平台验收 | 19 个生产库与 7 个嵌套 Cone 注释已补齐；三目标实际库功能验证通过，完整 CLI 回归与发行包验证继续进行 |

## 工作与验证约定

临时文件、测试 TMPDIR 和原始日志放在仓库外的本次工作目录，不使用 `/tmp`。按功能先格式化／lint，再做本机定向验证并提交；Linux 使用 nuc12 的 `~/repos/scoop`，在本机功能闭合后完成全量验收。开始时本机 target 约 3.4 GB；阶段间检查占用并清理不再使用的构建结果，保留仍用于验证的配套工具和 sysroot。

## A1：文档注释保留与附着

同一 lexer 保存块／行文档的原始文本和 byte span，普通注释与字符串保持原行为；parser 在实际声明、参数、variant、构造器及 accessor 语法入口建立精确关联，悬空片段保留为后续文档诊断输入。AST 的关联是文件内语法位置，后续 HIR 仍须绑定真实 typed 声明。长主构造器解析移入现有 constructors 模块。

组合测试发现无 body 的 class 会误吞下一声明的可见性／注解；修正换行处主构造器前缀的解析边界，保留跨行显式 constructor，增加独立回归。

本机 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过；AST 15 项、parser 520 项及 parser doctest 1 项通过，其中新增 10 项文档独立／组合测试。正式 `scoop doc`、文档 warning、HIR 投影及 Linux 验收尚未完成，不从 parser 结果推定完成。

## A2：普通 core 文档注解

新增 `DocHidden` 和可重复的 `DocAlias(String)`，复用普通注解的 Target、静态参数、use-site、alias、re-export 与产物保存。Intrinsic 类型及函数允许实际 core 文档注解；其他普通注解仍被拒绝，原有 NoGC／Unsafe 要求不变。为真实 Int、String 和 currentSourceLocation 添加代表性说明与搜索别名。

本机 fmt／workspace clippy 通过；47 项注解相关 HIR 测试通过（包含新增 6 项），11 个三目标通用 CLI fixture 在 Darwin 通过，共 14 个进程。用例检查错误消息与准确 byte span，并实际编译／运行普通注解、同名用户注解及跨 Cone 重导出。配套 release 命令已重建，开发 sysroot 的 19 个库与 runtime 已按维护脚本完整更新。Linux 验收留到功能闭合后进行。

清理约 729 MiB 本阶段不再使用的 MIR／LIR／codegen 增量缓存，保留当前前端测试及配套命令。原始日志与 fixture 工作区保存在仓库外。

## A3：Markdown、tag 与原始位置

新增 `scoop-doc` 文档 stage；正文、tag、待绑定路径、跨 tag 的显式 Markdown 引用定义及 byte 映射使用 AST 纯数据。仅消费 lexer／parser 保留的片段，不重新发现声明。pulldown-cmark 负责 Markdown 结构，代码、图片和显式 URL 不成为符号候选；反引号标识符中的点与空格保持原意。未知／缺参 tag、非法符号语法和悬空片段产生带原始位置的文档 warning，语义目标仍留给 HIR 绑定。

引入前已核对 pulldown-cmark 0.13.4 的维护状态及 Rust 1.71.1 要求，低于 workspace 的 1.85 基线；仅启用实际需要的 HTML 功能。MiniJinja 3.0.1 的 Rust 1.70 要求也已核对，待渲染阶段实际引入。

本机 fmt／workspace clippy 通过；14 项文档语法测试与 15 项 AST 测试通过，覆盖 Unicode／CRLF、装饰前缀／缩进、连续与分隔片段、8 类 tag、代码／HTML 边界、显式引用优先、quoted path、精确 warning span。此阶段尚未提供 `scoop doc` CLI，正式文档 warning fixture 随 CLI 闭环接入。

## A4：HIR 文档视图与引用绑定

CurrentConeSources 接收与选中源码身份、顺序一致的整理后注释；正常 HIR 完整成功后，Output 附带独立文档视图与 warning。声明沿原 AST／typed 目标关联，保存作者签名片段与已完成的推断类型，覆盖主／次构造器、字段／property／accessor、variant payload、annotation 参数、泛型 binder、context、扩展 receiver 和源码可见合成方法。局部函数及其子项不进入视图；内外泛型 binder 保持各自身份，主构造 property 说明不复制为参数说明。

文档链接复用正常 package、import、成员、继承及 direct dependency 查询，保留完整 overload 集、alias 身份及原文件作用域。tag 绑定实际目标，直接说明优先；缺失、歧义、不适用与重复描述产生准确 SourceIdentity／byte span 的文档 warning。查询使用独立语义副本，不增加普通机器调用选择；注释上下文复用现有写时复制存储，避免重载探测重复复制全文。

本机 fmt／workspace clippy、16 项 HIR 文档定向测试通过，包括真实 core 全部源码的文档请求、跨 Cone alias／成员、同名导入歧义、Unicode／CRLF、M55 条件、嵌套 binder、合成 equals、局部声明排除、普通构建无文档 warning，以及函数体错误仍拒绝输出。真实 core 的 DocHidden／DocAlias 按其 annotation identity 返回，已有说明正确关联且没有文档 warning。测试修正了 struct 字段与自动访问 property 的重复候选，并验证 context 参数直接归属 logical property。

本阶段未提供 CLI、持久化渲染输入、可见性筛选、继承关系的页面展示或 HTML；这些继续在 B／C 完成，不能从 HIR 测试推定站点验收已通过。清理约 498 MiB 不再使用的后端增量缓存，保留前端缓存、release 配套工具及开发 sysroot。Linux 完整验收仍在本机功能闭合后执行。

## B1：可缓存渲染输入与展示筛选

`scoop-doc` 直接消费完成后的 Export HIR、文档视图和已读源码，生成可序列化的展示输入；保留原签名、原 binder 名称、分段 package、源码／warning 位置、绑定链接及公开 re-export 的真实目标。路由通过现有 annotation target 的持久身份投影统一生成；参数使用实际 owner 和位置，派生 equals 使用 nominal owner 与派生角色，object 的类型／值 binding 指向同一源码条目。输入顺序与 arena 变化不改变对应路由。

公开筛选使用 effective lookup／inheritance domain，支持公开 open 类型的 protected 成员，以及 sealed 类型经公开 open 后代提供的扩展点；私有容器不泄漏公开子项。property 保存 getter／setter 的实际可见性。DocHidden 作用于真实目标及文档后代，字段与 logical property 分开，构造参数及源码原文保留；DocAlias 按 core 身份收集、合并重复关键词，并对空白关键词保存准确 warning。用户同名注解不产生文档效果，private 展示模式不取消隐藏。

本机 fmt／workspace clippy 和 24 项 HIR 文档测试通过（新增 8 项投影组合测试）；真实 core 全部源码完成展示投影且无文档 warning。覆盖 JSON 输入往返、Unicode／CRLF、泛型推断类型、M55／context、重载及稳定地址、private setter、sealed／protected、精确 use-site 与注解 alias。测试修正了 inheritance domain 自身 subclass 约束的递归解释。清理四份已被替代的前端增量缓存，约 5.16 GiB 文件逻辑大小，保留当前验证缓存及 release 工具。

此提交提供站点的渲染输入，尚未接入 single-Cone 文档协议、CLI、页面目录／HTML／搜索、继承成员页面或增量发布；完整命令与 Linux 验收继续实施。

## B2：single-Cone 文档协议与编译编排

配套协议升级至 12，以闭合的输出选择表达普通产物、仅文档及产物附带文档；成功结果对应携带真实产物摘要、文档输入位置与独立文档 warning。仅文档不使用虚构 `.slib`，不与 observation 混用，限 Production manifest 输入与 AST／HIR dump。

driver 复用完整正常 HIR，在同一次结果中投影文档；仅文档随后结束，联合输出在机器生产成功后发布文档输入。普通编译不整理文档。仅文档使用已有目标注册表的纯选择，不探测 C 工具链或进入 MIR／LIR、native 编译、runtime 和最终链接；依赖仍按原产物合同加载。文档 JSON 通过同目录临时文件替换，正文检查失败保留旧结果。

本机 fmt／workspace clippy 无警告；31 项协议测试及两组 driver 集成测试通过。集成测试以真实 core 验证 Darwin／GNU／musl 三目标的仅文档路径，以普通库验证 Production 排除测试目录、Unicode／CRLF warning 的准确 span、DocAlias、HIR dump、联合与普通编译产物一致、独立 warning 以及正文错误不覆盖旧输入。协议测试覆盖三种输出往返、非法用途／输入／dump 组合和独立文档诊断；同步更新协议 frame 的长度断言。

本次尚未提供用户侧 `scoop doc`、HTML／搜索、站点缓存与发布。正式 CLI fixture 和 Linux 完整验证继续随 B／C／D 闭合。

## B3：全站目录与静态页面渲染

先根据完整 Cone 身份、实际 typed 声明和筛选结果建立页面目录，再生成 Cone／package／类型／函数组、成员及参数锚点、源码行和共享搜索。函数重载保留独立声明地址；跨声明作用域的解析结果生成列出全部可见目标及其签名／所属 Cone 的组页。显示名称与输出路径分开，root 顺序、同名条目和不同 package 分段不覆盖页面。缺少文档目标保留文字；隐藏条目及其 re-export 不进入所选页面和搜索。

引入 MiniJinja 3.0.1；已复核官方包、发布记录和 Rust 1.70 要求。模板显式开启 HTML 转义，Markdown 原始 HTML 按文本呈现；签名、源码、标识符、tag 和 property getter／setter 能力使用实际投影。README 渲染消费调用方提供的资源路由，不自行读取或扫描文件。搜索使用本地静态索引脚本，包含名称、种类、所属 Cone／package、摘要、alias 和 re-export；无 JavaScript 时仍可从导航进入页面。

本机 fmt／workspace clippy 无警告，19 项文档测试通过（新增 5 项渲染组合测试）。验证跨 Cone／overload 链接、隐藏与 private 筛选、源文件转义、成员／accessor 锚点、README 资源和非法活动 URL；HTML parser 检查样例 6 页、46 个本地链接、唯一锚点及资源存在，搜索脚本通过 Node 语法检查。浏览器工具的 URL 安全策略拒绝 `file://`，本次未把实际离线浏览器验证计为通过。

清理 66 份已被替代的增量快照，约 3.59 GiB 文件逻辑大小，保留每个 crate 配置的最新快照。渲染层尚未接入用户命令、README 资源复制、更新记录与原子站点发布；继承／覆写关系的页面内容、真实库站点和 Linux 验收继续完成。

## 源码展示规则补充

按新增要求明确源码原文嵌入生成的 HTML，API 只链接站点内源码页和行锚点；渲染输入没有对应文本时，省略源码页、链接及空源码目录。`.slib` 继续仅保存 SourceRecord 元数据，完整源码与注释正文只进入本地文档输入。正常编译的必需输入缺失仍是 I/O 错误。

本机 fmt／workspace clippy 无警告；7 项渲染测试通过，其中新增两项覆盖嵌入文本、HTML 转义、原始文件地址不进入输出、部分／全部源码不可用。两项 driver 集成测试通过，确认联合生成文档与普通生产的 `.slib` 一致，归档不含测试的完整源码和文档正文。源码不可用用渲染输入直接表达，没有删除、移走或隐藏实际编译输入。

## B4：扩展导航与派生成员条件

扩展函数／property 保留实际 receiver 类型与声明链接，类型页列出扩展及其声明 Cone，扩展仍使用原 package 页面。派生 equality 页面展示字段／payload 在原 binder 中的类型要求；跨 Cone 链接可以指向未具体化的条件派生声明，沿普通同签名成员规则排除用户自定义 equality，不新增机器调用或具体化结果。

推断 primitive 类型导航分别使用 Defined／Imported core 协议中的真实声明身份，类型参数链接原 binder；不会因某类型尚未触发成员加载而漏掉导航。真实 core 投影测试覆盖自身的推断 Int／Boolean／String。

本机 fmt／workspace clippy 无警告，28 项相关 HIR 测试、7 项渲染测试和两项 driver 集成测试通过。首次测试发现导入 primitive 链接误依赖按需成员表，已改为直接读取完整 core 协议并复测通过。覆盖扩展函数和 property、M55／context 组合、generic／conditional／显式 equality、原 binder 名称，以及文档查询不改变机器派生选择。

## B5／C1：真实文档命令与站点发布

接入 `scoop doc` 的多 manifest root、目标／输出选择、private、no-deps、warning 政策和成功后打开入口。各 root 独立构建图，按实际 identity 合并相容源码任务；图发现和文档根使用纯目标选择，只有需生产依赖产物时准备 native 输入。复用现有机器缓存、single-Cone runner 和发布锁；源码依赖冷产物联合生成文档，已有机器产物只补文档，文档根不做机器生产或最终链接。

前端文档缓存按实际生产源码状态、依赖选择及配套编译器更新，完整命中不读源码或调用编译器。README 与实际引用资源独立检查状态，复制资源并重写站内地址；缺失资源保留精确 README 位置。站点在同一父目录完成暂存，使用系统目录交换原子替换；类型错误、文档 warning 升级和实际 I/O 失败均保留旧站点。JSON 结果包含站点与各 Cone 入口、渲染状态及真实子编译调用。

本机 fmt／workspace clippy、Ruff 通过。原有 87 项 Scoop 库测试中，协议 11→12 后遗漏更新的固定缓存向量失败，修正后定向复测通过，其余 86 项通过；13 项 CLI 测试、3 项 crate 边界测试、2 项 README 资源测试、53 项 fixture runner 测试通过。正式文档 CLI 的 3 个用例、4 个变体、16 个进程在 Darwin 通过，覆盖完整命中、README／private 更新、缺失页面补齐、Production／目标输入、两个 executable root、冷依赖联合生产、机器命中补文档、显式 root 与 no-deps、root 移除、精确文档／语言诊断及失败保留旧站点。

清理 71 份已被替代的增量快照，约 4.19 GiB 文件逻辑大小。当前站点变化仍统一重渲染所有 Cone；按链接影响范围复用页面、继承／覆写展示、真实库内容和 Linux 完整验收继续实施，尚不宣告 M60 完成。

## C2：按页面输入与链接变化更新

把共享页面、各 Cone 的 API 页面、首页／README 资源分开记录。站点完整命中仍直接返回；需要更新时消费已缓存的文档投影，按真实引用目标的可用地址、overload 组、re-export 可见性、反向扩展说明和全站导航判断受影响的 Cone。保持有效的页面以普通文件链接加入新的暂存站点，不读回页面核对内容；缺失结果只重生成对应页面组，发布仍使用同一原子目录交换。

本机 fmt／workspace clippy 无警告，9 项渲染测试通过。3 个正式 CLI 用例、4 个变体、18 个进程通过：README 改动只渲染对应首页；只修改一个 root 的说明时，仅调用该 root 的编译器，更新其页面和共享索引，复用其他 Cone 页面；private、root 移除、缺失页面与失败保留旧站点仍通过。现有的继承／覆写等功能缺项与双宿主完整验收继续推进；之后按更新后的设计完成所有 sysroot Cone 注释和发行文档。

## B6：显式签名类型链接

正常类型解析保存实际类型语法的位置；文档完成时在原声明作用域绑定这些位置，将源码签名拆成保留原文的链接片段。别名链接自身声明，泛型参数链接原 binder；元组、函数及泛型应用里的各个类型独立导航，不从任意源码单词猜测类型。查询产生的临时类型仍留在文档语义副本中。

本机 fmt／workspace clippy 无警告，25 项 HIR 文档测试通过，包括真实 core 全部源码、跨 Cone、泛型／别名及原始签名保持。静态站点检查另已覆盖 CLI 输出 29 页、212 个本地链接，无缺失页面／锚点、重复 ID 或原始文件 URL。

## B7：继承成员与覆写来源

复用普通 receiver 查询、泛型替换、接口成员选择和已检查的 dispatch／property 覆写关系，生成父类型、实际继承成员及覆写来源。继承签名保留 owner／method 的独立 binder、M55 条件、context、effect、默认参数与 vararg 能力；默认值摘要标为省略号，原声明页仍保留作者的完整签名。菱形继承去重，被覆写成员不重复列出，说明正文不自动继承。DocHidden 与公开／protected 展示规则同样作用于继承目录，未生成依赖页面时保留签名与来源文字。

本机 fmt／workspace clippy、Ruff 通过。27 项 HIR 文档测试、10 项渲染测试通过，补充 effect 展示后的两个继承用例复测通过；覆盖同 Cone／导入 Cone、类与接口、独立泛型 binder、M55、默认参数、属性、菱形继承、隐藏成员、接口实现与覆写、缺页文字及链接输入变化。查询副本没有增加普通 Export HIR 类型或机器调用。正式 CLI 新用例在 Darwin 的全部依赖／no-deps 两变体、4 个进程通过，静态检查所有页面的本地链接、唯一锚点、继承签名、re-export 与嵌入源码。尚未进行 Linux 完整验收或开始 sysroot 全库注释。

## C3：缺少依赖页面时的隐藏筛选

依赖绑定从已经加载的 typed 元数据取得真实声明与词法容器，文档投影据此省略隐藏的 re-export。嵌套类型、object 值和函数不会因为 no-deps／prebuilt 缺页重新出现在包目录；继承成员的隐藏判断也计入外层容器。无需读取依赖源码或产物复核。

本机 fmt／workspace clippy、Ruff 通过。27 项 HIR 文档用例中 26 项通过，新扩展用例首次因测试 import 位置不合法失败，修正后定向复测通过；正式 CLI 全部依赖／no-deps 两变体、4 个进程通过。另清理 52 份已被替代的增量快照，约 5.51 GiB 文件逻辑大小，保留当前工具与每个 crate 配置的最新快照。

## B8：签名中的类型命名空间与 setter 能力

签名类型链接按类型命名空间选择声明，同名的值参数不会遮蔽实际类型别名；普通正文引用仍允许参数链接。继承的可变属性展示实际 getter／setter 可见性，导入属性使用已有 accessor 声明，避免把 private setter 显示为公开写能力。

本机 fmt／workspace clippy、Ruff 和 27 项 HIR 文档测试通过。正式继承 CLI 的两变体、4 个进程通过，验证同 Cone／跨 Cone 的私有 setter、缺少依赖页面时的能力摘要及完整站内链接。

## C4：版本、菱形重导出与并发／资源发布

新增正式 CLI 组合用例：两个 executable root 分别选择同库的不同版本，同一版本经两个库重导出；检查真实类型／别名链接、并发更新同一输出目录、后续完整命中，以及同 identity 冲突输入失败保留旧站点。README 缺失资源的准确诊断、warning 升级不发布、资源补齐和单独更新均由真实命令验证。

Ruff 通过；Darwin 用例 1 个变体、11 个进程通过，所有站内页面／锚点与资源链接有效，资源更新不重新编译源码。53 项 fixture runner 规则测试通过。随后开始全工作区验证；真实库注释与发行集成仍按约定在功能验证后进行。

## 完整验证中的基线测试修正

首轮全工作区测试到 codegen 时发现既有 C 布局探针仍期望 metadata ABI 8／ImageDescriptor 240 bytes；当前规范及头文件已在基线提交中改为 ABI 9／208 bytes。同步这两项测试常量，本机 fmt／workspace clippy 与该探针定向复测通过；runtime 实现与 ABI 均未改动。已通过的 Scoop、ABI plan、AST 和 codegen 其他单测不重复全量执行，继续剩余工作区测试。

剩余工作区验证发现 21 项 driver 测试仍要求发布内部协程保存槽及 driver／adapter；现行规范和实现已将这些内部实体排除在公共接口之外。调整测试的公共／私有边界，保留公开有限类型、装箱 receiver slot、布局、字段身份及产物往返的精确断言。fmt／workspace clippy 通过，19 项 core 布局测试、1 项有限类型测试及 1 项 bootstrap 测试全部定向复测通过；其余工作区单测、集成测试与 doctest 已通过，没有修改生产 ABI 或导出实现。

Darwin 完整正式 CLI 验收发现旧夹具将 Int 移到 hidden package 时漏导入新增 DocAlias；补齐显式导入后，该夹具的 2 个进程复测通过，全量验收继续运行。Linux 在独立工作树准备配套工具及 GNU／musl 资源，不改动原 checkout。

## D1：真实内置库的功能验证

使用配套 release 工具显式选择 19 个内置库，以 no-deps／deny-doc-warnings 在 Darwin 生成共同站点：19 次前端调用、2,163 个输出文件。静态检查通过 2,160 个 HTML 页面、134,027 个本地链接、733 个内嵌源码页面及 6,065 个搜索条目，没有缺失页面／锚点、重复 ID 或原始文件 URL。现有源码产生 27 条普通编译 warning，文档 warning 为零；浏览器工具限制仍在，不将静态检查冒充实际离线浏览器验收。三目标功能验收完成后再开始全库注释与发行文档集成。

Linux 全工作区单测、集成测试与 doctest 已执行完毕；首轮 14 项 runtime／linker／toolchain 测试因遗漏 SCOOP_SYSROOT 找不到已准备的 unwind 资源，补齐环境后全部定向复验通过，85 项 driver 测试直接通过。两目标的文档 CLI 用例均通过；需要运行程序的 3 项注解用例留待更新后的 runtime 单独复验。GNU／musl 完整 CLI 回归已经启动。

维护脚本的独立 runtime_builder 锁文件漏列新增文档依赖，使 --locked 拒绝构建。补齐 5 个依赖包，不更新已有包版本；本机 fmt／workspace clippy、独立 locked/offline 解析通过，Linux 使用该锁文件完成 GNU／musl 各 19 个内置库与 runtime 的维护脚本准备。另清理 61 份已被替代的本机增量快照，约 7.44 GiB 文件逻辑大小，保留最新配置快照及正在使用的产物。

## D2：首批内置库说明

M60 新功能的 Darwin／GNU／musl 专项用例全部通过，Linux 的三个 annotation 正例在更新后的 runtime 上复验闭合。GNU 的 19 库共同站点通过 2,160 页、142,252 个页面及搜索入口链接、733 个源码页和 6,065 个搜索条目的静态检查。三个目标的完整 CLI 回归仍在运行。

为 charset、cli、idna、json、test、terminal.asyncio、tls 和 tls.asyncio 补充 305 处声明说明，包括编解码错误、流状态、资源所有权、异步取消、测试生命周期及真实类型引用。修正 annotation 声明注释应位于 modifiers／annotations 之前的附着位置后，fmt／workspace clippy 和严格文档生成通过；8 库站点 191 页、9,497 个本地链接、73 个内嵌源码页检查通过，没有文档 warning。

core 补充 1,071 处说明，覆盖数值与转换规则、集合／Option、operator、原子内存序、协程、显式资源和 FFI 生命周期、序列化、inspection／debug 及 typed reflection。严格文档生成通过，339 页、18,771 个本地入口链接、67 个源码页与 1,362 个搜索条目有效。musl 的 19 库功能站点也通过与 GNU 相同规模的静态检查。

URL、crypto、compression 和 asyncio 补充 568 处声明说明，明确不可变 URL 查询、密码学验证边界、压缩结束与资源所有权、异步 I/O 及取消规则。严格文档生成通过；前 3 库 170 页／7,662 个本地入口、asyncio 99 页／5,074 个本地入口静态检查有效，文档 warning 为零。fmt／workspace clippy 通过。

platform 与 http.asyncio 补充 577 处说明，覆盖文件／进程／网络资源、原生缓冲区借用、HTTP 消息生命周期、多路复用取消、multipart、SSE 和 WebSocket。严格文档生成通过，275 页／15,581 个本地入口／99 个源码页／690 个搜索条目有效；文档 warning 为零，现有普通编译 warning 不影响文档验收。

core 注释使内置声明字节位置改变，同步 6 个 core-negative 诊断夹具的位置，类型、消息与其他字段不变，16 个用例／16 个进程复验通过。另补齐既有非法 @Test + @Intrinsic 组合新增的文档注解兼容性诊断，该用例单进程复验通过。

terminal 补充 376 处说明，覆盖 ANSI 输出与刷新、坐标与范围、输入解析、查询匹配、raw mode 恢复、颜色策略及剪贴板协议边界。fmt／workspace clippy、严格文档生成通过，156 页／7,020 个本地入口／44 个源码页／535 个搜索条目有效；文档 warning 为零。

std 补充 843 处说明，覆盖集合与排序、惰性序列、Option／Result、字节编解码、文本、日期时间、随机数与 UUID。fmt／workspace clippy、严格文档生成通过，468 页／22,401 个本地入口／123 个源码页／901 个搜索条目有效，编译及文档 warning 均为零。

http 补充 710 处说明，明确 HTTP/1 定界与切换、HTTP/2 流控／输入输出进度、字段解析与原始字节、Cookie／公共后缀、multipart、SSE、WebSocket 握手和帧状态。fmt／workspace clippy、严格文档生成及页面／入口静态检查通过，文档 warning 为零。

http.client 补充 565 处说明，覆盖请求体重放、响应消费、认证／重试、缓存／Cookie、连接池／代理、超时／观察、下载续传、SSE 和 WebSocket。fmt／workspace clippy、严格文档生成通过，240 页／14,259 个本地入口／139 个源码页／627 个搜索条目有效，文档 warning 为零。至此 19 个生产库的公开声明说明均已补齐；一并清理注释展开引入的行尾空白。

## D3：全库注释归属与嵌套 Cone

为 7 个嵌套测试／辅助 Cone 补齐 README、测试入口和原生工作生命周期说明。严格文档生成通过，31 页／584 个本地入口／12 个源码页有效；4 个消费者 Cone 的 9 项实际测试全部通过。只有 native 符号的辅助 Cone 在 C 导出入口上说明合同，不添加没有声明目标的 Scoop 注释。

逐一对照手写说明与真实 HIR 文档投影，修正 6 个同一行 annotation／声明导致的说明错位；保留已有 Int 说明和后续修订的不可变 URL 说明，并明确随机数位选取与零位不推进状态。19 库共同站点通过 2,160 页／159,545 个本地入口／733 个源码页／6,065 个搜索条目检查；文档 warning 为零。fmt／workspace clippy 通过。

## D4：发行文档生成与安装

新增短小的打包文档模块：生产资源准备完成后，用本次配套工具为全部内置库逐目标生成严格文档；全部成功后才组装和归档。安装到 SYSROOT/docs，提供目标选择首页，并排除仅供工作目录更新判断的站点记录。同步打包说明、随包说明与 release notes。

Ruff 0.16.10 通过。Darwin 实际调用打包模块完成 19 库生成、安装与全部页面／链接检查；原输入和工具保持存在，重复生成用时约 0.1 秒，2,183 个工作站点文件的时间戳全部保持不变。完整 Darwin 打包停在已有 native 准备脚本的 CMake 配置，原生界面工具报告宿主锁定，已请求手动解锁；Linux 完整发行包验证继续进行。这些结果尚不代替完整归档及解压验收。

完整 Darwin CLI 回归中 10 个 HTTP peer 失败来自 PATH 选到 Python 3.9，预期 socket timeout 未被该版本的 TimeoutError 捕获。把 PATH 设置为实际 runner 的 Python 3.15 后，全部 10 项／20 变体／60 进程定向复验通过；没有修改生产代码或放宽断言。

## D5：完整 CLI 回归的后续闭合

另 5 项 TLS／HTTP peer 依赖较新的 Python 类型语法或 TLS 能力，修正同一 PATH 问题后全部通过，合计 10 变体／32 进程。至此该环境问题涉及的 15 个用例均已定向闭合。

两个旧链接夹具也已闭合：重定位 String 的前缀补充 DocAlias 导入，并以明确的声明／runtime helper 边界分开原文件，保留文档注释，避免用非嵌套块注释包裹带文档的类正文；正文常量变化的用例保留链接内容指纹变化、artifact-only／普通 build 指纹一致及 42→43 的实际运行断言，删除要求符号结构文本也必须变化的旧断言。fmt／workspace clippy、Ruff 0.16.10 与两个用例／20 进程复验通过。

清理本轮已经通过并退出的 fixture 临时目录，保留失败目录、运行中的用例、共享缓存与原始输入；本机释放约 170 GiB，Linux 两目标释放约 152 GiB。另清理 18 份已被替代的 Rust 增量快照，约 0.26 GiB，保留各配置的最新快照。原始日志及清理记录仍在仓库外。

Darwin 完整正式 CLI 已结束：发现 4,520 项，首轮 4,481 通过、22 失败、17 不适用，执行 5,263 个变体／17,980 个进程／89 个专用 golden。逐项对应修正后的定向 report，22 项失败全部闭合，因此 4,503 个适用项没有未闭合失败。没有重复启动全量验收；GNU／musl 全量及完整发行包验证仍在进行。

## D6：Darwin 归档与解压验收

复用已验证的 Darwin native 资源，重建当前配套 release 工具，并经现有 prepare_sysroot 更新 19 个内置库和 runtime；随后实际调用生产代码中的文档生成、组装、安装及归档函数，生成完整工具链归档。没有替换这些函数，也没有移走原源码、工具或产物。该路径验证新的文档交付链路；独立 CMake native 重建的阻塞仍保留为宿主环境限制，未宣称冷重建全部 native 依赖通过。

归档解压到带空格的新路径后，目标入口及 19 库清单正确，2,160 页／159,545 个本地入口／733 个嵌入源码页／6,065 个搜索条目全部有效，站点工作记录未进入发行包。三个包内命令的版本查询通过；使用包内默认 SYSROOT 的 crypto／TLS 消费者仅编译 root，运行输出正确，再次构建没有 child invocation。工具的相对 LLVM 加载路径有效。浏览器工具限制仍未解除，未把静态页面检查计为浏览器交互验收。

## D7：Linux 完整回归的定向闭合

GNU 的两个既有 C++ negative 夹具同时使用自己的 wrapper 和公共 link_args，runner 显式选择 CC 时重复传入 --cc，提前触发参数错误。改为只传目标和夹具自己的 wrapper，保留预期工具链诊断；完整发现／schema 检查及 Linux 两个用例／两个进程复验通过。继续保留原始全量工作树和失败现场，不中途更新运行中的输入。

独立固定工作树使用最新夹具和注释源码，定向复验 core 诊断、Intrinsic 注解组合及两个链接夹具：GNU 21 项／39 进程、musl 19 项／37 进程全部通过。原全量回归继续运行，最终按实际失败逐项关联这些报告。

## D8：Darwin 完整打包入口与失败保留

将打包工作目录改到仓库外、Documents 之外后，同一 CMake 的正常配置通过；无需修改 CMake 或打包代码，也不再依赖解锁宿主。完整 package_toolchain.py 入口重建原生依赖、19 个内置库和 runtime，严格生成文档，产出工具链及 LLVM 源码两个归档。D4／D6 的 native 重建限制由本次结果闭合。

完整入口产出的工具链再次解压到带空格的路径，19 库、2,160 页、159,545 个本地入口、733 个源码页及 6,065 个搜索条目检查通过。包内三个命令、相对 LLVM 加载路径、默认 SYSROOT 冷构建／实际运行／再次命中均通过；实际结果明确使用解压根内的 6 个依赖及 runtime index。

随后只给 README 增加真实的缺失资源引用，再调用完整打包入口。SCOOP_DOC_RESOURCE 导致命令失败，两个旧归档及 2,184 个工作文档文件的大小／时间戳全部不变，未留下待发布归档；README 已恢复。没有修改或隐藏源码、工具及原产物来证明复用。浏览器交互仍受工具 URL 限制，保持为明确的人工检查限制。

## D9：Linux 验证资源与大型客户端时限

同机另有编译回归运行，两目标各 7 路并发时出现构建超时；使用 runner 的中断清理保存完整发现集及逐项结果，保留已通过项，只继续尚未闭合的用例。降到每目标 3 路后，musl 的 Basic、Bearer 和 Bearer 生命周期用例完整通过，GNU 的异步 HTTP/1 用例也通过；但部分大型客户端 release 编译仍超过默认 300 秒。为 50 个客户端集成夹具的 build 步骤明确设置 900 秒，其他步骤与全部诊断、运行及缓存断言保持原样。完整发现／schema 检查通过，后续只补验未闭合项。

新验证工作树漏复制公共 C 头文件导致一项 HTTP/2 原生输入失败；补齐真实准备资源后，该用例两变体／6 进程复验通过。Linux 完整打包已生成两目标严格文档，组装时发现未安装 patchelf；从系统软件源下载到本次工作目录后继续完整入口。新增工具搜索路径属于实际原生编译输入变化，维护脚本按既有更新规则重建受影响资源，归档验收仍在继续。
