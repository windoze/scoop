# 可恢复 parser、M47 与 LSP 调研

日期：2026-10-21。仓库基线：`4de3ae9e8`。本文记录现状、外部依据与功能需求；采纳的架构、接口边界和实施顺序见[公共基础设计](DESIGN.md)。这是设计调研，未实施 parser 改造或 LSP，也未执行它们的功能验收。

## 1. 结论

Scoop 需要把“有错仍能继续报错”推进到“有错仍保留可查询的源码结构”，并让编辑器从当前源码位置取得已有语义事实。M47 已提出的类型、成员、注解、默认值与正文查询可以作为语义核心，但单靠它的固定 HIR 视图不能回答编辑中代码的补全与导航。

共同基础应包括源码与坐标、可恢复语法、源码到语义的关联、共享语义查询及输入更新。M47 的进程会话和 LSP 的文档协议分别适配这些能力。成功编译仍须产生完整 HIR；编辑态的未知、失败与未分析状态独立表达。

推荐保留现有递归下降语法实现，以 rowan 承载保留全部源码的语法树。先完整重解析变化文件并复用其他文件，随后按实际延迟决定局部重解析和更细的语义失效。Tree-sitter、Salsa 与异步 LSP 框架都有适用场景，但不构成这次公共基础的前置条件。

## 2. 仓库现状与具体缺口

| 现有能力 | 实际行为 | 对公共基础的影响 |
| --- | --- | --- |
| [完整文件入口](../../compiler/parser/src/lib.rs)与[解析主循环](../../compiler/parser/src/parser.rs) | `Result<SourceFile, Vec<Diagnostic>>`；按声明、成员、语句边界恢复；有错误时丢弃整份 AST | 要新增保留错误结构的结果，保留现有严格编译适配 |
| [多文件入口](../../compiler/parser/src/source_input.rs) | 一个文件失败时不返回该 Cone 的 AST 集合 | LSP 不能直接把这个成功／失败结果当作全部分析状态 |
| [lexer](../../compiler/parser/src/lexer.rs)与[注释处理](../../compiler/parser/src/lexer/comments.rs) | 非法词法单元可继续诊断；普通空白／注释丢弃，文档注释另行保留；未终止块注释读至 EOF | 现有 token／AST 无法逐字节还原源码；需要保留 trivia 和错误文本 |
| [Span 与诊断](../../compiler/source/src/diagnostic.rs) | 调研时位于 `compiler/ast/src/source.rs`：文件内 `u32` byte span、Error／Warning、主位置与 notes；CLI 逐次扫描文本得到一基行列 | 现已开始提取到共同源码 crate；不能把 CLI 的 Unicode 标量列当成 LSP UTF-16 列 |
| [HIR pipeline](../../compiler/hir-lower/src/pipeline/run.rs) | 已分声明、签名、继承、正文等步骤，但多个错误出口会结束整轮 lowering | 需要逐步暴露独立的声明／正文分析结果，不能只在 LSP 外面捕获整个编译失败 |
| [M60 文档视图](../../compiler/hir/src/documentation.rs)与[持久源码记录](../../compiler/hir/src/source_record.rs) | 已有实际声明、原始签名、文档与位置关联；持久记录不含完整源码 | hover 应复用现有事实；依赖位置缺少原文时不能凭 byte offset 伪造 UTF-16 范围 |
| [M47 查询](../milestone47/PROTOCOL.md) | 从待填方法的 typed 引用出发，固定声明／已有正文视图，返回四类正文可用状态 | 保留其语义合同，增加编辑态的定位与分析宿主；不把 MethodId 或生成会话作为所有查询的必需输入 |

当前的 `SnapshotArena` 用于前端候选状态共享，不等于文档版本数据库。M47 的 `PendingGeneration` 也不等于“源码打了一半”；二者不能因为都尚无正文就共用含混的状态。

## 3. 可恢复 parser 需要的能力

