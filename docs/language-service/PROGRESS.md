# 公共语言服务基础实施记录

开始日期：2026-10-22。实施基线：`033dd9e1d`。第一分支：`codex/language-service-foundation`。

按[设计](DESIGN.md)完整交付 A～E 后，才从第一分支的完成提交创建独立 M61 分支，实施[文本后端迁移](../milestone61/DESIGN.md)。每个完成的功能经过对应格式化、lint 和验证后独立提交；当前记录不缩减任一阶段的原完成条件。

## 交付状态

| 范围 | 状态与剩余工作 |
| --- | --- |
| A：共同源码／固定查询 | 已完成源码、坐标、诊断、完整片段入口、前端生成钩子及类型／成员／导出／注解查询；默认值应用视图和逻辑正文投影、类型／成员／注解／正文 JSON 及完整类型输入解析已接入；其余声明引用解析、查询分发与实际 generator 进程闭环仍待实现 |
| B：可恢复语法 | 已完成原文 CST、缺失／错误项、局部恢复、严格投影及本机代表性正式 CLI 验证；最终 Linux 验收待后续整体执行 |
| C：首个 LSP | 已完成 stdio、文档同步、位置编码、语法诊断及大纲／折叠／选择范围；真实进程本机验证通过，Linux 验收待后续整体执行 |
| D：编辑态语义 | 未开始：独立声明／正文分析、源码绑定、hover／定义／补全／签名帮助 |
| E：索引与编辑 | 未开始：工作区引用与实现、重命名／修复、版本失效与取消 |
| 公共基础验收 | 未开始：按功能本机验证，随后 Linux 完整验证；完成后核对设计各项条件 |
| 第二分支与 M61 | 未开始：保留其全部字符集、IDNA、NFC、ABI、三目标及分发要求 |

## 已完成的功能

