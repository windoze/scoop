# 公共语言服务基础实施记录

开始日期：2026-10-22。实施基线：`033dd9e1d`。第一分支：`codex/language-service-foundation`。

按[设计](DESIGN.md)完整交付 A～E 后，才从第一分支的完成提交创建独立 M61 分支，实施[文本后端迁移](../milestone61/DESIGN.md)。每个完成的功能经过对应格式化、lint 和验证后独立提交；当前记录不缩减任一阶段的原完成条件。

## 交付状态

| 范围 | 状态与剩余工作 |
| --- | --- |
| A：共同源码／固定查询 | 已完成源码、坐标、诊断及完整片段入口；共享语义上下文与实际 generator 闭环仍待实现 |
| B：可恢复语法 | 已接入保留原文的 CST、缺失／错误项及严格投影；正在细化列表、表达式与控制结构内的局部恢复 |
| C：首个 LSP | 未开始：stdio、文档同步、位置编码、语法诊断和结构查询 |
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

## 环境与验证策略

工作文件和原始日志位于仓库外的 `/Volumes/Data/home/chenxu/work/scoop-language-service/`；命令显式设置该目录内的 scratch 为 TMPDIR，不使用 `/tmp`。保留源码、工具和普通缓存，按真实输入变化与调用结果验证更新。Linux 环境为 `ssh nuc12` 的 `~/repos/scoop`，本机功能完成后再同步和验证。

初始 target 约 105 GiB，其中 debug incremental 约 49 GiB；无活动 Cargo 构建时使用 Cargo 清理命令回收旧开发产物。后续开发检查关闭增量并减少调试信息，保留普通 release 工具与实际复用缓存；定期在构建空闲时检查空间。格式化和 lint 先于测试；Rust 编译器测试与正式 CLI／LSP 进程 fixture 按职责选择，普通库行为遵守当前 AGENTS.md 的 Cone 测试规则。