| 能力 | 必须解决的问题 | 代表输入／验收 |
| --- | --- | --- |
| 保留原文 | 注释、空白、换行、字面量拼写、非法文本都可定位 | 正确与错误输入均满足树的源码文本等于原 UTF-8 文本 |
| 显式缺失与多余语法 | 区分“少了右括号”和“出现了无归属的 token” | 缺失项为零宽；错误节点覆盖实际字节；不插入可执行的默认表达式 |
| 局部恢复 | 错误尽量不吞掉同级参数、下一条语句、成员或声明 | `fun f(x: Int, y: ) {}` 后仍能查询下一函数；多余逗号后仍能识别后续参数 |
| 光标周围结构 | EOF、点号后、参数缺项、未写完的类型仍有父子关系 | `receiver.`、`call(x,`、类型位置的 `Box<` |
| 语法专有边界 | 避免把控制体当尾随 lambda、把比较当类型实参、把字符串内代码当声明 | 既有 M12、M26、M35、M37、M55 的消歧与换行组合 |
| 统一片段入口 | M47 的方法体与类型字符串使用正式语法 | 文件、带大括号的函数体、单个完整类型；片段外残余文本报错 |
| 诊断质量与进度 | 避免一次缺失产生大量重复错误或循环插入 | 每个恢复循环消费输入或完成当前语法项；独立错误按原文位置输出 |
| 可供工具遍历 | 能查询 token、父节点、范围、声明列表与注释 | 大纲、折叠、选择范围与语法高亮在 HIR 失败时仍可用 |
| 可演进的重解析 | 接受新文本版本，旧查询仍读取自己的版本 | 首先按文件更新；局部重解析以后验证与完整解析等价 |

未闭合 raw 字符串或块注释可能覆盖剩余文件，无法保证其后的文本仍被识别为声明。恢复必须尊重词法状态，不能为了保留大纲而扫描字符串里的 `fun` 猜测代码。错误恢复的目标是保留可确定的结构，不是猜出用户最终想写的程序。

## 4. LSP 的功能分层

[LSP 3.18](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/) 是调研时官网标注的当前版，3.19 为 upcoming。LSP 通过 capabilities 协商功能，不能将协议列举的所有请求都当作首版必做项；它也没有要求与 M47 一样的 `protocolVersion` 字段。

| 层次／优先级 | 功能 | 需要的共同事实 | 协议适配职责 |
| --- | --- | --- | --- |
| 服务基础，首批 | initialize／initialized、shutdown／exit、请求响应、日志、取消 | 一致分析视图、合作式取消 | stdio 分帧、capabilities、请求 ID、响应与日志分流 |
| 文本基础，首批 | didOpen／didChange／didClose／didSave，文件与配置变化 | 原文、修改范围、选中项目／target／源码角色 | URI、文档版本、编码协商、打开文档的内容归属 |
| 语法工具，首批 | 语法诊断、大纲、折叠、选择范围 | 可恢复 CST、声明边界、注释与错误范围 | Diagnostic、DocumentSymbol、FoldingRange、SelectionRange |
| 语义读取，第二批 | hover、definition／typeDefinition、语义诊断 | 实际绑定、完整类型、声明位置、M60 文档 | 客户端支持的 Markdown／纯文本与 Location／LocationLink |
| 输入辅助，第二批 | completion、signatureHelp | 光标语法上下文、可见名称、receiver 类型、候选签名、实参映射、期望类型 | replacement range、snippet、resolve、触发字符、isIncomplete |
| 导航与重构，后续闭环 | references、documentHighlight、implementation、workspaceSymbol、prepareRename／rename | 源码级 typed 引用索引、声明关系、名字冲突与访问域 | 索引范围说明、版本化 WorkspaceEdit；不直接写文件 |
| 精细呈现与修复，按需 | semanticTokens、inlayHint、codeAction／resolve、文档链接 | token 与语义分类、推断结果、带代码的诊断和具体修改 | token legend／编码、编辑版本、lazy resolve |
| 独立功能 | formatting、range／onTypeFormatting、call／type hierarchy、code lens | 前者还需格式化规则；后者需相应关系索引 | 只声明已经实现的能力；不阻塞 M47 或语法基础交付 |

关键协议细节：