- 共同源码：`scoop-source` 提供共享不可变文本、按需行索引、UTF-8／16／32 坐标和诊断；AST 重导出保持调用兼容，当前 Cone 与 driver 诊断报告共享同一份文本。parser 在 lex 前拒绝无法由 Span 表示的长度。CLI 原有 LF／一基标量列保持，编辑器使用 LF／CRLF／CR 与编码单位列。
- 本机验证：`cargo fmt --all`、全 workspace／all-targets Clippy（拒绝 warnings）通过；source／AST／parser 的 548 项测试及 driver 诊断报告的 4 项定向测试通过。覆盖 Unicode、换行、EOF、无效字符边界、文本共享以及既有诊断显示兼容。LLVM 明确使用 Homebrew 22.1.8。
- 严格片段入口：公开函数体和类型解析，复用文件解析的词法、诊断整理及原有 grammar；函数体要求外层大括号，两个入口均拒绝尾随实际 token 和内部恢复错误，原文位置直接保留。文件入口及输入整理按职责拆为 parser 子模块。格式化、全 workspace Clippy、原有 524 项 parser 测试及 7 项片段专项测试通过。
- 原文 CST：使用 rowan 0.17.0，在同一 lexer 保留 trivia，在原有 grammar 记录节点及零宽预期项；文件、函数体和类型的可恢复入口均保留错误文本，严格入口仅取无错误的 AST 投影。所有语法试探改用共同检查点，回退节点、诊断、文档附着和局部 callable 编号。全 workspace Clippy 及 parser／syntax／doc 的 563 项测试通过，包括逐字符删除／插入、嵌套插值、未终止 token、EOF 和实际 grammar 的检查点回退；局部恢复后续批次仍在实施。
- 首批列表恢复：函数参数、调用实参、类型参数及类型实参使用各自分隔符／follow 恢复，保留坏项后的同级节点和已知函数体。缺失逗号与结束符记录明确 kind；类型试探只接受无错误类型，普通比较与泛型消歧保持。原有尾随逗号、空项和 variance 错误继续拒绝，EOF 级联与传播的重复诊断合并。格式化、全 workspace Clippy 和 parser 的 546 项测试通过。
- 聚合语法恢复：扩展到主／次构造器、匿名函数与 context 参数、注解实参／数组、数组表达式和元组表达式／类型，保留各自空项与尾随逗号规则。构造器项和列表 follow 规则按职责拆入子模块；修复单个 `@` 在 EOF 的越界读取。格式化、全 workspace Clippy、parser／doc 的 575 项测试通过，新增各类坏项后的正常同级项、全部源码前缀与既有文档合同验证。
- lambda／when 恢复：参数表只在消费实际 header 箭头后提交，坏参数旁的完整参数及正文保留；普通 lambda 正文的试探完整回退。when 在所属块内恢复坏 arm，并保持 case／guard／else 规则。缺失成员 selector 明确记录 Name。格式化、全 workspace Clippy 和 parser 的 556 项测试通过，覆盖正常消歧、错误分支、receiver 与后续语句。
- 模式恢复：位置、tuple 与字段模式复用各自列表边界，坏模式项后的绑定及后续 when arm 可遍历；保留 Unit 简写、括号／tuple 区别和 rest 的位置／唯一性规则。模式的原子与聚合规则拆为独立子模块。格式化、全 workspace Clippy 和 parser 的 559 项测试通过。
- 字段恢复：struct、enum 三种字段形式与注解声明参数均保留错误项后的同级字段及后续成员，沿用各自空表、默认值、不可变性、换行与逗号规则。字段 grammar 按职责拆入子模块。格式化、全 workspace Clippy 与 parser／doc 的 586 项测试通过。
- 声明语法视图：原 grammar 记录声明实际名称、范围和本次解析内的父子索引；扩展声明使用既有消歧结果，错误签名／正文仍保留已取得的名称，缺失名称保持为空。没有增加名称推测或第二套 grammar。格式化、全 workspace Clippy 以及 9 项声明／原文语法专项测试通过。
- parser 正式 CLI：使用字段恢复批次构建的 release 配套命令，显式准备 19 个系统 Cone 与正常／stress runtime；37 个代表用例、38 个变体、55 次进程调用通过，无普通功能快照。覆盖模式、注解／文档、lambda 推断和尾随调用的运行与预期诊断。同步 runtime 辅助程序的独立 Cargo.lock，补入共同源码／语法依赖；准备脚本的 `--locked` 构建通过。
- 首个 LSP：新增不依赖 LLVM 的 `scoop-lsp` 与 `scoop-analysis` 输入宿主。基于 lsp-server 0.10／lsp-types 0.97 和 url 2.5.8，提供真实 stdio 生命周期、UTF-8／16／32 文档同步、语法诊断、分层／平面大纲、折叠与选择范围。SourceId／revision 与客户端 URI／version 分离，未变化文本和语法直接复用；输入按通知原子更新，关闭后读取磁盘，打开 buffer 优先。传输与分析 worker 分离，取消请求收到响应，旧快照与诊断不覆盖新版本。格式化、全 workspace Clippy、2 项输入快照测试和 9 项真实进程测试通过；使用方式见 `compiler/lsp/README.md`。语义功能仍按 D／E 实施。
- 源码 callable 身份构造：从最终 HIR 封装中提取原有声明、owner、binder 与约束的共同构造，保留局部函数和生成模板的原身份规则，为固定查询接入提供实际复用边界。格式化、全 workspace Clippy 及 2 项既有持久身份精确测试通过；固定查询和 generator 流程仍待接入。
- 前端生成接入：HIR 数据通道提供借用查询及已解析 block 的生成钩子。普通继承选择后固定缺失普通方法的签名、参数和 requirement；查询复用现有类型绑定、子类型与身份构造，最终封装复用已取得的源码 callable 身份。所有返回正文沿原方法作用域检查，遗漏或重复返回不能产生成功 HIR。定义文本和词法输入分别记录，片段内局部声明、alias、lambda 与参数来源保持正确。格式化、全 workspace Clippy、8 项生成专项、45 项持久身份及 97 项继承／文档相关回归通过。此批只完成真实前端边界；GenerateWith 配置、外部进程协议、CLI 和完整查询面仍在后续批次实现。
- 正文与声明读取：共享查询借用当前普通 HIR、共享依赖已发布正文，并区分 Available／PendingGeneration／NotPublished／NoBody 与未知引用。定义侧默认值保留原模板、binder、前参引用和继承实参；注解按真实目标与声明 ID 读取，数组元素来源及定义位置直接复用。格式化、全 workspace Clippy、5 项当前／依赖查询专项、8 项生成回归和 3 项相关查询回归通过。应用后的默认值／成员视图及 JSON 逻辑投影仍待后续接入，不把定义侧读取视为完整协议交付。
- 完整类型查询：描述结构类型、实际名义声明、原 binder／bound、父类型和 companion 应用；查询在前端已有的写时复制视图中运行，新增类型应用不污染程序使用记录或初始化请求。类型片段沿原注解和类型规则检查，包括 alias 内嵌的非法 NoGC 实例；失败查询不影响原程序。格式化、全 workspace Clippy 和 52 项类型查询／生成／companion／NoGC 定向测试通过。字段、成员、应用后的默认值和外部协议继续实施。
- 存储与 variant 查询：返回原字段／variant 身份、声明顺序、定义侧与应用后的类型、注解目标和精确源码来源；class backing／delegate 关联保留真实 property 身份，基类与计算 property 不混入直接字段。依赖加载保留既有 foundation 中的成员来源，不改产物格式。格式化、全 workspace Clippy 和 64 项 HIR／前端定向测试通过，覆盖新查询、原静态描述、生成回归与持久成员身份。logical property／访问器、构造器和方法的完整描述仍待接入。
- logical property／访问器：读取实际 property 与 getter／setter 身份、override、定义侧与应用类型、注解和来源；访问结论使用原前端规则，保留 private setter 及 protected receiver 限制。访问器正文复用四态可用性，查询取得的源码访问器身份由最终封装复用。格式化、全 workspace Clippy 和 42 项查询／可见性／身份／生成定向测试通过。构造器、完整方法描述、应用后的默认值、导出查找与外部 generator 闭环继续实施。
- 构造器与完整参数：描述源码 primary／secondary／variant 构造器，排除内部零参适配器；参数保留 callable 与源码位置、声明／应用类型、required／default／vararg 四种调用规则及实际字段／property 关联。默认值保持原定义，并计算本次应用的 binder 实参；源码构造器身份复用到最终封装。格式化、全 workspace Clippy 和 44 项查询／构造器身份／静态描述／生成定向测试通过。完整方法描述、默认值与正文的逻辑投影、导出查找和实际 generator 进程仍待完成。
- 完整方法描述：普通声明方法保留 receiver、context／值参数、结果、effect、正文引用、原身份及访问域；方法自身的泛型 bound 同时提供定义与 owner 应用视图，沿原调用解析补全并替换参数。依赖方法在同一查询视图内复用实际 owner binder 和一次准备的自身 binder；原生类型也使用实际 core 方法。格式化、全 workspace Clippy 和 49 项查询／生成／调用解析定向测试通过，包括原参数身份、default 实参、接口默认／抽象状态和最终身份复用。导出查找、逻辑正文投影、实际 generator 和 D／E 继续实施。
- 导出查找：共享查询按明确 provider 和 type／value 命名空间读取当前公共导出与直接依赖；复用最长 package 前缀和真实 static／companion 边，provider 筛选先于前缀选择。public import 保持原声明身份，重载集合和各类声明区分保留，private／internal 不成为导出；alias 身份由最终封装复用。格式化、全 workspace Clippy 和 132 项查询／命名空间／import／持久 alias 定向测试通过。默认值与正文的逻辑投影、实际 generator 进程及 D／E 继续实施。
- 声明类型上下文：类型描述和签名投影可使用实际名义／方法声明的 binder，独立于调用方的词法上下文；宿主参数和方法自身参数保持原签名层级，跨 Cone 的同名参数不会混用，读取后恢复原上下文。格式化、全 workspace Clippy 和 33 项共享查询／生成测试通过。正文与默认值投影继续复用这一上下文，实际协议及 D／E 尚未完成。
- 共同正文投影：从原默认模板／泛型正文生产器中提取借用类型、实体与来源的输入视图；普通正文和默认模板可沿同一转换形成完整可移植逻辑树，不要求先创建成功 ExportHir。原产物路径复用该实现，保留 capture、前参、effect 与定义来源。格式化、全 workspace Clippy 和 253 项 HIR／前端的模板、共享源码及生成查询测试通过。固定查询接入及 JSON 编码继续实施，此批不表示外部 generator 已完成。
- 固定正文投影查询：按需复用原身份构造并缓存当前正文／默认模板；投影借用普通编译图，不受失败类型查询或额外 application 影响。依赖调用只读已绑定签名，不提前完成排队正文；已发布正文共享原数据，继承 default 保留原提供方、前参和 binder。源码身份回传最终封装复用。格式化、全 workspace Clippy 通过；255 项相关测试覆盖类型／局部绑定、循环、闭包、访问器、构造和继承默认值、依赖调用与缓存共享，新增用例的非法可变捕获已改为 val 快照并复验。JSON schema、默认值应用视图和真实 generator 仍在后续范围内。
- 默认值应用视图：共享原定义投影，按 definition root 的真实宿主／callable binder 应用已有类型实参；源声明、依赖和继承后的参数接口均使用同一查询。既有类型替换扩展到局部泛型签名，保留新引入的 binder，并提升插入实参的开放参数层级，原树与映射不变。格式化、全 workspace Clippy、195 项默认模板测试和 32 项共享查询测试通过；覆盖源码与依赖构造器、不同泛型元数的继承、宿主／方法参数区分、两层局部 binder 和层级溢出。协议编码和真实进程仍待实现。
- JSON 基础值：新增 `scoop-generator-protocol`，按独立的显式 schema 编码完整签名类型、typed 声明引用、源码／定义来源和普通注解值；源码与依赖注解使用同一应用形状。整数按原宽度输出精确十进制字符串，浮点保留固定宽度位模式，数组逐元素来源保持。格式化、全 workspace Clippy、6 项编码专测及 32 项真实共享查询测试通过，包括 Unicode、receiver／binder、引用类别、64 位整数、负零／NaN、嵌套注解与 enum 数组。完整成员／正文消息、双向查询及子进程接入尚未完成。
- 待填方法 JSON 签名：完整参数查询复用原声明 binder，协议保留 receiver、宿主／方法约束、context、required／default／vararg、effects、release 条件与来源。合成方法登记真实 requirement 的普通默认值继承关系，沿原路径处理实参省略与冲突；本地接口 context 参数统一使用合成方法的触发文件和范围。格式化、全 workspace Clippy 及 82 项编码／生成／依赖查询／context／调用协议测试通过，覆盖不同宿主泛型元数、跨文件位置和默认值冲突。测试发现的空替换映射与遗漏继承关系已修复，新增源码用例的语法、context key 和可见性问题已闭合；完整正文 JSON 与实际生成器进程继续实施。
- 逻辑正文 JSON：按独立 schema 借用编码现有完整表达式、语句、模式、存储、调用、capture、效果与来源，数值保留精度，局部引用使用原 selector。普通正文、默认值定义和应用视图共用编码；局部泛型签名保留自身 binder，requirement 实现片段保持原定义，并单独应用外层类型实参。格式化、全 workspace Clippy 及 52 项协议／真实前端查询测试通过，覆盖默认值与依赖来源、循环／闭包、原子内存序、指针 offset、空分支与缺失分支、开放实参的层级提升。测试 fixture 的单文件来源构造已改为已有入口并复验。完整类型／成员描述消息、双向查询及真实 generator 进程仍待接入。