1. 内部范围保持 UTF-8 byte offset。LSP 默认 UTF-16，也可协商 UTF-8／UTF-32；行号与列号零基。`a😀中` 中“中”的起点分别为 UTF-8 列 5、UTF-16 列 3、UTF-32 列 2，不能直接转换整数。
2. LSP 将 LF、CRLF、CR 都定义为行终止符；CRLF 中间不是合法位置。协议对超出行长的 character 要求归到行尾。内部字节切片仍要检查字符边界，不能切开 UTF-8 或把半个代理对当成完整字符。
3. 同一 didChange 的多项修改依次作用于上一步结果。文档版本递增但不必连续；undo／redo 也递增。打开文档由客户端提供内容，不能被文件监听事件中的磁盘文本覆盖。
4. WorkspaceEdit 中同一 TextDocumentEdit 的修改共同基于一个版本，范围不可重叠，与 didChange 的顺序语义不同。
5. 取消的请求仍需响应；过期诊断不得覆盖新版本。依赖、target 或配置变化也会使结果失效，仅比较当前文件版本不够。
6. 首版选择 push diagnostics；以后引入 pull 时再增加 resultId／unchanged 与 workspace diagnostics 的实际支持。同一个分析结果不需要重复跑两条诊断生产流程。

## 5. 外部方案与采用范围

### 5.1 rust-analyzer／rowan