- 类型与成员 JSON：完整投影类型结构、原名义参数和来源、字段／variant、logical property、构造器、方法、访问域及正文可用性。声明类型与应用类型复用已有 binder 替换，保留方法自身参数，并提升查询位置的开放实参；默认值同时保留声明映射，避免查询同一个泛型方法时发生参数捕获。Unit／Any／Ptr／函数指针等结构表示沿真实 core 声明读取成员。格式化、全 workspace Clippy 及 55 项协议／生成／依赖查询测试通过，覆盖实际字段与构造参数关联、private setter、依赖来源、重复读取、同名及同一声明参数。依赖测试发现的原生空参数加载问题已闭合。注解声明／target 消息、远程引用解析、双向查询及真实 generator 进程继续实施。

- 注解声明与 target JSON：源码和依赖统一描述原注解参数、默认常量、允许目标、repeatable、可见性及来源；嵌套 annotation／annotationArray 保留各自的 typed 引用。target 消息区分字段、variant 字段、普通／扩展 property、callable、参数、receiver、alias 与存储位置，解码仍只是待读取的引用。格式化、全 workspace Clippy 及 58 项协议／生成／依赖查询测试通过，覆盖参数位置边界、默认数组与调用处显式数组的不同来源、嵌套注解和重复读取。最小 core fixture 缺少 Target／Repeatable 的问题已通过加载真实 sysroot 注解源码闭合，未更改语言规则。远程引用解析、查询分发和实际进程接入仍待实现。

- 完整类型输入：协议解码到既有签名结构与 typed 待解析引用，共享查询按当前源码和已选依赖的真实声明索引解析，不依赖先前响应或名称回退。名义应用和 binder 复用真实参数、原泛型 bound、结构 interner、companion 与函数指针规则；源码和依赖 core 的原生表示保持。类型片段与类型引用共用诊断隔离及 NoGC／Ptr／CLayout／函数指针的原表示检查，未增加另一套 classifier。格式化、全 workspace Clippy 及 79 项协议／生成／依赖／FFI 定向测试通过，覆盖未预先读取的声明与新应用、开放 companion、声明与调用方参数、未知或错误种类引用、非法泛型实参与表示，以及失败查询后的正常编译。测试中的临时借用问题已修复并复验。其余声明引用、八族查询分发和实际 generator 进程继续实施。

## 环境与验证策略

工作文件和原始日志位于仓库外的 `/Volumes/Data/home/chenxu/work/scoop-language-service/`；命令显式设置该目录内的 scratch 为 TMPDIR，不使用 `/tmp`。保留源码、工具和普通缓存，按真实输入变化与调用结果验证更新。Linux 环境为 `ssh nuc12` 的 `~/repos/scoop`，本机功能完成后再同步和验证。

初始 target 约 105 GiB，其中 debug incremental 约 49 GiB；无活动 Cargo 构建时使用 Cargo 清理命令回收旧开发产物。后续开发检查关闭增量并减少调试信息，保留普通 release 工具与实际复用缓存；定期在构建空闲时检查空间。格式化和 lint 先于测试；Rust 编译器测试与正式 CLI／LSP 进程 fixture 按职责选择，普通库行为遵守当前 AGENTS.md 的 Cone 测试规则。