[架构文档](https://rust-analyzer.github.io/book/contributing/architecture.html)明确：parser 产生树与错误，而不是只返回成功或失败；语法层独立于 LSP／Salsa；语法树不保存语义信息。当前 [parser 源码](https://github.com/rust-lang/rust-analyzer/blob/master/crates/parser/src/lib.rs)以事件描述解析结果，完整入口保证消费输入并产生单一根；[syntax 源码](https://github.com/rust-lang/rust-analyzer/blob/master/crates/syntax/src/lib.rs)保留错误文件的树。

[rowan](https://github.com/rust-analyzer/rowan)提供保留全部源码的树结构；它不替 Scoop 编写 grammar、恢复点、源码到语义关联或增量失效。采用其不可变 green tree 与按需定位的语法视图，避免自建同类树存储。保持 Scoop 现有递归下降与消歧规则，不照搬 Rust 的语法。

rust-analyzer 的 AnalysisHost／Analysis 分离对版本一致性有直接参考价值；输入宿主管理变化，查询读取固定视图。其“函数体编辑不影响全局事实”的实现经验不能无条件套到 Scoop：Scoop 的非局部推断结果、默认值和常量可能参与其他声明。还需按 Scoop 的既有语义依赖判断更新。

### 5.2 SwiftParser／SwiftSyntax

[SwiftParser 设计原则](https://github.com/swiftlang/swift-syntax/blob/main/Sources/SwiftParser/SwiftParser.docc/SwiftParser.md)区分 unexpected nodes 与 missing tokens，要求任何源码都可逐字节还原，并支持从文件、类型、表达式等入口解析及增量更新。

采用“实际多余文本”与“零宽缺失项”的区别、保留 trivia、片段解析和严格源码范围。Swift 将诊断从错误树中另行产生是其实现选择；Scoop 可保留当前 lexer／parser 的结构化诊断，不为模仿它重跑一次完整语法验证。

### 5.3 Tree-sitter

[官方介绍](https://tree-sitter.github.io/tree-sitter/)与[ERROR／MISSING 查询说明](https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html)展示了增量 CST 和错误恢复能力，适合独立编辑器语法工具。它不提供 Scoop 的类型、作用域、重载、引用与产物语义；采用时仍需这些基础。

当前 Scoop 已有完整且持续演进的 grammar。为 LSP 单独维护 Tree-sitter grammar 会增加两套语法的同步责任；全面替换又远超 M47 的片段入口需要。故本设计不采用它作为第二套权威 parser。以后独立编辑器插件若确有此需求，应作为工具适配单独评估。

### 5.4 增量与协议库

[Salsa](https://github.com/salsa-rs/salsa)提供按需计算、查询缓存与依赖失效；采用它需要先理清输入、查询粒度、循环与可取消执行。公共查询契约应能由普通现有数据视图实现，不要求整个 Lowerer 先迁入数据库。

协议优先考察 `lsp-server` 的普通消息／分帧能力。其 [Message::read／write](https://github.com/rust-lang/rust-analyzer/blob/master/lib/lsp-server/src/msg.rs)可用于同步管道；M47 不需要套用 LSP initialize 或线程化 Connection 的生命周期。LSP 自身须持续接收文档变化和取消，可由标准传输层与分析 worker 协作，不要求 M47 也改成异步。

以下是 2026-10-21 从 crates.io 核对到的稳定发布信息，不是新增依赖或锁定版本：

| 候选 | 核对版本／发布日期 | 判断 |
| --- | --- | --- |
| [rowan](https://crates.io/crates/rowan/0.17.0) | 0.17.0／2026-08-02 | 推荐语法树候选；已检查发布包 manifest，edition 2021，未声明 rust-version |
| [lsp-server](https://crates.io/crates/lsp-server/0.10.0) | 0.10.0／2026-07-16 | 推荐传输候选；发布包 edition 2024，未声明 rust-version；复用消息层与复用完整服务调度是两件事 |
| [lsp-types](https://crates.io/crates/lsp-types/0.97.0) | 0.97.0／2024-06-04 | 版本发布日期较早，采用前核对所需 3.18 类型覆盖和实际维护状况，不凭包名假定覆盖最新协议 |
| [tower-lsp](https://crates.io/crates/tower-lsp/0.20.0) | 0.20.0／2023-08-11 | 不作为默认选择；它的服务抽象也不是共享语义层 |
| [tower-lsp-server](https://crates.io/crates/tower-lsp-server/0.23.0) | 0.23.0／2025-12-07 | 有社区后续维护，声明 Rust 1.85；若实际选择异步服务可继续评估 |
| [line-index](https://crates.io/crates/line-index/0.1.2) | 0.1.2／2024-10-18 | 可参考 UTF-16／UTF-32 转换；所读当前源码按 LF 建索引，采用前必须补齐 LSP 的 CR／CRLF 边界合同 |
| [ropey](https://crates.io/crates/ropey/1.6.1) | 1.6.1／2023-10-18 | 大文本更新的候选；首版先用共享 UTF-8 文本，是否引入取决于实际编辑开销 |

未声明 rust-version 不代表已经兼容仓库的 Rust 1.85。引入依赖时需用实际锁定的发布包、传递依赖和仓库工具链验证；不因调研时上游主分支使用更高版本就自动提高 Scoop 的工具链要求。Salsa 的 crates.io 元数据查询本次超时，本文只依据其官方设计资料，不记录未经核对的版本。

## 6. 对 M47 的直接修订

M47 的通用查询仍覆盖实际语义事实，codec 算法继续由 generator 负责。需要补充的是：

- 将输入文本、片段诊断与范围纳入共同源码基础；M47 的方法体与 interface 类型字符串成为统一片段入口的实际调用方。
- 共享查询接收普通语义上下文；MethodId 到上下文的映射留在 generator 适配层，位置到上下文的映射由分析服务完成。
- 将“编辑中不完整源码、索引更新都属于 LSP 适配层”收敛为准确分工：协议生命周期属于 LSP，语法恢复、局部语义事实、源码关联及分析失效属于公共分析能力。
- 保留固定生成视图和四类正文状态；LSP 的未知／未分析状态不冒充 PendingGeneration，也不要求 M47 v1 增加编辑器会话字段。
- M47 可以先落地源码／片段入口和固定视图查询，后续可恢复 CST、编辑态分析与 LSP 分批接入。完整 LSP、局部重解析和增量数据库均不成为 codec 迁移的新验收门槛。

最终设计与验收按[公共基础设计](DESIGN.md)执行，语言行为与阶段边界以[实现规范 2.20](../specs/SCOOP-IMPL-SPEC.md#220-可恢复语法与语言服务公共基础设计合同待实施)为目标合同。
